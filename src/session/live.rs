//! 一趟跑起来之后攒下来的东西：总览、卷列表、每页结果**各取所需**（`p1-session/09`、`p3/07`）。
//!
//! **这个模块一个终端都不碰**，与 [`super::state`] 同一条规矩：它只把事件流折成几个数
//! 加一份报告，画成什么样是画法那一层（`super::shell`）的事。本模块的用例因此连终端库都编译不到。
//!
//! # 事件流就是报告的增量
//!
//! 一卷跑完那条事件带着那一卷的 [`VolumeReport`]（ADR 0011 决定第 2 条），
//! 这里把它接到 [`Live::report`] 上。**攒出来的就是命令行最后一次性拿到的那一份**，
//! 因此屏上读的是 [`crate::render`] 的那几个函数造出来的格——会话不另写一套措辞。
//!
//! 报告攒到一半也答得出抬头那几件事（`render::header` 吃的是整份报告，不是一个 profile），
//! 「已完成卷的判定、代表页、坏页当场可见」于是不必等整趟跑完。
//!
//! # 屏上那几行各自的来源
//!
//! | 屏上那一行 | 来源 |
//! |---|---|
//! | 总览的抬头与总进度那一行 | `RunStarted` 的 `volumes` 与 `steps`（03 号票的清点），加 [`Live::walked`] |
//! | 卷清单与每一卷此刻怎么样 | `RunStarted` 带的清点产出（`session-redesign/03`），此后逐条事件推出[卷状态](VolumeState) |
//! | 总览的当前卷那一行 | `VolumeStarted` 的卷名与步数，加 `PassStarted` 的[那一遍](Pass) |
//! | 总览的结论行 | 攒到此刻的 [`Live::report`]，按[起手按的哪一个键](Live::started_as)分岔，[第一卷真写完](Live::has_written)翻成转换那一副 |
//! | 总览的问题行 | 同上，而坏页那一样连当前这一卷已经报过的那几页一起数（[`Live::failures_so_far`]） |
//! | 卷列表那几行与每页结果 | `VolumeFinished` 带的卷报告、`VolumeFailed` 那一句（[`Live::report_at`]、[`Live::undone_at`]） |
//!
//! 预告的步数是**上界**不是承诺（`CONTEXT.md` 的《进度》）。拿它画全局进度的实现方
//! 因此要在一卷跑完时**结清**那一卷预告剩下的步——这是 [`tonefit::Event::RunStarted`]
//! 对实现方的要求，命令行那一份见 `crate::Bar::finish_volume`，本模块那一份见
//! [`Live::finish_volume`]。

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tonefit::{
    Event, Instruction, Mode as RunMode, NonVolumeFile, Pass, Report, Request, RunOutcome,
    SurveyedVolume, UnreachablePlace, VolumeFailure, VolumeReport,
};

use crate::render;

/// 这一趟**在确认点上等不等人**（`CONTEXT.md` 的《会话》：接着写出、等待确认）。
///
/// 一个枚举而不是一个 `bool`：它从 [`super::resuming`] 一路传到 [`Live::new`] 与
/// [`super::run::Running::start`]，而调用处一个裸 `false` 说不出它否掉的是哪件事
/// （与 `super::state::Listing` 同一条理由——本仓库不爱看不出意思的裸值）。
///
/// 判它的是 [`super::resuming`]，依据是 ADR 0012 决定第 3 条：**预览逐卷等待确认**
/// （几卷都一样），而等不等人是调用方的策略、不是库的行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resuming {
    /// **接着写出**：每走到一个确认点就停下来等人拿主意（预览，几卷都一样）。
    Waits,
    /// **不接着写出**：确认点上不等人，一趟走到底（执行）。
    GoesOn,
}

impl Resuming {
    /// 这一趟会停下来等人吗。
    fn waits(self) -> bool {
        self == Self::Waits
    }
}

/// 确认点上答的那个字**管几卷**（`CONTEXT.md` 的《会话》：后面都写出）。
///
/// 与 [`Resuming`] 同一副形状、同一条理由（不爱看不出意思的裸值）：它从
/// 按键表（`super::keymap::Deed::answer`）一路传到 [`Live::decide`] 与
/// [`super::run::Running::decide`]，而调用处一个裸 `true` 说不出它说的是哪件事。
///
/// **它不是闩**：闩只升不降，记的是「这一趟还走不走」；这一格记的是一个**可以是「继续」
/// 的粘性答案**，摆在观察者那一侧的「确认点的默认答案」上
/// （见 `super::run::Gate`）。两者分开放，按停止时按到的那一级因此一格不动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// 只答**这一卷**：下一卷的确认点照旧停下来问（`x` 与 `s`）。
    ThisVolume,
    /// **后面的卷都写出**：这个字连往下每一卷一起答了，从此不再停（`a`）。
    ForTheRest,
}

/// **卷状态**：卷清单上的一卷此刻怎么样（`CONTEXT.md` 的《会话》：卷状态）。
///
/// 卷的身份是**清单里的第几卷**（`session-redesign/03`）：一卷在开工之前就有身份，
/// 它此刻怎么样由随后的事件推出来——开卷翻成处理中，某一遍开工记下走到哪个环节，
/// 确认点上等人是等待确认，收摊按那一卷的报告分成完成、跳过、进了隔离，
/// 没做成是那一条事件，这一趟结束时还开着的那一卷是被立即停止掉的。
///
/// 三种收摊分开而不是各带一份报告：报告在 [`Live::report`] 上，这一格只答「怎么样」——
/// 行首记号问的正是这一件（`CONTEXT.md` 的《会话》：行首记号）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeState {
    /// **等待中**：还没轮到。这一趟结束之后仍是它的那几卷，就是被停止拿走的那几卷。
    Queued,
    /// **处理中**，带着走到哪个环节。开卷之后、第一条 `PassStarted` 到达之前是 `None`。
    Running { pass: Option<Pass> },
    /// **等待确认**：停在确认点上等用户拿主意（`CONTEXT.md` 的《会话》：等待确认）。
    ///
    /// 只有真停下来问的那一次才是它：不等人的那一趟、答过「后面的卷都写出」之后的那几卷，
    /// 走到写出那一遍仍是[处理中](Self::Running)——判据是 [`Live::stops_to_ask`]。
    /// 它不看确认点那一条带没带那份报告：闸停不停与那一格是两件事（`tonefit::Progress` 的
    /// `reads_the_report_at_the_decision_point`）。闩已经是立即停止时那道闸一句话都不问
    /// （`super::run::Watch`），这一格会短暂是它，紧接着结束那一条把它翻成
    /// [被立即停止掉](Self::Aborted)。
    Deciding,
    /// **完成**：收摊了，做过事、没进隔离。
    Done,
    /// **进了隔离**：收摊了，而它有坏页（`VolumeReport::isolated`）。
    Isolated,
    /// **跳过**：幂等命中，一页都没重做。
    Skipped,
    /// **没做成**：整卷没做成，报告上只有一句原因（`VolumeFailure`）。
    Failed,
    /// **被立即停止掉**：开了卷，这一趟就被立即停止了——既没收摊也没报没做成，那一卷等于没做
    /// （`tonefit::Event::VolumeFinished` 的文档：流上一条开卷、后面两条一条都没有）。
    ///
    /// 拒绝开始的那一趟撞在半路（互锁 ③，`RunOutcome::Refused`）留下的也是这一副形状，
    /// 那一卷同样等于没做，这里不另分一种（停车场 Q746）。
    Aborted,
}

/// 当前卷那一条：它叫什么、预告多少步、走了几步、在走哪一遍、这一遍写不写盘。
impl VolumeState {
    /// 这一卷**收摊了**吗：做完 · 进了隔离 · 跳过 · 没做成都算（`CONTEXT.md` 的《卷状态》）。
    ///
    /// **一处出处**：目录行的「做完几卷／共几卷」、总览结论行的「等待几卷」问的是同一件事。
    /// 被立即停止掉的那一卷**不算**——它既没收摊也没报没做成。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn settled(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Isolated | Self::Skipped | Self::Failed
        )
    }

    /// 这一卷的卷行**展得开**吗（`CONTEXT.md` 的《停得住 / 展得开》：收摊了的那几卷
    /// 连同确认点上那一份进得去每页结果）。
    ///
    /// **一处出处**：屏底摆不摆 `l → 每页结果`（[`super::view::Session::hints`]）
    /// 与按下去换不换屏（`super::terminal` 的 `open_a_volume`）问的是同一件事——
    /// 「屏上不摆按不动的键」那句话，只有这两处读同一份判据才成立。
    ///
    /// 与[收摊了](Self::settled)差两格：**没做成的那一卷收摊了、却没有每页结果**
    /// （报告上只有一句原因），而**等待确认**那一份还没收摊、每页结果已经算出来了。
    pub fn opens_the_pages(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Isolated | Self::Skipped | Self::Deciding
        )
    }
}

/// 一卷**需留意的页按种类各几页**（`CONTEXT.md` 的《需留意的页》）。
///
/// **只有数，没有屏上的词。** 这一份要同时答两个问题——卷行行尾写哪几样
/// （画法那一层，在 `tui` 特性后面）与 `]d`／`[d` 跳不跳到这一卷
/// （[`Live::troubled_at`]，在特性**外面**）——而措辞只有界面层说得算。
/// 数出自 [`Live::notable_at`]，词在 `super::shell::list` 一处。
///
/// **坏页、代表页与兜底上界三种不在这里**：坏页由**行首记号**与隔离那一句说，
/// 另两种不是「出了事」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotableTally {
    /// 差异大的页。
    pub outlier: usize,
    /// 页面超宽的页。
    pub overflowed: usize,
    /// 尺寸未贴合屏幕的页。
    pub outside_the_gate: usize,
    /// 残缺页。
    pub salvaged: usize,
}

impl NotableTally {
    /// 一共几页需留意。**一页要紧在好几处就数好几回**——屏上那几个数各报各的
    /// （`CONTEXT.md` 的《需留意的页》：一页可以同时要紧在好几处）。
    pub fn pages(self) -> usize {
        self.outlier + self.overflowed + self.outside_the_gate + self.salvaged
    }

    /// 有没有需留意的页——卷行行首那个 `!` 与 `]d` 的落点问的都是它。
    pub fn any(self) -> bool {
        self.pages() > 0
    }
}

#[derive(Debug, Clone)]
pub struct Walking {
    /// 卷标识：源目录路径，或源归档的文件路径。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法与那条循环读得到，而它们在 tui 特性后面")
    )]
    pub volume: PathBuf,
    /// 这一卷这一趟最多走多少步。**上界**，不是承诺。
    pub steps: u64,
    /// 已经走过的步数。
    pub walked: u64,
    /// 在走哪一遍。开卷之后、第一条 `PassStarted` 到达之前是 `None`。
    pub pass: Option<Pass>,
    /// **这一环节开工那一刻已经走过几步**：屏上「这一环节走到第几页」是
    /// [`walked`](Self::walked) 减它（`CONTEXT.md` 的《目录行 / 卷行》：
    /// 行尾那一句带的是**这一环节**的进度，不是这一卷累计的步数）。
    pub pass_from: u64,
    /// **这一卷在往盘上写吗**：走到写出那一遍，而且这一遍真写盘。
    ///
    /// 执行那一趟走到那一遍就在写；接着写出那一趟那一遍前头是确认点，**答了继续才写**
    /// （答做完再停的那一卷等于走了一次预览，`CONTEXT.md` 的《会话》：确认点），
    /// 答过「后面的卷都写出·继续」之后的那几卷不再问、走到那一遍当场就在写。
    /// 幂等命中跳过的卷到不了那一遍，恒是假。
    ///
    /// 它是[「这一趟真写出过没有」](Live::has_written)翻面的依据：这一卷收摊时它是真，
    /// 那一趟就真写出过一卷了。挂在当前卷这一条上而不是 `Live` 上，是因为它**逐卷问**——
    /// 卷收摊这一条整个撤掉，下一卷从假起，不必另记一格再手动清。
    pub writes: bool,
}

