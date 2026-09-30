# 03 — 处理选项与 `--preset` 吃与转换同一套

**What to build:** 改完 `--filter` 或 `--white-align-limit`，出一叠样张就**立刻看得见它改了什么**。

「样张等于产物」这句话只在两边吃同一套选项时才成立，因此样张收下转换那条路的十项处理选项
与 `--preset`。两处读法要单说：

- **`--bit-depth` / `--dither` 不裁样张的候选集**（spec《Implementation Decisions》第三条）。
  它们裁掉的是「这一趟不要」，不是「这一页不可能」；而样张存在的理由正是并排看。
  照出整套，另外说出判定被顶死成了哪一档、理由是 `Override`。
  屏幕灰阶数与尺寸贴合检查那两道照裁——被它们裁掉的候选本来就永远不会被写出去。
- **与卷有关的那几项点到时说得出为什么**：`--dry-run` 与「出样张」自相矛盾，
  `--envelope` 要整卷，`--brief`／`--io-mode`／`--cache-budget`／`--no-metadata` 在一张图上无从谈起。

**Blocked by:** 02 — 一张普通页出一叠样张

**Status:** resolved

- [x] 十项吃得下：`--fit`、`--no-crop`、`--no-split`、`--split-threshold`、`--reading-order`、`--filter`、`--white-align-limit`、`--gray-levels`、`--threshold`、`--preset`
- [x] `--preset` 套得上，**命令行上显式点到的那一项赢**（转换那条路有现成的用例形状）
- [x] `--bit-depth` / `--dither` 点到时候选集**照旧是整套**，判定那一格标着被顶死、理由是 `Override`
- [x] 与卷有关的那六项点到时各说得出为什么，六句各有各的话、不是一句通用的
- [x] 解析出的面板与转换那条路**是同一块**（`calibrate` 已有一条同样的用例）
- [x] `--help` 说得出样张答的是哪一问、怎么在真机上读它（`calibrate` 的 help 有一条同样的用例）
- [x] 神谕用例在**非默认选项**上再跑一遍：至少覆盖 `--fit inside` 与一个非默认 `--filter`，两边吃同一套，判定那一张照旧逐字节相同
- [x] `cargo xtask gate` 三条全绿——这台 macOS 上读作「除了基线就红的那一条，没有新增的红」，见《数》与 Q995

## 停车场结转

下面这一条由停车场转来（`proof-sheet/02` 记的，指明归本票判）。

#### Q999 — 命令行这一侧「处理选项 → `Request`」那份映射有了第三份（`proof_request`）

- **From:** 票 `proof-sheet/02`
- **Kind:** 评审提出、我拿主意的单项（Standards 轴：Duplicated Code／Shotgun Surgery）
- **Where:** `src/main.rs` 的 `proof_request`；另两份是 `Cli::request` 与 `session::state::Session::request`
- **Why it did not block:** 三份各自落到同一处默认值上（`preset::TasteLayer` 那几个方法），不会分家出不同的默认值；
  分家的风险只在 `Request` 多一格时要改三处。
- **What this ticket actually did:** 照 `Session::request` 那一副写了第三份：处理选项一项都没点，逐项落到 `TasteLayer::default()` 上，
  卷级那几格照实填。一条用例（`the_proof_takes_the_processing_options_a_plain_run_takes`）钉着「与一个 flag 都不点的转换那一趟逐项相同」。
- **Why it matters:** `Request` 每多一格处理选项，三处都得跟着加；漏一处，样张与产物就不是同一套选项，而那正是神谕那一条要防的事
  （神谕只在同一份 `Request` 上比，钉不住这一层）。
- **Options:** ① 照旧（已落地），03 接手时收；② 本票就抽一个共用的「`TasteLayer` + 型号 + 卷级几格 → `Request`」，
  会话与样张两处都调它；③ 让 `proof` 子命令复用 `Cli` 那一排 `fit_mode(preset)`……方法，与转换那一路走同一批解析。
- **Recommend:** ③，归 03：它要给 `proof` 接上 `--preset` 与那九项，天然就得走 `Cli` 那一批「命令行赢、预设次之、默认兜底」的方法，
  `proof_request` 那时整个改写；现在做 ② 只是给 03 多一处要拆的东西。
