//! 终端那一侧：进出终端、把键码翻译成会话认得的键、在两者之间转一个循环。
//!
//! **本仓库唯一一处认得 crossterm 键码的地方**（见 [`translate`]），也是唯一一处
//! 握着终端的地方（见 [`Screen`]）。状态机、边跑边攒的那一份、起线程与逐层补全
//! 都在 [`super`] 的另外四个模块里，摆在 `tui` 特性**外面**——分界与理由见
//! `super` 的模块文档《终端库在哪一半》。
//!
//! 除了那三件事，这一层还担着**状态机够不着的那几支**（见 [`press`]）：
//! 起一趟、按停止、展开、读写盘上那份预设、把灰阶测试图交给库里第三个 seam。
//! 那几支要的是那一趟、那块盘与那个库，而状态机三样都不碰。

use std::io::{IsTerminal, Stderr, stderr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use clap::Parser;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use tonefit::{Mode as RunMode, Request};

use super::cover;
use super::draw;
use super::draw::keys::Starters;
use super::home::Home;
use super::keymap::{Deed, Phase};
use super::live::{Branch, Live, Resuming, Volume, VolumeState};
use super::look::{Kind, Look, Segment};
use super::run::Running;
use super::state::{Action, Exit, Expansion, Key, Picker, Session};
use super::tone::Tone;
use super::view::{CHART_LINGERS, Cursor, Input, NamedPreset, Pages, View, Window};
use crate::preset::{Presets, Saved};

/// 没等到按键时隔多久重画一帧。
///
/// 跑着的那一趟就是靠它动起来的：事件从计算线程上折进 [`super::live::Live`]，而把它画出来的
/// 只有这一条循环。取八十毫秒——比人眼看得出的停顿短，又不至于把一趟长任务的
/// CPU 花在画横条上。
const TICK: Duration = Duration::from_millis(80);

/// 进会话，跑到用户退出为止。
///
/// 出的是**最后那一趟**的退出码，与命令行那一路同一套（见 [`super::live::Live::exit_code`]）：
/// 全部成功 `0`、有卷被隔离 `2`、有卷没做成 `3`、拒绝开始 `1`。一趟都没跑过是 `0`。
///
/// 退出前把那份报告照原格式印到 **stdout**：会话整个画在 stderr 上，
/// `tonefit > 报告.txt` 因此仍然成立。**最后那一趟没做成也照印**——
/// 攒下来的那一份连同它为什么没做成，先前那一趟做成了的报告也不跟着丢
/// （21 号票，见 [`Running::report`]）。
pub fn enter() -> Result<u8> {
    if !stderr().is_terminal() {
        return Err(no_terminal_error());
    }
    let mut screen = Screen::open()?;
    let mut session = Session::new();
    // 家目录问一次、摆在会话上往下传（`CONTEXT.md` 的《会话》：家目录）：新界面的屏上把它缩写成 `~`。
    session.home = Home::found();
    // 会话的时钟起点：屏上那个转轮转到第几格从它算起（`CONTEXT.md` 的《会话》：此刻）。
    session.views.clock = Some(Instant::now());
    // 跑着的那一趟**一定**要收手：`?` 提前返回、恐慌展开，走的都是 `Running` 的 `Drop`。
    // 终端同理，走 `Screen` 的 `Drop`。
    let mut running = Running::default();
    // 预设文件那一份。**找不到用户配置目录不在这里拦**：那台机器上会话照进，
    // 只是按下 `p` 那一刻它说得出为什么（见 `preset::Presets`）。
    let presets = Presets::found();
    // 它在哪也摆在会话上往下传：配置视图顶上那一条右端写着它（家目录缩写成 `~`）。
    // **问不出来就不写**——那台机器上会话照进，只是按下 `p` 那一刻它说得出为什么。
    session.views.presets = presets.path().ok().map(Path::to_path_buf);
    // 灰阶测试图落在会话是从哪儿敲起来的那个目录里（见 [`chart_file`]）。**一次问出来**：
    // 一趟会话里当前目录不会变，而按一次 `c` 问一次只会让两张图落在两个地方。
    // 问不出来（那个目录被删了）时退回空路径，`chart_file` join 出来的于是是个**裸文件名**：
    // 图仍旧落在同一处（进程的当前目录，只是这一头叫不出它的名字），
    // 而屏底那两行头一行也退成那个裸名——「图在哪儿」那一半这时说不全。
    // 仍旧写、仍旧说，因为另一条路是一个键按下去什么都不做，那更坏；
    // 而 `current_dir` 答不出话的机器上，别的路径也一样叫不出名字。
    let here = std::env::current_dir().unwrap_or_default();
    let outcome = drive(&mut screen, &mut session, &mut running, &presets, &here);
    drop(screen);
    outcome?;
    // 报告印在终端还回去**之后**：印进 alternate screen 的话它会随着那一屏一起消失。
    if let Some(report) = running.report() {
        print!("{report}");
    }
    Ok(running.exit_code())
}

/// 画一屏、等一个键（最多等 [`TICK`]）、做掉它，直到用户退出。
fn drive(
    screen: &mut Screen,
    session: &mut Session,
    running: &mut Running,
    presets: &Presets,
    here: &Path,
) -> Result<()> {
    loop {
        // **这一帧的「此刻」**：单调时钟一帧读一次，会话里要时刻的地方都读它
        // （`CONTEXT.md` 的《会话》：此刻；`session-redesign/04`）。眼下只有攒着的那一份
        // 要它——已用、预计、确认点上等人的那一截——屏上那几个数因此出自同一个时刻。
        let now = Instant::now();
        {
            // 借着锁画：画完当场还回去，计算线程最多等一帧的功夫（见 `Running::live`）。
            let mut live = running.live();
            if let Some(live) = live.as_deref_mut() {
                live.tick(now);
            }
            screen
                .terminal
                .draw(|frame| draw::shell(frame, session, live.as_deref()))?;
        }
        if event::poll(TICK)? {
            // 只认按下去那一下：Windows 上按键抬起也报一条，不滤掉的话每个键都走两遍。
            let Event::Key(pressed) = event::read()? else {
                continue;
            };
            if pressed.kind == KeyEventKind::Press
                && let Some(key) = translate(&pressed)
                && press(session, running, presets, here, key) == Exit::Leave
            {
                running.leave();
                return Ok(());
            }
        }
        // 那一趟停在确认点上了：会话跟着停下来等人答话（`p1-session/14`）。
        // 停在那儿的是计算线程，而状态机碰不到线程——这一层问得到，把答案交进去。
        session.at_the_decision_point(running.deciding());
        // 那一趟跑完了：配置又改得动。
        if running.reap() {
            session.run_finished();
        }
    }
}

/// 把一个键交给会话。
///
/// **只有够得着那一趟、或者够得着盘的那几支不走 [`Session::press`]**：
/// [起一趟](Action::Start)、[按停止](Action::Stop)、[展开](Action::Expand)与
/// [换一卷](Action::Turn)，加上预设那四支（[列出来](Action::Pick)、
/// [套用](Action::Take)、[存下来](Action::Store)、[删掉](Action::Erase)）
/// 与[出灰阶测试图](Action::Chart)。
/// 起线程、拼 `Request`、把观察者接上去、把按到的那一级送到计算线程上、
/// 从攒着的那份报告上数出此刻有哪几卷、读写用户配置目录下那份 TOML、
/// 把灰阶测试图交给库里那第三个 seam，
/// 都在这一层——状态机一个终端都不碰、不起线程，也读不到那一趟攒下来的东西与盘上的东西。
/// 拼不出 `Request` 的那两种（型号没挑、输出目录没填）当场说一句，会话原地不动。
///
/// `here` 是灰阶测试图落在哪个目录下（见 [`chart_file`]）：真会话里是进程的当前目录，
/// 由 [`enter`] 一次问出来。
fn press(
    session: &mut Session,
    running: &mut Running,
    presets: &Presets,
    here: &Path,
    key: Key,
) -> Exit {
    // 问一次就够：这几支之外的原样交回状态机，不让它再问一遍。
    let action = session.action(key);
    match action {
        Action::Start(mode) => {
            match session.request(mode) {
                Ok(request) => {
                    let (request, resumes) = resuming(request);
                    running.start(request, resumes);
                    session.run_started();
                }
                Err(error) => session.complain(format!("{error:#}")),
            }
            Exit::Stay
        }
        // 按停止：状态机把闩升一级（做完再停 → 立即停止，ADR 0013），这一层把升到的那一级
        // 交给跑着的那一趟。两处记的是同一个字，出处只有状态机那一份——
        // 这里读的就是它刚升完的结果，不自己再算一次。
        Action::Stop => {
            let exit = session.act(action);
            running.stop(session.stopping());
            exit
        }
        // 确认点上答话：状态机把会话放回「跑着」那一副，这一层把那个字交给停在
        // 确认点上的那条线程。与按停止同一条分工——认键在那边，碰线程在这边。
        // **两处记的是同一个字**，而它就在这个动作里带着：确认点回的是当场那个字、
        // 不是闩（ADR 0012 决定第 2 条），因此这里不去问状态机再算一次。
        // 它管几卷（「后面的卷都写出」）同样带在动作里，摆到那道闸的默认答案上去
        // （`Running::decide`）——那一格也不是闩。
        Action::Answer(said, reach) => {
            let exit = session.act(action);
            running.decide(said, reach);
            exit
        }
        // 展开与换一卷：要读那一趟攒下来的报告（此刻有哪几卷），而状态机读不到它。
        // 收起（`Action::Collapse`）不在这里——它不必读报告。
        Action::Expand | Action::Turn(_) => {
            expand(session, running, action);
            Exit::Stay
        }
        // 展开一枝：同样要读那一趟攒下来的报告（此刻有哪几枝），
        // 与[展开一卷](Action::Expand)同一条分法。
        Action::Open => {
            open(session, running);
            Exit::Stay
        }
        // 卷表上挪一卷：同样要读那一趟攒下来的报告（此刻有哪几卷）。
        // **一趟都没跑过时一格不动**——那时报告区里连一卷都没有，屏上也不摆这两个键。
        Action::Select(step) => {
            if let Some(live) = running.live() {
                session.select(&live, step);
            }
            Exit::Stay
        }
        // 预设那四支：列出来、套一份、存一份、删一份，四件都要碰盘，而状态机碰不到盘。
        // 四支各走各的函数，不合成一个收 `Action` 的分派——合起来就要留一支
        // 「到不了」的 `_`，而那正是新添一支动作（删一份就是这么添进来的，停车场 Q74）
        // 会被静默吃掉的地方。
        Action::Pick => {
            list_presets(session, presets);
            Exit::Stay
        }
        Action::Take => {
            take_preset(session, presets);
            Exit::Stay
        }
        Action::Store => {
            store_preset(session, presets);
            Exit::Stay
        }
        Action::Erase => {
            erase_preset(session, presets);
            Exit::Stay
        }
        // 出灰阶测试图：画图与落盘整件事在库里那第三个 seam 上，而状态机碰不到盘。
        // 与预设那三支同一条分法。
        Action::Chart => {
            write_chart(session, here);
            Exit::Stay
        }
        other => session.act(other),
    }
}

/// 把一个输入交给**新会话**（ADR 0019；spec《缝》）——与 [`press`] 并排，真会话仍走那一支，
/// 切换在 `session-redesign/15`。收的是键或鼠标（[`Input`]），带着这一帧的「此刻」与窗口的尺寸。
///
/// 分工与 [`press`] 同一条：先把输入认成按键表上的一件事（[`Session::deed_of`]，连击键在那里待着），
/// **够得着那一趟与屏的那几件在这一层做**，其余交回状态机（[`Session::perform`]）。
/// 这一层眼下有这几件：覆盖层上滚动（那一张有几行、露几行都从窗口的尺寸算，
/// [`cover::Sheet`]，而窗口有多大只有这一层知道）、`F` 之后光标跟上正在处理的那一卷、
/// 搜索与跳转的落点（哪几卷出了事只有那一趟答得出）。
/// 起一趟（走 [`press`] 的 `Action::Start` 起线程那条路）、按停止、答话、
/// 预设那几支与灰阶测试图，随各票在这里各接一支——接上之前那几个键交下去落在
/// [`Session::perform`] 的空处，原地不动。`running` 眼下只答一件事：那一趟清点完了没有。
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "真会话切到新界面（session-redesign/15）时接进那条循环"
    )
)]
pub(super) fn input(
    session: &mut Session,
    running: &mut Running,
    presets: &Presets,
    here: &Path,
    now: Instant,
    window: Window,
    input: Input,
) -> Exit {
    // **先盯一眼那一趟**：清点的产出到了就把树拼出来，自动滚动开着时光标跟到正在处理的
    // 那一卷（`Session::watch_the_run`）——两件事都要读那一趟攒下来的东西，而状态机读不到。
    // 真会话切过来之后那条循环每一帧也调它一次。
    let phase = {
        let live = running.live();
        if let Some(live) = live.as_deref() {
            session.watch_the_run(live);
        }
        Phase::of(session.stage(), live.as_deref())
    };
    let Some(deed) = session.deed_of(input, phase, now) else {
        return Exit::Stay;
    };
    if session.views.cover.is_some()
        && session
            .views
            .scroll_cover(deed, &cover::Sheet::of(phase, window), window)
    {
        return Exit::Stay;
    }
    // **半屏与一屏那四个**：光标挪几行要窗口有多高（`Session::scroll_list`）。
    if session.scroll_list(deed, window, now) {
        return Exit::Stay;
    }
    match deed {
        // **起一趟**：走 [`press`] 的 `Action::Start` 那条路——起线程、拼 `Request`、
        // 把观察者接上去，一件都不在状态机里。`t`／`x` 开跑**总回到任务视图**
        // （ADR 0019 决定第 1 条）。
        Deed::Preview | Deed::Convert => {
            let mode = if deed == Deed::Preview {
                RunMode::DryRun
            } else {
                RunMode::Process
            };
            begin(session, running, mode, now);
            Exit::Stay
        }
        // **按停止**：状态机把闩升一级（`Session::perform`），这一层把升到的那一级交给
        // 跑着的那一趟。两处记的是同一个字，出处只有状态机那一份——与 [`press`] 同一条分工。
        Deed::Stop => {
            let exit = session.perform(deed, now);
            running.stop(session.stopping());
            exit
        }
        // **`F` 交回自动滚动**：扳回那一格与屏底那一句是状态机的事（`Session::perform`），
        // 而**光标当场跟到正在处理的那一卷**要读那一趟——按下去这一帧就得跟上，
        // 因此这一层紧接着再盯一眼。与按停止那一件同一条分工。
        Deed::Follow => {
            let exit = session.perform(deed, now);
            let live = running.live();
            if let Some(live) = live.as_deref() {
                session.watch_the_run(live);
            }
            exit
        }
        // **搜索那一行上的 `⏎`**：收下这一句、跳到第一个结果。落点要问那一趟，
        // 而状态机读不到它（`super::typing::Session::confirm_typed` 把这一种让了出来）。
        // 别的输入行照旧交给状态机。
        Deed::Confirm if session.searching_line() => {
            let live = running.live();
            session.confirm_search(live.as_deref(), now);
            Exit::Stay
        }
        // **`]d`／`[d`／`n`／`N` 跳到下一处**：落点是「哪几卷出了事」与「哪一行装着
        // 这一句」，前者只有那一趟答得出（`Session::jump`）。
        Deed::NextProblem | Deed::PrevProblem | Deed::SearchNext | Deed::SearchPrev => {
            let live = running.live();
            session.jump(deed, live.as_deref(), now);
            Exit::Stay
        }
        // **预设那几支与灰阶测试图**：掀开预设栏要列出盘上那几份、`dd` 的第二下要删掉一份、
        // 起好名那一下要存一份、`c` 要写出一张图——四件都碰盘，而状态机碰不到盘。
        // 与[旧界面那一支](press)同一条分法；套用一份不在这里，掀开那一刻已经读进来了
        // （`Session::lift_picker`），那一下因此是纯状态（`Session::use_preset`）。
        Deed::Presets => {
            toggle_picker(session, presets, now);
            Exit::Stay
        }
        Deed::DeletePreset => {
            erase_a_preset(session, presets, now);
            Exit::Stay
        }
        Deed::Confirm if session.naming_a_preset() => {
            store_a_preset(session, presets, now);
            Exit::Stay
        }
        Deed::Chart => {
            draw_a_chart(session, here, now);
            Exit::Stay
        }
        // **卷行上按展开**：展不展得开要问那一趟这一卷此刻怎么样，而状态机读不到它
        // （`CONTEXT.md` 的《停得住 / 展得开》）——展得开的换屏进每页结果，
        // 展不开的屏底说一句为什么。
        Deed::Open if matches!(session.views.task.cursor, Cursor::Volume(_)) => {
            open_a_volume(session, running, now);
            Exit::Stay
        }
        _ => session.perform(deed, now),
    }
}

/// 起一趟：拼得出 `Request` 就起线程，拼不出（型号没挑、输出目录没填、一条路径都没勾）
/// 当场说一句，会话原地不动。**开跑总回到任务视图。**
fn begin(session: &mut Session, running: &mut Running, mode: RunMode, now: Instant) {
    match session.request(mode) {
        Ok(request) => {
            let (request, resumes) = resuming(request);
            running.start(request, resumes);
            session.run_started();
            session.views.view = View::Task;
            let (head, rest, kind) = match mode {
                RunMode::DryRun => (
                    "预览：",
                    "只分析不写文件，每卷分析完会停下来问你".to_owned(),
                    Kind::Preview,
                ),
                _ => (
                    "转换：",
                    format!("写到 {}", session.output_shown()),
                    Kind::Convert,
                ),
            };
            session.views.say(
                vec![
                    Segment::new(head, Look::kind(kind).bold()),
                    Segment::plain(rest),
                ],
                now,
            );
        }
        Err(error) => session.views.say(
            vec![
                Segment::new("✗ ", Look::tone(Tone::Trouble).bold()),
                Segment::new(format!("{error:#}"), Look::tone(Tone::Trouble)),
            ],
            now,
        ),
    }
}

