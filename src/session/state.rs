//! 会话的状态机：三层配置、光标停在哪、哪个键在这个状态下做什么。
//!
//! **这个模块一个终端都不碰。** 它不 use 终端库，也不读键盘——[`Key`] 是本模块自己的
//! 一个小枚举，把 crossterm 那一侧的键码翻译过来是 [`super`] 那一层十几行的事。
//! spec 的 story 44 要的「会话的状态机脱离终端可测」因此是**结构上**成立的，
//! 不是「这批用例碰巧没开终端」：本模块的用例连终端库都编译不到。
//!
//! 按键与状态的规矩由 [`Session::action`] 一处答完——它是纯函数，
//! 「哪些键在哪个状态下有效」那张表就是它，用例直接问它
//! （本模块的 `which_keys_do_what_in_which_state` 逐条问过它）。
//! [`Session::act`] 只是「把 `action` 说的那件事做掉」，用例那一侧的
//! [`Session::press`] 把两步并成一步。
//!
//! # 三层照预设那一份分
//!
//! 设备层与口味层**就是** [`DeviceLayer`] 与 [`TasteLayer`]——预设装的那两层
//! （`p1-session/07`），不是另立的一套。三层的分界线画在生命周期上
//! （`CONTEXT.md` 的《会话》），而分界线只有一处出处才谈得上是同一条线：
//! 会话里配好的两层存成预设（`p1-session/12`）时不必再做一次搬运。
//!
//! 范围层只在会话里有（[`ScopeLayer`]）：它每趟都不同，**不进预设**，
//! 而命令行那一侧它就是 `--out` 与那几个位置参数。
//!
//! # 每一项都有一个「没说」的位置
//!
//! 两层的每一格都是 `Option`——预设**只说它说到的那几项**（见 [`TasteLayer`]）。
//! 会话照搬这条：每一项的取值环上第一格是「默认」，转一圈回得到它。
//! 「没说」与「说了一个恰好等于默认值的值」因此在屏上分得开，
//! 而那正是存成预设时两者的差别。

use std::path::{Path, PathBuf};

use tonefit::{
    BitDepth, CacheBudget, Dither, Filter, FitMode, Instruction, IoMode, Mode as RunMode, Profile,
    ReadingOrder, Request, SplitThreshold,
};

use super::complete;
use crate::preset::{DeviceLayer, Preset, TasteLayer};

/// 会话认得的按键。**不是终端库那一侧的键码**——那一层的翻译在 [`super::translate`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Space,
    Tab,
    /// `⇧⇥`。终端把它报成一个**单独的**键码（crossterm 的 `BackTab`），
    /// 不是「Tab 加一个修饰键」——认它的地方因此与 [`Self::Tab`] 各占一支。
    BackTab,
    Backspace,
    Esc,
    Char(char),
    /// Ctrl-C。它在**每一个**状态下都是退出，编辑到一半也是。
    Interrupt,
}

/// 换一个取值时往哪边转。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Back,
}

/// 报告区往哪边挪一下。**四个方向不是两个 [`Step`]**：上下挪的是行、左右挪的是列，
/// 两根轴上一格的大小都不同（见 [`SIDEWAYS`]），合成一个取值只会让调用方再分一次岔。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    Up,
    Down,
    Left,
    Right,
}

/// 报告区横着滚一下走几列。
///
/// 一列一列太慢：逐页那两行轻松过 100 列，而窄终端上要挪的正是那几十列。
/// 取八列——比一个汉字宽，按一下看得出屏在动，又不至于一下跳过半屏。
const SIDEWAYS: u16 = 8;

/// 一个键在当前状态下会做的那件事。**这就是「哪些键在哪个状态下有效」那张表的值域。**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 光标上下移动。
    Move(Step),
    /// 就地把当前项换成取值环上的下一个（或上一个）。
    Cycle(Step),
    /// 把这一卷勾上或勾掉。只有范围层的卷行有它。
    Toggle,
    /// 进入编辑：文本项与路径项走这条。
    Edit,
    /// 把这一行的卷整条删掉。
    Remove,
    /// 编辑中：往缓冲里添一个字。
    Insert(char),
    /// 编辑中：退掉一个字。
    Backspace,
    /// 编辑中：**逐层补全**。只有路径项有它。
    Complete,
    /// 编辑中：收下缓冲里的东西。
    Commit,
    /// 编辑中：丢掉缓冲，回到浏览。
    Cancel,
    /// 起一趟：[试算](RunMode::DryRun)或[执行](RunMode::Process)。
    ///
    /// 两者**是同一件事的两半**，区别只在做到哪一步（`CONTEXT.md` 的《会话》：
    /// 试算就是会话里按下去的那一次 dry-run），因此是一个带参数的动作、不是两个变体——
    /// 分成两个，「试算与执行走同一条回路」这句话就得靠自觉。
    ///
    /// 真把线程起起来的那件事**不在状态机里**（见 [`Session::apply`]）：
    /// 本模块一个终端都不碰，也不该起线程。
    Start(RunMode),
    /// **按停**：把[闩](Session::stopping)往上升一级——没按过就是收尾，按过一次就是中止
    /// （ADR 0013）。
    ///
    /// 一个动作，不是两个：两级停是**同一个键按两次**（`CONTEXT.md` 的《会话》：
    /// 「中止是**再按一次**」）。分成 `Finish` 与 `Abort` 两个动作，
    /// 「再按一次退不回上一级」就得靠键盘上有没有第二个键来保证，而那不是一条性质。
    ///
    /// 升到哪一级由状态机自己记（[`Mode::Running`] 那一格）；把它交给跑着的那一趟
    /// 在 [`super::press`] 那一层——本模块不碰线程，与[起一趟](Self::Start)同一条规矩。
    Stop,
    /// **在决策点上答一个字**（`CONTEXT.md` 的《会话》：决策点）。
    ///
    /// 只在 [`Mode::Deciding`] 那个状态下派得出来，而那个状态只有单卷试算到得了。
    /// 两个字各有一个键：`x` 答[继续](Instruction::Continue)——第一遍不重算，直接进第二遍；
    /// `s` 答[收尾](Instruction::Finish)——这一卷一个字节都不写，等价于一次 dry-run。
    ///
    /// **它带着那个字，而不是像[按停](Self::Stop)那样让状态机自己升一级**：
    /// 决策点回的是**当场那个字**，不是闩（ADR 0012 决定第 2 条）。两个键因此是两个方向，
    /// 不是同一个键按两次。
    ///
    /// [中止](Instruction::Abort)不从这条路出去：等答话时按 `Ctrl-C` 是[退出会话](Self::Quit)，
    /// 而退出会话本来就走中止（`super::run::Running::leave`，停车场 Q63）——
    /// 那一卷等于没做，`partial` 也没留下。
    ///
    /// 把那个字送到计算线程上在 [`super::press`] 那一层（`Running::decide`），
    /// 与[按停](Self::Stop)同一条规矩：本模块不碰线程。
    Answer(Instruction),
    /// **展开**：把报告上第一卷的逐页那几行摊开来，左栏跟着收起
    /// （`CONTEXT.md` 的《会话》：展开）。
    ///
    /// 展开哪一卷、报告从第几行画起，都要读那一趟攒下来的报告，而本模块读不到它
    /// （攒着的那一份在 [`super::live::Live`] 上）。真做这件事的因此是
    /// [`super::press`]——与[起一趟](Self::Start)同一条分法。
    Expand,
    /// 换展开的那一卷：往后一卷或往前一卷，两头都转一圈。
    ///
    /// 带方向而不是只往后：几十卷的一趟里，往回看一卷不该按二十九下
    /// （票面：**选中一卷**可展开逐页）。方向用 [`Step`]，与三层那几个取值环
    /// 同一个取值——两处都是「在一圈上挪一格」。
    ///
    /// 与[展开](Self::Expand)同样落在 [`super::press`] 那一层：换完要把视口对到
    /// 那一卷的抬头上，而那是数报告有几行的事。
    Turn(Step),
    /// **收起**：逐页那几行收起来，左栏回来。
    ///
    /// 与展开不对称——收起不必读报告，因此它就在本模块做掉。
    Collapse,
    /// 展开着的报告区往那个方向挪一下。上下一行、左右 [`SIDEWAYS`] 列。
    Scroll(Toward),
    /// **去预设那一栏**：把盘上那份文件里有的那几份列出来（`CONTEXT.md` 的《会话》：预设栏）。
    ///
    /// 列什么要读盘，而本模块读不到——真做这件事的是 [`super::press`]，它随后调
    /// [`pick`](Session::pick)。与[展开](Self::Expand)同一条分法：那一支要读那一趟攒的报告，
    /// 这一支要读用户配置目录下那份 TOML；名字也照那一对取
    /// （`Expand` 进 [`Mode::Expanded`]，`Pick` 进 [`Mode::Picking`]）。
    Pick,
    /// **套用**光标停着的那一份预设：两层整个换成它，范围层一格不动。
    ///
    /// 同样落在 [`super::press`]：那一份的内容要现读（[`Presets::read`](crate::preset::Presets::read)），
    /// 而**读不懂的预设当场报错、不静默套默认值**（spec 的 story 39）——报出来的那句话
    /// 是库那一侧的原话，会话不另编一份。
    Take,
    /// **存**：把当前两层存成缓冲里打的那个名字。
    ///
    /// 落在 [`super::press`]，理由与上面两支同一条：它要往盘上写东西。
    /// 撞上同名的那一份时**不覆盖**——那一层先说一句，再按一次这个键才覆盖
    /// （见 [`Session::name_is_taken`]）。
    Store,
    /// **出标定图**：按当前设备层那块面板画一张，写到盘上（`CONTEXT.md` 的《会话》：
    /// 设备层里还挂着标定图）。
    ///
    /// **只在光标停在设备层上时派得出来**（见 [`Session::browsing_action`]）：
    /// 它出的那个数——感知可分辨级数——正是设备层唯一填不出来的一格，
    /// 而这个键与那一格挨着才说得出它是干什么用的。停在别的层上按它是 [`Ignored`](Self::Ignored)。
    ///
    /// 落在 [`super::press`]，与预设那三支同一条：它要往盘上写东西，而本模块碰不到盘。
    /// 真正画图与落盘的是库里那第三个 seam（[`tonefit::write_calibration_chart`]）——
    /// 会话一格像素都不拼、一个目录都不建。
    Chart,
    /// 退出会话。
    Quit,
    /// 这个键在这个状态下没有意义。**它是一个取值，不是遗漏**——
    /// 「编辑路径时上下键不动光标」这种规矩正是靠它说出来的。
    Ignored,
}

/// 按下一个键之后会话还开不开着。
///
/// **不叫 `Outcome`。** 那个词在 `CONTEXT.md` 里已经有主：**收场**（`RunOutcome`——
/// 这一趟是怎么结束的），与「会话还开不开着」不是一回事，同名会让两者迟早被看成一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 会话还开着。
    Stay,
    /// 退出会话。
    Leave,
}

/// 三层。分界线画在**生命周期**上，不画在「哪几个 flag 长得像」上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Device,
    Taste,
    Scope,
}

impl Layer {
    /// 左栏上这一块的抬头。括号里那半句说的正是这一层错了会怎样——
    /// 三层为什么分成三层，屏上就得看得见。
    pub fn title(self) -> &'static str {
        match self {
            Layer::Device => "设备层 · 判定的依据，绑面板，改一次管很久",
            Layer::Taste => "口味层 · 这一趟的立场",
            Layer::Scope => "范围层 · 每趟都不同，不进预设",
        }
    }
}

/// 左栏上可改的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    // 设备层
    Profile,
    GrayLevels,
    Threshold,
    // 口味层
    Fit,
    Crop,
    Split,
    SplitThreshold,
    ReadingOrder,
    Filter,
    BitDepth,
    Dither,
    PerPage,
    CacheBudget,
    IoMode,
    // 范围层
    Out,
    /// 已经打进来的第 n 个卷。
    Volume(usize),
    /// 再打一个卷进来的那一行。
    AddVolume,
}

/// 设备层的三项，次序就是屏上的次序（`p1-session/07` 的分法）。
pub const DEVICE_FIELDS: [Field; 3] = [Field::Profile, Field::GrayLevels, Field::Threshold];

/// 口味层的十一项，次序就是屏上的次序（`p1-session/07` 的分法）。
///
/// 前五项是页几何那一批添的（适配方式、裁边、拆分与阈值、阅读方向），
/// 后六项是 spec 的《会话：三层与预设》原本就列着的那几项。
/// 这张单子与 [`TasteLayer`] 的字段**一一对应**，由本模块的
/// `the_taste_layer_on_screen_is_the_taste_layer_a_preset_stores` 拴住。
pub const TASTE_FIELDS: [Field; 11] = [
    Field::Fit,
    Field::Crop,
    Field::Split,
    Field::SplitThreshold,
    Field::ReadingOrder,
    Field::Filter,
    Field::BitDepth,
    Field::Dither,
    Field::PerPage,
    Field::CacheBudget,
    Field::IoMode,
];

/// 一行怎么改。按键表按它分派（见 [`Session::action`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// 打字改：数、界、字节数。
    Text,
    /// 打字改，而且 `Tab` **逐层补全**。
    Path,
    /// 左右键在取值环上转。
    Cycle,
    /// 卷行：勾上／勾掉，另有一个删掉它的键。
    Volume,
}

impl Field {
    /// 这一行归哪一层。
    ///
    /// 逐个变体都列出来，**不留 `_`**：新添一行该归哪一层是个要当场拿的主意，
    /// 而 `_` 会替它默默拿成口味层（与本模块那几个取值环同一条规矩）。
    pub fn layer(self) -> Layer {
        match self {
            Field::Profile | Field::GrayLevels | Field::Threshold => Layer::Device,
            Field::Fit
            | Field::Crop
            | Field::Split
            | Field::SplitThreshold
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::PerPage
            | Field::CacheBudget
            | Field::IoMode => Layer::Taste,
            Field::Out | Field::Volume(_) | Field::AddVolume => Layer::Scope,
        }
    }

