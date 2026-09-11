# 04: 总览块答「此刻」，一直答到收场

**What to build:** 按 `t` 起的那一趟，哪怕在决策点上答过继续、真写了几卷出去、
**隔离了五卷**，钉住的那一块从头到收场之前一个「隔离」都不说——它一直在给判定分布。
而退出码那时已经注定是 `2`。

成因是分岔的谓词选错了一档。它原本问「此刻落过盘没有」，那会让**决策点上答继续那一帧**
换内容、还可能矮一行——而那一帧正是用户看完判定、正要按下 `x` 的那一下，
把他据以拿主意的那一份换掉正是要治的病。于是改成问「起手按的哪一个键」，
一趟之内一格不变——**代价就是这一条**。

两个谓词都不对，要的是第三个：**「这一趟到此刻为止真写出过东西没有」**。
它一趟之内**只从假变真一次**，而那一次发生在**第一卷真写完**，不在答继续那一帧。

纯试算（一卷都没答继续）收场时**照旧给判定分布**——那时「完成 0 卷」是真话却是废话。

收停车场的 **Q196**。

**Blocked by:** 01 — 夹具喂全事件流那一半（同一批快照）

**Status:** resolved

- [x] 按 `t` 起、答过继续、写出过卷的那一趟，**收场之后**总览块说得出完成几卷 · 跳过几卷 · 隔离几卷
- [x] **决策点上答继续那一帧一格不动**：不换内容、不矮一行（既有那条验收照旧绿）
- [x] 翻副发生在**第一卷真写完**那一刻，不在答继续那一帧
- [x] 那一格**只升不降**：一趟之内只从假变真一次，屏上不来回跳
- [x] **纯试算收场仍给判定分布**，不给「完成 0 卷」
- [x] 抬头照旧跟着「此刻在写没写」改口，一处不少
- [x] 三条闸门全绿

## 落地记录

### 第三个谓词：一格布尔，翻在第一卷真写完

```rust
// src/session/live.rs —— tui 特性外面，三条闸门都编到
pub fn has_written(&self) -> bool { self.written }        // 只升不降，形状与闩相同

// 翻面点只有这一处：收摊的这一卷在写，这一趟就真写出过一卷了
pub fn volume_finished(&mut self, report: &VolumeReport) {
    if self.volume.as_ref().is_some_and(|walking| walking.writes) {
        self.written = true;
    }
    ...
}
```

「这一卷在写吗」逐卷问，因此挂在当前卷那一条上（`Walking::writes`），卷收摊整条撤掉、下一卷从假起，
不必另记一格再手动清。两处置真：`pass_started(Pass::Second)`——执行那一趟走到按档写出就在写
（认库真收到的 `ran_as == Process`，用例里 `DryRun + GoesOn` 的夹具因此一个字节都不写），续做那一趟
只有 `for_the_rest == Some(Continue)`（答过「都这样」之后不再问、当场就在写）；`decide(Continue)`——
停在决策点上的这一卷从此在写。`volume_failed` 不翻：第二遍里废掉的卷盘上没有它。

真会话里的次序（`run.rs` 的 `Watch::observe`）：先 `Live::observe(PassStarted Second)` 再停在
`Gate::ask`，用户按 `x` 才到 `Live::decide`；`for_the_rest` 摆下之后 `Gate::ask` 短路、不调 `decide`，
那一支正是 `pass_started` 那半接住的。`mode()` 那几行一字未动（tpr/11 的边界）。

**为什么不是更省的一行** `mode() == Process && !report.skipped()`：答了继续的卷一废、下一卷答收尾，
它就翻了——那一刻盘上一个字节都没有，「第一卷真写完」成了假话。多一格逐卷的布尔换来的就是这句真。

### 总览块那两行照它翻

`overview.rs` 新增 `delivered_as(live)`：`has_written()` → 执行那一副，否则 `started_as()`。
结论行与出事行都改问它；`x` 起的一趟从头就是执行那一副（破折号那一支照旧），`t` 起的一趟在第一卷
真写完之前是试算那一副——纯试算收场因此照旧给判定分布。抬头照旧走 `Live::mode()`，一字未动。
「还没跑过」那一段与 `overview()` 签名没碰（nfl/06 的边界）。

### 会红的那几处

| 用例 | 问的是 | 闸门 |
|---|---|---|
| `live::tests::the_run_has_written_once_the_first_volume_it_wrote_is_finished` | 跳过不翻、答收尾不翻、答继续那一帧不翻、收摊才翻；`started_as` 不动 | 1、2 |
| `live::tests::having_written_is_a_latch` | 翻过之后再答收尾、再跳过、收场都不缩回 | 1、2 |
| `live::tests::a_volume_that_failed_while_being_written_does_not_count_as_written` | 答继续却废掉的卷不算；「都这样」之后自动继续的卷算；执行头一卷即翻；`DryRun+GoesOn` 永不翻 | 1、2 |
| `overview::tests::the_overview_changes_sides_when_the_first_volume_is_written_not_when_go_on_is_answered` | 答继续那一帧两行一格不动；那一卷收摊才翻；再答收尾不缩回 | 1 |
| `overview::tests::the_overview_of_a_trial_that_went_on_and_isolated_five_volumes` | 快照：`t` 起 · 答「都这样」· 隔离五卷 · 收场 → `完成 5 卷 · 跳过 1 卷` / `出事 隔离 5 卷 · 失败 5 页`，且 `exit_code() == 2` | 1 |
| `overview::tests::a_trial_that_never_went_on_still_gives_the_verdict_spread_when_it_ends` | 真会话形状的纯试算（Process + Waits，逐卷答收尾）收场仍 `判定 2 卷 4bit`，屏上无「完成」 | 1 |

