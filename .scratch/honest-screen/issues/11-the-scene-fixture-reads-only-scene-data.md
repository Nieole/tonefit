# 11 — 场景夹具只从场景数据来

**What to build:** 设计稿给每一卷**导出它干净时的去处**（与隔离去处同一处算法），场景数据多一格，夹具照读，不再自己算。场景夹具里每一处读取值的地方（视图、输入行的种类、覆盖层、配置视图的哪一栏……）把认得的写全，**认不出交回错误**（与认光标那一处同一副）；一条用例走遍全部场景与序列的场景数据。碰场景夹具（本轮的冲突热点）：与 10 不并行。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿导出干净去处，`npm run export` 重导，`npm run check` 绿；夹具照读、不再自己算
- [x] 干净的卷与隔离的卷，去处是同一副结构（一条用例核对）
- [x] 场景夹具每一处取值认不出就报错；走遍全部场景数据那一条绿；往一份数据里写一个认不出的取值它红（试完还原，数据一个字节不留）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

设计稿那头改个拼写，验收照样绿；夹具自己造数据，与库的镜像规则两副结构。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1058 — 隔离那一卷的去处照了库的镜像规则，夹具给其余各卷接的干净去处（`output_of`）没照：同一卷两个去处不是同一副结构

- **From:** 票 `design-parity/01`
- **Kind:** 路过发现的夹具与库规则不一致（Q765 的另一半）
- **Where:** `src/session/scene.rs` 的 `output_of`（输出目录接「分区或顶格目录的上一层」之下那一截）与 `volume_report`；
  `src/discover.rs` 的 `mirrored`；`src/lib.rs` 里「隔离目录只是在中间插一级 `_isolated`，镜像出来的结构一模一样」
- **Why it did not block:** 屏上一处都不露干净的去处（24 份快照与 140 串期望屏都没有），场景数据里也没有这一格；翻过来只动夹具一个函数。
- **What this ticket actually did:** 没动 `output_of` 的算法（票面第七条只要场景数据里隔离那一卷对上库），改了它的文档：
  原来那句「与场景数据给出的隔离去处同一个写法」已经不成立，现在写明它不照库的规则、指着本条。
  `集英社/海贼王/第07卷` 隔离时落在 `~/转好的/_isolated/漫画库/集英社/海贼王/第07卷`，
  干净时夹具会给 `~/转好的/集英社/海贼王/第07卷`（库会给 `~/转好的/漫画库/集英社/海贼王/第07卷`）；
  点名的压缩包那一行 `灰原哀` 同样差一级（夹具 `~/转好的/灰原哀/第01卷.cbz`，库 `~/转好的/第01卷.cbz`）。
- **Why it matters:** 哪天有一屏露出干净的去处（每页结果的输出路径、更宽的屏），夹具说的是库不会写出的那一条，逐格比对会替错的东西作证。
- **Options:** ① 夹具照库的规则现算（要把处理路径传进 `output_of`，与本票新添的 `an_isolated_volume_lands_where_the_library_mirrors_it` 同一个认法）；
  ② 设计稿给每一卷导出干净的去处（`output`），夹具照读——与隔离那一格同源；③ 照旧（已落地）。
- **Recommend:** ②。《场景夹具》那条是「数据只从场景数据来」，① 等于让夹具自己照一份规则**造**数据；② 让设计稿那一处算法（`v.named`＋`v.root`，本票刚立的）同时出两个去处。
  （本票那条新用例同样在 Rust 里写了一遍规则，但它是**神谕**——拿来核设计稿给的数，不拿来造数；库的 `mirrored` 是私有的，够不着。）
  本票没做 ②：那要在设计稿里添一个本票十处之外的函数（干净的去处），而派活说明把设计稿的改动限在那十处。
- **Whose call:** 拍板的人（动设计稿的导出）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：设计稿给每一卷导出干净的去处，夹具照读。

#### Q1381 — 只收了光标那一处；`views_of`、`config_of` 里另几处 `_ =>` 照旧默默落到默认那一档

- **From:** 票 `one-source/08`
- **Kind:** 落地时走的那条路（收多宽）
- **Where:** `src/session/scene.rs` 的 `views_of`（`view`：`_ => View::Task`；输入行的 `kind`：`_ => None`；覆盖层只认 `help`）与 `config_of`（`pane`：`_ => Pane::Settings`）
- **Why it did not block:** 场景数据里那几格今天全认得出（全部场景与序列 187 份逐份数过）：`view` 只有 `task`／`config`，`pane` 只有 `left`／`right`，输入行只有 `add`／`search`／`value`／`preset`／`edit`／`out` 或不在场，覆盖层只有 `help`／`note` 或不在场。
- **What this ticket actually did:** 只收光标（票面与 Q846 点的就是它）。另几处一格没动——其中 `task`、`left` 两个是**落进 `_` 才认出来的**，导出那一头改个拼写同样一声不响，Q846 那个洞的形状原样还在。
- **Options:** ① 照现在；② 本票一并收：每一处把认得的写全、`_` 交回认不出，走遍那一条多问几格；③ 另立一张票，与 `views_of` 往后要接的那几支一起收。
- **Recommend:** ③。`scene.rs` 是本轮的冲突热点，本票改得越窄越好；那几格今天都对得上，晚收一张票不丢东西。
- **Whose call:** 协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：场景夹具里每一处 `_ =>` 都改成认不出就报错，一条用例扫遍全部场景数据。


## 落地记录

