//! 裁边：按行列墨量占比裁掉页面白边，**在适配之前**。
//!
//! 这件事的要点不是省白边，是让用户**关得掉阅读器那一侧的裁切**。6 寸屏上自动裁边是刚需，
//! 但阅读器裁掉白边就改变了页尺寸，随后的适配于是不再是 1.0 倍——抖动连同 1 像素周期的
//! 结构一起被抹平，字节照付（见 measurements 的《真机像素完整性》，两台设备都实测过）。
//! 裁边收进 tonefit 之后好处烤进产物，阅读器那个开关变成空操作，1:1 恢复。
//!
//! **收益不在分辨率上**：漫画页比面板更瘦长、受高度约束，裁掉左右白边不改变 fit-inside
//! 的缩放比（实测 ×1.000–1.019，见 measurements 的《裁边》）。收益是白边消失本身。
//!
//! 裁法**按行列墨量占比**，不取内容外接框：外接框边缘沾一个墨点就退回整页，
//! 而扫描件的白边里本来就有噪点——实测外接框那样量出来的中位增益是 0。
//!
//! 逐页各裁各的，**不取卷级裁切框**：卷级框会被留白最多的那一页拖住，
//! 而用户要的是更大的实际利用面积。代价是页与页的字号会跳动——实测线性放大的卷内极差
//! B 类是 0.041–0.109、A 类双页扫描是 0.390（见 measurements 的《裁边》）。
//! **那是接受的形态，不是缺陷。**
//!
//! 两条守卫拦住退化：一页挑不出内容行列就原样通过（整页白纸），
//! 窗口留不到原页一半也原样通过（几乎空白的页，见 [`MIN_KEPT`]）。
//!
//! **裁边不碰几何门，也没有把 ADR 0003 的硬上界静默关掉。**ADR 0007《后果》要求
//! 「任何改动目标尺寸的改动，都要同时对 ADR 0003 的硬上界负责——两条约束的破裂条件
//! 是同一个」。裁边改的是**送进适配的那个尺寸**，不是目标尺寸的来源：目标尺寸仍只由
//! `FitMode::target` 算出，门仍只在 `GeometryGate::of(目标尺寸, 面板)` 判一次
//! （ADR 0007 决定第 6 条：判定它的开关与 ADR 0003 用的是同一个）。
//! 两条约束的破裂条件因此仍是同一个，账没有分家。

use crate::color::ColorImage;
use crate::decode::Salvage;
use crate::geometry::Size;
use crate::gray::{self, GrayImage};

/// 墨阈：灰度取值**低于**它的像素算一个墨点。
///
/// 取 200 而不是「非纯白」：B 类整批是有损转码的（见 measurements 的《B 类素材普查》），
/// 纸白在里面是 240 上下的一片抖动值，而扫描件的纸更暗。阈太高，页上大片浅灰会被当成纸；
/// 阈太低，真纸白裁不干净。这个数与 [`INK_FRACTION`] 一同出自
/// measurements 的《裁边》，报告抬头原样印出（见 [`ink_rule`]）。
const INK: u8 = 200;

/// 一行或一列要有这么大比例的墨点，才算内容。
///
/// **这一条就是「孤立噪点不算内容」**：白边里一个墨点在一行 1441 像素上占 0.07%，
/// 够不到这道线，那一行仍是白边；内容外接框在同一页上会当场退回整页。
///
/// 它是**比例**，页小到几百像素时一个墨点就够得着这道线——合成夹具上因此看不出噪点被挡下，
/// 要看得拿真实尺寸的页（见本模块的用例）。
const INK_FRACTION: f64 = 0.005;

/// 这个像素算一个墨点吗。**本仓库唯一一处拿墨阈判像素的地方。**
///
/// 跨页拆分那一侧也问它（`crate::spread`）：装订沟检测与裁边是同一类扫描，
/// 「什么算墨」不该有第二套答案。
pub(crate) fn is_ink(value: u8) -> bool {
    value < INK
}

/// 一行或一列要有这么多个墨点才算内容，`length` 是那一行/列有多长
/// （行按页宽算，列按页高算——一行有多长，取决于页有多宽）。
///
/// 与 [`is_ink`] 同一个用意：占比那条线只有一个出处。跨页拆分拿它判「这一列是不是空白」
/// ——空白就是够不着这条线（`crate::spread`）。
pub(crate) fn content_line(length: u32) -> f64 {
    f64::from(length) * INK_FRACTION
}

