# 22 — 命令行报告按目录折叠

**What to build:** 会话那一侧已经默认一个目录一行、两级展开（`volume-discovery/08`），
而命令行那一份印出去仍是**三级全部**——几百卷的报告重定向到文件之后要滚五百行找那一卷。

给命令行那一路一个**说得清的开关**，默认或按开关折成一个目录一行
（几卷 · 基准档分布 · 隔离几卷）。**两边拿的是同一份数据**：
目录那一级的聚合只有一处出处，会话与命令行读的是同一处。

收停车场的 **Q171**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 命令行报告折得起来，一个目录一行带卷数、基准档分布、进隔离的卷数
- [x] 摊开与折起由一个说得清的开关定，`--help` 上说得明白
- [x] 聚合仍只有一处出处，与会话读的是同一处
- [x] 不折的那一副与今天逐字相同
- [x] 三条闸门全绿

## 五条验收开工那一刻各是什么状态

派活说明判「票面第一句的后半（命令行印出去仍是三级全部）在今天的代码上不成立」。
**核下来它一字不差地成立**，认错的是一条测试的名字（停车场 **Q571**）：

- `the_command_line_folds_the_report_by_directory` 名字里有 fold，验的却是
  **目录行摆在它那几卷前面**——分组加一行小标题，底下两级一行没少。
- `src/render/plain.rs` 同一时期的模块文档写着「命令行这一副**三级一并摆出来**……
  停车场 Q171 记着这一笔」。
- `volume-discovery/08` 自己有一节《勾得有保留：第三条》：「落地成的是每一枝那几卷前面
  多一行目录行，而卷级与逐页那两段**一行没少**……软的那一半没做」，并把它记成 Q171。

逐条核完的结果：

| 验收 | 开工那一刻 | 这一票做了什么 |
|---|---|---|
| ① 一个目录一行带卷数、分布、隔离卷数 | **一半成立**：`render::directory` 三格全在、`plain::line` 三格全印，缺的是「折得起来」——底下两级照印 | 补上折起那一副 |
| ② 一个说得清的开关，`--help` 说得明白 | **不成立**：一个开关都没有 | `--brief` 加帮助加两条用例 |
| ③ 聚合只有一处出处 | **已成立**：`render::grouped` / `render::directory` 一处，两副都读它 | **一个字没动**（硬约束） |
| ④ 不折的那一副与今天逐字相同 | 前提没变：今天那一副就是不折的那一副 | 按构造成立，见下 |
| ⑤ 三条闸门全绿 | —— | 见《数》 |

**真正剩下的活是 ① 的后半与 ②。** 已经成立的 ③ 没重做，④ 靠不改那条路径守住。

## 落地记录

### 折起那一副：一个枚举，一句 `continue`

`src/render/plain.rs` 新出 `ReportFold { Off, ByDirectory }`，`report` 收它作第三个参数。
折起那一副在目录那一行之后 `continue`，卷级与逐页那两段一行都不印。

**类型名避开了 `fold`**：`crate::wrap::fold` 与 `CONTEXT.md` 的《折行 (Wrap / fold)》
已经把那个英文词绑给折行，而 `src/main.rs` 里本来就有 `fold_help` / `folded_help` 两个折行函数。
新词条因此叫《报告折叠 (Report fold)》，类型跟着叫 `ReportFold`，方法叫 `Cli::report_fold`。
取值那个 `Off` 照仓库既有的 `Dither::Off` / `WhiteAlignment::Off` 办。

**`CONTEXT.md` 只加了一个新词条，一条既有词条都没改写**（《折行》《展开》一个字没动）：
`--brief` 引入的是一个新开关、一个新状态，加进词汇表是落地的一部分。

### 折的只有正文——抬头与末尾那几小结两副都全印

折起那一副要答的是「一屏看得完这一趟怎么样」。**没做成的那几卷、进了隔离的那几卷、
发现走不进去的那几处，只有末尾那几小结点得出是哪几个**——连它们一起藏掉，
折起那一副就成了一份说不出事的报告。

一处认下的损失记在 **Q574**：隔离那一小结那句「原因逐条列在上面」在折起那一副里
指着一段不在场的正文。措辞住在 `render` 那一层而折没折是 `plain` 这一副的排版决定
（ADR 0016 划的正是这条线），改它要么把排版捅回措辞层、要么破掉「一个目录一行」。
那一句的措辞让给 27 号票——它正要动这几处末尾小结。

### `--brief` 不进 `Request`

它一个像素都不改，改的只是印出来什么样。**参数哈希收的是 `Request`**
（`tonefit` 的 `metadata`），进去了就等于加个 flag 让上一趟的输出整批静默过期。
不收预设走的是同一条（与 `--dry-run` 同理：那说的是这一趟做到哪一步，
不是一份存得住的立场）。一条用例拿**整份 `Request` 的 `Debug`** 比着钉住它。

默认取**不折**，理由与另外两条备选记在 **Q573**：`--dry-run` 的全部价值在逐页那几行判据，
默认折起会让它变成一个印不出东西的开关；命令行那一路没有一个键可按，
默认藏掉的东西在屏上再没有第二个地方看得到。

