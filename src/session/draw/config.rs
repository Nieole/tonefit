//! 屏上那一块：**左栏**——三层配置常驻的那一栏（`CONTEXT.md` 的《会话》：三层）。
//!
//! 一层一块、按生命周期从上到下，每一行是「标签 + 取值」。**跑起来之后整栏只读**，
//! 而那件事要在屏上看得出来，不能是按了没反应（见 [`config`]）。
//!
//! 这一栏在这一屏上占多宽、什么时候整个收起，归 [`super::config_width`]——
//! 那是布局的事；本模块只画格子里的东西。折行走终端库自己的 [`Wrap`]
//! （理由见 `crate::wrap` 的模块文档）。
//!
//! **卷打得多了这一栏就装不下**（行数是 21 加卷数）：从第几行画起由
//! [`Viewport`] 算，滚动条与正文一起画（见 [`super::scrolling`]）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::session::state::{Field, Layer, Mode, Session};
use crate::session::viewport::Viewport;

/// 跑起来之后左栏的抬头。**「只读」要看得出来**，不能是按了没反应
/// （`CONTEXT.md` 的《会话》：一趟跑起来之后三层都只读）。
const READ_ONLY_TITLE: &str = "配置 · 跑着，三层都只读";

/// 左栏：三层，各占一块，按生命周期从上到下。
///
/// **跑起来之后整栏只读**，而这一条要在屏上**看得出来**，不能是「按了没反应」：
/// 抬头改口（[`READ_ONLY_TITLE`]），光标不再反白，各行压暗。
/// 真正拦住按键的不是这里——是状态机在那个状态下一个改动键都不派
/// （见 `super::super::state::running_action`）；这里只把那件事说出来。
///
/// **反白只在左栏就是眼下动手的地方时才给**：那一格反白说的是「就在这一行上动手」。
/// 跑着时按不动（上面那一条），预设那一栏开着时按键**全归那一栏**——左栏此刻在屏上
/// 只为让人对照着看存出去的是什么（见 [`super::shell`]），反白它就是在指一个按不动的地方。
/// 压暗仍只给跑着那一种：那一种是「这一趟没跑完都改不动」，而预设那一栏一个 `Esc` 就回来了。
///
/// **打进来的卷多过这一格装得下的行数时，视口跟着光标走**（见 [`Viewport`]）：
/// 从前这一栏一点滚动都没有——行数是 21 加卷数，24 行的终端上打进三个卷，
/// 光标就走到屏外，而屏上看不出它去哪儿了。滚动量**算出来、不记着**：
/// 光标在哪一行，这一栏就跟到哪一行。
pub(super) fn config(frame: &mut Frame, area: Rect, session: &Session) {
    let running = matches!(session.mode(), Mode::Running(_));
    let acting = matches!(session.mode(), Mode::Browsing | Mode::Editing(_));
    let focus = session.focus();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut drawn: Option<Layer> = None;
    // 光标那一行落在正文的第几行：层与层之间还垫着抬头与空行，行号数不出来
    // （见 [`Viewport::new`]）。**跑着与预设那一栏开着时它照旧跟着 `focus` 走**：
    // 那两种只是不反白，光标本身没有挪窝，视口跟丢了才是屏上说不通的事。
    let mut cursor = 0;
    for field in session.rows() {
        let layer = field.layer();
        if drawn != Some(layer) {
            if drawn.is_some() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                layer.title().to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            drawn = Some(layer);
        }
        let style = match (running, acting && field == focus) {
            (true, _) => Style::default().add_modifier(Modifier::DIM),
            (false, true) => Style::default().add_modifier(Modifier::REVERSED),
            (false, false) => Style::default(),
        };
        if field == focus {
            cursor = lines.len();
        }
        lines.push(row(session, field, style));
    }
    let view = Viewport::new(
        lines.len(),
        usize::from(area.height.saturating_sub(2)),
        cursor,
    );
    let body = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(if running {
            READ_ONLY_TITLE
        } else {
            "配置"
        }))
        // 折行而不是切掉：阈值那一行要把**标定来源**原样带上来（spec 的 Further Notes），
        // 而那句话比这一栏宽；路径也一样，切掉尾巴的路径看不出是哪一个。
        // `trim: false` 让折下来的那一截保留缩进，读得出它还是上一行的。
        .wrap(Wrap { trim: false });
    super::scrolling(frame, area, body, &view);
}