/// 一趟跑起来之后攒下来的东西。**一趟一份**：按下预览或执行时新造一个，
/// 跑完仍留着——退出会话时印到 stdout 的就是它这一份报告。
#[derive(Debug, Clone)]
pub struct Live {
    /// 库那一侧真收到的那个 mode。**屏上照哪一种印走 [`mode`](Self::mode)**，
    /// 不直接读它：预览走的是 `Mode::Process`（参照要留着，ADR 0012 决定第 5 条），
    /// 而在确认点上答出继续之前它一个字节都没写。
    ran_as: RunMode,
    /// 这一趟**在确认点上等人**吗（`CONTEXT.md` 的《会话》：接着写出）。
    ///
    /// 预览是，几卷都一样；执行一趟走到底，在确认点上不停。
    /// 起手那一刻就定死（`crate::session::terminal` 起一趟、拼 `Request` 时判的），跑起来之后不再变。
    resumes: Resuming,
    /// 在确认点上答过的那几个字里**最弱**的那一个。一次都没答过就是 `None`。
    ///
    /// 屏上那句话与报告抬头都要它，而它们问的是**这一趟落过盘没有**：
    /// 答过一次继续就有一卷写了出去，答做完再停的那几卷一个字节都没写
    /// （见 [`mode`](Self::mode)）。
    ///
    /// **取最弱的那一个，与闩正好相反**（闩取最强的，`super::run::Latch`）：
    /// 两者问的不是同一件事——闩问「这一趟还走不走」，越强越说明要停；
    /// 这一格问「落过盘没有」，而落过盘的证据是那个最弱的字。
    decided: Option<Instruction>,
    /// 「后面的卷都写出」摆下的那个**默认答案**（`CONTEXT.md` 的《会话》：后面都写出）。
    /// 没答过这个手势就是 `None`。
    ///
    /// 真替往下那几卷答话的是观察者那一侧（`super::run::Gate`）；这一份是给**屏**的：
    /// 屏底那一行要说清「往下不再问了」，而[等人那一截](Self::deliberating_since)
    /// 也要从这一刻起不再开——不再停下来问，人就没有在等，
    /// 而那一格开了就再也关不上（确认点上的答话是关它的唯一一条路）。
    for_the_rest: Option<Instruction>,
    /// **这一趟到此刻为止真写出过东西没有**（`no-false-line/04`，收停车场 Q196）。
    ///
    /// 一格布尔、**只升不降**，形状与闩相同（`super::run::Latch`）：翻成真的那一刻是
    /// **第一卷真写完**——[在写的那一卷](Walking::writes)收摊
    /// （[`volume_finished`](Self::volume_finished)），此后答什么、收什么场都不再动它。
    /// 读法见 [`has_written`](Self::has_written)，与另两个谓词的分别也写在那儿。
    written: bool,
    /// 确认点上那一卷**到此刻为止**的报告（`PassStarted` 的 `so_far`，停车场 Q52）。
    ///
    /// 它不进 [`report`](Self::report)：那一份装的是**收摊了的卷**，而这一卷还停在确认点上，
    /// 写出环节一步没走。确认条与那一卷的每页结果读的就是它（[`Live::report_at`]），
    /// 「把那一卷画出来等你拿主意」靠的就是它。一卷收摊时清掉——那时正式的一份在报告里了。
    summarized: Option<VolumeReport>,
    /// 这一趟在确认点上**等人等掉的那一截**，累计（停车场 Q41，`CONTEXT.md` 的《会话》：
    /// 确认点上等人的那段时间不算进计时）。
    ///
    /// 库那一侧自己也减掉它（`Report::elapsed`、`VolumeTiming::elapsed`），但那一份要等
    /// 这一趟结束才交得出来。会话这一头**边跑边画**，因此自己也得记一份：不记的话，
    /// 屏上那两个数会在人看着报告的那几分钟里一路往上涨，而那几分钟里库一步都没走——
    /// 「剩 2h13m」说的就成了「用户拿主意还要多久」。
    deliberated: Duration,
    /// 这一次等是从什么时候起的。没在等就是 `None`。
    ///
    /// 只有**接着写出那一趟**记它：别的趟在确认点上不等人（观察者当场答字就返回），
    /// 那一格开了就再也关不上。
    deliberating_since: Option<Instant>,
    /// 攒到此刻的报告，**除了收摊了的那几卷**：那一列在 [`settled`](Self::settled)，
    /// 这一份的 `volumes` 恒空。开工那一刻它是「零卷的一份」——抬头那几件事已经答得出。
    /// 要整份的那几处（退出时印的报告、退出码）走 [`report`](Self::report)，当场拼一份。
    report: Report,
    /// **收摊了的那几卷的报告**，照收摊的先后——整份报告的 `volumes` 那一列。
    ///
    /// **一卷一个 `Arc`**（`session-redesign/17`）：画一帧之前在锁里拷一份 `Live`
    /// （`super::run::Running::glimpse`），拷的于是是几千个指针，不是逐页结果。
    /// 一卷收摊之后它那一份再也不变，共用不会看见半截。
    settled: Vec<Arc<VolumeReport>>,
    /// 清单上第几卷的报告是 [`settled`](Self::settled) 里第几份，与
    /// [`roster`](Self::roster) 同序同长。卷列表每画一行都要问它，逐条比卷根是
    /// 「卷数 × 卷数」（`session-redesign/17`，《卡顿的根因》）。
    settled_at: Vec<Option<usize>>,
    /// [`settled`](Self::settled) 每一份**折出来的那几个数**，同序同长。
    /// 收摊那一刻折一次（[`digest_at`](Self::digest_at)）：那一份此后不变，
    /// 而每画一帧逐页重判一遍几千卷，正是大库上卡的那一截。
    digests: Vec<Digest>,
    /// [`settled`](Self::settled) 里一共坏了几页（`Report::failures` 那个数），收摊时加上。
    settled_failures: usize,
    /// 清单上第几卷**没做成**的那一句在报告的 `failed_volumes` 里第几条，
    /// 与 [`roster`](Self::roster) 同序同长。
    failed_at: Vec<Option<usize>>,
    /// **卷根换回清单序号**：清点已按卷根收编过，清单里卷根不重。开工那一刻立起来，此后不变。
    index: Arc<HashMap<PathBuf, usize>>,
    /// 这一趟点名了几个卷（`RunStarted`）。
    volumes: usize,
    /// 这一趟最多走多少步（`RunStarted`，各卷之和）。
    steps: u64,
    /// **卷清单**：开工那一条带的清点产出，照发现的次序（`session-redesign/03`）。
    /// 开工那一刻整份收下，此后一格不变。
    roster: Arc<Vec<SurveyedVolume>>,
    /// 清单上每一卷**做了多久**，与 [`roster`](Self::roster) 同序同长。
    ///
    /// **只给没有报告的那几卷用**（没做成、被立即停止掉、还在跑的那一卷）：收摊了的卷
    /// 那个数在它自己那份报告的 `VolumeTiming::elapsed` 上，而只有库那一侧减得掉在
    /// 确认点上等人的那一截（与 [`deliberated`](Self::deliberated) 同一条理由）。
    /// 屏上那一列因此**有报告就走报告**，没有的才落到这一份上（`super::shell::list`）。
    timings: Vec<Duration>,
    /// 当前这一卷是什么时候开的。卷与卷之间是 `None`。
    began: Option<Instant>,
    /// 清单上每一卷此刻怎么样，**与 [`roster`](Self::roster) 同序同长**——两列在
    /// [`surveyed`](Self::surveyed) 里一起立起来，此后只改值、不增删。
    states: Vec<VolumeState>,
    /// 当前卷在清单上排第几。卷与卷之间是 `None`；开卷那一条报的卷根
    /// 不在清单上时也是 `None`（那时谁的状态都不动）。
    current: Option<usize>,
    /// 开工那一条带的非漫画文件那张表（`session-redesign/03`）。
    ///
    /// **摆在报告旁边，不当场进报告**：报告上那张表跟着 [`returned`](Self::returned) 换上的
    /// 那一份到（逐条相同，停车场 Q745、Q964）。
    /// 屏上读的是这一张。
    non_volume_files: Arc<Vec<NonVolumeFile>>,
    /// 开工那一条带的无法访问的地方那张表，与上一格同一个待遇。
    unreachable_places: Arc<Vec<UnreachablePlace>>,
    /// 全局走过的步数，含各卷收摊时结清的那一截。
    walked: u64,
    /// 已经收摊的卷数（跑完的与没做成的都算）。总览块抬头那个「第几卷」用它。
    finished: usize,
    /// **此刻**（`CONTEXT.md` 的《会话》）：会话层这一帧的时钟读数。
    ///
    /// 本模块**除造它那一刻外不问系统时钟**：开工那一刻、等人那一截的起止、已用与预计，
    /// 读的都是它。真会话每帧把单调时钟读一次交进来（[`tick`](Self::tick)，
    /// 见 `super::run::Running::glimpse`），用例给定值——屏上那几个数因此是定值、不随机器快慢变，
    /// 而一帧之内几处读到的是同一个时刻（停车场 Q118 那一格的差正是各读各的表读出来的）。
    ///
    /// 造它那一刻读的那一次是它的初值：下一帧到来之前计算线程就可能报到，
    /// 那几条事件要有一个时刻可记（停车场 Q754）。
    now: Instant,
    /// 开工那一刻（[此刻](Self::now)在开工那一条上的读数）。剩余时间由它与
    /// [`walked`](Self::walked) 算出。
    started: Instant,
    /// 当前卷。卷与卷之间是 `None`。
    volume: Option<Walking>,
    /// **当前这一卷已经报过的坏页**，几条。一卷收摊时清零（见
    /// [`finish_volume`](Self::finish_volume)）——那一刻它们的去处已经定了。
    in_flight_failures: usize,
    /// **跟着没做成的那几卷一起没了去处的坏页**，几条。
    ///
    /// 一卷没做成时它连一份卷报告都没有（[`VolumeFailure`] 只带一句原因），
    /// 它那几页因此**永远进不了** [`report`](Self::report)——而它们确实坏了。
    /// 不单记一格的话，那一卷废掉的那一刻屏上那个数会自己往回走
    /// （一个自称答「此刻」的数缩回去，正是 Q148 要治的病换了一种卷）。
    lost_failures: usize,
    /// 那条线程回来了没有。
    ///
    /// **这一趟收成了什么样不在这里**——那是[结束](RunOutcome)，在报告上
    /// （`Report::outcome`），会话不另立一个同义的词。这一格只答「还跑着吗」。
    ended: bool,
    /// 这一趟**没做成**时那句话（`CONTEXT.md` 的《失败》：退出码 `1` 那一种）。
    ///
    /// 两种都落在这里：**拒绝开始**（错在这一趟的参数上，`run` 返回的是错误本身），
    /// 以及那条线程**恐慌**了。两者都没有库交出来的那份报告，退出码都是命令行那一路的
    /// `1`——分得开它们的是这句话本身。
    ///
    /// **stdout 上仍旧有东西**（21 号票，收停车场 Q66）：攒到一半的那一份留着
    /// （见 [`returned`](Self::returned)），退出会话时它连同这一句一起印出去，
    /// 先前那一趟做成了的报告也不跟着丢（见 `super::run::Running::report`）。
    ///
    /// 会话不因此退出：把这句话画出来，用户当场改（spec 的《卷转换失败与退出码》）。
    undone: Option<String>,
}

