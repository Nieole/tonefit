//! `run(Request) -> Report` 这个 seam 上的行为测试。
//!
//! 只断言外部可见的事实：`Report` 里的内容、写出的文件有什么性质、源目录有没有被动过。

mod fixtures;

use fixtures::{Workspace, run_volume};
use tonefit::{
    BitDepth, CacheBudget, Candidate, Dither, Filter, GeometryGate, Mode, PageColor, Reason,
    Request, Size, VolumeVerdict,
};

#[test]
fn each_page_becomes_a_png_at_the_target_size_and_the_decided_bit_depth() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));

    let report = run_volume(&space, &volume);

    assert_eq!(report.volumes.len(), 1);
    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), 1);
    // 源正好是面板的两倍，fit-inside 后恰好落在面板上，没有取整歧义。
    assert_eq!(pages[0].size, Size::new(1264, 1680));

    let written = fixtures::read_png(&pages[0].output);
    assert_eq!(written.size, Size::new(1264, 1680));
    // 这一页贴住面板，几何门放行抖动：判据于是在 2bit+FS 上就够了，
    // 而同一档不抖动差得远——差多远看报告里这一页的判据曲线。
    assert_eq!(
        fixtures::verdict(&pages[0]).candidate,
        Candidate::new(BitDepth::Two, Dither::FloydSteinberg)
    );
    // 抖过的渐变页把 2bit 的四个格点铺满了，调色板买不到更窄的位宽，灰度于是胜出。
    assert_eq!(written.color_type, png::ColorType::Grayscale);
    assert_eq!(
        fixtures::written_bits(written.bit_depth),
        fixtures::verdict(&pages[0]).candidate.bit_depth.bits()
    );
}

/// 色带：平缓渐变被就近取整压成几条等值的宽带，带与带之间一步跨掉一个格距。
/// 抖动把那一步摊成高频误差，逐行均值于是平滑地爬上去（ADR 0007、measurements 的《抖动》）。
#[test]
fn a_gradient_written_at_a_low_bit_depth_comes_out_without_measurable_banding() {
    let dithered = Workspace::new();
    let volume = dithered.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));

    let report = run_volume(&dithered, &volume);

    let page = &report.volumes[0].pages[0];
    let depth = fixtures::verdict(page).candidate.bit_depth;
    assert_eq!(
        fixtures::verdict(page).candidate.dither,
        Dither::FloydSteinberg
    );
    assert!(depth <= BitDepth::Two, "判定落在 {depth}，色带那一档没测到");

    // 同一档位深、同一页，点名不抖动再跑一遍：差别只剩抖动这一项。
    let plain_space = Workspace::new();
    let plain_volume = plain_space.volume("volume-a");
    plain_volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
    let plain = tonefit::run(&Request {
        bit_depth: Some(depth),
        dither: Some(Dither::Off),
        ..fixtures::request(&plain_space, [plain_volume.path()])
    })
    .expect("处理应当成功");

    let with_dither = worst_banding_step(&fixtures::read_png(&page.output));
    let without = worst_banding_step(&fixtures::read_png(&plain.volumes[0].pages[0].output));

    // 不抖动时相邻两块的均值一步跨掉几十级——那一步就是看得见的色带（2bit 的格距是 85）。
    assert!(
        without > 40.0,
        "夹具不对：不抖动的 {depth} 本该出色带，最大跳变只有 {without:.2}"
    );
    assert!(
        with_dither < 10.0,
        "抖过之后局部均值仍有 {with_dither:.2} 级的跳变，色带没消掉"
    );
}

/// 相邻两块的均值之间的最大跳变，8 位灰度级。色带就是这个量。
///
/// 一块取 [`BANDING_BLOCK`] 行。**不能逐行量**：FS 把误差往下一行推，相邻两行的均值
/// 因此本来就在几十级上下摆——那正是抖动买下的高频，量它等于量这笔交换本身，
/// 而不是量色带。块高取低通核（4 px，见 ADR 0002）的两倍，摆动落在块内、色带落在块间。
fn worst_banding_step(written: &fixtures::DecodedPng) -> f64 {
    let stride = written.size.width as usize * BANDING_BLOCK;
    let means: Vec<f64> = written
        .pixels
        .chunks(stride)
        .map(|block| block.iter().map(|&level| f64::from(level)).sum::<f64>() / block.len() as f64)
        .collect();
    means
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f64::max)
}

/// 量色带的块高，行。
const BANDING_BLOCK: usize = 8;

#[test]
fn the_written_levels_all_sit_on_the_grid_of_the_decided_bit_depth() {
    // 写出去的取值只能落在判定那一档的格点上——判据比的就是这些格点与参照的差，
    // 文件里出现格点之外的取值，等于判据算的不是最终输出。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("01.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page(
        "02.png",
        &fixtures::color_page(fixtures::SMALLER_THAN_TARGET),
    );
    volume.page("03.png", &fixtures::solid(fixtures::DOUBLE_PANEL, 200));

    let report = run_volume(&space, &volume);

    for page in &report.volumes[0].pages {
        let depth = fixtures::verdict(page).candidate.bit_depth;
        let grid = tonefit::quantize(
            &tonefit::GrayImage::new(Size::new(256, 1), (0..=255u8).collect()),
            Candidate::new(depth, Dither::Off),
        );
        let written = fixtures::read_png(&page.output);
        for &level in &written.pixels {
            assert!(
                grid.pixels().contains(&level),
                "{} 判定 {depth}，却写出了格点外的 {level}",
                page.source.display()
            );
        }
    }
}

#[test]
fn a_page_with_few_levels_is_written_as_a_palette_narrower_than_its_verdict() {
    // 二值线稿页点名 4bit：格点有 16 个，页上只用得着 2 个。
    // 调色板于是把它装进 1 位，像素一个不动——判定说的是量化格点，
    // 文件里那个位宽是编码器接口以内的事（ADR 0004）。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::line_art(fixtures::SMALLER_THAN_TARGET);
    volume.page("001.png", &page);

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Four),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    assert_eq!(
        fixtures::verdict(&report.volumes[0].pages[0])
            .candidate
            .bit_depth,
        BitDepth::Four
    );
    assert_eq!(written.color_type, png::ColorType::Indexed);
    assert_eq!(written.bit_depth, png::BitDepth::One);
    assert_eq!(
        written.pixels.as_slice(),
        page.to_luma8().as_raw().as_slice()
    );
}

#[test]
fn pages_come_out_in_reading_order() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::line_art(fixtures::SMALLER_THAN_TARGET);
    // 故意让字典序（1、10、2）与阅读顺序（1、2、10）分道扬镳，章节目录同理。
    for name in ["10.png", "2.png", "1.png", "ch10/1.png", "ch2/1.png"] {
        volume.page(name, &page);
    }

    let report = run_volume(&space, &volume);

    let order: Vec<_> = report.volumes[0]
        .pages
        .iter()
        .map(|page| slash_path(page.source.strip_prefix(volume.path()).unwrap()))
        .collect();
    assert_eq!(
        order,
        ["1.png", "2.png", "10.png", "ch2/1.png", "ch10/1.png"]
    );
}

#[test]
fn every_supported_format_decodes() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::SMALLER_THAN_TARGET;
    let page = fixtures::gradient(size);
    let names = [
        "01.avif", "02.bmp", "03.gif", "04.jpg", "05.png", "06.tiff", "07.webp",
    ];
    for name in names {
        volume.page(name, &page);
    }

    // 量的是解码：把输出钉在 8bit，读回来的就是解码器给出的那张图。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), names.len());
    for (page, name) in pages.iter().zip(names) {
        assert!(page.source.ends_with(name), "{name} 没有被当成页读到");
        let written = fixtures::read_png(&page.output);
        assert_eq!(written.size, size, "{name}");
        assert_eq!(written.color_type, png::ColorType::Grayscale, "{name}");
        assert_eq!(written.bit_depth, png::BitDepth::Eight, "{name}");
        // 尺寸对而像素空白的解码器也能过尺寸断言，所以还要看内容：
        // 渐变页顶端近黑、底端近白，灰调级数不该塌。容差留给有损格式。
        let top = written.pixel(10, 2);
        let bottom = written.pixel(10, size.height - 3);
        let levels = distinct_levels(&written.pixels);
        assert!(top < 16, "{name} 顶端出成了 {top}");
        assert!(bottom > 239, "{name} 底端出成了 {bottom}");
        assert!(levels > 32, "{name} 只剩 {levels} 级灰调");
    }
}

