//! 会话的画法：左窄配置常驻 + 右宽主区（spec 的《会话：布局与交互》）。
//!
//! 主区自上而下两块：**总览块**（这一趟是什么 · 走到哪儿 · 怎么样）与**报告区**
//! （边跑边攒）。报告区有**三副**样子——默认一个目录一行，**展开一枝**摊出它底下那几卷，
//! **展开一卷**时逐页那几行摊开、左栏收起、主区吃满宽度
//! （见 [`report::report_pane`] 与 [`shell`]）。
//!
//! # 屏上那几块各住在哪儿
//!
//! **本模块只把屏分成格子**，而**摆不下时谁让位**在 [`room`]——那一份次序跨着屏上
//! 那几块，一块自己答不出来。每一格里画什么在各自的模块里，**一块一个**：
//!
//! | 屏上那一块 | 住在 |
//! |---|---|
//! | 摆不下时谁让位 | [`room`] |
//! | 左栏：三层配置 | [`config`] |
//! | 预设栏 | [`picker`] |
//! | 主区上面那一块：总览块 | [`overview`] |
//! | 主区下面那一块：报告区 | [`report`] |
//! | 报告区默认那张**目录表** | [`directories`] |
//! | 报告区**展开一枝**摊出来的那张**卷表** | [`table`] |
//! | 报告区**展开一卷**摊出来的那张**逐页表** | [`pages`] |
//! | 屏底那几行 | [`footer`] |
//!
//! 分法按**屏上那几块**走，不按「工具函数 vs 业务」——后者一年后没人分得清一个函数
//! 该归哪一边。整层连同这几个模块都在 `tui` 后面（分界见 `super` 的《终端库在哪一半》）。
//! 那几块共用的测试探针在 `probe`（`#[cfg(test)]`，不进非 test 那一趟的文档）。
//!
//! **这张表上只有屏上那几块，而颜色不是一块**：四种语义色与 `NO_COLOR` 在 [`paint`]，
//! 上面那几块各按**语义**要色（「注意」「出事」「不要紧」），一处画法都不自己挑颜色。
//!
//! # 摆不下的时候
//!
//! **谁让位、按什么次序，只有 [`room`] 一处**（宽度先让左栏再砍列，高度总览不砍、
//! 让的是表）：从前它散在布局常量与各块自己的判断里，各让各的。
//! 摆不下的**抬头**同样在那里从中间省略，不交给终端库硬截（[`yielding::title`]）。
//!
//! **从第几行画起是算出来的**，由 [`Viewport`] 那个纯函数答——它摆在终端库外面
//! （分界见 `super` 的《终端库在哪一半》）。**哪几块共用它、哪一块画滚动条，
//! 那张表在 [`Viewport`] 上**，本模块不抄第二份。
//!
//! 横向摆不下时**那两张表按各自的固定次序砍列**，次序同样摆在终端库外面
//! （[`crate::session::columns`]）：一列都不许在画法这一层再排第二次。
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
//! **屏上每一格的折行都走 [`crate::wrap`]**——`--help` 与命令行印出来的报告折的是
//! 同一套，而那两处根本没有终端库。这一层只交代**折到多宽**：那一格当场量得到自己有多宽。
//! 一格里那几行各带一份样式时（左栏与预设那一栏）走 [`folded`]：折完逐行把样式重挂一遍。
//!
//! **终端库自己的 `Wrap` 一处都不用了**（`p4-parking-lot/02` 推翻 `p2-loose-ends/07`
//! 那条「左栏那两栏原样留着」）：它折出来的行数这一层数不出来，而[视口](Viewport)要的正是那个数。

mod config;
mod directories;
mod footer;
mod overlay;
mod overview;
mod pages;
mod paint;
mod picker;
mod report;
mod table;
mod yielding;

#[cfg(test)]
mod probe;

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, ScrollbarOrientation, ScrollbarState};

use super::live::Live;
use super::state::Session;
use super::viewport::Viewport;
use config::config;
use footer::footer;
use overview::overview;
use picker::presets;
use report::report_pane;
use yielding::{Panes, main_split, panes};

