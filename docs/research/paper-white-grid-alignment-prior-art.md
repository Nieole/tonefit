# 量化前把纸白对齐到输出格点：外部现状调研

调研日期 **2026-09-09**。本文只记录**仓库之外**的现状与来源，不含本项目的决定，也不提任何建议。
项目自己的决定在 `docs/adr/`，项目自己的数字在 `docs/measurements.md`——本文引用它们的小节名，不复制。

同目录的 `halftone-metrics-and-dither-prior-art.md` 是上一轮调研，谈的是判据与抖动本身。
**本篇只谈一件事：把源的取值挪到输出格点上，这件事外面有没有人做过、怎么做的、代价是什么。**

## 怎么读这一篇

第一至第五节对应提出的五个问题，前面另加一节《零》交代格点本身的出处。
每节第一句是现状判断，只有三档：

- **有成熟方案**——有现成实现或标准，拿得来用
- **有学术工作但未落地**——文献解决了，没有可用实现
- **基本空白**——找不到

每条实质结论后面跟来源。**凡是我没查到确证的，写在《没查到的》一节，不在正文里编圆。**
凡引用源码，给仓库、文件、行号与抓取时的 commit；行号会随上游漂移，函数名是稳定引用。

**复核口径**：本轮由六个子 agent 分头取材，我逐条挑关键结论自己打开源码/原文复核。
带 **✅ 我复核过** 的是我亲自逐行对过的；没带的是子 agent 读出、我未复核的，引用前请再打开一次。

末节《直接可用的，与只是背景的》是全文唯一一处把外部现状对到我们处境上的地方。

## 五个判断，一眼看完

| # | 问题 | 判断 |
|---|---|---|
| 一 | KCC 的色调处理做了什么 | **它十二年来一直在做一件很像的事，而且默认开着**——但对齐的是「全页最亮像素」不是纸白，且**没有一行提到格点**；KCC 自己试过更贴近的做法并**写下理由放弃了** |
| 二 | 其它成熟工具怎么做 | **色彩管理侧有成熟方案，且把理由写进了源码注释**（lcms2 的 scum dot 修正）；**扫描侧是标准步骤，但目标值分成两派**；**漫画转换侧基本空白** |
| 三 | 文献里有没有名字 | **基本空白**——「格点」有五个现成术语，「把源搬到格点上」一个都没有；而最相关的那一支文献**方向正好相反** |
| 四 | 代价与已知坑 | **有一手证据，而且最硬的一条是反对的**——FADGI 把这个操作直接写成扫描仪缺陷；**但 Metamorfoze 自己划了界**：那些规范管的是归档母版，并明说为输出目的做 clipping 可以有用 |
| 五 | 逐页自适应 vs 全局固定 | 估计纸白**有成熟方案**；但「满版无纸边页」**被检测过，从没被解决过**——五个工具检测到它之后一律是放弃 |

**五条贯穿全文、值得单独记住的更正**：

1. **同一个物理事实，文献只从相反的那一侧写过。**「源值落在格点上 ⇒ 那一档不撒点」——
   多级半色调那一支在**渐变区**里把它当成 **banding 病**，解法是**往那一档注入点**；
   而我们在**平坦纸底**上把它当成药。**「离格 ⇒ 平坦白底必须撒点」这个方向没人单独写过**（第三节）。
2. **KCC 的 `optimizeImage(gamma)` 已经不存在了**——2025-07-09 拆成了 `gammaCorrectImage()` 与
   `autocontrastImage()` 两个互不相干的步骤。上一轮那条线索过期了（第一节）。
3. **calibre 的漫画管线默认就在跑 normalize**，注释写着 `# Do the Photoshop "Auto Levels" equivalent`。
   上一轮只记了 `--colors` 与 `eink_dither_image`，**把这条漏了**（第二节）。
4. **ImageMagick 官方自己写下：我们想要的那个算子不存在。**
   逐字 *"While a perfect 'white-point only' or 'black-point only' version is posible it has not been
   implemented at this time."*（第二节）
5. **这个现象在 ImageMagick 官方文档里有名字**：**E-Dither Pixel Speckling**。
   而 IM 给的处方是换有序抖动，**不是改输入值**（第二节）。

---

## 零、格点本身：这不是我们定的，是 PNG 规范定的

**判断：有标准文本，且它同时写下了抖动那条权衡。**

来源：*Portable Network Graphics (PNG) Specification (Third Edition)*，
**W3C Recommendation 24 June 2025**，<https://www.w3.org/TR/png-3/>。
**✅ 我复核过**——我下载整页 HTML 逐段读的，节号逐一对过。

**§11.3.2.4 sBIT Significant bits** 那一段是规范对格点的定义：

> To simplify decoders, PNG specifies that only certain sample depths may be used, and further
> specifies that **sample values should be scaled to the full range of possible values at the
> sample depth**. The sBIT chunk defines the original number of significant bits (which can be
> less than or equal to the sample depth).

**§12.4 Sample depth scaling**（编码器建议）给出算式：

> The most accurate scaling method is the linear equation:
> `output = floor((input * MAXOUTSAMPLE / MAXINSAMPLE) + 0.5)`
> where the input samples range from 0 to MAXINSAMPLE and the outputs range from 0 to MAXOUTSAMPLE
> (which is 2^sampledepth − 1).

> A close approximation to the linear scaling method is achieved by **"left bit replication"**,
> which is shifting the valid bits to begin in the most significant bit and repeating the most
> significant bits into the open bits.

**即 `255/(2ⁿ−1)` 是规范文本里的量，不是我们的推导。**

同一节还有一句与「对齐能省体积」同向的：

> When scaling up source image data, it is recommended that the low-order bits be filled
> consistently for all samples; that is, the same source value should generate the same sample
> value at any pixel position. **This improves compression by reducing the number of distinct
> sample values.** … For example, an encoder might instead **dither the low-order bits, improving
> displayed image quality at the price of increasing file size.**

**"抖动换体积"这条权衡，规范自己写下了。**

**§13.12 Sample depth rescaling**（解码器建议，即降位深）对「8bit 怎么降到 4bit」的回答只有一句：
同一条四舍五入的线性映射。**不提抖动，不提平坦区。**

**§13.11 Truecolor image handling** 里有一句限定语值得单独记：

> A simple, fast method for color quantization is to reduce the image to a fixed palette. …
> **For photograph-like images, dithering is recommended** to avoid ugly contours in what should be
> smooth gradients; however, **dithering introduces graininess that can be objectionable**.

**规范推荐抖动时限定了内容类型，而我们的内容不在那个限定里。**

> **对照**：这与上一篇《SSIMULACRA2 的标定域里没有这类图》是同一类事——
> 推荐语带着一个内容前提，前提不成立时推荐不自动成立。

---

## 一、KCC（Kindle Comic Converter）的色调处理

**判断：它一直在做一件很像的事，默认开着，但对齐的不是纸白；而且它自己试过更贴近的做法并写下理由放弃了。**

抓取基线：`ciromattia/kcc` @ `5b67d4019eb9074fc8a8278d124700bfd03f373d`（2026-09-06，v11.2.0）。
`python-pillow/Pillow` @ `6b5a7dbef1a1abfd8febdfb0df9917cd669e3df4`（2026-09-08）。

### 先更正上一轮的线索

**`optimizeImage(gamma)` 今天已经不存在。** 2025-07-09 的 commit
`a79c740387b17ccab790c9b5722c28b88647a4cc`（*"don't autocontrast color content (#1021)"*）
把它拆成 `gammaCorrectImage()` 与 `autocontrastImage()` 两个**互不相干**的步骤。

**`if gamma == 1.0` 提前返回仍在，41 条 profile 的 gamma 也确实全是 1.0——但这是 2025-10-20 才成立的。**
在那之前 profile 默认 gamma 是 1.8，改动来自 PR #1030，标题逐字
`disable default gamma correction/darkening of 1.8 (1.0 is disabled) (#1030)`。
**所以「gamma 那一步是死代码」只在最近一年成立。**

### autocontrast 今天跑不跑：**跑，默认开，而且有两条路径**

**✅ 我复核过**（逐行对过 `image.py:462-478`、`image.py:606-612`、`comic2ebook.py:1556-1563`、
`comic2ebook.py:744-762`）。

**路径 A：正文页**，`comic2ebook.py:751` 的 `img.autocontrastImage()`。函数全文（`image.py:462-478`）：

```python
def autocontrastImage(self):
    if self.opt.webtoon:
        return
    if self.opt.noautocontrast:
        return
    if self.color and not self.opt.colorautocontrast:
        return

    # if image is extremely low contrast, that was probably intentional
    extrema = self.image.convert('L').getextrema()
    if extrema[1] - extrema[0] < (255 - 32 * 3):
        return

    if self.opt.autolevel:
        self.autolevelImage()

    self.image = ImageOps.autocontrast(self.image, preserve_tone=True)
```

- **`--noautocontrast` 默认 `False`**（`comic2ebook.py:1560`）——**即 autocontrast 默认开**。
- `self.color` 是**源图**是否含色，与是否彩色输出无关。黑白漫画扫描页第三道守卫不拦。
- **`255 - 32*3 = 159`**：全图极值差小于 159 就整页跳过，注释理由逐字
  *"if image is extremely low contrast, that was probably intentional"*。
- **只传了 `preserve_tone=True`，`cutoff` 取默认 `0`。**

**路径 B：封面**，`image.py:608` 的 `ImageOps.autocontrast(self.image, preserve_tone=True)`。
`Cover.process()` 里**没有任何守卫**——不看 `noautocontrast`、不看 `color`、不看 extrema。
而且 `image.Cover(...)` 的调用点（`comic2ebook.py:1918`）在 `if options.noprocessing:` 判断之前，
**所以连 `-n/--noprocessing` 也拦不住封面的 autocontrast**。

> 这条子 agent 判为「一个尚未被报告的 KCC bug」。与本调研无关，但它说明这条链上没人在系统性地审视色调。

管线顺序（`comic2ebook.py:746-760`，**✅ 我复核过**）：
`gammaCorrectImage()` → `convertToGrayscale()` → **`autocontrastImage()`** → `resizeImage()` →
`optimizeForDisplay()` → （`--forcepng` 才走）`quantizeImage()`。

### `ImageOps.autocontrast` 到底做什么

Pillow `src/PIL/ImageOps.py`，docstring 逐字：

> Maximize (normalize) image contrast. This function calculates a histogram of the input image
> (or mask region), removes ``cutoff`` percent of the lightest and darkest pixels from the
> histogram, and remaps the image so that **the darkest pixel becomes black (0), and the lightest
> becomes white (255)**.

核心映射（`cutoff=0` 时）：`lo` = 最低非零 bin，`hi` = 最高非零 bin，
`scale = 255.0 / (hi - lo)`，`offset = -lo * scale`，逐值 `int(ix*scale + offset)` 再 clamp。

**`preserve_tone` 只决定直方图从哪来**（`True` 时用 `image.convert("L").histogram(mask)` 算出一条
LUT 复制到三通道，`False` 时每通道独立拉伸）。**对已经是 'L' 的图它是完全的 no-op**——
KCC 灰度路径正是这种情况。

