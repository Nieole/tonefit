//! `run` 的输出。

use std::path::PathBuf;

use crate::cache::CacheUsage;
use crate::decide::{CandidateScore, Verdict};
use crate::envelope::Envelope;
use crate::geometry::Size;
use crate::profile::Profile;
use crate::quantize::BitDepth;
use crate::resample::Scaling;

/// 一次处理调用的结果。
///
/// spec 固定的形状里还有失败页集与计时，它们随各自的票落地（错误隔离见 12 号票）。
#[derive(Debug, Clone)]
pub struct Report {
    /// 本次实际使用的 profile 与它的面板。这批输出该拿去哪台设备看，答案在这里。
    pub profile: Profile,
    pub volumes: Vec<VolumeReport>,
}

/// 一个卷的结果。卷级的抖动模式随 09 号票落地。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 该卷的输出：目录卷是一个目录，归档卷是一个归档文件。
    pub output: PathBuf,
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
    /// 本卷的卷级判定：这一卷的位深从哪来。
    ///
    /// 一页都没有的卷（只装着透传文件）是 `None`：那样的卷没有位深可判。
    pub verdict: Option<VolumeVerdict>,
    /// 本卷缓存的用量（ADR 0005）。
    pub cache: CacheUsage,
    /// 本卷解码源页的次数。
    ///
    /// 两遍管线的不变量是「每页只解码一次」（ADR 0005：解码一次，缓存缩放后的图），
    /// 这个数因此恒等于页数。它在报告里，是为了让那条不变量在 `run` 这个 seam 上量得出来——
    /// 第二遍一旦回头碰源页，它立刻大于页数，而别的外部可见事实都察觉不到这件事。
    pub decodes: usize,
}

/// 卷级判定：这一卷的位深从哪来（spec 的卷报告形状：判定位深、判定理由、驱动页）。
///
/// 三者绑成一个枚举而不是三个字段，因为它们不是每一种情形下都同时成立：
/// `--per-page` 一开，卷级根本没有位深，也就没有驱动页可指。
///
/// spec 固定的 `Skipped` 随 11 号票落地。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeVerdict {
    /// 卷级上包络定的基准档（ADR 0006：位深按卷取上包络并加迟滞）。
    Envelope(Envelope),
    /// `--bit-depth` 顶掉了判定：全卷同一档，卷级基准档无从谈起（spec 的 story 23）。
    ///
    /// 顶掉之后每一页的判定都是覆盖，逐页结果里没有分布可聚合——上包络于是不是「被关掉」，
    /// 而是无从谈起。
    Override(BitDepth),
    /// `--per-page` 关掉了上包络与迟滞：位深逐页最优，卷内没有基准档
    /// （ADR 0006 决定第 6 条）。翻页跳变随之回来。
    PerPage,
}

/// 一页的结果。
///
/// 归档卷的页没有文件系统路径，`source` 与 `output` 因此是**卷路径接上成员的相对路径**，
/// 长成 `卷.cbz/001.png`。这是页在卷里的身份，不是一个打得开的路径——
/// 归档卷真正打得开的那个路径是 [`VolumeReport::output`]。
#[derive(Debug, Clone)]
pub struct PageReport {
    pub source: PathBuf,
    pub output: PathBuf,
    /// 目标尺寸：实际写出的像素尺寸。
    pub size: Size,
    /// 这一页实际走过的缩放：总缩放比、预缩倍数、残差比（ADR 0001）。
    ///
    /// 预缩这条路径在 B 类素材上从不触发（见 measurements 的《B 类素材普查》），
    /// 报告里说清楚它有没有触发，是这条路径在真实素材上唯一的现场证据。
    pub scaling: Scaling,
    /// 各候选的判据值，位深由小到大。候选已按面板灰阶数裁过（ADR 0003）。
    ///
    /// 两种模式都求值：判据现在决定输出的位深，dry-run 因此预告的就是照做时的那一档。
    pub scores: Vec<CandidateScore>,
    /// 这一页最终定下的位深，以及定它的理由（spec 的 story 7）。
    ///
    /// 卷级那一层开着时，这里是上包络重定过的结果，不是逐页判定的原始输出——
    /// 写出去的就是它。逐页判定要的那一档仍可从 [`scores`](Self::scores) 与阈值读出来。
    ///
    /// 判定说的是**量化格点**。文件里写着的那个位深可能更低——一页只用得上几个取值时，
    /// 调色板装得下同样的像素而位宽更窄，那是编码器接口以内的事（ADR 0004，见 `encode`）。
    pub verdict: Verdict,
}
