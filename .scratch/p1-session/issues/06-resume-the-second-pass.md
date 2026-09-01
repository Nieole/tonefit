# 06 — 续做：单卷试算接着做第二遍

**What to build:** 主入口在每一卷的「汇总之后、第二遍之前」向观察者**要一次指令**。
答继续就往下做，答收尾就停在这儿——那就是 dry-run 的效果。

处理范围是**单卷**时，试算留下参照，答继续时**第一遍不重算**：
贵的那一遍是解码、缩放、算判据，第二遍只是量化与编码。
「试算满意就执行」是会话存在的理由，它必须便宜。

多卷不续做。理由是内存不是口味：缓存逐卷建、逐卷丢，峰值随单卷走（ADR 0005），
全量试算要续做就得同时押住**全部卷**的参照，而缓存预算是**每卷**的。

**等不等人是调用方的策略，不是库的行为**——库永远在那个点上问。

依据：ADR 0012（试算与执行之间续做）。

**Blocked by:** 02 — 事件流（决策点用的是同一条指令回路）。

**Status:** resolved

- [x] 决策点在汇总之后、第二遍之前，每一卷各问一次
- [x] 单卷 + 答继续 → 第一遍**只跑一次**（解码计数或第一遍事件计数为证）
- [x] 单卷 + 答收尾 → 与 dry-run 等价：输出根一个文件都没有，报告照出
- [x] 多卷不续做；缓存寿命仍是**一次运行**，不跨调用
- [x] 命令行的 `--dry-run` 不变：不留参照、不建溢写文件
- [x] 单卷试算那条路上「不写**输出**」成立（溢写临时文件可以有，运行结束即收走）

## 落地记录

决策点是**一个方法**：`progress::Events::ask_before_the_second_pass()`。它报的还是
`02` 定下的那条 `PassStarted { pass: Second }`（事件形状一字未改），只多做两件事——
把观察者答的那个字**交回来**，把它没返回的那段时间**掐出来**。
`process_volume` 拿那个字 `match` 出三条去处，位置就是 `02` 指出来的那一句：
`summarize_volume` 之后、`timed(second_pass)` 之前。

### 决策点回的是**当场那个字**，不是闩

这是它与两个检查点唯一的差别，也是本票唯一一处需要拿主意的地方。两个问题不同：

- 闩问的是「这一趟还走不走」；
- 决策点问的是「**这一卷的第二遍**还做不做」。

拿闩来答，第一遍的页边界上按下的**收尾**会顺手把当前卷的第二遍也吃掉——而收尾的定义
正是「当前卷跑完才停」（ADR 0013 决定第 1 条），盘上会因此少一整卷。
`tests/events.rs` 的 `the_two_checkpoints_stop_at_two_different_boundaries`
（第一遍那一支）本来就钉着这件事，改成读闩它当场红。

答复照样进闩：那一卷停在决策点上之后，剩下的卷由卷边界那个检查点拦下，不必开工。

### 三条去处

`Instruction` 不非穷尽（`02` 拍的），因此 `match` 穷尽写开——多一级的那一天这里编译不过。

| 答的字 | 这一卷 | 盘上 | 报告 |
|---|---|---|---|
| 继续 | 第二遍照走，参照从缓存里取，第一遍不重算 | 照常写出并改名 | 有 |
| 收尾 | **停在这儿**，第二遍一步不走 | 输出容器连建都不建，`partial` 也没有 | **有** |
| 中止 | 等于没做（与页边界上按下它同一个待遇） | 纹丝不动 | 无（回 `None`） |

收尾与中止的分界线就在报告上：停在决策点上的那一卷**做过事**——判定、逐页结果、
缓存用量、解码计数都是真的，只有 `timing.second_pass` 是零。那正是试算要看的那份东西。

### 续做不需要「续」的机制

试算与执行是**同一次 `run`**（ADR 0012 决定第 2 条），缓存、参照、解码计数从头到尾
没出过那一次调用。答继续因此不是「恢复」什么，只是**不要停**——第一遍只走一次是
这个形状的直接后件，不是一段要维护的状态。ADR 0012 决定第 4 条那句「缓存寿命不延长」
因此不必落地成代码：没有东西要延长。

### dry-run 那一路一字未动

`writes == false` 时决策点**连报都不报**：dry-run 没有第二遍，也就没有「还做不做」可问，
在它那条路上报一个决策点出去，会话就得替一个续不了的问题想一个答案。
`Retention::Account` 与 `--dry-run` 的行为一字未改（ADR 0012 决定第 5 条）。

### 命令行：一个字节都没变

`src/main.rs` **一行未改**，`Bar::observe` 仍恒回继续——没有人在决策点作答时一切照旧。
没有观察者时决策点直接答继续，连问都不问、连表都不掐。退出码一字未动：
按停不是失败，决策点上停下来同样不是。

### Q41：等人的那段时间**谁都不算**

