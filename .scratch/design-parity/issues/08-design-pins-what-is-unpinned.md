# 08 — 设计稿补上还没钉住的几件

**What to build:** 实现在这几件上的行为是对的，却没有一份快照钉着，或者设计稿自己写漏了一支——设计稿补齐、导出、钉住：

- **同名覆盖按两下**：第一下在预设栏里问「再按一次 ⏎ 覆盖「X」：覆盖后无法恢复，其他预设不受影响」，
  输入行与名字留着，第二下才盖，重开起名那一行就作废——措辞取实现今天那一句（Q894）；
- **「整卷统一灰阶 + 等待确认」一景**：钉住确认条上「差异大的页」那一截与卷行那一列（Q900）；
  设计稿确认条与每页结果抬头「需留意几页」同一种数法，代表页数进去（Q902）；
- **覆盖层掀着时答话**照样说那一句回话，与按停止一个待遇（Q903）；
- 设计稿里搜索那一行 `C-w` 之后同步此刻搜的那一句（Q866）；覆盖层上只认 `gg`，`gt`／`gT` 不在它底下换视图（Q793）；
  补全框去掉 `C-n`／`C-p`（Q798）；补全框只有归档标「压缩包」（Q791）；
- **「这里没有以……开头的项」画在输入行右端那几件的位置**，从此看得见——这一件实现跟着改（Q792）。

**Blocked by:** 01

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 同名覆盖那一串导出、比整屏且绿；配置视图里再没有一句话没有快照钉着
- [x] 「整卷统一灰阶 + 等待确认」那一景导出，确认条上「差异大的页」那一截与「需留意几页」比整屏且绿
- [x] 掀着覆盖层按 `x`／`a`／`s` 各一串，屏底说回话，比整屏且绿
- [x] 前缀一条都对不上时按 `Tab`，那一句在输入行右端看得见，一串比整屏且绿
- [x] 搜索那一行 `C-w`、覆盖层上的 `gt`、补全框里的文件标签各一串比整屏且绿（实现照旧，设计稿追上）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q894 — 同名覆盖那两下的那一问是会话自己编的、摆在预设栏里：设计稿根本没有覆盖这件事

- **From:** 票 `session-redesign/14`
- **Kind:** 票面没想到的第三种情形（设计稿缺了票面要的一件事）
- **Where:** 设计稿 `design.html` 的 `submitInput` 的 `preset` 那一支
  （`PRESETS.push({…})`——**撞名直接又推一份进去**，没有「已经有了」这一问，
  `PRESETS` 于是能摆出两份同名的）；实现那一侧是 `src/session/terminal.rs` 的 `store_a_preset`
  与 `src/session/view.rs` 的 `ConfigView::armed_save`
- **Why it did not block:** 票面第一条与 `CONTEXT.md` 的《预设》都写死了「**盖掉一份同名的预设要按两下**」，
  而 `crate::preset::Presets` 那一侧早就是两个动作（`save` 撞名回 `Saved::Taken`、一个字节不写；
  `replace` 才盖），旧界面也早就走这两下——**盘那一侧一个字都不用改**。
  缺的只有**屏上那一句**：设计稿没有这一串，因此没有一份期望屏钉得住它。
- **What this ticket actually did:** 第一下走 `save`，撞上就把那个名字闩在
  `ConfigView::armed_save` 上、**输入行留着、名字留在缓冲里**，并在**预设栏里**问一句
  「再按一次 ⏎ 覆盖「X」：覆盖后无法恢复，其他预设不受影响」——**那一句是会话自己写的**
  （照《预设》那一段「屏上那一句只说无条件成立的那两半」的写法，与 `dd` 那一句
  **同一副骨架、同一处代码**：`shell::picker::asked`）。重开一次起名那一行就把它作废。

  **那一问非摆在预设栏里不可**：这一刻输入行占着屏底，而 `shell::footer` 在输入行开着时
  整个让给它、一句回话都不画（设计稿 `drawFooter` 同形）——说给屏底等于一个字都没说，
  而《预设》写着「第一下只说一句」。头一版正是说给屏底的，评审当场逮住。
  **闩没有与 `dd` 那一格合用**：合用的话起名那一刻预设栏里会冒出一句「再按一次 dd 删除」。
  钉住它的是 `terminal::redesign::an_existing_name_takes_two_presses_before_it_overwrites`
  ——**没有期望屏可对**，比的是盘、输入行，以及屏上真画出了那一问。
