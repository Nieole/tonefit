# 24 — 三条闸门各用各的目录，收进一条命令；死代码报得出来

**What to build:** 两笔基建的账：

- **三条闸门各用各的 target 目录。** 它们今天共用一个，特性一来一回整棵树判失效、
  全量重编十几分钟；更坏的是**假红**：跑过闸门 2 再跑闸门 1，
  用例启动到的是上一趟留下的**不带终端库的二进制**，退出码 `2` 而非 `1`。
  收进一个 `xtask`（workspace 成员加一条别名），**一条命令跑满三条**，各用各的目录。
  `docs/agents/gate.md` 跟着改；
- **死代码报得出来。** 关掉终端库那一趟，整个会话模块上放开了 `dead_code`，
  将来添的死代码在闸门 2 上也不报。**收窄到逐处**。

**发布构建那一条哨兵不做**（验它要先引基准）；**CI 不做**（谁来跑这条命令是另一件事）。

收停车场的 **Q95**、**Q84**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 一条命令跑满三条闸门，三条各用各的 target 目录
- [x] 跑序不同不再假红：跑过闸门 2 再跑闸门 1，退出码那几条用例照旧绿
- [x] `docs/agents/gate.md` 说得出这条命令与各自的目录
- [x] `dead_code` 收窄到逐处，会话模块上不再整个放开
- [x] 收窄之后闸门 2 上没有新的告警
- [x] 三条闸门全绿

> **六条全做到，没有保留。**四处**判断**（不是保留，但要写明读法）：
>
> - **闸门 1 用的是默认 `target`，不另开一个。**三条因此仍是三个互不相干的目录，
>   而闸门 1 与日常的 `cargo build`、`cargo test`、编辑器那一路的特性组合逐格相同——
>   另开一个目录一处失效都不解决，只是让每个人把同一棵树建两遍。记在新的 **Q206**。
> - **`.cargo/config.toml` 从 gitignore 里放出来了。**别名要对每个人生效就必须入库，
>   而那个路径原先整份被忽略（装的是本机的 `PKG_CONFIG_PATH`）。本机那一半改走
>   shell 环境变量或用户级 `~/.cargo/config.toml`，`config.toml.example` 删掉、
>   说明搬进入库那一份的注释里。**Windows + vcpkg 那一路本机验不到**，记在新的 **Q205**。
> - **收窄 `dead_code` 的代价照收**：一句整模块的放松换成 **32 句**逐处的
>   （`live.rs` 9 处、`state.rs` 23 处，rustfmt 把每一句排成四到六行，两个文件各多 51 行与 92 行）。
>   Q84 自己预告过这笔代价（它当时数出九处），换来的正是它要的那件事。
> - **`xtask` 自己不在任何一条闸门里，也不在两遍 clippy 的照射范围里**——
>   `default-members` 把它挡在外面，那正是票面「不许进产物依赖」要的。记在新的 **Q208**。
>
> **`CONTEXT.md` 一个字没改**（闸门是工程约定，不是领域术语）。
> **闸门仍是三条**：没加发布构建那一条（Q86），没引 CI（Q82），两条都留给 29 号票。

## 落地记录

### 一、一条命令跑满三条

```
cargo xtask gate
```

`xtask/` 是一个 workspace 成员，别名在 `.cargo/config.toml`
（`xtask = "run --quiet --package xtask --"`）。**它不进产物的依赖**：
`default-members = ["."]` 把它挡在 `cargo build`／`cargo test` 之外，
`Cargo.lock` 上它多出来的是**四行、零依赖**的一格。**一个外部依赖都不收**——
它做的事只有「按次序起几个 cargo 子进程、把两条流原样转出来、把 `test result:` 那几行数下来」，
std 全都答得出。

三条各自的目录：

| | 命令 | target 目录 |
|---|---|---|
| 1 | `cargo test` | `target`（默认那个） |
| 2 | `cargo test --no-default-features` | `target/gate/no-default-features` |
| 3 | `cargo check --features profiling` | `target/gate/profiling` |

