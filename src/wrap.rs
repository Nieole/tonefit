//! 折行：按**显示宽度**把一行摆不下的文字折成几行。宽字符两格，窄字符一格。
//!
//! 它在**界面层**，不进库——与 [`crate::render`] 同一侧：库那一头出数据，
//! 印在多宽的地方上是这一头的事。
//!
//! # 为什么非要自己一套
//!
//! 现成的那几套都按**空格**断词，而中文长句里一个空格都没有：整句因此是「一个词」，
//! 一行摆不下就原样印出去，窄终端上从行尾切掉（停车场 Q32／Q75）。
//! clap 的帮助是这样，会话屏底那一行也是这样——而屏底那一行尾巴上摆的是退出。
//!
//! # 三处共用这一份
//!
//! | 印在哪 | 折到多宽 | 谁折 |
//! |---|---|---|
//! | `--help` | [`TERMINAL_WIDTH`] 减掉 clap 加在外面的缩进 | `crate::folded_help` |
//! | 命令行印出来的报告 | [`TERMINAL_WIDTH`] | `crate::execute` |
//! | 会话屏底与报告区 | 那一格真有多宽 | `crate::session::draw` |
//!
//! 会话那两处折到**当场量得到的宽度**，前两处折到一个定死的数：命令行这一头不问终端有多宽
//! （问了，`tonefit --help > 说明.txt` 就随着窗口大小变），会话那一头每一帧都知道。
//!
//! 左栏与预设那一栏仍走终端库自己的折行：那两栏摆的是**标签加取值**，折在哪儿读起来都一样，
//! 而这一票收的是长句从行尾切掉。
//!
//! # 折出来有几行，当场数得出
//!
//! [`fold`] 出的就是折完的那几行，`len()` 即行数——会话的报告区靠它算滚动量，
//! 不必再往一块临时缓冲上画一遍再数（停车场 Q65）。
//!
//! # 三个名字，一件事
//!
//! [`fold`] 给**那几行**（`Vec<String>`），[`folded_text`] 把它们拼回**一段文字**，
//! `crate::session::draw::report::folded_lines` 拼成终端库的 `Line`。折法只有 [`fold`] 那一处，
//! 另外两个各只管把它交出来的东西装成调用方要的形状。

use unicode_width::UnicodeWidthChar;

/// 印在终端上的东西折到多宽为止。
///
/// 80 列是终端的老规矩，帮助与报告都宽过它读着更顺，100 格是常见的下一档。
/// **命令行那两处折到它**；会话那两处不用它——那一头量得到自己那一格真有多宽。
pub const TERMINAL_WIDTH: u16 = 100;

/// 这一行占几格。宽字符（中日韩、全角记号）两格，控制字符不占格，其余一格。
pub fn width(text: &str) -> u16 {
    let total: u32 = text.chars().map(|glyph| u32::from(cells(glyph))).sum();
    u16::try_from(total).unwrap_or(u16::MAX)
}

/// 把一段文字折成一行不超过 `width` 格的那几行。原文里的换行照旧是换行。
///
/// **空文字折出零行**：屏底那一格靠这一条分得开「没有话要说」与「说了一句空话」。
///
/// `width` 是零就一行不折——折不出比一个字更窄的行，硬折只会折出一堆空行。
/// 那种格子本来也画不出东西（会话在报告区那一格上真会遇到，见
/// `crate::session::draw::report::report_pane`）。
pub fn fold(text: &str, width: u16) -> Vec<String> {
    text.lines()
        .flat_map(|line| fold_line(line, width))
        .collect()
}

/// 折好之后拼回一段文字，每一行后面一个换行。
///
/// 命令行印报告用它：报告本来就是每一段都以换行收尾的一段文字
/// （见 [`crate::render::report`]），折过之后仍是。
pub fn folded_text(text: &str, width: u16) -> String {
    let mut out = String::with_capacity(text.len());
    for row in fold(text, width) {
        out.push_str(&row);
        out.push('\n');
    }
    out
}

