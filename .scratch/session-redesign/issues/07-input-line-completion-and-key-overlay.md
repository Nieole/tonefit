# 07 — 输入行、补全框与全部按键

**What to build:** 新界面上打字与看帮助（spec《输入行与路径》《按键表、屏底与覆盖层》；停车场 Q715–Q717 的结论）。

- **输入行**占屏底：提示词 · 缓冲 · 光标，右端 `Tab` `C-w` `⏎` `Esc`。`o` 添加处理路径，`i`／`⏎` 修改一条，
  输出目录那一行上修改输出目录；
- 输入行认 `~/` 开头的路径；逐层补全，候选多于一个时屏底上方弹出**补全框**（能滚、不把屏底撑高），`Tab` 轮到下一个；
  `C-w` 删一段；找不到的路径当场说；
- **全部按键**：`?` 掀开，按用途分组，**只列此刻阶段派得出的键**，同一个键按阶段写它此刻做的事（`q` 跑着时说不退出）；
  宽时两栏；`j`／`k` 滚、底边说看到第几行；`?`、`Esc`、`q` 关掉；底下整屏压暗；掀着时除了停止与答话别的键不派；
- 打字时 `F1` 掀开全部按键，关掉之后输入行、缓冲与补全框原样回来。

真会话仍进旧界面。

**Blocked by:** 06 — 新会话的骨架与开跑之前那一副

**Status:** resolved

- [x] 「添加路径」120×36、80×24 逐格相等
- [x] 序列：`o` → 补全框 → `Tab` 轮换 → `C-w` → 打一个找不到的路径；`i` 修改一条处理路径；改输出目录；走完与期望屏相等（`Tab` 轮换那两屏比的是候选与缓冲、不比屏——次序两边不同，Q790）
- [x] 序列：打字时 `F1` → `j`／`k` → `Esc` 回到输入行；还没开始时 `?`；走完与期望屏相等
- [x] 用例：全部按键与屏底出自同一张按键表（表里加一个键，两处都跟着出现）
- [x] 真会话仍进旧界面，旧用例照绿；设计快照未改（第一步那六条拍板之外）

## 落地记录

**第一步：拍板落地（提交 `32368c0`）。** 06 记的六条按用户 2026-09-15 的拍板改设计稿、重导、再改实现（ADR 0019 决定第 13 条的次序）：

| 条目 | 设计稿改在哪 | 实现那一半 | 重导变了哪几屏 |
|---|---|---|---|
| Q775 ① | `viewport(cursor, total, h)` 从光标算、不记着，`S.from` 整个拿掉；格子高过 8 行留一行余量 | 余量做成 `Viewport::with_margin`（新界面的卷列表、补全框走它），`new` 照旧给旧界面——补进 `new` 会让旧界面五条用例红，记 Q789 | `ended-C-f-C-b`、`running-search-n-N` |
| Q776 ① | `elide` 改成 `columns::elide` 的截法 | 不动 | 一格没变（没有一屏带省略） |
| Q777 ① | `drawTooSmall` 跑着与等待确认时写 `C-c` | `shell::small` 按阶段从表上取：`Quit` 派不出就落到 `Interrupt`（那一行补上短的一句「退出」） | `running.56x14`、`running-shrink` |
| Q778 ① | `stageHints` 配置视图不摆 `v` | 不动（表本来就只在卷列表上派 `v`） | `deciding-2` |
| Q779 ① | `startsCombo`：`g` 哪一块都待；`d` 在 `dd` 派得出的块上待（开跑之前的卷列表、预设栏），不问光标那一行；`]`／`[` 只在清点完之后的卷列表上待 | 不动 | 一格没变 |
| Q783 ① | `footerHints` 清点中只剩停止、`j/k` 与 `?`（连同 `F`） | 不动 | `survey.120x36`、`survey.80x24` |
| Q774 ① | 照旧 | 照旧 | — |

`git diff --stat tests/fixtures/design` 十四个文件（七屏各两张网格）；06 认领的 `fresh.*` 三份与 `fresh-*` 七串一格没变，`cargo test --lib session` 照绿。

