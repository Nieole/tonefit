//! 会话的画法：左窄配置常驻 + 右宽主区（spec 的《会话：布局与交互》）。
//!
//! **这一票只画骨架。** 主区那一格眼下是空的：全局条、当前卷条与报告区归 `p1-session/09`，
//! 逐页展开与左栏收起归 `11`。这里留出位置、不预先替它们决定长什么样。
//!
//! 措辞不在这里长第二份：报告那一套在 [`crate::render`]，命令行与会话共用。
//! 本模块只写左栏那几行**配置**的标签与按键提示——那两样命令行上没有，
//! 会话是它们唯一的出处。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::complete;
use super::state::{Edit, Field, Layer, Mode, Session, Shape};

/// 左栏的宽度。配置一直在场，改一下就能在右边看到影响。
///
/// 固定列数而不是按比例：这一栏装的是**标签加取值**，两边都不随终端变宽而变长，
/// 按比例分只会在宽终端上留下一栏空白。
const CONFIG_WIDTH: u16 = 52;

/// 屏底那几行：编辑条、补全候选、要说的那句话。
const FOOTER_HEIGHT: u16 = 3;

/// 把一屏画出来。
pub fn shell(frame: &mut Frame, session: &Session) {
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(FOOTER_HEIGHT)])
        .areas(frame.area());
    let [left, main] =
        Layout::horizontal([Constraint::Length(CONFIG_WIDTH), Constraint::Min(0)]).areas(body);

    frame.render_widget(config(session), left);
    frame.render_widget(main_pane(), main);
    frame.render_widget(self::footer(session), footer);
}

/// 左栏：三层，各占一块，按生命周期从上到下。
fn config(session: &Session) -> Paragraph<'static> {
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
        lines.push(row(session, field, field == focus));
    }
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("配置"))
        // 折行而不是切掉：阈值那一行要把**标定来源**原样带上来（spec 的 Further Notes），
        // 而那句话比这一栏宽；路径也一样，切掉尾巴的路径看不出是哪一个。
        // `trim: false` 让折下来的那一截保留缩进，读得出它还是上一行的。
        .wrap(Wrap { trim: false })
}

/// 左栏上的一行：名字 + 取值，光标停着的那一行反白。
fn row(session: &Session, field: Field, focused: bool) -> Line<'static> {
    let text = match field {
        // 卷那一行的取值里已经带着勾与路径，再挂一个「卷」字是废话。
        Field::Volume(_) => format!("  {}", session.shown(field)),
        Field::AddVolume => format!("  {}", field.label()),
        _ => format!("  {:　<8}{}", field.label(), session.shown(field)),
    };
    let style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(Span::styled(text, style))
}

/// 右边那一大格。眼下它是空的，位置留给 `p1-session/09`。
fn main_pane() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(""),
        Line::from("  试算、执行与报告区画在这里。"),
        Line::from("  眼下先把左栏配好：型号与输出根是跑起来之前必须填的两项。"),
    ])
    .block(Block::default().borders(Borders::ALL).title("主区"))
}

/// 屏底：正在打字就显示缓冲与这一层列出来的候选，否则显示按键提示。
fn footer(session: &Session) -> Paragraph<'static> {
    let mut lines = match session.mode() {
        Mode::Editing(edit) => editing_lines(session, edit),
        Mode::Browsing => vec![Line::from(browsing_keys(session)), Line::from("")],
    };
    lines.push(Line::from(session.notice().unwrap_or("").to_owned()));
    Paragraph::new(lines)
}

fn editing_lines(session: &Session, edit: &Edit) -> Vec<Line<'static>> {
    let keys = match edit.field.shape() {
        Shape::Path => "⇥ 补这一层 · ⏎ 收下 · Esc 丢掉",
        _ => "⏎ 收下 · Esc 丢掉",
    };
    vec![
        Line::from(format!(" {} {}▏   {keys}", edit.field.label(), edit.buffer)),
        // 只列打到的那一层，且**只是列出来**：不留索引、不留缓存（ADR 0009）。
        Line::from(format!(" {}", listed(session, edit))),
    ]
}