> **追它的身世得到一条反直觉结论**：`preserve_tone` **不是为高光 clip 加的**。
> PR #5350（elejke，2021-03-21 开 / 03-29 合并）的理由逐字：
> *"autocontrast function changes the tone of the image **because of the usage of separate
> histograms for each channel**."* 它解决的是逐通道拉伸导致的色相偏移。
> 前身 PR #1246（broxeph，2015）的描述里写的是 *"Mimics Photoshop autocontrast functionality"*。
> （这两条 PR 的逐字由子 agent 读出，**我未复核**。）

### 所以 KCC 事实上在做「把纸白拉到 255」吗

**是，但只在「页内没有比纸白更亮的像素」时成立，而且它顺带改了全部中调。**

**三点要害**（推理链由子 agent 给出并用本机 Pillow 实跑验证，**那组实测数字不属于本文范围**，
此处只记结构性结论）：

1. **它对齐的不是纸白，是「全页最亮像素」。** 二者重合时白点确实落到 255；
   但只要有一个更亮的孤立像素——JPEG 振铃、扫描高光、白色页码框、一颗坏点——上端就被钉死，
   纸白原地不动。**这是个非鲁棒的白点对齐。**
2. **它是全域增益，不是白点吸附。** `scale = 255/(hi-lo)` 乘遍所有灰阶，黑点不为 0 时中调反而被压。
3. **它在 resize 之前跑**，之后才是量化。

### 是有意的还是副作用：**副作用**

`git log -L` 追这一行的完整血统，引入那一次是
**`f231e9112a2006de619a4cfb20dd9a2db566851e`，2013-01-17，Ciro Mattia Gonano**，
commit message 逐字 `Added a bunch of optimizations (autocontrast, page centering)`。
**整个改动只有一行**：

```python
+    def optimizeImage(self):
+        self.image = ImageOps.autocontrast(self.image)
```

裸调用，无参数，无注释。**十二年里触碰这一行的每一次提交都在修别的东西**
（float 崩溃、彩色通道串扰、`preserve_tone`、低对比页豁免、封面、PDF 源）。
**没有一条 message 或 PR body 提到「对齐输出格点」或「减少平坦区抖动」。**

> **与上一篇《转换层：判据存在过、正确过，15 天后被一次无声的提交翻掉》对照读**：
> 那里说的是抖动开关的默认值被无声翻转。这里是另一件事——
> **autocontrast 从来就没有过判据，它是 2013 年一次「一堆优化」里顺手加的一行。**

### KCC 自己试过最接近的方案，并写下了放弃的理由

**这是本节对我们最有价值的一条。** **✅ 我复核过**（GitHub API 直取 issue 正文）。

**issue #474 *"How best to implement ImageOps.autocontrast(cutoff) with gamma correction"***，
作者 **axu2**（KCC 主要维护者本人），2023-01-31 开，**同日关闭**。正文逐字：

> Basically, it will clamp anything close to black to black and same for white. **Making close to
> black pixels black on e-ink screens looks really nice in my basic testing.**
>
> My basic testing thought adding just 5% was good for normal B&W pages but maybe it should be
> parametrized based on image.
>
> EDIT: Further testing indicates **this would have to be a custom value on a page by page basis,
> so not easy.** Probably a lot more work for not much more gain.
>
> EDIT2: Can make some images look weird if it's already mostly black. **Definitely way more
> trouble than it's worth.**

**2025 年重提**（issue #994 *"autocontrast cutoff param"*，axu2，理由逐字
*"for cases when the peak black level isn't the lowest black level"*），
最终只落地成 **`--autolevel`——只管黑点**。

`autolevelImage()`（`image.py:480-496`，**✅ 我复核过**）：

```python
h = img.histogram()
most_common_dark_pixel_count = max(h[:64])
black_point = h.index(most_common_dark_pixel_count)
bp = black_point
img = img.point(lambda p: p if p > bp else bp)
```

**取最暗 64 个 bin 里的众数当黑点，把更暗的全部抬平。**
在 autocontrast 之前跑，效果是让这个众数成为新的 `lo`，随后被拉到 0。

**这是黑端的「众数吸附」。白端至今是空的。**

### 拉白点的害处有用户侧报告

**✅ 我复核过**。**issue #511 *"Add option to (really) disable autocontrast"***，
maximelenfant，2023-05-10，正文逐字：

> Everything is in the title, I think it would be nice to be able to turn off this processing as
> **some images that don't have pure white / black will have their dynamic range stretched and
> that s not what everyone would like.**

报告附了 before/after 两张截图。**「不含纯白纯黑的图会被拉伸」——这正是离格纸白的情形。**

它与 #356（*"Image-Out is darker than Image-Input"*，2020）、#731（2024）一起，
由 **PR #1128**（axu2，2025-10-21 合并）修掉，PR body 逐字（**✅ 我复核过**）：

> By default, KCC autocontrasts only BW pages. **Any pages with exceptionally low contrast will be
> skipped, since that was likely intentional.** You can disable autocontrast entirely.

**这就是那道 159 守卫的来历。PR 里没有给这个阈值任何实测依据。**

### 与色调有关的全部开关

**✅ 我复核过 `--gamma` / `--autolevel` / `--noautocontrast` / `--colorautocontrast` / `-c` 五条。**

| 开关 | 默认 | help 逐字 |
|---|---|---|
| `-g, --gamma` | `0.0`（→ 取 profile 值，全是 1.0） | `Apply gamma correction to linearize the image [Default=Auto]` |
| `--autolevel` | `False` | `Set most common dark pixel value to be black point for leveling.` |
| `--noautocontrast` | `False`（**即 autocontrast 开**） | `Disable autocontrast.` |
| `--colorautocontrast` | `False` | `Autocontrast color pages too. Skipped for pages without near blacks or whites.` |
| `--forcepng` | `False` | `Create PNG files instead JPEG for black and white images` |
| `--noquantize` | `False` | `Don't quantize to 16 color PNG` |

**`-c` 是 `--cropping` 不是 `--cutoff`**（默认 `"2"`）。
**KCC 没有任何 cutoff / white point / contrast / brightness 开关；唯一的 level 类开关只管黑点。**

`--gamma` 的帮助文本至今写着 *"linearize the image"*——**上一篇记的那句「指向不再发生的行为的化石」仍然成立。**

### KCC 打的靶就是我们的格点

**✅ 我复核过**：`image.py:67-84` 的 `Palette16` 是 `0x00, 0x11, 0x22, …, 0xff`，
即 `17k = 255k/15`——**4bit 格点一字不差**。
`quantizeImage()`（`image.py:501-508`）**没有传 `dither=`**，因此走 Pillow 默认的 Floyd–Steinberg。

### 逐页还是逐卷：**逐页，每页独立算直方图**

**✅ 我复核过**。`comic2ebook.py:680-696` 是一文件一条 work、进程池并行
（`Pool(maxtasksperchild=100)`），`imgFileProcessing` 每次新建 `ComicPage` 对象。
`autocontrastImage()` 里两处统计（`getextrema()` 与 `autocontrast` 内部的 `histogram()`）**都只看本页**。

**没有卷级 LUT、没有全书统计、没有缓存、没有跨进程共享状态。**

> **一条方向相反的推论值得记**：autocontrast 把每页最亮值钉到 255，
> 恰好让**大多数**页的白底更一致，所以它不表现为闪烁；
> 被用户注意到的是反方向（低对比页被整页拉开），即 #356 / #511 / #731。
> 子 agent 按 `white` / `washed out` / `flicker` / `inconsistent` / `page to page` / `banding`
> 等词逐条搜过 KCC 的 issue，**没有任何一条报告「白底变灰」或「页间白底不一致」**。

---

## 二、其它成熟工具在同一件事上怎么做

**判断：色彩管理侧有成熟方案且写了理由；扫描侧是标准步骤但目标值分成两派；漫画转换侧基本空白。**

### 2.1 色彩管理：lcms2 的 scum dot 修正——**与本提案机制完全同构的在产实现**

**这是本次调研在工程侧最实质的发现。** **✅ 我复核过**（逐行对过下列每一处）。

仓库 `mm2/Little-CMS` @ `ab329ad5ce09dbb1f3547b6c126031aca606eb42`（2026-09-08）。
Little-CMS 是 Ghostscript、cups-filters、GIMP、Krita 的默认 CMM。

**标志位定义**，`include/lcms2.h:1757`：

```c
#define cmsFLAGS_NOWHITEONWHITEFIXUP      0x0004    // Don't fix scum dot
```

**函数注释**，`src/cmsopt.c:563`：

```c
// Locate the node for the white point and fix it to pure white in order to avoid scum dot.
static
cmsBool FixWhiteMisalignment(cmsPipeline* Lut, cmsColorSpaceSignature EntryColorSpace, cmsColorSpaceSignature ExitColorSpace)
```

**"fix it to pure white in order to avoid scum dot"——scum dot 就是本该空白的纸上被撒出的孤点。
这就是我们说的「平坦白底上误差扩散必须撒点」，一个成熟 CMM 把它写在源码注释里。**

它做的事：取入口色空间白点端点，推过整条 LUT，与出口白端点比；不相等就**改写 CLUT 里那个格点**。

**而它带了三条「何时不该对齐」的判据，三条都值得记**：

1. **离格量太大就不修**，`src/cmsopt.c:548-560`：
   ```c
   if (abs(White1[i] - White2[i]) > 0xf000) return TRUE;  // Values are so extremely different that the fixup should be avoided
   ```
2. **不在格点上就不动**，`src/cmsopt.c:497-500`（另有 `:518-520`、`:533` 两处同样的判断）：
   ```c
   if (((px - x0) != 0) || ((py - y0) != 0) ||
       ((pz - z0) != 0) || ((pw - w0) != 0)) return FALSE; // Not on exact node
   ```
3. **absolute colorimetric 下显式关掉**，`src/cmsopt.c:801-806`（`:1229-1234` 同）：
   ```c
   // Don't fix white on absolute colorimetric
   if (Intent == INTENT_ABSOLUTE_COLORIMETRIC)
       *dwFlags |= cmsFLAGS_NOWHITEONWHITEFIXUP;

   if (!(*dwFlags & cmsFLAGS_NOWHITEONWHITEFIXUP)) {
       FixWhiteMisalignment(Dest, ColorSpace, OutputColorSpace);
   }
   ```

**即：这个领域把「对齐纸白」当成默认开、但可关的一档策略，关它的场景是打样（要看到纸白本身）。**

PostScript CRD 生成路径（`src/cmsps2.c`）里还有一条**吸附窗口**：
只有 L\*=100 且 a\*、b\* 都在 ±8 之内才吸附（`In[1] >= 0x7800 && In[1] <= 0x8800`）。
（这一处子 agent 读出，**我未复核**。）

### 2.2 ICC 规范：这件事的标准化版本叫 media-relative colorimetric intent

（以下逐字由子 agent 从 `ICC.1:2022-05` 官方 PDF 抽出，**我未复核**。）

**§6.2.2 Media-relative colorimetric intents**：

> Transformations for the Media-relative colorimetric intent shall re-scale the in-gamut,
> chromatically adapted tristimulus values such that **the white point of the actual medium is
> mapped to the PCS white point** (for either input or output) as defined in 6.3.2.

**Annex D.5 是规范自己写下的动机，最贴近我们的论证**：

