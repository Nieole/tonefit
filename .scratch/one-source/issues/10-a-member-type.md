# 10 — `(相对路径, 第几张, 共几张)` 捆成成员类型

**What to build:** 三个同型参数在四处同行旅行：算输出名、来路、幂等比对、认写出的那一族。捆成一个类型，四处都收它——
「这几处必须拿同一组值」从此由类型说，不由文档说。行为一格不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 四处都收成员类型；代码里不再有那三个参数同行
- [x] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** `(相对路径, 第几张, 共几张)` 捆成一个类型，四处（连同样张那一处）都只收它（收停车场 Q494）。行为一格不变。

1. **类型**（`src/metadata.rs`）：`MemberPart<'a>`，借着源成员的相对路径、`Copy`，紧挨 `Origin`。构造两种：`new(relative, ordinal, count)` 造一张，
   `family(relative, count)` 按阅读顺序给一族的每一张（Q1308）。放在 `metadata` 而不是 `pipeline`：四处里两处在这里，`pipeline` 本来就引它（Q1307）。
2. **四处都收它**：
   - 算输出名：`pipeline::output_name(part)`；`output_names(relative, count)` 改由 `family` 推出。
   - 来路：`Origin::new(relative, ordinal, count)` → `Origin::of(part)`（与 `PageSource::of` 同一个说法）。
   - 幂等比对：`PageRecord::matches(fingerprint, index, part)`；`compare_with_the_prior_output` 拿 `family(relative, family.len())` 与读回来的那一族逐张配。
   - 认写出的那一族：`PageRecord::is_the_page(part)`；`written_family` 一对一那一支造一张、切开那一支由头一张说出的张数造一族，头一张与余下几张出自同一个 `family`；
     `what_the_head_says` 探的两个名字照旧先一对一、再切开那一族的头一张。
   - 去处与来路一同算的 `Placement::new(part, records, page)`；分析环节切出几块就 `family(relative, pieces.len())` 几张、逐块配上，坏页那一张 `new(relative, 0, OUTPUTS_PER_FAILED_PAGE)`。
   - **样张**（`src/proof.rs`，票面没列）：`proof::write` 照转换那一趟同一副写法，`family(name, pieces.len())` 与编好的那几叠逐叠配，名字仍出自 `output_name`。
3. **词汇表**：`CONTEXT.md`《输出》表在《来路》之后添《成员的一张 (MemberPart)》——新概念，当场加；已有词条一字没动。

