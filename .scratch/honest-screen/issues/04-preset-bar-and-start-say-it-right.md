# 04 — 预设栏与开跑说得对

**What to build:** 预设栏光标停在末行「＋ 把当前设置保存为预设」上时，屏底写 **`⏎ → 保存为预设`**（按键表给末行那一件一句自己的短句，屏底与全部按键都从表上派）；停在一份预设上照旧 `⏎ → 使用`。起名那一行 **`Esc`**、**收起预设栏**时，「再按一次 ⏎ 覆盖「X」」那一问作废（重开起名那一行、存下了照旧作废）。设计稿**开跑之前先问型号**，次序与措辞照实现那一侧（先型号、再输出目录、再路径）。设计稿先改、重导，再改实现。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读；重导之后既有快照的变动只落在本票牵到的那几处
- [x] 末行那一串写 `⏎ → 保存为预设`，停在一份预设上照旧 `⏎ → 使用`；留一条反着钉的断言：末行上不再出现 `⏎ → 使用`
- [x] 起名那一行 `Esc` 之后那一问不在，导一串停在 `Esc` 那一刻；收起预设栏再掀开同样不在
- [x] 套了没写型号的预设按 `t`：设计稿与实现都说「先挑型号」，导一串比整屏
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

屏底说「使用」、按下去开的是起名那一行；那一问对着一个已经关掉的名字说话；设计稿上开跑那一屏与真程序不一致。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1090 — 预设栏末行上屏底仍写 `⏎ → 使用`，而那一行上 `⏎` 开的是起名那一行

- **From:** 票 `design-parity/02`
- **Kind:** 路过发现（设计稿与实现一样；票面只动末行上的 `dd`）
- **Where:** `design.html` 的 `footerHints` 预设那一支（`hint('⏎', '使用')`）；`src/session/view.rs` 的 `Session::hints` 预设栏那一支、
  `keymap::TABLE` 的 `Deed::UsePreset` 那一行；`sequences/config-p-G.text.txt` 末行
- **Why it did not block:** 按下去做得成事（开起名那一行），不是按不动的键；改那一句要动设计稿，不在票面列的那几处里。
- **What this ticket actually did:** 没动；新导出的 `config-p-G` 把它钉在屏上（`[⏎ → 使用] [p → 返回] [j/k → 选择] [? → 全部按键]`）。
- **Why it matters:** 屏底那一句说的不是按下去会做的那件事——「屏上不摆按不动的键」管不到它，可它照样骗人。
- **Options:** ① 照旧；② 末行上换一句（如 `⏎ → 保存为预设`）：设计稿 `footerHints` 按 `pcursor` 分，表上给保存那一件另起一行短句
  （或另起一个 `Deed`——实现里末行的 `⏎` 本来就走 `Deed::UsePreset` 的另一支），屏底点名要哪一句，与 `l → 展开／每页结果` 同一副。
- **Recommend:** ②。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：预设栏末行上屏底改成 `⏎ → 保存为预设`，设计稿与按键表一起改。

#### Q1189 — 起名那一行按 `Esc` 之后，预设栏里那一问「再按一次 ⏎ 覆盖「X」」还挂着：实现如此，设计稿照着画

- **From:** 票 `design-parity/08`
- **Kind:** 路过发现的缺陷（实现与设计稿同形，没有一串钉着）
- **Where:** `src/session/typing.rs` 的 `cancel_typed`（第 504 行：不碰 `armed_save`）、`src/session/view.rs` 的 `lift_picker`／`shut_picker`（第 1870、1879 行：只清 `armed_delete`）、
  预设栏上挪光标那一支；`design.html` 输入行 `Escape` 那一支（第 1750 行）与 `configKey` 的 `p`（第 1895 行）
- **Why it did not block:** 票面只说「重开起名那一行就作废」，没说 `Esc`；硬约束是实现一行不动，设计稿因此照实现画、不替它另立一种行为。
- **What this ticket actually did:** 设计稿的 `armedSave` 只在两处作废：重开起名那一行（第 1903 行）、存下了。`Esc`、收起再掀开预设栏、挪光标都不碰它——与实现同形。
  `config-p-save-taken-Escape-Enter` 钉的是重开之后那一问没了；停在 `Esc` 那一刻（那一问还挂着）的那一屏没导。
