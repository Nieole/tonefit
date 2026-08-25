//! 编码：把一张图编成 PNG 字节。
//!
//! 编出字节而不是写文件——页要么落到目录里，要么进归档，去处由输出容器决定（见 `sink`）。
//!
//! 位深现在恒为 8：1/2/4 位与调色板颜色类型随位深判定落地（06 号票）。
//! ADR 0004 的编码器接口这里还没有——P0 只有 PNG 一个实现，AVIF 输出不在范围内。

use anyhow::{Context, Result};

use crate::gray::GrayImage;

/// 把一张灰度图编成 8 位灰度 PNG。
pub fn gray8_png(image: &GrayImage) -> Result<Vec<u8>> {
    let size = image.size();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size.width, size.height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().context("写 PNG 头")?;
    writer
        .write_image_data(image.pixels())
        .context("写 PNG 像素")?;
    writer.finish().context("收尾 PNG")?;
    Ok(bytes)
}