/// 左栏上的一行：名字 + 取值。怎么标（反白、压暗、还是原样）由 [`config`] 定。
fn row(session: &Session, field: Field, style: Style) -> Line<'static> {
    let text = match field {
        // 卷那一行的取值里已经带着勾与路径，再挂一个「卷」字是废话。
        Field::Volume(_) => format!("  {}", session.shown(field)),
        Field::AddVolume => format!("  {}", field.label()),
        _ => format!("  {:　<8}{}", field.label(), session.shown(field)),
    };
    Line::from(Span::styled(text, style))
}

#[cfg(test)]
mod tests {
    use super::super::probe::{
        reversed_cells, reversed_rows, same_screen, screen, snapshot, tight,
    };
    use super::*;
    use crate::session::state::Key;

    /// 打进来几个卷的一个会话。走的是真会话那条路（停在「＋ 再打一个卷进来」上打字），
    /// 而不是往里塞一份状态：卷打进来之后光标停在哪一行是本票要问的事，
    /// 塞进去就把那一半跳过了。
    fn with_volumes(count: usize) -> Session {
        let mut session = Session::new();
        for at in 1..=count {
            session.focus_on(Field::AddVolume);
            session.press(Key::Enter);
            for glyph in format!("库/卷{at}").chars() {
                session.press(Key::Char(glyph));
            }
            session.press(Key::Enter);
        }
        session
    }

    /// 把左栏单独画进一格里。
    fn config_pane(session: &Session, width: u16, height: u16) -> String {
        snapshot(|frame| config(frame, frame.area(), session), width, height)
    }

    /// **跑起来之后：三层只读这件事在屏上看得出来**，而不是按了没反应（本票的验收）。
    ///
    /// 三样各说一遍同一件事：抬头改口说「只读」、光标那一行不再反白、改一行的那几个键
    /// 一个都不提。反白那一格非验不可——它说的是「就在这一行上动手」，而这时按不动。
    #[test]
    fn a_run_in_progress_says_on_screen_that_the_three_layers_are_read_only() {
        let mut session = Session::new();
        let before = tight(&screen(&mut session, None, 120, 40));
        assert!(before.contains(&tight("←→ 换一个")), "{before}");
        assert!(reversed_rows(&mut session) > 0, "浏览时光标那一行该反白");

        session.run_started();
        assert_eq!(reversed_rows(&mut session), 0, "跑着时光标还反白着");
        let running = tight(&screen(&mut session, None, 120, 40));

        assert!(running.contains(&tight(READ_ONLY_TITLE)), "{running}");
        // 三层还在屏上（看得见配的是什么），只是改不动了。
        for layer in [Layer::Device, Layer::Taste, Layer::Scope] {
            assert!(running.contains(&tight(layer.title())), "{running}");
        }
        // 改一行的那几个键一个都不提。试算与执行那两个键不在这张单子上，
        // 是因为它们还印在报告区那段「还没跑过」的说明里——真会话里那一段这时早换成了
        // 攒着的报告（`live` 一起线程就有了），这里 `screen` 传的是 `None`。
        // 「跑着时按不动 t 与 x」由按键表那一条钉住（`super::state` 的
        // `which_keys_do_what_in_which_state` 第六段）。
        for keys in ["←→ 换一个", "⏎ 改", "空格 勾上"] {
            assert!(
                !running.contains(&tight(keys)),
                "{keys} 还在屏上：{running}"
            );
        }
    }

