# 08 — 补两道扫描，场景数据的光标种类扫一遍

**What to build:** 《砍列》《视口》各添一条扫描用例，照「拒绝开始的名单只住一处」那条的形状：词汇表那一条里只许有名字、不许有值。
再加一条用例走遍全部场景与全部序列的场景数据，每一种光标都认得出——认不出的键名当场红，不再默默落到输出目录那一行。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `tests/single_source.rs` 添两条；往词汇表那一条里抄一份次序去试，它红
- [x] 场景数据光标种类那一条：往一份场景数据里写一个认不出的种类去试，它红
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 三件，屏上一格不变（收停车场 Q615、Q846）：`scene.rs` 整个只在 `test` 里编，产品代码一行没动。

1. **《砍列》的扫描**（Q615）：`tests/single_source.rs` 新添 `the_order_columns_drop_in_lives_in_one_place`。
   要比的那几项**当场从家里读**（`dropped_in_turn`）：`src/session/columns.rs` 过一遍 `code_only` 与 `squashed`，按 `impl Column for` 分段，
   取 `DROPPED_IN_TURN` 那一串变体、到同一段的 `fn head(self)` 里认回屏上的列头——今天读出两个次序共六个列头。用例里只写名字
   （宣告那一格、列头那一手、两句路标、词条名），一个值都不抄（Q1379）。三件事一起问：词条那一行（`vocabulary_row`：`CONTEXT.md` 里以
   `| **砍列** |` 起头的那一行，Q1378）一个列头都不点名（点名任何一项就红，Q1377）；家里真宣告着次序（读不出就当场红，不交回空次序）；
   词条里「`session::columns` 的 `Column::DROPPED_IN_TURN`」那句路标还在。
2. **《视口》的扫描**（Q615）：新添 `the_list_of_places_sharing_a_viewport_lives_in_one_place`，同一个形状。要比的那几处从 `src/session/viewport.rs`
   里 `| 用在哪 |` 那张表读（`sharing_a_viewport`：每一行头一格括号前的字，今天七处），并核那张表挂在 `struct Viewport` 身上。
   词条里「唯一的例外是覆盖层」不算抄：那是「滚动量是算出来的」那条规矩的例外，叫的是《覆盖层》，不是表上那一格。
3. **场景数据的光标种类**（Q846）：`src/session/scene.rs` 里认光标那一段从 `views_of` 挪进 `cursor_of`，交 `Result<Cursor, String>`——
   种类认不出、该带的那一格缺了都交回一句话，`_ => Cursor::Output` 那一支没了；`views_of` 拿到就 `panic`。备注行那一支核 `what` 在场、
   仍先停在输出目录那一行，真认它的仍是 `stand_on_a_note`（Q1380）。新添 `every_cursor_kind_in_the_scene_data_is_one_the_fixture_reads`：
   走遍全部场景与全部序列走完那一刻的场景数据（14 + 173 = 187 份；`advanced` 那一格只带 `run` 与 `now_ms`，不带光标），认不出的连同它在哪一份里一起报，
   一个种类都不点名。顺带：用例模块里「全部场景加全部序列的场景数据」那一串抽成 `every_scene_data`，隔离去处那一条也改走它；
   `views_of` 文档里那句过时的「给预设起名那一种输入行随那一票认」删掉（`preset` 那一种早已认得）。
   `views_of`、`config_of` 里另几处 `_ =>` 照旧（Q1381）。

**没动的**：`CONTEXT.md` 一字没动（两条词条本来就只指路）；fixtures 一个字节没动；设计稿、设计快照、黄金快照没动。

### 按反跑了什么

都在 `CONTEXT.md`、fixtures 或源码上临时改一处、窄跑那一条、再从备份拷回；每次拷回后 `git diff --exit-code` 核过那几个文件一个字节不差。

