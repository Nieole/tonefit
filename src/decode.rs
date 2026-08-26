//! 解码：把一页的字节读成内存中的像素缓冲。
//!
//! 收字节而不是路径——页可能来自目录里的文件，也可能来自归档成员（见 `source`）。
//!
//! AVIF 走 dav1d（见 measurements 的《AVIF 解码的可用路径》），由 `image` 的 `avif-native` 特性提供。
//!
//! **界线在「救不救得回像素」上**（04 号票）。12 号票把它画在「解不解得出完整尺寸」上，
//! 而尺寸买不到像素：一张截在第一个数据块头部的页尺寸齐全、一行像素也没有，
//! 按那条界线它是一张正常页。救回因此要报出**救回了多少**（见 [`Salvage`]），
//! 三种结局各自分开：
//!
//! - 整解成功：完好页。
//! - 整解失败、救回到像素：部分救回页，缺的那一段留成纸白。它的几何一点没缺。
//! - 整解失败、一个像素都没救回，或尺寸解不出来，或尺寸解得出来而缓冲分配不下：失败页。

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// 量救回了多少时，另一趟拿来填缓冲的那个值。只要与 [`PAPER`] 不同即可，见 [`measure`]。
const PROBE: u8 = 0;

/// 救回了多少：解码器真写进像素缓冲的那一份占整页的比例，0 到 1（04 号票）。
///
/// 量的是**缓冲的字节**，而每个像素在缓冲里占的字节数处处相同，比例因此就是像素的比例。
/// 不量「第几行」：隔行 PNG 写进缓冲的位置本来就不连续，行数说不出它救回了多少。
///
/// 它只在救回来的页上有意义：整解成功的页一整页都在，没有比例可谈（见 [`Decoded`]）。
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Salvage(f64);

impl Salvage {
    /// 救回的那一份占整页的比例，0 到 1。
    ///
    /// 一个像素都没有的页不会走到调用方手上——那是一张失败页（见 [`Decoder::decode`]），
    /// 因此从报告里读到的这个数恒大于 0。
    pub fn share(self) -> f64 {
        self.0
    }

    /// 直接造一个比例。**只给测试用**——生产路径上救回了多少只能由 [`measure`] 量出来。
    ///
    /// 与 [`Score::from_value`](crate::Score) 同一条规矩，但它够不着 `#[cfg(test)]`：
    /// 渲染那一层在另一个 crate 里（`src/main.rs`），它要造得出一份带部分救回页的报告。
    pub fn from_share(share: f64) -> Self {
        Self(share)
    }
}

impl std::fmt::Display for Salvage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "救回 {:.1}%", self.0 * 100.0)
    }
}

/// 一页解出来的东西：像素，加上它是整解出来的还是救回来的（04 号票）。
///
/// 救回了多少跟着像素一起交出去，而不是让调用方回头再问一次：那个数只有解码这一刻算得出来，
/// 出了这个模块就再没有第二个地方看得见「哪些字节是解码器真写下的」。
pub struct Decoded {
    pub image: DynamicImage,
    /// 整解出来的完好页是 `None`；救回来的页带着它救回了多少。
    pub salvage: Option<Salvage>,
}

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
/// 数的是「这一页被解了几回」，不是「解码器被叫了几回」——救回自己就要解两趟
/// （见 [`measure`]），按后者数，一张坏页会记成三次解码，而它只是一页。
///
/// 计数是原子的，解码本身因此**不需要独占**：第一遍在 rayon 上满核跑（13 号票），
/// 而一个要 `&mut` 的计数器会把整条计算层串回一条线。
#[derive(Debug, Default)]
pub struct Decoder {
    decodes: AtomicUsize,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 至此解了多少页。
    pub fn decodes(&self) -> usize {
        self.decodes.load(Ordering::Relaxed)
    }

