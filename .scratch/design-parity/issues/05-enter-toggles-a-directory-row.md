# 05 — `⏎` 在目录行上是开关

**What to build:** 目录行上按 `⏎`：展开着就收起，收着就展开（设计稿就是这样）。`l` 照旧只展开、`h` 照旧收起；
双击等于 `⏎`，跟着变。卷行上的 `⏎` 不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 按键表给目录行上的 `⏎` 派一件自己的事；`l` 照旧只展开，卷行上的 `⏎` 照旧进每页结果（`Deed::Toggle`，表上派在整张卷列表上、目录行上收不收由状态机按行分，Q1287）
- [x] `ended-h-Enter-Enter` 接上、比整屏且绿；`running-dblclick-dir` 照旧绿
- [x] 双击一个展开着的目录行收起它：经终端层喂两次单击，断言屏上那一行变回收着
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 红的仍只有 macOS 基线那一条，Q995）

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q806 — `⏎` 在目录行上是**切换**，而按键表里 `l` 与 `⏎` 派的是同一件事

- **From:** 票 `session-redesign/08`
- **Kind:** 票面写错了（「目录行 `l`／`⏎` 展开、`h` 收起」在 `⏎` 上不准）
- **Where:** `design.html` 的 `taskKey`（`if (S.expanded.has(row.d.id) && k === 'Enter') S.expanded.delete(...)` ——`l` 恒展开，`⏎` 展开着时收起）；`src/session/keymap.rs` 里 `l` 与 `⏎` 两行都派 `Deed::Open`
- **Why it did not block:** 没有一串序列踩到它——`running-dblclick-dir` 是双击（16 号票的鼠标），键盘上没有一串在展开着的目录行上按 `⏎`。`h` 收起那条路两边一致
- **What this ticket actually did:** `Deed::Open` 一律**展开**（`l` 的那一支），收起只走 `h`（`Deed::Close`）；票面第三条那条用例因此写成「`l` 展开、`h` 收起」
- **Options:** ① 照旧：`⏎` 与 `l` 同义，收起只有 `h`——按键表一格不动，屏上也没有一处写着 `⏎` 会收起 ② 表上给 `⏎` 另派一件 `Deed::Toggle`，状态机多一支 ③ 设计稿改成 `⏎` 也只展开，重导 `running-dblclick-dir`（双击等于 `⏎`，那一串跟着变）
- **Recommend:** ①，直到鼠标那一票（16）真撞上双击一个展开着的目录行为止——那时按②或③收
- **Whose call:** 16 号票的实现者
- **处置：** **本票了结，照票面（原选项 ②）**：`⏎` 那一行改派 `Deed::Toggle`，目录行上展开着就收起；`l` 照旧只展开，卷行与备注行上 `⏎` 与 `l` 同；设计稿没碰。见《落地记录》。

#### Q853 — `ended-h-Enter-Enter` 这一串要 `⏎` 在展开着的目录行上收起它，而表上 `l` 与 `⏎` 同义

- **From:** 票 `session-redesign/10`
- **Kind:** 票面没想到的第三种情形（**Q806 的续**：那一条判的依据变了）
- **Where:** `tests/fixtures/design/sequences/ended-h-Enter-Enter.*`；`src/session/keymap.rs` 里 `l` 与 `⏎` 两行都派 `Deed::Open`
- **Why it did not block:** 那一串的期望屏上 `集英社/海贼王` 是 `▸`（收着的）——`h`、`⏎`、`⏎` 走完，第二下 `⏎` 收起了它。Q806 记着这件事，而它的《Why it did not block》写的是「**没有一串序列踩到它**——键盘上没有一串在展开着的目录行上按 `⏎`」：**那一句不成立了，这一串就是**（它在 `ended` 那个场景上，08 只走了 `running` 那几串）。同一串的前三步（`ended-h`／`ended-h-l`／`ended-h-Enter`）这一票都接了、逐格相等
- **这一条是 Q806 的续**：同一件事、同一处代码，差的是**前提**。Q806 的 `Recommend` 是「照旧，直到鼠标那一票（16）真撞上双击一个展开着的目录行为止」，它的依据是「没有一串序列踩到它」——**那条依据今天不成立了**：键盘上有一串踩到了，而它不必等 16 号票开工。**Q806 那一条本身没动**（停车场只往里写、不往外清）；`/settle` 按批了结时请把这两条**一起读**
- **What this ticket actually did:** 这一串**没接**，用例上写清是 Q806；`Deed::Open` 仍一律展开（`l` 那一支）。前三步（`ended-h`／`-h-l`／`-h-Enter`）接了、逐格相等
- **Options:** ① 照现状：Q806 的①（`⏎` 与 `l` 同义）留着，这一串挂着 ② 按 Q806 的②给 `⏎` 另派一件 `Deed::Toggle` ——那一串跟着绿，而 `running-dblclick-dir`（16 号票的双击等于 `⏎`）也要它 ③ 按 Q806 的③改设计稿
- **Recommend:** ②，与 16 号票的双击一起做：那一票本来就要判这件事，而现在**键盘上也有一串踩到了**，②的理由比 Q806 当初写的时候硬
- **Whose call:** 16 号票的实现者（Q806 就是交给他的）
- **处置：** **本票了结，照推荐 ②**：`ended-h-Enter-Enter` 接上、整屏逐格相等；`running-dblclick-dir` 照旧绿，另添一条双击展开着的目录行收起它。见《落地记录》。

