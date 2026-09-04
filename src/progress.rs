//! 进度：长任务向外报到的地方（spec 的 story 30，ADR 0011）。
//!
//! 报的是一条**事件流**：库把一条 [`Event`] 交给观察者，观察者收下它、回一个 [`Instruction`]。
//! 回调因此是**双向**的——取消不必另开一条信号通路，而「第二遍之前停下来问一句」
//! （ADR 0012：续做）与「按停」（ADR 0013：两级停）走的是同一条回路。
//!
//! 库这一侧只报到，**印在哪、印不印、长什么样、要不要在决策点上等人由调用方定**。
//! 理由与 `run` 那个 seam 是同一个：往库里塞一个终端组件，就等于让它替 CLI 决定输出的样子，
//! 而测试也没法在不开终端的情况下问「它到底报到了吗」。CLI 在 `main` 里把它接到 indicatif 上，
//! 用例接一个记账本上去。
//!
//! **事件流就是报告的增量**：一卷跑完的那条事件带着那一卷的 [`VolumeReport`]，
//! 调用方边跑边把整份报告攒出来。命令行攒完在最后一次性渲染，会话攒到哪儿画到哪儿——
//! 两者拿的是同一份数据，不是两套（ADR 0011 决定第 2 条）。
//!
//! **观察者可能很久不返回**：会话要在决策点上等人拿主意（ADR 0012 的《后果》）。
//! 库这一侧因此有一条硬规矩——**任何持锁的地方都不许调它**。本模块自己那一格
//! 「至今为止收到过的最强指令」用的是原子量而不是锁（见 [`Standing`]），
//! 而卷缓存那把锁由[哨兵](LockSentinel)守着：调试构建上持着它走到报到就当场恐慌，
//! 发布构建上哨兵整个不在。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::report::{RunOutcome, VolumeReport};

