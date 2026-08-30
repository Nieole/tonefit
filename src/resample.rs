//! 缩放到目标尺寸：整数倍 box 预缩 + 残差段重采样（ADR 0001）。
//!
//! 两级职责不同，不能合并：预缩那一级是**等权完整窗口平均**，它把网点解析成灰调，
//! 顺带把灰调级数压到 n²+1（限于完整窗口，见 [`box_prescale`]）；残差段是主重采样器，
//! 在已是连续调的图上保真。等权平均当主重采样器只剩模糊，因此它恒定只作用于预缩那一级——
//! [`Filter`] 改的是残差段，改不到预缩。

use std::borrow::Cow;

use anyhow::{Context, Result, anyhow};
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

use crate::color::ColorImage;
use crate::geometry::Size;
use crate::gray::GrayImage;

/// 总缩放比到这个值才触发预缩：低于它，整数倍预缩无处可缩（`CONTEXT.md`）。
const PRESCALE_THRESHOLD: f64 = 2.0;

/// 残差段的重采样滤波器。
///
/// 全套常见滤波器暴露出来供标定与排查（ADR 0001），默认是 Lanczos3。
/// 预缩那一级不在这里选：它恒为等权 box。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// 等权窗口平均，也就是 area。在连续调源上是纯模糊，留作对照。
    Area,
    Bilinear,
    Hamming,
    /// Catmull-Rom，常说的 bicubic。
    Bicubic,
    /// 默认。主重采样器恒为它（ADR 0001）。
    #[default]
    Lanczos3,
}

impl Filter {
    /// 按名字解析。大小写不论，`box` 与 `area` 是同一个滤波器。
    pub fn resolve(name: &str) -> Result<Self> {
        let key = name.trim().to_ascii_lowercase();
        FILTERS
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, filter)| *filter)
            .ok_or_else(|| unknown_filter_error(name))
    }

    /// 这个滤波器的规范名，取表里第一个指向它的那个。
    ///
    /// 参数哈希拿它当稳定写法（见 `crate::metadata`）：那串字节要落进输出文件、
    /// 几个月后还要比对，因此不能搭在 `Debug` 那种没有稳定承诺的写法上。
    pub(crate) fn name(self) -> &'static str {
        FILTERS
            .iter()
            .find(|(_, filter)| *filter == self)
            .map(|(name, _)| *name)
            .expect("表覆盖全部滤波器")
    }

    /// 交给重采样器的那一个。
    fn filter_type(self) -> FilterType {
        match self {
            Filter::Area => FilterType::Box,
            Filter::Bilinear => FilterType::Bilinear,
            Filter::Hamming => FilterType::Hamming,
            Filter::Bicubic => FilterType::CatmullRom,
            Filter::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

/// 名字 → 滤波器。同一个变体可以有多个名字。
const FILTERS: &[(&str, Filter)] = &[
    ("area", Filter::Area),
    ("box", Filter::Area),
    ("bilinear", Filter::Bilinear),
    ("hamming", Filter::Hamming),
    ("bicubic", Filter::Bicubic),
    ("lanczos3", Filter::Lanczos3),
];

/// 未知滤波器的说法：把认得的名字全端出来。
fn unknown_filter_error(name: &str) -> anyhow::Error {
    let names: Vec<_> = FILTERS.iter().map(|(name, _)| *name).collect();
    anyhow!(
        "未知滤波器「{name}」。认得的是：{}。它只作用于残差段——整数倍预缩那一级恒为 box。",
        names.join(" ")
    )
}

/// 一页实际走过的缩放。
///
/// 三个量是一条链：`总缩放比 = 预缩倍数 × 残差比`。预缩退化为恒等时倍数是 1，
/// 残差比就是总缩放比本身。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    ratio: f64,
    prescale: u32,
    residual: f64,
}

impl Scaling {
    /// 这一页该怎么缩：总缩放比 ≥ 2 就取它的整数部分做预缩，否则预缩退化为恒等。
    ///
    /// 取整数部分而不是别的整数：`比 ÷ ⌊比⌋` 落在 [1, 2)，一步就把残差比压进不变量，
    /// 而更小的倍数会让残差段留在 ≥ 2 上。
    pub fn plan(source: Size, target: Size) -> Self {
        let ratio = f64::from(source.height) / f64::from(target.height);
        let prescale = if ratio >= PRESCALE_THRESHOLD {
            ratio as u32
        } else {
            1
        };
        // 残差比按预缩后的**实际**像素高算，不是 `比 ÷ 倍数`：整数倍除不尽时向上取整那半个块
        // 也在图里，报告要说的是这一页真走过的路。
        let residual =
            f64::from(prescaled_size(source, prescale).height) / f64::from(target.height);
        Self {
            ratio,
            prescale,
            residual,
        }
    }

    /// 总缩放比：源页高 ÷ 目标高（`CONTEXT.md`）。
    pub fn total_ratio(self) -> f64 {
        self.ratio
    }

    /// 预缩的整数倍数。1 表示预缩退化为恒等。
    pub fn prescale(self) -> u32 {
        self.prescale
    }

    /// 残差比：预缩之后剩下的非整数段，恒 < 2（`CONTEXT.md`）。
    pub fn residual_ratio(self) -> f64 {
        self.residual
    }

    /// 这一页有没有真的预缩过。
    pub fn prescaled(self) -> bool {
        self.prescale > 1
    }
}

impl std::fmt::Display for Scaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "缩放比 {:.3}", self.ratio)?;
        if self.prescaled() {
            write!(
                f,
                " · 预缩 {}× · 残差比 {:.3}",
                self.prescale, self.residual
            )?;
        } else {
            f.write_str(" · 未预缩")?;
        }
        Ok(())
    }
}