`--brief` 与 `--dry-run` 的互相削弱**没进《开关互锁》**（**Q572**）：`Interlock` 住在库里，
而这一项库根本不知道它存在——做成互锁要为一个纯界面开关扩一次库的对外形状。
话写在 `--brief` 自己的长帮助里。

### 第四条是**按构造**成立的，不是比出来的

`ReportFold::Off` 那条路径上**一个字符都没变**：diff 里只多了一句
`if fold == ReportFold::ByDirectory { continue; }`，删除行除调用点补参数外一条都没有。
26 条现存断言一个字没改，它们此刻走的是测试里那个 `unfolded(&Report, Mode)` 助手
——「不折的那一副」在测试里从此有一个名字，正是票面第四条的措辞。

会话退出时印到 stdout 的那一份硬写 `ReportFold::Off`：`--brief` 到不了那儿
（会话是**一个参数都没有**时才进的那一路）。

### 接线由一条跑真进程的用例钉着

`Cli::report_fold` 说得出该挑哪一副、`plain::report` 说得出那一副长什么样，两处各有各的用例;
说不出的是 `main` 有没有把前者交给后者。`tests/exit_code.rs` 补一条
`brief_folds_the_report_down_to_one_row_per_directory`：跑两趟真二进制，比两份 stdout。

**提交前把那一行手动换回不折验过一次，用例当场红**（第一条断言就抓住），改回再绿。

两趟**共用一个工作区、跑之前清掉上一趟的产物**：换个工作区第二趟的临时目录路径就不同，
报告里那几行连折都没折就已经不一样；不清产物则第二趟整卷幂等命中、卷级那一行改口说「跳过」。
断言**一个字面记号都不认**——印出去之前那一份还要过一遍折行（折到 100 格），
这个夹具的卷根是一条临时目录路径，卷级那一行铁定折断。问的因此全是行与行的关系：
折起那一副更短、它的每一行都在不折那一副里逐字出现过、两头那两行一格没动。

### 数

三条闸门跑满，三条都绿，一条失败都没有（`cargo xtask gate`，基底 `4f48f9b`）。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **920 通过 0 失败**；lib **243** / bin **363** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **789 通过 0 失败**；lib **243** / bin **232** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.52s`` | 干净，一条告警都没有 |

**lib 两趟都是 243，一格没动**：这一票一行库代码都没碰，改的全在界面层
（`src/render.rs`、`src/render/plain.rs`、`src/main.rs`、`src/session/run.rs`）。

**两趟各多 4 条**（闸门 1 从 916 到 920，闸门 2 从 785 到 789，两个基线是 25 号票在
`e1ceb2b` 上量的）。**bin 各 +3**：

| 用例 | 问的是 |
|---|---|
| `the_folded_report_stops_at_the_directory_level` | 折起那一副停在目录那一级：抬头与末尾那几小结照旧在，卷级与逐页那两段一行不印 |
| `the_report_folds_by_directory_only_when_the_command_line_says_so` | 不点名是不折那一副、`--brief` 折起，而它**不进 `Request`** |
| `the_brief_help_says_what_it_folds_away_and_what_it_keeps` | `--help` 说得出藏了哪两段、留了哪两段、默认哪一副 |

**`tests/` 那一头 +1**：`tests/exit_code.rs` 的
`brief_folds_the_report_down_to_one_row_per_directory`——跑真进程比两份 stdout，
钉的是 `main` 那一处接线。**它红过一次**：把那一行手动换回不折，第一条断言当场抓住，
改回再绿。

**闸门之外那一遍**（`cargo xtask polish`，四条全绿）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**（`tonefit (lib doc) generated 15 warnings`），
一条没多。

### 评审

`/code-review` 两轴各跑一遍（**只读，没碰工作区**）。

**Spec 轴**收一条：**`main` 那一处接线没人钉**——`Cli::report_fold` 与 `plain::report`
各有各的用例，说不出的是 `main` 有没有把前者交给后者，把那一行换回不折全套照样绿。
补的就是上面那条端到端用例。其余各条判为已成立（验收 ③ ④、27 号票边界），
默认值与《开关互锁》两条落在 Q573、Q572。

**Standards 轴**收四条：

- **`fold` 这个名字已经有主**（`crate::wrap::fold`、《折行 (Wrap / fold)》、
  `main.rs` 里的 `fold_help` / `folded_help`）。类型改叫 `ReportFold`、
  取值 `Off` / `ByDirectory`、方法 `Cli::report_fold`，与新词条《报告折叠》对齐。
- **重复的夹具**：两条目录级用例那 7 行逐字相同，抽成 `two_branch_report(broken)`。
- **散弹式改动**：测试里那 26 处 `ReportFold::Off` 收进一个 `unfolded(&Report, Mode)` 助手
  ——顺带让「不折的那一副」在测试里有了名字。
- **单一出处与渐进披露**：`plain.rs` 里重复三遍的那句压成一处指向 `CONTEXT.md`
  的《报告折叠》，开关那一段另起一节。

判为不收一条：`mode` 与 `fold` 结伴进 `report()`（Data Clumps）。**第三个排版旋钮出现时再合**
——此刻合是为不存在的需要造抽象。
