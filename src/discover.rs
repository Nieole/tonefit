//! 发现：**点名的一个路径展开成一批卷**。
//!
//! 两条规则，本模块就是它们（ADR 0014 决定第 1 条，那一条连同它是从哪个场景倒推出来的
//! 一并写在那里）：
//!
//! - 路径是**归档** → 一个卷，内部结构照收；
//! - 路径是**目录** → 直接躺着页就**同时**是一个卷（只收直接那一层），并且**继续往下发现**。
//!
//! 同一个目录因此可以既是卷又装着卷。**一页都不许被处理两遍**是这条规则的硬边界：
//! 目录卷只收直接那一层（见 `source::open_directory`），躺在那一层的归档是卷、不是成员。
//!
//! # 发现的是候选，不是卷
//!
//! 本模块**不解任何内容**：它按盘上的形状列出一批[候选](Candidate)，
//! 「里面到底有没有页」由预扫开卷时才答得出（见 `crate::survey`）。
//! 一页都没有的候选在那里被丢掉——**一页都没有的东西不是卷**，输出里一个字节都没有，
//! 而它没能收下的那些进**非卷文件**那张表（见 `crate::survey` 的《另一半产出》）。
//!
//! **「不解任何内容」有一处例外，只有一处**：一份 `.rar` 可以是另一份的**续**
//! （`x.part2.rar`），而那件事在盘上的形状里看不出来——续的那几份与头一份同名同扩展名。
//! 因此归档候选这一头要读一次归档头里的分卷标志（`source::split_part_of`，
//! ADR 0015 决定第 1 条的修订）：续的那几份不列成候选，头一份的卷名换成
//! [分卷序列](source::SplitPart)的名字。这一次读**只读归档头、不解一个内容字节**，
//! 而它换来的是预扫连开都不必开那几份。
//!
//! 因此这里列出的目录候选比真正的卷多：一个只装着子目录与 cbz 的目录也会被列出来，
//! 开出来是空的，随即丢掉。这笔多余换掉的是在两处各写一遍「什么算页」。
//!
//! # 点名的 / 发现的
//!
//! [`Provenance`] 只决定一件事：**点不开时的处置**。点名的整趟拒绝（他明说了要处理它），
//! 发现的记下来、其余照做——对推测出来的东西不用最重的处置。
//!
//! # 只在点名的路径底下走
//!
//! ADR 0009 关掉的三件事一件都没回来：不扫库根、不建索引、不监听。发现的起点恒是用户
//! 点名的那个路径，走的也只是它底下那棵子树。
//!
//! # 重叠的点名收编到最外层
//!
//! [`of`] 一次只看一个点名路径，而点名的路径可以**互相嵌套**（`库` 与 `库/作品`），
//! 也可以干脆点两遍。那时同一个卷会被发现两遍。[`Found`] 因此在发现之后按**卷根**
//! 折一遍：同一个卷根只留一份，镜像路径以**最外层**那个点名根为准（停车场 Q111）。

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::source::{self, Container};

/// 这一卷是**点名的**还是**发现的**。
///
/// 只决定[点不开时的处置](crate::survey)，别的一处都不看它。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provenance {
    /// 用户在命令行上点了名的那个路径。
    Named,
    /// 在点名的路径底下发现出来的。
    Discovered,
}

/// 发现出来的一个**卷候选**：一个路径，加上它将要写到输出根下的哪里。
pub(crate) struct Candidate {
    /// 卷根：目录路径，或归档文件路径。
    pub(crate) root: PathBuf,
    /// 这一卷在**输出根之下**的去处，相对路径。归档卷的扩展名已经归一成 `.cbz`。
    ///
    /// 由发现算出而不是由卷自己算出：镜像的基准点是**点名路径的父目录**，
    /// 而那件事只有发现知道（见 [`mirrored`]）。
    pub(crate) output_relative: PathBuf,
    /// 点名的还是发现的。
    pub(crate) provenance: Provenance,
    /// 这个候选在盘上是哪一种形状：一个目录，还是一个归档文件。
    ///
    /// 列目录时就知道了，不必再 stat 一次（见 [`Child`]）。预扫按它分岔**点不开时**
    /// 的处置：点不开的**归档**进非卷文件那张表，点不开的**目录**进走不进去的地方那一张
    /// ——那张表列的是文件，而一个目录不是文件（`CONTEXT.md` 的《处理对象》，
    /// ADR 0014 决定第 5 条）。
    pub(crate) container: Container,
}

