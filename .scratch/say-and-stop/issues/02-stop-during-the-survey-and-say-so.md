# 02 — 停得下、说得出：收场那一行与清点中那一问

**What to build:** **清点中按得停**：事件多一种「清点中」。清点每开一个归档头之前发一条，**不带卷数**（Q720：清点中只说正在清点）；
观察者照常回指令。答了做完再停或立即停止，清点在这一个归档之后收手：这一趟不发开工那一条，`run` 交回一份一卷都没有的报告，
收场是按停止停下。目录卷那一侧不开归档头，照旧不发。命令行那条横条收到它什么都不画；会话收到它只答话，清点那一屏一格不变。

**命令行报告说得出收场**：`Report` 身上本来就带着这一趟怎么收的场，命令行报告末尾照它印一行，措辞只在措辞那一层写一次——
走到头不印；做完再停说剩下几卷没开工；立即停止说当前那一卷丢掉了、最终位置没动过；清点途中停下说一卷都没开工。

**Blocked by:** 01

**Status:** resolved

- [x] 事件流用例：清点每开一个归档头之前一条清点中，不带卷数；观察者答停止时不发开工那一条，交回的报告一卷都没有、收场是按停止停下
- [x] 目录卷那一侧不发清点中；会话清点那一屏的设计快照照旧绿
- [x] `render` 的用例：三种收场各一句、走到头不出
- [x] 起真进程（`tests/stop.rs` 那一套）：命令行按停之后 stdout 上的报告有收场那一句；清点途中按停当场停，报告说一卷都没开工
- [x] `CONTEXT.md`《事件》添清点中那一种
- [x] 黄金快照原样过（正常跑完不印收场）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。**

1. **库：事件多一种清点中**（`src/progress.rs`）：`Event::Surveying {}`，不带字段（Q720），两级非穷尽照旧；`Events::surveying`
   报它，`Events::stopping` 问「按过停止没有」（卷边界那个检查点与清点那一问共用，原先那一处 `standing() != Continue` 换成它）。
2. **库：清点问话**（`src/survey.rs`）：`Survey::of(request, events) -> Result<Option<Survey>>`。枚举每一个**归档**候选之前报一条清点中、
   再问闩；按过停止就当场收手，这一个归档头不开（Q1139），交回 `Ok(None)`，连同攒到一半的点名坏路径一起丢掉——停止赢（Q1140）。
   目录候选不报也不问。模块文档添《清点中：每开一个归档头之前问一句》。
3. **库：`run`**（`src/lib.rs`）：`None` 那一支开工、结束两条都不发（Q1142），报告一卷都没有、两张表空着（Q1141）、结束方式
   `RunOutcome::of(闩)`。逐卷那一段照旧，只多记两格：卷边界上停下时 `unstarted = 1 + 还排着的`，页边界上立即停止时
   `aborted = 那一卷的卷根`、`unstarted = 还排着的`。
4. **库：`Report` 多两格**（`src/report.rs`，Q1138）：`aborted: Option<PathBuf>`（被立即停止掉的那一卷，卷边界上的立即停止是 `None`）、
   `unstarted: Option<usize>`（一卷都没开工的有几卷，走到头 `Some(0)`，清点途中停下 `None`）。`RunOutcome::Stopped`／`of`、
   `failed_volumes` 的文档跟着改成真话。
5. **措辞**（`src/render.rs` 的 `outcome`，Q1137）：旧的 `render::outcome` 早随旧会话删掉，新写一份，不挂特性。走到头 `None`；
   清点途中「清点途中按停止停下：一卷都没开工」；做完再停「按停止停下（做完再停）：剩下 N 卷没开工」；立即停止「按停止停下（立即停止）：
   <卷> 做到一半丢掉了，最终位置上一个字节都没动过；剩下 N 卷没开工」——没丢卷、剩零卷各省那半句。**排版**：`plain::report` 末尾接这一行，
   命令行与会话退出时 stdout 那一份同一处出（Q1146）。会话屏上照设计稿，不读它。
6. **命令行**（`src/main.rs`）：`Bar` 收到清点中走 `_` 那一支、什么都不画、照闩答话；`execute` 在印报告之前放掉 `request`，
   `Bar` 的 `Drop` 收掉清点那条转轮（清点途中停下的那一趟没有结束那一条，Q1142）。几处说「清点那一段一条事件都没有」的注释改成真话。
7. **会话**：一行代码没动（`Live::observe` 的 `_ => {}`、`Watch` 照 `stop::answer` 答）。`look.rs`、`view.rs`、`shell/overview.rs`
   各改一两行注释（同一句假话）。`live.rs` 攒到一半那份 `Report` 补两格。清点中按停止之后的结束屏设计稿里没有（Q1143）。
