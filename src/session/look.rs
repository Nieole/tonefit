//! 屏上一格**要什么样子**：颜色的要法（语义色、种类色、次要那一灰、默认色）加修饰
//! （`CONTEXT.md` 的《会话》：语义色——四种语义之外的**种类色**；ADR 0019 决定第 11 条）。
//!
//! 本模块只有几个枚举与一个小结构，**一个颜色名都不写**：种类色在屏上各是哪一个具名色、
//! `NO_COLOR` 在场时怎么退，都在画法那一层的 `super::draw::paint` 一处定
//! （本仓库唯一写得出颜色名的地方）。这里答的是**画法要的是哪一种**。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》），与 [`super::tone`] 同一侧。
//! 要它在这一侧的是新会话的状态机：屏底那一句回话由状态机说出口，而说出口的那一刻就定了
//! 每一截是什么样（`已删除 ` 是注意色、后面那条路径是默认色）——一句话带着几截各自的样子，
//! 状态机在特性前面，样子的**要法**因此也得在前面。
//!
//! # 修饰不归颜色管
//!
//! 加粗、压暗、下划线、斜体是[`Look`]上四个开关，与颜色分开：`NO_COLOR` 抹的是颜色，
//! 这四样照旧（`CONTEXT.md` 的《语义色》：它们不靠颜色说话，抹掉了屏上没有一个字补得回来）。

use tonefit::{BitDepth, Pass};

use super::tone::Tone;

/// **种类色**：说的是「这是哪一种」，不是「怎么样」（`CONTEXT.md` 的《语义色》）。
///
/// 每一种在屏上是哪一个具名色，只在 `super::draw::paint` 那张表里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "只有画法造得出它，而画法在 tui 特性后面")
)]
pub enum Kind {
    /// 灰阶档位：1bit · 2bit · 4bit 各一色（`+FS` 是这一色再压暗，见 [`Look::dim`]）。
    Depth(BitDepth),
    /// 环节：查重 · 分析 · 写出各一色。
    Pass(Pass),
    /// **完成**那一种：行首记号里的 `✓`，连同勾着的勾选框、此刻那个视图的号、
    /// 「＋ 添加路径」那一行——都是「开着的、成了的」那一种绿。
    Done,
    /// **处理中**那一种：行首那个转轮。
    Working,
    /// **预览**那一趟：只分析、不写文件。总览抬头那两个字与起一趟时屏底那一句都是它。
    Preview,
    /// **转换**那一趟：写到输出目录。与[预览](Self::Preview)分开——
    /// 「此刻在写没写」是屏上最要紧的一件事（`CONTEXT.md` 的《总览》）。
    Convert,
    /// **清点中**那一种：清点那一段转的那个转轮（总览那一行与处理路径那几行）。
    /// 与[处理中](Self::Working)分开——清点不是处理：那一段库一条事件都不报，
    /// 屏上一个卷数都给不出（`CONTEXT.md` 的《清点》）。
    Surveying,
    /// **聚焦框与光标**：焦点那一块的粗框、块里光标那一行行首的 `❯`、框底边上光标停在第几条。
    Focus,
    /// 顶栏右端那一块：程序名与版本、型号、这一趟走逐页判断还是整卷统一灰阶。
    Banner,
    /// **抬头**那一种：全部按键那一张上的组名、输入行的提示词——都是「这一段说的是什么」的那几个字。
    Caption,
    /// **键的写法**：全部按键那一张上每一行头上那个键。
    Key,
    /// **文件夹**：补全框里带 `/` 的那几条（旁边不带 `/` 的是文件，默认色）。
    Directory,
    /// **一卷**：树上卷行的名字。与目录行的名字分开（那一级是终端默认色），
    /// 一眼看得出树上这一行是哪一级。
    Volume,
    /// **一组的组名**：设置栏上设备设置与处理选项那两组的名字。
    /// 画质判定参数那一组**不上它**——它不是一组改得动的设置。
    Band,
    /// **这一趟的进度**：人在配置视图时顶栏右端那一截转轮与百分比。
    Progress,
}

/// 一格要**哪一种颜色**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hue {
    /// 终端默认色。
    #[default]
    Plain,
    /// **次要**：框线、标签、说明——设计稿里的那一灰。
    Faint,
    /// **说明正文**：详情栏、说明卡与预设栏里那几段解释文字。比默认色柔一点、比框线那一灰亮
    /// 一点——它是**读物**，与屏上那些「此刻是什么」的字分得开。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法造得出它，而画法在 tui 特性后面")
    )]
    Prose,
    /// 四种语义色之一。
    Tone(Tone),
    /// 种类色之一。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法造得出它，而画法在 tui 特性后面")
    )]
    Kind(Kind),
}

/// 一格的样子：颜色的要法加四样修饰。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Look {
    pub hue: Hue,
    pub bold: bool,
    pub dim: bool,
    pub underlined: bool,
    pub italic: bool,
}

impl Look {
    /// 终端默认色、一样修饰都没有。
    pub const PLAIN: Self = Self::of(Hue::Plain);

    /// 次要那一灰。
    pub const FAINT: Self = Self::of(Hue::Faint);

    /// 只挑颜色，一样修饰都没有。
    pub const fn of(hue: Hue) -> Self {
        Self {
            hue,
            bold: false,
            dim: false,
            underlined: false,
            italic: false,
        }
    }

    /// 语义色那一种。
    pub const fn tone(tone: Tone) -> Self {
        Self::of(Hue::Tone(tone))
    }

    /// 种类色那一种。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub const fn kind(kind: Kind) -> Self {
        Self::of(Hue::Kind(kind))
    }

    /// 加粗。
    pub const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// 压暗。
    pub const fn dim(self) -> Self {
        Self { dim: true, ..self }
    }

    /// 下划线。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub const fn underlined(self) -> Self {
        Self {
            underlined: true,
            ..self
        }
    }

    /// 斜体。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }
}

/// 一截字，连同它的样子。屏底那一句回话是几截拼成的，画法各处交给画布的也是它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub look: Look,
}

impl Segment {
    /// 一截字加一份样子。
    pub fn new(text: impl Into<String>, look: Look) -> Self {
        Self {
            text: text.into(),
            look,
        }
    }

    /// 终端默认色那一截。
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Look::PLAIN)
    }

    /// 次要那一灰的一截。
    pub fn faint(text: impl Into<String>) -> Self {
        Self::new(text, Look::FAINT)
    }

    /// 这一截占几格。
    pub fn width(&self) -> usize {
        usize::from(crate::wrap::width(&self.text))
    }
}

/// 几截加起来占几格。
pub fn width_of(segments: &[Segment]) -> usize {
    segments.iter().map(Segment::width).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 修饰是四个独立的开关，叠得起来；颜色只有一个。
    #[test]
    fn modifiers_stack_and_the_hue_stays() {
        let look = Look::tone(Tone::Caution).bold().dim();
        assert_eq!(look.hue, Hue::Tone(Tone::Caution));
        assert!(look.bold && look.dim);
        assert!(!look.underlined && !look.italic);
        assert_eq!(Look::PLAIN, Look::default());
    }

    /// 几截加起来的宽度按显示宽度算：汉字两格。
    #[test]
    fn segments_add_up_by_display_width() {
        let segments = [Segment::plain("ab"), Segment::faint("设备")];
        assert_eq!(width_of(&segments), 6);
    }
}