> Aside from the adaptive effects mentioned above, there is frequently a strong aesthetic
> preference for maintaining highlight detail in all renderings of an image. One way to guarantee
> this result for typical reflection media is to **modify the colorimetry of the reproduction so as
> to factor out the colorimetry of the substrate**. … According to the media-relative method the
> PCSLAB media white [100, 0, 0] is associated with the media white point, **regardless of its
> actual colorimetry**, and all other colours are modified accordingly.

**"regardless of its actual colorimetry"——媒体白不管实测是多少，一律映到 PCS 白。这就是 253→255。**

**Adobe 的 Black Point Compensation 白皮书**（<https://www.color.org/AdobeBPC.pdf>，ICC 官网托管）
里有一句反过来说明白点侧地位的话：

> **Although ICC profiles specify how to convert the lightest level of white** from the source
> device to the destination device, **the profiles do not specify how black should be converted.**

**即：白点对齐在这个领域是既成事实，BPC 只是把同样的事补到黑点那一侧。**

**Ghostscript 的文档把它当不言自明**（`doc/src/GhostscriptColorManagement.rst`）：

> Black point compensation is a mapping performed near the black point … **The mapping is similar
> to the mapping performed at the white point between devices.**

> **但 Ghostscript 同时记录了一个真实的翻车案例**，`base/gsicc_cache.c` 注释：
> *"we need to make sure that the CMM does not do something like **force a white point mapping like
> lcms does**. This is also the source of the issue with the flower picture that has the hand made
> ICC profile that is **highly nonlinear at the whitepoint**"*
> ——**源本身在白点附近非线性时，强制白点映射会把它搞坏。**（此条我未复核。）

### 2.3 ImageMagick：官方明说我们要的算子不存在

仓库 `ImageMagick/ImageMagick` @ `ce92ae07c2ca2c20c131eaf6da6da734c1cafce0`（2026-09-08）。

**最值钱的一条**，来自官方 usage 站（Anthony Thyssen，<https://usage.imagemagick.org/color_mods/>），
**✅ 我复核过**（逐字，含原文那个 "posible" 拼写）：

> A mathematically perfect normalization stretching operator is `-auto-level`.
> **While a perfect 'white-point only' or 'black-point only' version is posible it has not been
> implemented at this time.**

**即：ImageMagick 自己承认「只对齐白点」的算子可行、但至今没实现。**

**三个现成算子各自的语义与代价**（官方定义，**✅ 我复核过 usage 站三段**）：

| 算子 | 语义 | 代价 |
|---|---|---|
| `-normalize` | ≡ `-contrast-stretch 2%x1%` | **烧掉暗端 2%、亮端 1%**，且黑点一起动、中调整体平移 |
| `-contrast-stretch 0` | 取真实 min/max | 官方逐字 *"without any loss of data due to burn-out or clipping at either end"*，但黑点仍一起动 |
| `-auto-level` | 精确 min/max + `-level` | 官方逐字 *"a **single 'out-rider' pixel** can set a bad min/max values"*，*"not typically used for real-life images, image scans, or JPEG format images"* |
| `-level 0,W` | 只动白点 | 官方 usage 逐字 *"the colors outside the given range are 'clipped' or 'burned' … **This is the biggest problem with using a `-level` operator.**"* |

> **两个必须记的坑**：
>
> 1. **`-level` 的裸数字按 quantum 取用。** IM 7 默认 Q16，所以 `-level 0,253` 是「把 253/65535 当白点」，
>    会把整幅烧成白板；要写 `-level 0,99.2157%`。官方文档只说 "range from 0 to QuantumRange"，
>    **没有专门提醒这一点**。（此条子 agent 读源码 `MagickWand/mogrify.c` 得出，**我未复核**。）
> 2. **`-normalize` 的裁切比例 2022 年才改成文档写的那个数。** **✅ 我复核过**：
>    commit `32491af93965e30940bb0f4bd183eb867cc44678`（2022-07-24，**message 只有一个词 `release`**）
>    把 `0.0015 / 0.9995`（暗 0.15% / 亮 0.05%）改成 `0.02 / 0.99`（2% / 1%）。
>    **在 IM 7.0.0 – 7.1.0-43 这一整段里，官方文档那句 "2% … 1%" 是不成立的。引它必须连版本一起引。**

**「snap 到格点」这件事 IM 有现成定义，而且就是我们的式子。** **✅ 我复核过**
（`MagickCore/quantize.c:2854-2859`）：

```c
static inline Quantum PosterizePixel(const Quantum pixel,const size_t levels)
{
  double posterize_pixel = QuantumRange*MagickRound(QuantumScale*(double) pixel*
    ((double) levels-1.0))/MagickMax((double) levels-1.0,1.0);
  return(ClampToQuantum((MagickRealType) posterize_pixel));
}
```

即 `out = 255·round(in·(L−1)/255)/(L−1)`——**就是「snap 到 L 个等距级」，最近邻取整**。
开抖动时用的调色板也是同一个格点（`quantize.c:2921`：`scale = QuantumRange/(levels-1.0)`，**✅ 我复核过**）。

**但 `-posterize` 是把整幅图最近邻 snap，不是「只把纸白对齐」。**

### 2.4 这个现象在 ImageMagick 官方文档里有名字：**E-Dither Pixel Speckling**

**✅ 我复核过**（<https://usage.imagemagick.org/quantize/>，"E-Dither Pixel Speckling" 一节）。逐字：

> Another problem with E-Dithers is that they can produce **the occasional odd-colored pixels in
> areas which would otherwise be fairly uniform in color**. … This is especially typical of colored
> objects overlaid onto **flat colored backgrounds**, as you often get in diagrams and drawings.

它给的**成因诊断**正是「平坦区的值不在调色板上」：

> The odd colored pixel is caused by two factors. First, the Color Quantization was forced to
> include a single pure white color (but no other white-blue anti-aliasing colors) into the final
> colormap … **But as E-Dithers slowly accumulate errors** … Eventually the errors will add up to a
> value that is large enough make the one additional color the closest match. As such, every so
> often a highly contrasting white pixel is output to 'correct the error', at a pseudo-random
> location. **The result is a very light speckling** … **The slower the accumulation of error, the
> more spread out those white pixels are and the more, out-of-place, they appear.**

**而官方给的三条处方里没有一条是「改输入值」**：

> The best solution is to switch to some other image format that does not have a limited color
> table. … The next solution is to **replace the use of E-dither with some other dithering method,
> that 'localizes' any errors, such as Ordered Dithering**. … The best fix, to this is to somehow
> insure you **have other colors just outside the large group of colors** that is causing the
> E-Dither error accumulation.

> **第三条处方值得单独记**：它说的是「在那一坨颜色外面多放几个色」——
> 即**改调色板**。这与我们的「改输入值」是同一件事的两个方向。

### 2.5 calibre：默认就在跑 normalize，但差一位，仍然离格

**上一轮把这条漏了。** 仓库 `kovidgoyal/calibre` @ `bfab84570932c7939040edd154f6f8e907d103ce`（2026-09-08）。

**✅ 我复核过** `src/calibre/ebooks/comic/input.py:168-170`：

```python
# Do the Photoshop "Auto Levels" equivalent
if not self.opts.dont_normalize:
    img = normalize_image(img)
```

`--dont-normalize` 的 `recommended_value=False`（`conversion/plugins/comic_input.py:37-41`，**✅ 我复核过**），
帮助文本逐字 `Disable normalize (improve contrast) color range for pictures. Default: False`。
**即 normalize 默认开。**

**身世**：起源 commit `fc5dbaab4769b969e00cb78e4e6ccf6b7d20c27c`（2008-07-30），当时直接调
MagickWand 的 `MagickNormalizeImage`——**语义就是 IM 的 `-normalize`，从第一天起默认开**。
2016-05-10 的 `1cf86cb89e8a93d42dacb636aa0ef95b6d377edb`（*"Remove IM from Comic Input"*）换成自家实现。
**commit message 与注释都没有给理由**，只有那句 `# Do the Photoshop "Auto Levels" equivalent`。
（身世部分由子 agent 追出，**我未复核**。）

**关键：它与 IM 不等价，而且顶端差一位。** **✅ 我复核过**
（`src/calibre/utils/imageops/imageops.cpp` 的 `normalize()`）：

```c
threshold_intensity = count / 1000;          // 注释写 ".01 percent"，代码是 0.1%
...
for (high.red = 256; high.red >= 1; --high.red) {
    intensity.red += histogram[high.red - 1].red;   // ← 红通道索引 -1
    if (intensity.red > threshold_intensity) break;
}
...
for (high.green = high.red; high.green > low.red; --high.green) {
    intensity.green += histogram[high.green].green; // ← 绿/蓝不 -1
```

**我按这段代码自己推了一遍**：设顶端有效 bin 为 `T`，则 `high.red = T+1`，而 `high.green = high.blue = T`。
映射式是 `(255*(i-low))/(high-low)` 的整数除法。取 `T=253`、`low=20`：
红通道给 `255·233/234 → 253`，绿蓝给 `255·233/233 = 255`。

**即纸白 253 出来是 RGB(253,255,255)，qGray = 254——仍然不在格点上。**
随后第 7 步 `convertToFormat(Format_Grayscale16)` 再把它平均回去。

> **对照 IM 7 的 `-normalize`**：同一直方图下白点取 253，`253 → 255` 精确命中。
> **calibre 这个差一位，恰好毁掉了对我们有价值的那一半效果。**

**另有一条 normalize 造成的实际故障**（子 agent 读出，**我未复核**）：
Launchpad #1939908 *《Normalize on CBZ conversion changes white pages to black pages》*，
报告人逐字 *"When I convert a CBZ with full white pages without 'don't normalize' option checked,
the full white pages are converted to black pages."*
修复 commit `30c411acd1719c5c91b2e6e2bb42f82c0f7cb325`（2021-08-20）只加了四行——
就是我上面复核到的 `if (all_colors.size() < 2) return img;`。
**成因是纯白页上 `low == high`，分母为零。**

**而 calibre 那个 e-ink 抖动器仍然没接进漫画管线**（上一轮的结论成立）：
`src/calibre/utils/img.py` 的 `eink_dither_image()` 唯一调用点在封面/缩略图路径，
唯一传 `eink=True` 的是 Kobo 驱动。它的实现 `ordered_dither.cpp` 里 `* 17` 就是 `255/(16−1)`,
**输出严格落在 0/17/…/255**。（此条子 agent 读出，**我未复核**。）

### 2.6 打印驱动侧：「落在格点上就零代价」是被实现出来的快路径

（以下由子 agent 读出，**我未复核**。Gutenprint 在 SourceForge，`p/gimp-print/source` @
`76b0f3571a1cf3d07fe9068e87840be94d28df5a`。）

Gutenprint 的误差扩散 `src/main/dither-ed.c` 里：

```c
if (tmp == 0)
    return error0[direction];
```

以及整行全零就整段跳过（连续 5 行空直接 return 0）。
**即：值恰好落在「不打墨」那一级时，误差扩散一个点都不撒、一分误差都不传播。**

**但 Gutenprint 不做「测源图纸白再对齐」**——`src/main/` 全目录 grep
`white point` / `whitepoint` / `paper white` **零命中**。
它的模型是「输入编码白 = 不打墨」，由传递曲线的构造保证，是**定义而非测量**。
CUPS 侧（`OpenPrinting/libcupsfilters`）同理，`cfImageSetProfile()` 的 `density[0] = 0`。