**头一条红了就停**（后面那两条各要好几分钟）。走完印一张《数》：三条各自的最后一行、
通过与失败的合计，以及 lib / bin 两个数——**票据的《数》照抄它**。
`cargo test` 印结果的次序是定的（lib → bin → `tests/` → 文档测试），
因此 `test result:` 那几行按出现次序头一条算 lib、第二条算 bin。

**只跑其中几条、按点名的次序**：`cargo xtask gate 2 1`。假红那一条就是拿它验的。

**闸门之外那四条**收进 `cargo xtask polish`：`cargo fmt --check`、
`cargo clippy --all-targets`、`cargo clippy --all-targets --no-default-features`、
`cargo doc --no-deps`。收它们只为一件事——第三条的特性组合与闸门 1 不同，
摆在默认目录里同样会让整棵树来回失效，它因此跟着闸门 2 用同一个目录。
另外两条 clippy 与 `cargo doc` 留在默认 `target` 里：**clippy 与 `cargo test` 的产物互不失效**
（当场验过——`cargo xtask polish` 跑完紧接着 `cargo test --no-run` 是 **0.25 秒**，
一格都没重建），而文档因此照旧落在 `target/doc`。

哪几条是闸门、哪几条是顺带，写在 `docs/agents/gate.md`；`CLAUDE.md` 的《闸门》
那一段改成指这一条命令，三条各是什么仍只由 `gate.md` 说。

### 二、假红那一条怎么消掉的

两种特性组合的 `tonefit` 可执行文件从此落在两个目录里，谁都不覆盖谁——
`cargo test` 判定它「新鲜」时不重新摆放它这件事还在，但**没有第二种组合来覆盖它了**。

**当场验过**：`cargo xtask gate 2 1`（先闸门 2、紧接着闸门 1）两条全绿，
`tests/session.rs` 的 `no_arguments_without_a_terminal_says_so_and_still_shows_the_usage`
在闸门 1 那一趟里 `ok`，两条的数与 1→2→3 那一趟**逐格相同**（796 / 670）。

### 三、`dead_code` 收窄到逐处

`src/main.rs` 的 `mod session` 上那一句 `#[cfg_attr(not(feature = "tui"), allow(dead_code))]`
没了。`--no-default-features --all-targets` 数出来是 **18 条告警、32 个条目**
（`live.rs` 9、`state.rs` 23），各挂各的一句：

```rust
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "只有画法与那条循环读得到，而它们在 tui 特性后面")
)]
```

夹具那几处（`live::fixture` 里只有画法那一侧的用例用得着的五个）用的是另一句 reason。
**放开的面从「`mod session` 底下全部六个模块」缩到这 32 个条目**：将来在会话里添的
**真**死代码，闸门 2 上照旧当场报。收窄之后闸门 2 一条告警都没有。

分界那一段（`src/session.rs` 模块文档《终端库在哪一半》）跟着改：
说的是「各挂各的一句」，以及**为什么不挂在 `mod session` 上**。

### 四、`.cargo/config.toml` 入库（Q205）

别名要对每个人生效就必须入库，而 `.gitignore` 原先整份忽略那个路径——它装的是本机的
`PKG_CONFIG_PATH`（Windows 上 vcpkg 提供的 dav1d），入库的只有一份
`config.toml.example` 让人自己拷过去。**两件事抢同一个文件名**：拷过去的那一份
会把别名覆盖掉。

走的是「让它入库」这一条：`.gitignore` 里那一条删了，入库那一份只有 `[alias]`；
`config.toml.example` 删掉，dav1d 那段说明连同两种本机给法（shell 环境变量，
或者用户级的 `~/.cargo/config.toml`）搬进入库那一份的注释里。
Linux/macOS 本来就不必配任何东西，受影响的只有 Windows + vcpkg 那一路，**而本机上验不到它**。

### 数

三条闸门跑满，三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **796 通过 0 失败**；lib **213** / bin **328** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **670 通过 0 失败**；lib **213** / bin **202** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.33s`` | 干净，一条告警都没有 |

