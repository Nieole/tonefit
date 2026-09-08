//! 字形约定：**库出的字里**，哪个空格不许断、哪个字形宽度稳（`CONTEXT.md`《字形约定》）。
//!
//! 两条，都在这里，**都是库的公共 API**：[`HARD_SPACE`] 与 [`width_is_stable`]。
//!
//! # 为什么这两条在库里，不在界面层
//!
//! 它们量的是**界面层的需要**——列要对齐、命令要抄得出来——而**库得先守住**：
//! 同一句话从三张嘴里出来，最后一张在库内（ADR 0016 认下的那处例外）。
//! 库因此会把自己造的字**直接送进一条对齐的列里**（`裁边 A ⟶ B` 与 `缩放比 X ⋅ 预缩 N x`
//! 就是逐页表上的两列，见 `crate::Crop` 与 `crate::Scaling`），也会把一条**要用户照着抄**
//! 的命令写进一句拒绝里（`crate::Interlock`）。
//!
//! 规矩留在界面层、由 bin 事后改字形，是把约束漏进库里却不把规矩递过去：
//! 库这一头看不见那条列有多宽，只看得见自己写下的是哪个字符——**挑字形这一步在这里，
//! 那条规矩就得在这里**。
//!
//! # 折法不在这里
//!
//! 折到多宽、断在哪儿、禁则、缩进——那一整套在界面层（`src/wrap.rs`，理由写在它自己的
//! 模块文档上）。**两个去处量的不是同一块地方**：折行量的是「这一格真有多宽」，
//! 当场问终端或问那一格；这两条量的是「写下去的是哪个字符」，与印在多宽的地方上无关。
//!
//! [`HARD_SPACE`] 因此只是一层**标注**：把它换回一个普通空格是折行那一头的事
//! （`wrap::printed`），库这一头出的一律是带标注的原文。

use unicode_width::UnicodeWidthChar;

/// **不许断的那个空格。** 印出来仍是一个普通空格，折行不在它上面断。
///
/// `--fit height` 这样带空格的记号断开之后抄不出一条能用的命令（停车场 Q106），
/// 而**折行那一头看不出来**：`换 --fit height 试试` 里的三个空格长得一模一样。
/// 分得开的只有写那句话的人——因此这是一层**标注**：库这一侧（[`crate::Interlock`]
/// 那三句、几何门那条拒绝）与界面层各条帮助原文，都把记号**里面**那个空格写成它，
/// 别的空格照旧断得开。
///
/// **标注的规矩只有这一处。** 怎么写：`format!("换 --fit{HARD_SPACE}inside 能……")`；
/// 收不下运行期算出来的串的地方（`match` 出静态串的那几处）照原样写 `\u{a0}`，
/// 说的是同一个字符。
///
/// **印出去之前一律过 `wrap::printed`**，字节因此一个都没变——折行那几处由 `wrap::fold`
/// 顺手做了，**不折行的那一处得自己过**（拒绝那句话直接落在 stderr 上）。漏了它，
/// 用户照着抄那条命令 clap 认不出那个开关（`tests/exit_code.rs` 钉着这一条）。
pub const HARD_SPACE: char = '\u{a0}';

/// 这个字形在**哪种终端上都占同一格**吗——**靠宽度对齐的那几格能不能对齐，问的就是它**
/// （停车场 Q154、Q168、Q187）。
///
/// 东亚宽度表上标着 **Ambiguous** 的字形（`–` `—` `…` `·` `×` `→` 之类）在按 CJK 配置的
/// 终端上画两格、在西文终端上画一格，而界面层的 `wrap::width` 一律按一格算：
/// 一行上多一个这样的字形，它右边每一列就整体错开一格，同一列因此逐行参差。
///
/// **判据是「两套算法答得一样」**：`width` 把歧义宽度算一格，`width_cjk` 算两格，
/// 两者相等的字形与终端怎么配无关。宽字符（中日韩、全角记号）两边都是两格，**照样过关**——
/// 这一条问的是「稳不稳」，不是「窄不窄」。
///
/// **管得着两层**，各有用例钉着，添一个不稳的字形当场变红：
///
/// - **库自己造的字形**——本模块那条穷举的用例逐个扫库内每一个 `Display`；
/// - **界面层自己造的字形**——`crate::render` 摆进列里的那几格、`session::columns`
///   的省略号与三张表的行首记号，各在自己那一头问。
///
/// 它只在挑字形那一步问得着：印出去的那一头一律按 `wrap::width` 算。
pub fn width_is_stable(glyph: char) -> bool {
    UnicodeWidthChar::width(glyph) == UnicodeWidthChar::width_cjk(glyph)
}

#[cfg(test)]
mod tests {
    //! 库自己造的字形，**逐个过宽度那一关**。
    //!
    //! 扫的是**源文件本身**，不是某个运行时的取值：新写一个 `Display` 不必先有人把它
    //! 登记到一张名单上——扫描自己找得到它（同一副做法见 `tests/single_source.rs`）。

    use std::fs;
    use std::path::{Path, PathBuf};

    use super::width_is_stable;

