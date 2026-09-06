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
//! | `--help` | [`terminal_width`] 减掉 clap 加在外面的缩进 | `crate::folded_help` |
//! | 命令行印出来的报告 | [`terminal_width`] | `crate::execute` |
//! | 会话屏上那几格 | 那一格真有多宽 | `crate::session::draw` |
//!
//! **三处折到的都是当场量得到的宽度**：会话每一帧问自己那一格，命令行那两处问终端。
//! 命令行这一头从前折到一个定死的 100 格，理由是「问了，`tonefit --help > 说明.txt`
//! 就随着窗口大小变」——那条理由只在**产出不落在终端上**的那几趟成立，
//! [`terminal_width`] 因此把定值缩小到那几趟（停车场 Q105）。
//!
//! **屏上没有第二套折行规矩。** 左栏与预设那一栏从前走终端库自己的 `Wrap`
//! （`p2-loose-ends/07` 的决定，理由是那两栏摆的是标签加取值、折在哪儿读起来都一样），
//! `p4-parking-lot/02` 推翻了它：那两栏的行数要交给视口（`crate::session::viewport`），
//! 而终端库折出来几行这一头数不出来——视口因此数的是逻辑行，窄档上「内容已截、滚动条却不画」
//! （停车场 Q104／Q136）。
//!
//! # 折出来有几行，当场数得出
//!
//! [`fold`] 出的就是折完的那几行，`len()` 即行数——会话那几格靠它算滚动量，
//! 不必再往一块临时缓冲上画一遍再数（停车场 Q65）。
//!
//! # 三个名字，一件事
//!
//! [`fold`] 给**那几行**（`Vec<String>`），[`folded_text`] 把它们拼回**一段文字**，
//! 会话那一头的 `Painted::folded` 拼成终端库的 `Line`（一行一种语义色）。
//! 折法只有 [`fold`] 那一处，另外两个各只管把它交出来的东西装成调用方要的形状。

use std::borrow::Cow;
use std::io::IsTerminal;

use unicode_width::UnicodeWidthChar;

/// **输出不是终端时**印出去的东西折到多宽为止。
///
/// 80 列是终端的老规矩，帮助与报告都宽过它读着更顺，100 格是常见的下一档。
/// 它是个**定值**：重定向到文件、接进管道、以及问不出终端有多宽的那几趟都折到它，
/// 产出因此与窗口大小无关（见 [`terminal_width`]）。
pub const OFF_TERMINAL_WIDTH: u16 = 100;

/// **印出去的东西折到多宽为止：问终端。**
///
/// 问的是 **stdout**——命令行那两处（帮助与报告）都印在它上面。
///
/// **它不是终端就取 [`OFF_TERMINAL_WIDTH`]。** `tonefit --help > 说明.txt` 与
/// `tonefit … > 报告.txt` 的产出因此一个字节都不随窗口大小变。**是终端、却问不出宽度**
/// 的也取那个定值：探宽度走的是 `TIOCGWINSZ`（Windows 上是控制台那一份），
/// 答不上来的终端有的是。
///
/// **问出来多窄就折到多窄，不设下限。** 设一个下限只会让窄终端上多出溢出：折到 40 格
/// 印在一块 20 格的屏上，那 20 格照旧要靠终端自己**回绕**（回绕不从行尾切字，
/// 停车场 Q105 记的就是这笔账），而报告本来可以正好折进去。
/// 窄到 clap 那一档缩进以下时帮助仍旧过宽——那一头折行够不着，记在停车场 Q186。
///
/// **一趟只问一次**（`crate::execute`）：帮助与报告因此折到同一个数，
/// 中途改窗口大小也不会让同一趟里的两段各折各的。
pub fn terminal_width() -> u16 {
    // **两问缺一不可**：`is_terminal` 答的是「该不该问」（重定向出去的产出不许随窗口变），
    // `size_checked` 答的是「问不问得出来」。前者是这一条规矩本身，
    // 不靠 `console` 顺手替我们答。
    if !std::io::stdout().is_terminal() {
        return OFF_TERMINAL_WIDTH;
    }
    // `console` 本来就在树里（indicatif 的进度条走的是它），这一行只是把它变成本仓库
    // 自己说得出口的一条依赖——与 `unicode-width` 那一条同一个道理，见 `Cargo.toml`。
    console::Term::stdout()
        .size_checked()
        .map_or(OFF_TERMINAL_WIDTH, |(_, columns)| columns)
}

