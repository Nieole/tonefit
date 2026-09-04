//! 屏上那一块：**预设栏 (Picker)**——列出盘上那几份预设、并把当前两层存成一份的那一栏
//! （`CONTEXT.md` 的《会话》：预设栏）。
//!
//! 它**占的是主区，左栏照旧在场**（见 [`super::shell`]）：存出去的就是左栏上那两层，
//! 而「存的是什么」在按下去之前得看得见。这一栏此刻按得动哪几个键写在屏底，
//! 归 [`super::footer`]。
//!
//! 名字多过这一格装得下的行数时视口跟着光标走（[`opens_from`]）——
//! 这一栏自己没有滚动状态。

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::session::state::Picker;

/// 预设那一栏末尾那一行。**说的是「当前两层」而不是「这一份配置」**：
/// 存出去的不含范围层，而那一行是屏上唯一说得到这件事的地方
/// （抬头说的是这一栏装什么，这一行说的是按下去会存什么）。
const ADD_PRESET: &str = "＋ 把当前的设备层与口味层存成一份预设";

/// 预设那一栏：盘上有的那几份摆成一列，末尾一行是「存成一份新的」。
///
/// **抬头把「不装范围层」说出来**（`crate::preset` 的抬头写着为什么）：
/// 套用一份预设不会动输出根与卷清单，而那正是用户按下这个键之前最该放心的一件事。
/// 与左栏范围层那一块的抬头（`Layer::Scope`）说的是同一件事的两半。
///
/// 光标那一行反白，与左栏同一副样子（见 [`super::config::config`]）：
/// 反白说的是「就在这一行上动手」。
///
/// **头一行说这一栏是哪一份文件列出来的**：存出去的东西落在用户自己的配置目录里，
/// 而下一次多半是在命令行上 `--preset` 用它。这句话摆在这一格而不是屏底那一句里，
/// 是因为**屏底那一格不折行**，一条长路径会被切掉，而这一格折得下来。
///
/// **名字多过这一格装得下的行数时，视口跟着光标走**（见 [`opens_from`]）：
/// 末尾那一行是唯一存得出去的入口，它掉出屏外就等于这一栏没有出路了。
pub(super) fn presets(picker: &Picker, area: Rect) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            format!(" {}", picker.file().display()),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
    ];
    if picker.names().is_empty() {
        lines.push(Line::from(" 这份文件里还没有预设——末尾那一行存下第一份。"));
        lines.push(Line::from(""));
    }
    let rows = picker
        .names()
        .iter()
        .map(|name| format!("  {name}"))
        .chain([format!("  {ADD_PRESET}")]);
    let listed_from = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    for (at, text) in rows.enumerate() {
        let style = if at == picker.at() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("预设 · 装设备层与口味层，范围层不进"),
        )
        .scroll((opens_from(picker, area, listed_from), 0))
        .wrap(Wrap { trim: false })
}

/// 预设那一栏从第几行画起。**只往下滚到「光标那一行还在格子里」为止，不多滚一行。**
///
/// 滚动量是**算出来的，不是记着的**：这一栏没有自己的滚动状态——光标在哪儿，
/// 视口就跟到哪儿。列表短于这一格时它恒是零（`saturating_sub`），因此常见的那几份
/// 预设摆在最上面，与没有这一段时一模一样。
///
/// `listed` 是清单在正文里从第几行开始（前面那几行是文件位置与空行）。
/// 一行一行地数够用：这一栏每一行都是一个名字，不像报告那样会折行
/// （名字长过这一格的话折下来的那一截会挤掉一行，代价是滚多了一行，不是滚丢了光标）。
fn opens_from(picker: &Picker, area: Rect, listed: u16) -> u16 {
    let inside = area.height.saturating_sub(2);
    let cursor = listed.saturating_add(u16::try_from(picker.at()).unwrap_or(u16::MAX));
    cursor.saturating_add(1).saturating_sub(inside)
}

#[cfg(test)]
mod tests {
    use super::super::probe::{
        reversed_cells, reversed_rows, same_screen, screen, snapshot, tight,
    };
    use super::*;
    use crate::session::state::{Key, Layer, Session};

    /// 一个开着预设那一栏的会话。文件位置照真会话那一份的样子（`press` 从盘上读来）。
    fn picking(names: &[&str]) -> Session {
        let mut session = Session::new();
        session.pick(
            names.iter().map(|name| (*name).to_owned()).collect(),
            std::path::PathBuf::from("C:/配置/tonefit/presets.toml"),
        );
        session
    }

    /// 把预设那一栏单独画进一格里。
    fn preset_snapshot(session: &Session, width: u16, height: u16) -> String {
        let picker = session.picking().expect("那一栏开着");
        snapshot(
            |frame| frame.render_widget(presets(picker, frame.area()), frame.area()),
            width,
            height,
        )
    }