**本票做了什么。** 新界面上打字与看帮助，三处接缝各一条红→绿：

- **画面**：`add.120x36`、`add.80x24` 逐格相等（`shell::tests::the_add_scene_matches_its_design_snapshot_wide_and_narrow`，连同 `assert_no_background`）。
  输入行占屏底（`shell::footer`：提示词 · 缓冲 · `▏`，右端那四件从表上取）；补全框贴左下角盖在卷列表上、不把屏底撑高
  （`shell::completions`）；覆盖层整屏压暗、全部按键那一张画在上面、右框线上滚动条（`shell::overlay`）。画布多两手：`dim_all`
  与 `scrollbar`（终端库的 widget 算滑块，交给它的「内容有多长」是起点能取几个值——这么交滑块的长度正是设计稿的式子；位置差在 `.5`
  那一格上，归 Q780）。宽字符的第二格带着第一格的样子：补全框的右框线落在卷列表一个汉字中间时露出来的那半格是暗灰的，与设计稿的 `_split` 同一条。
- **交互**：经 `terminal::input` 喂序列，本票认领的二十二串里做了十四串——`fresh-o` `fresh-o-Escape` `fresh-o-Tab-Tab-C-w` `fresh-o-missing`
  `fresh-o-added` `fresh-i` `fresh-k-i` `fresh-k-i-C-w-typed` `add-F1` `add-F1-j` `add-F1-j-k-Escape` `add-narrow-F1-j` `add-narrow-F1-j-k`
  `add-narrow-F1-j-k-Escape` `fresh-help` 逐格相等；`fresh-o-Tab`、`fresh-o-Tab-Tab` 只比候选几条、轮到哪一条、缓冲（候选的次序两边不同，Q790）。
  **留给 09 的**：`help-question` `help-j` `help-Escape` `help-narrow-j` `help-narrow-j-k` `help-narrow-G`——起点 `help` 是转换中那一副，底下的树归 08、
  `G` 在覆盖层上到底那一下也留在那儿（`cover::Scroll::Bottom` 已接，`terminal::input` 那一支认得）。
- **纯逻辑**（`tui` 外面，闸门 2 照编照测）：`session::typing`（输入行、补全项、`~/` 走 `Home::expand`、逐层补全走 `complete::level` 与 `name`——
  设计稿补的是头一个候选、不补公共前缀，`common_prefix` 没用上；`⏎` 问一次盘、找不到当场说）；`session::cover`（覆盖层、全部按键那一张 `Sheet`：
  从表上按组摆、同一句并成一行、一行不剩的组整组不出、宽时从中间劈成两栏、末尾接灰阶写法那一节——写法从 `BitDepth`／`Candidate` 的 `Display` 取；
  滚动收在摆得下的那一段里）；`cover::tests::the_sheet_and_the_footer_both_come_from_the_key_table`：表上每一行长的那一句在它派得出的每一档都在那一张上、
  短的那一句在它派得出的每一块上屏底摆得出来，反过来那一张上每一行都是表上的一行——加一行两处都跟着出现。

**在 06 的骨架上改了什么**（阻塞边）：`Views` 上加 `input`／`cover` 两格，`focus()` 先看它们、另出 `block()`（总览与行上顺口提的键问底下那一块，
输入行开着时照样写着）；`Deed` 加 `Typed(char)`（不在表上：焦点在输入行上、表派不出的字符落到它）与 `Erase`（`⌫` 那一行）；`Group::ALL`、
`keymap::spelt`；`Interrupt` 那一行补上短的一句；`Kind` 加 `Caption`／`Key`／`Directory`（Q795）；`Views::say` 开成 `pub(super)`、另出 `say_for`（回话占多久由那一句定）；
`Session::perform` 接了掀开／关掉、打字、添改路径那几件；`terminal::input` 多收一个 `Window`（覆盖层滚动要知道那一张有几行、露几行，只有终端层知道窗口多大）；
`scene::views_of` 认得 `input`（`add`／`out`／`edit`，候选与轮到第几个）与 `overlay`（`help` 与 `from`）。06 的用例一条没改。

