# 10 — 并发那一条在 macOS 上问得出东西

**What to build:** 并发那一条用例在这台 macOS 上恢复检出力：给它补**第三种问法**——列本进程打开的描述符、逐个取路径、按根过滤。补上之后闸门 1、2 在 macOS 上不再基线就红，阳性对照与主断言在这个平台上都问得出东西。Linux 与 Windows 两种问法照旧。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] macOS 上那一条用例绿，阳性对照那一支真的问到了打开的文件（按反跑一次：让它多开一份，看见红）
- [x] Linux 与 Windows 两种问法照旧
- [x] `docs/agents/gate.md` 里若写着这台机器上这一条红，改成如实
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

这台机器上「三条全绿」跑不出来，每张票都得记一句「红的只有 Q995」，真红一条也可能被它遮住。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q995 — 这台 macOS 上闸门 1、2 在基线上就红：并发那一条的阳性对照只认 Linux 与 Windows

- **From:** 票 `proof-sheet/02`
- **Kind:** 路过发现（与本票无关的缺陷；本机是 macOS）
- **Where:** `tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，
  阳性对照那一支（`open_files_under` 答 `None` 就当成 Windows、改问「目录改不改得动名」）
- **Why it did not block:** 不在停线三条里：它在**没改过的树上**（`11a2370`）就红，闸门 1 与闸门 2 各红这一条、只红这一条；
  本票的改动一行都没碰它。
- **What this ticket actually did:** 一个字没动它。`cargo xtask gate` 头一条红了就停，而 `cargo test` 在头一个红的测试二进制上
  就不往下跑（`concurrency` 排在 `golden`、`counters`、`pipeline`、`proof` 前面）——照那样跑，本票要的两道钉子根本跑不到。
  因此基线与收尾都改成：闸门 1、2 各跑一遍**同一条命令加 `--no-fail-fast`**、各用自己那个 target 目录
  （`cargo test --no-fail-fast`；`cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features`），
  闸门 3 走 `cargo xtask gate 3`。数记在票据的《数》里，红的那一条两头都点名。
- **Why it matters:** 这台机器上「三条全绿」**永远跑不出来**，而且红法很安静：主断言在 macOS 上是**空转**的
  （没有 `/proc/self/fd` 可数、开着文件的目录照样改得动名，`between` 那两句恒答 0），
  只有阳性对照说出了实话。下一个在这台机器上做票的人会被同一处挡住，再把闸门的跑法重新摸一遍。
- **Options:** ① 给 macOS 补第三种问法：`/dev/fd` 列出本进程的描述符，逐个 `fcntl(F_GETPATH)` 取路径
  （`libc` 已是 dev 依赖），按 `root` 过滤——阳性对照与主断言在这个平台上都恢复检出力；
  ② 两种问法都答不出的平台上，这一条**明写跳过**（走自带 harness 那样分得清「跳过」与「跑过了」），不再假装问过；
  ③ 只在 `docs/agents/gate.md` 写一句「macOS 上这一条已知红、闸门改用 `--no-fail-fast` 跑」。
- **Recommend:** ①。③ 等于承认闸门在这台机器上没有检出力；② 诚实但把这台机器上的检出力整个让掉，
  而这正是日常做票的那一台。
- **Whose call:** 拍板的人（归 `green-counts` 那一批，或另立一张票）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `green-counts`（新添第 10 张），待 `/to-spec`／`/to-tickets`：给 macOS 补第三种问法：`/dev/fd` 列描述符、逐个 `fcntl(F_GETPATH)` 取路径。


## 落地记录

在 `4f3d1c1` 上直接实现 Q995（macOS 15.7.7 / Darwin arm64，本地工作区未提交）。先标 claimed，完成下列验证后 resolved；本票仅收 Q995，不代表 CI delivery 整体完成。

- `tests/concurrency.rs`：`open_files_under` 在 macOS 列 `/dev/fd`，通过 `fstat` 排除无文件系统路径的 pipe/socket，再用 `fcntl(F_GETPATH)` 取本进程描述符的路径。保留 Linux `/proc/self/fd` 计数与 Windows 目录改名探针、原来的开工零句柄断言和开遍阳性对照。
- `src/survey.rs`：仅 `#[cfg(test)]` 中相同的辅助函数及说明同步；不改清点或生产行为。两个 crate 继续各保留一份实现，未添依赖。
- 两份探针各添一个 macOS 聚焦用例：关闭文件为 0、库外开着的文件不计入、库内打开为 1、关闭后归零；原路径和软链根均检查。根 `canonicalize` 与内核路径对齐，覆盖 `/var` → `/private/var` 及显式软链别名，领域《同一处》的生产语义未变。
- Linux/macOS 描述符目录/目录项读取错误、根解析错误与未知系统调用错误均显式失败；只容忍并行测试关掉描述符/删除文件造成的已知消失错误。只有 Windows 返回 `None`，不再把 Linux `/proc` 不存在当作 Windows 语义。
- 已读 `docs/agents/gate.md`：其中没有「本机 Q995 已知红」的当前说明，故无需改它。源内说明现明确 Linux/macOS 计数与 Windows 改名各自负责检出。

