# 02 — 默认路径逐页到底

**What to build:** 默认路径上，**开卷之前答得出**的那一种顶死（两维都点了，两组门的候选集裁到同一个）照旧是覆盖顶死、分析环节就把字节编完；
**答不出的那一角一律逐页判**，不再等整卷判完门再问——那一角的理由从「覆盖」变成逐页那两种、卷级那一行从「覆盖」变成「逐页」；
开滚动窗口、参照不进缓存、写页级依据、按页跳过成立。`--envelope` 那条路不变。

**Blocked by:** 01

**Status:** resolved

- [x] 集成用例：一卷两组门混着、只点了一维覆盖项——报告说「逐页」、每页理由是逐页那两种、缓存里没有参照、每页写了页级依据
- [x] 同一卷改一页之后第二趟只重做那一页（按页跳过）
- [x] 两维都点了的那一趟照旧是覆盖顶死；`--envelope` 那条路一格不变
- [x] 第一遍那份档与汇总那份逐格相同那条断言照旧钉得住
- [x] `CONTEXT.md`《覆盖顶死》加一句「默认路径上只有两维都点了的那一种」；ADR 0005《覆盖项那一档答不出来时让位》改写；`VolumeVerdict::Override` 的文档跟着改
- [x] 黄金快照原样过（黄金里没有这一角）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 默认路径上「覆盖项顶死没有」只认开卷之前答得出的那一种；答不出的那一角（只点灰阶档位、有页门不成立）
与别的默认路径卷走同一条——`Settles::OnItsOwn`（票面「开滚动窗口」的今天的说法，Q1067）。

1. **`Settles::if_processing`**（`src/pipeline.rs`）：`CannotTell` 与 `NotPinned` 并成一支，按 `request.envelope` 分去
   等整卷或这一页自己。默认路径上答不出于是当场编字节、参照不进缓存、写页级依据、按页跳过——四样都跟着这一个谓词
   （`encodes_in_the_first_pass`），没有另改一处。`UpFrontAnswer` 三态照留（Q1077）。
2. **汇总**（`summarize_volume`）：新收 `candidates`，在里面现算 `Settles::if_processing`——顶死就是那一档、这一页自己就是没顶死，
   只有等整卷那条路才整卷判完门问其余页那一组（`GateGroups::pinned`，那条路一行没动）。分析环节编字节用的档与汇总定的档
   因此问的是同一句，`process_volume` 里那条 `debug_assert!` 照旧钉得住（按反跑过，见下）。预览也照照做那一趟问，
   不拿预览自己那个恒为等整卷的 `Settles`（Q1079）。
3. **样张**（`src/proof.rs` 的 `verdicts`，票面没点名）：不再按门分组，直接取 `Settles::if_processing(request, judged).pinned()`。
   不跟着改的话，门不成立的一张图只点灰阶档位时样张说「覆盖」、`run` 说逐页那一种——神谕红。`GateGroups` 只剩汇总一个用处，收成私有。
4. **文档**：`CONTEXT.md`《覆盖顶死》加「默认路径上只有两维都点了的那一种算」（带面板两级那一种的括号，Q1081）；
   ADR 0005《覆盖项那一档答不出来时让位》改写成「答不出时顶死让位：默认路径让给逐页判，`--envelope` 让给整卷」，标题照留
   （Q1081），状态行记一次修订；`VolumeVerdict::Override`、`Request::bit_depth`、`write_proof` 与 `pipeline` 里几处文档跟着改。
   《源哈希》词条与 ADR 0006《决定》末段那半句「只点了一维而两组门混着的那一角要等整卷、按卷」本票之后是假话，删了（Q1078）。
   《其余页》词条一个字没动。

**用例**（`tests/` 三个文件，+3 条、改 1 条）：

- `tests/idempotency.rs` 的 `a_single_override_on_a_volume_whose_pages_part_at_the_gate_goes_page_by_page_all_the_way`：
  票面第一、二条——两组门混着、`--bit-depth 2`：逐页、理由逐页那两种、参照零份、每页页级依据没有卷级；改门不成立那一页，
  第二趟只解一页、留一页，产物与往空去处整卷重做的逐字节相同。对照组 `the_same_single_override_under_the_envelope_waits_for_the_volume`：
  同一卷开 `--envelope`，参照两份、卷级依据、没有页级。