既有 `answering_go_on_does_not_move_the_overview` 断言一字未改，照旧绿——票面第二条。

**红过一眼**：热树上把 `delivered_as` 退回 `started_as()`、`volume_finished` 不置 `written`，六条里五条红
（纯试算那条两边都绿——它守的是不回归）；快照那条红出来的正是票面说的病：
`判定 5 卷 4bit · 1 卷 跳过` / `出事 失败 5 页`，一个「隔离」都没有，而退出码是 `2`。复原后七条绿。

### CONTEXT.md《总览》改了一句

「问的因此是起手按的哪一个键，一趟之内一格不变」改成「问的因此是这一趟至今真写出过东西没有……
第一卷真写完之前是试算那一副，之后翻成执行那一副，一趟之内只翻一次、翻过不翻回」。
这是改写已有词条的含义，拍板就是这张票本身（票面与 spec 第三节明写换谓词）；
词汇表不跟着改，`CLAUDE.md` 那条「类型名一律取自它」就对不上实现。

### 数

三条闸门跑满（`cargo xtask gate`，与 `cargo xtask polish` 串在同一根棒里），三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **945 通过 0 失败**；lib **249** / bin **380** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **807 通过 0 失败**；lib **249** / bin **242** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s`` | 干净，一条告警都没有 |

**闸门 1 的 bin 涨 6、闸门 2 的 bin 涨 3**，涨的正是上表那六条：`live.rs` 三条在特性外面两条闸门都跑到，
`overview.rs` 三条在 `tui` 里面只进闸门 1。lib 一格不动（本票不动库）。

**闸门之外那一遍**（`cargo xtask polish`）：

| 一遍 | 结果 |
|---|---|
| `cargo fmt --check` | 绿 |
| `cargo clippy --all-targets` | 绿，``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.81s``，零告警 |
| `cargo clippy --all-targets --no-default-features` | 绿，``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.37s``，零告警 |
| `cargo doc --no-deps` | 绿，仍是 **15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`），一条没多 |

## 停车场结转

**收 Q196**（索引行不动）。索引那一栏写的拍板是「收场之后翻成执行那一副」，票面把翻面点定得更准
——**第一卷真写完**。两处不矛盾：收场时那一格必然早已翻过——一趟里只要真写出过一卷，
那一卷收摊的那一刻就翻了，收场排在每一卷收摊之后；反过来一卷都没写出过的（纯试算）收场也不翻，
而那正是票面「纯试算收场仍给判定分布」要的。拍板说的是「至迟到那一刻」，票面说的是「恰在那一刻」。

**新记两条**，都在《待处理》（本票的 id 块是 Q673–Q680，用掉两个）：

- **Q673** — 票面要总览块「说得出隔离几卷」，既有决定是「隔离算进完成、由出事行说」。走的是既有决定：
  两行同一刻翻面，隔离由出事行说得出，结论行不重复。推荐保持。
- **Q674** — 翻面之后，答收尾（等于试算）的那几卷也数进「完成」——票面治的病换了一种卷还活着。
  用例把现状钉住；推荐日后结论行多一格「试算 N 卷」，数据 `Walking::writes` 上已经有一半。

### 评审

`/code-review` 跑过一遍，两轴并行（**只读，没碰工作区，一条 cargo 都没跑**），
worktree 路径与基底 `851ff4d` 一起交出去的。

**Spec 轴**：七条验收框逐条有用例钉住，无缺失、无 scope creep；真会话的事件次序核过
（`Watch::observe` 先折进 `Live` 再停在 `Gate::ask`；决策键只在等答话时收，`decide` 必晚于
`pass_started`；「都这样」之后 `Gate::ask` 短路不调 `decide`，由 `pass_started` 那半接住；中止不经
`decide`、也没有 `VolumeFinished`），没有翻早、翻晚、翻不了的路。**两条收下**：

- 「两处旧注释没跟上」（故事 23/24）：`overview.rs` 测试文档里「『是什么』手上已经有
  （`Live::started_as`）」、`live.rs` 测试文档里「总览块那两行走它，一趟之内一格不变」——
  两处改口指向 `delivered_as`／`has_written`。
- 「翻面那一帧出事行可能整行消失、矮一行」——提请知悉：不违票面，护的是决策点那一帧，且一趟只跳一次；
  `ThisVolume` 续做从第二个决策点起总览给的是「完成 N 卷」——那正是票面「翻副在第一卷真写完」要的。

**Standards 轴**：硬违规无。判断题五条，**收两条**：

- 单一出处（`CLAUDE.md`《文档写作》4）：Q149/Q196「另两个谓词各差一档」的理由在五处各展开一遍
  → 只留 `delivered_as` 一处为权威，`has_written` 与 `started_as` 的文档改成指路。
- 结果而非变更史（同上 1）：`CONTEXT.md`《总览》括号里我添的那半句被否掉的候选后果 → 删掉，
  原有那一句留着。

**三条不收，理由各一句**：Feature Envy（`delivered_as` 放进 `Live`）——派活边界要求 `live.rs`
只加信号及其读法，合成两个谓词是画法那两行的事；Repeated Switches（`pass_started` 里对 `Resuming`
第三处分岔）——`GoesOn` 那一支认的是库真收到的 `ran_as`，与 `started_as` 同一个答案但问的是
「这一遍写不写盘」，注释里写着为什么；`writes`／`written`／`has_written` 近形名——一个逐卷进行时、
一个一趟的闩，各自的文档第一句就分开了。

评审之后动了 `.rs` 的注释与 `CONTEXT.md`，闸门在**最终状态**上重跑了一趟（上面《数》抄的就是它）。
