//! **覆盖层**与它上面那一张**全部按键**（`CONTEXT.md` 的《会话》：覆盖层；spec《按键表、屏底与覆盖层》）。
//!
//! 一个键掀开、盖在视图上的那一张：`?` 是全部按键，打字时是 `F1`，**备注行上 `⏎` 是说明卡**；
//! 掀着的时候底下整屏压暗，关掉之后底下原样回来（它盖住一块焦点，不替掉它——
//! 输入行、缓冲与补全框都还在）。
//!
//! # 两张各自的内容都在这里
//!
//! [`Sheet`] 是全部按键那一张（出自按键表），[`Card`] 是**说明卡**（出自报告末尾那一小结）。
//! 两张都只出「摆好的字与框摆在哪儿」，画它们的是 `super::shell::overlay`。
//!
//! # 全部按键那一张出自按键表
//!
//! **一个键都不列**：[`Sheet`] 把 [`super::keymap::TABLE`] 上此刻阶段派得出、长的那一句不为空的行
//! 按组摆开——同一组里长的那一句相同的几行并成一行（`j k`、`l ⏎`），一行都不剩的组整组不出；
//! 末尾再接一节**灰阶写法**（那不是键，是屏上 `1bit`／`+FS` 那几个字各是什么意思）。
//! 表里加一个键，屏底与这一张都跟着出现——用例 `the_sheet_and_the_footer_both_come_from_the_key_table` 在本模块里。
//!
//! # 它记的是从第几行画起
//!
//! 屏上唯一记着滚动量的一处：它是读物，一行上没有第二步可走，没有光标可跟。宽时摆成两栏
//! （一栏放不下的那一半从中间劈开），装不下时 `j`／`k` 滚得动——几行、露几行都随屏的尺寸走
//! （[`Sheet::of`]），滚动因此要拿着窗口的尺寸算（[`Views::scroll_cover`]），窗口有多大只有终端层知道。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）；画它的是 `super::shell::overlay`。

use std::path::Path;

use tonefit::{BitDepth, Candidate, Dither};

use super::home::Home;
use super::keymap::{Deed, Group, Phase, Row, TABLE};
use super::look::{Kind, Look, Segment};
use super::tree::{Note, NoteKind};
use super::view::{Views, Window};

/// 盖在视图上的那一张。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// 全部按键；`from` 是从第几行画起（模块文档《它记的是从第几行画起》）。
    Keys { from: usize },
    /// **说明卡**：备注行上 `⏎` 掀开、`Esc` 关的那一张（`CONTEXT.md` 的《说明卡》；
    /// spec《按键表、屏底与覆盖层》）。
    ///
    /// 记的是**树上第几个节点的第几条备注**，不是那一条备注本身：树一趟只拼一次
    /// （[`super::tree`]），掀着的这一会儿它一格不动，而这么记本模块就不必克隆一份备注。
    /// 它**不滚**——一条备注装得下的那几处，卡自己就那么高（见 [`Card::of`]）。
    Note { node: usize, at: usize },
}

/// 全部按键那一张至多多宽（设计稿 `drawHelp` 的 `104`）。
const WIDEST: u16 = 104;
/// **说明卡**至多多宽，以及它离窗口左右两边各留几列（设计稿 `drawNote` 的 `76` 与 `8`）。
const CARD_WIDEST: u16 = 76;
const CARD_SIDE_MARGIN: u16 = 4;
/// 说明卡上下两边各留几行，以及框里除了正文还占几行（上下框线 · 「是哪几处」那一行 ·
/// 它上下各一个空行 · 正文前那一个空行，设计稿 `drawNote` 的 `4` 与 `6`）。
const CARD_TOP_MARGIN: u16 = 2;
const CARD_CHROME: u16 = 6;
/// 正文从框的左边第几格起写，以及左右各让出几格（设计稿 `drawNote` 的 `x + 3` 与 `w - 6`）。
const CARD_TEXT_INDENT: u16 = 3;
const CARD_TEXT_GUTTER: u16 = 6;
/// 框离窗口左右两边各留几列、离上下两边各留几行。
const SIDE_MARGIN: u16 = 2;
const TOP_MARGIN: u16 = 1;
/// 框宽到这么多列摆成两栏。
const TWO_COLUMNS_FROM: u16 = 100;
/// 键的写法那一列有多宽。
const KEY_COLUMN: usize = 10;

