# 08 — 提白上限注明未标定，默认路径上目录行那一格不在

**What to build:** **提白上限**：上限非 0 时那一行印「N 级（未标定占位值）」，与 K、细节放宽那两个数同一个待遇；取 0 时照旧「0 级（没开）」。

**目录行那一格**：命令行目录那一行（`--brief`）的基准档分布，默认路径上整格不在——那里每卷都是「逐页」，与卷数那一格说的是同一件事；
`--envelope` 那条路照旧有。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `render` 的用例：上限 0 与非 0 各一条
- [x] `render` 的用例：目录那一行在默认路径上没有基准档分布那一格，在 `--envelope` 上有
- [x] 会话屏上若读到这两处措辞，设计快照照旧绿（会话读不到这两处；设计快照照旧绿）
- [x] 黄金快照为本票的改动显式接受一次，diff 里只有本票那几行（**没有要接受的**：快照够不着命令行报告，见《落地记录》）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 各只红基线那一条 Q995，见《数》）

## 落地记录

**本票做了什么。**

1. **提白上限开着时注明是暂定值**（`src/render.rs` 的 `limit_line`）：非 0 印「N 级（暂定值）」，取 0 照旧「0 级（关闭）」。
   票面字面是「未标定占位值」，取的是同一句里「与 K、细节放宽同一个待遇」那一半——抬头那几行今天说的就是「暂定值」
   （1862ea8 换的大白话），见 Q1387。票面的「0 级（没开）」是同一趟改名之前的字面，以代码为准。
2. **目录那一行的统一档位分布只在 `--envelope` 那条路上在场**：`render::directory` 与 `plain::directory` 多收一个
   `envelope`，`plain::report` 照 `Report::envelope` 原样传；默认那条路上整格不在，跳过、没做成那几卷也不另起一格（Q1388）。
3. **文档跟上**：`Field::Bases`、`Field::WhiteAlignLimit`、`directory`、`base_of`（「会话的目录表也读它」本来就不成立，一并改掉）、
   `plain` 模块文档；几处把那一格说成「没开」的对齐成「关闭」（含夹具 `one_page_report` 上那一句）。
   `--brief` 的长帮助（`src/main.rs`）原说「两项一起点名，读得到的只剩每一枝的分布」——默认那一趟没有分布了，改口并说明分布只在 `--envelope` 那一趟印。
4. **`CONTEXT.md` 没动**（spec 的《词条随票落地》：报告措辞那几件词汇表不动）。
5. **用例**：`a_white_align_limit_that_is_on_is_marked_as_a_placeholder`（上限点名取 7，格与印出来那一行都比，光秃秃的「7 级」不许再出现）；
   既有的 `the_white_alignment_line_is_there_even_when_the_limit_is_zero` 添一句「关着时不挂暂定值」；
   `the_directory_row_spreads_bases_only_on_the_envelope_path` 走 `plain::report` 的 `--brief` 那一副，两条路各摆那条路上真出得来的卷
   （夹具里各有一卷跳过的），默认那条路的目录行不许再有「逐页」；`the_brief_help_says_what_it_folds_away_and_what_it_keeps` 添正反两句。
   既有用例：`two_branch_report` 两卷判的是整卷统一灰阶、报告上那一格却是 `false`，补成 `true`；
   直接调 `directory` 的那几条（夹具都是整卷统一灰阶的卷）传 `true`；逐字比「上限 4 级 · …」那一条跟着改。

**黄金快照**：`tests/golden.rs` 自己拼行（`fixtures::volume_verdict`），不调 `render`，够不着这两处措辞；`cargo test --test golden`
两条全过，`tests/golden-snapshot.txt` sha256 前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`——没有要接受的，没接受。

**设计快照**：会话不读纸色提白那一行，也不读 `render::directory`；`npm run check`（node 24.16.0）「与库里那一份逐字节相同」，
设计稿与 `tests/fixtures/design/` 一个字节没动。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `limit_line` 先写用例后改实现：红在 `left: Some("7 级")`。
- `plain::report` 把 `report.envelope` 写死成 `true`：目录那一条红在 `"库  3 卷 · 逐页 2 ⋅ 跳过 1"`。
- `directory` 里的条件取反：同一处红。条件恒假（永远不出分布）：红在 `--envelope` 那一半（`"库  3 卷"`）。
- `--brief` 帮助那一句改回旧话：帮助那一条红在反着钉的那一句。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff db8c01b`。

