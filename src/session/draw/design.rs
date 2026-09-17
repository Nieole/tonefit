//! **设计快照**的读法与**逐格比对**（`session-redesign/02`；spec《设计快照是验收标准》）。
//!
//! 设计稿（`.scratch/session-redesign/design.html`）导出的每一屏是两张网格：**字**一张、
//! **样式**一张（每格一个代号，代号表在文件头）；样式代号译回设计稿的类名，类名再按
//! **样式对照表**（`tests/fixtures/design/styles.json`，只有这一张）译成前景色与修饰。
//! 顶栏的版本号导出成占位（占位是什么写在 `manifest.json` 上），读进来时代入 crate 的版本。
//!
//! 比的是**每一格的字、前景色、修饰**三样。现成的探针不够：[`super::probe::same_screen`]
//! 只比字，`OnScreen::colours` 是每行去重后的颜色集合。对不上时把实际那一屏的两张网格
//! 整个印出来——改的人照着它逐格看差在哪。
//!
//! **读屏的跳格规矩与探针是同一处**（[`super::probe::visible`]）：宽字符占两格，第二格
//! 被终端库 `reset` 成空格，按显示宽度跳过去；导出那一头按 `TestBackend` 的读法跳的是同一格，
//! 两张网格因此一格对一格。
//!
//! **快照对实现只读**：实现不许为了变绿去改快照（ADR 0019 决定第 13 条）。
//! 重新导出的办法写在导出脚本旁边（`.scratch/session-redesign/export.js`）。

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use super::probe::{cells_of, visible};

/// 导出的产物住在哪儿（相对仓库根）。
const FIXTURES: &str = "tests/fixtures/design";

/// 屏上**画出来的一格**：字、前景色、修饰。宽字符是一格（它占住的第二格已经跳过了）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Painted {
    pub(super) symbol: String,
    pub(super) fg: Color,
    pub(super) modifiers: Modifier,
}

/// 一屏的期望：一行一行，每行若干格（[`Painted`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::session) struct Expected {
    /// 哪一份（文件名去掉后缀），对不上时报出来。
    pub(super) name: String,
    rows: Vec<Vec<Painted>>,
}

impl Expected {
    /// 同一份快照在 `NO_COLOR` 下该是什么样：每一格的前景色退回终端默认色，字与修饰一格不动
    /// （`CONTEXT.md` 的《语义色》）。新界面那条不上色的用例拿它比（`session-redesign/06`）。
    pub(in crate::session) fn without_colour(mut self) -> Self {
        for glyph in self.rows.iter_mut().flatten() {
            glyph.fg = Color::Reset;
        }
        self
    }

    /// 期望屏上**抹掉一段**：设计稿在那儿写着一句话，而实现照一条仍然成立的规矩**不写它**。
    ///
    /// **抹掉之后那几格要求是空白**——它仍旧是一条断言，不是放过：实现在那儿多写一个字
    /// 照样红。`from` 与 `width` 是**第几列、几列**（不是第几个字：抹掉的那一段与补上的
    /// 空白占的列数相同，字数不同）。
    ///
    /// **每一处用它的地方都得在用例上写清是哪一条停车场条目**：它抹掉的是设计稿与实现
    /// 对不上的那一格，而那是要拍板的人先改设计稿、重新导出的（ADR 0019 决定第 13 条）。
    pub(in crate::session) fn blanked(mut self, row: usize, from: u16, width: u16) -> Self {
        let Some(line) = self.rows.get_mut(row) else {
            return self;
        };
        let mut kept: Vec<Painted> = Vec::with_capacity(line.len());
        let mut column = 0u16;
        for glyph in line.iter() {
            let cells = crate::wrap::width(&glyph.symbol);
            if column + cells <= from || column >= from + width {
                kept.push(glyph.clone());
            } else if column == from {
                kept.extend((0..width).map(|_| Painted {
                    symbol: " ".to_owned(),
                    fg: Color::Reset,
                    modifiers: Modifier::empty(),
                }));
            }
            column += cells;
        }
        *line = kept;
        self
    }

