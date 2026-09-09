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
use crate::quantize::BitDepth;

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
/// 候选传的是它量化之后摊回 8 位工作精度的像素（见 [`crate::quantize`]），
/// `depth` 是把它量化出来的那一档位深。
/// 尺寸必须与参照一致——判据比的是同一页的两种量化，尺寸对不上是调用方的 bug。
///
/// **位深要单独传进来**，因为颗粒项那道可见度地板是**格点间距的一个比例**
/// （见 [`Composition`]），而格点间距只有位深说得出来。从候选的像素上反推格点数
/// 不成立：一张纯色页在任何一档上都只用得着一个格点，反推出来的是 1bit。
/// 每一个调用点手里本来就有 `Candidate`，位深因此是现成的。
///
/// 一块的读数是**两项相加**，再乘上这一块的掩蔽加权。
///
/// 相加而不是取更大的那个：抖动做的正是「拿低频换高频」，取更大的那个会让这笔交换在判据上
/// 免费。也不是平方和开方——颗粒项减过可见度地板之后已经不是一个 RMS 分量，
/// 两项各自是一种**看得见的损伤**，同一块上两种都摊上就该两笔都算。
pub fn score(reference: &Reference, candidate: &GrayImage, depth: BitDepth) -> Score {
    assert_eq!(
        candidate.size(),
        reference.size(),
        "候选与参照尺寸不一致：判据比的是同一页的两种量化"
    );
    let floor = composition().floor(depth);
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
                floor,
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
///
/// 地板由调用方按位深算出（[`Composition::floor`]）：它是格点间距的一个比例，
/// 而这里一块一块地算，位深在整页上只有一个。
fn visible_grain(candidate: f32, reference: f32, floor: f32) -> f32 {
    (candidate - reference - floor).max(0.0)
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
    // 一格都没有的页：下面两趟都要有「最近的那一个」才成立，而空页上没有。
    if pixels.is_empty() {
        return Vec::new();
    }
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
    sweep_columns(before, after, 1.0 / area, &rows, &mut out, width, height);
    out
}

/// 纵向那一趟一次走多少列。取 16：16 个 `f32` 正好一条 64 字节缓存行。
const LANES: usize = 16;

/// 纵向的滑动窗口，**一次走 [`LANES`] 列**。
///
/// 与逐列调用 [`sliding_sum`] **逐位相同**：每一列各有一个累加器，加与减的次序、
/// 起始窗口的折叠次序都一格没动，变的只是列与列之间谁先走完。
/// `f64` 的加法不结合，因此「一格没动」是这里唯一站得住的写法——
/// 把加减并成一句 `sum += add - subtract` 就少了一次舍入，判据会静默地漂。
/// [`the_column_sweep_matches_one_column_at_a_time`] 把这条钉成逐位相等的断言。
///
/// 为什么不逐列走：逐列取数要沿着列跨 `width` 个 `f32`，一页 1.5 兆像素上
/// 每取一个数就是一条新缓存行，而一条线只用得上其中一个 `f32`。分带之后一行只碰一条，
/// 取数的次数不变、缺失的次数降到十六分之一（见 measurements 的《分阶段耗时剖面》）。
fn sweep_columns(
    before: usize,
    after: usize,
    scale: f64,
    rows: &[f32],
    out: &mut [f32],
    width: usize,
    height: usize,
) {
    debug_assert_eq!(rows.len(), out.len(), "两侧是同一页");
    debug_assert!(
        height > 0 && width > 0,
        "空页在 low_pass 里就早退了：延拓要有一个「最近的那一行」才成立"
    );
    let last = height as isize - 1;
    // 越界的一律按最近的那一行算，与 [`sliding_sum`] 的延拓同一条。
    let clamped = |y: isize| (y.clamp(0, last) as usize) * width;
    let mut sums = [0f64; LANES];
    for x0 in (0..width).step_by(LANES) {
        let lanes = LANES.min(width - x0);
        let sums = &mut sums[..lanes];
        sums.fill(0.0);
        for y in -(before as isize)..=after as isize {
            let row = clamped(y) + x0;
            for (sum, &value) in sums.iter_mut().zip(&rows[row..row + lanes]) {
                *sum += f64::from(value);
            }
        }
        for y in 0..height {
            let row = y * width + x0;
            let entering = clamped(y as isize + 1 + after as isize) + x0;
            let leaving = clamped(y as isize - before as isize) + x0;
            // 三件事并在一个循环里：一条线的累加器每格只读写一次。
            // 拆成三个循环，累加器那一摊装不进寄存器，每格要多走四趟栈。
            // **一条线之内的次序仍是「先写出、再加进、再减去」**，与逐列走一格不差。
            let out = &mut out[row..row + lanes];
            let entering = &rows[entering..entering + lanes];
            let leaving = &rows[leaving..leaving + lanes];
            for lane in 0..lanes {
                out[lane] = (sums[lane] * scale) as f32;
                sums[lane] += f64::from(entering[lane]);
                sums[lane] -= f64::from(leaving[lane]);
            }
        }
    }
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
    /// 颗粒项那道可见度地板**占格点间距的比例**。地板本身由它乘间距算出，
    /// 见 [`Composition::floor`]。
    pub grain_ratio: f32,
}

impl Composition {
    /// 这一档位深上的可见度地板，8 位灰度级。低于它的高频起伏当作看不见。
    ///
    /// 一个比例乘各档自己的格点间距，**不是三档存一份表**——为什么，
    /// 见 ADR 0002 决定第 5 条。
    pub fn floor(self, depth: BitDepth) -> f32 {
        self.grain_ratio * quantisation_step(depth)
    }
}

/// 本次判据的构成。眼下对所有 profile 都一样。
pub const fn composition() -> Composition {
    Composition {
        grain_ratio: GRAIN_RATIO,
    }
}

impl std::fmt::Display for Composition {
    /// 地板按**比例**说，连同它的来源一并说出——它与阈值同一批盲测标定，读的人要判断得了
    /// 这个数对手上那块面板成不成立（与 [`Threshold`](crate::Threshold) 同一个做法）。
    ///
    /// 说比例而不说某一档的那个绝对值：判据一页要排开好几档位深，
    /// 说死其中一档的数，另几档那几个数就没有出处了。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "低通后的局部均值误差 ＋ 颗粒超出格点间距 {:.1}% 的那一部分（地板盲测标定于 boox-poke6，其余面板未复核）",
            self.grain_ratio * 100.0,
        )
    }
}

