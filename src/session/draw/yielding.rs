//! 屏上那几块**摆不下的时候谁让位**（`CONTEXT.md` 的《会话》：让位；spec 的《窄终端》）。
//!
//! **屏上那几块各占哪一格由本模块切**（[`panes`] 与 [`main_split`]），**让位的次序因此
//! 只有这一份**：从前它散在布局常量、左栏那一算、屏底那一算与各块自己的判断里——
//! 各让各的，而「谁先让」是**跨块**的事，一块自己答不出来。
//!
//! # 宽度不够：先让左栏，再砍列
//!
//! | 第几步 | 让的是 | 谁说了算 |
//! |---|---|---|
//! | 一 | **左栏**：先缩，缩到 [`CONFIG_MIN_WIDTH`] 以下就整个让掉 | [`config_width`] |
//! | 二 | 两张表**各按自己那个固定次序砍列** | [`crate::session::columns`] |
//!
//! 先让左栏：报告区挤到十几列就一个字都读不出来，而左栏那几行本来就折着行
//! （见 [`super::config::config`]），窄一点仍看得懂。
//!
//! # 高度不够：总览不砍，让的是表
//!
//! | 第几步 | 让的是 | 谁说了算 |
//! |---|---|---|
//! | 一 | **屏底**按折出来的行数往下长，长到主区只剩 [`MAIN_MIN_HEIGHT`] 为止 | [`footer_height`] |
//! | 二 | **总览块一行不砍**——它有几行由它自己说了算 | [`main_split`] |
//! | 三 | **报告区吃剩下的**：三者都让完仍摆不下时宁可少画表，不少画总览 | [`main_split`] |
//!
//! 总览块是钉住的那一块，也是唯一答得出「这一趟怎么样」的地方（spec 的《窄终端》）。
//!
//! # 抬头摆不下：从中间省略，不由终端库硬截
//!
//! 见 [`title`]（摆在边框上的那几个）与 [`pinned`]（钉在格子里的那一行）——
//! 截法只有 [`elided`] 一处，两个名字差的只是**两个角占不占格**。
//!
//! # 不在这里的三样
//!
//! 本模块只答**块与块之间**谁让位。这三样都是**一格之内**的事，各归那一块自己：
//!
//! - **屏底那一格里那三样谁先让**（说明那一行 → 要说的那句话 → 按键那几行）在
//!   [`super::footer::footer`]。
//! - **一格切成两截**：报告区展开那一副把自己那一格切成「钉住的抬头 + 正文」，
//!   那一句在 [`super::report::report_pane`] 里。
//! - **砍列的次序**在 [`crate::session::columns`]：它摆在终端库外面，两张表各一份，
//!   本模块只说它排在左栏后面。

use ratatui::layout::{Constraint, Layout, Rect};

use super::overview::OVERVIEW_HEIGHT;
use crate::session::columns;

/// 左栏摆得开时占的列数。配置一直在场，改一下就能在右边看到影响。
///
/// 固定列数而不是按比例：这一栏装的是**标签加取值**，两边都不随终端变宽而变长，
/// 按比例分只会在宽终端上留下一栏空白。
pub(super) const CONFIG_WIDTH: u16 = 52;

/// 左栏**窄到这个数以下就整个让掉**。
///
/// 一行是「两格缩进 + 最宽 16 格的标签 + 取值」（见 [`super::config::row`]），
/// 两条框线再吃掉两格：不到这么宽，取值那一截一格不剩，整栏折成一摞两三个字——
/// 那种窄条比不画更坏，它占着主区的列，却连一行取值都答不出来。
///
/// **让掉不是收起**：收起是[展开](crate::session::state::Focus::Expanded)带着的一件事，
/// 用户按得动也按得回来（`e`／`Esc`）；这里是屏太窄时的退化，没有开关。
/// 两者**屏上的结果是同一个**（这一栏一格都不画），因此 [`config_width`] 两支都给零——
/// 分不分得开是**状态**那一维的事，画法这一层只问「这一格有没有宽度」。
pub(super) const CONFIG_MIN_WIDTH: u16 = 30;

/// 主区无论如何要留下的列数。
///
/// **终端窄到放不下两栏时让的是左栏**（见本模块的《宽度不够》）：这个数是主区的地板，
/// 左栏在它上面把剩下的都让出去。
pub(super) const MAIN_MIN_WIDTH: u16 = 30;

