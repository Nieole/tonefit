# 01 — 把画法按「画什么」拆开

**What to build:** 纯搬家。会话的画法眼下是一个一千八百余行的模块，而本批要往里加卷表、
取值栏、焦点与两张覆盖层——先按「画什么」把它拆开，再往里加。否则一个三千行的文件里
没人找得到砍列的那一处出处。

**不是重写**：措辞、布局、每一个常量原样搬，只换它们住在哪儿。分法按**屏上那几块**走
（左栏、主区那几段、屏底、预设栏），不按「工具函数 vs 业务」那种切法——后者一年后
没人分得清一个函数该归哪一边。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 快照用例**一个字节都不变**，不必重录
- [x] 拆出来的每一块有模块文档说清它画的是屏上哪一块
- [x] 没有一处措辞、常量或布局数在搬家中被改动；横条的宽度与命令行那两条仍是同一个出处
- [x] 拆完之后单个模块不超过大约六百行
- [x] 折行仍旧只走界面层那一套，画法这一层只交代「折到多宽」
- [x] 三条闸门全绿

## 落地记录

### 拆成五块画法加一处探针，一块一个「屏上那一块」

`src/session/draw.rs` 从 1983 行拆成一个父模块加六个子模块（`src/session/draw/`），
五个画屏上那五块，第六个只装用例共用的探针：

| 文件 | 行 | 画的是屏上哪一块 |
|---|---|---|
| `draw.rs` | 462 | **骨架**：整屏与主区怎么分格（`shell`、`main_pane`、四个布局常量、`config_width`、`footer_height`），外加那张「屏上那几块各住在哪儿」的表 |
| `draw/config.rs` | 116 | **左栏**：三层配置常驻的那一栏 |
| `draw/picker.rs` | 208 | **预设栏**：占主区、左栏照旧在场的那一副 |
| `draw/bars.rs` | 259 | **全局条**与**当前卷条**：主区上面那两条横条 |
| `draw/report.rs` | 482 | **报告区**：主区第三段，两副样子（默认／展开） |
| `draw/footer.rs` | 492 | **屏底那几行**：按键提示、说明与要说的那句话 |
| `draw/probe.rs` | 165 | 那几块共用的**测试探针**（`#[cfg(test)]`） |

分法按**屏上那几块**走，票面否掉的「工具函数 vs 业务」一刀没切：`bar`、`spell`
跟着两条横条走（它们只画横条），`rows`、`widest`、`folded_lines`、`past_the_top`
跟着报告区走（它们只算报告区的滚动量与折行宽度）。

**布局留在父模块**：`shell` 与 `main_pane` 都只做一件事——`Layout::…` 把屏分成格子，
再把每一格交给它的模块。父模块因此答得出「哪一格多宽多高、放不下时让谁」，
而答不出任何一格里画的是什么。整棵仍在 `tui` 后面（`mod draw` 那一行没动），
`src/session.rs` 的《终端库在哪一半》添了一句：新拆一块不必再挂一次 `cfg`。

### 用例跟着它问的那一块走，探针摆一处

一千零四十行用例跟着代码一起拆——光挪代码达不到「单个模块不超过大约六百行」。
每一条按**它的题目**归位：`the_overall_bar_says_how_the_run_ended` 去横条那一块，
`the_expanded_report_scrolls_sideways…` 去报告区，`the_preset_column_…` 去预设栏。
跨着几块问的那四条（整屏三层三段、决策点那一屏、两张主区快照、窄终端那一档）
留在父模块——它们钉的正是布局。

九个探针（`tight`、`screen`、`snapshot`、`snapshot_of`、`main_snapshot`、
`reversed_rows`、`reversed_cells`、`a_run_in_flight`、`same_screen`）摆进
`draw/probe.rs`：屏上取回来的文字有两条各不相同的读法（逐格拼的那一条每个汉字后面
多一个空格，快照那一条走终端库自己的 `Display`），抄第二份就会有一份抄漏。
只有一块用得上的夹具留在原处（`a_run_worth_expanding`、`expanded` 在报告区那一块，
`picking`、`preset_snapshot` 在预设栏那一块）。

### 可见性：跨块用得着的才 `pub(super)`

拆之前一个文件里全是私有。拆之后升成 `pub(super)`（可见范围就是 `draw` 这一棵）的
只有真跨块被读的那几样：`config`、`presets`、`overall_bar`、`volume_bar`、
`report_pane`、`report_title`、`footer`、`BAR_HEIGHT`、`START_KEYS`、`stopping_name`、
`expandable`（`report_title` 是被屏底那一块的文档指着的，指路要指得到）。
`opens_at` 是唯一出 `draw` 的那一个（`terminal.rs` 换卷时对视口），它写成
`pub(in crate::session)` 并由父模块 `pub(super) use report::opens_at;` 转出来——
调用处仍是 `draw::opens_at`，一个字没改。

`Prompt` 与它的 `keys`／`what` 两个字段也升成了 `pub(super)`，那是一条**跨两块的用例**
顶开的（`the_overall_bar_says_on_its_title_that_the_run_is_stopping` 在横条那一块里读
屏底的 `running_prompt(..).keys`，验的是「措辞只有一处出处」）——记在 **Q129**。

### 一个字节都没改的是哪些

