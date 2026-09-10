//! 纸白对齐：量出这一页的纸白，把 `[纸白, 255]` 这一段钳到 255，低于纸白的取值一个都不动。
//!
//! 平坦白底上误差扩散**必须撒点**，密度就是《离格量》÷《格点间距》（`CONTEXT.md` 的《量化》）。
//! 纸白 253 的页在 2bit 上因此得把 2.4% 的像素撒到低一级的格点上，
//! 而真机上五个人里五个说「明显影响观感」、零接受（measurements 的《真机三组》）；
//! 纸白 255 的那八页几乎分不出。分界线不在画风也不在题材，
//! 就在**源的纸白落不落在输出格点上**这一个逐页算得出来的量上。
//!
//! **255 是唯一在三档上都落在格点上的值**（`255 = 3×85 = 15×17`，格点是套嵌的），
//! 所以对齐它等于对三个候选同时对齐，**不必先知道这一页会判哪一档**。
//!
//! # 它是一个纯函数
//!
//! 一张图进、一张图加「这一页做了什么」出，不碰文件系统、不碰全局状态，与 [`crate::score`] 同型。
//! 落点只有一处——灰度路径上，**缩放之后、构造参照之前**：参照与其后一切量化用的都是对齐过的
//! 像素，判据两侧因此同源，量化仍然是唯一被隔离出来的变量（ADR 0002 决定第 1 条）。
//! 取对齐前的参照等于把自己的修正当成误差收费。
//! 彩色分支不经这里（ADR 0010：那条路径既不量化也不抖动，对齐对它没有意义）。
//!
//! **几何门不成立的页照样对齐**：那种页只是没有抖动那一维，纸白该在格点上还是要在。
//!
//! # 手段是钳制近白，不是拉伸也不是平移
//!
//! 线性拉伸会改动整条色调曲线（数字档案规范反对得最凶的一类），
//! 整幅平移会把纯黑推离格点、在另一头制造同一个病。钳制只动 `[纸白, 255]` 那一段，
//! **钳制宽度就是《离格量》**（`255 − 纸白`），它同时是这一步的代价：被压平的色调有多宽。
//!
//! 外部先例：Little-CMS 有一个机制同构的在产实现（`cmsopt.c` 的
//! `// Locate the node for the white point and fix it to pure white in order to avoid scum dot.`），
//! 默认开、带一个显式关的标志、三条守卫。本模块的三条与它一一对应，详见
//! `docs/research/paper-white-grid-alignment-prior-art.md`。

use std::fmt;

use crate::gray::GrayImage;

/// 全页的平坦像素要有这么多个，纸白才量得出来。
///
/// 数出自 measurements 的《全语料普查：四成三的页纸白不落在格点上》——那一趟就是拿这道线
/// 量的全语料 1202 页，也是它把画集 Venus 整部划到线外（最大平坦区只有 4430 px）。
/// **满版无纸边的页本来就没有大片平坦白底**：治不到也就没有不治的代价，
/// 业界五个工具在这一情形上一律是放弃（见模块文档指的那篇调研）。
///
/// **它与《纸白对齐上限》不是同一档待遇**：上限是用户调得动的口味取舍，
/// 这个数是那条定义自带的一部分，眼下只有一个调用方（停车场 Q519）。
const MIN_FLAT: u64 = 5000;

/// 纸白对齐的**上限**：这一趟最多愿意为对齐压平多宽的色调，单位是灰度级。
///
/// **上限即开关**：取 `0` 时只有「本来就在格点上」的页满足条件，即**一页都不会被改动**。
/// 不另做一个布尔开关——`--no-crop`、`--no-split`、`--per-page` 三个「关掉什么」的开关
/// 在预设里的覆盖是单向的（命令行没有再开回来的写法），数值参数没有这个毛病。
///
/// **取值只有这一处**（[`Default`]）。别处凡是说到 0 的都是在**描述**它——`--help`、
/// [`Request`](crate::Request) 那一格的文档、`CONTEXT.md` 的《尚未确立》——
/// 抬默认值那一趟要跟着改的是那几处措辞，不是又一个取值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhiteAlignLimit(u8);

