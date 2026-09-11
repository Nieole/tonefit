# 14: 按页跳过

**What to build:** 改了卷里一页之后，只重做**那一页**，不再整卷重跑。跑到一半崩掉之后，下一趟从没做完的地方接着走。

用户看得见的变化：两百页的卷里换掉一页，下一趟几秒钟就完；今天是整卷重解、重判、重编。

**Blocked by:** 13

**Status:** resolved

- [x] 命中的页跳过，不解码、不判、不编
- [x] 改一页之后，重做的只有那一页，其余页跳过（ADR 0018：一页的档只取决于它自己）
- [x] 删一页、加一页都判得对——那会挪动后面每一页在阅读顺序里的位置
- [x] 卷仍旧是**去处**（隔离目录）、**撞名**与**透传文件**的单位，这三样不跟着降级
- [x] 卷级失败、隔离、部分救回这几条路上的行为不变
- [x] 报告说得出这一卷跳过了几页、重做了几页
- [x] 走 `--envelope` 那条路时照旧整卷跳、整卷重做

## 落地记录

落地于 `tpr/14-skip-by-page`，基底 b523538。

**做了什么**

- **幂等那一道一趟读出两个作用域的源哈希**（`SourceHashes`）：卷级一个数照旧进 `Fingerprint`，页级每个源页一个 `PageSource`
  趁字节在手上顺手算——只在页级依据这一趟成立、且干净去处里真有上一趟的输出时算（story 30）。
  谓词与 13 号票写不写页级依据的是同一句：`Settles::if_processing(..).encodes_in_the_first_pass()`（新拆出的
  `if_processing` 不看模式，试算因此预告得出按页跳过——story 6）。
- **比对的答案从两种扩成三种**（`Reuse`——上一趟的输出能复用多少；`compare_with_the_prior_output` 顶替 `can_skip`）：
  卷级四项每一页都对得上、透传文件都在→**整卷跳过**（与本票之前逐字相同，旧记录与 `--envelope` 那条路都走这一支）；
  卷不齐→**按页**（`Retained`：逐源页留不留）：`PageRecord::matches_by_page`（《页级依据》：工具、profile、参数三项 + 页级源哈希 + 来路）
  对得上的那一族**留下**（`RetainedPage`：来路、名字与尺寸，尺寸读自 IHDR，`PageRecord` 多一格 `size`），对不上的重做；
  页级依据不成立→整卷重做。`written_family` 改成只问来路、齐不齐（交出 `WrittenPage`），两个作用域的比对各在 `PageRecord` 的两个方法上。
  一张灰度页都没重做而留下了页（只改透传文件）时卷级判定仍是这条路的（`Settles::verdict_by_itself`：逐页或覆盖），不报「一张灰度页都没有」。
- **第一遍只走要重做的源页**（`first_pass` 收 `redo: &[usize]`）；**第二遍按阅读顺序交错**留下的与重做的
  （`Slot`／`in_reading_order`）：留下的页从上一趟的输出整页读回（`Written::bytes_of`，sink 层唯一新增），
  `metadata::restamp_source` 原地把 `tonefit:source` 那一项改写成这一趟的卷级源哈希（重算那一块 CRC，`crc32fast` 列成直接依赖），
  再照写页那条路写进这一趟的临时容器。**产物与整卷重做逐字节相同**——下一趟卷级那四项每一页都对得上、整卷跳过，
  不必再逐页比、逐页搬。sink 层收尾照旧整个换掉：不产出半成品、清掉陈旧产物（源里删掉的那一页正靠它清）、
  最终位置只在收尾这一步被碰到、归档卷成员顺序仍是阅读顺序，四条一字没动；目录卷与归档卷同一条路，
  归档卷没有走票面预想的「整卷重做」那第三种情形。
