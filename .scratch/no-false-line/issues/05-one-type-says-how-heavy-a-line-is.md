# 05: 语义色收成一个类型

**What to build:** 「这一句有多重」在屏上有**三个类型**说得出：卷表那套记号、
[语义色](../../../CONTEXT.md)、以及屏底那一句自己挂的那三档。

屏底那三档之所以存在，是因为语义色那个类型住在**画法层**，而画法整个在 `tui` 特性后面；
说出那句话的状态机在特性**前面**（闸门 2 那一趟照编不误），因此挂不上它。

而词汇表早就把话说清了：`CONTEXT.md` 把《语义色》放在**《会话》**那一节，
还写着「**只在会话里**」——它本来就是会话层的词，住在画法层是放错了地方。
**搬到 `tui` 特性前面**，屏底那三档并进它。

**它不带记号。** 《语义色》那句「颜色不是唯一载体——每一处上色的地方旁边都另有一个字
或一个行首记号」是**对每一处的要求**，不是这个类型的一个属性：语义色在**页**那一级是
**多对一**（「注意」一档收着隔离 · 部分救回 · 宽溢出 · 兜底上界 · 几何门不成立 · 特例页
六样），一个方法答不出那六样各配什么记号。卷表那四对四是**巧合**，不是构造。

那句话改成落成**一条闸门**：每一处上色的地方都配了一个字或一个行首记号。

卷表那套记号原样留着，它到语义色那个映射也原样留着——**方向只有一个**。

收停车场的 **Q198**。

**Blocked by:** 01 — 夹具喂全事件流那一半（同一批快照）

**Status:** resolved

- [x] 语义色那个类型在 `tui` 特性**前面**，关掉终端库那一趟照编不误
- [x] 屏底那一句原先挂的三档**没了**，并进语义色
- [x] 语义色**不带记号**：没有一个「这一档配哪个记号」的方法
- [x] `paint.rs` 里那条「每一处上色的地方都配了一个字或一个行首记号」的闸门**扩到屏底那一句**——今天它只照主区那一份夹具
- [x] **颜色一格不变**：既有那条「同一屏上色与不上色文字逐格相同」的恒等式用例照旧绿；
      快照里的颜色逐格核对过
- [x] 卷表那套记号与它到语义色的映射**一个字没动**
- [x] `CONTEXT.md`《语义色》已由 `p4-parking-lot/28` 按本票落地后的样子重写（屏底三档已排进四档、行首记号已列）——本票核一遍逐句成立，**不另改**
- [x] 三条闸门全绿

## 落地记录

基底 `67209d7`。**颜色一格没变、字一格没变**：`draw` 底下的成屏快照一张没重录，
`the_same_screen_reads_the_same_with_or_without_colour` 一字未动照旧绿。换的是**类型的家**与**谁说轻重**。

### 语义色搬到 `tui` 特性前面：新开 `session::tone`

`Tone` 四档连同 `Ord` 派生、以及「四种从轻到重」那条用例，从 `draw::paint` 搬进新模块
`src/session/tone.rs`；`session.rs` 的 `mod` 列表与《终端库在哪一半》改成**七个**特性外模块，
`docs/agents/gate.md` 闸门 2 那一格的名单跟着加一个词。没并进 `state.rs`：它不是状态，画法六块个个要它，
`CONTEXT.md` 把《语义色》当《会话》一节自己的一个词、与《视口》《砍列》同级——另一条路与代价在 **Q657**。

`paint.rs` 留下的是**样子**：`Tone::style()` 方法变成自由函数 `paint::style(tone)`——类型在特性前面、
颜色是终端库的东西只有画法认得，方法跨不过去；扩展 trait 要多起一个名字（`draw::Styled` 已占着）而调用点
只有 `Painted::line`／`folded` 与 `config.rs` 一处，不值。「本仓库唯一写得出颜色名的地方」仍只指它。

### 屏底那三档没了

`state.rs` 的 `NoticeKind`（没做成／先问一句／做成了）删掉，`Notice { said, tone: Tone }`。
三个出口 `Notice::refused`／`asked`／`done` 保留，各挂一档（出事／注意／平常）；`kind()` 改 `tone()`。
`footer::marked` 不再折一次，读 `notice.tone()` 上色，只配行首那个记号。
`state.rs` 只动了 `Notice` 那一块、`complain`／`notice` 两句文档、一行 `use`，加一条特性外用例
（`the_bottom_line_says_how_heavy_it_is_in_the_one_tone_type`，`--no-default-features` 那一趟跑得到）；
tpr/11 在改的 `Field` 那一带一行没碰。`overview.rs`／`live.rs` 只有 `use` 与 doc 链接。

### 它不带记号；「配了记号」是每一处的闸门

`Tone` 上没有任何返回字符的方法。屏底的记号由 `footer::marked` 自己那个 `match` 配——四档穷举、不留 `_`；
`Muted` 那一格状态机今天没有一个出口挂它，选了 `-`，理由与另两条路在 **Q658**。
这张表是**局部的** Tone→记号：状态机将来多一个出口挂在既有的一档上，这里不必改、自然得到那一档的记号——
记号跟的是轻重，不是哪一个出口；守着它的是底下那条闸门。卷表 `Mark::glyph`／`Mark::tone` 逐字未动，方向仍是记号→语义。

`paint.rs` 的 `every_painted_block_carries_a_word_or_a_mark_of_its_own` 从只走主区一份夹具，
扩成走**主区加屏底**：状态机三种里上色的两种（`complain` 红、`ask_before_erasing` 黄）各画一屏 `shell`，
先断言那一句真上了色，再逐段问载体。**咬得住**：把 footer 的 `✗` 换成空格试过一次，当场红
（「这一段头一行上了色却没有一个字接得住：  先挑型号：跑不起来」），复原后绿。

