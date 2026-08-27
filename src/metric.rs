//! 判据：**两项之和**，分块聚合取上分位与「第 K 差的那一块」之间更严的那个（ADR 0002）。
//!
//! - **低通项**：参照与候选各做低通之后的局部均值误差。量的是灰调塌陷与 banding。
//! - **颗粒项**：候选比参照多出来的高频起伏里，**超出可见度地板**的那一部分。
//!   量的是抖动颗粒自身有多显眼。
//!
//! 只有低通项时判据在同一页内把序排反：误差扩散的定义就是把量化误差摊进邻域、保住局部均值，
//! 而低通项量的正是那个量，于是结构性地偏袒抖动、看不见颗粒（见 measurements 的《位深盲测》）。
//! 补上颗粒项不是把低通拆掉——低通仍是对的，逐像素度量在「该不该抖」这一维上符号仍然是反的
//! （《抖动》）。**任何逐像素度量都不得单独作为候选之间的选择依据。**
//!
//! 判据是量，阈值是界。这里只出量：界在 `profile`，拿量去和界比在 `decide`。

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

    /// 直接造一个判据值。只给测试用——生产路径上判据只能由 [`score`] 算出来。
    #[cfg(test)]
    pub(crate) fn from_value(value: f32) -> Self {
        Score(value)
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
/// 参照这一侧的低通、掩蔽加权与高频起伏因此都在这里算一次就存下——
/// ADR 0002 认下的代价就是判据贵。
pub struct Reference {
    image: GrayImage,
    /// 低通核边长，由面板 PPI 推出。
    kernel: u32,
    /// 参照低通后的局部均值。
    low_pass: Vec<f32>,
    /// 分块连同各自的掩蔽加权与参照自己的高频起伏，行优先。
    tiles: Vec<WeightedTile>,
}

impl Reference {
    /// 记下一张参照，连同它要拿去哪块面板上看。
    pub fn new(panel: Panel, image: GrayImage) -> Self {
        let kernel = low_pass_kernel(panel.ppi);
        let low_passed = low_pass(image.pixels(), image.size(), kernel);
        // 掩蔽的活动度量在**块这个尺度**上（见 [`masking_weight`]），与低通核那一层不同，
        // 因此另求一份局部均值。它只与参照有关，一张参照只算一次。
        let structure = low_pass(image.pixels(), image.size(), STRUCTURE_KERNEL);
        let stride = image.size().width as usize;
        let tiles = tiles(image.size())
            .into_iter()
            .map(|tile| WeightedTile {
                weight: masking_weight(tile.activity(image.pixels(), &structure, stride)),
                grain: tile.grain(image.pixels(), &low_passed, stride),
                tile,
            })
            .collect();
        Self {
            image,
            kernel,
            low_pass: low_passed,
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
///
/// 一块的读数是**两项相加**，再乘上这一块的掩蔽加权。
///
/// 相加而不是取更大的那个：抖动做的正是「拿低频换高频」，取更大的那个会让这笔交换在判据上
/// 免费。也不是平方和开方——颗粒项减过可见度地板之后已经不是一个 RMS 分量，
/// 两项各自是一种**看得见的损伤**，同一块上两种都摊上就该两笔都算。
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
            let low = weighted
                .tile
                .low_pass_error(&reference.low_pass, &candidate_low_pass, width);
            let grain = visible_grain(
                weighted
                    .tile
                    .grain(candidate.pixels(), &candidate_low_pass, width),
                weighted.grain,
            );
            weighted.weight * (low + grain)
        })
        .collect();
    Score(aggregate(&mut errors))
}

/// 颗粒项：候选比参照多出来的高频起伏，减去可见度地板，负的算零。
///
/// 减参照那一份，是因为线稿与网点自带高频——候选把它照搬过来不是新长出来的颗粒。
/// 减地板，是因为高频起伏低到一定程度就真的看不见：抖动把误差摊到眼睛分不开的尺度上，
/// **那一段是它该得的便宜**，判据不收。收的是超出去的那一截。
fn visible_grain(candidate: f32, reference: f32) -> f32 {
    (candidate - reference - GRAIN_FLOOR).max(0.0)
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
///
/// 两趟可分离，每趟用**滑动窗口**：一格一加一减，耗时与核边长无关。判据要在两个尺度上
/// 各求一次局部均值（低通核 2~4 像素、结构尺度 32 像素，见 [`STRUCTURE_KERNEL`]），
/// 逐格重算的写法在后者上要贵一个数量级。
///
/// 窗口和累加在 `f64` 里走：滑动窗口一路加减，`f32` 的舍入会沿着行漂，
/// 而判据在纯色页上要读出手算得出的那个数（见 `tests/metric.rs`）。
fn low_pass(pixels: &[u8], size: Size, kernel: u32) -> Vec<f32> {
    let (width, height) = (size.width as usize, size.height as usize);
    // 核边长是偶数时窗口无法严格居中，左右差一格。参照与候选走同一个窗口，差值不受影响。
    let before = ((kernel - 1) / 2) as usize;
    let after = (kernel - 1) as usize - before;
    let mut rows = vec![0f32; pixels.len()];
    for y in 0..height {
        let row = y * width;
        // 横向那一趟只求和：除以核面积留到纵向那一趟的写出，一格只除一次。
        sliding_sum(before, after, 1.0, &mut rows[row..row + width], 1, |x| {
            f64::from(pixels[row + x])
        });
    }
    let mut out = vec![0f32; pixels.len()];
    let area = (kernel * kernel) as f64;
    let last = (height - 1) * width;
    for x in 0..width {
        // 切成**恰好**这一列的那几格：走几格由 out 与 stride 一起定，切多了就是走多了。
        sliding_sum(
            before,
            after,
            1.0 / area,
            &mut out[x..=x + last],
            width,
            |y| f64::from(rows[y * width + x]),
        );
    }
    out
}

/// 一维滑动窗口求和：`out[i * stride]` 收下 `value(i-before ..= i+after)` 的和乘 `scale`，
/// 越界的一律按最近的那一个算。
///
/// 走几格由 `out` 与 `stride` 一起定出来——两者对不上就不是「少算几格」而是**写进邻列**，
/// 那种错既不越界也不报错，只让判据整体偏一点点。`debug_assert` 把这个契约摆出来。
///
/// 一格都没有时什么也不做：延拓要有一个「最近的那一个」才成立，空的一维上没有。
fn sliding_sum(
    before: usize,
    after: usize,
    scale: f64,
    out: &mut [f32],
    stride: usize,
    value: impl Fn(usize) -> f64,
) {
    if out.is_empty() {
        return;
    }
    debug_assert_eq!(
        (out.len() - 1) % stride,
        0,
        "out 装不下整数格：走几格由它与 stride 一起定出来"
    );
    let count = (out.len() - 1) / stride + 1;
    let clamped = |index: isize| value(index.clamp(0, count as isize - 1) as usize);
    let mut sum: f64 = (-(before as isize)..=after as isize).map(clamped).sum();
    for index in 0..count {
        out[index * stride] = (sum * scale) as f32;
        sum += clamped(index as isize + 1 + after as isize);
        sum -= clamped(index as isize - before as isize);
    }
}

/// 判据由哪几项构成（ADR 0002 决定第 5 条）。报告要说得出逐页那一行的数是怎么来的。
///
/// 与 [`Aggregation`] 分工：那个说的是**块的读数怎么收成一个数**，这个说的是
/// **一块的读数本身由什么组成**。两者都是判据的形状，但不是同一层。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Composition {
    /// 颗粒项那道可见度地板，8 位灰度级。低于它的高频起伏当作看不见。
    pub grain_floor: f32,
}

/// 本次判据的构成。眼下对所有 profile 都一样。
pub const fn composition() -> Composition {
    Composition {
        grain_floor: GRAIN_FLOOR,
    }
}

impl std::fmt::Display for Composition {
    /// 地板的数值连同它的来源一并说出——它与阈值同一批盲测标定，读的人要判断得了
    /// 这个数对手上那块面板成不成立（与 [`Threshold`](crate::Threshold) 同一个做法）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "低通后的局部均值误差 ＋ 颗粒超出 {:.1} 灰度级的那一部分（地板盲测标定于 boox-poke6，其余面板未复核）",
            self.grain_floor,
        )
    }
}

