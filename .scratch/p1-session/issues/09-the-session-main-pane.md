# 09 — 会话主区：试算、执行、边跑边攒的报告

**What to build:** 会话里按一个键**试算**、按另一个键**执行**。主区自上而下画三段：
全局条（卷数 · 剩余时间）、当前卷条（在走哪一遍 · 步数）、报告区。

报告是**边跑边攒**的：已完成卷的判定、驱动页、失败页当场可见，
不必等全部跑完才发现参数错了。会话与命令行的措辞出自**同一套渲染**——
数据同一份，说法也该是同一套。

退出会话时把报告照原格式印到 stdout，`tonefit > 报告.txt` 仍然成立。

**Blocked by:** 01 — 渲染搬出 CLI 主文件；02 — 事件流；03 — 预扫；08 — 会话骨架。

**Status:** resolved

- [x] 试算与执行各有一个键，跑起来时主区实时更新
- [x] 全局条给出卷数与剩余时间；当前卷条说得出在走哪一遍
- [x] 一卷跑完当场显示它的判定与驱动页
- [x] 失败页出现的当场就在主区可见，带原因
- [x] 幂等命中的卷显示为跳过，并说清是哪四项依据没变
- [x] 这一趟怎么读的（介质与并发度）在卷级行里看得见
- [x] 会话与命令行**共用同一套渲染**，不是两份措辞
- [x] 退出会话时报告照原格式印到 stdout，退出码与命令行那一路一致
- [x] 渲染有快照用例：有失败页与没有两种、终端窄到放不下时的退化

## 落地记录

主区那一格从此有东西了：**`t` 试算、`x` 执行**，跑起来时三段实时更新。
两个键在键盘上离得远——按错一个会往盘上写东西，而这一路上没有第二道确认。

### 一趟跑起来之后的形状

```
drive 这条循环（UI 线程）              另一条线程
  画一屏 ────────────────┐              tonefit::run(&request)
  等一个键，最多等 80ms  │                   │ 每一条事件
  没等到就再画一帧       │                   ↓
  reap()：线程回来了吗 ──┘            Watch::observe → Live（一把锁，只折一条事件）
```

`tonefit::run` 一进去就跑到底，会话这一头还得接着画、接着认键，因此它在**另一条线程**上
（`session::run::Running`）。观察者回的恒是「继续」，只有一个例外见下。

### 主区三段

| 段 | 画什么 | 来源 |
|---|---|---|
| 全局条 | `3/12 卷 [===>  ] 1280/5400 步 · 已用 5m00s · 剩 3m20s` | `RunStarted` 的 `volumes`／`steps`（03 号票的预扫） |
| 当前卷条 | `卷三 · 第二遍 [==>   ] 1000/3000 步` | `VolumeStarted` 的卷名与步数，加 `PassStarted` |
| 报告区 | 抬头 + 逐卷卷级行 + 失败页（当场） + 末尾那几小结（收场后） | `VolumeFinished` 带的卷报告、`VolumeFailed`、`PageFailed` |

预告的步数是**上界**不是承诺，因此一卷收摊时要把它预告了却没走的那几步**结清**到全局条上
（`Event::RunStarted` 的 `steps` 对实现方的要求，命令行那一份是 `Bar::finish_volume`）。
剩余时间按至今为止的平均步速外推；一步都没走时答不出来，不编一个数。

收场之后「已用」就**定住**——那时用的是库交出来的 `Report::elapsed`，
它扣掉了在决策点上等人的那几分钟（停车场 Q41）；接着读自己那块表的话，
跑完坐着不动那个数会一路涨。

报告区长过一格就**滚到底**（`past_the_top`）：报告只增不减，而「当场看得见」说的正是
刚添上去的那几行。翻回去看前面几卷归 `11`（停车场 Q64）。

### 「共用同一套渲染」怎么保证的

不是靠自觉，是**结构上没有第二份**：

- **事件流就是报告的增量**（ADR 0011）。一卷跑完那条事件带着那一卷的 `VolumeReport`，
  `Live` 把它接到攒着的那一份 `Report` 上——攒出来的**就是**命令行最后一次性拿到的那一份。
- 报告区画的是 `render::header` / `render::volume` / `render::tail`，
  与命令行 `render::report` 拼的是同一批函数。会话里没有一句报告措辞。
- 逐页那几行默认不给（归 `11`）；**失败页当场那一段**是命令行没有的增量，
  措辞仍在 `render::failing_pages`，而它印的那一句与逐页那一行**同一个出处**
  （`render::failure_line`）。
