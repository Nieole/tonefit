//! 合成夹具生成器。
//!
//! 仓库里不放真实漫画素材：测试用的页全部由本模块按代码生成。
//! 每一类页对应一条待验证的性质，见调用它的用例。

// 每个测试二进制都单独编一份夹具，各自只用得上其中一部分。
#![allow(dead_code, unused_imports)]

mod cbz;

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageBuffer, Luma, Rgb, Rgba};
use tonefit::{BitDepth, Dither, GrayImage, Profile, Size};

pub use cbz::{Cbz, read_cbz};

/// B 类素材的中位尺寸（见 measurements 的《B 类素材普查》）。缩放比含小数。
pub const TYPICAL: Size = Size::new(1441, 2048);

/// 正好两倍面板，缩放后的尺寸没有取整歧义。总缩放比 2.000：预缩一步到位，残差比 1.000。
pub const DOUBLE_PANEL: Size = Size::new(2528, 3360);

/// 面板的 2.5 倍。总缩放比 2.500：预缩 2 之后残差段还剩 1.250，两级各真跑一次。
pub const TWO_AND_A_HALF_PANEL: Size = Size::new(3160, 4200);

/// 宽幅跨页：宽高比远超面板，fit-inside 由宽边定夺。
pub const SPREAD: Size = Size::new(5056, 1680);

/// **拆得开的跨页**（页几何批 04 号票）：形状取实测那一卷（哆啦A梦 8K 的 6048×4320，
/// 见 measurements 的《适配方式：fit-inside 与以高为准》）按半缩一档，宽高比 1.40 一模一样。
///
/// 拆开之后每半 1512×2160、宽高比 0.70——正是一张普通漫画页的形状。
/// 与 [`SPREAD`] 分开而不是共用一个尺寸：那一个宽高比 3.01，切成两半仍然远宽于面板，
/// 「拆开就不必横向翻动」在它身上不成立，而那正是本票的收益所在。
pub const SPREAD_WITH_GUTTER: Size = Size::new(3024, 2160);

/// 跨页夹具那条装订沟的中心，占页宽的比例。
///
/// 实测区间是 0.401–0.538（measurements 的《跨页拆分》），这里取偏离正中较远的一侧。
/// 「切点跟着沟走、不落在正中」那几条用例要的正是一个不在正中的沟：按正中盲切，
/// 这一页会切进画面 (0.5 − 0.441) × 页宽。
///
/// **不取实测最偏的 0.401**：那一条离装订沟检测窗口的边只剩 0.001，
/// 合成夹具落在那儿量出来的是窗口截断，不是切点（见 `tonefit` 的 `spread`）。
pub const GUTTER_CENTER: f64 = 0.441;

/// 跨页夹具那条装订沟有多宽，单位是列。占 [`SPREAD_WITH_GUTTER`] 页宽的 1.3%，
/// 落在实测的 0.17%–12.47% 之间（measurements 的《跨页拆分》）。
pub const GUTTER_WIDTH: u32 = 40;

/// 跨页夹具两半各自那圈墨边有多宽。
///
/// 不借 [`INK_BORDER`] 那个 4：一行要有页宽 0.5% 的墨点才算内容，而 3024 宽的页上
/// 那条线是 15.1 个像素——两半四条竖边合起来只有 4×4 = 16 个，堪堪压线。
/// 取 16 让它有四倍余量，裁边在这张页上因此稳稳是空操作。
const SPREAD_INK_BORDER: u32 = 16;

/// 宽高比 30:1 的长条：**目标尺寸的兜底上界拦得住它**（页几何批 07 号票）。
///
/// 在基准面板（1264×1680）上以高为准算出 50400×1680，8470 万像素，越过上界的 6710 万；
/// 退回 fit-inside 之后是 1264×42。解码那一侧拦不住这种页——它自己只有 30 万像素。
///
/// 拿它跑的用例一律配 [`solid`] 的纯墨：整页每一行每一列都是墨，裁边一个像素都拿不走，
/// 那道守卫因此插不上话，走到的只有兜底这一条（两者各自接不住对方那一张，
/// 见 `tonefit` 的 `crop` 里那条同名用例）。
pub const DEGENERATE_STRIP: Size = Size::new(3000, 100);

/// 宽高比 50:1 的**小**长条：以高为准算出的目标尺寸越过兜底上界，而退回的 fit-inside
/// 两边都比面板小——它因此**一条边都贴不住面板**，几何门在默认适配方式上不成立。
///
/// 那是 07 号票给「以高为准下门恒成立」开的唯一一个例外（见 `tonefit` 的
/// `GeometryGate::Broken`），也是互锁 ③ 在默认那条路上够得着的唯一形态
/// （页几何批 05 号票）。与 [`DEGENERATE_STRIP`] 差的正是这一条：那一根宽 3000，
/// 退回之后贴住了面板宽，门仍成立。
///
/// 在基准面板（1264×1680）上以高为准算出 84000×1680，1.41 亿像素，远过上界；
/// 退回 fit-inside 之后按不放大原样输出 1000×20。
/// 拿它跑的用例一律配 [`solid`] 的纯墨，理由同 [`DEGENERATE_STRIP`]。
pub const DEGENERATE_STRIP_SMALLER_THAN_PANEL: Size = Size::new(1000, 20);

/// 两边都小于面板：**fit-inside 下**不该被放大。
///
/// 以高为准会把它放大到面板高（页几何批 01 号票），几何门跟着成立——问「不放大」
/// 或问「门不成立」的用例因此要点名 [`run_volume_fitted_inside`]。
///
/// **它不是「一张便宜的页」**：默认那条路上它被放大到 1344×1680，像素比源多 2.8 倍。
/// 只要一张页的用例用 [`cheap_page`]（页几何批 09 号票）。
pub const SMALLER_THAN_TARGET: Size = Size::new(800, 1000);

/// 页数多的用例用的小页。卷级的性质只看逐页判定排开之后的分布，与页上有什么内容无关，
/// 页因此小到只够铺开几块判据分块就行。
///
/// 它**只在 fit-inside 上还是小页**：以高为准把每一页放大到面板高（页几何批 01 号票），
/// 一卷几十页的代价跟着涨两个数量级。拿它铺长卷的用例点名 [`run_volume_fitted_inside`]。
/// 默认那条路上的长卷用 [`NARROW_PASSES_THROUGH`]。
pub const TINY: Size = Size::new(160, 224);

