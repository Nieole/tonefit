//! 屏上那一块：配置视图**顶上一条预设**（`CONTEXT.md` 的《配置视图》）。
//!
//! 当前套的是哪一份、与它差了哪几项、`p`／`c` 两个提示，右上角是**预设文件在哪**
//! （家目录缩写成 `~`）。跑着与等待确认时抬头上跟着说一句设置暂时锁定。
//!
//! **`p`／`c` 那两件问的是底下那一块**（[`Views::block`]）：打字时输入行盖在屏底上，
//! 这一条照旧写着它们——与卷列表上行内那两句顺口提的键同一条规矩。
//!
//! 预设栏（`p` 掀开、替换详情栏的那一栏）是另一块（[`super::picker`]）。**掀着的时候
//! 这一条照旧写着 `[p → 预设]`**：它说的是这个键把人带到哪一栏，而屏底那一行写的是
//! 此刻按下去做什么（`p → 返回`）——同一件事两句，各从按键表上取自己那一句。

use ratatui::layout::Rect;

use super::super::config;
use super::super::keymap::{self, Deed, Phase, Want};
use super::super::look::{Kind, Look, Segment, width_of};
use super::super::state::Session;
use super::super::tone::Tone;
use super::canvas::{Border, Canvas, hint};

/// 这一条占几行（一个框：上边框 · 一行字 · 下边框）。
pub(super) const ROWS: u16 = 3;

/// 正文那一行离右端那两件留几格（设计稿 `drawConfig` 的 `W - 6 - segW(hs)` 与 `W - 2 - …`）。
const ROOM_BEFORE_HINTS: u16 = 6;
const HINTS_FROM_RIGHT: u16 = 2;

/// 画顶上那一条。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, area: Rect) {
    let locked = session.settings_locked();
    let mut title = vec![Segment::new("预设", Look::PLAIN.bold())];
    if locked {
        title.push(Segment::new(
            " ⋅ 正在转换，设置暂时锁定",
            Look::tone(Tone::Caution),
        ));
    }
    let file = session
        .views
        .presets
        .as_ref()
        .map(|path| session.home_shown(path))
        .unwrap_or_default();
    let right = [Segment::new(file, Look::FAINT.dim())];
    canvas.frame(
        area,
        &Border {
            thick: false,
            look: Look::FAINT,
            title: &title,
            right: &right,
            bottom_left: &[],
            bottom_right: &[],
        },
    );
    let hints = mentioned(session, phase);
    let width = area.width;
    let room = width
        .saturating_sub(ROOM_BEFORE_HINTS)
        .saturating_sub(width_of(&hints) as u16);
    canvas.line(area.x + 2, area.y + 1, &applied(session), Some(room));
    let x = width.saturating_sub(HINTS_FROM_RIGHT + width_of(&hints) as u16);
    canvas.line(x, area.y + 1, &hints, None);
}

/// 正文那一行：当前套的是哪一份、与它差了哪几项。
fn applied(session: &Session) -> Vec<Segment> {
    let name = match &session.views.config.applied {
        Some(applied) => format!("「{}」", applied.name),
        None => "（未使用预设）".to_owned(),
    };
    let mut segments = vec![
        Segment::faint("当前使用 "),
        Segment::new(name, Look::kind(Kind::Done).bold()),
    ];
    let changed = config::changed(session);
    if changed.is_empty() {
        segments.push(Segment::faint("   与预设一致"));
        return segments;
    }
    let names: Vec<&str> = changed.iter().map(|field| field.label()).collect();
    segments.push(Segment::new(
        format!("   改动了 {} 项：", changed.len()),
        Look::tone(Tone::Caution),
    ));
    segments.push(Segment::plain(names.join("、")));
    segments
}

/// 右端那两件：`p` 预设、`c` 灰阶测试图。键与那一句都从按键表取。
fn mentioned(session: &Session, phase: Phase) -> Vec<Segment> {
    let block = session.views.block();
    let wants = [Want::saying(Deed::Presets, "预设"), Want::of(Deed::Chart)];
    let mut segments = Vec::new();
    for (i, said) in keymap::hints(phase, block, &wants).iter().enumerate() {
        if i > 0 {
            segments.push(Segment::plain("  "));
        }
        segments.extend(hint(&said.spelt(), said.what));
    }
    segments
}
