# 03 — 屏上句子里提到的键出自按键表

**What to build:** 屏上凡是在一句话里提到一个键的地方，键的写法一律取按键表那个**不问阶段与块**的写法：
每页结果框底边那一件 `a → …`、跳过的卷那一句末尾的 `h → 回卷列表`、一页都不需留意那一句末尾的 `a → 全部页`、
搜索提示的 `n N`、确认条上的四个键。**确认条那四句仍是它自己的措辞**（与屏底那几句回话一样），按键表不为它添列。

再补回一条文件扫描：画法那几个模块的句子里不许手抄键——下一次手抄当场红。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 上面那几处键字面读按键表；屏上一格不变——全部设计快照与交互期望屏照旧绿（同一类另几处一并改了，Q1267）
- [x] `tests/single_source.rs` 补回那条扫描：家是按键表那个模块，读者是画法那几个模块，记号取新界面屏上真出现过的几句；拿一处手抄的键去试，它红
- [x] `CONTEXT.md`《确认条》里 `v` 那一句改成「查看每页结果」，与设计稿、屏上一致
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 红的仍只有 macOS 基线那一条，Q995）

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q874 — 屏上一块**自己那两句提到键的话**仍是手抄的，不出自按键表（Q190 的续）

- **From:** 票 `session-redesign/11`
- **Kind:** 与既有的一条同源（Q190：「还没跑过」那两句里的 `t`／`x` 手抄）
- **Where:** `src/session/shell/pages.rs` 三处：框底边那一件 `a → …`、跳过的卷那一句末尾的 `h → 回卷列表`、一页都不需留意那一句末尾的 `a → 全部页`。既有的同一副在 `src/session/shell/list.rs` 的 `searching_chip`（`n N 跳到下一个 / 上一个`）
- **Why it did not block:** **屏底那一行与全部按键那一张照旧全从按键表派生**（`keymap::hints`），一个字都没手抄；手抄的只有「屏上这一块自己的开关」那几句，而它们**不随阶段改口**——等待确认那一档 `a` 让给答话、屏底不摆它，框底边那一句照样写着（设计稿 `drawPages` 的 `bottomLeft` 与 `deciding-v` 那一串都是这样）。从按阶段过滤的 `keymap::hints` 取，这三处会在等待确认那一档上凭空消失，逐格对不上
- **What this ticket actually did:** 那三句里**会变的那一半**（全部页 / 只看需留意的页）收进了 `Session::listing_key_says` 一处，屏底与框底边读同一份；键那个字面照 `list.rs` 既有的做法手写
- **Options:** ① 照现状 ② 给 `keymap` 添一手**不问阶段与块**的键写法（`spelt(deed)`），这三处与 `searching_chip` 一起改读它 ③ 让这几句走 `keymap::hints`：不取——等待确认那一档上它们会消失
- **Recommend:** ②，与 Q190 一起判：两条要的是同一手
- **Whose call:** 拍板的人（按键表的对外形状）
- **处置：** **本票了结，照票面（原推荐 ②，那一手 `keymap::spelt_for` 已在）**：那三处与 `searching_chip` 的 `n N` 改读它；同一类的 `follow_chip`、屏底三句回话、可见灰阶数长说明一并改（Q1267）。`listing_key_says` 照旧只挑那一句。见《落地记录》。

#### Q896 — 确认条第二行那四句话是这一块自己写的，与按键表撞着车（Q874 的续）

- **From:** 票 `session-redesign/12`
- **Kind:** 一处第二份说法（**Q874 的续**：屏上一块自己那几句提到键的话仍是手抄的）
- **Where:** `src/session/shell/decision.rs` 的 `answers`；对着的是
  `src/session/keymap.rs` 上「确认」那一组四行（`Deed::Write`／`WriteAll`／`End`／`ViewPages`）
  与 `CONTEXT.md` 的《确认条》词条
- **Why it did not block:** 设计稿 `drawDecision` 把那四句连同**分段与颜色**写死在那一行上
  （`x` 的前半截默认色、后半截灰；`v` 整句默认色），而按键表交出来的是「键 + 一句」两截、
  拼不出这个形状。更要紧的是**字面对不上**：`x`／`a`／`s` 三句与表上长的那一句逐字相同，
  而 `v` 那一句**三处三副写法**——按键表是「查看这一卷的每页结果」，设计稿与快照是
  「查看每页结果」，`CONTEXT.md` 的《确认条》写的是「看这一卷的每页结果」。
  照表写，`v` 那一格与设计快照差四格（两个汉字）