/// **默认那条路上**铺长卷用的页：高已经等于基准面板的高，宽只够铺开两块判据分块。
///
/// 与 [`PASSES_THROUGH`] 同一条性质——两种适配方式下都恒等通过——只是窄得多：
/// 卷级那几条路径不看页上画着什么，只看逐页判定排开之后的分布，宽因此可以压到判据的
/// 分块聚合刚好还不退化（分块 32×32，两块就是 64）。一卷六十页约 1.2 秒。
///
/// **几何门在它每一页上都成立**，这正是它与 [`TINY`] 的分水岭：候选集因此是六个而不是三个
/// （多出抖动那一维），卷级那几条路径于是走在与默认路径上真实素材同一套候选上
/// （页几何批 08 号票）。
pub const NARROW_PASSES_THROUGH: Size = Size::new(64, 1680);

// 下面三个是**卷级用例造分布用的纯色取值**，本仓库测试侧唯一的出处。
//
// 判据在纯色页上算得出准数——量化误差就是取值到格点的距离，低通与掩蔽加权都不改它——
// 逐页判定因此由取值直接定死，而卷级那一层（上包络、离群、迟滞）要的正是一条排得开的分布。
// 三个取值摆在夹具这一侧而不是某个测试二进制里：`tests/golden.rs` 与 `tests/pipeline.rs`
// 是两个 crate，各写一份就会各自漂（`CLAUDE.md`《文档写作》：单一出处）。
//
// 判据读数一律取自基准设备（`BASELINE_DEVICE`），界是 5.5、离群线是 3 倍即 16.5。

/// 逐页判定要 `2bit` 的纯色页：85 正落在 2bit 的格点上（255 = 3×85）。
///
/// **两条适配方式上判出来的都是 `2bit`**：1bit 读 85；门成立时候选集多出抖动那一维，
/// 而 1bit+FS 在这一档灰调上的颗粒读 71.4，照样过不了界；第一个在界内的都是 2bit，读 0.000。
pub const NEEDS_TWO_BITS: u8 = 85;

/// 逐页判定**落在 [`NEEDS_TWO_BITS`] 那一档之上**的纯色页：96 在 2bit 上读 11.000——
/// 过了界（5.5），又远够不上「显著偏离」（16.5）。卷级迟滞那几条用例要的正是这个位置：
/// 基准档过不了它的界，它却不该被当成离群页摘走。
///
/// **落到哪一档由候选集说了算，而候选集由几何门裁**（ADR 0007），两条路上的机制并不相同：
///
/// - **门成立**（默认那条路，[`NARROW_PASSES_THROUGH`]）：候选六个，`2bit+FS` 读 3.693
///   在界内——判定与迟滞升上去的那一档都落在它上面，走的是「界以内最低的一档」。
///   纯色页在这套候选上**根本升不到 4bit**：`2bit+FS` 的颗粒是 `sqrt(d(85-d))`、上限 42.5，
///   压在可见度地板（55）以下，颗粒项恒为零，只剩低通残差 2.3~4.5。
/// - **门不成立**（[`TINY`] 那条路）：候选只有 {1bit, 2bit, 4bit}，读数 96.000 / 11.000 / 6.000
///   ——**一档都不在界内**，逐页判定走 `decide` 的兜底取候选上界 `4bit`，
///   迟滞那一段同样由 `envelope` 的 `lowest_for` 兜底抬到 `4bit`。
///
/// 两条路上它都落在基准档之上的那一档，两处的夹具因此共用它——但上面那两行说的不是
/// 同一套机制，读的人不该把它当成「同一条规则的两次应用」（页几何批 08 号票）。
pub const ONE_STEP_ABOVE_TWO_BITS: u8 = 96;

/// 逐页判定落在 `4bit` 的纯色页，且在 2bit 上读 42.000：**远在界外**（超过 16.5），
/// 离群页判据要的就是这一量级。用它的几条用例都跑在 fit-inside 上。
pub const FAR_OUTSIDE: u8 = 128;

/// **两种适配方式下都恒等通过**的尺寸：高已经等于基准面板的高，宽不到面板宽。
///
/// 以高为准原样输出（缩放比 1.000），fit-inside 也不放大——「输出与源逐字节相同」
/// 这类断言因此写得下来，而写下来的性质与这一趟走哪条适配方式无关。
/// 量解码、转灰、透明区、调色板的用例用它：那几条说的都不是几何。
///
/// **恒等只管缩放这一步**：默认那条路上裁边照跑，页上真有白边就照裁不误，
/// 「与源逐字节相同」当场不成立。这个尺寸因此要配一张四边顶着墨的页才算数——
/// 两样凑齐的那一张是 [`cheap_page`]（页几何批 09 号票）。
pub const PASSES_THROUGH: Size = Size::new(800, 1680);

/// 彩页的色带，自上而下。测试按序号取样。
///
/// 蓝与灰这一对是有意的：Rec.601/709 加权会把纯蓝压到 18~29，与第二条灰带混同；
/// OKLab 的 L 通道把它抬到 86 左右，两带保持可分。
pub const COLOR_BANDS: [[u8; 3]; 6] = [
    [0, 0, 255],     // 纯蓝
    [18, 18, 18],    // 深灰：纯蓝在 Rec.709 线性加权下的落点
    [255, 0, 0],     // 纯红
    [0, 255, 0],     // 纯绿
    [255, 255, 255], // 白
    [0, 0, 0],       // 黑
];

/// 连续渐变页：竖直方向 0→255 的线性斜坡，无边缘。
///
/// **裁边在它身上不是空操作**：下方 21.6% 亮于墨阈（200），按行列墨量占比就是白边，
/// 默认那条路上会被整片裁掉——1441×2048 的页因此出成 1507×1680，而不是 1182×1680。
/// 这一页于是**不等于**送进管线的那一页，几何上的断言在它身上写下来会撞上一个解释不了的数。
///
/// 因此：**问几何、问尺寸、问「输出与源逐字节相同」的用例一律用 [`full_bleed_gradient`]**
/// （页几何批 09 号票）。留着这一个的只有三种用例——白边本身是被测对象的（裁边那几条）、
/// 按 `--no-crop` 跑的（[`run_volume_keeping_margins`]），以及根本不进管线的
/// （`tests/metric.rs` 直接拿它喂判据）。黄金回归那一批夹具也留着它：裁边对它做了什么
/// 本身就记在快照里（见 `tests/golden.rs` 的 `KEPT_MARGINS`）。
pub fn gradient(size: Size) -> DynamicImage {
    let last = (size.height - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |_, y| {
        Luma([(y * 255 / last) as u8])
    }))
}

