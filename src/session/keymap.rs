//! **一张按键表**：每个键在（视图、阶段、焦点）上派什么、屏上怎么写、那件事怎么说
//! （spec《按键表、屏底与覆盖层》；`CONTEXT.md` 的《会话》：屏底、覆盖层、焦点）。
//!
//! **屏底与全部按键都从它派生**：屏底那一行摆的每一件、`?` 那一张列的每一行，都是这张表上的
//! 一行——加一个键不必改两处（spec 的 story 91）。设计稿的按键表与屏底那几组是手写的
//! （`design.html` 的 `KEYMAP` 与 `footerHints`），实现照它逐字、照它的轻重次序，
//! **结构上仍从这张表挑**：屏底那一层说的是「此刻摆哪几件」（[`super::view`]），
//! 每一件的键怎么写、那句话怎么说都从这里取。
//!
//! # 一行是什么
//!
//! 表上一行是**一个键在一处派一件事**：键（[`Chord`]）、它派的那件事（[`Deed`]）、
//! 键的写法、屏底那一句（短）、全部按键那一句（长）、派得出它的阶段与屏上哪几块。
//! 设计稿把 `j k` 写在一行上，这里是两行——屏底上它们并成 `j/k`，全部按键上并成 `j k`，
//! 并的依据是**屏上那句话相同**，两处各并各的（与旧界面 `super::draw::keys::merged` 同一条理由）。
//!
//! 同一个键在不同阶段做的事不同，就是几行各写各的（`q` 在还没开始与已结束时退出，
//! 跑着时不退，停车场 Q717）；同一件事在不同的块上说法不同也是几行（`j` 在卷列表上是「选择」，
//! 在每页结果上是「选页」）。**短的那一句为空的行只管派键、不上屏底**；
//! **长的那一句为空的行不上全部按键那一张**（覆盖层上 `Esc` 关掉它，设计稿的表上没有这一行）。
//!
//! # 阶段五档
//!
//! 表上的阶段是[`Phase`]：还没开始 · 清点中 · 转换中 · 等待确认 · 已结束。它比会话的
//! [`Stage`] 多分出「清点中」——那**不是阶段的新取值**（spec《库：开工那一条事件带上清点的产出》，
//! 停车场 Q720），是跑着的那一段里开工那一条还没到的那一截；按键表要分它，
//! 因为清点完之前树还没有，`l`／`/` 那几个键派不出去。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）。

use tonefit::Instruction;

use super::live::Live;
use super::state::{Key, Stage};
use super::view::Focus;

/// 按键表上的阶段：会话的[`Stage`]加上「清点中」那一截（模块文档《阶段五档》）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 还没开始。
    Fresh,
    /// 清点中：线程起了，开工那一条还没到。
    Surveying,
    /// 转换中（清点完了、还在跑）。
    Running,
    /// 等待确认。
    Deciding,
    /// 已结束。
    Ended,
}

impl Phase {
    /// 此刻是哪一档：阶段那一维加上那一趟清点完了没有。没有那一趟的跑着阶段算清点中——
    /// 线程起了、开工那一条还没到，屏上正是这一截。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法与那条循环读得到，而它们在 tui 特性后面")
    )]
    pub fn of(stage: Stage, live: Option<&Live>) -> Self {
        match stage {
            Stage::Fresh => Self::Fresh,
            Stage::Running(_) => match live {
                Some(live) if !live.surveying() => Self::Running,
                _ => Self::Surveying,
            },
            Stage::Deciding(_) => Self::Deciding,
            Stage::Ended => Self::Ended,
        }
    }

    /// 按了停止没有——跑着时 `s` 那一句「停止」与「再按一次立即停」分在这里。
    pub fn latched(stage: Stage) -> bool {
        matches!(
            stage,
            Stage::Running(Instruction::Finish) | Stage::Deciding(Instruction::Finish)
        )
    }
}

/// 一个键，会话认得的写法：一个键、Ctrl 加一个字母、连击两个字符、滚轮、单击。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    Key(Key),
    /// `C-d`／`C-u`／`C-f`／`C-b`／`C-w`……`C-c` 不在这里，它是 [`Key::Interrupt`]。
    Ctrl(char),
    /// 连击键：`gg`、`gt`、`gT`、`dd`、`]d`、`[d`。前半截按下去屏底右端留待续记号。
    Combo(char, char),
    /// 滚轮一格。
    Wheel,
    /// 单击一行。
    Click,
}

