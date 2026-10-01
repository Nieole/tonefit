//! **同一处**：两条路径指不指着盘上同一个地方（`CONTEXT.md` 的《同一处》）。
//! 收编、借住、撞名认这件事只用这一把尺子（`one-source/03`，收停车场 Q241、Q300、Q366）。
//!
//! 两半，各住一处：
//!
//! - **判等**（[`same_place`]，查表时用它的键 [`Place`]）：纯函数，两条路径加一个
//!   「这一侧认不认大小写」的答案进去，一次盘都不问。**字面规整**，再按那个答案折不折大小写。
//! - **探法**（[`case_sensitivity`]）：那个答案从哪儿来。只读地翻一个已有名字的大小写去问盘。
//!
//! 用它的有四处：**收编**认「同一个卷根」（`crate::discover::Found`）、**借住**认「这个卷的去处是不是
//! 那个卷去处的祖先」（`crate::survey`）、**撞名**认「同一个去处」（卷与卷在 `crate::run` 开工前那一道；
//! 成员与借住的卷在清点里比、切开之后的名字在那一卷里比，见 `one-source/04`），
//! 以及会话的**逐层补全**——它只用探法，按前缀筛名字用它自己的折法。
//!
//! # 每一趟开工时探，不跨趟记
//!
//! 一趟开工时**每条处理路径与输出根各探一次**（[`Side::probe`]：处理路径在清点里逐条探，
//! 输出根在 `crate::run` 探写之后探），答案只活在这一趟里——为什么不跨趟记见 ADR 0009，
//! 规矩本身见 `CONTEXT.md` 的《同一处》。补全自己记一格进程内的答案，那是它自己的事。
//!
//! # 认下的边界
//!
//! - **软链不解析**：一条软链与它指向的那棵树是两个名字（`CONTEXT.md` 的《尚未确立》）。
//!   解析要路径存在，而撞名比的是还没写出来的去处。
//! - **`..` 不解析**：与软链一起时，字面解析会指错地方。
//! - **相对与绝对不互相认**：`库` 与 `/当前目录/库` 是两个写法——规整是字面的。

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

/// 一个位置所在的文件系统**认不认大小写**（`CONTEXT.md` 的《大小写敏感性》）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaseSensitivity {
    /// 认：`Doraemon` 与 `doraemon` 是两个名字。
    Sensitive,
    /// 不认：两者是同一个。
    Insensitive,
}

/// **探**：`place` 所在的文件系统认不认大小写。探不出来就 `None`。
///
/// **只读**——拿一个已有的名字翻一次大小写，问盘上有没有翻出来的那个写法。源库只读
/// （ADR 0009 决定第 1 条），「造一个探针文件再去开它」那种常见探法在这里不成立，
/// 何况源库那一层多半根本不可写（停车场 Q365 判过不造探针文件）。
///
/// **问哪一级**：从离 `place` 最近的已存在的那一级起往上，**在同一块盘上**，头一个列得出、
/// 而且列得出一个翻得动的名字的那一级给答案。于是三种常见的样子都答得出：
///
/// - `place` 还不存在（头一趟的输出根）——从离它最近的已存在的上一级问起；
/// - `place` 是一个文件（点名的归档）——问它所在的那一级，那里列得出它自己；
/// - `place` 那一级一个翻得动的名字都没有（空的输出根、只有中文与数字的库）——
///   它自己的名字在上一级里。
///
/// **不跨盘**：大小写敏感性是一块盘的性质，往上走一过挂载点，答的就是别的盘——一个空的、
/// 不认大小写的 U 盘挂在认大小写的目录底下时，上一级会答「认」，正是输出那一侧最怕的那一边。
/// 因此只走到这块盘的根为止，走到头还答不出就是 `None`（那一侧按自己的默认办，见库内的 `Side::case`）。
/// 「同一块盘」Unix 上按设备号认；别的平台上盘符的根本身就是边界，挂进一个文件夹的卷认不出来。
///
/// **不记任何东西**：问一次付一次。记不记、记多久是调用方的事（会话的逐层补全记一格）。
pub fn case_sensitivity(place: &Path) -> Option<CaseSensitivity> {
    let place = std::path::absolute(place).ok()?;
    let mut levels = place
        .ancestors()
        .skip_while(|level| disk_of(level).is_none());
    let nearest = levels.next()?;
    let disk = disk_of(nearest);
    std::iter::once(nearest)
        .chain(levels.take_while(|level| disk_of(level) == disk))
        .find_map(answered_by)
}

