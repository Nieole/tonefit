//! 屏上那一块：**报告区**——主区最下面那一段，边跑边攒的那一份
//! （`CONTEXT.md` 的《会话》：报告区）。
//!
//! 措辞出自 [`crate::render`]：命令行与会话共用那几个函数，一个字都不在这里重写。
//! 一卷跑完那条事件带着那一卷的报告（ADR 0011），[`crate::render::volume`] 收下它
//! 就画得出判定、定档页、隔离与这一趟怎么读的。
//!
//! **默认那一副是一张表**：一卷一行，列对齐，窄了砍列——表怎么摆在 [`super::table`]，
//! 哪几列、砍哪几列在 [`crate::session::columns`]（那一块在终端库外面）。
//! 表**上面**跟着滚的是这一趟的抬头（profile、适配方式、裁边、跨页拆分、判据构成与聚合），
//! 表**下面**是当场冒出来的失败页、以及收场之后末尾那几小结——
//! 那几段本来就是句子，照旧当整段文字折行画（[`crate::wrap`]）。
//!
//! **两副样子由展开与否分**（`CONTEXT.md` 的《会话》：展开），差在哪几处见
//! [`report_pane`] 那张表。折行走 [`crate::wrap`]，本模块只交代**折到多宽**——
//! `--help` 与命令行印出来的报告折的是同一套，而那两处根本没有终端库。
//!
//! 这一段在主区占几行由 [`super::main_pane`] 分；上面那一块总览在 [`super::overview`]。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::paint::{Painted, Tone};
use super::table::table;
use crate::session::live::{Live, Volume};
use crate::session::state::{Focus, Follow, Session};
use crate::session::viewport::Viewport;
use crate::wrap;

