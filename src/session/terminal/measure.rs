//! **卡顿的替代量法**（`session-redesign/17`；`docs/measurements.md` 的《会话卡顿：替代量法》）。
//!
//! 不是真终端：同一条帧循环画到一个**数字节与写调用的 sink** 上，喂一串合成的终端事件
//! （触控板连滚几十格、夹几下方向键），另一条线程扮计算线程每毫秒拿一次锁、报一步，
//! 记它等锁最久等了多久。重跑：
//!
//! ```text
//! cargo test --release --bin tonefit measure_the_lag -- --ignored --nocapture
//! ```
//!
//! **量「前」**要在基线的代码上跑：检出 `97a9a7a`，把本文件摆进同一处、`terminal.rs` 末尾挂上
//! `#[cfg(test)] mod measure;`，再把底下《这一票之后那一条循环》那一段换成基线那一条——
//! 逐字抄自它的 `drive`：每读一条事件之前借着锁 `tick`、`watch_the_run`、画一整帧；
//! 终端后端不带缓冲；`consume` 一条事件画一帧、末尾再画一帧（`frames + 1`），
//! `frame` 借着锁画一帧（基线没有锁外那一份，`Glimpse` 摆一个空的单元结构占位）。
//! 基线的 `Running` 上照本票的 [`Running::a_step_from_another_thread`] 补同名一个
//! （锁里折一步，交回等了多久）。数字与施测条件在实测文档那一节。

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use tonefit::{Mode as RunMode, Pass, Request};

use super::*;
use crate::session::live::fixture;
use crate::session::state::NamedPath;

/// 大库：六十个目录、每个五十卷，前一半做完（每卷两百页），第一千五百零一卷正在处理。
const DIRECTORIES: usize = 60;
const PER_DIRECTORY: usize = 50;
const PAGES: usize = 200;
const COLS: u16 = 200;
const ROWS: u16 = 60;

#[derive(Default)]
struct Counts {
    bytes: u64,
    writes: u64,
    flushes: u64,
}

/// 写到哪儿都不去，只数。
struct Sink(Rc<RefCell<Counts>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut counts = self.0.borrow_mut();
        counts.bytes += buf.len() as u64;
        counts.writes += 1;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.borrow_mut().flushes += 1;
        Ok(())
    }
}

fn big_library(epoch: Instant) -> (Session, Live) {
    let names: Vec<String> = (0..DIRECTORIES)
        .flat_map(|d| (0..PER_DIRECTORY).map(move |v| format!("第{d:02}部/卷{v:03}")))
        .collect();
    let request = Request {
        inputs: vec![PathBuf::from("库")],
        ..fixture::request(RunMode::Process)
    };
    let mut live = fixture::live_for(epoch, &request, Resuming::GoesOn);
    live.run_started(names.len(), names.len() as u64 * 1000);
    live.surveyed(&fixture::roster(names.iter().map(String::as_str)), &[], &[]);
    let done = names.len() / 2;
    for name in &names[..done] {
        live.volume_started(&PathBuf::from(format!("库/{name}")), 1000);
        let mut report = fixture::processed_volume(name, None);
        let page = report.pages[0].clone();
        report.pages = vec![page; PAGES];
        live.volume_finished(&report);
    }
    live.volume_started(&PathBuf::from(format!("库/{}", names[done])), 1000);
    live.pass_started(Pass::First, None);
    let mut session = Session::new();
    session.scope.paths = vec![NamedPath {
        path: PathBuf::from("库"),
        on: true,
    }];
    session.views.clock = Some(epoch);
    session.run_started();
    (session, live)
}

