//! 一趟跑起来之后攒下来的东西：**主区三段各取所需**（`p1-session/09`）。
//!
//! **这个模块一个终端都不碰**，与 [`super::state`] 同一条规矩：它只把事件流折成几个数
//! 加一份报告，画成什么样是 [`super::draw`] 的事。本模块的用例因此连终端库都编译不到。
//!
//! # 事件流就是报告的增量
//!
//! 一卷跑完那条事件带着那一卷的 [`VolumeReport`]（ADR 0011 决定第 2 条），
//! 这里把它接到 [`Live::report`] 上。**攒出来的就是命令行最后一次性拿到的那一份**，
//! 因此报告区画的是 [`crate::render`] 的那几个函数——会话不另写一套措辞。
//!
//! 报告攒到一半也答得出抬头那几件事（`render::header` 吃的是整份报告，不是一个 profile），
//! 「已完成卷的判定、驱动页、失败页当场可见」于是不必等整趟跑完。
//!
//! # 三段各自的来源
//!
//! | 段 | 来源 |
//! |---|---|
//! | 全局条 | `RunStarted` 的 `volumes` 与 `steps`（03 号票的预扫），加 [`Live::walked`] |
//! | 当前卷条 | `VolumeStarted` 的卷名与步数，加 `PassStarted` 的[那一遍](Pass) |
//! | 报告区 | `VolumeFinished` 带的卷报告、`VolumeFailed` 那一句、`PageFailed` 那几条 |
//!
//! 预告的步数是**上界**不是承诺（`CONTEXT.md` 的《进度》）。拿它画全局进度的实现方
//! 因此要在一卷跑完时**结清**那一卷预告剩下的步——这是 [`tonefit::Event::RunStarted`]
//! 对实现方的要求，命令行那一份见 `crate::Bar::finish_volume`，本模块那一份见
//! [`Live::finish_volume`]。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tonefit::{
    Event, Mode as RunMode, Pass, Report, Request, RunOutcome, VolumeFailure, VolumeReport,
};

/// 当前卷那一条：它叫什么、预告多少步、走了几步、在走哪一遍。
#[derive(Debug, Clone)]
pub struct Walking {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 这一卷这一趟最多走多少步。**上界**，不是承诺。
    pub steps: u64,
    /// 已经走过的步数。
    pub walked: u64,
    /// 在走哪一遍。开卷之后、第一条 `PassStarted` 到达之前是 `None`。
    pub pass: Option<Pass>,
}

/// 一趟跑起来之后攒下来的东西。**一趟一份**：按下试算或执行时新造一个，
/// 跑完仍留着——退出会话时印到 stdout 的就是它这一份报告。
#[derive(Debug, Clone)]
pub struct Live {
    /// 这一趟做到哪一步：试算就是 `DryRun`，执行就是 `Process`
    /// （`CONTEXT.md` 的《会话》：试算就是会话里按下去的那一次 dry-run）。
    /// 报告抬头照它印，因此得留着。
    mode: RunMode,
    /// 攒到此刻的报告。开工那一刻它是「零卷的一份」——抬头那几件事已经答得出。
    report: Report,
    /// 这一趟点名了几个卷（`RunStarted`）。
    volumes: usize,
    /// 这一趟最多走多少步（`RunStarted`，各卷之和）。
    steps: u64,
    /// 全局走过的步数，含各卷收摊时结清的那一截。
    walked: u64,
    /// 已经收摊的卷数（跑完的与没做成的都算）。全局条那个「第几卷」用它。
    finished: usize,
    /// 开工那一刻。剩余时间由它与 [`walked`](Self::walked) 算出。
    started: Instant,
    /// 当前卷。卷与卷之间是 `None`。
    volume: Option<Walking>,
    /// **出现的当场**就说得出口的失败页（`PageFailed`），按出现次序。
    ///
    /// 它不等那一卷跑完：报告区默认只给卷级（逐页展开归 `p1-session/11`），
    /// 而卷级那几行只说得出「几页失败」，说不出**为什么**。同一份原因随后也会在
    /// 那一卷报告的 `PageOutcome::Failed` 里出现一次——那一份是结果，这几条是增量。
    failed_pages: Vec<(PathBuf, String)>,
    /// 那条线程回来了没有。
    ///
    /// **这一趟收成了什么样不在这里**——那是[收场](RunOutcome)，在报告上
    /// （`Report::outcome`），会话不另立一个同义的词。这一格只答「还跑着吗」。
    ended: bool,
    /// 这一趟**没做成**时那句话（`CONTEXT.md` 的《失败》：退出码 `1` 那一种）。
    ///
    /// 两种都落在这里：**拒绝执行**（错在这一趟的参数上，`run` 返回的是错误本身），
    /// 以及那条线程**恐慌**了。两者都没有报告可带，因此 stdout 上一个字节都没有，
    /// 退出码都是命令行那一路的 `1`——分得开它们的是这句话本身。
    ///
    /// 会话不因此退出：把这句话画出来，用户当场改（spec 的《卷级失败与退出码》）。
    undone: Option<String>,
}

