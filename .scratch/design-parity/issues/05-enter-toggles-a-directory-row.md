# 05 — `⏎` 在目录行上是开关

**What to build:** 目录行上按 `⏎`：展开着就收起，收着就展开（设计稿就是这样）。`l` 照旧只展开、`h` 照旧收起；
双击等于 `⏎`，跟着变。卷行上的 `⏎` 不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 按键表给目录行上的 `⏎` 派一件自己的事；`l` 照旧只展开，卷行上的 `⏎` 照旧进每页结果
- [ ] `ended-h-Enter-Enter` 接上、比整屏且绿；`running-dblclick-dir` 照旧绿
- [ ] 双击一个展开着的目录行收起它：经终端层喂两次单击，断言屏上那一行变回收着
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
- **处置：** 待处理。

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
- **处置：** 待处理。
