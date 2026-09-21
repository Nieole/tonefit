//! 屏上那一块：**设置栏**——配置视图左边那一栏（`CONTEXT.md` 的《设置栏》）。
//!
//! 三组，一项一行，项名与当前值列对齐：**设备设置** · **处理选项** · **画质判定参数**（只读）。
//! 与套着的预设不同的那一项行尾带 `*`；跑着与等待确认时抬头右端写 `[已锁定]`、各行的取值压暗。
//!
//! 哪几行、各印什么在 [`config`]（特性外面那一侧），本模块只把它们摆进格子里。
//! 装不下时视口跟着光标走、右框线上画滚动条（`CONTEXT.md` 的《视口》）。

use ratatui::layout::Rect;

use super::super::config::{self, Band, Item};
use super::super::look::{Hue, Kind, Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::view::{Focus, Target};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, padded};

/// 项名那一列至多、至少多宽，以及取值那一列留多少（设计稿 `drawCfgLeft` 的 `labelW`）。
const LABEL_WIDEST: u16 = 18;
const LABEL_NARROWEST: u16 = 10;
const VALUE_KEEPS_CLEAR: u16 = 24;

/// 画设置栏，占 `area`。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, area: Rect) {
    let focused = session.views.focus() == Focus::Settings;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    let locked = session.settings_locked();
    let lines = config::lines();
    let cursor = session.config_line();
    let (position, stops) = session.config_position();
    let shown = area.height.saturating_sub(2);
    let inner = area.width.saturating_sub(4);
    let viewport = Viewport::with_margin(lines.len(), usize::from(shown), cursor);
    let locked_mark = [Segment::new("[已锁定]", Look::tone(Tone::Caution).bold())];
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &[
                Segment::new("设置", Look::PLAIN.bold()),
                Segment::faint(" ⋅ 预设会保存这两组"),
            ],
            right: if locked { &locked_mark } else { &[] },
            bottom_left: &[],
            bottom_right: &[Segment::new(format!("{position} of {stops}"), look)],
        },
    );
    if let Some(bar) = viewport.scrollbar() {
        canvas.scrollbar(area, &bar);
    }
    let column = label_column(inner);
    let from = viewport.from();
    for (at, line) in lines.iter().enumerate().skip(from).take(usize::from(shown)) {
        let y = area.y + 1 + (at - from) as u16;
        match line {
            // 组抬头顶格写，停不住，行首没有光标那两格。
            config::Line::Band(band) => {
                canvas.line(area.x + 2, y, &band_row(*band), Some(inner));
            }
            config::Line::Item(item) => {
                let segments = item_row(session, *item, at == cursor, focused, locked, column);
                canvas.line(area.x + 1, y, &segments, Some(inner + 1));
                canvas.hit_row(area, y, Target::Item(*item));
            }
        }
    }
}

/// 项名那一列有多宽：至多 18 格、至少 10 格，还要给取值那一列留下 24 格。
fn label_column(inner: u16) -> u16 {
    inner
        .saturating_sub(VALUE_KEEPS_CLEAR)
        .clamp(LABEL_NARROWEST, LABEL_WIDEST)
}

/// 一组的抬头：组名与那半句注。**画质判定参数那一组的组名不上种类色**——
/// 它不是一组改得动的设置，屏上因此与另两组分得开（设计稿 `drawCfgLeft`）。
fn band_row(band: Band) -> Vec<Segment> {
    let name = match band {
        Band::Judging => Look::FAINT.bold(),
        _ => Look::kind(Kind::Band).bold(),
    };
    vec![
        Segment::new(band.name(), name),
        Segment::new(format!(" ⋅ {}", band.note()), Look::FAINT.dim()),
    ]
}

/// 一项的一行：光标那两格、项名、当前值，行尾与预设不同时那个 `*`。
fn item_row(
    session: &Session,
    item: Item,
    at_cursor: bool,
    focused: bool,
    locked: bool,
    column: u16,
) -> Vec<Segment> {
    let mark = match (at_cursor, focused) {
        (true, true) => "❯ ",
        (true, false) => "› ",
        (false, _) => "  ",
    };
    let mark_look = if focused {
        Look::kind(Kind::Focus).bold()
    } else {
        Look::FAINT
    };
    let label = if at_cursor {
        Look::PLAIN.bold()
    } else {
        Look::of(Hue::Prose)
    };
    // 说了的一档亮着，跟着默认值走的那一档次要那一灰；只读时整行压暗，光标那一行聚焦时加粗。
    let mut value = if item.said(session) {
        Look::PLAIN
    } else {
        Look::FAINT
    };
    if locked {
        value = value.dim();
    }
    if at_cursor && focused {
        value = value.bold();
    }
    let mut segments = vec![
        Segment::new(mark, mark_look),
        Segment::new(padded(item.label(), column), label),
        Segment::new(item.shown(session), value),
    ];
    if let Item::Setting(field) = item
        && config::starred(session, field)
    {
        segments.push(Segment::new(" *", Look::tone(Tone::Caution).bold()));
    }
    segments
}
