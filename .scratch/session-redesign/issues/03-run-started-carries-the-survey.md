# 03 — 开工那一条事件带上清点产出，`Live` 按清点清单记住每一卷

**What to build:** 清点一完成，会话就要画出整棵树——还没轮到的卷与它们的页数、分区末尾的备注行。而今天开工那一条
只带卷数与总步数，卷清单、非漫画文件、无法访问的地方要到收场才进报告（停车场 Q719）。

- `RunStarted` 追加三样：**卷清单**（照发现的次序；每一卷的卷根、步数上界、源页数），**非漫画文件**与
  **无法访问的地方**两张表。库里不新算东西——三样在发这一条时都已齐了，源页数是清点数成员时本来就数出来的，只是没留。
- `Live` 收下它：在今天的卷身份**旁边**另记一份**按清单序号**的每卷状态（等待中 · 处理中且走到哪个环节 ·
  等待确认 · 完成 · 进了隔离 · 跳过 · 没做成 · 被立即停止掉），开卷那一条按卷根认回清单里那一卷。
  今天那一份与旧界面一格不动，删它在切换那一票（15）。
- `CONTEXT.md`《事件》补一句开工那一条带着什么。

清点途中库仍一条事件都不报（Q720），这一票不加。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 事件流用例：开工那一条的卷清单与随后各卷开卷的次序一一对得上，步数与源页数与各卷报告对得上
- [x] 两张表与返回报告里那两张逐条相同；按停止停在半路的那一趟也是全的
- [x] 清点失败时仍一条事件都不发
- [x] `Live` 每卷状态的用例：一趟里跳过、隔离、没做成、立即停止掉、等待确认各至少一卷，逐卷对得上
- [x] 命令行报告字节不变，黄金快照原样过
- [x] 旧界面的用例照绿；三条闸门跑满

## 落地记录

**做了什么。** 两处接缝，各一片红-绿。

**库**：`tonefit::SurveyedVolume`（`src/survey.rs`，公开、字段全公开：`root`、`steps`、`source_pages`，与 `VolumeReport` 同一条理由——会话的夹具要按场景数据造得出）。清点数成员那一遍把源页数留下（`Surveyed::source_pages`，取自同一份 `MemberCounts`，不另数），`Survey::roster()` 把那一列摊成卷清单，`Survey::non_volume_files()` / `unreachable_places()` 借出两张表。`Event::RunStarted` **追加**三个字段：`roster: &[SurveyedVolume]`（照发现次序）、`non_volume_files: &[NonVolumeFile]`、`unreachable_places: &[UnreachablePlace]`；`volumes` 留着、恒等于 `roster.len()`。`run` 在同一处发它（`events.run_started(steps, &roster, …)`），清点失败仍在它之前返回、一条事件都不发。命令行那一路（`src/main.rs`）只读 `volumes, steps, ..`，一字没动；黄金快照原样过。

**会话**（`src/session/live.rs`，`tui` 特性外）：`VolumeState`（`CONTEXT.md`《卷状态》：Queued · Running{pass} · Deciding · Done · Isolated · Skipped · Failed · Aborted）；`Live` 多三格——`roster`、与它同序同长的 `states`、`current`（当前卷在清单上排第几）——加两张表两格（摆在报告旁边，见 Q745）。开工那一条在 `observe` 里转两个方法：原有的 `run_started(volumes, steps)`（总览那两个数，旧界面八十余处用例只喂它）与新的 `surveyed(roster, non_volume_files, unreachable_places)`（清单整份留下、每卷立成等待中）。此后逐条事件推状态：开卷按卷根 `position` 认回清单里那一卷 → 处理中；某一遍开工记环节，写出那一遍且这一趟真停下来问（`stops_to_ask`：等人的那一趟且还没答过「后面都写出」，与等人那一截同一判据）→ 等待确认；`decide` 答继续 → 处理中·写出（答做完再停不换档，收摊那一条紧跟着到）；收摊按 `skipped()` / `isolated()` 分成跳过 / 进了隔离 / 完成；没做成那一条 → 没做成；结束那一条时还开着的 → 被立即停止掉（拒绝开始撞在半路也是这一副形状，Q746）。今天的卷身份（`Volume::Settled/Failed/Summarized`）与旧界面一格不动。访问器 `roster()` / `states()` / `non_volume_files()` / `unreachable_places()` 眼下只有本模块用例读，各挂 `cfg_attr(not(test), expect(dead_code))`——画树那一票接上读者时拆掉。

**用例**：`tests/events.rs` 三条新的（清单次序／步数／源页数三卷页数各异地对上开卷事件与报告；两张表整份 `Debug` 比对，三类非漫画文件加一个关上门的目录；两级停止各停在第一卷之后清单与表仍是全的）加既有拒绝那一条多断言清单没报；`live.rs` 三条新的（六卷一趟跳过／等待确认→完成／隔离／没做成／等待确认→立即停止／等待中逐卷对上；按卷根认回、「后面都写出」之后不是等待确认；两张表清点一到就拿得到而报告上仍空）。关门那一手挪进 `tests/fixtures/mod.rs`（`shut_the_door` / `open_the_door`），`tests/exit_code.rs` 改用它。

**`CONTEXT.md`**：《事件》补一句开工那一条带着什么；《进度》新增**清点摘要 (SurveyedVolume)**（含**卷清单 (Roster)**）；《会话》新增**卷状态 (VolumeState)**。已有词条含义未改。

**review 之后改的**：卷清单三个词（`surveyed`／`listed`／`roster`）统一成 `roster` 并补英文名；两张表从「当场进报告」改成「摆在报告旁边」（Q745）；`decide` 只在答继续时换档；`stops_to_ask` 一处出处；`at` 改名 `current`；关门那一手去重；`assert_same_table` 一处；停半路那条两级各问一遍；`allow(dead_code)` 改成 `cfg_attr(not(test), expect(dead_code))`；`Event` 文档「两次」改「三次」。

**数。** `cargo xtask gate` 三条（`.tmp/gate.log`）：

- 闸门 1 · 默认构建：`cargo test`，合计 **986** 通过 0 失败（lib 238 / bin 407）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- 闸门 2 · 甩掉终端库：`cargo test --no-default-features`，合计 **834** 通过 0 失败（lib 238 / bin 255）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- 闸门 3 · 开着量具：`cargo check --features profiling`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.49s`

上一趟（02 收尾）是 980 / 828：三条事件流用例（两条闸门都跑）加三条 `live` 用例（两条闸门都编也都跑），各多六。黄金快照 `tests/golden-snapshot.txt` 原样过，一个字节没动。

`cargo xtask polish`（`.tmp/polish.log`）：四条全绿（`EXIT=0`）：fmt 过；clippy 两趟 `Finished`；`cargo doc --no-deps` 仍是 **15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 02 同数——`SurveyedVolume` 的文档一度链到私有的 `Survey::roster` 成了第 16 条，改成不链）。

### 停车场结转

Q744–Q748（`.scratch/非阻塞问题.md`《待处理》）。
