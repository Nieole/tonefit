//! 选档：在裁剪过的候选里定下这一页的那一个。
//!
//! 判据是量、阈值是界（`CONTEXT.md`）。这里做的是把量拿去和界比，选出**界以内最低的一档**——
//! 不是误差最小的那一档：误差最小的恒是候选上界，那样判定就白做了。
//!
//! 候选进来之前已经裁过两道（都在判据求值之前）：位深按面板灰阶数裁（ADR 0003），
//! 抖动模式按几何门裁（ADR 0007）。被裁掉的候选不在这里出现，`--bit-depth` 与 `--dither`
//! 也够不着它们——那两道界只有 `--gray-levels` 与几何本身动得了。
//!
//! 这里只有逐页判定。卷级的上包络、迟滞与离群页在 `envelope`，那一层建在这一层之上，
//! 并会把这里给出的档重定一遍（ADR 0006）。

use crate::metric::Score;
use crate::profile::Threshold;
use crate::quantize::Candidate;

/// 一个候选的判据值。
///
/// 判据是量、阈值是界：这里只有量。判据数值不可跨面板比较（ADR 0002），
/// 要看是哪块面板上的数，见 [`crate::Report::profile`]。
#[derive(Debug, Clone, Copy)]
pub struct CandidateScore {
    pub candidate: Candidate,
    pub score: Score,
}

/// 一页的判定：定下的那个候选，加上定它的理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub candidate: Candidate,
    pub reason: Reason,
}

/// 判定理由——这一档是怎么来的（spec 的 story 7：判定结果要可解释）。
///
/// 逐页与卷级两层共用本枚举，不是各起一个：两套并存的话，一份报告里
/// 「这一档为什么是它」就有两种读法，而判定可解释正是 story 7 要的东西。
/// 前三种由逐页判定给出，后三种由卷级汇总给出（`envelope`）；
/// spec 固定的 `Skipped` 随 11 号票落地。
///
/// 逐页的那三种能在最终报告里露面，只有在卷级那一层不在场的时候：
/// `--per-page` 关掉它，覆盖项顶掉它。
///
/// `Hysteresis` 在 spec 点名的那几种之外，理由在 ADR 0006 的后果里：
/// 上包络**不承诺**卷内绝对一致。升上去的那一段与主体之间就是一次翻页跳变，
/// 并进 `VolumeEnvelope` 就等于把这句话藏起来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 判据落在阈值以内的最低一档，比它更低的都越界了。
    LowestWithinThreshold,
    /// 没有一档的判据落在阈值以内：取候选里最高的那一档兜底。
    NoneWithinThreshold,
    /// 覆盖项裁到只剩一个候选，判定被顶掉（spec 的 story 23）。
    Override,
    /// 卷级上包络定的基准档：这一页跟着卷内主体走（ADR 0006 决定第 3 条）。
    VolumeEnvelope,
    /// 连续够了迟滞页数的一段，整段升到满足整段的最低一档（ADR 0006 决定第 4 条）。
    Hysteresis,
    /// 离群页单独定档：不参与上包络，按它自己那一档写出（ADR 0006 决定第 5 条）。
    Outlier,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Reason::LowestWithinThreshold => "阈值内最低的一档",
            Reason::NoneWithinThreshold => "没有一档在阈值内，取候选上界",
            Reason::Override => "覆盖项顶掉判定",
            Reason::VolumeEnvelope => "卷级上包络",
            Reason::Hysteresis => "迟滞升档",
            Reason::Outlier => "离群页单独定档",
        })
    }
}

