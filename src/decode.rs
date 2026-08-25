//! 解码：把一个页文件读成内存中的像素缓冲。
//!
//! AVIF 走 dav1d（见 measurements 的《AVIF 解码的可用路径》），由 `image` 的 `avif-native` 特性提供。

use std::path::Path;

use anyhow::{Context, Result};
use image::DynamicImage;

/// 能当作页解码的扩展名。
pub const PAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "gif", "jpeg", "jpg", "png", "tif", "tiff", "webp",
];

/// 扩展名是否表明这是一页。非图片文件的透传是另一张票的事，这里只负责认页。
pub fn is_page(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            PAGE_EXTENSIONS.contains(&extension.as_str())
        })
        .unwrap_or(false)
}

/// 解码一页。格式按内容判定，扩展名只用来挑出候选文件。
pub fn decode(path: &Path) -> Result<DynamicImage> {
    image::ImageReader::open(path)
        .with_context(|| format!("打开 {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("判定 {} 的格式", path.display()))?
        .decode()
        .with_context(|| format!("解码 {}", path.display()))
}
