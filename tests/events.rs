//! 事件流：观察者收事件、回指令（会话批 02 号票，ADR 0011）。
//!
//! 断言全在 `run` 这个 seam 上：装一个观察者进去，看它收到什么、看它回的那个字
//! 改变了什么。事件本身的转手——「没有观察者时一步都不报」、指令只升不降——
//! 在 `src/progress.rs` 的单元用例里，那里量得到转手的次数，这里量不到。
//!
//! 步数那几条不在这里，在 `tests/concurrency.rs`：预告的步数是**上界**这件事要一个够长的卷
//! 才说得清，而那个卷在那边。**全局总步数是个例外**——它问的是开工那条事件与开卷那几条
//! 之间的关系，是流的形状，跟卷有多长无关。
//!
//! **预扫**那几条也在这里（会话批 03 号票）：它落在事件流的最前头——一卷点不开就整趟拒绝、
//! 一条事件都不发，而它列出来的东西管线接着用、不重列一遍。

mod fixtures;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fixtures::{TINY, Workspace};
use tonefit::{
    Dither, Event, FitMode, Instruction, Mode, Pass, Progress, ProgressSink, Request, RunOutcome,
    VolumeReport,
};

/// 记录型观察者：事件收下来，一条不落。
///
/// **卷报告**整份克隆一份——本文件头一条用例要拿它与 `run` 的返回值比，
/// 而事件带的是借用（见 `tonefit::Event`），不克隆就留不下。带字段的另外几条
/// （失败页、卷级失败、收场）各留下它带的那几样，因为有用例要拿它们与报告比。
/// **其余事件**只留一个印出来的字样：那几条用来看流的形状，不用来比内容。
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
    /// 每一条「一整卷没做成」带着的那两样（05 号票）。
    volume_failures: Mutex<Vec<(PathBuf, String)>>,
    /// 收场那一条带着的「这一趟是怎么收的场」。没收到那一条就是 `None`。
    outcome: Mutex<Option<RunOutcome>>,
    /// 走过哪几遍，按到达顺序。
    passes: Mutex<Vec<Pass>>,
    /// 这一趟点名了几个卷（`RunStarted` 带的那个数）。
    named: AtomicUsize,
    /// 这一趟最多走多少步（`RunStarted` 带的那个数，预扫算出来的全局总步数）。
    global_steps: AtomicU64,
    /// 每个卷开始时预告的步数，按开始顺序。
    volume_steps: Mutex<Vec<u64>>,
    /// 收下第几卷之后改口。`None` 即一直继续。
    stop_after: Mutex<Option<(usize, Instruction)>>,
}

