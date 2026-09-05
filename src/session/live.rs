//! 一趟跑起来之后攒下来的东西：**主区那两块各取所需**（`p1-session/09`、`p3/07`）。
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
//! 「已完成卷的判定、定档页、失败页当场可见」于是不必等整趟跑完。
//!
//! # 屏上那几行各自的来源
//!
//! | 屏上那一行 | 来源 |
//! |---|---|
//! | 总览块的抬头与全局那一行 | `RunStarted` 的 `volumes` 与 `steps`（03 号票的预扫），加 [`Live::walked`] |
//! | 总览块的当前卷那一行 | `VolumeStarted` 的卷名与步数，加 `PassStarted` 的[那一遍](Pass) |
//! | 总览块的结论行与出事行 | 攒到此刻的 [`Live::report`]，按[这一趟是什么](Live::mode)分岔 |
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

use crate::render::{self, Listed, Row};

/// 这一趟**在决策点上等不等人**（`CONTEXT.md` 的《会话》：续做、等答话）。
///
/// 一个枚举而不是一个 `bool`：它从 [`super::resuming`] 一路传到 [`Live::new`] 与
/// [`super::run::Running::start`]，而调用处一个裸 `false` 说不出它否掉的是哪件事
/// （与 `super::state::Listing` 同一条理由——本仓库不爱看不出意思的裸值）。
///
/// 判它的是 [`super::resuming`]，依据是 ADR 0012 决定第 3 条：**试算逐卷等答话**
/// （几卷都一样），而等不等人是调用方的策略、不是库的行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resuming {
    /// **续做**：每走到一个决策点就停下来等人拿主意（试算，几卷都一样）。
    Waits,
    /// **不续做**：决策点上不等人，一趟走到底（执行）。
    GoesOn,
}

impl Resuming {
    /// 这一趟会停下来等人吗。
    fn waits(self) -> bool {
        self == Self::Waits
    }
}

/// 决策点上答的那个字**管几卷**（`CONTEXT.md` 的《会话》：都这样）。
///
/// 与 [`Resuming`] 同一副形状、同一条理由（不爱看不出意思的裸值）：它从
/// [`super::state::Action::Answer`] 一路传到 [`Live::decide`] 与
/// [`super::run::Running::decide`]，而调用处一个裸 `true` 说不出它说的是哪件事。
///
/// **它不是闩**：闩只升不降，记的是「这一趟还走不走」；这一格记的是一个**可以是「继续」
/// 的粘性答案**，摆在观察者那一侧的「决策点的默认答案」上
/// （见 `super::run::Gate`）。两者分开放，按停按到的那一级因此一格不动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// 只答**这一卷**：下一卷的决策点照旧停下来问（`x` 与 `s`）。
    ThisVolume,
    /// **剩下的卷都这样**：这个字连往下每一卷一起答了，从此不再停（`a`）。
    ForTheRest,
}

/// **卷表上停得住的那几卷各是哪一卷**（`CONTEXT.md` 的《会话》：卷表）。
///
/// 「报告上第几卷」答不出决策点上那一卷：它停在[攒着的那一份](Live::summarized)上、
/// 不在[收摊了的那几卷](Report::volumes)里，而 `p2-loose-ends/08` 记着
/// **不许摊开上一卷冒充它**。这个取值认得出这两处，报告区的光标与展开因此指得动它
/// （`p3-session-legibility/10`）。
///
/// **没做成的那几卷不在这里**：它们连一份卷报告都没有（[`VolumeFailure`] 只有一句原因），
/// 逐页那几行无从谈起。表上它们照旧占一行，光标停不上去——那一行要说的话就在它自己的行尾。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volume {
    /// [收摊了的](Report::volumes)第几卷。
    Settled(usize),
    /// **决策点上攒着的那一份**：这一卷还没收摊，第二遍一步没走。
    ///
    /// 那一格装的是**它前面收摊了几卷**，而那个数就是**它的身份**：它收摊之后正是
    /// [`Settled(after)`](Self::Settled)，而「攒着的那一份」这个位置从此归**下一卷**。
    /// 少了这个数，一个记着「停在攒着的那一份上」的光标会在下一个决策点悄悄跳到
    /// 另一卷身上——屏上还写着「跟随停了」，指的却已经不是同一卷
    /// （`p2-loose-ends/08` 那条「不许摊开另一卷冒充它」朝前的那一半）。
    ///
    /// **它不是「第几卷」**：那一卷没做成时它谁都不是（[`nearest`](Live::nearest)
    /// 那时就近收一收），而没做成的卷本来就不在这一列里。
    Summarized { after: usize },
}

