# 02 — 持锁处调观察者当场炸掉

**What to build:** 「不得在持锁处调观察者」这条性质现在靠人核。`p1-session/02` 逐个调用点
核过一遍五处都在锁外，还把最容易踩的那一处改写成明式——但那是一次性的，
下一个往管线里加一条报到的人得自己再核一遍，而漏了不会红，会死锁。

给它一个哨兵：调试构建上，持着缓存那把锁去调观察者就当场恐慌，指名说是哪一处。
发布构建上一格开销都不留。

收停车场的 **Q40**。条目里另记着两样本票**不动**：报到时手里还攥着 rayon 工作线程与
读取层的在途字节额度——那两样不是锁、不会死锁，观察者久不返回只会把读取层背压闸住，
而那多半正是想要的。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 调试构建上，持着缓存锁调观察者会恐慌，消息指得出是哪一处报到
- [x] 发布构建上哨兵整个不在，热路径一格开销不留
- [x] 有一条用例故意踩上去，证明它真的炸（不是靠读代码相信）
- [x] 现有五处报到一处都不触发它
- [x] 观察者久不返回仍旧只是慢，不死锁——`p1-session/02` 那条下限用例照旧绿

## 落地记录

哨兵是**两半**：一半知道「此刻持着锁」，一半在报到那一步问它。

**持锁那一半在 `src/lib.rs`。**`lock` 交出来的不再是裸 `MutexGuard`，是一枚
`CacheGuard`——`MutexGuard` 加一格 `progress::LockSentinel`。它 `Deref`/`DerefMut` 到
`PageCache`，三处调用点（`assemble` 读用量、第一遍 `insert`、第二遍 `load`）一个字都没改。
`LockSentinel::new()` 在**线程局部**那格计数上加一，析构时减一：第一遍每条 rayon 线程各拿各的锁，
一格全局计数会把别的线程那把算到自己头上。

**问的那一半在 `src/progress.rs`。**主漏斗是 `Events::ask`——八条报到都经 `report` 走它。
检查摆在「有没有观察者」那道判空**之前**：只在没人可问的那一趟才走到的报到点，
正是最容易漏掉的一处，而漏掉它的那天有人接上观察者就死锁。
决策点（`ask_before_the_second_pass`）**另问一次**：它有自己一道判空，绕得过 `ask`，
而它恰恰是按设计要等人的那一处（见《复审》第一条）。

**消息指得出是哪一处报到**：事件名加上报到那一行。行号靠 `#[track_caller]` 从
`crate` 那一侧的调用点一路传下来（`step`／`page_failed`／`volume_finished` …→ `report` → `ask`），
名字靠 `Event::name` ——穷尽 `match`、不留 `_`，多一个变体时它当场编译不过。
（哨兵收的是**名字**而不是一条 `Event`：决策点要在造出事件之前就问，
造它得先拼一份报告；`PassStarted` 那个字因此单拎成一个常量，两处报的是同一个。）
炸出来长这样：

```
在持着卷缓存那把锁的地方报到了：Stepped（src\lib.rs:2739:16）。观察者可能很久不返回……
```

**发布构建上整个不在。**`LockSentinel`、那格线程局部、两处检查、`CacheGuard` 里那一格，
全挂 `cfg(debug_assertions)`；十一处 `#[track_caller]` 挂的是 `cfg_attr(debug_assertions, …)`——
它在发布构建上要多传一个隐式的位置参数，留着就不是「一格开销不留」了。
守着这一半的只有那几处 `cfg` 本身，加上收尾时手跑的一条 `cargo check --release`
（它够得着的只是「没有一处无条件引用了被 `cfg` 掉的东西」）：三条闸门跑的都是 dev profile，
一条都够不着「不在」那一半。这笔缺口记成停车场 Q86。

**故意踩上去的用例有两条**，都在 `src/lib.rs` 的单元用例里，都挂 `cfg(debug_assertions)`：

- `reporting_a_step_while_holding_the_cache_lock_blows_up`——攥着锁，下一句 `events.step()`。
  用 `catch_unwind` 把那句话捞回来，**两样都断言**：哪一条事件（`Stepped`），
  以及哪一行（`message.contains(file!())`）。只比事件名的话，`#[track_caller]` 那一串
  掉了任意一处都不会红——消息会改口指进 `progress.rs`，而「指得出是哪一处报到」当场落空。
- `the_decision_point_trips_the_sentinel_even_with_no_observer_to_ask`——决策点那一支，
  且这一趟**没有观察者**。理由见下面《复审》第一条。

**五处报到重核了一遍**，两条路：一条是读——幂等那一道的 `step`、第一遍 `Compute::page`
的 `page_failed` 与 `step`、第二遍写页与透传成员那两处 `step`、卷级 `volume_started`／
`volume_finished`，加上决策点，此刻手上都没有缓存那把锁；`assemble` 里
`cache: lock(&cache).usage()` 那一句的 guard 掐在闭包自己那一句里（`p1-session/02` 认下的
「最容易踩的那一处」，`p1-session/14` 之后它从一条 `let` 变成了这个闭包）。
另一条是跑——**哨兵不问有没有观察者，因此每一条走管线的用例都替这五处验了一遍**，
553 条全绿即是那一侧的证据。这比逐个调用点读一遍强的地方正在这里：它不是一次性的。

