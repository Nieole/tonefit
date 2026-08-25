//! 幂等与 tEXt 元数据，在 `run(Request) -> Report` 这个 seam 上测。
//!
//! 只断言外部可见的事实：写出的 PNG 的 tEXt 里有什么字段，重跑时 `Report` 说这一卷做了什么。
//! 记录随文件走，因此这里的每一条都是文件的性质，不是某个内部表的性质。

mod fixtures;

use std::fs;

use fixtures::{Volume, Workspace};
use tonefit::{BitDepth, CacheBudget, Filter, Mode, PageColor, Reason, Request, VolumeVerdict};

/// 一处参数改动，连同它在断言里的说法。
type Change = (&'static str, fn(&mut Request));

/// 一处源改动，连同它在断言里的说法。
type Touch = (&'static str, fn(&Volume));

/// 一台彩色面板设备：彩页只有在彩色 profile 下才走彩色分支（ADR 0010）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

#[test]
fn a_rerun_with_the_same_parameters_and_source_skips_the_volume() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let first = fixtures::run_volume(&space, &volume);
    let written = fixtures::fingerprint(&first.volumes[0].output);
    let second = fixtures::run_volume(&space, &volume);

    let skipped = &second.volumes[0];
    assert_eq!(
        skipped.verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
    // 「不重复工作」量得出来：一页都没解码、一页都没进缓存，逐页结果也就无从谈起。
    assert_eq!(skipped.decodes, 0, "跳过的卷还是解码了");
    assert_eq!(skipped.cache.pages, 0, "跳过的卷还是往缓存里存了页");
    assert!(skipped.pages.is_empty(), "跳过的卷不该有逐页结果");
    // 页数是源那一侧的事实，不做工作也数得出来。
    assert_eq!(skipped.page_count(), 2);
    // 几何门是算出来的，这一趟一页都没算。
    assert_eq!(skipped.gate, None);
    assert_eq!(
        fixtures::fingerprint(&skipped.output),
        written,
        "跳过的那一趟动了输出"
    );
}

/// dry-run 预告的就是照做时会发生的事（spec 的 story 6）：会被跳过的卷，先说它会被跳过。
#[test]
fn a_dry_run_predicts_the_skip() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);

    let report = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
}

/// 参数哈希收的是**会改变输出**的每一项：其中任何一项变了，上一趟的输出就过期了。
#[test]
fn a_changed_parameter_redoes_the_volume() {
    let changes: [Change; 5] = [
        ("换 profile", |request| {
            request.profile = fixtures::profile("kobo-clara-hd")
        }),
        ("覆盖面板灰阶数", |request| {
            request.profile = fixtures::baseline_profile()
                .with_gray_levels(4)
                .expect("4 级灰阶")
        }),
        ("点名位深", |request| {
            request.bit_depth = Some(BitDepth::Four)
        }),
        ("换滤波器", |request| request.filter = Filter::Bicubic),
        ("关掉上包络", |request| request.per_page = true),
    ];

    for (what, change) in changes {
        assert_redone(rerun(|_| {}, change), what);
    }
}

/// 换一个指向同一块面板的型号别名：输出逐字节相同，但记录里的型号名说错了。
///
/// 重做一遍是明知故犯的交换——记录要说得出这批输出该拿去哪台设备看
/// （与 `Report::profile` 同一个理由）。
#[test]
fn switching_to_another_alias_of_the_same_panel_redoes_the_volume() {
    let verdict = rerun(
        |_| {},
        |request| request.profile = fixtures::profile("kobo-libra-h2o"),
    );

    assert_redone(verdict, "换了同一块面板的另一个别名");
}

/// 缓存预算限的是峰值内存，一个像素都不改（ADR 0005）：改它不该让整库重做。
#[test]
fn the_cache_budget_alone_does_not_redo_the_volume() {
    let verdict = rerun(
        |_| {},
        |request| request.cache_budget = CacheBudget::new(4 * 1024),
    );

    assert_eq!(verdict, Some(VolumeVerdict::Skipped { page_count: 2 }));
}

/// 源哈希是**卷级**的（为什么，见 ADR 0006 的《决定》末段）：卷里任何一个成员变了、
/// 多了、少了，整卷都得重做。「少了一页」是逐页哈希看不见、而这一条钉得住的那一种。
#[test]
fn a_changed_source_redoes_the_volume() {
    let changes: [Touch; 4] = [
        ("改了一页", |volume| {
            volume.page("001.png", &fixtures::gradient(fixtures::TINY));
        }),
        ("多了一页", |volume| {
            volume.page("003.png", &fixtures::solid(fixtures::TINY, 40));
        }),
        ("少了一页", |volume| {
            fs::remove_file(volume.path().join("002.png")).expect("删掉一页");
        }),
        ("改了透传文件", |volume| {
            volume.file("ComicInfo.xml", b"<ComicInfo><Title>2</Title></ComicInfo>");
        }),
    ];

    for (what, touch) in changes {
        assert_redone(rerun(touch, |_| {}), what);
    }
}