- `tests/pipeline.rs` 的 `a_single_override_on_a_volume_the_gate_shuts_everywhere_is_judged_page_by_page`：门处处不成立、
  `--bit-depth 4`——卷级那一行从「覆盖」变「逐页」真正发生在这种卷上（票面那种混着的卷本票之前就说逐页，Q1080）；
  预览与照做同一句。原先钉着这一卷在默认路径上是覆盖的 `an_override_leaves_no_volume_envelope_to_speak_of` 改成开着 `--envelope` 跑，
  那条路上它照旧是覆盖。
- `tests/proof.rs` 的 `a_single_override_on_a_page_the_geometry_gate_shuts_is_judged_as_run_judges_it`：一张门不成立的图、
  `--bit-depth 2`，样张的判定（档与理由）与 `run` 逐格相同、判定那一张逐字节相同。

**按反跑过的**（`docs/agents/testing.md`）：

- 动手前：门处处不成立那一条红在「理由是覆盖」；混着那一条门那两格、卷级那一行都过了，先红在头一页的页级依据；样张那一条
  在 `run` 改完、样张没改时红在「样张说成了被顶死」。
- 汇总在默认路径上照旧问其余页那一组、分析环节已改：门处处不成立那一条当场撞 `debug_assert!`「分析环节编字节用的档与汇总定的档分了家」
  ——第一遍那份与汇总那份逐格相同那条断言咬得住。
- `if_processing` 不看 `envelope`、答不出一律这一页自己：`--envelope` 对照组红（同样撞那条 `debug_assert!`，整卷统一灰阶把逐页判定改写了）。
- 汇总改问 `for_this_run`（预览那个恒为等整卷的）：门处处不成立那一条只红在「预览预告的卷级判定与照做那一趟不一样」。

**Q1025**（《其余页》词条，待拍板）原文没动。本票没让它更不对，反而收窄了它的前提：它说「覆盖项顶死没有」在每一条路上都问
「其余页那一组」，本票之后只有 `--envelope` 那条路还这样问，默认路径与样张不再分组。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 9e7cf7a`。Spec 轴没找到实现错：逐格核过默认路径三处（分析环节、汇总、样张）在两维都点、
只点灰阶档位、面板两级加 `--dither off`、`--bit-depth` 加 `--dither fs`、只点 `--dither fs`、预览／照做上一致，`--envelope` 那条路没变。

**收下的**：

- 《覆盖顶死》开头说「四道裁剪裁到只剩一个就顶掉、哪一道不影响」，新加那句读着像自相矛盾（Standards）——改成「默认路径上只有……**算**」、
  指到 ADR 0005；`Request::bit_depth`、`write_proof`、`proof::verdicts` 里「裁到只剩一个就顶死」的说法改成指向词条。
- 「只点一维」偏宽（Spec）：只点 `--dither` 是「答得出而没顶死」，答不出的只有只点灰阶档位——几处改成「只点灰阶档位」。
- `compare_with_the_prior_output` 的文档写「`--envelope` 而没顶死」——`--envelope` 上门处处不成立、只点灰阶档位的那一卷报顶死、
  走的仍是卷级（Standards）。改成「开卷之前没顶死」；《页级源哈希》那句同样对不上，记 Q1082、词条没动。
- 单一出处：《覆盖顶死》那一句删掉 `--envelope` 半句、指到 ADR；汇总里那段行内注释收成指回文档；`pinned` 那段收成一句。
- 变更史的字：`UpFrontAnswer` 文档里「（one-source/02 之后）」、ADR 里「照旧是顶死」、用例名里的 `still` 去掉。
- 按位置的引用：用例里两处「见下一条」改成点用例名，ADR 0005 那一节末尾「见下一节」改成点节名。
- 用例注释里写死的「85」改成 `fixtures::NEEDS_TWO_BITS`；那条被 `assert_eq!(PerPage)` 蕴含的 `assert_ne!(Override)` 删掉。
- 汇总里先 `match` 再 `.pinned()` 再 `match` 一次（Repeated Switches）：写成三支一个 `match`。
- Q1080 的选项 ② 像稻草人：换成「只用门处处不成立那一卷一条钉全部」。

**驳回的**：

- 「默认路径逐页判」的结论复述在十来处（Standards 的单一出处）——收了词条与行内注释那几处；`Settles::if_processing`、
  `summarize_volume`、`proof::verdicts`、`VolumeVerdict::Override`、ADR 0005 各留一句：每一处说的是自己那一截为什么这样问，
  删掉就得跳去别处才读得懂这一处的代码。
- `tests/idempotency.rs` 里「有页级、没卷级」那段循环五份（Duplicated Code）：四份是既有的，抽帮手要改别的票的用例，不在本票。
- 模块里四个 `pinned`（Mysterious Name）：三个既有；本票只把 `Settles::pinned` 提成 `pub(crate)`。
- `proof::verdicts` 只剩一次 `map`（Middle Man）：它是 Q1014 那段为什么的落脚处，留着。
- `(request, candidates)` 同行（Data Clumps）：记在 Q1079 的代价里。

### 停车场

本票用了 Q1077–Q1086 里的六个：

- **Q1077**：答不出与没顶死去处相同，`UpFrontAnswer` 仍留三态。
- **Q1078**：《源哈希》与 ADR 0006 那半句本票之后是假话，删了。
- **Q1079**：汇总与样张都从 `Settles::if_processing` 取「顶死没有」，汇总收 `candidates` 自己算。
- **Q1080**：票面那种混着的卷本票之前就说「逐页」，卷级那一行真变的是门处处不成立的那一卷；另添一条。
- **Q1081**：《覆盖顶死》那句带括号、ADR 0005 那一节标题照留。
- **Q1082**：《页级源哈希》「覆盖顶死那一趟有」对不上 `--envelope` 上门处处不成立的那一卷（路过发现，词条没动）。

结转四条（见《停车场结转》）：Q424、Q635、Q683、Q697，本票照上面的做法了结。

### 数

最终状态跑的**那一趟**：评审收完、`cargo fmt` 过之后跑的。日志是 `os-02.gate1.log`、`os-02.gate2.log`、`os-02.gate3.log`、`os-02.polish.log`，都在树外。
这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
所以照 `proof-sheet` 那几张的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1039 通过 1 失败**；lib 239 / bin 423；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 54.48s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **924 通过 1 失败**；lib 239 / bin 308；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.81s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.24s` |