impl Progress for Recorder {
    fn observe(&self, event: Event<'_>) -> Instruction {
        let name = match event {
            Event::RunStarted { volumes, steps, .. } => {
                self.0.named.store(volumes, Ordering::Relaxed);
                self.0.global_steps.store(steps, Ordering::Relaxed);
                "RunStarted"
            }
            Event::VolumeStarted { steps, .. } => {
                self.0
                    .volume_steps
                    .lock()
                    .expect("记账没有中毒")
                    .push(steps);
                "VolumeStarted"
            }
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
            Event::VolumeFailed { volume, reason, .. } => {
                self.0
                    .volume_failures
                    .lock()
                    .expect("记账没有中毒")
                    .push((volume.to_path_buf(), reason.to_owned()));
                "VolumeFailed"
            }
            Event::RunFinished { outcome, .. } => {
                *self.0.outcome.lock().expect("记账没有中毒") = Some(outcome);
                "RunFinished"
            }
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

    fn volume_failures(&self) -> Vec<(PathBuf, String)> {
        self.0.volume_failures.lock().expect("记账没有中毒").clone()
    }

    /// 收场那一条报的是什么。一条都没收到时是 `None`——「报过开工就一定报得到收场」
    /// 正是靠这个分得开（停车场 Q39）。
    fn outcome(&self) -> Option<RunOutcome> {
        *self.0.outcome.lock().expect("记账没有中毒")
    }

    fn passes(&self) -> Vec<Pass> {
        self.0.passes.lock().expect("记账没有中毒").clone()
    }

    fn named(&self) -> usize {
        self.0.named.load(Ordering::Relaxed)
    }

    fn global_steps(&self) -> u64 {
        self.0.global_steps.load(Ordering::Relaxed)
    }

    fn volume_steps(&self) -> Vec<u64> {
        self.0.volume_steps.lock().expect("记账没有中毒").clone()
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

/// 中止在**卷边界上**也停得下来：力度更强的那个字不该比收尾停得更晚。
///
/// 它按在一卷跑完那条事件上，而那正是收尾生效的地方——两级停在这一道上给出同一个答案。
/// 中止**更早**停的那一道在页边界上，见
/// [`aborting_at_a_page_boundary_throws_the_partial_container_away`]
/// 与 [`the_two_checkpoints_stop_at_two_different_boundaries`]。
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

/// 观察者在**页边界**上答中止：那一卷当场停下，它那格 `partial` 被丢掉
/// （04 号票，ADR 0013 决定第 2 条）。
///
/// 三件事在这一条里连成一串，缺一件断言就落空：
///
/// 一是按下那个字的**当口**——第二遍已经写出一张页，那格 `partial` 真的在盘上。
/// 不看这一眼，「丢弃」测的就可能只是「它压根没建出来过」。
///
/// 二是中止之后**最终位置上没有它这一趟的任何痕迹**：既没有 `volume-a`，
/// 也没有那格 `volume-a.partial`。看的是那个目录本身，不是 `run` 的返回值——
/// ADR 0013 的核心承诺是盘上的事实，返回值证明不了它。
///
/// 三是**下一趟整卷重做**：那一卷等于没做，幂等不该把它误判成做完了。
#[test]
fn aborting_at_a_page_boundary_throws_the_partial_container_away() {
    let space = Workspace::new();
    let volume = three_page_volume(&space, "volume-a");
    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-a",
        Pass::Second,
        AFTER_THE_FIRST_PAGE,
        Instruction::Abort,
    );

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("按停不是失败");

    assert_eq!(
        stop.names_when_pressed(),
        Some(vec!["volume-a.partial".to_owned()]),
        "按下中止的那一刻盘上不是一格写了一半的临时容器"
    );
    assert!(
        report.volumes.is_empty(),
        "被中止的那一卷进了报告：{:?}",
        report.volumes
    );
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "中止之后输出根下还留着东西"
    );

    // 下一趟：整卷重做，一张不少。
    let again = fixtures::run_paths(&space, [volume.path()]);
    assert!(
        !again.volumes[0].skipped(),
        "被中止的那一卷下一趟被当成做完了"
    );
    assert_eq!(
        fixtures::directory_members(&again.volumes[0].output),
        ["001.png", "002.png", "003.png"],
        "重做出来的不是完整的一卷"
    );
}

/// 页边界那个检查点在**每一遍**上都在：第一遍答中止，第二遍连开工都没有。
///
/// 「立刻停」不能等到写出那一遍才生效——一个几千页的卷，第一遍是解码、缩放、算判据，
/// 那是这一趟最贵的一段。停在第一遍的页边界上，第二遍那格 `partial` 因此连建都没建过。
///
/// 判别式是**点名那个卷走过哪几遍**：第二遍不在那张单子上，就说明中止没有等到写出才停。
/// 光看盘上没东西是不够的——中止在第二遍写完第一张之后才停，盘上同样什么都不剩。
#[test]
fn aborting_in_the_first_pass_never_lets_the_second_one_start() {
    let space = Workspace::new();
    let volume = three_page_volume(&space, "volume-a");
    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-a",
        Pass::First,
        AFTER_THE_FIRST_PAGE,
        Instruction::Abort,
    );

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("按停不是失败");

    assert_eq!(
        stop.passes(),
        vec![Pass::Fingerprint, Pass::First],
        "中止在第一遍上按下，第二遍却照样开了工"
    );
    assert!(report.volumes.is_empty(), "被中止的那一卷进了报告");
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "第二遍都没开工，输出根下却有东西"
    );
}

/// 页边界那个检查点在**幂等这一道**上也在：那一遍答中止，它后面两遍一遍都不开工。
///
/// 两个数一起断言，各钉一头：
///
/// - **走过哪几遍**只有幂等那一道——中止没有等到解码开始才生效；
/// - **那一道停在第几步**是按下去的那一步，不是整卷的成员数——循环头上那个检查点
///   真的拦住了后面的成员。只看前一个数的话，把循环头上的 `break` 删掉它照样绿。
#[test]
fn aborting_in_the_fingerprint_pass_never_lets_the_passes_after_it_start() {
    let space = Workspace::new();
    let volume = three_page_volume(&space, "volume-a");
    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-a",
        Pass::Fingerprint,
        AFTER_THE_FIRST_PAGE,
        Instruction::Abort,
    );

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("按停不是失败");

    assert_eq!(
        stop.passes(),
        vec![Pass::Fingerprint],
        "中止在幂等这一道上按下，后面的遍却照样开了工"
    );
    assert_eq!(
        stop.steps(),
        AFTER_THE_FIRST_PAGE,
        "幂等这一道在中止之后把余下的成员也读完了"
    );
    assert!(report.volumes.is_empty(), "被中止的那一卷进了报告");
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "第二遍都没开工，输出根下却有东西"
    );
}