- **《砍列》**：往词条「接着按次序砍」后面插「（卷列表：耗时 → 代表页 → 灰阶分布；每页结果：缩放 → 画质分 → 尺寸）」→ 红：
  `left: ["耗时", "代表页", "灰阶分布", "缩放", "画质分", "尺寸"]`——也证明那六个列头确是从家里读出来的。只插半截「（耗时最先让）」→ 红，`left: ["耗时"]`。
  删掉词条里的路标 → 红：`《砍列》里指回家里的路标「…」不在了`。
- **《视口》**：在「屏上凡是列在一个格子里的都共用这一份」后面插「（卷列表、预设栏、补全框都在内）」→ 红：`left: ["卷列表", "预设栏", "补全框"]`。
  在 `viewport.rs` 那张表与 `struct Viewport` 之间插一个 `const` → 红：`那张表不再挂在「struct Viewport」身上，而挂在「const PROBE: u8 = 0;」上`。
- **光标种类**：`sequences/ended-h.scene.json` 的 `"kind": "directory"` 改成 `"dir"`（Q846 原样那一个错名），`sequences/running-]d.scene.json`
  备注行光标的 `"what"` 改成 `"path"`（Q846 的另一个）→ 红，两份一起点名：
  `ended-h：光标的种类认不出：{"kind":"dir",…}`、``running-]d：光标少了 `what` 那一格：{"kind":"note","path":"私藏/"}``。
  这两份序列都不在「那一趟变了」那一批里，从前没有一条用例摆过它们走完那一刻的数据。评审之后抽出 `every_scene_data`，`dir` 那一处重跑一遍，照样红。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 5f64ed5`。Spec 轴：两条扫描与光标那一条都做到了，没找到会一直绿的判法；按反按代码推理逐条核过可信。

**收下的**：

- 两条扫描用例的文档里有变更史（「从前抄着一份……`p4-parking-lot/28` 把它改成……」，Standards）——改成说此刻成立的事：词条明写不抄，那是一句话，这一条是它的闸门。
- 砍列那一条的文档举例写了一个列头字面、`scene.rs` 新用例文档写了「那一百多份」（Standards，单一出处）——都去掉。
- `scenes()…chain(sequences()…)` 那一串在用例模块里出现第二次（Standards，Duplicated Code）——抽成 `every_scene_data`。
- `fn entry` 说不出交回的是什么、还被同名局部变量遮住（Standards，Mysterious Name）——改名 `vocabulary_row`，局部叫 `row`。
- Q1379 的 ② 像稻草人（Standards）——补一句：它是同一个文件里 `REFUSAL_MARKS`、`KEY_SENTENCE_MARKS` 的先例，不是凭空立的。

**驳回的**：

- 两条扫描主体同形、可抽一个共用断言（Standards）：这个文件的惯例是各写各的，每条自己说得出问的是哪三件。
- `cursor_of` 里的闭包 `said` 含糊（Standards）：`said` 在 `scene.rs` 里通篇指「场景数据说的那一格」（`config_of(said, …)`、`let said = sentence(…)`），照邻居写。
- 路标只核指向、不核「这里不抄第二份」那半句（Spec，story 27）：闸门撑的是那句话说的事——词条里没有第二份——由头一问拦；把那半句措辞也钉死，改个说法就红，换不来什么。
- 备注行那一支只核 `what` 在场、不核对不对得上树上哪一条（Spec）：票面要的是种类与键名认得出；对不对得上要树，摆场景那几条（`stand_on_a_note`）问的就是它。
- 票据没进 diff（两轴）：评审时还没写，现在补上；《数》随闸门那一趟。

### 停车场

本票用了 Q1377–Q1381：

- **Q1377**：两条扫描按「点名了任何一项」判抄，不按「整串依次出现」判。
- **Q1378**：只扫词汇表那一条，不照拒绝开始那一条扫全部交付文档。
- **Q1379**：要比的那几项当场从家里读（解析源码），用例里一项都不抄。
- **Q1380**：认光标挪进 `cursor_of`，备注行那一种在那里只核 `what` 在场，`stand_on_a_note` 照旧自己再读一遍。
- **Q1381**：只收了光标那一处；`views_of`、`config_of` 里另几处 `_ =>` 照旧默默落到默认那一档。

### 数

最终状态跑的那一趟：评审收完、`rustfmt --check` 过之后，四条顺序跑。日志是 `os-08.gate1.log`、`os-08.gate2.log`、`os-08.gate3.log`、`os-08.polish.log`，
都在树外（`/Users/nicoer/dev/tonefit-wt/`），每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。** 本票新添三条用例：`tests/single_source.rs` 10 → 12 条，bin 多一条（`scene.rs` 不在 `tui` 后面，两趟都有）。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1150 通过 1 失败**（1 ignored）；lib 253 / bin 479 / single_source 12；末行 `error: 1 target failed: \`--test concurrency\``；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.26s`（`concurrency`，Q995） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1009 通过 1 失败**；lib 253 / bin 338 / single_source 12；末行 `error: 1 target failed: \`--test concurrency\``；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 69.52s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 8.25s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 188.54 秒、闸门 2 上 257.36 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 479 条里，全绿；设计稿与场景数据一个字节没动。

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
- **处置：** **`one-source/08` 落地（2026-10-02）：照 ① 了结。**`tests/single_source.rs` 各添一条：词条那一行一项都不点名（比的那几项当场从家里读，用例里一项不抄）、家里真住着、路标还在（Q1377–Q1379）。