/// 这个字形在**哪种终端上都占同一格**吗——**靠宽度对齐的那几格能不能对齐，问的就是它**
/// （停车场 Q154、Q168）。
///
/// 东亚宽度表上标着 **Ambiguous** 的字形（`–` `—` `…` `·` `×` 之类）在按 CJK 配置的
/// 终端上画两格、在西文终端上画一格，而 [`width`] 一律按一格算：一行上多一个这样的字形，
/// 它右边每一列就整体错开一格，同一列因此逐行参差。
///
/// **判据是「两套算法答得一样」**：[`width`] 把歧义宽度算一格，`width_cjk` 算两格，
/// 两者相等的字形与终端怎么配无关。
///
/// **管得着两层**，各有用例钉着，添一个不稳的字形当场变红：
///
/// - **画法那一层自己造的字形**——`crate::session::columns` 的省略号与两张表的行首记号；
/// - **措辞那一层摆进列里的那几格**——`crate::render` 的尺寸、判据那一串与基准档分布
///   （`p4-parking-lot/05` 把管辖面扩到这一层，从前它们划在规矩外面）。
///
/// 它只在用例里问得着：印出去的那一头一律按 [`width`] 算，这一条管的是**选字形**那一步。
#[cfg(test)]
pub fn width_is_stable(glyph: char) -> bool {
    UnicodeWidthChar::width(glyph) == UnicodeWidthChar::width_cjk(glyph)
}

/// **不许断的那个空格。** 印出来仍是一个普通空格，折行不在它上面断。
///
/// `--fit height` 这样带空格的记号断开之后抄不出一条能用的命令（停车场 Q106），
/// 而**折行这一头看不出来**：`换 --fit height 试试` 里的三个空格长得一模一样。
/// 分得开的只有写那句话的人——因此这是一层**标注**：措辞那一层（[`crate::render`]）
/// 与各条帮助原文把记号**里面**那个空格写成它，别的空格照旧断得开。
///
/// **标注的规矩只有这一处。** 怎么写：`format!("换 --fit{HARD_SPACE}inside 能……")`——
/// 帮助原文里那几条同样只在这一处取这个字符（文档注释收不下运行期算出来的串时，
/// 照 `crate::inputs_help` 那一条的办法把它写成一个函数）。
///
/// **印出去之前一律过 [`printed`]**，字节因此一个都没变。折行那几处由 [`fold`] 顺手做了，
/// **不折行的那一处得自己过**——拒绝执行那句话直接落在 stderr 上（`crate::main` 那一行
/// `eprintln!`），一格都没折。漏了它，用户照着抄那条命令 clap 认不出那个开关。
///
/// **记号宽过一整行时仍旧硬断**：那时一行一格都不剩，与一个长过这一格的西文词同一个待遇
/// （见 [`ends`]）——切掉尾巴更坏。断在它上面时**行尾那个空格照旧去掉**：
/// 走到那一步说明这一行连整个记号都摆不下，标注已经不成立，留一个吊在行尾的空格只是噪声。
pub const HARD_SPACE: char = '\u{a0}';