/// 透传文件也在页边界那个检查点的管辖里：页写完了才答中止，那个透传成员就不再写。
///
/// 第二遍写出的是**输出成员**，页与透传文件都算（`CONTEXT.md` 的《进度》：第二遍写全部
/// 输出成员）。页写完之后还有一截，那一截同样要停得下来。
///
/// 判别式只能是**步数**：这一卷横竖要被丢掉，写没写那个透传文件在盘上留不下痕迹。
/// 停在最后一张页上，第二遍就该正好走 [`PAGES`] 步；透传那个循环头拦不住的话是 `PAGES + 1`。
#[test]
fn aborting_after_the_last_page_stops_before_the_pass_through_files() {
    let space = Workspace::new();
    let volume = three_page_volume(&space, "volume-a");
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-a",
        Pass::Second,
        AFTER_THE_LAST_PAGE,
        Instruction::Abort,
    );

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("按停不是失败");

    assert_eq!(stop.steps(), PAGES, "中止之后第二遍还把透传文件写了出去");
    assert!(report.volumes.is_empty(), "被中止的那一卷进了报告");
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "中止之后输出根下还留着东西"
    );
}

/// 中止只丢**它自己建的那一格**，别的一样都不碰（ADR 0013 的《后果》与《不要做的「简化」》）。
///
/// 场上摆了三样上一趟留下的东西，中止之后三样都要原封不动：
///
/// - 这一趟先做完并**已经改名**到位的那个卷（`volume-a`）——中止发生在它之后；
/// - 被中止的那个卷在最终位置上**上一趟的成品**（`volume-b`）——逐字节比，
///   收尾改名是最终位置唯一被碰到的那一步，不走它就一个字节都没动过；
/// - 隔离目录里那份**过期副本**——它是上一趟的产物，去留由用户定，
///   中止清的只有本次运行自己建的那一格。
#[test]
fn aborting_touches_nothing_but_the_partial_it_built_itself() {
    let space = Workspace::new();
    let done = small_volume(&space, "volume-a");
    let stopped = three_page_volume(&space, "volume-b");

    // 上一趟：两卷都做完。
    fixtures::run_paths(&space, [done.path(), stopped.path()]);
    let previous = fixtures::fingerprint(&space.out().join("volume-b"));
    // 手摆一份过期副本在另一个去处：上一趟这一卷有坏页时留下的就是这个样子。
    let superseded = space.out().join("_isolated").join("volume-b");
    std::fs::create_dir_all(&superseded).expect("建隔离目录");
    std::fs::write(superseded.join("001.png"), b"shabby but the user's").expect("摆一份过期副本");
    let stale = fixtures::fingerprint(&superseded);

    // 两卷的源都添一页，这一趟谁都不被幂等跳过。
    let page = fixtures::full_bleed_gradient(TINY);
    done.page("003.png", &page);
    stopped.page("004.png", &page);

    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-b",
        Pass::Second,
        AFTER_THE_FIRST_PAGE,
        Instruction::Abort,
    );
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [done.path(), stopped.path()])
    })
    .expect("按停不是失败");

    assert_eq!(report.volumes.len(), 1, "报告里不是只剩做完的那一卷");
    assert_eq!(
        fixtures::directory_members(&space.out().join("volume-a")),
        ["001.png", "002.png", "003.png"],
        "已经改名成功的那一卷被中止动过了"
    );
    assert_eq!(
        fixtures::fingerprint(&space.out().join("volume-b")),
        previous,
        "被中止的那一卷在最终位置上的上一趟成品被动过了"
    );
    assert_eq!(
        fixtures::fingerprint(&superseded),
        stale,
        "隔离目录里那份过期副本被中止碰了"
    );
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["_isolated", "volume-a", "volume-b"],
        "中止之后输出根下多了或少了东西"
    );
}

/// 归档卷那一格 `partial` 是个**文件**，中止照样把它丢掉。
///
/// 两种容器形态同形——改名成功才算数（`CONTEXT.md` 的《会话》）——因此这一条与目录卷那一条
/// 断言的是同一件事。分开跑是因为丢弃走的是两条不同的路：一条 `remove_dir_all`，
/// 一条先放掉写入器再 `remove_file`（Windows 上还开着句柄的文件删不掉）。
#[test]
fn aborting_an_archive_volume_throws_its_partial_file_away() {
    let space = Workspace::new();
    let page = fixtures::full_bleed_gradient(TINY);
    let mut cbz = space.cbz("volume-a");
    for name in ["001.png", "002.png", "003.png"] {
        cbz.page(name, &page);
    }
    let packed = cbz.write();
    let stop = StopsAtAPageBoundary::new(
        &space,
        "volume-a.cbz",
        Pass::Second,
        AFTER_THE_FIRST_PAGE,
        Instruction::Abort,
    );

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(stop.clone())),
        ..fixtures::request(&space, [packed.as_path()])
    })
    .expect("按停不是失败");

    assert_eq!(
        stop.names_when_pressed(),
        Some(vec!["volume-a.cbz.partial".to_owned()]),
        "按下中止的那一刻盘上不是一格写了一半的临时归档"
    );
    assert!(report.volumes.is_empty(), "被中止的那一卷进了报告");
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "中止之后输出根下还留着东西"
    );
}

