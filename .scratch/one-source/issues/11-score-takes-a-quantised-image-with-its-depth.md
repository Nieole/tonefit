# 11 — `score` 只收绑定了位深的量化图

**What to build:** 量化交出一个带着它那一档位深的图，`score` 只收它——位深与像素传错一档编译不过，而不是安静地按另一道地板读数。
用例里两处合成候选（没被目标位深量化过）走一个明写的构造，不再靠「传 8bit」的约定。读数一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `score` 的入参是绑定了位深的量化图；调用处全改
- [x] 两处合成候选走明写的构造
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 量化交出一个绑着灰阶档位的类型，画质分只收它（收停车场 Q306，走它的 ②）。读数一格不变：`score` 里算的东西一行没动，只是灰阶档位从量化图上取，不再由调用方另交。

1. **类型**（`src/quantize.rs`）：`Quantized`——摊回 8 位工作精度的像素，连同量化它的那一档灰阶档位。造得出它的只有两处：
   `quantize` 交出它，那一档取自候选；`Quantized::at_working_precision(image)` 给没被目标灰阶档位量化过的合成候选，那一档恒是 8bit。
   **没有收「像素 + 一档」的构造**，字段私有。只交 `image()` 与 `bit_depth()`（Q1368）。从 `lib.rs` 公开，crate 文档里 `score` 周边那串公开类型添上它。
   为什么绑在一起，单一出处在 `Quantized` 的文档；`score` 的文档只说为什么不从像素上反推，再指过去。
2. **画质分**（`src/metric.rs`）：`score(reference: &Reference, quantized: &Quantized)`，颗粒可见下限按 `quantized.bit_depth()` 算，函数体其余照旧。
3. **调用处**：`pipeline::candidate_scores` 少一个参数；`pipeline::candidate_bytes` 编码时两样都从量化图上取（按构造就等于 `candidate.bit_depth`，字节不变；`encode::png` 签名没动，Q1370）；
   `src/render.rs` 的 `score(` 调用量化出来的那几处少一个参数，合成的那几处改走 `at_working_precision`、那两行「灰阶档位取工作精度那一档」的注释删了（构造的名字说的就是它）；`src/session/live.rs` 一处合成，同样；
   `tests/metric.rs`、`tests/perceptual.rs` 的 `reading` 两份照旧逐字相同；读像素的地方经 `.image()` 取（`src/encode.rs`、`src/quantize.rs` 的用例，`tests/metric.rs` 的 `pixelwise_rmse`，`tests/alignment.rs`、`tests/pipeline.rs`、`tests/proof.rs`、`tests/perceptual.rs`）。
4. **合成候选**：票面说两处，那是 Q306 记下时的数——7228442 上量到的是 `tests/metric.rs` 两处（`a_slow_ramp_does_not_buy_itself_any_masking`、`the_same_error_counts_for_more_in_a_flat_area_than_in_a_textured_one` 那两个整页 +8 的闭包），外加 `src/render.rs` 八处、`src/session/live.rs` 一处编出来的 1×1。`score` 不再收别的，这几处一律走 `at_working_precision`；`git grep -n 'BitDepth::Eight' -- src tests` 里再没有一处是交给 `score` 的。
5. **词汇表**：《量化》表加《量化图 (Quantized)》一条（新词，Q1367 记命名）。

**留下的口子**：`at_working_precision` 公开，把一张别的档位量化出来的像素塞进去照样编得过，画质分按 8bit 的地板读它。生产路径不调它，构造的文档点了名；要不要连这一个也关上，记在 **Q1371**，归拍板的人。

**添的用例**：

- `quantize::tests::a_quantized_image_carries_the_depth_it_was_quantized_at`——四档 × 两种抖动，量化图带的就是候选那一档。
- `quantize::tests::an_image_at_working_precision_is_one_quantized_at_eight_bits`——合成那个构造与拿 8bit 不抖动量化一遍分不出来（同一档、同一份像素）。
- `Quantized` 文档里一对 doctest（Q1369）：`compile_fail` 那一段只交像素给 `score`；编得过那一段只差末一行，交量化图本身。它守的是 `score` 那一头，守不住「再添一个收像素 + 一档的构造」。

**按反跑的那几遍**（`docs/agents/testing.md`《守卫按反跑一次》）：

- `quantize` 里把那一档写死成 `BitDepth::Eight`、`at_working_precision` 里写成 `BitDepth::Four`：上面两条单元用例各红一条，改回全绿。
- `score` 临时改回直接收像素（`candidate: &GrayImage`，地板取 8bit；`candidate_scores` 跟着交 `.image()`）：`compile_fail` 那一段报 ``Test compiled successfully, but it's marked `compile_fail` ``，编得过那一段也红，两段都红；改回两段全绿。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 7228442`（工作树）。两轴都核过：读数不变（函数体只挪了取地板那一行，`composition()` 是 `const fn`）、合成候选全走明写构造、`candidate_bytes` 写出的字节不变。

**收下的**：