    /// **字网格**：一行一屏行，只有字、不带样式。场景夹具拿它核「报告那一处说出来的字
    /// 在设计快照上找得到」（`session-redesign/05`）——那一问只关字，不关颜色。
    pub(in crate::session) fn lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| row.iter().map(|glyph| glyph.symbol.as_str()).collect())
            .collect()
    }
}

/// 读快照要的两样：样式对照表（设计稿类名 → 颜色，修饰记号 → 修饰）与顶栏版本号的占位。
/// 两样各只有一处出处（`styles.json` 与 `manifest.json`），这里只读。
struct Fixtures {
    colours: HashMap<String, Color>,
    modifiers: HashMap<String, Modifier>,
    /// 顶栏版本号在快照里的占位；读进来时换成 crate 的版本，两者得**等宽**。
    placeholder: String,
}

/// 颜色名（`styles.json` 上写的）与终端库的 16 色一一对应；**两个方向都从这一张表走**。
const COLOURS: &[(&str, Color)] = &[
    ("reset", Color::Reset),
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("gray", Color::Gray),
    ("dark-gray", Color::DarkGray),
    ("light-red", Color::LightRed),
    ("light-green", Color::LightGreen),
    ("light-yellow", Color::LightYellow),
    ("light-blue", Color::LightBlue),
    ("light-magenta", Color::LightMagenta),
    ("light-cyan", Color::LightCyan),
    ("white", Color::White),
];

/// 修饰名与终端库的修饰一一对应，同上。
const MODIFIERS: &[(&str, Modifier)] = &[
    ("bold", Modifier::BOLD),
    ("dim", Modifier::DIM),
    ("italic", Modifier::ITALIC),
    ("underlined", Modifier::UNDERLINED),
    ("reversed", Modifier::REVERSED),
];

fn colour(name: &str) -> Color {
    COLOURS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, colour)| *colour)
        .unwrap_or_else(|| panic!("styles.json 上的颜色名认不出：{name}"))
}

fn colour_name(colour: Color) -> String {
    COLOURS
        .iter()
        .find(|(_, known)| *known == colour)
        .map_or_else(|| format!("{colour:?}"), |(name, _)| (*name).to_owned())
}

fn modifier(name: &str) -> Modifier {
    MODIFIERS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, modifier)| *modifier)
        .unwrap_or_else(|| panic!("styles.json 上的修饰名认不出：{name}"))
}

fn modifier_names(modifiers: Modifier) -> String {
    let names: Vec<&str> = MODIFIERS
        .iter()
        .filter(|(_, known)| modifiers.contains(*known))
        .map(|(name, _)| *name)
        .collect();
    if names.is_empty() {
        "无".to_owned()
    } else {
        names.join("+")
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES)
}

/// 读产物目录里的一份 JSON。
fn json(file: &str) -> serde_json::Value {
    let path = fixtures().join(file);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到 {}：{error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{file} 不是 JSON：{error}"))
}

impl Fixtures {
    /// 读那一张对照表与清单上的占位。
    fn load() -> Self {
        let table = json("styles.json");
        let manifest = json("manifest.json");
        let entries = |key: &str| -> Vec<(String, String)> {
            table[key]
                .as_object()
                .unwrap_or_else(|| panic!("styles.json 里没有「{key}」那一张"))
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        value.as_str().expect("对照表上的值是名字").to_owned(),
                    )
                })
                .collect()
        };
        Self {
            colours: entries("colours")
                .into_iter()
                .map(|(class, name)| (class, colour(&name)))
                .collect(),
            modifiers: entries("modifiers")
                .into_iter()
                .map(|(token, name)| (token, modifier(&name)))
                .collect(),
            placeholder: manifest["version_placeholder"]
                .as_str()
                .expect("manifest.json 上写着版本号的占位")
                .to_owned(),
        }
    }

    /// 一串类名（`c-gray d`）→ 前景色与修饰。
    fn style(&self, classes: &str) -> (Color, Modifier) {
        let mut fg = None;
        let mut modifiers = Modifier::empty();
        for token in classes.split(' ').filter(|token| !token.is_empty()) {
            if let Some(colour) = self.colours.get(token) {
                assert!(fg.is_none(), "「{classes}」里有两个颜色");
                fg = Some(*colour);
            } else if let Some(modifier) = self.modifiers.get(token) {
                modifiers |= *modifier;
            } else {
                panic!("「{classes}」里的「{token}」不在 styles.json 上");
            }
        }
        (fg.expect("每一格都有颜色（终端默认色也是一种）"), modifiers)
    }
}

