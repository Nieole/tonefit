//! 目标尺寸的算法与几何门：两种适配方式各怎么算出目标尺寸，以及输出与面板像素对不对得上。

use anyhow::{Result, anyhow};

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

/// 适配方式：目标尺寸怎么由源尺寸与面板算出（页几何批 01 号票）。
///
/// 两条路只在**比面板更扁的页**与**比面板矮的页**上分岔。漫画页比面板更瘦长、本来就受
/// 高度约束，两种方式在它们身上产出**同一个尺寸**——那不是巧合，是「受高度约束」的直接
/// 后果，前提与扫描都写在 `the_two_fit_modes_agree_on_every_page_that_is_height_constrained`
/// 那条用例里。实测棋魂 230 页、N和S 24 页 100% 一致（measurements 的《适配方式：fit-inside 与以高为准》）。
///
/// **它对阅读器的要求不是同一个。** fit-inside 留边，只要求阅读器填背景、不重采样；
/// 以高为准在宽边溢出，要求阅读器平移、不缩放——后者更强。两者都落在
/// **像素完整性**那一层（`CONTEXT.md`），tonefit 探不到，只有标定图问得出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FitMode {
    /// 以高为准（默认）：目标高**恒等于**面板高，宽按源宽高比算出，**不设上限**。
    ///
    /// 比面板矮的页放大到面板高，比面板扁的页宽边溢出面板——实测最宽 3.22 倍面板宽
    /// （改革之獸 7162×3000 → 3457×1448，见 measurements 的《适配方式：fit-inside 与以高为准》）。
    /// 跨页因此从压扁状态变得可读，代价是那样的卷体积涨到约三四倍。
    #[default]
    Height,
    /// fit-inside：等比缩到整个放进面板，源比面板小时**不放大**、保持原尺寸。
    ///
    /// 这是本仓库 P0 唯一的一条路（ADR 0007 决定第 7 条），眼下是显式可选项。
    Inside,
}

impl FitMode {
    /// 按名字解析。大小写不论，两边的空白不算。
    pub fn resolve(name: &str) -> Result<Self> {
        let key = name.trim().to_ascii_lowercase();
        FIT_MODES
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, mode)| *mode)
            .ok_or_else(|| unknown_fit_mode_error(name))
    }

    /// 这个适配方式的规范名，取表里第一个指向它的那个。
    ///
    /// 参数哈希拿它当稳定写法（见 `crate::metadata`）：那串字节要落进输出文件、
    /// 几个月后还要比对，因此不能搭在 `Debug` 那种没有稳定承诺的写法上。
    pub(crate) fn name(self) -> &'static str {
        FIT_MODES
            .iter()
            .find(|(_, mode)| *mode == self)
            .map(|(name, _)| *name)
            .expect("表覆盖全部适配方式")
    }

    /// 这一页的目标尺寸。**本仓库算目标尺寸的唯一入口。**
    ///
    /// 两条路各自的算法见 [`fit_height`] 与 [`fit_inside`]。
    pub fn target(self, source: Size, panel: Size) -> Size {
        match self {
            FitMode::Height => fit_height(source, panel),
            FitMode::Inside => fit_inside(source, panel),
        }
    }
}

impl std::fmt::Display for FitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FitMode::Height => "以高为准（宽随源比例，允许超出面板宽）",
            FitMode::Inside => "fit-inside（整页放进面板，不放大）",
        })
    }
}

/// 名字 → 适配方式。同一个变体可以有多个名字，头一个是规范名。
const FIT_MODES: &[(&str, FitMode)] = &[
    ("height", FitMode::Height),
    ("fit-height", FitMode::Height),
    ("inside", FitMode::Inside),
    ("fit-inside", FitMode::Inside),
];

/// 未知适配方式的说法：把认得的名字全端出来，并说清两者差在哪。
fn unknown_fit_mode_error(name: &str) -> anyhow::Error {
    let names: Vec<_> = FIT_MODES.iter().map(|(name, _)| *name).collect();
    anyhow!(
        "未知适配方式「{name}」。认得的是：{}。\
         height 让目标高恒等于面板高、宽允许超出面板（靠阅读器横向平移）；\
         inside 把整页放进面板、源比面板小时不放大。",
        names.join(" ")
    )
}