/// 这一级落在哪块盘上；这一级不在、或者问不出，就是 `None`。
#[cfg(unix)]
fn disk_of(level: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata(level).ok().map(|metadata| metadata.dev())
}

/// 同上。这些平台上 std 给不出卷号，一律当成同一块：往上走到盘符的根就到头了。
#[cfg(not(unix))]
fn disk_of(level: &Path) -> Option<()> {
    std::fs::metadata(level).ok().map(|_| ())
}

/// 这一级答得出就答：列一遍，拿头一个翻得动的名字问盘。列不出、或者一个翻得动的名字都没有，
/// 就是 `None`——由 [`case_sensitivity`] 往上问一级。
fn answered_by(level: &Path) -> Option<CaseSensitivity> {
    let names: Vec<String> = std::fs::read_dir(level)
        .ok()?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let other = names.iter().find_map(|name| flipped(name))?;
    if names.contains(&other) {
        // 两个只差大小写的名字同时在这一级里——不认大小写的文件系统装不下它们。
        // 这一级自己已经把话说完了，那一问一次都不必问（问了反倒答反：翻出来的那个写法
        // 正是旁边那个货真价实的兄弟）。
        return Some(CaseSensitivity::Sensitive);
    }
    // 问的是 `symlink_metadata`：翻出来的那个名字若是一条断了的链接，跟着它走会答「不在」。
    match std::fs::symlink_metadata(level.join(&other)) {
        Ok(_) => Some(CaseSensitivity::Insensitive),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Some(CaseSensitivity::Sensitive)
        }
        // 那一问自己失败了（这一级列得出名字、却不许进去问）：这不是答案，往上问一级。
        Err(_) => None,
    }
}

/// 把一个名字里每个**翻得动**的字都翻到另一个大小写；一个都翻不动就 `None`。
///
/// 只认「翻出来仍是一个字」的那些——`ß` 的大写是两个字母，翻出来的名字在**任何**
/// 文件系统上都问不着，拿它探等于白探。名字里带 U+FFFD 的一并不认：那是
/// `to_string_lossy` 给非 UTF-8 名字留下的记号，那个名字同样问不着盘，
/// 答出来的「认」是假的。
fn flipped(name: &str) -> Option<String> {
    if name.contains('\u{fffd}') {
        return None;
    }
    let mut any = false;
    let mut other = String::with_capacity(name.len());
    for here in name.chars() {
        match one_other_case(here) {
            Some(there) => {
                any = true;
                other.push(there);
            }
            None => other.push(here),
        }
    }
    any.then_some(other)
}

/// 一个字的另一个大小写，**仍是一个字**的那一种；没有就 `None`。
fn one_other_case(here: char) -> Option<char> {
    fn only_one(mut cased: impl Iterator<Item = char>, here: char) -> Option<char> {
        match (cased.next(), cased.next()) {
            (Some(one), None) if one != here => Some(one),
            _ => None,
        }
    }
    only_one(here.to_uppercase(), here).or_else(|| only_one(here.to_lowercase(), here))
}

/// 哪一侧。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Side {
    /// 源那一侧：处理路径。
    Source,
    /// 输出那一侧：输出根。
    Output,
}

impl Side {
    /// 这一侧拿什么去判等：探出来的答案原样用；**探不出时两侧取相反的一边**——源那一侧按认、
    /// 输出那一侧按不认。两边各取代价小的那一边，各是什么代价见 `CONTEXT.md` 的《同一处》。
    pub(crate) fn case(self, probed: Option<CaseSensitivity>) -> CaseSensitivity {
        probed.unwrap_or(match self {
            Side::Source => CaseSensitivity::Sensitive,
            Side::Output => CaseSensitivity::Insensitive,
        })
    }

    /// 探一次 `place`，按这一侧取答案。一趟开工时谁在什么时候叫它，见本模块的《每一趟开工时探，不跨趟记》。
    pub(crate) fn probe(self, place: &Path) -> CaseSensitivity {
        self.case(case_sensitivity(place))
    }
}

