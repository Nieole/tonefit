# 06 — 清点途中停下的结束屏

**What to build:** 清点途中按停止之后，结束屏抬头写「**已停止 ⋅ 清点途中停下，一卷都没开始**」，卷列表回到开跑之前那一副（处理路径列表，带「＋ 添加路径」）。设计稿先补这一屏、重导，再让会话跟上。CONTEXT《总览》《结束方式》《清点摘要》已在 `/settle` 里改对。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿补一串「清点中按 `s` 之后」，`npm run export` 重导，`npm run check` 绿
- [x] 会话照它画：抬头那一句与卷列表比整屏且绿
- [x] 留一条反着钉的断言：清点途中停下的那一趟不再说「处理到第 0 卷」
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

抬头说「处理到第 0 卷」，读的人以为停在了第一卷开头，而清点根本没走完。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1143 — 会话里清点途中按停止，结束那一屏设计稿里没有：卷列表停在开跑之前那一副，抬头说「处理到第 0 卷」

- **From:** 票 `say-and-stop/02`
- **Kind:** 本票改出来的一副屏，设计稿没覆盖
- **Where:** `src/session/live.rs`（没有开工那一条，`roster` 空着、`finished` 是 0）、`src/session/view.rs` 的 `lines`
  （`surveyed` 为假，列的是处理路径）、`src/session/shell/overview.rs` 的 `ended_title`
- **Why it did not block:** 票面对会话只要「收到它只答话、清点那一屏一格不变」，那一屏没变（`survey-s` 照绿、`npm run check` 照绿）；
  这一副是**之后**的那一屏。
- **What this ticket actually did:** 没动会话。从前清点中按 `s`，清点走完才停，开工那一条到了、树拼得出来，结束抬头「已停止 ⋅ 处理到第 0 卷」；
  现在清点当场收手，开工那一条不来，结束时卷列表仍是处理路径那一副（没有「＋ 添加路径」），抬头同一句「处理到第 0 卷」
  （立即停止是「第 0 卷未保存」，从前就是这一句）。
- **Options:** ① 现状；② 设计稿补一串「清点中按 `s` 之后」的结束屏，会话照它说（例如「已停止 ⋅ 清点途中，一卷都没开始」）。
- **Recommend:** ②，归设计稿那一批（`design-parity`）：先改设计稿、重导，再让会话跟上（ADR 0019 决定第 13 条）。
- **Whose call:** 设计稿那一侧（协调人排票）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：结束屏抬头「已停止 ⋅ 清点途中停下，一卷都没开始」，卷列表回到开跑之前那一副；设计稿先补一屏。


## 落地记录

在 `hs-06` 上从 `2e181ba0cc4bc4cc381f2ea4d9bf646a1658b8d2` 落地，仅交付本票；改动未提交。

- 先改设计稿：清点中按 `s` 后模拟清点收手，结束标题为「已停止 ⋅ 清点途中停下，一卷都没开始」，总览不画开工后的进度／当前卷，列表回到处理路径并带「＋ 添加路径」。导出 `survey-s-advance-120` 与 `survey-s-advance-80` 两串（`s`、推进 30 秒，让临时回话到点）。导出端给这趟空清单，不伪造开工数据。
- Rust 跟稿：只在确实收到清单时拼树；没有清单的停止结束屏用路径列表与开跑前总览正文，恢复添加行，抬头不报第零卷。场景回放走 `Live::returned`，不造清点途中本来就没有的开工／结束事件。
- 两道验收：终端键码重放两串、逐格比主稿和窄屏；另一条真正调 `tonefit::run`，在 `Surveying` 回答停止，事件交 `Live::observe`、真实报告交 `Live::returned`，逐格比同两张结束屏。终端重放另反断言禁止「处理到第 0 卷」「第 0 卷未保存」「总进度」「当前卷」，且要求添加路径。报告用时在停止时冻结，因此重放用例画结束后的屏钟，等停止回话到点，而不伪改报告用时。
- 成本：设计导出多两串（每串场景数据、文字、样式），夹具补一条没有开工事件的报告回路；未加依赖、抑制或第二套措辞出处。黄金快照与 `.gitattributes` 未动，既有导出快照未动。

### 红 → 绿与按反

日志均在仓库外 `/tmp/hs-06-*.log`：

