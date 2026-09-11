//! 报告区那三张表的**列**：有哪几列、各多宽、这个宽度上留得下哪几列、一行怎么摆出来
//! （`CONTEXT.md` 的《会话》：卷表、砍列）。
//!
//! 表真画出来是画法那一层的事（`super::draw::table` 与 `super::draw::pages`）；
//! 本模块只答四件事：**列的次序**、**砍列的次序**、**一格摆不下的字怎么省略**、
//! 以及**留下来的那几列摆成一行长什么样**。
//!
//! # 三张表，一套摆法
//!
//! 目录表一枝一行（[`DirectoryColumn`]，`volume-discovery/08`），
//! 展开一枝出来的卷表一卷一行（[`VolumeColumn`]），
//! 展开一卷出来的逐页表一页一行（[`PageColumn`]）——
//! 三张表的**列各不相同，摆法一模一样**：一样按显示宽度对齐、一样按各自那个固定次序砍列、
//! 一样在砍无可砍时收窄名字那一列。摆法因此写在[一个 trait](Column) 上，
//! 各表只交出自己那两个次序（`p3-session-legibility/11`：逐页也是一张表，
//! 与卷表同一套视口、砍列与上色——那三样一样都不另造）。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）：
//! `--no-default-features` 那一趟照编、照跑它自带的用例。同一条理由把
//! [`Viewport`](super::viewport::Viewport) 摆在了那一侧。
//!
//! # 宽度一律是**显示宽度**
//!
//! 中文两格，出处只有 [`crate::wrap::width`]——折行按它折、滚动按它算、
//! 这里按它对齐与省略，三处不许各数各的。
//!
//! **它按 `UnicodeWidthChar::width` 算，不按 `width_cjk`**：东亚宽度表上标着
//! **Ambiguous** 的字形在这里一律当一格。这是仓库既有的约定（`crate::wrap` 那一头也是
//! 它），本模块跟着走，不另立第二套——跟着走的代价由**字形的选法**接住：
//! **摆进列里的字形一个都不许是歧义宽度**，判据、边界与理由都在
//! [`tonefit::width_is_stable`]。
//!
//! 那条规矩管两层：**这一层自己造的字形**（[`ELLIPSIS`] 与三张表的行首记号），
//! 与**措辞那一层摆进列里的那几格**（哪几格由下一节那一维答，不在这里点名）。
//! 后者从前划在规矩外面——换它们是命令行印出去的字节的一次变动，不归画法这一层；
//! `p4-parking-lot/05` 换掉了那两个字形，管辖面跟着扩到那一层（停车场 Q168）。
//!
//! 停车场 Q154 记着这笔账的由来：从前报告是散文，错一格看不出来；表上头一次靠宽度吃饭。
//!
//! # 每一列说得出自己的字面出处
//!
//! 一列的字**是谁写的**——[措辞 · 原样 · 记号](Provenance)三档，逐列写在 [`Column::provenance`] 上
//! （`CONTEXT.md`《格》立的那一维）。三处 `match` 一个 `_` 都不留：
//! **添一列不写这一格，编译就过不去**。
//!
//! 「哪几格要过宽度那一关」因此**只有这一处出处**（`wording_cells` 从它导出）。
//! 从前措辞那一层还手抄着第二份，往表里添一列那一份不跟着添**也不会红**——
//! 逐页表添了五格几何列之后，裁边与缩放两格就是那么漏出去的（停车场 Q188）。

use std::marker::PhantomData;

use crate::render::Field;
use crate::wrap;

/// 列与列之间空几格。**表画出来与列摆不摆得下按同一个数算**，因此在这里。
pub(super) const GAP: usize = 2;

/// 摆不下时省略号那一格：一列的内容从**中间**掐掉一截，留下的两头之间摆它。
///
/// 取 `⋯`（U+22EF）而不是 `…`（U+2026）：后者过不了
/// [`width_is_stable`](tonefit::width_is_stable) 那一关（停车场 Q154）。
/// 省略过的是名字那一列，它右边还可能留着几列。
const ELLIPSIS: char = '⋯';

/// 一列的字**是谁写的**：`CONTEXT.md`《格》立的那一维——**字面出处 (Provenance)**，
/// 三档，一档不多（措辞 · 原样 · 记号，名字逐个取自词汇表）。
///
/// **挂在列上，不挂在格上。** 列与格不是一对一：[记号](Self::Mark)那一列压根不是一格字、
/// 页名那一列的字从另一行补上、[判定](PageColumn::Verdict)那一列对的是**另一个名字**的格
/// （[`Field::Candidate`]）。
///
/// **三档各归一条规矩，而规矩不在这里**——这一维只答「这一格该归谁管」：
/// 措辞归造字面的那一层查字形，原样归[从中间省略](elide)，记号归画它的那一层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provenance {
    /// **措辞 (Wording)**：界面层自己造的字面（尺寸、裁边、缩放、判定、理由、页数、分布……）。
    ///
    /// 字形宽度**必须稳**（[`tonefit::width_is_stable`]），而查它的是**造字面的那一层**——
    /// 带着的正是「查哪一格」：`Some(field)` 是 [`crate::render`] 出的那一格，
    /// 那一层那条用例从 `wording_cells` 拿走全部要查的格；`None` 是画法这一层
    /// 自己造的字，眼下只有[耗时](VolumeColumn::Elapsed)一列，它在自己那一头查。
    Wording(Option<Field>),
    /// **原样 (Verbatim)**：用户的字节原封带过来（目录名、卷名、页名、去处路径、定档页名）。
    ///
    /// 宽度**永远稳不住**——一个带 emoji 的文件名不该把整张表判红——摆不下时归
    /// [从中间省略](elide)管，**不进那一关**。它因此不必报出自己出自哪一格。
    Verbatim,
    /// **记号 (Mark)**：一个字符说完一件事，**不是一格字**
    /// （`CONTEXT.md`《语义色》：颜色不是唯一载体）。
    ///
    /// 三张表的行首记号都在这一档，那几个字形由画它的那一层各自问
    /// （`super::draw::table`、`super::draw::pages`、`super::draw::directories`）。
    Mark,
}

