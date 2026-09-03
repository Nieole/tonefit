//! 源：输入容器抽象。按阅读顺序吐出页，非图片文件原样透传。
//!
//! 目录与归档在这里收敛成同一个 [`Volume`]，调用方不必区分是哪一种。
//!
//! **收敛的是读取与输出，不是卷边界**（ADR 0014）：归档的边界由打包者定死了，
//! 内部结构照收；目录的边界可以再分，因此[目录卷只收直接那一层](open_directory)，
//! 子目录与躺在里面的归档各自成卷。哪些路径成卷由 [`crate::discover`] 定，本模块只管
//! 「给定一个卷根，它里面有什么」。

use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::decode;

/// 点名一个归档卷时认得的扩展名。
///
/// `.cbz` 就是 ZIP，`.zip` 与它是同一种字节，因此走同一条读法、不各开一条路。
/// `.rar` / `.7z` 也在格式集里，但本版本点名即拒——它们要先摊到临时目录才谈得上读
/// （见 ADR 0015）。
const ARCHIVE_EXTENSIONS: [&str; 2] = ["cbz", "zip"];

/// 归档卷的**输出**扩展名：一律 `.cbz`，输入是哪一个都不影响它。
///
/// 产物是给阅读器的，而 `.cbz` 是那边认的名字（ADR 0015）。归一在这里发生，
/// 因此幂等的去处也按它算：`第10话.zip` 与 `第10话.cbz` 指着同一份输出。
pub const OUTPUT_ARCHIVE_EXTENSION: &str = "cbz";

/// 卷的容器形态。输出容器随输入而定：输入是归档，输出也是归档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Directory,
    Archive,
}

/// 一个卷：一次处理调用的作用域。
pub struct Volume {
    /// 卷标识：源目录路径，或归档文件路径。
    pub root: PathBuf,
    /// 本卷的容器形态。
    pub container: Container,
    /// 按阅读顺序排好的页。
    pub pages: Vec<Member>,
    /// 非图片成员，按同一套顺序排好。它们原样透传，不解码也不改动。
    pub extras: Vec<Member>,
    /// 取字节的那一半。与成员表分开放，好让调用方一边遍历成员一边读。
    pub reader: Reader,
}

impl Volume {
    /// 一个成员的身份：卷根接上它的相对路径。报告与错误信息用它指人。
    ///
    /// 归档成员因此长成 `卷.cbz/001.png`——包内没有文件系统路径，这是它最接近的说法。
    pub fn identity(&self, member: &Member) -> PathBuf {
        self.root.join(&member.relative)
    }
}

/// 卷内的一个成员：一页，或一个原样透传的文件。
///
/// **可克隆**：读取层的并发那一支要把这张表整个搬进读取线程里（线程活得比那次借用长，
/// 借不过去），而一个成员就是三个小字段（见 `crate::read`）。
#[derive(Clone)]
pub struct Member {
    /// 相对卷根的路径。输出按它镜像出结构。
    pub relative: PathBuf,
    /// 这个成员摊开来有多少字节。
    ///
    /// 在这里而不是读的时候现问：读取层要在**读之前**按它占住在途字节的预算
    /// （13 号票，见 `crate::read`），而那一刻问文件系统就等于每页多一次 stat——
    /// 机械盘上那是一次真实的寻道。两边都是现成的：目录卷遍历时顺手拿到，
    /// 归档卷在中央目录里写着。
    pub bytes: u64,
    /// 归档内的成员序号。目录卷不用它。
    ///
    /// 它是**中央目录里的下标**，因此同一个归档文件另开一个句柄也指得回同一个成员——
    /// [`Reader::independent`] 靠的就是这一条。
    entry: usize,
}

