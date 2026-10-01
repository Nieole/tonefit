//! 路径的**逐层补全**：只列打到的那一层。
//!
//! ADR 0009 关掉的正是它的反面：不递归、不建索引、不缓存，源库只读。
//! 因此这个模块只有一个动作——**照打到的那一层 `read_dir` 一次，用完就扔**。
//! 记不住任何东西是它的性质，不是它偷懒：一份索引一旦存下来就会过期，
//! 而「大库里补全一个路径」这件事本来就只要问一层。**只有一格例外**，见下。
//!
//! 列出来的东西**按用户打的写法拼回去**：分隔符照他敲的那一个（Windows 上两种都认），
//! 打到一半那一层**之前**的路径原样留着。补全不该顺手替他把路径重写一遍。
//!
//! **大小写按这一层所在的文件系统的规矩办，而那件事是运行期探出来的**——那个概念叫
//! **大小写敏感性**，含义由 `CONTEXT.md` 的《会话》定，这里只说它在本模块怎么落地：
//! 不认大小写的文件系统上敲 `d` 补得出 `Doraemon`，而补回来的是**盘上那个写法**——
//! 打到一半的那一截因此是补全唯一会改写的地方，改的也只有大小写。
//! 认大小写的那一档上一个字都不放宽。
//!
//! **探法在库里**（[`tonefit::case_sensitivity`]）：一趟开工时收编、借住、撞名认「是不是同一处」
//! 问的是同一个探法（`CONTEXT.md` 的《同一处》），屏上补得出的名字与跑起来认的名字因此按同一把尺子。
//! 它只读——拿一个已有的名字翻一次大小写去问盘。
//! **一个进程只探一次**——那一格就是上面说的那个例外，而它记的不是这一层有什么，
//! 是**这台机器的文件系统怎么比名字**，后者不会在两次 `Tab` 之间变（见 [`asked_once`]）。
//! 记不记、记多久是这里的事，库里那个探法问一次付一次。
//!
//! 按平台常量猜是这里从前的病：Windows 不认大小写，**macOS 默认那套也不认**，
//! 一个 `cfg!(windows)` 在 macOS 上恰好全错——那台机器上敲小写补不出大写开头的卷名。

use std::path::Path;
use std::sync::OnceLock;

use tonefit::CaseSensitivity;

/// 路径分隔符，两种都认——Windows 上用户敲哪一个的都有。输入行拆「哪一层」与「打到一半的那一截」、
/// 删一段、认一条候选是不是文件夹，都读这一份（`super::typing`），不另抄。
pub(super) const SEPARATORS: [char; 2] = ['/', '\\'];

/// 打到这儿时，**这一层**里对得上的有哪些。
///
/// 返回的每一项都是「可以直接替换掉当前缓冲」的完整写法：目录带上一个分隔符，
/// 再按一次 `Tab` 就下到那一层。这一层点不开（路径不在、权限不够）就是空清单——
/// 补全不是错误处理的地方，那件事在真开工时由清点说。
pub fn level(typed: &str) -> Vec<String> {
    level_asking_case(typed, remembered)
}