- **Options:** ① 照本票：措辞由会话定，屏上没有一份快照钉着它；
  ② 给设计稿补上这一串（`configKey` 的 `preset` 提交那一支加一问、导出 `config-p-save-taken` 之类），
  措辞与逐格比对一起定下来；③ 不问、直接盖：不取——票面与《预设》都写死了两下
- **Recommend:** ②（屏上每一句都该有一份快照钉着；这一句眼下是配置视图里**唯一一句没有**的）
- **Whose call:** 拍板的人（动设计稿那一侧）
- **处置：** **本票了结，照推荐 ②**：设计稿 `submitInput` 的 `preset` 那一支撞名先闩上（`S.cfg.armedSave`）、输入行留着，`drawPresets` 与删那一问同一副骨架问一句，措辞取实现那一句；重开起名那一行作废。导 `config-p-save-taken`、`config-p-save-taken-Enter` 两串比整屏，实现一行没动。见《落地记录》。

#### Q900 — 确认条上「与其他页差异大 N」那一截**没有一张快照踩得到**

- **From:** 票 `session-redesign/12`
- **Kind:** 落地了却没有断言按着的一截
- **Where:** `src/session/shell/decision.rs` 的 `this_volume`（那一支照设计稿
  `drawDecision` 的 `pr.outlier ? […] : []` 写着）
- **Why it did not block:** 差异大的页只有**整卷统一灰阶**那一趟才有
  （设计稿 `pagesOf` 那一支 `envelope ? p.outliers || 0 : 0`），而夹具里唯一那一景
  （`envelope`）**没有停在确认点上**——两件事凑不到一屏上。照 11 号票评审第 1 条的教训
  （「眼下没有一张快照踩得到它」正是那种用例绿着、屏上已经坏了的洞），这一截**写了**，
  但屏上那一截没有断言按着
- **What this ticket actually did:** 照设计稿写出来，数走 `Live::notable_at` 一处
  （与卷行行尾报的是同一份），并在这里记一笔
- **Options:** ① 照现状 ② 导一张「整卷统一灰阶 + 等待确认」的景，这一截连同卷行那一列
  一起钉住 ③ 不写它：不取，那是明知设计稿有一截而故意漏掉
- **Recommend:** ②，归摆场景那一票或 18（两副并排审）
- **Whose call:** 协调人
- **处置：** **本票了结，照推荐 ②**：设计稿新景 `envelope-deciding`（整卷统一灰阶那一趟停在第一个确认点上、那一卷的目录展开着），120×36 与 80×24 两份快照比整屏；实现一行没动。

#### Q902 — 「需留意几页」那个数：设计稿自己的两块各数了一副，词汇表站在其中一边

- **From:** 票 `session-redesign/12`
- **Kind:** 设计稿内部对不上，而词汇表已经判过（**Q900 的邻居**）
- **Where:** 设计稿 `design.html`：`drawDecision` 数的是
  `pagesOf(v).filter((p) => p.tone).length`，`drawPages` 数的是
  `all.filter((p) => p.tone || p.driver).length`——**同一句「需留意几页」，两块差一个代表页**。
  实现这一侧是 `src/session/shell/decision.rs` 的 `this_volume` 与
  `src/session/view.rs` 的 `Pages::notable_count`
- **Why it did not block:** 代表页只有**整卷统一灰阶**那一趟才有，而夹具里那一景没有停在确认点上
  ——两副数法在现有的每一张快照上给的是同一个数（`deciding` 那一景走的是逐页判断，两边都是 1）。
  `CONTEXT.md` 的《需留意的页》**明写着**这几页里「加上**代表页**（它是这一卷的答案，非在不可）」，
  11 号票据此把判定收在 `render::notable` 一处