/// **禁则**：这几个收尾记号不落在行首。
///
/// 中文排版的老规矩，与 [`NEVER_ENDS_A_ROW`] 合成一张表——**表只有这一处**。
/// 断处的判据（[`breakable`]）只看「挨着空格还是挨着宽字符」，而
/// `……默认（跟随面板）` 断得开的地方里恰好有一处把 `）` 顶到下一行的行首
/// （停车场 Q103）。
///
/// **退一格仍不成立就接着退。** 禁则挡住的那一格**根本不算断处**，而 [`ends`] 记的是
/// **最后一处**断得开的地方——挡住一处它自然落到更早的那一处上，`）））` 连着三个也一样。
/// **退到一处都不剩时认下**：那时断在摆不下的那一格前面（[`ends`] 里那个 `unwrap_or`），
/// 记号落在行首。每一行至少留一格，因此退不动时既不打转、也不悄悄丢字。
/// 换句话说，**禁则是一条偏好，不是一条硬约束**：让得出去就让，让不出去宁可破例。
///
/// **半角那几个也在表上**：断处只落在挨着空格或宽字符的地方，因此 `中文)` 这种
/// 半角收尾记号照样落得到行首——同一条禁则，两种字形。
///
/// **间隔点 `·` 不在表上**：屏底那一行拿它当分隔，两侧各有一个空格，
/// 断在哪一侧读起来都成句。
const NEVER_STARTS_A_ROW: &[char] = &[
    '。', '，', '、', '；', '：', '？', '！', '）', '】', '》', '」', '』', '〉', '〕', '”', '’',
    '…', ')', ']', '}', ',', '.', ';', ':', '?', '!',
];

/// **禁则**：这几个起首记号不落在行尾。见 [`NEVER_STARTS_A_ROW`]（同一张表的另一半）。
const NEVER_ENDS_A_ROW: &[char] = &[
    '（', '【', '《', '「', '『', '〈', '〔', '“', '‘', '(', '[', '{',
];

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
///
/// **交出来的那几行是[印出去的样子](printed)**：原文里那几个[不许断的空格](HARD_SPACE)
/// 在这里已经换回普通空格，折到多宽、折不折得动都不影响这一条。
pub fn fold(text: &str, width: u16) -> Vec<String> {
    text.lines()
        .flat_map(|line| fold_line(line, width))
        .collect()
}

/// 折好之后拼回一段文字，每一行后面一个换行。
///
/// 命令行印报告用它：报告本来就是每一段都以换行收尾的一段文字
/// （见 [`crate::render::plain::report`]），折过之后仍是。
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
///
/// **缩进之后不先折出一个空行**（停车场 Q114）：见函数末尾那一段。
fn fold_line(line: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return vec![printed(line).into_owned()];
    }
    // 摆得下就一行不折。绝大多数行本来就摆得下（报告里逐页那几行、屏底那一行在宽终端上），
    // 不必为它们逐格走一遍。行尾那几格空白照旧去掉，与折下来的那几行同一个待遇。
    if self::width(line) <= width {
        return vec![printed(line).trim_end_matches(' ').to_owned()];
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
        folded.push(printed(&row).trim_end_matches(' ').to_owned());
        start = ends;
    }
    // **缩进之后不先折出一个空行**（停车场 Q114）。行首那一截缩进里面每一格都挨着空格、
    // 因此每一格都断得开，而缩进后面跟着一长串断不开的字（报告里那几张清单的形状：
    // 两格缩进加一条没有空格的长路径）时，**最后一处断得开的地方就落在缩进里面**——
    // 第一行于是只剩那几格空白，`trim_end` 之后成了一个空行，清单前面凭空多一行。
    //
    // 只有第一行长得出这个样子：后面每一行开头那几格空白在上面就跳过了。缩进本身宽过
    // 这一格时（那时它一格内容都摆不下）折出来的同样是它，一并在这里丢掉。
    // 原文里本来就空着的那一行不走这一支——它在上面「摆得下就一行不折」那里就回去了。
    if folded.len() > 1 && folded[0].is_empty() {
        folded.remove(0);
    }
    if folded.is_empty() {
        folded.push(String::new());
    }
    folded
}