    /// 解码一页。格式按内容判定，扩展名只用来挑出候选成员。
    ///
    /// 整解不成再试一次救回：按文件头的尺寸建好缓冲，解到哪个像素算哪个像素（12 号票）。
    /// **救回到一个像素都算数，一个都没救回来不算**（04 号票）：尺寸给不出像素，
    /// 而一张零像素的页与一张纸白页在输出里没有分别，当成正常页写出去就是把问题藏起来。
    ///
    /// 失败在这里只是一个 `Err`——把它变成一个失败页、把卷送进隔离目录，是 `crate::run` 那一层的事。
    pub fn decode(&self, bytes: &[u8]) -> Result<Decoded> {
        self.decodes.fetch_add(1, Ordering::Relaxed);
        let error = match reader(bytes)?.decode() {
            Ok(image) => {
                return Ok(Decoded {
                    image,
                    salvage: None,
                });
            }
            Err(error) => error,
        };
        let Some((image, salvage)) = salvage(bytes) else {
            return Err(anyhow::Error::new(error).context("解码"));
        };
        if salvage.share() == 0.0 {
            return Err(anyhow::Error::new(error).context("按文件头的尺寸救回，一个像素都没解出来"));
        }
        Ok(Decoded {
            image,
            salvage: Some(salvage),
        })
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

/// 整解失败之后的第二趟：按文件头里的尺寸建好缓冲，解到哪个像素算哪个像素，
/// 并量出救回了多少。
///
/// 这是「截断的图片只要能解出完整尺寸就使用」那一条的实现（12 号票），
/// 04 号票给它补上了量：**完整尺寸**只买到几何，买不到像素。救回到像素的页几何一点没缺，
/// 它因此有自己的目标尺寸、照常缩放与判定，缺的那一段留成纸白；把它算作失败页反而更糟——
/// 那会用卷内统一尺寸顶掉一个本来就正确的尺寸。一个像素都没救回来的页则相反，
/// 「解得出尺寸」在它身上什么都没买到，与那张分配不下缓冲的页同一个结局。
///
/// 尺寸解不出来、或尺寸大到缓冲分配不下，就没有第二趟可言：`None` 回到调用方，那是一个失败页。
///
/// 两个解码器一起建：量那一趟要的是**同一份字节的第二次解码**，建不出来就等于量不出来，
/// 而量不出来的救回没有资格叫救回。
fn salvage(bytes: &[u8]) -> Option<(DynamicImage, Salvage)> {
    let decoder = reader(bytes).ok()?.into_decoder().ok()?;
    let probe = reader(bytes).ok()?.into_decoder().ok()?;
    let total = decoder.total_bytes();
    if total > MAX_DECODED_BYTES {
        return None;
    }
    let (width, height) = decoder.dimensions();
    let color = decoder.color_type();
    let mut buffer = vec![PAPER; usize::try_from(total).ok()?];
    // 错误照吞：缓冲里已经落下的那些像素就是救回来的那一段，剩下的仍是纸白。
    let _ = decoder.read_image(&mut buffer);
    let salvage = measure(probe, &buffer);
    assemble(width, height, color, buffer).map(|image| (image, salvage))
}

/// 量出解码器究竟写下了多少：同一份字节再解一趟，这一趟把缓冲填成 [`PROBE`]。
///
/// 两趟里**对得上的那些字节就是解码器真写下的**：写过的地方两趟都是解出来的那个值，
/// 没写过的地方一边是 [`PAPER`]、一边是 [`PROBE`]，恒不相等。这一手不认哪个格式、
/// 也不假定救回来的是一段前缀（隔行 PNG 写进缓冲的位置本来就不连续）。
///
/// 换不成「数一数缓冲里还剩多少个 [`PAPER`]」：漫画页的天地留白本来就是纸白，
/// 那样数出来，一张只救回了页眉留白的页会被判成一个像素都没救回。**它恰恰是本票的判据**，
/// 判错的方向还是把一张救回来的页打成失败页。
///
/// 代价是救回这条路上解两趟、峰值多占一份缓冲。它整个落在**整解已经失败**的那条路上，
/// 正常页一趟都不多解。
fn measure(probe: impl ImageDecoder, salvaged: &[u8]) -> Salvage {
    let mut buffer = vec![PROBE; salvaged.len()];
    let _ = probe.read_image(&mut buffer);
    let recovered = salvaged
        .iter()
        .zip(&buffer)
        .filter(|(written, probed)| written == probed)
        .count();
    // 一页恒有像素，`salvaged` 因此非空；真出了个零长度的缓冲，那也是一个像素都没救回。
    Salvage(match salvaged.len() {
        0 => 0.0,
        total => recovered as f64 / total as f64,
    })
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
