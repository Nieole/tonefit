# 05 — 详情栏说对型号与面板

**What to build:** 光标停在型号那一行上，详情栏注**套着的预设里写的是哪一块型号**；预设没写型号时说「预设「X」中：没写型号（套用时型号回到没挑）」（《详情栏》已写）。画质门槛与设备配置两段说明里的**面板名单从库里的标定来源读**，与报告抬头、设置栏画质门槛那一行同一处，说明文字里不再写面板名。设计稿先改那几段、重导，再改实现。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读；重导之后既有快照的变动只落在本票牵到的那几处
- [x] 型号那一项：预设写了型号与没写型号各一屏，比整屏且绿
- [x] 两段说明说出的面板，与设置栏那一行、报告抬头同一份名单
- [x] 单一出处扫描添一条：详情栏说明里不许出现面板名字面；按反跑一次（抄一个面板名回去）看见红
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

换了型号之后设置栏那一行的 `*` 没有解释；同一屏上一行说两块面板、底下那段说一块。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1148 — 详情栏里型号那一项不说「预设「X」中：…」，而型号已进预设、设置栏那一行会带 `*`

- **From:** 票 `design-parity/07`
- **Kind:** 路过撞见的设计稿与词汇表对不上（本票让它露了面）
- **Where:** `design.html` 的 `drawCfgRight`（第 1460 行：`if (c.kind === 'ring' || c.kind === 'text')` 才写那一句）；
  `src/session/shell/details.rs` 的 `rows`（第 134 行：`field != Field::Profile`）；`CONTEXT.md` 的《详情栏》（「套着的预设里这一项怎么设（与此刻不同时注一句）」）
- **Why it did not block:** 屏上数的那三处（`*`、「改动了 N 项」、「包含 N 项设置」）已经读同一份单子；详情栏那一句不在票面上，
  硬约束是设计稿只改存与套两支。翻过来是设计稿那一行条件加上型号、重导 `config-model*` 那几串、实现删掉那一句条件。
- **What this ticket actually did:** 没动，照设计稿：型号那一项的详情栏只列屏幕规格与说明，不说套着的预设里型号是哪一块。
  换了型号之后，设置栏型号那一行带 `*`、顶上写「改动了 3 项：型号…」，光标停到那一行上，详情栏却说不出预设里写的是哪一块。
- **Why it matters:** 《详情栏》词条说它答「套着的预设里这一项怎么设」，型号进预设之后这一项是唯一的例外；`*` 在屏上、解释不在。
- **Options:** ① 设计稿型号那一支也写那一句（预设没写型号时写什么——照别的几项写「未设置（使用默认值）」说不通，型号没有默认——由设计稿另给一句），实现跟着；② 照旧，《详情栏》词条注明型号例外。
- **Recommend:** ①。型号进预设是拍过板的，`*` 与它的解释该在一处成套出现。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：型号那一项也注套着的预设里写的是哪一块，没写时「预设「X」中：没写型号（套用时型号回到没挑）」；设计稿先改（《详情栏》已写）。

#### Q1284 — 详情栏里画质门槛、设备配置两段长说明仍只说 boox-poke6

- **From:** 票 `say-and-stop/05`
- **Kind:** 路过发现（屏上那一行与它底下的说明说的不是同一件事）
- **Where:** `src/session/config.rs` 的 `about_setting` 的 `Field::Threshold`（「这个数是在 boox-poke6 上让人盲看对比定出来的」）与 `about_premise` 的 `Judging::Device`；设计稿 `CONFIG` 里 `threshold`、`p1` 两项的 `desc`
- **Why it did not block:** 不沾停线三条：两段都是会话自己的说明文字，不是取自库的句子；派活说明写死「设计稿只改取自库的那几句」，`design-parity/03` 此刻正改 `config.rs` 的长说明。
- **What this ticket actually did:** 一字没动。屏上画质门槛那一行（取自库）已说「在 boox-poke6 与 kobo-libra-2 上实测」，底下那段说明仍说「在 boox-poke6 上让人盲看对比定出来的」。
- **Options:** ① 记下，归下一张改设计稿的 `design-parity` 票：先改设计稿那两段 `desc`、重导，再改 `about_setting`／`about_premise`；② 本票顺手改。
- **Recommend:** ①。ADR 0019 决定第 13 条要先改设计稿，而那两段与 dp/03 改的是同一个函数；本票改它会撞上并行的那一槽。
- **Whose call:** 协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：详情栏两段长说明里的面板名单从库里读（与抬头同一处 `ThresholdSource`），设计稿先改。


## 落地记录