/// 卷行上按下展开：**展得开的只有收摊了的那几卷（跳过的也算）与确认点上那一份**；
/// 其余停得住、展不开，屏底说一句为什么（`CONTEXT.md` 的《停得住 / 展得开》）。
fn open_a_volume(session: &mut Session, running: &Running, now: Instant) {
    let Cursor::Volume(root) = session.views.task.cursor.clone() else {
        return;
    };
    let at = session.views.task.tree.index_of(&root);
    let state = session.volume_state(running.live().as_deref(), &root);
    // **展得开的那几卷换屏进每页结果**：屏底一句话都不说——换了一整屏，
    // 屏上自己就说清了这一下做成了什么。展不展得开的判据在
    // [`VolumeState::opens_the_pages`] 一处，屏底摆不摆 `l` 读的是同一份。
    //
    // **跳过的卷也进得来**（`CONTEXT.md` 的《停得住 / 展得开》），只是它一页结果都没有
    // ——那一屏里说一句它跳过了（`super::shell::pages`）。
    if state.opens_the_pages() {
        session.views.task.pages = Some(Pages::of(root));
        return;
    }
    let undone = at.and_then(|at| {
        running
            .live()
            .and_then(|live| live.undone_at(at).map(str::to_owned))
    });
    let said = match state {
        // 上面那道守卫已经把展得开的那几卷挡回去了。
        VolumeState::Done
        | VolumeState::Isolated
        | VolumeState::Skipped
        | VolumeState::Deciding => return,
        VolumeState::Failed => vec![
            Segment::new("✗ 转换失败：", Look::tone(Tone::Trouble).bold()),
            Segment::plain(format!(
                "{}，这一卷没有每页结果",
                undone.unwrap_or_default()
            )),
        ],
        VolumeState::Aborted => vec![
            Segment::new("已中断：", Look::tone(Tone::Caution).bold()),
            Segment::plain("这一卷没有保存，也没有每页结果"),
        ],
        VolumeState::Running { .. } => vec![
            Segment::new("还在处理：", Look::tone(Tone::Caution).bold()),
            Segment::plain("这一卷做完才有每页结果"),
        ],
        VolumeState::Queued => vec![
            Segment::new("还没轮到这一卷：", Look::FAINT.bold()),
            Segment::plain("做完才有每页结果"),
        ],
    };
    session.views.say(said, now);
}

// ───────────────────────── 新界面的预设那几支与灰阶测试图 ─────────────────────────
//
// 与[旧界面那四支](list_presets)同一条分工：**碰盘的在这一层**，认键与屏上那几格在状态机。
// 四支各走各的函数，不合成一个分派——理由与旧界面那四支相同（停车场 Q74）。

/// **`p`：掀开或收起预设栏。** 掀开那一下把盘上有的那几份连同它们的内容读进来
/// （`CONTEXT.md` 的《预设栏》：列的是**进这一栏那一刻**盘上有的几份），
/// 收起那一下一个字节都不碰盘。
///
/// **读得出名字就够开这一栏**：一份字段过时的预设不该让别的几份列不出来
/// （[`Presets::names`]），读不懂的那一份只列名字、屏上那一行说一句它读不懂。
/// 整份文件读不懂、或者配置目录答不出来，那一栏才开不了——屏底说库那一侧的原话。
fn toggle_picker(session: &mut Session, presets: &Presets, now: Instant) {
    if session.views.config.picker {
        session.shut_picker();
        return;
    }
    let names = match presets.names() {
        Ok(names) => names,
        Err(error) => {
            session.views.complain(format!("{error:#}"), now);
            return;
        }
    };
    let listed = names
        .into_iter()
        .map(|name| {
            let preset = presets.read(&name).ok();
            NamedPreset { name, preset }
        })
        .collect();
    session.lift_picker(listed);
}

/// **`dd`：删一份。** 第一下只把光标停着的那一份闩上（盘一个字节都不碰），
/// 第二下——问的与眼下停着的是**同一份**时——才走 [`Presets::remove`]。
///
/// 两下不是防手滑的礼节：删的是盘上长期存着的东西，按错一下没有撤销
/// （与旧界面那一支同一条，停车场 Q74 把这条约束说死了）。
/// **跑着与等待确认时照样删得掉**：只读的是三组设置，预设文件不是设置（ADR 0017）。
fn erase_a_preset(session: &mut Session, presets: &Presets, now: Instant) {
    // **是第几下由状态机一处判**（`Session::ask_then_erase`）：第一下只闩上、答 `None`，
    // 第二下才答出那个名字。这一层不再自己数一遍。
    let Some(name) = session.ask_then_erase() else {
        return;
    };
    match presets.remove(&name) {
        Ok(()) => session.preset_erased(&name, now),
        Err(error) => session.views.complain(format!("{error:#}"), now),
    }
}

/// **起好名那一下 `⏎`：存一份。**
///
/// **第一下盖不掉同名的那一份**：[`Presets::save`] 撞上就是 [`Saved::Taken`]，
/// **预设栏里问一句**、输入行留着（名字还在缓冲里等着改），再按一次 `⏎` 才走
/// [`Presets::replace`]（`CONTEXT.md` 的《预设》那一段：盖掉一份同名的要按两下）。
/// **那一问摆在栏里、不摆在屏底**：这一刻屏底整个让给了输入行（`shell::footer`），
/// 说给屏底等于一个字都没说。
/// **撞名的判断落在盘那一侧**，不落在掀开这一栏时列的那份名单上：名单是进来那一刻的快照。
///
/// 一个字都没打就只关掉那一行（设计稿 `submitInput` 那一支同样不存）。
fn store_a_preset(session: &mut Session, presets: &Presets, now: Instant) {
    let Some(line) = session.views.input.as_ref() else {
        return;
    };
    let name = line.buffer.trim().to_owned();
    if name.is_empty() {
        session.views.input = None;
        return;
    }
    let asked = session.views.config.armed_save.as_deref() == Some(name.as_str());
    let stored = session.preset_to_store();
    let written = if asked {
        presets.replace(&name, &stored).map(|()| Saved::Written)
    } else {
        presets.save(&name, &stored)
    };
    match written {
        Ok(Saved::Written) => {
            session.views.input = None;
            session.preset_saved(&name, stored, now);
        }
        // **同名那一下**：闩在起名那一格上（与 `dd` 那一格分开，见 `ConfigView::armed_save`），
        // 输入行留着、名字照旧在缓冲里，再按一次 `⏎` 就覆盖。**那一问摆在预设栏里**——
        // 这一刻屏底让给了输入行（`shell::footer`），说给屏底等于一个字都没说。
        Ok(Saved::Taken) => session.preset_name_is_taken(&name),
        Err(error) => {
            session.views.input = None;
            session.views.complain(format!("{error:#}"), now);
        }
    }
}

/// **`c`：出灰阶测试图。** 画图与落盘整件事在库里（[`tonefit::write_calibration_chart`]），
/// 这一层只点了个名——与[旧界面那一支](write_chart)同一条分法，`here` 也是同一个。
///
/// **屏底那一句与设计稿不同**：设计稿那一句末尾写的是「（原型不写文件）」，
/// 而这一副真写得出文件，票面第五条要的正是**回话说写到了哪里**。
/// 前半截照设计稿一字不差，末尾那一截换成图落在哪儿（停车场 Q890）。
fn draw_a_chart(session: &mut Session, here: &Path, now: Instant) {
    let drawn = session.chart_profile().and_then(|profile| {
        let out = chart_file(here, &profile);
        tonefit::write_calibration_chart(&profile, &out).map(|()| (profile.panel().resolution, out))
    });
    match drawn {
        Ok((size, out)) => session.views.say_for(
            vec![
                Segment::new("✓ 已生成灰阶测试图", Look::kind(Kind::Done).bold()),
                Segment::plain(format!("（{size}）：写到 {}", session.home_shown(&out))),
            ],
            CHART_LINGERS,
            now,
        ),
        Err(error) => session.views.complain(format!("{error:#}"), now),
    }
}

/// 终端那一侧的事件 → 新会话认得的[输入](Input)：键照 [`translate`]，Ctrl 加一个字母另认
/// （`C-d`／`C-u`／`C-f`／`C-b`／`C-w`），认不出的返回 `None`。滚轮与单击随鼠标那一票接上。
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "真会话切到新界面（session-redesign/15）时接进那条循环"
    )
)]
fn translate_input(pressed: &KeyEvent) -> Option<Input> {
    if pressed.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(letter) = pressed.code
        && letter != 'c'
    {
        return Some(Input::Ctrl(letter));
    }
    translate(pressed).map(Input::Key)
}

/// 这一趟**在确认点上等不等人**，以及它真正走的是哪一种模式（ADR 0012 决定第 3 条）。
///
/// **预览一律接着写出，点名了几个路径都一样**（决定第 3 条，`volume-discovery/07`）。
/// 那一趟因此改走 `Mode::Process`——参照要留着（决定第 5 条：预览走 `Retention::Keep`），
/// 答继续时分析环节才不必重算。「只算不写」在那条路上重述为**不写输出**：
/// 越过预算的页仍建溢写临时文件，运行结束即收走。
///
/// **不按卷数分岔，也不按点名了几个路径分岔。** 从前这里判的是
/// `inputs.len() == 1`，而那判的是「点名了一个**路径**」——发现落地之后
/// （`volume-discovery/03`：`inputs` 的语义是「一批**在里面找卷的地方**」），
/// 一个路径常常就是几十卷，这两件事早已脱钩。决定第 1 条那条内存理由拦的是
/// 「一次押住**全部卷**的参照」，而确认点本来就是逐卷的：逐卷停下来问，
/// 缓存始终只押着当前那一卷，内存一点不涨（见 ADR 0012
/// 《决定第 1 条那条内存理由不覆盖逐卷决策点》）。
///
/// **执行那一趟一格不改**：用户按 `x` 的时候已经拿过主意了，不该在半路再问他一次。
///
/// 判在这一层而不在状态机里：**等不等人是调用方的策略，不是库的行为**
/// （决定第 3 条），而状态机既碰不到线程、也不该替这一层拿这个主意。
fn resuming(request: Request) -> (Request, Resuming) {
    if request.mode == RunMode::DryRun {
        (
            Request {
                mode: RunMode::Process,
                ..request
            },
            Resuming::Waits,
        )
    } else {
        (request, Resuming::GoesOn)
    }
}

/// **列出来**：盘上那份文件里有的那几份，摆成预设那一栏。
///
/// 这三件事（列出来、套一份、存一份）都落在这一层，与[展开](Action::Expand)同一条分法——
/// 那一支要读那一趟攒的报告，这三支要读写用户配置目录下那份 TOML（[`Presets`]）。
///
/// 读得出名字就够：**一份字段过时的预设不该让别的几份列不出来**
/// （见 [`Presets::names`]）。整份文件都读不懂才说一句，而那时那一栏开不了——
/// 开一栏空的比说清为什么更坏。
///
/// 那一栏连**它是哪一份文件列出来的**一起收下（[`Presets::path`]）：存出去的东西落在
/// 用户自己的配置目录里，而屏上得说得出那是哪儿（见 `Picker::file`）。
fn list_presets(session: &mut Session, presets: &Presets) {
    match presets.path().and_then(|file| {
        let file = file.to_path_buf();
        presets.names().map(|names| (names, file))
    }) {
        Ok((names, file)) => session.pick(names, file),
        Err(error) => session.complain(format!("{error:#}")),
    }
}

/// **套一份**：把光标停着的那一份读出来，两层整个换成它。
///
/// **读不懂的预设当场报出库那一侧的原话**（spec 的 story 39）：会话不静默套默认值，
/// 也不另编一句——那句话里已经说清是哪一份、哪一项读不懂。说完仍留在这一栏上，
/// 用户接着挑别的一份。
fn take_preset(session: &mut Session, presets: &Presets) {
    let Some(name) = session
        .picking()
        .and_then(Picker::picked)
        .map(str::to_owned)
    else {
        return;
    };
    match presets.read(&name) {
        Ok(taken) => session.took(&name, taken),
        Err(error) => session.complain(format!("{error:#}")),
    }
}

/// **存一份**：把当前两层写成缓冲里打的那个名字。
///
/// **第一下盖不掉同名的那一份**：[`Presets::save`] 撞上就是 [`Saved::Taken`]，
/// 屏上说一句、闩上「再按一次」（[`Session::name_is_taken`]），第二下才走
/// [`Presets::replace`]。两下不是防手滑的礼节——盖掉的可能是别人手写的一份预设，
/// 而那一份原来的内容换掉之后撤不回来（文件里别的字节动不着，见 `preset::insert`）。
///
/// 撞名的判断**落在盘那一侧**，不落在这一栏进来时列的那份名单上：名单是进来那一刻的
/// 快照，而这中间别处可能刚添了一份同名的。
fn store_preset(session: &mut Session, presets: &Presets) {
    let Some(naming) = session.picking().and_then(Picker::naming).cloned() else {
        return;
    };
    let name = naming.name();
    let stored = session.preset();
    let written = if naming.asked() {
        presets.replace(name, &stored).map(|()| Saved::Written)
    } else {
        presets.save(name, &stored)
    };
    match written {
        Ok(Saved::Written) => session.saved(name),
        Ok(Saved::Taken) => session.name_is_taken(name),
        Err(error) => session.complain(format!("{error:#}")),
    }
}

/// **删一份**：把光标停着的那一份从盘上删掉。
///
/// **第一下只问一句**（[`Session::ask_before_erasing`]），盘一个字节都不碰；
/// 第二下——问的与眼下停着的是**同一份**时——才走 [`Presets::remove`]。
/// 两下不是防手滑的礼节：删的是盘上长期存着的东西，而按错一下没有撤销
/// （停车场 Q74 把这条约束说死了）。
///
/// 那一份在这中间被别处删掉了、或者那份文件整个读不懂了，回的都是库那一侧的原话
/// （已经说清是哪一份、有的是哪几份），这一层原样端到屏底——与套一份读不懂的预设同一条待遇。
fn erase_preset(session: &mut Session, presets: &Presets) {
    let Some(picker) = session.picking() else {
        return;
    };
    let Some(name) = picker.picked().map(str::to_owned) else {
        return;
    };
    if picker.asked() != Some(name.as_str()) {
        session.ask_before_erasing(&name);
        return;
    }
    match presets.remove(&name) {
        Ok(()) => session.erased(&name),
        Err(error) => session.complain(format!("{error:#}")),
    }
}

/// **出灰阶测试图**：按设备设置那块面板画一张，写到 [`chart_file`] 点的那个文件上。
///
/// 落盘整件事在库里（[`tonefit::write_calibration_chart`]）：这一层建的不是目录、
/// 写的不是文件，只是**点了个名**——父目录不在就建出来也是那一头的事
/// （加固批 12 号票把它移进库正是为了这个）。会话因此不必知道 PNG 长什么样，
/// 也不可能在这里给量具掺进一条管线。
///
/// **写不出去就说一句，会话原地不动**（票面第五条）：父目录建不了、盘满，
/// 库那一侧回的都是 `Err`，措辞里已经带着是哪一步、在哪条路径上出的事，
/// 这一层原样端到屏底、不另编一份——与套一份读不懂的预设同一条待遇。
fn write_chart(session: &mut Session, here: &Path) {
    let written = session.chart_profile().and_then(|profile| {
        let out = chart_file(here, &profile);
        tonefit::write_calibration_chart(&profile, &out).map(|()| out)
    });
    match written {
        Ok(out) => session.charted(&out),
        Err(error) => session.complain(format!("{error:#}")),
    }
}

/// 灰阶测试图落在哪个文件上：`here` 下面一个**照 profile 取名**的 PNG。
///
/// **不落在输出目录下面。** 那是被处理的页的去处，而灰阶测试图是量具——
/// 两者走的不是同一条路（`lib.rs` 的第三个 seam）；何况按 `c` 那一刻输出目录多半还空着，
/// 而「先填一个输出目录才出得了灰阶测试图」把设备设置的事拴在了路径与输出上。
/// 落在会话是从哪儿敲起来的那个目录里：那是用户此刻人在的地方，图出来就在手边。
///
/// 名字里带着**型号与屏幕灰阶数**，因为图跟着这两项变（屏幕灰阶数决定排几条阶梯）：
/// 换一台设备出的是另一张图，不该盖掉上一张。同一个 profile 再按一次写的是同样的字节——
/// 灰阶测试图不带记录、不带时间戳，重写一遍等于没变（见 `crate::calibrate`）。
///
/// 全用 ASCII：这张图是要**拷进设备**看的，而那一头认不认得中文文件名说不准
/// （图内留着英文说明也是这个理由）。型号名本来就是内置表里的规范名，
/// 已经是小写连字符那一套。
fn chart_file(here: &Path, profile: &tonefit::Profile) -> PathBuf {
    here.join(format!(
        "tonefit-calibration-{}-{}-levels.png",
        profile.device(),
        profile.panel().gray_levels,
    ))
}

/// 一趟都没跑过时[展开一卷](expand)与[展开一枝](open)那两支说的那一句。
///
/// 起一趟的两个键出自按键表（[`Starters::named`]，`draw::keys` 模块文档
/// 《屏上顺口提到一个键的那几句散文》），措辞是这一层自己的；两个都派不出来时只说要等报告。
fn not_run_yet(starters: &Starters) -> String {
    let keys = starters.named();
    match keys.is_empty() {
        true => "还没跑过：报告出来了才展得开".to_owned(),
        false => format!("还没跑过：先按 {}，报告出来了才展得开", keys.join("或 ")),
    }
}

/// 展开**光标停着的那一卷**的逐页，或者换到下一卷。
///
/// **展开的是报告区那个光标停着的那一卷**（`p3-session-legibility/10`）：
/// 自动滚动着的时候就是最新收摊的那一卷，自动滚动停了就是停着的那一卷——包括**确认点上
/// 那一卷**（`p2-loose-ends/08`：不许摊开上一卷冒充它）。从前它恒是第一卷，
/// 因为那时报告区还没有光标。
///
/// **视口对到那一卷的抬头上**（票面第七条）：这一副只画**这一卷**，抬头就钉在它顶上
/// （见 `super::draw::pages`），光标回到头一页——换一卷之后屏上第一眼看到的因此恒是
/// 那一卷的抬头。从前它要在整份报告里数出那一卷落在第几行（停车场 Q64 那一头），
/// 而那个数连同「报告区展开之后是一整份报告」一起没了。
///
/// **换一卷时列的是哪几页跟着走**（[`Expansion::turned_to`]）：`a` 按下去不该只管一卷。
///
/// 一卷都没有就说一句、不进展开态：展开的是**报告上的一卷**，
/// 而这一趟还没跑过或者第一卷还没跑完时，那样东西根本不在。
///
/// **光标停在没做成的那一卷上时说 [`CANNOT_EXPAND`]、不进展开态**
/// （`p4-parking-lot/10` 收停车场 Q159）：那一行在屏上占着一格，光标此刻停得上去，
/// 而它连一份卷报告都没有——**明说这一卷展不开**，比让 `⏎` 悄悄什么都不做好。
/// 与型号没挑时按 `t`／`x` 是同一条待遇：键照旧摆在屏上，按下去当场说清为什么没有第二步。
fn expand(session: &mut Session, running: &Running, action: Action) {
    let Some(live) = running.live() else {
        // **这一支到不了**：一趟都没跑过时按键表根本不派展开
        // （`super::state::Session::browsing_action` 那一道，停车场 Q167），
        // 而攒着的那一份没有恰恰只有那一种情形。留着是因为 `Running::live` 的取值域上
        // 它在，而悄悄什么都不做比说一句更坏。
        session.complain(not_run_yet(&draw::keys::starters(session)));
        return;
    };
    expanding(session, &live, action);
}

