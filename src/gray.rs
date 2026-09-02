//! 转灰：取 OKLab 的 L 通道，再按 sRGB 传输曲线编回 8bit。
//!
//! 两步缺一不可。L 是感知明度，直接当作输出字节会把灰度源整体提亮——而 B 类页绝大多数本就是灰的
//! （见 measurements 的《B 类素材普查》），整条管线的参照必须与源只差一次缩放。
//! 编回 sRGB 后消色像素恒等通过，彩页则按 OKLab 的加权收敛到灰度，
//! 靠颜色区分的对比（彩色标题字）不会像 Rec.601/709 加权那样塌进背景。

use std::sync::LazyLock;

use image::DynamicImage;

use crate::geometry::Size;

/// 内存中的 8 位灰度像素缓冲。
#[derive(Debug, Clone)]
pub struct GrayImage {
    size: Size,
    pixels: Vec<u8>,
}

impl GrayImage {
    pub fn new(size: Size, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(pixels.len(), (size.width as usize) * (size.height as usize));
        Self { size, pixels }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// 把解码结果转成 8 位灰度。灰度源原样通过，不经色彩换算。
pub fn to_gray(image: &DynamicImage) -> GrayImage {
    let size = Size::new(image.width(), image.height());
    let color = image.color();
    let pixels = match (color.has_color(), color.has_alpha()) {
        (false, false) => image.to_luma8().into_raw(),
        (true, false) => {
            let mut memo = Memo::new([0, 0, 0, u8::MAX], value(0, 0, 0));
            image
                .to_rgb8()
                .pixels()
                .map(|p| memo.gray([p[0], p[1], p[2], u8::MAX], || value(p[0], p[1], p[2])))
                .collect()
        }
        // 带 alpha 的源先按纸白合成：漫画页的透明区就是纸，直接丢 alpha 会把它变成底下的 RGB（常是黑）。
        (_, true) => {
            let mut memo = Memo::new([0, 0, 0, 0], gray_value_over_paper(0, 0, 0, 0));
            image
                .to_rgba8()
                .pixels()
                .map(|p| {
                    memo.gray([p[0], p[1], p[2], p[3]], || {
                        gray_value_over_paper(p[0], p[1], p[2], p[3])
                    })
                })
                .collect()
        }
    };
    GrayImage::new(size, pixels)
}

/// 转灰的**一格缓存**：记住上一个像素的输入与它的取值。
///
/// 漫画页上同色像素成片——平涂的网底、大块实心、整条留白——而一次换算要走三次开立方
/// 加一次幂（见 [`oklab_gray`]）。转灰在**源尺寸**上做，8K 全彩页因此是它最贵的地方：
/// 一页两千四百万像素全部走一遍，产出的只是缩放后的一百五十万像素。
/// 各阶段各占多少，见 measurements 的《分阶段耗时剖面》。
///
/// **它不改变任何一个像素的答案**：命中时给回的就是同一串输入字节上一次算出来的那个数，
/// 没命中就照常算。消色短路（见 [`value`]）挡的是另一批像素——灰的那些；
/// 这一格挡的是**重复的彩色**，两者不重叠。
struct Memo {
    /// 上一个像素的四个分量。不带 alpha 的那条路上第四格恒为 `u8::MAX`。
    last: [u8; 4],
    /// `last` 那个输入的取值。
    gray: u8,
}

impl Memo {
    /// 起手那一格要**由调用方算出来**：两条路对同一串字节给的答案不同——
    /// 全透明在带 alpha 那一条上是纸白，在不带的那一条上根本不会出现。
    /// 拿一个猜出来的默认值起手，第一个像素恰好等于 `last` 时就会答错。
    fn new(last: [u8; 4], gray: u8) -> Self {
        Self { last, gray }
    }

    fn gray(&mut self, pixel: [u8; 4], compute: impl FnOnce() -> u8) -> u8 {
        if pixel != self.last {
            self.last = pixel;
            self.gray = compute();
        }
        self.gray
    }
}

/// 一个 sRGB 像素的灰度取值。
///
/// 裁边那一侧量墨也用它（见 `crate::crop`）：一页的白边在哪，不该因为面板认不认得颜色而变。
pub(crate) fn value(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        // 消色像素的往返恒等（见本模块的测试），直接短路——顺带躲开三次开立方。
        return r;
    }
    let lut = &*SRGB_TO_LINEAR;
    oklab_gray(lut[r as usize], lut[g as usize], lut[b as usize])
}

/// 先在线性光下合到纸白上，再取灰度取值。
fn gray_value_over_paper(r: u8, g: u8, b: u8, alpha: u8) -> u8 {
    if alpha == u8::MAX {
        return value(r, g, b);
    }
    let alpha = f32::from(alpha) / 255.0;
    oklab_gray(
        over_paper(r, alpha),
        over_paper(g, alpha),
        over_paper(b, alpha),
    )
}

/// 一个 sRGB 分量在线性光下合到纸白上的取值。纸白是线性 1.0。
///
/// 彩色分支合成透明区用的是同一条（见 `crate::color`）：两条路径对「透明区就是纸」
/// 这件事必须给出同一个答案。
pub(crate) fn over_paper(channel: u8, alpha: f32) -> f32 {
    SRGB_TO_LINEAR[channel as usize] * alpha + (1.0 - alpha)
}

/// 完整公式：线性 RGB → OKLab 的 L → 立方回线性 → sRGB。
fn oklab_gray(r: f32, g: f32, b: f32) -> u8 {
    let long = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let medium = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let short = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short;
    linear_to_srgb(lightness * lightness * lightness)
}

static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|value| {
        let value = value as f32 / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    })
});

