//! 一张清单**只列前几条，剩下的报个数**——那个形状，一处出处（`p4-parking-lot/27`）。
//!
//! 一屏放不下的清单等于没有清单。仓库里要这么收口的地方有六处，从前各写各的：
//! 报告末尾那三小结（非卷文件 · 卷级失败 · 发现走不进去）、预扫那条拒绝、
//! 开工前撞名那条拒绝，加上几何门与纸白那两句里点名头几页的那一小截。
//!
//! # 共用的是哪两件，不共用的是哪两件
//!
//! **共用**：**列几条**（[`LIST_LIMIT`]）与**收口那一句怎么说**（`另有 N 个`），
//! 连同那一句在两副排版里各挂什么（见下一节）。
//!
//! **不共用**：**抬头那句话**与**条目怎么渲染**。六处的抬头一个字都不一样，
//! 量词各是各的（个／卷／处／页），条目有几行、缩几格更是各写各的——
//! 条目渲染是一个**闭包**参数，本模块一个字都不替它定。
//!
//! # 与 ADR 0016 的那道张力，摊开说
//!
//! ADR 0016 决定第 2 条**逐字点名**「点名的头几页 `001.png、002.png，另有 5 页`」
//! 是**措辞**，决定第 3 条说措辞只有 `render` 一处。收口那一句搬进库里，
//! 看起来正撞在这两条上。**它没有造出第二处出处，只是把那唯一一处挪进了库**——
//! 与互锁那几句话（`crate::Interlock` 的 `Display`）同一类：ADR 0016 认下的那处例外
//! 说的就是「同一句话要从几张嘴里出来，而其中一张在库内」。这里的几张嘴是
//! 报告末尾那三小结（bin）、预扫那条拒绝与撞名那条拒绝（**库自己写下的、
//! 直接落到 stderr 上的整段字**）。措辞留在界面层、由库那两处事后自己拼一遍，
//! 就是把这一句劈成两份——而票面要收的正是这件事。
//!
//! **ADR 原文没跟着改**：给一条决定添一支是改决定，不是订正说法（停车场 Q606）。
//!
//! # 与[字形约定](crate::glyph)不是同一条理由
//!
//! 那一条是「库会把自己造的字直接送进一条对齐的**列**里，规矩得跟着字递过去」；
//! 这一条是「**同一句话从几张嘴里出来，其中一张在库内**」。两条都让库担了一点界面层的事，
//! 但买的东西不同——不要并成一条说。
//!
//! # 两副排版
//!
//! [`FirstFew::stacked`] 摞成一块（一条一段，剩下的另起一行收口），
//! [`FirstFew::strung`] 串在一句话里（「、」连起来，剩下的接在后面收口）。
//! **两副都在本模块里**，添第三副也添在这里——「另有」那两个字只有一处出处，
//! `tests/single_source.rs` 钉着这一条。**不把那一句开成公共 API 让调用方自己挂**：
//! 那等于把「剩下的怎么说」重新摊回六处，而那正是本模块要收的东西。

/// **一张清单最多列几条。**
///
/// 取五：多到要收口的清单，前五条已经够读的人认出这是哪一类事、去源里对得上；
/// 再多几条并不会让他少做一件事，只会把报告刷满。
///
/// 点名头几页那两句**不取这个数**（见 [`FirstFew::at_most`]）：那一小截不是清单，
/// 是一句话里的抓手。
///
/// **它是 `pub` 的**，而库外没有一个调用方：`render` 在**另一个 crate** 里
/// （`src/render.rs` 属二进制侧），它那句「不取清单那个数」的文档要指得到这里。
pub const LIST_LIMIT: usize = 5;

/// 只列前几条的那张清单：**列出来的那几条**，加上**没列出来的还剩几条**。
///
/// 截断只发生在这一层——数据那一侧一条不少（`Report::non_volume_files`、
/// `Report::failed_volumes`、`Report::unreachable_places` 都是全的），
/// 要全部的调用方读那一列。
pub struct FirstFew<'a, T> {
    shown: &'a [T],
    rest: usize,
}

