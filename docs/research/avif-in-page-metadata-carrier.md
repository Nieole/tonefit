# AVIF 的页内元数据载体：本仓这条依赖链上的现状调研

调研日期 **2026-09-20**。

**本文记录的是这条依赖链今天的能力边界，与那些结论各自的出处。**
它不含本项目的决定，也不提任何建议——AVIF 该不该进来、那道闸要怎么读，
都在本文范围之外（决定在 `docs/adr/`，本轮记下的岔路在 `.scratch/非阻塞问题.md`）。

**与同目录另外两篇的一处不同要先说明。**
`halftone-metrics-and-dither-prior-art.md` 与 `paper-white-grid-alignment-prior-art.md`
记的是**仓库之外**的现状；本篇做不到那么干净，因为票面把口径钉死在
「**我们手上这套依赖**编得进去、读得回来吗」——回答它就必须点出我们锁了哪几个版本、
怎么配的特性。因此本篇里**关于本仓的只有三处**：《零》交代锁定版本与特性配置、
《本轮怎么验的》交代探针、末节把外部事实对到那道闸上。**其余全是上游的事实。**
项目自己的数字在 `docs/measurements.md`，构建前提在 `docs/agents/gate.md` 的
《跑得起来需要什么》——本文引用它们的小节名，不复制。

**本篇只谈一件事：一段自定的字节挂不挂得进一张 AVIF 里、读不读得回来。**

**口径是这条链，不是格式规范。**问的不是「AVIF／HEIF 规范支不支持 Exif 与 XMP」
——那一问的答案是「支持」，写在规范里，查一遍就有。
两问的答案在本轮**不一致**，而不一致的那一处就是本篇的全部内容。

## 四个判断，一眼看完

| # | 问题 | 判断 | 怎么来的 |
|---|---|---|---|
| 一 | **写得进去吗** | **够用**——`set_exif_metadata` 收，写出标准形状。但编码器**只在 `[dev-dependencies]`**，且整条链**只有 Exif 项一个装任意字节的口子** | 📖🔬 |
| 二 | **读出来与写进去逐字节相同吗** | **只够一半**——字节逐字节在文件里，**这条链取不回来**；而且取不回来**不报错** | 📖🔬 |
| 三 | **`--no-metadata` 关得掉吗** | **够用**——不调就一个字节不落，像素不受影响 | 📖🔬 |
| 四 | **取值受不受 Latin-1 那一类约束** | **够用，而且比 PNG 宽**——三层都是裸字节，没有这类约束 | 📖🔬 |

**两条贯穿全文、值得单独记住的**：

1. **写侧只会 Exif，读侧只会 ICC，两头不相交**（第二节）。
2. **读不回来在这条链上没有任何声音**——`Ok(None)`，与「这张图本来就没有元数据」
   返回值一个字不差（第二节）。

## 怎么读这一篇

四节对应四个问题，另加一节《零》交代这条链是哪几个包、哪几个版本。
每节第一句是判断，只有三档，问的都是「**这一问上这条链够不够用**」：

- **够用**——这一问不构成障碍
- **只够一半**——一头成立、另一头不成立
- **不够用**——这一问上这条链做不到

**每条实质结论后面标着它是怎么来的**，只有两种：

- 🔬 **跑出来的**——本轮在本机真编真解、真取字节比出来的（方法见《本轮怎么验的》）
- 📖 **读源码答的**——读的是 `~/.cargo/registry` 里**本仓锁定版本**的那一份源码

引用源码给**包名、锁定版本、文件、行号与函数名**。行号会随上游漂移，**函数名是稳定引用**；
版本取自 `Cargo.lock`，不是 crates.io 上的最新——**本篇一切结论只对《零》那几个版本成立**。

**凡是我没验到的，写在末节《没查到的、没验到的》，不在正文里编圆。**
倒数第二节《这四问各自落在哪一档》是全文唯一一处把外部现状对到我们处境上的地方。

---

