# 06 — 产物体积

**What to build:** 卷级各一行、趟级末尾一行合计，说出产物有多大。口径是**这一趟交出去的输出页与透传文件的字节之和**，不含容器开销——
预览与转换同一个口径；按页跳过留下的页，字节照算。只在白捡的地方给：默认路径与覆盖顶死那一趟，分析环节就把字节编完了，
预览照报；`--envelope` 那条路预览时字节还没编，**整行不出现**（不印零、不印估值）；转换那一趟两条路都有。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `render` 的用例：卷级与趟级各一行；`--envelope` 预览时两行都不在
- [x] 集成用例：默认路径上同一份源，预览与转换报的体积相同（按《预览》词条——会话那一趟、确认点上答做完再停——兑现；命令行 `--dry-run` 上用户故事 23 没落地，待 Q1257 拍板）
- [x] 按页跳过的卷，留下的页的字节算在里面
- [x] 黄金快照为本票的改动显式接受一次，diff 里只有本票那几行——**没有要接受的**：快照一个字节没动，理由见《落地记录》
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 照旧只红基线那一条 Q995）

## 落地记录

**本票做了什么。**

1. **库：`VolumeReport::output_bytes: Option<u64>`，`Report::output_bytes()` 是各卷之和**（`src/report.rs`）。
   口径：这一卷的输出页与透传文件的字节之和，按成员内容算、不含容器开销。口径与何时说得出**只写在那一格的文档上**，别处指过去。
   有一卷说不出，合计就是 `None`。
2. **库：三个来路，一个口径**（`src/pipeline.rs`、`src/lib.rs`、`src/sink.rs`）。
   - **走了写出环节**：写进容器的字节逐个数——`second_pass` 交回页那一笔（重做的、留下的都算），透传文件在 `process_volume` 里加上。两条路都有。
   - **写出之前**（`output_bytes_up_front`）：**按路说**——只在分析环节就编好字节的那两条路上（`Settles::encodes_in_the_first_pass`：默认那条路与顶死的那一趟，
     照做那一趟），而且要写的每一格此刻都有字节。灰度页的长度记在 `Branch::Gray` 新添的 `encoded` 上；彩页问它编好的那一串；留下的页比对时量过
     （`RetainedPage::bytes`）；透传文件读成员表上的 `Member::bytes`。`--envelope` 而没顶死、dry-run，一律 `None`；坏页的占位页要到写出环节才画，也是 `None`。
     确认点上交给观察者的那一份与答了做完再停的那一卷读它；两处都说得出时 `debug_assert!` 钉着与写出那一笔同数。
   - **整卷跳过**：`Reuse::Whole` 多一格 `bytes`，产物就是上一趟写在那儿的那一份（Q1258）。
   - 长度都是顺手量的：`Written::page_of` 读记录时连长度一起答（目录那一支问已开的句柄，归档那一支问中央目录），`Written::length_of` 答透传文件多大
     （卷级那条路上它兼作「都还在吗」）；`record_of` 是 `page_of` 的一层，`holds` 是 `length_of` 的一层。
3. **命令行报告两种新行**（`src/render.rs`、`src/render/plain.rs`）。
   - 卷级 `RowKind::OutputBytes`（一格 `Field::OutputBytes`，值走 `tonefit::format_bytes`，列头「产物」在纯文本那一副）：「  产物 3.5 MiB」。
     收住「这一卷怎么来的」那一组：按页跳过、为什么重做（`say-and-stop/04`）之后、卷级判定之前；没有那两行的卷接在卷那一行（与过期副本）底下（Q1261）。
     说不出的卷整行不在。
   - 趟级 `RowKind::OutputTotal`，末尾那几小结的头一行：「产物合计 2 卷 · 1.5 MiB：各卷输出页与透传文件的字节之和，不含输出容器自身的开销」。
     有一卷说不出、一卷都没有，整行不在。折起那一副照印；会话退出时那一份走的是同一处。