/// 连续渐变页，但**四边都顶着墨**：[`gradient`] 外面加一圈 [`INK_BORDER`] 像素宽的黑边。
///
/// 裁边在它身上是**空操作**（页几何批 02 号票），几何、解码、缩放那几条性质因此不与裁边
/// 缠在一起：`gradient` 下方亮于墨阈的那一段按行列墨量占比就是白边，会被裁掉，
/// 而那些用例说的都不是裁边。
///
/// 与 [`PASSES_THROUGH`] 同一个用意——那个尺寸让适配方式不起作用，这一圈墨边让裁边不起作用。
/// 页里仍是连续灰调：那一圈只占最外面几个像素。
pub fn full_bleed_gradient(size: Size) -> DynamicImage {
    inked_border(gradient(size))
}

/// **只要一张页**的用例用的那一张：[`PASSES_THROUGH`] 那个尺寸，配 [`full_bleed_gradient`]
/// 那圈墨边。
///
/// 那两处各自说了自己挡掉的是什么，这里不复述。凑齐之后在[基准面板](BASELINE_DEVICE)上
/// 走完整趟管线出来的页**逐字节地就是这里写下去的那一张**——容器形态、透传、命名、拒绝、
/// 隔离那一批用例问的都不是几何，它们要的正是这个（页几何批 09 号票）。
///
/// 它同时是这批夹具里**最便宜**的一张：不缩放、不裁切，一页就是 800×1680。
/// 从前那些用例拿的是 [`SMALLER_THAN_TARGET`]，而默认那条路把它裁完再放大到 1714×1680——
/// 像素多出两三倍，而多出来的那些一条断言都没参与。
///
/// 点名别的面板时缩放那一半不再是恒等（面板高不是 1680 了），墨边那一半照旧管用，
/// 而它仍是这批夹具里最便宜的那一张：拿它跑非基准 profile 的用例问的也都不是几何。
pub fn cheap_page() -> DynamicImage {
    full_bleed_gradient(PASSES_THROUGH)
}

/// 那一圈墨边多宽。取 4 而不是 1：有损格式会把一像素的黑边糊成灰，
/// 而 [`every_supported_format_decodes`](../pipeline.rs) 那一条要它在 AVIF 与 JPEG 上也还是墨。
pub const INK_BORDER: u32 = 4;

/// 给一张页加一圈纯黑边框，宽 [`INK_BORDER`]。
///
/// 裁法取的是**头一条与末一条**内容行列（见 `tonefit` 的 `crop`），
/// 四边各有一条满是墨的行/列，窗口于是恒等于整页。
fn inked_border(image: DynamicImage) -> DynamicImage {
    let (width, height) = (image.width(), image.height());
    let mut gray = image.to_luma8();
    for (x, y, pixel) in gray.enumerate_pixels_mut() {
        if x < INK_BORDER || y < INK_BORDER || x + INK_BORDER >= width || y + INK_BORDER >= height {
            *pixel = Luma([0]);
        }
    }
    DynamicImage::ImageLuma8(gray)
}

/// 一张**带装订沟的跨页**：两半各画一段竖直渐变、各自四边顶着墨，中间一条贯穿全高的纸白。
///
/// 那条纸白就是装订沟。四边顶着墨买两件事：**裁边在整页上是空操作**（拆分那几条用例
/// 因此不与裁边缠在一起，与 [`full_bleed_gradient`] 同一个用意），而**每半再裁**那一步
/// 恰好只收走沟那一侧——两半的窗口于是严丝合缝地贴着沟，切点错一列当场看得出来。
///
/// 沟的位置由 `center` 定，宽由 `gutter` 定；沟的头一列由 [`gutter_left`] 算出，
/// 用例拿它推两半该有多宽——那个算式**只有一个出处**。
pub fn spread_with_gutter(size: Size, center: f64, gutter: u32) -> DynamicImage {
    let left = gutter_left(size, center, gutter);
    let last = (size.height - 1).max(1);
    let edge = SPREAD_INK_BORDER;
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        if (left..left + gutter).contains(&x) {
            return Luma([255]);
        }
        let (from, to) = if x < left {
            (0, left)
        } else {
            (left + gutter, size.width)
        };
        if x < from + edge || x + edge >= to || y < edge || y + edge >= size.height {
            return Luma([0]);
        }
        Luma([(y * 255 / last) as u8])
    }))
}

/// 把一张灰度页染成**真彩色**：墨的地方按行给红／绿／蓝，纸白仍是纸白。
///
/// 「真彩色」是要点：三个平面放同一份灰度会走进 `gray::value` 的消色短路，
/// 转灰那一支一次都跑不到，而灰度路径与彩色分支真要分家只会分在那里
/// （与 `tonefit` 的 `crop`、`spread` 里那两条同名用例同一个用意）。
///
/// 纸白不染色：装订沟得留着，不然染完就没有沟可找了。
pub fn colorize(page: &DynamicImage) -> DynamicImage {
    let gray = page.to_luma8();
    let (width, height) = (gray.width(), gray.height());
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
        let value = gray.get_pixel(x, y)[0];
        if value > 240 {
            return Rgb([255, 255, 255]);
        }
        // 逐行换一个色相：整页因此有真实的色度覆盖，不是一片单色。
        let channel = (y % 3) as usize;
        let mut pixel = [0u8; 3];
        pixel[channel] = 255 - value / 2;
        Rgb(pixel)
    }))
}

/// [`spread_with_gutter`] 造出的那条沟的**头一列**。用例拿它推两半的窗口该落在哪儿。
pub fn gutter_left(size: Size, center: f64, gutter: u32) -> u32 {
    (f64::from(size.width) * center) as u32 - gutter / 2
}

