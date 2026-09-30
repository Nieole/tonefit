# 06 — 滚轮与方向键跟设计稿

**What to build:** 滚轮在「全部按键」那一张上滚它，补全框开着时挪候选（设计稿在这两处都照滚）。终端层把 `←`／`→` 翻成
`h`／`l`（设计稿在键的归一化上就这么做）——按键表不添行，屏底与全部按键不多一种写法；`⇧⇥` 照旧不翻。
设计稿本来就支持，导出几串新序列钉住。

**Blocked by:** 01

**Status:** resolved

- [x] 按键表上滚轮那一行派得到全部按键那一张与补全框
- [x] 终端层 `←`／`→` 翻成 `h`／`l`；键码翻译那条纯函数用例扩上它们，`⇧⇥` 仍答不认得
- [x] 导出几串并比整屏且绿：全部按键那一张上滚、补全框上滚、方向键在卷列表与每页结果上各一串
- [x] 屏底与全部按键那一张一格不变（不出现方向键的写法）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q979 — 掀着全部按键那一张、或补全框开着时，滚轮不认；设计稿 `moveBy` 在这两处照滚

- **From:** 票 `session-redesign/16`
- **Kind:** 设计稿与按键表对不上
- **Where:** `src/session/keymap.rs` 里「滚轮」「单击」两行标 `UNCOVERED`（输入行与覆盖层都不在内）；`design.html` 的 `moveBy`（`S.overlay.kind === 'help'` 与 `S.input.cands` 两支）
- **Why it did not block:** 那两行是先前的票定下的；没有一串序列在覆盖层或补全框上滚
- **What this ticket actually did:** 照按键表：这两处滚轮原地放过（`deed_of` 认不出）
- **Options:** ① 照旧 ② 那一行放宽到 `UNCOVERED_OR_OVERLAY` 加输入行，终端层 `Deed::Wheel` 那一支在覆盖层上走 `Views::scroll_cover`、在输入行上走 `InputLine::step`——表上一格加终端层两支
- **Recommend:** ②——全部按键那一张比一屏长，触控板用户会先去滚它
- **Whose call:** 拍板的人
- **处置：** 待处理。

#### Q967 — `←` `→` `⇧⇥` 三个键不再翻译：按键表上它们本来就没有主

- **From:** 票 `session-redesign/15`
- **Kind:** 实现决定（撤回代价是改一处）
- **Where:** `src/session/state.rs` 的 `Key`（删了 `Left`、`Right`、`BackTab`），
  `src/session/terminal.rs` 的 `translate` 与 `the_key_codes_the_session_answers_to`
- **Why it did not block:** 新界面按键表（`src/session/keymap.rs`）一行都没绑这三个键——左右是 `h`／`l`，
  它们翻过去也只落在「认不出，原地不动」上。删掉之后按下去的结果与从前逐字相同：什么都不发生。
  留着它们，闸门第二条那一趟就报「从没构造过」。
- **What this ticket actually did:** 三个变体删掉，`translate` 对这三个键码答 `None`，用例改问它们原地放过。
- **Options:** ① 就这样；② 按键表给 `←`／`→` 各添一行，与 `h`／`l` 同派一件事（设计稿是 vim 风格，
  但方向键对不熟 vim 的人是第一反应）
- **Recommend:** ②，归设计稿那一头先拍：屏底与全部按键会多出一种写法，快照要跟着重导。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 待处理。

## 落地记录

**本票做了什么。** 设计稿本来就支持，一行没动；导出器添六串、重导，再让实现跟上。结转的 Q979、Q967 照票面上方的做法收了。

| 处 | 设计稿（`.scratch/session-redesign/design.html`） | 实现 |
|---|---|---|
| 滚轮在哪一块上认 | `onWheel` → `moveBy(3n)`：覆盖层是全部按键那一张就滚它；输入行有候选就挪候选；否则一路落到配置视图／每页结果／`listGo` | 按键表滚轮那一行的块从 `UNCOVERED` 放宽到 `ANY_BLOCK`；做哪一件照 `moveBy` 的次序分，出处是 `Session::wheel` 的《滚轮在哪一块上做哪一件》 |
| 全部按键那一张上滚 | `S.overlay.from += 3n`，`drawHelp` 每一帧收 | 终端层滚轮那一支先问 `Views::wheel_cover`（那一张有几行要窗口的尺寸），一格三行、收在摆得下的那一段里；说明卡与没掀着交回 `false` |
| 补全框上滚 | `S.input.ci` 夹在两头，`fillCand` | `Session::wheel`：补全框有候选时 `InputLine::step(3n)`（与 `↓`／`↑` 同一处） |
| `←`／`→` | `keyName` 翻成 `h`／`l`；`onKey` 打字那一支认原来那个键（`raw.startsWith('Arrow')`）、原地放过 | 终端层 `translate` 交出 `Input::Arrow('h'／'l')`；`Input::chord` 按那个字母查表（表不添行）；`deed_of` 打字那一支问输入本身，方向键不落成字 |
| `⇧⇥` | `keyName` 不看 Shift，那边它就是 `Tab` | 照票面不翻：`translate` 答 `None` |

