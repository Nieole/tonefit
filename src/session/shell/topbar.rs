//! **顶栏**：屏上第一行（`CONTEXT.md` 的《会话》：视图）。左边两个视图名（此刻那一个加粗加下划线，
//! 号是完成那一色），右边一块方括号：程序名与版本 · 型号 · 这一趟走逐页判断还是整卷统一灰阶。
//! 人在配置视图时右边另带着这一趟的进度或「等待确认」（随配置视图那一票接上）。

use super::super::look::{Kind, Look, Segment, width_of};
use super::super::state::Session;
use super::super::view::View;
use super::canvas::Canvas;

/// 画顶栏。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session) {
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
        x = canvas.put(x, 0, &format!("{} ", view.number()), number);
        x = canvas.put(x, 0, view.name(), name);
    }
    let banner = Look::kind(Kind::Banner);
    let segments = [
        Segment::new("[ ", banner),
        Segment::new(format!("tonefit {}", env!("CARGO_PKG_VERSION")), banner),
        Segment::new(
            format!(" ⋅ {} ⋅ {}", model(session), judging(session)),
            banner,
        ),
        Segment::new(" ]", banner),
    ];
    let width = width_of(&segments) as u16;
    canvas.line(canvas.width().saturating_sub(1 + width), 0, &segments, None);
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