- **Whose call:** 03 号票的实现者
- **处置：** **03 号票判（2026-09-30）：照 ③ 走，走法是把那一排提成一个共用的 `clap::Args`（`PageOptions`）。**
  `--preset`、设备设置三项、处理选项里一张图上谈得上的那几项（缩放方式、裁白边一对、拆分一对、跨页判定宽度、阅读方向、
  缩放算法、提白上限、两道覆盖项）挪进 `PageOptions`，转换那一趟（`Cli`）与样张（`Command::Proof`）各 `flatten` 一份；
  「命令行赢、预设次之、默认兜底」那几个方法随之挂到它上面，两边调同一批——**定义一份、解析一份**，
  样张吃不出另一套值。`proof_request` 仍是自己那一份 `Request` 字面量（卷级那几格两边本来就不同），
  处理选项那几格却每一格都走 `PageOptions` 的方法；`Request` 多一格时编译器照旧逼每一处都填上。
  会话那一份（`Session::request`）不在此列：它没有命令行，直接读 `TasteLayer`。钉着的是
  `the_proof_takes_the_processing_options_a_run_takes`（三种来路：一个 flag 都不点、每一项都点成非默认值、
  套一份说满的预设），整份 `Request` 比（卷级那几格换成转换那一趟的），按反跑过。
  代价是那一排的帮助两条命令共用一份，`proof --help` 里有几句冲着转换那一趟说（记进 **Q1013**）。

## 落地记录

**本票做了什么。** 两件事，库一侧与命令行一侧各一件：

1. **覆盖项不裁样张的候选集**（spec《Implementation Decisions》第三条，收 02 留下的缺口）。`src/lib.rs` 的 `Candidates`
   添 `without_overrides`：只照两道界（屏幕灰阶数、尺寸贴合检查）裁的两套。`proof::write` 手上两套各管一件事——
   出哪几张照它（`shown`），判定从哪几个里挑照转换那一趟那一套（`judged`，即 `Candidates::new`）。
   判定在 `proof::verdict` 里：从整套画质分里挑出转换那一趟会留下的那几格，再走 `decide` 与 `pinned`，
   与转换那一趟逐格相同。覆盖项越界、互锁 ③ 的那两句拒绝也由 `judged` 照转换那一趟说。
   为了让撞上门的那一页两边听见同一句，`Candidates::for_gate` 收一个 `page`，把「某页关上了尺寸贴合检查」
   那一层上下文挪进自己里面（`examine_gray_page` 原来在外面套；措辞与错误链逐字不变）。
2. **命令行吃与转换同一套**。`Cli` 里从 `--preset` 到 `--dither` 那一排十五个字段连同它们的解析方法，
   挪进新的 `clap::Args`——`PageOptions`；`Cli` 与 `Command::Proof` 各 `flatten` 一份（Q999 照 ③ 判，见上面结转那一节）。
   与卷有关的那六项挂在 `VolumeOnly` 上（`hide`），点到时由 `VolumeOnly::refuse` 一项一句说为什么；
   整卷统一灰阶那一项连着它的反面 `--no-envelope`（Q1012）。`proof --help` 重写：头一行说它答哪一问，
   正文说怎么在真机上读、吃哪一套、覆盖项怎么读、不收哪几项；末尾挂上《选项冲突》，几项开关帮助里的那句指路在这里才不落空。
   `CONTEXT.md`《样张》补半句：`--bit-depth`／`--dither` 不裁候选集，只管判定从哪几个里挑（落实 spec 第三条，不改词条的含义）。

**验收第三条的读法。** 票面写「`--bit-depth` / `--dither` 点到时……判定那一格标着被顶死」。照《覆盖顶死》那条词条，
「顶死」说的是四道裁剪合起来**只剩一个**。只点一维、门成立时还剩抖与不抖两个，那时判定是画质分挑的，理由不是覆盖；
而且神谕要求判定与转换那一趟相同，转换那一趟在这里也不说覆盖。两条路里只有这一条说得通，所以不记停车场：
两维都点（或一维加一道界裁到只剩一个）时理由是 `Override`（`an_override_pins_the_verdict_and_leaves_every_candidate_on_the_sheets`），
只点一维时判定落在那一维里、理由不是 `Override`（`a_single_override_narrows_the_verdict_but_not_the_sheets`）。
两种情形下候选集都是整套。

