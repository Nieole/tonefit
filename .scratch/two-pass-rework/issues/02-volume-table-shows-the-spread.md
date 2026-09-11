# 02: 卷表那两列换成档位分布

**What to build:** 卷表一行说得出这一卷**用了哪几档、各多少页**（形如 `2bit+FS 229 · 1bit 1`），取代 `11` 号翻默认之后那个在默认路径上每行都写 `逐页` 的 `基准档` 格。`定档页` 那一格只在卷级上包络真的参与了这一卷时在场——一格在不在场本身就是一句话。

**Blocked by:** 11 —— 票面描述的「今天」是它落地后的世界（默认路径逐页、`--envelope` 显式打开）

**Status:** resolved

- [x] 卷表给出档位分布，一卷一行
- [x] 上包络那条路上分布集中在基准档，特例页与几何门不成立的页各自在场——分布本身就把「上包络买到了什么、没买到什么」显示出来（**不是「一档全包」**：`src/envelope.rs` 第一句就说这不是整卷一个档）
- [x] `定档页` 只在上包络参与时在场，否则整格不在——**今天已如此**（`0f0931f`），本票动那两列时它照旧
- [x] 跳过的卷、没做成的卷、等答话的卷这三种行首记号与措辞不变
- [x] 横向摆不下时的砍列次序仍然成立
- [x] 表那一副与纯文本那一副共用同一份措辞，两处不各写一遍

## 落地记录

落地于 `tpr/02-volume-table-spread`，基底 92fe5da。

**做了什么**

- **卷表那一列换成档位分布。** `render` 多一种行 `RowKind::Tally`、一格 `Field::Tally`，由 `tally_row` 从**页上写着的判定**
  （`PageReport::verdict`）数出来：哪几档、各多少页，页多的档在前、一样多的按头一次出现的先后
  （`2bit+FS 2 ⋅ 4bit 2 ⋅ 1bit 1`）。彩页与失败页没有判定，不在里面；一页判定都没有（跳过的卷、整卷彩页／整卷失败）
  就没有这一行——一格在不在场本身就是一句话。它接在卷级判定后面（上包络那一路在定档页之后），两条路上都在。
- **两副排版读同一格。** 纯文本那一副多一行 `  档位分布 …`（`plain::line` 一条新臂，`_` 照旧不留）；
  卷表那一列改问 `render::tally_column`——分布那一格原样摆上，没有那一行的卷写「跳过」「没做成」，
  两个词收进 `why_nothing_judged`，与目录那一行的基准档分布共用一处。逐页表钉住的抬头跟着换成「档位分布 … · 定档页 … · 要紧的页 …」。
- **上包络那一路不是「一档全包」**：分布从页上数，基准档那一档最多，特例页与几何门不成立的页各自在场
  （用例：`4bit+FS 3 ⋅ 8bit 1 ⋅ 4bit 1`）。覆盖顶死的卷每页同一个候选，分布只有一档（停车场 Q705）。
- **定档页照旧只在 `--envelope` 那条路上在场**：`VolumeColumn::Driver` 一格字都没有时由 `columns::fit` 第零步整列让掉，
  连列头都不占——这一条本票没动，只在文档与用例上钉了一句（`six_kinds_of_volume_each_get_their_own_row` 第三条）。
- **砍列次序重新想过**：`耗时 → 定档页 → 页数 → 档位分布`。分布两档就二十格，恒在的话卷名会被收窄到一个省略号；
  它排在收窄卷名之前让掉，让掉之后跳过／没做成两个词由行首记号接住（Q707）。80×24 那一档上分布反而摆得下了，卷名不再省略。
- `base_column` 改名 `base_of`、缩成私有：读它的只剩目录那一行的基准档分布（`Listed::base`）；
  `base_spread` 与 `tally_row` 共用 `tallied`（多的在前那套排法只有一处）。
- **夹具**：`fixture::overridden_volume` 那一页的判定改成被覆盖成的候选、理由「覆盖」——从前页上写 4bit、卷级说覆盖成 2bit+FS，
  是一份自相矛盾的报告。
- **快照**：`draw/report.rs` 11 张、`draw/yielding.rs` 1 张按实际成屏重录，逐张读过：列头「档位分布」、
  分布那一格、96 列双栏那一档耗时让掉（Q708）、80×24 那一档卷名不再省略。
- **文档**：`CONTEXT.md`《卷表》词条改列名与两条路的说法，新词条《档位分布 (Tally)》；ADR 0016 决定 4 那句「卷表要照它写基准档那一列」
  改成目录那一行；`table.rs` 模块文档的示例表换成默认路径那一副。黄金回归一字未动。
- **Q636 顺手改掉**：`draw/overview.rs`、`draw/pages.rs` 两处 `--per-page` 注释改成「默认逐页」。

**评审**（`/code-review`，两轴并行、只读，基底 92fe5da）收下的：票文件的《落地记录》与《停车场结转》（评审时还没写，Q636 结转一度悬空）；
ADR 0016 决定 4 里那句变更史删掉、只留当前成立的一句；`why_no_tier` 改名 `why_nothing_judged`（「tier」不在词汇表）；
`Field::Base`／`base_of`／`tally_column` 文档去掉「从前…此刻」；七处复述「用了哪几档、各多少页」收成指向《档位分布》词条；
`CONTEXT.md` 词条末句改准（纯文本里它自成一行）；展开一卷的抬头对跳过的卷不再写「档位分布 跳过」、只剩「跳过」一个词；
补一条 draw 层用例 `the_driver_column_is_only_there_when_a_volume_took_the_envelope_path`——全默认那一趟卷表连「定档页」列头都没有，
一卷走上包络它就回来。没收的：`tally_column` 与 `base_of` 同形的 `find_map(match …)`（两个问题、两处各认几种行，再长第三个再收）。