/// **一张表的那几列**：从左到右是哪几列、窄了按什么次序砍、砍无可砍时收窄谁。
///
/// 三张表各实现一份（[`DirectoryColumn`]、[`VolumeColumn`]、[`PageColumn`]），而摆法只有一份
/// （[`fit`]、[`plan`]、[`lay`]）：屏上砍成什么样、对齐成什么样一律问这几个函数，
/// 画法那一层不许再写第二份次序。
pub(super) trait Column: Copy + PartialEq + 'static {
    /// 全部列，**从左到右**。表就按这个次序摆。
    const ALL: &'static [Self];

    /// **砍列的次序**：横向摆不下时按它一列一列舍掉（`CONTEXT.md` 的《会话》：砍列）。
    ///
    /// **只有这一处出处。** 恒在的那几列不在这里边——一行上先要认得出这是哪一行、
    /// 它出没出事，剩下的几列都是这两件之后的事。
    const DROPPED_IN_TURN: &'static [Self];

    /// 砍无可砍仍摆不下时**收窄**的那一列：名字那一列（卷名／页名）。
    ///
    /// 收窄之后摆不下的那几个字[从中间省略](elide)——它恒在，因此没有「一列都不剩」那一档。
    const NARROWED: Self;

    /// 列头。屏上那一行写的就是它。
    ///
    /// 这几个词**命令行上根本没有**：那一路把同一批格摆成一段散文，一个列头都不需要
    /// （见 [`crate::render::plain`]）。列头因此长在会话这一侧，与左栏那几行标签同一条。
    fn head(self) -> &'static str;

    /// 这一列的格**靠右摆**吗。数靠右（一位数与三位数靠左摆就对不齐），词与名字靠左。
    fn to_the_right(self) -> bool;

    /// 这一列的字**是谁写的**（[字面出处](Provenance)）。
    ///
    /// **每一个变体都得自己答**：三处 `match` 一个 `_` 都不留，添一列不写这一格
    /// **编译就过不去**。从前「哪几格要过宽度那一关」在仓库里另有一份手抄的名单，
    /// 添一列不跟着添也不会红——逐页表的裁边与缩放两列就是那么漏出去的（停车场 Q188）。
    fn provenance(self) -> Provenance;

    /// 这一列在 [`Widths`] 里的第几格。
    fn at(self) -> usize {
        Self::ALL
            .iter()
            .position(|column| *column == self)
            .expect("每一列都在 ALL 里")
    }
}

/// **目录表**上的一列（`volume-discovery/08`）：报告区默认那一副，一个目录一行。
///
/// 与[卷表](VolumeColumn)同一个形状——行首记号与名字恒在，其余按一个固定次序砍。
/// 列的选法答的是**这一枝到底怎么样**：从左到右是「这一枝出没出事 · 是哪个目录 ·
/// 几卷 · 判成哪几档」，一路由结论走向明细。
///
/// **进隔离的卷数不占一列**：它跟在行尾，成句（`隔离 2 卷`）——与卷表上那个「隔离」
/// 同一条规矩，摆不下时整行折下去，不塞进格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectoryColumn {
    /// 行首记号：这一枝怎么样，一个字符说完。**恒在。**
    Mark,
    /// 目录名（只印最后那一段）。**恒在**，摆不下时从中间省略。
    Name,
    /// 这一枝底下几卷。没做成的那几卷也算在里面。
    Volumes,
    /// 基准档分布：各档各有几卷，排成一串。
    Bases,
}

impl Column for DirectoryColumn {
    const ALL: &'static [Self] = &[Self::Mark, Self::Name, Self::Volumes, Self::Bases];

    /// **砍列的次序：基准档分布 → 卷数。**
    ///
    /// 记号与目录名不在这里边——它们**恒在**：一行上先要认得出这是哪一枝、它出没出事。
    ///
    /// 分布最先让：它是这张表上最宽的一格，也是比结论深一层的明细；
    /// 卷数压后——「这一枝有多少卷」与目录名一起就已经是一句话。
    const DROPPED_IN_TURN: &'static [Self] = &[Self::Bases, Self::Volumes];

    const NARROWED: Self = Self::Name;

    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "目录",
            Self::Volumes => "卷数",
            Self::Bases => "基准档分布",
        }
    }

    /// 只有卷数靠右：它是个数，一位数与三位数靠左摆就对不齐，而「这一枝比别的枝厚多少」
    /// 正是扫一眼要看出来的（与卷表的页数同一条）。
    fn to_the_right(self) -> bool {
        matches!(self, Self::Volumes)
    }

    fn provenance(self) -> Provenance {
        match self {
            Self::Mark => Provenance::Mark,
            // 印的是**全路径**（见 `super::draw::directories`），与命令行那一副的
            // [`Field::Source`] 说的是同一个身份。
            Self::Name => Provenance::Verbatim,
            Self::Volumes => Provenance::Wording(Some(Field::VolumeCount)),
            Self::Bases => Provenance::Wording(Some(Field::Bases)),
        }
    }
}

/// **卷表**上的一列。屏上从左到右就是 [`Column::ALL`] 那个次序。
///
/// **行首记号也是一列**：它与别的列一样要对齐、要量宽度，把它排除在外只会让画法那一层
/// 自己再算一遍它占几格。它与卷名一起是砍不掉的那两列。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VolumeColumn {
    /// 行首记号：这一卷怎么样，一个字符说完。**恒在。**
    Mark,
    /// 卷名。**恒在**，摆不下时从中间省略（见 [`elide`]）。
    Name,
    /// 输出页数。
    Pages,
    /// 档位分布（`CONTEXT.md` 的《档位分布》），或者这一卷为什么一页都没判（跳过、没做成）。
    /// **一张灰度页都没有的卷这一格不在场**（`two-pass-rework/02`）。
    Tally,
    /// 定档页：这一卷的档位是哪一页定出来的。**只在 `--envelope` 那条路上在场**——
    /// 默认逐页那一趟整列不在（[`fit`] 的第零步让掉它，连列头都不占）。
    Driver,
    /// 这一卷做了多久。
    Elapsed,
}

