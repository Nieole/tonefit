//! 会话的状态：三组设置，与这一趟走到哪个阶段了。
//!
//! **这个模块一个终端都不碰。** 它不 use 终端库，也不读键盘——[`Key`] 是本模块自己的
//! 一个小枚举，把 crossterm 那一侧的键码翻译过来是终端层十几行的事。
//! spec 的 story 44 要的「会话的状态机脱离终端可测」因此是**结构上**成立的，
//! 不是「这批用例碰巧没开终端」：本模块的用例连终端库都编译不到。
//!
//! # 此刻在做什么由两维说（ADR 0017）
//!
//! [`Stage`]（这一趟走到哪个阶段了：没跑过 / 跑着 / 等待确认 / 结束了）是一维，
//! 界面那一维（视图 × 焦点，ADR 0019）在 [`super::view`]。**两维各答各的**：三组设置只读、
//! 按停止与答话归阶段，眼下在看哪一块归界面。「哪件事在哪一档派得出去」只在一张按键表上
//! （[`super::keymap`]），按这两维查。
//!
//! # 三组设置照预设那一份分
//!
//! 设备设置与处理选项**就是** [`DeviceLayer`] 与 [`TasteLayer`]——预设装的那两层
//! （`p1-session/07`），不是另立的一套。三组设置的分界线画在生命周期上
//! （`CONTEXT.md` 的《会话》），而分界线只有一处出处才谈得上是同一条线：
//! 会话里配好的两层存成预设（`p1-session/12`）时不必再做一次搬运。
//!
//! 路径与输出只在会话里有（[`ScopeLayer`]）：它每趟都不同，**不进预设**，
//! 而命令行那一侧它就是 `--out` 与那几个位置参数。
//!
//! # 每一项都有一个「没说」的位置
//!
//! 两层的每一格都是 `Option`——预设**只说它说到的那几项**（见 [`TasteLayer`]）。
//! 会话照搬这条：每一项的取值环上第一格是「默认」，转一圈回得到它。
//! 「没说」与「说了一个恰好等于默认值的值」因此在屏上分得开，
//! 而那正是存成预设时两者的差别。

use std::path::{Path, PathBuf};
use std::time::Instant;

use tonefit::{
    BitDepth, CacheBudget, Dither, Filter, FitMode, Instruction, IoMode, Mode as RunMode, Profile,
    ReadingOrder, Request, SplitThreshold, WhiteAlignLimit,
};

use super::home::Home;
use super::view::{Cursor, Views};
use crate::preset::{DeviceLayer, TasteLayer};

/// 会话认得的按键。**不是终端库那一侧的键码**——那一层的翻译在终端层（`super::terminal`）。
///
/// 只列按键表上有主的那几个（[`super::keymap`]）：认不出的键由终端层原地放过，
/// 状态机不必为它们各留一个「没有意义」的取值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Enter,
    Space,
    Tab,
    Backspace,
    Esc,
    Char(char),
    /// Ctrl-C。它在**每一个**状态下都是退出，编辑到一半也是。
    Interrupt,
    /// **不进缓冲的那一个键**：`F1`。输入行上它掀开全部按键那一张
    /// （`p4-parking-lot/07` 票面第三条，停车场 Q165）——那儿每一个字符都进缓冲，`?` 也是字。
    ///
    /// 代价记在这里：**终端自己截走它的话，会话一个字都收不到**（有几种终端
    /// 把 `F1` 绑在自己的帮助上）。出路一个不少——输入行退出去是一个 `Esc`，出去就有 `?`。
    F1,
}

/// **每页结果列的是哪几页**（`CONTEXT.md` 的《需留意的页》）。
///
/// 一个枚举而不是一个 `bool`：它从[每页结果](super::view::Pages)一路传到画法那一层，
/// 而调用处一个裸 `true` 说不出它列的是哪一批
/// （与 [`super::live::Resuming`] 同一条理由——本仓库不爱看不出意思的裸值）。
///
/// **默认那一档是[只列要紧的](Self::Notable)**：进一卷的目的通常只有一个——
/// 哪一页把整卷拉下来——而两百页的卷里那几页不该由用户自己在四百行里找
/// （`p3-session-legibility/11`）。哪几页算要紧的画质分在 [`crate::render::notable`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Listing {
    /// **只列需留意的页**：特例 · 失败 · 残缺 · 尺寸未贴合屏幕 · 页面超宽 · 兜底上界，
    /// 加上**代表页**（它是这一卷的答案，非在不可）。
    #[default]
    Notable,
    /// **全部页**：`a` 切过来的那一档。
    All,
}

impl Listing {
    /// 按一下 `a` 之后是哪一档。**两档来回**，与两级停止那个只升不降的闩正相反：
    /// 这一下是看法，不是决定，按错了再按一次就回来了。
    pub(super) fn flipped(self) -> Self {
        match self {
            Self::Notable => Self::All,
            Self::All => Self::Notable,
        }
    }
}

/// 按下一个键之后会话还开不开着。
///
/// **不叫 `Outcome`。** 那个词在 `CONTEXT.md` 里已经有主：**结束**（`RunOutcome`——
/// 这一趟是怎么结束的），与「会话还开不开着」不是一回事，同名会让两者迟早被看成一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 会话还开着。
    Stay,
    /// 退出会话。
    Leave,
}

/// 设置栏上可改的一行（设备设置与处理选项；`CONTEXT.md` 的《设置栏》）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    // 设备设置
    Profile,
    GrayLevels,
    Threshold,
    // 处理选项
    Fit,
    Crop,
    Split,
    SplitThreshold,
    ReadingOrder,
    Filter,
    WhiteAlignLimit,
    BitDepth,
    Dither,
    Envelope,
    CacheBudget,
    IoMode,
}

/// 设备设置的三项，次序就是屏上的次序（`p1-session/07` 的分法）。
pub const DEVICE_FIELDS: [Field; 3] = [Field::Profile, Field::GrayLevels, Field::Threshold];

/// 处理选项的十二项，次序就是屏上的次序（`p1-session/07` 的分法）。
///
/// 前五项是页几何那一批添的（缩放方式、裁白边、拆分与判定宽度、阅读方向），
/// 提白上限是纸色提白那一批添的（04 号票），摆在缩放算法之后——
/// 与它在管线里的位置同序（缩放之后、量化之前），也与 [`TasteLayer`] 的字段同序；
/// 其余六项是 spec 的《会话：三组设置与预设》原本就列着的那几项。
/// 这张单子与 [`TasteLayer`] 的字段**一一对应**，由本模块的
/// `the_two_layers_on_screen_are_the_two_layers_a_preset_stores` 拴住。
pub const TASTE_FIELDS: [Field; 12] = [
    Field::Fit,
    Field::Crop,
    Field::Split,
    Field::SplitThreshold,
    Field::ReadingOrder,
    Field::Filter,
    Field::WhiteAlignLimit,
    Field::BitDepth,
    Field::Dither,
    Field::Envelope,
    Field::CacheBudget,
    Field::IoMode,
];

/// 一行怎么改：详情栏按它摊开一个取值环，还是经输入行打字（`super::config`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// 打字改：数、界、字节数。
    Text,
    /// 在取值环上挑。
    Cycle,
}

impl Field {
    /// 设置栏上这一行的名字。
    pub fn label(self) -> &'static str {
        match self {
            Field::Profile => "型号",
            Field::GrayLevels => "可见灰阶数",
            Field::Threshold => "画质门槛",
            Field::Fit => "缩放方式",
            Field::Crop => "裁白边",
            Field::Split => "拆分跨页",
            Field::SplitThreshold => "跨页判定宽度",
            Field::ReadingOrder => "阅读方向",
            Field::Filter => "缩放算法",
            Field::WhiteAlignLimit => "提白上限",
            Field::BitDepth => "灰阶档位",
            Field::Dither => "抖动",
            Field::Envelope => "整卷统一灰阶",
            Field::CacheBudget => "内存上限",
            Field::IoMode => "读盘方式",
        }
    }

    /// **这一行摊开的是两层吗**（`CONTEXT.md` 的《会话》：下钻）。
    ///
    /// **摊得开的就是取值是[环](Shape::Cycle)的那几行**，不必另立一个谓词；
    /// 分岔只剩这一处：**型号那一行摊开的是面板**（内置表里有几块就是几块），
    /// 下钻进去才是那块面板底下的型号——两层，与别处那一层不是一个形状
    /// （详情栏那一副在 `super::config`）。
    ///
    /// **它还是「取值恒在环上」那条前提的守门人**：走**环**那一路的几行取值都是
    /// 枚举或布尔，一格不落地都在自己的环上——[`Session::ring`] 数「此刻生效的是
    /// 第几格」靠的就是这条前提。唯一可能落在环外的是型号（预设里塞进来的一个已删型号，
    /// 见 [`next_device`]），而它走的正是另一路：面板那一层认的是**这个名字在哪一块
    /// 面板底下**，认不出来就一格都不标（`p3-session-legibility/06` 票面第七条）。
    pub fn drills(self) -> bool {
        self == Field::Profile
    }

    /// 这一行怎么改。逐个变体都列出来，**不留 `_`**：新添一行怎么改是个要当场拿的主意。
    pub fn shape(self) -> Shape {
        match self {
            Field::GrayLevels
            | Field::Threshold
            | Field::SplitThreshold
            | Field::WhiteAlignLimit
            | Field::CacheBudget => Shape::Text,
            Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::Envelope
            | Field::IoMode => Shape::Cycle,
        }
    }
}

