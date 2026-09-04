//! `.7z` 夹具：造一个**固实**归档。
//!
//! 与 CBZ 那一份（`super::cbz`）不同，这里不手搓字节：7z 的头是压过的、带 CRC 与块结构，
//! 按字节拼出来的东西与真实打包工具产出的不是一回事。写它因此借
//! `sevenz-rust2` 的编码端——那是 dev-dependency 才有的一半，运行时那一份只解不压
//! （见 `Cargo.toml`）。
//!
//! **一个块装下整卷**（`push_archive_entries`），因为要测的正是固实：所有成员压成一条
//! 连续的流，取第 N 个要从块头解起。这正是 ADR 0015 决定「开工前整卷摊到临时目录」的前提。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};

use super::cbz::encode_page;

/// 一个待写出的 `.7z`。加完成员调 [`SevenZip::write`] 落盘。
pub struct SevenZip {
    path: PathBuf,
    members: Vec<(String, Vec<u8>)>,
}

impl SevenZip {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            members: Vec::new(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 加一页。格式由 `name` 的扩展名决定——那条规矩与 CBZ 那一份**共用一个出处**
    /// （`super::cbz::encode_page`），不然同一个成员名在两种包里会编出两种格式。
    pub fn page(&mut self, name: &str, image: &DynamicImage) -> &mut Self {
        self.file(name, &encode_page(name, image))
    }

    /// 加一个非图片成员。
    pub fn file(&mut self, name: &str, bytes: &[u8]) -> &mut Self {
        self.members.push((name.to_owned(), bytes.to_vec()));
        self
    }

    /// 落盘，返回归档路径。
    pub fn write(&self) -> PathBuf {
        let mut writer = ArchiveWriter::create(&self.path).expect("建 7z 夹具");
        let entries: Vec<ArchiveEntry> = self
            .members
            .iter()
            .map(|(name, _)| ArchiveEntry::new_file(name))
            .collect();
        let readers: Vec<SourceReader<Cursor<Vec<u8>>>> = self
            .members
            .iter()
            .map(|(_, bytes)| SourceReader::new(Cursor::new(bytes.clone())))
            .collect();
        writer
            .push_archive_entries(entries, readers)
            .expect("把成员压进同一个块");
        writer.finish().expect("收尾 7z 夹具");
        self.path.clone()
    }

    /// 落盘，但把压缩流那一段**打坏**：归档头完好、列得出成员，解那一段才发现解不开。
    ///
    /// 打坏的是签名头之后那一截——7z 的头压在文件**末尾**，动尾巴就成了「读不出归档结构」
    /// （那是非卷文件那条路），而这条夹具要的是另一件事：**预扫时打得开、摊开时才失败**。
    pub fn write_with_a_broken_stream(&self) -> PathBuf {
        self.write();
        let mut bytes = std::fs::read(&self.path).expect("读回 7z 夹具");
        // 32 字节的签名头之后就是打包流。整卷只有一个块，动它中间一段即可。
        let start = 32;
        let end = (start + 64).min(bytes.len());
        assert!(end > start, "夹具太小，压缩流那一段够不着");
        for byte in &mut bytes[start..end] {
            *byte = !*byte;
        }
        std::fs::write(&self.path, &bytes).expect("写坏掉的 7z 夹具");
        self.path.clone()
    }
}
