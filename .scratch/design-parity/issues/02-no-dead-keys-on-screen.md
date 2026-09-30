# 02 — 屏上不摆按不动的键，按得动的都摆

**What to build:** 按键表多一维**输入行的用途**：添加与修改处理路径、改输出目录、改一项设置的值、给预设起名、搜索。
`Tab` 补全只在路径那三种用途上派得出，`C-w` 删一段在每一种上都派得出。屏底右端那几件从表上一路派下来——
今天为搜索那一行手摆两件的那道岔路删掉。预设栏光标停在末行「＋ 把当前设置保存为预设」上时屏底不摆 `dd`。

设计稿跟着改：右端那几件按用途摆，搜索那一行摆 `C-w`，预设栏末行不摆 `dd`、在那一行上按 `dd` 不再抛错。

**Blocked by:** 01

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 改一项设置的值、给预设起名两种输入行屏底右端不摆 `Tab`；三种路径输入行照旧摆
- [x] 搜索那一行屏底右端摆 `C-w`
- [x] 预设栏光标在末行上时屏底不摆 `dd`，停在一份预设上照旧摆
- [x] 「屏底每一件都是按键表的一行」「全部按键与屏底出自同一张表」两条用例扩上用途这一维：每一种用途上屏底右端恰好是那一种上派得出的键
- [x] 画法里不再有按「是不是搜索那一行」手摆屏底的岔路
- [x] `CONTEXT.md`《输入行》改成右端那几件按用途摆（`⇥` 只在路径那几种上）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q794 — 输入行右端 `[Tab → 补全]` 按表只在还没开始那一档摆（`Complete` 那一行标 `I`），设计稿凡是不是搜索的输入行都摆它——改一项设置的值、给预设起名也摆，而那几种 `Tab` 什么都不做

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿自己前后不一（`KEYMAP` 的「输入」组把 `Tab 补全路径` 标成只有还没开始才派得出，`drawFooter` 却按输入行的种类摆），13／14 号票会撞
- **Where:** `design.html` 的 `drawFooter` 打字那一支（`inp.kind === 'search' ? … : [Tab, C-w, ⏎, Esc]`）与 `complete`（`['add','edit','out']` 之外直接返回）；`src/session/view.rs` 的 `hints` 输入行那一支（问表：`Complete` 只在 `FRESH`）
- **Why it did not block:** 本票的输入行都在还没开始那一档，两边都摆；改一项设置的值那几串（`config-levels-i`）起点也是还没开始，导出的屏上两边一样
- **What this ticket actually did:** 屏底右端那几件从表上取，派不出的不摆；表上没有「输入行用在哪件事上」这一维
- **Options:** ① 设计稿右端那几件按输入行的种类摆：值与预设名不摆 `Tab`（`complete()` 对它们本来就什么都不做），`KEYMAP` 那一行照旧；② 表上多一维「输入行的用途」，`Complete` 只在路径那几种上派——表因此比设计稿多一维
- **Recommend:** ①（屏上不摆按不动的键）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

#### Q827 — Q794 那条在这张票上**不成立冲突**：值那一种输入行的起点是「还没开始」，两边都摆 `Tab`

- **From:** 票 `session-redesign/13`
- **Kind:** 路过发现
- **Where:** `tests/fixtures/design/sequences/config-levels-i.*`（屏底右端 `[Tab → 补全]`）；
  `src/session/keymap.rs` 的 `Complete` 那一行（`FRESH`、`INPUT`）；`src/session/typing.rs` 的 `Purpose::completes`
- **Why it did not block:** Q794 说的是「表按阶段派 `Tab`，设计稿按输入行的种类摆」——两边会撞。
  本票核下来：**这一串的起点是还没开始那一档**，表上 `Complete` 正好派得出，屏底两边一字不差，
  逐格比对因此绿着。要改的是设计稿（Q794 的处置 ①），不是实现。
- **What this ticket actually did:** 屏底右端照旧从表上取；另给 `Purpose` 加一条 `completes()`
  ——改一项设置的值那一种按下 `Tab` 一个字都不动（设计稿 `complete()` 对非路径种类本来就直接返回）。
  **屏上摆着一个按下去什么都不做的键**，那一半仍然活着，处置在 Q794。
