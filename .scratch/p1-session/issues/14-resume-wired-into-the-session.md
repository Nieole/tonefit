# 14 — 会话：单卷续做接上决策点

**What to build:** 范围是**单卷**时，试算跑到决策点就停住，主区把报告画出来等你拿主意。
按执行就答继续——第一遍不重算，直接进第二遍。按停就答收尾——等价于 dry-run。

范围是多卷时不在决策点等人：试算另走一次 dry-run，屏上说清这一趟**不续做**，
免得用户误以为批量跑也是免费的。

**Blocked by:** 06 — 续做（库侧）；09 — 会话主区。

**Status:** resolved

- [x] 单卷：试算跑到决策点停住，主区把报告画出来等用户
- [x] 按执行 → 第一遍不重算，直接进第二遍
- [x] 按停 → 等价于 dry-run，输出根一个文件都没有
- [x] 多卷：不在决策点等人，屏上说清这一趟不续做
- [x] 等待期间会话仍然可交互，不冻屏
- [x] 等待期间按中止能干净退出，最终位置上没有那一卷，`partial` 也没留下

## 落地记录

### 谁判「等不等人」：`session::resuming`

**等不等人是调用方的策略，不是库的行为**（ADR 0012 决定第 3 条）。判它的是
`src/session.rs` 的 `resuming`——一个纯函数，收一份 `Request`，交回「真走哪一种模式」
加一个「等不等人」：

| 按下的键 | 范围 | 真走的模式 | 决策点上 |
|---|---|---|---|
| `t` 试算 | **单卷** | `Mode::Process`（`Retention::Keep`，决定第 5 条） | **等人** |
| `t` 试算 | 多卷 | `Mode::DryRun`（一格不改） | 不等人 |
| `x` 执行 | 任意 | `Mode::Process`（一格不改） | 不等人 |

单卷试算改走 `Process` 是为了留参照：答继续时第一遍才不必重算，而那正是续做买的东西。
「只算不写」在那条路上重述为**不写输出**（决定第 5 条）。判在这一层而不在状态机里，
是因为状态机既碰不到线程、也不该替这一层拿这个主意。

### 决策点在会话里长什么样

`Mode::Deciding(Instruction)`——一个**状态**，不是 `Mode::Running` 上的一个开关。
两处按得动的键是两套（跑着时只有 `s`，等答话时是 `x` 与 `s`），而「哪些键在哪个状态下
有效」那张表是 `session/state.rs` 唯一的产出；摆进同一个状态就要靠一个 flag 分岔。

- **屏上问什么**：全局条抬头写「整趟 · 等你拿主意」（横条这时一步不走，而眼睛盯着横条的
  人不会往下扫一行——与按停那一级挂在同一处，Q71）；报告区把那一卷画出来；
  屏底两行是「`x` 接着做第二遍（第一遍不重算）· `s` 收尾（这一卷不写，等价 dry-run）·
  `Ctrl-C` 退出会话」，加一句「上面那份报告是真的……输出根此刻一个字节都没有」。
- **哪些键能答**：`x` → `Action::Answer(Continue)`，`s` → `Action::Answer(Finish)`，
  `Ctrl-C` → `Action::Quit`。其余一律 `Ignored`——三层仍旧只读，`q`／`Esc` 与跑着时
  同一条（Q63：退出走中止，最容易手滑的两个键不该挂这个后果）。
- **答完回哪个状态**：`Deciding(闩) → Running(闩)`，**当场就转、不等下一帧**——
  慢一帧的话答话那两个键还在屏上摆着，按下去却已经没有人收了。
  **会话那个闩**（用户按停按到哪一级）原样带回去：决策点上答的字不进它，两者互不覆盖。
  答出去的那个字仍旧进**库那一侧**的闩（`progress::Standing`，记「观察者答过什么」），
  那是 `06` 定下的、也是「那一卷停在这儿之后剩下的卷不必开工」靠的东西——
  一个词两个闩，`CONTEXT.md` 的《会话》把这一点写开了。