- **What this ticket actually did:** **照词汇表**，走 `Pages::notable_count` 一处——
  确认条与每页结果抬头报的因此是同一个数，屏上上下两块不会为同一句话给出两个数。
  代价：整卷统一灰阶那一趟停在确认点上时，这一格比设计稿多一页
- **Options:** ① 照现状（词汇表那一边，一处出处） ② 照 `drawDecision` 另数一副：
  那是给「需留意几页」立第二条数法，`CLAUDE.md` 的单一出处拦的正是这件事
  ③ 改设计稿让 `drawDecision` 与 `drawPages` 一致、重导——ADR 0019 决定第 13 条
- **Recommend:** ③ 加 ①：设计稿那两块本来就该一致，而一致成哪一边词汇表已经答过了
- **Whose call:** 拍板的人（动设计稿），与 **Q900** 一起看（两条都要那一张「整卷统一灰阶 + 等待确认」的景才验得着）
- **处置：** **本票了结，照推荐 ③ 加 ①**：设计稿立一个 `notableOf`（代表页也数进去），确认条与每页结果同读它；`envelope-deciding` 那一景钉着「需留意的页 3」。实现一行没动。

#### Q903 — 掀着覆盖层答话时，设计稿不说那一句回话，这一副说

- **From:** 票 `session-redesign/12`
- **Kind:** 设计稿自己前后不一致的一处
- **Where:** 设计稿 `design.html` 覆盖层那一支（`if (S.overlay) { … 'xas'.includes(k) … }`，
  **只答话、不 toast**）对着全局那一支（答话之后 `toast(…)`）；实现这一侧是
  `src/session/view.rs` 的 `answer_the_point`——**两处都说**
- **Why it did not block:** 一串序列都观察不到它（`deciding-help` 掀着那一张，却没有按 `x`／`a`／`s`）。
  **设计稿自己前后不一致**：同一支里的 `s`（按停止）走 `stopKey()`，那一支**是**说话的
  ——「掀着覆盖层就不说回话」不是一条立过的规矩，看着像那一支写漏了。
  `CONTEXT.md` 的《覆盖层》只说「屏底让给覆盖层自己的提示」，说的是**摆哪几件键**，
  没说那一句回话该不该出
- **What this ticket actually did:** 答话与按停止一视同仁，两种都说那一句
  （`Session::perform` 那一层根本不问掀没掀覆盖层——问它就是在那一层立第二条「此刻该不该说话」）
- **Options:** ① 照现状 ② 照覆盖层那一支：`say` 之前问一句掀没掀——那要连 `s` 一起改，
  否则屏上同一张覆盖层上两个键一个说话一个不说 ③ 改设计稿把那一支补齐、重导
- **Recommend:** ③，与屏底那一行「回话几秒后退回」一起看（覆盖层掀着时那一行归谁，
  眼下两边各说了一半）
- **Whose call:** 拍板的人
- **处置：** **本票了结，照推荐 ③**：设计稿答话那一句收进 `answerKey`，覆盖层那一支与全局那一支都走它；导 `deciding-help-x`／`-a`／`-s` 三串比整屏。实现一行没动。

#### Q866 — 设计稿的 `C-w` 在搜索那一行上漏了同步「此刻搜的是哪一句」，实现这一头派生因此天然一致