/// 卷边界与页边界**各有一个**检查点，两者停出来的结果分得开（ADR 0013 的《后果》）。
///
/// 同一个夹具、同一个按下去的时机，只换那一个字：
///
/// - **收尾**问的是卷边界——当前卷照样写完并改名，盘上留下一整卷，下一趟幂等接着走；
/// - **中止**问的是页边界——那一卷当场停下、`partial` 丢掉，等于没做。
///
/// 一个检查点做两件事、或者收尾误落到页边界上，这一条当场红：那时两支给出同一个答案。
///
/// 收尾那一支按了**两处**页边界：第二遍上一次、第一遍上一次。页边界那个检查点在每一遍上
/// 都在（见上面那三条），因此「收尾不抢它的活」也要在每一遍上都成立——
/// 只在第二遍上对照的话，第一遍那个循环头误认了收尾，这一条看不出来。
#[test]
fn the_two_checkpoints_stop_at_two_different_boundaries() {
    let finishing = Workspace::new();
    let volume = three_page_volume(&finishing, "volume-a");
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(StopsAtAPageBoundary::new(
            &finishing,
            "volume-a",
            Pass::Second,
            AFTER_THE_FIRST_PAGE,
            Instruction::Finish,
        ))),
        ..fixtures::request(&finishing, [volume.path()])
    })
    .expect("按停不是失败");
    assert_eq!(report.volumes.len(), 1, "收尾没让当前卷跑完");
    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        ["001.png", "002.png", "003.png"],
        "收尾在页边界上就把当前卷截断了"
    );
    assert_eq!(
        fixtures::names_in(&finishing.out()),
        ["volume-a"],
        "收尾之后盘上不是一整卷"
    );

    // 同一个字按在第一遍的页边界上，答案该一模一样：收尾不在页边界上生效。
    let finishing_early = Workspace::new();
    let volume = three_page_volume(&finishing_early, "volume-a");
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(StopsAtAPageBoundary::new(
            &finishing_early,
            "volume-a",
            Pass::First,
            AFTER_THE_FIRST_PAGE,
            Instruction::Finish,
        ))),
        ..fixtures::request(&finishing_early, [volume.path()])
    })
    .expect("按停不是失败");
    assert_eq!(report.volumes.len(), 1, "收尾在第一遍上就把当前卷停掉了");
    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        ["001.png", "002.png", "003.png"],
        "收尾在第一遍的页边界上截断了当前卷"
    );

    let aborting = Workspace::new();
    let volume = three_page_volume(&aborting, "volume-a");
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(StopsAtAPageBoundary::new(
            &aborting,
            "volume-a",
            Pass::Second,
            AFTER_THE_FIRST_PAGE,
            Instruction::Abort,
        ))),
        ..fixtures::request(&aborting, [volume.path()])
    })
    .expect("按停不是失败");
    assert!(report.volumes.is_empty(), "中止把当前卷跑完了");
    assert_eq!(
        fixtures::names_in(&aborting.out()),
        Vec::<String>::new(),
        "中止之后盘上还留着那一卷"
    );
}

/// [`three_page_volume`] 有几页。按停那几条按下去的时机按它数出来，因此只此一个出处。
const PAGES: usize = 3;

/// 第二遍写完**头一张**页就按下去：那一刻 `partial` 里正好装着一张页，
/// 后面还有页没写——「当场停下」因此看得出来。幂等那一道上它是「头一个成员刚喂进哈希」。
const AFTER_THE_FIRST_PAGE: usize = 1;

/// 第二遍写完**最后一张**页才按下去：页都写完了，下一个要写的是透传文件。
/// 拿它问的是透传那个循环头拦不拦得住。
const AFTER_THE_LAST_PAGE: usize = PAGES;

/// 一个 [`PAGES`] 页的卷。中止那几条要它有页可写——第二遍写完第一张才有一格装着东西的 `partial`。
fn three_page_volume(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    let page = fixtures::full_bleed_gradient(TINY);
    for ordinal in 1..=PAGES {
        volume.page(&format!("{ordinal:03}.png"), &page);
    }
    volume
}