    /// **成句的那几份不问字形**：它们装的是整句话，画它的那一头把它当整段文字折行
    /// （`CONTEXT.md`《行》），错一格不牵连别人；而中文行文里的破折号 `——`
    /// 是句读，不是摆进列里的记号，换掉它换的是这句话怎么读。
    ///
    /// **只放开眼下真要放开的那一个。** 再写一份成句的 `Display` 就得在这里加一行——
    /// 那一步是**存心的**，正是这条名单要买的东西。
    const SENTENCES: &[&str] = &["Interlock"];

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_else(|why| panic!("读 {} 失败：{why}", path.display()))
    }

    /// 库的源文件：`lib.rs` 自己，加上它 `mod` 出来的每一个（挂在目录上的连同整棵）。
    ///
    /// **单子从 `lib.rs` 那几行 `mod` 上读出来**，不在这里另抄一份：库里添一个模块，
    /// 它自动进扫描面——抄一份的话，新模块要等有人想起来才被问到。
    /// 二进制那一侧（`main.rs`、`render.rs`、`wrap.rs`、`session/`）不在里面：
    /// 那一头自己造的字形在它自己那几处问。
    fn library_sources() -> Vec<PathBuf> {
        let lib = root().join("src/lib.rs");
        let declarations = without_comments(&read(&lib));
        let mut files = vec![lib];
        for line in declarations.lines() {
            let Some(name) = line
                .trim()
                .strip_prefix("mod ")
                .and_then(|rest| rest.strip_suffix(';'))
            else {
                continue;
            };
            let single = root().join("src").join(format!("{name}.rs"));
            if single.is_file() {
                files.push(single);
            }
            let folder = root().join("src").join(name);
            if folder.is_dir() {
                collect(&folder, &mut files);
            }
        }
        files
    }

    fn collect(dir: &Path, into: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|why| panic!("读 {} 失败：{why}", dir.display()));
        // 目录序不定，排一遍：报出来的那一处才不随文件系统变。
        let mut here: Vec<PathBuf> = entries.map(|entry| entry.expect("目录项").path()).collect();
        here.sort();
        for path in here {
            if path.is_dir() {
                collect(&path, into);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                into.push(path);
            }
        }
    }

    /// 把注释抹掉，字面原样留着。
    ///
    /// 抹掉是为了后面两步都只看得见代码：`Display for` 只在真的实现上出现（文档里那些
    /// 指路不算），花括号也数得准。字面留着——要问的正是它们。
    ///
    /// 认得出四样：行注释、块注释（Rust 的块注释**嵌套**）、字符串、原始字符串。
    fn without_comments(source: &str) -> String {
        let glyphs: Vec<char> = source.chars().collect();
        let mut out = String::with_capacity(source.len());
        let mut at = 0;
        while at < glyphs.len() {
            let here = glyphs[at];
            let next = glyphs.get(at + 1).copied();
            match here {
                '/' if next == Some('/') => {
                    while at < glyphs.len() && glyphs[at] != '\n' {
                        at += 1;
                    }
                }
                '/' if next == Some('*') => {
                    let mut depth = 1;
                    at += 2;
                    while at < glyphs.len() && depth > 0 {
                        if glyphs[at] == '/' && glyphs.get(at + 1) == Some(&'*') {
                            depth += 1;
                            at += 2;
                        } else if glyphs[at] == '*' && glyphs.get(at + 1) == Some(&'/') {
                            depth -= 1;
                            at += 2;
                        } else {
                            at += 1;
                        }
                    }
                }
                'r' if matches!(next, Some('"') | Some('#')) => {
                    let hashes = glyphs[at + 1..].iter().take_while(|&&ch| ch == '#').count();
                    if glyphs.get(at + 1 + hashes) != Some(&'"') {
                        out.push(here);
                        at += 1;
                        continue;
                    }
                    let closing: String = std::iter::once('"')
                        .chain(std::iter::repeat_n('#', hashes))
                        .collect();
                    let from = at;
                    at += hashes + 2;
                    while at < glyphs.len()
                        && !glyphs[at..].starts_with(&closing.chars().collect::<Vec<_>>()[..])
                    {
                        at += 1;
                    }
                    at = (at + hashes + 1).min(glyphs.len());
                    out.extend(&glyphs[from..at]);
                }
                '"' => {
                    let from = at;
                    at += 1;
                    while at < glyphs.len() && glyphs[at] != '"' {
                        at += if glyphs[at] == '\\' { 2 } else { 1 };
                    }
                    at = (at + 1).min(glyphs.len());
                    out.extend(&glyphs[from..at]);
                }
                _ => {
                    out.push(here);
                    at += 1;
                }
            }
        }
        out
    }

    /// 一个文件里每一个 `Display` 实现：**类型名**加上**它体内那一段**。
    ///
    /// **跟在后面的必须是一个类型名**（首字母大写的标识符）。注释已经抹掉了，剩下会撞上
    /// `Display for ` 这几个字的只有**字符串字面**——本模块自己那条用例的报错文案就写着
    /// ``impl Display for {name}``，后面跟的是 `{`，不是类型名。少了这一道，那几处会被当成
    /// 三个类型名为空的实现，把底下 `seen.len()` 那道自检虚抬三格。
    fn display_impls(source: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find("Display for ") {
            let after = &rest[at + "Display for ".len()..];
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            let named = name.starts_with(|ch: char| ch.is_ascii_uppercase());
            let Some(open) = after.find('{') else { break };
            if named {
                found.push((name, braced(&after[open..]).to_owned()));
            }
            rest = &after[open..];
        }
        found
    }

    /// 从开头那个 `{` 数到与它配对的 `}`。字符串里的花括号不算数。
    fn braced(from: &str) -> &str {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (at, glyph) in from.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if glyph == '\\' {
                    escaped = true;
                } else if glyph == '"' {
                    in_string = false;
                }
                continue;
            }
            match glyph {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &from[..=at];
                    }
                }
                _ => {}
            }
        }
        from
    }

    /// 这一段里写下的那些字形：字符串字面与字符字面里的，`\u{…}` 那种写法一并解开——
    /// 拿转义绕过这一关的话这条用例就只是在装样子。
    fn glyphs_written(body: &str) -> Vec<char> {
        let source: Vec<char> = body.chars().collect();
        let mut written = Vec::new();
        let mut at = 0;
        while at < source.len() {
            let quote = source[at];
            // **字符字面只认 `'x'` 与 `'\…'` 两种形状。** 一个单引号后面跟着标识符的
            // 是**生命周期**（`Formatter<'_>` 这种），把它当字面的开头，扫描会一路吞到
            // 下一个单引号为止——中间那些真的字符串字面就此看不见了。
            let char_literal = quote == '\''
                && (source.get(at + 2) == Some(&'\'') || source.get(at + 1) == Some(&'\\'));
            if quote != '"' && !char_literal {
                at += 1;
                continue;
            }
            at += 1;
            while at < source.len() && source[at] != quote {
                if source[at] != '\\' {
                    written.push(source[at]);
                    at += 1;
                    continue;
                }
                at += 1;
                // **转义也解开**：`"\u{2192}"` 印出去的与 `"→"` 是同一个字符。
                if source.get(at) == Some(&'u') {
                    let digits: String = source[at + 2..]
                        .iter()
                        .take_while(|&&ch| ch != '}')
                        .collect();
                    if let Some(glyph) = u32::from_str_radix(&digits, 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        written.push(glyph);
                    }
                    at += digits.len() + 3;
                } else {
                    at += 1;
                }
            }
            at += 1;
        }
        written
    }

    /// **库自己造的字形一个都不许是歧义宽度**（`CONTEXT.md`《字形约定》第二条）。
    ///
    /// 扫的是库里**每一个** `Display`：下一个在这里写 `Display` 的人写进一个歧义宽度的
    /// 字形，这一条当场红——不必先有人把新类型登记到哪张名单上。放开的只有
    /// [`SENTENCES`] 那几份成句的。
    ///
    /// **扫描面就是 `impl Display`，不含库里那几条成句的错误消息**
    /// （`crate::dither_outside_the_gate_error` 的破折号、撞车那一句的 `←`、
    /// 预扫那一句的 `……`）。那几句与 [`SENTENCES`] 同一个理由——整段文字折行，
    /// 错一格不牵连别人；扫描面要不要连它们一起罩，记在停车场 Q337。
    ///
    /// **三件事一起问**，少一件这一条就问不出话来：扫得到实现、扫得到字面、字形逐个过关。
    /// 只问末一件的话，扫描裂了（`Display for` 找不着、花括号数错）这一条照样是绿的。
    #[test]
    fn every_glyph_the_library_writes_is_the_same_width_on_any_terminal() {
        let mut seen: Vec<String> = Vec::new();
        let mut with_text = 0usize;
        for path in library_sources() {
            let source = without_comments(&read(&path));
            for (name, body) in display_impls(&source) {
                seen.push(name.clone());
                let written = glyphs_written(&body);
                if written.iter().any(|glyph| !glyph.is_ascii()) {
                    with_text += 1;
                }
                if SENTENCES.contains(&name.as_str()) {
                    continue;
                }
                for glyph in written {
                    assert!(
                        width_is_stable(glyph),
                        "{glyph}（U+{:04X}）是东亚歧义宽度：{} 的 `impl Display for {name}` 写着它",
                        u32::from(glyph),
                        path.display()
                    );
                }
            }
        }
        // 扫描本身还活着：库里的 `Display` 是几十个的量级，字面也真的读到了。
        assert!(
            seen.len() >= 20,
            "只扫到 {} 个 Display，扫描裂了",
            seen.len()
        );
        assert!(
            with_text >= 10,
            "只有 {with_text} 个 Display 读出了字面，扫描裂了"
        );
        // 放开的那几个名字没写错——写错一个字母，这条名单就是空转的。
        for sentence in SENTENCES {
            assert!(
                seen.iter().any(|name| name == sentence),
                "{sentence} 不在库里，名单上这一行是空转的"
            );
        }
    }
}
