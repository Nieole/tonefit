//! 窄计数器：报告上那几个只为「某件工作确实没有发生」而在的数。
//!
//! 接缝是 `run(Request) -> Report`，与别的行为用例同一条。这里的断言与别处不同的只有一点：
//! 它们钉的不是用户看得见的东西，而是**看不见的工作量**。屏上一处不露面的数因此需要
//! 一个自己的家——不然后面几票把某一趟的白付删掉时，除了「行为不变」再没有别的说法。
//!
//! 三个数：解码次数（既有，`VolumeReport::decodes`）、缩放次数、参照进缓存次数。
//! 解码那一个的断言散在 `idempotency`、`resume`、`concurrency` 几处，跟着它们各自那条性质走；
//! 这里只收另外两个，以及三个数之间那几条互相说明的关系。
//!
//! **参照进缓存那一个分两条路读**：上包络那条路（`--envelope`）上基准档要看完整卷才定得下，
//! 每张灰度输出页存一份参照；默认那条路（逐页，ADR 0018）上一页判完当场量化编码，
//! **参照一张都不进缓存**（spec 的 P-B）。问「按输出页数存参照」的用例因此开着上包络跑。

mod fixtures;

use fixtures::{Volume, Workspace};
use tonefit::{BitDepth, Candidate, Dither, Profile, Reason, VolumeReport, VolumeVerdict};

/// 一台彩色面板设备：彩页只有在彩色 profile 下才走彩色分支（ADR 0010）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

/// 对一个目录卷跑一趟**试算**，点名 profile。
///
/// 这三个数与试算的交集有好几条（05 与 06 两票都落在这里），而 `fixtures` 那一侧
/// 只有照做那一趟的入口（`run_volume_with`）。
fn dry_run_with(space: &Workspace, volume: &Volume, profile: Profile) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        profile,
        mode: tonefit::Mode::DryRun,
        ..fixtures::request(space, [volume.path()])
    })
    .expect("试算应当成功")
}

/// 跑一趟，把这一卷的报告取出来。
///
/// 顶死那几条用例各改各的参数（06 号票的三条来路各点各的那一维），收的因此是拼好的
/// `Request` 而不是一串参数。
fn one_volume(request: tonefit::Request) -> VolumeReport {
    tonefit::run(&request)
        .expect("处理应当成功")
        .volumes
        .into_iter()
        .next()
        .expect("一个卷")
}

/// 两页灰度卷，页恒等通过、几何门两条边都贴着面板。
///
/// 顶死那几条用例共用它：三条来路的差别全在**参数**上，卷一模一样才比得出那件事。
fn two_gray_pages(space: &Workspace) -> Volume {
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));
    volume
}

/// 顶死的那一趟该长什么样（06 号票）：候选在碰卷之前就裁到只剩 `candidate`，
/// 第二遍要用的那一档因此已经定死，量化与编码第一遍当场做完。
///
/// 四条断言各钉票面的一条：
///
/// - **参照一张都不进缓存**——票面第 1 条，也是这一票唯一的外部信号。
/// - **缓存里躺着的就是写出去的那几页字节**——账上那个数恰好等于写出的字节数，
///   「往返」那一趟因此真的省掉了，而不是换了个地方做。
/// - **判据曲线照旧求**——票面明写要保住的那一条。覆盖了判定也照求判据值，
///   试算才说得出「你点的那一档判据是多少」；候选只剩一个，那一个的值一格不少。
/// - **判定理由仍是「覆盖项顶掉判定」**——写进 tEXt 的正是它（`metadata::reason_text`），
///   理由变了输出字节就变了。
fn assert_the_pinned_volume_never_cached_a_reference(volume: &VolumeReport, candidate: Candidate) {
    assert_eq!(
        volume.verdict,
        Some(VolumeVerdict::Override(candidate)),
        "这一趟的判定没有被顶死，测的就不是 06 号票那条路"
    );
    assert_eq!(volume.resizes, volume.pages.len(), "缩放照旧每张一次");
    assert_eq!(
        volume.cached_references, 0,
        "顶死的那一趟还是把参照存进了缓存"
    );
    assert_eq!(
        volume.cache.pages,
        volume.pages.len(),
        "缓存里该躺着编好的那几页"
    );
    for page in &volume.pages {
        assert_eq!(page.scores().len(), 1, "顶死之后判据曲线不求了");
        assert_eq!(page.scores()[0].candidate, candidate);
        assert_eq!(
            page.verdict().expect("灰度页有判定").reason,
            Reason::Override
        );
    }
    let written: u64 = volume
        .pages
        .iter()
        .map(|page| std::fs::metadata(&page.output).expect("读回写出的页").len())
        .sum();
    assert_eq!(
        volume.cache.stored, written,
        "缓存里装的不是编好的那几页——那一趟往返还在"
    );
}

