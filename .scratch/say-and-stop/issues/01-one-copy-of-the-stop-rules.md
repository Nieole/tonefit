# 01 — 停止规则在 bin 里只留一份

**What to build:** 「确认点上做完再停要让、立即停止不让」与那张升级表（继续 → 做完再停 → 立即停止 → 立即停止），
命令行与会话各抄了一份、名字逐字相同。收进 bin 里一个模块，两路都调它。库的对外形状一格不动——
闩的编码早已在库里，这是 bin 内部的收口。屏上与行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] bin 里只剩一份「确认点上让不让」与升级表；命令行与会话都调它
- [x] 两路原先各自的用例搬进这个模块：升级表逐级、确认点上两级各让不让
- [x] 命令行与会话各留一条用例，断言调的是它
- [x] 库的公开形状、屏上、命令行输出一格不变
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。**

1. **新模块 `src/stop.rs`**（bin 根上，与 `wrap`、`preset` 同一层，不挂特性）：`next`（升级表）、`at_the_decision_point`、
   `answer`（确认点上做完再停要让、立即停止不让）三个纯函数，不存东西——闩仍旧各在各处，编码仍出自 `Instruction::code`。
   摆在 `session` 外面，因为那个模块整个挂在 `any(feature = "tui", test)` 上，命令行在不带 `tui` 的那一趟里也要调它。
2. **命令行调它**（`src/main.rs`）：`Latch::press` 走 `stop::next`，`Bar::observe` 走 `stop::at_the_decision_point` 与
   `stop::answer`；本地那三份删掉。`Bar` 多一格 `latch: &'static Latch`，`Bar::new` 转调 `Bar::with_latch(named, &PRESSED)`，
   生产里恒读 `PRESSED`（Q1118）。
3. **会话调它**（`src/session/`）：`Session::raise_stop` 走 `stop::next`；`Watch::observe` 走 `stop::at_the_decision_point` 与
   `stop::answer`，本地两份删掉。评审 Spec 轴指出闸那一侧还有第三份——`Running::stop`「只推立即停止」与 `Watch` 等人那一支
   「立即停止短路、别的问人」都是同一条规矩的另写：两处改成从 `stop::answer(true, ..)` 推，让成继续的交给用户、不让的照原字，
   行为一格不变。
4. **用例**：
   - 搬进 `stop` 两条：`each_press_climbs_exactly_one_level_and_abort_is_where_it_stays`（逐级点名，不只问「不变弱」）、
     `the_finish_press_gives_way_at_the_decision_point_and_the_abort_press_does_not`。原先直接问函数的三条删掉：
     命令行 `the_latch_only_ever_goes_up` 的前半（后半只问字节，改名 `the_latch_stores_the_bytes_the_public_encoding_gives`）、
     命令行 `the_finish_press_gives_way_at_the_decision_point_and_the_abort_press_does_not`、会话 `only_the_decision_point_makes_a_finish_step_aside`。
   - **两路各一条「调的是它」**，都在一趟真跑上过各自的观察者：命令行新添
     `a_ctrl_c_in_the_middle_of_a_volume_still_lets_that_volume_land_whole`（一卷开工时 `Latch::press` 一下、交给真的 `Bar::observe`，
     断言那一卷整卷落盘——这张票之前，命令行观察者「确认点上让路」没有一条用例走得到）；会话沿用
     `finishing_in_the_middle_of_a_volume_still_lets_that_volume_land_whole`（`Watch::observe`），文档点名它。
     两路按键的那条（`one_ctrl_c_is_the_finish_press_and_two_is_the_abort_press`、`the_stop_latch_only_goes_up_and_leaves_with_its_run`）
     照旧写死逐级字面值，钉的是各自那个键的契约（Q1119）。
   - 会话新添 `a_finish_pressed_while_the_decision_point_waits_leaves_the_question_to_the_user`：已经等在闸上时按做完再停，
     那一问仍旧留给用户——第 3 条改的那一处从前没有用例守着（按反见下）。
   - `tests/single_source.rs` 新添 `the_stop_rules_live_in_one_place`：规矩的 `match` 臂只在 `src/stop.rs`、两路真调着它（Q1117）。
   - 命令行那条用的是会话那一份真卷（`session::fixture::a_real_volume`，`src/session.rs` 在 `cfg(test)` 下把 `live::fixture` 敞给 crate 根）。