观察者在决策点上按设计会等人（ADR 0012 决定第 3 条），人会看着报告去泡茶。
那几分钟不属于任何一段，也不属于段外那一截——库在那段里一步都没走。
`progress::Deliberation` 是一格原子累加的纳秒（与闩同一条寿命、同一个不用锁的理由），
只在决策点这一处加；两处墙钟各自减掉它：

- `VolumeTiming::elapsed` 减这一卷那一截（开卷与拼报告时各读一次，取差）；
- `Report::elapsed` 减全部卷的和。

三段一个都没动，`accounted_for(timing) == timing.elapsed` 这条哨兵因此照旧成立。
另两条路没走：单列一段要给 `VolumeTiming` 加第四格（报告形状变了），
改写 `elapsed` 的定义是领域决定，本票不该顺手拍。

### 用例

`tests/resume.rs` 新开 7 条（续做自己的文件：`tests/events.rs` 是 ADR 0011 的地界，
决策点是 ADR 0012 的），`tests/timing.rs` 新增 1 条，`src/progress.rs` 新增 2 条。

观察者 `AtTheDecisionPoint` 点名「哪个卷的决策点」答哪个字，并在**每一个**决策点到来的
那一刻看一眼输出根——「第二遍还没开始」只有那一眼看得出来（同一个手法见
`tests/events.rs` 的 `StopsAtAPageBoundary`）。

| 用例 | 钉的是 |
|---|---|
| `every_volume_gets_one_decision_point_before_a_byte_of_it_is_written` | 三卷各走三遍、第二遍那条一卷不多不少；三个决策点看到的输出根依次是空、`[a]`、`[a, b]` |
| `finishing_at_the_decision_point_writes_nothing_and_still_reports_the_volume` | 答收尾：输出根**没被建出来**；报告有逐页结果、有卷级判定、`decodes == source_pages`、`second_pass == 0`；这一趟的收场是 `Completed`（停车场 Q53 记着的那一件） |
| `aborting_at_the_decision_point_leaves_the_volume_out_of_the_report_entirely` | 答中止：那一卷不进报告、不报「一卷跑完」、输出根空、收场是 `Stopped(Abort)`；决策点那一眼看到空输出根即「`partial` 从没建过」 |
| `resuming_walks_the_expensive_pass_once_where_two_runs_walk_it_twice` | 答继续：第一遍一次、解码一遍、输出写全；**对照组**是 dry-run 一趟 + 照做一趟，解码两遍 |
| `finishing_at_one_volume_decision_point_leaves_the_earlier_volumes_whole_and_starts_no_more` | 多卷：前一卷完整落盘、当前卷有报告没输出、后一卷不开工、收场是 `Stopped(Finish)`；每卷缓存只报自己那两页 |
| `a_dry_run_has_no_second_pass_and_therefore_no_decision_point` | `--dry-run` 只报两遍，决策点不在那条路上；用量照旧预告得出溢写 |
| `the_trial_path_spills_over_budget_pages_and_still_writes_no_output` | 试算那条路（`Retention::Keep`，预算为零）真溢写，而输出根一个文件都没有 |
| `waiting_at_the_decision_point_is_charged_to_nobody`（timing） | 在外面掐的表**至少**比报出来的多一个 `WAITS`，两处墙钟各断言一次；段与总仍对得上 |
| `the_decision_point_answers_with_the_word_just_said_not_the_latch`（progress） | 闩已是收尾而观察者当场答继续时，决策点回继续；决策点上答的字照样进闩 |
| `only_the_wait_at_the_decision_point_is_clocked_as_deliberation`（progress） | 掐的只有决策点这一处，别处报到一纳秒都不进那一格 |

「溢写临时文件运行结束即收走」与「只记账那一遍连文件都不建」两条不在这里：
它们在 `src/cache.rs` 的 `a_spilled_cache_leaves_no_file_behind` 与
`an_accounting_only_cache_measures_everything_and_keeps_nothing`，那里是它们唯一的出处。

### review 之后改的

`/code-review` 两轴各出了几条，都收了：

- **决策点上答中止没有用例**，而它相对 HEAD 是**行为变更**——从前那个字在这里不作数，
  第二遍照样开工、`Sink::create` 先建出 `partial` 再由析构丢掉；现在那一格压根不出现。
  补 `aborting_at_the_decision_point_leaves_the_volume_out_of_the_report_entirely`，
  用决策点那一眼把「从没建过」钉住。
- **单卷答收尾的收场没人钉**。原打算不钉（那个值是 Q53 记着的、将来可能要改），
  改成钉上并写明用意：让它变的那天红的是这一行。
- **同一条理由复述了四遍**（决策点认当场那个字、不是闩）。`process_volume` 那一节
  收成三条事实加一句指路，理由只留在 `Events::ask_before_the_second_pass`。
