//! 屏上那一块：**报告区**——主区最下面那一段，边跑边攒的那一份
//! （`CONTEXT.md` 的《会话》：报告区）。
//!
//! 措辞出自 [`crate::render`]：命令行与会话共用那几个函数，一个字都不在这里重写。
//! 一卷跑完那条事件带着那一卷的报告（ADR 0011），[`crate::render::volume`] 收下它
//! 就画得出判定、定档页、隔离与这一趟怎么读的。
//!
//! **两副都是表**：默认那一副一卷一行（[`super::table`]），展开那一副一页一行
//! （[`super::pages`]）——同一套视口、同一套砍列、同一组语义色，
//! 哪几列、砍哪几列在 [`crate::session::columns`]（那一块在终端库外面）。
//! **卷表从这一格的头一行起**：这一趟的抬头（profile、适配方式、裁边、跨页拆分、
//! 判据构成与聚合）从前摆在它上面、跟着表滚，`p3-session-legibility/12` 把那几行
//! 收进了[覆盖层](crate::session::state::Overlay::Premises)——`i` 一个键调得出。
//! 表**下面**是当场冒出来的失败页、以及收场之后末尾那几小结——
//! 那几段本来就是句子，照旧当整段文字折行画（[`crate::wrap`]）。
//!
//! **两副样子由展开与否分**（`CONTEXT.md` 的《会话》：展开），差在哪几处见
//! [`report_pane`] 那张表。折行走 [`crate::wrap`]，本模块只交代**折到多宽**——
//! `--help` 与命令行印出来的报告折的是同一套，而那两处根本没有终端库。
//!
//! 这一段在主区占几行由 [`super::main_pane`] 分；上面那一块总览在 [`super::overview`]。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use std::path::Path;

use super::directories::directories;
use super::pages;
use super::paint::{Painted, Tone};
use super::table::{Table, table};
use crate::session::live::{Live, Volume};
use crate::session::state::{Focus, Follow, Session};
use crate::session::viewport::Viewport;

