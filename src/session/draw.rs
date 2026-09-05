//! 会话的画法：左窄配置常驻 + 右宽主区（spec 的《会话：布局与交互》）。
//!
//! 主区自上而下两块：**总览块**（这一趟是什么 · 走到哪儿 · 怎么样）与**报告区**
//! （边跑边攒）。报告区有两副样子——默认只给卷级，**展开**一卷时逐页那几行
//! 摊开、左栏收起、主区吃满宽度（见 [`report::report_pane`] 与 [`shell`]）。
//!
//! # 屏上那几块各住在哪儿
//!
//! **本模块只把屏分成格子**：哪一格多宽多高、放不下时让谁、什么时候整个收起。
//! 每一格里画什么在各自的模块里，**一块一个**：
//!
//! | 屏上那一块 | 住在 |
//! |---|---|
//! | 左栏：三层配置 | [`config`] |
//! | 预设栏 | [`picker`] |
//! | 主区上面那一块：总览块 | [`overview`] |
//! | 主区下面那一块：报告区 | [`report`] |
//! | 屏底那几行 | [`footer`] |
//!
//! 分法按**屏上那几块**走，不按「工具函数 vs 业务」——后者一年后没人分得清一个函数
//! 该归哪一边。整层连同这几个模块都在 `tui` 后面（分界见 `super` 的《终端库在哪一半》）。
//! 那几块共用的测试探针在 `probe`（`#[cfg(test)]`，不进非 test 那一趟的文档）。
//!
//! # 纵向摆不下的时候
//!
//! **从第几行画起是算出来的**，由 [`Viewport`] 那个纯函数答——它摆在终端库外面
//! （分界见 `super` 的《终端库在哪一半》）。**哪几块共用它、哪一块画滚动条，
//! 那张表在 [`Viewport`] 上**，本模块不抄第二份。
//!
//! 这一层要做的只有一件事：**正文与那一条滚动条一起画的地方只有 [`scrolling`] 一处**。
//!
//! # 措辞在这里长第二份的只有两样
//!
//! 报告区画的是 [`crate::render`] 那几个函数——命令行与会话共用，一个字都不在这里重写。
//! 「边跑边攒」因此不必另有一套说法：一卷跑完那条事件带着那一卷的报告（ADR 0011），
//! [`crate::render::volume`] 收下它就画得出判定、定档页、隔离与这一趟怎么读的。
//!
//! 长在画法这一层的只有**命令行上根本没有**的那两样：左栏那几行配置的标签与按键提示
//! （[`config`] 与 [`footer`]），以及总览块的排版（[`overview`]）。
//!
//! # 折行不在这里
//!
//! 报告区与屏底那一格的折行走 [`crate::wrap`]——`--help` 与命令行印出来的报告折的是
//! 同一套，而那两处根本没有终端库。这一层只交代**折到多宽**：那一格当场量得到自己有多宽。
//! 左栏与预设那一栏例外，仍走终端库自己的 [`Wrap`](ratatui::widgets::Wrap)
//! （理由见 `crate::wrap` 的模块文档）。

mod config;
mod footer;
mod overview;
mod picker;
mod report;

#[cfg(test)]
mod probe;

pub(super) use report::opens_at;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::widgets::{Paragraph, ScrollbarOrientation, ScrollbarState};

use super::live::Live;
use super::state::Session;
use super::viewport::Viewport;
use config::config;
use footer::footer;
use overview::{OVERVIEW_HEIGHT, overview};
use picker::presets;
use report::report_pane;

/// 左栏的宽度。配置一直在场，改一下就能在右边看到影响。
///
/// 固定列数而不是按比例：这一栏装的是**标签加取值**，两边都不随终端变宽而变长，
/// 按比例分只会在宽终端上留下一栏空白。
const CONFIG_WIDTH: u16 = 52;

/// 主区无论如何要留下的列数。
///
/// **终端窄到放不下两栏时，让的是左栏。** 报告区挤到十几列就一个字都读不出来，
/// 而左栏那几行本来就折着行（见 [`config::config`]），窄一点仍看得懂。
///
/// 这不是「左栏收起」——那是[展开](crate::session::state::Mode::Expanded)带着的一件事，用户按得动、
/// 也按得回来（`e`／`Esc`）；这里是放不下时的退化，没有开关。
const MAIN_MIN_WIDTH: u16 = 30;