- `npm ci` 后 `npm test`：新增设计用例先红，缺少精确停止标题（`npm-red.log`）；改稿、export/check 后绿（`npm-final.log`，8/8）。最初依赖未装的执行不算行为红证据。
- `cargo test survey_stop_returns --bin tonefit` 初次实际执行 1 条、失败 1 条，结束整屏不符（`rust-red.log`）；实现后绿（`rust-green.log`）。真实报告用例也实际通过（`real.log`）。
- 分别按反清点停止标题守卫、是否已有清单才拼树的守卫、移除结束后添加行、按反结束正文守卫：`reversal-title.log`、`reversal-tree.log`、`reversal-add.log`、`reversal-body.log` 各跑 2 条、各失败 2 条；全部还原。最终整会话 `session-final.log`：337 通过、0 失败、1 忽略。
- 最终 `npm run export`、`npm run check`、`npm test` 已执行；`git diff --check` 绿。

### 数

所有 Cargo 命令均经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...` 串行运行，且设置 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-06-rustc-workspace.sh`。

| 检查 | 实际结果（日志） |
|---|---|
| 闸门 1 | 红；合计 768 通过、1 失败，lib 255 / bin 494；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.60s`（`gate1.log`） |
| 闸门 2 | 红；合计 617 通过、1 失败，lib 255 / bin 343；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.31s`（`gate2.log`） |
| 闸门 3 | 绿；`Finished dev profile [unoptimized + debuginfo] target(s) in 3.02s`（`gate3.log`） |
| polish 最终状态 | 红在 fmt：基线 `src/session/shell/canvas.rs` 文件尾多一空行，后面三项未跑（`polish-final.log`）。不为本票修改该无关文件。 |
| polish 其余项实际执行证据 | 先前 `cargo fmt` 顺带去掉该基线空行时，整趟 polish 四项通过（`polish.log`）；默认 clippy 6.53s、无默认特性 clippy 5.60s，doc 既有 15 warnings。随后恢复 canvas 原字节，故不把这一趟 fmt 绿当最终树全绿。 |

闸门 1、2 均只卡既有 Q995：`tests/concurrency.rs:755` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，macOS 重命名断言实际 0、期望 96。依批准不修、不削弱，所以「三条全绿」复选框保留未勾。额外基线 fmt 问题也如实保留；本票功能交付完成，不声称全仓检查全绿。

### 集成核验

集成时移除了前票移动 `canvas::elided_row` 后留下的文件尾空行（无行为变化），不再把本批引入的格式问题留给下一票。`RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo xtask polish` 四项通过，只有既有 `fetch_update` 弃用告警与 15 条文档告警；日志 `/tmp/hs06-integration-polish.log`。合入时两条清点停止定向用例也通过，日志 `/tmp/hs-06-merge-targeted.log`。


## 落地记录 — rc_e1f8d384 空清单修正

在 `hs-empty-survey-fix`，起点与当前 HEAD 均为 `c97f23df629d54deed61b996ee18a351f9937af0`；改动未提交。

独立复核指出本票的 `!live.roster().is_empty()` 把「没有收到开工事件」与「收到零卷的开工事件」混为一谈，破坏 `TaskView::surveyed` 已有的零卷保证。此前记录的「只在确实收到清单时拼树」实际实现不成立，现纠正：

- `Live` 明确记住开工事件是否到达，初值假，仅 `run_started` 置真；不从卷数、备注数量或返回报告反推。
- `watch_the_run` 以此决定是否拼树；总览的结束标题和正文也读同一事实。没有开工事件的停止仍保留已经批准的清点停止屏。
- `src/specs/session.md` 明确零卷事件仍切树、备注卡及问题跳转保留。没有新增 UI 决定，没有修改设计稿或导出文件；恢复的是既有已批准行为。
- 终端回归通过真实 `tonefit::run` 的公共事件 → `Live::observe` → view → 终端绘制/输入接缝：临时目录内一份 `readme.txt`、一个无读取权限的子目录，真实报告零卷、非漫画文件 1、无法访问 1；验证两条备注在屏上、`]d` 到无法访问备注、Enter 打开两种说明卡、Esc 关闭，且禁止「清点途中停下」「＋ 添加路径」「开始后会扫描」假话，要求总进度正文。权限在运行返回后恢复。
- 零卷库即使观察者在开工回答 Finish 也会完成，因此测试的第二轮仅将返回报告的结束方式设为 Stopped(Finish)，显式覆盖「已有开工事件后停止」的标题分支；不把它冒称为真实库产生的停止报告。事件与备注两轮都来自真实库。该权限夹具标记 `cfg(unix)`，Windows 不运行这条；原有停止屏两条整屏用例继续通过。