/// 路径与输出：这一趟点名哪几个卷、写到哪儿。
///
/// **不进预设**（`preset` 模块的抬头写着为什么）：它每趟都不同，
/// 混进预设会让人套用时误写到上一次的输出目录（ADR 0009）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeLayer {
    /// 输出目录。每个卷在它下面得到一份同名副本。
    pub out: Option<PathBuf>,
    /// 点名的那些**处理路径**，按打进来的次序。
    pub paths: Vec<NamedPath>,
}

/// 路径与输出里点名的一条**处理路径 (Named path)**，连同它这一趟算不算数
/// （`CONTEXT.md` 的《处理路径》；停车场 Q711）。
///
/// **不叫「卷」。** 它不是卷：一个归档或一个目录，发现把它展开成一批卷（ADR 0014），
/// 一条底下可以是几个系列、几百卷。这里装的是「用户点了它」这件事——
/// 一条路径加一个勾，连点不点得开都还没问过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPath {
    pub path: PathBuf,
    /// 勾着的才进这一趟。**打错一条勾掉就是了，不必把整份重打一遍**（spec 的 story 16）。
    pub on: bool,
}

impl NamedPath {
    /// 文件夹还是压缩包：**按扩展名认，不碰盘**，与发现认得的归档扩展名同一份
    /// （`tonefit::is_archive`；spec《卷列表》开跑之前）。
    pub fn is_archive(&self) -> bool {
        tonefit::is_archive(&self.path)
    }

    /// 屏上怎么叫它那一种。
    pub fn kind(&self) -> &'static str {
        Self::kind_of(&self.path)
    }

    /// 盘上这一处屏上怎么叫：压缩包还是文件夹——**措辞只在这里**，补全框旁边那一句也从这里取。
    pub fn kind_of(path: &Path) -> &'static str {
        if tonefit::is_archive(path) {
            "压缩包"
        } else {
            "文件夹"
        }
    }
}

/// **这一趟走到哪个阶段了**——会话两维中的**第一维**（ADR 0017）。
///
/// 四个取值答的是同一个问题：这一趟走到哪儿了。另一维（视图 × 焦点，[`super::view`]）答的是
/// 「眼下在看什么」。
///
/// **三组设置只读由这一维说了算**（[`read_only`](Self::read_only)），**与焦点在哪无关**：
/// 配置视图跑着时照样进得来，改的那一下被这一维拦下。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// **一趟都还没跑过。**
    Fresh,
    /// 一趟正跑着，带着**按停止时按到哪一级了**。
    ///
    /// **三组设置全只读**，两层各错在什么地方见 `CONTEXT.md` 的《会话》（`p1-session/10`
    /// 把路径与输出也算了进来，停车场 Q69）。
    ///
    /// 那一格装的是[闩](Session::stopping)：`Continue` 是没按过、`Finish` 是按过一次
    /// （做完再停）、`Abort` 是再按了一次（立即停止）。**只升不降**——按停止不是一个可以反悔的开关
    /// （`CONTEXT.md` 的《进度》）。
    Running(Instruction),
    /// 一趟**停在确认点上等人拿主意**（`CONTEXT.md` 的《等待确认》，ADR 0012 决定第 3 条）。
    ///
    /// **预览到得了这里，几卷都一样，一卷一次**（决定第 3 条，`volume-discovery/07`）：
    /// 确认点本来就是逐卷的，逐卷停下来问，缓存始终只押着当前那一卷，内存一点不涨。
    /// 转换那一趟不在这儿停——用户按 `x` 的时候已经拿过主意了。
    /// 答过「后面的卷都写出」之后也不再到这里：往下的确认点由观察者
    /// 那一侧的默认答案当场答掉（`super::run::Gate`），那条线程根本不停下来。
    ///
    /// **它是这一维上的一个取值，不是 [`Running`](Self::Running) 上的一个开关**
    /// （`p1-session/14`）：跑着与等待确认派得出的是两套（跑着时是停，
    /// 等待确认时是答话那三件）。摆进同一个取值，屏上就要靠一个 flag 分岔。
    ///
    /// 三组设置在这一刻**仍然只读**，与跑着时一个待遇：`Request` 在起线程那一刻就是一份快照，
    /// 而这一趟还没结束。
    ///
    /// 那一格装的还是[闩](Session::stopping)：在确认点上等着的时候，闩记着的是这一趟
    /// **此前**按过的停。答完话回 [`Running`](Self::Running) 时它原样带回去——
    /// 确认点上答的字不是闩，两者互不覆盖。
    Deciding(Instruction),
    /// **结束了**：三组设置又改得动，而报告一行不少地摆在那儿。
    Ended,
}

impl Stage {
    /// **三组设置此刻只读吗**（`CONTEXT.md` 的《会话》：一趟跑起来之后三组设置都只读）。
    ///
    /// 跑着与等待确认都是：`Request` 在起线程那一刻就是一份快照，而这一趟还没结束。
    /// **这是「只读」在本仓库唯一的判据**——设置栏照它写「只读」、改的那一下照它拦下
    /// （`super::view` 的 `settings_locked`）。
    pub fn read_only(self) -> bool {
        matches!(self, Self::Running(_) | Self::Deciding(_))
    }
}