/// 屏底那几行：编辑条、补全候选、要说的那句话。**下限，不是定数**——
/// 折出来的行摆不下时这一格往下长（见 [`footer_height`]）。
const FOOTER_HEIGHT: u16 = 3;

/// 主区无论如何要留下的行数：总览块最高 [`OVERVIEW_HEIGHT`] 行，报告区至少一行加上下两条边。
///
/// 与 [`MAIN_MIN_WIDTH`] 同一条，只是换了个方向：屏底那一格长起来时也不许把主区挤没。
/// 让的次序也同一条：屏矮下来时**先让报告区，总览块不砍**——它是钉住的那一块，
/// 也是唯一答得出「这一趟怎么样」的地方（spec 的《窄终端》）。
const MAIN_MIN_HEIGHT: u16 = OVERVIEW_HEIGHT + 3;

/// 左栏在这一屏上占多宽。装得下就是 [`CONFIG_WIDTH`]，装不下就让给主区。
///
/// **展开着的时候是零**：那一刻左栏整个收起，主区吃满宽度
/// （spec 的《会话：布局与交互》，逐页那两行轻松过 100 列）。
fn config_width(total: u16, expanded: bool) -> u16 {
    if expanded {
        return 0;
    }
    CONFIG_WIDTH.min(total.saturating_sub(MAIN_MIN_WIDTH))
}

/// 把一屏画出来。
///
/// 收的是 `&mut Session`：报告区展开之后要把滚动量收进这一格真滚得动的范围，
/// 而只有画的时候才知道这一格有几行几列（见 [`Session::clamp_report`]）。
/// 那是这一层**唯一**改状态的地方——认键那一路仍旧一步不经过它。
pub fn shell(frame: &mut Frame, session: &mut Session, live: Option<&Live>) {
    let screen = frame.area();
    // 屏底那一格先摆出来：它有几行由折行说了算，而上面那一块吃剩下的（见 [`footer_height`]）。
    let bottom_rows = footer(session, live, screen.width);
    let [body, bottom] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(footer_height(bottom_rows.len(), screen.height)),
    ])
    .areas(screen);
    let expanded = session.expansion().is_some();
    let [left, main] = Layout::horizontal([
        Constraint::Length(config_width(body.width, expanded)),
        Constraint::Min(0),
    ])
    .areas(body);

    // 收起的左栏一格都不画。给它一个零宽的格子也画不出东西来，
    // 但那样读代码的人得自己去推——「收起」这件事该在这一层看得见。
    if !expanded {
        config(frame, left, session);
    }
    // 预设那一栏**占的是主区，左栏照旧在场**：存出去的就是左栏上那两层，
    // 而「存的是什么」在按下去之前得看得见。它与展开那一副因此正好相反——
    // 那一副要的是宽度（逐页那两行过 100 列），这一副要的是**对照**。
    match session.picking() {
        Some(picker) => presets(frame, main, picker),
        None => main_pane(frame, main, session, live),
    }
    frame.render_widget(Paragraph::new(bottom_rows), bottom);
}

/// 屏底那一格有多高：**折出来几行就几行**，下限 [`FOOTER_HEIGHT`]，
/// 上限是主区留得下 [`MAIN_MIN_HEIGHT`]。
///
/// 宽终端上一格不动：那里折不出第四行来，这个数恒是 [`FOOTER_HEIGHT`]。
/// **代价只落在窄终端上**，而那正是这一格摆不下的时候（停车场 Q75 权衡的
/// 「折行还是加一行」，答的是两者都要——折在先，加行只在折完仍摆不下时才发生）。
fn footer_height(rows: usize, total: u16) -> u16 {
    let rows = u16::try_from(rows).unwrap_or(u16::MAX);
    rows.clamp(
        FOOTER_HEIGHT,
        total.saturating_sub(MAIN_MIN_HEIGHT).max(FOOTER_HEIGHT),
    )
}

