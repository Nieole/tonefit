//! 终端那一侧：进出终端、把键码翻译成会话认得的键、在两者之间转一个循环。
//!
//! **本仓库唯一一处认得 crossterm 键码的地方**（见 [`translate`] 与 [`translate_input`]），
//! 也是唯一一处握着终端的地方（见 [`Screen`]）。状态机、边跑边攒的那一份、起线程与逐层补全
//! 都在 [`super`] 的别的模块里，摆在 `tui` 特性**外面**——分界与理由见
//! `super` 的模块文档《终端库在哪一半》。
//!
//! 除了那三件事，这一层还担着**状态机够不着的那几支**（见 [`input`]）：
//! 起一趟、按停止、答话、进一卷的每页结果、读写盘上那份预设、把灰阶测试图交给库里第三个 seam。
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
use super::home::Home;
use super::keymap::{Deed, Phase};
use super::live::{Live, Resuming, VolumeState};
use super::look::{Kind, Look, Segment};
use super::run::Running;
use super::shell;
use super::state::{Exit, Key, Session};
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
    // 家目录问一次、摆在会话上往下传（`CONTEXT.md` 的《会话》：家目录）：屏上把它缩写成 `~`。
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

/// 画一屏、等一个输入（最多等 [`TICK`]）、做掉它，直到用户退出。
fn drive(
    screen: &mut Screen,
    session: &mut Session,
    running: &mut Running,
    presets: &Presets,
    here: &Path,
) -> Result<()> {
    loop {
        // **这一帧的「此刻」**：单调时钟一帧读一次，会话里要时刻的地方都读它
        // （`CONTEXT.md` 的《会话》：此刻；`session-redesign/04`）——屏上那几个数、
        // 屏底那句回话到没到点、连击键的待续记号，出自同一个时刻。
        let now = Instant::now();
        {
            // 借着锁画：画完当场还回去，计算线程最多等一帧的功夫（见 `Running::live`）。
            let mut live = running.live();
            if let Some(live) = live.as_deref_mut() {
                live.tick(now);
                // **每一帧盯一眼那一趟**：清点的产出到了就把树拼出来，自动滚动开着时
                // 光标跟到正在处理的那一卷（`Session::watch_the_run`）。
                session.watch_the_run(live);
            }
            screen
                .terminal
                .draw(|frame| shell::draw(frame, session, live.as_deref(), now))?;
        }
        if event::poll(TICK)? {
            // 只认按下去那一下：Windows 上按键抬起也报一条，不滤掉的话每个键都走两遍。
            let Event::Key(pressed) = event::read()? else {
                continue;
            };
            let size = screen.terminal.size()?;
            let window = Window {
                cols: size.width,
                rows: size.height,
            };
            if pressed.kind == KeyEventKind::Press
                && let Some(typed) = translate_input(&pressed)
                && input(session, running, presets, here, now, window, typed) == Exit::Leave
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

/// 把一个输入交给会话（ADR 0019；spec《缝》）。收的是键或鼠标（[`Input`]），
/// 带着这一帧的「此刻」与窗口的尺寸；那条循环每收到一个输入调它一次（[`drive`]）。
///
/// 先把输入认成按键表上的一件事（[`Session::deed_of`]，连击键在那里待着），
/// **够得着那一趟、那块盘与屏的那几件在这一层做**，其余交回状态机（[`Session::perform`]）：
/// 覆盖层上滚动（那一张有几行、露几行都从窗口的尺寸算，[`cover::Sheet`]，而窗口有多大
/// 只有这一层知道）、半屏与一屏、起一趟、按停止、答话、`F` 之后光标跟上正在处理的那一卷、
/// 搜索与跳转的落点（哪几卷出了事只有那一趟答得出）、进一卷的每页结果、
/// 预设那几支与灰阶测试图。
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
        // **起一趟**（[`begin`]）：起线程、拼 `Request`、把观察者接上去，一件都不在状态机里。`t`／`x` 开跑**总回到任务视图**
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
        // 跑着的那一趟。两处记的是同一个字，出处只有状态机那一份。
        Deed::Stop => {
            let exit = session.perform(deed, now);
            running.stop(session.stopping());
            exit
        }
        // **确认点上答话那三件**：状态机把会话放回「跑着」那一副、屏底说一句
        // （`Session::perform`），这一层把那个字**连同它管几卷**交给停在确认点上的那条线程。
        // 与按停止同一条分工——认键在那边，碰线程在这边；而「哪一件答哪个字」在
        // [`Deed::answer`] 一处。
        //
        // **它不进闩**：按停止按到的那一级一格不动（`CONTEXT.md` 的《等待确认》），
        // 因此这里不调 `running.stop`。「后面的卷都写出」进的是观察者那一侧的
        // 「确认点的默认答案」，那一格在 [`Running::decide`] 里。
        Deed::Write | Deed::WriteAll | Deed::End => {
            let exit = session.perform(deed, now);
            if let Some((said, reach)) = deed.answer() {
                running.decide(said, reach);
            }
            exit
        }
        // **等待确认时 `v` 进这一卷的每页结果**：要问那一趟「此刻停在哪一卷」，
        // 而状态机读不到它——与卷行上按展开同一条分工。
        Deed::ViewPages => {
            view_the_pages(session, running);
            Exit::Stay
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
        // 套用一份不在这里，掀开那一刻已经读进来了
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

/// **等待确认时 `v` 换屏进每页结果**（`CONTEXT.md` 的《等待确认》：`v` 进这一卷的每页结果，
/// `h` 回来再答）。
///
/// 进的是**确认点上那一卷**——不是光标停着的那一行（那一下是 `l`，走 [`open_a_volume`]）：
/// 屏上这一刻问的就是这一卷，而卷列表照样滚得动，光标早挪到别处去了也不影响这一问。
/// 「此刻停在哪一卷」只有那一趟答得出（[`Live::walking`]），状态机读不到它。
///
/// **一卷的身份是卷根**，与[卷列表的光标](Cursor::Volume)记的是同一样（[`Pages::of`]）：
/// `h` 回去时那一行本来就在光标底下。**屏底一句话都不说**——换了一整屏，
/// 那一屏自己就是回话（与 [`open_a_volume`] 展得开那一支同一条）。
fn view_the_pages(session: &mut Session, running: &Running) {
    let live = running.live();
    let Some(walking) = live.as_deref().and_then(Live::walking) else {
        return;
    };
    session.views.task.pages = Some(Pages::of(walking.volume.clone()));
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

// ───────────────────────── 预设那几支与灰阶测试图 ─────────────────────────
//
// **碰盘的在这一层**，认键与屏上那几格在状态机。四支各走各的函数，不合成一个收 `Deed`
// 的分派——合起来就要留一支「到不了」的 `_`，而那正是新添一支会被静默吃掉的地方
// （停车场 Q74）。

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
/// （停车场 Q74 把这条约束说死了）。
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
/// 这一层只点了个名：建的不是目录、写的不是文件，父目录不在就建出来也是那一头的事；
/// 写不出去时库那一侧回的 `Err` 原样端到屏底，会话原地不动。图落在哪儿见 [`chart_file`]。
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

/// 终端那一侧的事件 → 会话认得的[输入](Input)：键照 [`translate`]，Ctrl 加一个字母另认
/// （`C-d`／`C-u`／`C-f`／`C-b`／`C-w`），认不出的返回 `None`。滚轮与单击随鼠标那一票接上。
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
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
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

/// 经输入入口（[`input`]）的用例：喂交互序列，走完对交互期望屏（spec《交互序列》；`session-redesign/06`）。
#[cfg(test)]
mod redesign {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::super::cover::Overlay;
    use super::super::look::{Kind, Look, Segment};
    use super::super::run::Running;
    use super::super::scene::{self, Scene, Step};
    use super::super::shell;
    use super::super::shell::design::{self, Expected, assert_no_background, assert_same_cells};
    use super::super::state::{Exit, Key};
    use super::super::view::{Cursor, Focus, Input, Pane, Window};
    use crate::preset::Presets;
    use std::path::PathBuf;
    use tonefit::FitMode;

    /// 用例里那份预设文件：位置点在**临时目录**里（[`Presets::at`]），
    /// 因此不必去改进程的环境变量（`tests/preset.rs` 说过为什么不改）。
    /// 一个用户的东西都不碰。
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

    /// 从这一串的起点场景起，逐步喂给输入入口；回走完那一刻的场景、那一趟与最后一步的去留。
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
            // **夹具没有线程**：确认点上答了「不写出，结束预览」之后，那条线程把这一卷
            // 收了摊（写出环节一步不走，`tonefit::Pass::Second` 的文档）、这一趟就此收场，
            // 主循环随后 `reap` 到它、会话回到结束了——这一步走的是同一条路，只是当场走完。
            // 与底下按到立即停止那一段同形。
            //
            // **收摊用的就是确认点上攒着的那一份**：那一卷写出环节一步都没走，
            // 库交出来的与攒着的逐格相同（`scene::replay` 的 `trialed` 那一支同样这么摆）。
            let trialed = running.live().as_deref().and_then(|live| {
                if live.ended() || live.decided() != Some(tonefit::Instruction::Finish) {
                    return None;
                }
                live.summarized().cloned()
            });
            if let Some(report) = trialed
                && let Some(mut live) = running.live()
            {
                let elapsed = live.overall().elapsed;
                live.volume_finished(&report);
                live.run_finished(tonefit::RunOutcome::Stopped(tonefit::Instruction::Finish));
                let mut whole = live.report().clone();
                whole.elapsed = elapsed;
                live.returned(Ok(whole));
                drop(live);
                scene.session.run_finished();
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
    /// 接头处是本层唯一做的事：状态机把闩升一级，本层把升到的那一级交给 [`Running::stop`]。**两头记的是同一个字**。
    /// 一个终端都不碰——[`super::input`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn pressing_stop_through_the_input_reaches_the_run_at_both_levels() {
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

    /// **`t`／`x` 开跑**（票面第一条）：起一趟走的是 [`super::begin`]（拼 `Request`、
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

    /// 把一个字符经**新输入入口**（[`super::input`]）交给会话。
    fn feed(
        session: &mut crate::session::state::Session,
        running: &mut Running,
        presets: &Presets,
        here: &std::path::Path,
        glyph: char,
    ) -> Exit {
        super::input(
            session,
            running,
            presets,
            here,
            std::time::Instant::now(),
            Window {
                cols: 120,
                rows: 36,
            },
            Input::Key(Key::Char(glyph)),
        )
    }

    /// 等那条线程走到确认点上，会话跟着换一副样子——真会话里这一问每帧一次
    /// （见 [`super::drive`]）。**转到条件成立为止**，不 sleep 撞运气。
    fn settle_at_the_decision_point(
        session: &mut crate::session::state::Session,
        running: &mut Running,
    ) {
        while !running.deciding() {
            assert!(!running.reap(), "那一趟一句话都没问就跑完了");
            std::thread::yield_now();
        }
        session.at_the_decision_point(true);
    }

    /// 起一趟预览（两个卷各一页），跑到**头一个确认点**上停住：回会话与那一趟。
    fn waiting_at_the_first_decision_point(
        space: &tempfile::TempDir,
        out: &std::path::Path,
        names: [&str; 2],
    ) -> (crate::session::state::Session, Running) {
        let mut session = crate::session::state::Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(out.to_path_buf());
        for name in names {
            session.scope.paths.push(crate::session::state::NamedPath {
                path: crate::session::live::fixture::a_real_volume(space.path(), name),
                on: true,
            });
        }
        let mut running = Running::default();
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(space);
        // 按 `t`：预览，因此这一趟改走 `Mode::Process` 并在每个确认点上等人
        // （[`super::resuming`]）。
        assert_eq!(
            feed(&mut session, &mut running, &nowhere, space.path(), 't'),
            Exit::Stay
        );
        settle_at_the_decision_point(&mut session, &mut running);
        (session, running)
    }

    /// **三种答法经新输入入口到达等在确认点上的那条线程**（票面第三条）。
    ///
    /// 接头处与按停止那一条同一个位置
    /// （[`pressing_stop_through_the_input_reaches_the_run_at_both_levels`]）：
    /// 状态机把会话放回「跑着」那一副，本层把那个字**连同它管几卷**交给
    /// [`Running::decide`]。「哪一件答哪个字」只有 [`Deed::answer`] 一处。
    ///
    /// 走的是整条路，**两趟**：
    ///
    /// - 头一趟：头一卷按 `x`（只写这一卷）→ **第二卷照旧停下来问** → 按 `s`
    ///   （这一卷不写、就此收场）。盘上因此只有头一卷。
    /// - 第二趟：头一卷按 `a` → 一路做完，**一次都不再问**。盘上两卷都有。
    ///
    /// **按停止按到的那一级自始至终一格不动**（票面第三条末一句；
    /// `CONTEXT.md` 的《等待确认》：这里的 `s` 不是按停止）。
    ///
    /// 不开终端：[`super::input`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn the_three_answers_through_the_input_reach_the_thread_waiting_at_the_point() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let nowhere = presets(&space);

        // ── 头一趟：`x` 只管这一卷，`s` 让它就此收场 ──
        let out = space.path().join("出一");
        let (mut session, mut running) =
            waiting_at_the_first_decision_point(&space, &out, ["卷一", "卷二"]);
        assert!(session.deciding(), "那一趟停住了，会话却没跟着换一副样子");
        assert_eq!(
            feed(&mut session, &mut running, &nowhere, space.path(), 'x'),
            Exit::Stay
        );
        assert!(!session.deciding(), "答完话会话还停在确认点上");

        // 第二卷照旧问——`x` 没有替它答话。
        settle_at_the_decision_point(&mut session, &mut running);
        assert_eq!(
            feed(&mut session, &mut running, &nowhere, space.path(), 's'),
            Exit::Stay
        );
        while !running.reap() {
            session.at_the_decision_point(running.deciding());
            std::thread::yield_now();
        }
        session.run_finished();
        assert!(out.join("卷一").is_dir(), "答了 `x` 的那一卷没写出来");
        assert!(!out.join("卷二").exists(), "答了 `s` 的那一卷不该写出来");
        assert_eq!(
            session.stopping(),
            tonefit::Instruction::Continue,
            "确认点上答话不动按停止按到的那一级"
        );

        // ── 第二趟：`a` 之后一次都不再问 ──
        let out = space.path().join("出二");
        let (mut session, mut running) =
            waiting_at_the_first_decision_point(&space, &out, ["卷三", "卷四"]);
        assert_eq!(
            feed(&mut session, &mut running, &nowhere, space.path(), 'a'),
            Exit::Stay
        );
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
        assert!(out.join("卷三").is_dir(), "头一卷没写出来");
        assert!(out.join("卷四").is_dir(), "剩下的那一卷没写出来");
        assert_eq!(
            session.stopping(),
            tonefit::Instruction::Continue,
            "`a` 同样不动那一级"
        );
        assert_eq!(running.pressed(), tonefit::Instruction::Continue);
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.for_the_rest(), Some(tonefit::Instruction::Continue));
        assert_eq!(live.report().volumes.len(), 2);
    }

    /// **等待确认时 `v` 进这一卷的每页结果、`h` 回来再答**（票面第三条）：
    /// 进的是**确认点上那一卷**，不是光标停着的那一行（那一刻光标停在它那个目录行上）；
    /// 确认条照旧钉在总览底下，每页结果那一屏在这一档上照样画得出——灰阶分布那一行末尾
    /// 写着「等待确认：还没写入任何文件」，框底边那一件 `a → 全部页` 照旧写着
    /// （它不随阶段改口），而**屏底那一行 `a` 让给答话**、不摆它（停车场 Q778）。
    #[test]
    fn v_while_deciding_opens_the_pages_of_that_volume_and_h_comes_back() {
        let scene = assert_sequence("deciding-v");
        let pages = scene
            .session
            .views
            .task
            .pages
            .as_ref()
            .expect("`v` 换屏进了每页结果");
        assert_eq!(
            crate::render::volume_name(&pages.volume),
            "第05卷",
            "进的是确认点上那一卷"
        );
        assert_eq!(scene.session.views.block(), Focus::Pages);
        assert!(
            matches!(scene.session.views.task.cursor, Cursor::Directory(_)),
            "卷列表的光标一格没挪：它本来就停在这一卷那个目录行上"
        );
        assert!(scene.session.deciding(), "看一眼不算答话");

        let scene = assert_sequence("deciding-v-h");
        assert!(scene.session.views.task.pages.is_none(), "`h` 回了卷列表");
        assert!(scene.session.deciding(), "回来还在确认点上，照旧答得出");
    }

    /// **`x` 写出这一卷**（票面第二条、第三条）：那个字**连同它管几卷**交到了停在确认点上的
    /// 那条线程手里（[`Running::decide`]），确认条收起来，**抬头当场翻成「转换」**
    /// ——这一卷从此在写。**结论行仍是预览那一副**：盘上这一刻还什么都没有
    /// （`Live::has_written` 要等这一卷收摊才翻，票面第四条）。
    #[test]
    fn x_writes_this_volume_and_the_title_turns_before_the_conclusion_line() {
        let (scene, running, _) = walked("deciding-x");
        assert!(!scene.session.deciding(), "答完话不再停在确认点上");
        let live = running.live().expect("那一趟还在");
        assert_eq!(
            live.decided(),
            Some(tonefit::Instruction::Continue),
            "答的那个字没记下来"
        );
        assert_eq!(live.for_the_rest(), None, "`x` 只答这一卷");
        assert!(
            live.walking().is_some_and(|walking| walking.writes),
            "这一卷从此在写：抬头照它翻成转换"
        );
        assert!(!live.has_written(), "它还没收摊，结论行这一刻不该翻");
        drop(live);
        assert_sequence("deciding-x");
    }

    /// **`a` 写出、后面的卷不再询问**（票面第二条、第三条）：同一个字，管的是后面每一卷
    /// （[`Reach::ForTheRest`]）。**按停止按到的那一级一格不动**——那一格答的是另一问
    /// （`CONTEXT.md` 的《等待确认》：这里的 `s` 不是按停止），会话与那一趟两头都没动它。
    #[test]
    fn a_answers_for_the_rest_and_leaves_the_stop_latch_alone() {
        let (scene, running, _) = walked("deciding-a");
        assert!(!scene.session.deciding(), "答完话不再停在确认点上");
        assert_eq!(
            scene.session.stopping(),
            tonefit::Instruction::Continue,
            "`a` 不该动按停止按到的那一级"
        );
        assert_eq!(
            running.pressed(),
            tonefit::Instruction::Continue,
            "那一趟记着的那一格也没动"
        );
        let live = running.live().expect("那一趟还在");
        assert_eq!(
            live.for_the_rest(),
            Some(tonefit::Instruction::Continue),
            "「后面的卷都写出」没记下来"
        );
        drop(live);
        assert_sequence("deciding-a");
    }

    /// **`s` 不写出、结束预览**（票面第二条）：这一卷的写出环节不做了，这一趟就此收场
    /// ——总览抬头换成「已停止 ⋅ 处理到第 5 卷 ⋅ 用时 21s」、右端换成输出目录，
    /// 卷列表上那一卷收了摊（目录行从 4/12 卷走到 5/12 卷）。
    ///
    /// **夹具那一头替那条线程收了手**（见 [`walked`]）：真会话里收手的是它，
    /// 主循环随后 `reap` 到它。
    #[test]
    fn s_at_the_decision_point_ends_the_preview() {
        let (scene, running, _) = walked("deciding-s");
        assert_eq!(
            scene.session.stage(),
            super::super::state::Stage::Ended,
            "那一趟收了场"
        );
        let live = running.live().expect("那一趟还在");
        assert_eq!(live.decided(), Some(tonefit::Instruction::Finish));
        assert!(!live.has_written(), "这一卷一个字节都没写，结论行不翻");
        drop(live);
        assert_sequence("deciding-s");
    }

    /// **答完一卷再推进**（票面第二条那两串）：`x` 之后 30 秒，第 5 卷写完了、
    /// **第 6 卷又停下来问**——确认条回来了，而**抬头与结论行都成了转换那一副**
    /// （这一趟真写出过一卷了，票面第四条）；`a` 之后 30 秒，
    /// 一路做到第 33 卷，**一次都没再停**：屏底只剩 `s → 停止`。
    ///
    /// **「翻过不翻回」由这两串前后两屏夹住**：`deciding-x` 那一屏上第 5 卷还没收摊，
    /// 结论行仍是预览那一副（`已分析 1 卷`）；这一串推进之后它收了摊，结论行成了
    /// `完成 1 卷 ⋅ 跳过 4 卷 ⋅ 等待 79 卷`；`deciding-a-advance` 走到第 33 卷仍是那一副。
    /// **那一格本身只升不降**由 `super::super::live` 那条用例钉着
    /// （`the_run_has_written_once_the_first_volume_it_wrote_is_finished`，
    /// 连「结束把写出过的那一格抹掉了」一起问）。
    ///
    /// **推进那几秒由这一串自己的场景数据接上**（`Scene::advance_to`，停车场 Q805）；
    /// 两串的推进都摆在末一步上，「一串只推得动一次」那条断言踩不到（停车场 Q865）。
    ///
    /// **`deciding-x-advance` 总进度那一行的末一位数换一格**（换成同一行上
    /// `46809` 里那个 `9`，同色同修饰）：设计稿那一头攒出来的 `r.steps` 是个
    /// **差一丝不到 3799 的浮点数**，导出那一步 `num()` 四舍五入写成了 `3799`，
    /// 而屏上那一格走的是 `Math.floor`、印出来是 `3798`——**同一个量，夹具与期望屏
    /// 各印了一副**。这一趟一页一步，走出来的是整数 3799（场景数据自己的那一格也这么说，
    /// `Scene` 的自检按它核过）。换完仍是一条断言：实现在那一格上写别的照样红。
    /// 停车场 **Q899**，与 Q844 那一格（连续时间对一页一步）是同一类、不同根。
    #[test]
    fn answering_once_still_asks_the_next_volume_and_for_the_rest_does_not() {
        let (scene, running, _) = walked("deciding-x-advance");
        assert!(scene.session.deciding(), "下一卷照旧停下来问");
        let live = running.live().expect("那一趟还在");
        assert!(live.has_written(), "第一卷真写完了：结论行从此是转换那一副");
        assert_eq!(live.for_the_rest(), None, "`x` 没有替后面的卷答话");
        drop(live);
        assert_sequence_with("deciding-x-advance", |expected| {
            expected.cell_like(2, 57, 63)
        });

        let (scene, running, _) = walked("deciding-a-advance");
        assert!(
            !scene.session.deciding(),
            "答过「后面的卷都写出」，它不再停下来问"
        );
        let live = running.live().expect("那一趟还在");
        assert!(live.has_written());
        assert_eq!(
            live.for_the_rest(),
            Some(tonefit::Instruction::Continue),
            "那一格一路带着"
        );
        drop(live);
        assert_sequence("deciding-a-advance");
    }

    /// **等待确认时 `2` 切到配置视图**（`CONTEXT.md` 的《视图》：人在配置视图时顶栏右端
    /// 另带着「等待确认」）：确认条不跟过去——它是任务视图那一屏上的一块，
    /// 而那一问由顶栏右端那一枚说。三组设置此刻仍旧只读、看得见、进得去。
    #[test]
    fn two_while_deciding_shows_the_config_view_with_the_waiting_badge() {
        let scene = assert_sequence("deciding-2");
        assert_eq!(scene.session.views.view, super::super::view::View::Config);
        assert!(scene.session.deciding(), "切一次视图不算答话");
    }

    /// **等待确认时 `?` 掀开全部按键**：那一张**只列这一档派得出的键**——
    /// 「确认」那一组四行（`x`／`a`／`s`／`v`，每一行那一句出自按键表）在场，
    /// 底下整屏压暗、卷列表的框跟着细下来，而**确认条照旧是粗黄框**（它不随焦点改）。
    #[test]
    fn help_while_deciding_lists_the_keys_of_this_stage() {
        let scene = assert_sequence("deciding-help");
        assert!(matches!(
            scene.session.views.cover,
            Some(Overlay::Keys { .. })
        ));
        assert!(scene.session.deciding(), "掀一张覆盖层不算答话");
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
        // （宽字符占住的第二格画布清成空格）。
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
        // **屏底那一句摆到倒数第三列为止**（`shell::footer` 的 `room`：从第 1 列起、占宽减三列）：
        // 图落在临时目录里，那条路径长短随机器而变，长了就在那儿截住——期望屏照同一条截。
        let (width, _) = scene::sequence("config-c").size;
        let head = "✓ 已生成灰阶测试图";
        let mut room = usize::from(width - 3) - usize::from(crate::wrap::width(head));
        let tail: String = format!(
            "（1264x1680）：写到 {}",
            scene.session.home_shown(&landed[0])
        )
        .chars()
        .take_while(|glyph| {
            let cells = usize::from(crate::wrap::width(&glyph.to_string()));
            let fits = cells <= room;
            room = room.saturating_sub(cells);
            fits
        })
        .collect();
        let said = [
            Segment::plain(" "),
            Segment::new(head, Look::kind(Kind::Done).bold()),
            Segment::plain(tail),
        ];
        let buffer = painted(&scene, &running, scene::sequence("config-c").size);
        assert_no_background(&buffer);
        assert_same_cells(&buffer, &design::sequence("config-c").instead(35, &said));
    }

    /// 在配置视图那一景上逐个喂键（`here` 是灰阶测试图的落点），回最后一下的去留。
    fn tap_all(
        scene: &mut Scene,
        running: &mut Running,
        here: &std::path::Path,
        keys: impl IntoIterator<Item = Key>,
    ) -> Exit {
        let now = scene.now();
        let mut exit = Exit::Stay;
        for key in keys {
            exit = super::input(
                &mut scene.session,
                running,
                &scene.presets,
                here,
                now,
                Window {
                    cols: 120,
                    rows: 36,
                },
                Input::Key(key),
            );
        }
        exit
    }

    /// 屏底此刻那一句回话的字（没有就是空串）。
    fn replied(scene: &Scene) -> String {
        scene
            .session
            .views
            .reply(scene.now())
            .map(|segments| segments.iter().map(|one| one.text.as_str()).collect())
            .unwrap_or_default()
    }

    /// **灰阶测试图写不出去时说得清，会话照开着**（会话批 13 号票第五条，经新输入入口）。
    ///
    /// 逼出来的是「父目录建不了」那一种：图该落的那个目录的位置摆一个**文件**，
    /// 库那一侧 `create_dir_all` 当场失败。说的是库那一侧的原话（哪一步、哪条路径），
    /// 这一层不另编一份；三组设置一格没动，下一个键照按。
    #[test]
    fn a_chart_that_cannot_be_written_says_so_and_the_session_stays_open() {
        let mut scene = Scene::named("config");
        let mut running = Running::default();
        let here = charts_land_in(&scene).join("这是个文件");
        std::fs::write(&here, "不是目录").expect("写得出那个文件");
        let before = (scene.session.device.clone(), scene.session.taste.clone());

        let exit = tap_all(&mut scene, &mut running, &here, [Key::Char('c')]);

        assert_eq!(exit, Exit::Stay, "写不出去把会话带走了");
        let said = replied(&scene);
        assert!(said.contains("灰阶测试图"), "{said}");
        assert!(said.contains("这是个文件"), "{said}");
        assert_eq!(
            (scene.session.device.clone(), scene.session.taste.clone()),
            before,
            "写不出去却动了设置"
        );
        assert_eq!(
            tap_all(&mut scene, &mut running, &here, [Key::Char('j')]),
            Exit::Stay
        );
    }

    /// **命令行上 `--preset` 拿到的，与会话里存出去的是同一份**（会话批 12 号票第五条，
    /// 经新输入入口）：问的是**接头**——会话写出去的那份文件，`Cli` 那一路读得懂，
    /// 而且合出来的 `Request` 与会话拼的一样（型号不进预设，命令行那一头照样点名）。
    #[test]
    fn a_preset_saved_in_the_session_is_the_one_the_command_line_takes() {
        let mut scene = Scene::named("config");
        let mut running = Running::default();
        let here = charts_land_in(&scene);
        scene.session.taste.filter = Some(tonefit::Filter::Hamming);
        scene.session.taste.envelope = Some(true);

        tap_all(
            &mut scene,
            &mut running,
            &here,
            [Key::Char('p'), Key::Char('G'), Key::Enter]
                .into_iter()
                .chain("插图".chars().map(Key::Char))
                .chain([Key::Enter]),
        );

        let file = scene.presets.path().expect("说得出位置").to_path_buf();
        let text = std::fs::read_to_string(&file).expect("读得出来");
        let read_back = crate::preset::read(&text, "插图").expect("命令行这一路读得懂");
        assert_eq!(read_back, scene.session.preset_to_store());

        let device = scene
            .session
            .device
            .profile
            .clone()
            .expect("这一景挑了型号");
        let asked = scene
            .session
            .request(tonefit::Mode::Process)
            .expect("会话拼得出来");
        let out = asked.output_root.display().to_string();
        let mut line = vec![
            "tonefit".to_owned(),
            "--profile".to_owned(),
            device,
            "--out".to_owned(),
            out,
            "--preset".to_owned(),
            "插图".to_owned(),
        ];
        line.extend(asked.inputs.iter().map(|path| path.display().to_string()));
        let command_line = <crate::Cli as clap::Parser>::try_parse_from(line)
            .expect("命令行读得懂")
            .request(&read_back)
            .expect("拼得出来");
        assert_eq!(asked.profile, command_line.profile);
        assert_eq!(asked.filter, command_line.filter);
        assert_eq!(asked.envelope, command_line.envelope);
        assert_eq!(asked.inputs, command_line.inputs);
    }

    /// **那一份在两下 `dd` 之间被别处删掉了：说得清，不崩，那一栏还开着**
    /// （会话批 12 号票第六条，经新输入入口）。报的是库那一侧的原话：哪一份不在、有的是哪几份。
    #[test]
    fn erasing_a_preset_that_is_no_longer_on_disk_says_so_and_stays_open() {
        let mut scene = Scene::named("config");
        let mut running = Running::default();
        let here = charts_land_in(&scene);
        let file = scene.presets.path().expect("说得出位置").to_path_buf();

        tap_all(
            &mut scene,
            &mut running,
            &here,
            [Key::Char('p'), Key::Char('d'), Key::Char('d')],
        );
        assert_eq!(
            scene.session.views.config.armed_delete.as_deref(),
            Some("漫画")
        );
        // 两下之间，别处把那份文件换成了只剩另一份。
        std::fs::write(&file, "[preset.\"画集\".taste]\nenvelope = true\n").expect("写得出来");
        let exit = tap_all(
            &mut scene,
            &mut running,
            &here,
            [Key::Char('d'), Key::Char('d')],
        );

        assert_eq!(exit, Exit::Stay);
        let said = replied(&scene);
        assert!(said.contains("漫画"), "没说清点的是哪一份：{said}");
        assert!(said.contains("画集"), "没说有的是哪几份：{said}");
        assert!(scene.session.views.config.picker, "说完把那一栏关掉了");
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
    use crate::session::live;

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
        // 左右方向键在按键表上没有主（左右是 `h`／`l`），同样原地放过。
        assert_eq!(translate(&press(KeyCode::Left)), None);
        assert_eq!(translate(&press(KeyCode::BackTab)), None);
    }
}
