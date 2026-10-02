# 09 — 行为收紧：试算不造来路，拒绝的理由交出枚举

**What to build:** - **试算不造来路**：来路在场的谓词从「指纹在场」收成「指纹在场且这一趟真要写」；试算那一趟一份都不造，
  「来路在场 ⟺ 记录器在场」从此真的成立；
- **`why_nothing_is_left` 交出枚举**：位深那一支带一句话，抖动那一支（互锁 ③）只是一个标记，各调用处按它造拒绝——
  不再有一句没有用户看得到的半截话。互锁 ③「只有一处判定」那条用例照旧是它的闸门，形状跟着改。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 窄计数器断言：试算那一趟（记录开着）造的来路是零份；照做那一趟照旧每页一份
- [x] `why_nothing_is_left` 交出枚举；拒绝那几句话一个字不变
- [x] 互锁 ③「只有一处判定」那条用例照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 两处行为收紧（收停车场 Q490、Q583）。转换那一趟的输出字节、拒绝的每一句话一个字不变。

1. **预览不造来路**（Q490）：`Compute` 里那一格从「这一卷的指纹」收窄成「这一趟盖记录用的那份指纹」，改名 `records`，
   在 `process_volume` 装这一摊时只给真要写的那一趟（`fingerprint.as_ref().filter(|_| writes)`，`writes` 即 `mode == Process`）。
   读它的三处——来路（`Placement::new`）、彩页当场盖的 `Record`、灰度页当场造的 `Recorder`——因此同进同退；后两处本来就只在转换那一趟走得到，
   收窄对它们是空操作（Q1328）。幂等那一道用的指纹是另一个变量，没动。剩下一角：确认点上答做完再停的那一卷来路照旧造了，写在 `Placement::new` 的文档里（Q1329）。
2. **窄计数器「来路造了几份」**：`VolumeReport::origins`（公共字段，库对外形状多一格）。数记在造的动作上——`pipeline::Origins::of` 造一份记一份，
   与解码器、缩放器同形装进 `ComputeCounters`。`CONTEXT.md`《窄计数器》的列举从三个改成四个，含义没动（Q1327）；`decode.rs`、`tests/counters.rs` 模块文档、
   `render.rs` 那条「窄计数器不进渲染」的用例跟着改。19 处 `VolumeReport` 字面量各添 `origins: 0`（五处在 `src/session/`）。
3. **`why_nothing_is_left` 交出枚举**（Q583）：`NothingLeft::BitDepth(String)` 带着那句话，`NothingLeft::DitherOutsideTheGate` 只是个标记。
   唯一的调用处 `candidates` 按它办（Q1330）：灰阶档位那一支戴 `Refusal` 交 `Err`；抖动那一支交 `Ok(None)`，那句拒绝照旧由 `Candidates::for_gate` 在撞上的那一页上造全。
   `Candidates::new` 两侧都用 `?`，门成立那一侧的 `None` 由一句 `expect` 认定不可能；从前那一行 `.ok()` 连同它丢掉的半截话一起没了。
   位深那一句的 `format!` 原样搬进枚举，`for_gate` 与 `dither_outside_the_gate_error` 一个字没动。
4. **用例**：
   - `tests/counters.rs` 新添 `a_dry_run_builds_no_origin_and_a_conversion_builds_one_per_output_page`：一张跨页加一张不切的页（三张输出页），
     预览（记录开着）零份、转换三份、关掉记录的转换零份。
   - `src/interlock.rs` 的 `the_refusal_is_driven_by_this_interlock_alone` 改了形状：「裁空 ⟺ 说得出是哪一维」照旧；
     「抖动那一支 ⟺ 互锁 ③」改问变体；末了按 `candidates` 交回的三种形状（有候选 / 这一侧没有 / 拒绝）逐格核对它是按哪一支办的。

### 按反跑看见了什么

- 新用例落在谓词改动之前：预览那一趟 `origins` 是 3，红在「预览那一趟造了没有读者的来路」（`left: 3, right: 0`）。
- 谓词按反成「从不给」（`filter(|_| false)`）：预览那一格照绿，红在「转换那一趟该每张输出页一份来路」（`left: 0, right: 3`）——两条断言各自咬得住。
- 互锁那条用例：把 `candidates` 的抖动那一支退回旧形状（戴着规则那一句的 `Refusal` 交 `Err`），红在 `None Some(FloydSteinberg) Broken` 那一格。

