//! **总览**：任务视图最上面钉住的那一块（`CONTEXT.md` 的《会话》：总览）。
//!
//! 框的抬头答「此刻在做什么」，正文随阶段换。本票落地的是**还没开始**那一副（宽窄两档）：
//! 输出目录 · 勾了几条路径 · 型号 · 处理选项套的哪一份预设、改了几项；`t`／`x` 各做什么。
//! 清点中、跑起来之后与结束之后那几副随总览那一票接进 [`lines`]。

use ratatui::layout::Rect;

use super::super::keymap::{self, Deed, Phase, Want};
use super::super::look::{Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::canvas::{Border, Canvas, hint};
use super::topbar::model;
use super::yielding;

/// 画总览，从第 `y` 行起、占整宽；回它占了几行（正文加上下两条框线）。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, y: u16) -> u16 {
    let screen = Rect::new(0, 0, canvas.width(), canvas.height());
    let inner = screen.width.saturating_sub(4);
    let lines = lines(session, phase, yielding::compact(screen));
    let height = lines.len() as u16 + 2;
    canvas.frame(
        Rect::new(0, y, screen.width, height),
        &Border {
            thick: false,
            look: Look::FAINT,
            title: &title(phase),
            right: &[],
            bottom_left: &[],
            bottom_right: &[],
        },
    );
    for (i, line) in lines.iter().enumerate() {
        canvas.line(2, y + 1 + i as u16, line, Some(inner));
    }
    height
}

/// 框的抬头：还没开始时就这四个字。别的阶段随总览那一票接进来。
fn title(phase: Phase) -> Vec<Segment> {
    match phase {
        Phase::Fresh => vec![Segment::new("还没开始", Look::PLAIN.bold())],
        _ => Vec::new(),
    }
}

/// 正文那几行。`compact` 是不到 30 行高那一档。
fn lines(session: &Session, phase: Phase, compact: bool) -> Vec<Vec<Segment>> {
    match phase {
        Phase::Fresh => before_the_run(session, compact),
        _ => Vec::new(),
    }
}

/// 还没开始那两行。
fn before_the_run(session: &Session, compact: bool) -> Vec<Vec<Segment>> {
    let (on, off) = session.checked_paths();
    let changed = session.changed_from_preset();
    let applied = session
        .views
        .config
        .applied
        .as_ref()
        .map(|applied| &applied.name);
    // 起一趟那两个键与它们那一句都从按键表取（屏底摆的正是同一份）。
    let mentioned = |deed: Deed| {
        keymap::hints(Phase::Fresh, session.views.block(), &[Want::of(deed)])
            .first()
            .map(|said| hint(&said.spelt(), said.what))
            .unwrap_or_default()
    };
    let output = Segment::new(session.output_shown(), Look::PLAIN.bold());
    if compact {
        let preset = match applied {
            Some(name) => format!("预设「{name}」（改了 {changed} 项）"),
            None => "未使用预设".to_owned(),
        };
        let mut second = mentioned(Deed::Preview);
        second.push(Segment::plain(" "));
        second.extend(mentioned(Deed::Convert));
        second.push(Segment::faint(" ⋅ "));
        second.push(Segment::new(preset, Look::tone(Tone::Caution)));
        return vec![
            vec![
                Segment::faint("输出目录 "),
                output,
                Segment::faint(" ⋅ 路径 "),
                Segment::plain(format!("已勾选 {on} 个")),
                Segment::faint(format!(" ⋅ {}", model(session))),
            ],
            second,
        ];
    }
    let preset = match applied {
        Some(name) if changed > 0 => Segment::new(
            format!("预设「{name}」（改了 {changed} 项）"),
            Look::tone(Tone::Caution),
        ),
        Some(name) => Segment::faint(format!("预设「{name}」")),
        None => Segment::faint("未使用预设"),
    };
    let mut first = vec![
        Segment::faint("输出目录 "),
        output,
        Segment::faint("   路径 "),
        Segment::plain(format!("已勾选 {on} 个")),
    ];
    if off > 0 {
        first.push(Segment::faint(format!(" ⋅ {off} 个未勾选")));
    }
    first.extend([
        Segment::faint("   设备 "),
        Segment::plain(model(session)),
        Segment::faint("   处理选项 "),
        preset,
    ]);
    let mut second = mentioned(Deed::Preview);
    second.push(Segment::faint("  只分析，不写文件"));
    second.push(Segment::plain("     "));
    second.extend(mentioned(Deed::Convert));
    second.push(Segment::faint("  写到输出目录"));
    second.push(Segment::new(
        "     开始后会扫描每个路径里的卷",
        Look::tone(Tone::Muted),
    ));
    vec![first, second]
}
