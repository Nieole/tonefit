//! **清点之后的那棵树**：卷列表由开工那一条带回来的[清点清单](tonefit::SurveyedVolume)
//! 与这一趟勾着的[处理路径](super::state::NamedPath)拼出来的形状（`CONTEXT.md` 的《会话》：
//! 卷列表、分区、目录行 / 卷行、备注行、展开；ADR 0019 决定第 2 条；spec《卷列表》清点之后）。
//!
//! # 拼法
//!
//! 1. **没勾的不进树**，末行暗淡地说一句另有几条（[`Tree::unchecked`]）。
//! 2. **嵌套的并进最外层**：一条勾着的处理路径落在另一条勾着的**文件夹**底下时不单独成节点，
//!    它的卷归外面那一条——与清点的去重一致（认法与 [`super::state::Session::nested_in`] 同一条：
//!    按路径前缀认，不碰盘）。
//! 3. 每一卷归**最外层**那条勾着的处理路径；一条处理路径的卷**按卷根的父目录**分成目录，
//!    **分组仍是报告那一份**（[`crate::render::grouped_roots`]，命令行按目录折起读的同一个函数），
//!    喂的是清点清单。
//! 4. 分成两个及以上目录 → **分区**（[`Shape::Section`]）；只有一个 → 顶格**目录行**
//!    （[`Shape::Directory`]）。**顶格目录行按目录跨路径合并**：逐卷点名的那几个压缩包各自只展开出
//!    一个目录，父目录相同的因此合成一行，排在它第一个成员出现的位置——「按父目录跨路径合并」
//!    不是另一条规矩，是这一条的结果。
//! 5. **备注行**挂在它所在那条处理路径的节点末尾：无法访问的地方一处一行，非漫画文件合成一行。
//! 6. **次序照发现**：顶层按处理路径的先后，目录按它头一卷在清单上的先后，卷按清单上的先后。
//!    状态一个字都不参与——出事的靠**行首记号**跳出来，不靠位置。
//!
//! # 它一个终端都不碰
//!
//! 摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）：拼法是纯逻辑，闸门 2 那一趟
//! 照编照测。**这一层一个状态都不读**——一卷此刻怎么样（[`super::live::VolumeState`]）
//! 由画法那一层按清单序号去问那一趟，树只说「哪一行是什么、排在哪儿」。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tonefit::{NonVolumeFile, SurveyedVolume, UnreachablePlace};

use super::state::NamedPath;
use super::view::Cursor;

/// 树上的**一个目录行**：一个目录，连同它底下那几卷在清点清单里的序号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directory {
    /// 这一枝是哪个目录（卷根的父目录）。
    pub path: PathBuf,
    /// 屏上写的那个名字：目录落在它那条处理路径底下的那一截，顶格的那几行是最后一段。
    pub label: String,
    /// 它收下的那几卷，按清点清单上的先后。
    pub volumes: Vec<usize>,
}

/// **备注的两种**（`CONTEXT.md` 的《备注行》：报告末尾那两小结拆散后挂回来的那一条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// 无法访问的地方，一处一行（`✗`）。
    Unreachable,
    /// 非漫画文件，一组一行（`-`）。
    NonVolume,
}

/// 挂在分区（或顶格目录行）末尾的**一条备注**。
///
/// 三截字照设计稿：前面一个**名头**（`无法访问`／`已忽略 N 个文件`），中间**是哪几处**，
/// 行尾**那一句**。名头与「是哪几处」是会话自己的措辞；行尾那一句出自报告——
/// 无法访问的地方给那条错误链（[`UnreachablePlace::reason`]），非漫画文件给那一小结
/// 抬头那一句（[`crate::render::non_volume_heading`]），ADR 0016 那条规矩因此一处没破。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub kind: NoteKind,
    /// 光标停在这一行上时记着的身份：无法访问的地方是那一处，非漫画文件是它挂着的那个节点。
    pub at: PathBuf,
    /// 名头。
    pub label: String,
    /// 是哪几处。
    pub what: String,
    /// 行尾那一句。
    pub brief: String,
    /// **这一条装着的那几处**：一处一条，**那条路径，与那一句为什么**
    /// （报告那一处渲染出来的整句）。`⏎` 掀开的那张[说明卡](super::cover::Card)
    /// 的全文从它拼——摆法仍是报告末尾那一小结那一副
    /// （[`crate::render::unreachable_stack`] 与 [`crate::render::non_volume_stack`]），
    /// 这一层一个字都不重写。
    pub said: Vec<(PathBuf, String)>,
}

