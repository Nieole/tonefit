//! 闸门跑一遍：三种构建各一条命令，**各用各的 target 目录**。
//!
//! 三条闸门是什么、各盖住什么、结果怎么读，出处只有一处：`docs/agents/gate.md`。
//! 本文件只说**它是怎么跑起来的**。
//!
//! # 为什么要各用各的目录
//!
//! 三条命令的特性组合各不相同，而一个 target 目录一次只记得住一种：特性一来一回，
//! 整棵树判失效，交替跑一次要十几分钟。更坏的是**假红**——`cargo test` 判定
//! `tonefit` 那个可执行文件「新鲜」时不会重新摆放它，于是跑过闸门 2 再跑闸门 1，
//! 用例启动到的是上一趟留下的**不带 `tui`** 的那一个：无参数时它走 clap 的必填项错误、
//! 退出码 `2`，而那条用例要的是会话那条岔路的 `1`。**红得像真的，其实是跑序的锅。**
//!
//! 分目录一刀切掉这两件事：闸门 2 与闸门 3 各带一个 `CARGO_TARGET_DIR`，
//! 谁都不再让谁失效，跑序因此说明不了任何事。
//!
//! **闸门 1 用的就是默认的 `target/`**，不另开一个：它与日常的 `cargo build`、
//! `cargo test`、编辑器那一路的特性组合**逐格相同**，共用一个目录是白拿的复用，
//! 不是这里要修的病。
//!
//! # 数出来的那几个
//!
//! 收尾要把三条各自的最后一行与通过数记进票据的《数》。`cargo test` 印结果的次序
//! 是定的（lib 的单元测试 → bin 的单元测试 → `tests/` 那几个 → 文档测试），
//! 因此 `test result:` 那几行**按出现次序**头一条算 lib、第二条算 bin，
//! 其余并进合计。走完印一张表，照抄进票据即可。

use std::io::{BufRead, BufReader, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;

/// 一条要跑的命令。
struct Step {
    /// 屏上与总表里的名字。
    name: &'static str,
    /// `cargo` 后面那几个词。
    args: &'static [&'static str],
    /// 相对仓库根的 target 目录；`None` 表示用默认的 `target/`。
    target_dir: Option<&'static str>,
}

/// 三条闸门，按 `docs/agents/gate.md` 的次序。
const GATES: [Step; 3] = [
    Step {
        name: "闸门 1 · 默认构建",
        args: &["test"],
        target_dir: None,
    },
    Step {
        name: "闸门 2 · 甩掉终端库",
        args: &["test", "--no-default-features"],
        target_dir: Some(NO_DEFAULT_FEATURES),
    },
    Step {
        name: "闸门 3 · 开着量具",
        args: &["check", "--features", "profiling"],
        target_dir: Some(PROFILING),
    },
];

/// 闸门之外、收尾照例过一遍的那四条。
///
/// 它们不判「站不站得住」，因此**不是闸门**（`docs/agents/gate.md`）。
/// 收进这里只为一件事：`clippy --no-default-features` 与闸门 1 的特性组合不同，
/// 摆在默认目录里同样会让整棵树来回失效——它跟着闸门 2 用同一个目录。
const POLISH: [Step; 4] = [
    Step {
        name: "排版",
        args: &["fmt", "--check"],
        target_dir: None,
    },
    Step {
        name: "clippy · 默认构建",
        args: &["clippy", "--all-targets"],
        target_dir: None,
    },
    Step {
        name: "clippy · 甩掉终端库",
        args: &["clippy", "--all-targets", "--no-default-features"],
        target_dir: Some(NO_DEFAULT_FEATURES),
    },
    Step {
        name: "文档",
        args: &["doc", "--no-deps"],
        target_dir: None,
    },
];

const NO_DEFAULT_FEATURES: &str = "target/gate/no-default-features";
const PROFILING: &str = "target/gate/profiling";

