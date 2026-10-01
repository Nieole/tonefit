//! 会话的**整屏画法**：任务 / 配置两个视图，逐格照设计稿（ADR 0019；spec《画法按新的块分模块》）。
//!
//! 验收是**设计快照**：每一景画在 `TestBackend` 上、逐格对设计稿导出的那一份（[`design`]）。
//!
//! # 屏上那几块各住在哪儿
//!
//! 一块一个模块；本模块只把屏切成几块、按视图挑画哪几块。
//!
//! | 屏上那一块 | 住在 |
//! |---|---|
//! | 往格子里写字、画框、画滚动条 | [`canvas`] |
//! | 摆不下时谁让位（最小尺寸、总览两行那一档、路径那一列、单栏） | [`yielding`] |
//! | 顶栏 | [`topbar`] |
//! | 总览 | [`overview`] |
//! | 确认条 | [`decision`] |
//! | 卷列表 | [`list`] |
//! | 每页结果 | [`pages`] |
//! | 顶上一条预设 | [`preset`] |
//! | 设置栏 | [`settings`] |
//! | 详情栏 | [`details`] |
//! | 预设栏 | [`picker`] |
//! | 覆盖层：整屏压暗、全部按键那一张、备注行上那张说明卡（内容出自 [`super::cover`]） | [`overlay`] |
//! | 屏底与输入行 | [`footer`] |
//! | 补全框 | [`completions`] |
//! | 窗口太小 | [`small`] |
//! | 转轮 · 横条 · 行首记号，与环节名、时长的写法 | [`marks`] |
//!
//! 颜色一处在 [`paint`]（本仓库唯一写得出颜色名的地方），键的写法一处在按键表（[`super::keymap`]）。
//! 设计快照的读法与逐格比对在 [`design`]（只在 `test` 里）。
//!
//! # 此刻
//!
//! 画一帧收一个「此刻」（`CONTEXT.md` 的《会话》：此刻）：屏底那句回话到没到点、
//! 连击键的待续记号还在不在，都读它；用例给定值。

mod canvas;
mod completions;
mod decision;
mod details;
/// `pub(super)`：屏底那一行摆到哪一格为止（[`footer::room`]）——灰阶测试图那句回话照它省略路径。
pub(super) mod footer;
mod list;
mod marks;
mod overlay;
mod overview;
mod pages;
/// `pub(super)`：颜色一处（本仓库唯一写得出颜色名的地方）；读回屏上一格的用例也拿它的答案比。
pub(super) mod paint;
mod picker;
mod preset;
mod settings;
mod small;
mod topbar;
mod yielding;

// 设计快照的读法与逐格比对。**敞开到会话这一层**：场景夹具（`super::scene`）要拿设计快照的
// 字网格核它自己那几处数（`session-redesign/05`），终端层那几串交互序列也对它比。
#[cfg(test)]
pub(super) mod design;

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::keymap::Phase;
use super::live::Live;
use super::state::Session;
use super::view::{Focus, Hit, View};
use canvas::Canvas;