- **What this ticket actually did:** 照设计稿逐字写在这一块里，模块文档点名说它是手抄的、
  指着 Q874 与本条。`tests/single_source.rs` 那条不变量按不到它——它记的是从前手抄过的那六句
- **还有第三处**：`src/session/view.rs` 的 `answer_the_point`（屏底那三句回话）。
  **只有 `x` 那一句真撞上**——`写出这一卷` 加 `（不用重新分析）` 与条上、表上逐字相同；
  `a` 与 `s` 两句回话是另外两句话（`全部写出：后面的卷不再询问`、
  `已结束预览：这一卷不写出，后面的卷也不处理`），设计稿 `taskKey` 那一支自己就这么写。
  收的时候三处一起看
- **Options:** ① 照现状 ② 给表上那四行各添一格「条上怎么写」，确认条从表里取
  （按键表添一列的事） ③ 改设计稿让 `v` 那一句与表一致、重导快照
  ——ADR 0019 决定第 13 条，拍板的人的事
- **Recommend:** ②，与 **Q874**／**Q190** 一起收：那时「屏上一块自己那几句提到键的话」
  共有几处，一次看得清
- **Whose call:** 协调人
- **处置：** **本票了结，照票面**：四个键取自按键表，四句仍是确认条自己的措辞，表不添列；`CONTEXT.md`《确认条》`v` 那一句对齐到「查看每页结果」；`answer_the_point` 那三句回话不提键，没动。见《落地记录》。

#### Q963 — 「屏上顺口提到一个键的那几句不许手抄」那条文件扫描随旧界面删掉了，新界面没有同形的一条

- **From:** 票 `session-redesign/15`
- **Kind:** 守门用例随被守的东西一起没了
- **Where:** `tests/single_source.rs` 从前的 `the_keys_the_screen_mentions_come_from_the_key_table`
  （连同 `KEY_SENTENCE_MARKS`、`KEY_HOME`=`src/session/draw/keys.rs`、`KEY_READERS`、`code_only`）
- **Why it did not block:** 它守的六句散文与它点名的家（`draw/keys.rs`）、读者（`draw/*.rs`）都随旧界面删掉了，
  留着只会因文件不在而红。新界面屏上的键一律经按键表拼（`view.rs` 的
  `every_hint_on_the_footer_is_a_row_of_the_key_table`、`cover.rs` 的
  `the_sheet_and_the_footer_both_come_from_the_key_table` 问着屏底与全部按键两处）。
- **What this ticket actually did:** 删掉那条用例与它独用的常量、`code_only`；别的三条单一出处用例一条没动。
- **Options:** ① 就这样——新界面的散文里提键的地方由成屏快照与上面两条守着；
  ② 照原形给新界面补一条：家改成 `src/session/keymap.rs`，读者改成 `shell/*.rs`，
  记号换成新界面屏上真出现过的几句（如「再按一次 ⏎ 覆盖」「再按一次 dd 删除」）
- **Recommend:** ②。成屏快照钉的是「今天的键位」，换键位时快照会随设计稿重导而一起变，
  拦不住「代码里手抄了一个键」；那条扫描拦得住。
- **Whose call:** 下一张改按键表的票
- **处置：** **本票了结，照推荐 ②**：`the_keys_the_screen_mentions_come_from_the_key_table` 回来了，家 `src/session/keymap.rs`，读者画法那几块加 `view.rs`、`config.rs`，记号取新界面屏上真出现过的 20 句；分两段写的那几处靠读者表上的下界（Q1270）。见《落地记录》。

## 落地记录

**本票做了什么。** 屏上一句话里提到的键，一律读按键表那个**不问阶段与块**的写法（`keymap::spelt_for`，已在）；措辞仍是各自那一块的，表一列没添。屏上一格不变，设计稿没碰、没重导。