/// 这一档位深的**格点间距**，8 位灰度级：`255 / (2^n − 1)`。
///
/// 格点是套嵌的（255 = 3×85 = 15×17，见 [`crate::quantize`]），三档因此都是整数：
/// 1bit 255、2bit 85、4bit 17。
fn quantisation_step(depth: BitDepth) -> f32 {
    255.0 / (depth.levels() - 1) as f32
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
            "分块 {}x{} ⋅ 尾巴取 p{}，但不宽于 {} 块（K 未标定占位值）",
            self.tile,
            self.tile,
            (self.quantile * 100.0).round(),
            self.tail_tiles,
        )
    }
}

/// 颗粒可见度地板占**格点间距**的比例。高频起伏低于地板就当作看不见。
///
/// **地板是间距的一个份额，不是一个绝对值**（ADR 0002 决定第 5 条）。一个绝对的 55
/// 卡在 1bit 与 2bit 之间，颗粒项在 2bit 及以上恒读零、判据在那些档上退回只剩低通项
/// ——三重证据见 measurements 的《颗粒项只在 1bit 上生效》。
///
/// **下界算得出来，不必标定。**`sqrt(u(s−u)) − u` 的最大值在 `u/s = (2−√2)/4` 处取到
/// `0.2071·s`：比例低于 0.2071，某个灰调上抖动就会输给同档不抖动。
/// 本模块的用例里另有一份算术形态，三档各钉一次——它不是文档，是会红的断言。
///
/// 取值 **0.2157 = 0.2071 × 1.0414**：下界乘现行的安全系数。这个数保证 1bit 上算出的地板
/// **逐位**仍是 55.0——那是《位深盲测》整批数据的可比性底线，三档各是多少见
/// measurements 的《颗粒项只在 1bit 上生效》。安全系数那一头是上界，仍由真机盲测夹出，
/// 眼下只有 1bit 上有数据（现行上界 60）。
///
/// 它跟着面板走，与判据、阈值同一条（ADR 0002）：换面板即换低通核，
/// 「哪一段算高频」跟着变，这道地板也就不是同一件事。
const GRAIN_RATIO: f32 = 0.215_686_27;

