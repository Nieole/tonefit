# 第三方素材

tonefit 本体按 MIT 发布（见 `LICENSE`）。仓库里另有一份**随程序分发**的第三方素材，
许可与本体不同，单列在这里。

## GNU Unifont

- **在哪**：`src/calibrate/glyphs.hex`
- **是什么**：标定图印判读说明用的点阵字模，摘自 GNU Unifont 16.0.01，只留这张图用得到的
  190 个码位。位图一格未改，格式保持上游 `.hex` 原样。
- **版权**：Copyright (C) 1998-2025 Roman Czyborra, Paul Hardy, Qianqian Fang, et al.
- **许可**：双许可 SIL Open Font License 1.1 与 GNU GPL 2+ with the GNU font embedding
  exception。**本仓按 OFL 1.1 使用。**
- **许可全文**：`licenses/OFL-1.1.txt`（逐字取自 <https://openfontlicense.org/documents/OFL.txt>）
- **上游**：<https://unifoundry.com/unifont/>

字模只在生成标定图时用到。标定图是量具——把它拷进阅读设备，目视数出面板的感知可分辨级数，
并判断阅读器有没有把像素原样贴上。它不参与漫画页的处理。
