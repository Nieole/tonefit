# 04 — 《预览过》：答了不写出的那一卷在屏上认得出

**What to build:** 确认点上答了「不写出」、分析做完而写出环节一步没走的那一卷，收摊时落在卷状态新添的一档
**《预览过 (Trialed)》**，判的依据是这一卷写没写。行首记号与完成同一套（有需留意的是 `!`，没有是 `✓`），
行尾照设计稿写「已分析，未写出」，照样展得开；**总览上的「完成」不数它**。设计稿本来就有这一档，
导出一串看得到这一卷与总览的序列钉住它。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 卷状态多一档预览过：答了「不写出」的那一卷落在这一档；答继续、答后面都写出、不等人的那几卷照旧完成
- [ ] 卷行行首记号与行尾照设计稿；总览上的「完成」不数它
- [ ] 那一卷展得开，每页结果照常
- [ ] 导出一串（从确认点答「不写出」到这一卷收摊、总览看得见）比整屏且绿；夹具不再把这种卷当完成喂
- [ ] `CONTEXT.md`《卷状态》添《预览过 (Trialed)》
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q769 — 设计稿的卷状态多一档 `trialed`（确认点上答了不写出的那一卷），库那一侧它收摊成完成

- **From:** 票 `session-redesign/05`
- **Kind:** 上游（03）说「七种 state 各有一档」的边界
- **Where:** `design.html` 的 `finishVol(run, 'trialed')`、卷行行尾「已分析，未写出」；`tests/fixtures/design/sequences/deciding-s.scene.json`（唯一带它的一份）；`src/session/live.rs` 的 `VolumeState`（八档，没有它）；`src/session/scene.rs` 的 `state_of`／`replay`
- **Why it did not block:** 事件流上它就是「确认点上答了做完再停、随后一卷跑完」——`Live` 记成完成，`decided` 记着那个字，`has_written` 不翻。夹具照这条路喂（答 `Finish`，写出环节一步不走），用例把它比作完成。11 个场景里没有它，只有一串序列有
- **What this ticket actually did:** 映射成 `Done`，答话记 `Finish`
- **Options:** ① 12 号票画卷行时从别处认出「完成但没写」（这一趟 `started_as` 是预览、那一卷收摊时 `Walking::writes` 为假——`Live` 今天不按卷记这一格）；② `VolumeState` 加第九档 `Trialed`（`volume_finished` 时按 `Walking::writes` 分）；③ 设计稿把它并进 `done`、行尾那句改由别的东西说
- **Recommend:** ②——行首记号与行尾那句照卷状态画（`CONTEXT.md`《卷状态》），少一档就得回头认别的格
- **Whose call:** 12 号票的实现者（等待确认）
- **处置：** 待处理。

#### Q674 — 翻面之后，答收尾（等于试算）的那几卷也数进「完成」

- **From:** 票 `no-false-line/04`
- **Kind:** 票面治的那个病换了一种卷还活着，本票没伸手
- **Where:** `src/session/draw/overview.rs` 的 `finished_and_skipped`——数的是 `Report::volumes`
  里不是跳过的每一卷；用例 `the_overview_changes_sides_when_the_first_volume_is_written_not_when_go_on_is_answered`
  末一问（答了收尾的卷三进了「完成 3 卷」）
- **Why it did not block:** 票面与 spec 第三节定的是**分岔谓词**换成哪一个、翻面点在哪，一个字没提翻面之后
  「完成」数什么；那个数与报告末尾那几小结同一份数据（`settled_row` 的文档），改它就是改两处的口径。
  这一趟里答收尾的卷**报告照出**、判定照有，说它「处理过」不算假，只是「完成」这个词在 `x` 起的一趟里
  含着「写了出去」，在混着答的一趟里不含。
- **What this ticket actually did:** 翻面点落在第一卷真写完（`Live::has_written`，逐卷的依据是
  `Walking::writes`），翻面之后照旧走 `finished_and_skipped`；用例把这一格的现状钉住，改口时它会红。
- **Options:** ① 保持：「完成」读作「处理过、收了摊」，与末尾那几小结一致；② 结论行执行那一副多一格
  「试算 N 卷」（答收尾的那几卷），`完成` 只数写出去的——要 `Live` 逐卷记「这一卷写了没有」
  （`Walking::writes` 已经有，收摊时收进一个数即可），措辞一句、快照一张；③ 同样逐卷记，但
  「完成」只数写出去的、答收尾的既不进完成也不进跳过——那几卷在这一行上消失，
  与抬头的「N 卷」对不上数。
- **Recommend:** ②。这一格的病正是「说了一句此刻不成立的话」，而「完成 3 卷」里有一卷盘上没有；
  数据 `Live` 上已经有一半，改动不出 `overview.rs` 与 `live.rs`。
- **Whose call:** 拍板的人（「完成」这个词在混着答的一趟里指什么）
- **处置：** 待处理。