- **卷仍是去处、撞名、透传文件的单位**：重做的一页失败→整卷进隔离目录，留下的页跟着搬进去，干净去处那一份当过期副本；
  撞名校验（`ensure_no_two_outputs_collide`）连留下的页一起查；卷内统一尺寸数的是整本书（`uniform_size` 改收尺寸序列，
  留下的页的尺寸从记录里来）；透传文件照旧整卷搬。归档那一支的 `Written` 在收尾改名之前放掉（Windows 上开着句柄改不了名）。
- **报告**：`VolumeReport` 多一格 `retained_pages`（留下的**输出**页数），`pages` 从此只列这一趟做了的页，`page_count()`
  把留下的加回去；`VolumeVerdict` 不动。`render` 多一行 `RowKind::Retained`（成句：「按页跳过 留下 N 页、重做 M 页：……」），
  排在过期副本之后、判定之前；`render/plain.rs` 与 `session/draw/report.rs` 各多一条穷举分支（后者是那个「一种行一条、不留 `_`」
  的清单，报告形态变了非动不可的那一行）。`src/session/draw/` 的措辞一个字没动，`preset.rs`、`main.rs` 没碰。
- **「跑到一半崩掉之后下一趟从没做完的地方接着走」按与 ADR 0012／0013 对得上的那一种读法落地**：崩掉与中止一个待遇，
  `partial` 丢弃、最终位置不动；下一趟做完的卷整卷跳过，没做完的那一卷按页比——上一趟的输出里没变的页留下、变了的重做；
  头一趟就崩的卷整卷重来。卷内续做不做（Q684）。
- **用例**（都在 spec 许可的接缝上）：`tests/idempotency.rs` 新增十一条——改一页只重做那一页且产物与整卷重做逐字节相同；
  删页／加页／改名各只重做该重做的；切开的一族整族留下、整族重做（两半变一张时旧两半清走）；归档卷按页跳过且成员顺序仍是阅读顺序；
  按页跳过之后的下一趟整卷跳过；`--envelope` 照旧整卷重做；试算预告按页跳过；坏页隔离时留下的页跟着去、占位页尺寸数上留下的页、
  修好之后从干净去处里留下；只改透传文件时两页都留下、卷级判定仍是逐页；部分救回页照样留下；新切出的一张撞上留下的同名一张照样拦下。13 号票那条「页级各自、卷级一起变」改成按输出路径读（留下的页不在 `pages` 里），断言一字未改。
  `metadata.rs` 单测两条（改写那一项之后与按新指纹写的逐字节相同、页级那一问与卷级那一问只差源那一项）；
  `render.rs` 一条（按页跳过那一行与页数）；`lib.rs` 里 `can_skip` 的三条单测改走 `whole_skip`（只带卷级依据比）。
- **文档**：`CONTEXT.md` 新增词条《页级依据》《按页跳过》《留下的页 (Retained page)》；《幂等这一道》《页级源哈希》《跳过》三条
  各补一句指向《按页跳过》——那是 ADR 0018 决定第 4 条已经拍过的领域决定，这里只把说法跟上实现。
  `Stage::Hash`／`Stage::Write` 文档、`volume_steps` 文档各补一段。ADR 0006「源哈希的作用域是卷，跳过的单位也是卷」那段**未动**（15 号票的活）。
- 停车场 **Q681–Q686**（六条，见下）；Q666、Q669 了结。

