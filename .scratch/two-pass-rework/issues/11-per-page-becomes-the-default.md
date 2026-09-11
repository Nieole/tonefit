# 11: 逐页成为默认

**What to build:** 位深默认**逐页选最低可用档**，不做迟滞，不再默认走卷级上包络（ADR 0018）。要卷级齐整的人显式打开 `--envelope`。

用户看得见的变化：不再为一卷里少数几页的需要付**全卷**的体积，**每一页拿到判据说它要的那一档**。9 号票用眼睛验过：段式迟滞压回的那一档 74% 看得见色带（压到 `4bit 不抖`），迟滞因此去掉——翻页跳变本来就不是问题，漫画页本来就是离散的（ADR 0018）。

`--per-page` 这个开关**报错并指路**，不留成不做事的别名——留着最坏：脚本里那一行看着还在、行为已经反过来，而且它现在是默认，写不写都一样，用户永远不会发现。

**Blocked by:** 09（已 resolved，结论「挡回来重新设计」已按 ADR 0018 落进本票面）

**Status:** resolved

- [x] 默认路径 = 逐页判定，不做卷级聚合、不做迟滞
- [x] `--envelope` 显式打开卷级上包络与迟滞的旧形态，行为与今天的默认逐字节相同
- [x] `--per-page` 当场报错并指路，不静默、不当别名
- [x] 预设里那一项跟着翻，且命令行显式点到的那一项仍旧赢
- [x] **认下全库重跑**：逐页与否是参数哈希的输入项，默认一翻，从没点过这个开关的用户全部不命中。这一条写进 ADR 的《后果》
- [x] ADR 0005 **改原篇**：状态行加修订记录，被推翻的那条决定就地改写，另开一节说清原来那条理由为什么不成立；ADR 0006 的修订记录已由 ADR 0018 加上，本票核一遍它与落地后的实现对得上
- [x] `hysteresis.rs` 退场，`PAGES` 归 `envelope.rs`（ADR 0018 决定第 3 条）
- [x] `CONTEXT.md`《段式迟滞》《滚动窗口》两条词条删掉（随删代码这次提交走）
- [x] 报告与 tEXt 里 `段式迟滞压回邻居那一档` / `hysteresis pull-back` 那一种理由不再出现；读旧输出时那个字符串仍认得（旧记录判为不命中而不是判为错）
- [x] `CONTEXT.md` 里 `上包络` `基准档` `其余页` `定档页` `特例页` **保留原义**，只标注它们描述的是 `--envelope` 那条路；要改的是「默认走哪条路」那一句
- [x] 报告说得出这一趟走的是哪条路，两趟的结果不会被混着比
- [x] 黄金回归会动：判定与输出体积都变，快照**显式接受**，不许顺手覆盖
- [x] 三条闸都绿

## 落地记录

落地于 `tpr/11-per-page-default`，基底 a87f0f2。

**做了什么**

- **默认翻成逐页。** `Request.per_page` 换成 `Request.envelope`（默认 `false`）：默认路径上 `summarize_volume`
  把逐页判定原样收下（`VolumeVerdict::PerPage`），卷级一层都不跑；`--envelope` 打开上包络加迟滞那条路，
  形态一格没动——黄金快照里那 24 卷去掉标签那一格后与 a87f0f2 逐字节相同（落地时脚本核过）。
- **`--envelope`／`--no-envelope` 一对**照 `--crop`／`--no-crop` 的形状（`conflicts_with`，`said`），预设键 `envelope`，
  `TasteLayer::envelope()` 是默认值唯一出处，会话左栏那一行改叫「上包络」（`Field::Envelope`）。
  **命令行显式点到的仍旧赢**——走的是 `Cli::envelope` 里 `said(...).unwrap_or_else(preset)` 那条既有的路。
- **`--per-page`／`--no-per-page` 退场**：留成隐藏参数，点到任何一个 `Cli::refuse_retired_switches` 当场 `bail!`，
  说法里点出敲的是哪一个、指去 `--envelope`。不交给 clap 当「不认得的参数」——那一句说不出该改成什么。
