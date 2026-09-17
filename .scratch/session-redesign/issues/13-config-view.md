# 13 — 配置视图：设置栏、详情栏、下钻与画质判定参数

**What to build:** 新界面上调设置的那一屏（spec《配置视图》；停车场 Q714 的结论）。预设栏在 14，这一票只画顶上预设条那一行。

- `2` 进配置视图。顶上一条预设：当前套的是哪一份、改了哪几项、`p`／`c` 两个提示、预设文件位置（家目录缩写成 `~`）；
- **设置栏**三组：设备设置 · 处理选项 · 画质判定参数（只读）；项名与当前值列对齐；与套着的预设不同的项行尾带 `*`；
- **详情栏**答光标那一项：取值环（第一格恒是「没说」、生效那一格 `✓`、光标那一格 `❯`）、说明、预设里这一项怎么设；
  `l`／`⏎` 定，`h`／`Esc` 一格不改地回设置栏，`⇥` 在两栏之间切；自由填的项 `i` 经输入行改；
- **下钻**：型号那一项先列屏幕规格、进去再列型号；换了型号清空可见灰阶数与画质门槛；
- **画质判定参数**五行：行内那一句取自报告抬头那一处，选项冲突没咬上时写「无」；详情栏的长说明是会话自己的字；印的是此刻的设置；
- 不到 90 列退成单栏：`l`／`⏎` 进详情，`h`／`Esc` 回来；
- 跑着与等待确认时整个视图只读：进得去、看得见，定的那一下屏底说设置已锁定；顶栏右端带这一趟的进度或「等待确认」。

真会话仍进旧界面。

**Blocked by:** 07 — 输入行、补全框与全部按键

**Status:** resolved

- [x] 「配置」120×36、80×24 逐格相等
- [x] 序列：设置栏 `l` → 选一格 `l` 定；`l` → `h` 不改；型号两层下钻与 `h` 退回；自由填的一项 `i` 改；80 列单栏进出；跑着时 `2` 看只读与顶栏进度、定的那一下屏底提示；走完与期望屏相等
- [x] 用例：画质判定参数行内的字与报告抬头那一处的输出逐字相同
- [x] 用例：跑着时一个改动都定不下（阶段那一维拦下）
- [x] 真会话仍进旧界面，旧用例照绿；设计快照未改

## 落地记录

**本票做了什么。** 新界面的**配置视图**整屏落地：顶上一条预设、设置栏三组、详情栏（取值环 · 下钻 · 自由填 · 画质判定参数）、
不到 90 列退成单栏、跑着与等待确认时整屏只读。三处接缝各一条红→绿：

- **画面**：`config.120x36`、`config.80x24` 逐格相等（`shell::tests::the_config_scene_matches_its_design_snapshot_wide_and_narrow`，
  连同 `assert_no_background`）。画法按屏上那几块新添三个模块——`shell::preset`（顶上一条：当前套的是哪一份、改动了哪几项、
  `p`／`c` 两件与预设文件的位置；那两件问的是**底下那一块**，打字时照样写着）、`shell::settings`（三组二十三行、项名与取值列对齐、
  行尾那个 `*`、`[已锁定]`、装不下时右框线上的滚动条）、`shell::details`（取值环每一格、屏幕规格与它底下的型号、自由填那一项的
  `[i → 修改]`、画质判定参数整句摊开、说明、预设里这一项怎么设、锁定那一句）。让位多两档（`single_column`、`settings_width`）。
  顶栏补上人在配置视图时右端那一截进度或「等待确认」——**转轮的字形与那 90 毫秒摆在 `tui` 外面**（`view::SPINNER`／`Views::spinning`），
  顶栏、行首记号与总览三处将来读的是同一份。
- **交互**：经 `terminal::input` 喂序列，**十八串逐格相等**——`config-h` `config-Tab` `config-Escape`（三条路都回设置栏、一格不改）、
  `config-h-l` `config-h-l-k-h` `config-h-l-k-l`（进详情栏停在生效那一格、退回不改、定下上一格）、`config-model` `config-model-drill`
  `config-model-drill-h` `config-model-drill-j-l`（两层下钻、退回屏幕规格那一层、换型号清空那两个标定数）、`config-levels-i`
  `config-levels-i-typed`（自由填那一项 `i` 经输入行改）、`config-narrow-h` `config-narrow-h-l` `config-narrow-h-l-h`（80 列单栏进出）、
  `running-2` `running-2-fit-l` `running-2-fit-l-l`（跑着时进得去、看得见、定的那一下屏底说设置已锁定）。
