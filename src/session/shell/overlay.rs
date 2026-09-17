//! **覆盖层**的画法：底下整屏压暗，全部按键那一张画在上面（`CONTEXT.md` 的《会话》：覆盖层；
//! 设计稿 `drawHelp`）。
//!
//! 那一张上列什么、怎么分栏、从第几行画起收到哪儿，都在特性外面的 [`Sheet`]（出自按键表）；
//! 这里只画框、把摆好的行写进去、右框线上画滚动条。抬头右端那一句「? Esc → 关闭」的两个键
//! 也从表上取：掀开它的那个键再按一次关掉它、`Esc` 是屏底那一件（`q` 也关，抬头照设计稿不提它；
//! 掀开它的那个键在覆盖层上真派关掉，`super::super::cover` 的用例守着）。

use ratatui::layout::Rect;

use super::super::cover::{Card, Overlay, Sheet};
use super::super::keymap::{self, Deed, Phase, Want};
use super::super::look::{Kind, Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::tree::NoteKind;
use super::super::view::{Focus, Window};
use super::super::viewport::Scrollbar;
use super::canvas::{Border, Canvas};

/// 画覆盖层：没掀着什么都不画。**底下整屏先压暗**（设计稿 `drawAll` 的次序），再画那一张。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, phase: Phase) {
    let Some(cover) = session.views.cover else {
        return;
    };
    canvas.dim_all();
    match cover {
        Overlay::Keys { from } => keys(canvas, phase, from),
        Overlay::Note { node, at } => note(canvas, session, phase, node, at),
    }
}

/// **说明卡**：一条备注的全文（设计稿 `drawNote`）。备注不在树上（树刚换过）就不画。
fn note(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, node: usize, at: usize) {
    let window = Window {
        cols: canvas.width(),
        rows: canvas.height(),
    };
    let Some(said) = session.views.task.tree.note(node, at) else {
        return;
    };
    let card = Card::of(said, window, &session.home);
    let box_of_it = card.placement;
    // 无法访问那一种整张卡是出事色（框线与抬头），非漫画文件那一种是聚焦色加终端默认色
    // ——两处都问同一格（设计稿 `drawNote` 的 `bad`）。
    let bad = card.kind == NoteKind::Unreachable;
    let look = if bad {
        Look::tone(Tone::Trouble)
    } else {
        Look::kind(Kind::Focus)
    };
    canvas.frame(
        Rect::new(box_of_it.x, box_of_it.y, box_of_it.width, box_of_it.height),
        &Border {
            thick: true,
            look,
            title: &[Segment::new(
                &card.label,
                if bad {
                    Look::tone(Tone::Trouble).bold()
                } else {
                    Look::PLAIN.bold()
                },
            )],
            right: &[],
            bottom_left: &[],
            bottom_right: &[Segment::faint(closing(phase))],
        },
    );
    let room = Some(card.text_width());
    canvas.line(
        card.text_x(),
        box_of_it.y + 2,
        &[Segment::new(&card.what, Look::PLAIN.bold())],
        room,
    );
    for (i, line) in card.body.iter().enumerate() {
        canvas.line(
            card.text_x(),
            box_of_it.y + 4 + i as u16,
            &[Segment::new(line.as_str(), Look::kind(Kind::Prose))],
            room,
        );
    }
}

/// 覆盖层上**关掉它**那一件在屏上怎么写（`Esc → 关闭`），键与那一句都从按键表取。
fn closing(phase: Phase) -> String {
    keymap::hints(phase, Focus::Overlay, &[Want::of(Deed::CloseOverlay)])
        .first()
        .map(|said| format!("{} → {}", said.spelt(), said.what))
        .unwrap_or_default()
}

/// **全部按键那一张**（设计稿 `drawHelp`）。
fn keys(canvas: &mut Canvas<'_>, phase: Phase, from: usize) {
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
    let caption = match keymap::spelt_for(Deed::Help) {
        Some(again) => format!("{again} {}", closing(phase)),
        None => String::new(),
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
