//! 屏上那一块：**覆盖层**——一个键掀开、盖住屏上那几块的那一张
//! （`CONTEXT.md` 的《会话》：覆盖层）。
//!
//! # 一张画法，两份内容
//!
//! **两张覆盖层是同一副形状，不是两套画法**（`p3-session-legibility/12` 票面第四条）：
//! 这一格的抬头、边框、折行、视口与那条滚动条只有 [`overlay`] 一处，
//! 两张差的只有[正文那几行是谁出的](Overlay)——
//!
//! | 掀开的是 | 正文出自 | 它答的是 |
//! |---|---|---|
//! | [全部键](Overlay::Keys) | [`keys`]（问 [`Session::key_table`]） | 此刻按得动哪些键 |
//! | [这一趟的前提](Overlay::Premises) | [`premises`]（[`crate::render::header`]） | 这份报告是照哪几条算出来的 |
//!
//! # 键位表从按键表取，不另抄一份
//!
//! [`Session::key_table`] 把每一个键交给那一块的按键表问一遍，问出来什么就列什么
//! ——本模块**一个键都不列**，只把问出来的动作翻成屏上那句话（[`says`]）。
//! 改一处按键表，这一张当场跟着变。
//!
//! **分组就是按键表自己的头一层分岔**（[`KeyGroup`]，ADR 0017 决定第 2 条）：
//! 先按焦点分，落到哪一块再按阶段分——票面说的「按焦点分组」正是它。
//! 阶段那一维那几个键只在「任何时候」那一组里列一遍。
//!
//! # 装不下时滚得动
//!
//! 折行走 [`crate::wrap`]，从第几行画起走[视口](Viewport)那一份——
//! 与屏上别处一个待遇。**这一处记着「从第几行画起」**，理由与代价见
//! [`Covered::from`](crate::session::state::Covered::from)。
//!
//! 这一格占多大由 [`super::shell`] 分：掀着的时候屏上那几块整个让位，屏底那几行照旧。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use tonefit::{Instruction, Mode as RunMode};

use super::paint::Painted;
use crate::session::live::{Live, Reach};
use crate::session::state::{Action, Key, KeyGroup, Listing, Overlay, Pane, Session, Step};
use crate::session::viewport::Viewport;

