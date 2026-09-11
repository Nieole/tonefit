//! 幂等与 tEXt 元数据，在 `run(Request) -> Report` 这个 seam 上测。
//!
//! 只断言外部可见的事实：写出的 PNG 的 tEXt 里有什么字段，重跑时 `Report` 说这一卷做了什么。
//! 记录随文件走，因此这里的每一条都是文件的性质，不是某个内部表的性质。

mod fixtures;

use std::fs;

use fixtures::{Volume, Workspace};
use tonefit::{
    BitDepth, CacheBudget, Filter, FitMode, Mode, PageColor, Reason, Request, VolumeVerdict,
};

/// 一处参数改动，连同它在断言里的说法。
type Change = (&'static str, fn(&mut Request));

/// 一处源改动：它在断言里的说法、动手的那一下、以及重做之后输出里该剩下哪些成员。
type Touch = (&'static str, fn(&Volume), &'static [&'static str]);

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
    // 几何门跟着页走，而这一趟一页都没算：判定范围因此是空的。
    assert_eq!(skipped.judged_by_the_gate().count(), 0);
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
    let changes: [Change; 12] = [
        ("换 profile", |request| {
            request.profile = fixtures::profile("kobo-clara-hd")
        }),
        // 阈值是**界**，判据是**量**（`CONTEXT.md`）：界挪一格，逐页判定就可能落到另一档上，
        // 上一趟的输出整卷过期。换档位不需要判据变——标定重新夹一次窗口就够了
        // （`metric-recalibration/07` 就是这么一趟），幂等得拦得住那一趟。
        // 改动相对当前取值（翻倍）：标定把界换成多少，这一条都不必跟着改。
        ("换阈值", |request| {
            let doubled = request.profile.threshold().value() * 2.0;
            request.profile = request
                .profile
                .clone()
                .with_threshold(doubled)
                .expect("两倍仍在 0 与 255 之间")
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
        // 适配方式改的是目标尺寸本身（页几何批 01 号票）：换了它，这一卷每一页的尺寸、
        // 几何门、判据参照与判定都要重算，上一趟的输出一张都不能留。
        ("换适配方式", |request| request.fit = FitMode::Inside),
        // 裁边改的是**适配之前**的页尺寸（页几何批 02 号票）：同上，整卷重算。
        ("关掉裁边", |request| request.crop = false),
        // 拆分那三项改的是**这一卷有几页、每一页是哪一块**（页几何批 04 号票）。
        // 阅读方向只换两半的先后，但那正是成员名的次序——`001-1.png` 从右半变成左半，
        // 字节整个换了一张。
        ("关掉拆分", |request| request.split.on = false),
        ("换拆分阈值", |request| {
            request.split.threshold = tonefit::SplitThreshold::parse("2.5").expect("正数")
        }),
        ("换阅读方向", |request| {
            request.split.order = tonefit::ReadingOrder::LeftToRight
        }),
        // 走哪条路改的是每一页的档（ADR 0018 的《后果》：翻默认那一趟全库不命中，正是这一项）。
        ("打开上包络", |request| request.envelope = true),
        // 纸白对齐的上限改的是缩放之后那一步的像素（纸白对齐批 01 号票）：
        // 参照与其后一切量化跟着变。**默认值抬到 4 之后**（05 号票），这一格改的是
        // **从默认的 4（开）关回 0** 那一次——不放心的人关掉它，上一趟对齐过的输出
        // 必须整卷过期；而「取值 0 也照样进参数哈希」不成立时，唯一漏得掉的正是 0 这一头。
        // 漏了它，用户会看见「我关掉了却没效果」，还找不出原因（ADR 0002 的《后果》记过同型事故）。
        ("关掉纸白对齐", |request| {
            request.white_align_limit = tonefit::WhiteAlignLimit::OFF
        }),
    ];

    for (what, change) in changes {
        assert_redone(rerun(|_| {}, change).verdict, what);
    }
}

/// 换一个指向同一块面板的型号别名：输出逐字节相同，但记录里的型号名说错了。
///
/// 重做一遍是明知故犯的交换——记录要说得出这批输出该拿去哪台设备看
/// （与 `Report::profile` 同一个理由）。
#[test]
fn switching_to_another_alias_of_the_same_panel_redoes_the_volume() {
    let redone = rerun(
        |_| {},
        |request| request.profile = fixtures::profile("kobo-libra-h2o"),
    );

    assert_redone(redone.verdict, "换了同一块面板的另一个别名");
}

/// 缓存预算限的是峰值内存，一个像素都不改（ADR 0005）：改它不该让整库重做。
#[test]
fn the_cache_budget_alone_does_not_redo_the_volume() {
    let redone = rerun(
        |_| {},
        |request| request.cache_budget = CacheBudget::new(4 * 1024),
    );

    assert_eq!(
        redone.verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
}

/// 卷里任何一个成员变了、多了、少了，这一卷都**不能整卷跳过**（卷级源哈希看得见每一种，
/// ADR 0006 的《决定》末段）。默认路径上重做的只是变了的那几页（two-pass-rework/14，
/// 由 `adding_removing_or_renaming_a_page_redoes_only_what_changed` 钉），这里钉的是
/// 「没被跳过」加上**输出里剩下什么**：只断言前者的话，源里删掉的那一页仍然可以
/// 原封不动地留在输出里。为什么那是个陷阱，见 `sink::DirectorySink`。
#[test]
fn a_changed_source_redoes_the_volume() {
    const INTACT: &[&str] = &["001.png", "002.png", "ComicInfo.xml"];
    let changes: [Touch; 4] = [
        (
            "改了一页",
            |volume| {
                volume.page("001.png", &fixtures::gradient(fixtures::TINY));
            },
            INTACT,
        ),
        (
            "多了一页",
            |volume| {
                volume.page("003.png", &fixtures::solid(fixtures::TINY, 40));
            },
            &["001.png", "002.png", "003.png", "ComicInfo.xml"],
        ),
        (
            "少了一页",
            |volume| {
                fs::remove_file(volume.path().join("002.png")).expect("删掉一页");
            },
            &["001.png", "ComicInfo.xml"],
        ),
        (
            "改了透传文件",
            |volume| {
                volume.file("ComicInfo.xml", b"<ComicInfo><Title>2</Title></ComicInfo>");
            },
            INTACT,
        ),
    ];

    for (what, touch, members) in changes {
        let redone = rerun(touch, |_| {});
        assert_redone(redone.verdict, what);
        assert_eq!(redone.members, members, "{what}之后输出里的成员不对");
    }
}

/// 页名换了也是源变了：只哈希字节的话，两页对调名字看不出来，而输出会整个错位。
/// 这一卷不能整卷跳过；改了名的那一页重做，旧名字下的那一张清走（旁边没变的页留下，
/// 那一半由 `adding_removing_or_renaming_a_page_redoes_only_what_changed` 钉）。
#[test]
fn renaming_a_source_page_redoes_the_volume() {
    let redone = rerun(
        |volume| {
            fs::rename(volume.path().join("002.png"), volume.path().join("003.png"))
                .expect("给一页改名");
        },
        |_| {},
    );

    assert_redone(redone.verdict, "给一页改名");
    // 旧名字下的那一页也得跟着走：留着它，输出里会同时躺着改名前后的两份。
    assert_eq!(redone.members, ["001.png", "003.png", "ComicInfo.xml"]);
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

/// 扩展名归一之后，幂等的去处仍对得上：`第10话.zip` 与 `第10话.cbz` 指着同一份输出。
///
/// 去处按**源卷名**算，而归档卷的扩展名在算去处那一步就归一掉了（ADR 0015）。
/// 因此把源换个扩展名重打一遍——内容一字不改——下一趟仍找得到上一趟写在 `第10话.cbz`
/// 上的那份输出，一页都不重做。这一条断了的话症状是静默的：换个扩展名重打一次包，
/// 整库无声地重跑一遍。
#[test]
fn the_same_volume_packed_as_zip_or_cbz_lands_on_the_same_output_and_still_skips() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();
    let extra = b"<?xml version=\"1.0\"?>\n<ComicInfo/>\n";

    let mut as_cbz = space.archive("第10话.cbz");
    as_cbz.page("001.png", &page).file("ComicInfo.xml", extra);
    let as_cbz = as_cbz.write();

    let first = fixtures::run_paths(&space, [as_cbz.as_path()]);
    assert_eq!(first.volumes[0].output, space.out().join("第10话.cbz"));
    let written = fixtures::fingerprint(&first.volumes[0].output);

    // 同一卷换个扩展名重打一遍。
    let mut as_zip = space.archive("第10话.zip");
    as_zip.page("001.png", &page).file("ComicInfo.xml", extra);
    let as_zip = as_zip.write();

    let second = fixtures::run_paths(&space, [as_zip.as_path()]);

    let skipped = &second.volumes[0];
    assert_eq!(
        skipped.output,
        space.out().join("第10话.cbz"),
        "去处没按源卷名算"
    );
    assert_eq!(
        skipped.verdict,
        Some(VolumeVerdict::Skipped { page_count: 1 }),
        "换个扩展名重打的同一卷没被认出来"
    );
    assert_eq!(
        fixtures::fingerprint(&skipped.output),
        written,
        "跳过的那一趟动了输出"
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
///
/// 跑在 `--envelope` 那条路上：理由那一句要指名定档页（`driven by page`），
/// 而定档页只有上包络才有。默认那条路的记录由下面那一条钉。
#[test]
fn the_record_names_the_tool_the_profile_the_verdict_and_its_reason() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let report = tonefit::run(&Request {
        envelope: true,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let envelope = match report.volumes[0].verdict {
        Some(VolumeVerdict::Envelope(envelope)) => envelope,
        other => panic!("这一卷该由上包络定档，实际是 {other:?}"),
    };
    let page = report.volumes[0]
        .pages
        .iter()
        .find(|page| fixtures::verdict(page).reason == Reason::VolumeEnvelope)
        .expect("卷内其余页至少有一页");
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
    // 判定与理由：ADR 0006 要的那一句，定档页指名道姓。
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

/// **默认那条路（逐页）上那份记录是第一遍盖的，字段一格不差**（12 号票）。
///
/// 那条路上量化与编码提到了第一遍——一页判完当场就编好了，盖记录的于是不再是第二遍的
/// `Encode`，而是第一遍自己。这一条问的就是**换了盖章的人之后那七项还对不对**：
/// 判定与理由取自报告，而报告由汇总那一处独立算出来——两处对不上，
/// 写出去的字节就与报告说的那一档分了家。
///
/// 定档页那一项在这条路上恒不在场（上包络关着），理由因此是逐页那两种之一，
/// 不带 `driven by page`。段式迟滞那一种（`hysteresis pull-back`）已随迟滞退场（ADR 0018），
/// 不再产出。
#[test]
fn the_record_on_the_default_path_still_names_the_verdict_and_its_reason() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let report = fixtures::run_volume(&space, &volume);

    assert_eq!(
        report.volumes[0].verdict,
        Some(VolumeVerdict::PerPage),
        "默认走到了上包络，测的就不是逐页那条路"
    );
    for page in &report.volumes[0].pages {
        let verdict = fixtures::verdict(page);
        let text = fixtures::read_png_text(&page.output);
        let field = |keyword: &str| fixtures::png_field(&text, keyword);

        assert_eq!(
            field("Software"),
            Some(format!("tonefit {}", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(field("tonefit:profile"), Some("kobo-libra-2".to_owned()));
        for keyword in ["tonefit:params", "tonefit:source"] {
            let value = field(keyword).unwrap_or_else(|| panic!("{keyword} 没写进去"));
            assert!(
                value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()),
                "{keyword} 不是十六进制哈希：{value}"
            );
        }
        assert_eq!(
            field("tonefit:verdict"),
            Some(verdict.candidate.to_string()),
            "写进 tEXt 的那一档与报告说的不是同一档"
        );
        // 逐页那两种的字面，一律不带定档页。
        let reason = field("tonefit:reason").expect("理由没写进去");
        assert!(
            !reason.contains("driven by page"),
            "逐页那条路上竟指了一张定档页：{reason}"
        );
        assert_eq!(
            reason,
            match verdict.reason {
                Reason::LowestWithinThreshold => "lowest candidate within threshold",
                Reason::NoneWithinThreshold => "none within threshold, top candidate",
                other => panic!("逐页那条路上不该出现的理由：{other:?}"),
            },
            "写进 tEXt 的理由与报告说的不是同一句"
        );
    }
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
    assert_eq!(page.color(), Some(PageColor::Color));
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

/// **切开的卷跳得过，而少一半就重做**（页几何批 04 号票）。
///
/// 幂等要在碰像素之前答完，而一个源页产出几张由内容决定——名单因此改从**上一趟写在
/// 输出里的记录**读回来：每一张自己说得出它来自哪个源页、那一族该有几张
/// （`tonefit:origin`）。
///
/// 下半段钉的是**「输出里少一张就该察觉」这条能力没有丢**（`p0-hardening/03` 靠的正是它）：
/// 删掉后一半之后剩下那一张仍写着「1/2」，缺口因此看得见。只认「至少有一张」的实现
/// 在这里会答「跳过」，而那一半再也补不回来。
#[test]
fn a_split_volume_is_skipped_and_losing_one_half_redoes_it() {
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

    let first = fixtures::run_volume(&space, &volume);
    assert_eq!(first.volumes[0].page_count(), 2, "夹具没被切开");
    let output = first.volumes[0].output.clone();

    // 一张源页两张输出页，第二趟一页都不重做——跳过那一行报的是**输出**页数。
    let second = fixtures::run_volume(&space, &volume);
    assert_eq!(
        second.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 })
    );
    assert_eq!(second.volumes[0].source_pages, 1);
    assert_eq!(second.volumes[0].decodes, 0, "跳过的卷还是解码了");

    // 删掉后一半：整卷重做，两张都回来了。
    fs::remove_file(output.join("001-2.png")).expect("删掉后一半");
    let third = fixtures::run_volume(&space, &volume);
    assert_redone(third.volumes[0].verdict, "输出里少了一半");
    assert_eq!(
        fixtures::directory_members(&third.volumes[0].output),
        ["001-1.png", "001-2.png"]
    );
}

/// 记录里那一项**说得出这一张来自哪个源成员、是那一族的第几张**（页几何批 04 号票）。
///
/// 取值一律 ASCII（tEXt 只装得下 Latin-1），序号从 1 数起。它随文件走：
/// 一张切出来的半页离开报告的上下文之后，只有这一项还说得出它原来是哪一张跨页的一半。
#[test]
fn a_split_page_records_which_source_member_it_came_from() {
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
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));

    let report = fixtures::run_volume(&space, &volume);

    let origin = |index: usize| {
        let text = fixtures::read_png_text(&report.volumes[0].pages[index].output);
        fixtures::png_field(&text, "tonefit:origin").expect("来路那一项没写进去")
    };
    assert_eq!(origin(0), "001.png 1/2");
    assert_eq!(origin(1), "001.png 2/2");
    // 没切开的那一张同样带着它，写的是「一族只有一张」。
    assert_eq!(origin(2), "002.png 1/1");
}

/// **每张输出页记着它自己那一份源哈希**（two-pass-rework/13：页级依据）。
///
/// 卷级那一项 `tonefit:source` 照旧写、照旧全卷同一个；多出来的 `tonefit:page-source`
/// 只算这一张来自的那个源成员——改了一页，只有那一页的页级哈希变，旁边那一页的纹丝不动，
/// 而卷级那一项两页一起变。这正是 spec 的 story 23：一页的幂等依据只取决于这一页自己。
///
/// 13 号票只把它算出来、记下去；14 号票拿它按页跳过——改了一页之后没变的那一页**留下**，
/// 它那一项卷级源哈希在搬的时候改写成这一趟的（见 `metadata::restamp_source`），
/// 因此第二趟的输出里两页的卷级那一项仍旧全卷同一个。读的是输出里的文件而不是
/// `pages[index]`：留下的页不在逐页结果里（`VolumeReport::pages` 只列这一趟做了的）。
#[test]
fn every_page_on_the_default_path_carries_its_own_source_hash_beside_the_volume_one() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let first = fixtures::run_volume(&space, &volume);
    assert_eq!(
        first.volumes[0].verdict,
        Some(VolumeVerdict::PerPage),
        "默认走到了上包络，测的就不是逐页那条路"
    );
    let basis = |report: &tonefit::Report, name: &str| {
        let output = report.volumes[0].output.join(name);
        (
            recorded(&output, "tonefit:source"),
            recorded(&output, "tonefit:page-source"),
        )
    };
    let (volume_a, page_a) = basis(&first, "001.png");
    let (volume_b, page_b) = basis(&first, "002.png");
    for hash in [&page_a, &page_b] {
        assert!(
            hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()),
            "tonefit:page-source 不是十六进制哈希：{hash}"
        );
    }
    assert_eq!(volume_a, volume_b, "卷级源哈希该全卷同一个");
    assert_ne!(page_a, page_b, "两张不同的源页给出了同一个页级源哈希");
    assert_ne!(page_a, volume_a, "页级那一份与卷级那一份写成了同一个数");

    // 改第一页：只有它的页级哈希变，第二页的不动；卷级那一项两页一起变——
    // 第二页留下没重做，那一项是搬的时候改写的。
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    let second = fixtures::run_volume(&space, &volume);
    assert_redone(second.volumes[0].verdict, "改了一页");
    let (volume_a2, page_a2) = basis(&second, "001.png");
    let (volume_b2, page_b2) = basis(&second, "002.png");
    assert_ne!(page_a2, page_a, "改了第一页，它的页级源哈希却没变");
    assert_eq!(
        page_b2, page_b,
        "改的是第一页，第二页的页级源哈希却跟着变了"
    );
    assert_ne!(volume_a2, volume_a, "改了一页，卷级源哈希却没变");
    assert_eq!(volume_a2, volume_b2, "卷级源哈希该全卷同一个");
}

/// **跨页拆分下页级依据定义得清楚**（two-pass-rework/13）：一个源页出两张输出页，
/// 两张的页级源哈希**相同**——它们来自同一个成员的同一批字节——而来路那一项各说自己是哪一半。
/// 两项合在一起，每一张都指得回「源页 001.png 的第几半」；没切开的那一张另有自己的哈希。
///
/// 页级源哈希算的是**源成员**而不是切出来的像素，这一条钉的正是这个选择：反过来算像素的话，
/// 幂等就得先解码再问「这一半变没变」，而幂等的全部意义是在碰像素之前答完。
#[test]
fn both_halves_of_a_split_page_share_one_source_hash_and_the_origin_tells_them_apart() {
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
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));

    let report = fixtures::run_volume(&space, &volume);
    assert_eq!(report.volumes[0].page_count(), 3, "夹具没被切开");

    let basis = |index: usize| {
        let output = &report.volumes[0].pages[index].output;
        (
            recorded(output, "tonefit:page-source"),
            recorded(output, "tonefit:origin"),
        )
    };
    let (left, left_origin) = basis(0);
    let (right, right_origin) = basis(1);
    let (whole, whole_origin) = basis(2);

    assert_eq!(left, right, "同一张跨页切出的两半给出了两个页级源哈希");
    assert_eq!(left_origin, "001.png 1/2");
    assert_eq!(right_origin, "001.png 2/2");
    assert_ne!(whole, left, "另一张源页与那张跨页给出了同一个页级源哈希");
    assert_eq!(whole_origin, "002.png 1/1");
}

/// **走 `--envelope` 那条路时不记页级依据**（two-pass-rework/13）：那条路上一页的档由全卷定
/// （ADR 0006 决定第 3 条），「这一页变没变」答不了这一页该不该重做。卷级那一份照旧写，
/// 同一卷再跑一趟照旧整卷跳过——那条路上的记录与本票落地之前是同一批字段。
///
/// 卷里放一张彩页：灰度页的记录在第二遍盖，彩页的在第一遍就盖（ADR 0010），
/// 两处各走各的代码，只测灰度页的话彩页那一处写了也看不见。
#[test]
fn the_envelope_path_records_no_page_level_basis_and_still_skips_as_a_whole() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::color_page(fixtures::TINY));
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));
    volume.page("003.png", &fixtures::screentone(fixtures::TINY));
    let under_the_envelope = || {
        tonefit::run(&Request {
            envelope: true,
            profile: fixtures::profile(COLOR_DEVICE),
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功")
    };

    let first = under_the_envelope();
    assert!(
        matches!(first.volumes[0].verdict, Some(VolumeVerdict::Envelope(_))),
        "夹具没走到上包络：{:?}",
        first.volumes[0].verdict
    );
    assert_eq!(
        first.volumes[0].pages[0].color(),
        Some(PageColor::Color),
        "夹具里那张彩页没走彩色分支"
    );
    for page in &first.volumes[0].pages {
        let text = fixtures::read_png_text(&page.output);
        assert!(
            fixtures::png_field(&text, "tonefit:source").is_some(),
            "卷级源哈希没写进去"
        );
        assert_eq!(
            fixtures::png_field(&text, "tonefit:page-source"),
            None,
            "上包络那条路上写了页级源哈希：{}",
            page.output.display()
        );
    }

    let second = under_the_envelope();
    assert_eq!(
        second.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 3 })
    );
}