### 红 → 绿与按反

日志前缀 `/tmp/hs-empty-survey-`，全在仓库外：

- `red.log`：实际执行新增回归 1 条、失败 1 条；真实报告已拿到零卷及两类备注，屏上却为路径列表，缺少「已忽略 1 个文件」。此前编译/造错输入失败不算行为红证据。
- `green.log`：新增回归实际 1 通过。
- `reversal-tree.log`：恢复旧非空清单守卫，实际 1 失败（备注被藏）。
- `reversal-title.log`：只恢复旧结束标题的空清单判断，实际 1 失败（误说清点途中停止）。首次按反未覆盖到真实库不产生的停止结局，补上显式停止报告轮次后重新按反才红；此处记录的是最终有效证据。
- `reversal-body.log`：只恢复旧结束正文的空清单判断，实际 1 失败（总览退回开跑前）。三次按反全部恢复后再跑完整会话。
- `session.log`：`cargo test session:: --bin tonefit`，339 通过、0 失败、1 忽略，包括两条原有清点停止结束屏用例。
- `npm-ci.log`、`npm-check.log`、`npm-test.log`：本 worktree 装依赖后 `npm run check` 与 `npm test` 通过；check 逐字节一致，测试 10/10。未装依赖时的 check 失败不是产品红证据。未改依赖清单/锁文件。
- `git diff --check` 通过；黄金快照、`.gitattributes`、依赖和设计夹具均未动。

### 数 — 修正后最终检查

