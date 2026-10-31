# 09 — 命令行报告两句说对

**What to build:** **结束方式那一句去掉「按停止」**：写「停下（立即停止）」「停下（做完再停）」，清点途中那一种同理；命令行报告末尾与会话退出时 stdout 那一份同一处出。命令行 **`--brief` 在默认那条路上，目录那一行只拿掉「逐页」那几笔**，跳过、没做成、覆盖顶死照数——那一格在不在仍由「说没说出卷数之外的东西」定；`--envelope` 那条路不变。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 报告那一层的用例：结束方式两级各一条、清点途中一条；留一条反着钉的断言：报告里不再出现「按停止」
- [x] 会话跑着时 `C-c` 退出，stdout 那一份末尾是「停下（立即停止）」那一句（起真进程或报告用例钉住）
- [x] `--brief` 默认那条路：一枝全部跳过时目录行说出跳过几卷、不出「逐页」；`--envelope` 那条路分布照旧
- [x] 黄金快照一个字节不动
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

会话里按的是退出，报告说你按了停止；`--brief` 重跑一趟，一枝全部跳过与全部重做印出来一模一样。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1146 — 会话里 `C-c` 退出时印到 stdout 的那一份，末尾说「按停止停下（立即停止）」，而用户按的是退出

- **From:** 票 `say-and-stop/02`
- **Kind:** 共用一副排版带出来的连带变化（评审 Spec 轴提的）
- **Where:** `src/render/plain.rs` 的 `report`（末尾接 `render::outcome`）；`src/session/run.rs` 的 `Running::report` 与 `leave`（跑着时退出推的是立即停止）
- **Why it did not block:** 那一句说的是实情：退出会话跑着时走立即停止（`CONTEXT.md`《退出会话》），当前卷确实丢了、剩下的确实没开工；
  会话屏上一格没变（设计稿不读这一句）。
- **What this ticket actually did:** 结束方式那一句放进 `plain::report`，命令行与会话退出时的 stdout 那一份同一处出（单一出处）。
  于是跑着时按 `C-c` 退出会话，stdout 那份报告末尾多一行「按停止停下（立即停止）：<卷> 做到一半丢掉了……剩下 N 卷没开工」。
  从前那一份说不出这一趟没跑完。
- **Options:** ① 现状；② 只让命令行印（`main.rs` 自己接在报告后面），会话那一份不接；③ 措辞里不说「按停止」，改成「停下（立即停止）」，两种按法都对得上。
- **Recommend:** ③。①的「按停止」三个字对退出会话那一种不准；②让会话退出时那一份又说不出这一趟没跑完，正是 Q260 那个毛病。
- **Whose call:** 协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：结束方式那一句去掉「按停止」，写「停下（立即停止）」，命令行与会话共用。

#### Q1388 — 默认那条路上目录行整格不在，跳过、没做成、覆盖那几笔也跟着不在：`--brief` 默认那一趟上「跳过几卷」再没有一处说得出

- **From:** 票 `say-and-stop/08`
- **Kind:** 票面的理由只对了一半（「那里每卷都是逐页」）；落地时走的那条路
- **Where:** `src/render.rs` 的 `directory`（`envelope` 那个参数）与 `base_of`；末尾那几小结在 `tail`（没做成与隔离点名，跳过不点）
- **Why it did not block:** 票面明写「整格不在」，照办不会写出错的东西；换另一条只动 `directory` 那一个 `if` 与一条用例。
- **What this ticket actually did:** 照票面，默认那条路上整格不在。那一格在默认路径上除了「逐页 N」，还数着幂等命中而跳过的、没做成的、覆盖顶死的那几卷：没做成的照旧由末尾那几小结点名，**跳过几卷在 `--brief` 默认那一趟上一处都不剩**——重跑一趟 `--brief`，一枝 12 卷全部跳过与全部重做印出来一模一样（「库  12 卷」）。不折那一副照旧每卷一句「跳过 之前转换过……」。用例 `the_directory_row_spreads_bases_only_on_the_envelope_path` 夹具里摆了一卷跳过的，钉的正是「跳过的那几卷也不例外」。
- **Options:** ① 照现在，默认那条路上整格不在；② 默认那条路上只拿掉「逐页」那几笔，其余照数（「库  12 卷 · 跳过 11」）——在不在场仍由「说没说出卷数之外的东西」定；③ 跳过几卷另起一格或末尾另起一小结。
- **Recommend:** ②。票面嫌的是「逐页 N」与卷数那一格说的是同一件事，跳过与没做成那几笔说的不是；`--brief` 正是几百卷重跑一趟时拿来扫一眼的那一副，「这一枝有没有重做」恰好问在这里。
- **Whose call:** 拍板的人（Q706 的那个决定要不要收窄）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：`--brief` 默认那条路上只拿掉「逐页」那几笔，跳过、没做成、覆盖照数。


## 落地记录

