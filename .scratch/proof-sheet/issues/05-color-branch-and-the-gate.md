# 05 — 彩页与尺寸未贴合屏幕：候选少了或没有时说得出为什么

**What to build:** 有两种页拿不到整叠样张，而两种的理由完全不同。**两种都要说出口**——
不说，用户会以为是工具漏了几张。

- **彩色面板上的彩页走彩色分支**，不量化（ADR 0005 决定第 4 条）：没有候选、没有画质分。
  那一趟只出**一张**——`run` 会写出的那一张——并说清这一页走的是彩色分支。
- **黑白面板上同一张彩页转灰**，照灰度路径出整叠。走哪条由面板与页**共同**决定，
  样张照搬这条规矩，不自己另判一次。
- **尺寸未贴合屏幕的页**（ADR 0007）候选里没有抖动那一维：出得来的样张因此少几张，
  而交出来的数据要说得出**门为什么不成立**。

**Blocked by:** 02 — 一张普通页出一叠样张

**Status:** resolved

- [x] 彩色面板上的彩页只出一张（`run` 会写出的那一张），没有候选、没有画质分；交出来的数据说得出走的是彩色分支
- [x] 黑白面板上同一张彩页转灰，出整叠，与灰度路径上的普通页同形
- [x] 尺寸未贴合屏幕的页：候选里没有抖动那一维，交出来的数据说得出门不成立
- [x] 那两种情形在 stdout 上**各有一句话**，读的人不会以为工具漏了几张
- [x] **神谕用例覆盖彩页那一张**：与 `run --no-metadata` 写出的那一张逐字节相同
- [x] 神谕用例覆盖一张门不成立的页：候选集与 `run` 在同一页上用的那一套相同
- [x] `cargo xtask gate` 三条全绿——这台 macOS 上读作「除了基线就红的那一条，没有新增的红」，见《数》与 Q995

## 落地记录

**本票做了什么。** 三件，库、命令行文案、用例各一件：

1. **彩色分支换掉那句 `bail!`**。`src/proof.rs` 的 `write` 不再只认 `Pieces::Gray`：分流照旧只在 `open_source_page` 里问一次
   （面板与页共同决定，样张照它交出来的那一支走），灰度那一支是原来那一段（提成 `draft_gray`），彩色那一支是新的
   `draft_color`——目标尺寸走 `FitMode::target`，缩放与编码走新提出来的 `color_bytes`（`src/lib.rs`，
   `Compute::color_page` 照做那一支改调它，记录照旧由调用方备好；样张交 `None`）。那一叠只有一张，名字是
   `001.彩色分支.png`（Q1041）。`ProofPage` 从 `{ page, reference, candidates }` 改成 `{ page, sheets: Sheets }`，
   `Sheets` 两种：`Gray { reference, candidates }` 与 `Color(Sheet)`，跟着 `PageBranch` 走、在落盘那一步同一个分支里造出来；
   `Sheets::iter` 交出一叠落到盘上的每一张。`Candidates::new` 挪到分流之前：越界的灰阶档位那句拒绝，彩色分支上的页也照转换那一趟说。
2. **stdout 上两种各一句**（`src/render.rs`）。彩色那一叠：判定那一行的位置上是报告里逐页那一行说彩色分支的既有一句，
   逐张那一行只有「彩色分支」那一张。门不成立那一叠：尺寸贴合检查那一行底下多一句（`proof_gate_rows`），
   原因那半句与报告里「这几页……」那一句同出 `WHY_THE_GATE_SHUTS`，后半句「样张里因此没有带抖动的候选」是新写的（Q1040）。
   报告那一句的字节一个没变。`proof --help` 补一段说这两种页。
3. **文档**：`write_proof` 补《两种页拿不到整叠》一节；`CONTEXT.md`《样张》补彩色分支那个例外（spec 第七条已定，
   新词 `Sheets::Color` 随落地加，不改词条原有的含义）。

