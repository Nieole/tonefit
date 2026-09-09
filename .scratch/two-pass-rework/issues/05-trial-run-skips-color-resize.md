# 05: 试算不再为它根本不会编码的彩页做缩放

**What to build:** 彩色面板上跑 `--dry-run` 时，彩页不再走整套缩放。试算本来就不编码，而编码是缩放结果的**唯一**消费者——今天那一整趟三平面的预缩加卷积跑完就当场丢掉。缩放要报告的那个比例只靠源尺寸与目标尺寸做算术，不需要像素。

**Blocked by:** 04

**Status:** resolved

- [x] 彩色面板 + 试算：彩页的缩放次数降到零（用 04 那个计数器钉）
- [x] 报告里那一页的缩放比、目标尺寸、彩页标记一个字不变
- [x] 黑白面板不受影响——那上面彩页转灰后走灰度路径，缩放照做
- [x] 照做那一趟（非试算）不受影响
- [x] 峰值内存不再为试算的彩页留目标缓冲

## 落地记录

### 改的是一处：`Compute::color_page` 那个 `match`

`src/lib.rs` 里缩放与编码两条顺序语句收成一个 `match request.mode`：

- `Mode::Process` 照旧——缩、编、进报告，一个字节都不变。
- `Mode::DryRun` 只留 `Scaling::plan(image.size(), size)`。它与 `resize_color` 内部第一句
  **逐字相同的表达式**，报告那一格因此是等价而不是近似。

`Record::color` 的构造一并挪进 `Process` 那一支：它只喂给 `encode::color_png`，
`src/` 里没有第二个读者，试算下同属白付。

**灰度路径一个字没动**：那边缩放结果还有判据这个消费者，而试算存在的理由正是预告那个判定。

### 「降到零」是怎么落下来的：造全彩卷，没有改验收的读法

票面第一条验收在现有混合卷夹具上落不下来——`VolumeReport::resizes` 是**卷级合计、不分彩灰**，
一彩一灰那张卷上 05 之后只从 2 降到 1。`tpr/04` 的落地记录已经写明这一层
（《05 / 06 / 07 拿它们钉什么》：「**05 要另立一个全彩卷**」），本票照办：新夹具是
**两张彩页 + 彩色面板 + 试算**，那一卷一张灰度页都没有，`resizes` 于是落到字面上的零。

**票面那句「降到零」一个字没改。**

### 前提验过：编码是缩放结果的唯一消费者

这是整张票的地基，落地前先在代码里核过：`color_page` 里 `resize_color` 交出的 `scaled`
只出现在 `encode::color_png(&scaled, …)` 一处，`Outcome::Processed` 的每一格都不吃它。
报告那三格各有各的来源，全都不碰像素——`size` 出自 `FitMode::target`（只吃两个尺寸）、
`color` 是入参（在 `color_page` 之外就定好了，ADR 0010 决定第 3 条）、`scaling` 出自
`Scaling::plan`（只吃两个尺寸）。`/code-review` 的 spec 那一路独立核了一遍，结论相同。

### 用例四条

`tests/counters.rs`：

- **`a_dry_run_of_an_all_color_volume_resizes_nothing`**——正题，`resizes == 0`。
  同一条里钉住 `decodes` **不**跟着降：彩页识别排在解码之后，不解就认不出这是一张彩页。
- **`a_dry_run_skips_only_the_color_resize_of_a_mixed_volume`**——由
  `a_dry_run_does_exactly_as_much_first_pass_work_as_the_real_thing` **改名**而来。
  05 之后「与照做一样多」就是假话，而用例名也是一种断言；断言随之改成
  「差的正好是彩页那一次」，另两个数逐个相同。改名留下的悬空引用记 **Q464**。
- **`a_dry_run_on_a_monochrome_panel_still_resizes_every_color_page`**——第三条验收。
  同一卷、同一个模式，只换一台设备，两个数就都回来了：省掉的是**彩色分支那一条路**，
  不是「试算」这个开关。

`tests/pipeline.rs`：

- **`a_dry_run_reports_the_same_color_geometry_as_the_real_thing`**——第二条验收，
  试算与照做两趟比 `size`、`scaling()`、`color()`。夹具窄而高（64×3360），照
  `NARROW_PASSES_THROUGH` 的路子办：这一条只问几何，页上画着什么不影响，而高是面板的两倍，
  `prescaled()` 因此为真——省掉的那一趟不是空操作，用例里那句断言兜着这个前提。
  （起初取 `TWO_AND_A_HALF_PANEL`，一条用例跑 60 秒以上；换窄页后 0.95 秒。）

