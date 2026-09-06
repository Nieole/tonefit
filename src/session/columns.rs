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
//! [`crate::wrap::width_is_stable`]。
//!
//! 那条规矩管两层：**这一层自己造的字形**（[`ELLIPSIS`] 与两张表的行首记号），
//! 与**措辞那一层摆进列里的那几格**（`crate::render` 的尺寸、判据那一串与基准档分布）。
//! 后者从前划在规矩外面——换它们是命令行印出去的字节的一次变动，不归画法这一层；
//! `p4-parking-lot/05` 换掉了那两个字形，管辖面跟着扩到那一层（停车场 Q168）。
//!
//! 停车场 Q154 记着这笔账的由来：从前报告是散文，错一格看不出来；表上头一次靠宽度吃饭。

use std::marker::PhantomData;

use crate::wrap;

/// 列与列之间空几格。**表画出来与列摆不摆得下按同一个数算**，因此在这里。
pub(super) const GAP: usize = 2;

/// 摆不下时省略号那一格：一列的内容从**中间**掐掉一截，留下的两头之间摆它。
///
/// 取 `⋯`（U+22EF）而不是 `…`（U+2026）：后者过不了
/// [`width_is_stable`](crate::wrap::width_is_stable) 那一关（停车场 Q154）。
/// 省略过的是名字那一列，它右边还有三列。
const ELLIPSIS: char = '⋯';

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
    /// 基准档，或者这一卷为什么没有一档（跳过、逐页、覆盖、没做成）。
    Base,
    /// 定档页：这一卷的档位是哪一页定出来的。
    Driver,
    /// 这一卷做了多久。
    Elapsed,
}

impl Column for VolumeColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Pages,
        Self::Base,
        Self::Driver,
        Self::Elapsed,
    ];

    /// **砍列的次序：耗时 → 定档页 → 页数。**
    ///
    /// 记号与卷名不在这里边——它们**恒在**：一行上先要认得出这是哪一卷、它出没出事。
    ///
    /// 次序按「摆不下时先舍谁」排：耗时最先——它是这一卷做完之后的一个旁证；
    /// 定档页次之——追下去要展开那一卷才看得清；页数压后——它是这一卷有多厚，
    /// 与卷名一起就已经是一句话。
    const DROPPED_IN_TURN: &'static [Self] = &[Self::Elapsed, Self::Driver, Self::Pages];

    const NARROWED: Self = Self::Name;

    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "卷名",
            Self::Pages => "页数",
            Self::Base => "基准档",
            Self::Driver => "定档页",
            Self::Elapsed => "耗时",
        }
    }

    /// 只有页数靠右：它是个数，一位数与三位数靠左摆就对不齐，而「这一卷比别的卷厚多少」
    /// 正是扫一眼要看出来的。其余各列都是词或名字，靠左。
    fn to_the_right(self) -> bool {
        matches!(self, Self::Pages)
    }
}

/// **逐页表**上的一列（`p3-session-legibility/11`）：展开一卷之后那一副。
///
/// 与[卷表](VolumeColumn)同一个形状——行首记号与名字恒在，其余按一个固定次序砍。
/// 列的选法答的是**展开一卷要问的那一件事**：哪一页把整卷的档位拉下来。
/// 因此从左到右是「这一页怎么样 · 是哪一页 · 多大 · 判成哪一档 · 凭什么 · 数是多少」，
/// 一路由结论走向证据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageColumn {
    /// 行首记号：这一页要不要紧，一个字符说完。**恒在。**
    Mark,
    /// 页名（成员名，只印最后那一段）。**恒在**，摆不下时从中间省略。
    Name,
    /// 这一页的输出尺寸。
    Size,
    /// 这一页判成的那一档。彩色分支与失败页没有这一格。
    Verdict,
    /// 判成这一档的理由。
    Reason,
    /// 各候选的判据值排成一串。
    Scores,
}