impl Live {
    /// 开一趟：抬头那几件事从 [`Request`] 上就答得出，因此报告当场就有一份。
    pub fn new(request: &Request) -> Self {
        Self {
            mode: request.mode,
            report: Report {
                profile: request.profile.clone(),
                fit: request.fit,
                crop: request.crop,
                split: request.split,
                volumes: Vec::new(),
                failed_volumes: Vec::new(),
                outcome: RunOutcome::Completed,
                // 计时只进结构、不进渲染出的文字（见 `tonefit::Report::elapsed`），
                // 攒到一半的这一份因此填零就够——跑完会换成库交出来的那一份。
                elapsed: Duration::ZERO,
            },
            volumes: 0,
            steps: 0,
            walked: 0,
            finished: 0,
            started: Instant::now(),
            volume: None,
            failed_pages: Vec::new(),
            ended: false,
            undone: None,
        }
    }

    /// 收下一条事件，折进上面那几格。
    ///
    /// 它只是一张**对照表**：每一支立刻转给下面那几个方法，而那几个才是真正的状态转移。
    /// 分开是因为 [`Event`] 的变体两级非穷尽（ADR 0011），**库外造不出任何一条**——
    /// 用例因此只问得动那几个方法，这一层的对照表反倒是最不容易写错的一段。
    ///
    /// `_` 那一支不是遗漏：多一个变体不该逼着这里跟着改（ADR 0011 的《后果》）。
    pub fn observe(&mut self, event: &Event<'_>) {
        match event {
            Event::RunStarted { volumes, steps, .. } => self.run_started(*volumes, *steps),
            Event::VolumeStarted { volume, steps, .. } => self.volume_started(volume, *steps),
            Event::PassStarted { pass, .. } => self.pass_started(*pass),
            Event::Stepped { .. } => self.stepped(),
            Event::PageFailed { page, reason, .. } => self.page_failed(page, reason),
            Event::VolumeFinished { report, .. } => self.volume_finished(report),
            Event::VolumeFailed { volume, reason, .. } => self.volume_failed(volume, reason),
            Event::RunFinished { outcome, .. } => self.run_finished(*outcome),
            _ => {}
        }
    }

    /// 预扫完了，开工：全局条那两个数就是 `RunStarted` 报的这两个（03 号票）。
    pub fn run_started(&mut self, volumes: usize, steps: u64) {
        self.volumes = volumes;
        self.steps = steps;
        // 表从这里开始掐：开工之前那一段是预扫与那几道检查，剩余时间算不进去。
        self.started = Instant::now();
    }

    /// 开一卷。
    pub fn volume_started(&mut self, volume: &Path, steps: u64) {
        self.volume = Some(Walking {
            volume: volume.to_path_buf(),
            steps,
            walked: 0,
            pass: None,
        });
    }

    /// 当前卷开始走某一遍。「进度条现在在走哪一遍」只有它答得出来。
    pub fn pass_started(&mut self, pass: Pass) {
        if let Some(walking) = &mut self.volume {
            walking.pass = Some(pass);
        }
    }

    /// 又走完一步：当前卷那一条与全局那一条各进一格。
    pub fn stepped(&mut self) {
        self.walked = self.walked.saturating_add(1);
        if let Some(walking) = &mut self.volume {
            walking.walked = walking.walked.saturating_add(1);
        }
    }