/// 网格文件里的一行：两侧的引号去掉。不是一行的（代号表、空行）回 `None`。
fn grid_line(line: &str) -> Option<&str> {
    line.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
}

/// 把字网格与样式网格读成一屏的期望。
///
/// 两张网格一格对一格：字网格上第 i 个字与样式网格上第 i 个代号说的是同一格。
/// 顶栏的版本号占位在这里换成 crate 的版本。
fn parse(name: &str, text: &str, style: &str, fixtures: &Fixtures) -> Expected {
    let mut legend: HashMap<char, (Color, Modifier)> = HashMap::new();
    for line in style.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            let (code, classes) = rest
                .split_once(" = ")
                .unwrap_or_else(|| panic!("{name}：代号表上这一行认不出：{line}"));
            let mut codes = code.chars();
            let code = codes.next().expect("代号是一个字符");
            assert!(codes.next().is_none(), "{name}：代号不止一个字符：{code}");
            legend.insert(code, fixtures.style(classes));
        }
    }
    let text_rows: Vec<&str> = text.lines().filter_map(grid_line).collect();
    let style_rows: Vec<&str> = style.lines().filter_map(grid_line).collect();
    assert_eq!(
        text_rows.len(),
        style_rows.len(),
        "{name}：两张网格行数不同"
    );
    let mut rows: Vec<Vec<Painted>> = text_rows
        .iter()
        .zip(&style_rows)
        .enumerate()
        .map(|(y, (chars, codes))| {
            let chars: Vec<char> = chars.chars().collect();
            let codes: Vec<char> = codes.chars().collect();
            assert_eq!(
                chars.len(),
                codes.len(),
                "{name}：第 {y} 行两张网格格数不同"
            );
            chars
                .iter()
                .zip(&codes)
                .map(|(glyph, code)| {
                    let (fg, modifiers) = legend
                        .get(code)
                        .unwrap_or_else(|| panic!("{name}：第 {y} 行的代号「{code}」没有说明"));
                    Painted {
                        symbol: glyph.to_string(),
                        fg: *fg,
                        modifiers: *modifiers,
                    }
                })
                .collect()
        })
        .collect();
    if let Some(first) = rows.first_mut() {
        substitute_version(first, &fixtures.placeholder);
    }
    Expected {
        name: name.to_owned(),
        rows,
    }
}

/// 顶栏那一行：占位换成 crate 的版本，一格换一格。
///
/// 版本号变宽了（`0.10.0`）整条顶栏都会挪一格，那时快照得重新导出——这里只当场说出来，
/// 不替它挪。
fn substitute_version(row: &mut [Painted], placeholder: &str) {
    let placeholder: Vec<char> = placeholder.chars().collect();
    let version: Vec<char> = env!("CARGO_PKG_VERSION").chars().collect();
    let Some(at) = row.windows(placeholder.len()).position(|window| {
        window
            .iter()
            .zip(&placeholder)
            .all(|(glyph, c)| glyph.symbol.chars().eq(std::iter::once(*c)))
    }) else {
        return;
    };
    assert_eq!(
        version.len(),
        placeholder.len(),
        "crate 的版本 {} 与快照里的占位 {} 不等宽：顶栏整条挪了一格，快照要重新导出",
        env!("CARGO_PKG_VERSION"),
        placeholder.iter().collect::<String>()
    );
    for (glyph, c) in row[at..at + placeholder.len()].iter_mut().zip(&version) {
        glyph.symbol = c.to_string();
    }
}

