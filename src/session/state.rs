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
//! # 此刻在做什么由两维说（ADR 0017）
//!
//! [`Stage`]（这一趟走到哪个阶段了：没跑过 / 跑着 / 等答话 / 收场了）与
//! [`Focus`]（眼下在看什么：左栏 / 编辑一行 / 取值栏 / 报告区 / 展开着 / 预设栏）。
//! **两维各答各的**：三层只读、按停与答话那几个键归第一维（[`stage_action`] 一处答完），
//! `↑↓`／`⏎`／`⇥` 归第二维。那张按键表因此按两维查，而它仍旧是本仓库唯一一处
//! 答得出「哪些键在哪个状态下有效」的地方。
//!
//! `p1-session/10`、`11`、`14` 先后三次判过「不拆」，推翻的理由与代价在 **ADR 0017**。
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
    BitDepth, CacheBudget, Dither, Filter, FitMode, Instruction, IoMode, Mode as RunMode, Panel,
    Profile, ReadingOrder, Request, SplitThreshold,
};

use super::complete;
use super::live::{Branch, Live, Reach, Volume};
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
    /// **不进缓冲的那一个键**：`F1`。它掀开[全部键那一张](Overlay::Keys)，
    /// 而**打字的那两块上它是唯一掀得开的**（`p4-parking-lot/07` 票面第三条，
    /// 停车场 Q165）——那儿每一个字符都进缓冲，`?` 与 `i` 也是字，
    /// 「看不见的东西等于不存在」在那两块上因此一直成立。
    ///
    /// **挑 `F1` 的理由**：它不是一个字符，进不了任何一个缓冲；「按 F1 看帮助」
    /// 是键盘上最不必学的一条；而**它在六块焦点、四个阶段上一处都没有主**
    /// （用例 `the_key_table_is_asked_of_the_key_table_itself` 逐块问出来的，
    /// 与 `p3-session-legibility/12` 挑 `i` 时同一个做法）——`a` 那种撞车
    /// （停车场 Q161）在它身上不存在。
    ///
    /// 代价记在这里：**终端自己截走它的话，会话一个字都收不到**（有几种终端
    /// 把 `F1` 绑在自己的帮助上）。出路一个不少——那两块退出去是一个 `Esc`，
    /// 出去就有 `?`。
    F1,
}

/// 换一个取值时往哪边转。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Back,
}

/// **展开那一副列的是哪几页**（`CONTEXT.md` 的《会话》：要紧的页）。
///
/// 一个枚举而不是一个 `bool`：它从 [`Action::List`] 一路传到
/// [`super::draw::pages`]，而调用处一个裸 `true` 说不出它列的是哪一批
/// （与 [`super::live::Resuming`] 同一条理由——本仓库不爱看不出意思的裸值）。
///
/// **默认那一档是[只列要紧的](Self::Notable)**：展开一卷的目的通常只有一个——
/// 哪一页把整卷拉下来——而两百页的卷里那几页不该由用户自己在四百行里找
/// （`p3-session-legibility/11`）。哪几页算要紧的判据在 [`crate::render::notable`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Listing {
    /// **只列要紧的页**：特例 · 失败 · 部分救回 · 几何门不成立 · 宽溢出 · 兜底上界，
    /// 加上**定档页**（它是这一卷的答案，非在不可）。
    #[default]
    Notable,
    /// **全部页**：`a` 切过来的那一档。
    All,
}

impl Listing {
    /// 按一下 `a` 之后是哪一档。**两档来回**，与两级停那个只升不降的闩正相反：
    /// 这一下是看法，不是决定，按错了再按一次就回来了。
    fn flipped(self) -> Self {
        match self {
            Self::Notable => Self::All,
            Self::All => Self::Notable,
        }
    }
}

/// 一个键在当前状态下会做的那件事。**这就是「哪些键在哪个状态下有效」那张表的值域。**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 光标上下移动。
    Move(Step),
    /// 就地把当前项换成取值环上的下一个（或上一个）。
    Cycle(Step),
    /// **摊开取值栏**：在光标那一行**下面**摊开它那一列取值
    /// （`CONTEXT.md` 的《会话》：取值栏）。
    ///
    /// 只有[转得动的行](Shape::Cycle)派得出它。摊开的是什么随那一行而变：九行摊的是
    /// **那一行的取值环**，**型号那一行摊的是面板**——它是两层的那一行
    /// （[`Field::drills`]），第二层由[下钻](Self::Drill)进去。
    ///
    /// 它要的东西本模块全有（那一行的取值环、内置表的分组都在手边），因此不必像
    /// [展开](Self::Expand)那样落到 [`super::press`] 去：真做这件事的就是
    /// [`Session::unfold`]。
    Unfold,
    /// **下钻**：进到取值栏上停着的那一格**底下那一层**
    /// （`CONTEXT.md` 的《会话》：下钻）。
    ///
    /// **只有型号那一行有第二层**（[`Field::drills`]，spec 的《取值栏》：
    /// 下钻只有那一处有两层）：第一层摆的是**面板**，而面板不是型号那一行的一个取值
    /// ——定不下来，只能进去看它底下有哪几个型号。**设备只是面板的别名，多对一**
    /// （`CONTEXT.md`），面板相同的型号输出完全一致，挑替身因此从面板挑起。
    ///
    /// **它与[定](Self::Choose)是两个动作**：一个换一层看，一个写取值。派哪一个
    /// 随光标停着的那一格而变（[`Values::at_a_panel`]）——面板那一层的第一格「没挑」
    /// 是型号那一行真正的一个取值，它派的是定。
    ///
    /// 退回上一层走的是[退一步](Self::Cancel)，与打预设名退回那一栏的列表同一条。
    Drill,
    /// **定**：把取值栏上光标停着的那一格收下（票面：`⏎` 定）。
    ///
    /// **走的就是环那一套本身**（[`Session::cycle`] 转的那个环）：摊开选出来的值与
    /// 就地转出来的值因此改的是同一格、走的是同一条写入路径，
    /// 「摊开选一个」与「转一格」改出两种结果那件事结构上不存在（票面第五条一带）。
    ///
    /// **不叫 `Commit`。** 那个是[收下编辑缓冲里的东西](Self::Commit)，
    /// 而这一下没有缓冲可收——它挑的是一列现成的取值里的一格。
    Choose,
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
    /// **退一步**：丢掉眼下这一步，回到上一步。
    ///
    /// 编辑中是丢掉缓冲回浏览；打预设名时是退回那一栏的列表（见 [`naming_action`]）；
    /// **取值栏上是一格不改地回左栏**（`p3-session-legibility/05` 票面第三条），
    /// [下钻](Self::Drill)进去之后则是**退回上一层**（型号那一行的面板那一层）——
    /// 「退一步，不是退到底」在三处是同一个形状。
    ///
    /// 「一格不改」不必靠任何一处代码守着：这一支根本不写取值，
    /// 写的只有[定](Self::Choose)那一支。
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
    /// 升到哪一级由状态机自己记（[`Stage::Running`] 那一格）；把它交给跑着的那一趟
    /// 在 [`super::press`] 那一层——本模块不碰线程，与[起一趟](Self::Start)同一条规矩。
    Stop,
    /// **在决策点上答一个字**，连同这个字[管几卷](Reach)（`CONTEXT.md` 的《会话》：
    /// 决策点、都这样）。
    ///
    /// 只在 [`Stage::Deciding`] 那个状态下派得出来，而**每一卷的试算都到得了**那个状态。
    /// 三个字各有一个键：`x` 答[继续](Instruction::Continue)——第一遍不重算，直接进第二遍；
    /// `s` 答[收尾](Instruction::Finish)——这一卷一个字节都不写，等价于一次 dry-run；
    /// `a` 答的是**同一个继续，外加剩下的卷都这样**（[`Reach::ForTheRest`]）——
    /// 往下的决策点不再停，几十卷的一趟因此按一下就挂得住。
    ///
    /// **它带着那个字，而不是像[按停](Self::Stop)那样让状态机自己升一级**：
    /// 决策点回的是**当场那个字**，不是闩（ADR 0012 决定第 2 条）。三个键因此是三个方向，
    /// 不是同一个键按几次。
    ///
    /// **「剩下的卷都这样」同样不进闩**：闩只升不降，而它是个可以是「继续」的粘性答案。
    /// 它记在观察者那一侧的「决策点的默认答案」上（`super::run::Gate`），
    /// 与库那一侧记「至今收到过最强指令」的那一格分开放——按停按到的那一级一格不动。
    ///
    /// [中止](Instruction::Abort)不从这条路出去：等答话时按 `Ctrl-C` 是[退出会话](Self::Quit)，
    /// 而退出会话本来就走中止（`super::run::Running::leave`，停车场 Q63）——
    /// 那一卷等于没做，`partial` 也没留下。
    ///
    /// 把那个字送到计算线程上在 [`super::press`] 那一层（`Running::decide`），
    /// 与[按停](Self::Stop)同一条规矩：本模块不碰线程。
    Answer(Instruction, Reach),
    /// **切焦点**：左栏 ⇄ 报告区（`CONTEXT.md` 的《会话》：焦点，ADR 0017）。
    ///
    /// **`⇥` 按状态分派，三个意思不冲突**：焦点在左栏或报告区上时是这一支；
    /// [编辑一行](Self::Complete)时是逐层补全（那一层没有第二栏可切）；
    /// [展开着](Self::Turn)时是换下一卷。三处各是那个键在那个状态下最该做的事，
    /// 而「哪个键在哪个状态下做什么」这张表本来就一处答完（[`Session::action`]）。
    ///
    /// 带着[去哪一边](Pane)：屏底那一行要说出这一下去哪儿，而一个 toggle 说不出。
    Focus(Pane),
    /// **在卷表上挪一卷**：光标往上／往下一卷，两头都绕回去
    /// （`p3-session-legibility/10` 票面第二条）。
    ///
    /// 挪完[跟随就停了](Follow::Stopped)——票面第三条：光标一动就停，屏上说一句。
    ///
    /// 落在 [`super::press`] 那一层：挪到哪一卷要数**这一趟此刻有哪几卷**，
    /// 而本模块读不到那一趟攒下来的东西。与[展开](Self::Expand)同一条分法。
    Select(Step),
    /// **回到跟随**：卷表上那个光标交回给最新的那一卷（`g`，票面第三条）。
    ///
    /// 真做这件事的就在本模块（[`Session::follow_along`]）：它一格报告都不必读——
    /// 「跟着最新那一卷」是**算出来的**，而这一支只把那件事扳回去。
    Follow,
    /// **展开一个目录**：把[光标停着的那一枝](Session::standing)底下那几卷摊成卷表
    /// （`volume-discovery/08`，`CONTEXT.md` 的《会话》：展开的头一级）。
    ///
    /// **它是两级展开里的头一级**：报告区默认那一副是**目录表**（一个目录一行），
    /// 这一下摊出那一枝底下那几卷，再一下（[展开](Self::Expand)）才是逐页。
    /// 层次与**发现出来的那棵树**一致——分组只有 `crate::render::grouped` 一处出处，
    /// 会话这一侧一棵树都不另切。
    ///
    /// **左栏这一级不收起**：收起是[展开到页](Self::Expand)带着的那一件事
    /// （逐页那几行轻松过 100 列），而卷表那几列摆得下，左栏留着才对得上三层。
    ///
    /// 挪到哪一枝要数**此刻有哪几枝**，而本模块读不到那一趟攒下来的东西——
    /// 真做这件事的因此是 [`super::press`]，与[展开](Self::Expand)同一条分法。
    Open,
    /// **展开一卷**：把[光标停着的那一卷](Session::standing)的逐页那几行摊开来，
    /// 左栏跟着收起（`CONTEXT.md` 的《会话》：展开的第二级）。
    ///
    /// **展开的是光标停着的那一卷，不再恒是第一卷**（`p3-session-legibility/10`）：
    /// 报告区从此有自己的光标，而「展开哪一卷」正是它答的那件事。
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
    /// 与[展开](Self::Expand)同样落在 [`super::press`] 那一层：挪到哪一卷要数
    /// **此刻有哪几卷**，而本模块读不到那一趟攒下来的东西。
    Turn(Step),
    /// **收起一级**：退回展开进来的那一级（`volume-discovery/08` 票面第二条：
    /// 两级展开各有一个键回得去）。
    ///
    /// 与展开不对称——收起不必读报告，因此它就在本模块做掉。**退的恒是一级**：
    /// [展开着一卷](Focus::Expanded)时回到[它那一枝的卷表](Focus::Opened)，
    /// [展开着一枝](Focus::Opened)时回到[目录表](Focus::Report)。
    /// 一个动作而不是两个：它答的是同一句话——「退到上一级去」，
    /// 而退到哪一级由眼下站在哪一级说了算（与[退一步](Self::Cancel)同一个形状）。
    Collapse,
    /// **换一副列法**：只列[要紧的页](Listing::Notable) ⇄ 列[全部页](Listing::All)
    /// （`a`，票面第二条）。
    ///
    /// **带着去哪一档**，与[切焦点](Self::Focus)同一个形状、同一条理由：
    /// 屏底那一行要说出这一下按过去是什么样，而一个 toggle 说不出。
    ///
    /// **等答话时这个键不派它**（见 [`expanded_action`]）：那一刻 `a` 是
    /// [「剩下的卷都这样」](Self::Answer)，而答话那三个键在哪一块上都按得动
    /// （ADR 0017 决定第 4 条）。两个意思撞在同一个字母上，让的是这一个——
    /// 换一副列法等得起，一条停在决策点上的线程等不起。停车场 Q161 记着这一笔。
    List(Listing),
    /// **去预设那一栏**：把盘上那份文件里有的那几份列出来（`CONTEXT.md` 的《会话》：预设栏）。
    ///
    /// 列什么要读盘，而本模块读不到——真做这件事的是 [`super::press`]，它随后调
    /// [`pick`](Session::pick)。与[展开](Self::Expand)同一条分法：那一支要读那一趟攒的报告，
    /// 这一支要读用户配置目录下那份 TOML；名字也照那一对取
    /// （`Expand` 进 [`Focus::Expanded`]，`Pick` 进 [`Focus::Picking`]）。
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
    /// **删**：把光标停着的那一份预设从盘上删掉。
    ///
    /// 落在 [`super::press`]，与上面那三支同一条：它要动用户配置目录下那份 TOML。
    ///
    /// **要按两下**（与两级停、与覆盖同一个形状，ADR 0013）：第一下只说一句
    /// （见 [`Session::ask_before_erasing`]），第二下才真删。
    /// **不叫 `Remove`**——那个是[删掉卷清单上的一行](Self::Remove)，
    /// 而两者不是一个量级：那一条是这一趟屏上的一行，这一条是盘上长期存着的东西，
    /// 按错一下没有撤销。
    ///
    /// **停在末尾那一行上派不出它**（见 [`listing_action`]）：那一行不是一份预设。
    Erase,
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
    /// **掀开一张[覆盖层](Overlay)**：屏上那几块整个让位，只摆它那一份
    /// （`p3-session-legibility/12`）。
    ///
    /// **两张共一个动作**，与[切焦点](Self::Focus)、[换一副列法](Self::List)同一个形状：
    /// 带着**掀开的是哪一张**，而不是两个动作——两张是同一副形状（一张画法，两份内容），
    /// 分成两个动作就要在按键表、屏底那一行、画法三处各分一次岔。
    ///
    /// **掀着一张时按另一张那个键是换过去**，不叠第二层（见 [`Session::reveal`]）：
    /// 覆盖层盖住的恒是**焦点那一维上的一块**，而不是另一张覆盖层。
    ///
    /// [这一趟的前提](Overlay::Premises)那一张落在 [`super::press`]：它印的是这一趟的
    /// 报告抬头，而本模块读不到那一趟攒下来的东西（与[展开](Self::Expand)同一条分法）。
    /// [键位表](Overlay::Keys)那一张就在本模块做掉——它要的东西本模块全有
    /// （[`Session::key_table`] 问的就是这张按键表自己）。
    Reveal(Overlay),
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

    /// **这一行摊开的是两层吗**（`CONTEXT.md` 的《会话》：下钻）。
    ///
    /// **摊得开的就是取值是[环](Shape::Cycle)的那几行**，不必另立一个谓词；
    /// 分岔只剩这一处：**型号那一行摊开的是面板**（内置表里有几块就是几块，
    /// 每一行是那块面板自己的 `Display`：分辨率 · PPI · 灰阶数 · 黑白／彩色），
    /// `→` 再[下钻](Action::Drill)到那块面板底下的型号——两层，与别处那一层不是一个
    /// 形状（spec 的《取值栏》：下钻只有那一处有两层）。
    ///
    /// **「哪一行是两层」只有这一处出处**：摊开那一下（[`Session::unfold`]）、
    /// 光标停着的这一格是不是一块面板（[`Values::at_a_panel`]）、退一步退到哪儿
    /// （[`Session::cancel`]）问的都是它。与 [`around`] 那条「写两份就会有一处忘了绕」
    /// 同一条规矩。
    ///
    /// **它还是「取值恒在环上」那条前提的守门人**：摊开走**环**那一路的九行取值都是
    /// 枚举或布尔，一格不落地都在自己的环上——[`Session::unfold`] 数「此刻生效的是
    /// 第几格」靠的就是这条前提。唯一可能落在环外的是型号（预设里塞进来的一个已删型号，
    /// 见 [`next_device`]），而它走的正是另一路：面板那一层认的是**这个名字在哪一块
    /// 面板底下**，认不出来就一格都不标（`p3-session-legibility/06` 票面第七条）。
    pub fn drills(self) -> bool {
        self == Field::Profile
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

/// **这一趟走到哪个阶段了**——会话两维中的**第一维**（ADR 0017）。
///
/// 四个取值答的是同一个问题：这一趟走到哪儿了。另一维（[`Focus`]）答的是
/// 「眼下在看什么」。`p1-session/10`、`11`、`14` 先后三次判过「不拆成两维」，
/// 推翻的理由与代价在 **ADR 0017**。
///
/// **三层只读由这一维说了算**（[`read_only`](Self::read_only)），**与焦点在哪无关**：
/// 焦点落到报告区上不解锁任何一个改动键（见 [`Session::action`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// **一趟都还没跑过。** 报告区里连一卷都没有，`⇥` 因此切不过去——
    /// 屏上不摆按不动的键，而切过去无事可做（见 [`Session::browsing_action`]）。
    Fresh,
    /// 一趟正跑着，带着**按停按到哪一级了**。
    ///
    /// **三层全只读**，两层各错在什么地方见 `CONTEXT.md` 的《会话》（`p1-session/10`
    /// 把范围层也算了进来，停车场 Q69）。
    ///
    /// 「只读」不是靠画法上灰，是靠 [`Session::action`] 在这个阶段下一个改动键都不派
    /// （见 [`running_action`]）。画法那一侧另有一份**看得出来**的交代
    /// （左栏抬头写着「只读」、光标不反白），那是 [`super::draw::config::config`] 的事。
    ///
    /// 那一格装的是[闩](Session::stopping)：`Continue` 是没按过、`Finish` 是按过一次
    /// （收尾）、`Abort` 是再按了一次（中止）。**只升不降**——按停不是一个可以反悔的开关
    /// （`CONTEXT.md` 的《进度》）。
    Running(Instruction),
    /// 一趟**停在决策点上等人拿主意**（`CONTEXT.md` 的《会话》：续做与决策点，
    /// ADR 0012 决定第 3 条）。
    ///
    /// **试算到得了这里，几卷都一样，一卷一次**（决定第 3 条，`volume-discovery/07`）：
    /// 决策点本来就是逐卷的，逐卷停下来问，缓存始终只押着当前那一卷，内存一点不涨。
    /// 执行那一趟不在这儿停——用户按 `x` 的时候已经拿过主意了。
    /// 答过[「剩下的卷都这样」](Action::Answer)之后也不再到这里：往下的决策点由观察者
    /// 那一侧的默认答案当场答掉（`super::run::Gate`），那条线程根本不停下来。
    ///
    /// **它是这一维上的一个取值，不是 [`Running`](Self::Running) 上的一个开关**
    /// （`CONTEXT.md` 的《会话》，`p1-session/14`）。跑着与等答话按得动的键是两套：
    /// 跑着时是停（`s`），等答话时是答话那三个（`x` 接着做、`a` 剩下的卷都这样、
    /// `s` 收尾）。摆进同一个取值，屏底那一行就要靠一个 flag 分岔。
    ///
    /// 三层在这一刻**仍然只读**，与跑着时一个待遇：`Request` 在起线程那一刻就是一份快照，
    /// 而这一趟还没收场（见 [`deciding_action`]）。
    ///
    /// 那一格装的还是[闩](Session::stopping)：在决策点上等着的时候，闩记着的是这一趟
    /// **此前**按过的停。答完话回 [`Running`](Self::Running) 时它原样带回去——
    /// 决策点上答的字不是闩，两者互不覆盖。
    Deciding(Instruction),
    /// **收场了**：三层又改得动，而报告一行不少地摆在那儿。
    ///
    /// 与[没跑过](Self::Fresh)差的只有一件事——报告区里有东西了，`⇥` 因此切得过去。
    /// 别的地方两者一个待遇：按得动的键、屏上的样子都一样，而「上一趟怎么样」
    /// 摆在报告与总览块上，不摆在这一维上。
    Ended,
}

impl Stage {
    /// **三层此刻只读吗**（`CONTEXT.md` 的《会话》：一趟跑起来之后三层都只读）。
    ///
    /// 跑着与等答话都是：`Request` 在起线程那一刻就是一份快照，而这一趟还没收场。
    /// **这是「只读」在本仓库唯一的判据**——画法照它压暗（[`super::draw::config`]），
    /// 按键表照它一个改动键都不派（[`Session::action`]）。
    pub fn read_only(self) -> bool {
        matches!(self, Self::Running(_) | Self::Deciding(_))
    }
}

/// **眼下在看什么**——会话两维中的**第二维**，也就是**焦点落在屏上哪一块**
/// （`CONTEXT.md` 的《会话》：焦点）。
///
/// 六个取值里两个是光秃秃的（左栏与报告区，`⇥` 在它们之间切，见 [`Pane`]），
/// 另外四个各带着一份东西：打到一半的缓冲、摊开的那一列、展开的那一卷、预设那一栏。
/// **那四个都从左栏进得去、也各有一个键退得回来**，而它们在这一维上是六选一：
/// 「展开着而左栏还在」这种没人要的组合因此不必靠某处代码守着。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    /// **左栏**：光标在三层上走（[`Session::field`] 说得出停在哪一行）。
    Config,
    /// 某一行正在被打字改。
    Editing(Edit),
    /// **报告区**：卷表有[自己的光标](Session::follow)，`↑↓` 选一卷
    /// （`p3-session-legibility/10`）。
    ///
    /// **跑着与等答话时一样进得来**：几十分钟的一趟里想回头看第一卷，不必等它跑完。
    /// 那两个阶段按得动的键在这里照旧按得动（按停、答话那三个），
    /// 而三层照旧一个改动键都不派——那一条归[阶段](Stage)那一维。
    ///
    /// 光标停在哪一卷**不装在这一格里**（在 [`Session::follow`] 上）：
    /// `⇥` 切回左栏再切回来时它还在原处——切焦点不是「重新开始读」。
    Report,
    /// 报告区**展开着一个目录**：那一枝底下那几卷摊成卷表，左栏**还在场**
    /// （`volume-discovery/08`）。
    ///
    /// **两级展开的头一级**：默认那一副是[目录表](Self::Report)——一个目录一行，
    /// 一屏看得完；这一级摊出那一枝底下那几卷；[第二级](Self::Expanded)才是逐页。
    ///
    /// **这一级左栏不收起**：收起与展开**到页**才是同一件事（见 [`Self::Expanded`]）
    /// ——卷表那几列摆得下，而左栏在这一刻仍旧答着「这一趟照哪几项跑的」。
    ///
    /// 那一格装的是**哪一枝**（目录路径）。光标停在哪一卷**不装在这里**
    /// （在 [`Session::follow`] 上，与[目录表](Self::Report)共用同一个光标）：
    /// 屏上那个光标恒是**一卷**，目录表反白的是它所在的那一枝。
    Opened(PathBuf),
    /// 报告区**展开**着一卷的逐页，左栏收着（`CONTEXT.md` 的《会话》：展开）。
    ///
    /// 「展开」与「左栏收起」是**同一件事**，不是两个开关：spec 的《会话：布局与交互》
    /// 写的是「展开逐页时左栏收起、主区吃满宽度」——逐页那两行轻松过 100 列，
    /// 而左栏那 52 列在这一刻是宽度里最贵的一截。
    ///
    /// **收起不是删掉**：收起来的那些行原样回得来（[`Action::Collapse`] 只把焦点
    /// 换回[报告区](Self::Report)，三层一格没动，光标也还停在原处）。
    ///
    /// **跑着的时候也展得开**（`p3-session-legibility/10` 推翻了停车场 Q72）：
    /// 报告那时还在长，而长出来的那几卷正是这一格要给人看的东西。
    Expanded(Expansion),
    /// **预设那一栏**开着：盘上有的那几份摆成一列，末尾一行是「存成一份新的」。
    ///
    /// 与[展开](Self::Expanded)同一个形状：一个从左栏进得去、一个键退得回来的状态，
    /// 三层一格没动（**套用**才动，而那是用户在这一栏上按下去的那一下）。
    ///
    /// **跑着的时候开不了**：套用一份预设就是把两层整个换掉，而跑起来之后三层只读
    /// （`CONTEXT.md` 的《会话》）。这与 `e` 跑着时按不动是同一条。
    Picking(Picker),
    /// **取值栏**摊着：左栏上那一行的取值就地摊成一列（`CONTEXT.md` 的《会话》：取值栏）。
    ///
    /// 与[预设栏](Self::Picking)是**两个状态、两个词**：这一个就地摊在左栏那一行
    /// **下面**，左栏其余各行还在场（改一个值时看得见它在整份配置里的位置）；
    /// 那一个占主区。两者摆在同一维上是因为它们是同一种东西——一个从左栏进得去、
    /// 一个键退得回来、退回来时三层一格没动的状态。
    ///
    /// **跑着的时候摊不开**：一趟跑起来之后三层都只读（`CONTEXT.md` 的《会话》），
    /// 而摊开这一列正是为了改它。这与 `p`／`e` 跑着时按不动是同一条——
    /// 靠的也是同一件事：那个阶段的按键表一个改动键都不派（见 [`running_action`]）。
    Valuing(Values),
    /// **一张[覆盖层](Overlay)掀着**，屏上那几块整个让位（`p3-session-legibility/12`）。
    ///
    /// 与另外四个带东西的取值同一个形状：一个键掀开、一个键关掉，关掉时**底下那一块
    /// 原样回来**（[`Covered::under`]）——覆盖层**盖住**一块焦点，不替掉它。
    ///
    /// **进去之后除了按停与答话，别的键一律不派**：那三个键在焦点落在哪一块上都按得动
    /// （ADR 0017 决定第 4 条），这一块因此与[交键的那几块](stage_action)一样，
    /// 把自己不认的键交下去——跑着时掀开一张读物就停不下来，那是
    /// 「按了没反应」的另一种写法。剩下的一个都不派：`↑↓` 读、`Esc` 关、
    /// 另一张那个键换过去，就是这一块自己的全部（见 [`overlay_action`]）。
    Overlaid(Covered),
}

/// **覆盖层**：一个键掀开、盖住屏上那几块的那一张（`CONTEXT.md` 的《会话》：覆盖层）。
///
/// **两张是同一副形状，不是两套画法**（`p3-session-legibility/12` 票面第四条）：
/// 一张画法在 `super::draw::overlay`，这个枚举说的是**那一份内容是哪一份**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// **全部键**：此刻按得动的那些键，按[焦点](Focus)分组（见 [`Session::key_table`]）。
    ///
    /// 屏底那一行只摆得下最常用的四五个，而键变多之后「有哪些键」在屏上根本不存在
    /// ——用户只能试。这一张把那件事摆上屏。
    Keys,
    /// **这一趟的前提**：profile · 适配方式 · 裁边 · 跨页拆分 · 互锁 · 判据构成与聚合
    /// （措辞出自 [`crate::render::header`]，会话不另编一份）。
    ///
    /// 它们从前摆在卷表**上方**、跟着表滚（`p3-session-legibility/08`）：一趟只说一次，
    /// 在长任务里没人会反复看，而它们占的正是卷表要的行。
    Premises,
}