/// 页名换了也是源变了：只哈希字节的话，两页对调名字看不出来，而输出会整个错位。
#[test]
fn renaming_a_source_page_redoes_the_volume() {
    let verdict = rerun(
        |volume| {
            fs::rename(volume.path().join("002.png"), volume.path().join("003.png"))
                .expect("给一页改名");
        },
        |_| {},
    );

    assert_redone(verdict, "给一页改名");
}

/// 记录**就在文件里**：把页搬走、改名、重新打包，读回来还是同一份。
///
/// 这一条钉的是「不丢」，不是「还找得到」。tonefit 按源卷名算输出去处
/// （`Volume::output_path`），把输出容器改了名，它下一趟会在原来的名字上重写一份——
/// 改名的那一份不是被判成过期，而是压根不在它看的地方。记录本身毫发无损：
/// 换个工具、换台机器打开它，判定与四项依据一样读得出来。
#[test]
fn the_record_survives_moving_renaming_and_repacking_the_page() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let report = fixtures::run_volume(&space, &volume);

    let page = &report.volumes[0].pages[0];
    let written = fs::read(&page.output).expect("读回写出的页");
    let original = fixtures::png_text(&written);
    assert!(!original.is_empty(), "夹具不对：这一页没写记录");

    // 搬到别的目录、顺手改个名。
    let moved = space.out().with_file_name("elsewhere");
    fs::create_dir_all(&moved).expect("建目录");
    let renamed = moved.join("完全无关的名字.png");
    fs::rename(&page.output, &renamed).expect("搬走并改名");
    assert_eq!(
        fixtures::read_png_text(&renamed),
        original,
        "改名之后记录丢了"
    );

    // 重新打包进一个别的归档：记录跟着字节走，容器换了也一样。
    let mut repacked = space.cbz("repacked");
    repacked.file("whatever.png", &fs::read(&renamed).expect("读回改过名的页"));
    let members = fixtures::read_cbz(&repacked.write());
    assert_eq!(
        fixtures::png_text(&members[0].1),
        original,
        "重新打包之后记录丢了"
    );
}

/// 输出里少了一个透传文件同样要重做。
///
/// 记录只随页走，透传文件不带记录——只比指纹的话，有人从输出里删掉 ComicInfo.xml 之后
/// 这一卷会永远跳过，那个文件再也补不回来。
#[test]
fn a_passthrough_file_missing_from_the_output_redoes_the_volume() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let first = fixtures::run_volume(&space, &volume);
    fs::remove_file(first.volumes[0].output.join("ComicInfo.xml")).expect("删掉输出里的透传文件");

    let report = fixtures::run_volume(&space, &volume);

    assert_redone(report.volumes[0].verdict, "输出里少了透传文件");
    assert!(
        report.volumes[0].output.join("ComicInfo.xml").is_file(),
        "重做没有把透传文件补回来"
    );
}