/// 取成员字节的那一半。
///
/// 它是**可再开的**：[`independent`](Self::independent) 按同一个源要出第二份，两份互不影响。
/// 读取层的并发那一支给每条读取线程发一份（见 `crate::read`）。
///
/// # 一趟同时开着几个句柄
///
/// **这笔账只有这一处出处**，别处引这个小节名、不复述。来处只有三个：
///
/// - **正在处理的那一卷**：归档卷 **1** 个（[`Reader::Archive`] 那个 `ZipArchive`），
///   目录卷 **0** 个（[`Reader::Directory`] 只是一个卷根路径）。卷一个一个地处理、
///   处理完当场析构，而预扫**一个都不攥**（见 `crate::survey`）——这一格因此是 1，
///   **不是这一趟有几个卷**。
/// - **正在读的那几个成员**：一条读取线程一个（见 [`independent`](Self::independent)
///   与 `crate::read` 的 `reads`）。真读源字节的只有两处，各按[读取计划](crate::medium::IoPlan)
///   里自己那一格派：幂等那一道 `min(fingerprint.count, 成员数)` 条、
///   第一遍 `min(readers.count, 成员数)` 条（第二遍不读源，它写的是第一遍的结果）。
///   **归档卷**上第一遍恒为一条顺序扫、用的就是卷自己那个读取端，因此只有幂等那一道
///   多开句柄，`Reads` 一析构就全收。**目录卷**上两处都并发，但发下去的是几份卷根，
///   只有**正在读**的那个成员占一个句柄（本模块的 `read_file` 开完就关）。
/// - **写出那一侧**：缓存的溢写文件至多 1 个（`crate::cache`；第一遍起、整卷持有，
///   因此**与第一遍那几条读取重叠**），输出容器至多 1 个（`crate::sink`；归档卷是一个
///   打开的文件，目录卷逐个成员开完就关，第二遍才有）。
///
/// 三格相加，峰值封顶在 **`min(并发度, 这一卷的成员数) + 3`**，而并发度至多是核数
/// （[读取计划](crate::medium::IoPlan)按 `--io-mode` 与介质定它）。主项在两种容器上同形：
/// 归档卷的幂等那一段是「卷自己那一个 + 几条读取线程」，即 `1 + min(fingerprint.count, 成员数)`。
/// 自变量只有三个：**并发度**、**这一卷的成员数**、**容器形态**。
/// **这一趟有几个卷不在里面**——它是**核数量级，不是卷数量级**。
/// 发现落地之后这一句更要紧了：点名一个库，卷数由用户的目录树说了算（ADR 0014）。
///
/// ## 没有一道句柄上限，为什么
///
/// 峰值封顶在核数加个位数，而一个进程能同时开着的句柄比它多几个数量级——两个平台各自的
/// 上限见 `docs/measurements.md` 的《一个进程能同时开多少个文件句柄》，
/// 最紧的那一档也要上千个核才够得着。划一道线只能划在核数上，
/// 那等于为一个够不着的上界引一套句柄记账（这条判定出自 `p2-loose-ends/12`）。
///
/// 句柄真的开不出来（进程的额度被别的东西耗光）时也不撞出一个错：读取层用开得出来的
/// 那几条接着做（见 `crate::read` 的 `reads`），而 `--io-mode serial` 把中间那一格按回 1。
pub enum Reader {
    Directory {
        root: PathBuf,
    },
    Archive {
        /// 归档文件的路径。再开一份独立句柄只要它。
        path: PathBuf,
        archive: zip::ZipArchive<BufReader<File>>,
    },
}

impl Reader {
    /// 再要一份读取端：读的是同一个源，与自己**互不影响**。
    ///
    /// 目录卷只是把卷根抄一份，不碰盘。归档卷是**另开一个文件句柄、另解一遍中央目录**——
    /// 一个 `ZipArchive` 就是一个游标，几条读取线程共用一个游标无从谈起，各开各的才谈得上并发。
    ///
    /// 归档卷这一份因此**不便宜**：一次开文件加一次中央目录解析。按**读取线程**要一份、
    /// 一道读取要一次，不是按成员要一次。一趟同时因此开着几个，见本类型的
    /// 《一趟同时开着几个句柄》。
    pub fn independent(&self) -> Result<Reader> {
        match self {
            Reader::Directory { root } => Ok(Reader::Directory { root: root.clone() }),
            Reader::Archive { path, .. } => Ok(Reader::Archive {
                path: path.clone(),
                archive: open_archive_handle(path)?,
            }),
        }
    }

