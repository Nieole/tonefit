# 21 — 先前那几趟的报告照印，互锁 ③ 那条拒绝按页分岔

**What to build:** 两笔命令行措辞与收场的账：

- **最后那一趟没做成时，先前那几趟试算的报告也就印不到 stdout 了。**
  用户连已经算出来的东西一起丢掉。收场那一路要把攒下来的照印，
  再报最后那一趟为什么没做成；
- **互锁 ③ 那条拒绝一次把两条出路都说了**，而它其实**按页分岔**——
  说清是**哪一页**不成立，用户才改得动它。「拒绝时说什么」是措辞，
  仍在措辞那一层出，不在命令行那一头另写一份。

收停车场的 **Q66**、**Q102**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 最后一趟没做成时，先前那几趟的报告照旧印到 stdout
      （取的是**最近一份做成了的**，不是每一趟各一份——理由与备选见《落地记录》与停车场 **Q581**）
- [x] 那一趟为什么没做成也说得出来，两者分得开
- [x] 退出码仍照既有那四个走
- [x] 互锁 ③ 那条拒绝说得出是哪一页不成立
- [x] 拒绝的措辞仍只有一处出处
- [x] 钉着那句拒绝的几处逐处核过，**变的只有那句拒绝**——
      `src/interlock.rs` 自己的用例、`tests/pipeline.rs`、`tests/events.rs`、
      以及会话那两处（报告区与覆盖层）。
      **黄金快照够不着它**：`tests/golden.rs` 自己拼快照行，而它的夹具触发不到拒绝
      （`grep -c 互锁 tests/golden-snapshot.txt` 是 0）——这一条本来写着「黄金快照重录一次」，
      与 Q132／Q181／Q184 是同一个形状，落地时把这次核实写进《落地记录》
- [x] 三条闸门全绿

## 落地记录

### 〇、动手前核过的两笔账（前车之鉴那一条）

派活说明要求先核一眼两笔账今天的真实状态，别做已经做完的事。**两笔都原封不动**：

| 账 | 今天的状态（基底 `06cd5bd`） |
|---|---|
| Q66（收场照印） | `src/session/run.rs` 的 `Running::report` 仍是 `if live.undone().is_some() { return None }`——没做成就一个字节都不印 |
| Q102（按页分岔） | `dither_outside_the_gate_error(fit: FitMode)` 仍按**这一趟的适配方式**分两支、在碰卷之前备好、两条路的例外一次全说；`Interlock::voice` 仍是三条处置的唯一出处，没有按页分岔的痕迹 |

22 号票撞上的那种「票面第一句已经不成立」在这张票上没有发生。

### 一、互锁 ③ 那条拒绝按页分岔（Q102）

**判据只有一问：换成以高为准之后，*这一页*的门成不成立。**那一问住在**几何那一层**
（`src/geometry.rs` 的 `holds_by_height`）——门仍由 `GeometryGate::of` 判、目标尺寸仍由
`FitMode::target` 算，两者各自的唯一出处一格没动，这里只是把「换一条路会怎样」
摆到它该在的那一层。问它的是 `src/lib.rs` 的 `Candidates::for_gate`：

```
Refusal(dither_outside_the_gate_error(geometry::holds_by_height(source, panel)))
```

`for_gate` 收源尺寸与面板而不是收一个算好的布尔，为的是让**门成立那一支一分钱都不花**。

这一问统一了两种适配方式，`fit` 那个入参因此不再需要：以高为准上让每一页的高都等于面板高，
走得到这条拒绝的**只能是**被兜底上界退回 fit-inside 的页（07 号票开的唯一例外），
`by_height` 在它身上恒不成立——与从前 `FitMode::Height` 那一支说的是同一件事，
只是从「这一趟点的是哪个开关」换成了「这一页的几何」。

**措辞仍只有一处出处**，一格没搬：规则那一句仍是 `Interlock` 的 `Display`
（`src/interlock.rs`，一个字没动），出路那一句仍是 `src/lib.rs` 的
`dither_outside_the_gate_error`。命令行那一头没有第二份。变的只有它两支各说什么：

- **门跟着成立**：只说 `--fit height` 那条出路。那道「兜底上界退回去的页是例外」
  **不再说**——对这一页它不成立，从前够得着出路的人也得先读一遍与他无关的例外。
- **门仍不成立**：只说这一页是怎么走到这儿的（目标尺寸越过兜底上界、被退回 fit-inside），
  以及剩下那两条路。**不再劝人换 `--fit height`**——对这一页那是假话。

