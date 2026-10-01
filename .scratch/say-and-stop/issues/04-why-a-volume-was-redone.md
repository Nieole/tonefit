# 04 — 整卷重做说得出原因，退回串行说得出

**What to build:** **为什么整卷重做**：幂等那一道比不上时交出**差在哪一项**——工具版本、设备配置、选项（参数哈希）三项里的哪几项，
或者上一趟的输出在、记录读不出——随卷报告带出来，卷级印一句。源变了的那几页照旧由「按页跳过 留下 N 页、重做 M 页」那一句说，
这一句只管共用那三项。没有上一趟的输出时不出。参数哈希是一个整体，只说得出「选项变了」。

**退回串行**：源在开卷之后被换掉、那一卷退回串行读时，那句话不再丢掉，随卷报告带出来，卷级印一句
「这一卷的源在跑的过程中变过，退回串行读」。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `render` 的用例：工具版本、设备配置、选项各变一项各一条；几项一起变一条；记录读不出一条；没有上一趟的输出时不出
- [x] 集成用例：同一份源先后两趟换一个选项，第二趟那一卷的报告说出「选项变了」
- [x] 退回串行那一句：换掉归档的那条既有用例同时断言报告里有这一句
- [x] 黄金快照原样过（黄金那一趟没有上一趟的输出、也没有被换掉的归档）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。**

1. **库：`VolumeReport` 多两格**（`src/report.rs`）。
   - `why_redone: Option<WhyRedone>`。`WhyRedone` 是四格布尔：工具版本、设备配置（型号名）、选项（参数哈希）各变没变，
     加上「页在、记录读不出」。没什么可说的是 `None`：头一趟、整卷跳过、只因源变了而重做、`--no-metadata` 那一趟。
   - `fell_back_to_serial: Option<String>`：源在开卷之后被换掉、这一卷退回串行读时，`source::archive_was_replaced`
     拼的那句话原样带着。
   - `WhyRedone` 从 `lib.rs` 导出。
2. **库：比的同一遍顺手记下差在哪**（`src/pipeline.rs` 的 `compare_with_the_prior_output`）。
   - 齐了的那一族，记录本来就在手上，逐张问 `PageRecord::what_changed`（`src/metadata.rs` 新添的唯一一个方法，
     只比共用三项，`Origin`／`matches`／`PageSource` 一个字没动）。
   - 没齐的那一族回头问它的头一张（`what_the_head_says`）：记录读得出就比三项；页在、记录读不出记「读不出」；
     一张都不在就不说（源那一侧的事）。
   - 各页合起来（`WhyRedone::together`），后两种答案 `Reuse::ByPage`／`Reuse::Nothing` 带着它，
     `process_volume` 交进两处报告。
3. **库：退回串行那句话不再丢**（`src/read.rs`）。`Reads::Serial` 多一格 `fell_back`，`Reads::fell_back()` 取它；
   `volume_fingerprint` 取走、连同指纹一并交回，`process_volume` 交进两处报告（整卷跳过那一处也交——退回发生在幂等那一道上）。
   分析环节那一路退不回串行（`IoPlan::decide` 在归档句柄的卷上恒派一条），不取；理由只写在 `volume_fingerprint` 的
   《退回串行只撞得上这一遍》一处（Q1239）。`Independent::Replaced`、`reads` 两处「没有去处」「不进报告」的旧说法改成现在的事实。
4. **命令行报告多两种成句行**（`src/render.rs`、`src/render/plain.rs`，缩进两格，与别的成句行同一个摆法）。
   - `RowKind::Redone`（`redone_row`），接在按页跳过那一行后面：
     - 有项变了：「重做 之前转换过，但工具版本、设备配置、选项变了」（只列变了的，`、` 串起来）；
     - 只是读不出：「重做 上次的输出还在，但有页读不出记录，比不出变了什么」；
     - 两样都有：变了那一句后接「；另有页读不出记录」。
     三项的叫法只写在 `SHARED_BASIS` 一处，幂等命中那一句（`SKIPPED`）里逐字出现，有用例核着。
   - `RowKind::FellBackToSerial`（`fell_back_row`），接在读法那一行底下：「这一卷的源在跑的过程中变过，退回串行读」。
     库带着的那句长话不复述（Q1237）。
