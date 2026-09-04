//! 画法那几块共用的**测试探针**：把一屏画出来，再把屏上的东西取回来比。
//!
//! **读法一律摆在这里**，即便眼下只有一块用得上：屏上取回来的文字怎么读有两条各不相同的
//! 道理——逐格拼那一条每个汉字后面多一个空格（[`tight`] 说的是它），快照那一条走终端库
//! 自己的 `Display`（[`snapshot`] 说的是它）。分开放就会有人再抄一份，而抄第二份就会
//! 有一份抄漏。
//!
//! **夹具按用得上它的块数分**：跨块的摆这里（[`a_run_in_flight`]），
//! 只有一块用得上的留在那一块自己的 `mod tests` 里。

use std::path::Path;
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::{Frame, Terminal};
use tonefit::Mode as RunMode;

use super::{CONFIG_WIDTH, main_pane, shell};
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

/// 快照比对。对不上就把**实际那一屏**整个印出来——改的人照着它逐行确认，
/// 确认过了再抄回本文件。与黄金快照同一条规矩：任何一行变了都要有人当场答一句为什么。
pub(super) fn same_screen(actual: &str, expected: &str) {
    assert_eq!(
        actual,
        expected.trim_matches('\n'),
        "\n实际画出来的是：\n{actual}"
    );
}