/// 二值网点页：只有 0 与 255 两个取值，点的大小随横向位置由小到大。
///
/// 用经典 8×8 聚集点阈值矩阵对横向斜坡二值化——网点是因，灰调是果，
/// 这页故意只提供「因」，让缩放去解析出「果」。
pub fn screentone(size: Size) -> DynamicImage {
    const CLUSTERED: [[u8; 8]; 8] = [
        [24, 10, 12, 26, 35, 47, 49, 37],
        [8, 0, 2, 14, 45, 59, 61, 51],
        [22, 6, 4, 16, 43, 57, 63, 53],
        [30, 20, 18, 28, 33, 41, 55, 39],
        [34, 46, 48, 36, 25, 11, 13, 27],
        [44, 58, 60, 50, 9, 1, 3, 15],
        [42, 56, 62, 52, 23, 7, 5, 17],
        [32, 40, 54, 38, 31, 21, 19, 29],
    ];
    let last = (size.width - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        let tone = x * 255 / last;
        let threshold = u32::from(CLUSTERED[(y % 8) as usize][(x % 8) as usize]) * 4 + 2;
        Luma([if tone > threshold { 255 } else { 0 }])
    }))
}

/// 线稿页：白底黑线，硬边不抗锯齿。高对比度墨线，两端取值必然到底。
pub fn line_art(size: Size) -> DynamicImage {
    let (w, h) = (size.width, size.height);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(w, h, |x, y| {
        let border = x < 6 || y < 6 || x + 6 >= w || y + 6 >= h;
        let divider = y.abs_diff(h / 2) < 3;
        let diagonal = (x * h).abs_diff(y * w) < 3 * w;
        let back_diagonal = (x * h + y * w).abs_diff(w * h) < 3 * w;
        Luma([if border || divider || diagonal || back_diagonal {
            0
        } else {
            255
        }])
    }))
}

/// 一页留白，左上角一块 `patch` 大的灰调补丁——低位深下唯一会崩的就是这块。
///
/// 补丁竖直方向 0→255，与 [`gradient`] 同一条斜坡，只是圈在一小块里。
/// 页上其余部分是白的：白在任何位深上都是格点，误差恒为零，
/// 这一页的判据于是完全由那一小块说了算。
pub fn tone_patch(size: Size, patch: Size) -> DynamicImage {
    let last = (patch.height - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        Luma([if x < patch.width && y < patch.height {
            (y * 255 / last) as u8
        } else {
            255
        }])
    }))
}

/// 四周纸白、中间一块内容的页：**裁边那几条用例的被测对象**（页几何批 02 号票）。
///
/// 内容是竖直方向 0→190 的斜坡——整块都低于墨阈（200），因此内容里每一行、每一列
/// 都是满的墨，而白边一个墨点都没有。裁出来的窗口于是恰好是 `content` 那一块，
/// 期望值写得出字面值。
pub fn page_with_margins(size: Size, origin: (u32, u32), content: Size) -> DynamicImage {
    let (left, top) = origin;
    let last = (content.height - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        let inside = x >= left && x < left + content.width && y >= top && y < top + content.height;
        Luma([if inside {
            ((y - top) * 190 / last) as u8
        } else {
            255
        }])
    }))
}

/// 同上，但白边里另落了几粒**孤立噪点**：扫描件白边上的墨点。
///
/// 三粒顶在两个对角与下方白边正中，**内容外接框因此退回整页**（实测那样量出来的
/// 中位增益是 0）；按行列墨量占比的裁法不受它们影响——一粒墨在一行 1441 像素上
/// 占 0.07%，够不到 0.5% 那道线。用它与 [`page_with_margins`] 并排比，
/// 两者裁出来的窗口必须一模一样。
pub fn page_with_specks_in_the_margin(
    size: Size,
    origin: (u32, u32),
    content: Size,
) -> DynamicImage {
    let mut page = page_with_margins(size, origin, content).to_luma8();
    for (x, y) in [
        (0, 0),
        (size.width - 1, size.height - 1),
        (size.width / 2, size.height - 3),
    ] {
        page.put_pixel(x, y, Luma([0]));
    }
    DynamicImage::ImageLuma8(page)
}

/// 一页上全部墨点的**外接框**有多大。只给裁边那条用例当对照，不是被测代码。
///
/// 「本裁法不是外接框」这句话要有一个东西替它作证：同一页上外接框退回整页，
/// 而裁边裁出了内容那一块。墨阈与 `tonefit` 的 `crop` 同一个（200）。
pub fn ink_bounding_box(image: &DynamicImage) -> Size {
    let gray = image.to_luma8();
    let (mut left, mut top) = (gray.width(), gray.height());
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in gray.enumerate_pixels() {
        if pixel[0] >= 200 {
            continue;
        }
        left = left.min(x);
        right = right.max(x);
        top = top.min(y);
        bottom = bottom.max(y);
    }
    Size::new(right + 1 - left, bottom + 1 - top)
}

/// 纯色页：全图同一个灰度取值。
pub fn solid(size: Size, level: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |_, _| {
        Luma([level])
    }))
}

/// 白底、顶上 `black_rows` 行涂黑、**四边一圈墨**的一页：整页只有纯黑与纯白两个取值。
///
/// 这两个取值在 {1,2,4,8} 每一档上都是格点，量化与抖动对它们都是恒等；页小于面板时
/// 又一步都不缩放。写出的像素于是与源**逐字节相同**——断言写得起等号，不必留容差。
/// 同一卷里给每页配一个不同的 `black_rows`，页与页就两两分得开。
///
/// 那一圈墨边让裁边成为空操作（页几何批 02 号票）：没有它，白底那一大片就是白边，
/// 页会被裁成 `宽 × black_rows`，而这一条用例说的是「这一格装的是不是它自己的像素」，
/// 不是裁边。理由与 [`full_bleed_gradient`] 同一条。
pub fn black_top_band(size: Size, black_rows: u32) -> DynamicImage {
    inked_border(DynamicImage::ImageLuma8(ImageBuffer::from_fn(
        size.width,
        size.height,
        |_, y| Luma([if y < black_rows { 0 } else { 255 }]),
    )))
}

