//! 视口：**一个列表在一个格子里露出来的那一段**（`CONTEXT.md` 的《会话》：视口）。
//!
//! 列表有多少行、格子有多高、光标停在第几行 → **从第几行画起**，外加一条滚动条
//! 画成什么样。屏上共用这一份的有哪几处、各是什么样，**只在 [`Viewport`] 那张表里**。
//!
//! # 滚动量是算出来的，不是记着的
//!
//! 这里没有一个「滚到哪儿了」的字段：**光标在哪儿，视口就跟到哪儿**。
//! 共用它的那几处因此不必各记一份滚动状态，也就不会出现「记着的那个数与列表对不上」
//! 那一种错——列表短了、光标跳出去了、格子变高了，下一帧算出来的都是对的。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）：
//! `--no-default-features` 那一趟照编、照跑它自带的用例。
//! **滚动条真画出来是画法那一层的事**（`super::draw::scrolling`），
//! 本模块只出「画成什么样」那三个数。

/// 格子高过这么多行时 [`Viewport::with_margin`] 才留那一行余量（`CONTEXT.md` 的《视口》；
/// 设计稿 `viewport` 的 `h > 8`）——矮格子里留一行就少露一行，不值。
const MARGIN_WHEN_TALLER_THAN: usize = 8;

/// 一个列表在一个格子里露出来的那一段：**从第几行画起**、露出几行、还剩几行没露面。
///
/// **屏上哪几处共用它，只有这一张表说了算**（别处引这一节，不再抄一遍名单）：
///
/// | 用在哪 | 光标 | 一行是什么 | 滚动条 |
/// |---|---|---|---|
/// | 左栏（`super::draw::config`，**连同就地摊开的取值栏**） | 有 | 屏上的一行 | 画 |
/// | 预设栏（`super::draw::picker`） | 有 | 屏上的一行 | 画 |
/// | 报告区的**卷表**（`super::draw::report`） | 有：**自动滚动着的时候停在末行，自动滚动停了就停在光标那一卷上** | 屏上的一行（表上一卷一行，成句的那几段折出来的每一行也算一行） | 画 |
/// | 补全候选（`super::draw::footer`） | 无 | 一条候选 | **不画**：它列而不选，一个键都不派，滚不动的东西画一条滚动条是在指一个按不动的地方；它说的是「还有 N 条」 |
/// | 覆盖层（`super::draw::overlay`） | **无**：那一格记的是「[从第几行画起](super::state::Covered::from)」 | 屏上的一行 | 画 |
///
/// 卷表那一处的光标**自动滚动着的时候停在末行**：算出来的起点因此恰好是「滚到底」，
/// 与它从前自己算的那一个逐格相同——报告只增不减，而「一卷跑完当场看得见」说的正是
/// 刚添上去的那一行；表底下当场冒出来的坏页与结束之后那几小结跟着一起看得见。
///
/// **自动滚动停了**（`p3-session-legibility/10`）之后它停在光标那一卷上，往回翻因此不必
/// 另有一个滚动量。**本类型一格没变**：自动滚动记的是「光标停在哪一卷」，不是「滚到哪儿了」
/// ——`CONTEXT.md` 的《视口》那句「滚动量是算出来的，不是记着的」照旧成立，
/// 而自动滚动是**卷表独有**的（`CONTEXT.md` 的《会话》：自动滚动），别处一处都不记它。
///
/// 报告区**展开**那一副也进来了（`p3-session-legibility/11`）：它从前有横竖两个滚动量，
/// 竖的那一个换成了[逐页表上那个光标](super::state::Expansion::at)，
/// 横的那一个连同「逐页不折行、横着滚」一起没了——那一副此刻是一张表，
/// 横向摆不下时**砍列**（`super::columns`）。**本类型仍旧一格没变。**
///
/// **覆盖层那一处没有光标**（`p3-session-legibility/12`）：它是**读物**——一行上没有
/// 第二步可走，一个光标都停不上去，而「视口跟着光标走」要先有个光标。那一格因此记着
/// **从第几行画起**，是屏上唯一记着滚动量的地方（`CONTEXT.md` 的《视口》那一句
/// 「滚动量是算出来的，不是记着的」在它身上是个例外，停车场 Q163）。**本类型仍旧一格
/// 没变**：交给它的是「露出来的最后一行」，越界照旧由它就近收。
///
/// **往下只滚到光标那一行还在格子里为止，不多滚一行**：列表短于格子时
/// [`Viewport::from`] 恒是零，那一格因此与没有这一段时逐格相同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Viewport {
    /// 列表一共有多少行。
    rows: usize,
    /// 格子有多高。
    height: usize,
    /// 从第几行画起。
    from: usize,
}

