# 05 — 按场景数据摆出那一趟与三组设置的夹具

**What to build:** 快照要逐格比上，Rust 侧的数据必须与设计稿一模一样（停车场 Q718：复刻设计稿的假数据）。
夹具**读导出的场景数据**，不在 Rust 里重写设计稿的伪随机与模拟（spec《夹具》）：

- 按场景数据造各卷的 `VolumeReport`，沿真跑那条路把事件喂进 `Live`（开工带清单、开卷、环节、步、收摊……），给定「此刻」；
- 摆好设备设置、处理选项与预设（预设文件放临时目录）；
- 临时目录里照设计稿的假盘建出那棵树，作为家目录。

会话的界面状态（视图、光标、展开）等新骨架落地之后，由各票按场景数据补上。

**Blocked by:** 02 — 导出快照与场景数据；03 — 开工事件带清点产出；04 — 「此刻」可注入

**Status:** resolved

- [x] 11 个场景的那一趟与设置都摆得出来
- [x] 用例（不经画法）：摆出来的那一趟，总进度、当前卷与环节、每卷状态、问题计数、灰阶分布与场景数据逐项相同
- [x] 用例：报告那一处对场景数据说出来的逐页各格、卷行行尾、备注行，在设计快照的字网格里找得到（各抽几处）
- [x] 夹具不读写用户配置目录，不改进程的环境变量
- [x] 不依赖画法的那一部分在 `--no-default-features` 那一趟编得过

## 落地记录

**做了什么。** 新模块 `src/session/scene.rs`（`#[cfg(test)]`，摆在 `tui` 外面）：**场景夹具**（`CONTEXT.md` 新词条）。
`Scene::named("running")` 读 `tests/fixtures/design/scenes/running.json`，`Scene::after("deciding-a")` 读
`sequences/deciding-a.scene.json`（与起点场景一样的那几样写着 `"unchanged"`，从起点场景那一份补回来），
按它摆出三样：

- **那一趟**（`Scene::live`）：三组设置拼成 `Request`（`Session::request`，预览与转换都走 `Mode::Process`，等不等人由
  `Resuming` 定——与 `terminal::resuming` 同一条），从 `live::fixture::live_for(epoch, &request, resumes)` 起
  （`live_at` 的兄弟，Q754）；开工带清单（`run_started` + `surveyed`：`SurveyedVolume` 直接造，非漫画文件的原因从渲染后的整句
  反查回 `NonVolumeReason`——把三类各说一遍去比，措辞仍只在 `render::non_volume_reason` 一处），然后**逐卷沿真跑那条路喂**：
  开卷 → 幂等那一道走满源页数步 → 分析环节 → 写出那一遍（带到此刻为止的报告；等人的那一趟先停在确认点上再答话：完成的答继续、
  `trialed` 答做完再停、答过「后面的卷都写出」的那一趟按 `a` 的是头一个停下来问的确认点）→ 收摊
  （`fixture::volume_finished_with_its_failures`：坏页两半都喂）；没做成的报 `volume_failed`；处理中的卷走到场景数据说的那一遍第几页；
  等待确认的那一卷在确认点那一条**之前** `tick`；结束了的 `run_finished(结局)` 再 `returned(库那一份)`——攒着的报告加上计时与两张表。
  处理中的卷分析环节走完的那几张坏页也当场报（`page_failed`，与真跑同一条路；眼下的场景数据里处理中的卷恰没有坏页）。
  按停止按到第几级按在会话上（`Session::press('s')`，`Live` 不记它）；结束了的那一趟不再按——会话的阶段上那一格已经没了，结局带着它。
  场景数据顶层的 `applied_preset`（套的是哪一份）**没摆到任何地方**：会话今天没有这一格，它在 `scene.data.applied_preset` 上等 13／14 号票。