/// 把一屏画出来。
///
/// 收的是 `&mut Session`：报告区展开之后要把逐页表那个光标收进这一副真列出来的那几页里，
/// 而只有画的时候才知道这一副此刻列着几页（见 [`Session::clamp_report`]）。
/// 那是这一层**唯一**改状态的地方——认键那一路仍旧一步不经过它。
pub fn shell(frame: &mut Frame, session: &mut Session, live: Option<&Live>) {
    let screen = frame.area();
    // 屏底那一格先摆出来：它有几行由折行说了算，而这一屏怎么切在 [`panes`] 一处答完。
    let bottom_rows = footer(session, live, screen.width);
    let expanded = session.expansion().is_some();
    let Panes {
        bottom,
        body,
        left,
        main,
    } = panes(screen, expanded, bottom_rows.len());
    // **一张覆盖层掀着时，上面那几块整个让位**（`p3-session-legibility/12`）：
    // 它盖的是屏底之外的全部——`?` 那张键位表与这一趟的前提都要一眼扫得完，
    // 而屏上此刻没有第二件要读的事。屏底那几行照旧在场：那一行说的正是怎么关掉它。
    if session.overlay().is_some() {
        overlay::overlay(frame, body, session, live);
        frame.render_widget(Paragraph::new(bottom_rows), bottom);
        return;
    }
    // 左栏不在场时一格都不画。不在场有两种（展开着**收起**，或者屏太窄**让掉**，
    // 见 [`yielding::config_width`]），而屏上的结果是同一个：这一栏没有宽度。
    // 给它一个零宽的格子也画不出东西来，但那样读代码的人得自己去推——
    // 「不在场」这件事该在这一层看得见。
    if left.width > 0 {
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

/// 一格正文，连同它那条滚动条（哪几块用得上视口，见 [`Viewport`] 那张表）。
///
/// 滚动量与滚动条那三个数都从 [`Viewport`] 来，这里只把它们交给终端库：
/// **滚动条走终端库自带的那个 widget，不自己画一条**（见 [`scrollbar`]）；
/// **没有可滚的东西时不画**（[`Viewport::scrollbar`] 那时给 `None`）。
fn scrolling(frame: &mut Frame, area: Rect, body: Paragraph<'static>, view: &Viewport) {
    frame.render_widget(body.scroll((view.from(), 0)), area);
    scrollbar(frame, area, view);
}

/// 一行字，连同**它挂哪一份样式**。
///
/// 打成一个类型而不是一对裸值，理由与 [`Painted`](paint::Painted) 那一条逐字相同
/// （写在它的文档里）：`(String, Style)` 在调用处看不出哪一半是哪一半。
///
/// **与 [`Painted`](paint::Painted) 差的是「一行带的是什么」**：那一份带的是**语义**
/// （`Tone`——这一行讲的这件事怎么样，上不上色由 `paint` 一处定），这一份带的是终端库的
/// `Style`——这一行此刻**是什么状态**：光标停在它上面（反白）、整栏只读（压暗）、
/// 它是一层的抬头（加粗）。**状态不是语义**，两者因此不合成一个。
struct Styled {
    /// 这一行的字。**排版已经摆好了**——折行只按显示宽度断，一个空格都不添。
    text: String,
    /// 这一行挂哪一份样式。折出来的每一行都挂它。
    style: Style,
}

impl Styled {
    /// 一行字加一份样式。
    fn new(text: String, style: Style) -> Self {
        Self { text, style }
    }

    /// 不挂样式的那一种。层与层之间那个空行、下钻进去那一行都是它。
    fn plain(text: String) -> Self {
        Self::new(text, Style::default())
    }
}

/// 一格里那几行**折成这一格摆得下的样子**，并算出光标落在折完的第几行。
///
/// **折完逐行把样式重挂一遍**——折出来的那几行本来就是同一行，样式一格不差地跟着走
/// （折行只认字，见 [`crate::wrap`]）。
///
/// **光标那个数跟着折出来的行走**：交给 [`Viewport`] 的必须与屏上的行一一对应，
/// 否则「装得下就不画滚动条」与「光标还在不在格子里」两条都按错的行数判
/// （停车场 Q136 咬的正是这个）。光标那一行自己折成几行时指的是**头一行**，
/// 与报告区那一处同一条（`report::folded`）——屏上没有第二种「光标在第几行」。
/// `cursor` 越界时给的是第一行；两个调用点都保证它在范围内，而 [`Viewport::new`]
/// 那一头本来也会就近收。
///
/// **要一个空行就摆一个空格**，不是空串：[`crate::wrap::fold`] 给空文字**零行**
/// （屏底那一格靠那一条分得开「没有话要说」与「说了一句空话」），一个空格折出来正是一行。
/// 那是屏上早有的写法，`overlay` 里那几组之间空的那一行就是这么摆的——本函数因此
/// 一个特例都不开，与 [`Painted::folded`](paint::Painted::folded) 一个待遇。
fn folded(rows: Vec<Styled>, cursor: usize, width: u16) -> (Vec<Line<'static>>, usize) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut at = 0;
    for (row, Styled { text, style }) in rows.into_iter().enumerate() {
        if row == cursor {
            at = lines.len();
        }
        lines.extend(
            crate::wrap::fold(&text, width)
                .into_iter()
                .map(|text| Line::styled(text, style)),
        );
    }
    (lines, at)
}

/// 一格右边那条框线上的**滚动条**。**造那个 widget 的地方只有这一处。**
///
/// 分出来是为了[展开那一副](report::report_pane)：它顶上钉着一行抬头，正文因此不占满
/// 这一格，两者不是同一个 `Rect`——而滚动条画在**正文那一段**的右边才对得上
/// （对不上的话，滑块指着的行与屏上那一行差一行）。
///
/// 它落在框线上（上下各让一格给两个角），因此一列正文都不吃：有没有它，格子里的字一模一样。
fn scrollbar(frame: &mut Frame, area: Rect, view: &Viewport) {
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
/// **屏矮下来时让的是报告区，总览一行不砍**——那一条与切格子的次序一起摆在
/// [`main_split`] 上。
pub fn main_pane(frame: &mut Frame, area: Rect, session: &mut Session, live: Option<&Live>) {
    let top = overview(live, session.stopping(), session.deciding());
    let [pinned, report] = main_split(area, top.height());

    frame.render_widget(top.draw(pinned.width), pinned);
    report_pane(frame, report, session, live);
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::probe::{only_branch, screen, tight};
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
        // 报告区默认那一副是**目录表**（`volume-discovery/08`）：那一卷自己那一行
        // 要展开它那一枝才摊得出来。
        session.open(only_branch(&live).directory);
        let waiting = tight(&screen(&mut session, Some(&live), 120, 40));

        // 一、抬头。「还剩多久」这时让位给它：横条一动不动，报一个数出来说的就成了
        // 「用户还要想多久」（见 [`super::overview::overview`]）。
        assert!(
            waiting.contains(&tight("第 1/1 卷 · 等你拿主意")),
            "{waiting}"
        );
        assert!(!waiting.contains(&tight("还剩")), "{waiting}");
        // 二、那一卷画出来了：**表上给它一行**，记号、档位与定档页都在屏上——
        // 拿主意要看的正是这几个数，而末尾那一句说清它还没收摊
        // （见 [`super::report::the_volume_waiting_at_the_decision_point_gets_a_row_of_its_own`]）。
        for said in ["卷一", "4bit", "001.jpg", "等你拿主意"] {
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
        // 一卷都还没有：表连列头都不出（`super::table::table` 与
        // `super::directories::directories` 同一条）。
        assert!(!before.contains(&tight("记号  卷名")), "{before}");
        assert!(!before.contains(&tight("记号  目录")), "{before}");
        assert!(!before.contains(&tight("001.jpg")), "{before}");
    }
}