/// 报告区：**边跑边攒**的那一份，措辞出自 [`crate::render`]。
///
/// 卷级那几行一卷跑完就画得出来（一卷跑完那条事件带着那一卷的报告，ADR 0011）：
/// 判定、定档页、隔离、这一趟怎么读的、幂等命中说清哪四项依据没变，全在里面。
/// 失败页要在出现的当场看得见，那一段走 [`crate::render::failing_pages`]。
///
/// **两副样子，由展开与否分**（`CONTEXT.md` 的《会话》：展开）：
///
/// | | 目录表（默认） | 展开一枝（卷表） | 展开一卷（逐页表） |
/// |---|---|---|---|
/// | 一行是 | **一枝**（[`super::directories`]） | **一卷**（[`super::table`]） | **一页**（[`super::pages`]） |
/// | 列的是 | 此刻摆得出的那几枝 | 那一枝底下那几卷 | 那一卷[要紧的页](crate::render::notable)，`a` 切全部页 |
/// | 钉住的抬头 | 无（这一趟的前提在[覆盖层](crate::session::state::Overlay::Premises)里，`i` 调得出） | 无 | **有**：这一卷的基准档 · 定档页 · 列着几页 |
/// | 长过一格 | **跟随时钉在末行，跟随停了就跟着[光标那一卷](crate::session::state::Follow)走** | 同左 | 跟着[光标那一页](crate::session::state::Expansion::at)走 |
/// | 一行放不下 | 按固定次序**砍列**（[`crate::session::columns`]） | 同左，另一个次序 | 同左，第三个次序 |
/// | 成句的那几段 | 折行（按显示宽度，见 [`crate::wrap`]） | 折行 | 折行 |
/// | 左栏 | 在场 | **在场**（这一级摆得下） | 收起（见 [`super::shell`]） |
///
/// **表下面那两段（失败页、末尾那几小结）说的是整趟**，与表列的是哪一级无关：
/// 前两副都摆着它们（见 [`collapsed`]）。
///
/// 默认那一副跟着最新收摊的那一卷，是因为报告只增不减，而「一卷跑完当场看得见」说的正是
/// 刚添上去的那一行；**光标一挪跟随就停了**，往回翻因此不必另有一个滚动量
/// （`p3-session-legibility/10`，那正是停车场 Q64 记着的缺口）。
///
/// **两副的滚动量与滚动条都由 [`Viewport`] 出**（`p3-session-legibility/11`）：
/// 光标在哪儿视口就跟到哪儿，屏上没有一处记着「滚到哪儿了」（`CONTEXT.md` 的《视口》）。
/// 展开那一副从前有横竖两个滚动量，横的那一个连同「逐页不折行、横着滚」一起没了——
/// 它此刻是一张表，横向摆不下时**砍列**。
pub(super) fn report_pane(
    frame: &mut Frame,
    area: Rect,
    session: &mut Session,
    live: Option<&Live>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        // 抬头摆不下时从中间省略，不由终端库硬截（[`super::yielding::title`]）。
        .title(super::yielding::title(
            &report_title(session, live),
            area.width,
        ));
    // 边框各占一格，正文因此只剩这么大。
    let inside = Rect::new(
        0,
        0,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let Some(live) = live else {
        let rows = Painted::plain(NOT_RUN_YET.to_owned()).folded(inside.width);
        frame.render_widget(Paragraph::new(rows).block(block), area);
        return;
    };
    let Some(expansion) = session.expansion() else {
        // **焦点落在这一块上才反白**：屏上只有一处反白，而它说的恒是
        // 「就在这一行上动手」（见 [`super::config::config`]）。两级都算——
        // 目录表与卷表都是这一块，焦点落在哪一级上都该看得出光标停在哪儿。
        let shown = collapsed(
            live,
            inside.width,
            session.standing(live),
            matches!(session.focus(), Focus::Report | Focus::Opened(_)),
            session.opened(),
        );
        // **视口跟着光标走**（[`Viewport`]），而[跟随](Follow)那一档跟的是**末行**：
        //
        // - **跟随着**：末行。表底下还有当场冒出来的失败页与收场之后那几小结，
        //   而它们说的正是最新那几卷的事——钉在最新那一卷那一行上会把它们推出格子。
        //   这一档与从前那一副（「滚到底」）**逐格相同**。
        // - **跟随停了**：光标那一行。「翻回去看第一卷」于是不必另有一个滚动量
        //   （`CONTEXT.md` 的《视口》：滚动量是算出来的，不是记着的）。
        //
        // 光标指不着谁（一卷都没有）时同样退回末行。
        let last = shown.rows.len().saturating_sub(1);
        let cursor = match session.follow() {
            Follow::Latest => last,
            Follow::Stopped(_) => shown.cursor.unwrap_or(last),
        };
        let view = Viewport::new(shown.rows.len(), usize::from(inside.height), cursor);
        super::scrolling(frame, area, Paragraph::new(shown.rows).block(block), &view);
        return;
    };
    // **展开着的那一卷此刻指的是哪一卷**：它收摊了的话，「攒着的那一份」那个位置
    // 已经归下一卷了，而这一格要给的是**它自己**（[`Live::nearest`]）——
    // 不解析这一道，下一个决策点一到，屏上就会拿另一卷的逐页冒充它
    // （`p2-loose-ends/08` 那条硬约束朝前的那一半）。
    let Some(volume) = live
        .nearest(expansion.volume)
        .and_then(|at| live.volume(at))
    else {
        let rows = Painted::plain(GONE.to_owned()).folded(inside.width);
        frame.render_widget(Paragraph::new(rows).block(block), area);
        return;
    };
    // **抬头钉在这一格顶上，表在它底下滚**：逐页翻到第三屏时「这一卷判成哪一档」
    // 还得答得出来，而那正是翻这几页要比的东西（票面：抬头钉住这一卷的）。
    let [pinned, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(block.inner(area));
    let opened = pages::pages(
        volume,
        live.report().profile.panel(),
        body.width,
        expansion.listing,
        expansion.at,
    );
    // 光标收进这一副**真列出来**的那几页里（列头那一行不算）。不收的话，翻过了头再
    // 翻回来，头几下会按了没反应（见 [`Session::clamp_report`]）。
    session.clamp_report(opened.table.rows.len().saturating_sub(1));
    let (rows, cursor) = folded(&opened.table, body.width, true);
    // 一页都没列出来时那一格里摆的是一句话，光标无处可停——视口退回头一行。
    let view = Viewport::new(rows.len(), usize::from(body.height), cursor.unwrap_or(0));

    frame.render_widget(block, area);
    frame.render_widget(
        // 钉住的这一行也是一个抬头：摆不下时同样从中间省略。它钉在框**里**、
        // 一格边框都不占，走的因此是 [`super::yielding::pinned`] 那一支
        // ——与边框上那几个抬头差的只有「两个角占不占格」。
        Paragraph::new(
            Painted::plain(super::yielding::pinned(&opened.heading, pinned.width)).line(),
        ),
        pinned,
    );
    frame.render_widget(Paragraph::new(rows).scroll((view.from(), 0)), body);
    // 滚动条画在**正文那一段**的右边：抬头钉着不滚，跟着整格一起算就差一行。
    super::scrollbar(
        frame,
        Rect {
            y: area.y.saturating_add(1),
            height: area.height.saturating_sub(1),
            ..area
        },
        &view,
    );
}

/// 展开着的那一卷此刻指不着谁时，这一格里说什么。
///
/// 到得了的只有一种：报告整个换了一趟（又按了一次试算或执行，而报告一趟一份）。
/// 决策点上那一卷收摊之后照旧指得着它自己——那一道由 [`Live::nearest`] 收在前面。
const GONE: &str = "这一卷不在这一趟的报告里了：⇥ 换一卷，Esc 收起回卷表。";

/// 把一张表折成这一格摆得下的那几行，**光标那一行整行反白**。
///
/// 第二个数是光标落在**折出来的**第几行上：表上一行摆不下时会折成几行，
/// 而视口要的是屏上那个行号（[`Viewport`] 收的就是它）——两张表因此都不许自己数一遍。
///
/// **反白只在焦点落在这一块上时给**：屏上同一刻只有一处反白
/// （`CONTEXT.md` 的《会话》：焦点）。展开着的时候焦点就在这一块上，恒给。
fn folded(table: &Table, width: u16, focused: bool) -> (Vec<Line<'static>>, Option<usize>) {
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut cursor = None;
    for (row, said) in table.rows.iter().enumerate() {
        let here = table.cursor == Some(row);
        if here {
            cursor = Some(rows.len());
        }
        let folded = said.folded(width);
        rows.extend(match here && focused {
            true => highlighted(folded),
            false => folded,
        });
    }
    (rows, cursor)
}

/// 一趟都还没跑过时这一格里说什么。
const NOT_RUN_YET: &str = "
 按 t 试算：只算不写，报告照出。
              按 x 执行：写到输出根。
              跑起来之前必填的两项是型号与输出根。";

/// 默认那一副的正文：**卷表 · 失败页 · 末尾那几小结**。
///
/// **这一趟的前提不在这里了**（`p3-session-legibility/12` 票面第三条）：profile、
/// 适配方式、裁边、跨页拆分、判据构成与聚合那几行从前摆在表**上方**、跟着表滚
/// （`p3-session-legibility/08`），此刻收进[覆盖层](crate::session::state::Overlay::Premises)
/// ——`i` 一个键调得出。它们是「这一趟的前提」，一趟只说一次，在长任务里没人会反复看，
/// 而它们占的正是卷表要的行。措辞照旧只有 [`crate::render::header`] 一处。
///
/// 表以外的那几段**都是句子**，照旧当整段文字折行（[`crate::wrap`]）：
/// 末尾那几小结、以及没做成那一卷的那句原因（它跟在表上那一行的行尾，
/// 摆不下时整行折下去）。**一句都没塞进格**——拆开没有意义，而拆的那一刀会把措辞
/// 挪到排版这一层来（ADR 0016）。
///
/// 与命令行印出来的那一份**同源不同副**：同样的行、同样的格，摆法两副。
fn collapsed(
    live: &Live,
    width: u16,
    at: Option<Volume>,
    focused: bool,
    opened: Option<&Path>,
) -> Collapsed {
    let report = live.report();
    // **表从这一格的头一行起**：抬头那几行挪进覆盖层之后，表上方一行都不剩，
    // 光标那个行号因此不必再往下推一段。
    let (mut rows, cursor) = folded(&level(live, width, at, opened), width, focused);
    // **失败页是「出事」那一档**（spec 的《语义色》），而这一段头一行就叫「失败页」——
    // 颜色不是唯一载体（见 [`super::paint`]）。
    rows.extend(
        Painted::new(
            crate::render::failing_pages(live.failed_pages()),
            Tone::Trouble,
        )
        .folded(width),
    );
    // 末尾那几小结要看完整趟才给得出来（见 [`crate::render::tail`]），因此只在收场之后画。
    if live.ended() {
        // **末尾那几小结不上色**：那是一串小结（非卷文件 · 宽溢出 · 兜底上界 · 部分救回 ·
        // 隔离 · 卷级失败）拼出来的一段文字，六小结分属三档语义，而整段只上得了一种色。
        // 按小结分色要先把那一段拆开，而拆它就是把措辞挪进画法这一层（ADR 0016）——
        // 停车场 Q155 记着这一笔。表上那几行已经把同一批事按卷说了一遍。
        rows.extend(Painted::plain(crate::render::tail(report)).folded(width));
    }
    Collapsed { rows, cursor }
}

/// **光标停着的那一行：整行反白**（`CONTEXT.md` 的《会话》：焦点）。
///
/// **反白不是语义色**，因此不住在 [`super::paint`] 里（那一层的模块文档
/// 《不是语义色的那几样》把它与加粗、压暗一起划出去了）：它说的是
/// 「就在这一行上动手」，`NO_COLOR` 也不抹掉它。**语义色照旧上**——
/// 打的是折好的那几行的补丁（`patch_style`），一行既可以是「出事」的那一种，
/// 又可以是光标此刻停着的那一行。
///
/// 与左栏那一处（[`super::config`]）各自反白自己那一行，而**屏上同一刻只有一处**：
/// 焦点在哪一块由 [`Focus`] 说了算，两处各问它一次。
fn highlighted(rows: Vec<Line<'static>>) -> Vec<Line<'static>> {
    rows.into_iter()
        .map(|row| row.patch_style(Style::new().add_modifier(Modifier::REVERSED)))
        .collect()
}

/// **这一格此刻摆的是哪一级的表**：展开着一枝就是那一枝的[卷表](super::table)，
/// 否则是[目录表](super::directories)（`volume-discovery/08`）。
///
/// **展开着的那一枝此刻不在了时退回目录表**。这一支眼下**到不了**——展开进来的那一枝
/// 恒是算出来的一枝（见 `super::super::expand`，答不出就说一句、不进那一级），
/// 而报告换一趟要先按 `t`／`x`，那两个键在这一块上不派（得先 `⇥` 回左栏，
/// 那一下焦点就不在这一级上了）。留着是因为退一级仍旧有东西可看：
/// 与展开一卷那一处说一句 [`GONE`] 不同——那一处整格只有那一卷，
/// 而这一处说一句反而把屏上仅有的那份报告也遮了。
fn level(live: &Live, width: u16, at: Option<Volume>, opened: Option<&Path>) -> Table {
    let branches = live.branches();
    let branch =
        opened.and_then(|directory| branches.iter().find(|branch| branch.directory == directory));
    match branch {
        Some(branch) => table(live, width, at, branch),
        None => directories(live, &branches, width, at),
    }
}

/// 默认那一副画出来的那几行，外加**光标停在第几行**。
///
/// 两样装在一个类型里而不是一对裸值，与 [`Table`] 同一条理由：它们是同一次拼出来的，
/// 而「第二个数是什么」在调用处看不出来。
struct Collapsed {
    rows: Vec<Line<'static>>,
    /// 卷表上那个光标落在 [`rows`](Self::rows) 的第几行。**一卷都没有时是 `None`**
    /// ——那时视口退回末行（见 [`report_pane`]）。
    cursor: Option<usize>,
}

/// 报告区那一格的抬头。展开时说清**展开的是哪一卷、它是第几卷**。
///
/// 非说不可：展开出来的逐页那几行自己不说它属于谁（卷级那一行早翻到几十行之外去了），
/// 而换过一卷之后，屏上第一眼看不出换没换成。卷名走
/// [`crate::render::volume_name`]——命令行的进度条与会话的当前卷条印的是同一个，
/// 这里不另取一个名字。
///
/// **这里只说展开的是哪一卷，一个键都不提**：按键提示的家是屏底那一行
/// （`p1-session/10` 立的那一条），两处都摆就有了第二份措辞。
/// 那一行在展开着时恒摆着收起的键，因此抬头退回裸「报告」也不会把出路一起弄丢。
pub(super) fn report_title(session: &Session, live: Option<&Live>) -> String {
    let Some(live) = live else {
        return "报告".to_owned();
    };
    let Some(expansion) = session.expansion() else {
        // **与正文同一道解析**（见 [`level`]）：展开着的那一枝此刻不在了
        // （报告整个换了一趟）时正文退回目录表，抬头跟着退回裸「报告」——
        // 两处各说各的话，屏上就会写着「展开 某某」而底下摆的是目录表。
        let mut title = match session
            .opened()
            .and_then(|directory| nth_branch(live, directory))
        {
            Some(said) => format!("报告 · 展开 {said}"),
            None => "报告".to_owned(),
        };
        if stopped_following(session) {
            title.push_str(&format!(" · {FOLLOW_STOPPED}"));
        }
        return title;
    };
    // 与正文同一道解析（见 [`report_pane`]）：抬头说的必须是这一格真画着的那一卷。
    let Some(opened) = live.nearest(expansion.volume) else {
        return "报告".to_owned();
    };
    let Some(volume) = live.volume(opened) else {
        return "报告".to_owned();
    };
    // **「第几卷」数的是这一枝底下那几卷**，不是整趟（`volume-discovery/08`）：
    // `⇥` 换一卷只在这一枝里转（见 `super::super::expand`），拿整趟当分母的话，
    // 屏上那个数指的是一个按不到的集合。那一枝找不着（报告换了一趟）就退回整趟——
    // 说少了比说错了好。
    let volumes = live
        .branches()
        .into_iter()
        .find(|branch| branch.volumes.contains(&opened))
        .map_or_else(|| live.volumes(), |branch| branch.volumes);
    let at = volumes
        .iter()
        .position(|listed| *listed == opened)
        .unwrap_or_default();
    format!(
        "报告 · 展开 {}（第 {}/{} 卷）",
        crate::render::volume_name(&volume.volume),
        at + 1,
        volumes.len()
    )
}

/// 展开着的那一枝在抬头上怎么写：`库/甲（第 2/7 个目录）`。
///
/// **那一枝此刻不在了就是 `None`**（报告换了一趟）：抬头那时退回裸「报告」，
/// 与正文退回目录表是同一道解析（见 [`level`]）。
///
/// **印的是全路径**，与目录表那一列同一条（见 [`super::directories`]）：两枝的末一段
/// 常常一模一样，而这一行要答的正是「摊开的是哪一枝」。摆不下时从中间省略
/// （[`super::yielding::title`]），与别的抬头一个待遇。
fn nth_branch(live: &Live, directory: &Path) -> Option<String> {
    let branches = live.branches();
    let at = branches
        .iter()
        .position(|branch| branch.directory == directory)?;
    Some(format!(
        "{}（第 {}/{} 个目录）",
        directory.display(),
        at + 1,
        branches.len()
    ))
}

/// 报告区那一格的抬头上说**跟随停了**的那句话（`CONTEXT.md` 的《会话》：跟随）。
///
/// **措辞只有这一处**：屏底那一行摆的是那个键（`g 回到跟随`，见
/// [`super::footer::report_prompt`]），不重说这件事——按键提示的家是屏底，
/// 状态的家是抬头，与展开那一副「第几卷在抬头上、键在屏底」同一条。
const FOLLOW_STOPPED: &str = "跟随停了";

/// **这一刻该不该在抬头上说「跟随停了」**。
///
/// 两个条件：光标真的[挪出去过](Follow::Stopped)，而且**报告还在长**（跑着或等答话）。
/// 收场之后不说——那时报告不再长，跟着最新那一卷与停在某一卷上没有分别，
/// 屏上多一句是噪音。`g` 那个键照旧摆得出来（它仍旧把光标挪回最后一卷）。
fn stopped_following(session: &Session) -> bool {
    matches!(session.follow(), Follow::Stopped(_)) && session.stage().read_only()
}

/// 这一趟有没有一卷可以展开。**与 [`super::super::expand`] 挡在前面的那两条同一个判据**：
/// 没跑过、或者一卷都还没有，展开就无从谈起。
///
/// 问的是 [`Live::volumes`]（收摊了的那几卷，**外加决策点上攒着的那一份**），
/// 不是 `report().volumes`：那一份也展得开（`p3-session-legibility/10`），
/// 两处判据要是各问各的，就会有一个「按得动却不摆在屏上」的键。
pub(super) fn expandable(live: Option<&Live>) -> bool {
    live.is_some_and(|live| !live.volumes().is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::super::overview::OVERVIEW_HEIGHT;
    use super::super::probe::{
        a_run_in_flight, every_kind_of_volume, main_snapshot, only_branch, opened_snapshot,
        reversed_row, same_screen, screen, snapshot_of, tight,
    };
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::{Expansion, Key, Listing, Step};
    use crate::wrap;
    use tonefit::Mode as RunMode;

    /// **展开着头一枝**时报告区那一副的正文（卷表 · 失败页 · 末尾那几小结）。
    ///
    /// 卷表那几条问的是那一副长什么样，而它此刻要先展开一枝才摊得出来
    /// （`volume-discovery/08`）——夹具里那几卷都躺在同一个目录底下，因此恒是头一枝。
    fn opened_rows(live: &Live, width: u16) -> Collapsed {
        let branch = only_branch(live);
        collapsed(live, width, None, false, Some(&branch.directory))
    }

    /// 一趟**跑完了**的两卷：一卷幂等命中，一卷[每种页各一张](fixture::a_page_of_every_kind)。
    ///
    /// 展开那几条要的是**跑完的**那一份：这几条问的是那一副画成什么样，
    /// 而收场之后报告一行不少、也不再长——跑着与等答话时也展得开
    /// （`p3-session-legibility/10` 推翻了停车场 Q72），那两档各有自己那一张快照。
    fn a_run_worth_expanding() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 180));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.volume_finished(&fixture::a_page_of_every_kind("卷二"));
        let report = live.report().clone();
        live.returned(Ok(report));
        live.rewind(Duration::from_secs(300));
        live
    }

    /// 展开着的一个会话，连同那一趟跑完的报告。
    ///
    /// 展开那一下**列的是要紧的页、光标停在头一页上**（[`Expansion::new`]）——
    /// 与 [`super::super::press`] 那一层给的一模一样：这一副只画那一卷，
    /// 「视口对到那一卷的抬头上」因此不必再算一个行号。
    fn expanded(volume: usize) -> (Session, Live) {
        let live = a_run_worth_expanding();
        let mut session = Session::new();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(volume)));
        (session, live)
    }

    /// **决策点上那一卷展得开，摊出来的是它自己的逐页**
    /// （spec 的《焦点与两维模式》第五条，`p2-loose-ends/08`）。
    ///
    /// 它停在**攒着的那一份**上、不在收摊了的那几卷里，而 08 那条硬约束记着
    /// 「不许摊开上一卷冒充它」。展开的索引因此从「报告上第几卷」改成
    /// [`Volume`]（`p3-session-legibility/10`）——一个下标根本指不到这一卷上，
    /// 而这一改就是那条约束的解法；本票的逐页表接着走它。
    ///
    /// 两卷特意造得**认得出来**：收摊了的那一卷八页、要紧的六页，攒着的那一卷一页。
    /// 只比卷名的话，两卷的头一页都叫 `001.jpg`，冒充了也看不出来。
    #[test]
    fn the_volume_waiting_at_the_decision_point_expands_into_its_own_pages() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::a_page_of_every_kind("卷一"));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::processed_volume("卷二", None)),
        );
        let mut session = Session::new();
        session.run_started();
        session.at_the_decision_point(true);

        // 焦点切到报告区：跟随着的时候光标停的正是这一卷（它是表上最后一卷）。
        session.press(Key::Tab);
        let waiting = Volume::Summarized { after: 1 };
        assert_eq!(session.standing(&live), Some(waiting));

        // 展开它：抬头说得出展的是哪一卷，逐页那几行是**它自己的**。
        session.expand(Expansion::new(PathBuf::from("库"), waiting));
        let shown = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(shown.contains(&tight("展开 卷二")), "{shown}");
        assert!(shown.contains(&tight("第 2/2 卷")), "{shown}");
        assert!(shown.contains(&tight("要紧的页 1/1")), "{shown}");
        // 上一卷（收摊了的那一卷）的逐页一行都不许冒充它：它那几页一页都不在。
        // 比的是**页名**：那几个词（特例页、宽溢出）总览块的出事行上也有一份，
        // 而这一格里说的是哪几页。
        for hers in ["004.jpg", "005.jpg", "006.jpg", "007.jpg", "017.jpg"] {
            assert!(
                !shown.contains(&tight(hers)),
                "上一卷的 {hers} 冒充了：{shown}"
            );
        }

        // **它收摊之后，这一格给的仍旧是它自己**（`Live::nearest`）：
        // 「攒着的那一份」那个位置这时归**下一卷**，而展开着的是刚才那一卷。
        let mut settled = live.clone();
        settled.volume_finished(&fixture::processed_volume("卷二", None));
        settled.volume_started(Path::new("库/卷三"), 1000);
        settled.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::a_page_of_every_kind("卷三")),
        );
        let after = tight(&screen(&mut session, Some(&settled), 120, 40));
        assert!(after.contains(&tight("展开 卷二")), "{after}");
        assert!(after.contains(&tight("要紧的页 1/1")), "{after}");
        // **报告边跑边长，展开的那一副跟着更新**（票面第四条）：多出来的那一卷进了
        // 「共几卷」那个数，而展开着的仍是刚才那一卷。
        assert!(
            after.contains(&tight("第 2/3 卷")),
            "报告长了，抬头没跟上：{after}"
        );
        assert!(
            !after.contains(&tight("004.jpg")),
            "下一卷冒充了展开着的那一卷：{after}"
        );
    }

    /// **快照：焦点在左栏、在目录表、在卷表、跟随停了，四张**（票面第八条）。
    ///
    /// 四张钉的是**屏上看得出焦点在哪**（票面第一条）与**跟随停了屏上说一句**
    /// （票面第三条）。反白不进快照（[`snapshot`] 比的是字，而反白是样式），
    /// 因此另问一句「此刻反白的是哪一行」（[`reversed_row`]）——
    /// 那一处正是「焦点在哪一块」屏上唯一的载体。
    ///
    /// **中间多出来的那一张是目录表**（`volume-discovery/08`）：报告区默认那一副
    /// 一个目录一行，反白落在**光标那一卷所在的那一枝**上——屏上那个光标恒是一卷，
    /// 这一副只是把它归到一行上。展开那一枝才是卷表，反白这才落到那一卷自己身上。
    ///
    /// 跑着的一趟：**焦点切到报告区不解锁任何一个改动键**（票面第四条），
    /// 左栏三层因此照旧压暗、抬头照旧写着只读——四张里一张都没变。
    #[test]
    fn the_focus_and_the_stopped_follow_are_both_visible_on_screen() {
        let live = a_run_in_flight(false);
        let mut session = Session::new();
        session.run_started();
        let shot = |session: &mut Session| snapshot_of(session, &live, 96, 20);
        let cursor = |session: &mut Session| {
            reversed_row(
                |frame| super::super::shell(frame, session, Some(&live)),
                96,
                20,
            )
        };

        // 一、焦点在左栏：反白落在配置那一行上……只是这一趟正跑着，三层只读，
        // 而只读时左栏一格都不反白（见 [`super::config::config`]）——
        // 「跑着时光标不反白」那一条不因两维松动。
        same_screen(&shot(&mut session), FOCUS_ON_THE_CONFIG);
        assert_eq!(cursor(&mut session), None, "跑着时左栏还反白着");

        // 二、`⇥` 把焦点切到报告区：默认那一副是**目录表**，反白落到
        // **最新收摊的那一卷所在的那一枝**上，屏底换成这一块的键，
        // 而按停那一副跟在后面。
        session.press(Key::Tab);
        same_screen(&shot(&mut session), FOCUS_ON_THE_REPORT);
        let branch = tight(&cursor(&mut session).expect("焦点在报告区，该有一行反白"));
        assert!(branch.contains("库"), "反白没落在那一枝上：{branch}");

        // 三、展开那一枝：卷表摊出来，反白这才落到**最新收摊的那一卷**自己身上。
        session.open(only_branch(&live).directory);
        same_screen(&shot(&mut session), FOCUS_ON_THE_VOLUMES);
        let standing = tight(&cursor(&mut session).expect("焦点在卷表，该有一行反白"));
        assert!(
            standing.contains("卷二"),
            "反白没落在最新那一卷上：{standing}"
        );

        // 四、光标往回一卷：跟随停了，抬头说一句，屏底多出 `g 回到跟随`，
        // 反白跟着落到上一卷上。
        session.select(&live, Step::Back);
        same_screen(&shot(&mut session), THE_FOLLOW_STOPPED);
        let back = tight(&cursor(&mut session).expect("该有一行反白"));
        assert!(back.contains("卷一"), "光标没往回挪一卷：{back}");

        // `g` 交回给跟随：屏上那一句没了，反白回到最新那一卷上。
        session.press(Key::Char('g'));
        same_screen(&shot(&mut session), FOCUS_ON_THE_VOLUMES);
    }

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const FOCUS_ON_THE_CONFIG: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告──────────────────────────────────────┐"
"│口味层 · 这一趟的立场                             █│ 记号  目录  卷数  基准档分布             │"
"│  适配方式　　　　默认（height）                  █│ ✓     库       2  跳过 1 · 4bit 1        │"
"│  裁边　　　　　　默认（裁）                      ║│                                          │"
"│  跨页拆分　　　　默认（拆）                      ║│                                          │"
"│  拆分阈值　　　　默认（1.5）                     ║│                                          │"
"│  阅读方向　　　　默认（rtl）                     ║│                                          │"
"│  滤波器　　　　　默认（lanczos3）                ║│                                          │"
"│  位深　　　　　　自动（判据说了算）              ║│                                          │"
"│  抖动　　　　　　自动（判据说了算）              ║│                                          │"
"│  逐页　　　　　　默认（关）                      ▼│                                          │"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" ⇥ 报告区 · 跑着…… · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷"
" ） · ? 全部键                                                                                  "
"                                                                                                "
"#;

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const FOCUS_ON_THE_REPORT: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告──────────────────────────────────────┐"
"│口味层 · 这一趟的立场                             █│ 记号  目录  卷数  基准档分布             │"
"│  适配方式　　　　默认（height）                  █│ ✓     库       2  跳过 1 · 4bit 1        │"
"│  裁边　　　　　　默认（裁）                      ║│                                          │"
"│  跨页拆分　　　　默认（拆）                      ║│                                          │"
"│  拆分阈值　　　　默认（1.5）                     ║│                                          │"
"│  阅读方向　　　　默认（rtl）                     ║│                                          │"
"│  滤波器　　　　　默认（lanczos3）                ║│                                          │"
"│  位深　　　　　　自动（判据说了算）              ║│                                          │"
"│  抖动　　　　　　自动（判据说了算）              ║│                                          │"
"│  逐页　　　　　　默认（关）                      ▼│                                          │"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" 报告区 · ↑↓ 选一枝 · ⏎ 展开这一枝 · e 展开逐页 · i 这一趟的前提 · ⇥ 回配置 · 跑着…… · s 停（按 "
" 一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷） · ? 全部键               "
"                                                                                                "
"#;

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const FOCUS_ON_THE_VOLUMES: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告 · 展开 库（第 1/1 个目录）───────────┐"
"│口味层 · 这一趟的立场                             █│ 记号  卷名  页数  基准档  定档页   耗时  │"
"│  适配方式　　　　默认（height）                  █│ -     卷一   180  跳过             3s    │"
"│  裁边　　　　　　默认（裁）                      ║│ ✓     卷二     1  4bit    001.jpg  1m12s │"
"│  跨页拆分　　　　默认（拆）                      ║│                                          │"
"│  拆分阈值　　　　默认（1.5）                     ║│                                          │"
"│  阅读方向　　　　默认（rtl）                     ║│                                          │"
"│  滤波器　　　　　默认（lanczos3）                ║│                                          │"
"│  位深　　　　　　自动（判据说了算）              ║│                                          │"
"│  抖动　　　　　　自动（判据说了算）              ║│                                          │"
"│  逐页　　　　　　默认（关）                      ▼│                                          │"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" 卷表 · ↑↓ 选一卷 · ⏎ 展开逐页 · i 这一趟的前提 · Esc 回目录表 · ⇥ 回配置 · 跑着…… · s 停（按一 "
" 次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷） · ? 全部键                 "
"                                                                                                "
"#;

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const THE_FOLLOW_STOPPED: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告 · 展开 库（第 1/1 个目录） · 跟随停了┐"
"│口味层 · 这一趟的立场                             █│ 记号  卷名  页数  基准档  定档页   耗时  │"
"│  适配方式　　　　默认（height）                  █│ -     卷一   180  跳过             3s    │"
"│  裁边　　　　　　默认（裁）                      ║│ ✓     卷二     1  4bit    001.jpg  1m12s │"
"│  跨页拆分　　　　默认（拆）                      ║│                                          │"
"│  拆分阈值　　　　默认（1.5）                     ║│                                          │"
"│  阅读方向　　　　默认（rtl）                     ║│                                          │"
"│  滤波器　　　　　默认（lanczos3）                ║│                                          │"
"│  位深　　　　　　自动（判据说了算）              ║│                                          │"
"│  抖动　　　　　　自动（判据说了算）              ║│                                          │"
"│  逐页　　　　　　默认（关）                      ▼│                                          │"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" 卷表 · ↑↓ 选一卷 · ⏎ 展开逐页 · g 回到跟随 · i 这一趟的前提 · Esc 回目录表 · ⇥ 回配置 · 跑着…… "
" · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷） · ? 全部键     "
"                                                                                                "
"#;

    /// 报告长过一格时，留下的是**最后**那几行——**最新收摊的那一卷**不该掉到格子外面。
    ///
    /// 这一条与那两张快照是一对：快照里格子够高、一行都不少，这里问格子不够高时留谁。
    /// 「跟着最新收摊的那一卷走」与今天「滚到底」是同一个效果，而算它的现在是
    /// [`Viewport`]——光标停在末行（票面第一条）。
    #[test]
    fn a_report_taller_than_the_pane_keeps_its_last_lines() {
        // 没有失败页的那一趟：表就是这一格里最后那几行，问的正是「最新的那一卷还在不在」。
        let live = a_run_in_flight(false);
        let last = table(&live, 94, None, &only_branch(&live))
            .rows
            .pop()
            .expect("表上有卷");

        // 只给四行的格子：最新那一卷仍在，抬头已经让位。
        // **展开着那一枝**：卷表是那一级的事（`volume-discovery/08`）。
        let squeezed = opened_snapshot(&live, 96, 4 + OVERVIEW_HEIGHT);

        assert!(
            squeezed.contains(last.text.trim_end()),
            "最新收摊的那一卷掉出去了：{squeezed}"
        );
        assert!(
            !squeezed.contains("适配方式 以高为准"),
            "四行的格子装不下抬头，它却还在：{squeezed}"
        );
        // 一格都不剩的格子问不出滚动量，也不恐慌（[`Viewport`] 那一头的规矩）。
        let rows = opened_rows(&live, 94).rows.len();
        assert_eq!(Viewport::new(rows, 0, rows.saturating_sub(1)).from(), 0);
        // 窄到一格正文都不剩：折不出比一个字更窄的行（[`crate::wrap::fold`]），
        // 表也砍无可砍——两头都不恐慌就够了。
        opened_snapshot(&live, 2, 4 + OVERVIEW_HEIGHT);
        main_snapshot(&live, 2, 4 + OVERVIEW_HEIGHT);
        assert!(
            !table(&live, 0, None, &only_branch(&live)).rows.is_empty(),
            "砍无可砍时表也还在"
        );
    }

    /// **六种卷各有各的行**（票面第二条）：跳过、隔离、卷级失败、逐页、覆盖、等答话。
    ///
    /// 逐条问的是那一行**长什么样**，不是屏上第几行：档位那一列写什么出自
    /// [`crate::render::base_column`]，行首记号与行尾那句话是这一层的事
    /// （见 [`super::table`]）。**不重排**——六行的先后就是跑完的先后。
    #[test]
    fn six_kinds_of_volume_each_get_their_own_row() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let rows = table(&live, 120, None, &only_branch(&live)).rows;
        let body: Vec<&str> = rows.iter().skip(1).map(|row| row.text.as_str()).collect();

        assert_eq!(body.len(), 6, "六种卷六行：{body:?}");
        // 一、跳过：档位那一列写「跳过」，定档页留空，**耗时照给**。
        assert!(
            body[0].starts_with(" -"),
            "跳过那一行的记号不对：{}",
            body[0]
        );
        assert!(
            body[0].contains("棋魂 07") && body[0].contains("184"),
            "{}",
            body[0]
        );
        assert!(
            body[0].contains("跳过") && body[0].contains("3s"),
            "{}",
            body[0]
        );
        assert!(
            !body[0].contains(".jpg"),
            "跳过的卷不该有定档页：{}",
            body[0]
        );
        // 二、隔离：记号跳出来，行尾还有那个字——不上色的终端上也读得出。
        assert!(body[1].starts_with(" !"), "{}", body[1]);
        assert!(
            body[1].contains("哆啦 03") && body[1].ends_with("隔离"),
            "{}",
            body[1]
        );
        assert!(
            body[1].contains("4bit") && body[1].contains("001.jpg"),
            "{}",
            body[1]
        );
        // 三、逐页：档位那一列照卷级判定说的写，不编第二套说法。
        assert!(
            body[2].contains("名侦探 05") && body[2].contains("逐页"),
            "{}",
            body[2]
        );
        // 四、覆盖：同上，带着那个被覆盖成的候选。
        assert!(
            body[3].contains("浪客行 12") && body[3].contains("覆盖 2bit+FS"),
            "{}",
            body[3]
        );
        // 五、卷级失败：记号 `✗`、档位写「没做成」、理由成句跟在后面，
        //     页数那一格不在场（它连一份卷报告都没有）。整行是红的，
        //     那一条在 `src/session/draw/paint.rs` 的
        //     `the_row_that_went_wrong_is_red_and_says_so` 上。
        assert!(body[4].starts_with(" ✗"), "{}", body[4]);
        assert!(
            body[4].contains("消失的那卷") && body[4].contains("没做成"),
            "{}",
            body[4]
        );
        assert!(body[4].ends_with("卷根不在了"), "{}", body[4]);
        assert!(body[4].contains('-'), "页数那一格该留个记号：{}", body[4]);
        // 六、等答话：那一卷**到此刻为止**的那一份照画，末尾标着那一句。
        assert!(
            body[5].contains("棋魂 08") && body[5].ends_with("等你拿主意"),
            "{}",
            body[5]
        );
    }

    /// **卷按跑完的先后添上去，出事的靠行首记号跳出来、不靠位置**（票面第三条：不重排）。
    ///
    /// 比的是**整个卷名**（连卷号一起）：头一卷与末一卷同名不同号（`棋魂 07` / `棋魂 08`），
    /// 而它们恰是这条要钉的那一对——一个来自收摊了的那一列，一个是决策点上那一份。
    /// 只比到书名的话，这两卷对调了这条用例照样绿。
    #[test]
    fn the_table_never_reorders_the_volumes_it_has_added() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let body: Vec<String> = table(&live, 120, None, &only_branch(&live))
            .rows
            .into_iter()
            .skip(1)
            .map(|row| row.text)
            .collect();
        let order = [
            "棋魂 07",
            "哆啦 03",
            "名侦探 05",
            "浪客行 12",
            "消失的那卷",
            "棋魂 08",
        ];

        assert_eq!(body.len(), order.len(), "表上不是六行：{body:?}");
        for (at, name) in order.iter().enumerate() {
            assert!(
                body[at].contains(name),
                "第 {at} 行该是 {name}，实际是 {}",
                body[at]
            );
        }
    }

    /// **表上没有列、别处也没有位置的那两句摆在那一卷底下**（过期副本、部分救回）。
    ///
    /// 两种各有各的缺口：过期副本在末尾那几小结与总览块的出事行上**一个字都没有**，
    /// 少了这几行屏上就再也说不出「盘上还躺着上一趟那一份」；部分救回末尾说得出，
    /// 但那一段收场之后才画，而跑着的那几十分钟里源文件不全这件事没人提。
    ///
    /// 它们是**句子**，因此不塞进格：缩进摆在那一行底下，整段折行。
    #[test]
    fn the_sentences_the_table_has_no_column_for_sit_under_that_volume() {
        let mut superseded = fixture::processed_volume("棋魂 07", None);
        superseded.superseded = Some(std::path::PathBuf::from("出/隔离/棋魂 07"));
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/棋魂 07"), 1000);
        live.volume_finished(&superseded);

        let rows = table(&live, 120, None, &only_branch(&live)).rows;

        // 卷那一行照旧是一行，过期副本那一句在它**底下**、缩进摆着。
        assert_eq!(rows.len(), 3, "该是列头 + 一卷 + 那一句：{rows:?}");
        assert!(rows[1].text.contains("棋魂 07"), "{}", rows[1].text);
        assert!(
            rows[2].text.starts_with("   过期副本 出/隔离/棋魂 07："),
            "{}",
            rows[2].text
        );
        assert!(
            rows[2].text.ends_with("删不删由你"),
            "整句没摆全：{}",
            rows[2].text
        );
        // 那一句逐字来自 `render`——这一层一个字都没重写。
        let said = crate::render::volume(&superseded)
            .into_iter()
            .find(|row| row.kind == crate::render::RowKind::Superseded)
            .and_then(|row| row.cell(crate::render::Field::Sentence).map(str::to_owned))
            .expect("过期副本那一行");
        assert!(rows[2].text.contains(&said), "{}", rows[2].text);
        // 一卷都没被顶掉时它一行都不出——与末尾那几小结同一条规矩。
        let plain = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        assert!(
            table(&plain, 120, None, &only_branch(&plain))
                .rows
                .iter()
                .all(|row| !row.text.contains("过期副本")),
            "没有过期副本的那几卷也说了这句话"
        );
    }

    /// **卷名走与进度条同一个出处，摆不下时从中间省略**（票面第三条）。
    ///
    /// 那个出处是 [`crate::render::volume_name`]：命令行的进度条、会话的当前卷条、
    /// 报告区展开时的抬头印的都是它。
    #[test]
    fn the_volume_names_come_from_the_same_place_as_the_progress_bar() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let wide = table(&live, 120, None, &only_branch(&live)).rows;

        for volume in &live.report().volumes {
            let name = crate::render::volume_name(&volume.volume);
            assert!(
                wide.iter().any(|row| row.text.contains(&name)),
                "{name} 没上表：{wide:?}"
            );
        }
        // 窄到一列都砍无可砍：卷名从中间省略，书名与第几卷两头都还认得出。
        let narrow = table(&live, 26, None, &only_branch(&live)).rows;
        // 第五行是「消失的那卷」——十格的名字收进六格里。
        let elided = &narrow.iter().skip(1).nth(4).expect("那一行在表上").text;
        assert!(elided.contains('⋯'), "该省略却没省略：{elided}");
        assert!(elided.contains('消'), "书名那一头没留下：{elided}");
        assert!(elided.contains('卷'), "末一头没留下：{elided}");
    }

    /// **最窄那一档上卷名与行首记号仍在**（票面第五条）。
    ///
    /// 砍列那个次序的用例在 [`crate::session::columns`]（纯函数，终端库外面）；
    /// 这一条问的是**屏上**：80×24 那一档摆出来之后，几种记号一个不少、卷名认得出来。
    #[test]
    fn the_narrowest_screen_still_shows_every_mark_and_every_name() {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        let mut session = Session::new();
        // 卷表是**展开一枝**之后那一副（`volume-discovery/08`）：夹具里那几卷
        // 都躺在同一个目录底下，因此恒是头一枝。
        session.open(only_branch(&live).directory);

        let narrow = tight(&screen(&mut session, Some(&live), 80, 24));

        for mark in ['-', '!', '✗'] {
            assert!(narrow.contains(mark), "{mark} 掉出去了：{narrow}");
        }
        // 耗时与定档页这一档上砍掉了——次序只有 `columns` 那一处出处。
        assert!(!narrow.contains("耗时"), "最窄那一档还留着耗时：{narrow}");
        assert!(
            !narrow.contains("定档页"),
            "最窄那一档还留着定档页：{narrow}"
        );
        assert!(narrow.contains("记号"), "{narrow}");
        assert!(narrow.contains("卷名"), "{narrow}");
    }

    /// **成句的那几段仍旧当整段文字折行画，一句都没塞进格**（票面第六条）。
    ///
    /// 两种各问一遍：报告末尾那几小结、以及没做成那一卷的那句原因。
    /// 折行走 [`crate::wrap`]，因此它们在窄格子里会折下来，但**一个字都不少**。
    ///
    /// **互锁那几行不在这一格里了**（`p3-session-legibility/12`）：它们跟着抬头那几行
    /// 一起进了[覆盖层](crate::session::state::Overlay::Premises)，而那一张同样把它们
    /// 当整段文字折行（见 `super::overlay`）——这一条的末一段因此改问「它们真的走了」。
    #[test]
    fn the_sentences_are_still_drawn_as_folded_prose() {
        let mut live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        let report = live.report().clone();
        live.returned(Ok(report));

        let drawn = |width: u16| -> String {
            opened_rows(&live, width)
                .rows
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };

        // 一、末尾那几小结：整段照原样折行摆下去，**折出来的每一行都在**，
        //    一个字都没被塞进格（折法与命令行印它时同一套，见 [`crate::wrap`]）。
        let tail = crate::render::tail(live.report());
        let wide = drawn(120);
        for line in wrap::fold(&tail, 120) {
            assert!(wide.contains(&line), "末尾那一小结被拆了：{line}");
        }
        // 二、没做成那一卷的那句原因：它跟在表上那一行的行尾，整句在。
        assert!(wide.contains("卷根不在了"), "{wide}");
        // 三、窄下来时它们折行，而不是被从行尾切掉：折出来的行数多了，字一个没少。
        let narrow = drawn(40);
        assert!(
            narrow.lines().count() > wide.lines().count(),
            "窄了却没折行"
        );
        assert!(
            narrow.contains("卷根不在了"),
            "折行把那句原因切没了：{narrow}"
        );
        // 四、**这一趟的前提一行都不在这一格里**：profile 那一行与互锁那几行
        //    此刻在覆盖层那一张上（`p3-session-legibility/12` 票面第三条），
        //    而卷表从这一格的头一行起。默认那一套互锁一条都不咬，换一个咬得上的适配方式。
        let mut inside = Live::new(
            &tonefit::Request {
                fit: tonefit::FitMode::Inside,
                ..fixture::request(RunMode::Process)
            },
            Resuming::GoesOn,
        );
        inside.run_started(1, 1000);
        inside.volume_finished(&fixture::processed_volume("卷一", None));
        let said = opened_rows(&inside, 120)
            .rows
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !said.contains("互锁 拆分开着，适配方式却是 fit-inside"),
            "互锁那几行还在卷表上方：{said}"
        );
        assert!(!said.contains("profile "), "抬头那一行还在卷表上方：{said}");
        assert!(said.starts_with(" 记号"), "卷表不是这一格的头一行：{said}");
    }

    /// **快照：报告区就是一张表，宽终端。**（Q130 把它从布局那一块搬到这里）
    ///
    /// 钉住的是 `p1-session/09` 那九条验收里画得出来的那几条：卷数与剩余时间说得出来、
    /// 当前卷那一行说得出在走哪一遍、一卷跑完当场显示它的判定、幂等命中说清跳过了、
    /// 这一趟怎么读的（那一句进了展开那一副，卷表上是耗时那一列）。
    /// 总览块自己那几张快照在 [`super::overview`]。
    ///
    /// **它钉的是报告区的正文，因此住在这一块**：从前它跟着 `main_pane` 留在布局那一块，
    /// 改一句报告措辞就要回布局模块重录（停车场 Q130）。
    #[test]
    fn the_report_pane_without_a_failed_page() {
        let snapshot = opened_snapshot(&a_run_in_flight(false), 96, 30);

        same_screen(&snapshot, WITHOUT_A_FAILED_PAGE);
    }

    /// 见 [`the_report_pane_without_a_failed_page`]。
    const WITHOUT_A_FAILED_PAGE: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                              │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                             │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 库（第 1/1 个目录）───────────────────────────────────────────────────────────────┐"