/// 树顶层的一项是哪一种（`CONTEXT.md` 的《分区》：顶层只有分区与顶格目录行两种）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// **分区**：一条处理路径展开成两个及以上目录。光标停不上，也不折叠。
    Section {
        /// 它是哪一条处理路径。
        path: PathBuf,
        directories: Vec<Directory>,
    },
    /// **顶格目录行**：只展开成一个目录的处理路径，连同跨路径合并进来的那几条。
    Directory(Directory),
}

/// 树顶层的一项：一个分区或一个顶格目录行，加上挂在它末尾的备注。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub shape: Shape,
    pub notes: Vec<Note>,
}

impl Node {
    /// 这一项底下的目录行，按次序。
    pub fn directories(&self) -> &[Directory] {
        match &self.shape {
            Shape::Section { directories, .. } => directories,
            Shape::Directory(directory) => std::slice::from_ref(directory),
        }
    }

    /// 这一项底下的目录行，按次序，改得动。
    fn directories_mut(&mut self) -> &mut [Directory] {
        match &mut self.shape {
            Shape::Section { directories, .. } => directories,
            Shape::Directory(directory) => std::slice::from_mut(directory),
        }
    }

    /// 这一项是分区吗（分区多一行标题，底下的目录行缩进一级）。
    pub fn is_section(&self) -> bool {
        matches!(self.shape, Shape::Section { .. })
    }
}

/// 清点之后那棵树。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// 清点清单上每一卷的**卷根**，按清单序号——卷行的身份（[`Cursor::Volume`]）取自它。
    ///
    /// 留一份在树上，行才**自己答得出身份**：认一行是哪一卷不必再去问那一趟，
    /// 而状态机（[`super::view::Session::perform`]）本来就够不着它。
    pub roots: Vec<PathBuf>,
    /// 没勾的处理路径有几条——末行那一句说的就是它，零就没有那一行。
    pub unchecked: usize,
}

/// 树上的一行（`CONTEXT.md` 的《目录行 / 卷行》《备注行》）。
///
/// **缩进以两格为一级**：分区底下的目录行缩一级，卷行再缩一级；顶格目录行不缩。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// 分区标题：第几个顶层节点。**停不上**。
    Section { node: usize },
    /// 目录行：第几个顶层节点、节点里第几个目录。
    Directory { node: usize, at: usize, indent: u16 },
    /// 卷行：清点清单里第几卷。
    Volume { at: usize, indent: u16 },
    /// 备注行：第几个顶层节点、第几条备注。
    Note { node: usize, at: usize, indent: u16 },
    /// 分区前后那一行空白。**停不上**。
    Gap,
    /// 末行「另有 N 个路径未勾选」。**停不上**。
    Foot,
}