**尺寸未贴合屏幕那一半本来就通着。** 03 起样张出的就是 `Candidate::all(可见灰阶数, 这一页的门)`，门不成立交出来的数据也一直说得出
（`PageReport::gate`）。本票补的是用例（候选集没有抖动那一维、与 `run` 在同一页上用的那一套相同）与 stdout 那一句。

**Q1010 看过，没碰。** 本票没动 `--dither fs` 撞上门不成立那一页的路（`judged_scores` → `Candidates::for_gate`），
那条拒绝照旧；条目原样留在《待处理》。它推荐 ② 时要给 `ProofPage` 加一个「没有判定」的形状——本票加的 `Sheets` 不是它：
灰度那一种仍然必有判定。

**产物一个字节没变。** `git diff 5593a8c -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
两批用例都在最终那一趟里跑绿（见《数》）。转换那一路上的改动只有 `color_bytes` 那一处提取：缩放、编码两段的次序与计时格不变，
编码失败那一层上下文（「编 … 这一页」）一字不改，缩放器仍是这一卷那一个；唯一挪了位置的是记录的构造（缩放之前），它是纯函数、不会失败。

### 按反跑过的几遍（改完都还原了）

| 按反 | 结果 |
|---|---|
| 彩色那一支把缩放算法写死成 hamming | 红：彩页神谕那一条（样张 69536 字节，转换 98712 字节） |
| 分流不看面板（`panel.color && color.is_color()` 改成只问 `color.is_color()`，两条路一起动） | 红：黑白面板那一条（转灰的彩页走成了彩色分支，没有门）——神谕的等号钉不住它，钉住它的是「整叠、有门」那几句 |
| 样张那一套门不成立时也给门成立那一套（`without_overrides` 的 `broken` 取 `Holds`） | 红：门不成立那一条（候选里出现了三个 `+FS`） |
| 越界的灰阶档位那一问只放在灰度那一支里 | 红：彩页越界那一条（样张照出了一张 `001.彩色分支.png`） |

另两条是先红后绿写出来的：彩色面板那一条先红在那句 `bail!` 上（先是编译不过——`Sheets` 还不在），stdout 那一条先红在门不成立那一句上
（彩色那一半一上来就绿：借的是既有一句）。

### 用例（两条闸门各多 6 条）

- `tests/proof.rs`（17 条到 22 条）：彩色面板上的彩页只出一张、没有候选与画质分、数据说得出走的是彩色分支；
  彩页神谕（与 `run --no-metadata` 逐字节相同，前提先问裁过、缩过、留着颜色）；黑白面板上同一张彩页转灰出整叠、与普通页那一叠同名同数、
  判定那一张比字节；门不成立的页（`--fit inside` 上的 `SMALL`）候选里没有抖动那一维、候选集与 `run` 在同一页上用的那一套相同、判定那一张比字节；
  彩色面板上的彩页点了 `--bit-depth 8`，样张与转换那一趟同一句拒绝、去处里一张都没有。
- `src/render.rs`（1 条，在 bin 的数里）：`the_proof_note_says_why_a_color_page_or_a_page_the_geometry_gate_shuts_has_fewer_sheets`。
- lib 一条没多。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 5593a8c`。

**收下的**：

- **测试名里的「outside the gate」**（Standards）：词汇表里「判定范围之外」说的是彩页与坏页，这一页在判定范围里、只是门不成立。
  两条名字改成既有用例的说法 `the_geometry_gate_shuts`。
- **`ProofPage::page` 的注释说分支「不在这里另记一份」，而 `Sheets` 正是按分支分的第二份**（Standards）：注释改成说实情——
  两者一一对应，类型拦不住，拦住它的是构造（落盘那一步同一个分支里一起造）。
- **`CONTEXT.md`《样张》补的那几句里，「门不成立少几张」与同一条里「候选集照这一页自己的门算」是同一件事；
  「各说一句」是界面文案**（Standards）：两句都删，只留彩色分支那个例外。