## 零、这条链是哪几个包、哪几个版本

📖 `Cargo.lock`（基线 `5d406be`）锁着的那几份：

| 位置 | 包 | 版本 | 在哪一侧 |
|---|---|---|---|
| 编码入口 | `image`（`avif` 特性） | **0.25.10** | **只在 `[dev-dependencies]`** |
| 编码实现 | `ravif` | **0.13.0** | 同上（`image` 的 `avif` 特性拉进来） |
| 容器打包 | `avif-serialize` | **0.8.9** | 同上（`ravif` 拉进来） |
| AV1 编码 | `rav1e` | **0.8.1** | 同上 |
| 解码入口 | `image`（`avif-native` 特性） | **0.25.10** | **`[dependencies]`，运行时就有** |
| 容器解析 | `mp4parse` | **0.17.0** | 同上 |
| AV1 解码 | `dav1d`（绑定） | **0.11.1** | 同上 |
| AV1 解码 | dav1d（系统库） | 见下 | 系统装 |

📖 `rav1e` 那一份的汇编特性**关着**：`ravif` 0.13.0 `Cargo.toml:77–79` 对它写死
`default-features = false`，而 `asm` 在 `rav1e` 的默认特性里——因此**本机没有 NASM 也编得过**。

构建前提与本机此刻满足它们的形态（含系统 dav1d 的版本），
**唯一出处是 `docs/agents/gate.md` 的《跑得起来需要什么》**，本文不复制。
那一节说的是**两样**（一套 C++ 编译器、一份 `pkg-config` 找得到的 dav1d），两样都不出自 Rust 工具链。

### 编与解两侧不对称，而这个不对称是配置造成的

📖 `image` 的两个 AVIF 特性各管一头，互不相干
（`image` 0.25.10 `Cargo.toml:61–68`，`src/codecs/avif/mod.rs` 按同两个特性各挂一半）：

```
avif        = ["dep:ravif", "dep:rgb"]        # 只给编码器 AvifEncoder
avif-native = ["dep:mp4parse", "dep:dav1d"]   # 只给解码器 AvifDecoder
```

🔬 本仓 `[dependencies]` 只收 `avif-native`，`avif` 写在 `[dev-dependencies]` 里。
**resolver v2 起不把 dev-dependencies 的特性并进非 dev 构建**，本轮当场验了这一句：

```
$ cargo tree --no-dev-dependencies -i ravif
warning: nothing to print.

$ cargo tree -i ravif
ravif v0.13.0
└── image v0.25.10
    └── tonefit v0.1.0
    [dev-dependencies]
    └── tonefit v0.1.0
```

即：**`cargo build` 出来的产物里没有 AVIF 编码器**，只有解码器；
`cargo test` 那一趟两个特性都在，本轮探针跑得起来正是靠这一点。

---

## 一、写得进去吗

**判断：够用。写得进去。但写那一头只在 dev 侧，且整条链上只有一个装任意字节的口子。**

### 那个口子：Exif 项

📖 `image` 0.25.10 的 `AvifEncoder` 实现了 `ImageEncoder::set_exif_metadata`
（`src/codecs/avif/encoder.rs:134`，`impl ImageEncoder for AvifEncoder<W>`），转手交给
`ravif::Encoder::with_exif`（`ravif` 0.13.0 `src/av1encoder.rs:202`），
再交给 `avif_serialize::Aviffy::set_exif`（`avif-serialize` 0.8.9 `src/lib.rs:408`）。
`with_exif` 的文档注释两句，头一句是 **"Embedded into AVIF file as-is"**，
第二句说的是参数类型（`Vec<u8>` 或 `&[u8]`，与第四节相干）。

🔬 四种载荷（90 字节 ASCII／47 字节非 Latin-1／空／64 KiB）调 `set_exif_metadata` **全部 `Ok`**，
写出的文件里**全部**带着 `Exif` 四字码。

### 同一个编码器上别的元数据口子一个都没有

