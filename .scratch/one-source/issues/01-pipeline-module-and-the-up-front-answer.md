# 01 — 分析与写出两个环节连同滚动窗口搬成 `pipeline` 模块，开卷之前那一问换成具名三态

**What to build:** 纯搬家：滚动窗口那一簇（窗口、座位、开卷之前那一问）连同分析环节、写出环节、输出页，从库的根模块搬成一个 `pipeline` 模块；
根模块只剩对外的那几个 seam 与装配。开卷之前那一问的答案从嵌套 `Option` 换成具名三态枚举——
**答不出 / 答得出而没顶死 / 顶死在这一档**——`Settles` 从它派生，两个消费者各解一次、解法不同的那两处从此写不出来。
行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 滚动窗口、分析环节、写出环节、输出页住进 `pipeline` 模块；根模块里不再有它们
- [x] 开卷之前那一问交出具名三态枚举，`Settles` 从它派生；代码里不再有嵌套 `Option` 那种写法
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 一次纯搬家，外加开卷之前那一问换形状：

1. **`src/pipeline.rs`**：《管线》那三个环节——幂等这一道、分析环节、写出环节——连同夹在后两个之间的汇总、输出页
   （`OutputPage`／`Outcome`／`Branch`）、两套候选与覆盖项那几问（`Candidates`、`candidates`、`why_nothing_is_left`……）、
   `Compute`、`Settles`、开卷之前那一问、输出页的名字、缓存那把锁与 `cores`，从 `src/lib.rs` 原样搬过来。
   搬的是原文件里连着的两段（`lock`…`cores`，`uniform_size`…`output_names`）；逐行只动了 `pub(crate)`、链接目标、
   rustfmt 折的两处签名。边界为什么画在这里、`process_volume` 为什么留在根模块，见 Q1068。
2. **`src/lib.rs`**（4508 行到 1473 行）：留四个 seam、`run`、`process_volume`（装配），外加装配自己要用的步数预告
   （`volume_steps`／`MemberCounts`）、开工前与卷内那几道校验、`Refusal`。`pipeline` 往回够的只有 `Refusal` 一样。
   样张（`src/proof.rs`）共用的那几个——`open_source_page`、`examine_gray_page`、`candidate_bytes`、`color_bytes`、
   `output_name` 与它们带着的类型——改从 `crate::pipeline` 取，仓库里仍只有这一份。
3. **开卷之前那一问**：`pinned_up_front` 交出具名三态 `UpFrontAnswer`——`CannotTell`（答不出）／`NotPinned`
   （答得出而没顶死）／`PinnedAt(Candidate)`（顶死在这一档）；`Settles::if_processing` 从它派生，四支与从前逐一对应。
   `Option<Option<_>>`、`Some(None)` 在 `src/` 里一处都不剩。`CONTEXT.md`《覆盖顶死》那句「开卷之前答不答得出」
   后面补了类型名与三态（没改词条的含义）。
4. **指路跟着改**：别的模块里指向搬走那几样的 `crate::X` 改成 `crate::pipeline::X`（11 个文件）；
   ADR 0005 三处按文件路径的指路改成按条目名（`GateGroups::pinned`、`Settles::for_this_run`、`first_pass_verdicts`）。
5. **用例一条没添、一条没删**：库内用例按归属分家——进度步数与撞名那两条留在根模块，其余十条连同帮手搬进 `pipeline`；
   `tests/` 一个字节没动。

