//! 路径的**逐层补全**：只列打到的那一层。
//!
//! ADR 0009 关掉的正是它的反面：不递归、不建索引、不缓存，源库只读。
//! 因此这个模块只有一个动作——**照打到的那一层 `read_dir` 一次，用完就扔**。
//! 记不住任何东西是它的性质，不是它偷懒：一份索引一旦存下来就会过期，
//! 而「大库里补全一个路径」这件事本来就只要问一层。
//!
//! 列出来的东西**按用户打的写法拼回去**：分隔符照他敲的那一个（Windows 上两种都认），
//! 打到一半那一层**之前**的路径原样留着。补全不该顺手替他把路径重写一遍。
//!
//! **大小写按平台的规矩办**（见 [`CASE_INSENSITIVE`]）：不认大小写的文件系统上敲 `d`
//! 补得出 `Doraemon`，而补回来的是**盘上那个写法**——打到一半的那一截因此是补全
//! 唯一会改写的地方，改的也只有大小写。认大小写的平台上一个字都不放宽。

use std::path::Path;

/// 路径分隔符，两种都认——Windows 上用户敲哪一个的都有。
const SEPARATORS: [char; 2] = ['/', '\\'];

/// 本平台的文件系统认不认大小写。
///
/// Windows 不认：`D:\Comics` 与 `d:\comics` 是同一个目录，那一层点得开，
/// 打到一半的 `c` 因此也该筛得出 `Comics`。其余平台认——那儿两个只差大小写的目录
/// 是两个目录，一律折大小写来比会让它们互相污染（停车场 Q59 摆的两条路，走的是这一条）。
///
/// `cfg!` 是个**表达式**：两支在所有平台上都编译得到，模块里因此不必架 `#[cfg]` 那道墙。
const CASE_INSENSITIVE: bool = cfg!(windows);

/// 打到这儿时，**这一层**里对得上的有哪些。
///
/// 返回的每一项都是「可以直接替换掉当前缓冲」的完整写法：目录带上一个分隔符，
/// 再按一次 `Tab` 就下到那一层。这一层点不开（路径不在、权限不够）就是空清单——
/// 补全不是错误处理的地方，那件事在真开工时由预扫说。
pub fn level(typed: &str) -> Vec<String> {
    let (head, prefix) = split(typed);
    let directory = if head.is_empty() {
        Path::new(".")
    } else {
        Path::new(head)
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut listed: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !matches_prefix(&name, prefix) {
                return None;
            }
            let descend = entry.file_type().is_ok_and(|kind| kind.is_dir());
            Some(format!(
                "{head}{name}{}",
                if descend { separator(head) } else { "" }
            ))
        })
        .collect();
    listed.sort();
    listed
}

/// 一条补全项在**这一层里的那个名字**：前面的路径与末尾那个分隔符都去掉。
///
/// 与 [`level`] 拼出去的那一步是一对，因此和 [`split`] 共用同一份分隔符表——
/// 界面层要把候选摆成一行时不必自己再切一遍。
pub fn name(hit: &str) -> &str {
    let body = hit.strip_suffix(SEPARATORS).unwrap_or(hit);
    match body.rfind(SEPARATORS) {
        Some(at) => &body[at + 1..],
        None => body,
    }
}

/// 若干项共同的那一段。补到分岔口为止是补全该做的事，替用户从几项里挑一项不是。
pub fn common_prefix(listed: &[String]) -> Option<String> {
    let first = listed.first()?;
    let mut end = first.len();
    for other in &listed[1..] {
        end = end.min(shared(first, other));
    }
    Some(first[..end].to_owned())
}

/// 把打到一半的路径拆成「哪一层」与「这一层里的前缀」。
///
/// 分界是最后一个分隔符：`D:/库/哆啦` 拆成 `D:/库/` 与 `哆啦`，
/// `D:/库/` 拆成它自己与空前缀（那一层整层都对得上）。一个分隔符都没有的
/// 按当前目录那一层算。
fn split(typed: &str) -> (&str, &str) {
    match typed.rfind(SEPARATORS) {
        Some(at) => typed.split_at(at + 1),
        None => ("", typed),
    }
}