impl Overlay {
    /// 两张，次序就是屏底那一行上的次序。
    pub const ALL: [Overlay; 2] = [Overlay::Keys, Overlay::Premises];

    /// **掀开它的那个键。唯一出处**——按键表（[`revealing`]）与屏底那一行都读它，
    /// 各写一遍就会有一处对不上。
    ///
    /// 两个都**不在别处派过**——这一条由用例逐块问出来
    /// （`the_key_table_is_asked_of_the_key_table_itself`：六组各问这两个键一遍，
    /// 六组都答[没有意义](Action::Ignored)）。各块此刻占着的字母是
    /// 左栏 `j k d t x e p c q`、报告区 `k j e g`、展开着 `k j a e`、
    /// 预设栏 `k j d p q`、取值栏 `k j q`，阶段那一维 `s x a q`——
    /// `a` 那种撞车（停车场 Q161）因此在这两个键上不存在。
    pub fn key(self) -> char {
        match self {
            Self::Keys => '?',
            Self::Premises => 'i',
        }
    }

    /// 这一张叫什么。**屏底那一行、`?` 那张表与这一格的抬头共用它**
    /// （前两处经 `super::draw::keys::says`），三处不会各叫一个名字。
    pub fn what(self) -> &'static str {
        match self {
            Self::Keys => "全部键",
            Self::Premises => "这一趟的前提",
        }
    }
}

/// 掀着的那一张覆盖层，连同**它盖住的是哪一块**。
///
/// 盖住的那一块**整份带着**（一个 `Box<Focus>`）而不是记一个「回哪儿去」的小枚举：
/// 展开的那一卷、摊着的那一列、预设那一栏各带着一份东西，而关掉覆盖层要的是
/// **原样回去**——记一个枚举就要在关掉的那一刻把那几份东西重新拼一遍。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Covered {
    /// 掀开的是哪一张。
    pub overlay: Overlay,
    /// 盖住的那一块焦点。`Esc` 原样回到它（见 [`Session::cancel`]）。
    under: Box<Focus>,
    /// **从第几行画起**：这一格摆不下时 `↑↓` 挪的就是它。
    ///
    /// **这一处记的是滚动量，不是光标**——而屏上别处一处都不记它
    /// （`CONTEXT.md` 的《视口》：滚动量是算出来的，不是记着的）。差别在于覆盖层是
    /// **读物**：一行上没有第二步可走，一个光标都停不上去，而「视口跟着光标走」
    /// 要先有个光标。[视口](super::viewport::Viewport)那一份照旧用，它收着
    /// 「往下滚到哪儿为止」；越界由画法那一层每帧收一次
    /// （[`Session::clamp_overlay`]，与逐页表那一处同一条）。这一笔记在停车场 Q163。
    from: usize,
}

impl Covered {
    /// 从第几行画起。
    pub fn from(&self) -> usize {
        self.from
    }
}

/// `?` 那张键位表的**头一层分岔**：[焦点](Focus)那一维上的一块，外加「任何时候」那一组。
///
/// **就是[按键表](Session::action)自己的头一层分岔**（ADR 0017 决定第 2 条：先按焦点
/// 分岔，落到哪一块再按阶段分）——票面说的「按焦点分组」正是它，这里不另立一套分法。
///
/// **「任何时候」那一组是[阶段那一维](stage_action)**：按停、答话那三个、退出会话，
/// 加上掀开覆盖层那两个（[`revealing`]）。它们在哪一块上都按得动，因此不进任何一块。
///
/// **`?` 那张表上只有[七组](Self::ALL)**。这个枚举比那七组多三个——[编辑一行](Self::Editing)、
/// [打预设名](Self::Naming)与[覆盖层自己](Self::Overlaid)：**屏底那一行问的是同一张
/// 措辞表**（`super::draw::keys::says`），而措辞随「这是屏上哪一块」而变
/// （同一个 `Esc` 在取值栏上是「一格不改地回去」、编辑到一半是「丢掉」）。
/// 三块不进 `ALL` 各有理由：
///
/// - **打字那两块**上每一个字符都是一个字，一张按键表说不出「a 到 z 各进缓冲」
///   （停车场 Q165）；它们那三五个键全摆在屏底上，一个都没藏起来。
/// - **覆盖层自己**不是焦点那一维上的一块，是盖在一块上面的一层：那张表列的
///   恒是[它盖住的那一块](Session::beneath)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyGroup {
    /// 左栏：三层配置。
    Config,
    /// 取值栏：就地摊开的那一列取值。
    Valuing,
    /// 报告区：目录表。
    Report,
    /// 展开着一枝：卷表。
    Opened,
    /// 展开着一卷：逐页表。
    Expanded,
    /// 预设栏。
    Picking,
    /// **编辑一行**：左栏上打字改那一行。不在 [`Self::ALL`] 上（见本枚举的文档）。
    Editing,
    /// **打预设名**：预设那一栏里打一个新名字。不在 [`Self::ALL`] 上。
    Naming,
    /// **覆盖层自己**：掀着的那一张读物。不在 [`Self::ALL`] 上。
    Overlaid,
    /// 任何时候：阶段那一维派得出的那几个，加上覆盖层那两个。
    Always,
}

impl KeyGroup {
    /// 七组，次序就是 `?` 那张表上的次序：**屏上进得去的次序**——
    /// 左栏起头（一切从它进去），取值栏跟着它，报告区与展开着那两级是一串
    /// （目录表 → 卷表 → 逐页表），预设栏收尾，
    /// 「任何时候」摆在最后（它不是屏上的一块）。
    pub const ALL: [KeyGroup; 7] = [
        KeyGroup::Config,
        KeyGroup::Valuing,
        KeyGroup::Report,
        KeyGroup::Opened,
        KeyGroup::Expanded,
        KeyGroup::Picking,
        KeyGroup::Always,
    ];

    /// **这一组此刻进得去吗**——进不去的整组不列（屏上不摆按不动的键，
    /// 而一张进不去的块的键位表是同一件事）。
    ///
    /// 两条，都由[阶段](Stage)那一维说了算，与 ADR 0017 逐条对得上：
    ///
    /// - **取值栏与预设栏只从左栏进得去，而跑起来之后左栏一个改动键都不派**
    ///   ——它们与跑着、等答话**结构上碰不到面**（决定第 8 条《两维不是笛卡儿积》）。
    /// - **一趟都没跑过时报告区里连一卷都没有**：`⇥` 那时不派动作
    ///   （[`Session::browsing_action`]），展开也无从谈起。
    ///
    /// 左栏与「任何时候」恒在：一切从左栏进去，而阶段那一维在哪一块上都答得出话。
    fn reachable(self, stage: Stage) -> bool {
        match self {
            // 打字那两块与取值栏、预设栏同一条：都从左栏进去，而跑起来之后
            // 左栏一个改动键都不派。（这两组不在 [`Self::ALL`] 上，这张表问不到它们——
            // 答得出话是因为这一条是个判据，不是一份名单。）
            Self::Valuing | Self::Picking | Self::Editing | Self::Naming => !stage.read_only(),
            Self::Report | Self::Opened | Self::Expanded => stage != Stage::Fresh,
            // 覆盖层每一个阶段上都掀得开（`p4-parking-lot/06`）。
            Self::Config | Self::Overlaid | Self::Always => true,
        }
    }

    /// **屏上这一块是哪一组**。屏底那一行照它去问[措辞](super::draw::keys::says)
    /// ——那一行的键出自按键表，而措辞随「这是哪一块」而变。
    ///
    /// 打预设名与预设那一栏分得开：那一栏里打名字是它里面的一步，
    /// 而 `Esc` 在两处退到的不是同一个地方（见 [`naming_action`]）。
    pub fn of(focus: &Focus) -> Self {
        match focus {
            Focus::Config => Self::Config,
            Focus::Valuing(_) => Self::Valuing,
            Focus::Report => Self::Report,
            Focus::Opened(_) => Self::Opened,
            Focus::Expanded(_) => Self::Expanded,
            Focus::Picking(picker) => match picker.naming.is_some() {
                true => Self::Naming,
                false => Self::Picking,
            },
            Focus::Editing(_) => Self::Editing,
            Focus::Overlaid(_) => Self::Overlaid,
        }
    }

    /// 这一组在 `?` 那张表上的抬头。
    pub fn title(self) -> &'static str {
        match self {
            Self::Config => "左栏 · 三层配置",
            Self::Valuing => "取值栏 · 摊开的那一列",
            Self::Report => "报告区 · 目录表",
            Self::Opened => "展开一个目录 · 卷表",
            Self::Expanded => "展开一卷 · 逐页表",
            Self::Picking => "预设栏",
            // 底下三个不在 [`Self::ALL`] 上，`?` 那张表因此印不到它们的抬头；
            // 名字照旧取自屏上那一块自己（见本枚举的文档）。
            Self::Editing => "编辑一行",
            Self::Naming => "打预设名",
            Self::Overlaid => "掀着的那一张",
            Self::Always => "任何时候",
        }
    }
}

/// 焦点**切得过去**的那两块：**左栏**与**报告区**（`CONTEXT.md` 的《会话》：焦点）。
///
/// [`Focus`] 那一维有六个取值，而 `⇥` 只在这两个之间切——另外四个各带着一份东西
/// （缓冲、摊开的那一列、展开的那一卷、预设那一栏），进去要按各自的键，
/// 一个 `⇥` 拼不出它们。**带着去哪一边而不是「切一下」**：
/// 屏底那一行要说出这一下去哪儿（见 [`super::draw::footer`]），而一个 toggle 说不出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// 左栏。
    Config,
    /// 报告区。
    Report,
}

/// **跟随**：卷表上那个光标停在哪儿（`CONTEXT.md` 的《会话》：跟随）。
///
/// **跟随是卷表独有的**：别处的视口仍是「算出来的」（`CONTEXT.md` 的《视口》——
/// 光标在哪儿视口就跟到哪儿，屏上没有一处记着「滚到哪儿了」）。这里记着的**也不是
/// 滚动量，是光标停在哪一卷上**：视口照旧由光标算出来（见 [`super::viewport::Viewport`]），
/// 这一维只回答「光标归谁挪」——跑着时报告一直在长，而**一卷收摊就把光标带走**
/// 与「我正看着第三卷」是两件不能同时成立的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Follow {
    /// **跟着最新的那一卷**：一卷收摊，光标就落到它上面。这是默认那一档，
    /// 跑起来时[重新扳回](Session::run_started)它。
    #[default]
    Latest,
    /// **跟随停了**：光标停在这一卷上，报告再长它也不动。
    ///
    /// 屏上说得出这件事（报告区那一格的抬头，见 `super::draw::report::report_title`），
    /// `g` 扳回[跟随](Self::Latest)——不说的话，「它怎么不跟着最新那一卷了」
    /// 在屏上没有一处答得出。
    Stopped(Volume),
}

/// **取值栏**：左栏上就地摊开的那一列取值（`CONTEXT.md` 的《会话》：取值栏）。
///
/// 环让「**这一项有哪几个取值**」在屏上根本不存在——用户只有把它轮询过一整圈，
/// 才知道自己有哪些选择。这一列把那件事摆上屏。
///
/// 列的是**摊开那一刻**那一行的取值环（见 [`Session::unfold`]），
/// 与 [`Picker::names`]、[`Expansion::volumes`] 是同一种「进来那一刻记下的数」：
/// 摊着的时候没有一个键改得动那一行，它因此不会中途变。
///
/// **它带着「下钻到第几层」**（spec 的《取值栏》）：型号那一行摊开的第一层是**面板**，
/// [下钻](Action::Drill)进去才是那块面板底下的型号。装的不是一个层号，是
/// [下钻进了哪一块面板](Self::panel)——层号答不出「进的是哪一块」，而屏上那一行要印它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Values {
    /// 摊开的是左栏上哪一行。
    field: Field,
    /// **下钻进了哪一块面板**；`None` 是第一层（`CONTEXT.md` 的《会话》：下钻）。
    ///
    /// 只有型号那一行到得了 `Some`（[`Field::drills`]）：第一层摆的是面板，
    /// 这一格记着从哪一块进来的——屏上要印它（那一列此刻列的是型号名，
    /// 不说一句就没有一处答得出「这几个型号是哪块屏的」），
    /// 退一步也要靠它回到那一块上（[`Session::cancel`]）。
    panel: Option<Panel>,
    /// 那一列取值**在屏上的写法**，一格一条（出处是 [`Session::shown`]，
    /// 与那一行自己印的是同一份）。
    ///
    /// **第一格恒是「没说」那一格**（屏上多半印成`默认（…）`）：两层的每一格都有
    /// 一个「没说」的位置，而那正是存成预设时写不写进那份 TOML 的分别
    /// （`CONTEXT.md` 的《会话》：存出去的只有「说了的那几项」）。
    ///
    /// **下钻进去那一层没有这一格**：那一层列的是**一块面板底下的型号**，一格一个型号，
    /// 而「没挑」是型号那一行的取值、不是某一块面板底下的取值——它就摆在上一层的第一格，
    /// 一个 `Esc` 之外（停车场 Q143）。
    cells: Vec<String>,
    /// 光标停在第几格。
    at: usize,
    /// **此刻真正生效的是第几格**——屏上那个记号画在它前面，与光标那一格分得开
    /// （票面第二条）。
    ///
    /// 摊开走环那一路的行取值恒在自己的环上（[`Field::drills`] 是那条前提的守门人），
    /// 那几行上它恒指得出一格来。**型号那两层上它答得出「一格都不是」**：
    /// 型号停在内置表外的一个名字上时（预设里塞进来的一个已删型号）没有它的那一块面板，
    /// 而下钻进一块不含当前型号的面板时，这一层里也没有一格是生效着的
    /// （`p3-session-legibility/06` 票面第七条）。
    chosen: Option<usize>,
}

impl Values {
    /// 摊开的是左栏上哪一行。
    pub fn field(&self) -> Field {
        self.field
    }

    /// 那一列取值在屏上的写法，第一格恒是「没说」那一格。
    pub fn cells(&self) -> &[String] {
        &self.cells
    }

    /// 光标停在第几格。
    pub fn at(&self) -> usize {
        self.at
    }

    /// 此刻真正生效的是第几格，`None` 是「这一层里一格都不是」。
    pub fn chosen(&self) -> Option<usize> {
        self.chosen
    }

    /// **下钻进了哪一块面板**；`None` 是第一层。
    pub fn panel(&self) -> Option<Panel> {
        self.panel
    }

    /// **光标停着的这一格是一块面板吗**——是的话按下去是[下钻](Action::Drill)进去看，
    /// 不是的话按下去是[定](Action::Choose)。
    ///
    /// 只有型号那一行的**第一层**上会是「是」：那一层摆的是面板，而面板不是型号那一行的
    /// 一个取值——定不下来，只能进去看它底下有哪几个型号。**第一格「没挑」不是面板**：
    /// 它是型号那一行真正的一个取值（`CONTEXT.md` 的《会话》：两层的每一格都有一个
    /// 「没说」的位置），定得下来。下钻进去那一层的每一格都是一个型号，同样定得下来。
    ///
    /// **与 [`Field::drills`] 问的不是一件事**：那一条问「**这一行**摊开的是不是两层」，
    /// 这一条问「**这一格**按下去会怎样」。按键表（[`valuing_action`]）与屏底那一行
    /// （`super::draw::footer::valuing_prompt`）问的都是后者：**屏上不摆按不动的键**的
    /// 另一半是「这一格上按下去会怎样，按之前就该读得到」——同一个 `⏎` 在这一格上是
    /// 进去看，在下一格上是定。
    pub fn at_a_panel(&self) -> bool {
        self.field.drills() && self.panel.is_none() && self.at > 0
    }
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
    /// **问过一次「真删掉它吗」的是哪一份**（见 [`Session::ask_before_erasing`]）。
    /// `None` 是没问过。
    ///
    /// 与撞名那一问同一个形状（同一个键按两次，ADR 0013），也多同一条：
    /// **屏底那句话一换，它就作废**（见 [`Session::says`]）——那一问就摆在那句话里，
    /// 而屏底只摆得下一句。挪一行、按一个别的键、以及套用失败时报出的那句错，
    /// 因此都把它清掉。收的是名字而不是一个布尔，因为答的那一下与问的那一下
    /// 要点的是同一份，而这句话由这一格自己说得出来。
    asked: Option<String>,
}

