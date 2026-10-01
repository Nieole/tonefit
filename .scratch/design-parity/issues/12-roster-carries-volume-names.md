# 12 — 卷清单带卷名，分卷序列在屏上叫序列的名字

**What to build:** 《清点摘要》多一格**卷名**：清点时库本来就读过归档头认「这一份是不是另一份的续」，卷名（分卷序列取序列的名字）
在那一刻算出来，随卷清单进开工那一条事件。会话写卷名一律读它，不再从卷根按名字猜——分卷序列在屏上叫
`第01卷`，不叫 `第01卷.part1`。这是 `design-parity` 里唯一一处库的对外形状变化；命令行报告的字节不变
（报告里的卷名不经这一格，命令行进度条本票不动）。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 事件流用例：开工那一条带的卷清单上，目录卷、归档卷、分卷序列头一份各有卷名，分卷序列那一个不带 `.partN`，续的那几份不在清单上
- [x] 会话里分卷序列那一卷屏上写序列的名字：夹具里放一个分卷序列的卷，断言卷行
- [x] 黄金快照原样过
- [x] `CONTEXT.md`《清点摘要》多一格卷名
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q849 — `render::volume_name` 给归档卷去掉扩展名，**旧界面跟着变**

- **From:** 票 `session-redesign/10`
- **Kind:** 我确实拿不准的单项（改的是一处**共用**件，与 Q809 同一类）
- **Where:** `src/render.rs` 的 `volume_name`
- **Why it did not block:** `envelope` 那一屏的当前卷写的是 `灰原哀/第05卷`，而卷根是 `~/下载/灰原哀/第05卷.cbz`——从前那一处取 `file_name`，屏上因此是 `第05卷.cbz`。`CONTEXT.md` 的《分卷序列》把卷名定成「序列的名字」，库那一头认卷名的 `source::name_of` 对归档取的正是 `file_stem`，去处也是 `<卷名>.cbz`：屏上带着**输入的扩展名**是把它当成了卷名的一截
- **What this ticket actually did:** 改成「归档卷取 `file_stem`，别的取 `file_name`」，认「是不是归档」走 `tonefit::is_archive`（只看扩展名、不碰盘）。**页名一格没动**：认得的归档扩展名里没有一个是图片格式，因此一页与一卷仍共用这一条规矩。旧界面（报告区的卷表、逐页表抬头）与**命令行的进度条**跟着变——闸门全绿、黄金快照原样过，而那正说明**旧那几条用例一格都没钉住它**（改之前也不会红）。这一票因此补了一条钉住它的用例（`render` 里 `a_volume_loses_its_archive_extension_and_a_page_keeps_its_own`：目录卷 · 归档卷 · 大小写 · 两种页名 · 分卷那一截）
- **另有一截仍对不上**：**分卷序列头一份在屏上仍带着 `.partN`**（`第01卷.part1`），而 `CONTEXT.md` 的《分卷序列》说卷名取**序列的名字**（`第01卷`）。去掉那一截要读一次归档头（`source::name_in_sequence` 看的是内容，不是名字），而屏上这一问不碰盘——**那是这一条改完之后剩下的第二处不一致**，用例把它钉成了当前事实，函数文档也写着
- **Options:** ① 照现状：一处出处，两副界面同一种写法 ② 新界面另写一份取 `file_stem` 的，旧那一份留着 `file_name`——同一个卷在同一个程序的两副界面上两个名字 ③ 把「一个卷叫什么」与「一页叫什么」拆成两处：那一处的文档明写着两者共用这一条，拆开要先改那句话
- **Recommend:** ①。`.partN` 那一截要收，得让会话拿着**清点清单上的卷名**去屏上写（那一头是库算出来的、读过归档头的），而清单眼下只带卷根——那是 15 号票让真会话切过来时顺手能收的一格
- **Whose call:** 协调人（觉得动共用件越了界就退回②）
- **处置：** **本票了结，照票面（原推荐 ①，那一截 `.partN` 照它说的收法收）**：清单带上库算出来的卷名（`SurveyedVolume::name`），会话写卷名一律读它，分卷序列那一卷在屏上叫序列的名字；`render::volume_name` 只剩命令行进度条与页名两个读者，进度条上那一截仍在（票面：进度条本票不动，Q1297）。见《落地记录》。

## 落地记录

**本票做了什么。** 库只多一格：`SurveyedVolume::name`（`pub name: String`，排在 `root` 之后）。
卷名在**发现认卷那一刻**就有了——点名的走 `source::identity_of`，发现的走 `source::volume_name_of`，两条都经
`name_in_sequence`，分卷序列头一份在那里换成序列的名字。它随 `discover::Candidate.name` → `survey::Surveyed.name`
→ `Survey::roster` 进开工那一条，库里不另算。会话那一侧照旧经 `Live::run_started` 收清单，`Tree::of` 把卷名
与卷根同序存下（`Tree::name(at)`；留在树上是因为搜索那一问在状态机里，够不着那一趟）。屏上写卷名的五处一律改读它：

| 屏上 | 从前 | 现在 |
|---|---|---|
| 卷行、目录行行尾那一卷（在跑、等待确认两支） | `shell::list` 的 `volume_name`：`render::volume_name(卷根)` | `Tree::name` |
| 总览的当前卷（确认条与它共用 `current_name`） | `render::volume_name(walking.volume)` | 卷根认回序号 → `Tree::name` |
| 每页结果面包屑末一截 | `render::volume_name(pages.volume)` | 同上 |
| 搜索比的「目录名/卷名」 | `render::volume_name(卷根)` | `Tree::name` |