/// 在点名那个卷**某一遍的页边界**上按下一个字，并在按下的那一刻看一眼输出根。
///
/// 点名遍与点名卷都是为了让按下去的时机**落得准**：一趟里每一卷各走三遍，
/// 不点名就只按得到头一卷的头一遍。
///
/// 看那一眼是「`partial` 确实建出来过」唯一能测到的形式：跑完再看只看得见它已经不在了，
/// 而「从没建过」与「建了又丢掉」在那个形式上分不开（同一个手法见 `tests/container.rs`
/// 的 `WatchDuringRun`）。有东西可看的只有第二遍——写出去的页都在那格 `partial` 里。
#[derive(Clone)]
struct StopsAtAPageBoundary {
    /// 输出根。按下那个字的一刻看的就是它。
    out: PathBuf,
    /// 只在这个卷上按。卷标识的末一段与它相等即是它。
    volume: String,
    /// 只在这一遍上按。
    pass: Pass,
    /// 这一遍走完几步之后按。取值见 [`AFTER_THE_FIRST_PAGE`] 与 [`AFTER_THE_LAST_PAGE`]。
    after: usize,
    /// 按的是哪个字。
    instruction: Instruction,
    /// 观察者要交给库，用例又要读回它看见的那一眼，因此这一格共享。
    seen: Arc<Mutex<Pressed>>,
}

#[derive(Default)]
struct Pressed {
    /// 当前这一卷是点名的那个吗。
    named: bool,
    /// 点名那个卷走过哪几遍，按到达顺序。「中止之后第二遍还开不开工」问的就是它。
    passes: Vec<Pass>,
    /// 正在走的是点名那一遍吗。
    walking: bool,
    /// 点名那一遍走了几步。**只在进入那一遍时归零**，此后一路留着——
    /// 用例要读的是它最后停在第几步（透传那个循环头有没有拦住，只看得出这一个数），
    /// 而那一遍走完之后还会有别的遍开工。
    steps: usize,
    /// 按下那个字的一刻输出根下有哪些名字。没按过就是 `None`。
    names_when_pressed: Option<Vec<String>>,
}

impl StopsAtAPageBoundary {
    fn new(
        space: &Workspace,
        volume: &str,
        pass: Pass,
        after: usize,
        instruction: Instruction,
    ) -> Self {
        Self {
            out: space.out(),
            volume: volume.to_owned(),
            pass,
            after,
            instruction,
            seen: Arc::new(Mutex::new(Pressed::default())),
        }
    }

    /// 按下那个字的一刻输出根下有哪些名字。一趟里只看一眼——第一眼。
    fn names_when_pressed(&self) -> Option<Vec<String>> {
        self.seen
            .lock()
            .expect("记账没有中毒")
            .names_when_pressed
            .clone()
    }

    /// 点名那个卷走过哪几遍。
    fn passes(&self) -> Vec<Pass> {
        self.seen.lock().expect("记账没有中毒").passes.clone()
    }

    /// 点名那一遍统共走了几步。
    fn steps(&self) -> usize {
        self.seen.lock().expect("记账没有中毒").steps
    }
}

impl Progress for StopsAtAPageBoundary {
    fn observe(&self, event: Event<'_>) -> Instruction {
        let mut seen = self.seen.lock().expect("记账没有中毒");
        match event {
            Event::VolumeStarted { volume, .. } => {
                seen.named = volume
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy() == self.volume);
                seen.walking = false;
            }
            Event::PassStarted { pass, .. } => {
                if seen.named {
                    seen.passes.push(pass);
                }
                seen.walking = seen.named && pass == self.pass;
                if seen.walking {
                    seen.steps = 0;
                }
            }
            Event::Stepped { .. } if seen.walking => seen.steps += 1,
            _ => {}
        }
        if !seen.walking || seen.steps < self.after {
            return Instruction::Continue;
        }
        if seen.names_when_pressed.is_none() {
            seen.names_when_pressed = Some(fixtures::names_in(&self.out));
        }
        self.instruction
    }
}