/// 一格正文，连同它那条滚动条。**画滚动条的地方只有这一处**（哪几块用得上它，
/// 见 [`Viewport`] 那张表）。
///
/// 滚动量与滚动条那三个数都从 [`Viewport`] 来，这里只把它们交给终端库：
/// **滚动条走终端库自带的那个 widget，不自己画一条**；**没有可滚的东西时不画**
/// （[`Viewport::scrollbar`] 那时给 `None`）。
///
/// 它落在这一格**右边那条框线上**（上下各让一格给两个角），因此一列正文都不吃：
/// 有没有它，格子里的字一模一样。
fn scrolling(frame: &mut Frame, area: Rect, body: Paragraph<'static>, view: &Viewport) {
    frame.render_widget(body.scroll((view.from(), 0)), area);
    let Some(bar) = view.scrollbar() else {
        return;
    };
    frame.render_stateful_widget(
        // 全名摆在这儿：上一行那个 `bar` 也叫 `Scrollbar`（[`Viewport`] 出的那三个数），
        // 而这一个是**终端库的 widget**。两者在这一行上碰头，写全就不必猜是哪一个。
        ratatui::widgets::Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut ScrollbarState::new(bar.rows)
            .position(bar.at)
            .viewport_content_length(bar.window),
    );
}