    /// 左栏上这一行的名字。
    pub fn label(self) -> &'static str {
        match self {
            Field::Profile => "型号",
            Field::GrayLevels => "感知可分辨级数",
            Field::Threshold => "阈值",
            Field::Fit => "适配方式",
            Field::Crop => "裁边",
            Field::Split => "跨页拆分",
            Field::SplitThreshold => "拆分阈值",
            Field::ReadingOrder => "阅读方向",
            Field::Filter => "滤波器",
            Field::BitDepth => "位深",
            Field::Dither => "抖动",
            Field::PerPage => "逐页",
            Field::CacheBudget => "缓存预算",
            Field::IoMode => "读取策略",
            Field::Out => "输出根",
            Field::Volume(_) => "卷",
            Field::AddVolume => "＋ 再打一个卷进来",
        }
    }

    /// 这一行怎么改。逐个变体都列出来，理由与 [`layer`](Self::layer) 同一条。
    pub fn shape(self) -> Shape {
        match self {
            Field::GrayLevels | Field::Threshold | Field::SplitThreshold | Field::CacheBudget => {
                Shape::Text
            }
            Field::Out | Field::AddVolume => Shape::Path,
            Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::PerPage
            | Field::IoMode => Shape::Cycle,
            Field::Volume(_) => Shape::Volume,
        }
    }
}

/// 范围层：这一趟点名哪几个卷、写到哪儿。
///
/// **不进预设**（`preset` 模块的抬头写着为什么）：它每趟都不同，
/// 混进预设会让人套用时误写到上一次的输出目录（ADR 0009）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeLayer {
    /// 输出根。每个卷在它下面得到一份同名副本。
    pub out: Option<PathBuf>,
    /// 点名的那些卷，按打进来的次序。
    pub volumes: Vec<Picked>,
}

/// 范围层里点名的一个卷，连同它这一趟算不算数。
///
/// **不叫 `Volume`。** `CONTEXT.md` 的**卷**是一次处理调用的作用域（库里 `source::Volume`
/// 是打开了的那一个，带着成员表与读取端）；这里装的是「用户点了它」这件事——
/// 一条路径加一个勾，连点不点得开都还没问过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub path: PathBuf,
    /// 勾着的才进这一趟。**打错一条勾掉就是了，不必把整份重打一遍**（spec 的 story 16）。
    pub on: bool,
}

/// 会话此刻在做什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// 光标在左栏上走。
    Browsing,
    /// 某一行正在被打字改。
    Editing(Edit),
    /// 一趟正跑着，带着**按停按到哪一级了**。
    ///
    /// **三层全只读**，两层各错在什么地方见 `CONTEXT.md` 的《会话》（`p1-session/10`
    /// 把范围层也算了进来，停车场 Q69）。
    ///
    /// 「只读」不是靠画法上灰，是靠 [`Session::action`] 在这个状态下一个改动键都不派
    /// （见 [`running_action`]）。画法那一侧另有一份**看得出来**的交代
    /// （左栏抬头写着「只读」、光标不反白），那是 [`super::draw::config`] 的事。
    ///
    /// 那一格装的是[闩](Session::stopping)：`Continue` 是没按过、`Finish` 是按过一次
    /// （收尾）、`Abort` 是再按了一次（中止）。**只升不降**——按停不是一个可以反悔的开关
    /// （`CONTEXT.md` 的《进度》）。
    Running(Instruction),
    /// 一趟**停在决策点上等人拿主意**（`CONTEXT.md` 的《会话》：续做与决策点，
    /// ADR 0012 决定第 3 条）。
    ///
    /// 只有**单卷试算**到得了这里：多卷不续做（决定第 1 条，理由是内存不是口味），
    /// 执行那一趟也不在这儿停——用户按 `x` 的时候已经拿过主意了。
    ///
    /// **它是一个状态，不是 [`Running`](Self::Running) 上的一个开关。**跑着与等答话
    /// 按得动的键是两套：跑着时按得动的只有停（`s`），等答话时按得动的是答话那两个
    /// （`x` 接着做、`s` 收尾）。两套摆进同一个状态，屏底那一行就要靠一个 flag 分岔，
    /// 而「哪些键在哪个状态下有效」那张表正是本模块唯一的产出。
    ///
    /// 三层在这一刻**仍然只读**，与跑着时一个待遇：`Request` 在起线程那一刻就是一份快照，
    /// 而这一趟还没收场（见 [`deciding_action`]）。
    ///
    /// 那一格装的还是[闩](Session::stopping)：在决策点上等着的时候，闩记着的是这一趟
    /// **此前**按过的停。答完话回 [`Running`](Self::Running) 时它原样带回去——
    /// 决策点上答的字不是闩，两者互不覆盖。
    Deciding(Instruction),
    /// 报告区**展开**着一卷的逐页，左栏收着（`CONTEXT.md` 的《会话》：展开）。
    ///
    /// 「展开」与「左栏收起」是**同一件事**，不是两个开关：spec 的《会话：布局与交互》
    /// 写的是「展开逐页时左栏收起、主区吃满宽度」——逐页那两行轻松过 100 列，
    /// 而左栏那 52 列在这一刻是宽度里最贵的一截。分成两个开关的话，
    /// 「展开着而左栏还在」这种没人要的组合就得靠某处代码守着。
    ///
    /// **收起不是删掉**：收起来的那些行原样回得来（[`Action::Collapse`] 只把这个状态
    /// 换回 [`Browsing`](Self::Browsing)，三层一格没动，光标也还停在原处）。
    ///
    /// **跑着的时候展不开**，而这是本票拿的主意（停车场 Q72）：报告那时还在长，
    /// 而那一刻按得动的只有停。`Mode` 因此仍是一维的，与 `p1-session/10` 拿的那一条
    /// （范围层跟着冻住、不把 `Mode` 拆成两维）同一个形状。
    Expanded(Expansion),
    /// **预设那一栏**开着：盘上有的那几份摆成一列，末尾一行是「存成一份新的」。
    ///
    /// 与[展开](Self::Expanded)同一个形状：一个从浏览进得去、一个键退得回来的状态，
    /// 三层一格没动（**套用**才动，而那是用户在这一栏上按下去的那一下）。
    ///
    /// **跑着的时候开不了**：套用一份预设就是把两层整个换掉，而跑起来之后三层只读
    /// （`CONTEXT.md` 的《会话》）。这与 `e` 跑着时按不动（停车场 Q72）是同一条。
    Picking(Picker),
}

/// 预设那一栏：**盘上有的那几份**，加上末尾「存成一份新的」那一行。
///
/// 列的是**进这一栏那一刻**盘上有的（[`super::press`] 读的，见 [`Action::Pick`]）：
/// 本模块碰不到盘。这与 [`Expansion::volumes`] 是同一种「进来那一刻记下的数」。
///
/// 末尾那一行照范围层「＋ 再打一个卷进来」的样子（[`Field::AddVolume`]）：
/// **列一份清单与往清单里添一条是同一栏上的两件事**，分成两个键就要多记一个键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    /// 这一栏是**哪一份文件**列出来的。
    ///
    /// 屏上要说得出它（见 [`super::draw`]）：存出去的东西落在用户自己的配置目录里，
    /// 而下一次多半是在命令行上用它——「那份文件在哪」是接着要做的事的前提。
    /// 摆在这一栏里而不是摆进那句话里，是因为**那句话在窄终端上会被切掉**
    /// （屏底那一格不折行），而这一栏折行。
    file: PathBuf,
    /// 盘上那份文件里有的那几个名字，按字典序（`preset::names`）。
    names: Vec<String>,
    /// 光标停在第几行。`names.len()` 就是末尾那一行——「存成一份新的」。
    at: usize,
    /// 正在打的那个新名字。`None` 是在列表上走。
    naming: Option<Naming>,
}

impl Picker {
    pub(super) fn new(names: Vec<String>, file: PathBuf) -> Self {
        Self {
            file,
            names,
            at: 0,
            naming: None,
        }
    }

    /// 这一栏是哪一份文件列出来的。
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// 盘上有的那几份，按字典序。
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// 光标停在第几行。
    pub fn at(&self) -> usize {
        self.at
    }

    /// 这一栏有几行：盘上那几份，加上末尾「存成一份新的」那一行。
    pub fn rows(&self) -> usize {
        self.names.len() + 1
    }

    /// 光标停着的那一份预设。停在末尾那一行上就是 `None`——那一行不是一份预设。
    pub fn picked(&self) -> Option<&str> {
        self.names.get(self.at).map(String::as_str)
    }

    /// 正在打的那个名字。
    pub fn naming(&self) -> Option<&Naming> {
        self.naming.as_ref()
    }

    /// `name` 那一份刚存到盘上：名字进清单（还没有的话），光标停到它上面，名字不用打了。
    ///
    /// 清单按字典序（`preset::names` 给的就是这个次序），因此插在二分找到的那一格上——
    /// 重新排一遍会让光标那个下标失效，而下一屏正要按它反白。
    fn stored(&mut self, name: &str) {
        self.naming = None;
        self.at = match self
            .names
            .binary_search_by(|listed| listed.as_str().cmp(name))
        {
            Ok(at) => at,
            Err(at) => {
                self.names.insert(at, name.to_owned());
                at
            }
        };
    }
}

/// 正在打的新预设名，以及**撞名之后问过一次了没有**。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Naming {
    /// 打到哪儿了。
    pub buffer: String,
    /// 撞上同名的那一份、屏上已经说过一句了：**再按一次 `⏎` 才真覆盖**
    /// （见 [`Session::name_is_taken`]）。
    ///
    /// 与两级停同一个形状（同一个键按两次，ADR 0013），但多一条：
    /// **缓冲一改它就作废**（见 [`Session::edit_mut`]）——问的是「盖掉这一个名字吗」，
    /// 名字改了，那一问就不再作数。
    asked: bool,
}

impl Naming {
    /// 打出来的这个名字。**两头的空白不算数**，而这条规矩只有这一处：
    /// 「一个字都没打就按回车是算了」（见 [`naming_action`]）与「存成这个名字」
    /// （见 `super::store_preset`）问的是同一件事，各写一遍就会有一处忘了 `trim`。
    pub(super) fn name(&self) -> &str {
        self.buffer.trim()
    }

    /// 撞名那一句问过了没有。press 那一层照它分岔：问过了才走覆盖那一条
    /// （`preset::Presets::replace`），没问过走盖不掉同名的那一条（`save`）。
    pub(super) fn asked(&self) -> bool {
        self.asked
    }
}

/// 展开着的那一卷，以及报告区滚到哪儿了。
///
/// **不叫 `Reading`。** 展开是屏上那件事本身（报告区摊开了一卷的逐页、左栏收着），
/// 「在读」是用户的意图——后者会让人以为还有一个「不展开地读」的状态，而那个状态
/// 不存在（见 [`Mode::Expanded`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// 展开的是报告上的第几卷。
    pub volume: usize,
    /// 这一趟的报告里有几卷。**进来那一刻记下的**：`⇥` 转一圈靠它，而本模块读不到
    /// 那一趟攒的报告。展开着的时候报告不会再长——展开只从浏览进得去，
    /// 而浏览意味着没有一趟正跑着。
    pub volumes: usize,
    /// 报告从第几行开始画。**从顶数**，不是从底数：往回翻到零就是抬头那几行
    /// （profile、适配方式、裁边、拆分），而那正是跟着跑的时候滚出格子的那一截
    /// （停车场 Q64）。
    pub from: u16,
    /// 往右滚了几列。逐页那两行**不折行**（票面：逐页行不被折断），
    /// 窄终端上要看到行尾只能横着滚。
    pub right: u16,
}

impl Expansion {
    /// 摊开第 `volume` 卷，报告从第 `from` 行画起。
    ///
    /// 三个数由 [`super::press`] 一起算好——**它们是一伙的**：换一卷就要跟着换落位，
    /// 而「有几卷」是这两个数的定义域。横向那一格不在参数里：
    /// 它恒从零起，换一卷之后停在上一卷滚到的那几十列上，读的人会以为行是空的。
    pub(super) fn new(volume: usize, volumes: usize, from: u16) -> Self {
        Self {
            volume,
            volumes,
            from,
            right: 0,
        }
    }

    /// 往一边挪一卷，两头都转一圈——与三层那几个取值环同一条（见 [`ring`]）。
    pub(super) fn next(&self, step: Step) -> usize {
        match step {
            Step::Next => (self.volume + 1) % self.volumes,
            Step::Back => (self.volume + self.volumes - 1) % self.volumes,
        }
    }
}

/// 正在编辑的那一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// 改的是哪一行。
    pub field: Field,
    /// 打到哪儿了。
    pub buffer: String,
    /// 上一次按 `Tab` 列出来的那一层。**只有列出来的这一份，不留索引、不留缓存**
    /// （ADR 0009）——下一次按 `Tab` 重新列一遍。
    pub candidates: Vec<String>,
}