- **三组设置**（`Scene::session`）：设备设置与处理选项从场景数据那张平的表写成一份 TOML 再走 `preset::read` 读回
  （取值怎么解析只有盘上那一条路）；处理路径与勾选进 `scope`；阶段照这一趟（`run_started`／`at_the_decision_point`／`run_finished`）。
  **预设文件**（`Scene::presets`）写在临时目录的 `config/tonefit/presets.toml`，`Presets::at` 指着它——不在家目录底下，家目录底下只有假盘。
- **家目录**（`Scene::home`）：临时目录里的 `home/`，照场景数据提到的每一处建出那棵树（11 个场景的并集，一趟只算一次）。
  **设计稿的假盘本身没导出**（Q764）：票面「照设计稿的假盘建出那棵树」只做到了场景数据认得出的那一部分——`~/下载/轻小说插图`
  底下的三卷、`火之鸟`／`寄生兽` 底下的卷没建（没人清点也没人补全到）。`Scene::path("~/漫画库")` 把 `~/` 换成它。

**数据只从场景数据来。** 逐页结果只有屏上开着的那一卷有整份；其余各卷照灰阶分布补页（Q736 → Q768）：需留意的那几页按名字里的序号落位，
剩下的位置按分布的次序填满，补的页画质分填零、源页高取输出高。`Score::from_value` 因此从 `#[cfg(test)]` 里放出来（Q767）。
卷的去处照场景数据给的写法接（Q765）；无法访问的地方那句原因原样带（Q766）；`trialed` 映射成完成、答话记做完再停（Q769）。

**用例。** `session::scene::tests`（两趟闸门都跑）：`the_running_scene_is_the_run_its_data_describes`（头一道接缝，另对一遍设计快照抬头上那几个数）；
`every_scene_is_what_its_data_describes`（11 个场景）；`every_sequence_that_moves_the_run_ends_where_its_data_says`（那一趟变了的 22 串序列：
答话、按停止、推进、立即停止……——票面只说 11 个场景，`Scene::after` 与这一条是顺手扩的，06 起各票按序列认领用例时起点在这里）——三条共用 `agrees_with_its_data`：三组设置（写回 TOML 再比，转换只有 `preset::write` 一处）、预设文件、
假盘、清单、总进度、当前卷与环节、每卷状态、问题计数（坏页 · 转换失败 · 无法访问 · 非漫画文件）、灰阶分布、需留意的页
（`render::notable` 六种各几页）、隔离与去处、每卷耗时、写出过没有、答话、停止级别、结局；`every_kind_of_non_volume_reason_is_read_back_from_its_sentence`。
`session::scene::on_the_grid`（`tui` 后面）：`the_pages_of_the_open_volume_are_on_the_grid`（每页结果那一景：坏页的尺寸、缩放、那一句在
`pages.120x36` 同一行上；`pages-a` 那一屏头三页的尺寸、缩放、灰阶、原因、画质分；抬头的灰阶分布）、
`row_tails_and_notes_of_the_running_scene_are_on_the_grid`（隔离那一句、没做成的原因、无法访问那条错误链、非漫画文件那一小结的抬头）、
`the_envelope_and_deciding_scenes_are_on_the_grid`（跳过那一句、代表页、灰阶分布；确认条上攒着那一份的灰阶分布）。
为此 `draw::design` 敞开到会话这一层（`pub(super) mod design`，`snapshot`／`sequence` 与新加的 `Expected::lines`）。

**夹具怎么调（06 起每票）。**

```rust
use crate::session::scene::Scene;

let scene = Scene::named("running");            // 或 Scene::after("running-s")
let live = scene.live.as_ref().expect("有一趟"); // 还没开跑的场景（fresh · add · config）是 None
let mut session = scene.session.clone();         // 三组设置与阶段已摆好；界面状态按 scene.data.session 补
let home = &scene.home;                          // 家目录（假盘），往画法／补全传
let presets = &scene.presets;                    // 预设文件，往 press 传
let now = scene.now();                           // 画之前 live.tick(now) 已经给过；再推进就 epoch + 更多秒
```

