//! **窗口太小**：不到 60×16 时整屏只剩一句「窗口太小」、当前尺寸、总进度百分比与怎么退出，
//! 放大之后原样回来（`CONTEXT.md` 的《会话》：让位）。

use super::super::keymap::{self, Deed, Phase};
use super::super::live::Live;
use super::super::look::{Look, Segment, width_of};
use super::super::tone::Tone;
use super::super::view::Focus;
use super::canvas::{Canvas, hint};
use super::yielding::LEAST;

/// 画窗口太小那几行，整屏居中。
pub(super) fn draw(canvas: &mut Canvas<'_>, live: Option<&Live>, phase: Phase) {
    let (width, height) = (canvas.width(), canvas.height());
    // 退出那一件按阶段从表上取：还没开始与结束了是 `q`，跑着与等待确认时 `q` 不退、摆 `C-c`
    // （屏上不摆按不动的键，停车场 Q777）。
    let quit = [Deed::Quit, Deed::Interrupt].into_iter().find_map(|deed| {
        keymap::hints(phase, Focus::VolumeList, &[keymap::Want::of(deed)])
            .into_iter()
            .next()
    });
    let mut last = quit
        .map(|said| hint(&said.spelt(), said.what))
        .unwrap_or_default();
    last.push(Segment::faint("  放大窗口后自动恢复"));
    let lines: Vec<Vec<Segment>> = vec![
        vec![Segment::new("窗口太小", Look::tone(Tone::Caution).bold())],
        vec![Segment::plain(format!(
            "{width}x{height}，至少需要 {}x{}",
            LEAST.0, LEAST.1
        ))],
        Vec::new(),
        progress(live),
        Vec::new(),
        last,
    ];
    let top = height.saturating_sub(lines.len() as u16) / 2;
    for (i, line) in lines.iter().enumerate() {
        let x = width.saturating_sub(width_of(line) as u16) / 2;
        canvas.line(x, top + i as u16, line, Some(width));
    }
}

/// 中间那一行：还没开始，或者总进度（随总览那一票接上跑着的那几副）。
fn progress(live: Option<&Live>) -> Vec<Segment> {
    match live {
        None => vec![Segment::faint("还没开始")],
        Some(_) => Vec::new(),
    }
}
