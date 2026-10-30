# 07 — 需留意的那一卷、那一句

**What to build:** 预览过的那一卷有需留意的页（坏页也算）时**行首 `!`、`]d` 落得到**、目录行与分区算一处需留意，行尾仍是「已分析，未写出」；没有时 `✓`（《卷状态》已写）。屏上一律用词条名「**需留意的页**」——一页都不需留意的卷进去那一句写「这一卷没有需留意的页」。设计稿先改，补带坏页的预览过那一卷与一页都不需留意的卷各一屏，再改实现。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读；重导之后既有快照的变动只落在本票牵到的那几处
- [x] 带坏页的预览过那一卷：行首 `!`、`]d` 落得到、目录行算一处，导一串比整屏
- [x] 一页都不需留意的卷进去那一屏导出、比整屏；留一条反着钉的断言：屏上不再出现「需要留意」
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

总览挂着「坏页 N 页 ⋅ ]d → 跳到下一个」，按下去跳不到那一卷；同一个词设计稿与实现两种写法。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1168 — 预览过的那一卷带坏页时，照设计稿行首是 `✓`、`]d` 跳不到——而词汇表读下来该是 `!`、跳得到

- **From:** 票 `design-parity/04`
- **Kind:** 词汇表与实现对不上（本票照设计稿走，让这一处到得了）
- **Where:** `design.html` 的 `volMark`（第 843 行：`trialed` 与 `done` 同一支，只看差异大／超宽／尺寸／残缺，不看坏页）与 `targets`（第 1651 行，同一个条件），
  `dirStat`（目录行的 `warn` 同样不数它）；`src/session/shell/list.rs` 的 `volume_row`（交给 `volume_mark` 的是 `NotableTally`，它不数坏页）、
  `src/session/live.rs` 的 `troubled_at`（第 1068 行）；`CONTEXT.md` 的《需留意的页》（坏页是其中一种）、《行首记号》（「`!` 需留意（进了隔离、有需留意的页）」）、
  《卷列表》（`]d` 跳「有需留意的页的卷」）；票面原句「行首记号与完成同一套（有需留意的是 `!`，没有是 `✓`）」
- **Why it did not block:** 不是停线：两个方向都说得通（ADR 0019 决定第 13 条设计稿是权威，而词汇表与票面那半句读下来是 `!`），
  翻过来只动票面牵到的那几处——设计稿 `volMark`／`targets`／`dirStat` 里 `trialed` 那一支、实现交给行首记号的那一格、`troubled_at`、目录行的 `warn`；
  没有一景、一串里有带坏页的预览过的卷，改哪一边既有快照都不动。不可逆的一步都没有。
- **What this ticket actually did:** 照设计稿。分析环节里坏了页、确认点上答「不写出」的那一卷收在预览过（不是进了隔离：一个字节都没写，
  没有东西可挪——本票之前它收成进了隔离，行首 `!`，行尾却说「这一卷整卷写到隔离目录」，那句是假话）；它行首 `✓`、行尾「已分析，未写出」，
  `]d` 跳不到它，而总览的问题行照数「坏页 N 页」、挂着 `]d → 跳到下一个`。`CONTEXT.md`《卷状态》新添的那一条写的是票面那句「行首记号与完成同一套」，
  没替这一格判；完成的卷带不了坏页（带了就是进了隔离），「同一套」在这一格上两种读法都说得通。
- **Why it matters:** 屏上一行说「没事」（`✓`），而那一卷有坏页；问题行报了坏页，`]d` 却落不到它——《卷列表》那句「`]d` 跳的是有需留意的页的卷」在这一格上不成立。
- **Options:** ① 照旧（坏页只在问题行、目录行的汇总与每页结果里露面），《行首记号》或《预览过》注明这一格例外；
  ② 设计稿 `trialed` 那一支也认坏页：行首 `!`、`]d` 落得到、目录行算一处需留意，行尾仍是「已分析，未写出」（坏页不给词，`marks::notable_word`）；
  先改设计稿、补一串带坏页的预览过的卷钉住，再改实现。