## 落地记录

**本票做了什么。** 卷列表上的 `⏎` 在按键表上派它自己的一件事 `Deed::Toggle`（名字取自 Q806、Q853 的选项②），`l` 照旧派 `Deed::Open`。设计稿没改，也没重导；`src/session/scene.rs`、`CONTEXT.md` 一字未动（《展开》本来就写着「展开着时再按 `⏎` 收起」，实现这回跟上了）。

| 光标那一行 | `l`（`Deed::Open`） | `⏎`／双击（`Deed::Toggle`） |
|---|---|---|
| 收着的目录行 | 展开 | 展开 |
| 展开着的目录行 | 仍展开着（一格不动） | **收起**，光标不动；做的与 `h` 落在目录行上一样（`collapse_directory`） |
| 卷行 | 进每页结果，展不开的屏底说为什么 | 同 `l`：终端层那一支认 `Open | Toggle` 两件 |
| 备注行 | 掀开说明卡 | 同 `l` |

- **表上只多一件、屏上一格不变**：全部按键那一张按长的那一句并行，`l ⏎ 展开文件夹 / 查看每页结果` 仍是一行。备注行屏底那一件 `⏎ → 查看` 改为向 `Deed::Toggle` 要（`open_want`），目录行与卷行那两件仍向 `l` 要。`spelt_for(Deed::Open)` 照旧答 `l`，`tests/single_source.rs` 那几条扫描照旧绿。
- **表上没有「光标那一行是哪一种」这一维**：`⏎` 派在整张卷列表上，目录行上收不收由状态机 `toggle_under_cursor` 按行分（Q1287）。
- **双击等于 `⏎`**：终端层把双击当成一个 `⏎` 再交一次，跟着变了，一行没改。
- **代价**：屏上没有一处说 `⏎` 会收起——全部按键那一张写「展开文件夹 / 查看每页结果」，目录行屏底写 `l → 展开`（展开着时也这么写）。两处都照设计稿逐字，要改得先改设计稿。
- **用例**（`src/session/terminal.rs` 的 `redesign`，都经输入入口喂、比的是屏）：
  - 新 `enter_collapses_an_expanded_directory_row_and_l_leaves_it_expanded`：`ended-h-Enter-Enter` 接上、整屏逐格相等；`ended-h-l` 走完再按一次 `l`，整屏仍与 `ended-h-l` 那一屏逐格相同（`l` 只展开，没有一串设计稿钉着这一下）。
  - 新 `a_double_click_on_an_expanded_directory_row_collapses_it`：走完 `running-dblclick-dir`，在同一处经本层再喂那一步（两下单击，每一下之前照真会话画一帧、交出点得中的区域）。断言那一行从 `▾` 变回 `▸`、别的字不变；整屏与 `running-click-row`（同一处单击一下：选中、收着、自动滚动暂停）逐格相同。开头先核两串点的是同一处。
  - `running-dblclick-dir` 照旧在 `the_wheel_the_click_and_the_double_click_walk_to_the_designed_screens` 里；`h_collapses_the_directory_of_the_volume_and_l_opens_it_again` 只删了「这一串没接」那段文档。
  - `walked` 里逐个喂输入那一段抽成 `feed_step`，两条新用例接着走时用的是同一手。