/// 开工那条事件带的全局总步数，**等于各卷预告之和**（会话批 03 号票）。
///
/// 这是预扫存在的理由：一个卷要走多少步得先枚举它的成员才知道，而枚举原先发生在处理那一卷时——
/// 「整趟还剩多久」于是在开工时无从算起。等号一破，全局条要么冲过头要么停在半路，
/// 而剩余时间跟着一起胡说。
///
/// 三个卷，不是一个：一个卷时和与项恰好相等，等号成不成立看不出来。
///
/// 它钉的是**接线**，不是算术：两个数取自同一个 `Surveyed::steps`，因此这条等号在实现里
/// 是构造出来的，而这条用例守的是它别被拆开——`RunStarted` 改成另算一遍、或开卷那一条
/// 改报别的数，它当场红。「各卷步数算得对不对」在别处：`src/lib.rs` 的
/// `the_write_segment_counts_output_pages_and_the_read_segments_count_source_pages`
/// 逐段验算，`tests/concurrency.rs` 在一个够长的卷上比逐字的数。
#[test]
fn the_global_step_count_is_the_sum_of_what_each_volume_announces() {
    let space = Workspace::new();
    let volumes: Vec<fixtures::Volume> = ["volume-a", "volume-b", "volume-c"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let recorder = Recorder::default();

    tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, volumes.iter().map(fixtures::Volume::path))
    })
    .expect("处理应当成功");

    let announced = recorder.volume_steps();
    assert_eq!(announced.len(), 3, "三个卷该报三条开卷");
    assert!(announced.iter().all(|steps| *steps > 0), "有卷预告了零步");
    assert_eq!(
        recorder.global_steps(),
        announced.iter().sum::<u64>(),
        "全局总步数不等于各卷预告之和：{announced:?}"
    );
}

/// 一条点不开的路径让**整趟**当场被拒，且发生在任何卷级事件之前（会话批 03 号票）。
///
/// 三件事一起断言：`run` 回的是 `Err`、一条事件都没报到、输出根下一个文件都没有。
/// 第三件是要害——好卷排在坏路径**前面**，管线要是按点名顺序边做边发现，那一卷已经落了盘。
/// 理由与「处理范围为空是错误」同一条：范围层错了可能写到别人的目录里。
///
/// 顺带钉住**逐条列出**：两条坏路径要在同一句话里都说出来。点名十个卷写错三个路径的人
/// 该一次改完，不该被逐个叫回来三趟。
///
/// 坏的那两个是**损坏的归档**（`CONTEXT.md` 的《失败》：卷级失败列的第一批之一）：
/// 路径不在、扩展名不对那两种更早就被开工前那道去处校验挡下了——它算去处也要看路径
/// （见 `source::planned_output`）。到得了预扫的是「路径像个卷、打开才知道不是」这一类。
#[test]
fn a_path_that_cannot_be_opened_refuses_the_whole_run_before_any_volume_event() {
    let space = Workspace::new();
    let good = small_volume(&space, "volume-a");
    let broken = space.stray_file("坏卷一.cbz", b"not a zip at all");
    let also_broken = space.stray_file("坏卷二.cbz", b"nor is this");
    let recorder = Recorder::default();

    let error = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(
            &space,
            [good.path(), broken.as_path(), also_broken.as_path()],
        )
    })
    .expect_err("有卷点不开，整趟该被拒");

    assert_eq!(recorder.shape(), Vec::<&str>::new(), "被拒的一趟报了事件");
    assert!(!space.out().exists(), "被拒的一趟在输出根下留了东西");

    let said = format!("{error:#}");
    for named in ["坏卷一.cbz", "坏卷二.cbz"] {
        assert!(said.contains(named), "拒绝的那句话里没有 {named}：{said}");
    }
}

/// 管线用的是**预扫列出来的那份成员表**，同一个卷不被枚举两次（会话批 03 号票）。
///
/// 「不扫两次」值多少，见库那一侧 `survey` 的模块文档。这里说的是它**怎么测得出来**：
/// 预扫排在开工那条事件**之前**，因此在收到开工事件的那一刻往源卷里塞一张新页，
/// 管线要是自己再列一遍就会把它一起做掉。
///
/// 用的是**目录卷**。归档卷不另测：`source::open` 在这个 crate 里只剩一个调用点
/// （预扫那一个），会让这一条红的那种改动——把开卷挪回 `process_volume`——两种容器形态
/// 一起红。
#[test]
fn the_pipeline_reuses_what_the_survey_enumerated() {
    /// 收到开工事件就往源卷里塞一张新页。别的事件一概不管。
    struct SlipsInAPage {
        root: PathBuf,
    }

    impl Progress for SlipsInAPage {
        fn observe(&self, event: Event<'_>) -> Instruction {
            if let Event::RunStarted { .. } = event {
                std::fs::copy(self.root.join("001.png"), self.root.join("003.png"))
                    .expect("塞一张新页进源卷");
            }
            Instruction::Continue
        }
    }

    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let root = volume.path().to_path_buf();

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(SlipsInAPage { root: root.clone() })),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert!(root.join("003.png").exists(), "这条用例没塞进去那张页");
    assert_eq!(
        report.volumes[0].page_count(),
        2,
        "开工之后才出现的页被做掉了：这一卷被枚举了两次"
    );
    assert!(
        !report.volumes[0].output.join("003.png").exists(),
        "开工之后才出现的页落到了输出里"
    );
}

