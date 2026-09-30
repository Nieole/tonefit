# 04 — 一张跨页出两叠

**What to build:** 样张按**输出页**分组，不按源页（spec《Implementation Decisions》第六条）。
拆跨页把一个源页变成两个输出页（《来路》），因此一张跨页得到**两叠**——
用户看的是真会被写出去的那两张，而不是一张没人会看到的整页。

每一叠的文件名带着它是那一族的第几张，与 `run` 给的输出页名同一套写法。
`--no-split` 时同一张跨页出一叠。找不到装订沟的**连续跨页**照旧不切，也出一叠。

每一半**各自裁过白边**——逐页各裁各的，不取卷级裁切框，这是 `run` 那一侧已有的形态，
样张照搬，不另起一套。

**Blocked by:** 02 — 一张普通页出一叠样张

**Status:** resolved

- [x] 一张跨页出两叠，各自的输出页名与 `run` 给的**同一套**（带着那一族的第几张）
- [x] `--no-split` 时同一张跨页出一叠
- [x] 找不到装订沟的连续跨页出一叠，且交出来的数据说得出它**没被切开**（是连续跨页，不是没判成候选）
- [x] 每一半是各自裁过白边的：两半的裁切窗口各是各的
- [x] **神谕用例在跨页上跑一遍**：两半各比一次，各自与 `run --no-metadata` 写出的那一半逐字节相同
- [x] stdout 上两叠分得开，读的人看得出哪一叠是哪一半
- [x] `cargo xtask gate` 三条全绿——这台 macOS 上读作「除了基线就红的那一条，没有新增的红」，见《数》与 Q995

## 停车场结转

下面两条由停车场转来（都是指明归本票判的：Q996 由 `proof-sheet/02` 记，Q1014 由 `proof-sheet/03` 记）。

#### Q996 — 样张对切出来的**每一块**都出一叠：跨页在本票上已经出两叠，而跨页归 04

- **From:** 票 `proof-sheet/02`
- **Kind:** 票面没想到的第三种情形（两张票的边界）
- **Where:** `src/proof.rs` 的 `write`（对 `Pieces::Gray` 逐块 `examine_gray_page`）；
  票 `proof-sheet/04`；spec《Implementation Decisions》第六条
- **Why it did not block:** 本票只验普通页（一块），跨页怎么出、怎么印、神谕怎么比，四条验收都在 04；
  这里走哪条路都不写出错的东西。
- **What this ticket actually did:** 逐块出。前半截提出来之后交出来的就是一串块（`Opened::pieces`），
  逐块走下去是它最直的用法；文件名照 `run` 的成员名取（`output_name`），切开的两半因此自带 `-1`／`-2`。
  **一条用例都没为跨页写**，stdout 上两叠也还没分开说（04 的第六条验收）。
- **Why it matters:** 04 接手时这条路已经是通的：它要补的是验证与措辞，不是结构。要是拍板的人想让 02 严格只收普通页，
  眼下一张跨页会不声不响出两叠，而没有一条用例替它作保。
- **Options:** ① 逐块出（已落地），04 补用例与 stdout；② 02 只收一块，切出两块时当场说「跨页归 04」，04 再拆掉那一句；
  ③ 只出第一块。
- **Recommend:** ①。② 是写一句注定要删的拒绝，③ 会交出一叠名字对、内容却只有一半的样张。
- **Whose call:** 04 号票的实现者（记下即可）
- **处置：** **04 号票判（2026-09-30）：照 ① 走。**逐块出的那条路 02 已经落地，本票补的是验证与措辞，逐块出的结构没动：
  一张跨页出两叠、每一叠的名字从 `run` 写出的那个成员名推（`a_spread_gets_one_proof_page_per_half_under_the_names_run_gives_them`）、
  关掉拆分一叠、连续跨页一叠且说得出没切开、两半各裁各的、神谕两半各比一次，五条都在 `tests/proof.rs`，逐条按反跑过
  （见《落地记录》）；`proof::write` 里逐块出那一段旁边留了指回本条的注释。stdout 上两叠靠每一段打头的几何那一行分开——跨页那一格说是哪一半，行尾判定那一张的文件名带着
  那一族的第几张——两样都是那一行的既有格子，没添新措辞（`the_proof_note_tells_the_two_halves_of_a_spread_apart`）。

