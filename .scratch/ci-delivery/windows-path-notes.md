# Windows 安全路径前缀等价

## 真实红灯

Windows Server 2022，Actions run **41295011993**，源 **49fb856**；日志 `/tmp/ci-49fb-windows-failed.log`，2594 行起。默认库／二进制测试通过（lib 256、bin 509），默认闸门合计 876 passed / 2 failed，后两道未跑。

- `relative_and_absolute_spellings_of_one_library_do_not_collide`：相对 `库` 与 `\\?\C:\Users\runneradmin\…\库` 未收编，输出 `out\库` 及其中归档撞名，进程实际退出 1，期望 0。
- `relative_and_absolute_nested_named_paths_do_the_work_once`：实际同时输出 `作品/第1话.cbz`、`作品/第2话.cbz` 与 `库/作品/第1话.cbz`、`库/作品/第2话.cbz`，期望仅后两条。

父任务提供的同 SHA Linux 与两种 macOS 完整质量／包验收成功，不代表本次修复树通过，也不代表 Windows 成功。

## 设计与落点

用户已明确批准「补安全前缀等价」。词义归 `GLOSSARY.md`《同一处》，非平凡命名空间规则与取舍归 [ADR 0022](../../docs/adr/0022-safe-windows-path-prefix-equivalence.md)（保持 draft）。

生产修改只在 `src/place.rs`：接当前目录之后、分量规整与大小写折叠之前，检查扩展原串是否属于安全盘符／UNC 子集，安全才转换前缀。其余命名空间保留区别；不做 IO、canonicalize、软链或 `..` 解析，不改报告写法。检查包括 UNC 服务器／共享名，不能让分量迭代器先吞掉特殊语义。普通写法仍沿用既有 `.` 规整，不把扩展写法的点分量解释成它。

两条 Windows 条件纯表测覆盖盘符／UNC、双向比较、两档大小写、键／深度／祖先／below、设备保留名及扩展名／¹²³／CONIN$／CONOUT$（编号 0 也保守保留）、尾点空格、点分量、分隔符、流／禁用字符、其他命名空间和未配对 surrogate。`tests/discovery.rs` 的原 canonicalize 来源、输出与报告断言全部不动；黄金与夹具不动。

