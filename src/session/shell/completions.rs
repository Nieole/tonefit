//! **补全框**：输入行的候选多于一个时弹在屏底上方的那一格（`CONTEXT.md` 的《会话》：输入行；
//! 设计稿 `drawCompletions`）。
//!
//! 贴着屏的左下角、盖在卷列表上，**不把屏底撑高**；至多露 [`CANDIDATES_SHOWN`] 行，多了滚得动
//! （视口跟着轮到的那一条）。文件夹带 `/`、上文件夹那一色；压缩包旁边说一句（措辞与卷列表同一处）。

use ratatui::layout::Rect;

use super::super::look::{Kind, Look, Segment};
use super::super::state::Session;
use super::super::typing::CANDIDATES_SHOWN;
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas};

/// 框至多多宽（设计稿的 56）。
const WIDEST: u16 = 56;

/// 画补全框。输入行没开、或候选不多于一个，什么都不画。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session) {
    let Some(line) = &session.views.input else {
        return;
    };
    if line.candidates.is_empty() {
        return;
    }
    let total = line.candidates.len();
    let shown = total.min(CANDIDATES_SHOWN);
    let width = WIDEST.min(canvas.width().saturating_sub(2));
    let height = shown as u16 + 2;
    let area = Rect::new(1, canvas.height().saturating_sub(1 + height), width, height);
    let viewport = Viewport::with_margin(total, shown, line.at);
    let look = Look::kind(Kind::Focus);
    canvas.frame(
        area,
        &Border {
            thick: true,
            look,
            title: &[
                Segment::new("可选项", Look::PLAIN.bold()),
                Segment::faint(format!(" ⋅ {total} 项")),
            ],
            right: &[],
            bottom_left: &[],
            bottom_right: &[Segment::new(format!("{} of {total}", line.at + 1), look)],
        },
    );
    if let Some(bar) = viewport.scrollbar() {
        canvas.scrollbar(area, &bar);
    }
    let from = viewport.from();
    for (i, candidate) in line.candidates.iter().enumerate().skip(from).take(shown) {
        let current = i == line.at;
        let name = if candidate.directory {
            Look::kind(Kind::Directory)
        } else {
            Look::PLAIN
        };
        let segments = [
            Segment::new(if current { "❯ " } else { "  " }, look.bold()),
            Segment::new(candidate.shown(), if current { name.bold() } else { name }),
            Segment::new(
                candidate
                    .label()
                    .map_or_else(String::new, |label| format!("  {label}")),
                Look::FAINT.dim(),
            ),
        ];
        canvas.line(
            area.x + 2,
            area.y + 1 + (i - from) as u16,
            &segments,
            Some(width.saturating_sub(4)),
        );
    }
}
