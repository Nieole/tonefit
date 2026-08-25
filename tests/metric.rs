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

#[test]
fn a_page_of_white_around_the_damage_does_not_dilute_it() {
    let patch = Size::new(128, 128);
    let panel = fixtures::baseline_profile().panel();
    // 同一块灰调补丁，一次占满整页，一次泡在十六倍的留白里。
    let cramped = Reference::new(panel, page_with_tone_patch(patch, patch));
    let roomy = Reference::new(panel, page_with_tone_patch(Size::new(512, 512), patch));

    let cramped_score = score(
        &cramped,
        &quantize(cramped.image(), fixtures::plain(BitDepth::One)),
    );
    let roomy_score = score(
        &roomy,
        &quantize(roomy.image(), fixtures::plain(BitDepth::One)),
    );
    assert!(
        (roomy_score.value() - cramped_score.value()).abs() <= cramped_score.value() * 0.05,
        "留白把判据从 {cramped_score} 稀释到了 {roomy_score}"
    );

    // 换成全页聚合就会被稀释：同一块补丁，逐像素度量在大页上掉到约四分之一。
    let cramped_pixelwise = pixelwise_rmse(
        cramped.image(),
        &quantize(cramped.image(), fixtures::plain(BitDepth::One)),
    );
    let roomy_pixelwise = pixelwise_rmse(
        roomy.image(),
        &quantize(roomy.image(), fixtures::plain(BitDepth::One)),
    );
    assert!(
        cramped_pixelwise > roomy_pixelwise * 3.5,
        "夹具不对：全页聚合本该被留白稀释，{cramped_pixelwise:.2} 对 {roomy_pixelwise:.2}"
    );
}

/// 一页留白，左上角放一块 `patch` 大的灰调补丁——低位深下唯一会崩的就是这块。
fn page_with_tone_patch(size: Size, patch: Size) -> GrayImage {
    let mut pixels = vec![255u8; (size.width * size.height) as usize];
    let last = (patch.height - 1).max(1);
    for y in 0..patch.height.min(size.height) {
        for x in 0..patch.width.min(size.width) {
            pixels[(y * size.width + x) as usize] = (y * 255 / last) as u8;
        }
    }
    GrayImage::new(size, pixels)
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
