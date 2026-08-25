//! 编码：写出 PNG。
//!
//! 位深现在恒为 8：1/2/4 位与调色板颜色类型随位深判定落地（06 号票）。
//! ADR 0004 的编码器接口这里还没有——P0 只有 PNG 一个实现，AVIF 输出不在范围内。

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};

use crate::gray::GrayImage;

/// 写一张 8 位灰度 PNG，必要时建出上级目录。
pub fn write_gray8_png(path: &Path, image: &GrayImage) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("建输出目录 {}", parent.display()))?;
    }
    let size = image.size();
    let file = BufWriter::new(
        File::create(path).with_context(|| format!("建输出文件 {}", path.display()))?,
    );
    let mut encoder = png::Encoder::new(file, size.width, size.height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .with_context(|| format!("写 PNG 头 {}", path.display()))?;
    writer
        .write_image_data(image.pixels())
        .with_context(|| format!("写 PNG 像素 {}", path.display()))?;
    writer
        .finish()
        .with_context(|| format!("收尾 {}", path.display()))
}
