# 03: 判据参数进哈希，改了它旧输出就过期

**What to build:** 改了地板、掩蔽或聚合里任何一个数之后，用户下一趟跑同一个库，**旧输出自动过期整卷重做**，而不是被幂等静默跳过。今天 `params_hash` 一个判据参数都没收——ADR 0002 的《后果》自己记着这个缺口：「地板不在里面……旧输出会被静默跳过」。

**这一张是 `04` 的安全网，必须在它之前落地。**否则改地板那一趟，用户拿到的是上一趟的产物而不自知，而那**很难发现**——输出看着就是旧的那样。

**Blocked by:** 01

**Status:** resolved

- [x] `params_hash` 收 `Composition`、`Aggregation`、`Masking` 三者的**全部字段**
- [x] 收的是那三个结构体，不是散落的常数——将来加一个字段，哈希自动跟上，不必记得回来改这里
  - **偏差**：落地成**穷尽解构**——加了字段这里**编译不过**，而不是「自动跟上」。见 **Q461**。
- [x] `tests/pipeline.rs` 或 `tests/idempotency.rs` 上有一条用例钉住它：**改了判据参数之后旧输出过期重做**，写法照 `tests/idempotency.rs` 现有那几条
  - **偏差**：用例落在 `src/metadata.rs`（`every_metric_parameter_changes_the_hash`），不在 `tests/`；
    它断言的是「改了判据参数**哈希就变**」，「哈希变了就整卷重做」那一步由既有的
    `tests/idempotency.rs::a_changed_parameter_redoes_the_volume` 接住。见 **Q457**。
- [x] **落地这一张本身就让全库旧输出过期一次**（哈希多了输入）。这一条写进 ADR 0002 的《后果》——不写下来，将来没人知道那次全库重跑是从哪来的
- [x] 黄金快照：判定不变，但记录里的哈希会变。差异审查后显式接受
  - **偏差**：后半句没有对象——那一趟按 `--no-metadata` 跑，快照里既没有哈希那一列、
    字节数也不含 tEXt，**逐字节不变，没有东西要接受**。见 **Q458**。
- [x] 闸门三条绿（`cargo xtask gate`）

## 落地记录

### 一、哈希多收三行，收的是那三个类型

```
metric-composition grain-ratio 0.21568627
metric-aggregation tile 32 quantile 0.99 tail-tiles 8
metric-masking floor 0.5 knee 8
```

摆在 `threshold` 那一行的正下方——判据出量、阈值划界，两者是同一件事的两半，
收一半漏一半正是本票要补的缺口。三处一律**穷尽解构**：

```rust
let Composition { grain_ratio } = composition;
let Aggregation { tile, quantile, tail_tiles } = aggregation;
let Masking { floor, knee } = masking;
```

**票面第二条没有照字面兑现，落地成了更强的一格。**真能「自动跟上」的路有两条
（`Debug`、`serde` 的 derive——这个仓库两样都现成），两条都没走，各自的毛病与翻案代价
记在 **Q461**；为什么这段文本一律按名写死，写在 `params_hash` 的文档注释里。
买到的不是「自动跟上」，是**「跟不上就不让你走」**：往那三个结构体里加一个字段，
这里当场编译不过。代价是加字段的人要回来写一行，而那一行躲不过去。

**取值按 `Display` 原样写，不截精度。**浮点的 `Display` 给的是能还原原值的最短写法；
邻座那一行 `threshold` 用的 `{:.3}` 在这里是个陷阱——小数点后第四位的一次改动会被整个藏起来，
而那正是本几行要拦的东西（`GRAIN_RATIO` 今天就是 `0.215_686_27`）。

**`Aggregation` 那三个数只从 `aggregation()` 取**，`tiles()` 与 `aggregate()` 里的直读常数
一格没动——那是 **Q438** 划的地界，不是本票的。

### 二、接缝：判据那三件由调用方传进来

```rust
fn params_hash(request: &Request) -> String {
    hash(params_text(request, composition(), aggregation(), masking()).as_bytes())
}

fn params_text(request: &Request, composition: Composition,
               aggregation: Aggregation, masking: Masking) -> String
```