**没动的**：`PageSource` 不改名（归 `one-source/13`）；源页序号没并进类型（`matches`、`Placement::new` 仍各带一个，Q1309）；没添用例（spec《搬家与改名》：全部用例照旧绿就是行为没变的证明，约束由编译器守）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 40ff2f1`。Spec 轴：四处与样张那一处都收了类型、三样同行的只剩构造本身；`zip` 两边长度、`written_family` 三支、`what_the_head_says` 的次序逐支对过，没有发现行为变化。缺的只是本节的《数》与勾选框。

**收下的**（Standards）：

- 切两半那条用例里路径与「2」各写两遍（`docs/agents/testing.md`《用例点名取值，不借默认值》：同一个数只有一个出处）——改成只造一次 `family`，名字由 `output_name` 从它推出。
- `written_family` 头一张用 `new`、余下几张用 `family(..).skip(1)`，同一族两种造法——改成同一个 `family`，头一张取它的第一张。
- `Placement` 的文档与 `MemberPart` 的文档各写一遍「两处对不上……页几何批 04 号票」（单一出处）——`Placement` 那边改成指过去。
- 新词条重抄了《来路》那三样（单一出处）——改成「《来路》说的那一张，在写进记录之前的那一份」。
- Q1308 的 Recommend 原写「② 只做到签名那一层」，读着像稻草人——改成说清两条路各自的代价。

**驳回的**：

- `output_name` 只读 `part` 的三样（Feature Envy），建议挪成 `MemberPart` 的方法：输出成员名的规矩归 `pipeline`（`MORE_THAN_ONE`、撞名那一道都在那里），票面也点名「算输出名」那一处收这个类型，而不是变成它。
- 探头那两个名字在 `what_the_head_says` 与 `written_family` 各拼一遍：本票之前就是两处，`what_the_head_says` 的文档点名「照 `written_family` 探的那两个」；本票不添不减。
- `MemberPart` 与 `Piece` 读着易混（Mysterious Name）：`Piece` 是切出来那一块的几何，`MemberPart` 是它那一张的身份，名字之争记在 Q1307。

### 停车场

本票用了 Q1307–Q1309：

- **Q1307**：类型叫 `MemberPart`、词条《成员的一张》，不叫 `Member`；住在 `metadata.rs`；`Origin::new` 改叫 `Origin::of`。
- **Q1308**：添了 `MemberPart::family`，调用处 `zip` 而不是 `enumerate` 出序号再拼。
- **Q1309**：源页序号没并进类型。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-10.gate1.log`、`os-10.gate2.log`、`os-10.gate3.log`、`os-10.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**
条数与基线相同：本票没添用例。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1141 通过 1 失败**（1 ignored）；lib 253 / bin 476；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 85.60s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **1000 通过 1 失败**；lib 253 / bin 335；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 55.71s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 4.52s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 154.95 秒、闸门 2 上 152.86 秒），快照没动。
**设计快照**：会话里比设计快照的那几景在闸门 1 的 bin 476 条里，全绿；本票没碰会话与设计稿。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q494 — `(相对路径, 第几张, 共几张)` 这三个参数在四处同行旅行，本票把其中一处扩成了四参

- **From:** 票 `two-pass-rework/07`（`/code-review` Standards 轴报的基线坏味道）
- **Kind:** 既有坏味道（Data Clumps），本票把它**稍微加重了一点**
- **Where:** `src/lib.rs`：`output_name(relative, ordinal, count)`、
  `Origin::new(relative, ordinal, count)`、`Placement::new(relative, ordinal, count, …)`，
  外加 `written_family` 里那个 `matched(record, ordinal, count)` 闭包与
  `PageRecord::matches(fingerprint, relative, ordinal, count)`。
  **它是一个想出生的类型**：`Placement` 的文档自己就写着这三个是同一组
  （「两者由同一组 (源成员, 第几张, 共几张) 算出，因此一同算出、一同传下去」）。
  本票给 `Placement::new` 加了第四个参数，那一处从三参到四参。
- **Why it did not block:** 三个参数同型同序、`Placement` 那句文档已经把「必须一同传」
  写死，传错一个的失败模式是幂等去找的名字与真写出的名字错开——而那正是
  `ensure_one_member_per_output` 与 `written_family` 两处已经在拦的东西。
  本票只多加一个**不同型**的参数（`Option<&Fingerprint>`），没有把同型参数的队列拉长，
  传反的风险一格没涨。
- **What this ticket actually did:** 加了第四个参数，一个字没重构。
- **Options:** ① 照现在；② 把那三个捆成一个小类型（形如 `Member { relative, ordinal, count }`），
  四处一起改；③ 只在 `Placement::new` 上收，别处不动。
- **Recommend:** ②，但**不在本票**——它要动 `output_name`、`Origin::new`、
  `PageRecord::matches` 与 `written_family` 四处，而 `PageRecord::matches` 是幂等那条路
  上的判据，动它要连 `tests/idempotency.rs` 一起重读。③ 最坏：收一半等于让同一组值
  在库里有两种写法。② 的收益不只是少几个参数——`Origin::new` 与 `output_name` 今天
  **必须**拿同一组值调用才正确，捆起来之后那件事由类型说，不由文档说。
- **Whose call:** 排票的人（值不值得为它开一张纯重构的票）
- **处置：** **`one-source/10` 落地（2026-10-02）：照②了结。**三样捆成 `metadata::MemberPart`（词条《成员的一张》；不叫 `Member`，那是 `source::Member`，Q1307），`output_name`、`Origin::of`、`PageRecord::matches`／`is_the_page`、`Placement::new` 与样张那一处都只收它；一族逐张由 `MemberPart::family` 给（Q1308）。