- **`SMALLER_THAN_THE_SCREEN` 只说出了三截里的头一截**（Standards）：改名 `WHY_THE_GATE_SHUTS`。
- **render 那一条注释写「逐格点名」，卷级那几格却借了默认值**（Standards）：补一句卷级那几格样张不读、取值不进断言
  （与 04 驳回的那一条同一个理由，这回写进注释）。
- **Q1010 没留「看过」的记录**（Spec）：写在上面，条目本身按规矩没动。

**驳回的**，各写理由：

- **`DraftedSheets::Gray` 前四格原样装回 `PageBranch::Gray`，让 `Drafted` 直接持一个 `PageBranch`**（Standards，Data Clumps）。
  驳：那样 `Drafted` 上就是两个都按分支分的枚举（`PageBranch` 与编好的字节），落盘时要两个一起 match、再断言两者对得上——
  正是上面那一条要防的「两份分支对不上」。一个枚举装全，那一步只有一个 match。
- **`Pieces`／`DraftedSheets`／`Sheets`／`PageBranch` 四个平行枚举**（Standards，Repeated Switches）。驳：四个装的是四样东西
  （像素、编好的字节、落到盘上的文件、报告读的事实），分在管线的四个阶段；转换那一路本来就是 `Pieces`／`Branch`／`PageBranch` 三段，
  样张多出来的只是落盘那一段。收成一个带泛型的枚举，换回来的是每一处 match 多一层间接。
- **测试里 `let [ran] = …` 加比字节那两句与既有用例同形，抽个帮手**（Standards）。驳：既有八处都是这个写法，
  每一处的失败措辞各说各的情形（哪一半、顶死那一档、彩色分支那一张）；抽成一个帮手要么丢掉措辞，要么多一个参数，改的还是票外的用例。
- **`request.fit.target(…)` 在 `color_page` 与 `draft_color` 各一份**（Standards，评审自己也判了可接受）。驳：那是调同一个出处，
  不是抄一份；预览那一支只要尺寸、不要字节，收进 `color_bytes` 它就得另算一遍。
- **只有一张彩页时仍印画质门槛那一行**（Spec，小瑕疵）。驳：那一行是设备配置那一行（面板四项、「彩色」、门槛连同来源），
  spec 第九条要它打头；它说的是这块面板，不是这一页。

### 停车场

本票记三条（分到的 Q1040–Q1054 用了前三个）：

- **Q1040**：两种页那一句的措辞——彩色借报告里逐页那一句、门不成立新写后半句（对上 spec 第九条「不新写」）。
- **Q1041**：彩色分支那一张叫 `001.彩色分支.png`，不叫 `run` 写出的 `001.png`。
- **Q1042**：覆盖项越界又点了一张解不开的图，样张先说「解不开」、转换那一趟先说越界——推荐归 06，挪一行。

没有结转的条目：Q1010 看过、没碰（见上）。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `ps-05.gate1.log`、`ps-05.gate2.log`、`ps-05.gate3-polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs:685` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
因此照 02–04 的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红；这一条在最终那一趟里照旧只红它自己。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_NFF_EXIT=101`；合计 **1026 通过 1 失败**；lib 239 / bin 423；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 80.55s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_NFF_EXIT=101`；合计 **911 通过 1 失败**；lib 239 / bin 308；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 61.60s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 3.61s` |

**基线**是 `proof-sheet/04` 落地那一刻（`9b7559e`，`5593a8c` 是它的合并提交，树相同）：闸门 1 **1020 通过 1 失败**
（lib 239 / bin 422），闸门 2 **905 通过 1 失败**（lib 239 / bin 307），闸门 3 绿，红的是同一条。
**两条闸门各多 6 条，都在预期里**：`tests/proof.rs` 从 17 条到 22 条，bin 多 1 条（`src/render.rs` 的 stdout 那一条）。lib 239 一格没动。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 209.15 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 同上。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 04 同数，逐条都是既有的 `links to private item`，本票没添一条）。