5. **没动的**：库一个文件没碰；屏上、命令行输出、设计稿、场景数据一个字节没动；`CONTEXT.md` 不动（没有新概念，词条原有的
   做完再停、立即停止、确认点、闩照用）。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `stop::answer` 在确认点上不让、`stop::next` 起手跳到立即停止——`stop` 两条红；命令行按键那条红在「按一次不是做完再停」，
  命令行「调的是它」那条红在「闩该停在做完再停这一级」。
- `Bar::observe` 不过 `stop::answer`、直接回闩——命令行「调的是它」那条红在「盘上却没有它」。
- `Watch::observe` 不等人那一支直接回闩——会话 `finishing_in_the_middle…` 红在「盘上却没有它」；`raise_stop` 不走 `stop::next`
  （恒推到立即停止）——会话按键那条红。
- `Running::stop` 不问 `stop::answer`、按到哪一级就往闸上推哪一级——`session::run` 与 `session::terminal` 全绿（没有守卫）；
  补了上面那条新用例之后它红在「按了做完再停，闩替用户把确认点答掉了」。
- `single_source`：往 `state.rs` 抄回一份 `answer`——红在「长出了第二份」；`raise_stop` 不调 `stop::next`——红在「不再调」。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 695dca8` 加未跟踪的 `src/stop.rs`。

**收下的**：

- Spec：`Running::stop` 与 `Watch` 等人那一支是同一条规矩的第二份编码——改成从 `stop::answer` 推，补一条用例（见上第 3、4 条）。
- Spec：会话按键那条没搬、也没说自己是「调的是它」——文档点名它问的是 `s` 那个键的契约、表在 `stop`；取舍记 Q1119。
- Standards：命令行那条用例自己搓了一卷，与 `session::live::fixture::a_real_volume` 逐项相同、还丢了面板高那个名字——改用那一份。
- Standards：`Bar` 那一格叫 `pressed`，调用处读成 `self.pressed.pressed()`；构造器 `reading` 说不出读什么——改叫 `latch`、`with_latch`。
- Standards：`STOP_RULE_MARKS` 那三条 `Self::` 的理由照搬了闩编码那一条、在这里说不通——改成它真正防的去处（库里的 `impl Instruction`）。
- Standards：按反的结果要进《落地记录》——就是上面那一节。

**驳回的**：

- Spec：`Bar` 多一格注入口、`single_source` 断言源码字样——前者记 Q1118，后者照仓库那几条同形的先例，记 Q1117。
- Standards：`single_source` 里「carrying」那个形状又写了一遍——那个文件里每一条都这么写，是那个文件的写法。

### 停车场

本票用了 Q1117–Q1126 里的三个：

- **Q1117**：「只剩一份」另钉了一条扫文件的闸门，推荐留着。
- **Q1118**：命令行那条「调的是它」要过真的 `Bar::observe`，`Bar` 因此多一格点名的闩；列了按全进程那一份、起真进程、不加三条别的路。
- **Q1119**：两路按键那条照旧写死逐级字面值，没改成从 `stop::next` 推，推荐照旧。

结转两条（见《停车场结转》）：Q262、Q552，本票照上面的做法了结。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后；日志 `ss-01.gate1.log`、`ss-01.gate2.log`、`ss-01.gate3.log`、`ss-01.polish.log`，都在树外）。
这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1054 通过 1 失败**；lib 239 / bin 430（另 1 条 `ignore` 的量具）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.50s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **938 通过 1 失败**；lib 239 / bin 314；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.85s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行「全绿。」 |

**本票净添 3 条用例**：bin +2（`stop` 2 条、命令行 1 条、会话 1 条，减去搬走的 2 条），`tests/single_source.rs` +1（5 → 6）；
两条闸门各多 3 条，lib 没动。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 145.88 秒、闸门 2 上 149.85 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
设计快照：`npm run check`「与库里那一份逐字节相同」，设计稿与 `tests/fixtures/design/` 一个字节没动，闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`，末行「全绿。」）：`cargo fmt --check` 绿；`cargo clippy --all-targets` 绿；
`cargo clippy --all-targets --no-default-features` 绿；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q262 — 「决策点上收尾要让」与升级那张表，跟着也各成了两份

