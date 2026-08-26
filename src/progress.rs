//! 进度：长任务向外报到的地方（spec 的 story 30）。
//!
//! 库这一侧只报到，**印在哪、印不印、长什么样由调用方定**。理由与 `run` 那个 seam 是同一个：
//! 往库里塞一个终端组件，就等于让它替 CLI 决定输出的样子，而测试也没法在不开终端的情况下
//! 问「它到底报到了吗」。CLI 在 `main` 里把它接到 indicatif 上，用例接一个计数器上去。

use std::path::Path;
use std::sync::Arc;

/// 一次运行的进度观察者。
///
/// 报到的单位是**步**，不是页：一个卷要走的遍数随模式而变（幂等那一道、第一遍、第二遍），
/// 按页报到会让进度条在第一遍里一动不动、到第二遍猛地冲完。一个卷有多少步由管线那一侧算
/// （`crate::volume_steps`），观察者只管收。
pub trait Progress: Send + Sync {
    /// 一个卷开始了，这一卷总共要走 `steps` 步。
    fn volume_started(&self, volume: &Path, steps: u64);
    /// 又走完了一步。
    fn stepped(&self);
    /// 这一卷走完了。走完时未必恰好走满——幂等命中的卷剩下的两遍根本不做。
    fn volume_finished(&self);
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

/// 管线内部报到用的那一端：没有观察者时每一步都是空操作。
///
/// 调用处因此不必到处判空——那种判空写着写着就会漏掉一处，而漏掉的那一处正是进度条卡住的地方。
#[derive(Clone, Copy)]
pub(crate) struct Steps<'a> {
    sink: Option<&'a ProgressSink>,
}

impl<'a> Steps<'a> {
    pub(crate) fn new(sink: Option<&'a ProgressSink>) -> Self {
        Self { sink }
    }

    pub(crate) fn started(self, volume: &Path, steps: u64) {
        if let Some(sink) = self.sink {
            sink.0.volume_started(volume, steps);
        }
    }

    /// 走完一步。逐页、逐成员报到的那些地方用它。
    pub(crate) fn step(self) {
        if let Some(sink) = self.sink {
            sink.0.stepped();
        }
    }

    pub(crate) fn finished(self) {
        if let Some(sink) = self.sink {
            sink.0.volume_finished();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 记账用的观察者：报到什么就记什么。用例那一头留一份克隆，散场之后查得出来。
    ///
    /// 形状照 `tests/concurrency.rs` 的同名夹具办（句柄 + 一格共享记账）。那一个数的是
    /// 管线报到的步，这一个数的是 [`Steps`] 转手的次数，共用不了代码，至少共用一个样子。
    ///
    /// 开卷记的是**参数**而不只是次数：`volume_started` 带着卷路径与预告的步数两样东西，
    /// 只数次数的话，把卷报错、把总步数报错都不会红——而预告的步数报错正是进度条
    /// 「停在某个百分比上再也不动」的样子。
    #[derive(Clone, Default)]
    struct Tally(Arc<Counts>);

    #[derive(Default)]
    struct Counts {
        started: Mutex<Vec<(PathBuf, u64)>>,
        stepped: AtomicUsize,
        finished: AtomicUsize,
    }

    impl Progress for Tally {
        fn volume_started(&self, volume: &Path, steps: u64) {
            self.0
                .started
                .lock()
                .expect("记账没有中毒")
                .push((volume.to_owned(), steps));
        }

        fn stepped(&self) {
            self.0.stepped.fetch_add(1, Ordering::Relaxed);
        }

        fn volume_finished(&self) {
            self.0.finished.fetch_add(1, Ordering::Relaxed);
        }
    }

    impl Tally {
        fn started(&self) -> Vec<(PathBuf, u64)> {
            self.0.started.lock().expect("记账没有中毒").clone()
        }

        fn stepped(&self) -> usize {
            self.0.stepped.load(Ordering::Relaxed)
        }

        fn finished(&self) -> usize {
            self.0.finished.load(Ordering::Relaxed)
        }
    }

    /// 每一下报到都原样到得了装进去的那个观察者，此外哪儿都不到。
    ///
    /// 后半句只有拿前半句当参照才断言得出来：场上得先有一个收得到报到的观察者，
    /// 没装它的那一端走完之后它一动不动，那才叫没人收到。只调三下不断言，
    /// 测到的是「不恐慌」——而空操作与「悄悄少报了一步」都不恐慌，那个形式两者分不开。
    #[test]
    fn every_step_reaches_the_installed_observer_and_nowhere_else() {
        let tally = Tally::default();
        let sink = ProgressSink::new(tally.clone());

        let watched = Steps::new(Some(&sink));
        watched.started(Path::new("卷一"), 10);
        watched.step();
        watched.step();
        watched.finished();

        // 每一下都恰好到一次，带着的东西也没变样：多报一步进度条会冲过头，
        // 少报一步它会停在半路，而预告的步数报错则从头到尾都对不上。
        assert_eq!(
            tally.started(),
            vec![(PathBuf::from("卷一"), 10)],
            "开卷没有原样到达"
        );
        assert_eq!(tally.stepped(), 2, "走过的步没有原样到达");
        assert_eq!(tally.finished(), 1, "收摊没有原样到达");

        // 同一个记账本还在场上，而这一端没装它：三下报到一下都不该落到它那里。
        let unwatched = Steps::new(None);
        unwatched.started(Path::new("卷二"), 10);
        unwatched.step();
        unwatched.finished();

        assert_eq!(
            tally.started(),
            vec![(PathBuf::from("卷一"), 10)],
            "没装观察者，开卷却到了某处"
        );
        assert_eq!(tally.stepped(), 2, "没装观察者，步却到了某处");
        assert_eq!(tally.finished(), 1, "没装观察者，收摊却到了某处");
    }
}