#[test]
fn color_pages_go_through_the_oklab_lightness_channel() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 用不需要缩放的尺寸，像素与色带一一对应。
    let size = fixtures::SMALLER_THAN_TARGET;
    volume.page("001.png", &fixtures::color_page(size));

    // 量的是转灰：把输出钉在 8bit，色带落点才不被量化挪动。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    let band = |index| written.pixel(size.width / 2, fixtures::band_center_row(size, index));
    // 期望值的出处是 OKLab 原文给出的 L：蓝 0.452、红 0.628、绿 0.866，
    // 再按 sRGB 传输曲线编回 8bit。容差 ±1 只吸收取整。
    assert!((85..=87).contains(&band(0)), "纯蓝落在 {}", band(0));
    assert_eq!(band(1), 18, "灰带必须恒等通过");
    assert!((135..=137).contains(&band(2)), "纯红落在 {}", band(2));
    assert!((210..=212).contains(&band(3)), "纯绿落在 {}", band(3));
    assert_eq!(band(4), 255, "白");
    assert_eq!(band(5), 0, "黑");
    // 蓝带与灰带取自同一条 Rec.709 亮度（见夹具的 COLOR_BANDS），这里必须仍然可分。
    assert!(band(0) > band(1) + 50, "彩色与灰的对比不该塌掉");
}

/// 黑白 profile 下彩页转灰、走与其它页相同的灰度路径，但它仍然是一张彩页——
/// 报告分得清（ADR 0005 决定第 4 条；10 号票：`Report` 区分彩页与灰度页）。
#[test]
fn a_color_page_on_a_monochrome_profile_is_grayed_and_still_reported_as_color() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::color_page(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    let pages = &volume_report.pages;
    assert_eq!(pages[0].color(), Some(PageColor::Color), "彩页没被识别出来");
    assert_eq!(pages[1].color(), Some(PageColor::Gray));
    // 两页都走了灰度路径：都有判定，都进了灰度缓存，写出的都不是彩色 PNG。
    assert_eq!(
        volume_report.cache.pages, 2,
        "黑白 profile 下彩页也进灰度缓存"
    );
    for page in pages {
        assert!(
            page.verdict().is_some(),
            "{} 没有判定",
            page.source.display()
        );
        let written = fixtures::read_png(&page.output);
        assert_ne!(
            written.color_type,
            png::ColorType::Rgb,
            "黑白 profile 不留颜色"
        );
    }
}

/// 彩色 profile 下彩页走彩色分支：只做缩放，不量化、不进灰度缓存、没有判定，
/// 颜色原样留在输出里（ADR 0005 决定第 4 条）。同一卷里的灰度页照旧走灰度路径。
#[test]
fn a_color_page_on_a_color_profile_keeps_its_color_and_stays_out_of_the_gray_cache() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 不必缩放的尺寸：色带与像素一一对应，断言谈的就是源上那几个取值。
    let size = fixtures::SMALLER_THAN_TARGET;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::gradient(size));

    let report = fixtures::run_volume_with(&space, &volume, fixtures::profile("kobo-libra-colour"));

    let volume_report = &report.volumes[0];
    let pages = &volume_report.pages;
    assert_eq!(pages[0].color(), Some(PageColor::Color));
    // 彩色分支不量化：既没有判定，也没有判据曲线。
    assert_eq!(pages[0].verdict(), None, "彩色分支上不该有判定");
    assert!(pages[0].scores().is_empty(), "彩色分支上不该有判据曲线");
    // 不进灰度缓存：缓存里只剩那一张灰度页。
    assert_eq!(volume_report.cache.pages, 1, "彩页不该进灰度缓存");
    // 每页仍然只解码一次（ADR 0005）。
    assert_eq!(volume_report.decodes, 2);

    // 颜色留住了：色带一条不少地写在输出里。
    let written = fixtures::read_color_png(&pages[0].output);
    assert_eq!(written.size, size);
    for (index, band) in fixtures::COLOR_BANDS.iter().enumerate() {
        let row = fixtures::band_center_row(size, index);
        assert_eq!(
            written.pixel(size.width / 2, row),
            *band,
            "第 {index} 条色带"
        );
    }

    // 同一卷里的灰度页在彩色面板上照旧走灰度路径。
    assert_eq!(pages[1].color(), Some(PageColor::Gray));
    assert!(pages[1].verdict().is_some(), "灰度页该有判定");
    assert_eq!(
        fixtures::read_png(&pages[1].output).color_type,
        png::ColorType::Grayscale
    );
}

