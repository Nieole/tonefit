//! 事件流：观察者收事件、回指令（会话批 02 号票，ADR 0011）。
//!
//! 断言全在 `run` 这个 seam 上：装一个观察者进去，看它收到什么、看它回的那个字
//! 改变了什么。事件本身的转手——「没有观察者时一步都不报」、指令只升不降——
//! 在 `src/progress.rs` 的单元用例里，那里量得到转手的次数，这里量不到。
//!
//! 步数那几条不在这里，在 `tests/concurrency.rs`：预告的步数是**上界**这件事要一个够长的卷
//! 才说得清，而那个卷在那边。

mod fixtures;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fixtures::{TINY, Workspace};
use tonefit::{Event, Instruction, Mode, Pass, Progress, ProgressSink, Request, VolumeReport};

/// 记录型观察者：事件收下来，一条不落。
///
/// 记两样。**卷报告**整份克隆一份——本文件头一条用例要拿它与 `run` 的返回值比，
/// 而事件带的是借用（见 `tonefit::Event`），不克隆就留不下。**别的事件**只留一个
/// 印出来的字样：那几条用来看流的形状，不用来比内容。
///
/// 答什么由用例事先摆好：默认继续，[`stopping_after`](Recorder::stopping_after) 摆的是
/// 「收下第 n 卷之后改口」。
#[derive(Clone, Default)]
struct Recorder(Arc<Recorded>);

#[derive(Default)]
struct Recorded {
    /// 事件的名字，按到达顺序。带字段的那几条只留变体名——顺序与形状看它就够。
    shape: Mutex<Vec<&'static str>>,
    /// 一卷跑完那条事件带着的报告，按到达顺序整份留下。
    volumes: Mutex<Vec<VolumeReport>>,
    /// 每一条「一页失败了」带着的那两样。
    failures: Mutex<Vec<(PathBuf, String)>>,
    /// 走过哪几遍，按到达顺序。
    passes: Mutex<Vec<Pass>>,
    /// 这一趟点名了几个卷（`RunStarted` 带的那个数）。
    named: AtomicUsize,
    /// 收下第几卷之后改口。`None` 即一直继续。
    stop_after: Mutex<Option<(usize, Instruction)>>,
}

impl Progress for Recorder {
    fn observe(&self, event: Event<'_>) -> Instruction {
        let name = match event {
            Event::RunStarted { volumes, .. } => {
                self.0.named.store(volumes, Ordering::Relaxed);
                "RunStarted"
            }
            Event::VolumeStarted { .. } => "VolumeStarted",
            Event::PassStarted { pass, .. } => {
                self.0.passes.lock().expect("记账没有中毒").push(pass);
                "PassStarted"
            }
            Event::Stepped { .. } => "Stepped",
            Event::PageFailed { page, reason, .. } => {
                self.0
                    .failures
                    .lock()
                    .expect("记账没有中毒")
                    .push((page.to_path_buf(), reason.to_owned()));
                "PageFailed"
            }
            Event::VolumeFinished { report, .. } => {
                self.0
                    .volumes
                    .lock()
                    .expect("记账没有中毒")
                    .push(report.clone());
                "VolumeFinished"
            }
            Event::RunFinished { .. } => "RunFinished",
            _ => "别的",
        };
        self.0.shape.lock().expect("记账没有中毒").push(name);
        self.answer()
    }
}

impl Recorder {
    /// 收下 `after` 个卷之后改口回 `instruction`。
    fn stopping_after(after: usize, instruction: Instruction) -> Self {
        let recorder = Self::default();
        *recorder.0.stop_after.lock().expect("摆好答案") = Some((after, instruction));
        recorder
    }

    fn answer(&self) -> Instruction {
        // 就地问长度，不走 `volumes()`：那一个要把至今收下的每一份卷报告整份克隆一遍，
        // 而这里只要一个数——每条事件都克隆一遍整卷的逐页结果，观察者自己就成了热路径。
        let done = self.0.volumes.lock().expect("记账没有中毒").len();
        match *self.0.stop_after.lock().expect("读回答案") {
            Some((after, instruction)) if done >= after => instruction,
            _ => Instruction::Continue,
        }
    }

    fn shape(&self) -> Vec<&'static str> {
        self.0.shape.lock().expect("记账没有中毒").clone()
    }

    fn volumes(&self) -> Vec<VolumeReport> {
        self.0.volumes.lock().expect("记账没有中毒").clone()
    }

    fn failures(&self) -> Vec<(PathBuf, String)> {
        self.0.failures.lock().expect("记账没有中毒").clone()
    }

    fn passes(&self) -> Vec<Pass> {
        self.0.passes.lock().expect("记账没有中毒").clone()
    }

    fn named(&self) -> usize {
        self.0.named.load(Ordering::Relaxed)
    }
}

