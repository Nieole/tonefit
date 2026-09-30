# 03 — 摊开成为第一个环节

**What to build:** 要摊开的卷走四个环节——**摊开 → 查重 → 分析 → 写出**；不摊开的卷照旧三个。`Pass` 多一个取值摊开：
开卷之后、摊开开始之前报一条环节开始（摊开），摊开途中那几步记在它名下。幂等命中、整卷跳过的归档卷照样报摊开——
查重要读源字节，源字节要先摊开。卷级计时多一段摊开，「段外」那一截不再装着它；进度那一侧本来就是四段，从此计时与进度同一条分界线。

`Pass` 是 `#[non_exhaustive]`，会话眼下有兜底那一支：屏上的名字与颜色归 `design-parity/13`，在那之前摊开期间屏上写兜底那一句。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 事件流用例：要摊开的卷先报摊开、摊开的步记在它名下；不摊开的卷不报它；幂等命中的归档卷照样报摊开
- [x] 卷级计时的摊开那一段，在要摊开的卷上不为零、在不摊开的卷上为零；段外那一截不再含摊开
- [x] 报告里那几处说「三段」「段外」的措辞跟着改对
- [x] `CONTEXT.md`《环节》改成四个（摊开只在要摊开的卷上出现）、《卷级计时》多一段；ADR 0015 补一句「摊开是一个环节」
- [x] 会话的设计快照照旧绿（屏上改名归 `design-parity/13`）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。**

1. **`Pass::Extraction`**（`src/progress.rs`）：排在 `Pass` 最前，名字取自《摊开 (Extraction)》。`Pass`、`Event::Stepped`、
   `Event::PassStarted` 的文档改成四个环节、摊开的步记在它名下。
2. **开工与步**（`src/source.rs`）：`extract` 头一句报摊开开工——卷已打开、成员已列齐、第一个字节还没解；两个格式共用这一处，
   两份 `spread` 报的步因此都记在它名下。幂等命中的归档卷在查重之前照样摊开，流上照样报。开卷那一条上就答了立即停止的话，
   `open` 只列成员、不摊开，摊开连开工都不报（与另外三个环节开工前先问闩同一个待遇；评审 Spec 轴指出，用例先红后绿）。
3. **卷级计时多一段**（`src/report.rs`）：`VolumeTiming::extraction`；`outside_the_segments` 减四段。表那一格由 `process_volume`
   以 `&mut timing.extraction` 交进 `source::open`，在 `extract` 里掐——摊不下回 `Err` 时那一格照写（07 号票要它）。
   「报开工 + 掐表」收成 `timed_pass`（`src/lib.rs`），摊开、查重、分析三个环节走它；写出环节开工那一条是确认点，照旧分开。
   取舍见 Q1097。
4. **措辞跟着改对**：`VolumeTiming`、`outside_the_segments`、`volume_steps`、`timed`、`process_volume`（含《立即停止》那张检查点清单）
   里的「三段」「进度四段这里三段」「段外装着摊开」「第二段」都改掉；`outside_the_segments` 那张零头单子里的「查重」
   （早年「撞名查重」的意思，今天与环节同名）改成「查撞名」。会话 `live.rs` 两处「另外两遍」改成「其余环节」。
5. **会话只动注释**：`pass_name` 的 `Some(_) => "这一遍"` 与 `colour_of` 的 `Kind::Pass(_) => None` 两支本来就兜得住；
   旁边的注释改成「摊开眼下落在这一支上，归 `design-parity/13`」。设计稿、场景数据一个字节没动。
6. **词汇表与 ADR**：《环节》四个（摊开只在要摊开的卷上出现），《卷级计时》一个环节一段、与进度同一条分界线；
   ADR 0015 决定第 3 条补一句「摊开是一个环节，排在查重之前」。《步》没补摊开（story 39 冻着，Q1098）。
7. **用例**：`tests/events.rs` 四条——要摊开的卷（`.7z`、`.rar` 各一）先走摊开、开卷与摊开之间零步、摊开名下一个成员一步；
   目录卷与 `.cbz` 不报摊开；幂等命中的 `.7z`、`.rar` 照样先摊开再查重；开卷那一条上就立即停止的卷一个环节都不报。
   `tests/timing.rs` 三条——摊开途中磨蹭 `waits`，摊开那一段 ≥ 它、段外 < 它（Q1100）；跳过的归档卷摊开那一段不为零；
   `.cbz` 摊开那一段为零（目录卷那一半加进既有那一条）。`tests/container.rs` 里认「此刻在摊开」的观察者从
   「第一条 `PassStarted` 还没到」改成「最近一条报的是摊开」，那三条用例的断言不变。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- 事件流：摊开开工那一句挪到顺序扫**之后**——四环节那一条红在 `(None, 3)` 对 `(None, 0)`；那一句整个拿掉——四环节、跳过两条红在
  少了 `Extraction`。
