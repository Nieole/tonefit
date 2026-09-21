//! 卷列表那棵树与每页结果那两张表的**列**：有哪几列、窄了先让谁、各多宽、
//! 一格摆不下的字怎么省略（`CONTEXT.md` 的《砍列》《格》）。
//!
//! 表真画出来是画法那一层的事（`super::shell::list` 与 `super::shell::pages`）；
//! 本模块只答三件事：**列的次序**、**砍列的次序**（各表那几道宽度门槛），
//! 以及**一格摆不下的字怎么省略**（[`elide`]）。
//!
//! # 两张表，各一个次序
//!
//! 树一行一个目录或一卷（[`TreeColumn`]），每页结果一页一行（[`PagesColumn`]）。
//! 两张的列宽都是**定死的**——屏上一行一行要对齐在同一处，一行一行地量宽度会让上下两行
//! 的列错开——砍列因此是「框里有这么宽才摆得下它」的几道门槛（[`TreeWidths`]、
//! [`PagesWidths`]），门槛从宽到窄就是各自宣告的那个次序（[`Column::DROPPED_IN_TURN`]）。
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
//! **摆进列里的字形一个都不许是歧义宽度**，画质分、边界与理由都在
//! [`tonefit::width_is_stable`]。
//!
//! 那条规矩管两层：**这一层自己造的字形**（[`ELLIPSIS`] 与两张表的行首记号），
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
//! 从前措辞那一层还手抄着第二份，往表里添一列那一份不跟着添**也不会红**（停车场 Q188）。

use crate::render::Field;
use crate::wrap;

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
/// [灰阶](PagesColumn::Verdict)那一列对的是**另一个名字**的格（[`Field::Candidate`]）、
/// 树上[卷数](TreeColumn::Count)那一列两种行各出自一格。
///
/// **三档各归一条规矩，而规矩不在这里**——这一维只答「这一格该归谁管」：
/// 措辞归造字面的那一层查字形，原样归[从中间省略](elide)，记号归画它的那一层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provenance {
    /// **措辞 (Wording)**：界面层自己造的字面（尺寸、裁白边、缩放、判定、理由、页数、分布……）。
    ///
    /// 字形宽度**必须稳**（[`tonefit::width_is_stable`]），而查它的是**造字面的那一层**——
    /// 带着的正是「查哪一格」：`Some(field)` 是 [`crate::render`] 出的那一格，
    /// 那一层那条用例从 `wording_cells` 拿走全部要查的格；`None` 是画法这一层
    /// 自己造的字（[耗时](TreeColumn::Elapsed)、[树上那一列卷数](TreeColumn::Count)、
    /// [提示](PagesColumn::Notes)），
    /// 它们在自己那一头查。
    Wording(Option<Field>),
    /// **原样 (Verbatim)**：用户的字节原封带过来（目录名、卷名、页名、代表页名）。
    ///
    /// 宽度**永远稳不住**——一个带 emoji 的文件名不该把整张表判红——摆不下时归
    /// [从中间省略](elide)管，**不进那一关**。它因此不必报出自己出自哪一格。
    Verbatim,
    /// **记号 (Mark)**：一个字符说完一件事，**不是一格字**
    /// （`CONTEXT.md`《语义色》：颜色不是唯一载体）。
    ///
    /// 两张表的行首记号都在这一档，那几个字形由画它的那一层问（`super::shell::marks`）。
    Mark,
}

/// **一张表的那几列**：从左到右是哪几列、窄了按什么次序让、各出自哪一档字面出处。
pub(super) trait Column: Copy + PartialEq + 'static {
    /// 全部列，**从左到右**。表就按这个次序摆。
    const ALL: &'static [Self];

    /// **砍列的次序**：横向摆不下时按它一列一列舍掉（`CONTEXT.md` 的《砍列》）。
    ///
    /// **只有这一处宣告。** 恒在的那几列不在这里边——一行上先要认得出这是哪一行、
    /// 它出没出事，剩下的几列都是这两件之后的事。落实它的是各表那几道宽度门槛
    /// （[`TreeWidths::of`]、[`PagesWidths::of`]），两者对不对得上由本模块的用例钉住——
    /// 非测试那一趟因此没有读者，**那不是死代码，是那几道门槛要对上的那份宣告**。
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "落实它的是各表那几道宽度门槛，用例拿它对照")
    )]
    const DROPPED_IN_TURN: &'static [Self];

    /// 列头。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    fn head(self) -> &'static str;

    /// 这一列的字**是谁写的**（[字面出处](Provenance)）。
    ///
    /// **每一个变体都得自己答**：两处 `match` 一个 `_` 都不留，添一列不写这一格
    /// **编译就过不去**。从前「哪几格要过宽度那一关」在仓库里另有一份手抄的名单，
    /// 添一列不跟着添也不会红（停车场 Q188）。
    fn provenance(self) -> Provenance;
}

