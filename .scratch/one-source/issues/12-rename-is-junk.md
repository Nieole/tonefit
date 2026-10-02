# 12 — `is_junk` 改名

**What to build:** `is_junk` 判的是卷内成员两件事：躺在不看的地方里的，与打包环境留下的那几个文件。名字只说得出后一半。
改成说得出两半的名字（与目录那一侧的「不看的地方」同一个词），三处文档、五个调用点与那条跟着它取名的用例一起改。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 成员那一侧的谓词名字里有「不看」这个词，与目录那一侧对上
- [x] 三处文档、调用点、那条用例名一起改；旧名在仓库里一处不剩
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 成员那一侧的谓词 `is_junk` 改名 `is_ignored_member`（收停车场 Q477）。行为一格不变：函数体、调用处的条件与用例断言一字没动。

1. **谓词**（`src/source.rs`）：`fn is_junk` → `fn is_ignored_member`，函数体一字没动；与目录那一侧的 `is_ignored_directory`、名单 `IGNORED_DIRECTORIES`、
   词条英文名 `IgnoredPlace` 同一个词（取名的岔口见 Q1337）。
2. **调用点**：三个——`open_directory`、`open_archive`、`solid_members`（`.7z` 与 `.rar` 两路共用它）。票面写「五个调用点」，是 Q477 记下时的数；
   今天 `.7z`、`.rar` 两处已并进 `solid_members`，量出来是三个，三个都改了。
3. **文档**：指着它的五处都改了——票面那三处（`solid_members`、`strip_wrapper_directory`、`src/survey.rs` 的 `nothing_took_it`），
   外加名单 `IGNORED_DIRECTORIES` 与 `is_ignored_directory` 自己的文档各一处。四处 rustdoc 链接都解析得到，`cargo doc` 告警数不变。
4. **用例**：`tests/container.rs` 的 `a_directory_volume_ignores_the_same_system_junk` → `a_directory_volume_ignores_the_same_sidecars_and_index_files`，
   抬头那句「同一批垃圾」改成「打包环境留下的同一批边车与索引文件」（《成员》词条原话），断言没碰。
5. **顺带的散文**（Q1338）：`src/source.rs` 里六句「垃圾成员」改成「不看的成员」，`is_one_of` 的闭包参数 `junk` → `listed`。`JUNK_FILES` 没动——它装的正是打包环境留下的那几个文件。

**没动的**：`CONTEXT.md` 一字没动——《成员》词条只说了打包环境那一半，改写已有词条要先拍板，记 Q1339；`.scratch/` 里指着旧名的历史票据与 spec 照旧；没添用例
（纯改名，约束由编译器守，全部用例照旧绿就是行为没变的证明）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff d615eac`。Spec 轴：九处（定义一、调用点三、文档五）全改到，`src`、`tests`、`docs`、`CONTEXT.md`、`xtask` 里 `is_junk` 一处不剩；
谓词体与断言逐字没动；散文那一组算票面之外，已记 Q1338、理由站得住。

**收下的**：

- 用例头一版取名 `a_directory_volume_ignores_the_same_members`（两轴都指到）：照《成员》词条，边车与索引文件**不算**成员，名字却叫它们 members，
  而同一个文件里 `both_container_shapes_hold_the_same_members_after_a_page_is_deleted` 的 members 是真成员——改成 `…_sidecars_and_index_files`，取《成员》原话。
- Q1337 原先只列谓词的候选，没列用例名的（Standards）——补上用例名的三个候选与头一版被驳的理由。
- Q1339 的选项里有一个占位链接 `[…](#)`（文档写作第 5 条，稳定引用）——改成直接写词条名。

**驳回的**：

- Q1337 谓词那一半「Q477 已推荐过，不算岔口」（Standards）：派活说明把取哪个英文名点成岔口、要求记一条说清另外一两个名字；条目里注明了这一层。

### 停车场

本票用了 Q1337–Q1339：

- **Q1337**：谓词取 `is_ignored_member`、用例取 `…_ignores_the_same_sidecars_and_index_files`，各自考虑过的另外两个名字。
- **Q1338**：散文里的「垃圾成员」与 `is_one_of` 的闭包参数一起改。
- **Q1339**：《成员》词条只说了打包环境那一半，没顺手改，待拍板。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt --check` 过之后，四条顺序跑。日志是 `os-12.gate1.log`、`os-12.gate2.log`、`os-12.gate3.log`、`os-12.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
条数与基线（d615eac）相同：本票没添用例，只改了一条的名字。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1145 通过 1 失败**（1 ignored）；lib 253 / bin 478；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 79.51s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1004 通过 1 失败**；lib 253 / bin 337；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.46s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.73s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 288.06 秒、闸门 2 上 149.02 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 478 条里，全绿；本票没碰会话与设计稿。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q477 — 名单与目录那一侧的谓词改口了，成员那一侧的 `is_junk` 名字没跟着改

- **From:** 票 `no-false-line/07`
- **Kind:** 落地时认下的（一次改名只做了一半）
- **Where:** `src/source.rs` 的 `is_junk`，三处指着它的文档（`solid_members`、
  `strip_wrapper_directory`、`src/survey.rs` 的 `nothing_took_it`），
  以及跟着它取名的那条用例 `tests/container.rs` 的
  `a_directory_volume_ignores_the_same_system_junk`（连同它那句「同一批垃圾」）
- **Why it did not block:** 名单与**目录**那一侧已按词汇表改成 `IGNORED_DIRECTORIES`／
  `is_ignored_directory`（词条《不看的地方 (IgnoredPlace)》）。`is_junk` 判的是**卷内成员**，
  它今天收两样：躺在不看的地方里的，与打包环境留下的那几个文件（`JUNK_FILES`、AppleDouble
  边车）。名字只说得出后一半。`CLAUDE.md` 那条「类型名一律取自 `CONTEXT.md`」管的是
  类型名、模块名、测试名与 issue 标题，私有函数名不在其中，因此不算破规矩。
- **What this ticket actually did:** 文档改口了（第一句从「这个成员是不是打包环境留下的垃圾」
  改成「这个成员是不是不看的东西：躺在不看的地方里，或者本身就是打包环境留下的那几个文件」），
  名字一个字没动——票面写的是名单与判据，没写改名，而三处引用今天各自那句话都还成立。
- **Options:** ① 原样留着，靠第一句说清两半；② 改成 `is_ignored_member`，连同三处文档
  与五个调用点一起；③ 拆成两个谓词（不看的地方一条、打包环境留下的文件一条），
  调用点各按各的意思拼。
- **Recommend:** ②。一次改名，diff 落在三处文档、五个调用点与一条用例名上，
  而它把「不看」这个词从目录那一侧贯到成员那一侧——今天读代码的人会以为卷内那条判据
  只管打包垃圾，于是往名单里加系统目录时不会想到它在**两处**同时作数
  （Q478 说的正是这一半）。本票的 `/code-review` 两轴都独立指到了这一处
  （Standards 轴判为 Mysterious Name 并直接点名方案 ②）。
  ③ 不选：两条判据在**每一个**调用点都同时要，拆开只是把一个 `||` 从函数里搬到五处去。
- **Whose call:** 落地的人
- **处置：** **`one-source/12` 落地（2026-10-02）：照②了结。**`is_junk` 改名 `is_ignored_member`（取名见 Q1337），三个调用点（`.7z`、`.rar` 两处已并进 `solid_members`）、五处文档与那条用例（今名 `a_directory_volume_ignores_the_same_sidecars_and_index_files`）一起改；散文里的「垃圾成员」一并改口（Q1338）；《成员》词条那一半记 Q1339。
