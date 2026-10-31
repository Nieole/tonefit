# 08 — 「预计还要多久」以设计稿为准

**What to build:** 抬头那句「预计还要多久」以设计稿为准：设计稿那一侧**分子取走完的整步**（与总进度的步数向下取整同一族），实现补上「**总进度不到 2% 不报**、说正在估算剩余时间」那道门。两边一套算法之后，「正在摊开」那一景不必再挑两边恰好取整到同一秒的那一刻。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿分子取整步，`npm run export` 重导，`npm run check` 绿
- [x] 实现：总进度不到 2% 时抬头说正在估算，过了 2% 报整步算出的那个数；不到与过了各一屏比整屏
- [x] 「正在摊开」那一景为对齐两套算法而调的那一笔拿掉，或改成照实说
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

头一分钟报一个瞎估的数；设计稿与实现两套算法，每一景都得挑两边恰好对上的那一刻。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1129 — 抬头那句「预计还要多久」设计稿与实现两处算法不同；「正在摊开」那一景挑在两边四舍五入到同一秒的那一刻

- **From:** 票 `design-parity/13`
- **Kind:** 路过撞见的设计稿与实现对不上（与本票无关的那一句，新导的景头一回踩到）
- **Where:** `design.html` 的 `overviewTitle`（第 1099 行：`frac > 0.02` 才报，分子 `r.steps` 是连续时间攒下的小数步）；
  `src/session/live.rs` 的 `eta`（第 1191 行：一步没走才不报，分子是走完的整步）；`design.html` 的 `case 'extracting'`（第 1971 行）与摊开的速度（第 730 行）
- **Why it did not block:** 既有十一景都在 2% 以上、步数大到小数那一截不改秒数，所以从没红过；spec 说实现途中撞见的新对不上记停车场、不在本批里顺手判。
  翻过来（真修好它）之后那一景随便挑哪一刻都行，不必再挑。
- **What this ticket actually did:** 没动那一句。「正在摊开」那一景原想停在第02卷摊开到一半（总进度 1.7%：设计稿写「正在估算剩余时间」，实现报出一个数），
  改成前两卷做完、第03卷摊开到一半（3%）；那一刻两边差 0.5 秒、跨了取整的边，于是把摊开的模拟速度定成分析的 3 倍，
  两边都落在 `15m01s`。景的注释里写着这一笔。
- **Why it matters:** 换一景、换一个速度，同一句就可能差一秒；不到 2% 的那一截屏上两边说法不同。
- **Options:** ① 另开一张票：设计稿的分子取整步（与 Q899「总进度的步数向下取整」同一族），实现补上 2% 那道门（设计稿是权威）；
  ② 设计稿去掉 2% 那道门、分子取整步，实现不动；③ 照旧。
- **Recommend:** ①。整步是真程序拿得出的唯一读数；2% 那道门让头几卷的瞎估不上屏，实现没有理由不跟。
- **Whose call:** 协调人（开不开票）；门槛那一半归拍板的人（设计稿）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：以设计稿为准：设计稿的分子取整步，实现补上「不到 2% 不报」那道门。


## 落地记录

- 工作树 `/tmp/tonefit-honest-screen/08`，分支 `hs-08`，起点与当前 HEAD 均为 `9266edb`；接住中断 worker 的未提交设计稿、导出脚本、JS 回归与 claimed 状态，没有重置。所有改动保持未提交。
- 设计稿 ETA 先向下取完成整步；不到 2% 写「正在估算剩余时间」，恰好 2% 起按 `已用 × (总步数 − 完成整步) / 完成整步` 报数。Rust 的 `Live::overall` 同一条，整数交叉相乘判门槛，不借屏上百分比舍入；纳秒先乘后除。
- 新增 `eta-start` 预览起点、120/80 列快照，以及 `eta-start-estimating-{120,80}`、`eta-start-ready-{120,80}` 四串真实时钟回放。前者推进 0.01 秒仍不足 2%，后者推进 0.05 秒超过 2%；终端回放逐格比文字、前景色、修饰，并反断言不许提早报 ETA。数值用例另验 0、1%、1.99%、恰好 2%、2.01%、100%；JS 还验 2.99 小数步仍按 2 整步报 `8m10s`。
- 删去摊开模拟 `RATE * 3` 的舍入凑巧补丁，恢复与分析同速；不再为了两套算法凑秒。代价是 `extracting` 和同样经过摊开的 `retained` 的计时数据、快照与 retained 序列随导出改变（例如摊开 1s → 4s），没有手改导出产物。场景数量检查随新增场景从 15 更新为 16，未削弱检查；未碰黄金快照、依赖、`.gitattributes` 或其他票据。
- `src/specs/session.md` 记下 ETA 的完成整步与 2% 边界合同。

