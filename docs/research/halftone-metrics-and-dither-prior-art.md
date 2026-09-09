# 半色调、低位深与抖动：外部现状调研

调研日期 **2026-09-08**。本文只记录**仓库之外**的现状与来源，不含本项目的决定，也不提任何建议。
项目自己的决定在 `docs/adr/`，项目自己的数字在 `docs/measurements.md`——本文引用它们的小节名，不复制。

## 怎么读这一篇

五节对应五个问题，每节第一句是现状判断，只有三档：

- **有成熟方案**——有现成实现或标准，拿得来用
- **有学术工作但未落地**——文献解决了，没有可用实现
- **基本空白**——找不到

每条实质结论后面跟来源。**凡是我没查到确证的，写在《没查到的》一节，不在正文里编圆。**
凡引用源码，给仓库、文件、行号与抓取时的 commit；行号会随上游漂移，函数名是稳定引用。

末节《直接可用的与只是背景的》是全文唯一一处把外部现状对到我们处境上的地方。

## 五个判断，一眼看完

| # | 问题 | 判断 |
|---|---|---|
| 一 | 半色调 / 二值 / 线稿的感知质量度量 | **有学术工作，半色调那一支相当成熟——但没有现成实现拿得来用**；线稿与漫画专用度量**基本空白** |
| 二 | 误差扩散在文字与线稿边缘的伪影 | **成因有闭式定量模型（1998 年）**；"按区域关掉抖动"**有在产实现，且有 35 年工程正统** |
| 三 | 自动选择位深 / 量化参数 | "按指标搜参数"与"自动定级数"**有成熟方案**；**我们那个具体问题（逐页选 {1,2,4}bit × {抖,不抖}）基本空白** |
| 四 | 电子墨水与漫画转换工具 | 面板侧**有第一方规格书**、显示层**成熟且写了理由**；**转换层基本空白**——但不是没人想清楚过，是想清楚了没留下 |
| 五 | 从共同根源出发的框架 | 框架成熟，**但对我们这种输入基本空白**——没人说它无效，是从未把这类输入放进标定域 |

**三条贯穿全文、值得单独记住的更正**（前两条在末节《一条需要单独标出的口径问题》展开）：

1. SSIMULACRA2 **不是**"没见过抖动"——它见过（TID2013 第 22 类），没见过的是**这种内容**。
2. CAMBI **不是**排除抖动——它**为抖动而设计**，却按构造看不见线稿。
3. KCC 的抖动 **不是**"一直是个意外"——**2013 年它是有判据的决策**，
   15 天后被一次三个词的提交无声翻掉，理由随之失传（见第四节）。

---

## 一、半色调、二值、线稿图像的感知质量度量

**判断：有学术工作，且在半色调这一支上相当成熟——但没有一个现成实现拿得来直接用。**

### 通用度量在这类图上失效，是印刷社区从一开始就知道的事

最直接的一手表述来自 Kite 的博士论文第 2 章摘要（Thomas David Kite, *Design and Quality
Assessment of Forward and Inverse Error Diffusion Halftoning Algorithms*, Ph.D. dissertation,
The University of Texas at Austin, August 1998，导师 Alan C. Bovik 与 Brian L. Evans，
<https://users.ece.utexas.edu/~bevans/students/phd/tom_kite/phd.pdf>）：

> The peak signal-to-noise ratio (PSNR) measure commonly used for image quality is inadequate
> for all but the simplest degradations.

同一篇给出了失效的**机理**，而不只是结论：误差扩散的残差与原图**相关**，因此任何基于噪声的度量
（SNR / PSNR / WSNR）直接对着原图算都是错的（论文 §2.4.1，以及 §3.2 的展开）。这一条与我们
`ADR 0002` 记下的"逐像素度量在抖动候选上给出与目视相反的排序"是同一件事的两种说法。

近期仍有论文以"通用 IQA 用不了"为出发点，说明这个缺口到 2025 年仍未合上：
Xinhong Zhang, Jiayin Zhao, Fan Zhang, *A halftone image quality assessment method based on
gradient and texture*, **Displays** 90:103165, 2025, DOI `10.1016/j.displa.2025.103165`。
（Crossref 核对了题名、作者、卷期；**正文在付费墙后，我没读到**，只用它证明该问题在 2025 年仍是活题。）

### SSIMULACRA2 的标定域里没有这类图

这一条值得单独说清楚，因为**流行的说法是错的**。

SSIMULACRA2 的官方 README（<https://github.com/cloudinary/ssimulacra2>）写明权重的调参数据：

> The weights were tuned based on a large set of subjective scores (CID22, TID2013, Kadid10k,
> KonFiG-IQA), including images compressed with JPEG, JPEG 2000, JPEG XL, WebP, AVIF, HEIC,
> and various artificial distortions.

**它并非"没见过抖动"**——TID2013 的第 22 号失真类型就叫 *Image color quantization with dither*
（<https://www.ponomarenko.info/tid2013.htm>），KADID-10k 的 25 类里也有 quantization
（Otsu 多级阈值）与 pixelate（<https://database.mmsp-kn.de/kadid-10k-database.html>）。

它没见过的是**这类内容**。TID2013 的 25 张参照图是从 Kodak Lossless True Color Image Suite
裁出来的自然照片。CID22 的构成写在论文里（*CID22: Large-Scale Subjective Quality Assessment
for Compressed Images*，Jon Sneyers, Elad Ben Baruch, Yaron Vaxman；我下载 PDF 后逐字读的），
§Dataset：

> All images are **512×512** pixels. **Most are cropped and downscaled high-resolution photos
> sourced from stock photography service Pexels.** Images are clustered into 15 categories:
> animals (11 images), art-abstract-decoration (16), building-monument (26), **diagram-chart (13)**,
> food-drinks (26), **illustration-logo-text (12)**, indoors-rooms (25), landscape-nature (23),
> materials-clothes (8), night-nightlife (18), people-fashion (18), portrait (10), sky-clouds (9),
> sports (17), and urban-industrial-cars (18).

**250 张里非照片的只有 25 张（diagram-chart 13 + illustration-logo-text 12，占 10%）**，
而且全是 512×512 的彩色图。**四个数据集里没有一张网点扫描页、没有一张 1–4 bit 的线稿。**

有意思的是，**CID22 的作者自己就观察到内容类别会改变结论**（同文 §Results by image category）：

> There are notable differences between categories: e.g. in the **non-photographic categories
> (diagram-chart and illustration-logo-text)**, AVIF outperforms other codecs, while for
> landscape-nature and materials-clothes, it does not perform well.

即"换一类内容，排序就变"这件事在这份数据集内部已经出现过；我们的内容比它那两类还远。

同一篇还顺带说明了另外两个数据集的偏向：*"in KADID10k and TID2013, **only 2 out of 25 distortion
types correspond to image compression**"*，而 CID22 *"covers only image compression and a specific
range of qualities (medium quality to near visually lossless)"*。

所以"失效"的准确说法不是"没标定过抖动"，而是**标定域是自然照片上的压缩失真，我们的输入
（高对比线稿 + 规则网点 + 极低位深）整个落在标定域之外**。README 也给了刻度的含义
（90 = "Distortion not noticeable by an average observer in a flicker test at 1:1"，
分数范围 "-inf..100"，负分 = "extremely low quality, very strong distortion"）——负分本身
不是异常，异常的是**排序**。

> **版本对齐**：我们记的 "SSIMULACRA2 0.5.1" 是 Rust crate `ssimulacra2` 的版本号
> （rust-av/ssimulacra2，0.5.1 发布于 2024-12-29），不是指标本身的版本号。
> 该 crate 自 **0.4.0**（2023-04-20）起实现指标的 **2.1** 版（CHANGELOG："Update to version 2.1
> of the metric"）。指标 2.1 相对 2.0 的改动包括"weights retuned"与"added a polynomial
> remapping of the error score to allow a better fit to datasets with higher distortions"。

### 半色调社区实际用的：CSF 加权的低通度量

这一支很成熟，而且**结论与 `ADR 0002` 的低通决定同向**。

核心量是 **WSNR**（weighted signal-to-noise ratio）。定义见 Kite 论文 §2.3：

> In the same way that SNR is defined as the ratio of average signal power to average noise
> power, WSNR is defined as the ratio of average weighted signal power to average weighted
> noise power, where the weighting is derived from the CSF.

论文把它类比成音频的 A 计权。所用的 CSF 是 **Mannos–Sakrison** 的径向模型
（J. Mannos, D. Sakrison, *The effects of a visual fidelity criterion on the encoding of images*,
IEEE Trans. Information Theory 20:525–536, July 1974）加 **Sullivan** 的角向依赖，再叠一道
**Mitsa–Varkur** 的修正——把低频段压平，使 CSF 从带通变成**低通**
（T. Mitsa, K. Varkur, *Evaluation of contrast sensitivity functions for the formulation of
quality measures incorporated in halftoning algorithms*, Proc. IEEE ICASSP 1993, vol. 5,
pp. 301–304）。Kite 论文转述该文的结论：

> In [16] it was reported that predictions of halftone quality using the lowpass CSF presented
> in Section 2.2 correlated well with psychovisual measurements.

同一支的早期综述：T. Mitsa, *Image quality metrics for halftone images*, Proc. SPIE 1778,
Imaging Technologies and Applications, pp. 196–207, Mar. 1992。

**关键结构**：CSF 是**角频率**的函数，所以论文必须把图像尺寸与**观看距离**折算成 cycles/degree
才能用（Kite §2.2，含折算公式与随观看距离变化的 WSNR 表）。这与我们"低通核由面板 PPI 推出"
是同一个做法、同一个理由。

### HPSNR / MPSNR：就是"先低通再比"

"用低通后的图去比"这件事在文献里有独立的名字。综述
Mei Li, Erhu Zhang, Yutong Wang, Jinghong Duan, Cuining Jing, *Inverse Halftoning Methods
Based on Deep Learning and Their Evaluation Metrics: A Review*, **Applied Sciences** 10(4):1521,
2020, DOI `10.3390/app10041521`（Crossref 核对作者与卷期）记述：PSNR 对半色调图不是好度量，
因为好的半色调图并不因此得高 PSNR；替代物 **MPSNR** 即"原多调图与半色调图的**低通版本**之间的
PSNR"，且**读数取决于所用的低通滤波器**。

> **口径警告**：这段转述来自该综述，**综述正文我只拿到检索摘要，MDPI 与 ScienceDirect 都
> 403 拒绝抓取**。MPSNR / HPSNR 的**原始出处我没有追到**，见《没查到的》。
> 上面 Kite 与 Mitsa 两条是我直接读过原文的，那两条可以当准。

另一条独立的一手线索：Patrick Itoua, Azeddine Beghdadi, Patrick Viaris de Lesegno,
*Objective perceptual evaluation of halftoning using image quality metrics*, Proc. ISSPA 2010,
pp. 456–459, DOI `10.1109/ISSPA.2010.5605446`（Crossref 核对）——把通用 IQA 指标改造到半色调图上，
做法同样是先过一道模拟 HVS 光学低通的高斯滤波。

再一条：Xuemei Zhang, D. A. Silverstein, J. E. Farrell, B. A. Wandell, *Color image quality
metric S-CIELAB and its application on halftone texture visibility*, Proc. IEEE COMPCON 97,
pp. 44–48, DOI `10.1109/CMPCON.1997.584669`。S-CIELAB 是"先按 CSF 做空间滤波、再算色差"，
本身就是"低通再比"的彩色版，且**明确用在半色调纹理可见度上**。

### 相邻的两支：屏幕内容与文档图像

**屏幕内容（Screen Content IQA）这一支是成立的，而且它的动机与我们一样。**
Huan Yang, Yuming Fang, Weisi Lin, *Perceptual Quality Assessment of Screen Content Images*,
IEEE Trans. Image Processing 24(11):4408–4421, Nov. 2015, DOI `10.1109/TIP.2015.2465145`。
配套的 SIQAD 数据库（20 张参照 + 980 张失真图）是该方向的基准。**方法上的要点**：它把
**文字区与图像区分开加权**，因为两类区域的失真在主观上不等价。这正是我们那三类劣化里
第 3 类（文字与线稿边缘）与第 1 类（平滑区颗粒）不能用同一把尺子量的那件事，在别的领域被独立发现过。

> **别把这一点推广到整个屏幕内容 IQA**：四个主要指标里**只有 SPQA（就是上面这篇自己的方法）
> 真的做文字/图像分区**。SQMS 是显著性＋梯度加权（Gu et al., IEEE TMM 18(6):1098–1110, 2016,
> DOI `10.1109/TMM.2016.2547343`）、ESIM 是边缘对比/宽度/方向（Ni et al., IEEE TIP
> 26(10):4818–4831, 2017）、GFM 是 Gabor 边缘特征（Ni et al., IEEE TIP 27(9):4516–4528, 2018）。

**这一支还提供了"通用度量建立在自然图像统计上"这句话的一手、开放获取出处**——
Shiqi Wang, Ke Gu, Xiaofeng Zhang, Weisi Lin, Li Zhang, Siwei Ma, Wen Gao,
*Subjective and Objective Quality Assessment of Compressed Screen Content Images*,
IEEE Journal on Emerging and Selected Topics in Circuits and Systems 6(4):532–543, 2016,
DOI `10.1109/JETCAS.2016.2598756`，逐字：

> traditional IQA algorithms are usually devised **relying on the statistics of natural images**.

> The correlations between subjective and objective scores also suggest that **directly employing
> the IQA models developed for natural images may not suffice** for the purpose of trusted SCI IQA.

> most of the popular FR methods such as the structural similarity (SSIM) index and its variants,
> visual signal-to-noise ratio (VSNR), gradient similarity (GSIM) and visual saliency-induced index
> (VSI) **are designed based on the validations using natural images.**

该方向的综述：Xiongkuo Min et al., *Screen Content Quality Assessment: Overview, Benchmark, and
Beyond*, ACM Computing Surveys 54(9):1–36, 2021, DOI `10.1145/3470970`——
*"conventional quality assessment methods **can not handle such content effectively**"*。

> **口径**：SIQAD 本身是闭放获取，**子 agent 没能读到正文**，因此上面那句"通用度量不够用"的
> 逐字出处取自 Wang 2016 与 Min 2021，**不是** SIQAD 那篇。

**文档图像（DIQA）这一支存在，但目标不是我们的目标**：它基本上是为 OCR 服务的
（"这张扫描件识别得出来吗"），不是"这张页看着舒不舒服"。**我没有把这一支追到一手论文**，见《没查到的》。

### 线稿 / 漫画本身：基本空白，而且这一行有当事人签字画押

