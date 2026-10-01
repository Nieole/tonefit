//! **环节**在屏上叫什么、按什么次序、各在做什么（`CONTEXT.md` 的《环节》）。
//!
//! 屏上写环节名字的有四处：总览的当前卷那一行、卷列表上在跑的那几行（横条前那个词，
//! `super::shell::marks`）、全部按键那一张的环节一节（[`super::cover`]），以及每页结果的
//! 耗时那一行（[`took`]：各环节花了多久）——四处与词汇表逐字相同，**出处只在这里**。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）：全部按键那一张在外面，
//! 画横条的在里面，两边读的是同一份——反过来摆不成，画横条的那一层整个在特性后面。
//!
//! **自己一个模块，不并进 [`super::cover`] 或 `super::shell::marks`**：它不是覆盖层的一部分，
//! 也不是一个字形；`CONTEXT.md` 把《环节》当一个词，与《语义色》同级，那一个也自己一个模块
//! （[`super::tone`]）。

use std::time::Duration;

use tonefit::{Pass, VolumeTiming};

/// 环节**按走的次序**，各带屏上那个词与它在做什么（全部按键那一张的环节一节照这个次序列）。
///
/// 要摊开的卷走四个，不摊开的卷走后三个（《环节》）。「在做什么」那一句是设计稿
/// `PASS_WHAT` 的原话。
const PASSES: [(Pass, &str, &str); 4] = [
    // 固实归档开工前整卷解到临时目录，之后按目录卷走（`CONTEXT.md` 的《摊开》《读取形态》）。
    (Pass::Extraction, "摊开", ".rar / .7z 先整卷解到临时目录"),
    // 算出本卷指纹，与上一趟写在输出里的比（《幂等这一道》）。
    (Pass::Fingerprint, "查重", "之前转换过、源和设置没变就跳过"),
    // 解码、缩放、算画质分，定下每一页要哪一档（《分析环节》）。
    (Pass::First, "分析", "逐页解码、缩放，定下灰阶档位"),
    // 按定下的档量化、编码、写进输出（《写出环节》）。
    (Pass::Second, "写出", "照定下的档位编码，写进输出目录"),
];

/// 在走哪一个环节——**屏上那个词的唯一出处**。
///
/// **叫的是它在做什么，不是第几遍**（`two-pass-rework/01`）：「第一遍 / 第二遍」看得见
/// 进度在走，看不出在做什么、为什么非做两遍不可。
///
/// 兜底那一句（「这一遍」）不是遗漏：[`Pass`] 非穷尽，库多一个环节不该逼着这里跟着改。
pub fn name(pass: Option<Pass>) -> &'static str {
    match pass {
        // 开卷之后、第一条 `PassStarted` 到达之前：打开容器、列成员，还没走进任何一个环节。
        // 固实归档的摊开**不在这一段里**——它是一个环节，有自己那条 `PassStarted`。
        None => "开卷",
        Some(pass) => PASSES
            .iter()
            .find(|(listed, ..)| *listed == pass)
            .map_or("这一遍", |(_, said, _)| said),
    }
}

/// 全部按键那一张的环节一节：按走的次序，每个环节一行——屏上那个词与它在做什么。
pub fn legend() -> impl Iterator<Item = (&'static str, &'static str)> {
    PASSES.iter().map(|(_, said, what)| (*said, *what))
}

/// 一卷**各环节花了多久**：按走的次序，每一段原样取自[卷级计时](VolumeTiming)那一格
/// （`CONTEXT.md` 的《卷级计时》；每页结果的耗时那一行读它，`design-parity/10`）。
///
/// **是零的那一段不列**：库在那一格上记零，说的是「这一遍压根没走」（不摊开的卷没有摊开、
/// 跳过的卷没走分析与写出、确认点上答了不写出的那一卷没走写出），不是一个很小的数。
/// 总数（[`VolumeTiming::elapsed`]）不是一段，不在这里——它装着四段之外的那一截。
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
pub fn took(timing: &VolumeTiming) -> impl Iterator<Item = (Pass, Duration)> + '_ {
    PASSES
        .iter()
        .map(|(pass, ..)| (*pass, segment(timing, *pass)))
        .filter(|(_, took)| !took.is_zero())
}

/// 一个环节在卷级计时上是哪一段。**一个环节一段**（[`VolumeTiming`] 的文档）；
/// [`Pass`] 非穷尽，库多一个环节而这里还没认，那一段就当没走——不列，而不是编一个数。
fn segment(timing: &VolumeTiming, pass: Pass) -> Duration {
    match pass {
        Pass::Extraction => timing.extraction,
        Pass::Fingerprint => timing.fingerprint,
        Pass::First => timing.first_pass,
        Pass::Second => timing.second_pass,
        _ => Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **一卷各环节花了多久，逐段就是卷级计时那几段**（`design-parity/10`）：按走的次序，
    /// 每一段的时长原样取自 `VolumeTiming` 那一格；**是零的那一段不列**——零说的是
    /// 「这一遍压根没走」（不摊开的卷、跳过的卷、确认点上答了不写出的那一卷），不是一个很小的数。
    #[test]
    fn the_passes_walked_are_the_segments_of_the_volume_timing_in_walk_order() {
        let second = Duration::from_secs;
        let walked: Vec<(Pass, Duration)> = took(&VolumeTiming {
            extraction: second(62),
            fingerprint: second(4),
            first_pass: second(41),
            second_pass: second(18),
            elapsed: second(130),
        })
        .collect();
        assert_eq!(
            walked,
            [
                (Pass::Extraction, second(62)),
                (Pass::Fingerprint, second(4)),
                (Pass::First, second(41)),
                (Pass::Second, second(18)),
            ]
        );
        // 跳过的卷：只走了查重那一道；总数不是一段，不列。
        let skipped: Vec<(Pass, Duration)> = took(&VolumeTiming {
            extraction: Duration::ZERO,
            fingerprint: second(3),
            first_pass: Duration::ZERO,
            second_pass: Duration::ZERO,
            elapsed: second(5),
        })
        .collect();
        assert_eq!(skipped, [(Pass::Fingerprint, second(3))]);
    }

    /// 环节那个词说它在做什么；开卷之后、第一条 `PassStarted` 到达之前那一格不是一个环节。
    #[test]
    fn the_pass_says_what_it_does() {
        for (pass, said) in [
            (Some(Pass::Extraction), "摊开"),
            (Some(Pass::Fingerprint), "查重"),
            (Some(Pass::First), "分析"),
            (Some(Pass::Second), "写出"),
            (None, "开卷"),
        ] {
            assert_eq!(name(pass), said, "{pass:?}");
        }
    }

    /// 环节一节按走的次序列四个，词与横条上那个词是同一个（《环节》：三处逐字相同）。
    #[test]
    fn the_sheet_lists_the_four_passes_in_the_order_they_are_walked() {
        let said: Vec<&str> = legend().map(|(said, _)| said).collect();
        assert_eq!(said, ["摊开", "查重", "分析", "写出"]);
        for (pass, (said, _)) in [
            Pass::Extraction,
            Pass::Fingerprint,
            Pass::First,
            Pass::Second,
        ]
        .into_iter()
        .zip(legend())
        {
            assert_eq!(name(Some(pass)), said);
        }
    }
}