impl Column for VolumeColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Pages,
        Self::Tally,
        Self::Driver,
        Self::Elapsed,
    ];

    /// **砍列的次序：耗时 → 定档页 → 页数 → 档位分布。**
    ///
    /// 记号与卷名不在这里边——它们**恒在**：一行上先要认得出这是哪一卷、它出没出事。
    ///
    /// 次序按「摆不下时先舍谁」排：耗时最先——它是这一卷做完之后的一个旁证；
    /// 定档页次之——追下去要展开那一卷才看得清；页数再次——它是这一卷有多厚，
    /// 与卷名一起就已经是一句话；**档位分布压后**——它是这张表要答的那件事，
    /// 但它也是这张表上最宽的一格（两档就二十格，`two-pass-rework/02`），
    /// 留着它去收窄卷名，卷名就只剩一个省略号，那一行连是哪一卷都认不出了。
    /// 它让掉之后跳过与没做成那两个词跟着丢，而行首记号说的是同一件事
    /// （`super::draw::table::Mark`）：靠得住的载体是记号，不是这一格。
    /// 从前这一列写的是基准档、一格最宽十二格，那时它恒在、砍无可砍只收窄卷名；
    /// 分布那一格宽出一倍，这一条重新想过，结论翻了。
    const DROPPED_IN_TURN: &'static [Self] =
        &[Self::Elapsed, Self::Driver, Self::Pages, Self::Tally];

    const NARROWED: Self = Self::Name;

    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "卷名",
            Self::Pages => "页数",
            Self::Tally => "档位分布",
            Self::Driver => "定档页",
            Self::Elapsed => "耗时",
        }
    }

    /// 只有页数靠右：它是个数，一位数与三位数靠左摆就对不齐，而「这一卷比别的卷厚多少」
    /// 正是扫一眼要看出来的。其余各列都是词或名字，靠左。
    fn to_the_right(self) -> bool {
        matches!(self, Self::Pages)
    }

    fn provenance(self) -> Provenance {
        match self {
            Self::Mark => Provenance::Mark,
            Self::Name => Provenance::Verbatim,
            Self::Pages => Provenance::Wording(Some(Field::PageCount)),
            // **有判定的卷就是这一格**；跳过与没做成那两个词由 [`crate::render::tally_column`]
            // 就地写出、不占一格，而它们逐条进了目录那一行的[分布](Field::Bases)——
            // 那一关因此照旧问得到（停车场 Q377）。
            Self::Tally => Provenance::Wording(Some(Field::Tally)),
            // 定档页那一格是**一条路径的最后一段**（见 `super::draw::table::driver`）。
            Self::Driver => Provenance::Verbatim,
            // **这一列的字是画法那一层自己造的**（`super::draw::overview::spell`）：
            // 措辞那一层没有它那一格，字形因此在那一头问。
            Self::Elapsed => Provenance::Wording(None),
        }
    }
}

/// **逐页表**上的一列（`p3-session-legibility/11`）：展开一卷之后那一副。
///
/// 与[卷表](VolumeColumn)同一个形状——行首记号与名字恒在，其余按一个固定次序砍。
/// 列的选法答的是**展开一卷要问的那一件事**：哪一页把整卷的档位拉下来。
/// 因此从左到右是三段：**这一页是谁**（记号 · 页名）、**它这个样子是怎么来的**
/// （尺寸 · 裁边 · 缩放 · 跨页 · 彩页转灰）、**它判成哪一档、凭什么**
/// （判定 · 理由 · 判据），末一格是**去处**——一路由结论走向证据，最后落到盘上那个文件。
///
/// **十一列一格不少地对着 [`crate::render::pages`] 出的那几格**
/// （`p4-parking-lot/10`，收停车场 Q162）：几何那五格从前留在表外，跟着丢掉的还有
/// 失败页那一句「它的尺寸是卷内统一尺寸」（`p1-session/11` 的验收）。
/// 窄屏上它们由[砍列](Self::DROPPED_IN_TURN)让位，而那是**摆不下**，不是不给。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageColumn {
    /// 行首记号：这一页要不要紧，一个字符说完。**恒在。**
    Mark,
    /// 页名（成员名，只印最后那一段）。**恒在**，摆不下时从中间省略。
    Name,
    /// 这一页的输出尺寸。
    Size,
    /// 裁边裁掉了多少。**一个像素都没裁就不在场。**
    Crop,
    /// 缩放怎么算的；**失败页说的是它的尺寸从哪来**（`p1-session/11` 的验收）。
    Scaling,
    /// 跨页切出来的哪一半。**不是切出来的就不在场。**
    Cut,
    /// 这一页是彩页转灰。**不是就不在场。**
    ColorToGray,
    /// 这一页判成的那一档。彩色分支与失败页没有这一格。
    Verdict,
    /// 判成这一档的理由。
    Reason,
    /// 各候选的判据值排成一串。
    Scores,
    /// 去处：这一页写到哪个文件。
    Output,
}

impl Column for PageColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Size,
        Self::Crop,
        Self::Scaling,
        Self::Cut,
        Self::ColorToGray,
        Self::Verdict,
        Self::Reason,
        Self::Scores,
        Self::Output,
    ];

    /// **砍列的次序：去处 → 裁边 → 跨页 → 彩页转灰 → 判据 → 尺寸 → 缩放 → 理由。**
    ///
    /// 记号、页名与判定不在这里边——**判成哪一档就是这一副要答的那件事**，
    /// 而先要认得出这是哪一页、它要不要紧。
    ///
    /// 次序按「摆不下时先舍谁」排，而这一副要答的那一问是**哪一页把整卷拉下来**：
    ///
    /// 1. **去处**最先——它是一整条路径，这张表上最宽的一格，而「这一页落到哪儿」
    ///    与那一问离得最远：卷的去处在卷级那一行上，页名与它凑起来就是这一格。
    /// 2. **裁边 → 跨页 → 彩页转灰**——几何那三格说的是「这一页的形状是怎么来的」，
    ///    比判定隔着一层；三格之间按宽窄让（裁边是一对尺寸，另两格各一个词）。
    ///    多数卷这三列一格都不在场，那时它们由 [`fit`] 的第零步先让掉、根本轮不到这里。
    /// 3. **判据**——它是证据，比结论深一层，也是剩下几列里最宽的一格。
    /// 4. **尺寸**——宽溢出与兜底那两件事行尾那个词已经说了。
    /// 5. **缩放**——它是几何那一组里**唯一压到最后的**，只为一件事：失败页那一行靠它
    ///    说出「它的尺寸是卷内统一尺寸」（`p1-session/11` 的验收，停车场 Q162
    ///    记着它丢过一次）。那一句在别处一个字都没有。
    /// 6. **理由**压后——它一个词就说清「这一档是怎么来的」，与判定挨着才读得懂。
    ///
    /// **80×24 那一档上砍到第 3 步为止**：记号、页名、尺寸、缩放、判定、理由六列还在
    /// （`p3-session-legibility/13` 立的那三格一个不少，用例是本模块的
    /// `the_narrowest_supported_pane_still_says_where_a_failed_page_got_its_size`）。
    const DROPPED_IN_TURN: &'static [Self] = &[
        Self::Output,
        Self::Crop,
        Self::Cut,
        Self::ColorToGray,
        Self::Scores,
        Self::Size,
        Self::Scaling,
        Self::Reason,
    ];

    const NARROWED: Self = Self::Name;

    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "页名",
            Self::Size => "尺寸",
            Self::Crop => "裁边",
            Self::Scaling => "缩放",
            Self::Cut => "跨页",
            Self::ColorToGray => "彩页",
            Self::Verdict => "判定",
            Self::Reason => "理由",
            Self::Scores => "判据",
            Self::Output => "去处",
        }
    }

    /// 一列都不靠右：尺寸是一对数中间夹着 `x`，靠右摆反而让 `x` 对不齐；
    /// 其余各列都是词、名字或路径。
    fn to_the_right(self) -> bool {
        false
    }

    fn provenance(self) -> Provenance {
        match self {
            Self::Mark => Provenance::Mark,
            // 页名从**另一行**补上（见 `super::draw::pages` 的 `named`）：逐页那两行上
            // 没有它那一格，措辞那一层也没有。
            Self::Name => Provenance::Verbatim,
            Self::Size => Provenance::Wording(Some(Field::Size)),
            Self::Crop => Provenance::Wording(Some(Field::Crop)),
            Self::Scaling => Provenance::Wording(Some(Field::Scaling)),
            Self::Cut => Provenance::Wording(Some(Field::Cut)),
            Self::ColorToGray => Provenance::Wording(Some(Field::ColorToGray)),
            // **对的是另一个名字的格**：屏上这一列叫「判定」，措辞那一层那一格叫候选。
            Self::Verdict => Provenance::Wording(Some(Field::Candidate)),
            Self::Reason => Provenance::Wording(Some(Field::Reason)),
            Self::Scores => Provenance::Wording(Some(Field::Scores)),
            Self::Output => Provenance::Verbatim,
        }
    }
}