/// 库向外报的一条消息（ADR 0011 决定第 1 条）。
///
/// **非穷尽做了两层**：枚举自己，加**每一个**变体自己——包括眼下不带字段的
/// [`Stepped`](Self::Stepped)。以后多报一件事，无论是多一个变体还是往已有的变体里多塞一个数，
/// 都不该逼着现有的实现方跟着改。现在有三个实现方：CLI 的进度条、会话、用例里的记账本。
/// 这条性质已经兑现过两次——
/// 03 号票往 [`RunStarted`](Self::RunStarted) 里加了全局总步数，三个实现方一个都没被逼着改；
/// 05 号票加了 [`VolumeFailed`](Self::VolumeFailed)、并往 [`RunFinished`](Self::RunFinished)
/// 里塞进「这一趟是怎么收的场」，也只有真要用那件事的实现方动了手。
/// 库外的 `match` 因此一律要带 `..` 与 `_`，那正是这条性质起作用的样子。
///
/// 事件带的是**借用**：一卷跑完那条带着的 [`VolumeReport`] 还在库的手上，
/// 要留下来的观察者自己克隆（[`VolumeReport`] 是 `Clone`）。这样不留的那些实现方
/// ——进度条只想画一条横条——就不必为一份它转手就扔的报告付克隆的价。
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// 这一趟开始了，点名了 `volumes` 个卷，这一趟最多走 `steps` 步。
    ///
    /// 排在开工前那几道检查与**预扫**之后：范围为空、输出落在源里、两个卷撞同一个去处、
    /// 输出根写不进去、有卷点不开，五者都让 `run` 当场返回 `Err`，一条事件都不发。
    /// 「一卷点不开就整趟拒绝」因此天然发生在任何卷级事件之前（见 `crate::survey`）。
    #[non_exhaustive]
    RunStarted {
        /// 这一趟点名了几个卷。
        volumes: usize,
        /// 这一趟最多走多少步——**各卷步数之和**（见 `crate::survey`）。
        ///
        /// 与单卷那个数同一个性质：**上界**，不是承诺（`CONTEXT.md` 的《进度》：
        /// 预告的步数是上界，两处各出一次）。拿它画全局进度的实现方因此**要在一卷跑完时
        /// 结清**那一卷预告剩下的步——这是这个字段对实现方的要求，不是一句建议。
        /// CLI 那一份见二进制侧的 `Bar::finish_volume`。
        steps: u64,
    },
    /// 一个卷开始了，这一卷这一趟最多走 `steps` 步。
    ///
    /// `steps` 是**上界**，不是承诺（见 `crate::volume_steps`）：幂等命中的卷提前收摊，
    /// 而第二段那一截按「每张源页最多几张输出页」预告。
    #[non_exhaustive]
    VolumeStarted {
        /// 卷标识：源目录路径，或源归档的文件路径。
        volume: &'a Path,
        /// 这一卷这一趟最多走多少步。
        steps: u64,
    },
    /// 当前这一卷开始走某一遍。「进度条现在在走哪一遍」只有它答得出来。
    #[non_exhaustive]
    PassStarted {
        /// 在走哪一遍。
        pass: Pass,
        /// 这一卷**到此刻为止**的报告。只有[决策点](Pass::Second)那一条带着它，
        /// 另外两遍是 `None`（停车场 Q52）。
        ///
        /// 非带不可：决策点问的是「这一卷的第二遍还做不做」，而答得上这一问的东西
        /// ——卷级判定、逐页结果、缓存用量、解码计数——要到
        /// [`VolumeFinished`](Self::VolumeFinished) 才交出去，而那一条排在决策点**之后**。
        /// 不带的话，要在这里等人拿主意的调用方手上只有逐步事件，屏上画不出任何
        /// 可供拿主意的东西。
        ///
        /// 带的是**那一刻为真**的一份，不是预告：第二遍一步没走，因此
        /// `timing.second_pass` 是零，`output` 指的是这一卷**会**落到的那个位置
        /// （此刻盘上还没有它）。答收尾之后 `VolumeFinished` 交出来的那一份与它的差
        /// 也就只有计时那一格——这一卷等于走了一次试算（`CONTEXT.md` 的《会话》：决策点）。
        so_far: Option<&'a VolumeReport>,
    },
    /// 又走完了一步。
    ///
    /// 步的单位见 `CONTEXT.md` 的《进度》。这一条**从计算线程上报出来**，
    /// 因此同一卷内可能并发到达、页序不作数——要页序的东西在
    /// [`VolumeFinished`](Self::VolumeFinished) 带的那份报告里。
    #[non_exhaustive]
    Stepped {},
    /// 一页失败了，附上给人读的那句原因。
    ///
    /// 它不等整卷跑完：失败页要在**出现的当场**就说得出口（09 号票的会话主区）。
    /// 同一份原因随后也会在 [`VolumeFinished`](Self::VolumeFinished) 那份报告的
    /// `PageOutcome::Failed` 里出现一次——那一份是结果，这一条是增量。
    #[non_exhaustive]
    PageFailed {
        /// 是哪一页：卷根接上成员的相对路径，与报告那一侧的 `PageReport::source` 同一个身份。
        page: &'a Path,
        /// 为什么失败，由内到外的错误链。
        reason: &'a str,
    },
    /// 一卷跑完了，带着**这一卷的报告**（ADR 0011 决定第 2 条）。
    ///
    /// 幂等命中而跳过的卷同样报这一条：那也是一份完整的 [`VolumeReport`]，
    /// 只是判定是 `VolumeVerdict::Skipped`。「跳过」在屏幕上不该长成「卡住」。
    ///
    /// **被[中止](Instruction::Abort)掉的那一卷不报这一条**：它那格 `partial` 已经丢掉，
    /// 那一卷等于没做，没有报告可带。它也不报 [`VolumeFailed`](Self::VolumeFailed)——
    /// 被停下来不是失败。流上因此看得见一条开卷、后面两条一条都没有，
    /// 那正是「这一卷被中止了」在事件流上的样子；收场那一条会把它再说一次
    /// （[`RunOutcome::Stopped`]）。
    #[non_exhaustive]
    VolumeFinished {
        /// 这一卷的报告。攒下来即是 `Report::volumes`，一字不差。
        report: &'a VolumeReport,
    },
    /// 一整卷**没做成**，附上给人读的那句原因（05 号票：卷级失败）。
    ///
    /// 它与 [`VolumeFinished`](Self::VolumeFinished) 二选一：一条开卷之后到得了的只有其中
    /// 一条——那一卷要么交出一份报告，要么交出这一句原因。两条都没有的只剩一种情形，
    /// 就是[中止](Instruction::Abort)。
    ///
    /// 同一句原因随后也会在 `Report::failed_volumes` 里出现一次——那一份是结果，
    /// 这一条是增量，与 [`PageFailed`](Self::PageFailed) 同一个待遇。
    ///
    /// **这一趟不因此停下**：其余卷照做，收场那一条照报。画进度的实现方要在这里
    /// 把那一卷的横条收掉、把它预告剩下的步结清——与一卷跑完那一条一样，
    /// 理由见 [`RunStarted`](Self::RunStarted) 的 `steps`。
    #[non_exhaustive]
    VolumeFailed {
        /// 是哪一卷：源目录路径，或源归档的文件路径，与 `VolumeReport::volume` 同一个身份。
        volume: &'a Path,
        /// 为什么没做成，由内到外的错误链。
        reason: &'a str,
    },
    /// 这一趟完了，带着**它是怎么收的场**（停车场 Q39）。
    ///
    /// **报过开工，就一定报得到这一条**，拒绝执行的那一趟也不例外——那时 `outcome` 是
    /// [`RunOutcome::Refused`]，紧接着 `run` 返回那个错误本身（信息因此不少一条：
    /// 事件说了「完了」，返回值说了「为什么」）。
    ///
    /// 开工那条事件**之前**被拒的那几种（范围为空、输出落在源里、两个卷撞同一个去处、
    /// 输出根写不进去、有卷点不开）仍是一条事件都不发：那一趟连开工都没有，也就谈不上收场。
    #[non_exhaustive]
    RunFinished {
        /// 这一趟是怎么收的场。攒报告的那一端拿它填 `Report::outcome`——
        /// 拒绝执行那一种除外，那一趟没有报告可填。
        outcome: RunOutcome,
    },
}

