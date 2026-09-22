# 07 — 真实素材上跑一遍（opt-in）

**What to build:** 合成夹具证明得了形状，证明不了它在真页上站得住。
`tests/smoke.rs` 那条 opt-in 的路上加一格：`TONEFIT_SAMPLES` 指过来时，
在几页真页上出样张，断言不崩、文件数对、判定那一张与 `run` 一致。

**没有素材时照旧明确跳过。**那一条自带 harness 就是为这件事
（内建 harness 的跳过印出来是 `test ... ok`，读 CI 日志的人会把它当证据）——
新加的这一格走同一条路，不许挂回内建 harness。

真语料里跨页与彩页都有，这一格因此顺带把 04 与 05 在真页上过一遍。

**Blocked by:** 03 — 处理选项与 `--preset`；04 — 一张跨页出两叠；05 — 彩页与尺寸未贴合屏幕

**Status:** ready-for-agent

- [ ] `tests/smoke.rs` 加一格：`TONEFIT_SAMPLES` 指过来时在几页真页上出样张
- [ ] 断言：不崩、文件数 = 候选数 + 1、判定那一张与 `run` 写出的一致
- [ ] 至少覆盖一张跨页与一张彩页
- [ ] 没有素材时**明确跳过**，走自带 harness 那条路，印出来分得清「跳过」与「跑过了」
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面这一条由停车场转来（`/settle` Q206–Q994）：本票往 `tests/smoke.rs` 那条 opt-in 的路上加格，先定那条路吃哪一份素材。

#### Q471 — `TONEFIT_SAMPLES` 指向本机 `_samples` 根时，`tests/smoke.rs` 当场红，而且要跑一小时四十分

- **From:** `metric-recalibration/07`（给闸门设了 `TONEFIT_SAMPLES`，想让 `perceptual` 那五条全跑）
- **Kind:** 路过发现（一条 opt-in 的闸在本机上从来没被真正跑过，一跑就红）
- **Where:** `tests/smoke.rs` 的 `real_material_runs_through_the_pipeline`（121 行那条 `assert_eq!`）
- **Why it did not block:** **本票不需要它**。`docs/agents/gate.md` 写着口径：
  「`smoke` 在未设 `TONEFIT_SAMPLES` 时印一行跳过、贡献 0 个通过，那是正常的」——
  闸门的标准跑法本来就不设它。设它是我自己加的严，不是票面要求。
- **What this ticket actually did:** 闸门按**文档那条口径**跑（不设 `TONEFIT_SAMPLES`），
  `tests/perceptual.rs` 那五条**另跑一趟**、单独带素材。红的成因记在这里，一个字没动 `smoke`：

  1. **断言与素材的形态对不上。**它断 `report.volumes.len() == inputs.len()`，
     而 `volumes(root)` 取的是**顶层那几项**（本机 9 项），`tonefit::run` 会**发现下去**
     （ADR 0009），报告里出来 **482 卷**。素材根底下摆的是「目录卷的目录」时，这条恒不成立。
  2. **它走 `Mode::Process`，真的把整库处理了一遍。**闸① 因此跑了 **1 小时 43 分**
     （00:38:39 → 02:21:59），而其余十八个测试二进制合计不到 6 分钟。
  3. **它把整库写出去了一遍：盘上空闲从 449.2 GB 掉到 416.6 GB 的那一夜，它是其中一笔。**
     `Workspace` 用的是 `tempfile::TempDir`，panic 展开时 Drop 把那份输出删了——
     `%TEMP%` 底下今天一个 `.tmp*` 都没剩（123 个全是 9/4 的空壳），
     `_samples` 顶层 mtime 也全部 ≤ 9/9 22:19，**一手素材一个字节没被写过**。
     那 32.6 GB 是三棵 worktree 的 `target/`（本树 9.26 ＋ `q2-nfl07` 14.02 ＋ `q2-tpr07` 14.03），
     不是 smoke 的残留。**但它确实在跑的过程中占过几十 GB**——
     下一个想给闸门加严的人要知道，代价不只是那 1 小时 43 分。
  4. 两件合起来说明**这条闸在本机上从来没绿过**——它一直靠「没设环境变量」跳过，
     没有人验过它指过来时会怎样。
- **Options:** ① 记下，闸门照文档跑（不设它），`perceptual` 另跑；
  ② 断言换成「报告里的卷数 ≥ 点名数」或按发现之后的卷数比对——**改得动，但改完它验的东西变了**；
  ③ 让 `TONEFIT_SAMPLES` 指一个专门的小目录（几卷），本机 `_samples` 根不再直接喂给它。
- **Recommend:** ③ 加 ①。②把断言放松到「≥」等于把「点名了几个卷就该得到几个」这句话删掉，
  而那句话正是它要验的；③保住断言、也保住跑得完，代价是要约定一个目录并写进模块文档。
  在有人拍板之前照①走：闸门不设它，`perceptual` 单独带素材跑——**那五条是本票真正要的**，
  而它们不吃 `Mode::Process`。
- **Whose call:** 拍板的人（`smoke` 该吃哪一份素材，以及它断的到底是「点名」还是「发现」）
- **处置：** 待处理。