- **Recommend:** ②。词汇表三处读下来是同一个答案，设计稿那一支是从完成那一支借来的、没碰上过这一格。
- **Whose call:** 拍板的人（设计稿与词汇表谁让）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：带坏页的预览过那一卷行首 `!`、`]d` 落得到、目录行算一处需留意，行尾仍「已分析，未写出」；设计稿先改（《卷状态》已写）。

#### Q1169 — 一页都不需留意的那一卷，每页结果那一句设计稿写「需要留意的页」、实现写「需留意的页」，没有一串钉着

- **From:** 票 `design-parity/04`
- **Kind:** 路过撞见的设计稿与实现对不上（本票想导的那一串撞上了）
- **Where:** `design.html` 的 `drawPages`（第 1344 行：「这一卷没有需要留意的页 ⋅ 」）；`src/session/shell/pages.rs` 第 353 行（「这一卷没有需留意的页 ⋅ 」）；
  `CONTEXT.md` 的《需留意的页》
- **Why it did not block:** 与预览过无关——任何一页都不需留意的卷进去都是这一句；硬约束是设计稿只改票面牵到的那几处。
  翻过来是设计稿那一句换成词汇表的叫法、重导，再导一串钉住它。
- **What this ticket actually did:** 本想导「进那一卷」那一串（第 6 卷一页都不需留意），比到这一句红了；改成进去再按 `a` 列全部页
  （`deciding-x-advance-s-trialed-l-a`），这一句不在屏上。两边谁都没改。
- **Why it matters:** 这一屏到今天没有一串期望屏，两副措辞各说各的，换哪一边都没有东西会红。
- **Options:** ① 设计稿改成词汇表那个叫法「需留意的页」（先例：Q731「差异大的页」一个叫法），重导、补一串一页都不需留意的卷进去那一屏；
  ② 实现改成设计稿那一句；③ 照旧。
- **Recommend:** ①。《需留意的页》是词条名，屏上那一句该用它。
- **Whose call:** 拍板的人（设计稿）；开不开票归协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：设计稿改用词条名「需留意的页」，补一屏一页都不需留意的卷钉住。

#### Q1253 — 一页都不需留意那一句，设计稿写「需要留意」、实现写「需留意」，没有一屏钉着它

- **From:** 票 `design-parity/10`
- **Kind:** 设计稿与实现对不上（改设计稿 `drawPages` 时撞见）
- **Where:** `design.html` `drawPages` 的 `这一卷没有需要留意的页 ⋅ `；`src/session/shell/pages.rs` 的 `draw`（`这一卷没有需留意的页 ⋅ `）
- **Why it did not block:** 本票没导出列需留意的页而一页都没有的那一屏（「按页跳过」那一景列的是全部页），两边谁都没红；改哪一边都只是一句话加一串新导出。
- **What this ticket actually did:** 没动，记在这里。
- **Why it matters:** 那一屏一导出就对不上；词汇表的词是《需留意的页》。
- **Options:** ① 设计稿改成「需留意」（词汇表那个词），从「按页跳过」那一景导一串 `retained-a`（切回需留意的页，一页都没有）钉住；② 实现跟设计稿写「需要留意」。
- **Recommend:** ①。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 并入 Q1169，随它走（拷问定案 2026-10-02：同一句措辞）。


## 落地记录