**这一节原先只有弱证据，补查之后变成全文最硬的"空白"结论之一。**

**图形学社区自己在论文里承认没有可用度量。**
Chuan Yan, David Vanderhaeghe, Yotam Gingold, *A Benchmark for Rough Sketch Cleanup*,
ACM Trans. Graphics 39(6), Article 163, SIGGRAPH Asia 2020, DOI `10.1145/3414685.3417784`
（<https://cragl.cs.gmu.edu/sketchbench/>）。他们评估了各种距离之后，§5.1–5.2 逐字：

> The IOU is **extremely sensitive to local misalignments**

> The Hausdorff distance is **dominated by the behavior of outliers** … This makes it a
> **poor choice** for us.

> The Chamfer distance had the best Pearson correlation score with the other distances.
> Due to its good theoretical properties and **for lack of a better alternative**, we focus on
> the Chamfer distance for our evaluation.

**"for lack of a better alternative"——这是线稿领域在正式论文里签字承认没有感知度量。**
他们的评估里**没有 SSIM、没有 PSNR、没有 LPIPS**。

**素描领域有一篇直接点名 SSIM 失效的**：Deng-Ping Fan et al., *Scoot: A Perceptual Metric for
Facial Sketches*, ICCV 2019, pp. 5611–5621, DOI `10.1109/ICCV.2019.00571`（arXiv:1908.08433），
摘要逐字：*"existing two widely-used facial sketch metrics, e.g., **FSIM and SSIM fail to address
this perceptual similarity** in this field"*。**限定**：它是**人脸**素描、评的是素描**合成**，
不是压缩保真度。

**二维动画领域做了用户研究，结论同向**：Shuhong Chen, Matthias Zwicker, *Improving the
Perceptual Quality of 2D Animation Interpolation*, ECCV 2022, DOI `10.1007/978-3-031-19790-1_17`
（arXiv:2111.12792），摘要逐字：*"we **establish that the LPIPS perceptual metric and chamfer
line distance (CD) are more appropriate measures of quality than PSNR and SSIM** used in prior art."*

### 网点：两组人各自独立得出"逐像素度量要先换个空间才有意义"

**这是本节对我们最直接的一条。**同一个实验室（CUHK，Tien-Tsin Wong 组）的两篇论文，
从两个不同任务出发，都撞到"网点上逐像素比不了"这堵墙，并各自绕开：

- Minshan Xie, Menghan Xia, Tien-Tsin Wong, *Exploiting Aliasing for Manga Restoration*,
  CVPR 2021（arXiv:2105.06830）逐字：
  > we measure the screentone difference in the domain of **ScreenVAE map** … that represents the
  > screentone pattern as a smooth and interpolatable 4D vector and **enables the pixelwise
  > metrics (e.g. MSE) to be effective.**

  即：**先把网点投影到一个学出来的潜空间，逐像素度量才开始有意义。**
- 同组的网点保持缩放工作（*Screentone-Preserved Manga Retargeting*, Computer Graphics Forum,
  2025，DOI `10.1111/cgf.70096`）走的是**最佳位移匹配**的路子。

**两条路子的共同点是"不要在像素栅格上原位对齐着比"。**
（这两篇的逐字引文由子 agent 读出，**我未复核**。）

### 强化过的否定结论

子 agent 用与我不同的方法独立复查，结论一致：

- **arXiv 全文检索** `"cartoon image quality assessment"` / `"manga quality assessment"` /
  `"comic image quality assessment"` → **零相关命中**；**Crossref 里不存在任何卡通 IQA 数据集**。
  **没有卡通 / 动画 / 漫画专用的 IQA 指标，没有对应数据集。**
- **VMAF 没有内容类型这一维**：`resource/doc/models_v1.md` 对
  `anim|cartoon|synthetic|graphic|content type|genre|text` **零命中**——v1 按帧率与分辨率分，
  不按内容类别分。
- **`cloudinary/ssimulacra2` 与 `cloudinary/ssimulacra` 两个仓库的全部 31 个 issue 被逐条枚举读过，
  没有一条报告该指标在非照片 / 合成 / 线稿 / 屏幕内容 / 抖动内容上失灵。**
  如果有人撞上过我们这个问题并上报，**不在那里**。
- **HEVC 屏幕内容编码（SCC）的标准化记录里，找不到任何一句说客观指标对屏幕内容不够用**——
  官方 overview 与 JCT-VC 验证测试都只报 PSNR BD-rate，且称 MOS 与 PSNR 一致。

### 一个必须知道的反面案例：二值有损编码可以骗过所有像素度量

**Xerox 的 JBIG2 换数字事件**（David Kriesel，2013-08-02 公布；BBC News 2013-08-06 报道）：
WorkCentre 系列扫描仪的 JBIG2 有损符号编码**把字典里"长得像"的字符互换**，
输出*"look correct at first glance, even though numbers may actually be incorrect"*。

**这是二值有损编码在任何平滑的逐像素度量上都能拿接近满分、而语义已经错了的教科书案例。
没有任何 IQA 指标抓得到它。**这与 `ADR 0002` 那条"判据只答颗粒有多显眼，不答颗粒长什么形状"
是同一类盲区的极端版本。

---

## 二、误差扩散在文字与线稿边缘的伪影

**判断：成因有精确的定量模型，1998 年就有；"按区域关掉抖动"有成熟且在产的工程实现。**

这一节是全篇最直接对得上我们第 3 类劣化的地方。

### 成因：误差扩散在高频上把信号放大 4 倍

**这不是玄学，是一个可算的传递函数。**

Kite / Evans / Bovik 把量化器建模成"线性增益 + 加性噪声"，由此把误差扩散的两个效应
**解耦**开来。一手论文：T. D. Kite, B. L. Evans, A. C. Bovik, *Modeling and quality assessment
of halftoning by error diffusion*, IEEE Trans. Image Processing 9(5):909–922, May 2000,
DOI `10.1109/83.841536`。摘要给出结论的形状：

> the two primary effects of error diffusion: **edge sharpening and noise shaping** …
> Edge sharpening is proportional to the linear gain, and [we] provide a formula to estimate
> the gain from a given error filter.

论文（Kite 博士论文 §3.2–3.3，同一工作的完整版）给出的数字：

- **Floyd–Steinberg 的量化器信号增益 `Ks = 2.00`**；Jarvis 等的滤波器 `Ks = 4.37`（图 3.8）。
- **信号传递函数在直流处增益为 1，在高频处升到 4**（FS）／**9**（Jarvis）：

  > Both have unity gain at DC; the gain rises at high frequency to 4 for the Floyd-Steinberg
  > STF and 9 for the Jarvis STF. This qualitatively explains the image sharpening inherent
  > to error diffusion.

**这两句话精确解释了为什么低通项看不见毛刺**：误差扩散在**直流处增益恰好是 1**——而低通项量的
就是直流那一端。锐化整个落在高频那一端，低通项按定义把它抹掉了。参见 `ADR 0002` 记下的
"低通是对的，只是矫枉过正"。

模型的两条通路（`H(z)` 是误差滤波器）：

```
STF = Ks / (1 + (Ks - 1) H(z))     信号通路：高频提升 → 边缘锐化
NTF = 1 - H(z)                     噪声通路：高通 → 噪声整形，与 Ks 无关
```

Evans 本人 2003 年 SPIE/IS&T 讲义里给出的**逐图实测 `Ks`**（子 agent 从讲义 PDF 读出）：

| 图像 | Floyd–Steinberg | Stucki | Jarvis |
|---|---|---|---|
| barbara | 2.01 | 3.62 | 3.76 |
| boats | 1.98 | 4.28 | 4.93 |
| lena | 2.09 | 4.49 | 5.32 |
| mandrill | 2.03 | 3.38 | 3.45 |
| **平均** | **2.03** | 3.94 | 4.37 |

讲义并注明 FS 的 `Ks`「Does not vary much」——**它基本是滤波器的常数，不怎么跟着图变**。
（与我从博士论文里读到的 `Ks = 2.00` 一致。）

**机理那一层的一手出处**是 Knox 的"误差图像"系列：Keith T. Knox, *Error image in error
diffusion*, Proc. SPIE 1657, Image Processing Algorithms and Techniques III, 1992,
DOI `10.1117/12.58334`——**误差图像里含有输入图像的线性分量**，正是它在输出里诱发边缘增强。
Evans 讲义把它压成一句："Sharpening proportional to correlation between error image and input
image [Knox, 1992]"。

Ostromoukhov 论文里的独立表述：
*"the effect of edge enhancement of E-D is **very close to the effect of a simple Laplacian or a
similar sharpening filter**."*

论文还给出了**为什么**会锐化（§3.4，作者自述"the means by which sharpening occurs has not
been addressed before"）：锐化源于**量化误差与输入的相关性**，而且——

> if the quantization error is decorrelated using dither, sharpening will disappear.

即：**在量化器阈值上加抖动（threshold modulation），可以让边缘锐化消失。**

### 已知伪影清单

- **蠕虫 / 蛇形图案**与**极限环**（limit cycles）：Kite 论文 §3.1 图 3.1 专门画了误差扩散的极限环。
  更早的系统记述在 Robert Ulichney, *Digital Halftoning*, MIT Press, 1987,
  DOI `10.7551/mitpress/2421.001.0001`（该书是这一领域的奠基参考，蓝噪声判据也出自它）。
- **边缘增强 / 回响**：见上。
- **启动瞬态与扫描方向伪影**：Kite 论文 §3.5 用阶跃响应测量（图 3.14–3.17，含 serpentine
  扫描下的水平阶跃响应）。

> **未核**：Floyd 与 Steinberg 的 1976 年原文（*An adaptive algorithm for spatial greyscale*,
> Proc. SID 17(2):75–77）**不在 Crossref 里**（早于 DOI 制度），我只在二手引用里见过它的著录。
> 著录形式我按学界通行写法记在这里，**未经一手核对**。

### 解法一：改误差滤波器

Victor Ostromoukhov, *A Simple and Efficient Error-Diffusion Algorithm*, Proc. ACM SIGGRAPH 2001,
pp. 567–572, DOI `10.1145/383259.383326`
（作者自存 PDF：<https://perso.liris.cnrs.fr/victor.ostromoukhov/publications/pdf/SIGGRAPH01_varcoeffED.pdf>）。
做法是**变系数**：对每一个输入灰阶单独离线优化一组分配系数，使该灰阶的傅里叶谱尽量贴近蓝噪声谱，
再在灰阶之间平滑插值。**它治的是平坦调子上的结构性伪影，不是边缘锐化。**

### 解法二：蓝噪声掩模

Robert Ulichney, *Void-and-cluster method for dither array generation*, Proc. SPIE 1913,
Human Vision, Visual Processing, and Digital Display IV, pp. 332–343, 1993,
DOI `10.1117/12.152707`。

**蓝噪声在边缘处比误差扩散好还是差？两篇一手论文口径一致：不出乱子，但更糊。**

- Ostromoukhov（SIGGRAPH 2001）逐字：
  > the output of either 'void-and-cluster' or 'blue noise mask' techniques appear **a bit blurry
  > when compared with that produced with a typical E-D technique**. This is due to its inherent
  > edge enhancement.

  他并指出这两类掩模的另外两个缺点：*"the matrices are not publically available"*，以及周期性平铺。
- Pang et al.（SIGGRAPH 2008）逐字：
  > Although halftoning with blue noise properties can suppress many of the annoying patterns,
  > it may at the same time **over-blur the fine texture details** in the original images.

即：**误差扩散更锐但会出乱子（毛刺）；蓝噪声掩模不出乱子但更糊。**
掩模是无状态的逐像素阈值比较，因此没有跨边界的误差传播、没有扫描方向性、没有启动瞬态。
**这是一组权衡，不是一个更优解。**

### 解法三：结构感知的半色调

两篇，作者与年份都经 Crossref 核对：

- Wai-Man Pang, Yingge Qu, Tien-Tsin Wong, Daniel Cohen-Or, Pheng-Ann Heng,
  *Structure-aware halftoning*, ACM Trans. Graphics 27(3), SIGGRAPH 2008, pp. 1–8,
  DOI `10.1145/1360612.1360688`。
- Jianghao Chang, Benoît Alain, Victor Ostromoukhov, *Structure-aware error diffusion*,
  ACM Trans. Graphics 28(5), SIGGRAPH Asia 2009, pp. 1–8, DOI `10.1145/1618452.1618508`。

> 值得记一笔：Tien-Tsin Wong 那一组同时是**漫画网点**研究的主力。结构保持与网点是同一批人做的。

### 解法四：阈值调制 / 显式控制边缘增强

- Reiner Eschbach, Keith T. Knox, *Error-diffusion algorithm with edge enhancement*,
  J. Opt. Soc. Am. A 8(12):1844, 1991, DOI `10.1364/josaa.8.001844`。
- Keith T. Knox, *Symmetric edge enhancement in error diffusion*, Proc. SPIE 3963,
  pp. 536–542, 1999, DOI `10.1117/12.373435`。

这一支是把边缘增强变成**可调参数**（可以调大，也可以调到零），与 Kite 的"用 dither 解相关即可
消除锐化"是同一个旋钮的两种拧法。

**其中一条理论结果对我们特别有用**——Knox & Eschbach, *Threshold modulation in error
diffusion*, Journal of Electronic Imaging 2(3):185–192, 1993, DOI `10.1117/12.148736`：
**阈值的空间调制在数学上等价于**对"原图 ＋ 阈值调制信号的高通滤波版"做**标准**误差扩散。

即：**边缘增强的控制 ≡ 预锐化 / 预钝化。**要在边缘处削弱抖动，不必改扩散核，
等价地在边缘处反向预处理输入即可。

**但全局的那一个系数不够用**，两篇独立论文点同一个问题：

- Chang / Alain / Ostromoukhov（SIGGRAPH Asia 2009）逐字：
  > even these improved versions of edge-enhancement contain a major drawback: **the strength of
  > the enhancement is controlled by a unique global coefficient**. Consequently, the method is
  > **sensitive to only one particular sub-range of frequencies and contrasts**. In complex images
  > with mixed frequency/contrast content this method can fail.

  并补充：有重复结构时，边缘增强会产生 **Moiré 状伪影**。
- Pang et al.（SIGGRAPH 2008）逐字：*"since the modulation is **applied uniformly over the whole
  image**, low-frequency regions are affected as well."*

**含义：这条旋钮必须做成局部的才有用**——这正好接到下一段。

### 解法五：按区域关掉抖动——**这一条有成熟且在产的实现**

这是本次调研在工程侧最实质的发现。**pngquant / libimagequant 就是这么做的，而且把理由写在了源码注释里。**