- **Options:** ① 照 Q794 的处置 ① 改设计稿，那几串重新导出，屏底右端不再摆 `Tab`；
  ② 表上多一维「输入行的用途」
- **Recommend:** ①（与 Q794 同一条；本条只是把「这张票上为什么没红」写下来）
- **Whose call:** 拍板的人（同 Q794）
- **处置：** 待处理。

#### Q868 — 搜索那一行右端只摆两件，而 `C-w` 在它上面照样派得出（Q794 的续）

- **From:** 票 `session-redesign/09`
- **Kind:** 票面没想到的第三种情形（**Q794 的续**：同一处「屏底与按键表对不上」，这一票多出一格）
- **Where:** `src/session/view.rs` 的 `Session::hints` 输入行那一支（新添的那道岔路）；表上 `Deed::DeleteWord` 那一行的阶段是 `NOT_SURVEYING`、块是 `INPUT`
- **Why it did not block:** 设计稿 `drawFooter` 打字那一支写死了两副右端：搜索那一种是 `⏎ → 跳到结果` 加 `Esc → 取消`，别的是 `Tab`／`C-w`／`⏎`／`Esc` 四件。`search.120x36` 那一屏因此只摆两件——而 `C-w` 在搜索那一行上**按得动**（设计稿 `onKey` 那一支不分种类）。「屏上不摆按不动的键」那条规矩这里反过来了：**按得动的键屏上没摆**。Q794 记着由来——表上没有「输入行用在哪件事上」那一维
- **What this ticket actually did:** 照设计稿逐格：`hints` 里按 `Session::searching_line()` 分一道岔路，搜索那一行只摆 `⏎`（点名「跳到结果」那一句）与 `Esc`。**`Tab` 不摆是表自己拦的**（`Complete` 只在还没开始那一档派得出，且 `Purpose::Search` 的 `completes()` 答 `false`）；**`C-w` 不摆是这道岔路拦的**——它是这一票新添的第二格「屏底不从表上一路派下来」
- **Options:** ① 照现状：两副右端各手摆一次，理由写在那道岔路上 ② 给按键表加「输入行用在哪件事上」那一维（Q794 的根治）：`Complete` 与 `DeleteWord` 两行各标上它派得出的那几种用途，屏底重新一路从表上派下来 ③ 让 `C-w` 在搜索那一行上真的不派（那样规矩顺了，但删一段是个好用的键，而设计稿明写着它派得出）
- **Recommend:** ②，与 Q794 一并了结；③ 不取（那是为了对齐规矩去砍一个好用的键）
- **Whose call:** 协调人（Q794 就在他那里挂着）
- **处置：** 待处理。

#### Q893 — 预设栏上还剩两个按不动的键：末行那一件上的 `dd`，与起名那一行右端的 `Tab`

- **From:** 票 `session-redesign/14`
- **Kind:** 票面没想到的第三种情形（而且设计稿在这一格上会崩）
- **Where:** 设计稿 `design.html` 的 `footerHints` 预设那一支
  （`if (S.cfg.presets) return [hint('⏎','使用'), hint('dd','删除'), …]`，**不问光标停在哪一行**）
  与 `deleteHere`（`PRESETS.splice(S.cfg.pcursor, 1)` 在 `pcursor === PRESETS.length` 上
  取出 `undefined`，下一句 `gone.name` 当场抛）；实现那一侧是
  `src/session/terminal.rs` 的 `erase_a_preset` 与 `src/session/view.rs` 的 `ask_then_erase`
- **Why it did not block:** 屏底那一行有序列钉着的只有**光标停在一份预设上**那几串
  （`config-p`、`config-p-dd`、`config-p-dd-dd` 都停在第 0 行）；光标停在末行那一件上的两串
  （`config-p-save`、`config-p-save-named`）屏底让给了输入行与回话，`[dd → 删除]` 一次都没露面。
  因此两条路（照设计稿无条件摆、照「屏上不摆按不动的键」按行摆）在夹具上分不出来。