/// 补全列出来的那一层，摆成一行。空着就说一句这一层还没列过。
fn listed(session: &Session, edit: &Edit) -> String {
    if edit.candidates.is_empty() {
        // 有话要说时这一行让位——那句话就印在下一行。
        return match session.notice() {
            Some(_) => String::new(),
            None => "按 ⇥ 列出这一层".to_owned(),
        };
    }
    // 只留这一层里的那个名字，切法在 `complete` 那一侧——分隔符表只有一份。
    let names: Vec<&str> = edit
        .candidates
        .iter()
        .map(|hit| complete::name(hit))
        .take(12)
        .collect();
    format!("这一层：{}", names.join("  "))
}

/// 浏览时的按键提示，随光标停的那一行而变——按不动的键不该印在屏上。
fn browsing_keys(session: &Session) -> String {
    let common = "↑↓ 选 · q 退出";
    match session.focus().shape() {
        Shape::Cycle => format!(" ←→ 换一个 · {common}"),
        Shape::Text => format!(" ⏎ 改 · {common}"),
        Shape::Path => format!(" ⏎ 打一个路径进来（⇥ 逐层补全）· {common}"),
        Shape::Volume => format!(" 空格 勾上／勾掉 · d 删掉这一条 · {common}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::Key;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// 屏上的文字，**空白全去掉**再比。
    ///
    /// 宽字符在缓冲里占两格，第二格被 ratatui 重置成一个空格——逐格读回来的文字
    /// 因此在每个汉字之间多一个空格。要问的是「这几个字在不在屏上」，
    /// 两边都去掉空白最省事，也不会把断言比松：左栏这些标签没有一个靠空格分辨。
    fn tight(text: &str) -> String {
        text.chars()
            .filter(|glyph| !glyph.is_whitespace())
            .collect()
    }

    /// 把一屏画出来，取回屏上的文字。
    fn screen(session: &Session, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        terminal
            .draw(|frame| shell(frame, session))
            .expect("画得出来");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    /// 左栏按三块显示，各项都在屏上；主区那一格留着。
    ///
    /// 这不是 `p1-session/09` 那批渲染快照——报告区还没有东西可快照。
    /// 它钉的是本票的骨架：三块都画得出来、每一行的名字都在。
    #[test]
    fn the_shell_draws_three_layers_and_leaves_the_main_pane() {
        let session = Session::new();

        let screen = screen(&session, 120, 40);

        let screen = tight(&screen);
        for layer in [Layer::Device, Layer::Taste, Layer::Scope] {
            assert!(
                screen.contains(&tight(layer.title())),
                "{layer:?} 那一块没画出来"
            );
        }
        for field in session.rows() {
            assert!(
                screen.contains(&tight(field.label())),
                "{field:?} 那一行没画出来：{screen}"
            );
        }
        assert!(screen.contains("主区"), "主区那一格没留出来");
    }

    /// 打字时屏底摆着缓冲与这一层列出来的候选。
    #[test]
    fn typing_a_path_shows_the_buffer_and_the_level_underneath() {
        let mut session = Session::new();
        session.focus_on(Field::Out);
        session.press(Key::Enter);
        for character in "库".chars() {
            session.press(Key::Char(character));
        }

        let screen = tight(&screen(&session, 120, 40));

        assert!(screen.contains("输出根库"), "{screen}");
        assert!(screen.contains("补这一层"), "{screen}");
    }

    /// 终端窄到放不下时不恐慌——画得难看是一回事，崩掉是另一回事。
    #[test]
    fn a_terminal_too_narrow_for_the_left_column_still_draws() {
        let session = Session::new();

        // 比左栏还窄，且高度只够画个边框。
        screen(&session, 20, 6);
        screen(&session, 1, 1);
    }
}