**票面与代码对不上的那一处**：票面要搬的滚动窗口那一簇（`Window`／`Windowed`／`Seat`／`pinned_for_the_first_pass`）
早在 `8b38430` 就删了，第一条验收里那一截按空集读、没有造一个窗口出来搬（Q1067）。Q545 说的两个消费者随之只剩一个。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff c598cbe`。Spec 轴无阻塞项：逐格核过新旧 `pinned_up_front` 与 `if_processing` 的对应，
把搬家按行做了多重集比对，Q1067、Q1068 的读法认为站得住。

**收下的**（都来自 Standards 轴，Spec 轴点了其中第三条）：

- `pipeline` 的模块文档与 Q1068 把汇总也叫成环节——《环节》定死是三个。改成「三个环节，连同夹在后两个之间的汇总」。
- `pipeline` 的模块文档说「一卷做不成算哪一种失败」由 `process_volume` 说了算——拒绝开始的标记是环节里戴上、`run` 认的。删掉那半句。
- 根模块文档「只放四个 seam 与装配」说过了头：补上步数预告、那几道校验、拒绝开始的标记。
- ADR 0005 的指路只把 `src/lib.rs` 换成 `src/pipeline.rs`，而且那个文件里 `pinned` 有三个——改成按条目名指。
- `UpFrontAnswer` 没进词汇表：补在《覆盖顶死》那一句后面（`CLAUDE.md`《改 CONTEXT.md 的规矩》：新类型、新状态当场加）。
- 「两组对不上要等整卷」三十行里说了三遍：删掉函数体里那句注释，`CannotTell` 的文档改成指回 `pinned_up_front`；
  `for_this_run` 那句「三种答案三个落点」数不对（四支落到三个取值上），改成「各有去处」。
- 根模块用例文档里「那里的文档说了」指代不清：点名 `pipeline` 的用例模块。
- 根模块 `process_volume` 里一句普通注释还写着 `[`first_pass_verdicts`]`：改成 `pipeline::first_pass_verdicts`。
- Q1068 说「逐行没改」漏了本票真改的那几处：补上；审查点的《Divergent Change》（`pipeline.rs` 三千来行）记进 Q1068 的第四个选项。

**驳回的**：

- **链接三种写法并存**（`[`x`](crate::x)`／`[`x`](pipeline::x)`／靠 `use` 解析的裸 `[`x`]`）。驳：三处各取从自己那个模块
  最短解析得到的路径，仓库里本来就是这样（`crop.rs`、`cache.rs` 用 `crate::`，别处靠 `use`）；统一成一种要么多写前缀、要么多添只为文档的 `use`（后者会报未用）。
- **《Shotgun Surgery》：一次搬家改了 12 个文件的指路**。驳：那是指路跟着真相走，不是一个逻辑改动散在各处；根模块与 `pipeline` 之间接口宽，Q1068 已记。

### 停车场

本票用了 Q1067–Q1076 里的两个：

- **Q1067**：票面的滚动窗口那一簇早已不在，按今天的代码读；推荐收尾时把 spec 与 02 号票里「滚动窗口」的说法改成逐页那条路。
- **Q1068**：`pipeline` 的边界画在环节上（幂等这一道一并搬、`process_volume` 留根模块当装配）；列了只搬两环节、连 `process_volume` 一起搬、再切子模块三条别的路。

结转两条（见《停车场结转》）：Q431、Q545，本票照上面的做法了结。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后；日志 `os-01.gate1.log`、`os-01.gate2.log`、`os-01.gate3.log`、`os-01.polish.log`，都在树外）。
这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
因此照 `proof-sheet` 那几张的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1035 通过 1 失败**；lib 239 / bin 423；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 67.24s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **920 通过 1 失败**；lib 239 / bin 308；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.65s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.07s` |

**基线**是 `proof-sheet/06` 落地那一刻（`c598cbe`）：闸门 1 **1035 通过 1 失败**（lib 239 / bin 423），闸门 2 **920 通过 1 失败**
（lib 239 / bin 308），闸门 3 绿，红的是同一条。**两条闸门一格没动**：纯搬家不添不删用例，库内十二条用例只是换了模块。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 157.31 秒、闸门 2 上 158.09 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
`git diff c598cbe -- tests/` 为空。设计快照那几条在闸门 1 里照旧绿（`tests/` 与设计稿一个字节没动）。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数，逐条都是既有的 `links to private item`，本票没添一条）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q431 — 滚动窗口那三百行落在 `lib.rs` 里，而同量级的东西各占一个模块

- **From:** 票 `two-pass-rework/12`（`/code-review` 的 Standards 轴指出）
- **Kind:** 落地时走的那条路（放哪儿）
- **Where:** `src/lib.rs` 的 `Window` / `Windowed` / `Seat` / `pinned_up_front`（约 300 行）
- **Why it did not block:** `cache`、`hysteresis`、`envelope` 这些同量级的东西各占一个模块，
  而 `lib.rs` 已经三千多行——审查按《Divergent Change》点了这一条。本票没有搬，理由是
  **搬出去的那一个站不住**：窗口要用 `OutputPage`、`Outcome`、`Branch`、`Candidates`、`lock`
  五样，全是 `lib.rs` 根上的私有项，搬走之后那个模块每一句都在往回够根模块。
  而 `cache` / `hysteresis` / `envelope` 是三个自成一体的算法，接口都很窄。
  窗口与 `Compute`、`first_pass` 是同一件事的三段，眼下摆在一起。
