//! 汇总：把逐页判定收成卷级的一个基准档（ADR 0006：位深按卷取上包络并加迟滞）。
//!
//! 夹在两遍之间的那一步——要看完整卷才做得了。逐页判定在 `decide`，这里只重定它给出的档。
//!
//! **这不是「整卷一个档」。** 离群页单独定档、迟滞升档，两者都会在卷内造成档位差
//! （ADR 0006 认下的代价），[`Envelope`] 因此把这两处各出了多少页原样摆出来——
//! 报告不许把上包络说成绝对一致。
//!
//! 四个数——上包络的分位、迟滞页数、离群页判据的立脚点分位与倍数——全部**未标定**
//! （ADR 0006），[`Envelope`] 的 `Display` 把这句话写在数值旁边。

use crate::decide::{CandidateScore, Reason, Verdict};
use crate::metric::{Score, nearest_rank};
use crate::profile::Threshold;
use crate::quantize::Candidate;

/// 上包络取的分位。**未标定占位值**。
///
/// 取上分位而非最大值：极端内容不该独自定全卷的档（ADR 0006 决定第 3 条）。
const ENVELOPE_QUANTILE: f64 = 0.95;

/// 迟滞页数：要连续多少页要求高于基准档才升档。**未标定占位值**。
///
/// 一页说了不算（ADR 0006 决定第 4 条）。
const HYSTERESIS_PAGES: usize = 3;

/// 离群页判据的倍数：判据要超过阈值的这么多倍，才算「显著偏离卷内分布」。**未标定占位值**。
const OUTLIER_FACTOR: f32 = 3.0;

/// 离群页判据的立脚点所在的分位：偏离量在这一档上量。**未标定占位值**。
///
/// 它不等于 [`ENVELOPE_QUANTILE`]，两者问的不是同一件事。上包络问「主体要哪一档」，
/// 答案该贴着分布的上沿走；立脚点问「主体过得去的是哪一档」，答案要**经得住离群页自己的拉抬**。
/// p95 当不了立脚点：越过它的页至多 5%，离群页一旦多于此就把立脚点抬进自己那一档，
/// 检验又在那一档上做，于是人人达标、一个都摘不出。而 ADR 0006 决定第 5 条举的例子——
/// 黑白 profile 下的彩页转灰——在一卷里占到一成是常态。
///
/// 再往下取也不行。取中位数的话，一卷里过半页要更高一档时（同一位画师换了种网点，
/// 后半卷整片抬一档），那些页会被当成离群页整片摘走，而它们才是这一卷的主体——
/// 摘完剩下的主体反倒成了少数派，卷级基准档跟着塌下去。
///
/// p75 把两侧的余量摆在一处。摘得出来的占比有个上界，越过它立脚点就站进了离群那一组——
/// 那时报出来的档说的是这一卷本身长什么样，不再是「几页偏离」
/// （与 [`summarize`] 里「一页不剩地落到离群侧」同一个道理）。
///
/// **这个上界随卷长变，长卷上才趋近四分之一**：留一之后剩 `n-1` 页，
/// 立脚点站在第 `⌈0.75(n-1)⌉` 名上，离群页要占到 `n+1-⌈0.75(n-1)⌉` 页才够把它抬进自己那一组。
/// 60 页要 16 页（26.7%），20 页要 6 页（30%），5 页要 3 页（60%），2 页要 2 页。
/// 短卷更宽不是漏洞，是同一件事的另一面：几页的卷本来就没有「分布」，
/// 而那时离群页那一层是唯一还挡着的防线（见 [`envelope`] 里上分位在短卷上的退化）。
const ANCHOR_QUANTILE: f64 = 0.75;

/// 汇总要看的那一页：逐页判定的结果，加上它是从哪条判据曲线来的。
pub(crate) struct Page<'a> {
    /// 这一页各候选的判据值，由小到大。候选集由面板灰阶数与几何门裁出，
    /// 全卷同一套（ADR 0003、ADR 0007）。
    pub scores: &'a [CandidateScore],
    /// 逐页判定定下的那一个（[`crate::decide::decide`]）。
    pub decided: Candidate,
}

/// 一个卷的上包络：基准档、定出它的那一页，以及卷内档位差各出在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    /// 卷内主体的基准档。抖动那一维在这里跟着位深一起定下（ADR 0007：上包络取的是这个组合）。
    pub base: Candidate,
    /// 定出基准档的那一页：主体页按逐页判定排开后，站在上分位秩上的那一页。
    /// 序号指进 [`crate::VolumeReport::pages`]。
    pub driver: usize,
    /// 参与上包络的主体页数。离群页不在内，部分救回页也不在——后者在进这一层之前
    /// 就被摘走了（04 号票，见 `crate::summarize_volume`）。
    pub body_pages: usize,
    /// 摘出去单独定档的离群页数。
    pub outlier_pages: usize,
    /// 因迟滞升档的页数。
    pub raised_pages: usize,
}

