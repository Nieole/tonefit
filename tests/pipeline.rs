//! `run(Request) -> Report` 这个 seam 上的行为测试。
//!
//! 只断言外部可见的事实：`Report` 里的内容、写出的文件有什么性质、源目录有没有被动过。

mod fixtures;

use fixtures::{Workspace, run_volume};
use tonefit::{
    BitDepth, CacheBudget, Candidate, Dither, Filter, FitMode, GeometryGate, Mode, PageColor,
    Reason, Request, Size, VolumeVerdict,
};

#[test]
fn each_page_becomes_a_png_at_the_target_size_and_the_decided_bit_depth() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 四边顶着墨的渐变页：这一条说的是几何与位深，不是裁边（页几何批 02 号票）。
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );

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
    volume.page("02.png", &fixtures::color_page(fixtures::PASSES_THROUGH));
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
    // 恒等通过的尺寸：不经缩放，「像素一个不动」才谈得成（页几何批 01 号票）。
    let page = fixtures::line_art(fixtures::PASSES_THROUGH);
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
    let page = fixtures::line_art(fixtures::PASSES_THROUGH);
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
    // 恒等通过的尺寸：读回来的就是解码器给出的那张图，中间不隔一层缩放。
    let size = fixtures::PASSES_THROUGH;
    // 四边顶着墨：裁边在它身上是空操作，读回来的尺寸因此仍是源尺寸（页几何批 02 号票）。
    let page = fixtures::full_bleed_gradient(size);
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
        // 取样避开那一圈墨边（`fixtures::INK_BORDER`），量的是渐变本身。
        let top = written.pixel(10, fixtures::INK_BORDER + 2);
        let bottom = written.pixel(10, size.height - fixtures::INK_BORDER - 3);
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
    // 用两种适配方式都恒等通过的尺寸，像素与色带一一对应。
    let size = fixtures::PASSES_THROUGH;
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
    // 两种适配方式都恒等通过的尺寸：色带与像素一一对应，断言谈的就是源上那几个取值。
    // 灰度页那一张四边顶着墨：这个尺寸只管缩放，裁边照跑，不挡着它就把「恒等」裁没了
    // （页几何批 09 号票）。
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

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
    volume.page("001.png", &fixtures::color_page(fixtures::PASSES_THROUGH));
    volume.page("002.png", &fixtures::cheap_page());

    let report = tonefit::run(&Request {
        profile: fixtures::profile(COLOR_DEVICE),
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let pages = &report.volumes[0].pages;
    assert_eq!(pages[0].color(), Some(PageColor::Color));
    assert_eq!(pages[0].verdict(), None, "彩色分支上不该有判定");
    assert_eq!(
        pages[0].size,
        fixtures::PASSES_THROUGH,
        "彩页的目标尺寸照旧算出来"
    );
    assert!(pages[1].verdict().is_some(), "灰度页照旧有判定");
    assert!(!space.out().exists(), "dry-run 落了盘");
}

/// 彩色分支上的页不在几何门的**判定范围**内（ADR 0010 决定第 4 条）。
///
/// 门撑的是抖动与面板灰阶那道硬上界（ADR 0007、ADR 0003），两者只作用在灰度路径上：
/// 彩页既不量化也不抖动，它的几何事实对那两件事没有说话的资格。
///
/// 同一卷换到黑白 profile 上，那一页转灰、走灰度路径，这时它就落进范围里了——
/// 判定范围是**分支的函数**，不是页的常量。
#[test]
fn a_color_page_is_outside_the_scope_of_the_geometry_gate() {
    let build = |space: &Workspace| {
        let volume = space.volume("volume-a");
        // 源比目标小的彩页：一条边都贴不住面板。
        volume.page(
            "001.png",
            &fixtures::color_page(fixtures::SMALLER_THAN_TARGET),
        );
        // 正好两倍面板的灰度页：贴住，门在它这里是开的。
        volume.page(
            "002.png",
            &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
        );
        volume
    };

    // 两趟都点名 fit-inside：头一页要贴不住面板，而门不成立那一支只在这条路上走得到
    // （页几何批 01 号票）。
    let run = |space: &Workspace, volume: &fixtures::Volume, profile| {
        tonefit::run(&Request {
            profile,
            fit: FitMode::Inside,
            ..fixtures::request(space, [volume.path()])
        })
        .expect("处理应当成功")
    };
    let color_space = Workspace::new();
    let color = run(
        &color_space,
        &build(&color_space),
        fixtures::profile(COLOR_DEVICE),
    );
    let mono_space = Workspace::new();
    let mono = run(
        &mono_space,
        &build(&mono_space),
        fixtures::baseline_profile(),
    );

    // 彩色面板上那一页走彩色分支：没有门可判，判定范围因此只有另一页。
    let on_color = &color.volumes[0];
    assert_eq!(on_color.pages[0].gate(), None);
    assert_eq!(on_color.judged_by_the_gate().count(), 1);
    assert_eq!(on_color.outside_the_gate().count(), 0);

    // 同一页转灰之后走灰度路径，它的几何这时说得上话——但只对它自己那一页说。
    let on_mono = &mono.volumes[0];
    assert_eq!(on_mono.pages[0].gate(), Some(GeometryGate::Broken));
    assert_eq!(on_mono.pages[1].gate(), Some(GeometryGate::Holds));
    assert_eq!(on_mono.judged_by_the_gate().count(), 2);
}

/// 部分救回页**在**几何门的判定范围内（ADR 0007 决定第 1 条）：那是文件头里的真尺寸，
/// 它答得出「这一页会不会被下游再缩一次」。
///
/// 04 号票把它摘出去，是因为那时门对整卷只有一个结果——一张没解全的页不该替另外
/// 一百多页回答这个问题。门改成逐页判之后那条理由不在了：它答的只是自己那一页，
/// 而另一页照旧抖得动。
///
/// 上包络那一侧的豁免没有跟着变：它那条判据曲线仍是在一页大半留白的图上求出来的，
/// 代表不了这一卷。两处于是分了家，这一条把分家钉住。
#[test]
fn a_salvaged_page_answers_the_geometry_gate_for_itself_only() {
    let small = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 源比目标小、又只救回了一段的那一页：一条边都贴不住面板。
    volume.file("001.png", &fixtures::truncated(&small));
    // 正好两倍面板的完好页：贴住，门在它这里是开的。
    volume.page(
        "002.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );

    // 门不成立那一支只在 fit-inside 上走得到（页几何批 01 号票）。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);
    let reported = &report.volumes[0];

    // 夹具自证：那一页确实是救回来的，尺寸也确实贴不住面板。
    assert!(
        reported.pages[0].salvage().is_some(),
        "夹具不对：那一页没被截断，这条用例就什么都没钉住"
    );
    assert_eq!(reported.pages[0].size, fixtures::SMALLER_THAN_TARGET);

    // 门问了它，答案是不成立——而它只关掉自己那一页的抖动。
    assert_eq!(reported.judged_by_the_gate().count(), 2);
    assert_eq!(reported.pages[0].gate(), Some(GeometryGate::Broken));
    assert_eq!(reported.pages[1].gate(), Some(GeometryGate::Holds));
    assert_eq!(
        fixtures::verdict(&reported.pages[0]).candidate.dither,
        Dither::Off
    );
    assert_eq!(
        fixtures::verdict(&reported.pages[1]).candidate.dither,
        Dither::FloydSteinberg,
        "一张救回来的小页把另一页的抖动也带走了"
    );
    // 上包络那一侧照旧摘它：主体只剩那一张完好页。
    assert_eq!(envelope_of(reported).body_pages, 1);
}

/// 两刀落在同一页上时，几何门那一刀在外层（ADR 0007 决定第 3 条）。
///
/// 一张既没解全、又贴不住面板的页：04 号票让部分救回页「按自己那条曲线单独定档」，
/// 门这一条让门外的页不低于卷级基准档。两条撞在一起门赢——摘部分救回页的理由是
/// 它那条判据曲线不具代表性（一页大半留白，而留白在任何位深上都是格点、误差恒为零，
/// 判出来必偏低），而不具代表性的曲线更没有资格把这一页压到基准档以下。
///
/// 这一条钉的正是那个组合：两条规矩各自的用例都碰不到它。
#[test]
fn a_salvaged_page_outside_the_gate_never_falls_below_the_volume_base() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 既贴不住面板、又只救回一段的线稿页：它自己那条曲线判得很低。
    volume.file(
        "001.png",
        &fixtures::truncated(&fixtures::line_art(fixtures::SMALLER_THAN_TARGET)),
    );
    // 三页完好的渐变正片，都贴得住面板：基准档由它们定出，比那一页高。
    // 四边顶着墨，「正好两倍面板」「B 类中位页」这两个形状才真的是它们送进来的形状。
    volume.page("002.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page(
        "003.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );
    volume.page("004.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));

    // 门不成立那一支只在 fit-inside 上走得到（页几何批 01 号票）。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);
    let reported = &report.volumes[0];

    // 夹具自证：那一页两刀都挨着——既是部分救回，几何门在它身上又不成立。
    assert!(
        reported.pages[0].salvage().is_some(),
        "夹具不对：那一页没被截断"
    );
    assert_eq!(reported.pages[0].gate(), Some(GeometryGate::Broken));
    // 主体只剩三页完好正片：两刀摘的是同一页，各摘各的理由。
    let base = envelope_of(reported).base;
    assert_eq!(envelope_of(reported).body_pages, 3);

    // 它自己那条曲线要的那一档比基准档低——不然这条用例分不出谁在外层。
    let scores = reported.pages[0].scores();
    let threshold = fixtures::baseline_profile().threshold();
    let own = scores
        .iter()
        .find(|scored| threshold.admits(scored.score))
        .unwrap_or_else(|| scores.last().expect("候选集不会是空的"))
        .candidate;
    assert!(
        own.bit_depth < base.bit_depth,
        "夹具不对：那一页自己那一档不比基准档低（{:?} vs {:?}）",
        own.bit_depth,
        base.bit_depth
    );

    // 门在外层：拿的是基准档的位深、抖动关掉，不是它自己那条曲线判出的那一档。
    let verdict = fixtures::verdict(&reported.pages[0]);
    assert_eq!(
        verdict.candidate,
        Candidate::new(base.bit_depth, Dither::Off),
        "门外的部分救回页掉到了基准档以下"
    );
    assert_eq!(verdict.reason, Reason::OutsideTheGate);
}

/// 一卷里的灰度页一页不剩地落在救回那一侧时，**两处都不摘**：摘一页是为了护着别人，
/// 而那时没有别人可护（04 号票）。
///
/// 几何门那一侧同理（ADR 0007 决定第 5 条）：两页都贴不住面板，一页成立的都没有——
/// 它们于是自己就是主体，卷级基准档由它们定出、必然不抖。这一条要是不成立，
/// 一整卷会被下游再缩一次的页会带着抖动写出去，正是 ADR 0007 拦的那件事。
/// 主体不能空着，这两页因此照旧定得出一个基准档，理由也仍是「卷级上包络」——
/// 不是「几何门不成立」：那一种说的是「摘出去了」，而这一卷没有别人可摘给。
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

    // 门不成立那一支只在 fit-inside 上走得到（页几何批 01 号票）：以高为准让每一页的高
    // 都等于面板高，一条边永远贴着，这一卷就没有「一页成立的都没有」可谈。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(
        reported.salvaged().count(),
        2,
        "夹具不对：两页都该是救回来的"
    );
    // 页比面板小得多，一条边都贴不住：判定范围里一页成立的都没有。
    assert_eq!(reported.pages[0].size, fixtures::TINY);
    assert_eq!(reported.judged_by_the_gate().count(), 2);
    assert_eq!(reported.outside_the_gate().count(), 2);
    // 一页成立的都没有，抖动因此整卷关闭（ADR 0007 决定第 5 条）。
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
    // 恒等通过的尺寸：透明区与不透明区的边界不被重采样糊开。
    let size = fixtures::PASSES_THROUGH;
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

/// `--fit inside` 那条路上，比面板小的页**不放大**（spec 的 story 17）。
#[test]
fn a_page_smaller_than_the_target_keeps_its_size_and_its_pixels_when_fitted_inside() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 四边顶着墨：这一条说的是「不放大」，裁边不该插一脚（页几何批 02 号票）。
    let page = fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET);
    volume.page("001.png", &page);

    // 量的是几何与转灰：把输出钉在 8bit，「逐字节相同」才谈得成。
    let report = fixtures::run_volume_at_eight_bits_fitted_inside(&space, &volume);

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

/// **默认这条路反过来：比面板矮的页被放大到面板高**，几何门跟着成立，抖动不再被关掉
/// （页几何批 01 号票）。
///
/// 这是本票认下的第二笔代价，与跨页那一笔并列：比面板小的卷会被放大。
/// 上一条用例是它的对照——同一页同一块面板，只换适配方式。
#[test]
fn a_page_shorter_than_the_panel_is_enlarged_until_it_touches_the_panel_height() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let report = run_volume(&space, &volume);

    let page = &report.volumes[0].pages[0];
    // 期望值手算：800×1000 放大 1680/1000 = 1.68 倍，宽跟到 1344——比面板宽还宽，
    // 一张比面板小的页也可能要横向平移。
    assert_eq!(page.size, Size::new(1344, 1680));
    assert_eq!(fixtures::read_png(&page.output).size, page.size);
    // 门在它身上成立了，抖动那一维因此回到候选集里。
    assert_eq!(page.gate(), Some(GeometryGate::Holds));
    assert_eq!(
        fixtures::verdict(page).candidate.dither,
        Dither::FloydSteinberg,
        "门成立了，抖动却仍被关着"
    );
    // 报告点得出它超过了面板宽（01 号票：哪些页要横向翻动）。
    assert_eq!(report.wider_than_the_panel().count(), 1);
}