    /// 一页失败了，**当场**记下来（连同原因）。
    pub fn page_failed(&mut self, page: &Path, reason: &str) {
        self.failed_pages
            .push((page.to_path_buf(), reason.to_owned()));
    }

    /// 一卷跑完了，把那一卷的报告接到攒着的这一份上。
    pub fn volume_finished(&mut self, report: &VolumeReport) {
        self.report.volumes.push(report.clone());
        self.finish_volume();
    }

    /// 一整卷没做成：记一笔原因，其余卷照做。
    pub fn volume_failed(&mut self, volume: &Path, reason: &str) {
        self.report.failed_volumes.push(VolumeFailure {
            volume: volume.to_path_buf(),
            reason: reason.to_owned(),
        });
        self.finish_volume();
    }

    /// 这一趟完了，带着它是怎么收的场。
    pub fn run_finished(&mut self, outcome: RunOutcome) {
        self.report.outcome = outcome;
        self.volume = None;
    }

    /// 一卷收摊：抹掉当前卷那一条，并把它**预告了却没走**的那几步结清到全局那一条上。
    ///
    /// 为什么非结清不可，见 [`tonefit::Event::RunStarted`] 的 `steps`：预告的是上界，
    /// 幂等命中的卷提前收摊——不结清，全局条就永远走不到头。
    fn finish_volume(&mut self) {
        self.finished += 1;
        if let Some(walking) = self.volume.take() {
            self.walked = self
                .walked
                .saturating_add(walking.steps.saturating_sub(walking.walked));
        }
    }

    /// 那条线程回来了：把攒出来的报告换成库交出来的那一份，或者记下这一趟没做成。
    ///
    /// 换而不是接着用攒的那一份：两者的差别只有 [`Report::elapsed`]，
    /// 而「这一趟做了多久」只有库那一侧减得掉在决策点上等人的那几分钟（停车场 Q41）。
    pub fn returned(&mut self, done: anyhow::Result<Report>) {
        self.volume = None;
        self.ended = true;
        match done {
            Ok(report) => self.report = report,
            // 没做成那一趟没有报告：攒到一半的那一份留着，它说得出已经做完的卷。
            Err(error) => self.undone = Some(format!("{error:#}")),
        }
    }

    /// 那条线程回来了没有。
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// 这一趟**没做成**时那句话，做成了就是 `None`。
    pub fn undone(&self) -> Option<&str> {
        self.undone.as_deref()
    }

    /// 这一趟做到哪一步。
    pub fn mode(&self) -> RunMode {
        self.mode
    }

    /// 攒到此刻的报告。
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// 全局条那几个数：第几卷 / 共几卷、走了几步 / 共几步、已用多久、还剩多久。
    ///
    /// **收场之后「已用」就定住了**：那时用的是库交出来的 [`Report::elapsed`]——
    /// 它是这一趟真做了多久，扣掉了在决策点上等人的那几分钟（停车场 Q41）。
    /// 接着读自己那块表的话，跑完坐着不动，屏上那个数会一路往上涨。
    pub fn overall(&self) -> Overall {
        let elapsed = if self.ended {
            self.report.elapsed
        } else {
            self.started.elapsed()
        };
        Overall {
            volume: self
                .finished
                .saturating_add(usize::from(self.volume.is_some())),
            volumes: self.volumes,
            walked: self.walked,
            steps: self.steps,
            elapsed,
            // 完了就没有「还剩多久」可说。
            left: (!self.ended)
                .then(|| eta(elapsed, self.walked, self.steps))
                .flatten(),
        }
    }

    /// 当前卷那一条。卷与卷之间没有。
    pub fn walking(&self) -> Option<&Walking> {
        self.volume.as_ref()
    }

    /// 把开工那一刻往回拨一段。**只给用例用**：屏幕快照里有「已用」与「剩」两个数，
    /// 不拨回去它们就随机器快慢而变，快照因此每跑一次都不一样
    /// （与黄金快照同一条规矩，见 `tonefit::Report::elapsed`）。
    #[cfg(test)]
    pub fn rewind(&mut self, by: Duration) {
        self.started = self.started.checked_sub(by).unwrap_or(self.started);
    }

