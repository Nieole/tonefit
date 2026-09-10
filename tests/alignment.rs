//! `align_white(图, 上限) -> (图, 这一页做了什么)` 这个 seam 上的性质测试（纸白对齐批 01 号票）。
//!
//! 断言的是**性质，不是数值**——上限那个数还没标定，会动；而这几条性质正是这一步存在的理由，
//! 动了就是它错了。写法照 `tests/metric.rs`，那一篇的模块文档把这条理由写清楚了。
//!
//! 三条守卫各占一条：离格量为 0 不动、超过上限不动、量不出纸白不动，三条都要求**整页逐像素不变**。
//! 末一条钉的是整件事的目的——对齐之后，同一页在 `2bit+FS` 上的平坦白底一个点都不撒。

mod fixtures;

use tonefit::{
    BitDepth, Candidate, Dither, GrayImage, Size, WhiteAlignLimit, WhiteAlignment, align_white,
    quantize,
};

/// 性质测试用的页尺寸。对齐只吃像素，不要求尺寸恰好是目标尺寸；
/// 取得比目标尺寸小是为了让这一组跑得快，纸白那一片的平坦像素仍远在「量不出纸白」那道线之上。
const PAGE: Size = Size::new(320, 400);

/// 够得着夹具那 2 级离格量的上限。取值与理由只有 [`fixtures::ALIGNING_LIMIT`] 一处出处。
const LIMIT: WhiteAlignLimit = fixtures::ALIGNING_LIMIT;

/// 一张纸白落在 `paper` 上的页，转成对齐吃的灰度缓冲。
fn page(paper: u8) -> GrayImage {
    fixtures::gray_image(&fixtures::page_with_paper_white(PAGE, paper))
}

/// 满版无纸边的一页：平坦像素少到量不出纸白。
fn full_bleed() -> GrayImage {
    fixtures::gray_image(&fixtures::full_bleed_page_without_paper(PAGE))
}

/// 对齐之后这一页的纸白**恰好是 255**。
///
/// 「纸白是多少」不从外面量——再对齐一次就是答案：已经在格点上的页，守卫那一条会说
/// `OnTheGrid { paper_white: 255 }`。断言因此仍然只用这个 seam 自己看得见的事实。
#[test]
fn an_off_grid_page_lands_its_paper_white_exactly_on_255() {
    let (aligned, what) = align_white(page(fixtures::OFF_GRID_PAPER_WHITE), LIMIT);

    assert_eq!(
        what,
        WhiteAlignment::Aligned {
            paper_white: fixtures::OFF_GRID_PAPER_WHITE
        },
        "离格 2 级、上限 4 级的页没有被对齐"
    );
    let (_, again) = align_white(aligned, LIMIT);
    assert_eq!(
        again,
        WhiteAlignment::OnTheGrid { paper_white: 255 },
        "对齐之后这一页的纸白不是 255"
    );
}

/// **只有 `[纸白, 255]` 那一段变了**：低于纸白的取值一个都没动。
///
/// 这一条挡的是「线性拉伸」与「整幅平移」那两种手段——它们都会改动整条色调曲线，
/// 而平移还会把纯黑推离格点、在另一头制造同一个病。
#[test]
fn nothing_below_the_paper_white_moved() {
    let paper = fixtures::OFF_GRID_PAPER_WHITE;
    let before = page(paper);
    let (after, _) = align_white(before.clone(), LIMIT);

    assert_eq!(before.size(), after.size());
    let moved = before
        .pixels()
        .iter()
        .zip(after.pixels())
        .filter(|&(&was, &now)| was != now)
        .count();
    assert!(moved > 0, "夹具没咬住：这一页一个像素都没动");
    fixtures::assert_pixels(
        &fixtures::clamped_to_white(before.pixels(), paper),
        after.pixels(),
    );
}

/// **守卫一**：离格量为 0 的页整页逐像素不变。全语料 57% 的页落在这里。
#[test]
fn a_page_already_on_the_grid_keeps_every_pixel() {
    let before = page(255);
    let (after, what) = align_white(before.clone(), LIMIT);

    assert_eq!(what, WhiteAlignment::OnTheGrid { paper_white: 255 });
    assert_same(&before, &after);
}

