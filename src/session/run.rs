//! 起一趟、盯着它、收掉它。**会话里唯一起线程的地方。**
//!
//! 为什么要另起一条线程：[`tonefit::run`] 一进去就要跑到底，而会话这一头得接着画、
//! 接着认键——不然「跑起来时主区实时更新」就只剩一句话。库那一侧不需要知道这件事，
//! 它照旧在计算线程上报到（`progress` 那条硬规矩：观察者可能很久不返回，
//! 因此不在持锁处调它），这里收下每一条、折进 [`Live`]。
//!
//! # 观察者回的那个字
//!
//! 就是用户**按停按到的那一级**（ADR 0013）：没按过是[继续](Instruction::Continue)，
//! 按过一次是[收尾](Instruction::Finish)，再按一次是[中止](Instruction::Abort)。
//! 认键在 [`super::state`]，这一层只把那个字送到计算线程上——见 [`Latch`]。
//!
//! **只有一处例外，而它非有不可**：[决策点](at_the_decision_point)上的收尾要让成继续，
//! 不然第一遍里按下的那一下会把当前卷的第二遍一起吃掉——见 [`answer`]。
//!
//! # 决策点上等人：那条线程停在一道[闸](Gate)上
//!
//! **单卷试算跑到决策点就停住**（ADR 0012 决定第 1、3 条，`p1-session/14`）：计算线程
//! 在观察者里一直等到用户答话，会话这一头照旧画帧、认键——那是两条线程，屏不因此冻住。
//!
//! 握手只有一处，就是 [`Gate`]：计算线程在它上面等，UI 线程往它上面说一个字并敲钟。
//! **没有 sleep、没有靠轮询撞运气**——UI 那一侧问的 [`Running::deciding`] 只决定
//! 屏上此刻画哪一副，答话走的是另一条路（[`Running::decide`]）。
//!
//! 会话退出那一条也从这里出去：[`Running::leave`] 把闩推到中止，**同一下也往那道闸上
//! 说一个中止**——不说的话，那条线程会一直等在闸上，而 `leave` 正等着 join 它。
//!
//! **退出会话走的是同一条路**，只是不经过键盘：[`Running::leave`] 直接把闩推到中止
//! （停车场 Q63）。会话退出时不能把一条还在往盘上写东西的线程扔在身后，
//! 而中止停在**页边界**上（ADR 0013 决定第 2 条），当前卷那格 `partial` 丢掉、
//! 最终位置一个字节都没动过——退出会话不该在盘上留下半卷。

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use anyhow::{Result, anyhow};
use tonefit::{Event, Instruction, Pass, Progress, ProgressSink, Report, Request};

use super::live::{Live, Resuming};

/// 会话跑过的那一趟：后台那条线程，加上它边跑边攒的东西。
///
/// **跑完不清空**：攒着的那一份报告要留到退出会话那一刻——照原格式印到 stdout 的就是它。
/// 再按一次试算或执行会把它整个换掉，一趟一份。
#[derive(Default)]
pub struct Running {
    /// 边跑边攒的那一份。还没跑过就是 `None`。
    live: Option<Arc<Mutex<Live>>>,
    /// 那条线程。收掉之后（或者还没起过）是 `None`。
    thread: Option<JoinHandle<Result<Report>>>,
    /// 这一趟的[闩](Latch)：观察者下一条事件回的就是它记着的那个字。
    ///
    /// **一趟一份**——[`start`](Self::start) 每次换一个新的。上一趟按下的停不该跟着漏到
    /// 下一趟去（同一条理由让库把它自己那个闩放在 `run` 的栈上，见
    /// `tonefit` 的 `progress::Events`）。还没跑过时它也在，只是没有人读——
    /// [`leave`](Self::leave) 因此不必先判一次有没有线程。
    latch: Arc<Latch>,
    /// 这一趟的[闸](Gate)：**决策点上等人的那一趟**才有，别的趟是 `None`。
    ///
    /// 与闩同一条寿命，一趟一份（[`start`](Self::start) 每次换一个新的）：
    /// 上一趟在决策点上答过的字不该跟着漏到下一趟去。
    /// `None` 说的是「这一趟在决策点上不等人」——多卷试算与执行走的都是那一支，
    /// 那时决策点照旧由 [`answer`] 当场答字。
    gate: Option<Arc<Gate>>,
}

impl Running {
    /// 起一趟。观察者在这里接上去——`Request` 的那一格由本层填，不由状态机填。
    ///
    /// 上一趟攒的那一份当场换掉：一趟一份，两趟的报告混在一起没有意义。
    ///
    /// `resumes` 说的是**这一趟在决策点上等不等人**（ADR 0012 决定第 3 条：等不等人是
    /// 调用方的策略）。判它的是 [`super::resuming`]——那一层看得见处理范围有几卷，
    /// 本层只照它说的办：等人的那一趟多一道[闸](Gate)，不等人的那一趟一格不多。
    pub fn start(&mut self, mut request: Request, resumes: Resuming) {
        // 上一趟的线程一定已经收掉了：跑着的时候按键表根本不派「起一趟」
        // （见 `super::state::running_action`）。这里不靠那条远处的不变量——
        // 真漏了的话调试构建当场断掉，发布构建也是先 join 再起，
        // 而不是把一条还在往盘上写字节的线程悄悄甩掉。
        debug_assert!(self.thread.is_none(), "上一趟还没收掉就起了第二趟");
        self.collect();
        let live = Arc::new(Mutex::new(Live::new(&request, resumes)));
        // 闩换一个新的：上一趟按下的停到此为止（见 [`Self::latch`]）。
        // 会话那一侧同一刻也把它那一份归零（`super::state::Session::run_started`）。
        let latch = Arc::new(Latch::default());
        self.latch = Arc::clone(&latch);
        let gate = matches!(resumes, Resuming::Waits).then(|| Arc::new(Gate::default()));
        self.gate = gate.clone();
        request.progress = Some(ProgressSink::new(Watch {
            live: Arc::clone(&live),
            latch,
            gate,
        }));
        self.live = Some(live);
        self.thread = Some(std::thread::spawn(move || tonefit::run(&request)));
    }

