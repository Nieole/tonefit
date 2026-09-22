# 10 — `(相对路径, 第几张, 共几张)` 捆成成员类型

**What to build:** 三个同型参数在四处同行旅行：算输出名、来路、幂等比对、认写出的那一族。捆成一个类型，四处都收它——
「这几处必须拿同一组值」从此由类型说，不由文档说。行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 四处都收成员类型；代码里不再有那三个参数同行
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q494 — `(相对路径, 第几张, 共几张)` 这三个参数在四处同行旅行，本票把其中一处扩成了四参

- **From:** 票 `two-pass-rework/07`（`/code-review` Standards 轴报的基线坏味道）
- **Kind:** 既有坏味道（Data Clumps），本票把它**稍微加重了一点**
- **Where:** `src/lib.rs`：`output_name(relative, ordinal, count)`、
  `Origin::new(relative, ordinal, count)`、`Placement::new(relative, ordinal, count, …)`，
  外加 `written_family` 里那个 `matched(record, ordinal, count)` 闭包与
  `PageRecord::matches(fingerprint, relative, ordinal, count)`。
  **它是一个想出生的类型**：`Placement` 的文档自己就写着这三个是同一组
  （「两者由同一组 (源成员, 第几张, 共几张) 算出，因此一同算出、一同传下去」）。
  本票给 `Placement::new` 加了第四个参数，那一处从三参到四参。
- **Why it did not block:** 三个参数同型同序、`Placement` 那句文档已经把「必须一同传」
  写死，传错一个的失败模式是幂等去找的名字与真写出的名字错开——而那正是
  `ensure_one_member_per_output` 与 `written_family` 两处已经在拦的东西。
  本票只多加一个**不同型**的参数（`Option<&Fingerprint>`），没有把同型参数的队列拉长，
  传反的风险一格没涨。
- **What this ticket actually did:** 加了第四个参数，一个字没重构。
- **Options:** ① 照现在；② 把那三个捆成一个小类型（形如 `Member { relative, ordinal, count }`），
  四处一起改；③ 只在 `Placement::new` 上收，别处不动。
- **Recommend:** ②，但**不在本票**——它要动 `output_name`、`Origin::new`、
  `PageRecord::matches` 与 `written_family` 四处，而 `PageRecord::matches` 是幂等那条路
  上的判据，动它要连 `tests/idempotency.rs` 一起重读。③ 最坏：收一半等于让同一组值
  在库里有两种写法。② 的收益不只是少几个参数——`Origin::new` 与 `output_name` 今天
  **必须**拿同一组值调用才正确，捆起来之后那件事由类型说，不由文档说。
- **Whose call:** 排票的人（值不值得为它开一张纯重构的票）
- **处置：** 待处理。
