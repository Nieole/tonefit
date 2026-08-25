//! 目标尺寸的算法与几何门：fit-inside 且不放大，以及输出与面板像素对不对得上。

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

/// 几何门：这一页的目标尺寸贴住面板了吗——贴住才谈得上「输出不再被下游缩放」。
///
/// fit-inside 把等比缩小的页顶到面板的一条边上，另一条边留边。漫画页更瘦，留边出在两侧，
/// 阅读器填背景、不重采样，1:1 仍然成立（ADR 0007）。一条边都没贴住只有一种来路：
/// 源比目标小、按不放大原样输出——那一页到了阅读器手里还要被放大一次。
///
/// **这是本仓库唯一一处判定几何门的地方。**ADR 0003 的灰阶硬上界与 ADR 0007 的抖动
/// 依赖的是同一条不变量，ADR 0003 因此要求两者判定同源、不许各写一份。
pub fn one_to_one(target: Size, panel: Size) -> bool {
    target.width == panel.width || target.height == panel.height
}

/// 一个卷的几何门判定结果。
///
/// 门是**几何**的，不取决于页上有什么内容，因此**不参与卷级统计**——ADR 0007 的《决定》
/// 说的是「几何门先判」。它对整卷只有一个结果：不成立时抖动**整体关闭**、
/// 「不降级成更温和的抖动模式」，成立时也「不设页级抖动开关」。
/// 于是卷内只要有一页不成立，整卷就不成立。
///
/// 不成立的那一页要指得出来——与上包络指出驱动页同一个做法：门关掉了一整卷的抖动，
/// 报告不说是哪一页关的，用户就无从判断这一卷该不该换个 profile。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryGate {
    /// 成立：卷内每一页的目标尺寸都贴住面板。
    Holds,
    /// 不成立：这一页源比目标小，按不放大原样输出（spec 的 story 17），阅读器还要再缩一次。
    /// 序号指进 [`crate::VolumeReport::pages`]。
    Broken { page: usize },
}

impl GeometryGate {
    /// 门成立吗。
    pub fn holds(self) -> bool {
        self == GeometryGate::Holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 基准面板。几何门的用例只关心「贴没贴住」，不关心是哪块面板。
    const PANEL: Size = Size::new(1264, 1680);

    /// 缩下来的页贴住高的那条边：宽度小于面板宽是留边，1:1 仍然成立（ADR 0007）。
    #[test]
    fn a_page_scaled_down_to_the_panel_keeps_the_gate_open() {
        // B 类中位页（见 measurements 的《B 类素材普查》）：缩到 1182×1680，宽边留边。
        let target = fit_inside(Size::new(1441, 2048), PANEL);
        assert_eq!(target, Size::new(1182, 1680));
        assert!(one_to_one(target, PANEL));

        // 跨页宽幅页反过来贴住宽的那条边，上下留边。
        let spread = fit_inside(Size::new(5056, 1680), PANEL);
        assert_eq!(spread, Size::new(1264, 420));
        assert!(one_to_one(spread, PANEL));
    }

    /// 两边都小于面板的页按不放大原样输出，一条边都贴不住：阅读器会把它放大，门不成立。
    /// B 类里这样的页占比不低（1264×1680 面板上 19%），ADR 0007 认下的正是这笔代价。
    #[test]
    fn a_page_smaller_than_the_panel_breaks_the_gate() {
        let target = fit_inside(Size::new(800, 1000), PANEL);
        assert_eq!(target, Size::new(800, 1000), "小于目标的页不该被放大");
        assert!(!one_to_one(target, PANEL));
    }

    /// 只有一条边够长的页也原样输出，但那条边恰好贴住面板：阅读器按 fit-inside 显示
    /// 同样不必重采样，门成立。判定看的是贴没贴住，不是缩没缩过。
    #[test]
    fn a_page_that_already_touches_one_edge_keeps_the_gate_open() {
        let target = fit_inside(Size::new(PANEL.width, 1000), PANEL);
        assert_eq!(target, Size::new(PANEL.width, 1000));
        assert!(one_to_one(target, PANEL));
        // 差一个像素就贴不住了，阅读器会放大 0.08%——那就不是 1:1。
        let shy = fit_inside(Size::new(PANEL.width - 1, 1000), PANEL);
        assert!(!one_to_one(shy, PANEL));
    }
}