**code-review**（两轴并行、只读，基底 b523538）之后改的：`Prior` 改名 `Reuse`（装的是「能复用多少」，不是上一趟）；
`Vec<Option<Vec<RetainedPage>>>` 收成 `Retained`（`redo()`／`pages()`／`families()`，两处重复的求和随之没了）；
`written_family` 交出的元组换成 `WrittenPage`；`Slot::Redone` 把判定带在身上，`second_pass` 不再收一份平行数组；
`in_reading_order` 排不进去时回 `Err` 而不是 `debug_assert`（发布构建上静默少写一页是最坏的那种错）；
`restamp_source` 对新值长度也 `ensure!`、块长度用 `checked_add`；`Compute` 去掉 `Copy`；
一张灰度页都没重做的卷级判定改报这条路的（评审指出会话表上它会与整卷彩页混成「无判定」）；
`CONTEXT.md` 补《页级依据》《按页跳过》两条（评审指出「页级依据」两处定义不一、「按页跳过」无词条）；
两条旧用例（`a_changed_source_redoes_the_volume`、`renaming_…`）的文档从「整卷都得重做」改准；
补三条用例：只改透传文件时两页都留下且卷级判定是逐页、部分救回页照样留下且产物逐字节相同、新切出的一张撞上留下的同名一张照样拦下。
未采纳：改写票面「跑到一半崩掉之后接着走」那句（票面是提出的要求，落地记录说明按哪种读法做，见 Q684）；
`restamp_source` 那条设计决定按评审建议留给拍板的人（Q681 列了不改写那条路的代价）。

**数**

闸门（`cargo xtask gate`，最终状态、合过 main `e3baf66` 之后，2026-09-11）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo test` | 绿 · 合计 970 通过 0 失败；lib 238 / bin 397 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features` | 绿 · 合计 824 通过 0 失败；lib 238 / bin 251 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling` | 绿 · 末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.86s` |

polish（`cargo xtask polish`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check` | 绿 |
| 2 | `cargo clippy --all-targets` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 3.05s` |
| 3 | `cargo clippy --all-targets --no-default-features` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.63s` |
| 4 | `cargo doc --no-deps` | 绿 · `warning: \`tonefit\` (lib doc) generated 15 warnings`（与落地前同数） |

三条闸门分两趟得数：一趟 `cargo xtask gate` 跑完第 1 条（970/0，`tests/stop.rs` 两条 ok）、第 2 条编译到一半被系统低内存杀掉；
随即 `cargo xtask gate 2 3`（`CARGO_BUILD_JOBS=4`）补齐第 2、3 条，polish 紧接其后。第 1 条那趟的合计由它的完整日志逐段相加
（每个测试二进制的 `test result` 行），不是 xtask 印的表——那张表被杀在它之前。

**合并 main 之前那一趟**闸门 1 红在 `tests/stop.rs` 两条（`one_ctrl_c` 与 `two_ctrl_c`，并行负载下的时序抖动；单跑 4/4 绿）：
main `e3baf66` 已让那两条带 `--envelope`（了结 Q670），合并之后单跑 3/3 绿、闸门 1 绿。本票基底 b523538，合并 main `e3baf66`
（ta/04 与那条修复）只冲突在停车场（Q669/Q670 两侧各挪走一条）。`--test idempotency` 整文件 37 条，热树 143 s。

### 停车场结转

**Q666 了结**：按推荐 ①——占位页不写页级源哈希，本票不按 `failed` 特判：`compare_with_the_prior_output` 只认
`PageRecord::matches_by_page`，占位页的记录没有那一项、恒对不上，那一族重做；用例
`a_failed_page_isolates_the_volume_and_the_retained_pages_go_along` 钉着「坏页修好之后重做的只有它」。
《已了结》索引表那一行已加；条目原文转录于此：

#### Q666 — 失败页的占位页不写页级源哈希；14 号票因此天然跳不过它，不必按 `failed` 特判

- **From:** 票 `two-pass-rework/13`
- **Kind:** 设计取舍
- **Where:** `src/metadata.rs` 的 `Recorder::failed`；`src/lib.rs` 的 `Compute::split_and_branch`（`Err` 那一支 `Placement::new(…, None)`）
- **Why it did not block:** 占位页按卷内统一尺寸出（`uniform_size`，全卷众数），这一页的字节不只取决于它自己——
  与上包络那条路同一条理由；而隔离的卷本来每一趟都重做（`process_volume`），写不写都买不到跳过。
- **What this ticket actually did:** 不写；`Recorder::failed` 的 `page_source` 恒 `None`，
  用例 `a_placeholder_page_carries_no_page_level_basis_while_its_neighbour_does` 钉住。
