//! 画法那几块共用的**语义色**：四种语义、四种样子，**一处出处**
//! （`CONTEXT.md` 的《会话》：语义色）。
//!
//! | 语义 | 屏上 | 用在哪 |
//! |---|---|---|
//! | [平常](Tone::Plain) | 终端默认色 | 正常跑完的卷、普通的页 |
//! | [注意](Tone::Caution) | 黄 | 隔离、部分救回、宽溢出、兜底上界、几何门不成立、特例页 |
//! | [出事](Tone::Trouble) | 红 | 失败页、卷级失败、拒绝执行 |
//! | [不要紧](Tone::Muted) | 暗 | 跳过的卷、只读时的左栏 |
//!
//! # 画法各处按语义要色，不自己挑颜色
//!
//! **挑颜色的地方只有 [`Tone::style`] 一处**：画法各处交出来的是**语义**
//! （这一行是隔离、是跳过、是没做成），不是「黄」「红」「暗」——那是四处画法各挑一遍颜色
//! 与「改一次颜色只改一处」的分别。屏上那几块因此一个 `Color::` 都不写。
//!
//! （**读回来**的那一头是另一回事：测试探针 [`super::probe`] 认得出 `Color::Reset`
//! 才说得出「这一行没上色」。它不挑颜色，它问屏上有没有。）
//!
//! 屏上按语义要色的地方，眼下四处：卷表那几行（[`super::table`]，一行的语义由行首那个
//! [记号](super::table)说了算）、总览块的抬头与出事行（[`super::overview`]）、
//! 报告区当场冒出来的失败页那一段（[`super::report`]）、只读时的左栏（[`super::config`]）。
//!
//! # 颜色不是唯一载体
//!
//! **每一处上色的地方旁边都另有一个字或一个行首记号**：卷表那几行行首恒有一个记号
//! （`✓ ! – ✗`，砍列时它与卷名一起恒在，见 [`crate::session::columns`]），
//! 出事行行首是「出事」两个字，没做成那一趟的抬头里有「没做成」，失败页那一段头一行
//! 就叫「失败页」，只读的左栏抬头上写着「跑着，三层都只读」。
//!
//! 色盲、以及不上色的终端上因此一个字都不丢——[`NO_COLOR`](colourful) 那一张快照与
//! 上色那一张**文字逐格相同**（`the_same_screen_reads_the_same_with_or_without_colour`）。
//!
//! # 不是语义色的那几样
//!
//! 光标那一格**反白**、层抬头**加粗**、预设栏那条文件路径**压暗**：三样都不在这四种里，
//! `NO_COLOR` 也不抹掉它们。它们说的不是「这一趟怎么样」——反白说的是「就在这一行上动手」，
//! 而抹掉它之后屏上没有第二处说得出光标停在哪儿。**语义色抹掉了话还在**（上面那一条），
//! 这三样抹掉了话就没了，两者因此不归同一个开关管。
//!
//! 三样各自留在用它的那一块里（[`super::config`]、[`super::picker`]），本模块不收——
//! 收进来就等于说它们是同一件事。

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

/// **四种语义**，屏上一种一个样子（见模块文档那张表）。
///
/// 画法各处要的是这四个字之一，不是颜色：颜色名只在 [`style`](Self::style) 里出现。
///
/// **四种有轻重**，按 [`Ord`] 从轻到重排：不要紧 < 平常 < 注意 < 出事。
/// 派生它是为了一件具体的事——**一行上摆着好几件事时取最重的那一种**
/// （总览块的出事行同时数着隔离与失败页，见 [`super::overview`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Tone {
    /// **不要紧**：跳过的卷、只读时的左栏。屏上压暗。
    Muted,
    /// **平常**：正常跑完的卷、普通的页。屏上是终端默认色。
    Plain,
    /// **注意**：隔离、部分救回、宽溢出、兜底上界、几何门不成立、特例页。屏上黄。
    Caution,
    /// **出事**：失败页、卷级失败、拒绝执行。屏上红。
    Trouble,
}

