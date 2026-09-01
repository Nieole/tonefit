//! 跨页拆分：装订沟定切点，**找不到沟就不切**。
//!
//! 跨页在以高为准下读得了，但读法是横向平移：实测三卷双页片源的屏占比中位是 1.88–1.91。
//! 拆成两半之后落到 0.88–0.92，与单页卷没有分别，基本不必横向翻动；而**缩放系数完全相同**——
//! 拆分不是拿分辨率换来的，两条路上每一半的像素密度一模一样（见 measurements 的《跨页拆分》）。
//!
//! **判定是两级的，两级都不看画面语义**（spec 的《拆分是两级判定》）：
//!
//! 1. **跨页候选**按宽高比与面板比挑（[`SplitThreshold`]）。混排卷是常态——一卷里有些页
//!    已经拆好、有些还是连页，这一关把已经拆好的那些原样放过去。
//!    （「跨页候选」写全，不缩成「候选」：`CONTEXT.md`《量化》里那个候选是
//!    (位深, 抖动模式) 组合，两者不许同名。）
//! 2. **装订沟**在跨页候选里定切点（[`Gutter`]）。它是跨页正中附近贯穿全高的一段空白列，
//!    量法与裁边同一套（行列墨量占比，见 `crate::crop`）——装订沟检测与裁边是同一类扫描，
//!    没有第二套墨量判据。
//!
//! **找不到沟即判为连续跨页，不切。** 那是为视觉效果画满两页的一幅整画，切开就毁了。
//! 这一条是硬约束，不是启发式的兜底：「切不切」由装订沟决定，不只是「切在哪」。
//!
//! 不切的页**照这一趟的适配方式出**——默认以高为准，宽溢出面板、靠阅读器横向平移看。
//! 那是适配方式那个开关说了算的事，不是拆分说了算：`--fit inside` 下它仍然被压扁，
//! 而「拆分开着却配 fit-inside」是一条互锁（[`Interlock::SpreadsStayFlattened`](crate::Interlock)）：
//! 报告抬头提一次，不拦。
//!
//! 次序是**裁边 → 判跨页 → 拆分 → 每半再裁 → 适配**（spec 的《Solution》）。先裁再判，
//! 因为白边过宽的单页在裁之前宽高比会像跨页；每半再裁，因为装订沟那一侧的白边
//! 是切开之后才露出来的。三段窗口叠成源页上的一块，报告只印那一个
//! （见 [`Crop::then`](crate::Crop::then)）。
//!
//! **拆分不碰几何门，也没有把 ADR 0003 的硬上界静默关掉。**ADR 0007《后果》要求
//! 「任何改动目标尺寸的改动，都要同时对 ADR 0003 的硬上界负责——两条约束的破裂条件
//! 是同一个」。拆分改的与裁边改的是同一样东西：**送进适配的那个尺寸**，不是目标尺寸的来源。
//! 目标尺寸仍只由 `FitMode::target` 算出，门仍只在 `GeometryGate::of(目标尺寸, 面板)`
//! 判一次，而且**逐半各判各的**——切开之后两半各是一张页，各有各的几何
//! （ADR 0007 决定第 1 条：门逐页判）。两条约束的破裂条件因此仍是同一个，账没有分家。

use anyhow::{Result, anyhow};

use crate::color::ColorImage;
use crate::crop::{self, Crop};
use crate::decode::Salvage;
use crate::geometry::Size;
use crate::gray::{self, GrayImage};

/// 跨页候选阈值的默认值：页宽高比要有面板宽高比的这么多倍，才算跨页候选。
///
/// **未标定占位值**，但两侧离得远。实测（measurements 的《适配方式：fit-inside 与以高为准》）：
/// 面板宽高比 0.74，普通漫画页 0.65–0.70（比值 0.88–0.94），
/// 而真跨页是哆啦A梦 1.40（比值 1.89）与改革之獸 2.39（比值 3.22）。
/// 1.5 落在 0.94 与 1.89 之间，两侧各有半个量级的余地。
///
/// 它是**比值**而不是一个绝对的宽高比：面板换一块，这条线跟着走。
const DEFAULT_THRESHOLD: f64 = 1.5;

/// 装订沟只在页正中的这一段里找：中线左右各这么多页宽。
///
/// 窗口存在的理由不是省时间，是**别把画面当成沟**：一幅跨页画里离中线远的地方
/// 本来就可能有整列留白。
///
/// **实测沟中心落在 0.401–0.538 之间，但那个区间证不了这个窗口够宽**——窗外的沟根本
/// 检测不到，区间是窗口自己圈出来的。够不够宽是另外量的：把窗口放到 ±0.15 与 ±0.25 各跑一遍，
/// 切开的页**反而少了 31 页**（measurements 的《检测窗口：放宽不会捞回更多页，只会丢页》）。
///
/// 机理是它与 [`MAX_GUTTER`] 咬在一起：窗口一宽，最长的那段空白列跟着变长，
/// 随即越过沟宽那道线、被判成连续跨页。两个数因此**只能一起动**，各调各的必然打架。
const GUTTER_WINDOW: f64 = 0.10;

