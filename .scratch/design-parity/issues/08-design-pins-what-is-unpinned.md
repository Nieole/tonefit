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

**Status:** ready-for-agent

- [ ] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [ ] 同名覆盖那一串导出、比整屏且绿；配置视图里再没有一句话没有快照钉着
- [ ] 「整卷统一灰阶 + 等待确认」那一景导出，确认条上「差异大的页」那一截与「需留意几页」比整屏且绿
- [ ] 掀着覆盖层按 `x`／`a`／`s` 各一串，屏底说回话，比整屏且绿
- [ ] 前缀一条都对不上时按 `Tab`，那一句在输入行右端看得见，一串比整屏且绿
- [ ] 搜索那一行 `C-w`、覆盖层上的 `gt`、补全框里的文件标签各一串比整屏且绿（实现照旧，设计稿追上）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
- **处置：** 待处理。

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
- **处置：** 待处理。

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
- **处置：** 待处理。

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
- **处置：** 待处理。

#### Q866 — 设计稿的 `C-w` 在搜索那一行上漏了同步「此刻搜的是哪一句」，实现这一头派生因此天然一致

- **From:** 票 `session-redesign/09`
- **Kind:** 路过发现的无关缺陷（设计稿那一侧的）
- **Where:** `design.html` 的 `onKey` 打字那一支：打一个字与 `Backspace` 两处都写着 `if (inp.kind === 'search') S.search.q = inp.buf;`，而 **`C-w` 那一支没有这一句**——在设计稿里按 `C-w` 删掉一段之后，缓冲短了而匹配处的下划线与框底边那一截还写着删之前那一句
- **Why it did not block:** **没有一串序列踩到它**：`running-search-typed` 只打字，`search-Escape`／`search-Enter-Escape` 不删字。而实现这一头「此刻搜的是哪一句」是**派生出来的**（`Views::searching`：搜索那一行开着时就是它的缓冲），根本没有第二份数据要同步——`C-w` 之后屏上当场跟着短，与设计稿那一支的行为不同，但**与设计稿自己写着的意图一致**（另两支都同步）
- **What this ticket actually did:** 照派生那一条做，**没有照抄设计稿那个漏**。理由：三支里两支同步、一支漏，那是漏不是规矩；而照抄它要在实现里专门造一个「`C-w` 不同步」的岔路，那是把一个缺陷刻进代码。逐格验收一格没让——没有夹具照到这一格
- **Options:** ① 照现状：实现一致，设计稿那一支仍漏着 ② 按 ADR 0019 决定第 13 条改设计稿那一支、重新导出（那是拍板的人的事，而它一格快照都不会变——没有夹具落在这一格上） ③ 照抄那个漏：不取
- **Recommend:** ②，与别的设计稿订正一批做（它不改任何一张快照，是纯粹的原型订正）；在那之前①站得住
- **Whose call:** 拍板的人（设计稿是他的）
- **处置：** 待处理。

#### Q793 — 覆盖层掀着时按 `g` `t`：设计稿在覆盖层底下换了视图，表上 `gt` 只在没被盖着的块上派

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿与词汇表对不上（`CONTEXT.md`《覆盖层》：掀着的时候除了按停止与答话别的键一律不派）
- **Where:** `design.html` 的 `onKey`（连击键那一支在覆盖层那一支之前，`gt`／`gT` 不问 `S.overlay`）；`src/session/keymap.rs` 的 `NextView`／`PrevView` 两行（`UNCOVERED`）
- **Why it did not block:** 没有一串序列在覆盖层上按 `gt`；`g` 待着这一下两边一样（`gg` 在覆盖层上派得出）
- **What this ticket actually did:** 照表：覆盖层上 `g` 待着、`t` 合不上就丢掉，视图不换
- **Options:** ① 设计稿的连击键分支挪到覆盖层分支之后、覆盖层上只认 `gg`；② 表上 `gt`／`gT` 放宽到覆盖层——与《覆盖层》那一句相抵
- **Recommend:** ①
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

#### Q798 — 设计稿补全框里 `C-n`／`C-p` 也挪候选，表上只有 `↓`／`↑`（设计稿的 `KEYMAP` 也没列 `C-n`／`C-p`）

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿自己前后不一（派得出的键不在 `KEYMAP` 上）
- **Where:** `design.html` 的 `onKey` 打字那一支（`raw === 'ArrowDown' || k === 'C-n'`）；`src/session/keymap.rs`（`↓`／`↑` 那两行 `ANY_BLOCK`）
- **Why it did not block:** 没有一串序列按 `C-n`／`C-p`
- **What this ticket actually did:** 照表：`↓`／`↑` 挪候选，`C-n`／`C-p` 不派
- **Options:** ① 设计稿去掉 `C-n`／`C-p`；② 表上加两行、`KEYMAP` 也列
- **Recommend:** ①
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

#### Q791 — 设计稿把补全框里每一条不是文件夹的候选都标「压缩包」（`答案.txt  压缩包`），确定时也当压缩包收；实现按扩展名认

- **From:** 票 `session-redesign/07`
- **Kind:** 路过发现的设计稿缺陷
- **Where:** `design.html` 的 `drawCompletions`（`c.dir ? '' : '  压缩包'`）与 `submitInput`（`kind = hit && !hit.dir ? '压缩包' : '文件夹'`）；`src/session/shell/completions.rs`；`NamedPath::kind`（`tonefit::is_archive`）
- **Why it did not block:** 导出的候选全是文件夹（`~/` 与 `~/Comics/` 底下），没有一屏画到一个文件
- **What this ticket actually did:** 是归档的文件旁边标「压缩包」，不是归档的文件不标；添进去的那一条按扩展名认种类（与卷列表同一处，`NamedPath::kind_of`）——`⏎` 收下一个不是归档的文件时两边因此都说错：设计稿「压缩包」、实现「文件夹」，根子是同一条
- **Options:** ① 设计稿按扩展名标（假盘里 `字体包.zip` 是压缩包、`答案.txt`／`README.md` 不是），或补全时干脆不列不是归档的文件；② 实现照设计稿全标压缩包——屏上说假话
- **Recommend:** ①（先按扩展名标；不列不是归档的文件是另一个决定：一条处理路径只能是文件夹或压缩包，列出来也添不进去）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

#### Q792 — `Tab` 一个都对不上时设计稿 `toast` 一句「这里没有以「…」开头的项」，而输入行占着屏底，那一句根本画不出来

- **From:** 票 `session-redesign/07`
- **Kind:** 路过发现的设计稿缺陷（`drawFooter` 输入行那一支先返回，回话只在关掉输入行之后 1.6 秒内看得见）
- **Where:** `design.html` 的 `complete`（`toast(…, 1600)`）与 `drawFooter`；`src/session/typing.rs` 的 `complete_typed`（`NO_MATCH_LINGERS`）
- **Why it did not block:** 没有一串序列在这一步比屏；说与不说屏上都一样
- **What this ticket actually did:** 照设计稿说那一句、占 1.6 秒，画法照设计稿让输入行盖着它——与设计稿逐格相同，也一样看不见
- **Options:** ① 设计稿把这一句画在输入行右端那几件的位置（或候选框的位置），实现跟着；② 设计稿不说这一句，实现也不说；③ 照旧
- **Recommend:** ①（打了一个对不上的前缀，屏上该有个反应）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。