### 按反跑过的几遍（每一遍改一处、跑 `cargo test --bin tonefit session::`、还原，还原后 `git diff --stat` 核过）

| 按反 | 结果 |
|---|---|
| 实现之前（`⏎` 仍派 `Deed::Open`）先落 `ended-h-Enter-Enter` | 红：与设计快照 `ended-h-Enter-Enter` 对不上（第 8 行，那一行的记号与颜色） |
| 表上 `⏎` 那一行改回派 `Deed::Open` | 红 3：`enter_collapses_…`（`ended-h-Enter-Enter` 第 8 行）、`a_double_click_…`（同一行只换了那一枚记号）、`enter_on_a_note_row_…`（`ended-note` 屏底 `⏎ → 查看` 那一件不见了）。`running-dblclick-dir` 照旧绿：那一串只展开 |
| `Deed::Open` 也改成开关（`l` 走 `toggle_under_cursor`） | 红 1：`enter_collapses_…`（第二下 `l` 收起了它，与 `ended-h-l` 第 8 行对不上） |
| 终端层卷行那一支去掉 `Deed::Toggle` | 红 1：`l_on_a_volume_row_opens_the_pages_…` 里的 `ended-Enter`（`⏎` 落在卷行上不进每页结果了） |
| `toggle_under_cursor` 展开着那一支改成展开（表照旧派 `Deed::Toggle`） | 红 2：`enter_collapses_…`、`a_double_click_…`；`ended-note` 照旧绿（备注行走的不是这一支） |

### 评审收了什么、驳了什么

两轴各派一个只读的子代理，看的是 `git diff 8bf3d27`（还没提交的工作树）。

**收下的**：

- **用例断言了按键表派哪一件、状态里的 `expanded`**（Spec：本 effort spec《什么是好测试》，只断言外部行为、整屏）。删掉 `keymap` 那条（`Some(Deed::Toggle)`）与 `view` 那条状态机用例，删掉读 `task.expanded` 的辅助函数；`l` 那一下改成比整屏（`ended-h-l`），双击那一条添一道整屏比对（`running-click-row`）。代价：闸门 2 不再有本票的用例（屏在 `tui` 后面）。
- **「光标那个目录行展开着」那个判断写了三遍**（Standards，Duplicated Code）：上一条删完只剩 `toggle_under_cursor` 那一处。
- **双击那条用例照抄了 `walked` 逐个喂输入的那一段**（Standards）：抽成 `feed_step`，`walked` 也改用它。
- **拍过板的条目号没进代码旁边**（Standards，停车场文件头）：`Deed::Toggle` 的文档点名 Q806、Q853，并指向 Q1287。
- **「同一件」与「件就是 `Deed`」对不上**（Standards）：终端层那句改成「卷行上 `l` 与 `⏎` 做的事相同」，`toggle_under_cursor` 那句改成「与 `h` 落在目录行上做的一样」。
- **旧用例名管不着「`⏎` 收起」**（两轴）：拆出新用例 `enter_collapses_…`，旧那一条退回原样。

**驳下的**：

- **`Toggle` 与 `TogglePath` 挨着，而它在卷行、备注行上并不切换；先例 `ConfigEnter` 是按块加键起名的**（判断题）：名字照 Q806、Q853 选项②的原话，票面两条复选框都按这个名字写；它在卷行、备注行上与 `l` 同，这一点文档第一句就说了。
- **Q1287 少一格 Why it matters**（轻）：那一格不是必填，相邻的几条也都没写。
- **票面第一条「给目录行上的 `⏎`」没照字面做**（Spec 判：行为与设计稿 `taskKey` 一致，已记 Q1287，不算缺）。

### 数

下面是评审收完、改完、`cargo fmt` 过之后跑的**那一趟**，也就是最终状态。四条顺序跑，
日志是 `dp-05.gate1.log`、`dp-05.gate2.log`、`dp-05.gate3.log`、`dp-05.polish.log`，都在树外。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`。这是平台带来的，本票没碰它（Q995）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1135 通过、1 失败**；lib 252，bin 472（新添 2 条，1 条 ignored 不变）；红的那一个二进制是 `concurrency`：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.04s` |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **994 通过、1 失败**；lib 252，bin 331（两条新用例都在 `tui` 后面，不在这一趟）；红的那一个二进制是 `concurrency`，同一条：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 57.98s` |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 都没有告警；`cargo doc` 告警 15 条，与基线同数 |