impl Tree {
    /// 由清点清单与这一趟的处理路径拼出一棵树。
    ///
    /// `roster` 是开工那一条带回来的[清点清单](tonefit::SurveyedVolume)，照发现的次序；
    /// 另两张表是报告末尾那两小结的同一份内容，拆散之后挂成备注行。
    pub fn of(
        paths: &[NamedPath],
        roster: &[SurveyedVolume],
        non_volume_files: &[NonVolumeFile],
        unreachable_places: &[UnreachablePlace],
    ) -> Self {
        let outer: Vec<&NamedPath> = paths
            .iter()
            .filter(|named| named.on)
            .filter(|named| outermost(&named.path, paths))
            .collect();
        let mut nodes: Vec<Node> = Vec::new();
        for named in &outer {
            let mine: Vec<usize> = (0..roster.len())
                .filter(|at| owner(&roster[*at].root, &outer).is_some_and(|it| it == named.path))
                .collect();
            let roots: Vec<&Path> = mine.iter().map(|at| roster[*at].root.as_path()).collect();
            let groups = crate::render::grouped_roots(roots.iter().copied());
            let directories: Vec<Directory> = groups
                .iter()
                .map(|group| Directory {
                    label: label_of(&group.directory, &named.path),
                    path: group.directory.clone(),
                    volumes: group.at.iter().map(|at| mine[*at]).collect(),
                })
                .collect();
            // 一个目录都没展开出来的处理路径仍占一个顶格目录行：备注行要有地方挂，
            // 屏上也不该让一条勾着的路径整个消失。
            let node = match directories.len() {
                0 => Node {
                    shape: Shape::Directory(Directory {
                        label: label_of(&named.path, &named.path),
                        path: named.path.clone(),
                        volumes: Vec::new(),
                    }),
                    notes: Vec::new(),
                },
                1 => Node {
                    shape: Shape::Directory(directories.into_iter().next().expect("一个目录")),
                    notes: Vec::new(),
                },
                _ => Node {
                    shape: Shape::Section {
                        path: named.path.clone(),
                        directories,
                    },
                    notes: Vec::new(),
                },
            };
            // **跨路径合并**：已经有一个同一个目录的顶格目录行时并进去，不另起一行。
            if let Shape::Directory(directory) = &node.shape
                && let Some(already) = nodes.iter_mut().find_map(|node| match &mut node.shape {
                    Shape::Directory(one) if one.path == directory.path => Some(one),
                    _ => None,
                })
            {
                already.volumes.extend(directory.volumes.iter().copied());
                continue;
            }
            nodes.push(node);
        }
        // **跨路径合并进来的那几卷按清单的先后重排**：合并是按处理路径的次序并进来的，
        // 而树上一律照发现的先后（模块文档第 6 条）。
        for directory in nodes.iter_mut().flat_map(Node::directories_mut) {
            directory.volumes.sort_unstable();
        }
        attach_notes(&mut nodes, non_volume_files, unreachable_places);
        Self {
            nodes,
            roots: roster.iter().map(|listed| listed.root.clone()).collect(),
            unchecked: paths.iter().filter(|named| !named.on).count(),
        }
    }

