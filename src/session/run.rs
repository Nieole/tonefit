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
//! **退出会话走的是同一条路**，只是不经过键盘：[`Running::leave`] 直接把闩推到中止
//! （停车场 Q63）。会话退出时不能把一条还在往盘上写东西的线程扔在身后，
//! 而中止停在**页边界**上（ADR 0013 决定第 2 条），当前卷那格 `partial` 丢掉、
//! 最终位置一个字节都没动过——退出会话不该在盘上留下半卷。

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use anyhow::{Result, anyhow};
use tonefit::{Event, Instruction, Pass, Progress, ProgressSink, Report, Request};

use super::live::Live;

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
}

impl Running {
    /// 起一趟。观察者在这里接上去——`Request` 的那一格由本层填，不由状态机填。
    ///
    /// 上一趟攒的那一份当场换掉：一趟一份，两趟的报告混在一起没有意义。
    pub fn start(&mut self, mut request: Request) {
        // 上一趟的线程一定已经收掉了：跑着的时候按键表根本不派「起一趟」
        // （见 `super::state::running_action`）。这里不靠那条远处的不变量——
        // 真漏了的话调试构建当场断掉，发布构建也是先 join 再起，
        // 而不是把一条还在往盘上写字节的线程悄悄甩掉。
        debug_assert!(self.thread.is_none(), "上一趟还没收掉就起了第二趟");
        self.collect();
        let live = Arc::new(Mutex::new(Live::new(&request)));
        // 闩换一个新的：上一趟按下的停到此为止（见 [`Self::latch`]）。
        // 会话那一侧同一刻也把它那一份归零（`super::state::Session::run_started`）。
        let latch = Arc::new(Latch::default());
        self.latch = Arc::clone(&latch);
        request.progress = Some(ProgressSink::new(Watch {
            live: Arc::clone(&live),
            latch,
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
    pub fn stop(&self, level: Instruction) {
        self.latch.raise(level);
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
}

impl Progress for Watch {
    fn observe(&self, event: Event<'_>) -> Instruction {
        // 借出去的锁到这一句末尾就还回去了：下一句读闩时手上一把锁都没有。
        Running::held(&self.live).observe(&event);
        answer(at_the_decision_point(&event), self.latch.get())
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
        // 只装一个透传文件的卷：一页都没有，因此不必在用例里造图片，
        // 而这一票要问的（事件→报告→退出码→stdout）一件都不少。
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let volume = workspace.path().join("卷一");
        std::fs::create_dir_all(&volume).expect("建得出卷");
        std::fs::write(volume.join("说明.txt"), "透传").expect("写得出成员");

        let mut running = Running::default();
        running.start(Request {
            inputs: vec![volume],
            output_root: workspace.path().join("出"),
            ..fixture::request(RunMode::Process)
        });
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
        running.start(Request {
            inputs: Vec::new(),
            ..fixture::request(RunMode::DryRun)
        });
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

        // 只装一个透传文件的卷：第二遍照样要走（它写的是透传成员），
        // 而用例不必造图片。
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let volume = workspace.path().join("卷一");
        std::fs::create_dir_all(&volume).expect("建得出卷");
        std::fs::write(volume.join("说明.txt"), "透传").expect("写得出成员");
        let out = workspace.path().join("出");

        let request = Request {
            inputs: vec![volume],
            output_root: out.clone(),
            ..fixture::request(RunMode::Process)
        };
        let latch = Arc::new(Latch::default());
        let live = Arc::new(Mutex::new(Live::new(&request)));
        let report = tonefit::run(&Request {
            progress: Some(ProgressSink::new(FinishOnceTheVolumeStarts {
                watch: Watch {
                    live,
                    latch: Arc::clone(&latch),
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
        running.start(Request {
            inputs: Vec::new(),
            ..fixture::request(RunMode::DryRun)
        });
        running.stop(Instruction::Abort);
        until_done(&mut running);
        assert_eq!(running.pressed(), Instruction::Abort);

        running.start(Request {
            inputs: Vec::new(),
            ..fixture::request(RunMode::DryRun)
        });

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
        let volume = workspace.path().join("卷一");
        std::fs::create_dir_all(&volume).expect("建得出卷");
        std::fs::write(volume.join("说明.txt"), "透传").expect("写得出成员");
        let out = workspace.path().join("出");

        let mut running = Running::default();
        running.start(Request {
            inputs: vec![volume],
            output_root: out.clone(),
            ..fixture::request(RunMode::Process)
        });
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
        running.start(Request {
            inputs: Vec::new(),
            profile: Profile::resolve("boox-poke6").expect("内置型号"),
            ..fixture::request(RunMode::DryRun)
        });
        until_done(&mut running);
        assert_eq!(
            running.live().expect("跑过").report().profile.to_string(),
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .to_string()
        );

        running.start(Request {
            inputs: vec![PathBuf::from("库/不在的卷")],
            ..fixture::request(RunMode::DryRun)
        });
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