"│ 记号  卷名  页数  基准档  定档页   耗时                                                      │"
"│ -     卷一   180  跳过             3s                                                        │"
"│ ✓     卷二     1  4bit    001.jpg  1m12s                                                     │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：同一趟，其中一卷有失败页。**
    ///
    /// 「失败页出现的当场就在主区可见，带原因」——那一段与表上那一行的隔离记号
    /// 并排出现，两者说的是同一份原因（一份是增量，一份是结果）。
    #[test]
    fn the_report_pane_with_a_failed_page() {
        let snapshot = opened_snapshot(&a_run_in_flight(true), 96, 36);

        same_screen(&snapshot, WITH_A_FAILED_PAGE);
    }

    /// 见 [`the_report_pane_with_a_failed_page`]。
    const WITH_A_FAILED_PAGE: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                              │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                             │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"│ 出事 隔离 1 卷 · 失败 1 页                                                                   │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 库（第 1/1 个目录）───────────────────────────────────────────────────────────────┐"
"│ 记号  卷名  页数  基准档  定档页   耗时                                                      │"
"│ -     卷一   180  跳过             3s                                                        │"
"│ !     卷二     2  4bit    001.jpg  1m12s  隔离                                               │"
"│失败页（出现的当场，逐页那几行在整卷跑完后才有）                                              │"
"│  库/卷二/017.jpg                                                                             │"
"│    失败 解不出完整尺寸：JPEG 数据截断                                                        │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：执行那一副，宽终端（120 列）。**（票面第七条：宽那一张、执行那一张）
    ///
    /// 一卷一行、列对齐；抬头那几行摆在表上方、跟着表滚（12 号票才把它们收进覆盖层）。
    /// 跳过、隔离、逐页、覆盖、卷级失败五种各占一行——**不重排**，就是跑完的先后。
    /// 第六种（等答话）只有续做那一趟到得了，它在
    /// [`the_volume_table_of_a_dry_run`] 与
    /// [`the_volume_waiting_at_the_decision_point_gets_a_row_of_its_own`] 两张上。
    #[test]
    fn the_volume_table_on_a_wide_terminal() {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);

        same_screen(&opened_snapshot(&live, 120, 26), THE_TABLE_WIDE);
    }

    /// 见 [`the_volume_table_on_a_wide_terminal`]。
    const THE_TABLE_WIDE: &str = r#"