/// **一张覆盖层**：抬头一行、正文若干行、右边一条滚动条。
///
/// 收 `&mut Session` 只为一件事：把「从第几行画起」收进这一格真摆得下的那一段里
/// （[`Session::clamp_overlay`]）——只有画的时候才知道这一张折出来几行、这一格有多高，
/// 与逐页表那一处同一条（见 [`super::report::report_pane`]）。
///
/// **正文那几行折行**（[`crate::wrap`]）：键位表那几行短，前提那几行是句子，
/// 两者在窄终端上都折得开——13 号票要在 80×24 上整屏过一遍，而这一格此刻就折得住。
pub(super) fn overlay(frame: &mut Frame, area: Rect, session: &mut Session, live: Option<&Live>) {
    let Some(covered) = session.overlay() else {
        return;
    };
    let which = covered.overlay;
    // 两张差的只有这一句：底下那一副画法一格不分岔。
    let body = match which {
        Overlay::Keys => keys(session),
        Overlay::Premises => premises(live),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        // 抬头摆不下时从中间省略，不由终端库硬截（[`super::yielding::title`]）。
        .title(super::yielding::title(
            &format!("{} · Esc 关", which.what()),
            area.width,
        ));
    // 边框各占一格，正文因此只剩这么大。
    let width = area.width.saturating_sub(2);
    let height = usize::from(area.height.saturating_sub(2));
    let rows: Vec<Line<'static>> = body.iter().flat_map(|row| row.folded(width)).collect();
    session.clamp_overlay(rows.len().saturating_sub(height));
    let from = session.overlay().map_or(0, |covered| covered.from());
    // **视口收的是光标，而这一处没有光标**（覆盖层是读物）：交给它的因此是
    // 「露出来的最后一行」——算出来的起点恰好就是 `from`，而越界由它就近收
    // （见 [`Covered::from`](crate::session::state::Covered::from)）。
    let view = Viewport::new(
        rows.len(),
        height,
        from.saturating_add(height.saturating_sub(1)),
    );
    super::scrolling(frame, area, Paragraph::new(rows).block(block), &view);
}

/// **这一趟的前提**那一张的正文：`profile`、适配方式、裁边、跨页拆分、互锁、
/// 判据构成与聚合（`p3-session-legibility/12` 票面第三条）。
///
/// **措辞出自 [`crate::render::header`]**，会话不另编一份：命令行印出来的报告顶上
/// 摆的是同一段字，而措辞只有一处出处（ADR 0016）。
///
/// 那几行从前摆在卷表**上方、跟着表滚**（`p3-session-legibility/08`）——它们是
/// 「这一趟的前提」，一趟只说一次，而它们占的正是卷表要的行。
///
/// **一趟都没跑过时到不了这里**：那个键在屏底不摆，按下去也先由
/// `super::super::press` 挡一道（它说的那句话与展开那一支同一个形状）。
/// 真到了就说同一句——画不出来的东西不该画成一格空白。
fn premises(live: Option<&Live>) -> Vec<Painted> {
    let Some(live) = live else {
        return vec![Painted::plain(NOT_RUN_YET.to_owned())];
    };
    vec![Painted::plain(crate::render::header(
        live.report(),
        live.mode(),
    ))]
}

/// 一趟都还没跑过时，前提那一张里说什么。
const NOT_RUN_YET: &str = "还没跑过：这一趟的前提要等按下 t 试算或 x 执行才有。";

/// **全部键**那一张的正文：一组一段，一行一件事。
///
/// **一个键都不在这里列**：[`Session::key_table`] 问的是按键表自己，本函数只把
/// 问出来的每一个动作翻成屏上那句话（[`says`]），再把**派得出同一件事的那几个键
/// 并成一行**（`↑ ↓ j k` 是一件事，不是四件）。
///
/// **键那一列对齐**：一列对得齐才扫得动，而这一张就是拿来扫的。
/// 对齐按**显示宽度**算（[`crate::wrap::width`]）——`⇧⇥` 与 `Ctrl-C` 不一样宽。
fn keys(session: &Session) -> Vec<Painted> {
    let table: Vec<(KeyGroup, Vec<(String, &'static str)>)> = session
        .key_table()
        .into_iter()
        .map(|(group, keys)| (group, merged(group, &keys)))
        .collect();
    let column = table
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(spelt, _)| crate::wrap::width(spelt))
        .max()
        .unwrap_or_default();
    let mut rows = Vec::new();
    for (group, listed) in table {
        // 组与组之间空一行：一段一组，扫的时候先认组、再认键。
        // 摆的是一个空格而不是空串：**空文字折出零行**（`crate::wrap::fold`），
        // 而这里要的正是一行。
        if !rows.is_empty() {
            rows.push(Painted::plain(" ".to_owned()));
        }
        rows.push(Painted::plain(format!(" {}", group.title())));
        for (spelt, what) in listed {
            let pad = " ".repeat(usize::from(
                column.saturating_sub(crate::wrap::width(&spelt)),
            ));
            rows.push(Painted::plain(format!("   {spelt}{pad}   {what}")));
        }
    }
    rows
}

/// 一组里**派得出同一件事的那几个键并成一行**，次序照它们在按键表上被问到的次序。
///
/// 并的依据是**屏上那句话**，不是动作本身：`↑` 派的是「往上挪一格」、`↓` 派的是
/// 「往下挪一格」，两个动作，而屏上它们是同一件事（`↑ ↓ 在三层上挪一行`）。
/// 照动作并的话，这一张上一半的行是同一句话的两半。
fn merged(group: KeyGroup, keys: &[(Key, Action)]) -> Vec<(String, &'static str)> {
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    for (key, action) in keys {
        let what = says(group, *action);
        match rows.iter_mut().find(|(_, said)| *said == what) {
            Some((spelt, _)) => {
                spelt.push(' ');
                spelt.push_str(&spelled(*key));
            }
            None => rows.push((spelled(*key), what)),
        }
    }
    rows
}

/// 一个键在屏上怎么写。**与屏底那一行写的是同一批记号**（`⏎`、`⇥`、`⇧⇥`、`Esc`、
/// `Ctrl-C`）：同一个键在两处长得不一样的话，读的人要先认出它们是一个。
fn spelled(key: Key) -> String {
    match key {
        Key::Up => "↑".to_owned(),
        Key::Down => "↓".to_owned(),
        Key::Left => "←".to_owned(),
        Key::Right => "→".to_owned(),
        Key::Enter => "⏎".to_owned(),
        Key::Space => "空格".to_owned(),
        Key::Tab => "⇥".to_owned(),
        Key::BackTab => "⇧⇥".to_owned(),
        Key::Backspace => "⌫".to_owned(),
        Key::Esc => "Esc".to_owned(),
        Key::Interrupt => "Ctrl-C".to_owned(),
        Key::Char(letter) => letter.to_string(),
    }
}

/// 一个动作在这一张表上怎么说。**这一层只管措辞**——**哪些键派得出它**由
/// [`Session::key_table`] 答（那一处问的是按键表自己）。
///
/// **几支随[组](KeyGroup)而变**：同一个 [`Action::Move`] 在左栏上挪的是一行配置、
/// 在取值栏上挪的是一格取值、在逐页表上挪的是一页——动作相同，说的不是同一件事。
/// 别的几支与组无关，落在同一句话上。
///
/// **打字那几支到不了这一张表**（[`Action::Insert`]、[`Action::Backspace`]、
/// [`Action::Complete`]、[`Action::Commit`]、[`Action::Store`]）：编辑一行与打预设名
/// 两块不在这张表上（见 [`KeyGroup`]）。这张表照旧列全——少列一支，
/// 往后添一个新动作时这里不会红。
fn says(group: KeyGroup, action: Action) -> &'static str {
    match action {
        Action::Move(_) => match group {
            KeyGroup::Valuing => "在这一列取值上挪一格",
            KeyGroup::Expanded => "在逐页表上挪一页",
            KeyGroup::Picking => "在这一栏上挪一份",
            _ => "在三层上挪一行",
        },
        Action::Select(_) => "在卷表上挪一卷",
        Action::Cycle(_) => "就地换一个取值（不摊开）",
        Action::Unfold => "摊开这一行的取值",
        Action::Drill => "进去看这块面板底下的型号",
        Action::Choose => "把停着的这一格定下来",
        Action::Toggle => "把这一卷勾上／勾掉",
        Action::Edit => match group {
            KeyGroup::Picking => "打一个名字，存成一份预设",
            _ => "打字改这一行",
        },
        Action::Remove => "把这一条卷删掉",
        Action::Insert(_) => "把这个字添进缓冲",
        Action::Backspace => "退掉一个字",
        Action::Complete => "把这一层补出来",
        Action::Commit => "收下打的东西",
        Action::Cancel => match group {
            KeyGroup::Valuing => "一格不改地回左栏",
            _ => "退一步，回配置",
        },
        Action::Start(RunMode::DryRun) => "试算：只算不写，报告照出",
        Action::Start(RunMode::Process) => "执行：写到输出根",
        Action::Stop => "停：按一次收尾，再按一次中止",
        Action::Answer(Instruction::Continue, Reach::ThisVolume) => "接着做第二遍（第一遍不重算）",
        Action::Answer(Instruction::Continue, Reach::ForTheRest) => "剩下的卷都这样（往下不再问）",
        Action::Answer(..) => "收尾（这一卷不写，等价 dry-run）",
        Action::Focus(Pane::Report) => "把焦点切到报告区",
        Action::Focus(Pane::Config) => "把焦点切回左栏",
        Action::Follow => "回到跟随：光标交回给最新那一卷",
        Action::Expand => "把这一卷的逐页摊开",
        Action::Turn(Step::Next) => "换下一卷",
        Action::Turn(_) => "换上一卷",
        Action::Collapse => "收起，左栏回来",
        Action::List(Listing::All) => "列全部页",
        Action::List(_) => "只列要紧的页",
        Action::Pick => "开预设那一栏",
        Action::Take => "套用停着的那一份",
        Action::Store => "存下来",
        Action::Erase => "删掉停着的那一份（按两下）",
        Action::Chart => "按这块面板出一张标定图",
        Action::Reveal(overlay) => overlay.what(),
        Action::Quit => "退出会话",
        // 派不出动作的键根本不进这张表（[`Session::keys_of`] 先滤了一道）。
        Action::Ignored => "在这里没有意义",
    }
}

#[cfg(test)]
mod tests {
    use super::super::probe::{a_run_in_flight, same_screen, screen, snapshot, tight};
    use super::super::shell;
    use super::*;
    use crate::session::live::Volume;
    use crate::session::state::{Expansion, Key, Stage};

    /// 掀开一张覆盖层的会话（`?` 那一张）。
    fn key_table() -> Session {
        let mut session = Session::new();
        session.press(Key::Char('?'));
        session
    }

    /// **快照：`?` 那张键位表**（票面第二条与「快照各一张」那一条）。
    ///
    /// 钉的是 **80×24 整屏**——13 号票要在这一档上过一遍，而这一张此刻就摆得下：
    /// 一格覆盖层加屏底那三行，右边那条滚动条说出「下面还有」。
    ///
    /// 屏上读得出的五件事：
    ///
    /// - **按焦点分组**，头一层分岔就是按键表自己的头一层（ADR 0017 决定第 2 条）；
    /// - **一行一件事，派得出它的那几个键并在行首**（`↑ ↓ j k` 是一件事，不是四件）；
    /// - **屏底那一行摆不下的那几个键在这里**：`c 出标定图`、`e 展开`、`p 预设`
    ///   在瘦身之后的屏底上一个都没有（见 `super::super::footer::browsing_keys`）；
    /// - **一趟都没跑过时报告区与展开着两组整组不出**（[`KeyGroup::reachable`]）：
    ///   那时 `⇥` 进不去，展开也无从谈起——屏上不摆按不动的键，
    ///   而一张进不去的块的键位表是同一件事；
    /// - **`Esc` 归各块自己列，不进「任何时候」**：它在左栏上是退出会话、在取值栏上是
    ///   一格不改地回去——摆进「任何时候」的话，这张表就在几块上说了假话。
    ///   屏底那一行此刻摆的是**这一块自己的两个键加 `Ctrl-C`**，末尾不再是 `? 全部键`
    ///   （掀着的时候那个键是「关掉」），而前提那一张也不摆——这一趟还没跑过。
    #[test]
    fn the_key_table_overlay_groups_the_keys_by_focus() {
        let mut session = key_table();
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, None), 80, 24),
            THE_KEY_TABLE,
        );

        // 滚到底：剩下那几组连同「任何时候」都在。**这一张只列此刻这个阶段派得出的键**
        // ——没跑过时按停与答话那几个一个都不在（票面：屏上不摆按不动的键）。
        for _ in 0..40 {
            session.press(Key::Down);
        }
        let bottom = tight(&screen(&mut session, None, 80, 24));
        for said in ["预设栏", "任何时候", "退出会话", "全部键", "这一趟的前提"]
        {
            assert!(bottom.contains(&tight(said)), "{said}：{bottom}");
        }
        assert!(!bottom.contains(&tight("按一次收尾")), "{bottom}");
    }

    /// 见 [`the_key_table_overlay_groups_the_keys_by_focus`]。
    const THE_KEY_TABLE: &str = r#"
