# 09 — 「差异大的页」、长说明与灰阶档位那一环的措辞

**What to build:** 三处屏上的字对齐到一个出处，设计稿与实现一起改、重导：

- 总览、确认条、分区与目录行的汇总里，一律用词汇表的名字「差异大的页」（Q731）；
- 详情栏的长说明写成「`height`（按高度铺满）：……」这种写法，称呼与取值那一格同源，用户从屏上抄得出命令行（Q732）；
- 灰阶档位那一环列四格（1bit · 2bit · 4bit · 8bit），说明写清 8bit 那一格在电子墨水屏上派不上用场
  （灰阶硬上界，ADR 0003）；那一环照旧由穷尽的取值生成（Q730）。

**Blocked by:** 01

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 屏上再没有「与其他页差异大」「差异大 N 页」这类第二种叫法（每页结果原因那一列的「差异大，单独判断」是库的那一句理由，留着，Q1227）
- [x] 有取值写法的每一项，详情栏长说明里带着那个写法（读作环上每一格，Q1229）
- [x] 灰阶档位那一环四格，说明与之相符
- [x] 受影响的快照与序列比整屏且绿
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 红的仍只有 macOS 基线那一条，Q995）

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q731 — 「需留意的那几样」的词三处各说各的：库的 `Envelope` 说「差异大 N 页」，会话画法说「差异大的页」，设计稿总览说「与其他页差异大 N 页」

- **From:** 票 `session-redesign/01`
- **Kind:** 两处文档互相打架的那一类（词汇表的名字与库、界面层三处不一致）
- **Where:** `src/envelope.rs` 的 `impl Display for Envelope`（`差异大 {} 页`）；`src/session/draw/pages.rs` 的 `says`（`差异大的页`、`尺寸未贴合屏幕`、`页面超宽`、`兜底上界`、`代表页`）；
  `CONTEXT.md`《语义色》《需留意的页》（`差异大的页` · `残缺`）；设计稿总览的问题行与确认条（`与其他页差异大 N 页`，本票不碰）、分区与目录行的汇总（`转换失败 N`、`无法访问 N`，末尾那几小结的抬头是 `卷转换失败 N 卷`、`无法访问 N 处`）
- **Why it did not block:** 卷行行尾与每页结果提示那一列本票已对到 `says` 与词汇表（`差异大的页`、`残缺`、`救回 62.0%`）；总览与确认条是界面层自己的句子，票面明写不在对齐之列；
  目录行的隔离计数对到了 `render::isolated_note`（`隔离 N 卷`），另两个汇总词库里没有目录一级的说法。
- **What this ticket actually did:** 只对了票面点名的那几处；总览、确认条、分区/目录行的汇总词原样留着。目录行整目录跳过时行尾那一句沿用卷行的 `SKIPPED` 常量——库在目录一级没有这一句（`base_of` 只把它压成「跳过」一词进统一档位分布），那一行因此也是界面层复述，一并归这一条。
- **Options:** ① 08／10 号票落地时总览、确认条与分区/目录行的词统一取词汇表的名字（`差异大的页`）；② 词汇表与 `says` 改成库的「差异大」。
- **Recommend:** ①——词汇表的名字是六样一起定的，库那一句是整卷统一灰阶那一行里的计数，不是页那一级的名字。
- **Whose call:** 协调人（随 08／10 号票）。
- **处置：** **本票了结，照推荐 ①**：总览问题行宽窄两副、确认条一律写词汇表的名字「差异大的页」，词取 `marks::notable_word` 一处；分区与目录行的汇总本来就不数差异大的页，没有可改的；库的 `Envelope` 那一句没改（Q1227）。见《落地记录》。

#### Q732 — 详情栏长说明里对选项的称呼（「按高度铺满」「逐个读」）与取值那一格印的写法（`height`、`serial`）对不上

- **From:** 票 `session-redesign/01`
- **Kind:** 票面没想到的第三种情形（本票把取值写法对到库之后新生的）
- **Where:** `design.html` 的 `CONFIG` 里 `fit`、`crop`、`split`、`order`、`depth`、`dither`、`io` 各项的 `desc`（详情栏的长说明，票面明写不在对齐之列）；
  同一项的 `options` 已改成 `FitMode::name()` 一类的写法（`height`／`inside`、`rtl`／`ltr`、`off`／`fs`、`auto`／`serial`／`concurrent`）
- **Why it did not block:** 长说明是界面层自己的解释文字（spec《配置视图》：详情栏的长说明是会话自己的字），本票不碰；对不上的只是称呼，读得懂。
- **What this ticket actually did:** 取值那一格改了，长说明一个字没动。
- **Options:** ① 13 号票落地时长说明改成「`height`（按高度铺满）：……」这种写法，把两种称呼并在一处；② 长说明照旧只用中文称呼。
- **Recommend:** ①——取值那一格与命令行同一种写法是库定的（`src/preset.rs`《取值的写法只有一份》），说明里不带它，用户从屏上抄不出命令行。
- **Whose call:** 协调人（随 13 号票）。
- **处置：** **本票了结，照推荐 ①**：有取值环的每一项，长说明里叫取值用的就是取值那一格的写法、中文称呼跟在括号里；设计稿先改、重导，实现逐字照它。见《落地记录》。

