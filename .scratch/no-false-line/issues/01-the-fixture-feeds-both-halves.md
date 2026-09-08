# 01: 夹具喂全事件流那一半

**What to build:** 那份「每一种卷都有一卷」的探针夹具里，有一卷带着失败页——
而它**一条逐页失败事件都没报过**。真跑一趟时每一页失败都发一条事件，那一页随后又在
卷报告里出现一次（会话那一层开宗明义：事件流就是报告的增量）。这份夹具只喂了后一半。

后果不是数错——出事行数出来是同一个数，屏上一格不变——而是**报告区那一段
「失败页（出现的当场……）」在快照里从来没露过面**。真会话里出得来的那一副，
测试里一张都没有：它改坏了不会有任何一张快照红。

把前一半也喂上，那一段从此进快照。

**这一票排在本批最前面。** 它会牵动报告区、上色、让位三个模块十几张快照——
先落，后面每一张动画法的票各 rebase 一次而不是两次。

收停车场的 **Q195**。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 那一卷的失败页**两半都喂**：逐页失败事件照报，卷报告里那一页照旧在
- [x] 报告区「失败页（出现的当场）」那一段在**至少一张快照里露面**
- [x] 出事行那个数**一格不变**——它此刻数的是「报告里那几卷 + 当前这一卷已经报过的那几条」，
      两条路本来就该数出同一个数，这一票证明它们真的一样
- [x] 受牵动的快照**逐张核对后重录**，不是顺手覆盖；差异逐条说得出为什么
- [x] 三条闸门全绿

## 落地记录

### 一卷收摊走同一个口子，两半一起喂

```rust
// src/session/live.rs 的 fixture 模块——**`tui` 特性外面**，画法与 live 自己两侧都够得着
pub fn volume_finished_with_its_failures(live: &mut super::Live, report: &VolumeReport) {
    for page in report.failures() {
        if let PageOutcome::Failed { reason } = &page.outcome {
            live.page_failed(&page.source, reason);
        }
    }
    live.volume_finished(report);
}
```

**页名与那句原因逐字取自那一份卷报告**——这是这张票的要害。从前 `a_run_in_flight` 手抄了
一份（`Path::new("库/卷二/017.jpg")` 与 `reason` 各写一遍），`overview.rs` 那份夹具又抄了
一份（`库/卷一/017.jpg`）：报告里改了页名，事件流那一半不会跟着变，两处说的从此不是同一页。
手抄的那几份就是「假话」的来源，全部删掉。

**它摆在 `live::fixture` 而不是画法那一侧的探针里**，理由是那个模块自己的文档写着的：
「`draw` 那一侧的快照用例与本模块的用例共用它——两边要的是同一份东西，**各搓一份就会在
改动时走散**」。它只碰 `Live` 与 `VolumeReport`，一格探针自己的数据都不用。
跟着 `per_page_volume`、`overridden_volume`、`a_page_of_every_kind` 那几份同一副写法挂
`cfg_attr(not(feature = "tui"), allow(dead_code, ...))`——闸门 2 那一趟没有画法，也就没人读它。

**三条边界写在它自己的文档上**，不写成一句「一律」的空话：

- 屏上**画得出**「失败页（出现的当场……）」那一段的夹具**非走它不可**；
- 只问卷级那几行的（总览块、目录表那几条）直接报卷跑完也成——它们画的那一格里没有那一段；
- **两种都不许手抄页名与那句原因**；
- `live.rs` 自己那几条用例是**例外**：`the_failed_pages_of_the_volume_in_flight_count_towards_now`
  问的正是「在途那一格与报告那一格换手时和变不变」，喂成一体就问不出来了。

改走它的**六份夹具、十一处调用**：`probe.rs` 的 `a_run_in_flight`（2 处）与
`every_kind_of_volume`（4 处）、`report.rs` 的 `a_run_worth_expanding`（2 处）·
`a_run_with_every_tail_section`（1 处）·
`the_volume_waiting_at_the_decision_point_expands_into_its_own_pages`（1 处）、
`overview.rs` 的 `a_dry_run_with_a_broken_page_and_a_lost_volume`（1 处）。
没有失败页的那几卷（`skipped_volume`、`per_page_volume`、`overridden_volume`）
走它逐字节不变——它只在报告里真有 `PageOutcome::Failed` 时多发那一条。

### 会红的那一处

`probe::tests::every_failed_page_in_the_report_was_reported_the_moment_it_happened`
——三副夹具各走一遍，逐条比**页名与那句原因**：报告里那几页失败，事件流里必逐条报过，
两半说的必须是同一页。往这两份跨块夹具里添一卷带失败页却忘了走那个口子，这一条当场红。