4. **用例**（取值一律点名，路一律写出 `envelope`）。
   - `render` 两条新：卷级那一行的去处（按页跳过加为什么重做那一卷、整卷跳过那一卷各一串）、`None` 时纸上不留一个「产物」；
     合计是末尾的头一行、落在正文之后隔离那一小结之前，有一卷说不出与一卷都说不出（`--envelope` 预览）时两行都不在，一卷都没有时不出。
     `the_volume_and_page_lines_are_rows_that_each_say_what_they_are` 的行序补上新那一行。
   - `tests/container.rs` 一条：目录卷与归档卷、默认与 `--envelope` 两条路，转换报的就是容器里成员字节之和，归档文件比它大，合计是各卷之和。
   - `tests/resume.rs` 五条（确认点上答做完再停＝会话的预览，再转换一趟）：默认路径与 `--envelope` 底下两维点死，预览、确认点那一份、转换三个数相同
     （灰度页、彩页、透传文件各有）；`--envelope` 预览不报、转换报（含全彩的一卷，Q1260）；dry-run 两条路都不报（Q1257）；
     有坏页的卷预览不报、转换报（Q1259）；按页跳过的卷，转换报的是容器里全部成员（留下的那一页算在里面），预览同数。
   - `tests/idempotency.rs` 一条：整卷跳过的卷（目录卷两条路、归档卷）报的与写它那一趟相同，dry-run 预告的跳过同样。
5. **黄金快照没有要接受的**：`tests/golden.rs` 自己拼卷行、页行与成员字节数，链的是库、够不着命令行报告；`VolumeReport` 的新格它不读。
   `tests/golden-snapshot.txt` 的 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，`TONEFIT_ACCEPT_GOLDEN` 没设过。
6. **没改的**：`CONTEXT.md`（spec《词条随票落地》「其余几件是报告的措辞，词汇表不动」、story 39）；设计稿与会话屏（屏上出体积是设计稿的事）；
   会话代码只在两处夹具里补 `output_bytes: None`（`src/session/scene.rs`、`src/session/live.rs`），`src/progress.rs` 一处夹具同。

**票面前提的一处订正（Q1257）——用户故事 23 在命令行那一路上没有落地。**票面说「默认路径与覆盖顶死那一趟，分析环节就把字节编完了，预览照报」。
这只对照做那一趟（`Mode::Process`）成立：命令行的 `--dry-run` 照 ADR 0005《覆盖顶死的那一趟不必等到第二遍》里预览那一条一页不编
（`Settles::for_this_run`），彩页连缩都不缩。本票按票面自己那条规矩（只在白捡的地方给，不印零、不印估值）走：确认点上的预览（会话那一趟）
在默认路径与顶死那一趟上报，命令行 `--dry-run` 要处理的卷两条路都不报。验收第 2 格因此是按《预览》词条（会话那一趟）兑现的；
「先看一眼装不装得下再决定转不转」要在命令行上兑现，得改 ADR 0005 那一条，归拍板的人。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- `Report::output_bytes` 改成只加说得出的那几卷：合计那条 `render` 用例红在「有一卷说不出，合计却照出」。
- `output_bytes_up_front` 改成跳过说不出的那几格（当时还是逐格判）：头一遍红在 `process_volume` 的 `debug_assert!`（下游那道先挡住了），
  连它一起关掉再跑，`--envelope` 预览、坏页预览、dry-run 三条各红在自己那一句断言上（dry-run 那一半因此拆成单独一条用例）。