/// 屏上一行：左栏那几截、右栏那几截（单栏时右栏空着）。
pub type Line = (Vec<Segment>, Vec<Segment>);

/// 一节：组名、一键一行、末尾一个空行。
type Section = Vec<Vec<Segment>>;

/// 框摆在窗口的哪儿：左上角在第几列、第几行，多宽、多高。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// **全部按键那一张**：这一档上派得出的键按组摆开、按窗口的尺寸分栏，连同它在窗口里的位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// 摆好的行，一行一对（左栏、右栏）。
    pub lines: Vec<Line>,
    /// 摆成两栏没有。
    pub two_columns: bool,
    /// 框摆在哪儿。
    pub placement: Placement,
}

impl Sheet {
    /// 这一档、这么大的窗口里那一张。
    pub fn of(phase: Phase, window: Window) -> Self {
        let width = window.cols.saturating_sub(2 * SIDE_MARGIN).min(WIDEST);
        let height = window.rows.saturating_sub(2 * TOP_MARGIN);
        let x = (window.cols.saturating_sub(width)) / 2;
        let two_columns = width >= TWO_COLUMNS_FROM;
        let groups = groups(phase);
        let lines = if two_columns {
            let total: usize = groups.iter().map(Vec::len).sum();
            let mut split = groups.len();
            let mut seen = 0;
            for (i, group) in groups.iter().enumerate() {
                if seen * 2 >= total {
                    split = i;
                    break;
                }
                seen += group.len();
            }
            let (left, right) = groups.split_at(split);
            let mut left: Vec<Vec<Segment>> = left.iter().flatten().cloned().collect();
            let mut right: Vec<Vec<Segment>> = right.iter().flatten().cloned().collect();
            let rows = left.len().max(right.len());
            left.resize(rows, Vec::new());
            right.resize(rows, Vec::new());
            left.into_iter().zip(right).collect()
        } else {
            groups
                .into_iter()
                .flatten()
                .map(|line| (line, Vec::new()))
                .collect()
        };
        Self {
            lines,
            two_columns,
            placement: Placement {
                x,
                y: TOP_MARGIN,
                width,
                height,
            },
        }
    }

    /// 框里露得出几行。
    pub fn shown(&self) -> usize {
        usize::from(self.placement.height.saturating_sub(2))
    }

    /// 从第几行画起最多到哪儿：再往下就是把看得见的东西滚掉。
    pub fn last_from(&self) -> usize {
        self.lines.len().saturating_sub(self.shown())
    }

    /// 记着的起点收进这一张真摆得下的那一段里。
    pub fn from(&self, wanted: usize) -> usize {
        wanted.min(self.last_from())
    }

    /// 每一栏多宽（设计稿 `drawHelp` 的 `cw`：两栏时各占框宽减六的一半，单栏时框宽减四；
    /// 字从每栏第三格起写、占 `cw - 2` 格）。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn column_width(&self) -> u16 {
        if self.two_columns {
            self.placement.width.saturating_sub(6) / 2
        } else {
            self.placement.width.saturating_sub(4)
        }
    }
}

/// **说明卡**：一条[备注](super::tree::Note)的全文，连同它在窗口里的位置
/// （`CONTEXT.md` 的《说明卡》；设计稿 `drawNote`）。
///
/// 框**居中**，宽是「窗口宽减八，至多 76」，高随正文的行数走（至多窗口高减四）——
/// 装得下就装得下，**它不滚**：一条备注顶多装[列得出的那几处](crate::FirstFew)。
///
/// 正文的字**出自报告末尾那一小结**（[`crate::render::unreachable_stack`] 与
/// [`crate::render::non_volume_stack`]，ADR 0016：一格的字只有一处出处）——
/// 一条备注只装它自己那几处，因此喂的是它自己那一份 `said`。
/// 屏上那几条路径把家目录缩成 `~`（[`Home::abbreviate`]），命令行那一份原样印，
/// 两处差的只有「一条路径怎么写」。折行走屏上唯一那一套（[`crate::wrap::fold`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// 名头，也是框的抬头（`无法访问`／`已忽略 3 个文件`）。
    pub label: String,
    /// 是哪几处——框里头一行。
    pub what: String,
    /// 全文，按框里那一截的宽度折好的那几行。
    pub body: Vec<String>,
    /// 这一条备注是哪一种——**无法访问那一种**框线与抬头上出事那一色
    /// （设计稿 `drawNote` 的 `bad`）。带着它那一格、不折成一个 `bool`：
    /// 「是哪一种」在树上已经有一个名字（[`NoteKind`]），折一次画法那一头就得再认回来。
    pub kind: NoteKind,
    /// 框摆在哪儿。
    pub placement: Placement,
}

