# 08 — 补两道扫描，场景数据的光标种类扫一遍

**What to build:** 《砍列》《视口》各添一条扫描用例，照「拒绝开始的名单只住一处」那条的形状：词汇表那一条里只许有名字、不许有值。
再加一条用例走遍全部场景与全部序列的场景数据，每一种光标都认得出——认不出的键名当场红，不再默默落到输出目录那一行。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `tests/single_source.rs` 添两条；往词汇表那一条里抄一份次序去试，它红
- [ ] 场景数据光标种类那一条：往一份场景数据里写一个认不出的种类去试，它红
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q615 — 《砍列》与《视口》改成指路之后，没有一条用例问得出「词汇表里没有第二份」

- **From:** 票 `p4-parking-lot/28`
- **Kind:** 你确实拿不准的单项（本票的判断在身后没有闸门）
- **Where:** `CONTEXT.md` 的《砍列》（三张表的次序改成指 `Column::DROPPED_IN_TURN`）
  与《视口》（共用它的名单改成指 `Viewport` 那张表）；对照的是
  `tests/single_source.rs` 的 `the_refusal_list_lives_in_one_place`
- **Why it did not block:** 本票**代码一行不动**，添一条集成用例就破了那条硬约束，
  也会让闸门数动格——而验收末一条写的正是「闸门数一格不变」。
  两条词条此刻的状态与「拒绝执行」那张单子收拢之前一样：说得通、也没有东西拦着人再抄一份。
- **What this ticket actually did:** 只改说法。两条词条各自明写「这里不抄第二份」，
  把意图留在被守的地方——但那是一句话，不是一个闸门。
- **Options:** ① 照 `the_refusal_list_lives_in_one_place` 的形状各添一条扫描式用例
  （记号里只许有名字、不许有值——`p4-parking-lot/27` 立的那条判法）；
  ② 只留词条里那句话；③ 把三张表的次序搬回 `CONTEXT.md`，让词汇表当那一处出处。
- **Recommend:** ①，但**不在这张票**：它要添用例，而本票写死了代码一行不动、闸门数一格不变。
  ③ 与 `columns.rs` 那句「只有这一处出处」正面撞车，要先拍板哪一头是家。
- **Whose call:** 拍板的人（要不要为这两条各添一条用例）
- **处置：** 待处理。

#### Q846 — `scene.rs` 的 `views_of` 认光标那两种读错了键名，而两棵树的闸门都绿

- **From:** 票 `session-redesign/10`
- **Kind:** 路过发现的无关缺陷（`08` 落进 `main` 的代码里就有）
- **Where:** `src/session/scene.rs` 的 `views_of`
- **Why it did not block:** 夹具那一头光标的 `kind` 只有六种：`out`／`add`／`path`／`directory`（带 `root`）／`volume`（带 `root`）／`note`（带 `what`）。`08` 写的是 `Some("dir") => Cursor::Directory(at("dir"))` 与 `Some("note") => Cursor::Note(at("path"))`——**两个键名都不是夹具里的那个**。`match` 认不出就落到 `_ => Cursor::Output`，一声不响；`cursor_line` 再把它挪到头一行停得住的。**两棵树的闸门当初都绿**，因为 `08` 摆得出的那五屏里光标全是 `volume` 或 `path`——**有用例，用例照不到**。我撞见它是因为 `envelope` 那一屏的光标是 `directory`（自动滚动又正好把光标带到同一行，差别因此还藏了一层），而 `ended-note*` 那四串的光标是 `note`
- **What this ticket actually did:** 顺手修了：`directory` 认 `root`；`note` 那一种**认不出是本来的事**——它记的是「是哪几处」，而对上树上那一条要先有树，因此挪成一趟后置（新添的 `stand_on_a_note`，摆在 `watch_the_run` 之后），认不出当场 `panic`、不再默默落回输出目录那一行
- **Options:** ① 照现状 ② 另加一条用例：走一遍全部场景与全部序列的场景数据，核每一种 `kind` 都认得出（`cursor.kind` 认不出就红）——这一类洞的根治是「认不出要出声」，而我只在 `note` 那一支上做到了
- **Recommend:** ②，归 15 号票或 `/settle` 那一批：`views_of` 眼下还有几支是别的票要接的，一起收比这一票单收划算
- **Whose call:** 协调人
- **处置：** 待处理。