/// **卷列表那棵树**上的一列（`session-redesign/08`）：新界面清点之后那一副，
/// 目录行与卷行**共用同一套列**——两种行只差左边的缩进与记号，右边那几列对齐在同一处。
///
/// **页数恒在**（`CONTEXT.md` 的《目录行 / 卷行》：做完几卷／共几卷，或几页，
/// 与名字一起就是一行的身份）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TreeColumn {
    /// 行首记号：这一行此刻怎么样，一个字符说完。**恒在。**
    Mark,
    /// 目录名或卷名。**恒在**，摆不下时从中间省略（见 [`elide`]）。
    Name,
    /// 做完几卷／共几卷（目录行），或者几页（卷行）。**恒在。**
    Count,
    /// 灰阶分布（前两档），或者这一卷为什么一页都没判（跳过、没做成）。
    Tally,
    /// 代表页。**只在整卷统一灰阶那一趟在场**（停车场 Q712）。
    Driver,
    /// 这一行做了多久。
    Elapsed,
}

impl Column for TreeColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Count,
        Self::Tally,
        Self::Driver,
        Self::Elapsed,
    ];

    /// **砍列的次序：耗时 → 代表页 → 灰阶分布**（spec《卷列表》；`CONTEXT.md` 的《砍列》）。
    ///
    /// 记号、名字与卷数不在这里边——它们**恒在**：一行上先要认得出这是哪一行、它出没出事、
    /// 它有多厚。次序按「摆不下时先舍谁」排：耗时最先——它是这一行做完之后的一个旁证；
    /// 代表页次之，而它本来就只在整卷统一灰阶那一趟在场；**灰阶分布压后**——
    /// 它是这张表要答的那件事，让掉它之后跳过与没做成那两个词跟着丢，
    /// 而行首记号说的是同一件事：靠得住的载体是记号，不是这一格。
    const DROPPED_IN_TURN: &'static [Self] = &[Self::Elapsed, Self::Driver, Self::Tally];

    /// **这张表不画列头**：树上一行一个身份，屏上没有那一行标题（见设计稿的卷列表）。
    /// 列头留在这里只为与[每页结果那一张](PagesColumn)同形。
    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "名字",
            Self::Count => "卷数",
            Self::Tally => "灰阶分布",
            Self::Driver => "代表页",
            Self::Elapsed => "耗时",
        }
    }

    fn provenance(self) -> Provenance {
        match self {
            Self::Mark => Provenance::Mark,
            Self::Name => Provenance::Verbatim,
            // **这一列的字是画法那一层拼出来的**：一个数出自措辞那一层
            // （目录行是 [`Field::VolumeCount`]，卷行是 [`Field::PageCount`]），
            // 后面那个单位（`卷`／`页`）由画法接上——两种行共用一列，指不到单独一格上。
            Self::Count => Provenance::Wording(None),
            Self::Tally => Provenance::Wording(Some(Field::Tally)),
            Self::Driver => Provenance::Verbatim,
            // **这一列的字是画法那一层自己造的**（`super::shell::marks` 的 `spell`）。
            Self::Elapsed => Provenance::Wording(None),
        }
    }
}

/// 树上各列**此刻有多宽**：一列一个数，零就是这一列不在场。
///
/// **次序是 [`TreeColumn::DROPPED_IN_TURN`]**，列宽是**定死的**
/// （屏上目录行与卷行要对齐在同一处，一行一行地量宽度会让上下两行的列错开），
/// 砍列因此变成「框里有这么宽才摆得下它」的一道门槛。
/// 三道门槛从宽到窄正是那个次序：耗时先让、代表页次之、灰阶分布压后。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TreeWidths {
    /// 名字那一列（顶格那一行的宽度；缩进一级的行在它上面各减两格）。
    pub(super) name: u16,
    pub(super) count: u16,
    pub(super) tally: u16,
    pub(super) driver: u16,
    pub(super) elapsed: u16,
}