/// 一个会话：三组设置、这一趟走到哪个[阶段](Stage)了，以及界面状态（[`Views`]）。
///
/// **阶段是一维，界面是另一维**（ADR 0017 的两维，ADR 0019 把第二维换成视图 × 焦点）：
/// 三组设置只读、按停止与答话那几件归阶段，眼下在看哪个视图、哪一块归 [`Views`]。
/// 「哪件事在哪一档派得出去」只在一张按键表上（[`super::keymap`]）。
///
/// **跑起来的那一趟不在这里**：这个结构只记得「此刻在做什么」（[`Stage::Running`]），
/// 攒着的那份报告与两条进度在 [`super::live::Live`] 上。分开是因为它们的寿命不同——
/// 会话一个，跑过的趟一趟一份。
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// 设备设置：预设装的那一层，一格不多一格不少。
    pub device: DeviceLayer,
    /// 处理选项：预设装的那一层，一格不多一格不少。
    pub taste: TasteLayer,
    /// 路径与输出：只在会话里有。
    pub scope: ScopeLayer,
    /// 这一趟走到哪个阶段了（[两维](Stage)之一）。
    stage: Stage,
    /// 界面状态：视图、两个视图各自的光标与块、屏底那两样临时的东西
    /// （[`super::view`]，ADR 0019）。
    pub views: Views,
    /// **家目录**：屏上的路径把它缩写成 `~`（[`super::home`]）。由会话入口问一次摆进来，
    /// 问不出来就不缩写。
    pub home: Home,
    /// **会话打开那一刻**：屏上那个**转轮**转到第几格从它算
    /// （一格 90 毫秒、十格一圈，见 `super::shell::marks`）。
    ///
    /// 转轮说的是「还在动」，与这一趟跑了多久、这一卷走到第几页都无关——
    /// 清点中那一段一步都没走，转轮照样得转。它因此从**会话**那一头的钟算，
    /// 不从那一趟的计时算。用例给定它（`super::scene`）。
    pub opened_at: Instant,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            device: DeviceLayer::default(),
            taste: TasteLayer::default(),
            scope: ScopeLayer::default(),
            stage: Stage::Fresh,
            views: Views::default(),
            home: Home::unknown(),
            opened_at: Instant::now(),
        }
    }

    /// 用例把阶段直接摆到某一档上（新界面的按键表按阶段查，四档各问一遍）。
    #[cfg(test)]
    pub(super) fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
    }

    /// **这一趟走到哪个阶段了**（两维之一，ADR 0017）。
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// 一趟跑起来了：进 [`Stage::Running`]，配置从这一刻起只读。
    ///
    /// 闩从[继续](Instruction::Continue)起——**一趟一份**。上一趟按下的停不该跟着漏到
    /// 下一趟去，理由与库那一侧把闩放在 `run` 的栈上是同一条（见 `tonefit` 的
    /// `progress::Events`）。跑着的那一趟那一份见 [`super::run::Running::start`]。
    ///
    /// **界面那一副也从头来一遍**：树还没拼出来、一个目录都不展开、自动滚动扳回开着
    /// （`CONTEXT.md` 的《自动滚动》：每次开跑扳回开着）。
    pub fn run_started(&mut self) {
        self.stage = Stage::Running(Instruction::Continue);
        self.views.task.start_a_run();
    }

    /// 那一趟结束了：配置又改得动。
    ///
    /// **改的是[阶段](Stage)那一维，界面一格不动**（ADR 0017）：正在读卷列表的那个人
    /// 读的东西，不该因为最后一卷跑完而被搬走。
    ///
    /// **不叫 `Live::run_finished`。** 那一个折的是 `RunFinished` 那条**事件**
    /// （库说「这一趟完了」），这一个改的是**会话**此刻在做什么——两件事，两个接收者。
    pub fn run_finished(&mut self) {
        if self.stage.read_only() {
            self.stage = Stage::Ended;
        }
    }

    /// **结束之后 `o`／`i` 回到开跑之前那一副**（`CONTEXT.md` 的《卷列表》末一句；
    /// spec《卷列表》；设计稿 `taskKey` 的 `S.run = null`）：阶段退回[没跑过](Stage::Fresh)，
    /// 卷列表因此从那棵树换回处理路径，总览换回「还没开始」，屏底换回开跑之前那几件。
    ///
    /// **那一趟本身不丢**：退出会话时 stdout 上仍印得出上一趟的报告
    /// （`super::run::Running::report` 读的是那一趟攒下来的那一份）——屏底那一句说的就是它。
    /// 屏上「还没开始」因此由**阶段**说了算，不由「有没有那一趟」说了算
    /// （画法那几处问的是 [`super::keymap::Phase`]）。
    ///
    /// **只在结束了那一档按得动**（按键表上那两行标的是 `ENDED`）：跑着的时候
    /// `o`／`i` 一件事都不派。
    pub fn back_to_paths(&mut self) {
        if self.stage != Stage::Ended {
            return;
        }
        self.stage = Stage::Fresh;
        self.views.task.start_a_run();
        self.views.task.cursor = Cursor::Output;
    }

    /// **那一趟到确认点了没有**：在[跑着](Stage::Running)与[等待确认](Stage::Deciding)
    /// 之间转（`p1-session/14`）。
    ///
    /// 会话每帧问一次，与 `reap` 同一条（见 `super::drive`）：停在确认点上的是
    /// **计算线程**，而本模块碰不到线程——那一层问得到（`super::run::Running::deciding`），
    /// 把答案交进来。
    ///
    /// 别的状态一格不动：这一问只在这两者之间转场。答完话那一下不必等下一帧
    /// ——答话那一下当场就把状态放回去（见 [`Self::answered`]）。
    pub fn at_the_decision_point(&mut self, waiting: bool) {
        self.stage = match (self.stage, waiting) {
            (Stage::Running(pressed), true) => Stage::Deciding(pressed),
            (Stage::Deciding(pressed), false) => Stage::Running(pressed),
            _ => return,
        };
    }

    /// 确认点上答完话了：回[跑着](Stage::Running)那一副，闩原样带回去。
    ///
    /// **当场就转，不等下一帧**：那条线程收到那个字就接着跑，而屏底那两行要跟着换——
    /// 慢一帧的话，答完之后那两个答话键还在屏上摆着，按下去却已经没有人收了。
    ///
    /// **闩一格不动**（`CONTEXT.md` 的《会话》：等待确认时按的 `s` 不是按停止）：
    /// 这里换的只是阶段那一维，按停止按到的那一级原样带过去。答话那一下经
    /// `super::view::Session::perform` 到这里。
    pub(super) fn answered(&mut self) {
        if let Stage::Deciding(pressed) = self.stage {
            self.stage = Stage::Running(pressed);
        }
    }

    /// **按停止时按到哪一级了**：没按过是[继续](Instruction::Continue)，按过一次是
    /// [做完再停](Instruction::Finish)，再按一次是[立即停止](Instruction::Abort)（ADR 0013）。
    ///
    /// 没跑着的时候恒是继续：按停止是跑起来之后才有的事，浏览时那个键根本不派动作。
    ///
    /// 屏上照它写，而跑着的那一趟收到的是同一个字——`crate::session::terminal::input`
    /// 按下之后把它交给 [`super::run::Running::stop`]。
    pub fn stopping(&self) -> Instruction {
        match self.stage {
            Stage::Running(pressed) | Stage::Deciding(pressed) => pressed,
            Stage::Fresh | Stage::Ended => Instruction::Continue,
        }
    }

    /// **此刻停在确认点上等人拿主意吗**（`CONTEXT.md` 的《等待确认》）。用例拿它问阶段那一维；
    /// 屏上那几处问的是 [`super::keymap::Phase`]。
    #[cfg(test)]
    pub fn deciding(&self) -> bool {
        matches!(self.stage, Stage::Deciding(_))
    }

    /// 把三组设置拼成这一趟的 [`Request`]。**预览与转换只差 `mode` 一格**
    /// （`CONTEXT.md` 的《会话》：两者是同一条回路的两半）。
    ///
    /// 处理选项每一项落到默认值那一步走 [`TasteLayer`] 自己那几个方法——命令行
    /// 「命令行没点、预设也没说」那一档读的是同一个（见 `crate::Cli`），
    /// 默认值因此没有第二个出处。型号那一步走 [`crate::target_profile`]，
    /// 命令行与 `calibrate` 用的也是它：三处解析出来的必须是同一块面板。
    ///
    /// **只有两项在这里挡**：型号与输出目录。它们在 `Request` 上不是 `Option`，
    /// 拼不出来就无从下手。范围为空不在这里挡——那句话库那一侧已经有了
    /// （`run` 的「处理范围为空」），会话再写一句就是第二份措辞。
    pub fn request(&self, mode: RunMode) -> anyhow::Result<Request> {
        let Some(device) = self.device.profile.as_deref() else {
            anyhow::bail!("先挑型号：目标尺寸与画质分都从那块面板上来，没有它跑不起来");
        };
        let Some(output_root) = self.scope.out.clone() else {
            anyhow::bail!("先填输出目录：每个卷在它下面得到一份同名副本");
        };
        let taste = &self.taste;
        Ok(Request {
            inputs: self
                .scope
                .paths
                .iter()
                .filter(|named| named.on)
                .map(|named| named.path.clone())
                .collect(),
            output_root,
            profile: crate::target_profile(device, self.device.gray_levels, self.device.threshold)?,
            fit: taste.fit(),
            crop: taste.crop(),
            split: taste.split_rule(),
            filter: taste.filter(),
            white_align_limit: taste.white_align_limit(),
            bit_depth: taste.bit_depth,
            dither: taste.dither,
            envelope: taste.envelope(),
            cache_budget: taste.cache_budget(),
            mode,
            io_mode: taste.io_mode(),
            // 会话里没有 `--no-metadata` 那一项：它一开就把记录与幂等一起关掉，
            // 而那是对**这一批输出**的处置，不是一份存得住的立场（见 `preset::TasteLayer`）。
            metadata: true,
            // 观察者由起线程的那一层接上去（见 [`super::run::Running::start`]）：
            // 本模块一个终端都不碰，也不该起线程。
            progress: None,
        })
    }

    /// 灰阶测试图要按哪块面板画：把设备设置拼成一个 [`Profile`]。
    ///
    /// **画质门槛恒不带**（传 `None`），与命令行那一路逐字同一条：灰阶测试图是量具，不经判定
    /// （见 `crate::target_profile` 与 `crate::calibrate`）。合 profile 走的也是那一个函数——
    /// 处理卷、`calibrate` 子命令、会话，三处解析出来的必须是同一块面板。
    ///
    /// **型号没挑就说一句**，与拼不出 [`Request`](Self::request) 时同一个待遇：
    /// 那一格没有默认值可退，图按面板分辨率排布，没有面板就无从画起。
    ///
    /// **不是 [`calibrated_profile`](Self::calibrated_profile)。** 那一个是给设置栏
    /// 画质门槛那一行印数用的：它**要**画质门槛（印的就是它），而且拼不出来只意味着那一行印不出来，
    /// 因此吞成 `None`。这一个反过来：画质门槛一格不带，而拼不出来要说得出口。
    pub(super) fn chart_profile(&self) -> anyhow::Result<Profile> {
        let Some(device) = self.device.profile.as_deref() else {
            anyhow::bail!("先挑型号：灰阶测试图按那块面板的分辨率排布，没有它画不出来");
        };
        crate::target_profile(device, self.device.gray_levels, None)
    }

    /// 把闩往上升一级：继续 → 做完再停 → 立即停止 → 立即停止（ADR 0013）。
    ///
    /// **只升不降**是这个函数的形状本身：升到立即停止之后它就是个不动点，
    /// 而键盘上没有第二个键能往回按——两级停止是同一个键按两次。
    /// 库那一侧的闩用 `fetch_max` 说同一件事（`tonefit::Instruction` 的序即力度）。
    pub(super) fn raise_stop(&mut self) {
        if let Stage::Running(pressed) = &mut self.stage {
            *pressed = match *pressed {
                Instruction::Continue => Instruction::Finish,
                Instruction::Finish | Instruction::Abort => Instruction::Abort,
            };
        }
    }
}

