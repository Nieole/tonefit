//! 起一趟、盯着它、收掉它。**会话里唯一起线程的地方。**
//!
//! 为什么要另起一条线程：[`tonefit::run`] 一进去就要跑到底，而会话这一头得接着画、
//! 接着认键——不然「跑起来时主区实时更新」就只剩一句话。库那一侧不需要知道这件事，
//! 它照旧在计算线程上报到（`progress` 那条硬规矩：观察者可能很久不返回，
//! 因此不在持锁处调它），这里收下每一条、折进 [`Live`]。
//!
//! # 观察者回的那个字
//!
//! 恒是[继续](Instruction::Continue)，**只有一个例外**：用户要退出会话时回
//! [中止](Instruction::Abort)。
//!
//! 那不是两级停——收尾与中止那两个**键**归 `p1-session/10`，本票一个都不占。
//! 这里只堵一个洞：会话退出时不能把一条还在往盘上写东西的线程扔在身后。
//! 走中止那一级是因为它停在**页边界**上（ADR 0013 决定第 2 条），当前卷那格 `partial`
//! 丢掉、最终位置一个字节都没动过——退出会话不该在盘上留下半卷。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use anyhow::{Result, anyhow};
use tonefit::{Event, Instruction, Progress, ProgressSink, Report, Request};

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
    /// 用户要退出会话了。观察者读它，读到就回[中止](Instruction::Abort)。
    ///
    /// 一个会话一份、按下去就不再放开：退出这件事没有反悔。
    leaving: Arc<AtomicBool>,
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
        request.progress = Some(ProgressSink::new(Watch {
            live: Arc::clone(&live),
            leaving: Arc::clone(&self.leaving),
        }));
        self.live = Some(live);
        self.thread = Some(std::thread::spawn(move || tonefit::run(&request)));
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

    /// 用户退出会话：让那一趟中止，**等它收完手再走**。
    ///
    /// 非等不可：这条线程正往盘上写东西，`main` 一返回它就被连根拔掉，
    /// 盘上留下的是一格写了一半的 `partial`。中止停在页边界上，等的是一页的功夫。
    pub fn leave(&mut self) {
        self.leaving.store(true, Ordering::Relaxed);
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
/// 不画、不等人——画是 UI 那条线程的事。
struct Watch {
    live: Arc<Mutex<Live>>,
    leaving: Arc<AtomicBool>,
}

impl Progress for Watch {
    fn observe(&self, event: Event<'_>) -> Instruction {
        Running::held(&self.live).observe(&event);
        // 退出会话时中止，其余一律继续。两级停那两个键归 `p1-session/10`（见模块抬头）。
        if self.leaving.load(Ordering::Relaxed) {
            Instruction::Abort
        } else {
            Instruction::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::session::live::fixture;
    use tonefit::{Mode as RunMode, Profile};

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
