# 06 — 滚轮与方向键跟设计稿

**What to build:** 滚轮在「全部按键」那一张上滚它，补全框开着时挪候选（设计稿在这两处都照滚）。终端层把 `←`／`→` 翻成
`h`／`l`（设计稿在键的归一化上就这么做）——按键表不添行，屏底与全部按键不多一种写法；`⇧⇥` 照旧不翻。
设计稿本来就支持，导出几串新序列钉住。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 按键表上滚轮那一行派得到全部按键那一张与补全框
- [ ] 终端层 `←`／`→` 翻成 `h`／`l`；键码翻译那条纯函数用例扩上它们，`⇧⇥` 仍答不认得
- [ ] 导出几串并比整屏且绿：全部按键那一张上滚、补全框上滚、方向键在卷列表与每页结果上各一串
- [ ] 屏底与全部按键那一张一格不变（不出现方向键的写法）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

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