    /// **快照：预设那一栏。** 只钉这一格，与 [`main_snapshot`] 同一条理由——
    /// 左栏此刻在场（见下一条用例），把它一起钉进来，改一行配置标签就要重录这一段。
    #[test]
    fn the_preset_column_lists_what_is_on_disk_and_a_row_to_store_into() {
        same_screen(
            &preset_snapshot(&picking(&["漫画", "画集"]), 52, 9),
            THE_PRESET_COLUMN,
        );

        // 一份都还没有时那一栏自己说得出来，末尾那一行仍在——它是唯一的出路。
        let nothing = preset_snapshot(&picking(&[]), 52, 8);
        assert!(nothing.contains("还没有预设"), "{nothing}");
        assert!(nothing.contains(ADD_PRESET), "{nothing}");

        // **名字多过这一格装得下的行数时，视口跟着光标走。** 光标绕到末尾那一行上
        // （唯一存得出去的入口）时它仍在屏上——掉出去就等于这一栏没有出路了。
        let mut many = picking(&["一", "二", "三", "四", "五", "六", "七", "八"]);
        let top = preset_snapshot(&many, 52, 8);
        assert!(top.contains("  一"), "开头几份该在屏上：{top}");
        many.press(Key::Up);
        let bottom = preset_snapshot(&many, 52, 8);
        assert!(bottom.contains(ADD_PRESET), "末尾那一行掉出去了：{bottom}");
        assert!(!bottom.contains("  一"), "视口没跟着光标走：{bottom}");
    }

    /// 见 [`the_preset_column_lists_what_is_on_disk_and_a_row_to_store_into`]。
    const THE_PRESET_COLUMN: &str = r#"
"┌预设 · 装设备层与口味层，范围层不进───────────────┐"
"│ C:/配置/tonefit/presets.toml                     │"
"│                                                  │"
"│  漫画                                            │"
"│  画集                                            │"
"│  ＋ 把当前的设备层与口味层存成一份预设           │"
"│                                                  │"
"│                                                  │"
"└──────────────────────────────────────────────────┘"
"#;

    /// **预设那一栏开着时左栏照旧在场**：存出去的就是它上面那两层，
    /// 而「存的是什么」在按下去之前得看得见（与展开那一副正相反——那一副要的是宽度）。
    ///
    /// 顺带钉住光标与屏底那一行：停在一份预设上时那一行说的是「套用哪一个」，
    /// 停在末尾那一行上说的是「打个名字」，打起字来摆的是缓冲——三副样子各说各的。
    #[test]
    fn the_preset_column_keeps_the_config_in_sight_and_says_which_key_does_what() {
        let mut session = picking(&["漫画", "画集"]);

        let listing = tight(&screen(&mut session, None, 120, 40));
        assert!(listing.contains(&tight(Layer::Device.title())), "左栏没了");
        assert!(
            listing.contains("漫画") && listing.contains("画集"),
            "{listing}"
        );
        assert!(listing.contains(&tight("⏎ 套用「漫画」")), "{listing}");
        assert!(listing.contains(&tight("d 删掉")), "{listing}");
        // 套用把两层整个换掉，而那一下不可撤销——按下去之前就说在屏上。
        assert!(
            listing.contains(&tight("眼下配好的两层随之丢掉")),
            "{listing}"
        );
        // 反白的是**这一栏**的光标，不是左栏那一行的：按键这时全归这一栏，
        // 而反白说的是「就在这一行上动手」（见 [`super::super::config::config`]）。
        assert_eq!(
            reversed_rows(&mut session),
            0,
            "预设那一栏开着，左栏那一行却还反白着"
        );
        let picker = session.picking().expect("那一栏开着");
        assert!(
            reversed_cells(
                |frame| frame.render_widget(presets(picker, frame.area()), frame.area()),
                52,
                9
            ) > 0,
            "这一栏的光标那一行没反白"
        );

        // 挪到末尾那一行上：屏底改口说「打个名字」，而删那个键不摆了——
        // 那一行不是一份预设，它在那儿按不动（屏上不摆按不动的键）。
        session.press(Key::Up);
        let last = tight(&screen(&mut session, None, 120, 40));
        assert!(last.contains(&tight("⏎ 打个名字存下来")), "{last}");
        assert!(!last.contains(&tight("d 删掉")), "{last}");

        // 打起字来：缓冲在屏底，而**范围层不进预设**这句话就摆在它下面一行。
        session.press(Key::Enter);
        session.press(Key::Char('新'));
        let naming = tight(&screen(&mut session, None, 120, 40));
        assert!(naming.contains(&tight("预设名 新")), "{naming}");
        assert!(naming.contains(&tight("范围层")), "{naming}");
    }
}