### 2.7 扫描 / 文档处理：是标准步骤，但**目标值分成两派**

**Leptonica 派：故意归到 200，不归到 255。** **✅ 我复核过**
（`DanBloomberg/leptonica` @ `8fdef8f58ea3747ea3ae6525d03c3568a9f3fdf6`，`src/adaptmap.c:157-168`）：

```c
    /* Default input parameters for pixBackgroundNormSimple()
     * Notes:
     *    (1) mincount must never exceed the tile area (width * height)
     *    (2) bgval must be sufficiently below 255 to avoid accidental
     *        saturation; otherwise it should be large to avoid
     *        shrinking the dynamic range
     *    (3) results should otherwise not be sensitive to these values
     */
static const l_int32  DefaultTileWidth = 10;
static const l_int32  DefaultTileHeight = 15;
static const l_int32  DefaultFgThreshold = 60;
static const l_int32  DefaultMinCount = 40;
static const l_int32  DefaultBgVal = 200;
```

**注释第 (2) 条是本节最重要的反面证据**：
*"bgval must be sufficiently below 255 to avoid accidental saturation"*——
**Leptonica 明确拒绝归一化到 255，理由是防意外饱和。**

**但它自己在下游直接接二值化阈值时就改用 255。** **✅ 我复核过**（`src/binarize.c:258, 266, 396`）：
`pixOtsuThreshOnBackgroundNorm()` 的注释逐字 *"By doing a background normalization first,
**to get the background near 255**, we remove this problem"*，代码里 `bgval` 传的就是 `255`。

**即：Leptonica 的真实态度是「目标值取决于下游」。**
下游还要再过一道 TRC 就取 200 留余量；下游直接是阈值判断就取 255。

**ScanTailor 派：直接拉到 255。** **✅ 我复核过**
（`4lex4/scantailor-advanced` @ `3d1e74e6ace413733511086934a66f4e3f7a6027`，
`src/core/filters/output/OutputGenerator.cpp:359-370`）：

```cpp
struct RaiseAboveBackground {
  static uint8_t transform(uint8_t src, uint8_t dst) {
    // src: orig
    // dst: background (dst >= src)
    if (dst - src < 1) {
      return 0xff;
    }
    const unsigned orig = src;
    const unsigned background = dst;
    return static_cast<uint8_t>((orig * 255 + background / 2) / background);
  }
};
```

**两条路径都到 255**：像素等于局部背景直接 `return 0xff`；否则 `orig*255/background`，
代入 `orig == background` 同样得 255。UI 上叫 *"Equalize illumination"*。

> **ScanTailor 还有一条设计取向值得记**（**✅ 我复核过**，同文件 `:386-396`）：
> `reserveBlackAndWhite()` 把 0 和 255 两个极值**从内容里挤出来**（0→1，255→254），
> 专门留给「这是背景/前景标记」用。**它认为端点值有特殊语义，值得为此牺牲两级。**

**unpaper：不做连续重标定，只做「检测到就整块涂纯白」。** **✅ 我复核过**
（`unpaper/unpaper` @ `5bcef8a9733e601ec672b346e8504f3b4ae04c73`）：
man page 逐字 `Brightness ratio above which a pixel is considered white. (default: 0.9)`；
`unpaper.c:173` `float whiteThreshold = 0.9;`，`unpaper.c:950`
`options.abs_white_threshold = WHITE * (whiteThreshold);`，`constants.h:11` `#define WHITE 0xFF`
——**绝对阈值 229**。但它只被当作「数暗像素」的上界用，判定成立后的动作是
`wipe_rectangle(image, area, PIXEL_WHITE)`（`imageprocess/filters.c:373`）——**整块涂白，不是缩放**。

**Tesseract：自身完全不做背景归一化。**（子 agent 在 `src/` 全目录 grep
`pixBackgroundNorm` / `pixContrastNorm` / `pixCleanBackgroundToWhite` **零命中**，**我未复核**。）
它只用 Sauvola 分块或 Otsu 自适应两条 Leptonica 路径，**都不经过背景归一化**。
官方 wiki 承认 *"the result can be suboptimal, particularly if the page background is of uneven
darkness"*，但给的对策是换二值化方法，不是归一化背景。

### 2.8 漫画转换那一侧：基本空白

**mangle**（`FooSoft/mangle` @ `2d710ee74e1fe670f1e0a01857864079a72326fb`）**✅ 我复核过**：

- 调色板就在格点上：`Palette15a` / `Palette15b` 是 `0x00/0x11/…/0xff`，
  `Palette4` 是 `0x00/0x55/0xaa/0xff`——正是 2bit 的 `0/85/170/255`。
  **但两张 15 级表各缺一档**（15a 缺 `0xee`，15b 缺 `0x66`）。
- `quantizeImage()` 同样**没传 `dither=`**，走 Pillow 默认 FS。**这是继 KCC 之后第二个独立实例。**
- **零色调处理**：`autocontrast` / `normalize` / `level` / `gamma` / `equalize` 全仓检索零命中，
  `ImageFlags` 十二个标志位没有一个与色调有关。
- **而离格纸白会让它的自动裁边静默失效**：`autoCropImage()` 是
  `ImageChops.invert(image).getbbox()`，Pillow 的 `getbbox()` docstring 逐字
  *"Calculates the bounding box of the **non-zero** regions in the image."*
  ——**取反后为零的只有恰好 255 的像素**，纸白 253 取反是 2，bbox 覆盖全图，裁边成空操作。
  （`AutoCrop` 是可选标志，`DefaultImageFlags` 里只有 `Orient | Resize | Quantize`。）

> **注意别读成 bug**：`quantizeImage` 里 `256 - colors` 在 Python 3 上会抛 `TypeError`，
> 但 `requirements.txt` 钉的是 cp27 的 PyQt4 轮子——**这是 Python 2 代码**，那里是整数除法。
> 值得记的是另一件事：**这是一个在产工具里「纸白离格 2 就让一个功能静默失效」的实例。**

**Amazon 的官方发布规范里一个字都没有。** **✅ 我复核过**：
*Amazon Kindle Publishing Guidelines*, **version 2026.2**（© 2025 Amazon.com, Inc.），
<https://kindlegen.s3.amazonaws.com/AmazonKindlePublishingGuidelines.pdf>，130 页全文。
`dither` / `gamma` / `white point` / `bit depth` / `grayscale` / `halftone` —— **全部零命中**
（唯一的 `normalize` 是讲行高的 "text line height normalization"）。

有的只有格式与分辨率。§11.4.2：*"Images must meet the minimum quality standard of **300 ppi**"*、
*"Use **RGB** as the color profile"*。§11.4.6：*"Line-art should be in PNG format"*、
*"**Try reducing the number of colors used.**"*

> **而漫画那一节要求的格式与线稿那一节相反。**§13.2 Image Guidelines（graphic novels）逐字：
> *"**Images must be in the JPEG format.**"* ——**这个矛盾是原文里的，不是我读岔。**

**e-ink 漫画预处理的「事实标准」配方里也没有这一步。**
KOReader/FBInk 维护者 NiLuJe 在 KOReader issue #7949 里作为 "do it right" 给出的那条命令
（MobileRead，2018-07-19）是：缩放 → 灰度 → **直接 `-remap` 到 e-ink 调色板** → PNG。
**没有 `-level`、没有 `-normalize`、没有任何白点对齐。**
（此条子 agent 读出，**我未复核**；且 NiLuJe **没有写下「为什么不做」**，只是没做。）

### 2.9 pngquant / libimagequant 的 `--posterize`：动机对上了，做法没对上

**✅ 我复核过**（`kornelski/pngquant` @ `913a90de9671a6511d634519b1f8a5f329aebbcb`；
`ImageOptim/libimagequant` @ `9388d26902c53854c4f666dc3f07db6504b16924`）。

**动机逐字就是我们的处境**（`imagequant-sys/README.md:339`）：

> Ignores given number of least significant bits in all channels, posterizing image to `2^bits`
> levels. `0` gives full quality. Use `2` for VGA or 16-bit RGB565 displays, **`4` if image is going
> to be output on a RGB444/RGBA4444 display**.

pngquant man page（`pngquant.1:83-84`）：
*"`--posterize bits` — Truncate number of least significant bits of color (per channel).
**Use this when image will be output on low-depth displays**."*
CHANGELOG 记它进的是 version 2.1：*"option to generate posterized output (for use with 16-bit textures)"*。

**它确实发生在量化之前，而且判据跟着放宽**：

- `src/hist.rs:216-219` `add_color()`：`px_int = self.posterize_mask() & rgba_int`
  ——**直方图的键先被掩掉低位再入表**，早于选调色板。
- `src/attr.rs:319`：`target_mse.max((2^min_posterization_output / 1024)^2)`
  ——**posterize 越狠，质量目标越松**。这是「判据要知道输出格点有多粗」的一个在产实现。
- `src/attr.rs:158-163`：取值范围 `0..=4`，**默认 0**。doc 注释逐字
  `/// Number of least significant bits to ignore.`
- `src/hist.rs:273-275`：直方图太大时**自动**再加一位——内存驱动的兜底，不是画质决定。

**但它做的不是我们要的那件事**：

1. **掩码是截断不是就近取整**：`channel_mask = 255 << posterize_bits`，4 位时 `0xF0`，
   **255 → 240，纯白被打下来**。
2. **它掩的是直方图的键，不是像素值**：`.or_insert((boost, rgba))` 存的是**未掩码的原始 rgba**。
   posterize 在这里是**把相近颜色并到一个桶**，最终调色板**并不落在 `255/(2ⁿ−1)` 上**。
3. 全仓库对 `posterize_bits` 的引用只在 `src/hist.rs` 与 `src/attr.rs`——**remap 那一侧不碰它**。

**所以它是「面向输出位深的预处理」这个想法的在产实例，但它对齐的是桶，不是格点。**

---

## 三、文献里有没有名字

**判断：基本空白——没有统一名字。而且最相关的那一支文献，方向与本提案正好相反。**

### 3.1 「格点」有名字，「把源搬到格点上」没有

**文献里指称输出格点的现成术语有五个**（每个都有一手出处）：

| 术语 | 出处 |
|---|---|
| **printable gray levels** | Faheem/Arce/Lau 2002；Adobe *PostScript Language Reference* §7.4.8 |
| **printable output levels** | Park & Ha 系列（2006/2007/2009） |
| **native tones of the output device** | Zhang/Veis/Ulichney/Allebach 2012 |
| **representable gray levels** | Adobe *PostScript Language Reference* §7.4.4 |
| **intermediate gray levels** | Faheem/Arce/Lau 2002 |

**而「把源的取值搬到这些级上」这个操作没有名字。**
子 agent 在 OpenAlex 全库做题名＋摘要**精确短语**检索，逐条零命中：

`"quantization-aware preprocessing"` 0、`"encoder-aware preprocessing"` 0、
`"codec-aware preprocessing"` 0、`"white point clipping"` 0、`"snap to the palette"` 0、
`"input image is preprocessed before quantization"` 0。
`"printable gray levels"` **只有 1 条命中——就是 Faheem 2002 本身**。
`"level snapping"` / `"value snapping"` / `"tone alignment"` 有命中但全是同形异义
（SNAP 食品券、卡扣设计、语言学声调对齐）。