    /// 读出一个成员的原始字节。
    pub fn read(&mut self, member: &Member) -> Result<Vec<u8>> {
        match self {
            Reader::Directory { root } => read_file(&root.join(&member.relative)),
            Reader::Archive { archive, .. } => {
                let mut entry = archive
                    .by_index(member.entry)
                    .with_context(|| format!("取归档成员 {}", member.relative.display()))?;
                let mut bytes = Vec::with_capacity(entry.size() as usize);
                entry
                    .read_to_end(&mut bytes)
                    .with_context(|| format!("解出归档成员 {}", member.relative.display()))?;
                Ok(bytes)
            }
        }
    }
}

/// 读一个文件的全部字节。
///
/// 目录卷的读取只此一条路：串行那条与并发那条都经 [`Reader::read`] 走它，
/// 「读不出来」那句话才不会有两个版本。读取层不再自己调它——并发那一支也拿一个
/// [`Reader`] 去读（见 `crate::read`），因此这里不必是 `pub`。
fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("读 {}", path.display()))
}

/// 打开一个卷。源只读，这里不写任何东西。
pub fn open(path: &Path) -> Result<Volume> {
    match identity_of(path)?.1 {
        Container::Directory => open_directory(path),
        Container::Archive => open_archive(path),
    }
}

/// 卷名与容器形态。两者都只看路径，不看内容。
///
/// **它认「这是不是一个卷」**：路径既不是目录也不是认得的归档时当场拒绝，
/// 那句话说得出格式集（见 [`listed_archive_extensions`]）。因此[点名的那个路径](open)
/// 与[发现的起点](crate::discover::of)都经它。
///
/// 发现**往下走**的时候不经它：候选的形态在列目录时就知道了，只差一个卷名，走 [`name_of`]。
pub(crate) fn identity_of(path: &Path) -> Result<(String, Container)> {
    if path.is_dir() {
        let name = name_of(path, Container::Directory)
            .with_context(|| format!("{} 没有目录名，说不出这是哪一个卷", path.display()))?;
        return Ok((name, Container::Directory));
    }
    if !path.exists() {
        bail!("{} 不存在", path.display());
    }
    if !is_archive(path) {
        bail!(
            "{} 既不是目录，也不是认得的归档：一个卷是一个目录或一个归档（{}）",
            path.display(),
            listed_archive_extensions()
        );
    }
    let name = name_of(path, Container::Archive)
        .with_context(|| format!("{} 没有文件名，说不出这是哪一个卷", path.display()))?;
    Ok((name, Container::Archive))
}

/// 卷名：目录取目录名，归档取去掉扩展名的文件名。形态已经知道时取它。
///
/// 路径连一个普通的末级分量都没有（`/`、`.`、`..`）时是 `None`——那样的路径给不出卷名，
/// 也就决定不了输出位置。
pub(crate) fn name_of(path: &Path, container: Container) -> Option<String> {
    let raw = match container {
        Container::Directory => path.file_name(),
        Container::Archive => path.file_stem(),
    }?;
    Some(raw.to_string_lossy().into_owned())
}