impl Card {
    /// 这一条备注在这么大的窗口里摆出来的那张卡。
    pub fn of(note: &Note, window: Window, home: &Home) -> Self {
        let width = window
            .cols
            .saturating_sub(2 * CARD_SIDE_MARGIN)
            .min(CARD_WIDEST);
        let room = width.saturating_sub(CARD_TEXT_GUTTER);
        let shown = |path: &Path| home.abbreviate(path);
        let said = if note.kind == NoteKind::Unreachable {
            crate::render::unreachable_stack(&note.said, shown)
        } else {
            crate::render::non_volume_stack(&note.said, shown)
        };
        // 那一小结每一条都以换行收尾（命令行印的就是那一段），末尾那个换行折出来是一个空行
        // ——卡上不留它。
        let body = crate::wrap::fold(said.trim_end_matches('\n'), room);
        let height = window.rows.saturating_sub(2 * CARD_TOP_MARGIN).min(
            u16::try_from(body.len())
                .unwrap_or(u16::MAX)
                .saturating_add(CARD_CHROME),
        );
        Self {
            label: note.label.clone(),
            what: note.what.clone(),
            body,
            kind: note.kind,
            placement: Placement {
                x: (window.cols.saturating_sub(width)) / 2,
                y: (window.rows.saturating_sub(height)) / 2,
                width,
                height,
            },
        }
    }

    /// 正文与「是哪几处」那两行从框的左边第几格起写。
    pub fn text_x(&self) -> u16 {
        self.placement.x + CARD_TEXT_INDENT
    }

    /// 框里那一截字有多宽。
    pub fn text_width(&self) -> u16 {
        self.placement.width.saturating_sub(CARD_TEXT_GUTTER)
    }
}

/// 表上此刻派得出的行按组摆开：每组一行组名、一键一行、末尾一个空行；末尾再接灰阶写法那一节。
fn groups(phase: Phase) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for group in Group::ALL {
        let mut rows: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
        for row in TABLE.iter().filter(|row| listed(row, phase, group)) {
            match rows.iter_mut().find(|(long, _)| *long == row.long) {
                Some((_, keys)) => {
                    if !keys.contains(&row.spelt) {
                        keys.push(row.spelt);
                    }
                }
                None => rows.push((row.long, vec![row.spelt])),
            }
        }
        if rows.is_empty() {
            continue;
        }
        out.push(section(
            group.title(),
            rows.into_iter()
                .map(|(long, keys)| (keys.join(" "), long.to_owned())),
        ));
    }
    out.push(section("灰阶写法", depths().into_iter()));
    out
}

/// 这一行上不上这一档的全部按键那一张、归哪一组。
fn listed(row: &Row, phase: Phase, group: Group) -> bool {
    row.group == group
        && !row.long.is_empty()
        && (row.phases.is_empty() || row.phases.contains(&phase))
}

/// 一节：组名、每行一个键与它做的事、末尾一个空行。
fn section(title: &str, rows: impl Iterator<Item = (String, String)>) -> Section {
    let mut lines = vec![vec![Segment::new(title, Look::kind(Kind::Caption).bold())]];
    lines.extend(rows.map(|(keys, what)| {
        vec![
            Segment::new(padded(&keys, KEY_COLUMN), Look::kind(Kind::Key)),
            Segment::plain(what),
        ]
    }));
    lines.push(Vec::new());
    lines
}