- **From:** 票 `session-redesign/09`
- **Kind:** 路过发现的无关缺陷（设计稿那一侧的）
- **Where:** `design.html` 的 `onKey` 打字那一支：打一个字与 `Backspace` 两处都写着 `if (inp.kind === 'search') S.search.q = inp.buf;`，而 **`C-w` 那一支没有这一句**——在设计稿里按 `C-w` 删掉一段之后，缓冲短了而匹配处的下划线与框底边那一截还写着删之前那一句
- **Why it did not block:** **没有一串序列踩到它**：`running-search-typed` 只打字，`search-Escape`／`search-Enter-Escape` 不删字。而实现这一头「此刻搜的是哪一句」是**派生出来的**（`Views::searching`：搜索那一行开着时就是它的缓冲），根本没有第二份数据要同步——`C-w` 之后屏上当场跟着短，与设计稿那一支的行为不同，但**与设计稿自己写着的意图一致**（另两支都同步）
- **What this ticket actually did:** 照派生那一条做，**没有照抄设计稿那个漏**。理由：三支里两支同步、一支漏，那是漏不是规矩；而照抄它要在实现里专门造一个「`C-w` 不同步」的岔路，那是把一个缺陷刻进代码。逐格验收一格没让——没有夹具照到这一格
- **Options:** ① 照现状：实现一致，设计稿那一支仍漏着 ② 按 ADR 0019 决定第 13 条改设计稿那一支、重新导出（那是拍板的人的事，而它一格快照都不会变——没有夹具落在这一格上） ③ 照抄那个漏：不取
- **Recommend:** ②，与别的设计稿订正一批做（它不改任何一张快照，是纯粹的原型订正）；在那之前①站得住
- **Whose call:** 拍板的人（设计稿是他的）
- **处置：** **本票了结，照推荐 ②**：设计稿 `C-w` 那一支同步 `S.search.q`；导 `search-C-w` 比整屏。实现一行没动。

#### Q793 — 覆盖层掀着时按 `g` `t`：设计稿在覆盖层底下换了视图，表上 `gt` 只在没被盖着的块上派

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿与词汇表对不上（`CONTEXT.md`《覆盖层》：掀着的时候除了按停止与答话别的键一律不派）
- **Where:** `design.html` 的 `onKey`（连击键那一支在覆盖层那一支之前，`gt`／`gT` 不问 `S.overlay`）；`src/session/keymap.rs` 的 `NextView`／`PrevView` 两行（`UNCOVERED`）
- **Why it did not block:** 没有一串序列在覆盖层上按 `gt`；`g` 待着这一下两边一样（`gg` 在覆盖层上派得出）
- **What this ticket actually did:** 照表：覆盖层上 `g` 待着、`t` 合不上就丢掉，视图不换
- **Options:** ① 设计稿的连击键分支挪到覆盖层分支之后、覆盖层上只认 `gg`；② 表上 `gt`／`gT` 放宽到覆盖层——与《覆盖层》那一句相抵
- **Recommend:** ①
- **Whose call:** 拍板的人（动设计稿）
- **处置：** **本票了结，照推荐 ①**：覆盖层上的连击键只认 `gg`——`gt`／`gT`／`]d`／`[d`／`dd` 收进连击键那一支的 `if (!S.overlay)`，与 spec 字面「排在覆盖层分支之后」写法不同、行为相同（照字面挪的话覆盖层那一支得自己再管一遍 `g` 待着）；导 `help-gt` 比整屏。实现一行没动。

#### Q798 — 设计稿补全框里 `C-n`／`C-p` 也挪候选，表上只有 `↓`／`↑`（设计稿的 `KEYMAP` 也没列 `C-n`／`C-p`）

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿自己前后不一（派得出的键不在 `KEYMAP` 上）
- **Where:** `design.html` 的 `onKey` 打字那一支（`raw === 'ArrowDown' || k === 'C-n'`）；`src/session/keymap.rs`（`↓`／`↑` 那两行 `ANY_BLOCK`）
- **Why it did not block:** 没有一串序列按 `C-n`／`C-p`
- **What this ticket actually did:** 照表：`↓`／`↑` 挪候选，`C-n`／`C-p` 不派
- **Options:** ① 设计稿去掉 `C-n`／`C-p`；② 表上加两行、`KEYMAP` 也列
- **Recommend:** ①
- **Whose call:** 拍板的人（动设计稿）
- **处置：** **本票了结，照推荐 ①**：设计稿去掉 `C-n`／`C-p`；导 `add-C-n` 比整屏（候选不挪）。实现一行没动。

#### Q791 — 设计稿把补全框里每一条不是文件夹的候选都标「压缩包」（`答案.txt  压缩包`），确定时也当压缩包收；实现按扩展名认