    /// **左栏装不下时光标仍在屏上，上下走到头会跟着滚**（本票的验收头一条）。
    ///
    /// 行数是 21 加卷数：24 行的终端上打进三个卷正好 24 行，而那一格里面只有 22 行。
    /// 从前这一栏一点滚动都没有——光标停在「＋ 再打一个卷进来」上，屏上却看不到它，
    /// 也看不出它去哪儿了。
    ///
    /// 两头各问一次：光标在末尾时开头那几行让出去，走回开头时反过来。
    /// **反白那一格非验不可**——逐格拼回来的文字看不出光标停在哪一行，
    /// 而「光标仍在屏上」问的正是它（数反白用 `super::super::probe::reversed_cells`：
    /// 这几条只画左栏一格，`reversed_rows` 那个按列切的办法在这里用不上）。
    ///
    /// **只问 52 列这一档**（[`super::CONFIG_WIDTH`]，这一栏装得下的正常宽度）：
    /// 那里一行都不折，视口数的逻辑行与屏上的行一一对应。窄到左栏让出宽度那一档
    /// 上行行都折，两个数就对不上了——那一笔账记在停车场 **Q136**，本票没收它。
    #[test]
    fn a_config_column_taller_than_its_box_scrolls_with_the_cursor() {
        let mut session = with_volumes(3);
        assert_eq!(
            session.rows().len() + 5,
            24,
            "21 行加卷数，加的是三块抬头与两个空行"
        );

        // 光标停在末尾那一行上：它在屏上，而开头那几行让了出去。
        let bottom = tight(&config_pane(&session, 52, 24));
        assert!(
            bottom.contains(&tight(Field::AddVolume.label())),
            "光标那一行掉出屏外了：{bottom}"
        );
        assert!(
            !bottom.contains(&tight(Layer::Device.title())),
            "视口没跟着光标走：{bottom}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 52, 24) > 0,
            "光标那一行不在屏上：{bottom}"
        );

        // 一路走回开头：这一回让出去的是末尾那几行。
        for _ in 0..session.rows().len() - 1 {
            session.press(Key::Up);
        }
        let top = tight(&config_pane(&session, 52, 24));
        assert!(
            top.contains(&tight(Layer::Device.title())),
            "翻不回顶上：{top}"
        );
        assert!(
            !top.contains(&tight(Field::AddVolume.label())),
            "开头那几行在屏上，末尾那一行也在：{top}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 52, 24) > 0,
            "光标那一行不在屏上：{top}"
        );
    }

    /// **快照：左栏装不下的那一张**（本票的验收「快照：左栏装不下时的一张」）。
    ///
    /// 钉的是两件事：开头那两行让了出去（正文从「型号」下面那一行画起），
    /// 以及**滚动条画在右边那条框线上**——`▲`／`▼` 两头加中间那一截滑块说的是
    /// 「上面还有、下面还有」。滚动条走终端库自带的那个 widget，一列正文都不吃
    /// （见 [`super::scrolling`]）。
    ///
    /// **装得下时一条都不画**：那一格因此与没有这一段时逐格相同。
    /// 「装不装得下」按的也是逻辑行（同上一条：Q136）。
    #[test]
    fn the_config_column_that_does_not_fit() {
        same_screen(
            &config_pane(&with_volumes(3), 52, 24),
            THE_CONFIG_COLUMN_THAT_DOES_NOT_FIT,
        );

        // 一个卷都没打进来时是 21 行，22 行的格子装得下——滚动条一格都不画。
        let fits = config_pane(&Session::new(), 52, 24);
        assert!(
            !fits.contains('▲') && !fits.contains('▼'),
            "装得下还画了滚动条：{fits}"
        );
    }

    /// 见 [`the_config_column_that_does_not_fit`]。
    const THE_CONFIG_COLUMN_THAT_DOES_NOT_FIT: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│  感知可分辨级数　默认（跟随面板）                ▲"
"│  阈值　　　　　　跟着型号走（先挑一个）          ║"
"│                                                  █"
"│口味层 · 这一趟的立场                             █"
"│  适配方式　　　　默认（height）                  █"
"│  裁边　　　　　　默认（裁）                      █"
"│  跨页拆分　　　　默认（拆）                      █"
"│  拆分阈值　　　　默认（1.5）                     █"
"│  阅读方向　　　　默认（rtl）                     █"
"│  滤波器　　　　　默认（lanczos3）                █"
"│  位深　　　　　　自动（判据说了算）              █"
"│  抖动　　　　　　自动（判据说了算）              █"
"│  逐页　　　　　　默认（关）                      ║"
"│  缓存预算　　　　默认（512.0 MiB）               ║"
"│  读取策略　　　　默认（auto）                    ║"
"│                                                  ║"
"│范围层 · 每趟都不同，不进预设                     ║"
"│  输出根　　　　　未填（跑起来之前必填）          ║"
"│  [x] 库/卷1                                      ║"
"│  [x] 库/卷2                                      ║"
"│  [x] 库/卷3                                      ║"
"│  ＋ 再打一个卷进来                               ▼"
"└──────────────────────────────────────────────────┘"
"#;
}