- **Why it matters:** `Esc` 之后屏上那一问说「再按一次 ⏎ 覆盖」，而光标停在末行上按 `⏎` 是重开起名那一行、并不覆盖——一句假话；收起再掀开预设栏它还在。
- **Options:** ① 设计稿在关掉起名那一行（`Esc`）、收起预设栏时都作废它，导一串停在 `Esc` 那一刻的钉住，实现跟着（`cancel_typed`、`shut_picker`）；
  ② 照旧。
- **Recommend:** ①。那一问问的是「这一行里打的这个名字」，这一行没了它就该没了。
- **Whose call:** 拍板的人（设计稿）；开不开票归协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：起名那一行 `Esc`、收起预设栏时「再按一次 ⏎ 覆盖」那一问作废，设计稿与实现一起，导一串停在 `Esc` 那一刻。

#### Q1149 — 设计稿的 `startRun` 不认「型号没挑」：套了没写型号的那一份之后按 `t`，设计稿照样开跑，实现说「先挑型号：…」

- **From:** 票 `design-parity/07`
- **Kind:** 本票让一处从前到不了的设计稿缺口到得了
- **Where:** `design.html` 的 `startRun`（第 1627 行：只认「正在处理中」与「没有勾选任何路径」两种）；
  `src/session/state.rs` 的 `Session::request`（头一句「先挑型号」）；`src/session/terminal.rs` 的 `begin`
- **Why it did not block:** 从前设计稿里型号回不到「没挑」（详情栏没有那一格），这一支到不了；本票之后套「画集」就到得了。
  票面那一句「屏上照现成那一句说」指的是实现那一句，终端层用例钉着（`a_preset_that_names_no_model_leaves_the_run_asking_for_one`）；
  没有一串期望屏，因为设计稿在那一下会开跑。翻过来是设计稿 `startRun` 加一问、导一串 `config-p-j-Enter-t`。
- **What this ticket actually did:** 没动设计稿那一支（硬约束：只改存与套两支）。
- **Why it matters:** 那一刻设计稿与实现两副屏完全不同（一副开跑、一副一句回话），而没有快照会红。
- **Options:** ① 设计稿 `startRun` 先问型号（次序照 `Session::request`：型号、输出目录，再是路径），措辞取实现那一句，导一串期望屏；② 照旧。
- **Recommend:** ①。
- **Whose call:** 拍板的人（设计稿）；开不开票归协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：设计稿 `startRun` 先问型号，措辞照 `Session::request` 那一句，导一串钉住。


## 落地记录

基点 `a816ba3`，工作树 `/tmp/tonefit-honest-screen/04`，分支 `hs-04`；全部未提交。实现完成；最后一条验收框不勾：闸门 1、2 实际仍被本 effort 明确排除的 macOS 基线 Q995 挡住，不声称全绿。

### 做成与代价

- 先改设计稿、导出三串：`config-p-save-taken-Escape`、`config-p-save-taken-Escape-p-p`、`config-p-j-Enter-t`；既有 `config-p-G` 增加正反措辞断言。
- `keymap::TABLE` 给 `UsePreset` 增加「保存为预设」短句与全部按键长句；屏底按所选行点名短句，原有预设上仍是「使用」。全部按键新增说明「保存为预设（预设栏末行）」；代价是该说明使全部按键的既有屏重新分栏、滚动末页多一行。既有导出变动仅末行屏底与全部按键那一族（含其滚动场景数据），没有手改导出文件。
- Rust 取消预设起名、收起预设栏都会作废覆盖问句；保留重开起名、保存成功两处原有清理。独立收栏用例从尚在等覆盖的夹具喂 `p p` 比整屏，不借 Esc 的清理。末行与取消后的旧措辞都有禁止断言。
- 设计稿开跑先问型号、再问输出目录、再问路径；前两句照 Rust 现有措辞。Rust 的开跑守卫无需改动。无型号预设后按 `t` 从终端层重放、逐格比导出整屏。
- 新增 `honest-screen.test.js`，`npm test` 一并运行：型号与输出目录都缺且路径全未勾时先说型号；给了型号才说输出目录；两者齐了才问路径。独立夹具测试收栏，防上游清理掩盖它。
- 没改黄金快照、`.gitattributes`、公共 API、架构边界或其他票。