#### Q1014 — 一张图切成几块、门一块成立一块不成立时，样张判「顶死没有」按块问，转换那一趟按「其余页那一组」问

- **From:** 票 `proof-sheet/03`（评审 Spec 轴提的）
- **Kind:** 路过发现（`proof-sheet/02` 起就在，本票照原样保留）
- **Where:** `src/proof.rs` 的 `verdict`（`pinned(request, &scores)`，拿的是这一块自己的那一套）；
  `src/lib.rs` 的 `summarize_volume`（`pinned(request, scores(first))`，拿的是其余页那一组的头一页）；票 `proof-sheet/04`
- **Why it did not block:** 本票的对象是一张普通页，一块就是整卷，两种问法答案相同；
  分岔只在跨页切成两半、两半的门又不一样时出现，而跨页的用例与神谕归 04。
- **What this ticket actually did:** 照 02 的问法，一块一问，没动。例：`--fit inside --bit-depth 1` 上一张跨页，
  门不成立的那一半在样张上是 `1bit`（覆盖），转换那一趟判的是 `1bit`（判出来的，其余页那一组还剩抖与不抖两个）——
  **字节相同，`verdict()` 的理由不同**。
- **Why it matters:** 04 的神谕要在跨页上比判定（02 那一条比的是 `verdict()` 整个），撞上这一角就红，
  而红的原因不在跨页本身。
- **Options:** ① 04 落地时照转换那一趟的问法改：一张图的几块先分出其余页那一组，拿它问一次「顶死没有」，
  最好与 `summarize_volume` 共用那一段；② 照旧一块一问，04 的神谕在这一角只比字节、不比理由
- **Recommend:** ①。样张说的是「这一张图摆成一卷、转换那一趟会怎么判」，理由也在这句话里；
  ② 等于承认样张上那一格可以说一句转换那一趟不会说的话。
- **Whose call:** 04 号票的实现者
- **处置：** **04 号票判（2026-09-30）：照 ① 走，分组那一段与 `summarize_volume` 共用。**「按门分出其余页那一组」
  连同「在那一组的头一页上问顶死没有」一起提成 `GateGroups`（`src/lib.rs`：`GateGroups::of` 分组、`GateGroups::pinned` 作答），
  转换那一趟汇总一卷、样张判一张图（`proof::verdicts`）都调它；样张先把一张图的几块全量完，再问那一次，
  每一块拿自己那几格画质分判（`decide::decide`，本来就共用）。
  钉着的是 `a_single_override_on_a_spread_whose_halves_part_at_the_gate_is_judged_as_run_judges_it`（`--fit inside --bit-depth 2`，
  两半的门分家）：改之前红在左半的理由上——样张 `Override`、转换 `NoneWithinThreshold`，候选同为 `2bit`，字节相同。
  转换那一趟零行为改变（黄金快照与窄计数器见《落地记录》的《数》）。「其余页」这个说法在默认路径上与词条对不上，
  记进 **Q1025**。

## 落地记录

**本票做了什么。** 结构是 02 就落了地的（Q996）：前半截提成 `open_source_page` 之后，样张对切出来的每一块各出一叠，
名字照 `run` 的输出页名取（`output_name`）。本票补三件：

1. **用例**（跨页上一条都还没有）：`tests/proof.rs` 六条（验收前五条各一条，加 Q1014 那一条），`src/render.rs` 一条（stdout 两叠分得开）。
   跨页夹具新造一张 `lopsided_spread`：两半各是一块纸白离格的内容，**只有左半上下留着白边**——整页那一道裁白边一行都拿不走，
   左半那两截要「每半再裁」才收得走，两半的裁切窗口因此不一样高；`--fit inside` 下左半比面板小、门不成立，右半贴得住。
   每一半都真的裁、缩、提白，神谕那一条先断言这三件再比字节。
2. **收 Q1014**：「按门分出其余页那一组、在头一页上问顶死没有」连同分组一起从 `summarize_volume` 提成 `GateGroups`
   （`src/lib.rs`），转换那一趟与样张（`proof::verdicts`）都调它。`proof::write` 先把一张图的几块全量完，再一起判；
   原来的 `verdict` 拆成 `judged_scores`（这一块判定从哪几格里挑，互锁 ③ 那句拒绝仍在这里）与 `verdicts`（整张图一起问顶死，逐块 `decide`）。
   转换那一趟零行为改变：`summarize_volume` 只换了分组的写法，灰度页上 `gate()` 与 `scores()` 同出一支，次序不变。
