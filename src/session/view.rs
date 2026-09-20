//! **视图 × 阶段 × 焦点**：新会话的界面状态（ADR 0019；spec《状态：视图 × 阶段 × 焦点》；
//! `CONTEXT.md` 的《会话》：视图、任务视图、配置视图、焦点、卷列表、屏底、退出会话）。
//!
//! 会话此刻在做什么由三样说：**视图**（任务 · 配置，[`View`]）、**阶段**（这一趟走到哪儿了，
//! 仍是 [`Stage`]，一格没动）、**焦点**（视图里的哪一块，[`Focus`]）。两个视图**各记各的**
//! 光标与所在的块（[`TaskView`]、[`ConfigView`]），切走再切回原样；盖在上面的输入行与覆盖层
//! 是焦点的另两个取值，掀掉之后底下原样回来。
//!
//! **旧界面那一维（[`super::state::Focus`]）原样留着**，与本模块并存：真会话仍进旧界面，
//! 切换在 `session-redesign/15`。本模块挂在 [`Session`] 上一格（[`Session::views`]），
//! 新界面的状态机是本模块里那几个 `impl Session`——三组设置与阶段仍在 [`Session`] 自己身上，
//! 两副界面读的是同一份。
//!
//! # 按键表在别处
//!
//! 「哪些键在哪个状态下有效」一处答完，按视图、阶段、焦点查——那张表在 [`super::keymap`]；
//! 本模块只做两件事：把一个输入认成表上的一件事（[`Session::deed_of`]，连击键的前半截在这里待着），
//! 与把那件事做掉（[`Session::perform`]）。屏底此刻摆哪几件、按什么轻重也在这里
//! （[`Session::hints`]），每一件的键与那一句仍从表上取。
//!
//! # 卷列表的光标记的是行的身份
//!
//! 输出目录、哪一条处理路径、「＋ 添加路径」（[`Cursor`]），不是第几行：删掉一条、列表从处理路径
//! 换成树时光标不乱跳。开跑之前那一副的行由 [`Session::lines`] 现算——处理路径有几条就有几行。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）。屏底那一句回话
//! （[`Reply`]）与连击键的待续记号（[`Pending`]）都读会话的[「此刻」](CONTEXT.md)——
//! 几秒后退回、多久算过期，两个时长照设计稿。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::config::{self, Choices, Item};
use super::cover::Overlay;
use super::keymap::{self, Chord, Deed, Hint, Phase, Want};
use super::live::{Live, VolumeState};
use super::look::{Kind, Look, Segment};
use super::state::{Exit, Field, Key, Listing, NamedPath, OUTPUT_UNSET, Session, Shape, Stage};
use super::tone::Tone;
use super::tree;
use super::typing::{InputLine, Purpose};
use crate::preset::Preset;
use crate::render;
use tonefit::{Panel, VolumeReport};

/// 回话在屏底占几秒（设计稿 `toast` 的默认时长）。
pub const REPLY_LINGERS: Duration = Duration::from_millis(2600);

/// **跳一次之后那一句**在屏底占几秒（设计稿 `jump` 里那 1400 毫秒）：
/// 「问题 4/4」「搜索结果 1/1」都是它。比[寻常那一句](REPLY_LINGERS)短——
/// 它报的是一个数，看一眼就够。
const JUMP_LINGERS: Duration = Duration::from_millis(1400);

/// **`F` 交回自动滚动**那一句在屏底占几秒（设计稿 `taskKey` 里那 1600 毫秒）。
const FOLLOW_LINGERS: Duration = Duration::from_millis(1600);

/// 连击键按了前半截之后等后半截等多久（设计稿 `frame` 里那 900 毫秒）。
pub const COMBO_WAITS: Duration = Duration::from_millis(900);

/// 按停止之后屏底那一句占几秒（设计稿 `stopKey` 的 4000 毫秒）：它比寻常那一句久，
/// 因为它要人读完「再按一次 s 立即停止」。
pub const STOP_LINGERS: Duration = Duration::from_millis(4000);

/// **灰阶测试图那一句**在屏底占几秒（设计稿 `configKey` 的 `c` 那一支给的 3200 毫秒）：
/// 它比寻常那一句久——那一句里有一条路径要读。
#[cfg_attr(
    not(feature = "tui"),
    expect(
        dead_code,
        reason = "出图那一件要碰盘，落在终端层那一支上，而它在 tui 特性后面"
    )
)]
pub const CHART_LINGERS: Duration = Duration::from_millis(3200);
/// **转轮**转一格要多久（设计稿 `SPIN` 那一处的 90 毫秒）。
#[cfg_attr(
    not(feature = "tui"),
    expect(
        dead_code,
        reason = "只有画法读得到，而它在 tui 特性后面：顶栏此刻读它，行首记号与总览那两处随 08 接上"
    )
)]
pub const SPINS_EVERY: Duration = Duration::from_millis(90);

/// **转轮**的十格字形（设计稿的 `SPIN`）：顶栏右端那一截、行首记号的「处理中」、
/// 总览上清点那一条，三处同一份。转到第几格由会话的[「此刻」](CONTEXT.md)算
/// （[`Views::spinning`]）。
#[cfg_attr(
    not(feature = "tui"),
    expect(
        dead_code,
        reason = "只有画法读得到，而它在 tui 特性后面：顶栏此刻读它，行首记号与总览那两处随 08 接上"
    )
)]
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// 跑着与等待确认时定不下来那一刻屏底说的**短的那一句**（换型号那一下；设计稿 `configKey`）。
fn locked_short() -> Vec<Segment> {
    vec![Segment::new(
        "正在转换，设置已锁定",
        Look::tone(Tone::Caution),
    )]
}

/// **长的那一句**（取值环上定那一下）：前半截重、后半截说清什么时候才改得动。
fn locked_long() -> Vec<Segment> {
    vec![
        Segment::new("正在转换，", Look::tone(Tone::Caution).bold()),
        Segment::plain("设置已锁定，结束后才能修改"),
    ]
}

/// 会话的顶层：两个视图，各占整屏（`CONTEXT.md` 的《会话》：视图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// **任务视图 (Task view)**：做事的那一屏——总览、确认条、卷列表或每页结果。
    #[default]
    Task,
    /// **配置视图 (Config view)**：调设置的那一屏——设置栏与详情栏。
    Config,
}

impl View {
    /// 顶栏上的写法：号与名。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn number(self) -> &'static str {
        match self {
            Self::Task => "1",
            Self::Config => "2",
        }
    }

    /// 顶栏上的名字。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn name(self) -> &'static str {
        match self {
            Self::Task => "任务",
            Self::Config => "配置",
        }
    }

    /// 另一个视图（`gt`／`gT` 轮换，只有两个，两个方向是同一个）。
    pub fn other(self) -> Self {
        match self {
            Self::Task => Self::Config,
            Self::Config => Self::Task,
        }
    }
}

/// **焦点**的取值：视图里的块，外加盖在上面的两种（`CONTEXT.md` 的《会话》：焦点；
/// ADR 0017 修订记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// **卷列表 (Volume list)**：任务视图的主体。
    VolumeList,
    /// **每页结果 (Pages)**：进了一卷时换掉卷列表的那一副。
    Pages,
    /// **设置栏 (Settings pane)**：配置视图左边那一栏。
    Settings,
    /// **详情栏 (Details pane)**：配置视图右边那一栏。
    Details,
    /// **预设栏 (Picker)**：`p` 掀开、替换详情栏的那一栏。
    Picker,
    /// **输入行 (Input line)**：打字时占住屏底的那一行。
    Input,
    /// **覆盖层 (Overlay)**：全部按键或说明卡。
    Overlay,
}

/// 卷列表的光标停在哪一行——记的是**行的身份**（模块文档《卷列表的光标记的是行的身份》）。
///
/// 前三个是**开跑之前**那一副的行，后三个是**清点之后**那棵树上的行：展开收起、
/// 列表从处理路径换成树时光标都不乱跳。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    /// 输出目录那一行。
    Output,
    /// 哪一条处理路径。
    Path(PathBuf),
    /// 「＋ 添加路径」那一行。
    Add,
    /// 树上哪一个目录行（按那个目录记）。
    Directory(PathBuf),
    /// 树上哪一卷（按卷根记）。
    Volume(PathBuf),
    /// 树上哪一条备注行（无法访问的地方按那一处记，非漫画文件按它挂着的节点记）。
    Note(PathBuf),
}

/// 卷列表上的一行。**形状随阶段换**（`CONTEXT.md` 的《卷列表》）：清点完之前是开跑之前
/// 那一副（输出目录 · 「处理路径 (N)」· 一条条处理路径 · 「＋ 添加路径」），
/// 清点完之后同一张列表重排成[那棵树](super::tree)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// 输出目录那一行。
    Output,
    /// 「处理路径 (N)」那一行标签。停不住。
    Heading,
    /// 第几条处理路径。
    Path(usize),
    /// 「＋ 添加路径」。
    Add,
    /// 树上的一行。
    Tree(tree::Row),
}

impl Line {
    /// 这一行停得住的话，它的身份。
    pub fn stop(&self, paths: &[NamedPath], tree: &tree::Tree) -> Option<Cursor> {
        match self {
            Self::Output => Some(Cursor::Output),
            Self::Heading => None,
            Self::Path(at) => paths.get(*at).map(|named| Cursor::Path(named.path.clone())),
            Self::Add => Some(Cursor::Add),
            Self::Tree(row) => row.stop(tree),
        }
    }
}

/// **每页结果**开着的那一副（`CONTEXT.md` 的《会话》：每页结果）：进了哪一卷、
/// 光标停在第几页、这一副列的是哪几页。
///
/// **一卷的身份是卷根**，与[卷列表的光标](Cursor::Volume)记的是同一样：回卷列表那一下
/// 光标一格都不必挪——它本来就停在那一卷上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    /// 进了哪一卷（卷根）。
    pub volume: PathBuf,
    /// 光标停在**这一副列出来的第几页**上。
    ///
    /// **越界不算错，就近收到最后一页上**（[`settled`](Self::settled)）：
    /// 这一副列着几页要那一卷的报告，而报告只有那一趟给得出——
    /// 挪光标那一下（[`Session::place_cursor`]）因此不收上界，
    /// 每一下再由 [`Session::watch_the_run`] 收回来，屏上也照同一条画。
    pub at: usize,
    /// 这一副列的是[需留意的页](Listing::Notable)还是[全部页](Listing::All)。
    pub listing: Listing,
}

impl Pages {
    /// 刚进一卷那一刻：光标在头一页，只列需留意的页（设计稿 `taskKey` 那一支）。
    pub fn of(volume: PathBuf) -> Self {
        Self {
            volume,
            at: 0,
            listing: Listing::default(),
        }
    }

    /// **这一副此刻列着哪几页**（答的是「整卷那几页里的第几页」）——**判据只有这一处**：
    /// 屏上列哪几行、框底边那个 `n of m`、光标收到哪儿读的都是它。
    ///
    /// 「这一页要留意吗」判在 [`render::notable`] 一处，与卷行行尾那几个数
    /// （[`Live::notable_at`]）、与命令行印出去的那一份同一份判定——
    /// **代表页也算要留意**（它是这一卷的答案，非在不可）。
    ///
    /// **跳过的卷这一份是空的**：它这一趟一页都没重新分析，报告里一页都没有。
    pub fn listed(&self, report: &VolumeReport, panel: Panel) -> Vec<usize> {
        let all = self.listing == Listing::All;
        render::notable(report, panel)
            .into_iter()
            .enumerate()
            .filter(|(_, why)| all || !why.is_empty())
            .map(|(at, _)| at)
            .collect()
    }

    /// 这一卷**要留意的有几页**：抬头那一格 `需留意 1/189 页` 的头一个数。
    /// 与[列哪几页](Self::listed)同一份判定，只是不管此刻是哪一档。
    pub fn notable_count(report: &VolumeReport, panel: Panel) -> usize {
        render::notable(report, panel)
            .into_iter()
            .filter(|why| !why.is_empty())
            .count()
    }

    /// 光标停在**列出来的第几页**上：越界就近收到最后一页，一页都没列出来时是 0。
    pub fn settled(&self, listed: usize) -> usize {
        self.at.min(listed.saturating_sub(1))
    }
}

/// 任务视图记着的：所在的块、卷列表的光标、清点之后那棵树连同展开着的目录与自动滚动。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    /// **每页结果**进了哪一卷；`None` 就是卷列表在屏上。
    ///
    /// **「此刻在哪一块」只有这一格**：[`focus`](Self::focus) 从它算，不另存一格——
    /// 两格记同一件事，迟早有一格忘了跟着换（那时屏上画着卷列表、按键表却按每页结果派）。
    pub pages: Option<Pages>,
    pub cursor: Cursor,
    /// **清点之后那棵树**：开工那一条带回来的清点清单拼出来的形状（[`super::tree`]）。
    /// 清点完之前是空的——卷列表那时列的是处理路径本身。
    pub tree: tree::Tree,
    /// **展开着的目录**是一个集合（`CONTEXT.md` 的《展开》），按目录的路径记；每次开跑清空。
    pub expanded: BTreeSet<PathBuf>,
    /// **自动滚动**开着没有（`CONTEXT.md` 的《自动滚动》）：开着时光标跟到正在处理的那一卷，
    /// 只滚不展。每次开跑扳回开着。
    pub follow: bool,
    /// **搜索此刻搜的是那一句**（`CONTEXT.md` 的《卷列表》：`/` 搜卷名或目录名）：
    /// `⏎` 定下来的那一句，`Esc` 丢掉它。`n`／`N` 跳的就是它，屏上匹配处的下划线
    /// 与框底边那一截也按它画。**一个字都没打就不存**（那时它是 `None`，
    /// 不是一个空串）。
    ///
    /// **搜索那一行开着的时候不读它**：那一刻搜的是缓冲本身（[`Views::searching`]
    /// 一处答完）——打一个字下划线当场跟着动，而定下来之前这一格还是上一句。
    ///
    /// **每次开跑清掉**（设计稿 `startRun`）：上一趟搜的那一句在新的一趟上说不通。
    pub search: Option<String>,
    /// **清点完了没有**：开工那一条到了、树拼过一次就是真。
    ///
    /// 记一格而不是拿「树上几卷」与「清单上几卷」比：**一卷都没清点出来的那一趟也清点完了**
    /// ——点名的地方全都无法访问时，屏上有的正是那几条**备注行**，而两个零比出来的是
    /// 「还没清点」。卷列表换不换形状问的也是它。
    pub surveyed: bool,
}

impl Default for TaskView {
    fn default() -> Self {
        Self {
            pages: None,
            cursor: Cursor::Output,
            tree: tree::Tree::default(),
            expanded: BTreeSet::new(),
            follow: true,
            search: None,
            surveyed: false,
        }
    }
}

impl TaskView {
    /// **每次开跑重来一遍**：树还没拼出来、一个目录都不展开、自动滚动扳回开着、
    /// 上一趟搜的那一句清掉，光标退回列表头一行（这一刻列表刚从树换回处理路径，
    /// 或者反过来）。**每页结果也收掉**（设计稿 `startRun`）：上一趟那一卷的逐页结果
    /// 在新的一趟上说不通。
    pub fn start_a_run(&mut self) {
        self.tree = tree::Tree::default();
        self.expanded.clear();
        self.follow = true;
        self.search = None;
        self.surveyed = false;
        self.pages = None;
    }

    /// 任务视图此刻在哪一块：进了一卷就是[每页结果](Focus::Pages)，否则是卷列表。
    /// **从[进了哪一卷](Self::pages)那一格算**，不另存一格。
    pub fn focus(&self) -> Focus {
        if self.pages.is_some() {
            Focus::Pages
        } else {
            Focus::VolumeList
        }
    }
}

/// 套着的那一份预设：名字与它说的那两组，改了几项从它算。
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub name: String,
    pub preset: Preset,
}

