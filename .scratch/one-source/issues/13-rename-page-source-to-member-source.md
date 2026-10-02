# 13 — `PageSource` 改名 `MemberSource`

**What to build:** 透传文件那一份哈希借用了 `PageSource` 这个名字，而它不是页。单独一次全仓改名：类型改叫 `MemberSource`，
《页级源哈希》那一条改成《成员源哈希 (MemberSource)》（一页或一个透传文件，一个成员一份；页上写的仍是这一份）；
tEXt 键不动，输出一个字节不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 类型与它的文档改名；旧名在仓库里一处不剩
- [x] `CONTEXT.md` 那一条改成《成员源哈希 (MemberSource)》，引到它的几处跟着改
- [x] tEXt 键一个字不变；幂等用例照旧绿
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 一次全仓改名，只有改名与随之改的文档（收停车场 Q699）。输出一个字节不变：tEXt 键 `tonefit:page-source` 一个字没动。

1. **类型**（`src/metadata.rs`）：`PageSource` → `MemberSource`，装它的那一批 `PageSources` → `MemberSources`（每个成员一份，硬约束的 grep 也扫得到它）。
   类型文档开头改成《成员源哈希》的说法——每个成员一份，页上写的就是这一份，透传文件那一份只拿去比、不进记录，名字因此说「成员」不说「页」（注释指着 Q699）。
   `pipeline.rs` 的引入、幂等那一道喂哈希、`nothing_else_changed` 比透传文件、两处文档链接跟着改。
2. **词汇表**（`CONTEXT.md`《输出》表）：《页级源哈希 (PageSource)》→《成员源哈希 (MemberSource)》，位置不动；引到它的《源哈希》《页级依据》（两处）跟着改。
   「哪些页有」那一句（默认路径与覆盖顶死那一趟有，`--envelope` 那条路与坏页的空白占位页没有）**照原字搬过来**——它在 `--envelope` 上门处处不成立、只点灰阶档位的那一卷对不上，那是 **Q1082，仍待拍板**，本票不碰；
   Q1082 的 Where 写的《页级源哈希 (PageSource)》今天叫《成员源哈希 (MemberSource)》（停车场是历史记录，原文没改）。
3. **注释与断言里的旧词**：凡指这一份哈希的「页级源哈希」（与简称「页级哈希」）改成「成员源哈希」，钉写法的那条单元用例改名 `the_member_source_has_a_frozen_definition`——
   票面「旧名在仓库里一处不剩」与《写代码前》「测试名取自词汇表」都要它。「页级」作为作用域的说法（《页级依据》、`SourceHash::Page`、「页级那条路」）照留。
   跟着 tEXt 键起名的 `PageRecord::page_source`、`PAGE_SOURCE_KEYWORD` 与用例里的局部变量照留（Q1347）。

**没动的**：tEXt 键与它的读写；用例一条没添（全部用例照旧绿就是行为没变的证明，约束由编译器守）；`.scratch/` 里的旧票与停车场条目照旧写 `PageSource`（历史记录）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 0861a65`。Spec 轴：缺项、越界、做错的都没有；Q1082 那一句新旧逐字相同。

**收下的**：

- 词条末句丢了「在页级那条路上」这个限定（Spec）——补回：「透传文件在页级那条路上那一份只拿去与输出里那一份比，不进记录」。
- 头一版另记了一条「注释与断言里的旧词也一并改」的岔口（Standards）：另一条路留下词汇表里没有的词，违反票面与《写代码前》，只有一条路说得通——不算岔口，撤掉那条，写进上面第 3 条。
- Q699 拍过板、约束着这个类型的名字，类型文档却没指着它（停车场《拍过板的结论要进代码旁边》）——补上「（停车场 Q699）」。
- 类型文档开头与后面「名字说成员」那一段同一个结论说两遍、词条把《成员》的定义抄了一遍（单一出处）——并成开头一段；词条改成「每个《成员》一份」。
- `Placement::page` 的文档同一段里前一句「页级那一份源哈希」、后一句「成员源哈希」——统一成后者。
- 剩下那一条的选项 ② 把常量 `PAGE_SOURCE_KEYWORD` 也列进去，那是稻草人（它就是键的名字）——收窄到字段与局部变量，补上 Why it matters。

**驳回的**：

- 改 Q1082 的 Where（Standards，稳定引用）：协调人定了 `.scratch/` 里的票据与停车场是历史记录、不改；新名字在上面第 2 条接上。

### 停车场

本票用了 Q1347：

- **Q1347**：跟着 tEXt 键起名的 `PageRecord::page_source` 字段与用例里的局部变量照留，没改成 `member_source`。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-13.gate1.log`、`os-13.gate2.log`、`os-13.gate3.log`、`os-13.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
本票没添用例，只改了一条单元用例的名字。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1146 通过 1 失败**（1 ignored）；lib 253 / bin 478；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 67.14s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1005 通过 1 失败**；lib 253 / bin 337；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.54s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.49s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**幂等用例**：`tests/idempotency.rs` 46 条全过（窄跑一趟 182.75 秒，闸门 1、2 上照旧全过）。
**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 150.78 秒、闸门 2 上 180.23 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 478 条里，全绿；本票没碰会话与设计稿。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q699 — 透传文件在页级那条路上的那一份哈希借用了 `PageSource` 这个类型：「页级源哈希」词条本义是「这一张来自的那个源成员」

- **From:** 票 `two-pass-rework/15`
- **Kind:** 命名／类型借用——code-review（Standards 轴）指出的 Mysterious Name
- **Where:** `src/metadata.rs` 的 `PageSources::extras: Vec<Option<PageSource>>` 与 `PageSource` 的类型文档（「『页级』说的是作用域——一个成员一份」那一句）；
  `src/lib.rs` 的 `nothing_else_changed`（输出里那一份读回来按 `PageSource::of` 重算再比）；`CONTEXT.md`《页级源哈希》末句
- **Why it did not block:** 算法与写法确实是同一个（`SourceHasher` 只喂这一个成员），透传文件那一份只拿去比、不进任何 tEXt，
  用户看不见它叫什么；给它另起一个类型就是同一段字节两个名字。词汇表《成员》本来就是「一页，或一个透传文件」，
  「一个成员一份」说得通，只是类型名里的 `Page` 与它对不上。
- **What this ticket actually did:** 借用 `PageSource`，在类型文档与《页级源哈希》词条各补一句「透传文件也各算一份，只拿去比、不进记录」。
- **Options:** ① 现状；② 类型改名 `MemberSource`、词条改成《成员源哈希 (MemberSource)》——tEXt 键 `tonefit:page-source` 不动
  （页上写的仍是页级那一份）；改的是一个已有词条的绑定与说法，按《改 CONTEXT.md 的规矩》先拍板，做起来是一次全仓改名；
  ③ 透传文件那一份换成 `SourceHasher` 直接收口的 `VolumeSource`（同一条规矩、另一个类型）——把「卷级」这个词借给透传文件，比 ① 更歪。
- **Recommend:** ②，单独派——一次改名，不该夹在这一票的语义改动里。
- **Whose call:** 拍板的人
- **处置：** **`one-source/13` 落地（2026-10-02）：照②了结。**`PageSource` 改名 `MemberSource`（容器 `MemberSources`），词条改成《成员源哈希 (MemberSource)》，tEXt 键 `tonefit:page-source` 不动、输出一个字节不变。
