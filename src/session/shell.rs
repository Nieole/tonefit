//! 新会话的**整屏画法**：任务 / 配置两个视图，逐格照设计稿（ADR 0019；spec《画法按新的块分模块》）。
//!
//! **在测试里长出来**：真会话仍进旧界面（[`super::draw::shell`]），切换在 `session-redesign/15`。
//! 这一副画在 `TestBackend` 上、逐格对设计快照（`super::draw::design`），一块一块画绿。
//!
//! # 屏上那几块各住在哪儿
//!
//! 一块一个模块；本模块只把屏切成几块、按视图挑画哪几块。
//!
//! | 屏上那一块 | 住在 | 落地 |
//! |---|---|---|
//! | 往格子里写字、画框 | [`canvas`] | 本票 |
//! | 摆不下时谁让位 | [`yielding`] | 本票（最小尺寸、总览两行那一档、路径那一列） |
//! | 顶栏 | [`topbar`] | 本票 |
//! | 总览 | [`overview`] | 本票：还没开始那一副宽窄两档 |
//! | 确认条 | — | 等待确认那一票 |
//! | 卷列表 | [`list`] | 本票：开跑之前那一副；清点之后那棵树随树那一票 |
//! | 每页结果 | — | 每页结果那一票 |
//! | 设置栏 · 详情栏 · 预设栏 | — | 配置视图那几票 |
//! | 覆盖层 | — | 全部按键那一票 |
//! | 屏底与输入行 | [`footer`] | 本票：屏底；输入行随添加路径那一票 |
//! | 补全框 | — | 添加路径那一票 |
//! | 窗口太小 | [`small`] | 本票 |
//!
//! 颜色一处在 [`super::draw::paint`]（`look`），键的写法一处在按键表（[`super::keymap`]）。
//!
//! # 此刻
//!
//! 画一帧收一个「此刻」（`CONTEXT.md` 的《会话》：此刻）：屏底那句回话到没到点、
//! 连击键的待续记号还在不在，都读它；用例给定值。

mod canvas;
mod footer;
mod list;
mod overview;
mod small;
mod topbar;
mod yielding;

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::keymap::Phase;
use super::live::Live;
use super::state::Session;
use super::view::View;
use canvas::Canvas;

/// 把一屏画出来。
pub fn draw(frame: &mut Frame, session: &Session, live: Option<&Live>, now: Instant) {
    let screen = frame.area();
    let mut canvas = Canvas::new(frame.buffer_mut());
    if yielding::too_small(screen) {
        small::draw(&mut canvas, live);
        return;
    }
    let phase = Phase::of(session.stage(), live);
    topbar::draw(&mut canvas, session);
    match session.views.view {
        View::Task => task(&mut canvas, session, phase, screen),
        // 配置视图随它那几票接进来。
        View::Config => {}
    }
    footer::draw(&mut canvas, session, phase, now);
}

/// 任务视图：顶栏底下总览钉住，剩下的高度给卷列表（每页结果与确认条随各票接进来），屏底一行。
fn task(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, screen: Rect) {
    let mut y = 1;
    y += overview::draw(canvas, session, phase, y);
    let height = screen.height.saturating_sub(1 + y);
    list::draw(
        canvas,
        session,
        phase,
        Rect::new(0, y, screen.width, height),
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::super::draw::design::{self, Expected, assert_no_background, assert_same_cells};
    use super::super::draw::paint::forcing;
    use super::super::scene::Scene;
    use super::draw;

    /// 在测试后端上画一屏，取回缓冲。
    fn painted(scene: &Scene, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        terminal
            .draw(|frame| draw(frame, &scene.session, scene.live.as_ref(), scene.now()))
            .expect("画得出来");
        terminal.backend().buffer().clone()
    }

    /// 画一个场景、逐格对它的设计快照，顺带核一个背景色都没设（停车场 Q737）。
    fn assert_scene(scene: &str, width: u16, height: u16) {
        let scene = Scene::named(scene);
        let buffer = painted(&scene, width, height);
        assert_no_background(&buffer);
        assert_same_cells(&buffer, &design::snapshot(&scene.label, width, height));
    }

    /// **「还没开始」120×36 与 80×24 逐格相等**（票面第一条）：字、前景色、修饰。
    #[test]
    fn the_fresh_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("fresh", 120, 36);
        assert_scene("fresh", 80, 24);
    }

    /// **「窗口太小」还没开始那一份逐格相等**（票面第一条）。
    #[test]
    fn the_too_small_screen_matches_its_design_snapshot() {
        assert_scene("fresh", 56, 14);
    }

    /// **`NO_COLOR` 在场时颜色退回默认，加粗、下划线、粗框、压暗照旧**（票面第四条）：
    /// 不上色那一屏与设计快照逐格比，只差每一格的前景色都是终端默认色。
    #[test]
    fn without_colour_only_the_hues_fall_back_and_every_modifier_stays() {
        let scene = Scene::named("fresh");
        let buffer = forcing(false, || painted(&scene, 120, 36));
        let expected: Expected = design::snapshot("fresh", 120, 36).without_colour();
        assert_same_cells(&buffer, &expected);
        // 对照：上色那一屏与快照本来就相等，说明差的只有颜色。
        let coloured = forcing(true, || painted(&scene, 120, 36));
        assert_same_cells(&coloured, &design::snapshot("fresh", 120, 36));
    }
}