/// **三张表摆进列里、由[措辞](Provenance::Wording)那一层写下的那几格。**
///
/// 「摆进列里的字形一个都不许是歧义宽度」那一关问的就是这几格，
/// 而**那一关跑在造字面的那一层**（`crate::render` 那条
/// `every_glyph_this_layer_puts_in_a_lined_up_cell_is_the_same_width_on_any_terminal`）——
/// 它从这里导出，不再手抄第二份（停车场 Q188）。
///
/// **只有这一处出处。** 添一列时 [`Column::provenance`] 那个 `match` 不写就编译不过，
/// 新添的列进不进这一关**因此不由人的记性决定**。
///
/// 画法这一层自己造的那几格不在里面（[耗时](VolumeColumn::Elapsed)、[省略号](ELLIPSIS)、
/// 三张表的行首记号）：措辞那一层没有它们那一格，它们各在自己那一头问。
///
/// **读它的只有那一关**（`crate::render` 那条用例），非测试的那一趟因此没有一个调用方——
/// 与会话里那几处「只有画法读得到」的同一副写法，放开的是这一处，不是整个模块
/// （见 `super` 的模块文档）。[`Column::provenance`] 与 [`Provenance`] 跟着它一起立在那里：
/// **那不是死代码，是那一关的前提**。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn wording_cells() -> Vec<Field> {
    fn of<C: Column>(into: &mut Vec<Field>) {
        into.extend(
            C::ALL
                .iter()
                .filter_map(|column| match column.provenance() {
                    Provenance::Wording(field) => field,
                    Provenance::Verbatim | Provenance::Mark => None,
                }),
        );
    }

    let mut cells = Vec::new();
    of::<DirectoryColumn>(&mut cells);
    of::<VolumeColumn>(&mut cells);
    of::<PageColumn>(&mut cells);
    cells
}

/// 各列有多宽：**那一列上最长的一格**，列头也算一格。
///
/// 起手就是各列的列头（[`Widths::new`]），逐行往上撑（[`Widths::widen`]）。
/// 顺带记下**这一列上有没有一格字**（[`Widths::filled`]）：一整列都不在场时它先让掉
/// （见 [`fit`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Widths<C: Column> {
    /// 一列一格，次序与 [`Column::ALL`] 相同。
    of: Vec<usize>,
    /// 一列一格，次序同上：**这一列上有没有一格真写着字**。列头不算——
    /// 一列只剩列头，那一列就没有话说。
    filled: Vec<bool>,
    /// 量的是**哪一张表**的列。带上它，三张表的量不会串到一处去。
    which: PhantomData<C>,
}

impl<C: Column> Widths<C> {
    /// 起手：每一列先按它的**列头**量。
    ///
    /// 列头**不算一格字**（[`filled`](Self::filled) 起手全是假）：它是这一列的名字，
    /// 不是这一列上的内容。
    pub(super) fn new() -> Self {
        Self {
            of: C::ALL
                .iter()
                .map(|column| usize::from(wrap::width(column.head())))
                .collect(),
            filled: vec![false; C::ALL.len()],
            which: PhantomData,
        }
    }

    /// 这一列上又来了一格：撑得宽就撑宽，**有字就记一笔**（[`note`](Self::note)）。
    pub(super) fn widen(&mut self, column: C, text: &str) {
        let width = usize::from(wrap::width(text));
        let slot = &mut self.of[column.at()];
        *slot = (*slot).max(width);
        self.note(column, text);
    }

    /// 这一列上有一格**在场**，但那一行**这一副不列出来**：只记一笔，不撑宽。
    ///
    /// 两件事分得开是因为它们问的不是同一批行：**这一列在不在场按整卷算**
    /// （逐页那一副按 `a` 在「要紧的页」与「全部页」之间切，而切的是列哪几页——
    /// 屏上不该跟着换一副列），**多宽按真列出来的那几行算**（列不出来的那几行
    /// 一个像素都不占，拿它们撑宽只会白挤掉别的列）。
    pub(super) fn note(&mut self, column: C, text: &str) {
        self.filled[column.at()] |= !text.is_empty();
    }

    /// 这一列多宽。
    pub(super) fn of(&self, column: C) -> usize {
        self.of[column.at()]
    }

    /// 这一列上**有没有一格真写着字**。一行都没量过、或者量过的每一格都是空的就是 `false`。
    pub(super) fn filled(&self, column: C) -> bool {
        self.filled[column.at()]
    }

    /// 把这一列**收窄**到这么多格。砍无可砍时名字那一列走这一条（见 [`elide`]）。
    pub(super) fn narrow(&mut self, column: C, width: usize) {
        self.of[column.at()] = width;
    }
}

