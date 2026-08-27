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
        (true, false) => image
            .to_rgb8()
            .pixels()
            .map(|p| value(p[0], p[1], p[2]))
            .collect(),
        // 带 alpha 的源先按纸白合成：漫画页的透明区就是纸，直接丢 alpha 会把它变成底下的 RGB（常是黑）。
        (_, true) => image
            .to_rgba8()
            .pixels()
            .map(|p| gray_value_over_paper(p[0], p[1], p[2], p[3]))
            .collect(),
    };
    GrayImage::new(size, pixels)
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