impl Picker {
    pub(super) fn new(names: Vec<String>, file: PathBuf) -> Self {
        Self {
            file,
            names,
            at: 0,
            naming: None,
            asked: None,
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

    /// 「真删掉它吗」这一问已经问过的是哪一份。press 那一层照它分岔：
    /// 问的与眼下停着的是同一份才真删，否则先问一句（见 `super::erase_preset`）。
    pub(super) fn asked(&self) -> Option<&str> {
        self.asked.as_deref()
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

    /// `name` 那一份刚从盘上删掉：名字出清单，那一问跟着作废。
    ///
    /// **光标留在原来那一格上**——删掉一份之后接着往下看的是它下面那一份，
    /// 而不是跳回开头。清单短到光标那一格没了时落在末尾那一行上
    /// （[`rows`](Self::rows) 恒有它，删到最后一份也还在）。
    fn gone(&mut self, name: &str) {
        self.asked = None;
        if let Ok(at) = self
            .names
            .binary_search_by(|listed| listed.as_str().cmp(name))
        {
            self.names.remove(at);
        }
        self.at = self.at.min(self.names.len());
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

/// 展开着的那一卷：是哪一卷、逐页表上那个光标停在第几页上、这一副列的是哪几页。
///
/// **不叫 `Reading`。** 展开是屏上那件事本身（报告区摊开了一卷的逐页、左栏收着），
/// 「在读」是用户的意图——后者会让人以为还有一个「不展开地读」的状态，而那个状态
/// 不存在（见 [`Focus::Expanded`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// 展开进来的是**哪一枝**（目录路径，`volume-discovery/08`）。
    ///
    /// **[收起](Action::Collapse)靠它退回上一级**：两级展开各有一个键回得去，
    /// 而「上一级是哪一枝」只有展开那一刻答得出——本模块读不到那一趟攒下来的报告，
    /// 收起的时候再去数一遍就要多一个出处。
    ///
    /// **[换一卷](Action::Turn)也只在这一枝底下转**：层次与发现出来的那棵树一致，
    /// 一个 `⇥` 不该把人从这一枝甩到另一枝上去。
    pub directory: PathBuf,
    /// 展开的是**哪一卷**（[`Volume`]）。
    ///
    /// **不再是「报告上第几卷」**（`p3-session-legibility/10`）：决策点上那一卷也展得开，
    /// 而它停在攒着的那一份上、不在收摊了的那几卷里（`p2-loose-ends/08`：
    /// 不许摊开上一卷冒充它）。一个下标答不出那一卷，这个取值认得出。
    pub volume: Volume,
    /// **逐页表上那个光标停在这一副列出来的第几页上**（`p3-session-legibility/11`）。
    ///
    /// **不是滚动量。** 逐页那一副从此与卷表同一套[视口](super::viewport::Viewport)：
    /// 光标在哪儿视口就跟到哪儿，屏上没有一处记着「滚到哪儿了」
    /// （`CONTEXT.md` 的《视口》）。这一格因此是**光标**，从前那个 `from` 是滚动量
    /// ——两者差的正是那句话（停车场 Q64 记的缺口由光标这一头补上）。
    ///
    /// **越界不算错**：列的那一副刚换过、或者那一卷刚长出几页时它会越界，
    /// 画法那一层每帧收一次（[`Session::clamp_report`]），视口那一头也就近收
    /// （[`Viewport::new`](super::viewport::Viewport::new)）。
    pub at: usize,
    /// 这一副列的是[要紧的页](Listing::Notable)还是[全部页](Listing::All)。
    ///
    /// **换一卷时跟着走**（[`turned_to`](Self::turned_to)）：它是用户此刻的看法，
    /// 不是这一卷的属性——翻到下一卷就被扳回默认那一档的话，`a` 按下去只管一卷。
    pub listing: Listing,
}

impl Expansion {
    /// 摊开 `volume` 那一卷：**默认只列要紧的页**，光标停在头一页上。
    ///
    /// **落位不在参数里**（从前那一格是「报告从第几行画起」）：这一副只画**这一卷**，
    /// 抬头就钉在它顶上（见 `super::draw::pages`）——「视口对到那一卷的抬头上」
    /// 因此不必再由调用方算一个行号出来。
    ///
    /// **「这一趟有几卷」不记在这里**（从前那一格是进来那一刻的快照）：
    /// 跑着的时候也展得开，而报告那时还在长——记下来的那个数下一卷收摊就不作数了。
    /// 要它的两处（换一卷转一圈、抬头上那个「第几/共几卷」）各自现问 [`Live`]。
    pub(super) fn new(directory: PathBuf, volume: Volume) -> Self {
        Self {
            directory,
            volume,
            at: 0,
            listing: Listing::default(),
        }
    }

    /// **换一卷**：列的是哪几页跟着走，光标回到头一页上。
    ///
    /// 光标不跟着走，是因为「第几页」在两卷之间指的不是同一件事；
    /// 列法跟着走，是因为它指的是**同一件事**（见 [`listing`](Self::listing)）。
    pub(super) fn turned_to(&self, directory: PathBuf, volume: Volume) -> Self {
        Self {
            directory,
            volume,
            at: 0,
            listing: self.listing,
        }
    }

    /// 从 `at` 那一卷往一边挪一格，**两头都转一圈**——与三层那几个取值环同一条
    /// （见 [`around`]）。
    ///
    /// `volumes` 是**此刻**展得开的那几卷（[`Live::volumes`]），不是进来那一刻记下的
    /// 一个数：跑着的时候也展得开，而报告那时还在长。
    ///
    /// **`at` 是解析过的那一卷**（[`Live::nearest`]）：展开着的那一卷可能已经收摊，
    /// 而它那时换了个名字（「攒着的那一份」→「收摊了的第 n 卷」）——不先解析就找不着它。
    /// 真找不着（调用方没解析）时从**头一卷**起，而不是从头一卷再挪一格：
    /// 「不知道此刻在第几格」与「在第零格」是两件事。
    ///
    /// `volumes` 是空的这一步到不了：调用方先挡在前面（见 `super::expand`）。
    pub(super) fn next(volumes: &[Volume], at: Volume, step: Step) -> Volume {
        let Some(at) = volumes.iter().position(|listed| *listed == at) else {
            return volumes[0];
        };
        volumes[around(at, volumes.len(), step)]
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
/// 按 `⇥` 把焦点切到报告区（`↑↓` 选一卷，`⏎` 展开它的逐页，左栏跟着收起）、
/// 按 `p` 开预设那一栏（存下当前两层，或套用一份）、
/// 停在设备层上按 `c` 出标定图、按键退出。
///
/// # 此刻在做什么由**两维**说（ADR 0017）
///
/// [这一趟走到哪个阶段了](Stage)与[眼下在看什么](Focus)——**两维各答各的**：
/// 三层只读、按停与答话那几个键归第一维，`↑↓`／`⏎` 归哪一块管归第二维。
/// 「哪些键在哪个状态下有效」仍旧一处答完（[`Self::action`]），只是那张表按两维查。
/// **试算每走完一卷的第一遍都会停下来等人**（[`Stage::Deciding`]），那时按 `x` 接着做
/// 第二遍、按 `a` 剩下的卷都这样、按 `s` 收尾。
///
/// **出标定图不往这里加状态**：它一按就完，屏底说一句就是全部结果——
/// 会话此刻在做什么一格没变（见 [`Action::Chart`] 与 [`Self::charted`]）。
///
/// **跑起来的那一趟不在这里**：这个结构只记得「此刻在做什么」（[`Stage::Running`]），
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
    /// 左栏的光标停在 [`Session::rows`] 的第几行。
    cursor: usize,
    /// 这一趟走到哪个阶段了（[两维](Stage)之一）。
    stage: Stage,
    /// 眼下在看什么（[两维](Focus)之二）。
    focus: Focus,
    /// **卷表上那个光标**停在哪儿（`CONTEXT.md` 的《会话》：跟随）。
    ///
    /// **不装进 [`Focus::Report`] 里**：`⇥` 切回左栏再切回来时它还在原处——
    /// 切焦点不是「重新开始读」；展开一卷再收起同理。
    follow: Follow,
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
            stage: Stage::Fresh,
            focus: Focus::Config,
            follow: Follow::Latest,
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

    /// **左栏的光标停在哪一行。**
    ///
    /// **不叫 `focus`。** 那个词在两维之后已经有主：[`Focus`] 是「眼下在看什么」
    /// 那一维（左栏／报告区／展开着……），而这一个答的是**左栏里面**光标停在哪一行。
    /// 两者同名会让「焦点在哪」这一问在屏上有两个答案。
    pub fn field(&self) -> Field {
        let rows = self.rows();
        rows[self.cursor.min(rows.len() - 1)]
    }

    /// **这一趟走到哪个阶段了**（两维之一，ADR 0017）。
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// **眼下在看什么**（两维之二，ADR 0017）。
    pub fn focus(&self) -> &Focus {
        &self.focus
    }

    /// **此刻掀着的那一张覆盖层**，没掀就是 `None`。
    ///
    /// 画法那一层照它分岔：掀着的时候屏上那几块整个让位（见 `super::draw::shell`）。
    pub fn overlay(&self) -> Option<&Covered> {
        match &self.focus {
            Focus::Overlaid(covered) => Some(covered),
            Focus::Config
            | Focus::Editing(_)
            | Focus::Report
            | Focus::Opened(_)
            | Focus::Expanded(_)
            | Focus::Picking(_)
            | Focus::Valuing(_) => None,
        }
    }

    /// **卷表上那个光标停在哪儿**：跟随，还是停在某一卷上
    /// （`CONTEXT.md` 的《会话》：跟随）。
    pub fn follow(&self) -> Follow {
        self.follow
    }

    /// 把光标挪到某一行上。**只给用例用**——真会话里光标是一步步走过去的，
    /// 而用例问的是「停在这种行上时按键做什么」，走过去那几步不是它要说的事。
    #[cfg(test)]
    pub fn go_to(&mut self, field: Field) {
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
        self.says(Some(said));
    }

    /// 屏底那一句换成 `said`（`None` 是把上一句抹掉）。**本模块每一句话都从这里出去。**
    ///
    /// 它多做一件事：**把「真删掉它吗」那一问作废**（见 [`Picker::asked`]）。
    /// 那一问就摆在它自己那句话里（「再按一次 `d` 删掉它」），而屏底只摆得下一句——
    /// 换一句话说出口，读的人就再也看不见那一问了，闩因此不能比它活得长。
    /// 一处出口而不是各处各清一次：漏掉一处，那一下按 `d` 就成了不问自删。
    /// [`ask_before_erasing`](Self::ask_before_erasing) 自己那一句也走这里——
    /// 它是先说、后闩。
    fn says(&mut self, said: Option<String>) {
        self.notice = said;
        if let Focus::Picking(picker) = &mut self.focus {
            picker.asked = None;
        }
    }

    /// 一趟跑起来了：进 [`Stage::Running`]，配置从这一刻起只读。
    ///
    /// 闩从[继续](Instruction::Continue)起——**一趟一份**。上一趟按下的停不该跟着漏到
    /// 下一趟去，理由与库那一侧把闩放在 `run` 的栈上是同一条（见 `tonefit` 的
    /// `progress::Events`）。跑着的那一趟那一份见 [`super::run::Running::start`]。
    ///
    /// **[跟随](Follow)跟着扳回去**：新的一趟从第一卷起长，而上一趟停在哪一卷上
    /// 与这一趟没有关系。焦点那一维一格不动——起一趟只在左栏按得动，
    /// 而按完接着看的正是报告长出来。
    pub fn run_started(&mut self) {
        self.says(None);
        self.stage = Stage::Running(Instruction::Continue);
        self.follow = Follow::Latest;
    }

    /// 那一趟收场了：配置又改得动。
    ///
    /// **改的是[阶段](Stage)那一维，焦点一格不动**（ADR 0017）：焦点落在报告区上的
    /// 那个人正在读的东西，不该因为最后一卷跑完而被搬回左栏去。
    ///
    /// **不叫 `Live::run_finished`。** 那一个折的是 `RunFinished` 那条**事件**
    /// （库说「这一趟完了」），这一个改的是**会话**此刻在做什么——两件事，两个接收者。
    pub fn run_finished(&mut self) {
        if self.stage.read_only() {
            self.stage = Stage::Ended;
        }
    }

    /// **切焦点**：左栏 ⇄ 报告区（`CONTEXT.md` 的《会话》：焦点）。
    ///
    /// 两块之外的那四个取值这一下到不了（见 [`Pane`]）：从它们出去要按各自那个键。
    pub(super) fn look_at(&mut self, pane: Pane) {
        self.focus = match pane {
            Pane::Config => Focus::Config,
            Pane::Report => Focus::Report,
        };
    }

    /// **报告区那个光标此刻停在哪一卷上**（`CONTEXT.md` 的《会话》：跟随）。
    ///
    /// [跟随](Follow::Latest)着的时候是**最新的那一卷**——它是算出来的，不是记着的：
    /// 报告长一卷它就跟着走一卷。停了的时候是停着的那一卷，而那一卷若已经不在
    /// （决策点上那一份收摊了）就落到最后一卷上（[`Live::nearest`]）。
    ///
    /// **[展开着一枝](Focus::Opened)时它收在这一枝底下**（`volume-discovery/08`）：
    /// 跟随着的时候最新那一卷随时可能落到**另一枝**上，而屏上摆的是这一枝的卷表——
    /// 不收的话，那一格一行都不反白，而 `⏎` 展开的会是屏上根本没有的那一卷。
    /// 收法是落到**这一枝的末一卷**上：跟随说的是「跟着最新的走」，
    /// 而这一枝里最新的就是它。那一枝一卷都收不住时是 `None`——光标停不上去。
    ///
    /// **收 `&Live`**：这一问要数「此刻有哪几卷」，而那份东西在那一趟上，不在本模块。
    /// 一卷都没有时是 `None`——那时屏上没有一处画得出光标。
    pub fn standing(&self, live: &Live) -> Option<Volume> {
        let at = match self.follow {
            Follow::Latest => live.volumes().last().copied(),
            Follow::Stopped(at) => live.nearest(at),
        }?;
        let Some(directory) = self.opened() else {
            return Some(at);
        };
        // 那一枝不在了（报告换了一趟）就原样给出去：这一格答的是「光标停在哪一卷」，
        // 而「那一副此刻画的是哪一级」由画法那一层就近收（见 `super::draw::report`）。
        match live
            .branches()
            .into_iter()
            .find(|branch| branch.directory == directory)
        {
            Some(branch) if !branch.volumes.contains(&at) => branch.volumes.last().copied(),
            _ => Some(at),
        }
    }

    /// **报告区那个光标挪一格**：往前或往后，**两头都绕回去**（与左栏那一列同一条，
    /// 见 [`around`]）。挪完[跟随就停了](Follow::Stopped)——票面第三条：光标一动就停。
    ///
    /// **挪一格是什么随站在哪一级而变**（`volume-discovery/08`），而光标**恒是一卷**：
    ///
    /// - [目录表](Focus::Report)上挪的是**一枝**：落到相邻那一枝的**头一卷**上。
    ///   一卷都停不住的那一枝跳过——那几卷全没做成，连一份卷报告都没有
    ///   （见 [`Branch::volumes`](super::live::Branch::volumes)）。
    /// - [展开着一枝](Focus::Opened)时挪的是**这一枝底下那几卷**，转的圈也只有这一枝
    ///   ——层次与发现出来的那棵树一致，一个 `↓` 不该把人甩到另一枝上去。
    ///
    /// 收 `&Live` 的理由与 [`standing`](Self::standing) 同一条：挪到哪一卷要数
    /// 此刻有哪几卷。**一卷都没有时一格不动**，屏上那时也不摆这两个键
    /// （见 `super::draw::footer`）。
    pub(super) fn select(&mut self, live: &Live, step: Step) {
        // 上一个动作说的那句话到这里就作废了，与 [`act`](Self::act) 同一条——
        // 这一支不经过它（挪到哪一卷要读那一趟攒下来的报告，见 [`Action::Select`]）。
        self.says(None);
        let branches = live.branches();
        let Some(next) = (match self.opened() {
            Some(directory) => Self::next_volume(&branches, directory, self.standing(live), step),
            None => Self::next_branch(&branches, self.standing(live), step),
        }) else {
            return;
        };
        self.follow = Follow::Stopped(next);
    }

    /// **一枝底下挪一卷**：在这一枝停得住的那几卷上挪一格，两头转一圈。
    ///
    /// 那一枝不在了（报告换了一趟）或者它一卷都停不住时一格不动。
    fn next_volume(
        branches: &[Branch],
        directory: &Path,
        standing: Option<Volume>,
        step: Step,
    ) -> Option<Volume> {
        let inside = &branches
            .iter()
            .find(|branch| branch.directory == directory)?
            .volumes;
        let last = inside.len().checked_sub(1)?;
        let at = standing
            .and_then(|at| inside.iter().position(|listed| *listed == at))
            .unwrap_or(last);
        inside.get(around(at, inside.len(), step)).copied()
    }

    /// **挪一枝**：落到相邻那一枝的头一卷上，两头转一圈。
    ///
    /// **只在停得住的那几枝上转**：一卷都收不住的那一枝在屏上照旧占一行，
    /// 光标却停不上去——与卷表上没做成那几行同一条规矩。
    ///
    /// **只有一枝时一格不动，跟随也不停**（点名一个目录跑就是这一档）：转一圈回到
    /// 原地，屏上分毫不变，而把跟随停掉是**看不见的后果**——从此新卷收摊光标不再跟着走。
    /// 「按了没反应」比「按了偷偷改了个状态」好（`CONTEXT.md` 的《跟随》：
    /// 光标一挪跟随就停了——这一下压根没挪）。
    fn next_branch(branches: &[Branch], standing: Option<Volume>, step: Step) -> Option<Volume> {
        let standable: Vec<&Branch> = branches
            .iter()
            .filter(|branch| !branch.volumes.is_empty())
            .collect();
        let last = standable.len().checked_sub(1)?;
        if last == 0 {
            return None;
        }
        let at = standing
            .and_then(|at| {
                standable
                    .iter()
                    .position(|branch| branch.volumes.contains(&at))
            })
            .unwrap_or(last);
        standable
            .get(around(at, standable.len(), step))
            .and_then(|branch| branch.volumes.first().copied())
    }

    /// **展开一枝**：那一枝底下那几卷摊成卷表，左栏还在场
    /// （`volume-discovery/08` 票面第二条）。
    ///
    /// 展开的是哪一枝由 [`super::press`] 数出来（要读那一趟攒下来的报告），
    /// 与[展开一卷](Self::expand)同一条分法。
    pub(super) fn open(&mut self, directory: PathBuf) {
        self.says(None);
        self.focus = Focus::Opened(directory);
    }

    /// **回到跟随**：光标交回给最新的那一卷（`g`，票面第三条）。
    ///
    /// 已经在跟随时按它一格不变——它不是一个开关：跟随停了是**光标挪出去**的后果，
    /// 而不是一个按得回去的状态（票面：`g` 回到跟随，不是 `g` 切换跟随）。
    pub(super) fn follow_along(&mut self) {
        self.follow = Follow::Latest;
    }

    /// **那一趟到决策点了没有**：在[跑着](Stage::Running)与[等答话](Stage::Deciding)
    /// 之间转（`p1-session/14`）。
    ///
    /// 会话每帧问一次，与 `reap` 同一条（见 `super::drive`）：停在决策点上的是
    /// **计算线程**，而本模块碰不到线程——那一层问得到（`super::run::Running::deciding`），
    /// 把答案交进来。
    ///
    /// 别的状态一格不动：这一问只在这两者之间转场。答完话那一下不必等下一帧
    /// ——[`Action::Answer`] 当场就把状态放回去（见 [`Self::answered`]）。
    pub fn at_the_decision_point(&mut self, waiting: bool) {
        self.stage = match (self.stage, waiting) {
            (Stage::Running(pressed), true) => Stage::Deciding(pressed),
            (Stage::Deciding(pressed), false) => Stage::Running(pressed),
            _ => return,
        };
    }

    /// 决策点上答完话了：回[跑着](Stage::Running)那一副，闩原样带回去。
    ///
    /// **当场就转，不等下一帧**：那条线程收到那个字就接着跑，而屏底那两行要跟着换——
    /// 慢一帧的话，答完之后那两个答话键还在屏上摆着，按下去却已经没有人收了。
    fn answered(&mut self) {
        if let Stage::Deciding(pressed) = self.stage {
            self.stage = Stage::Running(pressed);
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
        match self.stage {
            Stage::Running(pressed) | Stage::Deciding(pressed) => pressed,
            Stage::Fresh | Stage::Ended => Instruction::Continue,
        }
    }

    /// **此刻停在决策点上等人拿主意吗**（`CONTEXT.md` 的《会话》：决策点）。
    ///
    /// 屏上那几处照它写：总览块那一格的抬头、屏底那两行（见 `super::draw`）。
    pub fn deciding(&self) -> bool {
        matches!(self.stage, Stage::Deciding(_))
    }

    /// 报告区此刻展开着哪一卷、滚到哪儿了。没展开就是 `None`——那是默认的那一档：
    /// **报告区只给卷级**，左栏在场（票面第一条）。
    pub fn expansion(&self) -> Option<&Expansion> {
        match &self.focus {
            Focus::Expanded(expansion) => Some(expansion),
            Focus::Config
            | Focus::Editing(_)
            | Focus::Report
            | Focus::Opened(_)
            | Focus::Picking(_)
            | Focus::Valuing(_)
            | Focus::Overlaid(_) => None,
        }
    }

    /// 报告区此刻**展开着哪一枝**（目录路径）。没展开就是 `None`——那是默认那一档：
    /// 报告区摆的是**目录表**，一个目录一行（`volume-discovery/08` 票面第一条）。
    pub fn opened(&self) -> Option<&Path> {
        match &self.focus {
            Focus::Opened(directory) => Some(directory),
            Focus::Config
            | Focus::Editing(_)
            | Focus::Report
            | Focus::Expanded(_)
            | Focus::Picking(_)
            | Focus::Valuing(_)
            | Focus::Overlaid(_) => None,
        }
    }

    /// 预设那一栏此刻的样子。没开着就是 `None`。
    pub fn picking(&self) -> Option<&Picker> {
        match &self.focus {
            Focus::Picking(picker) => Some(picker),
            Focus::Config
            | Focus::Editing(_)
            | Focus::Report
            | Focus::Opened(_)
            | Focus::Expanded(_)
            | Focus::Valuing(_)
            | Focus::Overlaid(_) => None,
        }
    }

    /// **取值栏此刻摊着的那一列**。没摊开就是 `None`——那是默认的那一档：
    /// 左栏一行是一行，取值只印当前那一个（票面要修的那个毛病）。
    ///
    /// 与 [`picking`](Self::picking) 分开两个读法，而不是合成一个「眼下摊着什么」：
    /// **两个状态、两个词**（`CONTEXT.md` 的《会话》：取值栏与预设栏），
    /// 而画它们的是屏上两块各不相干的地方（左栏与主区）。
    pub fn valuing(&self) -> Option<&Values> {
        match &self.focus {
            Focus::Valuing(values) => Some(values),
            Focus::Config
            | Focus::Editing(_)
            | Focus::Report
            | Focus::Opened(_)
            | Focus::Expanded(_)
            | Focus::Picking(_)
            | Focus::Overlaid(_) => None,
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
        self.says(Some(crate::render::calibration_notice(out)));
    }

    /// 这个键在当前状态下做什么。**「哪些键在哪个状态下有效」这张表就是它。**
    ///
    /// 纯函数：不改任何东西，用例问得动它，也因此不必去数「按下去之后屏幕变成什么样」。
    ///
    /// # 两维之后它仍是一处（ADR 0017）
    ///
    /// **先按[焦点](Focus)分岔，落到哪一支之后再按[阶段](Stage)分**——
    /// 那一维决定「这个键归屏上哪一块管」，这一维决定「这一刻它派不派得出来」。
    /// 三支根本用不到阶段（[取值栏](Focus::Valuing)、[编辑一行](Focus::Editing)、
    /// [预设栏](Focus::Picking)）：跑起来之后三层只读，那三个状态从浏览才进得去，
    /// 而浏览时按不动的正是起一趟——**两维不是笛卡儿积**，理由与代价在 ADR 0017。
    ///
    /// 剩下几支各按阶段分：左栏（[`Self::config_action`]）、[目录表](report_action)、
    /// [卷表](opened_action)、[逐页表](expanded_action)，加上[覆盖层](overlay_action)。
    /// **三层只读由阶段那一维说了算**，焦点落到报告区上不解锁任何一个改动键；
    /// **按停与答话那三个键反过来在这几块上都按得动**（ADR 0017 决定第 4 条）。
    pub fn action(&self, key: Key) -> Action {
        self.action_in(&self.focus, key)
    }

    /// 焦点落在 `focus` 上时这个键做什么。**[`action`](Self::action) 就是它**，
    /// 只是那一条问的恒是眼下这一块。
    ///
    /// 分出来是给 [`key_table`](Self::key_table) 用的：`?` 那张表要列**每一块**上
    /// 派得出的键，而它一次只站在一块上。**这张表因此仍旧只有一份**——
    /// 覆盖层那一张是问出来的，不是抄出来的。
    fn action_in(&self, focus: &Focus, key: Key) -> Action {
        // **掀开覆盖层那两个键归这一处**，与按停、答话那几个归 [`stage_action`] 一处
        // 是同一个形状（ADR 0017 决定第 2 条）：它们与眼下在看什么无关，
        // 摆进六块里就是六份措辞、六处要跟着改。
        //
        // 三块除外，各有理由，都在 [`minds_this_key_itself`] 上。
        if !minds_this_key_itself(focus, key)
            && let Some(action) = revealing(key, self.stage)
        {
            return action;
        }
        match focus {
            Focus::Config => self.config_action(key),
            Focus::Editing(edit) => editing_action(edit, key),
            Focus::Report => report_action(key, self.stage),
            Focus::Opened(_) => opened_action(key, self.stage),
            Focus::Expanded(expansion) => expanded_action(expansion, key, self.stage),
            Focus::Picking(picker) => picking_action(picker, key),
            Focus::Valuing(values) => valuing_action(values, key),
            Focus::Overlaid(covered) => overlay_action(covered, key, self.stage),
        }
    }

    /// **`?` 那张键位表**：按[焦点](KeyGroup)分组，每一组只列**此刻这个阶段**派得出的键
    /// （票面第二条）。
    ///
    /// **一个键都不在这里另列一份**：它把[每一个键](every_key)交给那一块的按键表
    /// （[`action_in`](Self::action_in)）问一遍，问出[没有意义](Action::Ignored)的丢掉。
    /// 改一处按键表，这张表当场跟着变——「哪些键在哪个状态下有效」因此仍旧一处答完
    /// （票面第五条）。
    ///
    /// **阶段那一维那几个键只列在「任何时候」那一组里**：那几个在哪一块上都按得动，
    /// 而屏上同一个键说两遍就是两份措辞（见 [`keys_of`](Self::keys_of) 那一道滤）。
    ///
    /// 一个键都不剩的那一组整组不出（没跑过与收场了这两个阶段上「任何时候」照旧有
    /// 退出与覆盖层那几个，因此不会整张空掉）。
    pub fn key_table(&self) -> Vec<(KeyGroup, Vec<(Key, Action)>)> {
        KeyGroup::ALL
            .into_iter()
            .filter(|group| group.reachable(self.stage))
            .map(|group| (group, self.keys_of(group)))
            .filter(|(_, keys)| !keys.is_empty())
            .collect()
    }

    /// 这一组上派得出动作的那几个键，连同它们各派什么。
    ///
    /// **三道滤**：
    ///
    /// - [没有意义的](Action::Ignored)不列——屏上不摆按不动的键；
    /// - **[「到处都是同一件事」的键](Self::means_the_same_everywhere)只在「任何时候」
    ///   那一组里列一遍**，别处一个都不列（照实列的话 `Ctrl-C 退出会话`
    ///   会在五组里各出现一次）；反过来，**有一块另派的键归各块自己列**，
    ///   「任何时候」那一组不收它；
    /// - **「任何时候」那一组还要过一道[眼下这一块](Self::action)**（见下）。
    ///
    /// 第二条那个反过来非有不可：`Esc` 在左栏与报告区上是退出会话，在展开着上是收起、
    /// 在取值栏与预设栏上是退一步——摆进「任何时候」的话，这张表就在四块上说了假话。
    ///
    /// # 第三道滤的判据：**此刻按下去有没有第二步**
    ///
    /// 不是「这个键存不存在」（`p4-parking-lot/07` 票面第二条）。别的块上那几行说的是
    /// 「**站到那一块上**按它做什么」——这张表本来就在列别的块的键，读它的人不会指望
    /// 每一行此刻都按得动。而**「任何时候」那一组说的正是「此刻」**：它宣称这几个键
    /// 在哪一块上都按得动，因此**在眼下这一块上按不动就不该在表上**。
    ///
    /// 这张表恒是**掀着一张覆盖层**的时候读的，而那一块自己认下几个键
    /// （[`overlay_action`]）：`q` 在它手上一个动作都不派，而底下那一块上它是退出会话
    /// ——照实列的话表上白纸黑字写着 `q 退出会话`，按下去却没反应（停车场 Q189）。
    /// 掀着的那一张自己那个键同理：按回去是**关掉**，不是再掀一张。
    ///
    /// 另一头的两支（一趟都没跑过时的[展开](Action::Expand)与[前提那一张](Overlay::Premises)）
    /// 不在这里滤——它们在按键表自己那一头就不派了（见 [`Session::browsing_action`]
    /// 与 [`revealing`]，停车场 Q167）。
    fn keys_of(&self, group: KeyGroup) -> Vec<(Key, Action)> {
        every_key()
            .into_iter()
            .filter_map(|key| {
                let action = self.acts(group, key);
                if action == Action::Ignored {
                    return None;
                }
                if self.means_the_same_everywhere(key) != (group == KeyGroup::Always) {
                    return None;
                }
                if group == KeyGroup::Always && self.action(key) != action {
                    return None;
                }
                Some((key, action))
            })
            .collect()
    }

    /// **眼下这一块上派得出动作的每一个键**，连同它派的那件事，次序照 [`every_key`]。
    ///
    /// **屏底那一行的键出自这里**（`p4-parking-lot/07` 票面第一条）：那一行从前是各状态
    /// 那几个函数里手写的字面串，同一个键因此在屏底与 `?` 那张表上各有一句措辞，
    /// 改一处漏一处（停车场 Q166）；取值栏那两层上三个键同义，而屏底只摆得出其中两个
    /// （停车场 Q180）。**问出来之后那两笔都不存在**：摆哪几个键由这一处答。
    ///
    /// 与 [`key_table`](Self::key_table) 差的是问的对象：那一张问的是**每一块**
    /// （屏上此刻不在的那几块也列），这一条问的恒是**眼下这一块**——
    /// 屏底那一行答的正是「此刻按什么」。它因此不过[「到处都是同一件事」那一道滤](Self::means_the_same_everywhere)：
    /// 屏底不分组，一个键摆一次就够。
    ///
    /// **摆哪几个仍由屏底那一层挑**（见 `super::draw::footer`）：那一行只摆此刻最常用的
    /// 几个，挑的是**动作**（「就在这一行上动手」「试算」「退出」），键与措辞一律出自这里。
    pub fn keys_here(&self) -> Vec<(Key, Action)> {
        every_key()
            .into_iter()
            .map(|key| (key, self.action(key)))
            .filter(|(_, action)| *action != Action::Ignored)
            .collect()
    }

    /// 这个键**在此刻进得去的每一块上都是同一件事**吗——是的话它归「任何时候」那一组。
    ///
    /// 比的是[阶段那一维](KeyGroup::Always)派出的那件事：某一块**另派一件**就不算
    /// （`Esc`：展开着上是收起）；某一块**一个动作都不派**不算另派（`s` 在取值栏上
    /// 没有意义，而那一块跑着时根本进不去）。阶段那一维自己不派动作时也不算
    /// ——那时这个键归各块自己（`x` 没跑过时是「执行」，只有左栏派得出）。
    ///
    /// **只问[此刻进得去的那几块](KeyGroup::reachable)**：进不去的那几块整组都不列，
    /// 让它们在这里改一个键的归属就是让一块不在屏上的东西说话。
    fn means_the_same_everywhere(&self, key: Key) -> bool {
        let everywhere = self.acts(KeyGroup::Always, key);
        if everywhere == Action::Ignored {
            return false;
        }
        KeyGroup::ALL
            .into_iter()
            .filter(|group| *group != KeyGroup::Always && group.reachable(self.stage))
            .all(|group| {
                let here = self.acts(group, key);
                here == everywhere || here == Action::Ignored
            })
    }

    /// 这一组的按键表怎么问。
    ///
    /// **站在眼下那一块上时问的就是它本人**（展开着的那一卷、摊着的那一列、
    /// 预设栏上停着的那一份）——而「眼下那一块」问的是[覆盖层盖住的那一块](Self::beneath)，
    /// 不是覆盖层自己：这张表本来就是掀着覆盖层的时候在读的，照 `self.focus` 问的话，
    /// 展开着按 `?` 看到的会是「列全部页」而屏上其实正列着全部页。
    ///
    /// 别的几块拿一副**代表的**问——那几块的按键表只有一处分岔看得出这一份是哪一份
    /// （`⏎` 在一块面板上是进去看、在别处是定；`d` 只在停着一份预设时派得出来），
    /// 而代表那一副挑的恒是**键最全**的那一格。
    /// 代表那一份屏上一个字都读不到：这里问它的只有「这个键派不派得出动作」。
    fn acts(&self, group: KeyGroup, key: Key) -> Action {
        match group {
            KeyGroup::Config => self.config_action(key),
            KeyGroup::Valuing => match self.beneath() {
                Focus::Valuing(values) => valuing_action(values, key),
                _ => valuing_action(&a_column_of_values(), key),
            },
            KeyGroup::Report => report_action(key, self.stage),
            KeyGroup::Opened => opened_action(key, self.stage),
            KeyGroup::Expanded => match self.beneath() {
                Focus::Expanded(expansion) => expanded_action(expansion, key, self.stage),
                _ => expanded_action(
                    &Expansion::new(PathBuf::new(), Volume::Settled(0)),
                    key,
                    self.stage,
                ),
            },
            KeyGroup::Picking => match self.beneath() {
                Focus::Picking(picker) => listing_action(picker, key),
                _ => listing_action(&a_preset_standing_under_the_cursor(), key),
            },
            KeyGroup::Always => {
                revealing(key, self.stage).unwrap_or_else(|| stage_action(key, self.stage))
            }
            // 这三组不在 [`KeyGroup::ALL`] 上，`?` 那张表问不到它们（见 [`KeyGroup`]）。
            // 屏底那一行要它们的只有**措辞**，键出自 [`Session::keys_here`]，不经过这里。
            KeyGroup::Editing | KeyGroup::Naming | KeyGroup::Overlaid => Action::Ignored,
        }
    }

    /// **覆盖层底下那一块焦点**；没掀着覆盖层就是眼下这一块本身。
    ///
    /// 只给 [`acts`](Self::acts) 用：`?` 那张表是**掀着的时候**在读的，而它要说的是
    /// 「刚才那一块上按得动什么」——问 `self.focus` 的话，六组里有三组永远问到的是
    /// 覆盖层，一份代表的都用不上眼下那份真东西。
    fn beneath(&self) -> &Focus {
        match &self.focus {
            Focus::Overlaid(covered) => &covered.under,
            other => other,
        }
    }

    /// **焦点落在左栏时的按键表**，按[阶段](Stage)分三副。
    ///
    /// 跑着与等答话时一个改动键都不派（三层只读，`CONTEXT.md` 的《会话》），
    /// 那两副各自的理由在 [`running_action`] 与 [`deciding_action`] 上。
    fn config_action(&self, key: Key) -> Action {
        if !self.stage.read_only() {
            return self.browsing_action(key);
        }
        match key {
            // **`⇥` 跑着与等答话时照样切得过去**（票面：跑着的时候一样能用）——
            // 它不改三层里的任何一格，而几十分钟的一趟里回头看第一卷正是这一下。
            Key::Tab => Action::Focus(Pane::Report),
            // 剩下的全交给阶段那一维：那一维一个改动键都没有，
            // 「跑着与等答话时左栏一个改动键都不派」因此是结构上成立的。
            other => stage_action(other, self.stage),
        }
    }

    /// 浏览时的按键表。左右键与回车做什么，随光标停的那一行的[形状](Shape)而变。
    fn browsing_action(&self, key: Key) -> Action {
        let shape = self.field().shape();
        match key {
            Key::Up | Key::Char('k') => Action::Move(Step::Back),
            Key::Down | Key::Char('j') => Action::Move(Step::Next),
            Key::Left => cycle_or(shape, Step::Back, Action::Ignored),
            Key::Right => cycle_or(shape, Step::Next, Action::Ignored),
            // 浏览时空格与回车**同义**：两个都是「就在这一行上动手」，做什么随行状分派。
            //
            // 转得动的行上「动手」是**摊开取值栏**（`p3-session-legibility/05` 票面
            // 第一条），不再是就地转一格：转一格的那一副仍旧在（`←→`，见上面两支），
            // 而这一下答的是「这一项有哪几个取值」——环答不出那个问题。
            //
            // **转得动的行一律摊得开**，型号那一行也在内：它摊开的是面板、`→` 再下钻
            // 一层（[`Field::drills`]），而那仍是同一个「摊开」——分岔在摊开**之后**，
            // 不在这个键上。
            Key::Space | Key::Enter => match shape {
                Shape::Cycle => Action::Unfold,
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
            // 而左栏此刻只是让位的那一方。**一趟都没跑过时它不派**（停车场 Q167）：
            // 那时报告上一卷都没有，按下去从前只换来一句「还没跑过」——
            // 而「按了有话说」在 `?` 那张表上与「按得动」长得一模一样。
            // 与 `⇥` 在这一块上的待遇同一条（见下）。
            Key::Char('e') if self.stage == Stage::Ended => Action::Expand,
            // 预设那一栏。同样与光标停在哪一行无关：存的是**整两层**，不是这一行。
            Key::Char('p') => Action::Pick,
            // 出标定图。**这是唯一一个认层的键**：它出的那个数是设备层唯一填不出来的
            // 一格，停在口味层或范围层上按它没有意义（见 [`Action::Chart`]）。
            // 「按当前 profile 出图」在那两层上也说得通，但那时屏上摆的是别的事——
            // 一个键在它够不着的地方仍旧有效，等于把这三行与那张图的关系抹掉了。
            Key::Char('c') if self.field().layer() == Layer::Device => Action::Chart,
            // **`⇥` 把焦点切到报告区**（ADR 0017）。**一趟都没跑过时它不派**：
            // 那时报告区里连一卷都没有，切过去无事可做，而屏上不摆按不动的键。
            // 上一趟收场之后它照旧按得动——报告一行不少地摆在那儿。
            Key::Tab if self.stage == Stage::Ended => Action::Focus(Pane::Report),
            Key::Char('q') | Key::Esc | Key::Interrupt => Action::Quit,
            // [不进缓冲那一个](Key::F1)到不了这里：它在 [`Session::action_in`] 那一头
            // 就交给覆盖层了。这张表照旧列全——`Ignored` 是一个取值，不是遗漏。
            Key::Char(_) | Key::Tab | Key::BackTab | Key::Backspace | Key::F1 => Action::Ignored,
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
        // 上一个动作说的那句话到这里就作废了：下一次按键就抹掉（连同它里面那一问，
        // 见 [`says`](Self::says)）。
        self.says(None);
        self.apply(action)
    }

    fn apply(&mut self, action: Action) -> Exit {
        match action {
            Action::Move(step) => self.move_cursor(step),
            Action::Cycle(step) => self.cycle(step),
            // 摊开与定都只碰本模块自己的东西（那一行的取值环就在这里），
            // 因此不必像展开与预设那几支那样落到 [`super::press`] 去。
            Action::Unfold => self.unfold(),
            Action::Drill => self.drill(),
            Action::Choose => self.choose(),
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
            Action::Answer(..) => self.answered(),
            // 展开、换卷与在卷表上挪一卷都要读那一趟攒下来的报告（有哪几卷、
            // 那一卷从第几行起），而本模块读不到它——真做这三件事的是 [`super::press`]，
            // 它随后调 [`expand`](Self::expand) 与 [`select`](Self::select)。
            // 与[起一趟](Action::Start)同一条分法，因此这里到不了；
            // 真到了也只是这一下没挪、没展开，不是错。
            Action::Open | Action::Expand | Action::Turn(_) | Action::Select(_) => {}
            // 切焦点与回到跟随两支反过来：一格报告都不必读，因此就在本模块做掉。
            Action::Focus(pane) => self.look_at(pane),
            Action::Follow => self.follow_along(),
            // 预设那四支都要碰盘（列出来、读一份、写一份、删一份），而本模块碰不到盘：
            // 真做这四件事的是 [`super::press`]，它随后调 [`pick`](Self::pick)、
            // [`took`](Self::took)、[`saved`](Self::saved)、[`erased`](Self::erased) 那几个。
            // 与上面两支同一条分法，因此这里到不了——真到了也只是这一下没动，不是错。
            Action::Pick | Action::Take | Action::Store | Action::Erase => {}
            // 出标定图同样要碰盘（真画图与落盘的是 `tonefit::write_calibration_chart`），
            // 走的是 [`super::press`]，它随后调 [`charted`](Self::charted)。
            // 与上面那三支同一条分法，因此这里到不了——真到了也只是这一下没出图，不是错。
            Action::Chart => {}
            // **收起退的恒是一级**（`volume-discovery/08` 票面第二条）：展开着一卷时
            // 回到它那一枝的卷表，展开着一枝时回到目录表。回的都不是左栏——
            // 展开是从报告区那一块进去的，而左栏一个 `⇥` 之外。
            Action::Collapse => {
                self.focus = match &self.focus {
                    Focus::Expanded(expansion) => Focus::Opened(expansion.directory.clone()),
                    _ => Focus::Report,
                };
            }
            // 掀开一张覆盖层就在本模块做掉：键位表那一张要的东西本模块全有
            // （[`key_table`](Self::key_table) 问的就是这张按键表自己）。
            // [这一趟的前提](Overlay::Premises)那一张先由 [`super::press`] 挡一道
            // （还没跑过时它一个字都印不出来），挡过之后仍旧走这里。
            Action::Reveal(overlay) => self.reveal(overlay),
            Action::List(listing) => self.list(listing),
            Action::Quit => return Exit::Leave,
            Action::Ignored => {}
        }
        Exit::Stay
    }

    /// 展开一卷的逐页，左栏跟着收起。
    ///
    /// 那一份 [`Expansion`] 由 [`super::press`] 拼好送进来：**展开哪一卷**要数
    /// 那一趟此刻有哪几卷，而本模块读不到它。列的是哪几页、光标停在第几页
    /// 都是本模块自己的事（[`Expansion::new`] 与 [`Expansion::turned_to`]）。
    pub(super) fn expand(&mut self, expansion: Expansion) {
        // 上一个动作说的那句话到这里就作废了，与 [`act`](Self::act) 同一条。
        self.says(None);
        self.focus = Focus::Expanded(expansion);
    }

    /// **掀开一张[覆盖层](Overlay)**：眼下那一块焦点整份[盖在底下](Covered::under)，
    /// `Esc` 原样回到它。
    ///
    /// **掀着一张时按另一张那个键是换过去，不叠第二层**：盖住的仍是原来那一块焦点。
    /// 叠起来的话，`Esc` 要按几下才回得到屏上那一块就要靠用户自己数——
    /// 而两张覆盖层是同一副形状，换一张与掀开一张没有分别。
    ///
    /// 从第几行画起每一张各从头起：换一张就是换一份内容，上一份读到哪儿不作数了。
    fn reveal(&mut self, overlay: Overlay) {
        let under = match std::mem::replace(&mut self.focus, Focus::Config) {
            Focus::Overlaid(covered) => covered.under,
            other => Box::new(other),
        };
        self.focus = Focus::Overlaid(Covered {
            overlay,
            under,
            from: 0,
        });
    }

    /// 把覆盖层那一格「从第几行画起」收进这一格真摆得下的那一段里：最多从第 `last` 行起。
    ///
    /// **画法那一层每帧调一次**（见 `super::draw::overlay`），与逐页表那一处同一条
    /// （[`clamp_report`](Self::clamp_report)）：只有它知道这一张此刻折出来几行、
    /// 这一格有多高。不收的话，往下按过了头再往回按，头几下会**按了没反应**。
    pub(super) fn clamp_overlay(&mut self, last: usize) {
        if let Focus::Overlaid(covered) = &mut self.focus {
            covered.from = covered.from.min(last);
        }
    }

    /// 进预设那一栏，列的是 `names`。
    ///
    /// 列什么、从哪一份文件列的，都由 [`super::press`] 从盘上读来（[`Action::Pick`]），
    /// 与[展开](Self::expand)收下一份 [`Expansion`] 是同一条：那一层读得到，本模块读不到。
    pub(super) fn pick(&mut self, names: Vec<String>, file: PathBuf) {
        self.says(None);
        self.focus = Focus::Picking(Picker::new(names, file));
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
        self.focus = Focus::Config;
        self.says(Some(format!(
            "套上了「{name}」：设备层与口味层换成了它，范围层一格没动"
        )));
    }

    /// 存好了：那个名字进这一栏的列表，光标停到它上面。
    ///
    /// **不退出这一栏**：刚存出去的那一份就摆在眼前的列表上，「存成了什么」因此看得见。
    /// 说的那句话里带着**命令行上怎么用它**——存预设是为了下一次不必重配，
    /// 而下一次多半在命令行上（spec 的 story 12）。**写到哪儿了不在这句话里**：
    /// 屏底那一格不折行，一条长路径会被切掉；那份文件的位置摆在这一栏自己身上
    /// （见 [`Picker::file`]），它折得下来。
    pub(super) fn saved(&mut self, name: &str) {
        if let Focus::Picking(picker) = &mut self.focus {
            picker.stored(name);
        }
        self.says(Some(format!(
            "存好了：「{name}」——命令行上 --preset {name} 就是它"
        )));
    }

    /// 那个名字已经有人占着：**说一句，闩上「再按一次就覆盖」**。
    ///
    /// 两下而不是一下，理由与两级停同一条（ADR 0013：中止是「再按一次」）：
    /// 盖掉的可能是别人手写的一份预设，而那一步不可逆。
    /// 这一句要说的因此是**盖掉之后什么没了**——那一份原来的内容。
    /// 后半句说的是**没跟着没的**：那份文件里其余几份预设照旧留着，而这一条无条件成立
    /// （见 `preset::insert`：换掉的只有那一份自己的那几节，而剪不动那种文件退回重排时，
    /// 每一份预设的内容也都还在）。
    ///
    /// **注释不写进这一句**：它在退回重排那一条路上留不下来，而屏上这一句不该说一件
    /// 只在多数文件上成立的事——说出口的每一句都要无条件为真（停车场 Q108）。
    pub(super) fn name_is_taken(&mut self, name: &str) {
        if let Focus::Picking(Picker {
            naming: Some(naming),
            ..
        }) = &mut self.focus
        {
            naming.asked = true;
        }
        self.says(Some(format!(
            "已经有一份「{name}」了：再按一次 ⏎ 覆盖它。\
             它原来的内容换成眼下这两层，撤不回来；那份文件里其余几份预设照旧留着"
        )));
    }

    /// 要删的是这一份：**说一句，闩上「再按一次就删」**。
    ///
    /// 两下而不是一下，与覆盖同一条（ADR 0013：中止是「再按一次」）——
    /// 删的是盘上长期存着的东西，按错一下没有撤销。闩记的是**问的哪一份**
    /// （见 [`Picker::asked`]）：光标挪到别的一行、或者屏底换一句别的话，那一问就作废。
    ///
    /// **先说、后闩**：那一问与说出它的那句话同生共死（见 [`says`](Self::says)），
    /// 而 `says` 恰恰要把上一问清掉——顺序反过来，刚闩上的这一个就被自己清掉了。
    pub(super) fn ask_before_erasing(&mut self, name: &str) {
        self.says(Some(format!(
            "真要删掉「{name}」吗：再按一次 d 删掉它。\
             那一份从预设文件里没了，撤不回来；文件里其余几份预设照旧留着"
        )));
        if let Focus::Picking(picker) = &mut self.focus {
            picker.asked = Some(name.to_owned());
        }
    }

    /// 删掉了：那个名字出这一栏的清单，光标落在它下面那一份上。
    ///
    /// **不退出这一栏**，与[存好了](Self::saved)同一条：删完接着挑下一份的多得是，
    /// 而「删成了什么样」就摆在眼前这份清单上。
    pub(super) fn erased(&mut self, name: &str) {
        if let Focus::Picking(picker) = &mut self.focus {
            picker.gone(name);
        }
        self.says(Some(format!(
            "删掉了：「{name}」——那份文件里其余几份原样留着"
        )));
    }

    /// **换一副列法**：只列要紧的页 ⇄ 列全部页（票面第二条）。
    ///
    /// 光标跟着回到头一页上：两副列的不是同一批页，「第几页」换一副就不再指同一页
    /// （与[换一卷](Expansion::turned_to)同一条）。
    fn list(&mut self, listing: Listing) {
        if let Focus::Expanded(expansion) = &mut self.focus {
            expansion.listing = listing;
            expansion.at = 0;
        }
    }

    /// 把逐页表那个光标收进这一副真列出来的那几页里：最多停在第 `rows` 页。
    ///
    /// **画法那一层每帧调一次**（见 [`super::draw::report::report_pane`]），因为只有它
    /// 知道这一副此刻列着几页（换一副列法、那一卷又长出几页，两处都会变）。不收的话，
    /// 往下翻过了头之后再往回翻，头几下会**按了没反应**——而那正是本仓库反复要躲的那件事
    /// （`p1-session/10` 的「屏上不摆按不动的键」）。
    ///
    /// 只往下收、不往上抬：`0` 恒是合法的落点。**一页都没列出来时收到零**：
    /// 那一格里摆的是一句话，没有一页停得上去（见 `super::draw::pages`）。
    pub(super) fn clamp_report(&mut self, rows: usize) {
        if let Focus::Expanded(expansion) = &mut self.focus {
            expansion.at = expansion.at.min(rows.saturating_sub(1));
        }
    }

    /// 把闩往上升一级：继续 → 收尾 → 中止 → 中止（ADR 0013）。
    ///
    /// **只升不降**是这个函数的形状本身：升到中止之后它就是个不动点，
    /// 而键盘上没有第二个键能往回按——两级停是同一个键按两次（见 [`Action::Stop`]）。
    /// 库那一侧的闩用 `fetch_max` 说同一件事（`tonefit::Instruction` 的序即力度）。
    fn raise_stop(&mut self) {
        if let Stage::Running(pressed) = &mut self.stage {
            *pressed = match *pressed {
                Instruction::Continue => Instruction::Finish,
                Instruction::Finish | Instruction::Abort => Instruction::Abort,
            };
        }
    }

    /// 光标挪一行。**挪的恒是眼前那一列**：预设那一栏开着时挪的是那一栏，
    /// 取值栏摊着时挪的是那一列，展开着时挪的是逐页表上那个光标——左栏在这三种
    /// 状态下要么不在屏上、要么只是那一列的抬头（见 [`expanded_action`]）。
    ///
    /// 挪一行**把「真删掉它吗」那一问作废**（见 [`Picker::asked`]），
    /// 与改一个字把撞名那一问作废是同一条（见 [`edit_mut`](Self::edit_mut)）：
    /// 问的是「删掉这一份吗」，换了一行，那一问就不再作数。
    /// 这一下不必自己去清：按键走的是 [`act`](Self::act)，而它从
    /// [`says`](Self::says) 出去——那里连同屏底那句话一起清掉。
    fn move_cursor(&mut self, step: Step) {
        // 覆盖层是**读物**：这一下挪的是[从第几行画起](Covered::from)，不是一个光标
        // ——那一格上一行没有第二步可走，一个光标都停不上去。
        // **两头不转圈**，与逐页表那一处同一条：往上收在零，往下由画法那一层每帧收一次
        // （[`clamp_overlay`](Self::clamp_overlay)）。
        if let Focus::Overlaid(covered) = &mut self.focus {
            covered.from = match step {
                Step::Back => covered.from.saturating_sub(1),
                Step::Next => covered.from.saturating_add(1),
            };
            return;
        }
        if let Focus::Picking(picker) = &mut self.focus {
            picker.at = around(picker.at, picker.rows(), step);
            return;
        }
        // 取值栏摊着时挪的是**那一列**：左栏那一行此刻只是它的抬头，
        // 而屏上只有一处反白（见 `super::draw::config::config`）。
        // 那一列至少有一格（「没说」那一格恒在），`around` 因此除得动。
        if let Focus::Valuing(values) = &mut self.focus {
            values.at = around(values.at, values.cells.len(), step);
            return;
        }
        // 展开着时挪的是**逐页表上那个光标**：左栏此刻不在屏上
        // （与预设那一栏同一条，见 [`expanded_action`]）。
        //
        // **两头不转圈**，与上面那三处不同：那三处是取值环与短列表，一圈几行；
        // 这一副是一张两百页的长表，从末一页一下转回头一页会让「翻到底了」
        // 在屏上没有落点。往上收在零，往下由画法那一层每帧收一次
        // （[`clamp_report`](Self::clamp_report)：只有它知道这一副此刻列着几页）。
        if let Focus::Expanded(expansion) = &mut self.focus {
            expansion.at = match step {
                Step::Back => expansion.at.saturating_sub(1),
                Step::Next => expansion.at.saturating_add(1),
            };
            return;
        }
        self.cursor = around(self.cursor, self.rows().len(), step);
    }

    /// 缓冲改一个字。**两处缓冲同一条规矩**：改完把上一次问出去的那件事作废——
    /// 编辑一行时那是列出来的候选，打预设名时那是「盖掉同名的那一份吗」这一问
    /// （见 [`Naming::asked`]）。
    fn edit_mut(&mut self, change: impl FnOnce(&mut String)) {
        match &mut self.focus {
            Focus::Editing(edit) => {
                change(&mut edit.buffer);
                edit.candidates.clear();
            }
            Focus::Picking(Picker {
                naming: Some(naming),
                ..
            }) => {
                change(&mut naming.buffer);
                naming.asked = false;
            }
            Focus::Config
            | Focus::Report
            | Focus::Opened(_)
            | Focus::Expanded(_)
            | Focus::Picking(_)
            | Focus::Valuing(_)
            | Focus::Overlaid(_) => {}
        }
    }

    /// 进入编辑，缓冲里先摆着当前的取值——改一个字比重打一遍便宜。
    ///
    /// **预设那一栏上是打一个新名字**：那一行本来就是「存成一份新的」，缓冲从空的起
    /// （与「＋ 再打一个卷进来」同一条——那一行也没有「当前取值」可摆）。
    fn begin_edit(&mut self) {
        if let Focus::Picking(picker) = &mut self.focus {
            picker.naming = Some(Naming::default());
            return;
        }
        let field = self.field();
        let buffer = match field {
            Field::AddVolume => String::new(),
            other => self.typed(other),
        };
        self.focus = Focus::Editing(Edit {
            field,
            buffer,
            candidates: Vec::new(),
        });
    }

    /// 丢掉眼下这一步。**退一步，不是退到底**：打预设名打到一半退回那一栏的列表上，
    /// 再按一次才出这一栏（见 [`naming_action`]）。
    ///
    /// **取值栏上这一下一格不改**（`p3-session-legibility/05` 票面第三条）：
    /// 它只把状态换回浏览，三层一个字节都不碰——「看一眼有哪些值」不该付出改掉它的代价。
    /// 那件事不必靠任何一处代码守着：这个函数根本没有写取值的路子。
    ///
    /// **型号那一行下钻进去之后，这一下退回的是面板那一层**
    /// （`p3-session-legibility/06` 票面第三条），
    /// 再按一次才出这一栏——与打预设名那一条同一个形状。**光标落回进来的那一块面板上**，
    /// 不落回「当前型号的那一块」：退一步该退到刚才站的地方去。
    fn cancel(&mut self) {
        // **覆盖层：原样回到它盖住的那一块**（票面第四条一带）。这一下同样是「退一步」
        // ——掀开覆盖层之前在看的是哪一块，关掉之后就该还在那一块上。
        if let Focus::Overlaid(covered) = &mut self.focus {
            let under = std::mem::replace(&mut *covered.under, Focus::Config);
            self.focus = under;
            return;
        }
        if let Focus::Picking(picker) = &mut self.focus
            && picker.naming.take().is_some()
        {
            return;
        }
        if let Focus::Valuing(values) = &self.focus
            && let Some(panel) = values.panel
        {
            self.focus = Focus::Valuing(self.panels(Some(panel)));
            return;
        }
        self.focus = Focus::Config;
    }

    /// **逐层补全**：只列打到的那一层，不递归、不建索引、不缓存（ADR 0009）。
    ///
    /// 列出来的若干项有共同的前缀就先补到那儿——补到分岔口为止是补全该做的事，
    /// 替用户从几项里挑一项不是。
    fn complete(&mut self) {
        let Focus::Editing(edit) = &mut self.focus else {
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
            self.says(Some("这一层下面没有对得上的东西".to_owned()));
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
        // [不进缓冲那一个](Key::F1)到不了这里：[`Session::action_in`] 那一头先把它接走了
        // ——这一块认下的只有**字母**（见 [`minds_this_key_itself`]）。
        Key::BackTab | Key::Up | Key::Down | Key::Left | Key::Right | Key::F1 => Action::Ignored,
    }
}

/// **掀开覆盖层那两个键**（`?` 与 `i`）。**唯一出处**——两张各自那个字母在
/// [`Overlay::key`] 上，这里只把它翻成一个动作。
///
/// 与[阶段那一维](stage_action)同一个形状：这两个键**与眼下在看什么无关**，
/// 因此不进焦点那六块里的任何一块（见 [`Session::action_in`]）。
fn revealing(key: Key, stage: Stage) -> Option<Action> {
    match opens(key)? {
        // **一趟都没跑过时前提那一张根本不派**（停车场 Q167）：那时它一个字都印不出来，
        // 而屏上不摆按不动的键——判据是「此刻按下去有没有第二步」，不是「这个键存不存在」。
        // 挡它的从前在 `super::press` 那一层（那一句「还没跑过」），而 `?` 那张表问不到
        // 那一层：一趟都没跑过时表上照旧列着 `i`，按下去只换来一句话。
        Overlay::Premises if stage == Stage::Fresh => None,
        overlay => Some(Action::Reveal(overlay)),
    }
}

/// 这个键掀开的是哪一张覆盖层。**哪个键掀哪一张只有这一处**——
/// 两张各自那个字母在 [`Overlay::key`] 上，[不进缓冲那一个](Key::F1)在这里接上。
///
/// 它与 [`revealing`] 分开：这一条问的是「这个键是掀开用的吗」，
/// 那一条问的是「此刻按下去掀不掀得开」——[覆盖层自己那一块](overlay_action)
/// 要的正是前者（掀开它的那个键按回去是关掉它）。
fn opens(key: Key) -> Option<Overlay> {
    match key {
        // 不进缓冲那一个只掀[全部键](Overlay::Keys)那一张：打字那两块上要的就是它
        // （票面第三条）。前提那一张在那两块上没有意义——那两块上摆着的是一个缓冲。
        Key::F1 => Some(Overlay::Keys),
        Key::Char(letter) => Overlay::ALL
            .into_iter()
            .find(|overlay| overlay.key() == letter),
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
        | Key::Interrupt => None,
    }
}

/// 这一块**自己认下这个键**，[掀开覆盖层那几个键](revealing)交不到它手上。三块，各有理由：
///
/// - **编辑一行**与**打预设名**：那儿每一个字符都是一个字，`?` 与 `i` 也是字
///   （见 [`editing_action`] 与 [`naming_action`]）。**认下的只有字母**——
///   [不进缓冲那一个](Key::F1)照旧掀得开，那两块上「看不见的东西等于不存在」
///   因此不再成立（`p4-parking-lot/07` 票面第三条，停车场 Q165）。
/// - **覆盖层自己**：那几个键在它手上是「关掉」与「换一张」（见 [`overlay_action`]），
///   而不是再掀一层。
fn minds_this_key_itself(focus: &Focus, key: Key) -> bool {
    match focus {
        // 覆盖层自己认下**全部三个**：两个字母是「关掉」与「换一张」，
        // 不进缓冲那一个同理——掀开它的那个键按回去就是关掉它。
        Focus::Overlaid(_) => true,
        // 打字那两块只认下**字母**：`?` 与 `i` 在那儿是字，而
        // [不进缓冲那一个](Key::F1)不是——那正是它添进来的理由。
        Focus::Editing(_) => matches!(key, Key::Char(_)),
        Focus::Picking(picker) => picker.naming.is_some() && matches!(key, Key::Char(_)),
        Focus::Config
        | Focus::Report
        | Focus::Opened(_)
        | Focus::Expanded(_)
        | Focus::Valuing(_) => false,
    }
}

/// **一张覆盖层掀着时的按键表**：`↑↓` 读，`Esc` 关，另一张那个键换过去，
/// **剩下的交给[阶段那一维](stage_action)**。
///
/// **除了按停与答话，别的键一律不派**：这一块与[交键的那几块](stage_action)一样
/// （是哪几块由那一处列着，这里不抄第二份），把自己不认的键交给 [`stage_action`]
/// ——那三个键**在焦点落在哪一块上都按得动**（ADR 0017 决定第 4 条），
/// 而一张盖在屏上的读物不该是那一条的例外：跑着的时候掀开一张表就停不下来，
/// 那是「按了没反应」的另一种写法。
///
/// **两级语义一格不变**：`s` 跑着时升的是[闩](Session::stopping)、等答话时答的是
/// 当场那个字，两副都由 [`stage_action`] 一处说了算——这一块一个键都不另派，
/// 也一个都不挡。
///
/// **`Ctrl-C` 照旧是退出会话**：它在每一个状态下都是（见 [`Key::Interrupt`]），
/// 编辑到一半也是，这里不该是例外。**`q` 不派**——这一块上的「退一步」是关掉它，
/// 而 `Esc` 已经说了那件事；`q` 留给底下那一块（关掉之后照旧按得动）。
/// 它因此是这一块**自己认下的一个字母**，不往下交：交下去的话，没跑过与收场了
/// 那两个阶段上它会退掉整个会话，而屏上那一刻摆着的只是一张读物。
///
/// **掀开它的那个键按回去就是关掉它**，与展开那一副的 `e`、预设那一栏的 `p` 同一个形状；
/// **另一张那个键换过去**，不必先关掉这一张——两张是同一副形状，换一张与掀开一张
/// 没有分别（见 [`Session::reveal`]）。
fn overlay_action(covered: &Covered, key: Key, stage: Stage) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Move(Step::Back),
        Key::Down | Key::Char('j') => Action::Move(Step::Next),
        Key::Esc => Action::Cancel,
        // `q` 归这一块自己（见上）：交给阶段那一维的话，没跑过与收场了那两个阶段上
        // 它就是退出会话——而这一块上「退一步」是 `Esc`。
        Key::Char('q') => Action::Ignored,
        // **掀开这一张的那个键按回去就是关掉它**。问的是[哪个键掀哪一张](opens)，
        // 不是「此刻掀不掀得开」——关掉一张摊在屏上的读物与那一张此刻印不印得出来无关。
        _ if opens(key) == Some(covered.overlay) => Action::Cancel,
        // 另一张那个键换过去，剩下的交给阶段那一维：按停（`s`）与答话那三个
        // （`x` `a` `s`）在这一块上因此照样按得动；`Ctrl-C` 同一条路
        // （[`stage_action`] 头一句接的就是它）。
        other => revealing(other, stage).unwrap_or_else(|| stage_action(other, stage)),
    }
}

/// 会话认得的**每一个键**，`?` 那张表逐个问过它们（见 [`Session::keys_of`]）。
///
/// 字母那一批照 a–z 排一遍，而不是手抄一份「用到的那几个」：手抄的那一份漏一个
/// 就是屏上少一个键，而那正是这张表要治的病。非字母的那几个跟在后面
/// （[`Overlay::key`] 里的 `?` 就是这么进来的）。
///
/// 次序就是那张表上一行接一行的次序：方向键、动手那几个、退出那一个，然后是字母，
/// [不进缓冲那一个](Key::F1)收尾——它与 `?` 并成一行（两个键派的是同一件事），
/// 而那一行上先说的该是 `?`：打字那两块之外，掀开覆盖层走的是那个字母。
fn every_key() -> Vec<Key> {
    let mut keys = vec![
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Space,
        Key::Tab,
        Key::BackTab,
        Key::Backspace,
        Key::Esc,
        Key::Interrupt,
    ];
    keys.extend(('a'..='z').map(Key::Char));
    keys.extend(
        Overlay::ALL
            .into_iter()
            .map(Overlay::key)
            .filter(|letter| !letter.is_ascii_lowercase())
            .map(Key::Char),
    );
    keys.push(Key::F1);
    keys
}

/// 取值栏那一组在**没摊开**的时候拿来问的那一副：一列现成的取值，光标停在第一格。
///
/// 只有一处分岔看得出这一份是哪一份（[`Values::at_a_panel`]：`⏎` 在一块面板上是
/// 进去看、在别处是定），而这一副挑的是**别处**那一支——型号那一行的面板那一层
/// 是六块里唯一一处两层的地方，拿它当代表会把另外十行的样子说错。
/// 屏上一个字都读不到它：[`Session::keys_of`] 问它的只有「这个键派不派得出动作」。
fn a_column_of_values() -> Values {
    Values {
        field: Field::Filter,
        panel: None,
        cells: Vec::new(),
        at: 0,
        chosen: None,
    }
}

/// 预设那一组在**那一栏没开着**的时候拿来问的那一副：光标停在一份预设上。
///
/// 停在末尾那一行（「存成一份新的」）上 `d` 不派、`⏎` 是打一个名字，键少两个——
/// 代表那一副因此挑**键最全**的那一格。名字是空的：屏上一个字都读不到它。
fn a_preset_standing_under_the_cursor() -> Picker {
    Picker::new(vec![String::new()], PathBuf::new())
}

/// **阶段那一维派得出的那几个键**：按停、答话那三个、退出会话。**唯一出处。**
///
/// 焦点那一维上摆得下它们的那几块——[左栏](Session::config_action)、
/// [目录表](report_action)、[卷表](opened_action)、[逐页表](expanded_action)，
/// 加上[覆盖层](overlay_action)——都把自己不认的键交到这里。**「三层只读由阶段那一维
/// 说了算」在结构上就是这一条**：那几块一个改动键都不派，而这一处一个改动键都没有，
/// 焦点落在哪儿因此改不动任何一格（ADR 0017）。反过来，**按停与答话那三个键
/// 在那几块上都按得动**（决定第 4 条）——覆盖层是最后补上的一块（`p4-parking-lot/06`）。
///
/// 另外三块（[编辑一行](editing_action)、[取值栏](valuing_action)、
/// [预设栏](picking_action)）不走这里，而这**不是漏了一条**：那三个状态只从左栏进得去，
/// 而跑起来之后左栏一个改动键都不派——它们与跑着、等答话**结构上碰不到面**
/// （ADR 0017 的《两维不是笛卡儿积》）。
///
/// **`Ctrl-C` 在每一个状态下都是退出**，跑到一半也是（见 [`Key::Interrupt`]）；
/// `q`／`Esc` 只在没跑过与收场了这两个阶段上退得出去（停车场 Q63：退出会话走中止，
/// 那一卷等于没做——最容易手滑的两个键不该挂这个后果）。
fn stage_action(key: Key, stage: Stage) -> Action {
    if key == Key::Interrupt {
        return Action::Quit;
    }
    match stage {
        Stage::Running(pressed) => running_action(key, pressed),
        Stage::Deciding(_) => deciding_action(key),
        // 这两个阶段上这一维一个键都不派：没有一趟可停，也没有一问要答。
        // 这张表照旧列全（`Ignored` 是一个取值，不是遗漏），与上面那两副同一条。
        Stage::Fresh | Stage::Ended => match key {
            Key::Char('q') | Key::Esc => Action::Quit,
            Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::Enter
            | Key::Space
            | Key::Tab
            | Key::BackTab
            | Key::Backspace
            | Key::Interrupt
            | Key::Char(_)
            | Key::F1 => Action::Ignored,
        },
    }
}

/// 跑起来之后阶段那一维派得出的键：**一个改动键都不派，只留按停**。
///
/// 「跑起来之后三层只读」（`CONTEXT.md` 的《会话》）因此是结构上成立的，
/// 不是画法上把它们涂灰：改一行的那几个动作在这个阶段根本不存在。
///
/// **按停只有 `s` 一个键，按两次**（ADR 0013：中止是「再按一次」）：
/// 第一次升到收尾，第二次升到中止。升到中止之后它**不再有意义**——闩到了顶，
/// 再按一次没有更强的一级可去，因此派 [`Action::Ignored`] 而不是一个什么都不改的动作。
/// 「按了中止之后退不回收尾」于是不必靠任何一处代码守着：键盘上没有那个键。
///
/// **它在焦点落在哪一块上都按得动**（[`stage_action`]，ADR 0017 决定第 4 条）：
/// 报告区上、展开着的时候、连同掀着一张覆盖层的时候，两级语义一格不变——
/// 按停问的是「这一趟还走不走」，与眼下在看什么无关。
///
/// 退出这一路照旧只有 [`Key::Interrupt`]（在 [`stage_action`] 那一头接的），
/// **`q` 与 `Esc` 跑着时按不动**，而这是 `p1-session/10` 拿的一个主意（停车场 Q63）。
///
/// **展开那个键（`e`）跑着时按得动了**（`p3-session-legibility/10`）：从前它不派
/// （停车场 Q72），理由是「`Mode` 得拆成两维」——本票正是把那一维拆出来的那一张。
fn running_action(key: Key, pressed: Instruction) -> Action {
    match key {
        Key::Char('s') if pressed < Instruction::Abort => Action::Stop,
        // `Ctrl-C` 到不了这里（[`stage_action`] 先接走了它），而这张表仍旧列全：
        // 少列一个键，往后添一个新键码时这里不会红。
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
        | Key::Interrupt
        | Key::Char(_)
        | Key::F1 => Action::Ignored,
    }
}

/// **停在决策点上等人拿主意时，阶段那一维派得出的键**：`x` 接着做第二遍，
/// `a` 剩下的卷都这样，`s` 收尾（`p1-session/14`、`volume-discovery/07`，ADR 0012）。
///
/// 那两个方向各拿一个**已经有主的键**，因为它们在这里做的正是那个键一直在做的事：
/// `x` 是执行——「接着做第二遍」就是把这一趟做完；`s` 是停——「收尾」是它的第一级，
/// 而决策点上答收尾停出来的现场恰好也是「盘上不留半卷」（这一卷一个字节都不写）。
/// 另取两个新键的话，屏上就要多记两个只在这一刻有效的记号，而它们与已有的那两个
/// 说的是同一件事。
///
/// **`a` 是这个阶段自己的键**（`all`：剩下的卷都这样）。它没有一个「一直在做这件事」
/// 的旧主可借——它答的字与 `x` 逐字相同，差别只在[管几卷](Reach)，而那件事别处不存在。
/// 借 `x` 按两下也说不出它：两级停那个形状说的是「再按一次更重一级」，
/// 而这一下不比 `x` 更重，只是更远。
///
/// **`s` 在这里不是两级停。**跑着时 `s` 升的是[闩](Session::stopping)，一次收尾、
/// 再一次中止；这里 `s` 答的是**当场那个字**，答完那条线程就接着走，没有第二次可按
/// （`CONTEXT.md` 的《会话》：决策点不是第三个检查点）。
///
/// **`x` 在这里不是「起一趟」。**浏览时 `x` 起的是新的一趟，这里它答的是眼前这一趟的
/// 那一问——两者都是「把它做出来」，而在这个阶段根本没有第二趟可起：三层此刻只读。
///
/// **三个键在焦点落在哪一块上都按得动**（[`stage_action`]，ADR 0017 决定第 4 条）：
/// 报告区上翻着旧卷的时候、掀着一张覆盖层的时候，这一问照旧答得出——
/// 答话问的是这一卷的第二遍做不做，与眼下在看哪一卷无关。
///
/// **三层仍旧只读**：一个改动键都不派，与 [`running_action`] 同一条。
/// 这一趟还没收场，`Request` 也早在起线程那一刻就是一份快照了。
fn deciding_action(key: Key) -> Action {
    match key {
        // `Ctrl-C` 到不了这里，理由与 [`running_action`] 那一句同。
        Key::Char('x') => Action::Answer(Instruction::Continue, Reach::ThisVolume),
        Key::Char('a') => Action::Answer(Instruction::Continue, Reach::ForTheRest),
        Key::Char('s') => Action::Answer(Instruction::Finish, Reach::ThisVolume),
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
        | Key::Interrupt
        | Key::Char(_)
        | Key::F1 => Action::Ignored,
    }
}

/// **焦点落在报告区时的按键表**：报告区默认那一副是**目录表**——一个目录一行
/// （`volume-discovery/08`）。`↑↓` 选一枝，`⏎` 展开它（摊出那一枝底下那几卷），
/// `e` 直接展开逐页，`g` 回到跟随，`⇥` 切回左栏。
///
/// **`↑↓` 挪的仍旧是那一个光标，而它停的恒是一卷**（`CONTEXT.md` 的《会话》：跟随）：
/// 目录表反白的是**光标所在的那一枝**，这两个键把它挪到相邻那一枝的头一卷上。
/// 一枝底下一卷都停不住时（那几卷全没做成）跳过它——与卷表上没做成那几行停不上去
/// 是同一条规矩。`j`／`k` 跟着 `↑↓`，与别处一个待遇。
///
/// **`⏎`／空格是「就在这一行上动手」**，与左栏、预设栏那两块逐字同一个手势：
/// 这一行是一枝，动手就是[展开它](Action::Open)。**`e` 不是它的同义词**——
/// `e` 恒是「展开这一卷的逐页」（左栏跟着收起），在左栏、目录表、卷表三块上是同一件事，
/// 而 `⏎` 是「往下走一级」。两个键因此各答各的：一个跳到底，一个走一级。
///
/// **`g` 回到跟随**（`CONTEXT.md` 的《会话》：跟随）：光标一动跟随就停了，
/// 而几十分钟的一趟里「跟着最新那一卷」是回得去的默认那一档。取 `g` 是因为
/// 「回到头上／回到最新」在别处的翻页器上也是它，而会话里这个字母还没有主。
///
/// **`←→` 在这里不派动作**：表横着摆不下时**砍列**（`crate::session::columns`），
/// 不横着滚。
///
/// **`t`／`x`／`p`／`c` 归左栏**：起一趟之前要看的是三层，而焦点此刻不在它上面
/// （与[展开着](expanded_action)同一条）。它们一个 `⇥` 之外。
///
/// 剩下的交给[阶段那一维](stage_action)：按停与答话那三个在这一块上照样按得动
/// （票面第五条），而 `q`／`Esc` 照旧只在没跑过与收场了那两个阶段上退得出去
/// ——**这一级已经是报告区的顶上，没有上一级可退**。
fn report_action(key: Key, stage: Stage) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Select(Step::Back),
        Key::Down | Key::Char('j') => Action::Select(Step::Next),
        Key::Enter | Key::Space => Action::Open,
        Key::Char('e') => Action::Expand,
        Key::Char('g') => Action::Follow,
        // `⇧⇥` 也切回去：这一维上只有两块，两个方向到的是同一处。
        Key::Tab | Key::BackTab => Action::Focus(Pane::Config),
        other => stage_action(other, stage),
    }
}

/// **展开着一枝时的按键表**：`↑↓` 选一卷，`⏎`／`e` 展开它的逐页，`g` 回到跟随，
/// `Esc` 收起回目录表，`⇥` 切回左栏（`volume-discovery/08`）。
///
/// **这一块就是从前的报告区**：`p3-session-legibility/10` 立的那几个键一个没动，
/// 只是它此刻列的是**一枝底下**那几卷，而不是整趟那几卷——层次与发现出来的那棵树一致。
///
/// **`Esc` 在这里是收起一级**，不是退出会话：这一级是展开进来的，退一步该退到
/// 刚才那一级去（与[展开着一卷](expanded_action)那一处同一个形状）。
/// 退出会话在这一块上仍旧有 `Ctrl-C`，而 `q` 归[阶段那一维](stage_action)——
/// 它在没跑过与收场了那两个阶段上照旧退得出去。
///
/// **`⏎` 与 `e` 在这一块上是同义的**：这一行是一卷，「往下走一级」与「展开这一卷的
/// 逐页」到的是同一处。目录表那一块上两者才分岔（见 [`report_action`]）。
fn opened_action(key: Key, stage: Stage) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Select(Step::Back),
        Key::Down | Key::Char('j') => Action::Select(Step::Next),
        // 「就在这一卷上动手」——与左栏上 `⏎`／空格 同义（见 [`Session::browsing_action`]）。
        // `e` 也留着：展开那个键在左栏上就是它，切过焦点不该换一个键。
        Key::Enter | Key::Space | Key::Char('e') => Action::Expand,
        Key::Char('g') => Action::Follow,
        Key::Esc => Action::Collapse,
        // `⇧⇥` 也切回去：这一维上只有两块，两个方向到的是同一处。
        Key::Tab | Key::BackTab => Action::Focus(Pane::Config),
        other => stage_action(other, stage),
    }
}

/// 展开之后的按键表：**`↑↓` 选一页，`a` 换一副列法，`⇥` 换一卷，`e`／`Esc` 收起。**
///
/// **`↑↓` 挪的是逐页表上那个光标**，不是左栏那一行——左栏此刻不在屏上
/// （见 [`Focus::Expanded`]），把它们留给一栏看不见的东西才是「按了没反应」。
/// `j`／`k` 跟着 `↑↓`，与别处一个待遇。挪到哪儿就是[视口跟到哪儿](Expansion::at)，
/// 与卷表那一头同一套（`p3-session-legibility/11`：逐页也是一张表，
/// 与卷表同一套视口、砍列与上色）。
///
/// **`←→` 在这里不派动作**：逐页那几行横着摆不下时**砍列**
/// （[`crate::session::columns`]），不横着滚——这一副与卷表从此同一条。
/// 从前它是往两边滚的那一副（那时逐页是一段不折行的散文），横滚那一套连同
/// `Expansion` 上那个横向滚动量一起没了。
///
/// **`a` 切到全部页、再按一次切回来**（票面第二条）。**等答话时它不派**：
/// 那一刻 `a` 是「剩下的卷都这样」，而答话那三个键在焦点落在哪一块上都按得动
/// （ADR 0017 决定第 4 条）——这一支因此先看一眼阶段，把那个字母让出去
/// （停车场 Q161）。屏底那一行跟着不摆它（见 `super::draw::footer`）。
///
/// **换卷用 `⇥` 与 `⇧⇥`**：`⇥` 在左栏与报告区之间是切焦点、在编辑路径时是「下一层」，
/// 三处都是「往下一个去」，这里接着用同一个意思；`⇧⇥` 是它的另一头。
/// 两头都有而不是只往后转一圈，是因为票面要的是**选中一卷**——
/// 几十卷的一趟里往回看一卷不该按二十九下。
///
/// **收起有两个键，而这不是重复**：`e` 是展开那个键的另一半（同一个键按回去），
/// `Esc` 是「退一步」——编辑到一半按它是丢掉缓冲回浏览，这里按它是收起回报告区，
/// 同一个意思。两级停那个 `s` 不给第二个键，因为**中止退不回收尾**；
/// 收起退得回去，因此不必守着只有一个入口。
///
/// **跑着与等答话时也展得开**（`p3-session-legibility/10`，推翻停车场 Q72）：
/// 按停与答话那三个键交给[阶段那一维](stage_action)，它们与这里的选页键
/// 一个都不冲突——`p1-session/11` 记着的那第二重代价（「按停那个键在一屏滚动键里
/// 会被挤没」）因此不成立：`s` 是个字母键，选页走的是方向键。
///
/// **`t` 与 `x` 在这里按不动**：起一趟要先收起——报告区正摊着上一趟的逐页，
/// 而新的一趟会当场把它换掉。收起是一个键的事。
fn expanded_action(expansion: &Expansion, key: Key, stage: Stage) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Move(Step::Back),
        Key::Down | Key::Char('j') => Action::Move(Step::Next),
        // 等答话时这个字母归答话（见上）：交给阶段那一维，它在那儿答的是
        // 「剩下的卷都这样」。
        Key::Char('a') if !matches!(stage, Stage::Deciding(_)) => {
            Action::List(expansion.listing.flipped())
        }
        Key::Tab => Action::Turn(Step::Next),
        Key::BackTab => Action::Turn(Step::Back),
        Key::Char('e') | Key::Esc => Action::Collapse,
        other => stage_action(other, stage),
    }
}

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
///
/// **`d` 是删掉停着的那一份**，与范围层那一栏上它一直在做的事同一个字面
/// （那里是删掉一行卷）。**停在末尾那一行上它按不动**：那一行不是一份预设，
/// 而屏上不摆按不动的键（见 `super::draw::footer::picking_prompt`）。
/// 真删要按两下，那一半在 [`Action::Erase`] 与 press 那一层。
fn listing_action(picker: &Picker, key: Key) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Move(Step::Back),
        Key::Down | Key::Char('j') => Action::Move(Step::Next),
        Key::Enter | Key::Space => match picker.picked() {
            Some(_) => Action::Take,
            None => Action::Edit,
        },
        Key::Char('d') if picker.picked().is_some() => Action::Erase,
        Key::Char('p') | Key::Esc => Action::Cancel,
        Key::Char('q') | Key::Interrupt => Action::Quit,
        Key::Left
        | Key::Right
        | Key::Tab
        | Key::BackTab
        | Key::Backspace
        | Key::Char(_)
        | Key::F1 => Action::Ignored,
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
        // [不进缓冲那一个](Key::F1)到不了这里，理由与 [`editing_action`] 那一句同。
        Key::Tab | Key::BackTab | Key::Up | Key::Down | Key::Left | Key::Right | Key::F1 => {
            Action::Ignored
        }
    }
}