#[cfg(debug_assertions)]
impl Event<'_> {
    /// [`PassStarted`](Self::PassStarted) 的名字。见 [`name`](Self::name) 末段：
    /// 决策点在造出这一条之前就要报给哨兵。
    const PASS_STARTED: &'static str = "PassStarted";

    /// 这一条事件的名字，[哨兵](LockSentinel)恐慌时指名的就是它。
    ///
    /// **穷尽写开，不留 `_`**：多一个变体时这里当场编译不过，而那正是要的——
    /// 新添的那一条报到点也该报得出自己叫什么。非穷尽拦的是库外的 `match`，
    /// 本模块自己在里面（见本枚举的《非穷尽做了两层》）。
    ///
    /// 报名字而不是 `Debug` 整份：一卷跑完那一条带着整份 `VolumeReport`，
    /// 印出来能把恐慌消息淹掉，而要答的问题只是「哪一处报到」。
    ///
    /// [决策点](Events::ask_before_the_second_pass)那一处要在**造出事件之前**就问哨兵
    /// （它有自己一道判空，绕得过 [`Events::ask`]），因此那一条的名字单拎成
    /// [`PASS_STARTED`](Self::PASS_STARTED)——两处报的是同一个字。
    fn name(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "RunStarted",
            Self::VolumeStarted { .. } => "VolumeStarted",
            Self::PassStarted { .. } => Self::PASS_STARTED,
            Self::Stepped { .. } => "Stepped",
            Self::PageFailed { .. } => "PageFailed",
            Self::VolumeFinished { .. } => "VolumeFinished",
            Self::VolumeFailed { .. } => "VolumeFailed",
            Self::RunFinished { .. } => "RunFinished",
        }
    }
}

/// 一个卷这一趟要走的那几遍中的一遍（`CONTEXT.md` 的《进度》：步的三段）。
///
/// 三段与 `VolumeTiming` 的三段是同一条分界线，而且**各段自己可能不在**：
/// `--no-metadata` 关掉幂等那一道，dry-run 没有第二遍。报的是这一趟真要走的那几遍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pass {
    /// 幂等这一道：算出本卷指纹，再与上一趟写在输出里的比。
    Fingerprint,
    /// 第一遍：解码、彩页识别、几何、缩放、算判据、进缓存。
    First,
    /// 第二遍：量化、编码、写页、搬透传文件、收尾改名。
    ///
    /// 这一条报出去的那一刻**汇总已经做完、第二遍还没开始**，而那正是续做的决策点
    /// （ADR 0012 决定第 2 条）：答继续就往下做，答收尾就停在这儿——那就是 dry-run 的效果。
    /// 观察者答的那个字在这一条上**当场作数**，不进闩就算数，见库内的
    /// `Events::ask_before_the_second_pass`。
    ///
    /// 它因此是唯一一条**答复会改变库下一步做什么**的事件：答收尾的话，这一卷的第二遍
    /// 一步都不走，报告照出（`CONTEXT.md` 的《会话》：决策点）。
    Second,
}

/// 观察者收下一条事件之后回给库的那一个字（ADR 0011 决定第 1 条）。
///
/// 三个，不是四个：进度、两级停、试算→执行的决策点共用这一条回路
/// （`CONTEXT.md` 的《进度》）。它**不非穷尽**——三级是 ADR 0013 拍死的形状，
/// 而它是观察者要**造**出来的东西，非穷尽会让库外根本造不出任何一个。
///
/// 序即**力度**：`Continue < Finish < Abort`。库把收到过的最强的那一个记下来
/// （见 [`Standing`]），因此指令**只升不降**——按停是个闩，不是一个可以反悔的开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Instruction {
    /// 继续。
    #[default]
    Continue,
    /// 收尾：**当前卷跑完就停**（ADR 0013 决定第 1 条）。
    ///
    /// 盘上因此只有完整的卷，下一趟幂等接着走。检查点在卷边界上，
    /// 见 `crate::run`。
    Finish,
    /// 中止：立刻停（ADR 0013 决定第 2 条）。
    ///
    /// 检查点在**页边界**上（见 `Events::aborting` 与 `crate::process_volume`）：
    /// 当前卷不跑完，它那格 `partial`
    /// 直接丢弃——那一卷等于没做，最终位置上一个字节都没动过。它因此不进报告。
    ///
    /// 卷边界上它与 [`Finish`](Self::Finish) 一样让整趟停下：力度更强的指令
    /// 不该比更弱的那个停得更晚。
    Abort,
}

impl Instruction {
    /// 记进原子量的那个数。手写而不是 `#[repr(u8)]` 加 `as u8`：那样写，「派生出来的序」
    /// 与「记下去的数」的一致靠的是变体的书写顺序，改一次顺序两者就悄悄分家。
    /// 手写的这一份与派生的 `Ord` 由
    /// [用例](tests::the_three_instructions_are_ordered_by_how_hard_they_stop)拴在一起。
    const fn code(self) -> u8 {
        match self {
            Self::Continue => 0,
            Self::Finish => 1,
            Self::Abort => 2,
        }
    }

    /// 从原子量里读回来。越界的数按最强的算——那一侧宁可多停一趟，不可漏停一趟。
    const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Continue,
            1 => Self::Finish,
            _ => Self::Abort,
        }
    }
}

/// 一次运行的进度观察者：**收事件、回指令**（ADR 0011 决定第 1 条）。
///
/// 一个方法，不是一排：多报一件事是多一个 [`Event`] 变体，而不是多一个方法——
/// 方法一多，三个实现方每一个都得跟着改一处，其中两处只会填成空函数。
///
/// 实现方**可以很久不返回**：会话要在决策点上把报告画出来并等用户拿主意
/// （ADR 0012 决定第 3 条）。库因此保证不在持锁的地方调它，用例里的记账本不受影响。
pub trait Progress: Send + Sync {
    /// 收下一条事件，回一个指令。
    ///
    /// 不想干预的实现方一律回 [`Instruction::Continue`]——CLI 的进度条就是这样。
    fn observe(&self, event: Event<'_>) -> Instruction;
}

/// [`Request`](crate::Request) 里装观察者的那一格。
///
/// 是个包装类型而不是直接一个 `Arc<dyn Progress>`，只为一件事：`Request` 要 `Debug`，
/// 而 trait 对象没有。包装在这里把那一格印成一个固定的字样。
#[derive(Clone)]
pub struct ProgressSink(Arc<dyn Progress>);

impl ProgressSink {
    /// 把一个观察者装进去。
    pub fn new(progress: impl Progress + 'static) -> Self {
        Self(Arc::new(progress))
    }
}

impl std::fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProgressSink")
    }
}

