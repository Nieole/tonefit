# 01 — 骨架：单页贯通与合成夹具

**What to build:** 指向一个装着图片的目录，工具把每页 fit-inside 缩到目标尺寸、转成灰度、写出 8bit 灰度 PNG 到新目录，并给出一份逐页的报告。源目录保持只读。

这张票确立两个对外 seam：`run(Request) -> Report` 作为唯一入口，CLI 只负责把参数拼成 `Request`、把 `Report` 渲染成文字。目标尺寸此时可以写死，profile 在下一张票接入。

同时建立合成夹具生成器——夹具由代码生成，仓库里不放真实漫画素材。

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] `run(Request) -> Report` 是公开入口，CLI 之外的调用方无需启动子进程
- [ ] 目录源按阅读顺序吐出页
- [ ] AVIF、JPEG、PNG、BMP、WebP、GIF、TIFF 均可解码（AVIF 走 dav1d，见 measurements 的《AVIF 解码的可用路径》）
- [ ] 转灰用 OKLab 的 L 通道
- [ ] fit-inside 缩放，源比目标小时保持原尺寸
- [ ] 输出 8bit 灰度 PNG 到新位置，源目录未被修改
- [ ] `Report` 逐页给出输出路径与尺寸
- [ ] 夹具生成器能造出：连续渐变页、二值网点页、线稿页、纯色页、彩页、宽幅跨页、小于目标尺寸的页
- [ ] 测试全部使用生成的夹具，仓库内无图片素材
