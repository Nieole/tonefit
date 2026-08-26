# 09 — 收掉三处名不副实的用例

**What to build:** 三处用例现在承诺的性质和它们实际断言的不是一回事，读 CI 日志会把它们当证据。

- **冒烟用例**在环境变量未设时打一行日志就返回，框架记为通过，在输出里与真跑过的用例长得一模一样。
- **并发用例**断言「并发的读取数不小于串行的」，展开就是不小于一，把并发悄悄退化成串行照样通过。
- **一条进度用例**只有三行调用、零个断言，实际只测「不恐慌」，名字承诺的却是别的性质。

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] 冒烟用例在未提供素材时是明确的跳过状态，不计入通过
- [x] 并发用例断言的形式在并发退化成串行时会红
- [x] 进度用例要么补上真实断言，要么改名为它实际验证的那件事
- [x] 测试输出里能一眼看出哪些是 opt-in、本次没跑

## 落地记录

三处各改各的，都按同一条标准验收：**把它该逮的那件事真做出来，看它红不红**。

### 冒烟用例：自带 harness

`tests/smoke.rs` 改成 `harness = false`（`Cargo.toml` 里多一节 `[[test]]`），文件自己有 `main`。
为什么内建 harness 表达不了这个跳过，写在 `tests/smoke.rs` 的模块文档里，此处不重述。

三种结局各印一行，头两个字就分得开：

```
跳过 real_material_runs_through_the_pipeline：TONEFIT_SAMPLES 没有指向任何目录，本次一个卷都没跑。
跑过 real_material_runs_through_the_pipeline：<素材目录> 下 27 页处理成。
```

红那一路走 panic，`cargo test` 照旧报 `error: test failed`。

**这个二进制不再向总数贡献任何「通过」**：跳过时它一条 `test result` 都不印，
269 于是变成 268 加一行跳过。验收第 1 条与第 4 条落在同一处。

自带 harness 带出一件本来由框架代劳的事：命令行的过滤词要自己认，
否则 `cargo test golden` 会顺带跑一遍真实素材。规矩写在 `selected()` 的文档里，
两条要点是多个过滤词之间为**或**（对着内建 harness 实测过），
以及**拿不准就跑**——libtest 那几个分离取值的选项（`--test-threads 4`）的值不以 `-` 开头，
照过滤词认会把 `4` 当成谁都不命中的过滤词，于是本该跑的一趟静悄悄跳过。
那正是这张票要收掉的那种谎，所以命令行上出现任何 flag 就一律跑。

顺带两处：输出不再被捕获，因此 `-- --nocapture` 没有必要了，
`summarize` 的 `eprintln!` 一并改成 `println!`，`samples-wanted.md` 里那行命令去掉这个尾巴。

**实跑过四种输入**（`_samples/网络资源/N和S/第10话.cbz` 单卷，release）：

| 输入 | 结果 |
|---|---|
| 环境变量未设 | 印「跳过」，0 通过，退出码 0 |
| 指向 `Cargo.toml`（不是目录） | panic，退出码 101 |
| 指向一个空目录 | panic「一页都没有处理成」，退出码 101 |
| 指向一个真卷 | 印「跑过」，27 页处理成 |
| `cargo test golden` 且环境变量已设 | 印「跳过：命令行的过滤词点的是别的用例」 |
| `cargo test real_material` 且环境变量已设 | 真跑 |

后两条是 harness=false 自己带出来的新面，一并钉住。

### 并发用例：不等号换成等号

`tests/concurrency.rs` 的 `io_mode_overrides_the_probe_and_the_report_says_where_the_number_came_from`：

```rust
// 从前
assert!(concurrent.readers >= serial.readers, ...);
// 现在
assert_eq!(concurrent.readers, num_cpus::get().max(1), ...);
```