/// 裁完至少要留下原页的这么大一截，宽与高**各自**都算。留不到就整页原样通过。
///
/// 它拦的是**几乎空白的页**：卷末那一张只印着一行版权字的页，按行列墨量占比量出来的窗口
/// 就是那一行字。裁掉之后再顶到面板高，一行字被放大几十倍，整页认不出是哪一页——
/// 实测棋魂完全版 Vol.01 的 `QH-01_0211` 正是这样一页（1582×2400 的窗口只剩 338×36，
/// 线性放大 66.7 倍，输出宽 13595）。
///
/// 「整页空白原样通过」拦不住它：那一页不空，只是几乎空。两条是同一条规矩的两半——
/// **裁边只拿走白边，不把一页放大到看不出是哪一页**。
///
/// **未标定占位值。**取 0.5 是因为两侧都离得远：实测六卷真实素材裁掉的面积中位 1.6%–16.1%、
/// 最多 35.9%（见 measurements 的《裁边》），一页都没有接近这道线；
/// 而上面那张退化页只留下 1.5% 的高。它同时把线性放大夹在 2 倍以内。
///
/// # 与目标尺寸那道兜底上界的关系（07 号票）
///
/// 那道上界（`crate::geometry` 的 `MAX_TARGET_PIXELS`）管的是「算出来的目标尺寸太大」，
/// 这一条管的是「裁出来的窗口太小」，后果看起来是同一个——一页被放大到荒唐的尺寸。
/// **两者仍然分开，理由有三条，每一条单独都足够：**
///
/// 1. **问的时刻不同。**这一条在裁哪一块的时候就要答，那时面板还没进来（[`Crop::of`]
///    连一个面板参数都没有）；兜底要等目标尺寸算出来才问得出口。合并等于把面板拖进裁边，
///    换来的是一个更宽的接口。
/// 2. **退法不同，而各自的退法只对各自那一张有用。**这一条退回整页原样通过——整页仍照
///    点名的适配方式出；兜底退回 fit-inside——裁边照旧生效。对调一下两张都收不了场：
///    一根 20000×100 的长条上白边根本不存在，退回整页什么都没改；
///    而棋魂那一页退回 fit-inside 会出成 338×36 的一小张，比整页出还糟。
/// 3. **两个数标定的东西不同。**这一条量的是「还认不认得出是哪一页」，是画质；
///    兜底量的是「分配得下分配不下」，是跑得完跑不完。一个常数同时服两条判据，
///    必然对其中一条太松或太紧。
///
/// **它们的作用区间也不重叠**，本模块的用例拿两张真页钉住了这件事：棋魂那一页裁完算出
/// 13595×1448，还在兜底上界之内——兜底接不住它；而长条那一张四边顶着墨、一个像素都裁不掉——
/// 这条守卫也接不住它。
///
/// 顺带得到的是一条界：留下的宽高各不少于原页的一半，裁边因此**至多把目标宽放大 2 倍**。
/// 越过兜底上界的必定是源页本身的形状，不会是裁边推过去的。
const MIN_KEPT: f64 = 0.5;

/// 裁法那两个数，报告抬头原样印出。
///
/// 与判据那一侧的 [`aggregation`](crate::aggregation) 同一条规矩：数摆出来，来源跟着摆出来，
/// 读的人自己判断它对手上这批素材成不成立。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkRule {
    /// 墨阈：灰度取值低于它的像素算一个墨点。
    pub ink: u8,
    /// 一行或一列要有这么大比例的墨点才算内容。
    pub fraction: f64,
}

impl std::fmt::Display for InkRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "按行列墨量占比 · 墨阈 {} · 行列占比 {:.1}%",
            self.ink,
            self.fraction * 100.0
        )
    }
}

/// 这一趟的裁法。**本仓库唯一的出处**，报告与用例都问它。
pub fn ink_rule() -> InkRule {
    InkRule {
        ink: INK,
        fraction: INK_FRACTION,
    }
}