/// 一个会话。
///
/// 三层可改、按 `t` 试算、按 `x` 执行、跑起来之后按 `s` 停（一次收尾、再一次中止）、
/// 按 `e` 展开逐页（左栏跟着收起）、按 `p` 开预设那一栏（存下当前两层，或套用一份）、
/// 停在设备层上按 `c` 出标定图、按键退出。
/// **单卷试算跑到决策点会停下来等人**（[`Mode::Deciding`]），那时按 `x` 接着做第二遍、
/// 按 `s` 收尾。
///
/// **出标定图不往这里加状态**：它一按就完，屏底说一句就是全部结果——
/// 会话此刻在做什么一格没变（见 [`Action::Chart`] 与 [`Self::charted`]）。
///
/// **跑起来的那一趟不在这里**：这个结构只记得「此刻在做什么」（[`Mode::Running`]），
/// 攒着的那份报告与两条进度在 [`super::live::Live`] 上。分开是因为它们的寿命不同——
/// 会话一个，跑过的趟一趟一份。
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// 设备层：预设装的那一层，一格不多一格不少。
    pub device: DeviceLayer,
    /// 口味层：预设装的那一层，一格不多一格不少。
    pub taste: TasteLayer,
    /// 范围层：只在会话里有。
    pub scope: ScopeLayer,
    /// 光标停在 [`Session::rows`] 的第几行。
    cursor: usize,
    mode: Mode,
    /// 上一个动作要说的那句话（多半是「这个值不对」）。下一次按键就抹掉。
    notice: Option<String>,
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
            cursor: 0,
            mode: Mode::Browsing,
            notice: None,
        }
    }

    /// 左栏自上而下的那些行。**卷有几个就有几行**，因此每次现算。
    pub fn rows(&self) -> Vec<Field> {
        let mut rows: Vec<Field> = DEVICE_FIELDS.into_iter().chain(TASTE_FIELDS).collect();
        rows.push(Field::Out);
        rows.extend((0..self.scope.volumes.len()).map(Field::Volume));
        rows.push(Field::AddVolume);
        rows
    }

    /// 光标停在哪一行。
    pub fn focus(&self) -> Field {
        let rows = self.rows();
        rows[self.cursor.min(rows.len() - 1)]
    }

    /// 会话此刻在做什么。
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// 把光标挪到某一行上。**只给用例用**——真会话里光标是一步步走过去的，
    /// 而用例问的是「停在这种行上时按键做什么」，走过去那几步不是它要说的事。
    #[cfg(test)]
    pub fn focus_on(&mut self, field: Field) {
        self.cursor = self
            .rows()
            .iter()
            .position(|listed| *listed == field)
            .unwrap_or_else(|| panic!("{field:?} 不在左栏上"));
    }

    /// 上一个动作要说的那句话。
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// 说一句。跑不起来的那几种（型号没挑、输出根没填）就是靠它说出口的。
    pub fn complain(&mut self, said: String) {
        self.notice = Some(said);
    }

    /// 一趟跑起来了：进 [`Mode::Running`]，配置从这一刻起只读。
    ///
    /// 闩从[继续](Instruction::Continue)起——**一趟一份**。上一趟按下的停不该跟着漏到
    /// 下一趟去，理由与库那一侧把闩放在 `run` 的栈上是同一条（见 `tonefit` 的
    /// `progress::Events`）。跑着的那一趟那一份见 [`super::run::Running::start`]。
    pub fn run_started(&mut self) {
        self.notice = None;
        self.mode = Mode::Running(Instruction::Continue);
    }

    /// 那一趟收场了：回到浏览，配置又改得动。
    ///
    /// **不叫 `Live::run_finished`。** 那一个折的是 `RunFinished` 那条**事件**
    /// （库说「这一趟完了」），这一个改的是**会话**此刻在做什么——两件事，两个接收者。
    pub fn run_finished(&mut self) {
        if matches!(self.mode, Mode::Running(_) | Mode::Deciding(_)) {
            self.mode = Mode::Browsing;
        }
    }

    /// **那一趟到决策点了没有**：在[跑着](Mode::Running)与[等答话](Mode::Deciding)
    /// 之间转（`p1-session/14`）。
    ///
    /// 会话每帧问一次，与 `reap` 同一条（见 `super::drive`）：停在决策点上的是
    /// **计算线程**，而本模块碰不到线程——那一层问得到（`super::run::Running::deciding`），
    /// 把答案交进来。
    ///
    /// 别的状态一格不动：这一问只在这两者之间转场。答完话那一下不必等下一帧
    /// ——[`Action::Answer`] 当场就把状态放回去（见 [`Self::answered`]）。
    pub fn at_the_decision_point(&mut self, waiting: bool) {
        self.mode = match (&self.mode, waiting) {
            (Mode::Running(pressed), true) => Mode::Deciding(*pressed),
            (Mode::Deciding(pressed), false) => Mode::Running(*pressed),
            _ => return,
        };
    }

    /// 决策点上答完话了：回[跑着](Mode::Running)那一副，闩原样带回去。
    ///
    /// **当场就转，不等下一帧**：那条线程收到那个字就接着跑，而屏底那两行要跟着换——
    /// 慢一帧的话，答完之后那两个答话键还在屏上摆着，按下去却已经没有人收了。
    fn answered(&mut self) {
        if let Mode::Deciding(pressed) = self.mode {
            self.mode = Mode::Running(pressed);
        }
    }

    /// **按停按到哪一级了**：没按过是[继续](Instruction::Continue)，按过一次是
    /// [收尾](Instruction::Finish)，再按一次是[中止](Instruction::Abort)（ADR 0013）。
    ///
    /// 没跑着的时候恒是继续：按停是跑起来之后才有的事，浏览时那个键根本不派动作。
    ///
    /// 屏底那两行照它写（[`super::draw`]），而跑着的那一趟收到的是同一个字——
    /// [`super::press`] 按下之后把它交给 [`super::run::Running::stop`]。
    pub fn stopping(&self) -> Instruction {
        match self.mode {
            Mode::Running(pressed) | Mode::Deciding(pressed) => pressed,
            Mode::Browsing | Mode::Editing(_) | Mode::Expanded(_) | Mode::Picking(_) => {
                Instruction::Continue
            }
        }
    }

    /// **此刻停在决策点上等人拿主意吗**（`CONTEXT.md` 的《会话》：决策点）。
    ///
    /// 屏上那几处照它写：全局条那一格的抬头、屏底那两行（见 `super::draw`）。
    pub fn deciding(&self) -> bool {
        matches!(self.mode, Mode::Deciding(_))
    }

    /// 报告区此刻展开着哪一卷、滚到哪儿了。没展开就是 `None`——那是默认的那一档：
    /// **报告区只给卷级**，左栏在场（票面第一条）。
    pub fn expansion(&self) -> Option<&Expansion> {
        match &self.mode {
            Mode::Expanded(expansion) => Some(expansion),
            Mode::Browsing
            | Mode::Editing(_)
            | Mode::Running(_)
            | Mode::Deciding(_)
            | Mode::Picking(_) => None,
        }
    }

    /// 预设那一栏此刻的样子。没开着就是 `None`。
    pub fn picking(&self) -> Option<&Picker> {
        match &self.mode {
            Mode::Picking(picker) => Some(picker),
            Mode::Browsing
            | Mode::Editing(_)
            | Mode::Running(_)
            | Mode::Deciding(_)
            | Mode::Expanded(_) => None,
        }
    }

    /// 当前的设备层与口味层，装成一份**预设**。**范围层不进去**——
    /// 它每趟都不同，混进预设会让人套用时误写到上一次的输出目录（`crate::preset` 的抬头）。
    ///
    /// 「不含范围层」在这里不必靠自觉：这一层拼得出来的只有那两个字段，
    /// 而盘上那两节各自写着 `deny_unknown_fields`。
    ///
    /// **「没说」原样带过去。** 屏上那一格「默认（lanczos3）」与「说了 lanczos3」
    /// 在这里是 `None` 与 `Some(Lanczos3)` 两个值，写进 TOML 就是「这一项不写」与
    /// 「这一项写着 `filter = "lanczos3"`」——两者的差别到这一步才落到盘上（停车场 Q58）。
    pub fn preset(&self) -> Preset {
        Preset {
            device: self.device.clone(),
            taste: self.taste.clone(),
        }
    }

    /// 把三层拼成这一趟的 [`Request`]。**试算与执行只差 `mode` 一格**
    /// （`CONTEXT.md` 的《会话》：两者是同一条回路的两半）。
    ///
    /// 口味层每一项落到默认值那一步走 [`TasteLayer`] 自己那几个方法——命令行
    /// 「命令行没点、预设也没说」那一档读的是同一个（见 `crate::Cli`），
    /// 默认值因此没有第二个出处。型号那一步走 [`crate::target_profile`]，
    /// 命令行与 `calibrate` 用的也是它：三处解析出来的必须是同一块面板。
    ///
    /// **只有两项在这里挡**：型号与输出根。它们在 `Request` 上不是 `Option`，
    /// 拼不出来就无从下手。范围为空不在这里挡——那句话库那一侧已经有了
    /// （`run` 的「处理范围为空」），会话再写一句就是第二份措辞。
    pub fn request(&self, mode: RunMode) -> anyhow::Result<Request> {
        let Some(device) = self.device.profile.as_deref() else {
            anyhow::bail!("先挑型号：目标尺寸与判据都从那块面板上来，没有它跑不起来");
        };
        let Some(output_root) = self.scope.out.clone() else {
            anyhow::bail!("先填输出根：每个卷在它下面得到一份同名副本");
        };
        let taste = &self.taste;
        Ok(Request {
            inputs: self
                .scope
                .volumes
                .iter()
                .filter(|picked| picked.on)
                .map(|picked| picked.path.clone())
                .collect(),
            output_root,
            profile: crate::target_profile(device, self.device.gray_levels, self.device.threshold)?,
            fit: taste.fit(),
            crop: taste.crop(),
            split: taste.split_rule(),
            filter: taste.filter(),
            bit_depth: taste.bit_depth,
            dither: taste.dither,
            per_page: taste.per_page(),
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

    /// 标定图要按哪块面板画：把设备层拼成一个 [`Profile`]。
    ///
    /// **阈值恒不带**（传 `None`），与命令行那一路逐字同一条：标定图是量具，不经判定
    /// （见 `crate::target_profile` 与 `crate::calibrate`）。合 profile 走的也是那一个函数——
    /// 处理卷、`calibrate` 子命令、会话，三处解析出来的必须是同一块面板。
    ///
    /// **型号没挑就说一句**，与拼不出 [`Request`](Self::request) 时同一个待遇：
    /// 那一格没有默认值可退，图按面板分辨率排布，没有面板就无从画起。
    ///
    /// **不是 [`calibrated_profile`](Self::calibrated_profile)。** 那一个是给左栏
    /// 阈值那一行印数用的：它**要**阈值（印的就是它），而且拼不出来只意味着那一行印不出来，
    /// 因此吞成 `None`。这一个反过来：阈值一格不带，而拼不出来要说得出口。
    pub(super) fn chart_profile(&self) -> anyhow::Result<Profile> {
        let Some(device) = self.device.profile.as_deref() else {
            anyhow::bail!("先挑型号：标定图按那块面板的分辨率排布，没有它画不出来");
        };
        crate::target_profile(device, self.device.gray_levels, None)
    }

    /// 标定图写出去了：屏底说清**图在哪儿**，以及**此刻**要做对的那一件事。
    ///
    /// 措辞取自界面文案那一份（[`crate::render::calibration_notice`]），本模块不另编一套：
    /// 同一件事命令行那一路也要说，而措辞只有一处出处。
    ///
    /// **怎么数不在这里重抄**：图内中英两份都印着，`calibrate --help` 里也写着。
    /// 屏上只说在别处来不及的那一条——图一旦被缩着显示过，它答的两件事一件都不作数了。
    pub(super) fn charted(&mut self, out: &Path) {
        self.notice = Some(crate::render::calibration_notice(out));
    }

    /// 这个键在当前状态下做什么。**「哪些键在哪个状态下有效」这张表就是它。**
    ///
    /// 纯函数：不改任何东西，用例问得动它，也因此不必去数「按下去之后屏幕变成什么样」。
    pub fn action(&self, key: Key) -> Action {
        match &self.mode {
            Mode::Browsing => self.browsing_action(key),
            Mode::Editing(edit) => editing_action(edit, key),
            Mode::Running(pressed) => running_action(key, *pressed),
            Mode::Deciding(_) => deciding_action(key),
            Mode::Expanded(_) => expanded_action(key),
            Mode::Picking(picker) => picking_action(picker, key),
        }
    }

    /// 浏览时的按键表。左右键与回车做什么，随光标停的那一行的[形状](Shape)而变。
    fn browsing_action(&self, key: Key) -> Action {
        let shape = self.focus().shape();
        match key {
            Key::Up | Key::Char('k') => Action::Move(Step::Back),
            Key::Down | Key::Char('j') => Action::Move(Step::Next),
            Key::Left => cycle_or(shape, Step::Back, Action::Ignored),
            Key::Right => cycle_or(shape, Step::Next, Action::Ignored),
            // 浏览时空格与回车**同义**：两个都是「就在这一行上动手」，做什么随行状分派。
            Key::Space | Key::Enter => match shape {
                Shape::Cycle => Action::Cycle(Step::Next),
                Shape::Text | Shape::Path => Action::Edit,
                Shape::Volume => Action::Toggle,
            },
            Key::Char('d') => match shape {
                Shape::Volume => Action::Remove,
                _ => Action::Ignored,
            },
            // 试算与执行。两个键**在键盘上离得远**：按错一个会往盘上写东西，
            // 而这一路上没有第二道确认（`t` 只算不写，`x` 真写）。
            Key::Char('t') => Action::Start(RunMode::DryRun),
            Key::Char('x') => Action::Start(RunMode::Process),
            // 展开逐页。它与光标停在哪一行无关：报告区是右边那一大格的事，
            // 而左栏此刻只是让位的那一方。
            Key::Char('e') => Action::Expand,
            // 预设那一栏。同样与光标停在哪一行无关：存的是**整两层**，不是这一行。
            Key::Char('p') => Action::Pick,
            // 出标定图。**这是唯一一个认层的键**：它出的那个数是设备层唯一填不出来的
            // 一格，停在口味层或范围层上按它没有意义（见 [`Action::Chart`]）。
            // 「按当前 profile 出图」在那两层上也说得通，但那时屏上摆的是别的事——
            // 一个键在它够不着的地方仍旧有效，等于把这三行与那张图的关系抹掉了。
            Key::Char('c') if self.focus().layer() == Layer::Device => Action::Chart,
            Key::Char('q') | Key::Esc | Key::Interrupt => Action::Quit,
            Key::Char(_) | Key::Tab | Key::BackTab | Key::Backspace => Action::Ignored,
        }
    }

    /// 按一个键，把它对应的那件事做掉。
    ///
    /// **只给用例用。** 真会话里那一层先把[起一趟](Action::Start)接走
    /// （见 [`super::press`]），剩下的原样交给 [`act`](Self::act)，不必再问一遍
    /// [`action`](Self::action)；而用例问的是「按下这个键之后会话变成什么样」，
    /// 那两步在它眼里本来就是一步。
    #[cfg(test)]
    pub fn press(&mut self, key: Key) -> Exit {
        self.act(self.action(key))
    }

    /// 把一个动作做掉。**问过 [`action`](Self::action) 的调用方走这一条**——
    /// [`super::press`] 先把[起一趟](Action::Start)那一支接走，剩下的原样交回来，
    /// 不必再问一次。
    pub(super) fn act(&mut self, action: Action) -> Exit {
        // 上一个动作说的那句话到这里就作废了：下一次按键就抹掉。
        self.notice = None;
        self.apply(action)
    }

    fn apply(&mut self, action: Action) -> Exit {
        match action {
            Action::Move(step) => self.move_cursor(step),
            Action::Cycle(step) => self.cycle(step),
            Action::Toggle => self.toggle_volume(),
            Action::Edit => self.begin_edit(),
            Action::Remove => self.remove_volume(),
            Action::Insert(character) => self.edit_mut(|buffer| buffer.push(character)),
            Action::Backspace => self.edit_mut(|buffer| {
                buffer.pop();
            }),
            Action::Complete => self.complete(),
            Action::Commit => self.commit(),
            Action::Cancel => self.cancel(),
            // 起线程、拼 `Request`、把观察者接上去，都在 [`super::press`] 那一层：
            // 本模块一个终端都不碰，也不该起线程。那一层把 `Start` 接走之后才调
            // [`act`](Self::act)，因此这里到不了——真到了也只是这一下没起来，不是错。
            Action::Start(_) => {}
            // 升一级就在这里；把升到的那一级交给跑着的那一趟在 [`super::press`]。
            Action::Stop => self.raise_stop(),
            // 状态转回「跑着」就在这里；把那个字交给停在决策点上的那条线程
            // 在 [`super::press`]（`Running::decide`）——与按停同一条分工。
            Action::Answer(_) => self.answered(),
            // 展开与换卷要读那一趟攒下来的报告（有几卷、那一卷从第几行起），
            // 而本模块读不到它——真做这两件事的是 [`super::press`]，它随后调
            // [`expand`](Self::expand)。与[起一趟](Action::Start)同一条分法，
            // 因此这里到不了；真到了也只是这一下没展开，不是错。
            Action::Expand | Action::Turn(_) => {}
            // 预设那三支都要碰盘（列出来、读一份、写一份），而本模块碰不到盘：
            // 真做这三件事的是 [`super::press`]，它随后调 [`pick`](Self::pick)、
            // [`took`](Self::took)、[`saved`](Self::saved) 那几个。与上面两支同一条分法，
            // 因此这里到不了——真到了也只是这一下没动，不是错。
            Action::Pick | Action::Take | Action::Store => {}
            // 出标定图同样要碰盘（真画图与落盘的是 `tonefit::write_calibration_chart`），
            // 走的是 [`super::press`]，它随后调 [`charted`](Self::charted)。
            // 与上面那三支同一条分法，因此这里到不了——真到了也只是这一下没出图，不是错。
            Action::Chart => {}
            Action::Collapse => self.mode = Mode::Browsing,
            Action::Scroll(toward) => self.scroll(toward),
            Action::Quit => return Exit::Leave,
            Action::Ignored => {}
        }
        Exit::Stay
    }

    /// 展开一卷的逐页，左栏跟着收起。
    ///
    /// 那一份 [`Expansion`] 由 [`super::press`] 拼好送进来：它读得到那一趟攒的报告，
    /// 本模块读不到。`from` 是报告从第几行画起——展开那一下是零（抬头那几行跟着回来，
    /// 见 [`Expansion::from`]），换卷那一下是那一卷的抬头在第几行。
    pub(super) fn expand(&mut self, expansion: Expansion) {
        // 上一个动作说的那句话到这里就作废了，与 [`act`](Self::act) 同一条。
        self.notice = None;
        self.mode = Mode::Expanded(expansion);
    }

    /// 进预设那一栏，列的是 `names`。
    ///
    /// 列什么、从哪一份文件列的，都由 [`super::press`] 从盘上读来（[`Action::Pick`]），
    /// 与[展开](Self::expand)收下一份 [`Expansion`] 是同一条：那一层读得到，本模块读不到。
    pub(super) fn pick(&mut self, names: Vec<String>, file: PathBuf) {
        self.notice = None;
        self.mode = Mode::Picking(Picker::new(names, file));
    }

    /// 套用一份预设：**两层整个换成它，范围层一格不动**（票面第三条）。
    ///
    /// 整个换而不是「只盖它说到的那几项」：一份预设就是一整份两层的立场，
    /// 套上它之后屏上看到的必须就是那一份——否则「存出去再套回来」不是一次往返，
    /// 而是与上一次配置的一次合并，而合出来的东西没人说得清是什么。
    /// 它说「没说」的那几项因此也换成「没说」（屏上落回`默认（…）`那一格）。
    ///
    /// 套完回浏览：这一栏的事做完了，而改完接着要看的是左栏。
    pub(super) fn took(&mut self, name: &str, preset: Preset) {
        self.device = preset.device;
        self.taste = preset.taste;
        self.mode = Mode::Browsing;
        self.notice = Some(format!(
            "套上了「{name}」：设备层与口味层换成了它，范围层一格没动"
        ));
    }

    /// 存好了：那个名字进这一栏的列表，光标停到它上面。
    ///
    /// **不退出这一栏**：刚存出去的那一份就摆在眼前的列表上，「存成了什么」因此看得见。
    /// 说的那句话里带着**命令行上怎么用它**——存预设是为了下一次不必重配，
    /// 而下一次多半在命令行上（spec 的 story 12）。**写到哪儿了不在这句话里**：
    /// 屏底那一格不折行，一条长路径会被切掉；那份文件的位置摆在这一栏自己身上
    /// （见 [`Picker::file`]），它折得下来。
    pub(super) fn saved(&mut self, name: &str) {
        if let Mode::Picking(picker) = &mut self.mode {
            picker.stored(name);
        }
        self.notice = Some(format!(
            "存好了：「{name}」——命令行上 --preset {name} 就是它"
        ));
    }

    /// 那个名字已经有人占着：**说一句，闩上「再按一次就覆盖」**。
    ///
    /// 两下而不是一下，理由与两级停同一条（ADR 0013：中止是「再按一次」）：
    /// 盖掉的可能是别人手写的一份预设，而那一步不可逆。
    /// 这一句非说不可的还有第二半——**覆盖会把那份文件整个重排**，
    /// 手写的注释与排版留不下来（见 `preset::insert`）。
    pub(super) fn name_is_taken(&mut self, name: &str) {
        if let Mode::Picking(Picker {
            naming: Some(naming),
            ..
        }) = &mut self.mode
        {
            naming.asked = true;
        }
        self.notice = Some(format!(
            "已经有一份「{name}」了：再按一次 ⏎ 覆盖它。\
             覆盖会把那份预设文件整个照标准格式重排，手写的注释与排版留不下来"
        ));
    }

    /// 报告区往一个方向挪一下。上下一行、左右 [`SIDEWAYS`] 列。
    ///
    /// **往上、往左都收在零上**（`saturating_sub`）：零就是报告的头一行、行首那一列。
    /// 另外两头收在哪儿这里答不出——那要知道这一格装得下几行几列，
    /// 而本模块一个终端都不碰。画法那一层每帧收一次（[`Self::clamp_report`]）。
    fn scroll(&mut self, toward: Toward) {
        let Mode::Expanded(expansion) = &mut self.mode else {
            return;
        };
        match toward {
            Toward::Up => expansion.from = expansion.from.saturating_sub(1),
            Toward::Down => expansion.from = expansion.from.saturating_add(1),
            Toward::Left => expansion.right = expansion.right.saturating_sub(SIDEWAYS),
            Toward::Right => expansion.right = expansion.right.saturating_add(SIDEWAYS),
        }
    }

    /// 把滚动量收进这一格真滚得动的范围：最多滚到 `down` 行、`right` 列。
    ///
    /// **画法那一层每帧调一次**（见 [`super::draw::report_pane`]），因为只有它知道
    /// 这一格装得下几行几列。不收的话，往下翻过了头之后再往回翻，头几下会**按了没反应**
    /// ——而那正是本仓库反复要躲的那件事（`p1-session/10` 的「屏上不摆按不动的键」）。
    ///
    /// 只往下收、不往上抬：`0` 恒是合法的落点。
    pub(super) fn clamp_report(&mut self, down: u16, right: u16) {
        if let Mode::Expanded(expansion) = &mut self.mode {
            expansion.from = expansion.from.min(down);
            expansion.right = expansion.right.min(right);
        }
    }

    /// 把闩往上升一级：继续 → 收尾 → 中止 → 中止（ADR 0013）。
    ///
    /// **只升不降**是这个函数的形状本身：升到中止之后它就是个不动点，
    /// 而键盘上没有第二个键能往回按——两级停是同一个键按两次（见 [`Action::Stop`]）。
    /// 库那一侧的闩用 `fetch_max` 说同一件事（`tonefit::Instruction` 的序即力度）。
    fn raise_stop(&mut self) {
        if let Mode::Running(pressed) = &mut self.mode {
            *pressed = match *pressed {
                Instruction::Continue => Instruction::Finish,
                Instruction::Finish | Instruction::Abort => Instruction::Abort,
            };
        }
    }

    /// 光标挪一行。**预设那一栏开着时挪的是那一栏**——左栏此刻不在屏上
    /// （与展开那一副同一条：`↑↓` 改的恒是眼前这一列，见 [`expanded_action`]）。
    fn move_cursor(&mut self, step: Step) {
        if let Mode::Picking(picker) = &mut self.mode {
            picker.at = around(picker.at, picker.rows(), step);
            return;
        }
        self.cursor = around(self.cursor, self.rows().len(), step);
    }

    /// 缓冲改一个字。**两处缓冲同一条规矩**：改完把上一次问出去的那件事作废——
    /// 编辑一行时那是列出来的候选，打预设名时那是「盖掉同名的那一份吗」这一问
    /// （见 [`Naming::asked`]）。
    fn edit_mut(&mut self, change: impl FnOnce(&mut String)) {
        match &mut self.mode {
            Mode::Editing(edit) => {
                change(&mut edit.buffer);
                edit.candidates.clear();
            }
            Mode::Picking(Picker {
                naming: Some(naming),
                ..
            }) => {
                change(&mut naming.buffer);
                naming.asked = false;
            }
            Mode::Browsing
            | Mode::Running(_)
            | Mode::Deciding(_)
            | Mode::Expanded(_)
            | Mode::Picking(_) => {}
        }
    }

    /// 进入编辑，缓冲里先摆着当前的取值——改一个字比重打一遍便宜。
    ///
    /// **预设那一栏上是打一个新名字**：那一行本来就是「存成一份新的」，缓冲从空的起
    /// （与「＋ 再打一个卷进来」同一条——那一行也没有「当前取值」可摆）。
    fn begin_edit(&mut self) {
        if let Mode::Picking(picker) = &mut self.mode {
            picker.naming = Some(Naming::default());
            return;
        }
        let field = self.focus();
        let buffer = match field {
            Field::AddVolume => String::new(),
            other => self.typed(other),
        };
        self.mode = Mode::Editing(Edit {
            field,
            buffer,
            candidates: Vec::new(),
        });
    }

    /// 丢掉眼下这一步。**退一步，不是退到底**：打预设名打到一半退回那一栏的列表上，
    /// 再按一次才出这一栏（见 [`naming_action`]）。
    fn cancel(&mut self) {
        if let Mode::Picking(picker) = &mut self.mode
            && picker.naming.take().is_some()
        {
            return;
        }
        self.mode = Mode::Browsing;
    }

    /// **逐层补全**：只列打到的那一层，不递归、不建索引、不缓存（ADR 0009）。
    ///
    /// 列出来的若干项有共同的前缀就先补到那儿——补到分岔口为止是补全该做的事，
    /// 替用户从几项里挑一项不是。
    fn complete(&mut self) {
        let Mode::Editing(edit) = &mut self.mode else {
            return;
        };
        let listed = complete::level(&edit.buffer);
        if let Some(common) = complete::common_prefix(&listed)
            && common.len() > edit.buffer.len()
        {
            edit.buffer = common;
        }
        let nothing_there = listed.is_empty();
        edit.candidates = listed;
        if nothing_there {
            self.notice = Some("这一层下面没有对得上的东西".to_owned());
        }
    }
}

/// 编辑时的按键表。
///
/// 上下左右**一个都不接**：编辑到一半时光标不该从这一行跑掉，
/// 而缓冲里没有行内光标这个概念（要改中间的字就退到那儿）。
fn editing_action(edit: &Edit, key: Key) -> Action {
    match key {
        Key::Char(character) => Action::Insert(character),
        Key::Space => Action::Insert(' '),
        Key::Backspace => Action::Backspace,
        Key::Tab => match edit.field.shape() {
            Shape::Path => Action::Complete,
            _ => Action::Ignored,
        },
        Key::Enter => Action::Commit,
        Key::Esc => Action::Cancel,
        Key::Interrupt => Action::Quit,
        Key::BackTab | Key::Up | Key::Down | Key::Left | Key::Right => Action::Ignored,
    }
}

/// 跑起来之后的按键表：**一个改动键都不派，只留按停与退出**。
///
/// 「跑起来之后三层只读」（`CONTEXT.md` 的《会话》）因此是结构上成立的，
/// 不是画法上把它们涂灰：改一行的那几个动作在这个状态下根本不存在。
///
/// **按停只有 `s` 一个键，按两次**（ADR 0013：中止是「再按一次」）：
/// 第一次升到收尾，第二次升到中止。升到中止之后它**不再有意义**——闩到了顶，
/// 再按一次没有更强的一级可去，因此派 [`Action::Ignored`] 而不是一个什么都不改的动作。
/// 「按了中止之后退不回收尾」于是不必靠任何一处代码守着：键盘上没有那个键。
///
/// 退出这一路照旧只有 [`Key::Interrupt`]：它在**每一个**状态下都是退出，跑到一半也是
/// （见 `Key::Interrupt` 自己的文档）。**`q` 与 `Esc` 跑着时按不动**，
/// 而这是本票拿的一个主意（停车场 Q63）：退出会话要连着把当前卷丢掉，
/// 那是中止那一级的事——而中止现在有专门的键，按两次 `s` 就到。
/// 让 `q`／`Esc` 也能一下子把当前卷丢掉，等于给最容易手滑的两个键挂上最重的后果。
/// 退出时那条还在写盘的线程怎么收，见 [`super::run::Running::leave`]。
///
/// **展开那个键（`e`）跑着时同样按不动**（停车场 Q72）：报告那时还在长，
/// 而这一刻按得动的只该有停。要展开就等这一趟收场——它跑完之后报告一行不少。
fn running_action(key: Key, pressed: Instruction) -> Action {
    match key {
        Key::Interrupt => Action::Quit,
        Key::Char('s') if pressed < Instruction::Abort => Action::Stop,
        Key::Up
        | Key::Down
        | Key::Left
        | Key::Right
        | Key::Enter
        | Key::Space
        | Key::Tab
        | Key::BackTab
        | Key::Backspace
        | Key::Esc
        | Key::Char(_) => Action::Ignored,
    }
}

/// **停在决策点上等人拿主意时的按键表**：`x` 接着做第二遍，`s` 收尾，`Ctrl-C` 退出会话
/// （`p1-session/14`，ADR 0012）。
///
/// 两个方向各拿一个**已经有主的键**，因为它们在这里做的正是那个键一直在做的事：
/// `x` 是执行——「接着做第二遍」就是把这一趟做完；`s` 是停——「收尾」是它的第一级，
/// 而决策点上答收尾停出来的现场恰好也是「盘上不留半卷」（这一卷一个字节都不写）。
/// 另取两个新键的话，屏上就要多记两个只在这一刻有效的记号，而它们与已有的那两个
/// 说的是同一件事。
///
/// **`s` 在这里不是两级停。**跑着时 `s` 升的是[闩](Session::stopping)，一次收尾、
/// 再一次中止；这里 `s` 答的是**当场那个字**，答完那条线程就接着走，没有第二次可按
/// （`CONTEXT.md` 的《会话》：决策点不是第三个检查点）。
///
/// **`x` 在这里不是「起一趟」。**浏览时 `x` 起的是新的一趟，这里它答的是眼前这一趟的
/// 那一问——两者都是「把它做出来」，而在这个状态下根本没有第二趟可起：三层此刻只读。
///
/// 退出照旧只有 [`Key::Interrupt`]，与跑着时同一条（停车场 Q63）：`q`／`Esc` 按不动。
/// 退出会话走中止，那一卷等于没做、`partial` 也没留下——最容易手滑的两个键不该挂这个后果。
///
/// **三层仍旧只读**：一个改动键都不派，与 [`running_action`] 同一条。
/// 这一趟还没收场，`Request` 也早在起线程那一刻就是一份快照了。
fn deciding_action(key: Key) -> Action {
    match key {
        Key::Interrupt => Action::Quit,
        Key::Char('x') => Action::Answer(Instruction::Continue),
        Key::Char('s') => Action::Answer(Instruction::Finish),
        Key::Up
        | Key::Down
        | Key::Left
        | Key::Right
        | Key::Enter
        | Key::Space
        | Key::Tab
        | Key::BackTab
        | Key::Backspace
        | Key::Esc
        | Key::Char(_) => Action::Ignored,
    }
}

/// 展开之后的按键表：**报告区在两根轴上滚，`⇥` 换一卷，`e`／`Esc` 收起。**
///
/// 上下左右那几个键在这里改的是报告区，不是左栏——左栏此刻不在屏上
/// （见 [`Mode::Expanded`]），把它们留给一栏看不见的东西才是「按了没反应」。
/// `j`／`k` 跟着 `↑↓`，与浏览时一个待遇（见 [`Session::browsing_action`]）。
/// 两根轴上一格的大小不同：上下一行，左右 [`SIDEWAYS`] 列（逐页那两行过 100 列）。
///
/// **换卷用 `⇥` 与 `⇧⇥`**：`⇥` 在浏览时没有意义、在编辑路径时是「下一层」，
/// 两处都是「往下一个去」，这里接着用同一个意思；`⇧⇥` 是它的另一头。
/// 两头都有而不是只往后转一圈，是因为票面要的是**选中一卷**——
/// 几十卷的一趟里往回看一卷不该按二十九下。
///
/// **收起有两个键，而这不是重复**：`e` 是展开那个键的另一半（同一个键按回去），
/// `Esc` 是「退一步」——编辑到一半按它是丢掉缓冲回浏览，这里按它是收起回配置，
/// 同一个意思。两级停那个 `s` 不给第二个键，因为**中止退不回收尾**；
/// 收起退得回去，因此不必守着只有一个入口。
///
/// **`q` 仍是退出会话**（与浏览时同一件事）：展开只是在读报告，
/// 没有「按错一下就丢掉一卷」那种后果，不必像跑着时那样把它按不动
/// （`p1-session/10` 的停车场 Q63）。
///
/// **`t` 与 `x` 在这里按不动**：起一趟要先收起——报告区正摊着上一趟的逐页，
/// 而新的一趟会当场把它换掉。收起是一个键的事。
fn expanded_action(key: Key) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Scroll(Toward::Up),
        Key::Down | Key::Char('j') => Action::Scroll(Toward::Down),
        Key::Left => Action::Scroll(Toward::Left),
        Key::Right => Action::Scroll(Toward::Right),
        Key::Tab => Action::Turn(Step::Next),
        Key::BackTab => Action::Turn(Step::Back),
        Key::Char('e') | Key::Esc => Action::Collapse,
        Key::Char('q') | Key::Interrupt => Action::Quit,
        Key::Enter | Key::Space | Key::Backspace | Key::Char(_) => Action::Ignored,
    }
}