impl Tone {
    /// 这一种语义在屏上什么样。**本仓库唯一写得出颜色名的地方。**
    ///
    /// **只用 16 色里的基本色，一处都不定背景色**：背景归用户自己的终端主题，
    /// 定了背景的那一格在深色底与浅色底之间必坏掉一边。
    ///
    /// 「**暗**」取的是 [`Modifier::DIM`] 而不是某一个灰：灰是个**绝对**的颜色，
    /// 深色底上的暗灰读不出来；DIM 压的是**当前这个前景色**，深色浅色两边都活。
    /// 它照旧归 [`colourful`] 管——它是这四种语义色之一，不是排版上的强调
    /// （模块文档《不是语义色的那几样》）。
    ///
    /// 不上色那一趟四种一律给 [`Style::default`]：**屏上的字一格不变**，
    /// 变的只有样式（样式改不动缓冲里的字符）。
    pub(super) fn style(self) -> Style {
        if !colourful() {
            return Style::default();
        }
        match self {
            Self::Plain => Style::default(),
            Self::Caution => Style::default().fg(Color::Yellow),
            Self::Trouble => Style::default().fg(Color::Red),
            Self::Muted => Style::default().add_modifier(Modifier::DIM),
        }
    }
}

/// 一行字，连同**它是哪一种语义**。
///
/// 打成一个类型而不是一对裸值：这两样是一起算出来的（一卷是不是隔离既定了行首那个记号、
/// 也定了这一行的语义），而 `(String, Tone)` 在调用处看不出哪一半是哪一半。
///
/// 一行**只有一种语义**：语义说的是「这一行讲的这件事怎么样」，而一行讲的是一件事。
/// 半行黄半行红没有对应的意思，也没有一处读得出它。
#[derive(Debug)]
pub(super) struct Painted {
    /// 这一行的字。**排版已经摆好了**——这一层只上色，一个空格都不动。
    pub(super) text: String,
    /// 这一行是哪一种语义。
    pub(super) tone: Tone,
}

impl Painted {
    /// 一行字加一种语义。
    pub(super) fn new(text: String, tone: Tone) -> Self {
        Self { text, tone }
    }

    /// 平常那一种。列头、横条那几行、还没跑过时说的那句话都是它。
    pub(super) fn plain(text: String) -> Self {
        Self::new(text, Tone::Plain)
    }

    /// 交给终端库画，**一行不折**。总览块那几行走它——那一格摆不下时由终端库自己截。
    pub(super) fn line(&self) -> Line<'static> {
        Line::styled(self.text.clone(), self.tone.style())
    }

    /// 折成这一格摆得下的那几行，**整段一个语义**（折出来的每一行都跟着上同一种色——
    /// 它们本来就是同一句话）。报告区那几段走它。
    ///
    /// 折行的规矩在**终端库之外**（[`crate::wrap`]）：`--help` 与命令行印出来的报告折的是
    /// 同一套，而那两处根本没有终端库。这里只交代**折到多宽**。
    pub(super) fn folded(&self, width: u16) -> Vec<Line<'static>> {
        let style = self.tone.style();
        crate::wrap::fold(&self.text, width)
            .into_iter()
            .map(|row| Line::styled(row, style))
            .collect()
    }

    /// 缩进一层摆在别人底下的那一份（卷表上摆在一卷那一行底下的那几句）。
    ///
    /// **语义原样跟着走**：缩进改的是摆法，不是这一行讲的那件事怎么样。
    pub(super) fn indented(&self, indent: &str) -> Self {
        Self::new(format!("{indent}{}", self.text), self.tone)
    }
}

/// **上不上色**。`NO_COLOR` 在场即不上色，而这里是它**唯一**生效的地方
/// （`CONTEXT.md` 的《会话》：语义色）——十个地方各判一次的话，漏掉一处就是
/// 「说好了不上色却还有一行是红的」。
///
/// 认的是**在不在场**，不是它的值：`NO_COLOR=` 与 `NO_COLOR=0` 一样算数
/// （<https://no-color.org> 那一条约定）。
///
/// 读一次记住：一趟会话里环境变量不会变，而这个判断每一帧要问几十次。
fn colourful() -> bool {
    #[cfg(test)]
    if let Some(forced) = forced() {
        return forced;
    }
    static COLOURFUL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *COLOURFUL.get_or_init(|| std::env::var_os("NO_COLOR").is_none())
}

#[cfg(test)]
mod forcing {
    use std::cell::Cell;