impl Live {
    /// 开一趟：抬头那几件事从 [`Request`] 上就答得出，因此报告当场就有一份。
    ///
    /// 系统时钟只在这里读一次，作头一个[此刻](Self::now)；此后每一帧由会话层给
    /// （[`tick`](Self::tick)）。
    pub fn new(request: &Request, resumes: Resuming) -> Self {
        let now = Instant::now();
        Self {
            ran_as: request.mode,
            resumes,
            decided: None,
            for_the_rest: None,
            written: false,
            summarized: None,
            deliberated: Duration::ZERO,
            deliberating_since: None,
            report: Report {
                profile: request.profile.clone(),
                fit: request.fit,
                crop: request.crop,
                split: request.split,
                white_align_limit: request.white_align_limit,
                volumes: Vec::new(),
                failed_volumes: Vec::new(),
                // 这两张表整份在清点走完就齐了，开工那一条事件带着它们（`session-redesign/03`），
                // 而攒到一半的这一份**仍旧空着**，跑完换成库交出来的那一份
                // （见 [`returned`](Self::returned)）：清点那一刻收下的那两张摆在报告旁边
                // （[`Self::non_volume_files`]），理由写在那一格上。
                non_volume_files: Vec::new(),
                unreachable_places: Vec::new(),
                outcome: RunOutcome::Completed,
                // 计时只进结构、不进渲染出的文字（见 `tonefit::Report::elapsed`），
                // 攒到一半的这一份因此填零就够——跑完会换成库交出来的那一份。
                elapsed: Duration::ZERO,
            },
            settled: Vec::new(),
            settled_at: Vec::new(),
            digests: Vec::new(),
            settled_failures: 0,
            failed_at: Vec::new(),
            index: Arc::default(),
            volumes: 0,
            steps: 0,
            roster: Arc::default(),
            states: Vec::new(),
            timings: Vec::new(),
            began: None,
            current: None,
            non_volume_files: Arc::default(),
            unreachable_places: Arc::default(),
            walked: 0,
            finished: 0,
            now,
            started: now,
            volume: None,
            in_flight_failures: 0,
            lost_failures: 0,
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
    ///
    /// **开工那一条转给两个方法**：总览块那两个数走 [`run_started`](Self::run_started)，
    /// 清点的三份产出走 [`surveyed`](Self::surveyed)。分成两半是因为只关心那两个数的用例
    /// 只喂前一半就够了。
    pub fn observe(&mut self, event: &Event<'_>) {
        match event {
            Event::RunStarted {
                volumes,
                steps,
                roster,
                non_volume_files,
                unreachable_places,
                ..
            } => {
                self.run_started(*volumes, *steps);
                self.surveyed(roster, non_volume_files, unreachable_places);
            }
            Event::VolumeStarted { volume, steps, .. } => self.volume_started(volume, *steps),
            Event::PassStarted { pass, so_far, .. } => self.pass_started(*pass, *so_far),
            Event::Stepped { .. } => self.stepped(),
            Event::PageFailed { .. } => self.page_failed(),
            Event::VolumeFinished { report, .. } => self.volume_finished(report),
            Event::VolumeFailed { volume, reason, .. } => self.volume_failed(volume, reason),
            Event::RunFinished { outcome, .. } => self.run_finished(*outcome),
            _ => {}
        }
    }

    /// 清点完了，开工：总览块那两个数就是 `RunStarted` 报的这两个（03 号票）。
    ///
    /// 同一条事件带的清点产出走 [`surveyed`](Self::surveyed)——见 [`observe`](Self::observe)
    /// 那一支为什么分成两半。
    pub fn run_started(&mut self, volumes: usize, steps: u64) {
        self.volumes = volumes;
        self.steps = steps;
        // 表从这里开始掐：开工之前那一段是清点与那几道检查，剩余时间算不进去。
        self.started = self.now;
    }

    /// **这一帧的此刻**（`session-redesign/04`，spec《时钟》）：会话层每帧读一次单调时钟
    /// 交进来，此后到下一帧为止本模块记的、算的时刻都是它（见 [`Self::now`]）。
    ///
    /// 用例给定值——夹具要摆出「开工了 21 秒」，就在开工那一条之前给一个时刻、
    /// 画之前再给那个时刻加 21 秒（`session-redesign/05`）。**开工那一条之前那一次不能省**：
    /// 没给过的话开工那一刻记的是造它那一刻读的系统时钟，「已用」于是又差了造它到给时刻
    /// 之间那几微秒——随机器快慢变、而且只在秒的进位上偶尔露面。用例因此一律从
    /// `fixture::live_at` 起：造与头一次给合成一步，漏不掉。
    #[cfg_attr(
        not(feature = "tui"),
        allow(
            dead_code,
            reason = "屏外只有本模块的用例读它，而那条循环在 tui 特性后面"
        )
    )]
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
    }

    /// 收下开工那一条带的**清点产出**（`session-redesign/03`）：卷清单整份留下、每一卷立成
    /// [等待中](VolumeState::Queued)，两张表摆在报告旁边。
    ///
    /// 三样在开工那一条上就齐了、此后不再变，分区末尾的备注行因此在第一卷开工之前
    /// 就画得出来。两张表**不当场进报告**，理由见 [`Self::non_volume_files`]。
    pub fn surveyed(
        &mut self,
        roster: &[SurveyedVolume],
        non_volume_files: &[NonVolumeFile],
        unreachable_places: &[UnreachablePlace],
    ) {
        self.roster = Arc::new(roster.to_vec());
        self.index = Arc::new(
            roster
                .iter()
                .enumerate()
                .map(|(at, listed)| (listed.root.clone(), at))
                .collect(),
        );
        self.states = vec![VolumeState::Queued; roster.len()];
        self.timings = vec![Duration::ZERO; roster.len()];
        self.began = None;
        self.current = None;
        self.non_volume_files = Arc::new(non_volume_files.to_vec());
        self.unreachable_places = Arc::new(unreachable_places.to_vec());
        self.index_the_settled();
    }

    /// 开一卷。**按卷根认回清单里的那一卷**（清点已按卷根收编过，清单里卷根不重），
    /// 它从此是[处理中](VolumeState::Running)。
    pub fn volume_started(&mut self, volume: &Path, steps: u64) {
        self.volume = Some(Walking {
            volume: volume.to_path_buf(),
            steps,
            walked: 0,
            pass: None,
            pass_from: 0,
            writes: false,
        });
        self.current = self.index.get(volume).copied();
        self.began = Some(self.now);
        self.set_state(VolumeState::Running { pass: None });
    }

    /// 把当前卷改成 `state`。没有当前卷（或它不在清单上）就什么都不做。
    fn set_state(&mut self, state: VolumeState) {
        if let Some(current) = self.current
            && let Some(slot) = self.states.get_mut(current)
        {
            *slot = state;
        }
    }

    /// **确认点上这一趟停不停下来问**：等人的那一趟，而且还没摆下「后面的卷都写出」
    /// 那个默认答案（`super::run::Gate`）。等待确认那一档与等人那一截那格都按它判。
    fn stops_to_ask(&self) -> bool {
        self.resumes.waits() && self.for_the_rest.is_none()
    }

    /// **当前卷此刻正停在确认点上等人吗**（`CONTEXT.md` 的《卷状态》：等待确认）。
    ///
    /// 问的是清单上那一格，**不是 [`Walking::pass`]**：库那一侧的确认点就是写出那一遍
    /// 那一条事件（`crate::progress` 的 `ask_before_the_second_pass`），走到这一刻
    /// 「在走哪一遍」已经是写出了，而那一遍还一步没走、也可能永远不走
    /// （答「不写出」的那一卷）。两者分两档，[`pass_started`](Self::pass_started) 那一支
    /// 立的就是这一条。
    ///
    /// 与另外两处「停在确认点上」各答各的：`super::run::Running::deciding` 问的是闸上
    /// 此刻有没有人在等，`super::state::Session::deciding` 问的是会话这一副样子换了没有。
    /// 这一处问的是**那一卷**。
    pub fn deciding(&self) -> bool {
        self.current
            .is_some_and(|at| self.states.get(at) == Some(&VolumeState::Deciding))
    }

    /// 当前卷开始走某一遍。「进度条现在在走哪一遍」只有它答得出来。
    ///
    /// **确认点那一条还带着这一卷到此刻为止的报告**（`so_far`，停车场 Q52）：收下它，
    /// 确认条就画得出「拿什么主意」。另外两遍那一格是 `None`，这里因此不动它。
    ///
    /// **走到写出那一遍，「这一卷写不写盘」在这里定一半**（[`Walking::writes`]）：
    /// 执行那一趟走到这一遍就在写；接着写出那一趟这一遍前头是确认点，要等待确认
    /// （另一半在 [`decide`](Self::decide)），只有答过「后面的卷都写出·继续」的那几卷
    /// 不再问、当场就在写。
    pub fn pass_started(&mut self, pass: Pass, so_far: Option<&VolumeReport>) {
        if let Some(walking) = &mut self.volume {
            walking.pass = Some(pass);
            walking.pass_from = walking.walked;
            if pass == Pass::Second {
                walking.writes = match self.resumes {
                    // 预览走的也是 `Mode::Process`（参照要留着），因此认的是库真收到的那个字：
                    // 用例里 `DryRun` 起而不等人的那一趟，这一遍照旧一个字节都不写。
                    Resuming::GoesOn => self.ran_as == RunMode::Process,
                    Resuming::Waits => self.for_the_rest == Some(Instruction::Continue),
                };
            }
        }
        // 清单上这一卷走到哪个环节；**真停下来问的那一次**是等待确认
        // （见 [`VolumeState::Deciding`]）——与下面等人那一截那格差在它不看 `so_far` 带没带。
        self.set_state(if pass == Pass::Second && self.stops_to_ask() {
            VolumeState::Deciding
        } else {
            VolumeState::Running { pass: Some(pass) }
        });
        if let Some(so_far) = so_far {
            self.summarized = Some(so_far.clone());
            // 确认点那一条报出来的下一刻，观察者就停在闸上了（见 `super::run::Watch`）。
            // 等人那一截从这里起算——但只有真等人的那一趟（见 [`Self::deliberating_since`]）。
            //
            // **答过「后面的卷都写出」之后就不再等**：那一刻起观察者当场照默认答案答字
            // （`super::run::Gate`），没有人在等。这一格照开的话它再也关不上——
            // 关它的只有确认点上的答话，而往下不会再有一次，屏上那两个数于是从此不动。
            if self.stops_to_ask() {
                self.deliberating_since = Some(self.now);
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

    /// 一页失败了，**当场**数上（总览的问题行此刻就说得出，不必等那一卷收摊）。
    /// 那一页连同原因随后在那一卷报告的 `PageOutcome::Failed` 里出现——那一份是结果，这一下是增量。
    pub fn page_failed(&mut self) {
        self.in_flight_failures = self.in_flight_failures.saturating_add(1);
    }

    /// 一卷跑完了，把那一卷的报告接到攒着的这一份上。
    ///
    /// **它那几页坏页从此在报告里**：在途那一格由 [`finish_volume`](Self::finish_volume)
    /// 清零，它们从此由 `Report::failures` 数——两截换手，
    /// [`failures_so_far`](Self::failures_so_far) 的和一格不变。
    ///
    /// **「这一趟真写出过没有」在这里翻面**（[`has_written`](Self::has_written)）：
    /// 收摊的这一卷[在写](Walking::writes)，那一趟就真写出过一卷了。翻面点是**这一条**，
    /// 不是答继续那一帧（那一刻盘上还什么都没有）——[`decide`](Self::decide) 只记
    /// 「这一卷要写了」，写完与否要等它收摊才知道：写出环节里没做成的那一卷走的是
    /// [`volume_failed`](Self::volume_failed)，不算写出过。
    pub fn volume_finished(&mut self, report: &VolumeReport) {
        if self.volume.as_ref().is_some_and(|walking| walking.writes) {
            self.written = true;
        }
        // 三种收摊：跳过（幂等命中）、进了隔离（有坏页）、完成。
        self.set_state(if report.skipped() {
            VolumeState::Skipped
        } else if report.isolated() {
            VolumeState::Isolated
        } else {
            VolumeState::Done
        });
        self.settle(Arc::new(report.clone()));
        self.finish_volume();
    }

    /// 这一卷收摊了：确认点上摆着的那份「到此刻为止」作废。
    ///
    /// 一卷跑完那一条带的是同一卷正式的一份（报告里已经有了），一卷没做成那一条说的是
    /// 它连报告都没有——两种情形下再画那一份都是在画一件已经不成立的事。
    fn summary_is_stale(&mut self) {
        self.summarized = None;
    }

    /// 一整卷没做成：记一笔原因，其余卷照做。
    ///
    /// **它那几页坏页跟着换一格记**（[`lost_failures`](Self::lost_failures)）：
    /// 这一卷没有报告，那几页因此永远进不了 [`report`](Self::report)——不记的话，
    /// 屏上那个「此刻坏了几页」会在这一刻自己往回走。
    pub fn volume_failed(&mut self, volume: &Path, reason: &str) {
        self.lost_failures = self.lost_failures.saturating_add(self.in_flight_failures);
        self.set_state(VolumeState::Failed);
        if let Some(at) = self.index.get(volume)
            && let Some(slot) = self.failed_at.get_mut(*at)
        {
            *slot = Some(self.report.failed_volumes.len());
        }
        self.report.failed_volumes.push(VolumeFailure {
            volume: volume.to_path_buf(),
            reason: reason.to_owned(),
        });
        self.finish_volume();
    }

    /// 这一趟完了，带着它是怎么收的场。
    pub fn run_finished(&mut self, outcome: RunOutcome) {
        self.report.outcome = outcome;
        // 这一趟结束时还开着的那一卷既没收摊也没报没做成：它被立即停止掉了
        // （见 [`VolumeState::Aborted`]）。**它做了多久照样留下**：它没有报告，
        // 屏上那一列只有这一份（见 [`timings`](Self::timings)）。
        self.set_state(VolumeState::Aborted);
        if let (Some(at), Some(began)) = (self.current, self.began)
            && let Some(slot) = self.timings.get_mut(at)
        {
            *slot = self.now.saturating_duration_since(began);
        }
        self.began = None;
        self.current = None;
        self.volume = None;
        // 停在确认点上被立即停止的那一趟从这里出去：那一等到此为止，没有人会来答它。
        self.stop_deliberating();
        // 停在确认点上被立即停止的那一卷不报「一卷跑完」（`Event::VolumeFinished` 的文档）：
        // 它那份「到此刻为止」到这一刻为止也就作废了，而没有别人会来清它。
        self.summary_is_stale();
    }

    /// 一卷收摊：抹掉当前卷那一条，并把它**预告了却没走**的那几步结清到全局那一条上。
    ///
    /// 为什么非结清不可，见 [`tonefit::Event::RunStarted`] 的 `steps`：预告的是上界，
    /// 幂等命中的卷提前收摊——不结清，那条横条就永远走不到头。
    ///
    /// **在途那几页坏页的账在这里清**（[`in_flight_failures`](Self::in_flight_failures)）：
    /// 到这一刻它们的去处已经定了——跑完那一支进了报告，没做成那一支由
    /// [`volume_failed`](Self::volume_failed) 先挪进 [`lost_failures`](Self::lost_failures)。
    fn finish_volume(&mut self) {
        self.summary_is_stale();
        self.in_flight_failures = 0;
        self.finished += 1;
        if let (Some(at), Some(began)) = (self.current, self.began)
            && let Some(slot) = self.timings.get_mut(at)
        {
            *slot = self.now.saturating_duration_since(began);
        }
        self.began = None;
        self.current = None;
        if let Some(walking) = self.volume.take() {
            self.walked = self
                .walked
                .saturating_add(walking.steps.saturating_sub(walking.walked));
        }
    }

    /// 那条线程回来了：把攒出来的报告换成库交出来的那一份，或者记下这一趟没做成。
    ///
    /// 换而不是接着用攒的那一份：两者的差别只有 [`Report::elapsed`]，
    /// 而「这一趟做了多久」只有库那一侧减得掉在确认点上等人的那几分钟（停车场 Q41）。
    pub fn returned(&mut self, done: anyhow::Result<Report>) {
        self.volume = None;
        self.ended = true;
        match done {
            Ok(mut report) => {
                self.settled = std::mem::take(&mut report.volumes)
                    .into_iter()
                    .map(Arc::new)
                    .collect();
                self.report = report;
                self.index_the_settled();
            }
            // 没做成那一趟没有报告：攒到一半的那一份留着，它说得出已经做完的卷。
            Err(error) => self.undone = Some(format!("{error:#}")),
        }
    }

    /// 一卷的报告接到[收摊了的那一列](Self::settled)上，连同查它要的那三格。
    fn settle(&mut self, report: Arc<VolumeReport>) {
        if let Some(at) = self.index.get(&report.volume)
            && let Some(slot) = self.settled_at.get_mut(*at)
        {
            *slot = Some(self.settled.len());
        }
        self.digests
            .push(Digest::of(&report, self.report.profile.panel()));
        self.settled_failures = self
            .settled_failures
            .saturating_add(report.failures().count());
        self.settled.push(report);
    }

    /// 查报告要的那几格从头立一遍：开工那一刻（清单换了）与库交回整份报告那一刻。
    fn index_the_settled(&mut self) {
        let settled = std::mem::take(&mut self.settled);
        self.settled_at = vec![None; self.roster.len()];
        self.digests.clear();
        self.settled_failures = 0;
        for report in settled {
            self.settle(report);
        }
        self.failed_at = vec![None; self.roster.len()];
        for (at, failed) in self.report.failed_volumes.iter().enumerate() {
            if let Some(listed) = self.index.get(&failed.volume)
                && let Some(slot) = self.failed_at.get_mut(*listed)
            {
                *slot = Some(at);
            }
        }
    }

    /// 那条线程回来了没有。
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// **清点中**：线程起了，开工那一条还没到（`CONTEXT.md` 的《总览》：清点中不报卷数）。
    /// 开工那一条报的卷数不会是零——清点出零卷那一趟在它之前就拒绝了。
    #[cfg_attr(
        all(test, not(feature = "tui")),
        allow(dead_code, reason = "只有画法与那条循环读得到，而它们在 tui 特性后面")
    )]
    pub fn surveying(&self) -> bool {
        self.volumes == 0 && !self.ended
    }

    /// 这一趟**没做成**时那句话，做成了就是 `None`。
    pub fn undone(&self) -> Option<&str> {
        self.undone.as_deref()
    }

    /// 报告抬头照哪一种印。
    ///
    /// **预览在答出第一个继续之前印的是 dry-run**，虽然它走的是 `Mode::Process`：
    /// 那条路留参照是为了答继续时分析环节不重算（ADR 0012 决定第 5 条），
    /// 而在确认点上答出继续之前，输出目录一个字节都没有——抬头那一行
    /// 「预览：只分析，不写文件，下面列出的输出路径都还没有写入」正是这时要说的话。
    /// 答了做完再停或立即停止同理：那一趟就此结束，盘上仍旧什么都没有。
    ///
    /// **几十卷的一趟里只要答过一次继续，印的就是执行**：那一卷真写了出去
    /// （见 [`decided`](Self::decided) 那条「取最弱的那一个」）。
    ///
    /// 别的两种（执行、一趟都没跑过）照库收到的那个字印，这一格与从前逐字相同。
    pub fn mode(&self) -> RunMode {
        match (self.resumes, self.decided) {
            // 立即停止那一支眼下到不了：会话在确认点上只答得出继续与做完再停，
            // 立即停止走的是「退出会话」那条路（`Running::leave` 直接推闩，不记这一格）。
            // 仍旧写开，因为 `Instruction` 不非穷尽——多一级的那一天这里编译不过；
            // 而真到了也是同一个答案：那一卷等于没做，盘上一个字节都没有。
            (Resuming::Waits, None | Some(Instruction::Finish | Instruction::Abort)) => {
                RunMode::DryRun
            }
            _ => self.ran_as,
        }
    }

    /// **起手按的是哪一个键**：`t` 起的那一趟是预览，`x` 起的那一趟是执行。
    ///
    /// 与 [`mode`](Self::mode) 差的是**问的时刻**：那一条答「此刻落过盘没有」，
    /// 确认点上答出第一个继续它就翻成执行；这一条答「这一趟是怎么起的」，
    /// 起手那一刻就定死，答什么都不动它。
    ///
    /// **总览抬头说「预览」还是「转换」先按它**（`super::shell::overview`）：`t` 起的那一趟
    /// 在真写出过一卷之前交出来的确实只有判定；之后问的是 [`has_written`](Self::has_written)。
    pub fn started_as(&self) -> RunMode {
        match self.resumes {
            Resuming::Waits => RunMode::DryRun,
            Resuming::GoesOn => self.ran_as,
        }
    }

    /// **这一趟到此刻为止真写出过东西没有**（`no-false-line/04`，收停车场 Q196）。
    ///
    /// 第三个谓词，与另两个各问一个时刻：[`started_as`](Self::started_as) 答「起手按的
    /// 哪一个键」，起手就定死；[`mode`](Self::mode) 答「此刻在写没写」，确认点上答出继续
    /// 那一帧就翻；这一条答「**真写出过没有**」，翻成真的那一刻是**第一卷真写完**
    /// ——[在写的那一卷](Walking::writes)收摊（[`volume_finished`](Self::volume_finished)），
    /// 不是答继续那一帧（那一刻盘上还什么都没有），也不是结束（结束时它必然早已翻过）。
    /// **一趟之内只从假变真一次**，形状与闩相同（`super::run::Latch`）。
    ///
    /// **总览的结论行与问题行按它翻面**：`t` 起的那一趟在此之前给判定分布，之后给完成与跳过、
    /// 隔离几卷（`super::shell::overview`）。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "屏外只有本模块的用例读它，而画法在 tui 特性后面")
    )]
    pub fn has_written(&self) -> bool {
        self.written
    }

    /// 确认点上答过的那个字。还没答、或者这一趟不在那儿停就是 `None`。
    ///
    /// **只给用例用**——屏上要的是它的**后件**（报告抬头照哪一种印，见
    /// [`mode`](Self::mode)），而那一件由那个函数一处答完。
    #[cfg(test)]
    pub fn decided(&self) -> Option<Instruction> {
        self.decided
    }

    /// 记下确认点上答的那个字，以及它[管几卷](Reach)。
    /// **由会话那一头记**（[`super::run::Running::decide`]）：答话的是用户，
    /// 而观察者那一侧只是把它转交给库。
    ///
    /// 记下来的是答过的那几个字里**最弱**的那一个（见 [`decided`](Self::decided)）：
    /// 一趟里每一卷各答一次，而抬头那一行问的是「这一趟落过盘没有」——
    /// 头一卷答了继续、第二卷答了做完再停的那一趟，盘上有头一卷。
    ///
    /// **答的是继续，停在确认点上的这一卷从此在写**（[`Walking::writes`]）：
    /// 「这一趟真写出过没有」要等它收摊才翻面（[`volume_finished`](Self::volume_finished)），
    /// 这里只记下它要写了。
    pub fn decide(&mut self, said: Instruction, reach: Reach) {
        self.decided = Some(match self.decided {
            Some(before) => before.min(said),
            None => said,
        });
        if reach == Reach::ForTheRest {
            self.for_the_rest = Some(said);
        }
        if said == Instruction::Continue
            && let Some(walking) = &mut self.volume
        {
            walking.writes = true;
        }
        // 答的是继续，停在确认点上的那一卷从此在走写出那一遍。**答做完再停不在这里换档**：
        // 那一卷写出环节一步不走、一卷跑完那一条紧跟着到（`tonefit::Pass::Second` 的文档），
        // 收摊那一条把它翻成完成；标成「写出」是假话。
        if said == Instruction::Continue && self.deciding() {
            self.set_state(VolumeState::Running {
                pass: Some(Pass::Second),
            });
        }
        self.stop_deliberating();
    }

    /// 等人那一截收口，累进 [`deliberated`](Self::deliberated)。没在等就什么都不做。
    ///
    /// 两条出路：用户答了话（那条线程接着跑），或者这一趟就此结束
    /// （停在确认点上被立即停止的那一趟走的是这一条，没有人会来答它）。
    fn stop_deliberating(&mut self) {
        if let Some(since) = self.deliberating_since.take() {
            self.deliberated = self
                .deliberated
                .saturating_add(self.now.saturating_duration_since(since));
        }
    }

    /// 至今为止等人等掉的那一截，**含正等着的这一次**（到[此刻](Self::now)为止）。
    ///
    /// 减掉这一截的那一处（[`overall`](Self::overall)）拿的是同一个此刻：从前这里各读各的表，
    /// 减数读得晚一点、因而多算一点，减出来的那个数就小一格——两次读表之间被调度器
    /// 抢走多久，就少多久（停车场 Q118 实测到 299.9999981s < 300s）。
    fn deliberated(&self) -> Duration {
        let waiting = self.deliberating_since.map_or(Duration::ZERO, |since| {
            self.now.saturating_duration_since(since)
        });
        self.deliberated.saturating_add(waiting)
    }

    /// 「后面的卷都写出」摆下的那个默认答案。没答过这个手势就是 `None`。
    ///
    /// 屏上不另说这件事——卷列表上那几卷不再标 `?` 就是回话；读它的是用例
    /// （场景夹具核它与场景数据对不对得上）。
    #[cfg(test)]
    pub fn for_the_rest(&self) -> Option<Instruction> {
        self.for_the_rest
    }

    /// 确认点上那一卷到此刻为止的报告。没停在确认点上就是 `None`。
    pub fn summarized(&self) -> Option<&VolumeReport> {
        self.summarized.as_ref()
    }

    /// 清点清单里第几卷**做了多久**，由会话这一头量的那一份。
    ///
    /// **只在那一卷没有报告时才该问它**：收摊了的卷那个数在它自己那份报告上
    /// （`VolumeTiming::elapsed`，`CONTEXT.md` 的《卷级计时》——只有库那一侧减得掉
    /// 在确认点上等人的那一截）。没做成的卷连一份报告都没有，被立即停止掉的与
    /// 还在跑的那一卷也还没有，而屏上那一列照样要写得出（`CONTEXT.md` 的
    /// 《目录行 / 卷行》：跳过的卷耗时照给）。一步都还没开的卷是 `None`。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn elapsed_at(&self, at: usize) -> Option<Duration> {
        if self.current == Some(at)
            && let Some(began) = self.began
        {
            return Some(self.now.saturating_duration_since(began));
        }
        self.timings.get(at).copied().filter(|one| !one.is_zero())
    }

    /// 清点清单里第几卷**没做成的那一句原因**。没做成之外的卷答 `None`。
    ///
    /// 按**卷根**认——清点已按卷根收编过，清单里卷根不重
    /// （spec《库：开工那一条事件带上清点的产出》）。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法与那条循环读得到，而它们在 tui 特性后面")
    )]
    pub fn undone_at(&self, at: usize) -> Option<&str> {
        let failed = (*self.failed_at.get(at)?)?;
        Some(self.report.failed_volumes.get(failed)?.reason.as_str())
    }

    /// 清点清单里第几卷**那一份报告**：收摊了的、或者确认点上攒着的那一份；没做成的、
    /// 还没轮到的、正在处理的、被立即停止掉的那几卷没有，答 `None`——那正是它们
    /// 展不开的原因（`CONTEXT.md` 的《停得住 / 展得开》）。
    ///
    /// 按**卷根**认，与 [`undone_at`](Self::undone_at) 同一条。卷列表每一行、跳转的落点、
    /// 逐页那几行、总览的判定分布都要它——只此一份。
    pub fn report_at(&self, at: usize) -> Option<&VolumeReport> {
        if let Some(settled) = self.settled_at.get(at).copied().flatten() {
            return self.settled.get(settled).map(|one| &**one);
        }
        let root = &self.roster.get(at)?.root;
        self.summarized.as_ref().filter(|one| one.volume == *root)
    }

    /// 清点清单里第几卷**需留意的页按种类各几页**（`CONTEXT.md` 的《需留意的页》）。
    ///
    /// **判在 [`render::notable`] 一处**，与每页结果、与命令行印出去的那一份同一份判定；
    /// 这里只按种类归堆。**屏上那几个词不在这儿**——措辞是界面层自己的
    /// （`super::shell::list`），本模块照旧只折出几个数（见模块文档头一句）。
    ///
    /// **一处出处**：卷行行尾写哪几样按它（画法那一层，在 `tui` 后面）、`]d`／`[d`
    /// 跳不跳到这一卷也按它（[`Self::troubled_at`]，在特性外面）——两处读的是同一份，
    /// 「行首挂 `!` 的卷跳得到」那句话才成立。
    ///
    /// **收摊了的卷读收摊那一刻判好的那一份**（[`notables`](Self::notables)）；
    /// 确认点上攒着的那一份还会换，当场判。
    pub fn notable_at(&self, at: usize) -> NotableTally {
        self.digest_at(at)
            .map_or_else(NotableTally::default, |digest| digest.notable)
    }

    /// 清点清单里第几卷那一份报告**折出来的那几个数**（[`Digest`]）；没有报告是 `None`。
    ///
    /// **收摊了的卷读收摊那一刻折好的那一份**；确认点上攒着的那一份还会换，当场折。
    pub fn digest_at(&self, at: usize) -> Option<Cow<'_, Digest>> {
        if let Some(settled) = self.settled_at.get(at).copied().flatten() {
            return self.digests.get(settled).map(Cow::Borrowed);
        }
        let report = self.report_at(at)?;
        Some(Cow::Owned(Digest::of(report, self.report.profile.panel())))
    }

    /// 清点清单里第几卷**是 `]d`／`[d` 的一个落点**吗（`CONTEXT.md` 的《卷列表》：
    /// 转换失败的卷 · 进了隔离的卷 · 有需留意的页的卷）。
    ///
    /// 它与**行首记号**那两个（`✗`／`!`）是同一批卷：`]d` 跳的正是屏上跳出来的那几行。
    /// 还没轮到、正在处理、被立即停止掉的那几卷一份报告都没有，判不出需留意几页，
    /// 因此一个都不是落点——那几行屏上也没有记号可跳。
    ///
    /// 第四种落点（**无法访问的地方**）不是卷，它是[备注行](super::tree::Note)，
    /// 由跳转那一支在树上认（`super::view::Session::jump`）。
    pub fn troubled_at(&self, at: usize) -> bool {
        match self.states.get(at) {
            Some(VolumeState::Failed | VolumeState::Isolated) => true,
            Some(VolumeState::Done) => self.notable_at(at).any(),
            _ => false,
        }
    }

    /// 攒到此刻的报告，**整份**：收摊了的那几卷当场拼回去（见 [`settled`](Self::settled)）。
    ///
    /// 拼一份要把每一卷每一页拷一遍——只给退出时印报告、算退出码那几处用，画一帧不走它。
    pub fn report(&self) -> Report {
        Report {
            volumes: self.settled.iter().map(|one| (**one).clone()).collect(),
            ..self.report.clone()
        }
    }

    /// 这一趟的阅读器面板（报告抬头上那个型号的）。画法判需留意的页要它，不必拼整份报告。
    pub fn panel(&self) -> tonefit::Panel {
        self.report.profile.panel()
    }

    /// 这一趟是怎么收的场（报告上那一格）。还没收场时是开工那一刻填的初值。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn outcome(&self) -> RunOutcome {
        self.report.outcome
    }

    /// **卷清单**：开工那一条带的那一列，照发现的次序（`session-redesign/03`）。
    /// 开工之前、或那一条没带清单时是空的。
    ///
    /// 下面四个访问器眼下只有本模块的用例读：画卷列表那棵树那一票接上读者时把那一行
    /// `expect` 拆掉——留着它会当场报「这个 `expect` 没用上」。
    pub fn roster(&self) -> &[SurveyedVolume] {
        &self.roster
    }

    /// 清单上每一卷此刻怎么样，**与 [`roster`](Self::roster) 同序**。
    pub fn states(&self) -> &[VolumeState] {
        &self.states
    }

    /// 开工那一条带的非漫画文件那张表。与这一趟跑完之后报告上那一张逐条相同。
    pub fn non_volume_files(&self) -> &[NonVolumeFile] {
        &self.non_volume_files
    }

    /// 开工那一条带的无法访问的地方那张表。与这一趟跑完之后报告上那一张逐条相同。
    pub fn unreachable_places(&self) -> &[UnreachablePlace] {
        &self.unreachable_places
    }

    /// 总览块要的那几个数：第几卷 / 共几卷、走了几步 / 共几步、已用多久、还剩多久。
    ///
    /// **结束之后「已用」就定住了**：那时用的是库交出来的 [`Report::elapsed`]——
    /// 它是这一趟真做了多久，扣掉了在确认点上等人的那几分钟（停车场 Q41）。
    /// 接着读自己那块表的话，跑完坐着不动，屏上那个数会一路往上涨。
    pub fn overall(&self) -> Overall {
        let elapsed = if self.ended {
            self.report.elapsed
        } else {
            // 减掉在确认点上等人的那一截（停车场 Q41）：库在那段里一步都没走，
            // 算进来的话「剩多久」说的就成了「用户拿主意还要多久」。
            // 结束之后换成库交出来的那一个——它减的是同一件事，只是准到纳秒。
            //
            // 被减数与减数都从同一个[此刻](Self::now)起算（停车场 Q118 那一格的差
            // 出自各读各的表，见 [`deliberated`](Self::deliberated)）。
            self.now
                .saturating_duration_since(self.started)
                .saturating_sub(self.deliberated())
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

    /// **到此刻为止坏了几页**：报告里那几页，加上报告收不了的那两截——
    /// [没做成的卷带走的](Self::lost_failures)与[当前这一卷在途的](Self::in_flight_failures)。
    ///
    /// 三截拼出来的不是三个出处：本模块开头那句「事件流就是报告的增量」说的正是这一件——
    /// 一卷跑完，它那几页从在途挪进报告，三截的和一格不变。
    ///
    /// **与 `Report::failures().count()` 故意不是一个数**：那一个答「已定案的那几卷坏了几页」
    /// （报告末尾那几小结数的正是它），这一条答「**此刻**坏了几页」——总览块的出事行要的
    /// 是后一个（停车场 Q148）。**这个数只涨不落**：一卷收摊时那几截只是换手。
    pub fn failures_so_far(&self) -> usize {
        self.settled_failures
            .saturating_add(self.lost_failures)
            .saturating_add(self.in_flight_failures)
    }

    /// 这一趟的退出码，**与命令行那一路同一套**：拒绝开始是 `1`，
    /// 其余交给 [`crate::exit_code`]——全部成功 `0`、有卷被隔离 `2`、
    /// 有卷没做成**或有地方无法访问** `3`。
    ///
    /// 还没跑完时问它没有意义，那时给的是「照现在这份报告结束会是几」——
    /// 会话只在退出那一刻问一次，而那时这一趟一定已经收了场。
    pub fn exit_code(&self) -> u8 {
        match self.undone {
            Some(_) => crate::REFUSED_EXIT,
            None => crate::exit_code(&self.report()),
        }
    }
}

/// 总览要的几个数（抬头与总进度那一行分着用，见 `super::shell::overview`）。
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

/// 一卷报告**折出来的那几个数**（[`Live::digest_at`]）：卷行行尾、目录行的汇总、`]d` 的落点与总览那两行
/// 每一帧都要，而折一次要逐页走一遍——收摊了的卷折一次存着（`session-redesign/17`）。
///
/// **判在 [`render::notable`] 一处**，与每页结果、与命令行印出去的那一份同一份判定；
/// 灰阶分布取自 [`render::tally_pairs`]。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Digest {
    /// 需留意的页按种类各几页（[`Live::notable_at`]）。
    pub notable: NotableTally,
    /// 灰阶分布：每个档位几页（总览的结论行）。
    pub tally: Vec<(tonefit::Candidate, usize)>,
    /// 带着差异大的那几页（总览的问题行，预览那一副），按页数。
    pub outlier_pages: usize,
    /// 带着页面超宽的那几页，同上。
    pub wide_pages: usize,
    /// 坏了几页（`VolumeReport::failures`；目录行的汇总）。
    pub failed_pages: usize,
}