/// 读一份快照或期望屏：`name` 是文件名去掉 `.text.txt`／`.style.txt`。
fn load(dir: &str, name: &str) -> Expected {
    let base = fixtures().join(dir).join(name);
    let read = |suffix: &str| {
        let path = base.with_file_name(format!("{name}.{suffix}"));
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("读不到 {}：{error}", path.display()))
    };
    parse(
        name,
        &read("text.txt"),
        &read("style.txt"),
        &Fixtures::load(),
    )
}

/// 某个场景某个尺寸的设计快照（`snapshots/<场景>.<宽x高>`）。
pub(in crate::session) fn snapshot(scene: &str, cols: u16, rows: u16) -> Expected {
    load("snapshots", &format!("{scene}.{cols}x{rows}"))
}

/// 某一串交互走完那一屏的期望（`sequences/<名字>`）。
pub(in crate::session) fn sequence(name: &str) -> Expected {
    load("sequences", name)
}

/// 从终端库的缓冲里逐格读回来，连同每一格在屏上是第几格；跳格走探针那一处（[`visible`]）。
fn painted(buffer: &Buffer) -> Vec<Vec<(usize, Painted)>> {
    let width = usize::from(buffer.area.width).max(1);
    buffer
        .content()
        .chunks(width)
        .map(|row| {
            visible(row)
                .map(|(column, cell)| {
                    (
                        column,
                        Painted {
                            symbol: cell.symbol().to_owned(),
                            fg: cell.fg,
                            modifiers: cell.modifier,
                        },
                    )
                })
                .collect()
        })
        .collect()
}

/// 从终端库的缓冲里逐格读回来：一行一行，每行若干格。
pub(super) fn read(buffer: &Buffer) -> Vec<Vec<Painted>> {
    painted(buffer)
        .into_iter()
        .map(|row| row.into_iter().map(|(_, cell)| cell).collect())
        .collect()
}

/// 哪一格差在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Differs {
    /// 行数不同。
    RowCount { actual: usize, expected: usize },
    /// 这一行的格数不同。
    RowLength { actual: usize, expected: usize },
    /// 字不同。
    Symbol { actual: String, expected: String },
    /// 前景色不同。
    Foreground { actual: Color, expected: Color },
    /// 修饰不同。
    Modifiers {
        actual: Modifier,
        expected: Modifier,
    },
}

/// 对不上：头一处差异，连同实际那一屏的两张网格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Mismatch {
    pub(super) name: String,
    /// 第几行（从 0 起）。
    pub(super) row: usize,
    /// 这一行的第几个字（从 0 起；宽字符算一个）。
    pub(super) glyph: usize,
    /// 屏上第几格（从 0 起；前面每个宽字符占两格）。
    pub(super) column: usize,
    pub(super) differs: Differs,
    grids: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match &self.differs {
            Differs::RowCount { actual, expected } => {
                format!("行数不同：实际 {actual} 行，期望 {expected} 行")
            }
            Differs::RowLength { actual, expected } => {
                format!("格数不同：实际 {actual} 格，期望 {expected} 格")
            }
            Differs::Symbol { actual, expected } => {
                format!("字不同：实际「{actual}」，期望「{expected}」")
            }
            Differs::Foreground { actual, expected } => format!(
                "前景色不同：实际 {}，期望 {}",
                colour_name(*actual),
                colour_name(*expected)
            ),
            Differs::Modifiers { actual, expected } => format!(
                "修饰不同：实际 {}，期望 {}",
                modifier_names(*actual),
                modifier_names(*expected)
            ),
        };
        write!(
            f,
            "与设计快照 {} 对不上：第 {} 行第 {} 格（这一行第 {} 个字）{what}\n实际画出来的两张网格：\n{}",
            self.name, self.row, self.column, self.glyph, self.grids
        )
    }
}