/// 分块边长。ADR 0002 定死 32×32：banding 是局部现象，全页均值会被留白稀释。
///
/// **绝对尺寸，不随页尺寸缩放**——它对齐的是 banding 的空间尺度，不是页的尺度。
/// 放大它，块内均值会自己把损伤与干净区平均掉，分块要防的稀释降一级重新出现
/// （ADR 0002 的《不要做的「简化」》）。
const TILE: u32 = 32;

/// 尾巴按比例走的那一半：上分位。ADR 0002 定死 p99。
const UPPER_QUANTILE: f64 = 0.99;

/// 尾巴按绝对块数走的那一半，即 ADR 0002 决定第 3 条的 K：尾巴永远不宽于这么多块。
///
/// **未标定占位值**，按量级推得、不是实测：一块值得报警的 banding 在 300 PPI 上
/// 是百来像素见方，铺在 32×32 的块上就是几块到十几块，8 取的是这个量级的下沿。
/// 为什么占位值往严的一侧取、为什么它第一批该被替掉，见 `CONTEXT.md` 的《尚未确立》。
const TAIL_TILES: usize = 8;

/// 判据聚合（ADR 0002 决定第 3 条）：块边长绝对，尾巴按比例走但永不宽于 K 块。
///
/// 三个数摆在一处，因为读它们的两端要的是同一件事：报告要把 K 标成未标定占位值，
/// 用例要按块边长与 K 造夹具。两端都不必抄下当前这几个数字，标定把 K 换掉时
/// 一行都不用改（与 [`Threshold::value`](crate::Threshold::value) 同一个理由）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aggregation {
    /// 分块边长。绝对尺寸，不随页尺寸缩放。
    pub tile: u32,
    /// 尾巴按比例走的那一半：上分位。
    pub quantile: f64,
    /// 尾巴按绝对块数走的那一半：永远不宽于这么多块。**未标定占位值**。
    pub tail_tiles: usize,
}