pub(crate) fn linear_to_srgb(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一格缓存给出的与逐像素硬算的**逐字节相同**。
    ///
    /// 缓存答错一个像素，那一页的参照就带着一个错值往下走判据、量化、编码，
    /// 而错的是一格、报告上一个字都不会变。断言因此比的是整页字节。
    ///
    /// 三种图各走一遍：成片重复（缓存全命中）、逐像素都换（全不命中）、
    /// 以及**第一个像素恰好等于起手那一格**的那种图——起手值取错就栽在这一张上。
    #[test]
    fn the_memo_hands_back_what_computing_every_pixel_would() {
        use image::{DynamicImage, ImageBuffer, Rgb, Rgba};

        let flat: Vec<Rgb<u8>> = (0..256).map(|_| Rgb([200, 40, 90])).collect();
        let varied: Vec<Rgb<u8>> = (0..256)
            .map(|index| {
                Rgb([
                    index as u8,
                    (index * 7 % 251) as u8,
                    (index * 13 % 241) as u8,
                ])
            })
            .collect();
        let leading_black: Vec<Rgb<u8>> = (0..256)
            .map(|index| {
                if index < 8 {
                    Rgb([0, 0, 0])
                } else {
                    Rgb([12, 200, 3])
                }
            })
            .collect();

        for source in [flat, varied, leading_black] {
            let buffer = ImageBuffer::from_fn(16, 16, |x, y| source[(y * 16 + x) as usize]);
            let image = DynamicImage::ImageRgb8(buffer);
            let want: Vec<u8> = image
                .to_rgb8()
                .pixels()
                .map(|p| value(p[0], p[1], p[2]))
                .collect();
            assert_eq!(to_gray(&image).pixels(), want, "不带 alpha 的那条路对不上");
        }

        // 带 alpha 的那一条：第一个像素就是全透明，正好压在起手那一格上。
        let buffer = ImageBuffer::from_fn(16, 16, |x, y| {
            let index = y * 16 + x;
            if index < 4 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([(index * 5 % 251) as u8, 30, 200, (index * 3 % 256) as u8])
            }
        });
        let image = DynamicImage::ImageRgba8(buffer);
        let want: Vec<u8> = image
            .to_rgba8()
            .pixels()
            .map(|p| gray_value_over_paper(p[0], p[1], p[2], p[3]))
            .collect();
        assert_eq!(to_gray(&image).pixels(), want, "带 alpha 的那条路对不上");
    }

    /// 消色短路的前提：完整公式对灰度输入恒等。它一旦不成立，灰度源会随改动悄悄漂移。
    #[test]
    fn the_full_formula_leaves_achromatic_pixels_alone() {
        let lut = &*SRGB_TO_LINEAR;
        for level in 0..=255u8 {
            let linear = lut[level as usize];
            assert_eq!(
                oklab_gray(linear, linear, linear),
                level,
                "灰度 {level} 变了"
            );
            assert_eq!(value(level, level, level), level);
        }
    }
}
