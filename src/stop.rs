//! 按停止的两条规矩：**按一下升到哪一级**（[`next`]），与**一条事件上回哪个字**
//! （[`answer`]：确认点上的做完再停要让、立即停止不让）。ADR 0013，ADR 0012 决定第 2 条。
//!
//! **bin 里只有这一份，命令行与会话都调它**：命令行的 `Ctrl-C`（`crate::install_the_stop_key`）
//! 与会话的 `s`（`crate::session` 的 `Session::raise_stop`）按一下都走 [`next`]；两路的观察者
//! （`crate::Bar` 与 `crate::session` 的 `Watch`）在每一条事件上回的那个字都过一遍 [`answer`]。
//! 两级的语义不该因为按的地方不同而不同——两处各写一份，改了一处的人不会知道另一处也该改。
//!
//! # 为什么在 bin 里，不在库里
//!
//! 库那一侧只管在该问的地方问、记下观察者答过的最强那一级；**让不让、等不等人都是调用方的策略**
//! （ADR 0012 决定第 3 条，`CONTEXT.md` 的《进度》：观察者拿什么字来答是它自己的事）。
//! 这两条规矩说的正是「拿什么字来答」，因此一格库的对外形状都不添。
//!
//! # 这里不存东西
//!
//! 闩仍旧各在各的地方——命令行那一份、会话那一份、库那一份，各记各的、各由各的那一头往上推；
//! 它们的编码出自库的 `Instruction::code`。本模块只有三个纯函数：谁在哪一刻按到哪一级、
//! 记在哪儿，都不归它。
//!
//! **摆在 `session` 外面**：那个模块整个挂在 `any(feature = "tui", test)` 上，而命令行那一路
//! 在 `tui` 关掉的那一趟里也要调它（`docs/agents/gate.md` 的闸门 2）。

use tonefit::{Event, Instruction, Pass};

/// 按一下之后是哪一级：继续 → 做完再停 → 立即停止 → 立即停止（ADR 0013）。
///
/// **只升不降**是这张表的形状本身：升到立即停止之后它就是个不动点——第三下与第二下一个待遇，
/// 两级停止就是两级，哪一头都不新造第三种停法（ADR 0013 决定第 3 条）。
/// 库那一侧的闩用 `fetch_max` 说同一件事（[`Instruction`] 的序即力度）。
pub const fn next(pressed: Instruction) -> Instruction {
    match pressed {
        Instruction::Continue => Instruction::Finish,
        Instruction::Finish | Instruction::Abort => Instruction::Abort,
    }
}

/// 这一条事件是不是**确认点**——每一卷「汇总之后、写出环节之前」那一次问话
/// （ADR 0012 决定第 2 条，`CONTEXT.md` 的《会话》：确认点）。
///
/// 库那一侧只有这一条事件的答复**当场作数**，其余的都只进闩；[`answer`] 因此只在这一条上
/// 分岔。判定依据是事件本身，不是数到第几条——数下去的话，多一条事件就错位。
pub fn at_the_decision_point(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::PassStarted {
            pass: Pass::Second,
            ..
        }
    )
}

/// 在一条事件上回哪个字：**闩记着的那一级，只有确认点上的做完再停要让**。
///
/// 让的理由是两处问的不是同一件事（`CONTEXT.md` 的《会话》：确认点不是第三个检查点）——
/// 闩答的是「这一趟还走不走」，确认点问的是「**这一卷的写出环节还做不做**」。
/// 拿闩去答确认点，分析环节里按下的**做完再停**会顺手把当前卷的写出环节也吃掉：那一卷等于走了
/// 一次预览、盘上一个字节都没写，而做完再停的定义正是「当前卷跑完才停」（ADR 0013 决定第 1 条）。
/// 盘上会因此少一整卷——而那正是按下第一级的人要留下的那一卷。
///
/// **立即停止在确认点上不让**：那一级要的就是当前卷等于没做（ADR 0013 决定第 2 条），
/// 与页边界上按下它一个待遇。
///
/// 让掉的那一下**不会丢**：答复照样进库那一侧的闩，而那是个 `fetch_max`——记一个更弱的字
/// 进去不作数，闩仍是做完再停，当前卷跑完之后卷边界那个检查点照样停。
///
/// **这里不等人**：停下来问用户是会话那道闸的事（`crate::session` 的 `Gate`）。
/// 等不等人是调用方的策略（ADR 0012 决定第 3 条）：命令行从来不等；会话等人的那一趟
/// 也先过这一道，这一道只管那一下按停止不要把当前卷吃掉，等人只决定**让给谁**——
/// 让成继续的交给用户当场答，不让的连闸一起推开（`Running::stop` 推不推闸，问的也是这里）。
pub fn answer(at_the_decision_point: bool, pressed: Instruction) -> Instruction {
    match pressed {
        Instruction::Finish if at_the_decision_point => Instruction::Continue,
        pressed => pressed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **升级那张表，逐级**：按一下做完再停、再按一下立即停止、第三下原地不动（ADR 0013）。
    ///
    /// 一级一级点名，不只问「升一级不会变弱」：那一问放得过「继续一下跳到立即停止」——
    /// 那是只升不降的，却把做完再停整个跳过去了，按一次 `Ctrl-C` 当前卷就丢了。
    #[test]
    fn each_press_climbs_exactly_one_level_and_abort_is_where_it_stays() {
        assert_eq!(
            next(Instruction::Continue),
            Instruction::Finish,
            "按一次不是做完再停"
        );
        assert_eq!(
            next(Instruction::Finish),
            Instruction::Abort,
            "按两次不是立即停止"
        );
        assert_eq!(
            next(Instruction::Abort),
            Instruction::Abort,
            "立即停止之上又长出了一级，或者闩退回去了"
        );
    }

    /// **确认点上的做完再停要让，立即停止不让**；别处一律照闩答
    /// （`CONTEXT.md` 的《会话》：确认点不是第三个检查点）。
    ///
    /// 让的那一下**不会丢**——它照样进库那一侧的闩，当前卷跑完之后卷边界那个检查点照样停；
    /// 那一句由两路各自的观察者在一趟真跑上钉着（`crate::tests` 与 `crate::session` 的 `run`
    /// 各一条：卷跑到一半按一次做完再停，那一卷仍旧整卷落盘）。
    #[test]
    fn the_finish_press_gives_way_at_the_decision_point_and_the_abort_press_does_not() {
        // 确认点上：做完再停让成继续，另外两个原样。
        assert_eq!(
            answer(true, Instruction::Continue),
            Instruction::Continue,
            "没按过的那一趟被拦下了"
        );
        assert_eq!(
            answer(true, Instruction::Finish),
            Instruction::Continue,
            "做完再停在确认点上没让，当前卷的写出环节被吃掉了"
        );
        assert_eq!(
            answer(true, Instruction::Abort),
            Instruction::Abort,
            "立即停止在确认点上让了"
        );
        // 别处：三级一律照闩答——那几处的答复只进闩，而停在哪一道边界上是管线的事。
        for pressed in [
            Instruction::Continue,
            Instruction::Finish,
            Instruction::Abort,
        ] {
            assert_eq!(answer(false, pressed), pressed, "{pressed:?} 在别处被改了");
        }
    }
}