/// 装订沟最宽占页宽的这么多。再宽的空白带不是装订沟。
///
/// 这一关是跨页候选那一关之后的**第二道**：那一关把单页挡在外面，而这一关拦的是
/// **候选页里的连续跨页**——一幅画中间恰好有一片天空或留白，宽得不像一条装订沟。
///
/// **两侧都有实测撑着**（见 measurements 的《跨页拆分》）：
///
/// - **下侧**：三卷双页片源上一共 665 条真装订沟，宽 0.17%–12.47%，
///   最宽的那一条离这道线还有两个半百分点。
/// - **上侧**：它与 [`GUTTER_WINDOW`] 咬着——窗口总宽 2×0.10 = 0.20，
///   一段横贯整个窗口的空白必然量出 20%，越过这道线，被判为连续跨页。
///   实测误报的「沟」宽 21–24% 页宽，正落在窗口之外那一侧。
///
/// 12.47% 与 20% 之间那一段没有实测数据，这道线就画在那个空档里。
/// 两个数一起动才改得动这个行为。
const MAX_GUTTER: f64 = 0.15;

/// 阅读方向：拆开后两半的先后（spec 的 story 3、story 4）。
///
/// 它是**口味层**的事（`page-geometry/05` 的《跨批次依赖》）：用户一批一批处理，
/// 一批内同质，因此不留逐卷的口子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadingOrder {
    /// 右开（默认）：拆开后**右半在先**。日式漫画是这个方向。
    #[default]
    RightToLeft,
    /// 左开：左半在先。国漫与西方漫画走这一条。
    LeftToRight,
}

/// 阅读方向的名字表。第一个指向某个方向的名字是它的规范名（见 [`ReadingOrder::name`]）。
const READING_ORDERS: &[(&str, ReadingOrder)] = &[
    ("rtl", ReadingOrder::RightToLeft),
    ("right-to-left", ReadingOrder::RightToLeft),
    ("ltr", ReadingOrder::LeftToRight),
    ("left-to-right", ReadingOrder::LeftToRight),
];

impl ReadingOrder {
    /// 按名字解析。大小写不论，两边的空白不算。
    pub fn resolve(name: &str) -> Result<Self> {
        let key = name.trim().to_ascii_lowercase();
        READING_ORDERS
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, order)| *order)
            .ok_or_else(|| unknown_reading_order_error(name))
    }

    /// 这个阅读方向的规范名，取表里第一个指向它的那个。
    ///
    /// 参数哈希拿它当稳定写法（见 `crate::metadata`），与 `FitMode::name` 同一个理由：
    /// 那串字节要落进输出文件、几个月后还要比对。
    ///
    /// 它是公开的，同样与 `FitMode::name` 同一条理由：**预设**要把这一项写回盘上。
    pub fn name(self) -> &'static str {
        READING_ORDERS
            .iter()
            .find(|(_, order)| *order == self)
            .map(|(name, _)| *name)
            .expect("表覆盖全部阅读方向")
    }

    /// 这个方向下，先读的是哪一侧。
    fn first(self) -> Side {
        match self {
            ReadingOrder::RightToLeft => Side::Right,
            ReadingOrder::LeftToRight => Side::Left,
        }
    }
}

impl std::fmt::Display for ReadingOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReadingOrder::RightToLeft => "右开（右半在先）",
            ReadingOrder::LeftToRight => "左开（左半在先）",
        })
    }
}

fn unknown_reading_order_error(name: &str) -> anyhow::Error {
    let names: Vec<_> = READING_ORDERS.iter().map(|(name, _)| *name).collect();
    anyhow!(
        "未知阅读方向「{name}」。认得的是：{}。\
         rtl 是日式漫画的右开——拆开后右半在先；ltr 是左开，国漫与西方漫画走这一条。",
        names.join(" ")
    )
}

/// **跨页候选**的阈值：页宽高比要有面板宽高比的这么多倍，才算跨页候选。
///
/// 「跨页候选」是拆分两级判定的第一级，与 `CONTEXT.md`《量化》里那个**候选**
/// （一个 (位深, 抖动模式) 组合）**不是一回事**——本模块因此一律写全「跨页候选」，
/// 不用光秃秃的「候选」。
///
/// 它是**可调的**（spec 的 story 7）：混排卷里已经拆好的单页与还没拆的连页要各自落对，
/// 而两类页在不同片源上离得远近不同。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitThreshold(f64);

