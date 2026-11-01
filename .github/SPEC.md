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

## 原生构建输入

`native.py` 的固定 URL 与 SHA256 是 dav1d 源码字节的权威输入；版本 1.5.4 满足
当前绑定要求的 >=1.3。校验失败或不安全归档直接拒绝，源码不由 Cargo.lock 覆盖。
每次全新目录构建静态库，禁用绑定的内部网络构建，并通过安装后的 pkg-config 元数据定位。
Meson/Ninja 固定版本；编译器、汇编器、pkg-config 等宿主工具版本、架构、runner image、
源码哈希、构建参数和提交记入 provenance。工具链设置不含 target-cpu=native。
Windows 使用 MSVC，Rust/C++/dav1d 统一静态 CRT；Linux 显式链接 UnRAR 需要的
C++ 标准运行库（静态 libstdc++，随包保留 GCC 运行库许可与例外）；macOS 部署目标 15.0。
这些构建设置不能代替最终归档的依赖审计与运行验证。

## 信任边界

默认 contents:read；checkout 不保留凭据；不使用 pull_request_target、写令牌或秘密运行 PR。
Action 引用完整提交 SHA。当前不保存 Cargo/native 缓存，PR 构建不向将来受信发行注入输出。
本模块不发布、不创建标签；后续发布扩展须保持同提交全绿屏障及独立的受信构建/发布权限边界。
PR/分支包仅为临时验收资产，不得提升为受信发行输入。批准的完整交付范围见 task-ci-delivery。

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
链接、特殊文件与文件/目录冲突；检查清单哈希、必需许可、版本/目标来源、真实二进制架构与 Unix 执行位。
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
校验和证明字节完整性，不证明发行者身份。发行发布属于下一阶段。