#[test]
fn the_target_size_is_the_page_fitted_inside_the_panel() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("01.png", &fixtures::line_art(fixtures::SPREAD));
    // 两页渐变都四边顶着墨：这两条说的是适配方式，裁边不该插一脚（页几何批 02 号票）。
    volume.page("02.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page(
        "03.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let report = fixtures::run_volume_fitted_inside(&space, &volume);

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

/// 默认这条路上目标高**恒等于面板高**，宽按源宽高比算出、不设上限（页几何批 01 号票）。
///
/// 与上一条用例同一批页、同一块面板，只换适配方式：分岔只出在头一页与末一页身上，
/// 中间那张 B 类中位页两条路上一模一样——下一条用例把那件事单独钉住。
#[test]
fn the_target_size_leads_with_the_panel_height_by_default() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("01.png", &fixtures::line_art(fixtures::SPREAD));
    // 两页渐变都四边顶着墨：这两条说的是适配方式，裁边不该插一脚（页几何批 02 号票）。
    volume.page("02.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page(
        "03.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let report = run_volume(&space, &volume);

    // 期望值手算：跨页 5056×1680 高已经等于面板高，原样出去——宽是面板宽的 4 倍；
    // B 类中位 1441×2048 仍由高边定夺，与 fit-inside 同一个尺寸；
    // 800×1000 放大到面板高，宽 800×1.68 = 1344。
    let expected = [
        Size::new(5056, 1680),
        Size::new(1182, 1680),
        Size::new(1344, 1680),
    ];
    for (page, expected) in report.volumes[0].pages.iter().zip(expected) {
        assert_eq!(page.size, expected, "{}", page.source.display());
        assert_eq!(fixtures::read_png(&page.output).size, expected);
        // 每一页的高都等于面板高，门因此**每一页**都成立。
        assert_eq!(page.gate(), Some(GeometryGate::Holds));
    }
    // 溢出面板宽的是头一页与末一页，中间那张没有。
    let wide: Vec<_> = report
        .wider_than_the_panel()
        .map(|page| page.source.file_name().expect("页有名字").to_owned())
        .collect();
    assert_eq!(wide, ["01.png", "03.png"]);
}

/// **算出来的目标尺寸大到分配不下的那一页退回 fit-inside，整趟照样跑完**（页几何批 07 号票）。
///
/// 以高为准让宽随源比例算出、阅读上不设上限，而目标宽 = 源宽 × 面板高 ÷ 源高：
/// 一根长条在面板上算出的尺寸能到几亿像素，那块缓冲分配不下会**中止整趟**——
/// 不是变成一个失败页，是整趟死掉。管线本来有隔离机制专防「一页坏内容毁掉一整卷」
/// （`CONTEXT.md` 的《失败》），这条用例要的正是那条性质回来。
///
/// 退回的是 fit-inside，**不是失败页**：那一页的像素是好的，退回来仍然读得了，
/// 变成一张白页反而丢内容。
///
/// 写在 `run` 这个 seam 上而不只写在几何那一层：几何那一层答得出「该退回」，
/// 答不出「整趟真的跑完了、别的页一点没受影响」。
#[test]
fn a_page_whose_target_would_not_fit_in_memory_falls_back_and_the_run_finishes() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 整页纯墨：裁边一个像素都拿不走，这一页走得到的只有兜底那一条。
    volume.page("01.png", &fixtures::solid(fixtures::DEGENERATE_STRIP, 0));
    // 一张普通页做对照，四边顶着墨，裁边同样不插一脚。
    volume.page("02.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), 2);
    // 整趟跑完了，而那一页也没被记成失败页——卷因此仍在干净的去处。
    assert_eq!(report.failures().count(), 0);
    assert!(!report.any_isolated());
    // 退回 fit-inside：3000×100 等比缩进面板是 1264×42。以高为准会算出 50400×1680。
    let backed_off = Size::new(1264, 42);
    assert_eq!(pages[0].size, backed_off);
    assert_eq!(fixtures::read_png(&pages[0].output).size, backed_off);
    assert!(pages[0].backstopped());
    // 门照旧问它，而且照 fit-inside 那条判：宽那条边贴住了面板。
    assert_eq!(pages[0].gate(), Some(GeometryGate::Holds));
    // **逐页可指认**：用户要知道哪几页没按点名的方式适配。
    let named: Vec<_> = report
        .backstopped()
        .map(|page| page.source.file_name().expect("页有名字").to_owned())
        .collect();
    assert_eq!(named, ["01.png"]);
    // **其余页一点不受影响**：照旧按以高为准出，1441×2048 → 1182×1680。
    assert_eq!(pages[1].size, Size::new(1182, 1680));
    assert!(!pages[1].backstopped());
}

/// **普通漫画页两种适配方式产出同一个尺寸。**
///
/// 那不是巧合，是「页比面板更瘦长、本来就受高度约束」的直接后果：两条路上宽都由同一个
/// `面板高 ÷ 源高` 算出。实测棋魂完全版 230 页、N和S 第43话 24 页 **100% 一致**
/// （measurements 的《适配方式：fit-inside 与以高为准》）。
///
/// 写在 `run` 这个 seam 上，而不只写在几何那一层：用户看得见的是输出文件，
/// 而「开关不该起作用的地方它不起作用」是 spec 的 story 16。
/// 断言连**输出字节**都比——尺寸相同还不够，判定与写出也得一字不差。
#[test]
fn an_ordinary_comic_page_comes_out_the_same_size_either_way() {
    // 宽高比 1.42：B 类素材的中位页（measurements 的《B 类素材普查》），
    // 比面板的 1.35 更瘦长，且比面板高——两条前提都在。
    let build = |space: &Workspace| {
        let volume = space.volume("volume-a");
        volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
        volume.page("002.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));
        volume.page(
            "003.png",
            &fixtures::line_art(fixtures::TWO_AND_A_HALF_PANEL),
        );
        volume
    };

    let height_space = Workspace::new();
    let by_height = run_volume(&height_space, &build(&height_space));
    let inside_space = Workspace::new();
    let inside = fixtures::run_volume_fitted_inside(&inside_space, &build(&inside_space));

    for (left, right) in by_height.volumes[0]
        .pages
        .iter()
        .zip(&inside.volumes[0].pages)
    {
        assert_eq!(left.size, right.size, "{}", left.source.display());
        assert_eq!(
            fixtures::verdict(left).candidate,
            fixtures::verdict(right).candidate,
            "{}",
            left.source.display()
        );
        // 比的是**像素**，不是文件字节：适配方式进了参数哈希（见 `crate::metadata`），
        // 两趟的 tEXt 本来就不同，而那正是幂等要的（换了它这一卷要重做）。
        let (left_png, right_png) = (
            fixtures::read_png(&left.output),
            fixtures::read_png(&right.output),
        );
        assert_eq!(
            left_png.pixels,
            right_png.pixels,
            "{} 两条路上写出的像素不同",
            left.source.display()
        );
        assert_eq!(left_png.bit_depth, right_png.bit_depth);
    }
    // 一页都没有溢出面板：普通漫画卷在以高为准下不必横向翻动。
    assert_eq!(by_height.wider_than_the_panel().count(), 0);
}

/// 裁边那几条用例共用的一页：1441×2048 的纸，中间 1200×1600 那一块是内容。
///
/// 四边的白边各不相等（左 120 上 100 右 121 下 348）：窗口摆错一边就当场对不上。
const MARGINED: Size = fixtures::TYPICAL;
const CONTENT: Size = Size::new(1200, 1600);
const CONTENT_AT: (u32, u32) = (120, 100);

/// **裁边发生在适配之前**（页几何批 02 号票）：目标尺寸从裁完的那个尺寸算出来，
/// 而不是从解出来的那个。
///
/// 同一页跑两趟，只换裁边这一个开关：裁的那一趟目标尺寸是 1200×1600 顶到面板高，
/// 不裁的那一趟是 1441×2048 顶到面板高。两个数不同，正是「先裁后适配」的现场——
/// 反过来（先适配再裁）会得出第三个数，而那个数一页都对不上。
///
/// 报告逐页说得出裁掉了多少：裁前裁后两个尺寸加左上角，四边因此都减得出来。
#[test]
fn margins_come_off_before_the_page_meets_the_panel() {
    // 两趟各起一个工作区：同一个输出根会被后一趟覆盖，那时比的就不是两趟的差别了。
    let build = |space: &Workspace| {
        let volume = space.volume("volume-a");
        volume.page(
            "001.png",
            &fixtures::page_with_margins(MARGINED, CONTENT_AT, CONTENT),
        );
        volume
    };
    let space = Workspace::new();
    let report = run_volume(&space, &build(&space));
    let kept_space = Workspace::new();
    let kept = fixtures::run_volume_keeping_margins(&kept_space, &build(&kept_space));

    let page = &report.volumes[0].pages[0];
    let crop = page.crop().expect("处理成了的页有裁边");
    // 裁掉了多少：报告自己说得出来。右边与下边减得出来——
    // 1441 - 1200 - 120 = 121，2048 - 1600 - 100 = 348。
    assert_eq!(crop.before(), MARGINED);
    assert_eq!(crop.after(), CONTENT);
    assert_eq!((crop.left(), crop.top()), CONTENT_AT);
    assert_eq!(
        (
            crop.before().width - crop.after().width - crop.left(),
            crop.before().height - crop.after().height - crop.top(),
        ),
        (121, 348)
    );
    // 期望值手算：裁完 1200×1600 顶到面板高 1680，宽 = 1200 × 1680 ÷ 1600 = 1260。
    assert_eq!(page.size, Size::new(1260, 1680));
    assert_eq!(fixtures::read_png(&page.output).size, page.size);

    // `--no-crop` 退回从前的行为：一个像素都不裁，目标尺寸从 1441×2048 算出
    // （1441 × 1680 ÷ 2048 = 1182）。
    let kept_page = &kept.volumes[0].pages[0];
    let kept_crop = kept_page.crop().expect("处理成了的页有裁边");
    assert!(!kept_crop.trimmed(), "关掉了裁边却还是裁了");
    assert_eq!(kept_crop.after(), MARGINED);
    assert_eq!(kept_page.size, Size::new(1182, 1680));
}

/// **白边里的孤立噪点不算内容。**
///
/// 这是本裁法与内容外接框的分水岭，也是这张票点名要钉住的那一条：外接框边缘沾一个墨点
/// 就退回整页，实测那样量出来的中位增益是 0（见 measurements 的《裁边》）。
///
/// 同一页两份，一份白边干净、一份白边里落了三粒墨点，两份裁出来的窗口必须一模一样；
/// 而那三粒顶在两个对角上，外接框在这一页上**一个像素都裁不掉**——那一句由夹具自己作证，
/// 不然这条用例在退化成外接框的实现下照样是绿的。
#[test]
fn a_speck_in_the_margin_is_not_content() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::page_with_margins(MARGINED, CONTENT_AT, CONTENT),
    );
    let specked = fixtures::page_with_specks_in_the_margin(MARGINED, CONTENT_AT, CONTENT);
    volume.page("002.png", &specked);

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    let window = |page: &tonefit::PageReport| page.crop().expect("处理成了的页有裁边").after();
    assert_eq!(window(&pages[0]), CONTENT);
    assert_eq!(window(&pages[1]), CONTENT, "三粒噪点把窗口撑回了整页");
    assert_eq!(pages[0].size, pages[1].size, "噪点改变了目标尺寸");
    // 夹具自证：外接框在带噪点的那一页上退回整页，本裁法没有。
    assert_eq!(
        fixtures::ink_bounding_box(&specked),
        MARGINED,
        "夹具没造对：外接框在这一页上本该失败"
    );
}