/// 预扫**只列成员**：它连页的字节都没读过，更没解过一个像素（会话批 03 号票）。
///
/// 判别式与上一条同一个：预扫排在开工那条事件之前。在收到开工事件的那一刻把一张好页
/// 改写成坏字节——预扫要是读过（或解过）它，手上就是**改写之前**那份好像素，这一页会照常出来；
/// 而它其实一个字节都没读，管线随后才去读，读到的是坏字节，于是这一页失败。
/// **那一页失败正是证据。**
///
/// 这一条与「不扫两次」是一对：那一条问预扫**列**的东西还在不在用，这一条问它有没有
/// 顺手多做一步。两条都测得出来，靠的都是「预扫在开工事件之前」这个时序。
#[test]
fn the_survey_lists_members_without_reading_a_single_pixel() {
    /// 收到开工事件就把点名的那张页改写成坏字节。
    struct BreaksAPage {
        page: PathBuf,
    }

    impl Progress for BreaksAPage {
        fn observe(&self, event: Event<'_>) -> Instruction {
            if let Event::RunStarted { .. } = event {
                std::fs::write(&self.page, b"no longer a png").expect("改写那张页");
            }
            Instruction::Continue
        }
    }

    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let page = volume.path().join("002.png");

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(BreaksAPage { page: page.clone() })),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("一张坏图不该毁掉整卷");

    let failures: Vec<&PathBuf> = report.failures().map(|page| &page.source).collect();
    assert_eq!(failures.len(), 1, "坏页的数目不对：{failures:?}");
    assert!(
        failures[0].ends_with("002.png"),
        "失败的不是被改写的那一页：{failures:?}"
    );
    assert_eq!(report.volumes[0].page_count(), 2, "另一页也跟着没出来");
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

/// 一整卷没做成也**当场**报出去，带着原因——不等这一趟跑完（05 号票）。
///
/// 与「一页失败了」同一条规矩，理由也一样：会话的主区要在出事的当口就说得出口，
/// 而这一趟后面还有几十卷要跑。次序在这里钉住两件事——那一条排在**下一卷开工之前**，
/// 而且那一卷**没有**与它配对的「一卷跑完」：一条开卷之后到得了的只有其中一条。
///
/// 事件带的那两样与报告里那一条是同一份（一份是增量，一份是结果），
/// 与失败页那一对同一个待遇。
#[test]
fn a_volume_that_never_got_done_is_reported_the_moment_it_fails() {
    let space = Workspace::new();
    // 归档结构完好、中央目录列得出这个透传成员，坏的是它的字节——预扫打得开，
    // 真去读才看得出来。那正是「预扫之后才做不成」。
    let mut doomed = space.cbz("volume-a");
    doomed
        .page("001.png", &fixtures::full_bleed_gradient(TINY))
        .rotten_file("ComicInfo.xml", b"<?xml version=\"1.0\"?>");
    let doomed = doomed.write();
    let good = small_volume(&space, "volume-b");
    let recorder = Recorder::default();

    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [doomed.as_path(), good.path()])
    })
    .expect("一卷做不成不该毁掉整趟");

    let failures = recorder.volume_failures();
    assert_eq!(failures.len(), 1, "卷级失败没有报出来：{failures:?}");
    assert_eq!(failures[0].0, doomed, "指错了卷：{failures:?}");
    assert!(failures[0].1.contains("ComicInfo.xml"), "{failures:?}");

    // 同一卷在报告里也有一份，两处指的是同一卷，说的是同一句。
    assert_eq!(
        report
            .failed_volumes
            .iter()
            .map(|failure| (failure.volume.clone(), failure.reason.clone()))
            .collect::<Vec<_>>(),
        failures,
        "事件说的那一卷与报告里的那一卷对不上"
    );

    let shape = recorder.shape();
    let failed_at = shape
        .iter()
        .position(|name| *name == "VolumeFailed")
        .expect("卷级失败那一条");
    let next_volume = shape
        .iter()
        .enumerate()
        .filter(|(_, name)| **name == "VolumeStarted")
        .nth(1)
        .map(|(at, _)| at);
    assert!(
        Some(failed_at) < next_volume,
        "没做成的卷要等下一卷开工之后才说得出口：{shape:?}"
    );
    // 那一卷只有开卷与没做成两条，没有「一卷跑完」——两条出口二选一。
    assert_eq!(
        shape
            .iter()
            .filter(|name| **name == "VolumeFinished")
            .count(),
        1,
        "没做成的卷也报了「一卷跑完」：{shape:?}"
    );
    // 后面那一卷照做，收场那一条照报。
    assert_eq!(report.volumes.len(), 1, "没做成的卷把后面那一卷也带走了");
    assert_eq!(recorder.outcome(), Some(RunOutcome::Completed));
}