    /// **按停**：把这一趟的闩推到 `level`（ADR 0013）。
    ///
    /// 按到哪一级由状态机记着（`super::state::Session::stopping`），这里只把那个字
    /// 送到计算线程上——本层不认键，状态机不碰线程。
    ///
    /// **只升不降**：推一个更弱的字进来不作数（[`Latch::raise`] 走 `fetch_max`）。
    /// 走这条路进来的只有两处，各自都只往上推：`s` 那个键升一级，
    /// [`leave`](Self::leave) 直接推到中止。
    ///
    /// **中止连那道[闸](Gate)一起推开**：那条线程可能正停在决策点上等人，而它在那儿
    /// 读不到闩——不推的话它会一直等下去，而按下中止的那一头（`Ctrl-C` 退出会话）
    /// 正等着 join 它。只推中止、不推收尾，与 [`answer`] 那条规矩是同一句话：
    /// 决策点上收尾要让，中止不让。收尾在这里让给的是**用户当场那个字**——
    /// 闸上还等着人答话，那一问不该由闩替他答。
    pub fn stop(&self, level: Instruction) {
        self.latch.raise(level);
        if level == Instruction::Abort
            && let Some(gate) = &self.gate
        {
            gate.say(Instruction::Abort);
        }
    }

    /// **这一趟此刻停在决策点上等人吗**（`p1-session/14`）。
    ///
    /// 会话每帧问一次，跑着与等答话之间那一下转场就是靠它（见 `super::drive`）。
    /// 不等人的那一趟恒是 `false`——它连闸都没有。
    pub fn deciding(&self) -> bool {
        self.gate.as_ref().is_some_and(|gate| gate.waiting())
    }

    /// **把用户当场答的那个字送到决策点上**，并记进 [`Live`]。
    ///
    /// 与 [`stop`](Self::stop) 同一条分工：认键在状态机（`super::state::deciding_action`），
    /// 本层只把那个字送到计算线程上。记进 [`Live`] 是因为屏上与报告抬头都要它——
    /// 单卷试算答了收尾，那一趟就等于一次 dry-run（见 `Live::mode`）。
    ///
    /// **它不是闩**：决策点问的是「这一卷的第二遍还做不做」，答完那条线程接着跑；
    /// 闩问的是「这一趟还走不走」，那一路走 [`stop`](Self::stop)。
    pub fn decide(&self, said: Instruction) {
        if let Some(gate) = &self.gate {
            gate.say(said);
        }
        if let Some(live) = &self.live {
            Self::held(live).decide(said);
        }
    }

    /// 那条线程跑完了就收掉它，并把库交出来的那份报告（或者那条拒绝）记进 [`Live`]。
    ///
    /// 出的是「这一下收掉了吗」：`true` 的那一次调用方要把会话从
    /// [`Running`](super::state::Mode::Running) 放回浏览。**不阻塞**——
    /// 没跑完就原样返回，画下一帧去。
    pub fn reap(&mut self) -> bool {
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return false;
        }
        self.collect();
        true
    }

    /// 用户退出会话：让那一趟**中止**，等它收完手再走（停车场 Q63）。
    ///
    /// 非等不可：这条线程正往盘上写东西，`main` 一返回它就被连根拔掉，
    /// 盘上留下的是一格写了一半的 `partial`。中止停在页边界上，等的是一页的功夫。
    ///
    /// 走中止而不是收尾：收尾要等当前卷跑完，那可能是几十分钟，而用户按的是「退出」。
    /// 手动按过 `s` 之后再退出仍是中止——闩[只升不降](Self::stop)。
    pub fn leave(&mut self) {
        self.stop(Instruction::Abort);
        self.collect();
    }

    /// 把那条线程 join 掉，结果交给 [`Live`]。没有线程就什么都不做。
    fn collect(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        // 恐慌是那条线程自己的事，会话不该跟着倒——它还得把终端还回去、把报告印出来。
        let done = thread
            .join()
            .unwrap_or_else(|_| Err(anyhow!("处理那一趟恐慌了：这一趟没有报告")));
        if let Some(live) = &self.live {
            Self::held(live).returned(done);
        }
    }

    /// 这一趟的闩此刻记着哪一级。**只给用例用**——真会话里没有人问它，
    /// 屏上那两行照状态机那一份写（`super::state::Session::stopping`）。
    /// 库外造不出一条 [`Event`] 来（它两级非穷尽），用例因此问不动
    /// [`Watch::observe`] 本身；观察者**回什么**由 [`answer`] 单独答，那一个测得动。
    #[cfg(test)]
    pub(super) fn pressed(&self) -> Instruction {
        self.latch.get()
    }

    /// 攒着的那一份。还没跑过就是 `None`。
    ///
    /// 借的是锁：画一帧的工夫里计算线程报到会等在这里。那是这一处**唯一**的代价，
    /// 而它有界——画一帧不等任何人。
    pub fn live(&self) -> Option<MutexGuard<'_, Live>> {
        self.live.as_ref().map(Self::held)
    }

    /// 中毒了照样用：里面是一份攒着的报告，一条线程恐慌不该让主区从此哑掉
    /// （与命令行那一侧的 `Bar::held` 同一条规矩）。
    fn held(live: &Arc<Mutex<Live>>) -> MutexGuard<'_, Live> {
        live.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 退出会话时交出去的那个数，**与命令行那一路一致**（见 [`Live::exit_code`]）。
    /// 一趟都没跑过就是全部成功那个数——什么都没做，不是失败。
    pub fn exit_code(&self) -> u8 {
        self.live()
            .map_or(crate::SUCCESS_EXIT, |live| live.exit_code())
    }

    /// 退出会话时印到 stdout 的那份报告，**照命令行那一路的原格式**
    /// （[`crate::render::report`]，四段一次性拼起来）。
    ///
    /// **没做成**的那一趟没有报告可印，与命令行同一条：那一趟 `run` 返回的是错误本身，
    /// stdout 上一个字节都没有。一趟都没跑过同理。
    pub fn report(&self) -> Option<String> {
        let live = self.live()?;
        if live.undone().is_some() {
            return None;
        }
        Some(crate::render::report(live.report(), live.mode()))
    }
}