/// **判等**：`a` 与 `b` 在 `case` 那一档上是不是同一处。纯函数，一次盘都不问。
///
/// 两步：**字面规整**——去掉 `.` 分量与尾分隔符，**`..` 不解析**（中间那一级是一条软链的话，
/// 字面解析会指错地方）——再按 `case` 决定折不折大小写。软链同样不解析：一条软链与它指向的
/// 那棵树是两个名字（`CONTEXT.md` 的《尚未确立》）。
///
/// `case` 是**这一侧**的答案（[`Side::case`]）。要拿它当表的键时用 [`Place`]，两者是同一件事。
pub(crate) fn same_place(a: &Path, b: &Path, case: CaseSensitivity) -> bool {
    Place::of(a, case) == Place::of(b, case)
}

/// 一个地方**按同一处认出来的样子**：规整过、按这一侧折过的那串分量。
/// 两个 `Place` 相等当且仅当 [`same_place`] 说是同一处——要按同一处查表时拿它当键。
///
/// 只拿来比、拿来查，**不拿来印**：折过的那一份不是任何人写过的写法，
/// 报告里印的恒是用户点的那一条（见 `crate::discover::Found`）。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Place(Vec<OsString>);

impl Place {
    /// `path` 在 `case` 那一档上认出来的样子。
    ///
    /// [`Path::components`] 已经把中间的 `.`、重复的与末尾的分隔符去掉了，剩下打头那一个 `./`
    /// 由这里去掉；`..` 原样留着。
    ///
    /// 折法见 [`fold`]。分量不是合法 UTF-8 的原样留着、不折：
    /// 走 `to_string_lossy` 的话，两个不同的坏名字会折成同一串替换字符。
    pub(crate) fn of(path: &Path, case: CaseSensitivity) -> Self {
        Self(
            path.components()
                .filter(|component| *component != Component::CurDir)
                .map(|component| fold(component.as_os_str(), case))
                .collect(),
        )
    }

    /// 有几级。
    pub(crate) fn depth(&self) -> usize {
        self.0.len()
    }

    /// 它往上数、只剩头 `depth` 级的那一个祖先。
    pub(crate) fn ancestor(&self, depth: usize) -> Self {
        Self(self.0[..depth].to_vec())
    }

    /// 它的每一个**真祖先**（到它自己那一级为止不含），从最外一级起，各带着它有几级。
    ///
    /// 「谁住在谁的去处里」与「哪几个去处住得下别的卷」数的都是它（`crate::survey`）：
    /// 后者是前者的超集，靠的正是两处数的是同一批祖先。
    pub(crate) fn into_ancestors(self) -> impl Iterator<Item = (usize, Self)> {
        (1..self.depth()).map(move |depth| (depth, self.ancestor(depth)))
    }

    /// 头 `depth` 级底下的那一段，拼回一条相对路径——它在那个祖先里的位置。
    ///
    /// 要的是**原来的写法**时从认大小写那一档的 `Place` 上取（折过的那一份不是谁的写法）。
    pub(crate) fn below(&self, depth: usize) -> PathBuf {
        self.0[depth..].iter().collect()
    }
}

/// 一个分量在 `case` 那一档上的样子。
///
/// **逐字**取小写，不用 `str::to_lowercase`：后者按上下文折（词尾的 `Σ` 折成 `ς`），
/// 同一个字在名字里的位置不同就折出两样来；文件系统那几张表都是逐码点折的。
fn fold(name: &OsStr, case: CaseSensitivity) -> OsString {
    match (case, name.to_str()) {
        (CaseSensitivity::Insensitive, Some(text)) => OsString::from(
            text.chars()
                .flat_map(char::to_lowercase)
                .collect::<String>(),
        ),
        _ => name.to_os_string(),
    }
}