/// 点名的一个路径展开成一批候选，按**发现顺序**排好。
///
/// 点名的路径自己恒是第一个候选：它是归档就只有它，是目录就还有它底下那些。
/// 顺序是先序深度优先，同一层内按[阅读顺序](source::reading_order)——报告里的卷序
/// 因此与文件管理器里看到的一致，而 `第2话` 排在 `第10话` 前面。
///
/// 点名的路径既不是目录也不是认得的归档时回 `Err`：那是**点名的**那一种点不开，
/// 整趟拒绝（见 `crate::survey::refuse`）。
pub(crate) fn of(named: &Path) -> Result<Vec<Candidate>> {
    let (name, container) = source::identity_of(named)?;
    let mut found = vec![Candidate {
        root: named.to_path_buf(),
        output_relative: mirrored(named, named, &source::output_name_of(&name, container)),
        provenance: Provenance::Named,
        container,
    }];
    if container == Container::Directory {
        expand(named, &mut found);
    }
    Ok(found)
}

/// 点名的**那几个**路径展开出来的候选，按**卷根**收编成一批：同一个卷根只留一份。
///
/// [`of`] 一次只看一个点名路径。点名的路径互相嵌套（`库` 与 `库/作品`）、或者同一个
/// 路径点了两遍时，同一个卷被发现两遍，而两遍的镜像去处还**不同**
/// （`out/库/作品/第1话.cbz` 与 `out/作品/第1话.cbz`）——撞名那一道
/// （`crate::ensure_no_two_volumes_share_an_output`）比的是去处，因此拦不住：
/// 同一卷做两遍、数两遍，盘上落下两份同内容不同位置的产物（停车场 Q111）。
///
/// 收编的读法与「点名一个目录就把底下的卷全找出来」是同一条：点名 `库` 已经包含了
/// `库/作品` 底下的一切，**再点一次不该改变结果**。因此镜像路径以**最外层**那个点名根
/// 为准——点名 `库 库/作品` 与只点名 `库` 得到同一棵输出树、同样多的卷、同一份
/// 非卷文件清单。
///
/// **次序按「头一回被发现」**：一个卷根头一回出现时占下它那一格，后来的只往那一格上收编。
/// 跨几个点名路径的卷序因此仍是「用户点名的次序，各自内部按[阅读顺序](source::reading_order)」
/// ——`tonefit 库/作品 库` 与 `tonefit 库 库/作品` 得到同一棵输出树、同样多的卷、
/// 同一份非卷文件清单，而报告里的**卷序**不同（停车场 Q243）。
///
/// **折的是卷根，不是点名路径之间的前缀关系。** 点名 `库` 与 `库/#recycle` 是嵌套的两条
/// 路径，而发现根本走不进后者（[`push_children`] 把不看的地方挡在外面），
/// 两边一个卷根都不重叠，于是两个点名各展各的——用户明说要那个回收站，就得到它。
/// 按前缀折的话它会连同它底下的卷一起消失。
#[derive(Default)]
pub(crate) struct Found {
    /// 收编之后的那些候选，按**发现顺序**：一个卷根头一回出现时占下它那一格，
    /// 后来的只往那一格上收编，不再排队。
    candidates: Vec<Candidate>,
    /// 卷根 → 它占着上面哪一格，加上收编下它的那个点名根**有几级**。
    ///
    /// 级数用来认「哪个点名根更外层」。这么认得住，是因为两个点名根展开出同一个卷根
    /// **当且仅当**它们互相嵌套——发现只在点名的路径底下走，也不跟符号链接
    /// （见本模块的《只在点名的路径底下走》与 [`push_children`]），子树因此严格按路径
    /// 前缀嵌套；而嵌套的两条路径里，级数少的那条在外层。
    ///
    /// 查的是**路径本身**，而 [`Path`] 的相等在哪个平台上都逐字节比：同一个卷根写成
    /// `D:\库` 与 `d:\库`（不区分大小写的文件系统上是同一个）、`库` 与 `./库`、
    /// 或者一条软链与它指向的那棵树，收编都认不出来（停车场 Q241）。
    ///
    /// 这张表随**卷数**长，活到 [`into_candidates`](Self::into_candidates) 为止。
    /// 它攥的是路径，不是句柄：与预扫那笔「不攥着几千个句柄」的账不是同一本
    /// （见 `crate::survey` 的模块文档）。
    at: HashMap<PathBuf, (usize, usize)>,
}