仓库 `ImageOptim/libimagequant`，抓取时 `main` HEAD = `9388d26902c53854c4f666dc3f07db6504b16924`
（2026-09-08）。`src/remap.rs:147` 的函数文档注释，逐字：

```rust
/// Uses edge/noise map to apply dithering only to flat areas. Dithering on edges creates
/// jagged lines, and noisy areas are "naturally" dithered.
```

**"Dithering on edges creates jagged lines"——这就是我们第 3 类劣化，一个在产工具把它写在注释里。**

机制（同一仓库）：

- `src/image.rs:241` `contrast_maps()` 求两张图，注释在 `:239-240`：
  `importance_map` 是"approximation of areas with high-frequency noise, except straight edges.
  1=flat, 0=noisy"，`edges` 是"noise map including all edges"。边缘检测是二阶差分
  （`:276` 的 `prev + next - curr * 2.`），随后 `:294-299` 用 max3 / min3 / blur 做形态学
  收缩再膨胀，"noise areas are shrunk and then expanded to remove thin edges from the map"。
- `src/remap.rs:160` 取 `dither_map`，缺省回落到 `edges`；`:179-180` 归一化；
  `:260-262` 逐像素 `dither_level *= f32::from(l)`——**抖动强度是逐像素连续加权的，不是开关**。
- `src/remap.rs:112-113` 另有一道**可见度地板**：
  ```rust
  if dither_error < 2. / 256. / 256. {
      // don't dither areas that don't have noticeable error — makes file smaller
  ```
  即"误差小到看不见就不抖，顺带省体积"。
- `src/remap.rs:136` 对超出 `max_dither_error` 的误差乘 0.75 衰减，防止极端误差扩散出去。

**2026-09-08 复读源码补两条精度**（`main` HEAD 同上，逐行核过）：

- **那张边缘图就是二阶差分，没有别的**：`image.rs` 里
  `horiz = |prev + next - 2*curr|`、`vert = |above + below - 2*curr|`（各通道取 max），
  `edge = max(horiz, vert)`，写出去的是 `edges[i] = (1 - edge) * 256`——
  **255 = 平坦 = 全抖，0 = 强边 = 不抖**。随后 `min3` → `max3`（形态学开运算）去掉细边，
  最后 `edges = min(noise, edges)` 与噪声图取交。**没有 Sobel，没有聚类，没有阈值参数。**
- **乘的是"喂进量化器的那个误差"，不是输出**：`dither_row` 里
  `dither_level = base * edges[col]`，再由 `get_dithered_pixel()` 算
  `s = thiserr * dither_level`，用 `px + s` 去调色板里找最近邻。
  **即：边缘处照常量化、照常向外扩散误差，只是不把邻域传来的误差**加进来**。**
  与上面 Kiyotomo 的"边缘处排除累积误差"是同一个动作的连续版（他归零，它按边缘强度衰减）。

学术侧的对应物（只在检出 banding 处抖）：Sitaram Bhagavathy, Joan Llach, Jiefu Zhai,
*Multi-Scale Probabilistic Dithering for Suppressing Banding Artifacts in Digital Images*,
Proc. IEEE ICIP 2007, pp. IV-397–IV-400, DOI `10.1109/ICIP.2007.4380038`。

### 多级误差扩散（multitoning）是另一支文献，而且它有一个专属伪影

**这一条是本次调研里最容易被漏掉、又最直接打中我们候选集的发现。**

上面引的绝大多数半色调文献讲的是**二值**半色调（1bit）。而我们的候选集里 2bit+FS 与 4bit+FS
是**多级**误差扩散——把 FS 的二值量化器换成 N 级量化器。**这在文献里是另一个名字（multitoning
/ multilevel halftoning），而且它有一个二值半色调不存在的伪影。**

一手来源：Faraz Faheem, Gonzalo R. Arce, Daniel L. Lau, *Digital Multitoning Using Gray Level
Separation*, Journal of Imaging Science and Technology 46(5):385–397, 2002,
DOI `10.2352/j.imagingsci.technol.2002.46.5.art00001`（Crossref 核对题名、作者、卷期页）。逐字：

> An early example of multitoning is the **Floyd and Steinberg error diffusion algorithm with an
> *N*-level quantizer replacing the conventional binary quantizer**. A major problem associated
> with this approach is the introduction of **unwanted texture near the intermediate gray levels**.

> …at the intermediate gray level there is **no quantization error** … In the neighborhood of
> this zero error region, there is a sparse distribution of the black and white pixels, in an
> otherwise constant gray region. This appears as a **banding artifact** to the human observer.

**机理**：恰好落在某个可用格点上的平坦区**不产生任何量化误差**，因此完全不抖；紧邻它的灰调却有
稀疏的黑白点。两者并置，看起来就是一圈圈的 band。**位深越高、格点越密，这种"零误差带"越多。**

> 与 `ADR 0002` 记下的那条代价对照着读：那里推导出"平坦灰调上抖动恒优于不抖动"要求
> 可见度地板 > `0.2071·s`，推导的前提正是与最近格点差 `u` 且 `0 < u < s`。
> **`u = 0` 那一点（恰好落在格点上）是推导的边界，而 Faheem 说的正是这一点及其邻域。**
>
> **2026-09-08 补：判读这一级已经确认两者是同一件事。**武器 v01 原档的纸白是 253，
> 距 2bit 的格点 255 只差 2——正是 Faheem 说的"格点邻域"。FS 于是把 **2.4% 的像素落到 170**，
> 平坦白底上出现稀疏点；五位判读员在四页上**全一致判「明显影响观感」、零接受**，
> 而判据读数全在阈值之内（3.25~3.54）。原因是颗粒项用 **RMS**：
> `sqrt(0.976×0.024)×85 = 12.88`，低于地板 `0.216×85 = 18.36`，恒收 0。
> **RMS 把"稀疏而响"读成了"安静"。**数字见 `docs/measurements.md` 的
> 《第五轮：十二页五判读员，原档 · 判据在文字白底页上整齐地漏掉》，处置见 `.scratch/非阻塞问题.md` 的 Q434。
> **真机那一级仍未做。**

该文的解法叫"灰阶分离"：按灰阶变换曲线把输入图拆成若干可打印的灰度分量，各分量用
**常规二值误差扩散**处理（彼此相关地），再合起来。

> **复核口径**：题录（题名、三位作者、卷 46 期 5 页 385–397、2002）与**现象本身**，
> 我在 IS&T 官方文章页（<https://library.imaging.org/jist/articles/46/5/art00001>）上
> 独立核过——官方摘要逐字有 *"the introduction of **unwanted texture near the intermediate
> gray levels** in the printed image"* 与 *"elimination of the undesirable **banding artifacts**"*。
> **上面那两段带缩进的逐字引文出自正文，由子 agent 从 PDF 读出，我没有拿到正文复核。**
> 现象与机理可信，逐字措辞请引用前再核。

同一支的后续与参考书（均经 Crossref 核对）：

- J. Bacca Rodriguez, G. R. Arce, D. L. Lau, *Blue-Noise Multitone Dithering*, IEEE Trans. Image
  Processing 17(8):1368–1382, 2008, DOI `10.1109/tip.2008.926145`。
- J. Bacca Rodriguez, G. R. Arce, D. L. Lau, *A New Method for Digital Multitoning using Gray
  Level Separation*, Proc. IEEE ICIP 2006, pp. 1505–1508, DOI `10.1109/icip.2006.312568`。
- Daniel L. Lau, Gonzalo R. Arce, *Modern Digital Halftoning*, 2nd ed., CRC Press,
  DOI `10.1201/9781315219790`（Crossref 记的年份是 2018 的重印本；通行著录写 2008，
  **两个年份我没有对齐**）。

### 多级 ＋ 边缘保持：唯一一篇正对我们候选档的论文，但它治的不是我们的病

**这一篇原先列在《没查到的》里，现已取到全文并通读**（IS&T 开放下载：
<https://library.imaging.org/admin/apis/public/api/ist/website/downloadArticle/ei/29/18/art00017>）。

Takuma Kiyotomo, Keisuke Hoshino, Yuki Tsukano, Hiroki Kibushi, Takahiko Horiuchi,
*Edge-Preserving Error Diffusion for Multi-Toning Based on Dual Quantization*,
Proc. IS&T Electronic Imaging 2017, Color Imaging XXII, pp. 123–129,
DOI `10.2352/ISSN.2470-1173.2017.18.COLOR-044`。千叶大学 ＋ 東京機械製作所。

**它是文献里唯一一篇同时具备"多级"与"边缘保持"的工作，而且它的实验档位就是 2bit（四级）。**

**但它要治的病不是我们的病。**问题陈述逐字：

> During the actual printing process, many kinds of noise (e.g., **ink spread and ink bleed**)
> are added to the image… **Black letters on a white background appear enlarged, whereas white
> letters on a black background appear reduced in size and faded.**

**这是印刷的 dot gain，是墨在纸上洇开。电子墨水没有这一项。**
所以它那套里**为补偿 dot gain 而设的部分对我们没有依据**——特别是"偏置阈值"的**方向**：

> `QE` assigns a **wider range of high-concentration values** than dark-concentration values…
> a pixel has a **high probability of constantly being assigned a bright value at the edge**.
> **Users have to design this biased quantization function based on attributes of their printer.**

**它把边缘往亮里推，理由是墨会洇黑——这个理由在 e-ink 上不存在。**

**能拿走的是它的另外两件事**，两件都与 dot gain 无关：

1. **在边缘像素上"排除累积误差"**（Dual Quantization 的第一半）：边缘处不吃邻域扩散进来的误差，
   `Y = Q_E(X)` 而非 `Q(X + E)`；但**误差仍按 `e = X + E − Q(X + E)` 计算并向外扩散**（式 9），
   作者明说这是为了"retain features of the ED mask"，否则"The edge exclusion process results in
   **loss of characteristics of error diffusion masks**"。
   **这与 libimagequant 的"逐像素衰减 dither_level"是同一根杠杆的两种极端拧法**：
   libimagequant 连续衰减**输入端**的误差，Kiyotomo 在边缘处**归零**输入端而保留输出端。
2. **边缘要筛，不能直接用**（Edge Selection）。作者自陈 Sobel *"often generated pseudo-edges…
   and they caused **extensive artefacts** on the multi-toned result"*。
   筛法：连通域聚类 → 丢掉元素数太少的簇 → 按**方差图（VM）**与**邻域黑像素数（NBPM）**的簇内均值
   做阈值。**这一条与本节末尾"真正的难点在交界处"是同一件事的另一个证词。**

**代价（论文 Table 2，2048×2560 自然图 / 2339×1654 字符图，i5-4590 单线程 C）**：

| 方法 | 自然图 | 字符图 |
|---|---|---|
| Floyd–Steinberg（二值） | 0.67 s | 0.48 s |
| Shiau & Fan（多级 2bit） | 0.77 s | 0.56 s |
| 本文（多级 2bit） | **1.97 s** | **1.28 s** |
| ——其中仅边缘检测 | 1.19 s | 0.72 s |

**约 2.6 倍，且六成开销在边缘那一步。**作者建议上 GPU、或给聚类设规模上限。

> **与 libimagequant 对照着读**（见上一节）：libimagequant 的边缘图是**二阶差分 ＋ 形态学开运算**，
> 没有聚类、没有统计筛选；Kiyotomo 是 Sobel ＋ 连通域 ＋ 方差/黑点数筛选。
> **两者在"边缘要先去伪"这一点上独立同意，在成本上差一个数量级。**

### "文字与连续调分开处理"有 35 年的工程正统

**这一段原先我只当领域常识写，证据等级低。补查之后它是本节证据最扎实的一块。**

按证据类型排开：

**厂商一手文档。** Xerox iGen 150 Press 产品手册（xerox.com 自有域名
<https://www.office.xerox.com/latest/IG1BR-01.PDF>）描述 **Object Oriented Halftoning**：

> For customer jobs with a mix of images and heavy text… **renders images at a 180-line screen
> for optimal smoothness**. For the different requirements of text, Object Oriented Halftoning
> **renders elements in the document at a 250-line screen that is optimized for sharpness**.

**同一页内，图像走低频屏求平滑、文字走高频屏求锐利。**
（反例也记一笔：Xerox Nuvera 的半调是按打印队列选 85/106/125/156 lpi，**不按对象类型**。
所以不能说"厂商普遍这么做"。）

**专利。** 两件直接命中：

- **US 5,034,990 A**，*Edge enhancement error diffusion thresholding for document images*，
  Kevin J. Klees / Eastman Kodak，1990-05-08 申请，1991-07-23 授权。含一个
  *"text/continuous tone discriminator"*，其输出直接调制误差扩散：
  > The strength of the error signal is **modulated by edge information** in the image… **if a
  > large edge signal is present, the error could be reduced by 1/2 so that less diffusion would
  > take place, and a sharper edge would result**.

  **"在边缘处把误差减半"——"局部减弱抖动"这件事 1991 年就有原型。**
- **US 8,059,311 B2**，*Font and line art rendering for multi-bit output devices*，
  Ching-Wei Chang / Sharp，2008-01-31 申请，2011-11-15 授权。**它的场景正是低位深设备**：
  半调在浅灰上造成 *"broken shapes and missing lines"*、*"preclude legible renderings of fonts
  and line art at the lighter levels of pigmentation"*；做法是对字体与线画**不铺半调图案**，
  把目标灰度 **snap 到设备原生灰阶并填实**——
  *"instead of filling a halftone pattern, a solid area will be filled"*。图像内容照走半调。

  > **✅ 两件专利我已用浏览器逐字复核**（2026-09-08，patents.google.com，`urllib` 直取仍是 503，
  > 浏览器可读）。上面所有带引号的片段在原文中**逐字存在**；受让人、发明人、申请/授权日期全部对上。
  > 复核中读到的三条原文里有、上面没写到的事实，见下条。

  > **复核补出来的三条，都收紧了这两件专利对我们的适用性：**
  >
  > 1. **US 5,034,990 不是二值专用的。** 原文逐字：*"a **2 bit or 4 bit output signal can be
  >    generated** if appropriate for a given application"*——**它自陈覆盖我们的候选档**。
  > 2. **它不解决"怎么认出文字"。** 那个 `text/continuous tone discriminator` 是**外挂**的，
  >    原文把实现指给另一件专利：*"One way for implementing the discriminator function is
  >    described in U.S. Pat. No. 4,577,235."*
  > 3. **它自己说增益要靠试。** 同一件专利里说了三遍：*"The determination of the gain factors to
  >    be applied is an **empirical task requiring many iterations**"*、*"actual values using the
  >    lookup tables are **determined empirically, and must be adjusted by experiment**"*。
  >    结构是 5-bit 边缘信号（3×3 邻域或跨行/跨列差分）选 **32 组**误差增益之一，边缘强时约取 1/2。
  >
  > **US 8,059,311 有一条决定性的前提**：它跑在 **PDL 解释器**里，字体/文字/线画是**对象**，
  > 不需要检测——*"For high density dpi rendering devices, a **PDL interpreter** may render black
  > fonts, text, and line art…"*、*"Method embodiments… may be implemented via a **PDL print
  > controller**"*。**与本节末尾 Allebach 2016 那条"object map extracted from the page
  > description language"是同一个前提。我们手上是栅格扫描页，这个前提不成立。**
  > 另外两条口径：它的示例设备正是 **4-bit / 15 级灰**（*"the 4-bit device, having 15 levels of
  > tone"*），与我们的目标面板同档；而 "填实" 的完整句是 *"instead of filling a halftone pattern,
  > a solid area will be filled **with one of the primary gray levels**"*——**填的是设备原生灰阶，
  > 不是纯黑**（正文的转述是对的，但引文截断处容易读岔）。