/// 卷名 + 容器形态 → 输出**那一级的名字**。目录卷是同名目录，归档卷是同名 `.cbz`。
///
/// 归档卷的扩展名在这里**归一**：输入的扩展名一点都不带过来。去处因此只取决于卷名与
/// 它在源里的位置，同一目录下同名的 `.zip` 与 `.cbz` 也就撞在一起——那道拒绝见 `crate::run`。
///
/// 整条去处由 [`crate::discover`] 拼出来：输出镜像源的结构，这里只管末一级
/// （见 ADR 0014 决定第 4 条）。
pub(crate) fn output_name_of(name: &str, container: Container) -> String {
    match container {
        Container::Directory => name.to_owned(),
        Container::Archive => format!("{name}.{OUTPUT_ARCHIVE_EXTENSION}"),
    }
}

/// 扩展名是否表明这是一个归档卷。大小写不敏感。
pub(crate) fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ARCHIVE_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// 认得的归档扩展名，拼成给人看的一串（`.cbz / .zip`）。
///
/// 拒绝那句话由它拼出，格式集与措辞因此只有一个出处：往 [`ARCHIVE_EXTENSIONS`] 里加一项，
/// 那句话自己跟着走。
fn listed_archive_extensions() -> String {
    ARCHIVE_EXTENSIONS
        .iter()
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 目录卷：**只收直接那一层**（ADR 0014 决定第 2 条，理由见本模块抬头）。
///
/// 子目录不是这一卷的成员——它自己是不是卷由发现说了算（见 [`crate::discover`]），
/// 而一页被两个卷各收一次就会被处理两遍。躺在这一层的**归档**同理：它是一个卷，
/// 不是这一卷的透传文件；当透传搬过去，那批 cbz 就又原样进了输出。
///
/// 符号链接与 junction **不跟进**：`DirEntry::file_type` 问的是链接自己，不是它指向的东西，
/// 因此链接既不当成员也不当子目录。
fn open_directory(root: &Path) -> Result<Volume> {
    let mut members = Vec::new();
    let entries =
        std::fs::read_dir(root).with_context(|| format!("列出 {} 这一层", root.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("列出 {} 这一层", root.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("问 {:?} 是什么", entry.path().display()))?;
        if !file_type.is_file() {
            continue;
        }
        let relative = PathBuf::from(entry.file_name());
        if is_junk(&relative) || is_archive(&relative) {
            continue;
        }
        // 列目录时已经 stat 过一次，这里拿的是那一次的结果，不再多问一次文件系统。
        // 问不出大小的成员按 0 算：它只是在读取层的预算上不占位，读法一点不变。
        let bytes = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        members.push(Member {
            relative,
            bytes,
            entry: 0,
        });
    }

    let (pages, extras) = split_and_sort(members);
    Ok(Volume {
        root: root.to_path_buf(),
        container: Container::Directory,
        pages,
        extras,
        reader: Reader::Directory {
            root: root.to_path_buf(),
        },
    })
}

/// 在一个归档文件上开一个句柄，中央目录跟着解一遍。开卷那一次与再开一份
/// （[`Reader::independent`]）共用它——「这个文件读不出归档结构」那句话因此只有一个版本。
fn open_archive_handle(path: &Path) -> Result<zip::ZipArchive<BufReader<File>>> {
    let file = File::open(path).with_context(|| format!("打开 {}", path.display()))?;
    zip::ZipArchive::new(BufReader::new(file)).with_context(|| {
        format!(
            "读不出 {} 的归档结构：CBZ 就是 ZIP，这个文件可能已损坏或根本不是 ZIP",
            path.display()
        )
    })
}

/// 归档卷：**内部结构照收**。它与 [`open_directory`] 的不对称由本模块抬头那一段交代。
fn open_archive(path: &Path) -> Result<Volume> {
    let mut archive = open_archive_handle(path)?;

    let mut members = Vec::with_capacity(archive.len());
    for entry in 0..archive.len() {
        let file = archive
            .by_index_raw(entry)
            .with_context(|| format!("读 {} 的第 {entry} 个成员", path.display()))?;
        if file.is_dir() {
            continue;
        }
        let name = decode_name(file.name_raw(), file.name());
        let relative = relative_path(&name)
            .with_context(|| format!("{} 的成员名 {name} 不能当作输出路径", path.display()))?;
        if is_junk(&relative) {
            continue;
        }
        let bytes = file.size();
        members.push(Member {
            relative,
            bytes,
            entry,
        });
    }
    strip_wrapper_directory(&mut members);

    let (pages, extras) = split_and_sort(members);
    Ok(Volume {
        root: path.to_path_buf(),
        container: Container::Archive,
        pages,
        extras,
        reader: Reader::Archive {
            path: path.to_path_buf(),
            archive,
        },
    })
}

/// 分成页与透传文件两摞，各自按阅读顺序排好。
fn split_and_sort(mut members: Vec<Member>) -> (Vec<Member>, Vec<Member>) {
    members.sort_by(|a, b| reading_order(&a.relative, &b.relative));
    members
        .into_iter()
        .partition(|member| decode::is_page(&member.relative))
}

/// 成员名的解码启发式。
///
/// ZIP 只有「这个名字是 UTF-8」一个标志位，而中文环境下的打包工具常写 GBK 且不置位，
/// 按规范的 cp437 去解就是一串乱码。因此顺序是：本身是合法 UTF-8 就当 UTF-8
/// （置了位的必然合法，没置位但实为 UTF-8 的也在这里被认出来）；否则按 GBK 解；
/// GBK 也解不出才退回 `cp437`——即 `zip` 按规范给出的那个名字，至少不丢字节。
///
/// GBK 的中文双字节里后一字节落在 0x40..=0xFE，与 UTF-8 续字节要求的 0x80..=0xBF 大量不交，
/// 所以「本身是合法 UTF-8」这一步几乎不会把 GBK 名误判进来。
///
/// 认下的代价：非 UTF-8 的名字一律当 GBK，这是**假定**而不是判别。Shift-JIS 的名字多半也能被
/// GBK 解出来，解成的是一串汉字乱码，且没有下一档去兜。日文片源要正确处理得先有判别，
/// 那是另一件事——本版本只按票面要求处理 GBK。
fn decode_name(raw: &[u8], cp437: &str) -> String {
    if let Ok(name) = std::str::from_utf8(raw) {
        return name.to_owned();
    }
    let (decoded, malformed) = encoding_rs::GBK.decode_without_bom_handling(raw);
    if malformed {
        return cp437.to_owned();
    }
    decoded.into_owned()
}

/// 成员名转成相对路径。
///
/// 分隔符两种都认：ZIP 规范只写 `/`，而老式 Windows 打包工具写的是 `\`。
/// 反斜杠在这里就归一掉，两种包于是往下走同一条路。不归一的话它在 Windows 上撞进
/// 下面那条为路径穿越准备的拒绝、整卷被拒，在别的平台上却被当成文件名里的一个普通字符——
/// 同一份归档两个平台两种结果。
///
/// 代价是文件名里真带一个反斜杠的成员会被劈成两级。那种名字在 Windows 上根本建不出来，
/// 而反斜杠分隔的包是眼下就在流通的东西。
///
/// 名字里的 `..` 与盘符会把输出写到卷外，一律拒绝而不是就地修正——修正会静默改变成员的
/// 去处，而这种归档本就不该被当成一个卷。**拒绝说的是「这个名字指着卷外」**，
/// 与分隔符写法无关：反斜杠写的穿越报的仍是穿越。
///
/// 盘符自己认而不交给 `Path`：`C:` 在 Windows 上是路径前缀、在别的平台上是一个普通名字，
/// 交给 `Path` 就等于让平台决定这份归档收不收。
fn relative_path(name: &str) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            bail!("片段 {part} 指着上一级，会走出卷外");
        }
        if is_drive_letter(part) {
            bail!("片段 {part} 是一个盘符，会走出卷外");
        }
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(part)), None) => relative.push(part),
            // 分隔符已经归一，`.`、`..`、盘符也都在上面拦掉了——`Path` 的其余几种分量
            // （根、UNC、设备名）无一不带分隔符，因此这条兜底够不着。留着是因为
            // 「片段最终必须是一个普通分量」这条不变量要写在它成立的地方，
            // 而不是靠上面那三条各自的正确性去推。
            _ => bail!("片段 {part} 不是一个普通的路径分量"),
        }
    }
    if relative.as_os_str().is_empty() {
        bail!("整个名字里没有一个普通的路径分量");
    }
    Ok(relative)
}