/// **整页空白的页原样通过，不裁成零尺寸**（页几何批 02 号票）。
///
/// 一个墨点都没有的页挑不出内容行列，窗口于是等于整页——那一页照常适配、照常写出。
/// 裁成零尺寸的实现在这里会当场崩掉：0 像素的页写不出去。
#[test]
fn a_blank_page_is_not_cropped_away() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::solid(MARGINED, 255));

    let report = run_volume(&space, &volume);

    let page = &report.volumes[0].pages[0];
    assert!(!page.crop().expect("处理成了的页有裁边").trimmed());
    // 与不裁边那一趟同一个目标尺寸：1441 × 1680 ÷ 2048 = 1182。
    assert_eq!(page.size, Size::new(1182, 1680));
    assert_eq!(fixtures::read_png(&page.output).size, page.size);
}

/// **页间字号跳动是接受的形态，不是缺陷**（页几何批 02 号票）。
///
/// 逐页各裁各的：白边宽的那一页裁完剩得少、被放得更大，白边窄的那一页放得少。
/// 同样一块 1200 宽的内容因此在两页上出成不同的宽度，翻页时字号跟着跳
/// （实测卷内极差 B 类 0.041–0.109、A 类双页 0.390，见 measurements 的《裁边》）。
///
/// **不要为了抹平它去取卷级裁切框**：那会被留白最多的那一页拖住，而用户明确要的是
/// 更大的实际利用面积（票面的《不要做的》）。这条用例写在这里，就是为了让后来的人
/// 在改成卷级框时当场撞响，而不是把它当回归修掉。
#[test]
fn pages_with_different_margins_are_magnified_differently_and_that_is_accepted() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 同一张纸、同样宽的内容，只有上下白边不同：一页留白多，一页留白少。
    volume.page(
        "001.png",
        &fixtures::page_with_margins(MARGINED, CONTENT_AT, Size::new(1200, 1200)),
    );
    volume.page(
        "002.png",
        &fixtures::page_with_margins(MARGINED, CONTENT_AT, Size::new(1200, 1900)),
    );

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    // 期望值手算：内容 1200×1200 顶到面板高，宽 = 1200 × 1680 ÷ 1200 = 1680；
    // 内容 1200×1900 顶到面板高，宽 = 1200 × 1680 ÷ 1900 = 1061。
    assert_eq!(pages[0].size, Size::new(1680, 1680));
    assert_eq!(pages[1].size, Size::new(1061, 1680));
    // 同一块内容在两页上放大得不一样多——那就是字号跳动。
    let gain = |page: &tonefit::PageReport| page.crop().expect("有裁边").linear_gain();
    assert!(
        gain(&pages[0]) - gain(&pages[1]) > 0.5,
        "两页的线性放大只差 {:.3}，夹具没把跳动造出来",
        gain(&pages[0]) - gain(&pages[1])
    );
}

/// **裁边与适配方式正交：四种组合都跑得通**（页几何批 02 号票）。
///
/// 两个开关各改一件事——裁边改的是适配之前的页尺寸，适配方式改的是页怎么顶上面板——
/// 组合起来不该有哪一种走不下去。断言只要外部可见的三件事：跑得完、写出来的页解得回来、
/// 尺寸与报告说的对得上。具体数值由上面那几条各自钉着。
#[test]
fn all_four_combinations_of_crop_and_fit_run_through() {
    for fit in [FitMode::Height, FitMode::Inside] {
        for crop in [true, false] {
            let space = Workspace::new();
            let volume = space.volume("volume-a");
            volume.page(
                "001.png",
                &fixtures::page_with_margins(MARGINED, CONTENT_AT, CONTENT),
            );
            volume.page("002.png", &fixtures::solid(MARGINED, 255));
            volume.page(
                "003.png",
                &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
            );

            let report = tonefit::run(&Request {
                fit,
                crop,
                ..fixtures::request(&space, [volume.path()])
            })
            .unwrap_or_else(|error| panic!("{fit:?} + 裁边 {crop} 没跑下来：{error:#}"));

            let pages = &report.volumes[0].pages;
            assert_eq!(pages.len(), 3, "{fit:?} + 裁边 {crop}");
            for page in pages {
                let written = fixtures::read_png(&page.output);
                assert_eq!(written.size, page.size, "{fit:?} + 裁边 {crop}");
                // 裁过没裁过，报告都答得出来；关着的那一趟一页都不许裁。
                let cropped = page.crop().expect("处理成了的页有裁边");
                assert!(crop || !cropped.trimmed(), "关掉了裁边却还是裁了");
            }
            assert_eq!(report.crop, crop, "报告没记住这一趟开没开裁边");
            assert_eq!(report.fit, fit);
        }
    }
}