impl Column for PageColumn {
    const ALL: &'static [Self] = &[
        Self::Mark,
        Self::Name,
        Self::Size,
        Self::Verdict,
        Self::Reason,
        Self::Scores,
    ];

    /// **砍列的次序：判据 → 尺寸 → 理由。**
    ///
    /// 记号、页名与判定不在这里边——**判成哪一档就是这一副要答的那件事**，
    /// 而先要认得出这是哪一页、它要不要紧。
    ///
    /// 次序按「摆不下时先舍谁」排：判据一串最先——它是证据，比结论深一层，
    /// 而它也是这张表上最宽的一格；尺寸次之——宽溢出与兜底那两件事行尾那个词已经说了；
    /// 理由压后——它一个词就说清「这一档是怎么来的」，与判定挨着才读得懂。
    const DROPPED_IN_TURN: &'static [Self] = &[Self::Scores, Self::Size, Self::Reason];

    const NARROWED: Self = Self::Name;

    fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "页名",
            Self::Size => "尺寸",
            Self::Verdict => "判定",
            Self::Reason => "理由",
            Self::Scores => "判据",
        }
    }

    /// 一列都不靠右：尺寸是一对数中间夹着 `x`，靠右摆反而让 `x` 对不齐；
    /// 其余各列都是词或名字。
    fn to_the_right(self) -> bool {
        false
    }
}

/// 各列有多宽：**那一列上最长的一格**，列头也算一格。
///
/// 起手就是各列的列头（[`Widths::new`]），逐行往上撑（[`Widths::widen`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Widths<C: Column> {
    /// 一列一格，次序与 [`Column::ALL`] 相同。
    of: Vec<usize>,
    /// 量的是**哪一张表**的列。带上它，三张表的量不会串到一处去。
    which: PhantomData<C>,
}

impl<C: Column> Widths<C> {
    /// 起手：每一列先按它的**列头**量。
    pub(super) fn new() -> Self {
        let mut widths = Self {
            of: vec![0; C::ALL.len()],
            which: PhantomData,
        };
        for column in C::ALL {
            widths.widen(*column, column.head());
        }
        widths
    }

    /// 这一列上又来了一格：撑得宽就撑宽。
    pub(super) fn widen(&mut self, column: C, text: &str) {
        let width = usize::from(wrap::width(text));
        let slot = &mut self.of[column.at()];
        *slot = (*slot).max(width);
    }

