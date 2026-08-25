//! `run` 的输出。

use std::path::PathBuf;

use crate::geometry::Size;
use crate::metric::Score;
use crate::profile::Profile;
use crate::quantize::BitDepth;

/// 一次处理调用的结果。
///
/// spec 固定的形状里还有失败页集与计时，它们随各自的票落地（错误隔离见 12 号票）。
#[derive(Debug, Clone)]
pub struct Report {
    /// 本次实际使用的 profile 与它的面板。这批输出该拿去哪台设备看，答案在这里。
    pub profile: Profile,
    pub volumes: Vec<VolumeReport>,
}

/// 一个卷的结果。判定位深、抖动模式与判定理由随位深判定一起落地（06、08 号票）。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 该卷的输出：目录卷是一个目录，归档卷是一个归档文件。
    pub output: PathBuf,
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
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
    /// 各候选的判据值，位深由小到大。候选已按面板灰阶数裁过（ADR 0003）。
    ///
    /// 只有 dry-run 求值：判据此刻还不改变输出，处理路径上算了也没人看（04 号票）。
    /// 据判据选出一档、并给出判定理由，是 06 号票。
    pub scores: Vec<CandidateScore>,
}

/// 一个候选的判据值。候选此刻只有位深这一维，抖动模式那一维随 09 号票加进来。
///
/// 判据是量、阈值是界：这里只有量。判据数值不可跨面板比较（ADR 0002），
/// 要看是哪块面板上的数，见 [`Report::profile`]。
#[derive(Debug, Clone, Copy)]
pub struct CandidateScore {
    pub bit_depth: BitDepth,
    pub score: Score,
}