/// **覆盖顶死的那一趟照写页级依据**（two-pass-rework/13）：位深与抖动都点名，每一页的档在碰卷之前
/// 就定死，字节只取决于这一页自己——「写不写」问的是这件事，不是开关的名字（停车场 Q665）。
/// 卷级那一行报的是 `Override`，不是逐页也不是上包络，而页级依据两种页都有。
#[test]
fn a_pinned_run_records_the_page_level_basis_too() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);

    let report = tonefit::run(&Request {
        bit_depth: Some(BitDepth::Four),
        dither: Some(tonefit::Dither::Off),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert!(
        matches!(report.volumes[0].verdict, Some(VolumeVerdict::Override(_))),
        "夹具没走到顶死那一趟：{:?}",
        report.volumes[0].verdict
    );
    for page in &report.volumes[0].pages {
        assert_eq!(
            fixtures::verdict(page).reason,
            Reason::Override,
            "{} 的档不是顶死的",
            page.source.display()
        );
        let page_source = recorded(&page.output, "tonefit:page-source");
        assert!(
            page_source.len() == 32 && page_source.chars().all(|c| c.is_ascii_hexdigit()),
            "tonefit:page-source 不是十六进制哈希：{page_source}"
        );
    }
}

/// **旧记录（只有卷级那一份）照旧读得懂**，跳过与重做一个字不变（two-pass-rework/13）。
///
/// 本票之前写出的每一页都没有 `tonefit:page-source`。把这一趟写出的页里那个块剥掉，
/// 造出来的就是一份老形态的输出——再跑一趟仍旧**整卷跳过**：幂等的判据还是卷级那四项加来路，
/// 页级那一项缺了是「页级答不了」，不是坏记录，更不是重做的理由。
/// 反过来说，这一条红了就是有人把页级那一项塞进了 `can_skip`，那是 14 号票的活，不是这一票的。
#[test]
fn an_output_carrying_only_the_volume_level_basis_is_still_skipped_as_a_whole() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let first = fixtures::run_volume(&space, &volume);

    for page in &first.volumes[0].pages {
        let written = fs::read(&page.output).expect("读回写出的页");
        let stripped = without_text_chunk(&written, "tonefit:page-source");
        assert_ne!(
            stripped.len(),
            written.len(),
            "夹具不对：这一页本来就没有页级源哈希"
        );
        fs::write(&page.output, &stripped).expect("写回剥掉那一项的页");
        let text = fixtures::read_png_text(&page.output);
        assert_eq!(fixtures::png_field(&text, "tonefit:page-source"), None);
        assert!(fixtures::png_field(&text, "tonefit:source").is_some());
    }

    let second = fixtures::run_volume(&space, &volume);
    assert_eq!(
        second.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 }),
        "只带卷级依据的旧输出没被整卷跳过"
    );
    assert_eq!(second.volumes[0].decodes, 0, "跳过的卷还是解码了");
}