/// 一页裁掉了多少：裁之前多大、留下的那一块在哪、留下多大。
///
/// 每一张处理成了的页都有一个（[`PageReport::crop`](crate::PageReport::crop)）——
/// 裁边关着的那一趟是一个原样通过的 [`Crop::keeping_all`]，
/// 「这一趟没裁」与「这一页没什么可裁」因此在报告里长得一样，
/// 分辨它们靠抬头那一行（裁边开着还是关着）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    /// 裁之前的尺寸，也就是这一页解出来的尺寸。
    before: Size,
    /// 留下的那一块的左上角。
    origin: (u32, u32),
    /// 留下的那一块的尺寸，**恒不为零**：一页白纸原样通过，不裁成零尺寸。
    after: Size,
}

impl Crop {
    /// 原样通过：一个像素都不裁。
    pub fn keeping_all(size: Size) -> Self {
        Self {
            before: size,
            origin: (0, 0),
            after: size,
        }
    }

    /// 直接造一个窗口：`origin` 是留下的那一块的左上角，`after` 是它的尺寸。
    ///
    /// **裁边那一侧一个都不用**——白边窗口只由 [`Crop::of_gray`] 与 [`Crop::of_color`]
    /// 量出来。生产路径上用它的只有跨页拆分（`crate::spread`）：切出来的那一半
    /// 同样是源页上的一块窗口，形状与裁出来的那一块一模一样，两者因此共用这个类型
    /// ——多一个只有名字不同的窗口类型，[`then`](Self::then) 那一步就得在两个类型之间翻译。
    ///
    /// 它公开还有第二个理由，与 [`Salvage::from_share`](crate::Salvage::from_share) 一样：
    /// [`Crop`] 是**报告那一侧**的类型，而渲染那一层与它的用例在另一个 crate 里
    /// （二进制那一侧的 `render`），要拼得出一份带裁边的报告。
    pub fn new(before: Size, origin: (u32, u32), after: Size) -> Self {
        Self {
            before,
            origin,
            after,
        }
    }

    /// 这一页的裁切窗口，按**灰度**上的行列墨量占比算出。
    ///
    /// 两种页原样通过：
    ///
    /// - 裁边**关着**（`--no-crop`）。
    /// - **部分救回页**。它缺的那一段留成纸白（`CONTEXT.md` 的《失败》），
    ///   而纸白按墨量就是白边——裁掉它等于把「这一页缺了一半」从产物里抹掉，
    ///   报告里那个救回比例也就再对不上尺寸。缺的那一段不是白边，是缺的那一段。
    pub fn of_gray(image: &GrayImage, enabled: bool, salvage: Option<Salvage>) -> Self {
        Self::of(image.size(), enabled, salvage, || {
            image.pixels().iter().copied().map(is_ink)
        })
    }

    /// 同上，但量的是彩色分支上那张图。
    ///
    /// 墨量仍按**灰度**量（走转灰那一条同一个换算，见 `crate::gray`）：一页的窗口不该因为
    /// 面板认不认得颜色而变——同一张彩页在黑白面板上转灰走灰度路径，两条路裁出来的
    /// 该是同一块。
    ///
    /// **不透明的源上两条路给出同一个答案**（本模块的用例逐像素比过）。带 alpha 的源上
    /// 合成次序不同：彩色那一侧先合到纸白再编回 sRGB（`color::over_paper`），
    /// 灰度那一侧在线性光里合完直接取 OKLab 的 L（`gray::gray_value_over_paper`），
    /// 两者在 sRGB 上差得到一两级——恰好卡在墨阈上的透明像素因此可能一路算墨、一路算纸。
    /// 那不是构造保证得了的，只是影响小到窗口至多差一个像素。
    pub fn of_color(image: &ColorImage, enabled: bool, salvage: Option<Salvage>) -> Self {
        let [red, green, blue] = image.planes();
        Self::of(image.size(), enabled, salvage, || {
            red.pixels()
                .iter()
                .zip(green.pixels())
                .zip(blue.pixels())
                .map(|((&red, &green), &blue)| is_ink(gray::value(red, green, blue)))
        })
    }

