# 04 — 会话的「此刻」可注入

**What to build:** 设计快照里的已用、预计还要多久、各卷耗时、转轮字形、回话多久退回，都是定值；实现里它们今天
各自问系统时钟，用例只能靠往回拨一段来近似。给会话一个**「此刻」**（spec《时钟》）：真会话每帧取一次单调时钟，
用例给定值；`Live` 里直接问系统时钟的那几处改成收它。

预先重构：屏上行为一处都不变，新界面的各票都站在它上面。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] `Live` 与会话里算已用、预计、各卷耗时、确认点上等人的时长，不再直接问系统时钟
- [x] 用例：给定「此刻」时，同一份攒下的东西算出的已用、预计、各卷耗时逐次相同
- [x] 用例：等待确认那一段仍不进计时
- [x] 原来靠往回拨时钟的用例改用给定「此刻」；旧界面用例照绿
- [x] 真会话里画面行为不变

## 落地记录

**做了什么。** 一处接缝（`Live`），三片红-绿：给定「此刻」已用与预计逐次相同 → 等人那一截逐格相等 → `rewind` 退场、画法侧夹具改给定「此刻」。屏上一格没变：会话用例 278 条照绿、黄金快照与画法快照一个字节没动。

**注入口**（`src/session/live.rs`，`tui` 特性外）：`Live` 多一格 `now: Instant`（`CONTEXT.md`《会话》：**此刻 (Now)**，本票加的词条），`pub fn tick(&mut self, now: Instant)` 是唯一的注入口。此后 `Live` 里要时刻的四处都读 `self.now`：`run_started` 记 `started`（表从开工那一条掐起）、`pass_started` 记 `deliberating_since`（等人那一截的起点，只在真停下来问的那一趟）、`stop_deliberating` 累进 `deliberated`（`now − since`）、`overall` 算已用（`now − started − deliberated()`，减数与被减数同一个此刻，Q118 那一格的差从结构上没了）与预计。`Live::new` 仍读一次系统时钟作初值（`Instant` 没有零值，下一帧到来之前计算线程就可能报到）——Q754 记了另两条路。`deliberated(now)` 的参数拿掉，直接读 `self.now`。`rewind` 删了。

**真会话**（`src/session/terminal.rs::drive`）：每一圈画之前 `let now = Instant::now()` 一次，借锁时 `live.tick(now)` 再画。事件在计算线程上折进 `Live`、答话在按键之后，两者读的都是上一帧的此刻——与逐事件读表差至多一帧（`TICK` 80ms 加画一帧），屏上以秒计看不出；差会累加的那一笔在 Q755。

**05 怎么用它。** 单位是 `Instant`，从哪一刻起算 = **开工那一条（`run_started`）到达时的此刻**。夹具一律从 `live::fixture::live_at(epoch, mode, resumes)` 起——它造 `Live` 并当场 `tick(epoch)`，开工之前那一次给漏不掉（漏了的话开工那一刻记的是造它那一刻的系统时钟，已用差几微秒、只在秒进位上偶尔红）。照场景数据摆：

```rust
let epoch = Instant::now();                       // 取什么都行，往后都从它往上加
let mut live = fixture::live_at(epoch, mode, resumes);
live.observe(RunStarted…);                        // started = epoch
…开卷、环节、步、收摊…
live.tick(epoch + Duration::from_secs_f64(scene.run.elapsed_s));   // 21s、178.5s……（已乘时间倍数）
live.pass_started(Pass::Second, Some(&so_far));   // 等待确认那一景：确认点在给定此刻**之后**到，
                                                  // 等人那一截从 epoch+elapsed 起算，再往后 tick 已用都不动
```

场景数据里 `now_ms` 是设计稿冻住的 `performance.now`，与 Rust 侧的 `epoch` 无关、不必对上；`run.elapsed_s` 是开工到那一刻的已用（已扣等人）；各卷 `elapsed_s` 里收摊了的卷出自 `VolumeReport::timing`（05 造报告时填进去），**当前卷（处理中／等待确认）那一格今天没有起点**——`Walking` 不记开卷时刻，08 号票画那一格时在 `volume_started` 里记 `since: self.now`（Q756，含等待确认时冻不冻结那一问）。

**用例。** `live.rs`：`a_given_now_makes_elapsed_and_eta_the_same_every_time_they_are_asked`（300s 走 250/1000 步：已用恰 300s、预计恰 900s、问两次同一份 `Overall`）、`the_clock_starts_at_the_run_started_event_not_at_construction`、`the_minutes_spent_deciding_are_charged_to_nobody` 改成给定此刻逐格相等（确认点前 300s、等 120s、答话后再 30s → 330s；第二卷再等 60s 不动；答「后面都写出」之后不再等；不等人的那一趟 420s 一格不减）、`the_elapsed_time_stops_moving_once_the_run_is_over` 加一句「跑完坐一小时仍是 42s」。画法侧：`probe::RAN_FOR = 300s` 一处出处，`a_run_walking`／`every_kind_of_volume`／`draw.rs` 与 `report.rs` 各一条等待确认快照改成 `live_at(epoch, …)` + `tick(epoch + RAN_FOR)`（等待确认那几份的第二次 `tick` 摆在确认点**之前**：摆在之后就把跑过的那一段减光了）；`report.rs` 两份已结束的夹具里那句 `rewind` 本来就是死的（结束之后读的是报告上的 `elapsed`），直接删。

**`CONTEXT.md`**：《会话》新增 **此刻 (Now)**。已有词条含义未改。

**review 之后改的**：`fixture::live_at` 抽出来（8 处「造 + 头一次给」合成一步）；「表从开工掐起」拆成独立用例；`yield_now` 死行删掉；`Live::now` 文档「从此不问」改成「除造它那一刻外」；词条去掉测试策略那一句、等人那一截改成引用「不算进计时」那一段；`probe::RAN_FOR` 的「一个出处」限定到画法这一侧；Q754 补 `live_at`，Q755 补「差会累加，最坏 (n+1) 帧」。`tick` 与 `terminal::TICK` 同名那一条没改：两者说的是同一个帧。

**数。** `cargo xtask gate` 三条（`.tmp/gate.log`，`EXIT=0`）：

- 闸门 1 · 默认构建：`cargo test`，合计 **988** 通过 0 失败（lib 238 / bin 409）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- 闸门 2 · 甩掉终端库：`cargo test --no-default-features`，合计 **836** 通过 0 失败（lib 238 / bin 257）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- 闸门 3 · 开着量具：`cargo check --features profiling`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.98s`

上一趟（03 收尾，`51d0db7`）是 986 / 834：`live.rs` 多两条用例（`tui` 外，两条闸门都跑），各多二。黄金快照 `tests/golden-snapshot.txt` 与画法快照一个字节没动。

`cargo xtask polish`（`.tmp/polish.log`，`EXIT=0`）：四条全绿——fmt 过；clippy 两趟 `Finished`；`cargo doc --no-deps` 仍是 **15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 03 同数）。

### 停车场结转

Q754–Q756（`.scratch/非阻塞问题.md`《待处理》）。

