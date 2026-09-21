# 15 — 切换到新界面，删掉旧的那一副

**What to build:** 并存收口。无参数敲 `tonefit` 进来的真会话从这一票起进新界面，旧的那一副整个删掉。

- 终端循环改用新输入入口与新画法；
- 删掉：旧的焦点取值（左栏、编辑一行、报告区、展开一枝、展开一卷、取值栏）与它们的按键分派；旧画法那几块
  （左栏、报告区、目录表、卷表、逐页表，以及旧的总览、预设栏、覆盖层、屏底）连同快照用例；「这一趟的前提」那张覆盖层；
  `Live` 里旧的卷身份；
- 要够着真一趟的用例（按停止、答话、预设读写、灰阶测试图）只留新输入入口那一份；
- 视口那张表改成新的几处：卷列表、每页结果、设置栏、详情栏、预设栏、补全框、全部按键；
- 文档：`CONTEXT.md`《会话》节首「落地之前，代码里仍是 ADR 0017 那一副」那一句删掉；ADR 0019 状态改成已落地；
  会话与画法的模块文档换成新块的名单；
- 退出时印到 stdout 的那一份照旧；stderr 不是终端时的进程级用例照旧。

**Blocked by:** 09 — 自动滚动、问题跳转与搜索；12 — 等待确认；14 — 预设栏

**Status:** resolved

- [x] 无参数进会话看到的是新界面；进程级入口用例照绿
- [x] 旧焦点取值、旧画法模块、旧快照用例、「这一趟的前提」覆盖层、`Live` 旧卷身份在仓库里一处不剩
- [x] 全部设计快照与交互期望屏用例照绿
- [x] 按停止到达那一趟、答话到达确认点、预设读写、灰阶测试图的用例经新输入入口照绿
- [x] 命令行报告字节不变，黄金快照原样过
- [x] 三条闸门跑满；`cargo xtask polish` 过
- [x] `CONTEXT.md` 节首那一句与 ADR 0019 的状态改好

## 落地记录

**本票做了什么。** 无参数进来的真会话从这一票起进新界面：`terminal::drive` 每一帧 `tick` → `watch_the_run` →
`shell::draw`，收到的键经 `translate_input` 交给 `input`（窗口尺寸当场量），之后照旧问确认点、`reap`。
旧的一副整个删掉：`press` 与它那几支（展开、展开一枝、旧预设四支、旧灰阶测试图）、`src/session/draw.rs` 连同
`draw/` 十四个模块（左栏、预设栏、总览、报告区、目录表、卷表、逐页表、覆盖层——含「这一趟的前提」——屏底、
键位、让位、探针）与它们的快照用例；`state.rs` 里旧焦点那一维（`Focus`、`Overlay`、`Covered`、`KeyGroup`、`Pane`、
`Follow`、`Values`、`Picker`、`Naming`、`Expansion`、`Edit`、`Notice`、`Action` 与整张旧按键表）、`Layer`、`Step`、
`Field` 上路径与输出那三行；`live.rs` 里旧的卷身份（`Volume`、`Branch` 及其取值器）与只给旧报告区的坏页清单。
留下来的两样搬进新画法：颜色（`shell::paint`，只剩新那一套）与设计快照读法（`shell::design`，吸收了探针的两个读法）；
环节名与时长写法并进 `shell::marks`，代表页那一列的取法进 `shell::list`。
`Live` 里一卷那一份报告改按**卷根**认（`report_at`、`undone_at`）。视口那张表换成七处新块；
`columns` 只剩树与每页结果两张表，字形宽度那一关跟着改从它们导出。

**够着真一趟的用例只留输入入口那一份**：按停止、三种答话、预设读写、灰阶测试图原本就在 `terminal::redesign`；
旧模块里另三条失败路径（图写不出去、会话存的预设命令行读得懂、`dd` 两下之间那一份没了）改写成经 `input` 的版本。
状态机那一层能不经界面问的（请求拼法、取值环、标定覆盖、提白上限、闩、确认点）改成直接问 `Session` 的方法。

基线上 `c_draws_a_calibration_chart_and_says_where_it_landed` 就是红的（期望屏没照屏底宽度截路径），本票修了用例，
屏底一格没动（Q968）。

**文档**：`CONTEXT.md` 只删《会话》节首那一句；ADR 0019 状态改成已落地；`session`、`shell`、`state`、`terminal`、
`live`、`columns`、`viewport`、`tone`、`look` 的模块文档换成新块的名单。词条与 ADR 里另几处还指着旧画法，没顺手改（Q966）。

**票面有没有说全根因。** 阻塞边成立：09／12／14 把新界面每一件接到了 `input` 上，切换只剩那条循环。
验收框全勾完之后还活着的是：鼠标（`Input::Wheel`／`Click` 终端层不翻译，挂着 `expect` 等 16）、
以及随旧界面一起消失的几道机械闸（Q962、Q963、Q965）。

**停车场。** Q961–Q968。

### 数

`cargo xtask gate` 三条全绿（`EXIT=0`，日志 `sr15-gate.log`）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（`target`） | 合计 981 通过 0 失败；lib 239 / bin 401 |
| 2 | `cargo test --no-default-features`（`target/gate/no-default-features`） | 合计 870 通过 0 失败；lib 239 / bin 290 |
| 3 | `cargo check --features profiling`（`target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 27.36s` |

基线 `aae2794`：bin 587（其中 1 条红，见上）。bin 少 186 条，全是旧界面的用例：`session::draw` 135 条、
`state` 52→16、`terminal::tests` 18→3，外加 `live`、`columns`、`complete`、`single_source` 各删几条；
`terminal::redesign` 65→68（添三条失败路径），`shell` 设计快照 14 条一条不少。lib 239 一条没动。
黄金快照 `tests/golden.rs` 两条原样过；`git diff aae2794 -- tests/fixtures/design .scratch/session-redesign/*.html` 为空。

`cargo xtask polish` 四条全绿：fmt 过；两趟 clippy 告警 0；`cargo doc` 告警 15 条（与基线同数）。
`grep -c '^warning'` → 16，除那一行合计之外全是 `links to private item`。