    /// 裁不裁由 `enabled` 与 `salvage` 定，裁哪一块由 `ink` 吐出的那串墨点判定定。
    ///
    /// 这两项**收在这里而不摊回两个调用处**，是为了让「谁不裁」只有一个出处：
    /// 灰度路径与彩色分支各判一次的话，救回页那条规矩迟早只剩一处还记得。
    /// 与几何门那一条同一个理由（`GeometryGate::of`：本仓库唯一一处判定几何门的地方）。
    ///
    /// `ink` 按**行优先**给出每个像素是不是墨点，懒求值：关着的那一趟一个像素都不看。
    fn of<I: Iterator<Item = bool>>(
        size: Size,
        enabled: bool,
        salvage: Option<Salvage>,
        ink: impl FnOnce() -> I,
    ) -> Self {
        if !enabled || salvage.is_some() {
            return Self::keeping_all(size);
        }
        let (width, height) = (size.width as usize, size.height as usize);
        let mut rows = vec![0u32; height];
        let mut columns = vec![0u32; width];
        for (index, is_ink) in ink().enumerate() {
            if is_ink {
                rows[index / width] += 1;
                columns[index % width] += 1;
            }
        }
        // 行按页宽算占比，列按页高算——一行有多长，取决于页有多宽。
        let (Some((top, bottom)), Some((left, right))) =
            (span(&rows, size.width), span(&columns, size.height))
        else {
            // 一条内容行、或一条内容列都挑不出来：整页白纸，原样通过（不裁成零尺寸）。
            return Self::keeping_all(size);
        };
        let after = Size::new(right - left + 1, bottom - top + 1);
        // 几乎空白的页原样通过：留下的这一块太小，那就不是白边，是一页几乎没有内容
        // （见 [`MIN_KEPT`]）。
        if f64::from(after.width) < f64::from(size.width) * MIN_KEPT
            || f64::from(after.height) < f64::from(size.height) * MIN_KEPT
        {
            return Self::keeping_all(size);
        }
        Self {
            before: size,
            origin: (left, top),
            after,
        }
    }

    /// 裁之前的尺寸。
    pub fn before(self) -> Size {
        self.before
    }

    /// 裁之后的尺寸。目标尺寸由**它**算出——裁边发生在适配之前。
    pub fn after(self) -> Size {
        self.after
    }

    /// 这一页真裁掉了东西吗。
    pub fn trimmed(self) -> bool {
        self.after != self.before
    }

    /// 左边裁掉了多少像素。
    ///
    /// 右边与下边不单开一个口子：`before`、`after` 与这一对左上角摆在一起，
    /// 四边就都减得出来，而多两个只会转发的读法（ADR 之外的话：那是 Middle Man）。
    pub fn left(self) -> u32 {
        self.origin.0
    }

    /// 上边裁掉了多少像素。见 [`left`](Self::left)。
    pub fn top(self) -> u32 {
        self.origin.1
    }

    /// **线性放大**：同一块内容在裁与不裁两种情形下，输出尺寸差了几倍。
    ///
    /// 页受高度约束时它就是 `裁之前的高 ÷ 裁之后的高`——裁掉上下白边之后，
    /// 同样的目标高留给内容的部分变多，字因此变大。逐页各裁各的，这个数于是**页与页不同**，
    /// 翻页时字号会跳动（实测卷内极差 0.041–0.390，见 measurements 的《裁边》）。
    /// 那是接受的形态，见本模块的模块文档。
    ///
    /// 管线自己不用它——它是**报告那一侧**的读法：跳动有多大只有这个量说得出来，
    /// 而「跳动是接受的形态」那条用例断言的正是它（`tests/pipeline.rs`）。
    /// 名字取自 measurements 的《裁边》那一列，两处说的是同一个数。
    pub fn linear_gain(self) -> f64 {
        f64::from(self.before.height) / f64::from(self.after.height)
    }

    /// 把这个窗口套在另一个窗口**里面**：`inner` 的坐标以本窗口留下的那一块为原点，
    /// 出来的仍是一个长在原页上的窗口。
    ///
    /// 跨页拆分要它（页几何批 04 号票）：整页裁一次、切成两半、每半再裁一次，
    /// 三段窗口叠起来仍是**源页上的一块**。报告因此只印一个窗口——读的人顺着
    /// 「解出来多大 → 留下哪一块」一路读下来，不必自己把三段坐标加起来，
    /// 而[线性放大](Self::linear_gain)也就仍然量得出「同一块内容裁与不裁差几倍」。
    pub fn then(self, inner: Crop) -> Crop {
        debug_assert_eq!(
            inner.before, self.after,
            "里层窗口不是长在外层留下的那一块上"
        );
        Crop {
            before: self.before,
            origin: (
                self.origin.0 + inner.origin.0,
                self.origin.1 + inner.origin.1,
            ),
            after: inner.after,
        }
    }