- **From:** 票 `p4-parking-lot/18`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/session/run.rs` 的 `answer`／`at_the_decision_point`、
  `src/session/state.rs` 的 `Session::raise_stop`；`src/main.rs` 新添的 `answer`／
  `at_the_decision_point`／`next`
- **Why it did not block:** **闩的编码变成三份不在本条账上**——那是 Q70 预告过的，
  去处也定了（`p4-parking-lot/19` 的票面写着「收停车场的 Q70」）。本条记的是它**之外**
  多出来的那一半：决策点上「收尾要让、中止不让」那条规矩，以及「继续 → 收尾 → 中止 → 中止」
  那张升级表，本票各复制了一份。两样都非有不可——会话那一份整个挂在 `tui` 特性后面
  （`src/session/` 在 `#[cfg(any(feature = "tui", test))]` 里），命令行这一路够不着；
  而库那一侧不该替调用方拿这个主意（等不等人、让不让都是调用方的策略，
  ADR 0012 决定第 3 条）。少了 `answer` 那一份，第一遍里按下的收尾会把当前卷的第二遍吃掉，
  盘上少的正是收尾说好要留下的那一卷。
- **What this ticket actually did:** **照抄，名字与会话那一份逐字相同**
  （`answer`、`at_the_decision_point`；升级那张表叫 `next`，与 `raise_stop` 里那个 `match`
  逐条相同），并在各自的文档里点名另一份在哪儿。名字取一样是为了让 19 号票一眼认得出
  这是**同一件事**，而不是几件长得像的事。**没有顺手收**：本票的硬约束写着
  「不新开对外 seam，闩的编码收成一处是 19 号票，本票不要顺手做」。
- **Whose call:** `p4-parking-lot/19`（它收 Q70 那一份闩的时候，要不要把这两样一并收进去；
  真要收，落点大概是 `src/main.rs` 之外的一个新模块——那个文件已经两千多行了）
- **处置：** **`say-and-stop/01` 落地（2026-10-01）：照票面走——收进 bin 里一个模块。**`src/stop.rs` 的
  `next`／`at_the_decision_point`／`answer`，命令行（`Latch::press`、`Bar::observe`）与会话（`Session::raise_stop`、
  `Watch::observe`、`Running::stop` 推不推闸）都调它；两路那两份连同各自直接问它们的用例删掉，
  `tests/single_source.rs` 添一条拦抄件（Q1117）。

#### Q552 — Q262 那两样（`answer` 与升级表 `next`）本票**判为不收**

- **From:** 票 `p4-parking-lot/19`
- **Kind:** 岔口（Q262 把这个决定指名交给了本票）
- **Where:** `src/main.rs` 的 `answer`／`at_the_decision_point`／`next`；
  `src/session/run.rs` 的 `answer`／`at_the_decision_point`；
  `src/session/state.rs` 的 `Session::raise_stop`
- **Why it did not block:** 本票验收那五条一条都没提它们。票面点名要收的是**停车场 Q70**
  （闩的编码），而 Q262 是 18 号票**另记**的一条，票面没写「收 Q262」——18 号票知道
  Q262 存在却仍只把 Q70 写进 19 号票面，那是一次划界，不是漏。
- **What this ticket actually did:** 只收编码，那三个函数一格没动。
- **Options:** ① 一并收进二进制侧一个新模块（Q262 自己建议的落点）；② 留着，另立一张票。
- **Recommend:** ①，但**不在本票上**。理由：那三个函数全在二进制 crate 里，收拢**一格公共
  API 都不动**，与本票这一次「对外契约的扩大」不是同一件事——混进一张票，「这一票到底扩了
  什么」就讲不清了。18 号票已经把两处名字取成逐字相同，收拢那一次编辑很小。
- **Whose call:** 拍板的人（要不要为它另开一张票；Q262 的《处置》已改成指着本条）
- **处置：** **`say-and-stop/01` 落地（2026-10-01）：推翻「不收」，照 ① 收。**另开的就是本票；落点与 Q262 一处，见上。