/// 预设那一栏的按键表。两副样子：在列表上走，或者正在打一个新名字。
fn picking_action(picker: &Picker, key: Key) -> Action {
    match &picker.naming {
        Some(naming) => naming_action(naming, key),
        None => listing_action(picker, key),
    }
}

/// 在预设那一栏的列表上走时的按键表。
///
/// **上下与回车照左栏那一副**（`↑↓`／`kj` 挪一行、`⏎`／空格「就在这一行上动手」）：
/// 这一栏与左栏是同一种东西——一列可以停上去的行，停在哪一行决定那一下做什么。
/// 停在一份预设上是**套用**，停在末尾那一行上是**打一个名字存下来**，
/// 与范围层「＋ 再打一个卷进来」逐字同一个手势。
///
/// **`p` 与 `Esc` 都退得回配置**，理由与展开那一副的 `e`／`Esc` 同一条：
/// 前者是开这一栏那个键按回去，后者是「退一步」。左右键在这里没有意义——
/// 这一栏上一行只有一个名字，没有取值环。
fn listing_action(picker: &Picker, key: Key) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Move(Step::Back),
        Key::Down | Key::Char('j') => Action::Move(Step::Next),
        Key::Enter | Key::Space => match picker.picked() {
            Some(_) => Action::Take,
            None => Action::Edit,
        },
        Key::Char('p') | Key::Esc => Action::Cancel,
        Key::Char('q') | Key::Interrupt => Action::Quit,
        Key::Left | Key::Right | Key::Tab | Key::BackTab | Key::Backspace | Key::Char(_) => {
            Action::Ignored
        }
    }
}