impl Default for SplitThreshold {
    fn default() -> Self {
        Self(DEFAULT_THRESHOLD)
    }
}

impl SplitThreshold {
    /// 按数值解析。非正数与解不出来的都是错——阈值是个比值，零与负数没有意义。
    pub fn parse(text: &str) -> Result<Self> {
        let value: f64 = text
            .trim()
            .parse()
            .map_err(|_| anyhow!("拆分阈值「{text}」不是一个数"))?;
        if !(value.is_finite() && value > 0.0) {
            return Err(anyhow!("拆分阈值要是一个正数，收到的是 {value}"));
        }
        Ok(Self(value))
    }

    /// 这个阈值的数值。参数哈希与报告都印它。
    pub fn value(self) -> f64 {
        self.0
    }

    /// 这一页够得上**跨页候选**吗——本仓库判跨页候选的唯一入口。
    ///
    /// 问的是裁完之后的尺寸（次序见本模块的模块文档）。
    ///
    /// 它**不公开**：spec 的《Testing Decisions》定死「Seam 只用 `run(Request) -> Report`
    /// 这一个……不新开公开 seam」。冒烟要数「有几页过了这一关」，读的是报告里那一项
    /// （`PageReport::spread_candidate`）——那是**观察**，不是拿同一个函数再算一遍。
    pub(crate) fn admits(self, page: Size, panel: Size) -> bool {
        aspect(page) >= self.0 * aspect(panel)
    }
}

impl std::fmt::Display for SplitThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// 一个尺寸的宽高比。
fn aspect(size: Size) -> f64 {
    f64::from(size.width) / f64::from(size.height)
}

/// 这一趟的拆法，报告抬头原样印出。
///
/// 三项绑成一个类型，与裁边那两个数收成 [`InkRule`](crate::InkRule) 同一条规矩：
/// 数摆出来，来源跟着摆出来，读的人自己判断它对手上这批素材成不成立。
/// 绑在一起还有第二个理由——三项总是一同传下去，三个同型的裸参数换了位置编译器一句话都不会说。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitRule {
    /// 拆不拆（`--no-split` 关掉它）。**默认拆。**
    pub on: bool,
    /// 跨页候选的阈值（`--split-threshold`）。
    pub threshold: SplitThreshold,
    /// 阅读方向（`--reading-order`）。
    pub order: ReadingOrder,
}

impl Default for SplitRule {
    fn default() -> Self {
        Self {
            on: true,
            threshold: SplitThreshold::default(),
            order: ReadingOrder::default(),
        }
    }
}

impl std::fmt::Display for SplitRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.on {
            return f.write_str("关（跨页整页出，照这一趟的适配方式走）");
        }
        write!(
            f,
            "跨页候选阈值 {} × 面板宽高比 · 装订沟定切点 · {}",
            self.threshold, self.order
        )
    }
}

/// 拆开之后的一半在源页的哪一侧。
///
/// 它与**先后**不是一回事：先读哪一侧由[阅读方向](ReadingOrder)定，
/// 而这一项说的是这张图原来长在页的哪边。报告要的是后者——反过阅读方向之后，
/// 同一张图仍然是那一侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 装订沟右边那一半。
    Right,
    /// 装订沟左边那一半。
    Left,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Side::Right => "右半",
            Side::Left => "左半",
        })
    }
}

/// 装订沟：跨页正中附近贯穿全高的一段空白列。
///
/// 两个下标都是**闭区间**的端点，坐标在判它的那张图上（也就是裁完之后的源页）。
/// 页宽跟着一同存下来：沟的位置与宽度单独摆出来没有意义，measurements 记的是**两个比例**
/// ——沟中心落在页宽的哪儿（实测 0.401–0.538），沟占页宽多少（实测 0.17%–12.47%）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gutter {
    /// 这条沟长在多宽的一张页上。
    page: u32,
    left: u32,
    right: u32,
}

impl Gutter {
    /// 沟有多宽，单位是列。
    pub fn width(self) -> u32 {
        self.right - self.left + 1
    }

    /// 沟占页宽的比例。误报的「沟」在这个数上与真装订沟分得开（见 [`MAX_GUTTER`]）。
    pub fn share(self) -> f64 {
        f64::from(self.width()) / f64::from(self.page)
    }

    /// 沟中心落在页宽的哪个位置：0 是最左，1 是最右，0.5 是正中。
    ///
    /// 实测 0.401–0.538（measurements 的《跨页拆分》）——**这个数就是「不切正中」那句话
    /// 量得出来的形式**。
    pub fn center(self) -> f64 {
        f64::from(self.cut()) / f64::from(self.page)
    }