/// 右边那一大格，自上而下两块：**总览块**与**报告区**。
///
/// 两块各占一格而不是挤成一段文字：上面那一块是**此刻**的事（步数一直在动），
/// 下面那一块是**攒下来**的事（只增不改）。挤在一起的话，报告长起来之后进度条就被顶出屏外，
/// 而那正是长任务里唯一有人真想看的东西。
///
/// **总览块因此是钉住的**：报告在它自己那一格里滚（见 [`scrolling`] 与
/// [`report::report_pane`]），一行都推不动上面这一块。`p1-session/09` 那条
/// 「三段各占一格」由这一条接住——三段合成两块，钉住这件事一格没让。
///
/// 上面那一块占几行**由它自己说了算**（[`Overview::height`](overview::Overview::height)）：
/// 出事行不在场时它是五行，让出来的那一行归报告区。算与画因此走同一份东西，不许各算各的。
pub fn main_pane(frame: &mut Frame, area: Rect, session: &mut Session, live: Option<&Live>) {
    let top = overview(live, session.stopping(), session.deciding());
    let [pinned, report] =
        Layout::vertical([Constraint::Length(top.height()), Constraint::Min(0)]).areas(area);

    frame.render_widget(top.draw(), pinned);
    frame.render_widget(report_pane(session, live, report), report);
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::probe::{a_run_in_flight, main_snapshot, same_screen, screen, snapshot, tight};
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::Layer;
    use tonefit::Mode as RunMode;

    /// 左栏按三块显示，各项都在屏上；主区两块都在。
    #[test]
    fn the_shell_draws_three_layers_and_the_two_blocks_of_the_main_pane() {
        let mut session = Session::new();

        let screen = tight(&screen(&mut session, None, 120, 40));

        for layer in [Layer::Device, Layer::Taste, Layer::Scope] {
            assert!(
                screen.contains(&tight(layer.title())),
                "{layer:?} 那一块没画出来"
            );
        }
        for field in session.rows() {
            assert!(
                screen.contains(&tight(field.label())),
                "{field:?} 那一行没画出来：{screen}"
            );
        }
        for block in ["总览", "报告"] {
            assert!(screen.contains(block), "主区少了「{block}」那一块");
        }
        // 一趟都没跑过时，主区说的是「按哪个键跑起来」。
        assert!(screen.contains("t试算"), "{screen}");
        assert!(screen.contains("x执行"), "{screen}");
    }

    /// **停在决策点上等人拿主意时屏上是什么样**（`p1-session/14`，ADR 0012）。
    ///
    /// 三处一次问齐，各是票面上的一条：
    ///
    /// - **总览块那一格的抬头说得出「它为什么不动」**——横条这时一步都不走，
    ///   而眼睛盯着横条的人不会往下扫一行（与按停那一级挂在同一处，停车场 Q71）。
    /// - **报告区把那一卷画出来了**（停车场 Q52）：判定与逐页那几个数就是拿主意的依据，
    ///   而那一卷此刻还没收摊，报告里没有它。
    /// - **屏底摆着答话那三个键**，各带着它买的东西：`x` 是第一遍不重算，
    ///   `a` 是往下不再问，`s` 是等价 dry-run**外加剩下的卷也不开工**。
    ///   跑着时那一副（`s` 停、两级）在这里一个字都不该剩下。
    #[test]
    fn waiting_at_the_decision_point_shows_the_report_and_the_three_ways_out() {
        let summarized = fixture::processed_volume("卷一", None);
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.pass_started(tonefit::Pass::Second, Some(&summarized));
        live.rewind(Duration::from_secs(300));

        let mut session = Session::new();
        session.run_started();
        // 跑着的时候屏底摆的是按停那一副，而续做那一趟先预告它会停下来。
        let running = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(running.contains(&tight("s 停（按一次收尾")), "{running}");
        assert!(
            running.contains(&tight("每一卷第一遍走完都会停下来等你拿主意")),
            "{running}"
        );

        session.at_the_decision_point(true);
        let waiting = tight(&screen(&mut session, Some(&live), 120, 40));

        // 一、抬头。「还剩多久」这时让位给它：横条一动不动，报一个数出来说的就成了
        // 「用户还要想多久」（见 [`super::overview::overview`]）。
        assert!(
            waiting.contains(&tight("第 1/1 卷 · 等你拿主意")),
            "{waiting}"
        );
        assert!(!waiting.contains(&tight("还剩")), "{waiting}");
        // 二、那一卷画出来了：它的去处、卷级基准档与定档页都在屏上——
        // 拿主意要看的正是这几个数。整段逐字比不了，那一段在这一格里会折行
        // （报告区默认那一副折行，见 [`super::report::report_pane`]）。
        for said in ["库/卷一→出/卷一", "卷级基准档", "定档页"] {
            assert!(waiting.contains(&tight(said)), "{said} 没画出来：{waiting}");
        }
        // 抬头那一行说清这一趟此刻只算不写——盘上一个字节都没有。
        assert!(waiting.contains(&tight("dry-run")), "{waiting}");
        // 三、答话那三个键，连同它们各自买的东西。
        for key in [
            "x 接着做第二遍",
            "第一遍不重算",
            "a 剩下的卷都这样",
            "往下不再问",
            "s 收尾",
            "等价 dry-run",
            "剩下的卷也不开工",
            "Ctrl-C 退出会话",
        ] {
            assert!(waiting.contains(&tight(key)), "{key}：{waiting}");
        }
        // 跑着那一副在这里一个字都不剩：两级停不是这一刻的事。
        assert!(
            !waiting.contains(&tight("按一次收尾，再按一次中止")),
            "{waiting}"
        );

        // 对照：那一份没交进来的时候，逐卷那几行一个都不在——那一卷还没收摊，
        // 报告里因此没有它（停车场 Q52 记的正是这一处缺口）。
        let mut running_only = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        running_only.run_started(1, 1000);
        running_only.volume_started(Path::new("库/卷一"), 1000);
        running_only.pass_started(tonefit::Pass::Second, None);
        let before = tight(&screen(&mut session, Some(&running_only), 120, 40));
        assert!(!before.contains(&tight("定档页")), "{before}");
    }

    /// **快照：一趟跑到一半，没有失败页。**
    ///
    /// 钉住的是 `p1-session/09` 那九条验收里画得出来的那几条：卷数与剩余时间说得出来、
    /// 当前卷那一行说得出在走哪一遍、一卷跑完当场显示它的判定、幂等命中说清是哪四项依据没变、
    /// 这一趟怎么读的在卷级行里看得见。总览块自己那几张快照在 [`overview`]。
    #[test]
    fn the_main_pane_without_a_failed_page() {
        let snapshot = main_snapshot(&a_run_in_flight(false), 96, 30);

        same_screen(&snapshot, WITHOUT_A_FAILED_PAGE);
    }

    /// 见 [`the_main_pane_without_a_failed_page`]。
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
"│库/卷一 → 出/卷一（180 页）                                                                   │"
"│  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│库/卷二 → 出/卷二（1 页）                                                                     │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                     │"
"│  卷级 基准档 4bit · 其余 1 页 · 特例 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 · 特│"
"│  例判据 p75 立脚点、3.0× 阈值，四者均未标定）                                                │"
"│    定档页 库/卷二/001.jpg                                                                    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│  缓存 1 页 1.0 MiB（压缩前 4.0 MiB），未溢写（预算 512.0 MiB）                               │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：同一趟，其中一卷有失败页。**
    ///
    /// 「失败页出现的当场就在主区可见，带原因」——那一段与整卷跑完之后的隔离行
    /// 并排出现，两者说的是同一份原因（一份是增量，一份是结果）。
    #[test]
    fn the_main_pane_with_a_failed_page() {
        let snapshot = main_snapshot(&a_run_in_flight(true), 96, 36);

        same_screen(&snapshot, WITH_A_FAILED_PAGE);
    }

    /// 见 [`the_main_pane_with_a_failed_page`]。
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
"│库/卷一 → 出/卷一（180 页）                                                                   │"
"│  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│库/卷二 → 出/隔离/卷二（2 页）                                                                │"
"│  隔离 1 页失败：本卷整卷写到隔离目录 出/隔离/卷二，失败页以卷内统一尺寸留白占位，页序不断    │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                     │"
"│  卷级 基准档 4bit · 其余 1 页 · 特例 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 · 特│"
"│  例判据 p75 立脚点、3.0× 阈值，四者均未标定）                                                │"
"│    定档页 库/卷二/001.jpg                                                                    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│  缓存 1 页 1.0 MiB（压缩前 4.0 MiB），未溢写（预算 512.0 MiB）                               │"
"│失败页（出现的当场，逐页那几行在整卷跑完后才有）                                              │"
"│  库/卷二/017.jpg                                                                             │"
"│    失败 解不出完整尺寸：JPEG 数据截断                                                        │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：终端窄到放不下两栏时的退化，以及屏底那一行折得开。**
    ///
    /// 让的是左栏（见 [`MAIN_MIN_WIDTH`]）：主区仍留得下 [`MAIN_MIN_WIDTH`] 列，
    /// 两块一块不少。再窄到连主区都放不下时**不恐慌**——画得难看是一回事，崩掉是另一回事。
    ///
    /// **屏底那一行在这一档上折成两行，`q 退出` 因此仍在屏上**（`p2-loose-ends/07` 的目的）。
    /// 从前这一行是从行尾切掉的，每多一个键尾巴上那个键就少露一截，而尾巴上摆的正是退出
    /// （停车场 Q75）。这一档上屏底那一格仍是 [`FOOTER_HEIGHT`] 行——折出来的两行加上
    /// 说明那一行正好摆得下，主区一行都没让（见 [`footer_height`]）。
    ///
    /// **左栏这一档上装不下，那条滚动条因此画在它右边那条框线上**（见 [`scrolling`]）：
    /// 十八行的屏留给左栏十三行，而三层一共二十一行。`▲`／`▼` 两头加中间那一截滑块
    /// 说的就是「上面还有、下面还有」——从前这一栏一点滚动都没有，掉出去的那几行
    /// 屏上一个字都不提。
    #[test]
    fn a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane() {
        let mut session = Session::new();
        let live = a_run_in_flight(false);

        assert_eq!(config_width(120, false), CONFIG_WIDTH, "宽终端上左栏不缩");
        assert_eq!(config_width(60, false), 30, "窄终端上左栏让出去");
        assert_eq!(config_width(20, false), 0, "再窄就整个让掉");
        // 展开着时不看屏有多宽：左栏整个收起，主区吃满（票面第三条）。
        assert_eq!(config_width(120, true), 0, "展开着左栏该收起");

        // 快照钉的是**整屏**：左栏让到 30 列、主区拿到 34 列，两块一块不少，
        // 而报告区与屏底那几行都按显示宽度折了行。
        let narrow = snapshot(|frame| shell(frame, &mut session, Some(&live)), 64, 18);
        same_screen(&narrow, TOO_NARROW_FOR_TWO_COLUMNS);
        // 快照自己已经钉住了，但这一条是整张票的目的，写出来才不会在下一次重录时被顺手改掉。
        assert!(narrow.contains("q 退出"), "退出那个键掉出屏外了：{narrow}");

        // **窄到 16 列它都还在**：屏底那一格跟着折出来的行数长（见 [`footer_height`]），
        // 屏够高就一行都不掉。去掉空白再比——窄到一定程度那两个字会分在两行上，
        // 而问的是「它在不在屏上」（停车场 Q60 记着逐格读回来的文字为什么要这么比）。
        for width in [16, 20, 24, 32, 40, 48, 80] {
            let screen = tight(&screen(&mut session, Some(&live), width, 24));
            assert!(
                screen.contains("q退出"),
                "{width} 列上退出那个键掉出屏外了：{screen}"
            );
        }

        // 比左栏还窄、且高度只够画个边框：一屏都摆不下，照样不恐慌。
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, Some(&live)), 20, 6),
            TOO_NARROW_FOR_ANYTHING,
        );
        snapshot(|frame| shell(frame, &mut session, None), 1, 1);
    }

    /// 见 [`a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`]。
    const TOO_NARROW_FOR_TWO_COLUMNS: &str = r#"