/// 屏上灰阶那几个字各是什么意思：电子墨水屏的三档，外加 `+FS` 是加抖动。写法从库取
/// （[`BitDepth`] 与 [`Candidate`] 各自的 `Display`），级数从 [`BitDepth::levels`] 取。
fn depths() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = [BitDepth::One, BitDepth::Two, BitDepth::Four]
        .into_iter()
        .map(|depth| (depth.to_string(), format!("{} 级灰", depth.levels())))
        .collect();
    let dithered = Candidate {
        bit_depth: BitDepth::One,
        dither: Dither::FloydSteinberg,
    }
    .to_string();
    let suffix = dithered
        .strip_prefix(&BitDepth::One.to_string())
        .unwrap_or(&dithered)
        .to_owned();
    out.push((suffix, "加抖动（误差扩散）".to_owned()));
    out
}

/// 补到 `width` 格宽（左对齐），按显示宽度算。
fn padded(text: &str, width: usize) -> String {
    let used = usize::from(crate::wrap::width(text));
    format!("{text}{}", " ".repeat(width.saturating_sub(used)))
}

impl Views {
    /// 掀开全部按键那一张（从头画起）。
    pub fn lift_keys(&mut self) {
        self.cover = Some(Overlay::Keys { from: 0 });
    }

    /// 掀开**说明卡**：树上第几个节点的第几条备注（[`Overlay::Note`]）。
    pub fn lift_note(&mut self, node: usize, at: usize) {
        self.cover = Some(Overlay::Note { node, at });
    }

    /// 关掉盖着的那一张；底下原样回来。
    pub fn drop_cover(&mut self) {
        self.cover = None;
    }