/// **守卫二**：离格量超过上限的页整页逐像素不变。代价超过这一趟愿意付的，就一个像素都不动。
#[test]
fn a_page_whose_offset_is_over_the_limit_keeps_every_pixel() {
    let before = page(fixtures::OFF_GRID_PAPER_WHITE);
    let (after, what) = align_white(before.clone(), WhiteAlignLimit::new(1));

    assert_eq!(
        what,
        WhiteAlignment::OverTheLimit {
            paper_white: fixtures::OFF_GRID_PAPER_WHITE
        },
        "离格 2 级、上限 1 级，这一页本该被守卫拦下"
    );
    assert_same(&before, &after);
}

/// **守卫三**：量不出纸白的页整页逐像素不变。
///
/// 满版画集没有大片平坦白底，**不硬猜一个纸白**——猜出来的那个值会把整页改坏
/// （spec 的 story 6）。业界五个工具在这一情形上一律是放弃。
#[test]
fn a_full_bleed_page_without_paper_keeps_every_pixel() {
    let before = full_bleed();
    let (after, what) = align_white(before.clone(), LIMIT);

    assert_eq!(what, WhiteAlignment::NoPaperWhite);
    assert_same(&before, &after);
}

/// **上限取 0 就是关闭**：任何输入都逐像素不变，不必另设一个开关。
///
/// 三张页各走一遍——离格的、在格点上的、量不出纸白的。只测第三张的话，
/// 「关闭」与「这一页本来就治不了」分不开。
#[test]
fn a_limit_of_zero_leaves_every_input_untouched() {
    let pages = [
        ("离格的页", page(fixtures::OFF_GRID_PAPER_WHITE)),
        ("在格点上的页", page(255)),
        ("满版无纸边的页", full_bleed()),
    ];

    for (what, before) in pages {
        let (after, outcome) = align_white(before.clone(), WhiteAlignLimit::default());
        assert_eq!(
            outcome,
            WhiteAlignment::Off,
            "{what}：上限 0 却不是「没开」"
        );
        fixtures::assert_pixels(before.pixels(), after.pixels());
    }
}

/// **整件事的目的**：对齐之后，那一页的纸白**恰好是 255**，而**同一页**在 `2bit+FS` 上的
/// 平坦白底**一个点都不撒**。票面那一条要的是同一页两句都成立，这里因此只用一张页。
///
/// 页取纯 [`fixtures::OFF_GRID_PAPER_WHITE`] 的一整片白底。**页上不放别的内容是有意的**：
/// 误差扩散顺着扫描线走，别处的误差会漏进白底，读出来的就不再是「白底自己撒不撒点」。
///
/// 对齐前那一撒**先断言得看得见**：看不见就说明夹具没咬住，后半句「一个点都不撒」也就没意义。
#[test]
fn an_aligned_page_reads_255_and_takes_not_one_dithered_dot() {
    let two_bit_fs = Candidate::new(BitDepth::Two, Dither::FloydSteinberg);
    let flat = fixtures::gray_image(&fixtures::solid(PAGE, fixtures::OFF_GRID_PAPER_WHITE));

    let sprinkled = dots(&quantize(&flat, two_bit_fs));
    assert!(
        sprinkled > 0,
        "夹具没咬住：纸白 {} 的白底在 2bit+FS 上本该撒点",
        fixtures::OFF_GRID_PAPER_WHITE
    );

    let (aligned, _) = align_white(flat, LIMIT);

    // 这一页的纸白现在恰好是 255——再对齐一次，守卫那一条自己说得出来。
    let (aligned, again) = align_white(aligned, LIMIT);
    assert_eq!(
        again,
        WhiteAlignment::OnTheGrid { paper_white: 255 },
        "对齐之后这一页的纸白不是 255"
    );
    assert_eq!(
        dots(&quantize(&aligned, two_bit_fs)),
        0,
        "对齐之后白底上还撒着点"
    );
}

/// 一张量化结果上，没落在纯白上的像素有多少个。
fn dots(quantized: &GrayImage) -> usize {
    quantized
        .pixels()
        .iter()
        .filter(|&&value| value != u8::MAX)
        .count()
}

/// 整页逐像素不变。三条守卫要的都是这一句。
fn assert_same(before: &GrayImage, after: &GrayImage) {
    assert_eq!(before.size(), after.size(), "尺寸变了");
    fixtures::assert_pixels(before.pixels(), after.pixels());
}
