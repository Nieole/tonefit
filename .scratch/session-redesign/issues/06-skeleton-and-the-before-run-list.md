# 06 — 新会话的骨架与开跑之前那一副（与旧界面并存）

**What to build:** 新界面的第一块，**在测试里长出来**；真会话仍进旧界面，切换在 15。

**骨架**（spec《状态：视图 × 阶段 × 焦点》《按键表、屏底与覆盖层》《颜色》）：

- 视图（任务 · 配置）× 阶段 × 焦点的新取值；两个视图各记各的光标与所在的块；
- **一张按键表**，摆在 `tui` 特性外面；屏底由它派生；
- 新的输入入口：终端层把一个输入交给新会话的那一支（起一趟、按停止、答话、预设这些够得着那一趟与盘的在它里面，随各票接上）；
- 顶栏；屏底恒一行（按轻重、摆不下从倒数第二件往前舍、`?` 恒在末尾；回话几秒退回；连击键待续记号）；
- 窗口太小；退出规则（`q` 什么时候退、`C-c`、`Esc` 只退一级）；
- 颜色一处：语义色、种类色与 `NO_COLOR`。

**开跑之前那一副**（spec《卷列表》开跑之前）：

- 输出目录一行 · 「处理路径 (N)」· 每条处理路径一行（勾选框、文件夹还是压缩包按扩展名认、被包含的标一句、没勾的压暗）·
  「＋ 添加路径」；这一副不碰盘；
- `j`／`k`、空格勾选、`dd` 删；
- 屏上路径的家目录缩写成 `~`（家目录由会话入口问一次往下传）；
- 处理路径那一族改名：屏上文案与类型名都叫处理路径（收停车场 Q711）；
- 总览「还没开始」宽窄两副。

与旧界面共用三组设置那一块；旧的焦点、画法与用例原样留着、照编照过。
对不上设计快照时：记停车场 → 拍板 → 先改设计稿并重新导出 → 再改实现；快照一格都不许为变绿而改。

**Blocked by:** 05 — 按场景数据摆出那一趟与三组设置的夹具

**Status:** resolved

- [x] 「还没开始」120×36、80×24 与「窗口太小」还没开始那一份逐格相等（字、前景色、修饰）
- [x] 交互序列 `j`／`k`、空格勾选、`dd` 删一条，走完与期望屏相等；还没开始时 `q` 交出退出
- [x] 用例：屏底每一件都出自那张按键表
- [x] 用例：`NO_COLOR` 在场时颜色退回默认，加粗、下划线、粗框、压暗照旧
- [x] 按键表、焦点、家目录缩写在 `--no-default-features` 那一趟编得过、测得到
- [x] 真会话仍进旧界面，旧用例照绿；设计快照未改
- [x] 停车场 Q711 挪进「已了结」
- [x] 类型名取自《会话》；没英文名的词条当场补上

## 落地记录

**做了什么。** 新界面的第一块在测试里长出来了；真会话仍进旧界面（`src/main.rs` 的 `session::enter` 一字没动），
旧的焦点、画法与用例原样留着、照编照过。三处接缝各一条红→绿：

- **画面**：`session::shell::draw` 在 `TestBackend` 上画整屏，逐格对 `fresh.120x36`、`fresh.80x24`、`fresh.56x14`
  （`src/session/shell.rs` 的三条用例；对不上时 `design::assert_same_cells` 印出实际那一屏的两张网格——
  头一趟红在卷列表抬头一个字上，改法就是照设计稿的字）。顶栏 → 总览「还没开始」宽窄两副 → 卷列表开跑之前那一副 → 屏底 → 窗口太小，
  一块一个模块。
- **交互**：`terminal::input`（与旧的 `press` 并排，收 `view::Input`——键或鼠标，带这一帧的「此刻」）喂 `fresh-j`、`fresh-k`、
  `fresh-Space`、`fresh-Space-Space`、`fresh-d`、`fresh-dd`，走完与期望屏逐格相等；`fresh-q` 那一支交出 `Exit::Leave`
  （那份期望屏是原型自己的话，见《停车场结转》与 Q774）。序列的步从 `manifest.json` 读（`scene::sequence`、`Step`），
  起点是 `Scene::named("fresh")`，那一趟由新加的 `Running::holding(live)`（只在 `test` 里）交给终端层。
- **纯逻辑**（`tui` 外面，闸门 2 照编照测）：按键表（`keymap`，屏底由它派生——`every_hint_on_the_footer_is_a_row_of_the_key_table`
  在四个阶段 × 两个视图上逐件核）、视图 × 阶段 × 焦点的新取值与状态机（`view`）、家目录缩写（`home`）、
  退出规则（`q_quits_only_before_and_after_a_run_ctrl_c_always_and_esc_never`）、`NO_COLOR`
  （`without_colour_only_the_hues_fall_back_and_every_modifier_stays` 整屏比；`every_kind_has_its_colour_and_no_colour_strips_only_the_hue` 逐种比）。