**不这样这条性质断言不出来**：那三件是 `const fn`，`Request` 上没有它们的旋钮，
用例换不动编译期常数。拆开之后用例换得动，生产路径仍只有一个调用点。

这条缝有一处代价，与 **Q306** 同型：穷尽解构管得住「加了字段」，管不住
「有人在调用点传了别的一份进来」。**没有记下来放过去，用一句断言钉住了**——
六字段那条用例的 baseline 取的就是生产路径那一串：

```rust
assert_eq!(params_hash(&request()), baseline, "生产路径喂给哈希的不是本次判据在用的那三件");
```

Q306 那一处（`score` 多收的位深）今天什么都没有；这一处有。

### 三、用例：一条新增，钉六个字段

`metadata::tests::every_metric_parameter_changes_the_hash`。改动一律**相对当前取值**
（翻倍、加一、乘 0.9），**一个当前的数都不写死**：标定把哪一个换掉，这一条都不必跟着改
（与 `Aggregation` 的 K、与 `01` 那条掩蔽用例同一条规矩）。头一版把上分位写成 `= 0.95`，
与紧挨着的那句自家注释自相矛盾——**standards 轴逮住的**。

它**不在 `tests/`**，而票面第三条点的是那里——集成用例只看得见公开 API，换不动 `const fn`。
两条备选路各自的毛病记在 **Q457**。端到端那一半由既有用例接住：跳过与否比的是含 `params`
的 `Fingerprint`，`a_changed_parameter_redoes_the_volume` 十项钉着，链条闭合。

### 四、ADR 0002《后果》两条

《全库重跑》那一条的后半段**订正**——它写着「幂等**拦不住它**……**地板不在里面**」，
本票落地后那句话是假的。按当前成立的事实重写，并**不再复述整份清单**
（清单的唯一出处是 `metadata::params_hash` 的文档，ADR 只说判据那三件进来了，`CLAUDE.md`
《文档写作》第 4 条）。

新增一条《把判据参数收进哈希，这一步本身让全库旧输出过期一次》：哈希多了输入，
同一套参数算出来的换了一串，判定一个没变而盘上每一页的记录都对不上。
**这一次全库重跑是本票造成的**，不写下来将来没人知道它从哪来。

**Q307 至此补上。**那一条记的正是这个缺口，当时走的是推荐①「靠 `04` 动阈值把哈希换掉」；
`04` 眼下挂起，而本票把机制本身补齐了——`04` 走不走，安全网都在。

### 五、没有动的

- **判据取值一格未动**：`GRAIN_RATIO`、`MASKING_FLOOR`、`MASKING_KNEE`、`TILE`、
  `UPPER_QUANTILE`、`TAIL_TILES`、阈值，七个数一个没碰。`src/metric.rs` **一个字未动**
  （`01` 的落地人答过「`03` 不必再碰 `metric.rs`」——**答对了**）。
- **`tests/` 一个文件未动**，包括 `tests/golden-snapshot.txt`：判定与像素都没变，
  而那一趟本来就看不见 tEXt（**Q458**）。
- **`CONTEXT.md` 一个字未动**：《参数哈希》那一条写的是「收的是**会改变输出**的每一项」，
  不列清单——本票让它更成立，没让它过期。列清单会给它开第二处出处。
- **报告与会话一行未动**：哈希不上屏。`01` 记的 Q439（前提那一张只剩一行空白）说
  「`03` 动哈希」时会撞到它——**没有撞到**，抬头一行没加。
- **`.scratch/grain-floor-absolute/spec.md` 与其余三张票、`docs/measurements.md`**，一个字未动。

### 六、停车场

**Q457–Q461**，五条：

| | 一句话 | 推荐 |
|---|---|---|
| Q457 | 那条用例落在 `src/metadata.rs`，票面点的是 `tests/`——集成用例换不动编译期常数 | ① 照落地这一版 |
| Q458 | 票面说黄金快照的哈希会变，而那一趟按 `--no-metadata` 跑，快照里没有哈希 | ① 记下，快照不动 |
| Q459 | ADR 那句被 `tone-alignment/spec.md` 引作先例，本票把它改成了反面 | ① 等那张票落地时顺手改 |
| Q460 | 行名取自字段名，`04` 换掉 `grain_ratio` 时全库会再过期一次 | ① 照旧，两趟各自过期 |
| Q461 | 票面要「加字段哈希自动跟上」，两条真能自动的路（`Debug` / `serde`）都没走 | ① 照落地这一版（穷尽解构） |

