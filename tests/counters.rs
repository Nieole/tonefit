//! 窄计数器：报告上那几个只为「某件工作确实没有发生」而在的数。
//!
//! 接缝是 `run(Request) -> Report`，与别的行为用例同一条。这里的断言与别处不同的只有一点：
//! 它们钉的不是用户看得见的东西，而是**看不见的工作量**。屏上一处不露面的数因此需要
//! 一个自己的家——不然后面几票把某一趟的白付删掉时，除了「行为不变」再没有别的说法。
//!
//! 三个数：解码次数（既有，`VolumeReport::decodes`）、缩放次数、参照进缓存次数。
//! 解码那一个的断言散在 `idempotency`、`resume`、`concurrency` 几处，跟着它们各自那条性质走；
//! 这里只收另外两个，以及三个数之间那几条互相说明的关系。

mod fixtures;

use fixtures::Workspace;

/// 一台彩色面板设备：彩页只有在彩色 profile 下才走彩色分支（ADR 0010）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

/// 灰度卷上三个数说的是同一批页：解码按**源页**数，缩放与参照进缓存按**输出页**数，
/// 而这一卷没有跨页，三者相等。
#[test]
fn every_page_of_a_gray_volume_is_resized_once_and_cached_once() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    let report = fixtures::run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.pages.len(), 2);
    assert_eq!(volume_report.decodes, 2);
    assert_eq!(volume_report.resizes, 2, "每张输出页缩放一次");
    assert_eq!(
        volume_report.cached_references, 2,
        "灰度路径上每张输出页存一份参照"
    );
}

/// 幂等命中的卷两个数都是零。「不重复工作」在这一趟上量得出来的形式又多了两个——
/// 从前只有解码那一个（`tests/idempotency.rs`）。
#[test]
fn a_skipped_volume_resizes_nothing_and_caches_nothing() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));

    fixtures::run_volume(&space, &volume);
    let second = fixtures::run_volume(&space, &volume);

    let skipped = &second.volumes[0];
    assert_eq!(skipped.decodes, 0, "跳过的卷还是解码了");
    assert_eq!(skipped.resizes, 0, "跳过的卷还是缩放了");
    assert_eq!(skipped.cached_references, 0, "跳过的卷还是存了参照");
}

/// 彩色面板上彩页走彩色分支：**照样缩一次**，但一张参照都不存。
///
/// 两个数在这一卷上第一次分家，而分家处正是 05 号票要动的地方：那一趟的缩放结果只有编码
/// 一个消费者，试算不编码，那一整套预缩加卷积因此是白付。今天它是 1，05 之后是 0——
/// 没有这个数，那张票只能靠「报告里那几格没变」兜着。
///
/// **一张彩页记一次，不是三次**：三个通道各走一遍是一张图的内部构造。这一条同时钉住它——
/// 按调用次数记的话，这里会是 4。
#[test]
fn a_color_page_is_resized_once_and_never_cached() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(COLOR_DEVICE));

    let volume_report = &report.volumes[0];
    assert_eq!(
        volume_report.pages[0].color(),
        Some(tonefit::PageColor::Color)
    );
    assert_eq!(volume_report.decodes, 2);
    assert_eq!(
        volume_report.resizes, 2,
        "彩页与灰度页各缩一次——彩页那三个通道是一张图，不是三张"
    );
    assert_eq!(
        volume_report.cached_references, 1,
        "彩色分支不进灰度缓存（ADR 0005 决定第 4 条）"
    );
}

/// 试算与照做两趟的三个数**逐个相同**：dry-run 省掉的只有写出那一头。
///
/// 05 与 06 两票动的都是「某个开关关掉之后第一遍该少做点什么」，而它们的对照组就是这一条——
/// 今天两趟一样多，改完之后差的正好是那一票删掉的那一份。
#[test]
fn a_dry_run_does_exactly_as_much_first_pass_work_as_the_real_thing() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    // 试算排在前头：照做那一趟写下了输出，跟在它后面的试算会被幂等整卷跳过。
    let profile = fixtures::profile(COLOR_DEVICE);
    let trial = tonefit::run(&tonefit::Request {
        profile: profile.clone(),
        mode: tonefit::Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("试算应当成功");
    let done = fixtures::run_volume_with(&space, &volume, profile);

    let (done, trial) = (&done.volumes[0], &trial.volumes[0]);
    assert_eq!(trial.decodes, done.decodes);
    assert_eq!(
        trial.resizes, done.resizes,
        "试算的缩放次数与照做那一趟不同"
    );
    assert_eq!(
        trial.cached_references, done.cached_references,
        "试算存的参照份数与照做那一趟不同"
    );
}

/// 一张源页切成两张：解码按**源页**数，缩放与参照进缓存按**输出页**数。
///
/// 三个数量的不是同一侧，这一卷把差别摆开——切开发生在解码之后、缩放之前
/// （页几何批 03 号票）。
#[test]
fn a_split_source_page_is_decoded_once_and_resized_twice() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::spread_with_gutter(
            fixtures::SPREAD_WITH_GUTTER,
            fixtures::GUTTER_CENTER,
            fixtures::GUTTER_WIDTH,
        ),
    );

    let report = fixtures::run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.source_pages, 1);
    assert_eq!(volume_report.pages.len(), 2);
    assert_eq!(volume_report.decodes, 1, "切开发生在解码之后，源页只解一次");
    assert_eq!(volume_report.resizes, 2, "两半各缩各的");
    assert_eq!(volume_report.cached_references, 2, "两半各存各的参照");
}

/// **判定被覆盖顶死的那一趟，参照今天照进缓存。**
///
/// 位深与抖动两维都点名之后，第二遍要用的那一档在开卷之前就定死了，缓存那一趟往返
/// 什么也不改变——06 号票删的就是它。今天这个数不是零，删完之后是零；
/// 而缩放那一个两边都不动，正好把「删掉的只是缓存那一趟」框住。
#[test]
fn a_pinned_verdict_still_caches_every_reference_today() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    let report = tonefit::run(&tonefit::Request {
        bit_depth: Some(tonefit::BitDepth::Two),
        dither: Some(tonefit::Dither::FloydSteinberg),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.resizes, 2);
    assert_eq!(
        volume_report.cached_references, 2,
        "两维都顶死了，参照还是等到了第二遍"
    );
}

/// **满核并行之下两个数仍然准。**页乱序算完，而缩放那一个是锁外的原子加。
///
/// 照 `tests/concurrency.rs` 的 `every_page_is_still_decoded_exactly_once_when_reading_concurrently`
/// 办：够长的卷才让几条读取线程真的互相错开，一两页的卷问不出这件事。
/// 参照那一个走的是缓存那把锁，不会丢，这里一并钉住。
#[test]
fn both_counters_are_exact_when_the_first_pass_runs_on_every_core() {
    const PAGES: usize = 24;
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 页窄而高：卷级那几条路径不看页上画着什么，窄到只够铺开两块判据分块就行。
    let size = fixtures::NARROW_PASSES_THROUGH;
    for index in 0..PAGES {
        volume.page(
            &format!("{index:03}.png"),
            &fixtures::full_bleed_gradient(size),
        );
    }

    let report = tonefit::run(&tonefit::Request {
        io_mode: tonefit::IoMode::Concurrent,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.pages.len(), PAGES);
    assert_eq!(volume_report.resizes, PAGES, "并发之下缩放次数少记了");
    assert_eq!(
        volume_report.cached_references, PAGES,
        "并发之下参照进缓存次数少记了"
    );
}