**开源实现。** Gutenprint 按内容类型分派**抖动算法**——`src/main/generic-options.c` 定义
`Text` / `Graphics` / `TextGraphics` / `Photo` / `LineArt` 五种；`src/main/dither-main.c` 的
`stpi_set_dither_function()` 把 `Text` 派给 `D_VERY_FAST`、`LineArt` 派给 `D_ADAPTIVE_HYBRID`、
`Photo` 派给 `D_EVENTONE`。帮助文本写着 *"Fast and Very Fast are considerably faster, and
**work well for text and line art**"*。**限定：这是整作业级设置，不是页内逐对象分区。**

**标准。** ITU-T T.44 *Mixed Raster Content*（现行 (01/2005)，ISO/IEC 16485 孪生）的三层模型
把文字**拆成两半**：笔画形状进 bi-level 的 mask 层，文字颜色进 multi-level 的 foreground 层，
连续调进 background 层。设计动机逐字：

> **Text and line-art data (bi-level data) would be compressed with an approach that puts high
> emphasis on maintaining the detail and structure of the input. Pictures and colour gradients
> (multi-level data) would be compressed using an approach that puts a high emphasis on
> maintaining the smoothness and accuracy of the colours.**

> **要精确**：MRC 的"硬二值"是**形状**层面的，不是**色调**层面的——文字最终颜色来自
> multi-level 的 foreground 层。把它当"文字不抖动"的先例是合理类比，
> **但标准并没有规定文字按 1-bit 色调渲染。**

同族的 ITU-T T.88（JBIG2）里有一句与我们直接相关的（§0.2.2）：
*"The preprocesses are **not well-suited for error diffused images and images dithered with
blue noise**."*

**两个有价值的否定结论**（子 agent 核过源码/规范）：

- **Ghostscript 有对象标签但不用于半调**：`base/gscms.h` 定义 `GS_TEXT_TAG` / `GS_IMAGE_TAG` /
  `GS_VECTOR_TAG`，但这些标签只驱动**色彩管理与抗锯齿**；`base/gxht.c`、`gxht_thresh.c` 等
  半调代码里**没有任何** `graphics_type_tag` 引用。
- **PostScript / PDF 格式层面没有按对象类型的半调**：PDF 32000-1:2008 的 ExtGState 只有
  **一个** `HT` 条目，无对象类型维度；PostScript 的 HalftoneType 唯一的专门化维度是**颜色分量**。
  但机制上可行——`HT` 在图形状态里，生产者完全可以在画文字前后各设一次。
  **与 T.44 把分割推给厂商是同一种设计取向。**

### 真正的难点在交界处，不在切换本身

**三个独立来源指向同一处。**

- Ostromoukhov（SIGGRAPH 2001）记录 Eschbach 的双系数集方案会让
  *"a visually disturbing **discontinuity** appears on the boundary"*。
- **Jae-Hyun Kwon, Chang-Hwan Son, Yang-Ho Cho, Yeong-Ho Ha**, *Text-Enhanced Error Diffusion
  Using Multiplicative Parameters and Error Scaling Factor*, Journal of Imaging Science and
  Technology 50(5):437–447, 2006, DOI `10.2352/J.ImagingSci.Technol.(2006)50:5(437)`。
  它让文字区走 text-enhanced ED、背景区走标准 FS，**并诚实记下了切换的代价**：
  > this combination of algorithms can generate **two kinds of artifact around the text regions:
  > boundary and dot-elimination artifacts**.

  更值得记的是它对**毛刺形状**的描述，与我们第 3 类劣化几乎逐字对应：
  > Boundary artifacts are **a series of dots distributed around text blocks**… As a result, the
  > uniformity of dot distributions is broken up and **a series of dot distributions are
  > generated below text line**.

  解法不是硬切换，而是用灰度膨胀造一条**渐变过渡带**（GDTR），让边缘增强参数沿带渐变
  （论文 Table I：GDTR 灰阶 1–31 / 32–63 / 64–127 / 128–191 / 192–255 对应 L = 0.125 /
  0.25 / 0.5 / 0.75 / 1.0）。
- **S. J. Park, M. Q. Shaw, G. Kerby, T. Nelson, D.-Y. Tzeng, K. R. Bengtson, J. P. Allebach**,
  *Halftone Blending Between Smooth and Detail Screens to Improve Print Quality With
  Electrophotographic Printers*, IEEE Trans. Image Processing 25(2):601–614, 2016,
  DOI `10.1109/TIP.2015.2500035`。**标题词就是 Blending**——重点是两种半调的交界处。
  而且它**不猜判据**：*"These regions are described by an **object map that is extracted from the
  page description language version of the document**."*

T.88 甚至专设一节叫 *Consequences of inadequate segmentation*——标准层面承认分割判错要付代价。

### 多少灰阶抗锯齿文字才活得下来

**基本空白。没有找到任何把这件事量化的工作。**

---

## 三、自动选择量化位深 / 量化参数

**判断：分三层看——"按指标搜参数"成熟；"自动定级数"有成熟实现；"给定 e-ink 面板在
{1,2,4}bit × {抖,不抖} 里逐页选最小可接受档"基本空白。**

### 自动定"用几个级"：pngquant 就是干这个的

pngquant 的 `--quality min-max` 语义与我们的目标句式几乎逐字相同。一手来源是仓库里的 man page
（`kornelski/pngquant`，`pngquant.1`，`main` 分支，第 63–78 行）：

> **pngquant will use the least amount of colors required to meet or exceed the max quality.**
> If conversion results in quality below the min quality the image won't be saved […] and
> pngquant will exit with status code 99.

同一份 man page 第 79–82 行的 `--skip-if-larger` 还写了一条**体积与质量的兑换规则**：

> Additionally, file size gain must be greater than the amount of quality lost. If quality
> drops by 50%, it will expect 50% file size reduction to consider it worthwhile.

以及第 83–84 行的 `--posterize`：

> Truncate number of least significant bits of color (per channel). **Use this when image will
> be output on low-depth displays** (e.g. 16-bit RGB).

**但判据是 MSE，不是感知指标。** `libimagequant` 里 `src/quant.rs:478` `quality_to_mse()` 与
`:487` `mse_to_quality()` 是一对经验拟合公式，把 0..100 映射成 MSE 阈值；真正的提前停机在
`src/mediancut.rs:294` `if self.total_box_error_below_target(target_mse) { break; }`
——**误差达标就停止分裂，产出少于上限的调色板**。"感知"成分只有 `src/mediancut.rs:214`
的一个直方图级加权。

> 这一点很重要：**离我们最近的在产工具，它的判据正是 `ADR 0002` 判定为"符号相反"的那一类。**
> 它绕开这个矛盾的方式是——**不用判据去决定要不要抖**：它总是抖（由边缘图逐像素调强度），
> 只对"用几个级"做搜索。

### 按指标闭环搜编码参数：成熟，且有两个真在产的例子

- **libaom `--tune=vmaf`**：`av1/encoder/tune_vmaf.c`（aomedia.googlesource.com/aom，main，
  2026-09-08 抓取）。`:610` `av1_set_mb_vmaf_rdmult_scaling()` 把源帧降采样后切成
  `BLOCK_32X32`（`:614`），**对每一块实际调用 VMAF**，由 `:721` 的 `dvmaf = kBaselineVmaf - vmaf`
  （`kBaselineVmaf = 97.42773`，`:19`）算出权重写进 `:735` 的 `rdmult_scaling_factors`。
  `:901` `av1_get_vmaf_base_qindex()` 再由 VMAF 推 qindex 偏移。**逐块、以真实感知指标为反馈。**
  （行号经我独立复核，与源码逐一对上。）
- **libjxl 的 Butteraugli 闭环**：`lib/jxl/enc_adaptive_quantization.cc` @
  `b5def9fb509d0f2421c8a5bcd7aa6f5a627363c4`，`:929` `FindBestQuantization()`；主循环 `:985`
  每轮实编实解一次，按每个 tile 的 `实测 Butteraugli 距离 / 目标距离` 之比直接乘到量化场上
  （`:1071`、`:1092`）。只在 `-e 8`（kitten）起启用。
- **Guetzli**：Jyrki Alakuijala et al., *Guetzli: Perceptually Guided JPEG Encoder*,
  arXiv:1703.04421, 2017——以 Butteraugli 为反馈的闭环优化器，自述
  "Guetzli's computation is currently extremely slow"。

**一个要纠正的常见误解**：`cjxl --distance` **不是**搜索，是参数映射
（libjxl `doc/man/cjxl.txt`："It is specified in multiples of a just-noticeable difference"）。
命令行上真做目标搜索的是 `cwebp -size/-psnr`（官方文档说明用 `-pass` 控制**二分**趟数）、
`avifenc --target-size`（`apps/avifenc.c:1310` 运行时打印 "Starting a binary search"，
判据是**纯字节数**）、以及第三方 `oavif`（以 **SSIMULACRA2** 为目标，`--score-tgt` 默认 80）。

**编码器侧的 AQ 不是这一类**：x264 的 `x264_adaptive_quant_frame()`
（`encoder/ratecontrol.c:304`，公式在 `:396`）是**开环方差启发式**——用方差当掩蔽的代理，
从不测量结果质量；源码注释自陈常数"only tuned to 2 [significant digits]"（`:348-349`）。
x265 的 `--aq-mode` 文档措辞同样是 "complexity analysis of the source image"。

### 逐题判断

| 子问题 | 判断 |
|---|---|
| 自动定量化级数（带停机准则） | 有成熟方案（准则是加权 MSE，不是感知指标） |
| "感知无损"的位深削减 | 有学术工作但未落地；且**调的是量化步长 / QP，不是位深** |
| 位深可分级编码自动定深 | 基本空白（JPEG XT / SHVC 是分层传输工具，不含判定机制） |
| 全图"要不要抖"二选一 | 基本空白 |
| 逐区域"这里要不要抖" | 有成熟方案（见第二节解法五） |
| 逐块 / 逐区域定量化参数 | 有成熟方案 |
| 按图指标驱动的参数搜索 | 有成熟方案 |
| 电子书 / 漫画的体积-质量自动调参 | **基本空白** |

"感知无损位深削减"那一行的代表作：Lee Prangnell, *Visually lossless coding in HEVC: A high bit
depth and 4:4:4 capable JND-based perceptual quantisation technique for HEVC*,
Signal Processing: Image Communication 63:125–140, 2018, DOI `10.1016/j.image.2018.02.007`
（arXiv:1708.06417）。**它是否定证据**：JND 文献一致地把 JND 用来定 QP，而不是用来定位深。

自动估计颜色数确有其事，但准则是统计的：M. Kılıçaslan, M. O. İncetaş, *Adaptive Color
Quantization Method with Multi-level Thresholding*, International Journal of Computational
Intelligence Systems 16, 2023, DOI `10.1007/s44196-023-00185-x`——"automatically estimates the
number of colors by **multi-level thresholding based on the histogram**"。

### 一个现成的、专打"级数不够"的指标：CAMBI

banding 正是"级数不够又没抖"产生的伪影，而它有专用的无参考检测器，且**有公开阈值**。

一手论文：Pulkit Tandon, Mariana Afonso, Joel Sole, Lukáš Krasula, *CAMBI: Contrast-aware
Multiscale Banding Index*, Proc. Picture Coding Symposium (PCS) 2021, pp. 1–5,
DOI `10.1109/PCS50896.2021.9477464`。已集成进 libvmaf，官方文档
`Netflix/vmaf` 的 `resource/doc/cambi.md`（我直接读过该文件），逐字：

> The CAMBI score starts at 0, meaning no banding is detected. […] The maximum CAMBI observed
> in a sequence is 24 (unwatchable). **As a rule of thumb, a CAMBI score around 5 is where
> banding starts to become slightly annoying** (also note that banding is highly dependent on
> the viewing environment - the brighter the display, and the dimmer the ambient light, the
> more visible banding is).

同一文档记：支持 8/10/12/16 bit 输入（内部统一到 10-bit 计算）；参数 `max_log_contrast` 默认 2
"equivalent to 4 luma levels at 10-bit and **1 luma level at 8-bit**"；`window_size` 默认 63
"corresponds to ~1 degree at 4K resolution and 1.5H"——**又是一个把窗口尺寸绑到视角上的模型**。

早期同类：Zhengzhong Tu, Jessie Lin, Yilin Wang, Balu Adsumilli, Alan C. Bovik,
*BBAND Index: A No-Reference Banding Artifact Predictor*, Proc. IEEE ICASSP 2020,
pp. 2712–2716, DOI `10.1109/ICASSP40776.2020.9053634`。

**⚠️ 但 CAMBI 拿不到我们的线稿页上用，理由有三条，而且都是设计上的，不是标定不足。**

1. **它不排除抖动，反而是为抖动设计的**——论文逐字：*"Presence of dithering makes banding
   detection harder … we use a **2 × 2 averaging low-pass filter (LPF)** to smoothen the
   intensity values, in an attempt to replicate the low-pass filtering done by the human
   visual system."* 这一条与流行猜测相反。
2. **它按构造把纹理丢掉**——论文逐字：*"Hyperparameter g **ensures avoidance of textures**
   during banding detection."* **线稿整页都是高梯度，会在打分之前被丢掉。**
3. **它的对比度天花板是 1 个 8-bit 灰阶**——TechBlog 逐字：*"The maximum computed contrast is
   one luma level in 8-bit and four luma levels in 10-bit content. **Larger contrasts are not
   typical in video compression pipelines**."* 我们 1bit 上的颗粒幅度在 100 灰度级以上。