    /// 按这个窗口裁一张灰度图。窗口原样通过时**原样返回**，一个字节都不搬。
    pub fn apply_gray(self, image: GrayImage) -> GrayImage {
        if !self.trimmed() {
            return image;
        }
        self.take_gray(&image)
    }

    /// 按这个窗口从一张灰度图里**取出**那一块。与 [`apply_gray`](Self::apply_gray)
    /// 的区别只在所有权：这一个借图，因此同一张图上取得出两块——跨页拆分要的正是这件事。
    ///
    /// 它恒复制一份，连整页那个窗口也复制。调用方因此要自己躲开那一下
    /// （`crate::Compute` 上一对一那条路根本不走这里）。
    pub(crate) fn take_gray(self, image: &GrayImage) -> GrayImage {
        GrayImage::new(self.after, self.cut(image.pixels()))
    }

    /// 按这个窗口裁一张彩色图：三个平面各裁一遍。
    pub fn apply_color(self, image: ColorImage) -> ColorImage {
        if !self.trimmed() {
            return image;
        }
        self.take_color(&image)
    }

    /// 彩色那一侧的 [`take_gray`](Self::take_gray)。
    pub(crate) fn take_color(self, image: &ColorImage) -> ColorImage {
        let planes = image
            .planes()
            .each_ref()
            .map(|plane| GrayImage::new(self.after, self.cut(plane.pixels())));
        ColorImage::new(self.after, planes)
    }

    /// 从一个按行优先摊平的平面里取出窗口那一块。
    fn cut(self, pixels: &[u8]) -> Vec<u8> {
        let stride = self.before.width as usize;
        let (left, width) = (self.left() as usize, self.after.width as usize);
        let mut cut = Vec::with_capacity(width * self.after.height as usize);
        for row in self.top()..self.top() + self.after.height {
            let start = row as usize * stride + left;
            cut.extend_from_slice(&pixels[start..start + width]);
        }
        cut
    }
}

impl std::fmt::Display for Crop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "裁边 {} → {}", self.before, self.after)
    }
}

/// 一串计数里内容那一段的头尾下标，`length` 是每一格的满量。
///
/// 一格算内容的条件是它的占比够得着 [`content_line`]。取的是**头一格与末一格**，
/// 中间的空档原样留着：页当中一片浅色（天空、留白的格子）不是白边，裁边只从外面往里收。
fn span(counts: &[u32], length: u32) -> Option<(u32, u32)> {
    let needed = content_line(length);
    let content = |count: &u32| f64::from(*count) >= needed;
    let first = counts.iter().position(&content)?;
    let last = counts
        .iter()
        .rposition(&content)
        .expect("有头一格就有末一格");
    Some((first as u32, last as u32))
}

#[cfg(test)]
mod tests {
    //! 这里量的是**窗口本身**：给一页像素，裁法圈出哪一块。
    //!
    //! 同名的那几条在 `tests/pipeline.rs` 上量的是另一件事——窗口对**产物**做了什么
    //! （目标尺寸、写出的页、报告里的数）。两层不是重复：这一层换一个墨阈就红，
    //! 那一层换一个「先适配后裁边」的次序才红。

    use super::*;

    /// 真实尺寸的一页：噪点那条性质是**比例**，页太小就看不出它挡下了什么。
    const PAGE: Size = Size::new(1441, 2048);

    /// 造一页：四周白边，中间一块内容。内容一律纯黑，白边一律纸白。
    fn margins(size: Size, left: u32, top: u32, content: Size) -> GrayImage {
        let mut pixels = vec![255u8; (size.width * size.height) as usize];
        for y in top..top + content.height {
            for x in left..left + content.width {
                pixels[(y * size.width + x) as usize] = 0;
            }
        }
        GrayImage::new(size, pixels)
    }