/// [`level`] 本身，**大小写那条判定依据由调用方给**。
///
/// 画质分从外面交进来，[`level`] 那一路才有一个够得着的缝：探测问的是**跑着的这台机器**，
/// 而两边的行为（macOS 折得开、Linux 不放宽）得在**每台**机器上都验得到——
/// 用例自己再按平台分岔一次，就是把本模块刚治好的那个病又犯一遍。
///
/// 这个缝交进来的是**答案**，不是探法：探法本身没有缝，在跑用例的那个文件系统上真验
/// （库里 `place` 那几条用例）。
fn level_asking_case(typed: &str, ask: impl FnOnce(&Path) -> CaseSensitivity) -> Vec<String> {
    let (head, prefix) = split(typed);
    let directory = if head.is_empty() {
        Path::new(".")
    } else {
        Path::new(head)
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    // 整层先收下来再筛。名字与条目各留一份，为的是 `file_type()` **仍旧只对筛得中的那几项问**——
    // 并进上面那一步的话，每一个条目都要摊上一次（`d_type` 答不出的文件系统上那是一次 stat）。
    let read: Vec<std::fs::DirEntry> = entries.flatten().collect();
    let names: Vec<String> = read
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let sensitivity = ask(directory);
    let mut listed: Vec<String> = read
        .iter()
        .zip(&names)
        .filter(|(_, name)| matches_prefix(name, prefix, sensitivity))
        .map(|(entry, name)| {
            let descend = entry.file_type().is_ok_and(|kind| kind.is_dir());
            format!("{head}{name}{}", if descend { separator(head) } else { "" })
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
/// 认大小写的那一档就是 `str::starts_with`，**一个字都不放宽**；不认的那一支走
/// [`starts_with_folded`]。分岔的依据是 [`CaseSensitivity`]，**由调用方交进来**。
fn matches_prefix(name: &str, prefix: &str, sensitivity: CaseSensitivity) -> bool {
    match sensitivity {
        CaseSensitivity::Insensitive => starts_with_folded(name, prefix),
        CaseSensitivity::Sensitive => name.starts_with(prefix),
    }
}

/// 真问文件系统那一路：探一次、记一格。探的是库里那一个（[`tonefit::case_sensitivity`]）。
///
/// 记的是**一个进程一格**，不是一个挂载点一格——大小写敏感性其实是挂载点的性质
/// （macOS 上挂得出大小写敏感的 APFS，Linux 上挂得出 ciopfs），跨挂载点补全会拿到
/// 头一处那个答案。一张会长大的表连同它的失效问题正是 ADR 0009 关掉的那件事的小号版本，
/// 而答错那一边只是多列或少列几项候选，一个字节都不写（停车场 Q364）。
fn remembered(directory: &Path) -> CaseSensitivity {
    asked_once(&KNOWN, || tonefit::case_sensitivity(directory))
}

/// 这台机器那一格。**整个进程共用它**，因此摆在模块这一层、不摆在 [`remembered`] 里面——
/// 摆在里面的一格与摆在这里的一格，除了这一条外的每一条用例都分辨不出来。
static KNOWN: OnceLock<CaseSensitivity> = OnceLock::new();

/// 探不出来时按哪一边：**严的那一边**。
const UNPROVEN: CaseSensitivity = CaseSensitivity::Sensitive;

/// 探一次并记住：那一格落下来之后**不再问第二次**。
fn asked_once(
    known: &OnceLock<CaseSensitivity>,
    ask: impl FnOnce() -> Option<CaseSensitivity>,
) -> CaseSensitivity {
    if let Some(known) = known.get() {
        return *known;
    }
    match ask() {
        Some(answer) => *known.get_or_init(|| answer),
        None => UNPROVEN,
    }
}

/// 折一次大小写再比前缀。
///
/// **逐字折**，不把两边整串 `to_lowercase` 了再比：一来那样每个候选都要新分配一个串，
/// 二来两者在「一个字折出好几个字」的那种字上分岔（土耳其语的 `İ` 折成 `i` 加一个组合点），
/// 逐字的那一种更**严**——敲 `i` 补不出 `İ`，而 Windows 自己那张表也是逐码点折的，
/// 不做多字折叠。
///
/// 它不看 [`CaseSensitivity`]——看它的是 [`matches_prefix`]，因此**每个平台上都测得到**。
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

    /// 前缀筛得动：只列这一层里以它打头的那几项。
    #[test]
    fn a_prefix_narrows_the_level_down_to_what_it_shares() {
        let root = tree();
        let typed = format!("{}/哆啦", root.path().display());

        let listed = level(&typed);

        assert_eq!(listed.len(), 2, "{listed:?}");
        assert!(
            listed.iter().all(|hit| name(hit).starts_with("哆啦A梦 0")),
            "{listed:?}"
        );
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

    /// 画质分由调用方给，因此**两边都在每个平台上跑得到**：不认大小写的那一档上敲 `d`
    /// 补得出 `Doraemon`，**补回来的是盘上那个写法**；认的那一档上一个字都不放宽。
    /// 这两条正是 macOS 与 Linux 各自那一边。
    #[test]
    fn the_case_the_level_folds_is_the_one_that_decides_the_prefix() {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon 01")).expect("建目录");
        let typed = format!("{}/d", root.path().display());

        let folded = level_asking_case(&typed, |_| CaseSensitivity::Insensitive);
        let strict = level_asking_case(&typed, |_| CaseSensitivity::Sensitive);

        assert_eq!(folded.len(), 1, "{folded:?}");
        assert!(
            folded[0].ends_with("Doraemon 01/"),
            "补回来的不是盘上那个写法：{folded:?}"
        );
        assert!(strict.is_empty(), "{strict:?}");
    }

    /// 探不出来的那一次按**严的那一边**走，而且**不记进那一格**——下一层也许答得出。
    ///
    /// 严的那一边选它是因为两边的失败模式不对称：严的失败起来是「补不出来」，
    /// 用户当场看得见、把字敲全就绕过去了；宽的失败起来是**补出一个盘上不存在的名字**，
    /// 那一串会进缓冲、进处理范围，直到清点才炸（停车场 Q363）。
    #[test]
    fn an_unanswerable_probe_keeps_case_and_is_not_remembered() {
        let known = OnceLock::new();

        assert_eq!(asked_once(&known, || None), CaseSensitivity::Sensitive);
        assert_eq!(known.get(), None, "探不出来的那一次不该记进那一格");

        // 下一层答得出，那一格才落下来。
        assert_eq!(
            asked_once(&known, || Some(CaseSensitivity::Insensitive)),
            CaseSensitivity::Insensitive
        );
        assert_eq!(known.get(), Some(&CaseSensitivity::Insensitive));
    }

    /// 那一格落下来之后**一次都不再问**——这正是「不让每按一次 `Tab` 都多一次系统调用」
    /// 落在哪儿：探测在 `ask` 里面，不问它就不动盘。
    #[test]
    fn an_answer_that_was_kept_is_never_asked_for_again() {
        let known = OnceLock::new();

        assert_eq!(
            asked_once(&known, || Some(CaseSensitivity::Insensitive)),
            CaseSensitivity::Insensitive
        );

        let answer = asked_once(&known, || -> Option<CaseSensitivity> {
            panic!("已经记住了，不该再问一次")
        });
        assert_eq!(answer, CaseSensitivity::Insensitive);
    }

    /// **一个进程只探一次**落在哪儿：那一格是模块级的 [`KNOWN`]，不是 [`remembered`]
    /// 每次调用新起的一格。把它挪进函数里，别的用例一条都不会红——只有这一条会。
    ///
    /// 落下来的那个答案与**当场拿库里那个探法独立探一次**得到的是同一个：那一格记的不是随便
    /// 什么东西，记的是库里那一个探法的答案——补全与一趟开工时认「是不是同一处」问的是同一个探法。
    #[test]
    fn the_answer_lands_in_the_one_the_whole_process_shares() {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon 01")).expect("建目录");
        let typed = format!("{}/", root.path().display());

        let _ = level(&typed);

        let landed = KNOWN.get().copied().expect("那一格没落下来");
        let straight = tonefit::case_sensitivity(root.path()).expect("这一层探得出来");
        assert_eq!(landed, straight);
    }

    /// 折法本身，**每个平台上都跑得到**：`starts_with_folded` 不看 `CaseSensitivity`，
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

    /// **端到端**，走 [`level`] 自己那一路：大小写按**这一层所在的文件系统**的规矩筛。
    /// 不认大小写的那一档上敲 `d` 两个都补得出，**补回来的是盘上那个写法**；
    /// 认的那一档上只补得出敲对了的那一个——那一支一个字都没放宽。
    ///
    /// 一条用例问两边，而**分岔的依据不再是编译期的东西**：翻过大小写的那个写法当场
    /// 开不开得开，就是这台机器的答案。断言因此与探测各自独立地问了同一件事，
    /// 探反了这条就红。
    #[test]
    fn case_is_folded_only_where_the_file_system_folds_it() {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon 01")).expect("建目录");
        std::fs::create_dir(root.path().join("doraemon 02")).expect("建目录");
        let folds = std::fs::symlink_metadata(root.path().join("DORAEMON 01")).is_ok();
        let typed = format!("{}/d", root.path().display());

        let listed = level(&typed);

        if folds {
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