"┌执行 · 第 5/6 卷 · 还剩约 1m00s───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s                                                      │"
"│                                                                                                                      │"
"│ 完成 3 卷 · 跳过 1 卷                                                                                                │"
"│ 出事 隔离 1 卷 · 失败 1 页 · 卷级失败 1 卷                                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 库（第 1/1 个目录）───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 记号  卷名        页数  基准档        定档页   耗时                                                                  │"
"│ -     棋魂 07      184  跳过                   3s                                                                    │"
"│ !     哆啦 03        2  4bit          001.jpg  1m12s  隔离                                                           │"
"│ ✓     名侦探 05      1  逐页                   1m12s                                                                 │"
"│ ✓     浪客行 12      1  覆盖 2bit+FS           1m12s                                                                 │"
"│ ✗     消失的那卷     -  没做成                        卷根不在了                                                     │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：同一趟，最窄那一档（80×24，整屏）。**（票面第五、七条：窄那一张）
    ///
    /// 左栏先让掉一截，报告区剩下二十几列——耗时与定档页按
    /// [`crate::session::columns`] 那个次序砍掉，**卷名与行首记号仍在**。
    ///
    /// 总览块的抬头在这一档上被终端库硬截（停车场 Q147），那不是这一票弄坏的。
    #[test]
    fn the_volume_table_on_the_narrowest_terminal() {
        let mut session = Session::new();
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        session.open(only_branch(&live).directory);

        same_screen(&snapshot_of(&mut session, &live, 80, 24), THE_TABLE_NARROW);
    }

    /// 见 [`the_volume_table_on_the_narrowest_terminal`]。
    const THE_TABLE_NARROW: &str = r#"