/// 名字那一列最窄与最宽各占几格，以及它占框内宽度的几成。
const NAME_LEAST: u16 = 12;
const NAME_MOST: u16 = 30;
const NAME_SHARE: u16 = 24;

impl TreeWidths {
    /// 框里内容那一截有 `inner` 格宽时各列有多宽。`envelope` 是整卷统一灰阶那一趟——
    /// 代表页那一列只在它在场（停车场 Q712）。
    pub(super) fn of(inner: u16, envelope: bool) -> Self {
        let keeps = |column: TreeColumn| inner >= least_width(column, envelope);
        Self {
            name: u16::try_from(u32::from(inner) * u32::from(NAME_SHARE) / 100)
                .unwrap_or(NAME_MOST)
                .clamp(NAME_LEAST, NAME_MOST),
            count: 9,
            tally: if keeps(TreeColumn::Tally) { 26 } else { 0 },
            driver: if envelope && keeps(TreeColumn::Driver) {
                15
            } else {
                0
            },
            elapsed: if keeps(TreeColumn::Elapsed) { 7 } else { 0 },
        }
    }

    /// 此刻还在场的那几列，从左到右。
    pub(super) fn kept(&self) -> Vec<TreeColumn> {
        TreeColumn::ALL
            .iter()
            .copied()
            .filter(|column| match column {
                TreeColumn::Mark | TreeColumn::Name | TreeColumn::Count => true,
                TreeColumn::Tally => self.tally > 0,
                TreeColumn::Driver => self.driver > 0,
                TreeColumn::Elapsed => self.elapsed > 0,
            })
            .collect()
    }
}

/// 一列要框内多宽才摆得下——**砍列次序就是这三个数从大到小**，
/// 而恒在的那三列一格都不要。
fn least_width(column: TreeColumn, envelope: bool) -> u16 {
    match column {
        TreeColumn::Mark | TreeColumn::Name | TreeColumn::Count => 0,
        TreeColumn::Tally => 92,
        TreeColumn::Driver => 100,
        TreeColumn::Elapsed => {
            if envelope {
                124
            } else {
                108
            }
        }
    }
}

/// **每页结果那一张表**上的一列（`session-redesign/11`）：新界面从卷行进到页的那一副。
///
/// 列宽**定死**（占整宽，一页一行要对齐在同一处）；**先让缩放、再让画质分、再让尺寸**，
/// 而**灰阶恒在**（`CONTEXT.md` 的《砍列》；spec 的《每页结果》）。
///
/// 列的选法答的是**从卷进到页要问的那件事**：哪一页判成了哪一档、凭什么。
/// 从左到右四段：**这一页是谁**（记号 · 页面）、**它这个样子是怎么来的**（尺寸 · 缩放）、
/// **它判成哪一档、凭什么**（灰阶 · 原因 · 画质分），末一列是**提示**——
/// 这一页要紧在哪几处那几个词，连同成句的那一句（坏页那一句原因）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PagesColumn {
    /// 行首记号：这一页要不要紧，一个字符说完。**恒在。**
    Mark,
    /// 页面（成员名，只印最后那一段）。**恒在**，摆不下时从中间省略。
    Name,
    /// 这一页的输出尺寸。
    Size,
    /// 缩放怎么算的；**坏页说的是它的尺寸从哪来**（`p1-session/11` 的验收）。
    Scaling,
    /// 这一页判成的那一档。**恒在**——它就是这一副要答的那件事。
    Verdict,
    /// 判成这一档的理由。**恒在。**
    Reason,
    /// **判成那一档在这一页上的那个分**（`2bit+FS 3.515`）——不是六个候选那一整串
    /// （那是命令行那一副的 [`Field::Scores`]，
    /// 停车场 Q725）。
    Scores,
    /// 提示：这一页要紧在哪几处那几个词，加上成句的那一句。**恒在**，吃剩下的宽度。
    Notes,
}

