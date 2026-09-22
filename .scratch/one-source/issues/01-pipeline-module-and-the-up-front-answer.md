# 01 — 分析与写出两个环节连同滚动窗口搬成 `pipeline` 模块，开卷之前那一问换成具名三态

**What to build:** 纯搬家：滚动窗口那一簇（窗口、座位、开卷之前那一问）连同分析环节、写出环节、输出页，从库的根模块搬成一个 `pipeline` 模块；
根模块只剩对外的那几个 seam 与装配。开卷之前那一问的答案从嵌套 `Option` 换成具名三态枚举——
**答不出 / 答得出而没顶死 / 顶死在这一档**——`Settles` 从它派生，两个消费者各解一次、解法不同的那两处从此写不出来。
行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 滚动窗口、分析环节、写出环节、输出页住进 `pipeline` 模块；根模块里不再有它们
- [ ] 开卷之前那一问交出具名三态枚举，`Settles` 从它派生；代码里不再有嵌套 `Option` 那种写法
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q431 — 滚动窗口那三百行落在 `lib.rs` 里，而同量级的东西各占一个模块

- **From:** 票 `two-pass-rework/12`（`/code-review` 的 Standards 轴指出）
- **Kind:** 落地时走的那条路（放哪儿）
- **Where:** `src/lib.rs` 的 `Window` / `Windowed` / `Seat` / `pinned_up_front`（约 300 行）
- **Why it did not block:** `cache`、`hysteresis`、`envelope` 这些同量级的东西各占一个模块，
  而 `lib.rs` 已经三千多行——审查按《Divergent Change》点了这一条。本票没有搬，理由是
  **搬出去的那一个站不住**：窗口要用 `OutputPage`、`Outcome`、`Branch`、`Candidates`、`lock`
  五样，全是 `lib.rs` 根上的私有项，搬走之后那个模块每一句都在往回够根模块。
  而 `cache` / `hysteresis` / `envelope` 是三个自成一体的算法，接口都很窄。
  窗口与 `Compute`、`first_pass` 是同一件事的三段，眼下摆在一起。
- **What this ticket actually did:** 留在 `lib.rs`，紧挨着 `Compute` 与 `first_pass`。
- **Options:** ① 照现在；② 开 `src/window.rs`，把那五样提成 `pub(crate)`；
  ③ 连 `Compute` / `first_pass` / `second_pass` / `OutputPage` 一起搬成一个 `src/pipeline.rs`。
- **Recommend:** ③ 才是真正解掉「`lib.rs` 三千行」的那一刀，而它是一次纯搬家、
  该单开一票（本票的 diff 已经一千三百行）。②只搬一半，换来的是一条新的跨模块依赖。
- **Whose call:** 拍板的人
- **处置：** 待处理。

#### Q545 — `pinned_up_front` 那个三态是嵌套 `Option`，而它的正确形状本票刚在下游造出来

- **From:** 票 `two-pass-rework/06`（`/code-review` 的 Standards 轴指出）
- **Kind:** 落地时走的那条路（一个已有类型的形状，本票没改它）
- **Where:** `src/lib.rs` 的 `pinned_up_front(...) -> Option<Option<Candidate>>`
  与它的两个消费者 `Window::open`、`pinned_for_the_first_pass`；本票新造的 `enum Settles`；
  停车场 **Q431**
- **Why it did not block:** 那三态读作「答不出来 / 答得出而没顶死 / 答得出是这一档」，
  **本票新造的 `Settles`（`AfterTheVolume` / `InTheWindow` / `UpFront`）正是它的形状**，
  却建在下游——上游仍是嵌套 `Option`，两个消费者各解一次、解法还不一样：
  `Window::open` 从前写 `!= Some(None)`（单看读不出意思），`pinned_for_the_first_pass`
  写 `.flatten()`。函数是 `12` 号票为滚动窗口写的，本票只是给那个答案添了第二个消费者；
  **改它的返回类型是改一个上游类型，不在本票的爆炸半径里**。
- **What this ticket actually did:** 类型没动。`Window::open` 那句 `!= Some(None)` 换成了
  三支写开的 `match`，每一支带自己那句理由——含义从此在代码里而不只在文档里。
  `pinned_ahead` 这个名字改成 `pinned_for_the_first_pass`：它与 `pinned_up_front`
  在英文里是近义词，同一个文件里读不出分别（同一轮审查指出）。
- **Options:** ① 照现在（嵌套 `Option` ＋ 两处各自写开）；② 给 `pinned_up_front` 一个具名
  三态枚举，`Settles` 直接从它派生；③ 连 `Window` / `Windowed` / `Seat` / `pinned_up_front`
  一起搬进一个 `src/window.rs` 或 `src/pipeline.rs`，那时顺手改类型。
- **Recommend:** ②，而它该跟着 **Q431**（那一簇三百行想搬出 `lib.rs`）一起做：
  单独改一个返回类型只动两个调用点，收益是「`!= Some(None)` 这种写法再也写不出来」；
  与搬家一起做则一次到位。**两条记的是同一簇代码，settle 时并在一起看。**
- **Whose call:** 拍板的人（与 Q431 合并处置）
- **处置：** 待处理。