/// 一个「没说 + 若干取值」的环：`None` 是「没说」那一格，走完一圈落回它。
///
/// 取值那一圈由 `next` 给（见下面那几个函数），`first` 是那一圈的起点——
/// 走到「下一个就是起点」时说明这一圈到头了，落回 `None`。
fn ring<T: Copy + PartialEq>(value: Option<T>, first: T, next: impl Fn(T) -> T) -> Option<T> {
    match value {
        None => Some(first),
        Some(current) => {
            let ahead = next(current);
            (ahead != first).then_some(ahead)
        }
    }
}

// 下面这几个环各自是一个**穷尽的 match**，不是一张手抄的清单：
// 库那一侧给某个枚举加一个变体，这里当场编译不过，而抄一份清单只会静默漏掉它。

fn next_fit(fit: FitMode) -> FitMode {
    match fit {
        FitMode::Height => FitMode::Inside,
        FitMode::Inside => FitMode::Height,
    }
}

fn next_order(order: ReadingOrder) -> ReadingOrder {
    match order {
        ReadingOrder::RightToLeft => ReadingOrder::LeftToRight,
        ReadingOrder::LeftToRight => ReadingOrder::RightToLeft,
    }
}

fn next_filter(filter: Filter) -> Filter {
    match filter {
        Filter::Area => Filter::Bilinear,
        Filter::Bilinear => Filter::Hamming,
        Filter::Hamming => Filter::Bicubic,
        Filter::Bicubic => Filter::Lanczos3,
        Filter::Lanczos3 => Filter::Area,
    }
}

fn next_io_mode(mode: IoMode) -> IoMode {
    match mode {
        IoMode::Auto => IoMode::Serial,
        IoMode::Serial => IoMode::Concurrent,
        IoMode::Concurrent => IoMode::Auto,
    }
}

fn next_dither(dither: Dither) -> Dither {
    match dither {
        Dither::Off => Dither::FloydSteinberg,
        Dither::FloydSteinberg => Dither::Off,
    }
}

fn next_bit_depth(depth: BitDepth) -> BitDepth {
    match depth {
        BitDepth::One => BitDepth::Two,
        BitDepth::Two => BitDepth::Four,
        BitDepth::Four => BitDepth::Eight,
        BitDepth::Eight => BitDepth::One,
    }
}

/// 布尔那一项的取值圈：开 → 关 → 开。「没说」那一格与别的项一样由 [`ring`] 补。
fn next_flag(flag: bool) -> bool {
    !flag
}

/// 型号那一环走内置表。表外的名字（预设里塞进来的一个已删型号）落回「没挑」，
/// 再按一下就回到表头——那比停在一个解析不出来的名字上强。
fn next_device(device: Option<&str>) -> Option<String> {
    let Some(current) = device else {
        return Profile::devices().next().map(str::to_owned);
    };
    let mut listed = Profile::devices().skip_while(|name| *name != current);
    listed.next();
    listed.next().map(str::to_owned)
}

impl Session {
    /// 把**某一行**的取值往前转一格。
    ///
    /// 拆出这一支只为一件事：**详情栏摊开的那一列与定下来的那一下走的就是它**
    /// （见 [`Self::ring`] 与 [`Self::settle`]）。两条路因此改的是同一格、
    /// 走的是同一条写入路径——分成两份就会有一处忘了跟着改（型号那一下要清掉
    /// 标定出来的两个数，就是这种一处忘了就错的东西）。
    fn turn_field(&mut self, field: Field) {
        match field {
            Field::Profile => {
                let turned = next_device(self.device.profile.as_deref());
                self.set_device(turned);
            }
            Field::Fit => self.taste.fit = ring(self.taste.fit, FitMode::Height, next_fit),
            Field::Crop => self.taste.crop = turn_flag(self.taste.crop),
            Field::Split => self.taste.split = turn_flag(self.taste.split),
            Field::ReadingOrder => {
                self.taste.reading_order = ring(
                    self.taste.reading_order,
                    ReadingOrder::RightToLeft,
                    next_order,
                );
            }
            Field::Filter => self.taste.filter = ring(self.taste.filter, Filter::Area, next_filter),
            Field::BitDepth => {
                self.taste.bit_depth = ring(self.taste.bit_depth, BitDepth::One, next_bit_depth);
            }
            Field::Dither => self.taste.dither = ring(self.taste.dither, Dither::Off, next_dither),
            Field::Envelope => self.taste.envelope = turn_flag(self.taste.envelope),
            Field::IoMode => {
                self.taste.io_mode = ring(self.taste.io_mode, IoMode::Auto, next_io_mode);
            }
            // 打字改的那几行转不动，详情栏根本不会为它们摊开一个环。
            // 逐个列出来而不写 `_`：新添一行转不转得动是个要当场拿的主意。
            Field::GrayLevels
            | Field::Threshold
            | Field::SplitThreshold
            | Field::WhiteAlignLimit
            | Field::CacheBudget => {}
        }
    }

    /// 这一行此刻停在「**没说**」那一格上吗（`CONTEXT.md` 的《会话》：
    /// 「存出去的只有『说了的那几项』」）。
    ///
    /// 两层的每一格都有这个位置，而它正是存成预设时**写不写进那份 TOML** 的分别：
    /// 「没说」跟着默认值走，「说了一个恰好等于默认的值」写下来、往后默认改了它也不变。
    ///
    /// 详情栏靠它认路：那一列从这一格起摊、走一圈落回它为止
    /// （见 [`Self::ring`] 与 [`Self::settle`]）。逐个变体都列出来，
    /// 理由与 [`Field::shape`] 同一条。
    pub(super) fn unsaid(&self, field: Field) -> bool {
        match field {
            Field::Profile => self.device.profile.is_none(),
            Field::GrayLevels => self.device.gray_levels.is_none(),
            Field::Threshold => self.device.threshold.is_none(),
            Field::Fit => self.taste.fit.is_none(),
            Field::Crop => self.taste.crop.is_none(),
            Field::Split => self.taste.split.is_none(),
            Field::SplitThreshold => self.taste.split_threshold.is_none(),
            Field::ReadingOrder => self.taste.reading_order.is_none(),
            Field::Filter => self.taste.filter.is_none(),
            Field::WhiteAlignLimit => self.taste.white_align_limit.is_none(),
            Field::BitDepth => self.taste.bit_depth.is_none(),
            Field::Dither => self.taste.dither.is_none(),
            Field::Envelope => self.taste.envelope.is_none(),
            Field::CacheBudget => self.taste.cache_budget.is_none(),
            Field::IoMode => self.taste.io_mode.is_none(),
        }
    }

    /// **一直往前转到「没说」那一格上**，答走了几步。
    ///
    /// 环上从任何一格出发都到得了它（走完一圈必落回它，见 [`ring`]），
    /// 因此这个循环停得下来。详情栏摊开与定下来都从这一格起算
    /// （见 [`Self::ring`] 与 [`Self::settle`]），两处共用它——
    /// 写两份就会有一处忘了跟着改。
    fn turn_to_unsaid(&mut self, field: Field) -> usize {
        let mut steps = 0;
        while !self.unsaid(field) {
            self.turn_field(field);
            steps += 1;
        }
        steps
    }