**产物一个字节没变。** `git diff fd41d74 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
两批用例都在最终那一趟里跑绿（见《数》）。转换那一路上唯一改动的是 `for_gate` 挪进来的那一层上下文。

### 按反跑过的几遍（改完都还原了）

| 按反 | 结果 |
|---|---|
| 判定照整套挑（覆盖项不进判定） | 红：两条覆盖项用例（顶死那条读到 `4bit+FS`、理由「没有档位达标」；单维那条判到 4bit） |
| 点了覆盖项一律当顶死 | 红：单维那一条（理由读到 `Override`） |
| 样张那一路把缩放算法写死成 lanczos3 | 红：非默认选项那条神谕（样张 70315 字节，转换 66917 字节） |
| 样张那一路把缩放方式写死成 height | 红：非默认选项那条神谕（判定对不上） |
| `judged` 也换成不看覆盖项的那一套 | 红：互锁 ③ 那一条（样张照出三张，判 `4bit`） |
| `proof_request` 的缩放算法／提白上限不走 `PageOptions`、落到默认值 | 红：`the_proof_takes_the_processing_options_a_run_takes` |
| `proof` 里不先挡那六项 | 红：六句那一条（先去读图，报的是「读 卷/001.png」） |
| `proof --help` 放回 02 那一版正文 | 红：帮助那一条 |
| `PageOptions` 那段说明写回 `///` | 红：`flattening_leaves_each_command_its_own_about`（`tonefit --help` 的开头变成了那段说明） |

非默认选项那条神谕的头一版把两条前提（fit-inside 真的起作用、换算法字节真的不同）问在**样张**那一侧，
按反时红在前提上，而不是红在等号上。改成在**转换那一趟**上问前提，按反就红在等号上了。
`VolumeOnly` 那段说明写成 `///` 照绿：它只摊在子命令里，子命令自己那段说明排在它后面，盖不着，于是留着文档注释。

### 用例（两条闸门各多 8 条）

- `tests/proof.rs`（4 条）：两维覆盖项顶死、候选集照旧整套、与 `run` 逐字节相同；单维覆盖项只收窄判定；
  非默认选项（`--fit inside` 加 `hamming`，夹具换成比面板更宽的那一种普通页 `WIDE`）上的神谕；
  `--dither fs` 撞上门不成立的页，样张与转换那一趟说同一句拒绝、去处里一张都没有（Q1010）。
- `src/main.rs`（多 4 条，都在 bin 的数里）：新添预设套得上且显式点到的那一项赢、六项各有各的话、
  `proof --help` 答哪一问怎么读（连同那句假话的反着钉）、`flatten` 没盖掉两条命令的开头。
  改写三条：解析出的面板是同一块（加上设备设置两项覆盖与预设供出的型号）、
  吃同一套（三种来路，整份 `Request` 比，卷级那几格换成转换那一趟的）、命令行这一层走通（加一趟点了覆盖项的）。
  `the_help_folds_every_line_into_the_terminal` 加上 `proof` 的两份帮助。