arXiv API 同样的精确短语检索同样零命中，**且有对照组**：`all:"error diffusion"` 返回 20 条正常命中，
证明语法有效。

> **口径**：以上零命中记录由子 agent 跑出，**我未复核**。但它是**否定结论的证据形式**——
> 记下检索式与库名，比"我没找到"有用。

### 3.2 最贴近的**标准化**名字是 ICC 的 media-relative colorimetric，但它不谈格点

见第二节 2.2。子 agent 另从 ICC.1:2022 §0.4 导言读到一句（**我未复核**）：

> Thus the media white will have the values of 100, 0, 0 in PCSLAB.
> **This ensures that highlight clipping will not occur when the media-relative colorimetric intent
> is used.**

**关键差别**：ICC 对齐的是**白点的色度/亮度**，落在连续的 PCS 值上；
它保证的是「纸白 = 输出白」，**不是「纸白 = `255/(2ⁿ−1)` 的整数格点」**。

### 3.3 工程侧最成熟的近亲叫 auto background suppression，但目标不是格点

复印机 / MFP 里「在量化前把纸底推到白」有名字：**Auto Background Suppression (ABS)**，
出处是专利与厂商文档而非论文。子 agent 检索到一批 Xerox 专利（**只读到 Google Patents 的检索片段，
未读全文**；题名/受让人/日期已核对）：

- **US 9,262,704 B1**，Xerox，2016-02-16。**它的流水线顺序正是我们要的那个**：
  neutral page detection → filtering → **image background suppression** → pixel classification →
  image scaling ——**降位深之前先压纸底**。
- US 10,986,250 B1（Xerox，2021）、US 6,522,791 B2 / US 6,665,098 B1（Xerox，2003，
  UI 里有 "auto-background suppression level"，检测 "background peak"）。

**ABS 的目标是去底灰 / 省墨，不是对齐输出格点。**

### 3.4 **最重要的一条：多级半色调那一支的方向与本提案正好相反**

**这是本节最实质的发现。**

多级半色调文献**确实完整讨论过「输入恰好等于输出级」这个结构**——
但它是从**渐变区**的视角看的，把「这一档突然不撒点、周围却在撒」当成 **banding 病**，
**解法是往那一档注入点**。

**✅ 我复核过**（OpenAlex 抓的出版方摘要，逐字；**正文均在付费墙后，未读到**）：

- **T. Park, M. Lee, C. Son, Y. Ha**, *Multitoning Method Based on Threshold Modulation Using
  MJBNM for Banding Artifact Reduction*, CGIV 2006, DOI `10.2352/cgiv.2006.3.1.art00065`：

  > **Since banding artifacts appear as uniform dot distributions around the intermediate output
  > levels**, such halftone patterns result in discontinuity and a visually unpleasing output in
  > smooth transition regions. … the proposed method then arranges the dot distribution by
  > **introducing pixels to the neighborhood of the output levels** through threshold modulation…

- **X. Zhang, A. Veis, R. Ulichney, J. P. Allebach**, *Multilevel halftone screen design: Keeping
  texture or keeping smoothness?*, ICIP 2012, DOI `10.1109/icip.2012.6466988`：

  > The traditional approach to multilevel halftoning is hampered by the appearance of
  > **contouring in the vicinity of native tones of the output device**.

同一支还有（题录经 Crossref 核对，**摘要级，正文未读到**）：
Park/Jang/Kwon/Park/Ha, *Banding-Artifact Reduction Using an Improved Threshold Scaling Function
in Multitoning with Stochastic Screen*, **JIST 51(6):502–508, 2007**,
DOI `10.2352/j.imagingsci.technol.(2007)51:6(502)`（**✅ 题录我用 Crossref 核过**）——
逐字 *"banding artifacts can appear due to **similar dot distributions near the printable output
levels**"*；以及 *Tone-Replacement Error Diffusion for Multitoning*, IEEE TIP 2015,
DOI `10.1109/tip.2015.2460451`——逐字 *"the banding effect, **indicating the areas with only one
tone level**"*。

> **这一条要读准，它是本篇最容易被误引的地方：**
>
> **同一个物理事实，两种相反的读法。**
> 「源值落在格点上 ⇒ 那一档不撒点」——渐变区里，这是**病**（它与邻档的撒点密度不连续，看成 band），
> 治法是**往那一档注入点**；平坦纸底上，这是**药**（整片白底一个点不撒）。
>
> **文献只从第一种视角写过。第二种视角——「离格 ⇒ 平坦白底必须撒点」——
> 在本轮检索范围内没有被单独命名或处理过。**

### 3.5 Faheem 2002 的引用文献里没有一条走预处理路线

子 agent 用 Semantic Scholar 取到 Faheem 2002 的**全部 15 条引用文献**并逐条看过题名与摘要
（**我未复核**）：Park/Ha 门限调制系列 5 篇、Bacca Rodríguez/Arce/Lau 2 篇、
Lee & Allebach *The Hybrid Screen*、Wang/Arce/Di Crescenzo 的半色调视觉密码、
Venugopal/Heath/Lau 的 FPGA 实现 3 篇、2 篇无关的生物信息学。

**没有一条走预处理路线。** 对 *Blue-Noise Multitone Dithering* 在 OpenAlex 上的 52 条被引扫描，
结论相同。

### 3.6 两条对我上一轮设问的更正

1. **「Shiau & Fan 的多级误差扩散」这个说法不成立。**
   子 agent 读了 **US 5,353,127**（*Method for quantization gray level pixel data with extended
   distribution set*，Jeng-Nau Shiau / Zhigang Fan，Xerox，1993 申请 / 1994 授权，
   **一手·已读全文 7 页 PDF**）：它讲的是**误差分配滤波器**（worm 伪影、边缘过增强、中间调纹理取向），
   **完全没有讨论多级输出级的放置，也没有「输入恰好落在输出级上」这个情形**。
2. **视频编码的 deadzone / trellis / RDOQ 不是最近的类比。**
   这三条（Sullivan & Sun 2005 `10.1117/12.631550`、Wen/Luttrell/Villasenor 2000
   `10.1109/83.855437`、Yang & Yu 2007 `10.1109/tip.2007.896685`，题录经 Crossref 核对、
   **正文均未读到**）**全部在量化侧改决策规则，源值一个字节都没动**。
   真正「知道量化行为、于是改输入」的那一支是 **preprocessing for coding**
   （如 Chadha & Andreopoulos, *Deep Perceptual Preprocessing for Video Coding*, CVPR 2021,
   DOI `10.1109/cvpr46437.2021.01461`，摘要自称 "rate-aware deep perceptual preprocessing"），
   **但它优化的是率失真，不是格点；而且这一支自己也没有固定名字**
   （`"encoder-aware preprocessing"` / `"codec-aware preprocessing"` 在 OpenAlex 全库零命中）。

### 3.7 「半色调之前先改输入值」有一份很老的权威规范

Adobe, *PostScript Language Reference*, 3rd edition（子 agent 读了 §7.3 / §7.4.4 / §7.4.8，
**我未复核**）：

> §7.3: the PostScript interpreter applies the transfer function after performing any needed
> conversions between color spaces, **but before applying a halftone function**… the purpose of a
> transfer function is **to compensate for the device's actual behavior**.

> §7.4.4: if there are 2 bits per pixel, each pixel can directly represent one of four different
> gray levels… In this situation, **the threshold values do not represent absolute gray levels,
> but rather gradations between any two adjacent representable gray levels**.

**即：「在半色调之前按设备实际行为改写输入值」这个位置，规范里早就有了（传递函数）；
规范也承认「落在 representable gray level 上就没有调制」这个结构。
但它没有说要不要把输入搬上去。**

---

## 四、代价与已知坑

**判断：有一手证据，而且最硬的一条是反对的。**

### 4.1 数字档案规范直接把这个操作写成缺陷

**这是本轮最硬的一条反面证据。** **✅ 我复核过**——我下载 PDF、抽出全部 129 页文本、逐句核过。

来源：**FADGI *Technical Guidelines for Digitizing Cultural Heritage Materials*, Third Edition,
May 2023**，<https://www.digitizationguidelines.gov/guidelines/>。

**§7.4 论设备局限，逐字**：

> Also, many office and document scanners are **set at the default to force the paper of the
> original document to pure white in the image, clipping all the texture and detail in the paper
> (not desirable for most originals in collections of cultural institutions).** These scanners will
> not be able to meet the desired tone reproduction without recalibration…

**这句话描述的就是本提案本身，而 FADGI 把它当成「这台扫描仪不合格」的判据。**

**§6.2 Color Correction and Tonal Adjustments，一组要求，逐字**：

> Images should be adjusted to render correct highlights and shadows … of appropriate brightness,
> and **without clipping detail**.

> **Avoid tools with less control that act globally, such as brightness and contrast, and that are
> more likely to compromise data, such as clipping tones.** Use tools with more control and numeric
> feedback, such as levels and curves.

> **Do not rely on "auto correct" features.** Most automatic color correction tools are designed to
> work with color photographic images and the programmers assumed a standard tone and color
> distribution that is not likely to match your images. **This is particularly true for scans of
> text documents, maps, plans, etc.**

> **一条硬数字，但要连限定一起引**：
> *"Values as measured on an 8-bit scale should not be less than 5 in the shadows or more than
> **250 in the highlights**."*
> ——**✅ 我复核过：这句在全文出现两次，两次都在「负片扫描」（negative scanning）那一节**，
> 不是对反射稿的通则。**引它必须带这个限定。**
>
> **另一处容易误读**：FADGI 第三版对**反射原件的纸白本身没有给数值目标**。
> 表里那条 "Highlight/Shadow, Tolerance 96 ± 2 (L\*)" 是**测试标板白块**的目标值，不是纸白。
> （此条子 agent 指出，**我未复核**。）

### 4.1b Metamorfoze 给了数字——但它自己划了一条界，而那条界把我们放在外面

**✅ 我复核过**——我在本地 PDF 上逐字读过下列每一句。

> **⚠ 出处口径必须写清楚**：这两份 PDF **不是本轮从官网抓的**。metamorfoze.nl 改版后，
> 我与子 agent 各自试过的路径全部返回 404。用的是 scratchpad 里前序会话留下的本地文件。
> **可交叉验证的凭据**：两份都是 44 页，PDF 元数据
> v1.0 `CreationDate D:20120412`、`Creator Adobe InDesign CS5.5`；
> v2.0 `CreationDate D:20250704` / `ModDate D:20250807`、`Creator Adobe InDesign 20.4 (Macintosh)`
> ——与各自标注的 2012-01 / 2025-04 版次相符，正文页脚也自称
> *"Metamorfoze Preservation Imaging Guidelines | Image Quality, Version 1.0, January 2012"*。
> **文件本身可信，但我没能独立确认它就是官网现行发布的那一份。**

**v2.0（2025-04，Hans van Dormolen）§2.6 Exposure**，两句**不相连**（中间隔着一句 Extra Light 容差）：

