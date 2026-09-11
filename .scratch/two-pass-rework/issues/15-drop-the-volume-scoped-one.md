# 15: 收掉卷级那一份依据（contract）

**What to build:** 按页跳过站稳之后，把默认路径上那份**卷级**幂等依据收掉——没有调用点了，留着就是第二个出处，将来两份对不上时没人说得清哪份作数。

这是 expand–contract 的 contract 那一步。

`--envelope` 那条路仍旧要它：那条路上一页的档由全卷定，跳过的单位只能是卷。所以这一票**不是删掉**卷级依据，是让它只服务于那条路。

**Blocked by:** 14

**Status:** resolved

- [x] 默认路径不再算、不再记卷级依据
- [x] `--envelope` 那条路照旧算、照旧记、照旧整卷跳
- [x] 两条路各自的依据只有一个出处，不重叠、不互相兜底
- [x] 旧输出（默认路径上带卷级依据的那些）判为不命中，重做一次之后转成新形态
- [x] ADR 0006 里「源哈希的作用域是卷，跳过的单位也是卷」那一段就地改写：说清它**只对上包络成立**——它论证的是基准档由全卷定，而逐页判定根本不看别的页（ADR 0018）
- [x] 三条闸都绿

## 落地记录

落地于 `tpr/15-drop-volume-basis`，基底 b2f2e91。

**做了什么**

- **源哈希按作用域只有一种**（`metadata::SourceHash`）：`Volume(VolumeSource)`——全卷一个数，`tonefit:source`；
  `Page(PageSources)`——每个源页一份、每个透传文件一份，`tonefit:page-source`。哪一趟走哪种由
  `Settles::if_processing(..).encodes_in_the_first_pass()` 一处说了算（与 13 号票写不写页级依据、14 号票按不按页跳同一句）：
  这一页的字节只取决于它自己 → 页级；由全卷定 → 卷级。`Fingerprint` 从此是「共用的三项（`Invocation`：工具、profile、参数）
  加这一趟的源哈希」，`Fingerprint::new(request, source)`；`SourceHasher::finish` 交出 `VolumeSource`。
- **幂等那一道一趟只算一种**（`volume_fingerprint` → `Fingerprint`，`Feeding` 两支）：卷级那一支与本票之前逐字相同；
  页级那一支给每个成员各算一份（透传文件也算，只拿去比），**不再看有没有上一趟的输出**——那一份现在也是第一遍盖进记录的那一份。
- **一份记录只带一种源哈希，由构造保证**（`Fingerprint::source_item(page: Option<usize>)`，`Record.source: Option<(keyword, value)>`）：
  两种取值都从同一份指纹里取——卷级那条路每一页写 `tonefit:source`（失败页也写）；页级那条路按源页序号取这一页自己的
  `tonefit:page-source`，失败页与第二遍盖记录的页不给序号、哪一项都不写。`Record::color`／`Recorder::gray` 收的是源页序号，
  不是哈希本身；`Placement` 因此只带一个 `page: usize`。`Record::fields()` 是三项、源那一项（在场时）、来路、判定、理由。
- **比对只问这一趟走的那一种，另一种在场就不认**（`PageRecord::matches(fingerprint, index, relative, ordinal, count)`——
  顶替 `matches`／`matches_by_page` 两个方法与 `same_but_for_the_source`）：三项、来路，加上卷级的全卷那一个数或页级的第 `index`
  个源页自己那一份；记录里带着另一种就是不命中。`PageRecord` 读回来两种都读、**读到什么是什么**（`source`／`page_source` 各存原文
  `Option<String>`，不解析）：「在场」按键在不在判，取值写成什么样都算在场——解析成「没有」会让一份带着坏键的外来记录命中
  （code-review 指出）。旧 tEXt 键读到仍认得——判不命中，不判错。旧的默认路径输出（只带卷级的、两种都带的）因此重做一次、转成新形态。
- **「齐」两条路各有各的说法**（`compare_with_the_prior_output(output, volume, fingerprint, lodgers)`）：卷级那条路——每一页
  对得上、透传文件都在 → `Reuse::Whole`，否则 `Reuse::Nothing`；页级那条路——每一页各自对得上，加 `nothing_else_changed`：
  每个透传文件与输出里那一份重新算出来的哈希相同、**输出里再没有别的**（`Written::holds_nothing_but(members, lodgers)`，
  sink 层唯一新增：目录卷只看直接那一层、通往借住的卷的不算，与收尾清陈旧产物是**同一句** `Lodgers::spoken_for`；
  归档卷看全部成员名）→ `Whole { page_count }`，`page_count` 由按页那一支给；否则 `ByPage`。后两句是卷级那一个数从前顺手盖住的：页级各比各的看不见透传文件的内容、看不见源里删掉的那一页留下的
  陈旧产物——14 号票的实现者报的「缺 `Written` 列成员的能力」正是这一处，真缺。