impl Digest {
    fn of(report: &VolumeReport, panel: tonefit::Panel) -> Self {
        let mut digest = Self {
            tally: render::tally_pairs(report),
            failed_pages: report.failures().count(),
            ..Self::default()
        };
        for page in render::notable(report, panel) {
            digest.outlier_pages += usize::from(page.contains(&render::Notable::Outlier));
            digest.wide_pages += usize::from(page.contains(&render::Notable::Overflowed));
            for why in page {
                let tally = &mut digest.notable;
                match why {
                    render::Notable::Outlier => tally.outlier += 1,
                    render::Notable::Overflowed => tally.overflowed += 1,
                    render::Notable::OutsideTheGate => tally.outside_the_gate += 1,
                    render::Notable::Salvaged => tally.salvaged += 1,
                    // 坏页由**行首记号**与隔离那一句说，代表页与兜底上界不是「出了事」。
                    render::Notable::Failed
                    | render::Notable::Backstopped
                    | render::Notable::Driver => {}
                }
            }
        }
        digest
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    //! 报告的夹具。画法那一侧、场景夹具与本模块的用例共用它——
    //! 几处要的是同一份东西，各搓一份就会在改动时走散。
    //!
    //! [`a_real_volume`] 是里面唯一**落到盘上**的一个：真起一条线程跑一趟的那几条用例
    //! （`super::super::run`、`super::super::terminal`）共用它。

    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, CandidateScore, ChosenBy, Crop, Dither,
        Envelope, GeometryGate, GrayImage, IoPlan, Medium, Mode as RunMode, PageBranch, PageColor,
        PageOutcome, PageReport, Processed, Profile, Readers, Reason, Reference, Request, Salvage,
        Scaling, Size, SurveyedVolume, Verdict, VolumeReport, VolumeTiming, VolumeVerdict,
        WhiteAlignment,
    };