**一条用例都没加、一条都没减**：三个数与本票开工前在 `main` 上核过的那三个逐格相同
（796 / 670，lib 213 / bin 328 与 213 / 202）。本票动的是**跑法**，不是被跑的东西。

**两边对照过，数一样**。三趟各自的来路：

| 跑法 | 闸门 1 | 闸门 2 | 闸门 3 |
|---|---|---|---|
| `cargo xtask gate`（1→2→3） | 796（213 / 328） | 670（213 / 202） | `Finished` |
| `cargo xtask gate 2 1`（验假红） | 796（213 / 328） | 670（213 / 202） | 没跑 |
| 老三条各跑一遍 | 796（213 / 328） | 670（213 / 202） | `Finished` |

**老三条那一趟里，闸门 2 与闸门 3 是带着各自的 `CARGO_TARGET_DIR` 跑的**
（`CARGO_TARGET_DIR=target/gate/no-default-features cargo test --no-default-features`，
闸门 3 同理），闸门 1 是光秃秃的 `cargo test`。不把后两条摆回共用目录里跑，
是因为那正是本票要治的病：摆回去要么整棵树重建十几分钟、要么当场造出一次假红，
而这台机器开工时盘上只剩 4.1 GB（见 Q207）。命令的字面一个字没变，变的只有目录。

**闸门之外那一遍**（`cargo xtask polish`，四条全绿）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都**零告警**；
`cargo doc --no-deps` 仍是 **15 条告警**（`tonefit (lib doc) generated 15 warnings`），
一条没多。

盘上三棵树量下来是 **5.4 GB + 5.5 GB + 499 MB ≈ 11.4 GB**——
**比开工时那个共用目录里积下的 19 GB 还小**（Q207）。

**评审那五条改完之后三条闸门与 polish 各又跑了一遍**（改的全在 `xtask/` 与文档里，
`src/` 一个字节没动）：三条仍是 796（213 / 328）／670（213 / 202）／`Finished`，
四条 polish 仍全绿，而那张《数》此刻自己印得出
`warning: \`tonefit\` (lib doc) generated 15 warnings` 那一行了。

### 评审提的五条

**五条都是真的，五条都照改**（没有一条改到 `src/` 里，闸门的数因此一格没动）：

- **`gate.md` 那张表把三条裸命令原样列着，照着敲一次就把假红召回来。**表右边那一列
  （各自的目录）**只有走 `cargo xtask` 才成立**，而文中没有一句话拦着人直接敲左边那一列。
  表底下补了一句：**这三条不要裸敲**，要单跑某一条走 `cargo xtask gate 1`／`2`／`3`。
- **一个非 UTF-8 字节就让整条流提前收摊。**`BufReader::lines()` 碰到非法字节回 `Err`，
  原来那一句 `else { break }` 会关掉管道读端，子进程下一次写拿到 EPIPE 当场死——
  **一条本来全绿的闸门被报成红的**，屏上还说不出为什么。改成按字节 `read_until` 加
  `from_utf8_lossy`：坏字节变成一个替换符，那一行照转，流不断。
- **两条流都是管道，cargo 因此一个颜色都不上。**我们自己这一头是终端时把
  `CARGO_TERM_COLOR=always` 传下去（转出去的是同一串字节）；重定向到文件时不传，
  免得落一堆转义码。`gate.md`《读结果》另补一句：**闸门 2 与闸门 3 头一趟同样要几分钟**
  （各自那个目录是空的），屏上一行行印着 `Compiling` 就是它在动。
- **`cargo doc` 那个告警数没被数出来。**`gate.md` 明写「那几条既有告警的条数记在票据的
  《数》里」，而走完那张表只印最后一行——`Generated .../index.html`，正好把唯一要追踪的
  那个数漏在前面几行。`xtask` 因此多认一种行（`… generated N warnings`）并原样印进那张表，
  两遍 clippy 的那一行一并收下。
