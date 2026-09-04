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
    Event, Instruction, Mode as RunMode, Pass, Report, Request, RunOutcome, VolumeFailure,
    VolumeReport,
};

/// 这一趟**在决策点上等不等人**（`CONTEXT.md` 的《会话》：续做、等答话）。
///
/// 一个枚举而不是一个 `bool`：它从 [`super::resuming`] 一路传到 [`Live::new`] 与
/// [`super::run::Running::start`]，而调用处一个裸 `false` 说不出它否掉的是哪件事
/// （与 `super::draw` 那个 `Unrolled` 同一条理由——本仓库不爱看不出意思的裸值）。
///
/// 判它的是 [`super::resuming`]，依据是 ADR 0012 决定第 1、3 条：续做只在单卷试算上成立，
/// 而等不等人是调用方的策略、不是库的行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resuming {
    /// **续做**：跑到决策点停下来等人拿主意（单卷试算）。
    Waits,
    /// **不续做**：决策点上不等人，一趟走到底（多卷试算与执行）。
    GoesOn,
}

impl Resuming {
    /// 这一趟会停下来等人吗。
    fn waits(self) -> bool {
        self == Self::Waits
    }
}

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
    /// 库那一侧真收到的那个 mode。**屏上照哪一种印走 [`mode`](Self::mode)**，
    /// 不直接读它：单卷试算走的是 `Mode::Process`（参照要留着，ADR 0012 决定第 5 条），
    /// 而在决策点上答收尾之前它一个字节都没写。
    ran_as: RunMode,
    /// 这一趟**在决策点上等人**吗（`CONTEXT.md` 的《会话》：续做）。
    ///
    /// 只有单卷试算是。多卷试算另走一次 dry-run，执行一趟走到底——两者在决策点上都不停。
    /// 起手那一刻就定死（[`super::press`] 拼 `Request` 时判的），跑起来之后不再变。
    resumes: Resuming,
    /// 在决策点上答过的那个字。还没答、或者这一趟根本不在那儿停就是 `None`。
    ///
    /// 屏上那句话与报告抬头都要它：**试算答了收尾，这一趟就等于一次 dry-run**
    /// （见 [`mode`](Self::mode)）。
    decided: Option<Instruction>,
    /// 决策点上那一卷**到此刻为止**的报告（`PassStarted` 的 `so_far`，停车场 Q52）。
    ///
    /// 它不进 [`report`](Self::report)：那一份装的是**收摊了的卷**，而这一卷还停在决策点上，
    /// 第二遍一步没走。报告区把它接在那几卷后面画出来（见 `super::draw::report_text`），
    /// 「主区把报告画出来等你拿主意」靠的就是它。一卷收摊时清掉——那时正式的一份在报告里了。
    summarized: Option<VolumeReport>,
    /// 这一趟在决策点上**等人等掉的那一截**，累计（停车场 Q41，`CONTEXT.md` 的《会话》：
    /// 决策点上等人的那段时间不算进计时）。
    ///
    /// 库那一侧自己也减掉它（`Report::elapsed`、`VolumeTiming::elapsed`），但那一份要等
    /// 这一趟收场才交得出来。会话这一头**边跑边画**，因此自己也得记一份：不记的话，
    /// 屏上那两个数会在人看着报告的那几分钟里一路往上涨，而那几分钟里库一步都没走——
    /// 「剩 2h13m」说的就成了「用户拿主意还要多久」。
    deliberated: Duration,
    /// 这一次等是从什么时候起的。没在等就是 `None`。
    ///
    /// 只有**续做那一趟**记它：别的趟在决策点上不等人（观察者当场答字就返回），
    /// 那一格开了就再也关不上。
    deliberating_since: Option<Instant>,
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
    pub fn new(request: &Request, resumes: Resuming) -> Self {
        Self {
            ran_as: request.mode,
            resumes,
            decided: None,
            summarized: None,
            deliberated: Duration::ZERO,
            deliberating_since: None,
            report: Report {
                profile: request.profile.clone(),
                fit: request.fit,
                crop: request.crop,
                split: request.split,
                volumes: Vec::new(),
                failed_volumes: Vec::new(),
                // 非卷文件整份在预扫走完就齐了，而事件流不报它——攒到一半的这一份因此
                // 恒是空的，跑完换成库交出来的那一份（见 [`returned`](Self::returned)）。
                // 末尾那几小结本来也只在收场之后画（见 `crate::session::draw`）。
                non_volume_files: Vec::new(),
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
            Event::PassStarted { pass, so_far, .. } => self.pass_started(*pass, *so_far),
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
    ///
    /// **决策点那一条还带着这一卷到此刻为止的报告**（`so_far`，停车场 Q52）：收下它，
    /// 报告区就画得出「拿什么主意」。另外两遍那一格是 `None`，这里因此不动它。
    pub fn pass_started(&mut self, pass: Pass, so_far: Option<&VolumeReport>) {
        if let Some(walking) = &mut self.volume {
            walking.pass = Some(pass);
        }
        if let Some(so_far) = so_far {
            self.summarized = Some(so_far.clone());
            // 决策点那一条报出来的下一刻，观察者就停在闸上了（见 `super::run::Watch`）。
            // 等人那一截从这里起算——但只有真等人的那一趟（见 [`Self::deliberating_since`]）。
            if self.resumes.waits() {
                self.deliberating_since = Some(Instant::now());
            }
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

    /// 这一卷收摊了：决策点上摆着的那份「到此刻为止」作废。
    ///
    /// 一卷跑完那一条带的是同一卷正式的一份（报告里已经有了），一卷没做成那一条说的是
    /// 它连报告都没有——两种情形下再画那一份都是在画一件已经不成立的事。
    fn summary_is_stale(&mut self) {
        self.summarized = None;
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
        // 停在决策点上被中止的那一趟从这里出去：那一等到此为止，没有人会来答它。
        self.stop_deliberating();
        // 停在决策点上被中止的那一卷不报「一卷跑完」（`Event::VolumeFinished` 的文档）：
        // 它那份「到此刻为止」到这一刻为止也就作废了，而没有别人会来清它。
        self.summary_is_stale();
    }

    /// 一卷收摊：抹掉当前卷那一条，并把它**预告了却没走**的那几步结清到全局那一条上。
    ///
    /// 为什么非结清不可，见 [`tonefit::Event::RunStarted`] 的 `steps`：预告的是上界，
    /// 幂等命中的卷提前收摊——不结清，全局条就永远走不到头。
    fn finish_volume(&mut self) {
        self.summary_is_stale();
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

    /// 报告抬头照哪一种印。
    ///
    /// **单卷试算在答继续之前印的是 dry-run**，虽然它走的是 `Mode::Process`：
    /// 那条路留参照是为了答继续时第一遍不重算（ADR 0012 决定第 5 条），
    /// 而在决策点上答出继续之前，输出根一个字节都没有——抬头那一行
    /// 「dry-run：只算不写，下面的路径都还没落盘」正是这时要说的话。
    /// 答了收尾或中止同理：那一趟就此收场，盘上仍旧什么都没有。
    ///
    /// 别的三种（多卷试算、执行、一趟都没跑过）照库收到的那个字印，这一格与从前逐字相同。
    pub fn mode(&self) -> RunMode {
        match (self.resumes, self.decided) {
            // 中止那一支眼下到不了：会话在决策点上只答得出继续与收尾，
            // 中止走的是「退出会话」那条路（`Running::leave` 直接推闩，不记这一格）。
            // 仍旧写开，因为 `Instruction` 不非穷尽——多一级的那一天这里编译不过；
            // 而真到了也是同一个答案：那一卷等于没做，盘上一个字节都没有。
            (Resuming::Waits, None | Some(Instruction::Finish | Instruction::Abort)) => {
                RunMode::DryRun
            }
            _ => self.ran_as,
        }
    }

    /// 这一趟在决策点上等人吗（`CONTEXT.md` 的《会话》：续做）。
    pub fn resumes(&self) -> bool {
        self.resumes.waits()
    }

    /// 决策点上答过的那个字。还没答、或者这一趟不在那儿停就是 `None`。
    ///
    /// **只给用例用**——屏上要的是它的**后件**（报告抬头照哪一种印，见
    /// [`mode`](Self::mode)），而那一件由那个函数一处答完。
    #[cfg(test)]
    pub fn decided(&self) -> Option<Instruction> {
        self.decided
    }

    /// 记下决策点上答的那个字。**由会话那一头记**（[`super::run::Running::decide`]）：
    /// 答话的是用户，而观察者那一侧只是把它转交给库。
    pub fn decide(&mut self, said: Instruction) {
        self.decided = Some(said);
        self.stop_deliberating();
    }

    /// 等人那一截收口，累进 [`deliberated`](Self::deliberated)。没在等就什么都不做。
    ///
    /// 两条出路：用户答了话（那条线程接着跑），或者这一趟就此收场
    /// （停在决策点上被中止的那一趟走的是这一条，没有人会来答它）。
    fn stop_deliberating(&mut self) {
        if let Some(since) = self.deliberating_since.take() {
            self.deliberated = self.deliberated.saturating_add(since.elapsed());
        }
    }

    /// 至今为止等人等掉的那一截，**含正等着的这一次**。
    ///
    /// **`now` 由调用方给**，本函数一次表都不读：减掉这一截的那一处
    /// （[`overall`](Self::overall)）要拿同一个时刻算两个数，各读各的表就会
    /// 让减出来的那个数小一格——两次读表之间被调度器抢走多久，就少多久
    /// （停车场 Q118 实测到 299.9999981s < 300s）。
    fn deliberated(&self, now: Instant) -> Duration {
        let waiting = self
            .deliberating_since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        self.deliberated.saturating_add(waiting)
    }

    /// 决策点上那一卷到此刻为止的报告。没停在决策点上就是 `None`。
    pub fn summarized(&self) -> Option<&VolumeReport> {
        self.summarized.as_ref()
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
            // 减掉在决策点上等人的那一截（停车场 Q41）：库在那段里一步都没走，
            // 算进来的话「剩多久」说的就成了「用户拿主意还要多久」。
            // 收场之后换成库交出来的那一个——它减的是同一件事，只是准到纳秒。
            //
            // **一次读表算两个数**：被减数与减数都从这一个 `now` 起算。各读各的表时，
            // 减数读得晚一点、因而多算一点，减出来的那个数就小一格（停车场 Q118）。
            let now = Instant::now();
            now.saturating_duration_since(self.started)
                .saturating_sub(self.deliberated(now))
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
    //!
    //! [`a_real_volume`] 是里面唯一**落到盘上**的一个：真起一条线程跑一趟的那几条用例
    //! （`super::super::run`、`super::super::terminal`）共用它。

    use std::path::{Path, PathBuf};

    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, CandidateScore, ChosenBy, Crop, Dither,
        Envelope, GeometryGate, GrayImage, IoPlan, Medium, Mode as RunMode, PageBranch, PageColor,
        PageOutcome, PageReport, Processed, Profile, Readers, Reason, Reference, Request, Scaling,
        Size, Verdict, VolumeReport, VolumeTiming, VolumeVerdict,
    };

    /// 在 `root` 底下摆一个叫 `name` 的、真跑得动的卷：**一页加一个透传文件**。
    /// 建出目录，返回卷根。
    ///
    /// 页非有不可：一页都没有的东西不是卷（ADR 0014 决定第 3 条），预扫当场把它丢掉，
    /// 那条线程于是根本走不到决策点。从前这几条用例只摆一个透传文件，为的是不必造图片。
    ///
    /// 透传文件仍留着：第二遍写的是**全部成员**，它也在里面
    /// （`tests/resume.rs` 的 `small_volume` 特意留一个透传成员正是这个理由），
    /// 因此「写没写出去」在它身上看得见。
    ///
    /// 页是这批用例里最便宜的一张：高恰是基准面板的高，纯墨到边——两样都是为了让管线
    /// 在它身上不做工作（不缩放、裁边一个像素都拿不走）。这几条问的是线程、闩与决策点，
    /// 页上画着什么一概不影响。
    pub fn a_real_volume(root: &Path, name: &str) -> PathBuf {
        /// 基准面板的高（`kobo-libra-2`，见 [`request`]）。
        const PANEL_HEIGHT: u32 = 1680;

        let volume = root.join(name);
        std::fs::create_dir_all(&volume).expect("建得出卷");
        let page = image::DynamicImage::ImageLuma8(image::ImageBuffer::from_pixel(
            64,
            PANEL_HEIGHT,
            image::Luma([85u8]),
        ));
        let mut bytes = std::io::Cursor::new(Vec::new());
        page.write_to(&mut bytes, image::ImageFormat::Png)
            .expect("编得出一页 PNG");
        std::fs::write(volume.join("001.png"), bytes.into_inner()).expect("写得出页");
        std::fs::write(volume.join("说明.txt"), "透传").expect("写得出成员");
        volume
    }

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

    /// 一份**三种页各一张**的卷报告：完好的灰度页、走彩色分支的页、失败页。
    ///
    /// 展开那几条要的正是这一种（`p1-session/11` 的验收后两条）：
    /// 失败页说得出它的尺寸是**卷内统一尺寸**、彩页说得出它不量化也不进上包络，
    /// 而这两句话只有逐页那几行说得出来——卷级那几行一句都没有。
    ///
    /// 它与 [`processed_volume`] 分开而不是给后者加一个开关：那两张快照
    /// （`p1-session/09` 录的）钉的是卷级那几行，添一张彩页会让它们一起重录，
    /// 而彩页与那两张快照要说的事无关。
    pub fn three_kinds_of_page(name: &str) -> VolumeReport {
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let source = Size::new(1441, 2048);
        let target = Size::new(1182, 1680);
        let whole = |at: &str, color: PageColor, branch: PageBranch| PageReport {
            source: PathBuf::from(format!("库/{name}/{at}.jpg")),
            output: PathBuf::from(format!("出/隔离/{name}/{at}.png")),
            size: target,
            outcome: PageOutcome::Whole(Processed {
                crop: Crop::keeping_all(source),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(source, target),
                color,
                branch,
            }),
        };
        let pages = vec![
            whole(
                "001",
                PageColor::Gray,
                PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    // 四个候选各一个数——**逐页那两行轻松过 100 列**（票面原话）就是
                    // 这么来的。摆一个候选的话那一行短得放得进 60 列，
                    // 而「宽度是稀缺资源」这件事就演不出来了。
                    scores: every_candidate(),
                    verdict: Verdict {
                        candidate,
                        reason: Reason::LowestWithinThreshold,
                    },
                },
            ),
            whole("002", PageColor::Color, PageBranch::Color),
            PageReport {
                source: PathBuf::from(format!("库/{name}/017.jpg")),
                output: PathBuf::from(format!("出/隔离/{name}/017.png")),
                size: target,
                outcome: PageOutcome::Failed {
                    reason: "解不出完整尺寸：JPEG 数据截断".to_owned(),
                },
            },
        ];
        VolumeReport {
            volume: PathBuf::from(format!("库/{name}")),
            output: PathBuf::from(format!("出/隔离/{name}")),
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
            decodes: 2,
            timing: VolumeTiming::default(),
        }
    }

    /// 一个判据值。从公开 seam 上真算一个——摆一个编出来的数上去，
    /// 快照就钉不住「报告说的是判据算出来的东西」。
    fn a_score() -> tonefit::Score {
        a_score_of(136)
    }

    /// 一个判据值，深浅由 `shade` 定。候选各不相同的那几个数由它来。
    fn a_score_of(shade: u8) -> tonefit::Score {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        tonefit::score(&reference, &GrayImage::new(Size::new(1, 1), vec![shade]))
    }

    /// 一页上各候选各一个数，档位由低到高——**逐页那一行印的就是这一串**
    /// （`render::score_line`）。
    fn every_candidate() -> Vec<CandidateScore> {
        [
            (BitDepth::One, Dither::FloydSteinberg, 160),
            (BitDepth::Two, Dither::Off, 148),
            (BitDepth::Four, Dither::Off, 136),
            (BitDepth::Eight, Dither::Off, 130),
        ]
        .into_iter()
        .map(|(depth, dither, shade)| CandidateScore {
            candidate: Candidate::new(depth, dither),
            score: a_score_of(shade),
        })
        .collect()
    }

    /// 一份读取计划：探到固态盘、并发读八条。「这一趟怎么读的」那一行印的就是它。
    fn io_plan() -> IoPlan {
        let readers = Readers {
            count: 8,
            chosen_by: ChosenBy::Probe,
        };
        IoPlan {
            medium: Medium::Solid,
            readers,
            fingerprint: readers,
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
        let mut live = Live::new(&request, Resuming::GoesOn);

        live.run_started(2, 10);
        assert_eq!(live.overall().volumes, 2);
        assert_eq!(live.overall().steps, 10);
        // 一步都没走：剩多久答不出来，编一个数出来是骗人。
        assert_eq!(live.overall().left, None);

        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::First, None);
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
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
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

        let mut refused = Live::new(&request, Resuming::GoesOn);
        refused.returned(Err(anyhow::anyhow!("处理范围为空")));
        assert_eq!(refused.exit_code(), crate::REFUSED_EXIT);
        assert!(refused.ended(), "那条线程回来了");
        assert!(
            refused.undone().expect("没做成").contains("处理范围为空"),
            "没做成的那一趟要说得出为什么"
        );

        let mut isolated = Live::new(&request, Resuming::GoesOn);
        let mut report = isolated.report().clone();
        report
            .volumes
            .push(fixture::processed_volume("卷一", Some("解不出来")));
        isolated.returned(Ok(report));
        assert_eq!(isolated.exit_code(), crate::ISOLATED_EXIT);

        let mut clean = Live::new(&request, Resuming::GoesOn);
        let mut report = clean.report().clone();
        report.volumes.push(fixture::skipped_volume("卷一", 20));
        clean.returned(Ok(report));
        assert_eq!(clean.exit_code(), crate::SUCCESS_EXIT);
    }

    /// 跑完之后「已用」就定住了：那个数是库交出来的，不是会话接着读自己那块表。
    #[test]
    fn the_elapsed_time_stops_moving_once_the_run_is_over() {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(1, 10);
        let mut report = live.report().clone();
        report.elapsed = Duration::from_secs(42);
        live.returned(Ok(report));

        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
        assert_eq!(live.overall().left, None, "完了就没有「还剩多久」可说");
        // 再问一次仍是同一个数——会话没有接着读自己那块表。
        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
    }

    /// **按停停下来的那一趟退出码照旧**：两级都一样，它是用户自己的决定，不是失败
    /// （ADR 0013；`crate::exit_code` 的文档写着「按停停下来的那一趟不在这里露面」）。
    ///
    /// 「照旧」不是「恒为零」：报告里有卷被隔离、有卷没做成，那两个数照给——
    /// 按停不改的是**这一趟收成了什么样**与退出码之间那条对应，而那条对应
    /// 与命令行那一路是同一段代码（`crate::exit_code`）。
    #[test]
    fn a_run_that_was_stopped_exits_with_the_code_its_report_earns() {
        for level in [Instruction::Finish, Instruction::Abort] {
            // 停之前跑完的那几卷干干净净：全部成功那个数。
            let mut clean = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
            let mut report = clean.report().clone();
            report.volumes.push(fixture::skipped_volume("卷一", 20));
            report.outcome = RunOutcome::Stopped(level);
            clean.returned(Ok(report.clone()));
            assert_eq!(clean.exit_code(), crate::SUCCESS_EXIT, "{level:?}");
            assert_eq!(clean.exit_code(), crate::exit_code(&report), "{level:?}");

            // 其中一卷带着坏页进了隔离：仍是「有卷被隔离」那个数，按停没把它盖掉。
            let mut isolated = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
            let mut report = isolated.report().clone();
            report
                .volumes
                .push(fixture::processed_volume("卷一", Some("解不出来")));
            report.outcome = RunOutcome::Stopped(level);
            isolated.returned(Ok(report.clone()));
            assert_eq!(isolated.exit_code(), crate::ISOLATED_EXIT, "{level:?}");
            assert_eq!(isolated.exit_code(), crate::exit_code(&report), "{level:?}");
        }
    }

    /// **决策点上那一卷的报告摆得住，也收得掉**（停车场 Q52，`p1-session/14`）。
    ///
    /// 它不进 [`Live::report`]：那一份装的是**收摊了的卷**，而这一卷还停在决策点上，
    /// 第二遍一步没走。混进去的话，退出会话时印到 stdout 的那一份里会多一卷
    /// 「写在那里、盘上却没有」的东西。
    ///
    /// 三条出路各收一次：一卷跑完（正式那一份进了报告）、一卷没做成（它连报告都没有）、
    /// 这一趟收场（停在决策点上被中止的那一卷两条都不报，没有别人会来清它）。
    #[test]
    fn the_summary_at_the_decision_point_stands_until_that_volume_lands() {
        let summarized = fixture::processed_volume("卷一", None);

        // 一卷跑完：正式那一份进报告，摆着的那一份作废。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        assert_eq!(
            live.summarized().map(|volume| volume.volume.clone()),
            Some(summarized.volume.clone()),
            "决策点上那一份没收下"
        );
        assert!(
            live.report().volumes.is_empty(),
            "它混进收摊了的那几卷里去了"
        );
        live.volume_finished(&summarized);
        assert!(live.summarized().is_none(), "这一卷收摊了，那一份还摆着");
        assert_eq!(live.report().volumes.len(), 1);

        // 一卷没做成：同样作废——它连报告都没有。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        live.volume_failed(Path::new("库/卷一"), "盘拔了");
        assert!(live.summarized().is_none());

        // 这一趟收场（决策点上被中止就是这一条）：同样作废。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        live.run_finished(RunOutcome::Stopped(Instruction::Abort));
        assert!(live.summarized().is_none());

        // 别的两遍那一格是 `None`，不该把摆着的那一份抹掉——它只在决策点上有。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        live.pass_started(Pass::First, None);
        assert!(live.summarized().is_some(), "另一遍开工把它抹掉了");
    }

    /// **等答话的那几分钟谁都不算**（停车场 Q41，`CONTEXT.md` 的《会话》：
    /// 决策点上等人的那段时间不算进计时）。
    ///
    /// 不减的话，屏上那两个数会在人看着报告拿主意的那几分钟里一路往上涨，
    /// 而那几分钟里库一步都没走——「剩多久」说的就成了「用户拿主意还要多久」。
    ///
    /// 断言不带余量：把开工那一刻往回拨一段，「已用」就该是那一段；等一小会儿再问，
    /// 它**一格都不该多**（拨回去的那一段远大于这中间的调度抖动）。
    ///
    /// **不等人的那一趟一格不减**：执行那一趟同样走到决策点，但观察者当场答字就返回——
    /// 那一段是库自己的开销，本来就该算进这一趟。
    #[test]
    fn the_minutes_spent_deciding_are_charged_to_nobody() {
        /// 往回拨这么久。够长，调度抖动淹不掉它。
        const RAN_FOR: Duration = Duration::from_secs(300);
        let summarized = fixture::processed_volume("卷一", None);

        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.stepped();
        live.rewind(RAN_FOR);
        // 决策点：等人那一截从这里起算。
        live.pass_started(Pass::Second, Some(&summarized));
        let waiting = live.overall().elapsed;
        assert!(waiting >= RAN_FOR, "等之前那一段被减掉了：{waiting:?}");

        // 人在看报告：屏上那个数一格都不该多。
        std::thread::yield_now();
        let still = live.overall().elapsed;
        assert!(
            still.saturating_sub(waiting) < Duration::from_secs(1),
            "等人的那一截算进了「已用」：{waiting:?} → {still:?}"
        );

        // 答完话接着跑：等掉的那一截留在账上，往后的时间照旧算。
        live.decide(Instruction::Continue);
        let resumed = live.overall().elapsed;
        assert!(
            resumed >= RAN_FOR && resumed.saturating_sub(waiting) < Duration::from_secs(1),
            "答完话之后那一截又被算回来了：{resumed:?}"
        );

        // 不等人的那一趟：决策点照样报，但那一格不开——观察者当场答字就返回。
        let mut going = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        going.run_started(1, 1000);
        going.volume_started(Path::new("库/卷一"), 1000);
        going.rewind(RAN_FOR);
        going.pass_started(Pass::Second, Some(&summarized));
        assert!(
            going.overall().elapsed >= RAN_FOR,
            "不等人的那一趟也开始减了"
        );
    }

    /// **单卷试算在答继续之前印的是 dry-run**（`p1-session/14`，ADR 0012 决定第 5 条）。
    ///
    /// 那一趟走的是 `Mode::Process`——参照要留着，答继续时第一遍才不必重算——
    /// 而在决策点上答出继续之前，输出根一个字节都没有。抬头那一行
    /// 「dry-run：只算不写，下面的路径都还没落盘」正是这时要说的话。
    ///
    /// 别的三种照库收到的那个字印，这一格与从前逐字相同。
    #[test]
    fn a_trial_that_never_walked_the_second_pass_prints_as_a_dry_run() {
        // 续做那一趟：起手、答收尾、答中止，三处都是 dry-run；只有答继续那一处不是。
        for (said, shown) in [
            (None, RunMode::DryRun),
            (Some(Instruction::Finish), RunMode::DryRun),
            (Some(Instruction::Abort), RunMode::DryRun),
            (Some(Instruction::Continue), RunMode::Process),
        ] {
            let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
            if let Some(said) = said {
                live.decide(said);
            }
            assert_eq!(live.decided(), said);
            assert_eq!(live.mode(), shown, "答了 {said:?}");
        }

        // 不续做的那两趟一格不改：执行印执行，多卷试算印 dry-run。
        assert_eq!(
            Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn).mode(),
            RunMode::Process
        );
        assert_eq!(
            Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn).mode(),
            RunMode::DryRun
        );
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
