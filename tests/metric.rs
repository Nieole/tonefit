//! `score(参照, 候选) -> Score` 这个 seam 上的性质测试（ADR 0002）。
//!
//! 断言的是判据的性质，不是具体数值——核尺寸、掩蔽加权的形状都还没标定，数值会动，
//! 而这几条性质正是判据存在的理由，动了就是判据错了。

mod fixtures;

use tonefit::{BitDepth, Candidate, Dither, GrayImage, Reference, Size, quantize, score};

/// 同一档位深上抖过的那个候选。不抖动的那个在 `fixtures::plain`。
const fn dithered(bit_depth: BitDepth) -> Candidate {
    Candidate::new(bit_depth, Dither::FloydSteinberg)
}

/// 性质测试用的页尺寸。判据只吃像素与面板 PPI，不要求尺寸恰好是目标尺寸；
/// 取得比目标尺寸小是为了让这一组用例跑得快，分块数（20×26）仍足够 p99 有意义。
const PAGE: Size = Size::new(640, 832);

#[test]
fn on_a_gradient_the_dithered_candidate_beats_the_undithered_one() {
    let reference = baseline_reference(fixtures::gradient(PAGE));
    let plain = quantize(reference.image(), fixtures::plain(BitDepth::One));
    let dithered = quantize(reference.image(), dithered(BitDepth::One));

    let plain_score = score(&reference, &plain);
    let dithered_score = score(&reference, &dithered);
    assert!(
        dithered_score < plain_score,
        "抖动候选 {dithered_score} 没有赢过不抖动的 {plain_score}"
    );

    // 同一对候选上，逐像素度量给出相反的排序。这正是判据必须低通的理由：
    // 抖动用高频误差换低频保真，逐像素度量只看得见前者（ADR 0002、measurements 的《抖动》）。
    let plain_pixelwise = pixelwise_rmse(reference.image(), &plain);
    let dithered_pixelwise = pixelwise_rmse(reference.image(), &dithered);
    assert!(
        dithered_pixelwise > plain_pixelwise,
        "逐像素度量本该反过来：抖动 {dithered_pixelwise:.2} 对不抖动 {plain_pixelwise:.2}"
    );
}

#[test]
fn a_known_offset_reads_back_as_that_many_gray_levels() {
    // 纯色页整页偏一个手算得出的量，判据读出的就该是那个量——量的单位是 8 位灰度级。
    // 200 在 1bit 的格点上落到 255（偏 55），在 2bit 的 {0,85,170,255} 上落到 170（偏 30）。
    let reference = baseline_reference(fixtures::solid(PAGE, 200));

    let one_bit = score(
        &reference,
        &quantize(reference.image(), fixtures::plain(BitDepth::One)),
    );
    let two_bit = score(
        &reference,
        &quantize(reference.image(), fixtures::plain(BitDepth::Two)),
    );

    assert!(
        (one_bit.value() - 55.0).abs() < 0.001,
        "1bit 读成了 {one_bit}"
    );
    assert!(
        (two_bit.value() - 30.0).abs() < 0.001,
        "2bit 读成了 {two_bit}"
    );
}

/// 基准设备的面板上的参照。
fn baseline_reference(page: image::DynamicImage) -> Reference {
    Reference::new(
        fixtures::baseline_profile().panel(),
        fixtures::gray_image(&page),
    )
}

/// 逐像素 RMSE：判据要压过的那个度量，测试自己算，不走被测代码。
fn pixelwise_rmse(reference: &GrayImage, candidate: &GrayImage) -> f64 {
    let sum: f64 = reference
        .pixels()
        .iter()
        .zip(candidate.pixels())
        .map(|(&a, &b)| {
            let difference = f64::from(a) - f64::from(b);
            difference * difference
        })
        .sum();
    (sum / reference.pixels().len() as f64).sqrt()
}