/// 屏底那几行：编辑条、补全候选、要说的那句话。**下限，不是定数**——
/// 折出来的行摆不下时这一格往下长（见 [`footer_height`]）。
pub(super) const FOOTER_HEIGHT: u16 = 3;

/// 主区无论如何要留下的行数：总览块最高 [`OVERVIEW_HEIGHT`] 行，报告区至少一行加上下两条边。
///
/// 与 [`MAIN_MIN_WIDTH`] 同一条，只是换了个方向：屏底那一格长起来时也不许把主区挤没。
/// 按总览块**最高**那一副留：矮下去的那两副让出来的行归报告区，而这个数要在屏底长高之前
/// 就答得出来（那时还没算总览有几行）。
pub(super) const MAIN_MIN_HEIGHT: u16 = OVERVIEW_HEIGHT + 3;

/// 屏上那几块各占的那一格。**切法只有 [`panes`] 一处**，每一格里画什么在各自的模块里。
pub(super) struct Panes {
    /// 屏底那一格（[`super::footer`]）。
    pub(super) bottom: Rect,
    /// 屏底之上那一整块。**一张覆盖层掀着时盖的就是它**（见 [`super::shell`]）。
    pub(super) body: Rect,
    /// 左栏（[`super::config`]）。**让掉了就是零宽**，见 [`config_width`]。
    pub(super) left: Rect,
    /// 主区：总览块与报告区，或者预设栏。
    pub(super) main: Rect,
}

/// 把一屏切成那几格，**让位的次序就在这一处**（见本模块的两张表）。
///
/// 屏底先摆：它有几行由折行说了算（`footer_rows` 是折出来的行数），上面那一块吃剩下的。
/// 左栏再摆：宽度由 [`config_width`] 答，主区吃剩下的。
pub(super) fn panes(screen: Rect, expanded: bool, footer_rows: usize) -> Panes {
    let [body, bottom] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(footer_height(footer_rows, screen.height)),
    ])
    .areas(screen);
    let [left, main] = Layout::horizontal([
        Constraint::Length(config_width(body.width, expanded)),
        Constraint::Min(0),
    ])
    .areas(body);
    Panes {
        bottom,
        body,
        left,
        main,
    }
}

/// 主区自上而下那两格：**总览块钉在上面，报告区吃剩下的**。
///
/// 上面那一块占几行由它自己说了算（[`Overview::height`](super::overview::Overview::height)）
/// ——出事行不在场时它是五行，让出来的那一行归报告区。算与画因此走同一份东西，
/// 不许各算各的。
///
/// **总览块一行不砍**：屏矮到剩下的行数摆不下一张表时，少画的是表
/// （spec 的《窄终端》：宁可少画表，不少画总览）。
pub(super) fn main_split(area: Rect, overview: u16) -> [Rect; 2] {
    Layout::vertical([Constraint::Length(overview), Constraint::Min(0)]).areas(area)
}