"┌配置────────────────────────────────────────────┐┌执行 · 第 5/6 ⋯ 还剩约 1m00s┐"
"│设备层 · 判定的依据，绑面板，改一次管很久       ▲│ 总体 [=====================│"
"│  型号　　　　　　未挑（跑起来之前必填）        █│                            │"
"│  感知可分辨级数　默认（跟随面板）              █│ 完成 3 卷 · 跳过 1 卷      │"
"│  阈值　　　　　　跟着型号走（先挑一个）        █│ 出事 隔离 1 卷 · 失败 1 页 │"
"│                                                █└────────────────────────────┘"
"│口味层 · 这一趟的立场                           █┌报告 · 展开 库⋯ 1/1 个目录）┐"
"│  适配方式　　　　默认（height）                █│ 记号  卷名     基准档      │"
"│  裁边　　　　　　默认（裁）                    █│ -     棋魂 07  跳过        │"
"│  跨页拆分　　　　默认（拆）                    █│ !     哆啦 03  4bit        │"
"│  拆分阈值　　　　默认（1.5）                   ║│ 隔离                       │"
"│  阅读方向　　　　默认（rtl）                   ║│ ✓     名⋯ 05   逐页        │"
"│  滤波器　　　　　默认（lanczos3）              ║│ ✓     浪⋯ 12   覆盖 2bit+FS│"
"│  位深　　　　　　自动（判据说了算）            ║│ ✗     消⋯那卷  没做成      │"
"│  抖动　　　　　　自动（判据说了算）            ║│ 卷根不在了                 │"
"│  逐页　　　　　　默认（关）                    ║│                            │"
"│  缓存预算　　　　默认（512.0 MiB）             ║│                            │"
"│  读取策略　　　　默认（auto）                  ║│                            │"
"│                                                ║│                            │"
"│范围层 · 每趟都不同，不进预设                   ▼│                            │"
"└────────────────────────────────────────────────┘└────────────────────────────┘"
" 卷表 · ↑↓ 选一卷 · ⏎ 展开逐页 · i 这一趟的前提 · Esc 回目录表 · ⇥ 回配置 · q 退"
" 出 · ? 全部键                                                                  "
" 跟随着最新的那一卷：一卷收摊，光标就落到它上面                                 "
"#;

    /// **快照：试算那一副。**（票面第七条：试算那一张）
    ///
    /// 与执行那一副差的是抬头那一句 `dry-run` 与总览块那两行——表本身一列不变，
    /// 那正是「同一批行，两副排版」买到的东西。续做那一趟停在决策点上，
    /// **等答话那一卷因此也在这张表上**（末一行）。
    #[test]
    fn the_volume_table_of_a_dry_run() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);

        same_screen(&opened_snapshot(&live, 120, 26), THE_TABLE_OF_A_DRY_RUN);
    }

    /// 见 [`the_volume_table_of_a_dry_run`]。
    const THE_TABLE_OF_A_DRY_RUN: &str = r#"
