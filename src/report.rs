//! `run` 的输出。

use std::path::PathBuf;

use crate::geometry::Size;

/// 一次处理调用的结果。
///
/// spec 固定的形状里还有失败页集与计时，它们随各自的票落地（错误隔离见 12 号票）。
#[derive(Debug, Clone)]
pub struct Report {
    pub volumes: Vec<VolumeReport>,
}

/// 一个卷的结果。判定位深、抖动模式与判定理由随位深判定一起落地（06、08 号票）。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径。
    pub volume: PathBuf,
    /// 该卷的输出目录。
    pub output: PathBuf,
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
}

/// 一页的结果。
#[derive(Debug, Clone)]
pub struct PageReport {
    pub source: PathBuf,
    pub output: PathBuf,
    /// 目标尺寸：实际写出的像素尺寸。
    pub size: Size,
}
