# 07 — 措辞只写一处：隔离目录名、卷列表那一格、字形宽度那一关

**What to build:** - **隔离目录的名字**：库里那个常量公开，会话那一句读它（对外契约多一个常量，`cargo doc` 的告警水位跟着核一次）；
- **卷列表那一格**：跳过的卷读措辞层那一格，没做成的卷读措辞层那一行；那一处「期待成为死代码」的标注拆掉；
- **字形宽度那一关**：改问措辞层出的全部格（原样那一档照旧不问），不再跟着屏上摆成列的那几格走。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 会话里不再手写隔离目录的名字与「跳过」「没做成」两个词
- [x] 字形宽度那一关问的是措辞层出的全部格；拿一个歧义宽度字形放进一格不在屏上的格里去试，它红
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 三件，屏上一格不变（收停车场 Q870、Q961、Q962）。

1. **隔离目录的名字**（Q870）：`src/lib.rs` 的 `ISOLATED_DIRECTORY` 公开，文档说清为什么公开；crate 文档在主 seam 那一段带上一句。
   每页结果头一行末尾那个短标签（`src/session/shell/pages.rs` 的 `tally_line`）改读它：`format!("这一卷输出在 {}/", tonefit::ISOLATED_DIRECTORY)`。
   场景数据那条「去处照库的镜像规则」的用例（`src/session/scene.rs`）也改读它；集成测试与 `render` 用例里的字面没动（Q1360）。
   `cargo doc` 告警数核过：公开之后仍是 15 条——那一格的文档不链任何私有项。
2. **卷列表那一格**（Q961）：`src/session/shell/list.rs` 的卷行不再手写「跳过」「没做成」。跳过的卷读 `render::tally_column(卷级那几行)`，
   没做成的卷读 `render::tally_column(&[render::failed_volume(那一条)])`——为此 `Live::failure_at` 由私有改公开；样子（压暗、出事那一色）仍在画法那一层。
   `tally_column` 上那句 `expect(dead_code, …Q961)` 拆掉，换成与 `tally_pairs` 同一副 `cfg_attr(not(feature = "tui"), allow(dead_code))`
   （闸门 2 关掉 `tui`，那一半里只剩用例读它）。顺带：卷行一帧只拼一次 `render::volume`（`volume_rows`），代表页那一列、灰阶分布那一格、
   行尾跳过与隔离那两句共用这一份——从前代表页与行尾各拼一次，Q961 记的「多拼一次」的代价因此没付，跳过与隔离的卷反而少拼一次；
   `driver_of`、`sentence` 两个方法退成吃行的自由函数。
3. **字形宽度那一关**（Q962）：`src/render.rs` 那条用例改名 `every_glyph_this_layer_puts_in_a_cell_is_the_same_width_on_any_terminal`，
   不再从 `columns::wording_cells` 拿名单，改问措辞这一层出的**全部**格。哪几种不问只在用例模块的 `every_field` 一处：原样那一档（`Source`、`Output`）
   与成句的那一格（`Sentence`，Q1357）；那张名单挨着一个不留 `_` 的 `match`（Q1358）。`wording_cells` 留着做核对：屏上列里说自己出自措辞那一层的那一格，
   必须在关里。夹具扩到摆得出要问的每一种格（产物体积、纸色提白三格与逐页纸白、兜底上界、残缺、样张两格，外加没做成那一行），只放开跨页一格（Q1359）。
   指着旧名单的文档跟着改：`columns.rs` 模块文档、`Provenance::Wording`、`wording_cells`，`glyph.rs` 的 `width_is_stable`，`Field::PaperWhite`。
4. **钉住前两件的扫描**：`tests/single_source.rs` 新添 `the_isolated_directory_name_and_the_why_nothing_words_live_in_one_place`——
   代码里（`code_only`）隔离目录名只在 `src/lib.rs` 出现一次、「"跳过"」「"没做成"」两个字面只在 `src/render.rs` 各出现一次，家里那一格真住着，
   `pages.rs` 真读 `tonefit::ISOLATED_DIRECTORY`、`list.rs` 真调 `render::tally_column(`。

**没动的**：`CONTEXT.md` 一字没动——《格》本来就说措辞那一档字形宽度必须稳，这一关此刻才照它的字面问；设计稿与设计快照没动；
屏上别处提到「跳过」的那几句（总览「⋅ 跳过 N」、每页结果「这一卷跳过了：」，后者是 Q877）不是卷列表那一格，没碰。

### 按反跑了什么

- **字形宽度那一关**（票面验收第二条）：把 `render.rs` 里彩页转灰那一格（屏上不摆）的「彩页转灰」改成「彩页·转灰」。
  **旧那一关照绿**（`test result: ok. 1 passed`——那一格不在屏上的列里，名单里没有它）；**新那一关红**：
  `· 是东亚歧义宽度：PageVerdict 的 ColorToGray 那一格写着「彩页·转灰」`。改回之后绿。
