//! **屏底**：屏上最后一行，恒一行（`GLOSSARY.md` 的《会话》：屏底）。
//!
//! 此刻按得动的几件事，每件写成 `[键 → 做什么]`，按轻重排（哪几件、什么次序在
//! [`Session::hints`]，每一件的键与那一句出自按键表）；摆不下时从倒数第二件往前舍，
//! `?` 全部按键恒在末尾。三样东西会临时占它：按下去之后的一句回话（几秒后退回）、
//! **输入行**（提示词 · 缓冲 · 光标，右端那几件同样从按键表取；覆盖层盖着它时屏底让给覆盖层，
//! 关掉之后原样回来）、连击键按了前半截时右端那个待续记号。输入行开着时说了一句回话，
//! 那一句画在输入行右端那几件的位置上，到点那几件退回来。

use super::super::columns::{self, Keep};
use super::super::keymap::Phase;
use super::super::live::Live;
use super::super::look::{Kind, Look, Segment, width_of};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::view::Focus;
use super::canvas::{Canvas, hint};
use std::time::Instant;

/// 待续记号占的宽度（`d…` 连同两边的空）。
const PENDING_ROOM: u16 = 6;

/// 输入行上的光标。
const CARET: &str = "▏";

/// 屏底那一行字**摆到倒数第三列为止**（设计稿 `footerRoom`）：按轻重排的那几件与回话都摆到这里。
/// 一句回话里有一截路径时，摆不摆得下照它算——灰阶测试图那一句的路径照它从中间省略
/// （`terminal::draw_a_chart`）。
pub(in crate::session) fn room(width: u16) -> u16 {
    width.saturating_sub(3)
}

/// 画屏底。
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
) {
    let y = canvas.height().saturating_sub(1);
    let width = canvas.width();
    if matches!(session.views.focus(), Focus::Input(_))
        && let Some(line) = &session.views.input
    {
        let prompt = line.purpose.prompt();
        let reply = session.views.reply(now);
        let mut groups = hints(session, phase, live);
        // 缓冲与光标先于提示：按用途派下来的次序从前往后让，全部让光才截缓冲。
        let left_width = 1
            + usize::from(crate::wrap::width(&prompt))
            + usize::from(crate::wrap::width(&line.buffer))
            + usize::from(crate::wrap::width(CARET));
        while reply.is_none()
            && !groups.is_empty()
            && left_width + 1 + spread(&groups) > usize::from(width.saturating_sub(1))
        {
            groups.remove(0);
        }
        // 回话照旧占右端那几件的位置，到点再退回提示。
        let right = match reply {
            Some(reply) => reply.to_vec(),
            None => spaced(groups),
        };
        let right_x = width.saturating_sub(1 + width_of(&right) as u16);
        let mut x = canvas.put(1, y, &prompt, Look::kind(Kind::Caption).bold());
        let buffer_room = right_x.saturating_sub(x + 1 + u16::from(!right.is_empty()));
        x = canvas.put(
            x,
            y,
            &columns::elide_at(&line.buffer, usize::from(buffer_room), Some(Keep::End)),
            Look::PLAIN,
        );
        canvas.put(x, y, CARET, Look::kind(Kind::Focus));
        canvas.line(right_x, y, &right, None);
        return;
    }
    let pending = session.views.pending(now);
    let space = room(width).saturating_sub(if pending.is_some() { PENDING_ROOM } else { 0 });
    let segments = match session.views.reply(now) {
        Some(reply) => reply.to_vec(),
        None => {
            let mut groups = hints(session, phase, live);
            // 摆不下从倒数第二件往前舍，末尾那一件（`?`）恒在。
            while groups.len() > 2 && spread(&groups) > usize::from(space) {
                groups.remove(groups.len() - 2);
            }
            spaced(groups)
        }
    };
    canvas.line(1, y, &segments, Some(space));
    if let Some(prefix) = pending {
        // 前半截那个字贴着右端倒数第二格，省略号占末一格（设计稿 `drawFooter`）。
        let x = width.saturating_sub(2 + crate::wrap::width(&prefix.to_string()));
        canvas.put(
            x,
            y,
            &format!("{prefix}…"),
            Look::tone(Tone::Caution).bold(),
        );
    }
}

/// 此刻按得动的那几件，一件一组 `[键 → 做什么]`（哪几件、什么次序在 [`Session::hints`]）。
fn hints(session: &Session, phase: Phase, live: Option<&Live>) -> Vec<Vec<Segment>> {
    session
        .hints(phase, live)
        .iter()
        .map(|said| hint(&said.spelt(), said.what))
        .collect()
}

/// 几件排成一行，件与件之间一个空。
fn spaced(groups: Vec<Vec<Segment>>) -> Vec<Segment> {
    let mut out = Vec::new();
    for (i, group) in groups.into_iter().enumerate() {
        if i > 0 {
            out.push(Segment::plain(" "));
        }
        out.extend(group);
    }
    out
}

/// 几件事摆开要几格：各件的宽加上件与件之间的一个空。
fn spread(groups: &[Vec<Segment>]) -> usize {
    groups.iter().map(|group| width_of(group)).sum::<usize>() + groups.len().saturating_sub(1)
}