- **一格几行**：`WHEEL_ROWS` 仍是私有常数，卷列表、补全框、全部按键那一张都经 `view::wheel_rows` 取。
- **导出**：`export.js` 添六串——`help-narrow-wheel-down`（80×24 上那一张滚到 `4–23`）、`add-narrow-F1-wheel-down`（`F1` 掀开的那一张盖着补全框，滚的是那一张）、
  `add-wheel-down`（候选 1 of 4 → 4 of 4）、`ended-ArrowLeft-ArrowRight`、`pages-ArrowLeft`、`fresh-o-ArrowLeft-ArrowRight`（缓冲仍是 `~/`）。
  重导之后 `git diff --stat -- tests/fixtures/design` 读过：只有 `manifest.json` 多 151 行，既有快照、场景数据、期望屏一份没动；十八个新文件。
  三串方向键的 `.text.txt`／`.style.txt`／`.scene.json` 与 `ended-h-l`、`pages-h`、`fresh-o` 逐字节相同。
  node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。
- **用例**：
  - `keymap`：新 `the_wheel_is_dealt_on_every_block`（`Focus::every()` 每一块上都派滚轮，单击照旧不派到覆盖层与输入行）、
    `the_left_and_right_arrows_have_no_row_of_their_own`（表上没有一行写着 `←`／`→`）。
  - `cover`：新 `the_wheel_scrolls_the_sheet_three_lines_a_notch`（三行一格、连滚一次挪完、两头收住、说明卡与没掀着交回 `false`）。
  - `typing`：新 `the_wheel_steps_through_the_completion_box`（八个候选：3 → 6 → 到底 7 → 连滚两格回 1 → 到头 0，底下光标不动；
    补全框没开时落到底下那张列表，Q1107）；`while_typing_every_character_…` 添一句：`Input::Arrow` 在输入行上什么都不派，按下 `h` 照旧是字。
  - `terminal`：`the_key_codes_the_session_answers_to` 扩上 `←`／`→`（`Input::Arrow`），`⇧⇥` 仍答 `None`；
    新 `the_wheel_scrolls_the_key_sheet_and_steps_through_the_completion_box`（三串滚轮比整屏，`F1` 那一串底下的候选仍在头一个）、
    `the_left_and_right_arrows_do_what_h_and_l_do_and_type_nothing`（三串方向键比整屏；先钉住清单上的 `ArrowLeft`／`ArrowRight`
    经夹具交给会话的，与 `translate_input` 把终端那两个键码翻出来的是同一个输入）。
  - `scene`：`key_named` 认 `ArrowLeft`／`ArrowRight`，`every_sequence_in_the_manifest_reads_into_steps` 添两句。
- **屏底与全部按键一格不变**：全部按键那一张按阶段列、不问块，滚轮那一行块变了它一格不动；既有各阶段的全部按键与屏底快照没重导出一格变化、照旧比整屏过；
  表上没有方向键的行由上面那条 `keymap` 用例钉着。
- **`CONTEXT.md` 没改**：《覆盖层》只举 `j`／`k` 滚得动、还写着「别的键一律不派」，滚轮没写进去——改已有词条要先拍板，记 Q1110。

### 按反跑过的几遍（前两遍是实现落地之后按反、改完还原；后两遍是写实现之前那一遍红）