/// 以高为准：目标高恒等于面板高，宽按源宽高比算出，**不设上限**。
///
/// 与 [`fit_inside`] 差在两处，两处都是有意的：
///
/// - **比面板矮的页放大到面板高。**几何门因此在这条路上每一页都成立，
///   抖动不再被那批页关掉（ADR 0007 决定第 1 条）。
/// - **比面板扁的页宽边溢出面板。**跨页不再被长边压扁——哆啦A梦最宽一页从
///   1072×766 变成 2027×1448。溢出的部分靠阅读器横向平移，代价见 [`FitMode::Height`]。
pub fn fit_height(source: Size, panel: Size) -> Size {
    let scale = f64::from(panel.height) / f64::from(source.height);
    // 宽不夹上界：用户明确接受任意宽度的横向翻动（01 号票的《不要做的》）。
    Size::new(scaled(source.width, scale), panel.height)
}

/// fit-inside：等比缩到能整个放进面板，源比目标小时保持原尺寸。
///
/// 不放大是这条路上的明确要求（spec 的 story 17）：放大只会让本就糊的页更糊。
/// 以高为准那条路的立场相反——它宁可放大也要贴住面板高，理由见 [`fit_height`]。
pub fn fit_inside(source: Size, panel: Size) -> Size {
    let scale = f64::min(
        f64::from(panel.width) / f64::from(source.width),
        f64::from(panel.height) / f64::from(source.height),
    );
    if scale >= 1.0 {
        return source;
    }
    // 四舍五入可能溢出面板一个像素，而 fit-inside 的目标尺寸恒不得超过面板。
    Size::new(
        scaled(source.width, scale).min(panel.width),
        scaled(source.height, scale).min(panel.height),
    )
}

/// 按 `scale` 缩过之后的边长，**至少 1 像素**：取整可能得 0，而 0 像素的页写不出去。
fn scaled(length: u32, scale: f64) -> u32 {
    ((f64::from(length) * scale).round() as u32).max(1)
}

/// 一页的几何门判定：这一页的目标尺寸贴住面板了吗——贴住才谈得上「输出不再被下游缩放」。
///
/// **门逐页判，一页的门只决定这一页**（ADR 0007 决定第 1 条）。它取决于几何、
/// 不取决于页上有什么内容，因此不参与卷级统计：卷级那一层（ADR 0006 的上包络）
/// 建在门放行的那套候选之上，门先判。
///
/// 不成立时抖动**整体关闭**，「不降级成更温和的抖动模式」；成立时也「不设页级抖动开关」，
/// 抖不抖跟着位深一起按卷决定。页级变化的只有「几何能不能承载抖动」这一件事，
/// 判据够不着它。
///
/// 判定范围是**灰度路径上的每一页**：彩色分支上的页不在范围内（ADR 0010 决定第 4 条——
/// 那条路径既不量化也不抖动），失败页连几何都没有。部分救回页在范围内，
/// 它的尺寸是文件头里的真尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryGate {
    /// 成立：这一页的目标尺寸贴住面板。
    Holds,
    /// 不成立：源比目标小，按不放大原样输出（spec 的 story 17），阅读器还要再缩一次。
    ///
    /// **只有 [`FitMode::Inside`] 那条路走得到它。**以高为准让每一页的高恒等于面板高，
    /// 一条边永远贴着（见 [`GeometryGate::of`]）。
    Broken,
}

