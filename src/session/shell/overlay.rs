//! **覆盖层**的画法：底下整屏压暗，全部按键那一张画在上面（`CONTEXT.md` 的《会话》：覆盖层；
//! 设计稿 `drawHelp`）。
//!
//! 那一张上列什么、怎么分栏、从第几行画起收到哪儿，都在特性外面的 [`Sheet`]（出自按键表）；
//! 这里只画框、把摆好的行写进去、右框线上画滚动条。抬头右端那一句「? Esc → 关闭」的两个键
//! 也从表上取：掀开它的那个键再按一次关掉它、`Esc` 是屏底那一件（`q` 也关，抬头照设计稿不提它；
//! 掀开它的那个键在覆盖层上真派关掉，`super::super::cover` 的用例守着）。

use ratatui::layout::Rect;

use super::super::cover::{Overlay, Sheet};
use super::super::keymap::{self, Deed, Phase, Want};
use super::super::look::{Kind, Look, Segment};
use super::super::state::Session;
use super::super::view::{Focus, Window};
use super::super::viewport::Scrollbar;
use super::canvas::{Border, Canvas};

/// 画覆盖层：没掀着什么都不画。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, phase: Phase) {
    let Some(Overlay::Keys { from }) = session.views.cover else {
        return;
    };
    canvas.dim_all();
    let sheet = Sheet::of(
        phase,
        Window {
            cols: canvas.width(),
            rows: canvas.height(),
        },
    );
    let from = sheet.from(from);
    let shown = sheet.shown();
    let total = sheet.lines.len();
    let at = sheet.placement;
    let area = Rect::new(at.x, at.y, at.width, at.height);
    let look = Look::kind(Kind::Focus);
    let closing = keymap::hints(phase, Focus::Overlay, &[Want::of(Deed::CloseOverlay)]);
    let caption = match (keymap::spelt_for(Deed::Help), closing.first()) {
        (Some(again), Some(close)) => format!("{again} {} → {}", close.spelt(), close.what),
        _ => String::new(),
    };
    canvas.frame(
        area,
        &Border {
            thick: true,
            look,
            title: &[
                Segment::new("全部按键", Look::PLAIN.bold()),
                Segment::faint(" ⋅ vim 风格 ⋅ 只列当前可用的"),
            ],
            right: &[Segment::faint(caption)],
            bottom_left: &[],
            bottom_right: &[Segment::new(
                format!("{}–{} of {total}", from + 1, total.min(from + shown)),
                look,
            )],
        },
    );
    if total > shown {
        canvas.scrollbar(
            area,
            &Scrollbar {
                rows: total,
                at: from,
                window: shown,
            },
        );
    }
    let column = sheet.column_width();
    for (i, (left, right)) in sheet.lines.iter().enumerate().skip(from).take(shown) {
        let y = area.y + 1 + (i - from) as u16;
        canvas.line(area.x + 3, y, left, Some(column.saturating_sub(2)));
        if sheet.two_columns {
            canvas.line(
                area.x + 3 + column + 1,
                y,
                right,
                Some(column.saturating_sub(2)),
            );
        }
    }
}
