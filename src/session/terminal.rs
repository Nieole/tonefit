//! 终端那一侧：进出终端、把键码翻译成会话认得的键、在两者之间转一个循环。
//!
//! **本仓库唯一一处认得 crossterm 键码的地方**（见 [`translate`]），也是唯一一处
//! 握着终端的地方（见 [`Screen`]）。状态机、边跑边攒的那一份、起线程与逐层补全
//! 都在 [`super`] 的另外四个模块里，摆在 `tui` 特性**外面**——分界与理由见
//! `super` 的模块文档《终端库在哪一半》。
//!
//! 除了那三件事，这一层还担着**状态机够不着的那几支**（见 [`press`]）：
//! 起一趟、按停、展开、读写盘上那份预设、把标定图交给库里第三个 seam。
//! 那几支要的是那一趟、那块盘、那个库与画法（[`super::draw::opens_at`]），
//! 而状态机四样都不碰。

use std::io::{IsTerminal, Stderr, stderr};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

use super::draw;
use super::live::Resuming;
use super::run::Running;
use super::state::{Action, Exit, Expansion, Key, Picker, Session};
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
/// 全部成功 `0`、有卷被隔离 `2`、有卷没做成 `3`、拒绝执行 `1`。一趟都没跑过是 `0`。
///
/// 退出前把那份报告照原格式印到 **stdout**：会话整个画在 stderr 上，
/// `tonefit > 报告.txt` 因此仍然成立。
pub fn enter() -> Result<u8> {
    if !stderr().is_terminal() {
        return Err(no_terminal_error());
    }
    let mut screen = Screen::open()?;
    let mut session = Session::new();
    // 跑着的那一趟**一定**要收手：`?` 提前返回、恐慌展开，走的都是 `Running` 的 `Drop`。
    // 终端同理，走 `Screen` 的 `Drop`。
    let mut running = Running::default();
    // 预设文件那一份。**找不到用户配置目录不在这里拦**：那台机器上会话照进，
    // 只是按下 `p` 那一刻它说得出为什么（见 `preset::Presets`）。
    let presets = Presets::found();
    // 标定图落在会话是从哪儿敲起来的那个目录里（见 [`chart_file`]）。**一次问出来**：
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
        {
            // 借着锁画：画完当场还回去，计算线程最多等一帧的功夫（见 `Running::live`）。
            let live = running.live();
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
        // 那一趟停在决策点上了：会话跟着停下来等人答话（`p1-session/14`）。
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
/// [起一趟](Action::Start)、[按停](Action::Stop)、[展开](Action::Expand)与
/// [换一卷](Action::Turn)，加上预设那四支（[列出来](Action::Pick)、
/// [套用](Action::Take)、[存下来](Action::Store)、[删掉](Action::Erase)）
/// 与[出标定图](Action::Chart)。
/// 起线程、拼 `Request`、把观察者接上去、把按到的那一级送到计算线程上、
/// 从攒着的那份报告上数出有几卷与那一卷落在第几行、读写用户配置目录下那份 TOML、
/// 把标定图交给库里那第三个 seam，
/// 都在这一层——状态机一个终端都不碰、不起线程，也读不到那一趟攒下来的东西与盘上的东西。
/// 拼不出 `Request` 的那两种（型号没挑、输出根没填）当场说一句，会话原地不动。
///
/// `here` 是标定图落在哪个目录下（见 [`chart_file`]）：真会话里是进程的当前目录，
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
        // 按停：状态机把闩升一级（收尾 → 中止，ADR 0013），这一层把升到的那一级
        // 交给跑着的那一趟。两处记的是同一个字，出处只有状态机那一份——
        // 这里读的就是它刚升完的结果，不自己再算一次。
        Action::Stop => {
            let exit = session.act(action);
            running.stop(session.stopping());
            exit
        }
        // 决策点上答话：状态机把会话放回「跑着」那一副，这一层把那个字交给停在
        // 决策点上的那条线程。与按停同一条分工——认键在那边，碰线程在这边。
        // **两处记的是同一个字**，而它就在这个动作里带着：决策点回的是当场那个字、
        // 不是闩（ADR 0012 决定第 2 条），因此这里不去问状态机再算一次。
        // 它管几卷（「剩下的卷都这样」）同样带在动作里，摆到那道闸的默认答案上去
        // （`Running::decide`）——那一格也不是闩。
        Action::Answer(said, reach) => {
            let exit = session.act(action);
            running.decide(said, reach);
            exit
        }
        // 展开与换一卷：要读那一趟攒下来的报告（有几卷、那一卷落在第几行），
        // 而状态机读不到它。收起（`Action::Collapse`）不在这里——它不必读报告。
        Action::Expand | Action::Turn(_) => {
            expand(session, running, action);
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
        // 出标定图：画图与落盘整件事在库里那第三个 seam 上，而状态机碰不到盘。
        // 与预设那三支同一条分法。
        Action::Chart => {
            write_chart(session, here);
            Exit::Stay
        }
        other => session.act(other),
    }
}

/// 这一趟**在决策点上等不等人**，以及它真正走的是哪一种模式（ADR 0012 决定第 3 条）。
///
/// **试算一律续做，点名了几个路径都一样**（决定第 3 条，`volume-discovery/07`）。
/// 那一趟因此改走 `Mode::Process`——参照要留着（决定第 5 条：试算走 `Retention::Keep`），
/// 答继续时第一遍才不必重算。「只算不写」在那条路上重述为**不写输出**：
/// 越过预算的页仍建溢写临时文件，运行结束即收走。
///
/// **不按卷数分岔，也不按点名了几个路径分岔。** 从前这里判的是
/// `inputs.len() == 1`，而那判的是「点名了一个**路径**」——发现落地之后
/// （`volume-discovery/03`：`inputs` 的语义是「一批**在里面找卷的地方**」），
/// 一个路径常常就是几十卷，这两件事早已脱钩。决定第 1 条那条内存理由拦的是
/// 「一次押住**全部卷**的参照」，而决策点本来就是逐卷的：逐卷停下来问，
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

/// **出标定图**：按设备层那块面板画一张，写到 [`chart_file`] 点的那个文件上。
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

/// 标定图落在哪个文件上：`here` 下面一个**照 profile 取名**的 PNG。
///
/// **不落在输出根下面。** 那是被处理的页的去处，而标定图是量具——
/// 两者走的不是同一条路（`lib.rs` 的第三个 seam）；何况按 `c` 那一刻输出根多半还空着，
/// 而「先填一个输出根才出得了标定图」把设备层的事拴在了范围层上。
/// 落在会话是从哪儿敲起来的那个目录里：那是用户此刻人在的地方，图出来就在手边。
///
/// 名字里带着**型号与灰阶数**，因为图跟着这两项变（灰阶数决定排几条阶梯）：
/// 换一台设备出的是另一张图，不该盖掉上一张。同一个 profile 再按一次写的是同样的字节——
/// 标定图不带记录、不带时间戳，重写一遍等于没变（见 `crate::calibrate`）。
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

/// 展开一卷的逐页，或者换到下一卷。
///
/// **展开那一下从第一卷起，且报告从头画**（`from` 取零）：抬头那几行
/// （profile、适配方式、裁边、拆分）跟着跑的时候早滚出了格子（停车场 Q64），
/// 而展开正是把它们找回来的那一下。往后翻一卷则把视口对到那一卷的抬头上
/// （[`super::draw::opens_at`]）——不对准的话，换没换成屏上第一眼看不出来。
///
/// 一卷都没有就说一句、不进展开态：展开的是**报告上的一卷**，
/// 而这一趟还没跑过或者第一卷还没跑完时，那样东西根本不在。
fn expand(session: &mut Session, running: &Running, action: Action) {
    let Some(live) = running.live() else {
        session.complain("还没跑过：先按 t 试算或 x 执行，报告出来了才展得开".to_owned());
        return;
    };
    let volumes = live.report().volumes.len();
    if volumes == 0 {
        session.complain("报告里还没有卷：一卷跑完才有它的逐页那几行".to_owned());
        return;
    }
    let opened = match (action, session.expansion()) {
        // 换一卷：两头都转一圈（`Expansion::next`），视口对到那一卷的抬头上。
        (Action::Turn(step), Some(expansion)) => {
            let next = expansion.next(step);
            Expansion::new(next, volumes, draw::opens_at(&live, next))
        }
        _ => Expansion::new(0, volumes, 0),
    };
    drop(live);
    session.expand(opened);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    // 两个兄弟模块的**名字**（`super::*` 带进来的是它们里面的东西，不是模块本身）：
    // 用例要按名字点它们里面的取值与夹具。
    use crate::session::{live, state};

    /// 一份**指向临时目录**的预设文件。
    ///
    /// 用例一律用它：真会话读写的是用户配置目录下那一份，而那是用户自己的东西——
    /// 用例不该读它，更不该写它。位置由 [`Presets::at`] 点名，因此不必去改进程的环境变量
    /// （`tests/preset.rs` 说过为什么不改）。
    fn presets(space: &tempfile::TempDir) -> Presets {
        Presets::at(space.path().join("tonefit").join("presets.toml"))
    }

    /// 按一个**不出标定图**的键。
    ///
    /// [`press`] 收的那个「图落在哪个目录下」只有 `c` 那一个键用得到，而这几条用例
    /// 一个都不按它。去处仍旧点在**临时目录**里（那份预设文件的上一层，见 [`presets`]）：
    /// 万一往后有人往这几条里加一下 `c`，写出去的东西也落在那儿，
    /// 不会掉进跑用例的那个目录——相对路径会。
    ///
    /// 出标定图那两条不走这里：它们要说的正是「写到哪儿了、写不出去时怎么办」，
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

    /// **按停那个键真的走到了跑着的那一趟身上。**
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

        // 一次：收尾。两头记的是同一个字。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(session.stopping(), tonefit::Instruction::Finish);
        assert_eq!(running.pressed(), tonefit::Instruction::Finish);

        // 再一次：中止。
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

    /// **答话那个键真的走到了停在决策点上的那条线程身上**（`p1-session/14`）。
    ///
    /// 这一条走的是整条路：按 `t` 起一趟（试算，因此 [`resuming`] 把它改成续做的那一趟）→
    /// 那条线程停在决策点上 → 会话跟着换一副样子 → 按 `s` 答收尾 → 那条线程接着跑完。
    /// 两头各自有用例（状态机那边 `deciding_action`，闸那边
    /// `answering_finish_at_the_decision_point_writes_nothing_and_still_reports_the_volume`），
    /// **接头处只有这一条**——而接头处正是本层唯一做的事。
    ///
    /// **等答话时会话不冻屏**由它的形状说出来：那条线程停在闸上，而这一头照旧收键、
    /// 照旧问得动 [`Session::mode`]。等的那一步走的是「转到条件成立为止」，
    /// 不是 sleep 撞运气（见 `Running::deciding`）。
    ///
    /// 不开终端：[`press`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn answering_at_the_decision_point_reaches_the_thread_waiting_there() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        // 一页加一个透传文件（见 [`super::live::fixture::a_real_volume`]）：页非有不可，
        // 一页都没有的东西不是卷，那条线程根本走不到决策点。
        let volume = crate::session::live::fixture::a_real_volume(space.path(), "卷一");
        let out = space.path().join("出");

        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(out.clone());
        session.scope.volumes.push(state::Picked {
            path: volume,
            on: true,
        });
        let mut running = Running::default();
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&space);

        // 按 `t`：试算，因此这一趟改走 `Mode::Process` 并在决策点上等人。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('t')),
            Exit::Stay
        );
        assert!(matches!(session.mode(), state::Mode::Running(_)));

        // 那条线程走到决策点上停住；会话每帧问一次，跟着换一副样子（见 [`drive`]）。
        while !running.deciding() {
            std::thread::yield_now();
        }
        session.at_the_decision_point(running.deciding());
        assert!(session.deciding(), "那一趟停住了，会话却没跟着换一副样子");
        assert!(
            running.live().expect("跑过一趟").summarized().is_some(),
            "决策点上没有报告可画"
        );

        // 等答话时会话仍旧收键：按一个没有意义的键，它照旧原地不动、不退出。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('e')),
            Exit::Stay
        );
        assert!(session.deciding(), "按了一个没有意义的键就走掉了");

        // 按 `s` 答收尾：这一卷一个字节都不写，那条线程接着跑完。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('s')),
            Exit::Stay
        );
        assert!(!session.deciding(), "答完话会话还停在决策点上");
        while !running.reap() {
            std::thread::yield_now();
        }
        session.run_finished();

        assert_eq!(
            session.mode(),
            &state::Mode::Browsing,
            "收场之后配置还改不动"
        );
        assert!(!out.exists(), "答了收尾，输出根却被建了出来");
        let live = running.live().expect("跑过一趟");
        assert_eq!(live.report().volumes.len(), 1, "答收尾把报告也一起停掉了");
        assert_eq!(
            live.decided(),
            Some(tonefit::Instruction::Finish),
            "答的那个字没记下来"
        );
    }

    /// **试算一律续做，点名了几个路径都一样**（ADR 0012 决定第 3、5 条，
    /// `volume-discovery/07` 票面）。
    ///
    /// 各情形问的是同一个函数交出来的那两样：这一趟**真走**哪一种模式、
    /// 它**在决策点上等不等人**。
    ///
    /// 试算那几支两样都变：模式从 `DryRun` 换成 `Process`（参照要留着，
    /// 答继续时第一遍才不必重算），并且等人。**这一处不再数点名了几个路径**——
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

        // 试算：改走 Process，并且在决策点上等人——点名一个路径与点名两个一个待遇。
        for request in [one(RunMode::DryRun), many(RunMode::DryRun)] {
            let inputs = request.inputs.len();
            let (request, resumes) = resuming(request);
            assert_eq!(resumes, Resuming::Waits, "试算没续做（{inputs} 个路径）");
            assert_eq!(
                request.mode,
                RunMode::Process,
                "续做那一趟得留参照（ADR 0012 决定第 5 条）"
            );
        }

        // 执行：一格不动，几个路径都一样——按 x 的时候用户已经拿过主意了。
        for request in [one(RunMode::Process), many(RunMode::Process)] {
            let inputs = request.inputs.len();
            let (request, resumes) = resuming(request);
            assert_eq!(
                resumes,
                Resuming::GoesOn,
                "执行那一趟停下来等人了（{inputs} 个路径）"
            );
            assert_eq!(request.mode, RunMode::Process);
        }

        // 一个卷都没勾：范围为空由库那一侧当场拒掉——那一趟一条事件都不发，
        // 决策点根本到不了，等不等人因此不影响任何事（见 `Running::start` 起的那道闸）。
        let (request, resumes) = resuming(Request {
            inputs: Vec::new(),
            ..live::fixture::request(RunMode::DryRun)
        });
        assert_eq!(resumes, Resuming::Waits);
        assert_eq!(request.mode, RunMode::Process);
    }

    /// **「剩下的卷都这样」那个键真的走到了停在决策点上的那条线程身上**
    /// （`volume-discovery/07`，spec 的 story 13）。
    ///
    /// 与答话那一条同一个位置：接头处是本层唯一做的事——把状态机认出来的那个字
    /// **连同它管几卷**交给 [`Running::decide`]。两头各自有用例（状态机那边
    /// `which_keys_do_what_in_which_state`，闸那边
    /// `answering_for_the_rest_once_stops_the_asking_and_leaves_the_latch_alone`）。
    ///
    /// 走的是整条路：按 `t` 起一趟**两个卷**的试算 → 停在头一卷的决策点上 →
    /// 按 `a` → 那条线程一路把两卷都做完，一次都不再停。
    #[test]
    fn pressing_the_rest_too_reaches_the_thread_waiting_at_the_decision_point() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let out = space.path().join("出");

        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        session.scope.out = Some(out.clone());
        for name in ["卷一", "卷二"] {
            session.scope.volumes.push(state::Picked {
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

        // 按 `a`：这一卷接着做，剩下的卷都这样。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('a')),
            Exit::Stay
        );
        assert!(!session.deciding(), "答完话会话还停在决策点上");
        while !running.reap() {
            // 真停下来的话当场红，而不是挂在那儿等一个不会来的人。
            assert!(
                !running.deciding(),
                "答过「剩下的卷都这样」，它却又停下来问了"
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

    /// **展开那个键找那一趟要报告，而报告不在时它说一句、不进展开态。**
    ///
    /// 接头处与按停那一条同一个位置：状态机读不到那一趟攒下来的东西，
    /// 「有几卷」「那一卷落在第几行」两个数都由本层从 [`Running::live`] 上数出来。
    /// 不开终端——[`press`] 收的是 `&mut Session` 与 `&mut Running`。
    #[test]
    fn expanding_asks_the_run_for_its_report_and_says_so_when_there_is_none() {
        let mut session = Session::new();
        let mut running = Running::default();
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        // 这一条一个预设键都不按（见 [`presets`]）。
        let nowhere = presets(&workspace);

        // 一趟都没跑过：说一句，会话原地不动。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Char('e')),
            Exit::Stay
        );
        assert!(session.expansion().is_none(), "没有报告却展开了");
        let said = session.notice().expect("该说一句").to_owned();
        assert!(said.contains("还没跑过"), "{said}");

        // 跑过一趟、报告里有两卷：展开落在第一卷上，报告从头画（抬头那几行回来了）。
        // 两个真跑得动的卷（见 [`live::fixture::a_real_volume`]）：这一条要问的
        // （几卷、落在第几行、转不转得回去）一件都不少。
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
            // 两卷，因此不续做：这一条问的是展开，与决策点无关（见 [`resuming`]）。
            Resuming::GoesOn,
        );
        while !running.reap() {
            std::thread::yield_now();
        }
        session.run_finished();
        tap(&mut session, &mut running, &nowhere, Key::Char('e'));
        let expansion = session.expansion().expect("该展开了");
        assert_eq!(expansion.volume, 0);
        assert_eq!(expansion.volumes, 2);
        assert_eq!(expansion.from, 0, "展开那一下该从报告头一行画起");
        assert!(session.notice().is_none(), "展开之后上一句话没抹掉");

        // `⇥` 往后一卷，视口对到那一卷的抬头上；再按一次转一圈回到第一卷。
        tap(&mut session, &mut running, &nowhere, Key::Tab);
        let second = session.expansion().expect("还展开着").clone();
        assert_eq!(second.volume, 1);
        assert!(second.from > 0, "换过一卷之后视口没对上去");
        tap(&mut session, &mut running, &nowhere, Key::Tab);
        assert_eq!(session.expansion().expect("还展开着").volume, 0, "没转回去");

        // `⇧⇥` 是另一头：往前一卷，同样转得回去。**两头都有**，
        // 因为几十卷的一趟里往回看一卷不该按二十九下（票面：选中一卷）。
        tap(&mut session, &mut running, &nowhere, Key::BackTab);
        let back = session.expansion().expect("还展开着").clone();
        assert_eq!(back.volume, 1, "⇧⇥ 没往前转");
        assert_eq!(back.from, second.from, "两头转到同一卷，落位却不一样");
        tap(&mut session, &mut running, &nowhere, Key::BackTab);
        assert_eq!(session.expansion().expect("还展开着").volume, 0);

        // 收起：一个键回到配置，展开态没了。
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Esc),
            Exit::Stay
        );
        assert!(session.expansion().is_none());
    }

    /// **停在设备层上按一个键，标定图就落在盘上**（13 号票第一、二、三条）。
    ///
    /// 接头处在这一层：状态机派得出[出标定图](Action::Chart)那个动作，落盘整件事在库里
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
        session.focus_on(state::Field::Profile);

        // 型号还没挑：说一句，会话原地不动，那个目录里一个文件都没多。
        assert_eq!(
            press(&mut session, &mut running, &nowhere, &here, Key::Char('c')),
            Exit::Stay
        );
        let said = session.notice().expect("该说一句").to_owned();
        assert!(said.contains("先挑型号"), "{said}");
        assert_eq!(
            std::fs::read_dir(&here).expect("读得出那个目录").count(),
            0,
            "型号没挑却写出了东西"
        );

        // 挑一个型号、覆盖一次灰阶数，再按一次：图落在那个目录下。
        session.device.profile = Some("boox-poke6".to_owned());
        session.device.gray_levels = Some(8);
        press(&mut session, &mut running, &nowhere, &here, Key::Char('c'));

        let written: Vec<PathBuf> = std::fs::read_dir(&here)
            .expect("读得出那个目录")
            .map(|entry| entry.expect("读得出那一条").path())
            .collect();
        assert_eq!(written.len(), 1, "{written:?}");
        let chart = &written[0];
        // 名字里带着型号与灰阶数：换一台设备出的是另一张图，不该盖掉上一张。
        let name = chart.file_name().expect("有名字").to_string_lossy();
        assert!(name.contains("boox-poke6") && name.contains("8"), "{name}");
        assert!(name.ends_with(".png"), "{name}");
        // 与库直接写出来的逐字节相同——会话一格像素都没自己拼。
        let straight = space.path().join("库自己写的.png");
        tonefit::write_calibration_chart(
            &session.chart_profile().expect("设备层填齐了"),
            &straight,
        )
        .expect("库写得出来");
        assert_eq!(
            std::fs::read(chart).expect("读得出图"),
            std::fs::read(&straight).expect("读得出库写的那张"),
            "会话写出来的图与库直接写的不一样"
        );
        // 屏上说清图在哪儿，以及此刻要做对的那一件事。
        let said = session.notice().expect("出完图要说一句").to_owned();
        assert!(said.contains(&*name), "{said}");
        assert!(said.contains("原尺寸"), "{said}");
        // 会话还在浏览：出图不改变它此刻在做什么。
        assert_eq!(session.mode(), &state::Mode::Browsing);
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
        let said = session.notice().expect("写不出去要说一句").to_owned();
        assert!(said.contains("标定图"), "{said}");
        assert!(said.contains("这是个文件"), "{said}");
        // 三层一格没动，会话还在浏览：下一个键照按。
        assert_eq!(session.mode(), &state::Mode::Browsing);
        assert_eq!(
            tap(&mut session, &mut running, &nowhere, Key::Down),
            Exit::Stay
        );
    }

    /// **存出去再套回来，两层逐格相同，而范围层一格没动**（本票的四条验收）。
    ///
    /// 走的是真文件：`p` 列出来、末行 `⏎` 打一个名字存下去、改乱两层、再 `p` 套回来。
    /// 盘在临时目录下（见 [`presets`]），一个用户的东西都不碰。
    ///
    /// 「没说」与「说了默认值」的差别一并钉在这里：存之前把适配方式转到**恰好等于默认值**
    /// 的那一档上，套回来之后它仍是「说了」而不是「没说」（停车场 Q58）。
    #[test]
    fn what_the_session_stores_is_what_it_takes_back() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let presets = presets(&space);
        let mut session = Session::new();
        let mut running = Running::default();
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.volumes.push(state::Picked {
            path: PathBuf::from("库/卷一"),
            on: true,
        });
        // 设备层挑一个型号，口味层点两项：一项与默认值不同，一项**恰好等于**默认值。
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
        let said = session.notice().expect("存完要说一句").to_owned();
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
        assert_eq!(session.scope, scope, "套用预设动了范围层");
        assert_eq!(
            session.taste.fit,
            Some(tonefit::FitMode::default()),
            "「说了一个恰好等于默认值的值」套回来变成了「没说」"
        );
        // 套完回到浏览，说的那句话里带着「范围层没动」。
        let said = session.notice().expect("套完要说一句").to_owned();
        assert!(said.contains("范围层"), "{said}");
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
        session.taste.per_page = Some(true);
        session.scope.out = Some(PathBuf::from("出"));
        session.scope.volumes.push(state::Picked {
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
            session.notice().is_some_and(|said| said.contains("存好了")),
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
        assert_eq!(asked.per_page, command_line.per_page);
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
                       [preset.\"画集\".taste]\nper-page = true\n";
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
        let said = session.notice().expect("要说一句").to_owned();
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
            session
                .notice()
                .is_some_and(|said| said.contains("画集") && said.contains("再按一次")),
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
            session.notice().is_some_and(|said| said.contains("存好了")),
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
        let said = session.notice().expect("要说一句").to_owned();
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
            session
                .notice()
                .is_some_and(|said| said.contains("画集") && said.contains("再按一次")),
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
            session.notice().is_some_and(|said| said.contains("删掉了")),
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
        let said = session.notice().expect("要说一句").to_owned();
        assert!(!said.contains("再按一次"), "那一问还摆在屏上：{said}");

        // 这一下 `d` 是**重新问一句**，不是删。
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        assert!(
            session
                .notice()
                .is_some_and(|said| said.contains("再按一次")),
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
        std::fs::write(&file, "[preset.\"画集\".taste]\nper-page = true\n").expect("写得出来");
        tap(&mut session, &mut running, &presets, Key::Char('d'));
        tap(&mut session, &mut running, &presets, Key::Char('d'));

        let said = session.notice().expect("要说一句").to_owned();
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

        let said = session.notice().expect("要说一句").to_owned();
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
}