- 开卷即停：修之前那一条红在 `[Extraction]` 对 `[]`。
- 计时：摊开那一格不掐——`extraction ≥ waits` 红；只留段外那一句——段外量到 243ms（那时一步 60ms），红。
- 「不摊开的卷为零」那两句（目录卷、`.cbz`）钉的是一件不发生的事，没有造一个按反的实现去跑。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff b42f9b0`。

**收下的**：

- Spec：开卷那一条就答立即停止时多报一个一步不走的摊开——`open` 在摊开之前问闩（见上第 2 条）。
- Spec：跳过的 `.rar` 没测（story 14 点名 `.rar`）、`.cbz` 的摊开那一段为零没钉——两条都补。
- Spec：`Pass` 文档「开工那一刻就是表开始走的那一刻」对写出不成立、`VolumeTiming`「只掐四次表」对不摊开的卷不成立——改口。
- Standards：`tests/timing.rs` 模块头「唯一一个具体时长…见末一条」失真——改成点名两条用例；用例文档里的「（末一条）」改成点名。
- Standards：`live.rs` 两处「另外两遍」。
- Standards：用例里成员数两个出处——夹具交回成员数，断言与 `waits` 都从它来。
- Standards：ADR 0015 那一段复述词汇表——收成一句；段外零头清单三处三版本——只留 `outside_the_segments` 一处，
  `VolumeTiming` 文档指过去，词汇表那一句删掉。
- Standards：`open_seven_zip`／`open_rar` 的形参 `extraction` 被同名的 `let extraction`（`Extraction`）遮蔽——形参改叫 `segment`，与 `timed` 同名。
- Standards：「报开工 + 掐表」三份——收成 `timed_pass`。
- Standards：用例名带「四」——改掉。
- Standards：Q1097 说「三种放法」而列了四条——改；原先那条取名的条目（`Extraction` 对 `Extract`）选项 ② 违反 `CLAUDE.md`、近稻草人——删掉，
  编号顺移；Q1100 的选项 ② 写成它真正的好处（不会因负载误红）。

**驳回的**：

- 两份 `.7z` 夹具、三个认「这一步在哪个环节」的观察者：各个测试二进制各带各的夹具是这个仓库的写法（`small_volume`、
  `two_pages_and_an_extra` 都是），三个观察者问的是三件事（按个数记账、看临时目录、磨蹭）。
- `(Events, &mut Duration)` 穿四层（Data Clumps）：记在 Q1097 的代价里；为两格捆一个类型只有这一处用。

### 停车场

本票用了 Q1097–Q1106 里的四个：

- **Q1097**：摊开的开工与表放在 `source::extract`，表那一格由 `process_volume` 交进去；列了围着整句 `source::open` 掐、拆成两步、`Extraction` 自带耗时三条别的路。
- **Q1098**：《步》没列摊开，story 39 冻着，推荐补一句。
- **Q1099**：《环节》「三处逐字相同」在 `design-parity/13` 之前对摊开不成立，推荐照旧不加过渡说明。
- **Q1100**：「段外不含摊开」钉成上界断言，推荐留着。

结转两条（见《停车场结转》）：Q280、Q283，本票照上面的做法了结。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后；日志 `ss-03.gate1.log`、`ss-03.gate2.log`、`ss-03.gate3.log`、`ss-03.polish.log`，都在树外）。
这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1047 通过 1 失败**；lib 239 / bin 424；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.86s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **932 通过 1 失败**；lib 239 / bin 309；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.74s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.74s` |