/// **取值栏摊着时的按键表**：`↑↓` 在那一列上挪一格，`⏎` 定，`Esc` 一格不改地回去。
///
/// **上下改的恒是眼前这一列**，与展开那一副、预设那一栏同一条（见 [`expanded_action`]
/// 与 [`listing_action`]）：光标此刻停在这一列上，把 `↑↓` 留给底下那一栏配置
/// 就是「按了没反应」。`j`／`k` 跟着 `↑↓`，与浏览时一个待遇。
///
/// **`←→` 归这一层，不再就地转那一行**（`p3-session-legibility/05` 票面第五条）。
/// 它们走的是这一列的两头：`→` 与 `⏎` 同义——**就在停着的那一格上动手**；
/// `←` 与 `Esc` 同义——一格不改地[退一步](Action::Cancel)。
/// 转一格那一副此刻不在场，而那**不是**把环收走了：退回左栏之后 `←→` 照旧转得动
/// （票面：环那一套保留）。
///
/// **`Esc` 一格不改**是这张表的形状本身：这里派得出的动作里只有
/// [`Action::Choose`] 写取值，[`Action::Cancel`] 一格都不碰
/// （见 [`Session::choose`] 与 [`Session::cancel`]）。「看一眼有哪些值」
/// 因此不必付出改掉它的代价。
///
/// **打字与补全在这里没有意义**：这一列是一份现成的取值，不是一个缓冲。
/// **`⇥` 同理**——没有「下一层」可补；[切焦点](Action::Focus)也不在这里派：
/// 那一下在左栏与报告区之间走（ADR 0017），而这一列是**摊在左栏那一行下面**的一格，
/// 退回左栏是一个 `Esc` 的事。
///
/// `q` 仍是退出会话，与浏览、展开、预设那一栏同一件事：这一列只是在看有哪些值，
/// 没有「按错一下就丢掉什么」那种后果。
fn valuing_action(values: &Values, key: Key) -> Action {
    match key {
        Key::Up | Key::Char('k') => Action::Move(Step::Back),
        Key::Down | Key::Char('j') => Action::Move(Step::Next),
        // 「动手」在这一格上是什么，随这一格而变（[`Values::at_a_panel`]）：面板那一层上
        // 停在一块面板上是**进去看**（面板不是一个取值，定不下来），别处一律是**定**。
        // 与浏览时 `⏎` 随行状分派同一条——一个键的意思由它落在哪一格上说了算。
        Key::Enter | Key::Space | Key::Right => match values.at_a_panel() {
            true => Action::Drill,
            false => Action::Choose,
        },
        Key::Esc | Key::Left => Action::Cancel,
        Key::Char('q') | Key::Interrupt => Action::Quit,
        Key::Char(_) | Key::Tab | Key::BackTab | Key::Backspace | Key::F1 => Action::Ignored,
    }
}