8. **`CONTEXT.md`**：《事件》添清点中；《进度》下面那段引文「库这一侧还没有话说」改成只问一句清点中。《总览》的括注落地途中改过、
   评审之后**回退**，与《结束方式》《清点摘要》两处一并记 Q1144。
9. **用例**：
   - `tests/events.rs` 新添三条：`the_survey_asks_before_each_archive_header_and_never_for_a_directory`（归档、目录、归档夹着摆：
     两条清点中都在开工之前；只有目录卷的一趟一条都没有）、`stopping_while_surveying_starts_nothing_and_returns_an_empty_report`
     （两级各一遍：第二个归档头之前答停止，流上只有两条清点中，开工、结束、清单都没有，报告空、`unstarted` 是 `None`、输出目录空）、
     `a_stop_while_surveying_wins_over_a_refusal_it_had_not_finished_collecting`。`Recorder` 认清点中、多一个
     `stopping_while_surveying`。改了两条：`a_path_that_cannot_be_opened_refuses_the_whole_run_before_any_volume_event` 从「一条事件都没有」
     改成「只有两条清点中」；`the_last_event_says_how_the_run_ended` 与 `aborting_at_a_page_boundary_throws_the_partial_container_away`
     补 `unstarted`／`aborted` 的断言（前者那趟立即停止答在卷边界上，一卷没丢——旧注释说「当前那一卷也丢掉」是错的，改了）。
   - `src/progress.rs` 那条「每一条都到得了观察者」多比一条，整份 Debug 就是 `Surveying`——不带卷数钉在这里。
   - `src/survey.rs` 的用例经一个「没人可问」的 `survey_of` 走。
   - `src/render.rs` 新添 `the_report_ends_with_how_a_stopped_run_stopped_and_says_nothing_when_it_ran_to_the_end`：六种停法逐字、
     走到头 `None` 且报告里没有「按停止」、命令行报告以那一句收尾、没丢卷的那一句不许出现「丢掉」（反着钉）。
   - `tests/stop.rs`：两条旧用例改读 stdout，各断言那一句（比之前去掉空白，报告按 100 格折过）；新添
     `ctrl_c_while_surveying_stops_there_and_the_report_says_no_volume_started`，记号是 40 个点名的 `.cbz` 命名管道（Q1145）。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `Survey::of` 不问闩（`&& false`）：`stopping_while_surveying…` 与 `a_stop_while_surveying_wins…` 都红，流上是
  `["Surveying", "Surveying", "Surveying", "RunStarted", "RunFinished"]`；进程级那条红在退出码 1（管道放完、清点走到头被拒）。
- `plain::report` 不接那一句：`render` 那条红在「命令行报告没以结束方式那一句收尾」。
- 进程级那条的头一版（写端开完就关）连跑十趟卡死一趟：XNU 上读端醒来再看一眼没有写端就接着睡。改成攥到下一个放行为止、
  加一分钟上限之后连跑二十趟全绿，每趟约 0.7 秒。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 9af0af7`。

**收下的**：

- Standards：五处文档说了假话——`RunOutcome::Stopped`（立即停止不一定丢卷、痕迹在 `aborted`）、`RunOutcome::of`（不止两条 `break`）、
  `failed_volumes`（停止赢之后那一句不总成立）、`aborted`（`partial` 不一定建过）、`tests/events.rs` 引的那一句与真印的不一样——都改了。
- Standards：新文字里用了 `CONTEXT.md` 标为旧称的「收场」——改成「结束方式」。
- Standards／Spec：《总览》括注改了已有词条——回退，记 Q1144（连同 Spec 指出的《结束方式》《清点摘要》两处）。
- Standards：`standing() != Continue` 两处各写一遍——添 `Events::stopping`。
- Standards：`run` 里那段说明与 `Event::Surveying` 重了——留一处，`run` 里只指路；`look.rs` 指向的小节与 `view.rs` 对齐成《事件》。
- Standards：用例里 5 毫秒 × 40 没有出处——`HOLES`／`BREATHER` 的文档写出余地的量级与依据。
- Spec：会话退出时 stdout 那一份也以那一句收尾、而用户按的是退出——记 Q1146；进程级用例押在 Q1140 上——写进 Q1140。

**驳回的**（记停车场，不改）：

- Standards：`aborted`／`unstarted` 合成一个小枚举——Q1138 的选项④，推荐等下一格加进来时一起做。
- Standards：`run` 嵌深、抽一个函数；`Instruction` 映成两级叫法的那张表在 `pressed_note` 另有一份——前者是判断题，
  后者 `pressed_note` 的文档写着它为什么在进度显示那一层。
- Spec：收手时机（Q1139）、立即停止那一句的说法（Q1137）、不报结束（Q1142）——各有条目，等拍板。

### 停车场

本票用了 Q1137–Q1146 全部十个：Q1137（措辞新写）、Q1138（`Report` 两格）、Q1139（不开这一个归档头）、Q1140（停止赢过拒绝）、
Q1141（两张表空着）、Q1142（不报结束、`main` 放掉观察者）、Q1143（会话结束屏设计稿没有）、Q1144（`CONTEXT.md` 三处）、
Q1145（命名管道当记号）、Q1146（会话退出那一份）。

结转两条（见《停车场结转》）：Q260、Q261，本票照上面的做法了结。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后；日志 `ss-02.gate1.log`、`ss-02.gate2.log`、`ss-02.gate3.log`、`ss-02.polish.log`，都在树外）。
这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1065 通过 1 失败**；lib 239 / bin 437（另 1 条 `ignore` 的量具）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.08s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **947 通过 1 失败**；lib 239 / bin 319；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.46s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行「全绿。」 |

**本票净添 5 条用例**，两条闸门各多 5 条：bin +1（`render` 那一条）、`tests/events.rs` +3（30 → 33）、`tests/stop.rs` +1（2 → 3）；
lib 没多（`progress` 那一条是改，`survey` 那四条只换了入口）。

**`tests/stop.rs` 在两条闸门里都是 3 条全过**（闸门 1 上 14.85 秒、闸门 2 上 17.04 秒）；清点途中那一条另外单独连跑二十趟全绿。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 145.26 秒、闸门 2 上 145.55 秒）；
`tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
设计快照：`npm run check`「与库里那一份逐字节相同」，设计稿与 `tests/fixtures/design/` 一个字节没动，闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`，末行「全绿。」）：`cargo fmt --check` 绿；`cargo clippy --all-targets` 绿；
`cargo clippy --all-targets --no-default-features` 绿；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q260 — 命令行按停停下来的那一趟，**报告上说不出它被按停过**

- **From:** 票 `p4-parking-lot/18`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/render.rs` 的 `outcome`（整个挂在 `#[cfg(feature = "tui")]` 上）；
  `src/main.rs` 里印报告那一句（`render::plain::report` 那四段）
