# 13 — 摊开这个环节在屏上

**What to build:** 要摊开的卷，屏上的环节名显示**摊开**——当前卷那一行、横条、`?` 那张表里的环节一节三处逐字相同；
它的种类色由设计稿给一色；不摊开的卷屏上照旧三个环节。设计稿先加这一色与一屏有一卷正在摊开的景，再改实现。

库里 `Pass` 多出摊开那一个取值、卷级计时多一段，是 `say-and-stop` 那一批的活（拷问结论 2）；本票只让屏上跟着。

**Blocked by:** 01；`say-and-stop/03` — 摊开成为第一个环节

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 要摊开的卷走在摊开那一段时，当前卷那一行与横条写「摊开」，颜色照设计稿；`?` 那张表的环节一节列出它
- [x] 不摊开的卷屏上照旧三个环节
- [x] 导出一屏有一卷正在摊开的景，比整屏且绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 先改设计稿、重导，再让实现跟上（ADR 0019 决定第 13 条）。

| 处 | 设计稿（`.scratch/session-redesign/design.html`） | 实现 |
|---|---|---|
| 环节 | `PASSES` 四个（摊开 · 查重 · 分析 · 写出），`PASS_ST` 按名字给色，`PASS_WHAT` 一句「在做什么」；`passesOf(v)`：`.rar`／`.7z` 的卷走四个，其余走后三个；卷状态的 `pass` 是这一卷自己那几个里的第几个 | 新模块 `session::passes`（特性外面）：一张表装四个环节的词与那一句，`passes::name` 给横条前那个词（原 `marks::pass_name` 挪过来），`passes::legend` 给全部按键那一张（Q1131） |
| 摊开那一色 | `c-green`；页面图例「步骤」那一句添上它 | `paint::colour_of`：`Kind::Pass(Pass::Extraction)` → `Color::Green`（Q1127） |
| `?` 那张表的环节一节 | `KEYMAP` 在灰阶写法之后加「环节」一节，四行，派得出的阶段 `RD` | `cover::groups` 在 `Running`／`Deciding` 两档接这一节（Q1128：那一节原本没有，随旧界面退场了） |
| 一屏有一卷正在摊开 | 新景 `extracting`（正在摊开）：`~/下载/虫师/第0N卷.rar` 三卷排在处理路径最前，前两卷做完、第03卷摊开到一半；假盘添这三卷；归档卷名去扩展名认 `.cbz`／`.zip`／`.rar`／`.7z` | 场景夹具：`Listed` 多一格 `extracts`，`Listed::passes` 按它挑这一卷走哪几个环节；跳过、没做成、收摊、在跑的卷都照它喂 `PassStarted`（Q1130） |
| 导出 | 快照添 `extracting` 两种尺寸；场景数据只在要摊开的卷上写 `extracts: true`；`help-j` 那一句跟着成了「1–31 of 31」；「11 个场景」的几处改成 12 | 「11 个场景」的几处计数改成 12 |

- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过——既有的变动只有 `help` 两份快照、`help-j`、`help-narrow-*`、`deciding-help` 几串
  （环节一节在跑着与等待确认两档出）与 `manifest.json`；`running`、`deciding`、`envelope` 等不摊开的景一格没动。新添 `extracting` 两份快照与它的场景数据。
  node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。
- **用例**：
  - `shell`：新 `the_extracting_scene_matches_its_design_snapshot_wide_and_narrow`（120×36 与 80×24 比整屏：总览当前卷那一行、目录行与卷行行尾都写「摊开」，词与横条绿）。
  - `cover`：新 `the_sheet_explains_the_passes_only_while_they_are_on_screen`（跑着与等待确认两档按走的次序列四行，其余三档没有这一节）；
    `the_sheet_and_the_footer_both_come_from_the_key_table` 认环节那一节是图例、不是表上的行。
  - `passes`：`the_pass_says_what_it_does`（从 `marks` 挪过来，添摊开）、`the_sheet_lists_the_four_passes_in_the_order_they_are_walked`。
  - `help` 两份快照与 `help-*`、`deciding-help` 那几串照旧比整屏（期望屏是新导的）。
- **抬头「预计还要多久」**：这一景头一回停在第02卷（总进度 1.7%），设计稿写「正在估算剩余时间」、实现报出一个数；挪到第03卷之后又差 0.5 秒跨了取整。
  那一句两边算法本来就两处不同，不在本票里判（Q1129），这一景挑在两边都落在 `15m01s` 的那一刻（摊开的模拟速度定成分析的 3 倍）。
- **`CONTEXT.md` 没改**：《环节》那句「横条上那个词、`?` 那张表、这里三处逐字相同」至此成立（ss/03 记的 Q1099）；
  《覆盖层》列的分组还只写「外加灰阶写法」，改它是改已有词条，等拍板（Q1128）。

### 按反跑过的几遍（实现落地之后按反、看见红、还原）

