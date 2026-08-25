//! `run` 的输入。

use std::path::PathBuf;

use crate::profile::Profile;

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
/// spec 固定的形状里还有覆盖项，它们随各自的票落地：位深与滤波器覆盖见 05、06 号票。
#[derive(Debug, Clone)]
pub struct Request {
    /// 点名要处理的卷。空集是错误，不做全库扫描（ADR 0009）。
    pub inputs: Vec<PathBuf>,
    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    pub output_root: PathBuf,
    /// 目标设备。目标尺寸由它的面板算出，`--gray-levels` 的覆盖已经折进来了。
    pub profile: Profile,
    /// 做到哪一步。
    pub mode: Mode,
}
