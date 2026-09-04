//! 屏上那一块：**左栏**——三层配置常驻的那一栏（`CONTEXT.md` 的《会话》：三层）。
//!
//! 一层一块、按生命周期从上到下，每一行是「标签 + 取值」。**跑起来之后整栏只读**，
//! 而那件事要在屏上看得出来，不能是按了没反应（见 [`config`]）。
//!
//! 这一栏在这一屏上占多宽、什么时候整个收起，归 [`super::config_width`]——
//! 那是布局的事；本模块只画格子里的东西。折行走终端库自己的 [`Wrap`]
//! （理由见 `crate::wrap` 的模块文档）。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::session::state::{Field, Layer, Mode, Session};

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
pub(super) fn config(session: &Session) -> Paragraph<'static> {
    let running = matches!(session.mode(), Mode::Running(_));
    let acting = matches!(session.mode(), Mode::Browsing | Mode::Editing(_));
    let focus = session.focus();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut drawn: Option<Layer> = None;
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
        lines.push(row(session, field, style));
    }
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(if running {
            READ_ONLY_TITLE
        } else {
            "配置"
        }))
        // 折行而不是切掉：阈值那一行要把**标定来源**原样带上来（spec 的 Further Notes），
        // 而那句话比这一栏宽；路径也一样，切掉尾巴的路径看不出是哪一个。
        // `trim: false` 让折下来的那一截保留缩进，读得出它还是上一行的。
        .wrap(Wrap { trim: false })
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
    use super::super::probe::{reversed_rows, screen, tight};
    use super::*;

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
}