/// **部分救回页不裁边**（页几何批 02 号票）。
///
/// 它缺的那一段留成纸白（`CONTEXT.md` 的《失败》），而纸白按墨量就是白边——
/// 裁掉它等于把「这一页缺了一半」从产物里抹掉，报告里那个救回比例也就再对不上尺寸。
/// 缺的那一段不是白边，是缺的那一段。
#[test]
fn a_salvaged_page_keeps_the_blank_it_could_not_decode() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 截断的纯黑页：解回来的那一段是 0，救不回来的那一段是纸白。
    volume.file("001.png", &fixtures::truncated_page(MARGINED));

    let report = run_volume(&space, &volume);

    let page = &report.volumes[0].pages[0];
    assert!(page.salvage().is_some(), "夹具没造出一张部分救回页");
    assert!(
        !page.crop().expect("处理成了的页有裁边").trimmed(),
        "把救回页缺的那一段当白边裁掉了"
    );
    // 尺寸仍是文件头里的那个：1441 × 1680 ÷ 2048 = 1182。
    assert_eq!(page.size, Size::new(1182, 1680));
}

/// 跨页拆分那几条用例共用的一页：带装订沟的跨页，沟中心取实测最偏的那一页。
fn spread_page() -> image::DynamicImage {
    fixtures::spread_with_gutter(
        fixtures::SPREAD_WITH_GUTTER,
        fixtures::GUTTER_CENTER,
        fixtures::GUTTER_WIDTH,
    )
}

/// 那条沟的头一列与末一列（含），坐标在源页上。**算式只有一个出处**
/// （`fixtures::gutter_left`），用例拿它推两半的窗口该落在哪儿。
fn gutter_span() -> (u32, u32) {
    let left = fixtures::gutter_left(
        fixtures::SPREAD_WITH_GUTTER,
        fixtures::GUTTER_CENTER,
        fixtures::GUTTER_WIDTH,
    );
    (left, left + fixtures::GUTTER_WIDTH - 1)
}

/// 一卷输出页的成员名，按阅读顺序。
fn output_names(volume: &tonefit::VolumeReport) -> Vec<String> {
    volume
        .pages
        .iter()
        .map(|page| fixtures::relative_name(&volume.output, &page.output))
        .collect()
}

/// **切点落在装订沟上，而每半各自再裁一次**（页几何批 04 号票）。
///
/// 这一条把票面的三件事钉在一处，因为它们在产物上是同一组数：
///
/// - **切点跟着沟走，不切正中。**沟中心取 0.441（实测区间 0.401–0.538 里偏离正中较远的
///   一侧），按正中盲切会切进右半的画面。两半的窗口因此宽度不等，
///   而那两个数只有跟着沟走才算得出来。
/// - **一列画面都没切掉，沟也没留在产物里。**两半的窗口严丝合缝地贴着沟的两侧：
///   切开是从沟中心下的刀，沟剩下的那两截纸白由「每半再裁」收走。
///   两半宽度之和恰好是页宽减去沟宽——这个等号是「每半各自再裁」量得出来的形式。
/// - **收益是不必横向翻动。**同一页不拆的话顶到面板高要 1.86 屏宽，拆开之后每半
///   0.81 与 1.03 屏（实测不拆的屏占比中位 1.88–1.91、拆开 0.88–0.92，
///   见 measurements 的《跨页拆分》）。**两条路的缩放系数完全相同**：
///   拆分不是拿分辨率换的。
#[test]
fn a_spread_is_cut_at_its_gutter_and_each_half_is_cropped_again() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &spread_page());

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    // 一张源页，两张输出页——报告两个数分开给（页几何批 03 号票）。
    assert_eq!(reported.source_pages, 1);
    assert_eq!(reported.page_count(), 2);
    assert_eq!(reported.decodes, 1, "切开发生在解码之后，源页只解一次");
    // 成员名一对多加序号，序号顺序就是阅读顺序；两张都指着同一个源页。
    assert_eq!(output_names(reported), ["001-1.png", "001-2.png"]);
    let pages = &reported.pages;
    assert!(pages.iter().all(|page| page.source.ends_with("001.png")));
    // 默认右开：右半在先。
    let side = |page: &tonefit::PageReport| page.cut().expect("这是切出来的一半").side();
    assert_eq!(
        [side(&pages[0]), side(&pages[1])],
        [tonefit::Side::Right, tonefit::Side::Left]
    );

    let source = fixtures::SPREAD_WITH_GUTTER;
    let (from, to) = gutter_span();
    let window = |page: &tonefit::PageReport| page.crop().expect("处理成了的页有裁边");
    let (right, left) = (window(&pages[0]), window(&pages[1]));
    // 窗口都长在源页上：裁边、切开、每半再裁三段叠成源页上的一块。
    assert_eq!((right.before(), left.before()), (source, source));
    assert_eq!(
        (left.left(), left.after()),
        (0, Size::new(from, source.height))
    );
    assert_eq!(
        (right.left(), right.after()),
        (to + 1, Size::new(source.width - to - 1, source.height))
    );
    // 两半之和恰好是页宽减去沟宽：一列画面都没丢，沟也没留下。
    assert_eq!(
        left.after().width + right.after().width,
        source.width - fixtures::GUTTER_WIDTH
    );
    // **不在正中**：按正中盲切会切进右半的画面这么多列。
    assert!(
        source.width / 2 - (to + 1) > 100,
        "切点离正中只有 {} 列，夹具没把「沟不在正中」造出来",
        source.width / 2 - (to + 1)
    );

    // 每半顶到面板高，写出来的与报告说的对得上。
    // 期望值手算：1313 × 1680 ÷ 2160 = 1021、1671 × 1680 ÷ 2160 = 1300。
    assert_eq!(pages[0].size, Size::new(1300, 1680));
    assert_eq!(pages[1].size, Size::new(1021, 1680));
    for page in pages {
        assert_eq!(fixtures::read_png(&page.output).size, page.size);
    }

    // 不拆的那一趟做对照：同一页 2352 宽，横向要平移 1.86 屏。
    let kept = Workspace::new();
    let whole = kept.volume("volume-a");
    whole.page("001.png", &spread_page());
    let unsplit = fixtures::run_volume_without_splitting(&kept, &whole);
    let unsplit = &unsplit.volumes[0];
    assert_eq!(unsplit.page_count(), 1);
    assert_eq!(unsplit.pages[0].size, Size::new(2352, 1680));
    // **缩放系数完全相同**：两条路都把源页的高顶到面板高，一像素内容占的屏幕面积一样大。
    // 拆分省下的是横向翻动，不是分辨率。
    let scale =
        |page: &tonefit::PageReport| page.scaling().expect("处理成了的页有缩放").total_ratio();
    assert_eq!(scale(&pages[0]), scale(&unsplit.pages[0]));
    assert_eq!(scale(&pages[1]), scale(&unsplit.pages[0]));
}

/// **找不到装订沟就是连续跨页，不切**（页几何批 04 号票的硬约束）。
///
/// 一幅画横跨两页，切开就毁了。它退回以高为准，靠阅读器横向平移看——报告因此把它
/// 点在「输出宽超过面板」那一小结里（页几何批 01 号票）。
///
/// 夹具自证：这一页**够得上跨页候选**，挡下它的是「没有沟」这一关，不是宽高比那一关。
/// 没有这一句，一个候选那一关就把它拦下的实现在这里照样是绿的。
#[test]
fn a_spread_without_a_gutter_is_left_whole_and_read_by_panning() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 竖直渐变的宽幅页：每一列长得一模一样，一条空白列都挑不出来。
    // 四边顶着墨，裁边因此不动它的宽高比——裁掉渐变下方那一片会把 3.01 推到 3.84，
    // 「够得上跨页候选」就成了裁边送的，而不是夹具造的（页几何批 09 号票）。
    volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::SPREAD));

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(reported.source_pages, 1);
    assert_eq!(reported.page_count(), 1, "连续跨页被切开了");
    assert_eq!(output_names(reported), ["001.png"], "没切开却改了成员名");
    let page = &reported.pages[0];
    assert!(page.cut().is_none());
    // 退回以高为准：宽溢出面板，靠横向平移看。
    assert!(page.size.width > report.profile.panel().resolution.width);
    assert_eq!(report.wider_than_the_panel().count(), 1);
    // **夹具自证**：这一页够得上跨页候选，挡下它的只能是「没有沟」。
    // 没有这一句，一个在候选那一关就把它拦下的实现在这里照样是绿的——
    // 而那时红的会是别的用例，不是这一条。
    assert!(
        page.spread_candidate(),
        "夹具没造对：这一页本该够得上跨页候选"
    );
}