- `render::volume_name` 留着，读者只剩命令行进度条与会话里的**页名**（每页结果的页面那一列、代表页那一列），文档改成如实说；
  分卷头在命令行进度条上仍是 `第01卷.part1`（票面：进度条本票不动，Q1297）。卷根认不回清单时写空串、不退回猜（Q1298）。
- 收编时卷名跟着卷根的写法走（Q1301）。
- `CONTEXT.md`《清点摘要》多一格卷名（新内容，当场加）；`survey`、`progress`（`RunStarted` 的 `roster`）、`live` 的模块文档跟着改。
- 夹具：场景夹具只多一格（清单上的卷名照场景数据的 `name` 给，`scene.rs` 别处一字没动）；`live::fixture::roster`、
  `tree`／`view` 两个 `listed` 补卷名（取卷根末一级）；`progress` 那条整份比 `Debug` 的用例多 `name` 一格。
- **用例三条**：
  - `tests/events.rs` 的 `the_roster_names_every_volume_and_a_split_sequence_after_the_sequence`：`库` 底下一个目录卷、一个 `.cbz`、
    一组两份的分卷（走发现），加一组直接点名头一份的分卷（走点名）；比整张「卷根 → 卷名」表，续的那一份多出来、名字带 `.partN` 都对不上。
  - `session::tree` 的 `a_volume_goes_by_its_name_on_the_roster_not_by_its_root`：分卷那一卷 `Tree::name` 与搜索比的那一截。
  - `session::shell` 的 `a_split_sequence_goes_by_the_sequence_name_on_screen`：设计稿里没有分卷序列，用 `live` 夹具手摆（Q1299）——
    `库/棋魂` 底下两组分卷，一组做完、一组在跑，目录展开，画 120×36；卷行、目录行行尾、总览当前卷、进了做完那一卷之后的面包屑
    四处写的是 `第01卷`／`第02卷`，两屏上**不许再有 `.part`**（反着钉）。
- 设计稿、设计快照一格没动；命令行报告一字节没变。

### 按反跑过的几遍（改完都还原了）

| 按反 | 结果 |
|---|---|
| `Survey::roster` 的卷名改成从卷根猜（归档取 `file_stem`、目录取 `file_name`） | 红：事件流那一条，`第01卷.part1`／`第02卷.part1` 对 `第01卷`／`第02卷`（发现、点名两条入口都红） |
| `Tree::searched_text` 退回 `render::volume_name(卷根)` | 红：树那一条，`棋魂/第01卷.part1` 对 `棋魂/第01卷`（这一条先红后改） |
| `shell::list` 的 `volume_name` 退回从卷根猜 | 红：屏上那一条，卷行 `✓ 第01卷.part1`（用例收成一处出处之后又按反一次，照红） |
| `overview::current_name` 退回 `render::volume_name` | 红：屏上那一条，`当前卷 棋魂/第02卷.part1` |
| 面包屑退回 `render::volume_name` | 红：屏上那一条，`任务 › 棋魂 › 第01卷.part1` |

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 54587ef`。

- **Spec**：缺项、越界都没有；确认条与总览共用 `current_name` 一起改对了。卷根认不回时写空串，已记 Q1298。
- **Standards，收了**：
  - `live` 模块文档那张表说当前卷来自「`VolumeStarted` 的卷名」，改成卷根加清单。
  - `view` 的 `listed` 文档说「本组用例不问卷名」，不对（搜索那几条比的正是它），改了。
  - 三处说命令行「手上只有卷根」，不实：`Bar` 收得到开工那一条，只是没留清单。改成如实说。
  - 屏上那条用例里步数、页数、卷路径各写了几遍（testing.md 第三条），收成一处。
  - `CONTEXT.md` 那一格复述了《分卷序列》读归档头那一截，收成引它。
  - `tests/events.rs`、`tests/discovery.rs` 两段旁注跟着改。
- **Standards，记下没改**：
  - 场景与终端那几条用例仍拿 `render::volume_name` 推卷名期望值：`scene.rs` 是热点，`terminal.rs` 另一槽在改，记 Q1302。
  - `render::volume_name` 名不副实，记 Q1300。
  - **驳了**：`Tree` 的 `roots`／`names` 平行数组（Data Clumps）。与同一个结构上的 `homes`、`index` 一个写法，清单开工后不变，拼树那一刻一次立齐。
  - **驳了**：`index_of → name` 在两处各走一遍。各自还要那个序号去问目录，抽一个按卷根问名字的方法省不下那一步。

### 数

review 收完、改完、`cargo fmt` 过之后跑的那一趟就是最终状态。四条顺序跑，日志 `dp-12.gate1.log`、`dp-12.gate2.log`、
`dp-12.gate3.log`、`dp-12.polish.log`，都在树外。这台 macOS 上闸门 1、2 在基线就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995），本票没碰。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1142 通过 1 失败**；lib 253 / bin 476（1 ignored）；红的那一个：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.18s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1002 通过 1 失败**；lib 253 / bin 336；红的那一个：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 55.73s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | `GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | `POLISH_EXIT=0`；`全绿。`；两道 clippy 零告警，`cargo doc` 告警 15 条（与基线同数） |

**本票加的用例**：闸门 1 多 3 条（events、tree、shell），闸门 2 多 2 条（events、tree；shell 那条挂在 `tui` 后面）。
基线 `54587ef` 没有重跑出总数。**黄金快照**：`tests/golden.rs` 与快照一字未动，`tests/golden-snapshot.txt` sha256 仍为 `2a6aabc0…`，
两趟闸门都过。
