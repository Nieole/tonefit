//! 画法那几块共用的**测试探针**：把一屏画出来，再把屏上的东西取回来比。
//!
//! **读法一律摆在这里**，即便眼下只有一块用得上：屏上取回来的文字怎么读有两条各不相同的
//! 道理——逐格拼那一条每个汉字后面多一个空格（[`tight`] 说的是它），快照那一条走终端库
//! 自己的 `Display`（[`snapshot`] 说的是它）。分开放就会有人再抄一份，而抄第二份就会
//! 有一份抄漏。
//!
//! **夹具按用得上它的块数分**：跨块的摆这里（[`a_run_in_flight`]、[`every_kind_of_volume`]），
//! 只有一块用得上的留在那一块自己的 `mod tests` 里。

use std::path::Path;
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::{Frame, Terminal};
use tonefit::Mode as RunMode;

use super::yielding::CONFIG_WIDTH;
use super::{main_pane, shell};
use crate::session::live::{Live, Resuming, fixture};
use crate::session::state::Session;

/// 屏上的文字，**空白全去掉**再比。
///
/// 宽字符在缓冲里占两格，第二格被 ratatui 重置成一个空格——逐格读回来的文字
/// 因此在每个汉字之间多一个空格（停车场 Q60）。要问的是「这几个字在不在屏上」，
/// 两边都去掉空白最省事，也不会把断言比松：这些标签没有一个靠空格分辨。
///
/// **快照那几条不走这里**：它们比的是 [`snapshot`]，而那一条路上宽字符那一格
/// 根本不在场。
pub(super) fn tight(text: &str) -> String {
    text.chars()
        .filter(|glyph| !glyph.is_whitespace())
        .collect()
}

/// 把一屏画出来，取回屏上的文字（逐格拼，宽字符后面多一个空格）。
pub(super) fn screen(
    session: &mut Session,
    live: Option<&Live>,
    width: u16,
    height: u16,
) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
    terminal
        .draw(|frame| shell(frame, session, live))
        .expect("画得出来");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// **左栏**上有几格是反白的。**光标停在哪一行只有它看得出来**——
/// 逐格拼回来的文字里一点痕迹都没有，而「跑起来之后按不动」与「预设那一栏开着时
/// 按键不归左栏」两条都要靠这一格说话。
///
/// 只数左栏那几列：主区自己也有反白的一行（预设那一栏的光标），一起数就分不出是谁。
pub(super) fn reversed_rows(session: &mut Session) -> usize {
    const WIDE: usize = 120;
    let mut terminal = Terminal::new(TestBackend::new(WIDE as u16, 40)).expect("测试后端起得来");
    terminal
        .draw(|frame| shell(frame, session, None))
        .expect("画得出来");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .enumerate()
        .filter(|(at, cell)| {
            at % WIDE < CONFIG_WIDTH as usize && cell.modifier.contains(Modifier::REVERSED)
        })
        .count()
}

/// 一格里有几处反白。预设那一栏的光标靠它验（[`reversed_rows`] 只数左栏）。
pub(super) fn reversed_cells(draw: impl FnOnce(&mut Frame), width: u16, height: u16) -> usize {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
    terminal.draw(draw).expect("画得出来");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(Modifier::REVERSED))
        .count()
}

/// 屏上**反白的那一行**上写的是什么（首尾空白去掉）。一行都没反白就是 `None`。
///
/// **「焦点落在哪一块」屏上只有这一处看得出来**（`CONTEXT.md` 的《会话》：焦点）：
/// 焦点在左栏时反白的是配置那一行，切到报告区之后是卷表上光标停着的那一卷。
/// 快照那一路问不出它——[`snapshot`] 比的是字，而反白是样式。
///
/// 只回**头一行**：一行摆不下折下来的那几行跟着同一个样式
/// （见 `super::report` 的 `highlighted`），而问的是「反白落在哪一行上」。
pub(super) fn reversed_row(
    draw: impl FnOnce(&mut Frame),
    width: u16,
    height: u16,
) -> Option<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
    terminal.draw(draw).expect("画得出来");
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width).max(1))
        .find(|row| {
            row.iter()
                .any(|cell| cell.modifier.contains(Modifier::REVERSED))
        })
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim()
                .to_owned()
        })
}