const USAGE: &str = "\
用法：
  cargo xtask gate            三条闸门按 1 2 3 跑满
  cargo xtask gate 2 1        只跑点名的那几条，按点名的次序
  cargo xtask polish          闸门之外那四条（排版、两遍 clippy、文档）

三条各盖住什么、各自的 target 目录在哪，见 docs/agents/gate.md。";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (steps, picked) = match arguments.split_first() {
        Some((first, rest)) if first == "gate" => match pick(rest) {
            Ok(picked) => (&GATES[..], picked),
            Err(bad) => {
                eprintln!("认不得的闸门号 `{bad}`——只有 1、2、3。\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
        },
        Some((first, rest)) if first == "polish" && rest.is_empty() => {
            (&POLISH[..], (0..POLISH.len()).collect())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let root = workspace_root();
    let mut done: Vec<(&Step, Tally)> = Vec::new();

    for (nth, index) in picked.iter().enumerate() {
        let step = &steps[*index];
        println!(
            "\n━━ 第 {} 条／共 {} 条　{}\n   {}　（目录 {}）\n",
            nth + 1,
            picked.len(),
            step.name,
            commandline(step),
            step.target_dir.unwrap_or("target"),
        );
        let tally = run(step, &root);
        let green = tally.green;
        done.push((step, tally));
        // **头一条红了就停**：三条都要绿，而后面那两条各要好几分钟。
        if !green {
            report(&done);
            match picked.len() - nth - 1 {
                0 => println!("\n{} 红了。", step.name),
                left => println!("\n{} 红了，后面 {left} 条没跑。", step.name),
            }
            return ExitCode::FAILURE;
        }
    }

    report(&done);
    println!("\n全绿。");
    ExitCode::SUCCESS
}

/// `gate` 后面那几个数：不写就是三条都跑。
fn pick(arguments: &[String]) -> Result<Vec<usize>, String> {
    if arguments.is_empty() {
        return Ok((0..GATES.len()).collect());
    }
    arguments
        .iter()
        .map(|argument| match argument.as_str() {
            "1" => Ok(0),
            "2" => Ok(1),
            "3" => Ok(2),
            other => Err(other.to_owned()),
        })
        .collect()
}

/// 一条跑完之后手里剩下的东西。
#[derive(Default)]
struct Tally {
    green: bool,
    /// `test result:` 那几行按出现次序数出来的（通过, 失败）。
    results: Vec<(usize, usize)>,
    /// 「一共几条告警」那几行（`cargo doc` 与两遍 clippy 各印一条）。
    notes: Vec<String>,
    /// stdout 与 stderr 各自最后一行不空的话——票据的《数》要的就是它。
    last_out: String,
    last_err: String,
}

/// 起一个 cargo 子进程，边转边数。
fn run(step: &Step, root: &Path) -> Tally {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(root)
        .args(step.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = step.target_dir {
        command.env("CARGO_TARGET_DIR", root.join(directory));
    }
    // 子进程的两条流都是管道，cargo 因此判自己不在终端上、一个颜色都不上。
    // **我们这一头**是终端时把颜色要回来（转出去的是同一串字节）；重定向到文件时不要——
    // 那样落进文件的是一堆转义码。
    if std::io::stdout().is_terminal() {
        command.env("CARGO_TERM_COLOR", "always");
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("起不来 `{}`：{error}", commandline(step));
            return Tally::default();
        }
    };
    let out = child.stdout.take().expect("stdout 是 piped 的");
    let err = child.stderr.take().expect("stderr 是 piped 的");

    // 两条流各一个线程，各自转到同名的那一头去——次序在两条流之间不保证，
    // 而要数的那几行全在 stdout 上，用不着跨流对齐。
    let elsewhere = thread::spawn(move || tee(err, Stream::Err));
    let mine = tee(out, Stream::Out);
    let theirs = elsewhere.join().unwrap_or_default();
    let green = child.wait().map(|status| status.success()).unwrap_or(false);

    let mut notes = mine.notes;
    notes.extend(theirs.notes);

    Tally {
        green,
        results: mine.results,
        notes,
        last_out: mine.last,
        last_err: theirs.last,
    }
}

/// 转到哪一头去。
#[derive(Clone, Copy)]
enum Stream {
    Out,
    Err,
}

/// 一条流上留下来的东西。
#[derive(Default)]
struct Kept {
    results: Vec<(usize, usize)>,
    /// 「一共几条告警」那几行原样留着——`cargo doc` 那个数要记进票据的《数》。
    notes: Vec<String>,
    last: String,
}

/// 边读边原样转出去，顺手把 `test result:` 与「一共几条告警」那几行数下来。
///
/// **按字节读、按 lossy 转字符**，不走 `lines()`：那一个碰到非法 UTF-8 回的是 `Err`，
/// 而一条流上出一个坏字节就把整条流截断的话，子进程下一次写会拿到 EPIPE 当场死掉——
/// 一条本来全绿的闸门于是报成红的，屏上还说不出为什么。
fn tee(stream: impl Read, to: Stream) -> Kept {
    let mut kept = Kept::default();
    let mut reader = BufReader::new(stream);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match reader.read_until(b'\n', &mut raw) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end_matches(['\n', '\r']);
        match to {
            Stream::Out => println!("{line}"),
            Stream::Err => eprintln!("{line}"),
        }
        if let Some(counted) = counts(line) {
            kept.results.push(counted);
        }
        if counted_warnings(line) {
            kept.notes.push(line.to_owned());
        }
        if !line.trim().is_empty() {
            kept.last = line.to_owned();
        }
    }
    kept
}

/// `test result: ok. 213 passed; 0 failed; ...` → `(213, 0)`。
fn counts(line: &str) -> Option<(usize, usize)> {
    let rest = line.trim_start().strip_prefix("test result:")?;
    let words: Vec<&str> = rest.split_whitespace().collect();
    let mut passed = 0;
    let mut failed = 0;
    for pair in words.windows(2) {
        let Ok(number) = pair[0].parse::<usize>() else {
            continue;
        };
        match pair[1].trim_end_matches(';') {
            "passed" => passed = number,
            "failed" => failed = number,
            _ => {}
        }
    }
    Some((passed, failed))
}

/// `warning: `tonefit` (lib doc) generated 15 warnings` —— 认的是这一行。
///
/// 不按 `warning:` 开头认：开了颜色之后那个词前面还有一串转义码。
fn counted_warnings(line: &str) -> bool {
    let line = line.trim_end();
    line.contains(" generated ") && (line.ends_with("warnings") || line.ends_with("warning"))
}

/// 走完印一张表——票据的《数》照抄它。
fn report(done: &[(&Step, Tally)]) {
    println!("\n━━ 数\n");
    for (step, tally) in done {
        println!(
            "{}　{}　（目录 {}）",
            if tally.green { "绿" } else { "红" },
            step.name,
            step.target_dir.unwrap_or("target"),
        );
        println!("   {}", commandline(step));
        if !tally.results.is_empty() {
            let passed: usize = tally.results.iter().map(|(passed, _)| passed).sum();
            let failed: usize = tally.results.iter().map(|(_, failed)| failed).sum();
            print!("   合计 {passed} 通过 {failed} 失败");
            if let Some((lib, _)) = tally.results.first() {
                print!("；lib {lib}");
            }
            if let Some((bin, _)) = tally.results.get(1) {
                print!(" / bin {bin}");
            }
            println!();
        }
        for note in &tally.notes {
            println!("   {note}");
        }
        let last = if tally.last_out.is_empty() {
            &tally.last_err
        } else {
            &tally.last_out
        };
        if !last.is_empty() {
            println!("   末行 {last}");
        }
    }
}

/// 屏上印出来的那一条命令。
fn commandline(step: &Step) -> String {
    format!("cargo {}", step.args.join(" "))
}

/// 仓库根：本包在它底下一层。
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}
