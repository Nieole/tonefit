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
为后续同源打包提供原生依赖。它调用本地闸门，不定义业务行为、不替换解码器、
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
矩阵不 fail-fast，任务有超时；汇总守卫要求所有必需任务 success，失败、取消、跳过均不算成功。
既有外部素材缺失报告不等于素材实测；不新增跳测或 continue-on-error。

## 原生构建输入

`native.py` 的固定 URL 与 SHA256 是 dav1d 源码字节的权威输入；版本 1.5.4 满足
当前绑定要求的 >=1.3。校验失败或不安全归档直接拒绝，源码不由 Cargo.lock 覆盖。
每次全新目录构建静态库，禁用绑定的内部网络构建，并通过安装后的 pkg-config 元数据定位。
Meson/Ninja 固定版本；编译器、汇编器、pkg-config 等宿主工具版本、架构、runner image、
源码哈希、构建参数和提交记入 provenance。工具链设置不含 target-cpu=native。
Windows 使用 MSVC，Rust/C++/dav1d 统一静态 CRT；Linux 显式链接 UnRAR 需要的
C++ 标准运行库；macOS 部署目标 15.0。这些设置不是最终包依赖闭包或最低系统运行证明。

## 信任边界

默认 contents:read；checkout 不保留凭据；不使用 pull_request_target、写令牌或秘密运行 PR。
Action 引用完整提交 SHA。当前不保存 Cargo/native 缓存，PR 构建不向将来受信发行注入输出。
本模块当前仅提供质量验收和原生构建基础，不发布、不创建包或标签；后续交付扩展须保持
同提交全绿屏障及独立的受信构建/发布权限边界。批准的完整交付范围见 task-ci-delivery。
