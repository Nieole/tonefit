# 05 — 清点那两张表一到就进报告，开工只一个方法收

**What to build:** 开工那一条一到，非漫画文件与无法访问的地方就写进报告；`Live` 旁边那两张删掉，屏上与退出时印到 stdout 的那一份读同一份。
`run_started` 与 `surveyed` 并成一个方法；只喂前一半的那八十余处用例改成喂整条（清单与卷数对得上）。屏上一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `Live` 用例：清点一到，报告上那两张就有，与屏上读的是同一份
- [x] `Live` 上只有一个方法收开工那一条
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 清点那两张表一到就进报告，开工那一条在 `Live` 上只由一个方法收（收停车场 Q745、Q964、Q747）。

1. **两张表进报告**（`src/session/live.rs`）：`Live` 旁边那两格 `non_volume_files`／`unreachable_places`（`Arc<Vec<…>>`）删掉；
   开工那一条一到，两张表写进攒着的那份报告。两个访问器留着，读的是报告上那一张——屏上（`view.rs` 的树、`shell/overview.rs` 的问题计数）
   与退出时印到 stdout 的（`run.rs` 的 `Running::report` → `Live::report`）从此同一份。报告那一格换成 `Arc<Report>`，
   改它的四处走 `Arc::make_mut`，画一帧之前在锁里拷 `Live` 仍只拷一个指针（Q1217）。
2. **开工一个方法收**：`run_started` 与 `surveyed` 并成 `run_started(steps, roster, non_volume_files, unreachable_places)`，
   与库里报它的那一处（`progress` 的 `Events::run_started`）同一副签名；共几卷读清单的长度，`Live::volumes` 那一格删掉（Q1218）。
   `observe` 那一支一条事件转一次。
3. **调用处改成喂整条**：`live.rs` 用例 18 处（其中 12 处原先只喂前一半）、`scene.rs` 的 `replay`、`terminal/measure.rs` 的 `big_library`。清单照各条用例开的卷摆，
   原先卷数与开的卷对不上的两条（`the_failed_pages_of_the_volume_in_flight_count_towards_now` 报两卷开三卷、
   `the_minutes_spent_deciding_are_charged_to_nobody` 报一卷开三卷）改成三卷。没有留只收前一半的便利方法。
   票面「八十余处」在 8fe64d9 上实为 21 处 `Live::run_started`（含 `observe` 那一处；同名的 `Session::run_started` 与库里的 `Events::run_started` 不相干）。
4. **夹具跟着收**：场景夹具收场那一段不再替库那一份补两张表；`row_tails_and_notes_of_the_running_scene_are_on_the_grid`
   不再拼一份「带表的报告」，直接 `render::tail(&live.report())`。

**用户看得见的变化**：屏上一格不变（设计快照、交互期望屏全绿）。只有一处 stdout 变了：**开工之后才没做成的那一趟**
（覆盖项在分析环节里撞上的拒绝、线程恐慌）退出时印的那份攒到一半的报告，从此带着非漫画文件与无法访问那两小结——
从前那两张在报告上是空的。这正是 spec 用户故事 21「屏上与退出时印到 stdout 的那一份读的是同一份」；
做成了的那一趟库交回的报告本来就带着它们，一个字节不变。

**用例**：`the_two_tables_are_at_hand_the_moment_the_survey_arrives` 改名 `the_two_tables_are_on_the_report_the_moment_the_survey_arrives`，
断言翻过来：清点一到报告上那两张就有、与两个访问器读到的逐条相同；这一趟随后没做成，`Live::report` 上两张表照在（stdout 印的那一份）。
库类型不带 `PartialEq`，逐条比路径与那一句为什么。

**按反跑过的**（`docs/agents/testing.md`）：

- 动手前新断言红在「报告上那两张表要等跑完才填上」（`([], [])`）。
- 落地之后把 `run_started` 里写进报告那两行拿掉：会话那一遍 **55 条红**（本条、场景几条、设计快照几景、`terminal::redesign` 一大批）——
  屏上读的就是报告那一张，拿掉它屏上也空了，没有第二份兜着。