/// 实际那一屏的两张网格，与导出的写法同一副：字一张、样式一张（代号按首次出现编号，
/// 代号表在头上写的是颜色名与修饰名）。
pub(super) fn grids(rows: &[Vec<Painted>]) -> String {
    const CODES: &str = ".abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut legend: Vec<((Color, Modifier), char)> = vec![((Color::Reset, Modifier::empty()), '.')];
    let mut text = String::new();
    let mut style = String::new();
    for row in rows {
        text.push('"');
        style.push('"');
        for glyph in row {
            text.push_str(&glyph.symbol);
            let key = (glyph.fg, glyph.modifiers);
            let code = match legend.iter().find(|(known, _)| *known == key) {
                Some((_, code)) => *code,
                None => {
                    let code = CODES.chars().nth(legend.len()).unwrap_or('?');
                    legend.push((key, code));
                    code
                }
            };
            style.push(code);
        }
        text.push_str("\"\n");
        style.push_str("\"\n");
    }
    let mut out = text;
    for ((fg, modifiers), code) in legend {
        out.push_str(&format!(
            "# {code} = {} {}\n",
            colour_name(fg),
            modifier_names(modifiers)
        ));
    }
    out.push_str(&style);
    out
}

/// 逐格比：字、前景色、修饰。头一处不同就回 [`Mismatch`]。
pub(super) fn same_cells(actual: &Buffer, expected: &Expected) -> Result<(), Mismatch> {
    let rows = painted(actual);
    let mismatch = |row: usize, glyph: usize, column: usize, differs: Differs| Mismatch {
        name: expected.name.clone(),
        row,
        glyph,
        column,
        differs,
        grids: grids(&read(actual)),
    };
    if rows.len() != expected.rows.len() {
        return Err(mismatch(
            0,
            0,
            0,
            Differs::RowCount {
                actual: rows.len(),
                expected: expected.rows.len(),
            },
        ));
    }
    for (y, (got, want)) in rows.iter().zip(&expected.rows).enumerate() {
        for (i, ((at, a), e)) in got.iter().zip(want).enumerate() {
            let differs = if a.symbol != e.symbol {
                Some(Differs::Symbol {
                    actual: a.symbol.clone(),
                    expected: e.symbol.clone(),
                })
            } else if a.fg != e.fg {
                Some(Differs::Foreground {
                    actual: a.fg,
                    expected: e.fg,
                })
            } else if a.modifiers != e.modifiers {
                Some(Differs::Modifiers {
                    actual: a.modifiers,
                    expected: e.modifiers,
                })
            } else {
                None
            };
            if let Some(differs) = differs {
                return Err(mismatch(y, i, *at, differs));
            }
        }
        if got.len() != want.len() {
            // 比到头都一样、格数却不同：报在多出来／缺掉的头一格上，那一格在这一行末尾之后。
            let i = got.len().min(want.len());
            let column = got.get(i).map_or_else(
                || got.last().map_or(0, |(at, a)| at + cells_of(&a.symbol)),
                |(at, _)| *at,
            );
            return Err(mismatch(
                y,
                i,
                column,
                Differs::RowLength {
                    actual: got.len(),
                    expected: want.len(),
                },
            ));
        }
    }
    Ok(())
}

/// 逐格比，对不上就带着两张网格恐慌（用例用它）。
pub(in crate::session) fn assert_same_cells(actual: &Buffer, expected: &Expected) {
    if let Err(mismatch) = same_cells(actual, expected) {
        panic!("{mismatch}");
    }
}

