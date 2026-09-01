//! 输出容器：收页与透传文件，写成目录或 CBZ。
//!
//! 与源对称：输入是目录就写目录，输入是 CBZ 就写 CBZ（[`crate::source::Container`] 定这件事）。
//!
//! 幂等要读回上一趟的输出，读的也是这个容器，因此 [`Written`] 落在这里：
//! 「一页在容器里怎么找」两个方向上是同一件事，分开写就是两份会走散的容器知识。
//!
//! 两种形态**都先写到临时容器、收尾时才改名到最终位置**（见 [`Sink`]）。

use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;

use crate::metadata::{PageRecord, RECORD_PREFIX};
use crate::source::Container;

/// 一个卷的输出容器。写完必须调用 [`Sink::finish`]。
///
/// 两种形态同形：**先写到一个临时容器，收尾时才改名到最终位置**。因此最终位置上
/// 要么是上一趟那一份、要么是这一趟完整的一份，中间那一份不出现——而收尾时最终位置
/// 整个被换掉，输出里于是只剩本趟写出的成员（见 [`DirectorySink`]、[`ArchiveSink`]）。
pub enum Sink {
    Directory(DirectorySink),
    /// 装箱是因为 `ZipWriter` 自带几 KB 的压缩状态，直接内嵌会让目录卷也背上这份体积。
    Archive(Box<ArchiveSink>),
}

impl Sink {
    /// 在 `path` 建出输出容器：目录卷是一个目录，归档卷是一个归档文件。
    ///
    /// 这一步建出来的是临时容器，`path` 此刻还没被碰——它要等 [`finish`](Sink::finish)。
    pub fn create(path: &Path, container: Container) -> Result<Self> {
        match container {
            Container::Directory => Ok(Sink::Directory(DirectorySink::create(path)?)),
            Container::Archive => Ok(Sink::Archive(Box::new(ArchiveSink::create(path)?))),
        }
    }

    /// 写一页。页是 PNG，已经 deflate 过一遍，归档里存原样不再压。
    pub fn write_page(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        self.write(relative, bytes, zip::CompressionMethod::Stored)
    }

    /// 写一个透传文件。内容逐字节照搬，只是换个容器。
    pub fn write_extra(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        self.write(relative, bytes, zip::CompressionMethod::Deflated)
    }

    fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        match self {
            Sink::Directory(directory) => directory.write(relative, bytes),
            Sink::Archive(archive) => archive.write(relative, bytes, compression),
        }
    }

    /// 收尾。两种容器都在这一步才落到最终位置。
    pub fn finish(self) -> Result<()> {
        match self {
            Sink::Directory(directory) => directory.finish(),
            Sink::Archive(archive) => archive.finish(),
        }
    }
}

/// 目录输出。
///
/// 先写到一个临时目录，收尾时把最终位置**整个**换掉。两件事各要它一半：
///
/// 一是**不产出半成品**：写到一半的目录里每一页都带着完全正确的记录，
/// 摆在文件管理器里与一本处理好的书没有分别。
///
/// 二是**清掉陈旧产物**：只覆盖本趟写出的文件的话，源里删掉的那一页会在输出里原地留着、
/// 还带着上一趟的记录，下一趟又被幂等跳过，从此永久留在输出里。
///
/// 归档卷本来就整个重写，两件事在它身上都不成立——同一条标准这才在两种容器形态上
/// 给出同一个答案。
pub struct DirectorySink {
    /// 最终位置。
    path: PathBuf,
    /// 正在写的临时目录。改名到位或析构之后为 `None`。
    partial: Option<PathBuf>,
}

impl DirectorySink {
    fn create(path: &Path) -> Result<Self> {
        let partial = partial_path(path);
        // 上一趟被硬停在半路、连析构都没跑到的临时目录，这一趟当垃圾清掉：
        // 留着它，里面的陈旧成员会混进本趟的输出。
        //
        // 临时名字是**推得出来**的，因此有一个远角上的代价：同一趟里另有一个卷就叫
        // `<名字>.partial` 时，删掉的是它已经写好的输出。挡这一下要在开工之前把点名的卷
        // 全枚举一遍、比一遍去处，那是预扫要做的事（ADR 0011），不在这一层。
        // 换成随机名字能躲开，但硬停留下的垃圾就再也认不出来、也没有下一趟去清它了。
        if partial.exists() {
            std::fs::remove_dir_all(&partial)
                .with_context(|| format!("清掉残留的临时目录 {}", partial.display()))?;
        }
        std::fs::create_dir_all(&partial)
            .with_context(|| format!("建输出目录 {}", partial.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            partial: Some(partial),
        })
    }

    fn write(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let root = self.partial.as_ref().expect("收尾之前临时目录恒在");
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建输出目录 {}", parent.display()))?;
        }
        std::fs::write(&path, bytes).with_context(|| format!("写 {}", path.display()))
    }

    /// 收尾：最终位置整个换成这一趟写出的那一份。
    ///
    /// 先腾位置再改名，而不是「把旧的改名到一边、放好新的、再删旧的」：多出来的那个
    /// 中间名字要么留在输出里让用户猜，要么还得为「删它又失败了」再写一手。
    /// 腾位置这一步失败就整趟失败，最终位置原样不动，临时目录由析构收走。
    fn finish(mut self) -> Result<()> {
        let partial = self.partial.clone().expect("收尾之前临时目录恒在");
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)
                .with_context(|| format!("腾出输出位置 {}", self.path.display()))?;
        }
        std::fs::rename(&partial, &self.path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), self.path.display()))?;
        // 改名成功，临时目录已经不在了。
        self.partial = None;
        Ok(())
    }
}