    /// 裁出来的正是那块内容：裁前裁后两个尺寸、左上角，四边因此都减得出来。
    #[test]
    fn the_window_is_the_block_of_ink_and_the_report_can_say_how_much_went() {
        let content = Size::new(1200, 1800);
        let page = margins(PAGE, 120, 100, content);

        let crop = Crop::of_gray(&page, true, None);

        assert!(crop.trimmed());
        assert_eq!(crop.before(), PAGE);
        assert_eq!(crop.after(), content);
        assert_eq!((crop.left(), crop.top()), (120, 100));
        // 右边与下边减得出来：1441 - 1200 - 120 = 121，2048 - 1800 - 100 = 148。
        assert_eq!(
            (
                crop.before().width - crop.after().width - crop.left(),
                crop.before().height - crop.after().height - crop.top(),
            ),
            (121, 148)
        );
        // 裁完的像素与窗口对得上：全黑，一个白点都不剩。
        let cropped = crop.apply_gray(page);
        assert_eq!(cropped.size(), content);
        assert!(cropped.pixels().iter().all(|&value| value == 0));
    }

    /// **白边里的孤立噪点不算内容。**
    ///
    /// 这是本裁法与内容外接框的分水岭：外接框沾上一个墨点就退回整页，
    /// 实测那样量出来的中位增益是 0（见模块文档）。用例把两者并排比一遍——
    /// 外接框那一侧在这里就是「一个墨点都不放过」的窗口。
    #[test]
    fn an_isolated_speck_in_the_margin_is_not_content() {
        let content = Size::new(1200, 1800);
        let mut page = margins(PAGE, 120, 100, content);
        // 白边里三粒噪点：最左上角、右下角、以及正下方的白边中间。
        let mut speck = |x: u32, y: u32| {
            let mut pixels = page.pixels().to_vec();
            pixels[(y * PAGE.width + x) as usize] = 0;
            page = GrayImage::new(PAGE, pixels);
        };
        speck(0, 0);
        speck(PAGE.width - 1, PAGE.height - 1);
        speck(700, PAGE.height - 5);

        let crop = Crop::of_gray(&page, true, None);

        assert_eq!(crop.after(), content, "噪点把窗口撑回了整页");
        assert_eq!((crop.left(), crop.top()), (120, 100));
        // **夹具自证**：两粒噪点就落在对角的那两个像素上，外接框在这一页上因此恰好是整页，
        // 一个像素都裁不掉。没有这一句，本用例在一个退化成外接框的实现下也分不出对错。
        let corner = |x: u32, y: u32| page.pixels()[(y * PAGE.width + x) as usize];
        assert!(corner(0, 0) < INK && corner(PAGE.width - 1, PAGE.height - 1) < INK);
    }

    /// **几乎空白的页也原样通过**：留下的那一块太小，那就不是白边（见 [`MIN_KEPT`]）。
    ///
    /// 期望值取实测那一页退化得最厉害的（棋魂完全版 Vol.01 的 `QH-01_0211`，
    /// 见 measurements 的《裁边》）：一行版权字，窗口只剩 338×36。
    /// 裁掉它再顶到面板高，那一行字会被放大 66.7 倍。
    #[test]
    fn a_page_that_is_almost_blank_passes_through_whole() {
        let page = margins(Size::new(1582, 2400), 620, 1180, Size::new(338, 36));

        let crop = Crop::of_gray(&page, true, None);

        assert!(!crop.trimmed(), "把一行字当成整页内容裁了出来");
        assert_eq!(crop.linear_gain(), 1.0);
        // 那一块确实挑得出来——挡住它的是留得太少这一条，不是「挑不出内容」。
        assert!(
            page.pixels().iter().any(|&value| value < INK),
            "夹具没造对：这一页该有内容"
        );
        // 刚好留下一半的页裁得动：这道线是「留得太少」，不是「有留白就不裁」。
        let half = margins(Size::new(1000, 1000), 0, 0, Size::new(500, 500));
        assert!(Crop::of_gray(&half, true, None).trimmed());
    }

