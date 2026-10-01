# 11 — 退出时的 stdout 与灰阶测试图那条路径

**What to build:** 两条会话 bug：

- 退出会话时印到 stdout 的**整份报告**先过命令行那一路同一道「印出去之前把不许断的空格换回普通空格」
  （《字形约定》），不折行——stdout 可能是一个文件，折到多宽另有一笔账（Q584）；
- 灰阶测试图那句回话里的路径摆不下时**从中间省略**（原样那一档的规矩），目录的头与文件名两头都在；
  设计稿同一条规矩（Q968）。

**Blocked by:** 01

**Status:** resolved

- [x] 会话退出时留在 stdout 上的那一份，报告正文里一个不许断的空格都没有；与命令行那一路读同一个纯函数，在 `render` 那几条问「stdout 上的命令敲得通」的用例旁边断言
- [x] 灰阶测试图回话的路径摆不下时从中间省略，文件名在屏上；那条用例不再按屏底宽度截期望（「文件名在屏上」读作文件名的尾在屏上，Q1207）
- [x] 设计稿的屏底同一条省略规矩，导出一串长路径的比整屏且绿；`npm run check` 绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q584 — 会话退出时印到 stdout 的**报告正文**既不折行、也不把标注换回普通空格

- **From:** 票 `p4-parking-lot/21`
- **Kind:** 路过发现的缺陷（不是本票造的，但本票把最需要抄的那句话送进了这一路）
- **Where:** `src/session/terminal.rs` 的 `enter`（那一句 `print!("{report}")`）；
  对照 `src/main.rs` 的 `execute`（`wrap::folded_text(..)`）与 `main`（`wrap::printed(..)`）
- **Why it did not block:** `wrap::printed` 的文档明写着「标注是给折行看的，不是印出去的东西……
  **不走折行的那一处自己过**」，而会话这一路两件都没做：`enter` 直接 `print!` 那一份报告。
  于是 stdout 上留下的 `--fit\u{a0}height` 带着一个 clap 认不出的字符，
  用户从 `tonefit > 报告.txt` 里抄出来的命令敲不通——Q106／Q183 要买的东西在这一路正好反了。
  **这不是本票造的**：报告抬头那几句互锁本来就带着标注，走的也是这一句 `print!`。
  本票只是让**那句拒绝**也从这一路出去，而它正是这几段字里唯一一句「劝人照着敲」的话。
  折行同理：命令行那一路折到终端宽，这一路一格不折。
- **What this ticket actually did:** **只把本票新铺的那一段补上了，报告正文没动。**
  `render::plain::undone` 里那句没做成的话过一遍 `wrap::printed`——它是这几段字里
  唯一一句「劝人照着敲」的话，而 `CONTEXT.md` 的**字形约定**把「印出去之前换回普通空格」
  写成了库的公共 API。钉着它的用例是 `render::tests::
  what_is_left_on_stdout_spells_its_commands_with_a_plain_space`（摘掉那一步当场红）。
  **报告正文那两段没动**：票面的硬约束是「变的只有那句拒绝」，而 `enter` 那一句管的是
  整份报告，顺手加一层会把抬头里那几处标注一起换掉——那是票面之外的行为改变。
  留下的因此是**报告正文那一半**，加上折行那一整笔。
- **Options:** ① `enter` 改成 `print!("{}", wrap::printed(&report))`——把报告正文那一半
  也补上，折行留着不动（会话没有一个「印在多宽的地方」的出处，`stdout` 可能是个文件）；
  ② 连折行一起补，走 `wrap::folded_text`，宽度取会话进来时问到的那一个；
  ③ 什么都不做——那句最要紧的话本票已经补上了，剩下的是抬头里那几处记号。
- **Recommend:** ①。标注这一半是**正确性**（抄出来的命令敲不敲得通），两条路上答案该一样，
  而本票只够得着自己新铺的那一段；
  折行那一半是**排版**，而重定向出去的那一份该折到多宽本来就另有一笔账
  （`p4-parking-lot/05` 记着「重定向出去时折到一个定值」），不该在这里顺手定。
- **Whose call:** 拍板的人（会话那一路的 stdout 要不要与命令行那一路对齐）
- **处置：** **本票了结，照推荐 ①**：整份退出时那一份过一遍 `wrap::printed`（`render::plain::left_on_stdout`，做成了与没做成两支都过），不折行。见《落地记录》。

#### Q968 — 灰阶测试图那句回话在屏底从行尾截断，截掉的正是文件名

- **From:** 票 `session-redesign/15`
- **Kind:** 屏上的字与它要答的那件事对不上
- **Where:** `src/session/shell/footer.rs`（回话摆到倒数第三列为止，摆不下的从行尾截掉）；
  `src/session/terminal.rs` 的 `draw_a_chart` 那一句「✓ 已生成灰阶测试图（WxH）：写到 {路径}」；
  用例 `c_draws_a_calibration_chart_and_says_where_it_landed`