    use super::{Live, Resuming};

    /// 在 `root` 底下摆一个叫 `name` 的、真跑得动的卷：**一页加一个透传文件**。
    /// 建出目录，返回卷根。
    ///
    /// 页非有不可：一页都没有的东西不是卷（ADR 0014 决定第 3 条），清点当场把它丢掉，
    /// 那条线程于是根本走不到确认点。从前这几条用例只摆一个透传文件，为的是不必造图片。
    ///
    /// 透传文件仍留着：写出环节写的是**全部成员**，它也在里面
    /// （`tests/resume.rs` 的 `small_volume` 特意留一个透传成员正是这个理由），
    /// 因此「写没写出去」在它身上看得见。
    ///
    /// 页是这批用例里最便宜的一张：高恰是基准面板的高，纯墨到边——两样都是为了让管线
    /// 在它身上不做工作（不缩放、裁白边一个像素都拿不走）。这几条问的是线程、闩与确认点，
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
            white_align_limit: tonefit::WhiteAlignLimit::default(),
            bit_depth: None,
            dither: None,
            envelope: false,
            cache_budget: CacheBudget::default(),
            mode,
            io_mode: tonefit::IoMode::default(),
            metadata: true,
            progress: None,
        }
    }

    /// 一趟**此刻在 `epoch`** 的会话状态：造出来当场给一次「此刻」（[`Live::tick`]）。
    ///
    /// 给定「此刻」的用例一律从它起。开工那一条之前那一次 `tick` 不能省（见 `Live::tick`），
    /// 而漏掉它没有一张快照会红——只在秒的进位上偶尔差一格。这里把「造」与「给」
    /// 合成一步，漏不掉。`epoch` 取什么都行，往后的时刻都从它往上加。
    pub fn live_at(epoch: Instant, mode: RunMode, resumes: Resuming) -> Live {
        live_for(epoch, &request(mode), resumes)
    }

    /// 同 [`live_at`]，抬头那几件事照**给定的那份参数**印：场景夹具（`super::super::scene`）
    /// 把三组设置拼成的 `Request` 交进来，报告抬头因此说的是场景数据里的型号与选项。
    /// [`Live::new`] 读一次系统时钟作初值（停车场 Q754），紧跟着的那一次 `tick` 把它盖掉——
    /// **给定「此刻」的夹具**一律走这两个函数之一（不问时钟的那几条用例照旧直接 `Live::new`）。
    pub fn live_for(epoch: Instant, request: &Request, resumes: Resuming) -> Live {
        let mut live = Live::new(request, resumes);
        live.tick(epoch);
        live
    }

    /// 一卷做了这么久。
    ///
    /// 夹具里给一个**非零**的数：卷列表耗时那一列问的正是它，而「跳过一卷为什么也要等这么久」
    /// 只有这个数答得出来（`VolumeTiming::elapsed`）。三份夹具各给各的，快照上分得开；
    /// 场景夹具（`super::super::scene`）给的是场景数据里那一卷的秒数。
    pub(crate) fn took(elapsed: Duration) -> VolumeTiming {
        VolumeTiming {
            elapsed,
            ..VolumeTiming::default()
        }
    }

    /// 一卷收摊，**两半一起喂**：那一卷报告里的坏页先各报一条
    /// [`Live::page_failed`](super::Live::page_failed)，再报那一卷跑完。
    ///
    /// 真跑一趟时一页失败**发两回话**：出现的当场一条事件，那一页随后又在卷报告的
    /// [`PageOutcome::Failed`] 里出现一次（本模块开头那句「事件流就是报告的增量」——
    /// 一份是增量，一份是结果）。直接调
    /// [`Live::volume_finished`](super::Live::volume_finished) 只喂得到后一半，
    /// 夹具于是摆得出一副**真会话里到不了的**计数：在途那一格从没数上过那几页。
    /// 页数逐一取自那一份卷报告，两半因此不会走散。
    ///
    /// **本模块自己那几条用例是例外**，它们照旧两半分开报：
    /// [`super::tests::the_failed_pages_of_the_volume_in_flight_count_towards_now`] 问的正是
    /// 「在途那一格与报告那一格换手时和变不变」，喂成一体就问不出来了。
    ///
    /// **「此刻坏了几页」不会因此变大**：那一卷收摊时在途那几页只是换手进报告
    /// （见 [`Live::failures_so_far`](super::Live::failures_so_far)），和一格不动。
    ///
    /// 场景夹具（`super::super::scene`）收摊每一卷也走它，两趟闸门因此都有读者。
    pub fn volume_finished_with_its_failures(live: &mut super::Live, report: &VolumeReport) {
        for page in report.failures() {
            if let PageOutcome::Failed { .. } = &page.outcome {
                live.page_failed();
            }
        }
        live.volume_finished(report);
    }

    /// 一份**卷清单**：`names` 那几卷，卷根在 `库/` 底下——与 [`skipped_volume`]、
    /// [`processed_volume`] 那几份卷报告的卷路径同一个写法，开卷那一条按卷根认回清单里
    /// 的那一卷靠的正是这一点。步数与源页数各一个固定的数：状态那几条用例不问它们。
    pub fn roster<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<SurveyedVolume> {
        names
            .into_iter()
            .map(|name| SurveyedVolume {
                root: PathBuf::from(format!("库/{name}")),
                steps: 1000,
                source_pages: 20,
            })
            .collect()
    }

    /// 一份**幂等命中**的卷报告：一页都没重做，逐页结果因此一条都没有。
    ///
    /// 快照要的正是这一种——它不必搓画质分、候选与尺寸贴合检查，而「跳过说清是哪四项依据没变」
    /// 与「这一趟怎么读的」两条验收都落在它身上。
    pub fn skipped_volume(name: &str, page_count: usize) -> VolumeReport {
        VolumeReport {
            volume: PathBuf::from(format!("库/{name}")),
            output: PathBuf::from(format!("出/{name}")),
            superseded: None,
            pages: Vec::new(),
            retained_pages: 0,
            source_pages: page_count,
            verdict: Some(VolumeVerdict::Skipped { page_count }),
            cache: cache_usage(),
            extracted: 0,
            io: io_plan(),
            decodes: 0,
            resizes: 0,
            cached_references: 0,
            timing: took(Duration::from_secs(3)),
        }
    }

    /// 一份**真做过事**的卷报告：一页完好的灰度页定出卷级统一档位。
    ///
    /// `broken` 给一句原因就再添一张坏页，那时整卷进隔离目录。
    /// 「一卷跑完当场显示它的判定与代表页」与「坏页带原因」两条验收落在它身上——
    /// 代表页指的就是那一页完好的。
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
                    white: WhiteAlignment::Off,
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
            retained_pages: 0,
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
            resizes: 1,
            cached_references: 1,
            timing: took(Duration::from_secs(72)),
        }
    }

    /// 一份**每一种页各一张**的卷报告：八页，其中[要紧的](crate::render::notable)六页。
    ///
    /// 每页结果那几条要的正是这一种（`p3-session-legibility/11`）：默认那一副与全部页
    /// 那一副要看得出差别，而「要紧」那六种要在同一卷里各出现一次。
    ///
    /// | 页 | 它要紧在哪儿 |
    /// |---|---|
    /// | `001` | 不要紧：判定跟着卷级统一档位走 |
    /// | `002` | 不要紧：走**彩色分支**，只缩放、不量化，也不进整卷统一灰阶 |
    /// | `003` | **代表页**（整卷统一灰阶站在它身上） |
    /// | `004` | **差异大的页**：画质分偏离卷内分布，单独定档；它同时**页面超宽** |
    /// | `005` | **尺寸未贴合屏幕**：源比目标小，抖动单独关掉 |
    /// | `006` | **兜底上界**：目标尺寸退回过 fit-inside |
    /// | `007` | **残缺**：解到哪个像素算哪个像素，行尾说得出救回了多少 |
    /// | `017` | **坏页**：这一页根本没解出来 |
    ///
    /// **「一页同时要紧在好几处」由 `004` 撑着**（特例加页面超宽），而不是拿兜底上界配页面超宽：
    /// 那一对**凑不到一起**——退回之后的页恒不超过面板宽
    /// （`Report::backstopped`：两张清单不重叠）。
    ///
    /// **页的三种状态在这一卷里都有**（完好、残缺、失败），彩色分支那一条也在：
    /// 坏页说得出它的尺寸是**卷内统一尺寸**、彩页说得出它不量化也不进整卷统一灰阶，
    /// 而这两句话只有逐页那几行说得出来——卷级那几行一句都没有（`p1-session/11` 的验收）。
    ///
    /// 与 [`processed_volume`] 分开而不是给它加几页：那一份钉着卷级那几张快照
    /// （`p1-session/09` 录的），添一页就要跟着重录。
    #[cfg_attr(
        not(feature = "tui"),
        allow(
            dead_code,
            reason = "只有画法那一侧的用例用得着，而画法在 tui 特性后面"
        )
    )]
    pub fn a_page_of_every_kind(name: &str) -> VolumeReport {
        let base = Candidate::new(BitDepth::Four, Dither::Off);
        let source = Size::new(1441, 2048);
        let target = Size::new(1182, 1680);
        // 面板宽 1264（`kobo-libra-2`，见 [`request`]）：这一张比它宽，因此页面超宽。
        let wide = Size::new(1600, 1680);
        let gray = |gate: GeometryGate, verdict: Verdict| PageBranch::Gray {
            white: WhiteAlignment::Off,
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
            // 彩色分支：只缩放，不量化，不进灰度缓存也不进整卷统一灰阶——它不要紧，
            // 但全部页那一副上要看得见它那一句。
            page("002", target, false, PageBranch::Color),
            // 代表页：这一卷的统一档位就是它判出来的（`Envelope::driver` 指着它）。
            page(
                "003",
                target,
                false,
                gray(
                    GeometryGate::Holds,
                    judged(base, Reason::LowestWithinThreshold),
                ),
            ),
            // 差异大的页，而且它**同时页面超宽**：不参与整卷统一灰阶、按它自己那一档写出，
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
            // 尺寸未贴合屏幕：抖动单独关掉，灰阶档位仍跟着统一档位。
            page(
                "005",
                target,
                false,
                gray(GeometryGate::Broken, judged(base, Reason::OutsideTheGate)),
            ),
            // 兜底上界退回过：它没按这一趟点名的缩放方式出。**退回之后恒不超过面板宽**
            // （`Report::backstopped`：与页面超宽那张清单不重叠），因此它拿的是普通尺寸。
            page(
                "006",
                target,
                true,
                gray(GeometryGate::Holds, judged(base, Reason::VolumeEnvelope)),
            ),
        ];
        // 残缺：它有自己的尺寸、画质分与判定，却不替整卷说话。
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
            retained_pages: 0,
            source_pages: pages.len(),
            // 其余页那一组是 `001`、`003`、`006` 三张：彩页、特例、未贴合屏幕、
            // 残缺、失败五张都在进这一层之前被摘走了（见 `Envelope::body_pages`）。
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
            resizes: 7,
            cached_references: 6,
            timing: took(Duration::from_secs(96)),
        }
    }

    /// 一个画质分值。从公开 seam 上真算一个——摆一个编出来的数上去，
    /// 快照就钉不住「报告说的是画质分算出来的东西」。
    fn a_score() -> tonefit::Score {
        a_score_of(136)
    }

    /// 一个画质分值，深浅由 `shade` 定。候选各不相同的那几个数由它来。
    fn a_score_of(shade: u8) -> tonefit::Score {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        tonefit::score(
            &reference,
            &GrayImage::new(Size::new(1, 1), vec![shade]),
            // 编出来的 1×1，没经过目标灰阶档位量化：取工作精度那一档（`metric::score` 的文档）。
            BitDepth::Eight,
        )
    }

    /// 一页上各候选各一个数，档位由低到高——**逐页那一行印的就是这一串**
    /// （`render::score_line`）。
    #[cfg_attr(
        not(feature = "tui"),
        allow(
            dead_code,
            reason = "只有画法那一侧的用例用得着，而画法在 tui 特性后面"
        )
    )]
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
    /// 场景夹具造的每一份卷报告也用它——场景数据里没有这一格，屏上也没有一处画它。
    pub(crate) fn io_plan() -> IoPlan {
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

    /// 一份缓存用量，与 [`io_plan`] 同一个待遇：场景数据里没有、屏上不画。
    pub(crate) fn cache_usage() -> CacheUsage {
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

    /// **卷清单上每一卷的状态跟着事件走**（`session-redesign/03`）：一趟里跳过、完成、
    /// 进了隔离、没做成、等待确认、被立即停止掉各至少一卷，还没轮到的仍是等待中，
    /// 逐卷对得上。
    ///
    /// 清单上的身份从开工那一刻就有（清单里的第几卷），此后每一条事件只改**它此刻怎么样**：
    /// 开卷翻成处理中、某一遍开工记下走到哪个环节、确认点上等人是等待确认、答了话回到处理中、
    /// 收摊按那一卷报告分成完成／跳过／进了隔离、没做成是那一条、这一趟结束时还开着的那一卷
    /// 是被立即停止掉的。
    #[test]
    fn every_volume_on_the_roster_has_a_state_that_follows_the_events() {
        use VolumeState::{Aborted, Deciding, Done, Failed, Isolated, Queued, Running, Skipped};

        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        let roster = fixture::roster(["卷一", "卷二", "卷三", "卷四", "卷五", "卷六"]);
        live.run_started(6, 6000);
        live.surveyed(&roster, &[], &[]);
        assert_eq!(live.roster(), roster, "清单没原样留下");
        assert_eq!(live.states(), [Queued; 6], "开工那一刻每一卷都该是等待中");

        // 卷一：幂等命中，跳过。
        live.volume_started(Path::new("库/卷一"), 1000);
        assert_eq!(live.states()[0], Running { pass: None });
        live.pass_started(Pass::Fingerprint, None);
        assert_eq!(
            live.states()[0],
            Running {
                pass: Some(Pass::Fingerprint)
            }
        );
        live.volume_finished(&fixture::skipped_volume("卷一", 20));
        assert_eq!(live.states()[0], Skipped);

        // 卷二：走到确认点等人，答继续，写完。
        live.volume_started(Path::new("库/卷二"), 1000);
        live.pass_started(Pass::First, None);
        let so_far = fixture::processed_volume("卷二", None);
        live.pass_started(Pass::Second, Some(&so_far));
        assert_eq!(
            live.states()[1],
            Deciding,
            "停在确认点上的那一卷该是等待确认"
        );
        live.decide(Instruction::Continue, Reach::ThisVolume);
        assert_eq!(
            live.states()[1],
            Running {
                pass: Some(Pass::Second)
            },
            "答了话就不再是等待确认"
        );
        live.volume_finished(&so_far);
        assert_eq!(live.states()[1], Done);

        // 卷三：有坏页，进了隔离。
        live.volume_started(Path::new("库/卷三"), 1000);
        live.volume_finished(&fixture::processed_volume("卷三", Some("解不出完整尺寸")));
        assert_eq!(live.states()[2], Isolated);

        // 卷四：整卷没做成。
        live.volume_started(Path::new("库/卷四"), 1000);
        live.volume_failed(Path::new("库/卷四"), "盘拔了");
        assert_eq!(live.states()[3], Failed);

        // 卷五：停在确认点上时被立即停止；卷六还没轮到。
        live.volume_started(Path::new("库/卷五"), 1000);
        let so_far = fixture::processed_volume("卷五", None);
        live.pass_started(Pass::Second, Some(&so_far));
        assert_eq!(
            live.states(),
            [Skipped, Done, Isolated, Failed, Deciding, Queued]
        );
        live.run_finished(RunOutcome::Stopped(Instruction::Abort));
        assert_eq!(
            live.states(),
            [Skipped, Done, Isolated, Failed, Aborted, Queued],
            "这一趟结束时还开着的那一卷是被立即停止掉的，没轮到的仍是等待中"
        );
    }

    /// **开卷那一条按卷根认回清单里的那一卷**，不是按「轮到第几个」——清点已按卷根收编过，
    /// 清单里卷根不重（`session-redesign/03`）。
    ///
    /// 「后面的卷都写出」之后的确认点**不是**等待确认：那一刻观察者当场照默认答案答字，
    /// 没有人在等——与等人那一截那格同一个判据。
    #[test]
    fn a_volume_is_recognised_on_the_roster_by_its_root() {
        use VolumeState::{Queued, Running};

        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.run_started(3, 3000);
        live.surveyed(&fixture::roster(["卷一", "卷二", "卷三"]), &[], &[]);

        live.volume_started(Path::new("库/卷三"), 1000);
        assert_eq!(live.states(), [Queued, Queued, Running { pass: None }]);

        // 答过「后面的卷都写出·继续」：往下的确认点不停，那一卷仍是处理中。
        live.decide(Instruction::Continue, Reach::ForTheRest);
        let so_far = fixture::processed_volume("卷三", None);
        live.pass_started(Pass::Second, Some(&so_far));
        assert_eq!(
            live.states()[2],
            Running {
                pass: Some(Pass::Second)
            },
            "不再停下来问的确认点不该是等待确认"
        );
    }

    /// **两张表在清点一到就拿得到**（`session-redesign/03`）：非漫画文件与无法访问的地方
    /// 不必等这一趟跑完。**报告上那两张照旧要等跑完**（停车场 Q745、Q964）。
    #[test]
    fn the_two_tables_are_at_hand_the_moment_the_survey_arrives() {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(1, 1000);
        live.surveyed(
            &fixture::roster(["卷一"]),
            &[tonefit::NonVolumeFile {
                path: PathBuf::from("库/字体包.zip"),
                reason: tonefit::NonVolumeReason::ArchiveWithoutAPage,
            }],
            &[tonefit::UnreachablePlace {
                path: PathBuf::from("库/私藏"),
                reason: "列出 库/私藏 这一层: Permission denied (os error 13)".to_owned(),
            }],
        );

        assert_eq!(live.non_volume_files().len(), 1, "非漫画文件没收下");
        assert_eq!(live.unreachable_places().len(), 1, "无法访问的地方没收下");
        assert!(
            live.report().non_volume_files.is_empty()
                && live.report().unreachable_places.is_empty(),
            "报告上那两张表在跑完之前就填上了（Q745）"
        );
        assert!(live.report().volumes.is_empty(), "一卷都还没收摊");
    }

    /// 一趟走完：全局那几个数、当前卷那一条、攒下来的报告，逐条对得上。
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

    /// **「此刻坏了几页」把当前这一卷也算上**（停车场 Q148）：报告只数收摊了的卷，
    /// 而总览块的出事行答的是此刻。
    ///
    /// 三段各问一遍：那一卷还在跑时两个数**故意**不一样、它收摊之后又相等、
    /// 下一卷再坏一页时又分开。**不许把同一页数两遍**，也**不许往回走一格**——
    /// 末一段问的正是后者：一卷没做成时它那几页没有任何一份报告收着，减掉就等于
    /// 让屏上那个数自己缩回去（评审提的）。
    #[test]
    fn the_failed_pages_of_the_volume_in_flight_count_towards_now() {
        const BROKEN: &str = "解不出完整尺寸：JPEG 数据截断";

        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(2, 8);
        live.volume_started(Path::new("库/卷一"), 4);
        live.page_failed();

        assert_eq!(live.failures_so_far(), 1, "当前这一卷坏的那一页没数上");
        assert_eq!(
            live.report().failures().count(),
            0,
            "报告不该数还没收摊的卷"
        );

        // 那一卷收摊：同一页此刻在报告里，两个数因此相等——而不是变成两页。
        live.volume_finished(&fixture::processed_volume("卷一", Some(BROKEN)));
        assert_eq!(live.report().failures().count(), 1);
        assert_eq!(live.failures_so_far(), 1, "同一页数了两遍");

        // 下一卷又坏一页，而这一卷**整卷没做成**：它连一份卷报告都没有，那一页因此
        // 一辈子进不了报告——而它确实坏了。这个数不许因为那一卷废掉就往回走一格。
        live.volume_started(Path::new("库/卷二"), 4);
        live.page_failed();
        assert_eq!(live.failures_so_far(), 2);
        assert_eq!(live.report().failures().count(), 1);

        live.volume_failed(Path::new("库/卷二"), "写不出去");
        assert_eq!(
            live.failures_so_far(),
            2,
            "那一卷废了，坏过的页跟着从屏上消失"
        );
        assert_eq!(
            live.report().failures().count(),
            1,
            "没做成的卷不进报告正文"
        );

        // 再下一卷收摊：它自己那一页照数，前面那两页一格不动。
        live.volume_started(Path::new("库/卷三"), 4);
        live.volume_finished(&fixture::processed_volume("卷三", Some(BROKEN)));
        assert_eq!(live.failures_so_far(), 3);
    }

    /// **确认点上答出继续，`mode` 翻面而「起手按的哪一个键」一格不动**（停车场 Q149）。
    ///
    /// 两条答的不是同一个问题：`mode` 答「此刻落过盘没有」（报告抬头与总览块的抬头走它），
    /// `started_as` 答「这一趟是怎么起的」（总览块那两行在第一卷真写完之前走它，一趟之内一格不变；
    /// 之后走 `has_written`，见 [`the_run_has_written_once_the_first_volume_it_wrote_is_finished`]）。
    #[test]
    fn answering_at_a_decision_point_moves_the_mode_but_not_what_the_run_started_as() {
        let mut trial = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        assert_eq!(trial.mode(), RunMode::DryRun);
        assert_eq!(trial.started_as(), RunMode::DryRun);

        trial.decide(Instruction::Continue, Reach::ThisVolume);
        assert_eq!(trial.mode(), RunMode::Process, "那一卷真写了出去");
        assert_eq!(
            trial.started_as(),
            RunMode::DryRun,
            "起手那一副不该跟着翻面"
        );

        // 执行那一趟两条恒是同一个答案：它在确认点上不停，一起手就在写。
        let processing = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        assert_eq!(processing.mode(), RunMode::Process);
        assert_eq!(processing.started_as(), RunMode::Process);
    }

    /// **「真写出过没有」在第一卷真写完那一刻翻面：不在答继续那一帧，也不为答做完再停与跳过的卷翻**
    /// （`no-false-line/04`，收停车场 Q196）。
    ///
    /// 三个谓词一路对着问：`started_as` 一格不动，`mode` 在答继续那一帧就翻，
    /// `has_written` 要等那一卷收摊——它们各答一个时刻，差的正是这一帧。
    #[test]
    fn the_run_has_written_once_the_first_volume_it_wrote_is_finished() {
        let mut trial = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        trial.run_started(3, 3000);
        assert!(!trial.has_written(), "还没开卷就说写出过了");

        // 头一卷幂等命中：到不了确认点，一个字节都没写。
        trial.volume_started(Path::new("库/卷一"), 1000);
        trial.volume_finished(&fixture::skipped_volume("卷一", 180));
        assert!(!trial.has_written(), "跳过的卷算成写出过了");

        // 第二卷停在确认点上，答做完再停：这一卷等于走了一次预览。
        trial.volume_started(Path::new("库/卷二"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷二", None)));
        trial.decide(Instruction::Finish, Reach::ThisVolume);
        trial.volume_finished(&fixture::processed_volume("卷二", None));
        assert!(!trial.has_written(), "答了做完再停的那一卷一个字节都没写");

        // 第三卷答继续：答话那一帧盘上还什么都没有，收摊那一刻才写完。
        trial.volume_started(Path::new("库/卷三"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷三", None)));
        trial.decide(Instruction::Continue, Reach::ThisVolume);
        assert_eq!(trial.mode(), RunMode::Process, "此刻在写");
        assert!(
            !trial.has_written(),
            "答继续那一帧就翻面了——正是 Q149 要拦的那一帧"
        );
        trial.volume_finished(&fixture::processed_volume("卷三", None));
        assert!(trial.has_written(), "第一卷真写完了，却还说没写出过");
        assert_eq!(trial.started_as(), RunMode::DryRun, "起手那一副不该跟着翻");
    }

    /// **只升不降**：翻成真之后，再答做完再停、再来一卷跳过的、结束，都不动它——
    /// 一趟之内只从假变真一次，屏上那一格因此不来回跳。
    #[test]
    fn having_written_is_a_latch() {
        let mut trial = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        trial.run_started(3, 3000);
        trial.volume_started(Path::new("库/卷一"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷一", None)));
        trial.decide(Instruction::Continue, Reach::ThisVolume);
        trial.volume_finished(&fixture::processed_volume("卷一", None));
        assert!(trial.has_written());

        trial.volume_started(Path::new("库/卷二"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷二", None)));
        trial.decide(Instruction::Finish, Reach::ThisVolume);
        trial.volume_finished(&fixture::processed_volume("卷二", None));
        assert!(
            trial.has_written(),
            "答了一次做完再停，写出过的那一格缩回去了"
        );

        trial.volume_started(Path::new("库/卷三"), 1000);
        trial.volume_finished(&fixture::skipped_volume("卷三", 180));
        trial.run_finished(RunOutcome::Completed);
        trial.returned(Ok(trial.report().clone()));
        assert!(trial.has_written(), "结束把写出过的那一格抹掉了");
    }

    /// **答了继续却没做成的那一卷不算写出过**：写出环节里废掉的卷走的是 `VolumeFailed`，
    /// 盘上没有它。「后面的卷都写出·继续」之后的那几卷不再问，走到写出那一遍
    /// 当场就在写——头一卷写出来就翻面。
    ///
    /// 执行那一趟每一卷都写，头一卷收摊就翻；用例里 `DryRun` 起而不等人的那一趟一个字节
    /// 都不写，跑完也不翻。
    #[test]
    fn a_volume_that_failed_while_being_written_does_not_count_as_written() {
        let mut trial = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        trial.run_started(3, 3000);
        trial.volume_started(Path::new("库/卷一"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷一", None)));
        trial.decide(Instruction::Continue, Reach::ForTheRest);
        trial.volume_failed(Path::new("库/卷一"), "写不出去");
        assert!(!trial.has_written(), "写出环节里废掉的那一卷算成写出过了");

        // 往下不再问：走到写出那一遍就在写。
        trial.volume_started(Path::new("库/卷二"), 1000);
        trial.pass_started(Pass::Second, Some(&fixture::processed_volume("卷二", None)));
        assert!(!trial.has_written(), "这一卷还没收摊");
        trial.volume_finished(&fixture::processed_volume("卷二", None));
        assert!(
            trial.has_written(),
            "「后面的卷都写出」之后写出的那一卷没让它翻面"
        );

        let mut processing = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        processing.run_started(1, 1000);
        processing.volume_started(Path::new("库/卷一"), 1000);
        processing.pass_started(Pass::Second, None);
        processing.volume_finished(&fixture::processed_volume("卷一", None));
        assert!(processing.has_written(), "转换那一趟头一卷收摊就该翻面");

        let mut dry = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        dry.run_started(1, 1000);
        dry.volume_started(Path::new("库/卷一"), 1000);
        dry.pass_started(Pass::Second, None);
        dry.volume_finished(&fixture::processed_volume("卷一", None));
        assert!(!dry.has_written(), "只算不写的那一趟说自己写出过了");
    }

    /// 退出码与命令行那一路一致：拒绝开始 `1`，有卷被隔离 `2`，全部成功 `0`。
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
        let epoch = Instant::now();
        let mut live = fixture::live_at(epoch, RunMode::Process, Resuming::GoesOn);
        live.run_started(1, 10);
        let mut report = live.report().clone();
        report.elapsed = Duration::from_secs(42);
        live.returned(Ok(report));

        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
        assert_eq!(live.overall().left, None, "完了就没有「还剩多久」可说");
        // 跑完坐着不动一小时再问，仍是同一个数——会话没有接着读自己那块表。
        live.tick(epoch + Duration::from_secs(3600));
        assert_eq!(live.overall().elapsed, Duration::from_secs(42));
    }

    /// **按停止停下来的那一趟退出码照旧**：两级都一样，它是用户自己的决定，不是失败
    /// （ADR 0013；`crate::exit_code` 的文档写着「按停止停下来的那一趟不在这里露面」）。
    ///
    /// 「照旧」不是「恒为零」：报告里有卷被隔离、有卷没做成，那两个数照给——
    /// 按停止不改的是**这一趟收成了什么样**与退出码之间那条对应，而那条对应
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

            // 其中一卷带着坏页进了隔离：仍是「有卷被隔离」那个数，按停止没把它盖掉。
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

    /// **确认点上那一卷的报告摆得住，也收得掉**（停车场 Q52，`p1-session/14`）。
    ///
    /// 它不进 [`Live::report`]：那一份装的是**收摊了的卷**，而这一卷还停在确认点上，
    /// 写出环节一步没走。混进去的话，退出会话时印到 stdout 的那一份里会多一卷
    /// 「写在那里、盘上却没有」的东西。
    ///
    /// 三条出路各收一次：一卷跑完（正式那一份进了报告）、一卷没做成（它连报告都没有）、
    /// 这一趟结束（停在确认点上被立即停止的那一卷两条都不报，没有别人会来清它）。
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
            "确认点上那一份没收下"
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

        // 这一趟结束（确认点上被立即停止就是这一条）：同样作废。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        live.run_finished(RunOutcome::Stopped(Instruction::Abort));
        assert!(live.summarized().is_none());

        // 别的两遍那一格是 `None`，不该把摆着的那一份抹掉——它只在确认点上有。
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.volume_started(Path::new("库/卷一"), 6);
        live.pass_started(Pass::Second, Some(&summarized));
        live.pass_started(Pass::First, None);
        assert!(live.summarized().is_some(), "另一遍开工把它抹掉了");
    }

    /// **给定「此刻」，已用与预计就是定值**（`session-redesign/04`，spec《时钟》）。
    ///
    /// 算这两个数的地方不再直接问系统时钟：真会话每帧把单调时钟读一次交进来
    /// （[`Live::tick`]），用例给定值。同一份攒下的东西于是逐次算出**同一个数**，
    /// 断言不带余量——从前靠把开工那一刻往回拨一段来近似，只断得出「不少于」。
    ///
    /// 表从开工那一条掐起：开工之前那一段（清点与那几道检查）不进已用。
    #[test]
    fn a_given_now_makes_elapsed_and_eta_the_same_every_time_they_are_asked() {
        let epoch = Instant::now();
        let mut live = fixture::live_at(epoch, RunMode::Process, Resuming::GoesOn);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷一"), 1000);
        for _ in 0..250 {
            live.stepped();
        }
        live.tick(epoch + Duration::from_secs(300));

        let first = live.overall();
        assert_eq!(first.elapsed, Duration::from_secs(300));
        assert_eq!(
            first.left,
            Some(Duration::from_secs(900)),
            "250 步用了 300s，剩下 750 步该是 900s"
        );
        assert_eq!(live.overall(), first, "同一个「此刻」问两次，答案变了");
    }

    /// **表从开工那一条掐起**：开工之前那一段（清点与那几道检查）不进已用，
    /// 而开工那一刻记的是那一条到达时的「此刻」。
    #[test]
    fn the_clock_starts_at_the_run_started_event_not_at_construction() {
        let epoch = Instant::now();
        let mut live = fixture::live_at(epoch, RunMode::Process, Resuming::GoesOn);
        live.tick(epoch + Duration::from_secs(60));
        live.run_started(1, 1000);
        live.tick(epoch + Duration::from_secs(90));
        assert_eq!(live.overall().elapsed, Duration::from_secs(30));
    }

    /// **等待确认的那几分钟谁都不算**（停车场 Q41，`CONTEXT.md` 的《会话》：
    /// 确认点上等人的那段时间不算进计时）。
    ///
    /// 不减的话，屏上那两个数会在人看着报告拿主意的那几分钟里一路往上涨，
    /// 而那几分钟里库一步都没走——「剩多久」说的就成了「用户拿主意还要多久」。
    ///
    /// 断言逐格相等：「此刻」是给定的（`session-redesign/04`），等人那一截从确认点那一条
    /// 起算、到答话那一条止，两头读的都是给定的那个时刻，「已用」因此该是**恰好**那几段之和。
    ///
    /// **不等人的那一趟一格不减**：执行那一趟同样走到确认点，但观察者当场答字就返回——
    /// 那一段是库自己的开销，本来就该算进这一趟。
    #[test]
    fn the_minutes_spent_deciding_are_charged_to_nobody() {
        /// 确认点之前跑了这么久。
        const RAN_FOR: Duration = Duration::from_secs(300);
        /// 人在确认点上看了这么久的报告。
        const DECIDED_FOR: Duration = Duration::from_secs(120);
        let epoch = Instant::now();
        let summarized = fixture::processed_volume("卷一", None);

        let mut live = fixture::live_at(epoch, RunMode::Process, Resuming::Waits);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.stepped();
        // 确认点：等人那一截从这里起算。
        live.tick(epoch + RAN_FOR);
        live.pass_started(Pass::Second, Some(&summarized));
        assert_eq!(live.overall().elapsed, RAN_FOR, "等之前那一段被减掉了");

        // 人在看报告：屏上那个数一格都不该多。
        live.tick(epoch + RAN_FOR + DECIDED_FOR);
        assert_eq!(
            live.overall().elapsed,
            RAN_FOR,
            "等人的那一截算进了「已用」"
        );

        // 答完话接着跑：等掉的那一截留在账上，往后的时间照旧算。
        live.decide(Instruction::Continue, Reach::ThisVolume);
        assert_eq!(
            live.overall().elapsed,
            RAN_FOR,
            "答话那一刻「已用」跳了一格"
        );
        live.tick(epoch + RAN_FOR + DECIDED_FOR + Duration::from_secs(30));
        assert_eq!(
            live.overall().elapsed,
            RAN_FOR + Duration::from_secs(30),
            "答完话之后那一截又被算回来了"
        );

        // 下一卷的确认点：照旧等人，那一格照旧开——一趟里每一卷各等一次
        // （`volume-discovery/07`）。
        live.volume_started(Path::new("库/卷二"), 1000);
        live.pass_started(Pass::Second, Some(&summarized));
        assert!(
            live.deliberating_since.is_some(),
            "第二卷的确认点上没开始等人"
        );
        live.tick(epoch + RAN_FOR + DECIDED_FOR + Duration::from_secs(90));
        assert_eq!(
            live.overall().elapsed,
            RAN_FOR + Duration::from_secs(30),
            "第二卷的确认点上等人的那一截被算进了「已用」"
        );

        // **答「后面的卷都写出」之后那一格再也不开**：往下的确认点由观察者那一侧
        // 当场答掉，没有人在等。照开的话它再也关不上——关它的只有确认点上的答话，
        // 而往下不会再有一次，屏上那两个数于是从此不动。
        live.decide(Instruction::Continue, Reach::ForTheRest);
        live.volume_started(Path::new("库/卷三"), 1000);
        live.pass_started(Pass::Second, Some(&summarized));
        assert!(
            live.deliberating_since.is_none(),
            "答过「后面的卷都写出」，等人那一格又开了——没有人在等，而它再也关不上"
        );
        live.tick(epoch + RAN_FOR + DECIDED_FOR + Duration::from_secs(100));
        assert_eq!(
            live.overall().elapsed,
            RAN_FOR + Duration::from_secs(40),
            "答过「后面的卷都写出」之后表停了"
        );

        // 不等人的那一趟：确认点照样报，但那一格不开——观察者当场答字就返回。
        let mut going = fixture::live_at(epoch, RunMode::Process, Resuming::GoesOn);
        going.run_started(1, 1000);
        going.volume_started(Path::new("库/卷一"), 1000);
        going.tick(epoch + RAN_FOR);
        going.pass_started(Pass::Second, Some(&summarized));
        going.tick(epoch + RAN_FOR + DECIDED_FOR);
        assert_eq!(
            going.overall().elapsed,
            RAN_FOR + DECIDED_FOR,
            "不等人的那一趟也开始减了"
        );
    }

    /// **预览在答出第一个继续之前印的是 dry-run**（`p1-session/14`，ADR 0012 决定第 5 条）。
    ///
    /// 那一趟走的是 `Mode::Process`——参照要留着，答继续时分析环节才不必重算——
    /// 而在确认点上答出继续之前，输出目录一个字节都没有。抬头那一行
    /// 「预览：只分析，不写文件，下面列出的输出路径都还没有写入」正是这时要说的话。
    ///
    /// **一趟里每一卷各答一次**（`volume-discovery/07`），而抬头那一行只有一句：
    /// 答过一次继续就有一卷写了出去，那一趟因此印执行——记下来的是那几个字里
    /// **最弱**的那一个（见 [`Live::decided`]）。
    ///
    /// 执行那一趟照库收到的那个字印，这一格与从前逐字相同。
    #[test]
    fn a_trial_that_never_walked_the_second_pass_prints_as_a_dry_run() {
        // 接着写出那一趟：起手、答做完再停、答立即停止，三处都是 dry-run；只有答继续那一处不是。
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

        // 几十卷的一趟：头一卷答继续（它写出去了），第二卷答做完再停（这一趟到此为止）。
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

        // 「后面的卷都写出」摆下的那个默认答案单独记一格：它不是闩，也不替
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
}