/// 为一页定一个候选。
///
/// `scores` 是这一页各候选的判据值，由小到大——[`Candidate::all`] 就是这个次序，
/// 「最低的一档」靠的正是它。
///
/// `pinned` 是覆盖项裁到只剩一个候选时的那一个（见 `crate::pinned`）：判定被顶掉，
/// 判据说什么都不改变结果。裁到只剩一个而**没有**覆盖项的面板（`--gray-levels 2`
/// 撞上几何门不成立）不走这条路——那不是「被顶掉」，那一档仍是判出来的。
pub fn decide(
    scores: &[CandidateScore],
    threshold: Threshold,
    pinned: Option<Candidate>,
) -> Verdict {
    debug_assert!(
        scores
            .windows(2)
            .all(|pair| pair[0].candidate < pair[1].candidate),
        "候选必须由小到大：选的是最低的一档"
    );
    if let Some(candidate) = pinned {
        return Verdict {
            candidate,
            reason: Reason::Override,
        };
    }
    // 候选非空：面板灰阶数至少 2 级（`Profile::with_gray_levels` 挡着），1bit 恒在里面。
    let top = scores.last().expect("候选集不会是空的").candidate;
    match scores.iter().find(|scored| threshold.admits(scored.score)) {
        Some(scored) => Verdict {
            candidate: scored.candidate,
            reason: Reason::LowestWithinThreshold,
        },
        None => Verdict {
            candidate: top,
            reason: Reason::NoneWithinThreshold,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
    use crate::quantize::{BitDepth, Dither};

    /// 基准设备的阈值。选档的用例只关心「界在哪」，不关心它是几。
    fn threshold() -> Threshold {
        Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .threshold()
    }

    /// 造一组判据值，候选由小到大——`Candidate::all` 给的就是这个次序。
    fn scores(values: &[(Candidate, f32)]) -> Vec<CandidateScore> {
        values
            .iter()
            .map(|&(candidate, value)| CandidateScore {
                candidate,
                score: Score::from_value(value),
            })
            .collect()
    }

    /// 界以内最低的那一档。更高的档误差更小，但买不到判据看得见的东西。
    #[test]
    fn the_lowest_candidate_within_the_threshold_wins() {
        let threshold = threshold();
        let scores = scores(&[
            (Candidate::plain(BitDepth::One), threshold.value() * 2.0),
            (Candidate::plain(BitDepth::Two), threshold.value()),
            (Candidate::plain(BitDepth::Four), 0.0),
        ]);

        let verdict = decide(&scores, threshold, None);

        // 界上那一档算在界内：`admits` 取的是闭区间。
        assert_eq!(verdict.candidate, Candidate::plain(BitDepth::Two));
        assert_eq!(verdict.reason, Reason::LowestWithinThreshold);
    }

    /// 抖动是候选的另一维，选的规则一条不变：同一档位深上抖动排在不抖动之后，
    /// 因此不抖动过不了界、抖动过得了时，选出的是抖动那一个，而不是升一档位深
    /// （ADR 0007：上包络取的是这个组合，不设页级抖动开关）。
    #[test]
    fn a_dithered_candidate_wins_before_the_next_bit_depth_does() {
        let threshold = threshold();
        let scores = scores(&[
            (Candidate::plain(BitDepth::One), threshold.value() * 2.0),
            (
                Candidate::new(BitDepth::One, Dither::FloydSteinberg),
                threshold.value(),
            ),
            (Candidate::plain(BitDepth::Two), 0.0),
        ]);

        let verdict = decide(&scores, threshold, None);

        assert_eq!(
            verdict.candidate,
            Candidate::new(BitDepth::One, Dither::FloydSteinberg)
        );
        assert_eq!(verdict.reason, Reason::LowestWithinThreshold);
    }

    /// 一档都不达标时取候选上界兜底，理由要说出这是兜底——不是「选中了它」。
    #[test]
    fn nothing_within_the_threshold_falls_back_to_the_top_candidate() {
        let threshold = threshold();
        let scores = scores(&[
            (Candidate::plain(BitDepth::One), threshold.value() * 4.0),
            (Candidate::plain(BitDepth::Two), threshold.value() * 2.0),
        ]);

        let verdict = decide(&scores, threshold, None);

        assert_eq!(verdict.candidate, Candidate::plain(BitDepth::Two));
        assert_eq!(verdict.reason, Reason::NoneWithinThreshold);
    }

    /// 覆盖是覆盖判定，不是参与判定：判据说什么都不影响结果。
    #[test]
    fn an_override_wins_regardless_of_the_metric() {
        let threshold = threshold();
        let scores = scores(&[
            (Candidate::plain(BitDepth::One), 0.0),
            (Candidate::plain(BitDepth::Two), 0.0),
        ]);

        let verdict = decide(&scores, threshold, Some(Candidate::plain(BitDepth::Two)));

        assert_eq!(verdict.candidate, Candidate::plain(BitDepth::Two));
        assert_eq!(verdict.reason, Reason::Override);
    }

    /// 只剩一档而没有覆盖项（`--gray-levels 2` 撞上几何门不成立）：那一档无论达不达标
    /// 都是答案，但理由仍是判出来的那两种之一——不是「被顶掉」。
    #[test]
    fn a_single_candidate_is_still_reported_with_a_reason() {
        let threshold = threshold();
        let within = decide(
            &scores(&[(Candidate::plain(BitDepth::One), 0.0)]),
            threshold,
            None,
        );
        let beyond = decide(
            &scores(&[(Candidate::plain(BitDepth::One), threshold.value() * 2.0)]),
            threshold,
            None,
        );

        assert_eq!(within.candidate, Candidate::plain(BitDepth::One));
        assert_eq!(within.reason, Reason::LowestWithinThreshold);
        assert_eq!(beyond.candidate, Candidate::plain(BitDepth::One));
        assert_eq!(beyond.reason, Reason::NoneWithinThreshold);
    }
}
