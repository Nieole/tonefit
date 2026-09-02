# 13 — 吞吐只有目标的一半

**What to build:** `fast` 模式达到 spec 定的每秒八十页。

实测一卷 180 页、固态盘、release 构建，两趟分别是每秒 41.9 与 44.7 页。参照系：同一批素材上纯解码满核可达每秒 140 页，因此解码约占三分之一预算，其余在缩放、判据、量化、编码、缓存上。

具体瓶颈在哪尚未测量——本票要先 profiling 再动手，不要凭猜优化。

**卷级三段计时已经在 `Report` 里了**（11 号票：幂等这一道 / 第一遍 / 第二遍，见 `VolumeTiming`）——
本票禁的是比它更细的那一层。取数走两条路，都不要把**解码/缩放/判据/编码**的分阶段计时塞进 `Report`：
事件流（ADR 0011：进度是事件流，
开工前先预扫）给得到卷级与页级的粒度；再细的走 feature-gated 插桩或外部 profiler。

那批页绝大多数不需要缩放，编码器在写全尺寸产物，是最贵的情形，而这正是网络发布版素材的常态。

**Blocked by:** `p0-hardening/11 — Report 补上计时`（没有计时就没有可调的依据）。

**排期建议**：宜在 `page-geometry` 全部落地之后。页几何把跨页卷的输出规模改到约三倍，
在那之前调出来的吞吐结论会被它作废。

**Status:** resolved

- [x] 先出一份分阶段耗时剖面，指明前三大开销。**`VolumeTiming` 那三段答不了这一条**——
      三个大头全挤在第一遍那一格里，要走事件流或 feature-gated 插桩才拿得到
- [x] 剖面结论进 `docs/measurements.md`
- [ ] `fast` 模式在同一批素材上达到每秒八十页 —— **达不到，改判**，见下一条
- [x] 或者依据剖面修订 spec 的吞吐目标，写明理由
- [x] 优化不改变任何判定结果，黄金快照可证

## 落地记录

**量具**：feature-gated 的分阶段插桩（`src/cost.rs`，特性 `profiling`，默认整个不在）。
它按**阶段**累加各线程墙钟之和并印到 stderr——分阶段计时一格都没有进 `Report`，
《卷级计时》那三段一个字没动。新词《阶段》进了 `CONTEXT.md` 的《管线》。

**前三大开销**与**改判后的目标**都在 `docs/measurements.md` 的《分阶段耗时剖面》，
新基线在《端到端吞吐》，三段的墙钟占比在它的小节《三段各占多少》。数不在本文重复。

**八十页/秒达不到，按素材形态改判成四档**，理由写在《吞吐目标：八十页/秒不成立，改判》：
判据那一格削不掉，而把它当成零推出来的上界仍够不着八十；页几何又把跨页卷的输出规模
改到约两倍，立票时那个参照系已经不存在了。数与推法都在那一节，本文不复述。
**那个「八十」翻不到 spec 出处**——`.scratch/` 下没有一份 spec 写着它，只在本票票面上，
因此改判落在 measurements（停车场 Q80）。

**四处优化，全部逐位不变**，黄金快照一格没动：

1. 低通纵向那一趟改为**分带扫列**（`metric::sweep_columns`），一次走 16 列。
   每列各自一个累加器，加减次序一格没动——`the_column_sweep_matches_one_column_at_a_time`
   钉的是 `to_bits()` 逐位相等。
2. 分块遍历交给闭包的改成**一行的下标区间**，一行只查一次边界。
   累加器仍由 `Tile::mean` 持有并原样传进去：按行各求各的再加起来就是换了折叠次序。
3. 误差扩散里的 `f32::round` 换成 `quantize::nearest_index`。基线指令集里没有
   「五入远离零」，编译器只能退到 libm 的 `roundf`，**一个像素一次函数调用**。
   `rounding_agrees_with_the_standard_one` 在每个半格前后逐 ULP 走满，
   区间其余部分等距抽样。
4. 转灰加一格**上一像素的缓存**（`gray::Memo`）。转灰在源尺寸上做，8K 全彩页上曾是
   那一卷最大的一格，而一次换算是三次开立方加一次幂。命中与否都不改变答案，
   `the_memo_hands_back_what_computing_every_pixel_would` 比的是整页字节。

**行为一个字节都没改**：`Report` 的字段、`VolumeTiming` 那三段、命令行与会话两路、
`IoPlan`、幂等这一道，全部原样。`src/read.rs` 只挨了两处插桩（掐取字节那一步），
读取策略与交付次序一格没动。

**买到了什么**：四卷各三趟与基线交替跑，中位对中位分别快 7.5% / 8.7% / 19.4% / 6.6%，
数在 measurements 的《端到端吞吐》。除黄金快照外另有一道旁证：同一批真实素材上，
基线与本票的产物**逐页字节相同**——棋魂 230 张、改革之獸 366 张、哆啦A梦 780 张，
合计 1376 张输出页逐张比过哈希，归档卷与目录卷两种形态都走到。
（归档**容器**本身只差 ZIP 条目的时间戳，那是两次运行时刻不同，与本票无关。）

**量了但没收**：归档卷上最大的那块结构性开销——幂等要把源字节再串行解压一遍
（占比见《三段各占多少》）。收它要改 `CONTEXT.md` 里「归档读取恒串行」那条领域决定，
是另一张票（停车场 Q78）。