impl Drop for Running {
    /// 会话从任何一条路走掉，那条线程都得先收手：正常退出、`?` 半路返回、恐慌展开。
    /// 与 `Screen` 的 [`Drop`] 同一个理由。
    fn drop(&mut self) {
        self.leave();
    }
}

/// 会话这一侧的观察者：**把事件折进 [`Live`]，回一个指令。**
///
/// 它从计算线程上被调到，因此拿的是一把锁；锁里做的事只有「折一条事件」，
/// 不画、不等人——画是 UI 那条线程的事。回的那个字读的是[闩](Latch)，**不进那把锁**。
struct Watch {
    live: Arc<Mutex<Live>>,
    latch: Arc<Latch>,
    /// 决策点上等人的那一趟才有这道闸（见 [`Running::gate`]）。
    gate: Option<Arc<Gate>>,
}

impl Progress for Watch {
    fn observe(&self, event: Event<'_>) -> Instruction {
        // 借出去的锁到这一句末尾就还回去了：下一句等人或者读闩时手上一把锁都没有。
        // **等人那一支尤其非还不可**：会话那一头每帧都要借同一把锁画报告区，
        // 而这一等可能是几分钟（`progress` 那条硬规矩的同一个理由）。
        Running::held(&self.live).observe(&event);
        // **两支都先过一遍 [`answer`]**：那条规矩（决策点上收尾要让、中止不让）
        // 因此仍旧只有一个出处，等人不等人只决定**让给谁**。
        match (at_the_decision_point(&event), &self.gate) {
            // 决策点，而且这一趟等人（ADR 0012 决定第 3 条：等不等人是调用方的策略）。
            (true, Some(gate)) => match answer(true, self.latch.get()) {
                // 中止不让，因此这里一句话都不必问：那一级要的就是当前卷等于没做，
                // 而人早就按下去了。`Running::stop` 把中止也推到闸上，那一处管的是
                // **已经等在闸上**的那条线程；这一处管的是它还没走到这儿的那一半。
                Instruction::Abort => Instruction::Abort,
                // `answer` 让掉的那一下（收尾），与从没按过停的那一种：
                // 停在闸上，交回用户当场答的那个字。
                _ => gate.ask(),
            },
            // 别处，或者这一趟不等人：照闩答，决策点上的收尾在那一支让成继续。
            (at_the_decision_point, _) => answer(at_the_decision_point, self.latch.get()),
        }
    }
}

/// 这一条事件是不是**决策点**——每一卷「汇总之后、第二遍之前」那一次问话
/// （ADR 0012 决定第 2 条，`CONTEXT.md` 的《会话》：决策点）。
///
/// 库那一侧只有这一条事件的答复**当场作数**，其余的都只进闩；[`answer`] 因此只在
/// 这一条上分岔。判据是事件本身，不是数到第几条——数下去的话，多一条事件就错位。
fn at_the_decision_point(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::PassStarted {
            pass: Pass::Second,
            ..
        }
    )
}

/// 会话在一条事件上回哪个字：**闩记着的那一级，只有决策点上的收尾要让**。
///
/// 让的理由是两处问的不是同一件事（`CONTEXT.md` 的《会话》：决策点不是第三个检查点）。
/// 闩答的是「这一趟还走不走」；决策点问的是「**这一卷的第二遍还做不做**」。
/// 拿闩去答决策点，第一遍里按下的**收尾**会顺手把当前卷的第二遍也吃掉——那一卷等于
/// 走了一次试算、盘上一个字节都没写，而收尾的定义正是「当前卷跑完才停」
/// （ADR 0013 决定第 1 条）。盘上会因此少一整卷。
///
/// **中止在决策点上不让**：那一级要的就是当前卷等于没做（ADR 0013 决定第 2 条），
/// 与页边界上按下它一个待遇。
///
/// 让掉的那一下**不会丢**：答复照样进库那一侧的闩，而那是个 `fetch_max`——
/// 记一个更弱的字进去不作数，闩仍是收尾，当前卷跑完之后卷边界那个检查点照样停。
///
/// **这里不等人**：单卷试算完了停下来问用户归 `p1-session/14`，本票只管那一下按停
/// 不要把当前卷吃掉。
fn answer(at_the_decision_point: bool, pressed: Instruction) -> Instruction {
    match pressed {
        Instruction::Finish if at_the_decision_point => Instruction::Continue,
        pressed => pressed,
    }
}