/// **阅读方向定两半的先后，成员名跟着反**（页几何批 04 号票，spec 的 story 3、story 4）。
///
/// 反过来之后同一张图仍然是原来那一侧——变的只有谁排在 `001-1.png` 上。
/// 断言因此同时问两件事：成员名的次序反了，而两半的窗口一个像素都没变。
#[test]
fn the_reading_order_decides_which_half_gets_the_first_name() {
    let run = |order: tonefit::ReadingOrder| {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        volume.page("001.png", &spread_page());
        let report = tonefit::run(&Request {
            split: tonefit::SplitRule {
                order,
                ..tonefit::SplitRule::default()
            },
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功");
        let reported = &report.volumes[0];
        // 每一半落在哪个成员名上，以及它的窗口。工作区一落地就把临时目录收走，
        // 因此在这里把要断言的都取出来。
        let landed: Vec<(tonefit::Side, String, tonefit::Crop)> = reported
            .pages
            .iter()
            .map(|page| {
                (
                    page.cut().expect("这是切出来的一半").side(),
                    fixtures::relative_name(&reported.output, &page.output),
                    page.crop().expect("处理成了的页有裁边"),
                )
            })
            .collect();
        (output_names(reported), landed)
    };

    let (names, right_open) = run(tonefit::ReadingOrder::RightToLeft);
    let (flipped_names, left_open) = run(tonefit::ReadingOrder::LeftToRight);

    // 成员名两趟一模一样：变的是哪一半落在哪个名字上。
    assert_eq!(names, ["001-1.png", "001-2.png"]);
    assert_eq!(flipped_names, names);
    let landed_on = |run: &[(tonefit::Side, String, tonefit::Crop)], side| {
        run.iter()
            .find(|(found, ..)| *found == side)
            .map(|(_, name, _)| name.clone())
            .expect("两半都在")
    };
    // 右开：右半排在 `001-1.png` 上。反过来之后两个名字跟着对调。
    assert_eq!(landed_on(&right_open, tonefit::Side::Right), "001-1.png");
    assert_eq!(landed_on(&right_open, tonefit::Side::Left), "001-2.png");
    assert_eq!(landed_on(&left_open, tonefit::Side::Left), "001-1.png");
    assert_eq!(landed_on(&left_open, tonefit::Side::Right), "001-2.png");
    // 两半的窗口一个像素都没变：阅读方向只改先后，不改切法。
    let window = |run: &[(tonefit::Side, String, tonefit::Crop)], side| {
        run.iter()
            .find(|(found, ..)| *found == side)
            .map(|(.., crop)| *crop)
            .expect("两半都在")
    };
    for side in [tonefit::Side::Right, tonefit::Side::Left] {
        assert_eq!(window(&right_open, side), window(&left_open, side));
    }
}

/// **混排卷两类页都落对**（页几何批 04 号票，spec 的 story 7）。
///
/// 一卷里有些页已经拆好、有些还是连页，这是常态。已经拆好的单页够不上候选、原样出一张；
/// 带沟的跨页切成两张；没有沟的连续跨页够得上候选却仍出一张。
/// 三类页因此在同一卷里给出 1 + 2 + 1 = 4 张输出页，而源页是 3 张。
#[test]
fn a_volume_that_mixes_single_pages_and_spreads_lands_both_right() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page("002.png", &spread_page());
    volume.page("003.png", &fixtures::full_bleed_gradient(fixtures::SPREAD));

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(reported.source_pages, 3);
    assert_eq!(reported.page_count(), 4);
    assert_eq!(
        output_names(reported),
        ["001.png", "002-1.png", "002-2.png", "003.png"]
    );
    // 切开的只有带沟的那一张：另外两张都不是切出来的一半。
    let cut: Vec<bool> = reported
        .pages
        .iter()
        .map(|page| page.cut().is_some())
        .collect();
    assert_eq!(cut, [false, true, true, false]);
    for page in &reported.pages {
        assert_eq!(fixtures::read_png(&page.output).size, page.size);
    }
}

/// **拆分关得掉，阈值调得动**（页几何批 04 号票的头两条验收）。
///
/// 关掉与把阈值抬到这一页的比值之上，两条路都让它原样出一张——而两条路**不是同一件事**，
/// 报告分得开：关掉那一趟连跨页候选都不判（`spread_candidate` 为假，因为整卷不拆），
/// 抬高阈值那一趟是这一页够不上候选。两处都断言，不然「阈值可调」与「开关关得掉」
/// 在产物上长得一模一样。
#[test]
fn the_split_switch_and_its_threshold_both_leave_the_spread_whole() {
    let whole = |split: tonefit::SplitRule| {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        volume.page("001.png", &spread_page());
        let report = tonefit::run(&Request {
            split,
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功");
        let reported = &report.volumes[0];
        (
            reported.page_count(),
            output_names(reported),
            reported.pages[0].spread_candidate(),
            report.split,
        )
    };

    // 默认那一套切得开——不然下面两条什么都没证明。
    let (count, names, candidate, rule) = whole(tonefit::SplitRule::default());
    assert_eq!(count, 2);
    assert_eq!(names, ["001-1.png", "001-2.png"]);
    assert!(candidate && rule.on);

    let (count, names, candidate, rule) = whole(tonefit::SplitRule {
        on: false,
        ..tonefit::SplitRule::default()
    });
    assert_eq!(count, 1, "拆分关着还是切开了");
    assert_eq!(names, ["001.png"], "没切开却改了成员名");
    assert!(!candidate, "拆分关着，这一页连跨页候选都不该判");
    assert!(!rule.on, "报告没记住这一趟没开拆分");

    // 这一页的比值是 (3024/2160) ÷ (1264/1680) = 1.86，抬到 2.5 就够不上跨页候选了。
    let raised = tonefit::SplitThreshold::parse("2.5").expect("正数");
    let (count, names, candidate, rule) = whole(tonefit::SplitRule {
        threshold: raised,
        ..tonefit::SplitRule::default()
    });
    assert_eq!(count, 1, "阈值抬到比值之上还是切开了");
    assert_eq!(names, ["001.png"]);
    assert!(!candidate, "阈值抬上去了，这一页不该再是跨页候选");
    assert_eq!(rule.threshold, raised, "报告没记住这一趟的阈值");
}

/// **彩色分支上的跨页也切得开，而且切在同一处**（页几何批 04 号票）。
///
/// 拆分的次序（裁边 → 判跨页 → 拆分 → 每半再裁）在灰度路径与彩色分支上**各写了一遍**
/// （`crate::Compute` 的 `gray_pages` 与 `color_pages`）。两处代码不共用，漏改一处
/// 不会有编译错——只会让彩页在切开这件事上悄悄走另一条路。这一条把两条路并排跑一遍：
/// 同一张跨页，一次在黑白面板上（转灰、走灰度路径）、一次在彩色面板上（走彩色分支），
/// **两半的窗口必须逐字相同**。
///
/// `Split::of_gray` 与 `Split::of_color` 答不答得一样，在 `src/spread.rs` 的用例里量过；
/// 这一条量的是它外面那一圈——切完之后每半再裁、三段窗口怎么叠起来。
#[test]
fn the_color_branch_splits_a_spread_at_the_very_same_place() {
    let cut_windows = |device: &str| {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        // 彩色的跨页：把带装订沟的那一页染成彩色，纸白仍是纸白。
        volume.page("001.png", &fixtures::colorize(&spread_page()));
        let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(device));
        let reported = &report.volumes[0];
        let windows: Vec<(tonefit::Side, tonefit::Crop)> = reported
            .pages
            .iter()
            .map(|page| {
                (
                    page.cut().expect("这是切出来的一半").side(),
                    page.crop().expect("处理成了的页有裁边"),
                )
            })
            .collect();
        (
            windows,
            reported
                .pages
                .iter()
                .map(|page| page.size)
                .collect::<Vec<_>>(),
            reported.pages[0].color(),
        )
    };

    // 黑白面板：这一页转灰，走灰度路径。
    let (gray_windows, gray_sizes, _) = cut_windows(fixtures::BASELINE_DEVICE);
    // 彩色面板：同一页保留颜色，走彩色分支。
    let (color_windows, color_sizes, identified) = cut_windows("kobo-libra-colour");

    assert_eq!(identified, Some(PageColor::Color), "夹具没造出一张彩页");
    assert_eq!(gray_windows.len(), 2, "灰度路径上这一页没切开");
    assert_eq!(color_windows.len(), 2, "彩色分支上这一页没切开");
    // 两条路切在同一处、每半裁掉的也一样多：窗口逐字相同。
    assert_eq!(gray_windows, color_windows);
    // 目标尺寸跟着窗口走，两条路因此也一样。
    assert_eq!(gray_sizes, color_sizes);
}

/// **部分救回页不切**（页几何批 04 号票）。
///
/// 它缺的那一段留成纸白（`CONTEXT.md` 的《失败》），而一张缺了右半边的跨页按墨量看
/// 就是「装订沟正好在中间」——切开它等于把「这一页缺了一半」变成两张各自完整的页。
/// 与「部分救回页不裁边」是同一句话的两半：缺的那一段不是白边，也不是装订沟。
#[test]
fn a_salvaged_spread_is_not_cut_at_the_blank_it_could_not_decode() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 截断的跨页：解回来的那一段是画面，救不回来的那一段是纸白。
    volume.file("001.png", &fixtures::truncated(&spread_page()));

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    let page = &reported.pages[0];
    assert!(page.salvage().is_some(), "夹具没造出一张部分救回页");
    assert_eq!(
        reported.page_count(),
        1,
        "把救回页缺的那一段当成装订沟切开了"
    );
    assert!(page.cut().is_none());
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
        // 四边顶着墨：这一条说的是「面板从 profile 来」，裁边不该插一脚（页几何批 02 号票）。
        volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));

        let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(device));

        let page = &report.volumes[0].pages[0];
        assert_eq!(page.size, expected, "{device}");
        assert_eq!(fixtures::read_png(&page.output).size, expected, "{device}");
    }
}

#[test]
fn the_report_gives_the_total_ratio_the_prescale_and_the_residual_of_every_page() {
    let space = Workspace::new();
    let volume = scaling_volume(&space);
    // 期望值手算，面板 1264×1680，`--fit inside`。总缩放比 = 源高 ÷ 目标高：
    //   5056×1680 → 1264×420 ：比 4.000，整数比，预缩一步到位
    //   3160×4200 → 1264×1680：比 2.500，预缩 2 之后残差段还剩 1.250
    //   2528×3360 → 1264×1680：比 2.000，预缩 2，残差 1.000
    //   1441×2048 → 1182×1680：比 1.219，不触发预缩，残差就是总比
    //   800×1000  → 原样      ：比 1.000，两级都没活干
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

    let expected = [
        (4.0, 4, 1.0),
        (2.5, 2, 1.25),
        (2.0, 2, 1.0),
        (1.219, 1, 1.219),
        (1.0, 1, 1.0),
    ];
    assert_scaling(&report, expected);
}