📖🔬 **ICC**：`AvifEncoder` **没有**实现 `set_icc_profile`，落到 `ImageEncoder` 的默认实现
（`image` 0.25.10 `src/io/encoder.rs:53`），恒返回 `Err`。本轮五种情形下各调一次，
回话逐字相同：

> `The decoder does not support the format feature ICC profiles are not supported for this format`

再往下，`avif-serialize` 0.8.9 与 `ravif` 0.13.0 的源码里 **`icc` 零命中**
——这一头不是「`image` 没接上去」，是它底下根本没有。

📖 **XMP**：`ImageEncoder` 这个 trait 上**没有** `set_xmp_metadata` 这类方法，
`avif-serialize` 也没有造 XMP／`mime` 项的代码。

📖 **别的自定盒子**：`Aviffy` 的全部公开 setter（`avif-serialize` 0.8.9 `src/lib.rs`）
除 `set_exif` 之外**都是定宽的色彩与几何字段**——`set_matrix_coefficients`、
`set_transfer_characteristics`、`set_color_primaries`、`set_full_color_range`、
`set_content_light_level`、`set_mastering_display`、`set_chroma_subsampling`、
`set_monochrome`、`set_seq_profile`、`set_width`、`set_height`、`set_bit_depth`、
`set_premultiplied_alpha`——**一个都装不下任意字节**。
而 `make_boxes`（`src/lib.rs:200` 起）造出来的项**恰好三种**：
`av01`（彩色）、`av01`（alpha）、`Exif`；**没有 `uuid` 盒、没有 `free` 盒、没有 `mime` 项**。

**所以在这条链上，AVIF 的自定字节载体有且只有一个，就是 Exif 项。**

### 写出去的是标准形状

📖 `avif-serialize` 0.8.9 `make_boxes`（`src/lib.rs:267–282`）在 `self.exif.is_some()` 时推三样：

1. `iinf` 里一条 `infe`，四字码 **`Exif`**；
2. `iloc` 里一条项，段由 `exif_extents` 算（`src/lib.rs:465`，见第二节）；
3. `iref` 里一条 **`cdsc`**（content describes），从 Exif 项指回彩色图项。

`iloc` 的线格式写死在 `src/boxes.rs` 的 `impl MpegBox for IlocBox`：
version 0、`offset_size = length_size = 4`、`base_offset_size = 0`，偏移是**绝对文件偏移**。
`infe` 是 version 2，`iinf` 与 `meta` 都是 version 0 的 FullBox。
🔬 本轮的探针照这个形状自己走了一遍箱子，**逐项对上**（见《本轮怎么验的》）。

---

## 二、读出来与写进去的逐字节相同吗

**判断：只够一半。**
字节**逐字节还在文件里**（🔬 本轮自己走 ISOBMFF 取出来比过），
但**这条链上没有一处 API 取得回来**（📖 + 🔬）。

**这一节是本篇的全部要点。**

### 读那一头，`image` 只实现了 ICC 一项

📖 `image` 0.25.10 `src/codecs/avif/decoder.rs`，`impl ImageDecoder for AvifDecoder<R>` 里
**只有 `icc_profile()`**（第 342 行）。`exif_metadata()`、`xmp_metadata()`、`iptc_metadata()`
一个都没实现，全部落到 `ImageDecoder` 的默认实现
（`src/io/decoder.rs:30`、`:38`、`:45`），而那三个默认实现的整个函数体是
**`Ok(None)`**——不看文件一眼，无条件。

### 再往下一层，`mp4parse` 也没有

📖 `mp4parse` 0.17.0 的 `AvifContext`（`src/lib.rs:1596` 起 `impl AvifContext`）
公开方法一共十来个：`primary_item_coded_data`、`alpha_item_coded_data`、
`spatial_extents_ptr`、`nclx_colour_information_ptr`、**`icc_colour_information`**、
`image_rotation`、`image_mirror_ptr`、`pixel_aspect_ratio_ptr` 等等——
**一个 Exif／XMP 访问器都没有**。全文 grep `Exif` 只命中三行讲旋转语义的文档注释
（`src/lib.rs:4009–4011`）。