/// dry-run 下彩色分支也一个文件都不落盘，报告照出（spec 的 story 6）。
///
/// 彩色分支上没有判据可预告——那条路径不量化——所以这一遍连编码都省了。
/// 要预告的只有几何：目标尺寸与缩放照旧算得出来。
#[test]
fn a_dry_run_reports_the_color_branch_without_writing_anything() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::color_page(fixtures::TINY));
    volume.page("002.png", &fixtures::gradient(fixtures::TINY));

    let report = tonefit::run(&Request {
        profile: fixtures::profile(COLOR_DEVICE),
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let pages = &report.volumes[0].pages;
    assert_eq!(pages[0].color(), Some(PageColor::Color));
    assert_eq!(pages[0].verdict(), None, "彩色分支上不该有判定");
    assert_eq!(pages[0].size, fixtures::TINY, "彩页的目标尺寸照旧算出来");
    assert!(pages[1].verdict().is_some(), "灰度页照旧有判定");
    assert!(!space.out().exists(), "dry-run 落了盘");
}

/// 彩色分支上的页不参与几何门（ADR 0010）。
///
/// 门撑的是抖动与面板灰阶那道硬上界（ADR 0007、ADR 0003），两者只作用在灰度路径上：
/// 彩页既不量化也不抖动，它的几何事实对那两件事没有说话的资格。让它关掉整卷的抖动，
/// 就是让一条不受影响的路径去削掉另一条路径的收益。
///
/// 同一卷换到黑白 profile 上，那一页转灰、走灰度路径，它的几何这时就说了算——
/// 门是**分支的函数**，不是页的常量。
#[test]
fn a_color_page_smaller_than_the_panel_does_not_close_the_geometry_gate() {
    let build = |space: &Workspace| {
        let volume = space.volume("volume-a");
        // 源比目标小的彩页：一条边都贴不住面板。
        volume.page(
            "001.png",
            &fixtures::color_page(fixtures::SMALLER_THAN_TARGET),
        );
        // 正好两倍面板的灰度页：贴住，门在它这里是开的。
        volume.page("002.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
        volume
    };

    let color_space = Workspace::new();
    let color = fixtures::run_volume_with(
        &color_space,
        &build(&color_space),
        fixtures::profile(COLOR_DEVICE),
    );
    let mono_space = Workspace::new();
    let mono = fixtures::run_volume_with(
        &mono_space,
        &build(&mono_space),
        fixtures::baseline_profile(),
    );

    assert_eq!(
        color.volumes[0].gate,
        Some(GeometryGate::Holds),
        "彩色分支上的那一页把整卷的抖动关掉了"
    );
    assert_eq!(
        mono.volumes[0].gate,
        Some(GeometryGate::Broken { page: 0 }),
        "同一页转灰之后走灰度路径，它的几何这时说了算"
    );
}

/// 部分救回页不参与几何门（04 号票）：一张没解全的页不替另外那些页回答
/// 「输出会不会被下游再缩一次」。
///
/// 同一张页不截断时它的几何说了算——门是**页状态的函数**，不是页尺寸的常量。
/// 两趟并排跑，钉的正是这个差别：只断言截断那一趟门开着，夹具选错尺寸也照样通过。
#[test]
fn a_salvaged_page_smaller_than_the_panel_does_not_close_the_geometry_gate() {
    let small = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    let build = |space: &Workspace, bytes: Vec<u8>| {
        let volume = space.volume("volume-a");
        // 源比目标小的那一页：一条边都贴不住面板。
        volume.file("001.png", &bytes);
        // 正好两倍面板的页：贴住，门在它这里是开的。
        volume.page("002.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
        volume
    };

    let salvaged_space = Workspace::new();
    let salvaged = run_volume(
        &salvaged_space,
        &build(&salvaged_space, fixtures::truncated(&small)),
    );
    let whole_space = Workspace::new();
    let whole = run_volume(
        &whole_space,
        &build(&whole_space, fixtures::encode_image(&small, "png")),
    );

    // 夹具自证：两趟里那一页都是同一个贴不住面板的尺寸，差别只在它救回来没有。
    assert_eq!(
        salvaged.volumes[0].pages[0].size,
        fixtures::SMALLER_THAN_TARGET
    );
    assert!(
        salvaged.volumes[0].pages[0].salvage().is_some(),
        "夹具不对：那一页没被救回，这条用例就什么都没钉住"
    );
    assert_eq!(
        whole.volumes[0].pages[0].size,
        fixtures::SMALLER_THAN_TARGET
    );

    assert_eq!(
        salvaged.volumes[0].gate,
        Some(GeometryGate::Holds),
        "部分救回页把整卷的抖动关掉了"
    );
    assert_eq!(
        whole.volumes[0].gate,
        Some(GeometryGate::Broken { page: 0 }),
        "同一页不截断时它的几何该说了算"
    );
}

/// 一卷里的灰度页一页不剩地落在救回那一侧时，**两处都不摘**：摘一页是为了护着别人，
/// 而那时没有别人可护（04 号票）。
///
/// 几何门于是听它们的——两页都贴不住面板，门因此关上、整卷不抖动。这一条要是不成立，
/// 一整卷会被下游再缩一次的页会带着抖动写出去，正是 ADR 0007 拦的那件事。
/// 卷级那一层同理：主体不能空着，这两页于是照旧定得出一个基准档。
#[test]
fn a_volume_of_nothing_but_salvaged_pages_lets_them_speak_for_themselves() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file(
        "001.png",
        &fixtures::truncated(&fixtures::line_art(fixtures::TINY)),
    );
    volume.file(
        "002.png",
        &fixtures::truncated(&fixtures::line_art(fixtures::TINY)),
    );

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(
        reported.salvaged().count(),
        2,
        "夹具不对：两页都该是救回来的"
    );
    // 页比面板小得多，一条边都贴不住：没有完好页可护，它们自己的几何这时说了算。
    assert_eq!(reported.pages[0].size, fixtures::TINY);
    assert_eq!(reported.gate, Some(GeometryGate::Broken { page: 0 }));
    // 门关着，抖动因此整体关闭（ADR 0007）。
    let envelope = envelope_of(reported);
    assert_eq!(envelope.base.dither, Dither::Off);
    // 一页不剩地落在救回那一侧，上包络那一侧同样一页都不摘：两页都进主体。
    assert_eq!(envelope.body_pages, 2);
    for page in &reported.pages {
        assert_eq!(fixtures::verdict(page).reason, Reason::VolumeEnvelope);
    }
}

#[test]
fn transparent_areas_come_out_as_paper_white() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::SMALLER_THAN_TARGET;
    volume.page("001.png", &fixtures::page_with_transparency(size));

    let report = run_volume(&space, &volume);

    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    let row = size.height / 2;
    assert_eq!(written.pixel(size.width / 4, row), 0, "不透明的黑仍是黑");
    assert_eq!(
        written.pixel(size.width * 3 / 4, row),
        255,
        "透明区应当是纸白"
    );
}

#[test]
fn a_page_smaller_than_the_target_keeps_its_size_and_its_pixels() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    volume.page("001.png", &page);

    // 量的是几何与转灰：把输出钉在 8bit，「逐字节相同」才谈得成。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    assert_eq!(
        report.volumes[0].pages[0].size,
        fixtures::SMALLER_THAN_TARGET
    );
    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    assert_eq!(written.size, fixtures::SMALLER_THAN_TARGET);
    // 灰度源不被放大、也不经色彩换算，输出与源逐字节相同。
    assert_eq!(
        written.pixels.as_slice(),
        page.to_luma8().as_raw().as_slice()
    );
}

#[test]
fn the_target_size_is_the_page_fitted_inside_the_panel() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("01.png", &fixtures::line_art(fixtures::SPREAD));
    volume.page("02.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("03.png", &fixtures::gradient(fixtures::SMALLER_THAN_TARGET));

    let report = run_volume(&space, &volume);

    // 期望值手算：跨页 5056×1680 由宽边定夺，×0.25；B 类中位 1441×2048 由高边定夺，
    // ×1680/2048 得 1182.07 → 1182；800×1000 两边都小于面板，原样。
    let expected = [
        Size::new(1264, 420),
        Size::new(1182, 1680),
        Size::new(800, 1000),
    ];
    for (page, expected) in report.volumes[0].pages.iter().zip(expected) {
        assert_eq!(page.size, expected, "{}", page.source.display());
        assert_eq!(fixtures::read_png(&page.output).size, expected);
    }
}

#[test]
fn the_target_size_comes_from_the_profiles_panel() {
    // 同一页送进三个 profile：目标尺寸随各自的面板走，没有写死的面板。
    // 期望值手算：B 类中位页 1441×2048 三块面板上都由高边定夺，宽 = round(1441 × 面板高 ÷ 2048)。
    let cases = [
        ("kobo-libra-2", Size::new(1182, 1680)),
        ("kobo-clara-2e", Size::new(1019, 1448)),
        ("boox-note-air3", Size::new(1317, 1872)),
    ];

    for (device, expected) in cases {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));

        let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(device));

        let page = &report.volumes[0].pages[0];
        assert_eq!(page.size, expected, "{device}");
        assert_eq!(fixtures::read_png(&page.output).size, expected, "{device}");
    }
}

#[test]
fn the_report_gives_the_total_ratio_the_prescale_and_the_residual_of_every_page() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 期望值手算，面板 1264×1680。总缩放比 = 源高 ÷ 目标高：
    //   5056×1680 → 1264×420 ：比 4.000，整数比，预缩一步到位
    //   3160×4200 → 1264×1680：比 2.500，预缩 2 之后残差段还剩 1.250
    //   2528×3360 → 1264×1680：比 2.000，预缩 2，残差 1.000
    //   1441×2048 → 1182×1680：比 1.219，不触发预缩，残差就是总比
    //   800×1000  → 原样      ：比 1.000，两级都没活干
    volume.page("01.png", &fixtures::line_art(fixtures::SPREAD));
    volume.page(
        "02.png",
        &fixtures::gradient(fixtures::TWO_AND_A_HALF_PANEL),
    );
    volume.page("03.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
    volume.page("04.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("05.png", &fixtures::gradient(fixtures::SMALLER_THAN_TARGET));

    let report = run_volume(&space, &volume);

    let expected = [
        (4.0, 4, 1.0),
        (2.5, 2, 1.25),
        (2.0, 2, 1.0),
        (1.219, 1, 1.219),
        (1.0, 1, 1.0),
    ];
    for (page, (ratio, prescale, residual)) in report.volumes[0].pages.iter().zip(expected) {
        let scaling = page.scaling().expect("处理成了的页有缩放");
        let name = page.source.display();
        assert!(
            (scaling.total_ratio() - ratio).abs() < 5e-4,
            "{name} 的总缩放比是 {}",
            scaling.total_ratio()
        );
        assert_eq!(scaling.prescale(), prescale, "{name} 的预缩倍数");
        assert_eq!(scaling.prescaled(), prescale > 1, "{name} 是否触发预缩");
        assert!(
            (scaling.residual_ratio() - residual).abs() < 5e-4,
            "{name} 的残差比是 {}",
            scaling.residual_ratio()
        );
        // 残差比恒 < 2 是 `CONTEXT.md` 的不变量，不只是这几页的巧合。
        assert!(scaling.residual_ratio() < 2.0, "{name} 的残差比越过了 2");
    }
}

#[test]
fn a_solid_page_stays_solid_after_scaling() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::solid(fixtures::DOUBLE_PANEL, 200));

    // 量的是重采样：把输出钉在 8bit，200 才应当原样出来（4bit 的格点上它会落到 204）。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    assert!(
        written.pixels.iter().all(|&level| level == 200),
        "纯色页缩放后出现了别的取值"
    );
}