#[test]
fn the_error_never_grows_when_the_bit_depth_does() {
    let pages = [
        ("渐变", fixtures::gradient(PAGE)),
        ("纯色", fixtures::solid(PAGE, 200)),
        ("高频纹理", fixtures::fine_texture(PAGE, 128, 40)),
    ];

    for (name, page) in pages {
        let reference = baseline_reference(page);
        let scores: Vec<_> = BitDepth::ALL
            .iter()
            .map(|&depth| {
                (
                    depth,
                    score(
                        &reference,
                        &quantize(reference.image(), fixtures::plain(depth)),
                    ),
                )
            })
            .collect();

        for pair in scores.windows(2) {
            let ((coarse, coarse_score), (fine, fine_score)) = (pair[0], pair[1]);
            assert!(
                fine_score <= coarse_score,
                "{name}页上 {fine} 的 {fine_score} 反而差过 {coarse} 的 {coarse_score}"
            );
        }
        // 参照本身未经目标位深量化：8bit 的格点就是工作精度，误差恒为零。
        assert_eq!(scores[3].1.value(), 0.0, "{name}页的 8bit 不是零误差");
    }
}

/// 大页那一侧取 [`fixtures::panel_sized`]，不拿小页挑软柿子。
///
/// 这条性质当初是在 512×512 上量的（16/256 块 = 6.25%，恰好在 p99 的 1% 门限之上），
/// 换成实际输出尺寸当场失败——2120 块上 p99 只圈得住最差的 22 块。
/// **测试在性质成立的那一侧挑了尺寸**，这才是那次缺陷真正的教训
/// （ADR 0002 的《第 3 条为什么改过》）。
#[test]
fn a_page_of_white_around_the_damage_does_not_dilute_it() {
    // 2K 块：够 K 圈住，又不到 p99 在输出尺寸上圈住的 22 块。
    let patch = patch_of(tonefit::aggregation().tail_tiles, 2);
    let panel = fixtures::baseline_profile().panel();
    // 同一块灰调补丁，一次占满整页，一次泡在一百多倍的留白里。
    let cramped = Reference::new(panel, tone_patch_page(patch, patch));
    let roomy = Reference::new(panel, tone_patch_page(fixtures::panel_sized(), patch));

    let cramped_score = score(
        &cramped,
        &quantize(cramped.image(), fixtures::plain(BitDepth::One)),
    );
    let roomy_score = score(
        &roomy,
        &quantize(roomy.image(), fixtures::plain(BitDepth::One)),
    );
    // 读数几乎不变，且大页那一侧不高过小页——留白只会往下拉，不会凭空添出误差来。
    //
    // 不主张两边**相等**：两张页取的不是同一个秩。局促页上分位只圈住最差的一块，
    // 大页上 K 说了算、取的是第 K 差的那一块，而那一块贴着补丁的边——
    // 低通在那里掺进了留白，掩蔽加权也因边界的活动度而收了手。两样都是判据照定义在做事。
    // 留下的余量按量的：K 从 4 到 20 落差都是 1.6%，一成的带子稳稳罩得住，
    // 而稀释一露头就是量级塌陷（同一份夹具上，聚合退回只取上分位读的是 0.000）。
    assert!(
        roomy_score <= cramped_score && roomy_score.value() >= cramped_score.value() * 0.9,
        "留白把判据从 {cramped_score} 稀释到了 {roomy_score}"
    );

    // 换成全页聚合就会被稀释：同一块补丁在实际输出尺寸上只占千分之几，
    // 逐像素度量按面积比开方掉下去，掉的是一个量级。倍数写得比实际宽松，
    // 因为补丁尺寸跟着 K 走——这一条要证的是「夹具确实摊薄了」，不是那个倍数本身。
    let cramped_pixelwise = pixelwise_rmse(
        cramped.image(),
        &quantize(cramped.image(), fixtures::plain(BitDepth::One)),
    );
    let roomy_pixelwise = pixelwise_rmse(
        roomy.image(),
        &quantize(roomy.image(), fixtures::plain(BitDepth::One)),
    );
    assert!(
        cramped_pixelwise > roomy_pixelwise * 5.0,
        "夹具不对：全页聚合本该被留白稀释，{cramped_pixelwise:.2} 对 {roomy_pixelwise:.2}"
    );
}

/// 一块 `wide × tall` 块大的补丁。判据的分块聚合数的是块，夹具因此也按块说话。
///
/// 块数按 K 推出而不抄下当前那个数字，两头都不能碰：要**够 K**，否则大页那一侧读回 0，
/// 量的就成了别的事；又不能大到 p99 自己都圈得住，否则退回只取上分位也照样绿
/// （2120 块上那是最差的 22 块）。K 换个值，用它的几条用例一行都不用改。
fn patch_of(wide: usize, tall: usize) -> Size {
    let shape = tonefit::aggregation();
    Size::new(shape.tile * wide as u32, shape.tile * tall as u32)
}