> To avoid clipping in the highlights, **the maximum L\* value should not exceed L\* 98 anywhere in
> the image.** This applies to all three Metamorfoze quality levels. …
>
> An L\* value of 98 corresponds to an 8-bit pixel value of **250** in color space eciRGBv2,
> and to an 8-bit pixel value of **249** in the Adobe RGB (1998) and sRGB IEC61966-2.1 color spaces.

> **Clipping in the highlights is unacceptable at any quality level.**

**它给出的理由，正是我们担心的那一档内容**：

> **Highlight information refers to faint textual information on a white background (white paper),
> such as discolored letters, pencil notes, signatures and stamps, as well as soft colors and paper
> hues.**

> **同一段还给了「高光」的定义**：*"The highlights are between **L\* 95 and L\* 85**."*
> ——**即它说的高光是纸白往下一整段，不是紧贴 255 的那两三级。**

**v1.0（2012-01）给的是另一个数**：*"To prevent clipping, **the maximum 8 bit pixel value in the
frame may never exceed 248**."*（在 **§2.8 Illumination**，不是 §2.6。）

> **而 v1.0 自己划了一条界，这是全节最需要记住的一句。**
> 它不是正文，是 **§2.6 Gain modulation 的边注 23**（页脚标 21），挂在正文
> *"Contrast enhancement in the high lights can result in clipping[23] and must therefore be
> prevented."* 上。边注全文逐字：
>
> > The term 'clipping' means that there is no image information on and from a certain luminance
> > level in a color or luminance channel. **In preparing a preservation master, clipping must be
> > prevented at all times. For output and specific purposes clipping can be useful in a certain
> > luminance area.**
>
> **即：这三份规范管的都是「档案里那一份」。Metamorfoze 明确承认为输出目的做 clipping 可以有用。**
>
> **所以 4.1 那条 FADGI 的反对意见有一个范围**：它反对的是**把这一步放进归档路径 / 当成母版**。
> 我们产出的是派生阅读件，不在它的射程内。**但它划的这条线本身要记住。**

**ISO 19264-1:2021 §4.4 Exposure**（子 agent 只读到 iTeh 公开预览第 1–8 页，**我未复核**）：

> The exposure shall be adjusted so a diffuse white flat surface … is captured and recorded using
> **encoding values that have an L\* value equal to the actual L\* value of the diffuse white flat
> surface**.

同标准 §4.8 a)：*"Particular attention should be paid to processing controls that apply nonlinear
tone reproduction, or **black or white clipping**."*

### 4.2 近白浅灰被压掉：一般命题有多份一手记录

（以下由子 agent 取材，除标注外**我未复核**。）

- **ImageMagick 官方**把 clip 称为 `-level` 的头号问题（**✅ 我复核过**，见 2.3 表下）。
- **ImageMagick 官方还解释了它为什么故意不拉到真极值**，逐字（**✅ 我复核过**）：
  *"**For practical reasons to do with JPEG color inaccuracies** … **and scanned image noise**,
  `-normalize` does not expand the very brightest and darkest colors, but a little beyond those
  values."*
- **GIMP** 把敢 clip 的那条限死在 **0.05% 的像素**（White Balance 文档逐字
  *"which are used by only 0.05% of the pixels"*），而不敢 clip 的那条明说
  *"it **does not reject any of the very dark or very bright pixels, so the white might be impure**"*。
- **Pillow issue #3331**（jnweiger，2018-09-04 开，2022-10-03 关）：浅灰底被压成纯白，
  报告人逐字 *"The background is half black 0x000000 and half white 0xffffff. That is unexpected."*
  维护者裁定 *"**This behaviour is what is described in the documentation**, so I don't think
  there's anything to do here."* ——**是文档化的正常行为，不是 bug，但它确实让人吃惊。**
- **Pillow 为「平坦图被拉爆」专门加了回归测试**：PR #5350 的 diff 里
  `test_autocontrast_preserve_one_color`，注释 `# single color images shouldn't change`。
  它是为堵 wiredfool 在 PR #1246 审阅时发现的塌缩加的，原话
  *"But (127,127,127) returns an image that's (127, 255, 255) … I don't think that this is
  intended behavior."*
- **扫描侧有人做了归一化、发现害了某类内容、于是关掉**：ScanTailor **PR #151
  *"Don't normalize mixed mode pictures"***（vphantom，2015-07-23，**至今 open，讨论区为空**），
  正文逐字 *"In non-dewarped mixed mode, **avoid equalizing illumination in pictures**."*
  **诚实标注：作者只写到「对图像区域有害」这一层，没有写出「浅灰/网点丢失」这类具体症状。**
- **最贴近的用户报告**：scantailor-experimental issue #73（ChekPuk，2026-03-09）逐字
  *"When I get to Output **picture becomes to bright, text of old book is barely visible**"*，
  维护者同日给的解法是把 `'Equalize illumination'` 关到 0。
  **限定：这是整幅提亮，不完全等同于白点对齐，但失效方向一致。**

> **一条必须写清楚的口径**：上面这些证据支持的是**「自动白点拉伸会把近白浅灰压成纯白」这个一般命题**。
> **「漫画里最淡那档网点/铅笔线恰好落在 253~255」这个具体前提，我没有找到任何一手来源**——
> 那是要用 `docs/measurements.md` 去实测的东西。

### 4.3 逐页做会闪烁：机制侧有官方文档写死

**✅ 我复核过**（<https://ffmpeg.org/ffmpeg-filters.html>，§`normalize` 与 §`deflicker`）。

FFmpeg `normalize` 滤镜的文档，**逐字**：

> For each channel of each frame, the filter computes the input range and maps it linearly to the
> user-specified output range.
>
> **Temporal smoothing can be used on the input range to reduce flickering (rapid changes in
> brightness)** caused when small dark or bright objects enter or leave the scene. This is similar
> to the auto-exposure (automatic gain control) on a video camera, and, like a video camera,
> **it may cause a period of over- or under-exposure of the video**.

`smoothing` 参数逐字：

> The number of previous frames to use for temporal smoothing. The input range of each channel is
> smoothed using **a rolling average over the current frame and the `smoothing` previous frames.
> The default is 0 (no temporal smoothing).**

官方示例把因果写死了：

> Stretch video contrast to use the full dynamic range, **with no temporal smoothing; may flicker
> depending on the source content**: `normalize=blackpt=black:whitept=white:smoothing=0`

`deflicker` 滤镜的定位是 *"**Remove temporal frame luminance variations.**"*，
`size` 默认 **5 帧**的移动平均，`mode` 提供 median 等稳健统计量。

**这就是我们那个问题一比一的对应物**：逐帧（=逐页）独立算白点 → 闪烁；
标准解法 = 对白点做跨帧滚动平均；**且官方自己警告平滑会带来过冲/滞后**。

**但两条必须分清**：

- **「逐页独立算白点会闪烁」有一手证据。**
- **「这在电子墨水上因刷新残留/白底面积大而特别明显」——子 agent 没找到任何一手证据，
  那目前只是推理。**
- **「没有任何漫画转换工具做逐卷统一」是个否定结论，见第五节。**

### 4.4 与有损编码的相互作用

**三条一手记录一致：抖动过的图不要交给有损编码。**

1. **KCC 2013 提交的源码注释**（commit `751e6eb4e700b0fdaf346feb1478cc5a0cb08162`，
   Frédéric Devernay，2013-03-05）diff 里：`# quantized images don't like JPEG`。
   同一作者在 PR #26 的正文里写清了理由（子 agent 从 API 取到，**我未复核**）：
   *"quantization uses the Floyd-Steinberg dithering algorithm which basically adds noise, and
   **JPEG compresses this noise by creating blocky artifacts**"*。
