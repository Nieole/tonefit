# 02 — 屏上不摆按不动的键，按得动的都摆

**What to build:** 按键表多一维**输入行的用途**：添加与修改处理路径、改输出目录、改一项设置的值、给预设起名、搜索。
`Tab` 补全只在路径那三种用途上派得出，`C-w` 删一段在每一种上都派得出。屏底右端那几件从表上一路派下来——
今天为搜索那一行手摆两件的那道岔路删掉。预设栏光标停在末行「＋ 把当前设置保存为预设」上时屏底不摆 `dd`。

设计稿跟着改：右端那几件按用途摆，搜索那一行摆 `C-w`，预设栏末行不摆 `dd`、在那一行上按 `dd` 不再抛错。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [ ] 改一项设置的值、给预设起名两种输入行屏底右端不摆 `Tab`；三种路径输入行照旧摆
- [ ] 搜索那一行屏底右端摆 `C-w`
- [ ] 预设栏光标在末行上时屏底不摆 `dd`，停在一份预设上照旧摆
- [ ] 「屏底每一件都是按键表的一行」「全部按键与屏底出自同一张表」两条用例扩上用途这一维：每一种用途上屏底右端恰好是那一种上派得出的键
- [ ] 画法里不再有按「是不是搜索那一行」手摆屏底的岔路
- [ ] `CONTEXT.md`《输入行》改成右端那几件按用途摆（`⇥` 只在路径那几种上）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