**所以这不是「`image` 忘了接上去」，是它底下没有东西可接。**

### 写侧只会 Exif，读侧只会 ICC，两头不相交

把上面两条并起来就是这条链的形状：

| | 写（dev 侧，ravif／avif-serialize） | 读（运行时，mp4parse／dav1d） |
|---|---|---|
| **Exif** | ✅ 写得进（`set_exif`） | ❌ 没有访问器 |
| **ICC** | ❌ 恒 `Err`，底下无代码 | ✅ 读得回（`icc_colour_information`） |
| **XMP** | ❌ 没有 | ❌ 没有 |

**两侧各有一样本事，而那两样不是同一样。**

### 读不回来不会以任何形式报错

🔬 这一条值得单独记下。四个访问器在**每一种**情形下的回话逐字相同：

| 文件 | `exif_metadata()` | `xmp_metadata()` | `iptc_metadata()` | `icc_profile()` |
|---|---|---|---|---|
| 挂着 90 字节记录 | `Ok(None)` | `Ok(None)` | `Ok(None)` | `Ok(None)` |
| 挂着 47 字节记录 | `Ok(None)` | `Ok(None)` | `Ok(None)` | `Ok(None)` |
| 挂着空记录 | `Ok(None)` | `Ok(None)` | `Ok(None)` | `Ok(None)` |
| 挂着 64 KiB 记录 | `Ok(None)` | `Ok(None)` | `Ok(None)` | `Ok(None)` |
| **一个记录都没挂** | `Ok(None)` | `Ok(None)` | `Ok(None)` | `Ok(None)` |

**返回的是 `Ok(None)` 而不是 `Err`。**
一张**挂着记录**的 AVIF 与一张**本来就没有记录**的 AVIF，在这条链的 API 上
**返回值一个字都不差**。读不回来这件事在这条链上**没有任何声音**——
它长得与「这张图本来就没有元数据」完全一样。

### 字节本身确实逐字节还在

🔬 探针自己走了一遍 ISOBMFF（`meta` → `iinf`/`infe` 找 `Exif` 项的 id → `iloc` 取它那几段
→ 按绝对偏移从文件里切出来），四种载荷全部对上：

| 载荷 | 载荷字节 | 文件字节 | 取回来的字节 | 取回来的 == `[4 字节零] ++ 载荷` |
|---|---|---|---|---|
| ① ASCII 记录 | 90 | 458 | **94** | ✅ |
| ② 非 Latin-1 | 47 | 415 | **51** | ✅ |
| ③ 空 | 0 | 368 | **4** | ✅ |
| ④ 64 KiB | 65536 | 65904 | **65540** | ✅ |

### 那 4 字节不是恒定的——`exif_extents` 有两支

📖 多出来的 4 字节是 HEIF 的 `ExifDataBlock` 要求的 `exif_tiff_header_offset` 字段，
标准形状，不是损坏。但**它加不加，取决于载荷自己长什么样**
（`avif-serialize` 0.8.9 `src/lib.rs:465` 的 `exif_extents`）：

- 载荷**看起来已经是框好的 HEIF Exif 项**——把头 4 字节当大端偏移 `V` 读，
  跳到 `4+V` 处正好是 TIFF 头（`II\x2a\0` 或 `MM\0\x2a`，见 `looks_like_tiff_header`
  与 `looks_like_heif_exif_item`，`src/lib.rs:478`）→ **写成一段，原样**，
  取回来与载荷**逐字节相同、不多 4 字节**；
- 否则 → 写成**两段**：先 `EXIF_TIFF_OFFSET_ZERO`
  （`const EXIF_TIFF_OFFSET_ZERO: [u8; 4] = 0_u32.to_be_bytes()`，`src/lib.rs:19`），
  再把载荷原样接上 → 取回来比载荷**多那 4 字节**。