**第五条验收（峰值内存）没有直接断言。**spec 的《Testing Decisions》说好的用例只测外部行为，
而内存不是外部行为。目标缓冲只在 `resize_color` 里建，`resizes == 0` 蕴含它一次都没分配——
窄计数器就是它的代理信号，这也正是 04 加这个数的理由。

### `/code-review` 改了什么

- **硬违规一处**：`Scaling` 那句「一页**实际走过**的缩放」在三处
  （`resample.rs` 的类型文档、`report.rs` 的字段、`report.rs` 的访问器）。05 之后
  彩色分支试算那一趟一次都不缩，这个值出自纯算术，那句话成了假话。权威定义收到**类型文档**
  上（「只由两个尺寸算出，不需要像素」），另两处引用它——同一句从写三遍变成写一遍。
- 同一个函数里「编码是唯一消费者」写了两遍（函数文档 + 紧随的行内注释），收成一遍；
  那个事实的权威位置定在 `Resampler::resize_color`。
- **Q465 的《Where》补上两处 ADR**：`docs/adr/0010` 决定第 5 条与第 6 条与 `CONTEXT.md`
  那句是同一个口径缺口，只改一处不算改完。

**没照办的一条**：审查建议把 `tests/counters.rs` 新加的 `dry_run_with` 搬进
`tests/fixtures/mod.rs`。不搬，两条理由——`tests/pipeline.rs` 里既有的五条 dry-run 用例
**全都**就地拼 `Request`，那是那个文件的邻居惯例，新用例照抄邻居；而 `fixtures/mod.rs`
是几棵 worktree 共写的热点（Q387 记的正是它）。`dry_run_with` 只服务 `counters.rs` 里
三条用例，留在本地。

### 停车场

**Q464**——`tpr/04` 的落地记录点名了被本票改名的那条用例，成了悬空引用；没有回头改别人票的文件。
**Q465**——`CONTEXT.md`《灰度路径 / 彩色分支》的「只缩放并编码」加 ADR 0010 决定第 5、6 条，
在试算那一趟两样都不做；照 `CLAUDE.md`「撞见词汇表与实现对不上，记进停车场，不要顺手改」办。

### 数

`cargo xtask gate` 三条全绿：

```
绿　闸门 1 · 默认构建　（目录 target）
   cargo test
   合计 864 通过 0 失败；lib 232 / bin 345
   末行 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
绿　闸门 2 · 甩掉终端库　（目录 target/gate/no-default-features）
   cargo test --no-default-features
   合计 736 通过 0 失败；lib 232 / bin 217
   末行 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
绿　闸门 3 · 开着量具　（目录 target/gate/profiling）
   cargo check --features profiling
   末行     Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s
```

`cargo xtask polish` 四条全绿：`cargo fmt --check` 干净，两遍 clippy 零告警，
`cargo doc --no-deps` 仍是 **15 条告警**，与基线一致，一条没多。

**三条都跑**：改动落在 `src/lib.rs` 与 `src/report.rs`，两条命令行外的构建各自看得见。

**lib / bin 两格本票一条没动**（232 / 345）：四条新用例全在 `tests/` 自己那个二进制里。
合计 864 = 基线 860 + 那四条。`tpr/04` 记的 845 / lib 230 / bin 335 是它自己分支上的数，
基线 `6e1718b` 之后又并进了几张票，对不上是正常的——**以这棵树上量到的为准**。

两趟都带 `TMPDIR=<worktree>/.tmp` 跑（Q340，本票不重复记）。闸门起在 `setsid` 的独立会话里：
冷树上一趟要十几分钟，跑在调用方的进程组里会被超时连根杀掉，而被杀的样子很像闸门红。

> **合流补记（改号）**：本票记的那两条停车场原编号是 `Q433`／`Q434`，合流时改成
> **`Q464`／`Q465`**。同一个号在另一台机器上被并行发给了别的两条（那两条已被 7 个文件 10 处引用，
> 且加它们那条提交写着「找回被合掉的两条停车场」——已经丢过一次）。改号取波及面小的那一侧，
> 本票这四处引用一并改了，条目正文一个字没动。根因见停车场 `Q435`。