/// 这一趟至今为止收到过的**最强**指令——文档里到处说的那个「闩」就是它。
///
/// 用原子量而不是锁，理由是本模块头上那条硬规矩：观察者可能很久不返回，
/// 而它是从计算线程上被调到的——记账本要是一把锁，报到一步就得先抢它，
/// 「不在持锁处调观察者」这句话当场就不成立了。
///
/// `fetch_max` 直接把「只升不降」写进了操作本身（见 [`Instruction`] 的序）：
/// 两条线程同时报到、一条答收尾一条答继续，结果恒是收尾。
#[derive(Debug, Default)]
pub(crate) struct Standing(AtomicU8);

impl Standing {
    fn record(&self, instruction: Instruction) {
        self.0.fetch_max(instruction.code(), Ordering::Relaxed);
    }

    fn get(&self) -> Instruction {
        Instruction::from_code(self.0.load(Ordering::Relaxed))
    }
}

/// 观察者在**决策点**上把人等掉的那一截，一次运行一份（停车场 Q41）。
///
/// 它不是任何一段的耗时，也不是段外那一截：那几个数说的是**库做了多久**，而这一截
/// 库一步都没走——人在看报告拿主意（ADR 0012 决定第 3 条）。两处墙钟
/// （[`VolumeTiming::elapsed`](crate::VolumeTiming::elapsed) 与
/// [`Report::elapsed`](crate::Report::elapsed)）因此各自把它减掉，
/// 减法在 `crate::process_volume` 与 `crate::run` 各做一次。
///
/// 掐的是**那一次问话的全程**，而不是「全程减去观察者自己那点开销」：在这一处，
/// 观察者要做的事就是等人拿主意，整段都不是库的账。其余报到点**一纳秒都不掐**——
/// 那几处观察者花掉的时间（进度条一次 `inc(1)`）是它自己的成本，本来就该算进这一趟。
///
/// 与 [`Standing`] 同一个理由用原子量而不是锁：本模块头上那条硬规矩说观察者可能很久
/// 不返回，而这一格记的正是它等了多久——拿锁来记，规矩当场就不成立了。
#[derive(Debug, Default)]
pub(crate) struct Deliberation(AtomicU64);

impl Deliberation {
    /// 记下等掉的一截。
    ///
    /// 换算成纳秒时**饱和**：`Duration` 的 `u128` 纳秒装得下的比 `u64` 多，真越界了
    /// 让它停在最大值，不绕回零。累加本身不设防，那不是漏——`u64` 纳秒是五百八十四年，
    /// 而这一格一卷只加一次。
    fn add(&self, waited: Duration) {
        let nanos = u64::try_from(waited.as_nanos()).unwrap_or(u64::MAX);
        self.0.fetch_add(nanos, Ordering::Relaxed);
    }

    fn total(&self) -> Duration {
        Duration::from_nanos(self.0.load(Ordering::Relaxed))
    }
}

/// **持锁哨兵**：调试构建上守住本模块头上那条硬规矩——不得在持锁处调观察者。
///
/// 规矩说的是「**没有**发生某件事」，而一把 guard 活到哪儿是编译期的作用域问题，
/// 跑起来看不见：往管线里加一条报到，摆错了地方不会红，会死锁（停车场 Q40）。
/// 哨兵把它变成看得见的——持着[卷缓存那把锁](crate::lock)走到报到那一步，
/// [`Events::ask`] 当场恐慌，消息里指名是哪一处报到。
///
/// 它是一枚 RAII 凭据：`crate::lock` 每交出一把锁就造一枚，锁放掉时它跟着析构。
/// 数的是**这条线程**手上有几把——第一遍每条 rayon 线程各拿各的缓存锁，
/// 一格全局计数会把别的线程的那把算到自己头上。计数而不是布尔，多握一把不抹掉前一把。
///
/// **只守卷缓存那一把。** 报到时手里还攥着一条 rayon 工作线程与读取层的在途字节额度
/// （`crate::read` 的额度），两样都不是锁、都不会死锁：观察者久不返回只把读取层背压闸住，
/// 而消费不动就别再读多半正是想要的。观察者**自己**拿的锁更不在这套里——
/// 会话那一端本来就要拿锁画屏。
///
/// **发布构建上整个不在**（`cfg(debug_assertions)`）：为一个调试期的检查给每张页加开销，
/// 得不偿失。`crate::CacheGuard` 那一格因此在发布构建上退化成一把裸 `MutexGuard`。
#[cfg(debug_assertions)]
pub(crate) struct LockSentinel(());

#[cfg(debug_assertions)]
thread_local! {
    /// 这条线程此刻攥着几把守着的锁。
    static LOCKS_HELD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(debug_assertions)]
impl LockSentinel {
    /// 记下「这条线程又拿了一把」。
    pub(crate) fn new() -> Self {
        LOCKS_HELD.with(|held| held.set(held.get() + 1));
        Self(())
    }