查阅的 Microsoft 一手文档：
- [Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)，正文副本 `/tmp/ci-windows-primary-naming.md`。
- [Maximum Path Length Limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)，副本 `/tmp/ci-windows-primary-maxpath.md`：扩展路径不作正斜线／点／双点转换。
- [Console Handles](https://learn.microsoft.com/en-us/windows/console/console-handles)，副本 `/tmp/ci-windows-primary-console.md`：CONIN$／CONOUT$。

## 本地验证

本机 macOS ARM64；实际 Rust **1.98.0 (88d9e12ae)** 位于 `/tmp/tonefit-rust198`。所有 Cargo 调用 `CARGO_INCREMENTAL=0`，经 `/tmp/tonefit-honest-screen-cargo.py` 串行；原生环境与完整命令见 `/tmp/ci-windows-prefix-verify.sh`（静态 dav1d、部署目标 15.0，无伪装 cfg 或编译 wrapper）。闸门与 polish 日志分别为 `/tmp/ci-windows-prefix-gates.log`、`/tmp/ci-windows-prefix-polish.log`，完成结果记录在下方。

匹配 Rust 1.98 的 `x86_64-pc-windows-msvc` std 已安装到临时工具链（下载／安装日志 `/tmp/ci-windows-std-download.log`）。`/tmp/ci-windows-prefix-check.py` 从真实 `src/place.rs` 提取完整生产模块与新增的两条 Windows 表测，补独立测试模块的 imports／same 调用，使用实际 Windows target `rustc --test --emit=metadata` 类型检查；不模拟 Windows cfg、不换 Windows Prefix 实现、不链接、不执行。脚本／提取文件／日志为 `/tmp/ci-windows-prefix-check.{py,rs}`、`/tmp/ci-windows-prefix-metadata.log`。这只证明该 seam 的 Windows 类型检查，不是全项目 Windows 构建或运行证明。

结果（`gates`／`polish`／`metadata` 三个 `.exit` 文件均为 0）：

| 检查 | 结果 |
| --- | --- |
| 默认闸门 | 1192 passed / 0 failed；lib 257 / bin 509 |
| 无默认特性闸门 | 1034 passed / 0 failed；lib 257 / bin 351 |
| profiling 闸门 | check 成功，2.41s |
| polish | fmt、两档 clippy、doc 全绿；doc 15 条既有告警，与 `/tmp/ci-native-fix-polish.log` 相同 |
| Windows 元数据 | 成功；提取 harness 的未使用项有 4 条 dead-code 告警；没有 Windows 执行证据 |
| 其他 | `git diff --check`（`/tmp/ci-windows-prefix-diffcheck.log`）与 spec graph 校验通过 |

两档 discovery 均 26/26，包括原两条相对／绝对回归；黄金均 2/2，快照未动。这是 macOS 行为，不是 Windows 行为。`TONEFIT_SAMPLES` 未设：smoke 未跑真实卷，perceptual 的 M/P 两组明确缺样本跳过；不能把总绿灯写成那些素材已验证。

工作流、交付 helper 与设计稿均未改。本次复用而未重跑的已有结果：46 helper tests（`/tmp/ci-candidate-fix-helpers.log`）、actionlint（`/tmp/ci-candidate-fix-actionlint.log`）、设计导出逐字节一致（`/tmp/ci-native-fix-final-design.log`）、JS 13/13（`/tmp/ci-native-fix-final-js.log`）。

## 原生证明尚待完成

Windows 原生 CI 必须在最终修复提交重新跑三道闸门、polish 与包验收，尤其确认上述两条原集成回归、新增两条表测、原报告写法断言。守卫反验证仍需 Windows 原生证据：暂时去掉前缀转换应被阳性表／原集成回归检出，绕过安全检查应被危险名字阴性表检出；本机没有执行这些 Windows 反事实，不声称已看见红灯。Windows 10/11 客户端仍未实测；Server 2022 成功也不能替代客户端验收。最终四平台提交级验收／发布由父任务继续，本次不提交、不推送、不打标签、不发布。

## Windows 原生证明接线（本次追加，尚未远端执行）

新增 `scripts/ci/windows_prefix_proof.py`，仅 Windows workflow step 调用，位于 Static dav1d／provenance 后、Three build gates 前。CLI 不提供 OS 绕过开关；核对真实 Windows x64、rustc host `x86_64-pc-windows-msvc`、Rust/Cargo 1.98.0，以及 tracked 源码干净。沿用原生 job 环境，不改变 target/CRT 配置。

1. `cargo test --locked --lib place::tests::safe_windows_prefixes_share_keys_without_changing_the_tree -- --exact --nocapture --color never`，要求 1 passed / 0 failed。
2. 同命令 exact 运行 `place::tests::windows_namespace_semantics_are_not_erased_by_prefix_equivalence`，要求 1 passed / 0 failed。
3. 只把唯一原串 `if !safe_windows_name(name) {` 临时换为 `if false && !safe_windows_name(name) {`，重跑 exact 阴性表，必须实际 panic、exit 101、0 passed / 1 failed、0 ignored；编译失败、基础设施失败、错名、零测试均拒绝。
4. `finally` 恢复原始 bytes（含换行），核对 SHA256 与 tracked clean；即使调用异常或红灯不合要求也先恢复再失败。恢复后重跑两表各 1 passed，再检查 tracked clean。五份 UTF-8 工具日志与前后 SHA256 放在 `$RUNNER_TEMP/windows-prefix-proof`，Actions 控制台也打印实际 Rust 名字、计数及哈希。
5. 随后 Windows-only `cargo test --locked --test stop` 及早检查既有控制台／oplock 测试。原完整三构建闸门、polish、设计与包验收不改、不替换、不加条件；只有恢复后正常源码可进入后续构建。原 discovery 两条不额外重复，仍由完整闸门执行。

这是刻意耦合生产守卫的 fault injection，不是 test-only hook；守卫原串或表名改动必须同步脚本与 helper tests。绕过 raw-name 守卫的原生反验证由此次接线提供；去掉整个前缀转换的额外反事实未接线，原两条集成红灯仍是上文已有的旧源码证据。

本次实际本机检查（macOS，不是 Windows 运行证据）：
- `python -m unittest discover -s scripts/ci/tests -v`：54/54（`/tmp/ci-windows-proof-helpers.log`）；新增 fake external-process adapters 覆盖 green/red/restore/green、异常恢复、错失败／编译失败拒绝、守卫更名拒绝、dirty tracked 拒绝及 UTF-8。它们不声称执行 Windows `Place`。
- `actionlint`：通过。
- `.scratch/session-redesign` 中 `npm run check`：逐字节相同；`npm test`：13/13，无 skip。
- spec graph 校验、`git diff --check`：通过。

未在本机运行 Windows CLI 证明或 Windows stop；未重跑 macOS Rust 完整闸门（生产源码未由本次接线改动，前述 macOS 实测仍只代表其平台）。最终提交 Windows 原生 guard 红／绿、stop、三闸门、polish、包验收及四平台远端同 SHA 结果仍待父任务推送后确认。没有提交、推送、标签或发布操作。
