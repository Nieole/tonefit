//! `run` 的输入。

use std::path::PathBuf;

use crate::cache::CacheBudget;
use crate::profile::Profile;
use crate::quantize::BitDepth;
use crate::resample::Filter;

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
/// spec 固定的形状里还有别的覆盖项，它们随各自的票落地：抖动模式见 09 号票，
/// 介质见 13 号票。
#[derive(Debug, Clone)]
pub struct Request {
    /// 点名要处理的卷。空集是错误，不做全库扫描（ADR 0009）。
    pub inputs: Vec<PathBuf>,
    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    pub output_root: PathBuf,
    /// 目标设备。目标尺寸由它的面板算出，`--gray-levels` 的覆盖已经折进来了。
    pub profile: Profile,
    /// 残差段的重采样滤波器（`--filter`）。整数倍预缩那一级不受它影响（ADR 0001）。
    pub filter: Filter,
    /// 位深覆盖（`--bit-depth`）。给了就顶掉自动判定，特殊卷靠它手工兜底（spec 的 story 23）。
    ///
    /// 顶掉的只是判定。面板灰阶数那道硬上界仍在，越界的覆盖当场被拒（ADR 0003）。
    pub bit_depth: Option<BitDepth>,
    /// 缓存预算（`--cache-budget`）：两遍之间的缓存最多在内存里留这么多字节，
    /// 超出的页溢写临时文件（ADR 0005）。限的是峰值内存，不是卷的大小上限。
    pub cache_budget: CacheBudget,
    /// 做到哪一步。
    pub mode: Mode,
}