#[test]
fn a_screentone_page_resolves_into_tones() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // B 类中位尺寸：总缩放比 1.219，不触发预缩，全程一次 Lanczos3。
    let page = fixtures::screentone(fixtures::TYPICAL);
    volume.page("001.png", &page);
    assert_eq!(
        distinct_levels(page.to_luma8().as_raw()),
        2,
        "网点页必须是二值的"
    );

    // 量的是重采样这一步解析出多少灰调：把输出钉在 8bit，量化不再压级数。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    // 网点是因，灰调是果：缩放把点阵解析成连续灰调。
    assert!(
        !report.volumes[0].pages[0]
            .scaling()
            .expect("处理成了的页有缩放")
            .prescaled()
    );
    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    let levels = distinct_levels(&written.pixels);
    assert!(levels > 16, "缩放后只剩 {levels} 级灰调");
}

#[test]
fn a_prescaled_screentone_page_takes_its_tones_from_the_box_step() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 总缩放比正好 2.000：预缩 2 一步到位，残差段无事可做，输出全部来自 box 那一级。
    // 量的因此是预缩这一级本身。比值非整数时残差段的 Lanczos3 会把级数重新推上去，
    // 那是 ADR 0001 要的分工，不是这一条要管的事。
    volume.page("001.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));

    // 同上：量的是 box 那一级产出几个取值，把输出钉在 8bit 才数得准。
    let report = fixtures::run_volume_at_eight_bits(&space, &volume);

    let page = &report.volumes[0].pages[0];
    assert_eq!(page.scaling().expect("处理成了的页有缩放").prescale(), 2);
    let levels = distinct_levels(&fixtures::read_png(&page.output).pixels);
    // 等权的完整窗口平均对二值输入只落在块内白点计数上，n×n 产出 n²+1 个取值
    // （见 measurements 的《滤波器与灰调级数》）。2×2 于是至多 5 级——
    // 灰调级数受控正是这一级要买的东西，非均匀加权在同一处会给出准连续谱。
    assert!(levels <= 5, "预缩之后出了 {levels} 级灰调，多于 2²+1");
    // 受控不是塌掉：网点仍然被解析成了灰调，不再是二值。
    assert!(levels > 2, "预缩之后只剩 {levels} 级，网点没被解析成灰调");
}

#[test]
fn the_filter_changes_the_residual_step_and_never_the_prescale() {
    // 总缩放比正好 2.000：预缩一步到位，残差段无事可做。换滤波器于是逐字节不变——
    // 预缩那一级恒为 box，`--filter` 够不着它。够得着的话，Lanczos3 会在这里
    // 把二值网点解析成准连续谱，与 area 的输出天差地别。
    assert_eq!(
        one_page_with(fixtures::DOUBLE_PANEL, Filter::Lanczos3),
        one_page_with(fixtures::DOUBLE_PANEL, Filter::Area),
        "预缩那一级被 --filter 改掉了"
    );

    // 比值 1.219 的页只有残差段：换滤波器就换结果。
    assert_ne!(
        one_page_with(fixtures::TYPICAL, Filter::Lanczos3),
        one_page_with(fixtures::TYPICAL, Filter::Area),
        "--filter 没有作用到残差段"
    );

    // 比值 2.500：预缩与残差段都真跑一遍。预缩那一级照旧不受 --filter 摆布，
    // 而残差段仍然听它的——上面两条各废掉一级，只有这一条两级同时在场。
    assert_ne!(
        one_page_with(fixtures::TWO_AND_A_HALF_PANEL, Filter::Lanczos3),
        one_page_with(fixtures::TWO_AND_A_HALF_PANEL, Filter::Area),
        "两级都在场时 --filter 没有作用到残差段"
    );
}

/// 用点名的滤波器处理一张网点页，把写出的 PNG 字节读回来。
///
/// 关掉自描述元数据：记录里带着参数哈希，而滤波器正是它收的一项——留着它，
/// 两次输出必然逐字节不同，`assert_ne!` 那几条会因为几行 tEXt 而通过，
/// 与像素有没有变毫无关系。关掉之后文件里只剩像素那一侧，比字节才是在比重采样。
fn one_page_with(size: Size, filter: Filter) -> Vec<u8> {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::screentone(size));

    let report = tonefit::run(&Request {
        filter,
        metadata: false,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    std::fs::read(&report.volumes[0].pages[0].output).expect("读回写出的页")
}

#[test]
fn the_report_names_the_profile_and_the_panel_it_used() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    // 覆盖过灰阶数的 profile：报告要给出本次实际用的那块面板，而不是内置表里的原样。
    let profile = fixtures::profile("boox-tab-x")
        .with_gray_levels(8)
        .expect("8 级可用");

    let report = fixtures::run_volume_with(&space, &volume, profile);

    assert_eq!(report.profile.device(), "boox-tab-x");
    let panel = report.profile.panel();
    assert_eq!(panel.resolution, Size::new(1650, 2200));
    assert_eq!(panel.ppi, 207);
    assert_eq!(panel.gray_levels, 8);
}

#[test]
fn the_source_volume_is_left_untouched() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page(
        "002.jpg",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );
    let before = fixtures::fingerprint(volume.path());

    run_volume(&space, &volume);

    assert_eq!(
        before,
        fixtures::fingerprint(volume.path()),
        "源目录被改动了"
    );
}

#[test]
fn files_that_are_not_pages_are_not_pages() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), 1);
    assert!(pages[0].source.ends_with("001.png"));
}

#[test]
fn each_volume_mirrors_its_source_tree_under_its_own_output_directory() {
    let space = Workspace::new();
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    let first = space.volume("volume-a");
    first.page("ch1/001.png", &page);
    let second = space.volume("volume-b");
    second.page("001.jpg", &page);

    let report = fixtures::run_paths(&space, [first.path(), second.path()]);

    assert_eq!(report.volumes.len(), 2);
    assert_eq!(report.volumes[0].output, space.out().join("volume-a"));
    assert_eq!(report.volumes[1].output, space.out().join("volume-b"));
    // 输出镜像源的目录结构，扩展名一律换成 png。
    let written: Vec<_> = report
        .volumes
        .iter()
        .flat_map(|volume| &volume.pages)
        .map(|page| {
            assert!(page.output.is_file(), "{} 没写出来", page.output.display());
            slash_path(page.output.strip_prefix(space.out()).unwrap())
        })
        .collect();
    assert_eq!(written, ["volume-a/ch1/001.png", "volume-b/001.png"]);
}

#[test]
fn two_pages_that_would_share_one_output_are_refused() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    // 扩展名一换成 png，这两页就撞在同一个输出上。
    volume.page("001.jpg", &page);
    volume.page("001.png", &page);

    let error = fixtures::run_paths_expecting_failure(&space, [volume.path()]);

    assert!(error.to_string().contains("都要写到"), "{error}");
}

#[test]
fn an_empty_scope_is_refused() {
    let space = Workspace::new();
    let error = fixtures::run_paths_expecting_failure(&space, []);
    assert!(error.to_string().contains("处理范围为空"), "{error}");
}

#[test]
fn writing_into_the_source_volume_is_refused() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    let before = fixtures::fingerprint(volume.path());

    let error = tonefit::run(&Request {
        output_root: volume.path().join("out"),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect_err("往源卷里写应当被拒绝");

    assert!(error.to_string().contains("源库只读"), "{error}");
    assert_eq!(
        before,
        fixtures::fingerprint(volume.path()),
        "拒绝之后仍然写了东西"
    );
}

/// 取值种类数。
fn distinct_levels(pixels: &[u8]) -> usize {
    let mut seen = [false; 256];
    for &level in pixels {
        seen[level as usize] = true;
    }
    seen.iter().filter(|&&hit| hit).count()
}

/// 相对路径拼成用 / 分隔的字符串，好写断言。
fn slash_path(path: &std::path::Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn a_dry_run_gives_the_metric_for_every_page_and_writes_nothing() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page(
        "002.png",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    let before = fixtures::fingerprint(volume.path());

    let report = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("dry-run 应当成功");

    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), 2);
    for page in pages {
        assert!(
            !page.scores().is_empty(),
            "{} 缺判据",
            page.source.display()
        );
        assert!(
            !page.output.exists(),
            "{} 被写出来了",
            page.output.display()
        );
    }
    // 渐变页在低位深上必然崩：报告里的数是真算出来的，不是一排零。
    let gradient = &pages[0].scores();
    assert!(
        gradient[0].score > gradient[2].score,
        "1bit 的 {} 没有差过 4bit 的 {}",
        gradient[0].score,
        gradient[2].score
    );

    assert!(!space.out().exists(), "dry-run 建出了输出目录");
    assert_eq!(before, fixtures::fingerprint(volume.path()), "源卷被动过了");
}