- **What this ticket actually did:** **屏底照设计稿无条件摆**（逐格比对要的就是它），
  而 `dd` 落在末行那一件上时**一件事都不做**（不闩、不删、不说话）——不照设计稿崩。
  于是留下一个小口子：光标停在「＋ 把当前设置保存为预设」那一行上时，
  屏底摆着一个按下去什么都不发生的 `[dd → 删除]`，与仓库那条**「屏上不摆按不动的键」**
  正面冲突（同一条在 08 的 `l → 每页结果` 上是按行摆的：`Session::open_want`）。
- **Options:** ① 照本票：屏底无条件摆，末行上按不动；
  ② 照「屏上不摆按不动的键」：`Session::hints` 的预设栏那一支问一句光标停在哪一行，
  末行上不摆 `dd`，设计稿的 `footerHints` 跟着改、重新导出（那几串屏底那一行眼下不受影响，
  因为它们都停在一份预设上）；③ 让末行也删得掉：说不通——那一行不是一份预设
- **另一个按不动的键，同一条毛病、另一个主**：起名那一行右端摆着 `[Tab → 补全]`
  （`config-p-save` 那一串的屏底钉着它），而 `typing::Purpose::completes` 里没有
  「给预设起名」那一种——按下去一个字都不动。**那不是本条新添的**：改一项设置的值那一种
  早就是这样（Q794，连同 13 记的 Q827），本票只是让它多了第三种输入行。收法与那两条同一条：
  屏底右端那几件按表摆，而表上没有「输入行用在哪件事上」那一维。
- **Recommend:** ②（与 `open_want` 同一条形状）。`Tab` 那一个照 Q794 的处置 ① 一起收。
  两样落地都要改设计稿，因此归拍板的人。
- **Whose call:** 拍板的人（动设计稿的 `footerHints`；`Tab` 那一半同 Q794）
- **处置：** 待处理。

## 落地记录

**本票做了什么。** 设计稿先改、重导，再让实现跟上：按键表多出「输入行的用途」那一格，屏底右端从表上一路派下来，
搜索那一行那道手摆的岔路删掉；预设栏末行上不摆 `dd`。结转的四条（Q794、Q827、Q868、Q893）照票面上方的做法都收了。

| 处 | 设计稿（`.scratch/session-redesign/design.html`） | 实现 |
|---|---|---|
| 输入行右端 | `drawFooter` 打字那一支只剩一个次序 `Tab`·`C-w`·`⏎`·`Esc`，按用途筛：`Tab` 只在 `COMPLETES`（`add`／`edit`／`out`，`complete()` 读同一份）上，`⏎` 在搜索那一行上说「跳到结果」 | `Focus::Input(Use)`：打着字时焦点带上这一行的用途（`typing::Use`，`Purpose` 去掉对象那一格）；表上 `Tab` 的块是 `PATH_LINES`，`⏎ → 确定`／`⏎ → 跳到结果` 分给 `NOT_SEARCH_LINE`／`SEARCH_LINE`，`C-w`、`Esc`、`⌫`、`F1` 是全部六种；`Session::hints` 输入行那一支只剩一支、一种用途都不问 |
| 预设栏末行的屏底 | `footerHints` 预设那一支：`pcursor < PRESETS.length` 才摆 `dd` | `Session::hints` 预设栏那一支：`picked()` 有一份才要 `DeletePreset`（与 `open_want` 同形） |
| 末行上按 `dd` | `deleteHere`：停在末行上直接返回（原先 `PRESETS.splice` 取出 `undefined`、下一句 `.name` 抛错） | 本来就不做事（`ask_then_erase` 答 `None`），没改 |

- **`Purpose::completes` 删了**：哪几种补得出只在表上答，`complete_typed` 头上那道门一并删（Q1089）。
- **`CONTEXT.md`《输入行》**：右端那几件按用途摆；添了「输入行的**用途 (Use)**」一词，注明与全部按键「按用途分组」不是一回事（Q1091）。
- **导出**：`export.js` 添两串 `config-p-G`（`G` 停到末行）、`config-p-G-dd`（末行上再按 `dd`，走完与前一串逐格相同）。
  重导之后 `git diff --stat -- tests/fixtures/design` 读过：网格变了的恰是 `config-levels-i`、`config-p-save`、`running-slash`、
  `running-search-typed`、`search.120x36`、`search.80x24` 这六份（各只动末行），加 `manifest.json` 那两串与六个新文件；场景数据一份没变。
  node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。
