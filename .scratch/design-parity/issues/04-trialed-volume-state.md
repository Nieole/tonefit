# 04 — 《预览过》：答了不写出的那一卷在屏上认得出

**What to build:** 确认点上答了「不写出」、分析做完而写出环节一步没走的那一卷，收摊时落在卷状态新添的一档
**《预览过 (Trialed)》**，判的依据是这一卷写没写。行首记号与完成同一套（有需留意的是 `!`，没有是 `✓`），
行尾照设计稿写「已分析，未写出」，照样展得开；**总览上的「完成」不数它**。设计稿本来就有这一档，
导出一串看得到这一卷与总览的序列钉住它。

**Blocked by:** 01

**Status:** resolved

- [x] 卷状态多一档预览过：答了「不写出」的那一卷落在这一档；答继续、答后面都写出、不等人的那几卷照旧完成
- [x] 卷行行首记号与行尾照设计稿；总览上的「完成」不数它
- [x] 那一卷展得开，每页结果照常
- [x] 导出一串（从确认点答「不写出」到这一卷收摊、总览看得见）比整屏且绿；夹具不再把这种卷当完成喂
- [x] `CONTEXT.md`《卷状态》添《预览过 (Trialed)》
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q769 — 设计稿的卷状态多一档 `trialed`（确认点上答了不写出的那一卷），库那一侧它收摊成完成

- **From:** 票 `session-redesign/05`
- **Kind:** 上游（03）说「七种 state 各有一档」的边界
- **Where:** `design.html` 的 `finishVol(run, 'trialed')`、卷行行尾「已分析，未写出」；`tests/fixtures/design/sequences/deciding-s.scene.json`（唯一带它的一份）；`src/session/live.rs` 的 `VolumeState`（八档，没有它）；`src/session/scene.rs` 的 `state_of`／`replay`
- **Why it did not block:** 事件流上它就是「确认点上答了做完再停、随后一卷跑完」——`Live` 记成完成，`decided` 记着那个字，`has_written` 不翻。夹具照这条路喂（答 `Finish`，写出环节一步不走），用例把它比作完成。11 个场景里没有它，只有一串序列有
- **What this ticket actually did:** 映射成 `Done`，答话记 `Finish`
- **Options:** ① 12 号票画卷行时从别处认出「完成但没写」（这一趟 `started_as` 是预览、那一卷收摊时 `Walking::writes` 为假——`Live` 今天不按卷记这一格）；② `VolumeState` 加第九档 `Trialed`（`volume_finished` 时按 `Walking::writes` 分）；③ 设计稿把它并进 `done`、行尾那句改由别的东西说
- **Recommend:** ②——行首记号与行尾那句照卷状态画（`CONTEXT.md`《卷状态》），少一档就得回头认别的格
- **Whose call:** 12 号票的实现者（等待确认）
- **处置：** **本票了结，照推荐 ②**：`VolumeState::Trialed`，收摊时按那一卷写没写分（`Live::volume_finished`）；夹具 `state_of` 把 `trialed` 读成它。见《落地记录》。

#### Q674 — 翻面之后，答收尾（等于试算）的那几卷也数进「完成」

- **From:** 票 `no-false-line/04`
- **Kind:** 票面治的那个病换了一种卷还活着，本票没伸手
- **Where:** `src/session/draw/overview.rs` 的 `finished_and_skipped`——数的是 `Report::volumes`
  里不是跳过的每一卷；用例 `the_overview_changes_sides_when_the_first_volume_is_written_not_when_go_on_is_answered`
  末一问（答了收尾的卷三进了「完成 3 卷」）
- **Why it did not block:** 票面与 spec 第三节定的是**分岔谓词**换成哪一个、翻面点在哪，一个字没提翻面之后
  「完成」数什么；那个数与报告末尾那几小结同一份数据（`settled_row` 的文档），改它就是改两处的口径。
  这一趟里答收尾的卷**报告照出**、判定照有，说它「处理过」不算假，只是「完成」这个词在 `x` 起的一趟里
  含着「写了出去」，在混着答的一趟里不含。
- **What this ticket actually did:** 翻面点落在第一卷真写完（`Live::has_written`，逐卷的依据是
  `Walking::writes`），翻面之后照旧走 `finished_and_skipped`；用例把这一格的现状钉住，改口时它会红。
- **Options:** ① 保持：「完成」读作「处理过、收了摊」，与末尾那几小结一致；② 结论行执行那一副多一格
  「试算 N 卷」（答收尾的那几卷），`完成` 只数写出去的——要 `Live` 逐卷记「这一卷写了没有」
  （`Walking::writes` 已经有，收摊时收进一个数即可），措辞一句、快照一张；③ 同样逐卷记，但
  「完成」只数写出去的、答收尾的既不进完成也不进跳过——那几卷在这一行上消失，
  与抬头的「N 卷」对不上数。
- **Recommend:** ②。这一格的病正是「说了一句此刻不成立的话」，而「完成 3 卷」里有一卷盘上没有；
  数据 `Live` 上已经有一半，改动不出 `overview.rs` 与 `live.rs`。