| 按反 | 结果 |
|---|---|
| `cover::groups` 环节一节五档都出（`if true`） | 红 5 条：`the_sheet_explains_the_passes_only_while_they_are_on_screen`；`question_mark_before_the_run_lifts_the_key_sheet`（「fresh-help 第 21 行第 61 格：实际『环』，期望『 』」）、`f1_while_typing_…`、`the_key_sheet_at_the_end_…`、`the_wheel_scrolls_the_key_sheet_…` |
| `Listed::passes` 一律走四个 | 红：`the_running_scene_matches_…`（「running.120x36 第 1 行第 32 格」，总步数对不上） |
| `Listed::passes` 一律走三个 | 红：`the_extracting_scene_matches_…`（「extracting.120x36 第 3 行第 23 格：实际『查』，期望『摊』」） |

写实现之前那一遍红：`extracting` 那两份快照先是字不同（夹具不认 `extracts`，屏上写「查重」），接上夹具之后是色不同（「第 3 行第 23 格前景色：实际 reset，期望 green」），
`help` 快照是「第 2 行第 61 格：实际『每』，期望『配』」（那张表少一节、两栏劈在别处）。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 1b9ced8`（未提交的工作树）。Spec 轴：三处逐字相同成立、新景有比整屏的用例、既有快照的变动只因本票。

**收下的**：

- **`CONTEXT.md`《覆盖层》改了已有词条**（两轴都点了，CLAUDE.md《改 CONTEXT.md 的规矩》）：撤回，转记进 Q1128。
- **夹具里几处注释还用旧称「遍」**（Standards，CONTEXT《环节》）：本票碰过的那几处改成环节（`begin_pass` 那句「另两遍直接开」摊开之后也不对了）。
- **`through_the_check` 重写了 replay 里那个走步的闭包**（Standards，Duplicated Code）：收成一个 `stepped`，两处共用；函数改名 `up_to_the_fingerprint`（词表叫查重，不叫 check）。
- **`passes::listed` 与夹具的 `Listed` 同名不同义**（Standards，Mysterious Name）：改叫 `passes::legend`（它就是环节那一节图例）。
- **横条的分母是页数，库按成员报摊开的步**（Spec）：记 Q1132。

**驳回的**：

- **夹具的 `PASSES` 又抄了一份走的次序**（Standards，Duplicated Code）：它是场景数据 `pass` 那个数的编号表（设计稿 `passesOf`），与屏上的词表各管各的；
  合成一份就得让 `passes` 为夹具敞一个只有测试用得着的常数。
- **`listed.passes()[volume.pass as usize]` 出现三处**（Standards，Feature Envy）：两处在用例里、一处是夹具的断言，各读各的；不为它另起方法。
- **加一景要改九处「11→12」**（Standards，Shotgun Surgery）：沿用既有的写法（几处计数各钉一层），不在本票里改。
- **设计稿三张平行表、按环节名字符串分支**（Standards）：原型的写法，与它其余的表一致。
- **为凑同一秒调了摊开的模拟速度，不稳**（Spec）：认；根子在 Q1129，修好那一句之后这一景随便停在哪一刻都行。

### 停车场

本票用了 Q1127–Q1136 里的六个：

- **Q1127**：摊开那一色挑了绿（拍板的人，设计稿）。
- **Q1128**：全部按键那一张补上环节一节，只在跑着与等待确认两档出；《覆盖层》那一串分组没改。
- **Q1129**：抬头「预计还要多久」设计稿与实现两处算法不同；新景挑在取整到同一秒的那一刻。
- **Q1130**：场景数据里「要摊开」记成清单上一格 `extracts`，`pass` 数这一卷自己那几个环节。
- **Q1131**：环节的词挪进自己一个模块 `session::passes`。
- **Q1132**：摊开那一段横条的分母是页数，库按成员报步。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-13.gate1.log`、`dp-13.gate2.log`、`dp-13.gate3.log`、`dp-13.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1060 通过 1 失败**；lib 239 / bin 437（另 1 ignored）；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.37s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **942 通过 1 失败**；lib 239 / bin 319；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.09s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.55s` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；四项全绿，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `1b9ced8`：闸门 1 **1057 通过 1 失败**，闸门 2 **939 通过 1 失败**，红的是同一条
——按 `design-parity/06`（1050／932）加 `say-and-stop/03`（两趟各多 7 条，两者都在 `1b9ced8` 里）推得，没在 `1b9ced8` 上重跑。
**两趟各多 3 条，都在预期里**：`passes` 两条与 `cover` 的 `the_sheet_explains_the_passes_only_while_they_are_on_screen` 两趟都编；
默认那一趟另多 `shell` 的 `the_extracting_scene_matches_…`、少 `marks` 的 `the_pass_says_what_it_does`（挪进 `passes`）。

**黄金快照逐格没动**：`git diff 1b9ced8 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。