/// 把灰度图缩到 `target`：先整数倍 box 预缩，剩下的残差段交给 `filter`。
///
/// 尺寸相同时原样返回，保证不放大的页逐字节不变。
pub fn resize(source: &GrayImage, target: Size, filter: Filter) -> Result<(GrayImage, Scaling)> {
    let scaling = Scaling::plan(source.size(), target);
    let prescaled = match scaling.prescale {
        1 => Cow::Borrowed(source),
        factor => Cow::Owned(box_prescale(source, factor)),
    };
    Ok((resample(&prescaled, target, filter)?, scaling))
}

/// 把彩色图缩到 `target`：三个通道各走一遍 [`resize`]。
///
/// **分通道跑与交织跑逐字节相同。**两级缩放都逐通道独立——预缩是等权块平均，
/// 残差段是卷积重采样，卷积核只在同一通道内取样。分通道于是不是近似，
/// 而是把灰度那条路径原样用过来：彩色分支不必另写一份缩放，两条路径也不会各自漂移。
///
/// 彩色分支只做缩放（ADR 0005 决定第 4 条），因此这里之后就直接编码写出，没有判据也没有量化。
pub fn resize_color(
    source: &ColorImage,
    target: Size,
    filter: Filter,
) -> Result<(ColorImage, Scaling)> {
    let scaling = Scaling::plan(source.size(), target);
    let [red, green, blue] = source.planes();
    let planes = [
        resize(red, target, filter)?.0,
        resize(green, target, filter)?.0,
        resize(blue, target, filter)?.0,
    ];
    Ok((ColorImage::new(target, planes), scaling))
}

/// 整数倍 box 预缩：每个输出像素是源上一个 `factor`×`factor` 块的等权平均。
///
/// 等权、完整窗口是这一级的全部意义：对二值输入，**完整**窗口只落在块内白点计数上，
/// 产出 n²+1 个取值（见 measurements 的《滤波器与灰调级数》）。
///
/// 边长除不尽时，末行末列剩下的不是完整窗口：它按自己那几个像素平均，分母因此不是 n²，
/// 会另添几个不在那一集里的取值（3×2 的半块给 43，而完整的 3×3 只给 28 与 57）。
/// 机理限于完整窗口，多出来的那些只占一行一列。把余数裁掉能让窗口全都完整，但那是动几何，
/// 是另一回事。
fn box_prescale(image: &GrayImage, factor: u32) -> GrayImage {
    let size = image.size();
    let target = prescaled_size(size, factor);
    let (width, height) = (size.width as usize, size.height as usize);
    let factor = factor as usize;
    let source = image.pixels();
    let mut pixels = Vec::with_capacity((target.width as usize) * (target.height as usize));
    for block_y in 0..target.height as usize {
        let top = block_y * factor;
        let bottom = (top + factor).min(height);
        for block_x in 0..target.width as usize {
            let left = block_x * factor;
            let right = (left + factor).min(width);
            let mut sum = 0u64;
            for row in top..bottom {
                let start = row * width;
                sum += source[start + left..start + right]
                    .iter()
                    .map(|&level| u64::from(level))
                    .sum::<u64>();
            }
            let count = ((bottom - top) * (right - left)) as u64;
            pixels.push(((sum + count / 2) / count) as u8);
        }
    }
    GrayImage::new(target, pixels)
}

/// 预缩 `factor` 倍之后的尺寸。除不尽时向上取整：末尾那个不满的块也是内容。
fn prescaled_size(size: Size, factor: u32) -> Size {
    Size::new(size.width.div_ceil(factor), size.height.div_ceil(factor))
}

