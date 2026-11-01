---
id: tonefit-delivery
type: module-design
title: 交付：质量验收与受信构建边界
status: active
parent: tonefit-architecture
references: [task-ci-delivery]
---

# 交付

## 职责与边界

交付 seam 在十个运行时边界之外：把一个确定提交的源码变成可追溯的质量结论，
生成同源可运行包及最终归档的验证记录。它调用本地闸门，不定义业务行为、不替换解码器、
不更改 vendored UnRAR；本地命令与各自 target 目录仍以
[闸门](../docs/agents/gate.md) 为唯一出处。

## 质量契约

PR、所有分支 push（包括 main）和手动验证运行四个原生任务，先检查系统与架构：

| runner | target |
|---|---|
| ubuntu-22.04 | x86_64-unknown-linux-gnu |
| windows-2022 | x86_64-pc-windows-msvc |
| macos-15 | aarch64-apple-darwin |
| macos-15-intel | x86_64-apple-darwin |

Rust 1.98.0；Node 24.22.2 LTS；Cargo 解析锁定，设计验收用 npm ci。
每个平台完整执行三构建闸门、polish、设计导出一致性与全部 JS 测试、交付辅助程序测试。
矩阵不 fail-fast，原生任务上限 180 分钟（完整闸门加打包），汇总 5 分钟；
汇总守卫要求所有必需任务 success，失败、取消、跳过均不算成功。
既有外部素材缺失报告不等于素材实测；不新增跳测或 continue-on-error。
交付 Python 辅助程序与测试对已知 UTF-8 源码、JSON、说明和捕获的工具文本显式使用 UTF-8，
不依赖 Windows 本地代码页；回归覆盖非 ASCII 路径、许可内容及工具输出。

Windows 原生任务在静态原生环境建立后、三构建闸门之前运行安全前缀证明：
`windows_prefix_proof.py` 核对真实 Windows x64 与 MSVC Rust/Cargo 1.98.0、干净 tracked 源码，
按 exact 名字运行 `Place` 阳性／阴性表（各 1 passed）；临时只绕过 `src/place.rs` 的
`if !safe_windows_name(name) {` 原串守卫，要求阴性表实际 panic、exit 101、0 passed / 1 failed。
编译／基础设施错误或零测试不算红灯。finally 按原始字节恢复源码、核对 SHA256 与 tracked 干净状态，
再跑两表各 1 passed；保留工具输出及恢复前后哈希日志。守卫／表名更名须同步证明脚本与 adapter 测试，
不添加生产 test hook。随后 Windows 额外先跑 `cargo test --locked --test stop`，
及早暴露控制台／oplock 故障；完整三闸门仍无条件在恢复后运行并重复全部既有测试，
mutant 不进入打包。非 Windows 不运行这两项预检；本机 fake adapter 只证明编排与恢复，不证明 Windows 行为。

## 原生构建输入

`native.py` 的固定 URL 与 SHA256 是 dav1d 源码字节的权威输入；版本 1.5.4 满足
当前绑定要求的 >=1.3。校验失败或不安全归档直接拒绝，源码不由 Cargo.lock 覆盖。
每次全新目录构建静态库，禁用绑定的内部网络构建，并通过安装后的 pkg-config 元数据定位。
Meson/Ninja 固定版本；编译器、汇编器、pkg-config 等宿主工具版本、架构、runner image、
源码哈希、构建参数和提交记入 provenance。工具链设置不含 target-cpu=native。
Windows 使用 MSVC，Rust/C++/dav1d 统一静态 CRT；环境键按 Windows 大小写不敏感语义归一化，从已初始化工具链的 VCToolsInstallDir
选取并验证 Hostx64/x64 的 cl/link/lib，不经 Git Bash PATH 猜测，Meson 使用显式 native file，
后续 Cargo/cc 使用绝对工具路径。Linux 显式链接 UnRAR 需要的 C++ 标准运行库：
`-l static=stdc++` 只覆盖 kind，不覆盖源码 link attribute 的 modifiers；Cargo 构建前用真实 C++ 符号与
同形 Rust attribute 构建 RLIB、链接并运行其 consumer，检查 ELF 无动态 libstdc++。
先解析并哈希 c++ 定位的 libstdc++.a，以显式 native 搜索目录及 kind-only flag 通过
CARGO_ENCODED_RUSTFLAGS 保留带空格路径；缺搜索目录的 RLIB 反例必须失败。记录编译器、静态库路径与哈希。
随包保留 GCC 运行库许可与例外；macOS 部署目标 15.0。
这些构建设置不能代替最终归档的依赖审计与运行验证。

