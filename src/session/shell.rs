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
//! | 总览 | [`overview`] | 06：还没开始那一副宽窄两档；08：清点中与跑起来；10：结束之后 |
//! | 确认条 | — | 等待确认那一票 |
//! | 卷列表 | [`list`] | 06：开跑之前那一副；08：清点中那一副与清点之后那棵树 |
//! | 每页结果 | — | 每页结果那一票 |
//! | 顶上一条预设 | [`preset`] | 13 |
//! | 设置栏 | [`settings`] | 13 |
//! | 详情栏 | [`details`] | 13 |
//! | 预设栏 | [`picker`] | 14 |
//! | 覆盖层 | [`overlay`] | 07：整屏压暗、全部按键那一张；10：备注行上那张**说明卡**（内容出自 [`super::cover`]） |
//! | 屏底与输入行 | [`footer`] | 06：屏底；07：输入行 |
//! | 补全框 | [`completions`] | 07 |
//! | 窗口太小 | [`small`] | 06；08 补上跑着那一行总进度 |
//! | 转轮 · 横条 · 行首记号 | [`marks`] | 08：总览、树、窗口太小三处共用 |
//!
//! 颜色一处在 [`super::draw::paint`]（`look`），键的写法一处在按键表（[`super::keymap`]）。
//!
//! # 此刻
//!
//! 画一帧收一个「此刻」（`CONTEXT.md` 的《会话》：此刻）：屏底那句回话到没到点、
//! 连击键的待续记号还在不在，都读它；用例给定值。

mod canvas;
mod completions;
mod details;
mod footer;
mod list;
mod marks;
mod overlay;
mod overview;
mod picker;
mod preset;
mod settings;
mod small;
mod topbar;
mod yielding;

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::keymap::Phase;
use super::live::Live;
use super::state::Session;
use super::view::{Focus, View};
use canvas::Canvas;

/// 把一屏画出来。
pub fn draw(frame: &mut Frame, session: &Session, live: Option<&Live>, now: Instant) {
    let screen = frame.area();
    let mut canvas = Canvas::new(frame.buffer_mut());
    let phase = Phase::of(session.stage(), live);
    if yielding::too_small(screen) {
        small::draw(&mut canvas, live, phase);
        return;
    }
    topbar::draw(&mut canvas, session, live, phase, now);
    match session.views.view {
        View::Task => task(&mut canvas, session, live, phase, now, screen),
        View::Config => config(&mut canvas, session, phase, screen),
    }
    // 补全框盖在卷列表上；覆盖层掀着时连它一起压暗（设计稿 `drawAll` 的次序）；屏底最后画，
    // 掀着覆盖层时它摆的是覆盖层自己的那两件。
    completions::draw(&mut canvas, session);
    overlay::draw(&mut canvas, session, phase);
    footer::draw(&mut canvas, session, live, phase, now);
}

/// 任务视图：顶栏底下总览钉住，剩下的高度给卷列表（每页结果与确认条随各票接进来），屏底一行。
fn task(
    canvas: &mut Canvas<'_>,
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
    screen: Rect,
) {
    let mut y = 1;
    y += overview::draw(canvas, session, live, phase, now, y);
    let height = screen.height.saturating_sub(1 + y);
    list::draw(
        canvas,
        session,
        live,
        phase,
        now,
        Rect::new(0, y, screen.width, height),
    );
}

