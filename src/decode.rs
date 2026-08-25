//! 解码：把一页的字节读成内存中的像素缓冲。
//!
//! 收字节而不是路径——页可能来自目录里的文件，也可能来自归档成员（见 `source`）。
//!
//! AVIF 走 dav1d（见 measurements 的《AVIF 解码的可用路径》），由 `image` 的 `avif-native` 特性提供。

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use image::DynamicImage;

/// 能当作页解码的扩展名。
pub const PAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "gif", "jpeg", "jpg", "png", "tif", "tiff", "webp",
];

/// 扩展名是否表明这是一页。不是页的成员原样透传，见 `source`。
pub fn is_page(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            PAGE_EXTENSIONS.contains(&extension.as_str())
        })
        .unwrap_or(false)
}

/// 解码一页。格式按内容判定，扩展名只用来挑出候选成员。
pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("判定格式")?
        .decode()
        .context("解码")
}