/// 一个两页的小卷。本文件问的是事件流的形状，页越小跑得越快。
fn small_volume(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    let page = fixtures::full_bleed_gradient(TINY);
    volume.page("001.png", &page);
    volume.page("002.png", &page);
    volume
}

/// 攒出来的报告与主入口的返回值**等价**：事件流就是报告的增量（ADR 0011 决定第 2 条）。
///
/// 这是本票的要害。会话不等 `run` 返回就要画报告，而它画的必须与命令行最后印的是同一份——
/// 两边各攒各的就会漂移，而漂移出来的是「跑到一半看到的」与「跑完看到的」长得不一样。
///
/// 比的是整份 `Debug`，不是几个挑出来的字段：漏掉一项就等于给那一项开了漂移的口子，
/// 而报告的字段还在长（页几何这一批就加了三项）。
#[test]
fn what_the_observer_collects_is_the_report_the_entry_point_returns() {
    let space = Workspace::new();
    let first = small_volume(&space, "volume-a");
    let second = small_volume(&space, "volume-b");
    let recorder = Recorder::default();

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [first.path(), second.path()])
    })
    .expect("处理应当成功");

    let collected = recorder.volumes();
    assert_eq!(collected.len(), 2, "两个卷该报两条");
    assert_eq!(
        format!("{collected:?}"),
        format!("{:?}", report.volumes),
        "攒出来的报告与返回的那一份不是同一份"
    );
}

/// 一趟顺当跑下来报得出的那六条，**次序**对得上管线。
///
/// 次序本身是要点：一趟开始排在第一条、这一趟完了排在最后一条，卷级那两条把每一卷夹在中间。
/// 会话的主区照这个次序画——开卷那一条不到，当前卷条就无从起头。
///
/// 第七条「一页失败了」不在这里：它要一张坏图才报得出来，
/// 见 [`a_failed_page_is_reported_the_moment_it_fails`]。
#[test]
fn the_six_events_of_a_clean_run_arrive_in_the_order_the_pipeline_does_them() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let recorder = Recorder::default();

    tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let shape = recorder.shape();
    assert_eq!(
        shape.first(),
        Some(&"RunStarted"),
        "头一条不是「这一趟开始了」：{shape:?}"
    );
    assert_eq!(
        shape.last(),
        Some(&"RunFinished"),
        "末一条不是「这一趟完了」：{shape:?}"
    );
    assert_eq!(shape[1], "VolumeStarted", "开卷没紧接在开工之后：{shape:?}");
    assert_eq!(
        shape[shape.len() - 2],
        "VolumeFinished",
        "一卷跑完没紧接在收场之前：{shape:?}"
    );
    assert_eq!(recorder.named(), 1, "开工那一条报错了卷数");
    assert!(
        shape.iter().filter(|name| **name == "Stepped").count() > 1,
        "一步都没报到：{shape:?}"
    );

    // 「在走哪一遍」：这一趟三遍都走——写元数据、Process 模式。
    assert_eq!(
        recorder.passes(),
        vec![Pass::Fingerprint, Pass::First, Pass::Second],
        "三遍没有按次序报出来"
    );
}