- **留下的页 raw copy**（`second_pass` 收 `Option<&mut Written>`，`Retaining` 与 `metadata::restamp_source` 退场）：
  读回整页、一个字节不改、照写页那条路写进去。记录里没有由全卷定的东西了，Q681 里的另一条路成了唯一的路。
  `crc32fast` 从 `[dependencies]` 退回只在 `[dev-dependencies]`（`Cargo.lock` 一格没动）。
- **第一遍从指纹里取页级源哈希**（`Compute::page`／`split_and_branch` 多收源页序号，一路传到 `Placement::page`，盖记录时
  `Fingerprint::source_item` 按它取）：不再算第二遍，`Stage::Hash` 只有幂等那一道喂——Q686 了结。代价是页级那条路上试算、没有上一趟输出的头一趟也把每个成员
  的哈希算齐（blake3 一页不到 1 ms，字节本来就在手上），换的是一处出处、一次哈希。
- **用例**：`tests/idempotency.rs` 39 条——改四条（默认路径的记录写页级不写卷级：灰度页、彩页、顶死那一趟；占位页哪一项都不写）、
  改写两条（`every_page_on_the_default_path_carries_its_own_source_hash_and_no_volume_level_one`；
  `an_old_output_carrying_the_volume_level_basis_is_redone_once_and_comes_out_in_the_new_form`——两种旧形态各钉一次：
  重做一次、`decodes == 2`、新形态、再跑整卷跳过；`with_text_chunk` 是造两种都带那种形态的唯一一条路）、
  新增两条（`a_stale_member_in_the_output_blocks_the_whole_skip_and_is_cleared`；
  `a_pinned_run_under_the_envelope_records_and_skips_by_page`——Q698 那一角）、三条只改文档。
  `tests/container.rs` 新增一条（`a_directory_volume_with_lodgers_still_skips_as_a_whole`：借住的卷不算陈旧产物）。
  `metadata.rs` 单测 13 条（新写三条：一份记录只带一种源哈希；两条路各比各的那一项、另一种在场判不命中、两种都带判不命中；
  页级那一项读回来、坏键在两条路上都判不命中），`lib.rs` 里 `whole_skip` 改走卷级那条路。`--envelope` 那条路上既有的用例（记录、整卷跳、整卷重做）一字未改、全绿。
- **黄金回归没动**：它按 `--no-metadata` 跑（快照头上第 67 行），tEXt 那一块从来不在它量的字节里——派活说明预想的「体积会变、要显式接受」
  在这一票上不成立，快照与提交前逐字节相同。
- **文档**：ADR 0006《决定》末段就地改写（那一段只对上包络成立；修订记录加一条）；`CONTEXT.md`《幂等这一道》《记录》《指纹》《源哈希》
  《页级源哈希》《页级依据》《按页跳过》《留下的页》《跳过》九条跟上实现（《源哈希》绑上 `SourceHash`／`VolumeSource` 两个类型名）；`metadata.rs`、`sink.rs`、`cost.rs`（`Stage::Hash`／`Write`）、
  `lib.rs`（`process_volume`、`Settles::if_processing`、`Reuse`、`compare_with_the_prior_output`）、`report.rs` 的文档同步。
  `src/session/` 一个字没动；`preset.rs`、`main.rs` 没碰。
- 停车场 **Q697、Q698、Q699**（三条，见下）；Q681、Q686 了结。
- **验收框 1 与「`--envelope` 逐字节相同」各有一角例外**，都出自同一个谓词（依据的作用域随「这一页的字节取决于什么」，不随开关）：
  默认路径上 Q635 那一角仍按卷（Q697）；`--envelope` 加两维都点名的那一趟按页、少一块 `tonefit:source`（Q698）。票面那两句在上包络
  真正接手的那条路上全部成立；两角各有说明，没改票面。