/// 这一层里的一个名字，对不对得上打到一半的那个前缀。
///
/// 认大小写的平台上就是 `str::starts_with`，**一个字都不放宽**；不认的那一支走
/// [`starts_with_folded`]。分岔的判据是 [`CASE_INSENSITIVE`]。
fn matches_prefix(name: &str, prefix: &str) -> bool {
    if CASE_INSENSITIVE {
        starts_with_folded(name, prefix)
    } else {
        name.starts_with(prefix)
    }
}

/// 折一次大小写再比前缀。
///
/// **逐字折**，不把两边整串 `to_lowercase` 了再比：一来那样每个候选都要新分配一个串，
/// 二来两者在「一个字折出好几个字」的那种字上分岔（土耳其语的 `İ` 折成 `i` 加一个组合点），
/// 逐字的那一种更**严**——敲 `i` 补不出 `İ`，而 Windows 自己那张表也是逐码点折的，
/// 不做多字折叠。
///
/// 它不看 [`CASE_INSENSITIVE`]，因此**每个平台上都测得到**。
fn starts_with_folded(name: &str, prefix: &str) -> bool {
    let mut listed = name.chars();
    prefix.chars().all(|typed| {
        listed
            .next()
            .is_some_and(|here| here == typed || here.to_lowercase().eq(typed.to_lowercase()))
    })
}

/// 补出来的目录后面挂哪一个分隔符：照用户这一层敲的那一个，没敲过就按本平台的。
fn separator(head: &str) -> &'static str {
    match head.chars().next_back() {
        Some('/') => "/",
        Some('\\') => "\\",
        _ if std::path::MAIN_SEPARATOR == '\\' => "\\",
        _ => "/",
    }
}