/// **面板那一层里，面板从第几格起排。**
///
/// 第一格是「没挑」（型号那一行真正的一个取值），面板跟在它后面——这条偏移两头都要用：
/// 摊开那一列时「第几块面板在第几格上」（[`Session::panels`]），
/// 下钻时「第几格对着第几块面板」（[`Session::drill`]）。**写成一个常量而不是两处
/// `+1`／`-1`**：一头改了另一头忘了跟着改，光标就会停到隔壁那块屏上。
const PANELS_START_AT: usize = 1;

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
///
/// **出发点不在环上时落回环上**（停车场 Q140）：这个循环因此有两个出口。
/// 型号那一环上**表外的名字不在环上**——`next_device` 对它给「没挑」，
/// 从它出发永远走不回它自己，而只认头一个出口的写法会在那里**转不回来**：
/// 套用一份写着已删型号的预设之后，停在型号那一行上按一下 `←` 就到得了。
/// 走满一圈仍没经过出发点，说明出发点在环外；这时落回的是**往前一步落到的那一格**
/// ——环外那个值两个方向都只能回到环上，而落回哪一格由环自己说了算
/// （型号那一环上正是「没挑」，见 [`next_device`]）。
fn back<T: Clone + PartialEq>(value: T, next: impl Fn(T) -> T) -> T {
    let ahead_of_start = next(value.clone());
    let mut cursor = ahead_of_start.clone();
    loop {
        let ahead = next(cursor.clone());
        if ahead == value {
            return cursor;
        }
        if ahead == ahead_of_start {
            return ahead_of_start;
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
        self.turn_field(self.field(), step);
    }

    /// 把**某一行**的取值往前（或往后）转一格。
    ///
    /// 拆出这一支只为一件事：**取值栏摊开的那一列与定下来的那一下走的就是它**
    /// （见 [`Self::unfold`] 与 [`Self::choose`]）。两条路因此改的是同一格、
    /// 走的是同一条写入路径——分成两份就会有一处忘了跟着改（型号那一下要清掉
    /// 标定出来的两个数，就是这种一处忘了就错的东西）。
    fn turn_field(&mut self, field: Field, step: Step) {
        match field {
            Field::Profile => {
                let current = self.device.profile.clone();
                let turned = turn(current, step, |device: Option<String>| {
                    next_device(device.as_deref())
                });
                self.set_device(turned);
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

    /// 这一行此刻停在「**没说**」那一格上吗（`CONTEXT.md` 的《会话》：
    /// 「存出去的只有『说了的那几项』」）。
    ///
    /// 两层的每一格都有这个位置，而它正是存成预设时**写不写进那份 TOML** 的分别：
    /// 「没说」跟着默认值走，「说了一个恰好等于默认的值」写下来、往后默认改了它也不变。
    ///
    /// 取值栏靠它认路：那一列从这一格起摊、走一圈落回它为止
    /// （见 [`Self::unfold`] 与 [`Self::choose`]）。逐个变体都列出来，
    /// 理由与 [`Field::layer`] 同一条。
    fn unsaid(&self, field: Field) -> bool {
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
            Field::BitDepth => self.taste.bit_depth.is_none(),
            Field::Dither => self.taste.dither.is_none(),
            Field::PerPage => self.taste.per_page.is_none(),
            Field::CacheBudget => self.taste.cache_budget.is_none(),
            Field::IoMode => self.taste.io_mode.is_none(),
            Field::Out => self.scope.out.is_none(),
            // 卷那两行没有「没说」那一格：一条路径加一个勾，打进来了就是打进来了，
            // 而「＋ 再打一个卷进来」根本不是一个取值。
            Field::Volume(_) | Field::AddVolume => false,
        }
    }

    /// **一直往前转到「没说」那一格上**，答走了几步。
    ///
    /// 环上从任何一格出发都到得了它（走完一圈必落回它，见 [`ring`]），
    /// 因此这个循环停得下来。取值栏摊开与定下来都从这一格起算
    /// （见 [`Self::unfold`] 与 [`Self::choose`]），两处共用它——
    /// 写两份就会有一处忘了跟着改，与 [`around`] 同一条规矩。
    fn turn_to_unsaid(&mut self, field: Field) -> usize {
        let mut steps = 0;
        while !self.unsaid(field) {
            self.turn_field(field, Step::Next);
            steps += 1;
        }
        steps
    }

    /// **摊开光标那一行的取值栏**（`p3-session-legibility/05` 票面第一条）。
    ///
    /// 那一列**就是那一行的取值环**，从「没说」那一格起走一圈落回它为止——
    /// 一步一步走的就是 [`Self::turn_field`]，因此这里没有第二份清单：
    /// 环上加一个取值，摊开那一列当场跟着多一格。
    /// 每一格印成什么走 [`Self::shown`]，与那一行自己印的是同一份。
    ///
    /// **此刻生效的是第几格，是数出来的、不是认字认出来的**：从这一行此刻停的那一格
    /// 起走 `back` 步到得了「没说」，它就在环上倒数第 `back` 格。
    /// 这一步的前提是「那一行的取值在环上」，而守门的是 [`Field::drills`]——
    /// **型号那一行走的是另一路**（[`Self::panels`]：摊开的是面板，不是环）。
    ///
    /// **摊不开的行在这里到不了**：按键表在那几行上根本不派这个动作
    /// （见 [`Self::browsing_action`]），这里的卫语句只是不让这个函数自己出岔子。
    fn unfold(&mut self) {
        let field = self.field();
        if field.shape() != Shape::Cycle {
            return;
        }
        if field.drills() {
            self.focus = Focus::Valuing(self.panels(None));
            return;
        }
        let mut probe = self.clone();
        let back = probe.turn_to_unsaid(field);
        let mut cells = Vec::new();
        loop {
            cells.push(probe.shown(field));
            probe.turn_field(field, Step::Next);
            if probe.unsaid(field) {
                break;
            }
        }
        let chosen = (cells.len() - back) % cells.len();
        self.focus = Focus::Valuing(Values {
            field,
            panel: None,
            cells,
            at: chosen,
            chosen: Some(chosen),
        });
    }

    /// 型号那一行摊开的**第一层：面板**（`p3-session-legibility/06` 票面第一条）。
    ///
    /// 每一行印的是那块面板自己的 `Display`——**分辨率 · PPI · 灰阶数 · 黑白／彩色**，
    /// 会话这一侧不另写一份格式（`tonefit::Panel` 那一份是唯一出处）。
    /// 分组走 [`Profile::devices_by_panel`]，与未知型号那条错误消息**同一份**
    /// （本票票面第五条）：内置表里加一个型号、多一块面板，这一列当场跟着变。
    ///
    /// 第一格仍是**「没挑」那一格**，与别的行同一条（`CONTEXT.md` 的《会话》：
    /// 两层的每一格都有一个「没说」的位置）；印成什么走 [`Self::shown`]。
    /// **它是这一层唯一定得下来的一格**——别的几格是面板，而面板不是型号那一行的取值
    /// （见 [`Values::drills`]）。
    ///
    /// **光标停在当前型号所在的那块面板上**（本票票面第四条），`cursor` 给了就停在它上面
    /// （从下钻那一层[退一步](Self::cancel)回来时用的正是它）。型号停在**表外的一个名字**
    /// 上时没有它的那一块面板：记号一格都不画，光标落回「没挑」那一格（本票票面第七条）。
    ///
    /// **不叫 `device_level` 一类的名字**：`CONTEXT.md` 的《会话》里**设备层**是三层配置的
    /// 第一层（[`Layer::Device`]），与这两层毫无关系——领域词一词一义。
    fn panels(&self, cursor: Option<Panel>) -> Values {
        let groups = Profile::devices_by_panel();
        let mut probe = self.clone();
        probe.device.profile = None;
        let mut cells = vec![probe.shown(Field::Profile)];
        cells.extend(groups.iter().map(|(panel, _)| panel.to_string()));
        // 认的是**这个名字在哪一块面板底下**，与型号那一环认名字用的是同一个比法
        // （见 [`next_device`]）：内置表里没有的那个名字两处一致地认不出来。
        let chosen = match self.device.profile.as_deref() {
            None => Some(0),
            Some(current) => groups
                .iter()
                .position(|(_, devices)| devices.contains(&current))
                .map(|at| at + PANELS_START_AT),
        };
        let at = cursor
            .and_then(|wanted| groups.iter().position(|(panel, _)| *panel == wanted))
            .map(|at| at + PANELS_START_AT)
            .or(chosen)
            .unwrap_or(0);
        Values {
            field: Field::Profile,
            panel: None,
            cells,
            at,
            chosen,
        }
    }

    /// 型号那一行下钻进去那一层：**这块面板底下的型号**
    /// （`p3-session-legibility/06` 票面第二条）。
    ///
    /// 一格一个型号，**没有「没挑」那一格**——它是型号那一行的取值、不是某一块面板底下的
    /// 取值，就摆在上一层的第一格（停车场 Q143）。这一层里挑哪一个输出都一样
    /// （面板相同的型号输出完全一致，见 [`Profile::devices_by_panel`]），
    /// 一块面板底下**只有一个型号**时这一层也照走：那一格答的是「这块屏只有这一台设备」，
    /// 而那是一句有内容的话（停车场 Q142）。
    ///
    /// **面板与它底下那几个型号一起收进来**：调用方（[`Self::drill`]）从分组里取出的
    /// 本来就是这一对，再按面板去查一次等于把同一张表翻两遍。
    ///
    /// **光标停在当前型号上**（本票票面第四条）；当前型号不在这块面板底下时没有一格是
    /// 生效着的，光标停在头一格上。
    fn devices_under(&self, panel: Panel, devices: &[&'static str]) -> Values {
        let chosen = self
            .device
            .profile
            .as_deref()
            .and_then(|current| devices.iter().position(|device| *device == current));
        Values {
            field: Field::Profile,
            panel: Some(panel),
            cells: devices.iter().copied().map(str::to_owned).collect(),
            at: chosen.unwrap_or(0),
            chosen,
        }
    }

    /// **下钻到光标停着的那一块面板底下**（`p3-session-legibility/06` 票面第二条）。
    ///
    /// 面板那一层的第几格对着哪一块面板，问的是 [`Profile::devices_by_panel`]——
    /// 与摊开那一列时问的是同一份，因此不必把面板逐块记在 [`Values`] 里
    /// （内置表在一趟会话里不会变）。
    ///
    /// **别的格子在这里到不了**：按键表在那几格上派的是[定](Action::Choose)
    /// （见 [`Values::at_a_panel`]），这里的卫语句只是不让这个函数自己出岔子。
    fn drill(&mut self) {
        let Focus::Valuing(values) = &self.focus else {
            return;
        };
        if !values.at_a_panel() {
            return;
        }
        let at = values.at;
        let Some((panel, devices)) = Profile::devices_by_panel()
            .into_iter()
            .nth(at - PANELS_START_AT)
        else {
            return;
        };
        self.focus = Focus::Valuing(self.devices_under(panel, &devices));
    }

    /// **定下取值栏上停着的那一格**，回左栏。
    ///
    /// 做法是**转到那一格上**：先转到「没说」那一格，再往前走 `at` 格——
    /// 走的每一步都是 [`Self::turn_field`]，也就是 `←→` 就地转一格走的那一条。
    /// 「两条路改的是同一格」因此不必靠自觉：摊开这一路根本没有自己的写入路径。
    ///
    /// **停着的还是生效着的那一格时一步都不走**：那一下与 `Esc` 一样一格不改。
    /// 不然「摊开、什么都不改、按 `⏎`」会把标定出来的两个数清掉——ADR 0002 那一下
    /// 跟着**换**型号走，而这一下根本没换。
    ///
    /// **型号那一行定的是一个型号名，不是环上走几步**：那两层列的是面板与面板底下的
    /// 型号，环上第几格答不出来。它写下去走的仍是[同一条写入路径](Self::set_device)
    /// ——`←→` 就地转一格落到的也是它，ADR 0002 那一下清空因此不会有一处忘了跟着做。
    fn choose(&mut self) {
        let Focus::Valuing(values) = &self.focus else {
            return;
        };
        let (field, at, chosen) = (values.field, values.at, values.chosen);
        if field.drills() {
            let device = match values.panel {
                // 下钻进去那一层：每一格是一个型号名。
                Some(_) => Some(values.cells[at].clone()),
                // 面板那一层定得下来的只有第一格「没挑」；别的几格上派的是
                // [下钻](Action::Drill)，走不到这里（见 [`Values::at_a_panel`]）。
                None if at == 0 => None,
                None => return,
            };
            self.focus = Focus::Config;
            if chosen == Some(at) {
                return;
            }
            self.set_device(device);
            return;
        }
        self.focus = Focus::Config;
        if chosen == Some(at) {
            return;
        }
        self.turn_to_unsaid(field);
        for _ in 0..at {
            self.turn_field(field, Step::Next);
        }
    }

    /// **换掉型号。写型号只有这一条路。**
    ///
    /// `←→` 就地转一格（[`Self::turn_field`] 的型号那一支）与取值栏上定下来的那一下
    /// （[`Self::choose`]）走的都是它：**换掉型号仍旧把标定出来的灰阶数与阈值清空**
    /// （ADR 0002：判据与阈值跟着面板走、不可跨面板比较），而那件事分成两份写就会有
    /// 一处忘了跟着做。
    fn set_device(&mut self, device: Option<String>) {
        self.device.profile = device;
        self.device.gray_levels = None;
        self.device.threshold = None;
    }

    fn toggle_volume(&mut self) {
        if let Field::Volume(at) = self.field()
            && let Some(volume) = self.scope.volumes.get_mut(at)
        {
            volume.on = !volume.on;
        }
    }

    fn remove_volume(&mut self) {
        if let Field::Volume(at) = self.field()
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
        let Focus::Editing(edit) = &self.focus else {
            return;
        };
        let (field, typed) = (edit.field, edit.buffer.trim().to_owned());
        match self.take(field, &typed) {
            Ok(()) => self.focus = Focus::Config,
            Err(error) => self.says(Some(format!("{error}"))),
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
    use crate::session::live::{Resuming, fixture};
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
    /// **两维之后逐段按两维问**（ADR 0017）：一到五段是没跑过那个阶段上焦点的几副样子
    /// （左栏停在转得动的行／打字改的行／卷行上、取值栏摊着、型号那两层、编辑到一半）；
    /// 六段起换阶段——跑着、等答话，各问一遍**焦点在左栏**与**焦点在报告区**；
    /// 七段是展开着，连同它在跑着与等答话时的那一副；八到十段是预设那一栏；
    /// 十一、十二两段是覆盖层那两个键与覆盖层自己那一支（问的是**没跑过**那个阶段）。
    ///
    /// 每一段都问到「这个键在这里没有意义」那几个——[`Action::Ignored`] 是一个取值，
    /// 不是遗漏；而**跨两维的那几条**另在两处再问一遍：三层只读、按停在报告区上照样
    /// 按得动、`⇥` 的三个意思在 [`the_two_dimensions_move_one_at_a_time`] 上，
    /// 覆盖层那一支的另外三个阶段在 [`the_overlay_hands_the_stage_keys_back`] 上。
    #[test]
    fn which_keys_do_what_in_which_state() {
        let mut session = Session::new();

        // 一、浏览，光标停在「型号」——一个转得动的行。
        assert_eq!(session.field(), Field::Profile);
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Right), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
        // 转得动的行上「动手」是摊开那一列取值；型号那一行摊的是面板，见三之三。
        assert_eq!(session.action(Key::Enter), Action::Unfold);
        assert_eq!(session.action(Key::Space), Action::Unfold);
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
            session.go_to(field);
            assert_eq!(session.action(Key::Char('c')), Action::Chart, "{field:?}");
        }
        session.go_to(Field::Profile);
        // 打字与补全在浏览时没有意义；删卷的键在不是卷的行上也没有。
        assert_eq!(session.action(Key::Char('z')), Action::Ignored);
        assert_eq!(session.action(Key::Char('d')), Action::Ignored);
        assert_eq!(session.action(Key::Tab), Action::Ignored);
        assert_eq!(session.action(Key::Backspace), Action::Ignored);
        // **展开逐页一趟都没跑过时不派**（停车场 Q167）：那时报告上一卷都没有，
        // 按下去从前只换来一句「还没跑过」——而「按了有话说」与「按得动」
        // 在 `?` 那张表上长得一模一样。收场之后它照旧按得动，两头见
        // [`the_key_table_leaves_out_the_keys_that_go_nowhere_right_now`]。
        assert_eq!(session.action(Key::Char('e')), Action::Ignored);
        // **不进缓冲那个键在哪一块上都掀得开全部键那一张**（`p4-parking-lot/07`
        // 票面第三条）：这一块上它与 `?` 派的是同一件事。
        assert_eq!(session.action(Key::F1), Action::Reveal(Overlay::Keys));

        // 二、浏览，光标停在一个打字改的行上：回车进编辑，左右转不动。
        session.go_to(Field::CacheBudget);
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
        session.go_to(Field::Volume(0));
        assert_eq!(session.action(Key::Space), Action::Toggle);
        assert_eq!(session.action(Key::Enter), Action::Toggle);
        assert_eq!(session.action(Key::Char('d')), Action::Remove);
        assert_eq!(session.action(Key::Left), Action::Ignored);
        // **范围层**上同样没有意义。
        assert_eq!(session.action(Key::Char('c')), Action::Ignored);

        // 三之二、**取值栏摊着**（`CONTEXT.md` 的《会话》：取值栏）：`↑↓` 在那一列上
        // 挪一格，`⏎`／`→` 定，`Esc`／`←` 一格不改地回去。`←→` 归这一层，
        // **不再就地转那一行**（票面第五条）。
        session.go_to(Field::Filter);
        // 摊开的是「就在这一行上动手」那个键——转得动的行上它从前是就地转一格。
        assert_eq!(session.action(Key::Enter), Action::Unfold);
        assert_eq!(session.action(Key::Space), Action::Unfold);
        // **环那一套照旧**：`←→` 在浏览时仍是就地转一格（票面：环保留）。
        assert_eq!(session.action(Key::Right), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
        session.press(Key::Enter);
        assert!(session.valuing().is_some(), "没摊开");
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Enter), Action::Choose);
        assert_eq!(session.action(Key::Space), Action::Choose);
        // 摊开时 `←→` 走的是这一列的两头，不是那一行的取值环。
        assert_eq!(session.action(Key::Right), Action::Choose);
        assert_eq!(session.action(Key::Left), Action::Cancel);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 这一列是一份现成的取值，不是一个缓冲：打字、补全、切一栏都没有意义；
        // 起一趟、展开、预设、标定图都要先退回左栏，而那是一个键的事。
        for key in [
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
            Key::Char('z'),
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('e'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('d'),
            Key::Char('s'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在取值栏上不该生效"
            );
        }
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config, "Esc 没退回左栏");

        // 三之三、**型号那一行摊开的是两层**（`CONTEXT.md` 的《会话》：下钻）：
        // 第一层是面板，`⏎`／`→` 在一块面板上是**进去看**（面板不是一个取值），
        // 在第一格「没挑」上仍是定；下钻那一层上一律是定，而 `Esc`／`←` 退回的是
        // **面板那一层**，再按一次才出这一栏。
        session.go_to(Field::Profile);
        // 摊开的键与别的转得动的行一个样：分岔在摊开**之后**，不在这个键上。
        assert_eq!(session.action(Key::Enter), Action::Unfold);
        assert_eq!(session.action(Key::Space), Action::Unfold);
        assert_eq!(session.action(Key::Right), Action::Cycle(Step::Next));
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
        session.press(Key::Enter);
        let panels = session.valuing().expect("没摊开").clone();
        assert_eq!(panels.panel(), None, "摊开落在第一层上");
        assert_eq!(panels.at(), 0, "还没挑型号，光标停在「没挑」那一格上");

        // 面板那一层，光标停在第一格「没挑」上：它是型号那一行真正的一个取值，定得下来。
        assert!(!panels.at_a_panel(), "「没挑」那一格该是定，不是下钻");
        assert_eq!(session.action(Key::Enter), Action::Choose);
        assert_eq!(session.action(Key::Space), Action::Choose);
        assert_eq!(session.action(Key::Right), Action::Choose);
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Left), Action::Cancel);

        // 挪到一块面板上：同一个键换了意思——进去看它底下有哪几个型号。
        session.press(Key::Down);
        assert!(
            session.valuing().expect("没摊开").at_a_panel(),
            "停的不是一块面板"
        );
        assert_eq!(session.action(Key::Enter), Action::Drill);
        assert_eq!(session.action(Key::Space), Action::Drill);
        assert_eq!(session.action(Key::Right), Action::Drill);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Left), Action::Cancel);

        // 下钻进去那一层：每一格是一个型号，一律定得下来。
        session.press(Key::Enter);
        let inside = session.valuing().expect("没下钻").clone();
        assert!(inside.panel().is_some(), "没进到面板底下");
        assert!(!inside.at_a_panel(), "下钻那一层上没有第三层可进");
        assert_eq!(session.action(Key::Enter), Action::Choose);
        assert_eq!(session.action(Key::Space), Action::Choose);
        assert_eq!(session.action(Key::Right), Action::Choose);
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 那十一个在这两层上同样没有意义：这一列仍是一份现成的取值，不是一个缓冲。
        for key in [
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
            Key::Char('z'),
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('e'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('d'),
            Key::Char('s'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在下钻那一层上不该生效"
            );
        }

        // **退一步，不是退到底**：`Esc` 退回面板那一层，光标落回进来的那一块上；
        // 再按一次才出这一栏。
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        session.press(Key::Esc);
        let back_out = session.valuing().expect("退过头了，出了这一栏").clone();
        assert_eq!(back_out.panel(), None, "没退回面板那一层");
        assert_eq!(back_out.at(), 1, "光标没落回进来的那一块面板上");
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config, "第二下 Esc 没退回左栏");

        // 四、编辑一个路径：字进缓冲，Tab 补全，回车收下，Esc 丢掉。
        session.go_to(Field::Out);
        session.press(Key::Enter);
        assert!(matches!(session.focus(), Focus::Editing(_)));
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
        session.go_to(Field::CacheBudget);
        session.press(Key::Enter);
        assert_eq!(session.action(Key::Tab), Action::Ignored);

        // 六、跑起来之后，**焦点还在左栏**：三层只读，试算与执行也按不动
        // （一趟里跑不了第二趟）。按得动的只剩三个：`s`（按停）、`⇥`（切焦点）
        // 与 Ctrl-C（退出）。
        session.press(Key::Esc);
        session.run_started();
        for key in [
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            // 回车与空格按不动，**取值栏因此摊不开**：三层只读那一条不因取值栏松动
            // （`CONTEXT.md` 的《会话》：一趟跑起来之后三层都只读）。
            Key::Enter,
            Key::Space,
            Key::Backspace,
            Key::Esc,
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('q'),
            Key::Char('d'),
            // 展开那个键在左栏上跑着时仍不派：展开的是**报告区**那一卷，
            // 而焦点此刻不在那一块上（切过去它就按得动了，见六之三）。
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
        // **`⇥` 跑着时切得动焦点**（ADR 0017）：它不改三层里的任何一格，
        // 而几十分钟的一趟里回头看第一卷正是这一下。
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Report));
        // Ctrl-C 仍旧退得出去：它在**每一个**状态下都是退出。
        assert_eq!(session.action(Key::Interrupt), Action::Quit);

        // 六之二、**停在决策点上等人拿主意**（`p1-session/14`、`volume-discovery/07`）：
        // 三层照旧只读，而按得动的换成了答话那三个——`x` 接着做第二遍，
        // `a` 剩下的卷都这样，`s` 收尾。
        session.at_the_decision_point(true);
        assert!(session.deciding(), "没进等答话那个状态");
        for key in [
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Space,
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
        // 答话那三个键。**`x` 在这里不是「起一趟」，`s` 也不是升闩**——
        // 决策点回的是当场那个字（ADR 0012 决定第 2 条）。
        // `a` 答的字与 `x` 逐字相同，差别只在它管几卷（`volume-discovery/07`）。
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Answer(Instruction::Continue, Reach::ThisVolume)
        );
        assert_eq!(
            session.action(Key::Char('a')),
            Action::Answer(Instruction::Continue, Reach::ForTheRest)
        );
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Answer(Instruction::Finish, Reach::ThisVolume)
        );
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // `⇥` 在这一刻也切得动焦点，与跑着时同一条：它不改三层里的任何一格。
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Report));

        // 六之三、**焦点落在报告区上**（`p3-session-legibility/10`，ADR 0017）：
        // 报告区默认那一副是**目录表**（`volume-discovery/08`）——`↑↓` 选一枝、
        // `⏎` 展开这一枝、`e` 直接展开逐页、`g` 回到跟随、`⇥` 切回左栏，
        // 而**阶段那一维那几个键一个不少**——答话那三个在这一块上照样按得动
        // （票面第五条），三层照旧一个改动键都不派（票面第四条）。
        session.press(Key::Tab);
        assert_eq!(session.focus(), &Focus::Report, "⇥ 没把焦点切过去");
        assert!(session.deciding(), "切个焦点把阶段那一维也带走了");
        assert_eq!(session.action(Key::Down), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Select(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Select(Step::Back));
        // `⏎`／空格是「就在这一行上动手」——这一行是一枝，动手就是展开它；
        // `e` 恒是「展开这一卷的逐页」，两个键各答各的（见 [`report_action`]）。
        assert_eq!(session.action(Key::Enter), Action::Open);
        assert_eq!(session.action(Key::Space), Action::Open);
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        assert_eq!(session.action(Key::Char('g')), Action::Follow);
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Config));
        assert_eq!(session.action(Key::BackTab), Action::Focus(Pane::Config));
        // 答话那三个：一个字都没变（`x` 接着做、`a` 剩下的卷都这样、`s` 收尾）。
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Answer(Instruction::Continue, Reach::ThisVolume)
        );
        assert_eq!(
            session.action(Key::Char('a')),
            Action::Answer(Instruction::Continue, Reach::ForTheRest)
        );
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Answer(Instruction::Finish, Reach::ThisVolume)
        );
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 三层照旧改不动，`q`／`Esc` 照旧退不出去：**两条都归阶段那一维**，
        // 焦点落在哪一块与它们无关。
        for key in [
            Key::Left,
            Key::Right,
            Key::Backspace,
            Key::Esc,
            Key::Char('q'),
            Key::Char('t'),
            Key::Char('d'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('z'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在报告区上（等答话）不该生效"
            );
        }
        // 答完话回「跑着」那一副：这一块上按停又是升闩了，两级语义一格不变，
        // 而这一块自己那四个键**跑着的时候一个不少**（票面第二条：三个阶段都用得动）。
        session.press(Key::Char('x'));
        assert!(!session.deciding(), "答完话还停在决策点上");
        assert_eq!(session.focus(), &Focus::Report, "答完话把焦点也搬走了");
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        assert_eq!(session.action(Key::Char('x')), Action::Ignored);
        assert_eq!(session.action(Key::Down), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Select(Step::Back));
        assert_eq!(session.action(Key::Enter), Action::Open);
        assert_eq!(session.action(Key::Char('g')), Action::Follow);
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Config));
        // 回左栏：按停那个键在那一块上照旧是升闩——两块上是同一件事。
        session.press(Key::Tab);
        assert_eq!(session.focus(), &Focus::Config);
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        assert_eq!(session.action(Key::Char('x')), Action::Ignored);

        // 收场之后配置又改得动，而按停那个键在浏览时没有意义——还没有东西可停。
        session.run_finished();
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('s')), Action::Ignored);
        // 浏览时 `e` 展开，而它与光标停在哪一行无关。
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        session.go_to(Field::Out);
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        // **`⇥` 收场之后照旧切得过去**：报告一行不少地摆在那儿。
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Report));

        // 六之四、**收场了、焦点落在报告区**：`↑↓`／`⏎`／`g`／`⇥` 与跑着时逐字相同，
        // 而阶段那一维这时一个键都不派——`q`／`Esc` 于是退得出去（跑着时它们按不动）。
        session.press(Key::Tab);
        assert_eq!(session.focus(), &Focus::Report);
        assert_eq!(session.action(Key::Down), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Enter), Action::Open);
        assert_eq!(session.action(Key::Char('g')), Action::Follow);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Esc), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 起一趟那两个键归左栏：起一趟之前要看的是三层，而焦点此刻不在它上面。
        for key in [
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('s'),
            Key::Char('a'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('d'),
            Key::Left,
            Key::Right,
            Key::Backspace,
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在报告区上（收场了）不该生效"
            );
        }
        // 一趟都没跑过时切不过去：那时报告区里连一卷都没有（屏上也不摆这个键）。
        session.press(Key::Tab);
        assert_eq!(Session::new().action(Key::Tab), Action::Ignored);

        // 六之五、**展开着一枝**（`volume-discovery/08`）：这一块就是从前的报告区——
        // `↑↓` 选一卷、`⏎`／`e` 展开它的逐页、`g` 回到跟随、`⇥` 切回左栏，
        // 出路多一个 `Esc`（收起回目录表）。
        session.press(Key::Tab);
        session.open(PathBuf::from("库"));
        assert_eq!(session.focus(), &Focus::Opened(PathBuf::from("库")));
        assert_eq!(session.action(Key::Down), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Select(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Select(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Select(Step::Back));
        // 这一行是一卷，「往下走一级」与「展开这一卷的逐页」到的是同一处。
        assert_eq!(session.action(Key::Enter), Action::Expand);
        assert_eq!(session.action(Key::Space), Action::Expand);
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        assert_eq!(session.action(Key::Char('g')), Action::Follow);
        assert_eq!(session.action(Key::Esc), Action::Collapse, "Esc 该收起一级");
        assert_eq!(session.action(Key::Tab), Action::Focus(Pane::Config));
        assert_eq!(session.action(Key::BackTab), Action::Focus(Pane::Config));
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        for key in [
            Key::Left,
            Key::Right,
            Key::Backspace,
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('s'),
            Key::Char('d'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('z'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 展开着一枝时不该生效"
            );
        }
        // 收起一级回目录表，再切回左栏——两级各有一个键回得去。
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Report);
        session.press(Key::Tab);

        // 七、展开之后：`↑↓` 选一页，`a` 换一副列法，`⇥` 换一卷，`e`／`Esc` 收起。
        // 起一趟的那两个键在这里按不动——报告区正摊着上一趟的逐页。
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(0)));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        // `a` 带着**去哪一档**，不是「切一下」：屏底那一行要说得出按过去是哪一副。
        assert_eq!(session.action(Key::Char('a')), Action::List(Listing::All));
        assert_eq!(session.press(Key::Char('a')), Exit::Stay);
        assert_eq!(
            session.expansion().expect("展开着").listing,
            Listing::All,
            "`a` 没切到全部页"
        );
        assert_eq!(
            session.action(Key::Char('a')),
            Action::List(Listing::Notable)
        );
        session.press(Key::Char('a'));
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
            // `←→` 在这里不派动作：逐页那一副横着摆不下时**砍列**，不横着滚
            // （`p3-session-legibility/11`，与卷表从此同一条）。
            Key::Left,
            Key::Right,
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

        // 七之二、**展开着而一趟正跑着**（`p3-session-legibility/10`，推翻停车场 Q72）：
        // 选页那几个键一格不变，而阶段那一维那几个键跟着进来——按停按得动，
        // `q` 反过来按不动（停车场 Q63）。**两者不冲突**：`s` 是字母键，选页走方向键。
        session.run_started();
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Tab), Action::Turn(Step::Next));
        assert_eq!(session.action(Key::Char('e')), Action::Collapse);
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        assert_eq!(session.action(Key::Char('q')), Action::Ignored);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        assert_eq!(
            session.action(Key::Char('a')),
            Action::List(Listing::All),
            "跑着时换一副列法该按得动"
        );
        // **等答话时答话那三个都在**：与焦点落在哪一块无关（票面第五条）。
        // `a` 这一刻归答话——换一副列法等得起，一条停在决策点上的线程等不起
        // （停车场 Q161，见 [`expanded_action`]）。
        session.at_the_decision_point(true);
        assert_eq!(
            session.action(Key::Char('a')),
            Action::Answer(Instruction::Continue, Reach::ForTheRest)
        );
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Answer(Instruction::Continue, Reach::ThisVolume)
        );
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Answer(Instruction::Finish, Reach::ThisVolume)
        );
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        session.at_the_decision_point(false);
        session.run_finished();

        // 八、预设那一栏，在列表上走：`↑↓` 挪一行，`⏎`／空格随停在哪一行分派
        // （停在一份预设上是套用它），`d` 删掉停着的那一份，`p`／`Esc` 回配置。
        session.press(Key::Esc);
        session.pick(vec!["漫画".to_owned(), "画集".to_owned()], presets_file());
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Enter), Action::Take);
        assert_eq!(session.action(Key::Space), Action::Take);
        assert_eq!(session.action(Key::Char('d')), Action::Erase);
        assert_eq!(session.action(Key::Char('p')), Action::Cancel);
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // 这一栏上一行只有一个名字：没有取值环，也没有第三件事可做。
        for key in [
            Key::Left,
            Key::Right,
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
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

        // 九、停在末尾那一行（＋ 存成一份新的）上：`⏎` 是「打个名字」，不是套用；
        // 而 `d` 在这一行上按不动——那一行不是一份预设，没有东西可删。
        session.press(Key::Up);
        assert_eq!(session.picking().expect("那一栏开着").picked(), None);
        assert_eq!(session.action(Key::Enter), Action::Edit);
        assert_eq!(session.action(Key::Char('d')), Action::Ignored);
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
        assert_eq!(session.focus(), &Focus::Config);

        // 十一、**掀开覆盖层那两个键在打字之外的每一块上都按得动**
        // （`p3-session-legibility/12`），而**打字的那两块上它们是字**。
        assert_eq!(
            session.action(Key::Char('?')),
            Action::Reveal(Overlay::Keys)
        );
        assert_eq!(
            session.action(Key::Char('i')),
            Action::Reveal(Overlay::Premises)
        );
        session.go_to(Field::Out);
        session.press(Key::Enter);
        assert_eq!(session.action(Key::Char('?')), Action::Insert('?'));
        assert_eq!(session.action(Key::Char('i')), Action::Insert('i'));
        // **打字那两块上掀得开的只有[不进缓冲那个键](Key::F1)**
        // （`p4-parking-lot/07` 票面第三条，停车场 Q165）：它不是一个字，
        // 那两块因此认不下它（见 [`minds_this_key_itself`]）。
        assert_eq!(session.action(Key::F1), Action::Reveal(Overlay::Keys));
        session.press(Key::Esc);

        // 十二、**覆盖层掀着：`↑↓` 读，`Esc` 关，另一张那个键换过去，别的一律不派。**
        session.press(Key::Char('?'));
        assert_eq!(session.action(Key::Down), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Char('j')), Action::Move(Step::Next));
        assert_eq!(session.action(Key::Up), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Char('k')), Action::Move(Step::Back));
        assert_eq!(session.action(Key::Esc), Action::Cancel);
        // 掀开它的那个键按回去就是关掉它；另一张那个键换过去。
        // [不进缓冲那个键](Key::F1)与 `?` 掀的是同一张，按回去因此也是关掉。
        assert_eq!(session.action(Key::Char('?')), Action::Cancel);
        assert_eq!(session.action(Key::F1), Action::Cancel);
        assert_eq!(
            session.action(Key::Char('i')),
            Action::Reveal(Overlay::Premises)
        );
        // `Ctrl-C` 照旧是退出会话——它在每一个状态下都是。
        assert_eq!(session.action(Key::Interrupt), Action::Quit);
        // **除了按停与答话，别的键一律不派**：`q` 也在内——这一块上的「退一步」是 `Esc`，
        // 而它是这一块自己认下的一个字母，不交给阶段那一维（`overlay_action`）。
        // **按停与答话那几个这一刻同样不派**，但那是**阶段**那一维说的：一趟都没跑过时
        // 它一个键都派不出来。它们在另外三个阶段上派得出什么，见
        // [`the_overlay_hands_the_stage_keys_back`]——覆盖层那一支的另一维在那里逐条问过。
        for key in [
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Space,
            Key::Tab,
            Key::BackTab,
            Key::Backspace,
            Key::Char('q'),
            Key::Char('s'),
            Key::Char('a'),
            Key::Char('t'),
            Key::Char('x'),
            Key::Char('e'),
            Key::Char('p'),
            Key::Char('c'),
            Key::Char('z'),
        ] {
            assert_eq!(
                session.action(key),
                Action::Ignored,
                "{key:?} 在覆盖层上不该派动作"
            );
        }
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config);
    }

    /// **覆盖层盖住一块焦点，不替掉它**（`p3-session-legibility/12` 票面第四条一带）。
    ///
    /// `Esc` 关掉之后回的是掀开之前那一块，一格没动——展开着那一块最认得出来
    /// （它带着展开的是哪一卷、列的是哪几页、光标停在第几页三样东西）。
    ///
    /// **掀着一张时按另一张那个键是换过去，不叠第二层**：`Esc` 照旧一下回到屏上那一块，
    /// 要按几下不必用户自己数。
    ///
    /// 先跑一趟再收场：[前提那一张](Overlay::Premises)**一趟都没跑过时根本不派**
    /// （停车场 Q167），而这一条要问的正是「换过去」——换到一张掀不开的上面
    /// 问不出那件事。
    #[test]
    fn an_overlay_covers_one_block_and_gives_it_back() {
        let mut session = Session::new();
        session.run_started();
        session.run_finished();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(2)));
        session.press(Key::Down);
        session.press(Key::Char('a'));
        let expanded = session.expansion().cloned().expect("展开着");

        session.press(Key::Char('?'));
        assert!(session.overlay().is_some(), "没掀开");
        assert!(session.expansion().is_none(), "掀着的时候展开那一块还在场");
        // **那张表问的是覆盖层盖住的那一块**：此刻列着全部页，`a` 按过去因此是
        // 「只列要紧的页」。问 `self.focus` 的话这里会答成「列全部页」——
        // 屏上正列着全部页，而表说按 `a` 去列全部页。
        assert_eq!(
            session
                .keys_of(KeyGroup::Expanded)
                .into_iter()
                .find(|(key, _)| *key == Key::Char('a'))
                .map(|(_, action)| action),
            Some(Action::List(Listing::Notable))
        );
        // 换一张：盖住的仍是展开着那一块，不是上一张覆盖层。
        session.press(Key::Char('i'));
        assert_eq!(
            session.overlay().map(|covered| covered.overlay),
            Some(Overlay::Premises)
        );
        session.press(Key::Esc);
        assert_eq!(session.expansion().cloned(), Some(expanded), "没原样还回来");
    }

    /// **覆盖层掀着时按停与答话那三个照样按得动**（ADR 0017 决定第 4 条，
    /// `p4-parking-lot/06` 收的停车场 Q164）：这一块与左栏、报告区、展开着一样，
    /// 把自己不认的键交给 [`stage_action`]。
    ///
    /// 三件事：
    ///
    /// - **跑着时 `s` 是按停，两级语义一格不变**——一次收尾、再一次中止，到顶不再有
    ///   （闩升的是同一格，不因为屏上盖着一张纸而另算一份）；
    /// - **等答话时 `x`／`a`／`s` 三个各答各的**，与它们在别的每一块上逐字同一件事；
    /// - **按停与答话都不关掉这一张**：它盖住的那一块原样在底下，`Esc` 照旧还得回来。
    ///
    /// 交下去的**只有这几个**——这一块自己认下的那几个字母见
    /// [`the_overlay_keeps_the_letters_that_are_its_own`]。
    #[test]
    fn the_overlay_hands_the_stage_keys_back() {
        let mut session = Session::new();
        session.run_started();
        session.press(Key::Char('?'));
        assert_eq!(session.stage(), Stage::Running(Instruction::Continue));

        // 跑着：按停按得动，而两级仍旧是同一个键按两次。
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
        session.press(Key::Char('s'));
        assert_eq!(
            session.stopping(),
            Instruction::Finish,
            "掀着的时候按停没升"
        );
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Abort, "第二下没升到中止");
        // 闩到了顶：再按一次没有更强的一级可去，与别的块上一个待遇。
        assert_eq!(session.action(Key::Char('s')), Action::Ignored);
        assert!(session.overlay().is_some(), "按停把这一张关掉了");

        // 等答话：那三个各答各的，答的是当场那个字，不是闩。
        let mut session = Session::new();
        session.run_started();
        session.at_the_decision_point(true);
        session.press(Key::Char('?'));
        assert_eq!(
            session.action(Key::Char('x')),
            Action::Answer(Instruction::Continue, Reach::ThisVolume)
        );
        assert_eq!(
            session.action(Key::Char('a')),
            Action::Answer(Instruction::Continue, Reach::ForTheRest)
        );
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Answer(Instruction::Finish, Reach::ThisVolume)
        );

        // 答完话回到跑着，而这一张还掀着——`Esc` 原样回到它盖住的那一块。
        session.press(Key::Char('x'));
        assert_eq!(
            session.stage(),
            Stage::Running(Instruction::Continue),
            "答完没回到跑着"
        );
        assert!(session.overlay().is_some(), "答话把这一张关掉了");
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config);
        assert_eq!(session.action(Key::Char('s')), Action::Stop);
    }

    /// **这一块自己认下的那几个字母不往下交**——「除了按停与答话，别的键一律不派」
    /// 的另一半（`p4-parking-lot/06`）。
    ///
    /// 三个键，四个阶段各问一遍：
    ///
    /// - **`Esc` 恒是关掉这一张**，不是退出会话——没跑过与收场了那两个阶段上
    ///   [阶段那一维](stage_action)派的正是退出，交下去的话这一张就关不掉了；
    /// - **`q` 一个动作都不派**：这一块上的「退一步」是 `Esc`，而 `q` 交下去的后果
    ///   与 `Esc` 一样重——屏上那一刻摆着的只是一张读物；
    /// - **`Ctrl-C` 照旧是退出会话**：它在每一个状态下都是（[`Key::Interrupt`]）。
    #[test]
    fn the_overlay_keeps_the_letters_that_are_its_own() {
        let mut session = Session::new();
        for stage in [
            Stage::Fresh,
            Stage::Running(Instruction::Continue),
            Stage::Deciding(Instruction::Continue),
            Stage::Ended,
        ] {
            session.press(Key::Char('?'));
            let covered = session.overlay().expect("掀开了").clone();
            assert_eq!(
                overlay_action(&covered, Key::Esc, stage),
                Action::Cancel,
                "{stage:?}：Esc 不是关掉这一张"
            );
            assert_eq!(
                overlay_action(&covered, Key::Char('q'), stage),
                Action::Ignored,
                "{stage:?}：q 在覆盖层上派得出动作"
            );
            assert_eq!(
                overlay_action(&covered, Key::Interrupt, stage),
                Action::Quit,
                "{stage:?}：Ctrl-C 退不出去"
            );
            session.press(Key::Esc);
        }
    }

    /// **不进缓冲那个键在六块焦点、四个阶段上一处都没有主**
    /// （`p4-parking-lot/07` 票面第三条）。
    ///
    /// 挑它之前问出来的就是这一条，与 `p3-session-legibility/12` 挑 `i` 时同一个做法
    /// ——`a` 那种撞车（停车场 Q161）在它身上因此不存在。打字那两块也各问一遍：
    /// **它正是为那两块添的**，而那儿一个字符都不许被它顶掉。
    ///
    /// 有主的只有一处：[阶段那一维之外的那一组](KeyGroup::Always)——
    /// 它掀开[全部键](Overlay::Keys)那一张，与 `?` 派的是同一件事。
    #[test]
    fn the_key_that_does_not_go_into_the_buffer_is_spoken_for_nowhere_else() {
        let mut session = Session::new();
        let edit = Edit {
            field: Field::Out,
            buffer: String::new(),
            candidates: Vec::new(),
        };
        for stage in [
            Stage::Fresh,
            Stage::Running(Instruction::Continue),
            Stage::Deciding(Instruction::Continue),
            Stage::Ended,
        ] {
            session.stage = stage;
            for group in KeyGroup::ALL {
                if group == KeyGroup::Always {
                    continue;
                }
                assert_eq!(
                    session.acts(group, Key::F1),
                    Action::Ignored,
                    "{stage:?}：F1 在 {group:?} 上早有主了"
                );
            }
            // 打字那两块：每一个字符都进缓冲，而它不是一个字——那两块因此不认它，
            // 它一路交到掀开覆盖层那一处（见 [`minds_this_key_itself`]）。
            assert_eq!(editing_action(&edit, Key::F1), Action::Ignored);
            assert_eq!(naming_action(&Naming::default(), Key::F1), Action::Ignored);
            assert_eq!(
                session.acts(KeyGroup::Always, Key::F1),
                Action::Reveal(Overlay::Keys),
                "{stage:?}：F1 掀不开全部键那一张"
            );
        }
    }

    /// **打字那两块上调得出全部键**（`p4-parking-lot/07` 票面第三条，停车场 Q165）。
    ///
    /// 三件事，两块各问一遍：
    ///
    /// - **`?` 与 `i` 照旧是字**：按下去进缓冲，一张覆盖层都不掀——那两块上
    ///   每一个字符都是一个字，这一条 `p3-session-legibility/12` 就立着；
    /// - **[不进缓冲那个键](Key::F1)掀得开**，而**缓冲一个字都不动**：
    ///   掀开覆盖层是「盖住」，不是「替掉」（`p3-session-legibility/12` 票面第四条）；
    /// - **`Esc` 关掉之后原样回到打字那一块**，缓冲照旧在那儿。
    ///
    /// 「看不见的东西等于不存在」因此在这两块上不再成立：屏底那一行摆得出它
    /// （见 `super::draw::footer::Asked::all_keys`）。
    #[test]
    fn the_typing_blocks_can_call_up_every_key_too() {
        // 一、编辑一行。
        let mut session = Session::new();
        session.go_to(Field::Out);
        session.press(Key::Enter);
        session.press(Key::Char('库'));
        session.press(Key::Char('?'));
        session.press(Key::Char('i'));
        assert!(
            session.overlay().is_none(),
            "`?` 与 `i` 在这一块上掀开了覆盖层"
        );
        let typed = |session: &Session| match session.focus() {
            Focus::Editing(edit) => edit.buffer.clone(),
            other => panic!("不在编辑那一块上：{other:?}"),
        };
        assert_eq!(typed(&session), "库?i", "那两个字没进缓冲");

        session.press(Key::F1);
        assert_eq!(
            session.overlay().map(|covered| covered.overlay),
            Some(Overlay::Keys),
            "F1 没掀开全部键那一张"
        );
        session.press(Key::Esc);
        assert_eq!(typed(&session), "库?i", "关掉之后缓冲变了");

        // 二、打预设名。同一副样子，同一个键。
        let mut session = Session::new();
        session.pick(vec!["漫画".to_owned()], presets_file());
        session.press(Key::Down);
        session.press(Key::Enter);
        session.press(Key::Char('?'));
        let named = |session: &Session| {
            session
                .picking()
                .and_then(Picker::naming)
                .map(|naming| naming.buffer.clone())
        };
        assert!(session.overlay().is_none(), "`?` 在这一块上掀开了覆盖层");
        assert_eq!(named(&session).as_deref(), Some("?"), "那个字没进缓冲");

        session.press(Key::F1);
        assert_eq!(
            session.overlay().map(|covered| covered.overlay),
            Some(Overlay::Keys),
            "F1 没掀开全部键那一张"
        );
        session.press(Key::Esc);
        assert_eq!(named(&session).as_deref(), Some("?"), "关掉之后缓冲变了");
    }

    /// **`?` 那张表是按键表自己问出来的，不是另抄的一份**（票面：不许另抄一份）。
    ///
    /// 三条：
    ///
    /// - **每一组列的键都真派得出动作**，而且派的正是那一块的按键表说的那件事；
    /// - **阶段那一维那几个键只在「任何时候」那一组里**——照实列的话 `q 退出`
    ///   会在四组里各出现一次；
    /// - **只列此刻这个阶段派得出的键**：跑起来之后按停在这张表上，没跑过时它不在。
    #[test]
    fn the_key_table_is_asked_of_the_key_table_itself() {
        let mut session = Session::new();
        session.press(Key::Char('?'));

        let table = session.key_table();
        assert!(
            table.iter().any(|(group, _)| *group == KeyGroup::Config),
            "左栏那一组不在表上"
        );
        // **掀开覆盖层那两个键在六块焦点上一处都没有主**——`i` 与 `?` 各在每一块的
        // 按键表上问一遍，一处都不派动作（`a` 那种撞车因此在这两个键上不存在，
        // 停车场 Q161 记着的是另一个字母）。
        for group in KeyGroup::ALL {
            if group == KeyGroup::Always {
                continue;
            }
            for overlay in Overlay::ALL {
                assert_eq!(
                    session.acts(group, Key::Char(overlay.key())),
                    Action::Ignored,
                    "{:?} 在 {group:?} 上早有主了",
                    overlay.key()
                );
            }
        }
        for (group, keys) in &table {
            assert!(!keys.is_empty(), "{group:?} 那一组空着还列了出来");
            assert!(
                group.reachable(session.stage()),
                "{group:?} 此刻进不去还列了出来"
            );
            for (key, action) in keys {
                assert_ne!(*action, Action::Ignored, "{key:?} 派不出动作还列了出来");
                // **一个键只列一处**：到处都是同一件事的归「任何时候」，
                // 有一块另派的归各块自己（`Esc`：左栏上是退出会话，展开着上是收起）。
                assert_eq!(
                    session.means_the_same_everywhere(*key),
                    *group == KeyGroup::Always,
                    "{key:?} 归错组了：{group:?}"
                );
            }
        }
        // `Ctrl-C` 到处都是退出会话，因此只在「任何时候」那一组里；
        // `Esc` 在展开着上是收起，因此归各块自己，「任何时候」那一组不收它。
        assert!(session.means_the_same_everywhere(Key::Interrupt));
        assert!(!session.means_the_same_everywhere(Key::Esc));
        // 没跑过：按停不在这张表上（屏上不摆按不动的键）。
        assert!(
            !listed(&table, Key::Char('s')),
            "没跑过时按停那个键还在表上"
        );

        // 跑起来：它在了，而且在「任何时候」那一组里。
        session.press(Key::Esc);
        session.run_started();
        session.press(Key::Char('?'));
        let running = session.key_table();
        assert!(listed(&running, Key::Char('s')), "跑着时按停那个键不在表上");
        assert!(
            running
                .iter()
                .filter(|(_, keys)| keys.iter().any(|(key, _)| *key == Key::Char('s')))
                .all(|(group, _)| *group == KeyGroup::Always),
            "按停那个键列了不止一处"
        );
    }

    /// **`?` 那张表再滤一道：此刻按下去没有第二步的键不列**
    /// （`p4-parking-lot/07` 票面第二条，停车场 Q167 与 Q189）。
    ///
    /// 判据不是「这个键存不存在」，是「**此刻按下去有没有第二步**」。两处来源：
    ///
    /// - **按键表自己那一头**（Q167）：一趟都没跑过时[展开](Action::Expand)与
    ///   [前提那一张](Overlay::Premises)根本不派——它们从前派得出动作，而 `super::press`
    ///   那一层挡在前面说一句话，表上因此白纸黑字列着，按下去只换来一句话；
    /// - **「任何时候」那一组多过的那一道**（Q189）：覆盖层掀着时 `q` 一个动作都不派
    ///   （那一块的「退一步」是 `Esc`），而底下那几块上它是退出会话——照实列的话
    ///   这张表就在说一句此刻不成立的话。
    ///
    /// 滤掉的是**此刻按不动**，不是「这个键没有了」：关掉那一张，`q` 照旧退得出去；
    /// 跑过一趟，那两支照旧派得出来。
    #[test]
    fn the_key_table_leaves_out_the_keys_that_go_nowhere_right_now() {
        let mut session = Session::new();
        // 一趟都没跑过：那两支在按键表这一头就不派。
        assert_eq!(session.action(Key::Char('e')), Action::Ignored);
        assert_eq!(session.action(Key::Char('i')), Action::Ignored);

        session.press(Key::Char('?'));
        let table = session.key_table();
        assert!(
            !listed(&table, Key::Char('e')),
            "没跑过时展开那个键还在表上"
        );
        assert!(
            !listed(&table, Key::Char('i')),
            "没跑过时前提那个键还在表上"
        );
        // `q` 到处都是同一件事（那一道归组没变），而**此刻这一块上它按不动**。
        assert!(session.means_the_same_everywhere(Key::Char('q')));
        assert_eq!(session.action(Key::Char('q')), Action::Ignored);
        assert!(!listed(&table, Key::Char('q')), "覆盖层掀着时 q 还在表上");
        // `Ctrl-C` 照旧在：它在这一块上也是退出会话。
        assert!(listed(&table, Key::Interrupt), "Ctrl-C 不在表上了");
        // 掀着的那一张自己那个键同理：按回去是关掉，不是再掀一张。
        assert!(!listed(&table, Key::Char('?')), "掀着的那一张自己还在表上");

        // 关掉之后：`q` 照旧退得出去。
        session.press(Key::Esc);
        assert_eq!(session.action(Key::Char('q')), Action::Quit);

        // 跑过一趟之后：那两支照旧派得出来。
        session.run_started();
        session.run_finished();
        assert_eq!(session.action(Key::Char('e')), Action::Expand);
        assert_eq!(
            session.action(Key::Char('i')),
            Action::Reveal(Overlay::Premises)
        );
        session.press(Key::Char('?'));
        let table = session.key_table();
        assert!(listed(&table, Key::Char('e')), "跑过之后展开那个键不在表上");
        assert!(listed(&table, Key::Char('i')), "跑过之后前提那个键不在表上");
    }

    /// 这张表上有没有这个键。
    fn listed(table: &[(KeyGroup, Vec<(Key, Action)>)], key: Key) -> bool {
        table
            .iter()
            .any(|(_, keys)| keys.iter().any(|(listed, _)| *listed == key))
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
        session.go_to(Field::Fit);
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
        assert_eq!(session.focus(), &Focus::Config, "套完没回到配置上");

        // 套一份**什么都没说的**：上一次点过的那几项跟着回到「没说」，不是留在原处。
        session.pick(vec!["空的".to_owned()], presets_file());
        session.took("空的", crate::preset::Preset::default());
        assert_eq!(session.taste.bit_depth, None);
        assert!(session.shown(Field::Filter).starts_with("默认"));
        assert_eq!(session.scope, scope);
    }

    /// **删掉一份之后这一栏还站得住**：名字出清单，光标落在它下面那一份上。
    ///
    /// 删到最后一份也不塌：末尾那一行（＋ 存成一份新的）恒在，光标退到它上面——
    /// 这一栏因此永远有一条出路。真去动盘的那一半在 `super::erase_preset`。
    #[test]
    fn erasing_a_preset_takes_it_out_of_the_column_and_keeps_the_cursor_in_range() {
        let mut session = Session::new();
        // 清单按字典序，与盘上那一侧给的次序同一个（`preset::names`）。
        session.pick(
            vec!["杂志".to_owned(), "漫画".to_owned(), "画集".to_owned()],
            presets_file(),
        );
        session.press(Key::Down);
        assert_eq!(
            session.picking().expect("那一栏开着").picked(),
            Some("漫画")
        );

        session.erased("漫画");

        let picker = session.picking().expect("删完还在这一栏上");
        assert_eq!(picker.names(), ["杂志", "画集"]);
        // 光标停在原来那一格上：接着往下看的是清单上它下面那一份。
        assert_eq!(picker.picked(), Some("画集"));
        assert!(
            session.notice().is_some_and(|said| said.contains("漫画")),
            "{:?}",
            session.notice()
        );

        // 删到最后一份：清单空了，光标退到末尾那一行上（那一行不是一份预设）。
        session.erased("画集");
        session.erased("杂志");
        let picker = session.picking().expect("删完还在这一栏上");
        assert!(picker.names().is_empty());
        assert_eq!(picker.at(), 0);
        assert_eq!(picker.rows(), 1);
        assert_eq!(picker.picked(), None);
        assert_eq!(session.action(Key::Char('d')), Action::Ignored);
    }

    /// **展开把左栏收起，收起把它原样还回来**（票面第三条）。
    ///
    /// 收起不是删掉：三层一格没动，光标还停在原处——那正是「一键回到配置」的意思。
    ///
    /// **收起回的是报告区**（ADR 0017）：展开是从那一块进去的，退一步该退到刚才站的
    /// 地方去。再一个 `⇥` 才回左栏——那一下之后，会话与展开之前**逐格相同**。
    #[test]
    fn collapsing_gives_back_everything_expanding_took_away() {
        let mut session = Session::new();
        session.scope.volumes.push(Picked {
            path: PathBuf::from("卷一"),
            on: true,
        });
        session.go_to(Field::Volume(0));
        session.taste.bit_depth = Some(BitDepth::Four);
        let before = session.clone();

        // 展开：报告区摊开第一卷，左栏这一刻不在屏上（画法那一侧，见 `super::draw`）。
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(0)));
        assert_eq!(
            session.expansion().map(|expansion| expansion.volume),
            Some(Volume::Settled(0))
        );
        // 三层在展开着的时候一格都没动——收起来的东西还在原处。
        assert_eq!(session.taste.bit_depth, Some(BitDepth::Four));

        // 收起：一个键（`Esc` 或 `e`）**退一级**回到那一枝的卷表，再一下回目录表，
        // `⇥` 再回左栏——那一串之后会话与展开之前逐格相同
        // （`volume-discovery/08` 票面第二条：两级展开各有一个键回得去）。
        assert_eq!(session.press(Key::Esc), Exit::Stay);
        assert!(session.expansion().is_none(), "收起之后还展开着");
        assert_eq!(
            session.focus(),
            &Focus::Opened(PathBuf::from("库")),
            "收起没退回展开进来的那一枝"
        );
        assert_eq!(session.press(Key::Esc), Exit::Stay);
        assert_eq!(session.focus(), &Focus::Report, "再一下没回目录表");
        assert_eq!(session.press(Key::Tab), Exit::Stay);
        assert_eq!(session, before, "收起之后会话与展开之前不一样了");

        // 另一个键也收得起来：`e` 是展开那个键按回去。
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(1)));
        assert_eq!(session.press(Key::Char('e')), Exit::Stay);
        assert_eq!(session.focus(), &Focus::Opened(PathBuf::from("库")));
        session.press(Key::Esc);
        session.press(Key::Tab);
        assert_eq!(session, before);
    }

    /// 两卷收摊了的一趟。**光标走的就是这一列**（[`Live::volumes`]）。
    fn two_volumes_in() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(3, 3000);
        for name in ["卷一", "卷二"] {
            live.volume_started(Path::new(name), 1000);
            live.volume_finished(&fixture::skipped_volume(name, 10));
        }
        live
    }

    /// **报告区那个光标选得动一卷，而跟随一动就停**（票面第二、三条）。
    ///
    /// 四件事一条问齐：跟随着的时候光标是**算出来的**（一卷收摊它就跟着走一卷）；
    /// 挪一下跟随就停了，报告再长它也不动；两头都绕得回去（与左栏那一列同一条）；
    /// `g` 把它交回给跟随。
    ///
    /// **挪一卷是展开一枝之后那一级的事**（`volume-discovery/08`）：默认那一副是
    /// 目录表，`↑↓` 在那一级上挪的是一枝。夹具里那几卷都躺在同一个目录底下，
    /// 因此先展开那一枝再问。
    #[test]
    fn the_report_cursor_picks_a_volume_and_stops_following_the_moment_it_moves() {
        let mut live = two_volumes_in();
        let mut session = Session::new();
        session.run_started();
        session.open(PathBuf::from("库"));

        // 跟随：光标恒是最新收摊的那一卷，没有一处记着「停在第几卷」。
        assert_eq!(session.follow(), Follow::Latest);
        assert_eq!(session.standing(&live), Some(Volume::Settled(1)));

        // 挪一下：跟随停了，停在挪到的那一卷上。
        session.select(&live, Step::Back);
        assert_eq!(session.follow(), Follow::Stopped(Volume::Settled(0)));

        // 报告再长它也不动——而跟随着的时候它会跟过去，两者的分别就在这里。
        live.volume_started(Path::new("卷三"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷三", 10));
        assert_eq!(session.standing(&live), Some(Volume::Settled(0)));

        // 两头都绕回去：头一卷再往前一格是末一卷。
        session.select(&live, Step::Back);
        assert_eq!(session.follow(), Follow::Stopped(Volume::Settled(2)));
        session.select(&live, Step::Next);
        assert_eq!(session.follow(), Follow::Stopped(Volume::Settled(0)));

        // `g` 回到跟随：光标交回给最新那一卷，而且从此又跟着走。
        session.act(Action::Follow);
        assert_eq!(session.follow(), Follow::Latest);
        assert_eq!(session.standing(&live), Some(Volume::Settled(2)));

        // 一卷都没有的那一趟：光标指不着谁，挪也挪不动，而这不是错。
        let empty = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        assert_eq!(session.standing(&empty), None);
        session.select(&empty, Step::Next);
        assert_eq!(session.follow(), Follow::Latest, "一卷都没有却停了跟随");
    }

    /// **目录表上挪的是一枝，展开一枝之后挪的才是一卷**（`volume-discovery/08`
    /// 票面第二条）。
    ///
    /// 屏上那个光标**恒是一卷**（`CONTEXT.md` 的《会话》：跟随）：目录那一级只是把它
    /// 归到一行上，挪一枝就是落到相邻那一枝的**头一卷**上。
    ///
    /// **一卷都停不住的那一枝跳过去**：那几卷全没做成，连一份卷报告都没有——
    /// 与卷表上没做成那几行停不上去是同一条规矩。
    #[test]
    fn the_cursor_moves_by_branch_on_the_directory_table_and_by_volume_inside_one() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(4, 4000);
        for name in ["甲/第1话", "甲/第2话", "乙/第1话"] {
            live.volume_started(Path::new(name), 1000);
            live.volume_finished(&fixture::skipped_volume(name, 10));
        }
        live.volume_failed(Path::new("库/丙/没做成"), "卷根不在了");
        let mut session = Session::new();
        session.run_started();

        // 三枝：甲（两卷）、乙（一卷）、丙（一卷都停不住）。
        assert_eq!(live.branches().len(), 3);
        // 跟随着：光标停在最新收摊的那一卷上，也就是乙那一枝。
        assert_eq!(session.standing(&live), Some(Volume::Settled(2)));

        // 目录表上往前一枝：落到甲那一枝的**头一卷**上，不是它的末一卷。
        session.select(&live, Step::Back);
        assert_eq!(session.follow(), Follow::Stopped(Volume::Settled(0)));
        // 再往前一枝：两头绕回去，而丙那一枝**跳过去了**——它一卷都停不住。
        session.select(&live, Step::Back);
        assert_eq!(
            session.follow(),
            Follow::Stopped(Volume::Settled(2)),
            "光标停到了一卷都停不住的那一枝上"
        );

        // 展开甲那一枝：这一级挪的是**它底下那几卷**，转的圈也只有这一枝。
        session.open(PathBuf::from("库/甲"));
        session.follow_along();
        session.select(&live, Step::Next);
        assert_eq!(
            session.follow(),
            Follow::Stopped(Volume::Settled(0)),
            "跟随着的那一卷不在这一枝上时该落到这一枝的末一卷再往下转一格"
        );
        session.select(&live, Step::Next);
        assert_eq!(session.follow(), Follow::Stopped(Volume::Settled(1)));
        session.select(&live, Step::Next);
        assert_eq!(
            session.follow(),
            Follow::Stopped(Volume::Settled(0)),
            "转出了这一枝"
        );
    }

    /// **只有一枝时，目录表上按 `↑↓` 一格不动，跟随也不停**（`volume-discovery/08`）。
    ///
    /// 点名一个目录跑就是这一档，仓库里的夹具也全是它。转一圈回到原地，屏上分毫不变，
    /// 而把跟随停掉是**看不见的后果**——从此新卷收摊光标不再跟着走
    /// （`CONTEXT.md` 的《跟随》：光标一挪跟随就停了，而这一下压根没挪）。
    #[test]
    fn one_branch_alone_neither_moves_the_cursor_nor_stops_the_follow() {
        let live = two_volumes_in();
        let mut session = Session::new();
        session.run_started();
        assert_eq!(live.branches().len(), 1, "夹具不止一枝");

        session.select(&live, Step::Next);
        assert_eq!(session.follow(), Follow::Latest, "一枝独走却停了跟随");
        session.select(&live, Step::Back);
        assert_eq!(session.follow(), Follow::Latest);
        // 跟随照旧跟着走：又收摊一卷，光标落到它上面。
        let mut longer = live.clone();
        longer.volume_started(Path::new("卷三"), 1000);
        longer.volume_finished(&fixture::skipped_volume("卷三", 10));
        assert_eq!(session.standing(&longer), Some(Volume::Settled(2)));
    }

    /// **展开着一枝时，光标收在这一枝底下**（`volume-discovery/08`）。
    ///
    /// 跟随着的时候最新那一卷随时可能落到**另一枝**上，而屏上摆的是这一枝的卷表：
    /// 不收的话，那一格一行都不反白，而 `⏎` 展开的会是屏上根本没有的那一卷。
    #[test]
    fn the_cursor_stays_inside_the_branch_that_is_open() {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(3, 3000);
        for name in ["甲/第1话", "甲/第2话"] {
            live.volume_started(Path::new(name), 1000);
            live.volume_finished(&fixture::skipped_volume(name, 10));
        }
        let mut session = Session::new();
        session.run_started();
        session.open(PathBuf::from("库/甲"));
        assert_eq!(session.standing(&live), Some(Volume::Settled(1)));

        // 下一卷收摊在**另一枝**上：跟随着的那一卷跑到了乙，而屏上摆的是甲的卷表。
        live.volume_started(Path::new("乙/第1话"), 1000);
        live.volume_finished(&fixture::skipped_volume("乙/第1话", 10));
        assert_eq!(live.volumes().last().copied(), Some(Volume::Settled(2)));
        assert_eq!(
            session.standing(&live),
            Some(Volume::Settled(1)),
            "光标漂到了另一枝上"
        );

        // 收起回目录表之后它照旧跟着最新那一卷走——收的只是「展开着那一枝」那一刻。
        session.act(Action::Collapse);
        assert_eq!(session.standing(&live), Some(Volume::Settled(2)));
    }

    /// **决策点上那一卷停得住、也展得开**（spec 的《焦点与两维模式》第五条）。
    ///
    /// 它停在**攒着的那一份**上、不在收摊了的那几卷里，而 `p2-loose-ends/08` 记着
    /// 「不许摊开上一卷冒充它」。展开的索引因此从「报告上第几卷」改成 [`Volume`]——
    /// 一个下标根本指不到这一卷上。
    ///
    /// 它收摊之后**光标跟着它走**：那一刻它正是收摊了的最后一卷（[`Live::nearest`]）。
    #[test]
    fn the_volume_waiting_at_the_decision_point_is_one_the_cursor_can_stand_on() {
        let summarized = fixture::processed_volume("卷二", None);
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(2, 2000);
        live.volume_started(Path::new("卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 10));
        live.volume_started(Path::new("卷二"), 1000);
        live.pass_started(tonefit::Pass::Second, Some(&summarized));

        // 表上停得住的是两卷：收摊了的那一卷，加上攒着的那一份——
        // 而那一份的身份带着**它前面收摊了几卷**（见 [`Volume::Summarized`]）。
        let waiting = Volume::Summarized { after: 1 };
        assert_eq!(live.volumes(), [Volume::Settled(0), waiting]);

        let mut session = Session::new();
        session.run_started();
        session.at_the_decision_point(true);
        session.open(PathBuf::from("库"));
        // 跟随停在**攒着的那一份**上，不是收摊了的最后一卷。
        assert_eq!(session.standing(&live), Some(waiting));

        // 停在它上面，然后它收摊了：光标**跟着它自己走**——它此刻正是收摊了的第 1 卷，
        // 而那个 `after` 就是认出这件事的凭据。
        session.select(&live, Step::Next);
        session.select(&live, Step::Next);
        assert_eq!(session.follow(), Follow::Stopped(waiting));
        live.volume_finished(&summarized);
        assert!(live.summarized().is_none(), "收摊了那一份还摆着");
        assert_eq!(session.standing(&live), Some(Volume::Settled(1)));

        // **下一卷停到决策点上时它不许把光标带走**：那一份此刻是另一卷
        // （`after` 是 2，不是 1），而光标还钉在刚才那一卷身上。
        live.volume_started(Path::new("卷三"), 1000);
        live.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::processed_volume("卷三", None)),
        );
        assert_eq!(live.volumes().len(), 3, "新的决策点没进表");
        assert_eq!(
            session.standing(&live),
            Some(Volume::Settled(1)),
            "光标被「攒着的那一份」那个位置带到下一卷上去了"
        );
    }

    /// **两维各改各的**（ADR 0017）：切焦点不动阶段，收场了不动焦点。
    ///
    /// 票面第四条与第五条一条问齐：**三层只读由阶段那一维说了算**——焦点落到报告区上
    /// 一个改动键都不解锁；**按停在报告区上照样按得动，两级语义不变**——
    /// 它问的是「这一趟还走不走」，与眼下在看什么无关。
    #[test]
    fn the_two_dimensions_move_one_at_a_time() {
        let mut session = Session::new();
        session.go_to(Field::Filter);
        // 没跑过的时候左栏改得动，而 `⇥` 不派——报告区里连一卷都没有。
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
        assert_eq!(session.action(Key::Tab), Action::Ignored);

        session.run_started();
        assert!(session.stage().read_only(), "跑起来了却不是只读");
        session.press(Key::Tab);
        // 焦点换了，阶段一格没动。
        assert_eq!(session.focus(), &Focus::Report);
        assert!(matches!(session.stage(), Stage::Running(_)));
        // 三层照旧改不动：改动键归左栏，而左栏此刻一个改动键都不派。
        assert_eq!(session.action(Key::Left), Action::Ignored);

        // 按停在这一块上照样按得动，而且照旧是两级（ADR 0013：中止是再按一次）。
        assert_eq!(session.stopping(), Instruction::Continue);
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Finish);
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Abort);
        assert_eq!(
            session.action(Key::Char('s')),
            Action::Ignored,
            "闩到了顶还派得出按停"
        );

        // 收场了：阶段那一维换了，**焦点原样留在报告区**——正在读的东西不该被搬走。
        session.run_finished();
        assert_eq!(session.stage(), Stage::Ended);
        assert_eq!(session.focus(), &Focus::Report);
        assert!(!session.stage().read_only());
        // 回左栏：三层又改得动了，而光标还停在原来那一行上。
        session.press(Key::Tab);
        assert_eq!(session.focus(), &Focus::Config);
        assert_eq!(session.field(), Field::Filter);
        assert_eq!(session.action(Key::Left), Action::Cycle(Step::Back));
    }

    /// **逐页表上那个光标翻得动，两头都收得住**（`p3-session-legibility/11`）。
    ///
    /// 往上收在零——零就是这一副列出来的头一页。往下那一头由画法那一层每帧收一次：
    /// 只有它知道这一副此刻列着几页（换一副列法、那一卷又长出几页，两处都会变）。
    ///
    /// **两头不转圈**：这一副是一张两百页的长表，从末一页一下转回头一页会让
    /// 「翻到底了」在屏上没有落点（与三层那几个取值环不同，见 [`Session::move_cursor`]）。
    #[test]
    fn the_cursor_on_the_per_page_table_moves_a_page_at_a_time_and_stops_at_both_ends() {
        let mut session = Session::new();
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(0)));

        // 展开那一下停在头一页上，往上翻不过它。
        assert_eq!(session.expansion().expect("展开着").at, 0);
        for _ in 0..5 {
            session.press(Key::Up);
        }
        assert_eq!(
            session.expansion().expect("展开着").at,
            0,
            "往上翻过了头一页"
        );

        // 往下翻一页是一页。
        for _ in 0..3 {
            session.press(Key::Down);
        }
        assert_eq!(session.expansion().expect("展开着").at, 3);

        // 画法那一层每帧把它收进这一副真列出来的那几页里。
        session.clamp_report(2);
        assert_eq!(
            session.expansion().expect("展开着").at,
            1,
            "翻过了头没被收回来"
        );
        // 一页都没列出来时收到零：那一格里摆的是一句话，没有一页停得上去。
        session.clamp_report(0);
        assert_eq!(session.expansion().expect("展开着").at, 0);

        // **换一副列法，光标跟着回到头一页**：两副列的不是同一批页。
        session.press(Key::Down);
        session.press(Key::Char('a'));
        let expansion = session.expansion().cloned().expect("展开着");
        assert_eq!(expansion.listing, Listing::All);
        assert_eq!(expansion.at, 0, "换了一副列法，光标却还停在原来那个数上");

        // 没展开的时候收不出事来。
        session.press(Key::Esc);
        session.clamp_report(0);
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
        assert_eq!(session.focus(), &Focus::Config, "没跑着也进了等答话");
        assert!(!session.deciding());

        session.run_started();
        // 第一遍里按了一次停：闩记着收尾。
        session.press(Key::Char('s'));
        assert_eq!(session.stopping(), Instruction::Finish);

        // 决策点到了：换一副样子，闩原样带过去。
        session.at_the_decision_point(true);
        assert!(matches!(session.stage(), Stage::Deciding(_)));
        assert_eq!(session.stopping(), Instruction::Finish, "转过去把闩弄丢了");

        // 答一个收尾：状态当场转回「跑着」，而闩仍旧是那一级——答话不是升闩。
        assert_eq!(session.press(Key::Char('s')), Exit::Stay);
        assert!(
            matches!(session.stage(), Stage::Running(_)),
            "答完话没转回去"
        );
        assert_eq!(session.stopping(), Instruction::Finish, "答话把闩推上去了");

        // **「剩下的卷都这样」同样不动那个闩**（`volume-discovery/07` 票面第四条）：
        // 它是个可以是「继续」的粘性答案，记在观察者那一侧的默认答案上，
        // 而按停按到的那一级答完话照旧作数。
        session.at_the_decision_point(true);
        assert!(session.deciding());
        assert_eq!(session.press(Key::Char('a')), Exit::Stay);
        assert!(
            matches!(session.stage(), Stage::Running(_)),
            "答完话没转回去"
        );
        assert_eq!(
            session.stopping(),
            Instruction::Finish,
            "「剩下的卷都这样」把闩推上去了"
        );

        // 那一趟停在决策点上被中止时收不到「跑完」以外的东西，收场那一下照样回浏览。
        session.at_the_decision_point(true);
        assert!(session.deciding());
        session.run_finished();
        assert_eq!(session.focus(), &Focus::Config, "等答话时收场没回浏览");
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
        assert!(matches!(session.stage(), Stage::Running(_)));

        // 那一趟收场：回到浏览，配置又改得动，闩跟着这一趟一起走。
        session.run_finished();
        assert_eq!(session.focus(), &Focus::Config);
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
        session.go_to(Field::GrayLevels);
        session.press(Key::Enter);
        assert!(matches!(session.focus(), Focus::Editing(_)));

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
            session.go_to(field);
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

    /// 左栏上**摊开的是一个取值环**的那几行。
    ///
    /// 「哪几行摊得开」如今**就是[取值是环](Shape::Cycle)那一条本身**——它是那条谓词的
    /// 出处，`p3-session-legibility/05` 那个 `Field::unfolds` 包一层的写法本票撤掉了
    /// （型号放开之后它恒等于这一条）。这里再问一次 [`Field::drills`] 把**型号那一行摘出去**
    /// ——它摊开的是面板、`→` 再下钻一层，与环那一路不是同一个形状；
    /// 下面这几条问的是环那一路，型号那两层另有几条问它
    /// （[`the_model_row_unfolds_into_panels`] 一带）。
    fn unfoldable() -> Vec<Field> {
        Session::new()
            .rows()
            .into_iter()
            .filter(|field| field.shape() == Shape::Cycle && !field.drills())
            .collect()
    }

    /// 摊开一行，把那一列取值取回来。
    fn unfolded(field: Field) -> Values {
        let mut session = Session::new();
        session.go_to(field);
        session.press(Key::Enter);
        session.valuing().expect("没摊开").clone()
    }

    /// **摊得开的那几行都摊得开，而摊开那一列就是那一行的取值环**（票面第一条）。
    ///
    /// 一列至少两格：只有一个取值的行摊开来没有意义，而那种行本仓库一个都没有
    /// （见 `the_value_rings_come_back_around`）。
    ///
    /// **一列一列不是手抄的**：这一条走一遍环、与摊开那一列逐格对，
    /// 环上加一个取值而摊开那一列没跟着多一格的话，这里当场红。
    #[test]
    fn every_row_whose_value_is_a_ring_unfolds_into_that_ring() {
        let rows = unfoldable();
        assert_eq!(rows.len(), 9, "摊得开的行数变了：{rows:?}");

        for field in rows {
            let cells = unfolded(field).cells;
            assert!(cells.len() >= 2, "{field:?} 摊开来只有一格：{cells:?}");

            // 就地转一圈，一格一格与摊开那一列对。
            let mut session = Session::new();
            session.go_to(field);
            for cell in &cells {
                assert_eq!(
                    &session.shown(field),
                    cell,
                    "{field:?} 摊开那一列与环对不上"
                );
                session.press(Key::Right);
            }
            // 转完一圈落回出发点：那一列摊的正是一整圈，不多一格也不少一格。
            assert_eq!(session.shown(field), cells[0], "{field:?} 摊的不是一整圈");
        }
    }

    /// **摊开的第一格是「没说」那一格，此刻生效的那一格另有一个记号**（票面第二条）。
    ///
    /// 两件事分得开：第一格恒是「没说」（它跟着默认走，存成预设时那一项不写进去），
    /// 而 [`Values::chosen`] 指的是**此刻生效的**那一格——两者只在这一行还没说过话时
    /// 落在同一格上。
    ///
    /// **「没说」那一格印成什么，照那一行自己的写法**（[`Session::shown`]）：
    /// 十行里八行印成`默认（…）`，位深与抖动那两行印的是「自动（判据说了算）」——
    /// 那一格照样是「没说」，只是那两行的默认不是一个定值（停车场 Q139）。
    #[test]
    fn the_first_cell_is_the_says_nothing_one_and_the_one_in_effect_is_marked() {
        let fresh = Session::new();
        for field in unfoldable() {
            let values = unfolded(field);
            assert_eq!(
                values.cells()[0],
                fresh.shown(field),
                "{field:?} 的第一格不是「没说」那一格"
            );
            assert!(
                values.cells()[0].starts_with("默认")
                    || matches!(field, Field::BitDepth | Field::Dither),
                "{field:?} 的第一格印成了 {}",
                values.cells()[0]
            );
            // 还没说过话：记号与光标都停在第一格上。
            assert_eq!(values.chosen(), Some(0), "{field:?}");
            assert_eq!(values.at(), 0, "{field:?}");

            // 说了一个值之后：记号跟着挪到那一格上，而第一格还是「没说」。
            let mut session = Session::new();
            session.go_to(field);
            session.press(Key::Right);
            session.press(Key::Enter);
            let said = session.valuing().expect("没摊开");
            assert_eq!(said.chosen(), Some(1), "{field:?} 记号没跟着生效的那一格");
            assert_eq!(said.at(), 1, "{field:?} 光标没落在生效的那一格上");
            assert_eq!(said.cells()[0], fresh.shown(field), "{field:?}");
        }
    }

    /// **`Esc` 从摊开的取值里退出来，那一行一格不改**（票面第三条）。
    ///
    /// 「看一眼有哪些值」不该付出改掉它的代价。走遍摊得开的每一行，
    /// 在那一列上**走到别的格子上去**再退出来——光标挪过窝而取值没动，
    /// 才谈得上「一格不改」。
    ///
    /// 比的是三层的全部内容，不是那一行印出来的那句话：`Esc` 不该改动**任何**一格
    /// （型号那一行换一下会连带清掉标定出来的两个数，那种连带同样落在这个断言里）。
    #[test]
    fn escaping_out_of_the_unfolded_values_changes_not_one_cell() {
        let mut session = Session::new();
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        session.taste.filter = Some(Filter::Bicubic);
        session.taste.crop = Some(false);
        session.scope.out = Some(PathBuf::from("出"));

        for field in unfoldable() {
            let before = session.clone();
            session.go_to(field);
            session.press(Key::Enter);
            // 在那一列上走两格：退出来之后光标停在哪儿不算数，取值才算数。
            session.press(Key::Down);
            session.press(Key::Down);
            let looking = session.valuing().expect("没摊开");
            assert_ne!(
                Some(looking.at()),
                looking.chosen(),
                "{field:?} 光标没挪过窝，这一条就没问到东西"
            );
            session.press(Key::Esc);

            assert_eq!(session.focus(), &Focus::Config, "{field:?} Esc 没退回左栏");
            assert_eq!(session.device, before.device, "{field:?} Esc 改了设备层");
            assert_eq!(session.taste, before.taste, "{field:?} Esc 改了口味层");
            assert_eq!(session.scope, before.scope, "{field:?} Esc 改了范围层");
        }
    }

    /// **摊开选出来的值与就地转出来的值改的是同一格**（票面：两条路改的是同一格）。
    ///
    /// 逐行逐格问一遍：摊开来走到第 k 格按 `⏎`，与从「没说」那一格起就地转 k 下，
    /// 两条路走完之后**两层逐格相同**。两条路各写一份的话，这里当场红。
    ///
    /// **停着的还是生效着的那一格时一步都不走**：第 0 格那一趟两边都是零下，
    /// 而那一趟正是「摊开、什么都不改、按 `⏎`」——它与 `Esc` 一样一格不改。
    #[test]
    fn unfolding_and_turning_in_place_write_the_same_cell() {
        for field in unfoldable() {
            let cells = unfolded(field).cells;
            for (at, cell) in cells.iter().enumerate() {
                // 一条路：摊开来，走到第 at 格，定。
                let mut picked = Session::new();
                picked.go_to(field);
                picked.press(Key::Enter);
                for _ in 0..at {
                    picked.press(Key::Down);
                }
                picked.press(Key::Enter);

                // 另一条路：就地转 at 下。
                let mut turned = Session::new();
                turned.go_to(field);
                for _ in 0..at {
                    turned.press(Key::Right);
                }

                assert_eq!(picked.focus(), &Focus::Config, "{field:?} 定完没回左栏");
                assert_eq!(&picked.shown(field), cell, "{field:?} 第 {at} 格定错了");
                assert_eq!(picked.device, turned.device, "{field:?} 第 {at} 格：设备层");
                assert_eq!(picked.taste, turned.taste, "{field:?} 第 {at} 格：口味层");
            }
        }
    }

    /// 型号那一行摊开的**第一层是面板**，每一行带着那块面板的
    /// 分辨率、PPI、灰阶数、黑白／彩色（`p3-session-legibility/06` 票面第一条）。
    ///
    /// **一行不多、一行不少**：这一列与 [`Profile::devices_by_panel`] 逐格对——
    /// 分组的规矩只有那一处出处（同一张票面第五条），内置表里加一块面板，这一列当场跟着多一格。
    /// **面板有几块不写死在用例里**：票面说的「八块」与实现对不上（停车场 Q141），
    /// 而照票面写死的话，内置表加一个型号就要来改这一条。
    ///
    /// 每一格的字面走 `tonefit::Panel` 自己的 `Display`，会话这一侧不另写一份格式。
    /// 第一格仍是「没挑」那一格，与别的行同一条。
    #[test]
    fn the_model_row_unfolds_into_panels() {
        let groups = Profile::devices_by_panel();
        let mut session = Session::new();
        session.go_to(Field::Profile);
        session.press(Key::Enter);
        let values = session.valuing().expect("型号那一行没摊开");

        assert_eq!(values.field(), Field::Profile);
        assert_eq!(values.panel(), None, "摊开落在第一层上");
        assert_eq!(
            values.cells().len(),
            groups.len() + 1,
            "面板那一层与分组对不上：{:?}",
            values.cells()
        );
        assert_eq!(
            values.cells()[0],
            Session::new().shown(Field::Profile),
            "第一格不是「没挑」那一格"
        );
        for (at, (panel, _)) in groups.iter().enumerate() {
            assert_eq!(
                &values.cells()[at + 1],
                &panel.to_string(),
                "第 {at} 块面板"
            );
            // 每一行四样俱全：分辨率 · PPI · 灰阶数 · 黑白／彩色。
            let printed = &values.cells()[at + 1];
            assert!(printed.contains(&panel.resolution.to_string()), "{printed}");
            assert!(printed.contains(&format!("{} PPI", panel.ppi)), "{printed}");
            assert!(
                printed.contains(&format!("{} 级灰阶", panel.gray_levels)),
                "{printed}"
            );
            assert!(
                printed.contains(if panel.color { "彩色" } else { "黑白" }),
                "{printed}"
            );
        }
    }

    /// **每一块面板都下钻得进去，进去列的正是共用它的那几个型号**
    /// （`p3-session-legibility/06` 票面第二条）。
    ///
    /// 逐块问一遍，包括**底下只有一个型号的那几块**——它们照走下钻这一步
    /// （停车场 Q142）：那一格答的是「这块屏只有这一台设备」，而那是一句有内容的话。
    ///
    /// 定完**回型号那一行**，而定下来的那个名字解析出来的面板就是进去的那一块——
    /// 「设备只是面板的别名」这条走了一遍全程。
    #[test]
    fn drilling_into_a_panel_lists_the_models_that_share_it() {
        let groups = Profile::devices_by_panel();
        for (at, (panel, devices)) in groups.iter().enumerate() {
            let mut session = Session::new();
            session.go_to(Field::Profile);
            session.press(Key::Enter);
            for _ in 0..=at {
                session.press(Key::Down);
            }
            session.press(Key::Right);

            let inside = session.valuing().expect("没下钻").clone();
            assert_eq!(inside.panel(), Some(*panel));
            assert_eq!(
                inside.cells(),
                devices.as_slice(),
                "{panel} 底下的型号对不上"
            );

            for (step, device) in devices.iter().enumerate() {
                let mut picking = session.clone();
                for _ in 0..step {
                    picking.press(Key::Down);
                }
                picking.press(Key::Enter);
                assert_eq!(picking.focus(), &Focus::Config, "定完没回型号那一行");
                assert_eq!(picking.device.profile.as_deref(), Some(*device));
                assert_eq!(
                    Profile::resolve(device).expect("内置型号").panel(),
                    *panel,
                    "{device} 定出来的不是进去的那一块面板"
                );
            }
        }
    }

    /// **进去时光标停在当前型号所在的那块面板／那一个型号上，不是停在表头**
    /// （`p3-session-legibility/06` 票面第四条）。
    ///
    /// 两层各问一次，另加**退一步落回进来的那一块面板上**：退一步该退到刚才站的地方去，
    /// 而不是退到「当前型号的那一块」——进的是别的一块时那两块不是同一块，
    /// 而用户正是从进去的那一块往下看的（那一趟由按键表 `which_keys_do_what_in_which_state`
    /// 的三之三段问，那里当前型号还没挑，两块分得开）。
    #[test]
    fn the_two_levels_open_on_the_model_in_effect() {
        let groups = Profile::devices_by_panel();
        let (at, (panel, devices)) = groups
            .iter()
            .enumerate()
            .find(|(_, (_, devices))| devices.len() > 1)
            .expect("总有一块面板底下不止一个型号");
        let device = devices[1];

        let mut session = Session::new();
        session.device.profile = Some(device.to_owned());
        session.go_to(Field::Profile);
        session.press(Key::Enter);
        let values = session.valuing().expect("没摊开");
        assert_eq!(values.at(), at + 1, "光标没停在当前型号的那块面板上");
        assert_eq!(values.chosen(), Some(at + 1), "记号没画在那块面板前面");

        session.press(Key::Right);
        let inside = session.valuing().expect("没下钻");
        assert_eq!(inside.panel(), Some(*panel));
        assert_eq!(inside.at(), 1, "光标没停在当前型号上");
        assert_eq!(inside.chosen(), Some(1), "记号没画在当前型号前面");

        // 退一步：落回**进来的那一块**面板上——这里它恰好就是当前型号那一块；
        // 进的是别的一块时同样落回它（按键表三之三那一段问的正是那一趟）。
        session.press(Key::Esc);
        let outside = session.valuing().expect("退过头了");
        assert_eq!(outside.panel(), None);
        assert_eq!(outside.at(), at + 1, "没落回进来的那一块面板上");
    }

    /// **摊开挑出来的型号与就地转出来的型号改的是同一格**，
    /// 而两条路都把标定出来的两个数清空（`p3-session-legibility/06` 票面第六条，ADR 0002）。
    ///
    /// 逐个内置型号问一遍：摊开→下钻→定，与从「没挑」那一格起就地转到它，
    /// **两条路走完之后设备层逐格相同**。写型号只有 [`Session::set_device`] 一条路，
    /// 两条路各写一份的话，这里当场红。
    #[test]
    fn picking_a_model_and_turning_in_place_write_the_same_cell() {
        for (at, (_, devices)) in Profile::devices_by_panel().into_iter().enumerate() {
            for (step, device) in devices.iter().enumerate() {
                // 一条路：摊开、下钻、定。
                let mut picked = Session::new();
                picked.device.gray_levels = Some(12);
                picked.device.threshold = Some(5.2);
                picked.go_to(Field::Profile);
                picked.press(Key::Enter);
                for _ in 0..=at {
                    picked.press(Key::Down);
                }
                picked.press(Key::Right);
                for _ in 0..step {
                    picked.press(Key::Down);
                }
                picked.press(Key::Enter);

                // 另一条路：就地一格一格转到它。
                let mut turned = Session::new();
                turned.device.gray_levels = Some(12);
                turned.device.threshold = Some(5.2);
                turned.go_to(Field::Profile);
                while turned.device.profile.as_deref() != Some(*device) {
                    turned.press(Key::Right);
                }

                assert_eq!(picked.device.profile.as_deref(), Some(*device));
                assert_eq!(picked.device, turned.device, "{device} 两条路写得不一样");
                assert_eq!(picked.device.gray_levels, None, "{device}：灰阶数没清空");
                assert_eq!(picked.device.threshold, None, "{device}：阈值没清空");
            }
        }
    }

    /// **`Esc` 从型号那两层退出来，一格不改**（`p3-session-legibility/06` 票面第三条）。
    ///
    /// 两层各问一次，而且**先在那一列上走到别的格子上去**——光标挪过窝而取值没动，
    /// 才谈得上「一格不改」。比的是三层的全部内容：换掉型号会连带清掉标定出来的两个数，
    /// 那种连带同样落在这个断言里。
    #[test]
    fn escaping_out_of_the_two_levels_changes_not_one_cell() {
        let mut session = Session::new();
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        session.device.threshold = Some(5.2);
        let before = session.clone();

        // 面板那一层：走到别的面板上再退。
        session.go_to(Field::Profile);
        session.press(Key::Enter);
        session.press(Key::Down);
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config, "Esc 没退回左栏");
        assert_eq!(session.device, before.device, "面板那一层的 Esc 改了设备层");

        // 下钻那一层：进去、走到别的型号上，再退两下。
        session.press(Key::Enter);
        session.press(Key::Down);
        session.press(Key::Right);
        session.press(Key::Down);
        session.press(Key::Esc);
        session.press(Key::Esc);
        assert_eq!(session.focus(), &Focus::Config, "两下 Esc 没退回左栏");
        assert_eq!(session.device, before.device, "下钻那一层的 Esc 改了设备层");
        assert_eq!(session.taste, before.taste);
        assert_eq!(session.scope, before.scope);
    }

    /// **型号停在内置表外的一个名字上时不崩、也不挂，落回「没挑」**
    /// （`p3-session-legibility/06` 票面第七条，
    /// 停车场 Q140）。
    ///
    /// 那个名字从预设里来：`Session::took` 原样收下 TOML 里写的型号名，
    /// 而那份预设可能是上一版写的、型号后来删了。
    ///
    /// 三件事：
    ///
    /// - **`←` 转得回来**。从前 `back` 一路往前走、走到「再走一步就回到出发点」为止，
    ///   而表外的名字不在环上——那个循环不停。**这一条要是回归，它不是红，是挂住**。
    /// - **`→` 与 `←` 落到同一格**：环外那个值两个方向都只能回到环上，
    ///   而落回哪一格由环自己说了算（`next_device`：落回「没挑」）。
    /// - **摊开来那一列一格都不标**：那个名字不在任何一块面板底下，
    ///   随便点实一格就是在指一个用户没挑过的型号。光标落回「没挑」那一格。
    #[test]
    fn a_model_outside_the_table_falls_back_to_nothing_picked() {
        let outside = || {
            let mut session = Session::new();
            session.device.profile = Some("kobo-glo-hd".to_owned());
            session.go_to(Field::Profile);
            session
        };
        assert!(
            !Profile::devices().any(|device| device == "kobo-glo-hd"),
            "这个名字进了内置表，这一条就没问到东西"
        );

        let mut turning_back = outside();
        turning_back.press(Key::Left);
        assert_eq!(turning_back.device.profile, None, "`←` 没落回「没挑」");

        let mut turning_on = outside();
        turning_on.press(Key::Right);
        assert_eq!(turning_on.device.profile, None, "`→` 没落回「没挑」");

        let mut session = outside();
        session.press(Key::Enter);
        let values = session.valuing().expect("没摊开");
        assert_eq!(values.chosen(), None, "表外的名字不该标在任何一块面板上");
        assert_eq!(values.at(), 0, "光标没落回「没挑」那一格");

        // 就在那一格上定下来：那个名字换成了「没挑」，而这一下走的是同一条写入路径。
        session.press(Key::Enter);
        assert_eq!(session.focus(), &Focus::Config);
        assert_eq!(session.device.profile, None);
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
        session.go_to(Field::GrayLevels);

        session.press(Key::Enter);
        for character in "12".chars() {
            session.press(Key::Char(character));
        }
        session.press(Key::Enter);

        // 没挑型号：留在编辑态，话说得出来。
        assert!(matches!(session.focus(), Focus::Editing(_)));
        assert!(session.notice().expect("要说一句").contains("先挑型号"));
        assert_eq!(session.device.gray_levels, None);

        // 挑了型号之后同一个数收得下，越界的数仍被库那一侧的界挡下。
        session.press(Key::Esc);
        session.device.profile = Some("boox-poke6".to_owned());
        session.go_to(Field::GrayLevels);
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
        assert!(matches!(session.focus(), Focus::Editing(_)), "0 级该被挡下");
        assert_eq!(session.device.gray_levels, Some(12), "挡下的值没有写进去");
    }

    /// 打错的取值留在编辑态，不把用户打的东西丢掉。
    #[test]
    fn a_value_that_does_not_parse_stays_in_the_editor() {
        let mut session = Session::new();
        session.go_to(Field::CacheBudget);

        session.press(Key::Enter);
        for character in "512T".chars() {
            session.press(Key::Char(character));
        }
        session.press(Key::Enter);

        let Focus::Editing(edit) = session.focus() else {
            panic!("解析不过该留在编辑态");
        };
        assert_eq!(edit.buffer, "512T", "用户打的东西被丢掉了");
        assert!(session.notice().is_some(), "解析不过要说一句");
        assert_eq!(session.taste.cache_budget, None);

        // 改对了就收得下，回到浏览。
        session.press(Key::Backspace);
        session.press(Key::Char('M'));
        session.press(Key::Enter);
        assert_eq!(session.focus(), &Focus::Config);
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
        session.go_to(Field::CacheBudget);

        session.press(Key::Enter);
        let Focus::Editing(edit) = session.focus() else {
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
        session.go_to(Field::AddVolume);

        // 打两个卷进来。每打完一个，光标仍停在「再打一个」上。
        for name in ["卷一", "卷二"] {
            session.press(Key::Enter);
            for character in name.chars() {
                session.press(Key::Char(character));
            }
            session.press(Key::Enter);
            assert_eq!(session.field(), Field::AddVolume);
        }
        assert_eq!(session.scope.volumes.len(), 2);
        assert!(session.scope.volumes.iter().all(|volume| volume.on));

        // 勾掉第二个：它还在清单上，只是这一趟不算数。
        session.go_to(Field::Volume(1));
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
        assert_eq!(session.field(), Field::Profile, "转一圈没回到第一行");

        session.press(Key::Up);
        assert_eq!(session.field(), Field::AddVolume, "往上一格没绕到最后一行");
    }
}