**新骨架的模块名单**（`src/session.rs`《新界面与旧界面并存》）：

| 模块 | 装的是 | 07／08 在它上面怎么接 |
|---|---|---|
| `session::look`（特性外） | `Kind`（种类色）、`Hue`（颜色的要法）、`Look`（要法加四样修饰）、`Segment`（一截字加样子） | 灰阶档位、环节、处理中那几色已在 `Kind` 上，画法直接要 |
| `session::home`（特性外） | `Home`：`abbreviate` 缩写 `~`、`expand` 认 `~/` | 输入行（07）认 `~/` 走 `expand`；家目录在 `Session::home` 上 |
| `session::keymap`（特性外） | 一张表 `TABLE`（键 · 事 · 写法 · 短句 · 长句 · 阶段 · 块）；`deed`／`starts_a_combo`／`spelt`／`hints` | 全部按键（07）按 `Group` 与 `long` 列、同 `(group, long)` 并 `spelt`；新键加一行 |
| `session::view`（特性外） | `View`、`Focus`（七块）、`Cursor`（行的身份）、`Line`（开跑之前的行）、`TaskView`／`ConfigView`／`Views`、`Input`、`Pending`／`Reply`；`Session::deed_of`／`perform`／`hints` | 覆盖层与输入行（07）：`Views` 上加 `cover`，`focus()` 先看它；树（07）：`Cursor` 加目录／卷／备注，`lines` 换成树的行；每页结果／自动滚动（08）挂在 `TaskView` 上 |
| `session::shell`（`tui` 后） | 整屏 `draw`；`canvas`（逐格写、画框——设计稿 `Screen` 的 `put`／`line`／`box`）、`yielding`、`topbar`、`overview`、`list`、`footer`、`small` | 各块随阶段换：`overview::lines`／`title` 按 `Phase` 分支；`list::draw` 换成树；`footer` 的输入行；确认条、每页结果、覆盖层各新开一个模块 |
| `terminal::input`／`translate_input` | 新的输入入口：认事（连击键在 `deed_of` 里待着），交回 `Session::perform`；`Home::found` 在 `enter` 问一次家目录 | 起一趟（走 `press` 的 `Action::Start` 那条路）、按停止、答话、添改路径、预设、灰阶测试图各在 `input` 里接一支 |
| `draw::paint::look` | 种类色在屏上各是哪一色、`NO_COLOR` 只退颜色 | 不加第二处 |
| `draw::design::assert_no_background` | Q737 那条断言 | 每条快照与序列用例都调 |
| `scene::views_of`、`scene::sequence` | 场景数据 `session` 那一段里本票认得的几格；清单上的序列读成步 | 树上的光标、展开、每页结果、覆盖层、输入行按各自的票在 `views_of` 里补 |

**改名（Q711）**：`Picked` → `NamedPath`、`scope.volumes` → `scope.paths`、`Field::Volume`／`AddVolume` → `Field::Path`／`AddPath`；
`NamedPath::is_archive`／`kind` 按扩展名认（`tonefit::is_archive` 公开出来，格式集仍只有一份）。旧界面那一行的文案
「＋ 再打一个卷进来」随那一副在 15 号票退场，旧快照一格没动。

**没做的（按票面归别的票）**：`o`／`i`／`?`（07）、`t`／`x` 起跑（08：`input` 里那两件眼下落在 `perform` 的空处）、清点之后的树、
每页结果、配置视图、覆盖层、输入行、鼠标。`Session::hints` 里别的阶段与配置视图那几支按设计稿 `footerHints` 抄了骨架，
只被「出自那张表」那条用例核过，没对过快照——各票接上时按自己的快照改。窗口太小那一屏跑着时的总进度归 08。