impl Envelope {
    /// 离群页占卷内**完好**灰度页的比例，0 到 1。彩页、失败页与部分救回页都不在分母里——
    /// 它们本来就不进上包络。
    ///
    /// 离群页机制是卷级分位聚合的安全网，而这个数是**这张网有没有张开**的唯一外部观测点。
    /// 只看 [`outlier_pages`](Self::outlier_pages)，「离群 0 页」读起来像「这一卷本来就没有
    /// 离群页」，而它同样可能是这张网整个失灵。占比把它摆到主体页数旁边，「零」由此有了刻度。
    ///
    /// 反过来，占比高本身也是话：高到立脚点已经站进那一档时，报出来的档说的是
    /// 这一卷本身长什么样，不再是「几页偏离」。那条界线**随卷长变**，短卷上宽得多，
    /// 算式在 `envelope` 的 `ANCHOR_QUANTILE`——所以这个数要连着页数一起读。
    pub fn outlier_share(&self) -> f64 {
        // 主体加离群就是灰度页的全部；`summarize` 挡掉了空卷，分母因此不会是零。
        self.outlier_pages as f64 / (self.body_pages + self.outlier_pages) as f64
    }
}

/// 汇总的产出：卷级的那一份，加上重定过的逐页判定。
pub(crate) struct Summary {
    pub envelope: Envelope,
    /// 与输入等长、同序：每一页最终定下的档与理由。
    pub verdicts: Vec<Verdict>,
}

impl Page<'_> {
    /// 这一页在 `candidate` 上的判据值。
    ///
    /// 候选集全卷同一套（ADR 0003、ADR 0007），因此每一页都有这一档。
    fn score_at(&self, candidate: Candidate) -> Score {
        self.scores
            .iter()
            .find(|scored| scored.candidate == candidate)
            .expect("候选集全卷同一套")
            .score
    }
}

/// 把逐页判定收成一个卷级的基准档。空卷没有上包络。
///
/// 两步，次序不能反：
/// 1. 摘出离群页（[`outlying`]），剩下的是主体；
/// 2. 在主体上取上包络定出基准档，再叠加迟滞（[`hysteresis`]）。
///
/// **这里不先取一次全卷的上包络。**离群检验要的立脚点整个在 [`outlying`] 以内逐页取，
/// 卷级聚合当不了它——那一档由全卷算出，被判的页自己也在里面
/// （`CONTEXT.md` 的《判据》：立脚点不含被判的这一页）。
pub(crate) fn summarize(pages: &[Page], threshold: Threshold) -> Option<Summary> {
    if pages.is_empty() {
        return None;
    }
    let all: Vec<usize> = (0..pages.len()).collect();
    let mut is_outlier = outlying(pages, threshold);
    // 一页不剩地落到离群侧，说明偏离的是这一卷本身，不是其中某几页：一页都不摘。
    // 候选上界都过不去的卷（`Reason::NoneWithinThreshold`，如 `--gray-levels 4` 撞上整卷灰调）
    // 就是这个局面——那时「远在界外」不再说明谁偏离了谁，而主体不能空着。
    if is_outlier.iter().all(|&taken| taken) {
        is_outlier.fill(false);
    }
    let body: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&index| !is_outlier[index])
        .collect();
    let (base, driver) = envelope(&body, pages);

    let mut verdicts = vec![
        Verdict {
            candidate: base,
            reason: Reason::VolumeEnvelope,
        };
        pages.len()
    ];
    for index in all.iter().copied().filter(|&index| is_outlier[index]) {
        verdicts[index] = Verdict {
            candidate: pages[index].decided,
            reason: Reason::Outlier,
        };
    }
    let raised_pages = hysteresis(&body, pages, base, threshold, &mut verdicts);

    Some(Summary {
        envelope: Envelope {
            base,
            driver,
            body_pages: body.len(),
            outlier_pages: pages.len() - body.len(),
            raised_pages,
        },
        verdicts,
    })
}

/// 上包络：把这些页的逐页判定排一遍，站在上分位秩上的那一页定出档位。返回 (候选, 那一页)。
///
/// 分位与判据的分块聚合共用最近秩取法（[`nearest_rank`]），不插值；
/// 排开的次序见 [`by_demand`]，同一卷跑两遍因此指出同一个驱动页。
///
/// **页数少到取不出分位时退化成判定最高的那一页**：p95 的秩在 20 页以内就是页数本身。
/// 这与判据的分块聚合是同一个取舍（见 `metric` 的 `aggregate`）——宁可严格，
/// 也不要把仅有的几页平均掉。代价是 ADR 0006 决定第 3 条要挡的「极端内容独自定全卷的档」
/// 在短卷上只剩离群页那一层挡着；短卷本来也没有「分布」可言。
///
/// `indices` 不得为空。
fn envelope(indices: &[usize], pages: &[Page]) -> (Candidate, usize) {
    let order = by_demand(indices, pages);
    let driver = order[nearest_rank(ENVELOPE_QUANTILE, order.len()) - 1];
    (pages[driver].decided, driver)
}

/// 这些页按逐页判定由低到高排开的次序。名次相同的按页序排，同一卷跑两遍因此排出同一条序列。
///
/// 上包络与离群检验的立脚点都站在这条序列的某个秩上（[`envelope`]、[`outlying`]），
/// 两者取的分位不同、秩不同，**序列必须是同一条**：一个说「主体要哪一档」、
/// 另一个说「其余页过得去的是哪一档」，两句话谈的得是同一个分布。
/// 排法写成一处，「同一条序列」才是构造出来的事实，而不是两边注释里的一句声称
/// （与 [`nearest_rank`] 同一个理由）。
fn by_demand(indices: &[usize], pages: &[Page]) -> Vec<usize> {
    let mut order = indices.to_vec();
    order.sort_by_key(|&index| (pages[index].decided, index));
    order
}