impl WhiteAlignLimit {
    /// 关掉：一页都不改，产物与不带这个功能时逐字节相同。
    pub const OFF: Self = Self(0);

    /// 上限取这么多级。
    pub const fn new(levels: u8) -> Self {
        Self(levels)
    }

    /// 上限是多少级。
    pub const fn levels(self) -> u8 {
        self.0
    }
}

impl Default for WhiteAlignLimit {
    /// **默认取 0，即关闭**（spec 的《Implementation Decisions》第 11 条：
    /// 代码路径与参数先全部到位，一页都不改；抬到 4 是另一趟，当作一次全库重跑处理）。
    ///
    /// 4 这个占位值未标定，记在 `CONTEXT.md` 的《尚未确立》里。
    fn default() -> Self {
        Self::OFF
    }
}

impl fmt::Display for WhiteAlignLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 纸白对齐对这一页做了什么。
///
/// 五种情形里只有 [`Aligned`](Self::Aligned) 动过像素，其余四种整页原样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteAlignment {
    /// 这一趟没开（上限取 0）。纸白连量都没量——量了也没有一页满足得了条件。
    Off,
    /// 平坦像素太少，**量不出纸白**。满版无纸边的页落在这里。
    NoPaperWhite,
    /// 纸白本来就在格点上（离格量为 0）。全语料 57% 的页落在这里。
    OnTheGrid {
        /// 量出来的纸白。这一格恒为 255。
        paper_white: u8,
    },
    /// 离格量**超过上限**，整页原样。
    OverTheLimit {
        /// 量出来的纸白。钳制宽度是 `255 − 它`。
        paper_white: u8,
    },
    /// 对齐了：`[纸白, 255] → 255`，低于纸白的取值一个都没动。
    Aligned {
        /// 量出来的纸白。钳制宽度是 `255 − 它`，也就是这一页付出的代价。
        paper_white: u8,
    },
}

impl WhiteAlignment {
    /// 这一页量出来的纸白。**两种情形答不出**：[没开](Self::Off)那一趟根本没量，
    /// [量不出纸白](Self::NoPaperWhite)的那一页量了而这条定义说不出话。
    ///
    /// **不公开**：那个数已经明摆在三个变体自己的 `paper_white` 字段上，读的那一端
    /// `match` 一下就有，再开一个公开读数是替没人提过的需要加接口。
    /// 它只为 [`clamp_width`](Self::clamp_width) 而在——那一个要的是**同一张表**，
    /// 两处各 `match` 一遍早晚会走散。
    const fn paper_white(self) -> Option<u8> {
        match self {
            Self::Off | Self::NoPaperWhite => None,
            Self::OnTheGrid { paper_white }
            | Self::OverTheLimit { paper_white }
            | Self::Aligned { paper_white } => Some(paper_white),
        }
    }

    /// 这一页的**钳制宽度**，也就是它的《离格量》：对齐要压平多宽的色调，
    /// 也就是这一页付出的代价。
    ///
    /// **两种情形答不出**，与那个纸白同一条：没开那一趟根本没量，量不出纸白的那一页
    /// 量了而这条定义说不出话。
    ///
    /// **它不由读的那一端算**：`255 − 纸白` 这一句在本模块里只有一处出处
    /// （`clamp_width_at`），守卫那一侧比的也是它——两处各减一遍，早晚有一处减错。
    pub const fn clamp_width(self) -> Option<u8> {
        match self.paper_white() {
            Some(paper_white) => Some(clamp_width_at(paper_white)),
            None => None,
        }
    }
}

