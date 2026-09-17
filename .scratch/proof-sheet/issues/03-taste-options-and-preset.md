# 03 — 处理选项与 `--preset` 吃与转换同一套

**What to build:** 改完 `--filter` 或 `--white-align-limit`，出一叠样张就**立刻看得见它改了什么**。

「样张等于产物」这句话只在两边吃同一套选项时才成立，因此样张收下转换那条路的十项处理选项
与 `--preset`。两处读法要单说：

- **`--bit-depth` / `--dither` 不裁样张的候选集**（spec《Implementation Decisions》第三条）。
  它们裁掉的是「这一趟不要」，不是「这一页不可能」；而样张存在的理由正是并排看。
  照出整套，另外说出判定被顶死成了哪一档、理由是 `Override`。
  屏幕灰阶数与尺寸贴合检查那两道照裁——被它们裁掉的候选本来就永远不会被写出去。
- **与卷有关的那几项点到时说得出为什么**：`--dry-run` 与「出样张」自相矛盾，
  `--envelope` 要整卷，`--brief`／`--io-mode`／`--cache-budget`／`--no-metadata` 在一张图上无从谈起。

**Blocked by:** 02 — 一张普通页出一叠样张

**Status:** ready-for-agent

- [ ] 十项吃得下：`--fit`、`--no-crop`、`--no-split`、`--split-threshold`、`--reading-order`、`--filter`、`--white-align-limit`、`--gray-levels`、`--threshold`、`--preset`
- [ ] `--preset` 套得上，**命令行上显式点到的那一项赢**（转换那条路有现成的用例形状）
- [ ] `--bit-depth` / `--dither` 点到时候选集**照旧是整套**，判定那一格标着被顶死、理由是 `Override`
- [ ] 与卷有关的那六项点到时各说得出为什么，六句各有各的话、不是一句通用的
- [ ] 解析出的面板与转换那条路**是同一块**（`calibrate` 已有一条同样的用例）
- [ ] `--help` 说得出样张答的是哪一问、怎么在真机上读它（`calibrate` 的 help 有一条同样的用例）
- [ ] 神谕用例在**非默认选项**上再跑一遍：至少覆盖 `--fit inside` 与一个非默认 `--filter`，两边吃同一套，判定那一张照旧逐字节相同
- [ ] `cargo xtask gate` 三条全绿
