//! 判据：参照与候选各做低通之后的局部均值误差，分块聚合取上分位（ADR 0002）。
//!
//! 任何逐像素度量都不得作为候选之间的选择依据：抖动用高频误差换低频保真，
//! 逐像素度量在「该不该抖」这一维上符号是反的（见 measurements 的《抖动》）。
//!
//! 判据是量，阈值是界。这里只出量——阈值与选档是 06 号票。

use crate::geometry::Size;
use crate::gray::GrayImage;
use crate::profile::Panel;

/// 一个候选离参照有多远。单位是 8 位灰度级，越小越好。
///
/// 低通核由面板 PPI 推出，**判据数值不可跨面板比较**（ADR 0002）：换面板即换核，
/// 同一个数在两块面板上不是同一件事。
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Score(f32);

impl Score {
    /// 8 位灰度级下的误差值。
    pub fn value(self) -> f32 {
        self.0
    }
}

impl std::fmt::Display for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}", self.0)
    }
}

/// 参照：缩放到目标尺寸后、未经目标位深量化的图，工作精度 8 位灰度（`CONTEXT.md`）。
///
/// 建的时候要给面板，因为低通核由面板 PPI 推出。一张参照要与好几个候选各比一遍，
/// 参照这一侧的低通因此在这里算一次就存下——ADR 0002 认下的代价就是判据贵。
pub struct Reference {
    image: GrayImage,
    /// 低通核边长，由面板 PPI 推出。
    kernel: u32,
    /// 参照低通后的局部均值。
    low_pass: Vec<f32>,
    /// 分块连同各自的掩蔽加权，行优先。
    tiles: Vec<WeightedTile>,
}

impl Reference {
    /// 记下一张参照，连同它要拿去哪块面板上看。
    pub fn new(panel: Panel, image: GrayImage) -> Self {
        let kernel = low_pass_kernel(panel.ppi);
        let low_pass = low_pass(image.pixels(), image.size(), kernel);
        let stride = image.size().width as usize;
        let tiles = tiles(image.size())
            .into_iter()
            .map(|tile| WeightedTile {
                weight: masking_weight(&tile, image.pixels(), &low_pass, stride),
                tile,
            })
            .collect();
        Self {
            image,
            kernel,
            low_pass,
            tiles,
        }
    }

    /// 参照的像素。候选由它量化而来，标定工具也从这里取图。
    pub fn image(&self) -> &GrayImage {
        &self.image
    }

    /// 参照的尺寸，也是候选必须有的尺寸。
    pub fn size(&self) -> Size {
        self.image.size()
    }
}

/// 判据：候选离参照有多远。纯函数，不碰文件系统与全局状态。
///
/// 候选传的是它量化之后摊回 8 位工作精度的像素（见 [`crate::quantize`]）。
/// 尺寸必须与参照一致——判据比的是同一页的两种量化，尺寸对不上是调用方的 bug。
pub fn score(reference: &Reference, candidate: &GrayImage) -> Score {
    assert_eq!(
        candidate.size(),
        reference.size(),
        "候选与参照尺寸不一致：判据比的是同一页的两种量化"
    );
    let candidate_low_pass = low_pass(candidate.pixels(), candidate.size(), reference.kernel);
    let width = reference.size().width as usize;
    let mut errors: Vec<f32> = reference
        .tiles
        .iter()
        .map(|weighted| {
            weighted
                .tile
                .rmse(&reference.low_pass, &candidate_low_pass, width)
                * weighted.weight
        })
        .collect();
    Score(upper_quantile(&mut errors))
}

/// 观看距离，毫米。ADR 0002 的论证前提：300 PPI、30 cm。
const VIEWING_DISTANCE_MM: f64 = 300.0;

/// 低通核张开的视角，弧分。锚点是 measurements 的《抖动》——那一组数在 300 PPI 面板上
/// 用 4×4 取得，30 cm 处 4 px 恰好张开这么多。
const KERNEL_ARC_MINUTES: f64 = 4.0;

const MM_PER_INCH: f64 = 25.4;

/// 低通核边长的取值范围：ADR 0002 要的「2~4 像素量级」。
const KERNEL_RANGE: std::ops::RangeInclusive<u32> = 2..=4;

/// 低通核边长，由面板 PPI 推出——抹掉人眼在观看距离上分不开的那一层，保留看得见的那一层。
///
/// 不是硬编码常数：PPI 变了核就变，同一个视角在密面板上占的像素更多。
fn low_pass_kernel(ppi: u32) -> u32 {
    let span_mm = VIEWING_DISTANCE_MM * (KERNEL_ARC_MINUTES / 60.0).to_radians().tan();
    let pixels = f64::from(ppi) * span_mm / MM_PER_INCH;
    (pixels.round() as u32).clamp(*KERNEL_RANGE.start(), *KERNEL_RANGE.end())
}