/// 一张灰度页摊平成 8 位灰度像素，按行优先。断言「写出的与源逐字节相同」时拿它当期望值。
pub fn luma_pixels(image: &DynamicImage) -> Vec<u8> {
    image.to_luma8().into_raw()
}

/// 彩页：`COLOR_BANDS` 自上而下的等高色带。
pub fn color_page(size: Size) -> DynamicImage {
    let bands = COLOR_BANDS.len() as u32;
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(size.width, size.height, |_, y| {
        let band = (y * bands / size.height).min(bands - 1) as usize;
        Rgb(COLOR_BANDS[band])
    }))
}

/// 截断的页：一张纯黑页的完整 PNG，只留前 `KEPT_FRACTION` 那一段。
///
/// 文件头与 IHDR 都在，IDAT 只剩一截：完整尺寸解得出来，像素解不全，但救得回一段。
/// 这是**部分救回页**——三种页状态里的第三种（04 号票）：它照常缩放、判定、写出，
/// 但不参与几何门与卷级上包络。
///
/// 纯黑是为了让两段一眼分得开：解回来的那一段是 0，救不回来的那一段是纸白 255。
pub fn truncated_page(size: Size) -> Vec<u8> {
    truncated(&solid(size, 0))
}

/// 同上，但页上画什么由调用方定：判定要跟着救回来的那一段走的用例需要它。
pub fn truncated(image: &DynamicImage) -> Vec<u8> {
    let bytes = encode_image(image, "png");
    bytes[..bytes.len() * KEPT_FRACTION / 100].to_vec()
}

/// 截断的页留下的那一段占原文件的百分之多少。
///
/// 取一半是为了让「解出来的那部分」与「缺的那一段」两侧都真的存在：
/// 留太多，整页都解得回来，留白那一段测不到；留太少，连一行都解不出来。
const KEPT_FRACTION: usize = 50;

/// 一行像素都救不回来的页：完整的文件头与 IHDR，第一个 IDAT 只剩块头。
///
/// 尺寸解得出来，救回那一趟一个像素都没写下——这正是 04 号票要拦的那一张：
/// 按 12 号票的界线（「解得出完整尺寸就照用」）它是一张正常页，写出去是一整张纸白，
/// 带着正常的判定元数据，卷还留在干净的去处。
///
/// 砍在第一个 IDAT 的块头上，而不是按比例砍：能不能救回取决于砍在哪里，不取决于比例
/// （见 measurements 的《截断页的解码容忍度》）。按比例砍出来的「零救回」会随
/// 编码器的输出长度漂，砍在块头上则是个定值。
pub fn salvages_nothing_page(size: Size) -> Vec<u8> {
    let bytes = encode_image(&solid(size, 0), "png");
    let mut at = PNG_SIGNATURE;
    loop {
        let length = u32::from_be_bytes(
            bytes[at..at + 4]
                .try_into()
                .expect("PNG 块头有四个字节的长度"),
        ) as usize;
        if &bytes[at + 4..at + 8] == b"IDAT" {
            // 留下块头、丢掉块身：解码器读得到「这里有一块数据」，读不到数据本身。
            return bytes[..at + 8].to_vec();
        }
        // 长度、类型、块身、CRC。
        at += 12 + length;
    }
}

/// PNG 文件签名的长度。块从它之后开始。
const PNG_SIGNATURE: usize = 8;

/// 尺寸大到解码器分配不出缓冲的页：只有文件头与 IHDR，横竖各 65535。
///
/// 像素缓冲要 4 GiB，越过解码器的分配上界。尺寸解得出来，这一页仍然算失败——
/// 分配不出来的页救不回任何像素（见 `decode::salvage`）。
pub fn oversized_page() -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(b"IHDR");
    header.extend_from_slice(&u32::from(u16::MAX).to_be_bytes());
    header.extend_from_slice(&u32::from(u16::MAX).to_be_bytes());
    // 位深 8、灰度、默认压缩与滤波、非隔行。
    header.extend_from_slice(&[8, 0, 0, 0, 0]);

    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend_from_slice(&(header.len() as u32 - 4).to_be_bytes());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&crc32fast::hash(&header).to_be_bytes());
    bytes
}

/// 带透明区的页：左半不透明黑，右半全透明——而 RGB 仍是黑。
/// 丢 alpha 会让右半出成黑，按纸白合成才出成白。
///
/// 四边另有一圈 [`INK_BORDER`] 宽的不透明黑，理由与 [`full_bleed_gradient`] 同一条：
/// 透明区合成之后就是纸白，而纸白按墨量就是白边，裁边会把右半整个裁掉
/// （页几何批 02 号票）——而这一条用例说的是透明区合成成了什么，不是裁边。
pub fn page_with_transparency(size: Size) -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        let border = x < INK_BORDER
            || y < INK_BORDER
            || x + INK_BORDER >= size.width
            || y + INK_BORDER >= size.height;
        if border || x < size.width / 2 {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    }))
}

/// 色带 `band` 的纵向中心行，供测试取样。
pub fn band_center_row(size: Size, band: usize) -> u32 {
    let bands = COLOR_BANDS.len() as u32;
    (band as u32 * 2 + 1) * size.height / (bands * 2)
}