/// 掩蔽加权的地板：结构再密也不至于完全不看。**未标定占位值**。
///
/// 活动度量法换到块尺度之后活动度整体变大，这个数跟着一起重定
/// （窗口见 measurements 的《位深盲测》）。
const MASKING_FLOOR: f32 = 0.5;

/// 掩蔽加权的拐点，8 位灰度级。块内活动度到这里，加权正好落在不打折与地板的中点。
/// **未标定占位值**。
const MASKING_KNEE: f32 = 8.0;

/// 判据掩蔽（ADR 0002 决定第 4 条）：一块的读数**怎么加权**。
///
/// 与 [`Composition`]、[`Aggregation`] **平级**，三者是判据形状的三件事，不是同一层：
/// 那个说**一块的读数由什么组成**，这个说**同一个读数落在有结构的块上该打几折**，
/// [`Aggregation`] 说**块的读数怎么收成一个数**。
///
/// 两个数摆在一处，与 [`Aggregation`] 同一个理由：读它们的两端要的是同一件事——
/// 报告要把它们标成未标定占位值，本模块算一块的掩蔽加权要按它们算。
/// 两端都不必抄下当前这两个数字，标定把它们换掉时一行都不用改
/// （与 [`Threshold::value`](crate::Threshold::value) 同一个理由）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Masking {
    /// 打折打到头的那一档：结构再密也不至于完全不看。**未标定占位值**。
    pub floor: f32,
    /// 拐点，8 位灰度级：块内活动度到这里，加权正好落在不打折与地板的中点。**未标定占位值**。
    pub knee: f32,
}

/// 本次判据用的掩蔽。两个数眼下对所有 profile 都一样。
pub const fn masking() -> Masking {
    Masking {
        floor: MASKING_FLOOR,
        knee: MASKING_KNEE,
    }
}