/// 一个键派的**那件事**。屏底与全部按键按它查表，状态机按它做事。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deed {
    // 全局
    TaskView,
    ConfigView,
    NextView,
    PrevView,
    NextBlock,
    Help,
    Quit,
    /// 跑着与等待确认时的 `q`：不退，屏底说先按 `s` 停止或按 `C-c`。
    QuitRefused,
    Interrupt,
    // 移动
    Down,
    Up,
    HalfDown,
    HalfUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
    Wheel,
    Click,
    // 卷列表
    Open,
    Close,
    Follow,
    Search,
    SearchNext,
    SearchPrev,
    /// 卷列表上的 `Esc`：**只丢掉搜索那一句**（`CONTEXT.md` 的《退出会话》：`Esc` 只退一级）。
    /// **不上全部按键那一张**——设计稿的表上没有这一行；`Esc` 那一句在覆盖层与输入行上说。
    ClearSearch,
    NextProblem,
    PrevProblem,
    // 路径
    AddPath,
    EditPath,
    TogglePath,
    DeletePath,
    BackToPaths,
    // 转换
    Preview,
    Convert,
    Stop,
    // 确认
    Write,
    WriteAll,
    End,
    ViewPages,
    // 每页结果
    ListAll,
    BackToList,
    // 配置
    ConfigOpen,
    /// 设置栏上的 `⏎`：与 `l` 同，**自由填的那几项另外直接开输入行**（设计稿 `configKey`）。
    /// 它与 [`ConfigOpen`](Self::ConfigOpen) 长的那一句相同，全部按键那一张因此并成一行。
    ConfigEnter,
    ConfigBack,
    /// 自由填的那几项上的 `i`：经输入行改这一项的值。**不上全部按键那一张**——
    /// 设计稿的表上没有这一行；它只在详情栏里顺口提一次（`[i → 修改]`）。
    EditValue,
    Presets,
    Chart,
    UsePreset,
    DeletePreset,
    // 输入
    Complete,
    DeleteWord,
    /// 退一个字（`⌫`）。
    Erase,
    Confirm,
    Cancel,
    HelpWhileTyping,
    /// **打字**：输入行上每一个字符都是一个字（`CONTEXT.md` 的《覆盖层》「那儿每一个字符都是一个字」）。
    /// 表上没有它那一行——表上派的是键，字不是键：焦点在输入行上、表上派不出的字符落到它
    /// （[`super::state::Session::deed_of`]）。
    Typed(char),
    // 覆盖层
    CloseOverlay,
}

/// 全部按键那一张按用途分的组，次序就是屏上的次序（设计稿的 `KEYMAP`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Global,
    Motion,
    VolumeList,
    Paths,
    Run,
    Confirm,
    Pages,
    Config,
    Input,
}

impl Group {
    /// 全部按键那一张上的次序（设计稿的 `KEYMAP`）。
    pub const ALL: [Self; 9] = [
        Self::Global,
        Self::Motion,
        Self::VolumeList,
        Self::Paths,
        Self::Run,
        Self::Confirm,
        Self::Pages,
        Self::Config,
        Self::Input,
    ];

    /// 这一组在屏上叫什么。
    pub fn title(self) -> &'static str {
        match self {
            Self::Global => "全局",
            Self::Motion => "移动",
            Self::VolumeList => "卷列表",
            Self::Paths => "路径",
            Self::Run => "转换",
            Self::Confirm => "确认",
            Self::Pages => "每页结果",
            Self::Config => "配置",
            Self::Input => "输入",
        }
    }
}

/// 表上的一行（模块文档《一行是什么》）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub group: Group,
    pub deed: Deed,
    pub chord: Chord,
    /// 键的写法：`j`、`空格`、`dd`、`C-c`、`⏎`。
    pub spelt: &'static str,
    /// 屏底那一句。空的不上屏底。
    pub short: &'static str,
    /// 全部按键那一句。空的不上那一张。
    pub long: &'static str,
    /// 派得出它的阶段；空的是任何阶段。
    pub phases: &'static [Phase],
    /// 派得出它的块；空的是任何一块。
    pub blocks: &'static [Focus],
}

impl Row {
    /// 这一行在这一档、这一块上派不派得出。
    pub fn applies(&self, phase: Phase, focus: Focus) -> bool {
        (self.phases.is_empty() || self.phases.contains(&phase))
            && (self.blocks.is_empty() || self.blocks.contains(&focus))
    }
}

const ANY_PHASE: &[Phase] = &[];
const FRESH: &[Phase] = &[Phase::Fresh];
const ENDED: &[Phase] = &[Phase::Ended];
/// 还没开始与已结束：一趟没在跑的那两档。
const NOT_RUNNING: &[Phase] = &[Phase::Fresh, Phase::Ended];
/// 清点中与转换中：停止按得动的那两档。
const STOPPABLE: &[Phase] = &[Phase::Surveying, Phase::Running];
/// 转换中与等待确认：自动滚动有东西可跟的那两档。
const IN_PROGRESS: &[Phase] = &[Phase::Running, Phase::Deciding];
/// 转换中与已结束：每页结果的列法切得过去的那两档（等待确认时 `a` 让给答话）。
const NOT_DECIDING_IN_A_RUN: &[Phase] = &[Phase::Running, Phase::Ended];
const DECIDING: &[Phase] = &[Phase::Deciding];
/// 一趟在跑的那三档：`q` 不退。
const IN_A_RUN: &[Phase] = &[Phase::Surveying, Phase::Running, Phase::Deciding];
/// 清点完之后的那三档：树在场。
const AFTER_SURVEY: &[Phase] = &[Phase::Running, Phase::Deciding, Phase::Ended];
/// 清点中之外的四档：输入行开得了的那几档。
const NOT_SURVEYING: &[Phase] = &[Phase::Fresh, Phase::Running, Phase::Deciding, Phase::Ended];