/// 各段自己可能不在，报的就少一遍：`--no-metadata` 没有幂等那一道，dry-run 没有第二遍。
///
/// 钉的是「报的是这一趟**真要走的**那几遍」。报一遍走不到的路，会话的当前卷条
/// 就会停在一个永远走不完的阶段上。
#[test]
fn a_pass_that_will_not_happen_is_not_announced() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");

    let recorder = Recorder::default();
    tonefit::run(&Request {
        mode: Mode::DryRun,
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("试算应当成功");
    assert_eq!(
        recorder.passes(),
        vec![Pass::Fingerprint, Pass::First],
        "dry-run 没有第二遍"
    );

    let recorder = Recorder::default();
    tonefit::run(&Request {
        metadata: false,
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");
    assert_eq!(
        recorder.passes(),
        vec![Pass::First, Pass::Second],
        "关掉元数据就没有幂等那一道"
    );
}

/// 一页失败了当场报出去，带着原因——不等整卷跑完（09 号票的会话主区）。
///
/// 「当场」这件事在这里由**次序**钉住：失败那一条排在这一卷的 `VolumeFinished` 之前。
/// 等报告出来才知道哪一页坏了，那份报告要等整卷——一个几百页的卷里，
/// 那是几分钟之后的事。
#[test]
fn a_failed_page_is_reported_the_moment_it_fails() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::full_bleed_gradient(TINY));
    volume.file("002.png", b"not a png at all");
    let recorder = Recorder::default();

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("一张坏图不该毁掉整卷");

    let failures = recorder.failures();
    assert_eq!(failures.len(), 1, "失败页没有报出来：{failures:?}");
    assert!(failures[0].0.ends_with("002.png"), "指错了页：{failures:?}");
    assert!(!failures[0].1.is_empty(), "报了失败却没说原因");

    // 同一页在报告里也有一份，两处指的是同一页（一份是增量，一份是结果）。
    let in_report: Vec<&PathBuf> = report.failures().map(|page| &page.source).collect();
    assert_eq!(
        in_report,
        vec![&failures[0].0],
        "事件指的那一页与报告里的那一页不是同一页"
    );

    let shape = recorder.shape();
    let failed_at = shape.iter().position(|name| *name == "PageFailed");
    let finished_at = shape.iter().position(|name| *name == "VolumeFinished");
    assert!(
        failed_at < finished_at,
        "失败页要等整卷跑完才说得出口：{shape:?}"
    );
}

/// 观察者在第 2 卷之后答**收尾**：当前卷跑完就停，盘上只有完整的卷（ADR 0013 决定第 1 条）。
///
/// 三件事一起断言：输出只有 2 个卷、报告只有 2 卷、这一趟不算失败（`run` 回的是 `Ok`，
/// 一个卷都没被隔离——命令行那一路的退出码 0 就是这两条拼出来的）。
///
/// 第三卷**一页都不该做**：它连开卷那条事件都不该有。检查点在卷边界上，
/// 停下来的结果是一份可以接着走的输出，不是一个需要清理的现场。
#[test]
fn finishing_up_after_the_second_volume_leaves_two_whole_volumes() {
    let space = Workspace::new();
    let volumes: Vec<fixtures::Volume> = ["volume-a", "volume-b", "volume-c"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let recorder = Recorder::stopping_after(2, Instruction::Finish);

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, volumes.iter().map(fixtures::Volume::path))
    })
    .expect("按停不是失败");

    assert_eq!(report.volumes.len(), 2, "报告不是两卷");
    assert!(!report.any_isolated(), "按停不该让任何卷进隔离目录");
    assert_eq!(
        recorder
            .shape()
            .iter()
            .filter(|name| **name == "VolumeStarted")
            .count(),
        2,
        "第三卷开工了：检查点没落在卷边界上"
    );
    let mut left = std::fs::read_dir(space.out())
        .expect("列输出根")
        .map(|entry| {
            entry
                .expect("读目录项")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    left.sort();
    assert_eq!(left, ["volume-a", "volume-b"], "盘上不是两个完整的卷");

    // 下一趟幂等接着走：做过的两卷跳过，剩下那一卷这才做。
    let second = Recorder::default();
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(second.clone())),
        ..fixtures::request(&space, volumes.iter().map(fixtures::Volume::path))
    })
    .expect("下一趟应当成功");
    assert_eq!(
        report
            .volumes
            .iter()
            .map(VolumeReport::skipped)
            .collect::<Vec<_>>(),
        [true, true, false],
        "接着走的那一趟没有把做过的两卷跳过"
    );
}

/// 中止此刻至少停得下来：力度更强的那个字不该比收尾停得更晚。
///
/// **它自己那个页边界检查点与丢弃 `partial` 由 04 号票落地**，本票只认下这个字。
/// 这条用例钉的正是那一层：卷边界上它照停，因此 04 号票要加的是「更早停」，
/// 而不是「让它开始起作用」。
#[test]
fn aborting_stops_at_least_as_early_as_finishing_up() {
    let space = Workspace::new();
    let volumes: Vec<fixtures::Volume> = ["volume-a", "volume-b"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let recorder = Recorder::stopping_after(1, Instruction::Abort);

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, volumes.iter().map(fixtures::Volume::path))
    })
    .expect("按停不是失败");

    assert_eq!(report.volumes.len(), 1, "中止之后还接着做了下一卷");
}

/// 观察者慢得离谱，管线照样跑得完：它是在没拿着锁的地方被调到的。
///
/// 会话要在决策点上等人拿主意（ADR 0012 决定第 3 条），因此「很久不返回」是观察者
/// **正常**的样子。库这一侧要是在持锁处调它，等人的那一刻别的线程就一起停住了——
/// 而那种停住不会报错，只会看起来像卡死。
///
/// 这条用例买的是那一侧的下限：真在持锁处调它，第一遍那几条计算线程会互相等到超时，
/// 这一趟就跑不完。每条事件只睡 1 毫秒，一个两页的卷统共十几条。
#[test]
fn a_slow_observer_does_not_wedge_the_pipeline() {
    struct Slow;
    impl Progress for Slow {
        fn observe(&self, _event: Event<'_>) -> Instruction {
            std::thread::sleep(std::time::Duration::from_millis(1));
            Instruction::Continue
        }
    }

    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(Slow)),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("慢观察者不该让这一趟跑不完");

    assert_eq!(report.volumes[0].page_count(), 2, "跑是跑完了，但少出了页");
}