**那句话不再在碰卷之前备好。**`Candidates::broken` 从 `Result<Vec<Candidate>>` 换成
`Option<Vec<Candidate>>`：那一格只答「裁空了没有」，对用户说什么由 `for_gate` 在页上现造。
`Candidates::new` 里 `.ok()` 丢掉的那个错误是**规则那一句**（`why_nothing_is_left`
抖动那一支现在只回它）——位深那一维在上一行就拦下了，走到那里的 `Err` 只可能是互锁 ③。
两支说得出的话因此不一样全，那不是漏：面板灰阶数是**这一趟**的事实，几何门是**页**的事实。
半截话没有用户看得到，记进停车场 **Q583**（附推荐：换成枚举，但那要动
`the_refusal_is_driven_by_this_interlock_alone` 的形状，值得单独一趟）。

`pinned_up_front` 里那个 `match &candidates.broken` 跟着从 `Err/Ok` 换成 `None/Some`，
语义一格没动。

### 二、收场把攒下来的照印（Q66）

`Running` 多一格 `settled: Option<String>`——**最近一份做成了的那一趟的报告**，
在做成的那一趟收场时记下（`collect`）。没做成的那一趟更新不了它，
`report()` 因此拿得到的恒是**先前**那一份，与这一趟攒下来的不会是同一段字。

`Running::report` 这一层**只答「取哪三段」**，怎么接不在它那里——那是纯文本那一副的摆法
（`render::plain::undone`，ADR 0016 决定第 3 条明写「会话退出时留在 stdout 上的」走那一副）。
三段按次序：

1. **先前那一份做成了的报告**（`settled`，有才印）；
2. **这一趟攒下来的那一份**——抬头那几件事从 `Request` 上就答得出，因此它恒有内容，
   那正是「这一句拒绝是哪一套参数撞出来的」；互锁 ③ 那种拒绝要真撞上那一页才拦得住，
   先做完的卷已经在盘上、也已经在这一份里；
3. **这一趟为什么没做成**（`render::plain::undone`）。

**做成的那一趟一格没动**：仍是 `render::plain::report(live.report(), live.mode())` 一份，
与落地之前逐字相同。**退出码一格没动**：仍取最后那一趟，四个数一格没挪。

**两条缝两种待遇，都在 `plain::undone` 一处说了算**：报告与报告之间不加任何东西
（两份各自都以换行收尾，与 `plain::report` 那四段同一条），那句话前面**空一行**——
它不是报告的一部分，读的人要分得开。用例断言 `printed.starts_with(&worked_out)`，
再在 `&printed[worked_out.len()..]` 上找那句话。

**那句话印出去之前把标注换回了普通空格**（`wrap::printed`）：它劝人换一条命令，
而记号中间那个空格带着[不许断的标注](`tonefit::HARD_SPACE`)，`CONTEXT.md` 的**字形约定**
把「印出去之前换回普通空格」写成了库的公共 API——而这一路不走折行。**只过这一段**，
报告正文那两份仍与落地之前逐字节相同（剩下那一半记在 Q584）。

**那一格不叫 `settled`，叫 `earlier`**：`render::Listed::Settled` 已经占着那个词，
指的是**收摊了的那一卷**，同词两义读起来是两回事。

**那句话的措辞也收成了一处**：`render::undone`（措辞）+ `render::plain::undone`（摆法，
前后各空一行）。会话主区收场之后的抬头（`session::draw::overview::ended_title`）
从前自己 `format!("这一趟没做成：{said}")`，现在取同一处——屏上与 stdout 上是同一句。

「先前那**几**趟」取的是「最近一份做成了的」而不是「每一趟各一份」，
理由与备选记进停车场 **Q581**；零卷那一份也照印，记进 **Q582**。

### 三、钉着那句拒绝的几处，逐处核过（验收第 6 条）