impl std::fmt::Display for Masking {
    /// 形状连同「这两个数都还没标定」一并说出——判据那一栏的每一个数都经过这道加权，
    /// 不说，读的人无从判断该信到什么程度（与 [`Aggregation`] 同一个做法）。
    ///
    /// 说的是加权曲线上**说得出名字的那几处**（平坦处不打折、地板压不下去、拐点在两者中点），
    /// 而不是那个式子：读报告的人要判断的是「这道折扣有多狠」，式子答不了这一问。
    ///
    /// **地板说成「压不到它以下」而不是「打到它」**：那是一条渐近线，本模块算加权那一步
    /// 永远到不了它，钉住这一条的用例就在下面。说成到达就是屏上一句假话。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "平坦块不打折 ⋅ 结构越密折得越狠，压不到 {:.2} 以下 ⋅ 活动度 {:.1} 时落在两者中点（地板与拐点均未标定占位值）",
            self.floor, self.knee,
        )
    }
}

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
/// 地板与拐点**两个都是未标定占位值**（`CONTEXT.md` 的《尚未确立》，measurements 的
/// 《位深盲测》）：拐点从来没标定过；地板的现值是量法换到块尺度那次跟着重定的，
/// 那一次夹的是窗口、不是一次标定（见 [`MASKING_FLOOR`]）。打折太狠时线稿密的块会被压到
/// 看不见，而灰调真崩在那种块上时判据就读不出来了（1bit 不抖动在网点页上正是这种块）。
///
/// **地板是下确界，不是到得了的那一档**：`knee / (knee + activity)` 恒大于零，
/// 加权因此永远压不到地板以下、也永远到不了它。报告那一行照这个说法印
/// （见 [`Masking`] 的 `Display`）。
///
/// 那两个数**从 [`masking`] 取**，不从常数直接取：报告印出来的与这里算的因此是同一份，
/// 而 [`Masking`] 不是一个只给报告看的摆设。
fn masking_weight(activity: f32) -> f32 {
    let Masking { floor, knee } = masking();
    floor + (1.0 - floor) * knee / (knee + activity)
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
    ///
    /// 交给闭包的是**一行的下标区间**，不是一格的下标：调用方切成片再走，
    /// 一行只查一次边界，而不是一格查两次（页尺寸上的差别见 measurements 的
    /// 《分阶段耗时剖面》）。累加器**由本函数持有并原样传进去**——三个读数都是
    /// 单精度的连加，换一个折叠次序判据就漂了，而按行各求各的再加起来正是换了次序。
    fn mean(&self, stride: usize, mut row: impl FnMut(&mut f32, std::ops::Range<usize>)) -> f32 {
        let mut sum = 0f32;
        for y in self.y..self.y + self.height {
            let start = y as usize * stride + self.x as usize;
            row(&mut sum, start..start + self.width as usize);
        }
        sum / (self.width * self.height) as f32
    }

    /// 块内的局部均值误差，即**低通项**。
    fn low_pass_error(&self, reference: &[f32], candidate: &[f32], stride: usize) -> f32 {
        self.mean(stride, |sum, row| {
            let reference = &reference[row.clone()];
            let candidate = &candidate[row];
            for (&reference, &candidate) in reference.iter().zip(candidate) {
                let difference = reference - candidate;
                *sum += difference * difference;
            }
        })
        .sqrt()
    }

    /// 块内的高频起伏：像素离自己那一层低通均值有多远。
    ///
    /// 参照与候选各求一份，两者之差才是候选**新长出来的**颗粒（见 [`visible_grain`]）。
    /// 它量的是低通丢掉的那一段——低通那一层量什么、这一层就量它丢了什么，两者拼起来
    /// 才是一整个误差。
    fn grain(&self, pixels: &[u8], low_pass: &[f32], stride: usize) -> f32 {
        self.mean(stride, |sum, row| {
            let pixels = &pixels[row.clone()];
            let low_pass = &low_pass[row];
            for (&pixel, &low_pass) in pixels.iter().zip(low_pass) {
                let difference = f32::from(pixel) - low_pass;
                *sum += difference * difference;
            }
        })
        .sqrt()
    }

    /// 块内的掩蔽活动度：像素离**块尺度**局部均值有多远（见 [`STRUCTURE_KERNEL`]）。
    fn activity(&self, pixels: &[u8], structure: &[f32], stride: usize) -> f32 {
        self.mean(stride, |sum, row| {
            let pixels = &pixels[row.clone()];
            let structure = &structure[row];
            for (&pixel, &structure) in pixels.iter().zip(structure) {
                *sum += (f32::from(pixel) - structure).abs();
            }
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
/// 判据的分块聚合、卷级上包络与特例页判据的立脚点（后两者见 `crate::envelope`）共用它。
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
        let floor = composition().floor(BitDepth::One);
        let over = floor + 10.0;

        // 候选比参照多出 地板 + 10：超出地板的那 10 级要收下。
        assert!((visible_grain(over + 8.0, 8.0, floor) - 10.0).abs() < 0.001);
        // 反过来，候选比参照少：一分不收，不是负数也不是那 10 级。
        assert_eq!(visible_grain(8.0, over + 8.0, floor), 0.0);
        // 刚好压在地板上：不收。
        assert_eq!(visible_grain(floor + 8.0, 8.0, floor), 0.0);
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

    /// 分带扫列与逐列扫**逐位相同**。
    ///
    /// 判据的读数最终要与阈值比大小，而 `f64` 的加法不结合：把一列的加减并成一句、
    /// 或者换一个折叠次序，判据都只是**略微**偏一点——性质测试全绿，黄金快照整片挪几个
    /// 字节。断言取的是逐位相等（`to_bits`），不是「差得不多」：分带买的是缓存局部性，
    /// 一个 ULP 都不该拿它去换。
    ///
    /// 宽度取得跨过带边界（16 与 16 的倍数各走到），核边长奇偶与两个尺度都走到。
    #[test]
    fn the_column_sweep_matches_one_column_at_a_time() {
        for width in [1u32, 15, 16, 17, 32, 33, 40] {
            let size = Size::new(width, 11);
            let pixels: Vec<u8> = (0..(size.width * size.height))
                .map(|index| (index * 37 % 251) as u8)
                .collect();
            for kernel in [2, 3, 4, TILE] {
                let (width, height) = (size.width as usize, size.height as usize);
                let before = ((kernel - 1) / 2) as usize;
                let after = (kernel - 1) as usize - before;
                let mut rows = vec![0f32; pixels.len()];
                for y in 0..height {
                    let row = y * width;
                    sliding_sum(before, after, 1.0, &mut rows[row..row + width], 1, |x| {
                        f64::from(pixels[row + x])
                    });
                }
                let scale = 1.0 / (kernel * kernel) as f64;

                // 逐列走一遍：本次改动之前纵向那一趟就是这么写的。
                let mut want = vec![0f32; pixels.len()];
                let last = (height - 1) * width;
                for x in 0..width {
                    sliding_sum(before, after, scale, &mut want[x..=x + last], width, |y| {
                        f64::from(rows[y * width + x])
                    });
                }

                let mut got = vec![0f32; pixels.len()];
                sweep_columns(before, after, scale, &rows, &mut got, width, height);

                for (index, (want, got)) in want.iter().zip(&got).enumerate() {
                    assert_eq!(
                        want.to_bits(),
                        got.to_bits(),
                        "宽 {width} 核 {kernel} 的第 {index} 格：逐列 {want}，分带 {got}"
                    );
                }
            }
        }
    }

    /// 三个块读数与**逐格累加**逐位相同。
    ///
    /// `Tile::mean` 现在把一行的下标区间交给闭包，调用方切成片再走。切片只该省掉边界检查，
    /// 不该动折叠次序——单精度的连加换个次序就漂，而漂出来的判据只是**略微**偏一点：
    /// 性质测试全绿，黄金快照整片挪几个字节。断言因此取 `to_bits()`。
    ///
    /// 逐格那一版就是改动之前的写法：一个 `f32` 累加器，行优先，一格一加。
    #[test]
    fn the_tile_readings_match_adding_one_cell_at_a_time() {
        let size = Size::new(70, 40);
        let stride = size.width as usize;
        let pixels: Vec<u8> = (0..(size.width * size.height))
            .map(|index| (index * 37 % 251) as u8)
            .collect();
        let one: Vec<f32> = (0..pixels.len()).map(|i| (i % 97) as f32 * 0.37).collect();
        let two: Vec<f32> = (0..pixels.len()).map(|i| (i % 89) as f32 * 0.53).collect();

        // 逐格累加：改动之前 `Tile::mean` 的形状。
        let cell_by_cell = |tile: &Tile, at: &dyn Fn(usize) -> f32| {
            let mut sum = 0f32;
            for y in tile.y..tile.y + tile.height {
                let row = y as usize * stride;
                for x in tile.x..tile.x + tile.width {
                    sum += at(row + x as usize);
                }
            }
            sum / (tile.width * tile.height) as f32
        };

        for tile in tiles(size) {
            let want = cell_by_cell(&tile, &|index| {
                let difference = one[index] - two[index];
                difference * difference
            })
            .sqrt();
            assert_eq!(
                want.to_bits(),
                tile.low_pass_error(&one, &two, stride).to_bits(),
                "低通项在 ({}, {}) 那一块上对不上",
                tile.x,
                tile.y
            );

            let want = cell_by_cell(&tile, &|index| {
                let difference = f32::from(pixels[index]) - one[index];
                difference * difference
            })
            .sqrt();
            assert_eq!(
                want.to_bits(),
                tile.grain(&pixels, &one, stride).to_bits(),
                "颗粒项在 ({}, {}) 那一块上对不上",
                tile.x,
                tile.y
            );

            let want = cell_by_cell(&tile, &|index| {
                (f32::from(pixels[index]) - two[index]).abs()
            });
            assert_eq!(
                want.to_bits(),
                tile.activity(&pixels, &two, stride).to_bits(),
                "掩蔽活动度在 ({}, {}) 那一块上对不上",
                tile.x,
                tile.y
            );
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

    /// 地板是**格点间距的一个比例**，不是三档的表：一个数乘各档自己的间距。
    ///
    /// 1bit 那一格**逐位**仍是 55.0——《位深盲测》整批数据全是在那个地板下量的，
    /// 它一动，那批数据的可比性就没了（measurements 的《颗粒项只在 1bit 上生效》）。
    /// 另两档由同一个比例推出、不另存一份表：三个数之间的算术关系存进表里就丢了，
    /// 而换一个位深不该要一次新标定。
    #[test]
    fn the_grain_floor_is_a_share_of_the_quantisation_step() {
        let floor = |depth| composition().floor(depth);

        assert_eq!(
            floor(BitDepth::One).to_bits(),
            55.0f32.to_bits(),
            "1bit 的地板不再逐位是 55.0，是 {}",
            floor(BitDepth::One)
        );
        // 另两档：同一个比例乘 85 与 17（measurements 的《颗粒项只在 1bit 上生效》）。
        assert!(
            (floor(BitDepth::Two) - 18.33).abs() < 0.01,
            "2bit 的地板是 {}",
            floor(BitDepth::Two)
        );
        assert!(
            (floor(BitDepth::Four) - 3.67).abs() < 0.01,
            "4bit 的地板是 {}",
            floor(BitDepth::Four)
        );
        // 地板与格点间距同比例：三档两两之比就是间距之比，一格不差。
        assert!((floor(BitDepth::One) / floor(BitDepth::Two) - 3.0).abs() < 0.001);
        assert!((floor(BitDepth::Two) / floor(BitDepth::Four) - 5.0).abs() < 0.001);
    }

    /// 颗粒地板的下界是**算出来的**，不是调出来的：低于它，「同一档上抖动优于不抖动」
    /// 这条性质会在某个灰调上翻掉——而那正是 ADR 0002 立判据时守的那一条。
    ///
    /// 推导写在 [`GRAIN_RATIO`] 的文档里。这里把它当成一条算术不变量钉住，
    /// **三档各钉一次**：谁把比例调到 0.2071 以下，三档里任何一档当场红。
    /// 地板还是绝对值时这一条只管得着 1bit——另两档上颗粒项恒读零，钉什么都是白钉。
    ///
    /// 断言只管颗粒项这一项。整个判据上抖动那一侧还要背自己的低通项——
    /// 平坦的中浅灰上两者因此仍会交叉，代价写在 ADR 0002 的《后果》里。
    #[test]
    fn the_grain_floor_stays_above_what_an_undithered_flat_tone_pays() {
        // 与最近格点差 u 的一块平坦灰调（`u ≤ s/2`）：不抖动的低通项读 u，
        // FS 的颗粒读 sqrt(u(s-u))。s 是那一档的格点间距。
        let worst_excess = |step: f32| {
            (0..=12_750)
                .map(|tick| {
                    let u = step * 0.5 * tick as f32 / 12_750.0;
                    (u * (step - u)).sqrt() - u
                })
                .fold(f32::MIN, f32::max)
        };

        for depth in [BitDepth::One, BitDepth::Two, BitDepth::Four] {
            let step = quantisation_step(depth);
            let worst = worst_excess(step);
            // 闭式解：最大值在 u/s = (2−√2)/4 处取到 0.2071·s（ADR 0002 的《第 5 条从哪来》）。
            assert!(
                (worst / step - 0.2071).abs() < 0.001,
                "{depth} 上推导的下界不再是 0.2071·s 了，是 {}·s",
                worst / step
            );
            assert!(
                composition().floor(depth) > worst,
                "{depth} 的地板 {} 低于下界 {worst}：这一档上抖动会在某个灰调上输给不抖动",
                composition().floor(depth)
            );
        }

        // 1bit 那一档的那个数：measurements 的《位深盲测》记的 52.8 就是它。
        assert!(
            (52.5..53.0).contains(&worst_excess(quantisation_step(BitDepth::One))),
            "1bit 的下界不再是 52.8 了"
        );
    }

    /// 掩蔽那两个数由 [`masking`] 一处出：报告读的那一份，就是加权真正用的那一份。
    ///
    /// 钉的是**类型不是装饰**——把 `masking()` 换掉，加权跟着换。
    ///
    /// 每条断言各钉加权曲线上一个**说得出名字的点**，外加一条单调不增；
    /// **当前那两个数字一个都不写死**，取的全是 `masking()` 交出来的 `floor` 与 `knee`。
    /// 它们是未标定占位值，标定把它们换掉时这一条不该跟着改——报告那几条断言同一条规矩。
    #[test]
    fn the_masking_weights_come_from_the_one_place_the_report_reads() {
        let Masking { floor, knee } = masking();
        // 单精度上比不得逐位相等：加权那一步是 `(1−地板)·拐点/(拐点+活动度)`，
        // `(a·k)/k` 只在 k 是 2 的幂时恰好还原 a，而拐点标定之后未必还是。
        let about = |left: f32, right: f32| (left - right).abs() < 1e-6;

        // 平坦块不打折：加权是**相对**的，平坦区取 1.0 作基准。
        assert!(
            about(masking_weight(0.0), 1.0),
            "平坦块上的加权是 {}，不是 1.0",
            masking_weight(0.0)
        );
        // 拐点上正落在「不打折」与地板的中点——那就是拐点这个词的定义。
        assert!(
            about(masking_weight(knee), (1.0 + floor) / 2.0),
            "拐点 {knee} 上的加权是 {}，不在 1.0 与地板 {floor} 的中点",
            masking_weight(knee)
        );
        // 地板是**下确界，到不了**：`拐点/(拐点+活动度)` 恒大于零，加权因此永远压不到
        // 地板以下、也永远不等于它。报告那一行照这个说法印（「压不到 0.50 以下」，
        // 不是「打到 0.50」）——说成到达就是屏上一句假话。
        assert!(masking_weight(255.0) > floor);
        // 只有活动度大到那一点余量在单精度上并进地板本身，才到得了 `>=`。
        assert!(masking_weight(f32::MAX) >= floor);
        // 单调不增：活动度越高折得越狠，中间不许翻头。
        let mut previous = f32::MAX;
        for tick in 0..=1_000 {
            let weight = masking_weight(tick as f32 * 0.1);
            assert!(
                weight <= previous,
                "活动度 {} 上加权翻了头",
                tick as f32 * 0.1
            );
            previous = weight;
        }
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