三次按反都已撤回。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff e6d95ab`。Spec 轴：没有阻塞项；谓词、拒绝措辞、互锁用例、新用例逐条核过等价。

**收下的**：

- 新写的文字用了旧称「试算」（`CONTEXT.md`《预览》：旧称「试算」）——代码、用例、停车场条目一律改成「预览」，用例名里的 `real_run` 改成 `conversion`（《模式》：转换）。票面原文的「试算」没动。
- 新用例三处 `pages.len()` 各写一个 `3`（`docs/agents/testing.md`：同一个数只有一个出处）——收成一个 `OUTPUT_PAGES`。
- `Placement::new` 的文档前一段说「两个方向都成立」、后一段又认下做完再停那一角，自相抵触——改成「只有一角对不上」，指到 Q1329。
- `Origins` 的文档说「来路只此一处造」，而幂等比对那几处也现造一份 `Origin` 去比——写明数的是跟着输出页、要写进记录的那一份。
- 「预览那一趟指纹照算」的理由写了四遍（单一出处）——只留在 `Compute::records`，`lib.rs` 那句注释与 `Origins` 的文档改成指过去；`ComputeCounters` 的文档不再数窄计数器有几个。
- 停车场四条缺 **Why it matters**——补上；Q1327 的另一条路原是「不进报告」，与票面相抵近于稻草人——换成「报告照添、拼报告时数产物」。

**驳回的**：

- `Compute::records` 名字指《记录》、装的是《指纹》：名字说用途、类型说内容，与 `Placement::new` 那个参数同名；岔口记在 Q1328。
- `Origins` 读着像「一堆来路」：与 `Decoder`、`Resampler` 同一个位置，名字取词条《来路》，文档首句说清它是带计数的造法。
- `self.records` 与 `&self.counters.origins` 在两处一起传进 `Placement::new`（Data Clumps）：两处在同一个函数里、隔十行；`Placement::new` 收几个参数已在 Q1309 记着。
- 19 处 `origins: 0`（Shotgun Surgery）：报告是结构体字面量造的，多一格公共字段就是这么多处，没有构造器可收；不为它另造一个夹具建造器（spec《Out of Scope》：卷报告夹具的建造器拷问判不做）。

### 停车场

本票用了 Q1327–Q1330：

- **Q1327**：「来路造了几份」做成第四个窄计数器，数记在动作上；`CONTEXT.md`《窄计数器》列举从三个改成四个——请协调人确认。
- **Q1328**：谓词收在装 `Compute` 的那一处，字段改名 `records`。
- **Q1329**：确认点上答做完再停的那一卷，来路照旧在分析环节里造了。
- **Q1330**：`candidates` 交 `Result<Option<Vec<Candidate>>>`，门成立那一侧由一句 `expect` 认定不可能。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-09.gate1.log`、`os-09.gate2.log`、`os-09.gate3.log`、`os-09.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
条数比基线（`e6d95ab`）多一条：`tests/counters.rs` 那条新用例；互锁那条改的是形状，条数不变。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1145 通过 1 失败**（1 ignored）；lib 253 / bin 478；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 77.34s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1003 通过 1 失败**；lib 253 / bin 336；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.76s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 8.06s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 200.80 秒、闸门 2 上 164.67 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 478 条里，全绿；本票在 `src/session/` 只给五处报告夹具添了 `origins: 0`，屏上一格没动。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q490 — 试算那一趟（记录开着）照旧白造来路：指纹在，而读它的第二遍不在

- **From:** 票 `two-pass-rework/07`
- **Kind:** 同型的白付，票面没点名的那一处
- **Where:** `src/lib.rs` 的 `Placement::new`，判据收的是 `Option<&Fingerprint>`。
  `Mode::DryRun` 那一趟指纹**照算**（幂等那一道要问它），而来路的读者 `Recorder` 只在两处
  造得出来——第二遍的 `Encode`，与逐页那条路上的 `Window::encode`——试算两处都不走
  （`Window::open` 要求 `mode == Mode::Process`，第二遍整个不在）。于是 `--dry-run`
  而不带 `--no-metadata` 的那一趟，每张输出页照旧造一份没有读者的来路。
- **Why it did not block:** 票面点名的是「**关掉记录**的那一趟」，试算是另一个开关。
  量级与本票删掉的那一半同级——一张输出页一个 `String`（转义过的成员名）。

  **它与 spec 那五处的分界在哪**：五处的形状是同一句话——「一个开关关掉了消费者、
  生产者还在跑」——按**开关 × 消费者**排的：默认路径 × 参照进缓存（P-C）、
  覆盖顶死判定 × 参照进缓存（06）、试算 × 彩色缩放（05）、命令行 × 卷报告（07 其一）、
  关掉记录 × 来路（07 其二）。这一处是**同一个形状、同一个类**，只是那张表上
  「试算 × 来路」那一格从来没人审过：05 号票审试算那一趟时只问到彩色缩放
  （spec 原话「现有注释只想到了省编码」），没有回头问「试算那一趟还有什么没人读」。
  **它不在名单上不是因为它不同类，是因为那一格漏审了**——这一条是那次漏审的记录。
- **What this ticket actually did:** 只把判据接到**指纹**上。「来路在场 ⟺ 记录器在场」
  因此眼下是**近似**成立而不是真的成立：`--no-metadata` 那一趟两边都不在（本票管的那一半，
  真的对上了），试算那一趟指纹在而记录器不在。`Placement::new` 的文档写的是
  「谓词就是指纹本身」而不是「记录器」——那句话今天为真。
- **Options:** ① 照现在；② 判据收紧成「这一趟会不会造 `Recorder`」（指纹在 **且**
  `mode == Mode::Process`）；③ 连同别处的试算白付一起，给试算立一条「这一趟什么都不写」的
  共用判据。
- **Recommend:** ②，**建议**在本 effort 里另开一张小票，**不要塞进 07**。
  ② 改的是同一句话（`Placement::new` 那一个参数），而它让那条不变量从「近似成立」变成
  真的成立，`zip` 那三处的「一起在、一起不在」从此没有例外。不塞进 07 的理由：
  07 的两个复选框逐字点名的是命令行与关掉记录那两趟，顺手多改一个开关会让
  「输出字节不变」那两条验收多背一个它没验过的现场。
  **开不开票不是本票定的**——落地的人在自己的票里替排票的人开票是抢板；
  这一条把分界与做法写实到这里为止，够 `/settle` 那一趟一眼判得出该不该开。
- **Whose call:** 排票的人（开不开票）；真做的时候是落地的人
- **处置：** **`one-source/09` 落地（2026-10-02）：照②了结。**谓词收在装 `Compute` 的那一处（`Compute::records`，只给真要写的那一趟那份指纹，Q1328）；预览那一趟来路零份，由新添的窄计数器 `VolumeReport::origins` 钉着（Q1327）。剩下确认点上答做完再停的那一角（Q1329）。

#### Q583 — `why_nothing_is_left` 抖动那一支回的是半截话，而没有一个用户看得到它

- **From:** 票 `p4-parking-lot/21`
- **Kind:** 你确实拿不准的单项
- **Where:** `src/lib.rs` 的 `why_nothing_is_left`、`candidates`、`Candidates::new`
- **Why it did not block:** 拒绝按页分岔之后，那句话的后半截（有没有出路）要**一页**才说得全，
  而 `why_nothing_is_left` 在碰卷之前就被调到。本票让它抖动那一支只回**规则那一句**
  （`Interlock::DitherOutsideTheGate` 的 `Display`），出路那一半由 `Candidates::for_gate`
  在页上补。于是那半截话**一个用户都看不到**：`Candidates::new` 用 `.ok()` 把它丢掉，
  另一条调用路径（`ensure_the_overrides_leave_a_candidate`，走门成立那一侧）
  根本触发不到抖动这一支。留着它是为了 `interlock.rs` 那条
  `the_refusal_is_driven_by_this_interlock_alone` 的立论——「说不说得出话 ⟺ 互锁 ③ 咬上」。
- **What this ticket actually did:** 留着那半截话，并在 `why_nothing_is_left` 的文档里
  写清「两支说得出的话不一样全，那不是漏」。没动 `interlock.rs` 那条用例的形状：
  它是 Q33 收口的产物，两处判定合成一处靠的就是它。
- **Options:** ① 留着半截话（本票走的）；② 把返回类型换成枚举
  （位深那一支带一句话、互锁 ③ 那一支只是个标记），`candidates` 各按各的造 `Refusal`——
  半截话不必存在，代价是 `why_nothing_is_left` 的签名、两处调用与那条用例一起动；
  ③ 让 `why_nothing_is_left` 收一个「这一页有没有出路」的入参，碰卷之前那一次传个
  说不出理由的值。
- **Recommend:** ②，但不在这张票里做——它动的是那条用例的形状，而那条用例是
  「互锁 ③ 只有一处判定」这条性质的唯一闸门，值得单独一趟。③ 不要：那个入参没有真值。
- **Whose call:** 拍板的人（`why_nothing_is_left` 的返回类型要不要换成枚举）
- **处置：** **`one-source/09` 落地（2026-10-02）：照②了结。**`why_nothing_is_left` 交 `NothingLeft`（`BitDepth(String)` / `DitherOutsideTheGate`），唯一的调用处 `candidates` 按它造拒绝（Q1330）；互锁那条用例改问变体与调用处的三种形状。拒绝的话一个字没变。