/// **决策点上等人的那道闸**：计算线程停在这里，会话那一头往上面说一个字
/// （`p1-session/14`，ADR 0012 决定第 3 条）。
///
/// 用条件变量而不是别的：这一头要等到答话为止，而「等到某件事成立」正是它的形状。
/// 轮询一个原子量也做得到，但那要靠 sleep 撑着——sleep 短了空转、长了迟钝，
/// 而两个数都是猜出来的。
///
/// **它与[闩](Latch)是两件事**，两者都在，各答各的问题：闩答「这一趟还走不走」，
/// 一路只升不降；这道闸答「这一卷的第二遍还做不做」，一问一答、答完就清空
/// （`CONTEXT.md` 的《会话》：决策点不是第三个检查点）。
///
/// **答话可以先到**：会话退出时 [`Running::stop`] 往这里说一个中止，而那时计算线程
/// 可能还没走到决策点。那个字因此留在 [`Asked::said`] 上等着——[`ask`](Self::ask)
/// 一进来就把它取走，一秒都不等。反过来漏掉这一条的话，`leave` 会 join 一条永远
/// 等在闸上的线程。
#[derive(Debug, Default)]
struct Gate {
    asked: Mutex<Asked>,
    /// 答话敲的那一下钟。
    answered: Condvar,
}

/// 闸上此刻的样子。
#[derive(Debug, Default)]
struct Asked {
    /// 计算线程正停在决策点上等着吗。屏上画哪一副由它定（[`Running::deciding`]）。
    waiting: bool,
    /// 会话答的那个字，还没答就是 `None`。取走即清空——下一个决策点重新问。
    said: Option<Instruction>,
}

impl Gate {
    /// **计算线程停在这里等人**，交回用户答的那个字。
    ///
    /// 先落座再等：`waiting` 与那一等之间一把锁都没松过，因此屏上不会看到
    /// 「等着」而实际早就走了那种中间态。答话先到的那一种（见类型文档）连座都不落——
    /// `wait_while` 的判据一进来就不成立，当场取走那个字返回。
    ///
    /// 中毒了照样答：里面是一个字，一条线程恐慌不该让另一条从此等不到人
    /// （与 [`Running::held`] 同一条规矩）。
    fn ask(&self) -> Instruction {
        let mut asked = self.held();
        asked.waiting = true;
        let mut asked = self
            .answered
            .wait_while(asked, |asked| asked.said.is_none())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        asked.waiting = false;
        // 取走即清空：一趟里每一卷各有一个决策点，上一个答的字不该替下一个作答。
        asked.said.take().unwrap_or(Instruction::Continue)
    }

    /// **会话答话**：把那个字摆上去、敲钟。等着的那条线程当场醒。
    fn say(&self, said: Instruction) {
        self.held().said = Some(said);
        self.answered.notify_all();
    }

    /// 此刻有线程停在这里**等着人答话**吗。
    ///
    /// **答上了就不算等着了**，哪怕那条线程还没被调度回来：`say` 与 `ask` 之间隔着
    /// 一次重新抢锁，而屏上那一副是每帧问一次这个函数画出来的（`super::drive`）。
    /// 只看 `waiting` 那一格的话，答完话到那条线程醒过来这中间的某一帧上，
    /// 屏底会把答话那两个键再摆一次——而那时已经没有人收了。
    fn waiting(&self) -> bool {
        let asked = self.held();
        asked.waiting && asked.said.is_none()
    }

    fn held(&self) -> MutexGuard<'_, Asked> {
        self.asked
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 会话这一侧的**闩**：用户按停按到过的最强那一级（ADR 0013）。
///
/// 「闩」这个说法出自 `CONTEXT.md` 的《进度》（「按停是个闩」）。库那一侧有一个同性质的
/// （`tonefit` 的 `progress::Standing`，记的是**观察者答过**什么），但它是 `pub(crate)` 的，
/// 二进制 crate 够不着——这一份记的也是另一件事：**用户按过**什么。
/// 两者的**序**出自同一处，[`Instruction`] 派生的 `Ord`；这里只是把那个序编成一个字节。
///
/// 用原子量而不是锁，与库那一侧同一条理由：它从 UI 线程写、从计算线程读，
/// 而计算线程读它的那一刻正是报到那一刻——拿锁来记，`progress` 那条
/// 「不在持锁处调观察者」的硬规矩当场就多了一处要守。
#[derive(Debug, Default)]
struct Latch(AtomicU8);

impl Latch {
    /// 往上推一级。**`fetch_max` 把「只升不降」写进了操作本身**：
    /// 推一个更弱的字进来不作数，按停因此是个闩，不是一个可以反悔的开关。
    fn raise(&self, level: Instruction) {
        self.0.fetch_max(code(level), Ordering::Relaxed);
    }

