//! 报告里的计时，在 `run(Request) -> Report` 这个 seam 上测（加固批 11 号票）。
//!
//! 这里**一个具体的秒数都不主张**：那是机器快慢，不是被测代码的性质，钉住它等于把用例的
//! 红绿交给当时的负载。断言的是**结构**——这一趟走过的段该有数、没走过的段该是零、
//! 段与段不重叠、段装得进总耗时。
//!
//! 「计时不进渲染出的文字」不在这里：那是界面层的事实，由 `src/render.rs` 的
//! `the_rendered_text_says_nothing_about_how_long_it_took` 钉着。

mod fixtures;

use std::time::Duration;

use fixtures::{Volume, Workspace};
use tonefit::{Mode, Request, VolumeTiming};

/// 两页加一个透传文件的目录卷。
///
/// 透传成员是有意的：第二遍那一段的段界照**步**那一侧划（写全部成员），
/// 卷里一个透传文件都没有的话，「写全部成员」与「写全部页」在这里分不开。
fn two_pages_and_an_extra(space: &Workspace, name: &str) -> Volume {
    let volume = space.volume(name);
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    volume.page("002.png", &fixtures::screentone(fixtures::TINY));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 三段加上段外那一截，收出来的那个数。
///
/// 它该恰好等于 [`VolumeTiming::elapsed`]。**这条等式是段不重叠的哨兵**：真有两段掐的表叠在
/// 一起，三段之和就会大于总耗时，`outside_the_segments` 被饱和成零，这个和当场小于 `elapsed`。
fn accounted_for(timing: &VolumeTiming) -> Duration {
    timing.fingerprint + timing.first_pass + timing.second_pass + timing.outside_the_segments()
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
        // 这一趟三段都真走了：记录开着、卷要处理、模式是照做。
        assert!(timing.fingerprint > Duration::ZERO, "幂等那一道没有耗时");
        assert!(timing.first_pass > Duration::ZERO, "第一遍没有耗时");
        assert!(timing.second_pass > Duration::ZERO, "第二遍没有耗时");
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
    assert_eq!(timing.first_pass, Duration::ZERO, "跳过的卷走了第一遍");
    assert_eq!(timing.second_pass, Duration::ZERO, "跳过的卷走了第二遍");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}

/// dry-run 一个文件都不落盘：第二遍那一段无从谈起，第一遍照走（spec 的 story 6）。
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
    assert!(timing.first_pass > Duration::ZERO, "dry-run 没走第一遍");
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
    assert!(timing.first_pass > Duration::ZERO, "第一遍没有耗时");
    assert!(timing.second_pass > Duration::ZERO, "第二遍没有耗时");
    assert_eq!(accounted_for(&timing), timing.elapsed, "段与总对不上");
}