/// `kernel`×`kernel` 的局部均值。边界按最近像素延拓，输出与输入同尺寸。
fn low_pass(pixels: &[u8], size: Size, kernel: u32) -> Vec<f32> {
    let (width, height) = (size.width as isize, size.height as isize);
    // 核边长是偶数时窗口无法严格居中，左右差一格。参照与候选走同一个窗口，差值不受影响。
    let before = ((kernel - 1) / 2) as isize;
    let after = (kernel - 1) as isize - before;
    let mut rows = vec![0f32; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0f32;
            for offset in -before..=after {
                let sampled = (x + offset).clamp(0, width - 1);
                sum += f32::from(pixels[(y * width + sampled) as usize]);
            }
            rows[(y * width + x) as usize] = sum;
        }
    }
    let area = (kernel * kernel) as f32;
    let mut out = vec![0f32; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0f32;
            for offset in -before..=after {
                let sampled = (y + offset).clamp(0, height - 1);
                sum += rows[(sampled * width + x) as usize];
            }
            out[(y * width + x) as usize] = sum / area;
        }
    }
    out
}

/// 分块边长。ADR 0002 定死 32×32：banding 是局部现象，全页均值会被留白稀释。
const TILE: u32 = 32;

/// 聚合取的上分位。ADR 0002 定死 p99。
const UPPER_QUANTILE: f64 = 0.99;

/// 掩蔽加权的地板：纹理再密也不至于完全不看。
const MASKING_FLOOR: f32 = 0.25;

/// 掩蔽加权的拐点，8 位灰度级。块内活动度到这里，加权正好落在不打折与地板的中点。
const MASKING_KNEE: f32 = 8.0;

/// 一块的对比度掩蔽加权：平坦低对比区不打折，高频纹理区放宽（ADR 0002）。
/// 崩掉的从来是灰调，不是线稿。
///
/// 活动度取块内「原值离局部均值有多远」的均值——参照自己丢掉的那部分高频。
/// 加权只由参照定、与候选无关：否则抖动候选会拿自己的高频噪声给自己放宽，
/// 而判据恰恰是要在「该不该抖」上说话的。
///
/// 加权是**相对**的：平坦区取 1.0 作基准、不打折，纹理区才打折。整体乘一个常数会被阈值
/// 标定原样吸收，能改变判定的只有两类区域之间的比。
///
/// 地板与拐点是**未标定的占位值**，形状取自 ADR 0002 要的方向，数值等真实样本 A/B 盲测。
fn masking_weight(tile: &Tile, pixels: &[u8], low_pass: &[f32], stride: usize) -> f32 {
    let mut sum = 0f32;
    for y in tile.y..tile.y + tile.height {
        let row = y as usize * stride;
        for x in tile.x..tile.x + tile.width {
            sum += (f32::from(pixels[row + x as usize]) - low_pass[row + x as usize]).abs();
        }
    }
    let activity = sum / (tile.width * tile.height) as f32;
    MASKING_FLOOR + (1.0 - MASKING_FLOOR) * MASKING_KNEE / (MASKING_KNEE + activity)
}

/// 一块，连同它从参照上取到的掩蔽加权。
struct WeightedTile {
    tile: Tile,
    weight: f32,
}

/// 判据的聚合单位。边上不足一块的按实际像素数算。
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Tile {
    /// 块内的局部均值误差。
    fn rmse(&self, reference: &[f32], candidate: &[f32], stride: usize) -> f32 {
        let mut sum = 0f32;
        for y in self.y..self.y + self.height {
            let row = y as usize * stride;
            for x in self.x..self.x + self.width {
                let difference = reference[row + x as usize] - candidate[row + x as usize];
                sum += difference * difference;
            }
        }
        (sum / (self.width * self.height) as f32).sqrt()
    }
}

/// 铺满整页的分块，行优先。
fn tiles(size: Size) -> Vec<Tile> {
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < size.height {
        let height = TILE.min(size.height - y);
        let mut x = 0;
        while x < size.width {
            tiles.push(Tile {
                x,
                y,
                width: TILE.min(size.width - x),
                height,
            });
            x += TILE;
        }
        y += height;
    }
    tiles
}

/// 上分位，最近秩取法。块数少到取不出分位时退化成最差的那一块——
/// 宁可严格，也不要把仅有的几块平均掉。
fn upper_quantile(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("局部均值误差不会是 NaN"));
    let rank = (UPPER_QUANTILE * values.len() as f64).ceil().max(1.0) as usize;
    values[rank - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 核尺寸由 PPI 推出，且落在 ADR 0002 要的量级里。
    /// 期望值：300 PPI 是 measurements《抖动》那一组的 4×4，另两个 PPI 由同一个视角折算。
    #[test]
    fn the_low_pass_kernel_follows_the_panel_ppi() {
        assert_eq!(low_pass_kernel(300), 4);
        assert_eq!(low_pass_kernel(227), 3);
        assert_eq!(low_pass_kernel(207), 3);
        // 面板表之外的极端 PPI 也不许跑出量级。
        assert!(KERNEL_RANGE.contains(&low_pass_kernel(96)));
        assert!(KERNEL_RANGE.contains(&low_pass_kernel(1200)));
    }

    /// 分块铺满整页，不重不漏。
    #[test]
    fn the_tiles_cover_the_page_exactly_once() {
        let size = Size::new(70, 33);
        let mut covered = vec![0u8; (size.width * size.height) as usize];
        for tile in tiles(size) {
            for y in tile.y..tile.y + tile.height {
                for x in tile.x..tile.x + tile.width {
                    covered[(y * size.width + x) as usize] += 1;
                }
            }
        }
        assert!(covered.iter().all(|&times| times == 1), "分块没有铺满一遍");
    }
}