impl Column for PagesColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Size,
        Self::Scaling,
        Self::Verdict,
        Self::Reason,
        Self::Scores,
        Self::Notes,
    ];

    /// **砍列的次序：缩放 → 画质分 → 尺寸**（spec 的《每页结果》：先让缩放、再让画质分、
    /// 再让尺寸；灰阶恒在）。
    ///
    /// 记号、页面、灰阶、原因与提示不在这里边——它们**恒在**：先要认得出这是哪一页、
    /// 它判成哪一档、凭什么、出没出事。次序按「摆不下时先舍谁」排：
    ///
    /// 1. **缩放**最先——它是这张表上最宽的一格（整整二十四格），而「这一页怎么缩的」
    ///    比「它判成哪一档」隔着一层。旧那一张把它压到最后，为的是坏页那一行靠它说出
    ///    「它的尺寸是卷内统一尺寸」；**这一张不必**——坏页那一句原因在提示那一列上，
    ///    而提示恒在。
    /// 2. **画质分**次之——它是证据，比结论深一层。
    /// 3. **尺寸**压后——页面超宽那件事提示那一列已经说了。
    ///
    /// **80×24 那一档上三列都让掉**：记号、页面、灰阶、原因、提示五列还在
    /// （见 [`PagesWidths::of`] 那三道门槛与本模块的
    /// `the_pages_pane_drops_its_columns_in_the_one_order_it_declares`）。
    const DROPPED_IN_TURN: &'static [Self] = &[Self::Scaling, Self::Scores, Self::Size];

    fn head(self) -> &'static str {
        match self {
            // **记号那一列不画列头**：它不是一格字（见 [`Provenance::Mark`]），
            // 屏上那一行在它的位置上留着两格空。
            Self::Mark => "记号",
            Self::Name => "页面",
            Self::Size => "尺寸",
            Self::Scaling => "缩放",
            Self::Verdict => "灰阶",
            Self::Reason => "原因",
            Self::Scores => "画质分",
            Self::Notes => "提示",
        }
    }

    fn provenance(self) -> Provenance {
        match self {
            Self::Mark => Provenance::Mark,
            Self::Name => Provenance::Verbatim,
            Self::Size => Provenance::Wording(Some(Field::Size)),
            Self::Scaling => Provenance::Wording(Some(Field::Scaling)),
            // **对的是另一个名字的格**：屏上这一列叫「灰阶」，措辞那一层那一格叫候选。
            Self::Verdict => Provenance::Wording(Some(Field::Candidate)),
            Self::Reason => Provenance::Wording(Some(Field::Reason)),
            Self::Scores => Provenance::Wording(Some(Field::VerdictScore)),
            // **这一列的字是画法那一层拼出来的**：要紧在哪几处那几个词是界面层自己的
            // （`super::shell::marks` 的 `notable_word` 一处），成句的那几格出自措辞那一层
            // （残缺救回了多少、纸白与钳制、坏页那一句原因）——几样并成一列，
            // 指不到单独一格上。
            Self::Notes => Provenance::Wording(None),
        }
    }
}

/// 每页结果那几列**此刻有多宽**：一列一个数，零就是这一列不在场。
///
/// **次序是 [`PagesColumn::DROPPED_IN_TURN`]**，与
/// [树那一张](TreeWidths)同一条理由——屏上一页一行要对齐在同一处，一行一行地量宽度
/// 会让上下两行的列错开。砍列因此是「框里有这么宽才摆得下它」的三道门槛。
///
/// **一列占几格连它与下一列之间那两格一起算**：各列挨着补空到这个数摆下去
/// （[`super::shell::pages`] 照它画）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PagesWidths {
    /// 页面那一列。**恒在。**
    pub(super) name: u16,
    pub(super) size: u16,
    pub(super) scaling: u16,
    /// 灰阶那一列。**恒在。**
    pub(super) verdict: u16,
    /// 原因那一列。**恒在。**
    pub(super) reason: u16,
    pub(super) scores: u16,
}

/// 行首那两截各占几格：光标记号（`❯ `）与这一页要不要紧那个记号（`✗ `）。
/// 两截都恒在，提示那一列吃剩下的宽度时要减掉它们。
pub(super) const PAGES_MARKS: u16 = 4;