- 工作树 `/tmp/tonefit-honest-screen/05`，分支 `hs-05`，起点与收尾 HEAD 均为 `139ee85`；全部修改未提交。只更改本票状态；黄金快照、`.gitattributes`、依赖与 Q995 均未改。
- 设计先行：型号详情复用 `stored` / `changedKeys`，仅型号与预设不同时注预设型号；没写型号明说「没写型号（套用时型号回到没挑）」。两段说明复用设计稿的 `thresholdText`。新增四串 `details-{model-specified,model-omitted,threshold-provenance,device-provenance}`，导出后才改 Rust；既有导出文件仅 `config-model-drill-j-l` 的字与样式增加型号注解，另更新 manifest。快照不手改。
- Rust 复用 `config::preset_says` / `starred`；仅解除不同型号的注解例外。两段说明通过内置表上的型号读取内置 Threshold，整个数值与来源交给库的 Display（与 render 抬头同源），不另拼面板名单或连接词。不增加公共 API；代价是说明包含完整的内置门槛值与来源，用户覆盖值仍在当前值与抬头显示，不冒充标定史。颗粒项的另一份标定说明不属于本票。
- 验收接缝照 accepted spec：JS 输入→整屏文字、终端真实键码分派→设计导出的字/色/修饰逐格全屏、单一出处扫描。增加库来源一致性用例，明确选 `kobo-libra-2`、覆盖门槛 `7.25` 与不选型号三种前提，说明都保留内置标定；不借默认型号。单一出处扫描拒绝两段说明内任何内置型号字面。

### 实际红证与按反

所有日志在仓库外 `/tmp/hs05-*.log`，没有保留变异。

- `npm ci` 后 `npm test`：新增 JS 用例先红，6 通过 / 1 失败；缺少 `预设「漫画」中：kobo-libra-2`（`/tmp/hs05-npm-red.log`）。最初未装 jsdom 的启动错误不算行为红证。
- 设计导出并 check 后、Rust 跟进前，`cargo test --bin tonefit details_name_the_preset_model_and_calibrated_panels`：0 通过 / 1 失败（`/tmp/hs05-rust-red.log`）。实现后同名用例 1 通过（`/tmp/hs05-rust-green.log`）。
- 单一出处按反：把 `boox-poke6` 抄回门槛说明，扫描 0 通过 / 1 失败，明确报「画质门槛说明抄了面板型号 boox-poke6」（`/tmp/hs05-mutation-scan.log`，退出 101）。
- 恢复型号排除守卫 `field != Field::Profile`，整屏用例红在 `details-model-specified` 第 23 行第 56 格，实际空白、期望「预」（`/tmp/hs05-mutation-model-guard.log`，0/1，退出 101）。
- 没写型号改回「未设置（使用默认值）」：整屏红在 `details-model-omitted` 第 23 行第 72 格，实际「未」、期望「没」（`/tmp/hs05-mutation-missing-model.log`，0/1，退出 101）。
- 来源改成用户覆盖值 `7.25`：整屏红在 `details-threshold-provenance` 第 8 行第 111 格，实际「7」、期望「5」（`/tmp/hs05-mutation-provenance.log`，0/1，退出 101）。
- 全部恢复后：`cargo test --bin tonefit details_` 6 通过 / 0 失败；`cargo test --test single_source` 13 通过 / 0 失败（`/tmp/hs05-details-green.log`、`/tmp/hs05-single-source.log`）。

## 数

所有 Cargo 命令均通过 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...` 串行 runner，且设 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-05-rustc-workspace.sh`（票专用透传包装）；三个闸门分别调用。

- `npm run export`：30 份快照 / 199 串期望屏；`npm run check`：与库里那一份逐字节相同；`npm test`：7 通过 / 0 失败（原有 6 条全部执行，另加本票 1 条）。日志 `/tmp/hs05-export.log`、`/tmp/hs05-check.log`、`/tmp/hs05-npm-green.log`。
- 闸门 1：退出 1，合计 **766 通过 / 1 失败；lib 255 / bin 492**（另 1 个既有测量用例 ignored）。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.81s`。日志 `/tmp/hs05-gate1.log`。
- 闸门 2：退出 1，合计 **617 通过 / 1 失败；lib 255 / bin 343**。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.63s`。日志 `/tmp/hs05-gate2.log`。
- 两条红均是已知基线 **Q995**：`tests/concurrency.rs:755` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，macOS 仍可重命名，left 0 / right 96；未削弱或修复。闸门会停在此处，后续测试不冒称已跑；单一出处另行定向跑满。上面「三条全绿」验收格因此保留未勾，本票范围内已完成。
- 闸门 3：退出 0，末行 `Finished dev profile [unoptimized + debuginfo] target(s) in 2.41s`（原日志中 dev 两侧为反引号）。日志 `/tmp/hs05-gate3.log`。
- `cargo xtask polish`：退出 0，fmt check、两种 clippy、doc 四项全绿；lib doc **15 个既有告警**；原子 `fetch_update` 弃用告警仍在未改的 `src/main.rs:1193`。日志 `/tmp/hs05-polish.log`。
- `git diff --check`：退出 0；没有改黄金快照、`.gitattributes` 或依赖清单。