"┌配置────────────────────────────┐┌执行 · 第 3/3 卷 · 还剩约 3m┐"
"│设备层 ·                        ▲│ 总体 [==================>  │"
"│判定的依据，绑面板，改一次管很久█│ 本卷 卷三 · 第二遍 [=======│"
"│  型号                          █│ 完成 1 卷 · 跳过 1 卷      │"
"│未挑（跑起来之前必填）          █└────────────────────────────┘"
"│  感知可分辨级数                █┌报告────────────────────────┐"
"│默认（跟随面板）                ║│  立脚点、3.0× 阈值，四者均 │"
"│  阈值                          ║│  未标定）                  │"
"│跟着型号走（先挑一个）          ║│    定档页 库/卷二/001.jpg  │"
"│                                ║│  介质 无寻道惩罚（固态盘） │"
"│口味层 · 这一趟的立场           ║│  · 读取并发 8              │"
"│  适配方式　　　　默认（height）║│  缓存 1 页 1.0 MiB（压缩前 │"
"│  裁边　　　　　　默认（裁）    ║│  4.0 MiB），未溢写（预算   │"
"│  跨页拆分　　　　默认（拆）    ▼│  512.0 MiB）               │"
"└────────────────────────────────┘└────────────────────────────┘"
" ←→ 换一个 · ⏎ 摊开 · c 出标定图 · ↑↓ 选 · t 试算 · x 执行 · e  "
" 展开 · p 预设 · q 退出                                         "
"                                                                "
"#;

    /// **快照：窄到一屏都摆不下的那一档。**见
    /// [`a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`]。
    ///
    /// **摆不下的是「高」，不是「宽」。** 同样 20 列、屏高 24 行时 `q 退出` 照旧在屏上
    /// （上一条那个循环问的就是它）：屏底那一格跟着折出来的行数长。这里屏只有 6 行，
    /// [`footer_height`] 的上限压着它——主区已经没得让（`total - MAIN_MIN_HEIGHT` 是零），
    /// 这一格就停在 [`FOOTER_HEIGHT`] 上，按键那一行折出来的六行只露得出头三行。
    ///
    /// **这一档钉的是「不恐慌、不错位」，不是「读得下去」**：6 行的屏上没有一副画法读得下去。
    /// 折下来的那两行带着行首那一格缩进（[`crate::wrap`]：缩进跟着折下来的每一行走）。
    const TOO_NARROW_FOR_ANYTHING: &str = r#"
"┌执行 · 第 3/3 卷 ·┐"
"│ 总体 [===========│"
"└──────────────────┘"
" ←→ 换一个 · ⏎ 摊开 "
" · c 出标定图 · ↑↓  "
" 选 · t 试算 · x 执 "
"#;
}