/// 屏上的一行，连同**这一行上的颜色**。
///
/// 快照那一路只比得了字（[`snapshot`] 走终端库自己的 `Display`），而语义色那一票要问的是
/// 「**出事那一行既是红的、也带着那个字**」——两半得在同一行上一起读得到
/// （spec 的《Testing Decisions》：颜色）。
pub(super) struct OnScreen {
    /// 这一行的字，逐格拼（宽字符后面多一个空格——与 [`tight`] 同一条读法）。
    pub(super) text: String,
    /// 这一行上出现过的前景色，去重、按出现次序。**终端默认色不算一种**：
    /// 「这一行没上色」问的就是这一列空不空。
    pub(super) colours: Vec<Color>,
    /// 逐格：这一格压没压暗（[`Tone::Muted`](super::paint::Tone) 是压暗的）。
    /// 整行问 [`dim`](Self::dim)，只问左栏那几列问 [`dim_before`](Self::dim_before)。
    dimmed: Vec<bool>,
}

impl OnScreen {
    /// 这一行上有没有压暗的格子。
    pub(super) fn dim(&self) -> bool {
        self.dim_before(u16::MAX)
    }

    /// 这一行**头 `columns` 列**里有没有压暗的格子。
    ///
    /// 左栏那几列单独问，与 [`reversed_rows`] 同一条道理：主区自己也上色，
    /// 一起数就分不出是谁压的暗。
    pub(super) fn dim_before(&self, columns: u16) -> bool {
        self.dimmed
            .iter()
            .take(usize::from(columns))
            .any(|dimmed| *dimmed)
    }
}

/// 把一屏画出来，**逐行**取回它的字与颜色（见 [`OnScreen`]）。
///
/// 空白那几格一概不算：它们身上多半跟着整行的样式，而问的是「**这几个字**是什么色」。
pub(super) fn painted(draw: impl FnOnce(&mut Frame), width: u16, height: u16) -> Vec<OnScreen> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
    terminal.draw(draw).expect("画得出来");
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width).max(1))
        .map(|row| {
            let mut colours: Vec<Color> = Vec::new();
            let mut dimmed = Vec::with_capacity(row.len());
            let mut text = String::new();
            for cell in row {
                let symbol = cell.symbol();
                let blank = symbol.chars().all(char::is_whitespace);
                text.push_str(symbol);
                dimmed.push(!blank && cell.modifier.contains(Modifier::DIM));
                if !blank && cell.fg != Color::Reset && !colours.contains(&cell.fg) {
                    colours.push(cell.fg);
                }
            }
            OnScreen {
                text,
                colours,
                dimmed,
            }
        })
        .collect()
}