| 处 | 句子（今天的屏） | 读表上哪一件 |
|---|---|---|
| `shell/pages.rs` 框底边 | `a → 全部页`／`a → 只看需留意的页` | `Deed::ListAll` |
| `shell/pages.rs` 跳过的卷那一句末尾 | `h → 回卷列表` | `Deed::BackToList` |
| `shell/pages.rs` 一页都不需留意那一句末尾 | `a → 全部页` | `Deed::ListAll` |
| `shell/list.rs` `searching_chip` | `n N 跳到下一个 / 上一个` | `Deed::SearchNext`、`Deed::SearchPrev` |
| `shell/decision.rs` `answers` | 确认条四个键（宽窄两副） | `Deed::Write`／`WriteAll`／`End`／`ViewPages`（闭包 `key` 改收 `Deed`） |
| `shell/list.rs` `follow_chip`（Q1267） | `[已暂停自动滚动 ⋅ F 恢复]` | `Deed::Follow` |
| `view.rs` `q` 拒绝退出那一句回话（Q1267） | `q 不会退出，请先按 s 停止，或按 C-c 立即退出` | `Deed::QuitRefused`、`Stop`、`Interrupt` |
| `view.rs` `stop_a_notch`（Q1267） | `做完当前卷就停 ⋅ 再按一次 s 立即停止` | `Deed::Stop` |
| `view.rs` `pause_follow`（Q1267） | `已暂停自动滚动 ⋅ 按 F 恢复` | `Deed::Follow` |
| `config.rs` `about_setting`（Q1267） | 可见灰阶数那一段「按 c 生成灰阶测试图」 | `Deed::Chart`（`Item::about` 因此回 `Cow<'static, str>`） |

- **`CONTEXT.md`《确认条》**：`v` 那一句「看这一卷的每页结果」改成「查看每页结果」，与设计稿、屏上一致（票面勾选项；别的词条没动，《按键表》该不该补一句记 Q1269）。
- **没动的三处**（Q1268）：顶栏视图号 `1`／`2`、搜索那一截与搜索行的 `/`、按键表 `q` 拒绝退出那一行长的那一句——前两样是号与提示符，不是一句话里顺口提的键；末一样在家里。
- **文档**：`keymap::spelt_for` 的文档是这条规矩在代码里的家（不随阶段改口所以不走 `hints`、措辞归各块、表不添列，指到扫描那一条）；`decision.rs` 模块文档《两行各说什么》改成「键取自按键表，四句是这一块自己的措辞」；`pages.rs` 模块文档点名那几句里的键不是这一块的；`picker.rs` 的 `asked` 从前指着《屏底》说「顺口提到的键不手抄」，而《屏底》没有这一句，改指 `keymap::spelt_for`。
- **用例**：
  - `tests/single_source.rs`：补回 `the_keys_the_screen_mentions_come_from_the_key_table`，照那一支已有几条的形状三件事一起问——**代码里**（`code_only`：砍掉头一个就地展开的 `#[cfg(test)] mod … {` 起的整段与注释行；只声明的 `mod scene;` 不砍）没有 `KEY_SENTENCE_MARKS` 那 20 句、家 `src/session/keymap.rs` 里有 `fn spelt_for(`、`KEY_READERS` 那 8 个文件 9 行的下界（下界取实数；`decision.rs` 另数 `key(Deed::` 8 次）。红的时候一个文件连同它抄着的那几句一起报。
  - `src/session/shell.rs`：新 `a_volume_with_no_notable_page_says_so_and_names_the_key_that_lists_every_page`——设计稿没有一屏钉着那一句（Q1253），「按页跳过」那一景切回需留意的页，宽窄两屏上查 `的页 ⋅ a → 全部页` 那一截（前半的措辞留给 Q1253，Q1271）。

### 按反跑过的几遍（每一遍改一处、跑窄的那一支、还原，还原后 `git diff --stat` 核过）