- **老克隆 `git pull` 会被未跟踪的 `.cargo/config.toml` 挡住。**那个路径从前整份被 gitignore，
  盘上那一份装着本机的 `[env] PKG_CONFIG_PATH`；拿它盖掉入库那一份的话别名就没了。
  迁移那句话写在入库那一份的注释顶上（补进 Q205）。

**评审另外核过、确认没问题的几点**：`default-members` 确实把 `xtask` 挡在
`cargo build`／`test`／`clippy`／`doc` 之外；`cargo fmt` 走全 workspace，排版那一条盖得住它；
用户级 `~/.cargo/config.toml` 的 `[env]` 与入库这份的 `[alias]` 由 cargo 合并、不冲突；
32 处放松与 `mod session` 上那句 `cfg` 自洽（`--no-default-features` 不带 `test` 时整个模块
不编译，`--features profiling` 那一趟 `tui` 开着、一处放松都不生效）。

## 停车场结转

**了结两条**（原文连同处置挪到这里，`## 待处理` 里已删掉，`## 已了结` 索引表各加一行），
**新记四条**（留在《待处理》里）：

| 新记的 | 一句话 |
|---|---|
| **Q205** | 别名非入库不可，而 `.cargo/config.toml` 原先整份被 gitignore（装的是本机的 `PKG_CONFIG_PATH`）——让它入库，本机那一半改走环境变量或用户级配置 |
| **Q206** | 闸门 1 用的是默认 `target`、不是自己一个目录：它与日常那一路的特性组合逐格相同，另开一个只是把同一棵树建两遍 |
| **Q207** | 分目录换来的是盘上三棵树（11.4 GB），而开工时盘 98% 满只剩 4.1 GB——`cargo clean` 一次开的工，三棵树反倒比原来那一个共用目录小 |
| **Q208** | `xtask` 自己不在任何一条闸门、也不在两遍 clippy 的照射范围里（`default-members` 把它挡在外面，那正是「不许进产物依赖」要的） |

**Q82（CI）与 Q86（发布构建那一条哨兵）不收**：P4 判为不做／认下，由 29 号票归档。

### Q95 — 跑过 `cargo test --no-default-features` 之后再跑 `cargo test`，`session` 那条会假红

- **From:** 票 `volume-discovery/01`
- **Kind:** 路过发现
- **Where:** `docs/agents/gate.md`《读结果》；`tests/session.rs` 的
  `no_arguments_without_a_terminal_says_so_and_still_shows_the_usage`
- **Why it did not block:** 两种特性组合的 `tonefit.exe` 落在**同一个** target 目录里、
  互相覆盖，而 `cargo test` 判定自己那一份「新鲜」时不会重新摆放它。于是闸门第二条跑完
  之后再回头跑第一条，用例启动到的是上一趟留下的**不带 `tui`** 的那个二进制：
  无参数时它走 clap 的必填项错误、退出码 `2`，而这一条要的是会话那条岔路的 `1`。
  当场量过三件事——`cargo build` 之后那个 exe 回 `1`（会话那条），`cargo test` 之后回 `2`
  （clap 那条）；换一个干净的 `CARGO_TARGET_DIR` 从头跑第一条，17 个二进制全绿、
  一条 `FAILED` 都没有。与任何一张票的改动都无关：它只取决于上一次在这个目录里
  build 过哪一种特性组合。
- **What this ticket actually did:** **没有动闸门，也没有动那条用例。**本票的三条闸门按
  1→2→3 的次序各跑一遍，第一条的数取自它自己那一趟（以及一次干净 target 目录的复跑）。
  这一笔连同「按次序跑」写进了本票落地记录的《数》。
- **Whose call:** 拍板的人（`docs/agents/gate.md`《读结果》要不要加一句「三条按次序跑」，
  或者让闸门各用各的 target 目录）