5. **用例**（取值一律点名，`WhyRedone` 四格逐格写出）。
   - `render` 四条：
     - 三项各变一项、三项一起变、只是读不出、变了又读不出，各一句逐字比；
     - 去处在按页跳过那一行后面，`None` 时一行不出；
     - 三项的叫法在 `SKIPPED` 里逐字出现；
     - 退回串行那一句在读法那一行底下，跳过的卷照样有，`None` 时不出。
   - `tests/idempotency.rs`：
     - 新三条：
       - 换一个选项（`Filter::Bicubic`），头一趟 `None`、第二趟只差选项；
       - 同一件事起真进程跑两趟，第二趟 stdout 里有「重做 之前转换过，但选项变了」，头一趟没有；
       - 把上一趟输出里的 `Software` 改成另一个版本，第二趟只差工具版本。
     - 既有四条添断言：
       - 十二种参数改动都说选项变了、工具版本没变；
       - 同一块面板的另一个别名只差设备配置；
       - `--no-metadata` 写出的输出只说读不出；
       - `--no-metadata` 那一趟与只改一页源的那一趟都是 `None`。
   - 退回串行：
     - 既有的 `read.rs` 那条换掉归档的用例，两半各添一问：没换时 `fell_back()` 是 `None`，换了之后那句话点得出卷名、说得出「退回串行」。
     - 它在读取层，够不着报告，于是 `tests/concurrency.rs` 新添一条：查重环节开工那一刻把归档换掉、点名并发，
       那一卷的 `VolumeReport::fell_back_to_serial` 带着那句话，同一趟没被换的那一卷是 `None`。
     - 单核机器上点名并发也只派一条、无从核起，那条直接返回。这是 `io_mode_overrides_the_probe_…` 那条的先例。
6. **没改的**：`CONTEXT.md`（spec《词条随票落地》：其余几件是报告的措辞，词汇表不动）、设计稿、会话屏（新的两种行会话不读）、
   黄金快照（那一趟 `--no-metadata`、没有被换掉的归档，`VolumeReport` 的新两格它不读）。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `volume_fingerprint` 把 `fell_back()` 换成 `None`：`tests/concurrency.rs` 那条新用例红在「换掉了的那一卷没说它退回了串行」。
- `PageRecord::what_changed` 三项恒答没变、`what_the_head_says` 不记读不出：`tests/idempotency.rs` 四条一起红
  （选项、别名、升级、`--no-metadata` 写出的输出，各红在自己那一项上）。
