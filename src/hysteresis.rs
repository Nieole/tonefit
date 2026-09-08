//! 「一页说了不算」：两条路共用的那个页数，加上**没有基准档**时的那一种迟滞
//! （《段式迟滞》，`CONTEXT.md`；10 号票）。
//!
//! 卷级上包络关着时（`--per-page`），档位不再由基准档兜着，一页孤立地高出邻居
//! 就是一对翻页跳变——上去一次、下来一次。这里压掉的就是它们。
//!
//! **段式，不是跑动式。** 一页的档只取决于它前后几页，回看长度是 [`PAGES`] 定死的
//! 有界常数（见 [`pull_back`] 的《回看多长》）。跑动式要维护一个从卷首累积下来的当前档，
//! 回看长度不是常数、是整卷，滑窗幂等就无从谈起。
//!
//! 上包络那条路上的迟滞不在这里：那一种问的是「高于基准档」，没有基准档就问不出来，
//! 实现在 `envelope`（ADR 0006 决定第 4 条）。两处共用的只有 [`PAGES`]。

use crate::decide::{Reason, Verdict};
use crate::quantize::Candidate;

/// 迟滞页数：「一页说了不算」要连续多少页才算数。**未标定占位值**。
///
/// 两条路共用（ADR 0006 决定第 4 条与本模块），`envelope` 的 `Display` 把它连同
/// 「未标定」一起印出来。**共用是眼下的取法，不是一条决定**：标定那一趟要不要给两条路
/// 各标一个数，见停车场。
pub(crate) const PAGES: usize = 3;

/// 序列里的一页：它在整卷页序里的下标，加上逐页判定给它的那一档。
pub(crate) struct Page {
    /// 指进调用方那份逐页判定——[`pull_back`] 返回的下标原样是它。
    pub index: usize,
    /// 逐页判定定下的那一个（[`crate::decide::decide`]）。
    pub decided: Candidate,
}

/// 一截**极大同档段**：序列上的下标范围，加上这一段共用的那一档。
struct Run {
    range: std::ops::Range<usize>,
    depth: Candidate,
}