**数**（`cargo xtask gate` 与 `cargo xtask polish` 串行一趟，各自末行 `EXIT=0`；照 `docs/agents/gate.md` 那张《数》抄）

| | 命令 | 结果 |
|---|---|---|
| 闸门 1 | `cargo test`（`target`） | 绿 · 合计 945 通过 0 失败；lib 234 / bin 393 |
| 闸门 2 | `cargo test --no-default-features`（`target/gate/no-default-features`） | 绿 · 合计 799 通过 0 失败；lib 234 / bin 247 |
| 闸门 3 | `cargo check --features profiling`（`target/gate/profiling`） | 绿 · `Finished \`dev\` profile` |
| 收尾 1 | `cargo fmt --check` | 绿 |
| 收尾 2 | `cargo clippy --all-targets` | 绿 · 零告警 |
| 收尾 3 | `cargo clippy --all-targets --no-default-features` | 绿 · 零告警 |
| 收尾 4 | `cargo doc --no-deps` | 绿 · `warning: \`tonefit\` (lib doc) generated 15 warnings`（与落地前同数，15 条都在 lib、本票一个没碰） |

基底 92fe5da 上闸门 1 是 lib 234 / bin 391：多出来的两条是 `render::tests::the_tally_row_counts_the_pages_by_the_candidate_they_were_written_at`
与 `session::draw::report::tests::the_driver_column_is_only_there_when_a_volume_took_the_envelope_path`。
黄金回归（`tests/golden.rs`，48 条那一组里的一条）一字未动、照旧绿。

### 停车场结转

**Q636 了结**：`src/session/draw/overview.rs` 与 `src/session/draw/pages.rs` 两处注释里的 `--per-page` 改成「默认逐页」，
前者那一句同时把「spec 的《卷表》」这个已过期的出处换成「与目录那一行的基准档分布同一套词」。
《已了结》索引表那一行原样留着；条目原文转录于此：

#### Q636 — `src/session/draw/` 两处注释仍写 `--per-page`，本票按票面没进那个目录

- **From:** 票 `two-pass-rework/11`
- **Kind:** 措辞漂移（注释，不是行为）
- **Where:** `src/session/draw/overview.rs`（`--per-page` 与覆盖顶掉判定的照卷级判定说的写）、
  `src/session/draw/pages.rs`（`--per-page` 那一卷没有定档页）；另 `CONTEXT.md`《卷表》词条那一句本票已改成「默认逐页」，
  tpr/02 正在改卷表那两列，合并时可能撞在同一行
- **Why it did not block:** 两处都是注释，编译与行为不受影响；票面明写 `src/session/draw/` 是 tpr/01（屏快照）与
  tpr/02（卷表那一格）的地盘，多动一行就是合并冲突。
- **What this ticket actually did:** 没动那两行注释；`draw/{config,report,yielding}.rs` 里 12 行屏快照期望串按闸门 1 的
  实际成屏把左栏标签「逐页」换成「上包络」（宽度不变，这是闸门变绿所需的最小改动，票面许可的那一种）；
  `render.rs`、`report.rs`、`live.rs` 里同类措辞已改。
- **Options:** ① tpr/02 改卷表措辞时顺手改掉那两处注释；② tpr/01、tpr/02 合进 main 之后本票 rebase 再补；
  ③ 现在改——两行注释，但正撞在别人手里的文件上。
- **Recommend:** ①。那两句说的正是卷表那一格的措辞，归 tpr/02 一并落最顺；② 是兜底。
- **Whose call:** 协调人（派活时捎给 tpr/02）
- **处置：** 本票了结（`two-pass-rework/02`）。

**Q637 没并进来**：它是 `Report` 与抬头的事，做分布时没有顺手出来，留在停车场。

**新记六条**（本票 id 块 Q705–Q712，用掉六个），都在《待处理》：

- **Q705** — 覆盖顶死的卷在卷表上写的是分布（`2bit+FS 229`），「覆盖」这个词从那一列上退了。推荐现状。
- **Q706** — story 18 的病在上一级还活着：目录那一行的基准档分布与总览块的判定行在默认路径上仍写「逐页 N」
  （后者还是 `base_of` 那五种说法的第二份手抄）。推荐默认路径上整格不在、并收掉那份手抄。
- **Q707** — 档位分布那一列从「恒在」变成「最后一个让」，砍列次序按票面重新想过。推荐现状。
- **Q708** — 列头「档位分布」比「基准档」宽两格，96 列双栏那一档上卷表因此丢了耗时。推荐现状。
- **Q709** — 展开一卷的抬头跟着换成分布，`--envelope` 那条路上「基准档」这个词在屏上不再出现。推荐现状，真机上有人问再加一格。
- **Q710** — `yielding.rs` 那张 40 列快照头上的文档写着卷表的列，屏上画的是目录表（措辞漂移，不在本票的行上）。
