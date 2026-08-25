//! `run` 的输入。

use std::path::PathBuf;

use crate::profile::Profile;

/// 一次处理调用的全部输入。
///
/// spec 固定的形状里还有 Mode 与覆盖项，它们随各自的票落地：
/// 位深与滤波器覆盖见 05、06 号票。
#[derive(Debug, Clone)]
pub struct Request {
    /// 点名要处理的卷。空集是错误，不做全库扫描（ADR 0009）。
    pub inputs: Vec<PathBuf>,
    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    pub output_root: PathBuf,
    /// 目标设备。目标尺寸由它的面板算出，`--gray-levels` 的覆盖已经折进来了。
    pub profile: Profile,
}
