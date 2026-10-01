# 07 — 没做成的卷带卷级计时

**What to build:** `VolumeFailure` 带上卷级计时，「没做成」那一条事件跟着带。会话对没做成的卷改读它；自己量的那一份只留给
还在跑的卷与被立即停止掉的卷——那两种没有库那一份可读。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 事件流用例：没做成的卷那一条带着卷级计时，走过的那几段不为零
- [x] 会话里没做成的卷，耗时那一列与目录行那个和读的是库那一份；会话自己量的那一份只剩还在跑与被立即停止掉的两种
- [x] 会话的设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。**

1. **库：`VolumeFailure` 多一格 `timing: VolumeTiming`**（`src/report.rs`）。段的分法与卷报告那一份相同：走过的环节各有一段，
   坏在哪个环节里那一段掐到坏的那一刻，没走到的是零；总数照样扣等人那一截、加回清点枚举那一截。`VolumeTiming` 的文档
   添一段「没做成的卷也有一份」，各段「什么时候是零」那几句与 `elapsed` 的起止跟着改口；`outside_the_segments` 添一样：
   没做成的卷上总数读在放掉这一卷之后，摊开的卷收走临时目录那一截落在段外。
2. **库：`Event::VolumeFailed` 多一格 `timing`**（`src/progress.rs`），与报告那一条同一份。变体本来就非穷尽，命令行横条一个字没动。
   取舍见 Q1197。
3. **库：卷的表收成 `Stopwatch`**（`src/lib.rs`）。`run` 每卷开一只、交给 `process_volume` 去掐；从前的 `wall_clock` 闭包与
   `timing` 局部量收进它，三处读总数换成 `stopwatch.read()`；卷转换失败那一支读它、交给事件与报告。取舍与代价见 Q1198。
4. **会话：一卷做了多久只在 `Live::elapsed_at` 一处定先后**（`src/session/live.rs`）：报告 → 没做成那一条 → 还在跑（开卷到此刻）
   → 被立即停止掉（`aborted_elapsed`）。逐卷一格的 `timings` 换成那一格，`finish_volume` 不再量；卷列表的 `elapsed_of`
   （`src/session/shell/list.rs`）只剩转手。取舍见 Q1199。
5. **场景夹具**（`src/session/scene.rs`）：没做成的卷不再推「此刻」，计时照场景数据交进 `volume_failed`；自检多核一句
   没做成那一条的计时等于场景数据的秒数。设计稿、`tests/fixtures/design/` 一个字节没动。
6. **命令行报告不印它**：命令行报告本来就不印卷级计时，黄金快照不动。`CONTEXT.md` 没改（spec《词条随票落地》），漏说的那一种记 Q1200。
7. **用例**：`tests/events.rs` 两条——坏在写出环节末一步（去处被占）的目录卷，查重、分析、写出三段不为零、摊开为零；
   坏在摊开的 `.7z`，摊开不为零、其余为零。哪几段该有数读的是流上报过开工的环节；两条都比事件与报告那一条是同一份。
   既有的 `a_volume_that_never_got_done_is_reported_the_moment_it_fails` 比的三样里添上计时。`src/progress.rs` 的转手用例带一份
   点名的计时整份比。`src/session/live.rs` 一条——收摊、没做成、在跑、被立即停止掉、没轮到五种卷，「此刻」与库报的数故意错开。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `run` 交给事件与报告的计时换成全零：两条新用例红在「Fingerprint／Extraction 开了工……却是零」。