- `returned(Err)` 那一支清掉一张表：本条红在「没做成那一趟印出去的那一份丢了两张表」。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 8fe64d9`。Spec 轴：缺失无、范围蔓延无；Standards 轴：硬违规无。

**收下的**：

- 开工之后才没做成的那一趟 stdout 多了两小结，没记、没钉（Spec）——记在上面《用户看得见的变化》，本条用例补一段钉住，按反跑过。
- Q1218 写的 `Observer::run_started` 在 `progress.rs` 里叫 `Events::run_started`（Spec，稳定引用）——改了；`run_started` 的文档原先说与
  `tonefit::Event::RunStarted`「同一副形状」，那个事件有五格，改成指 `Events::run_started`、「同一副签名」（Standards，单一出处）。
- `report` 那一格把 `Arc` 的理由又写一遍、还列着改它的几处（Standards，单一出处）——缩成「理由与 `settled` 相同」，指 Q1217。
- 几句回应已删代码的话（「旁边不另摆一份」「不另记一格」）（Standards，结果）——删掉或改成当前事实。
- 「与上一个同一份出处」靠位置指代（Standards，稳定引用）——改成链接到 `non_volume_files`。
- `run_started` 文档里「表从这一刻掐起」与函数体那句注释重复——删掉文档那半句。
- Q1217、Q1218 的 Why it did not block 没说差在停线哪一条（Standards）——补上。

**驳回的**：

- `steps` 加清单加两张表四样一起走（Data Clumps）：形状照库那一侧的事件与 `Events::run_started`，事件受 ADR 0011 约束，会话这一侧另捆一个类型只多一层转手。
- 两个访问器只是转手（Middle Man）：留着是为了不为一张表拼整份报告（`Live::report` 要逐卷拷一遍）。
- 用例里总步数与每卷步数分两处写（`3000` 配 `fixture::roster` 的每卷 `1000`）：沿用本文件既有写法，`fixture::roster` 的步数这几条用例不读。

### 停车场

本票用了 Q1217–Q1218：

- **Q1217**：报告那一格换成 `Arc<Report>`，两张表直接写进去而不是另摆或每帧深拷。
- **Q1218**：开工那一个方法不收卷数，共几卷读清单的长度。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-05.gate1.log`、`os-05.gate2.log`、`os-05.gate3.log`、`os-05.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
条数与基线相同：本票没添用例，只改了一条的名字与断言。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1104 通过 1 失败**（1 ignored）；lib 252 / bin 453；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 79.16s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **972 通过 1 失败**；lib 252 / bin 321；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 107.54s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 9.40s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 273.62 秒、闸门 2 上 356.06 秒），快照没动。
**设计快照**：`npm run check` 报「与库里那一份逐字节相同」；会话里比设计快照的那几景全绿。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q745 — 开工那一条带的两张表摆在 `Live` 旁边、跑完才进报告：报告上那两张在清点与跑完之间仍是空的

- **From:** 票 `session-redesign/03`
- **Kind:** 票面「旧界面一格不动」与「事件流就是报告的增量」相抵的一处，选了前者
- **Where:** `src/session/live.rs` 的 `Live::surveyed`（把开工那一条带的两张表收进 `Live::non_volume_files` / `Live::unreachable_places` 两格，各带访问器）与 `Live::new` 里报告那两张空表的注释；`src/session/draw/overview.rs` 的出事行读 `report.unreachable_places.len()`（旧界面）
- **Why it did not block:** 当场进报告的话，真跑一趟时旧界面的出事行从清点起就多一句「发现无法访问 N 处」（从前要到跑完），而票面写明旧界面在切换那一票之前一格不动（review 指出）；摆在旁边，旧界面逐字不变，新界面的分区备注行从访问器读，一样在第一卷开工之前就画得出
- **What this ticket actually did:** 头一版当场进了报告，review 之后改成摆在旁边；`the_two_tables_are_at_hand_the_moment_the_survey_arrives` 钉着「报告上那两张跑完之前仍是空的」
- **Options:** ① 照现状，15 号票删旧界面时把两格并回报告（那时「事件流就是报告的增量」在这两张表上也成立，那条用例改成反着断言）；② 现在就并回报告，接受旧界面那一行提前出现
- **Recommend:** ①
- **Whose call:** 15 号票的实现者
- **处置：** **`one-source/05` 落地（2026-10-02）：照票面了结。**开工那一条一到两张表就写进报告，`Live` 旁边那两格删掉；屏上与 stdout 读同一份（报告那一格成 `Arc<Report>`，Q1217）。那条用例改名 `the_two_tables_are_on_the_report_the_moment_the_survey_arrives`、断言反过来。

#### Q964 — 停车场 Q745 那条「报告上那两张表等跑完才进」的理由随旧界面没了，行为还在

- **From:** 票 `session-redesign/15`
- **Kind:** 决定的依据过期
- **Where:** `src/session/live.rs` 的 `non_volume_files` / `unreachable_places` 两格文档，与用例
  `the_two_tables_are_at_hand_the_moment_the_survey_arrives`；Q745 的理由是「旧界面的出事行读的是报告上那一张，
  当场进报告会让它从清点起就多一句」
- **Why it did not block:** 屏上读的是 `Live` 旁边那两张（清点一到就有），报告上那两张跟着 `returned` 到，
  逐条相同；退出时印到 stdout 的那一份读报告，那时这一趟已经收场。两条路今天给的字一样。
- **What this ticket actually did:** 行为一格没动；只把文档与断言消息里「旧界面的出事行」那半句换成指向 Q745 与本条。
- **Options:** ① 维持：报告只在 `returned` 时换上那两张；② 清点一到就写进报告，`Live` 旁边那两张删掉，
  屏上与 stdout 读同一份
- **Recommend:** ②。两份逐条相同的表摆在一起，唯一的理由已经不在了；合成一份少一处「两份会不会走散」。
- **Whose call:** 下一张碰 `Live` 的票
- **处置：** **`one-source/05` 落地（2026-10-02）：照②了结**，见 Q745 的处置。

#### Q747 — 开工那一条在 `Live` 上分成两半：`run_started(volumes, steps)` 与 `surveyed(清单, 两张表)` 两个方法

- **From:** 票 `session-redesign/03`
- **Kind:** 走了哪条路（一条事件、两个接收方法）
- **Where:** `src/session/live.rs` 的 `Live::observe` 开工那一支、`Live::run_started`、`Live::surveyed`；旧界面与状态机的用例里 `live.run_started(n, steps)` 八十余处，只喂前一半
- **Why it did not block:** 改成一个方法就要改那八十余处、而且它们喂的清单会是空的（与卷数对不上）；05 号票的夹具两半都喂，`observe` 那一支两半一起转
- **What this ticket actually did:** 两个方法，`observe` 里一条事件转两次，文档写明为什么分
- **Options:** ① 照现状，15 号票删旧界面时顺手并成一个方法；② 现在就并，八十余处用例加 `&[], &[], &[]`
- **Recommend:** ①
- **Whose call:** 15 号票的实现者
- **处置：** **`one-source/05` 落地（2026-10-02）：照票面了结。**并成 `Live::run_started(steps, roster, non_volume_files, unreachable_places)`，不收卷数（Q1218）；只喂前一半的用例改成喂整条，清单照各条开的卷摆，不是一律空清单。