/// 这段文字**印出去的样子**：[不许断的那个空格](HARD_SPACE)换回一个普通空格。
///
/// 标注是给折行看的，不是印出去的东西。[`fold`] 交出来的每一行都已经过了这一层；
/// **不走折行的那一处自己过**——拒绝执行那句话直接落在 stderr 上
/// （`crate::main` 那一行 `eprintln!`），漏了它用户照着抄的命令里就带着一个
/// clap 认不出的字符。
///
/// 一个标注都没有时借着原文回去，不白抄一遍——绝大多数行本来就一个都没有。
pub fn printed(text: &str) -> Cow<'_, str> {
    if text.contains(HARD_SPACE) {
        Cow::Owned(text.replace(HARD_SPACE, " "))
    } else {
        Cow::Borrowed(text)
    }
}

/// 行首那一截缩进有几格。全是空格，格数就是字节数。
fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// 从第 `start` 格起的这一行断在哪一格**之前**。
///
/// 摆得下就是整行的末尾；摆不下就退到最后一处[断得开的地方](breakable)，
/// 一处都没有就断在摆不下的那一格前面——**每一行至少留一格**，因此这个数恒大于 `start`，
/// 折行不会原地打转。
///
/// 一处都没有有三种来路，走的是同一条退路（`unwrap_or`）：一个长过这一格的西文词、
/// 一个长过这一格的[带空格的记号](HARD_SPACE)、以及[禁则](NEVER_STARTS_A_ROW)
/// 把这一行上仅有的那几处全挡住了。
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
/// 整个留在一行上，词中间一格都不断。带空格的**词组**（`--fit height`）要整个不断，
/// 得由写那句话的人把中间那个空格写成[不许断的那个空格](HARD_SPACE)——它在这里一律不算断处
/// （停车场 Q106）。后一种是中文长句唯一的断处——它一个空格都没有，只能字与字之间断。
///
/// 反过来说，**两个窄字符之间断不开**：`←→`、`⇧⇥` 那几个箭头因此不会被劈成两半
/// （停车场 Q75 点名的正是它们）。一个宽字符自己也不会被劈开——断的是格与格之间，
/// 而一个字符是一格，它那两列跟着它走。
///
/// **两头那几个记号让开**：收尾记号不落行首、起首记号不落行尾，表与「退不动怎么办」
/// 见[禁则](NEVER_STARTS_A_ROW)（停车场 Q103）。
///
/// **断在最后一处断得开的地方**，不挑更早的那个空格：中文本来就是逐字断的，
/// 而挑更早的那个空格会在窄终端上白白让出好几格——那一格恰恰是窄的时候才要折。
fn breakable(before: char, after: char) -> bool {
    if before == HARD_SPACE || after == HARD_SPACE {
        return false;
    }
    if NEVER_STARTS_A_ROW.contains(&after) || NEVER_ENDS_A_ROW.contains(&before) {
        return false;
    }
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

    /// **西文的词不被劈开**：断处只在空格上。带空格的词组（`--fit height`）没标注过时
    /// 仍断得开，标注过的那一条见
    /// [`a_marked_space_inside_a_token_is_never_a_break`]。
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
        // 那一格上第一行只剩缩进，因此它本身也不留（见
        // [`an_indent_before_an_unbreakable_run_does_not_fold_out_a_blank_row`]）。
        assert_eq!(fold("    一二", 4), ["一二"]);
    }

    /// **缩进之后不先折出一个空行**（停车场 Q114）。
    ///
    /// 报告里那几张清单的形状是「两格缩进 + 一条一个空格都没有的长路径」：缩进里面每一格
    /// 都挨着空格、因此每一格都断得开，而路径里一处都断不开——最后一处断得开的地方于是
    /// 落在缩进里面，第一行只剩那两格空白，清单前面凭空多一行。
    #[test]
    fn an_indent_before_an_unbreakable_run_does_not_fold_out_a_blank_row() {
        let folded = fold("  /home/alex/library/volume-a/001.jpg", 12);

        assert_eq!(
            folded,
            ["  /home/alex", "  /library/v", "  olume-a/00", "  1.jpg"]
        );
        // 缩进本身把这一格填满时同样不折出那一行——那一行一个字都没有。
        assert_eq!(fold("    一二", 4), ["一二"]);
        // 原文里本来就空着的那一行照旧是一行空行，不在这一条的管辖之内。
        assert_eq!(fold("一\n\n二", 4), ["一", "", "二"]);
    }

    /// **收尾记号不落行首、起首记号不落行尾**（停车场 Q103）。
    ///
    /// 第一条是左栏 34 列那一档上真长出来过的样子：`（跟随面板）` 折下来时 `）` 独占
    /// 下一行的行首（`p4-parking-lot/02` 记的那一句）。禁则挡住那一处之后，
    /// 折行退到更早的那一处上——**代价是行尾白让出两格**，那正是禁则要买的东西。
    #[test]
    fn a_closing_mark_never_starts_a_row() {
        assert_eq!(
            fold("  感知可分辨级数　默认（跟随面板）", 32),
            ["  感知可分辨级数　默认（跟随面", "  板）"]
        );
        // 两半各一处：断在 `（` 后面同样不许——那一行的行尾会剩一个张着口的括号。
        assert_eq!(fold("面板（跟随）", 6), ["面板", "（跟", "随）"]);
    }

    /// **退一格仍不成立就接着退，退到一处都不剩时认下**（停车场 Q103 那一问）。
    ///
    /// `）））` 连着三个时每一处都被禁则挡着，这一行上一处断得开的地方都不剩：
    /// 那时断在摆不下的那一格前面，记号落在行首。**每一行至少留一格**，
    /// 因此既不打转、也不悄悄丢字。
    #[test]
    fn a_row_that_cannot_retreat_any_further_gives_the_rule_up() {
        let folded = fold("一）））", 2);

        assert_eq!(folded, ["一", "）", "）", "）"]);
        // 一个字都没吃掉。
        assert_eq!(folded.concat(), "一）））");
    }

    /// **标注过的那个空格不断**：`--fit height` 抄得出一条能用的命令（停车场 Q106）。
    ///
    /// 折出来的每一行是**印出去的样子**——标注在这里已经换回一个普通空格，
    /// 无论那一行折没折过。
    #[test]
    fn a_marked_space_inside_a_token_is_never_a_break() {
        let marked = format!("换 --fit{HARD_SPACE}height 试试");

        // 没标注时断在中间那个空格上，标注过之后整个记号让到下一行去。
        assert_eq!(
            fold("换 --fit height 试试", 14),
            ["换 --fit", "height 试试"]
        );
        assert_eq!(fold(&marked, 14), ["换", "--fit height", "试试"]);
        // 一行摆得下时同样换回普通空格——那一行根本没走折行那一路。
        assert_eq!(fold(&marked, 20), ["换 --fit height 试试"]);
        // 标注不占地方：它与一个普通空格一样宽。
        assert_eq!(width(&marked), width("换 --fit height 试试"));
        // **记号宽过一整行时仍旧硬断**，而断在标注上时行尾那个空格照旧去掉：
        // 那一行连整个记号都摆不下，标注已经不成立（见 [`HARD_SPACE`]）。
        assert_eq!(
            fold(&format!("--fit{HARD_SPACE}height"), 6),
            ["--fit", "height"]
        );
    }

    /// **印出去的样子只有一处答得出**：不折行的那一处（拒绝那句话落到 stderr 上）
    /// 走的就是它，见 [`printed`] 与 `crate::main`。
    #[test]
    fn a_line_that_never_gets_folded_still_prints_a_plain_space() {
        let marked = format!("改得动的是几何：--fit{HARD_SPACE}height 把这一页放大到面板高");

        assert_eq!(
            printed(&marked),
            "改得动的是几何：--fit height 把这一页放大到面板高"
        );
        // 一个标注都没有时借着原文回去，不白抄一遍。
        assert!(matches!(printed("一句没有标注的话"), Cow::Borrowed(_)));
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