**收下的**：

- Spec：`CONTEXT.md`《尚未确立》添的那一句越了界（spec：词汇表不动），还把 Q1387 未拍板的字面抄进了词汇表——撤掉。
  顺带撞见的细节放宽那一条「连同『未标定』一并印出」与屏上不符，记进 Q1387，没顺手改。
- Standards：`directory` 里「空串表示不在场」又包了一层——改成 `if envelope` 里头再判空。
- Standards：新写的文档里用了旧称「卷表」——改成「卷列表」。
- Standards：「默认每卷都是逐页、与卷数说的是同一件事」那条理由抄了好几处——`Field::Bases`、`base_of` 改成指向 `directory`。
- Standards：用例里闭包参数 `envelope` 遮住同一条用例里调的 `envelope(…)`——改名 `enveloped`。
- Standards：`--brief` 帮助删掉一句假话没留反着钉的断言——添上。

**驳回的**：

- 用例名里的 `bases`（旧称「基准档」）：照的是既有的 `Field::Bases`，改名是另一件事。
- 同一个 7 在期望字串里写了几遍：期望是独立的字面（防同义反复），与本文件既有那几条（「上限 4 级」）同一个写法。
- 裸 `bool` 参数 / `listed` 与 `envelope` 同出一份报告（Data Clumps）：两处调用方，会话不调它；收成一个类型是另一圈形状。
- 两句反着钉的断言紧跟在逐字比之后、看着冗余：那是 testing.md 第二条要的，逐字比那一句日后被改松时它们还在。
- 用例文档里的「从前」：用例文档说的是它钉住的那句话为什么不许回来，本文件既有写法。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后，依次 polish → 闸门 3 → 闸门 1 → 闸门 2；日志 `ss-08.polish.log`、
`ss-08.gate3.log`、`ss-08.gate1.log`、`ss-08.gate2.log`，都在树外）。这台机器是 macOS，闸门 1、2 在基线上就各红一条：
`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1153 通过 1 失败**；末行 `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.86s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1012 通过 1 失败**；末行 `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 92.73s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 4.60s` |

**本票添 2 条用例**（都在 bin 的 `render` 里，两条闸门各多 2 条），既有 2 条添了断言、1 条改了期望。

`tests/golden.rs` 两条在两道闸门上都过；`tests/golden-snapshot.txt` sha256 动手前后同为
`2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` 绿、零告警；
`cargo clippy --all-targets --no-default-features` 绿、零告警；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q548 — 票面把「报告照旧标着未标定」当成既有事实，而报告那一半从没落地过

- **From:** 票 `tone-alignment/05`（票面第 3 条）
- **Kind:** 票面写错了的东西：把一条从没落地的要求写成了现状
- **Where:** spec 的《Implementation Decisions》第 12 条（「报告里连同来源一并标出，
  记进 `CONTEXT.md` 的《尚未确立》」）；`01` 号票只勾了 CONTEXT 那一半；
  `src/render.rs` 的 `limit_line`（今天印的是「4 级」，一个字不提占位）；
  对照 `src/metric.rs` 印的「K 未标定占位值」与 `src/envelope.rs` 印的「四者均未标定」
- **Why it did not block:** 抬默认值之前那一格印的是「0 级（没开）」——**屏上没有一个活着的数**，
  占位不占位无从谈起。抬完之后每一趟默认跑法都印着「上限 4 级」，而读的人无从知道
  那 4 是全语料分布上的一个断口、真机上一次都没验过。本票的硬约束是「只改一个默认值」，
  往报告里加一句措辞是另一处改动。