impl GeometryGate {
    /// 判这一页的门。
    ///
    /// 判据只有一条：目标尺寸有没有贴住面板的某一条边。贴住了，面板这一层的适配就是 1.0 倍，
    /// 「输出不再被下游缩放」谈得上（ADR 0007）。两条适配方式各自怎么落到这一条上：
    ///
    /// - **fit-inside** 把等比缩小的页顶到面板的一条边上，另一条边**留边**。
    ///   漫画页更瘦，留边出在两侧，阅读器填背景、不重采样，1:1 仍然成立。
    ///   一条边都没贴住只有一种来路：源比目标小、按不放大原样输出——那一页到了阅读器手里
    ///   还要被放大一次，门在它身上不成立。
    /// - **以高为准**让目标高恒等于面板高，高那条边因此**每一页**都贴着，门恒成立
    ///   （01 号票）。宽那条边不再留边而是**溢出**面板。
    ///
    /// **留边与溢出对阅读器的要求不是同一个，门问的却是同一件事。** 留边只要求阅读器
    /// 填背景、不重采样；溢出要求它平移、不缩放——后者更强。门问的是
    /// 「tonefit 交出去的像素与面板像素对不对得上」，两种情形下答案都是「对得上」；
    /// 阅读器实际上照不照做，是**像素完整性**那一层（`CONTEXT.md`），门够不着，
    /// 只有标定图问得出来。以高为准把这一层的要求提高了，而门的读数不变。
    ///
    /// **这是本仓库唯一一处判定几何门的地方。**ADR 0003 的灰阶硬上界与 ADR 0007 的抖动
    /// 依赖的是同一条不变量，ADR 0003 因此要求两者判定同源、不许各写一份。
    pub fn of(target: Size, panel: Size) -> Self {
        if target.width == panel.width || target.height == panel.height {
            GeometryGate::Holds
        } else {
            GeometryGate::Broken
        }
    }

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
        assert!(GeometryGate::of(target, PANEL).holds());