**票面有没有说全根因。** 验收框全勾完之后，「打字与看帮助照设计稿」还漏三样：① 候选的次序（Q790）——设计稿手写的次序不是任何一种排法，实现只能按名字排，
`Tab` 轮换那两屏因此比不了；② 覆盖层上的翻页与到底（`C-d`／`C-f`／`G`）——要窗口尺寸，本票把尺寸交进了 `terminal::input`、`Scroll::Top`／`Bottom` 接好，
半屏与一屏那两对随 08／09；③ 打字时那句「没有以「…」开头的项」屏上看不见（Q792）——票面与设计稿都说「当场说」，说了也画不出来。

**08／09／13 怎么接。** 08：`shell::small` 已按阶段取退出键，跑着那一份 `running.56x14` 只差总进度那一行；滚动条走 `Canvas::scrollbar`，取整对不上再动 Q780。
09：`help-*` 六串——`Scene::named("help")` 已摆好 `cover`，缺的是树；覆盖层上 `C-d`／`C-f` 在 `cover::Scroll::of` 加两个取值、`Sheet::shown()` 就是一屏。
13／14：输入行的种类在 `typing::Purpose` 上加（改一项设置的值、给预设起名），提示词随它；右端那几件按表——值与预设名不该摆 `Tab`，见 Q794。

**review 之后改的。** 标准轴：`viewport.rs` 那个常量插进了 `Viewport` 的文档块与结构体之间（文档挂错处）——挪到块前、改名
`MARGIN_WHEN_TALLER_THAN`；闸门 2 上「只有画法读得到」的四处（`CANDIDATES_SHOWN`、`Purpose::prompt`、`Sheet::column_width`、覆盖层滚动那一手）
各挂一句 `allow`，`Scroll` 那个只是转写 `Deed` 的枚举删掉、并进 `Views::scroll_cover(deed, sheet) -> bool`；`NamedPath::kind` 上那句
「只有画法读得到」已失实（`typing` 读它）——拿掉，措辞收成 `NamedPath::kind_of`，补全框旁边那一句从它取（`Completion::label`）；
`keymap` 里指错的文档链接（`view::Session` → `state::Session`）；抬头「? Esc → 关闭」里的 `?` 不再在画法里写死，改问表上掀开它的那一行
（`keymap::spelt_for(Deed::Help)`）；分隔符只认 `complete::SEPARATORS` 一份；`Completion::from_shown` 收掉夹具里那一份拆法；`Sheet` 的
四个位置字段打成 `Placement`、边距有名字、`depths()` 不再拿 `match` 凑 `&'static str`；设计稿 `viewport` 注释指向 `with_margin`。
规范轴：缓冲里一个分隔符都没有时设计稿在家目录底下补、补回来带 `~/`（`complete` 的 `base = '~'`）——照它改，用例补一条；
`⏎` 收下一个不是归档的文件时两边都说错——并进 Q791。

### 数

