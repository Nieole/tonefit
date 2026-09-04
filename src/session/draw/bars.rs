//! 屏上那两块：主区最上面那两条横条——**全局条**（卷数 · 剩余时间）与
//! **当前卷条**（在走哪一遍 · 步数）。
//!
//! 两条都只读那一趟边跑边攒的那一份（[`Live`]），一个字都不在这里重编：卷名走
//! [`crate::render::volume_name`]、收场那一句走 [`crate::render::outcome`]、
//! 按停按到的那一级走 [`super::footer::stopping_name`]。横条画多宽与命令行那两条
//! 同一个出处（[`BAR_WIDTH`]）。
//!
//! 主区第三段是报告区，在 [`super::report`]；三段各占几行由 [`super::main_pane`] 分。
//!
//! 长在本模块的只有**命令行上根本没有**的那一样：这两条横条的排版
//! （命令行那两条是 indicatif 的模板，见 `crate::bar_style`）。

use std::cmp::Ordering;
use std::time::Duration;

use ratatui::widgets::{Block, Borders, Paragraph};
use tonefit::{Instruction, Pass};

use super::footer::{START_KEYS, stopping_name};
use crate::session::live::{Live, Walking};

/// 全局条与当前卷条各占几行：一行正文加上下两条边。
pub(super) const BAR_HEIGHT: u16 = 3;

/// 一条横条画多宽。**与命令行那两条同一个出处**（`crate::BAR_WIDTH`）：
/// 两处的横条长得一样，读的人不必重新认一遍。
const BAR_WIDTH: u64 = crate::BAR_WIDTH as u64;

/// 全局条：**卷数与剩余时间**——长任务里唯一有人真想知道的两个数（ADR 0011）。
///
/// 两个数都出自开工那条事件（`RunStarted`），而它是**预扫**算出来的（03 号票）。
/// 预告的步数是**上界**不是承诺（`CONTEXT.md` 的《进度》），剩余时间因此也是估计：
/// 幂等命中的卷提前收摊，剩下的那一截会突然缩短。
///
/// **按停按到哪一级挂在这一格的抬头上**（停车场 Q71）：按下收尾之后横条照旧往前走，
/// 而「它在等什么」只写在屏底——眼睛盯着横条的人不会往下扫一行。
/// 抬头摆在**边框**上，一列正文都不占（Q71 记着的第一条代价：那一行在 96 列的屏上
/// 只剩十几列空白），也不随卷与卷之间那一段空白消失（第二条代价：当前卷条会）。
/// 措辞与屏底那一行同一个出处（[`stopping_name`]）。
///
/// **停在决策点上等人时抬头写的是那件事**（`p1-session/14`）：横条这时一动不动，
/// 而「它为什么不动」眼睛盯着横条的人第一眼要看到的就是它。它排在按停那一级之前——
/// 等答话是此刻更要紧的那一件（按过的停要等答完话才继续作数）。
pub(super) fn overall_bar(
    live: Option<&Live>,
    pressed: Instruction,
    deciding: bool,
) -> Paragraph<'static> {
    let block =
        Block::default()
            .borders(Borders::ALL)
            .title(match (deciding, stopping_name(pressed)) {
                (true, _) => "整趟 · 等你拿主意".to_owned(),
                (false, Some(name)) => format!("整趟 · {name}"),
                (false, None) => "整趟".to_owned(),
            });
    let Some(live) = live else {
        return Paragraph::new(format!(" 还没跑过。{START_KEYS}")).block(block);
    };
    let overall = live.overall();
    let line = if live.ended() {
        ended_line(live)
    } else {
        format!(
            " {}/{} 卷 {} {}/{} 步 · 已用 {} · 剩 {}",
            overall.volume,
            overall.volumes,
            bar(overall.walked, overall.steps),
            overall.walked,
            overall.steps,
            spell(overall.elapsed),
            overall.left.map_or_else(|| "—".to_owned(), spell),
        )
    };
    Paragraph::new(line).block(block)
}

