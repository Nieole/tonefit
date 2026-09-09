# 04: 给「这件事没发生」加得上断言（prefactor）

**What to build:** 报告上多两个**窄计数器**——这一卷缩放了几次、几张参照进了缓存。它们不进渲染、不上屏，只为让用例钉得住「某个开关关掉之后，某件工作确实没有发生」。既有的解码计数就是这个先例：它一处都不显示，却钉着「跳过的卷还是解码了」这类不变量。

这是 05、06、07 的 prefactor：那三票删掉的都是没有外部信号的白付，没有这两个数就只能靠「行为不变」兜着。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 报告上多出缩放次数与参照进缓存次数两个数
- [x] 两个数都不进渲染出的文字，屏上一处不露面
- [x] 现有行为下两个数是真的：用例先钉住**今天**的取值，才谈得上后面几票把它降下来
- [x] **不把阶段耗时那张表整个公开**——那张表是各线程墙钟之和，是记账，不是不变量
- [x] 计数是原子的，不因第一遍满核并行而失准

## 落地记录

### 两个数长什么样、摆在哪一层

报告上多两格，`VolumeReport` 的兄弟字段，与既有的 `decodes` 同形：

| 字段 | 数的是 | 记在哪 |
|---|---|---|
| `VolumeReport::resizes` | 这一卷把图缩到目标尺寸几次，按**输出页**数 | `resample::Resampler`（`AtomicUsize`） |
| `VolumeReport::cached_references` | 这一卷有几张**参照**进了缓存 | `cache::PageCache::references` |

**记在动作本身上，不记在调用方的循环里。**`decode::Decoder` 的文档为这条口径写过一遍
（「解码只此一条路，第二遍要是回头解一页，这个数瞒不住」），两个新数照抄它：
`resample` 原先那两个自由函数 `resize` / `resize_color` 收成 `Resampler` 的方法，
共用的那一段降成私有的 `fn scale`；缓存那个数加在 `PageCache::insert` 里。

**缩放按「张」记，一张彩页记一次而不是三次**——三个通道各走一遍是一张图的内部构造。
口径与 `decodes` 那句「数的是这一页被解了几回，不是解码器被叫了几回」是同一条（Q385）。

`process_volume` 里解码器与缩放器装成一个 `ComputeCounters`：加第八个参数会撞上
`clippy::too_many_arguments`，而仓库一处 `clippy::allow` 都没有（Q386）。
**它不叫 `NarrowCounters`**——那个词指三个数，而第三个不在计算层；名字取自
`CONTEXT.md` 的《读取层 / 计算层》（审查抓出来的）。

两处口径写进了文档：`resizes` **缩到一半失败的那一张也算**（问的是「走没走这一趟」），
`cached_references` 反过来——溢写写盘失败不记，那时缓存里确实没有这一页。
`decode::Decoder::new()` 与新写的 `Resampler::new()` 一并删掉：`ComputeCounters` 走
`Default` 之后它们是死代码，而 `polish` 那两遍 clippy 要求零告警。

`CONTEXT.md` 的《管线》新增词条**《窄计数器 (Narrow counter)》**，三个数（解码、缩放、
参照进缓存）在那里有一处共同的定义，外加一句把它与**用量**分开。

### 05 / 06 / 07 拿它们钉什么

- **05**（试算不为彩页做缩放）：`resizes` 直接钉得住。`tests/counters.rs` 的
  `a_color_page_is_resized_once_and_never_cached` 与
  `a_dry_run_does_exactly_as_much_first_pass_work_as_the_real_thing`
  （05 落地时改名为 `a_dry_run_skips_only_the_color_resize_of_a_mixed_volume`——
  原名在 05 之后就是假话了）是它的对照组——
  今天彩页那一份是 1、试算与照做一样多，05 之后彩页那一份归零而灰度那一份不动。
  **05 要另立一个全彩卷**：`resizes` 是卷级合计、不分彩灰，混合卷上它只从 2 降到 1，
  票面「降到零」的字面要一张灰度页都没有的卷才落得下（审查提出）。