- `src/render.rs` 的结束方式改为「停下（做完再停）」「停下（立即停止）」「清点途中停下」，包括没有剩余卷、没有丢弃卷的短句分支；命令行与会话退出仍共用 `plain::report`，不新增措辞出处。
- 默认路径的目录分布只过滤语义上的 `VolumeVerdict::PerPage`，不靠认字符串；跳过、覆盖、没做成照原先的分布规则计数排序，全部逐页时整格不在场，`--envelope` 不改。代价是默认路径现在也逐卷计算状态分布；没有新增依赖、参数或公共 API。
- 用例接缝按 spec《接缝 ②》：报告纯函数覆盖两级停止、清点途中两级、卷边界没有丢弃卷、剩余零卷；`--brief` 覆盖全跳过、全逐页、混合跳过／覆盖／失败与 `--envelope`。`session::run` 既有退出用例在真线程等确认后调用 `leave`，断言退出报告收尾及不含「按停止」；会话生产代码不改。`tests/stop.rs` 真进程三条跟随新措辞，并各自反向禁止「按停止」。`tests/events.rs` 同步那句报告文字的注释；`src/specs/cli.md` 记下当前契约。
- 实际 red：`/tmp/hs-09-red-outcome.log` 中 `Stopped(Finish)` 实得「按停止停下（做完再停）…」、期望「停下（做完再停）…」；`/tmp/hs-09-red-leave.log` 的退出报告因仍含「按停止」失败（单靠后缀正断言抓不到它）。`/tmp/hs-09-red-directory.log` 实得「库  3 卷」、期望「库  3 卷 · 跳过 1」。均在实现之前跑到断言红。
- 实际 reversal：把过滤守卫的 `!envelope` 反成 `envelope`，目录用例红为「库  3 卷 · 逐页 2 ⋅ 跳过 1」（`/tmp/hs-09-mutation-directory-guard.log`）；把生产措辞前缀加回「按停止」，退出报告的反向断言红（`/tmp/hs-09-mutation-exit-wording.log`）。两次均 exit 101，均已还原并重验绿。
- 验证：报告层 71 通过、退出报告用例 1 通过；真进程停止 3 通过。另跑 events 36、exit_code 13、golden 2、session 2、single_source 12、stop 3 全通过（`/tmp/hs-09-final-integration.log`）。`git diff --check` 通过；黄金快照、设计稿、导出夹具、场景夹具、`main.rs` 和 `.gitattributes` 均无改动。
- 所有 Cargo 均经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo …`。共享 target 在增量构建时曾跑到旧措辞可执行文件（`/tmp/hs-09-final-stop*.log`）；后续显式 `CARGO_INCREMENTAL=0`，真进程集成用例重验通过，三条 gate 与 polish 同样禁用增量。未改资源协议或清理共享 target。
- 最后一条验收框诚实留空：闸门 1、2 唯一失败均为 spec 明确排除的 macOS 基线 Q995（`many_archive_volumes_never_hold_more_than_the_one_being_processed`，阳性对照重命名实得 0、期望 96）；不修、不弱化、不跳过。闸门 3 与 polish 全绿。gate 会在 Q995 停下，未运行到的相关集成测试另按上一条单独点名跑完。
- 交付在 `hs-09` 工作树，起点与当前 HEAD 都为 `e9309bb25a28507349955235703393a1a42ddc97`；最终读取集成 HEAD 已推进为 `d252f9a5457f0b0ff8da0f914fe620ffb8d35e70`（票 01 的截法收口，改动文件与本票不重合）。按上级 worker「git 只读」指令未合并、未提交，全部改动保持 UNCOMMITTED，未操作索引；由协调人按票序集成。

## 数

命令均加 `CARGO_INCREMENTAL=0 python3 /tmp/tonefit-honest-screen-cargo.py` 前缀，日志在仓库外。

- `cargo xtask gate 1`：红，合计 **755 通过 1 失败；lib 255 / bin 481**。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.71s`。唯一红 Q995。日志 `/tmp/hs-09-gate1.log`。
- `cargo xtask gate 2`：红，合计 **614 通过 1 失败；lib 255 / bin 340**。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 51.67s`。唯一红 Q995。日志 `/tmp/hs-09-gate2.log`。
- `cargo xtask gate 3`：绿。末行：``Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.85s``。日志 `/tmp/hs-09-gate3.log`。
- `cargo xtask polish`：fmt、默认 clippy、无默认特性 clippy、doc 四条全绿；doc 既有 **15 warnings**，没有改库文档或增加告警。日志 `/tmp/hs-09-polish.log`。

### 当前集成复验更正（fresh merger，基于 c6d9ba6）

- 上述《数》与 worker 验证只描述源工作树 `hs-09`，不是当前集成的验证结果；尤其旧共享 target 的结果不能替代当前构建。本次从源 `e9309bb` 的 UNCOMMITTED delta 生成 `/tmp/hs-09-transfer.patch`，先 `git apply --check /tmp/hs-09-transfer.patch` 再 `git apply /tmp/hs-09-transfer.patch`，六个预期路径均无冲突，不覆盖整文件、不改现有集成修改。
- 以下每条 Cargo 命令均在当前集成运行，完整前缀为 `CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py`（序列锁 + workspace pass-through fingerprint）：
  - `cargo test --bin tonefit render::tests`：71 通过、0 失败，428 filtered；日志 `/tmp/hs-09-current-render.log`。
  - `cargo test --bin tonefit session::run::tests::leaving_while_the_decision_point_waits_throws_that_volume_away -- --exact`：1 通过、0 失败，498 filtered；日志 `/tmp/hs-09-current-exit-report.log`。
  - `cargo test --test stop --test events --test session --test golden`：events 36、golden 2、session 2、stop 3，合计 43 通过、0 失败；完整成功日志 `/tmp/hs-09-current-integration-rerun.log`。第一次同命令受工具 200 秒 timeout 中止，只有 events 36 已完成，golden 未完成，不能算该轮全绿（`/tmp/hs-09-current-integration.log`）；随后以 900 秒预算完整重跑。
- `git diff --check` exit 0（`/tmp/hs-09-current-diff-check.log`）；既有 `.gitattributes` diff 与应用前保存的 `/tmp/hs-09-preserve-attributes.patch` 逐字节比较一致。黄金快照没有修改，当前 golden 两条实跑通过。
- 本次只做上述 targeted 复验，没有重跑当前集成的 full gates/polish，也没有重做 mutation；源工作树的 Q995/gate 数字不冒充当前集成结论。所有变更未提交，最终 full gates 留给协调人。
