//! 屏上那一块：**报告区**——主区最下面那一段，边跑边攒的那一份
//! （`CONTEXT.md` 的《会话》：报告区）。
//!
//! 措辞出自 [`crate::render`]：命令行与会话共用那几个函数，一个字都不在这里重写。
//! 一卷跑完那条事件带着那一卷的报告（ADR 0011），[`crate::render::volume`] 收下它
//! 就画得出判定、驱动页、隔离与这一趟怎么读的。
//!
//! **两副样子由展开与否分**（`CONTEXT.md` 的《会话》：展开），差在哪几处见
//! [`report_pane`] 那张表。折行走 [`crate::wrap`]，本模块只交代**折到多宽**——
//! `--help` 与命令行印出来的报告折的是同一套，而那两处根本没有终端库。
//!
//! 这一段在主区占几行由 [`super::main_pane`] 分；上面那两条横条在 [`super::bars`]。

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::session::live::Live;
use crate::session::state::Session;
use crate::wrap;

/// 报告区：**边跑边攒**的那一份，措辞出自 [`crate::render`]。
///
/// 卷级那几行一卷跑完就画得出来（一卷跑完那条事件带着那一卷的报告，ADR 0011）：
/// 判定、驱动页、隔离、这一趟怎么读的、幂等命中说清哪四项依据没变，全在里面。
/// 失败页要在出现的当场看得见，那一段走 [`crate::render::failing_pages`]。
///
/// **两副样子，由展开与否分**（`CONTEXT.md` 的《会话》：展开）：
///
/// | | 只给卷级（默认） | 展开一卷 |
/// |---|---|---|
/// | 逐页那几行 | 一行不给 | 展开的那一卷全给（[`crate::render::pages`]） |
/// | 长过一格 | **滚到底**，留最后那几行 | 用户自己翻（[`Expansion::from`]） |
/// | 一行放不下 | 折行（按显示宽度，见 [`crate::wrap`]） | **不折**，横着滚 |
/// | 左栏 | 在场 | 收起（见 [`super::shell`]） |
///
/// 默认那一副滚到底，是因为报告只增不减，而「一卷跑完当场看得见」说的正是刚添上去的
/// 那几行；展开那一副翻得回去，那正是停车场 Q64 记着的缺口。
/// 展开那一副不折行，是因为票面写着**逐页行不被折断**——折了就看不出哪几个数是一页的。
pub(super) fn report_pane(
    session: &mut Session,
    live: Option<&Live>,
    area: Rect,
) -> Paragraph<'static> {
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
        return Paragraph::new(folded_lines(
            "
 按 t 试算：只算不写，报告照出。
              按 x 执行：写到输出根。
              跑起来之前必填的两项是型号与输出根。",
            inside.width,
        ))
        .block(block);
    };
    let Some(expansion) = session.expansion() else {
        // 折行是自己算的（[`crate::wrap`]）：折出来几行当场就数得出，报告区因此不必再
        // 往一块临时缓冲上画一遍再数底下空着几行（停车场 Q65）。
        let text = report_text(live, None).text;
        let rows = folded_lines(&text, inside.width);
        let past = past_the_top(rows.len(), inside);
        return Paragraph::new(rows).scroll((past, 0)).block(block);
    };
    let text = report_text(live, Some(expansion.volume)).text;
    // 翻页量收进这一格真滚得动的范围。不收的话，翻过了头再翻回来，
    // 头几下会按了没反应（见 [`Session::clamp_report`]）。
    session.clamp_report(
        rows(&text).saturating_sub(inside.height),
        widest(&text).saturating_sub(inside.width),
    );
    let expansion = session.expansion().expect("刚才还展开着");
    Paragraph::new(text)
        .scroll((expansion.from, expansion.right))
        .block(block)
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
    let (Some(expansion), Some(live)) = (session.expansion(), live) else {
        return "报告".to_owned();
    };
    let Some(volume) = live.report().volumes.get(expansion.volume) else {
        return "报告".to_owned();
    };
    format!(
        "报告 · 展开 {}（第 {}/{} 卷）",
        crate::render::volume_name(&volume.volume),
        expansion.volume + 1,
        expansion.volumes
    )
}