🔬 **本轮四种载荷全部落在第二支**，因此上表恒多 4 字节；
**第一支本轮没有实测**（见末节）。

**因此「取回来要不要剥掉头 4 字节」在这条链上不是一个定值，它取决于写进去的那段字节本身。**
一个无条件剥 4 字节的读回来实现，在第一支上会剥错。

🔬 另外：这四张挂着记录的 AVIF **都解得开**，`AvifDecoder::dimensions()` 全部答 `(16, 16)`，
像素照常读得出来。**挂上记录不影响这条链解这张图**——mp4parse 不因为多一个
不认识的项就拒绝整个文件。

---

## 三、`--no-metadata` 那条路照旧关得掉吗

**判断：够用。关得掉，而且比 PNG 那一侧还干脆。**

📖 `avif-serialize` 0.8.9 `make_boxes` 里那三样（`infe`／`iloc`／`iref`）整个包在
`if let Some(exif_data) = self.exif.as_deref()` 里（`src/lib.rs:267`）。不调
`set_exif_metadata`，`self.exif` 就是 `None`，三样一个都不写——
**不是写一个空的，是一个字节都不落。**

🔬 同一批像素、不调 `set_exif_metadata` 编出来的那一张：

- 文件里**没有** `Exif` 四字码；
- 自己走 ISOBMFF 找 Exif 项 → **找不到**；
- 文件 **295 字节**，而挂了 90 字节记录那一张是 **458 字节**
  （差 163 字节 = 94 字节存进去的 + 69 字节盒子开销）；
- **两张解出来的像素逐字节相同**——挂不挂记录不动一个像素。

---

## 四、取值受不受 Latin-1 那一类约束

**判断：够用，而且比 PNG 宽。这条链上没有这类约束——载体在三层上都是不透明的字节串。**

### 类型一路都是裸字节

📖 `ImageEncoder::set_exif_metadata(&mut self, exif: Vec<u8>)`
→ `ravif::Encoder::with_exif(impl Into<Cow<[u8]>>)`
→ `Aviffy::set_exif(exif: Vec<u8>)`。
**没有一层收 `String`**，因此没有一层谈得上字符集。

### 唯一看载荷内容的那一处只是在嗅，不是在校验

📖 `avif-serialize` 唯一读载荷的地方就是第二节那个 `exif_extents`，
而它只**嗅**头几个字节来决定分几段，**两支都不拒绝、都不改字节**。

🔬 本轮拿一段**故意难看**的载荷验了这一条：UTF-8 中文（`记录：改革之獸／第 001 卷`）
＋ 内嵌 `NUL` ＋ `0x01 0x7f 0x80 0xfe 0xff` ＋ 再一段 UTF-8 中文，共 47 字节。
`set_exif_metadata` → `Ok`，写进去、取回来**逐字节相同**。

### 对照组：PNG 的 `tEXt` 在同一段字节上说什么

🔬 同一轮里拿 `png` 0.18.1 跑了对照：

| 取值 | `add_text_chunk` | **真写出去** |
|---|---|---|
| ASCII 记录（90 字节） | `Ok` | `Ok`，183 字节 |
| 纯 Latin-1（`café ÿ ` ，全在 U+00FF 以内） | `Ok` | `Ok`，101 字节 |
| 非 Latin-1 那一段（UTF-8＋NUL＋高位） | `Ok` | **`Err`** |
| `改革之獸` | `Ok` | **`Err`** |

拒绝的原话：

> `The text metadata cannot be encoded into valid ISO 8859-1`

📖 **拒绝不在 `add_text_chunk` 那一步。**`png` 0.18.1 的
`Encoder::add_text_chunk`（`src/encoder.rs:416`）只是 `TEXtChunk::new` 之后 push 进 `Info`
并 `Ok(())`，**一个字都不校验、也不可能失败**。真正的校验在写出那一刻：
`impl EncodableTextChunk for TEXtChunk` 的 `encode`（`src/text_metadata.rs:216`）调
`encode_iso_8859_1`（`:161`），而它对每个 `char` 做 `u8::try_from(c as u32)`，
超出 U+00FF 就是 `TextEncodingError::Unrepresentable`。