- **From:** 票 `session-redesign/07`
- **Kind:** 路过发现的设计稿缺陷
- **Where:** `design.html` 的 `drawCompletions`（`c.dir ? '' : '  压缩包'`）与 `submitInput`（`kind = hit && !hit.dir ? '压缩包' : '文件夹'`）；`src/session/shell/completions.rs`；`NamedPath::kind`（`tonefit::is_archive`）
- **Why it did not block:** 导出的候选全是文件夹（`~/` 与 `~/Comics/` 底下），没有一屏画到一个文件
- **What this ticket actually did:** 是归档的文件旁边标「压缩包」，不是归档的文件不标；添进去的那一条按扩展名认种类（与卷列表同一处，`NamedPath::kind_of`）——`⏎` 收下一个不是归档的文件时两边因此都说错：设计稿「压缩包」、实现「文件夹」，根子是同一条
- **Options:** ① 设计稿按扩展名标（假盘里 `字体包.zip` 是压缩包、`答案.txt`／`README.md` 不是），或补全时干脆不列不是归档的文件；② 实现照设计稿全标压缩包——屏上说假话
- **Recommend:** ①（先按扩展名标；不列不是归档的文件是另一个决定：一条处理路径只能是文件夹或压缩包，列出来也添不进去）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** **本票了结，照推荐 ① 的前半**：设计稿 `drawCompletions` 按扩展名标（`isArchive`）；导 `fresh-o-files-Tab`（`~/漫画库/` 八项）比整屏。`submitInput` 收下不是归档的文件时当什么没动（spec《Out of Scope》）。实现一行没动。

#### Q792 — `Tab` 一个都对不上时设计稿 `toast` 一句「这里没有以「…」开头的项」，而输入行占着屏底，那一句根本画不出来

- **From:** 票 `session-redesign/07`
- **Kind:** 路过发现的设计稿缺陷（`drawFooter` 输入行那一支先返回，回话只在关掉输入行之后 1.6 秒内看得见）
- **Where:** `design.html` 的 `complete`（`toast(…, 1600)`）与 `drawFooter`；`src/session/typing.rs` 的 `complete_typed`（`NO_MATCH_LINGERS`）
- **Why it did not block:** 没有一串序列在这一步比屏；说与不说屏上都一样
- **What this ticket actually did:** 照设计稿说那一句、占 1.6 秒，画法照设计稿让输入行盖着它——与设计稿逐格相同，也一样看不见
- **Options:** ① 设计稿把这一句画在输入行右端那几件的位置（或候选框的位置），实现跟着；② 设计稿不说这一句，实现也不说；③ 照旧
- **Recommend:** ①（打了一个对不上的前缀，屏上该有个反应）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** **本票了结，照推荐 ①**：设计稿与实现都把没到点的回话画在输入行右端那几件的位置上、到点退回（通用那一副，Q1187）；导 `fresh-o-unmatched-Tab` 比整屏。实现改在 `shell::footer` 输入行那一支。

## 落地记录

**本票做了什么。** 先改设计稿、重导，再照新导出的期望屏比实现。九件里八件实现本来就对，**实现一行没动**、只补用例；
Q792 那一件实现跟着改（`shell::footer` 输入行那一支）。结转的九条都收了。

