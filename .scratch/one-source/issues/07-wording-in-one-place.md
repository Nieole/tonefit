# 07 — 措辞只写一处：隔离目录名、卷列表那一格、字形宽度那一关

**What to build:** - **隔离目录的名字**：库里那个常量公开，会话那一句读它（对外契约多一个常量，`cargo doc` 的告警水位跟着核一次）；
- **卷列表那一格**：跳过的卷读措辞层那一格，没做成的卷读措辞层那一行；那一处「期待成为死代码」的标注拆掉；
- **字形宽度那一关**：改问措辞层出的全部格（原样那一档照旧不问），不再跟着屏上摆成列的那几格走。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 会话里不再手写隔离目录的名字与「跳过」「没做成」两个词
- [ ] 字形宽度那一关问的是措辞层出的全部格；拿一个歧义宽度字形放进一格不在屏上的格里去试，它红
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q870 — 隔离目录那个名字在会话这一头写死了第二份，而库那一份是私有的

- **From:** 票 `session-redesign/11`
- **Kind:** 同一个字面有两处
- **Where:** `src/session/shell/pages.rs` 的 `tally_line`（`"这一卷输出在 _isolated/"`）；出处是 `src/lib.rs` 的 `const ISOLATED_DIRECTORY: &str = "_isolated"`——它**没有 `pub`**，而会话住在另一个 crate 里，够不着
- **Why it did not block:** 那一整句是**界面层自己的措辞**（设计稿 `drawPages` 写死的一句），不是报告那一格：库那一句是「隔离 N 页读不出：这一卷整卷写到隔离目录 <整条路径>，读不出的页用空白页占位，页码顺序不变」，一整条路径摆不进抬头那一行，而屏上那儿要的是一个短标签。卷行行尾摆的仍是库那一整句（`shell::list` 的 `sentence`），ADR 0016 那一条一处没破
- **What this ticket actually did:** 照设计稿逐字写了那一句，并在代码旁注明它是界面层的措辞、与卷行行尾那一句分工不同
- **Options:** ① 照现状：一个目录名两处，改名时要两处一起改 ② `ISOLATED_DIRECTORY` 提成 `pub`，这一头写 `format!("这一卷输出在 {ISOLATED_DIRECTORY}/")`——那是一次**对外契约扩大**，`cargo doc --no-deps` 那 15 条的水位要跟着核（Q183 那一笔是同一种代价） ③ 从 `report.output` 反解出那一截：**不取**——那是「表回头去认字符串」，ADR 0016《后果》拦的正是它
- **Recommend:** ②，与别的「库内私有常量被界面层需要」的账一起判
- **Whose call:** 拍板的人（库的对外契约）
- **处置：** 待处理。

#### Q961 — 卷列表那一格的「跳过」「没做成」写了第二份，`render::tally_column` 如今只有用例在读

- **From:** 票 `session-redesign/15`
- **Kind:** 单一出处裂开（措辞在画法那一层另写了一份）
- **Where:** `src/session/shell/list.rs` 卷行那一支的 `why_nothing`（手写 `"跳过"`、`"没做成"`）；
  对面是 `src/render.rs` 的 `tally_column` 与 `why_nothing_judged`——文档写着
  「「跳过」与「没做成」只有 `why_nothing_judged` 一处」
- **Why it did not block:** 屏上的字今天一个不差：设计快照逐格钉着那两个词，两处写的是同一串。
  旧画法退场之后 `tally_column` 在非测试那一趟没了读者，本票给它挂了
  `cfg_attr(not(test), expect(dead_code, …Q961))`，用例（render 自己那几条、场景夹具的
  `on_the_grid`）照旧读它。
- **What this ticket actually did:** 没动 `shell::list`；只让 `tally_column` 留着、挂上那一句期待，
  并把它文档里「读它的只有会话」改成此刻的读者。
- **Options:** ① 卷列表那一格改读 `render`：跳过的卷有卷报告，走 `tally_column(&render::volume(..))`；
  没做成的卷走 `render::failed_volume` 那一行——两个词从此只有一处，`tally_column` 的期待自己报没用上、拆掉；
  ② 认下两份，删掉 `tally_column`，`why_nothing_judged` 只剩目录那一级读，文档那句「只有一处」改掉
- **Recommend:** ①。ADR 0016「一套措辞」要的正是这个；代价是卷行每帧多拼一次那一卷的卷级行，
  只在跳过与没做成那两种卷上发生，比整棵树每帧已经做的事小得多。
- **Whose call:** 下一张碰卷列表那一格的票（或 `session-redesign/17` 量延迟时顺手）
- **处置：** 待处理。

#### Q962 — 「摆进列里的字形宽度稳不稳」那一关跟着屏上的表缩小了：裁白边、彩页转灰、跨页、画质分那一串、页数、卷数、统一档位分布不再被问

- **From:** 票 `session-redesign/15`
- **Kind:** 闸门的覆盖面随实现收窄（是不是该收窄，是一个决定）
- **Where:** `src/session/columns.rs` 的 `wording_cells`（从前从旧界面那三张表——目录表、卷表、
  逐页表——导出十三格，如今从卷列表那棵树与每页结果导出六格：灰阶分布、尺寸、缩放、判定、理由、
  判定那一档的分）；读它的是 `src/render.rs` 那条
  `every_glyph_this_layer_puts_in_a_lined_up_cell_is_the_same_width_on_any_terminal`
- **Why it did not block:** 那一关的规矩说的是「**摆进列里**的字形不许是歧义宽度」（`CONTEXT.md`《格》），
  而那七格在今天的屏上不成列——命令行那一份是成句的散文，错一格不牵连别人。按规矩的字面，
  收窄是对的；删掉旧表之后照旧导出旧表的列，就等于名单与屏各说各的。
- **What this ticket actually did:** 名单改从新两张表导出；那条 render 用例钉的两格换成「尺寸、缩放」，
  夹具补一处让「判定那一档的分」真摆得出来；`columns` 那条字面出处用例改问新两张表。
- **Options:** ① 照今天这样，名单只跟屏上的列走；② 另立一份「措辞那一层出的格一律要稳」的规矩，
  那一关改问 `render` 出的全部格（原样那一档照旧不问）——不再依赖屏上摆没摆成列
- **Recommend:** ②，但不急。那七格的字形今天全是稳的，收窄没有放进任何一个坏字形；
  ②的好处是下一次屏上换表时那一关不再跟着缩。
- **Whose call:** 拍板的人（`CONTEXT.md`《格》那条规矩的管辖面）
- **处置：** 待处理。
