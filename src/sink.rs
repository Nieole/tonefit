//! 输出容器：收页与透传文件，写成目录或 CBZ。
//!
//! 与源对称：输入是目录就写目录，输入是 CBZ 就写 CBZ（[`crate::source::Container`] 定这件事）。
//!
//! 幂等要读回上一趟的输出，读的也是这个容器，因此 [`Written`] 落在这里：
//! 「一页在容器里怎么找」两个方向上是同一件事，分开写就是两份会走散的容器知识。

use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;

use crate::metadata::{Fingerprint, RECORD_PREFIX};
use crate::source::Container;

/// 一个卷的输出容器。写完必须调用 [`Sink::finish`]。
pub enum Sink {
    Directory {
        root: PathBuf,
    },
    /// 装箱是因为 `ZipWriter` 自带几 KB 的压缩状态，直接内嵌会让目录卷也背上这份体积。
    Archive(Box<ArchiveSink>),
}

impl Sink {
    /// 在 `path` 建出输出容器：目录卷是一个目录，归档卷是一个归档文件。
    pub fn create(path: &Path, container: Container) -> Result<Self> {
        match container {
            Container::Directory => {
                std::fs::create_dir_all(path)
                    .with_context(|| format!("建输出目录 {}", path.display()))?;
                Ok(Sink::Directory {
                    root: path.to_path_buf(),
                })
            }
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
            Sink::Directory { root } => {
                let path = root.join(relative);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("建输出目录 {}", parent.display()))?;
                }
                std::fs::write(&path, bytes).with_context(|| format!("写 {}", path.display()))
            }
            Sink::Archive(archive) => archive.write(relative, bytes, compression),
        }
    }

    /// 收尾。归档在这一步才落到最终位置。
    pub fn finish(self) -> Result<()> {
        match self {
            Sink::Directory { .. } => Ok(()),
            Sink::Archive(archive) => archive.finish(),
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

    /// 读回一页里记着的指纹。成员不在、或它没有记录，就是 `None`
    /// （ADR 0006：读回 tEXt 比对）。
    ///
    /// 只读到第一个 IDAT 为止，一个像素都不解——成本停在这里，跳过一卷才比重做一卷便宜。
    pub fn fingerprint_of(&mut self, relative: &Path) -> Option<Fingerprint> {
        match self {
            Written::Directory(root) => {
                Fingerprint::read(BufReader::new(File::open(root.join(relative)).ok()?))
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
                Fingerprint::read(Cursor::new(prefix))
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
/// 不能让它顶着最终文件名留在输出里（03 号票：不产出半成品）。
pub struct ArchiveSink {
    /// 最终位置。
    path: PathBuf,
    /// 正在写的临时文件与它的写入器。收尾或析构之后为 `None`——两者同生同死，因此共用一个 `Option`。
    partial: Option<(PathBuf, zip::ZipWriter<BufWriter<File>>)>,
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
            partial: Some((partial, zip::ZipWriter::new(BufWriter::new(file)))),
        })
    }

    fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let name = archive_name(relative);
        let (_, writer) = self.partial.as_mut().expect("收尾之前临时文件恒在");
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
        let (partial, writer) = self.partial.take().expect("收尾只做一次");
        writer
            .finish()
            .with_context(|| format!("收尾 {}", self.path.display()))?
            .into_inner()
            .with_context(|| format!("刷出 {}", self.path.display()))?
            .sync_all()
            .with_context(|| format!("落盘 {}", self.path.display()))?;
        std::fs::rename(&partial, &self.path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), self.path.display()))
    }
}

impl Drop for ArchiveSink {
    fn drop(&mut self) {
        // 收尾过就没有临时文件了；还在就是中途出了错，别把半个归档留在输出里。
        if let Some((partial, writer)) = self.partial.take() {
            drop(writer);
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

/// 临时文件名：在最终文件名后面接一段固定后缀，与最终文件同目录，改名因此不跨卷。
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}
