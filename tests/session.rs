//! 会话的**入口**，在真进程上测。
//!
//! 会话本身（三层、按键、补全、画法）在二进制 crate 内单元测——那是 spec 的《Seam》
//! 定的位置，也是 story 44「状态机脱离终端可测」的落点。这里只测那一层观察不到的两件事：
//!
//! - **无参数、而 stderr 不是终端**时印什么、退出码是几。两样都只有进程那一层看得见：
//!   `is_terminal` 问的是这个进程的 stderr，而退出码是 `main` 交出去的那个数。
//! - **stdout 仍然只装报告**：会话画在 stderr，被截住的那一趟因此一个字节都不往 stdout 写。
//!
//! 用例把 stderr 接成管道，「不是终端」这个前提于是自动成立——CI 与 `2>日志`
//! 落到的正是同一条路。
//!
//! **无参数那一条按 `tui` 特性分成两半**：特性开着（默认）时它进会话那条岔路，
//! 关掉时它退回 clap 的必填项错误——那正是 spec 的《依赖》要的
//! 「feature 关掉时无参数退回 clap 的必填项错误」。两半各有一条用例，
//! `cargo test` 与 `cargo test --no-default-features` 因此各验各的那一半。

use std::process::{Command, Stdio};

/// 不带参数、而 stderr 不是终端：说清没有终端，**并且不把 clap 那条用法提示吃掉**，
/// 退出码 `1`（`p1-session/08` 的验收）。
#[cfg(feature = "tui")]
#[test]
fn no_arguments_without_a_terminal_says_so_and_still_shows_the_usage() {
    let done = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("启动 tonefit");

    assert_eq!(done.status.code(), Some(1), "没有终端那一趟的退出码不是 1");

    let said = String::from_utf8_lossy(&done.stderr);
    assert!(said.contains("这里没有终端"), "{said}");
    // clap 那一半：缺了哪几项、该怎么敲，一个字都没被吃掉。
    assert!(said.contains("--out"), "{said}");
    assert!(said.contains("--profile"), "{said}");
    assert!(
        said.contains("Usage") || said.contains("usage"),
        "clap 的用法行没了：{said}"
    );

    // 会话画在 stderr，stdout 只装报告——这一趟没有报告，因此一个字节都没有。
    assert!(
        done.stdout.is_empty(),
        "stdout 被占用了：{}",
        String::from_utf8_lossy(&done.stdout)
    );
}

/// 关掉 `tui` 特性之后，无参数**退回 clap 的必填项错误**（`p1-session/08` 的验收）。
///
/// 这一条只在关掉特性的那一趟跑得到，而它正是那一趟唯一的行为断言——
/// 会话自己那批用例整个在特性后面，关掉就不编译（停车场 Q61）。
/// 断言里有一句「不提会话」：退回去要退干净，不该留下半句只有会话才说得出的话。
#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_no_arguments_falls_back_to_clap() {
    let done = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("启动 tonefit");

    let said = String::from_utf8_lossy(&done.stderr);
    assert!(!said.contains("这里没有终端"), "退回去没退干净：{said}");
    assert!(said.contains("--out"), "{said}");
    assert!(said.contains("--profile"), "{said}");
    assert!(!done.status.success(), "缺必填项那一趟不该算成功");
    assert!(done.stdout.is_empty(), "stdout 被占用了");
}

/// **带参数那一路一字不变**：会话没有把必填项松掉，也没有把子命令顶掉。
///
/// 三项必填的判定在 `src/main.rs` 的单元测里逐条钉着；这里问的是进程那一层——
/// 参数只要有一个，入口就该把这一趟原样交给 clap，而不是拐进会话。
#[test]
fn a_command_line_with_arguments_never_reaches_the_session() {
    let tonefit = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tonefit"))
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .expect("启动 tonefit")
    };

    // 缺必填项：clap 自己那条错误，不是「这里没有终端」。
    let missing = tonefit(&["--profile", "kobo-libra-2"]);
    let said = String::from_utf8_lossy(&missing.stderr);
    assert!(!said.contains("这里没有终端"), "{said}");
    assert!(said.contains("--out"), "{said}");

    // `--help` 与 `--version` 照旧走 clap，印在 stdout 上。
    for flag in ["--help", "--version"] {
        let helped = tonefit(&[flag]);
        assert!(helped.status.success(), "{flag} 没跑成");
        assert!(!helped.stdout.is_empty(), "{flag} 什么都没印");
    }
}