    /// **出现的当场**收下的那些失败页。
    pub fn failed_pages(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.failed_pages
            .iter()
            .map(|(page, reason)| (page.as_path(), reason.as_str()))
    }

    /// 这一趟的退出码，**与命令行那一路同一套**：拒绝执行是 `1`，
    /// 其余交给 [`crate::exit_code`]——全部成功 `0`、有卷被隔离 `2`、有卷没做成 `3`。
    ///
    /// 还没跑完时问它没有意义，那时给的是「照现在这份报告收场会是几」——
    /// 会话只在退出那一刻问一次，而那时这一趟一定已经收了场。
    pub fn exit_code(&self) -> u8 {
        match self.undone {
            Some(_) => crate::REFUSED_EXIT,
            None => crate::exit_code(&self.report),
        }
    }
}

/// 全局条那一行要的几个数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overall {
    /// 走到第几卷（含正在走的那一个）。
    pub volume: usize,
    /// 这一趟点名了几个卷。
    pub volumes: usize,
    /// 走过的步数。
    pub walked: u64,
    /// 预告的总步数。**上界**，不是承诺。
    pub steps: u64,
    /// 开工到此刻。
    pub elapsed: Duration,
    /// 还剩多久。步数还没走出第一步时答不出来。
    pub left: Option<Duration>,
}

/// 剩余时间：按**至今为止的平均步速**外推。
///
/// 一步都还没走时答不出来（除以零），预告的步数是零时同理。走完了就是零。
/// 它与预告的步数同一个性质——**上界外推出来的估计**，不是承诺：
/// 幂等命中的卷提前收摊，剩下的那一截会突然缩短。
fn eta(elapsed: Duration, walked: u64, steps: u64) -> Option<Duration> {
    let left = steps.checked_sub(walked)?;
    if walked == 0 {
        return None;
    }
    // 先乘后除：剩余步数与已用纳秒都可能很大，`Duration::mul_f64` 那一条路
    // 在长任务上会把秒以下的位数丢光。
    let per_step = elapsed.as_nanos() / u128::from(walked);
    let nanos = per_step.saturating_mul(u128::from(left));
    Some(Duration::from_nanos(
        u64::try_from(nanos).unwrap_or(u64::MAX),
    ))
}

#[cfg(test)]
pub(crate) mod fixture {
    //! 报告的夹具。`draw` 那一侧的快照用例与本模块的用例共用它——
    //! 两边要的是同一份东西，各搓一份就会在改动时走散。