**code-review**（两轴并行、只读，基底 b2f2e91）之后改的：`PageRecord` 里源那两项改存原文、「在场」按键判不按写法判
（Spec 轴：解析成「没有」会让带着坏键的外来记录命中，补一条单测钉住）；`Placement` 改存源页序号 `page: usize`，
`Record::color`／`Recorder::gray`／`gray_bytes` 收 `Option<usize>`，`Fingerprint::source_item(page)` 成了全函数、
`debug_assert!` 与五跳的 `Option<PageSource>` 一起退场（Standards 轴：tramp data，断言是症状）；`Lodgers::spoken_for` 让收尾清陈旧产物
与 `holds_nothing_but` 共用同一句谓词；页级那条路的两问抽成 `nothing_else_changed`；`CONTEXT.md`《源哈希》绑上类型名；ADR 0006 正文里
与修订记录重复的那半句去掉；`PageSources::with_capacity`、`Feeding` 两个变体补文档。**未采纳**：`matches` 末三参收成 `&Origin`
（与 `is_the_page` 签名对称，两处一起改才值）；`Feeding` 并进 `SourceHash`（一个是累加器、一个是值）。`PageSource` 借给透传文件那一条
记 Q699，推荐单独派一次改名。Spec 轴核实：卷级那条路的喂序与字段序与基底逐字相同；黄金回归 `metadata: false`；删页、改透传、外来成员、
借住的卷四种各有用例。

**数**

闸门（`cargo xtask gate`，最终状态、code-review 之后，`CARGO_BUILD_JOBS=6`，2026-09-11，一趟跑满三条，`EXIT=0`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo test` | 绿 · 合计 973 通过 0 失败；lib 238 / bin 397 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features` | 绿 · 合计 827 通过 0 失败；lib 238 / bin 251 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling` | 绿 · 末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.60s` |

与 14 号票落地时（970 / 824）比：闸门 1 多 3 条（`idempotency` +2、`container` +1），闸门 2 同样 +3；lib 单测 238 条不多不少
（`metadata.rs` 删三条、新写三条）。

polish（`cargo xtask polish`，紧接闸门之后，`EXIT=0`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check` | 绿 |
| 2 | `cargo clippy --all-targets` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 4.01s` |
| 3 | `cargo clippy --all-targets --no-default-features` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 5.20s` |
| 4 | `cargo doc --no-deps` | 绿 · `warning: \`tonefit\` (lib doc) generated 15 warnings`（与落地前同数，无一条在本票改的文件里） |

热树单跑：`--test idempotency` 39 条 142 s；`--test golden` 2 条 270 s（快照未动）。黄金回归不接受、不改：`tests/golden.rs` 按 `metadata: false` 跑。

### 停车场结转

**Q681 了结**：本票收掉卷级那一项之后，Q681 的 ④（不改写、raw copy）的代价——「卷级那四项永远不齐」——随那四项一起消失，
raw copy 成了唯一的路：`second_pass` 从上一趟的输出读回整页、原样写进这一趟的容器，`Retaining` 与 `restamp_source` 退场。
Q681 列的另一条代价「`compare_with_the_prior_output` 要另加「每一页各自都没变且没有陈旧成员」那一支（要列容器成员）」
正是本票加的 `Written::holds_nothing_but`。条目原文转录于此：

#### Q681 — 留下的页靠「读回 → 改写卷级那一项 → 照写页写进新容器」搬过来，sink 层没有长出「部分保留」的新形态

- **From:** 票 `two-pass-rework/14`
- **Kind:** 设计取舍——13 号票的实现者报的 (b)「输出容器如何部分保留」，本票选了不部分保留
- **Where:** `src/lib.rs` 的 `Retaining::carry_over`、`Slot`／`in_reading_order`；`src/sink.rs` 的 `Written::bytes_of`（新增的唯一一处）与模块文档；
  `src/metadata.rs` 的 `restamp_source`（`crc32fast` 因此列成直接依赖，树里现成的那一份）
- **Why it did not block:** 两条路都站得住，而选这一条不改任何既有不变式：`.partial` 收尾整个换掉、不产出半成品、
  清掉陈旧产物（源里删掉的那一页正靠它清）、最终位置只在收尾这一步被碰到（ADR 0013 决定第 2 条）——四条一字没动，
  目录卷与归档卷同一条路，隔离那一支（留下的页跟着去 `_isolated/`）不必另写。改写 `tonefit:source` 那一项之后
  **按页跳过的产物与整卷重做的逐字节相同**（`changing_one_page_redoes_only_that_page` 下半段钉着），
  下一趟卷级那四项每一页都对得上、整卷跳过——不改写的话留下的页永远记着旧数，那一卷从此每趟都要按页比、按页搬。