const ANY_BLOCK: &[Focus] = &[];
/// 没被输入行或覆盖层盖着的那几块。
const UNCOVERED: &[Focus] = &[
    Focus::VolumeList,
    Focus::Pages,
    Focus::Settings,
    Focus::Details,
    Focus::Picker,
];
/// 没被盖着的几块加上覆盖层：按停止与答话在覆盖层掀着时也按得动（ADR 0017 决定第 4 条）。
const UNCOVERED_OR_OVERLAY: &[Focus] = &[
    Focus::VolumeList,
    Focus::Pages,
    Focus::Settings,
    Focus::Details,
    Focus::Picker,
    Focus::Overlay,
];
const LIST: &[Focus] = &[Focus::VolumeList];
const PAGES: &[Focus] = &[Focus::Pages];
const CONFIG: &[Focus] = &[Focus::Settings, Focus::Details, Focus::Picker];
/// 配置视图的两栏：预设栏掀着时那一栏替换详情栏，键归它自己那几行。
const PANES: &[Focus] = &[Focus::Settings, Focus::Details];
/// 只在设置栏上：`l` 在这一栏上说「展开」，`⏎` 在这一栏上另有一支（[`Deed::ConfigEnter`]）。
const SETTINGS: &[Focus] = &[Focus::Settings];
/// 只在详情栏上：`⏎` 在这一栏上说「确定」。
const DETAILS: &[Focus] = &[Focus::Details];
const PICKER: &[Focus] = &[Focus::Picker];
const INPUT: &[Focus] = &[Focus::Input];
const OVERLAY: &[Focus] = &[Focus::Overlay];
/// 上下挪一行的键在这几块上说的都是「选择」。
const SELECTING: &[Focus] = &[
    Focus::VolumeList,
    Focus::Settings,
    Focus::Details,
    Focus::Picker,
];

// 表上一行的八格一个都省不掉：少一格就是少一维。
#[allow(
    clippy::too_many_arguments,
    reason = "一行就是这八格，写成结构字面量反而让表读不下去"
)]
const fn row(
    group: Group,
    deed: Deed,
    chord: Chord,
    spelt: &'static str,
    short: &'static str,
    long: &'static str,
    phases: &'static [Phase],
    blocks: &'static [Focus],
) -> Row {
    Row {
        group,
        deed,
        chord,
        spelt,
        short,
        long,
        phases,
        blocks,
    }
}

const fn key(letter: char) -> Chord {
    Chord::Key(Key::Char(letter))
}