/// 报告区：**边跑边攒**的那一份，措辞出自 [`crate::render`]。
///
/// 卷级那几行一卷跑完就画得出来（一卷跑完那条事件带着那一卷的报告，ADR 0011）：
/// 判定、定档页、隔离、这一趟怎么读的、幂等命中说清哪四项依据没变，全在里面。
/// 失败页要在出现的当场看得见，那一段走 [`crate::render::failing_pages`]。
///
/// **两副样子，由展开与否分**（`CONTEXT.md` 的《会话》：展开）：
///
/// | | 卷表（默认） | 展开一卷 |
/// |---|---|---|
/// | 一卷 | **一行**，列对齐（[`super::table`]） | 那一卷的逐页那几行全给（[`crate::render::pages`]） |
/// | 逐页那几行 | 一行不给 | 展开的那一卷全给 |
/// | 长过一格 | **跟随时钉在末行，跟随停了就跟着[光标那一卷](crate::session::state::Follow)走**（[`Viewport`]） | 用户自己翻（[`Expansion::from`]） |
/// | 一行放不下 | 按固定次序**砍列**（[`crate::session::columns`]） | **不折**，横着滚 |
/// | 成句的那几段 | 折行（按显示宽度，见 [`crate::wrap`]） | 折行 |
/// | 左栏 | 在场 | 收起（见 [`super::shell`]） |
///
/// 默认那一副跟着最新收摊的那一卷，是因为报告只增不减，而「一卷跑完当场看得见」说的正是
/// 刚添上去的那一行；**光标一挪跟随就停了**，往回翻因此不必另有一个滚动量
/// （`p3-session-legibility/10`，那正是停车场 Q64 记着的缺口）。
/// 展开那一副不折行，是因为票面写着**逐页行不被折断**——折了就看不出哪几个数是一页的。
///
/// **默认那一副的滚动量与滚动条都由 [`Viewport`] 出**：跟随着的时候光标停在末行，
/// 算出来的起点因此恰好是「滚到底」——与从前那个自己算的一模一样，
/// 多出来的是右边框线上那条滚动条（画它的地方只有 [`super::scrolling`] 一处）。
pub(super) fn report_pane(
    frame: &mut Frame,
    area: Rect,
    session: &mut Session,
    live: Option<&Live>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(report_title(session, live));
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
        // 「就在这一行上动手」（见 [`super::config::config`]）。
        let shown = collapsed(
            live,
            inside.width,
            session.standing(live),
            matches!(session.focus(), Focus::Report),
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
    let opened = live.nearest(expansion.volume);
    let text = report_text(live, opened).text;
    // 翻页量收进这一格真滚得动的范围。不收的话，翻过了头再翻回来，
    // 头几下会按了没反应（见 [`Session::clamp_report`]）。
    session.clamp_report(
        rows(&text).saturating_sub(inside.height),
        widest(&text).saturating_sub(inside.width),
    );
    let expansion = session.expansion().expect("刚才还展开着");
    frame.render_widget(
        Paragraph::new(text)
            .scroll((expansion.from, expansion.right))
            .block(block),
        area,
    );
}

/// 一趟都还没跑过时这一格里说什么。
const NOT_RUN_YET: &str = "
 按 t 试算：只算不写，报告照出。
              按 x 执行：写到输出根。
              跑起来之前必填的两项是型号与输出根。";

/// 默认那一副的正文：**抬头几行 · 卷表 · 失败页 · 末尾那几小结**。
///
/// 抬头那几行（profile、适配方式、裁边、跨页拆分、判据构成与聚合）这一票**摆在表上方、
/// 跟着表滚**——它们是「这一趟的前提」，一趟只说一次；收进一个按键调得出的地方是
/// `p3-session-legibility/12` 的事。
///
/// 表以外的那几段**都是句子**，照旧当整段文字折行（[`crate::wrap`]）：
/// 末尾那几小结、抬头里互锁那几行、以及没做成那一卷的那句原因（它跟在表上那一行的行尾，
/// 摆不下时整行折下去）。**一句都没塞进格**——拆开没有意义，而拆的那一刀会把措辞
/// 挪到排版这一层来（ADR 0016）。
///
/// 与命令行印出来的那一份**同源不同副**：同样的行、同样的格，摆法两副。
fn collapsed(live: &Live, width: u16, at: Option<Volume>, focused: bool) -> Collapsed {
    let report = live.report();
    let mut rows = Painted::plain(crate::render::header(report, live.mode())).folded(width);
    let table = table(live, width, at);
    // 光标停在正文的第几行：表在抬头那几行**底下**，而抬头折出来几行要现数
    // （见 [`Viewport::new`]：行号推不出来）。
    let mut cursor = None;
    for (row, said) in table.rows.iter().enumerate() {
        let here = table.cursor == Some(row);
        if here {
            cursor = Some(rows.len());
        }
        // 反白只给光标那一行，而且只在焦点落在这一块上时给（见 [`super::paint::Painted`]）。
        let folded = said.folded(width);
        rows.extend(match here && focused {
            true => highlighted(folded),
            false => folded,
        });
    }
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

/// 默认那一副画出来的那几行，外加**光标停在第几行**。
///
/// 两样装在一个类型里而不是一对裸值，与 [`Unrolled`] 同一条理由：它们是同一次拼出来的，
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
        return match stopped_following(session) {
            true => format!("报告 · {FOLLOW_STOPPED}"),
            false => "报告".to_owned(),
        };
    };
    // 与正文同一道解析（见 [`report_pane`]）：抬头说的必须是这一格真画着的那一卷。
    let Some(opened) = live.nearest(expansion.volume) else {
        return "报告".to_owned();
    };
    let Some(volume) = live.volume(opened) else {
        return "报告".to_owned();
    };
    let volumes = live.volumes();
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

/// 这一份东西有几行。**不折行时的行数**，展开那一副用它。
fn rows(text: &str) -> u16 {
    u16::try_from(text.lines().count()).unwrap_or(u16::MAX)
}

/// 最长那一行有多宽（**显示宽度**：中文两列）。横着能滚多远由它定。
///
/// 量宽度而不是数字符：横向滚的是格子，而一个汉字占两格。宽度的出处只有
/// [`crate::wrap::width`]——折行按它折，滚动按它算，两处不许各数各的。
fn widest(text: &str) -> u16 {
    text.lines().map(wrap::width).max().unwrap_or(0)
}

/// 展开第 `volume` 卷之后，那一卷的抬头落在第几行。
///
/// **给 [`super::super::press`] 用**：`⇥` 换过一卷之后视口要对到那一卷的抬头上，
/// 而状态机算不出这个数——它读不到那一趟攒着的报告。算它只用得到
/// [`crate::render`] 那几个函数与 [`Live`]，一个终端都不碰。
pub(in crate::session) fn opens_at(live: &Live, volume: Volume) -> u16 {
    report_text(live, Some(volume)).opens_at
}

/// 报告区的正文，以及**展开的那一卷从第几行起**。
///
/// **与命令行印出来的那一份同源**：同样的段、同样的函数。展开出来的逐页那几行走的
/// 也是命令行那一份用的 [`crate::render::pages`]——失败页说得出它的尺寸是卷内统一的、
/// 彩色分支的页说得出它不量化也不进上包络，两句都在那里，会话不另写一份。
///
/// 与命令行那一份的差只有三处，各有理由：逐页那几行**只给展开的那一卷**
/// （默认一行不给，票面第一条）；失败页那一段是命令行没有的**增量**
/// （命令行攒完才印，那时逐页那几行已经把话说全了）；末尾那几小结要看完整趟才给得出来
/// （见 [`crate::render::tail`]），因此只在收场之后画。
///
/// 第二个出的数给 [`opens_at`]：换一卷之后视口对到那一卷的抬头上，靠的是它。
/// 与正文一起算出来而不是另写一遍，理由与「渲染只有一处出处」同一条——
/// 另数一遍就要把「哪一段在哪一段前面」抄第二份，而抄错了没人发现。
fn report_text(live: &Live, expand: Option<Volume>) -> Unrolled {
    let report = live.report();
    let mut text = crate::render::header(report, live.mode());
    let mut opens_at = 0;
    for (at, volume) in report.volumes.iter().enumerate() {
        if expand == Some(Volume::Settled(at)) {
            opens_at = rows(&text);
        }
        text.push_str(&crate::render::plain::volume(volume));
        if expand == Some(Volume::Settled(at)) {
            text.push_str(&crate::render::plain::pages(volume));
        }
    }
    // 决策点上那一卷**到此刻为止**的那一份，接在收摊了的那几卷后面（停车场 Q52）。
    // 「主区把报告画出来等你拿主意」就是这一段：判定、逐页结果、缓存用量都是真的，
    // 只有第二遍一步没走。
    //
    // **它也展得开**（`p3-session-legibility/10`，spec 的《焦点与两维模式》）：
    // 展开的索引认得出「收摊了的第几卷」与「攒着的那一份」两处（[`Volume`]），
    // 而不是「报告上第几卷」——后者根本指不到这一卷上，
    // 而 `p2-loose-ends/08` 记着**不许摊开上一卷冒充它**。
    if let Some(summarized) = live.summarized() {
        // 它的身份带着「前面收摊了几卷」（见 [`Volume::Summarized`]）：
        // 拿此刻这一份的那个数去比，指着上一卷的那个旧身份因此对不上。
        let here = expand
            == Some(Volume::Summarized {
                after: report.volumes.len(),
            });
        if here {
            opens_at = rows(&text);
        }
        text.push_str(&crate::render::plain::volume(summarized));
        if here {
            text.push_str(&crate::render::plain::pages(summarized));
        }
    }
    text.push_str(&crate::render::failing_pages(live.failed_pages()));
    if live.ended() {
        text.push_str(&crate::render::tail(report));
    }
    Unrolled { text, opens_at }
}

/// 摊开成这个样子的报告：正文，加上**展开的那一卷落在第几行**。
///
/// 两个数装在一个类型里而不是一对裸值：它们是同一次拼装出来的，
/// 而「第二个 `u16` 是什么」在调用处看不出来（那正是本仓库不爱的那种数）。
/// 没展开时 `opens_at` 是零，与「报告从头画起」同一个值——它那时没有人读。
struct Unrolled {
    text: String,
    /// 展开的那一卷的抬头在第几行。换一卷之后视口对到它上面（见 [`opens_at`]）。
    opens_at: u16,
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::super::overview::OVERVIEW_HEIGHT;
    use super::super::probe::{
        a_run_in_flight, every_kind_of_volume, main_snapshot, reversed_row, same_screen, screen,
        snapshot_of, tight,
    };
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::{Expansion, Key, Step};
    use tonefit::Mode as RunMode;

    /// 一趟**跑完了**的两卷：一卷幂等命中，一卷三种页各一张（完好、彩色、失败）。
    ///
    /// 展开那几条要的是**跑完的**那一份：这几条问的是那一副画成什么样，
    /// 而收场之后报告一行不少、也不再长——跑着的时候也展得开（`p3-session-legibility/10`
    /// 推翻了停车场 Q72），那一档另有用例（`super::state` 的
    /// `which_keys_do_what_in_which_state` 第七之二段）。
    fn a_run_worth_expanding() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 180));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.volume_finished(&fixture::three_kinds_of_page("卷二"));
        let report = live.report().clone();
        live.returned(Ok(report));
        live.rewind(Duration::from_secs(300));
        live
    }

    /// 展开着的一个会话，连同那一趟跑完的报告。
    ///
    /// `from` 照 [`super::press`] 那一层给的：视口对到那一卷的抬头上（[`opens_at`]）。
    fn expanded(volume: usize, from: u16) -> (Session, Live) {
        let live = a_run_worth_expanding();
        let mut session = Session::new();
        session.expand(Expansion::new(Volume::Settled(volume), from));
        (session, live)
    }

    /// **决策点上那一卷展得开，摊出来的是它自己的逐页**
    /// （spec 的《焦点与两维模式》第五条，`p2-loose-ends/08`）。
    ///
    /// 它停在**攒着的那一份**上、不在收摊了的那几卷里，而 08 那条硬约束记着
    /// 「不许摊开上一卷冒充它」。展开的索引因此从「报告上第几卷」改成
    /// [`Volume`]——一个下标根本指不到这一卷上，而这一改就是那条约束的解法。
    #[test]
    fn the_volume_waiting_at_the_decision_point_expands_into_its_own_pages() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let mut session = Session::new();
        session.run_started();
        session.at_the_decision_point(true);

        // 焦点切到报告区：跟随着的时候光标停的正是这一卷（它是表上最后一卷）。
        session.press(Key::Tab);
        let waiting = Volume::Summarized { after: 4 };
        assert_eq!(session.standing(&live), Some(waiting));

        // 展开它：抬头说得出展的是哪一卷，逐页那几行是**它自己的**。
        session.expand(Expansion::new(waiting, opens_at(&live, waiting)));
        let shown = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(shown.contains(&tight("展开 棋魂 08")), "{shown}");
        assert!(shown.contains(&tight("第 5/5 卷")), "{shown}");
        assert!(shown.contains(&tight("出/棋魂 08/001.png")), "{shown}");
        // 上一卷（收摊了的最后一卷）的逐页一行都不许冒充它。
        assert!(!shown.contains(&tight("出/浪客行 12/001.png")), "{shown}");

        // **它收摊之后，这一格给的仍旧是它自己**（`Live::nearest`）：
        // 「攒着的那一份」那个位置这时归**下一卷**，而展开着的是刚才那一卷。
        let mut settled = live.clone();
        let waited = fixture::processed_volume("棋魂 08", None);
        settled.volume_finished(&waited);
        settled.volume_started(Path::new("库/棋魂 09"), 1000);
        settled.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::processed_volume("棋魂 09", None)),
        );
        let after = tight(&screen(&mut session, Some(&settled), 120, 40));
        assert!(after.contains(&tight("展开 棋魂 08")), "{after}");
        assert!(after.contains(&tight("出/棋魂 08/001.png")), "{after}");
        assert!(
            !after.contains(&tight("出/棋魂 09/001.png")),
            "下一卷冒充了展开着的那一卷：{after}"
        );
    }

    /// **快照：焦点在左栏、焦点在报告区、跟随停了，三张**（票面第八条）。
    ///
    /// 三张钉的是**屏上看得出焦点在哪**（票面第一条）与**跟随停了屏上说一句**
    /// （票面第三条）。反白不进快照（[`snapshot`] 比的是字，而反白是样式），
    /// 因此另问一句「此刻反白的是哪一行」（[`reversed_row`]）——
    /// 那一处正是「焦点在哪一块」屏上唯一的载体。
    ///
    /// 跑着的一趟：**焦点切到报告区不解锁任何一个改动键**（票面第四条），
    /// 左栏三层因此照旧压暗、抬头照旧写着只读——三张里一张都没变。
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

        // 二、`⇥` 把焦点切到报告区：反白落到**最新收摊的那一卷**上（跟随着），
        // 屏底换成这一块的键，而按停那一副跟在后面。
        session.press(Key::Tab);
        same_screen(&shot(&mut session), FOCUS_ON_THE_REPORT);
        let standing = tight(&cursor(&mut session).expect("焦点在报告区，该有一行反白"));
        assert!(
            standing.contains("卷二"),
            "反白没落在最新那一卷上：{standing}"
        );

        // 三、光标往回一卷：跟随停了，抬头说一句，屏底多出 `g 回到跟随`，
        // 反白跟着落到上一卷上。
        session.select(&live, Step::Back);
        same_screen(&shot(&mut session), THE_FOLLOW_STOPPED);
        let back = tight(&cursor(&mut session).expect("该有一行反白"));
        assert!(back.contains("卷一"), "光标没往回挪一卷：{back}");

        // `g` 交回给跟随：屏上那一句没了，反白回到最新那一卷上。
        session.press(Key::Char('g'));
        same_screen(&shot(&mut session), FOCUS_ON_THE_REPORT);
    }

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const FOCUS_ON_THE_CONFIG: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告──────────────────────────────────────┐"
"│口味层 · 这一趟的立场                             █│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · ▲"
"│  适配方式　　　　默认（height）                  █│装订沟定切点 · 右开（右半在先）           ║"
"│  裁边　　　　　　默认（裁）                      ║│判据构成 低通后的局部均值误差 ＋ 颗粒超出 ║"
"│  跨页拆分　　　　默认（拆）                      ║│55.0 灰度级的那一部分（地板盲测标定于     █"
"│  拆分阈值　　　　默认（1.5）                     ║│boox-poke6，其余面板未复核）              █"
"│  阅读方向　　　　默认（rtl）                     ║│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于█"
"│  滤波器　　　　　默认（lanczos3）                ║│8 块（K 未标定占位值）                    ║"
"│  位深　　　　　　自动（判据说了算）              ║│ 记号  卷名  页数  基准档  定档页   耗时  ║"
"│  抖动　　　　　　自动（判据说了算）              ║│ –     卷一   180  跳过             3s    ║"
"│  逐页　　　　　　默认（关）                      ▼│ ✓     卷二     1  4bit    001.jpg  1m12s ▼"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" ⇥ 报告区 · 跑着…… · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷"
" ）                                                                                             "
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
"│口味层 · 这一趟的立场                             █│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · ▲"
"│  适配方式　　　　默认（height）                  █│装订沟定切点 · 右开（右半在先）           ║"
"│  裁边　　　　　　默认（裁）                      ║│判据构成 低通后的局部均值误差 ＋ 颗粒超出 ║"
"│  跨页拆分　　　　默认（拆）                      ║│55.0 灰度级的那一部分（地板盲测标定于     █"
"│  拆分阈值　　　　默认（1.5）                     ║│boox-poke6，其余面板未复核）              █"
"│  阅读方向　　　　默认（rtl）                     ║│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于█"
"│  滤波器　　　　　默认（lanczos3）                ║│8 块（K 未标定占位值）                    ║"
"│  位深　　　　　　自动（判据说了算）              ║│ 记号  卷名  页数  基准档  定档页   耗时  ║"
"│  抖动　　　　　　自动（判据说了算）              ║│ –     卷一   180  跳过             3s    ║"
"│  逐页　　　　　　默认（关）                      ▼│ ✓     卷二     1  4bit    001.jpg  1m12s ▼"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" 报告区 · ↑↓ 选一卷 · ⏎ 展开 · ⇥ 回配置 · 跑着…… · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出"
" 会话（当前卷中止，盘上不留半卷）                                                               "
"                                                                                                "
"#;

    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]。
    const THE_FOLLOW_STOPPED: &str = r#"