/// 一次测试的工作区：源卷与输出根目录分处两地，互不嵌套。
pub struct Workspace {
    tmp: tempfile::TempDir,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().expect("建临时目录"),
        }
    }

    /// 建一个空的目录卷。
    pub fn volume(&self, name: &str) -> Volume {
        Volume::new(self.tmp.path().join(name))
    }

    /// 建一个空的 CBZ 卷。加完成员要调 `Cbz::write` 才落盘。
    pub fn cbz(&self, name: &str) -> Cbz {
        Cbz::new(self.tmp.path().join(format!("{name}.cbz")))
    }

    /// 在工作区根下写一个不属于任何卷的文件。用来造「扩展名像卷、内容不是」的输入。
    pub fn stray_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.tmp.path().join(name);
        fs::write(&path, bytes).expect("写工作区文件");
        path
    }

    /// 输出根目录。此刻还不存在，由被测代码建出来。
    pub fn out(&self) -> PathBuf {
        self.tmp.path().join("out")
    }

    /// 另起一个输出根，与 [`out`](Self::out) 并列。同一个卷跑两趟、比两份输出的用例用它——
    /// 换个工作区跑第二趟就得把卷也生成两份，那时比的就不只是这两趟的差别了。
    pub fn out_named(&self, name: &str) -> PathBuf {
        assert_ne!(name, "out", "另起的输出根不该与默认那个撞名");
        self.tmp.path().join(name)
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

/// 同一档位深上不抖动的那个候选。几何门不成立的页只有这一种候选可选。
pub const fn plain(bit_depth: BitDepth) -> tonefit::Candidate {
    tonefit::Candidate::new(bit_depth, Dither::Off)
}

/// 基准设备：`CONTEXT.md` 里阈值标定的那台。不点名 profile 的用例都用它。
pub const BASELINE_DEVICE: &str = "kobo-libra-2";

/// 按型号名取 profile。型号必须在内置表里，不在就是夹具写错了。
pub fn profile(device: &str) -> Profile {
    Profile::resolve(device).unwrap_or_else(|error| panic!("解析 profile {device}：{error}"))
}

/// 基准设备的 profile。自己拼 `Request` 的用例用它填 profile 那一项。
pub fn baseline_profile() -> Profile {
    profile(BASELINE_DEVICE)
}

/// 与基准面板等大的页尺寸：源即目标，不缩放，几何门两条边都贴住，
/// 也就是这台 profile 输出得到的**最大尺寸**。
///
/// 判据的分块聚合按**目标尺寸**铺开块数，「多小的损伤读得出来」因此只在真实输出尺寸上
/// 问得准（ADR 0002 的《第 3 条为什么改过》）。从 profile 推出而不写死：
/// 面板表改了，用它的夹具跟着走。
pub fn panel_sized() -> Size {
    baseline_profile().panel().resolution
}

/// 对一个目录卷调用被测的 `run`，用基准设备的 profile。
pub fn run_volume(space: &Workspace, volume: &Volume) -> tonefit::Report {
    run_volume_with(space, volume, baseline_profile())
}

/// 同上，但点名 profile。
pub fn run_volume_with(space: &Workspace, volume: &Volume, profile: Profile) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        profile,
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 把输出钉在 8bit 再跑一遍：量化成了恒等，写出的就是它之前那一步的结果。
///
/// 量重采样、解码或转灰的用例用这个。判定位深会把取值压到那一档的格点上，
/// 混进来就分不清一处差异出自哪一步——而那几条性质说的都不是量化。
///
/// 三个开关都是用户手上真有的：抬上界走 ADR 0003 给的 `--gray-levels`，点名位深走
/// `--bit-depth`，点名不抖动走 `--dither`。抖动在 8bit 上本来就是恒等（格点即工作精度），
/// 点名它是为了把候选裁到只剩一个——判定于是整个被顶掉，报告里那一项也就没有歧义。
pub fn run_volume_at_eight_bits(space: &Workspace, volume: &Volume) -> tonefit::Report {
    at_eight_bits(space, volume, tonefit::FitMode::default())
}

/// 同上，但点名 fit-inside：问「比面板小的页不放大」的用例用它（页几何批 01 号票）。
pub fn run_volume_at_eight_bits_fitted_inside(
    space: &Workspace,
    volume: &Volume,
) -> tonefit::Report {
    at_eight_bits(space, volume, tonefit::FitMode::Inside)
}

fn at_eight_bits(space: &Workspace, volume: &Volume, fit: tonefit::FitMode) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        profile: baseline_profile().with_gray_levels(256).expect("全集可用"),
        fit,
        bit_depth: Some(BitDepth::Eight),
        dither: Some(Dither::Off),
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 把这一卷按 `--no-crop` 跑一遍：白边留着（页几何批 02 号票）。
///
/// 点名它的只有一种用例：**页上那一片白本身就是被测对象**，裁掉它就没什么可断言了。
/// 别的用例一律走默认那条路（裁边开着），需要的话把夹具换成四边顶着墨的那几个
/// （[`full_bleed_gradient`]、[`line_art`]）——那样钉住的性质与裁边无关，读起来也不必绕。
pub fn run_volume_keeping_margins(space: &Workspace, volume: &Volume) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        crop: false,
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 把这一卷**不拆跨页**跑一遍（`--no-split`，页几何批 04 号票）。
///
/// 拆与不拆的对照要它：跨页不拆时顶到面板高、宽溢出面板，靠阅读器横向平移看。
pub fn run_volume_without_splitting(space: &Workspace, volume: &Volume) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        split: tonefit::SplitRule {
            on: false,
            ..tonefit::SplitRule::default()
        },
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 把这一卷按 **fit-inside** 跑一遍（页几何批 01 号票）。
///
/// 两种用例点名它：问几何门**不成立**那一支的（默认那条路上它是空集——以高为准让每一页的
/// 高都等于面板高），以及拿 [`TINY`] 或 [`SMALLER_THAN_TARGET`] 铺出小页来图快的。
pub fn run_volume_fitted_inside(space: &Workspace, volume: &Volume) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        fit: tonefit::FitMode::Inside,
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 点名若干卷跑一遍，用基准设备的 profile。目录与归档都走这里。
pub fn run_paths<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> tonefit::Report {
    tonefit::run(&request(space, inputs)).expect("处理应当成功")
}

/// 同上，但预期失败，返回那个错误。
pub fn run_paths_expecting_failure<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> anyhow::Error {
    tonefit::run(&request(space, inputs)).expect_err("处理应当失败")
}

/// 拼一个用基准 profile、输出到工作区 `out/` 的 `Request`。
/// 换 profile 或要复用同一个 `Request` 的用例自己拼。
pub fn request<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> tonefit::Request {
    tonefit::Request {
        inputs: inputs.into_iter().map(Path::to_path_buf).collect(),
        output_root: space.out(),
        profile: baseline_profile(),
        fit: tonefit::FitMode::default(),
        crop: true,
        split: tonefit::SplitRule::default(),
        filter: tonefit::Filter::default(),
        bit_depth: None,
        dither: None,
        per_page: false,
        cache_budget: tonefit::CacheBudget::default(),
        mode: tonefit::Mode::Process,
        io_mode: tonefit::IoMode::default(),
        progress: None,
        metadata: true,
    }
}

/// 一个卷：磁盘上装着页的目录。
pub struct Volume {
    root: PathBuf,
}

impl Volume {
    /// 建目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        fs::create_dir_all(&root).expect("建卷目录");
        Self { root }
    }

    /// 写一页，格式由 `name` 的扩展名决定。返回写出的路径。
    pub fn page(&self, name: &str, image: &DynamicImage) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建页所在目录");
        }
        write_image(&path, image);
        path
    }

    /// 写一个非图片文件。名字带目录就把目录一起建出来，与 [`page`](Self::page) 同形。
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建文件所在目录");
        }
        fs::write(&path, bytes).expect("写非图片文件");
        path
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