## 停车场结转

落地途中由本票了结的条目，以及本票记下、后来由别的票了结的那几条——原文照搬，
处置注明是哪张票做的。

### Q77 — 卷报告现在每卷拼两次，而命令行那一趟只用得上第二次

  - **From:** 票 `p1-session/14`
  - **Kind:** 路过发现
  - **Where:** `src/lib.rs` 的 `process_volume`（`assemble` 闭包，决策点与收摊各调一次）；
    `OutputPage::to_report`；`src/progress.rs` 的 `Events::ask_before_the_second_pass`
  - **Why it did not block:** Q52 要决策点带一份报告，而第二遍夹在两次之间，
    逐页那一步因此从「吃掉这一页」改成「借着算」——每页多复制两条路径与一份判据曲线，
    外加每卷多读一次缓存用量（抢一次那把锁）。
    **命令行那一趟也照付**：`Bar` 是一个真观察者，`sink.is_some()` 那道短路挡不住它，
    而它的 `match` 根本不接 `PassStarted`——拼出来的那一份转手就扔。
    眼下不咬人的原因是量级：一页多三次小分配，而同一页刚刚被解码、缩放、算过判据；
    编好的字节与缓存序号本来就不进报告，**这一份因此不含任何像素那一侧的东西**。
    全量 542 条与两条黄金快照一秒没多走。
    真要收，得让观察者说得出「我要不要那一份」——那是往 `Progress` 这个公开 trait 上
    加一个方法（`fn wants_the_summary(&self) -> bool { false }`），
    为一笔量不出来的开销加宽一个 seam，本票判不值。
    另一条是把那一份做成惰性句柄（事件带一个「要的时候现拼」的东西），
    而那要给 `Event` 引入一个带借用的可调用类型，比它省下来的东西贵。
  - **What this ticket actually did:** **拼法只留一处（一个 `assemble` 闭包），
    并挡住了「没人可问也拼」那一种。**`ask_before_the_second_pass` 收的是闭包而不是值：
    一趟没有观察者（命令行不带进度条那条路、库的多数用例）就连拼都不拼、连表都不掐。
    `--dry-run` 那一路更是连决策点都不报，一次都不拼。
    没有测速：这一处的开销比一次解码低几个量级，而仓库里量数字的地方只有
    `docs/measurements.md`，往那里加一条量不动的数不如不加。
  - **Whose call:** 拍板的人（`Progress` 要不要多一个「我要不要那一份」的问句）

  **处置**：本票的剖面把它量出来了——「拼报告」那一格四卷分别是
  0.000 / 0.000 / 0.001 / 0.003 秒，占比一律读作 0.0%（measurements 的《分阶段耗时剖面》）。
  **不值得收**：`Progress` 那个公开 trait 不加问句，惰性句柄也不做，拼法仍是一处闭包。
  条目当初说「量不动的数不如不加」，而现在有一个量具量得动它，数因此进了 measurements——
  这条不再需要拍板。

- **State:** settled

### Q79 — `profiling` 特性不在 `cargo test` 的闸门里，clippy 也扫不到它

- **From:** 票 `p0-hardening/13`
- **Kind:** 路过发现（本票引进的）
- **Where:** `Cargo.toml` 的 `[features]`；`src/cost.rs`
- **Why it did not block:** 闸门只有 `cargo test` 一条，而它按默认特性走；
  `cargo clippy --all-targets` 同理。`profiling` 不在 `default` 里，
  于是 `src/cost.rs` 里 `tally` 那一半一条自动检查都没有——`profiling` 关着时它整个不编译。
  把它塞进 `default` 不对——那会让每一次量化、每一次低通都多掐两次表，
  而这个量具存在的前提正是「默认整个不在」。
  真要盖住，得让闸门变成两条命令（`cargo test` 再加一条 `cargo check --features profiling`），
  那是改闸门，不是改代码，本票不擅自动。
- **What this ticket actually did:** **把够得着的那一半拉进了闸门，其余手动过。**
  `ALL` 与 `Stage::name` 的 `cfg` 写成 `any(feature = "profiling", test)`，
  于是默认特性的 `cargo test` 也编译它们，`the_stages_are_their_own_index` 与
  `every_stage_has_a_name_of_its_own` 两条用例跟着跑——「加一格却漏在 `ALL` 里」
  这种静默漂移现在红得出来。计数那一半（`tally`）仍在闸门之外：
  落地前手动跑过 `cargo check --features profiling`、`cargo check`、
  `cargo check --no-default-features` 与 `cargo clippy --features profiling`，均无告警。
  复现剖面的命令写在 `src/cost.rs` 的模块文档与 measurements 的《分阶段耗时剖面》施测行里。
- **处置：** 由 `p2-loose-ends/01` 了结。**闸门真的变成了三条**，第三条正是本条预言的那一句
  `cargo check --features profiling`（三条与各自盖住什么见 `docs/agents/gate.md`）。
  代码这一侧比本条预期的多动了一刀：挑行与排版从 `tally` 里摘成两个纯函数，
  默认那一趟就验得到，`tally` 里只剩往原子表上加数与从它上面读回来。
  **`profiling` 仍然不在 `default` 里**——那个量具存在的前提没有松动。
  改法与数在 `p2-loose-ends/01` 的《落地记录》里，本条不复述。