- **Why it did not block:** 图照样写对了地方，回话前半截也对；只是路径长时（这台机器上用例的临时目录就长）
  屏上看到的是截到一半的目录，`tonefit-calibration-<型号>-<级数>-levels.png` 那一截看不见。
  这条用例在基线 `aae2794` 上就是红的——期望屏照整句填满一行，没照屏底的宽度截。
- **What this ticket actually did:** 用例里的期望照屏底那一格的宽度截（摆到倒数第三列），不再依赖临时目录多长；
  屏底的画法一格没动。
- **Options:** ① 就这样；② 那一句的路径**从中间省略**（`columns::elide`，原样那一档的规矩），
  两头都在——目录的头与文件名；③ 回话只说文件名，完整路径另想去处
- **Recommend:** ②。「图在哪儿」的答案是那条路径，而路径里最要紧的是末一段；原样那一档本来就归从中间省略管。
- **Whose call:** 下一张碰屏底或灰阶测试图的票
- **处置：** **本票了结，照推荐 ②**：路径照 `columns::elide` 从中间省略，摆得下多少照屏底那一格（`shell::footer::room`）算；设计稿同改，导 `config-narrow-c` 比整屏。见《落地记录》。

## 落地记录

**本票做了什么。** 两条会话 bug 各修一处：退出时那一份整份换回普通空格（实现）；灰阶测试图那条路径摆不下时从中间省略（先改设计稿、重导，再改实现）。

| 件 | 设计稿（`design.html`） | 导出（`export.js`） | 实现 |
|---|---|---|---|
| 退出时那一份（Q584） | — | — | `render::plain::undone` 改名 `left_on_stdout`、多收做成了那一支（`why_undone: Option<&str>`，做成了时先前那一份不印，与原先 `Running::report` 那一支同），**整份**过一遍 `wrap::printed`（原先只过没做成那一句）；不折行。`session::run::Running::report` 只答取哪三段，交给它 |
| 灰阶测试图那条路径（Q968） | 顶层 `footerRoom`（`W - 3`，`drawFooter` 改读它）；`configKey` 的 `c` 那一支照它算路径摆得下几格、`elide(chartPath(), room)` | `config-narrow-c`（`config` 那一景、80×24、按 `c`） | `shell::footer::room`（`pub(in crate::session)`，`footer` 模块随之 `pub(super)`）；`terminal::draw_a_chart` 多收 `window`，照 `room(window.cols)` 减掉前两截的宽、`columns::elide` 省那条路径 |

- **`draw_a_chart` 的文档**（`design-parity/01` 托付）：末段「屏底那一句与设计稿不同……（原型不写文件）」那一段删掉，换成现在的事实——与设计稿逐字相同、摆不下从中间省略、照这一刻的窗口算。
- **「那条用例不再按屏底宽度截期望」**：`c_draws_a_calibration_chart_and_says_where_it_landed` 自 `design-parity/01` 起已经比整屏（`config-c`），本票没再动它的断言，
  只把列落点那一段抽成 `charts_landed` 与新用例共用。
- **省略的样子**与验收第二格的读法：验收线上按哪一副省都摆不下整个文件名，那一格读作文件名的尾在屏上；数与另一条路在 Q1207。
- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过：既有快照、场景数据、期望屏**一个字节没变**；只有 `manifest.json` 添一条（+25 行），新文件是 `config-narrow-c` 的三份。
  头一遍只改设计稿、不添那一串时重导，产物一个字节都没变（`config-c` 那条路径 120 列上摆得下）。node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 全过。
- **用例**：
  - `render`：新 `the_report_left_on_stdout_spells_its_commands_with_a_plain_space`，紧挨着 `what_is_left_on_stdout_spells_its_commands_with_a_plain_space`——
    报告抬头带互锁 ①（`--fit height`）的一份真报告，做成了那一支与那一份过 `wrap::printed` **逐字节相同**（同一个纯函数、一行不折），没做成那一支一个标注都不剩；
    没做成的那句话取互锁 ③ 那条拒绝的原话；旁边那一条改调 `left_on_stdout`。
  - `terminal`：新 `a_chart_path_that_does_not_fit_is_elided_in_the_middle`（`config-narrow-c` 比整屏；省略号两边各是盘上那张图路径的头与尾）。
- **停车场 Q1146**（`C-c` 退出时 stdout 那份末尾写「按停止停下（立即停止）」）：措辞一个字没变，本票没碰它。
- **`CONTEXT.md`**：没动。《退出时那一份》说「照命令行那一路的原格式」，《字形约定》说「印出去之前换回普通空格」，两条现在都成立。

### 按反跑过的几遍（每一遍改一处、跑 `cargo test --bin tonefit <过滤>`、还原，还原后 `git diff --stat` 核过）