- **纯逻辑**（`tui` 外面，闸门 2 照编照测）：`session::config`——设置栏那二十三行（三组各一个抬头加它那几项，停得住的二十项）、
  一项此刻印什么值、详情栏列得出哪几格（[`Choices`]：环 · 屏幕规格 · 型号 · 自由填 · 只读）、与套着的预设差了哪几项、
  预设里这一项怎么设，以及每一项底下那一段长说明——**屏上唯一一段会话自己写的散文，出处只有这一处**。

**画质判定参数那一组：单一出处落在 `render` 上。** 行内那一句**就是报告抬头里的那一行**（ADR 0016），实现因此把抬头那四行
提成 `render::judging(which, profile, switches)`（行内那一句）加 `render::Judging`（五项与它们的项名），选项冲突那几条走
`render::interlocking`；`render::header` 与会话的设置栏、详情栏读的是同一处。用例
`config::tests::every_judging_row_is_a_line_of_the_report_header` 拿 `running` 那一景**真跑出来的那份报告**的抬头逐行比——
有人在会话这一侧另抄一份，它当场变红。选项冲突一条都没咬上时那一行写「无」，而**那一刻它不注「与抬头逐字相同」**
（抬头那一刻一个字都不说）——那一条另有一条用例。

**只读是阶段那一维拦下的。** `Session::settings_locked()` 问的是 `Stage::read_only()`，与焦点无关：详情栏照样进得去、看得见。
用例 `during_a_run_not_one_setting_settles_and_the_stage_is_what_stops_it` 走同一副键（`l` 进详情栏、挪一格、`l` 定下来，
型号那一项多一层下钻），**两趟只差阶段**：还没开始那一档设备设置与处理选项十五项一项不落都改得动，跑着与等待确认那两档一项都改不动，
自由填的那几项连输入行都开不起来。挪一格试两个方向——取值停在环末一格上时 `j` 挪不动，得往回挪才落到另一格上。

**在 07 的骨架上改了什么**（阻塞边）：`ConfigView` 从「一个块加一个 `Field` 光标」长成「两栏各自的光标加下钻进了哪一块」
（`cursor: Item`、`choice: usize`、`drill: Option<Panel>`——记那一块本身、不记它排第几，与旧界面 `Values::panel` 同一副）；
`Views` 添两格「入口问一次往下传」的东西：`presets`（预设文件在哪）与 `clock`（会话的时钟起点，转轮从它算）；
`Deed` 添 `ConfigEnter`（设置栏上的 `⏎`：自由填那几项直接开输入行）与 `EditValue`（`i`，长的那一句为空，不上全部按键那一张）；
按键表的 `ConfigOpen` 按**块**收窄成六行——设置栏是 `l → 展开`、详情栏是 `⏎ → 确定`、只读那三档两边都是「查看」，
短的那一句为空的两行只管派键，长的那一句四行相同、全部按键那一张照旧并成 `l ⏎`（`help` 那张快照一格没变）；
`Deed::Chart` 补上短的一句（顶上那一条摆 `[c → 灰阶测试图]`，屏底照旧不摆它）；`typing::Purpose` 添一种「改一项设置的值」，
提示词就是那一项的名字、`Tab` 在它上面什么都不动（`Purpose::completes`）、`⏎` 走那一项自己的界（`Session::take`）、
解析不过就留在输入行上；`state` 把 `ring`／`settle`／`take`／`typed`／`unsaid`／`set_device`／`calibrated_profile` 开成 `pub(super)`
——**两副界面、三条入口改的是同一格**，摊开这一路没有自己的写入路径；`scene::views_of` 认得 `session.config` 那几格与
`input.kind == "value"`。06／07 的用例一条没改。

**`CONTEXT.md` 只动了两处，都是加英文名**（spec《状态：视图 × 阶段 × 焦点》那条「词条没给英文名的几个，落地时当场补上」）：
《设置栏》里「三**组 (Band)**」、《画质判定参数 (Judging)》。**一条词条的含义都没改写**——颜色那几样与别的几处对不上的
都记进了停车场。

**代价。** 21 个文件、+2094／−84；新添四个模块（`session/config.rs` 一个、`shell/` 三个）。`tests/fixtures/design/` **一个字节没改**
（`git diff --stat -- tests/fixtures/design` 为空）。真会话仍进旧界面：`draw::shell` 与它那一副的用例一个字没动。

**停车场。** Q824（夹具里预设文件那条路径是摆出来的：设计稿把它写死了，而真文件挪不进假家目录——`~/` 底下多一项会让补全那几串红）、
Q825（种类色又添两样加颜色第三档「说明正文」，《语义色》那条名单第三次对不上）、Q826（会话的**时钟起点**：转轮转到第几格要一个原点，
而票面与 spec 都只说了「此刻」）、Q827（Q794 在这张票上**不成立冲突**：值那一种输入行的起点是还没开始，表上 `Complete` 正好派得出）、
Q828（「掀着预设栏」与「在哪一栏」在设计稿上是两维，实现把它们并成了焦点一维——本票掀不开预设栏，14 会撞上）。

