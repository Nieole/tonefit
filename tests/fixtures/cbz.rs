//! CBZ 夹具：按字节手搓归档。
//!
//! 不用 `zip` 的写入端，因为要测的恰恰是它写不出来的东西——GBK 编码的成员名，
//! 以及「非 ASCII 却没置 UTF-8 标志」这个中文环境下最常见的状态。
//! 成员一律存储不压缩，头部字段因此都是常量，整段代码没有分支。

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use image::DynamicImage;

use super::encode_image;

/// MS-DOS 时间戳的 1980-01-01 00:00:00：年 0、月 1、日 1。夹具不需要真实时间。
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = (1 << 5) | 1;

/// 通用标志位第 11 位：成员名是 UTF-8。
const UTF8_FLAG: u16 = 0x0800;

/// 一个待写出的 CBZ。加完成员调 [`Cbz::write`] 落盘。
pub struct Cbz {
    path: PathBuf,
    members: Vec<Member>,
}

struct Member {
    /// 成员名的原始字节，编码由加它的那个方法决定。
    name: Vec<u8>,
    flags: u16,
    data: Vec<u8>,
    /// 写进头里的 CRC。除 [`Cbz::rotten_page`] 之外都是 `data` 的真值。
    crc: u32,
}

impl Cbz {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            members: Vec::new(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 加一页。成员名按 UTF-8 编并置 UTF-8 标志，格式由 `name` 的扩展名决定。
    pub fn page(&mut self, name: &str, image: &DynamicImage) -> &mut Self {
        self.file(name, &encode_page(name, image))
    }

    /// 加一个非图片成员。
    pub fn file(&mut self, name: &str, bytes: &[u8]) -> &mut Self {
        self.push(name.as_bytes().to_vec(), UTF8_FLAG, bytes.to_vec())
    }

    /// 加一页，成员名按 GBK 编且**不**置 UTF-8 标志——中文环境下的打包工具就是这么写的。
    pub fn gbk_page(&mut self, name: &str, image: &DynamicImage) -> &mut Self {
        self.gbk_file(name, &encode_page(name, image))
    }

    /// 同上，非图片成员。
    pub fn gbk_file(&mut self, name: &str, bytes: &[u8]) -> &mut Self {
        let (encoded, _, unmappable) = encoding_rs::GBK.encode(name);
        assert!(!unmappable, "{name} 编不成 GBK");
        self.push(encoded.into_owned(), 0, bytes.to_vec())
    }

    /// 加一个目录项：名字以 `/` 结尾、内容为空。打包工具通常会写出这些。
    pub fn directory(&mut self, name: &str) -> &mut Self {
        self.file(&format!("{name}/"), &[])
    }

    /// 加一页，但把 CRC 写错——归档结构完好，这一个成员的字节却是坏的。
    pub fn rotten_page(&mut self, name: &str, image: &DynamicImage) -> &mut Self {
        self.page(name, image);
        self.rot()
    }

    /// 同上，非图片成员。透传文件读不出来是**卷级**的失败，不是失败页——
    /// 12 号票隔离的是页，而透传文件逐字节照搬，搬不动就没有别的办法。
    pub fn rotten_file(&mut self, name: &str, bytes: &[u8]) -> &mut Self {
        self.file(name, bytes);
        self.rot()
    }

    /// 把刚加进去的那个成员的 CRC 写错。
    fn rot(&mut self) -> &mut Self {
        let member = self.members.last_mut().expect("刚加进去的那个成员");
        member.crc = !member.crc;
        self
    }

    fn push(&mut self, name: Vec<u8>, flags: u16, data: Vec<u8>) -> &mut Self {
        let crc = crc32fast::hash(&data);
        self.members.push(Member {
            name,
            flags,
            data,
            crc,
        });
        self
    }

    /// 落盘，返回归档路径。
    pub fn write(&self) -> PathBuf {
        std::fs::write(&self.path, self.bytes()).expect("写 CBZ 夹具");
        self.path.clone()
    }

    /// 只落盘前一半字节：中央目录与尾记录都不见了，归档结构读不出来。
    pub fn write_truncated(&self) -> PathBuf {
        let bytes = self.bytes();
        std::fs::write(&self.path, &bytes[..bytes.len() / 2]).expect("写截断的 CBZ 夹具");
        self.path.clone()
    }

    /// 整个归档的字节：一串本地头 + 中央目录 + 尾记录。
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::with_capacity(self.members.len());
        for member in &self.members {
            offsets.push(out.len() as u32);
            out.extend_from_slice(&local_header(member));
            out.extend_from_slice(&member.data);
        }

        let directory_start = out.len() as u32;
        for (member, offset) in self.members.iter().zip(&offsets) {
            out.extend_from_slice(&directory_header(member, *offset));
        }
        let directory_size = out.len() as u32 - directory_start;

        let count = u16::try_from(self.members.len()).expect("夹具不会有那么多成员");
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // 本盘号
        out.extend_from_slice(&0u16.to_le_bytes()); // 中央目录所在盘号
        out.extend_from_slice(&count.to_le_bytes()); // 本盘上的成员数
        out.extend_from_slice(&count.to_le_bytes()); // 成员总数
        out.extend_from_slice(&directory_size.to_le_bytes());
        out.extend_from_slice(&directory_start.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // 归档注释长度
        out
    }
}

fn local_header(member: &Member) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    header.extend_from_slice(&shared_fields(member));
    header.extend_from_slice(&member.name);
    header
}