/// **那张表。** 次序照设计稿的 `KEYMAP`：全部按键那一张按它列，一组之内按它排。
pub const TABLE: &[Row] = &[
    // ───── 全局 ─────
    row(
        Group::Global,
        Deed::TaskView,
        key('1'),
        "1",
        "任务",
        "切换视图：任务 / 配置",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::ConfigView,
        key('2'),
        "2",
        "配置",
        "切换视图：任务 / 配置",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::NextView,
        Chord::Combo('g', 't'),
        "gt",
        "",
        "下一个 / 上一个视图",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::PrevView,
        Chord::Combo('g', 'T'),
        "gT",
        "",
        "下一个 / 上一个视图",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::NextBlock,
        Chord::Key(Key::Tab),
        "Tab",
        "",
        "同一视图里切换区块",
        ANY_PHASE,
        CONFIG,
    ),
    row(
        Group::Global,
        Deed::Help,
        key('?'),
        "?",
        "全部按键",
        "全部按键，再按一次关闭",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::CloseOverlay,
        key('?'),
        "?",
        "",
        "",
        ANY_PHASE,
        OVERLAY,
    ),
    row(
        Group::Global,
        Deed::Quit,
        key('q'),
        "q",
        "退出",
        "退出",
        NOT_RUNNING,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::QuitRefused,
        key('q'),
        "q",
        "",
        "不退出：先按 s 停止，或按 C-c",
        IN_A_RUN,
        UNCOVERED,
    ),
    row(
        Group::Global,
        Deed::CloseOverlay,
        key('q'),
        "q",
        "",
        "",
        ANY_PHASE,
        OVERLAY,
    ),
    // 短的那一句只有窗口太小那一屏要（跑着与等待确认时 `q` 不退，那一屏摆它，停车场 Q777）；
    // 屏底那一行从不要它。
    row(
        Group::Global,
        Deed::Interrupt,
        Chord::Key(Key::Interrupt),
        "C-c",
        "退出",
        "退出（当前卷不保存，不留半成品）",
        ANY_PHASE,
        ANY_BLOCK,
    ),
    // ───── 移动 ─────
    row(
        Group::Motion,
        Deed::Down,
        key('j'),
        "j",
        "选择",
        "上下一行",
        ANY_PHASE,
        SELECTING,
    ),
    row(
        Group::Motion,
        Deed::Up,
        key('k'),
        "k",
        "选择",
        "上下一行",
        ANY_PHASE,
        SELECTING,
    ),
    row(
        Group::Motion,
        Deed::Down,
        key('j'),
        "j",
        "选页",
        "上下一行",
        ANY_PHASE,
        PAGES,
    ),
    row(
        Group::Motion,
        Deed::Up,
        key('k'),
        "k",
        "选页",
        "上下一行",
        ANY_PHASE,
        PAGES,
    ),
    row(
        Group::Motion,
        Deed::Down,
        key('j'),
        "j",
        "滚动",
        "上下一行",
        ANY_PHASE,
        OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::Up,
        key('k'),
        "k",
        "滚动",
        "上下一行",
        ANY_PHASE,
        OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::Down,
        Chord::Key(Key::Down),
        "↓",
        "",
        "",
        ANY_PHASE,
        ANY_BLOCK,
    ),
    row(
        Group::Motion,
        Deed::Up,
        Chord::Key(Key::Up),
        "↑",
        "",
        "",
        ANY_PHASE,
        ANY_BLOCK,
    ),
    row(
        Group::Motion,
        Deed::HalfDown,
        Chord::Ctrl('d'),
        "C-d",
        "",
        "半屏",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::HalfUp,
        Chord::Ctrl('u'),
        "C-u",
        "",
        "半屏",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::PageDown,
        Chord::Ctrl('f'),
        "C-f",
        "",
        "一屏",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::PageUp,
        Chord::Ctrl('b'),
        "C-b",
        "",
        "一屏",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::Top,
        Chord::Combo('g', 'g'),
        "gg",
        "",
        "到顶 / 到底",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::Bottom,
        key('G'),
        "G",
        "",
        "到顶 / 到底",
        ANY_PHASE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Motion,
        Deed::Wheel,
        Chord::Wheel,
        "滚轮",
        "",
        "一格三行",
        ANY_PHASE,
        UNCOVERED,
    ),
    row(
        Group::Motion,
        Deed::Click,
        Chord::Click,
        "单击",
        "",
        "选中那一行",
        ANY_PHASE,
        UNCOVERED,
    ),
    // ───── 卷列表 ─────
    row(
        Group::VolumeList,
        Deed::Open,
        key('l'),
        "l",
        "展开",
        "展开文件夹 / 查看每页结果",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::Open,
        key('l'),
        "l",
        "每页结果",
        "展开文件夹 / 查看每页结果",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::Open,
        Chord::Key(Key::Enter),
        "⏎",
        "查看",
        "展开文件夹 / 查看每页结果",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::Close,
        key('h'),
        "h",
        "",
        "收起 / 回上一级",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::Follow,
        key('F'),
        "F",
        "自动滚动",
        "自动滚动到正在处理的卷",
        IN_PROGRESS,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::Search,
        key('/'),
        "/",
        "搜索",
        "搜索卷名或文件夹",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::SearchNext,
        key('n'),
        "n",
        "",
        "下一个 / 上一个搜索结果",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::SearchPrev,
        key('N'),
        "N",
        "",
        "下一个 / 上一个搜索结果",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::NextProblem,
        Chord::Combo(']', 'd'),
        "]d",
        "下一个问题",
        "下一个 / 上一个问题",
        AFTER_SURVEY,
        LIST,
    ),
    // 同一个键在总览的**问题行**行尾另有一句写法（`session-redesign/08`）：那一行已经
    // 说了「问题」，跟在后面的那一句因此说「跳到下一个」。屏底摆的仍是上面那一行。
    row(
        Group::VolumeList,
        Deed::NextProblem,
        Chord::Combo(']', 'd'),
        "]d",
        "跳到下一个",
        "下一个 / 上一个问题",
        AFTER_SURVEY,
        LIST,
    ),
    row(
        Group::VolumeList,
        Deed::PrevProblem,
        Chord::Combo('[', 'd'),
        "[d",
        "",
        "下一个 / 上一个问题",
        AFTER_SURVEY,
        LIST,
    ),
    // 卷列表上的 `Esc`：只丢掉搜索那一句。两句都空——屏底不摆它（没在搜的时候它
    // 一件事都不做），全部按键那一张上也没有它（设计稿的表上没有这一行）。
    row(
        Group::VolumeList,
        Deed::ClearSearch,
        Chord::Key(Key::Esc),
        "Esc",
        "",
        "",
        AFTER_SURVEY,
        LIST,
    ),
    // ───── 路径 ─────
    row(
        Group::Paths,
        Deed::AddPath,
        key('o'),
        "o",
        "添加路径",
        "添加路径",
        FRESH,
        LIST,
    ),
    // 「＋ 添加路径」那一行上顺口提的那一句短一截（设计稿 `[o → 添加]`）。
    row(
        Group::Paths,
        Deed::AddPath,
        key('o'),
        "o",
        "添加",
        "添加路径",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::EditPath,
        key('i'),
        "i",
        "修改",
        "修改这一条",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::EditPath,
        Chord::Key(Key::Enter),
        "⏎",
        "",
        "修改这一条",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::EditPath,
        key('l'),
        "l",
        "",
        "",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::TogglePath,
        Chord::Key(Key::Space),
        "空格",
        "勾选",
        "勾选 / 取消勾选",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::DeletePath,
        Chord::Combo('d', 'd'),
        "dd",
        "删除",
        "删除这一条",
        FRESH,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::BackToPaths,
        key('o'),
        "o",
        "",
        "返回路径列表",
        ENDED,
        LIST,
    ),
    row(
        Group::Paths,
        Deed::BackToPaths,
        key('i'),
        "i",
        "",
        "返回路径列表",
        ENDED,
        LIST,
    ),
    // ───── 转换 ─────
    row(
        Group::Run,
        Deed::Preview,
        key('t'),
        "t",
        "预览",
        "预览：只分析，不写文件",
        FRESH,
        UNCOVERED,
    ),
    row(
        Group::Run,
        Deed::Preview,
        key('t'),
        "t",
        "再预览",
        "预览：只分析，不写文件",
        ENDED,
        UNCOVERED,
    ),
    row(
        Group::Run,
        Deed::Convert,
        key('x'),
        "x",
        "转换",
        "转换：写到输出目录",
        FRESH,
        UNCOVERED,
    ),
    row(
        Group::Run,
        Deed::Convert,
        key('x'),
        "x",
        "再转换",
        "转换：写到输出目录",
        ENDED,
        UNCOVERED,
    ),
    row(
        Group::Run,
        Deed::Stop,
        key('s'),
        "s",
        "停止",
        "停止：按一次做完当前卷，按两次立即停",
        STOPPABLE,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Run,
        Deed::Stop,
        key('s'),
        "s",
        "再按一次立即停",
        "停止：按一次做完当前卷，按两次立即停",
        STOPPABLE,
        UNCOVERED_OR_OVERLAY,
    ),
    // ───── 确认 ─────
    row(
        Group::Confirm,
        Deed::Write,
        key('x'),
        "x",
        "写出",
        "写出这一卷（不用重新分析）",
        DECIDING,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Confirm,
        Deed::WriteAll,
        key('a'),
        "a",
        "全部写出",
        "写出，后面的卷不再询问",
        DECIDING,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Confirm,
        Deed::End,
        key('s'),
        "s",
        "结束",
        "不写出，结束预览",
        DECIDING,
        UNCOVERED_OR_OVERLAY,
    ),
    row(
        Group::Confirm,
        Deed::ViewPages,
        key('v'),
        "v",
        "每页结果",
        "查看这一卷的每页结果",
        DECIDING,
        LIST,
    ),
    // ───── 每页结果 ─────
    row(
        Group::Pages,
        Deed::ListAll,
        key('a'),
        "a",
        "全部页",
        "需留意的页 / 全部页",
        NOT_DECIDING_IN_A_RUN,
        PAGES,
    ),
    row(
        Group::Pages,
        Deed::ListAll,
        key('a'),
        "a",
        "只看需留意的页",
        "需留意的页 / 全部页",
        NOT_DECIDING_IN_A_RUN,
        PAGES,
    ),
    row(
        Group::Pages,
        Deed::BackToList,
        key('h'),
        "h",
        "回卷列表",
        "回卷列表",
        AFTER_SURVEY,
        PAGES,
    ),
    row(
        Group::Pages,
        Deed::BackToList,
        Chord::Key(Key::Esc),
        "Esc",
        "",
        "回卷列表",
        AFTER_SURVEY,
        PAGES,
    ),
    row(
        Group::Pages,
        Deed::BackToList,
        Chord::Key(Key::Backspace),
        "⌫",
        "",
        "",
        AFTER_SURVEY,
        PAGES,
    ),
    // ───── 配置 ─────
    // 屏底那一件**随此刻在哪一栏、改不改得动而变**（设计稿 `footerHints` 配置那一支）：
    // 设置栏是 `l → 展开`，详情栏是 `⏎ → 确定`，只读那三档两边都是「查看」。
    // 短的那一句为空的两行只管派键——`l` 在详情栏上、`⏎` 在设置栏上照样按得动，
    // 只是屏底那一行不同时摆两个键。长的那一句四行相同，全部按键那一张因此并成 `l ⏎`。
    row(
        Group::Config,
        Deed::ConfigOpen,
        key('l'),
        "l",
        "展开",
        "展开选项 / 确定",
        NOT_RUNNING,
        SETTINGS,
    ),
    row(
        Group::Config,
        Deed::ConfigOpen,
        key('l'),
        "l",
        "查看",
        "展开选项 / 确定",
        IN_A_RUN,
        SETTINGS,
    ),
    row(
        Group::Config,
        Deed::ConfigEnter,
        Chord::Key(Key::Enter),
        "⏎",
        "",
        "展开选项 / 确定",
        ANY_PHASE,
        SETTINGS,
    ),
    row(
        Group::Config,
        Deed::ConfigOpen,
        key('l'),
        "l",
        "",
        "展开选项 / 确定",
        ANY_PHASE,
        DETAILS,
    ),
    row(
        Group::Config,
        Deed::ConfigOpen,
        Chord::Key(Key::Enter),
        "⏎",
        "确定",
        "展开选项 / 确定",
        NOT_RUNNING,
        DETAILS,
    ),
    row(
        Group::Config,
        Deed::ConfigOpen,
        Chord::Key(Key::Enter),
        "⏎",
        "查看",
        "展开选项 / 确定",
        IN_A_RUN,
        DETAILS,
    ),
    // 自由填的那几项上的 `i`：长的那一句为空——设计稿的全部按键上没有这一行，
    // 它只在详情栏里顺口提一次。改不动的那三档根本不派它（屏上不摆按不动的键）。
    row(
        Group::Config,
        Deed::EditValue,
        key('i'),
        "i",
        "修改",
        "",
        NOT_RUNNING,
        PANES,
    ),
    row(
        Group::Config,
        Deed::UsePreset,
        Chord::Key(Key::Enter),
        "⏎",
        "使用",
        "",
        ANY_PHASE,
        PICKER,
    ),
    row(
        Group::Config,
        Deed::UsePreset,
        key('l'),
        "l",
        "",
        "",
        ANY_PHASE,
        PICKER,
    ),
    row(
        Group::Config,
        Deed::DeletePreset,
        Chord::Combo('d', 'd'),
        "dd",
        "删除",
        "",
        ANY_PHASE,
        PICKER,
    ),
    row(
        Group::Config,
        Deed::ConfigBack,
        key('h'),
        "h",
        "返回",
        "返回，不做修改",
        ANY_PHASE,
        PANES,
    ),
    row(
        Group::Config,
        Deed::ConfigBack,
        Chord::Key(Key::Esc),
        "Esc",
        "",
        "返回，不做修改",
        ANY_PHASE,
        PANES,
    ),
    row(
        Group::Config,
        Deed::ConfigBack,
        key('h'),
        "h",
        "",
        "返回，不做修改",
        ANY_PHASE,
        PICKER,
    ),
    row(
        Group::Config,
        Deed::ConfigBack,
        Chord::Key(Key::Esc),
        "Esc",
        "",
        "返回，不做修改",
        ANY_PHASE,
        PICKER,
    ),
    row(
        Group::Config,
        Deed::Presets,
        key('p'),
        "p",
        "预设",
        "预设",
        ANY_PHASE,
        PANES,
    ),
    row(
        Group::Config,
        Deed::Presets,
        key('p'),
        "p",
        "返回",
        "预设",
        ANY_PHASE,
        PICKER,
    ),
    // 短的那一句只有顶上那一条预设摆它（`[c → 灰阶测试图]`）——屏底那一行不摆它
    // （设计稿 `footerHints` 配置那一支没有它），而屏底摆哪几件由调用方点名。
    row(
        Group::Config,
        Deed::Chart,
        key('c'),
        "c",
        "灰阶测试图",
        "生成灰阶测试图",
        ANY_PHASE,
        CONFIG,
    ),
    // ───── 输入 ─────
    row(
        Group::Input,
        Deed::Complete,
        Chord::Key(Key::Tab),
        "Tab",
        "补全",
        "补全路径",
        FRESH,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::DeleteWord,
        Chord::Ctrl('w'),
        "C-w",
        "删一段",
        "删一段",
        NOT_SURVEYING,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::Erase,
        Chord::Key(Key::Backspace),
        "⌫",
        "",
        "",
        NOT_SURVEYING,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::Confirm,
        Chord::Key(Key::Enter),
        "⏎",
        "确定",
        "确定",
        NOT_SURVEYING,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::Confirm,
        Chord::Key(Key::Enter),
        "⏎",
        "跳到结果",
        "确定",
        NOT_SURVEYING,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::Cancel,
        Chord::Key(Key::Esc),
        "Esc",
        "取消",
        "取消",
        NOT_SURVEYING,
        INPUT,
    ),
    row(
        Group::Input,
        Deed::HelpWhileTyping,
        Chord::Key(Key::F1),
        "F1",
        "",
        "全部按键",
        NOT_SURVEYING,
        INPUT,
    ),
    // ───── 覆盖层自己的两个（不上全部按键那一张）─────
    row(
        Group::Global,
        Deed::CloseOverlay,
        Chord::Key(Key::Esc),
        "Esc",
        "关闭",
        "",
        ANY_PHASE,
        OVERLAY,
    ),
];