**review 之后改的。** 标准轴：`open_valuing` 顶着 `cancel_typed` 的那一句文档（插错了位置）——两边各归各位；
`render.rs` 里指着一个不存在的 `Premise::line`——改成 `premise_line`；`ConfigView.drill` 从裸下标改成 `Option<Panel>`
（`choices` 不再重新验边界，`scene` 也不再按名字反查一个序号）；`Choices::lands_on` 收掉四处重复的 `match`；
详情栏三种取值那三段一样的 `enumerate().map(…)` 并成一个闭包；`Row::said` → `Row::plain`（一处 diff 里三个「said」）；
`config.rs` 的模块文档把那一段长说明也认下来（它本来只说自己「答屏上那几行各是什么」）；`Judging` 与 `Band` 补进 `CONTEXT.md`。
规范轴：`terminal::enter` 补上 `session.views.presets`——那一格的文档说「由会话入口问一次摆进来」，而入口没摆，
真会话里顶上那一条右端本来会空着；`⇥` 在预设栏掀着时不再把它切没（改成一个字不动，并记 Q828）。

**票面有没有说全根因。** 验收框全勾完之后还有三样活着：
① **预设栏（`p`）与灰阶测试图（`c`）按不动**——两件都归 14，本票只画顶上那一条里提它们的两个提示，按下去眼下什么都不发生；
② **鼠标点不中**——设计稿每一帧交出的「点得中的区域」（设置栏每一项、详情栏每一个值）这一票一处都没交，`config-click-*` 归 16；
③ **真会话进不到这一屏**——整副仍只在用例里长着，切换归 15。票面自己写着这三条，不是漏；
**票面真正没说的是一处**：「定的那一下屏底说设置已锁定」在自由填的那几项上**不成立**——设计稿那一支只读时一个字都不说，
本票照设计稿（ADR 0019 决定第 13 条），两边因此差一句话。

### 数

review 收完、改完、`cargo fmt --check` 过之后跑的那一趟就是最终状态：`cargo xtask gate` 三条全绿
（`EXIT=0`，**告警一条都没有**）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1062 通过 0 失败；lib 238 / bin 483；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 875 通过 0 失败；lib 238 / bin 296；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1.12s` |

上一趟（07 收尾）是 1051 / 872：**闸门 1 多 11 条，闸门 2 多 3 条，两处都全在 bin，lib 238 一条没动**
（这张票一个字都没碰库）。闸门 1 多的 11 条是 `shell` 1（配置那两屏）、`terminal::redesign` 6（十八串序列）、
`config` 3（抬头逐字、没咬上时写「无」、跑着时定不下）、`shell::yielding` 1（90 列那一档与设置栏宽度）；
闸门 2 只多 `config` 那 3 条——另外 8 条要画法或终端层，都在 `tui` 后面。
黄金快照 `tests/golden-snapshot.txt` 原样过；`git diff --stat -- tests/fixtures/design` **为空**。

`cargo xtask polish` 四条全绿（`EXIT=0`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check`（目录 `target`） | 绿 |
| 2 | `cargo clippy --all-targets`（目录 `target`） | 绿，**零告警** |
| 3 | `cargo clippy --all-targets --no-default-features`（目录 `target/gate/no-default-features`） | 绿，**零告警** |
| 4 | `cargo doc --no-deps`（目录 `target`） | 绿，仍是 **15 条告警**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 06／07 同数） |

**第 3 条那一格是这张票踩过的坑，水位写在这里给下一个人：** 头一趟它报了 **12 条 `dead_code`**——
闸门 2 那一趟 `test` 开着，模块头上那句 `expect(dead_code)` 因此不生效，而「只有画法读得到」的那十几处
在那一趟里全没有读者。按 `docs/agents/gate.md`《读结果》**各挂各的一句**、不整个模块放开：
`config.rs` 十一处（`Band::name`／`note`、`Item::label`／`shown`／`said`／`about`、`Line`、`lines`、
`preset_says`、`about_setting`、`about_premise`）、`view.rs` 五处（`SPINS_EVERY`、`SPINNER`、`Views::spinning`、
`config_position`、`config_line`）挂的是 **`expect` 不是 `allow`**——将来 `tui` 外面接上读者，那一行自己报「没用上」。
转轮那三处的 reason 点明「顶栏此刻读它，行首记号与总览那两处随 08 接上」。
`render.rs` 的 `Judging::ALL` 另算一格：读它的是**会话**，而会话挂在 `any(feature = "tui", test)` 上，
条件因此是 `not(any(feature = "tui", test))`——写成 `not(feature = "tui")` 会在闸门 2 那一趟变成一句
**没兑现的 `expect`**（那一趟 `test` 开着，它有读者）。
