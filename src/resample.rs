//! 缩放到目标尺寸。
//!
//! 主重采样器恒为 Lanczos3（ADR 0001）。整数倍预缩与 `--filter` 属于 05 号票，这里还没有。

use anyhow::{Context, Result};
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

use crate::geometry::Size;
use crate::gray::GrayImage;

/// 把灰度图重采样到 `target`。尺寸相同时原样返回，保证不放大的页逐字节不变。
pub fn resize(source: &GrayImage, target: Size) -> Result<GrayImage> {
    if source.size() == target {
        return Ok(source.clone());
    }
    let size = source.size();
    let src = Image::from_vec_u8(
        size.width,
        size.height,
        source.pixels().to_vec(),
        PixelType::U8,
    )
    .context("建重采样输入缓冲")?;
    let mut destination = Image::new(target.width, target.height, PixelType::U8);
    Resizer::new()
        .resize(
            &src,
            &mut destination,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        )
        .context("重采样")?;
    Ok(GrayImage::new(target, destination.into_vec()))
}