- **06**（顶死的判定不攒参照）：`cached_references` 钉得住。
  `a_pinned_verdict_still_caches_every_reference_today` 钉的正是今天不为零那个事实。
  **但这条要说清**：`VolumeReport::cache.pages` 今天已经等于同一个数，06 不加这张票也钉得住
  （Q383）。这个新数买的是「不上屏」和「12 号票之后仍然只数参照」。
- **07**（两处没有消费者的白造）：**这两个数一个都用不上**。命令行那一路白拼的卷报告与
  关掉记录时白造的来路都不经过缩放与缓存，07 票面自己也写着「如果找不到站得住的断言，
  就在票里说清并留一条注释」。这张票没有替它多加第三个数——那要等 07 自己说得出要量什么。

### 屏上一处不露面

`src/render.rs` 新增 `the_rendered_text_says_nothing_about_the_narrow_counters`，
照既有的 `the_rendered_text_says_nothing_about_how_long_it_took` 办：同一份报告只把三个数
换成扎眼的值，四段（抬头、卷行、页行、末尾）加拼起来的那一份逐字节相同。
不写成「文字里找不到 222」——那只挡得住恰好那一个写法。

### 用例

`tests/counters.rs` 七条，全落在 `run(Request) -> Report`（spec 点名的接缝）：
灰度卷三数相等、跳过的卷全零、彩页缩一次且不进缓存、试算与照做一样多、
拆分页解一次缩两次、顶死的判定今天照攒参照，加上**满核并行之下两个数仍然准**
（24 页的长卷 + `IoMode::Concurrent`，照 `tests/concurrency.rs` 那条解码用例办；
审查指出小卷问不出这件事）。

**阶段耗时那张表没有被公开**，一个字段都没动（票面第 4 条）。

### 停车场

新记 **Q383–Q387** 五条：参照那个数与 `cache.pages` 今天恒等而票面没提（Q383）、
参照那个数不是原子的（它在缓存那把锁里串着走，Q384）、缩放按张记不按调用记（Q385）、
为躲开 clippy 参数上限新造 `ComputeCounters`（Q386）、
加一个报告字段要在十四处夹具字面量上各补一行（Q387）。

### 审查改了什么

`/code-review` 两轴各报了几条，照单改的有五处：`src/geometry.rs` 那句指着
`crate::resample::resize_color` 的注释被这张票改失效了（**稳定引用**那条硬性违规）；
`cache.rs` 与 `report.rs` 两段文档近乎互抄（**单一出处**），分工现在只写在
`VolumeReport::cached_references` 上，`PageCache::references` 只留一条给改那个模块的人的规矩；
类型改名；补并发用例；补两处失败口径。夹具那条 shotgun surgery 记成 Q387，不在这张票做。

### 数

`cargo xtask gate` 三条全绿：

```
绿　闸门 1 · 默认构建　（目录 target）
   cargo test
   合计 845 通过 0 失败；lib 230 / bin 335
   末行 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
绿　闸门 2 · 甩掉终端库　（目录 target/gate/no-default-features）
   cargo test --no-default-features
   合计 717 通过 0 失败；lib 230 / bin 207
   末行 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
绿　闸门 3 · 开着量具　（目录 target/gate/profiling）
   cargo check --features profiling
   末行     Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.25s
```

`cargo xtask polish` 四条全绿：`cargo fmt --check` 干净，两遍 clippy 零告警，
`cargo doc --no-deps` 仍是 **15 条告警**（`tonefit (lib doc) generated 15 warnings`），
与既有基线一致，一条没多。

三条都跑：改动动了 `src/render.rs`（闸门 2 点名）与 `cost::stage(Stage::Assemble)`
那个掐表点闭包的形状（闸门 3 点名）。

**基线本身与派活说明对不上**：说明写「闸门 1 此刻 lib 229 / bin 332」，
动手前在这棵树上量到的是 **lib 230 / bin 334**。这张票只加了 1 条 `src/` 里的用例
（`render.rs` 那条渲染守卫），bin 因此是 335；`tests/counters.rs` 七条走的是自己那个二进制，
不计进 lib/bin 两格。

**并发那条用例在满载机器上没有不稳**：闸门 1 与闸门 2 各跑一遍都过，
而那两趟正是四棵 worktree 并排、机器满载的时候。

两趟都带 `TMPDIR=<worktree>/.tmp` 跑（Q340，本票不重复记）。