**本票添 7 条用例**（`tests/events.rs` 4 条：26 → 30；`tests/timing.rs` 3 条：5 → 8），两条闸门各多 7 条；
lib、bin 两个数本票没动（改的库内、bin 内用例只改名、改注释）。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 145.87 秒、闸门 2 上 145.99 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
设计快照：`npm run check`「与库里那一份逐字节相同」，设计稿与 `tests/fixtures/design/` 一个字节没动，闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` 绿；
`cargo clippy --all-targets --no-default-features` 绿；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q280 — 摊开那一段有了步，却没有自己的一段计时：进度上四段，计时上仍是三段加段外

- **From:** 票 `p4-parking-lot/13`
- **Kind:** 落地时走的那条路（新代码带进来的一处不齐）
- **Where:** `src/lib.rs` 的 `volume_steps`（四段）与 `crate::VolumeTiming`（三段：
  `fingerprint` / `first_pass` / `second_pass`，加上 `outside_the_segments` 那一截）；
  `src/report.rs` 那两处文档写着「段与进度报到的那三段同一条分界线」
- **Why it did not block:** 本票给摊开那一段添了步与检查点，**没有给它添一段表**。
  摊开发生在 `source::open` 里，而那一句排在 `timed(&mut timing.fingerprint, ..)` 之前，
  它花掉的时间因此照旧落在 `VolumeTiming` 的**段外**那一截里（`elapsed` 减三段之和）。
  `VolumeTiming` 的文档从前写着「段是三个，与进度报到的那三段同一条分界线」——
  那半句从本票起不再成立（进度那一侧是四段），**本票已把它改口**：
  三段各自与进度同名那一段划在同一道界上，而摊开那一段在报告这一侧没有自己的一格。
  不阻塞是因为**那个数没有丢**：整段摊开一直有自己的一个阶段（`cost::Stage::Extract`），
  `--features profiling` 那张表上看得见它有多大；`VolumeReport::extracted`
  也照旧说得出这一卷摊了多少字节。少的只是「这一卷在摊开上花了多久」进不进那份**报告**。
  补它要往 `VolumeTiming` 加一格公开字段，而那是报告那一侧的形状，本票的地界在
  `source` 与 `progress` 之间。
- **What this ticket actually did:** **没有补那一格，只把两处文档各自改到说得出实情。**
  `volume_steps` 的文档如今说「四段」，`VolumeTiming` 的文档说「这里三段、进度四段，
  多出来的那一段落在段外」——两处各自都是对的，只是「几段」两处答得不一样。
- **路过还发现一句**（同一段文档，**不是本票改出来的**）：`VolumeTiming` 那句
  「前两段量的是**源**那一侧，**第二段**量的是**输出**那一侧」自相矛盾——「第二段」指的是
  三段里的**末一段**（写出那一段）。`crate::volume_steps` 那句同型的话本票已经改成
  「写出那一段 / 读那两段」，`report.rs` 这一句没跟着改：它是既有的，改它超出本票的地界。
- **Whose call:** 拍板的人（`VolumeTiming` 要不要多一格摊开；要的话
  `report.rs` 那句「进度四段、这里三段」跟着并回一句；上面那句「第二段」顺手改成「末一段」）
- **处置：** **`say-and-stop/03` 落地（2026-10-01）：照票面走。**`VolumeTiming` 多一格 `extraction`，`outside_the_segments` 减四段；
  「进度四段、这里三段」那句并回「一个环节一段，与进度同一条分界线」。「第二段」那句在本票之前已是「末一段」；
  `volume_steps` 与 `Event::VolumeStarted` 里剩下的「第二段」（指写出那一段）本票改成「写出那一段」。

#### Q283 — 事件流说不出「现在在摊开」：`Stepped` 不带段，而 `Pass` 仍是三个值

- **From:** 票 `p4-parking-lot/13`
- **Kind:** 落地时走的那条路（票面写死的一条边界带来的后果）
- **Where:** `src/progress.rs` 的 `Event::Stepped`（不带字段）与 `Pass`（三个变体）；
  `src/session/` 的当前卷那一行、`src/main.rs` 的 `Bar`
- **Why it did not block:** 摊开那一段如今报步，可**报出来的步说不出自己属于哪一段**。
  认它的唯一办法是流的形状——「第一条 `PassStarted` 还没到」，新添的那两条用例就是这么认的。
  命令行那条横条不受影响（它收到 `Stepped` 只 `inc(1)`）。会话那一侧也走得动，
  而「在走哪一遍」那一格此刻印的是**「开卷」**（`session::draw::overview` 的 `pass_name`
  在 `Pass` 为 `None` 时的那一支）——那个词从前只盖着一瞬（打开容器、列成员），
  如今要盖住一段分钟级的摊开。**它不是错的，是不够的**：横条在走，而抬头说的是「开卷」。
  不阻塞是因为票面把这条边界写死了——
  「观察者那条回路的形状一格不变：不给 `Event` 加格、不给 `Instruction` 加值」，
  而给 `Pass` 添一个 `Extract` 变体正是往那条回路上加格（`Pass` 是 `PassStarted` 的一格），
  同时也要改 `CONTEXT.md` 的《遍》——那是领域决定。
- **What this ticket actually did:** **一格都没加。**摊开那一段既不报 `PassStarted`、
  也不给 `Stepped` 加一格「哪一段」。用例靠流的形状认它，那一条写在用例自己的文档里。
- **Whose call:** 拍板的人（摊开要不要成为第四**遍**——那同时是 `CONTEXT.md`《遍》
  与 `VolumeTiming` 那三段的问题，与 Q280 是同一笔账的两半；不成为一遍的话，
  `pass_name` 那一支要不要改口）
- **处置：** **`say-and-stop/03` 落地（2026-10-01）：照票面走——摊开成为一个环节。**`Pass::Extraction` 排在最前，
  开卷之后、摊开开始之前报一条环节开始，摊开的步记在它名下；`CONTEXT.md`《环节》改成四个。
  `pass_name` 那一支不在本票改：摊开眼下落在兜底那一支（「这一遍」），屏上的名字与颜色归 `design-parity/13`。
