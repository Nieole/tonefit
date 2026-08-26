//! 标定图的点阵字模：一个字符 → 一块 16 行的位图。
//!
//! 判读说明要印在图上（14 号票），而这张图现在**含中文**（标定图批 01 号票）——
//! 印字就得有字模，字模就得随程序走，不能指望目标设备上有哪个字体。
//! 中文字模按 16×16 算，一句话就是几十个字形：手写不出来，也验不了，
//! 于是不手写——[`HEX`] 是从 GNU Unifont 摘出来的子集，一格都没有改过。
//!
//! 半宽（ASCII）8 格宽、全宽（汉字与全角标点）16 格宽，两种都是 16 行高。
//! 混排一行时按各自的宽度推进，不强行等宽：等宽会把 ASCII 撑成两倍空，
//! 而这张图上英文那几行正是最长的。

use std::sync::LazyLock;

/// 字模的行数。半宽与全宽共用它——两种字形只差在宽度上。
pub(super) const GLYPH_HEIGHT: u32 = 16;

/// 半宽字形的格数。ASCII 走这一档。
pub(super) const HALF_WIDTH: u32 = 8;

/// 全宽字形的格数。汉字与全角标点走这一档。
pub(super) const FULL_WIDTH: u32 = 16;

/// 一个字形：多宽，以及逐行的点阵。
///
/// 一行存成一个 `u16`，**最高位在左**——上游 `.hex` 就是这么排的，
/// 换成别的排法就得在解析时翻一遍，而翻错了图上看不出来（字形左右镜像仍是一团黑）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Glyph {
    width: u32,
    rows: [u16; GLYPH_HEIGHT as usize],
}

impl Glyph {
    /// 这个字形占几格。字距按它推进。
    pub(super) fn width(self) -> u32 {
        self.width
    }

    /// `(column, row)` 那一格点亮没有。越界一律当作没点亮。
    pub(super) fn lit(self, column: u32, row: u32) -> bool {
        if column >= self.width || row >= GLYPH_HEIGHT {
            return false;
        }
        let bits = self.rows[row as usize];
        (bits >> (self.width - 1 - column)) & 1 == 1
    }
}

/// `character` 的字模。表里没有的字符回 `None`——调用方画一个空框，
/// 「印不出来」这件事要看得见，而不是静静少一个字。
pub(super) fn glyph(character: char) -> Option<Glyph> {
    let table = table();
    table
        .binary_search_by_key(&character, |(listed, _)| *listed)
        .ok()
        .map(|index| table[index].1)
}

/// 字模表，按字符排好序，只解析一次。
///
/// 解析放在运行期而不是编译期：这张图一趟只画一遍，两百个字形的解析量可以忽略，
/// 而换来的是数据保持上游 `.hex` 的原样——比对上游、增删码位都不必碰 Rust 那一侧。
fn table() -> &'static [(char, Glyph)] {
    static TABLE: LazyLock<Vec<(char, Glyph)>> = LazyLock::new(|| {
        let mut table: Vec<(char, Glyph)> = HEX.lines().filter_map(parse).collect();
        table.sort_by_key(|(character, _)| *character);
        table
    });
    &TABLE
}

/// 解析 `.hex` 的一行：`码位:位图`。注释行、空行与不认得的行一律跳过。
///
/// 跳过而不是恐慌：这份数据是随程序走的常量，少解析出一条会被
/// `the_table_holds_every_line_of_the_asset` 当场抓住，
/// 而画图这条路上不该有一条因为字模而崩掉的分支。
fn parse(line: &str) -> Option<(char, Glyph)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (code, bitmap) = line.split_once(':')?;
    let character = char::from_u32(u32::from_str_radix(code, 16).ok()?)?;
    let width = match bitmap.len() as u32 / GLYPH_HEIGHT {
        2 => HALF_WIDTH,
        4 => FULL_WIDTH,
        _ => return None,
    };
    let digits = (width / 4) as usize;
    let mut rows = [0u16; GLYPH_HEIGHT as usize];
    for (row, cells) in rows.iter_mut().enumerate() {
        *cells = u16::from_str_radix(&bitmap[row * digits..(row + 1) * digits], 16).ok()?;
    }
    Some((character, Glyph { width, rows }))
}

/// 字模数据本身。格式、来源与许可写在文件头上。
const HEX: &str = include_str!("glyphs.hex");

#[cfg(test)]
mod tests {
    use super::*;

    /// 数据文件里每一条字模都进了表：解析静静丢掉一行，图上就静静缺一个字。
    #[test]
    fn the_table_holds_every_line_of_the_asset() {
        let entries = HEX
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .count();

        assert_eq!(table().len(), entries, "有字模没能解析出来");
    }

    /// 半宽只占左边 8 格，全宽占满 16 格。
    ///
    /// 位图存成 `u16` 而字形只有 8 格宽时，多出来的 8 位必须是高位那一半——
    /// 取反了字形就整体左移半格，而那种错在图上只显示成「字挨得有点近」。
    #[test]
    fn a_half_width_glyph_stays_inside_its_eight_cells() {
        let half = glyph('A').expect("ASCII 的 A 在表里");
        let full = glyph('图').expect("图 在表里");

        assert_eq!(half.width(), HALF_WIDTH);
        assert_eq!(full.width(), FULL_WIDTH);
        assert!(
            (0..GLYPH_HEIGHT).any(|row| (0..HALF_WIDTH).any(|column| half.lit(column, row))),
            "A 是一块空白"
        );
        assert!(
            !(0..GLYPH_HEIGHT).any(|row| half.lit(HALF_WIDTH, row)),
            "半宽字形越过了第 8 格"
        );
        assert!(
            (0..GLYPH_HEIGHT)
                .any(|row| (HALF_WIDTH..FULL_WIDTH).any(|column| full.lit(column, row))),
            "全宽字形右半边是空的"
        );
    }

    /// 除了空格，没有两个字形长得一模一样——印出来分不开的字等于没印。
    ///
    /// 一并挡住的是摘错行：同一条位图配了两个码位时，图上那两个字会是同一个样子。
    #[test]
    fn no_two_glyphs_look_alike() {
        let table = table();
        for (index, (character, bitmap)) in table.iter().enumerate() {
            for (other, other_bitmap) in &table[index + 1..] {
                assert_ne!(bitmap, other_bitmap, "「{character}」与「{other}」长得一样");
            }
        }
    }

    /// 空白只有空格那一个字符：别的字形一格都不亮的话，那一行数据是坏的。
    #[test]
    fn only_the_space_is_blank() {
        for (character, bitmap) in table() {
            let blank = (0..GLYPH_HEIGHT)
                .all(|row| (0..bitmap.width()).all(|column| !bitmap.lit(column, row)));
            assert_eq!(blank, *character == ' ', "「{character}」的字模是空的");
        }
    }

    /// 表里没有的字符回 `None`，调用方据此画空框。
    #[test]
    fn a_character_outside_the_table_has_no_glyph() {
        assert!(glyph('漫').is_none(), "这个字不在标定图印得出的集合里");
    }
}
