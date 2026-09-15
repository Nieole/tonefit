//! **卷列表**：任务视图的主体，一张整宽的列表，一个框（`CONTEXT.md` 的《会话》：卷列表、
//! 停得住 / 展得开、焦点——聚焦框）。
//!
//! **形状随阶段换。** 本票落地的是**开跑之前**那一副：输出目录那一行 · 「处理路径 (N)」·
//! 一条条处理路径（勾选框 · 路径 · 文件夹还是压缩包；被另一条包含着的标一句，没勾的压暗）·
//! 末行「＋ 添加路径」。**这一副一次都不碰盘**：文件夹还是压缩包按扩展名认
//! （[`NamedPath::is_archive`]），被包含按路径前缀认（[`Session::nested_in`]）。
//! 清点之后那棵树随树那一票接进 [`draw`]。
//!
//! 焦点在这一块上时框换成粗线、上聚焦色，光标那一行行首是 `❯`；别处聚焦时细线、光标行首是暗的 `›`。
//! 框底边右端说光标停在第几条、共几条。

use ratatui::layout::Rect;

use super::super::keymap::{self, Deed, Phase, Want};
use super::super::look::{Kind, Look, Segment};
use super::super::state::{NamedPath, Session};
use super::super::tone::Tone;
use super::super::view::{Focus, Line};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, hint, padded};
use super::yielding;

/// 画卷列表，占 `area`。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, area: Rect) {
    let focused = session.views.focus() == Focus::VolumeList;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    let (position, stops) = session.cursor_position();
    let lines = session.lines();
    let cursor = session.cursor_line();
    let shown = area.height.saturating_sub(2);
    let inner = area.width.saturating_sub(4);
    let viewport = Viewport::new(lines.len(), usize::from(shown), cursor);
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &[
                Segment::new("任务", Look::PLAIN.bold()),
                Segment::faint(" ⋅ 路径"),
            ],
            right: &[],
            bottom_left: &[],
            bottom_right: &[Segment::new(format!("{position} of {stops}"), look)],
        },
    );
    // 路径那一列有多宽，整张表一个数：最长那条路径说了算。
    let longest = session
        .scope
        .paths
        .iter()
        .map(|named| crate::wrap::width(&session.home_shown(&named.path)))
        .max()
        .unwrap_or(0);
    let column = yielding::path_column(inner, longest);
    let from = usize::from(viewport.from());
    for (i, line) in lines.iter().enumerate().skip(from).take(usize::from(shown)) {
        let segments = row(session, phase, line, i == cursor, focused, column);
        canvas.line(
            area.x + 2,
            area.y + 1 + (i - from) as u16,
            &segments,
            Some(inner),
        );
    }
}

/// 一行的字：光标那一格、勾选框、名字、种类、行尾那一句。`column` 是路径那一列的宽度。
fn row(
    session: &Session,
    phase: Phase,
    line: &Line,
    at_cursor: bool,
    focused: bool,
    column: u16,
) -> Vec<Segment> {
    // 行上顺口提的那两个键（`[i → 修改]`、`[o → 添加]`）连同那一句都从按键表取；派不出就不提。
    let mentioned = |want: Want| {
        keymap::hints(phase, session.views.focus(), &[want])
            .first()
            .map(|said| hint(&said.spelt(), said.what))
            .unwrap_or_default()
    };
    let cursor = match (at_cursor, focused) {
        (true, true) => Segment::new("❯ ", Look::kind(Kind::Focus).bold()),
        (true, false) => Segment::faint("› "),
        (false, _) => Segment::plain("  "),
    };
    // 光标那一行的名字加粗。
    let named = |look: Look| if at_cursor { look.bold() } else { look };
    match line {
        Line::Output => {
            let mut segments = vec![
                cursor,
                Segment::faint("输出目录   "),
                Segment::new(session.output_shown(), named(Look::PLAIN)),
                Segment::plain("   "),
            ];
            segments.extend(mentioned(Want::of(Deed::EditPath)));
            segments
        }
        Line::Heading => vec![
            Segment::plain("  "),
            Segment::new("处理路径", Look::PLAIN.bold()),
            Segment::faint(format!(" ({})", session.scope.paths.len())),
            Segment::new("   ⋅ 开始前不会读取里面的内容", Look::tone(Tone::Muted)),
        ],
        Line::Add => {
            let mut segments = vec![
                cursor,
                Segment::new("＋ 添加路径", named(Look::kind(Kind::Done))),
                Segment::plain("   "),
            ];
            segments.extend(mentioned(Want::saying(Deed::AddPath, "添加")));
            segments
        }
        Line::Path(at) => {
            let named_path: &NamedPath = &session.scope.paths[*at];
            let checkbox = if named_path.on {
                Segment::new("[x] ", Look::kind(Kind::Done))
            } else {
                Segment::faint("[ ] ")
            };
            let name_look = if named_path.on {
                Look::PLAIN
            } else {
                Look::tone(Tone::Muted)
            };
            let shown = session.home_shown(&named_path.path);
            let mut segments = vec![
                cursor,
                checkbox,
                Segment::new(
                    padded(
                        &crate::session::columns::elide(&shown, usize::from(column)),
                        column,
                    ),
                    named(name_look),
                ),
                Segment::faint(format!(" {}", named_path.kind())),
            ];
            if named_path.on
                && let Some(outer) = session.nested_in(*at)
            {
                segments.push(Segment::new(
                    format!(" ⋅ 已包含在 {} 中，不会重复处理", session.home_shown(outer)),
                    Look::tone(Tone::Caution),
                ));
            }
            if !named_path.on {
                segments.push(Segment::new(
                    " ⋅ 未勾选，本次不处理",
                    Look::tone(Tone::Muted),
                ));
            }
            segments
        }
    }
}