- `render::volume` 不出为什么重做那一行：起真进程那一条红在「第二趟没说选项变了」。
- `SHARED_BASIS` 里「设备配置」改成「型号」：核叫法那一条红在「跳过那一句里没有『型号』」。
- `read.rs` 那条用例的新断言落地前先红在编译（`fell_back` 不存在）。`render` 两组新用例同样先红在编译（`RowKind::Redone`／`FellBackToSerial` 不存在）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 84ae8fe`。

**收下的**：

- Spec：「第二趟那一卷的报告说出『选项变了』」只断言了结构体。收下：添起真进程那一条，比 stdout 上的字。
- Spec：只是读不出那一句读着像整份输出都读不出。收下：改成「有页读不出记录」。
- Standards：「哪一遍会退回串行」说了三遍，口气各不一样（「多半」「不会」「是这一道」）。收下：收进 `volume_fingerprint`
  《退回串行只撞得上这一遍》一处，`first_pass` 与 `process_volume` 指过去。
- Standards：`reads` 文档末段「降下来的条数不进报告」没跟上。收下：限定为句柄开不出那一支。
- Standards：三项的叫法在 `SKIPPED` 与 `redone_row` 各写一遍（Duplicated Code）。收下：收进 `SHARED_BASIS`，添一条用例核 `SKIPPED` 里有它们。
- Standards：`pipeline::either` 逐格读 `WhyRedone`（Feature Envy）。收下：挪成 `WhyRedone::together`。
- Standards：两条用例里卷名写了两遍。收下：从路径取。`Swap` 的 `Mutex<(bool, bool)>` 换成具名两格。
  `render` 用例手抄了 `archive_was_replaced` 的句式（第二份措辞），换成随便一句。

**驳回的**：

- `CONTEXT.md` 没添 `WhyRedone`／退回串行：spec《词条随票落地》明写「其余几件是报告的措辞，词汇表不动」，story 39 同义。
- ADR 0016 决定第 4 条「这样的行有八种」：落地前就已经不全（按页跳过、纸色提白注解都不在里面）。ADR 是决定记录，补全那张单子不归本票。
- `what_the_head_says` 与 `written_family` 都探那两个头名，回头还多读一遍（Duplicated Code）。
  驳回：两处要的东西不同，一个问来路齐不齐，一个问头一张说了什么。多读的只在要重做的页上，比起重做一页的解码不算账。函数文档写着这条。
- `fell_back_to_serial` 是 `Option<String>`、`render` 只问在不在（Primitive Obsession）：票面要的正是「那句话随卷报告带出来」，印不印记在 Q1237。
- `volume_fingerprint` 交回匿名元组：调用方只有 `process_volume` 一处，当场按名拆开，文档写明第二格是什么。
- 单核机器上那条用例直接返回：只能这样，有先例，见上第 5 条。
- 切开的一族头一张读得出、后面某张读不出，不记「读不出」：头一张已经答出了共用三项，这种半坏的一族要靠外力造。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后，依次 polish → 闸门 3 → 闸门 1 → 闸门 2；日志 `ss-04.polish.log`、
`ss-04.gate3.log`、`ss-04.gate1.log`、`ss-04.gate2.log`，都在树外）。这台机器是 macOS，闸门 1、2 在基线上就各红一条：
`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1114 通过 1 失败**；lib 252 / bin 459；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 68.05s`（`concurrency`，红的是 Q995 那一条） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **981 通过 1 失败**；lib 252 / bin 326；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 74.43s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 7.30s` |

**本票添 8 条用例**，两条闸门各多 8 条：

- bin 里 `render` 4 条；
- `tests/idempotency.rs` 3 条（41 → 44）；
- `tests/concurrency.rs` 1 条（12 → 13）。新添那一条两道闸门上都绿。

lib 的 252 条没添，只改了 `read` 那一条换归档的用例。

**两道钉子逐格没动**：
- `tests/golden.rs` 2 条全过（闸门 1 上 278.43 秒、闸门 2 上 162.46 秒）；`tests/counters.rs` 14 条全过。
- `tests/golden-snapshot.txt` 的 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
- 设计稿与 `tests/fixtures/design/` 一个字节没动；闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：
- `cargo fmt --check` 绿；
- `cargo clippy --all-targets` 绿、零告警；
- `cargo clippy --all-targets --no-default-features` 绿、零告警；
- `cargo doc --no-deps` **告警 15 条**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q528 — 屏上仍说不出「你的旧输出为什么过期了」（接 Q522）

- **From:** 票 `tone-alignment/02`（Q522 的推荐是「让 `02` 接住它」）
- **Kind:** 上一张票记下的缺口，本票只补到了一半
- **Where:** `src/render.rs` 的 `white_align_rows`；同一文件的 `SKIPPED`（幂等命中那一句）
- **Why it did not block:** Q522 说的是：默认取 0 那一趟产物像素逐字节不变，
  **而 PNG 里的参数哈希变了**，`metadata: true` 的用户升级后要整库重跑一次，
  而屏上没有一个字说得出为什么。本票新印的那一行补上了**一半**——每一卷都说得出
  「这一趟的纸白对齐上限是 0 级（没开）」，这个参数从此在屏上有名有姓。
  补不上的是**另一半**：「所以你上一趟的输出过期了」。