- **Why it did not block:** 本票的验收第 4 条要的是「按下之后屏上说得出按到了哪一级」，
  而那一句由信号那条线程当场说（`install_the_stop_key`，印在进度条上面），这一条因此成立。
  但那一句走 **stderr**，而且对面不是终端时 indicatif 一个字节都不写；报告走 stdout，
  `tonefit … > 报告.txt` 那一份于是看不出这一趟是被按停的——只看得出卷比点名的少了几个。
  措辞其实早就写好了（`render::outcome`：「按停（收尾）：当前卷跑完就停了……」），
  只是挂在 `tui` 后面，会话是它眼下唯一的读者，而那正是它文档里写着的话。
- **What this ticket actually did:** **没有动 `render.rs`。**本票的派活说明把可动的文件列死了
  （`src/main.rs`、`src/progress.rs`、`Cargo.toml`、`docs/adr/0013-*.md` 与相关用例），
  `render.rs` 不在其中；而把 `outcome` 从 `tui` 后面搬出来、再往命令行那四段报告里加一格，
  变的是**报告的形状**，不是接一个键。屏上那一句因此写在 `main.rs` 里，
  与进度条上那几句（「点名 N 个路径……」「整趟 N 卷」）一个待遇——理由只写在
  `pressed_note` 的文档上，这里不复述。
- **Whose call:** 拍板的人（命令行那份报告要不要多一格「收场」）
- **处置：** 待处理。

#### Q261 — 预扫那一段按下去**停不下来**：闩记住了，而第一个检查点在它之后

- **From:** 票 `p4-parking-lot/18`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/lib.rs` 的 `run`：`survey::Survey::of` 排在逐卷循环**之前**，
  而卷边界那个检查点在循环头上
- **Why it did not block:** 那一下**不会丢，屏上也说得出**——键装在 `run` 之前，
  按下的那一级当场记进闩，屏上那一句由 `ctrlc` 自己那条线程说（不等事件，见
  `install_the_stop_key`）；逐卷循环头上那个检查点随后看见它，第一卷因此一个字节都不写。
  差的是**停下来的时机**：几十个归档卷在慢盘上列归档头要一阵，那一段里进程停不下来，
  按下中止也一样——要等预扫整个走完。
- **What this ticket actually did:** 装在 `run` **之前**而不是之后（那一下因此按得进去、不会丢），
  屏上那一句改由信号那条线程当场说（原本挂在 `Bar::observe` 上，而预扫那一段一条事件都没有——
  评审当场指出那时屏上一声不吭）。**预扫自己那一段没有加检查点**，加它要动库里的 `survey`，
  越了本票可动的文件。这与 `p4-parking-lot/13`（摊开一整卷途中的检查点）是同一类东西：
  一段跑得久、而中间一次都不问的路。
- **Whose call:** 拍板的人（预扫要不要也有一个页边界那一级的检查点）
- **处置：** 待处理。