    /// **某一项的取值环在屏上的那几格**，连同此刻生效的是第几格。
    ///
    /// 那一列**就是那一行的取值环**，从「没说」那一格起走一圈落回它为止——一步一步走的就是
    /// [`Self::turn_field`]，因此这里没有第二份清单：环上加一个取值，摊开那一列当场跟着多一格。
    /// 每一格印成什么走 [`Self::shown`]，与那一行自己印的是同一份。
    ///
    /// **此刻生效的是第几格，是数出来的、不是认字认出来的**：从这一行此刻停的那一格起走
    /// `back` 步到得了「没说」，它就在环上倒数第 `back` 格。前提是「那一行的取值在环上」，
    /// 守门的是 [`Field::drills`]——型号那一行走的是另一路（[`Self::panels`]）。
    ///
    /// 详情栏（`super::config`）问的就是这一个环。
    pub(super) fn ring(&self, field: Field) -> (Vec<String>, usize) {
        let mut probe = self.clone();
        let back = probe.turn_to_unsaid(field);
        let mut cells = Vec::new();
        loop {
            cells.push(probe.shown(field));
            probe.turn_field(field);
            if probe.unsaid(field) {
                break;
            }
        }
        let chosen = (cells.len() - back) % cells.len();
        (cells, chosen)
    }

    /// **把某一项定到取值环上第几格**（新界面的详情栏定下来那一下）。
    ///
    /// 先转到「没说」那一格，再往前走 `at` 格——走的每一步都是 [`Self::turn_field`]。
    /// **摊开这一路因此没有自己的写入路径**：[`Self::ring`] 数出来的那一格与这里定下的
    /// 那一格是同一格。
    pub(super) fn settle(&mut self, field: Field, at: usize) {
        self.turn_to_unsaid(field);
        for _ in 0..at {
            self.turn_field(field);
        }
    }

    /// **换掉型号。写型号只有这一条路。**
    ///
    /// 取值环上转一格（[`Self::turn_field`] 的型号那一支）与详情栏上挑中一个型号的那一下
    /// （`super::view`）走的都是它：**换掉型号仍旧把标定出来的屏幕灰阶数与画质门槛清空**
    /// （ADR 0002：画质分与画质门槛跟着面板走、不可跨面板比较），而那件事分成两份写就会有
    /// 一处忘了跟着做。
    pub(super) fn set_device(&mut self, device: Option<String>) {
        self.device.profile = device;
        self.device.gray_levels = None;
        self.device.threshold = None;
    }

    /// 这一行当前取值的**可编辑写法**：进编辑时缓冲里摆的就是它，空串代表「没说」。
    pub(super) fn typed(&self, field: Field) -> String {
        match field {
            Field::GrayLevels => self.device.gray_levels.map(|n| n.to_string()),
            Field::Threshold => self.device.threshold.map(|value| value.to_string()),
            Field::SplitThreshold => self
                .taste
                .split_threshold
                .map(|threshold| threshold.value().to_string()),
            Field::WhiteAlignLimit => self.taste.white_align_limit.map(|limit| limit.to_string()),
            Field::CacheBudget => self.taste.cache_budget.map(crate::preset::spell_budget),
            // 转着改的行没有可编辑的写法。
            Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::Envelope
            | Field::IoMode => None,
        }
        .unwrap_or_default()
    }

    /// 把一行打出来的文本验成取值收下。空串是「没说」，落回默认值。
    pub(super) fn take(&mut self, field: Field, typed: &str) -> anyhow::Result<()> {
        match field {
            Field::GrayLevels => {
                let levels = self.calibrated(typed, |profile, levels: u32| {
                    profile.with_gray_levels(levels).map(|_| levels)
                })?;
                self.device.gray_levels = levels;
            }
            Field::Threshold => {
                let threshold = self.calibrated(typed, |profile, value: f32| {
                    profile.with_threshold(value).map(|_| value)
                })?;
                self.device.threshold = threshold;
            }
            Field::SplitThreshold => {
                self.taste.split_threshold = match typed {
                    "" => None,
                    text => Some(SplitThreshold::parse(text)?),
                };
            }
            Field::WhiteAlignLimit => {
                self.taste.white_align_limit = match typed {
                    "" => None,
                    text => Some(parse_white_align_limit(text)?),
                };
            }
            Field::CacheBudget => {
                self.taste.cache_budget = match typed {
                    "" => None,
                    text => Some(CacheBudget::parse(text)?),
                };
            }
            // 转着改的行打不了字，输入行根本不会为它们打开。
            Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::Envelope
            | Field::IoMode => {}
        }
        Ok(())
    }

    /// 设备设置那两个覆盖项：**要先挑型号**，界才验得动。
    ///
    /// 与预设那一侧是同一条规矩（见 `preset` 的 `no_panel_to_calibrate_against_error`）：
    /// 可见灰阶数是在某一台真机上数出来的，画质门槛是在某一块面板上盲测夹出来的，
    /// 而画质分跟着面板走、不可跨面板比较（ADR 0002）。界本身只有一处出处
    /// （`Profile::with_gray_levels` 与 `with_threshold`），这里验的就是它。
    fn calibrated<T: std::str::FromStr>(
        &self,
        typed: &str,
        check: impl FnOnce(Profile, T) -> anyhow::Result<T>,
    ) -> anyhow::Result<Option<T>>
    where
        T::Err: std::fmt::Display,
    {
        if typed.is_empty() {
            return Ok(None);
        }
        let Some(device) = self.device.profile.as_deref() else {
            anyhow::bail!(
                "先挑型号：这个数是在某一块面板上量出来的，不说是哪块就没有界可验（ADR 0002）"
            );
        };
        let value: T = typed
            .parse()
            .map_err(|error| anyhow::anyhow!("读不出这个数：{error}"))?;
        check(Profile::resolve(device)?, value).map(Some)
    }

    /// 画质门槛那一行照 `tonefit::Threshold` 自己的 `Display` 印：**数值加标定来源**，
    /// 与报告里那一行同一个出处。
    ///
    /// 不在这里另写一句（spec 的 Further Notes：「会话里显示时照报告的写法把来源原样带上来，
    /// 不自己另编一套说法」）。标定来源是画质门槛的一部分，不是旁注——画质分跟着面板走、
    /// 不可跨面板比较（ADR 0002），读的人得能自己判断它对手上那块板成不成立。
    ///
    /// 还没挑型号时印不出数：界挂在 profile 上，那是它唯一的出处。
    fn threshold_shown(&self) -> String {
        match self.calibrated_profile() {
            Some(profile) => profile.threshold().to_string(),
            None => "跟着型号走（先挑一个）".to_owned(),
        }
    }

    /// 眼下这一趟的 profile：型号加设备设置那两个覆盖项。挑了型号才有。
    ///
    /// 合出来的那一步走 [`crate::target_profile`]——命令行与 `calibrate` 用的是同一个，
    /// 三处解析出来的必须是同一块面板。
    ///
    /// 合法性在收下那一刻就验过了（见 [`Session::calibrated`]），失败只可能是
    /// 型号被换掉而覆盖项没跟着清，而那条路由 [`Session::cycle`] 堵着；
    /// 真落到 `ok()` 上等于「这一行印不出来」，不是错误。
    pub(super) fn calibrated_profile(&self) -> Option<Profile> {
        crate::target_profile(
            self.device.profile.as_deref()?,
            self.device.gray_levels,
            self.device.threshold,
        )
        .ok()
    }

    /// 这一行取值在屏上的写法。「没说」那一格连同它落到的默认值一并印出来——
    /// 两者的差别只有存成预设时才看得见，而屏上看不见的差别用户改不动。
    pub fn shown(&self, field: Field) -> String {
        match field {
            Field::Profile => self
                .device
                .profile
                .clone()
                .unwrap_or_else(|| "未挑（跑起来之前必填）".to_owned()),
            Field::GrayLevels => spell(self.device.gray_levels, "按屏幕规格"),
            Field::Threshold => self.threshold_shown(),
            Field::Fit => spell_name(self.taste.fit, FitMode::name),
            Field::Crop => spell_flag(self.taste.crop, self.taste.crop(), "裁", "不裁"),
            Field::Split => spell_flag(self.taste.split, self.taste.split_rule().on, "拆", "不拆"),
            Field::SplitThreshold => spell(
                self.taste.split_threshold.map(SplitThreshold::value),
                &SplitThreshold::default().value().to_string(),
            ),
            Field::ReadingOrder => spell_name(self.taste.reading_order, ReadingOrder::name),
            Field::Filter => spell_name(self.taste.filter, Filter::name),
            Field::WhiteAlignLimit => spell(
                self.taste.white_align_limit,
                &WhiteAlignLimit::default().to_string(),
            ),
            Field::BitDepth => match self.taste.bit_depth {
                Some(depth) => format!("{}bit", depth.bits()),
                None => "自动（画质分说了算）".to_owned(),
            },
            Field::Dither => match self.taste.dither {
                Some(dither) => dither.name().to_owned(),
                None => "自动（画质分说了算）".to_owned(),
            },
            Field::Envelope => spell_flag(self.taste.envelope, self.taste.envelope(), "开", "关"),
            Field::CacheBudget => match self.taste.cache_budget {
                Some(budget) => budget.to_string(),
                None => format!("默认（{}）", CacheBudget::default()),
            },
            Field::IoMode => spell_name(self.taste.io_mode, IoMode::name),
        }
    }
}

