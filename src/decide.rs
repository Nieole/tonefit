//! 选档：在裁剪过的候选里定下这一页的位深。
//!
//! 判据是量、阈值是界（`CONTEXT.md`）。这里做的是把量拿去和界比，选出**界以内最低的一档**——
//! 不是误差最小的那一档：误差最小的恒是候选上界，那样位深判定就白做了。
//!
//! 候选进来之前已经按面板灰阶数裁过（ADR 0003：裁剪在判据求值之前）。
//! 被裁掉的位深不在这里出现，`--bit-depth` 也够不着它，那条上界只有 `--gray-levels` 动得了。
//!
//! 这里只有逐页判定。卷级的上包络与迟滞是 08 号票，它建在逐页判定之上。

use crate::metric::Score;
use crate::profile::Threshold;
use crate::quantize::BitDepth;

/// 一个候选的判据值。候选此刻只有位深这一维，抖动模式那一维随 09 号票加进来。
///
/// 判据是量、阈值是界：这里只有量。判据数值不可跨面板比较（ADR 0002），
/// 要看是哪块面板上的数，见 [`crate::Report::profile`]。
#[derive(Debug, Clone, Copy)]
pub struct CandidateScore {
    pub bit_depth: BitDepth,
    pub score: Score,
}

/// 一页的位深判定：定下的那一档，加上定它的理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub bit_depth: BitDepth,
    pub reason: Reason,
}

/// 判定理由——这一档是怎么来的（spec 的 story 7：判定结果要可解释）。
///
/// spec 固定的那四种（`VolumeEnvelope`、`Outlier`、`Override`、`Skipped`）是**卷级**的形状，
/// 随 08、11 号票落地。逐页判定这一层只出得起这三种。
///
/// 那两张票要把卷级的几种并进本枚举，不是另起一个：两套并存的话，一份报告里
/// 「这一档为什么是它」就有两种读法，而判定可解释正是 story 7 要的东西。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 判据落在阈值以内的最低一档，比它更低的都越界了。
    LowestWithinThreshold,
    /// 没有一档的判据落在阈值以内：取候选里最高的那一档兜底。
    NoneWithinThreshold,
    /// `--bit-depth` 覆盖了自动判定（spec 的 story 23）。
    Override,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Reason::LowestWithinThreshold => "阈值内最低的一档",
            Reason::NoneWithinThreshold => "没有一档在阈值内，取候选上界",
            Reason::Override => "--bit-depth 覆盖",
        })
    }
}

/// 为一页定一档位深。
///
/// `scores` 是这一页各候选的判据值，位深由小到大——[`BitDepth::candidates`] 就是这个次序，
/// 「最低的一档」靠的正是它。
pub fn decide(
    scores: &[CandidateScore],
    threshold: Threshold,
    override_depth: Option<BitDepth>,
) -> Verdict {
    debug_assert!(
        scores
            .windows(2)
            .all(|pair| pair[0].bit_depth < pair[1].bit_depth),
        "候选必须由小到大：选的是最低的一档"
    );
    if let Some(bit_depth) = override_depth {
        return Verdict {
            bit_depth,
            reason: Reason::Override,
        };
    }
    // 候选非空：面板灰阶数至少 2 级（`Profile::with_gray_levels` 挡着），1bit 恒在里面。
    let top = scores.last().expect("候选集不会是空的").bit_depth;
    match scores.iter().find(|scored| threshold.admits(scored.score)) {
        Some(scored) => Verdict {
            bit_depth: scored.bit_depth,
            reason: Reason::LowestWithinThreshold,
        },
        None => Verdict {
            bit_depth: top,
            reason: Reason::NoneWithinThreshold,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    /// 基准设备的阈值。选档的用例只关心「界在哪」，不关心它是几。
    fn threshold() -> Threshold {
        Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .threshold()
    }

    /// 造一组判据值，位深由小到大——`BitDepth::candidates` 给的就是这个次序。
    fn scores(values: &[(BitDepth, f32)]) -> Vec<CandidateScore> {
        values
            .iter()
            .map(|&(bit_depth, value)| CandidateScore {
                bit_depth,
                score: Score::from_value(value),
            })
            .collect()
    }

    /// 界以内最低的那一档。更高的档误差更小，但买不到判据看得见的东西。
    #[test]
    fn the_lowest_candidate_within_the_threshold_wins() {
        let threshold = threshold();
        let scores = scores(&[
            (BitDepth::One, threshold.value() * 2.0),
            (BitDepth::Two, threshold.value()),
            (BitDepth::Four, 0.0),
        ]);

        let verdict = decide(&scores, threshold, None);

        // 界上那一档算在界内：`admits` 取的是闭区间。
        assert_eq!(verdict.bit_depth, BitDepth::Two);
        assert_eq!(verdict.reason, Reason::LowestWithinThreshold);
    }

    /// 一档都不达标时取候选上界兜底，理由要说出这是兜底——不是「选中了它」。
    #[test]
    fn nothing_within_the_threshold_falls_back_to_the_top_candidate() {
        let threshold = threshold();
        let scores = scores(&[
            (BitDepth::One, threshold.value() * 4.0),
            (BitDepth::Two, threshold.value() * 2.0),
        ]);

        let verdict = decide(&scores, threshold, None);

        assert_eq!(verdict.bit_depth, BitDepth::Two);
        assert_eq!(verdict.reason, Reason::NoneWithinThreshold);
    }

    /// 覆盖是覆盖判定，不是参与判定：判据说什么都不影响结果。
    #[test]
    fn an_override_wins_regardless_of_the_metric() {
        let threshold = threshold();
        let scores = scores(&[(BitDepth::One, 0.0), (BitDepth::Two, 0.0)]);

        let verdict = decide(&scores, threshold, Some(BitDepth::Two));

        assert_eq!(verdict.bit_depth, BitDepth::Two);
        assert_eq!(verdict.reason, Reason::Override);
    }

    /// 只剩一档的面板（`--gray-levels 2`）：那一档无论达不达标都是答案，理由仍要分得清。
    #[test]
    fn a_single_candidate_is_still_reported_with_a_reason() {
        let threshold = threshold();
        let within = decide(&scores(&[(BitDepth::One, 0.0)]), threshold, None);
        let beyond = decide(
            &scores(&[(BitDepth::One, threshold.value() * 2.0)]),
            threshold,
            None,
        );

        assert_eq!(within.bit_depth, BitDepth::One);
        assert_eq!(within.reason, Reason::LowestWithinThreshold);
        assert_eq!(beyond.bit_depth, BitDepth::One);
        assert_eq!(beyond.reason, Reason::NoneWithinThreshold);
    }
}