**基线**是 `one-source/01` 落地那一刻（`9e7cf7a`）：闸门 1 **1035 通过 1 失败**（lib 239 / bin 423），闸门 2 **920 通过 1 失败**
（lib 239 / bin 308），闸门 3 绿，红的是同一条。两条闸门各多 4 条，正好是本票新添的四条用例
（`idempotency` 2 条、`pipeline` 1 条、`proof` 1 条）；改的那一条（`an_override_leaves_no_volume_envelope_to_speak_of`）照旧一条。

**两道钉子**：`tests/golden.rs` 2 条全过（闸门 1 上 154.65 秒、闸门 2 上 148.01 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` 的 sha256 动手前后都是 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
这个文件与 `tests/golden.rs` 一个字节没动。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`，末行「全绿。」）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数，逐条都是既有的 `links to private item`，本票一条没添）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q424 — 覆盖项裁到只剩一个候选那一档，两组候选集一长一短时碰卷之前答不出来

- **From:** 票 `two-pass-rework/12`
- **Kind:** 落地时走的那条路（滚动窗口在一种参数组合上让位）
- **Where:** `src/lib.rs` 的 `pinned` 与新加的 `pinned_up_front`；`Window::open`
- **Why it did not block:** `pinned` 拿的是**其余页那一组**的候选集，而哪一组是其余页
  要等整卷判完几何门才知道（一页门成立的都没有时，门不成立的那些就当其余页）。
  滚动窗口要在第一遍里定档，因此要在碰卷之前答出这一问。两组答案相同时与分组无关，
  当场答得出；`--bit-depth 2` 这种一组剩两个（`2bit`/`2bit+FS`）、另一组剩一个（`2bit`）的
  参数上答不出，那一卷于是**不开窗口**，照旧攒整卷参照——与本票落地之前逐字节相同。
