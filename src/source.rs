//! 源：输入容器抽象。按阅读顺序吐出页，非图片文件原样透传。
//!
//! 目录与 CBZ 在这里收敛成同一个 [`Volume`]，调用方不必区分是哪一种。

use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::decode;

/// 归档卷的扩展名。CBZ 就是 ZIP，本版本只认这一个扩展名。
pub const ARCHIVE_EXTENSION: &str = "cbz";

/// 卷的容器形态。输出容器随输入而定：输入是 CBZ，输出也是 CBZ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Directory,
    Archive,
}

/// 一个卷：一次处理调用的作用域。
pub struct Volume {
    /// 卷标识：源目录路径，或归档文件路径。
    pub root: PathBuf,
    /// 卷名。目录取目录名，归档取去掉扩展名的文件名。
    pub name: String,
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
    /// 本卷的输出位置：目录卷是同名目录，归档卷是同名归档文件。
    pub fn output_path(&self, output_root: &Path) -> PathBuf {
        match self.container {
            Container::Directory => output_root.join(&self.name),
            Container::Archive => output_root.join(format!("{}.{ARCHIVE_EXTENSION}", self.name)),
        }
    }

    /// 一个成员的身份：卷根接上它的相对路径。报告与错误信息用它指人。
    ///
    /// 归档成员因此长成 `卷.cbz/001.png`——包内没有文件系统路径，这是它最接近的说法。
    pub fn identity(&self, member: &Member) -> PathBuf {
        self.root.join(&member.relative)
    }
}

/// 卷内的一个成员：一页，或一个原样透传的文件。
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
    entry: usize,
}

/// 取成员字节的那一半。
pub enum Reader {
    Directory { root: PathBuf },
    Archive(zip::ZipArchive<BufReader<File>>),
}

impl Reader {
    /// 目录卷的卷根。归档卷没有——它的成员待在一个游标背后，因此并发读无从谈起
    /// （见 `crate::read`）。
    pub fn directory_root(&self) -> Option<&Path> {
        match self {
            Reader::Directory { root } => Some(root),
            Reader::Archive(_) => None,
        }
    }

    /// 读出一个成员的原始字节。
    pub fn read(&mut self, member: &Member) -> Result<Vec<u8>> {
        match self {
            Reader::Directory { root } => read_file(&root.join(&member.relative)),
            Reader::Archive(archive) => {
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
/// 目录卷的读取只此一条路：串行那条与并发那条都走它（见 `crate::read`），
/// 「读不出来」那句话才不会有两个版本。
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("读 {}", path.display()))
}

/// 打开一个卷。源只读，这里不写任何东西。
pub fn open(path: &Path) -> Result<Volume> {
    if path.is_dir() {
        return open_directory(path);
    }
    if !path.exists() {
        bail!("{} 不存在", path.display());
    }
    if !is_archive(path) {
        bail!(
            "{} 既不是目录，也不是 .{ARCHIVE_EXTENSION}：一个卷是一个目录或一个 CBZ 归档",
            path.display()
        );
    }
    open_archive(path)
}

/// 扩展名是否表明这是一个归档卷。大小写不敏感。
fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(ARCHIVE_EXTENSION))
}

fn open_directory(root: &Path) -> Result<Volume> {
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("{} 没有目录名，无法决定输出位置", root.display()))?;

    let mut members = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.with_context(|| format!("遍历 {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("遍历结果恒在卷根之下")
            .to_path_buf();
        // 遍历时已经 stat 过一次，这里拿的是那一次的结果，不再多问一次文件系统。
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
        name,
        container: Container::Directory,
        pages,
        extras,
        reader: Reader::Directory {
            root: root.to_path_buf(),
        },
    })
}

fn open_archive(path: &Path) -> Result<Volume> {
    let name = path
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("{} 没有文件名，无法决定输出位置", path.display()))?;

    let file = File::open(path).with_context(|| format!("打开 {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).with_context(|| {
        format!(
            "读不出 {} 的归档结构：CBZ 就是 ZIP，这个文件可能已损坏或根本不是 ZIP",
            path.display()
        )
    })?;

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
        name,
        container: Container::Archive,
        pages,
        extras,
        reader: Reader::Archive(archive),
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
/// 归档来自外部，名字里的 `..`、绝对路径与盘符会把输出写到卷外，一律拒绝而不是就地修正——
/// 修正会静默改变成员的去处，而这种归档本就不该被当成一个卷。
fn relative_path(name: &str) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for part in name.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(part)), None) => relative.push(part),
            _ => bail!("片段 {part} 不是一个普通的路径分量"),
        }
    }
    if relative.as_os_str().is_empty() {
        bail!("整个名字里没有一个普通的路径分量");
    }
    Ok(relative)
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
fn reading_order(a: &Path, b: &Path) -> Ordering {
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