impl Found {
    /// 点名 `named` 展开出来的那一批（[`of`] 的产出）收进来。
    ///
    /// 卷根头一回见就原样进队；见过就**收编**到它那一格上，两样东西各按各的规矩：
    ///
    /// - **镜像路径**换成最外层那个点名根算出来的那一条（见本类型的文档）；
    /// - **[点名的那顶帽子](Provenance)** 两边有一顶就留着。用户明说了要处理它，
    ///   点不开就该整趟拒绝——收编改的是**去处**，不是 ADR 0014 决定第 5 条那条分别。
    ///
    /// [`Candidate::container`] 不必挑：同一个卷根在盘上是同一样东西。
    pub(crate) fn absorb(&mut self, named: &Path, candidates: Vec<Candidate>) {
        let depth = named.components().count();
        for candidate in candidates {
            let Some((at, outermost)) = self.at.get(&candidate.root).copied() else {
                self.at
                    .insert(candidate.root.clone(), (self.candidates.len(), depth));
                self.candidates.push(candidate);
                continue;
            };
            if candidate.provenance == Provenance::Named {
                self.candidates[at].provenance = Provenance::Named;
            }
            if depth < outermost {
                self.candidates[at].output_relative = candidate.output_relative;
                self.at.insert(candidate.root, (at, depth));
            }
        }
    }

    /// 收编完的那一批，按发现顺序。
    pub(crate) fn into_candidates(self) -> Vec<Candidate> {
        self.candidates
    }
}

/// 点名的那个目录底下的那些候选。
///
/// 用一个显式的栈而不是递归：库的深度由用户的目录树说了算，而爆栈是一种没有报告、
/// 也没有退出码的失败。
fn expand(named: &Path, found: &mut Vec<Candidate>) {
    let mut stack = Vec::new();
    push_children(named, &mut stack);
    while let Some(child) = stack.pop() {
        let Some(name) = source::volume_name_of(&child.path, child.container) else {
            // 「取不出卷名就不是卷」，两种（见 `source::volume_name_of`）：末级分量取不出
            // 名字的路径给不出去处——`read_dir` 吐出来的东西恒有文件名，这一支够不着，
            // 留着是因为这条要写在它成立的地方；以及**分卷序列里续的那一份**——
            // 它的字节由头一份那一卷读走，不列成候选正是「折成一个卷」这件事本身
            // （`p4-parking-lot/17`，见本模块的《发现的是候选，不是卷》）。
            continue;
        };
        found.push(Candidate {
            root: child.path.clone(),
            output_relative: mirrored(
                named,
                &child.path,
                &source::output_name_of(&name, child.container),
            ),
            provenance: Provenance::Discovered,
            container: child.container,
        });
        if child.container == Container::Directory {
            push_children(&child.path, &mut stack);
        }
    }
}