/// 上包络那条路上灰度卷的三个数说的是同一批页：解码按**源页**数，缩放与参照进缓存按
/// **输出页**数，而这一卷没有跨页，三者相等。
#[test]
fn every_page_of_a_gray_volume_is_resized_once_and_cached_once_under_the_envelope() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    let report = fixtures::run_volume_under_the_envelope(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.pages.len(), 2);
    assert_eq!(volume_report.decodes, 2);
    assert_eq!(volume_report.resizes, 2, "每张输出页缩放一次");
    assert_eq!(
        volume_report.cached_references, 2,
        "上包络那条路上每张灰度输出页存一份参照"
    );
}

/// **默认那条路（逐页）上参照一张都不进缓存**（spec 的 P-B；ADR 0018 之后窗口长度是 0）。
///
/// 一页的档只取决于它自己，判据一出来就定了：量化与编码第一遍当场做完，缓存那一格从头
/// 装的就是编好的字节——与覆盖顶死那一趟同一条路（06 号票），只是那一档是判出来的、
/// 不是被顶掉的。四条断言与 [`assert_the_pinned_volume_never_cached_a_reference`] 同形：
/// 缩放照旧每张一次；参照零份；缓存里躺着的就是写出去的那几页字节；理由是逐页判出来的那一种。
#[test]
fn the_default_path_caches_no_reference_and_encodes_each_page_as_it_is_decided() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::full_bleed_gradient(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    let report = fixtures::run_volume(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(
        volume_report.verdict,
        Some(VolumeVerdict::PerPage),
        "默认走到了上包络，测的就不是逐页那条路"
    );
    assert_eq!(volume_report.resizes, 2, "缩放照旧每张一次");
    assert_eq!(
        volume_report.cached_references, 0,
        "默认那条路上参照还是进了缓存——那一趟往返白付"
    );
    assert_eq!(
        volume_report.cache.pages,
        volume_report.pages.len(),
        "缓存里该躺着编好的那几页"
    );
    for page in &volume_report.pages {
        assert_eq!(
            page.verdict().expect("灰度页有判定").reason,
            Reason::LowestWithinThreshold,
            "{} 的档不是它自己判出来的",
            page.source.display()
        );
    }
    let written: u64 = volume_report
        .pages
        .iter()
        .map(|page| std::fs::metadata(&page.output).expect("读回写出的页").len())
        .sum();
    assert_eq!(
        volume_report.cache.stored, written,
        "缓存里装的不是编好的那几页——那一趟往返还在"
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
/// 两个数在这一卷上第一次分家，而分家处正是 05 号票动过的地方：那一趟的缩放结果只有编码
/// 一个消费者。**照做这一趟要编码，缩放因此照旧跑**——同一张页在试算里已经一次都不缩
/// （见 `a_dry_run_of_an_all_color_volume_resizes_nothing`）。没有这个数，那张票就只能
/// 靠「报告里那几格没变」兜着。
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

    // 开着上包络：灰度那一页要存参照，「彩页不存」才比得出来。
    let report = fixtures::run_volume_under_the_envelope_with(
        &space,
        &volume,
        fixtures::profile(COLOR_DEVICE),
    );

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

/// **全彩卷上试算的缩放次数降到零**（05 号票的正题）。
///
/// 彩色分支的缩放结果只有编码一个消费者，而试算不编码——那一整趟三平面的预缩加卷积
/// 跑完就当场丢掉。报告要的缩放比只靠源尺寸与目标尺寸做算术，不需要像素。
///
/// **卷得全彩才问得出「零」**：`resizes` 是卷级合计、不分彩灰，混合卷上灰度页那一次
/// 照旧记着（见 `a_dry_run_skips_only_the_color_resize_of_a_mixed_volume`）。
#[test]
fn a_dry_run_of_an_all_color_volume_resizes_nothing() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::color_page(size));

    let trial = dry_run_with(&space, &volume, fixtures::profile(COLOR_DEVICE));

    let volume_report = &trial.volumes[0];
    assert_eq!(volume_report.pages.len(), 2);
    for page in &volume_report.pages {
        assert_eq!(page.color(), Some(tonefit::PageColor::Color));
    }
    // 解码那一个不跟着降：彩页识别排在解码之后，不解就认不出这是一张彩页。
    assert_eq!(volume_report.decodes, 2, "试算连解码都省了，那就认不出彩页");
    assert_eq!(volume_report.resizes, 0, "试算为它根本不会编码的彩页缩了图");
    assert_eq!(
        volume_report.cached_references, 0,
        "彩色分支不进灰度缓存（ADR 0005 决定第 4 条）"
    );
}

/// 混合卷上试算与照做只差**彩页那一次缩放**，另两个数逐个相同。
///
/// 05 与 06 两票动的都是「某个开关关掉之后第一遍该少做点什么」，这一条是它们的对照组：
/// 05 之后差的正好是彩页那一张，而灰度页照旧要缩——判据要缩放结果，
/// 试算存在的理由正是预告那个判定。06 号票动的是第三个数。
#[test]
fn a_dry_run_skips_only_the_color_resize_of_a_mixed_volume() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::full_bleed_gradient(size));

    // 试算排在前头：照做那一趟写下了输出，跟在它后面的试算会被幂等整卷跳过。
    // 开着上包络：默认那条路上照做不存参照、试算照旧存（试算没有第二遍要那些字节），
    // 第三个数在那条路上本来就不同（停车场 Q430、Q537）。
    let profile = fixtures::profile(COLOR_DEVICE);
    let trial = tonefit::run(&tonefit::Request {
        profile: profile.clone(),
        mode: tonefit::Mode::DryRun,
        envelope: true,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("试算应当成功");
    let done = fixtures::run_volume_under_the_envelope_with(&space, &volume, profile);

    let (done, trial) = (&done.volumes[0], &trial.volumes[0]);
    assert_eq!(trial.decodes, done.decodes);
    assert_eq!(done.resizes, 2, "照做那一趟两张页各缩一次");
    assert_eq!(trial.resizes, 1, "试算该只剩灰度那一张要缩");
    assert_eq!(
        trial.cached_references, done.cached_references,
        "试算存的参照份数与照做那一趟不同"
    );
}

/// **黑白面板上试算照旧缩每一张彩页。**那上面彩页转灰、走灰度路径，
/// 而灰度路径的缩放结果有判据这个消费者，试算正是为了预告它。
///
/// 省掉的是彩色分支那一条路，不是「试算」这个开关——分流由面板定（ADR 0010 决定第 3 条）。
/// 同一卷、同一个模式，只换一台设备，两个数就都回来了。
#[test]
fn a_dry_run_on_a_monochrome_panel_still_resizes_every_color_page() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let size = fixtures::PASSES_THROUGH;
    volume.page("001.png", &fixtures::color_page(size));
    volume.page("002.png", &fixtures::color_page(size));

    let trial = dry_run_with(&space, &volume, fixtures::baseline_profile());

    let volume_report = &trial.volumes[0];
    // 识别与分流分开：这两张在黑白面板上仍旧是彩页，只是走了灰度路径。
    for page in &volume_report.pages {
        assert_eq!(page.color(), Some(tonefit::PageColor::Color));
        assert!(page.verdict().is_some(), "转灰之后该有判定");
    }
    assert_eq!(volume_report.resizes, 2, "转灰那一路上的缩放被一起省掉了");
    assert_eq!(volume_report.cached_references, 2, "转灰之后照旧存参照");
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

    let report = fixtures::run_volume_under_the_envelope(&space, &volume);

    let volume_report = &report.volumes[0];
    assert_eq!(volume_report.source_pages, 1);
    assert_eq!(volume_report.pages.len(), 2);
    assert_eq!(volume_report.decodes, 1, "切开发生在解码之后，源页只解一次");
    assert_eq!(volume_report.resizes, 2, "两半各缩各的");
    assert_eq!(volume_report.cached_references, 2, "两半各存各的参照");
}