它的验证域是九段 4K/10-bit SDR 片源、ffmpeg 有序抖动 ±1 个 8-bit 灰阶、23 名观察者。
**它量的是"大片平滑低梯度区里的 ±1 级台阶"，与我们的第 1 类劣化只是听起来像。**

（这三条逐字引文由子 agent 从 arXiv:2102.00079 与 Netflix TechBlog 读出，**我未复核**；
`cambi.md` 里确实没有 Scope 或 Limitations 小节——这一条我自己看过那份文件。）

BBAND 有同样的隐含排除：Sobel 梯度高于 T2=12 的像素被标为 *"textures"* 并丢弃。

---

## 四、电子墨水与漫画转换工具的实际做法

**判断：分三层，差别极大——面板侧有第一方规格书说死；显示层（KOReader / FBInk）有成熟且
写下了理由的做法；转换层（KCC / calibre / ComicRack）基本空白。**

### 面板侧：16 级是波形定义出来的原生态，不是抖出来的

**第一方来源**：E Ink Corporation, *AF 16 Tone Grayscale 5-Bit Waveform Flash File Product
Specification*, 文档号 `800-1101 REV01`（PDF 内嵌 XMP 元数据的 `dc:title`，CreationDate
2017-01-11）。§1 逐字：

> The AF waveform is a high performance **4-bit (16-level) grayscale waveform**. The AF waveform
> look-up tables are defined in a **5-bit (32-level) pixel state representation** where the 16
> graytones are assigned to the even pixel states (0, 2, 4, … 30), where 0 is black and 30 is white.

奇数态 29、31 是**特殊跃迁**，不是额外灰阶（§2.2）。

> **出处要打折**：这份文档 §1 自称
> *"for use by E Ink Corporation and their customers under non-disclosure agreements"*。
> 内容是第一方且权威，但它流到公开镜像（waveshare.com）上，**出处不体面**。引用时要带这句话。

同一份规格书的模式表（Table 1，25 °C / 85 Hz），这是"哪个更新模式几级灰"的第一方出处：

| Mode | 灰阶 | 用途（原文） | ms |
|---|---|---|---|
| **DU** | **2** | Monochrome menu, text input, and touch screen/pen input | 260 |
| **GC16** | **16** | High quality images | 450 |
| **GL16** | **16** | Text with white background | 450 |
| GLR16 / GLD16 (REAGL/-D) | 16 | Text (and graphics) with white background | 450 |
| **A2** | **2** | Fast page flipping at reduced contrast | 120 |
| **DU4** | **4** | **Anti-aliased text** in menus / touch and screen/pen input | 290 |

另一份第一方 datasheet 给出更干净的表述——E Ink Holdings, ED060SC7 Technical Specification,
P-511-608(V:2), Rev 2.0, 2010-11-15, §1：

> the display is capable to display images at **2-16 gray levels (1-4 bits) depending on the
> display controller and the associated waveform file used**.

即：**灰阶数不是墨水膜的属性，是控制器加波形文件的属性。**现代产品页（eink.com，ED060KHE /
Carta 1300，300 PPI）规格表写的是 "16 Gray Level (monochrome)"。

**⚠️ 一条重要的负面发现：这份 E Ink 波形规格书里 "dither" 与 "halftone" 出现 0 次。**
（我自己下载 PDF、pdftotext 之后 grep 复核过，计数为 0。）**E Ink 自己对抖动只字未提。**
它提到的唯一主机侧算法是 GLR16 / GLD16 所配的 *"image preprocessing algorithm"*（即 REGAL），
而且明说这两个模式 *"will point to the same voltage lists as the GL16 data"*
——**REGAL 是主机侧的图像处理，不是另一条波形。**
同样地，2008 年那代 EPD 控制器（Epson / E Ink Broadsheet `S1D13521B01` 硬件功能规格书，
Rev 1.2, 2008-12-09）里 "dither" / "halftone" / "error diffusion" 也是 0 次；它对位深的处理是
**纯高位截断**（§10.1："Appropriate bits from the byte are used for grey-scale conversion
(Most Significant bit is always utilized)"）。**抖动是后来才进硬件的。**

E Ink 自己的专利里倒是有抖动，但**动机不是"灰阶不够"**：US 9,495,918 B2（E Ink Corp）用
半色调 *"to place the grain into higher spatial frequencies so that it is less visible"*，
目标是**灰阶落点误差**（材料不均匀）；US 8,456,414 B2（原 SiPix）做 8-bit → 16 级的
gamma + 误差扩散，理由是 γ=2.2 映射后**实际只剩 11 个可区分灰阶**。
（专利受让人与要点由子 agent 逐件 fetch 核对，**我未独立复核**。）

**为什么快速模式偏爱 1-bit 内容**，有一份设备厂的一手出处：US 9,245,485 B1
（**Amazon Technologies**，发明人 Hao Hu，2016-01-26 授权）——
*"the one-bit display mode enables quicker updating […] but the one-bit images it produces have
far less fidelity"*，因此在 1-bit 快速模式下对**图像**补抖动。

### 驱动侧：业界实际落地的就是 ImageMagick 那张 8×8 表

（源码由子 agent 抓取，**行号我未复核**；逐字引文是稳定锚点。）

NXP `linux-imx` 分支 `lf-6.6.y` 的 EPDC 驱动 `drivers/video/fbdev/mxc/mxc_epdc_fb.c`
里有软件误差扩散，注释自陈 *"we chose to implement **Bill Atkinson's algorithm** as an example"*，
提供 Y8→Y1（配 A2 波形）与 Y8→Y4 两条。

而 PxP 硬件抖动（`drivers/dma/pxp/pxp_dma_v3.c`）的三张表 `bit1/bit2/bit4_dither_data_8x8[64]`,
**数值序列就是 ImageMagick `thresholds.xml` 里的 `o8x8` 阈值图**，也正是 FBInk `dither_o8x8()`
用的同一张表。软硬两条路径在数学上是同一个东西。

枚举 `PASS_THROUGH 0 / FLOYD 1 / ATKINSON 2 / ORDERED 3 / QUANT_ONLY 4` 里，
**硬件上只有 PASSTHROUGH 与 ORDERED 真实存在**——驱动遇到别的值会报
*"Not supported dithering mode. Forced to be Orderred mode!"*。NXP 第一方的 MCUXpresso SDK
文档印证：pre-dither LUT *"could only be used in Floyd mode or Atkinson mode, **which are not
supported by current PXP module**"*。

> **两套格点并存，跨层比对会踩**：FBInk / eInk cmap 是 `0x00, 0x11, … 0xFF`（步长 **17**，
> `fbink_internal.h:482` 的 `eInkFGCMap[16]`，我复核过这一条），而内核 Y4 抖动量化到
> `0x00, 0x10, … 0xF0`（步长 **16**，白 = 240）。到面板层因为只取高 4 位所以等价，
> 中间 Y8 缓冲的数值表示**不等价**。

### 电泳显示的图像准备：有学术工作，很薄，且集中在一个课题组

**唯一的综述在 2026 年才出现**：Xiangjie Zhong, Xiaoyan Zhao, Tiesong Zhao,
*Review on Image Preprocessing and Driving Waveform Design for Electrophoretic Displays*,
Journal of the SID 34(8):669–680, 2026, DOI `10.1002/jsid.70044`。
**这个方向近 20 年没被系统梳理过，这是唯一入口。**

主力是台师大 Wen-Chung Kao 一组（2009 至今连续发表），三条对 16 级管线直接相关的结论：

- W.-C. Kao et al., *Design of Real-Time Image Processing Engine for Electrophoretic Displays*,
  J. Display Technology 7(10):556–561, 2011, DOI `10.1109/JDT.2011.2159360`——
  阈值要按**当前温度下实际可用的灰阶数**现算，EPD 可用灰阶随温度变化。
- W.-C. Kao et al., *Image quality improvement for EPDs by combining contrast enhancement and
  halftoning*, IEEE Trans. Consumer Electronics 55(1):15–19, 2009, DOI `10.1109/TCE.2009.4814408`
  ——光靠半色调不够，**必须先做对比度增强再做误差扩散**，因为 EPD 反射率动态范围远窄于源图。
- Z. Qin et al., *Digital halftoning method with simultaneously optimized perceptual image
  quality and drive current*, Applied Optics 59(1):201, 2020, DOI `10.1364/AO.59.000201`——
  **半色调本身有能耗代价**：相邻像素频繁反向翻转会让驱动电流暴涨。

（Kao / Qin 这几条的题录由子 agent 经 Crossref 核对，**我未独立复核**。）

**明确的负面结果**（子 agent 的检索结论）：arXiv 上没有 e-paper 图像准备的论文；
IS&T CIC / Electronic Imaging 上没有专门做 e-paper 半色调的论文；
**没有 e-paper 图像质量的公开 benchmark 或数据集**；
上面那支 multitoning 文献与这支 e-paper 文献**几乎不互相引用**。

### 什么时候该抖、什么时候不该：没人写过"文字不要抖"，但四条独立证据指向同一处

**没有任何一家厂商写过"文字不要抖动"这句话。**但：

1. **E Ink 自己给文字配的全是"灰阶抗锯齿"模式**：GL16 用途是 "a page of **anti-aliased text**"，
   DU4 是 "**anti-aliased text** in menus"，Table 1 里 GL16/GLR16/GLD16 全是 "Text with white
   background"。而 1-bit 的 DU 用途写的是 "Monochrome menu, **text input**"——是输入延迟场景，
   不是排版正文。**整份规格书 0 次提到 dither。**
2. **KOReader 把"要抖动"定义成"这块内容里有图"**：`uimanager.lua` 逐字
   *"an optional hint … that this repaint could benefit from dithering (**e.g., because it
   contains an image**)"*；驱动侧 `framebuffer_mxcfb.lua` 重申
   *"We assume the dither flag is only set on **image content**"*。
3. **FBInk 的软件抖动只存在于 `draw_image()` 里，文字渲染路径一次都不调用**。
4. **Amazon 的官方出版指南（`AmazonKindlePublishingGuidelines.pdf`）里 "dither" 出现 0 次**，
   对线稿的建议是用 PNG、减色，并 *"pay close attention to the **legibility of text**"*。

**唯一一段正面讨论"抖动 vs 文字"的一手工程判断**在 KOReader `framebuffer_mxcfb.lua`
的 `refresh_k51` 注释里，而且讲的是**硬阈值**不是抖动：

> Since we mainly use DU for highlights, the color decimation quantization will effectively
> **crush antialiasing on text, avoiding making the text look fuzzy** during the refresh (it'll
> instead look **blockier**, because of the lack of AA). The higher density the screen is, the
> better this approach will look vs. fuzzy refresh artifacts

> In the very few cases we use A2 (i.e., the keyboard), using FORCE_MONOCHROME would be
> **actively harmful**

即：在 2 级模式下，把文字**硬阈值化成块状**优于让它**模糊**；同一招用在 A2 局部高亮上就有害。
代码里抖动与 `FORCE_MONOCHROME` 是**互斥**的，两者是替代关系，从不叠加。

> **注意这条与我们的第 3 类劣化方向相反**：它说的是"2 级下宁可块状不要模糊"，
> 而我们观察到的是"2bit 下边缘长毛刺"。**两者是不是同一个权衡的两端，本文不下判断。**

### 转换层：判据存在过、正确过，15 天后被一次无声的提交翻掉

**这是本次源码考古最实质的发现，而且它的第一版结论是错的**——我原先写"抖动是个意外，不是决策"。
补查 git 历史之后，准确的说法是：**它曾经是决策，理由写下来过，然后失传了。**

**2013 年 3 月 5 日，两条真正的工程判据被写进了 KCC**（三条提交的 message、作者、日期与
diff 内容我都用 GitHub API 与 `.patch` 复核过）：

`751e6eb`，Frédéric Devernay，2013-03-05，commit message 逐字：
**`save dithered images as PNG, and linearize (inverse gamma) before dithering`**

diff 里两条判据：

```python
+ self.image.save(..., "PNG") # quantized images don't like JPEG
+ def optimizeImage(self, gamma):
+     self.image = ImageOps.autocontrast(Image.eval(self.image, lambda a: 255*(a/255.)**gamma))
+ parser.add_option("--gamma", type="float", dest="gamma", default=2.2, ...)
```

1. **`# quantized images don't like JPEG`**——抖动过的图不能交给有损编码器。
   十年后 NiLuJe 在 koreader#7949 里逐字重申同一条。
2. **抖动前先线性化**，`--gamma` 默认 **2.2**。在感知编码值上做误差扩散是错的，
   误差要在线性光里扩散——这是一条正确且非平凡的判据。

同日 `b162425`（Paweł Jastrzębski，"Added option to disable dithering"）加了 `--nodithering`，
默认 False，**即抖动默认开**，并把格式与抖动正确地耦合起来。

**15 天后，`cff9b73`（Paweł Jastrzębski，2013-03-20），commit message 只有三个词：
`Force JPEG output`。** diff：

```diff
-    if not options.notquantize:
+    if options.forcepng:
-    parser.add_option("--nodithering", ..., default=False, ...)
+    parser.add_option("--forcepng",   ..., default=False, ...)
```

**一次开关重命名，把默认值翻了过来**：`--nodithering` 默认关 ⇒ 抖动开；
`--forcepng` 默认关 ⇒ 抖动关。同一次提交里 `--gamma` 默认值从 `2.2` 改成 Auto。
**对这次默认翻转零解释。**

**今天的状态**：那道门就是 `comic2ebook.py` 里 `elif opt.forcepng:` 那一行（13 年未动）；
线性化那一步**代码还在、执行顺序也还对，但已经空转**——41 条 profile 的 gamma 全是 `1.0`，
表达式恒等。而帮助文本至今写着 *"Apply gamma correction to **linearize the image**"*，
**是一句指向不再发生的行为的化石**。

> 因此本节的判决 `基本空白` 反而更站得住：**不是没人想清楚过，是想清楚了没写在会被读到的地方。**
> `docs/` 没有、README 没有、wiki 没有，CHANGELOG 只留下 `--nodithering` 那一行墓碑。

下面是今天这份代码的状态。

**Kindle Comic Converter**，`ciromattia/kcc` @ `5b67d4019eb9074fc8a8278d124700bfd03f373d`
（2026-09-07，v11.2.0）。全仓库唯一改变输出位深的地方是
`kindlecomicconverter/image.py:501-508`，我逐字复核过：