它的照射范围**写在它自己的文档里**：只走得到 `probe` 那两份跨块夹具，
`draw` 底下各块 `mod tests` 里自己搭的那几份够不着（停车场 Q354）。

`feeding_both_halves_does_not_count_the_same_page_twice` 钉票面第三条：
`failed_pages().count()` · `report().failures().count()` · `failures_so_far()` 三个都是 1。

### 屏上多出来的那三行

受牵动的快照三张，全部**从用例打印出来的实际屏逐字提取**替换，不是手抄：

| 快照 | 档 | 差异 |
|---|---|---|
| `THE_TABLE_WIDE` | 120×26 · 执行 | 表末行之后，**三行空白换成失败页那一段三行**；总行数不变 |
| `THE_TABLE_OF_A_DRY_RUN` | 120×26 · 试算 | 同上，位置在「棋魂 08 · 等你拿主意」那一行之后 |
| `THE_TABLE_NARROW` | 80×24 · 整屏 | 报告区 28 列上那一段折成**五行**（抬头 2 · 页名 1 · 原因 2），正好填满原先五行空白 |

**窄档没有挤出可视区**：行数 24 → 24，没有滚动，一个字都没丢。左栏与总览块一格未动。

**出事行三张上逐字不变**（`出事 隔离 1 卷 · 失败 1 页 · 卷级失败 1 卷` /
`出事 失败 1 页 · 卷级失败 1 卷`）——票面第三条在屏上也得到了证：那个数此刻数的是
「报告里那几卷 + 当前这一卷已经报过的那几条」，那一卷收摊时在途那一格只是**换手**进报告。

> **给 `no-false-line/03` 的一句话**：`THE_TABLE_NARROW` 这一档此刻**一格余量都不剩**
> ——「JPEG 数据截断」正好压在报告区末行。03 把屏底按需长高、报告区少一行时，
> **那一行会无声掉出去**。那正是这个 effort 要修的那类假话，别撞进去还不知道。

### 一条既有用例真红了，而它此前是因为夹具喂错了才绿的

`paint.rs` 的 `every_painted_row_carries_a_word_or_a_mark_of_its_own` 逐**行**判
「上了色的行必须自己带一个字或行首记号」，跑的正是 `every_kind_of_volume`。
喂上前一半之后失败页那一段进屏，中间那一行是**页的路径**（`  库/哆啦 03/017.jpg`）：
整段红，自己一个载体都没有，当场红。**不是本票弄坏的，是本票让它第一次被问到。**

规矩本身早写下来了：同一个 `mod tests` 里
`the_failing_pages_block_goes_red_under_a_heading_that_says_so` 的文档写着
「**载体是那一段，不是那一段里的每一行**——一段话是一起读的」，
`CONTEXT.md` 的《语义色》说的也是「每一处上色的地方**旁边**都另有一个字或一个行首记号」。

判据因此从逐行改成**逐段**，用例改名 `every_painted_block_carries_a_word_or_a_mark_of_its_own`。
这里认的「一段」是「挨着、且屏上是同一个样子（同一批前景色、压不压暗也一样）」的那几行
——**这不是《语义色》那个「档」**，屏上读回来的是色，不是档；那句区分写在代码旁边。

**这个判据有一道缝，当场补上了**：两段同色又紧挨着时它并作一段看，
而此刻正是这样（没做成那一卷那一行与失败页那一段都是红的），
于是新进屏的「失败页」抬头行在这份夹具上一次都检不到。
`the_row_that_went_wrong_is_red_and_says_so` 因此从三处扩到**四处**，
末一处就是失败页那一段（抬头行与原因行各一句）。停车场 **Q353** 记着这件事。

`p3-session-legibility/09` 的《落地记录》里提到旧名的那两处**没动**
——那是历史记录（`docs/agents/issue-tracker.md`）。

### 票面第二条的前提是假的

票面写着：

> 报告区那一段「失败页（出现的当场……）」在快照里从来没露过面

**改动前它已经在三张快照里**：`src/session/draw/report.rs` 的 `WITH_A_FAILED_PAGE`
（96×36）、`src/session/draw/yielding.rs` 的 `AT_EIGHTY_BY_TWENTY_FOUR_RUNNING`（80×24）
与 `FORTY_COLUMNS`（40×24）——因为另一份跨块夹具 `a_run_in_flight`
**本来就调了 `page_failed`**（那两行手抄的字面正是本票删掉的）。
spec 那条用户故事 22（「那一段在快照里露过面」）的前提同样不成立。