"┌全部键 · Esc 关───────────────────────────────────────────────────────────────┐"
"│ 左栏 · 三层配置                                                              ▲"
"│   ↑ ↓ j k    在三层上挪一行                                                  █"
"│   ← →        就地换一个取值（不摊开）                                        █"
"│   ⏎ 空格     摊开这一行的取值                                                █"
"│   Esc        退出会话                                                        █"
"│   c          按这块面板出一张标定图                                          █"
"│   e          把这一卷的逐页摊开                                              █"
"│   p          开预设那一栏                                                    █"
"│   t          试算：只算不写，报告照出                                        ║"
"│   x          执行：写到输出根                                                ║"
"│                                                                              ║"
"│ 取值栏 · 摊开的那一列                                                        ║"
"│   ↑ ↓ j k    在这一列取值上挪一格                                            ║"
"│   ← Esc      一格不改地回左栏                                                ║"
"│   → ⏎ 空格   把停着的这一格定下来                                            ║"
"│                                                                              ║"
"│ 预设栏                                                                       ║"
"│   ↑ ↓ j k    在这一栏上挪一份                                                ║"
"│   ⏎ 空格     套用停着的那一份                                                ▼"
"└──────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 读 · Esc 关（回到刚才那一块） · Ctrl-C 退出会话                             "
" 只列此刻这个阶段派得出的键，按焦点分组——屏底那一行摆的是最常用的几个，这里是全 "
" 部                                                                             "
"#;

    /// **快照：这一趟的前提**（票面第三条与「快照各一张」那一条）。
    ///
    /// **同一副形状**：同一格边框、同一句 `Esc 关`、同一副屏底那两行——
    /// 与上一张差的只有正文那几行是谁出的（[`overlay`] 那一句 `match`）。
    ///
    /// 措辞逐字出自 [`crate::render::header`]：命令行印出来的报告顶上摆的是同一段字。
    #[test]
    fn the_premises_overlay_carries_the_header_the_volume_table_used_to_wear() {
        let live = a_run_in_flight(true);
        let mut session = Session::new();
        session.press(Key::Char('i'));
        same_screen(
            &snapshot(|frame| shell(frame, &mut session, Some(&live)), 80, 16),
            THE_PREMISES,
        );

        // 措辞与命令行那一份**逐字同源**：这一格印的就是 `render::header` 出的那一段。
        let shown = tight(&screen(&mut session, Some(&live), 120, 30));
        for said in crate::render::header(live.report(), live.mode()).lines() {
            assert!(shown.contains(&tight(said)), "{said}：{shown}");
        }
    }

    /// 见 [`the_premises_overlay_carries_the_header_the_volume_table_used_to_wear`]。
    const THE_PREMISES: &str = r#"