/// 左栏在这一屏上占多宽：摆得开就是 [`CONFIG_WIDTH`]，摆不开就一路让给主区，
/// 让到窄过 [`CONFIG_MIN_WIDTH`] 就**整个让掉**。
///
/// **展开着的时候是零**：那一刻左栏整个收起，主区吃满宽度
/// （spec 的《会话：布局与交互》，逐页那两行轻松过 100 列）。
pub(super) fn config_width(total: u16, expanded: bool) -> u16 {
    if expanded {
        return 0;
    }
    let left = CONFIG_WIDTH.min(total.saturating_sub(MAIN_MIN_WIDTH));
    if left < CONFIG_MIN_WIDTH { 0 } else { left }
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

/// 摆在**边框**上的那个抬头，在这么宽的一格上写成什么样。
///
/// 两个角各占一格，能写字的因此是 `width - 2` 格；截法见 [`elided`]。
/// **屏上五个边框抬头都走这一处**：左栏、总览块、报告区、预设栏、覆盖层。
pub(super) fn title(said: &str, width: u16) -> String {
    elided(said, width.saturating_sub(2))
}

/// 钉在**格子里**的那一行抬头（展开那一副顶上那一行），在这么宽的一格上写成什么样。
///
/// 它一格边框都不占，`width` 就是它能写字的格数——与 [`title`] 差的只有这一件事。
pub(super) fn pinned(said: &str, width: u16) -> String {
    elided(said, width)
}

/// **抬头怎么截：摆不下时从中间省略**（[`columns::elide`]），不交给终端库硬截。
///
/// **非有一个记号不可**（停车场 Q147）：总览块的抬头末一截是「还剩约 3m20s」，
/// 硬截出来的 `还剩约 3m` 是一个**读起来完整、而且偏小**的估计，屏上没有一处痕迹说它
/// 被截过——那比截出半个字更坏。从中间省略两头都留得住：这一趟是什么在前，
/// 还剩多久在后，中间那一截换成省略号。
///
/// 一处定完，「截过的抬头一眼看得出来」才不靠人一块一块记着；
/// **省略法与卷名那一列同一副**（`columns::elide`），屏上没有第二种省略号。
fn elided(said: &str, room: u16) -> String {
    columns::elide(said, usize::from(room))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::probe::{
        a_run_in_flight, every_kind_of_volume, only_branch, same_screen, screen, snapshot,
        snapshot_of, tight,
    };
    use super::super::shell;
    use super::*;
    use crate::session::live::{Resuming, Volume};
    use crate::session::state::{Expansion, Field, Key, Overlay, Session};
    use tonefit::Mode as RunMode;

    /// **宽度不够时先让左栏**：先缩，缩不到 [`CONFIG_MIN_WIDTH`] 就整个让掉。
    ///
    /// 让掉那一档是本票立的（`p3-session-legibility/13`）：从前它一路缩到一列，
    /// 40 列的屏上因此留着一条 10 列宽的左栏，行行折成两三个字，主区照旧只有 30 列
    /// ——两边都读不出东西。
    #[test]
    fn width_is_taken_from_the_left_column_first_and_then_all_of_it() {
        assert_eq!(config_width(120, false), CONFIG_WIDTH, "宽终端上左栏不缩");
        // 主区先拿够它那一份，剩下的都归左栏：80 列的屏上左栏比标准的窄两格。
        assert_eq!(config_width(80, false), 50, "80 列上左栏该让出两格");
        assert_eq!(config_width(64, false), 34, "窄终端上左栏接着让");
        // 缩到底那一档：再窄一格就整个让掉，主区吃满。
        assert_eq!(config_width(60, false), CONFIG_MIN_WIDTH, "缩到底那一档");
        assert_eq!(config_width(59, false), 0, "缩不下去就整个让掉");
        assert_eq!(config_width(40, false), 0, "极窄那一档左栏不在");
        assert_eq!(config_width(1, false), 0, "一列的屏也不恐慌");
        // 展开着时不看屏有多宽：左栏整个收起，主区吃满。
        assert_eq!(config_width(120, true), 0, "展开着左栏该收起");
    }

    /// **高度不够时屏底往下长，长到主区只剩 [`MAIN_MIN_HEIGHT`] 为止。**
    #[test]
    fn the_footer_grows_until_the_main_pane_is_down_to_its_floor() {
        // 宽终端上折不出第四行来，这一格恒是下限。
        assert_eq!(footer_height(1, 40), FOOTER_HEIGHT);
        assert_eq!(footer_height(3, 40), FOOTER_HEIGHT);
        // 折出六行就长到六行——40 行的屏上主区还剩得下。
        assert_eq!(footer_height(6, 40), 6);
        // 屏矮下来时上限压着它：主区那一份一行都不让。
        assert_eq!(footer_height(6, MAIN_MIN_HEIGHT + 4), 4);
        // 主区已经没得让了，这一格就停在下限上——那时裁的是屏底自己的底下几行。
        assert_eq!(footer_height(6, MAIN_MIN_HEIGHT), FOOTER_HEIGHT);
        assert_eq!(footer_height(6, 0), FOOTER_HEIGHT);
    }

    /// **抬头摆不下时从中间省略**，两头都留得住（停车场 Q147）。
    #[test]
    fn a_title_that_does_not_fit_is_elided_in_the_middle() {
        let said = "执行 · 第 3/3 卷 · 还剩约 3m20s";

        // 摆得下就一个字不动。
        assert_eq!(title(said, 60), said);
        // 摆不下：头上那一截与末尾那个数都还在，中间换成省略号——
        // 硬截出来的 `还剩约 3m` 是一个读起来完整而偏小的估计，这一副读得出它被截过。
        let cut = title(said, 30);
        assert!(cut.starts_with("执行"), "{cut}");
        assert!(cut.ends_with("3m20s"), "{cut}");
        assert!(cut.contains('⋯'), "截过了却没有记号：{cut}");
        assert!(crate::wrap::width(&cut) <= 28, "{cut}");
        // 一格都没有的格子上不恐慌。
        assert_eq!(title(said, 0), "");
        assert_eq!(title(said, 2), "");
    }

    /// 取值栏摊着的那一份会话：适配方式那一行上按一下 `⏎`，取值就地摊在它下面。
    fn a_session_with_values_unfolded() -> Session {
        let mut session = Session::new();
        session.go_to(Field::Fit);
        session.press(Key::Enter);
        session
    }

    /// **80×24 上整屏读得下去**（票面第一条）：总览那几行在、表至少一行、
    /// 屏底的退出键在屏上。
    ///
    /// 三张快照钉的是同一档屏上的三副样子。**左栏在这一档上比标准的窄两格**
    /// （50 列，见 [`config_width`]）：主区先拿够它那 [`MAIN_MIN_WIDTH`] 列。
    #[test]
    fn the_whole_screen_reads_at_eighty_by_twenty_four() {
        let mut session = Session::new();
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, None), 80, 24),
            AT_EIGHTY_BY_TWENTY_FOUR_IDLE,
        );

        let live = a_run_in_flight(true);
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, Some(&live)), 80, 24),
            AT_EIGHTY_BY_TWENTY_FOUR_RUNNING,
        );

        let mut valuing = a_session_with_values_unfolded();
        same_screen(
            &snapshot(|frame| shell(frame, &mut valuing, None), 80, 24),
            AT_EIGHTY_BY_TWENTY_FOUR_VALUING,
        );
    }

    /// **展开那一副在 80×24 上**（票面第四条）：左栏收起，逐页表吃满 80 列。
    #[test]
    fn the_expanded_volume_reads_at_eighty_by_twenty_four() {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        let mut session = Session::new();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(1)));

        same_screen(
            &snapshot_of(&mut session, &live, 80, 24),
            AT_EIGHTY_BY_TWENTY_FOUR_EXPANDED,
        );
    }

    /// **极窄那一档**（票面第三条）：40 列上左栏整个让掉，主区吃满，一格乱码都没有。
    #[test]
    fn a_terminal_forty_columns_wide_gives_everything_to_the_main_pane() {
        let live = a_run_in_flight(true);
        let mut session = Session::new();

        same_screen(
            &snapshot(|frame| shell(frame, &mut session, Some(&live)), 40, 24),
            FORTY_COLUMNS,
        );
    }

    /// **极窄与极矮都不崩、不无限折行**（票面第三条）。
    ///
    /// 一格一格问过去：屏底那个退出键在不在、这一屏画不画得完。
    /// 上面那几张快照钉的是长什么样，这一条钉的是**每一档都画得出来**。
    #[test]
    fn neither_a_narrow_nor_a_short_terminal_falls_over() {
        let live = a_run_in_flight(true);
        let mut session = Session::new();

        for width in [1, 4, 10, 16, 20, 24, 30, 31, 40, 48, 59, 60, 64, 80] {
            for height in [1, 2, 3, 6, 10, 24] {
                snapshot(
                    |frame| shell(frame, &mut session, Some(&live)),
                    width,
                    height,
                );
            }
        }
        // 退出那个键在屏上：屏底那一格跟着折出来的行数长（见 [`footer_height`]），
        // 屏够高就一行都不掉。去掉空白再比——窄到一定程度那两个字会分在两行上
        // （停车场 Q60 记着逐格读回来的文字为什么要这么比）。
        for width in [16, 20, 24, 32, 40, 48, 80] {
            let screen = tight(&screen(&mut session, Some(&live), width, 24));
            assert!(
                screen.contains("q退出"),
                "{width} 列上退出那个键掉出屏外了：{screen}"
            );
        }
    }

    /// **窄档上另外那三块各过一遍**（票面第四条）：预设栏、两张覆盖层。
    ///
    /// 取值栏与展开那一副各有自己的快照（见上面那两条）；这三块问的是
    /// 「窄下来之后它要说的那句话还在不在」，快照钉不出这一点——覆盖层那两张
    /// 在这一档上滚得动，屏上露出来的是哪一段随视口走。
    #[test]
    fn the_picker_and_the_two_overlays_still_say_their_piece_when_narrow() {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);

        // 预设栏：占的是主区，左栏照旧在场（对照着看存的是什么）。
        let mut picking = Session::new();
        picking.pick(
            vec!["漫画".to_owned(), "画集".to_owned()],
            std::path::PathBuf::from("C:/配置/tonefit/presets.toml"),
        );
        let shown = tight(&screen(&mut picking, None, 80, 24));
        assert!(shown.contains("漫画"), "{shown}");
        assert!(shown.contains("预设"), "{shown}");
        assert!(shown.contains("配置"), "预设栏开着左栏该在场：{shown}");

        // `?` 那张键位表：掀着的时候盖住屏底之外的全部，而那一行说的正是怎么关掉它。
        let mut keys = Session::new();
        keys.press(Key::Char('?'));
        assert_eq!(keys.overlay().map(|c| c.overlay), Some(Overlay::Keys));
        let shown = tight(&screen(&mut keys, Some(&live), 80, 24));
        assert!(shown.contains("全部键"), "{shown}");
        assert!(shown.contains("Esc"), "{shown}");
        assert!(
            !shown.contains("判定的依据"),
            "覆盖层掀着左栏该整个让位：{shown}"
        );

        // `i` 这一趟的前提：同一副形状，另一份内容。
        let mut premises = Session::new();
        premises.run_started();
        premises.press(Key::Char('i'));
        assert_eq!(
            premises.overlay().map(|c| c.overlay),
            Some(Overlay::Premises)
        );
        let shown = tight(&screen(&mut premises, Some(&live), 80, 24));
        assert!(shown.contains("前提"), "{shown}");
        assert!(shown.contains("适配方式"), "{shown}");
    }

    /// **屏矮下来时让的是表，总览一行不砍**（票面第二条的高度那一半）。
    ///
    /// 24 行的屏上表还剩十来行；10 行的屏上总览把主区吃满，报告区只剩一条框线——
    /// 那正是「宁可少画表，不少画总览」。
    #[test]
    fn a_short_screen_gives_up_the_table_before_the_overview() {
        let live = a_run_in_flight(false);
        let mut session = Session::new();
        // 卷一那一行在**展开一枝**之后那一副上（`volume-discovery/08`）：
        // 默认那一副是目录表，一枝一行。
        session.open(only_branch(&live).directory);

        let tall = tight(&screen(&mut session, Some(&live), 80, 24));
        assert!(tall.contains("总体"), "{tall}");
        assert!(tall.contains("卷一"), "24 行的屏上表该在：{tall}");

        let short = tight(&screen(&mut session, Some(&live), 80, 10));
        // 总览那几行一行不少。
        for said in ["总体", "本卷", "完成"] {
            assert!(short.contains(said), "{said} 被砍了：{short}");
        }
        // 表让掉了：这一档上报告区连一行正文都摆不下。
        assert!(!short.contains("卷一"), "该少画表的：{short}");
    }

    /// **快照：终端窄到放不下两栏时的退化，以及屏底那一行折得开。**
    ///
    /// 让的是左栏（见 [`config_width`]）：主区仍留得下 [`MAIN_MIN_WIDTH`] 列，
    /// 两块一块不少。再窄到连主区都放不下时**不恐慌**——画得难看是一回事，崩掉是另一回事。
    ///
    /// **屏底那一行在这一档上一行摆得下了**（`p3-session-legibility/12` 的瘦身）：
    /// 五个键 64 列上不折行。从前它是十个键、折成两行——折行那一套照旧在
    /// （`p2-loose-ends/07` 的目的，见下面那个循环与 [`TOO_NARROW_FOR_ANYTHING`]），
    /// 只是这一档上用不着了。`q 退出` 一行不让那一条因此仍旧成立
    /// （从前它是从行尾切掉的，每多一个键尾巴上那个键就少露一截，停车场 Q75）。
    ///
    /// **左栏这一档上装不下，那条滚动条因此画在它右边那条框线上**（见 [`super::super::scrolling`]）：
    /// 十八行的屏留给左栏十三行，而三层一共二十一行。`▲`／`▼` 两头加中间那一截滑块
    /// 说的就是「上面还有、下面还有」——从前这一栏一点滚动都没有，掉出去的那几行
    /// 屏上一个字都不提。
    #[test]
    fn a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane() {
        let mut session = Session::new();
        let live = a_run_in_flight(false);

        // 快照钉的是**整屏**：左栏让到 34 列、主区拿到 30 列，两块一块不少，
        // 而**屏上每一格都按显示宽度折行**——左栏那一格是 `p4-parking-lot/02` 搬过来的
        // （见 [`super::super::folded`]），折下来的那一截带着行首缩进。
        let narrow = snapshot(|frame| shell(frame, &mut session, Some(&live)), 64, 18);
        same_screen(&narrow, TOO_NARROW_FOR_TWO_COLUMNS);
        // 快照自己已经钉住了，但这一条是整张票的目的，写出来才不会在下一次重录时被顺手改掉。
        assert!(narrow.contains("q 退出"), "退出那个键掉出屏外了：{narrow}");

        // 比左栏还窄、且高度只够画个边框：一屏都摆不下，照样不恐慌。
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, Some(&live)), 20, 6),
            TOO_NARROW_FOR_ANYTHING,
        );
    }

    /// 见 [`a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`]。
    const TOO_NARROW_FOR_TWO_COLUMNS: &str = r#"
