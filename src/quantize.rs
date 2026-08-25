//! 把参照按候选量化成候选的像素形态。
//!
//! 量化结果回到 8 位工作精度：判据比的是参照与候选在同一精度下的差（ADR 0002），
//! 候选只体现为取值落在哪些格点上、以及误差怎么分布。
//!
//! 候选是 (位深, 抖动模式) 组合（`CONTEXT.md`）。从裁剪后的候选里选出一个在 `decide`。

use anyhow::{Result, anyhow, bail};

use crate::geometry::GeometryGate;
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
    ///
    /// **几何门不成立时这条上界的依据失效，这里仍照裁。**像素与灰阶不再对齐，
    /// 多出来的级到不到眼睛就不再确定；ADR 0003 说了「不得套用」，也说了该用哪个集合
    /// 尚未测量。P0 于是留着这一裁并把洞记在 `CONTEXT.md` 的《尚未确立》里——
    /// 抖动那一维不同，它在门不成立时是**整体关闭**（见 [`Dither::candidates`]）。
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

/// 抖动模式：候选的另一维。抖动用高频误差换低频保真（`CONTEXT.md`）。
///
/// 只有开关两种，没有第三种：ADR 0007 不许在几何门不成立时「降级成更温和的抖动模式」，
/// 于是也就没有可降的中间档。抖的那一种取 FS 误差扩散——它量过的三档位深上
/// 低通判据都最优，另两种（Bayer、蓝噪声）没有一档赢过它（见 measurements 的《抖动》）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dither {
    /// 不抖动：按格点就近取整。
    Off,
    /// Floyd-Steinberg 误差扩散。
    FloydSteinberg,
}

impl Dither {
    /// 几何门放行的抖动模式（ADR 0007：抖动仅在目标尺寸未被下游缩放时启用）。
    ///
    /// 门不成立时只剩「不抖动」这一种——**整体关闭**，不降级、也不留页级开关。
    pub fn candidates(gate: GeometryGate) -> &'static [Dither] {
        if gate.holds() {
            &[Dither::Off, Dither::FloydSteinberg]
        } else {
            &[Dither::Off]
        }
    }

    /// 这个抖动模式的规范名，取表里第一个指向它的那个。
    ///
    /// 与 [`Filter::name`](crate::resample::Filter::name) 同一个用途：参数哈希要一个钉死的写法。
    /// `Display` 顶不了它——那一份是中文，而 tEXt 只装得下 Latin-1。
    pub(crate) fn name(self) -> &'static str {
        DITHERS
            .iter()
            .find(|(_, dither)| *dither == self)
            .map(|(name, _)| *name)
            .expect("表覆盖全部抖动模式")
    }

    /// 按名字解析（`--dither`）。大小写不论。
    pub fn resolve(name: &str) -> Result<Self> {
        let key = name.trim().to_ascii_lowercase();
        DITHERS
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, dither)| *dither)
            .ok_or_else(|| unknown_dither_error(name))
    }
}

/// 名字 → 抖动模式。同一个变体可以有多个名字。
const DITHERS: &[(&str, Dither)] = &[
    ("off", Dither::Off),
    ("none", Dither::Off),
    ("fs", Dither::FloydSteinberg),
    ("floyd-steinberg", Dither::FloydSteinberg),
];

/// 未知抖动模式的说法：把认得的名字全端出来。
fn unknown_dither_error(name: &str) -> anyhow::Error {
    let names: Vec<_> = DITHERS.iter().map(|(name, _)| *name).collect();
    anyhow!("未知抖动模式「{name}」。认得的是：{}。", names.join(" "))
}

impl std::fmt::Display for Dither {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Dither::Off => "不抖动",
            Dither::FloydSteinberg => "FS 误差扩散",
        })
    }
}

/// 一个候选：(位深, 抖动模式) 组合（`CONTEXT.md`）。
///
/// 两维绑成一个类型而不是各走各的：判据求值、上包络、迟滞、覆盖项要的处处是这个组合。
/// ADR 0007 说的正是它——「候选是 (位深, 抖动模式)，上包络取的是这个组合，不设页级抖动开关」。
///
/// **排序即体积由小到大**：位深先，同位深下不抖动在前。「界以内最低的一档」靠的就是这个次序。
/// 次序与体积同调有实测撑着：抖动的体积代价 +3%~+37%，够不上升一档位深的那一步
/// （见 measurements 的《抖动》：1bit+FS 13463 < 2bit 24263，2bit+FS 25074 < 4bit 43590）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate {
    pub bit_depth: BitDepth,
    pub dither: Dither,
}

