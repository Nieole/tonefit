//! 会话：**不带任何参数敲 `tonefit`** 进到的那一段（`CONTEXT.md` 的《会话》）。
//!
//! 它是 `run` 之上的第二个薄层，与命令行同级：不绕过 seam，也不多一条管线
//! （spec 的《Seam》）。带参数那一路一字不改——无参数在 clap **之前**就被截住
//! （见 [`crate::without_arguments`]），带参数的那一趟根本走不到这里。
//!
//! # 画在 stderr
//!
//! 与进度条同一个去处。**stdout 仍然只装报告**，`tonefit > 报告.txt` 因此仍然成立
//! （退出会话时把报告印到 stdout 归 `p1-session/09`，本票一个字节都不往 stdout 写）。
//!
//! stderr 不是终端时（CI、`2>日志`）不进会话，也不崩在 raw mode 里：
//! 印一条说得清的话，**连同 clap 那条必填项用法提示**，退出码 `1`
//! （见 [`no_terminal_error`]）。
//!
//! # 三层与终端分开
//!
//! 状态机在 [`state`]，一个终端都不碰；边跑边攒的那一份在 [`live`]，同样不碰；
//! 逐层补全在 [`complete`]；起线程在 [`run`]；画法在 [`draw`]。
//! 这一层剩下的只有三件事：进出终端、把键码翻译成 [`state::Key`]、在两者之间转一个循环。
//! spec 的 story 44（会话的状态机脱离终端可测）因此不必靠自觉——
//! 那几个模块的用例连终端都开不起来。
//!
//! # 一趟跑起来之后
//!
//! [`tonefit::run`] 一进去就跑到底，会话这一头还得接着画、接着认键，因此它在
//! **另一条线程**上（见 [`run::Running`]）。循环于是不再是「等一个键」，而是
//! 「等一个键，最多等 [`TICK`] 那么久」——没等到就画下一帧，跑着的那一趟因此看得见在动。

mod complete;
mod draw;
mod live;
mod run;
mod state;

use std::io::{IsTerminal, Stderr, stderr};
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

use run::Running;
use state::{Action, Exit, Expansion, Key, Session};

/// 没等到按键时隔多久重画一帧。
///
/// 跑着的那一趟就是靠它动起来的：事件从计算线程上折进 [`live::Live`]，而把它画出来的
/// 只有这一条循环。取八十毫秒——比人眼看得出的停顿短，又不至于把一趟长任务的
/// CPU 花在画横条上。
const TICK: Duration = Duration::from_millis(80);

/// 进会话，跑到用户退出为止。
///
/// 出的是**最后那一趟**的退出码，与命令行那一路同一套（见 [`live::Live::exit_code`]）：
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
    let outcome = drive(&mut screen, &mut session, &mut running);
    drop(screen);
    outcome?;
    // 报告印在终端还回去**之后**：印进 alternate screen 的话它会随着那一屏一起消失。
    if let Some(report) = running.report() {
        print!("{report}");
    }
    Ok(running.exit_code())
}

/// 画一屏、等一个键（最多等 [`TICK`]）、做掉它，直到用户退出。
fn drive(screen: &mut Screen, session: &mut Session, running: &mut Running) -> Result<()> {
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
                && press(session, running, key) == Exit::Leave
            {
                running.leave();
                return Ok(());
            }
        }
        // 那一趟跑完了：配置又改得动。
        if running.reap() {
            session.run_finished();
        }
    }
}