/// 盘上的一个候选：一个路径加它的形态。形态在列目录时就知道了，不必再 stat 一次。
struct Child {
    path: PathBuf,
    container: Container,
}

/// 把 `dir` 这一层里成得了卷的那些**倒序**压进栈，弹出来就是阅读顺序。
///
/// 列不动这一层就整棵子树跳过：那是**发现出来**的一处读不动，不是点名的那一个——
/// 其余照做（`CONTEXT.md` 的《失败》）。
///
/// **这里不出声，而这一趟不会因此不出声**：本函数收到的每一个目录，自己都是一个候选
/// ——`named` 是 [`of`] 造的头一个，往下每一层都由 [`expand`] 先推进 `found` 再走到这里。
/// 预扫开它的时候同一个 `read_dir` 再失败一次，那一处把它记进**走不进去的地方**
/// （见 `crate::survey`，`p4-parking-lot/11` 收的停车场 Q117）。这里因此不必再报一遍——
/// 报了就是同一个目录两条。
///
/// 三样东西被挡在外面：[本来就不看的地方](source::is_ignored_directory)、
/// 符号链接与 junction（`file_type` 问的是链接自己，因此它既不是目录也不是文件，
/// **环进不来**，深度不必设上界）、以及既不是目录也不是认得的归档的文件——
/// 后者要么是某个卷的透传成员，要么是**非卷文件**——分界是它躺的那一层有没有页，
/// 而那要开卷才答得出，因此不在这里定（见 `crate::survey` 的《另一半产出》）。
///
/// 头一样是在**这里**挡的，不是攒完候选再滤：不看的地方连同它底下的一切**根本不成为候选**，
/// 预扫因此看不见它、走不进去的地方那一栏收不到它、退出码一格不动、报告上一个字都没有。
fn push_children(dir: &Path, stack: &mut Vec<Child>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            let name = entry.file_name();
            if source::is_ignored_directory(&name.to_string_lossy()) {
                continue;
            }
            children.push(Child {
                path,
                container: Container::Directory,
            });
        } else if file_type.is_file() && source::is_archive(&path) {
            children.push(Child {
                path,
                container: Container::Archive,
            });
        }
    }
    children.sort_by(|a, b| source::reading_order(&a.path, &b.path));
    // 栈是后进先出，倒着压进去弹出来才是阅读顺序。
    stack.extend(children.into_iter().rev());
}

