# 06 — 转轮一段算式，会话的时钟一个起点

**What to build:** 会话的时钟起点只有一格——视图那一层那一格；行首记号的转轮从它算、再加自己那一行的错相，算式只有一段。
场景夹具不再为两格起点各摆一次（Q864 那次合并撞出来的次序问题随之消失）。屏上一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 转轮的算式与时钟起点各只有一处；单一出处扫描（`tests/single_source.rs` 那一族）添一条钉住
- [x] 场景夹具只摆一个时钟起点
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 转轮只剩一段算式，会话的时钟只剩一个起点（收停车场 Q864）。

1. **一段算式**（`src/session/view.rs`）：`Views::spinning(now, offset)` 从 `Views::clock` 算，一格 90 毫秒、十格一圈，
   再往后挪这一行自己的错相。顶栏右端那一截与总览上清点那一条传零，卷列表那棵树（`shell/list.rs` 的 `spin`）传这一行的错相。
   `marks::spinner` 删掉；`SPINNER`／`SPINS_EVERY` 收成私有，别处够不着、抄件只能自带一张表。
2. **一个起点**：`Session::opened_at` 删掉，起点只在 `Views::clock`。真会话照旧由入口在第一帧之前摆下它（`terminal.rs`）；
   画屏的那几条用例原先就摆着 `Some(epoch)`，行首记号从前读 `opened_at`、截到零，现在读 `epoch`、也是零，同一格。
3. **夹具只摆一处**（`src/session/scene.rs`）：往回推那一条式子收进 `clock_of`；摆一个场景时它在 `views_of` 里面摆，
   整份 `Views` 换上去的那一下就带着它——Q864 那个「摆在 `views_of` 之前被抹掉」的次序从结构上没了。
   `advance_to` 存回界面状态之后按新那一份重新往回推（Q1317）。
4. **文档跟着改**：`marks.rs` 模块文档写明转轮出自 `Views::spinning`、本模块只收转出来的那一格；`shell.rs` 那张出处表的那一行指到它；
   `Views::clock` 接过从前 `opened_at` 文档里「转轮从会话那一头的钟算」那一段。

**用户看得见的变化**：没有。屏上一格不变（设计快照、交互期望屏全绿）；行首记号与总览对任何场景数据都与从前逐格相同。
只有一处会不同：推进几秒之后**在配置视图里**看顶栏那一截，它从前留着起点场景的起点，现在跟行首记号一起按新那一份算——
本仓库没有一串推进之后停在配置视图，屏上因此没变（Q1317）。

**用例**：

- `tests/single_source.rs` 新加 `the_spinner_and_its_clock_start_live_in_one_place`，问四件事：算式那三截（字形表、
  `SPINS_EVERY.as_millis()`、`% SPINNER.len()`）在代码里（`code_only`）恰好一处、就在 `view.rs`；`pub clock: Option<Instant>` 住在 `view.rs`；
  `opened_at` 在交付的代码与文档里哪儿都不在；顶栏、卷列表、总览三处真调 `views.spinning(`。
- 转轮那条单元用例从 `marks.rs` 挪进 `view.rs`（`the_spinner_turns_one_frame_every_ninety_milliseconds_and_wraps_at_ten`），
  多一句：没起过表时从第一格起、错相照加。它因此也在闸门 2 里跑，那三项的 `cfg_attr` 从 `expect(dead_code)` 换成 `allow`，
  与同文件特性外有用例读到的那几项（`number`、`name`、`output_shown`）同一种写法。
- `advancing_swaps_the_run_and_leaves_the_interface_alone` 原先断言整份 `Views` 不动，改成「起点之外一格不动」，
  另钉「此刻减起点恰是推进之后那一份的 `now_ms`」。

**按反跑过的**（`docs/agents/testing.md`）：

- 动手前新扫描红在算式那一截：`SPINS_EVERY.as_millis()` 在 `marks.rs` 与 `view.rs` 各一处。
- 落地之后逐个按反，每个都红：字形表抄回 `marks.rs`（两个文件各一处）；`view.rs` 里另写一段算式（家里两处）；
  `state.rs` 换回 4c87a1b 那一份（`opened_at` 复活）；`list.rs` 的 `spin` 改成手写 `"⠋"`（那一处不再调 `views.spinning(`）。
  头一遍把抄件追加在文件末尾、落在 `#[cfg(test)] mod tests` 之后，被 `code_only` 砍掉、照绿——那是放错了地方，挪到代码里再跑才红。