### 红 → 绿与守卫按反

接缝按本 effort 已批准的 Testing Decisions：设计导出整屏、终端输入重放、按键表派生提示、设计稿按键后屏上回话。

- 安装依赖 `npm ci` 后，先加 `config-p-G` 新措辞断言，`npm run check` 红在旧的 `[⏎ → 使用]`（`/tmp/hs04-design-red.log`）；设计改完重导、check、test 绿，再开始 Rust。
- Rust 新增 `honest_preset_footer_and_cancelled_naming_match_the_design`，先跑 `cargo test --bin tonefit honest_preset -- --nocapture`，1 条匹配、1 条失败，整屏仍是旧末行提示（`/tmp/hs04-rust-red.log`）。修正表与清理后会话用例绿。
- `/tmp/hs04-reversals.py` 每次只反一处、运行后恢复原字节：Rust 的 Escape 清理、收栏清理、重开起名清理、保存成功清理、末行短句；各跑命名用例，均 1 条失败。设计稿分别拿掉型号守卫、输出目录守卫、`p` 收栏清理，`node --test .scratch/session-redesign/honest-screen.test.js` 各 1 条失败。证据：`/tmp/hs04-reversal-{escape,shut,reopen,saved,footer,design-model,design-output,design-shut}.log`。最终恢复后会话与设计用例重新绿。

### 实际命令与日志

以下所有 cargo（包括 fmt、反转用例和 xtask 子命令入口）都以同一形式调用：

```
RUSTC_WORKSPACE_WRAPPER=/tmp/hs-04-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo <参数>
```

wrapper 是票专用 `#!/bin/sh` / `exec "$@"`，target 保持指向集成树的原 symlink。

- 在 `.scratch/session-redesign`：`npm ci`（`/tmp/hs04-npm-ci.log`）；`npm run export`（`/tmp/hs04-export.log`）；`npm run check`（`/tmp/hs04-check.log`）；`npm test`（6 通过，`/tmp/hs04-npm-test.log`）。
- `cargo test --bin tonefit session::`：333 通过、0 失败、1 原有 ignored（`/tmp/hs04-session.log`）。
- `cargo test --test single_source --test session --test preset`：分别 12、2、6 通过（`/tmp/hs04-integration.log`），补验闸门提前停止之后的相关集成检查。
- `cargo xtask gate 1`、`cargo xtask gate 2`、`cargo xtask gate 3` 分开实际执行（`/tmp/hs04-gate1.log`、`/tmp/hs04-gate2.log`、`/tmp/hs04-gate3.log`）。前两条唯一已见失败均是 Q995 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，`tests/concurrency.rs:755`：打开时 macOS 仍能重命名目录，left 0 / right 96。没有改、跳过或削弱它；它之后的整套用例未由该闸门执行。
- `cargo xtask polish`：四项绿（`/tmp/hs04-polish.log`）；`git diff --check` 绿。

## 数

本票最终验证这一次（不是声称整套已经跑完）：

| 项 | 结果 / 末行 |
|---|---|
| 闸门 1 | 红；合计 764 通过 1 失败；lib 255 / bin 490；`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.21s` |
| 闸门 2 | 红；合计 616 通过 1 失败；lib 255 / bin 342；`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.24s` |
| 闸门 3 | 绿；`Finished dev profile [unoptimized + debuginfo] target(s) in 0.65s` |
| polish | fmt / clippy 默认 / clippy 无默认特性 / doc 四项绿；lib doc 15 条既有告警（未增加）；原有 `fetch_update` deprecated 告警仍在 |

余项只有已排除的 Q995 基线红，以及闸门受其提前停止而未覆盖后续整套测试的风险；没有本票未完成的行为变更。