- **用例**：
  - `keymap`：新 `the_input_line_deals_its_keys_by_what_it_is_used_for`（六种用途上 `Tab`／`C-w`／`⏎` 各派什么）；
    `no_key_is_dealt_to_two_deeds_in_the_same_place` 改从 `Focus::every()` 取块（六种输入行各算一格）。
  - `view`：`the_search_line_offers_only_jump_and_cancel` 换成 `the_input_line_offers_what_its_purpose_deals`（六种用途右端逐件点名，整列相等——值与预设名那两种里没有 `Tab`）；
    `every_hint_on_the_footer_is_a_row_of_the_key_table` 扩上用途：每一档、每一种用途上右端**恰好**是「输入」那一组派得出、上得了屏底的那几行；
    新 `the_picker_offers_dd_only_on_a_preset`（末行上整列相等，没有 `dd`）。
  - `cover`：`the_sheet_and_the_footer_both_come_from_the_key_table` 的块改从 `Focus::every()` 取，另添反向那一句（一处派不出的那一句屏底问它摆不出来）。
  - `typing`：新 `tab_is_not_dealt_on_a_line_that_is_not_a_path`（值与预设名在还没开始、已结束，搜索在清点完之后三档，`Tab` 都派不出、`C-w` 派得出）。
  - `terminal`：新 `dd_on_the_save_row_is_not_offered_and_does_nothing`（`config-p-G`、`config-p-G-dd` 比整屏，没闩、盘没动）；
    `running-slash`、`running-search-typed`、`config-levels-i`、`config-p-save` 与搜索那一景照新导出的屏比整屏。
- **Q1061 那一屏**：「添加路径」是路径那一种，右端照旧四件，80 列上缓冲仍被盖住——本票没把它松开。搜索那一行右端多了 `C-w`，
  80 列上起点从第 52 格挪到第 37 格，搜索词宽过 35 格也会被盖住，记 Q1092（推荐与 Q1061 一起定）。

### 按反跑过的几遍（改完都还原了）

