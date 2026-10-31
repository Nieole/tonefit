# 12 — 相对写法与绝对写法认作同一处

**What to build:** 《同一处》的字面规整**先给相对路径接上当前目录**（自己接，不做任何别的规整，两个平台同一个做法；不用会在 Windows 上算掉 `..` 的那个转绝对路径的函数），再照旧规整、比（《同一处》已写）。用 `库` 与 `/当前目录/库` 两种写法点名同一棵树：同名同级的卷不再被拒绝开始，里外嵌套的不再各做一遍；报告里印的仍是点名时的写法。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 「同一处」判等的纯函数用例：相对写法与绝对写法认作同一处；含 `..` 的相对路径接上当前目录之后 `..` 照旧不被算掉
- [x] 集成用例：两种写法点名同一棵树，同名同级的卷不拒绝开始、里外嵌套的只做一遍，报告里印的是点名时的写法
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

最后一项没有勾：三条已经分别实跑，1、2 在已知基线 Q995 红，3 绿。实现验收通过，基线例外依 spec《Out of Scope》留给 `green-counts/10`，不声称三条全绿。

## 不做会怎样

换一种写法点名同一个目录，结果就变了——有的被拒绝开始，有的做两遍。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1166 — 相对写法与绝对写法不互相认：`库` 与 `/当前目录/库` 是两处

- **From:** 票 `one-source/03`
- **Kind:** 票面定义（字面规整）够不着的一种写法
- **Where:** `src/place.rs` 的 `Place::of`
- **Why it did not block:** 票面把规整写死成「去掉 `.` 分量与尾分隔符，不解析 `..`」；同一条命令行上一个写相对、一个写绝对，场合窄。
- **What this ticket actually did:** 照票面，不补绝对路径。两种写法点名同一棵树时：同名同级的那几卷撞名那一道拒绝开始（去处相同），
  里外嵌套的那几卷各做一遍（去处不同）。
- **Options:** ① 现状；② 取键之前先过一遍 `std::path::absolute`——只接当前目录、不碰盘、不解析 `..`（Unix 上），键变、印的写法不变。
- **Recommend:** ②，但要改《同一处》的定义（「字面规整」多一步），归拍板。
- **Whose call:** 拍板的人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：相对路径先接上当前目录再取键——自己接，不用 `std::path::absolute`（Windows 上会算掉 `..`）；《同一处》已写。

## 落地记录

基于 `e9309bb`，工作区 `/tmp/tonefit-honest-screen/12`，分支 `hs-12`。按上层实现代理约束留作未提交改动；没有改索引、提交或合并历史。收尾读到 integration tip 已到 `d252f9a`（共享截法步骤，仅改会话相关文件，与本票路径不重叠）；本工作区未合并，交协调者按序整合。

- `src/place.rs`：所有同一处键先给相对路径接当前目录，再沿用分量规整与大小写折叠；不使用 `std::path::absolute`，不解析软链或 `..`。判等、收编、借住与撞名仍只有这一个出处。当前目录读不到时保留原来的相对写法，不猜位置。
- `src/discover.rs`：只订正注释，说明相对与绝对写法的深度在接当前目录后比较；候选的卷根、镜像与报告写法不改。
- `tests/discovery.rs`：真子进程接当前目录，同棵混装库的两种点名次序、嵌套路径两种相对/绝对分配及两种点名次序都验。问退出码、输出成员、报告卷数与原始写法；反向断言报告不许印用来比较的绝对写法。仅在测试搭建时解析临时工作区的物理路径，避开 macOS 临时目录软链这个不同问题。
- 代价：相对路径每次取键读取当前目录并分配接好的路径；没有加跨趟缓存，没有新增依赖或公共 API。没有 UI、设计稿、场景夹具、黄金快照、`.gitattributes` 或其它票的修改。现有 owning spec 与词汇表已规定本行为，不需改定义。

### 红 → 绿与反跑