```python
def quantizeImage(self):
    # remove all color pixels from image, since colorCheck() has some tolerance
    # quantize with a small number of color pixels in a mostly b/w image can have unexpected results
    self.image = self.image.convert("RGB")

    palImg = Image.new('P', (1, 1))
    palImg.putpalette(self.palette)
    self.image = self.image.quantize(palette=palImg)
```

**注意它没有传 `dither=`。** 而 Pillow 的签名（`python-pillow/Pillow`，`src/PIL/Image.py:1281`，
我复核过）是：

```python
dither: Dither = Dither.FLOYDSTEINBERG,
```

**所以 KCC 十几年来一直在做 Floyd–Steinberg 抖动，而今天的代码里没有一个字提到抖动。**
那两行注释解释的是"为什么先 `convert("RGB")`"，与位深、与抖动都无关。
**2013 年那句 `# quantized images don't like JPEG` 已经不在了**（我 grep 过当前 `image.py`）；
线性化那一步还在（`image.py:451-460` 的 `gammaCorrectImage()`），但 `:457` 有一道
`if gamma == 1.0:` 的提前返回，而 41 条 profile 的 gamma 全是 1.0——**它恒空转**。

写下来的理由，全项目只有 `README.md:84-87` 一句半（我逐字复核过）：

> compatibility with image formats better suited for manga like 4-bit PNG and WEBP. This enables
> better quality at **half the file size** compared to JPG which must be 8-bit.
> **This is visually lossless since eink screens are 4-bit.**

**"visually lossless since eink screens are 4-bit"——这就是整个生态记录在案的全部判据。**
没有实测，没有对网点的讨论。

其余（由子 agent 读取，行号我未逐条复核）：调色板表有 `Palette4` / `Palette15` / `Palette16`
三张，41 条 profile 里 39 条用 `Palette16`，41 条的 gamma 全是 1.0；默认输出 JPEG，
量化被 `--forcepng` 与 `--noquantize` 两道门挡着，**默认转换根本走不到量化**。
同一段时间还有第三条判据（issue #37 / PR #36，devernay，2013-03-18）：
"**Downscaling or upscaling a dithered image produces undesirable artifacts**"
——即抖动只在原样显示时才成立，缩放会毁掉它。

> **更正**：我原先写 `--nodithering` 这个开关"后来被删了"。**准确说法是它被改名成
> `--forcepng` 且语义反转**（见上）——删掉的是名字与默认值，功能一直在。

**KCC issue #980 存在，我独立核实过**：标题是 *Smarter Bit-Depth Detection for PNG Output*，
作者 9783e6，2025-06-18 开，**至今 Open、零评论**。逐字：

> not all source images that are genuinely 1-bit should remain 1-bit after processing. […]
> **resizing a 3000×4500 1-bit image to 1000×1500 and saving it again as 1-bit can result in a
> significant quality drop.** 2-bit helps somewhat, but 4-bit tends to preserve visual fidelity
> much better

> some images look like they only use 2 colors, but due to **anti-aliasing (especially on text)**,
> they actually use 256 shades of gray.

> 注意与我们 `docs/measurements.md` 的《A 类素材普查》对照：那里记的是本项目语料里
> **没有一页是 1bit 存储**。#980 描述的是同一条路径的另一端，两者不矛盾——它讲的是
> "源真是 1bit 时会怎样"，我们量的是"手头的源是不是 1bit"。

**calibre** 更值得记一笔（关键行号我复核过）：

- 漫画管线走 `Format_Grayscale16`（`src/calibre/ebooks/comic/input.py:233`），**65536 级，无抖动**。
- `--colors` 默认 **0**（`src/calibre/ebooks/conversion/plugins/comic_input.py:30`
  `recommended_value=0`），且只在 PNG 输出时生效；帮助文本给的理由是
  "It is useful to **reduce file sizes**"——体积，不是观感。
- 设备 profile 表里**没有任何一条记录位深或灰阶数**。
- **而 calibre 里有一个现成的 e-ink 抖动器**：`src/calibre/utils/img.py:563-574`
  `eink_dither_image()`，docstring 逐字："Dither the source image down to the eInk palette of
  **16 shades of grey**, using ImageMagick's OrderedDither algorithm."
  **它从未接进漫画管线**——唯一调用点在 `img.py:326` 的封面 / 缩略图路径（旁边的注释在讲 Kobo 的
  Nickel 固件）。

**ComicRack**：官方源码从未释出，能拿到的是反编译重建的 `maforget/ComicRackCE`。
导出管线只有缩放、JPEG 质量、重采样与色调调整，对 `dither` / `quantiz` / `palette` /
`Format4bppIndexed` 的检索全部零命中。**它不碰位深，也从未声称要碰。**

### 显示层：这一层是成熟的，而且写了理由

（以下由子 agent 读取，`koreader/koreader` @ `cccc87affe5ca2b45208814da0a6d12726f1ee83`、
`NiLuJe/FBInk` @ `886f25f13368859ad8a899b88d04c26e19cda32e`；**行号我未逐条复核**。）

- **问题陈述写进了文档**，KOReader `doc/Porting.md:55-57`：面板"will do a **decimating
  quantization pass on its own on refresh**"，所以任何 < `0x11` 的值都会被显示成纯黑。
- **目标调色板是显式常量**，FBInk `fbink_internal.h:482-485` 的 `eInkFGCMap[16]`
  = `0x00, 0x11, … 0xFF`——与 KCC 的 `Palette16` 是同一张表，区别是这边写了它为什么是这张表。
- **软件抖动的注释解释了为什么是 16 级**，`koreader-base/blitbuffer.c:1335-1336`：
  "Quantize an 8-bit color value down to a palette of 16 evenly spaced colors, using an ordered
  8x8 dithering pattern. With a grayscale input, this happens to match the eInk palette perfectly"。
  抖动矩阵用**源图坐标**索引（`:1390`），因此图案锚在图上、平移时不爬行。
- **失效模式点名了 banding**，`frontend/ui/widget/imageviewer.lua:379-382` 的注释说关掉抖动
  就会看到"color quantization artefacts (i.e., **banding**) like crazy"。
- **"什么时候值得抖"有量化判据**：`readerview.lua:279`，图像覆盖率 ≥ **7.5%** 才开抖动，
  阈值在 PR #4541 里从 50% 谈到 7.5%。原则（NiLuJe）："quantization is technically a highly
  destructive process, so you don't want to do it when you don't have to"。
- **"只量化一次"被写成规则**：FBInk `fbink.c:3101-3102` 的顺序陷阱（先阈值化会毁掉后面的抖动），
  与 `imagewidget.lua:547-548` 的"图标已在编码时抖过、不要重抖"，是同一条规则的两面。

**两层之间的断口，是这次考古最值得记的一句。** NiLuJe 在 koreader#7949 里明说
（<https://github.com/koreader/koreader/issues/7949#issuecomment-877433882>）：

> The reasoning being it's mostly useful for CBZs, and **you probably should be building those
> yourself in the first place for best results, in which case, that creation pass is the perfect
> place to do it right**.

**显示层把这件事推给转换层；而转换层没有接。**

### 网点降采样摩尔纹

**在整个工具生态里没有任何实现。** KCC 有 `--eraserainbow`（默认关，
`rainbow_artifacts_eraser.py`，在 rfft2 后衰减 135°±10°、≥0.30 cycles/pixel 的频率），
但它治的是**彩色 e-ink 的 CFA 干涉**，不是"下采样把网点打成摩尔纹"。两者都叫摩尔纹，成因不同。
用户反复提这件事（KCC #626 的解法是"把整条处理管线关掉"），无人回应。

学术侧的 descreening / inverse halftoning 文献是成熟的，但**没有一行进过这些工具**——
这一项单独判是**有学术工作但未落地**。

---

## 五、方法论：从共同根源出发的框架

**判断：框架本身成熟且谱系清楚，但对我们这种输入**基本空白**——
没有一个框架说过"我对近二值 / 高对比线稿无效"，它们只是**从未把这类输入放进标定域**。
不是"明确排除"，是"根本没考虑"。唯一自己写下这层意思的是 DICOM PS3.14。**

**能引用的不是它们的免责声明（那不存在），而是它们各自申明的标定域。**

### 谱系

- **Daly VDP**（这一支的起点）：Scott Daly, *The visible differences predictor: an algorithm for
  the assessment of image fidelity*, in A. B. Watson (ed.), *Digital Images and Human Vision*,
  MIT Press, 1993, pp. 179–206（ACM DL `10.5555/197765.197783`；ISBN 0-262-23171-9）。
  **输出是逐像素的检测概率图**，不是一个标量分数。流程：幅度非线性 → CSF →
  cortex transform（频率 × 朝向通道）→ 掩蔽函数 → 心理测量函数 → 概率求和。

  > **⚠️ 著录陷阱**：另有一篇**题名几乎相同**的早一年的会议论文——Scott Daly,
  > *Visible differences predictor: an algorithm for the assessment of image fidelity*,
  > Proc. SPIE **1666**, Human Vision, Visual Processing, and Digital Display III, pp. 2–15,
  > 1992, DOI `10.1117/12.135952`。**HDR-VDP-3 的参考文献表把这两篇混在了一条里**
  > （卷号写 1666、出版社写 MIT Press、页码写 179–206、DOI 给 SPIE 那一篇）。
  > **不要照抄那一条。**
- **HDR-VDP-2**（这一支被引用最多的一版）：Rafał Mantiuk, Kil Joong Kim, Allan G. Rempel,
  Wolfgang Heidrich, *HDR-VDP-2: a calibrated visual metric for visibility and quality
  predictions in all luminance conditions*, ACM Trans. Graphics 30(4):1–14, SIGGRAPH 2011,
  DOI `10.1145/2010324.1964935`。题名里的 "calibrated" 与 "all luminance conditions"
  指的是**亮度**范围，不是内容范围。
- **HDR-VDP-3**：Rafał K. Mantiuk, Dounia Hammou, Param Hanji, *HDR-VDP-3: A multi-metric for
  predicting image differences, quality and contrast distortions in high dynamic range and
  regular content*, arXiv:2304.13625, 2023。摘要自述可做 full-reference IQA、可见差异预测、
  对比失真预测三件事。
- **ColorVideoVDP**（这一支最新）：Rafał K. Mantiuk, Param Hanji, Maliha Ashraf, Yuta Asano,
  Alexandre Chapiro, *ColorVideoVDP: A visual difference predictor for image, video and display
  distortions*, ACM Trans. Graphics, SIGGRAPH 2024, Article 129, DOI `10.1145/3658144`。
  建立在色度时空 CSF 与**跨通道对比掩蔽**的新心理物理模型上，显式考虑观看条件与显示器的
  几何、光度特性。训练针对的是视频串流失真与 8 类 AR/VR 显示伪影。
- **Lubin 的 VDM**：Jeffrey Lubin, *A visual discrimination model for imaging system design and
  evaluation*, in E. Peli (ed.), *Vision Models for Target Detection and Recognition*,
  pp. 245–283, World Scientific, 1995。
- **掩蔽的经典出处**：Eli Peli, *Contrast in complex images*, J. Opt. Soc. Am. A 7:2032–2039,
  Oct. 1990。

（Lubin 与 Peli 两条的著录取自 Kite 论文的参考文献表 [80]、[81]，我从 PDF 里逐字读出。）

### "视觉无损需要多少灰阶"：DICOM GSDF 是这一问最硬的一手材料

**DICOM PS3.14 — Grayscale Standard Display Function** 是一份公开的标准文本
（<https://dicom.nema.org/medical/dicom/current/output/html/part14.html>）。要点：

- 它建立在 **Barten** 的对比敏感度模型上：Peter G. J. Barten, *Physical model for the contrast
  sensitivity of the human eye*, Proc. SPIE **1666**, pp. 57–72, 1992, DOI `10.1117/12.135956`。
  **注意 PS3.14 的 §8 参考文献只有两条 SPIE 会议论文（1992 那篇与 1993 的 spatio-temporal 篇），
  并不引用** Barten 那本常被一起引的专著（*Contrast Sensitivity of the Human Eye and Its
  Effects on Image Quality*, SPIE Press, 1999, DOI `10.1117/3.353254`）。两者别混着引。
- **在 0.05–4000 cd/m² 的亮度范围内约跨 1023 个 JND**，GSDF 就是这 1023 级的数学插值。
- **决定性的一条——它的标定刺激**。PS3.14 §3.1 对 Standard Target 的定义，逐字：

  > A 2-deg x 2-deg square filled with a horizontal or vertical grating with **sinusoidal
  > modulation of 4 cycles per degree**.

**这就是"适用范围"问题的答案形状**：JND 的整个刻度是在**单一频率的正弦光栅**上定出来的。
网点是高频规则脉冲串，线稿边缘是宽带阶跃——**两者都离 4 cycles/degree 的正弦光栅极远**。

**而这一次，标准自己把话说了。**PS3.14 在同一处加了一条 Note（子 agent 读全文得到）：

> **Note** — The **academic nature** of the Standard Target is recognized. … **Only spurious
> results with more realistic targets in complex surroundings were known at the time of writing
> PS3.14 and these were not assessed.**

Annex A 进一步限定：

> Perceptual linearization is realizable, in a strict sense, **only for rather simple images like
> square patterns or gratings in a uniform surrounding**.

> An image containing different Luminance levels with different targets and Luminance
> distributions at the same time is **in general not perceptually linearized**.

**这是本次调研里唯一一份自己写下了"我的标定刺激很学院派、复杂场景没评估过"的标准。**
其余框架都只是沉默。

**它对位深的态度是拒绝规定，但给了一个上界**（Annex C.2 与 Annex E）：

> PS3.14 does not prescribe a certain number of gray levels of output.

> if only 8 bits per pixel are presented to the Display System, the number of JNDs achievable
> **cannot exceed 2⁸ = 256 JNDs** because of the quantizing effect.

全篇唯一一次提到 contour：
*"If digitization levels lead to luminance or optical density levels that are perceptually
indistinguishable, they are wasted. **If they are too far apart, the observer may see contours.**"*

（PS3.14 §1 另自陈 "neither a performance nor an image display standard"。表 B-1 两端实测：
j=1 → 0.0500 cd/m²，j=1023 → **3993.4040** cd/m²，不是整 4000。）

### "视觉无损要 7–8 bit"这个说法，没有一个一手出处

**这是一条需要纠正的常见引用。**子 agent 追到的是一个**区间**，而且明确依赖源噪声：

