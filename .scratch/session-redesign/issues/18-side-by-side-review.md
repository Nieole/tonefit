# 18 — 真终端并排验收

**What to build:** 快照管住了字、前景色与修饰，管不住终端主题下的实际配色、字体里字形的真实宽度、触控板的手感。
在 120×36 的真终端里跑新界面，与设计稿并排看，深色、浅色两种主题各一遍。走一遍：

- 11 个场景对应的状态：还没开始、清点中、转换中、等待确认、已结束、每页结果、整卷统一灰阶、配置、全部按键、添加路径、搜索；
- 主要交互：开跑、滚轮与触控板连续滚动、展开收起、进出每页结果、答话、配置选值与下钻、预设栏；
- 窗口缩到 80×24，再缩到窗口太小，再放大回来。

差异一律记停车场；要改的，照 ADR 0019 决定第 13 条先改设计稿、重新导出，再立票改实现。

**Blocked by:** 17 — 卡顿根因；`design-parity`（设计稿说了算）整批——停车场 Q206–Q994 拷问的结论，那三十来处屏上差异先改完再并排看

**Status:** ready-for-human

- [ ] 两种主题各过一遍，上面的清单逐项打勾写进落地记录
- [ ] 看到的每一处差异在停车场里都有一条
- [ ] 触控板连续滚动跟手，与 17 的实测数字对得上
- [ ] 在真终端上按 17 号那一节的施测条件补量前后（大库、触控板连续滚动），数字补进实测文档那一节（`docs/measurements.md` 的《会话卡顿：替代量法》；停车场 Q991）

## 停车场结转

下面这一条由停车场转来（`/settle` Q206–Q994）：并排验收时人就坐在 Windows 的真终端前，顺手在命令行那一趟上按一次 `Ctrl-C` 两级，结果记进落地记录。

#### Q263 — `Ctrl-C` 的 Windows 那一半本机验不到

- **From:** 票 `p4-parking-lot/18`
- **Kind:** 我确实拿不准的单项
- **Where:** `Cargo.toml` 的 `ctrlc`；`tests/stop.rs` 头上那句 `#![cfg(unix)]`
- **Why it did not block:** 本机只装了 `x86_64-unknown-linux-gnu` 一个 target
  （`rustup target list --installed`），Windows 那一半（`SetConsoleCtrlHandler`）**编都编不到**。
  `src/medium.rs` 那几处 FFI 是同一种处境，本仓早就认下过这笔。
- **What this ticket actually did:** **引 `ctrlc` 而不是自己写那两套 API**——
  选它的理由只有一处，`Cargo.toml` 里那一条的注释，这里不复述。落在本条上的是它的结果：
  本仓这一侧只有一句 `ctrlc::set_handler`，两个平台同一句，Windows 那一半的实现
  不在本仓里。按下去那一半的用例（`tests/stop.rs`）挂了 `#![cfg(unix)]`——
  往进程送一个 `SIGINT` 在 Windows 上不成立。
- **Whose call:** 拍板的人（Windows 那一半要不要真在一台 Windows 上按一次）
- **处置：** 待处理。