impl Candidate {
    pub const fn new(bit_depth: BitDepth, dither: Dither) -> Self {
        Self { bit_depth, dither }
    }

    /// 同一档位深上不抖动的那个候选。只给测试用——几何门不成立时的候选集全长这样，
    /// 卷级的用例多半只在位深这一维上分胜负，摆到这里省得各个模块各写一遍。
    #[cfg(test)]
    pub(crate) const fn plain(bit_depth: BitDepth) -> Self {
        Candidate::new(bit_depth, Dither::Off)
    }

    /// 本次的候选集，由小到大：位深按面板灰阶数裁（ADR 0003），抖动模式按几何门裁
    /// （ADR 0007）。两道裁剪都在判据求值之前。
    ///
    /// e-ink 面板 + 几何门成立 = 六个候选；门一关就回到三个。
    pub fn all(gray_levels: u32, gate: GeometryGate) -> Vec<Candidate> {
        BitDepth::candidates(gray_levels)
            .into_iter()
            .flat_map(|bit_depth| {
                Dither::candidates(gate)
                    .iter()
                    .map(move |&dither| Candidate::new(bit_depth, dither))
            })
            .collect()
    }
}

impl std::fmt::Display for Candidate {
    /// 判据值一行要排开六个候选，因此取紧凑写法：`4bit` 与 `4bit+FS`。
    /// 卷级那一行另有一处把抖动模式的整名写出来（见 `main` 的几何门那一行）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.dither {
            Dither::Off => write!(f, "{}", self.bit_depth),
            Dither::FloydSteinberg => write!(f, "{}+FS", self.bit_depth),
        }
    }
}

/// 按 `candidate` 量化，再摊回 8 位工作精度。
///
/// 各位深的格点是套嵌的（255 = 3×85 = 15×17），所以位深升高只会让格点变密，
/// 不会把某个取值推到更远的格点上。抖动改的是误差落在哪里，不是可用的格点。
pub fn quantize(image: &GrayImage, candidate: Candidate) -> GrayImage {
    match candidate.dither {
        Dither::Off => nearest(image, candidate.bit_depth),
        Dither::FloydSteinberg => floyd_steinberg(image, candidate.bit_depth),
    }
}

/// 就近取整到 `depth` 的格点。
fn nearest(image: &GrayImage, depth: BitDepth) -> GrayImage {
    let table = levels_table(depth);
    let pixels = image
        .pixels()
        .iter()
        .map(|&level| table[level as usize])
        .collect();
    GrayImage::new(image.size(), pixels)
}

