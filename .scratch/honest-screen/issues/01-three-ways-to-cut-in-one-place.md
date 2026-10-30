# 01 — 三种截法收在一处

**What to build:** 《让位》改成按哪一头要紧截、三种各只有一处出处（2026-10-02 拷问定案）：**从中间省略**（抬头、卷名、路径，已有）、**末尾 `⋯`**（设置栏一行）、**前面 `⋯`**（输入行的缓冲）。后两种还没有。先把三种收进同一处、各一个实现、用例逐种钉住，画法里各自截字的那几处改调它——**屏上一格不变**。02 与 03 用它。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 三种截法各一个实现、住在同一处；画法里不再有各自截字的那几处
- [x] 纯函数用例逐种钉住：宽字与窄字混排、刚好摆得下、差一格、宽度窄过 `⋯` 本身
- [x] 全部设计快照与交互期望屏照旧绿（屏上一格不变）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（已逐条跑；1、2 只在明确排除的基线 Q995 处红，3 绿，见《数》）

## 不做会怎样

02 与 03 各写一套截法，《让位》「三种各只有一处出处」从落地那天起就不成立。

## 落地记录

基于 `hs-01` / `e9309bb25a28507349955235703393a1a42ddc97`，本票是机械收拢，屏上一格不改。

- `src/session/columns.rs`：三种省略共用 `elide_at`，`None` 从中间省略，`Some(Keep::Start)` 末尾放 `⋯`，`Some(Keep::End)` 前面放 `⋯`；已有 `elide` 保持调用方式与两头分配规矩。三种省略与不加记号的 `clip` 共用显示宽度取字循环。
- `src/session/shell/canvas.rs`：删除本地裁剪循环；画一行与补齐改向 `columns::clip` 要字。
- `src/session/shell/footer.rs`：输入行照旧留开头、不加记号，只把裁剪出处改到 `columns`。02、03 再启用本票备好的两种带记号截法。
- 代价：一份明确点名保留哪头的枚举与一个共享入口；没有新依赖、特性、抑制、设计稿或夹具变动，也没有修改黄金快照或 `.gitattributes`。未变措辞，不适用「删除假话」的负断言。

验收接缝沿用票据指定的纯函数与既有整屏验收。先添三种省略与无记号裁剪的字面期望值，首次运行在缺少 `Keep` / `elide_at` / `clip` 处编译红（`/tmp/hs-01-red.log`）。实现后纯函数 10 通过；整组 `session::` 326 通过、1 个既有忽略，设计快照及终端交互期望屏全绿（`/tmp/hs-01-columns.log`、`/tmp/hs-01-session.log`）。

守卫按反与反转实跑（均还原后复验绿）：

| 改坏哪一处 | 实际红证据 |
| --- | --- |
| 刚好摆得下 `<=` 改 `<` | 三种省略用例与既有卷名用例失败 |
| 零宽守卫改成只挡 `usize::MAX` | 三种省略用例、极窄列用例失败，减法溢出 |
| 末尾 `⋯` 改到前面 | 三种省略字面期望失败 |
| 前面 `⋯` 改到末尾 | 三种省略字面期望失败 |
| 中间省略交换头尾 | 三种省略与既有卷名、极窄列用例失败 |
| 取字边界 `>` 改 `>=` | 三种省略、无记号裁剪与既有两条省略用例失败 |

六次反转各退出 101，日志 `/tmp/hs-01-mutation-{exact-fit,zero-room,end-ellipsis,start-ellipsis,middle-ellipsis,wide-boundary}.log`。

所有 Cargo 均经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...`。共享 target 即使串行也会复用另一 worktree 的新鲜测试二进制：一次初跑零宽反转竟只列出基线的 8 条而非本票的 10 条，因此该次结果作废。后续统一加 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-01-rustc-workspace.sh`（内容仅 `exec "$@"`，不改编译参数），隔离 workspace 产物哈希、仍复用第三方依赖；六次反转全部重跑并确认各列出本票 10 条。闸门与 polish 也用该环境。

闸门 1、2 都在已知 macOS 基线 Q995 `tests/concurrency.rs:755` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed` 失败：重命名检出力断言得到 `0`、期望 `96`。照已批准范围不修、不弱化、不跳过；闸门在此停下，后续集成二进制未跑。该例不影响本票纯函数、全部会话验收与构建验证；因此按基线例外结票，「三条全绿」保留未勾。闸门 3 与 polish 四条全绿。

交付保持未提交（上级执行约束禁止实现代理改索引或提交）；集成分支核验仍在起点，无新集成 tip 需要合并。

## 数

以下为本票未提交改动验证时的实际输出；日志均在仓库外。

- 闸门 1（`/tmp/hs-01-gate-1.log`）：红，合计 **757 通过 1 失败；lib 255 / bin 483**。
  - `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.77s`
- 闸门 2（`/tmp/hs-01-gate-2.log`）：红，合计 **616 通过 1 失败；lib 255 / bin 342**。
  - `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.18s`
- 闸门 3（`/tmp/hs-01-gate-3.log`）：绿。
  - `Finished dev profile [unoptimized + debuginfo] target(s) in 2.46s`
- polish（`/tmp/hs-01-polish.log`）：`fmt --check`、两种构建的 clippy、`doc --no-deps` 全绿。初跑仅有新增用例数组的排版差异，按 rustfmt 调整后重跑四条。
  - 默认 clippy：`Finished dev profile [unoptimized + debuginfo] target(s) in 6.45s`
  - 无默认特性 clippy：`Finished dev profile [unoptimized + debuginfo] target(s) in 5.42s`
  - 文档：`warning: tonefit (lib doc) generated 15 warnings`（既有私有链接告警，未增加）；`Generated /private/tmp/tonefit-honest-screen/01/target/doc/tonefit/index.html`
  - 编译 / clippy 另有既有 `src/main.rs:1193` 的 `fetch_update` 弃用告警，本票不改。
- `git diff --check` 通过。
