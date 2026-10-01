# 10 — 屏上说出留下几页与各环节耗时

**What to build:** 按页跳过的卷，每页结果上说出这一卷**留下了几页**（《留下的页》不进逐页结果，屏上只报个数）；
展开一卷时给出它**各环节花了多久**，数取卷级计时那几段。位置与措辞由设计稿先定，实现照它。

**Blocked by:** 01

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 按页跳过的卷，每页结果上说出留下几页；没有留下的页时这一句不在
- [x] 展开一卷时看得到各环节耗时，与报告里那一卷的卷级计时逐段相同
- [x] 各导出一屏或一串，比整屏且绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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

## 落地记录

**本票做了什么。** 「展开一卷」在新界面里就是**每页结果**（`CONTEXT.md`《每页结果》的旧称），两件事都落在那一屏的头两行上。先改设计稿、重导，再改实现。

| 件 | 设计稿（`design.html`） | 导出（`export.js`） | 实现 |
|---|---|---|---|
| 留下几页（Q682 → Q1247） | `drawPages` 头一行在「需留意 N/M 页」之后添 `   留下 N 页没重做`（`retainedOf`），没有留下的页时整截不在 | 每卷添 `retained_pages`（只在按页跳过的卷上），`page_count` 把它加回去 | `shell::pages::tally_line`；分母 M 改读 `report.pages.len()`（这一趟做了的页，与设计稿 `all.length` 同一个数，Q1249） |
| 各环节耗时（Q624 → Q1248） | 框里正文头三行改成灰阶分布 · **耗时** · 列头（`PAGES_HEAD`）；`tookSegs`：`耗时 摊开 1s ⋅ 查重 1s ⋅ 分析 1s ⋅ 写出 3s`，只列走过的环节（Q1250）；跳过的卷那一句上移到头一行、耗时照旧在第二行（Q1251）；模拟每卷按环节记 `took` | 每卷添 `took_s`（键照 `VolumeTiming` 的字段名，只写走过的段） | `session::passes::took`（按走的次序、取自 `VolumeTiming` 那一格、零的那一段不列）；`shell::pages::timing_line`（词出自 `passes::name`、色出自 `marks::pass_look`、数出自 `marks::spell`）；`HEAD_ROWS` 一处定头三行 |
| 一卷按页跳过的假数据（Q1252） | 新一景「按页跳过」（`retained`）：只点 `~/下载/虫师/第04卷.rar`，重做第 41–64 页；模拟上分析环节只走重做的那几页（`passLen`），写出环节一页一步；假盘虫师那一层多一个 `.rar` | `SNAPSHOTS` 添 `retained` 两屏；`pages-click-page` 点的那一行下移一格（第五页如今在第 15 行） | `scene` 夹具读 `took_s`／`retained_pages` 造卷级计时与留下的页，分析环节只喂重做的那几步；`agrees_with_its_data` 逐段核卷级计时与留下几页 |

- **重导之后** `git diff --stat -- tests/fixtures/design` 读过：网格（`*.txt`）变动只在每页结果那几屏——`pages` 两屏与十一串（`ended-l`、`ended-l-a`、`ended-l-a-j`、`ended-Enter`、`ended-skipped-l`、`pages-a`、`pages-j`、`pages-click-page`、`deciding-v`、`deciding-x-advance-s-trialed-l-a`、`envelope-deciding-v`），每一屏都只是第二行插进耗时、表往下一格（跳过的卷那一句上移一行）；
  新文件是 `retained` 两屏与它的场景数据；`manifest.json` 添两屏、改一处点击坐标；场景数据（`*.json`）每卷多一格 `took_s`，是机械的。node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过（快照数那一条改成 14 个场景）。