/// 正在打一个新预设名时的按键表。**与编辑左栏一行同一副样子**
/// （见 [`editing_action`]）：字进缓冲、`⏎` 收下、`Esc` 丢掉、上下左右一个都不接。
///
/// 三处不同，各有理由：
///
/// - **`⇥` 在这里没有意义**：预设名不是路径，没有「下一层」可补（补全那件事在 `complete`）。
/// - **一个字都没打就按回车是「算了」**，与「再打一个卷进来」那一行同一条
///   （见 [`Session::take`]）：那不是要存一份没有名字的预设。
/// - **`Esc` 退回的是这一栏的列表，不是配置**：打名字是这一栏里的一步，
///   退一步该退到上一步（再按一次才出这一栏）。
fn naming_action(naming: &Naming, key: Key) -> Action {
    match key {
        Key::Char(character) => Action::Insert(character),
        Key::Space => Action::Insert(' '),
        Key::Backspace => Action::Backspace,
        Key::Enter if naming.name().is_empty() => Action::Cancel,
        Key::Enter => Action::Store,
        Key::Esc => Action::Cancel,
        Key::Interrupt => Action::Quit,
        Key::Tab | Key::BackTab | Key::Up | Key::Down | Key::Left | Key::Right => Action::Ignored,
    }
}

/// 一列 `rows` 行里挪一格，**两头都绕回去**。左栏与预设那一栏共用它——
/// 「挪到头就绕回来」是同一条规矩，写两份就会有一处忘了绕。
fn around(at: usize, rows: usize, step: Step) -> usize {
    match step {
        Step::Next => (at + 1) % rows,
        Step::Back => (at + rows - 1) % rows,
    }
}

/// 取值环上转一格，转不动的行状就是 `otherwise`。
fn cycle_or(shape: Shape, step: Step, otherwise: Action) -> Action {
    match shape {
        Shape::Cycle => Action::Cycle(step),
        _ => otherwise,
    }
}

/// 一个取值环上的上一格：一直往前走，走到「再走一步就回到出发点」为止。
///
/// 各项的环只写一遍（往前那一份），倒着转不必再写一份反向的表。
fn back<T: Clone + PartialEq>(value: T, next: impl Fn(T) -> T) -> T {
    let mut cursor = value.clone();
    loop {
        let ahead = next(cursor.clone());
        if ahead == value {
            return cursor;
        }
        cursor = ahead;
    }
}

/// 转一格：`Next` 就是那个环本身，`Back` 由 [`back`] 反着走。
fn turn<T: Clone + PartialEq>(value: T, step: Step, next: impl Fn(T) -> T) -> T {
    match step {
        Step::Next => next(value),
        Step::Back => back(value, next),
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
    /// 把光标那一行的取值往前（或往后）转一格。
    fn cycle(&mut self, step: Step) {
        match self.focus() {
            Field::Profile => {
                let current = self.device.profile.clone();
                self.device.profile = turn(current, step, |device: Option<String>| {
                    next_device(device.as_deref())
                });
                // 型号一换，标定出来的那两个数就不再是这块面板上的（ADR 0002）。
                self.device.gray_levels = None;
                self.device.threshold = None;
            }
            Field::Fit => {
                self.taste.fit = turn(self.taste.fit, step, |fit| {
                    ring(fit, FitMode::Height, next_fit)
                });
            }
            Field::Crop => self.taste.crop = turn_flag(self.taste.crop, step),
            Field::Split => self.taste.split = turn_flag(self.taste.split, step),
            Field::ReadingOrder => {
                self.taste.reading_order = turn(self.taste.reading_order, step, |order| {
                    ring(order, ReadingOrder::RightToLeft, next_order)
                });
            }
            Field::Filter => {
                self.taste.filter = turn(self.taste.filter, step, |filter| {
                    ring(filter, Filter::Area, next_filter)
                });
            }
            Field::BitDepth => {
                self.taste.bit_depth = turn(self.taste.bit_depth, step, |depth| {
                    ring(depth, BitDepth::One, next_bit_depth)
                });
            }
            Field::Dither => {
                self.taste.dither = turn(self.taste.dither, step, |dither| {
                    ring(dither, Dither::Off, next_dither)
                });
            }
            Field::PerPage => self.taste.per_page = turn_flag(self.taste.per_page, step),
            Field::IoMode => {
                self.taste.io_mode = turn(self.taste.io_mode, step, |mode| {
                    ring(mode, IoMode::Auto, next_io_mode)
                });
            }
            // 打字改的行与卷行转不动，按键表根本不会派 `Cycle` 过来。
            // 逐个列出来而不写 `_`：新添一行转不转得动是个要当场拿的主意。
            Field::GrayLevels
            | Field::Threshold
            | Field::SplitThreshold
            | Field::CacheBudget
            | Field::Out
            | Field::Volume(_)
            | Field::AddVolume => {}
        }
    }

    fn toggle_volume(&mut self) {
        if let Field::Volume(at) = self.focus()
            && let Some(volume) = self.scope.volumes.get_mut(at)
        {
            volume.on = !volume.on;
        }
    }

    fn remove_volume(&mut self) {
        if let Field::Volume(at) = self.focus()
            && at < self.scope.volumes.len()
        {
            self.scope.volumes.remove(at);
            // 删掉最后一个卷时光标掉到「再打一个」那一行上，那正是接着要做的事。
            self.cursor = self.cursor.min(self.rows().len() - 1);
        }
    }

    /// 这一行当前取值的**可编辑写法**：进编辑时缓冲里摆的就是它，空串代表「没说」。
    fn typed(&self, field: Field) -> String {
        match field {
            Field::GrayLevels => self.device.gray_levels.map(|n| n.to_string()),
            Field::Threshold => self.device.threshold.map(|value| value.to_string()),
            Field::SplitThreshold => self
                .taste
                .split_threshold
                .map(|threshold| threshold.value().to_string()),
            Field::CacheBudget => self.taste.cache_budget.map(crate::preset::spell_budget),
            Field::Out => self.scope.out.as_ref().map(|out| out.display().to_string()),
            // 转着改的行与卷行没有可编辑的写法；「再打一个」进编辑时缓冲是空的。
            Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::PerPage
            | Field::IoMode
            | Field::Volume(_)
            | Field::AddVolume => None,
        }
        .unwrap_or_default()
    }

    /// 收下缓冲里的东西。**解析不过就留在编辑态**——把用户打的东西丢掉再让他重打一遍
    /// 是最差的那一种处置。
    fn commit(&mut self) {
        let Mode::Editing(edit) = &self.mode else {
            return;
        };
        let (field, typed) = (edit.field, edit.buffer.trim().to_owned());
        match self.take(field, &typed) {
            Ok(()) => self.mode = Mode::Browsing,
            Err(error) => self.notice = Some(format!("{error}")),
        }
    }

    /// 把一行打出来的文本验成取值收下。空串是「没说」，落回默认值。
    fn take(&mut self, field: Field, typed: &str) -> anyhow::Result<()> {
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
            Field::CacheBudget => {
                self.taste.cache_budget = match typed {
                    "" => None,
                    text => Some(CacheBudget::parse(text)?),
                };
            }
            Field::Out => {
                self.scope.out = match typed {
                    "" => None,
                    text => Some(PathBuf::from(text)),
                };
            }
            // 一个字都没打就按了回车：那是「算了」，不是打进来一个空路径。
            Field::AddVolume if !typed.is_empty() => {
                self.scope.volumes.push(Picked {
                    path: PathBuf::from(typed),
                    on: true,
                });
                // 新的一行插在「再打一个」上面，光标跟着它往下挪一行、仍停在「再打一个」上。
                self.cursor = self.rows().len() - 1;
            }
            // 转着改的行与卷行打不了字，按键表根本不会派 `Edit` 过来；
            // 「再打一个」落到这里的只有上面那个卫语句放过的空串。
            Field::AddVolume
            | Field::Profile
            | Field::Fit
            | Field::Crop
            | Field::Split
            | Field::ReadingOrder
            | Field::Filter
            | Field::BitDepth
            | Field::Dither
            | Field::PerPage
            | Field::IoMode
            | Field::Volume(_) => {}
        }
        Ok(())
    }

    /// 设备层那两个覆盖项：**要先挑型号**，界才验得动。
    ///
    /// 与预设那一侧是同一条规矩（见 `preset` 的 `no_panel_to_calibrate_against_error`）：
    /// 感知可分辨级数是在某一台真机上数出来的，阈值是在某一块面板上盲测夹出来的，
    /// 而判据跟着面板走、不可跨面板比较（ADR 0002）。界本身只有一处出处
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

    /// 阈值那一行照 `tonefit::Threshold` 自己的 `Display` 印：**数值加标定来源**，
    /// 与报告里那一行同一个出处。
    ///
    /// 不在这里另写一句（spec 的 Further Notes：「会话里显示时照报告的写法把来源原样带上来，
    /// 不自己另编一套说法」）。标定来源是阈值的一部分，不是旁注——判据跟着面板走、
    /// 不可跨面板比较（ADR 0002），读的人得能自己判断它对手上那块板成不成立。
    ///
    /// 还没挑型号时印不出数：界挂在 profile 上，那是它唯一的出处。
    fn threshold_shown(&self) -> String {
        match self.calibrated_profile() {
            Some(profile) => profile.threshold().to_string(),
            None => "跟着型号走（先挑一个）".to_owned(),
        }
    }

    /// 眼下这一趟的 profile：型号加设备层那两个覆盖项。挑了型号才有。
    ///
    /// 合出来的那一步走 [`crate::target_profile`]——命令行与 `calibrate` 用的是同一个，
    /// 三处解析出来的必须是同一块面板。
    ///
    /// 合法性在收下那一刻就验过了（见 [`Session::calibrated`]），失败只可能是
    /// 型号被换掉而覆盖项没跟着清，而那条路由 [`Session::cycle`] 堵着；
    /// 真落到 `ok()` 上等于「这一行印不出来」，不是错误。
    fn calibrated_profile(&self) -> Option<Profile> {
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
            Field::GrayLevels => spell(self.device.gray_levels, "跟随面板"),
            Field::Threshold => self.threshold_shown(),
            Field::Fit => spell_name(self.taste.fit, FitMode::name),
            Field::Crop => spell_flag(self.taste.crop, true, "裁", "不裁"),
            Field::Split => spell_flag(self.taste.split, true, "拆", "不拆"),
            Field::SplitThreshold => spell(
                self.taste.split_threshold.map(SplitThreshold::value),
                &SplitThreshold::default().value().to_string(),
            ),
            Field::ReadingOrder => spell_name(self.taste.reading_order, ReadingOrder::name),
            Field::Filter => spell_name(self.taste.filter, Filter::name),
            Field::BitDepth => match self.taste.bit_depth {
                Some(depth) => format!("{}bit", depth.bits()),
                None => "自动（判据说了算）".to_owned(),
            },
            Field::Dither => match self.taste.dither {
                Some(dither) => dither.name().to_owned(),
                None => "自动（判据说了算）".to_owned(),
            },
            Field::PerPage => spell_flag(self.taste.per_page, false, "开", "关"),
            Field::CacheBudget => match self.taste.cache_budget {
                Some(budget) => budget.to_string(),
                None => format!("默认（{}）", CacheBudget::default()),
            },
            Field::IoMode => spell_name(self.taste.io_mode, IoMode::name),
            Field::Out => match &self.scope.out {
                Some(out) => out.display().to_string(),
                None => "未填（跑起来之前必填）".to_owned(),
            },
            Field::Volume(at) => match self.scope.volumes.get(at) {
                Some(volume) => format!(
                    "{} {}",
                    if volume.on { "[x]" } else { "[ ]" },
                    volume.path.display()
                ),
                None => String::new(),
            },
            Field::AddVolume => String::new(),
        }
    }
}