#### Q846 — `scene.rs` 的 `views_of` 认光标那两种读错了键名，而两棵树的闸门都绿

- **From:** 票 `session-redesign/10`
- **Kind:** 路过发现的无关缺陷（`08` 落进 `main` 的代码里就有）
- **Where:** `src/session/scene.rs` 的 `views_of`
- **Why it did not block:** 夹具那一头光标的 `kind` 只有六种：`out`／`add`／`path`／`directory`（带 `root`）／`volume`（带 `root`）／`note`（带 `what`）。`08` 写的是 `Some("dir") => Cursor::Directory(at("dir"))` 与 `Some("note") => Cursor::Note(at("path"))`——**两个键名都不是夹具里的那个**。`match` 认不出就落到 `_ => Cursor::Output`，一声不响；`cursor_line` 再把它挪到头一行停得住的。**两棵树的闸门当初都绿**，因为 `08` 摆得出的那五屏里光标全是 `volume` 或 `path`——**有用例，用例照不到**。我撞见它是因为 `envelope` 那一屏的光标是 `directory`（自动滚动又正好把光标带到同一行，差别因此还藏了一层），而 `ended-note*` 那四串的光标是 `note`
- **What this ticket actually did:** 顺手修了：`directory` 认 `root`；`note` 那一种**认不出是本来的事**——它记的是「是哪几处」，而对上树上那一条要先有树，因此挪成一趟后置（新添的 `stand_on_a_note`，摆在 `watch_the_run` 之后），认不出当场 `panic`、不再默默落回输出目录那一行
- **Options:** ① 照现状 ② 另加一条用例：走一遍全部场景与全部序列的场景数据，核每一种 `kind` 都认得出（`cursor.kind` 认不出就红）——这一类洞的根治是「认不出要出声」，而我只在 `note` 那一支上做到了
- **Recommend:** ②，归 15 号票或 `/settle` 那一批：`views_of` 眼下还有几支是别的票要接的，一起收比这一票单收划算
- **Whose call:** 协调人
- **处置：** **`one-source/08` 落地（2026-10-02）：照 ② 了结。**认光标挪进 `cursor_of`，认不出交回一句话、`views_of` 当场 `panic`，不再落到输出目录那一行；新添的用例走遍全部场景与全部序列的场景数据（Q1380）。`views_of` 另几处 `_ =>` 没收（Q1381）。