- **What this ticket actually did:** `pinned_up_front` 答不出就回 `None`，`Window::open` 跟着不开。
  写进 ADR 0005 新加的那一节《覆盖项那一档答不出来时让位》。
- **Options:** ① 照现在（那一种参数上让位）；② 把 `pinned` 改成只问 `Request` 与两套候选集，
  不再从「其余页那一组」的判据曲线上读——那样它与分组无关，碰卷之前恒答得出；
  ③ 窗口先按「不顶掉」走，撞见第一张门成立的灰度页再收口，一张都没有时整卷回退。
- **Recommend:** ②，但那是**领域决定**：`pinned` 今天那句注释明写「拿门不成立的页去问，
  答案会随卷里第一张灰度页碰巧是哪一种而变」，改它等于改「覆盖项顶掉判定」这句话的定义。
  ③做得到但要在窗口里留一条「回退成攒整卷」的路，为一种参数组合多养一条分支不划算。
- **Whose call:** 拍板的人
- **处置：** **`one-source/02` 落地（2026-10-01）：照 ② 的意思走，落在 `Settles` 上。**默认路径上「顶死没有」只问开卷之前那一问——答不出就逐页判，不再等整卷判完门；`pinned`／`GateGroups::pinned` 只在 `--envelope` 那条路上整卷判完门之后问。滚动窗口早已不在（Q1067），今天对应的是 `Settles::OnItsOwn`。

#### Q635 — 默认路径上「一组顶死、另一组没有」那一角仍攒整卷参照，等的只是理由那一个词

- **From:** 票 `two-pass-rework/11`
- **Kind:** 「默认路径上参照不再进缓存」的一处例外
- **Where:** `src/lib.rs` 的 `Settles::for_this_run`（`pinned_up_front` 答 `None` 那一支）与 `summarize_volume`
  里 `pinned(request, scores(first))`；ADR 0005《覆盖项那一档答不出来时让位》
- **Why it did not block:** 那一角的**字节**两条取法都一样：`--bit-depth 2` 而门不成立那一组只剩 `2bit` 时，
  每一页的候选无论怎么判都是 `2bit`；不同的只有 tEXt 里的理由（`override` 还是 `lowest candidate within threshold`）
  与卷级那一行（`覆盖 2bit` 还是 `逐页`），而理由取决于「哪一组是其余页」——要等整卷判完门才知道。
  06 号票为顶死那一趟定过同一条界，本票照搬，`debug_assert!`（第一遍那份档与汇总那份逐格相同）因此仍钉得住。
- **What this ticket actually did:** 默认路径在这一角退回 `Settles::AfterTheVolume`——照旧攒整卷参照、第二遍编。
  spec P-B 那句「默认路径上参照不再进缓存」在这一角不成立；只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **Options:** ① 保持（理由一处出处，那一角少见）；② 默认路径上 `summarize_volume` 不再问 `pinned`，
  一律 `decide(…, None)`——那一角的理由从 `override` 变成逐页那两种、卷级那一行从「覆盖」变成「逐页」，
  参照于是永不进缓存；代价是「覆盖顶死」这个词在默认路径上只剩两维都点了的那一种，`VolumeVerdict::Override`
  的文档要跟着改一句；③ 第一遍按 ② 编、汇总照旧——**不可取**，那正是 `debug_assert!` 挡的分家。
- **Recommend:** ②，但不在本票做：它改的是《覆盖顶死》词条的射程（`CONTEXT.md`「一个覆盖项都没点而只剩一个时不算」
  那一条旁边要再加一句），按《改 CONTEXT.md 的规矩》先拍板。做起来是 `summarize_volume` 里两行加一条用例。
- **Whose call:** 拍板的人
- **处置：** **`one-source/02` 落地（2026-10-01）：照 ② 走。**默认路径上汇总与分析环节问同一句（`Settles::if_processing`），那一角的理由是逐页那两种、卷级那一行是「逐页」、参照不进缓存；`VolumeVerdict::Override` 的文档与《覆盖顶死》各补一句。`debug_assert!` 照旧钉得住（按反跑过）。

#### Q683 — Q635 那一角（默认路径、一维覆盖、两组门混着）没有页级依据，按页跳过在那一角不成立：改一页整卷重做