/// FS 误差扩散：一个像素落到格点上剩下的误差，按 7/3/5/1 的权重推给右、左下、下、右下。
///
/// 误差只往右和往下传，两行的累积量因此就够了，不必为整页留一份浮点缓冲。
///
/// 落点由**含累积误差**的取值算出，而误差算的是它与格点的差、不夹到 [0,255]：
/// 夹一次就吞掉一部分误差，纯黑与纯白附近的平缓过渡会重新长出色带——那正是抖动要消的东西。
fn floyd_steinberg(image: &GrayImage, depth: BitDepth) -> GrayImage {
    let table = levels_table(depth);
    let size = image.size();
    let width = size.width as usize;
    let mut current = vec![0f32; width];
    let mut next = vec![0f32; width];
    let mut pixels = Vec::with_capacity(image.pixels().len());
    for row in image.pixels().chunks_exact(width) {
        std::mem::swap(&mut current, &mut next);
        next.fill(0.0);
        for x in 0..width {
            let wanted = f32::from(row[x]) + current[x];
            let level = table[wanted.clamp(0.0, 255.0).round() as usize];
            pixels.push(level);
            let residual = wanted - f32::from(level);
            if x + 1 < width {
                current[x + 1] += residual * 7.0 / 16.0;
                next[x + 1] += residual / 16.0;
            }
            if x > 0 {
                next[x - 1] += residual * 3.0 / 16.0;
            }
            next[x] += residual * 5.0 / 16.0;
        }
    }
    GrayImage::new(size, pixels)
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

    /// 量化不改尺寸——候选与参照要逐像素比。抖动那一路也一样。
    #[test]
    fn quantizing_keeps_the_size() {
        let size = Size::new(4, 3);
        let image = GrayImage::new(size, vec![128; 12]);
        for dither in [Dither::Off, Dither::FloydSteinberg] {
            let candidate = Candidate::new(BitDepth::Two, dither);
            assert_eq!(quantize(&image, candidate).size(), size, "{candidate}");
        }
    }

    /// 抖动写出的取值同样只落在那一档的格点上：抖动换的是误差的分布，不是可用的取值。
    /// 落到格点外，编码那一步就写不出这个位深（见 `encode`）。
    #[test]
    fn dithering_still_lands_on_the_grid_of_its_bit_depth() {
        let size = Size::new(64, 64);
        let ramp = GrayImage::new(
            size,
            (0..64)
                .flat_map(|y| std::iter::repeat_n((y * 255 / 63) as u8, 64))
                .collect(),
        );
        for depth in BitDepth::ALL {
            let table = levels_table(depth);
            let dithered = quantize(&ramp, Candidate::new(depth, Dither::FloydSteinberg));
            for &level in dithered.pixels() {
                assert!(table.contains(&level), "{depth} 抖出了格点外的 {level}");
            }
        }
    }

    /// 抖动把量化误差换成局部平均意义上的保真：一块平缓灰调抖过之后块内均值贴着原值，
    /// 而不抖动会整块塌到一个格点上。判据看的正是这个差别（ADR 0002）。
    #[test]
    fn dithering_holds_the_local_average_where_rounding_loses_it() {
        // 200 在 1bit 的格点 {0,255} 上就近取整落到 255，整块偏亮 55 级。
        let size = Size::new(64, 64);
        let flat = GrayImage::new(size, vec![200; 64 * 64]);
        let candidate = |dither| Candidate::new(BitDepth::One, dither);

        let mean = |image: &GrayImage| {
            image.pixels().iter().map(|&v| f64::from(v)).sum::<f64>() / image.pixels().len() as f64
        };
        let plain = mean(&quantize(&flat, candidate(Dither::Off)));
        let dithered = mean(&quantize(&flat, candidate(Dither::FloydSteinberg)));

        assert_eq!(plain, 255.0, "不抖动本该整块塌到一个格点上");
        assert!(
            (dithered - 200.0).abs() < 2.0,
            "抖动后的块内均值是 {dithered}，没有贴住原值 200"
        );
    }

    /// 8bit 上抖动是恒等：每个取值本来就在格点上，误差恒为零、无可扩散。
    #[test]
    fn dithering_at_eight_bits_changes_nothing() {
        let size = Size::new(16, 16);
        let pixels: Vec<u8> = (0..256).map(|value| value as u8).collect();
        let image = GrayImage::new(size, pixels.clone());

        let dithered = quantize(
            &image,
            Candidate::new(BitDepth::Eight, Dither::FloydSteinberg),
        );

        assert_eq!(dithered.pixels(), pixels.as_slice());
    }

    /// 候选集是两道裁剪的乘积，由小到大排——「界以内最低的一档」靠的就是这个次序。
    #[test]
    fn the_candidate_set_is_the_product_of_both_crops_in_ascending_order() {
        let open = Candidate::all(16, GeometryGate::Holds);
        assert_eq!(
            open,
            [
                Candidate::new(BitDepth::One, Dither::Off),
                Candidate::new(BitDepth::One, Dither::FloydSteinberg),
                Candidate::new(BitDepth::Two, Dither::Off),
                Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
                Candidate::new(BitDepth::Four, Dither::Off),
                Candidate::new(BitDepth::Four, Dither::FloydSteinberg),
            ]
        );
        // 门一关，抖动那一维整个消失——不是降级成更温和的模式（ADR 0007）。
        assert_eq!(
            Candidate::all(16, GeometryGate::Broken { page: 3 }),
            [
                Candidate::new(BitDepth::One, Dither::Off),
                Candidate::new(BitDepth::Two, Dither::Off),
                Candidate::new(BitDepth::Four, Dither::Off),
            ]
        );
        assert!(open.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// 认不出的抖动模式当场被挡下，认得的名字全端出来。
    #[test]
    fn only_the_listed_dither_modes_resolve() {
        assert_eq!(
            Dither::resolve("FS").expect("fs 应当认得"),
            Dither::FloydSteinberg
        );
        assert_eq!(Dither::resolve("none").expect("none 应当认得"), Dither::Off);

        let error = Dither::resolve("bayer")
            .expect_err("bayer 不在表里")
            .to_string();

        for (name, _) in DITHERS {
            assert!(error.contains(name), "清单里少了 {name}：{error}");
        }
        // 与滤波器那一侧同一条：规范名要能自己解析回来，参数哈希拿它当稳定写法。
        for (_, dither) in DITHERS {
            assert_eq!(Dither::resolve(dither.name()).expect("规范名"), *dither);
        }
    }
}
