//! 开关之间的**互锁**：几项凑在一起时互相削弱的那些组合（页几何批 05 号票）。
//!
//! 全集是 [`Interlock::ALL`]，判定与处置都只写在这里。规则命令行现在就要用、
//! 会话层将来也要用，写两份必然漂移。
//!
//! - **哪几条咬上了**：[`Interlock::engaged`]（只看开关的那些）与
//!   [`Interlock::dither_outside_the_gate`]（还要一页的那一条）。
//! - **每一条在哪儿说话**：[`Interlock::voice`]。三条处置各不相同，这张表是它们在代码里的唯一出处。
//!
//! **那句话本身也在这里**（[`Interlock`] 的 `Display`）。措辞照例归界面层，
//! 这一处是有理由的例外：同一句要从三张嘴里出来——报告抬头、`--help`、以及那条拒绝的
//! 错误，而最后一张嘴在库内（见 `crate::dither_outside_the_gate_error`）。把它挪去界面层，
//! 库里那一份就成了第二个出处。变体自己的文档因此**不复述那句话**，只说它咬的是哪几项、
//! 为什么这么处置。
//!
//! **呈现仍不在这里**：话落在报告的哪一段、前面挂什么标签、`--help` 里排成什么样，
//! 由 `render` 与命令行各自决定。判定与措辞各有一个出处，呈现各按各的来——
//! 这就是「只写一处」的意思。

use crate::geometry::{FitMode, GeometryGate};
use crate::quantize::Dither;
use crate::spread::SplitRule;

/// 一条互锁：几项开关凑在一起互相削弱的那种组合。
///
/// **不是错误。** 每一条各有各的处置（见 [`Voice`]），只有一条拦住用户。
/// 咬上之后对用户说的那句话由本类型的 `Display` 给出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interlock {
    /// 拆分开着，适配方式却是 fit-inside。
    ///
    /// 咬的是 `--no-split`（默认拆）与 `--fit`：拆分收得下的是**找得到装订沟**的那些跨页，
    /// 找不到沟的连续跨页不切（页几何批 04 号票），它们照这一趟的适配方式出。
    ///
    /// 处置是 [`Voice::Header`]：组合本身成立，拿到的是**部分收益**，说清楚就够，不该拦。
    SpreadsStayFlattened,
    /// 裁边关着。
    ///
    /// 咬的是 `--no-crop` 与抖动：裁边的要点不是省白边，是让用户**关得掉阅读器那一侧的
    /// 裁切**（页几何批 02 号票），关掉之后那一侧就得留着。
    ///
    /// 处置是 [`Voice::Silent`]：抹平只在用户的阅读器**会裁**时才发生，而那一层是
    /// **像素完整性**、在 tonefit 视野之外（ADR 0007），逐卷提醒等于噪音。
    ReaderCropWipesTheDither,
    /// `--dither fs` 撞上一页贴不住面板。
    ///
    /// 咬的是 `--dither` 与**这一页的几何**。别的几条只看开关，这一条不是：
    /// 几何门逐页判（ADR 0007 决定第 1 条），碰上那一页之前答不出来。
    /// 判定因此不在 [`Interlock::engaged`] 里，在 [`Interlock::dither_outside_the_gate`]；
    /// 真撞上的地方是 `Candidates::broken`。
    ///
    /// 处置是 [`Voice::Refusal`]：覆盖项是用户的显式指令，
    /// 不是可以按页悄悄放弃的东西（ADR 0007 的《后果》）。
    DitherOutsideTheGate,
}

/// 一条互锁咬上之后，**这一趟**怎么对用户交代。
///
/// 每条处置各占一个变体，页几何批 05 号票定死。它不管 `--help`——那是说明书，
/// 全集无论处置是什么都在里面逐条列着（见命令行的 `interlock_help`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    /// 报告抬头提一次，此外不拦、不改任何东西。
    Header,
    /// 这一趟一个字都不说，只进 `--help` 与文档。
    Silent,
    /// 当场拒绝，整趟不做。
    Refusal,
}