- `advance_to` 不再重新往回推：夹具那条红在 `1005.95s` 对 `1030s`。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 4c87a1b`（头一趟两个都断在网络上，重派一趟）。
Spec 轴：缺失无、越界无，屏上一格不变与 Q1317 的说法逐条核过；Standards 轴：硬伤无。

**收下的**：

- `shell.rs` 模块文档那张出处表，转轮那一行仍指向 `marks`（Spec）——改成指 `Views::spinning`。
- 夹具那条用例的文档写死了「24.05 秒」，那个数从场景数据推出来，数据一改就是一句假话（Standards，单一出处）——改成只说「不一样、不是整圈」，数字留在 Q1317。
- 记号 `"/ SPINS_EVERY.as_millis()"` 里那个 `/` 被 `squashed` 去掉，文档说「那一除」不实；头一段「散文里不带 `.as_millis()`」的论证在 `code_only` 下用不上（Standards）——
  记号去掉 `/`，文档改成为什么挑这三截（家里私有、抄件得自带表；家里另写一段得再除一次、再模一次）。
- 《落地记录》要写按反看见了什么（Standards，testing.md）——见上。

**驳回的**：

- 扫描文档与 `CLOCK_COPY_MARK` 写着「从前……现在」（《文档写作》第 1 条）：本文件那五条扫描都这么写，说的是这条扫描拦的是哪一种抄件，不是变更史。
- `clock_of(epoch, data)` 只读 `Data` 的两格（Feature Envy）：同文件 `elapsed` 也是自由函数，`Scene::now` 照同一副写法；`scene.rs` 是本轮的冲突热点，不为它挪。
- `clock`／`clock_of` 装的是起点，用例名 `…leaves_the_interface_alone` 与「起点会变」对不上（Mysterious Name）：`Views::clock` 是既有的名字，改名要动 `terminal.rs`、`measure.rs`、`shell.rs` 几处；
  起点不是输入摆出来的界面状态，用例文档写明了这一个例外。
- 扫描把 `views.spinning(` 这条访问路径当记号（Message Chains）：与 `the_stop_rules_live_in_one_place`、`the_case_probe_lives_in_one_place` 问「调用的形状」同一种做法。

### 停车场

本票用了 Q1317：

- **Q1317**：推进几秒之后，夹具的时钟起点跟着新那一份往回推，界面状态因此不再是「一格不动」（另一条路：留着起点场景那一格，此刻也全绿，只是凑巧）。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt --check` 过之后，四条顺序跑。日志是 `os-06.gate1.log`、`os-06.gate2.log`、`os-06.gate3.log`、`os-06.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。本栏读作：**除了这一条基线红，没有新增的红。**
条数：闸门 1 比 4c87a1b 多 1 条（新扫描；单元用例是挪的，默认构建里一出一进），闸门 2 多 2 条（新扫描，加上那条单元用例从特性后面挪到了特性外面）。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1145 通过 1 失败**（1 ignored）；lib 253 / bin 478；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 65.75s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1004 通过 1 失败**；lib 253 / bin 337；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.35s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.87s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 173.68 秒、闸门 2 上 246.01 秒），快照没动。
**设计快照**：`npm run check` 报「与库里那一份逐字节相同」；会话里比设计快照的那几景全绿。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q864 — 合并 08 与 13 时撞出两份转轮字形表，我在合并里收成了一处

- **From:** 队列协调人（合并 `sr/08` 与 `sr/13` 那一刻，不是任何一张票的落地途中）
- **Kind:** 票面没想到的第三种情形（两张票**彼此没有阻塞边**，各自那趟闸门各自全绿，合起来才有这件事）
- **Where:** `src/session/view.rs` 的 `SPINNER`／`SPINS_EVERY`（13 号票加的，`Views::spinning` 与顶栏读它）；
  `src/session/shell/marks.rs` 原先自己那一份 `SPIN`／`A_FRAME`（08 号票加的，行首记号与总览读它）
- **Why it did not block:** 两份是同样的十个盲文字形、同样的 90 毫秒。而 `marks.rs` 的模块文档第一句
  自己写着「三样各只有一处出处」，13 给 `SPINNER` 写的文档也写着「顶栏右端那一截、行首记号的「处理中」、
  总览上清点那一条，三处同一份」——**两份文档都在说一处，而盘上是两处**。
  `tests/single_source.rs` 那一族没有钉转轮，所以没有一条用例会红：闸门在两棵树上各自全绿，
  合起来也全绿，这件事只有人眼看得见。
  方向上只有一条路走得通：那一份必须摆在 `tui` 特性**外面**——`Views::spinning` 在特性外面、
  `marks.rs` 整个在特性后面，反过来摆会让闸门 2 编不过。
- **What this ticket actually did:** 合并里把 `view.rs` 的 `SPINS_EVERY` 开成 `pub`，
  `marks.rs` 丢掉自己那两份、改读 `view::{SPINNER, SPINS_EVERY}`，并在 `marks.rs` 模块文档里
  写明转轮那一样的出处在 `view`、本模块只多做「一行自己的错相（`offset`）」那一件。
  **行为一个字节没改**（同样的表、同样的周期、同样的错相算法）。
- **同一件事还有更咬人的另一半：时钟起点也是两格，而合并把它们摆错了次序。**
  `scene.rs` 的 `from_data` 里，13 加的 `session.views.clock = … − now_ms` 落在
  08 加的 `session.views = views_of(…)` **之前**——后者整份换掉 `session.views`，
  前者当场作废，`Views::spinning` 落回 `Duration::ZERO`。
  **两棵树各自的闸门都绿**：13 那棵树里没有 `views_of` 摆在那个位置，08 那棵树里没有 `views.clock`。
  只有合起来才红，而它红在一条真用例上——
  `during_a_run_the_config_view_is_readable_and_settles_nothing`，
  `running-2` 第 0 行第 63 格实际 `⠋`、期望 `⠙`，**差整一格转轮**。
  合并里的收法：两格起点都挪到 `views_of` **之后**，并从同一个 `origin`／`back` 算一次；
  08 的 `checked_sub` 与 13 的直减两种写法各自原样保留，两侧行为一个字节没改。
  另外我取 `view.rs` 导入并集时多带了 `DEVICE_FIELDS`／`TASTE_FIELDS`
  （13 的重构已把用到它们的代码换掉），clippy 报了一条 `unused_imports` 而 polish 照样退 0——
  **退出码在 clippy 这一条上说明不了干净**，已删。
- **还剩一件没收的：** 那**两段算式**仍是两处——`Views::spinning(now)`（从 `self.clock` 算、不带错相）
  与 `marks::spinner(now, opened_at, offset)`（带错相）；时钟起点也仍是两格
  （`views.clock` 与 `opened_at`，同一个值存两处）。收成一处要判「谁该拿着会话的时钟」，
  那是设计决定，不是合并该做的事。
- **Options:** ① 现状：数据一处，算式两处 ② `view` 里出一个带错相的算式，`Views::spinning` 与
  `marks::spinner` 都调它 ③ 退回两份数据各自留着——本轮不取，它与两处文档自己的话直接相抵
- **Recommend:** ②，归 15 号票（那一票让旧那一副退场、本来就要动这几处），或者鼠标那一票（16）真用上错相时顺手收
- **Whose call:** 15 号票的实现者
- **处置：** **`one-source/06` 落地（2026-10-02）：照票面了结（选项②，起点一并收成一格）。**算式只剩 `Views::spinning(now, offset)`，`marks::spinner` 删掉；起点只剩 `Views::clock`，`Session::opened_at` 删掉；夹具往回推那一条式子收进 `clock_of`、在 `views_of` 里面摆，次序从结构上没了；`tests/single_source.rs` 的 `the_spinner_and_its_clock_start_live_in_one_place` 钉住。推进几秒之后起点跟着新那一份换，记 Q1317。