    /// **这条守卫与目标尺寸那道兜底上界各自接不住对方那一张**（07 号票，见 [`MIN_KEPT`]）。
    ///
    /// 这是「两者为什么不合并」里唯一量得出来的那一条，两张页都是真页，面板取实测那一块
    /// （boox-poke6，1072×1448）：
    ///
    /// - 棋魂 `QH-01_0211` 裁完是 338×36，顶到面板高算出 13595×1448 ——**1968 万像素，
    ///   在兜底上界之内**。兜底接不住它，拦下它的只能是这条守卫。
    /// - 一根四边顶着墨的长条一个像素都裁不掉，这条守卫连话都插不上，
    ///   而它算出的目标尺寸越过了兜底上界。
    #[test]
    fn this_guard_and_the_target_size_backstop_each_miss_the_other_ones_page() {
        use crate::geometry::{FitMode, max_target_pixels, pixels};
        const POKE6: Size = Size::new(1072, 1448);

        // 守卫那一张：真裁下去会算出 13595×1448，而那个尺寸兜底上界够不着。
        let almost_blank = margins(Size::new(1582, 2400), 620, 1180, Size::new(338, 36));
        assert!(!Crop::of_gray(&almost_blank, true, None).trimmed());
        let if_it_had_been_cropped = FitMode::Height.target(Size::new(338, 36), POKE6);
        assert_eq!(if_it_had_been_cropped.size(), Size::new(13595, 1448));
        assert!(
            !if_it_had_been_cropped.backstopped(),
            "兜底上界接住了守卫那一张，两者本可收在一处"
        );
        assert!(pixels(if_it_had_been_cropped.size()) < max_target_pixels());

        // 兜底那一张：满页是墨，白边不存在，守卫一个像素都拿不走。
        let strip = Size::new(3500, 100);
        let solid_ink = GrayImage::new(strip, vec![0; (strip.width * strip.height) as usize]);
        assert!(
            !Crop::of_gray(&solid_ink, true, None).trimmed(),
            "守卫接住了兜底那一张，那这条用例的前提就不成立"
        );
        assert!(FitMode::Height.target(strip, POKE6).backstopped());
    }

    /// 整页白纸原样通过，**不裁成零尺寸**：一页没有内容的页仍要写得出去。
    #[test]
    fn a_blank_page_passes_through_whole() {
        let blank = GrayImage::new(PAGE, vec![255; (PAGE.width * PAGE.height) as usize]);

        let crop = Crop::of_gray(&blank, true, None);

        assert!(!crop.trimmed());
        assert_eq!(crop.after(), PAGE);
        assert_eq!(crop.apply_gray(blank).size(), PAGE);
    }

    /// 一条内容列都挑不出来的页同样原样通过：只有一行墨的页，行说得出内容、列说不出，
    /// 两条轴缺一条就不裁。
    #[test]
    fn a_page_whose_columns_never_reach_the_line_passes_through_whole() {
        // 满宽一行黑：那一行 100% 是墨，而每一列只有 1/2048 = 0.05%，够不着 0.5%。
        let mut pixels = vec![255u8; (PAGE.width * PAGE.height) as usize];
        for x in 0..PAGE.width {
            pixels[(7 * PAGE.width + x) as usize] = 0;
        }
        let page = GrayImage::new(PAGE, pixels);

        assert!(!Crop::of_gray(&page, true, None).trimmed());
    }

    /// 页当中的浅色不是白边：裁边只从外面往里收，中间的空档原样留着。
    #[test]
    fn a_pale_band_in_the_middle_of_the_page_is_not_a_margin() {
        let mut pixels = vec![255u8; (PAGE.width * PAGE.height) as usize];
        let ink_rows = [0, 1, 2, PAGE.height - 3, PAGE.height - 2, PAGE.height - 1];
        for y in ink_rows {
            for x in 0..PAGE.width {
                pixels[(y * PAGE.width + x) as usize] = 0;
            }
        }
        let page = GrayImage::new(PAGE, pixels);

        assert!(
            !Crop::of_gray(&page, true, None).trimmed(),
            "上下都顶着墨，中间那一片留白不该被裁掉"
        );
    }

    /// 开关关着就一个像素都不裁，**部分救回页**同样原样通过。
    ///
    /// 后一条是有意的：救回页缺的那一段留成纸白，而纸白按墨量就是白边——
    /// 裁掉它，报告里那个救回比例就与尺寸对不上了（见 [`Crop::of_gray`]）。
    #[test]
    fn the_switch_and_a_salvaged_page_both_pass_through_whole() {
        let page = margins(PAGE, 120, 100, Size::new(1200, 1800));

        assert!(!Crop::of_gray(&page, false, None).trimmed(), "开关关着");
        assert!(
            !Crop::of_gray(&page, true, Some(Salvage::from_share(0.5))).trimmed(),
            "部分救回页"
        );
    }