`scene.data` 是读成类型的场景数据（`Data`），`data.session` 那一段整个留成 `serde_json::Value`。要另一种趟（在确认点上按过停止、
答话不在头一个确认点）夹具当场 `panic`，说清缺什么。

**review 之后改的。** 标准轴：`Data::session` 那句「本模块不读它」改成只读两格；`session.rs` 与 `gate.md` 那句「七个模块」补上 `scene`；
`Scene::space` 那句假的 `allow`、`HEAD` 与 `metric.rs`／`live_for` 三句过强的文档各改一句；「要紧的页」改回词汇表的「需留意的页」；
`Said` → `PresetData`、反查那一处 → `non_volume_reason_of`、`shape_of` 拆成两个、`sentence_cell` 收成 `tally_text` 一处、
`Scene::live()` 收掉七处 `expect`、备注种类改成枚举、结局那段 match 收成 `outcome_of` 一处、`took` 复用 `live::fixture::took`（改收 `Duration`）、
处理中那一支的两次 `begin_pass` 收成一个循环。规范轴：处理中的卷当场报坏页；坏页的期望数不算被立即停止掉的那一卷（设计稿 `runTotals` 同一个数法）；
结束了的那一趟不再按停止；`Score::from_value` 与 spec《缝》的张力记进 Q767；假盘只做到一部分记进 Q764 与上文。

**红→绿，按接缝。** `Scene::named` 先是 `todo!` → running 那一条红（场景数据已读成类型）→ 造盘、写预设、摆设置、回放那一趟 → 绿。
扩到 11 个场景：`typical_size` 单看等待确认那一景只有一张超宽的页可查 → 改成全部场景的并集；等待确认那一卷少走了分析环节那一遍
（2903 ≠ 3127）→ 正在走的这一遍先走到第几页再停在确认点上；处理中且已到写出那一遍的卷没有攒着的那一份 → 写出那一遍恒带到此刻为止的报告
（会话的观察者默认读它）。扩到序列：结束之后会话的阶段上没有停止级别（结局带着它）→ 只在没结束时比。
字网格那一条：卷名在总览的当前卷那一行先出现一次 → 按两个记号找那一行。

**没做的。** 会话的界面状态（视图、光标、展开、覆盖层、输入行）——票面明说等新骨架落地之后由各票补；`Walking` 的开卷时刻（Q756，归 08）；
那一趟没变的那一百多串序列不另摆（与起点场景是同一趟）。