- 只有报告那一条换成全零、事件照旧：两条新用例与既有那一条红在「不是同一份」「对不上」。
- `elapsed_at` 不读没做成那一条：会话 24 条红——会话那一条红在「没做成的卷读的不是库那一份」，`running`、`ended`
  两屏的设计快照与 `terminal::redesign` 那一批串一起红（场景夹具不再推「此刻」，没做成那一卷屏上只剩零）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 891d436`。

**收下的**：

- Standards：`VolumeTiming` 各段文档对没做成的卷不成立（「那时才是零」「不许是零」「到这份卷报告成型」）——见上第 1 条；
  收走临时目录那一截原先只写在 `Stopwatch` 上，挪进 `outside_the_segments` 一处，`Stopwatch` 与 `VolumeFailure::timing` 指过去。
- Spec：`VolumeFailure::timing` 说「与卷报告同一个口径」说满了——改成「段的分法相同」，差的那一截指过去。
- Standards：`progress.rs`「要么交出这一句原因」没跟上；「只有库那一侧减得掉等人那一截」「表放在 `run` 手上」各说了几遍——收到一处、别处指过去。
- Standards：`Live::aborted` 与 `Report::aborted` 同名异义——改叫 `aborted_elapsed`。
- Standards：会话那一条里收摊那一卷的耗时借夹具的值——改成点名 72 秒。
- Spec：场景自检只核没做成那一卷的原因——添一句核计时。

**驳回的**：

- `render.rs` 九处 `timing: VolumeTiming::default()`（Shotgun Surgery）：那几条用例不读计时，同一个文件里已有十处同样写法。
- 三样一起穿 `volume_failed`（Data Clumps）：记在 Q1197。`Stopwatch` 自带一份 `events`、`process_volume` 另收一份——`Events` 是 `Copy`，
  让 `process_volume` 从表上取事件端是把两件事拧到一起。
- `elapsed_of` 只剩转手（Middle Man）：它还兜着「这一帧没有那一趟」，卷列表别处都读它，留着。
- 拒绝开始撞在半路的那一卷，会话也记成被立即停止掉、读自己量的：既有行为，卷状态归《卷状态》，本票没碰。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后，依次 polish → 闸门 3 → 闸门 1 → 闸门 2；日志 `ss-07.polish.log`、
`ss-07.gate3.log`、`ss-07.gate1.log`、`ss-07.gate2.log`，都在树外）。这台机器是 macOS，闸门 1、2 在基线上就各红一条：
`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1094 通过 1 失败**；lib 252 / bin 443；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.92s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **972 通过 1 失败**；lib 252 / bin 321；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 64.60s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 5.83s` |

**本票添 3 条用例**（`tests/events.rs` 2 条：33 → 35；bin 里 `session::live` 1 条，`live.rs` 在特性外面），两条闸门各多 3 条；
lib 的 252 条本票只改了一条（`progress` 的转手用例带上计时），没添。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 152.77 秒、闸门 2 上 174.35 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
设计快照：`npm run check`「与库里那一份逐字节相同」，设计稿与 `tests/fixtures/design/` 一个字节没动，闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` 绿、零告警；
`cargo clippy --all-targets --no-default-features` 绿、零告警；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q808 — 没做成的卷、还在跑的卷与被立即停止掉的卷**没有卷级计时**，屏上那一列只好由会话自己量

- **From:** 票 `session-redesign/08`
- **Kind:** 票面没想到的第三种情形（「跳过的卷耗时照给」想到了，「没做成的卷耗时照给」没想到）
- **Where:** `src/report.rs` 的 `VolumeFailure`（只有 `volume` 与 `reason`，没有 `VolumeTiming`）；`tests/fixtures/design/snapshots/running.120x36.*` 第 22 行（`✗ 第11卷 … 1s 压缩包损坏`）与第 10 行（`集英社/海贼王` 那一枝的 `1m34s` **含**这 1 秒）；`src/session/live.rs` 新加的 `timings`／`elapsed_at`
- **Why it did not block:** 屏上那一列与目录行那个和都要它，少了它目录行那个数当场就差一秒——不是一格空白，是一个**错的数**
- **What this ticket actually did:** `Live` 顺带记一份逐卷计时（`timings`，与清单同序），**只在那一卷没有报告时才读**；收摊了的卷照旧走报告那一份（`VolumeTiming::elapsed`，只有库那一侧减得掉在确认点上等人的那一截）。场景夹具（`scene::replay`）为没有报告的那几卷把「此刻」推到它开卷与收手那两刻，量出来的正是场景数据说的那个数
- **Options:** ① 照现状：两份出处，读的时候有报告走报告 ② `VolumeFailure` 加一格 `VolumeTiming`（库那一侧改，`Event::VolumeFailed` 跟着带），会话这一份删掉 ③ 屏上那一列对这几种卷留空——那会让目录行那个和悄悄少一截
- **Recommend:** ②，单开一张小票；在那之前①站得住（读的先后写在 `Live::elapsed_at` 的文档里）
- **Whose call:** 拍板的人（②动的是库那一侧的公开类型）
- **处置：** **`say-and-stop/07` 落地（2026-10-01）：照 ② 收。**`VolumeFailure` 加一格 `timing`，`Event::VolumeFailed` 跟着带
  （同一份）；会话对没做成的卷读它，自己量的那一份只剩还在跑与被立即停止掉的两种（`Live::aborted_elapsed` 一格），读的先后
  只在 `Live::elapsed_at` 一处。场景夹具不再为没做成的卷推「此刻」，`running.*` 那两屏的 `✗ 第11卷 … 1s` 与目录行的 `1m34s`
  照旧绿、而且只读得到库那一份。见本票《落地记录》与 Q1197–Q1200。
