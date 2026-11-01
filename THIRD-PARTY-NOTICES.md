# 第三方素材

tonefit 本体按 MIT 发布（见 `LICENSE`）。**随程序分发**而许可与本体不同的那几样单列在这里：
一份签在仓库里的字模，一份编进二进制的第三方源码。

## GNU Unifont

- **在哪**：`src/calibrate/glyphs.hex`
- **是什么**：灰阶测试图印判读说明用的点阵字模，摘自 GNU Unifont 16.0.01，只留这张图用得到的
  190 个码位。位图一格未改，格式保持上游 `.hex` 原样。
- **版权**：Copyright (C) 1998-2025 Roman Czyborra, Paul Hardy, Qianqian Fang, et al.
- **许可**：双许可 SIL Open Font License 1.1 与 GNU GPL 2+ with the GNU font embedding
  exception。**本仓按 OFL 1.1 使用。**
- **许可全文**：`licenses/OFL-1.1.txt`（逐字取自 <https://openfontlicense.org/documents/OFL.txt>）
- **上游**：<https://unifoundry.com/unifont/>

字模只在生成灰阶测试图时用到。灰阶测试图是量具——把它拷进阅读设备，目视数出面板的可见灰阶数，
并判断阅读器有没有把像素原样贴上。它不参与漫画页的处理。

## UnRAR

- **在哪**：那份 C++ 源码**不在仓库里**，随 `unrar-ng-sys` 这个依赖进来（见 `Cargo.toml`），
  由它把 UnRAR 的 C++ 源码**编进 tonefit 的二进制**。
  它上面那层 Rust 包装 `unrar-ng` 则**在仓库里**：`vendor/unrar-ng/`，与上游的
  0.7.7 只差一行（跨卷回调里一句越界读，为什么改、什么条件下撤掉见
  `vendor/unrar-ng/PATCH.md`）。那一层自身是 MIT OR Apache-2.0，两份全文跟着躺在
  那个目录里（`LICENSE-MIT`、`LICENSE-APACHE`）；**被它包进去的那份 C++ 源码不是**，
  约束是下面这一条。
- **是什么**：`.rar` 的解压端，随附的是 UnRAR 7.21.1（`unrar-ng-sys` 的
  `vendor/unrar/version.hpp`；**版本号这一处说了算**）。tonefit 只读 `.rar`、从不写——
  输出一律 `.cbz`（ADR 0015 决定第 2 条）。
- **版权**：RAR 与 UnRAR 的全部版权归作者 Alexander Roshal 独有（许可全文第 1 条）。
- **许可**：UnRAR license（见下面那条硬约束）。`unrar-ng` / `unrar-ng-sys` 这两层 Rust 包装
  自身是 MIT OR Apache-2.0，被包进去的那份 C++ 源码不是。
- **许可全文**：`licenses/UnRAR.txt`（逐字取自 `unrar-ng-sys` 随附的 `vendor/unrar/license.txt`）
- **上游**：<https://www.rarlab.com/rar_add.htm>

> **UnRAR 许可第 2 条是一条真实的分发约束，不是形式。**（英文原文见 `licenses/UnRAR.txt`，
> 这里只说它的意思，不作准。）它说的是：UnRAR 源码可以不受限制、免费地用在任何处理 RAR
> 归档的软件里，**但不得用于开发 RAR (WinRAR) 兼容的打包器、不得用于重建 RAR 的压缩算法**
> ——那是专有的；分发修改过的 UnRAR 源码时，该条全文必须出现在许可、文档或源码注释里。
>
> 它跟着二进制走：任何人分发 tonefit 的可执行文件，就一并分发了这份 UnRAR 源码编出来的东西，
> 因而要一并带上 `licenses/UnRAR.txt`。
>
> 这也是 ADR 0015 与 0014 分成两篇的理由，以及 `.rar` 不取纯 Rust 实现的理由：
> 一个从零写起的 rar 解码器要么就是那条禁止的事，要么许可说不清楚。

## dav1d（便携构建）

CI 便携包把 dav1d 静态编入程序；固定版本、来源与 SHA256 以 `scripts/ci/native.py` 为准。
原生库的许可不是 Rust 包装层的 MIT：dav1d 是 BSD-2-Clause，x86inc 汇编另有 ISC。
包内 `licenses/dav1d-BSD-2-Clause.txt` 与 `licenses/dav1d-x86-ISC.asm` 保留上游版权及完整条件，
包括静态链接时的通知义务。

## Rust 与 C++ 工具链（便携构建）

发行包生成 `dependency-licenses.json`：从目标传递依赖与 release 编译产物识别实际编译包，
保留各依赖及内嵌原生源码的许可/通知，不仅列 SPDX 名称。保守包含构建期依赖。
Rust 标准库及其不出现在 Cargo 图里的第三方代码见包内
`licenses/rust-stdlib/COPYRIGHT-library.html` 和配套 `licenses/`。

Linux 静态 libstdc++ 带有 GCC 版权、GPLv3 与 GCC Runtime Library Exception，全文随包提供；
运行库例外不把 tonefit 本体改成 GPL。Windows 的 Microsoft C++ STL 及 macOS libc++ 头文件代码
采用 Apache-2.0 WITH LLVM-exception；完整条款随 Rust 标准库的许可材料提供。
Microsoft STL copyright (c) Microsoft Corporation；libc++ copyright LLVM Project contributors。
Windows MSVC 静态 CRT 的 Microsoft 代码仍归 Microsoft Corporation 所有，按构建工具的
Visual Studio 可再分发代码条款链接分发，不适用 tonefit 的 MIT。
系统动态库归各操作系统，不从开发机器随意复制，也不声称由 tonefit 重新许可。