接缝沿用已批准 spec《Testing Decisions》的同一处判等与真命令行发现。所有 Cargo（包括下面命令的 `cargo fmt`）均经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo …` 串行执行，输出在仓库外。

1. 先加判等用例，旧实现实红：`库 在 Sensitive 上与绝对写法没认作同一处`（`/tmp/hs12-unit-red.log`）。接当前目录后 `place::tests` 全绿。
2. 把接当前目录的 `is_relative()` 守卫反成 `is_absolute()`，新集成用例实红：同级混装库报「4 个卷要写到同一批去处」并退出 1；嵌套点名在 `作品/` 与 `库/作品/` 各出一份（`/tmp/hs12-integration-red.log`）。还原后 discovery 全绿。
3. 临时让规整在 `ParentDir` 上弹出一级，用例实红：`Sensitive：接上当前目录时算掉了 ..`（`/tmp/hs12-parent-mutation.log`）；已还原。
4. 临时把收编后的卷根改成绝对写法，真命令行用例的反向断言实红：`报告印了用来判等的绝对写法`（`/tmp/hs12-report-mutation.log`）；已还原。
5. 临时恢复 `HEAD:src/place.rs` 的基线实现，单跑 Q995 对应用例仍红，`left: 0 / right: 96`，与本票两条闸门完全相同（`/tmp/hs12-baseline-q995.log`）；随后恢复本票实现再验绿。没有改、跳过或压制该用例。

## 数

以下为基于 `e9309bb` 的本票未提交实现的实际输出，不计尚未跑到的二进制；三条闸门分别执行，不让闸门 1 的基线红挡住另外两条。

| 命令（均经串行 runner） | 实际结果与末行 |
|---|---|
| `cargo xtask gate 1` | **红，Q995**；合计 756 通过 / 1 失败；lib 256 / bin 481。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.75s` |
| `cargo xtask gate 2` | **红，Q995**；合计 615 通过 / 1 失败；lib 256 / bin 340。末行 `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.12s` |
| `cargo xtask gate 3` | **绿**。末行 `Finished dev profile [unoptimized + debuginfo] target(s) in 2.24s` |
| `cargo xtask polish` | **全绿**：fmt、两种特性的 all-targets clippy、doc。既有 `warning: tonefit (lib doc) generated 15 warnings`，无新增文档告警 |
| `cargo test --lib place::tests` | **12 通过 / 0 失败**；含相对/绝对、`.`、空路径、大小写与 `..` 反向断言 |
| `cargo test --test discovery --test container --test isolation` | **99 通过 / 0 失败**：discovery 26、container 51、isolation 22 |
| `CARGO_TARGET_DIR=target/gate/no-default-features cargo test --no-default-features --test discovery relative_and_absolute` | **2 通过 / 0 失败**，使用闸门 2 专属目录 |

日志：`/tmp/hs12-gate{1,2,3}.log`、`/tmp/hs12-polish.log`、`/tmp/hs12-place-final.log`、`/tmp/hs12-integrations-final.log`、`/tmp/hs12-no-default-final.log`。

尚未验证 Windows 真机；`..` 用例不挂平台条件，Windows 上同样会跑。本机 macOS 基线 Q995 留在范围外，没有把它写成绿色。

## 当前 integration 合入验证

在 `implement-honest-screen` / `3eb5bb913972ed6622c2206e101220a374e974ff` 上，将源工作区
`/tmp/tonefit-honest-screen/12`（`hs-12` / `e9309bb25a28507349955235703393a1a42ddc97`）
的四个本票 tracked 路径用 `git diff --binary` 导出为 `/tmp/hs12-transfer.patch`；先
`git apply --check` 成功，再 apply，无冲突、不需重建 hunk。`tests/discovery.rs` 保留 integration
已有 ticket10 文档变化：临时副本仅反向移除本票 patch 后与 integration HEAD 字节完全相同。
`src/place.rs`、`src/discover.rs` 与源交付字节相同。既有 `.gitattributes` 修改哈希不变，黄金快照未动。
所有改动未提交。

以下是本次 **当前 integration** 实跑，不是上文源工作区旧结果。全部 Cargo 串行经
`python3 /tmp/tonefit-honest-screen-cargo.py cargo …`，统一
`CARGO_INCREMENTAL=0 RUSTC_WORKSPACE_WRAPPER=/tmp/hs-integration-rustc-workspace.sh`；
no-default 两条另设 `CARGO_TARGET_DIR=target/gate/no-default-features`。

| 当前命令（runner 后的参数） | 实际结果 | 日志 |
|---|---|---|
| `test --lib place::tests` | 12 passed; 0 failed; 244 filtered out | `/tmp/hs12-integration-current-place.log` |
| `test --test discovery --test container --test isolation` | 99 passed; 0 failed：container 51、discovery 26、isolation 22；均无 filtered | `/tmp/hs12-integration-current-suites.log` |
| `test --no-default-features --lib place::tests` | 12 passed; 0 failed; 244 filtered out | `/tmp/hs12-integration-current-no-default-place.log` |
| `test --no-default-features --test discovery relative_and_absolute` | 2 passed; 0 failed; 24 filtered out | `/tmp/hs12-integration-current-no-default-discovery.log` |

当前合计 125 次测试通过，0 失败；含两档大小写的相对/绝对判等、`..` 不消去，以及两种点名次序、
嵌套写法组合、报告保留原始写法与不印比较用绝对路径的反向断言。`git diff --check` 通过。
完整闸门与 polish 本次未重跑，按协调者要求留最终整体验证；上文 Q995 与 Windows 真机限制仍保留，
不把源工作区的闸门结果当作当前 integration 的结果。
