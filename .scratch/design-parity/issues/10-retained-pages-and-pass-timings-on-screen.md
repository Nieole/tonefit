# 10 — 屏上说出留下几页与各环节耗时

**What to build:** 按页跳过的卷，每页结果上说出这一卷**留下了几页**（《留下的页》不进逐页结果，屏上只报个数）；
展开一卷时给出它**各环节花了多久**，数取卷级计时那几段。位置与措辞由设计稿先定，实现照它。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [ ] 按页跳过的卷，每页结果上说出留下几页；没有留下的页时这一句不在
- [ ] 展开一卷时看得到各环节耗时，与报告里那一卷的卷级计时逐段相同
- [ ] 各导出一屏或一串，比整屏且绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q682 — `VolumeReport::pages` 只列这一趟做了的页，留下的页不进逐页结果，只有一个数 `retained_pages`

- **From:** 票 `two-pass-rework/14`
- **Kind:** 报告形态的取舍——13 号票的实现者报的 (c)「报告与 `Skipped` 的形态」
- **Where:** `src/report.rs` 的 `VolumeReport::pages`／`retained_pages`／`page_count`；`src/render.rs` 的 `retained_row`
  （`RowKind::Retained`，成句）；`src/render/plain.rs` 与 `src/session/draw/report.rs` 各多一条穷举分支
- **Why it did not block:** 票面只要「报告说得出这一卷跳过几页、重做几页」，两个数说得出；`VolumeVerdict` 不动
  （`PerPage`／`Override` 仍答「候选从哪来」，二十几处 `Some(VolumeVerdict::PerPage)` 的断言一处没改），
  `Skipped` 仍是整卷那一种。另一条路——给 `PageOutcome` 加一种 `Retained` 让 `pages` 仍是整本书——要么在报告里
  编几何、判据、判定那几格（留下的页一样都没算），要么让 `page_row`／`geometry_cells`／`notable`／会话逐页表
  各认一种新页，动到 `src/session/draw/` 的措辞，票面明写不动。
- **What this ticket actually did:** `pages` 的文档改成「这一趟做了的输出页」，`page_count()` 把留下的加回去；
  `retained_pages > 0` 时卷级多一行「按页跳过 留下 N 页、重做 M 页：……」排在过期副本之后、判定之前——
  底下几何门、纸白对齐、档位分布数的都只是重做的那几页，先说清整本书里几页没重做那几个数才读得对。
  会话卷表上档位分布那一列因此对按页跳过的卷写的是重做那几页的分布（两百页留下一百九十九页时是 `2bit+FS 1`），
  `under()` 只画过期副本与部分救回两句，这一句没画——本票不进 `src/session/draw/`。
- **Options:** ① 现状；② 会话卷表 `under()` 把 `RowKind::Retained` 那一句也画上（一行，`session/draw/table.rs`），
  或档位分布那一格附「· 留下 199」；③ `PageOutcome::Retained`，逐页表列出留下的页各一行「留下」。
- **Recommend:** ②，单独派——那是 `session/draw/` 的一行；③ 不值。
- **Whose call:** 协调人
- **处置：** 待处理。

#### Q624 — story 5「一趟跑得慢时知道慢在哪一步」的出口是 profiling 剖面，产品构建里没有

- **From:** 票 `two-pass-rework/03`（判 `wontfix` 时）
- **Kind:** 一个真需求，spec 给它配的方案写下时就假
- **Where:** spec story 5；spec P-A 第 82 行（已订正）；`src/cost.rs` 模块文档「不许合流」；
  `p0-hardening/13` 那条禁令；`CONTEXT.md`《阶段》
- **Why it did not block:** 需求今天有出口——`cargo build --features profiling` 之后跑，`cost` 把各阶段
  纳秒印到 stderr。开发者用得上，普通用户看不到，而 story 5 说的是「用户」。
- **What this ticket actually did:** `03` 判 `wontfix`，spec 那句订正，本条记「需求没满足」这件事。
- **Options:** ① 记下，需求由 profiling 剖面兜着；② 立一条决定推翻 `p0-hardening/13`，让阶段计时
  从量具升格为产品输出——要先答「各线程墙钟之和」这个数给用户看意味着什么；③ 不推翻禁令，但在会话
  卷表已有的 `Elapsed` 之下再细一层「三段各多久」（`VolumeTiming` 已经有，只是会话没全印）——那不是
  阶段，是段，禁令不管它。
- **Recommend:** ③ 先做，①兜底。`VolumeTiming` 三段（对指纹 / 读图定档 / 按档写出）今天已在 `Report` 里、
  已有卷级粒度、禁令明写「卷级三段计时已经在 Report 里了」——把它印到展开那一副，就能答「慢在哪一遍」，
  多数时候够用（跑得慢多半是读图定档那一遍）。②要动的东西太大，而且量具那个数本来就不是给人看的。
- **Whose call:** 拍板的人（③要在 spec 里补一句、可能并进 `01` 号——它正在动展开那一副的措辞）
- **处置：** 待处理。