"┌试算 · 第 6/6 卷 · 还剩约 1m00s───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s                                                      │"
"│ 本卷 棋魂 08 · 第二遍 [>                             ] 0/1000 步                                                     │"
"│ 判定 1 卷 4bit · 1 卷 覆盖 2bit+FS · 1 卷 跳过 · 1 卷 逐页                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 库（第 1/1 个目录）───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 记号  卷名        页数  基准档        定档页   耗时                                                                  │"
"│ -     棋魂 07      184  跳过                   3s                                                                    │"
"│ !     哆啦 03        2  4bit          001.jpg  1m12s  隔离                                                           │"
"│ ✓     名侦探 05      1  逐页                   1m12s                                                                 │"
"│ ✓     浪客行 12      1  覆盖 2bit+FS           1m12s                                                                 │"
"│ ✗     消失的那卷     -  没做成                        卷根不在了                                                     │"
"│ ✓     棋魂 08        1  4bit          001.jpg  1m12s  等你拿主意                                                     │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：停在决策点上等答话的那一卷在表上。**（票面第七条：等答话那一张）
    ///
    /// 它今天停在「攒着的那一份」上、不在收摊了的那几卷里（停车场 Q52），
    /// 而表上照样给它一行：记号与档位照它**到此刻为止**的那份报告画，末尾标着那一句。
    #[test]
    fn the_volume_waiting_at_the_decision_point_gets_a_row_of_its_own() {
        let summarized = fixture::processed_volume("棋魂 08", None);
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/棋魂 07"), 1000);
        live.volume_finished(&fixture::skipped_volume("棋魂 07", 184));
        live.volume_started(Path::new("库/棋魂 08"), 1000);
        live.pass_started(tonefit::Pass::Second, Some(&summarized));
        live.rewind(Duration::from_secs(300));

        same_screen(
            &opened_snapshot(&live, 96, 18),
            WAITING_AT_THE_DECISION_POINT,
        );
    }

    /// 见 [`the_volume_waiting_at_the_decision_point_gets_a_row_of_its_own`]。
    const WAITING_AT_THE_DECISION_POINT: &str = r#"
