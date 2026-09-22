# 07 — 没做成的卷带卷级计时

**What to build:** `VolumeFailure` 带上卷级计时，「没做成」那一条事件跟着带。会话对没做成的卷改读它；自己量的那一份只留给
还在跑的卷与被立即停止掉的卷——那两种没有库那一份可读。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 事件流用例：没做成的卷那一条带着卷级计时，走过的那几段不为零
- [ ] 会话里没做成的卷，耗时那一列与目录行那个和读的是库那一份；会话自己量的那一份只剩还在跑与被立即停止掉的两种
- [ ] 会话的设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q808 — 没做成的卷、还在跑的卷与被立即停止掉的卷**没有卷级计时**，屏上那一列只好由会话自己量

- **From:** 票 `session-redesign/08`
- **Kind:** 票面没想到的第三种情形（「跳过的卷耗时照给」想到了，「没做成的卷耗时照给」没想到）
- **Where:** `src/report.rs` 的 `VolumeFailure`（只有 `volume` 与 `reason`，没有 `VolumeTiming`）；`tests/fixtures/design/snapshots/running.120x36.*` 第 22 行（`✗ 第11卷 … 1s 压缩包损坏`）与第 10 行（`集英社/海贼王` 那一枝的 `1m34s` **含**这 1 秒）；`src/session/live.rs` 新加的 `timings`／`elapsed_at`
- **Why it did not block:** 屏上那一列与目录行那个和都要它，少了它目录行那个数当场就差一秒——不是一格空白，是一个**错的数**
- **What this ticket actually did:** `Live` 顺带记一份逐卷计时（`timings`，与清单同序），**只在那一卷没有报告时才读**；收摊了的卷照旧走报告那一份（`VolumeTiming::elapsed`，只有库那一侧减得掉在确认点上等人的那一截）。场景夹具（`scene::replay`）为没有报告的那几卷把「此刻」推到它开卷与收手那两刻，量出来的正是场景数据说的那个数
- **Options:** ① 照现状：两份出处，读的时候有报告走报告 ② `VolumeFailure` 加一格 `VolumeTiming`（库那一侧改，`Event::VolumeFailed` 跟着带），会话这一份删掉 ③ 屏上那一列对这几种卷留空——那会让目录行那个和悄悄少一截
- **Recommend:** ②，单开一张小票；在那之前①站得住（读的先后写在 `Live::elapsed_at` 的文档里）
- **Whose call:** 拍板的人（②动的是库那一侧的公开类型）
- **处置：** 待处理。