两个键都是**已经有主的键**，因为它们在这里做的正是那个键一直在做的事：`x` 是执行
（「接着做第二遍」就是把这一趟做完），`s` 是停（决策点上答收尾停出来的现场恰好也是
「盘上不留半卷」）。另取两个新键就要多记两个只在这一刻有效的记号。

### 握手：一道闸，不靠 sleep 撞运气

`session/run.rs` 的 `Gate`：一把锁装 `{ waiting, said }`，加一个 `Condvar`。

- 计算线程在观察者里走到决策点 → `gate.ask()`：落座（`waiting = true`）、
  `wait_while(said.is_none())`。**落座与那一等之间一把锁都没松过**，因此屏上不会看到
  「等着」而实际早就走了那种中间态。
- UI 线程 `Running::decide(said)` → `gate.say()`：摆上那个字、敲钟。等着的那条线程当场醒，
  取走即清空（一趟里每一卷各有一个决策点，上一个答的字不该替下一个作答）。
- `Running::deciding()` 问的是「**还等着人答话吗**」（`waiting && said.is_none()`），
  **只决定屏上此刻画哪一副**；答话走的是另一条路。
  只看 `waiting` 那一格是不够的：`say` 与 `ask` 之间隔着一次重新抢锁，
  而屏上那一副是每帧问一次画出来的——答完话到那条线程醒过来这中间的某一帧上，
  屏底会把答话那两个键再摆一次，而那时已经没有人收了。
  `drive` 每帧问一次，与 `reap` 同一条。用例里的 `until_deciding` 转的是
  「等一个会成立的条件」，不是猜一段时长。
- **答话可以先到**：`Running::stop(Abort)` 往闸上说一个中止，而那时计算线程可能还没走到
  决策点。那个字留在 `said` 上等着，`ask` 一进来就取走。漏掉这一条的话，
  `leave` 会 join 一条永远等在闸上的线程——`leaving_while_the_decision_point_waits_throws_that_volume_away`
  是那个死锁的哨兵（挂住而不是红）。

观察者在闸上等之前**先把 `Live` 那把锁还掉**：会话那一头每帧都要借同一把锁画报告区，
而这一等可能是几分钟（`progress` 那条硬规矩的同一个理由）。

### `run::answer` 那条规矩怎么保住的

一个字没动，而且**这一票的形状正是它的延长线**。`Watch::observe` 现在分两支：

```
决策点 + 这一趟等人  → match answer(true, 闩) {
                          中止 => 中止,                 // 中止不让，不必问人
                          _    => gate.ask(),           // 让给用户当场那个字
                      }
其余一律            → answer(是不是决策点, 闩)          // 原样，一字未改
```

**两支都先过一遍 `answer`**：那条规矩因此仍旧只有一个出处，等人不等人只决定**让给谁**。
中止那一支非有不可的是**次序**：`Running::stop` 把中止也推到闸上，但那一处管的是
「线程已经等在闸上」的那一半；这一处管的是「它还没走到这儿」的另一半。
`an_abort_pressed_before_the_decision_point_does_not_stop_to_ask` 两半都覆盖得到
（抢不抢得在前面不由用例说了算，而两种收场是同一个答案）。

`answer` 那条规矩说的是「决策点上收尾要**让**，中止不让」。让给谁由这一趟定：
不等人的那一趟让成继续（原样），等人的那一趟让给**用户当场答的那个字**——
闸上还等着人答话，那一问不该由闩替他答。同一句话在 `Running::stop` 上再落一次：
**只有中止连着推开那道闸，收尾不推**。

三条钉着它：`only_the_decision_point_makes_a_finish_step_aside`（原有，一字未改）、
`finishing_in_the_middle_of_a_volume_still_lets_that_volume_land_whole`（原有，
它那个 `Watch` 显式装 `gate: None`——走的就是不等人那一支）、
以及新的 `the_decision_point_is_a_second_face_of_the_same_run_and_leaves_the_latch_alone`
（第一遍里按下的收尾在等答话前后一格不动，答话也不把会话那个闩往上推）。

