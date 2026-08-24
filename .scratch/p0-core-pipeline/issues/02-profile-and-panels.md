# 02 — Profile 与面板表

**What to build:** 用 `--profile` 指定目标设备，目标尺寸由该设备的面板算出，不再写死。内置面板表覆盖 Kobo、BOOX、Kindle 主力型号，型号名到面板是多对一的别名表。`--gray-levels` 可覆盖面板灰阶数。

面板只含分辨率、PPI、灰阶数三项；阈值档位属于 profile，不属于面板。同一面板可以有多个 profile。

**Blocked by:** 01 — 骨架：单页贯通与合成夹具

**Status:** ready-for-agent

- [ ] `--profile` 接受型号名，经别名表解析到面板
- [ ] 目标尺寸 = fit-inside(面板分辨率)，随源页宽高比逐页算出
- [ ] 面板表用少量面板 + 型号别名表达，新增型号只需加一行别名
- [ ] `--gray-levels` 覆盖面板灰阶数
- [ ] 未知型号给出可操作的错误，并提示 `--gray-levels` 与内置型号清单
- [ ] `Report` 标明本次使用的 profile 与面板