    /// 切点：沟的中心那一列。**不切正中**——按正中盲切最偏的一页会切进画面三百多像素。
    pub fn cut(self) -> u32 {
        self.left + self.width() / 2
    }
}

/// 一张输出页是**那一刀的产物**：切在哪条装订沟上，这一张是哪一侧。
///
/// 两项绑成一个类型而不是各占一格：一张页要么整页出（两项都没有），要么是切出来的一半
/// （两项都有），中间没有第三种。分成两个可空的字段，读的那一端就得自己维护这条不变量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    gutter: Gutter,
    side: Side,
}

impl Cut {
    /// 定这一刀的那条装订沟。
    pub fn gutter(self) -> Gutter {
        self.gutter
    }

    /// 这一张在源页的哪一侧。
    pub fn side(self) -> Side {
        self.side
    }
}

impl std::fmt::Display for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "跨页{}", self.side)
    }
}

/// 一张源页切成的那几块。
///
/// 没切开时 [`halves`](Self::halves) 是 `None`，而**不是**一个「整页」窗口：
/// 一对一那条老路一个像素都不该多搬（见 `crate::Compute`）。
///
/// 「不是候选」与「是候选但没有装订沟」两种没切开分得开，靠的是
/// [`candidate`](Self::candidate)——后者就是**连续跨页**，而冒烟按这两项数误报率。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Split {
    candidate: bool,
    halves: Option<[Half; 2]>,
}

/// 切出来的一半：它在（裁完的）源页里的窗口，以及它是那一刀的哪一侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Half {
    window: Crop,
    cut: Cut,
}

impl Half {
    /// 这一半在源页里的那一块。
    pub fn window(self) -> Crop {
        self.window
    }

    /// 这一半是那一刀的产物：切在哪条沟上、这一半是哪一侧。
    pub fn cut(self) -> Cut {
        self.cut
    }
}

impl Split {
    /// 这一页没切开。`candidate` 说的是它**够不够得上跨页候选**——两级判定里第一级过了、
    /// 第二级没过的那些页就是连续跨页。
    fn whole(candidate: bool) -> Self {
        Self {
            candidate,
            halves: None,
        }
    }

    /// 这一页够得上**跨页候选**吗（拆分两级判定的第一级）。
    ///
    /// 与 [`halves`](Self::halves) 一起才说得全那两级：切得开的必然是候选；
    /// 候选而没切开的，就是**连续跨页**——够得上宽高比那一关，却找不到装订沟。
    /// 两者都为假的页根本没进过拆分。
    ///
    /// 这一项要一路走到报告里（`PageReport::spread_candidate`）：真实素材冒烟按它数
    /// 误报率，而**单页卷上它为真的页就是候选那一关漏下来的**（04 号票的验收）。
    pub fn candidate(self) -> bool {
        self.candidate
    }

    /// 切出来的两半，**按阅读顺序**。没切开就是 `None`。
    pub fn halves(self) -> Option<[Half; 2]> {
        self.halves
    }

    /// 一张**灰度**图切成几块。`panel` 是面板分辨率，候选那一关拿它比宽高比。
    pub fn of_gray(
        image: &GrayImage,
        panel: Size,
        rule: SplitRule,
        salvage: Option<Salvage>,
    ) -> Self {
        Self::of(image.size(), panel, rule, salvage, || {
            image.pixels().iter().map(|&value| crop::is_ink(value))
        })
    }

    /// 同上，但量的是彩色分支上那张图。墨量仍按**灰度**量，与 [`Crop::of_color`] 同一条规矩：
    /// 一页切在哪不该因为面板认不认得颜色而变。
    pub fn of_color(
        image: &ColorImage,
        panel: Size,
        rule: SplitRule,
        salvage: Option<Salvage>,
    ) -> Self {
        let [red, green, blue] = image.planes();
        Self::of(image.size(), panel, rule, salvage, || {
            red.pixels()
                .iter()
                .zip(green.pixels())
                .zip(blue.pixels())
                .map(|((&red, &green), &blue)| crop::is_ink(gray::value(red, green, blue)))
        })
    }