/// 一个卷根**镜像**到输出根之下的相对去处。
///
/// 基准点是**点名路径的父目录**——点名路径自己的名字因此恒出现在输出根下，
/// 点名一个卷与点名一个装着卷的目录因此共用这一套规则：
///
/// ```text
/// 点名 _samples\网络资源            → 网络资源\N和S\第10话.cbz
/// 点名 _samples\_cbz\改革之獸-Vol05 → 改革之獸-Vol05
/// ```
///
/// 取「点名路径自己」当基准点的话，点名一个**卷**时它的页会直接撒进输出根，
/// 于是实际要两套规则。
///
/// 末一级换成 `output_name`：归档卷的扩展名在那里归一成 `.cbz`
/// （见 [`source::output_name_of`]）。
fn mirrored(named: &Path, root: &Path, output_name: &str) -> PathBuf {
    let mut parts: Vec<OsString> = Vec::new();
    if let Some(base) = named.file_name() {
        parts.push(base.to_os_string());
    }
    if let Ok(inside) = root.strip_prefix(named) {
        parts.extend(
            inside
                .components()
                .map(|component| component.as_os_str().to_os_string()),
        );
    }
    // 源那一级的名字换成输出那一级的名字。`parts` 此刻至少有一项：
    // `named` 取得出卷名（`of` 的第一句已经确认过），`root` 恒在它之下。
    parts.pop();
    parts.push(OsString::from(output_name));
    parts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 点名一个卷：它的名字恒出现在输出根下，而不是把页撒进输出根。
    #[test]
    fn a_named_volume_keeps_its_own_name_under_the_output_root() {
        assert_eq!(
            mirrored(
                Path::new("库/改革之獸-Vol05"),
                Path::new("库/改革之獸-Vol05"),
                "改革之獸-Vol05"
            ),
            PathBuf::from("改革之獸-Vol05")
        );
    }

    /// 点名一个装着卷的目录：点名路径的名字打头，其下按源的结构镜像。
    #[test]
    fn a_discovered_volume_mirrors_the_tree_below_the_named_path() {
        assert_eq!(
            mirrored(
                Path::new("_samples/网络资源"),
                Path::new("_samples/网络资源/N和S/第10话.zip"),
                "第10话.cbz"
            ),
            PathBuf::from("网络资源/N和S/第10话.cbz")
        );
    }

    /// 归档卷的扩展名在末一级归一，源那一头叫什么都不带过来。
    #[test]
    fn a_named_archive_normalises_its_extension() {
        assert_eq!(
            mirrored(
                Path::new("库/第10话.zip"),
                Path::new("库/第10话.zip"),
                "第10话.cbz"
            ),
            PathBuf::from("第10话.cbz")
        );
    }

    /// 发现出来的候选按阅读顺序排，`第2话` 在 `第10话` 前面。
    #[test]
    fn discovered_volumes_come_out_in_reading_order() {
        let space = tempfile::tempdir().expect("建临时目录");
        let works = space.path().join("作品");
        std::fs::create_dir(&works).expect("建作品目录");
        for name in ["第10话.cbz", "第2话.cbz", "第1话.cbz"] {
            std::fs::write(works.join(name), b"").expect("摆一个归档");
        }

        let found = of(&works).expect("点名的目录点得开");

        let names: Vec<String> = found
            .iter()
            .map(|candidate| candidate.output_relative.display().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "作品".to_owned(),
                format!("作品{sep}第1话.cbz", sep = std::path::MAIN_SEPARATOR),
                format!("作品{sep}第2话.cbz", sep = std::path::MAIN_SEPARATOR),
                format!("作品{sep}第10话.cbz", sep = std::path::MAIN_SEPARATOR),
            ]
        );
    }

    /// 不看的地方整棵子树不进去——回收站里躺着的正是用户删掉的那些卷。
    ///
    /// 打包环境留下的与操作系统留下的**同一份名单**：一个 `#recycle` 与一个
    /// `System Volume Information` 在这一层受同一条判据，谁都不成为候选。
    #[test]
    fn an_ignored_place_is_never_walked_into() {
        let space = tempfile::tempdir().expect("建临时目录");
        let library = space.path().join("库");
        for ignored in ["#recycle", "System Volume Information"] {
            std::fs::create_dir_all(library.join(ignored).join("删掉的作品"))
                .expect("建不看的那一层");
            std::fs::write(library.join(ignored).join("删掉的作品/第1话.cbz"), b"")
                .expect("摆一个归档");
        }
        std::fs::create_dir(library.join("留着的作品")).expect("建作品目录");

        let found = of(&library).expect("点名的库点得开");

        let roots: Vec<&Path> = found
            .iter()
            .map(|candidate| candidate.root.as_path())
            .collect();
        assert_eq!(roots, [library.as_path(), &library.join("留着的作品")]);
    }

    /// 点名的路径自己是第一个候选，且戴着「点名的」那顶帽子。
    #[test]
    fn only_the_named_path_is_named() {
        let space = tempfile::tempdir().expect("建临时目录");
        let works = space.path().join("作品");
        std::fs::create_dir(&works).expect("建作品目录");
        std::fs::write(works.join("第1话.cbz"), b"").expect("摆一个归档");

        let found = of(&works).expect("点名的目录点得开");

        let provenances: Vec<Provenance> =
            found.iter().map(|candidate| candidate.provenance).collect();
        assert_eq!(
            provenances,
            [Provenance::Named, Provenance::Discovered],
            "点名的不止一个，或者点名的那个不在头一位"
        );
    }
}