- **What this ticket actually did:** 没动报告。票面第 3 条的**实义**（不把它变成测出来的数）照办：
  `CONTEXT.md`《尚未确立》那一条改写之后照旧写着「未标定、占位值」，并补上
  「默认开着不等于这个数被测出来了」与「真机那一问仍然挂着」；`WhiteAlignLimit::default`
  的文档写着同样两句。
- **Options:** ① 照现在（只有 `CONTEXT.md` 与代码文档说得出它是占位值）；
  ② 卷级那一行跟着标——`limit_line` 在上限非 0 时印「4 级（未标定占位值）」，
  与 `K`、掩蔽那两行同一个待遇；③ 只在 `--dry-run` 那一副标。
- **Recommend:** ②。「与 `K`、`MASKING_FLOOR`、`MASKING_KNEE` 同一档待遇」是 spec 写死的话，
  而屏上今天分了两档；改动是一处措辞加一条用例，落点 `src/render.rs` 的 `limit_line`。
  ③ 看着省事，但拿着成品问「这 4 是哪来的」的正是读报告的那个人。
- **Whose call:** 拍板的人（spec 的一条决定没落地，不是实现细节）
- **处置：** 本票落地：上限非 0 印「N 级（暂定值）」（字面取舍见停车场 Q1387）。

#### Q706 — story 18 的病在上一级还活着：目录那一行的基准档分布与总览块的判定行在默认路径上写的仍是「逐页 N」

- **From:** 票 `two-pass-rework/02`
- **Kind:** 本票范围外的同一种病（票面只点名卷表那两列）
- **Where:** `src/render.rs` 的 `base_of`／`base_spread`（目录那一行的 `Field::Bases`：一枝底下各档各有几卷，
  默认路径上每卷都算作「逐页」）；`src/session/draw/overview.rs` 的 `base_name`／`verdict_spread`
  （总览块试算那一副的判定行：「1 卷 逐页 · 1 卷 跳过」，它还是 `base_of` 那五种说法的第二份手抄）
- **Why it did not block:** 票面写的是卷表；目录表是 `volume-discovery/08` 的，总览块是 `no-false-line/04` 的，
  两处各有自己的快照与分组语义（数的是卷，不是页）。默认翻成逐页之后，这两处对每一卷答的都是「逐页」，
  一个目录 12 卷写「逐页 11 · 跳过 1」——那一格与本票换掉的卷表那一格是同一个病，只是高一级。
- **What this ticket actually did:** `base_of` 与 `overview::base_name` 的说法一个字没动，只把 `base_column`
  改名成 `base_of`、缩成私有（读它的只剩目录那一级），并把「跳过／没做成」两个词收进 `why_no_tier`
  给两列共用。
- **Options:** ① 现状——上一级照旧写「逐页 N」；② 目录那一行的分布改数**页**：把一枝底下各卷的档位分布
  合起来（`2bit+FS 2280 页 ⋅ 4bit 31 页 ⋅ 跳过 3 卷`），卷与页两个单位并列在一格里；③ 目录那一行只在
  `--envelope` 那条路上给基准档分布，默认路径上整格不在（一格在不在场本身就是一句话），
  总览块的判定行同理；④ 顺手把 `overview::base_name` 换成读 `render` 那一头，先收掉那份手抄。
- **Recommend:** ③ 加 ④。② 一格里两种单位读不顺；③ 与本票定档页那一列的处置同一条规矩，而且默认路径上
  「逐页 N」本来就与卷数那一格重复。④ 与主意无关，是措辞出处的卫生，哪一票进 `overview.rs` 顺手做。
- **Whose call:** 拍板的人（目录表与总览块各一张票的事，不是一次编辑）。
- **处置：** 本票落地：③——默认那条路上目录行整格不在（连带的取舍见停车场 Q1388）；④ 已随旧界面删掉 `overview::base_name` 不复存在。