    /// 报到之前问一次：此刻一把守着的锁都没攥着吧。
    ///
    /// **不问有没有观察者**：只在没人可问的那一趟才走到的报到点，正是最容易漏掉的一处，
    /// 而漏掉它的那一天有人接上观察者就死锁。[`Events::ask`] 因此把这一问摆在
    /// 「有没有观察者」那道判空**之前**，[决策点](Events::ask_before_the_second_pass)
    /// 自带的那道判空同理——它绕得过 `ask`，于是在它自己那一句上再问一次。
    ///
    /// 收的是**名字**而不是一条 [`Event`]：决策点那一处要在造出事件之前就问
    /// （造它要先拼一份报告）。
    #[track_caller]
    fn assert_none_held(event: &str) {
        assert!(
            LOCKS_HELD.with(std::cell::Cell::get) == 0,
            "在持着卷缓存那把锁的地方报到了：{event}（{}）。观察者可能很久不返回（见本模块的模块文档），持锁报到会把整条管线挂在一个外人手上——把那把锁掐在报到之前的那一句里。",
            std::panic::Location::caller(),
        );
    }
}

#[cfg(debug_assertions)]
impl Drop for LockSentinel {
    fn drop(&mut self) {
        LOCKS_HELD.with(|held| held.set(held.get() - 1));
    }
}

/// 管线内部报到用的那一端：没有观察者时每一条事件都是空操作。
///
/// 调用处因此不必到处判空——那种判空写着写着就会漏掉一处，而漏掉的那一处正是进度条卡住的地方。
/// 它是 `Copy` 的：第一遍要把它整个借给每一条计算线程。
#[derive(Clone, Copy)]
pub(crate) struct Events<'a> {
    sink: Option<&'a ProgressSink>,
    /// 收到过的最强指令。它活在 `run` 的栈上，一次运行一份——
    /// 存进 [`ProgressSink`] 是不行的：那一格在 `Request` 里，而 `Request` 会被复用，
    /// 上一趟按的停会跟着漏到下一趟去。
    standing: &'a Standing,
    /// 在决策点上等人等掉的那一截。与[闩](Standing)同一条寿命：活在 `run` 的栈上，
    /// 一次运行一份。
    deliberation: &'a Deliberation,
}

impl<'a> Events<'a> {
    pub(crate) fn new(
        sink: Option<&'a ProgressSink>,
        standing: &'a Standing,
        deliberation: &'a Deliberation,
    ) -> Self {
        Self {
            sink,
            standing,
            deliberation,
        }
    }

    /// 发一条事件，把回来的指令记进[闩](Standing)，并**把那个字交回来**。
    ///
    /// 交回来的那个字只有决策点用得着（见 [`ask_before_the_second_pass`](Self::ask_before_the_second_pass)）；
    /// 其余报到点走 [`report`](Self::report)，指令在那里只被**记下**、不被就地执行：
    /// 事件从计算线程上报出来，而停在哪一道边界上是管线的事。
    ///
    /// 停下来的检查点眼下有两个，各在一道边界上（ADR 0013 的《后果》）：
    /// 卷边界那个在 `crate::run` 的逐卷循环里，问的是 [`standing`](Self::standing)；
    /// 页边界那个是 [`aborting`](Self::aborting)，由 `crate::process_volume` 摆在它每一个
    /// 逐成员的循环头上——**摆在哪几处只在那里数得清**，本条不复述。
    ///
    /// 没有观察者时答的是[继续](Instruction::Continue)：没人可问就等于没人拦。
    #[cfg_attr(debug_assertions, track_caller)]
    fn ask(self, event: Event<'_>) -> Instruction {
        // **哨兵**：此刻一把守着的锁都不许攥着（见 [`LockSentinel`]）。问在判空**之前**，
        // 因此没有观察者的那一趟也照问不误。
        //
        // 报到点的位置由 `#[track_caller]` 一路传下来，恐慌消息里指的是 `crate` 那一侧
        // 真正报到的那一行；两个属性都挂在 `debug_assertions` 上，发布构建上整个不在。
        #[cfg(debug_assertions)]
        LockSentinel::assert_none_held(event.name());
        let Some(sink) = self.sink else {
            return Instruction::Continue;
        };
        let answer = sink.0.observe(event);
        self.standing.record(answer);
        answer
    }

    /// 发一条事件，答的那个字只进[闩](Standing)。
    #[cfg_attr(debug_assertions, track_caller)]
    fn report(self, event: Event<'_>) {
        let _ = self.ask(event);
    }