"┌配置────────────────────────────┐┌执行 · 第 3/3 ⋯ 还剩约 3m20s┐"
"│设备层 · 判定的依据，绑面板，改 ▲│ 总体 [==================>  │"
"│一次管很久                      █│ 本卷 卷三 · 第二遍 [=======│"
"│  型号　　　　　　未挑（跑起来之█│ 完成 1 卷 · 跳过 1 卷      │"
"│  前必填）                      █└────────────────────────────┘"
"│  感知可分辨级数　默认（跟随面  ║┌报告────────────────────────┐"
"│  板）                          ║│ 记号  目录  卷数           │"
"│  阈值　　　　　　跟着型号走（先║│ ✓     库       2           │"
"│  挑一个）                      ║│                            │"
"│                                ║│                            │"
"│口味层 · 这一趟的立场           ║│                            │"
"│  适配方式　　　　默认（height）║│                            │"
"│  裁边　　　　　　默认（裁）    ║│                            │"
"│  跨页拆分　　　　默认（拆）    ▼│                            │"
"└────────────────────────────────┘└────────────────────────────┘"
" ⏎ 摊开取值 · t 试算 · x 执行 · q 退出 · ? 全部键               "
"                                                                "
"                                                                "
"#;

    /// **快照：窄到一屏都摆不下的那一档。**见
    /// [`a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`]。
    ///
    /// **摆不下的是「高」，不是「宽」。** 同样 20 列、屏高 24 行时 `q 退出` 照旧在屏上
    /// （上一条那个循环问的就是它）：屏底那一格跟着折出来的行数长。这里屏只有 6 行，
    /// [`footer_height`] 的上限压着它——主区已经没得让（`total - MAIN_MIN_HEIGHT` 是零），
    /// 这一格就停在 [`FOOTER_HEIGHT`] 上。20 列上那一行折出来正好三行，
    /// 而三行正是这一格的下限——瘦身之前它折出六行，只露得出头三行。
    ///
    /// **这一档钉的是「不恐慌、不错位」，不是「读得下去」**：6 行的屏上没有一副画法读得下去。
    /// 折下来的那两行带着行首那一格缩进（[`crate::wrap`]：缩进跟着折下来的每一行走）。
    const TOO_NARROW_FOR_ANYTHING: &str = r#"