/// **报告区目录那一级摆得出的一枝**（`volume-discovery/08`）：一个目录，
/// 加上它底下此刻摆得出的那几卷。
///
/// 分组不在这里——它只有 [`crate::render::grouped`] 一处出处，命令行那一副读的是同一份
/// （`CONTEXT.md` 的《发现》：层次与发现出来的那棵树一致）。本结构只把分出来的那几组
/// 翻成会话认得的东西：[停得住的那几卷](Volume)，以及没做成的那几卷在报告上第几条。
///
/// **它不带那一行**：目录表上一枝写什么在 [`Live::branch_rows`]，而问「哪一枝底下有
/// 哪几卷」的地方多得多（光标停在哪一枝、展开哪一枝、`⇥` 在哪几卷之间转），
/// 那一半只是路径比较。把行摆进来，每问一次「哪一枝」就要把每一卷的基准档现算一遍。
#[derive(Debug, Clone)]
pub struct Branch {
    /// 这一枝是哪个目录。
    pub directory: PathBuf,
    /// 它底下**停得住**的那几卷（[`Volume`]），按表上的先后。
    ///
    /// **可以是空的**：一枝底下的卷全没做成时它一卷都收不住——那几卷连一份卷报告都没有
    /// （见 [`Volume`]）。光标因此停不到这一枝上，与卷表上没做成那几行停不上去
    /// 是同一条规矩。
    pub volumes: Vec<Volume>,
    /// 它底下**没做成**的那几卷在 `Report::failed_volumes` 里的第几条，按报告上的先后。
    pub failures: Vec<usize>,
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
    /// 不直接读它：试算走的是 `Mode::Process`（参照要留着，ADR 0012 决定第 5 条），
    /// 而在决策点上答出继续之前它一个字节都没写。
    ran_as: RunMode,
    /// 这一趟**在决策点上等人**吗（`CONTEXT.md` 的《会话》：续做）。
    ///
    /// 试算是，几卷都一样；执行一趟走到底，在决策点上不停。
    /// 起手那一刻就定死（[`super::press`] 拼 `Request` 时判的），跑起来之后不再变。
    resumes: Resuming,
    /// 在决策点上答过的那几个字里**最弱**的那一个。一次都没答过就是 `None`。
    ///
    /// 屏上那句话与报告抬头都要它，而它们问的是**这一趟落过盘没有**：
    /// 答过一次继续就有一卷写了出去，答收尾的那几卷一个字节都没写
    /// （见 [`mode`](Self::mode)）。
    ///
    /// **取最弱的那一个，与闩正好相反**（闩取最强的，`super::run::Latch`）：
    /// 两者问的不是同一件事——闩问「这一趟还走不走」，越强越说明要停；
    /// 这一格问「落过盘没有」，而落过盘的证据是那个最弱的字。
    decided: Option<Instruction>,
    /// 「剩下的卷都这样」摆下的那个**默认答案**（`CONTEXT.md` 的《会话》：都这样）。
    /// 没答过这个手势就是 `None`。
    ///
    /// 真替往下那几卷答话的是观察者那一侧（`super::run::Gate`）；这一份是给**屏**的：
    /// 屏底那一行要说清「往下不再问了」，而[等人那一截](Self::deliberating_since)
    /// 也要从这一刻起不再开——不再停下来问，人就没有在等，
    /// 而那一格开了就再也关不上（决策点上的答话是关它的唯一一条路）。
    for_the_rest: Option<Instruction>,
    /// 决策点上那一卷**到此刻为止**的报告（`PassStarted` 的 `so_far`，停车场 Q52）。
    ///
    /// 它不进 [`report`](Self::report)：那一份装的是**收摊了的卷**，而这一卷还停在决策点上，
    /// 第二遍一步没走。报告区在卷表上给它一行（见 `super::draw::table`），
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
    /// 已经收摊的卷数（跑完的与没做成的都算）。总览块抬头那个「第几卷」用它。
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
            for_the_rest: None,
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