#### Q730 — 灰阶档位那一环：设计稿三档（1bit · 2bit · 4bit），实现的环四格（多一格 8bit），词汇表的全集是 {1,2,4,8}

- **From:** 票 `session-redesign/01`
- **Kind:** 票面没想到的第三种情形（取值的写法对齐了，取值的个数对不上）
- **Where:** `design.html` 的 `CONFIG` 里 `depth` 那一项与它的说明（「电子墨水屏最多显示 16 级灰，所以只有这三档」）；`src/session/state.rs` 的 `next_bit_depth`
  （`ring_of(BitDepth::One, next_bit_depth).len() == 4`）；`CONTEXT.md`《灰阶档位》
- **Why it did not block:** 本票只对写法，环上有几格是配置视图那张票（13）的事；设计稿那句说明说的是判定会挑的三档，而 `--bit-depth 8` 命令行上仍收。
- **What this ticket actually did:** 三档的写法改成 `1bit`／`2bit`／`4bit`，没有加第四格。
- **Options:** ① 13 号票照实现列四格，设计稿说明改口；② 会话的环去掉 8bit（命令行照收），设计稿不动。
- **Recommend:** ①——环由穷尽的 `match` 生成（`state.rs` 那段注释写着为什么），少列一格就要在会话里另抄一份清单。
- **Whose call:** 协调人（随 13 号票）。
- **处置：** **本票了结，照推荐 ①**：设计稿那一环列四格，说明逐档说几级灰、说清 8bit 在电子墨水屏上派不上用场；实现的环照旧由 `next_bit_depth` 穷尽生成，一格没动。见《落地记录》。

## 落地记录

**本票做了什么。** 三处屏上的字对到一个出处，每一处都是先改设计稿、重导，再改实现。

| 件 | 设计稿（`design.html`） | 导出（`export.js`） | 实现 |
|---|---|---|---|
| 「差异大的页」一个叫法（Q731） | `drawOverview` 问题行宽那一副写 `差异大的页 N 页`、窄那一副写 `差异大的页 N`；`drawDecision` 宽那一副写 ` ⋅ 差异大的页 N`；`describe()` 里那一句（不上屏）同改 | — | `shell::overview` 问题行两副（`notable_bits`）、`shell::decision` 确认条那一截，词改从 `marks::notable_word` 取，与卷行行尾、每页结果提示那一列读同一份；`notable_word` 的文档写上这两处读者 |
| 长说明带上取值的写法（Q732） | `CONFIG` 九个取值环的 `desc` 改成「写法（称呼）：…」：缩放方式、阅读方向、抖动、读盘方式添上写法；裁白边、拆分跨页以那一格的写法打头、各添另一格一句；缩放算法五个都点名；整卷统一灰阶里「和其他页差异特别大的页」换成「差异大的页（…）」 | 8 串 `config-{crop,split,order,filter,depth,dither,envelope,io}`：`config` 那一景，`onItem(n)` 回设置栏、往下挪 n 项、`l` 进详情栏 | `config::about_setting` 逐字照设计稿 |
| 灰阶档位那一环四格（Q730） | `depth` 的 `options` 添 `8bit`；说明逐档说几级灰，说清 8bit 在电子墨水屏上派不上用场（可见灰阶数不到 256 时开跑就被拒） | `config-depth` | 环本来就四格（`state` 的 `next_bit_depth` 穷尽生成），`state.rs` 没动；说明照设计稿 |
| 设计稿的折行（Q1231） | `wrap` 补上起首记号不落行尾（`NO_TAIL`，与 `crate::wrap::NEVER_ENDS_A_ROW` 同一张表） | — | — |

- **库那一侧**：`Envelope` 的「差异大 N 页」与原因那一列的「差异大，单独判断」都没改，命令行与黄金快照一个字节没动（Q1227）。
- **宽那一副问题行的量词**照「名字 数 量词」的排法留着（`差异大的页 1 页`，Q1228）。
- **「有取值写法的每一项」读作环上每一格**（Q1229）；说明与取值那一格各写一份，对得上靠用例（Q1230）。
- **「全部按键」那一张的灰阶写法**照旧三档（Q1232）。
- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过——既有快照的变动只有缩放方式那一句说明（`config` 两屏与 `config-*`、`running-2-fit-*` 那十四串）与「差异大的页」那三处（`envelope-deciding` 两屏、`envelope-deciding-v` 一串），共 38 份网格；
  `manifest.json` 添 8 串；新文件是那 8 串各三份。设计稿 `wrap` 补的那一半没让任何既有快照动一格（补之前后各重导一遍，变动的文件是同一批）。node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 全过。