"┌执行 · 第⋯约 3m20s┐"
"│ 总体 [===========│"
"└──────────────────┘"
" ⏎ 摊开取值 · t 试算"
" · x 执行 · q 退出 ·"
" ? 全部键           "
"#;

    /// 见 [`the_whole_screen_reads_at_eighty_by_twenty_four`]。
    const AT_EIGHTY_BY_TWENTY_FOUR_IDLE: &str = r#"
"┌配置────────────────────────────────────────────┐┌总览────────────────────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久       ▲│ 还没跑过。t 试算 · x 执行  │"
"│  型号　　　　　　未挑（跑起来之前必填）        █└────────────────────────────┘"
"│  感知可分辨级数　默认（跟随面板）              █┌报告────────────────────────┐"
"│  阈值　　　　　　跟着型号走（先挑一个）        █│                            │"
"│                                                █│ 按 t 试算：只算不写，报告照│"
"│口味层 · 这一趟的立场                           █│ 出。                       │"
"│  适配方式　　　　默认（height）                █│              按 x 执行：写 │"
"│  裁边　　　　　　默认（裁）                    █│              到输出根。    │"
"│  跨页拆分　　　　默认（拆）                    █│              跑起来之前必填│"
"│  拆分阈值　　　　默认（1.5）                   ║│              的两项是型号与│"
"│  阅读方向　　　　默认（rtl）                   ║│              输出根。      │"
"│  滤波器　　　　　默认（lanczos3）              ║│                            │"
"│  位深　　　　　　自动（判据说了算）            ║│                            │"
"│  抖动　　　　　　自动（判据说了算）            ║│                            │"
"│  逐页　　　　　　默认（关）                    ║│                            │"
"│  缓存预算　　　　默认（512.0 MiB）             ║│                            │"
"│  读取策略　　　　默认（auto）                  ║│                            │"
"│                                                ║│                            │"
"│范围层 · 每趟都不同，不进预设                   ▼│                            │"
"└────────────────────────────────────────────────┘└────────────────────────────┘"
" ⏎ 摊开取值 · t 试算 · x 执行 · q 退出 · ? 全部键                               "
"                                                                                "
"                                                                                "
"#;

    /// 见 [`the_whole_screen_reads_at_eighty_by_twenty_four`]。
    const AT_EIGHTY_BY_TWENTY_FOUR_RUNNING: &str = r#"