    /// 预扫完了，开工：总览块那两个数就是 `RunStarted` 报的这两个（03 号票）。
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
            //
            // **答过「剩下的卷都这样」之后就不再等**：那一刻起观察者当场照默认答案答字
            // （`super::run::Gate`），没有人在等。这一格照开的话它再也关不上——
            // 关它的只有决策点上的答话，而往下不会再有一次，屏上那两个数于是从此不动。
            if self.resumes.waits() && self.for_the_rest.is_none() {
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
    /// 幂等命中的卷提前收摊——不结清，那条横条就永远走不到头。
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
    /// **试算在答出第一个继续之前印的是 dry-run**，虽然它走的是 `Mode::Process`：
    /// 那条路留参照是为了答继续时第一遍不重算（ADR 0012 决定第 5 条），
    /// 而在决策点上答出继续之前，输出根一个字节都没有——抬头那一行
    /// 「dry-run：只算不写，下面的路径都还没落盘」正是这时要说的话。
    /// 答了收尾或中止同理：那一趟就此收场，盘上仍旧什么都没有。
    ///
    /// **几十卷的一趟里只要答过一次继续，印的就是执行**：那一卷真写了出去
    /// （见 [`decided`](Self::decided) 那条「取最弱的那一个」）。
    ///
    /// 别的两种（执行、一趟都没跑过）照库收到的那个字印，这一格与从前逐字相同。
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

    /// 记下决策点上答的那个字，以及它[管几卷](Reach)。
    /// **由会话那一头记**（[`super::run::Running::decide`]）：答话的是用户，
    /// 而观察者那一侧只是把它转交给库。
    ///
    /// 记下来的是答过的那几个字里**最弱**的那一个（见 [`decided`](Self::decided)）：
    /// 一趟里每一卷各答一次，而抬头那一行问的是「这一趟落过盘没有」——
    /// 头一卷答了继续、第二卷答了收尾的那一趟，盘上有头一卷。
    pub fn decide(&mut self, said: Instruction, reach: Reach) {
        self.decided = Some(match self.decided {
            Some(before) => before.min(said),
            None => said,
        });
        if reach == Reach::ForTheRest {
            self.for_the_rest = Some(said);
        }
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

    /// 「剩下的卷都这样」摆下的那个默认答案。没答过这个手势就是 `None`。
    ///
    /// 屏底那一行要它：往下的决策点不再停，而这件事得说出来
    /// （见 `super::draw::footer::resuming_line`）——不说的话，一趟几十卷的批量跑
    /// 看上去与「它忘了问」没有分别。
    pub fn for_the_rest(&self) -> Option<Instruction> {
        self.for_the_rest
    }

    /// 决策点上那一卷到此刻为止的报告。没停在决策点上就是 `None`。
    pub fn summarized(&self) -> Option<&VolumeReport> {
        self.summarized.as_ref()
    }

    /// **表上停得住的那几卷**（[`Volume`]），按表上的先后：收摊了的那几卷，
    /// 末尾是决策点上攒着的那一份。
    ///
    /// 报告区的光标走的就是这一列，展开也只到得了它们（`p3-session-legibility/10`）。
    /// **没做成的那几卷不在里面**，理由见 [`Volume`]；表上它们照旧占一行
    /// （见 `super::draw::table`）。
    ///
    /// **与 [`Overall::volumes`] 不是一个数**：那一个是这一趟**点名了**几个卷，
    /// 这一列是**此刻说得出报告**的那几卷。
    pub fn volumes(&self) -> Vec<Volume> {
        let after = self.report.volumes.len();
        (0..after)
            .map(Volume::Settled)
            .chain(
                self.summarized
                    .iter()
                    .map(move |_| Volume::Summarized { after }),
            )
            .collect()
    }

    /// **报告区目录那一级此刻摆得出的那几枝**（[`Branch`]），按表上的先后。
    ///
    /// 收的那一列与[卷表那几行](super::draw::table)**同一个次序**：收摊了的那几卷、
    /// 没做成的那几卷、末尾是决策点上攒着的那一份——分组因此不会把两处排成两个样子。
    ///
    /// **分组只有一处出处**（[`crate::render::grouped`]）：命令行那一副的折叠读的是
    /// 同一份，两边不许各算各的。这里只把下标翻回会话认得的东西。
    pub fn branches(&self) -> Vec<Branch> {
        let listed = self.listed();
        let settled = self.report.volumes.len();
        let failed = self.report.failed_volumes.len();
        render::grouped(&listed)
            .into_iter()
            .map(|group| {
                let mut volumes = Vec::new();
                let mut failures = Vec::new();
                for at in &group.at {
                    if *at < settled {
                        volumes.push(Volume::Settled(*at));
                    } else if *at < settled + failed {
                        failures.push(*at - settled);
                    } else {
                        volumes.push(Volume::Summarized { after: settled });
                    }
                }
                Branch {
                    directory: group.directory,
                    volumes,
                    failures,
                }
            })
            .collect()
    }

    /// 这几枝各自在报告上的[那一行](Row)——几卷 · 基准档分布 · 几卷进了隔离，
    /// **与 [`branches`](Self::branches) 同序**（两处都是 [`crate::render::grouped`]
    /// 出的那几组，一个次序）。
    ///
    /// **只有目录表要它**：那一行的聚合要把每一卷的基准档问一遍，
    /// 而别处只要「哪一枝底下有哪几卷」（见 [`Branch`]）。措辞与聚合都在
    /// [`crate::render::directory`]，命令行那一副读的是同一份。
    pub fn branch_rows(&self) -> Vec<Row> {
        let listed = self.listed();
        render::grouped(&listed)
            .iter()
            .map(|group| render::directory(group, &listed))
            .collect()
    }

    /// 交给[分组](crate::render::grouped)的那一列，**按表上的先后**。
    ///
    /// 与命令行那一路的 [`crate::render::listed`] 差的只有末尾那一条：
    /// 决策点上攒着的那一份也占一条（它还没收摊，命令行那一路根本走不到这个时刻）。
    fn listed(&self) -> Vec<Listed<'_>> {
        render::listed(&self.report)
            .into_iter()
            .chain(self.summarized.iter().map(Listed::Settled))
            .collect()
    }

    /// 这一卷此刻对着哪一份卷报告。**指不着就是 `None`**——决策点上那一卷收摊之后
    /// [`Volume::Summarized`] 就指不着了（那时它是收摊了的最后一卷）。
    pub fn volume(&self, at: Volume) -> Option<&VolumeReport> {
        match at {
            Volume::Settled(at) => self.report.volumes.get(at),
            // **前面收摊了几卷要对得上**：对不上说明那一份已经不是它了
            // （见 [`Volume::Summarized`]）——那一刻它指的是**下一卷**，
            // 而调用方要的是刚才那一卷（先过一道 [`nearest`](Self::nearest)）。
            Volume::Summarized { after } => self
                .summarized
                .as_ref()
                .filter(|_| after == self.report.volumes.len()),
        }
    }

    /// 把一卷**收进此刻真停得住的那几卷**里。**指着旧位置的那几种在这里一次收齐**，
    /// 光标与展开因此都只问这一处。
    ///
    /// 三档：
    ///
    /// 1. **指得着**就是它自己；
    /// 2. **决策点上那一卷收摊了**——它此刻是[第 `after` 卷](Volume::Summarized)，
    ///    那个数就是它的身份，因此**跟着它自己走**，而不是跟着「攒着的那一份」
    ///    那个位置跑到下一卷身上；
    /// 3. 剩下的（那一卷没做成、报告换了一趟）**就近落到最后一卷上**——
    ///    与 `super::viewport::Viewport::new` 那条「光标越界不算错」同一条规矩。
    ///
    /// 一卷都没有时是 `None`。
    pub fn nearest(&self, at: Volume) -> Option<Volume> {
        let volumes = self.volumes();
        if volumes.contains(&at) {
            return Some(at);
        }
        if let Volume::Summarized { after } = at
            && after < self.report.volumes.len()
        {
            return Some(Volume::Settled(after));
        }
        volumes.last().copied()
    }

    /// 攒到此刻的报告。
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// 总览块要的那几个数：第几卷 / 共几卷、走了几步 / 共几步、已用多久、还剩多久。
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

/// 总览块那一块要的几个数（抬头与全局那一行分着用，见 `super::draw::overview`）。
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
    use std::time::Duration;

    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, CandidateScore, ChosenBy, Crop, Dither,
        Envelope, GeometryGate, GrayImage, IoPlan, Medium, Mode as RunMode, PageBranch, PageColor,
        PageOutcome, PageReport, Processed, Profile, Readers, Reason, Reference, Request, Salvage,
        Scaling, Size, Verdict, VolumeReport, VolumeTiming, VolumeVerdict,
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

    /// 一卷做了这么多秒。
    ///
    /// 夹具里给一个**非零**的数：卷表耗时那一列问的正是它，而「跳过一卷为什么也要等这么久」
    /// 只有这个数答得出来（`VolumeTiming::elapsed`）。三份夹具各给各的，快照上分得开。
    fn took(seconds: u64) -> VolumeTiming {
        VolumeTiming {
            elapsed: Duration::from_secs(seconds),
            ..VolumeTiming::default()
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
            extracted: 0,
            io: io_plan(),
            decodes: 0,
            timing: took(3),
        }
    }

    /// 一份**真做过事**的卷报告：一页完好的灰度页定出卷级基准档。
    ///
    /// `broken` 给一句原因就再添一张失败页，那时整卷进隔离目录。
    /// 「一卷跑完当场显示它的判定与定档页」与「失败页带原因」两条验收落在它身上——
    /// 定档页指的就是那一页完好的。
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
            extracted: 0,
            io: io_plan(),
            decodes: 1,
            timing: took(72),
        }
    }

    /// 一份 **`--per-page`** 的卷报告：上包络与迟滞关着，卷内没有基准档。
    ///
    /// 只换判定那一格，逐页那几行照 [`processed_volume`]：这一份要问的是
    /// 「卷表档位那一列照卷级判定说的写」（P3 卷表那一票），与页上画着什么无关。
    pub fn per_page_volume(name: &str) -> VolumeReport {
        VolumeReport {
            verdict: Some(VolumeVerdict::PerPage),
            ..processed_volume(name, None)
        }
    }

    /// 一份**覆盖顶掉判定**的卷报告：覆盖项把候选裁到只剩一个，卷级基准档无从谈起。
    ///
    /// 与 [`per_page_volume`] 同一条：只换判定那一格。
    pub fn overridden_volume(name: &str) -> VolumeReport {
        VolumeReport {
            verdict: Some(VolumeVerdict::Override(Candidate::new(
                BitDepth::Two,
                Dither::FloydSteinberg,
            ))),
            ..processed_volume(name, None)
        }
    }

    /// 一份**每一种页各一张**的卷报告：八页，其中[要紧的](crate::render::notable)六页。
    ///
    /// 逐页表那几条要的正是这一种（`p3-session-legibility/11`）：默认那一副与全部页
    /// 那一副要看得出差别，而「要紧」那六种要在同一卷里各出现一次。
    ///
    /// | 页 | 它要紧在哪儿 |
    /// |---|---|
    /// | `001` | 不要紧：判定跟着卷级基准档走 |
    /// | `002` | 不要紧：走**彩色分支**，只缩放、不量化，也不进上包络 |
    /// | `003` | **定档页**（上包络站在它身上） |
    /// | `004` | **特例页**：判据偏离卷内分布，单独定档；它同时**宽溢出** |
    /// | `005` | **几何门不成立**：源比目标小，抖动单独关掉 |
    /// | `006` | **兜底上界**：目标尺寸退回过 fit-inside |
    /// | `007` | **部分救回**：解到哪个像素算哪个像素，行尾说得出救回了多少 |
    /// | `017` | **失败页**：这一页根本没解出来 |
    ///
    /// **「一页同时要紧在好几处」由 `004` 撑着**（特例加宽溢出），而不是拿兜底上界配宽溢出：
    /// 那一对**凑不到一起**——退回之后的页恒不超过面板宽
    /// （`Report::backstopped`：两张清单不重叠）。
    ///
    /// **页的三种状态在这一卷里都有**（完好、部分救回、失败），彩色分支那一条也在：
    /// 失败页说得出它的尺寸是**卷内统一尺寸**、彩页说得出它不量化也不进上包络，
    /// 而这两句话只有逐页那几行说得出来——卷级那几行一句都没有（`p1-session/11` 的验收）。
    ///
    /// 与 [`processed_volume`] 分开而不是给它加几页：那一份钉着卷级那几张快照
    /// （`p1-session/09` 录的），添一页就要跟着重录。
    pub fn a_page_of_every_kind(name: &str) -> VolumeReport {
        let base = Candidate::new(BitDepth::Four, Dither::Off);
        let source = Size::new(1441, 2048);
        let target = Size::new(1182, 1680);
        // 面板宽 1264（`kobo-libra-2`，见 [`request`]）：这一张比它宽，因此宽溢出。
        let wide = Size::new(1600, 1680);
        let gray = |gate: GeometryGate, verdict: Verdict| PageBranch::Gray {
            gate,
            scores: every_candidate(),
            verdict,
        };
        let judged = |candidate: Candidate, reason: Reason| Verdict { candidate, reason };
        let page = |at: &str, size: Size, backstopped: bool, branch: PageBranch| PageReport {
            source: PathBuf::from(format!("库/{name}/{at}.jpg")),
            output: PathBuf::from(format!("出/隔离/{name}/{at}.png")),
            size,
            outcome: PageOutcome::Whole(Processed {
                crop: Crop::keeping_all(source),
                backstopped,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(source, size),
                color: PageColor::Gray,
                branch,
            }),
        };
        let ordinary = |at: &str| {
            page(
                at,
                target,
                false,
                gray(GeometryGate::Holds, judged(base, Reason::VolumeEnvelope)),
            )
        };
        let mut pages = vec![
            ordinary("001"),
            // 彩色分支：只缩放，不量化，不进灰度缓存也不进上包络——它不要紧，
            // 但全部页那一副上要看得见它那一句。
            page("002", target, false, PageBranch::Color),
            // 定档页：这一卷的基准档就是它判出来的（`Envelope::driver` 指着它）。
            page(
                "003",
                target,
                false,
                gray(
                    GeometryGate::Holds,
                    judged(base, Reason::LowestWithinThreshold),
                ),
            ),
            // 特例页，而且它**同时宽溢出**：不参与上包络、按它自己那一档写出，
            // 而它比面板宽——翻它要阅读器横向平移。一页因此要紧在两处。
            page(
                "004",
                wide,
                false,
                gray(
                    GeometryGate::Holds,
                    judged(
                        Candidate::new(BitDepth::Eight, Dither::Off),
                        Reason::Outlier,
                    ),
                ),
            ),
            // 几何门不成立：抖动单独关掉，位深仍跟着基准档。
            page(
                "005",
                target,
                false,
                gray(GeometryGate::Broken, judged(base, Reason::OutsideTheGate)),
            ),
            // 兜底上界退回过：它没按这一趟点名的适配方式出。**退回之后恒不超过面板宽**
            // （`Report::backstopped`：与宽溢出那张清单不重叠），因此它拿的是普通尺寸。
            page(
                "006",
                target,
                true,
                gray(GeometryGate::Holds, judged(base, Reason::VolumeEnvelope)),
            ),
        ];
        // 部分救回：它有自己的尺寸、判据与判定，却不替整卷说话。
        let PageOutcome::Whole(salvaged) = ordinary("007").outcome else {
            unreachable!("上面那一支造的就是完好页");
        };
        pages.push(PageReport {
            source: PathBuf::from(format!("库/{name}/007.jpg")),
            output: PathBuf::from(format!("出/隔离/{name}/007.png")),
            size: target,
            outcome: PageOutcome::Salvaged {
                page: salvaged,
                salvage: Salvage::from_share(0.62),
            },
        });
        pages.push(PageReport {
            source: PathBuf::from(format!("库/{name}/017.jpg")),
            output: PathBuf::from(format!("出/隔离/{name}/017.png")),
            size: target,
            outcome: PageOutcome::Failed {
                reason: "解不出完整尺寸：JPEG 数据截断".to_owned(),
            },
        });
        VolumeReport {
            volume: PathBuf::from(format!("库/{name}")),
            output: PathBuf::from(format!("出/隔离/{name}")),
            superseded: None,
            source_pages: pages.len(),
            // 其余页那一组是 `001`、`003`、`006` 三张：彩页、特例、门不成立、
            // 部分救回、失败五张都在进这一层之前被摘走了（见 `Envelope::body_pages`）。
            verdict: Some(VolumeVerdict::Envelope(Envelope {
                base,
                driver: 2,
                body_pages: 3,
                outlier_pages: 1,
                raised_pages: 0,
            })),
            pages,
            cache: cache_usage(),
            extracted: 0,
            io: io_plan(),
            decodes: 8,
            timing: took(96),
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
        assert_eq!(live.overall().walked, 10, "那条横条走不到头");
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
        live.decide(Instruction::Continue, Reach::ThisVolume);
        let resumed = live.overall().elapsed;
        assert!(
            resumed >= RAN_FOR && resumed.saturating_sub(waiting) < Duration::from_secs(1),
            "答完话之后那一截又被算回来了：{resumed:?}"
        );

        // 下一卷的决策点：照旧等人，那一格照旧开——一趟里每一卷各等一次
        // （`volume-discovery/07`）。
        live.volume_started(Path::new("库/卷二"), 1000);
        live.pass_started(Pass::Second, Some(&summarized));
        assert!(
            live.deliberating_since.is_some(),
            "第二卷的决策点上没开始等人"
        );
        let waiting = live.overall().elapsed;
        std::thread::yield_now();
        assert!(
            live.overall().elapsed.saturating_sub(waiting) < Duration::from_secs(1),
            "第二卷的决策点上等人的那一截被算进了「已用」"
        );

        // **答「剩下的卷都这样」之后那一格再也不开**：往下的决策点由观察者那一侧
        // 当场答掉，没有人在等。照开的话它再也关不上——关它的只有决策点上的答话，
        // 而往下不会再有一次，屏上那两个数于是从此不动。
        live.decide(Instruction::Continue, Reach::ForTheRest);
        live.volume_started(Path::new("库/卷三"), 1000);
        live.pass_started(Pass::Second, Some(&summarized));
        assert!(
            live.deliberating_since.is_none(),
            "答过「剩下的卷都这样」，等人那一格又开了——没有人在等，而它再也关不上"
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

    /// **试算在答出第一个继续之前印的是 dry-run**（`p1-session/14`，ADR 0012 决定第 5 条）。
    ///
    /// 那一趟走的是 `Mode::Process`——参照要留着，答继续时第一遍才不必重算——
    /// 而在决策点上答出继续之前，输出根一个字节都没有。抬头那一行
    /// 「dry-run：只算不写，下面的路径都还没落盘」正是这时要说的话。
    ///
    /// **一趟里每一卷各答一次**（`volume-discovery/07`），而抬头那一行只有一句：
    /// 答过一次继续就有一卷写了出去，那一趟因此印执行——记下来的是那几个字里
    /// **最弱**的那一个（见 [`Live::decided`]）。
    ///
    /// 执行那一趟照库收到的那个字印，这一格与从前逐字相同。
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
                live.decide(said, Reach::ThisVolume);
            }
            assert_eq!(live.decided(), said);
            assert_eq!(live.mode(), shown, "答了 {said:?}");
        }

        // 几十卷的一趟：头一卷答继续（它写出去了），第二卷答收尾（这一趟到此为止）。
        // 盘上有头一卷，抬头因此不能说「只算不写」——两个次序都问一遍，
        // 记的是最弱的那一个，与答话的先后无关。
        for said in [
            [Instruction::Continue, Instruction::Finish],
            [Instruction::Finish, Instruction::Continue],
        ] {
            let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
            for said in said {
                live.decide(said, Reach::ThisVolume);
            }
            assert_eq!(live.decided(), Some(Instruction::Continue));
            assert_eq!(
                live.mode(),
                RunMode::Process,
                "答过一次继续，抬头却说这一趟一个字节都没写：{said:?}"
            );
        }

        // 「剩下的卷都这样」摆下的那个默认答案单独记一格：它不是闩，也不替
        // [`Live::decided`] 作答——那一格记的仍是答过的字。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        assert_eq!(live.for_the_rest(), None, "没答过那个手势就该是空的");
        live.decide(Instruction::Continue, Reach::ForTheRest);
        assert_eq!(live.for_the_rest(), Some(Instruction::Continue));
        assert_eq!(live.decided(), Some(Instruction::Continue));

        // 执行那一趟一格不改：印执行。
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

    /// **一趟摆得出的那几枝，与卷表读的是同一列**（`volume-discovery/08`）。
    ///
    /// 三种条目各归各的枝：收摊了的那几卷按 [`Volume::Settled`] 认，
    /// 没做成的那几卷按它们在 `Report::failed_volumes` 里第几条认，
    /// 决策点上攒着的那一份按 [`Volume::Summarized`] 认——三种的**次序**与
    /// `super::draw::table` 那一头逐格相同，分组因此不会把两处排成两个样子。
    ///
    /// **一卷都停不住的那一枝照旧摆得出来**：那几卷全没做成，连一份卷报告都没有，
    /// 光标停不上去（与卷表上没做成那几行同一条），而它在屏上仍占一行。
    #[test]
    fn the_branches_are_the_directories_the_volumes_came_from() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(4, 4000);
        live.volume_started(Path::new("库/甲/第1话"), 1000);
        live.volume_finished(&fixture::skipped_volume("甲/第1话", 10));
        live.volume_started(Path::new("库/乙/第1话"), 1000);
        live.volume_finished(&fixture::skipped_volume("乙/第1话", 10));
        live.volume_failed(Path::new("库/丙/没做成"), "卷根不在了");
        live.volume_started(Path::new("库/甲/第2话"), 1000);
        live.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::processed_volume("甲/第2话", None)),
        );

        let branches = live.branches();

        let names: Vec<String> = branches
            .iter()
            .map(|branch| branch.directory.display().to_string())
            .collect();
        assert_eq!(names, ["库/甲", "库/乙", "库/丙"], "分的不是那几枝");
        // 甲那一枝：收摊了的第 0 卷，加上决策点上攒着的那一份。
        assert_eq!(
            branches[0].volumes,
            [Volume::Settled(0), Volume::Summarized { after: 2 }]
        );
        assert!(branches[0].failures.is_empty());
        assert_eq!(branches[1].volumes, [Volume::Settled(1)]);
        // 丙那一枝一卷都停不住：它只有一条没做成的。
        assert!(branches[2].volumes.is_empty(), "没做成的卷停得住了");
        assert_eq!(branches[2].failures, [0]);
        // 那几行与这几枝**同序**，说得出各枝几卷——措辞与聚合只有
        // `crate::render::directory` 一处出处。
        let rows = live.branch_rows();
        assert_eq!(rows.len(), branches.len(), "行与枝对不上");
        assert_eq!(rows[0].cell(crate::render::Field::VolumeCount), Some("2"));
        assert_eq!(rows[2].cell(crate::render::Field::VolumeCount), Some("1"));
    }
}
