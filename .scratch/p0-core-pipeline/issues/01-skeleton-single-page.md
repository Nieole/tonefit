# 01 — 骨架：单页贯通与合成夹具

**What to build:** 指向一个装着图片的目录，工具把每页 fit-inside 缩到目标尺寸、转成灰度、写出 8bit 灰度 PNG 到新目录，并给出一份逐页的报告。源目录保持只读。

这张票确立两个对外 seam：`run(Request) -> Report` 作为唯一入口，CLI 只负责把参数拼成 `Request`、把 `Report` 渲染成文字。目标尺寸此时可以写死，profile 在下一张票接入。

同时建立合成夹具生成器——夹具由代码生成，仓库里不放真实漫画素材。

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] `run(Request) -> Report` 是公开入口，CLI 之外的调用方无需启动子进程
- [x] 目录源按阅读顺序吐出页
- [x] AVIF、JPEG、PNG、BMP、WebP、GIF、TIFF 均可解码（AVIF 走 dav1d，见 measurements 的《AVIF 解码的可用路径》）
- [x] 转灰用 OKLab 的 L 通道
- [x] fit-inside 缩放，源比目标小时保持原尺寸
- [x] 输出 8bit 灰度 PNG 到新位置，源目录未被修改
- [x] `Report` 逐页给出输出路径与尺寸
- [x] 夹具生成器能造出：连续渐变页、二值网点页、线稿页、纯色页、彩页、宽幅跨页、小于目标尺寸的页
- [x] 测试全部使用生成的夹具，仓库内无图片素材

## Comments

`run` 在 `src/lib.rs`，源、解码、转灰、缩放、编码各一个模块；夹具生成器在 `tests/fixtures/`。

本票内定下、但还没有 ADR 承接的两件事：

- **转灰取 OKLab 的 L 之后，按 sRGB 传输曲线编回 8bit**，不是把 L 直接当输出字节。
  这样消色像素恒等通过，参照与源只差一次缩放；直接用 L 会把灰度源整体提亮。
- **带 alpha 的源按纸白合成**，不是丢掉 alpha——透明区是纸，不是它底下的 RGB。

写死的与推迟的：面板分辨率 `PANEL_RESOLUTION`（Kobo Libra 2），02 号票接入 profile 后退场；
管线是单遍，ADR 0005 的两遍与缓存等 07 号票；编码器接口 P0 内无票承接（AVIF 输出不在范围内）。

同一卷里 `001.jpg` 与 `001.png` 会撞到同一个输出名，此时报错退出，不静默覆盖。
