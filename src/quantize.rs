//! 把参照按位深量化成候选的像素形态。
//!
//! 量化结果回到 8 位工作精度：判据比的是参照与候选在同一精度下的差（ADR 0002），
//! 位深只体现为取值落在哪些格点上。
//!
//! 这里只有不抖动的那一种量化。抖动模式是候选的另一维，随 09 号票落地；
//! 从裁剪后的候选里选出一档在 `decide`。

use anyhow::{Result, bail};

use crate::gray::GrayImage;

/// 输出每像素比特数。编码属性，与面板的灰阶数不是同一个量（`CONTEXT.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BitDepth {
    One,
    Two,
    Four,
    Eight,
}

impl BitDepth {
    /// 位深全集 {1,2,4,8}，由小到大。
    pub const ALL: [BitDepth; 4] = [
        BitDepth::One,
        BitDepth::Two,
        BitDepth::Four,
        BitDepth::Eight,
    ];

    /// 面板灰阶数之内的候选位深，由小到大（ADR 0003：面板灰阶数是位深的硬上界）。
    ///
    /// 裁剪必须发生在判据求值**之前**：参照里没有面板，多出来的那些级到不了眼睛，
    /// 交给判据自己挑，8bit 会以零误差稳赢。e-ink 恒 16 级，于是裁成 {1,2,4}。
    ///
    /// 灰阶数填的是真机上数出来的实际可分辨级数，不必是 2 的幂——一档位深要么整个装得进，
    /// 要么不装（`--gray-levels 10` 留下 {1,2}）。1bit 只要两级，任何面板都留得住。
    pub fn candidates(gray_levels: u32) -> Vec<BitDepth> {
        BitDepth::ALL
            .into_iter()
            .filter(|depth| depth.levels() <= gray_levels)
            .collect()
    }

    /// 按每像素比特数解析（`--bit-depth`）。取值集合不进 CLI 的类型，库这一侧对 CLI 无知。
    pub fn from_bits(bits: u32) -> Result<BitDepth> {
        match BitDepth::ALL.into_iter().find(|depth| depth.bits() == bits) {
            Some(depth) => Ok(depth),
            None => bail!("位深 {bits} 不在全集 {{1, 2, 4, 8}} 里"),
        }
    }

    /// 每像素比特数。
    pub fn bits(self) -> u32 {
        match self {
            BitDepth::One => 1,
            BitDepth::Two => 2,
            BitDepth::Four => 4,
            BitDepth::Eight => 8,
        }
    }

    /// 这个位深能表示的灰度级数。
    pub fn levels(self) -> u32 {
        1 << self.bits()
    }
}

impl std::fmt::Display for BitDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}bit", self.bits())
    }
}

/// 按 `depth` 量化，再摊回 8 位工作精度。
///
/// 各位深的格点是套嵌的（255 = 3×85 = 15×17），所以位深升高只会让格点变密，
/// 不会把某个取值推到更远的格点上。
pub fn quantize(image: &GrayImage, depth: BitDepth) -> GrayImage {
    let table = levels_table(depth);
    let pixels = image
        .pixels()
        .iter()
        .map(|&level| table[level as usize])
        .collect();
    GrayImage::new(image.size(), pixels)
}

/// 一个格点上的 8 位取值在 `depth` 上的序号。[`quantize`] 的反函数，编码要拿它写进文件。
///
/// 只对落在格点上的取值有意义：格点之间的取值会被就近归到一个序号上，那是量化的事。
pub(crate) fn grid_index(level: u8, depth: BitDepth) -> u8 {
    let top = depth.levels() - 1;
    ((u32::from(level) * top + 127) / 255) as u8
}

/// 8 位取值 → 量化后落到的 8 位取值。一页有几百万像素，逐像素算浮点不值得。
fn levels_table(depth: BitDepth) -> [u8; 256] {
    let top = (depth.levels() - 1) as f32;
    std::array::from_fn(|level| {
        let quantized = (level as f32 * top / 255.0).round();
        (quantized * 255.0 / top).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    /// 8bit 的格点就是 8 位工作精度本身：参照在这一档上零误差，判据因此必须先裁候选
    /// 再选（ADR 0003），而不是交给判据自己挑。
    #[test]
    fn eight_bits_is_the_identity() {
        let table = levels_table(BitDepth::Eight);
        for level in 0..=255u8 {
            assert_eq!(table[level as usize], level);
        }
    }

    /// 格点套嵌：低位深能表示的取值，高位深一个不少。
    #[test]
    fn the_levels_of_a_lower_depth_all_survive_at_a_higher_one() {
        for depth in BitDepth::ALL {
            let table = levels_table(depth);
            let mut coarse: Vec<u8> = table.to_vec();
            coarse.dedup();
            for level in coarse {
                for finer in BitDepth::ALL.iter().filter(|finer| **finer > depth) {
                    let finer_table = levels_table(*finer);
                    assert_eq!(
                        finer_table[level as usize], level,
                        "{depth} 的格点 {level} 在 {finer} 上被挪动了"
                    );
                }
            }
        }
    }

    /// e-ink 面板恒 16 级，候选位深因此恒是 {1,2,4}：8bit 不进入候选，也就不进入判据。
    #[test]
    fn an_eink_panel_leaves_three_candidate_depths() {
        assert_eq!(
            BitDepth::candidates(16),
            [BitDepth::One, BitDepth::Two, BitDepth::Four]
        );
        // 数出来的级数不是 2 的幂：装不进的那一档整个裁掉。
        assert_eq!(BitDepth::candidates(10), [BitDepth::One, BitDepth::Two]);
        // 最低的面板也留得住 1bit；无硬上界的面板（LCD）拿到全集。
        assert_eq!(BitDepth::candidates(2), [BitDepth::One]);
        assert_eq!(BitDepth::candidates(256), BitDepth::ALL);
    }

    /// 全集之外的比特数当场被挡下：`--bit-depth 3` 是用户敲错了，不是一个待四舍五入的数。
    #[test]
    fn only_the_four_listed_bit_depths_resolve() {
        for depth in BitDepth::ALL {
            assert_eq!(
                BitDepth::from_bits(depth.bits()).expect("全集里的位深"),
                depth
            );
        }
        for bits in [0, 3, 5, 16] {
            assert!(BitDepth::from_bits(bits).is_err(), "{bits} 不该解析出位深");
        }
    }

    /// 序号与格点一一对应，两个方向都由 `BitDepth` 推出。错开一格就会整页偏色，
    /// 而两边分头写就是错开的由来——所以正反函数摆在一起。
    #[test]
    fn the_grid_index_undoes_the_quantization() {
        for depth in BitDepth::ALL {
            let table = levels_table(depth);
            for level in 0..=255u8 {
                let on_grid = table[level as usize];
                let index = grid_index(on_grid, depth);
                assert!(
                    u32::from(index) < depth.levels(),
                    "{depth} 的序号 {index} 越界"
                );
                let back = (u32::from(index) * 255 / (depth.levels() - 1)) as u8;
                assert_eq!(back, on_grid, "{depth} 的格点 {on_grid} 反查成了 {back}");
            }
        }
    }

    /// 量化不改尺寸——候选与参照要逐像素比。
    #[test]
    fn quantizing_keeps_the_size() {
        let size = Size::new(4, 3);
        let image = GrayImage::new(size, vec![128; 12]);
        assert_eq!(quantize(&image, BitDepth::Two).size(), size);
    }
}