- **处置：** **了结。**走的是这一条列出来的第二条去处：**闸门各用各的 target 目录**。
  闸门 2 落在 `target/gate/no-default-features`、闸门 3 落在 `target/gate/profiling`，
  闸门 1 照旧是默认的 `target`（它与日常那一路的特性组合逐格相同，共用不出这件事——见新记的 Q206）。
  两种特性组合的 `tonefit` 可执行文件从此落在两个目录里，谁都不覆盖谁，**跑序因此说明不了任何事**。
  第一条去处（在《读结果》里加一句「三条按次序跑」）**不必了**——次序不再是一个前提；
  `gate.md` 里换上的是《为什么各用各的目录》那一节，把这一条记的两笔（整棵树来回失效、假红）
  连同治法写在那里。
  **当场验过**：`cargo xtask gate 2 1`（先闸门 2、紧接着闸门 1）两条全绿，
  `no_arguments_without_a_terminal_says_so_and_still_shows_the_usage` 在闸门 1 那一趟里 `ok`，
  两条的数与 1→2→3 那一趟逐格相同（796 / 670）。

### Q84 — 关掉 `tui` 那一趟，整个 `session` 上放开了 `dead_code`

- **From:** 票 `p2-loose-ends/01`
- **Kind:** 我确实拿不准的单项
- **Where:** `src/main.rs` 的 `#[cfg_attr(not(feature = "tui"), allow(dead_code))] mod session;`
- **Why it did not block:** 状态机那四个模块搬出特性之后，`tui` 关掉的那一趟里读它们的只剩
  它们自己的用例——画法与那条循环整个不编译，于是只有画法读得到的取值器
  （`Live::resumes`、`Session::title`、`Presets::names` 等，`--force-warn dead_code` 数出来
  是九处：`live.rs` 四处、`state.rs` 五处）全成了「没人读」。
  票面第五条要的是「关掉之后不留 `dead_code` 告警」，而那九处不是死代码，
  是这一趟的前提。真要一条不放松，只有把画法也搬出终端库，而画法搬不出去。
- **What this ticket actually did:** **在关掉的那一趟上整模块放开，默认那一趟一格不放松。**
  `cargo clippy --all-targets` 与 `--no-default-features` 两遍都干净，
  真死掉的东西仍由第一条闸门当场红。**放开的面比那九处宽**：`mod session` 上一句管住
  它下面全部四个模块，`complete` 与 `run` 眼下一处不占，但**将来**在它们里添的死代码
  在第二条闸门上也不报了。代价因此是将来的，不是眼下的——收窄它得逐处挂 `cfg_attr`，
  那是把一句噪声换成九句。
- **后续（`p3-session-legibility/04`）:** 视口那个纯函数（`session::viewport`）也摆在了
  特性外面，关掉 `tui` 那一趟因此多出五条**真跑得到**的用例。**放开的面一格没变**——
  `mod session` 上那一句照旧管着底下五个模块，`viewport` 自己在那一趟里除用例外没人读。
  本条的处置不变。
- **Whose call:** 拍板的人（这笔放松认不认、要不要收窄到逐处）
- **处置：** **了结。收窄到逐处。**`mod session` 上那一句整模块的 `cfg_attr` 没了，
  改成 **32 个条目各挂各的一句**（`src/session/live.rs` 9 处、`src/session/state.rs` 23 处，
  `--no-default-features --all-targets` 数出来是 18 条告警）。
  这一条自己预告的代价照收：**一句噪声换成 32 句**（当时数出九处，此刻是 32 个条目——
  `viewport` 那一笔与后来几张票添的那几格状态、几份画法那一侧的夹具都在里面），
  rustfmt 把每一句排成四到六行，两个文件各多 51 行与 92 行。
  换来的正是这一条说的那件事：**放开的面不再是「将来」**——`complete` 与 `run` 里将来添的
  死代码，闸门 2 上当场报。收窄之后闸门 2 一条告警都没有（796 / 670 两趟的输出里
  `warning` 一行不出）。分界那一段（`src/session.rs` 模块文档《终端库在哪一半》）跟着改：
  说的是「各挂各的一句」，以及为什么不挂在 `mod session` 上。
