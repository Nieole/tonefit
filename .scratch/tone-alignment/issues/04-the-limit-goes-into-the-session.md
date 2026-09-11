# 04: 会话的口味层调得动

**What to build:** 在会话里跑的用户按 `↑↓` 走到「纸白对齐上限」那一行、`⏎` 进去改成 0（关掉）、再按一次试算，就看得出这一趟少对齐了多少页——不必退出会话去敲命令行。改完存进预设也是同一条路。

它与「拆分阈值」同型（一个数值项，取值栏里编辑），**不需要新机制**；这一张的大头是口味层从十一项变十二项之后，那几幅写死的版面用例要跟着重画。

**Blocked by:** 01, 03

**Status:** resolved

- [x] 口味层多出「纸白对齐上限」一项，位置与其余口味项同列
- [x] 走 `焦点` 已有的那一套：`左栏` → `编辑一行` → `取值栏`，进出键与「拆分阈值」一致，不新造键
- [x] 三层**只读那几个阶段**照旧只读（跑着、等答话时一个改动键都不派），与焦点在哪无关
- [x] 取值改完之后**下一趟**生效，趟与趟之间改得动（会话的定义就是这个）
- [x] 「存成预设」把这一项一并存进去（与 03 同一份取值写法）
- [x] `让位` 那一套照旧管它：宽度不够时左栏先缩、缩不下整个让掉；高度不够时按既有次序让
- [x] `视口` 与滚动条按多出来的这一行重算——滚动量是算出来的、不是记着的
- [x] 那几幅写死的版面用例（`src/session/draw/` 下的）跟着重画，**差异审查后接受**
- [x] 闸门三条绿（`cargo xtask gate`）

## 落地记录

**会话左栏的口味层多出「纸白对齐上限」一行，与拆分阈值同型：走到那一行、`⏎` 进编辑、打一个数、`⏎` 收下，
下一趟就按它跑；`0` 是关、是一个说了的值，存成预设时 `white-align-limit = 0` 原样写出去。**（2026-09-11，基底 `619c547`）

`src/session/state.rs`：`Field::WhiteAlignLimit` 照 `Field::SplitThreshold` 的每一处各加一格——`TASTE_FIELDS`
（`[Field; 12]`，摆在滤波器之后、位深之前，与管线里的位置、与 `TasteLayer` 的字段同序）、`layer`（口味层）、
`label`（「纸白对齐上限」，`CONTEXT.md` 词条名）、`shape`（`Text`）、`turn_field` 那张转不动的名单、`unsaid`、`typed`、
`take`、`shown`（说了印数，没说印「默认（4）」——那个 4 从 `WhiteAlignLimit::default` 的 `Display` 来，不复述）。
把字变成数的只有会话这一处（另两处入口各有 clap 与 TOML 替它做），`parse_white_align_limit` 收 `u8`、
报错一句中文，256 与一串字挡在编辑态、用户打的东西不丢（去处见 **Q689**）。`Session::request` 改读
`taste.white_align_limit()`，那句「口味层里还没有这一项」的注释删了。一条新用例
`the_white_align_limit_is_edited_in_the_session_and_takes_effect_next_run` 五段：没说→改 0（`Request` 与预设都照它走）
→越界→改 2 与清空→跑着与等答话只读、收场后改得动。

`src/preset.rs`：`every_field` 那一格 `None` → `Some(WhiteAlignLimit::OFF)`（选 0 的理由见 **Q690**），
`state.rs` 那条「屏上两层 == 盘上两层」的断言从此盖住十二项；替它顶着的 `the_white_align_limit_round_trips_through_the_file`
退场。

`src/session/draw/`：**十三幅写死的版面用例逐幅看过实际画出来的屏再收**——`config.rs` 五幅、`yielding.rs` 三幅
（80×24 的空闲／跑着／摊着）、`report.rs` 五幅（焦点四副与最窄那一档的卷表）。每一幅的差异都只是「多一行 +
视口与滚动条重算」：原高恰好装满的两幅末尾那一行空白没了；80×24 上光标在顶时**范围层的抬头滚出屏外**、
末一行是层与层之间的空行——那是既有让位次序算出来的样子，本票不重排；`the_unfolded_panels` 与
`the_unfolded_models_under_one_panel` 两幅钉的是「整层摆出来看」，格子各抬高一行，`the_config_column_that_does_not_fit`
保持 24 行、多让一行（三幅各走哪条路见 **Q691**）。屏上那一行取 0 时只印 `0`，报告那一行说的是「0 级（没开）」，
两处措辞对齐要进 tpr/02 的文件，记 **Q692**。