    /// 切不切由 `rule` 与 `salvage` 定，切在哪由 `ink` 吐出的那串墨点判定定。
    ///
    /// 这两项**收在这里而不摊回两个调用处**，与 [`Crop::of`] 同一个理由：
    /// 「谁不切」只有一个出处。
    ///
    /// **部分救回页不切**：它缺的那一段留成纸白（`CONTEXT.md` 的《失败》），
    /// 而纸白按墨量就是一条贯穿全高的空白——一张缺了右半边的页会被当成装订沟正好在中间。
    /// 与裁边那一条是同一句话的两半：缺的那一段不是白边，也不是装订沟。
    ///
    /// `ink` 按**行优先**给出每个像素是不是墨点，懒求值：拆分关着、或这一页不是候选时
    /// 一个像素都不看。
    fn of<I: Iterator<Item = bool>>(
        size: Size,
        panel: Size,
        rule: SplitRule,
        salvage: Option<Salvage>,
        ink: impl FnOnce() -> I,
    ) -> Self {
        // 第一级：够不够得上跨页候选。关着的那一趟与部分救回页连这一关都不问——
        // 前者根本不拆，后者缺的那一段留成纸白，按墨量看就是「装订沟正好在中间」。
        let candidate = rule.on && salvage.is_none() && rule.threshold.admits(size, panel);
        if !candidate {
            return Self::whole(false);
        }
        let width = size.width as usize;
        let mut columns = vec![0u32; width];
        for (index, is_ink) in ink().enumerate() {
            if is_ink {
                columns[index % width] += 1;
            }
        }
        // 第二级：找不到沟 = 连续跨页 = 不切（本模块的模块文档）。
        let Some(gutter) = gutter(&columns, size) else {
            return Self::whole(true);
        };
        let at = gutter.cut();
        let half = |window, side| Half {
            window,
            cut: Cut { gutter, side },
        };
        let left = half(
            Crop::new(size, (0, 0), Size::new(at, size.height)),
            Side::Left,
        );
        let right = half(
            Crop::new(size, (at, 0), Size::new(size.width - at, size.height)),
            Side::Right,
        );
        // 沟自己那几列跟着两半各走一半，不单独扔掉：它是纸白，**每半再裁**那一步会把它收走
        // （见本模块的模块文档）。少一条特例，也少一处「切点到底算在哪一半」的歧义。
        let halves = match rule.order.first() {
            Side::Right => [right, left],
            Side::Left => [left, right],
        };
        Self {
            candidate: true,
            halves: Some(halves),
        }
    }
}

/// 一串列墨量里的装订沟，`size` 是这张图的尺寸。找不到就是 `None`。
///
/// 只在页正中那一段里找（[`GUTTER_WINDOW`]），取其中**最长**的一段空白列；
/// 那一段宽过 [`MAX_GUTTER`] 就不是装订沟，答 `None`。
///
/// 一列算不算空白，用的是裁边那条线（[`crop::content_line`]）：一列的墨点占页高的比例
/// 够不着它就是空白。两处同一套判据，不是巧合——装订沟检测与裁边是同一类扫描。
///
/// 窗口的两端都**夹在页里面**：头一列至少是第 1 列，末一列至多是倒数第 1 列。
/// 这一夹保证切点两侧各留得下至少一列——沟落在第 0 列上会切出一张零宽的图。
/// 真实素材上够不着这一手（够得上候选的页宽以千计），只有一张两列宽的退化页碰得到它，
/// 而那种页解得出来（见本模块的用例）。
fn gutter(columns: &[u32], size: Size) -> Option<Gutter> {
    let line = crop::content_line(size.height);
    let blank = |x: u32| f64::from(columns[x as usize]) < line;

    let width = f64::from(size.width);
    let reach = width * GUTTER_WINDOW;
    let from = (width / 2.0 - reach).max(0.0) as u32;
    let to = ((width / 2.0 + reach) as u32).min(size.width - 1);

    let mut best: Option<Gutter> = None;
    let mut run: Option<u32> = None;
    let close = |best: &mut Option<Gutter>, run: Option<u32>, end: u32| {
        if let Some(start) = run {
            let found = Gutter {
                page: size.width,
                left: start,
                right: end,
            };
            if best.is_none_or(|widest: Gutter| found.width() > widest.width()) {
                *best = Some(found);
            }
        }
    };
    for x in from..=to {
        if blank(x) {
            run = run.or(Some(x));
        } else if let Some(start) = run.take() {
            close(&mut best, Some(start), x - 1);
        }
    }
    close(&mut best, run, to);

    let gutter = best?;
    // 太宽的空白带不是装订沟：那是一幅连续跨页画中间的一片留白（见 [`MAX_GUTTER`]）。
    (gutter.share() <= MAX_GUTTER).then_some(gutter)
}

#[cfg(test)]
mod tests {
    //! 这里量的是**判定与切点本身**：给一页像素，拆不拆、切在哪、两半谁在先。
    //!
    //! 同名的那几条在 `tests/pipeline.rs` 上量的是另一件事——拆分对**产物**做了什么
    //! （成员名、页数、每半的尺寸与裁切）。两层不是重复：这一层换一个候选阈值就红，
    //! 那一层换一个「先适配后拆分」的次序才红。