### Q52：决策点那一条现在带着报告

「主区把报告画出来等你拿主意」在事件流上原本缺一环：拿主意要看的东西——卷级判定、
逐页结果、缓存用量、解码计数——要到 `VolumeFinished` 才交出去，而那一条排在决策点**之后**。
Q52 列的两条路里走的是第一条：`Event::PassStarted` 多一格 `so_far: Option<&VolumeReport>`,
只有决策点那一条带着它。另一条（会话拿逐步事件自己攒一份）走不通——判定它算不出来。

`process_volume` 里那一卷的报告因此**拼两次，而拼法只有一处**（一个 `assemble` 闭包）：
一次交给决策点，一次在收摊时。两次之间夹着第二遍，所以逐页那一步从「吃掉这一页」
改成「借着算」（`OutputPage::into_report` → `to_report(&self, …)`）。
`tests/resume.rs` 的 `the_decision_point_carries_the_volume_report_as_it_stands`
把两份逐格比了一遍——分了家的话，屏上看到的与最终报告说的就不是同一件事。

`Live` 把它收在 `summarized` 上，**不进 `Live::report`**：那一份装的是收摊了的卷，
混进去的话退出会话时印到 stdout 的那一份会多一卷「写在那里、盘上却没有」的东西。
三条出路各清一次（一卷跑完、一卷没做成、这一趟收场）。

### 「这一趟等于一次 dry-run」怎么说出口

单卷试算走的是 `Mode::Process`，而在决策点上答出继续之前，输出根一个字节都没有。
`Live::mode()` 因此从一个字段变成一个函数：续做那一趟在 `decided ∈ {None, Finish, Abort}`
时印 `DryRun`，答了继续才印 `Process`。报告抬头那一行「dry-run：只算不写，
下面的路径都还没落盘」正是这时要说的话，而它一处出处、两处消费（屏上的报告区、
退出会话时印到 stdout 的那一份）。别的三种（多卷试算、执行、没跑过）一格不改。

### 多卷：屏上说清不续做

`draw::resuming_line` 摆在屏底第二行（还没按过停的时候那一行本来空着），
两句都在**跑起来的当口**说：续做那一趟预告它会停下来（不预告的话，横条停住看上去
与卡住没有分别），不续做那一趟说清代价（按 `x` 那一趟第一遍要重算一遍）。
执行那一趟这一行仍旧空着。

### 等答话的那几分钟谁都不算

库那一侧减掉它已经是 Q41 定下的（`Report::elapsed`、`VolumeTiming::elapsed`），
但那一份要等收场才交得出来，而会话**边跑边画**。`Live` 因此自己也记一份
（`deliberated` + `deliberating_since`，只有续做那一趟开）：不记的话，
屏上那两个数会在人看着报告的那几分钟里一路往上涨，「剩 2h13m」说的就成了
「用户拿主意还要多久」。收场之后照旧换成库交出来的那一个。

### 命令行：一个字节都没变

`src/main.rs` 一行未改——`Bar::observe` 的 `match` 本来就不接 `PassStarted`
（ADR 0011 那条「库外的 `match` 一律带 `..` 与 `_`」在这里第三次兑现）。
退出码一字未动，黄金快照未变、未重录。`--no-default-features` 下照编。

### review 之后改的

`/code-review` 两轴各出了几条，都收了：

- **`Gate::waiting` 那一格有一帧的窗口**（两轴都逮到）。`say` 与 `ask` 之间隔着一次重新抢锁，
  而 `drive` 在同一次循环里就问了一遍 `deciding()`——答完话的那一帧屏底会把答话那两个键
  再摆一次，而那时已经没有人收了。改成 `waiting && said.is_none()`：答上了就不算等着了。
  `a_resuming_trial_waits_at_the_decision_point_and_goes_on_when_told_to` 里加了一句
  **不等 `until_done` 就问**的断言，钉的正是那一帧。