/// 配置视图**左右那两栏**（`CONTEXT.md` 的《配置视图》：下面左右两栏）。
///
/// **它与[掀着预设栏没有](ConfigView::picker)是两维**（停车场 Q828 的 ②）：`⇥` 切的是这一维，
/// 切走再切回来右边那一栏仍是预设栏；[焦点](ConfigView::focus)由两维合起来推出来，
/// 屏上「焦点在哪」因此仍只有一处答案。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// 左边那一栏：**设置栏**。
    #[default]
    Settings,
    /// 右边那一栏：**详情栏**，或者预设栏掀着时的**预设栏**。
    Details,
}

/// **一份点了名的预设**：名字与它说的那两组（`CONTEXT.md` 的《预设栏》：每份说了哪几项）。
/// 预设栏列的就是它们——与[一条点了名的处理路径](super::state::NamedPath)同一副写法。
///
/// **读不懂的那一份只有名字**：一份字段过时的预设不该让别的几份列不出来
/// （与 [`crate::preset::Presets::names`] 同一条），屏上那一行因此说一句它读不懂。
#[derive(Debug, Clone, PartialEq)]
pub struct NamedPreset {
    pub name: String,
    /// 这一份的内容；读不懂就是 `None`。
    pub preset: Option<Preset>,
}

/// 配置视图记着的：在哪一栏、掀着预设栏没有、三块各自的光标、下钻进了哪一层、套着的预设，
/// 加上预设栏那几格（列的是哪几份、光标停在第几行、两处「再按一次」等着的是哪一份）。
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigView {
    /// 光标在左边那一栏还是右边那一栏（[`Pane`]）。
    pub pane: Pane,
    /// **掀着预设栏没有**（`CONTEXT.md` 的《预设栏》：`p` 掀开、替换详情栏）。
    /// 与[在哪一栏](Self::pane)是两维，见 [`Pane`]。
    pub picker: bool,
    /// **设置栏**的光标停在哪一项——记的是那一项的身份，不是第几行（组抬头停不住）。
    pub cursor: Item,
    /// **详情栏**的光标停在第几格（取值环的第几格、屏幕规格的第几块、下钻之后的第几个型号）。
    /// 一格都停不住的那两种（自由填、画质判定参数）上它恒是 0。
    pub choice: usize,
    /// **下钻**进了**哪一块屏幕规格**；`None` 是第一层（`CONTEXT.md` 的《下钻》）。
    /// 记的是那一块本身、不是它排第几——与旧界面取值栏那一格同一副（[`super::state::Values`]）。
    /// 设置栏上一挪光标它就作废：那一层是上一项的第二层。
    pub drill: Option<Panel>,
    /// 当前套的是哪一份预设。
    pub applied: Option<Applied>,
    /// 预设栏列的那几份——**进这一栏那一刻**盘上有的（`CONTEXT.md` 的《预设栏》）。
    /// 掀开那一下问一次盘（`super::terminal` 的 `toggle_picker`），掀着的时候不再问。
    pub listed: Vec<NamedPreset>,
    /// 预设栏的光标停在第几行：`0..listed.len()` 是那几份，`listed.len()` 是末行
    /// 「把当前设置保存为预设」。
    pub preset_cursor: usize,
    /// `dd` 按过第一下、等着第二下的是**哪一份**（按名字记，不按第几行）：
    /// 光标一挪它就作废，屏上那一句问的也是它。
    pub armed_delete: Option<String>,
    /// 起名那一行上 `⏎` 按过第一下、**撞上了同名的那一份**、等着第二下的那个名字
    /// （`CONTEXT.md` 的《预设》：盖掉一份同名的要按两下）。
    ///
    /// **与 `dd` 那一格分开**：屏上预设栏里那一句问的是删，而这一问在**屏底**、输入行还开着
    /// ——合成一格的话，起名那一刻那一栏里会冒出一句「再按一次 dd 删除」。
    /// 重开一次起名那一行就作废（[`Session::use_preset`]）：那一下起问的是新打的名字。
    pub armed_save: Option<String>,
}

impl Default for ConfigView {
    fn default() -> Self {
        Self {
            pane: Pane::Settings,
            picker: false,
            cursor: Item::Setting(Field::Profile),
            choice: 0,
            drill: None,
            applied: None,
            listed: Vec::new(),
            preset_cursor: 0,
            armed_delete: None,
            armed_save: None,
        }
    }
}

impl ConfigView {
    /// 焦点落在配置视图哪一块：左边是设置栏，右边看**掀着预设栏没有**
    /// ——掀着就是预设栏，否则是详情栏（停车场 Q828 的 ②：一维不够用，
    /// 而「焦点在哪」仍只有这一处答得出）。
    pub fn focus(&self) -> Focus {
        match (self.pane, self.picker) {
            (Pane::Settings, _) => Focus::Settings,
            (Pane::Details, true) => Focus::Picker,
            (Pane::Details, false) => Focus::Details,
        }
    }

    /// 预设栏上光标停着的是**哪一份**；停在末行那一件（保存）上是 `None`。
    pub fn picked(&self) -> Option<&NamedPreset> {
        self.listed.get(self.preset_cursor)
    }

    /// 预设栏上停得住几行：那几份加末行那一件。
    pub fn preset_stops(&self) -> usize {
        self.listed.len() + 1
    }
}

/// 连击键按了前半截：屏底右端留一个待续记号，等后半截等到 [`COMBO_WAITS`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub prefix: char,
    pub at: Instant,
}

/// 屏底那一句**回话**：按键之后临时占住屏底的几截字，到点退回按键提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub segments: Vec<Segment>,
    pub until: Instant,
}

/// **窗口**有多大：列 × 行。终端层每一帧问一次交进来（覆盖层滚到哪儿为止从它算，
/// 半屏与一屏那四个挪几行也从它算），用例给序列清单上的尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub cols: u16,
    pub rows: u16,
}

impl Window {
    /// **一屏挪几行**（设计稿的 `pageH`）：窗口高**减十二**，至少四行。
    ///
    /// 那十二行是屏上不归列表的那几行（顶栏、总览那个框、屏底）——它是一个**定数**，
    /// 不是当场量出来的列表高度：总览正文那一档一换，列表就高矮不同，而「按一次 `C-f`
    /// 挪多少」不该跟着变。`C-f`／`C-b` 挪这么多，`C-d`／`C-u` 挪它的一半。
    pub fn page(self) -> usize {
        usize::from(self.rows.saturating_sub(12).max(4))
    }
}

/// 新界面的状态：此刻在哪个视图、两个视图各自记着的、盖在上面的两样、屏底那两样临时的东西。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Views {
    pub view: View,
    pub task: TaskView,
    pub config: ConfigView,
    /// 打字时占住屏底的那一行（[`super::typing`]）。
    pub input: Option<InputLine>,
    /// 掀开着的那一张（[`super::cover`]）。盖住输入行，不替掉它。
    pub cover: Option<Overlay>,
    /// **会话的时钟起点**：转轮转到第几格从「此刻」减它算。由会话入口摆下这一刻
    /// （`CONTEXT.md` 的《会话》：此刻），用例给定值——设计稿把它那只表冻在场景数据的
    /// `now_ms` 上，夹具照它往回推。**还没起过表时转轮停在第一格**：真会话里到不了，
    /// 入口第一件事就是摆下它。
    pub clock: Option<Instant>,
    /// **预设文件在哪**：配置视图顶上那一条右端写它（家目录缩写成 `~`）。
    /// 由会话入口问一次摆进来（`Presets::path`），
    /// 问不出来就是 `None`——那一格空着，与家目录问不出来时不缩写同一条。
    pub presets: Option<PathBuf>,
    pending: Option<Pending>,
    reply: Option<Reply>,
}

impl Views {
    /// 焦点落在哪一块：盖在上面的先算——掀着覆盖层就是它，打着字就是输入行——都没有才是
    /// 此刻那个视图记着的块（[`block`](Self::block)）。
    pub fn focus(&self) -> Focus {
        if self.cover.is_some() {
            return Focus::Overlay;
        }
        if self.input.is_some() {
            return Focus::Input;
        }
        self.block()
    }

    /// 此刻那个视图记着的块——盖在上面的两样不算。屏上写在一块里的那几句顺口提的键
    /// （总览的 `t`／`x`、行上的 `i`／`o`）问的是这一块派什么，输入行开着时照样写着。
    pub fn block(&self) -> Focus {
        match self.view {
            View::Task => self.task.focus(),
            View::Config => self.config.focus(),
        }
    }

    /// 屏上那个**转轮**此刻转到哪一格（模块文档《它一个终端都不碰》：两个时长照设计稿）。
    #[cfg_attr(
        not(feature = "tui"),
        expect(
            dead_code,
            reason = "只有画法读得到，而它在 tui 特性后面：顶栏此刻读它，行首记号与总览那两处随 08 接上"
        )
    )]
    pub fn spinning(&self, now: Instant) -> &'static str {
        let since = self.clock.map_or(Duration::ZERO, |started| {
            now.saturating_duration_since(started)
        });
        let step = (since.as_millis() / SPINS_EVERY.as_millis()) as usize;
        SPINNER[step % SPINNER.len()]
    }

    /// **说一句没做成**：屏底那一句的「出事」那一副（行首一个 `✗`，整句出事红）。
    ///
    /// 库那一侧回的话原样端上来、不另编一份（与旧界面的 `Session::complain` 同一条）。
    /// 屏上凡是报「没做成」的地方走这一处——行首那个记号与那一档语义因此只有这一份。
    pub(super) fn complain(&mut self, said: impl std::fmt::Display, now: Instant) {
        self.say(
            vec![
                Segment::new("✗ ", Look::tone(Tone::Trouble).bold()),
                Segment::new(said.to_string(), Look::tone(Tone::Trouble)),
            ],
            now,
        );
    }

    /// **此刻搜的是哪一句**（`CONTEXT.md` 的《卷列表》：`/` 搜卷名或目录名）——
    /// 一处答完，屏上匹配处的下划线、框底边那一截与 `n`／`N` 的落点读的都是它。
    ///
    /// **搜索那一行开着时就是它的缓冲**：打一个字、退一个字，屏上当场跟着动，
    /// 中间不存第二份；关掉之后是 `⏎` [定下来的那一句](TaskView::search)。
    ///
    /// **空串不算在搜，这一处一次判掉**：刚按下 `/` 还没打字时一条下划线都不画、
    /// 框底边也不摆那一截、`n`／`N` 没有可跳的——设计稿那几处问的都是
    /// `S.search && S.search.q`（两件事一个条件），这一处因此把它收成一件。
    /// 「那一行开着吗」是另一问，问 [`Session::searching_line`]。
    pub fn searching(&self) -> Option<&str> {
        let query = match &self.input {
            Some(line) if line.purpose == Purpose::Search => line.buffer.as_str(),
            _ => self.task.search.as_deref()?,
        };
        (!query.is_empty()).then_some(query)
    }

    /// 屏底右端此刻要不要待续记号：连击键的前半截还没过期。
    pub fn pending(&self, now: Instant) -> Option<char> {
        self.pending
            .filter(|pending| now.saturating_duration_since(pending.at) <= COMBO_WAITS)
            .map(|pending| pending.prefix)
    }

    /// 屏底此刻占着的那句回话，到点就没有了。
    pub fn reply(&self, now: Instant) -> Option<&[Segment]> {
        self.reply
            .as_ref()
            .filter(|reply| now < reply.until)
            .map(|reply| reply.segments.as_slice())
    }

    /// 说一句回话，占屏底 [`REPLY_LINGERS`]。
    pub(super) fn say(&mut self, segments: Vec<Segment>, now: Instant) {
        self.say_for(segments, REPLY_LINGERS, now);
    }

    /// 说一句回话，占屏底多久由这一句自己定（设计稿 `toast` 的第二个参数）。
    pub(super) fn say_for(&mut self, segments: Vec<Segment>, lingers: Duration, now: Instant) {
        self.reply = Some(Reply {
            segments,
            until: now + lingers,
        });
    }

    /// 取走待着的前半截（不管过没过期都取走：新按下的键要么接上它、要么从头算）。
    fn take_pending(&mut self, now: Instant) -> Option<char> {
        let prefix = self.pending(now);
        self.pending = None;
        prefix
    }
}

/// 终端层交给新会话的一个输入：键或鼠标（spec《缝》：收的东西从「键」扩成「键或鼠标」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    /// Ctrl 加一个字母（`C-c` 不在这里，它是 [`Key::Interrupt`]）。
    Ctrl(char),
    /// 滚轮：往下几格（负数往上）。
    Wheel(i16),
    /// 单击屏上第几列第几行。
    Click {
        x: u16,
        y: u16,
    },
}

impl Input {
    fn chord(self) -> Chord {
        match self {
            Self::Key(key) => Chord::Key(key),
            Self::Ctrl(letter) => Chord::Ctrl(letter),
            Self::Wheel(_) => Chord::Wheel,
            Self::Click { .. } => Chord::Click,
        }
    }
}

/// **跳转找的是哪一种落点**（设计稿 `jump` 的 `kind`；`CONTEXT.md` 的《卷列表》）。
///
/// 两种共用一套挑法（[`Session::hunt`]）：树上全部目录都摊开那一副里，光标之后的头一个，
/// 没有就绕回头一个。差的只有「哪几行算落点」与屏底那一句。
enum Hunt {
    /// `]d`／`[d`：**问题**——转换失败的卷 · 进了隔离的卷 · 有需留意的页的卷 ·
    /// 无法访问的地方。前三种问那一趟（[`Live::troubled_at`]），末一种是树上的备注行。
    Problems,
    /// `⏎`／`n`／`N`：**搜索命中**——目录名装着这一句的目录行，与「目录名/卷名」装着它的卷行。
    Matching(String),
}

impl Hunt {
    /// 这一行是一个落点吗。
    fn lands_on(&self, row: tree::Row, tree: &tree::Tree, live: Option<&Live>) -> bool {
        match self {
            Self::Problems => match row {
                tree::Row::Volume { at, .. } => live.is_some_and(|live| live.troubled_at(at)),
                tree::Row::Note { node, at, .. } => tree
                    .note(node, at)
                    .is_some_and(|note| note.kind == tree::NoteKind::Unreachable),
                _ => false,
            },
            // **目录名自己就装着这一句时，它底下那几卷不再各算一个落点**
            // （设计稿 `targets` 那一条）：搜「海贼」跳到的是那个目录行一次，
            // 不是它底下十八卷各一次。屏上那十八行照旧加下划线——
            // 匹配与落点是两件事（[`tree::Tree::searched_text`]）。
            Self::Matching(query) => match row {
                tree::Row::Directory { node, at, .. } => tree
                    .directory(node, at)
                    .is_some_and(|directory| directory.label.contains(query.as_str())),
                tree::Row::Volume { at, .. } => {
                    tree.directory_of(at)
                        .is_some_and(|directory| !directory.label.contains(query.as_str()))
                        && tree
                            .searched_text(row)
                            .is_some_and(|text| text.contains(query.as_str()))
                }
                _ => false,
            },
        }
    }

    /// 跳到了：屏底报这是第几个、共几个。
    fn landed(&self, which: usize, total: usize) -> Vec<Segment> {
        let head = match self {
            Self::Problems => Segment::new("问题 ", Look::tone(Tone::Trouble).bold()),
            Self::Matching(_) => Segment::new("搜索结果 ", Look::tone(Tone::Caution).bold()),
        };
        vec![head, Segment::plain(format!("{which}/{total}"))]
    }

    /// 一个都没有：问题那一路说的是好消息，搜索那一路报的是这一句没命中。
    fn found_nothing(&self) -> Vec<Segment> {
        match self {
            Self::Problems => vec![
                Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                Segment::plain("目前没有问题"),
            ],
            Self::Matching(query) => vec![Segment::new(
                format!("没有找到和「{query}」相关的卷或文件夹"),
                Look::tone(Tone::Caution),
            )],
        }
    }
}