/// 输出目录没填时屏上那一句。两副界面同一句（新界面在 `super::view` 的 `output_shown`）。
pub(super) const OUTPUT_UNSET: &str = "未填（跑起来之前必填）";

/// 三个布尔项转一格：没说 → 开 → 关 → 没说。
fn turn_flag(flag: Option<bool>) -> Option<bool> {
    ring(flag, true, next_flag)
}

/// 提白上限那一行打出来的文本：**一个 0 到 255 的整数级数**，与 `--white-align-limit`
/// 同一条界（那一头是 clap 按 `u8` 收的，预设那一头是 TOML 按 `u8` 读的）。
///
/// 会话是三处里唯一要自己把字变成数的地方——另两处各有解析器替它做；
/// 这一句措辞因此只在这里，说的是那条界本身，不抄 `u8` 的英文报错。
fn parse_white_align_limit(text: &str) -> anyhow::Result<WhiteAlignLimit> {
    text.trim()
        .parse::<u8>()
        .map(WhiteAlignLimit::new)
        .map_err(|_| anyhow::anyhow!("提白上限「{text}」要是 0 到 255 之间的整数级数，0 是关"))
}

fn spell<T: std::fmt::Display>(value: Option<T>, fallback: &str) -> String {
    match value {
        Some(value) => value.to_string(),
        None => format!("默认（{fallback}）"),
    }
}

fn spell_name<T: Copy + Default>(value: Option<T>, name: impl Fn(T) -> &'static str) -> String {
    match value {
        Some(value) => name(value).to_owned(),
        None => format!("默认（{}）", name(T::default())),
    }
}