/// 收场那一条说得出**这一趟是怎么收的场**，三种各说一种（停车场 Q39、Q46）。
///
/// 中止掉的那一卷既不进报告、也不报「一卷跑完」——它等于没做（ADR 0013 决定第 2 条）。
/// 从前那件事在库的出口上**没有形式**：中止掉的卷与「压根没点名它」在 `Report` 上
/// 一模一样，事件流那一侧也只剩「一条没有配对的开卷」要调用方自己去认。这一条把那个形式
/// 钉住：收场那一条带着它，`Report` 上也带着同一个值。
///
/// 三种一起断言，因为分得开才有意义：走到头的那一趟不许说自己停过，
/// 收尾与中止不许混成同一个字——两者停下来的现场不同（前者盘上留着完整的卷，
/// 后者连当前那一卷都丢了）。
#[test]
fn the_last_event_says_how_the_run_ended() {
    let space = Workspace::new();
    let volumes: Vec<fixtures::Volume> = ["volume-a", "volume-b", "volume-c"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let inputs: Vec<&std::path::Path> = volumes.iter().map(fixtures::Volume::path).collect();

    let run = |recorder: &Recorder, out: PathBuf| {
        tonefit::run(&Request {
            progress: Some(ProgressSink::new(recorder.clone())),
            output_root: out,
            ..fixtures::request(&space, inputs.iter().copied())
        })
        .expect("按停不是失败")
    };

    // 一趟顺当跑下来：点名的卷都走过一遍了。
    let clean = Recorder::default();
    let report = run(&clean, space.out_named("走到头"));
    assert_eq!(clean.outcome(), Some(RunOutcome::Completed));
    assert_eq!(
        report.outcome,
        RunOutcome::Completed,
        "报告与事件说的不是同一件事"
    );

    // 收尾：当前卷跑完就停，剩下的卷没有开工。
    let winding_up = Recorder::stopping_after(1, Instruction::Finish);
    let report = run(&winding_up, space.out_named("收尾"));
    assert_eq!(
        winding_up.outcome(),
        Some(RunOutcome::Stopped(Instruction::Finish))
    );
    assert_eq!(report.outcome, RunOutcome::Stopped(Instruction::Finish));

    // 中止：当前那一卷也丢掉，而它两列报告里都没有——这一项是它唯一的痕迹。
    let aborting = Recorder::stopping_after(1, Instruction::Abort);
    let report = run(&aborting, space.out_named("中止"));
    assert_eq!(
        aborting.outcome(),
        Some(RunOutcome::Stopped(Instruction::Abort))
    );
    assert_eq!(report.outcome, RunOutcome::Stopped(Instruction::Abort));
    assert_eq!(report.volumes.len(), 1, "中止之后还接着做了下一卷");
}

/// **报过开工，就一定报得到收场**——拒绝执行的那一趟也不例外（停车场 Q39）。
///
/// 从前 `run` 返回 `Err` 时收场那一条根本不发，屏上那条横条因此要靠析构去收。
/// 现在那一趟照发，只是收场说的是「拒绝执行」，紧接着 `run` 才把错误交出去：
/// 事件说了「完了」，返回值说了「为什么」，信息一条不少。
///
/// 拿来撞的是互锁 ③：`--dither fs` 撞上一页贴不住面板，处置是**维持拒绝**
/// （页几何批 05 号票）。它是唯一一种在开工**之后**才撞得上的拒绝——
/// 门是页的几何事实，一卷里可能一页都不撞。这一条同时钉住它没有降级成卷级失败：
/// 用户点的 `--dither fs` 对每一卷都错，整趟当场停才对。
#[test]
fn a_refusal_after_the_run_started_still_says_the_run_is_over() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 源两边都比面板小的页在 fit-inside 上按不放大原样输出，一条边都贴不住：门不成立。
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );
    let recorder = Recorder::default();

    let error = tonefit::run(&Request {
        dither: Some(Dither::FloydSteinberg),
        fit: FitMode::Inside,
        progress: Some(ProgressSink::new(recorder.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect_err("几何门不成立时点名抖动应当整趟被拒");

    assert!(format!("{error:#}").contains("几何门"), "{error:#}");
    assert_eq!(
        recorder.outcome(),
        Some(RunOutcome::Refused),
        "拒绝执行的那一趟没有报出收场"
    );
    let shape = recorder.shape();
    assert_eq!(
        shape.last(),
        Some(&"RunFinished"),
        "末一条不是「这一趟完了」：{shape:?}"
    );
    // 拒绝不是卷级失败：这一趟一卷都没「没做成」，它整趟当场停。
    assert!(
        recorder.volume_failures().is_empty(),
        "拒绝执行被记成了卷级失败：{:?}",
        recorder.volume_failures()
    );
}