/// 一条滚动条画成什么样：**列表有多长、此刻停在第几行、格子里露出几行**。
///
/// 滑块画多长、画在哪一截，由这三个数定；**怎么画两副界面各一处**——旧那一副交给
/// 终端库自带的那个 widget（`ratatui::widgets::Scrollbar`，见 `super::draw::scrollbar`），
/// 新那一副照设计稿那两条式子自己算（见 `super::shell::canvas` 的 `scrollbar`：
/// 那个 widget 的位置取整与设计稿差在 `.5` 那一格上，而新界面要逐格相等）。
/// 过渡期两处，与两个砍列次序（停车场 Q804）是同一笔代价，合回一处是 15 号票的事
/// （停车场 Q850）。没有可滚的东西时根本拿不到这三个数（[`Viewport::scrollbar`] 给 `None`）。
///
/// 名字里的「条」是**滚动**条，与总览块那两条进度条（`super::draw::overview`）不是一回事。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Scrollbar {
    /// 列表一共有多少行。
    pub(super) rows: usize,
    /// 露出来的头一行是列表的第几行。
    pub(super) at: usize,
    /// 格子里露得下几行。
    pub(super) window: usize,
}

impl Viewport {
    /// 算一个视口：列表 `rows` 行、格子 `height` 行高、光标停在第 `cursor` 行。
    ///
    /// **`cursor` 越界不算错**：列表刚缩短、光标还停在原处时它会越界，
    /// 而那一帧仍旧要画得出来——就近收到最后一行上（`min`）。
    /// **没有光标的那一处传零**（补全候选：它列而不选）。
    ///
    /// `height` 是零时从头画起：那种格子一行都画不出来，滚到哪儿都一样，
    /// 而算出一个非零的起点只会让读代码的人以为它有意义。
    pub(super) fn new(rows: usize, height: usize, cursor: usize) -> Self {
        Self::at(rows, height, cursor, 0)
    }

    /// [`new`](Self::new) 那一手**留一行余量**：格子高过 [`MARGIN_WHEN_TALLER_THAN`] 行时光标停在格子倒数第二行上、
    /// 底下露着下一行，往下挪时看得见要去的地方（`CONTEXT.md` 的《视口》「格子高过 8 行时上下各留一行余量」）。
    /// 与设计稿的 `viewport` 同一个式子（停车场 Q775）——新界面的格子走这一手；
    /// 旧界面仍走 [`new`](Self::new)（它那几屏的快照不留余量，随那一副在 `session-redesign/15` 退场，停车场 Q789）。
    pub(super) fn with_margin(rows: usize, height: usize, cursor: usize) -> Self {
        Self::at(
            rows,
            height,
            cursor,
            usize::from(height > MARGIN_WHEN_TALLER_THAN),
        )
    }

    /// 两手共用的那一个式子：光标落在格子最后一行（留 `margin` 行余量时再往上 `margin` 行）上就够了，
    /// 再往下滚就是把已经看得见的东西滚掉——到了底停在 `rows - height`。
    /// 光标本来就在格子里时这个减法归零（`saturating_sub`），从头画起。
    fn at(rows: usize, height: usize, cursor: usize, margin: usize) -> Self {
        if height == 0 {
            return Self {
                rows,
                height,
                from: 0,
            };
        }
        let cursor = cursor.min(rows.saturating_sub(1));
        Self {
            rows,
            height,
            from: (cursor + 1 + margin)
                .saturating_sub(height)
                .min(rows.saturating_sub(height)),
        }
    }

    /// 从第几行画起。
    ///
    /// 出的是 `u16`：读它的是终端库的 `Paragraph::scroll`，而那一头收的就是 `u16`。
    /// 一格里画不下六万行，饱和在这里不会掉信息。
    pub(super) fn from(&self) -> u16 {
        u16::try_from(self.from).unwrap_or(u16::MAX)
    }

    /// 格子里露出来几行。
    pub(super) fn shown(&self) -> usize {
        self.rows.saturating_sub(self.from).min(self.height)
    }

    /// 还有几行没露面。**补全候选那一句「还有 N 条」就是它**。
    pub(super) fn hidden(&self) -> usize {
        self.rows.saturating_sub(self.shown())
    }