/// 本次判据用的聚合。三个数眼下对所有 profile 都一样。
pub const fn aggregation() -> Aggregation {
    Aggregation {
        tile: TILE,
        quantile: UPPER_QUANTILE,
        tail_tiles: TAIL_TILES,
    }
}

impl std::fmt::Display for Aggregation {
    /// 形状连同「K 还没标定」一并说出——判据那一栏的每一个数都是这个形状算出来的，
    /// 不说，读的人无从判断该信到什么程度（与 [`Threshold`](crate::Threshold) 同一个做法）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "分块 {}×{} · 尾巴取 p{}，但不宽于 {} 块（K 未标定占位值）",
            self.tile,
            self.tile,
            (self.quantile * 100.0).round(),
            self.tail_tiles,
        )
    }
}

/// 颗粒可见度地板，8 位灰度级。高频起伏低于它就当作看不见。
///
/// 取值落在窗口 [52.8, 60] 里：下界算得出来，上界由真机盲测夹出。
/// 两者的来历见 ADR 0002 的《第 5 条从哪来》，窗口那两个数在 measurements 的《位深盲测》。
/// 下界那一条在本模块的用例里另有一份算术形态——它不是文档，是会红的断言。
///
/// 它跟着面板走，与判据、阈值同一条（ADR 0002）：换面板即换低通核，
/// 「哪一段算高频」跟着变，这个地板也就不是同一件事。
const GRAIN_FLOOR: f32 = 55.0;

/// 掩蔽加权的地板：结构再密也不至于完全不看。
///
/// 活动度量法换到块尺度之后活动度整体变大，这个数跟着一起重定
/// （窗口见 measurements 的《位深盲测》）。
const MASKING_FLOOR: f32 = 0.5;

/// 掩蔽加权的拐点，8 位灰度级。块内活动度到这里，加权正好落在不打折与地板的中点。
const MASKING_KNEE: f32 = 8.0;