review 收完、改完、`cargo fmt --check` 过之后跑的那一趟就是最终状态：`cargo xtask gate` 三条全绿（`.tmp/gate.log`，`EXIT=0`，
一条告警都没有）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1051 通过 0 失败；lib 238 / bin 472；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 872 通过 0 失败；lib 238 / bin 293；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.10s` |

上一趟（06 收尾）是 1030 / 860：闸门 1 多 21 条（`cover` 6、`typing` 6、`viewport` 1、`canvas` 2、`shell` 1、`terminal::redesign` 6
减去改名的一条），闸门 2 多 12 条（特性外面那几个模块自带的：`cover` 6、`typing` 6、`viewport` 1，减去 `cover` 里改名的一条）。
黄金快照 `tests/golden-snapshot.txt` 原样过；设计快照只变了第一步那七屏（`32368c0`），本票之后 `git diff --stat -- tests/fixtures/design` 为空。
`cargo xtask polish` 四条全绿（`.tmp/polish.log`，`EXIT=0`）：fmt 过；clippy 两趟零告警；`cargo doc --no-deps` 仍是
**15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 06 同数）。

### 停车场结转

### Q774 — 序列 `fresh-q` 的期望屏是原型自己的话：屏底「退出（原型里不会真的退出）」，实现上 `q` 交出退出后不再画下一帧

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（原型无法真退出，只能印一句）
- **Where:** `design.html` 的 `onKey`（`q` 那一支的 `toast`）；`tests/fixtures/design/sequences/fresh-q.*`；`src/session/terminal.rs` 的 `redesign::q_before_the_run_hands_out_the_exit`
- **Why it did not block:** 票面写的验收是「还没开始时 `q` 交出退出」，那一支回 `Exit::Leave` 就够；期望屏上那句话是原型替代不了退出才印的，实现画它就是在屏上说一句假话
- **What this ticket actually did:** 用例只断言那一支交出退出，不拿走完那一屏比 `fresh-q`；那份期望屏留在盘上、清单里照旧有它（`every_exported_snapshot_scene_and_sequence_reads_back` 照读）
- **Options:** ① 照旧：这一串的用例断言去留，不比屏；② 设计稿里 `q` 退得出的那两档改成什么都不印、重导这一串，用例比「按了 `q` 之前那一屏」；③ 实现照印那句话——不行，屏上会有一句假话
- **Recommend:** ①——多一份永远比不上的期望屏没有坏处，改设计稿是为了一条不比屏的用例
- **Whose call:** 拍板的人（动设计稿才是 ②）
- **处置：** 待处理。

### Q775 — 设计稿的视口是记着的（`S.from` 粘着、上下各留一行余量），`CONTEXT.md`《视口》与 `session::viewport::Viewport` 是从光标算出来的、不留余量——长列表上光标往回挪时两边画出来的起点不同

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与词汇表对不上（spec《状态》明说《视口》不改）
- **Where:** `design.html` 的 `viewport()`（`S.from[name]` 粘着，`margin = h > 8 ? 1 : 0`）；`src/session/viewport.rs` 的 `Viewport::new`（`from = cursor + 1 - height`，光标本来在格子里就从头画起）；`CONTEXT.md`《视口》「滚动量是算出来的，不是记着的」
- **Why it did not block:** 本票只画开跑之前那一副，15 行摆在 34 行里，两边起点都是零；撞上的是树那一票（`running` 那一景八十多卷）与每页结果那一票——光标从底下往上挪一行时设计稿不动、`Viewport` 立刻跳回从头画
- **What this ticket actually did:** 卷列表照《视口》用 `Viewport`；记下
- **Options:** ① 设计稿的视口改成从光标算（带那一行余量的话 `Viewport` 也加余量，两边同一个式子）并重导有滚动的那几串；② `Viewport` 记一个 `from`——改《视口》那一句，是领域决定；③ 各留各的，滚动的序列一律不比屏——不行，那正是要验的东西
- **Recommend:** ①，余量那一行随它一起定（`CONTEXT.md`《视口》本来就写着「格子高过 8 行时上下各留一行余量」，`Viewport::new` 今天没留）
- **Whose call:** 拍板的人（一边动设计稿、一边动 `Viewport` 的算法）
- **处置：** 待处理。

### Q776 — 从中间省略的截法两边不同：设计稿 `elide` 的尾巴预算是定死的 `w-1-left`，`columns::elide` 把头上没用完的那一格还给尾巴

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（潜在，本票没撞上）
- **Where:** `design.html` 的 `elide`；`src/session/columns.rs` 的 `elide`（「头上没用完的那几格还给尾巴」那一段）；新界面开跑之前那一副的路径那一列读的就是 `columns::elide`（`src/session/shell/list.rs`）
- **Why it did not block:** 一个宽字符跨在头上的预算边界上时才分得出：`w=8`、`a` 后面跟十个汉字，设计稿截成 `a汉⋯汉`（六格），实现截成 `a汉⋯汉汉`（八格）。11 个场景与本票认领的序列里没有一条路径或卷名要省略
- **What this ticket actually did:** 路径那一列照《让位》「截法只有一处出处」用 `columns::elide`；记下
- **Options:** ① 设计稿改成实现的截法并重导有省略的那几屏；② 实现改成设计稿的截法——旧界面的快照跟着变，而那一副本来就要退场；③ 各留各的——撞上那一格就红
- **Recommend:** ①：实现那一格填得更满，`CONTEXT.md`《砍列》「两头留着」要的正是它
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q777 — 窗口太小那一屏在哪一档上都写 `[q → 退出]`，而跑着与等待确认时 `q` 不退（《退出会话》）

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与词汇表对不上（spec 的 story 84 也写着「与 `q`」）
- **Where:** `design.html` 的 `drawTooSmall`（恒 `hint('q', '退出')`）；`tests/fixtures/design/snapshots/running.56x14.*`；`CONTEXT.md`《退出会话》《让位》；`src/session/shell/small.rs`
- **Why it did not block:** 本票只比还没开始那一份（`fresh.56x14`），那一档上 `q` 真退；跑着那一份归总览那一票
- **What this ticket actually did:** 那一句从按键表还没开始那一档的 `q` 行取写法与那一句，不随阶段换——照设计稿；记下
- **Options:** ① 设计稿跑着与等待确认时写 `[C-c → 退出]`（或 `[s → 停止]`），重导 `running.56x14`，实现按阶段从表上取；② 照旧，屏上写着 `q` 而按了没反应（窗口太小那一屏也没有屏底说为什么）
- **Recommend:** ①——屏上不摆按不动的键，是本仓库一贯的规矩
- **Whose call:** 拍板的人（动设计稿与 story 84 的措辞）
- **处置：** 待处理。

### Q778 — 等待确认时配置视图的屏底摆着 `[v → 每页结果]`，而 `v` 只在任务视图里派得出

- **From:** 票 `session-redesign/06`
- **Kind:** 路过发现的设计稿缺陷
- **Where:** `design.html` 的 `stageHints`（`S.pages ? [] : [hint('v', '每页结果')]`，配置视图也走它）与 `onKey`（`v` 只在 `taskKey` 里认）；`src/session/keymap.rs` 的 `ViewPages` 那一行只在卷列表上派
- **Why it did not block:** 11 个场景里没有「配置视图 × 等待确认」那一屏；按键表照《焦点》只在卷列表上派 `v`，屏底照表不摆——真导出那一屏时那一格会对不上
- **What this ticket actually did:** 记下
- **Options:** ① 设计稿的配置视图屏底不摆 `v`；② `v` 在配置视图也派得出——切回任务视图并进那一卷的每页结果，表上那一行放宽到配置那几块
- **Recommend:** ①——答话那三个键在哪一块上都按得动是《焦点》定的，`v` 不是答话
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q779 — 连击键的前半截按表待着，设计稿按写死的几个字待着：`d` 在光标不停在处理路径上时一边待一边不待，`]`／`[` 在还没开始时正相反

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（潜在，本票认领的序列没撞上）
- **Where:** `design.html` 的 `onKey`（`k === 'g' || k === ']' || k === '[' || (k === 'd' && canDelete())` 才 `pending`，不问阶段）；`src/session/view.rs` 的 `Session::deed_of`（`keymap::starts_a_combo`：这一档、这一块上表里有以它开头的连击键才待）
- **Why it did not block:** 两个方向各差一下：光标停在输出目录或「＋ 添加路径」上按 `d`，实现右端多一个 `d…`、`dd` 落到空处，设计稿什么都不做；还没开始时按 `]` 或 `[`，设计稿右端留 `]…`，实现不待（表上 `]d`／`[d` 只在清点完之后派）。`fresh-d` 与 `fresh-dd` 的光标都停在处理路径上，两边逐格相等；`]`／`[` 在还没开始时没有一串序列按过。屏底那一件 `[dd → 删除]` 设计稿本来也不随光标换
- **What this ticket actually did:** 一律按表待着，不问光标那一行是什么、不写死哪几个字；记下
- **Options:** ① 设计稿改成与表一致：`d` 一律待着、`]`／`[` 只在清点完之后待着，重导——本票认领的序列一格不变；② 实现照设计稿写死那几个字外加问光标那一行——按键表之外多一维、多一份名单
- **Recommend:** ①（屏上摆着 `[dd → 删除]` 按下去就该待着；`]d` 派不出的档上留一个 `]…` 是在等一个不存在的后半截）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q783 — 清点中那一景的屏底摆着 `[]d → 下一个问题] [/ → 搜索]`，而设计稿自己的 `KEYMAP` 与实现的按键表都把这两个键标成清点完之后才派得出

- **From:** 票 `session-redesign/06`
- **Kind:** 路过发现的设计稿缺陷（`footerHints` 的跑着那一副不分清点中）
- **Where:** `design.html` 的 `footerHints`（`if (!r) … else [...stageHints(), hint(']d', …), …, hint('/', …), …]`，`surveying` 也走这一支）与 `KEYMAP`（`/`、`]d [d` 标 `RDE`）；`tests/fixtures/design/snapshots/survey.120x36.*`；`src/session/keymap.rs` 那两行（`AFTER_SURVEY`）；`CONTEXT.md`《按键表》「清点完之前 `/` 那几个键派不出去」
- **Why it did not block:** 本票不画清点中那一景；实现照「屏上不摆按不动的键」从表上挑，清点中那一档屏底不会有这两件——总览那一票（08）逐格比 `survey.120x36` 时那一行会对不上
- **What this ticket actually did:** 记下
- **Options:** ① 设计稿的清点中屏底不摆这两件，重导 `survey.*`；② 表上把 `/` 与 `]d`／`[d` 放宽到清点中——按下去没有树可搜、没有问题可跳，屏底摆着按不动的键
- **Recommend:** ①（设计稿的 `KEYMAP` 自己也是这么标的，`footerHints` 只是漏分了一档）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q764 — 设计稿的假盘没有导出：夹具建的那棵树是 11 个场景的场景数据提到的每一处的并集

- **From:** 票 `session-redesign/05`
- **Kind:** 票面没想到的第三种情形（票面说「照设计稿的假盘建出那棵树」，而 02 导出的场景数据里没有假盘）
- **Where:** `.scratch/session-redesign/design.html` 的 `FS`（假盘）；`export.js` 的 `sceneData`（不导它）；`src/session/scene.rs` 的 `disk`／`build_disk`
- **Why it did not block:** 屏上看得见的每一处都从场景数据认得出来：处理路径（文件夹还是压缩包）、清点清单上的分区、目录与卷根、备注里的路径（无法访问的地方是目录，非漫画文件是文件）、输出目录、输入行补全框列出的候选。认不出来的只有没人清点也没人补全到的几处：`~/下载/轻小说插图` 底下的三卷（没勾）、`~/Comics/火之鸟` 与 `寄生兽` 底下的卷（只在补全候选里露过名字，建成空目录）
- **What this ticket actually did:** 取 11 个场景的并集，一趟只算一次（`OnceLock`），每个场景各自在临时目录里建一棵；文件都是空的（夹具不碰盘上的内容，只问形状）。序列的场景数据没并进来：它们补全到的只有 `~/`，那几项本来就在。票面「照设计稿的假盘建出那棵树」因此只做到了场景数据认得出的那一部分
- **Options:** ① 照现状；② `export.js` 多导一份 `tests/fixtures/design/disk.json`（设计稿的 `FS` 原样），夹具照它建；③ 在 Rust 里抄一份 `FS`——第二个出处
- **Recommend:** ①，直到哪一票要补全进一个场景数据没提到的目录（`~/下载/轻小说插图/` 之类）再走 ②——那时并集就缺东西了
- **Whose call:** 07 号票的实现者（输入行补全那一票）
- **处置：** 待处理。