"┌配置 · 跑着，三层都只读───────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         ▲│ 总体 [==================>           ] 300│"
"│  型号　　　　　　未挑（跑起来之前必填）          █│ 本卷 卷三 · 第二遍 [==========>          │"
"│  感知可分辨级数　默认（跟随面板）                █│ 完成 1 卷 · 跳过 1 卷                    │"
"│  阈值　　　　　　跟着型号走（先挑一个）          █└──────────────────────────────────────────┘"
"│                                                  █┌报告 · 跟随停了───────────────────────────┐"
"│口味层 · 这一趟的立场                             █│0.5%                                      ▲"
"│  适配方式　　　　默认（height）                  █│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · ║"
"│  裁边　　　　　　默认（裁）                      ║│装订沟定切点 · 右开（右半在先）           ║"
"│  跨页拆分　　　　默认（拆）                      ║│判据构成 低通后的局部均值误差 ＋ 颗粒超出 █"
"│  拆分阈值　　　　默认（1.5）                     ║│55.0 灰度级的那一部分（地板盲测标定于     █"
"│  阅读方向　　　　默认（rtl）                     ║│boox-poke6，其余面板未复核）              █"
"│  滤波器　　　　　默认（lanczos3）                ║│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于║"
"│  位深　　　　　　自动（判据说了算）              ║│8 块（K 未标定占位值）                    ║"
"│  抖动　　　　　　自动（判据说了算）              ║│ 记号  卷名  页数  基准档  定档页   耗时  ║"
"│  逐页　　　　　　默认（关）                      ▼│ –     卷一   180  跳过             3s    ▼"
"└──────────────────────────────────────────────────┘└──────────────────────────────────────────┘"
" 报告区 · ↑↓ 选一卷 · ⏎ 展开 · g 回到跟随 · ⇥ 回配置 · 跑着…… · s 停（按一次收尾，再按一次中止）"
" · Ctrl-C 退出会话（当前卷中止，盘上不留半卷）                                                  "
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
        let last = table(&live, 94, None).rows.pop().expect("表上有卷");

        // 只给四行的格子：最新那一卷仍在，抬头已经让位。
        let squeezed = main_snapshot(&live, 96, 4 + OVERVIEW_HEIGHT);

        assert!(
            squeezed.contains(last.text.trim_end()),
            "最新收摊的那一卷掉出去了：{squeezed}"
        );
        assert!(
            !squeezed.contains("适配方式 以高为准"),
            "四行的格子装不下抬头，它却还在：{squeezed}"
        );
        // 一格都不剩的格子问不出滚动量，也不恐慌（[`Viewport`] 那一头的规矩）。
        let rows = collapsed(&live, 94, None, false).rows.len();
        assert_eq!(Viewport::new(rows, 0, rows.saturating_sub(1)).from(), 0);
        // 窄到一格正文都不剩：折不出比一个字更窄的行（[`crate::wrap::fold`]），
        // 表也砍无可砍——两头都不恐慌就够了。
        main_snapshot(&live, 2, 4 + OVERVIEW_HEIGHT);
        assert!(!table(&live, 0, None).rows.is_empty(), "砍无可砍时表也还在");
    }

    /// **六种卷各有各的行**（票面第二条）：跳过、隔离、卷级失败、逐页、覆盖、等答话。
    ///
    /// 逐条问的是那一行**长什么样**，不是屏上第几行：档位那一列写什么出自
    /// [`crate::render::base_column`]，行首记号与行尾那句话是这一层的事
    /// （见 [`super::table`]）。**不重排**——六行的先后就是跑完的先后。
    #[test]
    fn six_kinds_of_volume_each_get_their_own_row() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);
        let rows = table(&live, 120, None).rows;
        let body: Vec<&str> = rows.iter().skip(1).map(|row| row.text.as_str()).collect();

        assert_eq!(body.len(), 6, "六种卷六行：{body:?}");
        // 一、跳过：档位那一列写「跳过」，定档页留空，**耗时照给**。
        assert!(
            body[0].starts_with(" –"),
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
        assert!(body[4].contains('—'), "页数那一格该留个记号：{}", body[4]);
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
        let body: Vec<String> = table(&live, 120, None)
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

        let rows = table(&live, 120, None).rows;

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
            table(&plain, 120, None)
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
        let wide = table(&live, 120, None).rows;

        for volume in &live.report().volumes {
            let name = crate::render::volume_name(&volume.volume);
            assert!(
                wide.iter().any(|row| row.text.contains(&name)),
                "{name} 没上表：{wide:?}"
            );
        }
        // 窄到一列都砍无可砍：卷名从中间省略，书名与第几卷两头都还认得出。
        let narrow = table(&live, 26, None).rows;
        // 第五行是「消失的那卷」——十格的名字收进六格里。
        let elided = &narrow.iter().skip(1).nth(4).expect("那一行在表上").text;
        assert!(elided.contains('…'), "该省略却没省略：{elided}");
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

        let narrow = tight(&screen(&mut session, Some(&live), 80, 24));

        for mark in ['–', '!', '✗'] {
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
    /// 三种各问一遍：报告末尾那几小结、抬头里互锁那几行、以及没做成那一卷的那句原因。
    /// 折行走 [`crate::wrap`]，因此它们在窄格子里会折下来，但**一个字都不少**。
    #[test]
    fn the_sentences_are_still_drawn_as_folded_prose() {
        let mut live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        let report = live.report().clone();
        live.returned(Ok(report));

        let drawn = |width: u16| -> String {
            collapsed(&live, width, None, false)
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
        // 四、互锁那几行同样是句子。默认那一套一条都不咬，换一个咬得上的适配方式。
        let mut inside = Live::new(
            &tonefit::Request {
                fit: tonefit::FitMode::Inside,
                ..fixture::request(RunMode::Process)
            },
            Resuming::GoesOn,
        );
        inside.run_started(1, 1000);
        inside.volume_finished(&fixture::processed_volume("卷一", None));
        let said = collapsed(&inside, 120, None, false)
            .rows
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            said.contains("互锁 拆分开着，适配方式却是 fit-inside"),
            "{said}"
        );
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
        let snapshot = main_snapshot(&a_run_in_flight(false), 96, 30);

        same_screen(&snapshot, WITHOUT_A_FAILED_PAGE);
    }

    /// 见 [`the_report_pane_without_a_failed_page`]。
    const WITHOUT_A_FAILED_PAGE: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                              │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                             │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于         │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                               │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                     │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于 boox-poke6，其│"
"│余面板未复核）                                                                                │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                             │"
"│ 记号  卷名  页数  基准档  定档页   耗时                                                      │"
"│ –     卷一   180  跳过             3s                                                        │"
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
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：同一趟，其中一卷有失败页。**
    ///
    /// 「失败页出现的当场就在主区可见，带原因」——那一段与表上那一行的隔离记号
    /// 并排出现，两者说的是同一份原因（一份是增量，一份是结果）。
    #[test]
    fn the_report_pane_with_a_failed_page() {
        let snapshot = main_snapshot(&a_run_in_flight(true), 96, 36);

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
"┌报告──────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于         │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                               │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                     │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于 boox-poke6，其│"
"│余面板未复核）                                                                                │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                             │"
"│ 记号  卷名  页数  基准档  定档页   耗时                                                      │"
"│ –     卷一   180  跳过             3s                                                        │"
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

        same_screen(&main_snapshot(&live, 120, 26), THE_TABLE_WIDE);
    }

    /// 见 [`the_volume_table_on_a_wide_terminal`]。
    const THE_TABLE_WIDE: &str = r#"
"┌执行 · 第 5/6 卷 · 还剩约 1m00s───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s                                                      │"
"│                                                                                                                      │"
"│ 完成 3 卷 · 跳过 1 卷                                                                                                │"
"│ 出事 隔离 1 卷 · 失败 1 页 · 卷级失败 1 卷                                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于 boox-poke6，其余面板未复核）    │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                                                       │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                                        │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                                             │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于 boox-poke6，其余面板未复核）          │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                                                     │"
"│ 记号  卷名        页数  基准档        定档页   耗时                                                                  │"
"│ –     棋魂 07      184  跳过                   3s                                                                    │"
"│ !     哆啦 03        2  4bit          001.jpg  1m12s  隔离                                                           │"
"│ ✓     名侦探 05      1  逐页                   1m12s                                                                 │"
"│ ✓     浪客行 12      1  覆盖 2bit+FS           1m12s                                                                 │"
"│ ✗     消失的那卷     —  没做成                        卷根不在了                                                     │"
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

        same_screen(&snapshot_of(&mut session, &live, 80, 24), THE_TABLE_NARROW);
    }

    /// 见 [`the_volume_table_on_the_narrowest_terminal`]。
    const THE_TABLE_NARROW: &str = r#"
"┌配置────────────────────────────────────────────┐┌执行 · 第 5/6 卷 · 还剩约 1m┐"
"│设备层 · 判定的依据，绑面板，改一次管很久       ▲│ 总体 [=====================│"
"│  型号　　　　　　未挑（跑起来之前必填）        █│                            │"
"│  感知可分辨级数　默认（跟随面板）              █│ 完成 3 卷 · 跳过 1 卷      │"
"│  阈值　　　　　　跟着型号走（先挑一个）        █│ 出事 隔离 1 卷 · 失败 1 页 │"
"│                                                █└────────────────────────────┘"
"│口味层 · 这一趟的立场                           █┌报告────────────────────────┐"
"│  适配方式　　　　默认（height）                █│那一部分（地板盲测标定于    ▲"
"│  裁边　　　　　　默认（裁）                    █│boox-poke6，其余面板未复核）║"
"│  跨页拆分　　　　默认（拆）                    █│判据聚合 分块 32×32 · 尾巴取║"
"│  拆分阈值　　　　默认（1.5）                   ║│p99，但不宽于 8 块（K 未标定║"
"│  阅读方向　　　　默认（rtl）                   ║│占位值）                    ║"
"│  滤波器　　　　　默认（lanczos3）              ║│ 记号  卷名     基准档      █"
"│  位深　　　　　　自动（判据说了算）            ║│ –     棋魂 07  跳过        █"
"│  抖动　　　　　　自动（判据说了算）            ║│ !     哆啦 03  4bit        █"
"│  逐页　　　　　　默认（关）                    ║│ 隔离                       █"
"│  缓存预算　　　　默认（512.0 MiB）             ║│ ✓     名… 05   逐页        ║"
"│  读取策略　　　　默认（auto）                  ║│ ✓     浪… 12   覆盖 2bit+FS║"
"│                                                ║│ ✗     消…那卷  没做成      ║"
"│范围层 · 每趟都不同，不进预设                   ▼│ 卷根不在了                 ▼"
"└────────────────────────────────────────────────┘└────────────────────────────┘"
" ←→ 换一个 · ⏎ 摊开 · c 出标定图 · ↑↓ 选 · t 试算 · x 执行 · e 展开 · p 预设 · q"
" 退出                                                                           "
"                                                                                "
"#;

    /// **快照：试算那一副。**（票面第七条：试算那一张）
    ///
    /// 与执行那一副差的是抬头那一句 `dry-run` 与总览块那两行——表本身一列不变，
    /// 那正是「同一批行，两副排版」买到的东西。续做那一趟停在决策点上，
    /// **等答话那一卷因此也在这张表上**（末一行）。
    #[test]
    fn the_volume_table_of_a_dry_run() {
        let live = every_kind_of_volume(RunMode::DryRun, Resuming::Waits);

        same_screen(&main_snapshot(&live, 120, 26), THE_TABLE_OF_A_DRY_RUN);
    }

    /// 见 [`the_volume_table_of_a_dry_run`]。
    const THE_TABLE_OF_A_DRY_RUN: &str = r#"
"┌试算 · 第 6/6 卷 · 还剩约 1m00s───────────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s                                                      │"
"│ 本卷 棋魂 08 · 第二遍 [>                             ] 0/1000 步                                                     │"
"│ 判定 1 卷 4bit · 1 卷 覆盖 2bit+FS · 1 卷 跳过 · 1 卷 逐页                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于 boox-poke6，其余面板未复核）    │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                                                       │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                                        │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                                             │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于 boox-poke6，其余面板未复核）          │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                                                     │"
"│dry-run：只算不写，下面的路径都还没落盘                                                                               │"
"│ 记号  卷名        页数  基准档        定档页   耗时                                                                  │"
"│ –     棋魂 07      184  跳过                   3s                                                                    │"
"│ !     哆啦 03        2  4bit          001.jpg  1m12s  隔离                                                           │"
"│ ✓     名侦探 05      1  逐页                   1m12s                                                                 │"
"│ ✓     浪客行 12      1  覆盖 2bit+FS           1m12s                                                                 │"
"│ ✗     消失的那卷     —  没做成                        卷根不在了                                                     │"
"│ ✓     棋魂 08        1  4bit          001.jpg  1m12s  等你拿主意                                                     │"
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

        same_screen(&main_snapshot(&live, 96, 18), WAITING_AT_THE_DECISION_POINT);
    }

    /// 见 [`the_volume_waiting_at_the_decision_point_gets_a_row_of_its_own`]。
    const WAITING_AT_THE_DECISION_POINT: &str = r#"