/// 配置视图：顶栏底下一条预设钉住，剩下的高度给设置栏与右边那一栏，屏底一行。
/// **不到 90 列退成单栏**——那一刻屏上只画此刻聚焦的那一栏（`CONTEXT.md` 的《让位》）。
///
/// **右边那一栏是详情栏还是预设栏**由「掀着预设栏没有」说（`CONTEXT.md` 的《预设栏》：
/// 它替换详情栏，设置栏仍在屏上）——两者画的是同一块地方，只有一个在场。
fn config(canvas: &mut Canvas<'_>, session: &Session, phase: Phase, screen: Rect) {
    let strip = Rect::new(0, 1, screen.width, preset::ROWS);
    preset::draw(canvas, session, phase, strip);
    let y = 1 + preset::ROWS;
    let height = screen.height.saturating_sub(1 + y);
    let narrow = yielding::single_column(screen);
    let left = yielding::settings_width(screen);
    let on_details = session.views.block() != Focus::Settings;
    if !narrow || !on_details {
        settings::draw(canvas, session, Rect::new(0, y, left, height));
    }
    let right = if narrow {
        on_details.then(|| Rect::new(0, y, screen.width, height))
    } else {
        Some(Rect::new(left, y, screen.width - left, height))
    };
    if let Some(area) = right {
        if session.views.config.picker {
            picker::draw(canvas, session, area);
        } else {
            details::draw(canvas, session, area, narrow);
        }
    }
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

    /// **「清点中」120×36 与 80×24 逐格相等**（票面第一条）：总览只说正在清点、不报卷数，
    /// 处理路径那几行带转轮（一行错开一格），卷列表抬头说马上显示卷列表。
    ///
    /// **输出目录那一行行尾抹掉十格**：设计稿在那儿仍写着 `[i → 修改]`，而这一档 `i`
    /// 派不出去（它自己的 `taskKey` 在跑着时就返回 false），实现照「屏上不摆按不动的键」
    /// 不写它——停车场 **Q807**。抹掉的那十格仍要求是空白。
    #[test]
    fn the_survey_scene_matches_its_design_snapshot_wide_and_narrow() {
        for (width, height) in [(120, 36), (80, 24)] {
            let scene = Scene::named("survey");
            let buffer = painted(&scene, width, height);
            assert_no_background(&buffer);
            assert_same_cells(
                &buffer,
                &design::snapshot("survey", width, height).blanked(6, 26, 10),
            );
        }
    }

    /// **「转换中」120×36 与 80×24 逐格相等**（票面第一条）：总览给总进度、当前卷、
    /// 结论行与问题行，卷列表是那棵树。
    #[test]
    fn the_running_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("running", 120, 36);
        assert_scene("running", 80, 24);
    }

    /// **「已结束」120×36 与 80×24 逐格相等**（`session-redesign/10` 票面第一条）：
    /// 总览抬头换成结束那句话加用时、右端写输出目录，结论行是转换那一副；转换失败的卷是
    /// 它目录里的 `✗` 卷行、行尾是那句原因；备注行挂在分区末尾。**代表页那一列整个不在场**
    /// ——这一趟走的是默认逐页判断（停车场 Q712）。
    #[test]
    fn the_ended_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("ended", 120, 36);
        assert_scene("ended", 80, 24);
    }

    /// **「整卷统一灰阶」120×36 与 80×24 逐格相等**（`session-redesign/10` 票面第一条）：
    /// 卷行在灰阶分布之后多一列**代表页**，砍列时排在耗时之后（80 列那一档耗时与代表页都让掉了）。
    ///
    /// **那条环节横条换掉三格**（各换成它右边那一格）：设计稿那一头的模拟走的是**连续时间**
    /// ——`灰原哀/第05卷` 的 `done` 是 114.554，半页也占一格；而这一趟**一页一步**，
    /// 走到的是 114/166，同一条横条因此少满一格（24 格的满 16 不满 17，8 格的满 5 不满 6）。
    /// 换掉之后仍是一条断言，停车场 **Q844**。
    #[test]
    fn the_envelope_scene_matches_its_design_snapshot_wide_and_narrow() {
        let scene = Scene::named("envelope");
        // 换掉的那几格：120×36 上总览当前卷那一行（第 46 格）与树上 `灰原哀` 那一行
        // 行尾（第 111 格）各一格，80×24 上那一行（第 57 格）一格——三处都是同一条横条。
        for (width, height, cells) in [
            (120u16, 36u16, &[(3usize, 46u16), (28, 111)][..]),
            (80, 24, &[(20, 57)][..]),
        ] {
            let buffer = painted(&scene, width, height);
            assert_no_background(&buffer);
            let mut expected = design::snapshot("envelope", width, height);
            for (row, at) in cells {
                expected = expected.cell_like(*row, *at, at + 1);
            }
            assert_same_cells(&buffer, &expected);
        }
    }

    /// **「窗口太小」转换中那一份逐格相等**（票面第一条）：中间那一行是总进度。
    #[test]
    fn the_too_small_screen_while_running_matches_its_design_snapshot() {
        assert_scene("running", 56, 14);
    }

    /// **「添加路径」120×36 与 80×24 逐格相等**（`session-redesign/07` 票面第一条）：输入行占着屏底、
    /// 补全框弹在它上方盖住卷列表、卷列表的框细了、光标行首是暗的 `›`。
    #[test]
    fn the_add_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("add", 120, 36);
        assert_scene("add", 80, 24);
    }

    /// **「配置」120×36 与 80×24 逐格相等**（`session-redesign/13` 票面第一条）：
    /// 顶上一条预设、设置栏三组、详情栏的取值环连同说明；窄那一屏退成单栏，只剩详情栏。
    #[test]
    fn the_config_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("config", 120, 36);
        assert_scene("config", 80, 24);
    }

    /// **「搜索」120×36 与 80×24 逐格相等**（`session-redesign/09` 票面第一条）：
    /// 输入行占着屏底、提示词是 `/`、右端只有 `⏎ → 跳到结果` 与 `Esc → 取消`；
    /// 卷列表的框细了、光标行首是暗的 `›`；**匹配上的那几行名字那一列加下划线**
    /// （目录名装着这一句时它底下那几卷一起加）；框底边左起写着这一句与 `n`／`N`。
    ///
    /// **那条环节横条换掉两格**（各换成它右边那一格）：这一景与「整卷统一灰阶」同在
    /// 62% 上、当前卷同是 `灰原哀/第05卷`，踩的是同一条**已知的一格差**——设计稿那一头
    /// 的模拟走的是**连续时间**（`done` 是 114.554，半页也占一格），而这一趟**一页一步**、
    /// 走到的是 114/166。24 格那一条满 16 不满 17，8 格那一条满 5 不满 6。
    /// 换掉之后仍是一条断言（实现在那一格上写别的照样红），停车场 **Q844**。
    #[test]
    fn the_search_scene_matches_its_design_snapshot_wide_and_narrow() {
        let scene = Scene::named("search");
        // 换掉的那几格：120×36 上总览当前卷那一行（第 46 格）与树上 `灰原哀` 那一行
        // 行尾（第 103 格）各一格，80×24 上那一行（第 57 格）一格——三处都是同一条横条。
        for (width, height, cells) in [
            (120u16, 36u16, &[(3usize, 46u16), (16, 103)][..]),
            (80, 24, &[(14, 57)][..]),
        ] {
            let buffer = painted(&scene, width, height);
            assert_no_background(&buffer);
            let mut expected = design::snapshot("search", width, height);
            for (row, at) in cells {
                expected = expected.cell_like(*row, *at, at + 1);
            }
            assert_same_cells(&buffer, &expected);
        }
    }

    /// **「全部按键」120×36 与 80×24 逐格相等**（`session-redesign/09` 票面第一条）：
    /// 覆盖层掀在**转换中**那一副上——底下整屏压暗，那一张只列此刻这一档派得出的键，
    /// 宽那一屏两栏、窄那一屏一栏，底边说看到第几行。
    ///
    /// 这一景 07 摆不出来（底下那棵树归 08）：那一票因此把它连同它那六串留给了本票。
    #[test]
    fn the_help_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("help", 120, 36);
        assert_scene("help", 80, 24);
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