/// 一行折成几行。至少一行——`width` 再小也不会折出空行来。
///
/// **行首那一截缩进跟着折下来的每一行走。** 报告里的次级行、`--help` 里挂圆点的那几条，
/// 靠的都是行首缩进说「这一截还是上一条」（停车场 Q32 立的就是这个缩进）——
/// 折下来的那一截缩不回去，条目的边界就没了。缩进宽过这一格时不留：那时留下的只有缩进。
fn fold_line(line: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return vec![line.to_owned()];
    }
    // 摆得下就一行不折。绝大多数行本来就摆得下（报告里逐页那几行、屏底那一行在宽终端上），
    // 不必为它们逐格走一遍。行尾那几格空白照旧去掉，与折下来的那几行同一个待遇。
    if self::width(line) <= width {
        return vec![line.trim_end_matches(' ').to_owned()];
    }
    let glyphs: Vec<(char, u16)> = line.chars().map(|glyph| (glyph, cells(glyph))).collect();
    let hanging = leading_spaces(line);
    let hanging = if hanging < usize::from(width) {
        hanging
    } else {
        0
    };
    let mut folded: Vec<String> = Vec::new();
    let mut start = 0;
    while start < glyphs.len() {
        // 断在空格上时，那几格空白留在上一行的行尾等于没有——这一行从下一个字起。
        // 行首那一截缩进不在此列：它落在第一行上，一格都不吃。
        let indent = if folded.is_empty() {
            0
        } else {
            while glyphs.get(start).is_some_and(|(glyph, _)| *glyph == ' ') {
                start += 1;
            }
            if start >= glyphs.len() {
                break;
            }
            hanging
        };
        let room = width - u16::try_from(indent).unwrap_or(0);
        let ends = ends(&glyphs, start, room);
        let mut row = " ".repeat(indent);
        row.extend(glyphs[start..ends].iter().map(|(glyph, _)| *glyph));
        // 断在空格前面时那几格空白落在这一行的行尾——屏上看不出来，写进文件里是噪声。
        folded.push(row.trim_end_matches(' ').to_owned());
        start = ends;
    }
    if folded.is_empty() {
        folded.push(String::new());
    }
    folded
}

/// 行首那一截缩进有几格。全是空格，格数就是字节数。
fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// 从第 `start` 格起的这一行断在哪一格**之前**。
///
/// 摆得下就是整行的末尾；摆不下就退到最后一处[断得开的地方](breakable)，
/// 一处都没有（一个长过这一格的西文词）就断在摆不下的那一格前面——
/// **每一行至少留一格**，因此这个数恒大于 `start`，折行不会原地打转。
fn ends(glyphs: &[(char, u16)], start: usize, width: u16) -> usize {
    let mut used: u32 = 0;
    let mut last: Option<usize> = None;
    for at in start..glyphs.len() {
        let (glyph, cells) = glyphs[at];
        if at > start && breakable(glyphs[at - 1].0, glyph) {
            last = Some(at);
        }
        if at > start && used + u32::from(cells) > u32::from(width) {
            return last.unwrap_or(at);
        }
        used += u32::from(cells);
    }
    glyphs.len()
}

/// 这两格之间断得开吗。
///
/// 两种断得开：**挨着一个空格**，以及**挨着一个宽字符**。
///
/// 前一种把**西文的词**照旧留住：`--fit`、`Ctrl-C`、`tonefit-calibration-….png` 各自
/// 整个留在一行上，词中间一格都不断。**`--fit height` 这种带空格的词组仍会断在那个空格上**——
/// 要它整个不断，得给折行一条「这个空格不许断」的记号，那是另一件事（停车场 Q106）。
/// 后一种是中文长句唯一的断处——它一个空格都没有，只能字与字之间断。
///
/// 反过来说，**两个窄字符之间断不开**：`←→`、`⇧⇥` 那几个箭头因此不会被劈成两半
/// （停车场 Q75 点名的正是它们）。一个宽字符自己也不会被劈开——断的是格与格之间，
/// 而一个字符是一格，它那两列跟着它走。
///
/// **断在最后一处断得开的地方**，不挑更早的那个空格：中文本来就是逐字断的，
/// 而挑更早的那个空格会在窄终端上白白让出好几格——那一格恰恰是窄的时候才要折。
fn breakable(before: char, after: char) -> bool {
    before == ' ' || after == ' ' || cells(before) == 2 || cells(after) == 2
}