"┌试算 · 第 2/2 卷 · 还剩约 5m00s───────────────────────────────────────────────────────────────┐"
"│ 总体 [===============>              ] 1000/2000 步 · 已用 5m00s                              │"
"│ 本卷 棋魂 08 · 第二遍 [>                             ] 0/1000 步                             │"
"│ 判定 1 卷 跳过                                                                               │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────┐"
"│boox-poke6，其余面板未复核）                                                                  ▲"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                               █"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                █"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                     █"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于 boox-poke6，其█"
"│余面板未复核）                                                                                █"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                             ║"
"│dry-run：只算不写，下面的路径都还没落盘                                                       ║"
"│ 记号  卷名     页数  基准档  定档页   耗时                                                   ║"
"│ –     棋魂 07   184  跳过             3s                                                     ║"
"│ ✓     棋魂 08     1  4bit    001.jpg  1m12s  等你拿主意                                      ▼"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：展开一卷的逐页，左栏收起、主区吃满宽度。**（票面第二、三条）
    ///
    /// 钉的是整屏：左栏一格都不在，右边那一大格从第 0 列画到第 119 列。
    /// 逐页那两行走的是 [`crate::render::pages`]——命令行印出来的是同一批字。
    #[test]
    fn expanding_a_volume_collapses_the_left_column_and_shows_the_pages() {
        let (mut session, live) =
            expanded(1, opens_at(&a_run_worth_expanding(), Volume::Settled(1)));

        same_screen(
            &snapshot_of(&mut session, &live, 120, 25),
            EXPANDED_TO_ONE_VOLUME,
        );
    }

    /// 见 [`expanding_a_volume_collapses_the_left_column_and_shows_the_pages`]。
    const EXPANDED_TO_ONE_VOLUME: &str = r#"