/// 一批地方，**各带着自己那一侧的答案**，按同一处认出「这一个之前见过没有」。
///
/// 收编用它：每条处理路径各探一次，两条处理路径的答案可以不同
/// （一条在认大小写的盘上、一条在不认的盘上）。两条路径的认法因此是：
///
/// - 字面规整之后相同的，**恒是**同一处——哪一侧的答案都改不了这一句；
/// - 只差大小写的，**两边都不认**大小写时才是同一处。一边认一边不认时按认的那一边，
///   也就是拿两边里严的那一个去问 [`same_place`]——源那一侧宁可不收（见 [`Side::case`]）。
///
/// 记的是**每个地方占着第几格**，格子里放什么由调用方自己管。
#[derive(Default)]
pub(crate) struct Places {
    /// 按认大小写那一档认出来的样子 → 第几格。每一个地方都记在这里。
    exact: HashMap<Place, usize>,
    /// 按不认大小写那一档认出来的样子 → 第几格。只有不认大小写那一侧的地方记在这里，
    /// 也只有它们来查。
    folded: HashMap<Place, usize>,
}

impl Places {
    /// `path`（它那一侧的答案是 `case`）之前见过没有：见过就是它占着的那一格。
    pub(crate) fn find(&self, path: &Path, case: CaseSensitivity) -> Option<usize> {
        let exact = self.exact.get(&Place::of(path, CaseSensitivity::Sensitive));
        let folded = || match case {
            CaseSensitivity::Insensitive => self.folded.get(&Place::of(path, case)),
            CaseSensitivity::Sensitive => None,
        };
        exact.or_else(folded).copied()
    }

