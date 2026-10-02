# 10 — 并发那一条在 macOS 上问得出东西

**What to build:** 并发那一条用例在这台 macOS 上恢复检出力：给它补**第三种问法**——列本进程打开的描述符、逐个取路径、按根过滤。补上之后闸门 1、2 在 macOS 上不再基线就红，阳性对照与主断言在这个平台上都问得出东西。Linux 与 Windows 两种问法照旧。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] macOS 上那一条用例绿，阳性对照那一支真的问到了打开的文件（按反跑一次：让它多开一份，看见红）
- [ ] Linux 与 Windows 两种问法照旧
- [ ] `docs/agents/gate.md` 里若写着这台机器上这一条红，改成如实
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