3. **文档**：`write_proof` 补「跨页两半各一叠、关掉拆分或连续跨页整页一叠、页那一截是 `run` 的输出页名」与「顶死问其余页那一组」两句；
   `proof_note` 补「两半各占一段、靠几何那一行分开」一段。

**验收第六条的读法。** 「stdout 上两叠分得开，读的人看得出哪一叠是哪一半」。spec 第九条要措辞从既有出处取，
而每一叠打头的几何那一行已经有两样说得出：跨页那一格（`Cut` 的 `Display`，「跨页右半」）与行尾判定那一张的文件名
（`001-1.4bit+FS.png`，带着那一族的第几张）。两条路里只有这一条同时满足两条验收，所以不记停车场：
用例钉的是「每一叠的抬头说得出是哪一半、它那几行全落在自己的抬头与下一叠的抬头之间」，没新写一句。

**产物一个字节没变。** `git diff 3350886 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
两批用例都在最终那一趟里跑绿（见《数》）。

### 按反跑过的几遍（改完都还原了）

| 按反 | 结果 |
|---|---|
| 样张把切出来的几块倒过来排（`.rev()`） | 红：名字那一条（这一叠的切口是左半、转换那一趟排在同位的是右半）、各裁各的那一条（读到 `(1100×1000, 1100×1800)`）、神谕那一条（左半的理由对不上） |
| 每一叠都当独一张起名（`output_name(name, 0, 1)`） | 红：名字那一条（`001.1bit.png` 对 `001-1.1bit.png`）、神谕那一条（后一叠盖掉前一叠：119982 对 73257 字节） |
| 样张不认 `--no-split`（拆分恒开） | 红：关掉拆分那一条（出了两叠） |
| 样张丢掉跨页候选那一格（恒为假） | 红：连续跨页那一条 |
| 「每半再裁」关掉（`gray_pieces` 里那一刀取 off，两条路一起） | 红：各裁各的那一条（两半都是 1120×1800） |
| 样张一块一块问顶死（改之前的形状；改完又在新结构上按反一次） | 红：Q1014 那一条，两次都红在左半的理由上（样张 `Override`、转换 `NoneWithinThreshold`，同为 `2bit`） |
| `GateGroups::of` 把两组对调 | 红：Q1014 那一条，红在**前提**上（转换那一趟的左半也成了 `Override`）——分组只剩一处，两条路一起动，神谕的等号钉不住它，钉住它的是前提那一句 |
| `proof_note` 先印两叠的几何、再印逐张 | 红：stdout 那一条（头一叠第 5 行落到了两个抬头第 1、2 行之后） |

### 用例（两条闸门各多 7 条）

- `tests/proof.rs`（11 条到 17 条）：一张跨页两叠、名字从 `run` 的成员名推；关掉拆分一叠（判定那一张比字节）；
  连续跨页一叠、跨页候选为真而切口为空（与转换那一趟逐格相同）；两半各裁各的（裁完恰是自己那块内容，与 `run` 的窗口逐格相同）；
  神谕两半各比一次（判定与字节）；门分了家、只点 `--bit-depth` 一维时两半的判定与转换那一趟相同（Q1014）。
  夹具 `Plain` 改名 `Staged`（现在也摆跨页），拆分那三格点名取值（判定宽度 1.5、右开）。
- `src/render.rs`（1 条，在 bin 的数里）：`the_proof_note_tells_the_two_halves_of_a_spread_apart`。
- lib 一条没多：`GateGroups` 的两种分法由转换那一侧的既有用例与 Q1014 那一条一起钉着。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 3350886`。

**收下的**：

- **「取其余页那一组的头一页问顶死」两处各写一遍**（两轴都提了；Spec 读成 Q1014 ① 只做了一半）。第一版只共用了分组（`by_the_gate`，
  交回一对 `Vec`）；改成 `GateGroups`，分组与那一问（`GateGroups::pinned`）都在它上面，两个 `Vec` 有了名字（`rest`、`outside`）。