### 红 → 绿与按反

- 中断 worker 的 JS 原始红保留在 `/tmp/hs08-npm-red.log`：旧设计恰好 2% 仍说正在估算，11 条中 1 条红；其初次绿为 `/tmp/hs08-npm-green.log`。本次续做的完整最终绿见下。
- Rust 先加 `eta_waits_for_two_percent_of_completed_integer_steps` 与整屏回放回归，旧实现跑 `cargo test eta_ --bin tonefit`：1 通过 / 2 失败（`/tmp/hs08-rust-red.log`），1% 已报 ETA，整屏标题也红。实现后目标用例 3 通过（`/tmp/hs08-rust-green.log`）；完整会话与 gate 中再次覆盖最终场景命名后的用例。
- 按反 Rust `< 2%` 为 `> 2%`：3 条全红（`/tmp/hs08-reversal-early.log`）；改为 `<= 2%`：恰好 `2/100` 期望 `490s`、实际 None，1 红 / 2 绿（`/tmp/hs08-reversal-boundary.log`）。
- 按反设计稿：取消 `Math.floor` 时 JS 整步用例红；`>= 0.02` 改成 `> 0.02` 时边界用例红，均是 1 条执行 / 1 条失败（`/tmp/hs08-design-reversal-fractional.log`、`/tmp/hs08-design-reversal-boundary.log`）。所有变异已还原。
- 初次全会话还发现新增场景数量常量及序列前缀契约未接上（`/tmp/hs08-session-first.log`）；现已保留严格数量断言并给序列加 `eta-start-` 前缀，最终会话全绿，没有绕过契约。

### 验证命令

全部 Cargo 命令均用 `CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-08-rustc-workspace.sh python3 /tmp/tonefit-honest-screen-cargo.py cargo …`；wrapper 是合同要求的 pass-through，所有构建串行且不留 incremental 缓存。

- 设计目录依次 `npm run export`、`npm run check`、`npm test`：导出 34 份快照、209 串期望屏，逐字节检查绿；完整 JS 11 通过 / 0 失败 / 0 跳过。日志 `/tmp/hs08-export.log`、`/tmp/hs08-check.log`、`/tmp/hs08-npm-final.log`。
- `cargo test session:: --bin tonefit`：341 通过 / 0 失败 / 1 既有忽略，157 filtered；日志 `/tmp/hs08-session.log`。没有增加忽略或跳过。
- 三条 gate 分别执行，第一条红没有挡住后两条；最终结果见《数》。基线 Q995 在批准范围外，未修、未跳过、未放宽，因此「三条全绿」复选框诚实保留未勾；gate 1/2 遇 Q995 后未执行的后续集成二进制不算绿。
- `cargo xtask polish` 与 `git diff --check` 同样执行；现有弃用与文档告警不隐藏。

## 数

- 闸门 1：**红**，合计 **772 通过 / 1 失败**；lib **255** / bin **498**（bin 另 1 既有忽略）。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.21s`。日志 `/tmp/hs08-gate1.log`。
- 闸门 2：**红**，合计 **618 通过 / 1 失败**；lib **255** / bin **344**。末行：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.14s`。日志 `/tmp/hs08-gate2.log`。
- 两条唯一失败均为 Q995：`many_archive_volumes_never_hold_more_than_the_one_being_processed`，`tests/concurrency.rs:755`，macOS 重命名断言实际 0、期望 96。合计只计停止前真正执行到的测试。
- 闸门 3：**绿**。末行：`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.36s`。日志 `/tmp/hs08-gate3.log`。
- Polish：**四条绿**，fmt、两种 clippy、doc 都已跑；lib doc **15 条既有告警**，另有既有 `fetch_update` 弃用告警，未隐藏。末行：`Generated /private/tmp/tonefit-honest-screen/08/target/doc/tonefit/index.html`。日志 `/tmp/hs08-polish.log`。
- `git diff --check`：绿。功能无遗留缺口；仅保留批准范围外的 Q995 基线红，不声称全仓测试全绿。
