//! 彩页识别：一页有没有彩色内容，第一遍就要问出来（ADR 0005 决定第 1 条）。
//!
//! 答案决定这一页走哪条路径：彩色面板上的彩页走彩色分支，其余一律走灰度路径
//! （ADR 0010：彩页按 profile 分流）。识别排在转灰**之前**——转过之后就没有颜色可看了。

use image::DynamicImage;

use crate::geometry::Size;
use crate::gray::{self, GrayImage};

/// 一页有没有彩色内容。第一遍识别出来（`CONTEXT.md`：彩页）。
///
/// 这是**页的事实**，不是它走了哪条路径：黑白 profile 下彩页照样转灰走灰度路径，
/// 但它仍然是一张彩页，报告要区分得出来（ADR 0005 决定第 4 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageColor {
    /// 灰度页：消色，或色度低到判不出颜色。
    Gray,
    /// 彩页。
    Color,
}

impl PageColor {
    /// 这是一张彩页吗。
    pub fn is_color(self) -> bool {
        self == PageColor::Color
    }
}

impl std::fmt::Display for PageColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PageColor::Gray => "灰度页",
            PageColor::Color => "彩页",
        })
    }
}

/// 一个像素算「带色」的色度地板，sRGB 8 位上的极差。**未标定占位值**。
///
/// 不取「色度非零」，因为有损转码会给消色像素添上几级色度：B 类素材整批是 8bit YUV420 的
/// 有损 AVIF（见 measurements 的《B 类素材普查》），照非零判，那 97% 的近灰度页会整批
/// 被判成彩页。地板取在噪声之上、真彩色之下——`CONTEXT.md` 的《尚未确立》把
/// 「有损转码源的行为」记为未知，这个数就落在那个洞里。
const CHROMA_FLOOR: u8 = 16;

/// 一页要有这么大比例的像素带色，才算彩页。**未标定占位值**。
///
/// 判据除了幅度还要一道覆盖率：一页灰度扫描上零星几个带色像素，来路是压缩伪影或一枚彩色印章，
/// 不是「这一页要保留颜色」。
///
/// 《B 类素材普查》的「97% 近灰度，3% 彩色」是**分布**，不是判据形态——那一节没有给出
/// 它是怎么数出来的，因此这两个数都不能说成是从它标定来的。
const COLORED_FRACTION: f64 = 0.005;

/// 这一页是彩页还是灰度页。
///
/// 判据有两维：单个像素的**色度幅度**要过 [`CHROMA_FLOOR`]，这样的像素要占到
/// [`COLORED_FRACTION`]。两维都是未标定占位值。
pub fn identify(image: &DynamicImage) -> PageColor {
    // 没有颜色通道的源不必看像素：消色是文件层面的事实，一个字节都不用读。
    if !image.color().has_color() {
        return PageColor::Gray;
    }
    let pixels = u64::from(image.width()) * u64::from(image.height());
    let needed = ((pixels as f64) * COLORED_FRACTION).ceil() as u64;
    // 已经解成 RGB8 或 RGBA8 的源直接借用缓冲；16 位与浮点那几种少见变体才转一份出来。
    let enough = match (image.as_rgb8(), image.as_rgba8()) {
        (Some(rgb), _) => enough_colored(rgb.pixels().map(|p| chroma(p[0], p[1], p[2])), needed),
        (_, Some(rgba)) => enough_colored(
            rgba.pixels()
                .map(|p| chroma_over_paper(p[0], p[1], p[2], p[3])),
            needed,
        ),
        _ => enough_colored(
            image.to_rgb8().pixels().map(|p| chroma(p[0], p[1], p[2])),
            needed,
        ),
    };
    if enough {
        PageColor::Color
    } else {
        PageColor::Gray
    }
}

/// 带色像素够不够 `needed` 个。
///
/// 数到够就停：彩页多半在头几行就见分晓，把整页扫完是白扫。灰度页仍然要全扫——
/// 「不够」这件事只有看完才知道。
fn enough_colored(chromas: impl Iterator<Item = u8>, needed: u64) -> bool {
    let mut colored = 0;
    for chroma in chromas {
        if chroma >= CHROMA_FLOOR {
            colored += 1;
            if colored >= needed {
                return true;
            }
        }
    }
    false
}