    /// 屏上此刻的那几行：展开着的目录（按目录的路径记，`expanded`）才摊出它的卷。
    pub fn rows(&self, expanded: &BTreeSet<PathBuf>) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        for (node, one) in self.nodes.iter().enumerate() {
            let indent = if one.is_section() { 1 } else { 0 };
            if one.is_section() {
                if !matches!(rows.last(), None | Some(Row::Gap)) {
                    rows.push(Row::Gap);
                }
                rows.push(Row::Section { node });
            }
            for (at, directory) in one.directories().iter().enumerate() {
                rows.push(Row::Directory { node, at, indent });
                if expanded.contains(&directory.path) {
                    rows.extend(directory.volumes.iter().map(|at| Row::Volume {
                        at: *at,
                        indent: indent + 1,
                    }));
                }
            }
            rows.extend((0..one.notes.len()).map(|at| Row::Note {
                node,
                at,
                indent: 1,
            }));
            if one.is_section() {
                rows.push(Row::Gap);
            }
        }
        if self.unchecked > 0 {
            rows.push(Row::Foot);
        }
        rows
    }

    /// 第几个节点里的第几个目录。
    pub fn directory(&self, node: usize, at: usize) -> Option<&Directory> {
        self.nodes.get(node)?.directories().get(at)
    }

    /// 第几个节点里的第几条备注。
    pub fn note(&self, node: usize, at: usize) -> Option<&Note> {
        self.nodes.get(node)?.notes.get(at)
    }

    /// 头一条**答得上这一问**的备注在树上是**第几个节点的第几条**。
    ///
    /// 走一遍全部备注这一手只有这一处：[`locate_note`](Self::locate_note) 按
    /// [那一条的身份](Note::at)问，场景夹具按它的[「是哪几处」](Note::what)问
    /// （`super::scene` 的 `stand_on_a_note`：那一头记的是屏上那一截字）。
    pub fn locate(&self, mut is_it: impl FnMut(&Note) -> bool) -> Option<(usize, usize)> {
        self.nodes.iter().enumerate().find_map(|(node, one)| {
            one.notes
                .iter()
                .position(&mut is_it)
                .map(|which| (node, which))
        })
    }

    /// 光标记着的那条备注在树上是第几个节点的第几条。
    ///
    /// 掀说明卡那一下要它：光标记的是[那一条备注的身份](Note::at)（`CONTEXT.md` 的
    /// 《卷列表》：光标记的是行的身份），而那张卡记的是树上的位置——树一趟只拼一次，
    /// 掀着的这一会儿它一格不动。
    pub fn locate_note(&self, at: &Path) -> Option<(usize, usize)> {
        self.locate(|note| note.at == at)
    }

    /// 清单里第几卷的卷根。
    pub fn root(&self, volume: usize) -> Option<&Path> {
        self.roots.get(volume).map(PathBuf::as_path)
    }

    /// 树上**每一个目录**的路径。只有[全部摊开那一副](Self::every_row)要它。
    fn every_directory(&self) -> BTreeSet<PathBuf> {
        self.nodes
            .iter()
            .flat_map(Node::directories)
            .map(|directory| directory.path.clone())
            .collect()
    }

    /// **全部目录都摊开**那一副的行：**跳转按这一副数次序**
    /// （`CONTEXT.md` 的《卷列表》：收着的目录自动展开到那一卷）。
    ///
    /// 收着的目录里那几卷照样是落点——跳过去才把那个目录展开，
    /// 而「下一个」指的是**树上**的下一个，不是「屏上此刻摆着的那几行里的下一个」：
    /// 不然把一个目录收起来就能让它底下的问题跳不到。
    pub fn every_row(&self) -> Vec<Row> {
        self.rows(&self.every_directory())
    }

    /// 这一行**拿哪一截字给搜索比**（`CONTEXT.md` 的《卷列表》：`/` 搜卷名或目录名）：
    /// 目录行是它的名字，卷行是「目录名/卷名」，备注行是名头加「是哪几处」。
    /// 分区标题、空行与末行那一句不参与，答 `None`。
    ///
    /// **匹配与落点分开**：这一处答的是「屏上这一行要不要加下划线」；哪几行是
    /// `⏎`／`n`／`N` 的落点另有一条（目录名自己就命中时它底下那几卷不再各算一个落点，
    /// 见 `super::view::Session::jump`）。
    pub fn searched_text(&self, row: Row) -> Option<String> {
        match row {
            Row::Directory { node, at, .. } => Some(self.directory(node, at)?.label.clone()),
            Row::Volume { at, .. } => Some(format!(
                "{}/{}",
                self.directory_of(at)?.label,
                crate::render::volume_name(self.root(at)?)
            )),
            Row::Note { node, at, .. } => {
                let note = self.note(node, at)?;
                Some(format!("{}{}", note.label, note.what))
            }
            Row::Section { .. } | Row::Gap | Row::Foot => None,
        }
    }

    /// 这个卷根是清单里第几卷。**按卷根认**（清点已按卷根收编过，清单里卷根不重）——
    /// 那一趟报回来的卷根（[`super::live::Walking::volume`]）与光标记着的身份
    /// （[`Cursor::Volume`]）都得从这一处换回序号。
    pub fn index_of(&self, root: &Path) -> Option<usize> {
        self.roots.iter().position(|one| one == root)
    }

    /// 这一卷归哪一条**分区**——每页结果的面包屑上那一截（`CONTEXT.md` 的《分区》）。
    ///
    /// **顶格目录行底下的卷没有分区**：那一行本身就是顶层，面包屑上因此少一截
    /// （设计稿 `drawPages` 的 `crumbs` 那一支）。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn section_of(&self, volume: usize) -> Option<&Path> {
        self.nodes.iter().find_map(|node| match &node.shape {
            Shape::Section { path, directories } => directories
                .iter()
                .any(|directory| directory.volumes.contains(&volume))
                .then_some(path.as_path()),
            Shape::Directory(_) => None,
        })
    }

    /// 这一卷归哪一个目录（`h` 收起回到父目录、每页结果的面包屑都问它）。
    pub fn directory_of(&self, volume: usize) -> Option<&Directory> {
        self.nodes
            .iter()
            .flat_map(Node::directories)
            .find(|directory| directory.volumes.contains(&volume))
    }
}

impl Row {
    /// 这一行停得住的话，它的**身份**（`CONTEXT.md` 的《停得住 / 展得开》：
    /// 分区标题、空行与末行那一句停不上）。
    pub fn stop(&self, tree: &Tree) -> Option<Cursor> {
        match self {
            Self::Section { .. } | Self::Gap | Self::Foot => None,
            Self::Directory { node, at, .. } => tree
                .directory(*node, *at)
                .map(|directory| Cursor::Directory(directory.path.clone())),
            Self::Volume { at, .. } => tree
                .root(*at)
                .map(|root| Cursor::Volume(root.to_path_buf())),
            Self::Note { node, at, .. } => tree
                .note(*node, *at)
                .map(|note| Cursor::Note(note.at.clone())),
        }
    }
}