/// 一屏的**快照**：一行一行，每行两侧加引号，一格都不多一格不少。
///
/// 走的是 `TestBackend` 自己的 `Display`——它按 `cell_width` 跳过被宽字符盖住的那一格
/// （停车场 Q60 说的正是那一格）。自己逐格拼的话，每个汉字后面都会多出一个空格来，
/// 而快照要的恰恰是「屏上一模一样」。
pub(super) fn snapshot(draw: impl FnOnce(&mut Frame), width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
    terminal.draw(draw).expect("画得出来");
    terminal
        .backend()
        .to_string()
        .lines()
        // 它每行是 `"……"`，后面还可能跟一句「被宽字符盖住的是哪几格」——
        // 那一句是给人看的诊断，不进快照。引号留着：不留的话，行尾那几个空格
        // 在编辑器里会被吃掉，快照下一次就对不上了。
        .filter_map(|row| row.split('"').nth(1))
        .map(|row| format!("\"{row}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 主区单独一格的快照。快照只钉主区，是因为 `09` 做的就是它——
/// 把左栏一起钉进来，改一行配置标签就要重录一次这几段。
///
/// 会话给一个新的：那几张快照问的是「跑到这一步主区画成什么样」，与三层配了什么无关。
/// 展开那几条走 [`snapshot_of`]：那时要钉的是**整屏**（左栏在不在场是一半）。
pub(super) fn main_snapshot(live: &Live, width: u16, height: u16) -> String {
    let mut session = Session::new();
    snapshot(
        |frame| main_pane(frame, frame.area(), &mut session, Some(live)),
        width,
        height,
    )
}

/// **整屏**快照：三层与那一趟一起画，展开着时左栏因此不在场。
pub(super) fn snapshot_of(session: &mut Session, live: &Live, width: u16, height: u16) -> String {
    snapshot(|frame| shell(frame, session, Some(live)), width, height)
}

/// 一趟跑到一半：两卷跑完（一卷幂等命中、一卷带失败页），第三卷正走第二遍。
///
/// 时钟往回拨一段固定的量，快照因此不随机器快慢而变——与黄金快照同一条规矩
/// （`tonefit::Report::elapsed`：计时只进结构，不进渲染出的文字）。
pub(super) fn a_run_in_flight(failures: bool) -> Live {
    let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
    live.run_started(3, 5000);
    live.volume_started(Path::new("库/卷一"), 1000);
    live.volume_finished(&fixture::skipped_volume("卷一", 180));
    live.volume_started(Path::new("库/卷二"), 1000);
    let broken = failures.then_some("解不出完整尺寸：JPEG 数据截断");
    if let Some(reason) = broken {
        live.page_failed(Path::new("库/卷二/017.jpg"), reason);
    }
    live.volume_finished(&fixture::processed_volume("卷二", broken));
    live.volume_started(Path::new("库/卷三"), 3000);
    live.pass_started(tonefit::Pass::Second, None);
    for _ in 0..1000 {
        live.stepped();
    }
    live.rewind(Duration::from_secs(300));
    live
}

/// 一趟**六种卷都齐**的：跳过、隔离、逐页、覆盖、卷级失败，
/// 外加停在决策点上等答话的那一卷。
///
/// 卷名特意长短不一，宽终端上一列对得齐、窄终端上砍得看得见。
/// 末一种只有**续做那一趟**到得了：等答话是决策点上的事，而一趟走到底的执行
/// 在决策点上不停（`CONTEXT.md` 的《会话》：续做）。`resumes` 因此同时定了屏上那两个字
/// ——答出第一个继续之前它印的是「试算」（见 `Live::mode`）。
///
/// **跨块**：卷表那几条问「六种卷各长什么样」（[`super::report`]），
/// 语义色那几条问「哪几行上了色、上的是哪一种」（[`super::paint`]）——
/// 同一趟里六种卷恰好把四种语义占全。
pub(super) fn every_kind_of_volume(mode: RunMode, resumes: Resuming) -> Live {
    let mut live = Live::new(&fixture::request(mode), resumes);
    live.run_started(6, 6000);
    live.volume_started(Path::new("库/棋魂 07"), 1000);
    live.volume_finished(&fixture::skipped_volume("棋魂 07", 184));
    live.volume_started(Path::new("库/哆啦 03"), 1000);
    live.volume_finished(&fixture::processed_volume(
        "哆啦 03",
        Some("解不出完整尺寸：JPEG 数据截断"),
    ));
    live.volume_started(Path::new("库/名侦探 05"), 1000);
    live.volume_finished(&fixture::per_page_volume("名侦探 05"));
    live.volume_started(Path::new("库/浪客行 12"), 1000);
    live.volume_finished(&fixture::overridden_volume("浪客行 12"));
    live.volume_started(Path::new("库/消失的那卷"), 1000);
    live.volume_failed(Path::new("库/消失的那卷"), "卷根不在了");
    if live.resumes() {
        live.volume_started(Path::new("库/棋魂 08"), 1000);
        live.pass_started(
            tonefit::Pass::Second,
            Some(&fixture::processed_volume("棋魂 08", None)),
        );
    }
    live.rewind(Duration::from_secs(300));
    live
}

/// 快照比对。对不上就把**实际那一屏**整个印出来——改的人照着它逐行确认，
/// 确认过了再抄回本文件。与黄金快照同一条规矩：任何一行变了都要有人当场答一句为什么。
pub(super) fn same_screen(actual: &str, expected: &str) {
    assert_eq!(
        actual,
        expected.trim_matches('\n'),
        "\n实际画出来的是：\n{actual}"
    );
}