/// 这个键在这一档、这一块上派什么。派不出就是 `None`——那个键此刻没有意义。
///
/// 同一个键在同一处有几行（`s` 的两句停止）时取头一行：几行派的是同一件事，差的只是屏底那一句。
pub fn deed(phase: Phase, focus: Focus, chord: Chord) -> Option<Deed> {
    TABLE
        .iter()
        .find(|row| row.chord == chord && row.applies(phase, focus))
        .map(|row| row.deed)
}

/// 派这件事的头一个键在屏上怎么写（`?`、`Esc`、`空格`）。表上没有这件事是 `None`。
///
/// 屏上顺口提一个键而不是摆一件事的地方要它——全部按键那一张的抬头「? Esc → 关闭」里的 `?`
/// 是掀开它的那个键（[`Deed::Help`]），再按一次关掉它；哪个键、怎么写都只从表上取。
pub fn spelt_for(deed: Deed) -> Option<&'static str> {
    TABLE
        .iter()
        .find(|row| row.deed == deed)
        .map(|row| row.spelt)
}

/// 这个字符在这一档、这一块上是不是某个连击键的前半截。是的话它先待着，屏底右端留待续记号。
pub fn starts_a_combo(phase: Phase, focus: Focus, first: char) -> bool {
    TABLE.iter().any(|row| {
        matches!(row.chord, Chord::Combo(head, _) if head == first) && row.applies(phase, focus)
    })
}

