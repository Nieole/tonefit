//! 屏上那一块：**预设栏 (Picker)**——列出盘上那几份预设、并把当前两层存成一份的那一栏
//! （`CONTEXT.md` 的《会话》：预设栏）。
//!
//! 它**占的是主区，左栏照旧在场**（见 [`super::shell`]）：存出去的就是左栏上那两层，
//! 而「存的是什么」在按下去之前得看得见。这一栏此刻按得动哪几个键写在屏底，
//! 归 [`super::footer`]。
//!
//! 名字多过这一格装得下的行数时视口跟着光标走（[`Viewport`]）——
//! 这一栏自己没有滚动状态。**折行走 [`crate::wrap`]**，与屏上其余各格同一套
//! （见 [`super::folded`]）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::Styled;
use crate::session::state::Picker;
use crate::session::viewport::Viewport;

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
/// **名字多过这一格装得下的行数时，视口跟着光标走**（见 [`Viewport`]）：
/// 末尾那一行是唯一存得出去的入口，它掉出屏外就等于这一栏没有出路了。
/// 滚动条与正文一起画（[`super::scrolling`]），一份都不多时不画。
pub(super) fn presets(frame: &mut Frame, area: Rect, picker: &Picker) {
    // 一行字加这一行的样式（[`Styled`]）：折行只认字，样式折完由 [`super::folded`] 逐行重挂。
    // **空行摆的是一个空格而不是空串**：空文字折出零行（[`crate::wrap::fold`]），
    // 而这里要的正是一行——屏上早有这个写法（`super::overlay` 里那几组之间空的那一行）。
    let mut rows: Vec<Styled> = vec![
        Styled::new(
            format!(" {}", picker.file().display()),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Styled::plain(" ".to_owned()),
    ];
    if picker.names().is_empty() {
        rows.push(Styled::plain(
            " 这份文件里还没有预设——末尾那一行存下第一份。".to_owned(),
        ));
        rows.push(Styled::plain(" ".to_owned()));
    }
    let listed = picker
        .names()
        .iter()
        .map(|name| format!("  {name}"))
        .chain([format!("  {ADD_PRESET}")]);
    // 清单在**折行之前**的正文里从第几行开始：前面那几行是文件位置与空行。
    // 折完落在第几行由 [`super::folded`] 换算——头一行那条路径与末尾那一行都折得起来，
    // 一行一行地数会把视口数少（内容已截、滚动条却不画），停车场 Q136 咬的正是这个。
    let listed_from = rows.len();
    for (at, text) in listed.enumerate() {
        let style = if at == picker.at() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        rows.push(Styled::new(text, style));
    }
    // **折到多宽从骨架来**：这一格是主区那一格（[`super::yielding::panes`]），
    // 两条框线各吃一格——这一栏自己不猜第二个数。
    let (lines, cursor) = super::folded(
        rows,
        listed_from + picker.at(),
        area.width.saturating_sub(2),
    );
    let view = Viewport::new(
        lines.len(),
        usize::from(area.height.saturating_sub(2)),
        cursor,
    );
    let body = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(
        super::yielding::title("预设 · 装设备层与口味层，范围层不进", area.width),
    ));
    super::scrolling(frame, area, body, &view);
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
        snapshot(|frame| presets(frame, frame.area(), picker), width, height)
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
        // **没有可滚的东西时不画滚动条**：上面那张快照因此逐格照旧。
        assert!(!nothing.contains('▲'), "装得下还画了滚动条：{nothing}");

        // **名字多过这一格装得下的行数时，视口跟着光标走。** 光标绕到末尾那一行上
        // （唯一存得出去的入口）时它仍在屏上——掉出去就等于这一栏没有出路了。
        let mut many = picking(&["一", "二", "三", "四", "五", "六", "七", "八"]);
        let top = preset_snapshot(&many, 52, 8);
        assert!(top.contains("  一"), "开头几份该在屏上：{top}");
        many.press(Key::Up);
        let bottom = preset_snapshot(&many, 52, 8);
        assert!(bottom.contains(ADD_PRESET), "末尾那一行掉出去了：{bottom}");
        assert!(!bottom.contains("  一"), "视口没跟着光标走：{bottom}");
        // **装不下时右边那条框线上画着滚动条**（本票）：`▲`／`▼` 两头说的是
        // 「上面还有、下面还有」——从前这一栏滚归滚，屏上一个记号都没有。
        for edge in ['▲', '▼'] {
            assert!(top.contains(edge), "上下两头的记号没画出来：{top}");
            assert!(bottom.contains(edge), "上下两头的记号没画出来：{bottom}");
        }

        // **窄下来时这一栏数的也是折出来的行**（`p4-parking-lot/02` 收的 Q104／Q136）：
        // 24 列上抬头那条路径与末尾那一行各折成两行，五个逻辑行因此占七行。
        // 按逻辑行数的话六行的正文「装得下」，滚动条一条都不画——而内容其实已经被折掉。
        let folding = preset_snapshot(&picking(&["漫画", "画集"]), 24, 8);
        for edge in ['▲', '▼'] {
            assert!(
                folding.contains(edge),
                "内容已经被折掉，滚动条却不画：{folding}"
            );
        }
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
            reversed_cells(|frame| presets(frame, frame.area(), picker), 52, 9) > 0,
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