**两件事因此分得清了：**

1. **「取值一律 Latin-1」那个约束是 PNG `tEXt` 的，不是「页内元数据」这件事本身的。**
   同一段字节 AVIF 的 Exif 项照单全收、逐字节保得住，PNG 的 `tEXt` 当场拒绝。
   （本项目在 Latin-1 之上又收窄到 ASCII，那条规矩写在 `src/metadata.rs` 的模块文档里，
   不在本篇范围。）
2. **`tEXt` 的上界是 Latin-1（U+00FF），不是 ASCII。**`café ÿ` 写得进去。

### 树里没有 Exif 标签级的写入器

📖 `Cargo.lock` 里没有 `kamadak-exif` 或同类的包。
这条链给的是一个**装字节的口袋**，不是一个「写 Exif 标签」的库——
口袋里装什么、要不要装成合法 TIFF／Exif，全在调用方。
🔬 本轮四种载荷里没有一种是合法 Exif，四种都照收不误。

---

## 本轮怎么验的

一次性探针，源码与跑法在 `.scratch/encoder-gate/probe/`（同目录 `README.md` 说明怎么跑，
以及它为什么不在 `tests/` 下；那个判断记在 `.scratch/非阻塞问题.md` 的 **Q956**）。

它做了五件事：

1. 用 `image::codecs::avif::AvifEncoder::new_with_speed_quality(w, 10, 90)`
   编一张 16×16 的 RGB8 灰阶斜坡，分别带四种载荷与不带载荷；
2. 每一张上都调一次 `set_icc_profile`，记下回话；
3. **自己走一遍 ISOBMFF**——顶层箱 → `meta`（FullBox，跳 4 字节）→ `iinf` 里逐条 `infe`
   找四字码 `Exif` 的那个 item id → `iloc` 里取它那几段（version 0、4/4/0 的宽度）
   → 按**绝对文件偏移**从文件字节里切出来 → 与写进去的载荷逐字节比；
4. 用 `image::codecs::avif::AvifDecoder`（**运行时那一侧**）解同一张，
   问 `exif_metadata()`／`xmp_metadata()`／`iptc_metadata()`／`icc_profile()` 四个访问器，
   并读一遍像素；
5. 拿 `png` 0.18.1 在同样几段字节上做对照，**并且真把 PNG 写出去**
   （只调 `add_text_chunk` 什么都测不到，见第四节）。

**第 3 步是这一轮的关键手法。**读那一头在链上不存在，因此「读回来逐字节相同吗」
本来问不出口；自己走箱子把它拆成了两问，而两问的答案不同：
**字节在不在文件里**（在，逐字节）与**这条链取不取得回来**（取不回来）。
这两个答案对那道闸的意味完全不同，含混成一句「读不回来」就丢掉了这个区别。

---

## 这四问各自落在哪一档

| # | 问 | 档 | 怎么来的 |
|---|---|---|---|
| ① | **写得进去吗** | **够用**——写得进。但写那一头只在 `[dev-dependencies]` 那一半依赖里，产物里没有；且整条链只有 Exif 项一个装任意字节的口子 | 📖🔬 |
| ② | **读出来与写进去逐字节相同吗** | **只够一半**——字节逐字节在文件里；这条链读不回来。而且读不回来**不报错**，与「本来就没有元数据」返回值逐字相同 | 📖🔬 |
| ③ | **`--no-metadata` 关得掉吗** | **够用**——不调就一个字节不落，像素不受影响 | 📖🔬 |
| ④ | **取值受不受 Latin-1 那一类约束** | **够用，比 PNG 宽**——载体三层都是裸字节，UTF-8、内嵌 NUL、`0x80~0xFF` 全部原样收下。那个约束是 PNG `tEXt` 的 | 📖🔬 |