- **`Report::elapsed` 的措辞自相矛盾**（「墙钟耗时……减去」）。两处 `elapsed` 与
  `CONTEXT.md` 的**卷级计时**一并改口成「**做了多久**：墙钟，扣掉在决策点上等人的那一截」。
  改这条词条的授权来自 Q41 的 `Whose call: 06`，且它列的三条路里只有第三条
  （改写 `elapsed` 的定义使其**含**等人时间）被标为领域决定——本票走的是第一条。
- **`Deliberation::add` 的文档写「饱和」，而 `fetch_add` 到顶回绕**。文档改口说清饱和的
  是换算那一步，累加不设防且为什么不必设防。
- **没有观察者时也照样掐表**，与「其余报到点一纳秒都不掐」那句话相抵。加了短路：
  没人可问就连表都不掐。同时把「掐的是什么」讲准——掐的是**那一次问话的全程**，
  不是「全程减去观察者自己那点开销」。
- **两个名字**：`deliberated` → `deliberated_at_open`（它是开卷时的快照，与当前累计同名易混）；
  `resume` → `walks_the_second_pass`（**续做**是 `CONTEXT.md` 的术语，
  而普通命令行那一趟并没有什么在「续」）。
- **两处同形的 `Ponders` 观察者**（跨 crate 边界共用不了）互相加了指路，并写明两处问的
  不是同一件事。

### 数

全量 **421 通过 0 失败**（基线 411），**15** 个测试二进制（多的是 `tests/resume.rs`）。
`cargo fmt --check`、`cargo clippy --all-targets` 干净，`cargo doc --no-deps` 与 HEAD
同为 14 条既有告警。黄金快照未变、未重录。`CONTEXT.md` 的《会话》加了一行词条
（**决策点**）与两段说明，《管线》的**卷级计时**改口成「这一趟做了多久：墙钟，
扣掉在决策点上等人的那一截」（授权见下面 Q41 的 `Whose call`）。
停车场了结 Q41，新增 Q52、Q53。

## 停车场结转

### Q41 — 观察者花掉的时间算进了 `VolumeTiming`，而 `06` 要在决策点上等人

- **From:** 票 `p1-session/02`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/lib.rs` 的 `process_volume`：`events.step()` 落在
  `timed(&mut timing.first_pass, …)` 与 `timed(&mut timing.second_pass, …)` 之内；
  `events.pass_started(Pass::Second)` 在 `timed` 之外，但仍在 `started.elapsed()` 之内
- **Why it did not block:** 眼下不咬人：命令行的观察者是一条 indicatif 的 `inc(1)`，
  用例里的记账本是一次 `fetch_add`，两者都在微秒级，`VolumeTiming` 报出来的数没变过
  （`tests/timing.rs` 四条全绿）。它要到 `06` 才成为问题——那张票让 `run` 在
  「汇总之后、第二遍之前」停下来等人拿主意（ADR 0012 决定第 3 条），
  人去泡茶的那几分钟会原样计进这一卷的 `elapsed`，而 `elapsed` 的定义是
  「从打开卷到这份卷报告成型」的**墙钟**（加固批 11 号票）。
  改法不止一种：把观察者里花掉的时间掐出来减掉，或者给「等人」单列一段，
  或者认下来并改写 `elapsed` 的定义——第三条是领域决定，不该由本票顺手拍。
- **What this ticket actually did:** **没有动计时。**报到点照旧落在原来那几段里
  （本票一步都没挪，只把方法调用换成事件），`tests/timing.rs` 一行未改、四条全绿。
  记在这里，等 `06` 落地时连同「等人的那段算谁的」一起判。
- **Whose call:** `06`（续做）
- **处置：** 由本票了结。**走的是第一条：掐出来减掉，谁都不算。**
  `progress::Deliberation` 是一格原子累加的纳秒，与闩同一条寿命（活在 `run` 的栈上、
  一次运行一份）、同一个不用锁的理由（观察者可能很久不返回，而这一格记的正是它等了多久）。
  **只掐决策点这一处**：等人是那里按设计要做的事，而其余报到点上观察者花掉的那点时间
  是它自己的成本，本来就该算进这一趟。
  两处墙钟各自减掉它——`VolumeTiming::elapsed` 减这一卷那一截（`process_volume`
  在开卷与拼报告时各读一次累计值、取差），`Report::elapsed` 减全部卷的和。
  三段一个都没动，`tests/timing.rs` 那条「三段加段外恰好等于 `elapsed`」的哨兵照旧成立。
  **另两条路没走**，理由各一：给「等人」单列一段要往 `VolumeTiming` 加第四格，
  报告形状跟着变；改写 `elapsed` 的定义是领域决定，本条自己就写着不该由这张票顺手拍。
  两处文档改口说明减掉了什么，`CONTEXT.md` 的**卷级计时**词条补了同一句。
  用例是 `tests/timing.rs` 的 `waiting_at_the_decision_point_is_charged_to_nobody`：
  断言不带余量，在外面掐的表**至少**比报出来的多一个观察者等掉的时长。
- **State:** settled