- **撞见没收的**：一页都不需留意那一句设计稿写「需要留意」、实现写「需留意」，没有一屏钉着（Q1253）；`CONTEXT.md`《每页结果》没补第二行与留下几页（Q1254，改已有词条要先拍板）。
- **`src/report.rs`、`src/render.rs` 一字未动**（另一槽在改）；命令行输出与黄金快照不变。
- **用例**：
  - `passes`：新 `the_passes_walked_are_the_segments_of_the_volume_timing_in_walk_order`（四段原样、按走的次序；跳过的卷只剩查重，总数不是一段）。
  - `shell`：新 `the_retained_scene_matches_its_design_snapshot_wide_and_narrow`；`the_timing_line_on_the_pages_reads_the_volume_timing_segment_by_segment`（「按页跳过」宽窄两屏上的耗时那一行逐段等于那一卷 `VolumeTiming` 四格各自的写法；目录卷那一屏从查重说起、没有摊开）；
    `only_a_volume_skipped_by_page_says_how_many_pages_it_retained`（「留下 N 页没重做」与分母 `/{重做的页} 页` 在宽窄两屏上；反着钉：分母不是整本书；整卷重做的卷屏上没有「留下」）。
  - `scene`：`agrees_with_its_data` 添卷级计时四段与留下几页两项；场景数断言 13 → 14（`scene`、`shell::design` 各一处）。

### 按反跑过的几遍（每一遍改一处、跑 `cargo test --bin tonefit <过滤>`、还原，还原后 `git diff --stat` 核过）

| 按反 | 结果 |
|---|---|
| 实现之前（设计稿已重导、新用例已落） | 红：`the_pages_scene…`、`the_retained_scene…`、`the_timing_line…`、`only_a_volume…`；`passes::took` 落地之前那条用例编不过 |
| 留下几页那一截不画（`retained_pages > 0` 的条件按反） | 红：`only_a_volume…`、`the_retained_scene…` |
| 查重那一段读成分析那一格（`segment` 里 `Fingerprint => first_pass`） | 红：`the_timing_line…`、`the_pages_scene…`（`retained` 那一景两格同是 `1s`，照绿——那一条用例咬住了） |
| 零的那一段照列（`took` 不过滤） | 红：`passes` 那一条、`the_timing_line…`、`the_pages_scene…` 与每页结果那七串（`terminal::redesign`） |
| 分母读回 `page_count()` | 红：`only_a_volume…`（反着钉那一句）、`the_retained_scene…` |

### 数

评审收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-10.gate1.log`、`dp-10.gate2.log`、`dp-10.gate3.log`、`dp-10.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1114 通过 1 失败**；lib 252 / bin 463（新添 4 条）；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.04s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **976 通过 1 失败**；lib 252 / bin 325（新添 `passes` 那 1 条）；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 58.84s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 1b1e682`（未提交的工作树）加新文件。

**收下的**：

- **留下几页那一截与命令行那一句是同一个数的两份措辞**（Standards，ADR 0016）：`pages.rs` 模块文档那句「唯一撞车的是跳过那一句」不再成立，改成点名两处；Q1247 写上撞车与第④种摆法（取库那一句整句）。
- **`export.js` 的 `SEGMENT` 插在 `sceneData` 的文档与函数之间**（Standards）：挪到文档之上。
- **`pages-click-page` 那句注释写的是变更史**（Standards，CLAUDE.md《文档写作》第 1 条）：改成第五页落在第 15 行。
- **Q1254 漏了「跳过的卷只说一句」**（Standards、Spec 两轴都报）：补上。
- **Q1247 的 80×24 论证只算了本景**（Spec）：补最坏情形（三档灰阶、三位数约 77 格，先截短标签、再截「没重做」末一字，「留下 N 页」恒在）。
- **用例名用 `kept`**（Standards，CLAUDE.md：测试名取自词汇表《留下的页 (Retained page)》）：改成 `…_it_retained`。

**驳下的**：

- **「环节 → 段」那层映射在 `passes::segment`、`export.js` 的 `SEGMENT`、夹具的 `Took` 各有一份**（判断题）：后两份是设计稿那一头与它导出的数据形状，Rust 里只有 `passes::segment` 一份；挪进库的 `VolumeTiming` 要动 `src/report.rs`，本轮那一个文件归另一槽。
- **`took`／`took_s` 没用「段」**（判断题）：`pages.rs` 里屏上那一截字叫 `Segment`，函数再叫 `segments` 同一处两个意思；`took` 读作「各环节花了多久」。
- **场景数写死在四处**、**`tally_line` 里「暗标签 + 加粗的数」连着两组**（判断题）：前者是既有的写法，后者两组各三行，抽出来不比现在好读。
- **分母换掉（Q1249）、跳过的卷那一句上移（Q1251）算越界**（Spec，边缘）：两件都是添这两样之后那一屏必须定下来的摆法，各记了停车场，整卷重做的卷屏上不变。