"┌试算 · 第 2/2 卷 · 还剩约 5m00s───────────────────────────────────────────────────────────────┐"
"│ 总体 [===============>              ] 1000/2000 步 · 已用 5m00s                              │"
"│ 本卷 棋魂 08 · 第二遍 [>                             ] 0/1000 步                             │"
"│ 判定 1 卷 跳过                                                                               │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 库（第 1/1 个目录）───────────────────────────────────────────────────────────────┐"
"│ 记号  卷名     页数  基准档  定档页   耗时                                                   │"
"│ -     棋魂 07   184  跳过             3s                                                     │"
"│ ✓     棋魂 08     1  4bit    001.jpg  1m12s  等你拿主意                                      │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：展开一卷，默认只列要紧的页**（票面第一、二、八条）。
    ///
    /// 钉的是整屏：左栏一格都不在，右边那一大格从第 0 列画满
    /// （**展开与左栏收起是同一件事**，`CONTEXT.md` 的《会话》）。
    /// 抬头钉在这一格顶上，表在它底下滚——六页要紧的各带着自己那个词，
    /// 而普通那两页（含彩色分支那一张）一行都没有。
    ///
    /// 屏取 136 列：逐页那几行**轻松过 100 列**（票面原话），这一张钉的是这张表
    /// 本来的样子——一列不砍、一行不折。窄下来砍成什么样另有一条
    /// （[`a_narrow_pane_cuts_columns_instead_of_scrolling_the_page_rows_sideways`]）。
    #[test]
    fn expanding_a_volume_collapses_the_left_column_and_lists_the_pages_that_matter() {
        let (mut session, live) = expanded(1);

        same_screen(
            &snapshot_of(&mut session, &live, 136, 20),
            EXPANDED_TO_THE_PAGES_THAT_MATTER,
        );
    }

    /// 见 [`expanding_a_volume_collapses_the_left_column_and_lists_the_pages_that_matter`]。
    const EXPANDED_TO_THE_PAGES_THAT_MATTER: &str = r#"
"┌收场 点名的卷都走过了 · 2 卷 · 用了 0s────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [==============================] 2000/2000 步                                                                                   │"
"│ 判定 1 卷 4bit · 1 卷 跳过                                                                                                           │"
"│ 出事 特例页 1 张 · 宽溢出 1 页 · 几何门不成立 1 卷                                                                                   │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 卷二（第 2/2 卷）─────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 基准档 4bit · 定档页 003.jpg · 要紧的页 6/8                                                                                          │"
"│ 记号  页名     尺寸       判定  理由                      判据                                                                       │"
"│ *     003.jpg  1182×1680  4bit  阈值内最低的一档          1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  定档页             │"
"│ !     004.jpg  1600×1680  8bit  特例页单独定档            1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  特例页  宽溢出     │"
"│ !     005.jpg  1182×1680  4bit  几何门不成立，本页不抖动  1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  几何门不成立       │"
"│ !     006.jpg  1182×1680  4bit  卷级上包络                1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  兜底上界           │"
"│ !     007.jpg  1182×1680  4bit  卷级上包络                1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  救回 62.0%         │"
"│ ✗     017.jpg  1182×1680                                                                                          失败 解不出完整尺寸│"
"│ ：JPEG 数据截断                                                                                                                      │"
"│                                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 选一页 · a 列全部页 · ⇥ 换下一卷 · e／Esc 收起，左栏回来 · q 退出 · ? 全部键                                                        "
" 只列要紧的页：特例 · 失败 · 部分救回 · 几何门不成立 · 宽溢出 · 兜底上界，加上定档页                                                    "
"                                                                                                                                        "
"#;

    /// **快照：`a` 切到全部页**（票面第二条）。
    ///
    /// 与上一张是一对：同一卷、同一屏，差的只有列出来的那几行与抬头末一格。
    /// 普通那两页在这一副上回来了，彩色分支那一句也在——那一句只有逐页那几行说得出。
    #[test]
    fn pressing_a_lists_every_page_of_the_volume() {
        let (mut session, live) = expanded(1);

        session.press(Key::Char('a'));

        assert_eq!(
            session.expansion().expect("展开着").listing,
            Listing::All,
            "`a` 没切到全部页"
        );
        same_screen(
            &snapshot_of(&mut session, &live, 136, 20),
            EXPANDED_TO_EVERY_PAGE,
        );
    }

    /// 见 [`pressing_a_lists_every_page_of_the_volume`]。
    const EXPANDED_TO_EVERY_PAGE: &str = r#"
"┌收场 点名的卷都走过了 · 2 卷 · 用了 0s────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [==============================] 2000/2000 步                                                                                   │"
"│ 判定 1 卷 4bit · 1 卷 跳过                                                                                                           │"
"│ 出事 特例页 1 张 · 宽溢出 1 页 · 几何门不成立 1 卷                                                                                   │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 卷二（第 2/2 卷）─────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 基准档 4bit · 定档页 003.jpg · 全部 8 页（要紧的 6 页）                                                                              │"
"│ 记号  页名     尺寸       判定  理由                      判据                                                                       ▲"
"│ ✓     001.jpg  1182×1680  4bit  卷级上包络                1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000                     █"
"│ ✓     002.jpg  1182×1680                                                                                          彩页 · 彩色分支：只█"
"│ 缩放，不量化，不进灰度缓存也不进卷级上包络                                                                                           █"
"│ *     003.jpg  1182×1680  4bit  阈值内最低的一档          1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  定档页             ║"
"│ !     004.jpg  1600×1680  8bit  特例页单独定档            1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  特例页  宽溢出     ║"
"│ !     005.jpg  1182×1680  4bit  几何门不成立，本页不抖动  1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  几何门不成立       ║"
"│ !     006.jpg  1182×1680  4bit  卷级上包络                1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  兜底上界           ║"
"│ !     007.jpg  1182×1680  4bit  卷级上包络                1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000  救回 62.0%         ▼"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 选一页 · a 只列要紧的页 · ⇥ 换下一卷 · e／Esc 收起，左栏回来 · q 退出 · ? 全部键                                                    "
" 列着全部页：要紧的那几页照旧靠行首记号跳出来                                                                                           "
"                                                                                                                                        "
"#;

    /// **快照：跑着的时候展开**（票面第四条，`p3-session-legibility/10` 推翻停车场 Q72）。
    ///
    /// 报告边跑边长，展开的那一副跟着更新：这一张钉的是**跑着那一刻**的屏——
    /// 屏底摆着按停那一副（按停在这一块上照旧按得动，票面第六条），
    /// 而三层一个改动键都不派（左栏此刻连屏都不在）。
    #[test]
    fn a_volume_expands_while_the_run_is_still_going() {
        let live = a_run_in_flight(true);
        let mut session = Session::new();
        session.run_started();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(1)));

        same_screen(
            &snapshot_of(&mut session, &live, 120, 22),
            EXPANDED_WHILE_RUNNING,
        );
    }

    /// 见 [`a_volume_expands_while_the_run_is_still_going`]。
    const EXPANDED_WHILE_RUNNING: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                                                      │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                                                     │"