| 件 | 设计稿（`design.html`） | 导出（`export.js`） | 实现 |
|---|---|---|---|
| 同名覆盖按两下（Q894） | `submitInput` 的 `preset` 那一支：撞名而 `S.cfg.armedSave` 不是这个名字就闩上、把输入行放回去；是就原处覆盖。`drawPresets` 删与覆盖两问同一副骨架（`asking`）。重开起名那一行作废（`configKey`） | `config-p-save-taken`、`config-p-save-taken-Enter` | 没动 |
| 整卷统一灰阶 + 等待确认（Q900） | 新景 `envelope-deciding`：开着整卷统一灰阶起一趟预览，停在第一个确认点（哆啦A梦/第05卷），那一卷的目录展开 | `SNAPSHOTS` 添它（两种尺寸） | 没动 |
| 需留意几页一种数法（Q902） | 顶层 `notableOf`（`tone || driver`）；`drawDecision` 与 `drawPages` 同读它 | 同上那一景钉着「需留意的页 3」 | 没动 |
| 掀着覆盖层答话（Q903） | 答话那一句收进 `answerKey`，覆盖层那一支与全局那一支都走它 | `deciding-help-x`／`-a`／`-s` | 没动 |
| 搜索那一行 `C-w`（Q866） | `C-w` 那一支同步 `S.search.q` | `search-C-w` | 没动 |
| 覆盖层上只认 `gg`（Q793） | 连击键那一支里 `gt`／`gT`／`]d`／`[d`／`dd` 收进 `if (!S.overlay)`（spec 原话「排在覆盖层分支之后」，行为相同，一处管待续） | `help-gt` | 没动 |
| 去掉 `C-n`／`C-p`（Q798） | 补全框里只认 `↓`／`↑` | `add-C-n` | 没动 |
| 只有归档标「压缩包」（Q791） | 顶层 `isArchive`（cbz · zip · rar · 7z，不分大小写），`drawCompletions` 读它 | `fresh-o-files-Tab` | 没动 |
| 对不上的前缀那一句（Q792） | `drawFooter` 输入行那一支：回话没到点就画在右端那几件的位置上 | `fresh-o-unmatched-Tab` | `shell::footer` 输入行那一支同形（`views.reply(now)`）；`typing.rs` 三处说明跟着改 |

- **评审之后另补两串**：`config-p-save-taken-Escape-Enter`（撞名、`Esc`、末行上再按 `⏎`：那一问没了，钉住「重开起名那一行就作废」）、
  `envelope-deciding-v`（同一景按 `v`：每页结果抬头「需留意 3/224 页」与确认条「需留意的页 3」同屏）。
- **另补两串**（`config-h-G`、`config-h-l-k-l-G`）：票面第二格要「配置视图里再没有一句话没有快照钉着」，拿实现里配置视图的每一句对过导出的每一屏，
  画质判定参数那一组的详情栏（「⋅ 画质判定参数」抬头、「与报告抬头里的这一行逐字相同」、「这里是此刻的设置…」、没有冲突时那一段说明）一屏都没有；
  这两串把它钉上。设计稿到不了的三句（「没有设置任何项（全部默认）」「读不懂：…」「…读不懂，套不下来」）没钉，那一格的打勾读作「设计稿到得了的每一句」（Q1190）。
- **新景那一份场景数**：`envelope-deciding` 是第 13 景，`export.test.js`、`shell::design` 与 `scene` 里数场景与快照的四处断言跟着从 12 改 13。
- **导出器的堆**：`npm test` 一个进程导两趟，第一趟的清单里留着设计稿 realm 造的数组（`final_size: page.design.S.size.slice()`，起点场景那一份同理），
  经原型链拽着那一份伪 DOM 的整个全局对象——基线上已擦边（70 秒过），本票多出的十几屏把它推到 OOM（4 GB 堆）。改成抄成 Node 这一侧的普通数据（`toNodeSide`）、
  用完的伪 DOM 关掉（`release`）；量过：导完一趟、清单还拿着，GC 之后堆里留的从 2055 MB（018b281 那一份）降到 47 MB，产物一字节没变。
- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过：既有的快照、场景数据、期望屏**一个字节没变**；只有 `manifest.json` 添条目（+432 行），
  新文件是一景的两份快照与它的场景数据、十四串的期望屏。node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过（62 秒）。
