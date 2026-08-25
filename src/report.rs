//! `run` 的输出。

use std::path::PathBuf;

use crate::cache::CacheUsage;
use crate::color::PageColor;
use crate::decide::{CandidateScore, Verdict};
use crate::envelope::Envelope;
use crate::geometry::{GeometryGate, Size};
use crate::profile::Profile;
use crate::quantize::{Candidate, Dither};
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

/// 一个卷的结果。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 该卷的输出：目录卷是一个目录，归档卷是一个归档文件。
    pub output: PathBuf,
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
    /// 本卷的卷级判定：这一卷的候选从哪来。抖动模式在这个候选里。
    ///
    /// 一页都没有的卷（只装着透传文件）是 `None`：那样的卷没有候选可判。
    pub verdict: Option<VolumeVerdict>,
    /// 本卷的几何门判定（ADR 0007：抖动仅在目标尺寸未被下游缩放时启用）。
    ///
    /// 它不在 [`verdict`](Self::verdict) 里：门是几何的、判定是内容的，门先判，
    /// 判定在门放行的那套候选上做。门关着时抖动整体关闭，`verdict` 里那个候选
    /// 于是必然不抖——不说门，报告就解释不了「为什么这一卷没抖」。
    pub gate: GeometryGate,
    /// 本卷缓存的用量（ADR 0005）。
    pub cache: CacheUsage,
    /// 本卷解码源页的次数。
    ///
    /// 两遍管线的不变量是「每页只解码一次」（ADR 0005：解码一次，缓存缩放后的图），
    /// 这个数因此恒等于页数。它在报告里，是为了让那条不变量在 `run` 这个 seam 上量得出来——
    /// 第二遍一旦回头碰源页，它立刻大于页数，而别的外部可见事实都察觉不到这件事。
    pub decodes: usize,
}

/// 卷级判定：这一卷的候选从哪来（spec 的卷报告形状：判定位深、抖动模式、判定理由、驱动页）。
///
/// 几项绑成一个枚举而不是各占一个字段，因为它们不是每一种情形下都同时成立：
/// `--per-page` 一开，卷级根本没有候选，也就没有驱动页可指。
///
/// spec 固定的 `Skipped` 随 11 号票落地。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeVerdict {
    /// 卷级上包络定的基准档（ADR 0006：位深按卷取上包络并加迟滞）。
    /// 抖动那一维跟着位深一起定在这里（ADR 0007：上包络取的是这个组合）。
    Envelope(Envelope),
    /// 覆盖项把候选裁到只剩一个：判定被顶掉，卷级基准档无从谈起（spec 的 story 23）。
    ///
    /// 裁到只剩一个之后每一页的判定都是覆盖，逐页结果里没有分布可聚合——上包络于是不是
    /// 「被关掉」，而是无从谈起。裁不到只剩一个的覆盖不走这里：`--bit-depth` 点了一档位深、
    /// 而几何门开着时，抖动那一维还有得判，卷级仍是一次上包络。
    Override(Candidate),
    /// `--per-page` 关掉了上包络与迟滞：候选逐页最优，卷内没有基准档
    /// （ADR 0006 决定第 6 条）。翻页跳变随之回来，抖动模式也跟着逐页可变。
    PerPage,
}

impl VolumeVerdict {
    /// 这一卷定下的抖动模式（09 号票：`Report` 要标明本卷的抖动模式）。
    ///
    /// `--per-page` 下没有卷级的那一个：抖动跟着位深一起逐页可变
    /// （ADR 0006 决定第 6 条）。
    pub fn dither(&self) -> Option<Dither> {
        match self {
            VolumeVerdict::Envelope(envelope) => Some(envelope.base.dither),
            VolumeVerdict::Override(candidate) => Some(candidate.dither),
            VolumeVerdict::PerPage => None,
        }
    }
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
    /// 第一遍识别出这一页是彩页还是灰度页（ADR 0005 决定第 1 条：彩页识别在第一遍）。
    ///
    /// 这是**页的事实**，与它走了哪条分支不是一回事：黑白 profile 下彩页转灰、
    /// 走的是 [`PageBranch::Gray`]，但它仍然是一张彩页。两者都在报告里，
    /// 「这一页为什么没保留颜色」才答得出来。
    pub color: PageColor,
    /// 这一页走的那条分支，连同那条分支的产物。
    pub branch: PageBranch,
}

/// 一页走过的那条分支（ADR 0010：彩页按 profile 分流）。
///
/// 两条分支的产物不是同一套，所以它们是一个枚举而不是几个各自可空的字段：
/// 彩色分支上没有判据曲线、也没有判定——那条路径根本不量化，说「判定为空」
/// 会读成「判定丢了」。
#[derive(Debug, Clone)]
pub enum PageBranch {
    /// 灰度路径：算判据、进缓存、第二遍照判定量化写出。
    /// 黑白 profile 下的彩页转灰后也走这里（ADR 0005 决定第 4 条）。
    Gray {
        /// 各候选的判据值，由小到大。候选已按面板灰阶数与几何门裁过（ADR 0003、ADR 0007）。
        ///
        /// 两种模式都求值：判据现在决定输出的候选，dry-run 因此预告的就是照做时的那一个。
        scores: Vec<CandidateScore>,
        /// 这一页最终定下的候选，以及定它的理由（spec 的 story 7）。
        ///
        /// 卷级那一层开着时，这里是上包络重定过的结果，不是逐页判定的原始输出——
        /// 写出去的就是它。逐页判定要的那一档仍可从 `scores` 与阈值读出来。
        ///
        /// 判定说的是**量化格点**。文件里写着的那个位深可能更低——一页只用得上几个取值时，
        /// 调色板装得下同样的像素而位宽更窄，那是编码器接口以内的事（ADR 0004，见 `encode`）。
        verdict: Verdict,
    },
    /// 彩色分支：只做缩放，不量化、不进灰度缓存、不进卷级上包络
    /// （ADR 0005 决定第 4 条）。彩色 profile 下的彩页走这里。
    Color,
}

impl PageReport {
    /// 这一页定下的候选与理由。彩色分支上没有——那条路径不量化。
    pub fn verdict(&self) -> Option<Verdict> {
        match &self.branch {
            PageBranch::Gray { verdict, .. } => Some(*verdict),
            PageBranch::Color => None,
        }
    }

    /// 这一页各候选的判据值，由小到大。彩色分支上是空的。
    pub fn scores(&self) -> &[CandidateScore] {
        match &self.branch {
            PageBranch::Gray { scores, .. } => scores,
            PageBranch::Color => &[],
        }
    }
}