**数。** `cargo xtask gate` 三条全绿（`.tmp/gate.log`，`EXIT=0`；头一趟红在 `tests/single_source.rs` 的
`the_latch_encoding_lives_in_one_place`——用例里 `stop_level → Instruction` 那段 match 是闩编码的第二份，改成调
`Instruction::from_code` 之后重跑满一趟）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 995 通过 0 失败；lib 238 / bin 416；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 840 通过 0 失败；lib 238 / bin 261；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.17s` |

上一趟（04 收尾）是 988 / 836：闸门 1 多七条（`session::scene` 的四条不经画法 + 三条对字网格），闸门 2 多四条（不经画法的那四条）。
黄金快照 `tests/golden-snapshot.txt` 原样过，设计快照一格没动。
`cargo xtask polish` 四条全绿（`.tmp/polish.log`，`EXIT=0`）：fmt 过；clippy 两趟 `Finished`、零告警；`cargo doc --no-deps` 仍是
**15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 04 同数）。

### 停车场结转

### Q736 — 场景数据的逐页结果只给屏上开着的那一卷整份，其余各卷只给灰阶分布与要紧的那几页

- **From:** 票 `session-redesign/02`
- **Kind:** 票面没想到的第三种情形（产物体积）
- **Where:** `export.js` 的 `sceneData`（`volumes` 那一段：`pages` 只在 `S.pages.vol` 那一卷上，其余 `notable_pages` + `tally` + `page_count`）；`tests/fixtures/design/scenes/*.json` 与 `sequences/*.scene.json`
- **Why it did not block:** 整份逐页是 84 卷 × 近两百页：一个场景 6 MB、二十四份快照加一百四十串序列 295 MB，进不了库。屏上从一卷身上读得到的只有灰阶分布、要紧的那几页与页数——没开着的卷，不要紧的页长什么样屏上看不见
- **What this ticket actually did:** 只给开着的那一卷整份；序列走完那一刻与起点场景一样的那几样（`run`、路径、设置……）写成 `"unchanged"`，整目录 4.7 MB
- **Options:** ① 照旧：05 号票照灰阶分布补齐不要紧的页（页名按序号、判定按分布），设计稿的伪随机页不在 Rust 侧复刻；② 全部逐页导出、压紧写法（每页一个数组）——仍约 70 MB；③ 05 号票把设计稿的 `rng` 与 `pagesOf` 在 Rust 里复刻一遍——Q718 拍板时说过不这么做
- **Recommend:** ①。哪个用例要开另一卷的每页结果，加一串序列开那一卷再重导，那一卷的 `pages` 就跟着出来
- **Whose call:** 05 号票的实现者
- **处置：** 已了结（`session-redesign/05`，按推荐拍板 ①，2026-09-15）：夹具照灰阶分布补齐不要紧的页（`src/session/scene.rs` 的 `pages_of`），需留意的那几页按名字里的序号落位；补的页长什么样记在 Q768。

### Q754 — `Live::new` 仍读一次系统时钟作头一个「此刻」：此后每帧由会话层给，用例给定值

- **From:** 票 `session-redesign/04`
- **Kind:** 走了哪条路（注入口的形状）
- **Where:** `src/session/live.rs` 的 `Live::now`、`Live::new`、`Live::tick`；`src/session/terminal.rs` 的 `drive` 每帧读一次 `Instant::now()` 交进去
- **Why it did not block:** 票面要的是「算已用、预计、等人那一截的地方不再直接问系统时钟」，四处都改成读 `self.now`；`Instant` 没有零值，造 `Live` 那一刻要有一个时刻可记（下一帧到来之前计算线程就可能报到），`new` 因此读一次作初值。给定「此刻」的用例在开工那一条之前先 `tick` 一次，初值就不露面
- **What this ticket actually did:** `new` 读一次系统时钟作初值，`tick(now)` 是唯一的注入口；`tick` 的文档写明「开工那一条之前那一次不能省」，省了就是造它到给时刻之间那几微秒的差、在秒的进位上偶尔露面。用例一律从 `live::fixture::live_at(epoch, mode, resumes)` 起——造与头一次给合成一步，漏不掉（review 之后加的，05 的夹具也从它起）
- **Options:** ① 照现状；② `Live::new(request, resumes, now)` 多收一个参数——66 处调用改，`Running::start` 与 `press` 也要多传一个（`drive` 有帧的此刻，用例那十几处 `press` 要另给）；③ `now`／`started` 改成 `Option<Instant>`，第一帧之前没有此刻、开工那一刻记 `None`、已用答零——用例漏了开工前那一次 `tick` 时是**确定地**红，不是偶尔一秒的差，代价是三格 `Option` 与「没有时钟就没在等」那一格怪相
- **Recommend:** ①——②的一百来处改动只换来去掉一次读表；③把一个微秒级的状态摊成三格 `Option`。05 的夹具照 `tick` 文档那句写一次就够
- **Whose call:** 05 号票的实现者（夹具是唯一另一个调它的地方）
- **处置：** 已了结（`session-redesign/05`，按推荐拍板 ①，2026-09-15）：夹具从 `live::fixture::live_for(epoch, &request, resumes)` 起——`live_at` 的兄弟，收场景数据拼出的那份 `Request`，造与头一次给合成一步；`Live::new` 照旧读一次系统时钟作初值。