- 交付：先改既有设计稿，新增 `bad-page-deciding` 场景（只有坏页、没有其他需留意项，路径展开为分区），导出 `bad-page-deciding-s-jump`、`bad-page-deciding-s-collapsed`、`deciding-trialed-notable-jump`、`retained-no-notable-pages`。坏页预览过的卷行 `!`，`]d` 选中它，目录与分区汇总把它算一处；收起目录仍为 `!`。有其他需留意项的预览过卷行也保留「已分析，未写出」，不冒称已经写出或隔离。没有需留意页的空屏用「这一卷没有需留意的页」。
- 实现：`live::troubled_at` 认 Trialed 的坏页；列表行首与跳转同源，目录/分区计入该卷，Trialed 行尾不被其他需留意项取代。场景测试不再把「报告中有坏页」误等同于「卷状态已进隔离」；快照场景数随新增场景更新。终端测试经真实键码翻译走四串输入，逐格比字与样式，并禁止「需要留意」「整卷写到隔离」「隔离 1 卷」。
- 成本：新增一景、四串，设计导出新增四份快照网格与十二份序列文件、一份场景数据；既有快照/序列文件没有字节变化。未改依赖、黄金快照或 `.gitattributes`。
- TDD：中断前 `/tmp/hs07-design-red.log` 的两条新设计测试先红；`/tmp/hs07-rust-red.log` 的终端整屏测试先红。恢复后先完整重导并逐字节检查，才重新验实现。旧 `design-green`/`npm-test` 日志曾因多余旧导出文件不绿，旧 `check`、闸门 2/3、polish 曾遭 ENOSPC，均不能当作最终代码结果；恢复后的日志统一用 `/tmp/hs07-final-*`。
- 守卫反跑：设计与 Rust 各独立撤掉行首坏页判断、跳转坏页判断、目录/分区坏页计数、未写出行尾，并把空页措辞改回「需要留意」，五项分别红。Rust 五项均是运行一条测试后断言失败（退出 101），不是编译或磁盘失败：jump 光标落错，mark 第 10 行 `✓` 应为 `!`，branch 第 8 行 `◌` 应为 `!`，suffix 第 10 行缺「已」，word 第 10 行「要」应为「留」。日志 `/tmp/hs07-final-reversal-{jump,mark,branch,suffix,word}.log`；设计对应 `/tmp/hs07-final-design-reversal-{jump,mark,branch,suffix,word}.log`（各退出 1）。全部恢复后 Rust 1 passed、设计 6 passed，见 `final-restored.log`、`final-design-restored.log`；反跑脚本在 `/tmp/hs07-final-reversals.py`、`/tmp/hs07-design-reversals.py`。
- 最终验证：`npm run export` 导出 32 份快照、205 串，`npm run check` 逐字节一致，完整 `npm test` 10 passed / 0 failed（含全树文件集合与两次导出确定性检查，排除 ENOSPC 截断及旧文件残留）；日志 `/tmp/hs07-final-{export,check,npm-test}.log`。`cargo test --bin tonefit session::` 为 338 passed / 0 failed / 1 ignored，日志 `/tmp/hs07-final-session.log`。`git diff --check` 绿。
- 所有恢复后的 Cargo 都经 `/tmp/tonefit-honest-screen-cargo.py` 串行运行，并设 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-07-rustc-workspace.sh`、`CARGO_INCREMENTAL=0`。闸门分别执行，不借第一条提前停止；脚本 `/tmp/hs07-final-checks.sh`，状态 `/tmp/hs07-final-results`。
- 未完成的勾选仅为「三闸全绿」：闸门 1、2 均只在已知基线 Q995 `many_archive_volumes_never_hold_more_than_the_one_being_processed` 红（`tests/concurrency.rs`）；本票明确不修也不放宽。它们提前停止，后续集成二进制未跑到，不声称全套 Rust 绿。闸门 3 与 polish 绿；无本票遗留功能缺口。

## 数

恢复后的最终日志（`/tmp/hs07-final-gate{1,2,3}.log`、`/tmp/hs07-final-polish.log`）：

- 闸门 1：红；合计 769 通过、1 失败；lib 255 / bin 495。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 50.26s`（Q995）。
- 闸门 2：红；合计 617 通过、1 失败；lib 255 / bin 343。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.99s`（Q995）。
- 闸门 3：绿；末行 `Finished dev profile [unoptimized + debuginfo] target(s) in 2.83s`（原日志的 `dev` 带反引号）。
- polish：四条全绿；fmt、两种 clippy、doc 均完成。文档 `warning: tonefit (lib doc) generated 15 warnings`（原日志的 `tonefit` 带反引号）；既有 `src/main.rs` 的 `fetch_update` 弃用告警未改，未加 suppressions。