fn write_image(path: &Path, image: &DynamicImage) {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    fs::write(path, encode_image(image, &extension)).expect("写夹具页");
}

/// 把一张图编成 `extension` 指定的格式。归档夹具的成员也走这里，因此编出的是字节。
pub fn encode_image(image: &DynamicImage, extension: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    if extension == "avif" {
        // 默认的 speed 4 编一页要几秒；夹具只需要一个能解的 AVIF。
        let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut bytes, 10, 90);
        image.write_with_encoder(encoder).expect("编 AVIF");
        return bytes.into_inner();
    }
    let format = image::ImageFormat::from_extension(extension)
        .unwrap_or_else(|| panic!("不认识的夹具格式 {extension}"));
    if format == image::ImageFormat::Gif {
        // GIF 编码器只收 RGB/RGBA。灰度页转成 RGB 再写，解回来 r==g==b，灰度取值不变。
        image
            .to_rgb8()
            .write_to(&mut bytes, format)
            .expect("编 GIF 夹具页");
    } else {
        image.write_to(&mut bytes, format).expect("编夹具页");
    }
    bytes.into_inner()
}

/// 一页的判定。
///
/// 两种页没有判定：彩色分支上的（那条路径不量化，ADR 0005 决定第 4 条），
/// 以及失败页（它根本没解出来，12 号票）。取它就是用例写错了：
/// 要么点错了页，要么该断言的是 `verdict()` 为空。
pub fn verdict(page: &tonefit::PageReport) -> tonefit::Verdict {
    page.verdict().unwrap_or_else(|| {
        panic!(
            "{} 没有判定：它要么走的彩色分支，要么是失败页",
            page.source.display()
        )
    })
}

/// 解出的 PNG，供测试直接看编码结果而不经被测代码。
///
/// `color_type` 与 `bit_depth` 是文件里**写着**的那一对；`pixels` 一律摊回 8 位灰度，
/// 好让断言只谈灰度取值，不必跟着颜色类型分叉。
pub struct DecodedPng {
    pub size: Size,
    pub color_type: png::ColorType,
    pub bit_depth: png::BitDepth,
    pub pixels: Vec<u8>,
}

impl DecodedPng {
    pub fn pixel(&self, x: u32, y: u32) -> u8 {
        self.pixels[(y * self.size.width + x) as usize]
    }
}

/// 用 `png` crate 直接读回文件，绕开被测的编码路径。
pub fn read_png(path: &Path) -> DecodedPng {
    read_png_bytes(&fs::read(path).expect("读输出 PNG"))
}

/// 同上，直接从一页 PNG 的字节里读。归档卷的页没有文件系统路径，成员字节走这里——
/// 与 [`read_png_text`] 和 [`png_text`] 是同一种分工，读的是同一批字节。
pub fn read_png_bytes(bytes: &[u8]) -> DecodedPng {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    let header = decoder.read_header_info().expect("读 PNG 头").clone();
    // EXPAND 把低位深灰度按满量程摊回 8 位，把调色板摊成 RGB8。
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("读 PNG 信息");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
    let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
    pixels.truncate(info.buffer_size());
    if header.color_type == png::ColorType::Indexed {
        // 色板项恒是三个相等的分量（见被测的 `encode`），取一个就是灰度取值。
        pixels = pixels
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect();
    }
    DecodedPng {
        size: Size::new(info.width, info.height),
        color_type: header.color_type,
        bit_depth: header.bit_depth,
        pixels,
    }
}

/// 解出的彩色 PNG，供测试直接看彩色分支的编码结果。
///
/// 与 [`DecodedPng`] 分开：那一个把像素摊回灰度，专为「断言只谈灰度取值」；
/// 彩色这一侧要断言的恰恰是三个分量各是多少。
pub struct DecodedColorPng {
    pub size: Size,
    pub color_type: png::ColorType,
    pub bit_depth: png::BitDepth,
    /// 每像素三字节，RGB。调色板已摊开。
    pub pixels: Vec<u8>,
}

impl DecodedColorPng {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let start = ((y * self.size.width + x) * 3) as usize;
        [
            self.pixels[start],
            self.pixels[start + 1],
            self.pixels[start + 2],
        ]
    }
}

/// 用 `png` crate 直接读回一个彩色 PNG，绕开被测的编码路径。
pub fn read_color_png(path: &Path) -> DecodedColorPng {
    read_color_png_bytes(&fs::read(path).expect("读输出 PNG"))
}

/// 同上，直接从一页 PNG 的字节里读。归档卷的页没有文件系统路径，成员字节走这里——
/// 与 [`read_png_bytes`] 是同一种分工。
pub fn read_color_png_bytes(bytes: &[u8]) -> DecodedColorPng {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    let header = decoder.read_header_info().expect("读 PNG 头").clone();
    // EXPAND 把调色板摊成 RGB8。
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("读 PNG 信息");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
    let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
    pixels.truncate(info.buffer_size());
    DecodedColorPng {
        size: Size::new(info.width, info.height),
        color_type: header.color_type,
        bit_depth: header.bit_depth,
        pixels,
    }
}

/// PNG 头里写着的每像素比特数。`png::BitDepth` 的判别值就是比特数本身。
pub fn written_bits(depth: png::BitDepth) -> u32 {
    u32::from(depth as u8)
}