2. **KOReader issue #7949 的 NiLuJe 评论**，**✅ 我复核过**
   （<https://github.com/koreader/koreader/issues/7949#issuecomment-1439281961>，
   NiLuJe，**2023-02-22T00:48:19Z**），逐字：
   > **Do \*NOT\* compress dithered images with a lossy codec (any of 'em). That will absolutely
   > horrendously murder it (and you'll generate much bigger files than expected to boot).**
3. 同一人同期在 MobileRead 的展开（2023-02-21）：*"feeding it dithered content is the absolute
   worst thing you can do to a lossy compression algorithm"*。（**我未复核**。）

**JPEG 侧的标准依据**（子 agent 从 ITU-T T.81 全文抽出，**我未复核**）：§A.3.4 逐字
*"**Rounding is to the nearest integer**"*、
*"**Depending on the rounding used in quantization, it is possible that the dequantized coefficient
may be outside the expected range.**"*；libjpeg 文档另有
*"JDCT_FLOAT may also give different results on different machines due to varying roundoff behavior"*。

> **「拉白点之后再有损编码，会不会把白底重新推离格点」这个问题，文献没有直接答案。**
> 子 agent 在本机跑了一次最小往返验证，结论是「平坦区不会，线稿旁边会（ringing）」——
> **那是一次自测，不是外部来源，数字不进本文**，处置见《没查到的》第 5 条。

---

## 五、逐页自适应 vs 全局固定：怎么知道这一页的纸白是多少

**判断：估计纸白有成熟方案；但「满版无纸边页」被检测过，从没被解决过。**

### 5.1 成熟的那一套，常数都是公开的

| 做法 | 一手实现 | 关键常数 |
|---|---|---|
| 分块 + 邻居插值 | Leptonica `pixBackgroundNorm()` | tile 10×15、fg 阈值 60、`mincount` 40（≈27%）、`bgval` 200、平滑 2×1 |
| 高百分位 | ImageMagick `-normalize` | 暗 2% / 亮 1%（**2022 年前是 0.15% / 0.05%**，见 2.3） |
| 高百分位（局部+全局） | OCRopus `ocropus-nlbin` | 局部 80 百分位滤波（窗 20×2 再 2×20），全局 `--lo 5` / `--hi 90` |
| 多项式背景面 | ScanTailor `estimateBackground()` | 先降到 300×300，水平 8 次 / 垂直 5 次 |
| 众数（滑窗版） | ScanTailor-Advanced `calcDominantLevel` | 宽 10 的窗内取累计过半的 level |
| Otsu 取上半 | ScanTailor-Advanced `BackgroundColorCalculator` | Otsu 切一刀 → 亮侧 → 滑窗众数 |
| 边缘/条带投票 | KCC `fillCheck()` | 阈值 128 二值化，5 像素宽的横竖条投票 |

**OCRopus 有一条注释直接讲「别让平坦区污染百分位」**（子 agent 读出，**我未复核**）：

> ```python
> # by default, we use only regions that contain
> # significant variance; this makes the percentile
> # based low and high estimates more reliable
> ```

**它掩掉的是低方差区**——在满版绘图页上留下的恰恰是画面本身。

**KCC 的 `fillCheck()` 值得单独记**，**✅ 我复核过**（`image.py:245-282`）：
它先按阈值 128 二值化，比两个包围盒的面积；差 ≤ 0.5% 时走 5 像素条带投票，
`getImageHistogram()` 对每条返回 −1/0/+1，求和定 `'black'`/`'white'`，**默认退路是 `'white'`**。

> **关键事实：`fillCheck` 只输出 `'white'` / `'black'` 两个字符串，从不输出灰度值。**
> **KCC 全程没有「这一页的纸白是多少」这个量。** 它拿这个结果去填页面 padding 的底色。

### 5.2 「最大平坦区」这条路：原语成熟，用法是新的

**两个最硬的库都有 O(n) 的「最大空白矩形」原语，但都只用于版面分割，没有一处用它测灰度。**
（子 agent 读出，**我未复核**。）

- Leptonica `pixFindLargestRectangle()`（`src/pageseg.c`），作者注释自夸
  *"a simple and elegant solution … The solution is O(n)"*。
- ScanTailor `MaxWhitespaceFinder`（`src/imageproc/MaxWhitespaceFinder.h`），
  *"Finds white rectangles in a binary image starting from the largest ones."*

**即：我们走的这条路在工程原语上有成熟支撑，在「用它来量纸白」这个用法上是空白。**

### 5.3 失败判据：五个现成的，可以直接搬

| 工具 | 判据 | 出处 |
|---|---|---|
| **Leptonica** | `nmiss == nx`（一列有效数据都没有） | `adaptmap.c:1546` **✅ 我复核过** |
| **Leptonica** | 全图无一对 min/max 差 ≥ `mindiff`（建议 ≥50） | `adaptmap.c` 的 `pixSetLowContrast()` |
| **Tesseract** | 前景占比落在 **25%–75%** 之间 = "no thresholding information" | `src/ccstruct/otsuthr.cpp` |
| **OCRopus** | `mean < median`（页面必须以背景为主）；极值像素占比 > 95% | `ocropus-nlbin` |
| **KCC** | 全图极值差 < **159** | `image.py:471-473` **✅ 我复核过** |

**Otsu 本人给的可信度指标**：η\* = σ²_B(k\*)/σ²_T。子 agent 读了原文
（Otsu 1979, IEEE Trans. SMC 9(1):62–66, DOI `10.1109/TSMC.1979.4310076`），逐字（**我未复核**）：

> The maximum value η(k\*) … **can be used as a measure to evaluate the separability of classes
> (or ease of thresholding) for the original picture or the bimodality of the histogram.**
> … **The lower bound (zero) is attainable by, and only by, pictures having a single constant gray
> level**, and the upper bound (unity) is attainable by, and only by, two-valued pictures.

以及 Otsu 自己写下的失败情形：

> **However, for most real pictures, it is often difficult to detect the valley bottom precisely,
> especially in such cases as when the valley is flat and broad, imbued with noise, or when the two
> peaks are extremely unequal in height, often producing no traceable valley.**

**并且 Otsu 从不报错**——原文逐字 *"**It is, therefore, obvious that the maximum always exists.**"*

**Leptonica 另有一个「明说自己会拒答」的检测器**：`numaFindLocForThreshold()`
（`src/numafunc2.c`），四条错误返回：`"all array values are the same"`、
`"top of first peak not found"`、`"no minimum found"`、`"minimum at end of array; invalid"`。
调用者 `pixThresholdByHisto` 的注释逐字 *"Returns 0 in %pthresh if it can't find a good threshold."*
（子 agent 读出，**我未复核**。）

### 5.4 检测到之后：业界只有两种退路，都是放弃

**这一条是本节最重要的发现。**

| 工具 | 退路 | 出处 |
|---|---|---|
| **Leptonica** | `L_WARNING("no bg found; no data in any column")` → 上层 `L_WARNING("map not made; return a copy of the source")` → **原样返回源图** | `adaptmap.c:1546-1550, 377-381` **✅ 我复核过** |
| **OCRopus** | **整页 SKIPPED，不处理**；或 `comment = "no-normalization"`，`flat = image` | `ocropus-nlbin` |
| **KCC** | **跳过 autocontrast**，注释 `# ... that was probably intentional` | `image.py:470-473` **✅ 我复核过** |
| **calibre** | `all_colors.size() < 2` → 原样返回 | `imageops.cpp` **✅ 我复核过** |
| **Tesseract** | `// Use the best of the ones that were not good enough.` | `otsuthr.cpp` |
| **k2pdfopt** | **硬编码 192** | `k2pdfoptlib/k2bmp.c` 的 `bmp_adjust_contrast()`：`if ((*white) <= 0) (*white)=192;` **✅ 我复核过** |
| **unpaper** | **硬编码 0.9×255 = 229** | `unpaper.c:173` **✅ 我复核过** |
| **ScanTailor** | **无检测**。掩膜空 → 多项式退化成常数 0 面 → `RaiseAboveBackground` 把整页刷成纯白 | `PolynomialSurface.cpp` + `OutputGenerator.cpp:359-370` |

**即：五个工具检测到「量不到背景」之后，动作分别是「原样返回」「取最烂里最好的」「整页跳过」
「跳过不处理」「清空 map」——没有一处试图给出一个数。**

> **k2pdfopt 值得单独点名**：它是 e-ink 重排的事实标准工具，用户文档写着
> *"The default value for -wt is -1, which tells k2pdfopt to **pick the optimum value**."*
> 而代码里 "pick the optimum value" 就是 `(*white)=192;`。**文档与实现不符。**
> （文档那句由子 agent 读出，**我未复核**；`192` 那一行 **✅ 我复核过**。）

> **ScanTailor 那条要读准**：它的整页刷白**不是设计好的 fallback，是退化路径的意外结果**。
> `PolynomialSurface` 在数据点为 0 时把系数置成单个 0.0，`render()` 全 0，
> 而 `RaiseAboveBackground` 在 `dst - src < 1` 时 `return 0xff`——背景为 0 时该条件恒成立。
> **ScanTailor 没有任何一处写下「背景估计不出来时该怎么办」。**

### 5.5 Leptonica 作者自己写下的那一句，正中我们的失败情形

**这是整节最直接的一条。** 来源：<http://www.leptonica.org/binarization.html>
（`normalization.html` 已 **404**，对应内容在这一页）。逐字：

> The basic method is to determine the local background value and map the pixels linearly so that
> the background is put at some constant value like 150 or 200. **There are two basic problems.
> The first is that there must be an actual local background. If there is an image on the page,
> it will be degraded if we try to force a background on it. So it may be important to do
> text-image segmentation first and then do background mapping only on the text parts.**
> … **We require a minimum number of such pixels (typically about 1/3 of the pixels in the tile),
> and if there are fewer, we estimate the background value in the tile based on its neighbors.**

**Bloomberg 的建议不是「改进背景估计」，而是「先把图片区域分出来，只对文字区做背景映射」。
对满版页而言，「文字区」是空集——这条建议等价于「这一页不做」。**

### 5.6 `pixFillMapHoles()` 是「向邻居借值」，但只在页内借

**Leptonica 里处理「某个 tile 没有背景像素」的就是它。** 注释第 (3) 条逐字（**我未复核全文**）：

> The "holes" can come from two sources. **The first is when there are not enough foreground or
> background pixels in a tile**; the second is when a tile is at least partially covered by an
> image mask.

做法：先沿列上下复制、再沿行左右复制整列。
**它的边界正好是我们撞上的那条**：借值只在**同一页内**进行；
一旦「整页没有一列有数据」就返回 1（**✅ 我复核过那一行**）。

**Leptonica 里没有跨页版本。** 子 agent 逐仓库 grep 过 Leptonica、ScanTailor 两版、KCC、calibre、
k2pdfopt、unpaper、OCRopus、Tesseract，**「用整卷其它页的统计补一页量不到的」零命中**。
ScanTailor 的「应用到所有页」只是 UI 层的**参数**传播，不是**测量值**传播。

### 5.7 「用整批补单张」这条路，只在另一个领域被发表过

**这是本节唯一一条为我们这个想法背书的已发表工作，而且它不在文档图像领域。**

J. van de Weijer, T. Gevers, A. Gijsenij, *Edge-Based Color Constancy*,
IEEE Trans. Image Processing 16(9):2207–2214, 2007, DOI `10.1109/TIP.2007.901808`。
子 agent 读了作者开放副本全文，p.2207 逐字（**我未复核**）：

> **If the images under evaluation are part of a coherent image data base, Gershon et al. [14]
> showed that assuming the average of a scene to be equal to the average reflectance of the
> database, improves the results over the standard gray-world method.** As an example, they mention
> forest pictures full of green colors. In this case, most color constancy methods will predict
> light sources biased towards the green color. **The database-compensated gray-world algorithm
> resolves this problem.**

同文还记录了「大面积单色区域压垮估计」的实测：
*"**The second row shows an example where the large blue sky results in an light source estimation
which is much too blue for the gray-world methods.**"*

以及 white-patch / max-RGB 假设（即「最亮处就是白」，我们「最大平坦区就是纸白」的彩色对应物）：
*"Since a white patch reflects all the incident light, its position in the image can be found by
searching for the maximum RGB values."*

> **口径**：原始出处是 Gershon, Jepson & Tsotsos, *From [r,g,b] to surface reflectance*,
> Proc. 10th IJCAI, 1987, pp. 755–758，**子 agent 没读到原文**，上面这段是 2007 年那篇的复述。

### 5.8 反面：有没有人说「背景估计不可靠，别依赖」

**没有整体否定的论文或工具。但有四条来自作者本人的、条件明确的「这时候别信它」**——
Bloomberg 的「必须真的有背景」（5.5）、Leptonica 代码里三处明写放弃（5.4）、
Tesseract 的 *"we assume this channel contains no thresholding information"*、
Otsu 的 η\*。

另有一条关于 Sauvola 参数的（Lazzara & Géraud, IJDAR 17:105–123, 2014,
DOI `10.1007/s10032-013-0209-0`，子 agent 读了作者开放版全文，**我未复核**）：

> **there is no consensus in the research community regarding those parameter values.**

以及 Sauvola 在**纯背景窗口**上的著名失败模式，同文逐字：

> **We expect the algorithm to retrieve plain objects but in case of a too small window, statistics
> inside the objects may behave like in background: pixels values are locally identical. Since
> Sauvola's formula relies on the fact there is a minimum of contrast in the window to set a pixel
> as foreground, it is unable to make a proper choice.**

> **一条与我们语料直接相关的事实**：DIBCO 竞赛语料的退化类型清单（Gatos et al., IJDAR 14:35–44,
> 2011，子 agent 读了作者机构开放副本全文）里列的是
> *"variable background intensity, shadows, smear, smudge, low contrast, bleed-through or
> show-through"*——**没有「满版图画、无纸边」。**
> **DIBCO 语料是 5 张机印 + 5 张手写文档，全是文字页。
> 即：我们那个失败情形在这个领域的标准 benchmark 里根本不在测试范围内。**

---

## 直接可用的，与只是背景的

**本节是全文唯一一处把外部现状对到我们处境上的地方。仍然不含建议。**

### 直接对得上的

1. **lcms2 的 `FixWhiteMisalignment` 是本提案的同构先例，连三条「何时不该做」的判据都是现成的**
   （2.1）：离格量过大不修、不在格点上不修、某一档 intent 下整个关掉。
   **而它的动机注释 "in order to avoid scum dot" 与我们观察到的现象是同一件事。**
2. **格点的权威出处有两个**：PNG 规范 §12.4 / §13.12（零节），
   与 ImageMagick 的 `PosterizePixel()`（2.3）——后者的式子与我们的定义一字不差。
3. **`Leptonica` 的「目标值取决于下游」是可直接引用的判据**（2.7）：
   下游还要过 TRC 就取 200，下游直接是阈值判断就取 255。
   **我们的下游是量化 + 抖动，属于后者。**
4. **失败判据不必自创**（5.3）：Leptonica 的 `mincount ≈ tile 的 1/3`、Tesseract 的 25%/75%、
   Otsu 的 η\*、Leptonica 的 `mindiff ≈ 50`、KCC 的极值差 159。
   注意我们量到的「最大平坦区 4430 像素」这个数（见 measurements 的
   《全语料普查：四成三的页纸白不落在格点上》），对照上述口径应换算成**占页面的比例**才可比。
5. **量不到时的退路，业界只有两种，都要显式**（5.4）：「什么都不做，原样退回」或「整页跳过」。
   硬编码常数（k2pdfopt 的 192、unpaper 的 229）是被文档粉饰过的第一种。
6. **KCC 的 `autolevelImage()` 是一个可对称化的实现原型**（第一节）——
   它在黑端做众数吸附，白端至今空着。
7. **KCC #474 是一份「试过并放弃」的现成结论**（第一节）：
   逐页定制、成本高于收益。**它放弃的是 cutoff 那条路，不是我们这条。**
8. **「格点」有五个现成术语可用**（3.1），最通行的是 **printable gray levels**
   与 **native tones of the output device**。**取名不必自创，挂靠已有术语即可。**
9. **Metamorfoze v1.0 边注 23 划的那条界，是引用归档规范时的必备限定**（4.1b）：
   那些规范管的是**保存母版**，并明说 *"For output and specific purposes clipping can be useful"*。

### 只是背景的

- ICC 的 media-relative colorimetric intent（2.2 / 3.2）说明这件事在色彩管理里是标准化的，
  但它作用在连续的 PCS 值上，**不涉及输出格点**。
- Gutenprint 的「值落在不打墨那一级就零代价」（2.6）是我们那条论断的独立佐证，
  **但它的白是定义出来的，不是测出来的。**
- 复印机的 **auto background suppression**（3.3）在流水线位置上与我们一致（降位深之前压纸底），
  **但目标是去底灰 / 省墨，不是对齐格点**，且只读到专利检索片段。
- 5.7 那条「用整批补单张」来自彩色恒常性，**不是文档图像**，引用时要说明。
- FADGI 的要求（4.1）**服务的是文物保真，不是阅读观感**，两者的目标函数不同；
  Metamorfoze 自己把这条界写出来了（4.1b）。
  **但 FADGI 把这个操作点名成缺陷这件事本身，无论目标函数是什么都值得记。**

### 需要单独标出的口径问题之一：同一个事实，文献从相反的一侧写过

**3.4 那一条极易被引反，单独说清楚。**

多级半色调那一支**确实研究过「输入恰好落在输出级上」**，而且研究得很细
（Park & Ha 一系列、Zhang/Ulichney/Allebach）。
**但他们看的是渐变区**：那一档不撒点、邻档在撒，两者并置成 band，所以那是**病**，
解法是 *"introducing pixels to the neighborhood of the output levels"*——**往那一档注入点**。

**我们看的是平坦纸底**：整片区域都是那一个值，不存在"邻档"，所以不撒点就是**药**。

**所以正确的说法不是「文献没研究过这个现象」，而是**：
**文献研究过这个现象，但只从会让它变成缺点的那一侧研究过；
把它当成优点去主动构造，在检索范围内没有先例。**

> 与上一篇《多级误差扩散（multitoning）是另一支文献，而且它有一个专属伪影》对照读：
> 那里引 Faheem 2002 说"零误差带的邻域会出现稀疏点"，**说的正是渐变区那一侧**。
> 两篇合起来才是完整的图景。

### 需要单独标出的口径问题之二：「KCC 不做色调处理」这个说法是错的

**「calibre 不做」也是错的。**

上一篇第四节说 KCC 写下来的判据「全项目只有 `README.md` 一句半」，那句仍然成立——
**但那说的是位深与抖动，不是色调**。色调这一侧的真相是：

- **KCC 十二年来一直在跑 autocontrast，默认开，两条路径，其中一条连 `-n` 都关不掉。**
- **calibre 的漫画管线默认在跑 normalize，注释就写着 "Auto Levels"。**

**两家都在做「把白拉上去」，两家都没有为此写下过任何判据，两家的做法都不对齐格点**
——KCC 对齐的是全页最亮像素，calibre 因为一个差一位只走到 254。

**准确的说法是：这件事被做了很多年，但从来没有被当成一个问题想过。**

---

## 没查到的

诚实清单。**"没查到"比编一个看似合理的答案有用。**

1. **第三节引的论文，正文一篇都没读到。** Park & Ha 系列（2006 / 2007 / 2009，JIST 与 CGIV 付费墙）、
   Zhang/Veis/Ulichney/Allebach ICIP 2012、Guo 的 tone-replacement（IEEE TIP 2015）、
   Bacca Rodríguez/Arce/Lau *Blue-Noise Multitone Dithering*（unpaywall 报 `is_oa=false`，
   IEEE / ACM DL / academia.edu 全部 403）——**全部只到摘要**。
   本文引的那几句逐字来自出版方摘要（我用 OpenAlex 核过 CGIV 2006 与 ICIP 2012 两条），
   **机理与方向可信，措辞在引用进 ADR 前建议再核**。
   同样只到摘要或只到检索片段的还有：Xerox 的一批 ABS 专利（Google Patents 触发反爬，
   只有 US 5,353,127 从 patentimages 拿到完整 PDF）、Narne 2008 的 RIT 硕士论文
   （站点返回拦截页）。
2. **本轮的 WebSearch 配额在开工不久即耗尽（200/200）**，此后全部改用直接抓取、GitHub/GitLab API、
   Crossref/Semantic Scholar/arXiv API、PDF 下载后本地提取、本地 clone + grep。
   **有若干线索因此没有展开，不排除有遗漏。** 这与上一轮是同一个限制。
3. **Metamorfoze 的两份 PDF 我读到了正文，但没能确认它的来源。**
   metamorfoze.nl 改版后我与子 agent 各自试过的路径全部 404，用的是 scratchpad 里前序会话留下的本地文件
   （凭据见 4.1b）。**逐字引文我复核过，但「这就是官网现行发布的那一份」我没能独立确认。**
4. **ISO 12647 系列完全没读到**（ISO 付费墙，`iso.org` 对抓取返回 403）；
   **ISO 19264-1:2021 只读到 iTeh 公开预览第 1–8 页**（§4.4 / §4.5 / §4.8 全文可读，
   **附录 B 的容差表与白块目标值没读到**）。
   **所以三份归档规范里，只有 FADGI 是我从官方 URL 下载并全文核过的。**
5. **Sauvola & Pietikäinen 2000 原文全文**（DOI 经 Crossref 核对，ScienceDirect 403）、
   **Wolf & Jolion PAA 2004 原文**、**Buchsbaum 1980 原文**、**Gershon et al. 1987 原文**、
   **DIBCO 2011 之后各届报告**（IEEE 付费墙）。上述关于它们的转述全部来自二手或作者的其它文章。
6. **「拉白点之后再有损编码会不会把白底重新推离格点」没有一手来源。**
   子 agent 在本机跑了一次最小往返验证（Pillow + libjpeg，平坦块与线稿块各一组），
   结论是「大片平坦区往返精确，线稿边缘会被 ringing 推离」。
   **那是自测不是外部来源，数字不进本文；要用就得进 `docs/measurements.md`。**
   注意本项目默认输出 PNG（`ADR 0004`），这一问主要落在源侧与 `--codec=avif` 上。
7. **「页间背景不一致在电子墨水上特别明显」的一手证据：没找到。**
   没有 E Ink 厂商应用笔记，也没有 KOReader / Kindle / Kobo 的一手 issue 支持这条。
   FFmpeg 那条只证明了机制，没证明 e-ink 上的显著性。
8. **「漫画里最淡那档网点/铅笔线恰好落在 253~255」这个前提，没有任何一手来源。**
   4.2 那些证据支持的是一般命题，不是这个具体前提。
9. **calibre 论坛 / issue 里除 Launchpad #1939908 外，「normalize 拉白底 / 丢浅灰」类报告没找到。**
   子 agent 抓到的几个 MobileRead 漫画转换主题全文 grep `normali` 零命中。
10. **汉化 / scanlation 社区 cleaning guide 里 "don't crush your whites" 一类的表述：没读到。**
11. **Adobe Photoshop "Simulate Paper Color" 官方文档**（helpx 两次 60s 超时）、
    **Fogra / ISO 12647-7 的 paper simulation 规定**（付费 + 配额耗尽）。
    **`ICC.1:2022` 全文里 "paper simulation" 零命中**（这一条是确证的否定结论）。
12. **ImageMagick 官网 `command-line-options.php` 的网页版**现在是 JS 渲染，curl/WebFetch 拿到的正文
    只有 82 字符。**本文引的 IM 官方定义全部来自仓库内 `www/` 源文件与源码注释**，权威性更高，
    但**与线上渲染结果我没有逐字比对过**。
13. **`http://www.leptonica.org/normalization.html` 不存在**（服务器返回 404）。
    该页内容已并入 `binarization.html`，本文引的是后者。
14. **本文的复核分布不均。** **我自己复核过**：PNG 规范全部逐字与节号、Amazon 规范的负面计数与三处引文、
    lcms2 的五处、Leptonica 的 `DefaultBgVal` 与两处失败退路、ScanTailor 的 `RaiseAboveBackground`
    与 `reserveBlackAndWhite`、unpaper 的四处、KCC 的 `autocontrastImage` / `Cover.process` /
    `autolevelImage` / `fillCheck` / `Palette16` / 五个开关默认值 / 管线顺序 / issue #474 / #511 /
    PR #1021 / #1128、Pillow 的 `getbbox` docstring、calibre 的 `input.py:168-170` 与
    `dont_normalize` 默认值与 `normalize()` 全文（并自行推导了差一位的后果）、
    ImageMagick 的 usage 三段 + `PosterizePixel` + `-normalize` 裁切比例的那次改动、
    FFmpeg 的三处、FADGI 的四处、KOReader 的 NiLuJe 评论、k2pdfopt 的 `192`、
    pngquant / libimagequant 的六处、mangle 的四处、**Metamorfoze v1.0 边注 23 与 §2.8、
    v2.0 §2.6 的三处（本地 PDF 逐行核）**、**Park CGIV 2006 与 Zhang ICIP 2012 的摘要逐字
    （OpenAlex）**、**Park 2007 JIST 的题录（Crossref）**。
    **未复核**：ICC 规范全部引文、Adobe BPC 白皮书、Ghostscript、Gutenprint、cupsfilters、
    Tesseract、OCRopus、ITU-T T.81、Otsu 原文、Lazzara & Géraud、van de Weijer、DIBCO 报告、
    Pillow 的两条 PR、calibre 的 Launchpad 与身世 commit、ScanTailor PR #151 与
    scantailor-experimental #73、NiLuJe 的 MobileRead 帖、KCC 的 PR #26、
    **第三节的全部零命中检索记录**、**ICC §0.4 与 PostScript Language Reference 的引文**、
    **Xerox 的 ABS 专利**、**US 5,353,127（Shiau & Fan）的正文**、
    **视频编码那三条的题录与正文**。
    **引用未复核的那些之前建议再打开一次。**