    /// 墨阈是 200：正好 200 的像素是纸，199 才是墨。
    #[test]
    fn the_ink_threshold_takes_everything_below_two_hundred() {
        let page_of =
            |level: u8| GrayImage::new(PAGE, vec![level; (PAGE.width * PAGE.height) as usize]);
        // 整页 199：满页都是墨，没有白边可裁。
        assert!(!Crop::of_gray(&page_of(199), true, None).trimmed());
        // 整页 200：一个墨点都没有，整页白纸——同样原样通过，但走的是另一支。
        let pale = Crop::of_gray(&page_of(200), true, None);
        assert!(!pale.trimmed());
        assert_eq!(pale.after(), PAGE);
        assert_eq!(ink_rule().ink, 200);
    }

    /// 彩色分支与灰度路径裁出同一块：墨量按灰度量，与面板认不认得颜色无关。
    ///
    /// 页画的是**真彩色**，不是三个平面放同一份灰度——后者会走进 `gray::value` 的
    /// 消色短路，OKLab 那一支一次都跑不到，而两条路真要分家只会分在那里。
    /// 内容取顶上 200 行纯绿（转灰约 212，**亮于墨阈**）加下面 1600 行纯红（转灰约 130，是墨）：
    /// 绿那一段两条路都不该算内容，红那一段两条路都该算。
    /// 绿只占顶上一小条，是为了让剩下的红仍留得住原页的一半以上（见 [`MIN_KEPT`]）。
    #[test]
    fn the_color_branch_and_the_gray_path_cut_the_same_window() {
        const GREEN_ROWS: u32 = 200;
        let content = Size::new(1200, 1800);
        let planes = |channel: usize| {
            let mut pixels = vec![255u8; (PAGE.width * PAGE.height) as usize];
            for y in 100..100 + content.height {
                for x in 120..120 + content.width {
                    // 顶上一条纯绿、其余纯红：绿在 R 与 B 上是 0，红只在 R 上是 255。
                    let green = y < 100 + GREEN_ROWS;
                    pixels[(y * PAGE.width + x) as usize] = match (green, channel) {
                        (true, 1) | (false, 0) => 255,
                        _ => 0,
                    };
                }
            }
            GrayImage::new(PAGE, pixels)
        };
        let color = ColorImage::new(PAGE, [planes(0), planes(1), planes(2)]);
        // 灰度那一侧拿同一页真转一遍灰：两条路比的是同一批像素。
        let interleaved = color.interleaved();
        let gray = GrayImage::new(
            PAGE,
            interleaved
                .as_chunks::<3>()
                .0
                .iter()
                .map(|pixel| gray::value(pixel[0], pixel[1], pixel[2]))
                .collect(),
        );

        let by_gray = Crop::of_gray(&gray, true, None);
        let by_color = Crop::of_color(&color, true, None);

        assert_eq!(by_gray, by_color);
        // 只有红那一段算内容：绿转灰之后亮于墨阈，两条路都把它当纸。
        assert_eq!(
            by_color.after(),
            Size::new(content.width, content.height - GREEN_ROWS)
        );
        assert_eq!(by_color.top(), 100 + GREEN_ROWS);
        assert_eq!(by_color.apply_color(color).size(), by_color.after());
    }

    /// 逐页各裁各的，**线性放大因此页与页不同**——那是接受的形态，不是缺陷。
    ///
    /// 白边宽的那一页裁完剩得少、放得更大，白边窄的那一页放得少，翻页时字号跟着跳。
    /// 用户明确要更大的实际利用面积，不要卷级裁切框（那会被留白最多的一页拖住）。
    #[test]
    fn pages_with_different_margins_get_different_linear_gains_and_that_is_accepted() {
        let wide = Crop::of_gray(&margins(PAGE, 120, 300, Size::new(1200, 1400)), true, None);
        let narrow = Crop::of_gray(&margins(PAGE, 20, 20, Size::new(1400, 2000)), true, None);

        assert!(wide.linear_gain() > narrow.linear_gain());
        assert!(
            (wide.linear_gain() - narrow.linear_gain()).abs() > 0.1,
            "两页的放大差 {:.3}，夹具没把跳动造出来",
            wide.linear_gain() - narrow.linear_gain()
        );
        // 一个像素都没裁的页放大恒为 1：没裁就没有跳动可谈。
        assert_eq!(Crop::keeping_all(PAGE).linear_gain(), 1.0);
    }
}