/// 一个布尔项在屏上的写法。`taken` 是这一格**落到默认值之后**的取值，
/// 由 [`TasteLayer`] 那几个方法交出来（`taste.crop()`、`taste.split_rule().on`、……）。
///
/// **不收一个写死的默认值**：那会是处理选项默认值的第二份出处，而屏上那句「默认（裁）」
/// 与这一趟真拼出来的 `Request` 从此各说各的——处理选项改了向，屏上照旧印着旧话
/// （`p4-parking-lot/20` 验收第 4 条）。
///
/// **这一条没有用例钉得住，说清楚为什么。** 两头都从 [`TasteLayer`] 那几个方法取之后，
/// 「屏上说的等于真做的」就是同义反复：一条断言的两边落到同一个函数上，
/// 把 `taken` 换回字面量它照样绿——字面量此刻恰好等于默认值。
/// 拦住第二份出处的是**这个参数的名字与这段话**：它要的是「这一格最后取到什么」，
/// 不是「默认值是什么」，调用点因此写 `self.taste.crop()`。
/// `value` 与 `taken` 两个参数不合并，理由在
/// [`tests::saying_the_default_out_loud_still_reads_differently_from_saying_nothing`]。
fn spell_flag(value: Option<bool>, taken: bool, yes: &str, no: &str) -> String {
    let word = |flag: bool| if flag { yes } else { no };
    match value {
        Some(flag) => word(flag).to_owned(),
        None => format!("默认（{}）", word(taken)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// 每一个环转一圈都回得到出发点，而且**转一圈的长度就是那一项的取值个数**。
    fn ring_of<T: Clone + PartialEq + std::fmt::Debug>(start: T, next: impl Fn(T) -> T) -> Vec<T> {
        let mut seen = vec![start.clone()];
        let mut cursor = next(start.clone());
        while cursor != start {
            assert!(seen.len() < 64, "这个环转不回出发点：{seen:?}");
            seen.push(cursor.clone());
            cursor = next(cursor);
        }
        seen
    }

    /// **灰阶测试图按设备设置那块面板画，而画质门槛一格都不带**（13 号票的第六条：它仍是量具）。
    ///
    /// 型号与屏幕灰阶数进图——图的尺寸恒等于面板分辨率，排几条阶梯由屏幕灰阶数定；画质门槛不进，
    /// 因为灰阶测试图不经判定。会话这一路与命令行那一路走的是同一个 [`crate::target_profile`]，
    /// 两处解析出来的必须是同一块面板，因此这里问的是**这一层交出去了什么**。
    ///
    /// 同一份设备设置拼出的 `Request` 带着那个画质门槛：两个出口分得开，才谈得上「不带」。
    #[test]
    fn the_chart_is_drawn_for_the_panel_and_carries_no_threshold() {
        let mut session = Session::new();
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        session.device.threshold = Some(3.0);
        session.scope.out = Some(PathBuf::from("出"));

        let chart = session.chart_profile().expect("设备设置填齐了");
        assert_eq!(chart.device(), "boox-poke6");
        assert_eq!(chart.panel().gray_levels, 8, "屏幕灰阶数没进图");
        assert_eq!(
            chart.threshold(),
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .threshold(),
            "灰阶测试图带上了判定用的界"
        );
        // 同一份设备设置，跑一趟用的那个 profile 带着它——两个出口分得开。
        let running = session.request(RunMode::DryRun).expect("三组设置填齐了");
        assert_eq!(running.profile.threshold().value(), 3.0);

        // 型号没挑：说一句，不是画一张空图。
        let said = Session::new()
            .chart_profile()
            .expect_err("没有型号该说不出话")
            .to_string();
        assert!(said.contains("先挑型号"), "{said}");
    }

    /// 预览与转换拼出来的 `Request` **只差 `mode` 一格**，其余逐项相同。
    ///
    /// 跑不起来的那两种（型号没挑、输出目录没填）当场说得出口，而不是拼出一个
    /// 编造了默认值的请求交给库。
    #[test]
    fn a_trial_and_a_run_differ_only_in_how_far_they_go() {
        let mut session = Session::new();

        // 两项必填都缺：先说型号，那是判定的依据。
        let said = session.request(RunMode::DryRun).expect_err("跑不起来");
        assert!(format!("{said:#}").contains("先挑型号"), "{said:#}");

        session.device.profile = Some("kobo-libra-2".to_owned());
        let said = session.request(RunMode::DryRun).expect_err("跑不起来");
        assert!(format!("{said:#}").contains("输出目录"), "{said:#}");

        session.scope.out = Some(PathBuf::from("出"));
        session.scope.paths = vec![
            NamedPath {
                path: PathBuf::from("库/卷一"),
                on: true,
            },
            // 勾掉的那一条不进这一趟：打错一条勾掉就是了（spec 的 story 16）。
            NamedPath {
                path: PathBuf::from("库/卷二"),
                on: false,
            },
        ];

        let trial = session.request(RunMode::DryRun).expect("拼得出来");
        let run = session.request(RunMode::Process).expect("拼得出来");

        assert_eq!(trial.mode, tonefit::Mode::DryRun);
        assert_eq!(run.mode, tonefit::Mode::Process);
        assert_eq!(trial.inputs, vec![PathBuf::from("库/卷一")]);
        assert_eq!(trial.inputs, run.inputs);
        assert_eq!(trial.output_root, run.output_root);
        assert_eq!(trial.crop, run.crop);
        assert_eq!(trial.split, run.split);
        assert_eq!(trial.cache_budget, run.cache_budget);
    }

    /// 屏上「**没说**」与「**说了一个恰好等于默认的值**」仍是两句话
    ///（`p4-parking-lot/20` 验收第 5 条）。
    ///
    /// 三行布尔项各验一遍：没说那一格印「默认（裁）」，说了 `Some(true)` 印「裁」。
    /// 两者拼出来的 `Request` 一模一样，差别只在**存成预设时**才落到盘上
    /// （「这一项不写」与「这一项写着 `crop = true`」，见 [`Session::preset`] 与停车场 Q58）——
    /// 而屏上看不见的差别用户改不动。
    ///
    /// **收默认值那一步因此不能把 `Option` 拍平**：`spell_flag` 收的是
    /// 「说了没有」与「落到默认之后取到什么」两个东西，拍成一个具体值就一刀切掉了这个区分。
    #[test]
    fn saying_the_default_out_loud_still_reads_differently_from_saying_nothing() {
        let mut session = Session::new();

        for (field, spoken, said, silent) in [
            (Field::Crop, true, "裁", "默认（裁）"),
            (Field::Split, true, "拆", "默认（拆）"),
            (Field::Envelope, false, "关", "默认（关）"),
        ] {
            assert_eq!(session.shown(field), silent, "{field:?} 没说那一格");

            match field {
                Field::Crop => session.taste.crop = Some(spoken),
                Field::Split => session.taste.split = Some(spoken),
                Field::Envelope => session.taste.envelope = Some(spoken),
                _ => unreachable!("上面那张表只有这三行"),
            }
            assert_eq!(
                session.shown(field),
                said,
                "{field:?} 说了一个恰好等于默认的值，屏上却与没说是同一句"
            );
        }
    }

    /// 一项都没改的会话拼出来的，与**一个 flag 都不加的命令行**拼出来的**逐格相同**
    ///（`p4-parking-lot/20` 验收第 4 条）。
    ///
    /// 「命令行没点、预设也没说」那一档与会话读的是同一个（`preset::TasteLayer` 那几个
    /// 方法），这一条钉的就是那件事：默认值没有第二个出处。
    ///
    /// **比的是整份 `Request` 的 `Debug`，不是挑几个字段**，与命令行那一侧的
    /// `crate::tests::request_line` 同一条道理：挑着比就等于在用例里自己重列一遍那张单子，
    /// 而**漏掉一项**恰恰是这条要防的事——往 `Request` 添一格、两边各填各的，
    /// 逐项那种写法一个字都不会说。观察者两边都是 `None`，比得起来。
    #[test]
    fn an_untouched_session_asks_for_what_a_bare_command_line_asks_for() {
        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.paths = vec![NamedPath {
            path: PathBuf::from("库/卷一"),
            on: true,
        }];

        let asked = session.request(RunMode::Process).expect("拼得出来");
        let command_line = crate::Cli::try_parse_from([
            "tonefit",
            "--profile",
            "kobo-libra-2",
            "--out",
            "出",
            "库/卷一",
        ])
        .expect("命令行读得懂")
        .request(&crate::preset::Preset::default())
        .expect("拼得出来");

        assert_eq!(
            format!("{asked:?}"),
            format!("{command_line:?}"),
            "会话与光命令行拼出来的不是同一份"
        );
    }

    /// 库那一侧的取值环转得回来，长度也对得上——环是穷尽 match 写的，
    /// 这一条守的是「写反了」而不是「漏了一个」（漏了的话编译就不过）。
    #[test]
    fn the_value_rings_come_back_around() {
        assert_eq!(ring_of(FitMode::Height, next_fit).len(), 2);
        assert_eq!(ring_of(ReadingOrder::RightToLeft, next_order).len(), 2);
        assert_eq!(ring_of(Filter::Area, next_filter).len(), 5);
        assert_eq!(ring_of(IoMode::Auto, next_io_mode).len(), 3);
        assert_eq!(ring_of(BitDepth::One, next_bit_depth).len(), 4);
        // 抖动与三个布尔项转起来时环上都多一格「没说」。
        assert_eq!(
            ring_of(None::<Dither>, |dither| ring(
                dither,
                Dither::Off,
                next_dither
            ))
            .len(),
            3
        );
        assert_eq!(
            ring_of(None::<bool>, |flag| ring(flag, true, next_flag)).len(),
            3
        );
        // 型号环走的是内置表，「没挑」是环上的一格。
        let devices = Profile::devices().count();
        let ring = ring_of(None::<String>, |device| next_device(device.as_deref()));
        assert_eq!(ring.len(), devices + 1);
    }

    /// 画质门槛那一行印的是**数值加标定来源**，与报告里那一行逐字相同。
    ///
    /// spec 的 Further Notes：「会话里显示时照报告的写法把来源原样带上来，
    /// 不自己另编一套说法。」标定来源是画质门槛的一部分（ADR 0002：画质分跟着面板走、
    /// 不可跨面板比较），会话另写一句就等于把那半句丢了。
    #[test]
    fn the_threshold_row_prints_the_source_the_report_prints() {
        let mut session = Session::new();

        // 还没挑型号：界挂在 profile 上，这一行印不出数来。
        assert!(!session.shown(Field::Threshold).contains("画质门槛"));

        session.device.profile = Some("boox-poke6".to_owned());
        let printed = session.shown(Field::Threshold);

        assert_eq!(
            printed,
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .threshold()
                .to_string(),
            "会话另编了一套说法"
        );
        assert!(printed.contains("其他屏幕未验证"), "{printed}");

        // 会话里点了名的那个数同样照库那一份印，而那句话**不提入口**——在会话里打的数
        // 说成「命令行指定」是停车场 Q62 记的那件事。
        session.device.threshold = Some(2.0);
        let pinned = session.shown(Field::Threshold);

        assert_eq!(
            pinned,
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .with_threshold(2.0)
                .expect("2.0 在界的取值范围内")
                .threshold()
                .to_string(),
            "会话另编了一套说法"
        );
        assert!(pinned.contains("由你指定"), "{pinned}");
    }

    /// 屏上的两层与预设装的两层是**同一层**：格数一项不多一项不少。
    ///
    /// 断的**不是** `TASTE_FIELDS.len() == 12`——那个数写在类型里，永远红不了。
    /// 断的是它与**盘上那份预设**的格数对得上：`preset::write` 把一份说满了的
    /// `Preset`（`preset::every_field`，没有 `..Default::default()`）写成 TOML，
    /// 那两节里各有几个键，两层就各有几格。往 `TasteLayer` 加一个字段而设置栏没跟着加一行，
    /// 这一条当场变红。
    ///
    /// 顺带把「每一行都印得出取值」一起断言：新字段没有 [`Session::shown`] 的分支就编译不过，
    /// 而印出个空串是另一种漏。
    #[test]
    fn the_two_layers_on_screen_are_the_two_layers_a_preset_stores() {
        let text = crate::preset::write(&std::collections::BTreeMap::from([(
            "说满了".to_owned(),
            crate::preset::every_field(),
        )]))
        .expect("写得出来");
        let on_disk: toml::Value = toml::from_str(&text).expect("读得回来");
        let spelled = |section: &str| {
            on_disk["preset"]["说满了"][section]
                .as_table()
                .unwrap_or_else(|| panic!("预设里该有 {section} 那一节"))
                .len()
        };

        assert_eq!(spelled("taste"), TASTE_FIELDS.len(), "处理选项：{text}");
        assert_eq!(spelled("device"), DEVICE_FIELDS.len(), "设备设置：{text}");

        let session = Session::new();
        for field in DEVICE_FIELDS.into_iter().chain(TASTE_FIELDS) {
            assert!(!field.label().is_empty(), "{field:?} 没有名字");
            assert!(!session.shown(field).is_empty(), "{field:?} 印不出取值");
        }
    }

    /// 设备设置与处理选项里**取值是环**的那几行（型号那一行不在里面：它摊开的是面板，
    /// 由 `super::config` 那一头的用例问）。
    fn cyclable() -> Vec<Field> {
        DEVICE_FIELDS
            .into_iter()
            .chain(TASTE_FIELDS)
            .filter(|field| field.shape() == Shape::Cycle && !field.drills())
            .collect()
    }

    /// **每一项都有一个「没说」的位置，取值环从它起、走一圈落回它**——预设的 `Option`
    /// 在屏上就长这样。环的第一格就是「没说」那一格，定到第几格屏上就印第几格，
    /// 定回第一格又是「没说」。
    #[test]
    fn every_setting_has_a_says_nothing_position_and_comes_back_to_it() {
        let fresh = Session::new();
        for field in cyclable() {
            let (cells, chosen) = fresh.ring(field);
            assert!(cells.len() >= 2, "{field:?} 的取值环只有一格");
            assert_eq!(chosen, 0, "{field:?} 还没说过话，生效的却不是第一格");
            assert_eq!(cells[0], fresh.shown(field), "{field:?} 第一格不是「没说」");
            assert!(
                cells[0].starts_with("默认") || matches!(field, Field::BitDepth | Field::Dither),
                "{field:?} 第一格印成了 {}",
                cells[0]
            );

            let mut session = Session::new();
            for (at, cell) in cells.iter().enumerate() {
                session.settle(field, at);
                assert_eq!(&session.shown(field), cell, "{field:?} 定到第 {at} 格");
                assert_eq!(session.ring(field).1, at, "{field:?} 生效的那一格没跟着");
                assert_eq!(session.unsaid(field), at == 0, "{field:?} 第 {at} 格");
            }
        }
    }

    /// **表外的型号定回「没挑」**（停车场 Q140）：套用一份写着已删型号的预设之后，
    /// 型号那一格不在环上，从它出发往前一步落回的是「没挑」，定到第一格也是它。
    #[test]
    fn a_model_outside_the_table_falls_back_to_nothing_picked() {
        assert!(
            !Profile::devices().any(|device| device == "kobo-glo-hd"),
            "这个名字进了内置表，这一条就没问到东西"
        );
        let mut session = Session::new();
        session.device.profile = Some("kobo-glo-hd".to_owned());
        assert!(!session.unsaid(Field::Profile));

        session.settle(Field::Profile, 0);

        assert_eq!(session.device.profile, None);
    }

    /// 换型号把设备设置那两个覆盖项一起清掉：它们是在**上一块面板**上量出来的（ADR 0002）。
    /// 写型号只有 [`Session::set_device`] 一条路，定到环上哪一格走的也是它。
    #[test]
    fn changing_the_model_drops_the_numbers_measured_on_the_old_one() {
        let mut session = Session::new();
        session.set_device(Some("boox-poke6".to_owned()));
        session.device.gray_levels = Some(12);
        session.device.threshold = Some(5.2);

        session.settle(Field::Profile, 2);

        assert_ne!(session.device.profile.as_deref(), Some("boox-poke6"));
        assert_eq!(session.device.gray_levels, None, "屏幕灰阶数跟着换了面板");
        assert_eq!(session.device.threshold, None, "画质门槛跟着换了面板");
    }

    /// 设备设置那两个覆盖项**要先挑型号**——与预设那一侧同一条规矩（ADR 0002）。
    /// 挑了型号之后同一个数收得下，越界的数仍被库那一侧的界挡下，已收下的值不动。
    #[test]
    fn a_calibrated_override_needs_a_panel_to_be_measured_on() {
        let mut session = Session::new();

        let said = session
            .take(Field::GrayLevels, "12")
            .expect_err("没挑型号该挡下");
        assert!(format!("{said:#}").contains("先挑型号"), "{said:#}");
        assert_eq!(session.device.gray_levels, None);

        session.device.profile = Some("boox-poke6".to_owned());
        session
            .take(Field::GrayLevels, "12")
            .expect("挑了型号收得下");
        assert_eq!(session.device.gray_levels, Some(12));

        session
            .take(Field::GrayLevels, "0")
            .expect_err("0 级该被挡下");
        assert_eq!(session.device.gray_levels, Some(12), "挡下的值没有写进去");
    }

    /// 打错的取值收不下、说得出为什么，已有的那一格不动；改对了就收得下。
    #[test]
    fn a_value_that_does_not_parse_is_refused_and_changes_nothing() {
        let mut session = Session::new();

        assert!(session.take(Field::CacheBudget, "512T").is_err());
        assert_eq!(session.taste.cache_budget, None);

        session
            .take(Field::CacheBudget, "512M")
            .expect("认得的写法");
        assert_eq!(
            session.taste.cache_budget,
            Some(CacheBudget::parse("512M").expect("认得的写法"))
        );
    }

    /// 空串是「没说」：清掉一项就落回默认值，而不是落到 0 上。进输入行时缓冲里摆的是当前取值。
    #[test]
    fn clearing_a_field_puts_it_back_to_saying_nothing() {
        let mut session = Session::new();
        session.taste.cache_budget = Some(CacheBudget::parse("1G").expect("认得的写法"));
        assert_eq!(session.typed(Field::CacheBudget), "1G");

        session
            .take(Field::CacheBudget, "")
            .expect("空串是「没说」");

        assert_eq!(session.taste.cache_budget, None);
        assert!(session.shown(Field::CacheBudget).starts_with("默认"));
    }

    /// **提白上限在会话里调得动，改完下一趟生效**（纸色提白批 04 号票的验收）。
    ///
    /// **打 0 是「关」，是一个说了的值**——拼出来的 `Request` 照它走、不落到默认上，
    /// 存成预设时 `white-align-limit = 0` 写出去。清空才是「没说」，落回默认值。
    /// 越界的数（256）收不下，已收下的值不动。
    #[test]
    fn the_white_align_limit_is_edited_in_the_session_and_takes_effect_next_run() {
        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(PathBuf::from("出"));
        let limit_of = |session: &Session| {
            session
                .request(RunMode::Process)
                .expect("拼得出来")
                .white_align_limit
        };
        let stored = |session: &Session| {
            crate::preset::write(&std::collections::BTreeMap::from([(
                "存出去".to_owned(),
                session.preset_to_store(),
            )]))
            .expect("写得出来")
        };

        // 一、没说：屏上印默认值，拼出来的请求走默认，预设里不写这一项。
        assert!(TASTE_FIELDS.contains(&Field::WhiteAlignLimit));
        assert!(session.shown(Field::WhiteAlignLimit).starts_with("默认"));
        assert_eq!(limit_of(&session), WhiteAlignLimit::default());
        assert!(!stored(&session).contains("white-align-limit"));

        // 二、打 0：关掉。
        session.take(Field::WhiteAlignLimit, "0").expect("收得下");
        assert_eq!(session.taste.white_align_limit, Some(WhiteAlignLimit::OFF));
        assert_eq!(
            session.shown(Field::WhiteAlignLimit),
            "0",
            "0 是说了的值，不印「默认」"
        );
        assert_eq!(limit_of(&session), WhiteAlignLimit::OFF, "下一趟该按关掉走");
        assert!(
            stored(&session).contains("white-align-limit = 0"),
            "存成预设时 0 该写出去：{}",
            stored(&session)
        );

        // 三、越界的数收不下，已收下的值不动。
        assert!(session.take(Field::WhiteAlignLimit, "256").is_err());
        assert_eq!(session.taste.white_align_limit, Some(WhiteAlignLimit::OFF));

        // 四、改成 2 收得下；清空落回「没说」。
        session.take(Field::WhiteAlignLimit, "2").expect("收得下");
        assert_eq!(limit_of(&session), WhiteAlignLimit::new(2));
        session.take(Field::WhiteAlignLimit, "").expect("收得下");
        assert_eq!(session.taste.white_align_limit, None);
        assert!(session.shown(Field::WhiteAlignLimit).starts_with("默认"));
        assert_eq!(limit_of(&session), WhiteAlignLimit::default());
    }

    /// **两级停止：闩只升不降**（ADR 0013）。一次做完再停，再一次立即停止，第三次没有
    /// 更强的一级可去；没跑着时闩不动；那一趟结束之后闩跟着这一趟一起走。
    #[test]
    fn the_stop_latch_only_goes_up_and_leaves_with_its_run() {
        let mut session = Session::new();
        session.raise_stop();
        assert_eq!(session.stopping(), Instruction::Continue, "没跑着也升了闩");

        session.run_started();
        assert_eq!(session.stopping(), Instruction::Continue, "起手没按过");
        session.raise_stop();
        assert_eq!(session.stopping(), Instruction::Finish);
        session.raise_stop();
        assert_eq!(session.stopping(), Instruction::Abort);
        session.raise_stop();
        assert_eq!(session.stopping(), Instruction::Abort, "闩退回去了");

        session.run_finished();
        assert_eq!(session.stage(), Stage::Ended);
        assert_eq!(session.stopping(), Instruction::Continue);
        // 下一趟从头起：上一趟按下的停没有漏过来。
        session.run_started();
        assert_eq!(
            session.stopping(),
            Instruction::Continue,
            "上一趟的停漏过来了"
        );
    }

    /// **等待确认是同一趟的另一副样子，闩原样带过去**（`p1-session/14`）：没跑着时这一问
    /// 不作数；答完话当场转回跑着，闩仍是那一级——答话不是升闩。
    #[test]
    fn the_decision_point_is_a_second_face_of_the_same_run_and_leaves_the_latch_alone() {
        let mut session = Session::new();
        session.at_the_decision_point(true);
        assert_eq!(session.stage(), Stage::Fresh, "没跑着也进了等待确认");

        session.run_started();
        session.raise_stop();
        session.at_the_decision_point(true);
        assert_eq!(session.stage(), Stage::Deciding(Instruction::Finish));

        session.answered();
        assert_eq!(
            session.stage(),
            Stage::Running(Instruction::Finish),
            "答完话没转回去，或者把闩推上去了"
        );

        // 停在确认点上时那一趟结束，照样回到结束了，闩跟着这一趟一起走。
        session.at_the_decision_point(true);
        session.run_finished();
        assert_eq!(session.stage(), Stage::Ended);
        assert_eq!(session.stopping(), Instruction::Continue);
    }
}