    use super::*;

    /// 实测那块面板（boox-poke6）。候选那一关拿它比宽高比。
    const PANEL: Size = Size::new(1072, 1448);

    /// 一页跨页：哆啦A梦 8K 那一卷的实测尺寸（measurements 的《适配方式：fit-inside 与以高为准》）。
    const SPREAD: Size = Size::new(6048, 4320);

    /// 造一页跨页：满页是墨，只在 `center` 那个位置留一条 `width` 列宽的空白。
    ///
    /// 满页是墨是有意的：这一层问的是「沟在哪」，页上画着什么无关，
    /// 而满墨让「白边」这个变量整个不在场。
    fn with_gutter(size: Size, center: f64, width: u32) -> GrayImage {
        let mut pixels = vec![0u8; (size.width * size.height) as usize];
        let left = (f64::from(size.width) * center) as u32 - width / 2;
        for y in 0..size.height {
            for x in left..left + width {
                pixels[(y * size.width + x) as usize] = 255;
            }
        }
        GrayImage::new(size, pixels)
    }

    /// 满页是墨、一条空白列都没有的跨页：**连续跨页，不切**。
    fn continuous(size: Size) -> GrayImage {
        GrayImage::new(size, vec![0u8; (size.width * size.height) as usize])
    }

    /// **切点跟着沟走，不落在正中。**
    ///
    /// 沟中心取 0.441，落在实测区间（0.401–0.538，见 measurements 的《跨页拆分》）里
    /// 偏离正中较远的那一侧：按正中盲切会切进画面 (0.5 − 0.441) × 6048 = 356 像素，
    /// 而那是切进画面里去。夹具不取实测最偏的 0.401——那一条离检测窗口的边只剩 0.001，
    /// 合成夹具落在那儿量的就成了窗口截断，不是切点。
    #[test]
    fn the_cut_follows_the_gutter_and_not_the_middle_of_the_page() {
        let page = with_gutter(SPREAD, 0.441, 40);

        let split = Split::of_gray(&page, PANEL, SplitRule::default(), None);

        let halves = split.halves().expect("有沟就该切开");
        let gutter = halves[0].cut().gutter();
        assert_eq!(gutter.width(), 40);
        // 两半说的是同一条沟：切点只有一个。
        assert_eq!(halves[1].cut().gutter(), gutter);
        // 两半在沟上会合，宽度加起来是整页：切点只有一个，一列都没丢。
        let widths: Vec<u32> = halves
            .iter()
            .map(|half| half.window().after().width)
            .collect();
        assert_eq!(widths.iter().sum::<u32>(), SPREAD.width);
        // 切点落在沟里，而**不在正中**：正中那一刀会切进画面三百多像素。
        let cut = gutter.cut();
        assert!(gutter.left <= cut && cut <= gutter.right, "切点不在沟里");
        assert!(
            SPREAD.width / 2 - cut > 300,
            "切点离正中只有 {} 像素，夹具没把「沟不在正中」造出来",
            SPREAD.width / 2 - cut
        );
    }

    /// **找不到装订沟就是连续跨页，不切。**一幅画横跨两页，切开就毁了。
    #[test]
    fn a_spread_without_a_gutter_is_continuous_and_is_not_cut() {
        let page = continuous(SPREAD);

        let split = Split::of_gray(&page, PANEL, SplitRule::default(), None);

        assert!(split.halves().is_none(), "连续跨页被切开了");
        // 夹具自证：这一页够得上候选，挡下它的是「没有沟」这一关，不是宽高比那一关。
        assert!(SplitRule::default().threshold.admits(SPREAD, PANEL));
    }

    /// **候选那一关按宽高比挑**：普通漫画页连沟都不找。
    ///
    /// 混排卷是常态——一卷里有些页已经拆好、有些还是连页（spec 的 story 7）。
    /// 已经拆好的那些正中若恰好有一条留白，没有这一关就会被再切一刀。
    #[test]
    fn a_single_page_never_reaches_the_gutter_check() {
        // B 类中位尺寸，正中一条 40 列宽的留白——沟那一关会当场认下它。
        let single = with_gutter(Size::new(1441, 2048), 0.5, 40);

        let split = Split::of_gray(&single, PANEL, SplitRule::default(), None);

        assert!(split.halves().is_none(), "单页被当成跨页切开了");
        assert!(!SplitRule::default().threshold.admits(single.size(), PANEL));
        // 夹具自证：那条留白真的在，挡下它的只能是候选那一关。
        let widened = Size::new(1441 * 3, 2048);
        assert!(
            Split::of_gray(
                &with_gutter(widened, 0.5, 40),
                PANEL,
                SplitRule::default(),
                None
            )
            .halves()
            .is_some(),
            "同一条留白在够得上候选的页上该被认成沟"
        );
    }