    fn get(&self) -> Instruction {
        from_code(self.0.load(Ordering::Relaxed))
    }
}

/// 记进原子量的那个数。手写而不是 `as u8`：那样写，「派生出来的序」与「记下去的数」
/// 的一致靠的是变体的书写顺序，改一次顺序两者就悄悄分家（库那一侧同一条理由）。
/// 两者由 [`tests::the_latch_only_ever_goes_up`] 拴在一起。
fn code(level: Instruction) -> u8 {
    match level {
        Instruction::Continue => 0,
        Instruction::Finish => 1,
        Instruction::Abort => 2,
    }
}

/// 从原子量里读回来。越界的数按最强的算——那一侧宁可多停一趟，不可漏停一趟。
fn from_code(code: u8) -> Instruction {
    match code {
        0 => Instruction::Continue,
        1 => Instruction::Finish,
        _ => Instruction::Abort,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::session::live::fixture;
    use tonefit::{Mode as RunMode, Profile, RunOutcome};

    /// 等那一趟停到决策点上。真会话里这一步是「画一帧、问一次 `deciding`」转的那个圈
    /// （见 `super::super::drive`），用例不必画，只问。
    ///
    /// **等的是一个会成立的条件，不是一段猜出来的时长**：那条线程一定会走到决策点
    /// （单卷、`Mode::Process`），而它停在那儿之后 [`Running::deciding`] 恒为真——
    /// 转到为止即可，转多少圈不影响结论。`sleep` 反过来是猜：短了没等到、长了白等。
    fn until_deciding(running: &Running) {
        while !running.deciding() {
            std::thread::yield_now();
        }
    }

    /// 一趟处理：一个真跑得动的卷（见 [`fixture::a_real_volume`]），连同它的输出根。
    fn a_one_volume_run(workspace: &tempfile::TempDir) -> Request {
        Request {
            inputs: vec![fixture::a_real_volume(workspace.path(), "卷一")],
            output_root: workspace.path().join("出"),
            ..fixture::request(RunMode::Process)
        }
    }

    /// 输出根下面此刻有哪些名字。没建出来就是空的——**「连建都没建」与「建了是空的」
    /// 在这个形式上是同一个答案**，而两者本票都算「一个文件都没写」。
    fn landed(out: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(out)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// 等那一趟跑完。真会话里这一步是「画一帧、`reap` 一次」转的那个圈
    /// （见 `super::super::drive`），用例不必画，只 `reap`。
    fn until_done(running: &mut Running) {
        while !running.reap() {
            std::thread::yield_now();
        }
    }

    /// 一趟真跑起来：事件从那条线程上折进 [`Live`]，报告攒得出来，
    /// 退出码与命令行那一路一致，stdout 那一份**逐字**就是命令行印的那一份。
    #[test]
    fn a_run_goes_through_the_thread_and_comes_back_as_a_report() {
        // 一页加一个透传文件的卷（见 [`fixture::a_real_volume`]）：这一条要问的
        // （事件→报告→退出码→stdout）一件都不少。
        let workspace = tempfile::tempdir().expect("建得出临时目录");

        let mut running = Running::default();
        running.start(a_one_volume_run(&workspace), Resuming::GoesOn);
        until_done(&mut running);

        let live = running.live().expect("跑过一趟");
        assert!(live.ended(), "该收场了");
        assert_eq!(live.undone(), None, "这一趟做成了");
        assert_eq!(live.report().volumes.len(), 1, "事件流没攒出那一卷");
        // 全局条那两个数是预扫报的，而预扫排在开工那条事件之前（03 号票）。
        assert_eq!(live.overall().volumes, 1);
        drop(live);

        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
        let printed = running.report().expect("有报告可印");
        assert!(printed.starts_with("profile "), "{printed}");
    }

    /// **拒绝执行**：会话不退出，那句话留在 [`Live`] 上；stdout 一个字节都没有，
    /// 退出码是命令行那一路的 `1`。那条线程恐慌了走的是同一条路——两者都是
    /// 「这一趟没做成」，分得开它们的是那句话本身。
    #[test]
    fn a_refused_run_leaves_the_session_open_and_prints_nothing() {
        let mut running = Running::default();
        // 范围为空是 `run` 当场拒掉的四种之一，一条事件都不发。
        running.start(
            Request {
                inputs: Vec::new(),
                ..fixture::request(RunMode::DryRun)
            },
            Resuming::GoesOn,
        );
        until_done(&mut running);

        let live = running.live().expect("跑过一趟");
        let said = live.undone().expect("这一趟没做成");
        assert!(said.contains("处理范围为空"), "{said}");
        drop(live);

        assert_eq!(running.exit_code(), crate::REFUSED_EXIT);
        assert!(running.report().is_none(), "没做成的那一趟没有报告可印");
    }

    /// **闩只升不降**，编进原子量的那个数与 [`Instruction`] 的序对得上。
    ///
    /// 「按了中止之后再按收尾仍然是中止」这条性质在会话里有两道保险：键盘上没有那个键
    /// （`super::state::running_action` 在中止那一级派的是「没有意义」），
    /// 而就算有，`fetch_max` 也不让它降回去。这一条问的是第二道。
    #[test]
    fn the_latch_only_ever_goes_up() {
        // 编进去的那个数与派生出来的序一致——两者分家的话，`fetch_max` 就不是「取更强的」了。
        for (weaker, stronger) in [
            (Instruction::Continue, Instruction::Finish),
            (Instruction::Finish, Instruction::Abort),
            (Instruction::Continue, Instruction::Abort),
        ] {
            assert!(weaker < stronger, "{weaker:?} 该弱于 {stronger:?}");
            assert!(code(weaker) < code(stronger), "编进去的数反了");
        }
        for level in [
            Instruction::Continue,
            Instruction::Finish,
            Instruction::Abort,
        ] {
            assert_eq!(from_code(code(level)), level, "记进去再读回来变了");
        }
        // 越界的数按最强的算：宁可多停一趟，不可漏停一趟。
        assert_eq!(from_code(9), Instruction::Abort);

        let running = Running::default();
        assert_eq!(running.pressed(), Instruction::Continue, "起手没按过");
        running.stop(Instruction::Finish);
        assert_eq!(running.pressed(), Instruction::Finish);
        running.stop(Instruction::Abort);
        assert_eq!(running.pressed(), Instruction::Abort);
        running.stop(Instruction::Finish);
        assert_eq!(running.pressed(), Instruction::Abort, "闩退回收尾了");
        running.stop(Instruction::Continue);
        assert_eq!(running.pressed(), Instruction::Abort, "闩被抹掉了");
    }

    /// **决策点上的收尾要让成继续**，其余一律照闩答（ADR 0012 决定第 2 条）。
    ///
    /// 这一条是本模块唯一一处「回什么」的规矩，而它测得动——`Watch::observe` 本身
    /// 测不动（库外造不出一条事件来），因此那个规矩单独摆成 [`answer`]。
    ///
    /// 让的是**收尾**那一级，因为决策点问的是「这一卷的第二遍还做不做」而不是
    /// 「这一趟还走不走」。不让的话，第一遍里按一次 `s`，当前卷等于走了一次试算——
    /// 盘上一个字节都没写，而收尾说好的是「当前卷跑完才停」。
    #[test]
    fn only_the_decision_point_makes_a_finish_step_aside() {
        // 决策点上：收尾让成继续，另两级原样。
        assert_eq!(answer(true, Instruction::Continue), Instruction::Continue);
        assert_eq!(answer(true, Instruction::Finish), Instruction::Continue);
        assert_eq!(answer(true, Instruction::Abort), Instruction::Abort);

        // 别处：三级一律照闩答——那几处的答复只进闩，而停在哪一道边界上是管线的事。
        for pressed in [
            Instruction::Continue,
            Instruction::Finish,
            Instruction::Abort,
        ] {
            assert_eq!(answer(false, pressed), pressed, "{pressed:?} 在别处被改了");
        }
    }

    /// **卷跑到一半按一次收尾，那一卷仍旧整卷落盘**——决策点没有把它吃掉。
    ///
    /// 这一条走的是[真观察者](Watch)，**不开线程也不掐表**（spec 的《Testing Decisions》：
    /// 双向观察者最直接的好处）：套一层替身，在「一卷开工」那条事件上把闩推到收尾——
    /// 那正是「用户在这一卷跑着的时候按了一次 `s`」，而时机由用例定死，不靠跟线程抢。
    /// 按在更早的地方（开工那条事件之前）不问决策点：卷边界那个检查点当场就把整趟停了，
    /// 那也对，只是问不到本条要问的那一处。
    ///
    /// 让路那一步要是没了，这一卷就等于走了一次试算——报告照出、盘上一个字节都没有，
    /// 而收尾说好的是「当前卷跑完才停」（ADR 0013 决定第 1 条）。
    #[test]
    fn finishing_in_the_middle_of_a_volume_still_lets_that_volume_land_whole() {
        /// 一卷开工那一刻按一次 `s`，随后原样交给真正的观察者。
        struct FinishOnceTheVolumeStarts {
            watch: Watch,
            latch: Arc<Latch>,
        }

        impl Progress for FinishOnceTheVolumeStarts {
            fn observe(&self, event: Event<'_>) -> Instruction {
                if matches!(event, Event::VolumeStarted { .. }) {
                    self.latch.raise(Instruction::Finish);
                }
                self.watch.observe(event)
            }
        }

        // 一页加一个透传文件的卷（见 [`fixture::a_real_volume`]）：第二遍照样要走，
        // 它写的是那个透传成员。
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();
        let latch = Arc::new(Latch::default());
        let live = Arc::new(Mutex::new(Live::new(&request, Resuming::GoesOn)));
        let report = tonefit::run(&Request {
            progress: Some(ProgressSink::new(FinishOnceTheVolumeStarts {
                watch: Watch {
                    live,
                    latch: Arc::clone(&latch),
                    // 这一条走的是不等人那一支：它问的是「第一遍里按下的收尾会不会
                    // 把当前卷的第二遍吃掉」，而那正是 [`answer`] 那条规矩管的事。
                    gate: None,
                },
                latch: Arc::clone(&latch),
            })),
            ..request
        })
        .expect("按停不是失败");

        assert_eq!(latch.get(), Instruction::Finish, "闩该停在收尾这一级");
        assert_eq!(report.volumes.len(), 1, "当前卷被收尾吃掉了");
        assert!(
            out.join("卷一").join("说明.txt").is_file(),
            "收尾说好当前卷跑完才停，盘上却没有它"
        );
        assert_eq!(crate::exit_code(&report), crate::SUCCESS_EXIT);
    }

    /// **单卷试算停在决策点上，答继续就接着做第二遍**（`p1-session/14`，ADR 0012）。
    ///
    /// 三件事一次问齐：停下来那一刻**盘上什么都没有**（第二遍还没开始）、
    /// 停着的时候 [`Running::deciding`] 说得出「在等人」、答完那条线程接着跑并把这一卷写全。
    ///
    /// **停下来那一眼非看不可**：跑完再看只看得见最终的样子，而「此刻还没落盘」与
    /// 「落过盘又被收走了」在那个形式上分不开（同一个手法见 `tests/resume.rs`）。
    #[test]
    fn a_resuming_trial_waits_at_the_decision_point_and_goes_on_when_told_to() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::Waits);
        until_deciding(&running);

        assert!(running.deciding(), "停在决策点上，却说没在等人");
        assert_eq!(
            landed(&out),
            Vec::<String>::new(),
            "第二遍还没开始，盘上却有东西了"
        );
        // 停在这儿的那一卷手上有一份报告可画（停车场 Q52）——「主区把报告画出来
        // 等你拿主意」靠的就是它。
        let live = running.live().expect("跑过一趟");
        assert!(live.summarized().is_some(), "决策点上没有报告可画");
        assert!(
            live.report().volumes.is_empty(),
            "这一卷还没收摊，报告里却已经有它了"
        );
        assert_eq!(live.mode(), RunMode::DryRun, "答继续之前它等于一次 dry-run");
        drop(live);

        running.decide(Instruction::Continue);
        // **答上了就立刻不算等着了**，哪怕那条线程还没被调度回来：屏上那一副是每帧
        // 问一次 `deciding` 画出来的，慢一帧就会把答话那两个键再摆一次——而那时
        // 已经没有人收了（见 [`Gate::waiting`]）。这一问不等 `until_done`，
        // 问的正是那一帧。
        assert!(!running.deciding(), "答完话立刻问，它还说在等人");
        until_done(&mut running);

        assert!(!running.deciding(), "跑完了还说在等人");
        assert_eq!(landed(&out), ["卷一"], "答了继续，这一卷却没写出来");
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.decided(), Some(Instruction::Continue));
        assert_eq!(live.mode(), RunMode::Process, "答了继续还印成 dry-run");
        assert_eq!(live.report().volumes.len(), 1);
        assert!(
            live.summarized().is_none(),
            "这一卷收摊了，那份「到此刻为止」还摆着"
        );
        drop(live);
        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
    }