/// 收场那一行。
///
/// 没做成那一句照库那一侧的原话（拒绝执行是一种，那条线程恐慌了是另一种）；
/// 做成了那一种照 [`crate::render::outcome`]——「按停停在半路」与「点名的卷都走过了」
/// 的分别在 `Report::outcome` 上，措辞跟报告那一套走，会话不另编一句。
///
/// 「用了」那个数收场之后就定住了（见 [`Live::overall`]）：它是库交出来的那一个，
/// 扣掉了在决策点上等人的那几分钟。
fn ended_line(live: &Live) -> String {
    match live.undone() {
        Some(said) => format!(" 这一趟没做成：{said}"),
        None => format!(
            " 收场 {} · {} 卷 · 用了 {}",
            crate::render::outcome(live.report().outcome),
            live.report().volumes.len(),
            spell(live.overall().elapsed),
        ),
    }
}

/// 当前卷条：**在走哪一遍**，以及这一遍走到第几步。
///
/// 「在走哪一遍」只有 `PassStarted` 答得出（命令行那一路当下没有去处，见 `crate::Bar`）。
/// 非说不可，是因为三遍的性质完全不同：幂等那一道只读不写，第一遍碰像素，
/// 第二遍才往盘上写字节——「跑到一半停下来会留下什么」全看它停在哪一遍。
pub(super) fn volume_bar(live: Option<&Live>) -> Paragraph<'static> {
    let block = Block::default().borders(Borders::ALL).title("当前卷");
    let line = match live.and_then(Live::walking) {
        Some(walking) => walking_line(walking),
        // 卷与卷之间、以及还没跑过时都是空的：编一条横条上去只会让人以为它卡住了。
        None => String::new(),
    };
    Paragraph::new(line).block(block)
}

fn walking_line(walking: &Walking) -> String {
    // 卷名怎么取只有一处：命令行那条横条印的是同一个（`crate::Bar::start`）。
    let name = crate::render::volume_name(&walking.volume);
    format!(
        " {name} · {} {} {}/{} 步",
        pass_name(walking.pass),
        bar(walking.walked, walking.steps),
        walking.walked,
        walking.steps,
    )
}

/// 在走哪一遍。三段与 `VolumeTiming` 的三段是同一条分界线（`CONTEXT.md` 的《进度》）。
///
/// `_` 那一支不是遗漏：[`Pass`] 非穷尽，多一遍不该逼着这里跟着改。
fn pass_name(pass: Option<Pass>) -> &'static str {
    match pass {
        // 开卷之后、第一条 `PassStarted` 到达之前：打开容器、列成员，还没走进任何一遍。
        None => "开卷",
        Some(Pass::Fingerprint) => "幂等这一道",
        Some(Pass::First) => "第一遍",
        Some(Pass::Second) => "第二遍",
        Some(_) => "这一遍",
    }
}

/// 一条横条。样子与命令行那两条一致：`=` 是走过的，`>` 是当前这一格，空白是还没走的。
///
/// 预告的步数是零（还没开工、或者这一卷一步都不走）时整条是空的：那时没有比例可画，
/// 而画一个「刚起步」的箭头是编的。
fn bar(done: u64, total: u64) -> String {
    let filled = (total > 0).then(|| {
        // 先乘后除：先除的话，步数比条格数少的小卷会被整个抹成 0。
        done.min(total) * BAR_WIDTH / total
    });
    let mut text = String::with_capacity(BAR_WIDTH as usize + 2);
    text.push('[');
    for at in 0..BAR_WIDTH {
        text.push(match filled.map(|filled| (at.cmp(&filled), filled)) {
            Some((Ordering::Less, _)) => '=',
            Some((Ordering::Equal, filled)) if filled < BAR_WIDTH => '>',
            _ => ' ',
        });
    }
    text.push(']');
    text
}