/// [展开](expand)那件事**除掉「找哪一趟要报告」那一步**剩下的全部。
///
/// 分出来是为了**测得动**：本层唯一起线程的地方在 [`Running::start`]，而用例造得出一份
/// [`Live`]（`super::live::fixture` 那几个夹具），造不出一趟真跑着的。挡在前面那一句
/// 「还没跑过」留在 [`expand`] 上——它问的正是「有没有那一趟」。
///
/// **那把锁握到这一支做完**：[`Running::live`] 给的是一把 `MutexGuard`，而拆成两个函数
/// 之后 [`expand`] 还不回去（从前它在 `session.expand` 那两下之前先 `drop`）。
/// 代价有界，而且没有变大多少——贵的那一步（[`Live::branches`]，那是一遍分组）
/// 本来就在锁里，多握的只是一次路径克隆与一次结构体赋值。
/// [`open`] 那一头照旧 `drop` 得掉：它不必把 `live` 借进第二个函数。
fn expanding(session: &mut Session, live: &Live, action: Action) {
    let volumes = live.volumes();
    let Some(first) = volumes.first().copied() else {
        session.complain("报告里还没有卷：一卷跑完才有它的逐页那几行".to_owned());
        return;
    };
    let branches = live.branches();
    let opened = match (action, session.expansion()) {
        // 换一卷：在**这一枝**底下那几卷上挪一格，两头都转一圈（`Expansion::next`）。
        // 只在这一枝里转：层次与发现出来的那棵树一致，一个 `⇥` 不该把人甩到另一枝上去
        // （`volume-discovery/08`）。
        // **先把展开着的那一卷解析一道**（`Live::nearest`）：它可能已经收摊，
        // 而收摊之后「攒着的那一份」那个位置归的是下一卷，不是它。
        (Action::Turn(step), Some(expansion)) => {
            let at = live.nearest(expansion.volume).unwrap_or(first);
            // 那一枝找不着这一步到不了（`at` 恒来自 `live.volumes()`，而每一卷都挂在
            // 某一枝上）；真到了就原地不动，与展开那一支同一条。
            let Some(branch) = branch_of(&branches, at) else {
                return;
            };
            // **只在展得开的那几卷之间转**（[`Branch::expandable`]）：`⇥` 转到没做成的
            // 那一卷上，这一格里就只剩一句话——那不是「换一卷」要给的东西。
            // 一卷都展不开时原地不动（展开态本来就进不来，这一支到不了）。
            let turnable = branch.expandable();
            if turnable.is_empty() {
                return;
            }
            let turned = expansion.turned_to(
                branch.directory.clone(),
                Expansion::next(&turnable, at, step),
            );
            session.expand(turned);
            return;
        }
        // 展开：光标停着的那一卷。它此刻指不着谁（那一卷收摊了）时由
        // `Session::standing` 就近收一收，仍收不着就从头一卷起。
        _ => session.standing(live).unwrap_or(first),
    };
    // **没做成的那一卷展不开**：明说一句，不进展开态（见本函数的文档）。
    if !opened.expandable() {
        session.complain(CANNOT_EXPAND.to_owned());
        return;
    }
    // **哪一枝答不出来就不进展开态**：`opened` 恒来自 `live.volumes()`，而每一卷都挂在
    // 某一枝上（[`crate::render::grouped`] 收的就是那一列），这一支到不了。
    // 拿一个空路径兜底更坏：那是一枝**不存在**的目录，收起之后屏上摆的是目录表、
    // 屏底说的却是卷表（Q170 那一类自相矛盾正是这么来的）。
    let Some(branch) = branch_of(&branches, opened) else {
        session.complain("这一卷不在这一趟的哪一枝上：报告换了一趟，Esc 回目录表".to_owned());
        return;
    };
    let directory = branch.directory.clone();
    session.expand(Expansion::new(directory, opened));
}

/// 光标停在**没做成的那一卷**上按展开时说的那一句（停车场 Q159）。
///
/// **它不重说那一卷为什么没做成**：那句原因跟在卷表上那一行的行尾，出自
/// [`crate::render::failed_volume`]——措辞只有那一处（ADR 0016）。这一句只答
/// 「按下去为什么没有第二层」，并指回屏上已经写着答案的那个地方。
const CANNOT_EXPAND: &str = "这一卷展不开：它一整卷没做成，连一份卷报告都没有，逐页那几行无从谈起——行尾那一句说的就是为什么";

/// **展开光标停着的那一枝**：它底下那几卷摊成卷表（`volume-discovery/08` 票面第二条）。
///
/// 与[展开一卷](expand)同一条分法落在这一层：哪一枝要数那一趟攒下来的报告，
/// 而状态机读不到它。挡在前面的那两句也与那一头同一副形状——一卷都没有就说一句、
/// 不进那一级：展开的是**报告上的一枝**，而这一趟还没跑过或者第一卷还没跑完时，
/// 那样东西根本不在。
///
/// 光标停着的那一卷在哪一枝上就展哪一枝；它此刻指不着谁时从**头一枝**起。
fn open(session: &mut Session, running: &Running) {
    let Some(live) = running.live() else {
        // 与[展开一卷](expand)那一支同一条：一趟都没跑过时这个键不派动作，
        // 焦点也进不到报告区上去——这一支到不了。
        session.complain(not_run_yet(&draw::keys::starters(session)));
        return;
    };
    let branches = live.branches();
    let standing = session.standing(&live);
    let Some(branch) = standing
        .and_then(|at| branch_of(&branches, at))
        .or_else(|| branches.first())
    else {
        session.complain("报告里还没有卷：一卷跑完才有它那一枝".to_owned());
        return;
    };
    let directory = branch.directory.clone();
    drop(live);
    session.open(directory);
}

/// 这一卷挂在**哪一枝**上。**分组只有一处出处**（`crate::render::grouped`），
/// 这里只在算好的那几枝里找它。
///
/// 出的是整一枝而不是它的某一格：这一层要的两样（那一枝叫什么、它底下有哪几卷）
/// 同出一次查找，各查一遍就是把 `branches` 扫两趟。
fn branch_of(branches: &[Branch], at: Volume) -> Option<&Branch> {
    branches.iter().find(|branch| branch.volumes.contains(&at))
}

/// 终端那一侧的键码 → 会话认得的 [`Key`]。
///
/// **这是本仓库唯一一处认得 crossterm 键码的地方**，也是状态机能脱离终端受测的原因：
/// 翻译在这里，规矩在那边。认不出的键（功能键、翻页键）返回 `None`，
/// 由调用方原地忽略——状态机不必为它们各留一个「没有意义」的取值。
fn translate(pressed: &KeyEvent) -> Option<Key> {
    if pressed.modifiers.contains(KeyModifiers::CONTROL) && pressed.code == KeyCode::Char('c') {
        return Some(Key::Interrupt);
    }
    Some(match pressed.code {
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        // `⇧⇥` 是一个**单独的**键码，不是 Tab 加一个修饰键。
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Esc => Key::Esc,
        KeyCode::Char(' ') => Key::Space,
        KeyCode::Char(character) => Key::Char(character),
        // **不进缓冲的那一个**（[`Key::F1`]）：打字那两块上掀得开全部键那一张的只有它
        // （`p4-parking-lot/07` 票面第三条）。别的功能键照旧不认——认不出的键由调用方
        // 原地忽略，状态机不必为它们各留一个「没有意义」的取值。
        KeyCode::F(1) => Key::F1,
        _ => return None,
    })
}

/// 借来的终端。**它的 [`Drop`] 是「退出时终端恢复原状」这条验收唯一的实现**——
/// 正常退出、`?` 半路返回、恐慌展开，三条路都经过它。
///
/// 恐慌那一条还多一道：[`hook_the_panic`] 让恐慌信息印在**还原之后**的屏幕上。
/// 只靠 `Drop` 的话那几行会印进 alternate screen，然后随着它一起消失。
struct Screen {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl Screen {
    fn open() -> Result<Self> {
        hook_the_panic();
        enable_raw_mode()?;
        // 进了 raw mode 之后每一步都可能失败，而失败也得把终端还回去——
        // `Screen` 还没造出来，`Drop` 顶不上，只能在这里自己收。
        match execute!(stderr(), EnterAlternateScreen)
            .and_then(|()| Terminal::new(CrosstermBackend::new(stderr())))
        {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = restore();
                Err(error.into())
            }
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // 还不回去也没有第二条路可走，而这时正在退出——错误没有去处。
        let _ = restore();
    }
}

/// 把终端还原：退出 alternate screen、关掉 raw mode。
///
/// **两件事各收各的，中间不放 `?`。** 验收要的是「不留在 raw mode **或** alternate screen 里」，
/// 而 `?` 会让前一件的失败把后一件整个吃掉——退不出 alternate screen 的那一次，
/// 终端就连 raw mode 一起留着了。两件都做完，再把先出的那个错误交出去。
fn restore() -> std::io::Result<()> {
    let left = execute!(stderr(), LeaveAlternateScreen);
    let raw = disable_raw_mode();
    left.and(raw)
}

/// 恐慌之前先把终端还原，再让原来那个钩子把信息印出来。
///
/// 只挂一次：会话一次运行只进一回，而 `Once` 让「以后多进几回」也不会把钩子套成一串。
fn hook_the_panic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore();
            previous(info);
        }));
    });
}

/// 无参数、而 stderr 不是终端时的说法。
///
/// 两件事都得说到：**为什么没进会话**，以及**带参数那一路要什么**。
/// 后半段原样取自 clap——那条用法提示是它写得最好的东西，
/// 这里不重抄一份（重抄的那份迟早与 `Cli` 走散）。
fn no_terminal_error() -> anyhow::Error {
    let usage = crate::Cli::try_parse_from(["tonefit"]).map_or_else(
        |error| error.render().to_string(),
        // 到不了：`Cli` 那几项必填拦在前面（见 `REQUIRED_BY_CLAP`）。
        |_| String::new(),
    );
    anyhow!(
        "这里没有终端：不带参数敲 tonefit 是要进**会话**，而会话画在 stderr 上——\
         这一次 stderr 不是终端（CI、或者 2> 重定向到了文件）。\n\
         带参数那一路不需要终端，它要的是：\n\n{usage}"
    )
}

/// 新会话那一支的用例：喂交互序列，走完对交互期望屏（spec《交互序列》；`session-redesign/06`）。
#[cfg(test)]
mod redesign {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::super::cover::Overlay;
    use super::super::draw::design::{self, Expected, assert_no_background, assert_same_cells};
    use super::super::look::{Kind, Look, Segment};
    use super::super::run::Running;
    use super::super::scene::{self, Scene, Step};
    use super::super::shell;
    use super::super::state::{Exit, Key};
    use super::super::view::{Cursor, Focus, Input, Pane, Window};
    use crate::preset::Presets;
    use std::path::PathBuf;
    use tonefit::FitMode;

    /// 用例里那份预设文件：位置点在**临时目录**里（[`Presets::at`]），
    /// 因此不必去改进程的环境变量（`tests/preset.rs` 说过为什么不改）。
    /// 一个用户的东西都不碰——与旧那一支的 `tests::presets` 同一招。
    fn presets(space: &tempfile::TempDir) -> Presets {
        Presets::at(space.path().join("tonefit").join("presets.toml"))
    }

    /// 灰阶测试图在用例里落到哪儿：**那份预设文件的上一层**——场景夹具的临时目录里
    /// （真会话里是会话从哪儿敲起来的那个目录）。**不点在家目录里**：家目录底下只有
    /// 假盘那几样，`~/` 底下多一项会让补全那几串红（与预设文件摆在那里同一条，停车场 Q824）。
    fn charts_land_in(scene: &Scene) -> PathBuf {
        scene
            .presets
            .path()
            .expect("用例里那份预设文件的位置是定死的")
            .parent()
            .expect("它上一层")
            .to_path_buf()
    }

    /// 从这一串的起点场景起，逐步喂给新会话那一支；回走完那一刻的场景、那一趟与最后一步的去留。
    fn walked(name: &str) -> (Scene, Running, Exit) {
        let sequence = scene::sequence(name);
        let mut scene = Scene::named(&sequence.scene);
        let mut running = match scene.live.take() {
            Some(live) => Running::holding(live),
            None => Running::default(),
        };
        let mut now = scene.now();
        let window = Window {
            cols: sequence.size.0,
            rows: sequence.size.1,
        };
        // 预设那几支要读写盘，灰阶测试图要一个落点：两样都点在**临时目录**里
        // （`Scene::presets` 与 [`charts_land_in`]），一个用户的东西都不碰。
        let here = charts_land_in(&scene);
        let mut exit = Exit::Stay;
        let mut advanced = false;
        for step in &sequence.steps {
            assert!(
                !matches!(step, Step::Resize(_, _)),
                "「{name}」里换尺寸那一种步随它那一票接上"
            );
            // **推进几秒**：夹具没有线程，推进之后那一趟走到哪儿由这一串自己的场景数据说
            // （`Scene::advance_to`，停车场 Q805 的同一条）。界面状态一格不动。
            //
            // **一串只推得动一次**：场景数据说的是这一串**走完那一刻**，一串只有一份
            // ——推第二次拿的还是同一份，那是假的（停车场 Q852）。
            //
            // **推进之后还有输入照样摆得对**，前提是后面那几步一格都不动那一趟：
            // `running-j-advance-F` 的 `F` 只扳自动滚动那一格与屏底那一句。
            // 按住这一条的是**屏本身**——总览那三行印着第几卷、已用多久、走了几步，
            // 后面那几步真动了那一趟，走完那一屏当场对不上。
            if matches!(step, Step::Advance(_)) {
                assert!(
                    !advanced,
                    "「{name}」推进了两次：夹具只摆得出走完那一刻（Q852）"
                );
                advanced = true;
                scene.advance_to(scene::sequence_data(name));
                running = Running::holding(scene.live.take().expect("推进之后那一趟"));
                now = scene.now();
                continue;
            }
            for input in step.inputs() {
                exit = super::input(
                    &mut scene.session,
                    &mut running,
                    &scene.presets,
                    &here,
                    now,
                    window,
                    input,
                );
            }
            // **夹具没有线程**：按到立即停止之后替那条线程收手。真会话里那条线程收到这个字
            // 就停在页边界上，主循环随后 `reap` 到它、会话回到结束了（`super::drive`）——
            // 这一步走的是同一条路，只是当场走完。
            if running.pressed() == tonefit::Instruction::Abort
                && let Some(mut live) = running.live()
                && !live.ended()
            {
                // 库交出来的那一份与攒着的只差计时（与 `scene::replay` 收场那一段同形）。
                let elapsed = live.overall().elapsed;
                live.run_finished(tonefit::RunOutcome::Stopped(tonefit::Instruction::Abort));
                let mut report = live.report().clone();
                report.elapsed = elapsed;
                live.returned(Ok(report));
                drop(live);
                scene.session.run_finished();
            }
        }
        (scene, running, exit)
    }

