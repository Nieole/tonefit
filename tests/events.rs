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