/// **一个背景色都不设**（停车场 Q737；spec《颜色》）：逐格比对比不到背景色——设计稿表达不了它，
/// 快照上没有那一维——新画法的每一条快照与序列用例另问这一句：整屏每一格的背景都是终端默认色。
pub(in crate::session) fn assert_no_background(buffer: &Buffer) {
    let width = usize::from(buffer.area.width).max(1);
    for (i, cell) in buffer.content().iter().enumerate() {
        assert_eq!(
            cell.bg,
            Color::Reset,
            "第 {} 行第 {} 格设了背景色",
            i / width,
            i % width
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    /// 一张手写的期望：两行、几个字，样式照代号表。
    fn expected(text: &str, style: &str) -> Expected {
        parse("手写的", text, style, &Fixtures::load())
    }

    /// 一屏缓冲，逐段摆字与样式。
    fn buffer(width: u16, rows: &[&[(&str, Style)]]) -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, rows.len() as u16));
        for (y, row) in rows.iter().enumerate() {
            let mut x = 0;
            for (text, style) in *row {
                buffer.set_string(x, y as u16, text, *style);
                x += crate::wrap::width(text);
            }
        }
        buffer
    }

    /// **差一个字，红得出来、说得清差在哪一格。**
    #[test]
    fn a_different_glyph_is_caught_and_located() {
        let want = expected(
            "\"ab cd\"\n\"ef gh\"\n",
            "# . = c-fg\n# a = c-gray\n\"....a\"\n\".....\"\n",
        );
        let same = buffer(
            5,
            &[
                &[
                    ("ab c", Style::default()),
                    ("d", Style::default().fg(Color::DarkGray)),
                ],
                &[("ef gh", Style::default())],
            ],
        );
        assert_eq!(same_cells(&same, &want), Ok(()));
        // 用例里断言用的那一个：对得上就静静过去。
        assert_same_cells(&same, &want);

        let off = buffer(
            5,
            &[
                &[
                    ("ab c", Style::default()),
                    ("d", Style::default().fg(Color::DarkGray)),
                ],
                &[("ef gX", Style::default())],
            ],
        );
        let mismatch = same_cells(&off, &want).expect_err("差了一个字");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (1, 4, 4));
        assert_eq!(
            mismatch.differs,
            Differs::Symbol {
                actual: "X".to_owned(),
                expected: "h".to_owned()
            }
        );
        // 对不上时把实际那一屏的两张网格印出来。
        let said = mismatch.to_string();
        assert!(said.contains("\"ef gX\""), "{said}");
        assert!(said.contains("# a = dark-gray 无"), "{said}");
    }

    /// **差一个颜色，红得出来**：字一样、前景色不一样。
    #[test]
    fn a_different_foreground_is_caught_and_located() {
        let want = expected("\"ab\"\n", "# . = c-fg\n# a = c-yellow\n\".a\"\n");
        let off = buffer(
            2,
            &[&[
                ("a", Style::default()),
                ("b", Style::default().fg(Color::Red)),
            ]],
        );

        let mismatch = same_cells(&off, &want).expect_err("差了一个颜色");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (0, 1, 1));
        assert_eq!(
            mismatch.differs,
            Differs::Foreground {
                actual: Color::Red,
                expected: Color::Yellow
            }
        );
        assert!(
            mismatch.to_string().contains("实际 red，期望 yellow"),
            "{mismatch}"
        );
    }

    /// **少一个修饰，红得出来**：字与颜色都一样、少了加粗。多一个修饰同理（那时实际那一侧多）。
    #[test]
    fn a_missing_modifier_is_caught_and_located() {
        let want = expected("\"ab\"\n", "# . = c-fg\n# a = c-fg b d\n\".a\"\n");
        let off = buffer(
            2,
            &[&[
                ("a", Style::default()),
                ("b", Style::default().add_modifier(Modifier::DIM)),
            ]],
        );

        let mismatch = same_cells(&off, &want).expect_err("少了一个修饰");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (0, 1, 1));
        assert_eq!(
            mismatch.differs,
            Differs::Modifiers {
                actual: Modifier::DIM,
                expected: Modifier::BOLD | Modifier::DIM
            }
        );
        assert!(
            mismatch.to_string().contains("实际 dim，期望 bold+dim"),
            "{mismatch}"
        );

        let same = buffer(
            2,
            &[&[
                ("a", Style::default()),
                (
                    "b",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
            ]],
        );
        assert_eq!(same_cells(&same, &want), Ok(()));
    }

    /// **宽字符的后半格跳过去**：一个汉字是一格，它占住的第二格不比、也不算格数；
    /// 报出来的「第几格」按屏上的格算（前面每个宽字符占两格），「第几个字」按字算。
    #[test]
    fn the_second_half_of_a_wide_glyph_is_skipped_on_both_sides() {
        let want = expected("\"设备x\"\n", "# . = c-fg\n\"...\"\n");
        let same = buffer(5, &[&[("设备x", Style::default())]]);
        assert_eq!(same_cells(&same, &want), Ok(()));
        // 读回来的那一行是三个字，不是五格。
        assert_eq!(
            read(&same)[0]
                .iter()
                .map(|glyph| glyph.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["设", "备", "x"]
        );

        // 第二个汉字换了：这一行第 1 个字、屏上第 2 格。
        let off = buffer(5, &[&[("设各x", Style::default())]]);
        let mismatch = same_cells(&off, &want).expect_err("差了一个汉字");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (0, 1, 2));
        assert_eq!(
            mismatch.differs,
            Differs::Symbol {
                actual: "各".to_owned(),
                expected: "备".to_owned()
            }
        );

        // 汉字后面那个字换了：这一行第 2 个字、屏上第 4 格——前面两个汉字占了四格。
        let off = buffer(5, &[&[("设备y", Style::default())]]);
        let mismatch = same_cells(&off, &want).expect_err("差了一个字");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (0, 2, 4));

        // 期望是两个窄字、实际是一个宽字：字对不上，格数也对不上。
        let want = expected("\"abx\"\n", "# . = c-fg\n\"...\"\n");
        let mismatch = same_cells(&same, &want).expect_err("宽窄不同");
        assert_eq!(
            mismatch.differs,
            Differs::Symbol {
                actual: "设".to_owned(),
                expected: "a".to_owned()
            }
        );
    }

    /// **行数或格数对不上也红**：屏比期望矮、一行比期望短，各说各的。
    #[test]
    fn a_short_screen_or_a_short_row_is_caught() {
        let want = expected("\"ab\"\n\"cd\"\n", "# . = c-fg\n\"..\"\n\"..\"\n");
        let short = buffer(2, &[&[("ab", Style::default())]]);
        assert_eq!(
            same_cells(&short, &want).expect_err("少一行").differs,
            Differs::RowCount {
                actual: 1,
                expected: 2
            }
        );
        let narrow = buffer(1, &[&[("a", Style::default())], &[("c", Style::default())]]);
        let mismatch = same_cells(&narrow, &want).expect_err("少一格");
        assert_eq!((mismatch.row, mismatch.glyph, mismatch.column), (0, 1, 1));
        assert_eq!(
            mismatch.differs,
            Differs::RowLength {
                actual: 1,
                expected: 2
            }
        );
    }

    /// **一条用例读完全部快照、场景数据与期望屏，格式都认得**（票面第五条）。
    ///
    /// 快照与期望屏：两张网格读得成一屏，行数与格数与那一份的尺寸对得上，每一格的类名
    /// 都在对照表上；顶栏那一行代入了 crate 的版本。场景数据：读得成 JSON，几处非有不可的
    /// 字段都在。清单（`manifest.json`）说有的每一份都在盘上，盘上的每一份清单里也都有。
    #[test]
    fn every_exported_snapshot_scene_and_sequence_reads_back() {
        let manifest = json("manifest.json");
        let snapshots = manifest["snapshots"].as_array().expect("快照清单");
        let sequences = manifest["sequences"].as_array().expect("序列清单");
        assert_eq!(
            snapshots.len(),
            11 * 2 + 2,
            "11 个场景 × 两种尺寸外加窗口太小两份"
        );
        assert!(!sequences.is_empty());

        let size_in = |entry: &serde_json::Value, key: &str| -> (u16, u16) {
            let size = entry[key].as_array().expect("尺寸");
            (
                size[0].as_u64().expect("列数") as u16,
                size[1].as_u64().expect("行数") as u16,
            )
        };
        let fits = |expected: &Expected, (cols, rows): (u16, u16)| {
            assert_eq!(
                expected.rows.len(),
                usize::from(rows),
                "{}：行数",
                expected.name
            );
            for (y, row) in expected.rows.iter().enumerate() {
                let width: usize = row.iter().map(|cell| cells_of(&cell.symbol)).sum();
                assert_eq!(
                    width,
                    usize::from(cols),
                    "{}：第 {y} 行的宽度",
                    expected.name
                );
            }
        };
        let scene_data = |path: &str| {
            let data: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(fixtures().join(path))
                    .unwrap_or_else(|error| panic!("读不到 {path}：{error}")),
            )
            .unwrap_or_else(|error| panic!("{path} 不是 JSON：{error}"));
            for key in [
                "scene",
                "now_ms",
                "time_multiplier",
                "output",
                "paths",
                "settings",
                "presets",
                "run",
                "session",
            ] {
                assert!(data.get(key).is_some(), "{path} 里没有「{key}」");
            }
            // 序列走完那一刻与起点场景一样的那几样写成 `"unchanged"`（拿起点场景那一份补上）。
            if let Some(run) = data["run"].as_object() {
                for key in [
                    "mode",
                    "stage",
                    "elapsed_s",
                    "steps",
                    "total_steps",
                    "survey",
                    "volumes",
                ] {
                    assert!(run.contains_key(key), "{path} 的 run 里没有「{key}」");
                }
            }
        };

        let mut seen_versions = 0;
        for entry in snapshots {
            let scene = entry["scene"].as_str().expect("场景名");
            let (cols, rows) = size_in(entry, "size");
            let expected = snapshot(scene, cols, rows);
            fits(&expected, (cols, rows));
            if cols >= 60 && rows >= 16 {
                let top: String = expected.rows[0]
                    .iter()
                    .map(|glyph| glyph.symbol.as_str())
                    .collect();
                assert!(
                    top.contains(&format!("tonefit {}", env!("CARGO_PKG_VERSION"))),
                    "{scene}：顶栏没代入版本：{top}"
                );
                seen_versions += 1;
            }
            if let Some(data) = entry["data"].as_str() {
                scene_data(data);
            }
        }
        assert_eq!(seen_versions, 11 * 2);
        for entry in sequences {
            let name = entry["name"].as_str().expect("序列名");
            let expected = sequence(name);
            // 走完那一屏多大（换尺寸那几串与起点不同）。
            fits(&expected, size_in(entry, "final_size"));
            assert!(
                entry["steps"]
                    .as_array()
                    .is_some_and(|steps| !steps.is_empty()),
                "{name}：没有步"
            );
            scene_data(entry["data"].as_str().expect("序列走完那一刻的场景数据"));
        }

        // 盘上的每一份清单里都有：多出来的一份就是没人认领的一份。
        let listed: std::collections::HashSet<String> = snapshots
            .iter()
            .map(|entry| {
                let (cols, rows) = size_in(entry, "size");
                format!(
                    "{}.{cols}x{rows}",
                    entry["scene"].as_str().unwrap_or_default()
                )
            })
            .chain(
                sequences
                    .iter()
                    .map(|entry| entry["name"].as_str().unwrap_or_default().to_owned()),
            )
            .collect();
        for dir in ["snapshots", "sequences"] {
            for file in fs::read_dir(fixtures().join(dir)).expect("目录在") {
                let file = file
                    .expect("读得到")
                    .file_name()
                    .to_string_lossy()
                    .into_owned();
                let name = file
                    .trim_end_matches(".text.txt")
                    .trim_end_matches(".style.txt")
                    .trim_end_matches(".scene.json");
                assert!(listed.contains(name), "{dir}/{file} 不在 manifest.json 里");
            }
        }
    }
}