#[test]
fn the_candidates_a_dry_run_scores_are_the_ones_the_panel_can_show() {
    // 候选是两道裁剪的乘积，都在判据求值之前：位深按面板灰阶数裁（ADR 0003），
    // 抖动模式按几何门裁（ADR 0007）。这一页贴住面板，门放行，于是每档位深各两个候选。
    let cases = [(None, 16), (Some(4), 4), (Some(256), 256)];

    for (gray_levels, effective) in cases {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
        let mut profile = fixtures::baseline_profile();
        if let Some(gray_levels) = gray_levels {
            profile = profile.with_gray_levels(gray_levels).expect("级数可用");
        }

        let report = tonefit::run(&Request {
            mode: Mode::DryRun,
            profile,
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("dry-run 应当成功");

        let candidates: Vec<_> = report.volumes[0].pages[0]
            .scores()
            .iter()
            .map(|scored| scored.candidate)
            .collect();
        assert_eq!(
            candidates,
            Candidate::all(effective, GeometryGate::Holds),
            "{gray_levels:?} 级灰阶下的候选不对"
        );
    }
}

/// 几何门是**几何**的，对整卷只有一个结果（ADR 0007：整体关闭，不降级）：一页贴不住面板，
/// 整卷的抖动就整体关闭——不是只关那一页，也不是降级成更温和的模式。
#[test]
fn one_page_smaller_than_the_target_shuts_the_dither_off_for_the_whole_volume() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 头一页贴住面板，本可以抖；第二页源比目标小，按不放大原样输出，把门关上。
    volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
    volume.page(
        "002.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    // 门关在哪一页要指得出来：它关掉的是整卷的抖动。
    assert_eq!(volume_report.gate, Some(GeometryGate::Broken { page: 1 }));
    for page in &volume_report.pages {
        assert_eq!(
            fixtures::verdict(page).candidate.dither,
            Dither::Off,
            "{} 在门关着时抖了",
            page.source.display()
        );
        // 被裁掉的候选不进入判据：门先判，判据在门放行的那套候选上求值。
        assert!(
            page.scores()
                .iter()
                .all(|scored| scored.candidate.dither == Dither::Off),
            "{} 的判据里还留着抖动候选",
            page.source.display()
        );
    }
}

/// 每一页都缩下来贴住面板时门就成立，抖动这才跟着位深一起按卷定下
/// （ADR 0007：上包络取的是这个组合，不设页级抖动开关）。
#[test]
fn a_volume_whose_pages_all_land_on_the_panel_keeps_the_gate_open() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
    volume.page("002.png", &fixtures::gradient(fixtures::TYPICAL));
    // 跨页宽幅页贴住的是宽边，两侧留边换成上下留边，门同样成立。
    volume.page("003.png", &fixtures::gradient(fixtures::SPREAD));

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.gate, Some(GeometryGate::Holds));
    let base = match volume_report.verdict {
        Some(VolumeVerdict::Envelope(envelope)) => envelope.base,
        other => panic!("这一卷该由上包络定档，实际是 {other:?}"),
    };
    assert_eq!(base.dither, Dither::FloydSteinberg);
    // 抖动模式全卷共用一个：页级没有开关。
    for page in &volume_report.pages {
        assert_eq!(fixtures::verdict(page).candidate.dither, base.dither);
    }
}

#[test]
fn processing_writes_the_pages_a_dry_run_only_predicted() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));

    let predicted = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("dry-run 应当成功");
    let written = run_volume(&space, &volume);

    // 报告先给出的路径、尺寸与判定，就是照做之后真正落盘的那一份（spec 的 story 6）。
    let page = (&predicted.volumes[0].pages[0], &written.volumes[0].pages[0]);
    assert_eq!(page.0.output, page.1.output);
    assert_eq!(page.0.size, page.1.size);
    assert_eq!(
        page.0.verdict(),
        page.1.verdict(),
        "dry-run 预告的位深与照做时不一样"
    );
    assert!(page.1.output.is_file());
    // 预告的那一档就是文件里写着的那一档：这一页的取值铺满 4bit 格点，灰度胜出，
    // 位宽因此与判定一致。取值稀疏的页会走调色板、位宽更窄，那条路见
    // a_page_with_few_levels_is_written_as_a_palette_narrower_than_its_verdict。
    assert_eq!(
        fixtures::written_bits(fixtures::read_png(&page.1.output).bit_depth),
        fixtures::verdict(page.0).candidate.bit_depth.bits()
    );
}

#[test]
fn per_page_turns_the_envelope_off_and_gives_every_page_its_own_bit_depth_and_reason() {
    // `--per-page` 关闭上包络与迟滞，给「只要最小体积」留的出口（ADR 0006 决定第 6 条）。
    // 换回来的正是翻页跳变：这两页的判据差得远，档位于是也差着。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 连续渐变页：低位深上必然崩，判定该落在候选上界那一档。
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    // 二值线稿页：1bit 就装得下，判据因此是零，最低那一档直接过关。
    volume.page(
        "002.png",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );

    let report = tonefit::run(&Request {
        per_page: true,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::PerPage),
        "上包络没被关掉"
    );
    let pages = &report.volumes[0].pages;
    assert_eq!(
        fixtures::verdict(&pages[0]).candidate.bit_depth,
        BitDepth::Four
    );
    assert_eq!(
        fixtures::verdict(&pages[0]).reason,
        Reason::LowestWithinThreshold
    );
    assert_eq!(
        fixtures::verdict(&pages[1]).candidate.bit_depth,
        BitDepth::One
    );
    assert_eq!(
        fixtures::verdict(&pages[1]).reason,
        Reason::LowestWithinThreshold
    );
    // 判定是从判据来的：报告里同时给出被判定的那一档的判据值。
    for page in pages {
        assert!(
            page.scores()
                .iter()
                .any(|scored| scored.candidate.bit_depth
                    == fixtures::verdict(page).candidate.bit_depth),
            "{} 的判定位深不在候选里",
            page.source.display()
        );
    }
}

#[test]
fn the_lowest_bit_depth_within_the_threshold_wins() {
    // 判据是量、阈值是界：选的是「界以内的最低一档」，不是「误差最小的一档」——
    // 后者恒是候选上界，位深判定就白做了。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );

    let report = run_volume(&space, &volume);

    let page = &report.volumes[0].pages[0];
    let threshold = report.profile.threshold();
    let chosen = page
        .scores()
        .iter()
        .position(|scored| {
            scored.candidate.bit_depth == fixtures::verdict(page).candidate.bit_depth
        })
        .expect("判定位深必须在候选里");
    assert!(
        threshold.admits(page.scores()[chosen].score),
        "判定的那一档越过了阈值"
    );
    assert!(
        page.scores()[..chosen]
            .iter()
            .all(|scored| !threshold.admits(scored.score)),
        "还有更低的一档也在阈值内"
    );
}

#[test]
fn an_override_replaces_what_the_metric_would_have_chosen() {
    // 两维都点名，候选只剩一个：判定整个被顶掉，判据说什么都不改变结果。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 这一页自动判定会给 2bit+FS（见 each_page_becomes_a_png_at_the_target_size...）。
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Two),
        dither: Some(Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("覆盖后应当照常处理");

    let page = &report.volumes[0].pages[0];
    assert_eq!(
        fixtures::verdict(page).candidate,
        fixtures::plain(BitDepth::Two)
    );
    assert_eq!(fixtures::verdict(page).reason, Reason::Override);
    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::Override(fixtures::plain(BitDepth::Two)))
    );
    // 覆盖了判定，不等于不给判据：报告仍要说得清「你点的这一档判据是多少」。
    assert!(!page.scores().is_empty(), "覆盖之后判据值没了");
}

/// 覆盖项裁的是候选集，一次裁一维：只点了位深，抖动那一维还有得判，判据照旧说了算。
/// 报告因此不说「被顶掉」——那一卷仍有分布可聚合。
#[test]
fn an_override_on_one_axis_leaves_the_other_to_the_metric() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Four),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("覆盖后应当照常处理");

    let page = &report.volumes[0].pages[0];
    assert_eq!(fixtures::verdict(page).candidate.bit_depth, BitDepth::Four);
    assert_ne!(
        fixtures::verdict(page).reason,
        Reason::Override,
        "还有一维在判，不该说被顶掉"
    );
    // 候选只剩点名那一档位深的两个，抖动那一维原样留着。
    assert_eq!(
        page.scores()
            .iter()
            .map(|scored| scored.candidate)
            .collect::<Vec<_>>(),
        [
            fixtures::plain(BitDepth::Four),
            Candidate::new(BitDepth::Four, Dither::FloydSteinberg)
        ]
    );
}

