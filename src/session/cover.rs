//! **覆盖层**与它上面那一张**全部按键**（`CONTEXT.md` 的《会话》：覆盖层；spec《按键表、屏底与覆盖层》）。
//!
//! 一个键掀开、盖在视图上的那一张：`?` 是全部按键，打字时是 `F1`；掀着的时候底下整屏压暗，
//! 关掉之后底下原样回来（它盖住一块焦点，不替掉它——输入行、缓冲与补全框都还在）。
//! 说明卡随备注行那一票添进 [`Overlay`]。
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

use tonefit::{BitDepth, Candidate, Dither};

use super::keymap::{Deed, Group, Phase, Row, TABLE};
use super::look::{Kind, Look, Segment};
use super::view::{Views, Window};

/// 盖在视图上的那一张。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// 全部按键；`from` 是从第几行画起（模块文档《它记的是从第几行画起》）。
    Keys { from: usize },
}

/// 全部按键那一张至多多宽（设计稿 `drawHelp` 的 `104`）。
const WIDEST: u16 = 104;
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

    /// 关掉盖着的那一张；底下原样回来。
    pub fn drop_cover(&mut self) {
        self.cover = None;
    }

    /// 覆盖层上滚一步：`j`／`k` 一行、`gg`／`G` 到顶到底，起点收在那一张真摆得下的那一段里
    /// （到顶为止、到底为止）。这件事不是滚动、或者没掀着那一张，什么都不做、交回 `false`。
    /// 半屏与一屏那四个随每页结果那一票接上。
    pub fn scroll_cover(&mut self, deed: Deed, sheet: &Sheet) -> bool {
        let Some(Overlay::Keys { from }) = &mut self.cover else {
            return false;
        };
        *from = match deed {
            Deed::Down => sheet.from(*from + 1),
            Deed::Up => sheet.from(from.saturating_sub(1)),
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
        assert!(views.scroll_cover(Deed::Up, &sheet));
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }));
        views.scroll_cover(Deed::Down, &sheet);
        assert_eq!(views.cover, Some(Overlay::Keys { from: 1 }));
        assert!(!views.scroll_cover(Deed::Help, &sheet), "不是滚动的事不管");
        assert_eq!(views.cover, Some(Overlay::Keys { from: 1 }));
        views.scroll_cover(Deed::Bottom, &sheet);
        assert_eq!(
            views.cover,
            Some(Overlay::Keys {
                from: sheet.last_from()
            })
        );
        views.scroll_cover(Deed::Down, &sheet);
        assert_eq!(
            views.cover,
            Some(Overlay::Keys {
                from: sheet.last_from()
            }),
            "到底为止"
        );
        views.scroll_cover(Deed::Top, &sheet);
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }));
        let roomy = Sheet::of(Phase::Fresh, wide());
        views.scroll_cover(Deed::Down, &roomy);
        assert_eq!(views.cover, Some(Overlay::Keys { from: 0 }), "装得下就不滚");
        views.drop_cover();
        assert_eq!(views.cover, None);
        assert!(!views.scroll_cover(Deed::Down, &sheet), "没掀着就不管");
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