- **What this ticket actually did:** 留在 `lib.rs`，紧挨着 `Compute` 与 `first_pass`。
- **Options:** ① 照现在；② 开 `src/window.rs`，把那五样提成 `pub(crate)`；
  ③ 连 `Compute` / `first_pass` / `second_pass` / `OutputPage` 一起搬成一个 `src/pipeline.rs`。
- **Recommend:** ③ 才是真正解掉「`lib.rs` 三千行」的那一刀，而它是一次纯搬家、
  该单开一票（本票的 diff 已经一千三百行）。②只搬一半，换来的是一条新的跨模块依赖。
- **Whose call:** 拍板的人
- **处置：** **`one-source/01` 落地（2026-10-01）：照 ③ 走。**`Compute`／`first_pass`／`second_pass`／`OutputPage` 连同
  幂等这一道、汇总、`Settles` 与开卷之前那一问搬进 `src/pipeline.rs`；`Window`／`Windowed`／`Seat` 在 `8b38430` 已删，
  不在这一刀里（Q1067）。边界画在哪、`process_volume` 为什么留在根模块，见 Q1068。

#### Q545 — `pinned_up_front` 那个三态是嵌套 `Option`，而它的正确形状本票刚在下游造出来

- **From:** 票 `two-pass-rework/06`（`/code-review` 的 Standards 轴指出）
- **Kind:** 落地时走的那条路（一个已有类型的形状，本票没改它）
- **Where:** `src/lib.rs` 的 `pinned_up_front(...) -> Option<Option<Candidate>>`
  与它的两个消费者 `Window::open`、`pinned_for_the_first_pass`；本票新造的 `enum Settles`；
  停车场 **Q431**
- **Why it did not block:** 那三态读作「答不出来 / 答得出而没顶死 / 答得出是这一档」，
  **本票新造的 `Settles`（`AfterTheVolume` / `InTheWindow` / `UpFront`）正是它的形状**，
  却建在下游——上游仍是嵌套 `Option`，两个消费者各解一次、解法还不一样：
  `Window::open` 从前写 `!= Some(None)`（单看读不出意思），`pinned_for_the_first_pass`
  写 `.flatten()`。函数是 `12` 号票为滚动窗口写的，本票只是给那个答案添了第二个消费者；
  **改它的返回类型是改一个上游类型，不在本票的爆炸半径里**。
- **What this ticket actually did:** 类型没动。`Window::open` 那句 `!= Some(None)` 换成了
  三支写开的 `match`，每一支带自己那句理由——含义从此在代码里而不只在文档里。
  `pinned_ahead` 这个名字改成 `pinned_for_the_first_pass`：它与 `pinned_up_front`
  在英文里是近义词，同一个文件里读不出分别（同一轮审查指出）。
- **Options:** ① 照现在（嵌套 `Option` ＋ 两处各自写开）；② 给 `pinned_up_front` 一个具名
  三态枚举，`Settles` 直接从它派生；③ 连 `Window` / `Windowed` / `Seat` / `pinned_up_front`
  一起搬进一个 `src/window.rs` 或 `src/pipeline.rs`，那时顺手改类型。
- **Recommend:** ②，而它该跟着 **Q431**（那一簇三百行想搬出 `lib.rs`）一起做：
  单独改一个返回类型只动两个调用点，收益是「`!= Some(None)` 这种写法再也写不出来」；
  与搬家一起做则一次到位。**两条记的是同一簇代码，settle 时并在一起看。**
- **Whose call:** 拍板的人（与 Q431 合并处置）
- **处置：** **`one-source/01` 落地（2026-10-01）：照 ② 走，与 Q431 一起做。**`pinned_up_front` 交出具名三态
  `UpFrontAnswer`（`CannotTell`／`NotPinned`／`PinnedAt`），`Settles::if_processing` 从它派生、一处解完；
  两个消费者里 `Window::open`、`pinned_for_the_first_pass` 已随滚动窗口删掉（Q1067），今天只剩这一处。