- **W. M. Goodall, *Television by Pulse Code Modulation*, Bell System Technical Journal
  30(1):33–49, Jan 1951**, DOI `10.1002/j.1538-7305.1951.tb01365.x`（子 agent 读了全文扫描）：
  > an **eight- or nine-digit PCM system** would be needed to avoid appreciable degradation of
  > the 60 db signal.

  > the contour effects which are strikingly apparent for one, two, and three digits are
  > **hardly noticeable for five digits**.

  > As was expected, **the number of digits required depends upon the amount of noise in the
  > test signal**.
- **L. G. Roberts, *Picture coding using pseudo-random noise*, IRE Trans. Information Theory
  8(2):145–154, 1962**, DOI `10.1109/TIT.1962.1057702`（仅摘要）：
  > at least **six bits** are required at each sample point, since the eye is very sensitive to
  > small steps introduced by quantization. However, by simply **adding some noise to the signal
  > before it is quantized and subtracting the same noise at the receiver**, quantization steps
  > can be broken up and the source rate reduced to **three bits** per sample.

**不要把"7–8 bit"归给任何单一出处——没有这样一句话。**
文献支持的是 **6 到 8–9 的区间，且明确以源噪声为条件**。
Goodall 早 11 年就确立了"噪声换灰阶"这个事实（他用的是不减回去的加性噪声，并自陈"付了代价"），
Roberts 的贡献是把抖动做成**可减除的**。

> **这一条与我们的处境直接相关的地方在于**：它说明"要多少 bit"从一开始就不是一个纯数字问题，
> 而是**"源里有多少噪声"的函数**。我们的两类素材（规则网点 vs 连续灰调）在这个轴上正好是两端。

### 这些框架在我们这种输入上站不站得住

**没有一个框架说过"我对近二值 / 半色调 / 线稿无效"。这句话在一手文献里不存在。
它们的沉默本身就是结论。**

子 agent 对**实际拿到全文**的十二份来源做了 `halftone` / `dither` / `line art` / `binary image`
的全文检索，相关命中数如下（HDR-VDP-3 与 DICOM PS3.14 两项我自己复核过）：

| 来源 | 相关命中 |
|---|---|
| HDR-VDP-2 (2011) 全文 | 0（唯一一次 dither 指的是**显示器**为达到 12-bit 用的抖动） |
| **HDR-VDP-3 (2023) 全文** | **0**（我自己 grep 复核） |
| HDR-VDP FAQ ＋ 随码 README | 0 |
| FovVideoVDP (2021) 全文 | 0 |
| Legge & Foley 1980 / Watson & Solomon 1997 / DCTune 全文 | 0 |
| Sarnoff 1997 ANSI 提交稿（33 页） | 0 |
| **DICOM PS3.14 全文** | **0**（`dither` / `halftone` / `banding` / `line art` / `binary` 各 0 次；`contour` 恰好 1 次） |

**但"沉默"不等于"没有边界"。能引用的是它们各自申明的标定域，而那些域把我们的内容排除在外。**

**这一条是最硬的**——HDR-VDP-3 论文 §3 逐字（**我自己下载 PDF、pdftotext 后读出的**）：

> **detection** — the detection task predicts the probability of detecting the difference between
> two images (single-valued) and was calibrated on the same datasets as HDR-VDP-2 — basic
> psychophysical detection and discrimination data for **Gabor patches, sinusoidal gratings, and
> discs**. **This task should provide better accuracy for simple stimuli, but potentially lower
> accuracy for complex images.**

HDR-VDP-2 自陈的标定刺激（子 agent 从 PDF 读出）：正弦光栅经高斯包络衰减，
背景亮度 0.02–150 cd/m²，频率 **0.125–16 cpd**，固定 93 cm 观看距离；
加上 ModelFest 的 *"43 small detection targets at 30 cd/m² uniform background"*。
它**自陈的主要局限与内容无关**——只说不管颜色、不管时间。

**⭐ 而最该带走的是这一句**（HDR-VDP-2 正文，讲高对比度强掩蔽区）：

> **The visual model that relies on a CSF alone would make worse predictions for strongly masked
> regions than the visual model without any CSF weighting.**

**这是第一方陈述：在强掩蔽（高对比、有结构）的区域，只做 CSF 加权比不加权更糟。**
论文并承认自己那版模型在那里 *"the reversal is exaggerated for the high masking region"*。

背后的物理是**对比度恒常性**：Georgeson & Sullivan, *Contrast constancy: deblurring in human
vision by spatial frequency channels*, Journal of Physiology 252(3):627–656, 1975。
近阈值时不同空间频率的感知对比差异很大，**远高于阈值时这种差异被抹平**。
**线稿整个活在被抹平的那一侧。**

同一组人在 FovVideoVDP（2021）里把这层意思讲得更直接，而且直接针对"拿它当选择判据"这件事：

> **threshold metrics do not produce discriminative results when comparing inputs that contain
> significant differences, and so can be unsuitable for use as an optimization criterion**

理由正是 *"the near-threshold CSF model cannot predict supra-threshold performance due to
**contrast constancy** [Georgeson and Sullivan 1975]"*。

**⭐ 掩蔽模型这一侧有一条现成的、逐字对得上的警告**——Andrew B. Watson, R. Borthwick,
M. Taylor, *Image quality and entropy masking*, Proc. SPIE 3016, Human Vision and Electronic
Imaging II, pp. 2–12, 1997, DOI `10.1117/12.274501`：

> If a model is calibrated with **simple, low entropy masks such as cosines**, and then applied to
> **complex high entropy masks** such as a novel "natural" image, **a failure of prediction can be
> expected to result.**

Watson 还直接点了 Daly 的名，说他调整通道内掩蔽幂次的做法
*"clearly cannot take into account the image-specific knowledge that the observer may have"*。

**几个把边界钉死的数字**（子 agent 从各自全文读出，**我未复核**）：

- Legge & Foley 1980（*Contrast masking in human vision*, JOSA 70(12):1458–1471,
  DOI `10.1364/JOSA.70.001458`）的掩蔽对比度最高 **51.2%**，刺激是 2.0 cpd 正弦光栅。
- Watson & Solomon 1997（JOSA A 14(9):2379–2391, DOI `10.1364/JOSAA.14.002379`）自陈
  *"the mask is a simple pattern such as a **sinusoid or Gabor function**… Considerations
  associated with **stochastically defined masks such as visual noise remain outside the scope**
  of the current paper"*；拟合的掩蔽对比度上限约 31.6%。
- DCTune 自陈 *"we consider **only masking within a block and a particular DCT coefficient**"*。
- Sarnoff JND 模型的空间拟合 *"was not extended beyond **15 cycles/deg**"*。

**半色调与抖动颗粒的局部对比度接近 100%，空间频率通常高于 16 cpd。
这个组合落在上面每一个标定集之外。**

> **⚠️ 三条对常见说法的更正**（子 agent 核出，我未独立复核）：
> Sarnoff JND 模型对应的是 **ANSI/ATIS T1.TR.75-2001** 与 **ITU-T J.144 附录 II（信息性，
> 非规范性）**，**不是** ANSI T1.801.03（那是 NTIA/ITS 另一组人的参数标准）；
> **DICOM PS3.14 并没有引用 Barten 1999 那本书**，它的 §8 参考文献只有两条 SPIE 会议论文；
> Barten 模型自陈的拟合有效上限是 **10³ cd/m²**，**低于** GSDF 宣称的 4000 cd/m² 顶。

### 这一支和半色调那一支，是 1992 年在同一间屋子里分的家

一个结构性的观察，值得记下来。**SPIE Vol. 1666**（*Human Vision, Visual Processing, and
Digital Display III*，1992）**同一卷**里同时收了：

- Daly 的 VDP（p. 2）——后来长成 VDP → HDR-VDP → ColorVideoVDP
- Barten 的 CSF 模型（p. 57）——后来长成 DICOM GSDF
- Analoui & Allebach 的 model-based halftoning（p. 96，DOI `10.1117/12.135959`）
- Pappas & Neuhoff 的 least-squares model-based halftoning（p. 165，DOI `10.1117/12.135965`）

**连续调那一支成了通用 IQA 与显示标准；二值那一支成了另一条小得多的
"HVS 低通滤波后算误差"的传统**（Pappas & Neuhoff, IEEE TIP 8(8):1102–1116, 1999,
DOI `10.1109/83.777090`；**Fredrik Nilsson, *Objective quality measures for halftoned images*,
JOSA A 16(9):2151–2162, 1999, DOI `10.1364/JOSAA.16.002151`**；Itoua et al. ISSPA 2010）。

**两支再没有合流。**而二值那一支到 2025 年仍在发论文说通用 IQA 指标搬不过来
（Displays 90:103165，见第一节）。

> Nilsson 1999 这一条**部分填上了我第一节留的那个洞**：它是"半色调客观质量度量"这一支
> 能追到的较早的一手期刊论文。**但 HPSNR / MPSNR 这两个具体名字的首创出处仍未确认。**

**但文献里有一个人正面回答过这个问题，而且答案是"别用通用度量"。**
Kite 论文第 6 章结论，逐字：

> Other visual quality metrics, such as that described by Lubin [81], **combine all image
> distortions into one figure**. However, as was shown in Chapter 3, halftoning by error
> diffusion is accurately modeled as a process which **sharpens an image and adds noise**,
> i.e., a process which introduces a small, predictable set of distortions. **Applying to
> halftones a quality metric that is designed for any type of image therefore seems
> unnecessarily complicated, and discards information about the process used to generate the
> image.** Furthermore, characterizing halftone sharpening independently is desirable, because
> of its highly subjective and viewer-dependent effect on the quality of an image.

同一章也承认了**自己那条路的缺口**：

> The linear contrast sensitivity function (CSF) that is used to weight the residual image is a
> **simple model of the human visual system**. A possible way to improve the correlation between
> the noise metric and visual quality would be to use a more detailed model, such as that
> described by Peli [80]. This model takes into account effects such as **local contrast and
> frequency masking**.

即：**纯 CSF 加权不含掩蔽，作者自己指出这是下一步。**

---

## 直接可用的，与只是背景的

全文唯一一处把外部现状对到我们处境上的地方。**这里不含任何建议，只标"对得上"与"对不上"。**

### 直接对得上我们三类劣化的

| 我们的现象 | 外部对应物 | 状态 |
|---|---|---|
| 第 3 类：文字与线稿边缘长毛刺 | 误差扩散的**边缘锐化**，有闭式模型：FS 的 `Ks = 2.00`，信号传递函数**直流增益 1、高频增益 4** | 定量、可算，1998 年的一手结果 |
| 同上 | libimagequant 的 edge/noise map 逐像素调抖动强度，注释直书 "Dithering on edges creates jagged lines" | 在产实现，源码可读 |
| 第 1 类：平滑区被撒颗粒 | CAMBI（banding 专用无参考指标，公开阈值"约 5 起惹眼"）——**但按构造丢掉纹理、对比度天花板只有 1 个灰阶，拿不到线稿页上用** | 在产实现，**适用域不覆盖我们** |
| 判据必须低通 | 半色调社区的 WSNR / MPSNR，独立得出同一结论；CSF 折算到 cycles/degree 需要观看距离 | 与 `ADR 0002` 同向，属**独立佐证** |
| 掩蔽加权那两个占位常数 | Peli 1990 的 local contrast / masking 模型，被 Kite 明确点名为"CSF 之上的下一步" | 有文献，无现成实现 |
| 同上——**而且有一条反向警告** | HDR-VDP-2 第一方陈述：**"只做 CSF 加权，在强掩蔽区比不加权更糟"**；Watson 1997 熵掩蔽："用余弦标定、拿去套复杂掩蔽，可以预期预测失败" | 一手陈述，方向明确 |
| 判据被拿来**在候选之间选**（不只是打分） | FovVideoVDP 第一方陈述：**阈值型度量在差异显著时"不产生有区分度的结果，因而可能不适合用作优化判据"** | 一手陈述 |
| "在边缘处削弱抖动"该怎么做 | Knox & Eschbach 1993：**阈值调制 ≡ 对预锐化/预钝化过的输入做标准误差扩散**——不必改扩散核 | 理论结果，1993 年 |
| 同上——**是否有先例** | Kodak US 5,034,990（1991，边缘处误差减半）、Sharp US 8,059,311（低位深设备上文字 snap 到实心不铺半调）、Xerox iGen 150（图像 180 lpi / 文字 250 lpi）、Gutenprint（按 ImageType 换抖动算法） | 35 年工程正统 |
| 真做了之后会遇到什么 | **交界处**才是难点：Kwon 2006 记录了切换造成的 boundary / dot-elimination 伪影，并给出 GDTR 渐变过渡带的现成参数表 | 一手论文，配方可读 |
| 文字区与图像区不能一把尺子量 | 屏幕内容 IQA（Yang/Fang/Lin 2015）把文字区与图像区分开加权 | 有学术工作 |
| 2bit / 4bit + FS 这两个候选本身 | **multitoning 的中间调 banding**（Faheem/Arce/Lau 2002）：N 级量化器直接替换二值量化器，恰好落在格点上的平坦区完全不抖、邻域却有稀疏点 | 一手论文，机理明确 |
| 面板灰阶数是硬上界（`ADR 0003`） | E Ink 波形规格书：16 级是 4-bit 波形的原生态；ED060SC7 datasheet："灰阶数由控制器与波形文件决定" | 第一方规格书（NDA 材料） |
| **网点区怎么比**——判据在网点上读数异常（见《掩蔽加权在大面积规则网点上可能反向》） | CUHK 两组人各自独立得出"**不要在像素栅格上原位对齐着比**"：一路投影到 ScreenVAE 潜空间后逐像素度量才有效，一路用最佳位移匹配 | 有学术工作，无现成实现 |
| 线稿那一半怎么比 | SIGGRAPH Asia 2020 的 sketch cleanup 基准"**为缺乏更好的替代**"选了 Chamfer 距离；IOU 对局部错位极敏感、Hausdorff 被离群点主导 | 有基准，结论是"没有更好的" |
| 判据的盲区有多深 | Xerox JBIG2 换数字事件：二值有损编码可在任何平滑逐像素度量上拿接近满分而语义已错 | 已发生的工业事故 |

> **这张表的第 2 行已经实测过了，结论是"对得上但救不了"**：把 libimagequant 那套边缘门控
> 逐字实现跑在武器 v01 上，门在被抱怨的那几页**基本不关**（增益均值 0.96/0.96/0.81），
> 暗场颗粒只压掉 11%~24%。原因是构造上的：门量二阶差分，而抱怨是**平坦暗块里的颗粒**，
> 平坦正是门判"该抖"的条件。**它管第 3 类，不管第 1 类。**
>
> **同一次实测还翻掉了这批劣化的前提**：那几页出自**成品再转**的素材，
> 换到同一卷的原档，判据 95% 的页直接判 4bit，2bit+FS 根本进不了决赛。
> 数字见 `docs/measurements.md` 的《边缘门控抖动救不了那三页》。