"┌配置────────────────────────────────────────────┐┌执行 · 第 3/3 ⋯ 还剩约 3m20s┐"
"│设备层 · 判定的依据，绑面板，改一次管很久       ▲│ 总体 [==================>  │"
"│  型号　　　　　　未挑（跑起来之前必填）        █│ 本卷 卷三 · 第二遍 [=======│"
"│  感知可分辨级数　默认（跟随面板）              █│ 完成 1 卷 · 跳过 1 卷      │"
"│  阈值　　　　　　跟着型号走（先挑一个）        █│ 出事 隔离 1 卷 · 失败 1 页 │"
"│                                                █└────────────────────────────┘"
"│口味层 · 这一趟的立场                           █┌报告────────────────────────┐"
"│  适配方式　　　　默认（height）                █│ 记号  目录  卷数           │"
"│  裁边　　　　　　默认（裁）                    █│ !     库       2  隔离 1 卷│"
"│  跨页拆分　　　　默认（拆）                    █│失败页（出现的当场，逐页那几│"
"│  拆分阈值　　　　默认（1.5）                   ║│行在整卷跑完后才有）        │"
"│  阅读方向　　　　默认（rtl）                   ║│  库/卷二/017.jpg           │"
"│  滤波器　　　　　默认（lanczos3）              ║│    失败 解不出完整尺寸：   │"
"│  位深　　　　　　自动（判据说了算）            ║│    JPEG 数据截断           │"
"│  抖动　　　　　　自动（判据说了算）            ║│                            │"
"│  逐页　　　　　　默认（关）                    ║│                            │"
"│  缓存预算　　　　默认（512.0 MiB）             ║│                            │"
"│  读取策略　　　　默认（auto）                  ║│                            │"
"│                                                ║│                            │"
"│范围层 · 每趟都不同，不进预设                   ▼│                            │"
"└────────────────────────────────────────────────┘└────────────────────────────┘"
" ⏎ 摊开取值 · t 试算 · x 执行 · q 退出 · ? 全部键                               "
"                                                                                "
"                                                                                "
"#;

    /// 见 [`the_whole_screen_reads_at_eighty_by_twenty_four`]。
    const AT_EIGHTY_BY_TWENTY_FOUR_VALUING: &str = r#"