- 工作树 `/tmp/tonefit-honest-screen/11`，分支 `hs-11`，起点/当前 HEAD `b810feb30c54bc8c21ddd3d4504aa62222a5b98f`；全部未提交。仅本票。
- 先改设计稿：`design.html` 的 `volumeOutput(v, isolated)` 同一算法给两个去处；`render.js` 暴露它；`export.js` 每卷导出 `output`，仍只在实际隔离时导出 `isolated_output`。没有另造屏、没有手改导出物。新增一条 JS 接缝测试。
- `src/session/scene.rs` 删除 `output_of`，包括幂等命中在内的干净去处直接读导出的 `output`；隔离读 `isolated_output`。原隔离布局扫描扩到全部干净去处且保留隔离非空断言。新增真实 `tonefit::run` 接缝：从导出数据选含隔离去处的目录卷，在全新临时盘各跑一次干净/含坏页，比较报告及盘上目录，不依赖夹具镜像计算。测试显式关闭抖动以隔离布局与几何互锁。
- 视图、光标、输入种类、覆盖层、配置栏/光标/屏幕规格逐支识别，显式未知值报错。配置预设读取不再 `.ok()` 吞错；动态 JSON 的布尔/序号/字符串不再靠类型转换失败落默认档。合法 `null`（没跑、无输入/覆盖层/下钻/结局/判定）照旧。解析还验证所有卷状态、模式/阶段/结局/停止级别、路径种类、灰阶分布候选、逐页候选/理由/种类、设置和预设，含未显示的页。
- 新增全部场景、全部序列末态与 `advanced` 中间态的实际夹具扫描；负变异覆盖各类 session 取值、run/page/清单取值、设置枚举。`src/specs/session.md` 写明去处与严格解析契约。
- 成本：新增 Rust 接缝/守卫测试 4 条、JS 测试 1 条；无新增依赖。导出差分审计 `/tmp/hs-11-export-diff-audit.log`：48 份 JSON 仅新增卷 `output`，所有字/样式网格、黄金快照、`.gitattributes`、依赖文件不变。完整改动路径清单 `/tmp/hs-11-changed-paths.txt`。

### Red → green 与反转证据

1. JS 先红：`node --test --test-name-pattern='每一卷导出' honest-screen.test.js`，`output` 为 undefined，期望 `~/转好的/漫画库/哆啦A梦/第01卷`（`/tmp/hs-11-design-red.log`）。首次缺依赖后仅在本树 `npm ci`（`/tmp/hs-11-npm-ci.log`），再获得该行为红。
2. Rust 先红：`cargo test --bin tonefit unknown_session_variants -- --nocapture`，`/view silently accepted`（`/tmp/hs-11-rust-red.log`）。严格分派后绿。
3. 真数据变异：把 `sequences/fresh-dd.scene.json` 的 view 写成未知拼写，全部数据扫描 exit 101（`/tmp/hs-11-data-mutation-red.log`）；finally 原字节恢复，SHA256 前后均 `284dcaba8a3d85079af5ebd1558e0a11349e24ce9b18739a1f12164ab2dddb71`（`/tmp/hs-11-mutation-restore.log`）。
4. 布局守卫按反：把 `scenes/running.json` 对应卷的干净输出删掉 `/漫画库`，真实库接缝 exit 101，实际库输出仍含 `/漫画库`（`/tmp/hs-11-layout-mutation-red.log`）；finally 原字节恢复，SHA256 前后均 `ca5f2467c7af0a8a562bc9938a1631816b6b4df2cc9307c87f6a29f1ca3dae3c`（`/tmp/hs-11-layout-restore.log`）。
5. 还原后 `npm run check` 再核逐字节一致（`/tmp/hs-11-check-final.log`）。没有为绿削弱任何现有断言；本票不改屏上措辞，无需新增假话禁词断言。

## 数

所有 Cargo 命令均用 `CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-11-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo ...`，串行执行；wrapper 为本票 pass-through。

- 设计先行：`.scratch/session-redesign` 下 `npm run export`、`npm run check`、`npm test` 绿，full npm **12 passed / 0 failed**。日志 `/tmp/hs-11-export.log`、`/tmp/hs-11-check.log`、`/tmp/hs-11-npm-test.log`。
- `cargo test --bin tonefit session::scene::tests`：**13 passed / 0 failed**（`/tmp/hs-11-scene-green.log`）。
- `cargo test --bin tonefit session::`：**345 passed / 0 failed / 1 ignored**（`/tmp/hs-11-session.log`）。
- 分别运行 `cargo xtask gate 1`：**777 通过 / 1 失败；lib 255 / bin 503**。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.34s`（`/tmp/hs-11-gate1.log`）。
- 分别运行 `cargo xtask gate 2`：**623 通过 / 1 失败；lib 255 / bin 349**。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.75s`（`/tmp/hs-11-gate2.log`）。
- 分别运行 `cargo xtask gate 3`：绿，末行 `Finished dev profile [unoptimized + debuginfo] target(s) in 0.13s`（`/tmp/hs-11-gate3.log`）。
- `cargo xtask polish` 四项全绿；lib doc 既有 **15 warnings**；既有 deprecated `fetch_update` 与 no-default 的 `page_name` dead-code 告警保留（`/tmp/hs-11-polish.log`）。`git diff --check` 绿。

### 留开的基线红

验收最后一项不勾：闸门 1、2 均在已接受排除的 Q995 `tests/concurrency.rs::many_archive_volumes_never_hold_more_than_the_one_being_processed` 红（macOS 重命名检测 left 0 / right 96），闸门在那里停止，并非全套通过。本票交付已完成；不修、不 suppress、不弱化该基线用例。


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