    thread_local! {
        /// 用例里按住的那个答案。**每个用例各跑在自己的线程上**（Rust 自带的那套 harness
        /// 就是这么跑的），因此按住它不会漏到别的用例上——而进程级的那一个会。
        ///
        /// 它同时挡掉另一件事：**开发机上真设着 `NO_COLOR` 的人**跑这几条用例照样绿。
        static FORCED: Cell<Option<bool>> = const { Cell::new(None) };
    }

    /// 用例这一刻按住的答案；没按就是 `None`，照环境变量走。
    pub(super) fn forced() -> Option<bool> {
        FORCED.with(Cell::get)
    }

    /// 在 `body` 这一段里按住「上色」或「不上色」，跑完松开。
    ///
    /// 两副样子要在同一条用例里比（票面第五条：两张快照文字逐格相同），
    /// 而环境变量在一个进程里只有一份。
    pub(super) fn forcing<T>(colourful: bool, body: impl FnOnce() -> T) -> T {
        FORCED.with(|held| held.set(Some(colourful)));
        let out = body();
        FORCED.with(|held| held.set(None));
        out
    }
}

#[cfg(test)]
use forcing::{forced, forcing};

#[cfg(test)]
mod tests {
    use super::super::probe::{
        OnScreen, a_run_in_flight, every_kind_of_volume, main_snapshot, painted, tight,
    };
    use super::super::{CONFIG_WIDTH, main_pane, shell};
    use super::*;
    use crate::session::live::{Live, Resuming};
    use crate::session::state::Session;
    use tonefit::Mode as RunMode;

    /// 一屏够宽够高，六种卷一行不砍。
    const WIDE: u16 = 96;
    const TALL: u16 = 30;

    /// **四种语义，四个样子，一处出处**（票面第一条）。
    ///
    /// 逐条问的是那三条硬约束：平常是终端默认色（一格都不改）、四种**互不相同**
    /// （否则「一眼看出重点」就少了一档）、**一处都不定背景色**（背景归用户的终端主题）。
    #[test]
    fn the_four_tones_are_the_only_place_a_colour_is_named() {
        let four = [Tone::Muted, Tone::Plain, Tone::Caution, Tone::Trouble];

        forcing(true, || {
            assert_eq!(Tone::Plain.style(), Style::default(), "平常就是终端默认色");
            assert_eq!(Tone::Caution.style().fg, Some(Color::Yellow));
            assert_eq!(Tone::Trouble.style().fg, Some(Color::Red));
            assert!(
                Tone::Muted.style().add_modifier.contains(Modifier::DIM),
                "「暗」压的是当前这个前景色"
            );
            for tone in four {
                assert_eq!(tone.style().bg, None, "{tone:?} 定了背景色");
            }
            for (at, one) in four.iter().enumerate() {
                for two in &four[at + 1..] {
                    assert_ne!(one.style(), two.style(), "{one:?} 与 {two:?} 长得一样");
                }
            }
        });
    }

    /// **`NO_COLOR` 在场即全不上色，一处生效**（票面第三条）。
    ///
    /// 四种一律退回终端默认色——不上色的终端上「这一趟怎么样」全靠那几个字与行首记号说
    /// （见模块文档《颜色不是唯一载体》）。
    #[test]
    fn no_color_takes_every_tone_back_to_the_terminal_default() {
        forcing(false, || {
            for tone in [Tone::Muted, Tone::Plain, Tone::Caution, Tone::Trouble] {
                assert_eq!(tone.style(), Style::default(), "{tone:?} 还上着色");
            }
        });
    }

    /// **四种语义有轻重，一行上摆着好几件事时取最重的那一种。**
    ///
    /// 总览块的出事行同时数着隔离（注意）与失败页（出事），而它只有一种颜色
    /// （见 [`super::super::overview`]）。
    #[test]
    fn the_four_tones_run_from_the_least_to_the_most_serious() {
        assert!(Tone::Muted < Tone::Plain);
        assert!(Tone::Plain < Tone::Caution);
        assert!(Tone::Caution < Tone::Trouble);
        assert_eq!(
            [Tone::Caution, Tone::Trouble, Tone::Plain]
                .into_iter()
                .max(),
            Some(Tone::Trouble)
        );
    }