    /// 这一格的滚动条。**没有可滚的东西时给 `None`——那时不画**
    /// （列表短于格子、格子一行都没有、列表是空的，三种都落到这里）。
    pub(super) fn scrollbar(&self) -> Option<Scrollbar> {
        (self.height > 0 && self.hidden() > 0).then_some(Scrollbar {
            rows: self.rows,
            at: self.from,
            window: self.height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **光标在头、在尾，视口都跟得到**，而且不多滚一行。
    ///
    /// 十行的列表摆进四行的格子：光标在第一行时从头画起，一路走到最后一行时
    /// 从第七行画起——光标恰好落在格子的最后一行上。**再往下滚就是把看得见的东西滚掉**，
    /// 因此起点到 `rows - height` 为止。
    #[test]
    fn the_viewport_follows_the_cursor_to_either_end_and_not_a_row_further() {
        assert_eq!(
            Viewport::new(10, 4, 0).from(),
            0,
            "光标在头，却不是从头画起"
        );
        assert_eq!(Viewport::new(10, 4, 3).from(), 0, "光标还在格子里就滚了");
        assert_eq!(
            Viewport::new(10, 4, 4).from(),
            1,
            "光标走出格子一行，该滚一行"
        );
        assert_eq!(Viewport::new(10, 4, 9).from(), 6, "光标在尾，起点该是 10-4");

        // 每一步上光标都在格子里：起点 ≤ 光标 < 起点 + 格子高度。
        for cursor in 0..10 {
            let view = Viewport::new(10, 4, cursor);
            let from = usize::from(view.from());
            assert!(
                (from..from + view.shown()).contains(&cursor),
                "光标 {cursor} 掉出格子了：从 {from} 起画 {} 行",
                view.shown()
            );
        }
    }

    /// **列表短于格子：一点都不滚，也不画滚动条。**
    ///
    /// 常见的那几份预设、卷还没打进来的左栏都落在这一支上——那一格因此与
    /// 没有这一段时逐格相同（本票不许白改屏上的字节）。
    /// 恰好装满也算装得下：一行都没有掉在外面，画一条滚动条是在指一个空的地方。
    #[test]
    fn a_list_shorter_than_the_box_neither_scrolls_nor_gets_a_scrollbar() {
        for rows in 0..=4 {
            let view = Viewport::new(rows, 4, rows.saturating_sub(1));
            assert_eq!(view.from(), 0, "{rows} 行的列表滚了");
            assert_eq!(view.hidden(), 0, "{rows} 行的列表说还有东西没露面");
            assert_eq!(view.scrollbar(), None, "{rows} 行的列表画了滚动条");
        }

        // 多出一行就有滚动条了，而它说得出列表多长、停在哪儿、露出几行。
        let view = Viewport::new(5, 4, 4);
        assert_eq!(
            view.scrollbar(),
            Some(Scrollbar {
                rows: 5,
                at: 1,
                window: 4
            })
        );
        assert_eq!(view.hidden(), 1);
    }

    /// **格子高度为零：从头画起，一行都不露，滚动条不画。**
    ///
    /// 真会遇到：一格只剩上下两条框线时里面就是零行（左栏与预设栏都按
    /// `area.height - 2` 算），屏底那一格让完位之后给补全候选的也可能是零行。
    #[test]
    fn a_box_with_no_room_shows_nothing_and_draws_no_scrollbar() {
        let view = Viewport::new(10, 0, 7);
        assert_eq!(view.from(), 0, "画不出东西的格子不该算出一个起点来");
        assert_eq!(view.shown(), 0);
        assert_eq!(view.hidden(), 10, "一行都没露面");
        assert_eq!(view.scrollbar(), None, "画不出东西的格子还画了滚动条");
    }

    /// **光标跳到列表外：就近收到最后一行上，不恐慌、也不滚过头。**
    ///
    /// 列表刚缩短、光标还停在原处时就是这一副样子（左栏删掉一个卷、预设删掉一份）。
    #[test]
    fn a_cursor_past_the_end_lands_on_the_last_row() {
        let past = Viewport::new(10, 4, 99);
        assert_eq!(
            past.from(),
            Viewport::new(10, 4, 9).from(),
            "该与停在尾行同"
        );
        assert_eq!(past.from(), 6);

        // 列表是空的时候光标停在哪儿都一样：什么都没有可画。
        let empty = Viewport::new(0, 4, 3);
        assert_eq!(empty.from(), 0);
        assert_eq!(empty.shown(), 0);
        assert_eq!(empty.scrollbar(), None);
    }

    /// **留一行余量的那一手**：格子高过 8 行时光标停在倒数第二行上，到了底不多滚一行、
    /// 短列表照旧从头画起；格子不高过 8 行时与不留余量那一手逐格相同。
    #[test]
    fn the_margined_viewport_keeps_one_row_below_the_cursor_in_a_tall_box() {
        assert_eq!(Viewport::with_margin(30, 10, 0).from(), 0, "光标在头");
        assert_eq!(
            Viewport::with_margin(30, 10, 8).from(),
            0,
            "还在格子里（倒数第二行）"
        );
        assert_eq!(
            Viewport::with_margin(30, 10, 9).from(),
            1,
            "走到最后一行就滚一行"
        );
        assert_eq!(
            Viewport::with_margin(30, 10, 29).from(),
            20,
            "到了底不多滚一行"
        );
        assert_eq!(Viewport::with_margin(5, 10, 4).from(), 0, "短列表不滚");
        for cursor in 0..30 {
            assert_eq!(
                Viewport::with_margin(30, 8, cursor).from(),
                Viewport::new(30, 8, cursor).from(),
                "不高过 8 行的格子不留余量"
            );
        }
    }

    /// **没有光标的那一处传零**：从第一条起，剩下几条说得出来（补全候选就是这一副）。
    #[test]
    fn a_list_without_a_cursor_starts_at_the_top_and_counts_what_is_left() {
        let view = Viewport::new(40, 12, 0);

        assert_eq!(view.from(), 0);
        assert_eq!(view.shown(), 12);
        assert_eq!(view.hidden(), 28, "还有 28 条没露面");
    }
}
