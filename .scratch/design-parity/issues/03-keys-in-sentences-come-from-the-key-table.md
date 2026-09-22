# 03 — 屏上句子里提到的键出自按键表

**What to build:** 屏上凡是在一句话里提到一个键的地方，键的写法一律取按键表那个**不问阶段与块**的写法：
每页结果框底边那一件 `a → …`、跳过的卷那一句末尾的 `h → 回卷列表`、一页都不需留意那一句末尾的 `a → 全部页`、
搜索提示的 `n N`、确认条上的四个键。**确认条那四句仍是它自己的措辞**（与屏底那几句回话一样），按键表不为它添列。

再补回一条文件扫描：画法那几个模块的句子里不许手抄键——下一次手抄当场红。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 上面那几处键字面读按键表；屏上一格不变——全部设计快照与交互期望屏照旧绿
- [ ] `tests/single_source.rs` 补回那条扫描：家是按键表那个模块，读者是画法那几个模块，记号取新界面屏上真出现过的几句；拿一处手抄的键去试，它红
- [ ] `CONTEXT.md`《确认条》里 `v` 那一句改成「查看每页结果」，与设计稿、屏上一致
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
- **处置：** 待处理。

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
- **处置：** 待处理。

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
- **处置：** 待处理。