/// 一个字符占几格。
fn cells(glyph: char) -> u16 {
    UnicodeWidthChar::width(glyph)
        .and_then(|wide| u16::try_from(wide).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 宽字符两格，窄字符一格，混着来的按格数加。
    #[test]
    fn a_wide_glyph_takes_two_cells() {
        assert_eq!(width(""), 0);
        assert_eq!(width("abc"), 3);
        assert_eq!(width("退出"), 4);
        assert_eq!(width("q 退出"), 6);
        // 箭头与间隔点在这套表上都是一格——屏底那一行的宽度按它们算。
        assert_eq!(width("←→"), 2);
        assert_eq!(width("·"), 1);
    }

    /// **中文长句折得开**：一个空格都没有的整句照旧按显示宽度折成几行，
    /// 不再原样印成一行等着从行尾切掉（票面第一条）。
    #[test]
    fn a_chinese_sentence_without_a_single_space_still_folds() {
        let sentence = "跑起来之前必填的两项是型号与输出根";

        let folded = fold(sentence, 10);

        assert!(folded.len() > 1, "整句还是一行：{folded:?}");
        for row in &folded {
            assert!(width(row) <= 10, "这一行 {} 格：{row}", width(row));
        }
        // 一个字都没吃掉。
        assert_eq!(folded.concat(), sentence);
    }

    /// **一个宽字符不会被折成两半**，那两格也不会分到两行上（票面第二条）。
    ///
    /// 折到奇数格时最咬人：一格空着也不许把下一个字劈开。
    #[test]
    fn a_wide_glyph_is_never_cut_in_half() {
        let folded = fold("退出会话", 3);

        assert_eq!(folded, ["退", "出", "会", "话"]);
        for row in &folded {
            assert!(width(row) <= 3, "{row}");
        }
    }

    /// **西文的词不被劈开**：断处只在空格上。带空格的词组（`--fit height`）仍断得开，
    /// 那一条见 [`breakable`] 的文档与停车场 Q106。
    #[test]
    fn a_latin_word_is_not_broken_at_a_space_that_is_not_there() {
        // `--fit height` 断得开的只有中间那个空格。
        assert_eq!(
            fold("换 --fit height 试试", 8),
            ["换 --fit", "height", "试试"]
        );
        // 长过这一格的西文词无处可断，只好硬断——切掉尾巴更坏。
        assert_eq!(
            fold("tonefit-calibration.png", 8),
            ["tonefit-", "calibrat", "ion.png"]
        );
    }

    /// **两个窄字符之间断不开**：`←→`、`⇧⇥` 那几个记号不会被劈成两半（停车场 Q75）。
    #[test]
    fn a_run_of_narrow_marks_stays_on_one_row() {
        for mark in ["←→", "⇧⇥", "↑↓"] {
            let folded = fold(&format!("按 {mark} 换一个"), 4);
            assert!(
                folded.iter().any(|row| row.contains(mark)),
                "{mark} 被劈开了：{folded:?}"
            );
        }
    }

    /// 断在空格上时那几格空白不带到下一行去。
    #[test]
    fn the_spaces_at_a_break_do_not_start_the_next_row() {
        assert_eq!(fold("aaa bbb", 3), ["aaa", "bbb"]);
    }

    /// **行首那一截缩进跟着折下来的每一行走**：折下来的那一截仍看得出是上一条的
    /// （停车场 Q32 立那个缩进就是为了这个）。
    #[test]
    fn an_indented_line_keeps_its_indent_on_every_row_it_folds_into() {
        assert_eq!(fold("  缓存 1 页", 6), ["  缓存", "  1 页"]);
        // 缩进宽过这一格时不跟下来——跟下来的话，折出的每一行都只剩缩进。
        assert_eq!(fold("    一二", 4), ["", "一二"]);
    }

    /// 原文里的换行照旧是换行，空行留着；整段空文字折出**零行**。
    #[test]
    fn the_line_breaks_in_the_source_survive() {
        assert_eq!(fold("一\n\n二", 10), ["一", "", "二"]);
        assert!(fold("", 10).is_empty(), "空文字该折出零行");
        assert_eq!(folded_text("一\n二", 10), "一\n二\n");
    }

    /// **零宽的格子问不出折行**：一行不折，也不打转。
    #[test]
    fn a_pane_without_a_single_column_folds_nothing() {
        assert_eq!(fold("一句话", 0), ["一句话"]);
    }

    /// 折出来有几行当场数得出来——报告区的滚动量靠它（停车场 Q65）。
    #[test]
    fn how_many_rows_it_folds_into_is_the_length_of_what_it_gives_back() {
        let text = "第一行\n第二行长一点，折得开";

        assert_eq!(fold(text, 6).len(), 5);
        assert_eq!(fold(text, 100).len(), 2);
    }
}
