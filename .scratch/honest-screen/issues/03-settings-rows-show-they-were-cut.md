# 03 — 设置栏装不下时末尾放 `⋯`

**What to build:** 设置栏三组（设备设置、处理选项、画质判定参数）一行装不下时**末尾放 `⋯`**（01 那一种），不再从行尾硬截。画质门槛那一行在宽屏两栏与窄屏上都看得出后面还有。设计稿逐格改、重导——这一批里重导面最大的一张，不与别的改设计稿的票并行。

**Blocked by:** 01 — 三种截法收在一处

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读；重导之后既有快照的变动只落在本票牵到的那几处
- [x] 宽屏（120 列两栏）与窄屏画质门槛那一行截断时带 `⋯`，比整屏且绿
- [x] 设置栏里不再有从行尾硬截的那一路
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

画质门槛那一行读起来像只在一块面板上测过；设备配置那一行同病——硬截读成「说完了」。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1283 — 设置栏画质门槛那一行装不下两块面板：宽屏截在「boox-poke6」之后，看着仍像只有一块；窄屏截在「其」字

- **From:** 票 `say-and-stop/05`
- **Kind:** 票面措辞落到屏上的后果
- **Where:** `tests/fixtures/design/snapshots/config.120x36.text.txt`（宽屏）与 `tests/fixtures/design/sequences/config-narrow-h.text.txt`、`config-narrow-h-l-h.text.txt`（窄屏）的画质门槛那一行；设计稿 `thresholdText`
- **Why it did not block:** 不沾停线三条：设置栏硬截长句、不带省略号是既有画法（设备配置那一行在宽屏上截在「300 P」）。
- **What this ticket actually did:** 设计稿只改了那一句（ADR 0019 决定第 13 条），重导之后两串窄屏比整屏，那一行截成「画质门槛 5.123（在 boox-poke6 与 kobo-libra-2 上实测，其」。**宽屏（120 列两栏）那一副一格没变**：那一列截在「（在 boox-poke6 」之后，「与」两格宽放不下——截出来的样子与旧句截出来的一模一样，读的人看不出后面还有一块面板。全句在报告抬头里。
- **Options:** ① 认下截断；② 设置栏画质门槛那一项只印数（「5.123」或「5.123（标定值）」），来源留给设备配置那一行与报告抬头——要改设计稿与 `Session::shown`，还要动「设备设置里画质门槛那一项印的也是抬头里那一截」那条规矩（《画质判定参数》词条）；③ 设置栏截断时带省略号，整屏一律——设计稿逐格改、重导一大片。
- **Recommend:** ①，另开 ③ 的口子交给 `design-parity`。硬截读成「说完了」是整张设置栏的事（设备配置那一行同病），不是这一句的事；② 改的是一条词条里写着的规矩，为的是一档窗宽。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：设置栏三组摆不下时末尾放 `⋯`（《让位》已写），重导一大片，单独一张票。


## 落地记录