/// 提示那一列至少留几格：剩不下这么多就不再往下缩——半句话比没有话更坏。
const NOTES_LEAST: u16 = 4;

impl PagesWidths {
    /// 框里内容那一截有 `inner` 格宽时各列有多宽。
    pub(super) fn of(inner: u16) -> Self {
        let keeps = |column: PagesColumn| inner >= least_pages_width(column);
        Self {
            name: 10,
            size: if keeps(PagesColumn::Size) { 11 } else { 0 },
            scaling: if keeps(PagesColumn::Scaling) { 24 } else { 0 },
            verdict: 9,
            reason: 22,
            scores: if keeps(PagesColumn::Scores) { 15 } else { 0 },
        }
    }

    /// 这一列占几格。提示那一列不在里边——它吃剩下的（[`notes`](Self::notes)）。
    pub(super) fn of_column(&self, column: PagesColumn) -> u16 {
        match column {
            PagesColumn::Mark | PagesColumn::Notes => 0,
            PagesColumn::Name => self.name,
            PagesColumn::Size => self.size,
            PagesColumn::Scaling => self.scaling,
            PagesColumn::Verdict => self.verdict,
            PagesColumn::Reason => self.reason,
            PagesColumn::Scores => self.scores,
        }
    }

    /// **提示那一列吃剩下的**：一行那么宽，减掉行首那两截与前面几列，至少 [`NOTES_LEAST`] 格。
    ///
    /// `row` 是一行摆得下几格（框里那一截再加一格：行从光标记号那一格起笔，
    /// 比抬头那一行靠左一格）。
    pub(super) fn notes(&self, row: u16) -> u16 {
        let used = PAGES_MARKS
            + PagesColumn::ALL
                .iter()
                .map(|column| self.of_column(*column))
                .sum::<u16>();
        row.saturating_sub(used).max(NOTES_LEAST)
    }

    /// 此刻还在场的那几列，从左到右——与[树那一张](TreeWidths::kept)答的是同一个问题。
    pub(super) fn kept(&self) -> Vec<PagesColumn> {
        PagesColumn::ALL
            .iter()
            .copied()
            .filter(|column| match column {
                PagesColumn::Mark
                | PagesColumn::Name
                | PagesColumn::Verdict
                | PagesColumn::Reason
                | PagesColumn::Notes => true,
                PagesColumn::Size => self.size > 0,
                PagesColumn::Scaling => self.scaling > 0,
                PagesColumn::Scores => self.scores > 0,
            })
            .collect()
    }
}

/// 一列要框内多宽才摆得下——**砍列次序就是这三个数从大到小**，
/// 而恒在的那五列一格都不要。
fn least_pages_width(column: PagesColumn) -> u16 {
    match column {
        PagesColumn::Mark
        | PagesColumn::Name
        | PagesColumn::Verdict
        | PagesColumn::Reason
        | PagesColumn::Notes => 0,
        PagesColumn::Scaling => 112,
        PagesColumn::Scores => 98,
        PagesColumn::Size => 84,
    }
}