真正新增的是 `every_kind_of_volume` 那三张（覆盖面确有扩大），
以及**上色那一层第一次问得到它**——`every_painted_block_...` 那一条当场红，就是证据。
按 `docs/agents/issue-tracker.md`，更正一条写下时就已经是假的说法不算改写历史，
那是记录本身的缺陷。

### 数

三条闸门跑满（`cargo xtask gate`），三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **823 通过 0 失败**；lib **218** / bin **334** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **695 通过 0 失败**；lib **218** / bin **206** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.77s`` | 干净，一条告警都没有 |

**闸门 1 的 bin 涨了 2**（821 → 823），涨的两条都在 `tui` 里面（`session::draw::probe::tests`），
因此只进闸门 1：

| 用例 | 问的是 |
|---|---|
| `every_failed_page_in_the_report_was_reported_the_moment_it_happened`（新） | 报告里那几页失败，事件流里逐条报过没有——页名与那句原因都得对得上 |
| `feeding_both_halves_does_not_count_the_same_page_twice`（新） | 两半都喂之后「此刻坏了几页」还是不是 1 |

**闸门 2 一格不动**（695，lib 218 / bin 206）：新添那两条与改名那一条都在画法那一侧。
`live::fixture` 里新添的那个函数在特性**外面**，闸门 2 因此真编到了它——
它挂的 `cfg_attr(not(feature = "tui"), allow(dead_code, ...))` 也只有这一条与
`clippy --no-default-features` 那一遍照得到。
`every_painted_row_carries_a_word_or_a_mark_of_its_own` **改名**成
`every_painted_block_carries_a_word_or_a_mark_of_its_own`，不进涨的那一列。

**闸门之外那一遍**（`cargo xtask polish`）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`），
**一条没多**。

## 停车场结转

**新记两条**，都在《待处理》里（本票的 id 块是 Q353–Q362，用掉两个）：

- **Q353** — 失败页那一段一进屏，「每一处上色的地方都配了一个字或一个行首记号」按**逐行**判就不成立。
  走的是逐段那一条，并补上失败页那一段的载体断言。**点给 `no-false-line/05`**：
  那张票票面第四条正是「有一条闸门问得出这句话」。
- **Q354** — 「带失败页的卷两半都喂」没有一处守卫走得遍全仓。规矩与守卫的照射范围
  各写在自己的文档上，不留一句「一律」的空话。

**一条都没了结**：Q195（票面收的那一条）本轮 `/settle` 时就已经点给这个 effort，
索引表里那一行原样留着。

### 评审

`/code-review` 跑过一遍，两轴并行（**只读，没碰工作区，一条 cargo 都没跑**），
worktree 路径与基线 `f9e12b5` 一起交出去的。两轴共提 7 条，**全收**：

- **Spec ①「票面第二条的前提是假的」** → 见上面《票面第二条的前提是假的》。
- **Spec ②「逐段判据留了一道缝」**：新进屏的「失败页」抬头行与紧挨着的红色
  「消失的那卷」并作一段，在 `every_kind_of_volume` 上一次都检不到。
  **改法**：`the_row_that_went_wrong_is_red_and_says_so` 从三处扩到四处。
- **Spec ③「窄档一格余量都不剩」** → 写进上面给 `03` 的那一句。
- **Standards ①「改名后文档头一段仍说逐行」**：同一份文档里判据写了两遍且不一致
  （`CLAUDE.md`《文档写作》第 1 条）。**改法**：头一段改口成「逐段问」。
- **Standards ②「一档色」占了《语义色》的词**，而那个键其实是 `Vec<Color> + dim`
  的原样读回（`CLAUDE.md`《写代码前》：一律取自 `CONTEXT.md`）。**改法**：
  改成「这一行在屏上是什么样子」，并写明「这不是语义色：屏上读回来的是色，不是档」；
  闭包 `shade` 改名 `looks`。
- **Standards ③「模块文档立的规矩仓库自己不守」**（与它提的 Feature Envy 同指：
  那个函数只碰 `Live`，一格探针自己的数据都不用）。**改法**：把口子从
  `draw::probe` 搬进 `live::fixture`，屏上画得出那一段的夹具**全部**改走它
  （多出 `report.rs` 三处、`overview.rs` 一处），三条边界写进它自己的文档。
- **Standards ④ 单一出处**：同一段解释逐字出现三处。**改法**：收成一处
  （写在那个口子自己的文档上），别处只指路。

Standards 顺带问了一句 Q323–Q352 为什么全仓一处都不存在——那是**按块发号**的正常空档
（本票的块从 Q353 起），不是丢了东西。
