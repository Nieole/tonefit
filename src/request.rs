//! `run` 的输入。

use std::path::PathBuf;

/// 一次处理调用的全部输入。
///
/// spec 固定的形状里还有 Profile、Mode 与覆盖项，它们随各自的票落地：
/// profile 见 02 号票，位深与滤波器覆盖见 05、06 号票。
#[derive(Debug, Clone)]
pub struct Request {
    /// 点名要处理的卷。空集是错误，不做全库扫描（ADR 0009）。
    pub inputs: Vec<PathBuf>,
    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    pub output_root: PathBuf,
}