impl Session {
    /// 卷列表此刻那几行。**清点完之前**是开跑之前那一副（输出目录 · 「处理路径 (N)」·
    /// 每条处理路径 · 「＋ 添加路径」）；**清点完之后**是[那棵树](super::tree)。
    ///
    /// 分界是[**清点完了没有**](TaskView::surveyed)：清点途中库一条事件都不报
    /// （`CONTEXT.md` 的《清点》），开工那一条到了树才拼得出来——屏上正是这么换的
    /// （清点中那一副仍列着处理路径，只把勾选框换成转轮）。
    pub fn lines(&self) -> Vec<Line> {
        let tree = &self.views.task.tree;
        if self.views.task.surveyed {
            return tree
                .rows(&self.views.task.expanded)
                .into_iter()
                .map(Line::Tree)
                .collect();
        }
        let mut lines = vec![Line::Output, Line::Heading];
        lines.extend((0..self.scope.paths.len()).map(Line::Path));
        // 开跑之后「＋ 添加路径」那一行不在：跑着的时候加不进路径（设计稿 `taskRows`）。
        if self.stage() == Stage::Fresh {
            lines.push(Line::Add);
        }
        lines
    }

    /// 光标此刻停在第几行。记着的身份找不到了（那一条删掉了、列表刚换了形状）就停到
    /// 头一行停得住的。
    pub fn cursor_line(&self) -> usize {
        let lines = self.lines();
        let wanted = &self.views.task.cursor;
        let tree = &self.views.task.tree;
        lines
            .iter()
            .position(|line| line.stop(&self.scope.paths, tree).as_ref() == Some(wanted))
            .or_else(|| {
                lines
                    .iter()
                    .position(|line| line.stop(&self.scope.paths, tree).is_some())
            })
            .unwrap_or(0)
    }

    /// 光标停在第几条停得住的行上（从 1 起），与停得住的行共几条——框底边那句 `2 of 15`。
    pub fn cursor_position(&self) -> (usize, usize) {
        let tree = &self.views.task.tree;
        let stops: Vec<usize> = self
            .lines()
            .iter()
            .enumerate()
            .filter(|(_, line)| line.stop(&self.scope.paths, tree).is_some())
            .map(|(at, _)| at)
            .collect();
        let here = self.cursor_line();
        let position = stops
            .iter()
            .position(|at| *at == here)
            .map_or(0, |at| at + 1);
        (position, stops.len())
    }

    /// 光标停在哪一条处理路径上（停在别的行上就是 `None`）。
    pub fn path_under_cursor(&self) -> Option<usize> {
        match &self.views.task.cursor {
            Cursor::Path(path) => self
                .scope
                .paths
                .iter()
                .position(|named| named.path == *path),
            _ => None,
        }
    }

    /// 这一条处理路径被另一条**勾着的文件夹**包含着的话，是哪一条（按路径前缀认，不碰盘）。
    pub fn nested_in(&self, at: usize) -> Option<&Path> {
        let inner = &self.scope.paths[at];
        self.scope
            .paths
            .iter()
            .enumerate()
            .filter(|(other, named)| *other != at && named.on && !named.is_archive())
            .map(|(_, named)| named.path.as_path())
            .find(|outer| inner.path.starts_with(outer) && inner.path != *outer)
    }

    /// 勾着几条、没勾几条。
    pub fn checked_paths(&self) -> (usize, usize) {
        let on = self.scope.paths.iter().filter(|named| named.on).count();
        (on, self.scope.paths.len() - on)
    }

    /// 与套着的预设不同的有几项。是哪几项、为什么型号不算，都在 [`config::changed`]
    /// ——总览与顶上那一条预设读的是同一份。
    pub fn changed_from_preset(&self) -> usize {
        config::changed(self).len()
    }

    /// 这一项此刻的值与那份预设说的不同吗。
    pub(super) fn differs_from(&self, field: Field, preset: &Preset) -> bool {
        match field {
            Field::Profile => self.device.profile != preset.device.profile,
            Field::GrayLevels => self.device.gray_levels != preset.device.gray_levels,
            Field::Threshold => self.device.threshold != preset.device.threshold,
            Field::Fit => self.taste.fit != preset.taste.fit,
            Field::Crop => self.taste.crop != preset.taste.crop,
            Field::Split => self.taste.split != preset.taste.split,
            Field::SplitThreshold => self.taste.split_threshold != preset.taste.split_threshold,
            Field::ReadingOrder => self.taste.reading_order != preset.taste.reading_order,
            Field::Filter => self.taste.filter != preset.taste.filter,
            Field::WhiteAlignLimit => {
                self.taste.white_align_limit != preset.taste.white_align_limit
            }
            Field::BitDepth => self.taste.bit_depth != preset.taste.bit_depth,
            Field::Dither => self.taste.dither != preset.taste.dither,
            Field::Envelope => self.taste.envelope != preset.taste.envelope,
            Field::CacheBudget => self.taste.cache_budget != preset.taste.cache_budget,
            Field::IoMode => self.taste.io_mode != preset.taste.io_mode,
            Field::Out | Field::Path(_) | Field::AddPath => false,
        }
    }

    /// 把一个输入认成按键表上的一件事。认不出（这个键此刻没有意义）就是 `None`。
    ///
    /// **连击键在这里待着**：前半截按下去先记下（屏底右端留待续记号），后半截来了合成一个连击键
    /// 去查表；合不上就把后半截当一个单独的键从头认。过了 [`COMBO_WAITS`] 前半截作废。
    pub fn deed_of(&mut self, input: Input, phase: Phase, now: Instant) -> Option<Deed> {
        let focus = self.views.focus();
        let chord = input.chord();
        if let Some(prefix) = self.views.take_pending(now)
            && let Chord::Key(Key::Char(second)) = chord
            && let Some(deed) = keymap::deed(phase, focus, Chord::Combo(prefix, second))
        {
            return Some(deed);
        }
        if let Chord::Key(Key::Char(first)) = chord
            && keymap::starts_a_combo(phase, focus, first)
        {
            self.views.pending = Some(Pending {
                prefix: first,
                at: now,
            });
            return None;
        }
        if let Some(deed) = keymap::deed(phase, focus, chord) {
            return Some(deed);
        }
        // 打字：输入行上表派不出的每一个字符都是一个字（`?`、`q`、`j` 也是）。
        if focus == Focus::Input {
            return match chord {
                Chord::Key(Key::Char(glyph)) => Some(Deed::Typed(glyph)),
                Chord::Key(Key::Space) => Some(Deed::Typed(' ')),
                _ => None,
            };
        }
        None
    }

    /// 把一件事做掉——**状态机够得着的那几件**：挪光标、勾选、删一条、换视图、退出、
    /// 掀开与关掉全部按键、打字与添改路径（[`super::typing`]：补全与确定那两下问一次盘）。
    ///
    /// 起一趟、按停止、答话、灰阶测试图，加上预设那几支里**碰盘的那三件**（掀开那一栏、
    /// `dd` 的第二下、起好名那一下），都要够着那一趟或者盘，归终端层那一支
    /// （`super::terminal`）；交到这里的那几件当作没有意义，原地不动。
    /// **套用一份在这里**（[`Deed::UsePreset`]）：列的那几份掀开那一刻已经读进来了。
    /// **跳转那几件同样**（`]d`／`[d`、`n`／`N`、搜索那一行上的 `⏎`）：落点要问那一趟，
    /// 那一支是 [`Session::jump`] 与 [`Session::confirm_search`]。
    /// 覆盖层上滚动要知道窗口有多大，同样在终端层那一支（[`super::cover::Sheet`] 与
    /// `Views::scroll_cover`）；半屏与一屏那四个同样（[`Self::scroll_list`]）。
    pub fn perform(&mut self, deed: Deed, now: Instant) -> Exit {
        let focus = self.views.focus();
        match deed {
            Deed::Quit | Deed::Interrupt => return Exit::Leave,
            Deed::Help | Deed::HelpWhileTyping => self.views.lift_keys(),
            Deed::CloseOverlay => self.views.drop_cover(),
            Deed::AddPath => self.open_adding(),
            Deed::EditPath => self.open_editing(),
            Deed::Typed(glyph) => {
                if let Some(line) = &mut self.views.input {
                    line.type_in(glyph);
                }
            }
            Deed::Erase => {
                if let Some(line) = &mut self.views.input {
                    line.erase();
                }
            }
            Deed::DeleteWord => {
                if let Some(line) = &mut self.views.input {
                    line.delete_word();
                }
            }
            Deed::Complete => self.complete_typed(now),
            Deed::Confirm => self.confirm_typed(now),
            Deed::Cancel => self.cancel_typed(),
            Deed::Down | Deed::Up if focus == Focus::Input => {
                if let Some(line) = &mut self.views.input {
                    line.step(if deed == Deed::Down { 1 } else { -1 });
                }
            }
            Deed::QuitRefused => self.views.say(
                vec![
                    Segment::new("正在转换：", Look::tone(Tone::Caution).bold()),
                    Segment::plain("q 不会退出，请先按 s 停止，或按 C-c 立即退出"),
                ],
                now,
            ),
            Deed::TaskView => self.views.view = View::Task,
            Deed::ConfigView => self.views.view = View::Config,
            Deed::NextBlock => self.switch_pane(),
            Deed::ConfigOpen if self.views.config.pane == Pane::Settings => self.enter_details(),
            Deed::ConfigOpen => self.settle_choice(now),
            Deed::ConfigEnter => self.config_enter(),
            Deed::ConfigBack => self.config_back(),
            Deed::EditValue => self.open_valuing(),
            // **预设栏上 `⏎`／`l`**：套用光标停着的那一份、或者停在末行时开输入行起名。
            // 两件都够不着盘——列的那几份是掀开那一刻读进来的（[`Self::lift_picker`]）。
            // 掀开与收起（`p`）、`dd` 删一份、存一份、灰阶测试图那四件要读写盘，
            // 归终端层那一支（`super::terminal::input`）。
            Deed::UsePreset => self.use_preset(now),
            Deed::NextView | Deed::PrevView => self.views.view = self.views.view.other(),
            Deed::Down => self.place_cursor(now, |here, last| (here + 1).min(last)),
            Deed::Up => self.place_cursor(now, |here, _| here.saturating_sub(1)),
            Deed::Bottom => self.place_cursor(now, |_, last| last),
            Deed::Top => self.place_cursor(now, |_, _| 0),
            // **展开只管目录行**：卷行按下去要问那一趟「这一卷展不展得开」，
            // 而状态机读不到它——那一支在终端层（`super::terminal::input`）。
            // **按停止升一级**（ADR 0013 的两级停止）：把升到的那一级交给跑着的那一趟
            // 是终端层那一支（`super::terminal::input`）——两处记的是同一个字，
            // 出处只有这一份。
            Deed::Stop => self.stop_a_notch(now),
            Deed::Open => self.open_under_cursor(),
            Deed::Close => self.collapse_directory(),
            // **`F` 交回自动滚动**：扳回那一格、屏底说一句是这里的事；而**光标当场
            // 跟到正在处理的那一卷**要读那一趟，那一步在终端层那一支上
            // （`super::terminal::input` 按下这一件之后再盯一眼 [`Session::watch_the_run`]）
            // ——与按停止那一件同一条分工，记着那一格的仍只有这一处。
            Deed::Follow => {
                self.views.task.follow = true;
                self.views.say_for(
                    vec![
                        Segment::new("自动滚动：", Look::kind(Kind::Done).bold()),
                        Segment::plain("跟到正在处理的卷"),
                    ],
                    FOLLOW_LINGERS,
                    now,
                );
            }
            // **`/` 开搜索那一行**：屏底换成 `/` 加缓冲，底下那张列表照旧（框细一档）。
            Deed::Search => {
                self.views.input = Some(InputLine::new(Purpose::Search, ""));
            }
            // **卷列表上的 `Esc` 只丢掉搜索**（`CONTEXT.md` 的《退出会话》：`Esc` 只退一级）
            // ——没在搜的时候它一件事都不做。说明卡那一级在 [`Deed::CloseOverlay`]。
            Deed::ClearSearch => self.views.task.search = None,
            // **`a` 换列法**（spec 的《每页结果》：默认只列需留意的页，`a` 切换）：
            // **换过之后光标退回头一页**——换的是列哪几页，上一副的第几页在新的一副上
            // 不是同一页（设计稿 `taskKey` 那一支同样把它扳回 0）。
            Deed::ListAll => {
                if let Some(pages) = &mut self.views.task.pages {
                    pages.listing = pages.listing.flipped();
                    pages.at = 0;
                }
            }
            // **`h`／`Esc`／`⌫` 回卷列表原处**：那一卷的行本来就在光标底下
            // （[`Pages::volume`] 与 [`Cursor::Volume`] 记的是同一样），收掉这一副就回到原处。
            Deed::BackToList => self.views.task.pages = None,
            Deed::TogglePath => {
                if let Some(at) = self.path_under_cursor() {
                    self.scope.paths[at].on = !self.scope.paths[at].on;
                }
            }
            Deed::DeletePath => self.delete_path(now),
            // 结束之后 `o`／`i` 回到开跑之前那一副；上一趟的报告留到退出时印。
            Deed::BackToPaths => {
                self.back_to_paths();
                self.views.say(
                    vec![
                        Segment::new("返回路径列表", Look::PLAIN.bold()),
                        Segment::faint(" ⋅ 上次的结果会在退出时打印"),
                    ],
                    now,
                );
            }
            _ => {}
        }
        Exit::Stay
    }

    /// **半屏与一屏那四个**（`C-d`／`C-u`／`C-f`／`C-b`）：光标挪[一屏](Window::page)
    /// 或半屏那么多行。**挪几行要窗口有多高**，而那件事只有终端层知道——与
    /// [`Views::scroll_cover`] 同一条分工，因此这一支也在那一层调
    /// （`super::terminal` 的 `input`）。不是这四件就交回 `false`，让状态机接着认。
    ///
    /// **卷列表与每页结果挪的行数是同一个**（[`Window::page`]）：两块都占着屏上那一整格。
    pub fn scroll_list(&mut self, deed: Deed, window: Window, now: Instant) -> bool {
        // **不在任务视图上就一件都不认**：这四个键在表上派给**没被盖着的每一块**
        // （`UNCOVERED_OR_OVERLAY`），而这一支只挪得动任务视图那两块的光标
        // （卷列表与每页结果，两块共用 [`Self::place_cursor`]）——认下来却什么都不做，
        // 配置视图按下去会是一片静默（那两栏自己的滚动随它那一票接）。
        if self.views.view != View::Task {
            return false;
        }
        let page = window.page();
        let by: isize = match deed {
            Deed::HalfDown => (page / 2) as isize,
            Deed::HalfUp => -((page / 2) as isize),
            Deed::PageDown => page as isize,
            Deed::PageUp => -(page as isize),
            _ => return false,
        };
        self.place_cursor(now, |here, last| here.saturating_add_signed(by).min(last));
        true
    }

    /// 光标挪到停得住的行里的哪一条：`to` 收「此刻在第几条、最后一条是第几条」，答挪到第几条。
    /// 任务视图在卷列表或[每页结果](Pages)上挪，配置视图两栏各挪各的
    /// （[`Self::place_config_cursor`]）。
    fn place_cursor(&mut self, now: Instant, to: impl Fn(usize, usize) -> usize) {
        if self.views.view == View::Config {
            self.place_config_cursor(to);
            return;
        }
        // **每页结果挪的是「第几页」**：这一副列着几页要那一卷的报告，而状态机读不到
        // 那一趟——因此这一处不收上界（拿 `usize::MAX` 当「最后一页」），光标每一下由
        // [`Self::watch_the_run`] 收进真列出来的那几页里，屏上照同一条画
        // （[`Pages::settled`]）。设计稿也是这一副：`moveBy` 只往上收到 0，
        // 往下那一头由 `drawPages` 每一帧收。
        //
        // **这一下不暂停自动滚动**：进了一卷就不在跟着的那张列表上了
        // （设计稿 `moveBy` 那一支在 `listGo` 之前返回）。
        if let Some(pages) = &mut self.views.task.pages {
            pages.at = to(pages.at, usize::MAX);
            return;
        }
        if !self.on_the_volume_list() {
            return;
        }
        let stops: Vec<Cursor> = self
            .lines()
            .iter()
            .filter_map(|line| line.stop(&self.scope.paths, &self.views.task.tree))
            .collect();
        let Some(last) = stops.len().checked_sub(1) else {
            return;
        };
        let here = stops
            .iter()
            .position(|stop| *stop == self.views.task.cursor)
            .unwrap_or(0);
        // **按了挪光标那几个键就暂停自动滚动，挪得动挪不动都算**（设计稿 `listGo`
        // 那一支不问光标有没有真挪）：已经到底了再按一下 `j` 同样是「这一下起我自己看」
        // ——不暂停的话下一帧 [`Self::watch_the_run`] 就把光标拽回正在处理的那一卷，
        // 而躲开那一下正是按这个键的用意。跟着的那一卷正停在列表两头时踩得到。
        self.pause_follow(now);
        self.views.task.cursor = stops[to(here, last).min(last)].clone();
    }