### 顺手订正的假话

`paint.rs` 与 `footer.rs` 各有一句「`CONTEXT.md` 的《语义色》那一列还没收……改它归 28 号票，Q163 记着」——
28 号票已落地（`390c26c`），那两句已是假话，重写成指新家的话。`draw.rs`／`probe.rs`／`directories.rs` 三处
doc 链接从 `super::paint::Tone` 改指 `crate::session::tone::Tone`。评审另抓到一句我从票面抄来的不准确的话
（「语义色在页那一级多对一……六样」——隔离是卷级记号，页级只有五样），代码文档改成引词汇表那一档、不抄名单；
票面《What to build》里那句原样留着，它是当时的判据不是词条。

### `CONTEXT.md`《语义色》逐句核过，一字未改

四种与各自的名单（含「屏底那一句报的『做成了』／问的『再按一次』／报的『没做成』」）与
`Notice::done`／`asked`／`refused` 三个出口逐条对得上；「报告末尾那几小结按小结分属三档」是 `report::tail_row`；
「颜色不是唯一载体……行首记号（三张表那几行行首那一个，以及屏底那一句行首那一个）」——三张表各有 `Mark::glyph`、
屏底是 `footer::marked`，闸门此刻问到主区与屏底；「只用 16 色里的基本色，不定背景色」「认 `NO_COLOR`……一处生效」
「光标反白、层抬头加粗、预设栏路径压暗不在这四种里」「只在会话里」——都在 `paint.rs`，一句没变。

### 数

三条闸门在评审之后的**最终状态**上跑的（`cargo xtask gate`，随后 `cargo xtask polish`，
同一 detached 脚本，两份日志各自 `EXIT=0`）：

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **949 通过 0 失败**；lib **249** / bin **383** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **807 通过 0 失败**；lib **249** / bin **241** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.11s`` | 干净，一条告警都没有 |

**闸门 2 涨 2**（bin 239 → 241）：`state.rs` 那条「屏底那一句自己说得出它有多重」与 `tone.rs` 那条
「四种从轻到重」——后者从 `paint.rs` 搬来，从前只有闸门 1 跑得到。**闸门 1 本票净涨 1**（+2 −1，
搬走那条不再在 `paint` 里数）；bin 378 → 383 里其余的是基底 `67209d7` 收进的 tpr/01 带的，
基底那一趟本票没有单跑。lib 249 两条都没动。**闸门 2 一条告警都没有**：`Notice::tone()` 与 `Tone::Muted`
不挂 `allow(dead_code)` 是对的（见《评审》）。

**闸门之外那一遍**（`cargo xtask polish`）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警
（头一趟默认那遍多出 1 条 `type_complexity`，出在扩闸门那条用例里的 `[(fn(&mut Session), &str); 2]`，
改成一个闭包逐句调两次之后重跑，回到零）；`cargo doc --no-deps` 仍是 **15 条告警**
（``warning: `tonefit` (lib doc) generated 15 warnings``），**一条没多**。

### 停车场结转

**Q198 了结**：`Tone` 搬到 `tui` 特性前面，`NoticeKind` 删掉，屏底那一句直接挂语义色——
「这一句有多重」只剩一个类型说得出。《已了结》索引表那一行原样留着。

**新记两条**（本票 id 块 Q657–Q664，用掉两个），都在《待处理》：

- **Q657** — 语义色的家：新开 `session/tone.rs`（vs 并进 `state.rs` 紧挨 `Notice`，vs 放进 `columns.rs`）。推荐现状。
- **Q658** — 屏底那一句「不要紧」那一档配什么记号：穷举逼出来的一格，选了 `-`（vs `unreachable!`，vs 并进平常的 `✓`）。推荐现状。

### 评审

`/code-review` 跑过一遍，两轴并行（**只读，没碰工作区，一条 cargo 都没跑**），worktree 路径与基底
`67209d7` 一起交出去的。Standards 提 2 硬 + 4 判断，Spec 提 0 缺（收口除外）+ 1 提醒；**收 5 条**：

- **Standards 硬「六样／页那一级」是假话**——真的（隔离是卷级记号）。`tone.rs`／`paint.rs` 改成引词汇表那一档。
- **Standards 判断「单开模块的理由只在 Q657」**——`tone.rs` 模块文档补一段《自己一个模块，不并进 state》。
- **Standards 味道「不带记号的论证三处复述」**——`paint.rs`／`footer.rs` 收成引 `tone.rs` 那一节。
- **Standards 味道「`blocks_on` 读不出计数」**——改名 `painted_blocks_on`。
- **Spec 提醒「`marked` 是一张局部反向表，新出口静默得既有记号」**——是删 `NoticeKind` 的必然代价，
  写进 `marked` 的文档与本记录。

**没收的**：两轴都按 rustc 死码规则**推断** `Notice::tone()` 与 `Tone::Muted` 在闸门 2 上要挂
`allow(dead_code)`。实测不是：`cargo check --tests --no-default-features`（闸门 2 的 target 目录）零告警——
`state.rs` 那条新用例读 `tone()`、`tone.rs` 那条用例构造 `Muted`，用例是活根；而不带 `test` 的那一趟
整个 `session` 都不编。挂上去反而是句假话（reason 写「只有画法读得到」，而用例就在读）。闸门 2 的最后一行是判官。

