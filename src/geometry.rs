//! 目标尺寸的算法：fit-inside，且不放大。

/// 像素尺寸。用于面板分辨率与目标尺寸两处——它们不是一回事，见 `CONTEXT.md`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

/// 目标尺寸 = fit-inside(面板分辨率)：等比缩到能整个放进面板，源比目标小时保持原尺寸。
///
/// 不放大是明确要求（spec 的 story 17）：放大只会让本就糊的页更糊。
pub fn fit_inside(source: Size, panel: Size) -> Size {
    let scale = f64::min(
        f64::from(panel.width) / f64::from(source.width),
        f64::from(panel.height) / f64::from(source.height),
    );
    if scale >= 1.0 {
        return source;
    }
    Size::new(
        scaled(source.width, scale, panel.width),
        scaled(source.height, scale, panel.height),
    )
}

/// 取整后夹在 `[1, 面板边长]` 内：四舍五入可能溢出面板一个像素，而目标尺寸恒不得超过面板。
fn scaled(length: u32, scale: f64, limit: u32) -> u32 {
    let rounded = (f64::from(length) * scale).round() as u32;
    rounded.clamp(1, limit)
}