"┌配置────────────────────────────────────────────┐┌总览────────────────────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久       ▲│ 还没跑过。t 试算 · x 执行  │"
"│  型号　　　　　　未挑（跑起来之前必填）        █└────────────────────────────┘"
"│  感知可分辨级数　默认（跟随面板）              █┌报告────────────────────────┐"
"│  阈值　　　　　　跟着型号走（先挑一个）        █│                            │"
"│                                                █│ 按 t 试算：只算不写，报告照│"
"│口味层 · 这一趟的立场                           █│ 出。                       │"
"│  适配方式　　　　默认（height）                █│              按 x 执行：写 │"
"│    ● 默认（height）                            █│              到输出根。    │"
"│    ○ height                                    █│              跑起来之前必填│"
"│    ○ inside                                    ║│              的两项是型号与│"
"│  裁边　　　　　　默认（裁）                    ║│              输出根。      │"
"│  跨页拆分　　　　默认（拆）                    ║│                            │"
"│  拆分阈值　　　　默认（1.5）                   ║│                            │"
"│  阅读方向　　　　默认（rtl）                   ║│                            │"
"│  滤波器　　　　　默认（lanczos3）              ║│                            │"
"│  位深　　　　　　自动（判据说了算）            ║│                            │"
"│  抖动　　　　　　自动（判据说了算）            ║│                            │"
"│  逐页　　　　　　默认（关）                    ║│                            │"
"│  缓存预算　　　　默认（512.0 MiB）             ▼│                            │"
"└────────────────────────────────────────────────┘└────────────────────────────┘"
" 适配方式 · ↑↓ 选 · ⏎／→ 定 · Esc／← 一格不改地回去 · q 退出 · ? 全部键         "
" 第一格是「没说」：它跟着默认值走，存成预设时那一项不写进去——与「说了一个恰好等 "
" 于默认的值」是两件事，后者往后默认改了也仍是那个值                             "
"#;

    /// 见 [`the_expanded_volume_reads_at_eighty_by_twenty_four`]。
    const AT_EIGHTY_BY_TWENTY_FOUR_EXPANDED: &str = r#"