- **From:** 票 `two-pass-rework/14`
- **Kind:** 票面「改一页只重做那一页」的射程差——Q635 的直接后件，不是收 Q635
- **Where:** `src/lib.rs` 的 `Settles::if_processing`（`pinned_up_front` 答 `None` → `AfterTheVolume`）与 `process_volume`
  里 `by_page` 的谓词；13 号票在同一谓词上不写页级依据（Q665）
- **Why it did not block:** 谓词一处出处：写不写页级依据与按不按页跳过问的是同一句
  `Settles::if_processing(..).encodes_in_the_first_pass()`——那一角像素只取决于页，tEXt 里的理由那一句
  （`override` 还是 `lowest candidate within threshold`）却由全卷定，留一页就是留一句可能过期的理由，
  13 号票判不写、本票顺着判不留。那一角只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **What this ticket actually did:** 那一角走 `Prior::Nothing`／`Prior::Whole` 两种，与 `--envelope` 同一个待遇：
  整卷跳或整卷重做。没有用例专门钉它——钉它要一卷两组门混着加一维覆盖，与 Q635 的处置绑在一起。
- **Options:** ① 现状，等 Q635 按它的推荐 ②（默认路径不再问 `pinned`）落地，那一角自然消失、页级依据自然成立；
  ② 本票把那一角的谓词单独放开（像素确实只取决于页）——留下的页理由那一句可能与整卷重做写出的不同，
  产物不再逐字节相同，破 Q681 那条性质。
- **Recommend:** ①。
- **Whose call:** 拍板的人（随 Q635）
- **处置：** **`one-source/02` 落地（2026-10-01）：照 ① 了结。**Q635 按 ② 落地，那一角走 `Settles::OnItsOwn`，页级依据与按页跳过随那个谓词一起成立；`tests/idempotency.rs` 有一条钉它。

#### Q697 — 依据的作用域由 `Settles` 定，不由 `--envelope` 定：默认路径上 Q635 那一角仍算、仍记卷级依据

- **From:** 票 `two-pass-rework/15`
- **Kind:** 票面「默认路径不再算、不再记卷级依据」的一处例外——Q635／Q683 那一角的直接后件
- **Where:** `src/lib.rs` 的 `Settles::if_processing`（`pinned_up_front` 答 `None` → `AfterTheVolume`）与 `process_volume`
  里 `by_page = walks.encodes_in_the_first_pass()`；`src/metadata.rs` 的 `SourceHash`；`CONTEXT.md`《源哈希》
- **Why it did not block:** 谓词一处出处：13 号票写不写页级依据、14 号票按不按页跳、本票按哪种作用域算记比，
  问的都是同一句 `Settles::if_processing(..).encodes_in_the_first_pass()`——「依据的作用域 = 这一页的字节取决于什么」。
  那一角像素只取决于页，tEXt 里的理由那一句（`override` 还是 `lowest candidate within threshold`）却由全卷定，
  留一页就是留一句可能过期的理由（Q683 说过），页级依据在那一角不成立；不给它卷级依据它就永远不跳
  （今天它整卷跳），给它页级依据就破「留下的页与整卷重做逐字节相同」。两条路的依据仍不重叠、不互相兜底：
  一份记录只带一种，谓词一处。那一角只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **What this ticket actually did:** 那一角走卷级那条路（`SourceHash::Volume`）：算、记 `tonefit:source`，整卷跳或整卷重做，
  与 `--envelope` 同一个待遇。票面那一句在这一角不成立，ADR 0006 改写那一段与《源哈希》词条都写明了「问的是字节取决于什么，
  不是开关的名字」。
- **Options:** ① 现状——等 Q635 按它的推荐 ②（默认路径不再问 `pinned`）落地，那一角自然消失，票面那一句随之全部成立；
  ② 谓词改成 `request.envelope`，那一角哪一种依据都不写、永远不跳——多一种「没有依据」的记录形态，且是一处静默回退；
  ③ 那一角放开页级依据（Q683 的 ②）——破逐字节相同。
- **Recommend:** ①。
- **Whose call:** 拍板的人（随 Q635）
- **处置：** **`one-source/02` 落地（2026-10-01）：照 ① 了结。**那一角在默认路径上按页算、记、比；《源哈希》与 ADR 0006 里那半句例外删了（Q1078）。