## 信任边界

默认 contents:read；checkout 不保留凭据；不使用 pull_request_target、写令牌或秘密运行 PR。
Action 引用完整提交 SHA。当前不保存 Cargo/native 缓存，PR 构建不向将来受信发行注入输出。
本模块不创建或移动标签。`release.yml` 仅响应 alpha/beta/rc 阶段标签，调用同树的
`quality.yml` workflow_call，在该标签完整 SHA 上重新执行四平台全部验收及打包。
PR/分支包仅为临时验收资产，不得提升为受信发行输入。批准的完整交付范围见 task-ci-delivery。
阶段 job 在执行源码 helper 前，以无插值 shell 数据核对远端标签 commit 与新取 origin/main 祖先关系；
只有 main 历史内代码进入发布权限边界。工作流及 main/标签写入者本身是受信配置边界；本次不修改仓库规则。

## 打包与最终字节验证

每个矩阵任务在三闸门、polish、设计验收全部成功后，用同一静态原生环境构建默认 TUI release。
Cargo 应用版本、包名与程序 --version 一致；当前为 0.1.0-alpha.1，无依赖升级。
每目标仅一个 `tonefit-<version>-<exact Rust target>` 顶层目录：Windows ZIP，Unix tar.gz 保留执行位。
只归档一次；macOS strip 后 ad-hoc 签名再归档，不声称 Developer ID 或公证。

包内包含程序、MIT、使用/系统与未签名提示、第三方通知、完整许可材料和来源信息。
许可清单从目标的传递 normal/build 依赖图与 release 编译产物交集收集（避免 metadata 的 dev 特性合并），
保留上游递归许可/版权/通知文件；少数漏装全文的锁定 crate 使用 `licenses/rust-supplemental/` 中
按原始 VCS revision/许可声明核对的 MIT 全文补充，未识别的缺文本就失败；
不把 Cargo.lock 的包名列表当完整许可。额外包含 Rust 标准库及其第三方版权、静态 dav1d BSD-2
与 x86 ISC、UnRAR、Unifont，以及 Linux GCC 运行库许可。构建来源记录 SHA、指向该 SHA 的本地标签
（若有）、dirty 状态、目标、Rust/native 版本、锁文件哈希及 runner/toolchain 资料。

`package.py` 对最终归档先验 SHA256，再在新目录校验并解压：拒绝越界路径、别名、重复/大小写碰撞、
链接、特殊文件与文件/目录冲突；Unix 执行位在实际 TAR entry 元数据边界强制检查（含 Windows 宿主），
在 Unix 宿主额外检查解压后的文件系统执行位，不依赖 NTFS 表示 Unix mode；检查清单哈希、必需许可、版本/目标来源、真实二进制架构与 Unix 执行位。
依赖审计采用拒绝未知项的系统库白名单，不从 System32 或 Homebrew 随意复制库：
Linux ELF 全部 NEEDED 必须是基线系统库，无 RPATH/RUNPATH，GLIBC 符号最高 2.35；
Windows PE 仅系统 DLL/core API sets，拒绝动态 CRT/未知 DLL；macOS 仅 /usr/lib、/System/Library，
部署 15.0、无额外 rpath，并验证签名。当前策略静态链接非系统 native 代码，不接受未经实现的 app-local 闭包。

实际启动解压出的程序（不是 CARGO_BIN_EXE 备用路径），验证 alpha 版本、help 及 AVIF、PNG、JPEG、BMP、
TIFF、WebP、GIF、固实 RAR 转换和输出页。小图由既有 dev AVIF 编码器确定性生成，RAR 复用自有夹具；
普通本地测试明确使用 Cargo 的生产二进制，只证明非打包上下文。Linux 在 helper 固定 digest 的原版
Ubuntu 22.04 容器执行，不安装构建/解码/Python 包；Windows/macOS 清理运行搜索路径并结合系统导入审计。
运行验证不能声称 Windows 10/11 客户端、旧 macOS、旧内核或浏览器下载隔离已实测。

最终 SHA256SUMS 与 verification.json 随每目标包保存为 7 天 Actions artifact，upload-artifact 固定完整
提交 SHA，无新增写权限。只在完整包审计/运行成功后上传；矩阵汇总也因此覆盖打包失败。
校验和证明字节完整性，不证明发行者身份。完整工作流重跑可替换同一 run 的同名临时 artifact，
不覆盖任何 Release 资产。workflow_call 必须给完整 SHA，checkout 后先断言 HEAD 相等；分支/PR 入口不变。