- **Options:** ① 不写（现状）；② 照写——读不出字节的成员用 `SourceHasher::unreadable` 那个记号、解不出来的用字节哈希，
  14 号票再按 `tonefit:verdict == failed` 特判不跳；旁边卷里换一页尺寸、众数一变，旧占位页就会被判命中而尺寸已过期。
- **Recommend:** ①。「页级依据在场 ⟺ 这一页的字节只取决于它自己」这条不变量一处成立，读记录的人不必再问 verdict。
- **Whose call:** 14 号票的实现者（可复议）
- **处置：** 本票了结（`two-pass-rework/14`）。

**Q669 了结**：`PageRecord::page_source` 的读者接上（`PageRecord::matches_by_page`），那句 `cfg_attr(not(test), expect(dead_code))`
拆掉——编译器如预期报「预期未兑现」，删掉即绿。条目原文转录于此：

#### Q669 — `PageRecord::page_source` 生产路径上没有读者，挂着 `cfg_attr(not(test), expect(dead_code))`，14 号票接上读者时要拆掉

- **From:** 票 `two-pass-rework/13`
- **Kind:** expand 阶段的临时允许
- **Where:** `src/metadata.rs` 的 `PageRecord::page_source`
- **Why it did not block:** 本票的边界就是「算出来、记下去、读得回来」，读它做判定是 14 号票的活；不挂就是一条 `dead_code`，
  polish 那两遍 clippy 的数会多一条，而票面说「谁都还没开始用它跳过」——那一句挂在字段上就是这条 `allow`。
- **What this ticket actually did:** 挂了 `expect(dead_code, reason = …)`——不是 `allow`：读者一接上，预期没兑现，编译器当场报
  `unfulfilled_lint_expectations`，删这一句就不靠人记（code-review 的建议）。单测读它（`the_page_level_basis_reads_back_…`），所以只在 `not(test)` 下挂。
  与 `src/session.rs`／`preset.rs`／`render.rs` 那三处 `allow` 不同类：那三处是「特性关掉才没读者」，这一处是 expand 阶段的**真**死代码。
- **Options:** ① `expect`，14 号票接上读者时编译器逼着删（现状）；② 现在就给它一个生产读者（比如 `written_family` 顺手读一下、不做判定）——白读，
  而且让人误以为它进了判据；③ 不挂，认一条告警。
- **Recommend:** ①。
- **Whose call:** 14 号票的实现者
- **处置：** 本票了结（`two-pass-rework/14`）。

**新记六条**（本票 id 块 Q681–Q688，用掉六个），都在《待处理》：

- **Q681** — 留下的页靠「读回 → 改写卷级那一项 → 照写页写进新容器」搬过来，sink 层没有长出「部分保留」的新形态；
  代价是按页跳过那一趟的 I/O 与卷大小成正比。推荐现状；15 号票收掉卷级依据之后可回头改 raw copy。
- **Q682** — `VolumeReport::pages` 只列这一趟做了的页，留下的页只有一个数；会话卷表档位分布那一列对按页跳过的卷写的是重做那几页的分布，
  `under()` 没画按页跳过那一句。推荐给 `session/draw/table.rs` 的 `under()` 加一行，单独派。
- **Q683** — Q635 那一角（默认路径、一维覆盖、两组门混着）没有页级依据，改一页整卷重做；推荐等 Q635 按它的推荐 ② 落地。
- **Q684** — 「跑到一半崩掉之后接着走」在卷内不成立（ADR 0013 决定第 2 条）；本票按可达读法做。推荐现状。
- **Q685** — `volume_steps` 的预告减不掉留下的页（预扫在幂等之前）；按「预告是上界」兜住。推荐现状。
- **Q686** — 重做的页页级源哈希算了两遍（幂等那一道拿去比、第一遍写进记录）。推荐现状。