/// 掩蔽活动度量在多大的尺度上——**与分块边长同一个数**。
///
/// 掩蔽要防的是「参照本身有结构的地方，同样的偏移看不出来」，而判据的读数按块出，
/// 那个「有没有结构」问的就该是块内的事。核尺度（2~4 像素）只看得见网点残留那一层，
/// 看不见线稿与画面构成——真机盲测正是在这里出的岔：画集 040 的坏块贴着画上的边，
/// 056 的坏块泡在一片没有内容的平滑渐变里，核尺度的活动度把两者读成一样的
/// （见 measurements 的《位深盲测》里 4bit 不抖那一条）。
///
/// 取局部均值而不是块内均值，是为了让**平缓斜坡透明**：box 均值不改变线性斜坡，
/// 斜坡因此不贡献活动度——而斜坡恰恰是 banding 最显眼的地方，它不该给自己买到掩蔽。
const STRUCTURE_KERNEL: u32 = TILE;

/// 一块的对比度掩蔽加权：平坦低对比区不打折，有结构的区域放宽（ADR 0002）。
/// 崩掉的从来是灰调，不是线稿。
///
/// 活动度取块内「原值离**块尺度**局部均值有多远」的均值（见 [`STRUCTURE_KERNEL`]）。
/// 加权只由参照定、与候选无关：否则抖动候选会拿自己的高频噪声给自己放宽，
/// 而判据恰恰是要在「该不该抖」上说话的。
///
/// 加权是**相对**的：平坦区取 1.0 作基准、不打折，有结构的区域才打折。整体乘一个常数会被
/// 阈值标定原样吸收，能改变判定的只有两类区域之间的比。
///
/// 地板与拐点是**占位值**：拐点仍未标定；地板与量法一起重定（见 [`MASKING_FLOOR`]）——
/// 打折太狠时线稿密的块会被压到看不见，而灰调真崩在那种块上时判据就读不出来了
/// （1bit 不抖动在网点页上正是这种块）。
fn masking_weight(activity: f32) -> f32 {
    MASKING_FLOOR + (1.0 - MASKING_FLOOR) * MASKING_KNEE / (MASKING_KNEE + activity)
}

/// 一块，连同它从参照上取到的掩蔽加权与参照自己的高频起伏。
struct WeightedTile {
    tile: Tile,
    weight: f32,
    /// 参照在这一块上的高频起伏。颗粒项减掉的就是它。
    grain: f32,
}