- **用例**（全在 `#[cfg(test)]` 里）：
  - `terminal`：`an_existing_name_takes_two_presses_before_it_overwrites` 改成两串各比整屏、另核盘上那份文件（另加一句「盖上去的与原来那一份不同」，免得第二下是空操作照绿）；
    新 `answering_under_the_key_sheet_still_says_what_the_answer_did`、`ctrl_w_on_the_search_line_shortens_what_is_searched`、
    `g_t_under_the_key_sheet_does_not_switch_the_view`、`the_completion_box_labels_only_the_archives`、`ctrl_n_does_not_step_through_the_completion_box`、
    `a_prefix_that_matches_nothing_says_so_at_the_right_end_of_the_input_line`（比整屏，另在 `NO_MATCH_LINGERS` 之后画一帧核右端那几件退回来；那个时长因此开到 `pub(super)`）、
    `the_judging_rows_explain_themselves_in_the_details_pane`、`reopening_the_naming_line_forgets_the_name_it_asked_about`、
    `at_an_envelope_point_the_pages_and_the_decision_bar_count_the_same_notable_pages`；`painted` 拆出 `painted_at`（在给定的那一刻画一屏）。
  - `shell`：新 `the_envelope_deciding_scene_matches_its_design_snapshot_wide_and_narrow`。
- **Q1061／Q1092**（80 列上输入行被右端那几件盖住）那两屏没有回话在场，一格没变；回话在场的那一两秒里，右端是那一句而不是那四件——
  补全那一句比那四件窄，盖得反而少。这两条本票没去拍板。
- **`CONTEXT.md`**：没动。《输入行》「右端是这一行上按得动的那几件」没说回话会临时占那里，改写已有词条要先拍板（Q1188）。

### 按反跑过的几遍（每一遍改一处、跑 `cargo test --bin tonefit <过滤>`、还原，还原后 `git diff` 核过）