所有 Cargo 命令（含 fmt、定向测试及按反）都带 `CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-empty-survey-rustc-workspace.sh`，经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...` 串行运行。三闸门分别执行，不因第一条红跳过第二、三条。

| 命令 | 实际结果及日志 |
|---|---|
| `cargo xtask gate 1` | 红；770 通过、1 失败，lib 255 / bin 496；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.57s`（`gate1.log`） |
| `cargo xtask gate 2` | 红；617 通过、1 失败，lib 255 / bin 343；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.85s`（`gate2.log`） |
| `cargo xtask gate 3` | 绿；`Finished dev profile [unoptimized + debuginfo] target(s) in 0.96s`（`gate3.log`） |
| `cargo xtask polish` | 四项全绿：fmt、两种 clippy、doc；文档仍为 15 warnings（`polish.log`） |

闸门 1、2 唯一失败仍为批准范围外的 Q995：`tests/concurrency.rs:755`，`many_archive_volumes_never_hold_more_than_the_one_being_processed`，macOS 重命名断言实际 0、期望 96。未修复或削弱它；三条全绿复选框仍不勾。既有 `fetch_update` 弃用告警保留。功能修正完成，剩余限制为上述基线红与 Unix 权限夹具不覆盖 Windows。


## 落地记录 — scene-survey-fix 场景回放修正

本次修正 Status: resolved。工作树 `/tmp/tonefit-honest-screen/scene-survey-fix`，分支 `hs-scene-survey-fix`，起点 `1484a76`；不改写以上历史验收记录，全部改动未提交。

最终 Spec 复核指出：生产路径已经以明确的 `surveyed` 标志区分开工与清点途中停止，但场景回放仍以空清单提前返回，零卷完成的两张备注表没有交给 `Live::run_started`。此前关于夹具完整性的记录不能覆盖这一缺口。

- 设计先行：新建语义场景 `zero-volume`，清点自然发现零卷、无法访问 1 处与非漫画文件 1 个；树允许没有卷的目录及其备注，不造假卷。导出主稿与窄屏完整快照，以及 `]d`、打开无法访问卡、打开非漫画文件卡各两种尺寸共六串；保留原清点停止序列。
- 导出必填布尔 `run.surveyed`：初始清点为假，清点完成置真，清点停止不置真；零卷完成仍为真。清点停止导出空清单和空备注，零卷完成保留两类备注。`src/specs/session.md` 记录契约。
- Rust 夹具严格反序列化布尔（缺失、null、字符串、数字均拒绝），回放据标志分支；完成零卷走原有 tables / RunStarted / RunFinished，清点停止仅返回报告。测试直接使用新导出数据的 `Scene::named` 与终端序列，不绕过回放去注入生产事件。
- 成本：无新依赖、无新产品范围；现有文字/样式网格不变，新增两张快照与六串。黄金快照、`.gitattributes`、依赖文件不动。

- 零卷设计补齐此前未能表达的空目录、零分母 0% 与空目录的等待标记/颜色；不改变既有任何文字或样式网格。新景复用已有 `~/漫画库/私藏`，避免假盘并集新增家目录候选干扰旧补全序列。备注错误原因取 `Permission denied (os error 13)`，未扩展到既有折行差异的修复。
- 本次新增 Rust 接缝测试 4 条、JS 测试 1 条，调整场景总数断言（17 景、36 快照、215 串）。完整文件清单在 `/tmp/hs-scene-survey-changed-paths.txt`；导出审计 `/tmp/hs-scene-survey-export-audit.log`：50 份既有 JSON 仅增加 surveyed，既有网格零差分。全部具体 run（含继承 unchanged 的末态与 advanced 中间态）核过：fresh 没有 run；清点中 5 份为假，清点停止 2 份为假，运行 63、完成 55、确认 9 及 advanced 确认 2 份均为真。

### 本次 red → green 与反转

日志前缀 `/tmp/hs-scene-survey-`，均在仓库外。

- `design-red.log`：装好依赖后新增设计用例实际失败（原稿没有 zero-volume 场景）；`design-green.log`：新增设计用例通过。完整最终 JS 测试见 `npm-test.log`（13/13）。依赖缺失不算 red。
- `rust-red.log`：新导出数据直接交给旧回放，实际执行 3 条、失败 3 条，备注找不到；`rust-green.log`：三条均通过（夹具的开工/表、完整快照、真实终端输入的六串说明卡/问题跳转）。未绕过 replay。
- `reversal.log`：只把回放条件改回旧空清单捷径，实际执行 3 条、失败 3 条。`data-mutation.log`：把真实导出场景 surveyed 改为字符串 unknown，严格扫描实际执行 1 条、失败 1 条，报 expected a boolean。`mutation-restore.log` 记录两次变异各自前后 SHA256 相等、原字节恢复；恢复后再跑场景/会话/闸门/polish。
- `strict-type.log`：缺失、null、数字、未知字符串均被拒绝，实际 1 条通过。屏上反断言禁止「清点途中停下」「＋ 添加路径」「NaN」，原清点停止的第零卷禁词断言照旧保留。

### 本次数

所有 Cargo 均设置 `CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-scene-survey-rustc-workspace.sh`，通过 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...` 串行；只在本工作树跑 npm。

| 检查 | 最终实际结果（日志） |
|---|---|
| npm export / check / test | 全绿；36 份快照、215 串，13 passed / 0 failed（`export.log`、`check-final.log`、`npm-test.log`） |
| scene 定向 | 15 passed / 0 failed（`scene.log`） |
| session 全部 | 349 passed / 0 failed / 1 ignored（`session.log`） |
| 独立 gate 1 | 782 通过 / 1 失败，lib 256 / bin 507；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.73s`（`gate1.log`） |
| 独立 gate 2 | 626 通过 / 1 失败，lib 256 / bin 351；末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.89s`（`gate2.log`） |
| 独立 gate 3 | 绿；`Finished dev profile [unoptimized + debuginfo] target(s) in 0.13s`（`gate3.log`） |
| polish | 四项全绿；既有 lib doc 15 warnings、fetch_update deprecated 与无默认特性 page_name dead-code 告警不增（`polish.log`） |
| 审计 | `git diff --check` 绿；黄金快照、.gitattributes、依赖未动（`protected-audit.log`、`export-audit.log`） |

本次修正 resolved；整仓仍不宣称全绿：gate 1、2 各仅卡批准排除的 Q995 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（macOS 重命名计数实际 0 / 期望 96），没有修复、削弱或 suppress。以上历史记录保留，纠正的是场景回放空清单捷径与导出完成事实的缺口。
