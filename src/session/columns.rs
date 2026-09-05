//! 卷表的**列**：有哪几列、各多宽、这个宽度上留得下哪几列
//! （`CONTEXT.md` 的《会话》：卷表、砍列）。
//!
//! 表真画出来是画法那一层的事（`super::draw::table`）；本模块只答三件事：
//! **列的次序**、**砍列的次序**、以及**一格摆不下的字怎么省略**。
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
//! **它按 `UnicodeWidthChar::width` 算，不按 `width_cjk`**：`✓`／`✗`／`–`／`—`／`…`
//! 这几个记号在 Unicode 上是**东亚歧义宽度**，这里一律当一格。终端若按 CJK 配置
//! （歧义宽度算两格），这几格会比算出来的宽一格，同一列因此对不齐——
//! 这是仓库既有的约定（`crate::wrap` 那一头也是它），本模块跟着走，不另立第二套。
//! 停车场 Q154 记着这笔账：从前报告是散文，错一格看不出来；表上头一次靠宽度吃饭。

use crate::wrap;

/// 列与列之间空几格。**表画出来与列摆不摆得下按同一个数算**，因此在这里。
pub(super) const GAP: usize = 2;

/// 摆不下时省略号那一格：一列的内容从**中间**掐掉一截，留下的两头之间摆它。
const ELLIPSIS: char = '…';

/// 卷表上的一列。屏上从左到右就是 [`Column::ALL`] 那个次序。
///
/// **行首记号也是一列**：它与别的列一样要对齐、要量宽度，把它排除在外只会让画法那一层
/// 自己再算一遍它占几格。它与卷名一起是砍不掉的那两列（见 [`DROPPED_IN_TURN`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Column {
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

impl Column {
    /// 全部列，**从左到右**。表就按这个次序摆。
    pub(super) const ALL: [Self; 6] = [
        Self::Mark,
        Self::Name,
        Self::Pages,
        Self::Base,
        Self::Driver,
        Self::Elapsed,
    ];

    /// 列头。屏上那一行写的就是它。
    ///
    /// 这几个词**命令行上根本没有**：那一路把同一批格摆成一段散文，一个列头都不需要
    /// （见 [`crate::render::plain`]）。列头因此长在会话这一侧，与左栏那几行标签同一条。
    pub(super) fn head(self) -> &'static str {
        match self {
            Self::Mark => "记号",
            Self::Name => "卷名",
            Self::Pages => "页数",
            Self::Base => "基准档",
            Self::Driver => "定档页",
            Self::Elapsed => "耗时",
        }
    }

    /// 这一列的格**靠右摆**吗。
    ///
    /// 只有页数：它是个数，一位数与三位数靠左摆就对不齐，而「这一卷比别的卷厚多少」
    /// 正是扫一眼要看出来的。其余各列都是词或名字，靠左。
    pub(super) fn to_the_right(self) -> bool {
        matches!(self, Self::Pages)
    }

    /// 这一列在 [`Widths`] 里的第几格。
    fn at(self) -> usize {
        Self::ALL
            .iter()
            .position(|column| *column == self)
            .expect("每一列都在 ALL 里")
    }
}

/// **砍列的次序：耗时 → 定档页 → 页数。**
///
/// **只有这一处出处**（spec 的《卷表》）：画法那一层不许再写第二份次序，
/// 屏上砍成什么样一律问 [`fit`]。
///
/// 记号与卷名不在这里边——它们**恒在**：一行上先要认得出这是哪一卷、它出没出事，
/// 剩下的几列都是这两件之后的事。
///
/// 次序按「摆不下时先舍谁」排：耗时最先——它是这一卷做完之后的一个旁证；
/// 定档页次之——追下去要展开那一卷才看得清；页数压后——它是这一卷有多厚，
/// 与卷名一起就已经是一句话。
const DROPPED_IN_TURN: [Column; 3] = [Column::Elapsed, Column::Driver, Column::Pages];

/// 各列有多宽：**那一列上最长的一格**，列头也算一格。
///
/// 起手就是各列的列头（[`Widths::new`]），逐行往上撑（[`Widths::widen`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Widths([usize; Column::ALL.len()]);

impl Widths {
    /// 起手：每一列先按它的**列头**量。
    pub(super) fn new() -> Self {
        let mut widths = Self([0; Column::ALL.len()]);
        for column in Column::ALL {
            widths.widen(column, column.head());
        }
        widths
    }

    /// 这一列上又来了一格：撑得宽就撑宽。
    pub(super) fn widen(&mut self, column: Column, text: &str) {
        let width = usize::from(wrap::width(text));
        let slot = &mut self.0[column.at()];
        *slot = (*slot).max(width);
    }

    /// 这一列多宽。
    pub(super) fn of(&self, column: Column) -> usize {
        self.0[column.at()]
    }

    /// 把这一列**收窄**到这么多格。砍无可砍时卷名走这一条（见 [`elide`]）。
    pub(super) fn narrow(&mut self, column: Column, width: usize) {
        self.0[column.at()] = width;
    }
}

/// 这几列并排摆下来占几格：各列的宽度，加上列与列之间那几个 [`GAP`]。
pub(super) fn line_width(kept: &[Column], widths: &Widths) -> usize {
    let cells: usize = kept.iter().map(|column| widths.of(*column)).sum();
    cells + GAP * kept.len().saturating_sub(1)
}