- **What this ticket actually did:** 印出参数与它的取值，**不印因果**。
  理由是这个仓库**从来没有一处说得出「这一卷为什么没被跳过」**：幂等命中那一句
  （`SKIPPED`）列的是「四项依据均未变」，而它的反面一句话都没有。
  只为这一个参数开一句因果，等于给那件事开了第二种说法，
  而另外十几个进哈希的参数照旧一个字不说。何况「升级之后要重跑一次」是**一次性的
  迁移事实**，不是这一趟的事实，而报告说的是这一趟。
- **Options:** ① 照现在（印参数，不印因果），迁移那句话归发布说明；
  ② 幂等那一句加一个反面——「这一卷重做了，因为四项依据里的『参数』变了」，
  一句话管住全部进哈希的参数，不是只管纸白对齐；
  ③ 只为纸白对齐加一句因果。
- **Recommend:** ②，另开一张票。它治的是 Q522 真正的病（**任何**参数改动都说不出因果），
  而 ③ 是把一个通病治在一个症状上。① 是眼下的现状，够用但答不出 Q522 那句
  「屏上今天没有一个字说得出为什么」。
- **Whose call:** 拍板的人（「这一卷为什么没被跳过」要不要成为报告的一行，是报告形状的决定）
- **处置：** **`say-and-stop/04` 落地（2026-10-02）：照票面收（选项 ② 的形态，一句话管住全部进哈希的参数）。**`VolumeReport::why_redone` 交出共用三项哪几项变了、或记录读不出，卷级印「重做 之前转换过，但……变了」；参数哈希只说得出「选项变了」。见本票《落地记录》与 Q1237–Q1240。

#### Q222 — 退回串行这件事用户看不见：那句「为什么」眼下只有用例读得到

- **From:** 票 `p4-parking-lot/12`
- **Kind:** 你确实拿不准的单项
- **Where:** `src/source.rs` 的 `Independent::Replaced`；`src/read.rs` 的 `reads`
- **Why it did not block:** 核不上时那一句话是拼出来了的（两个印记都摆着、指得出是哪一个卷、
  说得出接下来怎么走），可它**没有去处**：`read.rs` 那一支把它绑成 `_why` 就丢掉了。
  要让它到用户手上得动 `crate::report` 的 `Report`、命令行那一侧的 `render::tail`、
  会话那一侧的画法——而本票的文件清单写死了只动两个源文件；票面自己也写着
  「不新开对外 seam……别新开第五张表」。
  眼下不咬人的理由与 11 号票那条规矩不冲突：**没有任何一样东西没被处理**。
  这一卷、其余卷、其余页一页不少，变的只是这一卷这一趟不吃并发——用户看到的是慢一点，
  不是少一点。11 号票那条规矩管的是「报告里说一样东西没被处理」，这里没有那句话要说。
- **What this ticket actually did:** **不进报告，把那句话留在 `Independent::Replaced` 上，
  由 `source` 的两条用例读它**（`a_reopened_handle_says_when_the_archive_is_no_longer_the_one_it_opened`
  断言它指得出卷名、说得出「退回串行」）。三处各写了一句指着本条：那个变体的文档、
  `read.rs` 那一支的注释、以及本票的落地记录。
- **Whose call:** 拍板的人（一趟里有卷退回过串行，报告要不要说；若要，是进 `IoPlan` 那一栏
  「真派了几条」，还是另起一句「这一卷的源在跑的过程中变过」）
- **处置：** **`say-and-stop/04` 落地（2026-10-02）：照票面收（另起一句，不进 `IoPlan` 那一栏）。**`Independent::Replaced` 那句话随串行那一批交出去（`read::Reads::fell_back`），原样进 `VolumeReport::fell_back_to_serial`，卷级在读法那一行底下印「这一卷的源在跑的过程中变过，退回串行读」。见本票《落地记录》与 Q1237、Q1239。