/// 屏底上的一件事：派得出它的几个键各怎么写、做什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub keys: Vec<&'static str>,
    pub what: &'static str,
}

impl Hint {
    /// 几个键在屏底上并成一个写法：`j/k`。
    pub fn spelt(&self) -> String {
        self.keys.join("/")
    }
}

/// 屏底那一行要的一件事：派它的那件事，加上短的那一句是哪一份（同一件事有几句时点名，
/// 比如 `s` 按过一次之后是「再按一次立即停」；`None` 是这件事只有一句）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Want {
    pub deed: Deed,
    pub said: Option<&'static str>,
}

impl Want {
    /// 只有一句的那件事。
    pub const fn of(deed: Deed) -> Self {
        Self { deed, said: None }
    }

    /// 几句里点名一句。
    pub const fn saying(deed: Deed, said: &'static str) -> Self {
        Self {
            deed,
            said: Some(said),
        }
    }
}

/// 屏底那一行：按轻重列的几件事各从表上取键的写法与那一句，**派不出的那件不摆**
/// （屏上不摆按不动的键）；相邻两件说的是同一句话就并成一件（`j/k → 选择`）。
///
/// `?` 全部按键恒在末尾由调用方摆（它在每一处都派得出）；摆不下从倒数第二件往前舍是画法那一层
/// 按宽度做的事（这里不知道屏有多宽）。
pub fn hints(phase: Phase, focus: Focus, wants: &[Want]) -> Vec<Hint> {
    let mut hints: Vec<Hint> = Vec::new();
    for want in wants {
        let Some(row) = TABLE.iter().find(|row| {
            row.deed == want.deed
                && !row.short.is_empty()
                && want.said.is_none_or(|said| row.short == said)
                && row.applies(phase, focus)
        }) else {
            continue;
        };
        match hints.last_mut() {
            Some(last) if last.what == row.short => last.keys.push(row.spelt),
            _ => hints.push(Hint {
                keys: vec![row.spelt],
                what: row.short,
            }),
        }
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 表上没有两行在同一档、同一块上把同一个键派给两件不同的事：那样的键屏上说不清它做什么。
    /// 同一个键几行派同一件事（差在短的那一句）不算。
    #[test]
    fn no_key_is_dealt_to_two_deeds_in_the_same_place() {
        const PHASES: [Phase; 5] = [
            Phase::Fresh,
            Phase::Surveying,
            Phase::Running,
            Phase::Deciding,
            Phase::Ended,
        ];
        const BLOCKS: [Focus; 7] = [
            Focus::VolumeList,
            Focus::Pages,
            Focus::Settings,
            Focus::Details,
            Focus::Picker,
            Focus::Input,
            Focus::Overlay,
        ];
        for phase in PHASES {
            for focus in BLOCKS {
                for (i, a) in TABLE.iter().enumerate() {
                    for b in &TABLE[i + 1..] {
                        if a.chord == b.chord && a.applies(phase, focus) && b.applies(phase, focus)
                        {
                            assert_eq!(
                                a.deed, b.deed,
                                "{phase:?} 的 {focus:?} 上「{}」派了两件事：{:?} 与 {:?}",
                                a.spelt, a.deed, b.deed
                            );
                        }
                    }
                }
            }
        }
    }

    /// 还没开始时卷列表上的键：`t` 预览、`空格` 勾选、`dd` 删除；`s` 与 `/` 派不出。
    #[test]
    fn keys_before_the_run_are_dealt_from_the_table() {
        let at = |chord| deed(Phase::Fresh, Focus::VolumeList, chord);
        assert_eq!(at(key('t')), Some(Deed::Preview));
        assert_eq!(at(key('x')), Some(Deed::Convert));
        assert_eq!(at(Chord::Key(Key::Space)), Some(Deed::TogglePath));
        assert_eq!(at(Chord::Combo('d', 'd')), Some(Deed::DeletePath));
        assert_eq!(at(key('q')), Some(Deed::Quit));
        assert_eq!(at(Chord::Key(Key::Esc)), None, "还没开始时没有东西可关");
        assert_eq!(at(key('s')), None, "还没开始时没有停止可按");
        assert_eq!(at(key('/')), None, "清点完之前 `/` 不派");
        assert!(starts_a_combo(Phase::Fresh, Focus::VolumeList, 'd'));
        assert!(starts_a_combo(Phase::Fresh, Focus::VolumeList, 'g'));
        assert!(!starts_a_combo(Phase::Fresh, Focus::VolumeList, ']'));
    }

    /// 同一个键按阶段换事：`q` 还没开始时退出、跑着时不退；`x` 跑着时不派、等待确认时是写出；
    /// `s` 跑着时停止、等待确认时结束。
    #[test]
    fn the_same_key_does_different_things_in_different_phases() {
        let list = Focus::VolumeList;
        assert_eq!(deed(Phase::Fresh, list, key('q')), Some(Deed::Quit));
        assert_eq!(deed(Phase::Ended, list, key('q')), Some(Deed::Quit));
        assert_eq!(
            deed(Phase::Running, list, key('q')),
            Some(Deed::QuitRefused)
        );
        assert_eq!(
            deed(Phase::Deciding, list, key('q')),
            Some(Deed::QuitRefused)
        );
        assert_eq!(deed(Phase::Running, list, key('x')), None);
        assert_eq!(deed(Phase::Deciding, list, key('x')), Some(Deed::Write));
        assert_eq!(deed(Phase::Running, list, key('s')), Some(Deed::Stop));
        assert_eq!(deed(Phase::Deciding, list, key('s')), Some(Deed::End));
        // 覆盖层掀着时只有停止与答话交得下去（ADR 0017 决定第 4 条），别的键不派。
        assert_eq!(
            deed(Phase::Deciding, Focus::Overlay, key('x')),
            Some(Deed::Write)
        );
        assert_eq!(
            deed(Phase::Running, Focus::Overlay, key('s')),
            Some(Deed::Stop)
        );
        assert_eq!(deed(Phase::Running, Focus::Overlay, key('t')), None);
        assert_eq!(
            deed(Phase::Running, Focus::Overlay, key('q')),
            Some(Deed::CloseOverlay)
        );
        // `C-c` 在哪儿都退。
        for focus in [Focus::VolumeList, Focus::Input, Focus::Overlay] {
            assert_eq!(
                deed(Phase::Running, focus, Chord::Key(Key::Interrupt)),
                Some(Deed::Interrupt)
            );
        }
    }

    /// 屏底的几件事从表上取写法与那一句，相邻同句并成一件，派不出的不摆。
    #[test]
    fn hints_are_spelt_from_the_table_and_merge_neighbours_that_say_the_same() {
        let got = hints(
            Phase::Fresh,
            Focus::VolumeList,
            &[
                Want::of(Deed::Preview),
                Want::of(Deed::Down),
                Want::of(Deed::Up),
                Want::of(Deed::Follow),
                Want::of(Deed::Help),
            ],
        );
        let said: Vec<(String, &str)> = got.iter().map(|hint| (hint.spelt(), hint.what)).collect();
        assert_eq!(
            said,
            [
                ("t".to_owned(), "预览"),
                ("j/k".to_owned(), "选择"),
                ("?".to_owned(), "全部按键")
            ],
            "自动滚动还没开始时派不出，不摆"
        );
        // 已结束时同一件事换一句。
        let again = hints(Phase::Ended, Focus::VolumeList, &[Want::of(Deed::Preview)]);
        assert_eq!(again[0].what, "再预览");
        // 几句里点名一句。
        let latched = hints(
            Phase::Running,
            Focus::VolumeList,
            &[Want::saying(Deed::Stop, "再按一次立即停")],
        );
        assert_eq!(latched[0].what, "再按一次立即停");
    }

    /// 全部按键那一张的组照设计稿的次序与名字，每一组至少有一行上得了那一张。
    #[test]
    fn every_group_has_a_title_and_at_least_one_row_for_the_overlay() {
        let titles: Vec<&str> = Group::ALL.iter().map(|group| group.title()).collect();
        assert_eq!(
            titles,
            [
                "全局",
                "移动",
                "卷列表",
                "路径",
                "转换",
                "确认",
                "每页结果",
                "配置",
                "输入"
            ]
        );
        for group in Group::ALL {
            assert!(
                TABLE
                    .iter()
                    .any(|row| row.group == group && !row.long.is_empty()),
                "{group:?} 那一组一行都上不了全部按键那一张"
            );
        }
    }
}
