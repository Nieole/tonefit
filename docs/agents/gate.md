# 闸门

三条命令。三条都绿，就知道**三种构建**都还站得住。

「闸门」说的是**这三条命令**，与 `CONTEXT.md` 的领域术语《几何门》没有关系——
那一个判的是一页的像素完整性，这一个判的是这个仓库编不编得过、跑不跑得绿。

## 跑得起来需要什么

三条命令之前先要有两样东西，**两样都不出自 Rust 工具链，换一台机器都要重新备**。
它们不挂在任何特性后面（`image` 与 `unrar-ng` 都是普通依赖），**三条闸门因此各要一份**。

| 要什么 | 谁要它 | 缺了它报错来自 |
|---|---|---|
| **一套 C++ 编译器** | `unrar-ng-sys` 的 `build.rs`（`cc::Build` 开着 `.cpp(true)`）要编 UnRAR 那几十份 `.cpp` | `cc` |
| **一份 `pkg-config` 找得到的 dav1d** | `image` 的 `avif-native` 特性把 AVIF 解码交给 dav1d（`p0-core-pipeline/spec.md` 的《输入》：解码覆盖 AVIF） | `pkg-config` |

**判据是报错来自谁。** 缺了任何一样，**哪一条闸门都**在**编依赖**那一步就停住，
而那一行报错出自 `cc` 或 `pkg-config`、一个 tonefit 的符号都不提——
照它去查自己刚改的那几行，查不出任何东西。两句核验各一行，都不必进 cargo：

```
c++ --version
pkg-config --modversion dav1d
```

**本机（Linux）此刻是这么满足的**（measured，2026-09-10）：

- C++ 那一样是系统装的 `g++`——`/usr/bin/cc`、`/usr/bin/c++`、`/usr/bin/g++` 三个都在，
  `g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0`；
- dav1d 那一样是**系统装的 1.4.1**，`pkg-config --modversion dav1d` 直接答得出，
  **`PKG_CONFIG_PATH` 一个字都不用设**（这个环境里它根本没设）。

**换一台机器要重新备的正是这两样**，而各自的形态随机器变：Windows MSVC 那一台上
C++ 由 MSVC 出，dav1d 经 vcpkg 装、`PKG_CONFIG_PATH` 指到它的 pkgconfig 目录
（那台机器上量到的一张表在 `docs/measurements.md` 的《AVIF 解码的可用路径》）。
C++ 这一条是收下 `.rar` 之后才有的（`volume-discovery/06`）：在那之前依赖树里
一个 C/C++ 编译单元都没有，clone 下来只要一个 Rust 工具链就构建得起来。

**本节只说「跑得起来需要什么」，一个决定都不改。** AVIF 解码换不换那条产物无系统依赖的路
（`zenavif`／rav1d，构建期改要 NASM，见《AVIF 解码的可用路径》），
以及 `.rar` 那份 UnRAR 许可要认的代价（ADR 0015），都不在这里判。

## 一条命令跑满三条

```
cargo xtask gate
```

按 1 2 3 跑，**头一条红了就停**（后面那两条各要好几分钟，先修头一条再说）。
走完印一张《数》：三条各自的最后一行、通过与失败的合计，以及 lib / bin 两个数——
票据的《数》那一节照抄它。

只跑其中几条、按点名的次序：`cargo xtask gate 2 1`。

那条别名在 `.cargo/config.toml`，跑的那件工具在 `xtask/`——一个 workspace 成员，
`default-members` 把它挡在 `cargo build`／`cargo test` 之外，一个外部依赖都不收。

## 三条各盖住什么，各用哪个目录

| | 命令 | target 目录 | 盖住的那一趟 |
|---|---|---|---|
| 1 | `cargo test` | `target`（默认那个） | **默认构建**（`tui` 开着）。库、命令行、会话，加上 `tests/` 那十几个二进制。 |
| 2 | `cargo test --no-default-features` | `target/gate/no-default-features` | **甩掉终端库**那一趟。会话里不碰终端的那几个模块（`session` 下的 `state`、`live`、`run`、`complete`、`viewport`、`columns`、`tone`）摆在特性外面，这一趟连它们自带的用例一起跑。 |
| 3 | `cargo check --features profiling` | `target/gate/profiling` | **开着量具**那一趟。`src/cost.rs` 的 `tally`（两张原子计数表）只有这一条够得着。 |

**这三条不要裸敲。**右边那一列只有走 `cargo xtask` 才成立：裸敲一次
`cargo test --no-default-features`，不带 `tui` 的那个二进制就被摆回默认的 `target` 里，
下一趟闸门 1 当场假红（下一节说的正是这件事）。要单跑某一条，走
`cargo xtask gate 1`／`2`／`3`。