/// 记录随文件走，不依赖外部状态库：输出整个搬到别处，幂等判定仍然成立。
#[test]
fn moving_the_output_elsewhere_keeps_the_judgment() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);
    let moved = space.out().with_file_name("moved-out");
    fs::rename(space.out(), &moved).expect("把输出整个搬走");

    let report = tonefit::run(&Request {
        output_root: moved,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
}

/// `--no-metadata` 关掉元数据写入，幂等能力随之消失：没有记录可写，也就没有依据可读。
#[test]
fn without_metadata_nothing_is_recorded_and_nothing_is_skipped() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let bare = || {
        tonefit::run(&Request {
            metadata: false,
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功")
    };

    let first = bare();
    assert!(
        fixtures::read_png_text(&first.volumes[0].pages[0].output).is_empty(),
        "关掉元数据之后仍然写了 tEXt"
    );

    let second = bare();
    assert_redone(second.volumes[0].verdict, "关掉元数据");
    assert_eq!(second.volumes[0].decodes, 2, "该重做的卷没有真的重做");
}

/// 上一趟写的记录是**这一趟**能不能跳过的唯一依据：`--no-metadata` 写出的输出没有记录，
/// 后来一趟带着元数据跑，只能重做。
#[test]
fn an_output_written_without_metadata_is_redone() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    tonefit::run(&Request {
        metadata: false,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let report = fixtures::run_volume(&space, &volume);

    assert_redone(report.volumes[0].verdict, "上一趟没写记录");
}

/// 记录写全六项：幂等那四项，加上判定与它的理由（spec 的 story 7 随文件走的那一份）。
#[test]
fn the_record_names_the_tool_the_profile_the_verdict_and_its_reason() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let report = fixtures::run_volume(&space, &volume);

    let envelope = match report.volumes[0].verdict {
        Some(VolumeVerdict::Envelope(envelope)) => envelope,
        other => panic!("这一卷该由上包络定档，实际是 {other:?}"),
    };
    let page = report.volumes[0]
        .pages
        .iter()
        .find(|page| fixtures::verdict(page).reason == Reason::VolumeEnvelope)
        .expect("卷内主体至少有一页");
    let text = fixtures::read_png_text(&page.output);
    let field = |keyword: &str| fixtures::png_field(&text, keyword);

    // 工具版本、profile 名：这批输出是谁写的、该拿去哪台设备看。
    assert_eq!(
        field("Software"),
        Some(format!("tonefit {}", env!("CARGO_PKG_VERSION")))
    );
    assert_eq!(field("tonefit:profile"), Some("kobo-libra-2".to_owned()));
    // 参数哈希与源哈希：幂等比对读的就是这两项。
    for keyword in ["tonefit:params", "tonefit:source"] {
        let value = field(keyword).unwrap_or_else(|| panic!("{keyword} 没写进去"));
        assert!(
            value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()),
            "{keyword} 不是十六进制哈希：{value}"
        );
    }
    // 判定与理由：ADR 0006 要的那一句，驱动页指名道姓。
    assert_eq!(
        field("tonefit:verdict"),
        Some(fixtures::verdict(page).candidate.to_string())
    );
    assert_eq!(
        field("tonefit:reason"),
        Some(format!(
            "volume-p95, driven by page {:03}",
            envelope.driver + 1
        ))
    );
}

/// 彩色分支不量化，没有判定位深可写（ADR 0005 决定第 4 条）；幂等那四项一项不少。
#[test]
fn a_color_page_carries_the_same_record_without_a_bit_depth() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::color_page(fixtures::TINY));
    volume.page("002.png", &fixtures::gradient(fixtures::TINY));

    let report = fixtures::run_volume_with(&space, &volume, fixtures::profile(COLOR_DEVICE));

    let page = &report.volumes[0].pages[0];
    assert_eq!(page.color, PageColor::Color);
    assert_eq!(page.verdict(), None, "这一页该走彩色分支");
    let text = fixtures::read_png_text(&page.output);
    assert_eq!(
        fixtures::png_field(&text, "Software"),
        Some(format!("tonefit {}", env!("CARGO_PKG_VERSION")))
    );
    assert!(fixtures::png_field(&text, "tonefit:params").is_some());
    assert!(fixtures::png_field(&text, "tonefit:source").is_some());
    assert_eq!(
        fixtures::png_field(&text, "tonefit:verdict"),
        Some("color".to_owned()),
        "彩色分支上没有判定位深可写"
    );

    // 同一卷再跑一趟照样跳过：彩页的记录与灰度页的是同一批字段。
    let again = fixtures::run_volume_with(&space, &volume, fixtures::profile(COLOR_DEVICE));
    assert_eq!(
        again.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
}

/// 归档卷同样跳得过：记录在成员的字节里，容器是目录还是 CBZ 与它无关。
#[test]
fn an_archive_volume_is_skipped_too() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page("001.png", &fixtures::solid(fixtures::TINY, 128))
        .page("002.png", &fixtures::gradient(fixtures::TINY))
        .file("ComicInfo.xml", b"<ComicInfo/>");
    let path = cbz.write();

    let first = fixtures::run_paths(&space, [path.as_path()]);
    let second = fixtures::run_paths(&space, [path.as_path()]);

    // 头一趟写出的成员里带着记录。
    let members = fixtures::read_cbz(&first.volumes[0].output);
    let (_, bytes) = members
        .iter()
        .find(|(name, _)| name == "001.png")
        .expect("头一页");
    assert_eq!(
        fixtures::png_field(&fixtures::png_text(bytes), "tonefit:profile"),
        Some("kobo-libra-2".to_owned())
    );
    assert_eq!(
        second.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
    assert_eq!(second.volumes[0].decodes, 0);
}

/// 一页都没有的卷永远不跳过：记录随页走，没有页就没有地方放记录。
#[test]
fn a_volume_without_pages_is_never_skipped() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    fixtures::run_volume(&space, &volume);
    let report = fixtures::run_volume(&space, &volume);

    assert_eq!(report.volumes[0].verdict, None);
}

/// 两页加一个透传文件的卷。两页都小于面板，几何门因此关着——本文件测的每一条都与门无关。
fn two_pages_and_an_extra(space: &Workspace) -> Volume {
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::solid(fixtures::TINY, 128));
    volume.page("002.png", &fixtures::screentone(fixtures::TINY));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 跑两趟：中间按 `touch` 动一动源，按 `change` 改一改参数。返回第二趟的卷级判定。
fn rerun(touch: impl FnOnce(&Volume), change: impl FnOnce(&mut Request)) -> Option<VolumeVerdict> {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);

    touch(&volume);
    let mut request = fixtures::request(&space, [volume.path()]);
    change(&mut request);
    let report = tonefit::run(&request).expect("第二趟应当成功");

    report.volumes.into_iter().next().expect("一个卷").verdict
}

/// 这一卷该重做，不该跳过。
fn assert_redone(verdict: Option<VolumeVerdict>, what: &str) {
    assert!(
        !matches!(verdict, Some(VolumeVerdict::Skipped { .. })),
        "{what}之后这一卷仍然被跳过了：{verdict:?}"
    );
}