- 新那一关里另外三道守卫各按反一次，各红在自己那一句上：兜底上界那一页改回没退回过 → `Backstop 在那一关里，而这一份夹具一行都没摆出它`；
  `every_field` 把 `Size` 改成不问 → `Size 摆在屏上的列里，却不在那一关里`；没做成那一句里的 `·` 去掉 → `Sentence 没喂进一个歧义宽度的字形，这一条的反面问不着`。
- **扫描那一条**：先写、在改代码之前跑，红在头一问上——`[(src/lib.rs, 1), (src/session/shell/pages.rs, 1)]` 对 `[(src/lib.rs, 1)]`；
  改完 `pages.rs` 之后红在「"跳过" 不止一处」上（`src/session/shell/list.rs` 那一份）；`list.rs` 接回 `render::tally_column` 之后绿。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 8cd6e81`。Spec 轴：三件都落了地，没找到实现错的；偏离票面字面的只有成句那一格（已记 Q1357，待拍板）。

**收下的**：

- `src/lib.rs` 的 crate 文档没提新公开的常量（Spec）——主 seam 那一段补一句。
- Q1360 原稿（「卷行一帧只拼一次卷级那几行」）的另一条路是稻草人（Standards）——删掉，那件事写进上面第 2 条与提交说明；原 Q1361 顺延成 Q1360。
- `every_field` 的文档说「添一种格先在这里编译不过，并把它添进名单」说过头了：编译器只管 `match`，名单不管（Standards）——改成如实说。
- 「原样与成句的不问」那一句在文档里重述了好几遍，`every_field` 文档里还有一句变更史（Standards）——用例文档与 `columns.rs` 两处改成指向 `every_field`，变更史那句删掉。
- 扫描里的 `standalone` 交的是个数、名字读着像谓词（Standards）——改名 `standalone_count`。
- `volume_row` 里同一份报告取了两次（Standards）——`volume_rows` 改吃报告。

**驳回的**：

- `Live::failure_at` 用 `pub(crate)` 就够（Spec）：`Live` 上读的那几手（`undone_at`、`report_at`、`elapsed_at`）都是 `pub`，照邻居写。
- `Asked` 不是词条（Standards）：它是用例模块里那一关的处置，不是领域概念；领域那一维（字面出处）照旧叫 `Provenance`。
- `why_nothing` 失败那一支先造整行再取一个词、绕一圈（Standards）：票面要的正是「没做成的卷读措辞层那一行」，不认行而直接认 `RowKind` 就是 Q961 要拆的那种绕开。
- 扫描用例的脚手架与转轮那一条相像、`sentence`/`driver` 与 `tally_column` 同形、跳过与没做成的样子在两处各配一遍（Standards，都是判断）：前一件是 `single_source.rs` 一贯的写法；后两件是既有代码挪了位置，不在本票。

### 停车场

本票用了 Q1357–Q1360：

- **Q1357**：那一关除了原样那一档，还放开成句的那一格；顺带记下「彩页 · 彩色分支」那一句里有一个中点。
- **Q1358**：「问不问」那张表按格写在 `render` 的用例里，`wording_cells` 留着做核对，字面出处仍挂在列上。
- **Q1359**：夹具扩到摆得出每一种要问的格，只放开跨页一格。
- **Q1360**：集成测试与 `render` 用例里的 `_isolated` 照旧手写，场景数据那一条改读库那一格。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt --check` 过之后，四条顺序跑。日志是 `os-07.gate1.log`、`os-07.gate2.log`、`os-07.gate3.log`、`os-07.polish.log`，
都在树外（`/Users/nicoer/dev/tonefit-wt/`），每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。** 本票新添一条用例（`tests/single_source.rs` 9 → 10 条）；`render` 那条是改名重写，bin 的条数没变。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1147 通过 1 失败**（1 ignored）；lib 253 / bin 478 / single_source 10；末行 `error: 1 target failed: \`--test concurrency\``；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.22s`（`concurrency`，Q995） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1006 通过 1 失败**；lib 253 / bin 337 / single_source 10；末行 `error: 1 target failed: \`--test concurrency\``；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 57.31s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.51s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数——公开 `ISOLATED_DIRECTORY` 没添一条 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 156.28 秒、闸门 2 上 150.56 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 478 条里，全绿；`npm run check`（`.scratch/session-redesign/`）`与库里那一份逐字节相同`。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q870 — 隔离目录那个名字在会话这一头写死了第二份，而库那一份是私有的