    /// **阅读方向定两半的先后，不定它们是哪一侧。**
    ///
    /// 反过来之后，同一张图仍然是原来那一侧——变的只有谁排在前面。
    #[test]
    fn the_reading_order_decides_which_half_comes_first() {
        let page = with_gutter(SPREAD, 0.5, 40);
        let split = |order| {
            Split::of_gray(
                &page,
                PANEL,
                SplitRule {
                    order,
                    ..SplitRule::default()
                },
                None,
            )
            .halves()
            .expect("有沟就该切开")
        };

        let right_first = split(ReadingOrder::RightToLeft);
        let left_first = split(ReadingOrder::LeftToRight);

        assert_eq!(right_first[0].cut().side(), Side::Right);
        assert_eq!(right_first[1].cut().side(), Side::Left);
        assert_eq!(left_first[0].cut().side(), Side::Left);
        assert_eq!(left_first[1].cut().side(), Side::Right);
        // 两半的窗口一个像素都没变，变的只有次序。
        assert_eq!(right_first[0].window(), left_first[1].window());
        assert_eq!(right_first[1].window(), left_first[0].window());
    }

    /// **太宽的空白带不是装订沟**（见 [`MAX_GUTTER`]）。
    ///
    /// 实测误报页的「沟」宽 21–24% 页宽，真装订沟窄得多。这一关拦的是候选里的连续跨页：
    /// 一幅画中间恰好有一片天空。
    #[test]
    fn a_band_too_wide_to_be_a_gutter_leaves_the_page_whole() {
        let narrow = (f64::from(SPREAD.width) * MAX_GUTTER) as u32;

        // 恰好在线上的那一条仍是沟。
        assert!(
            Split::of_gray(
                &with_gutter(SPREAD, 0.5, narrow),
                PANEL,
                SplitRule::default(),
                None
            )
            .halves()
            .is_some()
        );
        // 宽出去一截就不是了：那是一幅连续跨页画中间的留白。
        assert!(
            Split::of_gray(
                &with_gutter(SPREAD, 0.5, narrow * 2),
                PANEL,
                SplitRule::default(),
                None
            )
            .halves()
            .is_none()
        );
    }

    /// **离中线太远的空白列不是装订沟**（见 [`GUTTER_WINDOW`]）。
    #[test]
    fn a_blank_column_far_from_the_middle_is_not_a_gutter() {
        let page = with_gutter(SPREAD, 0.25, 40);

        assert!(
            Split::of_gray(&page, PANEL, SplitRule::default(), None)
                .halves()
                .is_none()
        );
    }

    /// 开关关着就不切，**部分救回页**同样不切。
    ///
    /// 后一条与裁边那一条是同一句话的两半：救回页缺的那一段留成纸白，
    /// 而一张缺了右半边的页按墨量看就是「装订沟正好在中间」。
    #[test]
    fn the_switch_and_a_salvaged_page_both_leave_the_page_whole() {
        let page = with_gutter(SPREAD, 0.5, 40);
        let off = SplitRule {
            on: false,
            ..SplitRule::default()
        };

        assert!(
            Split::of_gray(&page, PANEL, off, None).halves().is_none(),
            "开关关着"
        );
        assert!(
            Split::of_gray(
                &page,
                PANEL,
                SplitRule::default(),
                Some(Salvage::from_share(0.5))
            )
            .halves()
            .is_none(),
            "部分救回页"
        );
    }

    /// **阈值可调**：调低了收得进更多页，调高了把跨页也放过去（spec 的 story 7）。
    #[test]
    fn the_threshold_moves_where_the_candidate_line_sits() {
        let page = with_gutter(SPREAD, 0.5, 40);
        let at = |threshold: f64| SplitRule {
            threshold: SplitThreshold::parse(&threshold.to_string()).expect("正数"),
            ..SplitRule::default()
        };

        // 默认这一档收得进它。
        assert!(
            Split::of_gray(&page, PANEL, SplitRule::default(), None)
                .halves()
                .is_some()
        );
        // 抬到比值之上，同一页落到线外——实测比值是 1.89（6048/4320 ÷ 1072/1448）。
        assert!(
            Split::of_gray(&page, PANEL, at(2.5), None)
                .halves()
                .is_none()
        );
        // 认不出的阈值在拼 Request 之前就被挡下。
        assert!(SplitThreshold::parse("零").is_err());
        assert!(SplitThreshold::parse("0").is_err());
        assert!(SplitThreshold::parse("-1").is_err());
    }

