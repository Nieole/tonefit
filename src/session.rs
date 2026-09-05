//! 会话：**不带任何参数敲 `tonefit`** 进到的那一段（`CONTEXT.md` 的《会话》）。
//!
//! 它是 `run` 之上的第二个薄层，与命令行同级：不绕过 seam，也不多一条管线
//! （spec 的《Seam》）。带参数那一路一字不改——无参数在 clap **之前**就被截住
//! （见 [`crate::without_arguments`]），带参数的那一趟根本走不到这里。
//!
//! # 画在 stderr
//!
//! 与进度条同一个去处。**stdout 仍然只装报告**，`tonefit > 报告.txt` 因此仍然成立
//! （退出会话时把报告印到 stdout 归 `p1-session/09`）。
//!
//! stderr 不是终端时（CI、`2>日志`）不进会话，也不崩在 raw mode 里：
//! 印一条说得清的话，**连同 clap 那条必填项用法提示**，退出码 `1`
//! （见 [`terminal::no_terminal_error`]）。
//!
//! # 三层与终端分开
//!
//! 状态机在 [`state`]，一个终端都不碰；边跑边攒的那一份在 [`live`]，同样不碰；
//! 逐层补全在 [`complete`]；起线程在 [`run`]；一个列表在一个格子里露出哪一段在
//! [`viewport`]；卷表有哪几列、窄了砍哪几列在 [`columns`]。画法在 [`draw`]，
//! 进出终端、键码翻译与那条循环在 [`terminal`]。
//!
//! 画法自己按**屏上那几块**又分了几个模块（左栏、预设栏、总览块、报告区连同它那张卷表、
//! 屏底），**名单只在 [`draw`] 的模块文档那张表里**，这里不抄第二份。
//!
//! # 终端库在哪一半
//!
//! **分界就是这几行 `mod`。**上面六个模块（[`columns`]、[`state`]、[`live`]、[`run`]、
//! [`complete`]、[`viewport`]）一个终端库都不 `use`，因此摆在特性**外面**：`--no-default-features`
//! 那一趟照编、照跑它们自带的用例（`p2-loose-ends/01`，闸门的第二条）。真要终端库的那两个
//! （[`draw`] 画，[`terminal`] 进出终端并翻译 crossterm 键码）留在 `tui` 后面——
//! [`draw`] 底下那几个模块跟着它整棵在后面，新拆一块不必再挂一次 `cfg`。
//!
//! 整个 `session` 模块挂在 `any(feature = "tui", test)` 上，而不是无条件：
//! `tui` 关掉的那一趟**没有会话**（[`crate::without_arguments`] 恒不接手），
//! 状态机于是没有一个非测试的用户——不挂 `test` 就是一片 `dead_code` 告警。
//! 同一副写法见 `src/cost.rs`。
//!
//! 那一趟还整个放开了 `dead_code`（`mod session` 上那一句 `cfg_attr`）：
//! 画法不编译，只有画法读得到的那些取值器于是全成了「没人读」——
//! **那不是死代码，是这一趟的前提**。默认那一趟一格都不放松，
//! 真死掉的东西照旧当场红（停车场 Q84 记着这笔放松）。
//!
//! # 一趟跑起来之后
//!
//! [`tonefit::run`] 一进去就跑到底，会话这一头还得接着画、接着认键，因此它在
//! **另一条线程**上（见 [`run::Running`]）。循环于是不再是「等一个键」，而是
//! 「等一个键，最多等 [`terminal::TICK`] 那么久」——没等到就画下一帧，
//! 跑着的那一趟因此看得见在动。

mod columns;
mod complete;
mod live;
mod run;
mod state;
mod viewport;

#[cfg(feature = "tui")]
mod draw;
#[cfg(feature = "tui")]
mod terminal;

#[cfg(feature = "tui")]
pub use terminal::enter;