- 收场那一句走 `render::outcome`，不印 `{:?}`——中文界面上不该冒出 `Stopped(Abort)`。
- 卷名怎么取只有一处（`render::volume_name`）：命令行那条横条与会话的当前卷条印的是同一个。
- 横条多宽只有一处（`crate::BAR_WIDTH`），indicatif 的模板由它拼出来。

`Request` 也只有一份：口味层每一项「落到默认值」那一步搬进了 `TasteLayer` 自己的方法，
命令行「命令行没点、预设也没说」那一档读的是同一个。
`an_untouched_session_asks_for_what_a_bare_command_line_asks_for` 把两边逐项钉在一起
（停车场 Q67）。**带参数那一路因此行为一字未变**——同一段代码走过去，不是靠比对。

### 退出会话

报告照原格式印到 **stdout**（`render::report`，四段一次性拼起来），印在终端还回去**之后**——
印进 alternate screen 的话它会随那一屏一起消失。退出码取**最后那一趟**，
与命令行那一路同一套：`0` / `2` / `3` 走 `crate::exit_code`，**没做成**是 `1`。
没做成收着两种（拒绝执行、那条线程恐慌），两种都没有报告可印（停车场 Q66）。

**唯一一处动用了停的地方**：退出会话时观察者回一次 `Instruction::Abort`，随后 `join`。
那不是两级停——收尾与中止那两个**键**归 `10`，本票一个都不占；这里只堵一个洞：
会话退出时不能把一条还在往盘上写字节的线程扔在身后（停车场 Q63）。

### 跑起来之后配置只读

不是画法上涂灰，是 `running_action` 一个改动键都不派。范围层也一起冻住了
（spec 只要求前两层），理由与代价见停车场 Q69。`Ctrl-C` 仍旧退得出去——
它在**每一个**状态下都是退出，那是 `08` 立下的性质。

### 快照

三张，都走 `TestBackend` 自己的 `Display`——它按 `cell_width` 跳过被宽字符盖住的那一格
（停车场 Q60 说的正是那一格），自己逐格拼的话每个汉字后面会多一个空格。

- `the_main_pane_without_a_failed_page`：一卷幂等命中（四项依据说全）、一卷真做过事
  （几何门、卷级基准档、**驱动页**、介质与并发度、缓存），第三卷正走第二遍。
- `the_main_pane_with_a_failed_page`：同一趟，其中一卷有失败页——隔离那一行与
  「失败页（出现的当场）」那一段并排出现，两者说的是同一份原因。
- `a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`：
  64 列的屏，左栏让到 34 列、主区拿满 30 列（`MAIN_MIN_WIDTH`），三段一段不少。
  再窄到 20×6、1×1 只验不恐慌。

屏上有时间的那两个数靠 `Live::rewind` 把时钟往回拨一段固定的量，快照因此不随机器快慢而变。

### 没做的

- **收尾与中止那两个键**归 `10`；本票只在退出会话时用了中止那一级，没有加键。
- **逐页展开与左栏收起**归 `11`：报告区默认只给卷级，滚不回去。
- **一键出标定图**归 `13`、**单卷续做接上决策点**归 `14`：观察者在决策点上不等人，
  照旧回「继续」。

### review 之后改的

`/code-review` 两轴各出了几条，收了这些：

- **`Ending` 重切了词汇表已有的《收场》**（Standards 轴）。整个删掉，换成
  `Live::ended()`（那条线程回来了没有）加 `Live::undone()`（没做成时那句话）——
  「收成了什么样」仍旧只有 `RunOutcome` 一处。
- **收场之后「用了」那个数一直涨**（Spec 轴）。`Live::overall` 收场后取
  `Report::elapsed`，不再读自己那块表；`the_elapsed_time_stops_moving_once_the_run_is_over` 钉住。
- **`format!("{:?}", outcome)` 把 Rust 标识符印在中文界面上**（Spec 轴）。
  换成 `render::outcome`，逐个变体列出、不留 `_`。
- **线程恐慌被折成「这一趟被拒了」**（Spec 轴）。改口说「这一趟没做成」——
  拒绝执行与恐慌都是它，分得开的是那句话本身。
- **窄终端那一条只有存在性断言，没有快照**（Spec 轴）。补了整屏快照。
- **`last_lines` 声称往多了估，其实按词断行会估少**（Spec 轴）。不估了：
  让 ratatui 自己画一遍再数（`past_the_top`），折行的规矩仍旧只有它那一份（停车场 Q65）。