| 按反 | 结果 |
|---|---|
| `Session::hints` 放回搜索那一行的手摆岔路 | 红 4 条：`the_search_scene_matches_…`（「search.120x36 第 35 行第 77 格 实际「 」期望「[」」）、`slash_opens_…`（running-slash 同一格）、`every_hint_on_the_footer_…`、`the_input_line_offers_…` |
| 表上 `Tab` 的块放回全部六种（`INPUT`） | 红 4 条：`the_input_line_deals_…`、`the_input_line_offers_…`、`i_edits_a_filled_in_setting_…`（「config-levels-i 第 35 行第 68 格 实际「[」期望「 」」）、`saving_a_named_preset_…`（config-p-save 同一格）；`tab_is_not_dealt_…` 头一版只问已结束那一档、没红（`Tab` 本来只在还没开始派），改成按各用途开得起来的几档问之后红在「Fresh 的 Setting(GrayLevels)」 |
| 预设栏那一支 `dd` 无条件摆 | 红：`dd_on_the_save_row_…`，「config-p-G 第 35 行第 13 格 实际「d」期望「p」」；`the_picker_offers_dd_only_on_a_preset` 同红 |
| `keymap::hints` 在输入行上不问派不派得出 | 红：`the_sheet_and_the_footer_…`，「Fresh 的 Input(AddPath) 上派不出的「1 → 任务」摆上了屏底」 |
| 设计稿 `deleteHere` 拿掉末行那道守卫 | `node export.js --show config-p-G-dd` 抛 `TypeError: Cannot read properties of undefined (reading 'name')` |
| 设计稿 `footerHints` 末行上照旧摆 `dd` | `--show config-p-G` 末行是 `[⏎ → 使用] [dd → 删除] …`，`export` 的 `lacks` 当场拦下 |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 8b756fa`（未提交的工作树）。

**收下的**：

- **按反那一遍没写**（Standards，testing.md 第一条）：写进上一节。
- **「用途」一词撞车**（Standards）：《覆盖层》「按用途分组」与 `Group` 已占着这个词。词条与模块文档都改成「输入行的用途」并注明不是一回事，记 Q1091。
- **`Purpose` 与 `Use` 名字分不出**（Standards、Spec）：`Purpose` 的文档写明「连同那件事的对象」，`Use` 的写明是它去掉对象那一格；另一条路记 Q1091。
- **同一条规矩写了五处**（Standards，单一出处）：`Focus::Input`、`hints`、`complete_typed`、表上几个常量的注释改成只指 `keymap` 的《输入行的用途》，那一节写明它是代码里的唯一出处。
- **Q1087 的 Where 漏了《焦点》《按键表》**（Standards）：补上。
- **`config-p-G` 的自检正着钉了 `[⏎ → 使用]`**（Standards）：那正是 Q1090 说的那句，自检换成 `[p → 返回]`，重导（只动 `manifest.json` 那一格）。
- **词条开头仍无条件写着右端四件**（Spec）：《输入行》那一句改成「右端是这一行上按得动的那几件，按用途摆」，四件各说摆在哪几种上。
- **搜索那一行右端变宽、80 列上会盖住长搜索词**（Spec）：记 Q1092。
- **末行上 `d` 仍亮待续记号**（Spec）：记 Q1093（Q779 定的规矩管着它）。
- **cover 那条反向断言问的是 `keymap::hints`，不是 `Session::hints`**（Spec）：文档写明这一条问的是表与 `hints` 那一层、「恰好」在 `view` 那一条；
  反向那一句留着——按反那一遍它咬得住（`hints` 不问派不派得出时红）。

**驳回的**：

- **`INPUT`、`NOT_SEARCH_LINE` 手列一遍 `Use` 的取值**（Standards，Duplicated Code）：表上的块集照 `UNCOVERED`、`SELECTING` 那几组一样写出来，一眼读得出一行派在哪几处；
  漏列一种的话 `the_input_line_deals_…` 在 `C-w` 那一句上红。
- **`searching_line`、`naming_a_preset` 仍比对 `Purpose`**（Standards，Repeated Switches）：本票没碰它们，它们答的是终端层把 `⏎` 分给谁，不是屏底。
- **删 `Purpose::completes`、搜索那一行的次序是票面没要的**（Spec）：两处都是本票必须走的岔口，各记 Q1089、Q1088。

### 停车场

本票用了 Q1087–Q1096 里的七个：

- **Q1087**：用途挂在焦点上（`Focus::Input(Use)`），不在表上另起一列。
- **Q1088**：搜索那一行右端 `C-w` 排在 `⏎` 之前，与别的输入行同一个次序。
- **Q1089**：`Purpose::completes` 删了，`Tab` 在哪几种上补得出只在表上答。
- **Q1090**：预设栏末行上屏底仍写 `⏎ → 使用`（那一行上 `⏎` 开的是起名）。
- **Q1091**：「用途」两个类型，`Use` 与 `Purpose`。
- **Q1092**：搜索那一行右端变宽，80 列上长搜索词被盖住（Q1061 同一个毛病）。
- **Q1093**：末行上按 `d` 仍亮待续记号。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-02.gate1.log`、`dp-02.gate2.log`、`dp-02.gate3.log`、`dp-02.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
照前几票的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1040 通过 1 失败**；lib 239 / bin 428；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.38s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **924 通过 1 失败**；lib 239 / bin 312；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.65s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.24s` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；四项全绿，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `8b756fa`：闸门 1 **1036 通过 1 失败**（lib 239 / bin 424），闸门 2 **921 通过 1 失败**（lib 239 / bin 309），红的是同一条。
**闸门 1 多 4 条、闸门 2 多 3 条，都在预期里**：新添 `the_input_line_deals_…`（keymap）、`the_input_line_offers_…`、`the_picker_offers_dd_…`（view）、
`tab_is_not_dealt_…`（typing）两趟都编；`dd_on_the_save_row_…`（terminal）只在默认那一趟；换掉的 `the_search_line_offers_only_jump_and_cancel`（view）两趟各少一条。

**黄金快照逐格没动**：`git diff 8b756fa -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 同为 `2a6aabc0…`；`tests/golden.rs` 2 条、`tests/counters.rs` 14 条两趟都全过。