- **神谕那一条的前提 `crop.trimmed()` 在半页上恒真**（Spec）：半页的窗口叠在整张源页上，切开那一刀就让它答是。
  改成「裁完恰是这一半自己那块内容」。
- **两条用例靠默认的阅读方向排两半**（Spec），**render 那一条借 `SplitRule::default()`、同一个数写了几遍**（Standards）：
  `Staged` 与 render 那一条都把拆分三格点名；render 那一条的几个数各留一个常量。
- **旧称「装订沟」**（Standards）：改《中缝》。**测试名里的 `stack` 不在词汇表**：改 `proof_page`（《样张》词条的 `ProofPage`）。
- **`Staged::holding` 与分组里的 `holding` 撞义、`judgeable` 是个形容词**（Standards）：改 `Staged::of`、`judged_scores`。
- **Q996 拍了板、代码旁没留指向**（Standards）：`proof::write` 逐块那一段旁边补了。**Q1025 没说差在停线哪一条**：补上。
- **Q996 的处置写「六条」而只列了五条**（Spec）：改成五条，第六条归 Q1014。

**驳回的**，各写理由：

- **逐块 `decide` 那一步两处各写一遍**（Spec）。驳：那一步就是一句 `decide::decide(scores, threshold, pinned)`，出处在 `decide`；
  两边迭代的序列不同（一卷的灰度页序号、一张图的几块），收成一个函数要交回一张按序号写的判定表，多一层间接换不回什么。
- **`(GeometryGate, &[CandidateScore])` 成对地走**（Standards，Data Clumps）。驳：成对的那一团就是 `Examined`（门与画质分同在），
  `verdicts` 收的是从它筛出来的一对，只在一个函数里走一趟；再立一个两格的类型不值。
- **「块」撞上《分块 (Tile)》**（Standards）。驳：`Piece`／`Pieces`／「每一块」是 02 把前半截提出来时就立下的说法，本票沿用；
  改说法是那一层的事，不在本票。
- **render 那一条的 `CacheBudget::default()`**（Standards）。驳：卷级那一格样张一格都不读，它的取值不进任何断言（用例里的注释写明了）。

### 停车场

本票记 **Q1025** 一条（分到的 Q1025–Q1039 只用了这一个号）：《其余页》词条说它只描述 `--envelope` 那条路，
而「覆盖项顶死没有」在每一条路上都问「其余页那一组」。

结转的两条：**Q996** 照 ① 判、**Q1014** 照 ① 判，处置写在上面《停车场结转》那一节，停车场《已了结》那张表里指回本票。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `ps-04.gate1.log`、`ps-04.gate2.log`、`ps-04.gate3-polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs:685` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
因此照 02、03 的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红；这一条在最终那一趟里照旧只红它自己。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_NFF_EXIT=101`；合计 **1020 通过 1 失败**；lib 239 / bin 422；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.74s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_NFF_EXIT=101`；合计 **905 通过 1 失败**；lib 239 / bin 307；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 87.37s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.64s` |

**基线**是 `proof-sheet/03` 落地那一刻（`7d094f0`，`3350886` 是它的合并提交，树相同）：闸门 1 **1013 通过 1 失败**
（lib 239 / bin 421），闸门 2 **898 通过 1 失败**（lib 239 / bin 306），闸门 3 绿，红的是同一条。
**两条闸门各多 7 条，都在预期里**：`tests/proof.rs` 从 11 条到 17 条，bin 多 1 条（`src/render.rs` 的 stdout 那一条）。lib 239 一格没动。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 186.02 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 同上。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 03 同数，逐条都是既有的 `links to private item`，本票没添一条）。

**命令行真跑过一遍**（最终那一版，生成的 3024×2160 带中缝跨页，`--profile "Kobo Libra 2"`）：
去处里十四张（两半各六档加参照），退出码 0；两段各由几何那一行打头，分别是
`1300x1680  裁白边 3024x2160 ⟶ 1671x2160 · … · 跨页右半`（判定那一张 `001-1.4bit+FS.png`）
与 `1021x1680  裁白边 3024x2160 ⟶ 1313x2160 · … · 跨页左半`（`001-2.4bit+FS.png`），逐张那几行各在自己那一段里。