    /// 覆盖层上滚一步：`j`／`k` 一行、`C-d`／`C-u`／`C-f`／`C-b` [一屏](Window::page)、
    /// `gg`／`G` 到顶到底，起点收在那一张真摆得下的那一段里（到顶为止、到底为止）。
    /// 这件事不是滚动、或者掀着的不是那一张，什么都不做、交回 `false`。
    ///
    /// **那四个在这一张上挪的都是一整屏**（设计稿 `taskKey` 的覆盖层那一支：
    /// `C-d` 与 `C-f` 同挪 `pageH()`）——这一张是读物，半屏与一屏的分别在它身上没有意义。
    pub fn scroll_cover(&mut self, deed: Deed, sheet: &Sheet, window: Window) -> bool {
        // 说明卡不滚：一条备注装得下的那几处，卡自己就那么高（[`Card::of`]）。
        let Some(Overlay::Keys { from }) = &mut self.cover else {
            return false;
        };
        let page = window.page();
        *from = match deed {
            Deed::Down => sheet.from(*from + 1),
            Deed::Up => sheet.from(from.saturating_sub(1)),
            Deed::HalfDown | Deed::PageDown => sheet.from(*from + page),
            Deed::HalfUp | Deed::PageUp => sheet.from(from.saturating_sub(page)),
            Deed::Top => 0,
            Deed::Bottom => sheet.last_from(),
            _ => return false,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::Key;

    fn wide() -> Window {
        Window {
            cols: 120,
            rows: 36,
        }
    }

    fn narrow() -> Window {
        Window { cols: 80, rows: 24 }
    }

    /// 一行的字连起来。
    fn text(line: &[Segment]) -> String {
        line.iter().map(|segment| segment.text.as_str()).collect()
    }

    /// 那一张只列此刻派得出的键，按用途分组，同一件事的几个键并成一行；一个键都不剩的组整组不出。
    /// 还没开始那一档：`o` 添加路径在、`s` 停止不在、「确认」整组不出；末尾接灰阶写法那一节。
    #[test]
    fn the_sheet_lists_only_the_keys_dealt_in_this_phase_grouped_by_purpose() {
        let sheet = Sheet::of(Phase::Fresh, wide());
        let lines: Vec<String> = sheet
            .lines
            .iter()
            .flat_map(|(left, right)| [text(left), text(right)])
            .map(|line| line.trim_end().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        assert!(
            lines.contains(&"o         添加路径".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"j k       上下一行".to_owned()),
            "j 与 k 并成一行"
        );
        assert!(lines.contains(&"i ⏎       修改这一条".to_owned()));
        assert!(lines.contains(&"q         退出".to_owned()));
        assert!(
            !lines.iter().any(|line| line.starts_with("s ")),
            "还没开始时停止派不出"
        );
        assert!(
            !lines.contains(&"确认".to_owned()),
            "一个键都不剩的组整组不出"
        );
        assert!(lines.contains(&"灰阶写法".to_owned()));
        assert!(lines.contains(&"+FS       加抖动（误差扩散）".to_owned()));
        // 跑着那一档 `q` 换一句、`s` 在、`o` 不在。
        let running = Sheet::of(Phase::Running, wide());
        let lines: Vec<String> = running
            .lines
            .iter()
            .flat_map(|(left, right)| [text(left), text(right)])
            .map(|line| line.trim_end().to_owned())
            .collect();
        assert!(lines.contains(&"q         不退出：先按 s 停止，或按 C-c".to_owned()));
        assert!(lines.iter().any(|line| line.starts_with("s         停止")));
        assert!(!lines.iter().any(|line| line.starts_with("o ")));
    }

    /// 宽时两栏、窄时一栏：120×36 上 104 格宽、两栏，80×24 上 76 格宽、一栏；
    /// 两栏时从中间劈开，行数是两栏里长的那一栏。
    #[test]
    fn the_sheet_splits_into_two_columns_when_wide_enough() {
        let sheet = Sheet::of(Phase::Fresh, wide());
        assert!(sheet.two_columns);
        assert_eq!(
            sheet.placement,
            Placement {
                x: 8,
                y: 1,
                width: 104,
                height: 34
            }
        );
        let single = Sheet::of(Phase::Fresh, narrow());
        assert!(!single.two_columns);
        assert_eq!(
            single.placement,
            Placement {
                x: 2,
                y: 1,
                width: 76,
                height: 22
            }
        );
        let total = single.lines.len();
        assert!(sheet.lines.len() < total && sheet.lines.len() * 2 >= total);
        assert!(sheet.lines.iter().any(|(_, right)| !right.is_empty()));
        assert!(single.lines.iter().all(|(_, right)| right.is_empty()));
    }

    /// 滚动收在那一张摆得下的那一段里：到顶为止、到底为止，装得下时一动不动。
    #[test]
    fn scrolling_the_cover_stops_at_either_end() {
        let mut views = Views::default();
        views.lift_keys();
        let sheet = Sheet::of(Phase::Fresh, narrow());
        assert!(sheet.last_from() > 0, "80×24 上那一张装不下");
        assert!(views.scroll_cover(Deed::Up, &sheet, narrow()));
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }));
        views.scroll_cover(Deed::Down, &sheet, narrow());
        assert_eq!(views.cover, Some(Overlay::Keys { from: 1 }));
        assert!(
            !views.scroll_cover(Deed::Help, &sheet, narrow()),
            "不是滚动的事不管"
        );
        assert_eq!(views.cover, Some(Overlay::Keys { from: 1 }));
        views.scroll_cover(Deed::Bottom, &sheet, narrow());
        assert_eq!(
            views.cover,
            Some(Overlay::Keys {
                from: sheet.last_from()
            })
        );
        views.scroll_cover(Deed::Down, &sheet, narrow());
        assert_eq!(
            views.cover,
            Some(Overlay::Keys {
                from: sheet.last_from()
            }),
            "到底为止"
        );
        views.scroll_cover(Deed::Top, &sheet, narrow());
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }));
        let roomy = Sheet::of(Phase::Fresh, wide());
        views.scroll_cover(Deed::Down, &roomy, wide());
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }), "装得下就不滚");
        // **半屏与一屏那四个在这一张上挪一整屏**（设计稿覆盖层那一支）。
        views.scroll_cover(Deed::HalfDown, &sheet, narrow());
        assert_eq!(
            views.cover,
            Some(Overlay::Keys {
                from: sheet.from(narrow().page())
            }),
            "`C-d` 在这一张上挪一整屏"
        );
        views.scroll_cover(Deed::PageUp, &sheet, narrow());
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }));
        // **说明卡不滚**：它没有第二屏，滚动那几件在它身上一件都不派。
        views.lift_note(0, 0);
        assert!(!views.scroll_cover(Deed::Down, &sheet, narrow()));
        assert_eq!(views.cover, Some(Overlay::Note { node: 0, at: 0 }));
        views.drop_cover();
        assert_eq!(views.cover, None);
        assert!(
            !views.scroll_cover(Deed::Down, &sheet, narrow()),
            "没掀着就不管"
        );
    }

    /// 一条备注：两种各造一条，路径落在家目录底下。
    fn a_note(kind: NoteKind) -> Note {
        let home = std::path::PathBuf::from("/home/me");
        match kind {
            NoteKind::Unreachable => Note {
                kind,
                at: home.join("漫画库/私藏"),
                label: "无法访问".to_owned(),
                what: "私藏/".to_owned(),
                brief: "列出 …: Permission denied".to_owned(),
                said: vec![(
                    home.join("漫画库/私藏"),
                    "列出 …: Permission denied".to_owned(),
                )],
            },
            NoteKind::NonVolume => Note {
                kind,
                at: home.join("漫画库"),
                label: "已忽略 2 个文件".to_owned(),
                what: "字体包.zip、答案.txt".to_owned(),
                brief: crate::render::non_volume_heading(2),
                said: vec![
                    (
                        home.join("漫画库/字体包.zip"),
                        "压缩包里没有图片".to_owned(),
                    ),
                    (home.join("漫画库/答案.txt"), "不属于任何一卷".to_owned()),
                ],
            },
        }
    }

    /// **说明卡的全文出自报告末尾那一小结**（ADR 0016：一格的字只有一处出处），
    /// 屏上那几条路径把家目录缩成 `~`，折行走屏上唯一那一套（`crate::wrap`）。
    ///
    /// 抬头那一句、逐条「路径一行、原因一行」都不在本模块里写第二遍：
    /// 这一条拿[非漫画文件那一小结](crate::render::non_volume_stack)的头一句去比。
    #[test]
    fn the_card_says_what_the_report_says_with_the_home_written_as_a_tilde() {
        let home = Home::at("/home/me");
        let card = Card::of(&a_note(NoteKind::NonVolume), wide(), &home);
        assert_eq!(card.label, "已忽略 2 个文件");
        assert_eq!(card.what, "字体包.zip、答案.txt");
        assert_eq!(card.kind, NoteKind::NonVolume);
        // 抬头那一句就是报告那一小结的抬头（折行之后是头一行的那一截）。
        let heading = crate::render::non_volume_heading(2);
        assert!(
            heading.starts_with(card.body.first().expect("正文不空").as_str()),
            "正文头一行不是那一小结的抬头：{:?}",
            card.body.first()
        );
        // 逐条：路径一行（缩成 `~`）、原因一行。
        assert!(
            card.body.iter().any(|line| line == "  ~/漫画库/字体包.zip"),
            "{:?}",
            card.body
        );
        assert!(
            card.body.iter().any(|line| line == "    压缩包里没有图片"),
            "{:?}",
            card.body
        );
        assert!(
            card.body.iter().all(|line| !line.contains("/home/me")),
            "屏上还写着家目录的全名：{:?}",
            card.body
        );
        // 末尾那个换行不折出一个空行来。
        assert!(!card.body.last().expect("正文不空").is_empty());
        // 无法访问那一种是出事那一色，抬头是它自己的名头。
        let bad = Card::of(&a_note(NoteKind::Unreachable), wide(), &home);
        assert_eq!(
            (bad.kind, bad.label.as_str()),
            (NoteKind::Unreachable, "无法访问")
        );
    }

    /// **卡居中，宽至多 76、离两边各留四列，高随正文走**（设计稿 `drawNote`）：
    /// 正文之外还占六行（上下框线 · 「是哪几处」那一行与它上下的空行 · 正文前那一行）。
    #[test]
    fn the_card_is_centred_and_as_tall_as_its_text() {
        let home = Home::at("/home/me");
        let note = a_note(NoteKind::NonVolume);
        let card = Card::of(&note, wide(), &home);
        let at = card.placement;
        assert_eq!(at.width, 76, "120 列上是那个上限");
        assert_eq!(at.x, (120 - 76) / 2, "居中");
        assert_eq!(
            at.height,
            card.body.len() as u16 + 6,
            "高随正文走（还没到窗口那个上限）"
        );
        assert_eq!(at.y, (36 - at.height) / 2, "居中");
        assert_eq!(card.text_x(), at.x + 3);
        assert_eq!(card.text_width(), at.width - 6);
        // 窄窗口上让给两边各四列，正文跟着折得更碎、卡更高。
        let narrow = Card::of(&note, Window { cols: 60, rows: 24 }, &home);
        assert_eq!(narrow.placement.width, 60 - 8);
        assert!(narrow.body.len() > card.body.len(), "窄了折出更多行");
        assert!(
            narrow.placement.height <= 24 - 4,
            "再高也不越过窗口那个上限"
        );
    }

    /// **全部按键与屏底出自同一张表**（`session-redesign/07` 票面第四条）：表上每一行，长的那一句不空的
    /// 在它派得出的每一档上都在那一张上（键的写法在那一行头上、那一句在后面），短的那一句不空的在它派得出的
    /// 那一块上屏底问它就摆得出来；反过来那一张上每一行都是表上的一行（组名与灰阶写法那一节除外）。
    /// 表里加一个键——加一行——两处都跟着出现，不必改第二处。
    #[test]
    fn the_sheet_and_the_footer_both_come_from_the_key_table() {
        use super::super::keymap::{Want, hints};
        use super::super::view::Focus;
        const PHASES: [Phase; 5] = [
            Phase::Fresh,
            Phase::Surveying,
            Phase::Running,
            Phase::Deciding,
            Phase::Ended,
        ];
        const BLOCKS: [Focus; 7] = [
            Focus::VolumeList,
            Focus::Pages,
            Focus::Settings,
            Focus::Details,
            Focus::Picker,
            Focus::Input,
            Focus::Overlay,
        ];
        let titles: Vec<&str> = Group::ALL
            .iter()
            .map(|group| group.title())
            .chain(["灰阶写法"])
            .collect();
        let legend: Vec<String> = depths().into_iter().map(|(spelt, _)| spelt).collect();
        for phase in PHASES {
            let lines: Vec<String> = Sheet::of(phase, narrow())
                .lines
                .iter()
                .map(|(left, _)| text(left).trim_end().to_owned())
                .filter(|line| !line.is_empty())
                .collect();
            let carries = |line: &str, spelt: &str, long: &str| {
                line.ends_with(long)
                    && line[..line.len() - long.len()]
                        .split_whitespace()
                        .any(|key| key == spelt)
            };
            for row in TABLE {
                let dealt_now = row.phases.is_empty() || row.phases.contains(&phase);
                if !row.long.is_empty() && dealt_now {
                    assert!(
                        lines.iter().any(|line| carries(line, row.spelt, row.long)),
                        "{phase:?} 的那一张上没有「{} {}」",
                        row.spelt,
                        row.long
                    );
                }
                if !row.short.is_empty() {
                    for focus in BLOCKS
                        .into_iter()
                        .filter(|focus| row.applies(phase, *focus))
                    {
                        let said = hints(phase, focus, &[Want::saying(row.deed, row.short)]);
                        assert!(
                            said.iter()
                                .any(|hint| hint.what == row.short && !hint.keys.is_empty()),
                            "{phase:?} 的 {focus:?} 上屏底摆不出「{} → {}」",
                            row.spelt,
                            row.short
                        );
                    }
                }
            }
            for line in &lines {
                if titles.contains(&line.as_str()) {
                    continue;
                }
                let (keys, what) = line.split_at(line.find("  ").expect("键与那一句之间空着"));
                let keys: Vec<&str> = keys.split_whitespace().collect();
                let what = what.trim_start();
                let on_the_table = TABLE.iter().any(|row| {
                    row.long == what
                        && (row.phases.is_empty() || row.phases.contains(&phase))
                        && keys.contains(&row.spelt)
                });
                let on_the_legend = keys
                    .iter()
                    .all(|key| legend.iter().any(|spelt| spelt == key));
                assert!(
                    on_the_table || on_the_legend,
                    "{phase:?} 的那一张上「{line}」不出自表"
                );
            }
        }
    }

    /// 掀开那一张的键再按一次与 `Esc` 都关得掉它——抬头上写的正是这两个键。
    #[test]
    fn the_key_that_lifts_the_sheet_and_escape_both_close_it() {
        use super::super::keymap::{Chord, deed, spelt_for};
        use super::super::view::Focus;
        let lifts = TABLE
            .iter()
            .find(|row| row.deed == Deed::Help)
            .expect("表上有掀开它的那一行");
        assert_eq!(spelt_for(Deed::Help), Some(lifts.spelt));
        for chord in [lifts.chord, Chord::Key(Key::Esc)] {
            assert_eq!(
                deed(Phase::Fresh, Focus::Overlay, chord),
                Some(Deed::CloseOverlay)
            );
        }
    }
}