        // 跨页宽幅页反过来贴住宽的那条边，上下留边。
        let spread = fit_inside(Size::new(5056, 1680), PANEL);
        assert_eq!(spread, Size::new(1264, 420));
        assert!(GeometryGate::of(spread, PANEL).holds());
    }

    /// 两边都小于面板的页按不放大原样输出，一条边都贴不住：阅读器会把它放大，门不成立。
    /// B 类里这样的页占比不低（1264×1680 面板上 19%），ADR 0007 认下的正是这笔代价——
    /// 但只这一页认，同卷里贴住面板的页照旧抖得动（ADR 0007 决定第 1 条）。
    #[test]
    fn a_page_smaller_than_the_panel_breaks_the_gate() {
        let target = fit_inside(Size::new(800, 1000), PANEL);
        assert_eq!(target, Size::new(800, 1000), "小于目标的页不该被放大");
        assert_eq!(GeometryGate::of(target, PANEL), GeometryGate::Broken);
    }

    /// 只有一条边够长的页也原样输出，但那条边恰好贴住面板：阅读器按 fit-inside 显示
    /// 同样不必重采样，门成立。判定看的是贴没贴住，不是缩没缩过。
    #[test]
    fn a_page_that_already_touches_one_edge_keeps_the_gate_open() {
        let target = fit_inside(Size::new(PANEL.width, 1000), PANEL);
        assert_eq!(target, Size::new(PANEL.width, 1000));
        assert!(GeometryGate::of(target, PANEL).holds());
        // 差一个像素就贴不住了，阅读器会放大 0.08%——那就不是 1:1。
        let shy = fit_inside(Size::new(PANEL.width - 1, 1000), PANEL);
        assert!(!GeometryGate::of(shy, PANEL).holds());
    }

    /// **普通漫画页两种适配方式产出同一个尺寸。**
    ///
    /// 这不是巧合，是「页比面板更瘦长、本来就受高度约束」的直接后果：两条路上宽都由
    /// 同一个 `面板高 ÷ 源高` 算出，而 fit-inside 那条路上的宽不会撞到面板宽这道夹子。
    /// 实测棋魂完全版 230 页、N和S 第43话 24 页 **100% 一致**
    /// （measurements 的《适配方式：fit-inside 与以高为准》）。
    ///
    /// 前提有两条，缺一不可，两条都在扫描里带着：
    ///
    /// 1. **受高度约束**：`源宽 × 面板高 ≤ 面板宽 × 源高`。更扁的页在 fit-inside 下改由
    ///    宽边定夺，两条路当场分岔——跨页正是这一支。
    /// 2. **不比面板矮**：`源高 ≥ 面板高`。更矮的页 fit-inside 不放大、以高为准放大到面板高。
    ///
    /// 扫的高度是面板高到四倍面板高，宽高比三档：真实漫画页那一段 1.43–1.53
    /// （measurements 同一节）、**恰好等于面板宽高比**的那一条、以及比面板更扁的两条。
    ///
    /// 等于面板宽高比那一条是**边界**，非扫不可：`fit_inside` 在那里正好顶到
    /// `.min(面板宽)` 那道夹子上，两条路差一个像素的话，只有在这里差得出来。
    /// 比面板更扁的那两条则是让**前提本身**被走到——不然那句 `continue` 是死代码，
    /// 文档里「缺一条就该分岔」这句话没有任何东西替它作证。末尾那两句断言钉住这件事：
    /// 两侧都必须真的走到过。
    ///
    /// 写下来是为了让后面的改动碰得响：一旦某次改动让「开关不该起作用的地方」起了作用，
    /// 红的就是这一条。
    #[test]
    fn the_two_fit_modes_agree_on_every_page_that_is_height_constrained() {
        let mut agreed = 0usize;
        let mut skipped = 0usize;
        for panel in [PANEL, Size::new(1072, 1448), Size::new(1448, 1072)] {
            // 面板自己的宽高比，加上真实漫画页那一段与两条更扁的。
            let panel_ratio = f64::from(panel.height) / f64::from(panel.width);
            for height in (panel.height..=panel.height * 4).step_by(7) {
                for ratio in [1.43, 1.45, 1.48, 1.50, 1.53, panel_ratio, 1.0, 0.4] {
                    let width = (f64::from(height) / ratio).round() as u32;
                    let source = Size::new(width.max(1), height);
                    if u64::from(source.width) * u64::from(panel.height)
                        > u64::from(panel.width) * u64::from(source.height)
                    {
                        // 比面板更扁：不在这条性质的前提里，两条路本来就该分岔。
                        skipped += 1;
                        continue;
                    }
                    assert_eq!(
                        FitMode::Height.target(source, panel),
                        FitMode::Inside.target(source, panel),
                        "{source} 在 {panel} 上受高度约束，两种适配方式该产出同一个尺寸"
                    );
                    agreed += 1;
                }
            }
        }
        assert!(agreed > 1000, "扫到的前提内的点只有 {agreed} 个，太少");
        assert!(
            skipped > 0,
            "那句 continue 是死代码：没有一个点落在前提之外"
        );
    }

    /// 以高为准：目标高恒等于面板高，宽按源宽高比算出，**不设上限**。
    ///
    /// 期望值取实测那两页最极端的（measurements 的《适配方式：fit-inside 与以高为准》，面板 1072×1448）：
    /// 哆啦A梦从 fit-inside 的 1072×766 变成 2027×1448，改革之獸从 1072×449 变成
    /// 3457×1448——**3.22 倍面板宽**。跨页因此从压扁状态变得可读。
    #[test]
    fn fitting_to_the_panel_height_lets_the_width_run_past_the_panel() {
        const MEASURED: Size = Size::new(1072, 1448);

        for (source, inside, height_first) in [
            (
                Size::new(6048, 4320),
                Size::new(1072, 766),
                Size::new(2027, 1448),
            ),
            (
                Size::new(7162, 3000),
                Size::new(1072, 449),
                Size::new(3457, 1448),
            ),
        ] {
            assert_eq!(FitMode::Inside.target(source, MEASURED), inside);
            assert_eq!(FitMode::Height.target(source, MEASURED), height_first);
            // 宽不夹上界：这两页一个 1.89 倍面板宽、一个 3.22 倍。
            assert!(height_first.width > MEASURED.width * 3 / 2);
        }
    }

    /// 比面板矮的页在以高为准下**被放大到面板高**，几何门随之成立——
    /// fit-inside 那条路上它按不放大原样输出，门不成立、抖动被关掉（ADR 0007）。
    ///
    /// 这是 01 号票认下的第二笔代价：比面板小的卷会被放大。
    #[test]
    fn a_page_shorter_than_the_panel_is_enlarged_to_the_panel_height() {
        let source = Size::new(800, 1000);

        let inside = FitMode::Inside.target(source, PANEL);
        let height_first = FitMode::Height.target(source, PANEL);

        assert_eq!(inside, source, "fit-inside 那条路上不放大");
        assert_eq!(GeometryGate::of(inside, PANEL), GeometryGate::Broken);
        // 1000 → 1680 放大 1.68 倍，宽跟着到 1344——比面板还宽，那一页也要横向平移。
        assert_eq!(height_first, Size::new(1344, PANEL.height));
        assert!(GeometryGate::of(height_first, PANEL).holds());
    }

    /// **以高为准这条路上几何门每一页都成立**（01 号票）。
    ///
    /// 「恒成立」说的是每一页都成立，不是卷级恒成立——门是**逐页**判的
    /// （ADR 0007 决定第 1 条），只是不成立的那一支在这条路上成了空集。
    /// 那套逐页机制不白做：换回 fit-inside 它照旧有效，本文件另外三条用例正走在那条路上。
    #[test]
    fn every_page_keeps_the_gate_open_when_the_height_leads() {
        for panel in [PANEL, Size::new(1072, 1448), Size::new(300, 400)] {
            for height in [1u32, 2, 7, 399, 400, 401, 1447, 1448, 1680, 4320, 20000] {
                for width in [1u32, 3, 100, 1264, 3000, 7162, 60000] {
                    let source = Size::new(width, height);
                    let target = FitMode::Height.target(source, panel);
                    assert_eq!(target.height, panel.height, "{source} 在 {panel} 上");
                    assert!(
                        GeometryGate::of(target, panel).holds(),
                        "{source} 在 {panel} 上算出 {target}，门竟不成立"
                    );
                }
            }
        }
    }

    /// 取整不许把哪条边压成 0：0 像素的页写不出去。
    ///
    /// 以高为准把宽按比例缩，极瘦的页（一条 1×20000 的窄条缩到面板高）算出来的宽不足半个
    /// 像素，四舍五入就是 0。fit-inside 那一侧同理。
    #[test]
    fn neither_fit_mode_ever_rounds_a_side_down_to_zero() {
        for source in [
            Size::new(1, 20000),
            Size::new(20000, 1),
            Size::new(1, 1),
            Size::new(3, 5000),
        ] {
            for mode in [FitMode::Height, FitMode::Inside] {
                let target = mode.target(source, PANEL);
                assert!(
                    target.width >= 1 && target.height >= 1,
                    "{source} → {target}"
                );
            }
        }
    }

    /// 名字都解析得回来，默认是**以高为准**（01 号票：默认换了）。
    #[test]
    fn every_fit_mode_name_resolves_and_the_default_leads_with_the_height() {
        for (name, mode) in FIT_MODES {
            assert_eq!(FitMode::resolve(name).expect("表里的名字"), *mode);
            assert_eq!(
                FitMode::resolve(&format!("  {} ", name.to_ascii_uppercase())).expect("归一"),
                *mode
            );
        }
        assert_eq!(FitMode::default(), FitMode::Height);
        // 规范名要能自己解析回来：参数哈希拿它当稳定写法（见 `crate::metadata`）。
        for (_, mode) in FIT_MODES {
            assert_eq!(FitMode::resolve(mode.name()).expect("规范名"), *mode);
        }
    }

    /// 认不出的名字要把认得的全端出来——用户是从这段文字里挑的。
    #[test]
    fn the_unknown_fit_mode_error_lists_every_name() {
        let message = FitMode::resolve("stretch").unwrap_err().to_string();
        for (name, _) in FIT_MODES {
            assert!(message.contains(name), "清单里少了 {name}：{message}");
        }
    }
}