/// 这一趟有没有一卷可以展开。**与 [`super::super::expand`] 挡在前面的那两条同一个判据**：
/// 没跑过、或者报告里一卷都还没有，展开就无从谈起。
pub(super) fn expandable(live: Option<&Live>) -> bool {
    live.is_some_and(|live| !live.report().volumes.is_empty())
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

/// 把一段文字折成这一格摆得下的那几行（[`crate::wrap`]）。
///
/// 折行的规矩因此在**终端库之外**：`--help` 与命令行印出来的报告折的是同一套，
/// 而那两处根本没有终端库（见 `crate::wrap` 的《三处共用这一份》）。
fn folded_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    wrap::fold(text, width)
        .into_iter()
        .map(Line::from)
        .collect()
}

/// 展开第 `volume` 卷之后，那一卷的抬头落在第几行。
///
/// **给 [`super::super::press`] 用**：`⇥` 换过一卷之后视口要对到那一卷的抬头上，
/// 而状态机算不出这个数——它读不到那一趟攒着的报告。算它只用得到
/// [`crate::render`] 那几个函数与 [`Live`]，一个终端都不碰。
pub(in crate::session) fn opens_at(live: &Live, volume: usize) -> u16 {
    report_text(live, Some(volume)).opens_at
}

/// 报告折行之后有几行掉在这一格**上面**——把它当滚动量，格子里留下的就是最后那几行。
///
/// 报告只增不减：从头画的话，跑到第十几卷时新添的那几行全掉在格子外面，
/// 而这一票要的正是「不必等全部跑完才发现参数错了」。收场之后留最后那几行同样是对的：
/// 末尾那几小结按「这一趟出的事有多重」往下排，最重的压在最后（见 [`crate::render::tail`]）。
///
/// **翻回去看前面几卷走展开那一副**（`e`）：那一副不自动滚，`↑↓` 翻得动，
/// 翻到零就是抬头那几行（停车场 Q64 记着的正是这一条代价）。
///
/// **折行有几行是数出来的，不是估出来的**：[`folded_lines`] 出的就是折完的那几行，
/// `len()` 即行数。
/// 从前这里往一块临时缓冲上真画一遍再数底下空着几行——那是因为折行的规矩在终端库那一份里，
/// 而它那个直接答得出行数的 `Paragraph::line_count` 挂着 unstable 的门（停车场 Q65）。
/// 折行搬进 [`crate::wrap`] 之后，这个数与折出来的那几行是同一次算出来的，那道门不必开。
fn past_the_top(rows: usize, inside: Rect) -> u16 {
    if inside.width == 0 || inside.height == 0 {
        return 0;
    }
    u16::try_from(rows)
        .unwrap_or(u16::MAX)
        .saturating_sub(inside.height)
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
fn report_text(live: &Live, expand: Option<usize>) -> Unrolled {
    let report = live.report();
    let mut text = crate::render::header(report, live.mode());
    let mut opens_at = 0;
    for (at, volume) in report.volumes.iter().enumerate() {
        if expand == Some(at) {
            opens_at = rows(&text);
        }
        text.push_str(&crate::render::volume(volume));
        if expand == Some(at) {
            text.push_str(&crate::render::pages(volume));
        }
    }
    // 决策点上那一卷**到此刻为止**的那一份，接在收摊了的那几卷后面（停车场 Q52）。
    // 「主区把报告画出来等你拿主意」就是这一段：判定、逐页结果、缓存用量都是真的，
    // 只有第二遍一步没走。逐页那几行这里不给——与卷级默认那一副同一条（票面第一条），
    // 它也展不开：展开索引数的是报告上收摊了的那几卷，而这一卷还不在里面。
    if let Some(summarized) = live.summarized() {
        text.push_str(&crate::render::volume(summarized));
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

    use super::super::bars::BAR_HEIGHT;
    use super::super::probe::{
        a_run_in_flight, main_snapshot, same_screen, screen, snapshot_of, tight,
    };
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::{Expansion, Key};
    use tonefit::Mode as RunMode;

    /// 一趟**跑完了**的两卷：一卷幂等命中，一卷三种页各一张（完好、彩色、失败）。
    ///
    /// 展开那几条要的是跑完的那一份：展开只从浏览进得去（`super::state` 的
    /// `expanded_action`，停车场 Q72），而浏览意味着没有一趟正跑着。
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
    /// `from` 照 [`super::press`] 那一层给的：展开那一下是零（报告从头画），
    /// `⇥` 换过一卷是 [`opens_at`]（视口对到那一卷的抬头上）。
    fn expanded(volume: usize, from: u16) -> (Session, Live) {
        let live = a_run_worth_expanding();
        let mut session = Session::new();
        let volumes = live.report().volumes.len();
        session.expand(Expansion::new(volume, volumes, from));
        (session, live)
    }

    /// 报告长过一格时，留下的是**最后**那几行——新添上去的那几行不该掉到格子外面。
    ///
    /// 这一条与那两张快照是一对：快照里格子够高、一行都不少，这里问格子不够高时留谁。
    #[test]
    fn a_report_taller_than_the_pane_keeps_its_last_lines() {
        let live = a_run_in_flight(true);
        let full = report_text(&live, None).text;
        let last = full.lines().next_back().expect("报告不是空的");

        // 只给四行的格子：最后那一行仍在，头一行已经让位。
        let squeezed = main_snapshot(&live, 96, 4 + BAR_HEIGHT * 2);

        assert!(squeezed.contains(last), "最新的那一行掉出去了：{squeezed}");
        assert!(
            !squeezed.contains("适配方式 以高为准"),
            "四行的格子装不下抬头，它却还在：{squeezed}"
        );
        // 一格都不剩的格子问不出滚动量，也不恐慌。
        assert_eq!(
            past_the_top(wrap::fold(&full, 96).len(), Rect::new(0, 0, 96, 0)),
            0
        );
        assert_eq!(
            past_the_top(wrap::fold(&full, 0).len(), Rect::new(0, 0, 0, 10)),
            0
        );
    }

    /// **快照：展开一卷的逐页，左栏收起、主区吃满宽度。**（票面第二、三条）
    ///
    /// 钉的是整屏：左栏一格都不在，右边那一大格从第 0 列画到第 119 列。
    /// 逐页那两行走的是 [`crate::render::pages`]——命令行印出来的是同一批字。
    #[test]
    fn expanding_a_volume_collapses_the_left_column_and_shows_the_pages() {
        let (mut session, live) = expanded(1, opens_at(&a_run_worth_expanding(), 1));

        same_screen(
            &snapshot_of(&mut session, &live, 120, 25),
            EXPANDED_TO_ONE_VOLUME,
        );
    }

    /// 见 [`expanding_a_volume_collapses_the_left_column_and_shows_the_pages`]。
    const EXPANDED_TO_ONE_VOLUME: &str = r#"
"┌整趟──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 收场 点名的卷都走过了 · 2 卷 · 用了 0s                                                                               │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌当前卷────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐"
"│                                                                                                                      │"
"└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 卷二（第 2/2 卷）─────────────────────────────────────────────────────────────────────────────────────────┐"
"│库/卷二 → 出/隔离/卷二（3 页，其中彩页 1 页）                                                                         │"
"│  隔离 1 页失败：本卷整卷写到隔离目录 出/隔离/卷二，失败页以卷内统一尺寸留白占位，页序不断                            │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                                             │"
"│  卷级 基准档 4bit · 主体 1 页 · 离群 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 · 离群判据 p75 立脚点、3.0× │"
"│    驱动页 库/卷二/001.jpg                                                                                            │"
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
        let pages = crate::render::pages(opened);

        let folded = report_text(&live, None).text;
        let unfolded = report_text(&live, Some(1)).text;

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
        assert_eq!(crate::render::pages(&live.report().volumes[0]), "");
        assert!(!report_text(&live, Some(0)).text.contains("判定 4bit"));
    }

    /// **屏窄时逐页那几行不折断，横着滚看得到行尾，而页面不整体错位**（票面第四条）。
    ///
    /// 窄到 60 列：逐页那一行的判据一串被切在框外——那一行本来就轻松过 100 列，
    /// 而这正是「宽度是稀缺资源」的现场。往右滚到底它就回来了，
    /// 而边框、抬头与屏底那两行一格都没动：滚的是格子里的正文，不是这一屏。
    #[test]
    fn the_expanded_report_scrolls_sideways_instead_of_folding_a_page_row() {
        let (mut session, live) = expanded(1, opens_at(&a_run_worth_expanding(), 1));
        // 判据那一串的最后一个候选——逐页那一行的行尾。
        let pages = crate::render::pages(&live.report().volumes[1]);
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
        assert!(frame[0].starts_with("\"┌整趟"), "{narrow}");

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
    /// 默认那一副仍旧滚到底（见 [`a_report_taller_than_the_pane_keeps_its_last_lines`]），
    /// 那是跟着跑的时候该有的落点；翻回去是展开那一副的事。
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
        session.expand(Expansion::new(0, 1, opens_at(&live, 0)));

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