/// 把一屏画出来，交出这一帧**点得中的区域**（spec《鼠标》：顶栏视图名、卷列表每一行、
/// 每页结果每一页、设置栏每一项、详情栏每一个值、预设栏每一行）。窗口太小那一屏一段都没有。
pub fn draw(frame: &mut Frame, session: &Session, live: Option<&Live>, now: Instant) -> Vec<Hit> {
    let screen = frame.area();
    let mut canvas = Canvas::new(frame.buffer_mut());
    let phase = Phase::of(session.stage(), live);
    if yielding::too_small(screen) {
        small::draw(&mut canvas, live, phase);
        return Vec::new();
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
    canvas.into_hits()
}

/// 任务视图：顶栏底下总览钉住，等待确认时接一条**确认条**，剩下的高度给卷列表**或者**
/// 每页结果，屏底一行。
///
/// **进了一卷就换掉整张卷列表**（`CONTEXT.md` 的《每页结果》：换掉卷列表、占整宽）：
/// 那一格里两块只画得出一块，判据是[进了哪一卷](super::view::TaskView::pages)那一格，
/// 与按键表查的那一块（[`super::view::TaskView::focus`]）读的是同一份。
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
    // **确认条钉在总览正下方**（`CONTEXT.md` 的《确认条》：不弹窗、不盖住卷列表）：
    // 它只在等待确认那一档在场，底下那一块跟着矮几行。
    if phase == Phase::Deciding {
        y += decision::draw(canvas, session, live, y);
    }
    let height = screen.height.saturating_sub(1 + y);
    let area = Rect::new(0, y, screen.width, height);
    if session.views.task.pages.is_some() {
        pages::draw(canvas, session, live, area);
        return;
    }
    list::draw(canvas, session, live, phase, now, area);
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

    use super::super::scene::Scene;
    use super::design::{self, Expected, assert_no_background, assert_same_cells};
    use super::draw;
    use super::paint::forcing;

    /// 在测试后端上画一屏，取回缓冲。
    fn painted(scene: &Scene, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        terminal
            .draw(|frame| {
                draw(frame, &scene.session, scene.live.as_ref(), scene.now());
            })
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
    /// 处理路径那几行带转轮（一行错开一格），卷列表抬头说马上显示卷列表；
    /// 输出目录那一行不摆 `i → 修改`——这一档 `i` 派不出去。
    #[test]
    fn the_survey_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("survey", 120, 36);
        assert_scene("survey", 80, 24);
    }

    /// **「转换中」120×36 与 80×24 逐格相等**（票面第一条）：总览给总进度、当前卷、
    /// 结论行与问题行，卷列表是那棵树。
    #[test]
    fn the_running_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("running", 120, 36);
        assert_scene("running", 80, 24);
    }

    /// **「正在摊开」120×36 与 80×24 逐格相等**（`design-parity/13`）：要摊开的卷（`.rar`）
    /// 走在摊开那一段时，总览的当前卷那一行、目录行与卷行的行尾都写「摊开」，词与横条上
    /// 摊开那一色；做完的那一卷与还没轮到的卷照旧。
    #[test]
    fn the_extracting_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("extracting", 120, 36);
        assert_scene("extracting", 80, 24);
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

    /// **「每页结果」120×36 与 80×24 逐格相等**（`session-redesign/11` 票面第一条）：
    /// 抬头是面包屑（任务 › 分区的路径 › 目录 › 卷）、右端写着此刻的列法，
    /// 头一行是这一卷的灰阶分布与需留意几页，第二行是各环节耗时，此后一页一行；
    /// 框底边左起是 `a` 那一件、右端说光标停在第几页。
    ///
    /// **80 列那一档缩放、画质分与尺寸三列都让掉**，只剩页面 · 灰阶 · 原因 · 提示
    /// ——砍列的次序在 `session::columns` 一处。
    #[test]
    fn the_pages_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("pages", 120, 36);
        assert_scene("pages", 80, 24);
    }

    /// **「按页跳过」120×36 与 80×24 逐格相等**（`design-parity/10`）：只点了一卷要摊开的 `.rar`，
    /// 上一趟转过、这一趟换了一话的扫描；进它的每页结果、列全部页——表里只有重做的那几页，
    /// 灰阶分布那一行在需留意几页之后说出**留下了几页**，第二行是**四个环节各花了多久**。
    #[test]
    fn the_retained_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("retained", 120, 36);
        assert_scene("retained", 80, 24);
    }

    /// **一页都不需留意那一句末尾那一件**（`design-parity/03`）：设计稿没有一屏钉着它
    /// （停车场 Q1253），这里从「按页跳过」那一景切回需留意的页——那一卷一页都没有——钉住那一截。
    /// 末尾那个键与框底边那一件同一处出处（按键表不问阶段与块的那一手）。
    ///
    /// **只钉「…的页 ⋅ a → 全部页」那一截**：那一句前半的措辞（「需留意」还是「需要留意」）
    /// 等 Q1253 拍板；框底边那一件也写着 `a → 全部页`，前面那几个字把两处分开。
    #[test]
    fn a_volume_with_no_notable_page_says_so_and_names_the_key_that_lists_every_page() {
        use super::super::state::Listing;

        let mut scene = Scene::named("retained");
        scene
            .session
            .views
            .task
            .pages
            .as_mut()
            .expect("这一景停在每页结果上")
            .listing = Listing::Notable;
        for (width, height) in [(120, 36), (80, 24)] {
            let screen = design::lines_of(&painted(&scene, width, height)).join("\n");
            assert!(
                screen.contains("的页 ⋅ a → 全部页"),
                "{width}×{height} 上没有那一句：\n{screen}"
            );
        }
    }

    /// 每页结果上开着的那一卷的报告。
    fn opened(scene: &Scene) -> &tonefit::VolumeReport {
        scene
            .session
            .pages_report(scene.live.as_ref())
            .unwrap_or_else(|| panic!("「{}」上开着一卷", scene.label))
    }

    /// **耗时那一行逐段就是报告里那一卷的卷级计时**（`design-parity/10`，收停车场 Q624）：
    /// 环节的词出自 `session::passes`，每一段的数是 `VolumeTiming` 那一格、与卷行耗时那一列同一种写法，
    /// 按走的次序；**没走的那一段不列**——不摊开的卷没有摊开那一截。宽窄两屏都摆得下整行。
    #[test]
    fn the_timing_line_on_the_pages_reads_the_volume_timing_segment_by_segment() {
        use super::super::passes::name;
        use super::marks::spell;
        use tonefit::Pass;

        let said = |pass: Pass, took| format!("{} {}", name(Some(pass)), spell(took));
        for (width, height) in [(120, 36), (80, 24)] {
            let scene = Scene::named("retained");
            let timing = opened(&scene).timing;
            let expected = format!(
                "耗时 {} ⋅ {} ⋅ {} ⋅ {}",
                said(Pass::Extraction, timing.extraction),
                said(Pass::Fingerprint, timing.fingerprint),
                said(Pass::First, timing.first_pass),
                said(Pass::Second, timing.second_pass),
            );
            let screen = design::lines_of(&painted(&scene, width, height));
            assert!(
                screen.iter().any(|line| line.contains(&expected)),
                "{width}×{height} 上没有「{expected}」：\n{}",
                screen.join("\n")
            );
        }
        // 目录卷不摊开：摊开那一段是零，耗时那一行从查重说起。
        let scene = Scene::named("pages");
        let timing = opened(&scene).timing;
        assert!(timing.extraction.is_zero(), "目录卷没有摊开");
        let expected = format!(
            "耗时 {} ⋅ {} ⋅ {}",
            said(Pass::Fingerprint, timing.fingerprint),
            said(Pass::First, timing.first_pass),
            said(Pass::Second, timing.second_pass),
        );
        let screen = design::lines_of(&painted(&scene, 120, 36));
        let line = screen
            .iter()
            .find(|line| line.contains("耗时 "))
            .unwrap_or_else(|| panic!("每页结果上没有耗时那一行：\n{}", screen.join("\n")));
        assert!(line.contains(&expected), "{line}");
        assert!(!line.contains(name(Some(Pass::Extraction))), "{line}");
    }

    /// **按页跳过的卷说出留下了几页，没有留下的页时那一句不在**（`design-parity/10`，收停车场 Q682）：
    /// 留下的页不进逐页结果，屏上只报个数——跟在「需留意 N/M 页」之后，M 是这一趟做了的页，
    /// 两个数加起来就是整本书。
    #[test]
    fn only_a_volume_skipped_by_page_says_how_many_pages_it_retained() {
        let scene = Scene::named("retained");
        let report = opened(&scene);
        assert!(report.retained_pages > 0, "这一景开着的是按页跳过的卷");
        for (width, height) in [(120, 36), (80, 24)] {
            let screen = design::lines_of(&painted(&scene, width, height)).join("\n");
            for said in [
                format!("/{} 页", report.pages.len()),
                format!("留下 {} 页没重做", report.retained_pages),
            ] {
                assert!(
                    screen.contains(&said),
                    "{width}×{height} 上没有「{said}」：\n{screen}"
                );
            }
            // 反着钉：分母不是整本书——留下的页一页都没问过需不需留意。
            let whole = format!("/{} 页", report.page_count());
            assert!(
                !screen.contains(&whole),
                "{width}×{height} 上需留意几页拿整本书作分母：\n{screen}"
            );
        }
        let scene = Scene::named("pages");
        assert_eq!(opened(&scene).retained_pages, 0, "这一卷整卷重做");
        let screen = design::lines_of(&painted(&scene, 120, 36)).join("\n");
        assert!(!screen.contains("留下"), "没有留下的页却说了：\n{screen}");
    }

    /// **「等待确认」120×36 与 80×24 逐格相等**（`session-redesign/12` 票面第一条）：
    /// 总览抬头是「预览 ⋅ 第 5/84 卷 ⋅ **等待确认**」、**不画预计**（那一刻横条一动不动），
    /// 底下钉着**确认条**——这一卷 · 灰阶分布 · 需留意几页一行，三种答法与 `v` 一行，
    /// 底边说目前还没有写入任何文件；卷列表照旧在屏上，那一卷标 `?`、自动滚动停在它那一行。
    ///
    /// **80 列那一档确认条收成短句**（`CONTEXT.md` 的《让位》：不到 110 列）：
    /// 卷名从中间省略、灰阶分布只报前两档、四句各收短一截。
    #[test]
    fn the_deciding_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("deciding", 120, 36);
        assert_scene("deciding", 80, 24);
    }

    /// **「整卷统一灰阶」120×36 与 80×24 逐格相等**（`session-redesign/10` 票面第一条）：
    /// 卷行在灰阶分布之后多一列**代表页**，砍列时排在耗时之后（80 列那一档耗时与代表页都让掉了）。
    ///
    /// 当前卷走到半页上（设计稿的模拟走连续时间），环节横条与它旁边那个数一样只数走完的整页。
    #[test]
    fn the_envelope_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("envelope", 120, 36);
        assert_scene("envelope", 80, 24);
    }

    /// **「整卷统一灰阶 + 等待确认」120×36 与 80×24 逐格相等**（`design-parity/08`，停车场 Q900、Q902）：
    /// 差异大的页与代表页只有整卷统一灰阶那一趟才有，而它同样停得到确认点上——
    /// 确认条头一行在需留意几页之后另报「差异大的页 N」，**需留意几页把代表页数进去**
    /// （与每页结果抬头同一个数，`CONTEXT.md` 的《需留意的页》）；那一卷所在的目录展开着，
    /// 卷行带着代表页那一列、行尾说等待确认。
    ///
    /// **80 列那一档确认条收成短句**，「差异大的页」那一截随长句一起让掉，需留意几页照旧是那个数。
    #[test]
    fn the_envelope_deciding_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("envelope-deciding", 120, 36);
        assert_scene("envelope-deciding", 80, 24);
    }

    /// **差异大的页屏上只有一个叫法**（`design-parity/09`，收停车场 Q731）：总览的问题行
    /// （宽窄两副）、确认条、卷行行尾说的都是词汇表那个名字「差异大的页」。
    ///
    /// 屏上改掉的那两种叫法反着钉（`docs/agents/testing.md`）：「与其他页差异大 N 页」与窄屏那一副的
    /// 「差异大 N」——这几屏上每一处「差异大」后面都得跟着「的页」。每页结果原因那一列的
    /// 「差异大，单独判断」是判定的理由（库的那一句，命令行报告也印它），不在这几屏上。
    #[test]
    fn an_outlier_goes_by_one_name_on_the_overview_the_decision_bar_and_the_volume_row() {
        for (name, width, height) in [
            ("envelope", 120, 36),
            ("envelope", 80, 24),
            ("envelope-deciding", 120, 36),
            ("envelope-deciding", 80, 24),
        ] {
            let scene = Scene::named(name);
            let screen = design::lines_of(&painted(&scene, width, height)).join("\n");
            assert!(
                !screen.contains("与其他页差异大"),
                "{name} {width}×{height} 上又出现了第二种叫法：\n{screen}"
            );
            for (at, _) in screen.match_indices("差异大") {
                assert!(
                    screen[at + "差异大".len()..].starts_with("的页"),
                    "{name} {width}×{height} 上有一处「差异大」不是「差异大的页」：\n{screen}"
                );
            }
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
    #[test]
    fn the_search_scene_matches_its_design_snapshot_wide_and_narrow() {
        assert_scene("search", 120, 36);
        assert_scene("search", 80, 24);
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