/// 一页 PNG 剥掉关键字为 `keyword` 的那个 tEXt 块，其余字节原样。
///
/// 按块走一遍：8 字节签名，然后每块是 4 字节长度、4 字节类型、数据、4 字节 CRC。
/// tEXt 的数据是「关键字 `\0` 取值」。造老形态的输出只有这一条路——本工具今天写不出没有那一项的页。
fn without_text_chunk(png: &[u8], keyword: &str) -> Vec<u8> {
    const SIGNATURE: usize = 8;
    let mut kept = png[..SIGNATURE].to_vec();
    let mut at = SIGNATURE;
    while at < png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().expect("4 字节")) as usize;
        let end = at + 4 + 4 + length + 4;
        let chunk = &png[at..end];
        let is_the_one = &chunk[4..8] == b"tEXt"
            && chunk[8..8 + length]
                .split(|byte| *byte == 0)
                .next()
                .is_some_and(|listed| listed == keyword.as_bytes());
        if !is_the_one {
            kept.extend_from_slice(chunk);
        }
        at = end;
    }
    kept
}

/// **失败页的占位页不记页级依据**，同一卷里的好页照记（two-pass-rework/13）。
///
/// 占位页按卷内统一尺寸出（12 号票），那个尺寸由全卷定——这一页的字节因此不只取决于它自己，
/// 与上包络那条路同一条理由。它随身带的仍是七项：卷级依据、来路、`failed` 与那句自证。
#[test]
fn a_placeholder_page_carries_no_page_level_basis_while_its_neighbour_does() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", b"not a png at all");
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));

    let report = fixtures::run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert!(reported.isolated(), "夹具不对：这一卷没进隔离目录");
    let placeholder = fixtures::read_png_text(&reported.pages[0].output);
    assert_eq!(
        fixtures::png_field(&placeholder, "tonefit:verdict"),
        Some("failed".to_owned()),
        "头一页不是占位页"
    );
    assert!(fixtures::png_field(&placeholder, "tonefit:source").is_some());
    assert_eq!(
        fixtures::png_field(&placeholder, "tonefit:page-source"),
        None,
        "占位页写了页级源哈希——它的尺寸由全卷定，页级答不了"
    );
    let neighbour = fixtures::read_png_text(&reported.pages[1].output);
    assert!(
        fixtures::png_field(&neighbour, "tonefit:page-source").is_some(),
        "同一卷里的好页丢了页级源哈希"
    );
}

/// **改了一页，只重做那一页**（two-pass-rework/14：按页跳过；ADR 0018 决定第 4 条）。
///
/// 默认路径上一页的档只取决于它自己，页级依据（`tonefit:page-source` 加来路）因此答得了
/// 「这一页变没变」。改掉两页里的一页再跑：没变的那一页**不解码、不判、不编**——解码次数
/// 只有 1；报告说得出这一卷留下了几页、重做了几页，而卷那一行的页数仍是整本书的页数。
///
/// 下半段钉的是**产物一字不差**：按页跳过省的是工，不是结果——同一卷往一个空的输出根
/// 整卷重做一遍，两份输出逐字节相同。留下的页那一项卷级源哈希因此是改写过的
/// （`metadata::restamp_source`），不是原样照搬的旧数。
#[test]
fn changing_one_page_redoes_only_that_page() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let first = fixtures::run_volume(&space, &volume);
    let before = fixtures::fingerprint(&first.volumes[0].output);

    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    let second = fixtures::run_volume(&space, &volume);

    let redone = &second.volumes[0];
    assert_eq!(redone.verdict, Some(VolumeVerdict::PerPage));
    assert_eq!(redone.decodes, 1, "没变的那一页也被解码了");
    assert_eq!(redone.retained_pages, 1, "报告说不出留下了几页");
    assert_eq!(redone.pages.len(), 1, "报告里重做的不止那一页");
    assert_eq!(
        redone.pages[0]
            .source
            .file_name()
            .and_then(|name| name.to_str()),
        Some("001.png"),
        "重做的不是改了的那一页"
    );
    assert_eq!(redone.page_count(), 2, "卷那一行的页数该是整本书的页数");
    let after = fixtures::fingerprint(&redone.output);
    assert_ne!(after, before, "改了的那一页没有重写");
    assert_eq!(
        fixtures::directory_members(&redone.output),
        ["001.png", "002.png", "ComicInfo.xml"]
    );

    // 同一卷整卷重做一遍：与按页跳过那一趟的产物逐字节相同。
    let from_scratch = tonefit::run(&Request {
        output_root: space.out_named("from-scratch"),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    assert_eq!(
        from_scratch.volumes[0].decodes, 2,
        "夹具不对：这一趟没有整卷重做"
    );
    assert_eq!(
        after,
        fixtures::fingerprint(&from_scratch.volumes[0].output),
        "按页跳过的产物与整卷重做的不是同一份"
    );
}

/// **删一页、加一页、改名一页，都只重做该重做的**（two-pass-rework/14）。
///
/// 三种都挪动了后面每一页在阅读顺序里的位置，而位置**不进依据**：页级依据是这一页的名字
/// 加字节（加来路），挪了位置的页两样都没变，留下它们正对。删掉的那一页在输出里成了陈旧产物，
/// 收尾整个换掉时一并清走；加进来的、改了名的那一页在新名字下没有记录，只有它重做。
#[test]
fn adding_removing_or_renaming_a_page_redoes_only_what_changed() {
    struct Case {
        what: &'static str,
        touch: fn(&Volume),
        decodes: usize,
        retained: usize,
        members: &'static [&'static str],
    }
    let cases = [
        Case {
            what: "删了一页",
            touch: |volume| {
                fs::remove_file(volume.path().join("001.png")).expect("删掉一页");
            },
            decodes: 0,
            retained: 1,
            members: &["002.png", "ComicInfo.xml"],
        },
        Case {
            what: "加了一页",
            touch: |volume| {
                volume.page("000.png", &fixtures::gradient(fixtures::TINY));
            },
            decodes: 1,
            retained: 2,
            members: &["000.png", "001.png", "002.png", "ComicInfo.xml"],
        },
        Case {
            what: "改了一页的名字",
            touch: |volume| {
                fs::rename(volume.path().join("001.png"), volume.path().join("000.png"))
                    .expect("给一页改名");
            },
            decodes: 1,
            retained: 1,
            members: &["000.png", "002.png", "ComicInfo.xml"],
        },
    ];

    for case in cases {
        let space = Workspace::new();
        let volume = two_pages_and_an_extra(&space);
        fixtures::run_volume(&space, &volume);
        (case.touch)(&volume);

        let report = fixtures::run_volume(&space, &volume);

        let redone = &report.volumes[0];
        assert_redone(redone.verdict, case.what);
        assert_eq!(
            redone.decodes, case.decodes,
            "{}之后解码的页数不对",
            case.what
        );
        assert_eq!(
            redone.retained_pages, case.retained,
            "{}之后留下的页数不对",
            case.what
        );
        assert_eq!(
            fixtures::directory_members(&redone.output),
            case.members,
            "{}之后输出里的成员不对",
            case.what
        );
    }
}

/// **归档卷也按页跳过**（two-pass-rework/14）：留下的成员从上一趟的归档里搬进新包，
/// 不解码、不判、不编——包仍是整个重打的（成员按写入顺序排），但留下的那一页一个像素都不碰。
/// 产物与整卷重做的逐字节相同，成员顺序因此也是阅读顺序。
#[test]
fn an_archive_volume_is_skipped_by_page_too() {
    let space = Workspace::new();
    let pack = |first: &image::DynamicImage| {
        let mut cbz = space.cbz("volume-a");
        cbz.page("001.png", first)
            .page("002.png", &fixtures::screentone(fixtures::TINY))
            .file("ComicInfo.xml", b"<ComicInfo/>");
        cbz.write()
    };
    let path = pack(&fixtures::solid(fixtures::TINY, 128));
    fixtures::run_paths(&space, [path.as_path()]);

    let path = pack(&fixtures::gradient(fixtures::TINY));
    let second = fixtures::run_paths(&space, [path.as_path()]);

    let redone = &second.volumes[0];
    assert_redone(redone.verdict, "改了归档里的一页");
    assert_eq!(redone.decodes, 1, "没变的那一页也被解码了");
    assert_eq!(redone.retained_pages, 1);
    assert_eq!(redone.page_count(), 2);
    let members = fixtures::read_cbz(&redone.output);
    assert_eq!(
        members
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["001.png", "002.png", "ComicInfo.xml"],
        "成员顺序不是阅读顺序"
    );

    let from_scratch = tonefit::run(&Request {
        output_root: space.out_named("from-scratch"),
        ..fixtures::request(&space, [path.as_path()])
    })
    .expect("处理应当成功");
    assert_eq!(
        members,
        fixtures::read_cbz(&from_scratch.volumes[0].output),
        "按页跳过的归档与整卷重做的不是同一份"
    );
}

/// **按页跳过之后的下一趟整卷跳过**（two-pass-rework/14）：留下的页那一项卷级源哈希在搬的时候
/// 改写成了这一趟的，输出于是与整卷重做的一样齐——下一趟卷级那四项每一页都对得上，
/// 不必再逐页比、逐页搬。这一条断了的症状是静默的：那一卷从此每趟都把整卷搬一遍。
#[test]
fn the_run_after_a_partial_one_skips_the_volume_as_a_whole() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    let partial = fixtures::run_volume(&space, &volume);
    assert_eq!(
        partial.volumes[0].retained_pages, 1,
        "夹具不对：这一趟没有按页跳过"
    );

    let third = fixtures::run_volume(&space, &volume);

    assert_eq!(
        third.volumes[0].verdict,
        Some(VolumeVerdict::Skipped { page_count: 2 }),
        "按页跳过之后的下一趟没有整卷跳过"
    );
    assert_eq!(third.volumes[0].decodes, 0);
}

/// **走 `--envelope` 那条路时照旧整卷重做**（two-pass-rework/14）：那条路上一页的档由全卷定，
/// 记录里没有页级依据，改一页整卷重做——两页都解码，一页都不留。
#[test]
fn under_the_envelope_a_changed_page_still_redoes_the_whole_volume() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume_under_the_envelope(&space, &volume);
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));

    let report = fixtures::run_volume_under_the_envelope(&space, &volume);

    let redone = &report.volumes[0];
    assert!(
        matches!(redone.verdict, Some(VolumeVerdict::Envelope(_))),
        "夹具没走到上包络：{:?}",
        redone.verdict
    );
    assert_eq!(redone.decodes, 2, "上包络那条路上留下了页");
    assert_eq!(redone.retained_pages, 0);
    assert_eq!(redone.pages.len(), 2);
}

/// **试算预告的就是按页跳过**（spec 的 story 6，two-pass-rework/14）：改一页之后试算，
/// 报告说留下一页、只解一页——与照做那一趟同一个数，尽管试算一个字节都不写。
#[test]
fn a_dry_run_predicts_the_skip_by_page() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));

    let report = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let predicted = &report.volumes[0];
    assert_eq!(predicted.decodes, 1, "试算把没变的那一页也解了");
    assert_eq!(predicted.retained_pages, 1);
    assert_eq!(predicted.pages.len(), 1);
}

/// **一页坏了整卷进隔离目录，留下的页跟着去**（two-pass-rework/14；12 号票的隔离不变）。
///
/// 卷仍是去处的单位：重做的那一页失败，整卷去隔离目录，而隔离目录里那一份得是整本书——
/// 留下的页从干净去处里搬过去，干净去处那一份原样留着当过期副本。占位页按卷内统一尺寸出，
/// 那个尺寸数的是整本书：这一卷只重做了一页而且它失败了，众数由留下的那一页定。
/// 坏页修好之后再跑：没变的那一页仍从干净去处里留下，重做的只有修好的那一页。
#[test]
fn a_failed_page_isolates_the_volume_and_the_retained_pages_go_along() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    let first = fixtures::run_volume(&space, &volume);
    let clean = first.volumes[0].output.clone();
    let neighbour = fixtures::read_png(&clean.join("002.png"));

    volume.file("001.png", b"not a png at all");
    let isolated = fixtures::run_volume(&space, &volume);

    let reported = &isolated.volumes[0];
    assert!(reported.isolated(), "坏页没让这一卷进隔离目录");
    assert_eq!(reported.decodes, 1, "没变的那一页也被解码了");
    assert_eq!(reported.retained_pages, 1);
    assert_eq!(reported.superseded.as_deref(), Some(clean.as_path()));
    assert_eq!(
        fixtures::directory_members(&reported.output),
        ["001.png", "002.png", "ComicInfo.xml"],
        "隔离目录里那一份不是整本书"
    );
    let placeholder = fixtures::read_png(&reported.output.join("001.png"));
    assert_eq!(
        placeholder.size, neighbour.size,
        "占位页没按卷内统一尺寸出——留下的那一页没数进众数"
    );

    // 坏页修好：没变的那一页仍从干净去处里留下，只重做修好的那一页。
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    let mended = fixtures::run_volume(&space, &volume);
    let reported = &mended.volumes[0];
    assert!(!reported.isolated(), "修好了还在隔离目录里");
    assert_eq!(reported.output, clean);
    assert_eq!(reported.decodes, 1, "修好之后没变的那一页也被解码了");
    assert_eq!(reported.retained_pages, 1);
    assert!(
        reported.superseded.is_some(),
        "隔离目录里那一份该报成过期副本"
    );
}

/// **切开的那一族整族留下、整族重做**（two-pass-rework/14；页几何批 04 号票的来路）。
///
/// 一张跨页切成两张输出页，页级源哈希相同、来路各说自己是哪一半。改旁边那一页，
/// 这一族两张都留下——留下的数按**输出页**数，是 2。反过来把那张跨页换成一张单页：
/// 一族从两张变一张，`001-1.png`、`001-2.png` 成了陈旧产物，收尾整个换掉时一并清走。
#[test]
fn a_split_family_is_retained_or_redone_as_a_whole() {
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
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));
    let first = fixtures::run_volume(&space, &volume);
    assert_eq!(first.volumes[0].page_count(), 3, "夹具没被切开");

    // 改旁边那一页：跨页那一族两张都留下。
    volume.page("002.png", &fixtures::gradient(fixtures::TINY));
    let second = fixtures::run_volume(&space, &volume);
    let redone = &second.volumes[0];
    assert_eq!(redone.decodes, 1, "跨页那一族被重做了");
    assert_eq!(redone.retained_pages, 2, "留下的数该按输出页数");
    assert_eq!(redone.page_count(), 3);
    assert_eq!(
        fixtures::directory_members(&redone.output),
        ["001-1.png", "001-2.png", "002.png"]
    );

    // 跨页换成单页：那一族从两张变一张，旧的两半清走。
    volume.page("001.png", &fixtures::solid(fixtures::TINY, 40));
    let third = fixtures::run_volume(&space, &volume);
    let redone = &third.volumes[0];
    assert_eq!(redone.decodes, 1, "没变的那一页也被解码了");
    assert_eq!(redone.retained_pages, 1);
    assert_eq!(redone.page_count(), 2);
    assert_eq!(
        fixtures::directory_members(&redone.output),
        ["001.png", "002.png"],
        "切开的旧两半留在了输出里"
    );
}

/// **一张灰度页都没重做、却留下了页，卷级判定仍是这条路的**（two-pass-rework/14）：只改透传文件，
/// 两页都留下、零解码、透传文件补上——报告说的是「逐页」，不是「一张灰度页都没有」，
/// 卷表上它因此不会与整卷彩页、整卷失败的卷混成一档。
#[test]
fn a_volume_with_only_its_passthrough_file_changed_keeps_every_page_and_says_per_page() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);
    volume.file("ComicInfo.xml", b"<ComicInfo><Title>2</Title></ComicInfo>");

    let report = fixtures::run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(reported.verdict, Some(VolumeVerdict::PerPage));
    assert_eq!(reported.decodes, 0, "没变的页也被解码了");
    assert_eq!(reported.retained_pages, 2);
    assert_eq!(reported.pages.len(), 0);
    assert_eq!(reported.page_count(), 2);
    assert_eq!(
        fs::read(reported.output.join("ComicInfo.xml")).expect("读回透传文件"),
        b"<ComicInfo><Title>2</Title></ComicInfo>",
        "改了的透传文件没有重写"
    );
}