- **中止那一支绕开了 `answer`**。原来 `gate.ask()` 那一支根本不问闩，
  「决策点上收尾要让、中止不让」这条规矩靠的是 `Running::stop` 那一处远端的耦合。
  改成两支都先过一遍 `answer`，规矩因此仍旧只有一个出处（见上）。
- **`(Request, bool)` 那个裸标志**。`false` 在二十来个调用处说不出它否掉的是哪件事，
  换成 `live::Resuming { Waits, GoesOn }`——与 `draw` 那个 `Unrolled` 同一条理由。
- **`Live::mode()` 里中止那一支到不了**。写开是因为 `Instruction` 不非穷尽（多一级的那天
  这里编译不过），文档补了一句说明它眼下为什么到不了、真到了为什么也是同一个答案。
- **停车场新条目的收尾格式**用了 `State: open`，而《待处理》那 33 条一律是 `处置：待处理。`
  ——改齐。
- **`CONTEXT.md` 新写的那段与两段之上自相矛盾**：「等答话时按的 `s` ……闩一格不动」，
  而上面写着「决策点上答的字照样进闩」。一个词两个闩（会话记「用户按过什么」，
  库记「观察者答过什么」），那段话改成把两者分开说。
- **用例里搭同一个卷搭了四遍**：收成一个 `a_pass_through_volume`，本票新添的三条与
  原有的三条共用它。

### 数

全量 **543 通过 0 失败**（基线 529），**17** 个测试二进制。
`cargo fmt --check`、`cargo clippy --all-targets`（含 `--no-default-features`）干净，
`cargo doc --no-deps` 与基线同为 14 条既有告警。
`CONTEXT.md` 的《会话》加了一个词条（**等答话**）、给**决策点**那一条补了「带着报告」
那半句，并加了两段说明。停车场了结 Q52、Q53，Q72 上记了本票查过的那一条，新增 Q76、Q77。

### 用例

| 用例 | 钉的是 |
|---|---|
| `resume.rs::the_decision_point_carries_the_volume_report_as_it_stands` | 决策点那一条带着的报告：第一遍走完了、第二遍是零、与最终那一份只差计时 |
| `run.rs::a_resuming_trial_waits_at_the_decision_point_and_goes_on_when_told_to` | 停下来那一眼盘上什么都没有；`deciding()` 说得出在等人；答继续之后这一卷写全 |
| `run.rs::answering_finish_at_the_decision_point_writes_nothing_and_still_reports_the_volume` | 答收尾＝一次 dry-run：输出根空、报告照出、`second_pass == 0`、stdout 那一份印着 dry-run |
| `run.rs::leaving_while_the_decision_point_waits_throws_that_volume_away` | 等答话时退出会话：线程收得回来（死锁哨兵）、那一卷不进报告、盘上不留东西 |
| `run.rs::a_run_that_does_not_resume_never_waits_for_anybody` | 不续做的那一趟连闸都没有，照 `answer` 当场答字 |
| `state.rs::which_keys_do_what_in_which_state`（六之二段） | 等答话那个状态下哪些键有效：`x`／`s` 答话，别的十五个一律没有意义 |
| `state.rs::the_decision_point_is_a_second_face_of_the_same_run_and_leaves_the_latch_alone` | 跑着 ⇄ 等答话转场，会话那个闩一格不动；没跑着时这一问不作数 |
| `run.rs::an_abort_pressed_before_the_decision_point_does_not_stop_to_ask` | 按过中止的那一趟不停下来问人；两种抢法都不挂住、都不留东西 |
| `session.rs::answering_at_the_decision_point_reaches_the_thread_waiting_there` | 接头处：按 `t` → 停住 → 按一个没意义的键仍不冻屏 → 按 `s` → 跑完 |
| `session.rs::only_a_single_volume_trial_resumes` | 三种情形各走哪一种模式、等不等人 |
| `live.rs::the_summary_at_the_decision_point_stands_until_that_volume_lands` | 那一份不进报告，三条出路各清一次 |
| `live.rs::a_trial_that_never_walked_the_second_pass_prints_as_a_dry_run` | 抬头照哪一种印，四种答复各一次 |
| `live.rs::the_minutes_spent_deciding_are_charged_to_nobody` | 等答话的那几分钟不进「已用」，不等人的那一趟一格不减 |
| `draw.rs::waiting_at_the_decision_point_shows_the_report_and_the_two_ways_out` | 屏上那三处：抬头、那一卷画出来了、答话那两个键 |
| `draw.rs::a_trial_that_will_not_resume_says_so_while_it_runs` | 多卷说「不续做」、单卷预告会停、执行那一行空着 |