/// 一阵触控板：连滚几十格，夹几下方向键。积压在终端里、一次到齐——卡的就是这种时候。
fn burst() -> Vec<Event> {
    let wheel = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            column: 20,
            row: 10,
            modifiers: KeyModifiers::NONE,
        })
    };
    let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
    let mut events = Vec::new();
    events.extend((0..60).map(|_| wheel(MouseEventKind::ScrollDown)));
    events.extend((0..10).map(|_| key(KeyCode::Down)));
    events.extend((0..40).map(|_| wheel(MouseEventKind::ScrollUp)));
    events.extend((0..10).map(|_| key(KeyCode::Up)));
    events
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "实测量具，不进闸门"]
fn measure_the_lag() {
    let epoch = Instant::now();
    let (mut session, live) = big_library(epoch);
    let mut running = Running::holding(live);
    let presets = Presets::at(std::env::temp_dir().join("tonefit-measure-presets.toml"));
    let here = std::env::temp_dir();
    let counts = Rc::new(RefCell::new(Counts::default()));
    let mut terminal = terminal_over(Sink(Rc::clone(&counts)));
    let window = Window {
        cols: COLS,
        rows: ROWS,
    };

    let mut glimpse = Glimpse::default();
    // 头一帧把树拼出来，不计。
    frame(&mut terminal, &mut session, &mut running, &mut glimpse);

    let step_from = running.a_step_from_another_thread();
    let stop = AtomicBool::new(false);
    let (bursts, idle, waits) = std::thread::scope(|scope| {
        // 扮计算线程：每毫秒拿一次锁、报一步，记等锁最久等了多久。
        let contender = scope.spawn(|| {
            let mut waits = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                waits.push(step_from());
                std::thread::sleep(Duration::from_millis(1));
            }
            waits
        });

        // 一：一阵积压的事件（五阵，取每阵耗时）。
        let mut bursts = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            let frames = consume(
                &mut terminal,
                &mut session,
                &mut running,
                &presets,
                &here,
                window,
                &mut glimpse,
                burst(),
            );
            bursts.push((started.elapsed(), frames));
        }
        // 二：没有事件、那一趟在走——空转五十帧，每帧耗时。
        let mut idle = Vec::new();
        for _ in 0..50 {
            let started = Instant::now();
            frame(&mut terminal, &mut session, &mut running, &mut glimpse);
            idle.push(started.elapsed());
        }
        stop.store(true, Ordering::Relaxed);
        (
            bursts,
            idle,
            contender.join().expect("扮计算线程的那条没恐慌"),
        )
    });

    let counts = counts.borrow();
    let frames: usize = bursts.iter().map(|(_, f)| f).sum();
    let burst_ms: Vec<f64> = bursts.iter().map(|(d, _)| millis(*d)).collect();
    let mut idle = idle;
    idle.sort();
    let mut waits = waits;
    waits.sort();
    println!(
        "卷 {} / 已做完 {} / 每卷 {PAGES} 页 / 窗口 {COLS}×{ROWS}",
        DIRECTORIES * PER_DIRECTORY,
        DIRECTORIES * PER_DIRECTORY / 2
    );
    println!(
        "一阵事件 {} 条（滚轮 100 格、方向键 20 下），五阵",
        burst().len()
    );
    println!("每阵耗时 ms: {burst_ms:.1?}");
    println!("五阵共画 {frames} 帧");
    println!(
        "空转一帧 ms: 中位 {:.2} / p90 {:.2} / 最大 {:.2}",
        millis(percentile(&idle, 0.5)),
        millis(percentile(&idle, 0.9)),
        millis(*idle.last().expect("有帧"))
    );
    println!(
        "写出总计: {} 字节 / {} 次写调用 / {} 次冲刷",
        counts.bytes, counts.writes, counts.flushes
    );
    println!(
        "计算线程等锁 ms: 中位 {:.3} / p99 {:.3} / 最大 {:.3}（{} 次）",
        millis(percentile(&waits, 0.5)),
        millis(percentile(&waits, 0.99)),
        millis(*waits.last().expect("拿过锁")),
        waits.len()
    );
}

// ───── 这一票之后那一条循环（真会话走的就是 `one_turn`）─────

fn terminal_over(sink: Sink) -> Terminal<CrosstermBackend<BufWriter<Sink>>> {
    Terminal::new(CrosstermBackend::new(BufWriter::with_capacity(
        FRAME_BUFFER,
        sink,
    )))
    .expect("造得出终端")
}

/// 空转一帧：没有积压的一转。
fn frame<B: Backend<Error = std::io::Error>>(
    terminal: &mut Terminal<B>,
    session: &mut Session,
    running: &mut Running,
    glimpse: &mut Glimpse,
) {
    let presets = Presets::at(std::env::temp_dir().join("tonefit-measure-presets.toml"));
    one_turn(
        terminal,
        session,
        running,
        &presets,
        Path::new("."),
        glimpse,
        Vec::new(),
    )
    .expect("画得出");
}

/// 这一票之后：积压的那一批一转做完、只画一帧。
#[allow(clippy::too_many_arguments)]
fn consume<B: Backend<Error = std::io::Error>>(
    terminal: &mut Terminal<B>,
    session: &mut Session,
    running: &mut Running,
    presets: &Presets,
    here: &Path,
    window: Window,
    glimpse: &mut Glimpse,
    events: Vec<Event>,
) -> usize {
    let _ = window;
    let pending = events.iter().filter_map(translate_input).collect();
    one_turn(terminal, session, running, presets, here, glimpse, pending).expect("画得出");
    1
}