- **用例**：
  - `shell`：新 `an_outlier_goes_by_one_name_on_the_overview_the_decision_bar_and_the_volume_row`——`envelope`、`envelope-deciding` 两景各两种尺寸，屏上不许有「与其他页差异大」，每一处「差异大」后面都得跟着「的页」（反着钉）。
  - `config`：新 `every_value_on_a_ring_is_spelt_in_its_description_as_the_ring_spells_it`（环上每一格都在说明里从一句的句首点名，单字的 `裁` 不算进 `不裁`）、
    `the_bit_depth_ring_is_the_whole_set_and_its_description_says_so`（环上那四格就是 `BitDepth::ALL`；说明逐档有「`Nbit` = M 级灰」、说「派不上用场」、不许再说「三档」）。
  - `terminal`：新 `every_ring_spells_its_values_in_its_description_on_the_details_pane`（那 8 串比整屏，停在那一项的详情栏上）。
- **`Item::about`、`about_setting`、`about_premise` 上那句 `expect(dead_code)` 删掉**：`session` 只在 `any(tui, test)` 下编译，不带终端库那一趟只有用例那一份，而用例现在读它们（`session` 模块文档《终端库在哪一半》：放松只挂在那一趟「没人读」的那几处）。
- **`CONTEXT.md`**：没动。《差异大的页》《灰阶档位》（全集 {1,2,4,8}）《折行》（起首记号不落行尾）现在屏上都成立。

### 按反跑过的几遍（每一遍改一处、跑 `cargo test --bin tonefit <过滤>`、还原，还原后 `git diff --stat` 核过）

| 按反 | 结果 |
|---|---|
| 实现之前（设计稿还没改，新用例先落） | 红：`an_outlier_goes_by_one_name…`（屏上有「与其他页差异大」）；`every_value_on_a_ring…`（缩放方式的说明里没有 `height`）；`the_bit_depth_ring…`（说明里没有「8bit = 256 级灰」——同一条里「环上那四格就是全集」那一句已经绿，环本来就四格） |
| 设计稿的 `wrap` 补那一半之前（实现已照新说明改） | 红：`config.80x24` 第 9 行第 74 格，期望「（」落在行尾，实现照 `crate::wrap` 挪到下一行 |
| 环上拿掉 `8bit`（`next_bit_depth` 里 `Four => One`） | 红：`the_bit_depth_ring…`（「环上那几格就是全集」）与 `config-depth` 那一串 |
| 裁白边的说明去掉打头的 `裁：` | 红：`every_value_on_a_ring…`（只查「有这几个字」的那一版在同一处照绿，评审之后收紧的） |

### 数

评审收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-09.gate1.log`、`dp-09.gate2.log`、`dp-09.gate3.log`、`dp-09.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1110 通过 1 失败**；lib 252 / bin 459（新添 4 条）；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 59.43s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **975 通过 1 失败**；lib 252 / bin 324（新添 `config` 那 2 条）；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 82.50s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 211fa74`（未提交的工作树）加新文件。

**收下的**：

- **三处 `cfg_attr(all(not(tui), not(test)), expect(dead_code))` 永远不生效**（Standards，`session` 模块文档《终端库在哪一半》）：整句删掉。
- **单字的取值格用 `contains` 查等于没查**（Standards）：改成句首点名（`names`），按反一遍见红。
- **`notable_word` 的文档还写「两处读同一份」**（Standards，CLAUDE.md《文档写作》第 1 条）：写上总览与确认条。
- **8bit 那一句写成无条件的「选了它就会被拒绝」**（Spec：可见灰阶数填到 256 时不拒）：设计稿与实现同改成「可见灰阶数不到 256 时选它」，重导只动 `config-depth` 一串。
- **新 helper `notable` 与 `render::notable` 同名**（Standards，Mysterious Name）：改叫 `notable_bits`（与 `shell::list` 那一个同一个说法）。
- **设计稿注释写「标点」**（Standards，词汇）：改成 CONTEXT《折行》的「记号」。
- **导出器 `...'j'.repeat(1)`**（Standards）：收成 `onItem(n)`，产物一个字节没变。

**驳回的**：

- **Q1227 过不了「翻过来只在这张票之内」**（Standards）：协调人点名要一条条目说清库那一句改没改，照记。
- **`notable_word` 对差异大的页恒是 `Some`，该 `expect`**（Standards）：卷行行尾（`shell::list`）是同一副 `filter_map`，几处读法一致；它回 `None` 的只有坏页，那一样本来就有自己那一格。
- **每页结果原因那一列的「差异大，单独判断」没被新用例扫到**（Spec）：那是库的一句理由，留着（Q1227）；用例文档写明只查总览、确认条与卷列表那几屏。