/// 把 `sequence` 上孤立地高出邻居的那些页压回邻居那一档。
/// 返回要改的那几页——没被压回的一页不在里面。
///
/// `sequence` 是按阅读顺序数「连续」的那条页序列。**同一套候选集的页才编得进同一条序列**：
/// 压回给出的那一档取自序列内某一页的判定，而候选集逐页可能不同（几何门逐页判，ADR 0007），
/// 混着数会把抖动发给一页门不成立的页。序列里缺的页不切断连续——它们整个不在其中，
/// 与上包络那一层数连续的方式一致（`envelope` 的 `raise`）。
///
/// # 规则
///
/// 序列按逐页判定切成**极大同档段**。一段够不上 [`PAGES`] 页时，看它两侧紧邻的段：
///
/// - **两侧都在、都要得更低** → 整段压回那两段里较高的那一档；
/// - **只有一侧**（这一段贴着序列的头或尾），那一侧要得更低、**而且它自己够 [`PAGES`] 页**
///   → 整段压回它那一档；
/// - 其余一律不压。
///
/// 三件事随之成立：
///
/// - **孤立偏离压回邻居那一档**：落点就是邻居那一档，字面意思。
/// - **够长的一段整段留住**：段够 [`PAGES`] 页就不问了。段内各页要的既然同为一档，
///   「满足整段的最低一档」就是它自己那一档（spec 的原话是「连续够长的一段**要求同一档**
///   就整段统一」——段按同档切，这半句因此恒真；混着档的长段不在这条规则的射程内）。
/// - **只降不升**：落点取的是邻居里较高的那一档，而「两侧都要得更低」这一条
///   保证它必然低于这一段自己那一档。升档是上包络那一层的事，这一层只压。
///
/// 候选的次序即体积由小到大（`quantize` 的 [`Candidate`]），压回因此是往体积小的那一侧走
/// ——**认下那笔降配**：被压回的页拿不到判据说它要的那一档，换来的是不为个别页抬邻居
/// （spec 的《认下那笔降配》）。降配**只落在孤立地高出邻居的页身上**，spec 认下的正是这一批。
///
/// # 不压的那三种
///
/// - **只比一侧高**——`[2bit, 2bit, 4bit, 8bit, 8bit]` 里那个 `4bit`。落点要取两个邻居里
///   较高的那一档，而这里那是 `8bit`：压它会变成**抬**它。「只降不升」正是靠
///   「两侧都要得更低」这一条成立的，这一种因此够不着。
/// - **比两侧都低**——`[4bit, 4bit, 1bit, 4bit, 4bit]` 里那个 `1bit`。压它得往上抬，
///   而抬是上包络那一层的事。更要紧的是**不能反过来让它把两侧的页拉下来**：
///   那两侧的页判据要的就是 `4bit`，它们不是孤立地高出邻居，spec 认下的那笔降配不含它们
///   （撞得上的不是造出来的输入：卷首两页封面接一页纸白就是）。
/// - **只有一侧邻居、而那一侧自己也够不上长度**——`[4bit, 4bit, 1bit, …]` 开头那两页。
///   一侧说了不算，除非那一侧本身够分量：邻段够 [`PAGES`] 页时它压得动
///   （`[4bit, 2bit, 2bit, 2bit, 2bit]` 里卷首那一页孤岛照压），够不上就两边都不算数。
///
/// # 回看多长
///
/// 一页的结果只取决于**它所在的那一段、加两侧紧邻的段各一页**（只有一侧邻段时，
/// 还要数那一段有多长）。段短于 [`PAGES`] 页才压得动，窗口因此不出
/// `[i-PAGES+1, i+PAGES-1]` 这 `2·PAGES-1` 页——**双向的有界常数**。
/// 双向是规则本身要的：只往前看的话，一段够长的偏离里排在段首的那几页会被段前的邻居压回，
/// 「整段留住」当场落空。
///
/// **这个界数的是 `sequence` 上的位置，不是卷内的页序。** 序列滤掉了彩页、失败页、
/// 部分救回页与另一门组的页（见 `crate::summarize_volume`），序列上紧挨着的两页
/// 在卷里可能隔着几页。P-D 的滑窗要照序列算，不是照页序算。
///
/// **一趟算完，看的全是逐页判定，不是压回途中的结果。** 有界正是这么来的：
/// 压回之后重新切段、再压一趟，回看长度就成了链长而不是常数。代价是一趟压不到底
/// ——`[1bit, 2bit, 8bit, 4bit, 1bit]` 这种一路不重样的序列上，压完 `8bit` 之后
/// 新冒出来的那个短段留在原地。一趟够不够，等标定迟滞页数那一趟（16 号票）
/// 拿真实素材看。
pub(crate) fn pull_back(sequence: &[Page]) -> Vec<(usize, Verdict)> {
    let runs = runs(sequence);
    let mut pulled = Vec::new();
    for (position, run) in runs.iter().enumerate() {
        if run.range.len() >= PAGES {
            continue;
        }
        let before = position.checked_sub(1).and_then(|index| runs.get(index));
        let after = runs.get(position + 1);
        let candidate = match (before, after) {
            // 两侧都在：都要得更低才算孤岛，落点取较高的那个邻居。
            (Some(before), Some(after)) if before.depth < run.depth && after.depth < run.depth => {
                before.depth.max(after.depth)
            }
            // 只有一侧：一侧说了不算，除非那一侧本身够分量。
            (Some(only), None) | (None, Some(only))
                if only.depth < run.depth && only.range.len() >= PAGES =>
            {
                only.depth
            }
            _ => continue,
        };
        pulled.extend(sequence[run.range.clone()].iter().map(|page| {
            (
                page.index,
                Verdict {
                    candidate,
                    reason: Reason::RunHysteresis,
                },
            )
        }));
    }
    pulled
}