/// **预缩与总缩放比跟着适配方式走**（页几何批 01 号票）。
///
/// 同一批页、同一块面板，只换适配方式：目标高改成恒等于面板高，总缩放比 = 源高 ÷ 面板高
/// 跟着变，预缩那一级于是也可能换一个倍数。分岔只出在两头——中间三页两条路上目标尺寸相同，
/// 三个数因此一个不动。
#[test]
fn the_total_ratio_follows_the_fit_mode() {
    let space = Workspace::new();
    let volume = scaling_volume(&space);
    // 期望值手算，面板 1264×1680，默认的以高为准。目标高恒为 1680：
    //   5056×1680 → 5056×1680：比 1.000，高已经是面板高，两级都没活干
    //   3160×4200 → 1264×1680：比 2.500，与 fit-inside 同一个目标，三个数不动
    //   2528×3360 → 1264×1680：比 2.000，同上
    //   1441×2048 → 1182×1680：比 1.219，同上
    //   800×1000  → 1344×1680：比 0.595，**放大**——比小于 1，预缩自然退化为恒等
    let report = run_volume(&space, &volume);

    let expected = [
        (1.0, 1, 1.0),
        (2.5, 2, 1.25),
        (2.0, 2, 1.0),
        (1.219, 1, 1.219),
        (1000.0 / 1680.0, 1, 1000.0 / 1680.0),
    ];
    assert_scaling(&report, expected);
}