/// **摆进列里、由[措辞](Provenance::Wording)那一层写下的那几格。**
///
/// 从屏上那两张表（[树](TreeColumn)与[每页结果](PagesColumn)）逐列导出；**一格只进一列**
/// ——同一格报两遍，那一关反而分不清问的是哪一列（本模块那条用例钉着）。
///
/// 「摆进列里的字形一个都不许是歧义宽度」那一关问的就是这几格，
/// 而**那一关跑在造字面的那一层**（`crate::render` 那条
/// `every_glyph_this_layer_puts_in_a_lined_up_cell_is_the_same_width_on_any_terminal`）——
/// 它从这里导出，不再手抄第二份（停车场 Q188）。
///
/// **只有这一处出处。** 添一列时 [`Column::provenance`] 那个 `match` 不写就编译不过，
/// 新添的列进不进这一关**因此不由人的记性决定**。
///
/// 画法这一层自己造的那几格不在里面（[耗时](TreeColumn::Elapsed)、[省略号](ELLIPSIS)、
/// 两张表的行首记号）：措辞那一层没有它们那一格，它们各在自己那一头问。
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
    of::<TreeColumn>(&mut cells);
    of::<PagesColumn>(&mut cells);
    cells
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

    /// **省略号那一格在哪种终端上都占一格**（画质分见 [`tonefit::width_is_stable`]）。
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

    /// **两张表的每一列都说得出自己的[字面出处](Provenance)**（`CONTEXT.md`《格》那一维）。
    ///
    /// **编译先红，这一条是第二道**：[`Column::provenance`] 那两处 `match` 一个 `_` 都不留，
    /// 添一列不写那一格就编译不过。这里问的是**答出来的那一档站不站得住**——
    /// 三档各归一条规矩，认错档就是把这一列交给了错的那条规矩。
    #[test]
    fn every_column_says_where_its_text_comes_from() {
        /// 一张表逐列走一遍，收出措辞那一档的那几列（连同它各自报出的那一格）。
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
            wording
        }

        let mut wording = walk::<TreeColumn>("树");
        wording.extend(walk::<PagesColumn>("每页结果"));
        // **摆不下时从中间省略的名字那一列是原样那一档**：从中间省略正是那一档的规矩，
        // 而措辞那一档摆不下时不靠省略活着（`CONTEXT.md`《格》）。
        assert_eq!(TreeColumn::Name.provenance(), Provenance::Verbatim);
        assert_eq!(PagesColumn::Name.provenance(), Provenance::Verbatim);

        // **一格只进一列**：两列报出同一格，那一关就把其中一列真正装的东西漏问了。
        let cells: Vec<Field> = wording.iter().filter_map(|(_, field)| *field).collect();
        for (at, field) in cells.iter().enumerate() {
            assert!(!cells[..at].contains(field), "{field:?} 被两列报出来了");
        }

        // **措辞那一档里「这一层自己造的」只放开这几列。** 名单只放开眼下真要放开的那几个，
        // 再来一列就得存心加一行（同一副做法见 `tonefit::glyph` 那条用例的 `SENTENCES`）：
        // 报 `None` 等于说「措辞那一层没有我这一格」，而那句话是逃得出那一关的。
        let made_here: Vec<&str> = wording
            .iter()
            .filter(|(_, field)| field.is_none())
            .map(|(who, _)| who.as_str())
            .collect();
        assert_eq!(
            made_here,
            ["树的Count", "树的Elapsed", "每页结果的Notes"],
            "措辞那一档多了一列说自己不出自措辞那一层"
        );

        // 导出去给那一关的就是这几格，一格不多一格不少。
        assert_eq!(wording_cells(), cells);
    }

    /// **树上砍列的次序：耗时 → 代表页 → 灰阶分布**（`session-redesign/08` 票面第五条）。
    ///
    /// 这一条钉的是[那个次序](TreeColumn::DROPPED_IN_TURN)真管着屏上砍成什么样：
    /// 宽度一格格收窄，让掉的先后就是它，而记号、名字与卷数**一格都不让**。
    /// 画法那一层不许再写第二份——它问的是 [`TreeWidths`]（`super::shell::list`）。
    #[test]
    fn the_tree_drops_its_columns_in_the_one_order_it_declares() {
        let gone = |inner: u16, envelope: bool| -> Vec<TreeColumn> {
            let kept = TreeWidths::of(inner, envelope).kept();
            // 按**那个次序**排出来：这一条要的正是「让掉的先后」。
            TreeColumn::DROPPED_IN_TURN
                .iter()
                .copied()
                .filter(|column| !kept.contains(column))
                .collect()
        };
        // 整卷统一灰阶那一趟：一格格收窄，让掉的先后就是那个次序。
        assert_eq!(gone(130, true), []);
        assert_eq!(gone(120, true), [TreeColumn::Elapsed]);
        assert_eq!(gone(95, true), [TreeColumn::Elapsed, TreeColumn::Driver]);
        assert_eq!(
            gone(80, true),
            [TreeColumn::Elapsed, TreeColumn::Driver, TreeColumn::Tally]
        );
        assert_eq!(
            gone(80, true),
            TreeColumn::DROPPED_IN_TURN,
            "让到最后，让掉的正是声明的那一串"
        );
        // 默认逐页那一趟**代表页整列不在场**（停车场 Q712），连列头都不占。
        assert!(gone(130, false).contains(&TreeColumn::Driver));
        // 记号、名字与卷数一格都不让。
        for inner in [200, 116, 76, 40, 12] {
            let kept = TreeWidths::of(inner, false).kept();
            assert!(
                kept.contains(&TreeColumn::Mark)
                    && kept.contains(&TreeColumn::Name)
                    && kept.contains(&TreeColumn::Count),
                "{inner} 列宽上让掉了恒在的那几列"
            );
        }
    }

    /// **每页结果上砍列的次序：缩放 → 画质分 → 尺寸**（`session-redesign/11` 票面第二条）。
    ///
    /// 与[树那一条](the_tree_drops_its_columns_in_the_one_order_it_declares)同一个形状：
    /// 宽度一格格收窄，让掉的先后就是[那个次序](PagesColumn::DROPPED_IN_TURN)，
    /// 而记号、页面、**灰阶**、原因与提示一格都不让。
    /// 画法那一层不许再写第二份——它问的是 [`PagesWidths`]（`super::shell::pages`）。
    ///
    /// 主稿 120×36 那一屏框内 116 格，七列全在；验收线 80×24 那一屏框内 76 格，
    /// 三列一个不剩。
    #[test]
    fn the_pages_pane_drops_its_columns_in_the_one_order_it_declares() {
        let gone = |inner: u16| -> Vec<PagesColumn> {
            let kept = PagesWidths::of(inner).kept();
            PagesColumn::DROPPED_IN_TURN
                .iter()
                .copied()
                .filter(|column| !kept.contains(column))
                .collect()
        };
        assert_eq!(gone(116), [], "主稿那一屏七列全在");
        assert_eq!(gone(111), [PagesColumn::Scaling]);
        assert_eq!(gone(97), [PagesColumn::Scaling, PagesColumn::Scores]);
        assert_eq!(
            gone(76),
            [PagesColumn::Scaling, PagesColumn::Scores, PagesColumn::Size],
            "验收线那一屏三列都让掉了"
        );
        for inner in [200, 116, 76, 40, 12] {
            let kept = PagesWidths::of(inner).kept();
            for column in [
                PagesColumn::Mark,
                PagesColumn::Name,
                PagesColumn::Verdict,
                PagesColumn::Reason,
                PagesColumn::Notes,
            ] {
                assert!(kept.contains(&column), "{inner} 列宽上让掉了 {column:?}");
            }
        }
    }

    /// **提示那一列吃剩下的，至少留四格**：一行那么宽减掉行首那两截与前面几列。
    /// 主稿那一屏上正是设计稿那个数（`notesW`）。
    #[test]
    fn the_notes_column_takes_what_is_left_of_the_row() {
        assert_eq!(PagesWidths::of(116).notes(117), 22);
        assert_eq!(PagesWidths::of(76).notes(77), 32);
        assert_eq!(PagesWidths::of(76).notes(10), NOTES_LEAST, "剩不下就不再缩");
    }

    /// **默认逐页那一趟代表页那一列连列头都不在场**（`session-redesign/10` 票面第三个
    /// 验收框；停车场 Q712）。
    ///
    /// 问的是**两件事**：它不在[此刻在场的那几列](TreeWidths::kept)里，而且**宽度是零**
    /// ——画法那一层照这个数往右走笔（`super::shell::list` 的 `lined_row`），
    /// 只问 `kept()` 的话，一列「在场但宽零」与「不在场」分不开，而屏上差的是十七格。
    /// 再宽都不在场：它不是让位让掉的，是这一趟没有这件事。
    #[test]
    fn without_the_envelope_the_driver_column_is_not_there_at_all() {
        for inner in [200, 130, 116, 100, 92, 80, 40] {
            let widths = TreeWidths::of(inner, false);
            assert_eq!(widths.driver, 0, "{inner} 列宽上代表页那一列占了格");
            assert!(
                !widths.kept().contains(&TreeColumn::Driver),
                "{inner} 列宽上代表页那一列在场"
            );
        }
        // 整卷统一灰阶那一趟够宽就在场，而它仍要让在耗时之后（上一条钉的是那个次序）。
        let envelope = TreeWidths::of(130, true);
        assert!(envelope.driver > 0);
        assert!(envelope.kept().contains(&TreeColumn::Driver));
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
}