| 按反 | 结果 |
|---|---|
| 扫描先落、实现之前 | 红：`list.rs`（`n N 跳到下一个`、`F 恢复`）、`pages.rs`（`a → 全部页`、`h → 回卷列表`）、`view.rs`（四句）；三处改完之后 `decision.rs` 下界 1 → 0 红 |
| **`pages.rs` 手抄回 `h → 回卷列表`**（票面那一问） | 红：`[("src/session/shell/pages.rs", ["h → 回卷列表"])]` |
| `decision.rs` 一个键退回 `Segment::new("v", …)` | 红：`key(Deed::` 从 8 处掉到 7 处（`spelt_for(` 仍是 1，评审两轴都点了这个洞，读者表因此添这一行） |
| `session.rs` 中段 `#[cfg(test)] mod scene;` 之后抄一句 | 红：`src/session.rs` 那一句（从前的 `code_only` 在那一行就把后面整段砍掉，照绿） |
| 按键表 `spelt_for` 对一件事改答 `Z`，逐件 12 遍（`cargo test --bin tonefit session::`） | 每一件都有用例红：`ListAll` 9 条（每页结果两景与七串）、`BackToList` 1（`a_skipped_volume…`）、`SearchNext`／`SearchPrev` 各 5、`Follow` 10、`Write`／`WriteAll`／`End`／`ViewPages` 各 5–6（`deciding` 两景与几串）、`QuitRefused` 1、`Stop` 4、`Interrupt` 2 |
| 同上，`Chart` | 红 2：`i_edits_a_filled_in_setting…`、`the_wheel_the_click…` |
| 一页都不需留意那一句的键换一个字（`{}Q → 全部页`） | 红：新那一条；`ListAll` 那一遍分不出它，读过确认没有别的钉着它 |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 8a7c350`（未提交的工作树）。

**收下的**：

- **确认条四个键共用一个闭包，`spelt_for(` 下界只有 1，退回一个键手抄照绿**（Standards、Spec 两轴都报）：读者表添 `("src/session/shell/decision.rs", "key(Deed::", 8)`，按反看见红；扫描文档那句「由下界拦」写清每处怎么拦。
- **`code_only` 在 `session.rs` 中段的 `#[cfg(test)] mod scene;` 就把后面整段砍了**（Standards）：改成只砍就地展开的 `mod 名字 {`，按反看见红；纯用例文件照扫、方向是误红，写进文档。
- **「哪几句提到键」的清单在 `spelt_for` 文档、用例文档、记号常量三处**（Standards，《文档写作》第 4 条）：`spelt_for` 文档只留规矩、指到用例；`pages.rs` 行内那句只指 `keymap::spelt_for`；常量文档不再复述清单。
- **`config.rs` 文档说「键取自按键表」又写死今天的 `c`**（Standards）：改成读表上 `Deed::Chart` 那一行。
- **`KEY_HOME_MARK` 写 `pub fn`，收窄可见性会误红**（Standards）：改成 `fn spelt_for(`。
- **新用例钉死了「这一卷没有需留意的页」的措辞，Q1253 还没拍板**（Spec）：只钉 `的页 ⋅ a → 全部页` 那一截（Q1271）。
- **按键表 `q` 拒绝退出那一行长的那一句仍手写 `s`、`C-c`**（两轴）：记进 Q1268。

**驳下的**：

- **`spelt_for(..).unwrap_or_default()` 重复 18 处，`None` 时静默画出空键，建议加一手 `spelt(deed) -> &'static str` 内部 `expect`**（判断题）：那一手只是转手（Middle Man），而且把一处取不到变成画屏时崩；空键屏上必红——12 件逐件按反，每一处都有成屏用例咬住。沿用既有写法（`picker.rs`、`details.rs` 早就这么写）。
- **`about_setting` 在 `Cow::Borrowed(match …)` 里一支 `return Cow::Owned`**（判断题）：另一种写法是十五支各包一层 `Cow::Borrowed`，只有一支要拼，现在这样改动最小、读得出哪一支特殊。
- **`KEY_HOME_MARK` 那一问与编译重复**（判断题）：照那一支已有几条的形状（「家里真住着」那一问），留着；已改成不问可见性。
- **几处一并改（`F`、回话、`c`）算越界吗**（Spec 判：在票面「凡是」之内，不算）：记 Q1267。

### 数

评审收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-03.gate1.log`、`dp-03.gate2.log`、`dp-03.gate3.log`、`dp-03.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1124 通过 1 失败**；lib 252 / bin 468（新添 1 条）/ `single_source` 8（新添 1 条）；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.13s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **985 通过 1 失败**；lib 252 / bin 329 / `single_source` 8（新添 1 条；`shell` 那一条在 `tui` 后面，不在这一趟）；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 61.87s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |
