//! 报告里的计时，在 `run(Request) -> Report` 这个 seam 上测（加固批 11 号票）。
//!
//! 这里**一个具体的秒数都不主张**：那是机器快慢，不是被测代码的性质，钉住它等于把用例的
//! 红绿交给当时的负载。断言的是**结构**——这一趟走过的段该有数、没走过的段该是零、
//! 段与段不重叠、段装得进总耗时。
//!
//! 断言里出现的具体时长都是**用例自己等掉的那一段**：确认点上等人的那一截不算进计时
//! （`waiting_at_the_decision_point_is_charged_to_nobody`），摊开途中磨蹭的那一截算进摊开那一段
//! （`an_extracted_volume_times_its_extraction_and_nothing_of_it_falls_outside_the_segments`）。
//! 它们不是机器快慢，是用例摆好的输入，因此可以钉。
//!
//! 「计时不进渲染出的文字」不在这里：那是界面层的事实，由 `src/render.rs` 的
//! `the_rendered_text_says_nothing_about_how_long_it_took` 钉着。

mod fixtures;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fixtures::{Volume, Workspace};
use tonefit::{Event, Instruction, Mode, Pass, Progress, ProgressSink, Request, VolumeTiming};

/// 两页加一个透传文件的目录卷。
///
/// 透传成员是有意的：写出环节那一段的段界照**步**那一侧划（写全部成员），
/// 卷里一个透传文件都没有的话，「写全部成员」与「写全部页」在这里分不开。
fn two_pages_and_an_extra(space: &Workspace, name: &str) -> Volume {
    let volume = space.volume(name);
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    volume.page("002.png", &fixtures::screentone(fixtures::TINY));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 两页加一个透传文件的固实 `.7z`，连同它有几个成员（摊开那一段一个成员一步）。
///
/// 页取最便宜的那一张：这里问的是摊开那一段，分析环节只要走过就行。
fn two_pages_and_an_extra_in_a_seven_zip(space: &Workspace, name: &str) -> (PathBuf, u32) {
    let page = fixtures::cheap_page();
    let mut sevenz = space.sevenz(name);
    let pages = ["001.png", "002.png"];
    for page_name in pages {
        sevenz.page(page_name, &page);
    }
    sevenz.file("ComicInfo.xml", b"<ComicInfo/>");
    let members = u32::try_from(pages.len() + 1).expect("几个成员装得进 u32");
    (sevenz.write(), members)
}

/// 四段加上段外那一截，收出来的那个数。
///
/// 它该恰好等于 [`VolumeTiming::elapsed`]。**这条等式是段不重叠的哨兵**：真有两段掐的表叠在
/// 一起，四段之和就会大于总耗时，`outside_the_segments` 被饱和成零，这个和当场小于 `elapsed`。
fn accounted_for(timing: &VolumeTiming) -> Duration {
    timing.extraction
        + timing.fingerprint
        + timing.first_pass
        + timing.second_pass
        + timing.outside_the_segments()
}

#[test]
fn every_volume_and_the_whole_run_say_how_long_they_took() {
    let space = Workspace::new();
    let one = two_pages_and_an_extra(&space, "volume-a");
    let other = two_pages_and_an_extra(&space, "volume-b");

    let report = fixtures::run_paths(&space, [one.path(), other.path()]);

    assert!(report.elapsed > Duration::ZERO, "整趟没有报出耗时");
    assert_eq!(report.volumes.len(), 2);
    let mut volumes = Duration::ZERO;
    for volume in &report.volumes {
        let timing = volume.timing;
        // 目录卷不摊开：那一段是零，而不是一个很小的数。
        assert_eq!(timing.extraction, Duration::ZERO, "目录卷摊开了");
        // 其余三段都真走了：记录开着、卷要处理、模式是照做。
        assert!(timing.fingerprint > Duration::ZERO, "幂等那一道没有耗时");
        assert!(timing.first_pass > Duration::ZERO, "分析环节没有耗时");
        assert!(timing.second_pass > Duration::ZERO, "写出环节没有耗时");
        assert!(timing.elapsed > Duration::ZERO, "这一卷没有报出耗时");
        assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
        volumes += timing.elapsed;
    }
    // 整趟装得下每一卷。是「不小于」而不是「等于」：开工前那几道检查在卷外，也要摸文件系统。
    assert!(report.elapsed >= volumes, "整趟比各卷之和还短");
}

/// 幂等命中要把整卷成员读一遍才判得出来，那不是零成本，报告里得看得见（加固批 11 号票）。
#[test]
fn a_skipped_volume_still_reports_what_the_idempotency_read_cost() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space, "volume-a");
    fixtures::run_volume(&space, &volume);

    let report = fixtures::run_volume(&space, &volume);

    let skipped = &report.volumes[0];
    assert!(
        skipped.skipped(),
        "第二趟没有被跳过，这条用例测的就不是跳过了"
    );
    let timing = skipped.timing;
    assert!(timing.elapsed > Duration::ZERO, "跳过的卷报了零耗时");
    assert!(timing.fingerprint > Duration::ZERO, "幂等那一道没有耗时");
    // 提前收摊：两遍一遍都没走，那两段因此是零而不是一个很小的数。
    assert_eq!(timing.first_pass, Duration::ZERO, "跳过的卷走了分析环节");
    assert_eq!(timing.second_pass, Duration::ZERO, "跳过的卷走了写出环节");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// dry-run 一个文件都不落盘：写出环节那一段无从谈起，分析环节照走（spec 的 story 6）。
#[test]
fn a_dry_run_times_the_first_pass_and_leaves_the_second_at_zero() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space, "volume-a");

    let report = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let timing = report.volumes[0].timing;
    assert!(timing.first_pass > Duration::ZERO, "dry-run 没走分析环节");
    assert_eq!(timing.second_pass, Duration::ZERO, "dry-run 写了东西");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// `--no-metadata` 一关，幂等那一整道不在：它那一段因此是零，而不是一个很小的数。
#[test]
fn without_metadata_there_is_no_idempotency_pass_to_time() {
    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space, "volume-a");

    let report = tonefit::run(&Request {
        metadata: false,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let timing = report.volumes[0].timing;
    assert_eq!(timing.fingerprint, Duration::ZERO, "关了记录还在算指纹");
    assert!(timing.first_pass > Duration::ZERO, "分析环节没有耗时");
    assert!(timing.second_pass > Duration::ZERO, "写出环节没有耗时");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// 在**确认点**上等人的那几分钟不算进计时，两处墙钟都不算（停车场 Q41，ADR 0012）。
///
/// 会话在那里把报告画出来、等用户拿主意，人会看着报告去泡茶（ADR 0012 的《后果》）。
/// 算进来的话，`elapsed` 报出来的就不再是「库做了多久」，而是「用户拿主意花了多久」——
/// 同一个卷、同一台机器，两趟能差出几个数量级。
///
/// 断言不带余量，因此不看机器快慢：在外面掐的那个表**至少**比报出来的多一个 `WAITS`。
/// 报的数要是把等人那段算进去了，两者之差就只剩 `run` 进出之间那点零头，当场红。
#[test]
fn waiting_at_the_decision_point_is_charged_to_nobody() {
    /// 观察者在确认点上等这么久。够长，`run` 自己的零头淹不掉它；够短，用例不因此变慢。
    const WAITS: Duration = Duration::from_millis(200);

    /// 只在确认点上磨蹭的观察者，答的是继续——写出环节照走，等的那一段却不该算进它。
    ///
    /// `src/progress.rs` 的 `only_the_wait_at_the_decision_point_is_clocked_as_deliberation`
    /// 有一个同形的，隔着 crate 边界共用不了。那一个问「掐出来的那个数对不对」，
    /// 这一个问「报告上的两个数减掉它没有」——同一件事的两头。
    struct Ponders;

    impl Progress for Ponders {
        fn observe(&self, event: Event<'_>) -> Instruction {
            if matches!(
                event,
                Event::PassStarted {
                    pass: Pass::Second,
                    ..
                }
            ) {
                std::thread::sleep(WAITS);
            }
            Instruction::Continue
        }
    }

    let space = Workspace::new();
    let volume = two_pages_and_an_extra(&space, "volume-a");

    let outside = Instant::now();
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(Ponders)),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    let outside = outside.elapsed();

    assert!(
        outside.saturating_sub(report.elapsed) >= WAITS,
        "整趟的耗时把等人的那一截算进去了：外面掐的是 {outside:?}，报的是 {:?}",
        report.elapsed
    );
    let timing = report.volumes[0].timing;
    assert!(
        outside.saturating_sub(timing.elapsed) >= WAITS,
        "这一卷的耗时把等人的那一截算进去了：外面掐的是 {outside:?}，报的是 {:?}",
        timing.elapsed
    );
    // 减掉一截之后段与总仍然对得上：等人不在任何一段里，因此三段一个都没变短。
    assert!(timing.second_pass > Duration::ZERO, "写出环节没走");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// **要摊开的卷，摊开那一段有它自己的一格，段外那一截不再装着它**（say-and-stop/03）。
///
/// 「不再装着它」要一个量得到的差别才钉得住：观察者在摊开途中每一步都磨蹭 `PAUSE`，
/// 这一卷的摊开因此至少多花 `waits`（每个成员一次）。这一截落在摊开那一段里，那一段就不小于它；
/// 落在段外的话，段外那一截当场不小于它。两句各钉一头。
///
/// 与确认点上等人那一截（[`waiting_at_the_decision_point_is_charged_to_nobody`]）**不是一回事**：
/// 摊开途中库在等观察者返回才摊下一个成员，那是这一卷真花掉的墙钟，算进 `elapsed`——
/// 只是得算在摊开那一段里。
///
/// 段外那一截在这一卷上只剩几样零头（见 `VolumeTiming::outside_the_segments`），这么小的一卷上
/// 远小于 `waits`；它若装着摊开，至少就是 `waits`。
#[test]
fn an_extracted_volume_times_its_extraction_and_nothing_of_it_falls_outside_the_segments() {
    /// 摊开途中每一步磨蹭这么久。
    const PAUSE: Duration = Duration::from_millis(100);

    /// 只在摊开途中磨蹭的观察者：摊开开工起、下一个环节开工止，每一步都睡一会儿。
    #[derive(Default)]
    struct DawdlesWhileExtracting {
        extracting: AtomicBool,
    }

    impl Progress for DawdlesWhileExtracting {
        fn observe(&self, event: Event<'_>) -> Instruction {
            match event {
                Event::PassStarted { pass, .. } => {
                    self.extracting
                        .store(pass == Pass::Extraction, Ordering::Relaxed);
                }
                Event::Stepped { .. } if self.extracting.load(Ordering::Relaxed) => {
                    std::thread::sleep(PAUSE);
                }
                _ => {}
            }
            Instruction::Continue
        }
    }

    let space = Workspace::new();
    let (solid, members) = two_pages_and_an_extra_in_a_seven_zip(&space, "volume-a");
    // 摊开那一段至少多花这么久：一个成员一步，每一步磨蹭一次。
    let waits = PAUSE * members;

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(DawdlesWhileExtracting::default())),
        ..fixtures::request(&space, [solid.as_path()])
    })
    .expect("点名一个 .7z 该跑得起来");

    let timing = report.volumes[0].timing;
    assert!(
        timing.extraction >= waits,
        "摊开那一段没装下它自己花掉的时间：{:?}，而摊开途中至少磨蹭了 {waits:?}",
        timing.extraction
    );
    assert!(
        timing.outside_the_segments() < waits,
        "段外那一截还装着摊开：{:?}",
        timing.outside_the_segments()
    );
    // 其余三段照旧都走了：摊开多了一段，不是顶掉了哪一段。
    assert!(timing.fingerprint > Duration::ZERO, "幂等那一道没有耗时");
    assert!(timing.first_pass > Duration::ZERO, "分析环节没有耗时");
    assert!(timing.second_pass > Duration::ZERO, "写出环节没有耗时");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// **幂等命中而整卷跳过的归档卷，摊开那一段照样有数**：查重要读源字节，源字节要先摊开。
///
/// 与 [`a_skipped_volume_still_reports_what_the_idempotency_read_cost`] 同一个道理——
/// 跳过不是零成本——只是这种卷上跳过要付两笔。
#[test]
fn a_skipped_archive_volume_still_reports_what_the_extraction_cost() {
    let space = Workspace::new();
    let (solid, _) = two_pages_and_an_extra_in_a_seven_zip(&space, "volume-a");
    fixtures::run_paths(&space, [solid.as_path()]);

    let report = fixtures::run_paths(&space, [solid.as_path()]);

    let skipped = &report.volumes[0];
    assert!(
        skipped.skipped(),
        "第二趟没有被跳过，这条用例测的就不是跳过了"
    );
    let timing = skipped.timing;
    assert!(
        timing.extraction > Duration::ZERO,
        "跳过的归档卷没有摊开的耗时"
    );
    assert!(timing.fingerprint > Duration::ZERO, "幂等那一道没有耗时");
    assert_eq!(timing.first_pass, Duration::ZERO, "跳过的卷走了分析环节");
    assert_eq!(timing.second_pass, Duration::ZERO, "跳过的卷走了写出环节");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// **随机取的归档卷（`.cbz`）不摊开，摊开那一段是零**，而不是一个很小的数（say-and-stop/03）。
///
/// 目录卷那一半在 [`every_volume_and_the_whole_run_say_how_long_they_took`] 里；这一条问的是
/// 「归档卷一律掐摊开」那种错法——摊开由格式定（ADR 0015 决定第 3 条），不由容器形态定。
#[test]
fn an_archive_that_is_not_extracted_times_no_extraction() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();
    let mut cbz = space.cbz("volume-a");
    cbz.page("001.png", &page).page("002.png", &page);
    let cbz = cbz.write();

    let report = fixtures::run_paths(&space, [cbz.as_path()]);

    let timing = report.volumes[0].timing;
    assert_eq!(timing.extraction, Duration::ZERO, "`.cbz` 摊开了");
    assert!(timing.first_pass > Duration::ZERO, "分析环节没有耗时");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}
