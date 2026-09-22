# 01 — 停止规则在 bin 里只留一份

**What to build:** 「确认点上做完再停要让、立即停止不让」与那张升级表（继续 → 做完再停 → 立即停止 → 立即停止），
命令行与会话各抄了一份、名字逐字相同。收进 bin 里一个模块，两路都调它。库的对外形状一格不动——
闩的编码早已在库里，这是 bin 内部的收口。屏上与行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] bin 里只剩一份「确认点上让不让」与升级表；命令行与会话都调它
- [ ] 两路原先各自的用例搬进这个模块：升级表逐级、确认点上两级各让不让
- [ ] 命令行与会话各留一条用例，断言调的是它
- [ ] 库的公开形状、屏上、命令行输出一格不变
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
- **处置：** 待处理。`p4-parking-lot/19` 落地时**判为不收**——它只收了票面点名的 Q70
  （闩的编码），这两样一格没动，理由与推荐见 **Q552**。去处未定。

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
- **处置：** 待处理。