/// 纸白落在 `paper_white` 上时的**钳制宽度**（《离格量》）。这一句只有这一处。
const fn clamp_width_at(paper_white: u8) -> u8 {
    u8::MAX - paper_white
}

/// 把这一页的纸白对齐到 255。
///
/// # 三条守卫
///
/// 与 Little-CMS 的三条一一对应，三条**都整页原样返回**，一个像素都不动：
///
/// - **离格量为 0** → 不动。它已经在格点上了。
/// - **离格量大于 `limit`** → 不动。代价超过这一趟愿意付的。
/// - **量不出纸白**（全页的平坦像素不足 `MIN_FLAT` 个，那个数与它的出处见本模块内的常量）
///   → 不动。**不硬猜一个纸白**：满版画集没有大片平坦白底，猜出来的那个值会把整页改坏。
///
/// **黑扉页与彩页不必单列一条守卫**：那种页量出来的「纸白」是一个很暗的取值，
/// 离格量随之几百级，第二条守卫当场把它拦下。普查那一份在 `paper_white` **外面**
/// 另加了一道「代表值 < 200 就跳过」（1260 页里因此跳掉 45 页），
/// 那是**普查的卫生**，不是这条定义的一部分——照抄进来只会多一个够不着的分支，
/// 也会让两侧不再是同一份实现。
///
/// **上限取 0 时连纸白都不量**：那时唯一满足得了条件的是「本来就在格点上」的页，
/// 而钳它是恒等——量了也白量，默认那条路上因此一格开销都没有。
///
/// 这条短路**不是**「0 不改像素」赖以成立的东西：拆掉它，离格那条守卫照样把每一页拦下
/// （`离格量 > 0` 恒成立），一个像素还是不会动。两道各挡各的，
/// 短路挡的是**白花的工夫**，守卫挡的是**像素**。**试算那一趟正是拆掉它跑的**——
/// 它绕开这里、直接问本模块内那个只判不改的 `judge`，为的是让还没决定上限的用户
/// 看得见每一页差多少（理由写在那一头）。三条守卫因此只有 `judge` 一处，
/// 这里判完只管钳。
///
/// # 纸白的定义是可执行的
///
/// **全页 3×3 邻域方差为零的像素里，出现次数最多的那个灰度。**
///
/// 量的是**散布全页的那一张平坦掩码**，一次连通域分析都不做。
/// **它不是「最大那一块平坦区里的众数」**——两句只在最大那块恰好也是最常见取值时才同解，
/// 而普查那一份实现（见下）取的从来是**全页众数**那一句。
/// 票面第 2 条、`survey_flat.paper_white` 的文档字符串、以及 measurements 那一节眼下
/// 都还写着「最大那一块」，三处同源、都不归本模块改（停车场 Q520）。
///
/// 落到字节上的三条细节，**每一条都是复现口径的一部分**——第十轮真机那四页是拿普查那一份
/// 量出来的，对不上就比不了字节：
///
/// - **「方差为 0」比的是字节，不求浮点方差**：整数像素上它就是「九个邻居与中心逐个相等」，
///   两种写法同解，而比字节没有 epsilon。
/// - **边界按最近像素延拓**，与普查那一份的 `mode="edge"` 同一条。
/// - **众数并列时取更小的那个**：`np.bincount(...).argmax()` 并列时给的是最小下标，
///   移植跟着取小。
///
/// **不取最亮像素**（那是 KCC 的做法，一个亮斑就带偏），**不取高百分位**
/// （大片纯黑页上会取到错的值）。现成的那一份参照在
/// `.scratch/metric-recalibration/calibration/survey_flat.py` 的 `paper_white`。
pub fn align_white(image: GrayImage, limit: WhiteAlignLimit) -> (GrayImage, WhiteAlignment) {
    if limit.levels() == 0 {
        return (image, WhiteAlignment::Off);
    }
    let what = judge(&image, limit);
    let WhiteAlignment::Aligned { paper_white } = what else {
        // 三条守卫各放过一类页，**整页原样返回**。
        return (image, what);
    };
    let clamped = image
        .pixels()
        .iter()
        .map(|&value| if value >= paper_white { u8::MAX } else { value })
        .collect();
    (GrayImage::new(image.size(), clamped), what)
}