/// 离群页：判据显著偏离卷内分布的页（`CONTEXT.md`）。不参与上包络，单独定档
/// （ADR 0006 决定第 5 条）。
///
/// 偏离量在**立脚点**那一档上量（`CONTEXT.md` 的《判据》）：把被判的这一页从
/// [`by_demand`] 排出的序列里抽掉，站在 [`ANCHOR_QUANTILE`] 秩上的那一页就是它。
/// 卷内其余页过得去的那一档，离群页远远过不去。
///
/// 抽掉自己与分位取多少，两者各挡一种退化，缺一不可。**留一**挡「拿自己当尺子」：
/// 不抽的话立脚点站在全卷的第 `⌈0.75n⌉` 名上，而恰好站在那个秩上的那一页量的是**自己**
/// 那一档——按定义必在界内，它自己永远摘不出来。`n ≤ 3` 时那一页就是最极端的那一页，
/// 短卷整个失守；卷再长些，它也把摘得出来的占比上界整整放宽一页。
/// **分位**挡高占比，见 [`ANCHOR_QUANTILE`]。
///
/// **一页的卷没有离群页**：一页构不成分布，也就没有谁偏离谁——抽掉它就什么都不剩了。
///
/// 判据是**幅度**，判据形态里没有「连着几页」这一维：卷首连着几页的彩页转灰后仍是离群页，
/// 那正是 ADR 0006 决定第 5 条举的例子。成段与否只在迟滞那一层说话
/// （见 [`hysteresis`]），那一层管的是升不升档，不是摘不摘页。
fn outlying(pages: &[Page], threshold: Threshold) -> Vec<bool> {
    let mut taken = vec![false; pages.len()];
    // 抽掉被判的那一页之后一页不剩：一页的卷没有别人可比。
    if pages.len() < 2 {
        return taken;
    }
    // 立脚点从这条序列上取，与上包络同一条（[`by_demand`]）。
    let all: Vec<usize> = (0..pages.len()).collect();
    let order = by_demand(&all, pages);
    let mut place = vec![0usize; pages.len()];
    for (position, &index) in order.iter().enumerate() {
        place[index] = position;
    }
    // 抽掉一页之后剩 `len - 1` 页，立脚点站在其中这个秩上（0 起）。
    let rank = nearest_rank(ANCHOR_QUANTILE, pages.len() - 1) - 1;
    for (index, page) in pages.iter().enumerate() {
        // 抽掉的那一页排在秩之前，后面的整段前移一格；排在秩之后则不影响这个秩。
        // 一次排序换来逐页 O(1)，不必真的为每一页抽一遍。
        let position = if rank < place[index] { rank } else { rank + 1 };
        let anchor = pages[order[position]].decided;
        taken[index] = threshold.far_outside(page.score_at(anchor), OUTLIER_FACTOR);
    }
    taken
}

/// 迟滞：主体页里连续够了 [`HYSTERESIS_PAGES`] 页**基准档不够用**的，整段一起升档。
/// 返回升上去的页数。
///
/// 一页说了不算——升档要有持续的证据，否则翻页跳变的密度就退回逐页可变
/// （ADR 0006 决定第 4 条）。段不够长的那些页留在基准档上，也就得不到它们各自要的那一档：
/// 那是上分位明知故犯的另一半。
///
/// 「基准档不够用」判的是**基准档过不过得了这一页的界**，不是「判定排在基准档之后」。
/// 后者在位深那一维上与前者等价：判定是这一页最低的合格候选，格点套嵌保证更高的档也合格。
/// 抖动那一维上不等价——`1bit+FS` 排在 `2bit` 之前，判据却可能好过它，
/// 于是有页判定排在基准档**之前**、基准档却过不了它的界。照排序判会把这样的页
/// 静默写成越界档，而 ADR 0006 决定第 4 条说的本来就是「要求高于基准档」。
///
/// 整段升到**满足整段的最低一档**：与逐页判定同一条规则（界以内最低的一档）抬到段上，
/// 只是「整段」把界的检验摊到段内每一页上（见 [`lowest_for`]）。
/// 段内不会有离群页——那些已经摘走了，所以这一档抬不到极端内容上去。
///
/// 「连续」数的是**主体页**的序列，离群页整个不在其中。否则一页离群就能把一段持续的要求
/// 切成两截，而离群页恰恰爱出现在段的边上（彩页常在章节交界）。
fn hysteresis(
    body: &[usize],
    pages: &[Page],
    base: Candidate,
    threshold: Threshold,
    verdicts: &mut [Verdict],
) -> usize {
    let mut raised = 0;
    for run in runs(body.len(), |position| {
        !threshold.admits(pages[body[position]].score_at(base))
    }) {
        if run.len() < HYSTERESIS_PAGES {
            continue;
        }
        let stretch = &body[run];
        let candidate = lowest_for(stretch, pages, base, threshold);
        // 兜底那一路可能给回基准档本身：整卷在每一档上都过不去时就是这个局面
        // （见 [`summarize`] 里「一页不剩地落到离群侧」那一段）。那时一页都没升上去，
        // 段留在基准档上，理由仍是上包络，`raised_pages` 也不该把它算进来。
        if candidate == base {
            continue;
        }
        for &index in stretch {
            verdicts[index] = Verdict {
                candidate,
                reason: Reason::Hysteresis,
            };
        }
        raised += stretch.len();
    }
    raised
}