**Q307 记的缺口本票补上了**，但它自己那一行**没动**——那个文件的规矩是只追加、不改别人的行，
了结与否归拍板的人。

**Q438 那道缝本票关不上，而且它比看上去重要一格。**哈希收的是 `aggregation()` 的返回值，
真正在算的 `tiles()` 与 `aggregate()` 却直读常数。今天两边同源，但没有东西逼它们同步——
有人改了 `TAIL_TILES` 而 `aggregation()` 忘了跟上（或反过来），**哈希会忠实地记下一份
与判据实际不符的聚合参数**：旧输出照旧过期重做，可记录上那一串说的不是这一趟真用的数。
本票只让哈希多收几个输入，收口不在它的地界（**spec 轴点出来的残留**）。

### 七、code-review 逮到四处

spec 轴逐条核过硬约束：判据七个数、`04` 号票、spec 与其余三张票，**一格未破**；
三处点名判断（穷尽解构、用例位置、黄金快照）都判站得住。它逮到的是**理由不完整**那一处：

| | 哪一处 | 为什么是真问题 |
|---|---|---|
| spec 轴 | 「为什么不是 `Debug`」只排除了 `Debug` | 这个仓库有 `serde` 的 derive 现成——字段名同样写死、又**真能**自动跟上。不提它，那段理由就是拿一条既有不变量把票面挡回去。已补进 **Q461**，两条路各自的毛病与翻案代价都摆出来 |
| spec 轴 | 新用例只证「传进去的变了哈希就变」，不证「传进去的就是判据实际在算的那套」 | Q438 那道缝在哈希这一侧的后果，比它在报告那一侧的更重：哈希会**忠实地**记下一份与判据不符的聚合参数。已写进《六》 |
| standards 轴 | 用例里 `aggregation.quantile = 0.95` 写死了一个绝对值 | 与紧挨着的自家注释「一律相对当前取值、一个当前的数都不写死」自相矛盾；`UPPER_QUANTILE` 若标定成 0.95，这条当场假红。改成 `*= 0.9` |
| standards 轴 | 论证抄了两遍：《一》与 `params_hash` 文档、《三》与 Q457、`params_text` 行内注释与那份文档 | `CLAUDE.md`《文档写作》第 4 条要的是每条结论只有一处权威位置，其余引小节名。四处都改成指路 |

standards 轴还点出票面那两条**被改写成落地后的说法再打勾**，验收条因此不可证伪。
已把三条复选框恢复成票面原文，偏差另起一行写在它下面——原文是被交付的要求，
它不该被落地结果覆盖掉。

**这四处改完，先前那一趟绿不再说明这一版**，下面《数》记的是重跑那一趟。

### 数

`cargo xtask gate`（三条各用各的 target 目录），跑的是 **code-review 改完之后那一版**：

- **闸门 1 · 默认构建**（目录 `target`）`cargo test`
  合计 **879 通过 0 失败**；lib 243 / bin 349
  末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- **闸门 2 · 甩掉终端库**（目录 `target/gate/no-default-features`）`cargo test --no-default-features`
  合计 **748 通过 0 失败**；lib 243 / bin 218
  末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- **闸门 3 · 开着量具**（目录 `target/gate/profiling`）`cargo check --features profiling`
  末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 30.05s`（干净）

`cargo xtask polish` 四条全绿；`cargo doc` **15 条告警**，与既有基线同数——一条没多。

**用例净增一条**：`src/metadata.rs` 一条（lib 242 → 243）。bin 两侧一格未动
（349 / 218，与 `01` 落地那一趟逐个相同）——本票一个集成用例都没加。

**黄金那一条在闸门 1 与闸门 2 里各原样通过一次**
（`golden::the_fixed_fixtures_still_decide_the_same_way`），**没有跑过
`TONEFIT_ACCEPT_GOLDEN=1`**：判定与像素都没变，快照逐字节不变（**Q458**）。