fn directory_header(member: &Member, offset: u32) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    header.extend_from_slice(&20u16.to_le_bytes()); // 打包方版本
    header.extend_from_slice(&shared_fields(member));
    header.extend_from_slice(&0u16.to_le_bytes()); // 成员注释长度
    header.extend_from_slice(&0u16.to_le_bytes()); // 起始盘号
    header.extend_from_slice(&0u16.to_le_bytes()); // 内部属性
    header.extend_from_slice(&0u32.to_le_bytes()); // 外部属性
    header.extend_from_slice(&offset.to_le_bytes());
    header.extend_from_slice(&member.name);
    header
}

/// 两种头共有的那一段，字段顺序完全相同：解压所需版本一直到扩展字段长度。
fn shared_fields(member: &Member) -> Vec<u8> {
    let mut fields = Vec::new();
    fields.extend_from_slice(&20u16.to_le_bytes()); // 解压所需版本
    fields.extend_from_slice(&member.flags.to_le_bytes());
    fields.extend_from_slice(&0u16.to_le_bytes()); // 压缩方法：存储
    fields.extend_from_slice(&DOS_TIME.to_le_bytes());
    fields.extend_from_slice(&DOS_DATE.to_le_bytes());
    fields.extend_from_slice(&member.crc.to_le_bytes());
    fields.extend_from_slice(&(member.data.len() as u32).to_le_bytes()); // 压缩后大小
    fields.extend_from_slice(&(member.data.len() as u32).to_le_bytes()); // 原始大小
    fields.extend_from_slice(&(member.name.len() as u16).to_le_bytes());
    fields.extend_from_slice(&0u16.to_le_bytes()); // 扩展字段长度
    fields
}

/// 按成员名的扩展名把一页编成字节。
///
/// `.7z` 那份夹具（`super::sevenz`）也用它：「成员名的扩展名决定格式」这条规矩
/// 两种归档共用一份，各写一套的话，同一个成员名在两种包里会编出两种格式。
pub fn encode_page(name: &str, image: &DynamicImage) -> Vec<u8> {
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    encode_image(image, &extension)
}

/// 读回一个归档的全部成员：成员名到字节，按归档里的顺序。
/// 用 `zip` 直接读，绕开被测的写入路径。
pub fn read_cbz(path: &Path) -> Vec<(String, Vec<u8>)> {
    let file = std::fs::File::open(path).expect("打开输出归档");
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).expect("读输出归档的结构");

    (0..archive.len())
        .map(|index| {
            let mut member = archive.by_index(index).expect("取输出归档成员");
            let name = member.name().to_owned();
            let mut bytes = Vec::new();
            member.read_to_end(&mut bytes).expect("读输出归档成员");
            (name, bytes)
        })
        .collect()
}
