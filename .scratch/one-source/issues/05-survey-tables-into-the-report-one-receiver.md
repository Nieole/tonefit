# 05 — 清点那两张表一到就进报告，开工只一个方法收

**What to build:** 开工那一条一到，非漫画文件与无法访问的地方就写进报告；`Live` 旁边那两张删掉，屏上与退出时印到 stdout 的那一份读同一份。
`run_started` 与 `surveyed` 并成一个方法；只喂前一半的那八十余处用例改成喂整条（清单与卷数对得上）。屏上一格不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `Live` 用例：清点一到，报告上那两张就有，与屏上读的是同一份
- [ ] `Live` 上只有一个方法收开工那一条
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q745 — 开工那一条带的两张表摆在 `Live` 旁边、跑完才进报告：报告上那两张在清点与跑完之间仍是空的

- **From:** 票 `session-redesign/03`
- **Kind:** 票面「旧界面一格不动」与「事件流就是报告的增量」相抵的一处，选了前者
- **Where:** `src/session/live.rs` 的 `Live::surveyed`（把开工那一条带的两张表收进 `Live::non_volume_files` / `Live::unreachable_places` 两格，各带访问器）与 `Live::new` 里报告那两张空表的注释；`src/session/draw/overview.rs` 的出事行读 `report.unreachable_places.len()`（旧界面）
- **Why it did not block:** 当场进报告的话，真跑一趟时旧界面的出事行从清点起就多一句「发现无法访问 N 处」（从前要到跑完），而票面写明旧界面在切换那一票之前一格不动（review 指出）；摆在旁边，旧界面逐字不变，新界面的分区备注行从访问器读，一样在第一卷开工之前就画得出
- **What this ticket actually did:** 头一版当场进了报告，review 之后改成摆在旁边；`the_two_tables_are_at_hand_the_moment_the_survey_arrives` 钉着「报告上那两张跑完之前仍是空的」
- **Options:** ① 照现状，15 号票删旧界面时把两格并回报告（那时「事件流就是报告的增量」在这两张表上也成立，那条用例改成反着断言）；② 现在就并回报告，接受旧界面那一行提前出现
- **Recommend:** ①
- **Whose call:** 15 号票的实现者
- **处置：** 待处理。

#### Q964 — 停车场 Q745 那条「报告上那两张表等跑完才进」的理由随旧界面没了，行为还在

- **From:** 票 `session-redesign/15`
- **Kind:** 决定的依据过期
- **Where:** `src/session/live.rs` 的 `non_volume_files` / `unreachable_places` 两格文档，与用例
  `the_two_tables_are_at_hand_the_moment_the_survey_arrives`；Q745 的理由是「旧界面的出事行读的是报告上那一张，
  当场进报告会让它从清点起就多一句」
- **Why it did not block:** 屏上读的是 `Live` 旁边那两张（清点一到就有），报告上那两张跟着 `returned` 到，
  逐条相同；退出时印到 stdout 的那一份读报告，那时这一趟已经收场。两条路今天给的字一样。
- **What this ticket actually did:** 行为一格没动；只把文档与断言消息里「旧界面的出事行」那半句换成指向 Q745 与本条。
- **Options:** ① 维持：报告只在 `returned` 时换上那两张；② 清点一到就写进报告，`Live` 旁边那两张删掉，
  屏上与 stdout 读同一份
- **Recommend:** ②。两份逐条相同的表摆在一起，唯一的理由已经不在了；合成一份少一处「两份会不会走散」。
- **Whose call:** 下一张碰 `Live` 的票
- **处置：** 待处理。

#### Q747 — 开工那一条在 `Live` 上分成两半：`run_started(volumes, steps)` 与 `surveyed(清单, 两张表)` 两个方法

- **From:** 票 `session-redesign/03`
- **Kind:** 走了哪条路（一条事件、两个接收方法）
- **Where:** `src/session/live.rs` 的 `Live::observe` 开工那一支、`Live::run_started`、`Live::surveyed`；旧界面与状态机的用例里 `live.run_started(n, steps)` 八十余处，只喂前一半
- **Why it did not block:** 改成一个方法就要改那八十余处、而且它们喂的清单会是空的（与卷数对不上）；05 号票的夹具两半都喂，`observe` 那一支两半一起转
- **What this ticket actually did:** 两个方法，`observe` 里一条事件转两次，文档写明为什么分
- **Options:** ① 照现状，15 号票删旧界面时顺手并成一个方法；② 现在就并，八十余处用例加 `&[], &[], &[]`
- **Recommend:** ①
- **Whose call:** 15 号票的实现者
- **处置：** 待处理。