`CONTEXT.md`：《口味层》清单加「纸白对齐上限」一项，《会话》那句引文「十一项」改「十二项」——两处各一个词，
别的词条一个字没动。**没进的文件**：`src/session/columns.rs`、`src/session/draw/table.rs`、`src/render/`、`src/lib.rs`、
`src/metadata.rs`、`src/white.rs`。

**给协调人的一句**：`report.rs` 那五幅整屏快照的右半边是卷表——tpr/02 若改了卷表的列，合并时同一批常量两边都动过，
冲突在这几个 `const` 上，左半边取本票、右半边取 tpr/02 即可。

### 数

| | 命令 | 结果 | 对账 |
|---|---|---|---|
| 闸门 1 | `cargo test` | `946 通过 0 失败`（lib 234 / bin 394） | 基底 `619c547`：ta/03 在 `6dbcd74` 上记的 945／393，其间 nfl/05（`92fe5da`）用例 +2 −1，即 946／394；本票 +1（`state.rs`）−1（`preset.rs`），**净 0** |
| 闸门 2 | `cargo test --no-default-features` | `801 通过 0 失败`（lib 234 / bin 249） | 同上：799／247 + nfl/05 的 +2 = 801／249；本票**净 0**（那两条用例都在 `tui` 外面，两条闸门同一个账） |
| 闸门 3 | `cargo check --features profiling` | 绿，末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.57s` | — |

闸门 1、2 的末行都是 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
（doc-tests 那一趟）。`cargo xtask gate` 印的《数》：「全绿。」，`EXIT=0`。协调人预告的 `tests/stop.rs` 那条抖动这一趟没抖。

`cargo xtask polish` 四条绿：`cargo fmt --check` 干净，`cargo clippy --all-targets` 与 `--no-default-features` 各 0 条，
`cargo doc --no-deps` **15 条，与基线相同**（`warning: \`tonefit\` (lib doc) generated 15 warnings`），`EXIT=0`。

### review 收下的

`/code-review` 两轴并行（只读，没碰工作区，一条 cargo 都没跑），worktree 路径与基底 `619c547` 一起交出去的。
Spec 轴 0 缺项、0 越界，提醒两条；Standards 轴 1 硬 + 4 判断。**收 5 条**：

- **硬「停车场索引指向票里不存在的《停车场结转》」**——真的（review 跑的时候那一节还没写），补上，就是下面那一节。
- **Spec 提醒「等答话只由 `Stage::read_only` 间接盖住」**——新用例第五段加一句 `at_the_decision_point(true)` 之后回车仍 `Ignored`。
- **判断「`state.rs` 那句注释仍写 `TASTE_FIELDS.len() == 11`」**——改 12。
- **判断「`typed` 走 `levels()`、`shown` 走 `Display`，同一个数两条路」**——`typed` 也走 `Display`。
- **判断「`config.rs` 那行文档 177 字节」**——折成三行。

**没收的**：「新用例五段合一，邻近用例一事一名」——第五段（跑着只读、收场后改得动）正是用例名里
「takes effect next run」那半句，拆开反而让名字说一半；`the_two_dimensions_move_one_at_a_time` 盖的是通用性质，
这里问的是这一行。

### 停车场结转

**Q649、Q650 了结**——两条都是 ta/03 明写交给本票的活，原文按《已了结》那一节的规矩挪到这里。
Q650 的 Whose call 是「拍板的人（词汇表）」：票面那句「口味层从十一项变十二项」就是那一票，
本票据它只加一项、订一个数，不改写别的词条。