/// 这一条处理路径是**最外层**的吗：没有另一条勾着的**文件夹**把它包在里面。
///
/// 与 [`super::state::Session::nested_in`] 同一条认法（按路径前缀，不碰盘），
/// 只是那一处答的是「被谁包着」、这一处答的是「包没包着」。
fn outermost(path: &Path, paths: &[NamedPath]) -> bool {
    !paths.iter().any(|other| {
        other.on && other.path != path && !other.is_archive() && path.starts_with(&other.path)
    })
}

/// 这一卷归哪一条处理路径：**最外层**那几条里包着它的那一条（最长的那一条前缀，
/// 而那几条互不嵌套，因此至多一条真包得住它）。
fn owner<'a>(root: &Path, outer: &'a [&NamedPath]) -> Option<&'a Path> {
    outer
        .iter()
        .map(|named| named.path.as_path())
        .filter(|path| root.starts_with(path))
        .max_by_key(|path| path.as_os_str().len())
}

/// 屏上这一个目录写成什么名字：它落在那条处理路径底下的那一截（`集英社/海贼王`）；
/// 目录就是那条处理路径本身、或者根本不在它底下（跨路径合并进来的那几条）时，是最后一段。
fn label_of(directory: &Path, named: &Path) -> String {
    directory
        .strip_prefix(named)
        .ok()
        .filter(|rest| !rest.as_os_str().is_empty())
        .map(|rest| rest.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| directory.display().to_string())
        })
}

/// 报告末尾那两小结拆散了挂回各自的位置：一处无法访问的地方一行，一个节点底下的
/// 非漫画文件合成一行。认它归谁的办法与卷一样——落在哪一条处理路径底下。
fn attach_notes(
    nodes: &mut [Node],
    non_volume_files: &[NonVolumeFile],
    unreachable_places: &[UnreachablePlace],
) {
    let bases: Vec<PathBuf> = nodes.iter().map(base_of).collect();
    // **一处归谁，看它落在哪个节点底下**（最长的那一条前缀）：跨路径合并之后，
    // 那一条处理路径本身已经不是任何一个节点了（它是一个压缩包，节点是它的父目录），
    // 回头去认处理路径会把那几条备注整个丢掉。
    let which = |path: &Path| -> Option<usize> {
        bases
            .iter()
            .enumerate()
            .filter(|(_, base)| path.starts_with(base))
            .max_by_key(|(_, base)| base.as_os_str().len())
            .map(|(at, _)| at)
    };
    let mut hung: Vec<(usize, Note)> = Vec::new();
    for place in unreachable_places {
        let Some(node) = which(&place.path) else {
            continue;
        };
        hung.push((
            node,
            Note {
                kind: NoteKind::Unreachable,
                at: place.path.clone(),
                label: "无法访问".to_owned(),
                what: format!("{}/", trimmed(&place.path, &bases[node])),
                brief: place.reason.clone(),
                said: vec![(place.path.clone(), place.reason.clone())],
            },
        ));
    }
    // 非漫画文件一个节点一行：那一小结抬头数的是整趟，这一行数的是它装着的那几个。
    for (node, base) in bases.iter().enumerate() {
        let mine: Vec<&NonVolumeFile> = non_volume_files
            .iter()
            .filter(|file| which(&file.path) == Some(node))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let names: Vec<String> = mine.iter().map(|file| trimmed(&file.path, base)).collect();
        hung.push((
            node,
            Note {
                kind: NoteKind::NonVolume,
                at: base.clone(),
                label: format!("已忽略 {} 个文件", mine.len()),
                what: names.join("、"),
                brief: crate::render::non_volume_heading(mine.len()),
                said: mine
                    .iter()
                    .map(|file| {
                        (
                            file.path.clone(),
                            crate::render::non_volume_reason(&file.reason),
                        )
                    })
                    .collect(),
            },
        ));
    }
    for (node, note) in hung {
        nodes[node].notes.push(note);
    }
}

/// 这一个节点挂在哪条路径上（分区是那条处理路径，顶格目录行是那个目录）。
fn base_of(node: &Node) -> PathBuf {
    match &node.shape {
        Shape::Section { path, .. } => path.clone(),
        Shape::Directory(directory) => directory.path.clone(),
    }
}