**对着 ADR 0009 那道闸读**（「一个编码器进不了这个项目，除非它有页内元数据载体」）：

- **①③④ 三问在这条链上都够用**，而且 ④ 比 PNG 那一侧更宽松；
- **卡在 ②**，且卡的是很具体的一格：**不是字节挂不上去**（挂得上，逐字节保得住），
  **是这条链上没有一处现成 API 把它取回来。**

**「载体」这个词今天要求的是哪一格成立——「字节挂得住」还是「链上现成地读得回」——
`ADR 0009` 与 `CONTEXT.md` 的《记录》都没有把它拆开写过。**
本篇只交出上表那四格；那一问不在本篇范围，记在 `.scratch/非阻塞问题.md` 的 **Q958**。

---

## 没查到的、没验到的

1. **上游将来会不会补上。**本篇能证明的是
   「`mp4parse` 0.17.0 与 `image` 0.25.10 上没有 Exif 访问器」，
   **证明不了「以后也不会有」**。我没去翻 mp4parse 或 `image` 的 issue／PR 与路线图
   （本机对外网的可用性本轮没有稳定确认，crates.io 的 API 返回 403）。
   要赌上游补齐，得自己再去看一次。
2. **`exif_extents` 的第一支没有实测**（第二节）。本轮四种载荷全部落在第二支，
   「载荷本身已经是框好的 HEIF Exif 项 → 原样一段、不多 4 字节」这一支
   **只读了源码，没有造一个真的 TIFF 头载荷去跑**。
3. **PNG `tEXt` 里单独一个 NUL 会怎样，没有单独验。**本轮载荷 ② 把内嵌 NUL 与 UTF-8 中文
   混在一起，写出去时的那个 `Err` **出自中文，不出自 NUL**。
   📖 按 `encode_iso_8859_1`（`png` 0.18.1 `src/text_metadata.rs:161`）的写法，
   `u8::try_from(0u32)` 是 `Ok(0)`，**NUL 不会被它拒绝**；
   写出去之后那个 `tEXt` 块合不合规、别的读者怎么处理，本轮没有验。
4. **别的 AVIF 解析器能不能读出这些字节，没验。**本机上
   `exiftool`、`exiv2`、`avifdec`、`ffprobe` **一个都没有**，本地 Pillow 是 10.2.0（无 AVIF 支持）。
   「写出来的是标准形状」这一条的依据是**源码与自己走箱子对上**，
   **不是第三方工具认了它**。
5. **XMP 那条路没验**——因为链上没有可验的东西（第一节：没有构造 XMP／`mime` 项的代码）。
   「AVIF 规范支持 XMP」这句话在本篇范围之外。
6. **绕开 `image` 直接用 `ravif`／`avif-serialize` 没有另验。**
   `image` 的 `set_exif_metadata` 只是一层转发（第一节给了三跳），
   直接调不会改变任何一格结论；但**自己拼一个 `uuid`／`mime` 项**这条路
   （要么改 `avif-serialize`、要么自己拼 ISOBMFF）本轮**没有走过**。
7. **像素那一头一个数都没量。**本篇用的是 16×16 的合成斜坡，
   `speed 10 / quality 90` 只是为了跑得快。**AVIF 编完解回来还剩多少级灰不在本篇**
   ——那是另一格测量，口径见 `docs/measurements.md` 的《有损转存与灰调级数》。
8. **多张 Exif 项、或 Exif 加 alpha 一起在场时的形状，没验。**
   本轮每张图只有一个 Exif 项、没有 alpha。
9. **64 KiB 之上没试。**`iloc` 在这条链上是 4 字节偏移 4 字节长度
   （📖 `src/boxes.rs` 的 `IlocBox::write`），因此上界在 4 GiB 一侧，
   但**中间那一大段没有实测**。