impl Interlock {
    /// 互锁的全集，报告与 `--help` 都照它遍历。
    ///
    /// 加一条互锁就得在这里露面，否则 [`voice`](Self::voice) 的穷举、`--help` 那一节
    /// 与用例一起盖不到它。
    pub const ALL: [Interlock; 3] = [
        Interlock::SpreadsStayFlattened,
        Interlock::ReaderCropWipesTheDither,
        Interlock::DitherOutsideTheGate,
    ];

    /// 这一条在哪儿说话。**每条处置在代码里的唯一出处。**
    pub const fn voice(self) -> Voice {
        match self {
            Interlock::SpreadsStayFlattened => Voice::Header,
            Interlock::ReaderCropWipesTheDither => Voice::Silent,
            Interlock::DitherOutsideTheGate => Voice::Refusal,
        }
    }

    /// 这几项开关咬上了哪几条互锁，按 [`ALL`](Self::ALL) 的次序。
    ///
    /// 只答**开关咬得出来**的那几条。[`DitherOutsideTheGate`](Self::DitherOutsideTheGate)
    /// 不在里面，而那不是漏：它的触发条件里有一件不是开关（撞上的那一页贴不贴得住面板），
    /// 走 [`dither_outside_the_gate`](Self::dither_outside_the_gate)。
    /// 那句 `false` 因此不是死支——再添一条互锁时，它逼着人当场答一句
    /// 「开关答得出它吗」。
    ///
    /// 报告跑完之后问它（[`Report::interlocks`](crate::Report::interlocks)），
    /// 会话的设置面板在跑之前问它，而两处要说的是同一件事——那正是本票不许复制一份的东西。
    pub fn engaged(fit: FitMode, crop: bool, split: SplitRule) -> impl Iterator<Item = Interlock> {
        Interlock::ALL
            .into_iter()
            .filter(move |interlock| match interlock {
                Interlock::SpreadsStayFlattened => split.on && fit == FitMode::Inside,
                Interlock::ReaderCropWipesTheDither => !crop,
                Interlock::DitherOutsideTheGate => false,
            })
    }

    /// [`DitherOutsideTheGate`](Self::DitherOutsideTheGate) 在这一页上咬上了吗。
    ///
    /// 它单独一个入口，不并进 [`engaged`](Self::engaged)：别的几条只看开关，这一条还要一页——
    /// 几何门是**页**的几何事实，一卷里可能一页都不撞（ADR 0007 决定第 1 条）。
    ///
    /// **那条拒绝就由这一处判出来**（见 `crate::why_nothing_is_left`）：候选集抖动那一维被裁空
    /// 与这一问是同一件事，不是拿它去核对的第二个说法。
    ///
    /// 问的是「这一趟点名的那一档，门放不放行」——门放行哪几档只有
    /// [`Dither::candidates`] 一个出处，这里问它，不另写一遍「门拿走的是 FS」。
    /// 两件事因此是推论，不是各写一遍的巧合：点名「不抖动」撞不上（`Dither::Off`
    /// 在门的两侧都在里面），不点名也撞不上（没有覆盖项可顶，判据自己会替这一页
    /// 把抖动关掉）。
    ///
    /// **判据这么写，措辞没跟着广**：本条对门拿走的**任何**一档都成立，而
    /// [`DitherOutsideTheGate`](Self::DitherOutsideTheGate) 的 `Display` 写死着
    /// `--dither fs`。今天抖动只有两档、两者重合；再添一档就得同一口气改那句话，
    /// 否则拒绝报的是一档、说的是另一档。
    pub(crate) fn dither_outside_the_gate(dither: Option<Dither>, gate: GeometryGate) -> bool {
        dither.is_some_and(|named| !Dither::candidates(gate).contains(&named))
    }
}

