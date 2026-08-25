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

    /// 没有观察者时每一步都是空操作——库不会因为没人看着就走别的路。
    #[test]
    fn a_run_without_an_observer_reports_into_nowhere() {
        let steps = Steps::new(None);

        steps.started(Path::new("卷一"), 10);
        steps.step();
        steps.finished();
    }
}