    /// 记下 `path` 占着第 `at` 格。同一个样子已经记过的话**不改**——头一回占下的那一格算数。
    ///
    /// 认出了一个见过的地方之后也叫它一次：这一回的写法与那一侧的答案可能是头一回见，
    /// 记下来之后，只差大小写的第三种写法才认得回这一格。
    pub(crate) fn insert(&mut self, path: &Path, case: CaseSensitivity, at: usize) {
        self.exact
            .entry(Place::of(path, CaseSensitivity::Sensitive))
            .or_insert(at);
        if case == CaseSensitivity::Insensitive {
            self.folded.entry(Place::of(path, case)).or_insert(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use CaseSensitivity::{Insensitive, Sensitive};

    /// 两条路径在 `case` 那一档上是不是同一处，按字面写进用例里。
    fn same(a: &str, b: &str, case: CaseSensitivity) -> bool {
        same_place(Path::new(a), Path::new(b), case)
    }

    /// **规整那一半，两档都一样**：`.` 分量与尾分隔符去掉，打头那个 `./` 也去掉。
    #[test]
    fn a_dot_and_a_trailing_separator_do_not_make_another_place() {
        for case in [Sensitive, Insensitive] {
            assert!(
                same("库", "./库", case),
                "{case:?}：`库` 与 `./库` 认成了两处"
            );
            assert!(same("库/作品", "库/./作品", case), "{case:?}");
            assert!(same("库/作品/", "库/作品", case), "{case:?}");
            assert!(same("./库/./作品/", "库/作品", case), "{case:?}");
            assert!(same(".", "./", case), "{case:?}");
        }
    }

    /// **`..` 不解析**：`库/作品/..` 字面上不是 `库`——中间那一级是一条软链的话，
    /// 字面解析会指错地方。两档都一样。
    #[test]
    fn a_parent_component_is_kept_as_it_is_written() {
        for case in [Sensitive, Insensitive] {
            assert!(!same("库/作品/..", "库", case), "{case:?}：`..` 被解析掉了");
            assert!(!same("../库", "库", case), "{case:?}");
            assert!(same("../库", "./../库/", case), "{case:?}");
        }
    }

    /// **折不折那一半**：只差大小写的两条路径，不认大小写的那一档上是同一处，
    /// 认的那一档上是两处。差的不只是大小写的，哪一档上都是两处。
    #[test]
    fn case_folds_only_where_the_side_does_not_keep_it() {
        assert!(same("Lib/作品/Abc.cbz", "lib/作品/abc.cbz", Insensitive));
        assert!(!same("Lib/作品/Abc.cbz", "lib/作品/abc.cbz", Sensitive));
        assert!(
            same("./Lib/", "lib", Insensitive),
            "规整与折叠没有一起起作用"
        );
        for case in [Sensitive, Insensitive] {
            assert!(!same("Abc.cbz", "Abd.cbz", case), "{case:?}");
            assert!(same("Abc.cbz", "Abc.cbz", case), "{case:?}");
        }
    }

    /// **盘符的大小写**（停车场 Q241 那一例）：`D:\库` 与 `d:\库\作品` 里的 `d:\库`。
    /// 盘符只在 Windows 上是一个分量，别的平台上 `D:\库` 是一个普通的名字。
    #[cfg(windows)]
    #[test]
    fn a_drive_letter_folds_like_any_other_name() {
        assert!(same(r"D:\库", r"d:\库", Insensitive));
        assert!(same(r"D:\库\作品", r"d:/库/作品/", Insensitive));
        assert!(!same(r"D:\库", r"d:\库", Sensitive));
    }

    /// **探不出时两侧取相反的一边**：源那一侧按认，输出那一侧按不认。
    /// 探得出时两侧都照答案办。
    #[test]
    fn an_unanswered_probe_keeps_case_on_the_source_side_and_folds_it_on_the_output_side() {
        assert_eq!(Side::Source.case(None), Sensitive);
        assert_eq!(Side::Output.case(None), Insensitive);
        for side in [Side::Source, Side::Output] {
            for answer in [Sensitive, Insensitive] {
                assert_eq!(side.case(Some(answer)), answer, "{side:?} 没照答案办");
            }
        }
        // 落到判等上：同一对只差大小写的名字，源那一侧收不到一起，输出那一侧撞在一起。
        assert!(!same("Abc.cbz", "abc.cbz", Side::Source.case(None)));
        assert!(same("Abc.cbz", "abc.cbz", Side::Output.case(None)));
    }

    /// 一个刚建的目录，装着一个带大小写的名字 `Doraemon 01`，交回它与**这台机器的答案**。
    ///
    /// 答案由用例自己问盘：翻过大小写的那个写法开不开得开。它与探法各自独立地问同一件事，
    /// 探反了就对不上——探法在**跑用例的那个文件系统上**验，不开一个注入的口子。
    fn a_level_with_a_cased_name() -> (tempfile::TempDir, CaseSensitivity) {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon 01")).expect("建目录");
        let answer = if std::fs::symlink_metadata(root.path().join("DORAEMON 01")).is_ok() {
            Insensitive
        } else {
            Sensitive
        };
        (root, answer)
    }

    /// **探法答的是这台机器的事实**，而且只读：探完盘上一个名字都不多。
    #[test]
    fn the_probe_answers_what_this_file_system_does_and_leaves_nothing_behind() {
        let (root, answer) = a_level_with_a_cased_name();
        let before = names(root.path());

        assert_eq!(case_sensitivity(root.path()), Some(answer));
        assert_eq!(names(root.path()), before, "探完盘上多了东西");
    }

    /// **还不存在的地方探离它最近的已存在的上一级**——输出根头一趟还没建出来。
    #[test]
    fn a_place_that_is_not_there_yet_is_answered_by_the_nearest_level_that_is() {
        let (root, answer) = a_level_with_a_cased_name();

        assert_eq!(
            case_sensitivity(&root.path().join("还没有").join("也没有")),
            Some(answer)
        );
    }

    /// **这一级翻不出一个名字就往上问一级**：一个只装着中文与数字的目录，翻大小写无从翻起，
    /// 它自己的名字在上一级里——空的输出根、只有中文名的库都落在这一支上。
    /// 归档那种**文件**同样问它所在的那一级。
    #[test]
    fn a_level_without_a_cased_name_is_answered_by_the_level_above() {
        let (root, answer) = a_level_with_a_cased_name();
        let library = root.path().join("Doraemon 01");
        std::fs::create_dir(library.join("棋魂")).expect("建目录");
        std::fs::write(library.join("001"), b"x").expect("写文件");
        let empty = root.path().join("Doraemon 01").join("棋魂");

        assert_eq!(
            case_sensitivity(&library),
            Some(answer),
            "只有中文与数字的那一级"
        );
        assert_eq!(case_sensitivity(&empty), Some(answer), "空目录");
        assert_eq!(
            case_sensitivity(&library.join("001")),
            Some(answer),
            "一个文件"
        );
    }

    /// **两个只差大小写的名字同时在一级里**，这件事本身就答完了：不认大小写的文件系统装不下它们。
    /// 翻出来的那个写法正是旁边那个货真价实的兄弟，问盘反倒答反。
    ///
    /// 建不出这一对的机器（不认大小写）上这条问不出来，当场收工——它在那种机器上恒不成立，
    /// 误报不了；在建得出来的机器上，问盘那一版当场红。
    #[test]
    fn two_names_that_differ_only_in_case_settle_it_as_kept() {
        let root = tempfile::tempdir().expect("建临时目录");
        std::fs::create_dir(root.path().join("Doraemon")).expect("建目录");
        if std::fs::create_dir(root.path().join("dORAEMON")).is_err() {
            return;
        }

        assert_eq!(case_sensitivity(root.path()), Some(Sensitive));
    }

    /// **那一问自己被拒了不是答案**：这一级列得出名字、却不许进去问（只读不可进的目录），
    /// 问盘那一下失败——那不是「认」，往上问一级。上一级列得出这一级自己的名字，答的仍是这块盘。
    ///
    /// 弄不出「列得出、进不去」的机器上（root 底下权限位不作数，Windows 没有这一手）这条问不出来，
    /// 当场收工；在问得出的机器上，把失败当成「认」的那一版在不认大小写的盘上当场红。
    #[cfg(unix)]
    #[test]
    fn a_question_the_disk_refuses_is_not_an_answer() {
        use std::os::unix::fs::PermissionsExt;

        let (root, answer) = a_level_with_a_cased_name();
        let level = root.path().join("Doraemon 01");
        std::fs::create_dir(level.join("Inner")).expect("建目录");
        std::fs::set_permissions(&level, std::fs::Permissions::from_mode(0o444)).expect("改权限");
        let refused = std::fs::symlink_metadata(level.join("Inner")).is_err();

        let probed = case_sensitivity(&level);
        // 断言之前先把门打开：断言红了也不至于留下一个删不掉的临时目录。
        std::fs::set_permissions(&level, std::fs::Permissions::from_mode(0o755)).expect("改回权限");

        if refused {
            assert_eq!(probed, Some(answer));
        }
    }

    /// 一级里的名字，排好序。
    fn names(level: &Path) -> Vec<OsString> {
        let mut names: Vec<OsString> = std::fs::read_dir(level)
            .expect("列目录")
            .map(|entry| entry.expect("列目录项").file_name())
            .collect();
        names.sort();
        names
    }

    /// **两侧答案不同时怎么认**（收编用的那张表）：字面规整之后相同的恒是同一处；只差大小写的，
    /// 两边都不认大小写时才是同一处——一边认的话按认的那一边，源那一侧宁可不收。
    #[test]
    fn places_fold_case_only_between_two_that_both_fold_it() {
        let mut places = Places::default();
        places.insert(Path::new("Lib/作品"), Insensitive, 0);
        places.insert(Path::new("Other"), Sensitive, 1);

        assert_eq!(
            places.find(Path::new("./Lib/作品/"), Sensitive),
            Some(0),
            "字面相同却没认出来"
        );
        assert_eq!(places.find(Path::new("lib/作品"), Insensitive), Some(0));
        assert_eq!(
            places.find(Path::new("lib/作品"), Sensitive),
            None,
            "认大小写的一侧被折了"
        );
        assert_eq!(
            places.find(Path::new("other"), Insensitive),
            None,
            "认大小写的那一个被折了"
        );
        assert_eq!(places.find(Path::new("Other"), Insensitive), Some(1));

        // 认出来之后记下这一回的写法：只差大小写的第三种写法认得回同一格。
        places.insert(Path::new("Other"), Insensitive, 1);
        assert_eq!(places.find(Path::new("OTHER"), Insensitive), Some(1));
    }

    /// 判等与键是同一件事：键相等当且仅当判等说是同一处。
    #[test]
    fn two_places_share_a_key_exactly_when_they_are_the_same_place() {
        let pairs = [
            ("Lib/作品", "lib/./作品/"),
            ("库", "./库"),
            ("库/..", "库"),
            ("Abc.cbz", "abd.cbz"),
        ];
        for (a, b) in pairs {
            for case in [Sensitive, Insensitive] {
                assert_eq!(
                    Place::of(Path::new(a), case) == Place::of(Path::new(b), case),
                    same(a, b, case),
                    "{a} / {b} 在 {case:?} 上键与判等对不上"
                );
            }
        }
    }
}