/// 几何门是几何事实，不是自动选择：`--dither` 覆盖不了它（ADR 0007：整体关闭，不降级）。
/// 门不成立时点名抖动当场被拒，不静默照抖。
#[test]
fn a_dither_the_geometry_gate_forbids_is_refused() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let error = tonefit::run(&Request {
        dither: Some(Dither::FloydSteinberg),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect_err("几何门不成立时点名抖动应当被拒绝");

    let said = error.to_string();
    assert!(said.contains("几何门"), "{said}");
    // 是哪一页关的门要说出来——那是唯一能让用户看懂这条拒绝的信息。
    assert!(format!("{error:#}").contains("001.png"), "{error:#}");
}

#[test]
fn a_bit_depth_the_panel_cannot_show_is_refused() {
    // 灰阶数是硬上界（ADR 0003）：覆盖的是自动判定，不是上界。
    // 上界只有 `--gray-levels` 动得了，错误信息因此必须指向它。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let error = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Eight),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect_err("8bit 在 e-ink 面板上应当被拒绝");

    assert!(error.to_string().contains("--gray-levels"), "{error}");
    assert!(!space.out().exists(), "拒绝之后仍然建了输出目录");
}

#[test]
fn when_no_candidate_is_within_the_threshold_the_top_one_is_used() {
    // 灰阶数压到 4 级、抖动点名关掉，候选只剩 {1,2}；渐变页在这两档上都远远越界。
    // 没有可用档时取候选上界兜底——判定要有，理由要说出是兜底。
    //
    // 抖动要点名关掉，否则测不到这一条：同一页同一块面板上 2bit+FS 落在界内
    // （见 dithering_can_bring_a_page_back_within_the_threshold），兜底就不触发了。
    //
    // 兜底是**逐页**那一层的规则，`--per-page` 是它在报告里露面的地方：卷级那一层开着时，
    // 理由说的是基准档从哪来，而「一档都不达标」这件事仍摆在同一页的判据值里。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    let profile = fixtures::baseline_profile()
        .with_gray_levels(4)
        .expect("4 级可用");

    let report = tonefit::run(&Request {
        profile: profile.clone(),
        dither: Some(Dither::Off),
        per_page: true,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let page = &report.volumes[0].pages[0];
    assert_eq!(
        fixtures::verdict(page).candidate,
        fixtures::plain(BitDepth::Two)
    );
    assert_eq!(fixtures::verdict(page).reason, Reason::NoneWithinThreshold);

    // 上包络开着时档位不变：一页的卷，基准档只能是这一页要的那一档。
    // 兜底的那一档是候选上界，卷级那一层不会、也不该把它再抬高。
    let with_envelope = tonefit::run(&Request {
        profile,
        dither: Some(Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    assert_eq!(
        fixtures::verdict(&with_envelope.volumes[0].pages[0]).candidate,
        fixtures::plain(BitDepth::Two)
    );
}

/// 抖动买到的是**低位深上的保真**：同一页同一块面板，不抖动时一档都不达标，
/// 抖过之后 2bit 就落回界内（ADR 0007 的收益，见 measurements 的《抖动》）。
#[test]
fn dithering_can_bring_a_page_back_within_the_threshold() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    // 灰阶数压到 4 级：候选位深只剩 {1,2}，4bit 那条退路不在。
    let profile = fixtures::baseline_profile()
        .with_gray_levels(4)
        .expect("4 级可用");

    let report = tonefit::run(&Request {
        profile,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let page = &report.volumes[0].pages[0];
    assert_eq!(
        fixtures::verdict(page).candidate,
        Candidate::new(BitDepth::Two, Dither::FloydSteinberg)
    );
    assert_eq!(fixtures::verdict(page).reason, Reason::VolumeEnvelope);
    // 不抖动的那两档全部越界，抖过的这一档在界内：判据自己说出了这笔交换。
    let threshold = report.profile.threshold();
    for scored in page.scores() {
        assert_eq!(
            threshold.admits(scored.score),
            scored.candidate >= fixtures::verdict(page).candidate,
            "{} 的 {} 落在了界的另一边",
            scored.candidate,
            scored.score
        );
    }
}

#[test]
fn the_threshold_says_it_has_not_been_calibrated() {
    // spec 的 Further Notes：P0 交付的阈值是占位值，报告必须自己说出这一点。
    let profile = fixtures::baseline_profile();
    let said = profile.to_string();
    assert!(said.contains("未标定"), "{said}");
}

#[test]
fn every_page_is_decoded_exactly_once() {
    // 两遍管线的不变量（ADR 0005：解码一次，缓存缩放后的图）：第一遍解码，
    // 第二遍只从缓存读。计数是这条不变量在 `run` 这个 seam 上看得见的形式——
    // 第二遍一旦回头碰源页，这个数立刻大于页数。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page(
        "002.png",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );
    volume.page("003.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));
    // 透传文件不解码，也就不计数。
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.pages.len(), 3);
    assert_eq!(volume_report.decodes, 3, "源页被解码了不止一遍");
}

#[test]
fn the_report_says_how_much_the_cache_held() {
    // 卷成为不可分割的处理单元，峰值内存随卷大小线性增长（ADR 0005 认下的代价）。
    // 用量因此要在报告里说得出来，否则这条代价对用户是不可见的。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let report = run_volume(&space, &volume);

    let cache = report.volumes[0].cache;
    // 透传文件不进缓存：缓存里只有页。
    assert_eq!(cache.pages, 2);
    // 缓存的是缩放到**目标尺寸**的参照，8 位灰度每像素一字节——未压缩的用量因此手算得出。
    // 存成源尺寸或存成别的精度，这一条立刻不成立。
    let expected: u64 = report.volumes[0]
        .pages
        .iter()
        .map(|page| u64::from(page.size.width) * u64::from(page.size.height))
        .sum();
    assert_eq!(cache.raw, expected);
    // LZ4 之后实际占的字节：正数，且不会比未压缩的还大。
    assert!(cache.stored > 0, "缓存里什么都没有");
    assert!(cache.stored <= cache.raw, "压过之后反而更大了");
}

#[test]
fn a_cache_past_its_budget_spills_to_a_temp_file_and_writes_the_very_same_pages() {
    // 缓存内存优先，超出 `--cache-budget` 溢写临时文件（ADR 0005）。
    // 溢写换的是内存，不是结果：同一卷在两种预算下写出的字节必须逐字节相同。
    let (roomy, roomy_bytes) = one_volume_with_budget(CacheBudget::default());
    let (cramped, cramped_bytes) = one_volume_with_budget(CacheBudget::new(0));

    // 预算够用：全留在内存里，一个字节都不溢写。
    assert_eq!(roomy.cache.spilled, 0, "预算够用时不该溢写");
    assert_eq!(roomy.cache.resident, roomy.cache.stored);
    // 预算为零：一页都留不住，全部溢写。
    assert!(cramped.cache.spilled > 0, "预算为零时没有发生溢写");
    assert_eq!(cramped.cache.resident, 0, "预算为零时不该有常驻");
    assert_eq!(cramped.cache.spilled, cramped.cache.stored);
    // 两种预算存下的总量相同：溢写换的是它待在哪里，不是它有多大。
    assert_eq!(roomy.cache.stored, cramped.cache.stored);
    // 报告里的预算就是本次实际用的那个。
    assert_eq!(cramped.cache.budget, CacheBudget::new(0));

    // 判定与写出的字节都不受缓存去处影响。
    let verdicts = |volume: &tonefit::VolumeReport| -> Vec<_> {
        volume.pages.iter().map(|page| page.verdict()).collect()
    };
    assert_eq!(verdicts(&roomy), verdicts(&cramped), "溢写之后判定变了");
    assert_eq!(roomy_bytes, cramped_bytes, "溢写之后写出的页变了");
}

/// 用点名的缓存预算处理同一个卷，把卷报告与写出的字节一起带回来。
fn one_volume_with_budget(budget: CacheBudget) -> (tonefit::VolumeReport, Vec<Vec<u8>>) {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));

    let report = tonefit::run(&Request {
        cache_budget: budget,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let volume_report = report.volumes.into_iter().next().expect("一个卷");
    let written = volume_report
        .pages
        .iter()
        .map(|page| std::fs::read(&page.output).expect("读回写出的页"))
        .collect();
    (volume_report, written)
}

#[test]
fn a_dry_run_predicts_what_the_cache_will_hold() {
    // 第一遍在两种模式下是同一遍：判据照求，缓存也照建。`--cache-budget` 是本次的参数之一，
    // 而 dry-run 存在的意义就是「照做之前先看一眼这组参数」（spec 的 story 6）——
    // 预告里少了缓存用量，撑不住的预算就要等到照做时才发现。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));

    let predicted = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("dry-run 应当成功");
    let done = run_volume(&space, &volume);

    assert!(predicted.volumes[0].cache.stored > 0, "dry-run 没有建缓存");
    assert_eq!(
        predicted.volumes[0].cache, done.volumes[0].cache,
        "dry-run 预告的缓存用量与照做时不一样"
    );
    // 第二遍在 dry-run 里无事可做，第一遍照旧只解码一次。
    assert_eq!(predicted.volumes[0].decodes, 2);
}

/// 卷级用例的合成卷：`levels` 里每一项造一页，页名按阅读顺序编号。
///
/// 造页一律用纯色页：判据在纯色页上算得出准数——量化误差就是取值到格点的距离，
/// 低通与掩蔽加权都不改它。逐页判定因此由取值直接定死，卷级那一层要的正是一条排得开的分布。
fn volume_of_solids(space: &Workspace, levels: &[u8]) -> fixtures::Volume {
    let volume = space.volume("volume-a");
    for (index, &level) in levels.iter().enumerate() {
        volume.page(
            &format!("{:03}.png", index + 1),
            &fixtures::solid(fixtures::TINY, level),
        );
    }
    volume
}

/// 逐页判定要 2bit 的纯色页：85 正落在 2bit 的格点上，1bit 上差 85。
const NEEDS_TWO_BITS: u8 = 85;

/// 逐页判定要 4bit 的纯色页：96 在 2bit 上差 11——过了阈值，但远够不上「显著偏离」。
const NEEDS_FOUR_BITS: u8 = 96;

/// 逐页判定同样要 4bit，但在 2bit 上差 42：远在界外，离群页判据要的就是这一量级。
const FAR_OUTSIDE: u8 = 128;

#[test]
fn the_body_of_a_volume_shares_one_bit_depth_and_the_report_names_the_page_that_set_it() {
    // 位深不再逐页各判各的（ADR 0006）：主体页共用一个基准档，翻页时不逐页变动。
    // 九页只要 1bit、十页要 2bit：p95 站在 2bit 上，那九页跟着多付一档——
    // 体积不再最优是明知故犯的交换。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    for index in 0..9 {
        volume.page(
            &format!("{:03}.png", index + 1),
            &fixtures::line_art(fixtures::TINY),
        );
    }
    for index in 9..19 {
        volume.page(
            &format!("{:03}.png", index + 1),
            &fixtures::solid(fixtures::TINY, NEEDS_TWO_BITS),
        );
    }

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    let envelope = envelope_of(volume_report);
    assert_eq!(envelope.base, fixtures::plain(BitDepth::Two));
    assert_eq!(envelope.body_pages, 19);
    assert_eq!(envelope.outlier_pages, 0);
    assert_eq!(envelope.raised_pages, 0);
    for page in &volume_report.pages {
        assert_eq!(
            fixtures::verdict(page).candidate.bit_depth,
            BitDepth::Two,
            "{} 没跟着基准档走",
            page.source.display()
        );
        assert_eq!(fixtures::verdict(page).reason, Reason::VolumeEnvelope);
    }
    // 驱动页指得出来，而且它的需求就是基准档：站在 p95 秩上的正是它。
    let driver = &volume_report.pages[envelope.driver];
    assert_eq!(lowest_within_threshold(driver, &report), BitDepth::Two);
}

#[test]
fn a_page_far_outside_the_threshold_is_taken_out_of_the_envelope_and_decided_on_its_own() {
    // 离群页不参与上包络，单独定档（ADR 0006 决定第 5 条）。
    // 卷内只有一页远在界外：它自己拿 4bit，主体照旧留在 2bit 上。
    let space = Workspace::new();
    let mut levels = vec![NEEDS_TWO_BITS; 19];
    levels.push(FAR_OUTSIDE);
    let volume = volume_of_solids(&space, &levels);

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    let envelope = envelope_of(volume_report);
    assert_eq!(envelope.base, fixtures::plain(BitDepth::Two));
    assert_eq!(envelope.outlier_pages, 1);
    assert_eq!(envelope.body_pages, 19);
    assert_ne!(envelope.driver, 19, "离群页不该定出基准档");

    let outlier = &volume_report.pages[19];
    assert_eq!(
        fixtures::verdict(outlier).candidate.bit_depth,
        BitDepth::Four
    );
    assert_eq!(fixtures::verdict(outlier).reason, Reason::Outlier);
    // 交界处那一次跳变是认下的代价，位置指得出来就行：主体一页不动。
    for page in &volume_report.pages[..19] {
        assert_eq!(fixtures::verdict(page).candidate.bit_depth, BitDepth::Two);
        assert_eq!(fixtures::verdict(page).reason, Reason::VolumeEnvelope);
    }
}

/// 二十页里两页远在界外，占一成：主体那十八页留在自己要的那一档上，不被这两页拖高。
///
/// 与上一条用例的差别只有离群页的**页数**：一页与两页在 `run` 这个 seam 上必须同一个结论。
/// 离群页多到什么程度才不再算离群，由立脚点那一层说了算（见 `envelope` 的 `ANCHOR_QUANTILE`），
/// 而不该由「恰好有几页」说了算。
#[test]
fn the_body_keeps_its_base_when_a_tenth_of_the_volume_is_far_outside() {
    let space = Workspace::new();
    let mut levels = vec![NEEDS_TWO_BITS; 18];
    levels.extend([FAR_OUTSIDE; 2]);
    let volume = volume_of_solids(&space, &levels);

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    let envelope = envelope_of(volume_report);
    assert_eq!(envelope.outlier_pages, 2);
    assert_eq!(envelope.body_pages, 18);
    // 主体档不被那两页抬高：十八页主体页要的仍然是 2bit。
    assert_eq!(envelope.base, fixtures::plain(BitDepth::Two));
    // 占比进报告：「一页都没摘出来」与「本来就没有离群页」要分得开（01 号票）。
    assert_eq!(envelope.outlier_share(), 0.1);

    for page in &volume_report.pages[18..] {
        assert_eq!(fixtures::verdict(page).candidate.bit_depth, BitDepth::Four);
        assert_eq!(fixtures::verdict(page).reason, Reason::Outlier);
    }
    for page in &volume_report.pages[..18] {
        assert_eq!(
            fixtures::verdict(page).candidate.bit_depth,
            BitDepth::Two,
            "{} 被那两页拖着走了",
            page.source.display()
        );
        assert_eq!(fixtures::verdict(page).reason, Reason::VolumeEnvelope);
    }
}

/// 彩页不污染灰度页的卷级上包络（ADR 0006 决定第 5 条：彩色 profile 下彩页
/// 根本不进灰度上包络）。同一批灰度页，把彩页混进去前后，基准档、驱动页与逐页判定一个不变。
///
/// 混排还钉住驱动页那个序号：上包络在**灰度页**的序列上取分位，报告里的序号却指进整卷的页。
/// 卷内混着彩页时两者不重合，换算漏掉一次，报告就会指着另一页说「就是它定的档」。
#[test]
fn color_pages_stay_out_of_the_envelope_of_the_gray_pages() {
    let alone = run_with_a_color_page_every(0);
    let mixed = run_with_a_color_page_every(4);

    // 彩页真的混进去了，而且都走了彩色分支：没有判定的就是它们。
    assert_eq!(alone.pages.len(), 20);
    assert_eq!(mixed.pages.len(), 25);
    assert_eq!(
        mixed
            .pages
            .iter()
            .filter(|page| page.verdict().is_none())
            .count(),
        5,
        "彩页该走彩色分支"
    );

    // 灰度页的判定一个不变。
    let verdicts = |volume: &tonefit::VolumeReport| -> Vec<_> {
        volume
            .pages
            .iter()
            .filter_map(|page| page.verdict())
            .collect()
    };
    assert_eq!(
        verdicts(&alone),
        verdicts(&mixed),
        "混进彩页之后灰度页的判定变了"
    );

    // 上包络只数灰度页：主体 19 页 + 离群 1 页，彩页一页不算。
    let envelope = envelope_of(&mixed);
    assert_eq!(envelope.base, envelope_of(&alone).base);
    assert_eq!(envelope.body_pages, 19);
    assert_eq!(envelope.outlier_pages, 1);

    // 驱动页指的仍是同一张灰度页——序号已经从灰度序换回页序。
    let driver_rank = |volume: &tonefit::VolumeReport| {
        let driver = envelope_of(volume).driver;
        assert!(
            volume.pages[driver].verdict().is_some(),
            "驱动页必须是一张灰度页"
        );
        volume.pages[..driver]
            .iter()
            .filter(|page| page.verdict().is_some())
            .count()
    };
    assert_eq!(
        driver_rank(&alone),
        driver_rank(&mixed),
        "驱动页指到了另一页上"
    );
    assert_ne!(
        envelope_of(&alone).driver,
        envelope_of(&mixed).driver,
        "夹具不对：混排没有把驱动页的序号推开，这条用例就什么都没钉住"
    );
}

/// 部分救回页不进卷级上包络，按自己那条判据曲线单独定档（04 号票）。
///
/// 它的判据是在一页大半留白的图上求出来的：留白在任何位深上都是格点、误差恒为零，
/// 那条曲线代表不了这一卷。让它进上包络，一张残页就替整卷定了档。
///
/// 夹具让那一页**比主体更吃位深**：三页主体加它一共四页，p95 的秩落在最后一名上，
/// 它一旦进得去，基准档就是它那一档。断言因此钉得住「摘出去改变了结果」，
/// 而不只是「字段填对了」。
#[test]
fn salvaged_pages_stay_out_of_the_volume_envelope() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 主体三页纯白：任何位深上都是格点，判据恒为零，定的是最低那一档。
    for name in ["001.png", "002.png", "003.png"] {
        volume.page(name, &fixtures::solid(fixtures::TINY, 255));
    }
    // 救回来的那一页：解回来的那一段是连续渐变，低位深上必然崩。
    volume.file(
        "004.png",
        &fixtures::truncated(&fixtures::gradient(fixtures::TINY)),
    );

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    let salvaged = &reported.pages[3];
    assert!(
        salvaged.salvage().is_some(),
        "夹具不对：那一页没被救回，这条用例就什么都没钉住"
    );

    // 主体只有三页：救回来的那一页不在里面。
    let envelope = envelope_of(reported);
    assert_eq!(envelope.body_pages, 3);
    assert_eq!(envelope.outlier_pages, 0);
    // 驱动页指的是一张完好页——序号已经从主体序换回页序。
    assert!(
        reported.pages[envelope.driver].salvage().is_none(),
        "驱动页是一张部分救回页"
    );

    // 它自己那一档由它自己的判据定，理由因此不是「卷级上包络」。
    let decided = fixtures::verdict(salvaged);
    assert_ne!(decided.reason, Reason::VolumeEnvelope);
    // 而它确实比主体更吃位深：进得去的话，基准档就是它那一档。
    assert!(
        decided.candidate > envelope.base,
        "夹具不对：这一页不比主体更吃位深（{} 对 {}），摘不摘它都是同一个基准档",
        decided.candidate,
        envelope.base
    );
    // 主体三页仍然跟着基准档走。
    for page in &reported.pages[..3] {
        assert_eq!(fixtures::verdict(page).candidate, envelope.base);
    }
}

/// 二十页灰度纯色页（十九页主体 + 一页远在界外），每 `every` 页之前插一张彩页。
/// `every` 为 0 就一张彩页都不插。跑的是彩色 profile——彩页只在那上面才走彩色分支。
fn run_with_a_color_page_every(every: usize) -> tonefit::VolumeReport {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let mut levels = vec![NEEDS_TWO_BITS; 19];
    levels.push(FAR_OUTSIDE);

    let mut written = 0;
    let mut write = |image: &image::DynamicImage| {
        written += 1;
        volume.page(&format!("{written:03}.png"), image);
    };
    for (position, &level) in levels.iter().enumerate() {
        if every > 0 && position % every == 0 {
            write(&fixtures::color_page(fixtures::TINY));
        }
        write(&fixtures::solid(fixtures::TINY, level));
    }

    let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(COLOR_DEVICE));
    report.volumes.into_iter().next().expect("一个卷")
}

/// 一台彩色面板设备：彩页只有在彩色 profile 下才走彩色分支（ADR 0010）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

#[test]
fn a_sustained_run_raises_the_depth_but_one_page_short_of_it_does_not() {
    // 迟滞限制档位切换频率：一页说了不算，连续够了才升档（ADR 0006 决定第 4 条）。
    // 两个卷除了要求更高的那一段长了一页，其余完全相同。
    let raised = run_with_a_run_of(3);
    let unchanged = run_with_a_run_of(2);

    assert_eq!(envelope_of(&raised).raised_pages, 3);
    for page in &raised.pages[30..33] {
        assert_eq!(fixtures::verdict(page).candidate.bit_depth, BitDepth::Four);
        assert_eq!(fixtures::verdict(page).reason, Reason::Hysteresis);
    }
    // 段外的页不受影响：升的是那一段，不是全卷。
    assert_eq!(
        fixtures::verdict(&raised.pages[29]).candidate.bit_depth,
        BitDepth::Two
    );
    assert_eq!(
        fixtures::verdict(&raised.pages[33]).candidate.bit_depth,
        BitDepth::Two
    );

    assert_eq!(envelope_of(&unchanged).raised_pages, 0);
    for page in &unchanged.pages {
        assert_eq!(
            fixtures::verdict(page).candidate.bit_depth,
            BitDepth::Two,
            "{} 靠不足一段的要求升了档",
            page.source.display()
        );
    }
}

/// 六十页的卷，第 31 页起有 `length` 页要求高于基准档。
///
/// 六十页是 p95 撑得住一段三页的最小规模：秩落在 57，基准档之上还剩得下三页。
fn run_with_a_run_of(length: usize) -> tonefit::VolumeReport {
    let space = Workspace::new();
    let mut levels = vec![NEEDS_TWO_BITS; 60];
    levels[30..30 + length].fill(NEEDS_FOUR_BITS);
    let volume = volume_of_solids(&space, &levels);

    let report = run_volume(&space, &volume);

    report.volumes.into_iter().next().expect("一个卷")
}

#[test]
fn an_override_leaves_no_volume_envelope_to_speak_of() {
    // `--bit-depth` 顶掉的是判定本身，卷级基准档因此无从谈起——理由仍分得清是覆盖。
    let space = Workspace::new();
    let volume = volume_of_solids(&space, &[NEEDS_TWO_BITS; 4]);

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Four),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    // 覆盖之后逐页判定全是 `Override`，逐页结果里没有分布可聚合：
    // 卷级不是「被关掉」，而是无从谈起，报告要说的正是这一句。
    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::Override(fixtures::plain(BitDepth::Four)))
    );
    for page in &report.volumes[0].pages {
        assert_eq!(fixtures::verdict(page).reason, Reason::Override);
    }
}

/// 取这一卷的上包络。不是上包络定的档就是用例造错了输入。
fn envelope_of(volume: &tonefit::VolumeReport) -> tonefit::Envelope {
    match volume.verdict {
        Some(VolumeVerdict::Envelope(envelope)) => envelope,
        other => panic!("这一卷该由上包络定档，实际是 {other:?}"),
    }
}

/// 这一页逐页判定会选的那一档：判据落在阈值以内的最低一档。
///
/// 卷级那一层把 `verdict` 重定过了，逐页要的那一档因此得从判据曲线与阈值现算——
/// 报告留着 `scores`，算得出来正是它留着的理由。
fn lowest_within_threshold(page: &tonefit::PageReport, report: &tonefit::Report) -> BitDepth {
    let threshold = report.profile.threshold();
    page.scores()
        .iter()
        .find(|scored| threshold.admits(scored.score))
        .map(|scored| scored.candidate.bit_depth)
        .unwrap_or_else(|| page.scores().last().expect("候选非空").candidate.bit_depth)
}