/// 这个片段是不是盘符：一个 ASCII 字母接一个冒号。`C:` 与 `C:001.png` 都算。
fn is_drive_letter(part: &str) -> bool {
    let bytes = part.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// 打包环境留下的目录：整个子树都不是卷的内容。
///
/// `__MACOSX` 是 macOS 的「压缩」菜单写出来的兄弟目录，里面按原结构镜像着每个成员的
/// AppleDouble 边车；其余几个是各家文件管理器、版本控制与 NAS 自己的索引与回收站目录。
///
/// 后四个是**发现**才撞得到的（ADR 0014）：从前只在归档成员名里比它，一份包里不会有
/// `.git`；如今发现要走进真实的库目录，而回收站里躺着的正是用户删掉的那些卷——
/// 走进去就是把删掉的东西又转一遍。
///
/// 这一组在**两处**同时作数：卷内不当成员（见 [`is_junk`]），发现时整棵子树不进去
/// （见 [`is_junk_directory`]）。
const JUNK_DIRECTORIES: [&str; 10] = [
    "__MACOSX",
    ".Spotlight-V100",
    ".Trashes",
    ".TemporaryItems",
    ".fseventsd",
    "@eaDir",
    ".git",
    "#recycle",
    "@Recycle",
    ".@__thumb",
];

/// 打包环境留下的单个文件。
const JUNK_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

/// AppleDouble 边车文件的名字前缀：本体叫什么它就叫 `._` 加什么，**扩展名照抄**。
const APPLE_DOUBLE_PREFIX: &str = "._";

/// 这个成员是不是打包环境留下的垃圾。
///
/// 它们既不当页也不当透传文件：边车的扩展名照抄本体，当页解必然解不出图，
/// 整卷因此进隔离目录还被插上白页；当透传文件搬过去，则是把打包环境的产物带进成品。
///
/// 目录卷与归档卷共用这一条：两者在源之下同形，同一份卷解开到磁盘上再处理不该换一个答案。
fn is_junk(relative: &Path) -> bool {
    let mut parts = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str());
    if parts.any(|part| is_one_of(&JUNK_DIRECTORIES, part)) {
        return true;
    }
    let Some(name) = relative.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.starts_with(APPLE_DOUBLE_PREFIX) || is_one_of(&JUNK_FILES, name)
}