/// 这几列并排摆下来占几格：各列的宽度，加上列与列之间那几个 [`GAP`]。
pub(super) fn line_width<C: Column>(kept: &[C], widths: &Widths<C>) -> usize {
    let cells: usize = kept.iter().map(|column| widths.of(*column)).sum();
    cells + GAP * kept.len().saturating_sub(1)
}

/// **这么宽的一格上留得下哪几列**：先让掉[一整列都不在场](Widths::filled)的那几列，
/// 再按 [`Column::DROPPED_IN_TURN`] 那个次序砍，砍到摆得下为止。
///
/// # 一整列都不在场就不占地方
///
/// **那是砍列的第零步，与宽度无关**：那一列上一行字都没有，留着它只是让一个列头
/// 挤掉别的列（逐页那张表上裁边、跨页、彩页转灰三列多数卷一格都不在场，
/// `p4-parking-lot/10`）。一格在不在场本身就是一句话（`CONTEXT.md` 的《格》）——
/// **一整列都不在场时那一列没有话说**。
///
/// **只让得掉[砍得掉的那几列](Column::DROPPED_IN_TURN)**：恒在的那几列是这张表的身份
/// （认得出这是哪一行、它出没出事、判成哪一档），空着也留着——那时那一格的空白正是答案。
///
/// 砍完仍摆不下时就到此为止：恒在的那几列一列不让，
/// [名字那一列](Column::NARROWED)由 [`plan`] 收窄。
/// 屏再窄也要认得出这是哪一行、它出没出事——那正是这张表存在的理由。
pub(super) fn fit<C: Column>(room: usize, widths: &Widths<C>) -> Vec<C> {
    let mut kept: Vec<C> = C::ALL
        .iter()
        .copied()
        .filter(|column| widths.filled(*column) || !C::DROPPED_IN_TURN.contains(column))
        .collect();
    for victim in C::DROPPED_IN_TURN {
        if line_width(&kept, widths) <= room {
            break;
        }
        kept.retain(|column| column != victim);
    }
    kept
}

/// **这么宽的一格上这张表怎么摆**：先[砍列](fit)，砍无可砍再把
/// [名字那一列](Column::NARROWED)收窄到摆得下为止。出的是留下来的那几列。
///
/// 三张表共用这一处，砍与收窄因此不会一张表做全、另一张只做一半。
pub(super) fn plan<C: Column>(room: usize, widths: &mut Widths<C>) -> Vec<C> {
    let kept = fit(room, widths);
    let over = line_width(&kept, widths).saturating_sub(room);
    if over > 0 {
        widths.narrow(
            C::NARROWED,
            widths.of(C::NARROWED).saturating_sub(over).max(1),
        );
    }
    kept
}

/// **一行摆出来**：留哪几列由 `kept` 说了算，每一列占几格由 `widths` 说了算。
///
/// 靠左还是靠右问 [`Column::to_the_right`]。行尾那几句**不占格**，也不参与对齐——
/// 它们是句子，摆不下时跟着整行折下去。
///
/// 行首恒留一格空白：那一格既让表离开框线，也是行尾那句话折下来时的**悬挂缩进**
/// （[`crate::wrap`]：缩进跟着折下来的每一行走）。
pub(super) fn lay<C: Column>(
    kept: &[C],
    widths: &Widths<C>,
    mut cell: impl FnMut(C) -> String,
    notes: &[String],
) -> String {
    let mut line = String::from(" ");
    for (at, column) in kept.iter().enumerate() {
        if at > 0 {
            line.push_str(&" ".repeat(GAP));
        }
        let room = widths.of(*column);
        let text = elide(&cell(*column), room);
        let pad = " ".repeat(room.saturating_sub(usize::from(wrap::width(&text))));
        if column.to_the_right() {
            line.push_str(&pad);
            line.push_str(&text);
        } else {
            line.push_str(&text);
            line.push_str(&pad);
        }
    }
    for note in notes {
        line.push_str(&" ".repeat(GAP));
        line.push_str(note);
    }
    // 行尾那几格空白留着没有意义：折行那一头本来也要去掉它们（[`crate::wrap::fold`]）。
    line.trim_end().to_owned()
}

/// 一格摆不下时**从中间省略**，两头留着：书名与第几卷都要认得出。
///
/// 从中间掐而不是从行尾切：卷名的两头恰恰是最要紧的两截——前面是书名，后面是第几卷，
/// 从尾巴切掉的话满屏的卷长得一模一样。
///
/// 摆得下就一个字都不动。一格都没有时给空串；只剩一格时只剩[省略号](ELLIPSIS)——
/// 那一档上这一列已经答不出任何事，但它仍占着自己那一格，表不会因此错位。
pub(super) fn elide(text: &str, room: usize) -> String {
    if usize::from(wrap::width(text)) <= room {
        return text.to_owned();
    }
    if room == 0 {
        return String::new();
    }
    // 省略号自己占一格，两头分掉剩下的：多出来的那一格给头上——书名比卷号长。
    let keep = room - 1;
    let head = take(text.chars(), keep.div_ceil(2));
    // **头上没用完的那几格还给尾巴**：一个汉字跨在预算边界上时头上会白剩一格，
    // 而那一格摆到尾巴上多半正好再认出一个字（`消⋯卷` → `消⋯那卷`）。
    // 反过来不必再来一轮——头上先分到的就是多的那一半。
    let tail: String = take(
        text.chars().rev(),
        keep.saturating_sub(usize::from(wrap::width(&head))),
    )
    .chars()
    .rev()
    .collect();
    format!("{head}{ELLIPSIS}{tail}")
}

