# 04 — 中止：页边界检查点与丢弃未完成的容器

**What to build:** 按停的第二级。收尾要等当前卷跑完（02 已经给了）；**中止**立刻停，
当前卷那格 `partial` 直接丢弃——那一卷等于没做，最终位置上一个字节都没动过。

两种容器形态现在同形：归档卷的 `partial` 是文件，目录卷的是目录，改名成功才算数。
中止因此不必去最终位置删任何东西，只需丢掉**它自己建的**那一格。

依据：ADR 0013（两级停——收尾与中止）。

**Blocked by:** 02 — 事件流。

**Status:** resolved

- [x] 观察者在页边界答中止 → 处理当场停下，不等当前卷跑完
- [x] 当前卷那格 `partial` 被丢弃，最终位置上没有它这一卷这一趟的任何痕迹
- [x] 别的卷已经改名成功的输出一个不动
- [x] 上一趟留下的过期副本不被中止碰到——中止只丢它自己建的那一格
- [x] 中止之后重跑，被中止的那一卷整卷重做，幂等不把它误判为做完了
- [x] 进程正常退出时 `partial` 已经清掉；异常死亡留下的残留，下一趟能识别并清理
- [x] 卷边界与页边界各有一个检查点，两者行为分得开

## 落地记录

页边界那个检查点是**一个谓词**：`progress::Events::aborting()`，只认中止这一级
（收尾停在卷边界，页边界不抢它的活）。`process_volume` 把它摆在**每一个逐成员的循环头上**——
幂等这一道、第一遍发页、第二遍写页、第二遍搬透传文件。摆在哪几处只在
`process_volume` 的《中止：回 `None`》那一节数得清，别处一律不复述那个数目。

### 停下来之后不必往上传「我是被中止的」

闩只升不降（02 号票的 `Standing`，`fetch_max`），因此**再问一次恒得同一个答案**。
各段于是只管 `break` / `return`，由 `process_volume` 在段与段之间自己再问一次闩收摊。
`first_pass` 那一处是 `.take_while(|_| !events.aborting())`，拦在 `par_bridge` **之前**——
停下来的不止计算层，读取层的发号闸跟着关上，那几条读取线程当场收摊
（`read::Throttle::stop`）。

### `partial` 怎么丢的

**不收尾就行。**`sink` 那一侧一个字都没改：`Sink::finish` 是最终位置唯一被碰到的一步
（先腾位置、再改名），中止那一支直接不走它，`Sink` 走出作用域，两个 `Drop`
分别 `remove_dir_all` / 放掉写入器再 `remove_file`。最终位置因此一个字节都没动过——
包括另一个去处留着的过期副本，它的去留仍由用户定（ADR 0013 的《不要做的「简化」》）。

`process_volume` 因此改回 `Result<Option<VolumeReport>>`，`None` 即「这一卷等于没做」；
`run` 里 `let Some(report) = … else { break }`。中止掉的卷不进报告、也不报
`VolumeFinished`——流上一条没有配对的开卷就是中止的样子（记在 `Event::VolumeFinished` 上）。

### 用例：断言看的是**那个目录**，不是返回值

`tests/events.rs` 新增 7 条，`tests/container.rs` 新增 1 条。观察者
`StopsAtAPageBoundary` 点名「哪个卷、哪一遍、第几步」按下哪个字，并在按下的**那一刻**
看一眼输出根——那是「`partial` 确实建出来过」唯一测得到的形式，跑完再看，
「从没建过」与「建了又丢掉」分不开。

| 用例 | 钉的是 |
|---|---|
| `aborting_at_a_page_boundary_throws_the_partial_container_away` | 按下那刻盘上是 `volume-a.partial`；中止后输出根**空**；重跑整卷重做、不被跳过 |
| `aborting_in_the_fingerprint_pass_never_lets_the_passes_after_it_start` | 幂等那一道停在第几步（删掉循环头的 `break` 当场红）；后面两遍不开工 |
| `aborting_in_the_first_pass_never_lets_the_second_one_start` | 第二遍连开工都没有——「立刻停」不等写出那一遍 |
| `aborting_after_the_last_page_stops_before_the_pass_through_files` | 透传那个循环头拦得住（步数 `PAGES` 而不是 `PAGES + 1`） |
| `aborting_touches_nothing_but_the_partial_it_built_itself` | 已改名的别的卷、被中止卷在最终位置上一趟的成品、隔离目录里的过期副本，三样逐字节不动 |
| `aborting_an_archive_volume_throws_its_partial_file_away` | 归档卷那一格是**文件**，走的是另一条丢弃路径 |
| `the_two_checkpoints_stop_at_two_different_boundaries` | 同一时机只换那个字：收尾写完并改名，中止等于没做；收尾在第一遍与第二遍两处页边界上都不生效 |
| `a_partial_left_behind_by_a_hard_kill_is_cleaned_up_by_the_next_run`（container） | 硬杀留下的两格残留下一趟认得出、清得掉，且成员不混进本趟输出 |

`tests/fixtures/mod.rs` 新开 `names_in`（目录顶层有哪些名字）：`tests/container.rs`
原先那个同构的 `left_in` 一并并了过去，半成品在这个清单里看得见，而铺平的
`directory_members` 认不出它。

### 命令行：一个字节都没变

`src/main.rs` 与 `src/sink.rs` **一行未改**，`Bar::observe` 仍恒回继续——没有人按停时
一切照旧。命令行还没有按停的键，见停车场 Q38（那是 `10` 的活）。退出码一字未动（`05` 的地界）。

### 数

全量 **406 通过 0 失败**（基线 398），14 个测试二进制。`cargo fmt --check`、
`cargo clippy --all-targets` 干净，`cargo doc --no-deps` 与 HEAD 同为 14 条既有告警。
黄金快照未变、未重录。`CONTEXT.md` 的《会话》加了一行词条（**检查点**）与一句
「中止掉的那一卷不进报告」。停车场新增 Q46、Q47，并把 Q38 的 **Whose call** 改指 `10`。