/// **来路一 · 两维都被显式点名。**
///
/// `--bit-depth 2 --dither off`：门那两组各自都只剩 `2bit`，两组给出同一个答案，
/// 那一档因此碰卷之前就答得出（06 号票）。
#[test]
fn a_verdict_pinned_by_naming_both_dimensions_caches_no_reference() {
    let space = Workspace::new();
    let volume = two_gray_pages(&space);

    let report = one_volume(tonefit::Request {
        bit_depth: Some(BitDepth::Two),
        dither: Some(Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    });

    assert_the_pinned_volume_never_cached_a_reference(
        &report,
        Candidate::new(BitDepth::Two, Dither::Off),
    );
}

/// **来路二 · 位深那一维由面板灰阶数裁到只剩一档。**
///
/// 两级灰阶的面板上写得出的只有 `1bit`（ADR 0003 的硬上界），`--dither off` 裁掉另一维——
/// **一个 `--bit-depth` 都没点**，候选照样只剩一个。
///
/// 它单独证明自己有效：谓词若写成「两维都被点名」，另外两条来路照绿，只有这一条红。
#[test]
fn a_verdict_pinned_by_the_panels_gray_levels_caches_no_reference() {
    let space = Workspace::new();
    let volume = two_gray_pages(&space);

    let report = one_volume(tonefit::Request {
        profile: fixtures::baseline_profile()
            .with_gray_levels(2)
            .expect("两级在取值范围里"),
        dither: Some(Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    });

    assert_the_pinned_volume_never_cached_a_reference(
        &report,
        Candidate::new(BitDepth::One, Dither::Off),
    );
}

/// **来路三 · 抖动那一维由几何门裁，门不成立那一组因此整个不在。**
///
/// `--dither fs` 撞上一页贴不住面板就是互锁 ③，处置是整趟被拒（ADR 0007 的《后果》）——
/// 走得完的卷里其余页只可能是门成立那一组，那一档于是不必等整卷判完门。
///
/// 它单独证明自己有效：谓词若只认「门那两组给出同一个答案」，这一条当场红——
/// 门不成立那一组在这一趟上根本不是一个候选集，而是一条拒绝。
#[test]
fn a_verdict_pinned_where_the_gate_leaves_only_one_group_caches_no_reference() {
    let space = Workspace::new();
    let volume = two_gray_pages(&space);

    let report = one_volume(tonefit::Request {
        bit_depth: Some(BitDepth::Two),
        dither: Some(Dither::FloydSteinberg),
        ..fixtures::request(&space, [volume.path()])
    });

    assert_the_pinned_volume_never_cached_a_reference(
        &report,
        Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
    );
}

/// **没被顶死的那一趟一切照旧**（票面末一条）。
///
/// 只点名位深：其余页那一组的几何门开着，抖动那一维还有得判，判据照旧说了算——
/// 上包络那条路上那一档要等整卷汇总，参照因此照旧攒到第二遍。
///
/// 它是上面三条的对照组，也是「谓词别放得太宽」那一面的钉子。开着上包络跑：
/// 默认那条路上没被顶死的页同样当场编码，参照零份，分不出「顶死」与「没顶死」。
#[test]
fn naming_only_the_bit_depth_still_caches_every_reference_under_the_envelope() {
    let space = Workspace::new();
    let volume = two_gray_pages(&space);

    let report = one_volume(tonefit::Request {
        bit_depth: Some(BitDepth::Two),
        envelope: true,
        ..fixtures::request(&space, [volume.path()])
    });

    assert_eq!(report.pages.len(), 2);
    assert_eq!(
        report.cached_references, 2,
        "判定还没定死，参照就该等到第二遍"
    );
}

/// **试算不在其内。**
///
/// 那一趟没有第二遍，编出来的字节一个读者都没有；缓存也只记账、不留页
/// （`cache::Retention::Account`），没有块可换。顶死的试算因此照旧攒参照——
/// 这条界与默认那条路（逐页）逐字相同：当场编码只在照做那一遍。
///
/// 代价是预告的缓存用量与照做那一趟不再是同一个数，与逐页那条路同型（停车场 Q430、Q537）。
#[test]
fn a_pinned_dry_run_still_caches_every_reference() {
    let space = Workspace::new();
    let volume = two_gray_pages(&space);

    let report = one_volume(tonefit::Request {
        mode: tonefit::Mode::DryRun,
        bit_depth: Some(BitDepth::Two),
        dither: Some(Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    });

    assert_eq!(report.resizes, 2, "试算的灰度路径照旧要缩放：判据要它");
    assert_eq!(
        report.cached_references, 2,
        "试算编了一遍码——那一趟没有第二遍要这些字节"
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

    // 开着上包络：参照那一个要有东西可数。
    let report = tonefit::run(&tonefit::Request {
        io_mode: tonefit::IoMode::Concurrent,
        envelope: true,
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