## 停车场结转

### Q52 — 决策点那一条事件不带这一卷的报告，而会话要在那里把报告画出来

- **From:** 票 `p1-session/06`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/progress.rs` 的 `Event::PassStarted`（只带一个 `pass`）与
  `Event::VolumeFinished`（带 `VolumeReport`，排在决策点**之后**）；ADR 0012 决定第 3 条
- **Why it did not block:** 库这一侧按票面落全了——决策点在那个点上问，答收尾就停在这儿，
  六条验收全绿。露出来的是会话那一侧的事：决策点到来时观察者手上只有逐步事件与失败页事件，
  **没有**这一卷的卷级判定、逐页结果与缓存用量——那份 `VolumeReport` 要到
  `VolumeFinished` 才交出去，而那一条排在决策点之后。ADR 0012 决定第 3 条说
  「会话在单卷时于此处把报告画出来并等用户拿主意」，而「拿什么画」在事件流上还缺一环。
  绕不过去的是次序：答收尾能拿到那份报告，但那时这一趟已经收场，续做的机会正好错过。
  真要补，形状上有两条路——往 `PassStarted` 里塞一份「此刻为止的卷报告」（变体非穷尽，
  加得进去，`02` 号票留的正是这个余地），或者让会话拿逐步事件自己攒一份不完整的。
  两条都要一个真消费方才判得出哪条对，而那个消费方是 `14`；本票的硬约束也写明
  UI 那一侧留给它。
- **What this ticket actually did:** **没有改事件的形状。**决策点照 `02` 定下的样子报
  `PassStarted { pass: Second }`，只把答复接了回来、把等人那段掐了出来。
  库这一侧「答收尾就停在这儿」因此完整可测（`tests/resume.rs` 六条），
  而「拿什么给人看」原样留给 `14`。`Pass::Second` 的文档写明了这一条事件上的答复当场作数，
  没有替 `14` 预设它该看什么。
- **Whose call:** `14`（会话里接上决策点）
- **处置：** 由本票（`p1-session/14`）了结。**走的是第一条：往 `PassStarted` 里塞一格。**
  `Event::PassStarted` 多了 `so_far: Option<&VolumeReport>`——**只有决策点那一条带着它**，
  另外两遍恒是 `None`（那时还没有汇总可交）。第二条路（会话拿逐步事件自己攒一份）
  真到有消费方时才看得清走不通：卷级判定、逐页判据、缓存用量、解码计数，
  会话一个都算不出来，逐步事件里也一条都没有。
  拼那一份要遍历逐页结果、读一次缓存用量，因此 `ask_before_the_second_pass` 收的是一个
  **闭包**——没人可问的那一趟连拼都不拼（命令行不带进度条的那条路走的正是那一支）。
  `process_volume` 里那一卷的报告因此拼两次而**拼法只有一处**（一个 `assemble` 闭包）：
  一次交给决策点、一次在收摊时。两次之间夹着第二遍，所以逐页那一步从「吃掉这一页」
  改成「借着算」（`OutputPage::into_report` → `to_report(&self, …)`）。
  两份逐格比过一遍（`tests/resume.rs` 的 `the_decision_point_carries_the_volume_report_as_it_stands`）
  ——分了家的话，屏上看到的与最终报告说的就不是同一件事。
  会话那一侧收在 `Live::summarized` 上，**不进 `Live::report`**：那一份装的是收摊了的卷，
  混进去的话退出会话时印到 stdout 的那一份会多一卷「写在那里、盘上却没有」的东西。
  命令行一行未改——`Bar::observe` 本来就不接 `PassStarted`。
  这一步换来的两笔代价另记：Q76（那一格只有一条事件用得上）、Q77（卷报告每卷拼两次）。
- **State:** settled

### Q53 — 单卷在决策点上答收尾，这一趟的收场仍是「走到头」

- **From:** 票 `p1-session/06`
- **Kind:** 你确实拿不准的单项
- **Where:** `src/lib.rs` 的 `run`（逐卷循环里只有两条 `break` 造得出 `Stopped`）；
  `src/report.rs` 的 `RunOutcome::of` 与 `RunOutcome::Completed`
- **Why it did not block:** 按现有定义走得通：`Completed` 说的是「点名的卷都走过一遍了」，
  而停在决策点上的那一卷**进了报告**（它做过事，只是没写出去），点名的卷因此一个都不少；
  `RunOutcome::of` 的文档也明写它只在「真被拿走了东西」的那两处叫得到，
  而「收尾按在最后一卷上、一卷都没被拿走」本来就不走那里。多卷时后面还有卷没开工，
  收的照旧是 `Stopped(Finish)`。露出来的是这个：**同一个手势，单卷收「走到头」、
  多卷收「停在半路」**，而单卷那一趟盘上一个字节都没写。
  要抹平它得给决策点一条自己的收场（或让 `Stopped` 也认这一处），
  而那要先答「一趟什么都没写出去算不算走到头」——那是领域决定，
  且它第一个真正的消费方是 `14`：会话拿这个值决定画什么。
  退出码不受影响：按停与决策点上停下都不是失败，那一趟仍是 `0`。
- **What this ticket actually did:** **没有动 `run` 的收场判定。**按现有定义走，
  **两种情形各由 `tests/resume.rs` 一条钉着**——多卷那一条断言 `Stopped(Instruction::Finish)`，
  单卷那一条断言 `Completed`。单卷那一行本来打算不写（不给一个将来可能要改的值钉桩），
  code-review 指出「不钉就没人看得见它变」，改成钉上并在注释里写明：
  钉它不是主张它一定对，是要让 Q53 的答案落下来时红的是这一行，
  而不是某个会话里画错的一屏。
  `RunOutcome::Completed` 的文档补了一句，把「有卷停在决策点上」并进
  「那是**卷**的结局，不是这一趟的」那一列，并点出后面还有卷时收的是 `Stopped`。
- **Whose call:** `14`（会话里接上决策点）——它是第一个真要读这个值的消费方
- **处置：** 由本票（`p1-session/14`）了结。**收场那个值一格没动，而会话不需要它变。**
  本票读它的地方只有一处：`draw::ended_line`（「收场 走到头 · 1 卷 · 用了 5s」）。
  露出来的那件事——「这一趟写没写」——**不由收场答**，而由报告抬头那一行答：
  `Live::mode()` 在续做那一趟答了收尾或中止时印 `DryRun`，抬头因此写着
  「dry-run：只算不写，下面的路径都还没落盘」。两者各答各的问题：收场答的是
  「点名的卷走没走完」，抬头那一行答的是「盘上有没有东西」。屏上两句都在。
  **没有给决策点单开一条收场**，理由是本条自己写着的那句：那要先答「一趟什么都没写出去
  算不算走到头」，而那是领域决定。真要抹平它，代价也已经量得出来——`Stopped` 眼下
  一律配一个 `Instruction`，而决策点上停下来的那一级恰恰不该让「剩下的卷不开工」
  跟着变；给它一个新变体则是 `RunOutcome` 的形状变更，命令行那一路的措辞
  （`render::outcome`）与退出码判定要一并复核，而本票的硬约束是命令行一字不变。
  **也没有在会话里另编一句措辞**：`ended_line` 的措辞跟报告那一套走
  （`crate::render::outcome`），会话不另立第二份出处。
  钉桩照旧在 `tests/resume.rs` 那两条上，一动未动——Q53 的答案哪天落下来，红的仍是那两行。
- **State:** settled