/// 留白里一块灰调补丁的那种页，转成判据吃的灰度缓冲。页本身由 [`fixtures::tone_patch`] 造，
/// 黄金回归的 `local-damage` 用的是同一份——两处量的必须是同一种页。
fn tone_patch_page(size: Size, patch: Size) -> GrayImage {
    fixtures::gray_image(&fixtures::tone_patch(size, patch))
}

#[test]
fn the_same_error_counts_for_more_in_a_flat_area_than_in_a_textured_one() {
    let panel = fixtures::baseline_profile().panel();
    let flat = Reference::new(panel, fixtures::gray_image(&fixtures::solid(PAGE, 128)));
    let textured = Reference::new(
        panel,
        fixtures::gray_image(&fixtures::fine_texture(PAGE, 128, 40)),
    );
    // 同一个偏移加在两张参照上。低通是线性的，两边的局部均值误差因此逐像素相等，
    // 判据只剩掩蔽加权这一项还能不同。
    let lifted = |reference: &Reference| {
        GrayImage::new(
            reference.size(),
            reference.image().pixels().iter().map(|&v| v + 8).collect(),
        )
    };

    let flat_score = score(&flat, &lifted(&flat));
    let textured_score = score(&textured, &lifted(&textured));

    // 平坦低对比区不打折：8 级偏移就算 8 级误差。
    assert!(
        (flat_score.value() - 8.0).abs() < 0.01,
        "平坦区的 8 级偏移算成了 {flat_score}"
    );
    assert!(
        textured_score < flat_score,
        "高频纹理区没有放宽：{textured_score} 对平坦区的 {flat_score}"
    );
}

#[test]
fn two_panels_of_the_same_resolution_but_different_ppi_do_not_share_a_metric() {
    // 面板表里这两块的分辨率一模一样，只有 PPI 不同（见 profile 的面板常量）。
    let denser = fixtures::profile("kobo-aura-one").panel();
    let coarser = fixtures::profile("kobo-elipsa").panel();
    assert_eq!(denser.resolution, coarser.resolution, "夹具挑错了面板");
    assert!(denser.ppi > coarser.ppi);

    let page = fixtures::gray_image(&fixtures::gradient(PAGE));
    let dithered = quantize(&page, dithered(BitDepth::One));
    let on = |panel| {
        let reference = Reference::new(panel, page.clone());
        score(&reference, &dithered)
    };

    // 核由 PPI 推出：面板越密，同一个视角盖住的像素越多，抖动的高频被抹得越干净。
    assert!(
        on(denser) < on(coarser),
        "换了 PPI 判据没变：{} 对 {}，低通核像是写死的",
        on(denser),
        on(coarser)
    );
}

/// 一块绝对尺寸的损伤，在该 profile 的**实际输出尺寸**上照样读得出来。
///
/// 分块上分位是**比例**，而值得报警的损伤是**绝对面积**：p99 在 1264×1680 上只圈得住
/// 最差的 22 块，覆盖不到那么多块的损伤被整块丢掉，三个候选位深读数全是 0，
/// 那一页于是判成最低档——而那一小块正是唯一会崩的地方（ADR 0002 的《第 3 条为什么改过》）。
#[test]
fn damage_covering_the_tail_width_reads_back_at_every_page_size() {
    let shape = tonefit::aggregation();
    // 正好盖住 K 块的一条：宽 K 块、高一块，块边界对齐——保证的下边界就在这里。
    let patch = patch_of(shape.tail_tiles, 1);

    // 三个尺寸各压一端：补丁自己那么大时块数少到分位退化成最差的一块；
    // 512×512 铺出 256 块，p99 圈住最差的 3 块，分位比 K 严、分位说了算；
    // 实际输出尺寸铺出 2120 块，p99 圈住 22 块，K 说了算——只有这一端从前读回 0。
    for page in [patch, Size::new(512, 512), fixtures::panel_sized()] {
        let reference = Reference::new(
            fixtures::baseline_profile().panel(),
            tone_patch_page(page, patch),
        );
        let score = score(
            &reference,
            &quantize(reference.image(), fixtures::plain(BitDepth::One)),
        );
        assert!(
            score.value() > 0.0,
            "{page} 的页上，盖住 {} 块的损伤读成了 {score}",
            shape.tail_tiles
        );
    }
}