impl<'a, T> FirstFew<'a, T> {
    /// 上限取 [`LIST_LIMIT`]。
    pub fn of(items: &'a [T]) -> Self {
        Self::at_most(items, LIST_LIMIT)
    }

    /// 上限自己给。
    ///
    /// **给别的数要有别的理由**：几何门与纸白那两句里点名头几页取三，理由是那一小截
    /// 不是清单而是一句话里的抓手——真要逐页看，逐页那几行一页不落地列着
    /// （见 `render::first_few_names`）。没有这样一条理由的地方用 [`of`](Self::of)。
    pub fn at_most(items: &'a [T], limit: usize) -> Self {
        let (shown, rest) = items.split_at(items.len().min(limit));
        Self {
            shown,
            rest: rest.len(),
        }
    }

    /// 列出来的那几条。
    ///
    /// **三个内部件都不公开**（这一个连同 [`rest`](Self::rest) 与
    /// [`says_the_rest`](Self::says_the_rest)）：调用方要的是**一整段摆好的字**，
    /// 而两副排版都在本模块里。开出去等于给「剩下的怎么说」留第二条路
    /// ——那正是本模块要收的东西。第三副排版添在这里，不添在调用方那头。
    fn shown(&self) -> &'a [T] {
        self.shown
    }

    /// **没列出来的还剩几条**——不是总数。
    ///
    /// 抬头那一句报的是总数，两个数不是一回事：清单截断了，总数不许跟着截。
    fn rest(&self) -> usize {
        self.rest
    }

    /// **收口那一句**：`另有 3 个`。一条都不剩就一个字都不说。
    ///
    /// **量词由调用方给**（个／卷／处／页）：六处数的不是同一种东西。
    /// 前面挂什么、后面接不接换行是**排版**，归那一副
    /// （见 [`stacked`](Self::stacked) 与 [`strung`](Self::strung)）。
    ///
    /// 名字里不带 `and_`：`survey::Survey::into_volumes_and_the_rest` 已经占着那个说法，
    /// 指的是**剩下的那些东西**，而这一个出的是**说剩下多少的那句话**。
    fn says_the_rest(&self, unit: &str) -> Option<String> {
        (self.rest() > 0).then(|| format!("另有 {} {unit}", self.rest()))
    }

    /// **摞成一块的那一副**：一条一段照 `say` 写下来，剩下的另起一行收口。
    ///
    /// `say` 交出来的是**那一条的整段**，自带缩进与换行——条目有几行、缩几格，
    /// 六处各不相同（路径一行加原因一行、去处一行加来路几行），本模块不替它定。
    ///
    /// **收口那一行的缩进与前面那个省略号归这一副**（`  ……`）：五处从前各写一遍的
    /// 正是同一副，它与条目缩几格是两件事——条目那一副六处不同，这一副六处相同。
    pub fn stacked(&self, say: impl Fn(&T) -> String, unit: &str) -> String {
        let mut text: String = self.shown().iter().map(say).collect();
        if let Some(rest) = self.says_the_rest(unit) {
            text.push_str(&format!("  ……{rest}\n"));
        }
        text
    }

    /// **串在一句话里的那一副**：条目用「、」连起来，剩下的接在后面收口。
    ///
    /// 它出的是**一句话的一小截**，前后都没有换行——那一句怎么起头（「不成立：」
    /// 「没对上：」）归说它的那一处。
    pub fn strung(&self, say: impl Fn(&T) -> String, unit: &str) -> String {
        let listed: Vec<String> = self.shown().iter().map(say).collect();
        let listed = listed.join("、");
        match self.says_the_rest(unit) {
            Some(rest) => format!("{listed}，{rest}"),
            None => listed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 长清单只列前几条，剩下的报个数。
    ///
    /// **两件事一起问**：列出来的恰好是**头几条**（第六条起一条都不在），
    /// 剩下那个数是**没列出来的**、不是总数。只问头一件的话，
    /// 把 `rest` 改成 `items.len()` 这一条也是绿的。
    #[test]
    fn a_long_list_shows_the_first_few_and_counts_the_rest() {
        let items: Vec<usize> = (1..=7).collect();

        let listed = FirstFew::of(&items);

        assert_eq!(listed.shown(), &[1, 2, 3, 4, 5]);
        assert_eq!(listed.rest(), 2);
        assert_eq!(listed.says_the_rest("个").as_deref(), Some("另有 2 个"));
    }

    /// 短清单一条不落，也**不说「另有」**——一句「另有 0 个」比不说更糟。
    #[test]
    fn a_short_list_is_listed_whole_and_says_nothing_about_a_rest() {
        let items: Vec<usize> = (1..=3).collect();

        let listed = FirstFew::of(&items);

        assert_eq!(listed.shown(), &[1, 2, 3]);
        assert_eq!(listed.rest(), 0);
        assert_eq!(listed.says_the_rest("个"), None);
        assert_eq!(
            listed.stacked(|nth| format!("  {nth}\n"), "个"),
            "  1\n  2\n  3\n"
        );
        assert_eq!(listed.strung(|nth| nth.to_string(), "个"), "1、2、3");
    }

    /// **恰好到上限的那一档一条都不截，也不说「另有」**：`rest` 是零，不是一。
    ///
    /// 这一档是从前那几处各写各的 `saturating_sub` 与 `len() > SHOWN` 想拦的同一件事。
    ///
    /// 头一句问的是「一条都没少」而不是「列了 `LIST_LIMIT` 条」：后者两边同源
    /// （夹具本来就是照那个数造的），把 `at_most` 写成忽略入参它照旧绿。
    #[test]
    fn a_list_exactly_at_the_limit_is_listed_whole_and_says_nothing_about_a_rest() {
        let items: Vec<usize> = (1..=LIST_LIMIT).collect();

        let listed = FirstFew::of(&items);

        assert_eq!(
            listed.shown(),
            items.as_slice(),
            "恰好到上限那一档不许截掉谁"
        );
        assert_eq!(listed.rest(), 0);
        assert_eq!(listed.says_the_rest("个"), None);
    }

    /// **两副排版形状不同，收口那句话是同一句。**
    ///
    /// 同一张清单走两副：摞成一块的那一副一条一段、剩下的另起一行；串在一句里的那一副
    /// 用「、」连、剩下的接在后面。两副里「另有 2 处」逐字相同——共用的是句子那一半，
    /// 排版那一半各是各的。
    #[test]
    fn the_two_layouts_share_the_sentence_and_not_the_shape() {
        let items: Vec<&str> = vec!["甲", "乙", "丙", "丁", "戊", "己", "庚"];

        let listed = FirstFew::of(&items);

        assert_eq!(
            listed.stacked(|name| format!("  {name}\n"), "处"),
            "  甲\n  乙\n  丙\n  丁\n  戊\n  ……另有 2 处\n"
        );
        assert_eq!(
            listed.strung(|name| (*name).to_owned(), "处"),
            "甲、乙、丙、丁、戊，另有 2 处"
        );
    }

    /// 上限压得下去，而**压下去的是这一处的上限，不是那个常量**。
    ///
    /// 点名头几页那两句自己给一个更小的数（见 [`FirstFew::at_most`]）；
    /// 同一批条目走 [`FirstFew::of`] 仍列得更多。**两边的期望都是字面量**，
    /// 不是拿 [`LIST_LIMIT`] 算回来的——那样两边同源，把常量改成 3 也照旧绿，
    /// 而票面点名不许把那两个数拍平成一个。
    #[test]
    fn a_handhold_may_name_fewer_than_a_list_does() {
        let items: Vec<usize> = (1..=7).collect();

        let handhold = FirstFew::at_most(&items, 3);
        let list = FirstFew::of(&items);

        assert_eq!(handhold.shown(), &[1, 2, 3]);
        assert_eq!(handhold.rest(), 4);
        assert_eq!(list.shown(), &[1, 2, 3, 4, 5]);
        assert!(
            list.shown().len() > handhold.shown().len(),
            "清单那个上限被这一处的抓手拍平了"
        );
    }
}