    /// **光标此刻挪得动吗**：人在任务视图，而那个视图记着的块是卷列表。
    ///
    /// 问的是[视图记着的那一块](Views::block)、不是[此刻的焦点](Views::focus)：
    /// 掀着说明卡时 `j`／`k` 挪的仍是底下那张列表的光标（设计稿 `moveBy` 那一支；
    /// 全部按键那一张自己会先把滚动那几件收走，见 [`Views::scroll_cover`]）。
    fn on_the_volume_list(&self) -> bool {
        self.views.view == View::Task && self.views.task.focus() == Focus::VolumeList
    }

    /// 按停止升一级，屏底说一句：按一次做完当前卷再停，再按一次立即停止。
    fn stop_a_notch(&mut self, now: Instant) {
        self.raise_stop();
        let (head, rest) = if self.stopping() == tonefit::Instruction::Abort {
            ("! 已立即停止：", "当前卷未保存，输出目录里不会留下半成品")
        } else {
            ("! 正在停止：", "做完当前卷就停 ⋅ 再按一次 s 立即停止")
        };
        self.views.say_for(
            vec![
                Segment::new(head, Look::tone(Tone::Caution).bold()),
                Segment::plain(rest),
            ],
            STOP_LINGERS,
            now,
        );
    }

    /// **光标一挪，自动滚动就暂停**（`CONTEXT.md` 的《自动滚动》）：挪光标那几个键、
    /// 滚轮、单击都算，屏底跟着说一句「已暂停自动滚动 ⋅ 按 F 恢复」，`F` 交回。
    /// 滚轮与单击那一路随鼠标那一票（16）接到这一处上。
    ///
    /// **跳转不走这一处**：`]d`／`[d` 与搜索跳过去同样暂停，但屏底说的是它们自己那一句
    /// （「问题 4/4」「搜索结果 1/1」），不是这一句——两句抢同一行，
    /// 说出口的只能是人刚按下那件事（[`Self::hunt`] 因此自己扳那一格）。
    ///
    /// **跟不上东西的那几档一声不响**：还没开跑、清点中、已结束时本来就没有
    /// 「正在处理的那一卷」可跟（[`Self::following_matters`]），挪光标不算离开它。
    fn pause_follow(&mut self, now: Instant) {
        if !self.following_matters() || !self.views.task.follow {
            return;
        }
        self.views.task.follow = false;
        self.views.say(
            vec![
                Segment::new("已暂停自动滚动", Look::tone(Tone::Caution).bold()),
                Segment::faint(" ⋅ 按 "),
                Segment::new("F", Look::PLAIN.bold()),
                Segment::faint(" 恢复"),
            ],
            now,
        );
    }

    /// **自动滚动此刻跟得上东西吗**：一趟正在跑（转换中或等待确认）而且**清点完了**。
    ///
    /// 清点那一段树还没有、列表列的仍是处理路径，屏上一个「正在处理的那一卷」都指不出来
    /// ——那一档挪光标不该说「已暂停」。问的正是[清点完了没有](TaskView::surveyed)
    /// 那一格，与框右端那一枚（`super::shell::list` 读 [`Phase::Running`]／
    /// [`Phase::Deciding`]）分的是同一条界。
    fn following_matters(&self) -> bool {
        matches!(self.stage(), Stage::Running(_) | Stage::Deciding(_)) && self.views.task.surveyed
    }

    /// `l`／`⏎` 按在光标那一行上：**目录行展开**（`CONTEXT.md` 的《展开》：目录→卷
    /// 就地展开），**备注行掀开说明卡**（`Esc` 关）。
    ///
    /// 卷行不在这里——那一下要问那一趟「这一卷展不展得开」，而状态机读不到它
    /// （那一支在 `super::terminal` 的 `input`）。
    fn open_under_cursor(&mut self) {
        match &self.views.task.cursor {
            Cursor::Directory(path) => {
                let path = path.clone();
                self.views.task.expanded.insert(path);
            }
            // 光标记的是那一条备注的身份，卡记的是它在树上的位置（[`Overlay::Note`]）。
            Cursor::Note(at) => {
                if let Some((node, which)) = self.views.task.tree.locate_note(at) {
                    self.views.lift_note(node, which);
                }
            }
            Cursor::Output | Cursor::Path(_) | Cursor::Add | Cursor::Volume(_) => {}
        }
    }

    /// `h`：目录行上收起它；卷行上收起它那个目录，光标跟着停到目录行上
    /// （收起之后那一卷的行没了，光标得有地方落）。
    fn collapse_directory(&mut self) {
        let directory = match &self.views.task.cursor {
            Cursor::Directory(path) => Some(path.clone()),
            Cursor::Volume(root) => {
                let root = root.clone();
                let at = self
                    .views
                    .task
                    .tree
                    .roots
                    .iter()
                    .position(|one| *one == root);
                at.and_then(|at| self.views.task.tree.directory_of(at))
                    .map(|directory| directory.path.clone())
            }
            _ => None,
        };
        let Some(directory) = directory else {
            return;
        };
        self.views.task.expanded.remove(&directory);
        self.views.task.cursor = Cursor::Directory(directory);
    }

    /// **盯着跑着的那一趟**：清点的产出一到就把树拼出来，自动滚动开着时光标跟到
    /// 正在处理的那一卷（`CONTEXT.md` 的《自动滚动》：它的目录展开着就停在卷行上，
    /// 收着就停在目录行上，**只滚不展**）。
    ///
    /// 由够得着那一趟的那一层每一下调一次（`super::terminal::input`，真会话切过来之后
    /// 是那条循环每一帧）：树的拼法要清点清单，而状态机读不到它。
    /// **树一趟只拼一次**——清单在开工那一条之后一格不变。
    pub fn watch_the_run(&mut self, live: &Live) {
        // **每页结果的光标收进它此刻列着的那几页里**：`a` 刚换过一副列法、那一卷刚做完，
        // 「列着几页」就换了一个数。收在这一处是因为那个数要那一卷的报告，而状态机读不到
        // 那一趟——与底下把树拼出来、把光标带到正在处理那一卷同一条分工。
        self.settle_the_pages(live);
        if !live.surveying() && !self.views.task.surveyed {
            self.views.task.tree = tree::Tree::of(
                &self.scope.paths,
                live.roster(),
                live.non_volume_files(),
                live.unreachable_places(),
            );
            self.views.task.surveyed = true;
        }
        if !self.views.task.follow || live.ended() {
            return;
        }
        let Some(walking) = live.walking() else {
            return;
        };
        let task = &self.views.task;
        let Some(at) = task.tree.index_of(&walking.volume) else {
            return;
        };
        let Some(directory) = task.tree.directory_of(at) else {
            return;
        };
        let cursor = if task.expanded.contains(&directory.path) {
            Cursor::Volume(task.tree.roots[at].clone())
        } else {
            Cursor::Directory(directory.path.clone())
        };
        self.views.task.cursor = cursor;
    }

    /// 光标收进[每页结果](Pages)此刻真列出来的那几页里（见 [`Pages::at`]）。
    fn settle_the_pages(&mut self, live: &Live) {
        let Some(listed) = self.listed_pages(Some(live)) else {
            return;
        };
        if let Some(pages) = &mut self.views.task.pages {
            pages.at = pages.settled(listed.len());
        }
    }