impl std::fmt::Display for Interlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Interlock::SpreadsStayFlattened => {
                "拆分开着，适配方式却是 fit-inside：切得开的跨页切开了，\
                 没有装订沟的连续跨页仍被长边压扁。要那几页也用满面板高，得换 --fit height，\
                 代价是它们的宽溢出面板、要横向平移着看。组合本身成立——拿到的是部分收益，\
                 不是白开"
            }
            Interlock::ReaderCropWipesTheDither => {
                "裁边关着：阅读器那一侧的白边裁切就得留着，而它一裁就改了页尺寸——\
                 适配不再是 1.0 倍，抖动连同 1 像素周期的结构一起被抹平，字节白付。\
                 两台设备实测过（见 docs/measurements.md 的《真机像素完整性》）。\
                 阅读器那一层 tonefit 看不到，因此只在这里说一次，不进每趟报告"
            }
            Interlock::DitherOutsideTheGate => {
                "--dither fs 撞上一页贴不住面板：那一页源比目标尺寸还小，按不放大原样输出，\
                 阅读器显示时还要再缩一次，抖动推到高频的误差会被折回低频。\
                 几何门在它身上不成立，抖动因此关闭，--dither 覆盖不了它——\
                 门是页的几何事实，不是一个可以放宽的档位（ADR 0007）。\
                 撞上就整趟拒绝、不静默照抖：覆盖项是用户的显式指令，\
                 不是可以按页悄悄放弃的东西"
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Request;
    use crate::quantize::{BitDepth, Candidate};

    /// 三条处置照票面钉死（05 号票的处置 ①②③）。
    ///
    /// 这是本模块存在的理由：三条各自在哪儿说话是**领域决定**，不是渲染细节。
    /// 改了这张表，报告、`--help` 与那条拒绝会一起改口——那正是它该有的样子。
    #[test]
    fn each_interlock_says_where_it_speaks() {
        assert_eq!(Interlock::SpreadsStayFlattened.voice(), Voice::Header);
        assert_eq!(Interlock::ReaderCropWipesTheDither.voice(), Voice::Silent);
        assert_eq!(Interlock::DitherOutsideTheGate.voice(), Voice::Refusal);
    }

    /// ① 咬的是「拆分开着」× 「fit-inside」这一格，四种组合逐格扫一遍。
    ///
    /// 默认那一套（拆、以高为准）不咬：连续跨页在那条路上贴住面板高，收益是全的。
    #[test]
    fn spreads_stay_flattened_only_where_splitting_meets_fit_inside() {
        let split_on = SplitRule::default();
        let split_off = SplitRule {
            on: false,
            ..SplitRule::default()
        };
        let engaged = |fit, split| {
            Interlock::engaged(fit, true, split).any(|it| it == Interlock::SpreadsStayFlattened)
        };

        assert!(engaged(FitMode::Inside, split_on));
        // 拆分关着就没有「切得开的切开了」可言，压扁是用户点名要的那条路。
        assert!(!engaged(FitMode::Inside, split_off));
        // 以高为准上连续跨页贴住面板高，一条都不缺。
        assert!(!engaged(FitMode::Height, split_on));
        assert!(!engaged(FitMode::Height, split_off));
    }

    /// ② 只看裁边这一项：关着就咬上，适配方式与拆分改不了它。
    #[test]
    fn the_reader_crop_interlock_watches_only_the_crop_switch() {
        for fit in [FitMode::Height, FitMode::Inside] {
            for split in [
                SplitRule::default(),
                SplitRule {
                    on: false,
                    ..SplitRule::default()
                },
            ] {
                let engaged = |crop| {
                    Interlock::engaged(fit, crop, split)
                        .any(|it| it == Interlock::ReaderCropWipesTheDither)
                };
                assert!(engaged(false), "{fit:?} {split:?}");
                assert!(!engaged(true), "{fit:?} {split:?}");
            }
        }
    }

    /// ③ 要一页才答得出来：开关那一路一条都数不出它，问它自己那道判定才数得出。
    #[test]
    fn the_dither_interlock_needs_a_page_and_never_shows_up_among_the_switches() {
        // 开关那一路答不出它——`--dither fs` 点着也一样。
        for fit in [FitMode::Height, FitMode::Inside] {
            assert!(
                !Interlock::engaged(fit, false, SplitRule::default())
                    .any(|it| it == Interlock::DitherOutsideTheGate)
            );
        }

        // 它自己那道判定要两项一起成立：点名了 FS，而这一页贴不住面板。
        assert!(Interlock::dither_outside_the_gate(
            Some(Dither::FloydSteinberg),
            GeometryGate::Broken
        ));
        assert!(!Interlock::dither_outside_the_gate(
            Some(Dither::FloydSteinberg),
            GeometryGate::Holds
        ));
        // 点名「不抖动」撞不上：门拿走的只有抖动那一档。
        assert!(!Interlock::dither_outside_the_gate(
            Some(Dither::Off),
            GeometryGate::Broken
        ));
        // 不点名就没有覆盖项可顶，判据自己会把这一页的抖动关掉。
        assert!(!Interlock::dither_outside_the_gate(
            None,
            GeometryGate::Broken
        ));
    }

    /// **③ 咬上与「候选集抖动那一维被裁空」是同一件事**——那条拒绝因此只有这一处判它。
    ///
    /// 从前候选集那一侧另有一处形态（「两维一起裁完还剩几个」），两处靠一句
    /// `debug_assert!` 拴着，只在调试构建上验。这一条接下那份工，两种构建上都跑。
    ///
    /// 扫的是两个覆盖项 × 两种门的全部组合，逐格问三件事：裁完还剩不剩、
    /// `crate::why_nothing_is_left` 说不说得出话、以及说话的是不是本条。
    /// 第三问只在位深那一维过得去时问——两维一起对不上时报的是位深那一句
    /// （见 `crate::why_nothing_is_left`）。
    #[test]
    fn the_refusal_is_driven_by_this_interlock_alone() {
        let panel = crate::tests::request().profile.panel();
        let depths = BitDepth::candidates(panel.gray_levels);
        for gate in [GeometryGate::Holds, GeometryGate::Broken] {
            for bit_depth in std::iter::once(None).chain(BitDepth::ALL.map(Some)) {
                for dither in [None, Some(Dither::Off), Some(Dither::FloydSteinberg)] {
                    let request = Request {
                        bit_depth,
                        dither,
                        ..crate::tests::request()
                    };
                    // 独立数一遍：两维一起裁完，还剩不剩。这一路不问互锁。
                    let empty =
                        !Candidate::all(panel.gray_levels, gate)
                            .into_iter()
                            .any(|candidate| {
                                bit_depth.is_none_or(|named| candidate.bit_depth == named)
                                    && dither.is_none_or(|named| candidate.dither == named)
                            });
                    let said = crate::why_nothing_is_left(&request, gate);
                    let at = format!("{bit_depth:?} {dither:?} {gate:?}");

                    assert_eq!(empty, said.is_some(), "{at}");
                    assert_eq!(empty, crate::candidates(&request, gate).is_err(), "{at}");
                    if bit_depth.is_none_or(|named| depths.contains(&named)) {
                        assert_eq!(
                            said.is_some(),
                            Interlock::dither_outside_the_gate(dither, gate),
                            "{at}"
                        );
                    }
                }
            }
        }
    }

    /// 三条都说得出话，而且各说各的：措辞只有这一份，报告、`--help` 与那条拒绝都取它。
    #[test]
    fn every_interlock_says_what_it_is() {
        let said: Vec<String> = Interlock::ALL.iter().map(ToString::to_string).collect();
        for one in &said {
            assert!(!one.is_empty());
            // 终端文字，不是 markdown：强调符号会被原样印出来（页几何批 01 号票的自检）。
            assert!(!one.contains("**"), "{one}");
        }
        // 三句话互不相同——同一句说三遍等于三条里有两条没被说出来。
        for (index, one) in said.iter().enumerate() {
            assert!(!said[..index].contains(one), "{one}");
        }
    }
}