    /// 走完那一刻画一屏。
    fn painted(scene: &Scene, running: &Running, (width, height): (u16, u16)) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        let live = running.live();
        terminal
            .draw(|frame| shell::draw(frame, &scene.session, live.as_deref(), scene.now()))
            .expect("画得出来");
        terminal.backend().buffer().clone()
    }

    /// 走完一串，逐格对它的交互期望屏，顺带核一个背景色都没设（停车场 Q737）。
    fn assert_sequence(name: &str) -> Scene {
        assert_sequence_with(name, |expected| expected)
    }

    /// **走完一串再比一次屏，那一套只有这一处**：喂完、核会话还开着、画一屏、
    /// 核一个背景色都没设、逐格对期望屏。
    ///
    /// `adjust` 是调用方**动一动期望屏**的机会：设计稿在那儿摆的字与实现照规矩写下的
    /// 差一截缩进（[`Expected::shifted`]）、设计稿写着一句而实现照规矩不写它
    /// （[`Expected::blanked`]）、或者差的只有它自己那套模拟算出来的一格
    /// （[`Expected::cell_like`]）。**动完仍是一条断言**，而**每一处用它的地方都得在
    /// 用例上写清是哪一条停车场条目**。
    fn assert_sequence_with(name: &str, adjust: impl FnOnce(Expected) -> Expected) -> Scene {
        let (scene, running, exit) = walked(name);
        assert_eq!(exit, Exit::Stay, "「{name}」走完会话还开着");
        let size = scene::sequence(name).size;
        let buffer = painted(&scene, &running, size);
        assert_no_background(&buffer);
        assert_same_cells(&buffer, &adjust(design::sequence(name)));
        scene
    }

    /// 同上，另外**把期望屏上几段往右推几格**（[`Expected::shifted`]）。
    fn assert_sequence_shifting(name: &str, shifts: &[(usize, u16, u16, u16)]) -> Scene {
        assert_sequence_with(name, |expected| {
            shifts
                .iter()
                .fold(expected, |expected, (row, from, width, by)| {
                    expected.shifted(*row, *from, *width, *by)
                })
        })
    }

    /// 同上，另外**把期望屏上几格各换成它右边那一格**（[`Expected::cell_like`]）。
    fn assert_sequence_like(name: &str, cells: &[(usize, u16)]) -> Scene {
        assert_sequence_with(name, |expected| {
            cells.iter().fold(expected, |expected, (row, at)| {
                expected.cell_like(*row, *at, at + 1)
            })
        })
    }

    /// 同上，另外**抹掉期望屏上一段**（[`Expected::blanked`]）。
    fn assert_sequence_blanking(name: &str, (row, from, width): (usize, u16, u16)) -> Scene {
        assert_sequence_with(name, |expected| expected.blanked(row, from, width))
    }

    /// **`j`／`k` 挪光标**，走完与期望屏逐格相等（票面第二条）。
    #[test]
    fn j_and_k_move_the_cursor_over_the_paths() {
        assert_sequence("fresh-j");
        assert_sequence("fresh-k");
    }

    /// **空格勾选、再按一次取消**：勾掉外层那一条，里层那一句「已包含在」跟着没了。
    #[test]
    fn space_toggles_the_checkbox_and_back() {
        let scene = assert_sequence("fresh-Space");
        assert!(!scene.session.scope.paths[0].on);
        let scene = assert_sequence("fresh-Space-Space");
        assert!(scene.session.scope.paths[0].on);
    }

    /// **`d` 按了前半截右端留待续记号，`dd` 删一条**：光标停到下一条上，屏底说已删除。
    #[test]
    fn d_waits_for_its_second_half_and_dd_deletes_the_path() {
        assert_sequence("fresh-d");
        let scene = assert_sequence("fresh-dd");
        assert_eq!(scene.session.scope.paths.len(), 12);
    }

    /// **还没开始时 `q` 交出退出**（票面第二条）：那一支回的是退出，屏上不再画下一帧——
    /// 设计稿在这一串上画的那句「原型里不会真的退出」是原型自己的话，实现不画它（停车场 Q774）。
    #[test]
    fn q_before_the_run_hands_out_the_exit() {
        let (_, _, exit) = walked("fresh-q");
        assert_eq!(exit, Exit::Leave);
    }

    /// **按停止那个键真的走到了跑着的那一趟身上**（票面第四条），**两级各自到达**。
    ///
    /// 与旧那一支那条用例（`pressing_stop_reaches_the_run_that_is_going`）同一条接头：
    /// 状态机把闩升一级，本层把升到的那一级交给 [`Running::stop`]。**两头记的是同一个字**。
    /// 一个终端都不碰——[`super::input`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn pressing_stop_through_the_new_input_reaches_the_run_at_both_levels() {
        let mut session = crate::session::state::Session::new();
        let mut running = Running::default();
        let now = std::time::Instant::now();
        let window = Window {
            cols: 120,
            rows: 36,
        };
        session.run_started();
        // 这一条一个预设键都不按，灰阶测试图也不出（见 [`presets`]）。
        let space = tempfile::tempdir().expect("建得出临时目录");
        let nowhere = presets(&space);
        let press = |session: &mut _, running: &mut _| {
            super::input(
                session,
                running,
                &nowhere,
                space.path(),
                now,
                window,
                Input::Key(Key::Char('s')),
            )
        };

        // 一次：做完再停。
        assert_eq!(press(&mut session, &mut running), Exit::Stay);
        assert_eq!(session.stopping(), tonefit::Instruction::Finish);
        assert_eq!(running.pressed(), tonefit::Instruction::Finish);

        // 再一次：立即停止。
        assert_eq!(press(&mut session, &mut running), Exit::Stay);
        assert_eq!(session.stopping(), tonefit::Instruction::Abort);
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 第三次起那个键不再动它：闩只升不降。
        assert_eq!(press(&mut session, &mut running), Exit::Stay);
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 还没开跑时按它什么都不发生：那一档表上根本没有这个键。
        let mut idle = crate::session::state::Session::new();
        let mut nothing = Running::default();
        assert_eq!(press(&mut idle, &mut nothing), Exit::Stay);
        assert_eq!(nothing.pressed(), tonefit::Instruction::Continue);
    }

    /// **`t`／`x` 开跑**（票面第一条）：起一趟走的是 [`press`] 那条路（拼 `Request`、
    /// 起线程、把观察者接上去），会话回到任务视图、卷列表换成清点中那一副，屏底说这一趟做什么。
    ///
    /// 走完那一屏与设计稿逐格相等——两串各按一个键，`t` 是预览、`x` 是转换。
    #[test]
    fn t_and_x_start_a_run_and_come_back_to_the_task_view() {
        for name in ["fresh-t", "fresh-x"] {
            // 按下去那一刻起就是清点中：输出目录那一行行尾那十格照 Q807 抹掉。
            let scene = assert_sequence_blanking(name, (6, 26, 10));
            assert_eq!(
                scene.session.views.view,
                super::super::view::View::Task,
                "「{name}」开跑总回到任务视图"
            );
            assert!(
                !matches!(
                    scene.session.stage(),
                    super::super::state::Stage::Fresh | super::super::state::Stage::Ended
                ),
                "「{name}」那一趟真起来了"
            );
        }
    }

    /// **清点中按停止**（票面第三条）：抬头接一句「正在停止」，屏底说再按一次立即停。
    #[test]
    fn stopping_while_surveying_says_so_on_the_title_and_the_footer() {
        // 输出目录那一行行尾那十格照 `shell` 那条清点中的用例抹掉：停车场 **Q807**。
        assert_sequence_blanking("survey-s", (6, 26, 10));
    }

    /// **`s` 按一次、再按一次**（票面第三条）：一次是做完当前卷就停，两次立即停止——
    /// 那一趟当场结束，当前卷标成已中断、未保存。
    #[test]
    fn s_once_finishes_the_volume_and_twice_stops_at_once() {
        let scene = assert_sequence("running-s");
        assert_eq!(scene.session.stopping(), tonefit::Instruction::Finish);
        let scene = assert_sequence("running-s-s");
        assert_eq!(scene.session.stopping(), tonefit::Instruction::Continue);
    }

    /// **`s` 按一次、做完当前卷就停**（票面第二条、第二个验收框的「两种结束抬头」之一）：
    /// 再推进 30 秒，那一趟收了场——总览抬头换成「已停止 ⋅ 处理到第 27 卷 ⋅ 用时 …」、
    /// 右端换成输出目录，框右端那一枚自动滚动不在了，屏底换成 `t`／`x` 再开一趟
    /// 与 `l → 每页结果`。另一种抬头（「已中断」）走 `running-s-s`。
    ///
    /// **推进那几秒由这一串自己的场景数据接上**（`Scene::advance_to`）：夹具没有线程，
    /// 那一卷不会自己做完（停车场 Q805 的同一条）。
    #[test]
    fn stopping_once_and_letting_it_finish_says_it_stopped() {
        let scene = assert_sequence("running-s-advance");
        assert_eq!(
            scene.session.stage(),
            super::super::state::Stage::Ended,
            "那一趟收了场"
        );
    }

    /// **立即停止之后那一卷展不开**：屏底说它没有保存，也没有每页结果。
    #[test]
    fn an_aborted_volume_says_why_it_cannot_be_opened() {
        assert_sequence("running-s-s-l");
    }

    /// **跑着时按 `q` 不退出**（票面第三条）：屏底说先按 `s` 停止，或按 `C-c` 立即退出。
    #[test]
    fn q_while_running_refuses_and_says_what_to_press() {
        let (_, _, exit) = walked("running-q");
        assert_eq!(exit, Exit::Stay, "跑着时 `q` 不退出");
        assert_sequence("running-q");
    }

    /// **半屏与一屏那四个键挪光标**（`session-redesign/10` 票面第二条的滚动那几串）：
    /// `C-d`／`C-u` 挪[半屏](super::super::view::Window::page)、`C-f`／`C-b` 挪一屏，
    /// `G`／`gg` 到底到顶；走完与期望屏逐格相等。36 行的窗口上一屏是 24 行、半屏 12 行。
    #[test]
    fn half_a_screen_and_a_whole_screen_move_the_cursor_that_many_rows() {
        for name in [
            "ended-C-d",
            "ended-C-d-C-u",
            "ended-C-f",
            "ended-C-f-C-b",
            "ended-G",
            "ended-G-gg",
        ] {
            assert_sequence(name);
        }
    }

    /// **`h` 在卷行上收起它那个目录、光标跟着停到目录行上，`l`／`⏎` 再展开**
    /// （`CONTEXT.md` 的《展开》）：三串走完与期望屏逐格相等，屏底那一件跟着光标那一行
    /// 从 `l → 每页结果` 换成 `l → 展开`。
    ///
    /// **`ended-h-Enter-Enter` 这一票没接**：那一串要 `⏎` 在**展开着的**目录行上收起它，
    /// 而表上 `l` 与 `⏎` 派的是同一件事（一律展开）——停车场 **Q806** 记着那一条，
    /// 收法归鼠标那一票（双击等于 `⏎`）。
    #[test]
    fn h_collapses_the_directory_of_the_volume_and_l_opens_it_again() {
        let scene = assert_sequence("ended-h");
        assert!(
            matches!(scene.session.views.task.cursor, Cursor::Directory(_)),
            "光标跟着停到目录行上"
        );
        assert_sequence("ended-h-l");
        assert_sequence("ended-h-Enter");
    }

    /// **没做成的那一卷停得住、展不开**：屏底说的是**它行尾那句原因**
    /// （票面第二条那一句「转换失败的卷…行尾是那句原因」；`CONTEXT.md` 的
    /// 《停得住 / 展得开》：「没做成的那一句就是它行尾的原因」）。
    #[test]
    fn a_failed_volume_says_the_reason_from_its_own_row() {
        assert_sequence("ended-failed-l");
    }

    /// 走完一串，**画之前把开着那一卷的逐页详略补齐**，再逐格对它的期望屏。
    ///
    /// **逐页结果只有屏上开着的那一卷有整份**（[`scene`] 的模块文档，停车场 Q736）：
    /// 起点那一景（`ended`）里那一卷还没开着，它那一份因此只有需留意的那一页，
    /// 别的页由夹具照灰阶分布补出来——补出来的那几页缩放比与画质分都是占位的数。
    /// 而**这一串自己的场景数据带着它那 189 页**（`l` 按下去之后它就是开着的那一卷）。
    ///
    /// **键一个都不碰那一趟**（这一串没有推进、没有答话、没有开跑），两份说的是**同一趟**、
    /// 只是详略不同，照这一串那一份重放一遍即可；界面状态由 [`Scene::advance_to`] 自己保住，
    /// 走完那一屏仍是**逐格断言**。重放前后核一遍那一趟真没动（各卷此刻怎么样、收摊了几卷），
    /// 补的只是详略。
    ///
    /// **停车场 Q876**：真要收干净是让导出给每一景都带上整份逐页
    /// （ADR 0019 决定第 13 条：先改设计稿、重导）。
    fn assert_sequence_with_every_page_of_the_open_volume(name: &str) -> Scene {
        let (mut scene, running, exit) = walked(name);
        assert_eq!(exit, Exit::Stay, "「{name}」走完会话还开着");
        let (states, settled) = {
            let live = running.live().expect("这一串在一趟里");
            (live.states().to_vec(), live.report().volumes.len())
        };
        drop(running);
        scene.advance_to(scene::sequence_data(name));
        let running = Running::holding(scene.live.take().expect("重放之后那一趟"));
        {
            let live = running.live().expect("重放之后那一趟");
            assert_eq!(live.states(), states, "「{name}」补详略那一下动了那一趟");
            assert_eq!(live.report().volumes.len(), settled, "{name}");
        }
        let size = scene::sequence(name).size;
        let buffer = painted(&scene, &running, size);
        assert_no_background(&buffer);
        assert_same_cells(&buffer, &design::sequence(name));
        scene
    }

    /// **卷行上 `l`／`⏎` 换屏进每页结果，`a` 切列法，`j` 挪一页，`h` 回卷列表原处**
    /// （`session-redesign/11` 票面第二条那一串）。
    ///
    /// 五串一路走完：`l` 进去（默认只列需留意的页）、`a` 换成全部页、`j` 挪一页、
    /// `h` 回卷列表；`⏎` 与 `l` 派的是同一件事。**回去那一下卷列表的光标一格没动**
    /// ——那一行本来就停在光标底下。
    ///
    /// 列着全部页的那两串走[补齐详略那一手](assert_sequence_with_every_page_of_the_open_volume)。
    /// 只列需留意的页那三串**不必补**：屏上那一行就是需留意的那一页，
    /// 而它在起点那一景的场景数据里本来就是整份的。
    #[test]
    fn l_on_a_volume_row_opens_the_pages_and_h_comes_back_to_the_same_row() {
        let scene = assert_sequence("ended-l");
        assert!(
            scene.session.views.task.pages.is_some(),
            "`l` 换屏进了每页结果"
        );
        assert_eq!(scene.session.views.block(), Focus::Pages);
        assert_sequence("ended-Enter");
        assert_sequence_with_every_page_of_the_open_volume("ended-l-a");
        assert_sequence_with_every_page_of_the_open_volume("ended-l-a-j");
        let scene = assert_sequence("ended-l-a-j-h");
        assert!(scene.session.views.task.pages.is_none(), "`h` 回了卷列表");
        assert!(
            matches!(scene.session.views.task.cursor, Cursor::Volume(_)),
            "回到原处：光标仍停在那一卷的行上"
        );
    }

    /// **跳过的卷进得来，只说一句它跳过了**（票面第五条末一句；`CONTEXT.md` 的
    /// 《停得住 / 展得开》：跳过的也算）：它这一趟一页都没重新分析，那一屏里
    /// 连灰阶分布与列头都没有——给的是一句话，不是一张空表。
    #[test]
    fn a_skipped_volume_opens_and_only_says_that_it_was_skipped() {
        let scene = assert_sequence("ended-skipped-l");
        assert!(
            scene.session.views.task.pages.is_some(),
            "跳过的卷照样进得来"
        );
    }

    /// **每页结果上 `a`／`j`／`h`／`Esc`**（票面第二条那一串、第四条）：
    /// `a` 换列法（屏底那一件跟着换口）、`j` 挪一页、`h` 与 `Esc` 都回卷列表。
    #[test]
    fn the_pages_pane_switches_its_listing_moves_a_row_and_closes() {
        assert_sequence("pages-a");
        assert_sequence("pages-j");
        for name in ["pages-h", "pages-Escape"] {
            let scene = assert_sequence(name);
            assert!(
                scene.session.views.task.pages.is_none(),
                "「{name}」走完回到了卷列表"
            );
        }
    }

    /// **备注行上 `⏎` 掀开说明卡、`Esc` 关**（票面第二条、第二个验收框）：
    /// 卡居中、底下整屏压暗、卷列表的框跟着细下来，关掉之后底下一格不差地回来。
    ///
    /// 两种备注各掀一张：**无法访问**那一张框与抬头是出事色（`ended-note-Enter`），
    /// **非漫画文件**那一张是聚焦色、正文里三个文件各一段（`ended-nonvolume-Enter`）。
    #[test]
    fn enter_on_a_note_row_lifts_the_card_and_escape_closes_it() {
        let scene = assert_sequence("ended-note");
        assert!(scene.session.views.cover.is_none(), "还没掀开");
        let scene = assert_sequence("ended-note-Enter");
        assert!(
            matches!(scene.session.views.cover, Some(Overlay::Note { .. })),
            "`⏎` 掀开的是说明卡"
        );
        let scene = assert_sequence("ended-note-Enter-Escape");
        assert!(scene.session.views.cover.is_none(), "`Esc` 关掉了它");
    }

    /// **非漫画文件那一条备注的说明卡**：正文那几行**出自报告末尾那一小结**
    /// （`render::non_volume_stack`，ADR 0016），路径把家目录缩成 `~`。
    ///
    /// **正文折下来的那两行往右推四格**：设计稿那一头的折行**不带悬挂缩进**，
    /// 而屏上折行只有一套规矩（`crate::wrap`：行首那一截缩进跟着折下来的每一行走，
    /// 停车场 Q32／Q114）——那一条仍然成立，因此这一串照它缩。推开之后仍是一条断言，
    /// 停车场 **Q845**。
    #[test]
    fn the_card_of_the_ignored_files_says_what_the_report_says() {
        let scene =
            assert_sequence_shifting("ended-nonvolume-Enter", &[(20, 25, 70, 4), (23, 25, 70, 4)]);
        assert!(matches!(
            scene.session.views.cover,
            Some(Overlay::Note { .. })
        ));
    }

    /// **结束之后 `o` 回到开跑之前那一副**（票面第二条、第二个验收框）：卷列表从那棵树
    /// 换回处理路径（末行「＋ 添加路径」跟着回来）、总览换回「还没开始」、屏底换回开跑之前
    /// 那几件，屏底那一句说上次的结果会在退出时打印。**那一趟仍攒在手上**——
    /// 退出时 stdout 上印得出它。
    #[test]
    fn o_after_the_run_goes_back_to_the_path_list() {
        let (scene, running, _) = walked("ended-o");
        assert_eq!(
            scene.session.stage(),
            super::super::state::Stage::Fresh,
            "回到了开跑之前那一档"
        );
        assert!(!scene.session.views.task.surveyed, "树不在了");
        assert!(
            running.report().is_some(),
            "上一趟那一份报告还在，退出时印得出"
        );
        assert_sequence("ended-o");
    }

    /// **结束了那一档上 `o`／`i`／`t`／`x` 各派什么**（票面第四条那一句「`o`／`i` 回到
    /// 开跑之前那一副，`t`／`x` 再开一趟」）：两个键回路径列表、两个键再开一趟。
    ///
    /// `i` 与 `o` 派的是**同一件事**（表上那两行），而 `i` 在还没开始那一档派的是「修改这一条」
    /// ——同一个键在两档上两件事，问的因此得是「这一档上派什么」。
    #[test]
    fn at_the_end_o_and_i_go_back_and_t_and_x_start_another_run() {
        let mut scene = Scene::named("ended");
        let now = scene.now();
        let phase = super::super::keymap::Phase::Ended;
        let deed = |scene: &mut Scene, letter: char| {
            scene
                .session
                .deed_of(Input::Key(Key::Char(letter)), phase, now)
        };
        for letter in ['o', 'i'] {
            assert_eq!(
                deed(&mut scene, letter),
                Some(super::super::keymap::Deed::BackToPaths),
                "结束了那一档上 `{letter}` 该回路径列表"
            );
        }
        assert_eq!(
            deed(&mut scene, 't'),
            Some(super::super::keymap::Deed::Preview)
        );
        assert_eq!(
            deed(&mut scene, 'x'),
            Some(super::super::keymap::Deed::Convert)
        );
    }

    /// **结束之后 `?` 掀开的那一张只列这一档派得出的键**（票面预告的 `ended-help`）：
    /// `t`／`x` 写的是「再预览」「再转换」那两行长句，`s` 停止整个不在，
    /// 「路径」那一组只剩 `o i → 返回路径列表`。
    #[test]
    fn the_key_sheet_at_the_end_lists_what_this_phase_deals() {
        assert_sequence("ended-help");
    }

    /// **结束了 `q` 交出退出**（spec《退出会话》：`q` 只在还没开始与结束了时退出）。
    ///
    /// 与 `fresh-q` 那一条同一个判法（停车场 **Q774**）：设计稿在这一串上画的那句
    /// 「退出（原型里不会真的退出）」是原型自己的话——它自己就这么写着——实现不画它，
    /// 这一条断的是**那一支交出退出**，不比屏。
    #[test]
    fn q_after_the_run_hands_out_the_exit() {
        let (_, _, exit) = walked("ended-q");
        assert_eq!(exit, Exit::Leave);
    }

    // ───────────────────────── 自动滚动、跳转与搜索（09） ─────────────────────────

    /// **按键挪光标就暂停自动滚动**（票面第一条那一档「暂停」，加第二条那一串）：
    /// 框右端从 `[自动滚动]` 换成 `[已暂停自动滚动 ⋅ F 恢复]`，屏底说一句
    /// 「已暂停自动滚动 ⋅ 按 F 恢复」，光标真挪了一行。
    ///
    /// **不暂停的话下一帧就被拽回去**：`watch_the_run` 每一下把光标带回正在处理的那一卷
    /// （08 号票因此提前做了暂停那一格本身），这一条钉的是它的两句外显。
    #[test]
    fn moving_the_cursor_pauses_following_and_says_so() {
        let scene = assert_sequence("running-j");
        assert!(!scene.session.views.task.follow, "挪过光标之后不再跟");
    }

    /// **暂停之后推进几秒光标一格不动**（票面第二条那一串）：那一趟往前走了三秒、
    /// 当前卷换了一卷，而光标仍停在 `j` 挪到的那一行上；屏底那一句到点退回按键提示，
    /// **`[F → 自动滚动]` 这时摆出来了**（跟着的时候不摆按不动的键）。
    #[test]
    fn while_paused_the_cursor_stays_where_it_was_put() {
        let scene = assert_sequence("running-j-advance");
        assert!(!scene.session.views.task.follow);
    }

    /// **`F` 交回自动滚动**（票面第一条「`F` 交回」）：那一格扳回开着、光标当场跟到
    /// 正在处理的那一卷（推进之后那一卷换了，光标跟着换），屏底说
    /// 「自动滚动：跟到正在处理的卷」。
    ///
    /// **这一串的推进不在最后一步上**：夹具只摆得出这一串**走完那一刻**（停车场 Q852），
    /// 而 `F` 只扳自动滚动那一格与屏底那一句、一个字节都不碰那一趟——推进之后那一份
    /// 因此就是 `F` 之后那一份。按住这一条的是屏本身：总览那三行印着第几卷、已用多久、
    /// 走了几步。
    #[test]
    fn f_hands_following_back_and_the_cursor_catches_up() {
        let scene = assert_sequence("running-j-advance-F");
        assert!(scene.session.views.task.follow, "`F` 之后又跟上了");
    }

    /// **`]d` 连跳两次、`[d` 回跳**（票面第二条那三串）：落点是转换失败的卷、
    /// 进了隔离的卷、有需留意的页的卷与无法访问的地方；屏底报「问题 第几个/共几个」，
    /// 跳过去即暂停自动滚动。
    ///
    /// 这一景共四处问题：`哆啦A梦/第05卷`（页面超宽）· `海贼王/第07卷`（进了隔离）·
    /// `海贼王/第11卷`（转换失败）· 那一条无法访问的备注。光标开跑时跟在第 15 卷上
    /// （前三处之后），头一下因此跳到第四处；再一下**绕回头一个**，而那一卷在收着的
    /// 目录里——跳过去把它那个目录展开了。
    #[test]
    fn bracket_d_jumps_between_problems_both_ways() {
        let scene = assert_sequence("running-]d");
        assert!(!scene.session.views.task.follow, "跳过去之后不再跟");
        let scene = assert_sequence("running-]d-]d");
        assert!(
            matches!(
                scene.session.views.task.cursor,
                super::super::view::Cursor::Volume(_)
            ),
            "绕回头一个问题，它是一卷"
        );
        assert_sequence("running-]d-]d-[d");
    }

    /// **结束之后 `]d`／`[d` 照样跳**（票面第二条那两串）：这一趟七处问题，
    /// 光标正停在第二处上——`]d` 到第三处，`[d` 到第一处（**光标那一行本身不算
    /// 「下一个」**，两个方向都不算）。
    #[test]
    fn bracket_d_jumps_between_problems_after_the_run() {
        assert_sequence("ended-]d");
        assert_sequence("ended-[d");
    }

    /// **`/` 开搜索那一行**（票面第三条）：屏底换成 `/` 加缓冲加光标，右端只剩
    /// `⏎ → 跳到结果` 与 `Esc → 取消`（这一种输入行不补全）；底下那张列表框细了、
    /// 光标行首换成暗的 `›`。打上字之后**匹配处加下划线**、框底边左起写着这一句与 `n`／`N`。
    #[test]
    fn slash_opens_the_search_line_and_underlines_what_matches() {
        let scene = assert_sequence("running-slash");
        assert_eq!(
            scene.session.views.searching(),
            None,
            "刚开那一行还没打字：空串不算在搜，框底边因此不摆那一截"
        );
        assert!(scene.session.searching_line(), "而那一行确实开着");
        let scene = assert_sequence("running-search-typed");
        assert_eq!(scene.session.views.searching(), Some("海贼"));
        assert!(scene.session.views.task.follow, "打字不挪光标，照旧跟着");
    }

    /// **`⏎` 跳到第一个，`n`／`N` 在结果之间跳**（票面第二条那五串、第三条）：
    /// `⏎` 之后暂停自动滚动、屏底报「搜索结果 第几个/共几个」；`n` 往下、`N` 往上，
    /// **收着的目录自动展开到那一卷**。
    ///
    /// 「海贼」只有一个结果：那个目录行——**目录名自己就装着这一句时它底下那十八卷
    /// 不再各算一个落点**（屏上那十八行照旧加下划线）。「第05」有七个，一卷一个。
    #[test]
    fn enter_jumps_to_the_first_match_and_n_cycles_through_them() {
        let scene = assert_sequence("running-search-Enter");
        assert!(!scene.session.views.task.follow, "跳过去之后不再跟");
        assert_eq!(scene.session.views.searching(), Some("海贼"));
        assert_sequence("running-search-05-Enter");
        assert_sequence("running-search-n");
        assert_sequence("running-search-n-N");
    }

    /// **搜索那一行上 `Esc` 连那一句一起丢**（票面第三条）：屏底换回按键提示、
    /// 框底边左起那一截没了、一条下划线都不剩。
    ///
    /// **`⏎` 之后再 `Esc` 丢的只有那一句**（`CONTEXT.md` 的《退出会话》：`Esc` 只退一级）：
    /// 光标停在刚跳到的那一行上不动，屏底那句「搜索结果 1/1」还在。
    ///
    /// 两串都从「搜索」那一景起手，因此都带着那一景那**两格已知的一格差**
    /// （停车场 **Q844**，理由与 `the_search_scene_…` 那一条逐字相同）。
    #[test]
    fn escape_drops_the_search_and_its_underlines() {
        const BAR: &[(usize, u16)] = &[(3, 46), (16, 103)];
        let scene = assert_sequence_like("search-Escape", BAR);
        assert_eq!(scene.session.views.searching(), None);
        let scene = assert_sequence_like("search-Enter-Escape", BAR);
        assert_eq!(scene.session.views.searching(), None);
        assert!(scene.session.views.input.is_none(), "那一行已经关了");
    }

    /// **搜进收着的目录里那一卷**（票面第二条那一串）：`棋魂/第15` 命中的是一卷，
    /// 而它那个目录收着——跳过去把目录展开、光标停在那一卷上。
    #[test]
    fn searching_into_a_collapsed_directory_expands_it() {
        let scene = assert_sequence("ended-search-into-collapsed");
        assert!(
            matches!(
                scene.session.views.task.cursor,
                super::super::view::Cursor::Volume(_)
            ),
            "停在那一卷上"
        );
    }

    /// **一个都没找到时说一句**（票面第三条）：屏底写「没有找到和「…」相关的卷或文件夹」，
    /// 光标一格不动；**那一句仍留着**（框底边左起照旧写着它）——`n`／`N` 跳的就是它。
    #[test]
    fn a_search_that_matches_nothing_says_so() {
        let scene = assert_sequence("ended-search-nothing");
        assert_eq!(scene.session.views.searching(), Some("不存在"));
    }

    /// **全部按键那一张掀在转换中那一副上**（07 号票留给本票的那六串）：`?` 与 `Esc`
    /// 各关得掉它（关掉之后底下原样回来，框右端那一枚 `[自动滚动]` 露出来），
    /// `j` 滚一行、`k` 滚回来、`G` 到底；底边说看到第几行。
    ///
    /// 宽那一屏 27 行一屏装得下（`j` 滚不动，底边仍写 `1–27 of 27`），
    /// 窄那一屏装不下、滚得动。
    #[test]
    fn the_key_sheet_over_the_running_tree_closes_and_scrolls() {
        for name in [
            "help-question",
            "help-Escape",
            "help-j",
            "help-narrow-j",
            "help-narrow-j-k",
            "help-narrow-G",
        ] {
            assert_sequence(name);
        }
    }

    /// **还在处理与还没轮到的卷停得住、展不开**：按下去不换屏，屏底说为什么
    /// （`CONTEXT.md` 的《停得住 / 展得开》）。
    #[test]
    fn a_volume_still_in_flight_or_still_queued_says_why_it_cannot_be_opened() {
        assert_sequence("running-vol-l");
        assert_sequence("running-queued-l");
    }

    /// **目录行 `l` 展开、`h` 收起**（票面第三条）：展开之后它那几卷缩进挂在底下，
    /// 收起之后行数回到原样；`h` 停在卷行上时收的是它那个目录，光标跟着停到目录行上。
    #[test]
    fn l_expands_a_directory_row_and_h_collapses_it_again() {
        let mut scene = Scene::named("running");
        let mut running = Running::holding(scene.live.take().expect("跑着的那一趟"));
        let now = scene.now();
        let window = Window {
            cols: 120,
            rows: 36,
        };
        // 这一条一个预设键都不按，灰阶测试图也不出（见 [`presets`]）。
        let space = tempfile::tempdir().expect("建得出临时目录");
        let nowhere = presets(&space);
        let press = |scene: &mut Scene, running: &mut Running, letter: char| {
            super::input(
                &mut scene.session,
                running,
                &nowhere,
                space.path(),
                now,
                window,
                Input::Key(Key::Char(letter)),
            );
        };
        // 光标停在展开着的那个目录里的一卷上：`h` 收起它，光标停到目录行上。
        let before = scene.session.lines().len();
        press(&mut scene, &mut running, 'h');
        let collapsed = scene.session.lines().len();
        assert!(collapsed < before, "收起之后行少了");
        assert!(matches!(
            scene.session.views.task.cursor,
            super::super::view::Cursor::Directory(_)
        ));
        // 再 `l` 展开回来，行数与一开始相同。
        press(&mut scene, &mut running, 'l');
        assert_eq!(scene.session.lines().len(), before, "展开回来行数照旧");
        press(&mut scene, &mut running, 'h');
        assert_eq!(scene.session.lines().len(), collapsed);
    }

    /// **`o` 打开输入行**（`session-redesign/07` 票面第二条）：屏底换成「添加路径  ~/▏」与右端那四件，
    /// 卷列表的框细了；`Esc` 丢掉这一步，屏底原样回来。
    #[test]
    fn o_opens_the_input_line_and_escape_closes_it() {
        assert_sequence("fresh-o");
        assert_sequence("fresh-o-Escape");
    }

    /// **`Tab` 列出这一层、再按轮到下一个、`C-w` 删一段**：`~/` 底下四项——轮换那两屏不比屏，
    /// 设计稿的候选按它假盘的写法次序摆、实现按名字排（停车场 Q790），比的是候选有几条、轮到哪一条、
    /// 缓冲跟着换；`C-w` 之后候选没了、缓冲回到 `~/`，那一屏逐格相等。
    #[test]
    fn tab_lists_the_level_cycles_through_it_and_ctrl_w_deletes_a_segment() {
        let (scene, _, _) = walked("fresh-o-Tab");
        let line = scene.session.views.input.as_ref().expect("输入行开着");
        assert_eq!(line.candidates.len(), 4, "`~/` 底下四项");
        assert!(line.candidates.iter().all(|listed| listed.directory));
        assert_eq!(line.at, 0);
        assert_eq!(line.buffer, format!("~/{}", line.candidates[0].shown()));
        let (scene, _, _) = walked("fresh-o-Tab-Tab");
        let line = scene.session.views.input.as_ref().expect("输入行开着");
        assert_eq!(line.at, 1);
        assert_eq!(line.buffer, format!("~/{}", line.candidates[1].shown()));
        let scene = assert_sequence("fresh-o-Tab-Tab-C-w");
        let line = scene.session.views.input.as_ref().expect("输入行开着");
        assert_eq!((line.buffer.as_str(), line.candidates.len()), ("~/", 0));
    }

    /// **打一个找不到的路径**：`⏎` 之后输入行关了、屏底说「找不到」、列表一条没多；
    /// **打一个找得到的**：添上、勾着、光标停到它上面、屏底说「已添加」。
    #[test]
    fn enter_says_when_the_path_is_missing_and_adds_it_when_it_is_there() {
        let scene = assert_sequence("fresh-o-missing");
        assert_eq!(scene.session.scope.paths.len(), 13);
        let scene = assert_sequence("fresh-o-added");
        assert_eq!(scene.session.scope.paths.len(), 14);
        assert_eq!(
            scene.session.scope.paths[13].path,
            scene.path("~/Comics/火之鸟")
        );
    }

    /// **`i` 修改一条处理路径、输出目录那一行上改输出目录**：缓冲先摆着那一条；
    /// `C-w` 删掉末一段、打上新的、`⏎` 定下，总览与卷列表都换了。
    #[test]
    fn i_edits_the_path_or_the_output_directory_under_the_cursor() {
        assert_sequence("fresh-i");
        assert_sequence("fresh-k-i");
        let scene = assert_sequence("fresh-k-i-C-w-typed");
        assert_eq!(scene.session.scope.out, Some(scene.path("~/Comics")));
    }

    /// **打字时 `F1` 掀开全部按键**（票面第三条）：底下整屏压暗、输入行让给覆盖层自己的两件；
    /// `j`／`k` 滚（宽时装得下、一动不动；窄时滚一行）；`Esc` 关掉之后输入行、缓冲与补全框原样回来。
    #[test]
    fn f1_while_typing_lifts_the_key_sheet_and_escape_brings_the_input_line_back() {
        assert_sequence("add-F1");
        assert_sequence("add-F1-j");
        assert_sequence("add-F1-j-k-Escape");
        assert_sequence("add-narrow-F1-j");
        assert_sequence("add-narrow-F1-j-k");
        let scene = assert_sequence("add-narrow-F1-j-k-Escape");
        let line = scene.session.views.input.as_ref().expect("输入行回来了");
        assert_eq!(
            (line.buffer.as_str(), line.candidates.len()),
            ("~/Comics/", 4)
        );
    }

    /// **还没开始时 `?` 掀开全部按键**：只列这一档派得出的键、两栏。
    #[test]
    fn question_mark_before_the_run_lifts_the_key_sheet() {
        assert_sequence("fresh-help");
    }

    // ───────────────────────── 配置视图（`session-redesign/13`）─────────────────────────

    /// **两栏之间来回**（票面第二条）：`h`（详情栏上）、`⇥`、`Esc` 三条路都回到设置栏，
    /// 三屏因此相等——回来那一下**一格不改**。
    #[test]
    fn three_ways_lead_back_to_the_settings_pane_and_change_not_one_cell() {
        for name in ["config-h", "config-Tab", "config-Escape"] {
            let scene = assert_sequence(name);
            assert_eq!(scene.session.views.config.focus(), Focus::Settings);
            assert_eq!(scene.session.taste.fit, Some(FitMode::Inside), "一格没改");
        }
    }

    /// **设置栏 `l` 进详情栏，光标停在此刻生效的那一格上**（票面第二条）：
    /// `l` → `h` 回来一格不改，`l` → `k` → `l` 定下上一格。
    #[test]
    fn l_opens_the_details_pane_and_only_the_second_l_settles_a_choice() {
        let scene = assert_sequence("config-h-l");
        assert_eq!(scene.session.views.config.focus(), Focus::Details);
        assert_eq!(scene.session.views.config.choice, 2, "停在生效的那一格上");

        let scene = assert_sequence("config-h-l-k-h");
        assert_eq!(scene.session.taste.fit, Some(FitMode::Inside), "一格没改");

        let scene = assert_sequence("config-h-l-k-l");
        assert_eq!(scene.session.taste.fit, Some(FitMode::Height));
        assert_eq!(
            scene.session.views.config.focus(),
            Focus::Settings,
            "定完回来"
        );
    }

    /// **型号那一项两层下钻**（票面第二条）：第一层是屏幕规格，`l` 进去才是型号，
    /// `h` 退回屏幕规格那一层；挑了一个型号，先前填的可见灰阶数与画质门槛一并清空。
    #[test]
    fn the_model_drills_through_the_panels_and_picking_one_clears_the_numbers() {
        assert_sequence("config-model");
        let scene = assert_sequence("config-model-drill");
        assert!(scene.session.views.config.drill.is_some());

        let scene = assert_sequence("config-model-drill-h");
        assert_eq!(scene.session.views.config.drill, None, "退回屏幕规格那一层");
        assert_eq!(
            scene.session.device.profile.as_deref(),
            Some("kobo-libra-2"),
            "退一步一格不改"
        );

        let scene = assert_sequence("config-model-drill-j-l");
        assert_eq!(scene.session.device.profile.as_deref(), Some("boox-leaf2"));
        assert_eq!(scene.session.device.gray_levels, None);
        assert_eq!(scene.session.device.threshold, None);
    }

    /// **自由填的那一项 `i` 经输入行改**（票面第二条）：屏底换成「可见灰阶数  ▏」，
    /// 打完 `⏎` 收下，设置栏那一行跟着变、行尾多一个 `*`。
    #[test]
    fn i_edits_a_filled_in_setting_through_the_input_line() {
        let scene = assert_sequence("config-levels-i");
        let line = scene.session.views.input.as_ref().expect("输入行开着");
        assert_eq!(line.purpose.prompt(), "可见灰阶数  ");

        let scene = assert_sequence("config-levels-i-typed");
        assert_eq!(scene.session.device.gray_levels, Some(14));
        assert!(scene.session.views.input.is_none(), "收下之后输入行关掉");
    }

    /// **不到 90 列退成单栏**（票面第二条）：`l` 进详情，`h` 回来，两栏从不同时在场。
    #[test]
    fn under_ninety_columns_the_two_panes_take_turns() {
        assert_sequence("config-narrow-h");
        assert_sequence("config-narrow-h-l");
        assert_sequence("config-narrow-h-l-h");
    }

    /// **跑着时 `2` 进得去、看得见、定不下**（票面第二条与第四条）：顶栏右端带着这一趟的进度，
    /// 设置栏抬头写 `[已锁定]`，详情栏照样进得去——定的那一下屏底说设置已锁定，一格没改。
    #[test]
    fn during_a_run_the_config_view_is_readable_and_settles_nothing() {
        assert_sequence("running-2");
        assert_sequence("running-2-fit-l");
        let scene = assert_sequence("running-2-fit-l-l");
        assert_eq!(
            scene.session.taste.fit,
            Some(FitMode::Inside),
            "定那一下一格没改"
        );
        assert_eq!(
            scene.session.views.config.focus(),
            Focus::Details,
            "拦下了就留在原处"
        );
    }

    // ───────────────────────── 预设栏（`session-redesign/14`）─────────────────────────

    /// **`p` 掀开预设栏**（票面第一条）：它**替换详情栏**、设置栏仍在屏上（框细了、
    /// 光标行首换成暗的 `›`），列的是进这一栏那一刻盘上有的那两份、每份说了哪几项、
    /// 哪一份在用，末行是「把当前设置保存为预设」。再按一次 `p`、或者 `h`，都回设置栏。
    #[test]
    fn p_lifts_the_picker_over_the_details_pane_and_p_or_h_puts_it_back() {
        let scene = assert_sequence("config-p");
        assert_eq!(scene.session.views.config.focus(), Focus::Picker);
        assert_eq!(scene.session.views.config.pane, Pane::Details);
        let listed: Vec<&str> = scene
            .session
            .views
            .config
            .listed
            .iter()
            .map(|one| one.name.as_str())
            .collect();
        assert_eq!(listed, ["漫画", "画集"], "列的是盘上那两份");
        for name in ["config-p-p", "config-p-h"] {
            let scene = assert_sequence(name);
            assert!(!scene.session.views.config.picker, "「{name}」收起来了");
            assert_eq!(scene.session.views.config.focus(), Focus::Settings);
        }
    }

    /// **套用一份**（票面第一条）：`j` 挪到「画集」、`⏎` 套下来——设备设置与处理选项
    /// **整个换成它**（它没说的那几项回到「没说」），路径与输出一格不动，
    /// 顶上那一条换成它、设置栏上一个 `*` 都不剩。
    #[test]
    fn enter_uses_the_preset_under_the_cursor_and_replaces_both_bands() {
        let scene = assert_sequence("config-p-j-Enter");
        let session = &scene.session;
        assert_eq!(
            session
                .views
                .config
                .applied
                .as_ref()
                .map(|one| one.name.as_str()),
            Some("画集")
        );
        assert_eq!(session.taste.fit, Some(FitMode::Inside));
        assert_eq!(session.taste.crop, Some(false));
        assert_eq!(session.taste.dither, None, "它没说的回到「没说」");
        assert_eq!(session.changed_from_preset(), 0, "与预设一致");
        assert_eq!(
            session.device.profile.as_deref(),
            Some("kobo-libra-2"),
            "型号一格不动"
        );
        // **路径与输出一格不动**：与没按过那几下的同一景比。两份夹具各有各的临时目录，
        // 因此比的是**屏上那几条**（家目录缩写成 `~` 之后），不是绝对路径。
        let untouched = Scene::named("config");
        let listed = |scene: &Scene| -> Vec<(String, bool)> {
            scene
                .session
                .scope
                .paths
                .iter()
                .map(|named| (scene.session.home_shown(&named.path), named.on))
                .collect()
        };
        assert_eq!(listed(&scene), listed(&untouched), "路径一格不动");
        assert_eq!(
            session.output_shown(),
            untouched.session.output_shown(),
            "输出目录一格不动"
        );
    }

    /// **`dd` 按两下删一份**（票面第一条与第二条）：第一下只在那一栏里问一句、
    /// **盘一个字节都不动**；第二下才删——正在用的那一份也删得掉，删完顶上那一条是
    /// 「（未使用预设）」。**文件里别的预设原样留着**。
    #[test]
    fn dd_twice_erases_the_preset_and_the_other_one_stays_as_it_was() {
        let scene = assert_sequence("config-p-dd");
        assert_eq!(
            scene.session.views.config.armed_delete.as_deref(),
            Some("漫画")
        );
        assert_eq!(
            scene.presets.names().expect("读得出名字"),
            ["漫画", "画集"],
            "第一下盘一个字节都没动"
        );
        let untouched = scene.presets.read("画集").expect("另一份读得出");

        let scene = assert_sequence("config-p-dd-dd");
        assert_eq!(scene.presets.names().expect("读得出名字"), ["画集"]);
        assert_eq!(
            scene.presets.read("画集").expect("另一份原样留着"),
            untouched
        );
        assert!(
            scene.session.views.config.applied.is_none(),
            "正在用的也删得掉"
        );
        assert_eq!(scene.session.views.config.armed_delete, None);
    }

    /// **保存并起名**（票面第一条与第二条）：`G` 停到末行、`⏎` 开输入行
    /// （提示词是「保存为预设，名称  」），打完 `⏎` 存进**临时目录那份预设文件**——
    /// 文件里原来那两份原样留着，存下的那一份当场成了套着的那一份、**不记型号**。
    #[test]
    fn saving_a_named_preset_writes_it_into_the_preset_file_on_disk() {
        let scene = assert_sequence("config-p-save");
        let line = scene.session.views.input.as_ref().expect("输入行开着");
        assert_eq!(line.purpose.prompt(), "保存为预设，名称  ");
        assert_eq!(
            scene.presets.names().expect("读得出名字"),
            ["漫画", "画集"],
            "还没打名字，盘一个字节都没动"
        );
        let before = scene.presets.read("画集").expect("另一份读得出");

        let scene = assert_sequence("config-p-save-named");
        let mut names = scene.presets.names().expect("读得出名字");
        names.sort();
        assert_eq!(names, ["插图", "漫画", "画集"]);
        assert_eq!(scene.presets.read("画集").expect("原样留着"), before);
        let stored = scene.presets.read("插图").expect("存下来了");
        assert_eq!(stored, scene.session.preset_to_store());
        assert_eq!(stored.device.profile, None, "存出去的那一份不记型号");
        assert_eq!(
            scene
                .session
                .views
                .config
                .applied
                .as_ref()
                .map(|one| one.name.as_str()),
            Some("插图")
        );
        assert!(scene.session.views.input.is_none(), "收下之后输入行关掉");
    }

    /// **同名覆盖要按两下**（票面第一条）：第一下**在预设栏里**问一句、盘一个字节都不动、
    /// 输入行留着，第二下才盖掉；盖掉的只有那一份，文件里另一份原样留着。
    ///
    /// 设计稿没有这一串（`submitInput` 那一支直接 push），因此**没有一份期望屏可对**；
    /// 这一条比的是盘上那份文件、输入行还在不在，以及**屏上真画出了那一问**
    /// ——屏底这一刻让给了输入行，那一句只有画出来才算说了（停车场 Q894）。
    #[test]
    fn an_existing_name_takes_two_presses_before_it_overwrites() {
        let mut scene = Scene::named("config");
        let mut running = Running::default();
        let now = scene.now();
        let here = charts_land_in(&scene);
        let window = Window {
            cols: 120,
            rows: 36,
        };
        let before = scene.presets.read("画集").expect("另一份读得出");
        let tap = |scene: &mut Scene, running: &mut Running, key: Key| {
            super::input(
                &mut scene.session,
                running,
                &scene.presets,
                &here,
                now,
                window,
                Input::Key(key),
            );
        };
        // 掀开预设栏、停到末行、开输入行，打上一个**已经有了**的名字。
        for key in [Key::Char('p'), Key::Char('G'), Key::Enter] {
            tap(&mut scene, &mut running, key);
        }
        for character in "漫画".chars() {
            tap(&mut scene, &mut running, Key::Char(character));
        }
        tap(&mut scene, &mut running, Key::Enter);
        assert!(scene.session.views.input.is_some(), "第一下输入行留着");
        assert_eq!(
            scene.presets.read("漫画").expect("读得出"),
            crate::preset::Preset::default(),
            "第一下盘一个字节都没动"
        );
        // **那一问真在屏上**：预设栏里、说明底下那一行——屏底这一刻让给了输入行，
        // 说给屏底等于一个字都没说（停车场 Q894）。比的是**去掉空白之后**屏上有没有这几个字
        // （宽字符占住的第二格画布清成空格，与 `draw::probe::tight` 同一条读法）。
        let screen: String = painted(&scene, &running, (120, 36))
            .content()
            .iter()
            .flat_map(|cell| cell.symbol().chars())
            .filter(|glyph| !glyph.is_whitespace())
            .collect();
        assert!(
            screen.contains("再按一次⏎覆盖「漫画」："),
            "屏上没画出那一问"
        );
        assert!(
            !screen.contains("再按一次dd删除"),
            "那一问不该串成删除那一句"
        );
        // 第二下才盖。
        tap(&mut scene, &mut running, Key::Enter);
        assert!(scene.session.views.input.is_none(), "收下之后输入行关掉");
        assert_eq!(
            scene.presets.read("漫画").expect("读得出"),
            scene.session.preset_to_store(),
            "第二下才盖掉"
        );
        assert_eq!(scene.presets.read("画集").expect("原样留着"), before);
        assert_eq!(
            scene.presets.names().expect("读得出名字"),
            ["漫画", "画集"],
            "覆盖不添第三份"
        );
    }

    /// **`c` 出灰阶测试图**（票面第一条）：图按此刻那块面板画出来、写到盘上，
    /// 屏底那一句**说它写到了哪里**。
    ///
    /// **屏底那一行整个换掉**（[`Expected::instead`]，停车场 **Q890**）：设计稿那一句
    /// 末尾写的是「（原型不写文件）」——原型不写，而这一副真写得出文件，票面第五条要的
    /// 正是回话说写到了哪里。换掉之后仍是一条断言：那一行连同每一格的样子由这条用例写出来，
    /// 实现说别的照样红。屏上别的 35 行一格不差。
    #[test]
    fn c_draws_a_calibration_chart_and_says_where_it_landed() {
        let (scene, running, exit) = walked("config-c");
        assert_eq!(exit, Exit::Stay, "走完会话还开着");
        // 图真落在了临时目录里（一个用户的东西都不碰）。
        let landed: Vec<PathBuf> = std::fs::read_dir(charts_land_in(&scene))
            .expect("临时目录读得出")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|kind| kind == "png"))
            .collect();
        assert_eq!(landed.len(), 1, "只写出一张：{landed:?}");
        let said = [
            Segment::plain(" "),
            Segment::new("✓ 已生成灰阶测试图", Look::kind(Kind::Done).bold()),
            Segment::plain(format!(
                "（1264x1680）：写到 {}",
                scene.session.home_shown(&landed[0])
            )),
        ];
        let buffer = painted(&scene, &running, scene::sequence("config-c").size);
        assert_no_background(&buffer);
        assert_same_cells(&buffer, &design::sequence("config-c").instead(35, &said));
    }

    /// **跑着时 `dd` 照样删得掉盘上那一份**（照设计稿 `deleteHere`：那一支没有只读那一问）。
    ///
    /// 只读的是**三组设置**（ADR 0017 决定第 3 条），而预设文件不是设置：删掉一份
    /// 不改这一趟的任何一项，正在用的那一份删掉也只是「未使用预设」。
    /// 它与 spec《配置视图》那句「跑着与等待确认时**整个视图只读**」读起来有张力，
    /// 收法归拍板的人——停车场 **Q895**。这一条把眼下是哪一副钉下来。
    #[test]
    fn during_a_run_dd_still_erases_a_preset_from_the_file() {
        let mut scene = Scene::named("running");
        let mut running = match scene.live.take() {
            Some(live) => Running::holding(live),
            None => Running::default(),
        };
        let now = scene.now();
        let here = charts_land_in(&scene);
        let window = Window {
            cols: 120,
            rows: 36,
        };
        let tap = |scene: &mut Scene, running: &mut Running, key: Key| {
            super::input(
                &mut scene.session,
                running,
                &scene.presets,
                &here,
                now,
                window,
                Input::Key(key),
            );
        };
        for key in [Key::Char('2'), Key::Char('p')] {
            tap(&mut scene, &mut running, key);
        }
        assert_eq!(scene.session.views.config.focus(), Focus::Picker);
        // 头两下只问一句，盘一个字节都不动。
        tap(&mut scene, &mut running, Key::Char('d'));
        tap(&mut scene, &mut running, Key::Char('d'));
        assert_eq!(
            scene.presets.names().expect("读得出名字"),
            ["漫画", "画集"],
            "第一下盘一个字节都没动"
        );
        tap(&mut scene, &mut running, Key::Char('d'));
        tap(&mut scene, &mut running, Key::Char('d'));
        assert_eq!(scene.presets.names().expect("读得出名字"), ["画集"]);
    }

    /// **跑着时预设栏进得去、套用定不下**（票面末一条）：`2` 进配置视图、`p` 掀开那一栏
    /// （顶上一条写着设置暂时锁定、设置栏抬头写 `[已锁定]`），`⏎` 那一下屏底说设置已锁定，
    /// 两组一格不改——拦它的是**阶段那一维**（ADR 0017）。
    #[test]
    fn during_a_run_the_picker_opens_but_uses_no_preset() {
        let scene = assert_sequence("running-2-p-Enter");
        assert_eq!(scene.session.views.config.focus(), Focus::Picker);
        assert_eq!(scene.session.taste.fit, Some(FitMode::Inside), "一格没改");
        assert_eq!(
            scene
                .session
                .views
                .config
                .applied
                .as_ref()
                .map(|one| one.name.as_str()),
            Some("漫画"),
            "套着的那一份没换"
        );
    }

    /// 终端那一侧的键码翻成新会话的输入：Ctrl 加一个字母另认，`C-c` 仍是那个中断键，别的照旧。
    #[test]
    fn control_letters_translate_to_ctrl_inputs_and_ctrl_c_stays_the_interrupt() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let ctrl = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(super::translate_input(&ctrl('d')), Some(Input::Ctrl('d')));
        assert_eq!(super::translate_input(&ctrl('w')), Some(Input::Ctrl('w')));
        assert_eq!(
            super::translate_input(&ctrl('c')),
            Some(Input::Key(Key::Interrupt))
        );
        assert_eq!(
            super::translate_input(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
            Some(Input::Key(Key::Char('j')))
        );
        assert_eq!(
            super::translate_input(&KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
            None
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::session::state::Listing;
    // 两个兄弟模块的**名字**（`super::*` 带进来的是它们里面的东西，不是模块本身）：
    // 用例要按名字点它们里面的取值与夹具。
    use crate::session::live::Volume;
    use crate::session::{live, state};

    /// 一份**指向临时目录**的预设文件。
    ///
    /// 用例一律用它：真会话读写的是用户配置目录下那一份，而那是用户自己的东西——
    /// 用例不该读它，更不该写它。位置由 [`Presets::at`] 点名，因此不必去改进程的环境变量
    /// （`tests/preset.rs` 说过为什么不改）。
    fn presets(space: &tempfile::TempDir) -> Presets {
        Presets::at(space.path().join("tonefit").join("presets.toml"))
    }

    /// 按一个**不出灰阶测试图**的键。
    ///
    /// [`press`] 收的那个「图落在哪个目录下」只有 `c` 那一个键用得到，而这几条用例
    /// 一个都不按它。去处仍旧点在**临时目录**里（那份预设文件的上一层，见 [`presets`]）：
    /// 万一往后有人往这几条里加一下 `c`，写出去的东西也落在那儿，
    /// 不会掉进跑用例的那个目录——相对路径会。
    ///
    /// 出灰阶测试图那两条不走这里：它们要说的正是「写到哪儿了、写不出去时怎么办」，
    /// 因此自己直接调 [`press`]，把去处摆在明面上。
    fn tap(session: &mut Session, running: &mut Running, presets: &Presets, key: Key) -> Exit {
        let file = presets.path().expect("用例里那份预设文件的位置是定死的");
        press(
            session,
            running,
            presets,
            file.parent().unwrap_or(file),
            key,
        )
    }

    /// 「这里没有终端」那条错误里，**clap 那条必填项提示一个字都没被吃掉**。
    #[test]
    fn the_no_terminal_error_keeps_what_clap_has_to_say() {
        let message = no_terminal_error().to_string();

        assert!(message.contains("这里没有终端"), "{message}");
        // clap 那一半：缺的三项与用法行都在。
        assert!(message.contains("--out"), "{message}");
        assert!(message.contains("--profile"), "{message}");
        assert!(
            message.contains("Usage") || message.contains("用法"),
            "{message}"
        );
    }

    /// **按停止那个键真的走到了跑着的那一趟身上。**
    ///
    /// 两头各自有用例（状态机那边 `one_key_pressed_twice_is_the_two_stage_stop`、
    /// 闩那边 `the_latch_only_ever_goes_up`），接头处只有这一条——而接头处正是本层
    /// 唯一做的事：把状态机升到的那一级交给 [`Running::stop`]。
    ///
    /// 不开终端：[`press`] 收的是 `&mut Session` 与 `&mut Running`，一个终端都不碰
    /// （碰终端的是 [`drive`] 那条循环）。
    #[test]
    fn pressing_stop_reaches_the_run_that_is_going() {
        let mut session = Session::new();
        let mut running = Running::default();
        let space = tempfile::tempdir().expect("建得出临时目录");
        // 这一条一个预设键都不按：那一份摆在临时目录下，一个字节都不会被读到。
        let nowhere = presets(&space);
        session.run_started();

        // 一次：做完再停。两头记的是同一个字。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(session.stopping(), tonefit::Instruction::Finish);
        assert_eq!(running.pressed(), tonefit::Instruction::Finish);

        // 再一次：立即停止。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(session.stopping(), tonefit::Instruction::Abort);
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 第三次起那个键没有意义，闩两头都不再动。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 浏览时按它什么都不发生：还没有东西可停。
        let mut idle = Session::new();
        let mut nothing = Running::default();
        let nowhere = presets(&space);
        assert_eq!(
            tap(&mut idle, &mut nothing, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(nothing.pressed(), tonefit::Instruction::Continue);
    }

    /// **答话那个键真的走到了停在确认点上的那条线程身上**（`p1-session/14`）。
    ///
    /// 这一条走的是整条路：按 `t` 起一趟（预览，因此 [`resuming`] 把它改成接着写出的那一趟）→
    /// 那条线程停在确认点上 → 会话跟着换一副样子 → 按 `s` 答做完再停 → 那条线程接着跑完。
    /// 两头各自有用例（状态机那边 `deciding_action`，闸那边
    /// `answering_finish_at_the_decision_point_writes_nothing_and_still_reports_the_volume`），
    /// **接头处只有这一条**——而接头处正是本层唯一做的事。
    ///
    /// **等待确认时会话不冻屏**由它的形状说出来：那条线程停在闸上，而这一头照旧收键、
    /// 照旧问得动 [`Session::mode`]。等的那一步走的是「转到条件成立为止」，
    /// 不是 sleep 撞运气（见 `Running::deciding`）。
    ///
    /// 不开终端：[`press`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn answering_at_the_decision_point_reaches_the_thread_waiting_there() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        // 一页加一个透传文件（见 [`super::live::fixture::a_real_volume`]）：页非有不可，
        // 一页都没有的东西不是卷，那条线程根本走不到确认点。
        let volume = crate::session::live::fixture::a_real_volume(space.path(), "卷一");
        let out = space.path().join("出");

        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(out.clone());
        session.scope.paths.push(state::NamedPath {
            path: volume,
            on: true,
        });
        let mut running = Running::default();
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&space);

        // 按 `t`：预览，因此这一趟改走 `Mode::Process` 并在确认点上等人。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('t')),
            Exit::Stay
        );
        assert!(matches!(session.stage(), state::Stage::Running(_)));

        // 那条线程走到确认点上停住；会话每帧问一次，跟着换一副样子（见 [`drive`]）。
        while !running.deciding() {
            std::thread::yield_now();
        }
        session.at_the_decision_point(running.deciding());
        assert!(session.deciding(), "那一趟停住了，会话却没跟着换一副样子");
        assert!(
            running.live().expect("跑过一趟").summarized().is_some(),
            "确认点上没有报告可画"
        );

        // 等待确认时会话仍旧收键：按一个没有意义的键，它照旧原地不动、不退出。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('e')),
            Exit::Stay
        );
        assert!(session.deciding(), "按了一个没有意义的键就走掉了");

        // 按 `s` 答做完再停：这一卷一个字节都不写，那条线程接着跑完。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert!(!session.deciding(), "答完话会话还停在确认点上");
        while !running.reap() {
            std::thread::yield_now();
        }
        session.run_finished();

        assert_eq!(
            session.focus(),
            &state::Focus::Config,
            "结束之后配置还改不动"
        );
        assert!(!out.exists(), "答了做完再停，输出目录却被建了出来");
        let live = running.live().expect("跑过一趟");
        assert_eq!(
            live.report().volumes.len(),
            1,
            "答做完再停把报告也一起停掉了"
        );
        assert_eq!(
            live.decided(),
            Some(tonefit::Instruction::Finish),
            "答的那个字没记下来"
        );
    }

    /// **预览一律接着写出，点名了几个路径都一样**（ADR 0012 决定第 3、5 条，
    /// `volume-discovery/07` 票面）。
    ///
    /// 各情形问的是同一个函数交出来的那两样：这一趟**真走**哪一种模式、
    /// 它**在确认点上等不等人**。
    ///
    /// 预览那几支两样都变：模式从 `DryRun` 换成 `Process`（参照要留着，
    /// 答继续时分析环节才不必重算），并且等人。**这一处不再数点名了几个路径**——
    /// 从前数的是 `inputs.len() == 1`，而发现落地之后一个路径常常就是几十卷
    /// （`volume-discovery/03`），那个数早就不说明有几卷了。
    ///
    /// 执行那几支一格不动：按 `x` 的时候用户已经拿过主意了。
    #[test]
    fn every_trial_resumes_however_many_paths_it_names() {
        let one = |mode| Request {
            inputs: vec![PathBuf::from("库/卷一")],
            ..live::fixture::request(mode)
        };
        let many = |mode| Request {
            inputs: vec![PathBuf::from("库/卷一"), PathBuf::from("库/卷二")],
            ..live::fixture::request(mode)
        };

        // 预览：改走 Process，并且在确认点上等人——点名一个路径与点名两个一个待遇。
        for request in [one(RunMode::DryRun), many(RunMode::DryRun)] {
            let inputs = request.inputs.len();
            let (request, resumes) = resuming(request);
            assert_eq!(
                resumes,
                Resuming::Waits,
                "预览没接着写出（{inputs} 个路径）"
            );
            assert_eq!(
                request.mode,
                RunMode::Process,
                "接着写出那一趟得留参照（ADR 0012 决定第 5 条）"
            );
        }

        // 执行：一格不动，几个路径都一样——按 x 的时候用户已经拿过主意了。
        for request in [one(RunMode::Process), many(RunMode::Process)] {
            let inputs = request.inputs.len();
            let (request, resumes) = resuming(request);
            assert_eq!(
                resumes,
                Resuming::GoesOn,
                "转换那一趟停下来等人了（{inputs} 个路径）"
            );
            assert_eq!(request.mode, RunMode::Process);
        }

        // 一个卷都没勾：范围为空由库那一侧当场拒掉——那一趟一条事件都不发，
        // 确认点根本到不了，等不等人因此不影响任何事（见 `Running::start` 起的那道闸）。
        let (request, resumes) = resuming(Request {
            inputs: Vec::new(),
            ..live::fixture::request(RunMode::DryRun)
        });
        assert_eq!(resumes, Resuming::Waits);
        assert_eq!(request.mode, RunMode::Process);
    }

    /// **「后面的卷都写出」那个键真的走到了停在确认点上的那条线程身上**
    /// （`volume-discovery/07`，spec 的 story 13）。
    ///
    /// 与答话那一条同一个位置：接头处是本层唯一做的事——把状态机认出来的那个字
    /// **连同它管几卷**交给 [`Running::decide`]。两头各自有用例（状态机那边
    /// `which_keys_do_what_in_which_state`，闸那边
    /// `answering_for_the_rest_once_stops_the_asking_and_leaves_the_latch_alone`）。
    ///
    /// 走的是整条路：按 `t` 起一趟**两个卷**的预览 → 停在头一卷的确认点上 →
    /// 按 `a` → 那条线程一路把两卷都做完，一次都不再停。
    #[test]
    fn pressing_the_rest_too_reaches_the_thread_waiting_at_the_decision_point() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let out = space.path().join("出");

        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(out.clone());
        for name in ["卷一", "卷二"] {
            session.scope.paths.push(state::NamedPath {
                path: crate::session::live::fixture::a_real_volume(space.path(), name),
                on: true,
            });
        }
        let mut running = Running::default();
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&space);

        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('t')),
            Exit::Stay
        );
        while !running.deciding() {
            std::thread::yield_now();
        }
        session.at_the_decision_point(running.deciding());
        assert!(session.deciding(), "那一趟停住了，会话却没跟着换一副样子");

        // 按 `a`：这一卷接着做，后面的卷都写出。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('a')),
            Exit::Stay
        );
        assert!(!session.deciding(), "答完话会话还停在确认点上");
        while !running.reap() {
            // 真停下来的话当场红，而不是挂在那儿等一个不会来的人。
            assert!(
                !running.deciding(),
                "答过「后面的卷都写出」，它却又停下来问了"
            );
            session.at_the_decision_point(running.deciding());
            std::thread::yield_now();
        }
        session.run_finished();

        assert!(out.join("卷一").is_dir(), "头一卷没写出来");
        assert!(out.join("卷二").is_dir(), "剩下的那一卷没写出来");
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.for_the_rest(), Some(tonefit::Instruction::Continue));
        assert_eq!(live.report().volumes.len(), 2);
    }

    /// **一趟都没跑过时展开那个键根本不派动作，跑过之后它找那一趟要报告。**
    ///
    /// 前一半是停车场 Q167 收的那一笔：从前它派得出动作，而这一层挡在前面说一句
    /// 「还没跑过」——`?` 那张表因此列着它，按下去只换来一句话，
    /// 与「按得动」在屏上长得一模一样。眼下按键表在那个阶段上就不派它
    /// （`super::state::Session::browsing_action`），屏上因此一处都不摆。
    ///
    /// 后一半的接头处与按停止那一条同一个位置：状态机读不到那一趟攒下来的东西，
    /// 「有几卷」「那一卷落在第几行」两个数都由本层从 [`Running::live`] 上数出来。
    /// 不开终端——[`press`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn expanding_asks_the_run_for_its_report_and_says_so_when_there_is_none() {
        let mut session = Session::new();
        let mut running = Running::default();
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&workspace);

        // 一趟都没跑过：那个键一个动作都不派，会话原地不动、一句话都不说
        //（停车场 Q167：屏上不摆按不动的键，而「按了有话说」与「按得动」长得一样）。
        assert_eq!(session.action(Key::Char('e')), Action::Ignored);
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('e')),
            Exit::Stay
        );
        assert!(session.expansion().is_none(), "没有报告却展开了");
        assert_eq!(session.notice(), None, "按不动的键还说了一句");

        // 跑过一趟、报告里有两卷：展开落在**光标停着的那一卷**上，光标停在它的头一页。
        // 两个真跑得动的卷（见 [`live::fixture::a_real_volume`]）：这一条要问的
        // （哪一卷、落在第几行、转不转得回去）一件都不少。
        let inputs: Vec<PathBuf> = ["卷一", "卷二"]
            .iter()
            .map(|name| live::fixture::a_real_volume(workspace.path(), name))
            .collect();
        running.start(
            tonefit::Request {
                inputs,
                output_root: workspace.path().join("出"),
                ..live::fixture::request(tonefit::Mode::DryRun)
            },
            // 两卷，因此不接着写出：这一条问的是展开，与确认点无关（见 [`resuming`]）。
            Resuming::GoesOn,
        );
        // 状态机那一头也跟着走一步——真会话里 [`press`] 起完线程就调它
        // （两处记的是同一趟）。展开那个键**要等这一趟结束**才派得出动作，
        // 而「结束了」是从「跑着」回来的（见 [`Session::run_finished`]）。
        session.run_started();
        while !running.reap() {
            std::thread::yield_now();
        }
        session.run_finished();
        tap(&mut session, &mut running, &nowhere, Key::Char('e'));
        let expansion = session.expansion().cloned().expect("该展开了");
        // **展开的是光标停着的那一卷**（`p3-session-legibility/10`）：自动滚动着的时候
        // 那是**最新收摊的那一卷**，也就是第二卷。从前它恒是第一卷——那时报告区还没有光标。
        assert_eq!(expansion.volume, Volume::Settled(1));
        assert_eq!(expansion.at, 0, "没落在那一卷的头一页上");
        assert_eq!(
            expansion.listing,
            Listing::Notable,
            "展开那一下该只列需留意的页"
        );
        assert!(session.notice().is_none(), "展开之后上一句话没抹掉");

        // `⇥` 往后一卷，两头都转一圈：第二卷之后回到第一卷。
        tap(&mut session, &mut running, &nowhere, Key::Tab);
        let first = session.expansion().cloned().expect("还展开着");
        assert_eq!(first.volume, Volume::Settled(0), "⇥ 没转到第一卷上");
        assert_eq!(first.at, 0, "换一卷之后光标没回到头一页");
        tap(&mut session, &mut running, &nowhere, Key::Tab);
        assert_eq!(
            session.expansion().expect("还展开着").volume,
            Volume::Settled(1),
            "没转回去"
        );

        // `⇧⇥` 是另一头：往前一卷，同样转得回去。**两头都有**，
        // 因为几十卷的一趟里往回看一卷不该按二十九下（票面：选中一卷）。
        tap(&mut session, &mut running, &nowhere, Key::BackTab);
        let back = session.expansion().cloned().expect("还展开着");
        assert_eq!(back.volume, Volume::Settled(0), "⇧⇥ 没往前转");
        assert_eq!(back.at, first.at, "两头转到同一卷，落位却不一样");
        tap(&mut session, &mut running, &nowhere, Key::BackTab);
        assert_eq!(
            session.expansion().expect("还展开着").volume,
            Volume::Settled(1)
        );

        // **换一卷时列的是哪几页跟着走**：`a` 按下去不该只管一卷（票面第二条）。
        tap(&mut session, &mut running, &nowhere, Key::Char('a'));
        assert_eq!(session.expansion().expect("还展开着").listing, Listing::All);
        tap(&mut session, &mut running, &nowhere, Key::Tab);
        assert_eq!(
            session.expansion().expect("还展开着").listing,
            Listing::All,
            "换一卷把「列全部页」扳回去了"
        );

        // 收起：一个键回到报告区，展开态没了。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Esc),
            Exit::Stay
        );
        assert!(session.expansion().is_none());
    }

    /// **光标停得上没做成的那一卷，而在它上面按展开时明说这一卷展不开**
    /// （`p4-parking-lot/10`，收停车场 Q159）。
    ///
    /// 两半各钉一句：
    ///
    /// - **停得上**——`↑↓` 走的那一列（[`Live::volumes`]）此刻收着它
    ///   （[`Volume::Failed`]），光标因此落得上那一行；
    /// - **按下去有话说**——不进展开态、也不悄悄什么都不做，屏上当场多一句
    ///   [`CANNOT_EXPAND`]。这与型号没挑时按 `t`／`x` 是同一条待遇。
    ///
    /// **`⇥` 不转到它身上**：换一卷只在[展得开的那几卷](Branch::expandable)之间转——
    /// 转过去那一格里就只剩一句话，而那不是「换一卷」要给的东西。
    ///
    /// 走的是 [`expanding`] 而不是 [`press`]：造得出一份攒着的报告，造不出一趟
    /// 真跑着的（起线程那一处在 `super::run`）。
    #[test]
    fn a_volume_that_never_got_made_can_be_selected_and_says_it_cannot_be_expanded() {
        let mut live = live::Live::new(&live::fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/棋魂 07"), 1000);
        live.volume_finished(&live::fixture::skipped_volume("棋魂 07", 184));
        live.volume_failed(Path::new("库/消失的那卷"), "卷根不在了");
        let mut session = Session::new();
        session.run_started();

        // 表上两行，两行都停得住——从前没做成的那一卷不在这一列里。
        assert_eq!(
            live.volumes(),
            [Volume::Settled(0), Volume::Failed(0)],
            "没做成的那一卷停不上去"
        );
        // 自动滚动着的时候光标停在最新**收摊**的那一卷上：没做成的那一卷不抢自动滚动。
        assert_eq!(session.standing(&live), Some(Volume::Settled(0)));

        // 光标挪到没做成的那一卷上（这一趟只有一枝，`↑↓` 在这一枝底下挪）。
        session.open(PathBuf::from("库"));
        session.select(&live, state::Step::Next);
        assert_eq!(session.standing(&live), Some(Volume::Failed(0)));

        // 在它上面按展开：不进展开态，屏上当场说清为什么没有第二步。
        expanding(&mut session, &live, Action::Expand);
        assert!(session.expansion().is_none(), "没做成的那一卷展开了");
        let said = session.notice().expect("该说一句").said().to_owned();
        assert_eq!(said, CANNOT_EXPAND, "说的不是那一句：{said}");

        // 展开收摊了的那一卷照旧进得去，而 `⇥` 转一圈仍旧落回它自己：
        // 这一枝底下展得开的只有它一卷，没做成的那一条不在那个圈里。
        session.select(&live, state::Step::Next);
        expanding(&mut session, &live, Action::Expand);
        assert_eq!(
            session.expansion().expect("该展开了").volume,
            Volume::Settled(0)
        );
        expanding(&mut session, &live, Action::Turn(state::Step::Next));
        assert_eq!(
            session.expansion().expect("还展开着").volume,
            Volume::Settled(0),
            "`⇥` 转到了展不开的那一卷上"
        );
    }

    /// **停在设备设置上按一个键，灰阶测试图就落在盘上**（13 号票第一、二、三条）。
    ///
    /// 接头处在这一层：状态机派得出[出灰阶测试图](Action::Chart)那个动作，落盘整件事在库里
    /// （[`tonefit::write_calibration_chart`]）。写出来的字节**与库直接写的逐字节相同**——
    /// 这一条就是「会话只是调那个接口」的说法：会话若自己拼过一格像素，两份就分得开。
    /// 图仍是量具（不判定、不量化、无损写出、不带记录）由库那一侧的用例钉着。
    ///
    /// 型号没挑那一下也在这里：说一句，盘上一个字节都不多。
    #[test]
    fn the_chart_key_hands_the_device_layer_to_the_library_seam() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let here = space.path().join("会话是从这儿敲起来的");
        std::fs::create_dir_all(&here).expect("建得出那个目录");
        let mut session = Session::new();
        let mut running = Running::default();
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&space);
        session.go_to(state::Field::Profile);

        // 型号还没挑：说一句，会话原地不动，那个目录里一个文件都没多。
        assert_eq!(
            press(&mut session, &mut running, &nowhere, &here, Key::Char('c')),
            Exit::Stay
        );
        let said = session.notice().expect("该说一句").said().to_owned();
        assert!(said.contains("先挑型号"), "{said}");
        assert_eq!(
            std::fs::read_dir(&here).expect("读得出那个目录").count(),
            0,
            "型号没挑却写出了东西"
        );

        // 挑一个型号、覆盖一次屏幕灰阶数，再按一次：图落在那个目录下。
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        press(&mut session, &mut running, &nowhere, &here, Key::Char('c'));

        let written: Vec<PathBuf> = std::fs::read_dir(&here)
            .expect("读得出那个目录")
            .map(|entry| entry.expect("读得出那一条").path())
            .collect();
        assert_eq!(written.len(), 1, "{written:?}");
        let chart = &written[0];
        // 名字里带着型号与屏幕灰阶数：换一台设备出的是另一张图，不该盖掉上一张。
        let name = chart.file_name().expect("有名字").to_string_lossy();
        assert!(name.contains("boox-poke6") && name.contains("8"), "{name}");
        assert!(name.ends_with(".png"), "{name}");
        // 与库直接写出来的逐字节相同——会话一格像素都没自己拼。
        let straight = space.path().join("库自己写的.png");
        tonefit::write_calibration_chart(
            &session.chart_profile().expect("设备设置填齐了"),
            &straight,
        )
        .expect("库写得出来");
        assert_eq!(
            std::fs::read(chart).expect("读得出图"),
            std::fs::read(&straight).expect("读得出库写的那张"),
            "会话写出来的图与库直接写的不一样"
        );
        // 屏上说清图在哪儿，以及此刻要做对的那一件事。
        let said = session.notice().expect("出完图要说一句").said().to_owned();
        assert!(said.contains(&*name), "{said}");
        assert!(said.contains("原尺寸"), "{said}");
        // 会话还在浏览：出图不改变它此刻在做什么。
        assert_eq!(session.focus(), &state::Focus::Config);
    }

    /// **写不出去时会话说得清，而且不崩**（13 号票第五条）。
    ///
    /// 逼出来的是「父目录建不了」那一种：把图该落的那个目录的位置摆一个**文件**，
    /// 库那一侧 `create_dir_all` 当场失败。盘满那一种走的是同一条回路
    /// （都是库交回一个 `Err`，见 `crate::calibrate::write_chart`），
    /// 在用例里造不出来，也不必造第二遍。
    #[test]
    fn a_chart_that_cannot_be_written_says_so_and_the_session_stays_open() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        // 这儿本该是个目录，摆的却是个文件——图落不进去，父目录也建不出来。
        let here = space.path().join("这是个文件");
        std::fs::write(&here, "不是目录").expect("写得出那个文件");
        let mut session = Session::new();
        let mut running = Running::default();
        let nowhere = presets(&space);
        session.device.profile = Some("boox-poke6".to_owned());

        assert_eq!(
            press(&mut session, &mut running, &nowhere, &here, Key::Char('c')),
            Exit::Stay,
            "写不出去把会话带走了"
        );

        // 说得清是哪一步、在哪条路径上出的事——库那一侧的原话，这一层不另编一份。
        let said = session
            .notice()
            .expect("写不出去要说一句")
            .said()
            .to_owned();
        assert!(said.contains("灰阶测试图"), "{said}");
        assert!(said.contains("这是个文件"), "{said}");
        // 三组设置一格没动，会话还在浏览：下一个键照按。
        assert_eq!(session.focus(), &state::Focus::Config);
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Down),
            Exit::Stay
        );
    }

    /// **存出去再套回来，两层逐格相同，而路径与输出一格没动**（本票的四条验收）。
    ///
    /// 走的是真文件：`p` 列出来、末行 `⏎` 打一个名字存下去、改乱两层、再 `p` 套回来。
    /// 盘在临时目录下（见 [`presets`]），一个用户的东西都不碰。
    ///
    /// 「没说」与「说了默认值」的差别一并钉在这里：存之前把缩放方式转到**恰好等于默认值**
    /// 的那一档上，套回来之后它仍是「说了」而不是「没说」（停车场 Q58）。
    #[test]
    fn what_the_session_stores_is_what_it_takes_back() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.paths.push(state::NamedPath {
            path: PathBuf::from("库/卷一"),
            on: true,
        });
        // 设备设置挑一个型号，处理选项点两项：一项与默认值不同，一项**恰好等于**默认值。
        session.device.profile = Some("boox-poke6".to_owned());
        session.taste.filter = Some(tonefit::Filter::Hamming);
        session.taste.fit = Some(tonefit::FitMode::default());
        let stored = session.preset();
        let scope = session.scope.clone();

        // 存：`p` 开那一栏，光标落在唯一那一行（＋ 存成一份新的）上，打个名字按 ⏎。
        tap(&mut session, &mut running, &presets, Key::Char('p'));
        let picker = session.picking().expect("那一栏该开着");
        assert!(picker.names().is_empty(), "临时目录下还不该有预设");
        tap(&mut session, &mut running, &presets, Key::Enter);
        for character in "漫画".chars() {
            tap(&mut session, &mut running, &presets, Key::Char(character));
        }
        tap(&mut session, &mut running, &presets, Key::Enter);
        let said = session.notice().expect("存完要说一句").said().to_owned();
        assert!(said.contains("漫画") && said.contains("--preset"), "{said}");
        // 存好的那一份就摆在眼前的列表上，光标停在它上面。
        let picker = session.picking().expect("存完仍在那一栏上");
        assert_eq!(picker.names(), ["漫画"]);
        assert_eq!(picker.picked(), Some("漫画"));

        // 改乱两层，再把那一份套回来。
        tap(&mut session, &mut running, &presets, Key::Esc);
        session.taste.filter = Some(tonefit::Filter::Area);
        session.taste.fit = None;
        session.device.profile = Some("kobo-libra-2".to_owned());
        tap(&mut session, &mut running, &presets, Key::Char('p'));
        tap(&mut session, &mut running, &presets, Key::Enter);

        assert_eq!(session.preset(), stored, "套回来的两层与存出去的不一样");
        assert_eq!(session.scope, scope, "套用预设动了路径与输出");
        assert_eq!(
            session.taste.fit,
            Some(tonefit::FitMode::default()),
            "「说了一个恰好等于默认值的值」套回来变成了「没说」"
        );
        // 套完回到浏览，说的那句话里带着「路径与输出没动」。
        let said = session.notice().expect("套完要说一句").said().to_owned();
        assert!(said.contains("路径与输出"), "{said}");
    }

    /// **命令行上 `--preset` 拿到的，与会话里存出去的是同一份**（本票的第五条验收）。
    ///
    /// 两侧同一份格式这件事在 `preset` 那一层就成立（往返用例），这里问的是**接头**：
    /// 会话写出去的那份文件，`Cli` 那一路读得懂，而且合出来的 `Request` 与会话拼的一样。
    #[test]
    fn a_preset_saved_in_the_session_is_the_one_the_command_line_takes() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(12);
        session.taste.filter = Some(tonefit::Filter::Hamming);
        session.taste.envelope = Some(true);
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.paths.push(state::NamedPath {
            path: PathBuf::from("库/卷一"),
            on: true,
        });

        tap(&mut session, &mut running, &presets, Key::Char('p'));
        tap(&mut session, &mut running, &presets, Key::Enter);
        for character in "漫画".chars() {
            tap(&mut session, &mut running, &presets, Key::Char(character));
        }
        tap(&mut session, &mut running, &presets, Key::Enter);
        assert!(
            session
                .notice()
                .is_some_and(|said| said.said().contains("存好了")),
            "{:?}",
            session.notice()
        );
        tap(&mut session, &mut running, &presets, Key::Esc);

        // 命令行那一路读盘上那份文件的正文，拿到的是同一份预设。
        let text = std::fs::read_to_string(presets.path().expect("说得出位置")).expect("读得出来");
        let read_back = crate::preset::read(&text, "漫画").expect("命令行这一路读得懂");
        assert_eq!(read_back, session.preset());

        // 合出来的这一趟也一样：会话拼的与 `--preset 漫画` 拼的逐项相同。
        let asked = session
            .request(tonefit::Mode::Process)
            .expect("会话拼得出来");
        let command_line =
            crate::Cli::try_parse_from(["tonefit", "--out", "出", "--preset", "漫画", "库/卷一"])
                .expect("命令行读得懂")
                .request(&read_back)
                .expect("拼得出来");
        assert_eq!(asked.profile, command_line.profile);
        assert_eq!(asked.filter, command_line.filter);
        assert_eq!(asked.envelope, command_line.envelope);
        assert_eq!(asked.inputs, command_line.inputs);
    }

    /// **撞上同名的那一份：先说一句，再按一次才覆盖**——不静默盖掉别人手写的东西。
    ///
    /// 三件事一条钉住：第一下一个字节都不写；**名字一改那一问就作废**（不然改成另一个
    /// 已有的名字就被上一次的确认捎带着盖掉了）；覆盖之后**别的那几份预设与手写的注释仍在**。
    /// 换掉的恰好是那一份自己那几节，逐字节那一条在 `preset::insert` 那一侧钉着。
    #[test]
    fn overwriting_a_preset_takes_a_second_press() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        session.taste.filter = Some(tonefit::Filter::Hamming);
        // 盘上先摆两份手写的预设，连注释一起。
        let file = presets.path().expect("说得出位置").to_path_buf();
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得出配置目录");
        let by_hand = "# 手写的\n[preset.\"漫画\".taste]\nfilter = \"box\"\n\n\
                       [preset.\"画集\".taste]\nenvelope = true\n";
        std::fs::write(&file, by_hand).expect("写得出来");

        // `p` 开那一栏，`↑` 绕到末尾那一行上，`⏎` 打一个名字。
        tap(&mut session, &mut running, &presets, Key::Char('p'));
        tap(&mut session, &mut running, &presets, Key::Up);
        assert_eq!(
            session.picking().expect("那一栏该开着").picked(),
            None,
            "↑ 没绕到末尾那一行上"
        );
        tap(&mut session, &mut running, &presets, Key::Enter);
        for character in "漫画".chars() {
            tap(&mut session, &mut running, &presets, Key::Char(character));
        }

        // 打的是已经有的那个名字：说一句，盘上一个字节都没动。
        tap(&mut session, &mut running, &presets, Key::Enter);
        let said = session.notice().expect("要说一句").said().to_owned();
        assert!(said.contains("再按一次"), "{said}");
        assert!(said.contains("撤不回来"), "覆盖的代价没说出口：{said}");
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            by_hand,
            "第一下就把手写的那份盖掉了"
        );

        // 名字改成另一个**也已经有的**：上一次那一问不作数，这一下仍是先问一句。
        for _ in 0..2 {
            tap(&mut session, &mut running, &presets, Key::Backspace);
        }
        for character in "画集".chars() {
            tap(&mut session, &mut running, &presets, Key::Char(character));
        }
        tap(&mut session, &mut running, &presets, Key::Enter);
        assert!(
            session.notice().is_some_and(
                |said| said.said().contains("画集") && said.said().contains("再按一次")
            ),
            "改过名字之后那一问该重新来一遍：{:?}",
            session.notice()
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            by_hand,
            "改了个名字就被上一次的确认捎带着盖掉了"
        );

        // 再按一次：这一下才覆盖，而另一份一个字都没丢。
        tap(&mut session, &mut running, &presets, Key::Enter);
        assert!(
            session
                .notice()
                .is_some_and(|said| said.said().contains("存好了")),
            "{:?}",
            session.notice()
        );
        assert_eq!(
            presets.read("画集").expect("读得回来"),
            session.preset(),
            "覆盖之后盘上那一份不是刚存的"
        );
        assert_eq!(
            presets.read("漫画").expect("读得回来").taste.filter,
            Some(tonefit::Filter::Area),
            "覆盖一份把另一份也改了"
        );
        // 换掉的只有那一份自己那几节：手写的那行注释还在盘上（本票第三条）。
        let after = std::fs::read_to_string(&file).expect("读得出来");
        assert!(after.starts_with("# 手写的\n"), "手写的注释没了：\n{after}");
    }

    /// **删一份要按两下，而按错一下没有撤销**（停车场 Q74，本票第一、二条）。
    ///
    /// 三件事一条钉住：第一下盘上一个字节都不动；**光标一挪那一问就作废**
    /// （不然挪到另一份上再按一下，就被上一次的确认捎带着删了）；
    /// 删掉之后那份文件里其余的字节逐个在原处，手写的注释与本版本读不懂的那一份都在。
    #[test]
    fn erasing_a_preset_takes_a_second_press() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        let file = presets.path().expect("说得出位置").to_path_buf();
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得出配置目录");
        // 盘上两份手写的预设，连注释一起；后一份本版本读不懂。
        let head = "# 我手写的\n";
        let mine = "[preset.\"漫画\".taste]\nfilter = \"box\"\n\n";
        let tail = "# 这一项本模块读不懂\n[preset.\"画集\".taste]\nsharpen = true\n";
        let by_hand = format!("{head}{mine}{tail}");
        std::fs::write(&file, &by_hand).expect("写得出来");

        // `p` 开那一栏，光标停在第一份上。`d` 第一下只问一句，盘上一个字节都没动。
        tap(&mut session, &mut running, &presets, Key::Char('p'));
        assert_eq!(
            session.picking().expect("那一栏该开着").picked(),
            Some("漫画")
        );
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        let said = session.notice().expect("要说一句").said().to_owned();
        assert!(said.contains("漫画") && said.contains("再按一次"), "{said}");
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            by_hand,
            "第一下就把那一份删了"
        );

        // 挪到另一份上：上一次那一问不作数，这一下仍是先问一句。
        tap(&mut session, &mut running, &presets, Key::Down);
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        assert!(
            session.notice().is_some_and(
                |said| said.said().contains("画集") && said.said().contains("再按一次")
            ),
            "挪过一行之后那一问该重新来一遍：{:?}",
            session.notice()
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            by_hand,
            "挪了一行就被上一次的确认捎带着删了"
        );

        // 挪回来按两下：这一下才真删，而删掉的恰好是那一份自己写下的那几节。
        tap(&mut session, &mut running, &presets, Key::Up);
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        assert!(
            session
                .notice()
                .is_some_and(|said| said.said().contains("删掉了")),
            "{:?}",
            session.notice()
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            format!("{head}\n{tail}"),
            "删掉的不止那一份自己"
        );
        assert!(presets.read("漫画").is_err(), "删完还读得回来");
        assert_eq!(
            session.picking().expect("删完还在这一栏上").names(),
            ["画集"]
        );
    }

    /// **屏底换了一句别的话，那一问就不作数了**——不然下一下 `d` 成了不问自删。
    ///
    /// 中间那一句由**套用一份读不懂的预设**顶上来（`complain` 那一条路，留在这一栏上）：
    /// 「再按一次 d」四个字被顶掉之后，用户看见的是一条错误，而不是一问——
    /// 闩不该比说出它的那句话活得长（见 `Session::says`）。
    #[test]
    fn a_question_that_scrolled_off_the_screen_is_no_longer_a_question() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        let file = presets.path().expect("说得出位置").to_path_buf();
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得出配置目录");
        // 这一份本版本读不懂：套用它是一条错误，而会话留在这一栏上。
        let by_hand = "[preset.\"漫画\".taste]\nsharpen = true\n";
        std::fs::write(&file, by_hand).expect("写得出来");

        tap(&mut session, &mut running, &presets, Key::Char('p'));
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        // 套用失败：屏底改说那条错误，「再按一次 d」没了。
        tap(&mut session, &mut running, &presets, Key::Enter);
        let said = session.notice().expect("要说一句").said().to_owned();
        assert!(!said.contains("再按一次"), "那一问还摆在屏上：{said}");

        // 这一下 `d` 是**重新问一句**，不是删。
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        assert!(
            session
                .notice()
                .is_some_and(|said| said.said().contains("再按一次")),
            "{:?}",
            session.notice()
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("读得出来"),
            by_hand,
            "那一问被一句别的话顶掉之后，下一下 d 不问自删了"
        );
    }

    /// **那一份在这中间没了：说得清，不崩，那一栏还开着**（本票第六条）。
    ///
    /// 这一栏列的是**进来那一刻**盘上有的（`Picker`），而删是盘那一侧的事——
    /// 两下之间别处把它删掉了，这一下报的是库那一侧的原话（哪一份不在、有的是哪几份）。
    #[test]
    fn erasing_a_preset_that_is_no_longer_on_disk_says_so_and_stays_open() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        let file = presets.path().expect("说得出位置").to_path_buf();
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得出配置目录");
        std::fs::write(&file, "[preset.\"漫画\".taste]\nfilter = \"box\"\n").expect("写得出来");

        tap(&mut session, &mut running, &presets, Key::Char('p'));
        // 开了那一栏之后，别处把它换成了另一份内容。
        std::fs::write(&file, "[preset.\"画集\".taste]\nenvelope = true\n").expect("写得出来");
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        tap(&mut session, &mut running, &presets, Key::Char('d'));

        let said = session.notice().expect("要说一句").said().to_owned();
        assert!(said.contains("漫画"), "没说清点的是哪一份：{said}");
        assert!(said.contains("画集"), "没说有的是哪几份：{said}");
        assert!(session.picking().is_some(), "说完把那一栏关掉了");
    }

    /// **读不懂的预设在会话里当场报错，不静默套默认值**（本票的第四条验收，spec 的 story 39）。
    ///
    /// 报出来的是库那一侧的原话（会话不另编一句），而两层一格都没动——
    /// 套用失败不该留下一份「套了一半」的配置。
    #[test]
    fn a_preset_the_session_cannot_read_says_so_and_changes_nothing() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let file = presets.path().expect("说得出位置").to_path_buf();
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得出配置目录");
        std::fs::write(&file, "[preset.\"旧的\".taste]\nsharpen = true\n").expect("写得出来");
        let mut session = Session::new();
        let mut running = Running::default();
        session.taste.filter = Some(tonefit::Filter::Hamming);
        let before = session.preset();

        tap(&mut session, &mut running, &presets, Key::Char('p'));
        // 列出来这一步只读名字：那一份读不懂，仍列得出来。
        assert_eq!(
            session.picking().expect("那一栏该开着").names(),
            ["旧的".to_owned()]
        );
        tap(&mut session, &mut running, &presets, Key::Enter);

        let said = session.notice().expect("要说一句").said().to_owned();
        assert!(said.contains("旧的"), "{said}");
        assert_eq!(session.preset(), before, "套不成却把两层改了");
        assert!(session.picking().is_some(), "读不懂就把那一栏也关掉了");
    }

    /// 键码翻译认得会话要的那几个，别的原地放过。
    ///
    /// 这一层薄到只剩一张对照表，规矩在 [`super::state`]——那边的用例问的是
    /// 「这个键在这个状态下做什么」，这边只问「这个键码是哪个键」。
    #[test]
    fn the_key_codes_the_session_answers_to() {
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

        assert_eq!(translate(&press(KeyCode::Up)), Some(Key::Up));
        assert_eq!(translate(&press(KeyCode::Enter)), Some(Key::Enter));
        assert_eq!(translate(&press(KeyCode::Tab)), Some(Key::Tab));
        // `⇧⇥` 报的是一个单独的键码，不是 Tab 加一个修饰键。
        assert_eq!(translate(&press(KeyCode::BackTab)), Some(Key::BackTab));
        assert_eq!(translate(&press(KeyCode::Esc)), Some(Key::Esc));
        assert_eq!(translate(&press(KeyCode::Char(' '))), Some(Key::Space));
        assert_eq!(translate(&press(KeyCode::Char('q'))), Some(Key::Char('q')));
        // Ctrl-C 在**每一个**状态下都是退出，因此先于普通字符认出来。
        assert_eq!(
            translate(&KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Key::Interrupt)
        );
        // 认不出的键原地放过，不必在状态机那边各占一个取值。
        assert_eq!(translate(&press(KeyCode::F(5))), None);
        assert_eq!(translate(&press(KeyCode::PageDown)), None);
    }

    /// **「还没跑过」那一句里的两个键出自交给它的那张表**（`no-false-line/06`，
    /// 收停车场 Q190）：喂一副假键（[`Starters::faked`]），句子里得是这一副。
    /// 那两支按键本来到不了（见 [`expand`]），直接调它问那一句。
    #[test]
    fn the_not_run_yet_complaint_names_the_keys_the_table_hands_it() {
        assert_eq!(
            not_run_yet(&Starters::faked(Some("r"), Some("w"))),
            "还没跑过：先按 r 预览或 w 转换，报告出来了才展得开"
        );
        assert_eq!(
            not_run_yet(&Starters::faked(None, None)),
            "还没跑过：报告出来了才展得开"
        );

        // 真会话里那一句：问的是真按键表。
        let mut session = Session::new();
        expand(&mut session, &Running::default(), Action::Expand);
        let said = session.notice().expect("说一句").said().to_owned();
        let starters = draw::keys::starters(&session);
        assert!(
            said.contains(&format!("{} 预览", starters.dry.expect("预览那个键"))),
            "{said}"
        );
        assert!(
            said.contains(&format!("{} 转换", starters.run.expect("转换那个键"))),
            "{said}"
        );
    }
}