/// 这个目录名是不是打包环境留下的目录。发现走到它就整棵子树不进去（见 [`crate::discover`]）。
///
/// 与 [`is_junk`] 同一份名单：同一个 `__MACOSX`，在归档成员名里不算成员，
/// 在盘上也不该被走进去找卷。
pub(crate) fn is_junk_directory(name: &str) -> bool {
    is_one_of(&JUNK_DIRECTORIES, name)
}

/// 名字命中这一组里的哪一个吗。
///
/// 大小写不敏感地比：这些名字来处的文件系统本就不分大小写，`Thumbs.db` 各家写法不一。
fn is_one_of(names: &[&str], name: &str) -> bool {
    names.iter().any(|junk| junk.eq_ignore_ascii_case(name))
}

/// 剥掉包内统一的目录前缀。
///
/// 打包工具惯常把整卷塞进一个目录再压缩，那一层是打包的产物、不是卷的结构，留着会让输出多一级。
///
/// 判据是**没有兄弟**，不是名字对不对得上卷名：一层目录若装下了整卷，它就不承担任何排序信息，
/// 剥掉不丢东西，而卷名五花八门（`volume-a`、`第01话`、`raw`），按名字认必然认漏。
/// 因此一剥到底——`a/b/001.png` 这样嵌两层的包会一路剥到 `001.png`。
/// 一旦某一层出现两个以上的名字（并列的章节目录），剥就停下：那一层开始承担顺序了。
///
/// 目录卷没有这一步：用户点名的那个目录就是卷根，里面的第一层是他自己的组织方式。
///
/// 垃圾成员在这一步之前就摘掉了（见 [`is_junk`]）：`__MACOSX` 与 `.DS_Store` 都是包装层的
/// **兄弟**，留着它们，这一层就一层都剥不掉。
fn strip_wrapper_directory(members: &mut [Member]) {
    loop {
        let mut wrapper: Option<Component> = None;
        for member in members.iter() {
            let mut components = member.relative.components();
            let (Some(first), Some(_)) = (components.next(), components.next()) else {
                return;
            };
            match wrapper {
                None => wrapper = Some(first),
                Some(shared) if shared == first => {}
                Some(_) => return,
            }
        }
        let Some(wrapper) = wrapper else { return };
        let wrapper = wrapper.as_os_str().to_os_string();
        for member in members.iter_mut() {
            member.relative = member
                .relative
                .strip_prefix(&wrapper)
                .expect("上一轮刚确认过这一层是全卷共有的")
                .to_path_buf();
        }
    }
}