/// 三个布尔项转一格：没说 → 开 → 关 → 没说。
fn turn_flag(flag: Option<bool>, step: Step) -> Option<bool> {
    turn(flag, step, |flag| ring(flag, true, next_flag))
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

fn spell_flag(value: Option<bool>, fallback: bool, yes: &str, no: &str) -> String {
    let word = |flag: bool| if flag { yes } else { no };
    match value {
        Some(flag) => word(flag).to_owned(),
        None => format!("默认（{}）", word(fallback)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// 预设那一栏是哪一份文件列出来的。真会话里由 `super::press` 从盘上读来，
    /// 而本模块碰不到盘——这几条用例问的也不是它。
    fn presets_file() -> PathBuf {
        PathBuf::from("配置/tonefit/presets.toml")
    }

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

    /// **哪些键在哪个状态下做什么。** 这一条就是那张表，不靠手点。
    ///
    /// 分四段问：浏览时停在一个转得动的行上、停在一个打字改的行上、停在一个卷行上、
    /// 以及编辑到一半时。每一段都问到「这个键在这里没有意义」那几个——
    /// [`Action::Ignored`] 是一个取值，不是遗漏。
    #[test]
    fn which_keys_do_what_in_which_state() {
        let mut session = Session::new();

        // 一、浏览，光标停在「型号」——一个转得动的行。
        assert_eq!(session.focus(), Field::Profile);
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Right), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
        assert_eq!(session.action(Key::Enter), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Space), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Esc), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 试算与执行在每一行上都按得动：它们与光标停在哪儿无关。
        assert_eq!(
            session.action(Key::Char('t')),
            Action::Start(RunMode::DryRun)
        );
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Start(RunMode::Process)
        );
        // 预设那一栏也在每一行上开得动：存的是整两层，与光标停在哪儿无关。
        assert_eq!(session.action(Key::Char('p')), Action::Pick);
        // 出标定图**只在设备层上**按得动，而「型号」正是设备层的头一行。
        // 型号还没挑也照样派得出来：说不出话的是那一层，不是这张表（见 `chart_profile`）。
        assert_eq!(session.action(Key::Char('c')), Action::Chart);
        // 设备层另外两行上也一样：认的是层，不是某一行。
        for field in [Field::GrayLevels, Field::Threshold] {
            session.focus_on(field);
            assert_eq!(session.action(Key::Char('c')), Action::Chart, "{field:?}");
        }
        session.focus_on(Field::Profile);
        // 打字与补全在浏览时没有意义；删卷的键在不是卷的行上也没有。
        assert_eq!(session.action(Key::Char('z')), Action::Ignored);
        assert_eq!(session.action(Key::Char('d')), Action::Ignored);
        assert_eq!(session.action(Key::Tab), Action::Ignored);
        assert_eq!(session.action(Key::Backspace), Action::Ignored);

        // 二、浏览，光标停在一个打字改的行上：回车进编辑，左右转不动。
        session.focus_on(Field::CacheBudget);
        assert_eq!(session.action(Key::Enter), Action::Edit);
        assert_eq!(session.action(Key::Left), Action::Ignored);
        assert_eq!(session.action(Key::Right), Action::Ignored);
        // 出标定图那个键在**口味层**上没有意义：它出的数是设备层那一格的。
        assert_eq!(session.action(Key::Char('c')), Action::Ignored);

        // 三、浏览，光标停在一个卷行上：空格勾，d 删，左右转不动。
        session.scope.volumes.push(Picked {
            path: PathBuf::from("卷一"),
            on: true,
        });
        session.focus_on(Field::Volume(0));
        assert_eq!(session.action(Key::Space), Action::Toggle);
        assert_eq!(session.action(Key::Enter), Action::Toggle);
        assert_eq!(session.action(Key::Char('d')), Action::Remove);
        assert_eq!(session.action(Key::Left), Action::Ignored);
        // **范围层**上同样没有意义。
        assert_eq!(session.action(Key::Char('c')), Action::Ignored);

        // 四、编辑一个路径：字进缓冲，Tab 补全，回车收下，Esc 丢掉。
        session.focus_on(Field::Out);
        session.press(Key::Enter);
        assert!(matches!(session.mode(), Mode::Editing(_)));
        assert_eq!(session.action(Key::Char('D')), Action::Insert('D'));
        assert_eq!(session.action(Key::Space), Action::Insert(' '));
        assert_eq!(session.action(Key::Backspace), Action::Backspace);
        assert_eq!(session.action(Key::Tab), Action::Complete);
        assert_eq!(session.action(Key::Enter), Action::Commit);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 编辑到一半时光标不动：上下左右一个都不接。
        for key in [Key::Up, Key::Down, Key::Left, Key::Right] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 不该在编辑时生效"
            );
        }

        // 五、编辑一个**不是路径**的行：Tab 没有意义（那一层根本不是路径）。
        session.press(Key::Esc);
        session.focus_on(Field::CacheBudget);
        session.press(Key::Enter);
        assert_eq!(session.action(Key::Tab), Action::Ignored);

        // 六、跑起来之后：三层只读，试算与执行也按不动（一趟里跑不了第二趟）。
        // 按得动的只剩两个：`s`（按停）与 Ctrl-C（退出）。
        session.press(Key::Esc);
        session.run_started();
        for key in [
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Space,
            Key::Tab,
            Key::Backspace,
            Key::Esc,
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('q'),
            Key::Char('d'),
            // 展开那个键跑着时同样按不动（停车场 Q72）：报告还在长。
            Key::Char('e'),
            // 预设那一栏也开不了：套用一份预设就是把两层整个换掉，而这时三层只读。
            Key::Char('p'),
            // 出标定图也按不动：光标此刻不在设备层上——跑着时它根本没停在任何一行上
            // （左栏一格都不反白），而这一刻按得动的只该有停。
            Key::Char('c'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 跑着时不该生效"
            );
        }
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        // Ctrl-C 仍旧退得出去：它在**每一个**状态下都是退出。
        assert_eq!(session.action(Key::Interrupt), Action::Quit);

        // 六之二、**停在决策点上等人拿主意**（`p1-session/14`）：三层照旧只读，
        // 而按得动的换成了答话那两个——`x` 接着做第二遍，`s` 收尾。
        session.at_the_decision_point(true);
        assert!(session.deciding(), "没进等答话那个状态");
        for key in [
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Space,
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
            // `q`／`Esc` 与跑着时同一条（停车场 Q63）：退出会话走中止，
            // 而那一卷此刻正停在决策点上——最容易手滑的两个键不该挂这个后果。
            Key::Esc,
            Key::Char('q'),
            // 起一趟那两个键里 `t` 仍旧按不动：这一趟还没收场。
            Key::Char('t'),
            // 别的那几个照旧：三层只读，报告展不开，预设栏开不了，标定图出不了。
            Key::Char('d'),
            Key::Char('e'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('z'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在决策点上不该生效"
            );
        }
        // 答话那两个键。**`x` 在这里不是「起一趟」，`s` 也不是升闩**——
        // 决策点回的是当场那个字（ADR 0012 决定第 2 条）。
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Answer(Instruction::Continue)
        );
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Answer(Instruction::Finish)
        );
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 答完话回「跑着」那一副，按停那个键又是升闩了。
        session.press(Key::Char('x'));
        assert!(!session.deciding(), "答完话还停在决策点上");
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        assert_eq!(session.action(Key::Char('x')), Action::Ignored);

        // 收场之后配置又改得动，而按停那个键在浏览时没有意义——还没有东西可停。
        session.run_finished();
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('s')), Action::Ignored);
        // 浏览时 `e` 展开，而它与光标停在哪一行无关。
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        session.focus_on(Field::Out);
        assert_eq!(session.action(Key::Char('e')), Action::Expand);

        // 七、展开之后：上下左右改的是报告区，`⇥` 换一卷，`e`／`Esc` 收起。
        // 起一趟的那两个键在这里按不动——报告区正摊着上一趟的逐页。
        session.expand(Expansion::new(0, 3, 0));
        assert_eq!(session.action(Key::Up), Action::Scroll(Toward::Up));
        assert_eq!(session.action(Key::Char('k')), Action::Scroll(Toward::Up));
        assert_eq!(session.action(Key::Down), Action::Scroll(Toward::Down));
        assert_eq!(session.action(Key::Char('j')), Action::Scroll(Toward::Down));
        assert_eq!(session.action(Key::Left), Action::Scroll(Toward::Left));
        assert_eq!(session.action(Key::Right), Action::Scroll(Toward::Right));
        assert_eq!(session.action(Key::Tab), Action::Turn(Step::Next));
        assert_eq!(session.action(Key::BackTab), Action::Turn(Step::Back));
        assert_eq!(session.action(Key::Char('e')), Action::Collapse);
        assert_eq!(session.action(Key::Esc), Action::Collapse);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        for key in [
            Key::Enter,
            Key::Space,
            Key::Backspace,
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('s'),
            Key::Char('d'),
            // 预设那一栏在这里也开不了：它占的是主区，而主区此刻正摊着逐页。
            Key::Char('p'),
            // 出标定图同理：左栏此刻收着，设备层不在屏上。
            Key::Char('c'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 展开着时不该生效"
            );
        }

        // 八、预设那一栏，在列表上走：`↑↓` 挪一行，`⏎`／空格随停在哪一行分派
        // （停在一份预设上是套用它），`p`／`Esc` 回配置。
        session.press(Key::Esc);
        session.pick(vec!["漫画".to_owned(), "画集".to_owned()], presets_file());
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Enter), Action::Take);
        assert_eq!(session.action(Key::Space), Action::Take);
        assert_eq!(session.action(Key::Char('p')), Action::Cancel);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 这一栏上一行只有一个名字：没有取值环，也没有第二件事可做。
        for key in [
            Key::Left,
            Key::Right,
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
            Key::Char('d'),
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('e'),
            // 出标定图也一样：这一栏开着时左栏虽在屏上，光标却归这一栏管
            // （见 `move_cursor`），设备层那三行此刻停不上去。
            Key::Char('c'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在预设那一栏上不该生效"
            );
        }

        // 九、停在末尾那一行（＋ 存成一份新的）上：`⏎` 是「打个名字」，不是套用。
        session.press(Key::Up);
        assert_eq!(session.picking().expect("那一栏开着").picked(), None);
        assert_eq!(session.action(Key::Enter), Action::Edit);
        session.press(Key::Enter);

        // 十、正在打名字：字进缓冲，`⏎` 存下，`Esc` 退回列表。
        assert_eq!(session.action(Key::Char('漫')), Action::Insert('漫'));
        assert_eq!(session.action(Key::Space), Action::Insert(' '));
        assert_eq!(session.action(Key::Backspace), Action::Backspace);
        // 名字不是路径，没有「下一层」可补。
        assert_eq!(session.action(Key::Tab), Action::Ignored);
        // 一个字都没打就按回车是「算了」，与「再打一个卷进来」那一行同一条。
        assert_eq!(session.action(Key::Enter), Action::Cancel);
        session.press(Key::Char('漫'));
        assert_eq!(session.action(Key::Enter), Action::Store);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        for key in [Key::Up, Key::Down, Key::Left, Key::Right, Key::BackTab] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 打名字时不该生效"
            );
        }

        // `Esc` 退的是一步：先回到那一栏的列表上，再按一次才出这一栏。
        session.press(Key::Esc);
        assert!(
            session
                .picking()
                .is_some_and(|picker| picker.naming().is_none()),
            "Esc 一下就把整栏关掉了"
        );
        session.press(Key::Esc);
        assert_eq!(session.mode(), &Mode::Browsing);
    }

    /// **标定图按设备层那块面板画，而阈值一格都不带**（13 号票的第六条：它仍是量具）。
    ///
    /// 型号与灰阶数进图——图的尺寸恒等于面板分辨率，排几条阶梯由灰阶数定；阈值不进，
    /// 因为标定图不经判定。会话这一路与命令行那一路走的是同一个 [`crate::target_profile`]，
    /// 两处解析出来的必须是同一块面板，因此这里问的是**这一层交出去了什么**。
    ///
    /// 同一份设备层拼出的 `Request` 带着那个阈值：两个出口分得开，才谈得上「不带」。
    #[test]
    fn the_chart_is_drawn_for_the_panel_and_carries_no_threshold() {
        let mut session = Session::new();
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        session.device.threshold = Some(3.0);
        session.scope.out = Some(PathBuf::from("出"));

        let chart = session.chart_profile().expect("设备层填齐了");
        assert_eq!(chart.device(), "boox-poke6");
        assert_eq!(chart.panel().gray_levels, 8, "灰阶数没进图");
        assert_eq!(
            chart.threshold(),
            Profile::resolve("boox-poke6")
                .expect("内置型号")
                .threshold(),
            "标定图带上了判定用的界"
        );
        // 同一份设备层，跑一趟用的那个 profile 带着它——两个出口分得开。
        let running = session.request(RunMode::DryRun).expect("三层填齐了");
        assert_eq!(running.profile.threshold().value(), 3.0);

        // 型号没挑：说一句，不是画一张空图。
        let said = Session::new()
            .chart_profile()
            .expect_err("没有型号该说不出话")
            .to_string();
        assert!(said.contains("先挑型号"), "{said}");
    }

    /// **屏底那两行说得出图在哪儿，也说得出此刻要做对的那一件事**（13 号票第三、四条）。
    ///
    /// 措辞取自界面文案那一份，本模块只问它说到了没有：路径、以及「以原尺寸打开」
    /// 连同要关掉的那三个开关。**怎么数不在屏上**——图内已印，`--help` 里也写着。
    #[test]
    fn what_the_screen_says_after_the_chart_is_written() {
        let mut session = Session::new();
        session.charted(Path::new("图/标定.png"));

        let said = session.notice().expect("出完图要说一句");
        assert!(said.contains("标定.png"), "{said}");
        assert!(said.contains("原尺寸"), "{said}");
        for switch in ["缩放", "适配屏幕", "白边裁切"] {
            assert!(said.contains(switch), "少说了「{switch}」：{said}");
        }
        // 两行：屏底那一格不折行，路径与那句话挤在一行必被切掉。
        assert_eq!(said.lines().count(), 2, "{said}");
        // 怎么数不在屏上重抄一遍。
        for copied in ["先看像素完整性", "最右", "回填"] {
            assert!(!said.contains(copied), "屏上抄了「{copied}」：{said}");
        }
        // 下一次按键就把它抹掉，与别处那几句同一条。
        session.press(Key::Down);
        assert!(session.notice().is_none(), "上一句话没抹掉");
    }

    /// **「没说」与「说了一个恰好等于默认值的值」存出去是两份不同的 TOML**（停车场 Q58）。
    ///
    /// 屏上那一格的差别（`默认（height）` 与 `height`）到这一步才落到盘上：前者那一项
    /// **不写**，后者写。不分开的话，存一份「只说了两项」的预设就无从谈起——只能十一项
    /// 全写满，而那意味着套用它时把每一项都盖了一遍，命令行上再想只改一项就没有余地了。
    #[test]
    fn what_was_never_said_is_not_written_and_a_default_that_was_said_is() {
        let spelled = |session: &Session| {
            crate::preset::write(&std::collections::BTreeMap::from([(
                "存出去".to_owned(),
                session.preset(),
            )]))
            .expect("写得出来")
        };

        // 一项都没碰过的会话：两节都在，一个键都没有。
        let untouched = spelled(&Session::new());
        assert!(
            untouched.contains("[preset.\"存出去\".taste]"),
            "{untouched}"
        );
        assert!(
            !untouched.contains("fit"),
            "一项都没说却写出了取值：{untouched}"
        );

        // 把适配方式转到**恰好等于默认值**的那一档上：屏上从「默认（…）」变成那个值本身。
        let mut session = Session::new();
        session.focus_on(Field::Fit);
        assert!(session.shown(Field::Fit).starts_with("默认"));
        session.press(Key::Right);
        assert_eq!(session.taste.fit, Some(FitMode::default()));
        assert_eq!(session.shown(Field::Fit), FitMode::default().name());

        let said = spelled(&session);
        assert!(
            said.contains(&format!("fit = \"{}\"", FitMode::default().name())),
            "说了一个恰好等于默认值的值，盘上却没有它：{said}"
        );
        // 别的项仍是「没说」，一项都没跟着写出去。
        assert!(!said.contains("filter"), "{said}");
    }

    /// 套用一份预设：**两层整个换成它，范围层一格不动**（票面第三条）。
    ///
    /// 「整个换」包括它没说的那几项——套完屏上看到的就是那一份，
    /// 而不是它与上一次配置合出来的某种东西。范围层那一半在这里就地验：
    /// [`Session::preset`] 拼得出来的只有两层，而套用只动那两个字段。
    #[test]
    fn taking_a_preset_swaps_both_layers_and_leaves_the_scope_alone() {
        let mut session = Session::new();
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.volumes.push(Picked {
            path: PathBuf::from("库/卷一"),
            on: true,
        });
        let scope = session.scope.clone();
        // 上一次配的：位深点了名，滤波器也点了名。
        session.taste.bit_depth = Some(BitDepth::Eight);
        session.taste.filter = Some(Filter::Bicubic);

        session.pick(vec!["漫画".to_owned()], presets_file());
        session.took("漫画", crate::preset::every_field());

        assert_eq!(session.preset(), crate::preset::every_field());
        assert_eq!(session.scope, scope, "套用预设动了范围层");
        assert_eq!(session.mode(), &Mode::Browsing, "套完没回到配置上");

        // 套一份**什么都没说的**：上一次点过的那几项跟着回到「没说」，不是留在原处。
        session.pick(vec!["空的".to_owned()], presets_file());
        session.took("空的", crate::preset::Preset::default());
        assert_eq!(session.taste.bit_depth, None);
        assert!(session.shown(Field::Filter).starts_with("默认"));
        assert_eq!(session.scope, scope);
    }

    /// **展开把左栏收起，收起把它原样还回来**（票面第三条）。
    ///
    /// 收起不是删掉：三层一格没动，光标还停在原处——那正是「一键回到配置」的意思。
    #[test]
    fn collapsing_gives_back_everything_expanding_took_away() {
        let mut session = Session::new();
        session.scope.volumes.push(Picked {
            path: PathBuf::from("卷一"),
            on: true,
        });
        session.focus_on(Field::Volume(0));
        session.taste.bit_depth = Some(BitDepth::Four);
        let before = session.clone();

        // 展开：报告区摊开第一卷，左栏这一刻不在屏上（画法那一侧，见 `super::draw`）。
        session.expand(Expansion::new(0, 2, 0));
        assert_eq!(
            session.expansion().map(|expansion| expansion.volume),
            Some(0)
        );
        // 三层在展开着的时候一格都没动——收起来的东西还在原处。
        assert_eq!(session.taste.bit_depth, Some(BitDepth::Four));

        // 收起：一个键（`Esc` 或 `e`）就回到配置，会话与展开之前逐格相同。
        assert_eq!(session.press(Key::Esc), Exit::Stay);
        assert!(session.expansion().is_none(), "收起之后还展开着");
        assert_eq!(session, before, "收起之后会话与展开之前不一样了");

        // 另一个键也收得起来：`e` 是展开那个键按回去。
        session.expand(Expansion::new(1, 2, 40));
        assert_eq!(session.press(Key::Char('e')), Exit::Stay);
        assert_eq!(session, before);
    }

    /// **报告区在两根轴上翻得动，两头都收得住**（票面第四条与停车场 Q64）。
    ///
    /// 往上、往左收在零上——零就是报告的头一行、行首那一列，
    /// 而抬头那几行（profile、适配方式、裁边、拆分）正躺在那儿。
    /// 另外两头由画法那一层每帧收一次：只有它知道这一格装得下几行几列。
    #[test]
    fn the_expanded_report_scrolls_on_both_axes_and_stops_at_both_ends() {
        let mut session = Session::new();
        session.expand(Expansion::new(0, 1, 0));

        // 往上翻不过头一行：报告的第零行就是抬头。
        for _ in 0..5 {
            session.press(Key::Up);
        }
        assert_eq!(session.expansion().expect("展开着").from, 0);

        // 往下翻一行是一行。
        for _ in 0..3 {
            session.press(Key::Down);
        }
        assert_eq!(session.expansion().expect("展开着").from, 3);

        // 横着滚一下走 `SIDEWAYS` 列，往左同样收在行首。
        session.press(Key::Right);
        session.press(Key::Right);
        assert_eq!(session.expansion().expect("展开着").right, 2 * SIDEWAYS);
        for _ in 0..5 {
            session.press(Key::Left);
        }
        assert_eq!(session.expansion().expect("展开着").right, 0);

        // 画法那一层每帧把它收进真滚得动的范围。
        session.press(Key::Down);
        session.press(Key::Right);
        session.clamp_report(1, 0);
        let expansion = session.expansion().expect("展开着");
        assert_eq!(expansion.from, 1, "往下翻过了头没被收回来");
        assert_eq!(expansion.right, 0, "往右滚过了头没被收回来");

        // 没展开的时候收不出事来：那时报告区滚到底，滚动量不是状态。
        session.press(Key::Esc);
        session.clamp_report(0, 0);
        assert!(session.expansion().is_none());
    }

    /// **跑着 ⇄ 等答话是同一趟的两副样子**（`p1-session/14`）：转过去、转回来，
    /// 中间那个闩一格没动。
    ///
    /// 闩非钉不可：决策点上答的字**不是闩**（ADR 0012 决定第 2 条）。第一遍里按下的收尾
    /// 要在答完话之后照旧作数——被决策点抹掉的话，那一趟会一直跑到最后一卷。
    /// 反过来，答话也不该把闩往上推：`s` 在决策点上答的是「这一卷的第二遍不做了」，
    /// 而不是「这一趟不走了」。
    #[test]
    fn the_decision_point_is_a_second_face_of_the_same_run_and_leaves_the_latch_alone() {
        let mut session = Session::new();
        // 没跑着的时候这一问不作数：决策点是一趟跑起来之后才有的事。
        session.at_the_decision_point(true);
        assert_eq!(session.mode(), &Mode::Browsing, "没跑着也进了等答话");
        assert!(!session.deciding());

        session.run_started();
        // 第一遍里按了一次停：闩记着收尾。
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Finish);

        // 决策点到了：换一副样子，闩原样带过去。
        session.at_the_decision_point(true);
        assert!(matches!(session.mode(), Mode::Deciding(_)));
        assert_eq!(session.stopping(), Instruction::Finish, "转过去把闩弄丢了");

        // 答一个收尾：状态当场转回「跑着」，而闩仍旧是那一级——答话不是升闩。
        assert_eq!(session.press(Key::Char('s')), Exit::Stay);
        assert!(matches!(session.mode(), Mode::Running(_)), "答完话没转回去");
        assert_eq!(session.stopping(), Instruction::Finish, "答话把闩推上去了");

        // 那一趟停在决策点上被中止时收不到「跑完」以外的东西，收场那一下照样回浏览。
        session.at_the_decision_point(true);
        assert!(session.deciding());
        session.run_finished();
        assert_eq!(session.mode(), &Mode::Browsing, "等答话时收场没回浏览");
        assert_eq!(session.stopping(), Instruction::Continue);
    }

    /// **两级停：同一个键按两次。** 一次收尾，再一次中止，第三次没有意义。
    ///
    /// 两级的定义在 ADR 0013：收尾是当前卷跑完才停，中止是当场停。
    /// 这一条问的是「按下去之后闩升到哪一级」，停出来的现场对不对是库那一侧的事
    /// （`tests/events.rs` 的两个检查点）。
    #[test]
    fn one_key_pressed_twice_is_the_two_stage_stop() {
        let mut session = Session::new();
        // 还没跑起来：按停没有意义，闩也不动。
        assert_eq!(session.press(Key::Char('s')), Exit::Stay);
        assert_eq!(session.stopping(), Instruction::Continue);

        session.run_started();
        assert_eq!(session.stopping(), Instruction::Continue, "起手没按过");

        // 一次收尾。
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Finish);
        // 再一次中止。
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Abort);
        // 第三次起没有更强的一级可去：那个键从此没有意义，闩也不再动。
        assert_eq!(session.action(Key::Char('s')), Action::Ignored);
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Abort, "闩退回去了");

        // 按停之后会话仍开着——停下来不是退出（本票的验收）。
        assert_eq!(session.press(Key::Char('s')), Exit::Stay);
        assert!(matches!(session.mode(), Mode::Running(_)));

        // 那一趟收场：回到浏览，配置又改得动，闩跟着这一趟一起走。
        session.run_finished();
        assert_eq!(session.mode(), &Mode::Browsing);
        assert_eq!(session.stopping(), Instruction::Continue);

        // 下一趟从头起：上一趟按下的停没有漏过来。
        session.run_started();
        assert_eq!(
            session.stopping(),
            Instruction::Continue,
            "上一趟的停漏过来了"
        );
    }

    /// 试算与执行拼出来的 `Request` **只差 `mode` 一格**，其余逐项相同。
    ///
    /// 跑不起来的那两种（型号没挑、输出根没填）当场说得出口，而不是拼出一个
    /// 编造了默认值的请求交给库。
    #[test]
    fn a_trial_and_a_run_differ_only_in_how_far_they_go() {
        let mut session = Session::new();

        // 两项必填都缺：先说型号，那是判定的依据。
        let said = session.request(RunMode::DryRun).expect_err("跑不起来");
        assert!(format!("{said:#}").contains("先挑型号"), "{said:#}");

        session.device.profile = Some("kobo-libra-2".to_owned());
        let said = session.request(RunMode::DryRun).expect_err("跑不起来");
        assert!(format!("{said:#}").contains("输出根"), "{said:#}");

        session.scope.out = Some(PathBuf::from("出"));
        session.scope.volumes = vec![
            Picked {
                path: PathBuf::from("库/卷一"),
                on: true,
            },
            // 勾掉的那一条不进这一趟：打错一条勾掉就是了（spec 的 story 16）。
            Picked {
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

    /// 一项都没改的会话拼出来的，与**一个 flag 都不加的命令行**拼出来的逐项相同。
    ///
    /// 「命令行没点、预设也没说」那一档与会话读的是同一个（`preset::TasteLayer` 那几个
    /// 方法），这一条钉的就是那件事：默认值没有第二个出处。
    #[test]
    fn an_untouched_session_asks_for_what_a_bare_command_line_asks_for() {
        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.volumes = vec![Picked {
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

        assert_eq!(asked.fit, command_line.fit);
        assert_eq!(asked.crop, command_line.crop);
        assert_eq!(asked.split, command_line.split);
        assert_eq!(asked.filter, command_line.filter);
        assert_eq!(asked.bit_depth, command_line.bit_depth);
        assert_eq!(asked.dither, command_line.dither);
        assert_eq!(asked.per_page, command_line.per_page);
        assert_eq!(asked.cache_budget, command_line.cache_budget);
        assert_eq!(asked.io_mode, command_line.io_mode);
        assert_eq!(asked.metadata, command_line.metadata);
        assert_eq!(asked.inputs, command_line.inputs);
        assert_eq!(asked.output_root, command_line.output_root);
    }

    /// 退出只由那几个键说得出来，别的键按到底都退不出去。
    #[test]
    fn only_the_quit_keys_leave_the_session() {
        for key in [Key::Char('q'), Key::Esc, Key::Interrupt] {
            assert_eq!(Session::new().press(key), Exit::Leave, "{key:?} 该退出");
        }
        let mut session = Session::new();
        for key in [
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Space,
            Key::Tab,
            Key::Backspace,
            Key::Char('x'),
            Key::Char('s'),
        ] {
            assert_eq!(session.press(key), Exit::Stay, "{key:?} 不该退出");
        }
    }

    /// 编辑到一半按 Ctrl-C 照样退得出去：它在**每一个**状态下都是退出。
    #[test]
    fn an_interrupt_leaves_even_in_the_middle_of_typing() {
        let mut session = Session::new();
        session.focus_on(Field::GrayLevels);
        session.press(Key::Enter);
        assert!(matches!(session.mode(), Mode::Editing(_)));

        assert_eq!(session.press(Key::Interrupt), Exit::Leave);
    }

    /// 左栏就是三块，次序是设备层 → 口味层 → 范围层，各项都在。
    #[test]
    fn the_left_column_is_three_layers_in_lifecycle_order() {
        let mut session = Session::new();
        session.scope.volumes.push(Picked {
            path: PathBuf::from("卷一"),
            on: true,
        });

        let layers: Vec<Layer> = session.rows().iter().map(|field| field.layer()).collect();
        let mut seen: Vec<Layer> = Vec::new();
        for layer in layers {
            if seen.last() != Some(&layer) {
                assert!(!seen.contains(&layer), "{layer:?} 那一块被拆散了");
                seen.push(layer);
            }
        }
        assert_eq!(seen, vec![Layer::Device, Layer::Taste, Layer::Scope]);
    }

    /// 屏上的两层与预设装的两层是**同一层**：格数一项不多一项不少。
    ///
    /// 断的**不是** `TASTE_FIELDS.len() == 11`——那个数写在类型里，永远红不了。
    /// 断的是它与**盘上那份预设**的格数对得上：`preset::write` 把一份说满了的
    /// `Preset`（`preset::every_field`，没有 `..Default::default()`）写成 TOML，
    /// 那两节里各有几个键，两层就各有几格。往 `TasteLayer` 加一个字段而左栏没跟着加一行，
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

        assert_eq!(spelled("taste"), TASTE_FIELDS.len(), "口味层：{text}");
        assert_eq!(spelled("device"), DEVICE_FIELDS.len(), "设备层：{text}");

        let session = Session::new();
        for field in session.rows() {
            assert!(!field.label().is_empty(), "{field:?} 没有名字");
            if !matches!(field, Field::AddVolume) {
                assert!(!session.shown(field).is_empty(), "{field:?} 印不出取值");
            }
        }
    }

    /// 每一项都有一个「没说」的位置，转一圈回得到它——预设的 `Option` 在屏上就长这样。
    #[test]
    fn every_setting_has_a_says_nothing_position_and_comes_back_to_it() {
        let mut session = Session::new();
        let cyclable: Vec<Field> = session
            .rows()
            .into_iter()
            .filter(|field| field.shape() == Shape::Cycle)
            .collect();
        assert!(!cyclable.is_empty());

        for field in cyclable {
            session.focus_on(field);
            let start = session.shown(field);
            assert!(
                start.starts_with("默认")
                    || matches!(field, Field::Profile | Field::BitDepth | Field::Dither),
                "{field:?} 一开始不是「没说」：{start}"
            );

            // 往前转一圈，一路上每一格都印得出来，最后回到出发点。
            let mut turns = 0;
            loop {
                session.press(Key::Right);
                turns += 1;
                assert!(turns < 64, "{field:?} 的取值环转不回去");
                assert!(!session.shown(field).is_empty());
                if session.shown(field) == start {
                    break;
                }
            }
            assert!(turns >= 2, "{field:?} 的取值环只有一格");

            // 反着转一格就该落在环上最后一格，再转回来仍是出发点。
            session.press(Key::Left);
            assert_ne!(session.shown(field), start);
            session.press(Key::Right);
            assert_eq!(session.shown(field), start);
        }
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

    /// 阈值那一行印的是**数值加标定来源**，与报告里那一行逐字相同。
    ///
    /// spec 的 Further Notes：「会话里显示时照报告的写法把来源原样带上来，
    /// 不自己另编一套说法。」标定来源是阈值的一部分（ADR 0002：判据跟着面板走、
    /// 不可跨面板比较），会话另写一句就等于把那半句丢了。
    #[test]
    fn the_threshold_row_prints_the_source_the_report_prints() {
        let mut session = Session::new();

        // 还没挑型号：界挂在 profile 上，这一行印不出数来。
        assert!(!session.shown(Field::Threshold).contains("阈值"));

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
        assert!(printed.contains("其余面板未复核"), "{printed}");

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
        assert!(pinned.contains("点名指定"), "{pinned}");
    }

    /// 换型号把设备层那两个覆盖项一起清掉：它们是在**上一块面板**上量出来的（ADR 0002）。
    #[test]
    fn changing_the_model_drops_the_numbers_measured_on_the_old_one() {
        let mut session = Session::new();
        session.press(Key::Right);
        let first = session.device.profile.clone().expect("挑到了一个型号");
        session.device.gray_levels = Some(12);
        session.device.threshold = Some(5.2);

        session.press(Key::Right);

        assert_ne!(session.device.profile, Some(first));
        assert_eq!(session.device.gray_levels, None, "灰阶数跟着换了面板");
        assert_eq!(session.device.threshold, None, "阈值跟着换了面板");
    }

    /// 设备层那两个覆盖项**要先挑型号**——与预设那一侧同一条规矩（ADR 0002）。
    #[test]
    fn a_calibrated_override_needs_a_panel_to_be_measured_on() {
        let mut session = Session::new();
        session.focus_on(Field::GrayLevels);

        session.press(Key::Enter);
        for character in "12".chars() {
            session.press(Key::Char(character));
        }
        session.press(Key::Enter);

        // 没挑型号：留在编辑态，话说得出来。
        assert!(matches!(session.mode(), Mode::Editing(_)));
        assert!(session.notice().expect("要说一句").contains("先挑型号"));
        assert_eq!(session.device.gray_levels, None);

        // 挑了型号之后同一个数收得下，越界的数仍被库那一侧的界挡下。
        session.press(Key::Esc);
        session.device.profile = Some("boox-poke6".to_owned());
        session.focus_on(Field::GrayLevels);
        session.press(Key::Enter);
        session.press(Key::Char('1'));
        session.press(Key::Char('2'));
        session.press(Key::Enter);
        assert_eq!(session.device.gray_levels, Some(12));

        session.press(Key::Enter);
        session.press(Key::Backspace);
        session.press(Key::Backspace);
        session.press(Key::Char('0'));
        session.press(Key::Enter);
        assert!(matches!(session.mode(), Mode::Editing(_)), "0 级该被挡下");
        assert_eq!(session.device.gray_levels, Some(12), "挡下的值没有写进去");
    }

    /// 打错的取值留在编辑态，不把用户打的东西丢掉。
    #[test]
    fn a_value_that_does_not_parse_stays_in_the_editor() {
        let mut session = Session::new();
        session.focus_on(Field::CacheBudget);

        session.press(Key::Enter);
        for character in "512T".chars() {
            session.press(Key::Char(character));
        }
        session.press(Key::Enter);

        let Mode::Editing(edit) = session.mode() else {
            panic!("解析不过该留在编辑态");
        };
        assert_eq!(edit.buffer, "512T", "用户打的东西被丢掉了");
        assert!(session.notice().is_some(), "解析不过要说一句");
        assert_eq!(session.taste.cache_budget, None);

        // 改对了就收得下，回到浏览。
        session.press(Key::Backspace);
        session.press(Key::Char('M'));
        session.press(Key::Enter);
        assert_eq!(session.mode(), &Mode::Browsing);
        assert_eq!(
            session.taste.cache_budget,
            Some(CacheBudget::parse("512M").expect("认得的写法"))
        );
    }

    /// 空串是「没说」：清掉一项就落回默认值，而不是落到 0 上。
    #[test]
    fn clearing_a_field_puts_it_back_to_saying_nothing() {
        let mut session = Session::new();
        session.taste.cache_budget = Some(CacheBudget::parse("1G").expect("认得的写法"));
        session.focus_on(Field::CacheBudget);

        session.press(Key::Enter);
        let Mode::Editing(edit) = session.mode() else {
            panic!("该进编辑态");
        };
        assert_eq!(edit.buffer, "1G", "进编辑时缓冲里该摆着当前取值");
        for _ in 0..edit.buffer.len() {
            session.press(Key::Backspace);
        }
        session.press(Key::Enter);

        assert_eq!(session.taste.cache_budget, None);
        assert!(session.shown(Field::CacheBudget).starts_with("默认"));
    }

    /// 打进来的卷勾得掉，也删得掉（spec 的 story 16）。
    #[test]
    fn a_volume_that_was_typed_in_can_be_ticked_off_and_removed() {
        let mut session = Session::new();
        session.focus_on(Field::AddVolume);

        // 打两个卷进来。每打完一个，光标仍停在「再打一个」上。
        for name in ["卷一", "卷二"] {
            session.press(Key::Enter);
            for character in name.chars() {
                session.press(Key::Char(character));
            }
            session.press(Key::Enter);
            assert_eq!(session.focus(), Field::AddVolume);
        }
        assert_eq!(session.scope.volumes.len(), 2);
        assert!(session.scope.volumes.iter().all(|volume| volume.on));

        // 勾掉第二个：它还在清单上，只是这一趟不算数。
        session.focus_on(Field::Volume(1));
        session.press(Key::Space);
        assert!(!session.scope.volumes[1].on);
        assert!(session.shown(Field::Volume(1)).starts_with("[ ]"));
        session.press(Key::Space);
        assert!(session.scope.volumes[1].on);

        // 删掉第二个：清单上就没有它了，光标不会掉到外面去。
        session.press(Key::Char('d'));
        assert_eq!(session.scope.volumes.len(), 1);
        assert_eq!(session.scope.volumes[0].path, PathBuf::from("卷一"));
        assert!(session.cursor < session.rows().len());
    }

    /// 光标上下都绕得回来，绕一圈正好是行数。
    #[test]
    fn the_cursor_wraps_around_the_left_column() {
        let mut session = Session::new();
        let rows = session.rows().len();

        for _ in 0..rows {
            session.press(Key::Down);
        }
        assert_eq!(session.focus(), Field::Profile, "转一圈没回到第一行");

        session.press(Key::Up);
        assert_eq!(session.focus(), Field::AddVolume, "往上一格没绕到最后一行");
    }
}