    use std::path::PathBuf;

    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, CandidateScore, ChosenBy, Crop, Dither,
        Envelope, GeometryGate, GrayImage, IoPlan, Medium, Mode as RunMode, PageBranch, PageColor,
        PageOutcome, PageReport, Processed, Profile, Reason, Reference, Request, Scaling, Size,
        Verdict, VolumeReport, VolumeTiming, VolumeVerdict,
    };

    /// 这一趟的参数。抬头那几行照它印。
    pub fn request(mode: RunMode) -> Request {
        Request {
            inputs: vec![PathBuf::from("库/卷一")],
            output_root: PathBuf::from("出/"),
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: tonefit::FitMode::default(),
            crop: true,
            split: tonefit::SplitRule::default(),
            filter: tonefit::Filter::default(),
            bit_depth: None,
            dither: None,
            per_page: false,
            cache_budget: CacheBudget::default(),
            mode,
            io_mode: tonefit::IoMode::default(),
            metadata: true,
            progress: None,
        }
    }

    /// 一份**幂等命中**的卷报告：一页都没重做，逐页结果因此一条都没有。
    ///
    /// 快照要的正是这一种——它不必搓判据、候选与几何门，而「跳过说清是哪四项依据没变」
    /// 与「这一趟怎么读的」两条验收都落在它身上。
    pub fn skipped_volume(name: &str, page_count: usize) -> VolumeReport {
        VolumeReport {
            volume: PathBuf::from(format!("库/{name}")),
            output: PathBuf::from(format!("出/{name}")),
            superseded: None,
            pages: Vec::new(),
            source_pages: page_count,
            verdict: Some(VolumeVerdict::Skipped { page_count }),
            cache: cache_usage(),
            io: io_plan(),
            decodes: 0,
            timing: VolumeTiming::default(),
        }
    }

    /// 一份**真做过事**的卷报告：一页完好的灰度页定出卷级基准档。
    ///
    /// `broken` 给一句原因就再添一张失败页，那时整卷进隔离目录。
    /// 「一卷跑完当场显示它的判定与驱动页」与「失败页带原因」两条验收落在它身上——
    /// 驱动页指的就是那一页完好的。
    pub fn processed_volume(name: &str, broken: Option<&str>) -> VolumeReport {
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let source = Size::new(1441, 2048);
        let target = Size::new(1182, 1680);
        let mut pages = vec![PageReport {
            source: PathBuf::from(format!("库/{name}/001.jpg")),
            output: PathBuf::from(format!("出/{name}/001.png")),
            size: target,
            outcome: PageOutcome::Whole(Processed {
                crop: Crop::keeping_all(source),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(source, target),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    scores: vec![CandidateScore {
                        candidate,
                        score: a_score(),
                    }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::LowestWithinThreshold,
                    },
                },
            }),
        }];
        if let Some(reason) = broken {
            pages.push(PageReport {
                source: PathBuf::from(format!("库/{name}/017.jpg")),
                output: PathBuf::from(format!("出/隔离/{name}/017.png")),
                size: target,
                outcome: PageOutcome::Failed {
                    reason: reason.to_owned(),
                },
            });
        }
        let out = if broken.is_some() {
            format!("出/隔离/{name}")
        } else {
            format!("出/{name}")
        };
        VolumeReport {
            volume: PathBuf::from(format!("库/{name}")),
            output: PathBuf::from(out),
            superseded: None,
            source_pages: pages.len(),
            verdict: Some(VolumeVerdict::Envelope(Envelope {
                base: candidate,
                driver: 0,
                body_pages: 1,
                outlier_pages: 0,
                raised_pages: 0,
            })),
            pages,
            cache: cache_usage(),
            io: io_plan(),
            decodes: 1,
            timing: VolumeTiming::default(),
        }
    }

    /// 一个判据值。从公开 seam 上真算一个——摆一个编出来的数上去，
    /// 快照就钉不住「报告说的是判据算出来的东西」。
    fn a_score() -> tonefit::Score {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        tonefit::score(&reference, &GrayImage::new(Size::new(1, 1), vec![136]))
    }

    /// 一份读取计划：探到固态盘、并发读八条。「这一趟怎么读的」那一行印的就是它。
    fn io_plan() -> IoPlan {
        IoPlan {
            medium: Medium::Solid,
            readers: 8,
            chosen_by: ChosenBy::Probe,
        }
    }

    fn cache_usage() -> CacheUsage {
        CacheUsage {
            budget: CacheBudget::default(),
            pages: 1,
            raw: 4 * 1024 * 1024,
            stored: 1024 * 1024,
            resident: 1024 * 1024,
            spilled: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonefit::{Instruction, Mode as RunMode};

    /// 一趟走完：全局那几个数、当前卷那一条、报告区那一份，逐条对得上。
    #[test]
    fn the_event_stream_adds_up_to_the_report_and_the_two_bars() {
        let request = fixture::request(RunMode::Process);
        let mut live = Live::new(&request);

        live.run_started(2, 10);
        assert_eq!(live.overall().volumes, 2);
        assert_eq!(live.overall().steps, 10);
        // 一步都没走：剩多久答不出来，编一个数出来是骗人。
        assert_eq!(live.overall().left, None);

        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::First);
        live.stepped();
        let walking = live.walking().expect("有一卷在走");
        assert_eq!(walking.pass, Some(Pass::First));
        assert_eq!(walking.walked, 1);
        assert_eq!(live.overall().volume, 1, "走到第几卷");

        // 一卷跑完：报告接上一条，预告剩下的五步当场结清到全局那一条上。
        let finished = fixture::skipped_volume("卷一", 20);
        live.volume_finished(&finished);
        assert_eq!(live.report().volumes.len(), 1);
        assert_eq!(live.overall().walked, 6, "预告了却没走的那几步没结清");
        assert!(live.walking().is_none(), "卷与卷之间不该还有一条");

        // 一卷没做成：同样收摊、同样结清，原因进报告。
        live.volume_started(Path::new("库/卷二"), 4);
        live.volume_failed(Path::new("库/卷二"), "盘拔了");
        assert_eq!(live.overall().walked, 10, "全局条走不到头");
        assert_eq!(live.report().failed_volumes.len(), 1);

        live.run_finished(RunOutcome::Completed);
        assert_eq!(live.report().outcome, RunOutcome::Completed);
    }

    /// 失败页在**出现的当场**就收得下，带着原因，不必等那一卷跑完。
    #[test]
    fn a_failed_page_is_visible_the_moment_it_happens() {
        let mut live = Live::new(&fixture::request(RunMode::Process));
        live.run_started(1, 4);
        live.volume_started(Path::new("库/卷一"), 4);
        live.page_failed(
            Path::new("库/卷一/003.jpg"),
            "读不出这一页的字节：文件被删了",
        );

        let seen: Vec<(&Path, &str)> = live.failed_pages().collect();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, Path::new("库/卷一/003.jpg"));
        assert!(seen[0].1.contains("文件被删了"));
        // 那一卷还没跑完，报告里因此还没有它——「当场」说的正是这一段时间差。
        assert!(live.report().volumes.is_empty());
    }

    /// 退出码与命令行那一路一致：拒绝执行 `1`，有卷被隔离 `2`，全部成功 `0`。
    #[test]
    fn the_exit_code_is_the_one_the_command_line_would_have_given() {
        let request = fixture::request(RunMode::Process);

        let mut refused = Live::new(&request);
        refused.returned(Err(anyhow::anyhow!("处理范围为空")));
        assert_eq!(refused.exit_code(), crate::REFUSED_EXIT);
        assert!(refused.ended(), "那条线程回来了");
        assert!(
            refused.undone().expect("没做成").contains("处理范围为空"),
            "没做成的那一趟要说得出为什么"
        );

        let mut isolated = Live::new(&request);
        let mut report = isolated.report().clone();
        report
            .volumes
            .push(fixture::processed_volume("卷一", Some("解不出来")));
        isolated.returned(Ok(report));
        assert_eq!(isolated.exit_code(), crate::ISOLATED_EXIT);

        let mut clean = Live::new(&request);
        let mut report = clean.report().clone();
        report.volumes.push(fixture::skipped_volume("卷一", 20));
        clean.returned(Ok(report));
        assert_eq!(clean.exit_code(), crate::SUCCESS_EXIT);
    }

    /// 跑完之后「已用」就定住了：那个数是库交出来的，不是会话接着读自己那块表。
    #[test]
    fn the_elapsed_time_stops_moving_once_the_run_is_over() {
        let mut live = Live::new(&fixture::request(RunMode::Process));
        live.run_started(1, 10);
        let mut report = live.report().clone();
        report.elapsed = Duration::from_secs(42);
        live.returned(Ok(report));

        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
        assert_eq!(live.overall().left, None, "完了就没有「还剩多久」可说");
        // 再问一次仍是同一个数——会话没有接着读自己那块表。
        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
    }

    /// 按停停下来的那一趟退出码照旧：它是用户自己的决定，不是失败。
    #[test]
    fn a_run_that_was_stopped_still_exits_zero() {
        let mut live = Live::new(&fixture::request(RunMode::Process));
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Stopped(Instruction::Abort);
        live.returned(Ok(report));

        assert_eq!(live.exit_code(), crate::SUCCESS_EXIT);
    }

    /// 剩余时间按至今为止的平均步速外推；走完就是零，一步没走就答不出来。
    #[test]
    fn the_time_left_is_extrapolated_from_the_pace_so_far() {
        assert_eq!(eta(Duration::from_secs(10), 0, 100), None);
        assert_eq!(
            eta(Duration::from_secs(10), 10, 100),
            Some(Duration::from_secs(90))
        );
        assert_eq!(eta(Duration::from_secs(10), 100, 100), Some(Duration::ZERO));
        // 走过的步数超过预告（预告是上界，理应不会，但它是个 `u64` 减法）：不绕回去。
        assert_eq!(eta(Duration::from_secs(10), 120, 100), None);
    }
}