| 哪一处 | 核的结果 |
|---|---|
| `src/interlock.rs` 自己的用例 | **一个字没动。**`Interlock` 的 `Display`（规则那一句）、`voice`（三条处置）、`ALL`、`engaged`、`dither_outside_the_gate` 全部原样；五条用例原样跑绿。`the_refusal_is_driven_by_this_interlock_alone` 仍成立——它问的是「说不说得出话」，而抖动那一支照旧只在互锁 ③ 咬上时说话 |
| `tests/pipeline.rs` | **两条跟着改，改的正是它们钉的那句话。**`a_dither_the_geometry_gate_forbids_is_refused`（`SMALLER_THAN_TARGET` + fit-inside，**够得着**出路）加了两条断言：不再带「兜底上界」与「仍是这条拒绝」那道与它无关的例外。`on_the_default_fit_the_refusal_stops_offering_a_fit_mode_that_changes_nothing`（`DEGENERATE_STRIP_SMALLER_THAN_PANEL`，两种适配方式下都**够不着**）改名 `on_neither_fit_does_the_refusal_offer_a_fit_mode_that_changes_nothing`：fit-inside 那一侧也不再提那个开关，两条路上 `assert_eq!` 逐字是同一句 |
| `tests/events.rs` | **一个字没动**，12 条原样绿。`a_refusal_after_the_run_started_still_says_the_run_is_over` 断言的是「几何门」三个字与收场那条事件，两者都没变 |
| `tests/exit_code.rs` | **夹具换了一张页**（`solid(DEGENERATE_STRIP_SMALLER_THAN_PANEL)` → `full_bleed_gradient(SMALLER_THAN_TARGET)`）。`the_refusal_on_stderr_spells_its_commands_with_a_plain_space` 要一次问到**两条**命令（`--fit height` 与 `不点 --dither fs`），而按页分岔之后只有够得着出路的页才听得见前一条。断言与它要钉的那件事（记号里那个空格换回普通空格）一格没动 |
| `tests/counters.rs`、`tests/concurrency.rs` | **一个字没动**，13 与 23 条原样绿 |
| 会话报告区（`src/session/state.rs` 的 `Overlay::Premises`） | **一个字没动。**它印的是 `render::header` 那一份——互锁**抬头**（`Voice::Header` 那一条），不是那条拒绝。两者只共用 `Interlock` 的 `Display`，而那一份没改 |
| 会话覆盖层（`src/session/draw/overlay.rs` 的 `premises`） | **一个字没动**，同上一行：`Painted::plain(crate::render::header(..))` |
| **黄金快照** | **重录不了，也不必重录。**`grep -c 互锁 tests/golden-snapshot.txt` 是 **0**，`grep -c 这一趟没做成` 也是 **0**：`tests/golden.rs` 自己拼快照行，而它的夹具一条都触发不到拒绝、也触发不到「这一趟没做成」。黄金够不着这一票改的两句话，因此**这一条是核实、不是重录**（与 Q132／Q181／Q184 同一个形状） |

### 四、断言会不会永远绿——把实现改坏，看它红不红

上一轮那三条同义反复的教训。逐个真验过：

| 改坏哪一处 | 谁红了 |
|---|---|
| `for_gate` 恒答「有出路」 | `on_neither_fit_does_...` 红（`--fit height` 又冒出来了），`a_dither_...` 绿 |
| `for_gate` 恒答「没出路」 | `a_dither_...` 红（`--fit height` 没了），`on_neither_fit_does_...` 绿 |
| **把例外换个说法写回够得着出路那一支** | `a_dither_...` 红（见下一段） |
| `report` 不印 `earlier` | `a_refused_run_does_not_take_the_earlier_pass_off_stdout` 红 |
| `report` 不印攒下来的那一份 | `a_refused_run_leaves_the_session_open_and_still_prints_what_was_worked_out` 红 |
| `report` 不印那句为什么没做成 | 两条都红 |
| `plain::undone` 不过 `wrap::printed` | `what_is_left_on_stdout_spells_its_commands_with_a_plain_space` 红 |

**评审揪出一条真的永远绿的**，已经换掉：`assert!(!said.contains("仍是这条拒绝"))`——
「仍是这条拒绝」那五个字随旧措辞一起从 `src/` 删干净了，全仓只剩这条断言自己在引它，
把 `for_gate` 改坏也红不了。换成一正一反两条：反的问例外在不在（`!contains("兜底上界")`），
**正的问它接着说的是什么**（`contains("门跟着成立。剩下两条路")`）——出路那一句说完直接接
「剩下两条路」，中间插不进第三段。真验过：把例外换个说法（「宽高比极端的页换过去照样过不去」）
写回去，新那条当场红，旧那条一声不吭。

另外两处会空转的也补硬了：`worked_out` 先断言 `starts_with("profile ")`（空串上
`starts_with` 与那个切片恒成立，少了它第一段丢掉也红不了）；先前那一趟那条加了
`printed.matches("profile ").count() == 2`——**两份报告两个抬头**，中间那一段整个删掉
从前照旧绿，而它正是「已经算出来的东西」那一半。

### 五、评审两轴改出来的那几格

`/code-review` 两轴各自独立揪出的，逐条处置：