/// 两段文本从头共有多少个**字节**，且落在字符边界上。
fn shared(one: &str, other: &str) -> usize {
    one.char_indices()
        .zip(other.chars())
        .take_while(|((_, here), there)| here == there)
        .map(|((at, here), _)| at + here.len_utf8())
        .last()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一棵两层的树，返回它的根。
    fn tree() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("建临时目录");
        for volume in ["哆啦A梦 01", "哆啦A梦 02", "棋魂"] {
            let directory = root.path().join(volume);
            std::fs::create_dir(&directory).expect("建目录");
            // 下一层：补全**不该**列到它。
            std::fs::create_dir(directory.join("下一层")).expect("建目录");
            std::fs::write(directory.join("001.png"), b"x").expect("写文件");
        }
        std::fs::write(root.path().join("说明.txt"), b"x").expect("写文件");
        root
    }

    /// 打到哪一层就只列哪一层：下一层的东西一个都不出现（ADR 0009：不递归）。
    #[test]
    fn only_the_level_that_was_typed_into_is_listed() {
        let root = tree();
        let typed = format!("{}/", root.path().display());

        let listed = level(&typed);

        assert_eq!(listed.len(), 4, "{listed:?}");
        assert!(
            listed.iter().all(|hit| !hit.contains("下一层")),
            "补全递归到了下一层：{listed:?}"
        );
        assert!(
            listed.iter().all(|hit| !hit.contains("001.png")),
            "补全递归到了下一层：{listed:?}"
        );
        // 目录带分隔符，文件不带——再按一次 `Tab` 就下到那一层。
        assert!(
            listed.iter().any(|hit| hit.ends_with("棋魂/")),
            "{listed:?}"
        );
        assert!(
            listed.iter().any(|hit| hit.ends_with("说明.txt")),
            "{listed:?}"
        );
    }

    /// 前缀筛得动，而且补出来的是「共同的那一段」，不替用户挑。
    #[test]
    fn a_prefix_narrows_the_level_down_to_what_it_shares() {
        let root = tree();
        let typed = format!("{}/哆啦", root.path().display());

        let listed = level(&typed);
        let common = common_prefix(&listed).expect("有共同的那一段");

        assert_eq!(listed.len(), 2, "{listed:?}");
        assert!(common.ends_with("哆啦A梦 0"), "{common}");
        // 补到分岔口为止：两卷各自的号没有被替用户挑一个。
        assert!(!common.ends_with('1') && !common.ends_with('2'), "{common}");
    }

    /// **不缓存**：两次补全之间新建的东西，第二次就列得到（ADR 0009：不建索引）。
    #[test]
    fn nothing_is_remembered_between_two_completions() {
        let root = tree();
        let typed = format!("{}/", root.path().display());
        let before = level(&typed);

        std::fs::create_dir(root.path().join("新来的")).expect("建目录");
        let after = level(&typed);

        assert_eq!(after.len(), before.len() + 1, "{after:?}");
        assert!(after.iter().any(|hit| hit.contains("新来的")), "{after:?}");
    }

    /// 摆成一行时只留这一层里的那个名字，前面的路径与末尾的分隔符都去掉。
    #[test]
    fn a_hit_shows_as_the_name_it_has_in_this_level() {
        assert_eq!(name("D:/库/棋魂/"), "棋魂");
        assert_eq!(name(r"D:\库\棋魂\"), "棋魂");
        assert_eq!(name("D:/库/说明.txt"), "说明.txt");
        assert_eq!(name("棋魂"), "棋魂");
    }

    /// 点不开的那一层是空清单，不是恐慌——补全不是错误处理的地方。
    #[test]
    fn a_level_that_does_not_open_lists_nothing() {
        let root = tree();
        let missing = format!("{}/根本没这个目录/", root.path().display());

        assert!(level(&missing).is_empty());
        assert_eq!(common_prefix(&[]), None);
    }

    /// 分隔符照用户敲的那一个，前缀原样留着——补全不重写用户打的路径。
    ///
    /// 只在 Windows 上跑：反斜杠在别的平台上是文件名里的一个普通字符，不是分隔符。
    /// 「照敲的那一个还回来」在正斜杠上由
    /// [`only_the_level_that_was_typed_into_is_listed`] 一并验着。
    #[cfg(windows)]
    #[test]
    fn the_separator_the_user_typed_is_the_one_that_comes_back() {
        let root = tree();
        let backslash = format!("{}\\棋", root.path().display());

        let listed = level(&backslash);

        assert_eq!(listed.len(), 1, "{listed:?}");
        assert!(listed[0].ends_with("棋魂\\"), "{listed:?}");
        assert!(listed[0].starts_with(&root.path().display().to_string()));
    }

    /// 折法本身，**每个平台上都跑得到**：`starts_with_folded` 不看 `CASE_INSENSITIVE`，
    /// 看它的是 `matches_prefix`。
    #[test]
    fn folding_lets_the_other_case_through_and_nothing_else() {
        assert!(starts_with_folded("Doraemon 01", "d"));
        assert!(starts_with_folded("Doraemon 01", "DORAEMON"));
        assert!(starts_with_folded("哆啦A梦 01", "哆啦a"));
        assert!(!starts_with_folded("棋魂", "d"));
        // 前缀比名字长：够不着，不算对得上。
        assert!(!starts_with_folded("d", "doraemon"));
    }

    /// 大小写按平台的规矩筛：不认大小写的平台上敲 `d` 两个都补得出，**补回来的是盘上那个写法**；
    /// 认大小写的平台上只补得出敲对了的那一个——那一支一个字都没放宽。
    ///
    /// 一条用例问两边，不挂 `cfg`：两支的代码在所有平台上都编译得到（`cfg!` 是表达式），
    /// 分岔的只有断言。
    #[test]
    fn case_is_folded_only_where_the_file_system_folds_it() {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon 01")).expect("建目录");
        std::fs::create_dir(root.path().join("doraemon 02")).expect("建目录");
        let typed = format!("{}/d", root.path().display());

        let listed = level(&typed);

        if CASE_INSENSITIVE {
            assert_eq!(listed.len(), 2, "{listed:?}");
            assert!(
                listed.iter().any(|hit| hit.ends_with("Doraemon 01/")),
                "补回来的不是盘上那个写法：{listed:?}"
            );
        } else {
            assert_eq!(listed.len(), 1, "{listed:?}");
            assert!(listed[0].ends_with("doraemon 02/"), "{listed:?}");
        }
    }
}