/// 一段时长：`42s`、`6m40s`、`1h06m`。
///
/// 只留两级：秒以下在一趟几十分钟的任务里没有意义，而三级读起来要数位数。
fn spell(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    match (seconds / 3600, (seconds % 3600) / 60, seconds % 60) {
        (0, 0, second) => format!("{second}s"),
        (0, minute, second) => format!("{minute}m{second:02}s"),
        (hour, minute, _) => format!("{hour}h{minute:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::footer::running_prompt;
    use super::super::probe::{a_run_in_flight, main_snapshot, screen, tight};
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::{Key, Session};
    use tonefit::{Mode as RunMode, RunOutcome};

    /// 收场之后全局条说得出这一趟是怎么收的，报告末尾那几小结也补上了。
    #[test]
    fn the_overall_bar_says_how_the_run_ended() {
        let mut live = a_run_in_flight(false);
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Completed;
        live.returned(Ok(report));

        let snapshot = main_snapshot(&live, 78, 18);

        assert!(snapshot.contains("收场"), "{snapshot}");
        assert!(snapshot.contains("点名的卷都走过了"), "{snapshot}");
    }

    /// 拒绝执行的那一趟：会话不退出，把那句话画在全局条上，用户当场改。
    #[test]
    fn a_refused_run_says_why_on_the_overall_bar() {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.returned(Err(anyhow::anyhow!("处理范围为空：至少点名一个卷")));

        let snapshot = main_snapshot(&live, 78, 10);

        assert!(snapshot.contains("没做成"), "{snapshot}");
        assert!(snapshot.contains("处理范围为空"), "{snapshot}");
    }

    /// **按停按到哪一级，全局条的抬头上就看得出来**（停车场 Q71）。
    ///
    /// 屏底那两行说的是同一件事，措辞同一个出处（[`stopping_name`]）；
    /// 摆在抬头上是因为眼睛盯着横条的人不会往下扫一行。
    #[test]
    fn the_overall_bar_says_on_its_title_that_the_run_is_stopping() {
        let mut session = Session::new();
        session.run_started();
        assert!(
            tight(&screen(&mut session, None, 120, 40)).contains("┌整趟"),
            "没按过时抬头不该多一截"
        );

        session.press(Key::Char('s'));
        let finishing = tight(&screen(&mut session, None, 120, 40));
        assert!(finishing.contains(&tight("整趟 · 收尾中")), "{finishing}");

        session.press(Key::Char('s'));
        let aborting = tight(&screen(&mut session, None, 120, 40));
        assert!(aborting.contains(&tight("整趟 · 中止中")), "{aborting}");

        // 抬头与屏底那一行说的是同一个词：措辞只有一处。
        for pressed in [Instruction::Finish, Instruction::Abort] {
            let name = stopping_name(pressed).expect("按过的那两级都有名字");
            assert!(
                running_prompt(pressed, None).keys.contains(name),
                "屏底那一行没用 stopping_name：{pressed:?}"
            );
        }
        assert_eq!(stopping_name(Instruction::Continue), None, "没按过没有名字");
    }

    /// 横条的两头：一步没走是空的，走完是满的，总步数为零时不画比例。
    #[test]
    fn the_bar_fills_from_empty_to_full() {
        assert_eq!(bar(0, 100), format!("[>{}]", " ".repeat(29)));
        assert_eq!(bar(100, 100), format!("[{}]", "=".repeat(30)));
        assert_eq!(bar(0, 0), format!("[{}]", " ".repeat(30)));
        // 步数比条格数少的小卷不该被抹成 0。
        assert!(bar(1, 3).starts_with("[========="));
    }

    /// 时长两级就够：秒、分秒、时分。
    #[test]
    fn a_duration_is_spelled_with_two_units() {
        assert_eq!(spell(Duration::from_secs(0)), "0s");
        assert_eq!(spell(Duration::from_secs(42)), "42s");
        assert_eq!(spell(Duration::from_secs(400)), "6m40s");
        assert_eq!(spell(Duration::from_secs(3960)), "1h06m");
    }
}