impl Drop for DirectorySink {
    fn drop(&mut self) {
        // 改名到位就没有临时目录了；还在就是中途出了错，别把半个卷留在输出里。
        if let Some(partial) = self.partial.take() {
            let _ = std::fs::remove_dir_all(partial);
        }
    }
}

/// 上一趟写出的输出容器，只读。幂等要问它两件事：一页里记着什么指纹，一个成员还在不在。
///
/// 与 [`Sink`] 对称，也共用同一套容器知识——归档成员名怎么拼只此一份（见 [`archive_name`]）。
/// 两个方向分成两个类型，因为它们的生命期不同：写那一侧要建容器、要收尾，
/// 读这一侧连打开都可能失败，而失败就是「重做」这个平常答案。
pub enum Written {
    Directory(PathBuf),
    Archive(Box<zip::ZipArchive<BufReader<File>>>),
}

impl Written {
    /// 打开上一趟的输出。容器根本不在就是 `None`——那是头一趟，没什么可比的。
    pub fn open(path: &Path, container: Container) -> Option<Self> {
        match container {
            Container::Directory => path
                .is_dir()
                .then(|| Written::Directory(path.to_path_buf())),
            Container::Archive => {
                let file = BufReader::new(File::open(path).ok()?);
                Some(Written::Archive(Box::new(zip::ZipArchive::new(file).ok()?)))
            }
        }
    }

    /// 读回一页里记着的那份记录：幂等那四项，加上这一张的来路。成员不在、
    /// 或它没有记录，就是 `None`（ADR 0006：读回 tEXt 比对）。
    ///
    /// 只读到第一个 IDAT 为止，一个像素都不解——成本停在这里，跳过一卷才比重做一卷便宜。
    pub fn record_of(&mut self, relative: &Path) -> Option<PageRecord> {
        match self {
            Written::Directory(root) => {
                PageRecord::read(BufReader::new(File::open(root.join(relative)).ok()?))
            }
            Written::Archive(archive) => {
                // 归档成员不能回退寻址，解码器却要得起 `Seek`：先取开头一截到内存里。
                // 只取一截而不是整页，理由见 `RECORD_PREFIX`。
                let mut prefix = Vec::new();
                archive
                    .by_name(&archive_name(relative))
                    .ok()?
                    .take(RECORD_PREFIX)
                    .read_to_end(&mut prefix)
                    .ok()?;
                PageRecord::read(Cursor::new(prefix))
            }
        }
    }

    /// 这个成员还在吗。透传文件不带记录，能问的只有在不在。
    pub fn holds(&mut self, relative: &Path) -> bool {
        match self {
            Written::Directory(root) => root.join(relative).is_file(),
            Written::Archive(archive) => archive.by_name(&archive_name(relative)).is_ok(),
        }
    }
}

/// 归档输出。
///
/// 先写到一个临时文件，收尾时才改名到位：中途失败的归档没有中央目录，是打不开的垃圾，
/// 不能让它顶着最终文件名留在输出里。目录卷同形，理由见 [`DirectorySink`]。
pub struct ArchiveSink {
    /// 最终位置。
    path: PathBuf,
    /// 正在写的临时文件。改名到位或析构之后为 `None`——与 [`DirectorySink`] 同一格。
    partial: Option<PathBuf>,
    /// 往临时文件里写的那个写入器。收尾时取走：中央目录一写完它就没用了，
    /// 而临时文件还要等改名，两者因此各占一格。
    writer: Option<zip::ZipWriter<BufWriter<File>>>,
}

impl ArchiveSink {
    fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建输出目录 {}", parent.display()))?;
        }
        let partial = partial_path(path);
        let file =
            File::create(&partial).with_context(|| format!("建输出归档 {}", partial.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            partial: Some(partial),
            writer: Some(zip::ZipWriter::new(BufWriter::new(file))),
        })
    }

    fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let name = archive_name(relative);
        let writer = self.writer.as_mut().expect("收尾之前写入器恒在");
        writer
            .start_file(
                &name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .with_context(|| format!("在 {} 里建成员 {name}", self.path.display()))?;
        writer
            .write_all(bytes)
            .with_context(|| format!("写 {} 的成员 {name}", self.path.display()))
    }

    fn finish(mut self) -> Result<()> {
        let partial = self.partial.clone().expect("收尾之前临时文件恒在");
        self.writer
            .take()
            .expect("收尾之前写入器恒在")
            .finish()
            .with_context(|| format!("收尾 {}", self.path.display()))?
            .into_inner()
            .with_context(|| format!("刷出 {}", self.path.display()))?
            .sync_all()
            .with_context(|| format!("落盘 {}", self.path.display()))?;
        std::fs::rename(&partial, &self.path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), self.path.display()))?;
        // 改名成功，临时文件已经不在了。
        self.partial = None;
        Ok(())
    }
}

impl Drop for ArchiveSink {
    fn drop(&mut self) {
        // 改名到位就没有临时文件了；还在就是中途出了错，别把半个归档留在输出里。
        // 先放掉写入器：Windows 上还开着句柄的文件删不掉。
        drop(self.writer.take());
        if let Some(partial) = self.partial.take() {
            let _ = std::fs::remove_file(partial);
        }
    }
}

/// 归档里的成员名：ZIP 规范只认 `/`，Windows 上的 `\` 要换过来。
fn archive_name(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 临时容器的位置：在最终名字后面接一段固定后缀，与最终位置**同一层**，
/// 改名因此不跨卷，也就不会退化成一次逐字节复制。
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}