/// 满足整段的最低一档：基准档之上、段内每一页的判据都在界以内的那个最低候选。
///
/// 不能拿段内各页判定的最大值代替。位深那一维上两者相等——格点套嵌（见 `quantize`），
/// 位深升高只会让误差更小；抖动那一维上不成立：`2bit` 排在 `1bit+FS` 之后，
/// 却不因此就在每一页上都比它更贴近参照。规则本来就是「满足整段」，那就照着检验。
///
/// **只在基准档之上找。**迟滞是升档规则（ADR 0006 决定第 4 条），不是给一段页
/// 另配一档的口子：段内某一页要的那一档排在基准档之前时，照样得抬到基准档之上，
/// 否则卷内就多出一次向下的跳变，而 `raised_pages` 也不再是实话。
///
/// 一档都满足不了整段时退回基准档与段内判定的较高者——那是逐页那一层的兜底
/// （`Reason::NoneWithinThreshold`：没有一档在界内，取候选上界）抬到段上，
/// 同样不许往基准档以下走。
fn lowest_for(
    stretch: &[usize],
    pages: &[Page],
    base: Candidate,
    threshold: Threshold,
) -> Candidate {
    // 候选集全卷同一套，取段内任一页的那一份即可。
    pages[stretch[0]]
        .scores
        .iter()
        .map(|scored| scored.candidate)
        .filter(|&candidate| candidate > base)
        .find(|&candidate| {
            stretch
                .iter()
                .all(|&index| threshold.admits(pages[index].score_at(candidate)))
        })
        .unwrap_or_else(|| {
            stretch
                .iter()
                .map(|&index| pages[index].decided)
                .chain(std::iter::once(base))
                .max()
                .expect("段非空")
        })
}

/// `0..len` 里满足 `belongs` 的下标切成的极大连续段。
fn runs(len: usize, belongs: impl Fn(usize) -> bool) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    while start < len {
        if !belongs(start) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < len && belongs(end) {
            end += 1;
        }
        runs.push(start..end);
        start = end;
    }
    runs
}

impl std::fmt::Display for Envelope {
    /// 四个数一并说出，并标明都还没标定——报告不许把上包络说成绝对一致（ADR 0006）。
    ///
    /// 离群页数后面跟着占比（[`Envelope::outlier_share`]）：
    /// 「离群 0 页」要读得出它是相对多少页的零。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "基准档 {} · 主体 {} 页 · 离群 {} 页（{:.1}%）· 迟滞升档 {} 页\
             （上包络 p{} · 迟滞 {} 页 · 离群判据 p{} 立脚点、{:.1}× 阈值，四者均未标定）",
            self.base,
            self.body_pages,
            self.outlier_pages,
            self.outlier_share() * 100.0,
            self.raised_pages,
            (ENVELOPE_QUANTILE * 100.0).round(),
            HYSTERESIS_PAGES,
            (ANCHOR_QUANTILE * 100.0).round(),
            OUTLIER_FACTOR,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::GeometryGate;
    use crate::profile::Profile;
    use crate::quantize::{BitDepth, Dither};

    /// 基准设备的阈值。卷级的用例只关心「界在哪」，不关心它是几。
    fn threshold() -> Threshold {
        Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .threshold()
    }

    /// e-ink 面板 + 几何门不成立时的候选集：{1,2,4} 三档，全不抖动。
    /// 卷级的多数用例只在位深这一维上分胜负，抖动那一维单开两条用例。
    fn candidates() -> Vec<Candidate> {
        Candidate::all(16, GeometryGate::Broken { page: 0 })
    }

    /// 一卷合成的逐页判定。判据曲线要按页存下来，`Page` 借的就是它。
    struct Volume {
        scores: Vec<Vec<CandidateScore>>,
        decided: Vec<Candidate>,
    }

    impl Volume {
        /// 按「逐页判定 + 越界量」造一卷。
        ///
        /// 越界量是判定那一档**以下**各档的判据值：那些档全部越界，判定那一档起全部在界内——
        /// 逐页判定给出的正好是这一档。同一个数也是这一页在低档上的偏离量，
        /// 离群页判据看的就是它。
        fn new(pages: &[(Candidate, f32)]) -> Self {
            Self::over(candidates(), pages)
        }

        /// 同上，但点名候选集——抖动那一维要的是几何门成立时那一套六个候选。
        fn over(candidates: Vec<Candidate>, pages: &[(Candidate, f32)]) -> Self {
            let scores = pages
                .iter()
                .map(|&(decided, over)| over_curve(&candidates, decided, over))
                .collect();
            Self {
                scores,
                decided: pages.iter().map(|&(decided, _)| decided).collect(),
            }
        }

        /// 一卷在候选上界上也过不去的页：逐页判定只能取候选上界兜底
        /// （`Reason::NoneWithinThreshold`），判据在每一档上都远在界外。
        fn bottomed_out(count: usize) -> Self {
            let candidates = candidates();
            let top = *candidates.last().expect("候选非空");
            let scores = (0..count)
                .map(|_| {
                    candidates
                        .iter()
                        .map(|&candidate| CandidateScore {
                            candidate,
                            score: Score::from_value(far_out()),
                        })
                        .collect()
                })
                .collect();
            Self {
                scores,
                decided: vec![top; count],
            }
        }