- 拿掉 `output_bytes_up_front` 头上按路那一句：`--envelope` 预览那一条红在全彩那一卷上（混着灰度页的那一卷照样 `None`，挡不住）。
- `RetainedPage::bytes` 填 0（连 `debug_assert!` 一起关掉）：按页跳过那一条红在「预览与转换报的体积不同」。
- 写出环节搬留下的页时不记长度（同样关掉 `debug_assert!`）：同一条红在「留下的页没算进产物体积」。
- 整卷跳过不算透传文件：整卷跳过那一条红在目录卷默认路径那一格。
- 新用例落地前各自先红：卷级那一行、合计（末尾头一行是隔离）、转换（`None`）、预览（`None`）、整卷跳过（`None`）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 8a9eb9b`（未提交的工作树）。

**收下的**：

- Spec：派活说明要卷级那一行「与 `why_redone`、`fell_back_to_serial` 那两句排在一起」，初稿摆在卷那一行正下方。收下：挪到为什么重做之后（Q1261）。
- Spec：「`--envelope` 那条路预览时整行不出现」在全彩的一卷上不成立（初稿逐格判）。收下：改成按路说（Q1260），用例添全彩那一卷。
- Spec + Standards：新添的 `CONTEXT.md`《产物体积》与 spec「词汇表不动」相抵，而且与字段文档各说各的（dry-run 预告的跳过照报）。收下：撤掉词条，
  口径只写在 `VolumeReport::output_bytes` 的文档上，别处指过去。
- Spec：story 23 在命令行上没落地、会话里只落一半，票面第 2 格不该读成全兑现。收下：《落地记录》与 Q1257 写明。
- Standards：「ADR 0005 的《试算》」不是一个小节、「试算」是旧称。收下：改指《覆盖顶死的那一趟不必等到第二遍》里预览那一条。
- Standards：用例名与局部量用了 kept，词条是「留下的页 (Retained page)」。收下：`a_volume_skipped_by_page_counts_its_retained_pages`、`retained_bytes`。
- Standards：两条用例借夹具默认的 `envelope: false`。收下：点名写出。
- Standards：`length_of` 与 `holds` 同形。收下：`holds` 改成 `length_of(..).is_some()`。
- Standards：`written` 一处是上一趟的输出、一处是字节数。收下：字节数改叫 `delivered`。

**驳回的**：

- `Written::page_of` 交回 `(PageRecord, u64)` 元组（Primitive Obsession）：`sink` 在 `pipeline` 底下，`WrittenPage` 是 `pipeline` 的私有类型；
  两处调用当场按名拆开，再起一个公开类型只为装两格不值。
- 加一格逼着十来处夹具补 `output_bytes: None`（Shotgun Surgery）：那是 `VolumeReport` 用结构体字面量建夹具的既有形状，`say-and-stop/04` 加两格时同样。
- 整卷跳过照报（Spec 记作范围蔓延）：留着，理由与另一条路记在 Q1258。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后，依次闸门 1 → 闸门 2 → 闸门 3 → polish；日志 `ss-06.gate1.log`、`ss-06.gate2.log`、
`ss-06.gate3.log`、`ss-06.polish.log`，都在树外）。这台机器是 macOS，闸门 1、2 在基线上就各红一条：
`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1127 通过 1 失败**；lib 252 / bin 466；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.80s`（`concurrency`，红的是 Q995 那一条） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **992 通过 1 失败**；lib 252 / bin 330；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.61s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.89s` |

**本票添 9 条用例**，两条闸门各多 9 条：bin 里 `render` 2 条；`tests/container.rs` 1 条（50 → 51）；`tests/resume.rs` 5 条（9 → 14）；
`tests/idempotency.rs` 1 条（44 → 45）。lib 的 252 条没添。

**两道钉子逐格没动**：

- `tests/golden.rs` 2 条全过（闸门 1 上 161.52 秒、闸门 2 上 150.52 秒）；`tests/counters.rs` 14 条全过。
- `tests/golden-snapshot.txt` 的 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
- 设计稿与 `tests/fixtures/design/` 一个字节没动；闸门 1 里比整屏那几条照旧绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：

- `cargo fmt --check` 绿；
- `cargo clippy --all-targets` 绿、零告警；
- `cargo clippy --all-targets --no-default-features` 绿、零告警；
- `cargo doc --no-deps` **告警 15 条**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q801 — 报告里没有产物体积这一行；而 `--envelope` 那条路上预览也报不出

- **From:** 设计评审（`grill-with-docs`，2026-09-17），Q800 的另一半
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/report.rs`、`src/render.rs`；`tonefit::format_bytes`（摊开的字节数在用它）
- **Why it did not block:** Q800 判为不做之后，「装不装得下」的答法是**预告合计体积**，而今天报告里没有它。
- **What this review actually did:** 形态想清了、没有开工。**趟级尾巴一行，卷级各一行**，
  并且**只在它免费的地方给**：默认路径与《覆盖顶死》那一趟，分析环节一页判完当场就编，
  合计体积是白捡的；`--envelope` 那条路缓存里装的是**参照**、量化与编码都在写出环节，
  而预览「一个输出都不落盘」——要在那条路上报体积，只能**白编一遍再把字节丢掉**。
  因此那条路上预览时这一行**整个不出现**，不印零、也不印估值（说不出话要说出来，
  不给一个会骗人的数）。转换那一趟两条路都有。
- **Whose call:** 拍板的人（排期）
- **处置：** **`say-and-stop/06` 落地（2026-10-02）：照票面收，口径与「`--envelope` 预览整行不出现」照办；「默认路径预览照报」只兑现在会话的预览上。**`VolumeReport::output_bytes`／`Report::output_bytes()`，卷级「产物 N」、末尾「产物合计」；转换两条路都有，确认点上的预览在默认路径与顶死那一趟有。命令行 `--dry-run` 要处理的卷两条路都不报——拷问那一轮以为默认路径的 dry-run 分析环节就编了，实际没有（ADR 0005《覆盖顶死的那一趟不必等到第二遍》里预览那一条），用户故事 23 在命令行上因此没落地，见 Q1257。另见本票《落地记录》与 Q1258–Q1262。
