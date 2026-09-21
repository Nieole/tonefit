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
//! # 各模块管什么
//!
//! 界面是 ADR 0019 那一副：任务 / 配置两个视图（`CONTEXT.md` 的《会话》）。
//!
//! 三组设置与阶段在 [`state`]，一个终端都不碰；视图 × 焦点与界面的状态机在 [`view`]
//! （挂在 [`state::Session`] 上一格）；**一张按键表**在 [`keymap`]（屏底与全部按键都由它派生）；
//! 输入行与补全在 [`typing`]（逐层读盘那一步在 [`complete`]）；覆盖层与全部按键那一张在 [`cover`]；
//! 清点之后卷列表那棵树（分区 · 目录行 · 卷行 · 备注行）的拼法在 [`tree`]；
//! 配置视图那一副的内容（设置栏那几行、详情栏那几格、画质判定参数那五行）在 [`config`]；
//! 家目录缩写在 [`home`]；边跑边攒的那一份在 [`live`]；起线程在 [`run`]；
//! 一个列表在一个格子里露出哪一段在 [`viewport`]；卷列表与每页结果那两张表各有哪几列、
//! 窄了先让谁在 [`columns`]；屏上一件事有多重分成哪四种在 [`tone`]，一格要什么样子在 [`look`]。
//! 整屏画法在 [`shell`]（屏上一块一个模块，**名单只在它的模块文档那张表里**，这里不抄第二份），
//! 进出终端、键码翻译与那条循环在 [`terminal`]（它把每一个输入交给 `input`）。
//! 按设计稿的场景数据摆出那一趟与三组设置的测试夹具在 [`scene`]（只在 `test` 里）。
//!
//! # 终端库在哪一半
//!
//! **分界就是这几行 `mod`。**上面十五个模块（[`columns`]、[`state`]、[`live`]、[`run`]、
//! [`complete`]、[`viewport`]、[`tone`]、[`look`]、[`home`]、[`keymap`]、[`view`]、[`cover`]、
//! [`typing`]、[`tree`]、[`config`]）一个终端库都不 `use`，因此摆在特性**外面**：`--no-default-features`
//! 那一趟照编、照跑它们自带的用例（`p2-loose-ends/01`，闸门的第二条）；只在 `test` 里的
//! [`scene`] 也在外面，那一趟照跑它不经画法的那几条。真要终端库的那两个
//! （[`shell`] 画屏，[`terminal`] 进出终端并翻译 crossterm 键码）留在 `tui` 后面——
//! [`shell`] 底下那几个模块跟着它整棵在后面，新拆一块不必再挂一次 `cfg`。
//!
//! 整个 `session` 模块挂在 `any(feature = "tui", test)` 上，而不是无条件：
//! `tui` 关掉的那一趟**没有会话**（[`crate::without_arguments`] 恒不接手），
//! 状态机于是没有一个非测试的用户——不挂 `test` 就是一片 `dead_code` 告警。
//! 同一副写法见 `src/cost.rs`。
//!
//! 那一趟里「没人读」的那几处**各挂各的一句** `cfg_attr(not(feature = "tui"), allow(dead_code, ...))`：
//! 画法与那条循环不编译，只有它们读得到的那些取值器、那几格状态，以及画法那一侧的用例
//! 才用得着的那几份夹具（`live::fixture`，它自己也挂着 `cfg(test)`），于是全成了「没人读」——
//! **那不是死代码，是这一趟的前提**。
//!
//! **挂在逐处，不挂在 `mod session` 上。**整模块放开一句就管住底下特性外面的每一个模块，
//! 将来在会话里添的**真**死代码于是在第二条闸门上也不报——放开的是将来，不是眼下那几处
//! （`p4-parking-lot/24` 收窄的正是这一笔）。默认那一趟一格都不放松。
//!
//! # 一趟跑起来之后
//!
//! [`tonefit::run`] 一进去就跑到底，会话这一头还得接着画、接着认键，因此它在
//! **另一条线程**上（见 [`run::Running`]）。循环于是不再是「等一个键」，而是
//! 「等一个键，最多等 [`terminal::TICK`] 那么久」——没等到就画下一帧，
//! 跑着的那一趟因此看得见在动。

// 卷列表与每页结果那两张表的列。**只有它比别的模块敞开一格**（`pub(crate)`）：
// 「哪几格要过宽度那一关」由各列自己的**字面出处**答，而问那一关的用例住在
// 造字面的那一层（`crate::render`）——那份名单从前在它那一头手抄着第二份
// （停车场 Q188）。摆法与砍列仍是 `pub(super)`，一格都没敞开。
pub(crate) mod columns;
mod complete;
mod live;
mod run;
mod state;
mod tone;
mod viewport;

mod config;
mod cover;
mod home;
mod keymap;
mod look;
mod tree;
mod typing;
mod view;

// 场景夹具：按设计稿导出的场景数据摆出那一趟与三组设置（`session-redesign/05`）。
// 它一个终端库都不 `use`，因此与上面十五个一样摆在特性外面：`--no-default-features`
// 那一趟照编、照跑它自带的用例；只有对着设计快照字网格的那几条挂在 `tui` 后面。
#[cfg(test)]
mod scene;

#[cfg(feature = "tui")]
mod shell;
#[cfg(feature = "tui")]
mod terminal;

#[cfg(feature = "tui")]
pub use terminal::enter;
