---
id: adr-0022
type: architecture-design
status: draft
title: ADR 0022：Windows 路径只在安全分量上合并扩展前缀
parent: tonefit-scope
references: [tonefit-glossary, adr-0014]
---

# Windows 路径只在安全分量上合并扩展前缀

用户已批准「补安全前缀等价」：《同一处》须认出常规绝对路径与不改变分量解释的 Windows 扩展前缀，才能让相对点名与 canonicalize 产生的写法收编为一份，守住 ADR 0014 的「一页不处理两遍」。修生产比较，不改原 discovery 回归的夹具或断言。

## 决定与边界

只在比较键中把安全的 `\\?\C:\…`、`\\?\UNC\server\share\…` 换成常规盘符／UNC 前缀，再用既有的大小写规则。盘符须为 ASCII 字母且紧跟 `:\`；UNC 须有非空服务器与共享名。用 Rust 前缀分类后检查完整原串，再迭代主体分量；不从已被迭代器吞掉特殊分量的结果反推安全。

扩展写法的每个分量（包括 UNC 服务器、共享名）须无歧义：不含空分量、`.`／`..`、正斜线、控制字符、`<>:"|?*`、尾点或尾空格。一个尾反斜线允许，重复分隔符不放行。DOS 保留名不合并：CON、PRN、AUX、NUL、COM1–9、LPT1–9（含 ¹²³），以及 CONIN$／CONOUT$；编号 0 也保守不合并（文档的 NT 命名空间示例包含 COM0 别名）。大小写、附扩展名或扩展名前的空格不能绕过检查。非法 UTF-8／未配对 surrogate 不猜测。通用 Verbatim、DeviceNS、卷 GUID、GLOBALROOT、相对盘符等其他命名空间不转换。

常规写法沿用原有字面规整（包括 `.`）；安全检查针对扩展写法，因为扩展命名空间会改变这些分量的解释。转换不增减键的分量级数：祖先、深度与 below 仍用同一把尺子。不做文件系统查询、canonicalize、软链解析、`..` 消解或新的 Unicode 折叠；报告与源标识仍保留原写法。

## 取舍

拒绝「一律去掉 `\\?\`」：会把设备、流或命名空间特殊名字错误收成一个。也不靠 canonicalize 比较：尚未写出的去处不存在，且会解析软链。保守子集可能漏掉本可合并的罕见写法，宁可保留区别，不猜测危险名字的等价。

依据：[Microsoft Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)（命名约定、全限定／相对路径、Win32 命名空间）、[Maximum Path Length Limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)（扩展 UNC 与正斜线／点分量的限制）与 [Console Handles](https://learn.microsoft.com/en-us/windows/console/console-handles)（CONIN$／CONOUT$）。实现与原生验证记录见 `.scratch/ci-delivery/windows-path-notes.md`。本 ADR 保持 draft，待用户审阅；已批准的词义不因此撤回。