    /// **续做的决策点**：把「第二遍要开始了」报出去，回的是观察者**当场答的那个字**
    /// （ADR 0012 决定第 2 条，`CONTEXT.md` 的《会话》：决策点）。
    ///
    /// 报出去的那一条**带着这一卷到此刻为止的报告**（见 [`Event::PassStarted`] 的 `so_far`）：
    /// 要在这里等人拿主意的调用方靠它画出「拿什么主意」。拼那一份要遍历逐页结果、
    /// 读一次缓存用量，因此收的是一个**闭包**——[没人可问](Self::sink)的那一趟连拼都不拼，
    /// 而命令行不带进度条的那条路走的正是那一支。
    ///
    /// 回的**不是**[闩](Self::standing)，而这是这一处与两个检查点唯一的差别，理由是问题不同：
    /// 闩答的是「这一趟还走不走」，这里问的是「这一卷的第二遍还做不做」。拿闩来答的话，
    /// 第一遍里按下的**收尾**会顺手把当前卷的第二遍也吃掉，而收尾的定义正是
    /// 「当前卷跑完才停」（ADR 0013 决定第 1 条）——盘上会因此少一整卷。
    /// 答复照样进闩：这一卷停在这儿之后，剩下的卷也不必开工了。
    ///
    /// 观察者在这里**按设计会等人**（ADR 0012 决定第 3 条：等不等人是调用方的策略）。
    /// 这一次问话的**全程**掐出来记进 [`Deliberation`]，两处墙钟各自减掉它——
    /// 掐全程而不是掐「等人那一半」的理由，见那个类型。
    ///
    /// **没人可问就连表都不掐**：那一趟根本没有人在这里等，掐出来的会是两次取时刻之差，
    /// 而那是库自己的开销，不该从这一趟的墙钟里减掉。命令行不带进度条的那条路走的正是这里。
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn ask_before_the_second_pass(
        self,
        so_far: impl FnOnce() -> VolumeReport,
    ) -> Instruction {
        // **哨兵**：下面那道判空绕得过 [`Self::ask`] 里那一问，而这一处正是按设计要等人的
        // 那一处——没人可问的那一趟（命令行不带进度条）也照问不误，见 [`LockSentinel`]。
        #[cfg(debug_assertions)]
        LockSentinel::assert_none_held(Event::PASS_STARTED);
        if self.sink.is_none() {
            return Instruction::Continue;
        }
        // 拼那一份报告是**库自己的工夫**，掐在等人那一截之外：表从它拼完才起
        // （见 [`Deliberation`]：只掐观察者没返回的那一段）。
        let so_far = so_far();
        let asked = Instant::now();
        let answer = self.ask(Event::PassStarted {
            pass: Pass::Second,
            so_far: Some(&so_far),
        });
        self.deliberation.add(asked.elapsed());
        answer
    }

    /// 这一趟至今在决策点上等掉的那一截，累计（停车场 Q41）。
    ///
    /// 只升不降，因此两次读数之差就是这中间等掉的时间——`crate::process_volume` 拿它
    /// 算这一卷该减掉多少。
    pub(crate) fn deliberated(self) -> Duration {
        self.deliberation.total()
    }

    /// 至今为止收到过的最强指令。**卷边界**那个检查点问的就是它。
    pub(crate) fn standing(self) -> Instruction {
        self.standing.get()
    }