- 基点 / 交付位置：`hs-03`，HEAD 与起点都是 `4f4b469bdb7ebe300912e9343f56fe419f6e3116`；全部改动未提交。
- 先改 `design.html`：设置栏三组与组抬头共用带样式的末尾省略；短值原样保留。`export.js` 增加 `settings-elision-120`、`settings-elision-80`、`settings-elision-90` 三串，均经终端按键回设置栏、移到首项；120 列两栏、80 列单栏与 90 列两栏边界各比整屏。完成 export/check 后才改 Rust。
- Rust `shell/settings.rs` 将完整行交给票 01 的 `columns::elide_at(..., Some(Keep::Start))`，只把所得字串分回原样式，不另算截点；省略号沿用被截那一段的样式。设备设置、处理选项、画质判定参数走同一路。`terminal.rs` 一条新验收走三串，比整屏字、颜色与修饰，再反断言旧的假完整行尾不许出现，短值 `inside *` 不受影响。
- 重导产物共 30 份快照、191 串期望屏。此次 110 个设计产物文件有变（101 个既有文件、9 个新文件），其中既有 50 张字网格仅设置行 280 行变化；逐格解析样式图例后，50 张样式网格仅设置行 190 个格样式变化，右侧详情不变。场景数据既有文件不变；黄金快照、`.gitattributes`、其他票均未动。总代价为 115 个文件（5 个源/票据文件 + 110 个生成文件），没有新依赖。
- 红证据：设计稿先重导、新终端验收先跑而 Rust 仍为旧硬截时，新用例 `0 passed; 1 failed`（`/tmp/hs03-red.log`）。实现后同一用例通过。将 `Keep::Start` 反改成 `Keep::End` 再跑，新验收再次 `0 passed; 1 failed`，退出 101（`/tmp/hs03-mutation.log`）；已还原，后续闸门与回归使用正确实现。
- 接缝由批准 spec 的接缝①沿用：终端事件 → 会话状态 → TestBackend 整屏，对设计稿导出屏逐格比较；不加私有 helper 用例。
- `npm run export`、`npm run check` 与最终 `npm test` 都通过（后者 4/4，包含两次导出逐字节相同）。`cargo test session:: --bin tonefit`：330 通过、0 失败、1 忽略。`git diff --check` 通过。
- 三条 gate 分别执行，没有因第一条红而跳过后两条；gate 1/2 均仅在合同已列为基线 Q995 的 `tests/concurrency.rs:755` 失败：macOS 开着归档时仍能重命名，实际 0、期望 96。未修、未跳过、未弱化；上方“三条全绿”复选框因此保留未勾。此票的功能验收已完成，按批准的 Q995 范围例外交付，不声称全绿。
- 补跑 gate 1 在 concurrency 后未到的全部 20 个集成测试二进制，命令为 `cargo test` 加逐个 `--test`：container/counters/discovery/events/exit_code/golden/idempotency/isolation/metric/perceptual/pipeline/preset/profile/proof/resume/session/single_source/smoke/stop/timing。完整结果 `/tmp/hs03-integration-full.log`，18 个标准 harness 共 399 通过；另 perceptual 自定义 harness 实跑 3、缺样本跳过 2，smoke 缺 `TONEFIT_SAMPLES` 跳过、贡献 0，均非失败。首次同命令 200 秒超时（`/tmp/hs03-integration.log`）不计成功，重跑 1200 秒限时完整结束成功。不将此补跑说成 gate 2 的剩余集成测试也跑满。
- 所有 cargo（包括 fmt）均经 `/tmp/tonefit-honest-screen-cargo.py` 串行 runner，且设置独立的 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-03-rustc-workspace.sh`。polish 四项均成功；既有 deprecated `fetch_update` 告警未动，文档 15 条既有告警。npm ci 提示 Node 25 不在 jsdom 支持范围及 2 条 high audit 提示，实际导出/核对/测试均成功，未扩大范围升级依赖。

## 数

本票未提交交付时（起点 `4f4b469`）的实际结果，日志均在 `/tmp/hs03-*.log`：

- 闸门 1：**红**，合计 **761 通过 / 1 失败**；lib **255** / bin **487**（bin 另 1 忽略）。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.61s`。唯一失败 Q995；尚未执行到后续测试时便停止，不能把 761 当全库总数。
- 闸门 2：**红**，合计 **616 通过 / 1 失败**；lib **255** / bin **342**。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.11s`。同一 Q995。
- 闸门 3：**绿**，只编不跑。末行：`Finished dev profile [unoptimized + debuginfo] target(s) in 2.45s`。
- polish：fmt / clippy 默认 / clippy no-default / doc 均绿；doc：`warning: tonefit (lib doc) generated 15 warnings`。末行：`Generated /private/tmp/tonefit-honest-screen/03/target/doc/tonefit/index.html`。
- 会话定向：**330 通过 / 0 失败 / 1 忽略**；新验收 1 条覆盖 3 张整屏。npm **4 通过 / 0 失败**。补跑集成标准 harness **399 通过 / 0 失败**，自定义 perceptual 另 **3 实跑 / 2 跳过**，smoke **0 实跑**；与 gate 数不相加（部分命令覆盖面重叠）。
