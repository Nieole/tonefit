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
    /// 以高为准（默认）：目标高**恒等于**面板高，宽按源宽高比算出，**不设阅读上的上限**。
    ///
    /// 比面板矮的页放大到面板高，比面板扁的页宽边溢出面板——实测最宽 3.22 倍面板宽
    /// （改革之獸 7162×3000 → 3457×1448，见 measurements 的《适配方式：fit-inside 与以高为准》）。
    /// 跨页因此从压扁状态变得可读，代价是那样的卷体积涨到约三四倍。
    ///
    /// 宽上唯一的一道线是[兜底上界](MAX_TARGET_PIXELS)，它管的是**跑得完跑不完**，
    /// 不是读得动读不动：越过那道线的页退回 fit-inside，别的什么都不改（07 号票）。
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
    ///
    /// 它是公开的，因为**预设**要把这一项写回盘上（`CONTEXT.md` 的《会话》）：
    /// 写出去的那个词必须就是 [`resolve`](Self::resolve) 认得的那个词，
    /// 而两处各写一份迟早会走散。
    pub fn name(self) -> &'static str {
        FIT_MODES
            .iter()
            .find(|(_, mode)| *mode == self)
            .map(|(name, _)| *name)
            .expect("表覆盖全部适配方式")
    }

    /// 这一页的目标尺寸。**本仓库算目标尺寸的唯一入口。**
    ///
    /// 两条路各自的算法见 [`fit_height`] 与 [`fit_inside`]，
    /// 越过[兜底上界](MAX_TARGET_PIXELS)那一支见 [`Fit::backstopped`]。
    ///
    /// 兜底落在**这里**，不落在缩放那一层：ADR 0007 决定第 7 条要目标尺寸只有一个来源，
    /// 而「这一页照哪条规矩出」正是目标尺寸的一部分。改在下游等于开了第二个来源，
    /// 报告里那个尺寸也就不再是这个函数说的那个。
    pub fn target(self, source: Size, panel: Size) -> Fit {
        let size = match self {
            FitMode::Height => fit_height(source, panel),
            FitMode::Inside => fit_inside(source, panel),
        };
        if pixels(size) <= MAX_TARGET_PIXELS {
            return Fit {
                size,
                backstopped: false,
            };
        }
        // 越界的页**退回 fit-inside**，不当失败页：那一页的像素是好的，退回来仍然读得了，
        // 变成一张白页反而丢内容（07 号票的《不要做的》）。
        //
        // 退回这一步自己不会再越界：`fit_inside` 要么原样返回一张比面板还小的页，
        // 要么等比缩到面板以内，两支的面积都不超过面板面积，而面板远在上界之内
        // （本模块的用例扫了这件事）。
        Fit {
            size: fit_inside(source, panel),
            backstopped: true,
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

/// 一页适配出来的东西：目标尺寸，加上这个尺寸是不是[兜底上界](MAX_TARGET_PIXELS)改出来的。
///
/// 两项一起出而不是分两次问：兜底改的正是尺寸本身，分开问就有两个出处，
/// 而报告要逐页指认「哪几页没按点名的那条规矩出」（07 号票，见 `crate::Report::backstopped`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    size: Size,
    backstopped: bool,
}

impl Fit {
    /// 这一页的目标尺寸：实际写出去的那个。
    pub fn size(self) -> Size {
        self.size
    }

    /// 这一页是不是被兜底上界改过——点名的那条规矩算出的尺寸越过了 [`MAX_TARGET_PIXELS`]，
    /// 这一页因此改按 fit-inside 出。
    ///
    /// fit-inside 那条路上恒为假：它的目标尺寸恒不超过面板，够不着那道线。
    pub fn backstopped(self) -> bool {
        self.backstopped
    }
}

/// 目标尺寸的**兜底上界**：算出来的目标像素数越过它，这一页退回 fit-inside（07 号票）。
///
/// 它防的是**分配不下**，不是画质。以高为准让宽随源比例算出、阅读上不设上限（01 号票定的），
/// 而目标宽 = 源宽 × 面板高 ÷ 源高：一张 20000×100 的长条解码只占 2 MB、
/// [`MAX_DECODED_BYTES`](crate::decode::MAX_DECODED_BYTES) 拦不住它，
/// 到了 1448 高的面板上却算出 289600×1448。那块缓冲分配不下会**中止整趟**——
/// 不是变成一个失败页，是整趟死掉，而管线本来有隔离机制专防「一页坏内容毁掉一整卷」
/// （`CONTEXT.md` 的《失败》）。
///
/// **取值是解码那一侧的上界折算过来的**：512 MiB 的像素缓冲除以
/// [`PEAK_BYTES_PER_TARGET_PIXEL`]，得 **32 Mi 像素**。两侧因此不会一边收得下、
/// 另一边分配不下——各定一个数才会出那种事，而后者中止的是整趟。
/// 折算完目标那一侧的峰值是 302 MiB，**严格低于**解码那一侧的 512 MiB：
/// 解得进来的页，缩出去必定也分配得下。
///
/// **它定在「肯定跑得完」那一侧，离真实素材很远。** 实测最宽的一页是面板宽的 3.22 倍
/// （改革之獸 7162×3000，见 measurements 的《适配方式：fit-inside 与以高为准》）；
/// 这道线在 1072×1448 的面板上是 21.6 倍面板宽、在 1264×1680 上是 15.8 倍。
/// 换成源页的形状：宽高比要超过 12:1 才够得着——那不是一页漫画，是一根长条
/// （实测最宽那一页是 2.4:1）。
///
/// # ADR 0003 那笔账
///
/// ADR 0007《后果》押着一条：「**任何改动目标尺寸的改动，都要同时对 ADR 0003 的硬上界
/// 负责**——两条约束的破裂条件是同一个，只算自己那一条的账，等于把另一条静默关掉。」
/// 兜底改的正是目标尺寸，这笔账因此要当场算：
///
/// - **判定仍同源。**退回之后的尺寸照旧从 [`FitMode::target`] 出来，门照旧只在
///   `GeometryGate::of(目标尺寸, 面板)` 判一次（ADR 0007 决定第 6 条：判定它的开关与
///   ADR 0003 用的是同一个）。两条约束的破裂条件仍是同一个，账没有分家。
/// - **门可能因此不成立，而那正是它该给出的答案。**退回之后这一页是一张 fit-inside 的页，
///   源两边都比面板小时按不放大原样输出、一条边都贴不住——阅读器还要再放大一次，
///   ADR 0003 的硬上界在它身上本来就撑不住。门读出 [`GeometryGate::Broken`] 不是漏洞，
///   是实情被如实报了出来：那一页照 ADR 0007 决定第 2、3 条走（抖动关掉、位深不低于基准档），
///   候选集由 `Candidates::for_gate` 现取一套，这条路一个字都没改。
/// - **兜底不会把门从「不成立」抬成「成立」。**它只在以高为准算出的尺寸越界时出手，
///   而那种页在以高为准下门本来是成立的——兜底只会让门变严，不会让它变松。
const MAX_TARGET_PIXELS: u64 = crate::decode::MAX_DECODED_BYTES / PEAK_BYTES_PER_TARGET_PIXEL;

/// 一个目标像素在整条管线上的峰值字节数，[`MAX_TARGET_PIXELS`] 拿它折算。
///
/// 数出来的是**彩色分支**那条更贵的路（`crate::resample::resize_color` 往后）：
/// 三个平面各一份缩放缓冲（3），交织给编码器一份（3），编码输出再留一份（≤ 3）——
/// **数到 9**。灰度路径只占 2：缩放缓冲一份、进缓存前那一份。
///
/// **取 16 而不是 9，方向是有讲究的**：这个系数是**除数**，取大了上界更小、更安全。
/// 取 9 会让目标那一侧的峰值正好顶在 512 MiB 上——与解码那一侧齐平，
/// 而「齐平」不是「肯定跑得完」。取 9 之上最近的 2 的幂，目标那一侧的峰值落到 302 MiB，
/// **严格低于**解码那一侧收得下的量：解得进来的页，缩出去必定也分配得下。
///
/// **它是照代码数出来的，不是量出来的**：`docs/measurements.md` 没有峰值内存这一项，
/// 而实测数字的来源只有它（`CLAUDE.md`）。记在停车场 Q22。
const PEAK_BYTES_PER_TARGET_PIXEL: u64 = 16;

/// 目标尺寸的兜底上界，报告要印它。**本仓库唯一的出处**，用例也问它。
///
/// 与裁法那两个数同一条规矩（`crate::ink_rule`）：数摆出来，读的人自己判断
/// 它对手上这批素材成不成立。
pub fn max_target_pixels() -> u64 {
    MAX_TARGET_PIXELS
}

/// 一个尺寸有多少像素。
///
/// `u64` 不是防御性的：极瘦的源页上目标宽会顶到 `u32` 的上沿（`as u32` 在那里饱和），
/// 而正是那样的页要被兜底上界拦下——用 `u32` 乘出来的面积会绕回一个小数，一路放行。
pub(crate) fn pixels(size: Size) -> u64 {
    u64::from(size.width) * u64::from(size.height)
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

/// 以高为准：目标高恒等于面板高，宽按源宽高比算出，**不设阅读上的上限**。
///
/// 与 [`fit_inside`] 差在两处，两处都是有意的：
///
/// - **比面板矮的页放大到面板高。**几何门因此在这条路上每一页都成立，
///   抖动不再被那批页关掉（ADR 0007 决定第 1 条）。
/// - **比面板扁的页宽边溢出面板。**跨页不再被长边压扁——哆啦A梦最宽一页从
///   1072×766 变成 2027×1448。溢出的部分靠阅读器横向平移，代价见 [`FitMode::Height`]。
///
/// 这个函数自己什么都不拦。兜底上界在 [`FitMode::target`] 那一层，
/// 拦下的页从此走的是 [`fit_inside`]，上面两条随之不再说它。
pub fn fit_height(source: Size, panel: Size) -> Size {
    let scale = f64::from(panel.height) / f64::from(source.height);
    // 宽不夹阅读上的上界：用户明确接受任意宽度的横向翻动（01 号票的《不要做的》）。
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
    /// **走得到它的是 fit-inside 出的页。**以高为准让每一页的高恒等于面板高，
    /// 一条边永远贴着（见 [`GeometryGate::of`]）——除非[兜底上界](MAX_TARGET_PIXELS)
    /// 把这一页退了回去：退回之后它就是一张 fit-inside 的页，门照 fit-inside 那条判
    /// （07 号票）。够得着那道线的是宽高比超过 24:1 的源页，真实素材里没有。
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
    ///   （01 号票）。宽那条边不再留边而是**溢出**面板。兜底上界退回去的那些页是这句话
    ///   唯一的例外：它们从此按 fit-inside 判（07 号票，见 [`GeometryGate::Broken`]）。
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
                        FitMode::Height.target(source, panel).size(),
                        FitMode::Inside.target(source, panel).size(),
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
            assert_eq!(FitMode::Inside.target(source, MEASURED).size(), inside);
            let fitted = FitMode::Height.target(source, MEASURED);
            assert_eq!(fitted.size(), height_first);
            // 宽不夹阅读上的上界：这两页一个 1.89 倍面板宽、一个 3.22 倍。
            assert!(height_first.width > MEASURED.width * 3 / 2);
            // **真实素材一页都不受兜底上界影响**（07 号票）：最宽那一页离那道线还有 13 倍。
            assert!(!fitted.backstopped(), "{source} 被兜底上界拦下了");
        }
    }

    /// 比面板矮的页在以高为准下**被放大到面板高**，几何门随之成立——
    /// fit-inside 那条路上它按不放大原样输出，门不成立、抖动被关掉（ADR 0007）。
    ///
    /// 这是 01 号票认下的第二笔代价：比面板小的卷会被放大。
    #[test]
    fn a_page_shorter_than_the_panel_is_enlarged_to_the_panel_height() {
        let source = Size::new(800, 1000);

        let inside = FitMode::Inside.target(source, PANEL).size();
        let height_first = FitMode::Height.target(source, PANEL).size();

        assert_eq!(inside, source, "fit-inside 那条路上不放大");
        assert_eq!(GeometryGate::of(inside, PANEL), GeometryGate::Broken);
        // 1000 → 1680 放大 1.68 倍，宽跟着到 1344——比面板还宽，那一页也要横向平移。
        assert_eq!(height_first, Size::new(1344, PANEL.height));
        assert!(GeometryGate::of(height_first, PANEL).holds());
    }

    /// **以高为准这条路上几何门每一页都成立——被兜底上界退回去的那些页除外**
    /// （01 号票、07 号票）。
    ///
    /// 「恒成立」说的是每一页都成立，不是卷级恒成立——门是**逐页**判的
    /// （ADR 0007 决定第 1 条），只是不成立的那一支在这条路上几乎成了空集。
    /// 那套逐页机制不白做：换回 fit-inside 它照旧有效，本文件另外三条用例正走在那条路上，
    /// 而兜底退回去的页从此也走在那条路上——末尾那句断言钉住这一支真的被扫到过，
    /// 不然「唯一的例外」这句话没有东西替它作证。
    #[test]
    fn every_page_keeps_the_gate_open_when_the_height_leads_unless_the_backstop_stepped_in() {
        let mut backstopped = 0usize;
        for panel in [PANEL, Size::new(1072, 1448), Size::new(300, 400)] {
            for height in [1u32, 2, 7, 399, 400, 401, 1447, 1448, 1680, 4320, 20000] {
                for width in [1u32, 3, 100, 1264, 3000, 7162, 60000] {
                    let source = Size::new(width, height);
                    let fitted = FitMode::Height.target(source, panel);
                    if fitted.backstopped() {
                        // 退回去的页是一张 fit-inside 的页，一个字都不多改。
                        assert_eq!(
                            fitted.size(),
                            fit_inside(source, panel),
                            "{source} 在 {panel} 上"
                        );
                        backstopped += 1;
                        continue;
                    }
                    let target = fitted.size();
                    assert_eq!(target.height, panel.height, "{source} 在 {panel} 上");
                    assert!(
                        GeometryGate::of(target, panel).holds(),
                        "{source} 在 {panel} 上算出 {target}，门竟不成立"
                    );
                }
            }
        }
        assert!(
            backstopped > 0,
            "扫描里没有一页越过兜底上界，那一支是死代码"
        );
    }

    /// **兜底上界：越界的页退回 fit-inside，不中止整趟，也不当失败页**（07 号票）。
    ///
    /// 期望值取票面那一张：20000×100 的长条在 1072×1448 的面板上算出 289600×1448
    /// ——289600 × 1448 是 4.19 亿像素，彩色分支上要三四 GB，`Image::new` 那一步
    /// 分配不下、整趟当场死掉。解码那一侧拦不住它：这张图只有 2 MB。
    #[test]
    fn a_page_whose_target_would_not_fit_in_memory_falls_back_to_fit_inside() {
        const POKE6: Size = Size::new(1072, 1448);
        let strip = Size::new(20000, 100);

        // 不设兜底的话算出来的是这个，四亿像素。
        assert_eq!(fit_height(strip, POKE6), Size::new(289600, 1448));
        assert!(pixels(fit_height(strip, POKE6)) > MAX_TARGET_PIXELS * 12);

        let fitted = FitMode::Height.target(strip, POKE6);

        assert!(fitted.backstopped());
        assert_eq!(fitted.size(), fit_inside(strip, POKE6));
        assert_eq!(fitted.size(), Size::new(1072, 5));
        // 退回之后自己不越界——这是兜底成立的全部意思。
        assert!(pixels(fitted.size()) <= MAX_TARGET_PIXELS);
    }

    /// **兜底之后的尺寸恒不越界，且 fit-inside 那条路一次都够不着这道线。**
    ///
    /// 前半句是兜底的意义所在：退回去的那一步要是自己也分配不下，这道上界就什么都没买到。
    /// 后半句是它为什么单向：fit-inside 的目标尺寸恒不超过面板（源比面板小就原样返回，
    /// 否则等比缩进面板），而面板面积远在上界之内——扫描里同时钉住这两句。
    #[test]
    fn the_backstop_never_fires_on_fit_inside_and_never_hands_back_an_oversized_target() {
        for panel in [PANEL, Size::new(1072, 1448), Size::new(300, 400)] {
            assert!(pixels(panel) * 15 < MAX_TARGET_PIXELS, "{panel} 离上界太近");
            for height in [1u32, 2, 7, 400, 1448, 1680, 4320, 20000, u16::MAX as u32] {
                for width in [1u32, 3, 100, 1264, 3000, 7162, 60000, u16::MAX as u32] {
                    let source = Size::new(width, height);
                    for mode in [FitMode::Height, FitMode::Inside] {
                        let fitted = mode.target(source, panel);
                        assert!(
                            pixels(fitted.size()) <= MAX_TARGET_PIXELS,
                            "{mode:?}：{source} 在 {panel} 上出了 {}，仍在上界之外",
                            fitted.size()
                        );
                    }
                    assert!(
                        !FitMode::Inside.target(source, panel).backstopped(),
                        "{source} 在 {panel} 上让 fit-inside 触发了兜底"
                    );
                }
            }
        }
    }

    /// 上界那个数由**解码那一侧的上界**折算而来，不是另拍一个（07 号票）。
    ///
    /// 两侧各定一个数，就会出现「解码收得下、目标分配不下」，而后者中止的是整趟。
    /// 这一条钉住折算关系本身：改了任一头，另一头跟着走。
    #[test]
    fn the_backstop_is_the_decode_limit_divided_by_what_a_target_pixel_costs() {
        assert_eq!(
            max_target_pixels(),
            crate::decode::MAX_DECODED_BYTES / PEAK_BYTES_PER_TARGET_PIXEL
        );
        assert_eq!(max_target_pixels(), 32 * 1024 * 1024);
        // 折算完目标那一侧的峰值严格低于解码那一侧收得下的量——「肯定跑得完」这句话
        // 在字节上的形式就是这个不等号，取等都不行。
        assert!(
            max_target_pixels() * PEAK_BYTES_PER_TARGET_PIXEL <= crate::decode::MAX_DECODED_BYTES
        );
        // 实测最宽那一页（改革之獸，3457×1448）离这道线还有 6 倍：上界不贴着真实素材的上沿卡。
        assert!(pixels(Size::new(3457, 1448)) * 6 < max_target_pixels());
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
                let target = mode.target(source, PANEL).size();
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