"┌这一趟的前提 · Esc 关─────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲 │"
"│测标定于 boox-poke6，其余面板未复核）                                         │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                               │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）     │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定 │"
"│于 boox-poke6，其余面板未复核）                                               │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）             │"
"│                                                                              │"
"│                                                                              │"
"│                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────┘"
" ↑↓ 读 · Esc 关（回到刚才那一块） · ? 全部键 · Ctrl-C 退出会话                  "
" 这一趟的前提：一趟只说一次，因此不占卷表那几行——它们说的是这一份报告是照哪几条 "
" 算出来的                                                                       "
"#;

    /// **覆盖层盖住一块，不替掉它**：`Esc` 关掉之后回的是刚才那一块，一格没动。
    ///
    /// 展开着那一块最认得出来：它带着「展开的是哪一卷、列的是哪几页、光标停在第几页」
    /// 三样东西，而覆盖层掀开又关掉之后三样都还在——记一个「回哪儿去」的小枚举的话，
    /// 关掉的那一刻要把这三样重新拼一遍。
    #[test]
    fn an_overlay_covers_a_block_and_gives_it_back_untouched() {
        let live = a_run_in_flight(false);
        let mut session = Session::new();
        session.expand(Expansion::new(Volume::Settled(1)));
        session.press(Key::Down);
        let expanded = session.expansion().copied().expect("展开着");

        session.press(Key::Char('?'));
        assert!(
            session.expansion().is_none(),
            "覆盖层掀着时展开那一块不在场"
        );
        // 掀着的时候屏上只有这一格：左栏、总览块、报告区一个都不画。
        let covering = tight(&screen(&mut session, Some(&live), 120, 30));
        assert!(covering.contains(&tight("全部键")), "{covering}");
        assert!(!covering.contains(&tight("总览")), "{covering}");
        assert!(!covering.contains(&tight("设备层")), "{covering}");

        session.press(Key::Esc);
        assert_eq!(session.expansion().copied(), Some(expanded), "没原样还回来");
    }

    /// **装不下时滚得动**（票面第六条，走 `04` 那套视口）。
    ///
    /// 一格矮到摆不下这一张时：`↓` 往下读一行，`↑` 读回来，**往下滚到末一行为止**
    /// ——再按不动了（那一头由 [`Session::clamp_overlay`] 每帧收一次）。
    /// **屏上不摆按不动的键**在这一处是「按了真有反应」的另一半。
    #[test]
    fn an_overlay_too_tall_for_its_box_scrolls() {
        let mut session = key_table();
        let top = screen(&mut session, None, 80, 12);
        assert!(tight(&top).contains(&tight("左栏 · 三层配置")), "{top}");

        session.press(Key::Down);
        let scrolled = screen(&mut session, None, 80, 12);
        assert_ne!(top, scrolled, "按了 ↓ 一行都没动");
        session.press(Key::Up);
        assert_eq!(screen(&mut session, None, 80, 12), top, "按 ↑ 回不到原处");

        // 往下按到底：末一组在屏上，而**再按也不多滚一行**。
        for _ in 0..200 {
            session.press(Key::Down);
        }
        let bottom = screen(&mut session, None, 80, 12);
        assert!(tight(&bottom).contains(&tight("任何时候")), "{bottom}");
        session.press(Key::Down);
        assert_eq!(screen(&mut session, None, 80, 12), bottom, "滚过头了");
        // 一下就回得来：按住 ↑ 收在头一行上。
        for _ in 0..200 {
            session.press(Key::Up);
        }
        assert_eq!(screen(&mut session, None, 80, 12), top, "回不到头一行");
    }

    /// **这一张表是按键表自己，不是另抄的一份**（票面：`?` 那张表要从按键表取）。
    ///
    /// 两头对：**按键表派得出的每一个动作都在这一张上**（逐个键问一遍
    /// [`Session::action`]，问出来不是「没有意义」的就该在屏上读得到它那句话），
    /// 而**按键表派不出的一个都不在**（`z` 在这一屏上一处都没有）。
    ///
    /// 这一条钉的是那条约束本身：改一处按键表，这一张跟着变——它不会漏、也不会多。
    #[test]
    fn the_key_table_is_the_key_table_itself() {
        let session = key_table();
        let mut shown = Session::new();
        shown.press(Key::Char('?'));
        let screen = tight(&screen(&mut shown, None, 120, 60));

        for (group, keys) in session.key_table() {
            assert!(
                screen.contains(&tight(group.title())),
                "{group:?} 那一组不在屏上"
            );
            for (key, action) in keys {
                assert_ne!(action, Action::Ignored);
                assert!(
                    screen.contains(&tight(says(group, action))),
                    "{key:?} 派的那件事不在屏上：{}",
                    says(group, action)
                );
                assert!(screen.contains(&tight(&spelled(key))), "{key:?} 不在屏上");
            }
        }
        // 一个没有主的字母在这一张上一处都没有。
        assert!(!screen.contains('z'), "{screen}");
    }

    /// **阶段那一维那几个键只在「任何时候」那一组里列一遍**（ADR 0017 决定第 2 条）。
    ///
    /// 焦点那一维上摆得下它们的三块（左栏、报告区、展开着）都把自己不认的键交给
    /// `stage_action`，照实列的话 `q 退出` 会在四组里各出现一次——而屏上同一个键
    /// 说两遍就是两份措辞。
    #[test]
    fn the_keys_the_stage_answers_are_listed_once() {
        let mut session = Session::new();
        session.run_started();
        assert_eq!(
            session.stage(),
            Stage::Running(tonefit::Instruction::Continue)
        );
        session.press(Key::Char('?'));

        let listed: Vec<crate::session::state::KeyGroup> = session
            .key_table()
            .into_iter()
            .filter(|(_, keys)| keys.iter().any(|(key, _)| *key == Key::Char('s')))
            .map(|(group, _)| group)
            .collect();
        assert_eq!(listed, vec![KeyGroup::Always], "按停那个键列了不止一处");

        // 跑着的时候它在这一张上：**只列此刻这个阶段派得出的键**的另一半。
        let running = tight(&screen(&mut session, None, 120, 60));
        assert!(running.contains(&tight("按一次收尾")), "{running}");
    }
}
