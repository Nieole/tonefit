# 11 — `score` 只收绑定了位深的量化图

**What to build:** 量化交出一个带着它那一档位深的图，`score` 只收它——位深与像素传错一档编译不过，而不是安静地按另一道地板读数。
用例里两处合成候选（没被目标位深量化过）走一个明写的构造，不再靠「传 8bit」的约定。读数一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `score` 的入参是绑定了位深的量化图；调用处全改
- [ ] 两处合成候选走明写的构造
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