/// 从这一头取到 `room` 格为止。**宽字符跨在边界上就不要它**——半个汉字画出来是一格空白，
/// 而那一格本来就是留给两头的字的。
fn take(glyphs: impl Iterator<Item = char>, room: usize) -> String {
    let mut taken = String::new();
    let mut used = 0;
    let mut buffer = [0u8; 4];
    for glyph in glyphs {
        let width = usize::from(wrap::width(glyph.encode_utf8(&mut buffer)));
        if used + width > room {
            break;
        }
        taken.push(glyph);
        used += width;
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **省略号那一格在哪种终端上都占一格**（判据见 [`tonefit::width_is_stable`]）。
    ///
    /// 它是这一层自己造的唯一一个字形，两张表的行首记号各在自己那一头问；
    /// 措辞那一层摆进列里的那几格在 `crate::render` 那一头问。
    #[test]
    fn the_ellipsis_this_module_makes_is_the_same_width_on_any_terminal() {
        assert!(
            tonefit::width_is_stable(ELLIPSIS),
            "{ELLIPSIS} 是东亚歧义宽度"
        );
        assert_eq!(usize::from(wrap::width(&ELLIPSIS.to_string())), 1);
    }

    /// **三张表的每一列都说得出自己的[字面出处](Provenance)**（`CONTEXT.md`《格》那一维）。
    ///
    /// **编译先红，这一条是第二道**：[`Column::provenance`] 那三处 `match` 一个 `_` 都不留，
    /// 添一列不写那一格就编译不过。这里问的是**答出来的那一档站不站得住**——
    /// 三档各归一条规矩，认错档就是把这一列交给了错的那条规矩。
    #[test]
    fn every_column_says_where_its_text_comes_from() {
        /// 一张表逐列走一遍，收出措辞那一档的那几列（连同它各自报出的那一格）。
        ///
        /// 每张表自己那两条就地问掉，跨表的那两条由调用方合起来问。
        fn walk<C: Column + std::fmt::Debug>(table: &str) -> Vec<(String, Option<Field>)> {
            let mut marks = Vec::new();
            let mut wording = Vec::new();
            for column in C::ALL {
                match column.provenance() {
                    Provenance::Mark => marks.push(*column),
                    Provenance::Wording(field) => {
                        wording.push((format!("{table}的{column:?}"), field))
                    }
                    Provenance::Verbatim => {}
                }
            }
            // **记号那一档恰好一列，而且是行首那一列**：它一个字符说完一件事，
            // 别的列装的都是字（`CONTEXT.md`《语义色》）。
            assert_eq!(marks.len(), 1, "{table}：记号那一档不止一列 {marks:?}");
            assert_eq!(marks[0], C::ALL[0], "{table}：记号不是行首那一列 {marks:?}");
            // **砍无可砍时收窄的那一列是原样那一档**：从中间省略正是那一档的规矩，
            // 而措辞那一档摆不下时不靠省略活着（`CONTEXT.md`《格》）。
            assert!(
                matches!(C::NARROWED.provenance(), Provenance::Verbatim),
                "{table}：收窄的那一列不是原样那一档"
            );
            wording
        }

        let mut wording = walk::<DirectoryColumn>("目录表");
        wording.extend(walk::<VolumeColumn>("卷表"));
        wording.extend(walk::<PageColumn>("逐页表"));

        // **一格只进一列**：两列报出同一格，那一关就把其中一列真正装的东西漏问了。
        let cells: Vec<Field> = wording.iter().filter_map(|(_, field)| *field).collect();
        for (at, field) in cells.iter().enumerate() {
            assert!(!cells[..at].contains(field), "{field:?} 被两列报出来了");
        }

        // **措辞那一档里「这一层自己造的」只放开一列。** 名单只放开眼下真要放开的那一个，
        // 再来一列就得存心加一行（同一副做法见 `tonefit::glyph` 那条用例的 `SENTENCES`）：
        // 报 `None` 等于说「措辞那一层没有我这一格」，而那句话是逃得出那一关的。
        let made_here: Vec<&str> = wording
            .iter()
            .filter(|(_, field)| field.is_none())
            .map(|(who, _)| who.as_str())
            .collect();
        assert_eq!(
            made_here,
            ["卷表的Elapsed"],
            "措辞那一档多了一列说自己不出自措辞那一层"
        );

        // 导出去给那一关的就是这几格，一格不多一格不少。
        assert_eq!(wording_cells(), cells);
    }

    /// 一份够宽的量：各列都比列头宽一点。
    ///
    /// 档位分布那一格照真实那一副的量级给（两档，`two-pass-rework/02`）：
    /// 它是这张表上最宽的一格，砍列的次序正是对着它重新想过的。
    fn measured() -> Widths<VolumeColumn> {
        let mut widths = Widths::new();
        widths.widen(VolumeColumn::Mark, "✓");
        widths.widen(VolumeColumn::Name, "棋魂 07");
        widths.widen(VolumeColumn::Pages, "184");
        widths.widen(VolumeColumn::Tally, "2bit+FS 183 ⋅ 4bit 1");
        widths.widen(VolumeColumn::Driver, "087.png");
        widths.widen(VolumeColumn::Elapsed, "1m12s");
        widths
    }

    /// **一列有多宽是那一列上最长的一格，列头也算一格。**
    #[test]
    fn a_column_is_as_wide_as_its_widest_cell_and_its_head_counts_as_one() {
        let widths = measured();

        // 「记号」两个汉字四格，而记号本身一格：列头撑着这一列。
        assert_eq!(widths.of(VolumeColumn::Mark), 4);
        // 「档位分布」八格，`2bit+FS 183 ⋅ 4bit 1` 二十格：这一次是格撑着列头。
        assert_eq!(widths.of(VolumeColumn::Tally), 20);
        // 中文两格：「棋魂 07」是 2+2+1+2 = 7 格，不是七个字符。
        assert_eq!(widths.of(VolumeColumn::Name), 7);
    }

    /// **给一个宽度，问该留哪几列**：按 `耗时 → 定档页 → 页数 → 档位分布` 那个次序砍。
    ///
    /// 这一条钉的是那个次序本身——它只有 [`Column::DROPPED_IN_TURN`] 一处出处，
    /// 而屏上砍成什么样全由 [`fit`] 说了算。
    #[test]
    fn a_narrower_box_drops_its_columns_in_one_fixed_order() {
        let widths = measured();
        let all = VolumeColumn::ALL.to_vec();
        let full = line_width(&all, &widths);

        assert_eq!(fit(full, &widths), all, "摆得下就一列都不砍");
        assert_eq!(fit(full + 40, &widths), all, "宽得多也不该多砍");

        // 窄一格：先砍耗时。
        let without_elapsed = vec![
            VolumeColumn::Mark,
            VolumeColumn::Name,
            VolumeColumn::Pages,
            VolumeColumn::Tally,
            VolumeColumn::Driver,
        ];
        assert_eq!(fit(full - 1, &widths), without_elapsed);

        // 再窄：定档页跟着走。
        let without_driver = vec![
            VolumeColumn::Mark,
            VolumeColumn::Name,
            VolumeColumn::Pages,
            VolumeColumn::Tally,
        ];
        assert_eq!(
            fit(line_width(&without_elapsed, &widths) - 1, &widths),
            without_driver
        );

        // 再窄：页数也让掉，剩下记号、卷名、档位分布三列。
        let without_pages = vec![VolumeColumn::Mark, VolumeColumn::Name, VolumeColumn::Tally];
        assert_eq!(
            fit(line_width(&without_driver, &widths) - 1, &widths),
            without_pages
        );

        // 再窄：分布也让掉，剩下记号与卷名——**它在收窄卷名之前让**：
        // 二十格的一格留着，卷名就只剩一个省略号，那一行就认不出是哪一卷了。
        let bare = vec![VolumeColumn::Mark, VolumeColumn::Name];
        assert_eq!(fit(line_width(&without_pages, &widths) - 1, &widths), bare);
    }

    /// **目录那张表按它自己那个次序砍：基准档分布 → 卷数**（`volume-discovery/08`）。
    ///
    /// 三张表并排问一遍，钉的是同一条：**各表各有各的次序，而砍的是同一套代码**。
    /// 记号与目录名在最窄那一档上仍在——先要认得出这是哪一枝、它出没出事。
    #[test]
    fn the_directory_table_drops_its_own_columns_in_its_own_order() {
        let mut widths: Widths<DirectoryColumn> = Widths::new();
        widths.widen(DirectoryColumn::Mark, "!");
        widths.widen(DirectoryColumn::Name, "网络资源");
        widths.widen(DirectoryColumn::Volumes, "12");
        widths.widen(DirectoryColumn::Bases, "2bit+FS 9 ⋅ 4bit+FS 3");
        let all = DirectoryColumn::ALL.to_vec();
        let full = line_width(&all, &widths);

        assert_eq!(fit(full, &widths), all, "摆得下就一列都不砍");
        // 分布最先让：它是这张表上最宽的一格，也是比结论深一层的明细。
        let without_bases = vec![
            DirectoryColumn::Mark,
            DirectoryColumn::Name,
            DirectoryColumn::Volumes,
        ];
        assert_eq!(fit(full - 1, &widths), without_bases);
        // 最窄那一档：记号与目录名两列。
        let bare = vec![DirectoryColumn::Mark, DirectoryColumn::Name];
        for room in [0, 1, 5, line_width(&without_bases, &widths) - 1] {
            assert_eq!(fit(room, &widths), bare, "{room} 格上砍成了别的样子");
        }
    }

    /// 逐页那张表的一份量：十一列各一格，宽窄与真实那一副同一个数量级。
    fn per_page() -> Widths<PageColumn> {
        let mut widths: Widths<PageColumn> = Widths::new();
        widths.widen(PageColumn::Mark, "!");
        widths.widen(PageColumn::Name, "087.png");
        widths.widen(PageColumn::Size, "1182x1680");
        widths.widen(PageColumn::Crop, "裁边 1441x2048 ⟶ 1400x2000");
        widths.widen(PageColumn::Scaling, "失败页 ⋅ 卷内统一尺寸留白");
        widths.widen(PageColumn::Cut, "跨页右半");
        widths.widen(PageColumn::ColorToGray, "彩页转灰");
        widths.widen(PageColumn::Verdict, "2bit+FS");
        widths.widen(PageColumn::Reason, "特例页单独定档");
        widths.widen(
            PageColumn::Scores,
            "1bit+FS 32.000 ⋅ 2bit 20.000 ⋅ 4bit 8.000 ⋅ 8bit 2.000",
        );
        widths.widen(PageColumn::Output, "出/隔离/棋魂 07/087.png");
        widths
    }

    /// **逐页那张表按它自己那个次序砍**（`p3-session-legibility/11`，
    /// 次序由 `p4-parking-lot/10` 添成八步，收停车场 Q162）。
    ///
    /// **那个次序不在这里重抄一遍**（`CLAUDE.md`《文档写作》：单一出处）——
    /// 它连同理由只写在 [`PageColumn::DROPPED_IN_TURN`](Column::DROPPED_IN_TURN) 上，
    /// 而这一条照着它一步一步走完，因此**添一步、挪一步都不必改这里**。
    ///
    /// 与卷表并排问一遍，钉的是「两张表各有各的次序，而砍的是同一套代码」：
    /// 记号、页名与**判定**在最窄那一档上仍在——展开一卷要答的正是「这一页判成哪一档」。
    #[test]
    fn the_per_page_table_drops_its_own_columns_in_its_own_order() {
        let widths = per_page();
        let all = PageColumn::ALL.to_vec();
        let full = line_width(&all, &widths);

        assert_eq!(fit(full, &widths), all, "摆得下就一列都不砍");
        // **一步一步照那个次序砍**：每砍掉一列就把宽度再收一格，问下一步舍的是谁。
        // 剩下的那一列就是「此刻该舍谁」，而恒在的那三列一步都不该出现在这里。
        let mut kept = all.clone();
        for victim in PageColumn::DROPPED_IN_TURN {
            let room = line_width(&kept, &widths) - 1;
            kept.retain(|column| column != victim);
            assert_eq!(
                fit(room, &widths),
                kept,
                "{victim:?} 不是这一步该舍的那一列"
            );
        }
        // 最窄那一档：记号、页名、判定三列（`p3-session-legibility/13` 立的那三格）。
        let bare = vec![PageColumn::Mark, PageColumn::Name, PageColumn::Verdict];
        assert_eq!(kept, bare, "砍到底剩下的不是那三列");
        for room in [0, 1, 5, 12, line_width(&bare, &widths) - 1] {
            assert_eq!(fit(room, &widths), bare, "{room} 格上砍成了别的样子");
        }
    }

    /// **80 列那一档上缩放那一列还在**——失败页那一行靠它说出「它的尺寸是卷内统一尺寸」
    /// （`p1-session/11` 的验收，停车场 Q162 记着它丢过一次）。
    ///
    /// 这一条钉的是[砍列次序](Column::DROPPED_IN_TURN)排得对不对：缩放压在倒数第二步，
    /// 最窄的那几档之外它一直在。屏上那一副另有一条（`super::draw::pages`）。
    #[test]
    fn the_narrowest_supported_pane_still_says_where_a_failed_page_got_its_size() {
        // 屏 80 列：左右两道框线各一格，表自己再让出行首那一格（见 `lay`）。
        const ROOM: usize = 80 - 2 - 1;
        let widths = per_page();

        let kept = fit(ROOM, &widths);

        for column in [
            PageColumn::Mark,
            PageColumn::Name,
            PageColumn::Verdict,
            PageColumn::Scaling,
        ] {
            assert!(kept.contains(&column), "{column:?} 在 80 列上被砍掉了");
        }
        assert!(line_width(&kept, &widths) <= ROOM, "{kept:?} 摆不下");
    }

    /// **最窄那一档上卷名与行首记号仍在。**
    ///
    /// 砍到没得砍了也停在这两列上：一行上先要认得出这是哪一卷、它出没出事。
    /// 档位分布不在里面（`two-pass-rework/02`）：跳过与没做成那两个词丢了，
    /// 行首记号仍说着同一件事——靠得住的载体是它（见 `super::draw::table::Mark`）。
    #[test]
    fn the_narrowest_table_still_has_its_marks_and_its_volume_names() {
        let widths = measured();

        for room in [0, 1, 2, 5, 10, 20] {
            let kept = fit(room, &widths);
            assert!(
                kept.contains(&VolumeColumn::Mark),
                "{room} 格上砍掉了行首记号"
            );
            assert!(kept.contains(&VolumeColumn::Name), "{room} 格上砍掉了卷名");
            assert!(
                !kept.contains(&VolumeColumn::Elapsed),
                "{room} 格上还留着耗时"
            );
            assert!(
                !kept.contains(&VolumeColumn::Driver),
                "{room} 格上还留着定档页"
            );
            assert!(
                !kept.contains(&VolumeColumn::Pages),
                "{room} 格上还留着页数"
            );
            assert!(
                !kept.contains(&VolumeColumn::Tally),
                "{room} 格上还留着档位分布"
            );
        }
    }

    /// **头上没用完的那几格还给尾巴**：宽字符跨在预算边界上时不白扔。
    ///
    /// 「消失的那卷」十格收进七格：头上分到 3 格却只摆得下「消」（2 格），
    /// 剩下那一格还给尾巴，于是尾巴摆得下「那卷」而不是只有「卷」。
    /// 窄终端上卷名那一列本来就只有几格，白扔一格就少认出一个字。
    #[test]
    fn what_the_head_does_not_use_goes_back_to_the_tail() {
        assert_eq!(elide("消失的那卷", 7), "消⋯那卷");
        assert_eq!(usize::from(wrap::width("消⋯那卷")), 7);
    }

    /// **卷名摆不下时从中间省略，两头留着。**
    #[test]
    fn a_volume_name_too_wide_for_its_column_is_elided_in_the_middle() {
        assert_eq!(elide("棋魂 07", 7), "棋魂 07", "摆得下就一个字都不动");
        assert_eq!(elide("棋魂 07", 99), "棋魂 07");

        // 书名与卷号两头都还认得出。
        let long = "光之棋：完全版 第 07 卷";
        let short = elide(long, 12);
        assert_eq!(usize::from(wrap::width(&short)), 12);
        assert!(short.starts_with('光'), "书名那一头没留下：{short}");
        assert!(short.ends_with('卷'), "第几卷那一头没留下：{short}");
        assert!(short.contains(ELLIPSIS), "中间那一截没说省略过：{short}");
    }

    /// **窄到只剩一两格也不错位、不恐慌**：宽字符跨在边界上就不要它。
    #[test]
    fn eliding_into_a_sliver_of_a_column_neither_panics_nor_splits_a_wide_glyph() {
        assert_eq!(elide("棋魂 07", 0), "");
        assert_eq!(elide("棋魂 07", 1), "⋯");
        // 两格里塞不下「棋」加省略号：头上那一格给不了半个汉字，那一格于是**还给尾巴**，
        // 正好摆得下卷号的末一位——半个字画不出来，一个窄字画得出来。
        assert_eq!(elide("棋魂 07", 2), "⋯7");
        for room in 0..=8 {
            let short = elide("光之棋：完全版", room);
            assert!(
                usize::from(wrap::width(&short)) <= room,
                "{room} 格上省略出来的还是 {} 格：{short}",
                wrap::width(&short)
            );
        }
    }

    /// **列摆下来占几格：各列宽度加上中间那几个空。**
    #[test]
    fn a_row_is_as_wide_as_its_columns_plus_the_gaps_between_them() {
        let widths = measured();

        assert_eq!(
            line_width::<VolumeColumn>(&[], &widths),
            0,
            "一列都没有就不占地方"
        );
        assert_eq!(line_width(&[VolumeColumn::Mark], &widths), 4, "一列不加空");
        assert_eq!(
            line_width(&[VolumeColumn::Mark, VolumeColumn::Name], &widths),
            4 + GAP + 7
        );
    }

    /// **砍无可砍时名字那一列收窄，这一行因此正好摆得下**（[`plan`]）。
    ///
    /// 三张表共用这一步：收窄哪一列由 [`Column::NARROWED`] 一处说了算。
    #[test]
    fn narrowing_the_name_column_makes_the_row_fit() {
        let mut widths = measured();
        // 记号四格加一个空再加卷名七格是十三格：十格上砍无可砍，只剩收窄卷名。
        let room = 10;

        let kept = plan(room, &mut widths);

        assert_eq!(kept, vec![VolumeColumn::Mark, VolumeColumn::Name]);
        assert_eq!(line_width(&kept, &widths), room, "收窄之后没有正好摆下");
        // 名字那一列收得再窄也留一格：它恒在（见 [`plan`]）。
        let mut sliver: Widths<PageColumn> = Widths::new();
        sliver.widen(PageColumn::Name, "087.png");
        let kept = plan(0, &mut sliver);
        assert!(kept.contains(&PageColumn::Name));
        assert_eq!(sliver.of(PageColumn::Name), 1);
    }

    /// **一行摆出来：靠左的靠左、靠右的靠右，行尾那几句不占格**（[`lay`]）。
    #[test]
    fn a_row_pads_each_cell_to_its_column_and_leaves_the_notes_outside() {
        let widths = measured();
        let kept = vec![VolumeColumn::Mark, VolumeColumn::Name, VolumeColumn::Pages];

        let row = lay(
            &kept,
            &widths,
            |column| match column {
                VolumeColumn::Mark => "!".to_owned(),
                VolumeColumn::Name => "棋魂 07".to_owned(),
                VolumeColumn::Pages => "7".to_owned(),
                _ => String::new(),
            },
            &["隔离".to_owned()],
        );

        // 行首那一格空白（悬挂缩进），页数靠右摆在四格里（列头「页数」四格）。
        assert!(row.starts_with(" !"), "{row}");
        assert!(row.contains("棋魂 07"), "{row}");
        assert!(row.contains("   7"), "页数没靠右：{row}");
        assert!(row.ends_with("隔离"), "行尾那一句不在：{row}");
    }
}