/// **只判不改**：这一页在这个上限下会落到[哪一种情形](WhiteAlignment)，一个像素都不碰。
///
/// 三条守卫**只有这一处**——[`align_white`] 判完再钳，两处各判一遍早晚会走散。
/// 它**不带那道短路**：上限取 0 时照样量、照样判，于是答的是
/// [`OverTheLimit`](WhiteAlignment::OverTheLimit) 一类的真话，而不是
/// [`Off`](WhiteAlignment::Off)。谁需要那句真话见下。
///
/// # 试算那一趟为什么要它（纸白对齐批 02 号票第 3 条）
///
/// 逐页那一层是给**还没决定上限取多少**的用户看的，而那个用户按定义上限就是 0——
/// 走 [`align_white`] 的话，那道短路让他每一页都读到「没开」，一个数都拿不到。
/// 试算因此绕开短路另判一遍：`--dry-run` 一个字节都不写，
/// 判一遍的代价是每页一遍平坦掩码，摆在同一页那六档判据旁边不算什么。
///
/// **照做那一趟不走这里**：那一趟的报告说的是「做过什么」，而上限取 0 时它什么都没做，
/// 连量都不该量（[`align_white`] 那道短路挡的正是这份白花的工夫）。
pub(crate) fn judge(image: &GrayImage, limit: WhiteAlignLimit) -> WhiteAlignment {
    let Some(paper_white) = paper_white(image) else {
        return WhiteAlignment::NoPaperWhite;
    };
    let clamp_width = clamp_width_at(paper_white);
    if clamp_width == 0 {
        return WhiteAlignment::OnTheGrid { paper_white };
    }
    if clamp_width > limit.levels() {
        return WhiteAlignment::OverTheLimit { paper_white };
    }
    WhiteAlignment::Aligned { paper_white }
}

/// 这一页的纸白，量不出来就是 `None`。定义见 [`align_white`] 的《纸白的定义是可执行的》。
fn paper_white(image: &GrayImage) -> Option<u8> {
    let size = image.size();
    // 空图不必单挡：两重循环一次都不进，`flat` 停在 0，末尾那道门限自己就答 `None`。
    // 下面那两处减一都在循环体里，走不到就减不着。
    let (width, height) = (size.width as usize, size.height as usize);
    let pixels = image.pixels();
    let mut counts = [0u64; 256];
    let mut flat = 0u64;
    for y in 0..height {
        let (top, bottom) = (y.saturating_sub(1), (y + 1).min(height - 1));
        for x in 0..width {
            let value = pixels[y * width + x];
            let (left, right) = (x.saturating_sub(1), (x + 1).min(width - 1));
            if is_flat(pixels, width, value, (top, bottom), (left, right)) {
                counts[value as usize] += 1;
                flat += 1;
            }
        }
    }
    if flat < MIN_FLAT {
        return None;
    }
    // 并列时取更小的那个（严格大于才换），与普查那一份的 `np.bincount(...).argmax()` 同解。
    let mode = (1..counts.len()).fold(0, |best, value| {
        if counts[value] > counts[best] {
            value
        } else {
            best
        }
    });
    Some(mode as u8)
}

/// 这一格的 3×3 邻域是不是严格平坦——九个（边界上更少）邻居与中心逐个相等。
fn is_flat(
    pixels: &[u8],
    stride: usize,
    value: u8,
    (top, bottom): (usize, usize),
    (left, right): (usize, usize),
) -> bool {
    (top..=bottom).all(|y| {
        pixels[y * stride + left..=y * stride + right]
            .iter()
            .all(|&p| p == value)
    })
}
