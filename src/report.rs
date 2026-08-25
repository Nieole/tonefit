//! `run` 的输出。

use std::path::PathBuf;

use crate::geometry::Size;
use crate::profile::Profile;

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
}
