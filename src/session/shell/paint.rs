//! 屏上一格**要什么样子在终端里是什么样**：颜色的要法（[`Look`]）译成具名色与修饰，
//! **本仓库唯一写得出颜色名的地方**（`CONTEXT.md` 的《语义色》）。
//!
//! 四种语义那个类型不住在这里：它在 [`crate::session::tone`]，种类色与修饰的要法在
//! [`crate::session::look`]，两者都在 `tui` 特性**前面**——状态机说出一句话的那一刻就定了
//! 它有多重。本模块管的是**样子**：各是什么颜色、上不上色（[`colourful`]，`NO_COLOR`）。
//!
//! 画法各处按要法要色（[`look`]），一处都不自己挑颜色：屏上那几块因此一个 `Color::` 都不写。

use ratatui::style::{Color, Modifier, Style};

use crate::session::look::{Hue, Kind, Look};
use crate::session::tone::Tone;
use tonefit::{BitDepth, Pass};

/// 一格什么样（`session-redesign/06`；spec《颜色》）：颜色的要法（[`Hue`]）译成
/// 16 个具名色里的一个，四样修饰照搬。**种类色在屏上各是哪一色，只在这一处**：
///
/// | 要的是 | 屏上 |
/// |---|---|
/// | 默认 | 终端默认色 |
/// | 次要 | 暗灰（ANSI 8）——框线、标签 |
/// | 说明正文 | 灰（ANSI 7）——详情栏与说明卡里那几段解释文字 |
/// | 语义色 | 平常默认色 · 注意黄 · 出事红 · 不要紧暗灰**并压暗** |
/// | 预览 · 清点中 | 品红 |
/// | 转换 | 青 |
/// | 卷名 | 灰（ANSI 7） |
/// | 灰阶档位 | 1bit 品红 · 2bit 青 · 4bit 蓝 |
/// | 环节 | 查重品红 · 分析蓝 · 写出青 |
/// | 完成 · 处理中 | 绿 · 蓝 |
/// | 聚焦框与光标 | 亮绿 |
/// | 顶栏右端那一块 | 蓝 |
/// | 抬头（全部按键的组名、输入行的提示词） | 绿 |
/// | 键的写法（全部按键那一张上） | 黄 |
/// | 补全框里的文件夹 | 蓝 |
/// | 一组的组名（设置栏） | 品红 |
/// | 这一趟的进度（顶栏右端） | 青 |
///
/// **`NO_COLOR` 在场时颜色一律退回终端默认色，修饰不退**（`CONTEXT.md` 的《语义色》）：
/// 加粗、下划线、斜体、压暗都不靠颜色说话，抹掉了屏上没有一个字补得回来；「不要紧」那一档
/// 的压暗也因此照旧——它退掉的只是那一灰。
///
/// **不设背景色**：一处都不定，深色浅色两边都得活（Q737 那条断言在 `super::design`）。
pub(in crate::session) fn look(look: Look) -> Style {
    let mut style = Style::default();
    if colourful()
        && let Some(colour) = colour_of(look.hue)
    {
        style = style.fg(colour);
    }
    if look.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if look.dim || look.hue == Hue::Tone(Tone::Muted) {
        style = style.add_modifier(Modifier::DIM);
    }
    if look.underlined {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if look.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

/// 颜色的要法 → 具名色；终端默认色是 `None`。[`look`] 那张表就是这个 `match`。
fn colour_of(hue: Hue) -> Option<Color> {
    match hue {
        Hue::Plain | Hue::Tone(Tone::Plain) => None,
        Hue::Faint | Hue::Tone(Tone::Muted) => Some(Color::DarkGray),
        Hue::Prose => Some(Color::Gray),
        Hue::Tone(Tone::Caution) => Some(Color::Yellow),
        Hue::Tone(Tone::Trouble) => Some(Color::Red),
        Hue::Kind(Kind::Preview | Kind::Surveying) => Some(Color::Magenta),
        Hue::Kind(Kind::Convert) => Some(Color::Cyan),
        Hue::Kind(Kind::Depth(BitDepth::One)) => Some(Color::Magenta),
        Hue::Kind(Kind::Depth(BitDepth::Two)) => Some(Color::Cyan),
        Hue::Kind(Kind::Depth(BitDepth::Four)) => Some(Color::Blue),
        Hue::Kind(Kind::Depth(BitDepth::Eight)) => None,
        Hue::Kind(Kind::Pass(Pass::Fingerprint)) => Some(Color::Magenta),
        Hue::Kind(Kind::Pass(Pass::First)) => Some(Color::Blue),
        Hue::Kind(Kind::Pass(Pass::Second)) => Some(Color::Cyan),
        // 环节那个枚举是 `non_exhaustive`：库添第四遍时这一色再定。
        Hue::Kind(Kind::Pass(_)) => None,
        Hue::Kind(Kind::Done | Kind::Caption) => Some(Color::Green),
        Hue::Kind(Kind::Working | Kind::Banner | Kind::Directory) => Some(Color::Blue),
        Hue::Kind(Kind::Volume | Kind::Prose) => Some(Color::Gray),
        Hue::Kind(Kind::Focus) => Some(Color::LightGreen),
        Hue::Kind(Kind::Key) => Some(Color::Yellow),
        Hue::Kind(Kind::Band) => Some(Color::Magenta),
        Hue::Kind(Kind::Progress) => Some(Color::Cyan),
    }
}

/// **上不上色**。`NO_COLOR` 在场即不上色，而这里是它**唯一**生效的地方
/// （`CONTEXT.md` 的《会话》：语义色）——十个地方各判一次的话，漏掉一处就是
/// 「说好了不上色却还有一行是红的」。
///
/// 认的是**在不在场**，不是它的值：`NO_COLOR=` 与 `NO_COLOR=0` 一样算数
/// （<https://no-color.org> 那一条约定）。
///
/// 读一次记住：一趟会话里环境变量不会变，而这个判断每一帧要问几十次。
fn colourful() -> bool {
    #[cfg(test)]
    if let Some(forced) = forced() {
        return forced;
    }
    static COLOURFUL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *COLOURFUL.get_or_init(|| std::env::var_os("NO_COLOR").is_none())
}

#[cfg(test)]
use forcing::forced;
// 用例要按住「不上色」（`super::super::shell` 那条 `NO_COLOR` 用例）。
#[cfg(test)]
pub(in crate::session) use forcing::forcing;

#[cfg(test)]
mod forcing {
    use std::cell::Cell;

    thread_local! {
        /// 用例里按住的那个答案。**每个用例各跑在自己的线程上**（Rust 自带的那套 harness
        /// 就是这么跑的），因此按住它不会漏到别的用例上——而进程级的那一个会。
        ///
        /// 它同时挡掉另一件事：**开发机上真设着 `NO_COLOR` 的人**跑这几条用例照样绿。
        static FORCED: Cell<Option<bool>> = const { Cell::new(None) };
    }

    /// 用例这一刻按住的答案；没按就是 `None`，照环境变量走。
    pub(super) fn forced() -> Option<bool> {
        FORCED.with(Cell::get)
    }

    /// 在 `body` 这一段里按住「上色」或「不上色」，跑完松开。
    ///
    /// 两副样子要在同一条用例里比（票面第五条：两张快照文字逐格相同），
    /// 而环境变量在一个进程里只有一份。
    pub(in crate::session) fn forcing<T>(colourful: bool, body: impl FnOnce() -> T) -> T {
        FORCED.with(|held| held.set(Some(colourful)));
        let out = body();
        FORCED.with(|held| held.set(None));
        out
    }
}