- **From:** 票 `session-redesign/11`
- **Kind:** 同一个字面有两处
- **Where:** `src/session/shell/pages.rs` 的 `tally_line`（`"这一卷输出在 _isolated/"`）；出处是 `src/lib.rs` 的 `const ISOLATED_DIRECTORY: &str = "_isolated"`——它**没有 `pub`**，而会话住在另一个 crate 里，够不着
- **Why it did not block:** 那一整句是**界面层自己的措辞**（设计稿 `drawPages` 写死的一句），不是报告那一格：库那一句是「隔离 N 页读不出：这一卷整卷写到隔离目录 <整条路径>，读不出的页用空白页占位，页码顺序不变」，一整条路径摆不进抬头那一行，而屏上那儿要的是一个短标签。卷行行尾摆的仍是库那一整句（`shell::list` 的 `sentence`），ADR 0016 那一条一处没破
- **What this ticket actually did:** 照设计稿逐字写了那一句，并在代码旁注明它是界面层的措辞、与卷行行尾那一句分工不同
- **Options:** ① 照现状：一个目录名两处，改名时要两处一起改 ② `ISOLATED_DIRECTORY` 提成 `pub`，这一头写 `format!("这一卷输出在 {ISOLATED_DIRECTORY}/")`——那是一次**对外契约扩大**，`cargo doc --no-deps` 那 15 条的水位要跟着核（Q183 那一笔是同一种代价） ③ 从 `report.output` 反解出那一截：**不取**——那是「表回头去认字符串」，ADR 0016《后果》拦的正是它
- **Recommend:** ②，与别的「库内私有常量被界面层需要」的账一起判
- **Whose call:** 拍板的人（库的对外契约）
- **处置：** 待处理。

#### Q961 — 卷列表那一格的「跳过」「没做成」写了第二份，`render::tally_column` 如今只有用例在读

- **From:** 票 `session-redesign/15`
- **Kind:** 单一出处裂开（措辞在画法那一层另写了一份）
- **Where:** `src/session/shell/list.rs` 卷行那一支的 `why_nothing`（手写 `"跳过"`、`"没做成"`）；
  对面是 `src/render.rs` 的 `tally_column` 与 `why_nothing_judged`——文档写着
  「「跳过」与「没做成」只有 `why_nothing_judged` 一处」
- **Why it did not block:** 屏上的字今天一个不差：设计快照逐格钉着那两个词，两处写的是同一串。
  旧画法退场之后 `tally_column` 在非测试那一趟没了读者，本票给它挂了
  `cfg_attr(not(test), expect(dead_code, …Q961))`，用例（render 自己那几条、场景夹具的
  `on_the_grid`）照旧读它。
- **What this ticket actually did:** 没动 `shell::list`；只让 `tally_column` 留着、挂上那一句期待，
  并把它文档里「读它的只有会话」改成此刻的读者。
- **Options:** ① 卷列表那一格改读 `render`：跳过的卷有卷报告，走 `tally_column(&render::volume(..))`；
  没做成的卷走 `render::failed_volume` 那一行——两个词从此只有一处，`tally_column` 的期待自己报没用上、拆掉；
  ② 认下两份，删掉 `tally_column`，`why_nothing_judged` 只剩目录那一级读，文档那句「只有一处」改掉
- **Recommend:** ①。ADR 0016「一套措辞」要的正是这个；代价是卷行每帧多拼一次那一卷的卷级行，
  只在跳过与没做成那两种卷上发生，比整棵树每帧已经做的事小得多。
- **Whose call:** 下一张碰卷列表那一格的票（或 `session-redesign/17` 量延迟时顺手）
- **处置：** 待处理。

#### Q962 — 「摆进列里的字形宽度稳不稳」那一关跟着屏上的表缩小了：裁白边、彩页转灰、跨页、画质分那一串、页数、卷数、统一档位分布不再被问

- **From:** 票 `session-redesign/15`
- **Kind:** 闸门的覆盖面随实现收窄（是不是该收窄，是一个决定）
- **Where:** `src/session/columns.rs` 的 `wording_cells`（从前从旧界面那三张表——目录表、卷表、
  逐页表——导出十三格，如今从卷列表那棵树与每页结果导出六格：灰阶分布、尺寸、缩放、判定、理由、
  判定那一档的分）；读它的是 `src/render.rs` 那条
  `every_glyph_this_layer_puts_in_a_lined_up_cell_is_the_same_width_on_any_terminal`
- **Why it did not block:** 那一关的规矩说的是「**摆进列里**的字形不许是歧义宽度」（`CONTEXT.md`《格》），
  而那七格在今天的屏上不成列——命令行那一份是成句的散文，错一格不牵连别人。按规矩的字面，
  收窄是对的；删掉旧表之后照旧导出旧表的列，就等于名单与屏各说各的。
- **What this ticket actually did:** 名单改从新两张表导出；那条 render 用例钉的两格换成「尺寸、缩放」，
  夹具补一处让「判定那一档的分」真摆得出来；`columns` 那条字面出处用例改问新两张表。
- **Options:** ① 照今天这样，名单只跟屏上的列走；② 另立一份「措辞那一层出的格一律要稳」的规矩，
  那一关改问 `render` 出的全部格（原样那一档照旧不问）——不再依赖屏上摆没摆成列
- **Recommend:** ②，但不急。那七格的字形今天全是稳的，收窄没有放进任何一个坏字形；
  ②的好处是下一次屏上换表时那一关不再跟着缩。
- **Whose call:** 拍板的人（`CONTEXT.md`《格》那条规矩的管辖面）
- **处置：** 待处理。