/// **这么宽的一格上留得下哪几列**：按 [`DROPPED_IN_TURN`] 那个次序砍，砍到摆得下为止。
///
/// 三列都砍完仍摆不下时就到此为止：**记号与卷名恒在**，卷名那一列由 [`elide`] 收窄。
/// 屏再窄也要认得出这是哪一卷、它出没出事——那正是这张表存在的理由。
pub(super) fn fit(room: usize, widths: &Widths) -> Vec<Column> {
    let mut kept = Column::ALL.to_vec();
    for victim in DROPPED_IN_TURN {
        if line_width(&kept, widths) <= room {
            break;
        }
        kept.retain(|column| *column != victim);
    }
    kept
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
    // 而那一格摆到尾巴上多半正好再认出一个字（`消…卷` → `消…那卷`）。
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

    /// 一份够宽的量：各列都比列头宽一点。
    fn measured() -> Widths {
        let mut widths = Widths::new();
        widths.widen(Column::Mark, "✓");
        widths.widen(Column::Name, "棋魂 07");
        widths.widen(Column::Pages, "184");
        widths.widen(Column::Base, "2bit+FS");
        widths.widen(Column::Driver, "087.png");
        widths.widen(Column::Elapsed, "1m12s");
        widths
    }

    /// **一列有多宽是那一列上最长的一格，列头也算一格。**
    #[test]
    fn a_column_is_as_wide_as_its_widest_cell_and_its_head_counts_as_one() {
        let widths = measured();

        // 「记号」两个汉字四格，而记号本身一格：列头撑着这一列。
        assert_eq!(widths.of(Column::Mark), 4);
        // 「基准档」六格，`2bit+FS` 七格：这一次是格撑着列头。
        assert_eq!(widths.of(Column::Base), 7);
        // 中文两格：「棋魂 07」是 2+2+1+2 = 7 格，不是七个字符。
        assert_eq!(widths.of(Column::Name), 7);
    }

    /// **给一个宽度，问该留哪几列**：按 `耗时 → 定档页 → 页数` 那个次序砍。
    ///
    /// 这一条钉的是那个次序本身——它只有 [`DROPPED_IN_TURN`] 一处出处，
    /// 而屏上砍成什么样全由 [`fit`] 说了算。
    #[test]
    fn a_narrower_box_drops_its_columns_in_one_fixed_order() {
        let widths = measured();
        let all = Column::ALL.to_vec();
        let full = line_width(&all, &widths);

        assert_eq!(fit(full, &widths), all, "摆得下就一列都不砍");
        assert_eq!(fit(full + 40, &widths), all, "宽得多也不该多砍");

        // 窄一格：先砍耗时。
        let without_elapsed = vec![
            Column::Mark,
            Column::Name,
            Column::Pages,
            Column::Base,
            Column::Driver,
        ];
        assert_eq!(fit(full - 1, &widths), without_elapsed);

        // 再窄：定档页跟着走。
        let without_driver = vec![Column::Mark, Column::Name, Column::Pages, Column::Base];
        assert_eq!(
            fit(line_width(&without_elapsed, &widths) - 1, &widths),
            without_driver
        );

        // 再窄：页数也让掉，剩下记号、卷名、基准档三列。
        let bare = vec![Column::Mark, Column::Name, Column::Base];
        assert_eq!(fit(line_width(&without_driver, &widths) - 1, &widths), bare);
    }

    /// **最窄那一档上卷名与行首记号仍在。**
    ///
    /// 砍到没得砍了也停在这三列上：一行上先要认得出这是哪一卷、它出没出事。
    #[test]
    fn the_narrowest_table_still_has_its_marks_and_its_volume_names() {
        let widths = measured();

        for room in [0, 1, 2, 5, 10, 20] {
            let kept = fit(room, &widths);
            assert!(kept.contains(&Column::Mark), "{room} 格上砍掉了行首记号");
            assert!(kept.contains(&Column::Name), "{room} 格上砍掉了卷名");
            assert!(!kept.contains(&Column::Elapsed), "{room} 格上还留着耗时");
            assert!(!kept.contains(&Column::Driver), "{room} 格上还留着定档页");
            assert!(!kept.contains(&Column::Pages), "{room} 格上还留着页数");
        }
    }

    /// **头上没用完的那几格还给尾巴**：宽字符跨在预算边界上时不白扔。
    ///
    /// 「消失的那卷」十格收进七格：头上分到 3 格却只摆得下「消」（2 格），
    /// 剩下那一格还给尾巴，于是尾巴摆得下「那卷」而不是只有「卷」。
    /// 窄终端上卷名那一列本来就只有几格，白扔一格就少认出一个字。
    #[test]
    fn what_the_head_does_not_use_goes_back_to_the_tail() {
        assert_eq!(elide("消失的那卷", 7), "消…那卷");
        assert_eq!(usize::from(wrap::width("消…那卷")), 7);
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
        assert_eq!(elide("棋魂 07", 1), "…");
        // 两格里塞不下「棋」加省略号：头上那一格给不了半个汉字，那一格于是**还给尾巴**，
        // 正好摆得下卷号的末一位——半个字画不出来，一个窄字画得出来。
        assert_eq!(elide("棋魂 07", 2), "…7");
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

        assert_eq!(line_width(&[], &widths), 0, "一列都没有就不占地方");
        assert_eq!(line_width(&[Column::Mark], &widths), 4, "一列不加空");
        assert_eq!(
            line_width(&[Column::Mark, Column::Name], &widths),
            4 + GAP + 7
        );
    }

    /// **收窄之后那一列就是那么宽**：砍无可砍时卷名走的就是这一条。
    #[test]
    fn narrowing_a_column_makes_the_row_fit() {
        let mut widths = measured();
        let kept = vec![Column::Mark, Column::Name, Column::Base];
        let room = 16;

        let over = line_width(&kept, &widths).saturating_sub(room);
        widths.narrow(Column::Name, widths.of(Column::Name) - over);

        assert_eq!(line_width(&kept, &widths), room);
    }
}