## 阶段发布契约

标签严格为 `v<major>.<minor>.<patch>-<alpha|beta|rc>.<正整数>`，无前导零、额外后缀或稳定标签，
去掉 v 后必须等于该提交 Cargo 版本（当前 0.1.0-alpha.1）。轻量及附注标签均剥离至唯一 40 位 commit；
fresh fetch 明确读取远端标签与 main，要求该 commit 是 main 祖先，不要求它恰好为 main tip。
拒绝远端标签消失、移动、非 main 提交、dirty tracked checkout、错误版本或伪造 CI 上下文。
不手动触发发布，不自动创建标签，也不在配置实施阶段创建实际 Release。

同标签尝试由 concurrency 串行，禁止自动取消在运行的尝试。stage 成功后才能运行 reusable quality；
publish 显式 needs stage 与 quality，二者必须 success，矩阵及 quality-success 的失败、取消、跳过均不能发布。
默认 contents:read，仅 publish 得到 contents:write（及下载本 run artifact 所需 actions:read），
GITHUB_TOKEN 仅来自该 job 的 CI 环境，无嵌入秘密。不存在提权的 fork/PR 入口。

`release.py` 只从当前 release workflow 的 push run 下载四个精确 target/SHA 名的 artifact；
拒绝缺失、多余、重复、过期、错误 run SHA/事件/工作流、传输 digest 不符及不安全 ZIP。
每个 artifact 仅含一份预期 archive、自有 SHA256SUMS、verification.json，后两者必须与最终字节、目标及版本一致。
发行 helper 不执行跨架构程序：复用 package.safe_extract/validate_tree 检查安全路径、架构/权限、许可与 manifest，
再核对包内 commit、clean 状态、阶段标签、锁文件、静态 dav1d 来源及默认 TUI 特性；原生审计及真实运行由同 SHA 矩阵完成。
对这四份最终归档只汇总生成一个全局 SHA256SUMS，不重建归档。

先重查标签/main，以 publish 的写令牌分页枚举 `GET /releases` 的全部结果（包含可见草稿），
严格要求同 tag 至多一个 Release；重复时拒绝，不能选其中一个。`GET /releases/tags/{tag}` 只返回已公开
Release，不能以它的 404 判断没有草稿。仅在全表无同 tag 记录时为已存在的标签创建 draft prerelease。
创建请求的 target_commitish 仅给 `refs/tags/<tag>`，不提供 SHA 或 main 退路；GitHub 文档明确该字段在
标签已存在时不使用，因此不能要求响应或既有草稿的 target_commitish 等于此值，也不能用它证明标签 SHA。
权威标签身份来自反复 fresh fetch 的实际远端 tag commit/main 祖先检查与包内 provenance。
body 中绑定 repository/tag/full SHA 的版本化所有权标记，作者必须 github-actions[bot]。
只恢复 tag、该标记、作者及 draft/prerelease 状态完全一致的本流水线草稿，已有资产必须逐字节相同；
未知/手工草稿或任何已发布 Release 一律 fail closed，不删除、覆盖、替换或重定向。
上传恰好四 archive 与一个 SHA256SUMS。分页枚举全体资产，要求名字/数量/ID/状态/大小均精确，
并重新下载五份资产逐字节比较。最后重新读取草稿状态/所有权，fresh fetch 再核对远端标签与 main 祖先关系，
才一次 PATCH 为 draft:false、prerelease:true。之前任何失败都不主动公开草稿；响应异常同样失败关闭。
公开 PATCH 已送达后的网络断连存在不可消除的不确定性，重跑仍不覆盖已公开资产。

离线入口：`python scripts/ci/release.py check --tag v0.1.0-alpha.1 --sha <40位SHA>
--artifacts <四个artifact名目录的父目录> --output <不存在的新目录>`。
它只验证静态输入并写五个最终文件，不访问 GitHub、不执行包内程序。外部边界测试使用公开 Git/REST adapter
与本机 HTTP server，覆盖部分上传、草稿恢复、标签竞态、分页、损坏回读及已公开不可替换策略。
本机测试不是真实四平台 CI 或真实发布证明；本轮配置变更后的全树远端验证与最终实际交付仍须另行完成。