措辞、常量、布局数、快照——**一条快照都没重录**。逐行核过：新旧两边去掉缩进之后
对不上的只有可见性关键字、`use`、`mod tests` 的壳、rustfmt 的重排，以及下面那一类
指路。`const BAR_WIDTH: u64 = crate::BAR_WIDTH as u64;` 原样搬进 `draw/bars.rs`，
横条的宽度与命令行那两条仍是同一个出处；折行仍旧只走 `crate::wrap`
（报告区与屏底两处调 `wrap::fold`／`wrap::width`），`src/wrap.rs` 的**折行一个字节没碰**。

### 指路跟着搬

文档里指名道姓写着旧路径的那几处跟着改（**只改路径，一句措辞都没动**）：
`src/session/state.rs` 三处、`src/session/live.rs` 两处、`src/wrap.rs` 两处
（`crate::session::draw::folded_lines` → `…::draw::report::folded_lines`）。
`src/session/terminal.rs` 那两处指的是 `draw::opens_at`，转出来之后仍然成立，没动。
画法内部原来写 `super::X`（X 在 `session` 那一层）的文档按深度补了一级；
`super::press`／`super::expand` 那几处**没有**改成准确的 `terminal::press`——
那是票面不许做的「顺手能改好」，记在 **Q128**。

父模块的模块文档里另有两句是搬家逼着改的，一并记在这里：「长在**本模块**的只有……
那两样」改成「长在**画法这一层**的……」（那两样眼下住在两个子模块里），
`[`Wrap`]` 补成 `[`Wrap`](ratatui::widgets::Wrap)`（父模块不再 `use` 它，
不补就指不到）。两句说的事一个字没变。

### 数

三条闸门按 1→2→3 的次序跑满，三条都绿（各自 `exit=0`）。本票**一条用例都没加、没删、
没改写**，三条的通过数因此与落地前逐格相同。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **637 通过 0 失败**；lib **209**、bin **178**，`tests/` 各文件 11/40/12/21/5/2/19/22/11/80/6/6/8/2/5 |
| `cargo test --no-default-features` | 同上（Doc-tests 那一格） | **598 通过 0 失败**；lib **209**、bin **139**——与落地前同一个数，画法那一棵仍整个在 `tui` 后面 |
| `cargo check --features profiling` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.24s` | 干净 |

**闸门之外那一遍**：`cargo fmt --check` 干净；`cargo clippy --all-targets` 与
`--all-targets --no-default-features` 两遍都零告警；`cargo doc --no-deps` 仍是
**15 条告警**，一条没多（那 15 条全在 lib 那一侧，本票一个字都没动库；bin 与 lib 同名，
cargo 跳过它的文档，画法这一层的文档链接因此从来不进这个数）。

### 停车场

- **Q128**（待处理）：画法里那几处 `super::press`／`super::expand` 指的其实是
  `terminal::press`／`terminal::expand`（仓库通例，十来处）。本票按搬家的深度补了一级，
  没有改成准确路径——那是票面不许做的「顺手能改好」。
- **Q129**（了结）：`the_overall_bar_says_on_its_title_that_the_run_is_stopping` 跨着
  横条与屏底两块问「措辞只有一处出处」，把 `Prompt` 与它两个字段的可见性顶成了
  `pub(super)`。纯搬家不许改用例，因此按可见性放开那一条走；`Prompt` 出不了 `draw` 这一棵。
- **Q130**（待处理）：拆完之后仍有两样东西跨着两块——两张主区快照钉的是报告区正文却
  跟着 `main_pane` 住在布局那一块，`START_KEYS` 与 `stopping_name` 是两块共用的措辞却
  只能寄在屏底名下。两样都照原样搬，留给下一张动主区的票（总览与卷表）连同快照一起重定。

评审两轴各跑了一遍：Spec 轴无缺项、无做错的；Standards 轴揪出的两处指路（`report_title`
私有指不到、`probe` 挂着 `cfg(test)` 不该当链接）与 `probe.rs` 模块文档说不准的那一句
已就地改掉，其余两条判断题落成 Q130。`src/session.rs` 上原先多写的一句规矩
（「不许把只有画法读得到的东西搬到另一半去」）收回了——那句 `docs/agents/gate.md` 已有，
是重复立规。

### 停车场结转

### Q129 — 拆开之后有一条用例跨着两块，把 `Prompt` 与它两个字段的可见性顶开了

- **From:** 票 `p3-session-legibility/01`
- **Kind:** 票面没想到的第三种情形（分块之后才看得见的耦合）
- **Where:** `src/session/draw/bars.rs` 的
  `the_overall_bar_says_on_its_title_that_the_run_is_stopping`，末尾那一段读的是
  `src/session/draw/footer.rs` 的 `running_prompt(..).keys`
- **Why it did not block:** 那一段问的是「全局条的抬头与屏底那一行用的是同一个
  `stopping_name`」——**它本来就该跨两块问**，措辞只有一处出处正是它要钉的东西。
  而本票是纯搬家，一条用例都不许改写。
- **What this ticket actually did:** 用例照它的题目（全局条的抬头）留在 `bars.rs`，
  `Prompt` 与它的 `keys`／`what` 两个字段从私有升成 `pub(super)`——拆之前它们同在一个
  文件里，私有就够。`running_prompt` 与 `stopping_name` 同样是 `pub(super)`：
  前者这条用例要，后者全局条的抬头本来就要。
- **Whose call:** 无（真要把 `Prompt` 收回私有，得把那条断言拆成两半，而那是改用例）
- **处置：** 了结。可见性按上面那条放开，用例一个字没改；`Prompt` 出不了 `draw` 这一棵。