"│ 完成 1 卷 · 跳过 1 卷                                                                                                │"
"│ 出事 隔离 1 卷 · 失败 1 页                                                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 卷二（第 2/2 卷）─────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 基准档 4bit · 定档页 001.jpg · 要紧的页 2/2                                                                          │"
"│ 记号  页名     尺寸       判定  理由              判据                                                               │"
"│ *     001.jpg  1182×1680  4bit  阈值内最低的一档  4bit 8.000  定档页                                                 │"
"│ ✗     017.jpg  1182×1680                                      失败 解不出完整尺寸：JPEG 数据截断                     │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 选一页 · a 列全部页 · ⇥ 换下一卷 · e／Esc 收起，左栏回来 · 跑着…… · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会 "
" 话（当前卷中止，盘上不留半卷） · ? 全部键                                                                              "
"                                                                                                                        "
"#;

    /// **快照：等答话的时候展开的正是那一卷**（票面第五条，`p2-loose-ends/08`）。
    ///
    /// 那一卷停在**攒着的那一份**上、不在收摊了的那几卷里，而屏上给的是**它自己的**逐页
    /// ——「不许摊开上一卷冒充它」那一条的现场（另一条用例逐格问它：
    /// [`the_volume_waiting_at_the_decision_point_expands_into_its_own_pages`]）。
    /// 屏底摆着答话那三个键，`a` 那一刻归答话、不摆「列全部页」（停车场 Q161）。
    #[test]
    fn the_volume_waiting_at_the_decision_point_expands_while_it_waits() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let mut session = Session::new();
        session.run_started();
        session.at_the_decision_point(true);
        session.expand(Expansion::new(
            PathBuf::from("库"),
            Volume::Summarized { after: 4 },
        ));

        same_screen(
            &snapshot_of(&mut session, &live, 120, 22),
            EXPANDED_WHILE_DECIDING,
        );
    }

    /// 见 [`the_volume_waiting_at_the_decision_point_expands_while_it_waits`]。
    const EXPANDED_WHILE_DECIDING: &str = r#"
"┌试算 · 第 6/6 卷 · 等你拿主意─────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s                                                      │"
"│ 本卷 棋魂 08 · 第二遍 [>                             ] 0/1000 步                                                     │"
"│ 判定 1 卷 4bit · 1 卷 覆盖 2bit+FS · 1 卷 跳过 · 1 卷 逐页                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 棋魂 08（第 5/5 卷）──────────────────────────────────────────────────────────────────────────────────────┐"
"│ 基准档 4bit · 定档页 001.jpg · 要紧的页 1/1                                                                          │"
"│ 记号  页名     尺寸       判定  理由              判据                                                               │"
"│ *     001.jpg  1182×1680  4bit  阈值内最低的一档  4bit 8.000  定档页                                                 │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"│                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 选一页 · ⇥ 换下一卷 · e／Esc 收起，左栏回来 · 等你拿主意…… · x 接着做第二遍（第一遍不重算）· a 剩下的卷都这样（往下 "
" 不再问）· s 收尾（这一卷不写，等价 dry-run；剩下的卷也不开工）· Ctrl-C 退出会话 · ? 全部键                             "
" 上面那份报告是真的：判定、逐页结果、缓存用量都算出来了，只有第二遍一步没走——这一卷此刻一个字节都没写                   "
"#;

    /// **逐页那几行只在展开的那一卷上出，而且逐字来自 [`crate::render`]。**
    ///
    /// 票面第一条（默认那一副只给卷级）与「渲染只有一处出处」两件事由同一条钉：
    /// 表上那几格（尺寸、判定、理由、判据）逐字能在 [`crate::render::pages`] 出的
    /// 那几行里找到，而没展开的那一份一行逐页都没有。
    #[test]
    fn the_per_page_cells_come_from_render_and_only_for_the_volume_that_is_open() {
        let live = a_run_worth_expanding();
        let opened = &live.report().volumes[1];
        let rows = crate::render::pages(opened);
        let panel = live.report().profile.panel();

        let table = pages::pages(opened, panel, 200, Listing::All, 0).table;
        let said = table
            .rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // 每一格都逐字来自 `render` 那几行——这一层一个字都没重写。
        for row in &rows {
            for cell in &row.cells {
                if matches!(
                    cell.field,
                    crate::render::Field::Size
                        | crate::render::Field::Candidate
                        | crate::render::Field::Reason
                        | crate::render::Field::Sentence
                ) {
                    assert!(said.contains(&cell.text), "少了这一格：{}", cell.text);
                }
            }
        }
        // 失败页那一句原因、彩页那一句「不量化也不进上包络」，两句都在——
        // 它们是成句的那一格，跟在行尾（`p1-session/11` 的后两条验收）。
        // 「失败页 · 卷内统一尺寸留白」那一格不在表上（缩放那一列不进表，
        // 见 `super::pages` 的《表有意不带的那几格》，停车场 Q162）。
        assert!(said.contains("失败 解不出完整尺寸"), "{said}");
        assert!(said.contains("彩色分支：只缩放，不量化"), "{said}");
        // 卷级那一副一行逐页都没有：展开才给（票面第一条）。
        let collapsed = opened_rows(&live, 200)
            .rows
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !collapsed.contains("彩色分支"),
            "没展开却给了逐页：{collapsed}"
        );
        // 幂等命中的卷展开了也一行逐页都没有（`render::pages` 那道守卫），不恐慌：
        // 那一格里摆的是一句话（见 `super::pages`）。
        let skipped = pages::pages(&live.report().volumes[0], panel, 200, Listing::All, 0);
        assert_eq!(skipped.table.rows.len(), 1);
        assert!(skipped.table.rows[0].text.contains("一页都没有重做"));
    }

    /// **窄下来砍列，逐页那几行不横着滚、也不整体错位**（票面第一条：与卷表同一套砍列）。
    ///
    /// 从前这一副是一段不折行的散文，窄了要往右滚才看得到行尾；它此刻是一张表，
    /// 窄了按 [`crate::session::columns`] 那个次序**砍列**——判据一串先让掉，
    /// 而「哪一页判成哪一档」一直在。`←→` 因此在这一块上不派动作
    /// （见 `super::super::state::expanded_action`）。
    #[test]
    fn a_narrow_pane_cuts_columns_instead_of_scrolling_the_page_rows_sideways() {
        let (mut session, live) = expanded(1);

        let narrow = snapshot_of(&mut session, &live, 60, 20);

        // 判定那一列在，判据那一串让掉了——次序只有 `columns` 那一处出处。
        assert!(narrow.contains("判定"), "判定那一列被砍了：\n{narrow}");
        assert!(!narrow.contains("判据"), "窄了还留着判据：\n{narrow}");
        // `←→` 一格都不动这一屏：它在这一块上根本不派动作。
        let before = session.expansion().cloned();
        session.press(Key::Right);
        session.press(Key::Left);
        assert_eq!(
            session.expansion().cloned(),
            before,
            "`←→` 动了展开着的那一格"
        );
        assert_eq!(snapshot_of(&mut session, &live, 60, 20), narrow);

        // 收起：左栏回来，报告区又只给卷级。
        session.press(Key::Esc);
        let back = snapshot_of(&mut session, &live, 60, 20);
        assert!(back.contains("配置"), "收起之后左栏没回来：\n{back}");
        assert!(!back.contains("页名"), "收起了还给逐页：\n{back}");
    }

    /// **抬头钉住，翻到底也还在**（票面：抬头钉住这一卷的；停车场 Q64 的另一半）。
    ///
    /// 逐页翻到第三屏时「这一卷判成哪一档、是哪一页定的」还得答得出来，
    /// 而那正是翻这几页要比的东西。默认那一副的抬头跟着表滚（那是这一趟的前提，
    /// 收进覆盖层是 `p3-session-legibility/12` 的事），这一副的抬头**不滚**。
    #[test]
    fn the_pinned_heading_of_the_expanded_volume_never_scrolls_away() {
        let (mut session, live) = expanded(1);
        session.press(Key::Char('a'));
        // 矮到八页摆不下：翻得动，才问得出「翻下去它还在不在」。
        let head = |session: &mut Session| -> String {
            snapshot_of(session, &live, 120, 12)
                .lines()
                .nth(6)
                .expect("这一格的头一行")
                .to_owned()
        };

        let top = head(&mut session);
        assert!(top.contains("基准档"), "抬头没钉在这一格顶上：{top}");
        assert!(top.contains("定档页"), "{top}");

        // 翻到底：抬头一格没动，而表底下那几行换过了。
        let before = snapshot_of(&mut session, &live, 120, 12);
        for _ in 0..8 {
            session.press(Key::Down);
        }
        let after = snapshot_of(&mut session, &live, 120, 12);
        assert_eq!(head(&mut session), top, "翻下去抬头跟着滚了");
        assert_ne!(after, before, "翻了八页却一行都没动");

        // 翻回头一页：与翻下去之前逐格相同（往上收在零，不必按第九下）。
        for _ in 0..8 {
            session.press(Key::Up);
        }
        assert_eq!(snapshot_of(&mut session, &live, 120, 12), before);
    }

    /// **展开一卷没有要紧的页的：说一句话，不给一张空表**（票面第三条）。
    ///
    /// 屏上那一句的措辞与「有几页」两处都在 [`super::pages`]，那一头逐条问过；
    /// 这一条问的是**屏上**：那一句真画出来了，而表上一行页都没有。
    #[test]
    fn a_volume_with_no_pages_worth_listing_says_so_on_screen() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/名侦探 05"), 1000);
        live.volume_finished(&fixture::per_page_volume("名侦探 05"));
        let report = live.report().clone();
        live.returned(Ok(report));
        let mut session = Session::new();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(0)));

        let open = tight(&screen(&mut session, Some(&live), 120, 22));

        assert!(open.contains(&tight("这一卷没有要紧的页")), "{open}");
        assert!(
            open.contains(&tight("要紧的页 0/1")),
            "抬头没说这一副列着几页：{open}"
        );
        assert!(!open.contains(&tight("记号  页名")), "给了一张空表：{open}");
        // 切到全部页就有表了：那一页在，而它一个要紧的词都不带。
        session.press(Key::Char('a'));
        let all = tight(&screen(&mut session, Some(&live), 120, 22));
        assert!(all.contains(&tight("记号  页名")), "{all}");
        assert!(all.contains(&tight("001.jpg")), "{all}");
    }
}