/// 把一个键交给会话。
///
/// **只有够得着那一趟的那几支不走 [`Session::press`]**：[起一趟](Action::Start)、
/// [按停](Action::Stop)、[展开](Action::Expand)与[换一卷](Action::Turn)。
/// 起线程、拼 `Request`、把观察者接上去、把按到的那一级送到计算线程上、
/// 从攒着的那份报告上数出有几卷与那一卷落在第几行，都在这一层——
/// 状态机一个终端都不碰、不起线程，也读不到那一趟攒下来的东西。
/// 拼不出 `Request` 的那两种（型号没挑、输出根没填）当场说一句，会话原地不动。
fn press(session: &mut Session, running: &mut Running, key: Key) -> Exit {
    // 问一次就够：这几支之外的原样交回状态机，不让它再问一遍。
    let action = session.action(key);
    match action {
        Action::Start(mode) => {
            match session.request(mode) {
                Ok(request) => {
                    running.start(request);
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
        // 展开与换一卷：要读那一趟攒下来的报告（有几卷、那一卷落在第几行），
        // 而状态机读不到它。收起（`Action::Collapse`）不在这里——它不必读报告。
        Action::Expand | Action::Turn(_) => {
            expand(session, running, action);
            Exit::Stay
        }
        other => session.act(other),
    }
}

/// 展开一卷的逐页，或者换到下一卷。
///
/// **展开那一下从第一卷起，且报告从头画**（`from` 取零）：抬头那几行
/// （profile、适配方式、裁边、拆分）跟着跑的时候早滚出了格子（停车场 Q64），
/// 而展开正是把它们找回来的那一下。往后翻一卷则把视口对到那一卷的抬头上
/// （[`draw::opens_at`]）——不对准的话，换没换成屏上第一眼看不出来。
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
        session.run_started();

        // 一次：收尾。两头记的是同一个字。
        assert_eq!(
            press(&mut session, &mut running, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(session.stopping(), tonefit::Instruction::Finish);
        assert_eq!(running.pressed(), tonefit::Instruction::Finish);

        // 再一次：中止。
        assert_eq!(
            press(&mut session, &mut running, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(session.stopping(), tonefit::Instruction::Abort);
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 第三次起那个键没有意义，闩两头都不再动。
        assert_eq!(
            press(&mut session, &mut running, Key::Char('s')),
            Exit::Stay
        );
        assert_eq!(running.pressed(), tonefit::Instruction::Abort);

        // 浏览时按它什么都不发生：还没有东西可停。
        let mut idle = Session::new();
        let mut nothing = Running::default();
        assert_eq!(press(&mut idle, &mut nothing, Key::Char('s')), Exit::Stay);
        assert_eq!(nothing.pressed(), tonefit::Instruction::Continue);
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

        // 一趟都没跑过：说一句，会话原地不动。
        assert_eq!(
            press(&mut session, &mut running, Key::Char('e')),
            Exit::Stay
        );
        assert!(session.expansion().is_none(), "没有报告却展开了");
        let said = session.notice().expect("该说一句").to_owned();
        assert!(said.contains("还没跑过"), "{said}");

        // 跑过一趟、报告里有两卷：展开落在第一卷上，报告从头画（抬头那几行回来了）。
        // 两个只装透传文件的卷：一页都没有，因此不必在用例里造图片，
        // 而这一条要问的（几卷、落在第几行、转不转得回去）一件都不少。
        let workspace = tempfile::tempdir().expect("建得出临时目录");
        let inputs: Vec<PathBuf> = ["卷一", "卷二"]
            .iter()
            .map(|name| {
                let volume = workspace.path().join(name);
                std::fs::create_dir_all(&volume).expect("建得出卷");
                std::fs::write(volume.join("说明.txt"), "透传").expect("写得出成员");
                volume
            })
            .collect();
        running.start(tonefit::Request {
            inputs,
            output_root: workspace.path().join("出"),
            ..live::fixture::request(tonefit::Mode::DryRun)
        });
        while !running.reap() {
            std::thread::yield_now();
        }
        session.run_finished();
        press(&mut session, &mut running, Key::Char('e'));
        let expansion = session.expansion().expect("该展开了");
        assert_eq!(expansion.volume, 0);
        assert_eq!(expansion.volumes, 2);
        assert_eq!(expansion.from, 0, "展开那一下该从报告头一行画起");
        assert!(session.notice().is_none(), "展开之后上一句话没抹掉");

        // `⇥` 往后一卷，视口对到那一卷的抬头上；再按一次转一圈回到第一卷。
        press(&mut session, &mut running, Key::Tab);
        let second = session.expansion().expect("还展开着").clone();
        assert_eq!(second.volume, 1);
        assert!(second.from > 0, "换过一卷之后视口没对上去");
        press(&mut session, &mut running, Key::Tab);
        assert_eq!(session.expansion().expect("还展开着").volume, 0, "没转回去");

        // `⇧⇥` 是另一头：往前一卷，同样转得回去。**两头都有**，
        // 因为几十卷的一趟里往回看一卷不该按二十九下（票面：选中一卷）。
        press(&mut session, &mut running, Key::BackTab);
        let back = session.expansion().expect("还展开着").clone();
        assert_eq!(back.volume, 1, "⇧⇥ 没往前转");
        assert_eq!(back.from, second.from, "两头转到同一卷，落位却不一样");
        press(&mut session, &mut running, Key::BackTab);
        assert_eq!(session.expansion().expect("还展开着").volume, 0);

        // 收起：一个键回到配置，展开态没了。
        assert_eq!(press(&mut session, &mut running, Key::Esc), Exit::Stay);
        assert!(session.expansion().is_none());
    }

    /// 键码翻译认得会话要的那几个，别的原地放过。
    ///
    /// 这一层薄到只剩一张对照表，规矩在 [`state`]——那边的用例问的是
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