理由写在那处注释里：`serial.readers` 恒为 1（同一个用例上一行就断言了），
不等号展开就是「不小于 1」，一条永真的断言。等号那一边取的是库自己用的那个数
（`crate::cores`），同一个进程问到的结果一致。

**这一条的边界照直说**：单核机器上并发与串行本就分不开，等号在那里退成 `1 == 1`，
验收第 2 条在单核 runner 上不成立。与主机无关的那一半靠 `src/medium.rs` 的
`io_mode_overrides_the_probe_in_both_directions`——那一条直接传 `cores = 8` 进 `IoPlan::decide`，
但它是**本票之前就有的**用例，不是这次补的覆盖。`run` 那个 seam 上注入不了核数，
要在单核上也红，得先让核数变成一个能从外面塞进去的参数。

顺带记一处**没有改**的东西：`CONTEXT.md`《I/O 与并发》说读取层与计算层的并发度
「两个数不相干」，而代码里它们相等。这处分歧本票之前就在，只是被这个等号照亮了，
已记入《非阻塞问题》。

### 进度用例：补断言，同时改名

`src/progress.rs` 的 `a_run_without_an_observer_reports_into_nowhere` 三行调用零个断言，
测到的只有「不恐慌」。改成 `every_step_reaches_the_installed_observer_and_nowhere_else`：
测试模块里加一个记账观察者 `Tally`，先走装了观察者的那一端、断言每一下**恰好到一次**，
再走 `Steps::new(None)` 那一端、断言那三个数**一个不动**。为什么后半句非要前半句当参照，
写在该用例的文档里。

`Tally` 的形状照 `tests/concurrency.rs` 里的同名夹具办（句柄 + 一格共享记账）：
两者跨 crate 共用不了代码，至少共用一个样子。三样各有各的读取器、各断各的，
断言失败时印得出是哪一格错了。

开卷那一格记的是**参数**而不只是次数——卷路径与预告的步数原样收下来比对。
只数次数的话，把卷报错、把总步数报错都不会红，而预告的步数报错正是进度条
「停在某个百分比上再也不动」的样子。

### 三组变异，每组都对照跑了旧形式

基线全绿。每一组都做了两次：先在新断言上跑（该红），再把旧断言放回去、变异不动（对照）。

| 变异 | 新断言 | 旧断言 |
|---|---|---|
| `IoPlan::decide` 里 `IoMode::Concurrent` 改派 1 条读取 | 红：`left: 1, right: 32` | **绿** |
| `Steps::step()` 改成空操作，报到悄悄丢掉 | 红：`走过的步没有原样到达 left: 0, right: 2` | **绿** |
| 用例里 `Steps::new(None)` 换成 `Steps::new(Some(&sink))` | 红：`没装观察者，步却到了某处 left: 2, right: 1` | —— |
| `Steps::started` 把预告的步数报成 `steps + 1` | 红：`left: [("卷一", 11)], right: [("卷一", 10)]` | —— |

头两行就是这张票的票面：并发整个退化成串行、进度整段丢掉，旧断言两次都全绿。
第三行钉的是新断言的后一半——「报到了空处」那一句真的有牙。

### 逐个二进制的通过数

| 测试二进制 | 通过 |
|---|---|
| `unittests src/lib.rs` | 107 |
| `unittests src/main.rs` | 20 |
| `tests/concurrency.rs` | 9 |
| `tests/container.rs` | 28 |
| `tests/exit_code.rs` | 1 |
| `tests/golden.rs` | 2 |
| `tests/idempotency.rs` | 16 |
| `tests/isolation.rs` | 15 |
| `tests/metric.rs` | 7 |
| `tests/pipeline.rs` | 57 |
| `tests/profile.rs` | 6 |
| `tests/smoke.rs` | **跳过**（0） |
| Doc-tests | 0 |
| **合计** | **268** |

改动前是 269：差的那一项正是冒烟那条假通过。`cargo fmt --check` 与
`cargo clippy --all-targets` 均无输出。