/// 缩放那两条用例共用的卷：五页各落在预缩的一个档上。
fn scaling_volume(space: &Workspace) -> fixtures::Volume {
    let volume = space.volume("volume-a");
    volume.page("01.png", &fixtures::line_art(fixtures::SPREAD));
    // 四页渐变一律四边顶着墨：这两条说的是预缩与总缩放比，而裁边会改掉每一页的源高
    // （页几何批 02 号票），那时手算的期望值量的就不是缩放了。
    volume.page(
        "02.png",
        &fixtures::full_bleed_gradient(fixtures::TWO_AND_A_HALF_PANEL),
    );
    volume.page(
        "03.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );
    volume.page("04.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page(
        "05.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );
    volume
}

/// 逐页比对总缩放比、预缩倍数与残差比。
fn assert_scaling(report: &tonefit::Report, expected: [(f64, u32, f64); 5]) {
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
    volume.page("001.png", &fixtures::cheap_page());
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
    volume.page("002.jpg", &fixtures::line_art(fixtures::PASSES_THROUGH));
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
    volume.page("001.png", &fixtures::cheap_page());
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    assert_eq!(pages.len(), 1);
    assert!(pages[0].source.ends_with("001.png"));
}

#[test]
fn each_volume_mirrors_its_source_tree_under_its_own_output_directory() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();
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
    let page = fixtures::cheap_page();
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
    volume.page("001.png", &fixtures::cheap_page());
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
    volume.page("002.png", &fixtures::line_art(fixtures::PASSES_THROUGH));
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
    // 抖动模式按这一页的几何门裁（ADR 0007）。这一页贴住面板，门放行，
    // 于是每档位深各两个候选。页四边顶着墨，裁边不改它贴住的是哪条边。
    let cases = [(None, 16), (Some(4), 4), (Some(256), 256)];

    for (gray_levels, effective) in cases {
        let space = Workspace::new();
        let volume = space.volume("volume-a");
        volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
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

/// **一张不具代表性的封面不否决整卷的抖动**（06 号票，ADR 0007 决定第 1～3 条）。
///
/// 混合尺寸卷：四页正片贴住面板，一张封面比目标还小。门逐页判，那一张封面只关掉自己
/// 那一页的抖动；另外四页照旧跟着卷级基准档抖。同一卷在从前那套口径下会**整卷**不抖动。
///
/// 三件事一条用例里钉齐，因为它们互为对方的前提：少数页不连坐（第二条验收标准）、
/// 真会被下游缩放的页确实不抖（第三条）、而位深仍按卷统一（ADR 0006 要消灭的翻页跳变
/// 没有跟着回来）。
#[test]
fn one_undersized_cover_does_not_take_the_dither_away_from_the_rest_of_the_volume() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 封面：源比目标小，按不放大原样输出，门在它这里不成立。
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );
    // 四页正片，都贴得住面板。五页一律四边顶着墨：这一条说的是几何门逐页判，
    // 而裁边会改掉每一页的几何（页几何批 02 号票）。
    volume.page(
        "002.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );
    volume.page("003.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page("004.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    volume.page("005.png", &fixtures::full_bleed_gradient(fixtures::SPREAD));

    // 混排卷只在 fit-inside 上是混排卷（页几何批 01 号票）：以高为准会把那张封面放大到
    // 面板高，门跟着成立，一卷五页都拿满候选，「一张封面否决整卷」这件事就无从谈起。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);
    let reported = &report.volumes[0];

    // 判定范围是五页，被排除的只有封面那一张——报告说得出这两个数（06 号票）。
    assert_eq!(reported.judged_by_the_gate().count(), 5);
    let outside: Vec<_> = reported
        .outside_the_gate()
        .map(|page| page.source.file_name().expect("页有名字").to_owned())
        .collect();
    assert_eq!(outside, vec!["001.png"]);

    let base = envelope_of(reported).base;
    assert_eq!(
        base.dither,
        Dither::FloydSteinberg,
        "一张封面把整卷的抖动带走了"
    );
    // 正片各页跟着基准档走，抖动那一维一个不落。
    for page in &reported.pages[1..] {
        assert_eq!(
            fixtures::verdict(page).candidate,
            base,
            "{} 没跟上卷级基准档",
            page.source.display()
        );
    }
    // 封面这一页真的会被阅读器再缩一次：它不抖，理由也说得出为什么。
    let cover = fixtures::verdict(&reported.pages[0]);
    assert_eq!(cover.candidate.dither, Dither::Off);
    assert_eq!(cover.reason, Reason::OutsideTheGate);
    // 门只拿走抖动，不拿走档次（ADR 0007 决定第 3 条）：抖动被拿走之后封面在剩下那套候选里
    // 自己判一次，判出来比基准档高就用它自己那一档——不抖的同一档保真更差，只给基准档会亏着它。
    let threshold = fixtures::baseline_profile().threshold();
    let cover_scores = reported.pages[0].scores();
    let own = cover_scores
        .iter()
        .find(|scored| threshold.admits(scored.score))
        // 一档都不达标就取候选上界兜底——与逐页选档那一条同一个规则。
        .unwrap_or_else(|| cover_scores.last().expect("候选集不会是空的"))
        .candidate;
    assert!(
        own.bit_depth > base.bit_depth,
        "夹具不对：封面自己那一档不比基准档高，这条用例就分不出「取更严的」与「只拿基准档」"
    );
    assert_eq!(
        cover.candidate.bit_depth,
        own.bit_depth.max(base.bit_depth),
        "封面没取更严的那一档"
    );
    // 被裁掉的候选不进入判据：门先判，判据在门放行的那套候选上求值。
    assert!(
        reported.pages[0]
            .scores()
            .iter()
            .all(|scored| scored.candidate.dither == Dither::Off),
        "封面的判据里还留着抖动候选"
    );
    assert!(
        reported.pages[1]
            .scores()
            .iter()
            .any(|scored| scored.candidate.dither == Dither::FloydSteinberg),
        "正片的判据里被连坐掉了抖动候选"
    );
}

/// 每一页都缩下来贴住面板时门就成立，抖动这才跟着位深一起按卷定下
/// （ADR 0007：上包络取的是这个组合，不设页级抖动开关）。
#[test]
fn a_volume_whose_pages_all_land_on_the_panel_keeps_the_gate_open() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::DOUBLE_PANEL),
    );
    volume.page("002.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    // 跨页宽幅页贴住的是宽边，两侧留边换成上下留边，门同样成立。
    volume.page("003.png", &fixtures::full_bleed_gradient(fixtures::SPREAD));

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.judged_by_the_gate().count(), 3);
    assert_eq!(volume_report.outside_the_gate().count(), 0);
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
    // 四边顶着墨，裁边不改它的几何（页几何批 02 号票）。
    volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
    // 二值线稿页：1bit 就装得下，判据因此是零，最低那一档直接过关。
    volume.page(
        "002.png",
        &fixtures::line_art(fixtures::SMALLER_THAN_TARGET),
    );

    let report = tonefit::run(&Request {
        per_page: true,
        // 第二页要贴不住面板才钉得住「另一页的几何不牵连这一页」，而门不成立那一支
        // 只在 fit-inside 上走得到（页几何批 01 号票）。
        fit: FitMode::Inside,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::PerPage),
        "上包络没被关掉"
    );
    let pages = &report.volumes[0].pages;
    // 头一页贴住面板，门在它这里放行（ADR 0007 决定第 1 条：门逐页判）：抖动那一维在场，
    // 2bit+FS 就够得着界。第二页源比目标小，门在它那里不成立——但那一页的事与这一页无关，
    // 从前那套口径下它会把这一页的抖动一并带走，判定跟着退到 4bit 兜底。
    assert_eq!(pages[0].gate(), Some(GeometryGate::Holds));
    assert_eq!(pages[1].gate(), Some(GeometryGate::Broken));
    assert_eq!(
        fixtures::verdict(&pages[0]).candidate,
        Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
        "另一页的几何把这一页的抖动带走了"
    );
    // 第二页会被下游再缩一次，抖动因此不在它的候选里——`--per-page` 也放不开这一维。
    assert_eq!(
        fixtures::verdict(&pages[1]).candidate,
        Candidate::new(BitDepth::One, Dither::Off)
    );
    // 卷级那一层关着，两页的理由因此都是逐页判出来的那一种，而档位差着——
    // 翻页跳变正是 `--per-page` 换回来的东西。
    for page in pages {
        assert_eq!(
            fixtures::verdict(page).reason,
            Reason::LowestWithinThreshold,
            "{} 的理由不是逐页判出来的",
            page.source.display()
        );
    }
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
    volume.page("001.png", &fixtures::line_art(fixtures::PASSES_THROUGH));

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

/// 几何门是**页的**几何事实，不是自动选择：`--dither` 覆盖不了它
/// （ADR 0007：不成立时整体关闭，不降级）。那是互锁 ③，处置是**维持拒绝**
/// （页几何批 05 号票）。
///
/// 门逐页判之后拒绝仍是**整趟**的：覆盖项是用户的显式指令，不是可以按页悄悄放弃的东西
/// （ADR 0007 的《后果》）。撞上的是哪一页因此要说出来——那是唯一能让用户看懂这条拒绝的信息。
///
/// 这一条走 fit-inside；默认那条路上的那一缝在
/// [`on_the_default_fit_the_refusal_stops_offering_a_fit_mode_that_changes_nothing`]。
#[test]
fn a_dither_the_geometry_gate_forbids_is_refused() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 四边顶着墨：门问的是裁完之后的几何，而这一条要的是「源两边都比面板小」本身
    // 关的门，不是裁边替它关的（页几何批 09 号票）。
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let error = tonefit::run(&Request {
        dither: Some(Dither::FloydSteinberg),
        // 这一条走 fit-inside：源两边都比面板小的页在那条路上按不放大原样输出，
        // 一条边都贴不住（页几何批 01 号票）。以高为准上门恒成立，
        // 只有兜底上界退回去的页是例外（见下一条用例，07 号票）。
        fit: FitMode::Inside,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect_err("几何门不成立时点名抖动应当被拒绝");

    let said = error.to_string();
    assert!(said.contains("几何门"), "{said}");
    // 是哪一页关的门要说出来——那是唯一能让用户看懂这条拒绝的信息。
    assert!(format!("{error:#}").contains("001.png"), "{error:#}");
    // 门放宽不了，几何却动得了：换个适配方式这一页就贴住面板了（页几何批 01 号票）。
    // 不说这一句，用户手上只剩「换一批源页」。
    assert!(format!("{error:#}").contains("--fit height"), "{error:#}");
}

/// **以高为准上 `--dither fs` 仍撞得上几何门，而那时那条出路是假话**
/// （页几何批 05 号票的处置 ③，07 号票开的那个例外）。
///
/// 以高为准让每一页的高都等于面板高，门恒成立——除了被兜底上界退回 fit-inside 的那几页：
/// 退回之后它是一张 fit-inside 的页，源两边都比面板小时一条边都贴不住。
/// 这一趟走的正是那条缝。
///
/// 处置仍是**维持拒绝**：覆盖项是显式指令，不按页悄悄放弃。要钉住的是**措辞**——
/// 同一张页在两种适配方式上都被拒，而 fit-inside 那一侧原本无条件劝人「换 --fit height，
/// 门跟着成立」。这一张页正是那句话的反例：换过去照样被拒。两条路因此一起断言，
/// 一条说「以高为准上不再提那个开关」，一条说「fit-inside 上提了，但把例外也说了」。
#[test]
fn on_the_default_fit_the_refusal_stops_offering_a_fit_mode_that_changes_nothing() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 整页纯墨：裁边一个像素都拿不走，走得到的只有兜底那一条。
    volume.page(
        "001.png",
        &fixtures::solid(fixtures::DEGENERATE_STRIP_SMALLER_THAN_PANEL, 0),
    );
    let refused = |fit| {
        format!(
            "{:#}",
            tonefit::run(&Request {
                dither: Some(Dither::FloydSteinberg),
                fit,
                ..fixtures::request(&space, [volume.path()])
            })
            .expect_err("这一页两种适配方式下都贴不住面板，点名抖动应当被拒绝")
        )
    };

    let on_height = refused(FitMode::Height);
    assert!(on_height.contains("几何门"), "{on_height}");
    // 是哪一页关的门照旧说得出来。
    assert!(on_height.contains("001.png"), "{on_height}");
    // **不再劝人换适配方式**：这一趟用的就是以高为准。
    assert!(!on_height.contains("--fit height"), "{on_height}");
    // 换成说清这一页是怎么走到这儿的，以及剩下的那两条路。
    assert!(on_height.contains("兜底上界"), "{on_height}");
    assert!(on_height.contains("不点 --dither fs"), "{on_height}");

    // 同一张页在 fit-inside 上也被拒，那一侧仍指 `--fit height`——但**这一张页是它的例外**，
    // 例外因此得跟着说出来，否则用户照着敲一遍只会撞第二次。
    let on_inside = refused(FitMode::Inside);
    assert!(on_inside.contains("--fit height"), "{on_inside}");
    assert!(on_inside.contains("兜底上界"), "{on_inside}");
    assert!(on_inside.contains("仍是这条拒绝"), "{on_inside}");
}

#[test]
fn a_bit_depth_the_panel_cannot_show_is_refused() {
    // 灰阶数是硬上界（ADR 0003）：覆盖的是自动判定，不是上界。
    // 上界只有 `--gray-levels` 动得了，错误信息因此必须指向它。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::cheap_page());

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
    volume.page("001.png", &fixtures::full_bleed_gradient(fixtures::TYPICAL));
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
fn the_threshold_says_where_it_came_from() {
    // spec 的 Further Notes：报告要自己说出一个数是怎么来的。阈值标定之后这条不是消失了，
    // 是换了内容——它现在要说出标定在哪块面板上做的，以及本机这块有没有复核。
    let said = fixtures::baseline_profile().to_string();
    assert!(said.contains("标定"), "{said}");
    assert!(
        said.contains("boox-poke6"),
        "没说出标定在哪块面板上做的：{said}"
    );
    assert!(
        said.contains("未复核"),
        "没说出其余面板沿用同一个数：{said}"
    );

    // 点名覆盖的那一种要与内置值分得开：读的人得知道这个数是谁定的。
    let pinned = fixtures::baseline_profile()
        .with_threshold(2.0)
        .expect("2.0 在界的取值范围内")
        .to_string();
    assert!(pinned.contains("阈值 2.000（命令行指定）"), "{pinned}");
}

#[test]
fn every_page_is_decoded_exactly_once() {
    // 两遍管线的不变量（ADR 0005：解码一次，缓存缩放后的图）：第一遍解码，
    // 第二遍只从缓存读。计数是这条不变量在 `run` 这个 seam 上看得见的形式——
    // 第二遍一旦回头碰源页，这个数立刻大于页数。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::line_art(fixtures::PASSES_THROUGH));
    volume.page("003.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));
    // 透传文件不解码，也就不计数。
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let report = run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.pages.len(), 3);
    assert_eq!(volume_report.decodes, 3, "源页被解码了不止一遍");
}

/// 报告把**源页数与输出页数分开**说，两者眼下相等（页几何批 03 号票）。
///
/// 一个源页可以产出多张输出页，两个数从此不是同一个。跳过的卷同样答得出源页数——
/// 那是源枚举就数得出的事实，不做工作也在。
#[test]
fn the_report_counts_source_pages_and_output_pages_separately() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.page("002.png", &fixtures::line_art(fixtures::TYPICAL));
    // 透传文件既不是源页也不是输出页。
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    let done = run_volume(&space, &volume);

    let processed = &done.volumes[0];
    assert_eq!(processed.source_pages, 2);
    assert_eq!(processed.page_count(), 2, "输出页数");
    // 逐页结果是**输出**那一侧的，与 `page_count()` 只有一个出处。
    assert_eq!(processed.pages.len(), processed.page_count());

    // 第二趟幂等命中：一页都没重做，两个数照样答得出来。
    let skipped = run_volume(&space, &volume);
    let skipped = &skipped.volumes[0];
    assert!(skipped.skipped(), "第二趟该被跳过");
    assert!(skipped.pages.is_empty(), "跳过的卷没有逐页结果");
    assert_eq!(skipped.source_pages, 2);
    assert_eq!(skipped.page_count(), 2, "输出页数");
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

/// 卷级用例的合成卷：`levels` 里每一项造一页，页名按阅读顺序编号。页用 `fixtures::TINY`，
/// 因此只在 **fit-inside** 上还是小页。
///
/// 造页一律用纯色页：判据在纯色页上算得出准数——量化误差就是取值到格点的距离，
/// 低通与掩蔽加权都不改它。逐页判定因此由取值直接定死，卷级那一层要的正是一条排得开的分布。
fn volume_of_solids(space: &Workspace, levels: &[u8]) -> fixtures::Volume {
    volume_of_solids_sized(space, fixtures::TINY, levels)
}

/// 同上，但点名页尺寸。**默认那条路上的卷级用例走这一条**（08 号票）：
/// `fixtures::TINY` 在以高为准下会被放大到面板高，一卷六十页的代价涨两个数量级，
/// 而 `fixtures::NARROW_PASSES_THROUGH` 两种适配方式下都恒等通过。
fn volume_of_solids_sized(space: &Workspace, size: Size, levels: &[u8]) -> fixtures::Volume {
    let volume = space.volume("volume-a");
    for (index, &level) in levels.iter().enumerate() {
        volume.page(
            &format!("{:03}.png", index + 1),
            &fixtures::solid(size, level),
        );
    }
    volume
}

// 卷级用例造分布用的三个纯色取值在 `fixtures` 那一侧（`NEEDS_TWO_BITS`、
// `ONE_STEP_ABOVE_TWO_BITS`、`FAR_OUTSIDE`）：黄金回归那个 crate 用的是同一批数，
// 出处只留一个（08 号票）。

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
            &fixtures::solid(fixtures::TINY, fixtures::NEEDS_TWO_BITS),
        );
    }

    // 小页夹具只在 fit-inside 上还是小页（页几何批 01 号票）：以高为准会把每一页
    // 放大到面板高，而这几条问的是上包络、离群与迟滞，与几何无关。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

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
    let mut levels = vec![fixtures::NEEDS_TWO_BITS; 19];
    levels.push(fixtures::FAR_OUTSIDE);
    let volume = volume_of_solids(&space, &levels);

    // 小页夹具只在 fit-inside 上还是小页（页几何批 01 号票）：以高为准会把每一页
    // 放大到面板高，而这几条问的是上包络、离群与迟滞，与几何无关。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

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
    let mut levels = vec![fixtures::NEEDS_TWO_BITS; 18];
    levels.extend([fixtures::FAR_OUTSIDE; 2]);
    let volume = volume_of_solids(&space, &levels);

    // 小页夹具只在 fit-inside 上还是小页（页几何批 01 号票）：以高为准会把每一页
    // 放大到面板高，而这几条问的是上包络、离群与迟滞，与几何无关。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

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

    // 小页夹具只在 fit-inside 上还是小页（页几何批 01 号票）：以高为准会把每一页
    // 放大到面板高，而这几条问的是上包络、离群与迟滞，与几何无关。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

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
    let mut levels = vec![fixtures::NEEDS_TWO_BITS; 19];
    levels.push(fixtures::FAR_OUTSIDE);

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

    // 同上：小页只在 fit-inside 上还是小页。
    let report = tonefit::run(&Request {
        profile: fixtures::profile(COLOR_DEVICE),
        fit: FitMode::Inside,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    report.volumes.into_iter().next().expect("一个卷")
}

/// 一台彩色面板设备：彩页只有在彩色 profile 下才走彩色分支（ADR 0010）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

#[test]
fn a_sustained_run_raises_the_depth_but_one_page_short_of_it_does_not() {
    // 迟滞限制档位切换频率：一页说了不算，连续够了才升档（ADR 0006 决定第 4 条）。
    // 两个卷除了要求更高的那一段长了一页，其余完全相同。
    // 这一条跑在 **fit-inside** 上，几何门在每一页上都不成立，候选集里没有抖动那一维——
    // 升的因此是位深。默认那条路由下面那一条守着（08 号票），两条路升的不是同一档。
    let raised = run_with_a_run_of_fitted_inside(3);
    let unchanged = run_with_a_run_of_fitted_inside(2);

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

/// 六十页的取值：第 31 页起 `length` 页要求高于基准档，其余都是主体页。
///
/// **六十页是 p95 撑得住一段三页的最小规模**：秩 `ceil(0.95n)` 要落在主体页上，
/// 即 `ceil(0.95n) ≤ n-3`，解得 n ≥ 60；60 页时秩是 57，基准档之上还剩得下三页。
/// 这条不等式数的是页在判定序列上的**名次**，与候选集是三个还是六个无关——
/// 两条适配方式因此喂同一串取值，差别全在页尺寸上（08 号票）。
fn levels_with_a_run_of(length: usize) -> Vec<u8> {
    let mut levels = vec![fixtures::NEEDS_TWO_BITS; 60];
    levels[30..30 + length].fill(fixtures::ONE_STEP_ABOVE_TWO_BITS);
    levels
}

/// [`levels_with_a_run_of`] 那一卷跑在 **fit-inside** 上。
fn run_with_a_run_of_fitted_inside(length: usize) -> tonefit::VolumeReport {
    let space = Workspace::new();
    let volume = volume_of_solids(&space, &levels_with_a_run_of(length));

    // 小页夹具只在 fit-inside 上还是小页（页几何批 01 号票）：以高为准会把每一页
    // 放大到面板高，而这几条问的是上包络、离群与迟滞，与几何无关。
    let report = fixtures::run_volume_fitted_inside(&space, &volume);

    report.volumes.into_iter().next().expect("一个卷")
}

/// **迟滞升档在默认适配方式上也真跑一遍**（08 号票，了结停车场 Q6）。
///
/// ADR 0006 决定第 4 条是这个产品的核心行为之一，而 `page-geometry/01` 把默认适配方式
/// 换成以高为准之后，喂它的那几个长卷全留在了 `--fit inside` 上——上面那一条端到端用例
/// 因此从没在用户默认走的那条路上跑过。`src/envelope.rs` 那一层的单元用例不碰几何，
/// 丢的不是覆盖，是「默认这一趟真跑一遍」。
///
/// **两条路升的不是同一档，所以两条都要留着。** 这一趟的页两种适配方式下都恒等通过，
/// 几何门于是**每一页都成立**（ADR 0007 决定第 1 条），候选集多出抖动那一维：
/// 基准档是 `2bit`，而基准档过不了界的那一段升到 `2bit+FS`——**升的是抖动，不是位深**。
/// fit-inside 那条路上同一批取值升的是 `4bit`，而且走的是另一套机制（兜底取候选上界）：
/// 两条路各自的判据读数与机制，出处是 `fixtures` 的 `ONE_STEP_ABOVE_TWO_BITS`。
#[test]
fn a_sustained_run_raises_that_stretch_on_the_default_fit_but_one_page_short_does_not() {
    let raised = run_with_a_run_of_on_the_default_fit(3);
    let unchanged = run_with_a_run_of_on_the_default_fit(2);

    // 门在每一页上都成立——这一条与上面那一条的全部差别就在这里。
    assert!(
        raised
            .pages
            .iter()
            .all(|page| page.gate() == Some(GeometryGate::Holds)),
        "夹具不对：这一卷有页落在门外，候选集就不是六个那一套了"
    );

    let base = Candidate::new(BitDepth::Two, Dither::Off);
    let stretch = Candidate::new(BitDepth::Two, Dither::FloydSteinberg);
    assert_eq!(envelope_of(&raised).base, base);
    assert_eq!(envelope_of(&raised).raised_pages, 3);
    for page in &raised.pages[30..33] {
        assert_eq!(fixtures::verdict(page).candidate, stretch);
        assert_eq!(fixtures::verdict(page).reason, Reason::Hysteresis);
    }
    // 段外的页一页不动：升的是那一段，不是全卷。
    assert_eq!(fixtures::verdict(&raised.pages[29]).candidate, base);
    assert_eq!(fixtures::verdict(&raised.pages[33]).candidate, base);

    // 差一页就不算持续，默认路径上同样如此。
    assert_eq!(envelope_of(&unchanged).raised_pages, 0);
    for page in &unchanged.pages {
        assert_eq!(
            fixtures::verdict(page).candidate,
            base,
            "{} 靠不足一段的要求升了档",
            page.source.display()
        );
    }
}

/// 同一卷跑在**默认适配方式**上：取值一个不改，只把页换成两种适配方式下都恒等通过的
/// 那一个，几何门因此每一页都成立。
fn run_with_a_run_of_on_the_default_fit(length: usize) -> tonefit::VolumeReport {
    let space = Workspace::new();
    let volume = volume_of_solids_sized(
        &space,
        fixtures::NARROW_PASSES_THROUGH,
        &levels_with_a_run_of(length),
    );

    let report = run_volume(&space, &volume);

    report.volumes.into_iter().next().expect("一个卷")
}

#[test]
fn an_override_leaves_no_volume_envelope_to_speak_of() {
    // `--bit-depth` 顶掉的是判定本身，卷级基准档因此无从谈起——理由仍分得清是覆盖。
    let space = Workspace::new();
    let volume = volume_of_solids(&space, &[fixtures::NEEDS_TWO_BITS; 4]);

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Four),
        // 门不成立时候选集里没有抖动那一维，`--bit-depth` 一点名就只剩一个候选，
        // 判定整个被顶掉——这一条要的正是那个局面（页几何批 01 号票）。
        fit: FitMode::Inside,
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

/// 同名的两个卷会写到同一个地方，后到的把先到的整卷盖掉。开工前就要拒。
///
/// 卷名重复在真实素材上是常态：一部漫画一个目录，每部里都有「第 1 话」。
/// 一次点名多部就撞在一起，而覆盖掉的那一卷在阅读器里与真卷毫无分别——
/// 静默是这条缺陷最要命的地方。
#[test]
fn two_volumes_that_would_write_to_the_same_place_are_refused() {
    let space = Workspace::new();
    let first = space.volume("甲部/第1话");
    first.page("001.png", &fixtures::gradient(fixtures::TINY));
    let second = space.volume("乙部/第1话");
    second.page("001.png", &fixtures::line_art(fixtures::TINY));

    let error =
        fixtures::run_paths_expecting_failure(&space, [first.path(), second.path()]).to_string();

    // 撞在一起的两个卷都要点名：只说「撞车了」，用户无从知道是哪两卷。
    assert!(error.contains("甲部"), "{error}");
    assert!(error.contains("乙部"), "{error}");
    // 拒绝要发生在写出第一个字节之前，输出根因此根本不该被建出来。
    assert!(
        !space.out().exists(),
        "拒之前已经动过输出根：撞车没能在开工前查出来"
    );
}

/// 一个目录卷和一个归档卷即使卷名相同也不撞：去处一个是 `名字`、一个是 `名字.cbz`。
///
/// 这条是上一条的反面。查撞车比的是**去处**，不是卷名——比卷名会把这一对误判成撞车。
#[test]
fn a_directory_and_an_archive_of_the_same_name_do_not_collide() {
    let space = Workspace::new();
    let directory = space.volume("第1话");
    directory.page("001.png", &fixtures::gradient(fixtures::TINY));
    let mut archive = space.cbz("第1话");
    archive.page("001.png", &fixtures::line_art(fixtures::TINY));
    let archive = archive.write();

    let report = fixtures::run_paths(&space, [directory.path(), archive.as_path()]);

    assert_eq!(report.volumes.len(), 2);
    assert_ne!(report.volumes[0].output, report.volumes[1].output);
}