"┌收场 点名的卷都走过了 · 2 卷 · 用了 0s────────────────────────────────────────────────────────────────────────────────┐"
"│ 总体 [==============================] 2000/2000 步                                                                   │"
"│ 判定 1 卷 4bit · 1 卷 跳过                                                                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 卷二（第 2/2 卷）─────────────────────────────────────────────────────────────────────────────────────────┐"
"│  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做                            │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                                              │"
"│库/卷二 → 出/隔离/卷二（3 页，其中彩页 1 页）                                                                         │"
"│  隔离 1 页失败：本卷整卷写到隔离目录 出/隔离/卷二，失败页以卷内统一尺寸留白占位，页序不断                            │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                                             │"
"│  卷级 基准档 4bit · 其余 1 页 · 特例 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 · 特例判据 p75 立脚点、3.0× │"
"│    定档页 库/卷二/001.jpg                                                                                            │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                                              │"
"│  缓存 1 页 1.0 MiB（压缩前 4.0 MiB），未溢写（预算 512.0 MiB）                                                       │"
"│  1182×1680  缩放比 1.219 · 未预缩  出/隔离/卷二/001.png                                                              │"
"│    判定 4bit（阈值内最低的一档）  判据 1bit+FS 32.000 · 2bit 20.000 · 4bit 8.000 · 8bit 2.000                        │"
"│  1182×1680  缩放比 1.219 · 未预缩  出/隔离/卷二/002.png                                                              │"
"│    彩页 · 彩色分支：只缩放，不量化，不进灰度缓存也不进卷级上包络                                                     │"
"│  1182×1680  失败页 · 卷内统一尺寸留白  出/隔离/卷二/017.png                                                          │"
"│    失败 解不出完整尺寸：JPEG 数据截断                                                                                │"
"│隔离 1 卷 · 失败 1 页：失败页以卷内统一尺寸留白占位，原因逐条列在上面                                                 │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 翻一行 · ←→ 横着滚 · ⇥／⇧⇥ 换下一卷／上一卷 · e／Esc 收起，左栏回来 · q 退出                                        "
" 逐页那两行不折行：屏窄时行尾被切掉，往右滚就看得到——页面不会跟着整体错位                                               "
"                                                                                                                        "
"#;

    /// **逐页那几行只在展开的那一卷上出，而且逐字来自 [`crate::render::pages`]。**
    ///
    /// 票面第一条（默认只给卷级）与「渲染只有一处出处」两件事由同一条钉：
    /// 展开出来的那几行**逐行**能在 `render::pages` 的输出里找到，
    /// 而没展开的那一份一行都没有。失败页与彩页那两句因此也不必在这里另比一遍——
    /// 它们本来就在那一份里（票面后两条）。
    #[test]
    fn the_per_page_rows_come_from_render_and_only_for_the_volume_that_is_open() {
        let live = a_run_worth_expanding();
        let opened = &live.report().volumes[1];
        let pages = crate::render::plain::pages(opened);

        let folded = report_text(&live, None).text;
        let unfolded = report_text(&live, Some(Volume::Settled(1))).text;

        for line in pages.lines() {
            assert!(unfolded.contains(line), "展开之后少了这一行：{line}");
            assert!(!folded.contains(line), "没展开却给了逐页：{line}");
        }
        // 那一份里说得出失败页的尺寸是**卷内统一尺寸**、彩页不量化也不进上包络。
        assert!(pages.contains("失败页 · 卷内统一尺寸留白"), "{pages}");
        assert!(pages.contains("彩色分支：只缩放，不量化"), "{pages}");
        // 卷级那几行两副样子里都在——展开是**添**，不是换一份。
        assert!(folded.contains("库/卷二 → 出/隔离/卷二"), "{folded}");
        assert!(unfolded.contains("库/卷二 → 出/隔离/卷二"), "{unfolded}");
        // 幂等命中的卷展开了也一行逐页都没有（`render::pages` 那道守卫），不恐慌。
        assert_eq!(crate::render::plain::pages(&live.report().volumes[0]), "");
        assert!(
            !report_text(&live, Some(Volume::Settled(0)))
                .text
                .contains("判定 4bit")
        );
    }

    /// **屏窄时逐页那几行不折断，横着滚看得到行尾，而页面不整体错位**（票面第四条）。
    ///
    /// 窄到 60 列：逐页那一行的判据一串被切在框外——那一行本来就轻松过 100 列，
    /// 而这正是「宽度是稀缺资源」的现场。往右滚到底它就回来了，
    /// 而边框、抬头与屏底那两行一格都没动：滚的是格子里的正文，不是这一屏。
    #[test]
    fn the_expanded_report_scrolls_sideways_instead_of_folding_a_page_row() {
        let (mut session, live) =
            expanded(1, opens_at(&a_run_worth_expanding(), Volume::Settled(1)));
        // 判据那一串的最后一个候选——逐页那一行的行尾。
        let pages = crate::render::plain::pages(&live.report().volumes[1]);
        let tail = pages
            .lines()
            .find(|line| line.contains("判据"))
            .and_then(|line| line.rsplit(" · ").next())
            .expect("逐页那一行里有判据");

        let narrow = snapshot_of(&mut session, &live, 60, 20);
        assert!(
            narrow.contains("判据 1bit+FS"),
            "判据那一行不在：\n{narrow}"
        );
        assert!(!narrow.contains(tail), "行尾折下来了：\n{narrow}");
        let frame: Vec<&str> = narrow.lines().collect();
        assert!(frame[0].starts_with("\"┌收场"), "{narrow}");

        // 往右滚到底：行尾回来了，而滚动量停在真滚得动的那一格上（不是按了没反应）。
        for _ in 0..20 {
            session.press(Key::Right);
        }
        let rolled = snapshot_of(&mut session, &live, 60, 20);
        assert!(rolled.contains(tail), "往右滚到底也没看到行尾：\n{rolled}");
        let stopped = session.expansion().expect("展开着").right;
        session.press(Key::Right);
        snapshot_of(&mut session, &live, 60, 20);
        assert_eq!(
            session.expansion().expect("展开着").right,
            stopped,
            "滚到底了还往右挪"
        );
        // 边框在原处：横着滚不把这一屏带歪（票面：页面不整体错位）。
        assert_eq!(
            rolled.lines().next(),
            frame.first().copied(),
            "横着滚把整屏也带歪了：\n{rolled}"
        );

        // 收起：左栏回来，报告区又只给卷级。
        session.press(Key::Esc);
        let back = snapshot_of(&mut session, &live, 60, 20);
        assert!(back.contains("配置"), "收起之后左栏没回来：\n{back}");
        assert!(!back.contains("判据 1bit+FS"), "收起了还给逐页：\n{back}");
    }

    /// **翻得回去**（停车场 Q64）：展开那一副不自动滚到底，`↑` 翻到零就是抬头那几行。
    ///
    /// 默认那一副**跟随着的时候**仍旧滚到底（见
    /// [`a_report_taller_than_the_pane_keeps_its_last_lines`]），那是跟着跑的时候该有的落点；
    /// 它那一头翻回去靠的是卷表上那个光标（跟随一停视口就跟着它走，
    /// 见 [`the_focus_and_the_stopped_follow_are_both_visible_on_screen`]）。
    #[test]
    fn the_expanded_report_can_be_scrolled_back_to_the_header() {
        // 展开那一下 `from` 是零：抬头那几行当场就在屏上。
        let (mut session, live) = expanded(0, 0);
        let top = screen(&mut session, Some(&live), 100, 20);
        assert!(tight(&top).contains(&tight("适配方式")), "{top}");

        // 往下翻够远，抬头让位；再往回翻，它一行不少地回来。
        for _ in 0..8 {
            session.press(Key::Down);
        }
        let down = screen(&mut session, Some(&live), 100, 20);
        assert!(!tight(&down).contains(&tight("适配方式")), "{down}");
        for _ in 0..8 {
            session.press(Key::Up);
        }
        assert_eq!(
            screen(&mut session, Some(&live), 100, 20),
            top,
            "翻回去之后不是原来那一屏"
        );
    }

    /// **展开一卷没有失败页的：逐页那几行照给，而失败那一句一个字都没有。**
    ///
    /// spec 的《Testing Decisions》要「有失败页与没有两种」，
    /// 而展开那一副的另一种在这里：`processed_volume(_, None)` 一页完好的灰度页，
    /// 逐页那两行照出（尺寸、缩放、判定、判据），隔离与失败那两句一句不出。
    #[test]
    fn a_volume_without_a_failed_page_expands_to_page_rows_and_nothing_about_failure() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷三"), 1000);
        live.volume_finished(&fixture::processed_volume("卷三", None));
        let report = live.report().clone();
        live.returned(Ok(report));
        let mut session = Session::new();
        // 对到那一卷的抬头上（`⇥` 换过一卷之后的落位），逐页那几行才在这一格里。
        session.expand(Expansion::new(
            Volume::Settled(0),
            opens_at(&live, Volume::Settled(0)),
        ));

        let open = snapshot_of(&mut session, &live, 120, 22);

        assert!(
            open.contains("判定 4bit"),
            "逐页那两行没出：
{open}"
        );
        assert!(open.contains("出/卷三/001.png"), "{open}");
        for absent in ["失败", "隔离", "彩页"] {
            assert!(
                !open.contains(absent),
                "没有失败页却说了「{absent}」：
{open}"
            );
        }
    }
}
