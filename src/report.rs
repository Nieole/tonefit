//! `run` 的输出。

use std::path::PathBuf;

use crate::cache::CacheUsage;
use crate::decide::{CandidateScore, Verdict};
use crate::geometry::Size;
use crate::profile::Profile;
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

/// 一个卷的结果。卷级的判定位深、抖动模式与判定理由随上包络落地（08 号票）；
/// 此刻判定是逐页的，见 [`PageReport::verdict`]。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 该卷的输出：目录卷是一个目录，归档卷是一个归档文件。
    pub output: PathBuf,
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
    /// 本卷缓存的用量（ADR 0005）。
    pub cache: CacheUsage,
    /// 本卷解码源页的次数。
    ///
    /// 两遍管线的不变量是「每页只解码一次」（ADR 0005：解码一次，缓存缩放后的图），
    /// 这个数因此恒等于页数。它在报告里，是为了让那条不变量在 `run` 这个 seam 上量得出来——
    /// 第二遍一旦回头碰源页，它立刻大于页数，而别的外部可见事实都察觉不到这件事。
    pub decodes: usize,
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
    /// 这一页定下的位深，以及定它的理由（spec 的 story 7）。
    ///
    /// 判定说的是**量化格点**。文件里写着的那个位深可能更低——一页只用得上几个取值时，
    /// 调色板装得下同样的像素而位宽更窄，那是编码器接口以内的事（ADR 0004，见 `encode`）。
    pub verdict: Verdict,
}