| 轴 | 发现 | 处置 |
|---|---|---|
| Spec | `assert!(!said.contains("仍是这条拒绝"))` 永远绿 | **换掉**，见上一节 |
| Spec | `worked_out` 若为空串，`starts_with` 与那个切片空转 | **补** `starts_with("profile ")` |
| Spec | 先前那一趟那条抓不住中间那一段 | **补** 两个抬头那一条 |
| Spec | 「先前那**几**趟」只留了一趟 | **不改**，复选框加注、理由与备选在 Q581 |
| Spec | `tests/exit_code.rs` 换夹具不在第 6 条名单里 | **不改**：断言本身没动，是行为逼出来的夹具改，已写进第三节 |
| Standards | `enter` 那一路不把标注换回普通空格（`CONTEXT.md` 的**字形约定**是库的公共 API） | **补了本票新铺的那一段**（`plain::undone` 过 `wrap::printed`），报告正文那一半留在 Q584 |
| Standards | 三段怎么接落在 `run.rs` 里，而 ADR 0016 决定第 3 条说 stdout 那一份是 `plain` 那一副 | **搬**：`plain::undone` 收下三段，`Running::report` 只答「取哪三段」 |
| Standards | `settled` 与 `render::Listed::Settled` 同词两义 | **改名** `earlier` |
| Standards | `for_gate` 拿几何比拿自己的多（Feature Envy） | **搬**：那一问变成 `geometry::holds_by_height` |
| Standards | `why_nothing_is_left` 头上「戴 `Refusal` 由 `candidates` 统一做」已与代码分家 | **改掉那段文档**：位深那一支仍由 `candidates` 戴，抖动那一支由补全它的 `for_gate` 戴，分家的地方写在 `Candidates::new` 那一行 `.ok()` 上 |
| Standards | `earlier` 存渲染文本而非 `(Report, Mode)`（Primitive Obsession） | **不改**：它只有一个读者、原样接出去，而留结构要克隆整份报告；理由写进那一格的文档 |
| Standards | `CONTEXT.md` 没跟上 | **加了一条新词条**《退出时那一份》（`## 会话`）——新概念当场加，不是改写已有词条 |

### 六、停车场

本票开出 **Q581**、**Q582**、**Q583**、**Q584**（块是 Q581–Q590，留下 Q585–Q590 的空洞）。
前三条是这一票自己的岔口；**Q584 是路过发现的**——会话退出时印到 stdout 的那一份
既不折行、也不把[不许断的空格](`tonefit::HARD_SPACE`)换回普通空格。**本票补了自己新铺的
那一段**（`plain::undone` 过 `wrap::printed`，那句拒绝是这几段字里唯一一句「劝人照着敲」
的话），**报告正文那两份没动**：`enter` 那一句管的是整份报告，补一层会把抬头里那几处标注
一起换掉，而票面的硬约束是「变的只有那句拒绝」。留在 Q584 的因此是报告正文那一半，
加上折行那一整笔。
收掉的是 **Q66** 与 **Q102**，两条都已在《已了结》表里指向本票。

### 七、数

三条闸门（`cargo xtask gate`，各用各的 target 目录），**全绿**：

| | 命令 | 合计 | 末行 |
|---|---|---|---|
| 1 · 默认构建 | `cargo test` | **920 通过 0 失败**；lib **244** / bin **362** | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（doc-tests） |
| 2 · 甩掉终端库 | `cargo test --no-default-features` | **789 通过 0 失败**；lib **244** / bin **231** | 同上 |
| 3 · 开着量具 | `cargo check --features profiling` | —— | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.94s` |

两条测试闸门各比基底（918 / 787，bin 360 / 229）**多 2**，正是本票新增的两条用例
（`a_refused_run_does_not_take_the_earlier_pass_off_stdout` 与
`what_is_left_on_stdout_spells_its_commands_with_a_plain_space`；另外两条是改名，不是新增）。
lib 的 244 一格没动——本票一条库内用例都没加，`src/interlock.rs` 那五条原样跑绿。

`cargo xtask polish` 四条**全绿**：`fmt --check`、`clippy --all-targets`、
`clippy --all-targets --no-default-features`、`cargo doc --no-deps`。
**`cargo doc` 告警 15 条，与基线一格不差。**

**头一趟闸门 2 带回 2 条 `dead_code` 告警，是本票引入的**，已修：
`--no-default-features` 的**非测试**构建里 `session` 整个模块不在
（它挂在 `any(feature = "tui", test)` 上），`render::undone` 与 `render::plain::undone`
因此一个读者都没有。按 `src/session.rs` 模块文档那条规矩处置——**逐处挂、不整块放开**
（`p4-parking-lot/24` 收窄的正是这一笔）：两处各一句
`cfg_attr(not(feature = "tui"), allow(dead_code, reason = ...))`，文档里写清
「那不是死代码，是那一趟的前提」。重跑之后闸门 2 那一行告警没了。