- **`hysteresis.rs` 整个删除**（ADR 0018 决定第 3 条），`PAGES` 归 `envelope.rs`；`Reason::RunHysteresis` 与
  tEXt 的 `hysteresis pull-back` 不再产出。旧输出里那一句照旧读得回、按指纹比、判不命中
  （`metadata::tests::a_record_carrying_the_retired_pull_back_reason_still_reads_back_as_a_miss`）。
- **滚动窗口随之不再是一样东西**（ADR 0018 决定第 4 条：窗口长度是 0）：`lib.rs` 的 `Window`／`Windowed`／`Seat`
  整块删除，`Settles` 收成 `AfterTheVolume | OnItsOwn | UpFront`——默认路径一页判完当场量化编码，
  与覆盖顶死那一趟同走 `insert_encoded`，**参照一张都不进缓存**（spec 的 P-B）；`cache.rs` 的 `replace`／`reference`／`forget`
  随之删除。一角例外见停车场 Q635。
- **参数哈希**那一行从 `per-page` 换成 `envelope`：从没点过开关的与点过的两批用户一起不命中，写进 ADR 0018《后果》。
- **报告说得出走的哪条路**：`VolumeVerdict::Envelope` 只在 `--envelope` 下出现、`PerPage` 只在默认下出现；
  逐页那一句改成「无（默认逐页）：……要卷级齐整开 --envelope」。
- **测试**：`fixtures::request()` 现在是真默认；问上包络内部构造的用例显式走
  `run_volume_under_the_envelope[_fitted_inside]`；新写的默认路径用例：
  `an_isolated_page_keeps_the_depth_the_metric_gave_it_on_the_default_path`（不压了）、
  `the_default_path_caches_no_reference_and_encodes_each_page_as_it_is_decided`、
  `the_retired_per_page_switches_are_refused_and_point_at_envelope`、`the_envelope_is_off_unless_the_command_line_opens_it`。
- **黄金回归改成两路五组**：头一组默认（逐页，14 卷逐页记行），其余四组 `--envelope`（原样）；卷行标签多一格
  `per-page`／`envelope`。快照显式接受；变的只有默认组——例：`mixed-size` 003/005 从 4bit+FS（26713／706727 字节）
  落到 1bit+FS（10748）／2bit（36736），正是「不为少数几页付全卷体积」。
- **文档**：`CONTEXT.md`《上包络》《基准档》《其余页》《定档页》《特例页》《迟滞》加「描述的是 `--envelope` 那条路」，
  《段式迟滞》《滚动窗口》删；ADR 0005 改原篇（状态行、决定 1–3、《第二遍…纯写出》改写，新开《滚动窗口为什么没了》）；
  ADR 0006 修订记录补一句开关退场；ADR 0018《后果》补「点过的也不命中」；ADR 0016 一处措辞。
- 停车场 Q633–Q636。

**`src/session/draw/` 只动了闸门 1 变绿所需的最小一处**：`config.rs`／`report.rs`／`yielding.rs` 里 12 行屏快照期望串
把左栏那一行的标签「逐页」+6 个全角空格换成「上包络」+5 个（宽度不变，版面一格没动）；两处注释仍写 `--per-page`（Q636）。
`VolumeVerdict::PerPage`／`RowKind::PerPage` 名字不改——它说的就是「逐页」。
ADR 0006 的修订记录在「只核对」之外补了一句「`--per-page` 开关退场、反面是 `--no-envelope`」——原句说的开关已不存在，不补就悬空；
属实，请协调人认。report 抬头不带上包络开关、全卷幂等命中那趟说不出走的哪条路，记 Q637。

**数**

闸门（`cargo xtask gate`，最终状态那一趟，2026-09-11）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo test` | 绿 · 合计 923 通过 0 失败；lib 234 / bin 372 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features` | 绿 · 合计 792 通过 0 失败；lib 234 / bin 241 · 末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling` | 绿 · 末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 28.81s` |

polish（`cargo xtask polish`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check` | 绿 |
| 2 | `cargo clippy --all-targets` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.87s` |
| 3 | `cargo clippy --all-targets --no-default-features` | 绿 · `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.47s` |
| 4 | `cargo doc --no-deps` | 绿 · `warning: \`tonefit\` (lib doc) generated 15 warnings`（与落地前同数，15 条都在本票没碰的文件里） |

黄金回归单跑 273 s（热树、满核）；`--test pipeline` 整文件 182 s。