    /// 这一列多宽。
    pub(super) fn of(&self, column: C) -> usize {
        self.of[column.at()]
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

/// **这么宽的一格上留得下哪几列**：按 [`Column::DROPPED_IN_TURN`] 那个次序砍，
/// 砍到摆得下为止。
///
/// 三列都砍完仍摆不下时就到此为止：恒在的那几列一列不让，
/// [名字那一列](Column::NARROWED)由 [`plan`] 收窄。
/// 屏再窄也要认得出这是哪一行、它出没出事——那正是这张表存在的理由。
pub(super) fn fit<C: Column>(room: usize, widths: &Widths<C>) -> Vec<C> {
    let mut kept = C::ALL.to_vec();
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

    /// **省略号那一格在哪种终端上都占一格**（判据见 [`crate::wrap::width_is_stable`]）。
    ///
    /// 它是这一层自己造的唯一一个字形，两张表的行首记号各在自己那一头问；
    /// 措辞那一层摆进列里的那几格在 `crate::render` 那一头问。
    #[test]
    fn the_ellipsis_this_module_makes_is_the_same_width_on_any_terminal() {
        assert!(wrap::width_is_stable(ELLIPSIS), "{ELLIPSIS} 是东亚歧义宽度");
        assert_eq!(usize::from(wrap::width(&ELLIPSIS.to_string())), 1);
    }

    /// 一份够宽的量：各列都比列头宽一点。
    fn measured() -> Widths<VolumeColumn> {
        let mut widths = Widths::new();
        widths.widen(VolumeColumn::Mark, "✓");
        widths.widen(VolumeColumn::Name, "棋魂 07");
        widths.widen(VolumeColumn::Pages, "184");
        widths.widen(VolumeColumn::Base, "2bit+FS");
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
        // 「基准档」六格，`2bit+FS` 七格：这一次是格撑着列头。
        assert_eq!(widths.of(VolumeColumn::Base), 7);
        // 中文两格：「棋魂 07」是 2+2+1+2 = 7 格，不是七个字符。
        assert_eq!(widths.of(VolumeColumn::Name), 7);
    }

    /// **给一个宽度，问该留哪几列**：按 `耗时 → 定档页 → 页数` 那个次序砍。
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
            VolumeColumn::Base,
            VolumeColumn::Driver,
        ];
        assert_eq!(fit(full - 1, &widths), without_elapsed);

        // 再窄：定档页跟着走。
        let without_driver = vec![
            VolumeColumn::Mark,
            VolumeColumn::Name,
            VolumeColumn::Pages,
            VolumeColumn::Base,
        ];
        assert_eq!(
            fit(line_width(&without_elapsed, &widths) - 1, &widths),
            without_driver
        );

        // 再窄：页数也让掉，剩下记号、卷名、基准档三列。
        let bare = vec![VolumeColumn::Mark, VolumeColumn::Name, VolumeColumn::Base];
        assert_eq!(fit(line_width(&without_driver, &widths) - 1, &widths), bare);
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

    /// **逐页那张表按它自己那个次序砍：判据 → 尺寸 → 理由**
    /// （`p3-session-legibility/11`）。
    ///
    /// 与卷表并排问一遍，钉的是「两张表各有各的次序，而砍的是同一套代码」：
    /// 记号、页名与**判定**在最窄那一档上仍在——展开一卷要答的正是「这一页判成哪一档」。
    #[test]
    fn the_per_page_table_drops_its_own_columns_in_its_own_order() {
        let mut widths: Widths<PageColumn> = Widths::new();
        widths.widen(PageColumn::Mark, "!");
        widths.widen(PageColumn::Name, "087.png");
        widths.widen(PageColumn::Size, "1182x1680");
        widths.widen(PageColumn::Verdict, "2bit+FS");
        widths.widen(PageColumn::Reason, "特例页单独定档");
        widths.widen(
            PageColumn::Scores,
            "1bit+FS 32.000 ⋅ 2bit 20.000 ⋅ 4bit 8.000 ⋅ 8bit 2.000",
        );
        let all = PageColumn::ALL.to_vec();
        let full = line_width(&all, &widths);

        assert_eq!(fit(full, &widths), all, "摆得下就一列都不砍");
        // 判据一串最先让掉：它是这张表上最宽的一格，也是比结论深一层的证据。
        let without_scores = vec![
            PageColumn::Mark,
            PageColumn::Name,
            PageColumn::Size,
            PageColumn::Verdict,
            PageColumn::Reason,
        ];
        assert_eq!(fit(full - 1, &widths), without_scores);
        // 再窄：尺寸走，理由留着。
        let without_size = vec![
            PageColumn::Mark,
            PageColumn::Name,
            PageColumn::Verdict,
            PageColumn::Reason,
        ];
        assert_eq!(
            fit(line_width(&without_scores, &widths) - 1, &widths),
            without_size
        );
        // 最窄那一档：记号、页名、判定三列。
        let bare = vec![PageColumn::Mark, PageColumn::Name, PageColumn::Verdict];
        for room in [0, 1, 5, 12, line_width(&without_size, &widths) - 1] {
            assert_eq!(fit(room, &widths), bare, "{room} 格上砍成了别的样子");
        }
    }

    /// **最窄那一档上卷名与行首记号仍在。**
    ///
    /// 砍到没得砍了也停在这三列上：一行上先要认得出这是哪一卷、它出没出事。
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
        let room = 16;

        let kept = plan(room, &mut widths);

        assert_eq!(
            kept,
            vec![VolumeColumn::Mark, VolumeColumn::Name, VolumeColumn::Base]
        );
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