- 留下的那个口子（Spec）：`at_working_precision(quantize(…2bit…).image().clone())` 编得过——构造文档点名，另记 **Q1371**；上面《留下的口子》一段。
- Q1369 选项 ③ 那句「添一个收『像素 + 一档』的构造，一条用例都不会红」对选中的 ① 一样成立（Spec）——收窄成 `score` 那一头，`What` 里写明这一对守不到构造那一头。
- crate 文档里 `score` 周边那串公开类型漏了 `Quantized`（两轴）——补上。
- 票据与停车场里写死的处数（Standards，`issue-tracker.md`《Conventions》末条）——钉上 7228442，或换成数它的命令。
- 《落地记录》第 3 条把 `tests/alignment.rs` 列进「改成 `.image().pixels()`」、漏了 `pixelwise_rmse`（两轴）——改成「经 `.image()` 取」，补上。
- `score` 的参数换了类型还叫 `candidate`、函数体里被同名遮蔽，与 Q1367 否掉 `CandidateImage` 的理由打架（Standards）——参数改叫 `quantized`。
- 访问器叫 `depth()`，同一个概念在 `Candidate` 上叫 `bit_depth`（Standards）——改叫 `bit_depth()`，字段同名。
- 单一出处（Standards）：「传错一档就安静地按另一道地板读数」在 `score` 与 `Quantized` 的文档各写一遍——留在 `Quantized`，`score` 指过去；「8bit 上量化是恒等」在词条、构造文档、新用例文档各写一遍——留在构造文档，用例文档指过去，词条删掉。
- 《量化图》词条前半句说「《参照》按候选量化」，后半句又把用例里编的合成候选收进来，还在跟旧做法辩论（Standards，《文档写作》1、2 条）——后半句删了，词条只说这个领域概念。
- 十一处注释「取工作精度那一档」只是复述构造的名字（Standards）——删了；`src/render.rs` 那几处的 diff 因此也更窄。
- Q1368 一条装了两个岔口（Standards）——访问器形状留在 Q1368，构造的可见性拆成 Q1371。

**没收的**：

- `src/render.rs` 里八段逐字相同的 128 对 136 夹具（Standards，判断题，霰弹式修改的迹象）：重复早于本票；另一个槽正在改 `src/render.rs`，协调人要这几行改得越窄越好，本票不抽。

### 停车场

本票用了 Q1367–Q1371：

- **Q1367**：类型叫 `Quantized`（词条《量化图》），合成候选的构造叫 `at_working_precision`。
- **Q1368**：`Quantized` 只交 `image()` 与 `bit_depth()`，没有 `Deref`、没有转发。
- **Q1369**：「传错一档编译不过」用一对 doctest 钉住（仓库里头一回）；它只守 `score` 那一头。
- **Q1370**：`encode::png` 照旧分头收像素与灰阶档位。
- **Q1371**：`at_working_precision` 公开，是这条硬约束留下的那一个口子——归拍板的人。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-11.gate1.log`、`os-11.gate2.log`、`os-11.gate3.log`、`os-11.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
合计比基线（7228442 上 1146／1005）各多 4 条：lib 两条新单元用例（253 → 255），外加 `Quantized` 那一对 doctest（仓库里头一回有 doctest）。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1150 通过 1 失败**（1 ignored）；lib 255 / bin 478 / doctest 2；末一个 `test result` 是 compile_fail 那一段：`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.50s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1009 通过 1 失败**；lib 255 / bin 337 / doctest 2；末一个 `test result`：`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.33s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.60s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警，没有要修的；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数，没有一条出自本票的文档 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 150.55 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 478 条里，全绿；`a_score_of` 喂给它们的数一格没变。
**幂等用例**：`tests/idempotency.rs` 46 条全过。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q306 — `score` 多收了一个位深参数，而没有东西逼它与手上那张候选图一致

- **From:** 票 `metric-recalibration/01`
- **Kind:** 落地时走的那条路（既有 seam 上加了一个参数）
- **Where:** `src/metric.rs` 的 `score(&Reference, &GrayImage, BitDepth)`；调用点在
  `src/lib.rs` 的 `candidate_scores`、`src/render.rs` 十来处与 `tests/metric.rs`
- **Why it did not block:** 地板改成格点间距的比例之后，判据要知道位深，而 `score`
  从前只收像素。**从候选图上反推格点数不成立**：一张纯色页在任何一档上都只用得着一个格点，
  反推出来恒是 1bit——那恰恰是判据最要说话的那一类页。每个调用点手里本来就有 `Candidate`，
  把位深穿进去是一次编辑。留下的那一格是：位深与候选像素之间没有任何约束，传错一档，
  判据安静地按另一道地板读数、一条用例都不会红。用例里还有两处候选根本不是量化出来的
  （整页 +8 的合成候选），那两处传的是 `BitDepth::Eight`——「没被目标位深量化过」，
  说得通，但那是约定，不是编译器逼出来的。
- **What this ticket actually did:** **把位深顺着 `score` 穿进去**，函数文档里写明为什么不反推；
  两处合成候选按「工作精度那一档」传 8bit，`fixtures::plain(depth)` 那几处一律传 `depth` 本身。
  没有新开类型——spec 的《Testing Decisions》明写「三个 seam 全是既有的，一个都不新开」。
- **Options:** ① 照现在；② 新开一个把「量化过的图」与「量化它的那一档位深」绑在一起的类型
  （`quantize` 出它，`score` 只收它），一处构造、编译期对齐，代价是对外多一个类型；
  ③ 改收 `Candidate`（抖动那一维在判据里用不上，收它是一条假的依赖）。
- **Recommend:** ②，但排在 04 号票（真机标定）之后——那一票落地时判据这一侧还要动，
  两次一起动比分两次便宜。
- **Whose call:** 拍板的人（对外形状要不要多一个类型）
- **处置：** 待处理。