    /// **页边界那个检查点**：这一趟按下中止了吗（ADR 0013 决定第 2 条）。
    ///
    /// 逐个成员往下走的每一个循环在自己的循环头上问它一次（清单见 `crate::process_volume`），
    /// 答是就当场停下，剩下的成员不做了。停下来之后调用方还要再问一次它，
    /// 决定这一卷算不算做完；**再问一次恒得同一个答案**，因为闩[只升不降](Standing)——
    /// 中止之上没有更强的指令了。各段因此不必把「我是被中止的」当成返回值往上传。
    ///
    /// 只认中止这一级：[收尾](Instruction::Finish)停在卷边界上，页边界不该抢它的活
    /// （ADR 0013 决定第 1 条：收尾要当前卷跑完，盘上只留完整的卷）。
    pub(crate) fn aborting(self) -> bool {
        self.standing() == Instruction::Abort
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn run_started(self, volumes: usize, steps: u64) {
        self.report(Event::RunStarted { volumes, steps });
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn volume_started(self, volume: &Path, steps: u64) {
        self.report(Event::VolumeStarted { volume, steps });
    }

    /// 某一遍开工了。
    ///
    /// [第二遍](Pass::Second)不走这里，走
    /// [`ask_before_the_second_pass`](Self::ask_before_the_second_pass)——同一条事件，
    /// 只是那一处要把答复接回来。事件的形状因此没有分家。
    ///
    /// 这两遍不带 `so_far`：那一格是给决策点用的，而这里还没有汇总可交
    /// （见 [`Event::PassStarted`]）。
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn pass_started(self, pass: Pass) {
        self.report(Event::PassStarted { pass, so_far: None });
    }

    /// 走完一步。逐页、逐成员报到的那些地方用它。
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn step(self) {
        self.report(Event::Stepped {});
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn page_failed(self, page: &Path, reason: &str) {
        self.report(Event::PageFailed { page, reason });
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn volume_finished(self, report: &VolumeReport) {
        self.report(Event::VolumeFinished { report });
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn volume_failed(self, volume: &Path, reason: &str) {
        self.report(Event::VolumeFailed { volume, reason });
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn run_finished(self, outcome: RunOutcome) {
        self.report(Event::RunFinished { outcome });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::Mutex;

    /// 决策点那一条带的那份报告，本模块的用例用的一份空壳。
    ///
    /// **本模块问的不是它装了什么**——那是 `crate::process_volume` 拼出来的，
    /// 由 `tests/resume.rs` 在真跑一趟的现场断言。这里要的只是「有这么一份可交」，
    /// 好让[决策点](Events::ask_before_the_second_pass)那个闭包调得起来。
    fn nothing_yet() -> VolumeReport {
        let serial = crate::Readers {
            count: 1,
            chosen_by: crate::ChosenBy::Probe,
        };
        VolumeReport {
            volume: PathBuf::from("卷一"),
            output: PathBuf::from("出/卷一"),
            superseded: None,
            pages: Vec::new(),
            source_pages: 0,
            verdict: None,
            cache: crate::CacheUsage::new(crate::CacheBudget::default()),
            extracted: 0,
            io: crate::IoPlan {
                medium: crate::Medium::Unknown {
                    reason: "用例".to_owned(),
                },
                readers: serial,
                fingerprint: serial,
            },
            decodes: 0,
            timing: crate::VolumeTiming::default(),
        }
    }

    /// 记账用的观察者：报到什么就记什么，回的指令由用例事先摆好。
    ///
    /// 形状照 `tests/events.rs` 的 `Recorder` 办（句柄 + 一格共享记账）。那一个收的是
    /// 真跑一趟报出来的事件，这一个收的是 [`Events`] 转手的那些，共用不了代码，
    /// 至少共用一个样子。
    ///
    /// 记的是**事件本身印出来的样子**而不只是次数：带字段的事件报错了——卷路径、
    /// 预告的步数、失败原因——只数次数的话一条都不会红，而预告的步数报错正是进度条
    /// 「停在某个百分比上再也不动」的样子。
    #[derive(Clone, Default)]
    struct Tally(Arc<Counts>);

    #[derive(Default)]
    struct Counts {
        seen: Mutex<Vec<String>>,
        answer: Mutex<Instruction>,
    }

    impl Progress for Tally {
        fn observe(&self, event: Event<'_>) -> Instruction {
            self.0
                .seen
                .lock()
                .expect("记账没有中毒")
                .push(format!("{event:?}"));
            *self.0.answer.lock().expect("记账没有中毒")
        }
    }

    impl Tally {
        fn seen(&self) -> Vec<String> {
            self.0.seen.lock().expect("记账没有中毒").clone()
        }

        fn answers(&self, instruction: Instruction) {
            *self.0.answer.lock().expect("记账没有中毒") = instruction;
        }
    }

    /// 每一条事件都原样到得了装进去的那个观察者，此外哪儿都不到。
    ///
    /// 后半句只有拿前半句当参照才断言得出来：场上得先有一个收得到报到的观察者，
    /// 没装它的那一端走完之后它一动不动，那才叫没人收到。只调几下不断言，
    /// 测到的是「不恐慌」——而空操作与「悄悄少报了一步」都不恐慌，那个形式两者分不开。
    #[test]
    fn every_event_reaches_the_installed_observer_and_nowhere_else() {
        let tally = Tally::default();
        let sink = ProgressSink::new(tally.clone());
        let standing = Standing::default();
        let deliberation = Deliberation::default();

        let watched = Events::new(Some(&sink), &standing, &deliberation);
        watched.run_started(2, 30);
        watched.volume_started(Path::new("卷一"), 10);
        watched.pass_started(Pass::First);
        watched.step();
        watched.page_failed(Path::new("卷一/003.png"), "解不出来");
        watched.volume_failed(Path::new("卷二"), "盘拔了");
        watched.run_finished(RunOutcome::Completed);

        // 七条整份比对，不是逐条挑几个字：少报一条进度条会停在半路，多报一条它会冲过头，
        // 而带着的东西报错了——卷路径、预告的步数、失败原因、这一趟怎么收的场——
        // 挑着比就漏得掉，其中预告的步数报错正是进度条「停在某个百分比上再也不动」的样子。
        assert_eq!(
            tally.seen(),
            [
                "RunStarted { volumes: 2, steps: 30 }",
                r#"VolumeStarted { volume: "卷一", steps: 10 }"#,
                "PassStarted { pass: First, so_far: None }",
                "Stepped",
                r#"PageFailed { page: "卷一/003.png", reason: "解不出来" }"#,
                r#"VolumeFailed { volume: "卷二", reason: "盘拔了" }"#,
                "RunFinished { outcome: Completed }",
            ],
            "报到的那几条与发出去的对不上"
        );

        // 同一个记账本还在场上，而这一端没装它：几条报到一条都不该落到它那里。
        let elsewhere = Standing::default();
        let unhurried = Deliberation::default();
        let unwatched = Events::new(None, &elsewhere, &unhurried);
        unwatched.run_started(2, 30);
        unwatched.volume_started(Path::new("卷二"), 10);
        unwatched.step();
        unwatched.run_finished(RunOutcome::Completed);

        assert_eq!(tally.seen().len(), 7, "没装观察者，事件却到了某处");
        assert_eq!(
            unwatched.standing(),
            Instruction::Continue,
            "没人可问，闩却动了"
        );
    }

    /// 指令是个**闩**：收到过的最强的那一个作数，此后答什么都降不回去。
    ///
    /// 会话按一次停、手指离开键盘，管线不该因为下一条事件答了「继续」就接着跑完——
    /// 那正是「按停没反应」的样子。
    #[test]
    fn the_strongest_instruction_so_far_latches() {
        let tally = Tally::default();
        let sink = ProgressSink::new(tally.clone());
        let standing = Standing::default();
        let deliberation = Deliberation::default();
        let events = Events::new(Some(&sink), &standing, &deliberation);

        events.step();
        assert_eq!(
            events.standing(),
            Instruction::Continue,
            "没人按停，闩却动了"
        );

        tally.answers(Instruction::Finish);
        events.step();
        assert_eq!(events.standing(), Instruction::Finish, "按下的停没记住");

        // 回到继续，闩不松。
        tally.answers(Instruction::Continue);
        events.step();
        assert_eq!(
            events.standing(),
            Instruction::Finish,
            "按下的停被后一条事件抹掉了"
        );

        // 再升一级升得上去。
        tally.answers(Instruction::Abort);
        events.step();
        assert_eq!(events.standing(), Instruction::Abort, "升不到中止那一级");
    }

    /// 决策点回的是观察者**当场答的那个字**，不是闩（ADR 0012 决定第 2 条）。
    ///
    /// 分开它们要一个特定的现场：闩已经落在收尾上，而观察者当场答继续——那正是
    /// 「第一遍里按下收尾」（ADR 0013 决定第 1 条：收尾要**当前卷跑完**）。
    /// 决策点拿闩当答复的话，那一卷的第二遍会被顺手吃掉，盘上少一整卷。
    /// 在 `run` 那个 seam 上这两者多数情形下同值，分不开。
    #[test]
    fn the_decision_point_answers_with_the_word_just_said_not_the_latch() {
        let tally = Tally::default();
        let sink = ProgressSink::new(tally.clone());
        let standing = Standing::default();
        let deliberation = Deliberation::default();
        let events = Events::new(Some(&sink), &standing, &deliberation);

        // 第一遍的页边界上按下收尾：闩记住了它。
        tally.answers(Instruction::Finish);
        events.step();
        assert_eq!(events.standing(), Instruction::Finish, "按下的停没记住");

        // 手指离开键盘，决策点上答的是继续——这一卷的第二遍照走。
        tally.answers(Instruction::Continue);
        assert_eq!(
            events.ask_before_the_second_pass(nothing_yet),
            Instruction::Continue,
            "决策点拿闩当了答复，第一遍里按下的收尾把当前卷的第二遍也吃掉了"
        );
        assert_eq!(
            events.standing(),
            Instruction::Finish,
            "决策点把闩降回去了：这一卷之后剩下的卷会接着做"
        );

        // 反过来：决策点上答的那个字照样进闩——那一卷停在这儿之后，剩下的卷也不必开工。
        let deciding = Tally::default();
        let sink = ProgressSink::new(deciding.clone());
        let fresh = Standing::default();
        let unhurried = Deliberation::default();
        let events = Events::new(Some(&sink), &fresh, &unhurried);
        deciding.answers(Instruction::Finish);
        assert_eq!(
            events.ask_before_the_second_pass(nothing_yet),
            Instruction::Finish,
            "决策点没把当场答的那个字交回来"
        );
        assert_eq!(
            events.standing(),
            Instruction::Finish,
            "决策点上答的收尾没进闩"
        );
    }

    /// 决策点上等人的那一截掐得出来，别处报到的不掐（停车场 Q41）。
    ///
    /// 会话要在这里把报告画出来并等用户拿主意，而人会看着报告去泡茶（ADR 0012 的《后果》）。
    /// 那几分钟原样计进墙钟的话，报出来的耗时说的就不再是「库做了多久」。
    /// [`VolumeTiming::elapsed`](crate::VolumeTiming::elapsed) 与
    /// [`Report::elapsed`](crate::Report::elapsed) 各自减掉的就是这个数。
    #[test]
    fn only_the_wait_at_the_decision_point_is_clocked_as_deliberation() {
        /// 观察者在决策点上等这么久。够长，调度抖动淹不掉它；够短，用例不因此变慢。
        const WAITS: Duration = Duration::from_millis(20);

        /// 只在决策点上磨蹭的观察者。
        ///
        /// `tests/timing.rs` 的 `waiting_at_the_decision_point_is_charged_to_nobody`
        /// 有一个同形的：那一个隔着 crate 边界，共用不了代码。两处问的不是同一件事——
        /// 这一个问「掐出来的那个数对不对」（那一格是私有的，只有这里读得到），
        /// 那一个问「报告上的两个数减掉了它没有」。
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

        let sink = ProgressSink::new(Ponders);
        let standing = Standing::default();
        let deliberation = Deliberation::default();
        let events = Events::new(Some(&sink), &standing, &deliberation);

        assert_eq!(events.deliberated(), Duration::ZERO, "还没问就等上了");
        events.pass_started(Pass::First);
        events.step();
        assert_eq!(
            events.deliberated(),
            Duration::ZERO,
            "别处报到也算进了等人的那一截"
        );

        events.ask_before_the_second_pass(nothing_yet);
        assert!(
            events.deliberated() >= WAITS,
            "决策点上等掉的那一截没掐出来：{:?}",
            events.deliberated()
        );
    }

    /// 三级的序就是力度，而记进原子量的那个数与它同序。
    ///
    /// [`Standing`] 靠 `fetch_max` 实现「只升不降」，这两件事一旦对不上，
    /// 「按了中止却退回收尾」会静默发生。
    #[test]
    fn the_three_instructions_are_ordered_by_how_hard_they_stop() {
        assert!(
            Instruction::Continue < Instruction::Finish,
            "继续没排在收尾之前"
        );
        assert!(
            Instruction::Finish < Instruction::Abort,
            "收尾没排在中止之前"
        );
        assert!(
            Instruction::Continue.code() < Instruction::Finish.code(),
            "记下去的数与派生的序分了家"
        );
        assert!(
            Instruction::Finish.code() < Instruction::Abort.code(),
            "记下去的数与派生的序分了家"
        );
        for instruction in [
            Instruction::Continue,
            Instruction::Finish,
            Instruction::Abort,
        ] {
            assert_eq!(
                Instruction::from_code(instruction.code()),
                instruction,
                "记进去再读回来变了样"
            );
        }
        assert_eq!(
            Instruction::default(),
            Instruction::Continue,
            "默认的那个字不是继续"
        );
    }
}