/// 备注行里这一处写成什么：它落在这个节点底下的那一截。
fn trimmed(path: &Path, under: &Path) -> String {
    path.strip_prefix(under)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一条处理路径。
    fn named(path: &str, on: bool) -> NamedPath {
        NamedPath {
            path: PathBuf::from(path),
            on,
        }
    }

    /// 清点清单上的一卷（步数与源页数这一层不看）。
    fn listed(root: &str) -> SurveyedVolume {
        SurveyedVolume {
            root: PathBuf::from(root),
            steps: 3,
            source_pages: 1,
        }
    }

    /// 顶层那几行的样子，写成一句好读的话：分区带 `§`，目录行带缩进，卷行再缩一级。
    fn sketch(tree: &Tree, expanded: &BTreeSet<PathBuf>, roster: &[SurveyedVolume]) -> Vec<String> {
        tree.rows(expanded)
            .iter()
            .map(|row| match row {
                Row::Section { node } => match &tree.nodes[*node].shape {
                    Shape::Section { path, .. } => format!("§ {}", path.display()),
                    Shape::Directory(_) => unreachable!("分区那一行出自分区"),
                },
                Row::Directory { node, at, indent } => format!(
                    "{}{}",
                    "  ".repeat(usize::from(*indent)),
                    tree.directory(*node, *at).expect("目录行").label
                ),
                Row::Volume { at, indent } => format!(
                    "{}{}",
                    "  ".repeat(usize::from(*indent)),
                    roster[*at]
                        .root
                        .file_name()
                        .expect("卷名")
                        .to_string_lossy()
                ),
                Row::Note { node, at, .. } => {
                    format!("# {}", tree.note(*node, *at).expect("备注").label)
                }
                Row::Gap => String::new(),
                Row::Foot => format!("… {} 条没勾", tree.unchecked),
            })
            .collect()
    }

    /// **一条处理路径展开成两个及以上目录 → 分区**（票面第二条）：横线标题一行，
    /// 底下的目录行缩进一级，前后各一行空白；分区标题**停不上**。
    #[test]
    fn a_named_path_that_spreads_over_two_directories_becomes_a_section() {
        let paths = [named("/库", true)];
        let roster = [
            listed("/库/甲/第01卷"),
            listed("/库/甲/第02卷"),
            listed("/库/乙/第01卷"),
        ];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(tree.nodes.len(), 1);
        assert!(tree.nodes[0].is_section());
        let rows = tree.rows(&BTreeSet::new());
        assert_eq!(
            sketch(&tree, &BTreeSet::new(), &roster),
            ["§ /库", "  甲", "  乙", ""]
        );
        assert!(
            rows.iter().filter(|row| row.stop(&tree).is_some()).count() == 2,
            "分区标题与那一行空白停不上，停得住的只有两个目录行"
        );
    }

    /// **只展开成一个目录的处理路径直接是顶格目录行**（票面第二条）：不多一层没用的标题，
    /// 目录行不缩进。
    #[test]
    fn a_named_path_with_one_directory_is_a_top_level_directory_row() {
        let paths = [named("/库/棋魂", true)];
        let roster = [listed("/库/棋魂/第01卷"), listed("/库/棋魂/第02卷")];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(sketch(&tree, &BTreeSet::new(), &roster), ["棋魂"]);
        assert!(!tree.nodes[0].is_section());
    }

    /// **点名单卷的处理路径按父目录跨路径合并**（票面第二条）：八个压缩包合成一行，
    /// 排在它第一个成员出现的位置；**只合进一卷**的那一个照样是一行。
    #[test]
    fn named_archives_in_one_folder_merge_into_a_single_row_even_when_only_one() {
        let paths = [
            named("/下载/哀/第01卷.cbz", true),
            named("/下载/哀/第02卷.cbz", true),
            named("/别处/独/只此一卷.cbz", true),
        ];
        let roster = [
            listed("/下载/哀/第01卷.cbz"),
            listed("/下载/哀/第02卷.cbz"),
            listed("/别处/独/只此一卷.cbz"),
        ];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(sketch(&tree, &BTreeSet::new(), &roster), ["哀", "独"]);
        let expanded: BTreeSet<PathBuf> = [PathBuf::from("/下载/哀"), PathBuf::from("/别处/独")]
            .into_iter()
            .collect();
        assert_eq!(
            sketch(&tree, &expanded, &roster),
            ["哀", "  第01卷.cbz", "  第02卷.cbz", "独", "  只此一卷.cbz"],
            "合并行排在第一个成员出现的位置，只有一卷的那一条也是一行"
        );
    }

    /// **嵌套的处理路径并进最外层那一条**（票面第二条）：里层那一条不单独成节点，
    /// 它的卷归外面那一条——与清点的去重一致。
    #[test]
    fn a_nested_named_path_folds_into_the_outermost_one() {
        let paths = [named("/库", true), named("/库/乙", true)];
        let roster = [
            listed("/库/甲/第01卷"),
            listed("/库/乙/第01卷"),
            listed("/库/乙/第02卷"),
        ];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(tree.nodes.len(), 1, "里层那一条不单独成节点");
        assert_eq!(
            sketch(&tree, &BTreeSet::new(), &roster),
            ["§ /库", "  甲", "  乙", ""]
        );
    }

    /// **没勾的处理路径不进树，末行暗淡地说一句**（票面第二条）。
    #[test]
    fn unchecked_named_paths_stay_out_of_the_tree_and_the_last_line_says_so() {
        let paths = [named("/库/棋魂", true), named("/下载/插图", false)];
        let roster = [listed("/库/棋魂/第01卷")];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(tree.unchecked, 1);
        assert_eq!(
            sketch(&tree, &BTreeSet::new(), &roster),
            ["棋魂", "… 1 条没勾"]
        );
        assert!(
            tree.rows(&BTreeSet::new())
                .last()
                .expect("末行")
                .stop(&tree)
                .is_none(),
            "末行那一句停不上"
        );
    }

    /// **备注行挂在它所在那条处理路径的末尾**：无法访问的地方一处一行，非漫画文件合成一行，
    /// 行尾那一句出自报告（[`crate::render::non_volume_heading`]）。
    #[test]
    fn notes_hang_at_the_end_of_the_node_they_belong_to() {
        let paths = [named("/库", true)];
        let roster = [listed("/库/甲/第01卷"), listed("/库/乙/第01卷")];
        let tree = Tree::of(
            &paths,
            &roster,
            &[tonefit::NonVolumeFile {
                path: PathBuf::from("/库/答案.txt"),
                reason: tonefit::NonVolumeReason::NeitherPageNorArchive,
            }],
            &[tonefit::UnreachablePlace {
                path: PathBuf::from("/库/私藏"),
                reason: "打不开".to_owned(),
            }],
        );
        assert_eq!(
            sketch(&tree, &BTreeSet::new(), &roster),
            [
                "§ /库",
                "  甲",
                "  乙",
                "# 无法访问",
                "# 已忽略 1 个文件",
                ""
            ]
        );
        let notes = &tree.nodes[0].notes;
        assert_eq!(
            (notes[0].what.as_str(), notes[1].what.as_str()),
            ("私藏/", "答案.txt")
        );
        assert_eq!(notes[1].brief, crate::render::non_volume_heading(1));
        // **这一条装着的那几处**：说明卡的全文从它拼（`super::cover::Card`），
        // 路径原样带、原因走报告那一处（[`crate::render::non_volume_reason`]）。
        assert_eq!(
            notes[0].said,
            [(PathBuf::from("/库/私藏"), "打不开".to_owned())]
        );
        assert_eq!(
            notes[1].said,
            [(
                PathBuf::from("/库/答案.txt"),
                crate::render::non_volume_reason(&tonefit::NonVolumeReason::NeitherPageNorArchive)
            )]
        );
        // 光标记着的身份换回树上的位置：掀说明卡那一下问它。
        assert_eq!(tree.locate_note(&notes[0].at), Some((0, 0)));
        assert_eq!(tree.locate_note(&notes[1].at), Some((0, 1)));
        assert_eq!(tree.locate_note(Path::new("/库/没这一条")), None);
    }

    /// **次序照发现，不因状态重排**：顶层按处理路径的先后，目录按它头一卷在清单上的先后。
    #[test]
    fn the_order_follows_the_survey_and_never_the_state() {
        let paths = [named("/乙", true), named("/甲", true)];
        let roster = [listed("/甲/一/第01卷"), listed("/乙/二/第01卷")];
        let tree = Tree::of(&paths, &roster, &[], &[]);
        assert_eq!(sketch(&tree, &BTreeSet::new(), &roster), ["二", "一"]);
    }
}
