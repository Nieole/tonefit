# 06 — 转轮一段算式，会话的时钟一个起点

**What to build:** 会话的时钟起点只有一格——视图那一层那一格；行首记号的转轮从它算、再加自己那一行的错相，算式只有一段。
场景夹具不再为两格起点各摆一次（Q864 那次合并撞出来的次序问题随之消失）。屏上一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 转轮的算式与时钟起点各只有一处；单一出处扫描（`tests/single_source.rs` 那一族）添一条钉住
- [ ] 场景夹具只摆一个时钟起点
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
- **处置：** 待处理。