**另一条值得单独记的**：我们候选集里的 2bit+FS 与 4bit+FS 属于 **multitoning**，不是二值半色调。
大部分被引用的半色调文献讲的是 1bit，**结论不能直接搬**；multitoning 是一支独立的文献，
而且它有一个二值半色调不存在的伪影（见第二节）。**引用半色调文献时要先分清是哪一支。**

**最值得单独记一句的**：Kite 那条"误差扩散 = 锐化 + 噪声整形"的分解，与我们的
"低通项 + 颗粒项"是同一个形状的两项分解——**但我们那两项对应的是他的"噪声"那一半，
他的"锐化"那一半在我们的判据里没有对应项**。这正好是第 3 类劣化漏掉的位置。
这是本次调研在方法论上最直接的一条对照。

### 只是背景的

- **Ostromoukhov 变系数、蓝噪声掩模、结构感知半色调**——它们改的是抖动算法本身。
  `ADR 0002` 已记明判据"不能用来挑抖动算法"，这几条因此够不到我们眼下的问题。
- **libaom `--tune=vmaf`、libjxl 的 Butteraugli 闭环、Guetzli**——证明"按指标闭环搜参数"
  是成熟工程，但它们的判据、代价与吞吐都不在我们的量级（Guetzli 自陈 "extremely slow"）。
- **JPEG XT / SHVC 的位深可分级**——是分层传输工具，不含判定机制。
- **面板驱动的 FRC / 空间抖动**——目标位深由面板定死，自适应的是噪声整形，不是位深。
- **KCC / calibre / ComicRack 的做法**——没有可借鉴的判据。它们的价值是**反证**：
  这件事至今没人做对，而且 KCC #980 与 calibre 那个没接线的 `eink_dither_image()`
  说明连意识到问题的人也停在了那里。
- **DICOM GSDF 的 1023 JND**——它是"灰阶够不够"这一问最硬的公开标准，但标定刺激是
  4 cycles/degree 正弦光栅，与我们的输入相距极远。**它是背景，不是可搬运的数**。
- **EPDC 驱动与 PxP 的硬件抖动、E Ink 的更新模式表**——那一层在**我们的输出之后**：
  我们写出的是文件，抖不抖由固件在显示时再决定一次。它解释了 `ADR 0003` 那条
  "多余精度没到过眼睛的前提是显示固件不再自行抖动"为什么至今存疑，**但它不是我们能控制的旋钮**。
- **e-paper 图像准备的学术工作（Kao / Qin / Zhong 综述）**——它们服务的是**驱动波形与功耗**，
  施加对象是面板控制器，不是离线转出的文件。三条结论里只有"EPD 反射率动态范围窄"
  与我们的观感沾边，而那一条我们没有量过。

### 一条需要单独标出的口径问题

**SSIMULACRA2 排序失效的原因，不是"没标定过抖动"**——TID2013 第 22 类就是
"Image color quantization with dither"，KADID-10k 也有 quantization。
原因是**内容**：CID22 的 250 张原图里非照片只有 25 张（10%），且全是 512×512 彩色图；
TID2013 的参照图是 Kodak 自然照片。**四个数据集里没有一张网点扫描页、没有一张低位深线稿。**

任何"它没见过抖动"的说法都是错的，引用时要按这个口径写：
**它见过抖动这种失真，没见过我们这种内容。**

**但也别把这条推过头。**要诚实的话还有三点：CID22 那 25 张非照片图是作者**自己标了类别的**，
不是被忽略的；所有图都是 **512×512 下采样**过的，硬 1 像素边缘与抖动结构在被人看到之前
就已经被这一步抹掉了；而且——

> **CID22 论文的 Table 2 只报了全数据集上的指标相关性，全文没有任何按类别拆分的
> 指标相关性数字。** 也就是说：**"SSIMULACRA2 在 diagram-chart 或 illustration-logo-text
> 上相关性多少"这个数，公开文献里根本不存在。**

所以准确的表述是"**落在标定域之外**"，不是"**已被证明在这类内容上失效**"——
后者需要一个没人发表过的数。我们自己那两页读数是现有的唯一证据，样本量是 2。

**同一个口径错误在 CAMBI 上会犯第二次，而且方向相反。**自然的猜测是"CAMBI 大概排除了抖动内容"
——**错**。CAMBI 是**为抖动设计的**（用 2×2 低通去平滑抖动后再检测 banding）。
它排除我们的理由是另外三条：**按构造丢掉纹理**（线稿全是高梯度）、
**对比度天花板只有 1 个 8-bit 灰阶**、验证域只有 SDR 自然视频的大片平滑区。
**"它见过抖动，但它按定义看不见线稿。"**

**这两处合起来是同一条教训**：判断一个指标能不能用，要看**它标定在什么内容上**，
不是看它见过哪些失真类型。

---

## 没查到的

诚实清单。**"没查到"比编一个看似合理的答案有用。**

1. **HPSNR 与 MPSNR 的原始出处**。二者的定义我转述自 Li et al. 2020 那篇综述的检索摘要；
   **MDPI 与 ScienceDirect 都 403 拒绝抓取，综述正文我没读到**，原始定义者是谁没有追到。
   Kite 与 Mitsa 那两条是我读过原文的，可以当准。
2. **文档图像质量评估（DIQA）的一手论文**。这一支存在（有 SmartDoc-QA 之类数据集），但
   **两轮检索都没有追到一手论文**，也没有确认它有没有面向"观感"而非"OCR 可识别性"的分支。
   相关的 **DIBCO 竞赛报告是付费墙**——"DRD 是 DIBCO 的标准度量"这一说法只有二手引用支撑。
   - **SIQAD（Yang/Fang/Lin 2015）正文没读到**（闭放获取）。题录与摘要经 PubMed 记录核实，
     但**没有一句出自该文正文的逐字引文**；本文用的"通用度量不够用"引文来自 Wang 2016 与
     Min 2021。**"SPQA" 这个缩写在我读过的一手文献里从未被定义过**——方法与分区设计可信，
     这个**名字**不可信。ESIM / GFM 正文同样未读到（闭放获取，作者站 PDF 404）。
   - **`Sakuga-42M`（arXiv:2405.07425）已被 arXiv 管理员撤下**（提交者当时无权同意许可）。
     它也不是 IQA 数据集。**要么带这个说明引，要么别引。**
3. ~~蓝噪声掩模在硬边缘上与误差扩散的对比~~——**已补上**（Ostromoukhov 2001 与 Pang 2008
   两篇一手论文，口径一致：蓝噪声更糊、误差扩散更锐但出乱子）。仍**没有**定量的边缘保真对比。
4. ~~RIP / 打印机厂商的一手文档~~——**已补上**（Xerox iGen 150 手册的 Object Oriented
   Halftoning，180 lpi 图像 / 250 lpi 文字）。但**只证实了 Xerox 一家**；
   HP / Canon / Ricoh / Epson 的官方文档**一份都没取到**，不能说"厂商普遍这么做"
   （Xerox Nuvera 本身就是反例，它按打印队列选屏、不按对象类型）。
5. **"抗锯齿文字在多少灰阶下开始活不下来"的任何量化工作**。基本空白，两轮独立检索都是这个结论。
   最接近的 US 8,059,311 只定性说浅灰上会 "broken shapes and missing lines"，**不给阈值**。
   要这个数字只能自己测。
   - ~~两件专利的逐字引文我没能复核~~——**已复核**（2026-09-08，浏览器直读 Google Patents；
     `urllib` 直取仍返回 503，这就是当初取不到的原因）。US 5,034,990 与 US 8,059,311 的
     全部逐字片段、受让人、发明人、日期均对上，并补出三条收紧适用性的事实，见第二节。
   - ~~Kiyotomo et al. 2017~~——**已取到全文并通读**（IS&T 开放下载端点），见第二节
     《多级 ＋ 边缘保持：唯一一篇正对我们候选档的论文，但它治的不是我们的病》。
   - 仍**未打开就不该引用**的线索：Li 2006 *Edge-directed error diffusion halftoning*
     （IEEE SPL 13(11):688–690，仅索引）；一批 Xerox 专利号（US 5,687,303 / 6,006,013 等，
     **一件都没打开**）。
6. ~~CAMBI 的适用范围是否排除已抖动内容~~——**已补上，而且答案与直觉相反**：
   它不排除抖动，是**为抖动设计的**；排除我们的是"按构造丢纹理"与"对比度天花板 1 个灰阶"。
   仍未确认的是：`cambi.md` **确实没有 Scope 与 Limitations 小节**（这一条我自己看过），
   而三条逐字引文出自 arXiv 版与 TechBlog，**我未复核**。
7. ~~HDR-VDP-2 / -3 的标定域与外推警告~~——**已补上，且是全文里最硬的一条**
   （HDR-VDP-3 §3 的 "Gabor patches, sinusoidal gratings, and discs… potentially lower accuracy
   for complex images"，我自己复核过）。仍**没有**任何一处说它对近二值输入无效——**沉默是结论**。
   - **Daly 本人对 VDP 适用范围的陈述**：**仍是洞，但比原先小了一点。**
     浏览器取到了 SPIE 1666 那篇的**摘要全文**（<https://www.spiedigitallibrary.org/
     conference-proceedings-of-spie/1666/1/10.1117/12.135952.short>），逐字：
     *"The visual model, which is the central component of the algorithm, is comprised of three
     parts: an **amplitude nonlinearity**, a **contrast sensitivity function**, and a **hierarchy
     of detection mechanisms**."*——这坐实了本文《谱系》里那条流程的前三段，
     **且摘要里没有任何适用范围或内容类型的限定语**。**正文仍在付费墙后，一句正文都没读到。**
     另有一条准一手材料：HDR-VDP 项目自建的 VDP'93 复现说明
     （<https://hdrvdp.sourceforge.net/reports/2.0/visibility/vdp_daly_blackwell/index.html>）
     声称"based on **corresondence with the author**"，并记下三处偏离——
     掩蔽斜率对所有频带改用 **0.9**（拟合掩蔽数据集所得，非书中默认）、
     cortex 频带内的对比度取**全局**而非局部、
     以及 **phase uncertainty "is not mentioned in the '93 book chapter, but is described in the
     patent application"**（该复现改用 Lukin 2009 的复数 cortex 变换替代）。
     **即：书章里那一版与后来被当作"VDP"引用的那一版并不完全是一回事。**
   - **Lubin 1993 / 1995 两章、ANSI/ATIS T1.TR.75-2001、Barten 两篇原文**均为纸本或付费墙，
     未取到。Sarnoff 的引文出自 1997 年的 ANSI 提交稿，不是那两章。
   - **Roberts 1962 正文**未读（IEEE Xplore 挡住），只有摘要；其 MIT 学位论文**未证实**，不要引。
     Goodall 1951 是读了全文扫描的（由子 agent 读出，我未复核）。
8. **Floyd & Steinberg 1976 原文的一手核对**。不在 Crossref（早于 DOI），著录按通行写法记，未核。
9. **ISO/IEC 29170-2**（"visually lossless" 的 flicker 评测规程）。iso.org 被 Cloudflare 挡住，
   **标准号、题名与年份我都没有一手确证**，只作为线索记在这里。
10. **Celebi 2023 那篇颜色量化综述里 "the number of distinguishable colors" 一节**。
    Springer 付费墙，只拿到章节标题。
11. **Netflix 那两篇工程博客原文**（per-title 2015、dynamic optimizer 2018）。Medium 返回 403，
    因此正文只引了同作者经同行评审的 SPIE 论文（Katsavounidis & Guo, Proc. SPIE 10752,
    107520Q, 2018, DOI `10.1117/12.2322118`）。
12. **Cloudinary `q_auto` 的实际搜索算法**。两篇一手博客只到原则层面，趟数、是否二分、
    阈值全部未公开。
13. **第四节的行号哪些复核过、哪些没有**——见下面第 20 条，那里列全了。
14. **`Modern Digital Halftoning` 的年份**。Crossref 记的是 2018（重印/第 2 版电子版），
    通行著录写 CRC 2008。**两个年份我没有对齐**，引用前请自行确认版次。
15. **E Ink 官网上没有公开的图像准备 / 抖动应用笔记**。开发者资源页 SSL 握手失败抓不到，
    其余资料都在 NDA 后面。第四节引的那份波形规格书是**流到公开镜像的 NDA 文档**。
16. **Pearl 世代的专属 datasheet 逐字表述**没拿到（ED060SCF 无公开 PDF），
    用的是同代 ED060SC7 的 "2-16 gray levels (1-4 bits)"。
17. **PxP 的 `quant_bit` / `NUM_QUANT_BIT` 确切语义**。NXP 文档说 "quantize down to, valid 1~7"，
    内核 LUT 分支却是 `<2 / <4 / else`，FBInk 对 GC16 传 7（7 位 = 128 级，与 16 级对不上）。
    FBInk 作者自己写的是 "what **appears to be** sane values"。**存疑，寄存器定义没抓到。**
18. **Onyx BOOX 的 `UpdateMode` 枚举本体**是闭源 AAR，序数值不可得；
    **Amazon 没有任何公开的第一方 SDK 文档**描述 Kindle update mode，模式表只能从按 GPL
    公布的内核头拿到。**libremarkable 是逆向产物，模式↔编号映射不可信，不要当规范引用。**
19. **"256 级灰阶"那类厂商宣传的真伪**。只找到新闻/博客口径的反驳（Good e-Reader），
    **不引任何一手技术来源，低置信度**。方向上与 E Ink 2025 年"扩色靠波形不靠抖动"的
    新闻稿一致，但这不构成证据。
20. **第四节的复核分布不均**。**我自己复核过**：E Ink `800-1101 REV01` 的全部逐字引文
    （§1、§2.2、§2.3.2–2.3.8）与 "dither/halftone 计数为 0"、FBInk 的 `eInkFGCMap[16]`、
    KCC 的 `quantizeImage` 与 `README.md:84-87`、Pillow 的 `dither` 默认值、KCC #980、
    calibre 的四处关键行。**未复核**：E Ink 专利要点与受让人、ED060SC7 与 Broadsheet 规格书、
    内核 EPDC 与 PxP 的行号、KOReader / FBInk 的行号、e-paper 论文题录（Kao / Qin / Zhong）。
    引用未复核的那些之前建议再打开一次。
21. **本次调研的 WebSearch 配额在中途用尽**（200/200），后段只能用直接抓取。
    有若干线索因此没有展开，不排除有遗漏。