    /// **每页结果开着的那一卷那一份报告**：没进哪一卷、那一趟还没起来、
    /// 或者那一卷没有报告（没做成、还没轮到）时都是 `None`。
    ///
    /// **卷根换回清单序号只有一处**（[`tree::Tree::index_of`]），与
    /// [`Self::volume_state`] 走的是同一条。
    pub fn pages_report<'a>(&self, live: Option<&'a Live>) -> Option<&'a VolumeReport> {
        let pages = self.views.task.pages.as_ref()?;
        let at = self.views.task.tree.index_of(&pages.volume)?;
        live?.report_at(at)
    }

    /// **每页结果此刻列着哪几页**（整卷那几页里的第几页）——屏上列哪几行、
    /// 框底边那个数、光标收到哪儿，读的都是这一份（[`Pages::listed`]）。
    pub fn listed_pages(&self, live: Option<&Live>) -> Option<Vec<usize>> {
        let pages = self.views.task.pages.as_ref()?;
        let report = self.pages_report(live)?;
        Some(pages.listed(report, live?.report().profile.panel()))
    }

    // ───────────────────────── 跳转与搜索 ─────────────────────────

    /// **搜索那一行此刻开着吗**：终端层按它把 `⏎` 分给
    /// [`confirm_search`](Self::confirm_search)，别的输入行照旧交给状态机。
    pub fn searching_line(&self) -> bool {
        self.views.input.as_ref().map(|line| &line.purpose) == Some(&Purpose::Search)
    }

    /// **给预设起名那一行此刻开着吗**：终端层按它把 `⏎` 分给存一份那一支
    /// （`super::terminal` 的 `store_a_preset`——写盘状态机够不着），
    /// 别的输入行照旧交给状态机。与[搜索那一行](Self::searching_line)同一条分工。
    pub fn naming_a_preset(&self) -> bool {
        self.views.input.as_ref().map(|line| &line.purpose) == Some(&Purpose::Preset)
    }

    /// **搜索那一行上的 `⏎`**：把这一句定下来、跳到第一个结果
    /// （`CONTEXT.md` 的《卷列表》：`⏎` 跳到第一个）。
    ///
    /// **一个字都没打就只关掉那一行**：没有那一句，跳无处可跳（设计稿 `submitInput`
    /// 那一支同样不跳），屏底也不该报「没有找到和「」相关的卷或文件夹」。
    /// 空串在 [`Views::searching`] 一处判掉，这里只管把打出来的那一句收下。
    ///
    /// **定下来的那一句留着**（哪怕一个都没找到）——`n`／`N` 跳的是它，
    /// 框底边那一截写的也是它。
    pub fn confirm_search(&mut self, live: Option<&Live>, now: Instant) {
        let Some(line) = self.views.input.take() else {
            return;
        };
        self.views.task.search = Some(line.buffer);
        let Some(query) = self.views.searching().map(str::to_owned) else {
            return;
        };
        self.hunt(&Hunt::Matching(query), live, true, now);
    }

    /// **`]d`／`[d`／`n`／`N`**：跳到下一处／上一处。不是这四件就交回 `false`，
    /// 让状态机接着认。
    ///
    /// **落点要问那一趟**（哪几卷出了事、哪几卷在哪个目录里），而状态机读不到它
    /// ——与 [`Deed::Open`] 落在卷行上时同一条分工，因此这一支也在终端层那一层调
    /// （`super::terminal::input`）。
    ///
    /// `n`／`N` 在**没搜过**（或者搜的是空串）时一件事都不做：设计稿 `taskKey`
    /// 那一支就是这么拦的——那两个键在表上派得出，而没有那一句可跳。
    pub fn jump(&mut self, deed: Deed, live: Option<&Live>, now: Instant) -> bool {
        let (hunt, forward) = match deed {
            Deed::NextProblem => (Hunt::Problems, true),
            Deed::PrevProblem => (Hunt::Problems, false),
            Deed::SearchNext | Deed::SearchPrev => {
                // **没搜过就一件事都不做**（空串也算没搜，判在 [`Views::searching`] 一处）：
                // 那两个键在表上派得出，而没有那一句可跳——设计稿 `taskKey` 那一支
                // 就是这么拦的。
                let Some(query) = self.views.searching().map(str::to_owned) else {
                    return true;
                };
                (Hunt::Matching(query), deed == Deed::SearchNext)
            }
            _ => return false,
        };
        self.hunt(&hunt, live, forward, now);
        true
    }

    /// 跳一次：**树上全部目录都摊开那一副**（[`tree::Tree::every_row`]）里挑一个落点，
    /// 光标落上去，屏底报第几个（设计稿 `jump`）。
    ///
    /// **次序按树，不按屏上此刻摆着的那几行**：收着的目录里那几卷照样跳得到，
    /// 跳过去把那个目录**展开**（`CONTEXT.md` 的《卷列表》：收着的目录自动展开到那一卷）。
    /// 光标此刻那一行之后的头一个是「下一个」；一个都没有就**绕回头一个**
    /// （往上是绕回最后一个）。
    ///
    /// **跳过去即暂停自动滚动**（`CONTEXT.md` 的《自动滚动》）：不暂停的话下一帧
    /// 就被拽回正在处理的那一卷。屏底说的是这一次跳的第几个，
    /// 不是[挪光标那一句](Self::pause_follow)。
    fn hunt(&mut self, hunt: &Hunt, live: Option<&Live>, forward: bool, now: Instant) {
        let (landings, here) = {
            let tree = &self.views.task.tree;
            let rows = tree.every_row();
            let stops: Vec<Option<Cursor>> = rows.iter().map(|row| row.stop(tree)).collect();
            let landings: Vec<(usize, Cursor)> = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| hunt.lands_on(**row, tree, live))
                .filter_map(|(at, _)| stops[at].clone().map(|stop| (at, stop)))
                .collect();
            let here = stops
                .iter()
                .position(|stop| stop.as_ref() == Some(&self.views.task.cursor));
            (landings, here)
        };
        if landings.is_empty() {
            self.views.say(hunt.found_nothing(), now);
            return;
        }
        let which = if forward {
            landings
                .iter()
                .position(|(at, _)| here.is_none_or(|here| *at > here))
                .unwrap_or(0)
        } else {
            landings
                .iter()
                .rposition(|(at, _)| here.is_some_and(|here| *at < here))
                .unwrap_or(landings.len() - 1)
        };
        let (_, stop) = landings[which].clone();
        // 停在一卷上就把它那个目录展开——光标记的是那一卷，而它的行收着的时候不在屏上。
        let opened = match &stop {
            Cursor::Volume(root) => {
                let tree = &self.views.task.tree;
                tree.index_of(root)
                    .and_then(|at| tree.directory_of(at))
                    .map(|directory| directory.path.clone())
            }
            _ => None,
        };
        if let Some(path) = opened {
            self.views.task.expanded.insert(path);
        }
        self.views.task.cursor = stop;
        self.views.task.follow = false;
        self.views
            .say_for(hunt.landed(which + 1, landings.len()), JUMP_LINGERS, now);
    }

    /// 配置视图上挪光标：**三块各挪各的**——设置栏挪的是停得住的那 20 项，
    /// 详情栏挪的是此刻列得出的那几格（[`Choices::stops`]），预设栏挪的是那几份加末行那一件。
    ///
    /// **设置栏上一挪就退出下钻**：下钻那一层是**上一项**的第二层，光标换了项它就说不通了
    /// （设计稿 `moveBy` 配置那一支）。
    fn place_config_cursor(&mut self, to: impl Fn(usize, usize) -> usize) {
        match self.views.config.focus() {
            Focus::Settings => {
                let items = config::items();
                let Some(last) = items.len().checked_sub(1) else {
                    return;
                };
                let here = items
                    .iter()
                    .position(|item| *item == self.views.config.cursor)
                    .unwrap_or(0);
                self.views.config.cursor = items[to(here, last).min(last)];
                self.views.config.drill = None;
            }
            Focus::Details => {
                let last = self.config_choices().stops().saturating_sub(1);
                let here = self.views.config.choice.min(last);
                self.views.config.choice = to(here, last).min(last);
            }
            // **预设栏**：那几份加末行那一件，末行停得住（`⏎` 在它上面开输入行）。
            // **挪一格就把 `dd` 等着的那一下作废**（设计稿 `moveBy` 预设那一支）：
            // 「再按一次」问的是光标停着的那一份，光标挪走了那一问就不成立了。
            Focus::Picker => {
                let last = self.views.config.preset_stops().saturating_sub(1);
                let here = self.views.config.preset_cursor.min(last);
                self.views.config.preset_cursor = to(here, last).min(last);
                self.views.config.armed_delete = None;
            }
            _ => {}
        }
    }

    /// 配置视图此刻**三组设置改不改得动**（ADR 0017 决定第 3 条）。
    ///
    /// **它问的是阶段那一维，与焦点无关**：跑着与等待确认时详情栏、预设栏照样进得去、看得见，
    /// 一个改动都定不下（`CONTEXT.md` 的《焦点》最后那一句）。
    pub fn settings_locked(&self) -> bool {
        self.stage().read_only()
    }

    /// 设置栏上光标停在**第几项、共几项**（框底边那句 `4 of 20`）。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn config_position(&self) -> (usize, usize) {
        let items = config::items();
        let at = items
            .iter()
            .position(|item| *item == self.views.config.cursor)
            .map_or(0, |at| at + 1);
        (at, items.len())
    }

    /// 设置栏上光标停在**第几行**（组抬头也占一行）——视口按它算。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn config_line(&self) -> usize {
        config::lines()
            .iter()
            .position(|line| *line == config::Line::Item(self.views.config.cursor))
            .unwrap_or(0)
    }

    /// 详情栏此刻列得出的那几格：光标停在设置栏哪一项、下钻进了哪一块屏幕规格说了算。
    pub fn config_choices(&self) -> Choices {
        config::choices(self, self.views.config.cursor, self.views.config.drill)
    }

    /// 光标那一项是**自由填**的那几项之一吗（`i` 经输入行改的正是它们）。
    /// **开输入行与 `⏎` 那一支问的是这一处**，不各自认一遍。
    pub(super) fn filled_item(&self) -> Option<Field> {
        match self.views.config.cursor {
            Item::Setting(field) if field.shape() == Shape::Text => Some(field),
            _ => None,
        }
    }

    /// `⇥`：在左右那两栏之间切（设计稿 `configKey` 的 `Tab` 那一支）。
    /// **两栏各自的光标都不动**——切走再切回原样。
    ///
    /// **切的是[在哪一栏](Pane)那一维，不问预设栏掀着没有**：设计稿那一支摆在
    /// `S.cfg.presets` 那个分支之前，切回来右边那一栏仍是预设栏（停车场 Q828 的 ②，
    /// 13 那时并成一维、只好让它一个字不动）。
    fn switch_pane(&mut self) {
        if self.views.view != View::Config {
            return;
        }
        self.views.config.pane = match self.views.config.pane {
            Pane::Settings => Pane::Details,
            Pane::Details => Pane::Settings,
        };
    }

    /// 设置栏上 `l`：进详情栏，光标停在**此刻生效的那一格**上
    /// （型号那一项停在当前型号所在的那块屏幕规格上；生效的那一格答不出来就停在头一格）。
    fn enter_details(&mut self) {
        self.views.config.drill = None;
        self.views.config.choice = self.config_choices().lands_on();
        self.views.config.pane = Pane::Details;
    }

    /// 设置栏上 `⏎`：与 `l` 同，**自由填的那几项另外直接开输入行**——少按一下
    /// （设计稿 `configKey` 左栏那一支的 `k === 'Enter'`）。改不动的时候照旧只是进去看。
    fn config_enter(&mut self) {
        match self.filled_item() {
            Some(_) if !self.settings_locked() => self.open_valuing(),
            _ => self.enter_details(),
        }
    }

    /// 详情栏上 `h`／`Esc`：下钻着就**退回屏幕规格那一层**（停回进去时那一块），
    /// 否则回设置栏——两样都**一格不改**（`CONTEXT.md` 的《详情栏》）。
    ///
    /// **预设栏上它把那一栏收起来**（设计稿 `configKey` 预设那一支的 `h`／`Escape`）：
    /// 与再按一次 `p` 同一件事，收完落回设置栏。
    fn config_back(&mut self) {
        if self.views.config.focus() == Focus::Picker {
            self.shut_picker();
            return;
        }
        if self.views.config.focus() != Focus::Details {
            return;
        }
        let Some(panel) = self.views.config.drill.take() else {
            self.views.config.pane = Pane::Settings;
            return;
        };
        // 退回屏幕规格那一层，光标停回进去时那一块上。
        self.views.config.choice = config::panels()
            .iter()
            .position(|(block, _)| *block == panel)
            .unwrap_or(0);
    }

    /// 详情栏上 `l`／`⏎`：**定下停着的那一格**。
    ///
    /// 屏幕规格那一层定不下来——按下去是[下钻](CONTEXT.md)进去看它底下有哪几个型号；
    /// 自由填的那几项按下去开输入行；画质判定参数那一组一格都定不下。
    /// **跑着与等待确认时一个都定不下**（[`Self::settings_locked`]），屏底说设置已锁定。
    fn settle_choice(&mut self, now: Instant) {
        let Item::Setting(field) = self.views.config.cursor else {
            return;
        };
        match self.config_choices() {
            Choices::Panels { .. } => {
                let Some((panel, _)) = config::panels().into_iter().nth(self.views.config.choice)
                else {
                    return;
                };
                self.views.config.drill = Some(panel);
                self.views.config.choice = self.config_choices().lands_on();
            }
            Choices::Models { cells, .. } => {
                if self.refuse_locked(locked_short, now) {
                    return;
                }
                let Some(device) = cells.get(self.views.config.choice) else {
                    return;
                };
                let device = (*device).to_owned();
                self.set_device(Some(device.clone()));
                self.views.config.drill = None;
                self.views.config.pane = Pane::Settings;
                self.views.say(
                    vec![
                        Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                        Segment::plain(format!("设备型号已改为 {device}")),
                        Segment::faint("（之前填的可见灰阶数和画质门槛已清空）"),
                    ],
                    now,
                );
            }
            Choices::Ring { .. } => {
                if self.refuse_locked(locked_long, now) {
                    return;
                }
                self.settle(field, self.views.config.choice);
                self.views.config.pane = Pane::Settings;
                self.views.say(
                    vec![
                        Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                        Segment::plain(format!("{} → {}", field.label(), self.shown(field))),
                    ],
                    now,
                );
            }
            // 自由填的那几项按下去开输入行；改不动的时候一个字都不说——屏上那一句
            // 「[i → 修改]」本来就没摆出来（按键表在只读那几档上不派 `i`）。
            Choices::Filled(_) => {
                if !self.settings_locked() {
                    self.open_valuing();
                }
            }
            Choices::Premise => {}
        }
    }

    /// 改不动的时候屏底说一句、那一下不生效；答「拦下了没有」。
    ///
    /// **拦它的是阶段那一维**（[`Self::settings_locked`]），不是焦点——详情栏照样进得来。
    fn refuse_locked(&mut self, said: impl FnOnce() -> Vec<Segment>, now: Instant) -> bool {
        if !self.settings_locked() {
            return false;
        }
        self.views.say(said(), now);
        true
    }

    /// `p`：**掀开预设栏**（`CONTEXT.md` 的《预设栏》）——替换详情栏，设置栏仍在屏上。
    ///
    /// 列的是**进这一栏那一刻**盘上有的那几份，而盘只有终端层碰得到：那一层读好了交进来
    /// （`super::terminal` 的 `toggle_picker`）。**光标停在原处**（设计稿 `configKey` 的
    /// `p` 那一支不动 `pcursor`），那几份少了就夹回末行那一件上。
    pub(super) fn lift_picker(&mut self, listed: Vec<NamedPreset>) {
        self.views.config.preset_cursor = self.views.config.preset_cursor.min(listed.len());
        self.views.config.listed = listed;
        self.views.config.picker = true;
        self.views.config.pane = Pane::Details;
        self.views.config.armed_delete = None;
    }

    /// 再按一次 `p`（或预设栏上的 `h`／`Esc`）：**收起预设栏**，落回设置栏。
    pub(super) fn shut_picker(&mut self) {
        self.views.config.picker = false;
        self.views.config.pane = Pane::Settings;
        self.views.config.armed_delete = None;
    }

    /// 预设栏上 `⏎`／`l`：**套用光标停着的那一份**；停在末行那一件上时经输入行起名。
    ///
    /// **改不改得动先问**（设计稿 `configKey` 预设那一支：`ro` 那一问摆在最前面）——
    /// 跑着与等待确认时套不下也存不了，屏底说一句设置已锁定。
    fn use_preset(&mut self, now: Instant) {
        if self.refuse_locked(locked_short, now) {
            return;
        }
        let Some(listed) = self.views.config.picked().cloned() else {
            // 末行那一件：**把当前设置保存为预设**，名字经输入行打。
            // 重开一次就把上一次撞名等着的那一下作废：那一问问的是上一次打的名字。
            self.views.config.armed_save = None;
            self.views.input = Some(InputLine::new(Purpose::Preset, ""));
            return;
        };
        let Some(preset) = listed.preset else {
            // 读不懂的那一份套不下来：屏上那一行已经说了它读不懂，这里不另编一句。
            self.views
                .complain(format!("预设「{}」读不懂，套不下来", listed.name), now);
            return;
        };
        self.apply_preset(&listed.name, preset);
        self.views.say(
            vec![
                Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                Segment::plain(format!("已使用预设「{}」", listed.name)),
            ],
            now,
        );
    }

    /// 把设备设置与处理选项**整个换成这一份**（`CONTEXT.md` 的《预设栏》）：
    /// 它没说的那几项回到「没说」，**路径与输出一格不动**。
    ///
    /// **型号也一格不动**：设计稿存与套都跳过它（`configKey` 那一支只收取值环与自由填的
    /// 那几项），与设置栏上行尾那个 `*` 不数型号是同一条（[`config::starred`]）。
    /// 换了型号要走[它自己那一路](Self::set_device)——那一下还要清掉两个标定数。
    pub(super) fn apply_preset(&mut self, name: &str, preset: Preset) {
        self.device.gray_levels = preset.device.gray_levels;
        self.device.threshold = preset.device.threshold;
        self.taste = preset.taste.clone();
        self.views.config.applied = Some(Applied {
            name: name.to_owned(),
            preset,
        });
    }

    /// **存出去的是哪一份**：设备设置与处理选项两组里[预设记得下的那几项](config::stored_fields)。
    ///
    /// 与[套用](Self::apply_preset)对着来：型号不写进去，因此存完再读回来仍是同一份
    /// ——顶上那一条当场就说「与预设一致」。
    pub(super) fn preset_to_store(&self) -> Preset {
        Preset {
            device: crate::preset::DeviceLayer {
                profile: None,
                ..self.device.clone()
            },
            taste: self.taste.clone(),
        }
    }

    /// `dd` 按下去那一下（`CONTEXT.md` 的《预设栏》：删一份按两下）。
    ///
    /// **第一下只把光标停着的那一份闩上**、答 `None`——盘一个字节都不碰，屏上那一栏里问一句；
    /// **第二下**（问的与眼下停着的是同一份时）**答出要删的那一份**，真把它从盘上拿走
    /// 是终端层的事（`super::terminal` 的 `erase_a_preset`）。**两下记在一处**，
    /// 调用方不必再判一遍「这是第几下」——与按停止那两级（[`Self::stop_a_notch`]）同一条分工。
    ///
    /// 停在末行那一件上时两下都答 `None`：那一行不是一份预设，删不掉。
    pub(super) fn ask_then_erase(&mut self) -> Option<String> {
        let name = self.views.config.picked().map(|one| one.name.clone())?;
        if self.views.config.armed_delete.as_deref() == Some(name.as_str()) {
            return Some(name);
        }
        self.views.config.armed_delete = Some(name);
        None
    }

    /// 存那一下撞上了一份**同名的**：把那个名字闩上，屏上那一栏里问一句
    /// （`CONTEXT.md` 的《预设》：盖掉一份同名的要按两下），再按一次 `⏎` 才覆盖。
    ///
    /// **那一问摆在预设栏里、不摆在屏底**：这一刻输入行占着屏底（名字还留在缓冲里等着改），
    /// 而屏底同一刻只摆得下一样——说给屏底就等于一个字都没说。`dd` 那一问摆在同一处。
    pub(super) fn preset_name_is_taken(&mut self, name: &str) {
        self.views.config.armed_save = Some(name.to_owned());
    }

    /// 删掉了一份：从列的那几份里拿走它，**正在用的那一份也删得掉**
    /// （删完就是「未使用预设」）。屏底说一句——那一句只说**无条件成立的那两半**
    /// （`CONTEXT.md` 的《预设》）：那一份的内容没了、撤不回来，文件里其余几份照旧留着。
    pub(super) fn preset_erased(&mut self, name: &str, now: Instant) {
        self.views.config.listed.retain(|one| one.name != name);
        let last = self.views.config.listed.len();
        self.views.config.preset_cursor = self.views.config.preset_cursor.min(last);
        self.views.config.armed_delete = None;
        if self
            .views
            .config
            .applied
            .as_ref()
            .is_some_and(|applied| applied.name == name)
        {
            self.views.config.applied = None;
        }
        self.views.say(
            vec![Segment::new(
                format!("已删除预设「{name}」"),
                Look::tone(Tone::Caution),
            )],
            now,
        );
    }

    /// 存下了一份：**列的那几份跟着多一条**（同名的换掉原处那一条），套着的换成它，屏底说一句。
    pub(super) fn preset_saved(&mut self, name: &str, preset: Preset, now: Instant) {
        let listed = NamedPreset {
            name: name.to_owned(),
            preset: Some(preset.clone()),
        };
        let at = self
            .views
            .config
            .listed
            .iter()
            .position(|one| one.name == name);
        match at {
            Some(at) => self.views.config.listed[at] = listed,
            None => self.views.config.listed.push(listed),
        }
        self.views.config.armed_save = None;
        self.apply_preset(name, preset);
        self.views.say(
            vec![
                Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                Segment::plain(format!("已保存预设「{name}」")),
            ],
            now,
        );
    }

    /// 删掉光标那一条处理路径；光标停到它下一条上，没有下一条就停到「＋ 添加路径」；屏底说一句。
    fn delete_path(&mut self, now: Instant) {
        let Some(at) = self.path_under_cursor() else {
            return;
        };
        let gone = self.scope.paths.remove(at);
        self.views.task.cursor = match self.scope.paths.get(at) {
            Some(next) => Cursor::Path(next.path.clone()),
            None => Cursor::Add,
        };
        self.views.say(
            vec![
                Segment::new("已删除 ", Look::tone(Tone::Caution)),
                Segment::plain(self.home_shown(&gone.path)),
            ],
            now,
        );
    }

    /// 屏上这一条路径怎么写（家目录缩写成 `~`）。家目录由会话入口问一次、摆在这里往下传。
    pub fn home_shown(&self, path: &Path) -> String {
        self.home.abbreviate(path)
    }

    /// 屏上输出目录怎么写：缩写成 `~/…`；没填就说没填。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn output_shown(&self) -> String {
        match &self.scope.out {
            Some(out) => self.home_shown(out),
            None => OUTPUT_UNSET.to_owned(),
        }
    }

    /// 屏底此刻按轻重要摆哪几件（设计稿 `footerHints` 的次序），`?` 恒在末尾。
    /// 每一件的键怎么写、那一句怎么说从按键表取（[`keymap::hints`]），派不出的不摆。
    ///
    /// **收那一趟**：树上光标那一行摆的是 `l → 展开` 还是 `l → 每页结果`、
    /// 还是一件都不摆，末一问要问那一卷此刻怎么样（[`Session::open_want`]），
    /// 而那件事只有那一趟答得出。没有那一趟时（还没开跑）树也不在，一件都不摆。
    pub fn hints(&self, phase: Phase, live: Option<&Live>) -> Vec<Hint> {
        let focus = self.views.focus();
        let mut wants: Vec<Want> = Vec::new();
        match (self.views.view, focus) {
            // 覆盖层掀着时屏底让给它自己的两件（设计稿 `footerHints` 头一支）：`?` 不另摆——它在抬头上。
            (_, Focus::Overlay) => {
                return keymap::hints(
                    phase,
                    focus,
                    &[
                        Want::of(Deed::Down),
                        Want::of(Deed::Up),
                        Want::of(Deed::CloseOverlay),
                    ],
                );
            }
            // 输入行右端那几件（设计稿 `drawFooter` 打字那一支）。**搜索那一行只有两件**：
            // 它不补全（[`Purpose::completes`]），`C-w` 按得动而不摆——右端只摆这一行
            // 此刻最要紧的两件（同一处 `⏎` 换成「跳到结果」）。
            (_, Focus::Input) if self.searching_line() => {
                return keymap::hints(
                    phase,
                    focus,
                    &[
                        Want::saying(Deed::Confirm, "跳到结果"),
                        Want::of(Deed::Cancel),
                    ],
                );
            }
            (_, Focus::Input) => {
                return keymap::hints(
                    phase,
                    focus,
                    &[
                        Want::of(Deed::Complete),
                        Want::of(Deed::DeleteWord),
                        Want::of(Deed::Confirm),
                        Want::of(Deed::Cancel),
                    ],
                );
            }
            (View::Config, Focus::Picker) => wants.extend([
                Want::of(Deed::UsePreset),
                Want::of(Deed::DeletePreset),
                Want::saying(Deed::Presets, "返回"),
                Want::of(Deed::Down),
                Want::of(Deed::Up),
            ]),
            (View::Config, _) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.extend([
                    // 那一句与那个键都随此刻在哪一栏、改不改得动而变，**分在表上**：
                    // 设置栏是 `l → 展开`，详情栏是 `⏎ → 确定`，只读那几档两边都是「查看」。
                    Want::of(Deed::ConfigOpen),
                    Want::saying(Deed::ConfigBack, "返回"),
                    Want::saying(Deed::Presets, "预设"),
                    Want::of(Deed::TaskView),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
            (View::Task, Focus::Pages) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.extend([
                    // **那一句随此刻列的是哪几页换**（表上两行，屏底摆的是**按下去会到的
                    // 那一副**）：只列需留意的页时写「全部页」，反过来写「只看需留意的页」。
                    // **等待确认时这一件根本派不出**（`a` 让给答话，表上那两行只给
                    // 转换中与已结束），屏底因此一个字都不摆——不必在这里另设一道门。
                    Want::saying(Deed::ListAll, self.listing_key_says()),
                    Want::of(Deed::BackToList),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
            (View::Task, _) if phase == Phase::Fresh => wants.extend([
                Want::of(Deed::Preview),
                Want::of(Deed::Convert),
                Want::of(Deed::AddPath),
                Want::of(Deed::TogglePath),
                Want::of(Deed::DeletePath),
                Want::of(Deed::Down),
                Want::of(Deed::Up),
                Want::of(Deed::ConfigView),
            ]),
            (View::Task, _) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.push(Want::of(Deed::NextProblem));
                // **`F` 只在自动滚动暂停着的时候摆**（设计稿 `footerHints` 那一条）：
                // 跟着的时候它一件事都不做，屏上不摆按不动的键。已结束那一档表上
                // 本来就派不出它（[`keymap::TABLE`] 只给转换中与等待确认）。
                if !self.views.task.follow {
                    wants.push(Want::of(Deed::Follow));
                }
                // 光标那一行展得开什么摆在 `]d`／`F` 之后、`/` 之前（设计稿 `footerHints`
                // 那一支的次序，`openHint` 就摆在这一格）。
                wants.extend(self.open_want(live));
                wants.extend([
                    Want::of(Deed::Search),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
        }
        wants.push(Want::of(Deed::Help));
        keymap::hints(phase, focus, &wants)
    }

    /// 屏底那一件 `a` 此刻写的是哪一句：**按下去会到的那一副**。
    /// 两句都在按键表上（[`keymap::TABLE`] 的 `Group::Pages`），这一处只挑哪一句。
    pub(super) fn listing_key_says(&self) -> &'static str {
        let all = self
            .views
            .task
            .pages
            .as_ref()
            .is_some_and(|pages| pages.listing == Listing::All);
        if all {
            "只看需留意的页"
        } else {
            "全部页"
        }
    }

    /// 屏底那一件「展开／每页结果／查看」——**光标那一行展得开什么**
    /// （设计稿 `openHint`；`CONTEXT.md` 的《停得住 / 展得开》）：
    /// 目录行是 `l → 展开`，卷行是 `l → 每页结果`，备注行是 `⏎ → 查看`。
    ///
    /// **卷行展不开就一件都不摆**（屏上不摆按不动的键）：展不展得开只有那一趟答得出，
    /// 判据在 [`VolumeState::opens_the_pages`] 一处——按下去换不换屏读的是同一份。
    fn open_want(&self, live: Option<&Live>) -> Option<Want> {
        match &self.views.task.cursor {
            Cursor::Directory(_) => Some(Want::saying(Deed::Open, "展开")),
            Cursor::Volume(root) => self
                .volume_state(live, root)
                .opens_the_pages()
                .then(|| Want::saying(Deed::Open, "每页结果")),
            Cursor::Note(_) => Some(Want::saying(Deed::Open, "查看")),
            Cursor::Output | Cursor::Path(_) | Cursor::Add => None,
        }
    }

    /// 这个卷根在那一趟上此刻怎么样。清单上找不到它、或者那一趟还没起来，就是**等待中**
    /// ——那时它一页结果都没有，与还没轮到一个待遇。
    ///
    /// **卷根换回清单序号只有一处**（[`tree::Tree::index_of`]）：状态按序号记
    /// （[`Live::states`]），而屏上与光标记着的都是卷根。
    pub fn volume_state(&self, live: Option<&Live>, root: &Path) -> VolumeState {
        self.views
            .task
            .tree
            .index_of(root)
            .and_then(|at| live?.states().get(at).copied())
            .unwrap_or(VolumeState::Queued)
    }

    /// 阶段那一维派的几件：等待确认时答话那三个与 `v`，跑着时停止，结束了再来一趟。
    fn stage_wants(&self, phase: Phase, focus: Focus) -> Vec<Want> {
        match phase {
            Phase::Deciding => {
                let mut wants = vec![
                    Want::of(Deed::Write),
                    Want::of(Deed::WriteAll),
                    Want::of(Deed::End),
                ];
                // `v` 只在卷列表上派得出：每页结果已经在那一卷里了，配置视图不是它的去处
                // （停车场 Q778）。
                if focus != Focus::Pages && self.views.view != View::Config {
                    wants.push(Want::of(Deed::ViewPages));
                }
                wants
            }
            Phase::Surveying | Phase::Running => vec![Want::saying(
                Deed::Stop,
                if Phase::latched(self.stage()) {
                    "再按一次立即停"
                } else {
                    "停止"
                },
            )],
            Phase::Ended if self.views.view == View::Task => {
                vec![Want::of(Deed::Preview), Want::of(Deed::Convert)]
            }
            Phase::Ended | Phase::Fresh => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::home::Home;
    use crate::session::keymap::TABLE;
    use crate::session::state::Stage;
    use crate::session::viewport::Viewport;

    fn key(letter: char) -> Input {
        Input::Key(Key::Char(letter))
    }

    /// 三条处理路径、一个输出目录，光标停在头一条上。
    fn three_paths() -> Session {
        let mut session = Session::new();
        session.home = Home::at("/home/me");
        session.scope.out = Some(PathBuf::from("/home/me/转好的"));
        for (path, on) in [
            ("/home/me/漫画库", true),
            ("/home/me/漫画库/银河", true),
            ("/home/me/下载/第01卷.cbz", false),
        ] {
            session.scope.paths.push(NamedPath {
                path: PathBuf::from(path),
                on,
            });
        }
        session.views.task.cursor = Cursor::Path(PathBuf::from("/home/me/漫画库"));
        session
    }

    /// 清点清单上的一卷（步数与源页数这一层不看）。
    fn listed(root: &str) -> tonefit::SurveyedVolume {
        tonefit::SurveyedVolume {
            root: PathBuf::from(root),
            steps: 3,
            source_pages: 1,
        }
    }

    /// 一趟跑着、清点完了的会话：一条处理路径 `/库` 摊成两个目录（分区），
    /// 甲底下两卷、乙底下一卷。**没有那一趟**——本组用例问的都是特性外面那几件
    /// （光标、暂停、搜索的落点），一卷此刻怎么样不在里面。
    fn a_running_tree() -> Session {
        let mut session = Session::new();
        session.home = Home::at("/home/me");
        session.scope.paths.push(NamedPath {
            path: PathBuf::from("/库"),
            on: true,
        });
        session.run_started();
        session.views.task.tree = tree::Tree::of(
            &session.scope.paths,
            &[
                listed("/库/甲/第01卷"),
                listed("/库/甲/第02卷"),
                listed("/库/乙/第01卷"),
            ],
            &[],
            &[],
        );
        session.views.task.surveyed = true;
        session
    }

    /// 屏底此刻那一句连起来。
    fn said(session: &Session, now: Instant) -> String {
        session
            .views
            .reply(now)
            .expect("屏底说了一句")
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    /// **自动滚动暂停时只记光标，视口照旧由光标算**（票面第三条验收）。
    ///
    /// 两问。① 挪一下光标就暂停，屏底说「已暂停自动滚动 ⋅ 按 F 恢复」，`F` 交回、
    /// 屏底换成那一句。② **暂停这件事一个滚动量都不记**：把另一份会话的光标直接摆到
    /// 同一行、自动滚动也扳掉，两份任务视图**一个字节都不差**——中间没有第二个数
    /// 躲在别处。屏上从第几行画起因此只是光标的函数（`CONTEXT.md` 的《视口》）。
    #[test]
    fn pausing_follow_only_records_the_cursor_and_the_viewport_comes_from_it() {
        let mut session = a_running_tree();
        let now = Instant::now();
        assert!(session.views.task.follow, "开跑那一刻跟着");
        let head = session.cursor_line();
        session.perform(Deed::Down, now);
        assert!(!session.views.task.follow, "挪一下光标就暂停");
        assert_eq!(said(&session, now), "已暂停自动滚动 ⋅ 按 F 恢复");
        assert!(session.cursor_line() > head, "光标真挪了一行");

        let mut twin = a_running_tree();
        twin.views.task.follow = false;
        twin.views.task.cursor = session.views.task.cursor.clone();
        assert_eq!(
            twin.views.task, session.views.task,
            "暂停记下来的只有光标，没有第二个数"
        );
        let rows = session.lines().len();
        let from = |session: &Session| Viewport::with_margin(rows, 2, session.cursor_line()).from();
        assert_eq!(from(&twin), from(&session), "视口是光标算出来的");

        session.perform(Deed::Follow, now);
        assert!(session.views.task.follow, "`F` 交回");
        assert_eq!(said(&session, now), "自动滚动：跟到正在处理的卷");
    }

    /// **挪不动的那一下照样暂停自动滚动**（设计稿 `listGo` 不问光标有没有真挪）：
    /// 跟着的那一卷正停在列表最后一行上时按 `j`——光标挪不动，而**自动滚动必须停**，
    /// 不然下一帧 [`Session::watch_the_run`] 就把人拽回去，而躲开那一下正是按这个键的用意。
    #[test]
    fn a_keypress_that_cannot_move_the_cursor_still_pauses_following() {
        let mut session = a_running_tree();
        let now = Instant::now();
        // 光标停在最后一条停得住的行上（这一景两条：两个目录行）。
        session.perform(Deed::Bottom, now);
        session.views.task.follow = true;
        let last = session.views.task.cursor.clone();
        session.perform(Deed::Down, now);
        assert_eq!(session.views.task.cursor, last, "到底了，光标挪不动");
        assert!(!session.views.task.follow, "挪不动也暂停");
        assert_eq!(said(&session, now), "已暂停自动滚动 ⋅ 按 F 恢复");
    }

    /// **每次开跑扳回跟着、上一趟搜的那一句清掉**（票面第一条末一句；设计稿 `startRun`）。
    #[test]
    fn starting_a_run_follows_again_and_forgets_the_last_query() {
        let mut session = a_running_tree();
        let now = Instant::now();
        session.views.task.search = Some("甲".to_owned());
        session.perform(Deed::Down, now);
        assert!(!session.views.task.follow);
        assert_eq!(session.views.searching(), Some("甲"));

        session.run_started();
        assert!(session.views.task.follow, "开跑扳回跟着");
        assert_eq!(session.views.searching(), None, "上一趟搜的那一句清掉了");
    }

    /// **清点中挪光标不说「已暂停」**：那一档树还没有、列表列的仍是处理路径，
    /// 屏上一个「正在处理的那一卷」都指不出来（[`Session::following_matters`]）。
    #[test]
    fn moving_the_cursor_while_surveying_says_nothing_about_following() {
        let mut session = three_paths();
        session.run_started();
        let now = Instant::now();
        assert!(!session.views.task.surveyed, "清点中：开工那一条还没到");
        session.perform(Deed::Down, now);
        assert!(
            session.views.reply(now).is_none(),
            "清点中挪光标一句话都不说"
        );
    }

    /// **`/` 搜卷名或目录名，`⏎` 跳到第一个，`n`／`N` 在结果之间跳**（票面第三条）。
    ///
    /// 四问：① 打着字的时候搜的就是缓冲（`searching` 一处答完）；② 目录名自己装着这一句时
    /// **它底下那几卷不再各算一个落点**（搜「甲」只有一个结果，那个目录行）；
    /// ③ 跳到一卷上会把它那个目录**展开**；④ `Esc` 连那一句一起丢，之后 `n`／`N`
    /// 一件事都不做。
    #[test]
    fn search_lands_on_directories_and_volumes_and_opens_what_it_needs_to() {
        let mut session = a_running_tree();
        let now = Instant::now();
        session.perform(Deed::Search, now);
        assert_eq!(
            session.views.searching(),
            None,
            "刚开那一行还没打字：空串不算在搜"
        );
        assert!(session.searching_line(), "而那一行确实开着");
        for glyph in "甲".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        assert_eq!(session.views.searching(), Some("甲"), "搜的就是缓冲");

        session.confirm_search(None, now);
        assert_eq!(
            session.views.task.cursor,
            Cursor::Directory(PathBuf::from("/库/甲"))
        );
        assert_eq!(said(&session, now), "搜索结果 1/1", "两卷不各算一个落点");
        assert!(!session.views.task.follow, "跳过去之后不再跟");
        assert!(
            session.views.task.expanded.is_empty(),
            "停在目录行上不展开它"
        );

        // 卷名那一路：`甲/第02卷` 命中一卷，跳过去把它那个目录展开。
        session.views.task.search = Some("第02".to_owned());
        assert!(
            session.jump(Deed::SearchNext, None, now),
            "`n` 归跳转那一支"
        );
        assert_eq!(
            session.views.task.cursor,
            Cursor::Volume(PathBuf::from("/库/甲/第02卷"))
        );
        assert!(
            session
                .views
                .task
                .expanded
                .contains(&PathBuf::from("/库/甲")),
            "收着的目录自动展开到那一卷"
        );

        // `Esc` 丢掉那一句：之后 `n`／`N` 一件事都不做（没有那一句可跳）。
        session.perform(Deed::ClearSearch, now);
        assert_eq!(session.views.searching(), None);
        let before = session.views.task.cursor.clone();
        assert!(session.jump(Deed::SearchPrev, None, now));
        assert_eq!(session.views.task.cursor, before, "没搜过就不跳");
    }

    /// **一个都没找到时各说一句**（票面第三条）：搜索那一路报这一句没命中，
    /// `]d` 那一路报的是好消息。两句都不挪光标、不碰自动滚动。
    #[test]
    fn a_hunt_that_lands_nowhere_says_so_and_leaves_the_cursor_alone() {
        let mut session = a_running_tree();
        let now = Instant::now();
        let before = session.views.task.cursor.clone();

        session.views.task.search = Some("不存在".to_owned());
        session.jump(Deed::SearchNext, None, now);
        assert_eq!(said(&session, now), "没有找到和「不存在」相关的卷或文件夹");

        // 没有那一趟就问不出哪几卷出了事：这一景因此一处问题都没有。
        session.jump(Deed::NextProblem, None, now);
        assert_eq!(said(&session, now), "✓ 目前没有问题");
        assert_eq!(session.views.task.cursor, before, "一句话，光标不动");
        assert!(session.views.task.follow, "没跳成就不暂停");
    }

    /// **搜索那一行右端只有两件**（票面第三条）：它不补全，`C-w` 按得动而不摆；
    /// `⏎` 那一句换成「跳到结果」。别的输入行照旧四件。
    #[test]
    fn the_search_line_offers_only_jump_and_cancel() {
        let mut session = a_running_tree();
        let now = Instant::now();
        session.perform(Deed::Search, now);
        let spelt = |session: &Session, phase| {
            session
                .hints(phase, None)
                .iter()
                .map(|hint| format!("{} → {}", hint.spelt(), hint.what))
                .collect::<Vec<String>>()
        };
        assert_eq!(
            spelt(&session, Phase::Running),
            ["⏎ → 跳到结果", "Esc → 取消"]
        );
        // 对照：添加路径那一行上仍是四件（`Tab` 只在还没开始那一档派得出）。
        session.perform(Deed::Cancel, now);
        let mut fresh = three_paths();
        fresh.open_adding();
        assert_eq!(
            spelt(&fresh, Phase::Fresh),
            ["Tab → 补全", "C-w → 删一段", "⏎ → 确定", "Esc → 取消"]
        );
    }

    /// **`F` 只在自动滚动暂停着的时候摆上屏底**（票面第一条那一句「屏底提 `F`」）：
    /// 跟着的时候它一件事都不做，屏上不摆按不动的键。
    #[test]
    fn the_footer_offers_f_only_while_following_is_paused() {
        let mut session = a_running_tree();
        let listed = |session: &Session| {
            session
                .hints(Phase::Running, None)
                .iter()
                .map(|hint| hint.spelt())
                .collect::<Vec<String>>()
        };
        assert!(
            !listed(&session).contains(&"F".to_owned()),
            "跟着的时候不摆它"
        );
        session.views.task.follow = false;
        assert!(listed(&session).contains(&"F".to_owned()), "暂停了才摆");
    }

    /// **屏底每一件都出自那张按键表**（票面第三条）：键的写法与那一句合起来是表上的一行，
    /// 而且那一行在此刻的阶段与块上派得出。还没开始、跑着、等待确认、已结束各问一遍。
    #[test]
    fn every_hint_on_the_footer_is_a_row_of_the_key_table() {
        let mut session = three_paths();
        for (phase, stage) in [
            (Phase::Fresh, Stage::Fresh),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
            (Phase::Ended, Stage::Ended),
        ] {
            session.set_stage(stage);
            for view in [View::Task, View::Config] {
                session.views.view = view;
                let focus = session.views.focus();
                let hints = session.hints(phase, None);
                assert!(!hints.is_empty(), "{phase:?} 的 {view:?} 上屏底空着");
                for hint in &hints {
                    for spelt in &hint.keys {
                        assert!(
                            TABLE.iter().any(|row| row.spelt == *spelt
                                && row.short == hint.what
                                && row.applies(phase, focus)),
                            "{phase:?} 的 {view:?} 上屏底那件「{spelt} → {}」不出自按键表",
                            hint.what
                        );
                    }
                }
                assert_eq!(
                    hints.last().map(|hint| hint.what),
                    Some("全部按键"),
                    "`?` 恒在末尾"
                );
            }
        }
    }

    /// **半屏与一屏那四个键各挪几行**（`session-redesign/10` 票面滚动那几串）：
    /// 一屏是[窗口高减十二](Window::page)、半屏是它的一半，到顶到底为止；
    /// 不是这四件就一件都不做、交回 `false`。
    ///
    /// 这一条在 `tui` 特性**外面**跑：挪几行是纯算术，与画不画得出来无关。
    #[test]
    fn half_a_screen_is_half_of_the_window_minus_twelve() {
        let sized = |cols, rows| Window { cols, rows };
        assert_eq!(sized(120, 36).page(), 24);
        assert_eq!(sized(80, 24).page(), 12);
        assert_eq!(sized(60, 14).page(), 4, "再矮也是四行");

        let mut session = three_paths();
        session.scope.paths.extend((0..60).map(|at| NamedPath {
            path: PathBuf::from(format!("/home/me/卷{at}")),
            on: true,
        }));
        session.views.task.cursor = Cursor::Output;
        let window = sized(120, 36);
        // 停得住的共 65 行（输出目录 · 63 条处理路径 · 「＋ 添加路径」），光标停在头一行。
        assert_eq!(session.cursor_position(), (1, 65));
        let now = Instant::now();
        let walk = |session: &mut Session, deed| {
            assert!(session.scroll_list(deed, window, now), "{deed:?} 该归它管");
            session.cursor_position().0
        };
        assert_eq!(walk(&mut session, Deed::HalfDown), 1 + 12);
        assert_eq!(walk(&mut session, Deed::PageDown), 1 + 12 + 24);
        assert_eq!(walk(&mut session, Deed::HalfUp), 1 + 24);
        assert_eq!(walk(&mut session, Deed::PageUp), 1, "到顶为止");
        assert!(
            !session.scroll_list(Deed::Down, window, now),
            "上下一行不归它（那一件状态机自己认）"
        );
    }

    /// 还没开始时屏底那几件照设计稿的轻重次序。
    #[test]
    fn before_the_run_the_footer_lists_the_eight_things_in_order() {
        let session = three_paths();
        let said: Vec<String> = session
            .hints(Phase::Fresh, None)
            .iter()
            .map(|hint| format!("{} → {}", hint.spelt(), hint.what))
            .collect();
        assert_eq!(
            said,
            [
                "t → 预览",
                "x → 转换",
                "o → 添加路径",
                "空格 → 勾选",
                "dd → 删除",
                "j/k → 选择",
                "2 → 配置",
                "? → 全部按键",
            ]
        );
    }

    /// `j`／`k` 在停得住的行上挪，标签那一行跳过，两头到底为止；`gg`／`G` 到顶到底。
    /// 问的是框底边那句「第几条 of 几条」，不问光标那个枚举。
    #[test]
    fn the_cursor_moves_over_the_rows_that_can_be_stopped_on() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.cursor_position(), (2, 5));
        session.perform(Deed::Up, now);
        assert_eq!(session.cursor_position(), (1, 5));
        session.perform(Deed::Up, now);
        assert_eq!(session.cursor_position(), (1, 5), "到顶为止");
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        assert_eq!(session.cursor_position(), (3, 5), "标签那一行跳过");
        for _ in 0..10 {
            session.perform(Deed::Down, now);
        }
        assert_eq!(session.cursor_position(), (5, 5), "到底为止");
        session.perform(Deed::Top, now);
        assert_eq!(session.cursor_position(), (1, 5));
        session.perform(Deed::Bottom, now);
        assert_eq!(session.cursor_position(), (5, 5));
        // 光标记的是身份：删掉光标前面那一条，它仍停在同一条路径上（位置往前挪一位）。
        session.perform(Deed::Top, now);
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        session.scope.paths.remove(0);
        assert_eq!(session.cursor_position(), (2, 4));
    }

    /// 空格勾选、再按一次取消；被包含按路径前缀认，勾掉外层那一条里层就不算被包含。
    #[test]
    fn space_toggles_the_path_and_nesting_follows_the_checked_outer_folder() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.checked_paths(), (2, 1));
        assert_eq!(session.nested_in(1), Some(Path::new("/home/me/漫画库")));
        assert_eq!(session.nested_in(0), None);
        session.perform(Deed::TogglePath, now);
        assert!(!session.scope.paths[0].on);
        assert_eq!(session.checked_paths(), (1, 2));
        assert_eq!(session.nested_in(1), None, "外层没勾就不算被包含");
        session.perform(Deed::TogglePath, now);
        assert!(session.scope.paths[0].on);
    }

    /// `dd` 删掉光标那一条：光标停到下一条上，屏底说一句、路径缩写成 `~`；删末一条停到「＋ 添加路径」。
    #[test]
    fn dd_deletes_the_path_under_the_cursor_and_says_so() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.deed_of(key('d'), Phase::Fresh, now), None);
        assert_eq!(session.views.pending(now), Some('d'), "前半截待着");
        assert_eq!(
            session.deed_of(key('d'), Phase::Fresh, now),
            Some(Deed::DeletePath)
        );
        assert_eq!(session.views.pending(now), None);
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.scope.paths.len(), 2);
        assert_eq!(session.cursor_position(), (2, 4), "停到下一条上");
        let reply = session.views.reply(now).expect("屏底说了一句");
        assert_eq!(reply[0].text, "已删除 ");
        assert_eq!(reply[1].text, "~/漫画库");
        assert_eq!(session.views.reply(now + REPLY_LINGERS), None, "到点退回");
        // 删末一条：光标掉到「＋ 添加路径」上，那正是接着要做的事。
        session.perform(Deed::Bottom, now);
        session.perform(Deed::Up, now);
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.cursor_position(), (3, 3));
        // 光标不在处理路径上时 `dd` 什么都不删。
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.scope.paths.len(), 1);
    }

    /// 连击键的前半截过了 [`COMBO_WAITS`] 作废；合不上的后半截当一个单独的键认。
    #[test]
    fn a_pending_prefix_expires_and_an_unmatched_second_key_stands_alone() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.deed_of(key('d'), Phase::Fresh, now), None);
        assert_eq!(
            session
                .views
                .pending(now + COMBO_WAITS + Duration::from_millis(1)),
            None
        );
        assert_eq!(
            session.deed_of(key('d'), Phase::Fresh, now + COMBO_WAITS * 2),
            None,
            "过期之后这一下又是前半截"
        );
        assert_eq!(
            session.deed_of(key('j'), Phase::Fresh, now + COMBO_WAITS * 2),
            Some(Deed::Down),
            "`dj` 合不上，`j` 自己算"
        );
        assert_eq!(session.deed_of(key('g'), Phase::Fresh, now), None);
        assert_eq!(
            session.deed_of(key('t'), Phase::Fresh, now),
            Some(Deed::NextView)
        );
    }

    /// **退出规则**（`CONTEXT.md` 的《退出会话》）：`q` 只在还没开始与结束了时退出，跑着与等待确认时
    /// 屏底说先按 `s` 或按 `C-c`；`C-c` 任何时候都退；`Esc` 从不退出。
    #[test]
    fn q_quits_only_before_and_after_a_run_ctrl_c_always_and_esc_never() {
        let mut session = three_paths();
        let now = Instant::now();
        let q = key('q');
        let esc = Input::Key(Key::Esc);
        let interrupt = Input::Key(Key::Interrupt);
        for (phase, stage) in [(Phase::Fresh, Stage::Fresh), (Phase::Ended, Stage::Ended)] {
            session.set_stage(stage);
            let deed = session.deed_of(q, phase, now).expect("q 派得出");
            assert_eq!(session.perform(deed, now), Exit::Leave, "{phase:?}");
        }
        for (phase, stage) in [
            (
                Phase::Surveying,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
        ] {
            session.set_stage(stage);
            let deed = session.deed_of(q, phase, now).expect("q 派得出");
            assert_eq!(session.perform(deed, now), Exit::Stay, "{phase:?}");
            let reply = session.views.reply(now).expect("屏底说为什么不退");
            assert!(reply[1].text.contains("先按 s 停止，或按 C-c"), "{reply:?}");
        }
        for (phase, stage) in [
            (Phase::Fresh, Stage::Fresh),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
            (Phase::Ended, Stage::Ended),
        ] {
            session.set_stage(stage);
            let deed = session.deed_of(interrupt, phase, now).expect("C-c 派得出");
            assert_eq!(session.perform(deed, now), Exit::Leave, "{phase:?}");
            match session.deed_of(esc, phase, now) {
                None => {}
                Some(deed) => assert_eq!(session.perform(deed, now), Exit::Stay, "{phase:?}"),
            }
        }
    }

    /// `1`／`2` 换视图，`gt` 轮换。问的是屏底：在任务视图上摆着 `2 → 配置`，
    /// 切过去之后摆着 `1 → 任务`，切回来原样。
    #[test]
    fn the_two_views_switch_and_the_footer_follows() {
        let mut session = three_paths();
        let now = Instant::now();
        let footer = |session: &Session| -> Vec<String> {
            session
                .hints(Phase::Fresh, None)
                .iter()
                .map(|hint| format!("{} → {}", hint.spelt(), hint.what))
                .collect()
        };
        let task = footer(&session);
        assert!(task.contains(&"2 → 配置".to_owned()), "{task:?}");
        session.perform(Deed::ConfigView, now);
        let config = footer(&session);
        assert!(config.contains(&"1 → 任务".to_owned()), "{config:?}");
        assert!(!config.contains(&"2 → 配置".to_owned()), "{config:?}");
        session.perform(Deed::NextView, now);
        assert_eq!(footer(&session), task, "轮换回来原样");
        session.perform(Deed::PrevView, now);
        assert_eq!(footer(&session), config);
        session.perform(Deed::TaskView, now);
        assert_eq!(footer(&session), task);
    }

    /// 改了几项从套着的预设算：型号不算，取值环与自由填的项算。
    #[test]
    fn changed_items_are_counted_against_the_applied_preset_without_the_model() {
        let mut session = three_paths();
        assert_eq!(session.changed_from_preset(), 0, "没套预设");
        session.views.config.applied = Some(Applied {
            name: "漫画".to_owned(),
            preset: Preset::default(),
        });
        assert_eq!(session.changed_from_preset(), 0);
        session.device.profile = Some("kobo-libra-2".to_owned());
        assert_eq!(session.changed_from_preset(), 0, "型号不算");
        session.taste.fit = Some(tonefit::FitMode::Inside);
        session.taste.dither = Some(tonefit::Dither::FloydSteinberg);
        session.device.gray_levels = Some(12);
        assert_eq!(session.changed_from_preset(), 3);
    }

    // ───────────────────────── 每页结果（11） ─────────────────────────

    /// 那一卷此刻在每页结果里：光标停在第几页由调用方给。
    fn opened(session: &mut Session, at: usize) {
        session.views.task.cursor = Cursor::Volume(PathBuf::from("/库/甲/第01卷"));
        session.views.task.pages = Some(Pages {
            volume: PathBuf::from("/库/甲/第01卷"),
            at,
            listing: Listing::Notable,
        });
    }

    /// 夹具那一卷与这一趟那块面板：八页，要留意的六页（另两页跟着卷级档位走）。
    fn a_volume_of_every_kind() -> (VolumeReport, Panel) {
        let report = super::super::live::fixture::a_page_of_every_kind("卷二");
        let panel = super::super::live::fixture::request(tonefit::Mode::DryRun)
            .profile
            .panel();
        (report, panel)
    }

    /// **进了一卷，屏上那一块就换了**（`CONTEXT.md` 的《每页结果》：换掉卷列表）：
    /// 按键表查的那一块跟着换，而**「此刻在哪一块」只有[进了哪一卷](TaskView::pages)
    /// 那一格**——两格记同一件事就会有一格忘了跟着换。
    ///
    /// `h` 回去时**卷列表的光标一格没动**：那一行本来就停在光标底下，
    /// 「回到原处」因此不必记第二个位置。
    #[test]
    fn entering_a_volume_switches_the_block_and_h_comes_back_to_the_same_row() {
        let mut session = a_running_tree();
        session.views.task.cursor = Cursor::Volume(PathBuf::from("/库/甲/第02卷"));
        assert_eq!(session.views.block(), Focus::VolumeList);
        let row = session.views.task.cursor.clone();

        session.views.task.pages = Some(Pages::of(PathBuf::from("/库/甲/第02卷")));
        assert_eq!(session.views.block(), Focus::Pages, "换到了每页结果那一块");
        assert_eq!(session.views.task.focus(), session.views.block());

        session.perform(Deed::BackToList, Instant::now());
        assert!(session.views.task.pages.is_none(), "这一副收掉了");
        assert_eq!(session.views.block(), Focus::VolumeList);
        assert_eq!(session.views.task.cursor, row, "回到原处：那一行一格没动");
    }

    /// **`a` 两档来回，换过之后光标退回头一页**（spec 的《每页结果》：默认只列需留意的页，
    /// `a` 切换）——换的是列哪几页，上一副的第几页在新的一副上不是同一页。
    ///
    /// 屏底那一件跟着换口：它写的是**按下去会到的那一副**。
    #[test]
    fn a_flips_the_listing_and_puts_the_cursor_back_on_the_first_page() {
        let mut session = a_running_tree();
        let now = Instant::now();
        opened(&mut session, 5);
        assert_eq!(session.listing_key_says(), "全部页");

        session.perform(Deed::ListAll, now);
        let pages = session.views.task.pages.as_ref().expect("还在每页结果里");
        assert_eq!(pages.listing, Listing::All);
        assert_eq!(pages.at, 0, "换过列法，光标退回头一页");
        assert_eq!(session.listing_key_says(), "只看需留意的页");

        session.perform(Deed::ListAll, now);
        let pages = session.views.task.pages.as_ref().expect("还在每页结果里");
        assert_eq!(pages.listing, Listing::Notable, "两档来回");
    }

    /// **默认只列需留意的页，`a` 列全部页**（票面第三条）：判在
    /// [`render::notable`] 一处，与卷行行尾那几个数、与命令行印出去的那一份同一份判定。
    /// 夹具那一卷八页，要留意的六页。
    #[test]
    fn the_pages_pane_lists_the_pages_that_matter_until_a_shows_them_all() {
        let (report, panel) = a_volume_of_every_kind();
        let mut pages = Pages::of(report.volume.clone());
        assert_eq!(Pages::notable_count(&report, panel), 6);
        assert_eq!(pages.listed(&report, panel).len(), 6, "默认只列需留意的页");
        pages.listing = Listing::All;
        assert_eq!(pages.listed(&report, panel), (0..8).collect::<Vec<_>>());
    }

    /// **跳过的卷进得来，一页结果都没有**（`CONTEXT.md` 的《停得住 / 展得开》：
    /// 跳过的也算展得开）：它这一趟一页都没重新分析，两档列出来都是空的——
    /// 屏上那时给的是一句话，不是一张空表（`super::super::shell::pages`）。
    #[test]
    fn a_skipped_volume_opens_with_no_pages_at_all() {
        let panel = a_volume_of_every_kind().1;
        let report = super::super::live::fixture::skipped_volume("卷三", 190);
        let mut pages = Pages::of(report.volume.clone());
        assert!(pages.listed(&report, panel).is_empty());
        pages.listing = Listing::All;
        assert!(pages.listed(&report, panel).is_empty(), "全部页那一档也空");
        assert_eq!(pages.settled(0), 0, "一页都没有时光标停在 0");
    }

    /// **光标越界就近收到最后一页上**（[`Pages::at`]）：这一副列着几页要那一卷的报告，
    /// 挪光标那一下收不到上界，因此这一处每一下把它收回来。
    #[test]
    fn the_page_cursor_never_points_past_the_last_page() {
        let mut session = a_running_tree();
        let now = Instant::now();
        opened(&mut session, 0);
        session.perform(Deed::Bottom, now);
        let pages = session.views.task.pages.as_ref().expect("还在每页结果里");
        assert_eq!(pages.settled(6), 5, "`G` 落在最后一页上");
        assert_eq!(pages.settled(1), 0);
        assert_eq!(Pages::of(PathBuf::new()).settled(6), 0);
    }

    /// **每页结果上挪光标不碰卷列表那一头**：卷列表的光标一格不动
    /// （`h` 要回到那一行上），自动滚动那一格也不扳——进了一卷就不在跟着的那张列表上了。
    #[test]
    fn moving_in_the_pages_leaves_the_volume_list_and_following_alone() {
        let mut session = a_running_tree();
        let now = Instant::now();
        opened(&mut session, 0);
        let row = session.views.task.cursor.clone();
        for _ in 0..3 {
            session.perform(Deed::Down, now);
        }
        session.perform(Deed::Up, now);
        let pages = session.views.task.pages.as_ref().expect("还在每页结果里");
        assert_eq!(pages.at, 2);
        assert_eq!(session.views.task.cursor, row, "卷列表的光标一格没动");
        assert!(
            session.views.task.follow,
            "不在那张列表上，自动滚动照旧跟着"
        );
        assert!(session.views.reply(now).is_none(), "屏底一句话都没说");
    }

    /// **停得住与展得开是两件事**（`CONTEXT.md` 的《停得住 / 展得开》；停车场 Q713）。
    ///
    /// 卷行**一律停得住**——光标停得上去，与那一卷此刻怎么样无关（树上那一行
    /// 压根不问那一趟）；**展得开的只有四种**：收摊了的三种（做完 · 进了隔离 · 跳过）
    /// 与确认点上攒着的那一份。没做成的、被立即停止掉的、还在处理的、还没轮到的
    /// 四种停得住、展不开——屏底那一件 `l` 因此一件都不摆。
    #[test]
    fn every_volume_row_stops_but_only_four_kinds_open_the_pages() {
        let session = a_running_tree();
        let tree = &session.views.task.tree;
        for row in tree.every_row() {
            let tree::Row::Volume { at, .. } = row else {
                continue;
            };
            assert_eq!(
                row.stop(tree),
                Some(Cursor::Volume(tree.roots[at].clone())),
                "卷行停不住了"
            );
        }
        let opens = |state: VolumeState| state.opens_the_pages();
        assert!(opens(VolumeState::Done));
        assert!(opens(VolumeState::Isolated));
        assert!(opens(VolumeState::Skipped));
        assert!(opens(VolumeState::Deciding));
        assert!(!opens(VolumeState::Failed), "没做成的没有每页结果");
        assert!(!opens(VolumeState::Aborted), "被立即停止掉的没有保存");
        assert!(!opens(VolumeState::Running { pass: None }), "还在处理");
        assert!(!opens(VolumeState::Queued), "还没轮到");
    }

    /// 摆一份掀着预设栏的会话：列的是两份，光标停在头一份上。
    fn with_a_picker() -> Session {
        let mut session = three_paths();
        session.views.view = View::Config;
        session.lift_picker(vec![
            NamedPreset {
                name: "漫画".to_owned(),
                preset: Some(Preset::default()),
            },
            NamedPreset {
                name: "画集".to_owned(),
                preset: Some(Preset {
                    device: crate::preset::DeviceLayer::default(),
                    taste: crate::preset::TasteLayer {
                        fit: Some(tonefit::FitMode::Inside),
                        crop: Some(false),
                        ..crate::preset::TasteLayer::default()
                    },
                }),
            },
        ]);
        session
    }

    /// **「掀着预设栏」与「在哪一栏」是两维**（停车场 Q828 的 ②）：`⇥` 切的是底下那一维
    /// ——切到设置栏再切回来，右边那一栏**仍是预设栏**；焦点从两维推出来，只有一处答案。
    #[test]
    fn the_picker_and_the_pane_are_two_dimensions_that_do_not_replace_each_other() {
        let mut session = with_a_picker();
        let now = Instant::now();
        assert_eq!(session.views.focus(), Focus::Picker);
        session.perform(Deed::NextBlock, now);
        assert_eq!(session.views.focus(), Focus::Settings, "切到底下那一维");
        assert!(session.views.config.picker, "预设栏没被切没");
        session.perform(Deed::NextBlock, now);
        assert_eq!(session.views.focus(), Focus::Picker, "切回来仍是预设栏");
        // `h` 把这一栏收起来，落回设置栏；收起来之后 `⇥` 才在设置栏与详情栏之间切。
        session.perform(Deed::ConfigBack, now);
        assert_eq!(session.views.focus(), Focus::Settings);
        assert!(!session.views.config.picker);
        session.perform(Deed::NextBlock, now);
        assert_eq!(session.views.focus(), Focus::Details);
    }

    /// **预设栏上挪光标**：那几份加末行那一件；`G` 到末行、`gg` 回头一份，
    /// **挪一格就把 `dd` 等着的那一下作废**。
    #[test]
    fn the_picker_cursor_runs_over_the_presets_and_the_save_row() {
        let mut session = with_a_picker();
        let now = Instant::now();
        assert_eq!(session.views.config.preset_stops(), 3);
        session.views.config.armed_delete = Some("漫画".to_owned());
        session.perform(Deed::Down, now);
        assert_eq!(session.views.config.preset_cursor, 1);
        assert_eq!(session.views.config.armed_delete, None, "挪一格就作废");
        session.perform(Deed::Bottom, now);
        assert_eq!(session.views.config.preset_cursor, 2, "末行那一件停得住");
        assert!(
            session.views.config.picked().is_none(),
            "那一行不是一份预设"
        );
        session.perform(Deed::Down, now);
        assert_eq!(session.views.config.preset_cursor, 2, "末行之后停下来");
        session.perform(Deed::Top, now);
        assert_eq!(session.views.config.preset_cursor, 0);
    }

    /// **末行那一件按下去开输入行**，而**重开一次就把上一次撞名等着的那一下作废**
    /// （`ConfigView::armed_save`）：那一问问的是上一次打的名字。
    #[test]
    fn the_save_row_opens_the_input_line_and_forgets_the_name_it_asked_about() {
        let mut session = with_a_picker();
        let now = Instant::now();
        session.views.config.armed_save = Some("漫画".to_owned());
        session.perform(Deed::Bottom, now);
        session.perform(Deed::UsePreset, now);
        let line = session.views.input.as_ref().expect("输入行开起来了");
        assert_eq!(line.purpose.prompt(), "保存为预设，名称  ");
        assert_eq!(session.views.config.armed_save, None, "重开一次就作废");
        assert!(session.naming_a_preset(), "⏎ 归存那一支");
        // 打的这个名字盘上已经有了：闩上、输入行留着，屏上那一栏里问一句
        // （那一句画在哪儿见 `shell::picker::asked`；真去盘上问的是终端层那一支）。
        session.preset_name_is_taken("漫画");
        assert_eq!(session.views.config.armed_save.as_deref(), Some("漫画"));
        assert!(session.views.input.is_some(), "输入行留着等第二下");
    }

    /// **套用一份就是把两组整个换成它**（`CONTEXT.md` 的《预设栏》）：它没说的那几项回到
    /// 「没说」，**型号与路径、输出一格不动**，顶上那一条当场说「与预设一致」。
    #[test]
    fn using_a_preset_replaces_both_bands_and_leaves_the_model_and_the_paths_alone() {
        let mut session = with_a_picker();
        let now = Instant::now();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.device.gray_levels = Some(12);
        session.taste.dither = Some(tonefit::Dither::FloydSteinberg);
        let paths = session.scope.paths.clone();
        session.perform(Deed::Down, now);
        session.perform(Deed::UsePreset, now);
        assert_eq!(session.taste.fit, Some(tonefit::FitMode::Inside));
        assert_eq!(session.taste.crop, Some(false));
        assert_eq!(session.taste.dither, None, "它没说的回到「没说」");
        assert_eq!(session.device.gray_levels, None, "它没说的回到「没说」");
        assert_eq!(
            session.device.profile.as_deref(),
            Some("kobo-libra-2"),
            "型号一格不动"
        );
        assert_eq!(session.scope.paths, paths, "路径一格不动");
        assert_eq!(session.changed_from_preset(), 0, "与预设一致");
    }

    /// **存出去的那一份不记型号**，因此存完就是「与预设一致」（[`config::stored_fields`]）。
    #[test]
    fn the_preset_a_save_would_store_leaves_the_model_out() {
        let mut session = three_paths();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.taste.fit = Some(tonefit::FitMode::Inside);
        let stored = session.preset_to_store();
        assert_eq!(stored.device.profile, None, "型号不写进去");
        assert_eq!(stored.taste.fit, Some(tonefit::FitMode::Inside));
        session.preset_saved("插图", stored, Instant::now());
        assert_eq!(
            session
                .views
                .config
                .applied
                .as_ref()
                .map(|one| one.name.as_str()),
            Some("插图"),
            "存下的那一份当场成了套着的那一份"
        );
        assert_eq!(session.changed_from_preset(), 0, "存完就是与预设一致");
        assert_eq!(session.views.config.listed.len(), 1, "列的那几份多一条");
    }

    /// **跑着与等待确认时套用定不下**（票面末一条）：预设栏照样掀得开、看得见，
    /// `⏎` 那一下屏底说设置已锁定，两组一格不改——拦它的是**阶段那一维**。
    #[test]
    fn during_a_run_the_picker_opens_but_uses_nothing() {
        for stage in [
            Stage::Running(tonefit::Instruction::Continue),
            Stage::Deciding(tonefit::Instruction::Continue),
        ] {
            let mut session = with_a_picker();
            session.set_stage(stage);
            let now = Instant::now();
            let before = (session.device.clone(), session.taste.clone());
            session.perform(Deed::Down, now);
            session.perform(Deed::UsePreset, now);
            assert_eq!((session.device.clone(), session.taste.clone()), before);
            assert!(session.views.config.applied.is_none(), "套不下来");
            // **末行那一件也开不起来**：光标先挪到它上面再按，不然这一问压根走不到那一支。
            session.perform(Deed::Bottom, now);
            assert!(session.views.config.picked().is_none(), "光标真在末行上");
            session.perform(Deed::UsePreset, now);
            assert!(session.views.input.is_none(), "末行那一件也开不起来");
        }
    }

    /// **读不懂的那一份套不下来**：屏底说一句，两组一格不改、套着的那一份不换
    /// （一份字段过时的预设不该让别的几份列不出来，`Presets::names` 同一条）。
    #[test]
    fn a_preset_that_cannot_be_read_says_so_and_changes_nothing() {
        let mut session = three_paths();
        session.views.view = View::Config;
        session.lift_picker(vec![NamedPreset {
            name: "旧版".to_owned(),
            preset: None,
        }]);
        let now = Instant::now();
        let before = (session.device.clone(), session.taste.clone());
        session.perform(Deed::UsePreset, now);
        assert_eq!((session.device.clone(), session.taste.clone()), before);
        assert!(session.views.config.applied.is_none(), "套不下来");
        let said: String = session
            .views
            .reply(now)
            .expect("屏底说了一句")
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert!(said.contains("旧版") && said.contains("读不懂"), "{said}");
    }

    /// **删掉正在用的那一份**：列的那几份少一条，套着的换成「未使用预设」，
    /// 光标夹回列表里（`CONTEXT.md` 的《预设栏》：正在用的那一份也删得掉）。
    #[test]
    fn erasing_the_preset_in_use_leaves_the_session_using_none() {
        let mut session = with_a_picker();
        let now = Instant::now();
        session.perform(Deed::UsePreset, now);
        assert!(session.views.config.applied.is_some());
        assert_eq!(session.ask_then_erase(), None, "第一下只闩上");
        assert_eq!(session.views.config.armed_delete.as_deref(), Some("漫画"));
        assert_eq!(
            session.ask_then_erase().as_deref(),
            Some("漫画"),
            "第二下才答出要删的那一份"
        );
        session.preset_erased("漫画", now);
        assert_eq!(session.views.config.listed.len(), 1);
        assert!(session.views.config.applied.is_none());
        assert_eq!(session.views.config.armed_delete, None);
    }
}