"┌执行 · 第 5/6 卷 · 还剩约 1m00s───────────────────────────────────────────────┐"
"│ 总体 [=========================>    ] 5000/6000 步 · 已用 5m00s              │"
"│                                                                              │"
"│ 完成 3 卷 · 跳过 1 卷                                                        │"
"│ 出事 隔离 1 卷 · 失败 1 页 · 卷级失败 1 卷                                   │"
"└──────────────────────────────────────────────────────────────────────────────┘"
"┌报告 · 展开 哆啦 03（第 2/4 卷）──────────────────────────────────────────────┐"
"│ 基准档 4bit · 定档页 001.jpg · 要紧的页 2/2                                  │"
"│ 记号  页名     尺寸       判定  理由              判据                       │"
"│ *     001.jpg  1182x1680  4bit  阈值内最低的一档  4bit 8.000  定档页         │"
"│ ✗     017.jpg  1182x1680                                      失败 解不出完整│"
"│ 尺寸：JPEG 数据截断                                                          │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 选一页 · a 列全部页 · ⇥ 换下一卷 · e／Esc 收起，左栏回来 · q 退出 · ? 全部键"
" 只列要紧的页：特例 · 失败 · 部分救回 · 几何门不成立 · 宽溢出 · 兜底上界，加上定"
" 档页                                                                           "
"#;

    /// **快照：极窄那一档。**见
    /// [`a_terminal_forty_columns_wide_gives_everything_to_the_main_pane`]。
    ///
    /// 左栏在这一档上整个不在（[`config_width`]），40 列全归主区：两条横条读得出走到哪儿，
    /// 表砍到只剩记号 · 卷名 · 页数 · 基准档 · 定档页。从前这一档上左栏还占着 10 列——
    /// 行行折成两三个字，而主区照旧只有 [`MAIN_MIN_WIDTH`] 列。
    const FORTY_COLUMNS: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────┐"
"│ 总体 [==================>           ]│"
"│ 本卷 卷三 · 第二遍 [==========>      │"
"│ 完成 1 卷 · 跳过 1 卷                │"
"│ 出事 隔离 1 卷 · 失败 1 页           │"
"└──────────────────────────────────────┘"
"┌报告──────────────────────────────────┐"
"│ 记号  目录  卷数  基准档分布         │"
"│ !     库       2  跳过 1 ⋅ 4bit 1  隔│"
"│ 离 1 卷                              │"
"│失败页（出现的当场，逐页那几行在整卷跑│"
"│完后才有）                            │"
"│  库/卷二/017.jpg                     │"
"│    失败 解不出完整尺寸：JPEG 数据截断│"
"│                                      │"
"│                                      │"
"│                                      │"
"│                                      │"
"│                                      │"
"│                                      │"
"└──────────────────────────────────────┘"
" ⏎ 摊开取值 · t 试算 · x 执行 · q 退出 ·"
" ? 全部键                               "
"                                        "
"#;
}