/// 一个目录**顶层**有哪些名字，按名字排序。目录不递归进去，就问这一层。
///
/// 与 [`directory_members`] 分工：那一个铺平整棵树、答「这个容器里装着什么」，
/// 这一个只看一层、答「这个目录下此刻摆着哪几样」。**半成品因此看得见**——
/// 输出根下那格 `<卷名>.partial` 在这个清单里，而它在铺平的成员清单里认不出来。
/// 断言「中止之后输出根干净」「残留的临时容器不在了」用的都是它。
///
/// 根还没建出来就是「里面什么都没有」：还没轮到输出落盘的用例正要这个答案。
pub fn names_in(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| {
            entry
                .expect("列目录项")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// 一个目录容器里的成员名清单，按名字排序，分隔符归一成 `/`。
///
/// 归档那一侧的清单按**写入顺序**（见 `container.rs` 的 `member_names`），目录没有顺序，
/// 只有排序才比得出「输出里有什么」。断言「输出里只剩本趟的产物」用它。
pub fn directory_members(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|entry| !missing_root(entry))
        .map(|entry| entry.expect("遍历目录"))
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| relative_name(root, entry.path()))
        .collect();
    names.sort();
    names
}

/// 遍历的头一条就是「这个根不在」吗。
///
/// 目录还没建出来就是「里面什么都没有」，问「此刻输出里有什么」的用例正要这个答案。
/// **只放过这一种**：遍历途中的错误照旧当场炸（与 [`fingerprint`] 同一个口径），
/// 一律吞掉的话，一次 IO 抖动就能让「输出里只剩本趟的产物」假性通过。
fn missing_root(entry: &walkdir::Result<walkdir::DirEntry>) -> bool {
    let Err(error) = entry else { return false };
    error.depth() == 0
        && error
            .io_error()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
}

/// 目录内容的指纹：相对路径 + 每个文件的字节哈希。用来断言源目录未被改动。
pub fn fingerprint(root: &Path) -> Vec<(String, String)> {
    let mut entries: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.expect("遍历目录"))
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let relative = e
                .path()
                .strip_prefix(root)
                .expect("相对路径")
                .to_string_lossy()
                .into_owned();
            let bytes = fs::read(e.path()).expect("读文件");
            (relative, blake3::hash(&bytes).to_hex().to_string())
        })
        .collect();
    entries.sort();
    entries
}

/// 高频纹理页：`base` 上下各 `amplitude` 的逐像素交替。
///
/// 局部均值恒为 `base`，高频能量却拉满——判据的掩蔽加权该按后者放宽。
/// 取值不触顶不触底，加一个偏移上去不会被截断，加权方向因此可以单独测。
pub fn fine_texture(size: Size, base: u8, amplitude: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        Luma([if (x + y) % 2 == 0 {
            base.saturating_add(amplitude)
        } else {
            base.saturating_sub(amplitude)
        }])
    }))
}

/// 把夹具页转成判据吃的 8 位灰度缓冲。夹具造的都是灰度页，取 luma 即可。
pub fn gray_image(image: &DynamicImage) -> GrayImage {
    let luma = image.to_luma8();
    GrayImage::new(Size::new(luma.width(), luma.height()), luma.into_raw())
}

/// 一页输出 PNG 里的 tEXt 记录：关键字到取值，按文件里的顺序。
///
/// 用 `png` 直接读，绕开被测的写入路径。归档卷的页没有文件系统路径，成员字节走
/// [`png_text`]——两者读的是同一批字节。
pub fn read_png_text(path: &Path) -> Vec<(String, String)> {
    png_text(&fs::read(path).expect("读输出 PNG"))
}

/// 同上，直接从一页 PNG 的字节里读。
pub fn png_text(bytes: &[u8]) -> Vec<(String, String)> {
    let reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .expect("读 PNG 信息");
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .map(|chunk| (chunk.keyword.clone(), chunk.text.clone()))
        .collect()
}

/// 一页 tEXt 记录里某个关键字的取值。没有这个关键字就是 `None`。
pub fn png_field(text: &[(String, String)], keyword: &str) -> Option<String> {
    text.iter()
        .find(|(listed, _)| listed == keyword)
        .map(|(_, value)| value.clone())
}

/// 一个成员相对某个容器根的名字，分隔符归一成 `/`。
///
/// 归一是因为**这个名字要被断言**：路径分隔符随平台而变，而黄金回归的快照要在哪台机器上
/// 都是同一份（15 号票）。`sink` 往归档里写成员名时用的也是这个算法，归档那一侧因此按它取得回来。
pub fn relative_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 一个卷的卷级判定，写成一行给人读的话。
///
/// 用词取自 `CONTEXT.md`：上包络定出的那一档叫**基准档**，站在分位秩上的那一页叫**驱动页**，
/// 因迟滞升上去的那些页叫**迟滞升档**。`Envelope` 自己的 `Display` 另有一份，那一份还带着
/// 「四者均未标定」那句注脚——快照要的是钉死的一行，注脚每趟都一样，摆进去只是噪声。
///
/// 黄金回归与真实素材冒烟共用这一份（15 号票）：两处说的是同一件事，各写一份迟早会走散。
pub fn volume_verdict(volume: &tonefit::VolumeReport) -> String {
    match volume.verdict {
        Some(tonefit::VolumeVerdict::Envelope(envelope)) => format!(
            "基准档 {} · 驱动页 {} · 主体 {} 页 · 离群 {} 页 · 迟滞升档 {} 页",
            envelope.base,
            page_at(volume, envelope.driver),
            envelope.body_pages,
            envelope.outlier_pages,
            envelope.raised_pages,
        ),
        Some(tonefit::VolumeVerdict::Override(candidate)) => format!("覆盖 {candidate}"),
        Some(tonefit::VolumeVerdict::PerPage) => "逐页".to_owned(),
        Some(tonefit::VolumeVerdict::Skipped { page_count }) => {
            // 数的是**输出**页：跳过说的是「上一趟写在那儿的那些页一张都没重做」。
            // 源页数是另一个数（`VolumeReport::source_pages`），一个源页产出一到多张。
            format!("跳过 · 输出 {page_count} 页")
        }
        None => "无 · 这一卷一页都没有".to_owned(),
    }
}

/// 卷内第 `index` 页在卷里的名字。驱动页靠它指人。
pub fn page_at(volume: &tonefit::VolumeReport, index: usize) -> String {
    volume
        .pages
        .get(index)
        .map(|page| relative_name(&volume.volume, &page.source))
        .unwrap_or_else(|| format!("第 {index} 页"))
}