/// **部分救回页也留得下，留下之后它仍是部分救回页**（two-pass-rework/14；04 号票的救回不变）。
///
/// 救回页有自己的尺寸、判据与判定，字节只取决于它自己，页级依据照写；改旁边那一页，它留下——
/// 记录里那句「salvaged …」随它走，产物与整卷重做逐字节相同。报告里的救回清单只数这一趟重做的页，
/// 留下的那一张不在里面（`VolumeReport::pages` 只列这一趟做了的）。
#[test]
fn a_salvaged_page_is_retained_like_any_other() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", &fixtures::truncated_page(fixtures::TINY));
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 128));
    let first = fixtures::run_volume(&space, &volume);
    assert_eq!(
        first.volumes[0].salvaged().count(),
        1,
        "夹具不对：头一页没被救回"
    );
    assert!(
        !first.volumes[0].isolated(),
        "夹具不对：救回页把卷送进了隔离目录"
    );

    volume.page("002.png", &fixtures::gradient(fixtures::TINY));
    let second = fixtures::run_volume(&space, &volume);

    let reported = &second.volumes[0];
    assert_eq!(reported.decodes, 1, "救回页也被重做了");
    assert_eq!(reported.retained_pages, 1);
    assert_eq!(
        reported.salvaged().count(),
        0,
        "留下的页进了这一趟的救回清单"
    );
    let text = fixtures::read_png_text(&reported.output.join("001.png"));
    assert!(
        fixtures::png_field(&text, "tonefit:reason")
            .is_some_and(|reason| reason.starts_with("salvaged ")),
        "留下的救回页丢了那句自证"
    );
    let from_scratch = tonefit::run(&Request {
        output_root: space.out_named("from-scratch"),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    assert_eq!(
        fixtures::fingerprint(&reported.output),
        fixtures::fingerprint(&from_scratch.volumes[0].output),
        "按页跳过的产物与整卷重做的不是同一份"
    );
}

/// **新切出的一张撞上留下的同名一张，照样拦下**（two-pass-rework/14；页几何批 04 号票的撞名）。
///
/// 头一趟 `001.jpg` 是单页（输出 `001.png`）、`001-1.png` 是另一页；把 `001.jpg` 换成跨页，
/// 它切出的 `001-1.png` 撞上留下的那一张——撞名要在写出第一个字节之前说，卷级失败、
/// 上一趟的输出纹丝不动。
#[test]
fn a_freshly_split_page_colliding_with_a_retained_one_is_caught() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.jpg", &fixtures::solid(fixtures::TINY, 128));
    volume.page("001-1.png", &fixtures::screentone(fixtures::TINY));
    let first = fixtures::run_volume(&space, &volume);
    assert!(first.failed_volumes.is_empty(), "夹具不对：头一趟就撞了名");
    let before = fixtures::fingerprint(&first.volumes[0].output);

    volume.page(
        "001.jpg",
        &fixtures::spread_with_gutter(
            fixtures::SPREAD_WITH_GUTTER,
            fixtures::GUTTER_CENTER,
            fixtures::GUTTER_WIDTH,
        ),
    );
    let second = fixtures::run_volume(&space, &volume);

    let [failed] = &second.failed_volumes[..] else {
        panic!("撞名没被记成卷级失败：{:?}", second.failed_volumes);
    };
    assert!(
        failed.reason.contains("001-1.png"),
        "那句原因没指出撞在哪个成员上：{}",
        failed.reason
    );
    assert_eq!(
        fixtures::fingerprint(&first.volumes[0].output),
        before,
        "撞名那一趟动了上一趟的输出"
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

/// 一页都没有的东西**不是卷**（ADR 0014 决定第 3 条），幂等因此根本问不到它。
///
/// 从前它是一个合法的卷：每一趟都把透传文件重写一遍、判定恒是 `None`，理由是
/// 「记录随页走，没有页就没有地方放记录」。发现落地之后那条理由不必再用了——
/// 它连卷都不是，报告里一条都没有。跑两趟才问，是因为这一条要排除的正是
/// 「第二趟被上一趟的输出蒙混成跳过」。
///
/// 「输出里一个字节都没有」那一半由 `tests/discovery.rs` 的
/// `nothing_without_a_page_writes_a_single_byte` 钉着，这里不复述。
#[test]
fn something_without_a_page_never_gets_as_far_as_idempotency() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("ComicInfo.xml", b"<ComicInfo/>");

    fixtures::run_volume(&space, &volume);
    let report = fixtures::run_volume(&space, &volume);

    assert!(
        report.volumes.is_empty(),
        "一页都没有的东西成了卷：{:?}",
        report.volumes.len()
    );
}

/// 一页输出 PNG 的记录里 `keyword` 那一项的取值；没写进去就是夹具或被测代码错了，当场炸。
fn recorded(output: &std::path::Path, keyword: &str) -> String {
    fixtures::png_field(&fixtures::read_png_text(output), keyword)
        .unwrap_or_else(|| panic!("{keyword} 没写进 {}", output.display()))
}

/// 两页加一个透传文件的卷。两页都小于面板，几何门在两页上都不成立——本文件测的每一条都与门无关。
fn two_pages_and_an_extra(space: &Workspace) -> Volume {
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::solid(fixtures::TINY, 128));
    volume.page("002.png", &fixtures::screentone(fixtures::TINY));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 第二趟跑完留下的两样东西：这一卷的卷级判定，以及输出里剩下哪些成员。
///
/// 「这一卷重做了没有」与「重做之后输出里有什么」是同一件事的两半，判定只答得出前一半。
struct Redone {
    verdict: Option<VolumeVerdict>,
    /// 成员清单在 [`rerun`] 里就取好：工作区是个临时目录，那个函数一返回它就被删了，
    /// 输出路径带回来也问不出东西。
    members: Vec<String>,
}

/// 跑两趟：中间按 `touch` 动一动源，按 `change` 改一改参数。返回第二趟留下的东西。
fn rerun(touch: impl FnOnce(&Volume), change: impl FnOnce(&mut Request)) -> Redone {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space);
    fixtures::run_volume(&space, &volume);

    touch(&volume);
    let mut request = fixtures::request(&space, [volume.path()]);
    change(&mut request);
    let report = tonefit::run(&request).expect("第二趟应当成功");

    let redone = report.volumes.into_iter().next().expect("一个卷");
    Redone {
        verdict: redone.verdict,
        members: fixtures::directory_members(&redone.output),
    }
}

/// 这一卷该重做，不该跳过。
fn assert_redone(verdict: Option<VolumeVerdict>, what: &str) {
    assert!(
        !matches!(verdict, Some(VolumeVerdict::Skipped { .. })),
        "{what}之后这一卷仍然被跳过了：{verdict:?}"
    );
}