/// 残差段：一次卷积重采样。
fn resample(source: &GrayImage, target: Size, filter: Filter) -> Result<GrayImage> {
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
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter.filter_type())),
        )
        .context("重采样")?;
    Ok(GrayImage::new(target, destination.into_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::FitMode;

    /// 基准面板。缩放的算术与面板无关，取一块具体的只为把手算的期望值写死。
    const PANEL: Size = Size::new(1264, 1680);

    /// 预缩倍数与残差比的算术。期望值手算，见每一行的注释。
    #[test]
    fn the_prescale_is_the_integer_part_of_the_total_ratio() {
        let cases = [
            // 源比目标小：比 1.000，两级都没活干。
            (Size::new(800, 1000), Size::new(800, 1000), 1, 1.0),
            // B 类中位页：比 1.219 < 2，预缩退化为恒等，残差比就是总比。
            (
                Size::new(1441, 2048),
                Size::new(1182, 1680),
                1,
                2048.0 / 1680.0,
            ),
            // 正好两倍：预缩一步到位。
            (Size::new(2528, 3360), PANEL, 2, 1.0),
            // 2.5 倍：预缩取整数部分 2，残差段还剩 1.25。
            (Size::new(3160, 4200), PANEL, 2, 1.25),
            // 跨页 4 倍。
            (Size::new(5056, 1680), Size::new(1264, 420), 4, 1.0),
            // 除不尽：3361 行预缩 2 倍得 1681 行（末尾半块也是内容），残差比按实际像素算。
            (Size::new(2529, 3361), PANEL, 2, 1681.0 / 1680.0),
        ];

        for (source, target, prescale, residual) in cases {
            let scaling = Scaling::plan(source, target);
            let ratio = f64::from(source.height) / f64::from(target.height);
            assert_eq!(
                scaling.prescale(),
                prescale,
                "{source} → {target} 的预缩倍数"
            );
            assert_eq!(scaling.prescaled(), prescale > 1, "{source} → {target}");
            assert!(
                (scaling.total_ratio() - ratio).abs() < 1e-12,
                "{source} → {target} 的总缩放比是 {}",
                scaling.total_ratio()
            );
            assert!(
                (scaling.residual_ratio() - residual).abs() < 1e-12,
                "{source} → {target} 的残差比是 {}",
                scaling.residual_ratio()
            );
        }
    }

    /// 不变量（`CONTEXT.md`）：残差比恒 < 2，且预缩绝不缩过头——预缩之后仍不小于目标。
    ///
    /// 扫的是真实几何算出来的目标尺寸，不是编出来的比值：取整发生在适配方式那一层，
    /// 这条不变量必须在取整之后仍然成立。**两种适配方式各扫一遍**（页几何批 01 号票）——
    /// 目标尺寸的来源换了，总缩放比跟着换，而这两条不变量是缩放这一层的，不该跟着换。
    ///
    /// 以高为准带来了一支从前不存在的情形：**总缩放比小于 1**，也就是放大。
    /// 那时预缩退化为恒等（它只在比 ≥ 2 时存在），残差段反过来升采样，
    /// 「预缩之后不小于目标」因此只在真预缩过的页上问得出口——下面那道守卫说的就是这件事。
    #[test]
    fn the_residual_ratio_stays_under_two_at_every_size() {
        for fit in [FitMode::Height, FitMode::Inside] {
            for height in 1..=6000u32 {
                for width in [height * 3 / 4, height * 2, height / 3] {
                    let source = Size::new(width.max(1), height);
                    let target = fit.target(source, PANEL).size();
                    let scaling = Scaling::plan(source, target);

                    assert!(
                        scaling.residual_ratio() < 2.0,
                        "{fit:?}：{source} → {target} 的残差比 {} 越过了 2",
                        scaling.residual_ratio()
                    );
                    assert_eq!(
                        scaling.prescaled(),
                        scaling.total_ratio() >= 2.0,
                        "{fit:?}：{source} → {target} 的总缩放比是 {}",
                        scaling.total_ratio()
                    );
                    // 没预缩过的页没有「缩过头」可谈：以高为准下它可能本来就比目标小，
                    // 那一段是残差段升采样，不是预缩失手。
                    if !scaling.prescaled() {
                        continue;
                    }
                    let prescaled = prescaled_size(source, scaling.prescale());
                    assert!(
                        prescaled.width >= target.width && prescaled.height >= target.height,
                        "{fit:?}：{source} 预缩 {} 倍得 {prescaled}，已经小于目标 {target}",
                        scaling.prescale()
                    );
                }
            }
        }
    }

    /// 预缩就是块内等权平均，末尾不满一块时按实际像素数平均。
    #[test]
    fn prescaling_averages_each_block_with_equal_weights() {
        // 3×3 预缩 2 倍 → 2×2，右列与末行都只有半块。
        let image = GrayImage::new(Size::new(3, 3), vec![0, 255, 0, 255, 0, 255, 0, 255, 0]);

        let prescaled = box_prescale(&image, 2);

        assert_eq!(prescaled.size(), Size::new(2, 2));
        // 左上是整块 (0+255+255+0)/4 = 127.5 → 128；其余三处都是 (0+255)/2 与单个 0。
        assert_eq!(prescaled.pixels(), [128, 128, 128, 0]);
    }

    /// ADR 0001 的机理：等权完整窗口平均对二值输入只落在块内白点计数上，产出 n²+1 个取值
    /// （见 measurements 的《滤波器与灰调级数》）。这条性质是预缩存在的理由。
    #[test]
    fn prescaling_a_binary_page_yields_at_most_n_squared_plus_one_tones() {
        // 96 整除 2、3、4：每一块都是完整窗口，机理成立的正是这种情形。
        let image = binary_page(Size::new(96, 96));

        for factor in [2, 3, 4] {
            let levels = distinct_levels(box_prescale(&image, factor).pixels());
            assert!(
                levels <= (factor * factor + 1) as usize,
                "{factor} 倍预缩出了 {levels} 级灰调，多于 n²+1",
            );
            assert!(
                levels > 2,
                "{factor} 倍预缩只剩 {levels} 级，网点没解析成灰调"
            );
        }
    }

    /// 机理的边界：边长除不尽时，末尾那块按自己的像素数平均，取值因此落到 n²+1 那一集之外。
    ///
    /// 上一条用的是整除的尺寸，量不到这里。这条钉住的是「n²+1 只对完整窗口成立」——
    /// 文档里那句限定必须有据。
    #[test]
    fn the_last_partial_block_averages_over_its_own_pixel_count() {
        // 8 除以 3 余 2：末列那块是 3×2 = 6 像素，分母 6 而不是 9。
        let mut pixels = vec![0u8; 8 * 3];
        pixels[6] = 255; // 末列那块里唯一的白点
        let image = GrayImage::new(Size::new(8, 3), pixels);

        let prescaled = box_prescale(&image, 3);

        // 6 个像素里 1 个白：255 ÷ 6 = 42.5 → 43。
        assert_eq!(prescaled.size(), Size::new(3, 1));
        assert_eq!(prescaled.pixels(), [0, 0, 43]);
        // 而完整的 3×3 窗口只给得出白点计数 k 的那 10 个取值，43 不在其中。
        let full_window: Vec<u8> = (0..=9u32).map(|k| ((k * 255 + 4) / 9) as u8).collect();
        assert!(
            !full_window.contains(&43),
            "43 竟落在完整窗口那一集里：{full_window:?}"
        );
    }

    /// 二值页：每个 8×8 单元里黑点数随位置递增，块内白点计数因此取遍 0..=n²。
    fn binary_page(size: Size) -> GrayImage {
        let pixels: Vec<u8> = (0..size.height)
            .flat_map(|y| {
                (0..size.width).map(move |x| {
                    let cell = (y / 8) * 12 + (x / 8);
                    let index = (y % 8) * 8 + (x % 8);
                    if index < cell % 64 { 0 } else { 255 }
                })
            })
            .collect();
        GrayImage::new(size, pixels)
    }

    #[test]
    fn every_filter_name_resolves_and_box_is_area() {
        for (name, filter) in FILTERS {
            assert_eq!(
                Filter::resolve(name).expect("表里的名字必须解析得出"),
                *filter
            );
            // 大小写与两边的空白不该影响解析。
            assert_eq!(
                Filter::resolve(&format!("  {} ", name.to_ascii_uppercase())).expect("归一"),
                *filter
            );
        }
        assert_eq!(Filter::resolve("box").unwrap(), Filter::Area);
        assert_eq!(Filter::default(), Filter::Lanczos3);
        // 规范名要能自己解析回来：参数哈希拿它当稳定写法（见 `crate::metadata`），
        // 解析不回去，落进输出文件的就是一串没人认得的字。
        for (_, filter) in FILTERS {
            assert_eq!(Filter::resolve(filter.name()).expect("规范名"), *filter);
        }
    }

    /// 认不出的名字要把认得的全端出来——用户是从这段文字里挑的。
    #[test]
    fn the_unknown_filter_error_lists_every_name() {
        let message = Filter::resolve("mitchell").unwrap_err().to_string();
        for (name, _) in FILTERS {
            assert!(message.contains(name), "清单里少了 {name}：{message}");
        }
    }

    /// 取值种类数。
    fn distinct_levels(pixels: &[u8]) -> usize {
        let mut seen = [false; 256];
        for &level in pixels {
            seen[level as usize] = true;
        }
        seen.iter().filter(|&&hit| hit).count()
    }
}
