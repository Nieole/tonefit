//! `run` 的输入。

use std::path::PathBuf;

use crate::cache::CacheBudget;
use crate::geometry::FitMode;
use crate::medium::IoMode;
use crate::profile::Profile;
use crate::progress::ProgressSink;
use crate::quantize::{BitDepth, Dither};
use crate::resample::Filter;
use crate::spread::SplitRule;
use crate::white::WhiteAlignLimit;

/// 一次调用做到哪一步。
///
/// spec 固定的形状里还有 `Calibrate`，它随 14 号票落地。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 照常处理并写出。
    #[default]
    Process,
    /// 只算不写：判据照求、报告照出，一个文件都不落盘（spec 的 story 6）。
    DryRun,
}

/// 一次处理调用的全部输入。
///
/// spec 固定的形状里还有一个覆盖项没有落地：编码器（ADR 0004 说了 P0 只实现 PNG）。
#[derive(Debug, Clone)]
pub struct Request {
    /// 点名要处理的卷。空集是错误，不做全库扫描（ADR 0009）。
    pub inputs: Vec<PathBuf>,
    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    pub output_root: PathBuf,
    /// 目标设备。目标尺寸由它的面板算出，`--gray-levels` 的覆盖已经折进来了。
    pub profile: Profile,
    /// 这一趟怎么把页适配到面板（`--fit`，页几何批 01 号票）。
    ///
    /// 默认 [`FitMode::Height`]：目标高恒等于面板高，宽按源宽高比算出、允许超过面板宽。
    /// 它是**读法偏好**，不是面板的物理事实，因此不进 profile——profile 的主键是面板
    /// （`CONTEXT.md`）。
    ///
    /// 它改的是**目标尺寸**，而目标尺寸是几何门、总缩放比与判据参照三样东西的来源：
    /// 换一个适配方式，这一卷的判定要重算，幂等因此收着它（见 `crate::metadata`）。
    pub fit: FitMode,
    /// 裁不裁掉页面白边（`--no-crop` 关掉它，页几何批 02 号票）。**默认裁**。
    ///
    /// 要点不是省白边，是让用户**关得掉阅读器那一侧的裁切**：阅读器裁掉白边就改变了页尺寸，
    /// 随后的适配不再是 1.0 倍，抖动连同 1 像素周期的结构一起被抹平（见 measurements 的
    /// 《真机像素完整性》）。裁边收进来之后好处烤进产物，那个开关变成空操作。
    ///
    /// 裁法按行列墨量占比、逐页各裁各的，详见 `crate::crop`。它改的是**适配之前**的页尺寸，
    /// 目标尺寸、几何门、判据参照与判定因此全跟着变，幂等收着它（见 `crate::metadata`）。
    pub crop: bool,
    /// 这一趟怎么拆跨页（`--no-split`、`--split-threshold`、`--reading-order`，
    /// 页几何批 04 号票）。**默认拆。**
    ///
    /// 三项收成一个类型而不是三个相邻的字段：它们总是一同传下去，
    /// 而三个同型的裸参数换了位置编译器一句话都不会说（见 [`SplitRule`]）。
    ///
    /// 它改的是**这一卷有几页、每一页是哪一块**：一个源页有装订沟就切成两张输出页，
    /// 没有就是连续跨页、原样出一张。成员名、页尺寸与判定因此全跟着变，
    /// 幂等收着这三项（见 `crate::metadata`）。
    pub split: SplitRule,
    /// 残差段的重采样滤波器（`--filter`）。整数倍预缩那一级不受它影响（ADR 0001）。
    pub filter: Filter,
    /// 纸白对齐的上限（`--white-align-limit`，纸白对齐批 01、05 号票）。
    /// **默认 4 级，即默认开着**（取值出处只有 [`WhiteAlignLimit::default`]）；取 0 是关闭。
    ///
    /// 它是**口味层**的一项：这一趟愿意为对齐付多少色调，不是面板的物理事实。
    /// 上限即开关，不另做布尔开关（理由见 [`WhiteAlignLimit`]）。
    ///
    /// 它改的是**缩放之后、构造参照之前**那一步的像素，参照与其后一切量化因此都跟着变，
    /// 幂等收着它（见 `crate::metadata`）——**取值 0 也照样收**：从「关」改到「开」
    /// 与从 4 改到 2 是同一类改动，而 ADR 0002 的《后果》里记过一次漏收的同型事故。
    pub white_align_limit: WhiteAlignLimit,
    /// 位深覆盖（`--bit-depth`）。特殊卷靠它手工兜底（spec 的 story 23）。
    ///
    /// 覆盖项裁的是**候选集**：点名一档位深，候选就只剩那一档的。裁到只剩一个候选时判定
    /// 整个被顶掉；只点位深而其余页那一组的几何门开着时，抖动那一维还有得判，判据照旧说了算。
    ///
    /// 裁得掉的只有候选。面板灰阶数那道硬上界仍在，越界的覆盖当场被拒（ADR 0003）。
    pub bit_depth: Option<BitDepth>,
    /// 抖动模式覆盖（`--dither`）。与 [`bit_depth`](Self::bit_depth) 同一个作用方式：
    /// 裁候选集的另一维。
    ///
    /// 裁得掉的同样只有候选。几何门是**页的**几何事实、不是自动选择，点名抖动顶不掉它——
    /// 卷里有一页门不成立，`--dither fs` 就当场被拒（错误指得出是哪一页），不静默照抖。
    /// 拒绝仍是整趟的：覆盖项是用户的显式指令，不是可以按页悄悄放弃的东西
    /// （ADR 0007 的《后果》）。
    pub dither: Option<Dither>,
    /// 关掉卷级上包络（`--per-page`），位深回到逐页最优，档位由段式迟滞收一道。
    ///
    /// 给「只要最小体积」留的出口（ADR 0006 决定第 6 条）。一开，本决定的保护即降级：
    /// 相邻两页重新可能落在不同档上，翻页处的颗粒感因此会变粗细——**孤立地高出邻居的
    /// 那些页除外**，它们由段式迟滞压回邻居那一档（`crate::hysteresis`）。
    /// 孤立地要得**更低**的那一页不在此列：压它得往上抬，而抬是上包络那一层的事。
    pub per_page: bool,
    /// 缓存预算（`--cache-budget`）：两遍之间的缓存最多在内存里留这么多字节，
    /// 超出的页溢写临时文件（ADR 0005）。限的是峰值内存，不是卷的大小上限。
    pub cache_budget: CacheBudget,
    /// 做到哪一步。
    pub mode: Mode,
    /// 读取策略（`--io-mode`），覆盖按路径的介质探测。
    ///
    /// 默认 [`IoMode::Auto`]：路径解析到所在的卷再探寻道惩罚，有惩罚的串行读、
    /// 没有的并发读（ADR 0009 决定第 2 条）。探不出来的一律按未知退到串行——
    /// NAS 与网络路径都落在那里，而它们的最优策略尚未测量（`CONTEXT.md` 的《尚未确立》），
    /// 实测出并发更快的用户从这里点名。
    pub io_mode: IoMode,
    /// 进度观察者（spec 的 story 30）。`None` 即没人看着，管线一步都不报。
    ///
    /// 库这一侧只报到，印在哪、印不印由调用方定——CLI 接 indicatif，用例接一个计数器。
    pub progress: Option<ProgressSink>,
    /// 写不写自描述元数据（`--no-metadata` 关掉它）。
    ///
    /// 判定与理由随输出文件走，同一批字段兼作幂等依据（ADR 0006）。关掉之后两件事一起消失：
    /// 输出不再说得出自己是怎么来的，重跑时也无从判断这一卷变没变——每一趟都整卷重做。
    /// 幂等不是元数据之外的另一件事，它就搭在这批字段上，因此没有「不写但仍跳过」这一档。
    pub metadata: bool,
}
