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
//! 调用处则各自把它记在注释里。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::report::VolumeReport;

/// 库向外报的一条消息（ADR 0011 决定第 1 条）。
///
/// **非穷尽做了两层**：枚举自己，加**每一个**变体自己——包括眼下不带字段的那两个。
/// 以后多报一件事，无论是多一个变体还是往已有的变体里多塞一个数，都不该逼着现有的实现方
/// 跟着改。现在有三个实现方：CLI 的进度条、会话、用例里的记账本，而两件事已经排着队了：
/// 03 号票要往 [`RunStarted`](Self::RunStarted) 里加全局总步数，
/// [`RunFinished`](Self::RunFinished) 迟早要说得出这一趟是怎么收的场（停车场 Q39）。
/// 库外的 `match` 因此一律要带 `..` 与 `_`，那正是这条性质起作用的样子。
///
/// 事件带的是**借用**：一卷跑完那条带着的 [`VolumeReport`] 还在库的手上，
/// 要留下来的观察者自己克隆（[`VolumeReport`] 是 `Clone`）。这样不留的那些实现方
/// ——进度条只想画一条横条——就不必为一份它转手就扔的报告付克隆的价。
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// 这一趟开始了，点名了 `volumes` 个卷。
    ///
    /// 排在开工前那几道检查**之后**：范围为空、输出落在源里、两个卷撞同一个去处，
    /// 三者都让 `run` 当场返回 `Err`，一条事件都不发。预扫连同全局总步数
    /// 也将落在这条事件上（03 号票）。
    #[non_exhaustive]
    RunStarted {
        /// 这一趟点名了几个卷。
        volumes: usize,
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
    #[non_exhaustive]
    VolumeFinished {
        /// 这一卷的报告。攒下来即是 `Report::volumes`，一字不差。
        report: &'a VolumeReport,
    },
    /// 这一趟完了。
    ///
    /// 只在**正常收场**时报——`run` 返回 `Err` 时不报，那时调用方拿到的是那个错误本身。
    /// 收尾停下来的那一趟照报：它是正常收场的一种，只是卷没做完。
    #[non_exhaustive]
    RunFinished {},
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
    /// 06 号票落地的是「停在这儿」，本票先把这个点报出去。
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
    /// **页边界那个检查点与丢弃 `partial` 那一格由 04 号票落地。** 本票只把这个字认下来：
    /// 卷边界上它与 [`Finish`](Self::Finish) 一样让整趟停下——力度更强的指令
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
}

impl<'a> Events<'a> {
    pub(crate) fn new(sink: Option<&'a ProgressSink>, standing: &'a Standing) -> Self {
        Self { sink, standing }
    }

    /// 发一条事件，把回来的指令记进[闩](Standing)。
    ///
    /// 指令在这里只被**记下**、不被就地执行：事件从计算线程上报出来，而停在哪一道边界上
    /// 是管线的事。眼下检查点**只有一个**，在 `crate::run` 的逐卷循环里——那就是卷边界
    /// （ADR 0013 决定第 1 条）。页边界那一个由 04 号票加，第二遍之前那一个由 06 号票加。
    fn report(self, event: Event<'_>) {
        if let Some(sink) = self.sink {
            self.standing.record(sink.0.observe(event));
        }
    }

    /// 至今为止收到过的最强指令。检查点问的就是它。
    pub(crate) fn standing(self) -> Instruction {
        self.standing.get()
    }

    pub(crate) fn run_started(self, volumes: usize) {
        self.report(Event::RunStarted { volumes });
    }

    pub(crate) fn volume_started(self, volume: &Path, steps: u64) {
        self.report(Event::VolumeStarted { volume, steps });
    }

    pub(crate) fn pass_started(self, pass: Pass) {
        self.report(Event::PassStarted { pass });
    }

    /// 走完一步。逐页、逐成员报到的那些地方用它。
    pub(crate) fn step(self) {
        self.report(Event::Stepped {});
    }

    pub(crate) fn page_failed(self, page: &Path, reason: &str) {
        self.report(Event::PageFailed { page, reason });
    }

    pub(crate) fn volume_finished(self, report: &VolumeReport) {
        self.report(Event::VolumeFinished { report });
    }

    pub(crate) fn run_finished(self) {
        self.report(Event::RunFinished {});
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

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

        let watched = Events::new(Some(&sink), &standing);
        watched.run_started(2);
        watched.volume_started(Path::new("卷一"), 10);
        watched.pass_started(Pass::First);
        watched.step();
        watched.page_failed(Path::new("卷一/003.png"), "解不出来");
        watched.run_finished();

        // 六条整份比对，不是逐条挑几个字：少报一条进度条会停在半路，多报一条它会冲过头，
        // 而带着的东西报错了——卷路径、预告的步数、失败原因——挑着比就漏得掉，
        // 其中预告的步数报错正是进度条「停在某个百分比上再也不动」的样子。
        assert_eq!(
            tally.seen(),
            [
                "RunStarted { volumes: 2 }",
                r#"VolumeStarted { volume: "卷一", steps: 10 }"#,
                "PassStarted { pass: First }",
                "Stepped",
                r#"PageFailed { page: "卷一/003.png", reason: "解不出来" }"#,
                "RunFinished",
            ],
            "报到的那几条与发出去的对不上"
        );

        // 同一个记账本还在场上，而这一端没装它：几条报到一条都不该落到它那里。
        let elsewhere = Standing::default();
        let unwatched = Events::new(None, &elsewhere);
        unwatched.run_started(2);
        unwatched.volume_started(Path::new("卷二"), 10);
        unwatched.step();
        unwatched.run_finished();

        assert_eq!(tally.seen().len(), 6, "没装观察者，事件却到了某处");
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
        let events = Events::new(Some(&sink), &standing);

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