/// 内存中的 8 位 RGB 像素缓冲，按通道分平面存。
///
/// 分平面而不是交织：预缩是等权块平均、残差段是卷积重采样，两级都**逐通道独立**，
/// 分平面跑与交织跑逐字节相同（见 `crate::resample`）。分平面于是能把灰度那条路径
/// 原样用过来，彩色分支不必另写一份缩放。
#[derive(Debug, Clone)]
pub struct ColorImage {
    size: Size,
    planes: [GrayImage; 3],
}

impl ColorImage {
    pub fn new(size: Size, planes: [GrayImage; 3]) -> Self {
        debug_assert!(planes.iter().all(|plane| plane.size() == size));
        Self { size, planes }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// R、G、B 三个平面。
    pub fn planes(&self) -> &[GrayImage; 3] {
        &self.planes
    }

    /// 交织成每像素三字节的 RGB 扫描行。编码器要的就是它。
    pub fn interleaved(&self) -> Vec<u8> {
        let [red, green, blue] = &self.planes;
        red.pixels()
            .iter()
            .zip(green.pixels())
            .zip(blue.pixels())
            .flat_map(|((&red, &green), &blue)| [red, green, blue])
            .collect()
    }
}

/// 把解码结果转成 8 位 RGB。彩色分支的入口。
///
/// 带 alpha 的源先按纸白合成，与转灰那一侧同一条曲线（见 `crate::gray`）：
/// 漫画页的透明区就是纸，两条路径对它必须给出同一个答案。
pub fn to_color(image: &DynamicImage) -> ColorImage {
    let size = Size::new(image.width(), image.height());
    let count = (size.width as usize) * (size.height as usize);
    let mut planes = [
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
    ];
    let mut push = |pixel: [u8; 3]| {
        for (plane, channel) in planes.iter_mut().zip(pixel) {
            plane.push(channel);
        }
    };
    if image.color().has_alpha() {
        for pixel in image.to_rgba8().pixels() {
            push(over_paper(pixel[0], pixel[1], pixel[2], pixel[3]));
        }
    } else {
        for pixel in image.to_rgb8().pixels() {
            push([pixel[0], pixel[1], pixel[2]]);
        }
    }
    ColorImage::new(size, planes.map(|pixels| GrayImage::new(size, pixels)))
}

/// 一个 sRGB 像素在线性光下合到纸白上，再编回 sRGB。
fn over_paper(red: u8, green: u8, blue: u8, alpha: u8) -> [u8; 3] {
    if alpha == u8::MAX {
        return [red, green, blue];
    }
    let alpha = f32::from(alpha) / 255.0;
    [red, green, blue].map(|channel| gray::linear_to_srgb(gray::over_paper(channel, alpha)))
}

/// 一个 sRGB 像素的色度：三分量的极差。0 就是消色。
fn chroma(r: u8, g: u8, b: u8) -> u8 {
    r.max(g).max(b) - r.min(g).min(b)
}

/// 同上，但先把透明折进去：漫画页的透明区就是纸，合成到纸白之后是消色的。
///
/// 纸白消色，合成后的色度因此随 alpha 单调折减，这里按 alpha 线性折算。
/// 这是个近似——转灰那一侧在线性光下真做一次合成（见 `gray`）——但识别只要过不过地板，
/// 折减的方向对了就够。
fn chroma_over_paper(r: u8, g: u8, b: u8, alpha: u8) -> u8 {
    (u32::from(chroma(r, g, b)) * u32::from(alpha) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{ImageBuffer, Luma, Rgb, Rgba};

    const SIZE: (u32, u32) = (64, 64);

    /// 这一页的像素数。
    const PIXELS: u32 = SIZE.0 * SIZE.1;

    /// 恰好够把这一页判成彩页的带色像素数。用判据本身算出来，不抄那个占位数字。
    fn just_enough_colored() -> u32 {
        (f64::from(PIXELS) * COLORED_FRACTION).ceil() as u32
    }

    /// 一张 RGB 编码的图，像素由 `pixel` 给出。
    fn rgb(pixel: impl Fn(u32, u32) -> [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(SIZE.0, SIZE.1, |x, y| {
            Rgb(pixel(x, y))
        }))
    }

    /// 灰度编码的一页。
    fn luma(level: impl Fn(u32, u32) -> u8) -> DynamicImage {
        DynamicImage::ImageLuma8(ImageBuffer::from_fn(SIZE.0, SIZE.1, |x, y| {
            Luma([level(x, y)])
        }))
    }

    /// 识别看的是**像素里有没有颜色**，不是文件用了几个通道。
    ///
    /// 「97% 近灰度」的 B 类素材整批是 RGB 编码的有损 AVIF（见 measurements 的
    /// 《B 类素材普查》）：只看颜色类型，那 97% 会整批被判成彩页。
    #[test]
    fn a_page_is_color_only_when_its_pixels_carry_color() {
        assert_eq!(identify(&luma(|_, y| (y * 4) as u8)), PageColor::Gray);
        assert_eq!(
            identify(&rgb(|x, _| if x < 32 { [255, 0, 0] } else { [0, 0, 255] })),
            PageColor::Color
        );
        // 三个分量相等的 RGB 图是灰度页，哪怕文件里写着三个通道。
        let level = |y: u32| (y * 4) as u8;
        assert_eq!(
            identify(&rgb(|_, y| [level(y), level(y), level(y)])),
            PageColor::Gray
        );
    }

    /// 有损转码给灰度源添上的色度不该把它变成彩页。
    ///
    /// B 类素材整批是 8bit YUV420 的有损 AVIF（见 measurements 的《B 类素材普查》）：
    /// 色度子采样与量化让消色像素解回来带上几级色度。识别因此有一道**地板**，
    /// 不是「色度非零」。`CONTEXT.md` 的《尚未确立》记着「有损转码源的行为」这个洞，
    /// 这道地板的高度就在洞里——它是未标定占位值。
    #[test]
    fn the_chroma_a_lossy_transcode_adds_does_not_make_a_page_color() {
        let noise = CHROMA_FLOOR - 1;
        let noisy = rgb(|x, y| {
            let level = ((x + y) % 200 + 28) as u8;
            [level, level - noise / 2, level + noise / 2]
        });
        assert_eq!(identify(&noisy), PageColor::Gray);

        // 地板上那一级算在内：闭区间，与阈值那一侧同一个取舍。
        let floor = CHROMA_FLOOR;
        let at_the_floor = rgb(|_, _| [128, 128 - floor / 2, 128 + floor.div_ceil(2)]);
        assert_eq!(identify(&at_the_floor), PageColor::Color);
    }

    /// 零星几个带色像素不足以让整页成为彩页：判据除了幅度还有一道覆盖率。
    ///
    /// 一页灰度扫描上有几十个带色像素，来路是压缩伪影或一枚彩色印章，
    /// 不是「这一页要保留颜色」。覆盖率同样是未标定占位值。
    #[test]
    fn a_handful_of_colored_pixels_leaves_the_page_gray() {
        let colored_pixels = |count: u32| {
            rgb(|x, y| {
                if y * SIZE.0 + x < count {
                    [255, 0, 0]
                } else {
                    [128, 128, 128]
                }
            })
        };

        assert_eq!(
            identify(&colored_pixels(just_enough_colored() - 1)),
            PageColor::Gray
        );
        assert_eq!(
            identify(&colored_pixels(just_enough_colored())),
            PageColor::Color
        );
    }

    /// 全透明区里的颜色不算数：漫画页的透明区就是纸，合成之后是消色的。
    ///
    /// 转灰那一侧按纸白合成（见 `gray`），识别这一侧要给出一致的答案——
    /// 否则一页会被判成彩页，转灰之后却与灰度页毫无二致。
    #[test]
    fn color_hidden_under_full_transparency_does_not_count() {
        let hidden = DynamicImage::ImageRgba8(ImageBuffer::from_fn(SIZE.0, SIZE.1, |_, _| {
            Rgba([255, 0, 0, 0])
        }));
        assert_eq!(identify(&hidden), PageColor::Gray);

        // 不透明的同一片红仍然是彩页。
        let shown = DynamicImage::ImageRgba8(ImageBuffer::from_fn(SIZE.0, SIZE.1, |_, _| {
            Rgba([255, 0, 0, 255])
        }));
        assert_eq!(identify(&shown), PageColor::Color);
    }
}