- **Whose call:** 拍板的人（「完成」这个词在混着答的一趟里指什么）
- **处置：** **本票了结，照票面与设计稿 `runTotals`**：「完成」只数写了出去的卷，预览过的那一卷不进完成、也不进等待（它收摊了）——形状是原条目的 ③，
  抬头的「N 卷」与「完成 · 跳过 · 等待」三个数之和差的正是那一卷（答「不写出」就此收场，一趟至多一卷）。原条目点名的 `finished_and_skipped` 与那条用例已随旧界面退场。

## 落地记录

**本票做了什么。** 设计稿本来就有这一档（`design.html` 的 `answer` 那一支 `finishVol(run, 'trialed')`），设计稿一个字没改；
实现跟上，导两串新的期望屏钉住。结转的 Q769、Q674 都收了。

| 处 | 设计稿（`design.html`） | 实现 |
|---|---|---|
| 卷状态 | `s` 那一支 `finishVol(run, 'trialed')`；`FINISHED` 含它 | `VolumeState::Trialed`。`Live::volume_finished`：走到写出那一遍（`Walking::pass` 是写出）而这一卷没写（`Walking::writes` 假）就是它，排在隔离之前——带坏页也是它；`settled` 含它 |
| 行首记号 | `volMark` 里 `trialed` 与 `done` 同一支 | `marks::volume_mark` 两档同一支 |
| 行尾 | 有需留意的几样就报几样，没有写「已分析，未写出」（`c-gray d`） | `list::volume_tail`：`Trialed` 有需留意的走完成那一支，没有写「已分析，未写出」（`Look::FAINT.dim()`） |
| 总览 | `runTotals`：`done` 不数它，`finished` 数它 | `overview::Counted` 本来就只把完成与进了隔离数进「完成」；它收摊了、不进等待；已分析照数 |
| 展得开、`]d` | `EXPANDABLE` 含它；`targets` 与完成同一个条件 | `opens_the_pages` 含它；`troubled_at` 与完成同一支；`terminal::open_a_volume` 那道守卫跟着 |
| 夹具 | 场景数据 `state: "trialed"` | `scene::state_of` 读成 `Trialed`（从前读成完成）；`replay` 照旧答 `Finish`、写出环节一步不走，判它的是 `Live`，夹具不替它判 |

- **导两串**（`.scratch/session-redesign/export.js`）：`deciding-x-advance-s-l`——`x` 写完第 5 卷、推进 30 秒到第 6 卷的确认点、`s`、`l` 展开哆啦A梦：
  总览「完成 1 卷 ⋅ 跳过 4 卷 ⋅ 等待 78 卷」，第 6 卷「✓ … 已分析，未写出」；`deciding-x-advance-s-trialed-l-a`——再挪到第 6 卷、`l` 进去、`a` 列全部页，224 页逐格。
  第二串本想停在 `l`（只列需留意的页），撞上一句与本票无关的措辞差异，改成多按一下 `a`（Q1169）。
- **推进之后又答了话**：夹具没有线程，推进那一步摆的是场景数据说的那一趟，而一串只有走完那一刻一份——那时第 6 卷已经收摊，`s` 答不到话。
  导出改成推进完那一刻另记一份，走完那一刻那一趟与它不同时写进场景数据的 `advanced`（`run`、`now_ms`）；Rust 侧 `scene::advanced_data` 读它，
  `terminal` 的 `walked` 推进时摆它（Q1167）。既有 148 串一字节没变，`advanced` 只在本票两串上。
- **夹具替线程收摊那一处认法改了**：从前认 `Live::decided() == Finish`，而那一格记的是答过的字里最弱的那一个——先答过继续再答「不写出」，它仍是继续。
  改成认「会话答完了话、那一卷却仍停在确认点上」（答继续的那一卷当场翻成处理中）。
- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过：只有 `manifest.json` 多两条（+96 行）与六个新文件，别的快照、场景数据一字节没变。
  node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。
- **用例**：
  - `live`：新 `a_volume_answered_not_to_write_settles_as_trialed`——答继续、答「不写出」、答「不写出」且带坏页、答过「后面的卷都写出」四卷，外加不等人的一趟，
    收摊各是完成／预览过／预览过／完成／完成；预览过收摊了、展得开。
  - `terminal`：新 `a_volume_answered_not_to_write_is_trialed_and_not_counted_as_done`——两串比整屏；屏上反着钉「完成 2 卷」不许出现
    （新 `shell::design::lines_of` 读实际那一屏的字网格，与 `Expected::lines` 共用一副读法）。
  - `scene`：新 `every_sequence_that_moves_the_run_after_advancing_stands_where_it_advanced_to`——推进完那一刻那一份也与它的场景数据逐项相同；
    `every_sequence_that_moves_the_run_ends_where_its_data_says` 照旧覆盖两串走完那一刻（第 6 卷是预览过）。
  - `view`：`opens_the_pages` 那一张表添预览过。
