//! `run(Request) -> Report` 这个 seam 上的行为测试。
//!
//! 只断言外部可见的事实：`Report` 里的内容、写出的文件有什么性质、源目录有没有被动过。

mod fixtures;

use fixtures::{Workspace, run_volume};
use tonefit::{BitDepth, Mode, Request, Size};

#[test]
fn each_page_becomes_a_gray8_png_at_the_target_size() {
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
    assert_eq!(written.color_type, png::ColorType::Grayscale);
    assert_eq!(written.bit_depth, png::BitDepth::Eight);
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

    let report = run_volume(&space, &volume);

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

    let report = run_volume(&space, &volume);

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

    let report = run_volume(&space, &volume);

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
fn a_solid_page_stays_solid_after_scaling() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::solid(fixtures::DOUBLE_PANEL, 200));

    let report = run_volume(&space, &volume);

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
    let page = fixtures::screentone(fixtures::DOUBLE_PANEL);
    volume.page("001.png", &page);
    assert_eq!(
        distinct_levels(page.to_luma8().as_raw()),
        2,
        "网点页必须是二值的"
    );

    let report = run_volume(&space, &volume);

    // 网点是因，灰调是果：缩放把点阵解析成连续灰调。
    let written = fixtures::read_png(&report.volumes[0].pages[0].output);
    let levels = distinct_levels(&written.pixels);
    assert!(levels > 16, "缩放后只剩 {levels} 级灰调");
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
        assert!(!page.scores.is_empty(), "{} 缺判据", page.source.display());
        assert!(
            !page.output.exists(),
            "{} 被写出来了",
            page.output.display()
        );
    }
    // 渐变页在低位深上必然崩：报告里的数是真算出来的，不是一排零。
    let gradient = &pages[0].scores;
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
    // 灰阶数是位深的硬上界，裁剪在判据求值之前（ADR 0003）：e-ink 恒 16 级，8bit 不进候选。
    let cases = [
        (None, vec![BitDepth::One, BitDepth::Two, BitDepth::Four]),
        (Some(4), vec![BitDepth::One, BitDepth::Two]),
        (Some(256), BitDepth::ALL.to_vec()),
    ];

    for (gray_levels, expected) in cases {
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

        let depths: Vec<_> = report.volumes[0].pages[0]
            .scores
            .iter()
            .map(|scored| scored.bit_depth)
            .collect();
        assert_eq!(depths, expected, "{gray_levels:?} 级灰阶下的候选不对");
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

    // 报告先给出的路径与尺寸，就是照做之后真正落盘的那一份（spec 的 story 6）。
    let page = (&predicted.volumes[0].pages[0], &written.volumes[0].pages[0]);
    assert_eq!(page.0.output, page.1.output);
    assert_eq!(page.0.size, page.1.size);
    assert!(page.1.output.is_file());
}
