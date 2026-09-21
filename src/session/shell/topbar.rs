//! **顶栏**：屏上第一行（`CONTEXT.md` 的《会话》：视图）。左边两个视图名（此刻那一个加粗加下划线，
//! 号是完成那一色），右边一块方括号：程序名与版本 · 型号 · 这一趟走逐页判断还是整卷统一灰阶。
//!
//! **人在配置视图时右边另带着这一趟的进度或「等待确认」**：看设置时不错过要人答话的那一刻
//! （spec 的 story 4）。任务视图上不带——那一屏的总览本来就把进度摆在正中间。

use std::time::Instant;

use super::super::keymap::Phase;
use super::super::live::Live;
use super::super::look::{Kind, Look, Segment, width_of};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::view::{Target, View};
use super::canvas::Canvas;

/// 画顶栏。
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
) {
    let mut x = 1;
    for (i, view) in [View::Task, View::Config].into_iter().enumerate() {
        if i > 0 {
            x = canvas.put(x, 0, " │ ", Look::FAINT);
        }
        let on = session.views.view == view;
        let number = if on {
            Look::kind(Kind::Done).bold()
        } else {
            Look::FAINT
        };
        let name = if on {
            Look::PLAIN.bold().underlined()
        } else {
            Look::FAINT
        };
        let start = x;
        x = canvas.put(x, 0, &format!("{} ", view.number()), number);
        x = canvas.put(x, 0, view.name(), name);
        canvas.hit(start, 0, x - start, Target::View(view));
    }
    let banner = Look::kind(Kind::Banner);
    let mut segments = vec![Segment::new("[ ", banner)];
    segments.extend(in_flight(session, live, phase, now));
    segments.extend([
        Segment::new(format!("tonefit {}", env!("CARGO_PKG_VERSION")), banner),
        Segment::new(
            format!(" ⋅ {} ⋅ {}", model(session), judging(session)),
            banner,
        ),
        Segment::new(" ]", banner),
    ]);
    let width = width_of(&segments) as u16;
    canvas.line(canvas.width().saturating_sub(1 + width), 0, &segments, None);
}

/// 人在配置视图时右端多出来的那一截：转轮与总进度，等待确认时换成那一句。
/// 任务视图上、以及这一趟收摊之后都没有它。
fn in_flight(session: &Session, live: Option<&Live>, phase: Phase, now: Instant) -> Vec<Segment> {
    if session.views.view == View::Task {
        return Vec::new();
    }
    let Some(live) = live.filter(|live| !live.ended()) else {
        return Vec::new();
    };
    if phase == Phase::Deciding {
        return vec![Segment::new(
            "? 等待确认 ⋅ ",
            Look::tone(Tone::Caution).bold(),
        )];
    }
    let overall = live.overall();
    let percent = match overall.steps {
        0 => 0,
        steps => overall.walked.saturating_mul(100) / steps,
    };
    vec![Segment::new(
        format!("{} 处理中 {percent}% ⋅ ", session.views.spinning(now)),
        Look::kind(Kind::Progress),
    )]
}

/// 顶栏与总览上写的型号：没挑就说没挑。
pub(super) fn model(session: &Session) -> String {
    session
        .device
        .profile
        .clone()
        .unwrap_or_else(|| "型号未选择".to_owned())
}

/// 这一趟走逐页判断还是整卷统一灰阶。
fn judging(session: &Session) -> &'static str {
    if session.taste.envelope() {
        "整卷统一灰阶"
    } else {
        "逐页判断"
    }
}
