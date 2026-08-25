//! 解码：把一页的字节读成内存中的像素缓冲。
//!
//! 收字节而不是路径——页可能来自目录里的文件，也可能来自归档成员（见 `source`）。
//!
//! AVIF 走 dav1d（见 measurements 的《AVIF 解码的可用路径》），由 `image` 的 `avif-native` 特性提供。
//!
//! **界线在「出不出得来一张对的页」上**（12 号票），而尺寸是其中的大头：解得出完整尺寸的页照用，
//! 截断的页因此不是坏页——它的几何一点没缺，缺的只是末尾几行像素，见 [`salvage`]。
//! 尺寸解不出来、或尺寸解得出来而缓冲分配不下，两种都算失败——后者救不回任何像素。

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use image::{ColorType, DynamicImage, ImageBuffer, ImageDecoder, ImageReader, Limits};

/// 能当作页解码的扩展名。
pub const PAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "gif", "jpeg", "jpg", "png", "tif", "tiff", "webp",
];

/// 一页像素缓冲的上界：要得比这多的页一个像素都不解。
///
/// 取的是 `image` 自带的那个默认值，在这里显式写出来，因为**整解与救回要照同一个数拒绝**：
/// 两条路径各有各的上界，就会出现「整解嫌大拒了，救回却去分配 4 GiB」这种事。
///
/// 它也是「超大尺寸页不导致进程中止」这一条的落点（12 号票）：尺寸荒唐的页
/// 变成一个失败页，而不是让进程被 OOM 收走。
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

/// 救回来的那一页里，没解出来的地方填什么：纸白。
///
/// 与转灰对透明区的处理同一条（`crate::gray` 的 `over_paper`）：漫画页上没有内容的地方就是纸。
const PAPER: u8 = u8::MAX;

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

/// 解码器：解一页记一次。
///
/// 解码是本管线上最贵的一步，而 ADR 0005 的两遍管线拿「每页只解码一次」换下了缓存那一整套
/// 代价。这个计数是那条不变量唯一的守卫，因此它记在**解码这个动作本身**上，
/// 而不是记在调用方的循环里——解码只此一条路，第二遍要是回头解一页，这个数瞒不住。
///
/// 救回那一趟不另记一次：它在同一个 [`decode`](Self::decode) 调用里，
/// 数的是「这一页被解了几回」，不是「解码器被叫了几回」。
#[derive(Debug, Default)]
pub struct Decoder {
    decodes: usize,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 至此解了多少页。
    pub fn decodes(&self) -> usize {
        self.decodes
    }

    /// 解码一页。格式按内容判定，扩展名只用来挑出候选成员。
    ///
    /// 整解不成再试一次救回：尺寸还解得出来就按尺寸出一页（12 号票）。两趟都不成才是失败，
    /// 而失败在这里只是一个 `Err`——把它变成一个失败页、把卷送进隔离目录，是 `crate::run` 那一层的事。
    pub fn decode(&mut self, bytes: &[u8]) -> Result<DynamicImage> {
        self.decodes += 1;
        let error = match reader(bytes)?.decode() {
            Ok(image) => return Ok(image),
            Err(error) => error,
        };
        match salvage(bytes) {
            Some(image) => Ok(image),
            None => Err(anyhow::Error::new(error).context("解码")),
        }
    }
}

/// 一个按本模块的上界设过限的读取器。整解与救回共用它。
fn reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("判定格式")?;
    // 只设分配这一个上界：尺寸本身不设界，能分配得出来的页就该解得出来。
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    Ok(reader)
}

/// 整解失败之后的第二趟：按文件头里的尺寸建好缓冲，解到哪一行算哪一行。
///
/// 这是「截断的图片只要能解出完整尺寸就使用」那一条的实现（12 号票）。**完整尺寸**是关键：
/// 页的几何一点没缺，它因此有自己的目标尺寸、照常参与几何门与判据，与好页毫无二致；
/// 缺的只是末尾几行像素，那一段留成纸白。把它算作失败页反而更糟——那会用卷内统一尺寸
/// 顶掉一个本来就正确的尺寸，还平白把整卷送进隔离目录。
///
/// 尺寸解不出来、或尺寸大到缓冲分配不下，就没有第二趟可言：`None` 回到调用方，那是一个失败页。
fn salvage(bytes: &[u8]) -> Option<DynamicImage> {
    let decoder = reader(bytes).ok()?.into_decoder().ok()?;
    let total = decoder.total_bytes();
    if total > MAX_DECODED_BYTES {
        return None;
    }
    let (width, height) = decoder.dimensions();
    let color = decoder.color_type();
    let mut buffer = vec![PAPER; usize::try_from(total).ok()?];
    // 错误照吞：缓冲里已经落下的行就是救回来的那一段，剩下的仍是纸白。
    let _ = decoder.read_image(&mut buffer);
    assemble(width, height, color, buffer)
}

/// 把 [`ImageDecoder::read_image`] 写出的原始缓冲装回一张图。
///
/// `image` 只在 [`DynamicImage::from_decoder`] 里做这件事，而那一个遇错就整张丢掉——
/// 救回来的那一段正是要留下的东西，因此这里自己装一遍。
fn assemble(width: u32, height: u32, color: ColorType, buffer: Vec<u8>) -> Option<DynamicImage> {
    Some(match color {
        ColorType::L8 => DynamicImage::ImageLuma8(ImageBuffer::from_raw(width, height, buffer)?),
        ColorType::La8 => DynamicImage::ImageLumaA8(ImageBuffer::from_raw(width, height, buffer)?),
        ColorType::Rgb8 => DynamicImage::ImageRgb8(ImageBuffer::from_raw(width, height, buffer)?),
        ColorType::Rgba8 => DynamicImage::ImageRgba8(ImageBuffer::from_raw(width, height, buffer)?),
        ColorType::L16 => {
            DynamicImage::ImageLuma16(ImageBuffer::from_raw(width, height, wide(&buffer))?)
        }
        ColorType::La16 => {
            DynamicImage::ImageLumaA16(ImageBuffer::from_raw(width, height, wide(&buffer))?)
        }
        ColorType::Rgb16 => {
            DynamicImage::ImageRgb16(ImageBuffer::from_raw(width, height, wide(&buffer))?)
        }
        ColorType::Rgba16 => {
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(width, height, wide(&buffer))?)
        }
        ColorType::Rgb32F => {
            DynamicImage::ImageRgb32F(ImageBuffer::from_raw(width, height, floating(&buffer))?)
        }
        ColorType::Rgba32F => {
            DynamicImage::ImageRgba32F(ImageBuffer::from_raw(width, height, floating(&buffer))?)
        }
        // `ColorType` 是 non_exhaustive：认不出的颜色类型救不回来，那一页算失败。
        _ => return None,
    })
}

/// 缓冲里的 16 位分量。`read_image` 按原生字节序写，读回来也照原生字节序。
fn wide(buffer: &[u8]) -> Vec<u16> {
    buffer
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_ne_bytes(pair))
        .collect()
}

/// 同上，32 位浮点分量。
///
/// 没被解到的那一段是 [`PAPER`] 填出来的，按浮点读就是 NaN——浮点这一路的纸白是 1.0，
/// 在这里换回去。非有限值本来也不该出现在一页里。
fn floating(buffer: &[u8]) -> Vec<f32> {
    buffer
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&quad| f32::from_ne_bytes(quad))
        .map(|value| if value.is_finite() { value } else { 1.0 })
        .collect()
}