        fn pages(&self) -> Vec<Page<'_>> {
            self.scores
                .iter()
                .zip(&self.decided)
                .map(|(scores, &decided)| Page { scores, decided })
                .collect()
        }

        fn summarize(&self) -> Summary {
            summarize(&self.pages(), threshold()).expect("卷非空")
        }
    }

    /// 一页的判据曲线：`decided` 以下各档都是 `over`，`decided` 起全部零误差。
    fn over_curve(candidates: &[Candidate], decided: Candidate, over: f32) -> Vec<CandidateScore> {
        candidates
            .iter()
            .map(|&candidate| CandidateScore {
                candidate,
                score: Score::from_value(if candidate < decided { over } else { 0.0 }),
            })
            .collect()
    }

    /// 刚过线的越界量：够让逐页判定往上走一档，够不上「显著偏离」。
    fn just_over() -> f32 {
        threshold().value() * 1.5
    }

    /// 远在界外的越界量：离群页判据要的就是这一量级。
    fn far_out() -> f32 {
        threshold().value() * OUTLIER_FACTOR + 1.0
    }

    /// 一卷 `count` 页，全部只要 1bit；`extra` 按 (下标, 逐页判定, 越界量) 插到指定位置上。
    fn volume_of(count: usize, extra: &[(usize, Candidate, f32)]) -> Volume {
        let mut pages = vec![(Candidate::plain(BitDepth::One), 0.0); count];
        for &(index, decided, over) in extra {
            pages[index] = (decided, over);
        }
        Volume::new(&pages)
    }

    /// 主体页共用一个基准档：判定低于基准档的页也照基准档定，翻页时不逐页变动。
    /// 体积不再最优是 ADR 0006 明知故犯的交换。
    #[test]
    fn the_body_pages_all_land_on_one_base_depth() {
        // 十页只要 1bit、十页要 2bit：上分位站在 2bit 上，全卷主体跟着走 2bit。
        let mut pages = vec![(Candidate::plain(BitDepth::One), 0.0); 10];
        pages.extend(vec![(Candidate::plain(BitDepth::Two), just_over()); 10]);
        let volume = Volume::new(&pages);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::Two));
        assert_eq!(summary.envelope.body_pages, 20);
        assert_eq!(summary.envelope.outlier_pages, 0);
        assert_eq!(summary.envelope.raised_pages, 0);
        assert!(
            summary.verdicts.iter().all(|verdict| verdict
                == &Verdict {
                    candidate: Candidate::plain(BitDepth::Two),
                    reason: Reason::VolumeEnvelope,
                }),
            "{:?}",
            summary.verdicts
        );
    }

    /// 上包络取的是 (位深, 抖动模式) 这个组合，不是位深单独一维（ADR 0007：
    /// 「上包络取的是这个组合，不设页级抖动开关」）：
    /// 抖动跟着位深一起按卷定下，全卷共用同一个抖动模式，页级没有开关。
    #[test]
    fn the_envelope_is_taken_over_the_whole_candidate_including_the_dither_mode() {
        let dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        // 十页不抖动就够，十页要抖：上分位站在抖的那一档上，全卷主体跟着抖。
        let mut pages = vec![(Candidate::plain(BitDepth::One), 0.0); 10];
        pages.extend(vec![(dithered, just_over()); 10]);
        let volume = Volume::over(Candidate::all(16, GeometryGate::Holds), &pages);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.base, dithered);
        // 升的是抖动那一维，位深一档没动——组合排序里 1bit+FS 就排在 2bit 之前。
        assert_eq!(summary.envelope.base.bit_depth, BitDepth::One);
        assert!(
            summary
                .verdicts
                .iter()
                .all(|verdict| verdict.candidate.dither == Dither::FloydSteinberg),
            "{:?}",
            summary.verdicts
        );
    }

    /// 驱动页是站在上分位秩上的那一页：它的判定就是基准档，而且它必是主体页。
    #[test]
    fn the_envelope_names_the_page_whose_demand_is_the_base() {
        let volume = volume_of(20, &[(7, Candidate::plain(BitDepth::Two), just_over())]);

        let summary = volume.summarize();
        let driver = summary.envelope.driver;

        assert_eq!(volume.decided[driver], summary.envelope.base);
        assert_eq!(summary.verdicts[driver].reason, Reason::VolumeEnvelope);
    }

    /// 一页说了不算：孤零零一页要求高于基准档，不足以让全卷升档。
    #[test]
    fn one_page_asking_for_more_does_not_move_the_volume() {
        let volume = volume_of(20, &[(9, Candidate::plain(BitDepth::Four), just_over())]);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::One));
        assert_eq!(summary.envelope.raised_pages, 0);
        assert_eq!(
            summary.verdicts[9].candidate,
            Candidate::plain(BitDepth::One)
        );
        assert_eq!(summary.verdicts[9].reason, Reason::VolumeEnvelope);
    }

    /// 连续够了迟滞页数才升档，整段一起升到满足整段的最低一档。
    #[test]
    fn a_run_long_enough_to_be_sustained_raises_that_stretch() {
        // 六十页里让三页连续要求更高：上分位的秩落在 57，这三页因此仍在基准档之上。
        let raised: Vec<_> = (30..33)
            .map(|index| (index, Candidate::plain(BitDepth::Four), just_over()))
            .collect();
        let volume = volume_of(60, &raised);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::One));
        assert_eq!(summary.envelope.raised_pages, 3);
        for index in 30..33 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::plain(BitDepth::Four)
            );
            assert_eq!(summary.verdicts[index].reason, Reason::Hysteresis);
        }
        // 段外的页一页不动：迟滞限的是切档频率，不是把全卷抬上去。
        assert_eq!(
            summary.verdicts[29].candidate,
            Candidate::plain(BitDepth::One)
        );
        assert_eq!(
            summary.verdicts[33].candidate,
            Candidate::plain(BitDepth::One)
        );
    }

    /// 整段升到的那一档要**满足整段**，不是段内判定的最大值。
    ///
    /// 段里一页要 `1bit+FS`、两页要 `2bit`，最大值是 `2bit`；但 `2bit` 在头一页上过不了界
    /// ——抖动那一维上没有位深那种格点套嵌的单调性。满足整段的最低一档因此是 `2bit+FS`。
    #[test]
    fn a_raised_stretch_lands_on_a_candidate_that_satisfies_every_page_in_it() {
        let all = Candidate::all(16, GeometryGate::Holds);
        let dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        let mut volume = Volume::over(
            all.clone(),
            &vec![(Candidate::plain(BitDepth::One), 0.0); 60],
        );
        for (index, decided) in [
            (30, dithered),
            (31, Candidate::plain(BitDepth::Two)),
            (32, Candidate::plain(BitDepth::Two)),
        ] {
            volume.decided[index] = decided;
            volume.scores[index] = over_curve(&all, decided, just_over());
        }
        // 要 `1bit+FS` 的那一页在 `2bit` 上也越界：这才是「取最大值」不够用的由来。
        let two_bit = all
            .iter()
            .position(|&candidate| candidate == Candidate::plain(BitDepth::Two))
            .expect("2bit 在候选集里");
        volume.scores[30][two_bit].score = Score::from_value(just_over());

        let summary = volume.summarize();

        assert_eq!(summary.envelope.raised_pages, 3);
        for index in 30..33 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
                "第 {index} 页升到了满足不了整段的那一档"
            );
        }
    }

    /// 主体页跟着基准档走，前提是基准档**过得了这一页的界**。判定排在基准档之前的页
    /// 不等于基准档够用它——抖动那一维上没有位深那种单调性，`1bit+FS` 排在 `2bit` 之前，
    /// 判据却可能好过它。这样的页要照样进迟滞那一段，否则会被静默写成越界档。
    #[test]
    fn a_page_the_base_cannot_admit_is_raised_even_when_it_sorts_below_the_base() {
        let all = Candidate::all(16, GeometryGate::Holds);
        let dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        // 五十七页只要 2bit，基准档落在它上面。
        let mut volume = Volume::over(
            all.clone(),
            &vec![(Candidate::plain(BitDepth::Two), just_over()); 60],
        );
        // 另三页连着：不抖动的两档都越界，`1bit+FS` 起在界内——判定因此排在基准档**之前**。
        for index in 30..33 {
            volume.decided[index] = dithered;
            volume.scores[index] = all
                .iter()
                .map(|&candidate| CandidateScore {
                    candidate,
                    score: Score::from_value(if candidate.dither == Dither::Off {
                        just_over()
                    } else {
                        0.0
                    }),
                })
                .collect();
        }

        let summary = volume.summarize();

        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::Two));
        assert_eq!(summary.envelope.raised_pages, 3);
        for index in 30..33 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
                "第 {index} 页被写成了基准档，而基准档过不了它的界"
            );
            assert_eq!(summary.verdicts[index].reason, Reason::Hysteresis);
        }
    }

    /// 差一页就不算持续：同样的内容少一页，全卷留在基准档上。
    #[test]
    fn a_run_one_page_short_of_the_hysteresis_stays_at_the_base() {
        let short: Vec<_> = (30..30 + HYSTERESIS_PAGES - 1)
            .map(|index| (index, Candidate::plain(BitDepth::Four), just_over()))
            .collect();
        let volume = volume_of(60, &short);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.raised_pages, 0);
        assert!(
            summary
                .verdicts
                .iter()
                .all(|verdict| verdict.candidate == Candidate::plain(BitDepth::One)),
            "{:?}",
            summary.verdicts
        );
    }

    /// 离群页：远在界外的那些页。不参与上包络，单独定到它自己要的那一档。
    #[test]
    fn a_page_far_outside_the_threshold_is_taken_out_and_decided_on_its_own() {
        let volume = volume_of(20, &[(4, Candidate::plain(BitDepth::Four), far_out())]);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.outlier_pages, 1);
        assert_eq!(summary.envelope.body_pages, 19);
        assert_ne!(summary.envelope.driver, 4, "离群页不该定出基准档");
        assert_eq!(
            summary.verdicts[4].candidate,
            Candidate::plain(BitDepth::Four)
        );
        assert_eq!(summary.verdicts[4].reason, Reason::Outlier);
        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::One));
    }

    /// 离群看的是幅度，不是「连着几页」：卷首连着几页的彩页仍然一页一页地摘出去。
    /// ADR 0006 决定第 5 条举的正是这个例子，它不能因为彩页成段就落回主体。
    #[test]
    fn a_stretch_of_pages_far_outside_the_threshold_is_a_stretch_of_outliers() {
        // 六十页里的三页。占比多少不改变这一条：三页占 5% 摘得出来，占一成同样摘得出来
        // （`a_tenth_of_the_volume_falling_far_outside_is_a_tenth_of_the_volume_taken_out`）。
        // 60 页的卷要占到 16 页，立脚点才站进这一组，那时它们不再是离群页，
        // 而是这一卷本身长这样——[`ANCHOR_QUANTILE`] 那一头的余量。
        let opening: Vec<_> = (0..3)
            .map(|index| (index, Candidate::plain(BitDepth::Four), far_out()))
            .collect();
        let volume = volume_of(60, &opening);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.outlier_pages, 3);
        assert_eq!(summary.envelope.body_pages, 57);
        assert_eq!(summary.envelope.raised_pages, 0);
        for index in 0..3 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::plain(BitDepth::Four)
            );
            assert_eq!(summary.verdicts[index].reason, Reason::Outlier);
        }
    }

    /// 离群页整个被摘出去：主体页的连续性是**去掉离群页之后**的那条序列。
    /// 否则一页离群就能把一段持续的要求切成两截，迟滞跟着失灵。
    #[test]
    fn an_outlier_does_not_break_the_run_its_neighbours_form() {
        let volume = volume_of(
            80,
            &[
                (10, Candidate::plain(BitDepth::Two), just_over()),
                (11, Candidate::plain(BitDepth::Two), just_over()),
                // 夹在中间的这一页远在界外，单独定档。
                (12, Candidate::plain(BitDepth::Four), far_out()),
                (13, Candidate::plain(BitDepth::Two), just_over()),
            ],
        );

        let summary = volume.summarize();

        assert_eq!(summary.envelope.outlier_pages, 1);
        assert_eq!(summary.verdicts[12].reason, Reason::Outlier);
        assert_eq!(
            summary.verdicts[12].candidate,
            Candidate::plain(BitDepth::Four)
        );
        // 10、11、13 在主体序列上是连着的三页，够一次升档。
        assert_eq!(summary.envelope.raised_pages, 3);
        for index in [10, 11, 13] {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::plain(BitDepth::Two)
            );
            assert_eq!(summary.verdicts[index].reason, Reason::Hysteresis);
        }
    }

    /// 六十页里六页远在界外，占一成：六页一页不少地摘出去，主体档不被它们抬高。
    ///
    /// ADR 0006 决定第 5 条举的例子——黑白 profile 下的彩页转灰——在一卷里占到一成是常态，
    /// 而立脚点必须扛得住这一成（见 [`ANCHOR_QUANTILE`]）。
    #[test]
    fn a_tenth_of_the_volume_falling_far_outside_is_a_tenth_of_the_volume_taken_out() {
        let summary = far_out_pages(60, 6).summarize();

        assert_eq!(summary.envelope.outlier_pages, 6);
        assert_eq!(summary.envelope.body_pages, 54);
        // 主体档不被这六页抬高：五十四页主体页要的仍然是 1bit。
        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::One));
        for index in 0..6 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::plain(BitDepth::Four)
            );
            assert_eq!(summary.verdicts[index].reason, Reason::Outlier);
        }
        for index in 6..60 {
            assert_eq!(
                summary.verdicts[index].candidate,
                Candidate::plain(BitDepth::One),
                "第 {index} 页被那六页拖着走了"
            );
        }
    }

    /// 一卷 `count` 页，只有末页远在界外。短卷那一串用例喂的都是它。
    fn one_outlier_in(count: usize) -> Volume {
        volume_of(
            count,
            &[(count - 1, Candidate::plain(BitDepth::Four), far_out())],
        )
    }

    /// 卷页数从 1 到 20 逐个走一遍：除了一页的卷，末页那张离群页每一档卷长都摘得出来。
    ///
    /// CBZ 章节包普遍落在这个区间，而这正是卷级分位站不住的那一段：上分位的秩在 20 页以内
    /// 就是页数本身。逐个钉，是因为这类退化只在某几个卷长上出现，挑一两个数试不出来。
    #[test]
    fn a_lone_outlier_is_taken_out_at_every_volume_length_up_to_twenty() {
        // 一页的卷没有别人可比：一页构不成分布，摘不出也不该摘。
        let alone = one_outlier_in(1).summarize();
        assert_eq!(alone.envelope.outlier_pages, 0);
        assert_eq!(alone.envelope.body_pages, 1);

        for count in 2..=20 {
            let summary = one_outlier_in(count).summarize();

            assert_eq!(
                summary.envelope.outlier_pages, 1,
                "{count} 页的卷没把末页那张离群页摘出来"
            );
            assert_eq!(summary.envelope.body_pages, count - 1);
            assert_eq!(
                summary.verdicts[count - 1].reason,
                Reason::Outlier,
                "{count} 页的卷里末页没被单独定档"
            );
            assert_eq!(
                summary.envelope.base,
                Candidate::plain(BitDepth::One),
                "{count} 页的卷被末页拖高了基准档"
            );
        }
    }

    /// 19 页与 20 页的相邻对照：同样的内容，多一页少一页，结论必须一样。
    ///
    /// 这一对是上一条用例里最要紧的那两格，单列出来指名道姓：`ceil(0.95n)` 在 n=19 时等于 n、
    /// 在 n=20 时是 19，卷级分位恰在这里从「秩落在排序末位」跨到「真的是个分位」。
    /// 离群判定不许跟着这条坎翻面——它站的是自己那个立脚点，不是上包络那一档。
    #[test]
    fn nineteen_pages_and_twenty_pages_reach_the_same_conclusion() {
        let nineteen = one_outlier_in(19).summarize();
        let twenty = one_outlier_in(20).summarize();

        assert_eq!(
            nineteen.envelope.outlier_pages,
            twenty.envelope.outlier_pages
        );
        assert_eq!(nineteen.envelope.base, twenty.envelope.base);
        assert_eq!(
            nineteen.verdicts[18].reason, twenty.verdicts[19].reason,
            "两卷的末页一个被摘出、一个没有"
        );
    }

    /// 摘得出来的占比有个上界，而这个上界**随卷长变**：
    /// 离群页占到 `n+1-⌈0.75(n-1)⌉` 页时，立脚点就站进了它们那一组，一页也摘不出来。
    ///
    /// [`ANCHOR_QUANTILE`] 的文档写着这条算式与它在几个卷长上的取值。那句话不许只是注释里的
    /// 声称——本票修的原缺陷，正是一句「上分位至多让 5% 的页越过它」被当成了成立的性质。
    /// 60 页与 5 页各喂一对相邻的 k，界线两侧因此各有一处钉子。
    #[test]
    fn one_page_past_the_bound_is_where_the_anchor_moves_in_and_nothing_is_taken_out() {
        // 界线随卷长变，短卷宽得多：60 页要 16 页才越界，5 页要 3 页。
        for (count, bound) in [(60, 16), (5, 3)] {
            let under = far_out_pages(count, bound - 1).summarize();
            assert_eq!(
                under.envelope.outlier_pages,
                bound - 1,
                "{count} 页的卷里 {} 页远在界外，界线以内，该一页不少地摘出来",
                bound - 1
            );

            let over = far_out_pages(count, bound).summarize();
            assert_eq!(
                over.envelope.outlier_pages, 0,
                "{count} 页的卷里 {bound} 页远在界外，立脚点已站进这一组，不该再摘出谁"
            );
            assert_eq!(over.envelope.body_pages, count);
        }
    }

    /// 一卷 `count` 页，头 `far` 页远在界外，其余只要 1bit。
    fn far_out_pages(count: usize, far: usize) -> Volume {
        let extreme: Vec<_> = (0..far)
            .map(|index| (index, Candidate::plain(BitDepth::Four), far_out()))
            .collect();
        volume_of(count, &extreme)
    }

    /// 一页不剩地落到离群侧时一页都不摘：偏离的是这一卷本身，主体不能空着。
    #[test]
    fn a_volume_that_is_entirely_far_outside_the_threshold_has_no_outliers_at_all() {
        let volume = Volume::bottomed_out(20);

        let summary = volume.summarize();

        assert_eq!(summary.envelope.outlier_pages, 0);
        assert_eq!(summary.envelope.body_pages, 20);
        assert_eq!(summary.envelope.base, Candidate::plain(BitDepth::Four));
        assert!(
            summary
                .verdicts
                .iter()
                .all(|verdict| verdict.reason == Reason::VolumeEnvelope),
            "{:?}",
            summary.verdicts
        );
    }

    /// 离群页占比进报告：「一页都没摘出来」得在报告里看得见。
    ///
    /// 这个数是这张安全网**有没有张开**的唯一外部观测点。只报计数的话，
    /// 「离群 0 页」读起来像「这一卷本来就没有离群页」，而它同样可能是判定整个失灵——
    /// 本票修的就是后一种。占比把它摆在主体页数旁边，两个读数一起说话。
    #[test]
    fn the_envelope_says_what_share_of_the_volume_was_taken_out() {
        let taken = far_out_pages(60, 6).summarize().envelope;
        assert_eq!(taken.outlier_share(), 0.1);
        assert!(taken.to_string().contains("10.0%"), "{taken}");

        // 一页都没摘出来的卷照样把这个数说出来，而不是省掉不提。
        let none = volume_of(60, &[]).summarize().envelope;
        assert_eq!(none.outlier_share(), 0.0);
        assert!(none.to_string().contains("0.0%"), "{none}");
    }

    /// 一页都没有的卷没有上包络：卷级基准档无从谈起。
    #[test]
    fn an_empty_volume_has_no_envelope() {
        assert!(summarize(&[], threshold()).is_none());
    }

    /// 四个数都没标定，报告要自己说出这一点（ADR 0006）。
    #[test]
    fn the_envelope_says_none_of_its_four_numbers_have_been_calibrated() {
        let volume = volume_of(20, &[(4, Candidate::plain(BitDepth::Four), far_out())]);

        let said = volume.summarize().envelope.to_string();

        assert!(said.contains("未标定"), "{said}");
        // 四个数各自都要露面，读的人才知道「未标定」说的是哪几个。
        assert!(said.contains("p95"), "{said}");
        assert!(said.contains(&format!("{HYSTERESIS_PAGES} 页")), "{said}");
        assert!(said.contains(&format!("{OUTLIER_FACTOR:.1}")), "{said}");
        assert!(
            said.contains(&format!("p{}", (ANCHOR_QUANTILE * 100.0).round())),
            "{said}"
        );
    }
}