| 按反 | 结果 |
|---|---|
| 滚轮那一行只加了块、终端层不先问 `wheel_cover`（滚轮一律交给 `Session::wheel`） | 红 1 条：`the_wheel_scrolls_the_key_sheet_…`，「help-narrow-wheel-down 第 2 行第 5 格」——那一张没滚，底下的列表挪了 |
| `deed_of` 打字那一支把 `Input::Arrow` 也当字 | 红 2 条：`the_left_and_right_arrows_do_what_h_and_l_do_…`（fresh-o-ArrowLeft-ArrowRight，缓冲成了 `~/hl`）、`while_typing_every_character_…` |
| 补全框滚轮那一支还没写（`Session::wheel` 只挪光标） | 红：`the_wheel_steps_through_the_completion_box`，候选停在 0 |
| 滚轮那一行块仍是 `UNCOVERED` | 红：`the_wheel_is_dealt_on_every_block`，「Input(AddPath) 上的滚轮」 |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff fad1d49`（未提交的工作树）。Spec 轴：与设计稿逐处一致，没有阻塞项。

**收下的**：

- **补全框那条用例一格三个与到底为止分不开**（Standards，testing.md「一条用例里同一个数只有一个出处」）：四个候选时末一个下标恰是 3。换成八个候选重写。
- **注释说「哪一块做哪一件归终端层」**（Standards）：不成立——补全框与底下那一块在 `Session::wheel` 里分。那条规矩收进 `Session::wheel` 的
  《滚轮在哪一块上做哪一件》一处，表上、终端层、`wheel_cover` 与用例的注释都只指到那一节；方向键那条规矩同样只在 `Input::Arrow` 写全（单一出处）。
- **`⇧⇥`「设计稿也没有」**（Standards）：不成立，设计稿里它就是 `Tab`。注释改成「照票面不翻（Q967），设计稿的 `keyName` 不看 Shift」。
- **`WHEEL_ROWS * isize::from(notches)` 写了两遍、常数为此敞成 `pub`**（Standards，Duplicated Code）：抽成 `view::wheel_rows`，常数退回私有。
- **《覆盖层》没写滚轮**（两轴都点了）：记 Q1110；Q1109 的 Where 补上那一句。
- **Q1108 的 Where 写了变更史、Q1108／Q1109 缺 Why it matters**（Standards）：改成当前事实，补上；Q1108 的 Options 补上「方向带一个两值类型」那一条（Primitive Obsession 那一问）。
- **输入行没有候选、说明卡上滚轮落到底下那一块，spec 转来的 Q979 ② 读作「不动」**（Spec，越界）：照设计稿走（ADR 0019 第 13 条），两种读法写进 Q1107 的 Why it did not block，Q1107 推荐拍板的人改成不动。

**驳回的**：

- **`wheel_cover` 与 `scroll_cover` 同形**（Standards，Duplicated Code）：一个按表上的一件事挪、一个按输入上带着的格数挪，并成一支就得让 `scroll_cover` 收一个只有滚轮用得着的数。
- **`Input::Arrow(char)` 只有 `h`／`l` 合法**（Standards，Primitive Obsession）：换成两值的方向，字母就得在会话那一层认，与票面「终端层翻成 `h`／`l`」相反；记进 Q1108 的 ④。
- **「补全框开着」内联成 `!candidates.is_empty()`**（Standards，Feature Envy）：画补全框那一处（`shell::completions`）问的是同一句，现成的写法，不为这一处另起名字。
- **清单键名到输入的映射写了两份**（Spec）：夹具认浏览器的键名、终端层认 crossterm 的键码，两份各认各的来源；相等由一条用例钉住。

### 停车场

本票用了 Q1107–Q1116 里的四个：

- **Q1107**：输入行开着而补全框没开时滚轮落到底下那一块（设计稿 `moveBy` 一路落下来）；推荐改成不动。
- **Q1108**：`←`／`→` 交出 `Input::Arrow`，不是 `Key::Char`——输入行上不是字。
- **Q1109**：说明卡上滚轮落到卡底下那张列表（与 `j`／`k` 一样）。
- **Q1110**：《覆盖层》没写滚轮。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-06.gate1.log`、`dp-06.gate2.log`、`dp-06.gate3.log`、`dp-06.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
照前几票的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1050 通过 1 失败**；lib 239 / bin 434；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.87s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **932 通过 1 失败**；lib 239 / bin 316；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.57s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.17s` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；四项全绿，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `fad1d49`：闸门 1 **1044 通过 1 失败**（lib 239 / bin 428），闸门 2 **928 通过 1 失败**（lib 239 / bin 312），红的是同一条
——按 `design-parity/02`（1040／924）与 `one-source/02`（两趟各多 4 条，都在 `tests/` 底下）两票记下的数推得，没在 `fad1d49` 上重跑。
**闸门 1 多 6 条、闸门 2 多 4 条，都在预期里**：`the_wheel_is_dealt_on_every_block`、`the_left_and_right_arrows_have_no_row_of_their_own`（keymap）、
`the_wheel_scrolls_the_sheet_three_lines_a_notch`（cover）、`the_wheel_steps_through_the_completion_box`（typing）两趟都编；
`the_wheel_scrolls_the_key_sheet_…`、`the_left_and_right_arrows_do_what_h_and_l_do_…`（terminal）只在默认那一趟。

**黄金快照逐格没动**：`git diff fad1d49 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。