    /// 彩色分支与灰度路径切在同一处：墨量按灰度量，与面板认不认得颜色无关。
    ///
    /// 页画的是**真彩色**，不是三个平面放同一份灰度——后者会走进 `gray::value` 的消色短路，
    /// OKLab 那一支一次都跑不到，而两条路真要分家只会分在那里。
    #[test]
    fn the_color_branch_and_the_gray_path_cut_at_the_same_column() {
        // 小一点的跨页：这一条不问沟在哪，只问两条路答不答得一样。
        const SIZE: Size = Size::new(1512, 1080);
        let planes = |channel: usize| {
            let mut pixels = vec![0u8; (SIZE.width * SIZE.height) as usize];
            for y in 0..SIZE.height {
                for x in 0..SIZE.width {
                    // 纯红铺满，正中留一条纸白：红转灰约 130（是墨），白是纸。
                    let gutter = (700..740).contains(&x);
                    pixels[(y * SIZE.width + x) as usize] = match (gutter, channel) {
                        (true, _) | (false, 0) => 255,
                        _ => 0,
                    };
                }
            }
            GrayImage::new(SIZE, pixels)
        };
        let color = ColorImage::new(SIZE, [planes(0), planes(1), planes(2)]);
        let interleaved = color.interleaved();
        let gray = GrayImage::new(
            SIZE,
            interleaved
                .as_chunks::<3>()
                .0
                .iter()
                .map(|pixel| gray::value(pixel[0], pixel[1], pixel[2]))
                .collect(),
        );

        let by_gray = Split::of_gray(&gray, PANEL, SplitRule::default(), None);
        let by_color = Split::of_color(&color, PANEL, SplitRule::default(), None);

        assert_eq!(by_gray, by_color);
        assert!(by_gray.halves().is_some(), "夹具没造对：这一页该切得开");
    }

    /// **切出来的两半都不许是零宽**，页窄到什么程度都不许。
    ///
    /// 零宽的图下游没有一处拿得动它：`FitMode::target` 要拿宽高比算尺寸，编码器要写一张
    /// 0 像素的 PNG。挡住它的**不是**一句显式的守卫，而是 [`MAX_GUTTER`] 与 [`GUTTER_WINDOW`]
    /// 咬出来的结果——切点要落在第 0 列上，沟就得从第 0 列起且只有一列宽，
    /// 而那样的一列在够得上跨页候选的页上必然越过沟宽那道线。这条用例把那个结论钉住：
    /// 两个数改动之后它是第一个撞响的地方。
    ///
    /// 扫的是**窄页**，从两列一直到普通漫画页的宽度；每一种宽度都把沟摆遍每一列。
    #[test]
    fn no_page_narrow_enough_to_break_it_ever_yields_a_zero_wide_half() {
        let rule = SplitRule::default();
        let mut split_at_all = 0;
        for width in 2..64u32 {
            for gutter in 0..width {
                // 高取 1：宽高比因此恒等于宽，够得上跨页候选的门槛最低。
                let mut pixels = vec![0u8; width as usize];
                pixels[gutter as usize] = 255;
                let page = GrayImage::new(Size::new(width, 1), pixels);
                let Some(halves) = Split::of_gray(&page, PANEL, rule, None).halves() else {
                    continue;
                };
                split_at_all += 1;
                for half in halves {
                    let window = half.window().after();
                    assert!(
                        window.width > 0,
                        "{width} 列的页在沟落于第 {gutter} 列时切出了一张零宽的图"
                    );
                }
            }
        }
        // 夹具自证：这一趟真的切开过。一页都没切开的话上面那个断言一次都没跑到。
        assert!(
            split_at_all > 0,
            "扫了一遍一页都没切开，这条用例什么都没验证"
        );
    }

    /// 阅读方向的名字认得出来，规范名钉死——参数哈希拿它当稳定写法。
    #[test]
    fn every_reading_order_resolves_and_has_a_frozen_canonical_name() {
        assert_eq!(
            ReadingOrder::resolve(" RTL ").expect("认得"),
            ReadingOrder::RightToLeft
        );
        assert_eq!(
            ReadingOrder::resolve("Left-To-Right").expect("认得"),
            ReadingOrder::LeftToRight
        );
        assert!(ReadingOrder::resolve("japanese").is_err());
        assert_eq!(ReadingOrder::RightToLeft.name(), "rtl");
        assert_eq!(ReadingOrder::LeftToRight.name(), "ltr");
        // 默认是右开：日式漫画是绝大多数（spec 的 story 3）。
        assert_eq!(ReadingOrder::default(), ReadingOrder::RightToLeft);
    }
}