**三条互相说明不了对方**：

- 第一条绿不说明第二条编译得过——会话那几个模块只要有一个 `use` 到终端库，
  或者哪个只有画法读得到的东西被搬出画法，第二条当场红，第一条一声不吭。
- 前两条绿都不说明第三条编译得过——`profiling` **不在 `default` 里**
  （那个量具存在的前提就是默认整个不在），前两条一行都不碰 `tally`。

## 为什么各用各的目录

**一个 target 目录一次只记得住一种特性组合。**三条共用一个时，特性一来一回整棵树判失效——
闸门 1 与闸门 2 交替跑一次要十几分钟。

更坏的是**假红**：`cargo test` 判定 `tonefit` 那个可执行文件「新鲜」时不会重新摆放它，
于是跑过闸门 2 再跑闸门 1，用例启动到的是上一趟留下的**不带 `tui`** 的那一个——
无参数时它走 clap 的必填项错误、退出码 `2`，而那条用例要的是会话那条岔路的 `1`。
**红得像真的，其实是跑序的锅。**

分目录一刀切掉这两件：闸门 2 与闸门 3 各带一个 `CARGO_TARGET_DIR`，谁都不让谁失效，
**跑序因此说明不了任何事**。闸门 1 用的就是默认的 `target`，不另开一个：
它与日常的 `cargo build`、`cargo test`、编辑器那一路的特性组合逐格相同，
共用一个目录是白拿的复用，不是这里要修的病。

三个目录**都在 `target` 底下**，`cargo clean` 一条清得干净，`.gitignore` 的 `/target` 一条盖得住。

## 什么时候必须跑满三条

改动触及下面任何一项：

- `src/session/`、`src/main.rs` 的 `mod session`、`src/render.rs` 上挂着 `tui` 的那几项——第二条。
- `src/cost.rs`、往 `Stage` 加一格、或任何一处 `cost::stage(...)` 的掐表点——第三条。
- `Cargo.toml` 的 `[features]`——三条都要。

其余改动第一条足矣（`cargo xtask gate 1`）。**落地一张票收尾时按惯例三条都跑**，
票据的《数》那一节记下各自的最后一行。

## 读结果

- **第一条**跑满要几分钟，别以为它挂了。`smoke` 在未设 `TONEFIT_SAMPLES` 时印一行跳过、
  贡献 0 个通过，那是正常的（理由见 `tests/smoke.rs` 的模块文档）。
  **闸门 2 与闸门 3 头一趟同样要几分钟**——它们各自那个目录是空的，整棵树要从零建一遍；
  屏上照旧一行行印着 `Compiling`，那就是它在动。
- **第二条**不该比第一条少掉任何一条状态机的用例。少了，就是有人把某个模块挪回了
  `tui` 后面——分界写在 `src/session.rs` 的模块文档《终端库在哪一半》。
  那一趟里「只有画法读得到」的那几处**各挂各的一句** `allow(dead_code)`，
  不是整个模块放开：会话里添的**真**死代码在这一条上照旧当场报。
- **第三条**只编译、不跑。计数那一半的**行为**断言在第一条里（`cost::tests`）：
  挑哪几行、按什么排、印成什么样都是纯函数，摆在特性外面；
  第三条验的是往原子表上加数、从它上面读回来的那一截。

各票据的《数》一节记着它落地那一刻三条各自的最后一行，**数以那里为准**，本文件不复述。

## 闸门之外，收尾照例过一遍的

```
cargo xtask polish
```

四条，按次序：

| | 命令 | target 目录 |
|---|---|---|
| 1 | `cargo fmt --check` | 不建目录 |
| 2 | `cargo clippy --all-targets` | `target` |
| 3 | `cargo clippy --all-targets --no-default-features` | `target/gate/no-default-features` |
| 4 | `cargo doc --no-deps` | `target`（文档因此照旧落在 `target/doc`） |

它们不判「站不站得住」，因此**不算闸门**；但一张票落地时照例过一遍。
第三条跟着闸门 2 用同一个目录——它的特性组合与闸门 1 不同，摆在默认目录里
同样会让整棵树来回失效，那正是上面《为什么各用各的目录》说的那件事。

`cargo doc` 那几条既有告警的条数记在票据的《数》里——**别让它变多**。
那一行（`warning: \`tonefit\` (lib doc) generated N warnings`）由 `cargo xtask polish`
原样印进走完那张《数》里，不必自己往回翻。