    /// **决策点上答收尾＝一次 dry-run**（`p1-session/14` 票面第三条）：
    /// 输出根一个文件都没有，而**报告照出**（ADR 0012，`CONTEXT.md` 的《会话》：决策点）。
    ///
    /// 报告那一半非钉不可：收尾与中止的分界线就在它上面——停在决策点上的那一卷
    /// **做过事**（判定、逐页结果、解码计数都是真的），只有 `timing.second_pass` 是零。
    /// 那正是试算要看的那份东西。
    #[test]
    fn answering_finish_at_the_decision_point_writes_nothing_and_still_reports_the_volume() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::Waits);
        until_deciding(&running);
        running.decide(Instruction::Finish);
        until_done(&mut running);

        assert_eq!(
            landed(&out),
            Vec::<String>::new(),
            "答了收尾，盘上却写了东西"
        );
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.report().volumes.len(), 1, "答收尾的那一卷该照出报告");
        assert_eq!(
            live.report().volumes[0].timing.second_pass,
            std::time::Duration::ZERO,
            "第二遍一步没走，那一格却不是零"
        );
        assert_eq!(
            live.mode(),
            RunMode::DryRun,
            "答了收尾，报告抬头却不说这一趟只算不写"
        );
        drop(live);
        // 按停不是失败，决策点上停下来同样不是。
        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
        let printed = running.report().expect("有报告可印");
        assert!(printed.contains("dry-run"), "{printed}");
    }

    /// **等答话的时候退出会话**：那条线程收得回来，那一卷等于没做（票面第六条）。
    ///
    /// 走的是[中止](Running::leave)那一级，与页边界上按下它一个待遇：那一卷不进报告、
    /// 最终位置上一个字节都没动过、`partial` 也没留下——第二遍还没开始，那一格连建都没建。
    ///
    /// **这一条同时是那道闸的死锁哨兵**：`leave` 要 join 一条正等在闸上的线程，
    /// 而闩它在那儿读不到。中止不连着把闸推开的话，这条用例会挂住而不是红
    /// （见 [`Running::stop`]）。
    #[test]
    fn leaving_while_the_decision_point_waits_throws_that_volume_away() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::Waits);
        until_deciding(&running);
        running.leave();

        assert_eq!(running.pressed(), Instruction::Abort);
        let live = running.live().expect("跑过一趟");
        assert!(live.ended(), "那条线程还等在闸上没回来");
        assert_eq!(live.undone(), None, "退出会话不是「这一趟没做成」");
        assert!(
            live.report().volumes.is_empty(),
            "中止掉的那一卷进了报告：{:?}",
            live.report().volumes
        );
        assert!(
            live.summarized().is_none(),
            "那一卷已经作废，报告区还摆着它"
        );
        drop(live);
        assert_eq!(landed(&out), Vec::<String>::new(), "盘上留下了东西");
    }

    /// **按下中止之后那一趟不会停下来等人**（ADR 0013 决定第 2 条）：中止在决策点上不让，
    /// 而那一级要的就是当前卷等于没做——人早就按下去了，再问一句没有意义。
    ///
    /// 抢不抢得在那条线程走到决策点之前按下去不由用例说了算，**而两种收场是同一个答案**：
    /// 抢到了走 [`Watch::observe`] 那一处短路（闩已是中止，一句话都不问），
    /// 没抢到走 [`Running::stop`] 推开的那道闸（它已经等在上面了）。两条都不该挂住，
    /// 也都不该把这一卷留在盘上或报告里。
    #[test]
    fn an_abort_pressed_before_the_decision_point_does_not_stop_to_ask() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::Waits);
        // 真会话里这两下由 `s` 那个键派下来（见 `super::press`）：收尾 → 中止。
        running.stop(Instruction::Finish);
        running.stop(Instruction::Abort);
        until_done(&mut running);

        assert!(!running.deciding(), "按过中止了还停下来等人");
        let live = running.live().expect("跑过一趟");
        assert!(live.ended(), "那条线程还等在闸上没回来");
        assert_eq!(live.undone(), None, "按停不是「这一趟没做成」");
        assert!(
            live.report().volumes.is_empty(),
            "中止掉的那一卷进了报告：{:?}",
            live.report().volumes
        );
        drop(live);
        assert_eq!(landed(&out), Vec::<String>::new(), "盘上留下了东西");
        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
    }

    /// **不续做的那一趟在决策点上不等人**：闸根本不在，那一条照 [`answer`] 当场答字。
    ///
    /// 多卷试算与执行走的都是这一支。它与上面三条的差只有 [`Running::start`] 那个参数——
    /// 判它的是 `super::resuming`，本层只照办。
    #[test]
    fn a_run_that_does_not_resume_never_waits_for_anybody() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::GoesOn);
        until_done(&mut running);

        assert!(!running.deciding(), "不续做的那一趟停下来等人了");
        assert_eq!(landed(&out), ["卷一"], "没人拦它，它却没走到底");
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.decided(), None, "没人在决策点上作过答");
        assert_eq!(
            live.mode(),
            RunMode::Process,
            "这一趟真写了东西，却印成 dry-run"
        );
    }

    /// **退出会话走的是中止那一级**（停车场 Q63）：不干等当前卷跑完，
    /// 盘上也不留半卷。
    #[test]
    fn leaving_the_session_aborts_whatever_is_running() {
        let mut running = Running::default();
        running.leave();

        assert_eq!(running.pressed(), Instruction::Abort);
    }

    /// **起下一趟把上一趟按下的停丢掉**：闩一趟一份。
    ///
    /// 不换的话，第一趟按过中止之后，同一个会话里再按执行会当场被自己上一趟的那一下停掉。
    #[test]
    fn a_second_run_does_not_inherit_the_stop_pressed_in_the_first() {
        let mut running = Running::default();
        running.start(
            Request {
                inputs: Vec::new(),
                ..fixture::request(RunMode::DryRun)
            },
            Resuming::GoesOn,
        );
        running.stop(Instruction::Abort);
        until_done(&mut running);
        assert_eq!(running.pressed(), Instruction::Abort);

        running.start(
            Request {
                inputs: Vec::new(),
                ..fixture::request(RunMode::DryRun)
            },
            Resuming::GoesOn,
        );

        assert_eq!(
            running.pressed(),
            Instruction::Continue,
            "上一趟按下的停漏到下一趟去了"
        );
        until_done(&mut running);
    }

    /// **按停之后仍然收得回来**：那条线程回来了，退出码照旧是命令行那一路的数，
    /// 盘上不留半卷（ADR 0013 决定第 2 条）。
    ///
    /// 抢不抢得在它跑完之前按下去不由用例说了算——这一卷小到几毫秒就走完。
    /// 两种收场因此都对：抢到了是「按停」，没抢到是「走到头」。要问的那三件事
    /// 在两种收场下是同一个答案，而**停出来的现场对不对**是库那一侧的事
    /// （`tests/events.rs` 的 `aborting_at_a_page_boundary_throws_the_partial_container_away`
    /// 那一批，观察者当场答字，不开线程也不掐表）。
    #[test]
    fn a_stopped_run_comes_back_with_the_same_exit_code_and_no_half_volume() {
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let request = a_one_volume_run(&workspace);
        let out = request.output_root.clone();

        let mut running = Running::default();
        running.start(request, Resuming::GoesOn);
        // 按两次：收尾 → 中止。真会话里这两下由 `s` 那个键派下来（见 `super::press`）。
        running.stop(Instruction::Finish);
        running.stop(Instruction::Abort);
        until_done(&mut running);

        let live = running.live().expect("跑过一趟");
        assert!(live.ended(), "按停之后那条线程没回来");
        assert_eq!(live.undone(), None, "按停不是「这一趟没做成」");
        assert!(
            matches!(
                live.report().outcome,
                RunOutcome::Stopped(Instruction::Abort) | RunOutcome::Completed
            ),
            "收场说不清：{:?}",
            live.report().outcome
        );
        drop(live);

        // 退出码照旧走 `crate::exit_code`：按停是用户自己的决定，不是失败。
        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
        // 中止丢掉的是**它自己建的**那格 `partial`，最终位置上一个字节都没动过。
        let left_behind: Vec<_> = std::fs::read_dir(&out)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("partial"))
            .collect();
        assert!(left_behind.is_empty(), "盘上留下了半卷：{left_behind:?}");
    }

    /// 一趟都没跑过：退出码是全部成功那个数，stdout 上一个字节都没有。
    #[test]
    fn a_session_that_never_ran_anything_exits_clean_and_silent() {
        let running = Running::default();

        assert_eq!(running.exit_code(), crate::SUCCESS_EXIT);
        assert!(running.report().is_none());
    }

    /// 起第二趟把上一趟整个换掉：一趟一份，两趟的报告混在一起没有意义。
    #[test]
    fn starting_a_second_run_replaces_the_first() {
        let mut running = Running::default();
        running.start(
            Request {
                inputs: Vec::new(),
                profile: Profile::resolve("boox-poke6").expect("内置型号"),
                ..fixture::request(RunMode::DryRun)
            },
            Resuming::GoesOn,
        );
        until_done(&mut running);
        assert_eq!(
            running.live().expect("跑过").report().profile.to_string(),
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .to_string()
        );

        running.start(
            Request {
                inputs: vec![PathBuf::from("库/不在的卷")],
                ..fixture::request(RunMode::DryRun)
            },
            Resuming::GoesOn,
        );
        until_done(&mut running);

        let live = running.live().expect("跑过");
        assert_eq!(
            live.report().profile.to_string(),
            Profile::resolve("kobo-libra-2")
                .expect("内置型号")
                .to_string(),
            "上一趟的报告没被换掉"
        );
    }
}