/// 阅读顺序：逐层比路径分量，分量内数字段按数值比。
///
/// 字典序会把 `10.png` 排到 `2.png` 前面，而页号正是漫画的阅读顺序本身。
///
/// 卷内的成员与**发现出来的那批卷**共用它：一个作品目录下的 `第2话.cbz` 与 `第10话.cbz`
/// 要按同一条规矩排（见 [`crate::discover`]），不然报告里的卷序与卷内的页序两套说法。
pub(crate) fn reading_order(a: &Path, b: &Path) -> Ordering {
    let mut a = a.components();
    let mut b = b.components();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left), Some(right)) => {
                let left = left.as_os_str().to_string_lossy();
                let right = right.as_os_str().to_string_lossy();
                match natural(&left, &right) {
                    Ordering::Equal => continue,
                    ordering => return ordering,
                }
            }
        }
    }
}

/// 自然序：数字段按数值比，其余按大小写不敏感的字典序比。
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        if a.is_empty() || b.is_empty() {
            return a.len().cmp(&b.len());
        }
        let ordering = match (starts_with_digit(a), starts_with_digit(b)) {
            (true, true) => {
                let left = take_run(&mut a, true);
                let right = take_run(&mut b, true);
                compare_numbers(left, right)
            }
            (false, false) => {
                let left = take_run(&mut a, false);
                let right = take_run(&mut b, false);
                left.to_lowercase()
                    .cmp(&right.to_lowercase())
                    .then_with(|| left.cmp(right))
            }
            // 一边是数字一边不是，首字符已经能定序。
            _ => return a.cmp(b),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn starts_with_digit(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_digit())
}

/// 切下开头一段同类字符（`digits` 为真取数字段，否则取非数字段），
/// 返回它并让 `text` 指向剩下的部分。
fn take_run<'a>(text: &mut &'a str, digits: bool) -> &'a str {
    let end = text
        .find(|c: char| c.is_ascii_digit() != digits)
        .unwrap_or(text.len());
    let (run, rest) = text.split_at(end);
    *text = rest;
    run
}

/// 数字段按数值比：去掉前导零后先比位数再比字典序，位数不设上限；数值相同则前导零少的在前。
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let left = a.trim_start_matches('0');
    let right = b.trim_start_matches('0');
    left.len()
        .cmp(&right.len())
        .then_with(|| left.cmp(right))
        .then_with(|| a.len().cmp(&b.len()))
}