/// 把序列切成极大同档段，按序列次序排开。
fn runs(sequence: &[Page]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (position, page) in sequence.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if run.depth == page.decided => run.range.end = position + 1,
            _ => runs.push(Run {
                range: position..position + 1,
                depth: page.decided,
            }),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::{BitDepth, Dither};

    /// 按逐页判定造一条序列，下标就是位置——端到端那一层才有「缺的页不切断连续」，
    /// 这一层只问规则本身。
    fn sequence(decided: &[Candidate]) -> Vec<Page> {
        decided
            .iter()
            .enumerate()
            .map(|(index, &decided)| Page { index, decided })
            .collect()
    }

    /// 走完一趟之后每一页那一档：没被压回的留着自己的。
    fn settled(decided: &[Candidate]) -> Vec<Candidate> {
        let mut settled = decided.to_vec();
        for (index, verdict) in pull_back(&sequence(decided)) {
            settled[index] = verdict.candidate;
        }
        settled
    }

    const ONE: Candidate = Candidate::plain(BitDepth::One);
    const TWO: Candidate = Candidate::plain(BitDepth::Two);
    const FOUR: Candidate = Candidate::plain(BitDepth::Four);
    const EIGHT: Candidate = Candidate::plain(BitDepth::Eight);

    #[test]
    fn an_isolated_page_is_pulled_back_to_its_neighbours() {
        assert_eq!(settled(&[TWO, TWO, FOUR, TWO, TWO]), [TWO; 5]);
    }

    #[test]
    fn a_run_of_exactly_the_hysteresis_length_keeps_its_depth() {
        assert_eq!(
            settled(&[TWO, TWO, FOUR, FOUR, FOUR, TWO, TWO]),
            [TWO, TWO, FOUR, FOUR, FOUR, TWO, TWO]
        );
    }

    /// 差一页就不算持续：整段压回，与上包络那条路上「差一页就不升档」是同一句话。
    #[test]
    fn a_run_one_page_short_is_pulled_back_whole() {
        assert_eq!(settled(&[TWO, TWO, FOUR, FOUR, TWO, TWO]), [TWO; 6]);
    }

    /// 阶梯的一级不是孤岛：落点要取两个邻居里较高的那一档，而这里那是 8bit——
    /// 压它会变成抬它。而 8bit 那一段两侧都更低、又够不上长度，照压。
    #[test]
    fn a_step_on_the_way_up_keeps_its_depth_while_the_short_top_is_pulled_back() {
        assert_eq!(
            settled(&[TWO, TWO, FOUR, EIGHT, EIGHT, TWO]),
            [TWO, TWO, FOUR, FOUR, FOUR, TWO]
        );
    }

    /// **一页要得更低时谁都不动。** 压它得往上抬，而抬是上包络那一层的事；
    /// 更要紧的是它不许把两侧的页拉下来——那两页判据要的就是 4bit，
    /// 它们不是孤立地高出邻居。撞得上这一条的是卷首两页封面接一页纸白。
    #[test]
    fn a_dip_pulls_nobody_down_with_it() {
        assert_eq!(
            settled(&[FOUR, FOUR, ONE, FOUR, FOUR]),
            [FOUR, FOUR, ONE, FOUR, FOUR]
        );
        assert_eq!(
            settled(&[FOUR, FOUR, ONE, FOUR, FOUR, FOUR, FOUR]),
            [FOUR, FOUR, ONE, FOUR, FOUR, FOUR, FOUR]
        );
    }

    /// 卷首卷尾的孤岛照压——只要那一侧的邻段自己够长。
    /// 一侧说了不算，除非那一侧本身够分量。
    #[test]
    fn an_island_at_either_end_is_pulled_back_when_its_one_neighbour_is_long_enough() {
        assert_eq!(settled(&[FOUR, TWO, TWO, TWO, TWO]), [TWO; 5]);
        assert_eq!(settled(&[TWO, TWO, TWO, TWO, FOUR]), [TWO; 5]);
    }

    /// 那一侧自己也够不上长度时两边都不算数——`[4bit, 4bit, 1bit, 4bit, 4bit]` 开头那两页
    /// 唯一的邻居是那个孤立的 `1bit`，压过去正是要挡的那笔降配。
    #[test]
    fn an_island_at_the_end_is_left_alone_when_its_one_neighbour_is_short_too() {
        assert_eq!(
            settled(&[FOUR, TWO, FOUR, FOUR, FOUR]),
            [FOUR, TWO, FOUR, FOUR, FOUR]
        );
    }

    /// 整条序列摆不下一个「前中后」时一页不动：那是上面两条的直接后件，不另开一条分支。
    #[test]
    fn a_sequence_too_short_to_have_a_middle_is_left_alone() {
        assert!(pull_back(&sequence(&[TWO, FOUR])).is_empty());
        assert!(pull_back(&sequence(&[FOUR])).is_empty());
        assert!(pull_back(&sequence(&[])).is_empty());
    }

    /// 抖动那一维照同一条规则压：次序即体积由小到大，`2bit` 之下是 `1bit+FS`。
    #[test]
    fn the_dither_dimension_is_pulled_back_by_the_same_rule() {
        let dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        assert_eq!(
            settled(&[dithered, dithered, TWO, dithered, dithered]),
            [dithered; 5]
        );
    }

    /// **回看是有界的**（票据第 1 条）：把远处那几页整片换掉，这一页的结果一个字都不变。
    /// 两条序列只在头四页上不同，第六页那个孤岛两边看到的邻居完全一样。
    #[test]
    fn a_page_is_decided_by_its_own_run_and_the_pages_beside_it() {
        let near = [TWO, TWO, TWO, TWO, TWO, FOUR, TWO, TWO, TWO];
        let far = [EIGHT, ONE, EIGHT, ONE, TWO, FOUR, TWO, TWO, TWO];
        assert_eq!(settled(&near)[5], TWO);
        assert_eq!(settled(&far)[5], TWO);
    }

    /// **一趟算完，看的全是逐页判定。** 压完 8bit 之后新冒出来的那个短段留在原地——
    /// 有界就是这么换来的（见 [`pull_back`] 的《回看多长》）。
    #[test]
    fn one_pass_over_the_original_verdicts_can_leave_a_new_short_run_behind() {
        assert_eq!(
            settled(&[ONE, TWO, EIGHT, FOUR, ONE]),
            [ONE, TWO, FOUR, FOUR, ONE]
        );
    }
}