- `src/render.rs`：样张措辞那一条的 `Request` 改成逐格写出的字面量，不再借 `proof_request` 与默认预设（评审提的），数不变。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff fd41d74`。

**收下的**：

- **`[`Cli::no_device_error`]` 两处失效**（两轴都提了）。改指 `PageOptions::no_device_error`。
- **「吃同一套」那条用例手列了 `Request` 的 8 格**（Standards）。`Request` 多出一格处理选项时它照绿。改成整份比，
  卷级那几格换成转换那一趟的（`with_the_volume_side_of`），按反跑过。
- **两处「处处不同的另一套值」各抄一份**（Standards）。提成 `every_field_otherwise`，转换那一侧与样张那一侧共用。
- **渲染层的用例为拼一个 `Request` 去解析整条命令行，还借了默认预设**（Standards）。改成逐格写出的字面量。
- **`proof --help` 里有几句冲着转换那一趟说**（两轴都提了）。本票自己添的那句「拿它定画质门槛的读法见 `--threshold`」撤了
  （那一条指着 `--dry-run`）。其余三句在共用的帮助里，记进 **Q1013**。
- **一张图切成几块、门又不一样时，样张判「顶死没有」按块问，转换那一趟按其余页那一组问**（Spec）。02 起就在，
  本票的对象碰不上，04 的神谕会碰上。记进 **Q1014**，归 04。

**驳回的**，各写理由：

- **`PageOptions`、`VolumeOnly` 该进 `CONTEXT.md`**。驳：两者是命令行这一侧的搬运结构，装的全是既有词条
  （《预设》《设备设置》《处理选项》里的项），不引入新概念。照 Q921 维持的惯例（`Piece`、`Opened`、`Examined` 都不进），
  在类型旁边的注释里指回出处。02 补进词条的 `WhiteWhenOff` 是一个**状态**，不是这一类。
- **`PageOptions` 这个名字对不上任何一个词条**。收下事实，名字不改：这一刀本来就不落在任何一条既有词条上
  （Q916 的处置写过：「预设 + 设备设置 + 处理选项减去卷级三项」没有现成的词），借一个词条名反而会说谎。
  名字取的是「一张图上谈得上的那一排」，注释说清了里面装什么。
- **`Cli::request` 与 `proof_request` 里 `options.xxx(preset)` 各出现 8 次，该收成一个 `resolve`**。驳：
  收成一个方法就得交回「`Request` 的处理选项那一半」，那是一个新类型，也就是 Q916 判过不取的 ②。
  现在每一格都走同一个方法，`Request` 多一格时编译器会逼两处字面量都填上，整份比的那条用例会逼两边填成同一个值。

### 停车场

本票记 **Q1010–Q1014** 五条：

- **Q1010** `--dither fs` 撞上门不成立的页，样张照转换那一趟整张拒绝（覆盖项已不裁候选集）。
- **Q1011** 预设里卷级那三项，样张不读也不吭声，而命令行上点到同一项要被拒绝。
- **Q1012** `--no-envelope` 与 `--envelope` 同一句拒绝。
- **Q1013** 那一排的帮助两条命令共用一份，`proof --help` 里有三句冲着转换那一趟说。
- **Q1014** 切成几块、门又不一样时，「顶死没有」样张按块问、转换那一趟按组问（归 04）。

结转的一条：**Q999** 照 ③ 判，处置写在上面结转那一节，停车场《已了结》那张表里指回本票。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `ps-03.gate1.log`、`ps-03.gate2.log`、`ps-03.gate3-polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs:685` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
因此照 02 的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红；这一条在最终那一趟里照旧只红它自己。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_NFF_EXIT=101`；合计 **1013 通过 1 失败**；lib 239 / bin 421；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 68.23s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_NFF_EXIT=101`；合计 **898 通过 1 失败**；lib 239 / bin 306；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 70.90s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.60s` |

**基线**是 `proof-sheet/02` 落地那一刻（`a661520`，`fd41d74` 是它的合并提交，树相同）：闸门 1 **1005 通过 1 失败**
（lib 239 / bin 417），闸门 2 **890 通过 1 失败**（lib 239 / bin 302），闸门 3 绿，红的是同一条。
**两条闸门各多 8 条，都在预期里**：`tests/proof.rs` 从 7 条到 11 条，bin 多 4 条（`src/main.rs` 新添 4 条；
`the_proof_takes_the_processing_options_a_plain_run_takes` 改写成 `…_a_run_takes`，数不变）。lib 239 一格没动。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（182.91 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 同上。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 02 同数，逐条都是既有的 `links to private item`，本票没添一条）。
clippy 头一遍报过 `large_enum_variant`（`Command::Proof` 摊进那一排之后比 `Calibrate` 大出几倍），
`options` 因此装进 `Box`，理由写在字段旁边。

**命令行真跑过一遍**（生成的 1000×1500 页、带白边与离格纸白，`--profile "Kobo Libra 2" --filter hamming --bit-depth 2 --dither off`）：
去处里七张（六档加参照），退出码 0；判定那一行是 `判定 2bit（由你指定的选项定死）`，画质分那一串六档都在，
行尾是 `纸色 253 ⋅ 提了 2 级`。`--envelope` 与 `--io-mode serial` 各被一句自己的话拒绝，退出码 1，去处没建。