| 按反 | 结果 |
|---|---|
| 预设栏不画覆盖那一问（`picker::asked` 那一支回 `None`） | 红：`config-p-save-taken` 第 13 行第 56 格，期望「再」实际空 |
| 确认条不报「与其他页差异大」那一截 | 红：`envelope-deciding.120x36` 第 8 行第 65 格前景色 |
| 确认条的需留意几页少数一页（照设计稿旧的 `tone` 数法） | 红：`envelope-deciding.120x36` 第 8 行，期望「3」实际「2」 |
| 覆盖层掀着时答话不说回话 | 红：`deciding-help-x` 屏底第 1 格，期望「写」实际「[」 |
| 搜索那一行缓冲删空时退回定下的那一句（`C-w` 不同步） | 红：`search-C-w` 第 10 行第 10 格多了下划线 |
| `gt` 放宽到覆盖层 | 红：`help-gt` 第 0 行顶栏前景色（底下换成了配置视图） |
| 按键表添一行 `C-n` 挪候选 | 红：`add-C-n` 第 30 行，光标那一格换了一行 |
| 补全框每个文件都标「压缩包」 | 红：`fresh-o-files-Tab` 第 26 行 `README.md` 那一行多了标签 |
| 详情栏「逐字相同」那一句改一个字 | 红：`config-h-G` 第 9 行第 76 格 |
| 屏底输入行那一支不看回话到没到点（拿 10 秒之前那一刻问） | 红：`a_prefix_that_matches_nothing…` 的「那几件退回来了」 |
| 重开起名那一行不作废那一问（`use_preset` 不清 `armed_save`） | 红：`config-p-save-taken-Escape-Enter` 第 13 行第 56 格，那一问还在 |
| 每页结果抬头的需留意几页少数一页（确认条照旧） | 红：`envelope-deciding-v` 第 12 行，期望「3」实际「2」 |

实现之前那一遍：导出 `fresh-o-unmatched-Tab` 之后，`a_prefix_that_matches_nothing…` 红在第 35 行第 68 格（期望空、实际「[」——右端那几件照旧画着，那一句被盖住）；
`shell::footer` 改完转绿。另八条（Q894、Q900／Q902、Q903、Q866、Q793、Q798、Q791 各自那一条，加上补的画质判定参数那一条）导出之后头一跑就绿——实现本来就对，红是上表那几遍按反跑出来的。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 018b281`（未提交的工作树）加新文件。

**收下的**：

- **两条用例里同一个数写了两遍**（Standards，testing.md「一条用例里同一个数只有一个出处」）：对不上前缀那一条的尺寸改读 `scene::sequence(name).size`、
  到点的时刻改读 `typing::NO_MATCH_LINGERS`（原先手写 2 秒）；`every_exported_snapshot…` 里的场景数收成一个 `scenes`。
- **用例名 `premise`**（Standards，CLAUDE.md「测试名一律取自 `CONTEXT.md`」）：词条是《画质判定参数 (Judging)》，改成 `the_judging_rows_…`。
- **Rust 那一侧没指着条目号**（Standards，停车场「拍过板的结论要进代码旁边」）：`shell::footer` 输入行那一支补「停车场 Q792」，`Completion::label` 补回「停车场 Q791」。
- **屏底两支各拼一遍「件与件之间一个空」**（Standards，Duplicated Code）：抽成 `footer::hints` 与 `footer::spaced`，两支共用；
  设计稿两处「回话还没到点」收成 `liveToast`；用例里手搭的终端收进 `painted_at`，读屏底那一行改用 `design::lines_of`。
- **导出器 `done`／`plain` 名字看不出做什么**（Standards，Mysterious Name）：改成 `release`／`toNodeSide`。
- **两条停车场条目不是岔口**（Standards，停车场《什么才值得记一条》）：覆盖层上只认 `gg` 的写法、导出器的堆，都只有一条路站得住，挪进本记录与 Q793 的处置，条目删掉。
- **「重开起名那一行就作废」没有一串钉着**（Spec，票面第一件）：补 `config-p-save-taken-Escape-Enter`。
- **每页结果抬头那个数没与确认条同屏比过**（Spec，Q902「确认条与每页结果抬头同一种数法」）：补 `envelope-deciding-v`。
- **《数》还没写就打了勾**（两轴）：那一格先取消，闸门跑完再勾、再填《数》。

**驳回的**：

- **`FitMode::Height` 是缩放方式的默认值**（Standards，testing.md「改成 X 那一族里的 X 不许等于当前默认值」）：那一串把 `Some(Inside)` 改成 `Some(Height)`，
  不是从「没说」改到默认——空操作的那种照绿踩不到；改没改由整屏比对与 `Some(Height)` 那一句一起按着。
- **`S.search.q = inp.buf` 有了第三份**（Standards，Duplicated Code）：Q866 要的正是「与打字、退格两支一样」，三支平行写比收成一个只用在设计稿里的小函数更好对照。
- **场景数散在六处**（Standards，Shotgun Surgery）：导出器与 Rust 各自数一遍是两侧互相核的意思；Rust 那一条用例里已收成一处。
- **回话比整行还宽时两边截法不同**（Spec，边角）：设计稿从负坐标起画、露尾，实现贴左、露头；回话宽过整行这一种两边都没有一串踩到，也不是本票走过的岔口，不记。

### 停车场

本票用了 Q1187–Q1196 里的四个（Q1191–Q1196 留空）：

- **Q1187**：输入行开着时任何一句没到点的回话都画在右端，不只补全那一句；推荐照现在。
- **Q1188**：《输入行》词条没说回话会临时占右端；推荐添一句。
- **Q1189**：起名那一行 `Esc` 之后覆盖那一问还挂着、说的是假话（实现如此，设计稿照画）；推荐设计稿与实现都在关掉那一行时作废它。
- **Q1190**：配置视图里设计稿到不了的三句没钉；推荐照现在。

### 数

评审收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-08.gate1.log`、`dp-08.gate2.log`、`dp-08.gate3.log`、`dp-08.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1096 通过 1 失败**；lib 250 / bin 452；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.42s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **964 通过 1 失败**；lib 250 / bin 320；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.78s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`；两道 clippy 一条告警都没有，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `018b281`，没在它上面重跑。本票新添的十条用例全在 `tui` 后面（`terminal` 九条、`shell` 一条），
`--no-default-features` 那一趟一条都没多——闸门 2 的 964 就是基线的数，闸门 1 推得基线 1086、本票多 10 条。

**黄金快照逐格没动**：`git diff 018b281 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。