**review 之后改的。** 标准轴：闸门 2 那一趟「只有画法读得到」的几处从模块级 `allow` 改成各挂各的一句（`gate.md`《读结果》）；
`input` 的文档那两个不存在的参数、`session.rs` 那句「七个」；用例改问行为不问枚举（视图切换问屏底、光标问「第几条 of 几条」，
序列用例不再重复断言光标）；`canvas` 的用例拿 `paint::look` 的答案比、不点颜色名；按键表的阶段集从 `I`／`IE`／`SRD` 改成
`FRESH`／`NOT_RUNNING`／`IN_A_RUN`；`canvas::Frame` 改叫 `Border`（与终端库的一帧撞名）；Q711 剩下的半截
（`Shape::Named`、`toggle_path`／`remove_path`、`with_paths`）；屏上顺口提一个键的三处（总览 `t`／`x`、行上 `i`／`o`、窗口太小的 `q`）
收成一手——都走 `keymap::hints`，`[o → 添加]` 那一句进表（`AddPath` 第二行）；路径那一列的宽度整张表算一次；
`Deed::Top`／`Bottom` 不再拿 `isize::MAX / 2` 冒充到底；`CONTEXT.md` 补 `Reply`／`Pending`／`Cursor`／`Phase`／`Chord`／`Deed`／
`Look`／`Hue`／`Segment` 的英文名；种类色名单与设计稿对不上记进 Q781。规范轴：`t`／`x` 起一趟那一支从 `input` 里拿掉（归 08，
半接不如不接）；`Home::found` 在会话入口问一次；`tonefit::is_archive` 公开与 spec《缝》的张力记进 Q782；连击键前半截的两个方向
并进 Q779；清点中那一景屏底的 `]d`／`/` 记进 Q783。留着没动的两处判断题：`differs_from` 那个按 `Field` 的 match（与 `shown` 同形，
配置视图那一票要的 `*` 也读它，那时再看要不要合）；`keymap` 与 `view` 互相 `use`（`Focus` 归视图那一维，表只是引用它）。

**数。** review 收完、改完、`cargo fmt --check` 过之后跑的那一趟就是最终状态：`cargo xtask gate` 三条全绿
（`.tmp/gate.log`，`EXIT=0`）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1030 通过 0 失败；lib 238 / bin 451；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 860 通过 0 失败；lib 238 / bin 281；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.50s` |

上一趟（05 收尾）是 995 / 840：闸门 1 多 35 条（`keymap` 6、`view` 9、`home` 3、`look` 2、`scene` 1、`shell` 9 连同 `canvas`／`yielding`、
`terminal::redesign` 5、`paint` 1 减去删掉的一条），闸门 2 多 20 条（特性外面那几个模块自带的）。黄金快照 `tests/golden-snapshot.txt` 原样过，
设计快照与序列一格没动（`git diff --stat -- tests/fixtures/design` 为空）。
`cargo xtask polish` 四条全绿（`.tmp/polish.log`，`EXIT=0`）：fmt 过；clippy 两趟 `Finished`、零告警；`cargo doc --no-deps` 仍是
**15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 05 同数）。

### 停车场结转

### Q711 — 范围层把点名路径叫成「卷」：`＋ 再打一个卷进来`、`[x] 库/卷1`

- **From:** 会话界面重设计的拷问（`tui-redesign`，尚未立票）
- **Kind:** 词汇表与实现对不上（屏上措辞与类型名）
- **Where:** `src/session/state.rs` 的 `Field::AddVolume`（屏上 `＋ 再打一个卷进来`），`src/session/draw/config.rs` 范围层那几行（`[x] 库/卷1`），以及 `scope.volumes` 这一族名字
- **Why it did not block:** 这一格装的是《点名路径》——一个归档或一个目录，发现把它展开成一批卷（ADR 0014），一条底下可以是几个系列、几百卷；叫它「卷」会让人以为点名一个库根只算一卷。词条是本次拷问里新立的，实现还没跟上。
- **What this ticket actually did:** 只立了词条，一个字的实现都没动。
- **Options:** ① 界面重设计落地时一并改名（屏上文案与类型名都取《点名路径》）；② 先单独开一张小票改名。
- **Recommend:** ①——重设计要把点名路径从左栏末尾搬成任务视图的主干，那一刀会重写这几处。
- **Whose call:** 协调人。
- **处置：** 待处理。

### Q737 — 逐格比对不比背景色：设计稿表达不了它，实现真设了背景色比对看不见

- **From:** 票 `session-redesign/02`
- **Kind:** 验收条含糊（「逐格比字、前景色、修饰」三样，spec 说的也是这三样；背景色那一维快照上没有）
- **Where:** `src/session/draw/design.rs` 的 `same_cells`；spec《颜色》「不设背景色」
- **Why it did not block:** spec 定了不设背景色，快照上因此没有那一维；这一票的比对器照 spec 的三样比
- **What this ticket actually did:** 只比字、前景色、修饰；落地记录《漏网》里记着这一类比对看不见
- **Options:** ① 比对器另加一条独立断言：实际那一屏每一格的背景都是 `Color::Reset`（与快照无关，一处写死）；② 照旧，18 号票并排看时人眼查
- **Recommend:** ①，随 06 号票（骨架、颜色一处）加进去——一行代码换一整类差异
- **Whose call:** 06 号票的实现者
- **处置：** 待处理。