- **失败页那一段是第二套措辞**（Spec 轴）。`render::failure_line` 收成一处，
  逐页那一行与当场那一段印的是同一句。
- **票没有 `## 落地记录`**（Standards 轴）。就是这一节。
- **卷名取法在两处逐字重复**（Standards 轴，Duplicated Code）→ `render::volume_name`。
- **`BAR_WIDTH` 与 indicatif 模板里的 `30` 是两个出处**（Standards 轴）→ 收成 `crate::BAR_WIDTH`。
- **`press` 问了两遍 `action`**（Standards 轴，Repeated Switches）→ `Session::act(action)`，
  `Session::press` 降成 `#[cfg(test)]` 的两步并一步。
- **「t 试算 / x 执行」三处三种说法**（Standards 轴）→ `START_KEYS` 一处；
  它们各自**做什么**只在报告区那一段说。
- **`Running::start` 直接覆盖 `self.thread`，把旧线程甩掉**（Standards 轴）→
  调试构建当场断掉，发布构建先 `collect()` 再起。
- **三个「结束」贴着放**（Standards 轴，Mysterious Name）→ `Live::finish` 改叫
  `Live::returned`；`Session::run_finished` 的文档点名它与 `Live::run_finished` 的分别。

几条没收，有理由：

- **`Live::observe` 与 `Bar::observe` 是同一张事件表的两份实现**（Standards 轴，
  Duplicated Code）。那正是 ADR 0011 定的形状：库只报到，**印在哪、长什么样由调用方定**，
  三个实现方各画各的。合成一份就等于让库替界面拿主意。
- **`draw::report_text` 只碰 `Live` 的取值器**（Standards 轴，Feature Envy）。
  挪到 `Live` 上就把「画成什么样」搬进了那个「一个终端都不碰」的模块，
  而两者分开正是 story 44 成立的原因。
- **失败页那一对 `(路径, 原因)` 没有类型**（Standards 轴，Data Clumps）。
  给它一个类型的话 `render` 就得认识会话那一侧的类型，而 `render` 眼下谁都不认识；
  这一对只在相邻的两个函数之间走，收益不抵那条依赖。

### 数

全量 **493 通过 0 失败**（基线 474），**17** 个测试二进制。
`cargo fmt --check`、`cargo clippy --all-targets`（含 `--no-default-features`）干净，
`cargo doc --no-deps` 与 HEAD 同为 **14** 条既有告警。
**黄金快照未变、未重录**——命令行那一路一字未动。
停车场新增 Q63–Q69 七条，一条都没了结。

## 停车场结转

本票记下、由 `p2-loose-ends/01` 了结的条目，原文照搬。

### Q68 — `tui` 关掉之后，本票新添的那三十几条用例同样一条都不跑

- **From:** 票 `p1-session/09`
- **Kind:** 路过发现
- **Where:** `Cargo.toml` 的 `[features]`；`src/session/`
- **Why it did not block:** 与 Q61 同一个洞，
  本票把它变大了：`live`、`run`、`draw`、`state` 四个模块新添的那三十几条用例
  整个在 `tui` 后面，`cargo test --no-default-features` 那一趟一条都编译不到。
  票面要的「`--no-default-features` 下仍要能编译」成立
  （`cargo clippy --all-targets --no-default-features` 干净）。
  `render::failing_pages` 是本票唯一因此挂上 `#[cfg(feature = "tui")]` 的东西——
  关掉之后没有人读它，不挂就是一条 `dead_code` 告警。
- **What this ticket actually did:** **没有搬。** 与 Q61 走同一条路：会话没有终端库以外的
  用户，把它搬到 feature 之外只为测试好看。默认那一趟（`tui` 开着）跑得到全部，
  基线因此从 474 涨到 493。
- **Whose call:** 拍板的人（同 Q61：关掉终端库那条路要不要有行为断言）
- **处置：** 由 `p2-loose-ends/01` 了结，与 Q61 同一刀（处置全文见 `p1-session/08` 的
  《停车场结转》）。本票新添的那三十几条里，`live`（9 条）与 `run`（15 条）跟着状态机
  搬到了 `tui` 特性外面，`cargo test --no-default-features` 那一趟从此跑得到；
  `draw` 那 23 条留在特性后面——它们画的正是终端，搬不出去，也不该搬。
  `render::failing_pages` 照旧挂 `#[cfg(feature = "tui")]`（读它的是画法）；
  同一批里的 `render::calibration_notice` 改成了 `any(feature = "tui", test)`——
  读它的是状态机，而状态机现在摆在特性外面。
