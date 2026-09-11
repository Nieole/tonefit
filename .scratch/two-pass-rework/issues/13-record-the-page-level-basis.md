# 13: 记下页级依据（expand）

**What to build:** 幂等的依据多出一份**页级**形态：每张输出页记下它自己的源哈希（连同 `来路`）。新形态与既有的卷级那一份**并存**，谁都还没开始用它跳过——这一票只负责把它算出来、记下去、读得回来。

这是 expand–contract 的 expand 那一步：先把新形式摆在旧形式旁边，一个调用点都不改，14 号票才谈得上按页跳过。

默认路径上一页的档只取决于它自己（ADR 0018：不做迟滞），页级依据因此成立——这正是卷级上包络做不到、而它做得到的那件事。

**Blocked by:** 11

**Status:** resolved

- [x] 每张输出页记下页级哈希，与既有的卷级依据并存
- [x] **跨页拆分下页级依据定义得清楚**：一个源页产出多张输出页，每张的依据要指得回源页的哪一半；`来路` 那一项正是为这件事留的
- [x] 旧记录（只有卷级那一份）照旧读得懂，判为不命中而不是判为错
- [x] 跳过与重做的行为这一票**一个字不变**——仍旧整卷跳、整卷重做
- [x] 走 `--envelope` 那条路时不记页级依据：那条路上一页的档由全卷定，页级答不了

## 落地记录

落地于 `tpr/13-page-level-basis`，基底 6dbcd74。

**做了什么**

- **多一个 tEXt 键 `tonefit:page-source`**，不是 `tonefit:source` 那一格多一层形态：旧读法碰都不碰新键，`Fingerprint` 的相等
  仍是「四项逐字相同」，`can_skip`／`written_family` 一字未动（expand，不 contract）。缺这个键的记录读回来是 `page_source: None`
  ——老形态，不是坏记录。为什么这样选，写在 `metadata::PageSource` 的类型文档《为什么是多一个键》。
- **定义**：`PageSource::of(relative, bytes)` = `SourceHasher` 只喂这一个成员再 `finish`（名字带长度前缀、字节带长度前缀，
  与卷级同一条规矩、两个作用域）。写法用字面量钉死（`the_page_source_has_a_frozen_definition`）——按页跳过落地之后改一次定义
  就是全库页级不命中一趟。
- **跨页拆分**：算的是**源成员**，切出的两张页级源哈希相同，哪一半由来路说（`001.png 1/2`／`2/2`）——两项合在一起才是页级依据。
- **在第一遍里顺手算、不多读一遍源**：`Compute::split_and_branch` 在解码之前趁字节在手上喂一次 blake3，
  `cost::stage(Stage::Hash)` 掐表（`Stage::Hash` 文档改成「两遍都喂」；新掐表点，闸门 3 跟着跑）。
  哈希随 `Placement` 走到第一遍盖记录那一下（`Record::color`／`Recorder::gray`）即止，**不进 `OutputPage`**：
  第二遍盖记录的两种页（上包络路上的灰度页、失败页）按规矩都不写。
- **谁有谁没有**——谓词一处出处：`指纹在 && Settles::encodes_in_the_first_pass()`，即「这一页的字节只取决于它自己」。
  默认路径与覆盖顶死那一趟写；`--envelope`、失败页的占位页（尺寸由全卷定）、试算（一个字节不写）、`--no-metadata`（连算都不算）
  都不写。`--envelope` 加两维都点名那一趟按理由**写**，字面上多出票面一角——记 Q665；失败页不写记 Q666。
- **`Record::fields()`** 从定长 7 改成 `Vec`（7 或 8 项），新键紧跟卷级那一份之后。默认路径每页因此多约 64 字节的 tEXt；
  黄金按 `--no-metadata` 跑，快照一字未动；`counters.rs` 那两条 `stored == written` 两边都含它。
- **`PageRecord::page_source`** 生产路径上还没有读者：挂 `cfg_attr(not(test), expect(dead_code, reason = …))`——按页跳过那一票
  接上读者时编译器当场报「预期未兑现」，逼着删（Q669）。
- **用例**（都在票面许可的接缝上）：`tests/idempotency.rs` 六条——默认路径两页各有、改一页只动那一页的页级哈希而卷级两页一起变
  （story 23）；跨页两半同哈希、来路分半；`--envelope`（卷里带一张彩页，两条盖记录的代码都盖到）不写且再跑仍整卷跳；
  顶死那一趟照写；把新键从写出的页里剥掉（`without_text_chunk`，造老形态输出）再跑仍整卷跳过、零解码；
  占位页不写而邻页写。`metadata.rs` 单测两条——写下读回、旧记录读回 `None` 且卷级照旧命中、坏取值读成 `None`；定义冻结。
- **文档**：`CONTEXT.md` 新增词条《页级源哈希 (PageSource)》，《源哈希》只加一句指路、《记录》字段清单加一项；旧义未改。
  ADR 0006「源哈希的作用域是卷，跳过的单位也是卷」那段**未动**（15 号票的活）。
- 停车场 **Q665–Q670**（Q670 是闸门第一趟撞上的 `tests/stop.rs` 时序抖动，排除法证明与本票无关，见那条）。

**code-review**（两轴）之后改的：模块文档与 `CONTEXT.md`《记录》「只在默认路径上」改成指向《哪些页有》；`/// tEXt 的关键字`
那句「其余四项」→「其余几项」；`allow(dead_code)` → `expect`；同构断言换成冻结字面量；补顶死那一趟的用例；
两条用例的读字段闭包收成 `recorded()`。未采纳：把来路与页级源哈希捆成一型（要改 `OutputPage` 与第二遍那条路，
超出 expand 的边界；两个相邻 `None` 类型不同，写错编不过）；`fields()` 回 `impl Iterator`（每页一次 8 项的堆分配，量级可忽略）。

**数**

闸门（`cargo xtask gate`，最终状态那一趟，2026-09-11）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo test` | 绿 · 合计 950 通过 0 失败；lib 236 / bin 390 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features` | 绿 · 合计 804 通过 0 失败；lib 236 / bin 244 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling` | 绿 · 末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.34s` |

polish（`cargo xtask polish`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check` | 绿 |
| 2 | `cargo clippy --all-targets` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.88s` |
| 3 | `cargo clippy --all-targets --no-default-features` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.43s` |
| 4 | `cargo doc --no-deps` | 绿 · `warning: \`tonefit\` (lib doc) generated 15 warnings`（与落地前同数） |

闸门头一趟红在 `tests/stop.rs:120`（`two_ctrl_c_…`，`["卷01"]`），后面两条没跑、polish 绿；单跑 5 次 2 红，
关掉本票改动再单跑 6 次仍 2 红——时序抖动，见 Q670。改动不变，重跑一趟三条全绿（上表）。
`--test idempotency` 整文件 26 条，热树 221 s。