| 按反 | 结果 |
|---|---|
| 实现之前（重导 `config-narrow-c` 之后、`draw_a_chart` 没改） | 红：`config-narrow-c` 第 23 行第 58 格，期望「⋯」实际「o」（从行尾截） |
| 实现之前（只换没做成那一句） | 红：`the_report_left_on_stdout…` 的逐字节比，实际带着 `--fit\u{a0}height` |
| 做成了那一支不过 `printed` | 红：`the_report_left_on_stdout…` |
| 没做成那一支整份不过 `printed` | 红：两条 stdout 用例都红 |
| 做成了那一支也把先前那一份印出来 | 红：`the_report_left_on_stdout…` 与 `a_refused_run_does_not_take_the_earlier_pass_off_stdout` |
| `footer::room` 多给一格（`W - 2`） | 红：`config-narrow-c` 第 23 行第 58 格，期望「⋯」实际「o」 |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 895121f`（未提交的工作树）加新文件。

**收下的**：

- **新用例写了 `SplitRule::default()`**（Standards，testing.md《用例点名取值，不借默认值》）：改成点名 `on: true`（互锁 ① 咬上靠的是它）。
- **两条用例同一句手写的「没做成」**（Standards，Duplicated Code）：新的那一条改取互锁 ③ 那条拒绝的原话（`Interlock::DitherOutsideTheGate`）。
- **做成了那一支丢掉先前那一份，用例没钉**（Standards，Data Clumps 那一条的实质）：用例在做成了那一支也递一份先前的，逐字节比只剩这一趟那一份；
  文档写明 `earlier` 这时不印、`Running::report` 那句「只答取哪三段」改成「只交出那三样，用不用、怎么接、怎么印在 `left_on_stdout`」。
- **`on_exit` 像个事件钩子、参数 `undone` 与 `super::undone` 重名**（Standards，Mysterious Name）：改名 `left_on_stdout`、`why_undone`。
- **宽度一半 `Segment::width` 一半 `wrap::width`**（Standards）：`draw_a_chart` 先摆前两截、`width_of` 量，路径作第三截接上（与第二截同一副样子，屏上逐格不变）。
- **局部变量 `room` 遮住了同名函数**（Standards）：`footer::draw` 里那个改叫 `space`。
- **两条灰阶测试图用例各取一遍回话**（Standards，Duplicated Code）：收进 `said_to_land_at`。
- **数写了三遍**（Standards，CLAUDE.md 文档写作「单一出处」）：路径与那一格的宽只留在 Q1207，本记录与 Q1209 指过去；Q1209 不再写「十三景」那个会变的数。
- **Q1207 的 Kind**（Standards）：改成「验收条读着含糊」；验收第二格的读法写进那一格本身（Spec：「文件名在屏上」只做到尾）。

**驳回的**：

- **验收第二格该等 Q1207 拍板再勾**（Spec）：照 `design-parity/08` 那一格（Q1190）的先例，按写明的读法打勾、岔口记停车场；
  验收线上按哪一副省都摆不下整个文件名，那一格的字面在 80×24 上本来就不可能成立，等拍板换不来别的屏。
- **「目录的头」那一半没有一屏单独钉着、`cwd` 没换长**（Spec）：已记 Q1209；`config-narrow-c` 的头上是 `~/`，就是那一屏的目录的头。
- **`draw_a_chart` 伸进 `shell::footer::room`**（Standards，Feature Envy）：另一条路是回话带一截「原样」、屏底每一帧省它，已记 Q1208；
  敞开的只有 `room` 这一个数（`footer` 模块 `pub(super)`、`room` 本身 `pub(in crate::session)`，`draw` 照旧只给 `shell`）。
- **`left_on_stdout` 改成一个枚举**（Standards，Primitive Obsession）：它只有一个调用方，`Option` 的两支就是《退出时那一份》那两种；为它立一个类型不划算。
- **「clap 认不出的字符」那条理由在三处注释里各写一遍**（Standards）：两处是既有的；新写的那一处（`plain`）没写它。

### 停车场

本票用了 Q1207–Q1216 里的三个（Q1210–Q1216 留空）：

- **Q1207**：路径照卷名那一副对半省，验收线上型号的前半跟着省掉；推荐照现在（同一副省略法）。
- **Q1208**：省到多宽在按下 `c` 那一刻定，那 3.2 秒里改窗口不跟着重省；推荐照现在。
- **Q1209**：「一串长路径」用验收线的窗口钉，不换长 `cwd`；推荐照现在。

### 数

评审收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-11.gate1.log`、`dp-11.gate2.log`、`dp-11.gate3.log`、`dp-11.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1103 通过 1 失败**；lib 252 / bin 454；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 75.46s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **970 通过 1 失败**；lib 252 / bin 321；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 87.18s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `895121f`，没在它上面重跑。本票新添两条用例：`render` 那一条不挂特性（两趟都多一条），`terminal` 那一条在 `tui` 后面（只闸门 1 多）——
bin 闸门 1 多 2 条、闸门 2 多 1 条。

**黄金快照逐格没动**：`git diff 895121f -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。