### 红与按反证据

所有 Cargo 调用均使用以下完整前缀（包括 fmt），命令在仓库根执行，无额外 target 命名空间：

```sh
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo test --test concurrency many_archive_volumes_never_hold_more_than_the_one_being_processed -- --exact
```

- 修复前的新鲜红：上面命令退出 101，0 passed / 1 failed，1.08s，阳性对照实际 0 / 期望 96。日志 `/tmp/ci-delivery-q995-red.log`（此前本轮已执行并记录的 first red，不重复猜测根因）。
- 实现后同命令：1 passed / 0 failed，1.09s；`/tmp/ci-delivery-q995-green.log`。
- 按反：仅在同一用例的 `let watch = Handles::new(&library)` 前临时插入 `let _extra_archive = std::fs::File::open(&inputs[0]).expect("Q995 mutation: hold extra archive");`，让额外归档活到测试结束。原命令退出 101，0 passed / 1 failed，1.08s，主守卫「开工那一刻还开着别的卷的归档」实际 1 / 期望 0；`/tmp/ci-delivery-q995-mutation.log`。已移除变异，之后才跑完整三闸门，全绿；不是跳过或削弱阳性对照。

### 数

其余精确验证命令：

```sh
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo fmt
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo test macos_descriptor_probe_counts_only_open_files_under_the_canonical_root
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo test --lib survey::tests::a_survey_keeps_no_archive_open -- --exact
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo xtask gate
CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo xtask polish
```

- 聚焦描述符对照：lib 1 passed、concurrency 1 passed，其他目标过滤后 0，均 0 failed；`/tmp/ci-delivery-q995-controls.log`。
- 清点句柄守卫：1 passed / 0 failed，0.14s；`/tmp/ci-delivery-q995-survey.log`。
- 完整 `cargo xtask gate`，退出 0，三条全绿，日志 `/tmp/ci-delivery-q995-gate.log`：
  - 1 `cargo test`（target）：合计 **1189 通过 / 0 失败；lib 257 / bin 507**。末行 `all doctests ran in 1.34s; merged doctests compilation took 0.56s`。
  - 2 `cargo test --no-default-features`（target/gate/no-default-features）：合计 **1033 通过 / 0 失败；lib 257 / bin 351**。末行 `all doctests ran in 0.97s; merged doctests compilation took 0.43s`。
  - 3 `cargo check --features profiling`（target/gate/profiling）：末行 `Finished dev profile [unoptimized + debuginfo] target(s) in 1.89s`（原日志 dev 有反引号）。只编译，不计作运行测试。
- `cargo xtask polish` 退出 0，四条全绿；日志 `/tmp/ci-delivery-q995-polish.log`：fmt --check；默认 clippy 4.68s；无默认特性 clippy 3.95s；doc 生成 `target/doc/tonefit/index.html`。lib doc 15 条既有 private-item 链接告警；两种 bin 构建各有既有 `src/main.rs:1194 fetch_update` deprecated 告警，未顺手修改无关代码。

### 未验与交接

- 未设置 `TONEFIT_SAMPLES`：两个 gate 测试构建各明确报告 perceptual 真机护栏跑过 3 条、缺素材跳过 2 条，smoke 缺素材跳过、贡献 0 个通过；没有添加跳过，不声称真实素材全部实测。
- 本地仅 macOS ARM64，未执行 Linux / Windows / macOS Intel 原生运行；这些保留给 CI delivery 后续真实四平台 CI，不以读代码冒充实测。
- 未提交、未动用户 `.gitattributes`；交付范围仍仅测试/测量辅助与本票记录。