**没有动的两样**：报到时手里还攥着一条 rayon 工作线程与读取层的在途字节额度
（`read::Read` 的 `_permit`）。两样都不是锁、都不会死锁，哨兵照设计不管它们；
这条边界写进了 `LockSentinel` 自己的文档（原先它只是 Q40 里的一段话，
Q40 了结之后就没有出处了），并另开成停车场 Q85。

**行为一字未改**：命令行与会话两路一步没动，退出码照旧，黄金快照未变、未重录。
`CONTEXT.md` 的《进度》加了一行词条（**持锁哨兵 (LockSentinel)**）与一段说那条硬规矩的话——
规矩本身此前只写在 `src/progress.rs` 的模块文档里，词汇表上一个字都没有。

### 复审

两轴各跑一遍（Standards / Spec）。收了这几条：

- **决策点绕过了哨兵**（Spec 轴，最重的一条）。`ask_before_the_second_pass` 有自己一道
  「没人可问就直接回继续」的判空，先于 `ask` 返回——八个漏斗里唯一**按设计要等人**的那一处，
  在没有观察者的那一趟（命令行不带进度条走的正是它）根本走不到检查。
  哨兵的收参数从 `&Event` 改成名字，那一处在自己的判空之前也问一次；
  新添的第二条用例正是踩这一支。
- **那条编译期断言是空的**（Spec 轴）。`LockSentinel` 是零尺寸类型，带不带那一格
  `CacheGuard` 都是 16 字节，`size_of` 相等恒成立；真开销在 `Drop` 那两次线程局部加减与
  `#[track_caller]` 多传的隐式参数，它一个都量不到。**删了**，并如实记成停车场 Q86：
  「不在」那一半眼下没有闸门。
- **用例只断言了事件名，没断言行号**（Spec 轴）。`#[track_caller]` 那一串掉任意一处都不会红，
  而「指得出是哪一处报到」正是验收条第一句。改成 `catch_unwind` 捞回那句话，
  事件名与 `file!()` 两样都断言。
- **`CONTEXT.md` 把范围说大了**（Spec 轴）。原话是「这条规矩由持锁哨兵守着」，
  而哨兵只数 `crate::lock` 那一把——`read::Throttle::lock` 是另一把真锁，不在里面。
  改成「守的是最要命的那一把，别的锁仍靠代码自己站住」。词条本身也按《渐进披露》收短，
  `cfg`／发布构建那半句退回 `progress.rs`。
- **Q85 缺《处置》行**（Standards 轴，停车场自己的格式）。补上。
- **`assemble` 头上那句注释已经过期**（Standards 轴，《文档写作》第 1 条：写当前成立的事实）。
  它说「用量在拼进结构体**之前**读回来」，而 `p1-session/14` 之后那一句就在结构体字面量里。
  改成说当前成立的事：guard 是 `usage()` 那一句的临时量，闭包一返回就没了。
- **`none_held` 名字像个谓词**（Standards 轴，Mysterious Name）。它不返回 `bool`，它恐慌——
  改名 `assert_none_held`。

几条没收，有理由：

- **哨兵劈成两半在两个文件里**（Standards 轴，Shotgun Surgery／Divergent Change）。
  那正是这件事的形状：规矩是 `progress` 的（观察者是它的），锁是 `lib` 的（缓存是它的），
  合到一处就得让 `progress` 认识 `PageCache`。下一把要守的锁确实会同时动两个文件，
  但那是一行 `LockSentinel::new()` 的事。
- **`CacheGuard` 只是转发**（Standards 轴，Middle Man）。发布构建上它确实是个零收益包装，
  而那正是要的：调用处一个字不改，`cfg` 掉之后什么都不剩。
- **`CacheGuard` 没进 `CONTEXT.md`**（Standards 轴，口径不齐）。词汇表收的是领域说得出口的东西
  ——`Events`、`Standing`、`ProgressSink` 同层，一个都不在里面。进去的是**持锁哨兵**，
  因为它守的那条规矩是观察者这个 seam 的性质，而那条规矩此前词汇表上一个字都没有。
- **停车场里另外三条（Q78／Q80／Q81）也缺《处置》行**（Spec 轴顺带点出）。不是本票记的，
  也不是本票要了结的，动它们是替别的票收摊。

### 数

| 闸门 | 结果 | 最后一行 |
|---|---|---|
| `cargo test` | **554 通过 0 失败**（基线 552，多的两条是故意踩哨兵那两条），17 个测试二进制 | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（doc-test 那一格） |
| `cargo test --no-default-features` | **519 通过 0 失败**（基线 517，同那两条），同样 17 个二进制 | 同上（doc-test 那一格） |
| `cargo check --features profiling` | 干净 | ``Finished `dev` profile [unoptimized + debuginfo] target(s)`` |

闸门之外：`cargo fmt --check` 干净；`cargo clippy --all-targets` 三种特性组合
（默认、`--no-default-features`、`--features profiling`）均干净；`cargo doc --no-deps`
与 HEAD 同为 **14** 条既有告警。另手跑了一条闸门之外的 `cargo check --release`——
「发布构建上哨兵整个不在」只有它够得着，干净（够得着的也只是编译这一层，见 Q86）。
**黄金快照未变、未重录。**

停车场了结 Q40（→ `p1-session/02`《停车场结转》），新增 Q85、Q86；待处理 34 → 35。