    /// 六种卷都齐的那一趟，画在主区上、屏上那几行连同颜色一起取回来。
    fn rows_of_a_run_with_every_kind(colourful: bool) -> Vec<OnScreen> {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);
        let mut session = Session::new();
        forcing(colourful, || {
            painted(
                |frame| main_pane(frame, frame.area(), &mut session, Some(&live)),
                WIDE,
                TALL,
            )
        })
    }

    /// 屏上带着 `said` 那几个字的头一行。
    fn row_saying<'a>(rows: &'a [OnScreen], said: &str) -> &'a OnScreen {
        rows.iter()
            .find(|row| tight(&row.text).contains(&tight(said)))
            .unwrap_or_else(|| panic!("屏上没有说「{said}」的那一行"))
    }

    /// **出事那一行既是红的、也带着那个字**（票面第六条）。
    ///
    /// 快照那一路读的是缓冲，**样式读得出来**（spec 的《Testing Decisions》：颜色）。
    /// 三处一次问齐，各是一种语义配一个载体：
    ///
    /// - **卷级失败**那一行是**红**的，行首记号是 `✗`，档位那一列写着「没做成」；
    /// - **隔离**那一行是**黄**的，行首记号是 `!`，行尾那个词是「隔离」；
    /// - **跳过**那一行是**暗**的，行首记号是 `–`，档位那一列写着「跳过」。
    ///
    /// 正常跑完的那几卷一格都不上色：**四种里有一种是「不上色」**，
    /// 而屏上多数行属于它——人人都是红的等于没有红的。
    #[test]
    fn the_row_that_went_wrong_is_red_and_says_so() {
        let rows = rows_of_a_run_with_every_kind(true);

        let failed = row_saying(&rows, "消失的那卷");
        assert!(failed.colours.contains(&Color::Red), "没做成那一行不是红的");
        assert!(tight(&failed.text).contains('✗'), "{}", failed.text);
        assert!(tight(&failed.text).contains("没做成"), "{}", failed.text);

        let isolated = row_saying(&rows, "哆啦 03");
        assert!(
            isolated.colours.contains(&Color::Yellow),
            "隔离那一行不是黄的"
        );
        assert!(tight(&isolated.text).contains('!'), "{}", isolated.text);
        assert!(tight(&isolated.text).contains("隔离"), "{}", isolated.text);

        let skipped = row_saying(&rows, "棋魂 07");
        assert!(skipped.dim(), "跳过那一行没压暗");
        assert!(tight(&skipped.text).contains('–'), "{}", skipped.text);
        assert!(tight(&skipped.text).contains("跳过"), "{}", skipped.text);

        // 逐页判定与被覆盖的那两卷正常跑完，一格都不上色。
        for name in ["名侦探 05", "浪客行 12"] {
            let plain = row_saying(&rows, name);
            assert!(plain.colours.is_empty() && !plain.dim(), "{name} 上了色");
        }
    }

    /// **屏上每一处上过色的地方，都另有一个字或一个行首记号**（票面第二条）。
    ///
    /// 逐行走一遍主区：凡是上了色的行，必带着底下那几个载体之一。
    /// 这一条挡的是「往后添一处上色却忘了配一个字」——那种改动去掉颜色就丢信息，
    /// 而 `NO_COLOR` 那一趟与色盲的眼睛看到的正是去掉颜色的那一份。
    #[test]
    fn every_painted_row_carries_a_word_or_a_mark_of_its_own() {
        /// 四种记号，加上屏上说得出「怎么了」的那几个词。
        const CARRIERS: [&str; 10] = [
            "✓",
            "!",
            "–",
            "✗",
            "隔离",
            "跳过",
            "没做成",
            "出事",
            "失败",
            "部分救回",
        ];

        let rows = rows_of_a_run_with_every_kind(true);
        let painted: Vec<&OnScreen> = rows
            .iter()
            .filter(|row| !row.colours.is_empty() || row.dim())
            .collect();

        assert!(
            painted.len() >= 3,
            "这一趟该有几行上色的：{}",
            painted.len()
        );
        for row in painted {
            let text = tight(&row.text);
            assert!(
                CARRIERS.iter().any(|carrier| text.contains(carrier)),
                "这一行上了色却没有一个字接得住：{}",
                row.text
            );
        }
    }

    /// **一段整段上色时，接住颜色的是那一段的头一行**（票面第二条）。
    ///
    /// 失败页那一段整段红（见 [`super::super::report`]），而它头一行就叫「失败页」、
    /// 逐条的原因行上还有「失败」两个字——去掉颜色一个字都不丢。
    /// 中间那几行是页的路径：**载体是那一段，不是那一段里的每一行**——一段话是一起读的。
    ///
    /// 同一屏上顺带问出事行那一条：它同时数着隔离（注意）与失败页（出事），
    /// 而一行只上得了一种色——取最重的那一个（见 [`super::super::overview`]）。
    #[test]
    fn the_failing_pages_block_goes_red_under_a_heading_that_says_so() {
        let live = a_run_in_flight(true);
        let mut session = Session::new();

        let rows = forcing(true, || {
            painted(
                |frame| main_pane(frame, frame.area(), &mut session, Some(&live)),
                WIDE,
                TALL,
            )
        });

        for said in ["失败页（出现的当场", "失败 解不出完整尺寸"] {
            let row = row_saying(&rows, said);
            assert!(
                row.colours.contains(&Color::Red),
                "「{said}」那一行不是红的"
            );
        }
        let trouble = row_saying(&rows, "出事 隔离");
        assert!(
            trouble.colours.contains(&Color::Red),
            "出事行没取最重的那一档：{}",
            trouble.text
        );
    }

    /// **上色的那一张与 `NO_COLOR` 那一张，文字逐格相同**（票面第五条）。
    ///
    /// 快照比的是**屏上的字**（[`super::super::probe::snapshot`] 走终端库自己的
    /// `Display`），而上色只动样式、一个字符都不动——这一条把那件事钉住：
    /// 不上色的终端上**一个字都不丢**。
    ///
    /// 两头各再问一句「确实上了色／确实没上色」：少了它，这一条在「两边都没上色」
    /// 时照样绿，而那正是它要挡的回归。
    #[test]
    fn the_same_screen_reads_the_same_with_or_without_colour() {
        let live = every_kind_of_volume(RunMode::Process, Resuming::GoesOn);

        let coloured = forcing(true, || main_snapshot(&live, WIDE, TALL));
        let colourless = forcing(false, || main_snapshot(&live, WIDE, TALL));

        assert_eq!(coloured, colourless, "上色改动了屏上的字");
        assert!(
            rows_of_a_run_with_every_kind(true)
                .iter()
                .any(|row| !row.colours.is_empty() || row.dim()),
            "上色那一张一处都没上色，这一条问了个空"
        );
        assert!(
            rows_of_a_run_with_every_kind(false)
                .iter()
                .all(|row| row.colours.is_empty() && !row.dim()),
            "NO_COLOR 那一张还有上了色的行"
        );
    }

    /// **只读时的左栏压暗，而那件事另有一句话说得出来**（票面第二条的另一半）。
    ///
    /// 左栏那几行本身没有一个字说得出「只读」——接住颜色的是**那一格的抬头**
    /// （`配置 · 跑着，三层都只读`，见 [`super::super::config`]）。
    /// `NO_COLOR` 那一趟因此照样说得清楚。
    #[test]
    fn the_read_only_left_column_is_muted_and_the_title_says_why() {
        let mut session = Session::new();
        session.run_started();
        let live = Live::new(
            &crate::session::live::fixture::request(RunMode::Process),
            Resuming::GoesOn,
        );

        let running = forcing(true, || {
            painted(|frame| shell(frame, &mut session, Some(&live)), WIDE, TALL)
        });

        // 左栏那几行压暗了：只数左栏那几列，主区自己也有上了色的行。
        let muted = running
            .iter()
            .filter(|row| row.dim_before(CONFIG_WIDTH))
            .count();
        assert!(muted > 3, "跑起来之后左栏没压暗：{muted} 行");
        // 而「为什么压暗」写在抬头上，去掉颜色它还在。
        let colourless = forcing(false, || {
            painted(|frame| shell(frame, &mut session, Some(&live)), WIDE, TALL)
        });
        assert!(
            colourless
                .iter()
                .all(|row| !row.dim_before(CONFIG_WIDTH) && row.colours.is_empty()),
            "NO_COLOR 那一趟左栏还压着暗"
        );
        assert!(
            colourless
                .iter()
                .any(|row| tight(&row.text).contains(&tight("跑着，三层都只读"))),
            "抬头没说这一栏为什么按不动"
        );
    }
}