/// 判据的聚合单位。边上不足一块的按实际像素数算。
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Tile {
    /// 块内每一格算一个数，取平均。三个读数——局部均值误差、高频起伏、掩蔽活动度——
    /// 走的是同一趟遍历，只有格子上算什么不同。
    fn mean(&self, stride: usize, at: impl Fn(usize) -> f32) -> f32 {
        let mut sum = 0f32;
        for y in self.y..self.y + self.height {
            let row = y as usize * stride;
            for x in self.x..self.x + self.width {
                sum += at(row + x as usize);
            }
        }
        sum / (self.width * self.height) as f32
    }

    /// 块内的局部均值误差，即**低通项**。
    fn low_pass_error(&self, reference: &[f32], candidate: &[f32], stride: usize) -> f32 {
        self.mean(stride, |index| {
            let difference = reference[index] - candidate[index];
            difference * difference
        })
        .sqrt()
    }

    /// 块内的高频起伏：像素离自己那一层低通均值有多远。
    ///
    /// 参照与候选各求一份，两者之差才是候选**新长出来的**颗粒（见 [`visible_grain`]）。
    /// 它量的是低通丢掉的那一段——低通那一层量什么、这一层就量它丢了什么，两者拼起来
    /// 才是一整个误差。
    fn grain(&self, pixels: &[u8], low_pass: &[f32], stride: usize) -> f32 {
        self.mean(stride, |index| {
            let difference = f32::from(pixels[index]) - low_pass[index];
            difference * difference
        })
        .sqrt()
    }

    /// 块内的掩蔽活动度：像素离**块尺度**局部均值有多远（见 [`STRUCTURE_KERNEL`]）。
    fn activity(&self, pixels: &[u8], structure: &[f32], stride: usize) -> f32 {
        self.mean(stride, |index| {
            (f32::from(pixels[index]) - structure[index]).abs()
        })
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

/// 分块误差收成一个数：**上分位与「第 K 差的那一块」之间更严的那个**（ADR 0002 决定第 3 条）。
///
/// 尾巴按比例走，但永远不宽于 K 块。两端都不退化：小页上分位本就只圈住两三块、比 K 严，
/// 分位说了算；大页上 K 说了算，绝对尺度的损伤因此穿得过去。
///
/// 只按比例走会让「多小的损伤会被丢掉」随页面积漂——p99 在 2120 块上圈住最差的 22 块，
/// 盖不满那么多块的损伤读数就是 0。块数少到取不出分位时退化成最差的那一块，
/// 宁可严格，也不要把仅有的几块平均掉。
fn aggregate(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("局部均值误差不会是 NaN"));
    // 升序排开，秩越靠后取到的块越差、判据越严：两个秩取靠后的那个，就是取更严的那个。
    let by_share = nearest_rank(UPPER_QUANTILE, values.len());
    let by_count = values.len() + 1 - TAIL_TILES.min(values.len());
    values[by_share.max(by_count) - 1]
}

/// 最近秩：`count` 个数排开后，上分位 `quantile` 落在第几名（从 1 数起）。不插值。
///
/// 判据的分块聚合、卷级上包络与离群页判据的立脚点（后两者见 `crate::envelope`）共用它。
/// 三处站的分位各不相同，取的都是「那个秩上的那一个」；取法写成一处，
/// 「同一套取法」才是构造出来的事实，而不是三边注释里的一句声称。
///
/// `count` 少到取不出分位时秩就是 `count`，退化成最差的那一个。
pub(crate) fn nearest_rank(quantile: f64, count: usize) -> usize {
    (quantile * count as f64).ceil().max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 颗粒项减的是**参照**那一份，不是候选那一份：两个同型参数换个位置，
    /// 判据会安静地把「候选新长出来的颗粒」读成「候选抹掉的颗粒」，方向恰好反过来。
    #[test]
    fn the_grain_term_subtracts_the_reference_side() {
        let over = GRAIN_FLOOR + 10.0;

        // 候选比参照多出 GRAIN_FLOOR + 10：超出地板的那 10 级要收下。
        assert!((visible_grain(over + 8.0, 8.0) - 10.0).abs() < 0.001);
        // 反过来，候选比参照少：一分不收，不是负数也不是那 10 级。
        assert_eq!(visible_grain(8.0, over + 8.0), 0.0);
        // 刚好压在地板上：不收。
        assert_eq!(visible_grain(GRAIN_FLOOR + 8.0, 8.0), 0.0);
    }

    /// 滑动窗口与逐格重算给出同一份局部均值。
    ///
    /// 低通改成滑动窗口是为了让结构尺度那次求值不贵一个数量级（见 [`low_pass`]），
    /// 而窗口的加减一旦错开一格，判据整体只是**略微**偏一点——性质测试全绿，
    /// 黄金快照整片挪几个字节，谁都看不出是这里错了。逐格重算是这条路上唯一的照妖镜。
    ///
    /// 两个尺度、奇偶两种核边长、边界与角落都要走到，所以页取得比核大不了多少。
    #[test]
    fn the_sliding_window_agrees_with_recomputing_each_cell() {
        let size = Size::new(9, 7);
        let pixels: Vec<u8> = (0..(size.width * size.height))
            .map(|index| (index * 37 % 251) as u8)
            .collect();

        for kernel in [2, 3, 4, TILE] {
            let naive = naive_low_pass(&pixels, size, kernel);
            let swept = low_pass(&pixels, size, kernel);
            for (index, (want, got)) in naive.iter().zip(&swept).enumerate() {
                assert!(
                    (want - got).abs() < 0.001,
                    "核 {kernel} 的第 {index} 格：逐格 {want}，滑窗 {got}"
                );
            }
        }
    }

    /// 逐格重算的局部均值。测试自己算，不走被测代码。
    fn naive_low_pass(pixels: &[u8], size: Size, kernel: u32) -> Vec<f32> {
        let (width, height) = (size.width as isize, size.height as isize);
        let before = ((kernel - 1) / 2) as isize;
        let after = (kernel - 1) as isize - before;
        let mut out = vec![0f32; pixels.len()];
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0f32;
                for down in -before..=after {
                    for right in -before..=after {
                        let row = (y + down).clamp(0, height - 1);
                        let column = (x + right).clamp(0, width - 1);
                        sum += f32::from(pixels[(row * width + column) as usize]);
                    }
                }
                out[(y * width + x) as usize] = sum / (kernel * kernel) as f32;
            }
        }
        out
    }

    /// 颗粒地板的下界是**算出来的**，不是调出来的：低于它，「1bit 上抖动优于不抖动」
    /// 这条性质会在某个灰调上翻掉——而那正是 ADR 0002 立判据时守的那一条。
    ///
    /// 推导写在 [`GRAIN_FLOOR`] 的文档里。这里把它当成一条算术不变量钉住：
    /// 谁把地板调低到 52.8 以下，这里当场红，不必等到某张真实页上才发现。
    ///
    /// 断言只管颗粒项这一项。整个判据上抖动那一侧还要背自己的低通项——
    /// 平坦的中浅灰上两者因此仍会交叉，代价写在 ADR 0002 的《后果》里。
    #[test]
    fn the_grain_floor_stays_above_what_an_undithered_flat_tone_pays() {
        // 与最近格点差 u 的一块平坦灰调：不抖动的低通项读 u，FS 的颗粒读 sqrt(u(255-u))。
        let excess = |u: f32| (u * (255.0 - u)).sqrt() - u;
        let worst = (0..=1275)
            .map(|tenth| excess(tenth as f32 / 10.0))
            .fold(f32::MIN, f32::max);

        assert!(
            (52.5..53.0).contains(&worst),
            "推导的下界不再是 52.8 了，是 {worst}"
        );
        assert!(
            GRAIN_FLOOR > worst,
            "地板 {GRAIN_FLOOR} 低于下界 {worst}：1bit 上抖动会在某个灰调上输给不抖动"
        );
    }

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

    /// 块边长**不随页尺寸变化**：绝对尺寸，对齐的是 banding 的空间尺度，不是页的尺度。
    ///
    /// 「让块边长随页放大，块数稳住了，分位自然就对了」读起来像同一件事的更简单做法，
    /// 实则把稀释问题从页级降到块级重新引入一遍——1264×1680 上要保持 256 块，
    /// 块得放大到约 91×91，那个尺寸的一块已横跨多个内容区，**块内均值自己就开始
    /// 把损伤与干净区平均掉**（ADR 0002 的《备选方案》与《不要做的「简化」》两处都钉死了它）。
    /// 这条把它钉在代码里：块边长一旦跟着页走，这里当场红。
    #[test]
    fn the_tile_edge_does_not_follow_the_page_size() {
        // 左上角那一块在这几张页上都是满块，边长直接读得出来。
        let edge = |size| {
            let corner = tiles(size).into_iter().next().expect("页上总有块");
            (corner.width, corner.height)
        };
        let cramped = edge(Size::new(256, 256));
        for size in [
            Size::new(512, 512),
            // 基准面板的实际输出尺寸，以及贴住宽边的跨页。
            Size::new(1264, 1680),
            Size::new(1264, 420),
        ] {
            assert_eq!(edge(size), cramped, "{size} 上的块边长跟着页变了");
        }
        assert_eq!(cramped, (TILE, TILE));

        // 块数因此随页面积走，而不是被稳在某个常数上：基准面板的输出尺寸上铺出
        // 40×53 = 2120 块——ADR 0002 的《第 3 条为什么改过》算的就是这个数。
        assert_eq!(tiles(Size::new(1264, 1680)).len(), 2120);
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