- **`CONTEXT.md`**：《卷状态》添《预览过 (Trialed)》（新词，当场加），别的词条没动。
- **代价**：导出与夹具多一格 `advanced`（Q1167）；预览过的那一卷带坏页时照设计稿行首 `✓`、`]d` 跳不到，与词汇表读下来的 `!` 相左（Q1168）；
  一页都不需留意的那一卷进去那一句两边措辞不同、仍没有一串钉着（Q1169）。

### 按反跑过的几遍（实现落地之后按反、跑 `cargo test --bin tonefit <过滤>`、还原）

| 按反 | 结果 |
|---|---|
| `volume_finished` 里判预览过那一格恒假（本票之前的样子） | 红 2 条：`a_volume_answered_not_to_write_settles_as_trialed`（四卷成了完成／完成／进了隔离／完成）、`a_volume_answered_not_to_write_is_trialed_and_not_counted_as_done`（反钉那一句：「完成 2 卷」上了屏） |
| 总览把预览过数进「完成」 | 红 1 条：`a_volume_answered_not_to_write_is_trialed_and_not_counted_as_done`，红在反钉那一句 |
| 夹具替线程收摊照旧认 `decided() == Finish` | 红 1 条：同上，「答「不写出」之后那一趟收了场」——第 6 卷没人替它收摊 |
| 推进摆走完那一刻那一份（不读 `advanced`） | 红 1 条：同上，`deciding-x-advance-s-l` 比屏红在屏底（`s` 在结束了的那一趟上按下去，说的不是「已结束预览」） |

实现之前那几趟：`live` 那条新用例编不过（没有 `Trialed`）；`Live` 落地、夹具还把 `trialed` 读成完成时，
`every_sequence_that_moves_the_run_ends_where_its_data_says` 红在 `deciding-s` 第 5 卷（`Trialed` 对 `Done`）；
导出两串、行尾还没写时，`deciding-x-advance-s-l` 比屏红在第 6 卷行尾（期望「已分析，未写出」，实际空着），总览那几行已经对得上。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff ab02adf`（未提交的工作树）加六个新文件。

**收下的**：

- **`lines_of` 照抄了 `Expected::lines` 的身子**（Standards，Duplicated Code）：两处共用一个 `text_of`。
- **`volume_finished` 里当前卷取了两遍**（Standards）：绑一次。
- **新用例里序列名写了三遍**（Standards，testing.md「一条用例里同一个数只有一个出处」）：收成一处。
- **Q1168 没说清它与词汇表相左**（Spec）：《需留意的页》含坏页、《行首记号》「`!` 需留意（…有需留意的页）」、《卷列表》`]d` 跳有需留意的页的卷、
  票面「有需留意的是 `!`」——四处读下来是 `!`，照设计稿是 `✓`。条目重写，点名这四处与设计稿那三处，推荐改设计稿。

**驳回的**：

- **Q1168 该停线**（Spec）：停线三种都不沾——两个方向都说得通、翻过来只动票面牵到的那几处、没有一景一串碰得到这一格，不可逆的一步都没有。照设计稿走、记条目、回话里点名。
- **「`Done | Trialed` 且有需留意的就是 `!`」散在三处，收成一个谓词**（Standards，Repeated Switches）：那三处（行首记号、行尾、`]d`）本来就各有完成那一支，
  预览过跟着进同一支；Q1168 拍板之前再立一个谓词是替还没定的事搭架子。
- **「完成」不数它该写进《总览》而不是《卷状态》**（Standards，单一职责）：票面点名《卷状态》；动《总览》是改写已有词条，要先拍板。
- **反钉那一句与整屏比对重复**（Spec）：testing.md 第二条要的就是它——整屏比对换了期望屏就跟着换，反钉那一句不跟。

### 停车场

本票用了 Q1167–Q1176 里的三个：

- **Q1167**：推进之后又答了话的那一串，导出另记推进完那一刻（`advanced`），不另立一景；推荐照现在。
- **Q1168**：预览过的那一卷带坏页，照设计稿 `✓`、`]d` 跳不到，与词汇表相左；推荐改设计稿。
- **Q1169**：一页都不需留意那一句两边措辞不同、没有一串钉着；推荐设计稿用词汇表的叫法、补一串。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-04.gate1.log`、`dp-04.gate2.log`、`dp-04.gate3.log`、`dp-04.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1075 通过 1 失败**；lib 239 / bin 447；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.84s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **953 通过 1 失败**；lib 239 / bin 325；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.35s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `ab02adf`，没在它上面重跑：`design-parity/07` 记的 1067／946 是在 `a9d2978` 上量的，`say-and-stop/02`（两趟各多 5 条）先进了 main，
推得闸门 1 **1072 通过 1 失败**、闸门 2 **951 通过 1 失败**。**闸门 1 多 3 条、闸门 2 多 2 条，都在预期里**：
`a_volume_answered_not_to_write_settles_as_trialed`（live）与 `every_sequence_that_moves_the_run_after_advancing_stands_where_it_advanced_to`（scene）两趟都编；
`a_volume_answered_not_to_write_is_trialed_and_not_counted_as_done`（terminal）只在默认那一趟。

**黄金快照逐格没动**：`git diff ab02adf -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。