- **What this ticket actually did:** 留下的页第二遍从上一趟的输出（`Written`，比对那一步开着的同一份）整页读回、
  `restamp_source` 原地改那一个 tEXt 块与它的 CRC、`Sink::write_page` 写进这一趟的临时容器；按阅读顺序与重做的页交错
  （`in_reading_order`），归档卷因此仍是整包重打、成员顺序仍是阅读顺序。代价：每一趟按页跳过的 I/O 与卷大小成正比
  （读一遍旧输出、写一遍新容器），不是零；票面的「几秒钟」在两百页、两百兆的卷上是读写各两百兆。
- **Options:** ① 现状；② 目录卷用硬链接（`hard_link` 失败退回复制）省掉那一份读写——但改写那一项要碰字节，硬链接就碰到了
  最终位置上的那一份，只能先复制再改，省不下来；③ 目录卷原地留旧、只写重做的、清掉陈旧的（sink 层新形态）——
  同样要改写留下的页那一项，等于原地改最终位置上的文件，破「最终位置只在收尾这一步被碰到」；④ 不改写那一项、
  raw copy（归档 `raw_copy_file`、目录 `fs::copy`）——I/O 减半，代价是卷级那四项永远不齐，`compare_with_the_prior_output`
  要另加「每一页各自都没变且没有陈旧成员」那一支（要列容器成员、认借住的卷），且 13 号票的用例
  「卷级那一项两页一起变」不再成立。
- **Recommend:** ①。②③④ 省的都是那一份读写，付的都是一条不变式或一份永远不齐的记录；15 号票收掉卷级依据之后
  ④ 的代价才消失，那时可以回头改成 raw copy。
- **Whose call:** 拍板的人
- **处置：** 本票了结（`two-pass-rework/15`）。

**Q686 了结**：按它的 ②——幂等那一道算好的页级源哈希随指纹传进第一遍（`Fingerprint::page_source(index)`），
`split_and_branch` 不再算。它说的「比对与处理之间源变了」那一角：记录写的是比对那一刻的哈希、像素是处理那一刻的字节，
下一趟源哈希对不上、那一页重做一次即自愈——方向对、多一趟。条目原文转录于此：

#### Q686 — 重做的页页级源哈希算了两遍：幂等那一道一遍（拿去比），第一遍再一遍（写进记录）

- **From:** 票 `two-pass-rework/14`
- **Kind:** 白付一处——量级小，记着
- **Where:** `src/lib.rs` 的 `volume_fingerprint`（`by_page` 那一份）与 `Compute::split_and_branch`（13 号票的那一次）
- **Why it did not block:** blake3 一页不到 1 ms（Q668 量过口径），两遍都摆在解码旁边不改前三大开销的名单；
  两处算的是同一个函数 `PageSource::of` 喂同一批字节，第一遍那一份是**这一页真被处理的那份字节**的哈希——
  比对与第一遍之间源被人改了，写进记录的是处理过的那一份，更对。
- **What this ticket actually did:** 两处都留着；`Stage::Hash` 文档写清一格里三处喂。
- **Options:** ① 现状；② 幂等那一道算好的 `PageSource` 随 `redo` 传进第一遍，`split_and_branch` 不再算——
  省一次哈希，多一条参数与一个「比对与处理之间源变了」的静默角。
- **Recommend:** ①。
- **Whose call:** 拍板的人
- **处置：** 本票了结（`two-pass-rework/15`）。

**新记三条**（本票 id 块 Q697–Q704，用掉三个），都在《待处理》：

- **Q697** — 依据的作用域由 `Settles` 定、不由 `--envelope` 定：默认路径上 Q635 那一角（一维覆盖、两组门混着）仍算、仍记卷级依据，
  票面「默认路径不再算、不再记」在这一角不成立。推荐等 Q635 按它的推荐 ② 落地，那一角自然消失。
- **Q698** — 同一个谓词的另一面：`--envelope` 加两维都点名的那一趟顶死、按页记依据，那一角的输出少一块 `tonefit:source`，
  票面「`--envelope` 路上的输出逐字节相同」在这一角不成立（黄金回归的五组都不点覆盖项，在它们身上成立）。推荐现状；
  用例 `a_pinned_run_under_the_envelope_records_and_skips_by_page` 钉着这一角。
- **Q699** — 透传文件在页级那条路上的那一份哈希借用了 `PageSource` 这个类型（code-review 指出的命名借用）。推荐改名 `MemberSource`、
  单独派——一次改名不该夹在这一票的语义改动里。