> ### Q649 — 口味层有了纸白对齐上限，会话那一侧仍按十一项：`every_field` 那一格故意留白，`Session::request` 恒取默认
>
> - **From:** 票 `tone-alignment/03`
> - **Kind:** 两张票之间的交接（本票加了字段，屏上那一行归 `tone-alignment/04`）
> - **Where:** `src/preset.rs` 的 `every_field`（`white_align_limit: None`，唯一一格「没说」）与
>   `the_white_align_limit_round_trips_through_the_file`（替那一格补的往返用例）；`src/session/state.rs` 的
>   `TASTE_FIELDS`（仍是 11 项）、`the_two_layers_on_screen_are_the_two_layers_a_preset_stores`
>   （断口味层键数 == `TASTE_FIELDS.len()`）、`Session::request`（`white_align_limit: WhiteAlignLimit::default()`，
>   注释仍写「口味层里还没有这一项」）、`Session::took`／`Session::preset`（整层搬运，这一格跟着走）
> - **Why it did not block:** 票面明写不动 `src/session/`（nfl/05 在画法层，ta/04 在口味层那一行）。`every_field` 写成
>   `Some` 会让 `state.rs` 那条断言当场红（盘上 12 键、屏上 11 行）——那条红是 04 号票的，不该由本票的闸门替它先红；
>   写成 `None` 三条闸门照旧绿，而本票自己的往返另有一条用例钉着。行为上有一处**临时不一致**：会话里套一份写着
>   `white-align-limit = 2` 的预设，屏上看不见这一项、跑的时候按默认的 4 走，而按下存把 `2` 原样写回盘上——
>   用户从会话里存出去的东西比他屏上看见的多一项。这一档只到 04 落地为止，命令行那一路不受影响。
> - **What this ticket actually did:** `TasteLayer` 加 `white_align_limit: Option<WhiteAlignLimit>` 与取值器，
>   `OnDiskTaste` 加 `white_align_limit: Option<u8>`，`resolve`／`From<&Preset>` 两处搬运；`every_field` 那一格留 `None`，
>   注释指到本条；`src/session/` 一个字没动。
> - **Options:** ① 04 号票落地时：`TASTE_FIELDS` 加 `Field::WhiteAlignLimit`、`Session::request` 改读 `taste.white_align_limit()`、
>   `every_field` 那一格改 `Some`、单独那条往返用例退场、`state.rs` 那句过时注释一并删；② 本票越界改 `state.rs` 那三处——
>   与 nfl/05、ta/04 撞同一个文件；③ `every_field` 写 `Some`，让 `state.rs` 那条断言红着交给 04——闸门不绿，票面不许。
> - **Recommend:** ①，四步都在 04 的票面范围内（「口味层从十一项变十二项」）。
> - **Whose call:** 协调人（派活时捎给 ta/04）
> - **处置：** 由 `tone-alignment/04` 按 Recommend ① 收掉——四步全做：`TASTE_FIELDS` 加 `Field::WhiteAlignLimit`、
>   `Session::request` 改读 `taste.white_align_limit()`、`every_field` 那一格改 `Some(WhiteAlignLimit::OFF)`（写哪个值见 **Q690**）、
>   `the_white_align_limit_round_trips_through_the_file` 退场，`state.rs` 那句过时注释一并删。

> ### Q650 — `CONTEXT.md` 的《口味层》词条列了十一项，没有纸白对齐上限；《纸白对齐上限》词条自己却说它属于口味层
>
> - **From:** 票 `tone-alignment/03`
> - **Kind:** 词汇表与实现对不上（两条词条互相打架）
> - **Where:** `CONTEXT.md` 的《口味层》（「适配方式、裁边、……、缓存预算、读取策略」十一项）；《会话》那一节的引文
>   「而不是把十一项全按今天的默认写死」；《纸白对齐上限》词条（「它属于**口味层**」）；`src/preset.rs` 的 `TasteLayer`
>   文档已写成十二项
> - **Why it did not block:** 领域决定早就下了——《纸白对齐上限》词条与 spec 第 8 条都说它在口味层，缺的只是《口味层》那张
>   清单没跟上。`CLAUDE.md` 的规矩是改写已有词条要先拍板、撞见对不上记在这里不顺手改；上一批词条对齐是单独一张票
>   （p4/28）做的，本票照那个惯例。
> - **What this ticket actually did:** `CONTEXT.md` 一个字没动；`TasteLayer` 的文档写成十二项并指到《纸白对齐上限》词条。
> - **Options:** ① 《口味层》清单加「纸白对齐上限」一项、引文那句「十一项」改「十二项」——两处各一个词；
>   ② 等 04 号票把会话也改到十二项之后一并改，免得中间态里词条说十二、屏上十一；③ 不改，靠《纸白对齐上限》词条自己那句。
> - **Recommend:** ②——03 与 04 合完之后清单与屏上、盘上三处同时对齐，一次改齐；①是兜底。③不行：两条词条互相打架
>   比清单短一项更坏。
> - **Whose call:** 拍板的人（词汇表）
> - **处置：** 由 `tone-alignment/04` 收掉——03 与 04 合完那一趟就是本票（Recommend ②）：《口味层》清单加「纸白对齐上限」
>   一项、《会话》那句引文「十一项」改「十二项」（Option ① 的两处，各一个词），别的词条一个字没动。

本票新记 **Q689–Q692** 四条（块是 Q689–Q696，留下 Q693–Q696 的空洞），都在《待处理》。
