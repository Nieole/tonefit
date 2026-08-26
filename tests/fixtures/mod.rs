//! 合成夹具生成器。
//!
//! 仓库里不放真实漫画素材：测试用的页全部由本模块按代码生成。
//! 每一类页对应一条待验证的性质，见调用它的用例。

// 每个测试二进制都单独编一份夹具，各自只用得上其中一部分。
#![allow(dead_code, unused_imports)]

mod cbz;

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageBuffer, Luma, Rgb, Rgba};
use tonefit::{BitDepth, Dither, GrayImage, Profile, Size};

pub use cbz::{Cbz, read_cbz};

/// B 类素材的中位尺寸（见 measurements 的《B 类素材普查》）。缩放比含小数。
pub const TYPICAL: Size = Size::new(1441, 2048);

/// 正好两倍面板，缩放后的尺寸没有取整歧义。总缩放比 2.000：预缩一步到位，残差比 1.000。
pub const DOUBLE_PANEL: Size = Size::new(2528, 3360);

/// 面板的 2.5 倍。总缩放比 2.500：预缩 2 之后残差段还剩 1.250，两级各真跑一次。
pub const TWO_AND_A_HALF_PANEL: Size = Size::new(3160, 4200);

/// 宽幅跨页：宽高比远超面板，fit-inside 由宽边定夺。
pub const SPREAD: Size = Size::new(5056, 1680);

/// 两边都小于面板：不该被放大。
pub const SMALLER_THAN_TARGET: Size = Size::new(800, 1000);

/// 页数多的用例用的小页。卷级的性质只看逐页判定排开之后的分布，与页上有什么内容无关，
/// 页因此小到只够铺开几块判据分块就行。
pub const TINY: Size = Size::new(160, 224);

/// 彩页的色带，自上而下。测试按序号取样。
///
/// 蓝与灰这一对是有意的：Rec.601/709 加权会把纯蓝压到 18~29，与第二条灰带混同；
/// OKLab 的 L 通道把它抬到 86 左右，两带保持可分。
pub const COLOR_BANDS: [[u8; 3]; 6] = [
    [0, 0, 255],     // 纯蓝
    [18, 18, 18],    // 深灰：纯蓝在 Rec.709 线性加权下的落点
    [255, 0, 0],     // 纯红
    [0, 255, 0],     // 纯绿
    [255, 255, 255], // 白
    [0, 0, 0],       // 黑
];

/// 连续渐变页：竖直方向 0→255 的线性斜坡，无边缘。
pub fn gradient(size: Size) -> DynamicImage {
    let last = (size.height - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |_, y| {
        Luma([(y * 255 / last) as u8])
    }))
}

/// 二值网点页：只有 0 与 255 两个取值，点的大小随横向位置由小到大。
///
/// 用经典 8×8 聚集点阈值矩阵对横向斜坡二值化——网点是因，灰调是果，
/// 这页故意只提供「因」，让缩放去解析出「果」。
pub fn screentone(size: Size) -> DynamicImage {
    const CLUSTERED: [[u8; 8]; 8] = [
        [24, 10, 12, 26, 35, 47, 49, 37],
        [8, 0, 2, 14, 45, 59, 61, 51],
        [22, 6, 4, 16, 43, 57, 63, 53],
        [30, 20, 18, 28, 33, 41, 55, 39],
        [34, 46, 48, 36, 25, 11, 13, 27],
        [44, 58, 60, 50, 9, 1, 3, 15],
        [42, 56, 62, 52, 23, 7, 5, 17],
        [32, 40, 54, 38, 31, 21, 19, 29],
    ];
    let last = (size.width - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        let tone = x * 255 / last;
        let threshold = u32::from(CLUSTERED[(y % 8) as usize][(x % 8) as usize]) * 4 + 2;
        Luma([if tone > threshold { 255 } else { 0 }])
    }))
}

/// 线稿页：白底黑线，硬边不抗锯齿。高对比度墨线，两端取值必然到底。
pub fn line_art(size: Size) -> DynamicImage {
    let (w, h) = (size.width, size.height);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(w, h, |x, y| {
        let border = x < 6 || y < 6 || x + 6 >= w || y + 6 >= h;
        let divider = y.abs_diff(h / 2) < 3;
        let diagonal = (x * h).abs_diff(y * w) < 3 * w;
        let back_diagonal = (x * h + y * w).abs_diff(w * h) < 3 * w;
        Luma([if border || divider || diagonal || back_diagonal {
            0
        } else {
            255
        }])
    }))
}

/// 一页留白，左上角一块 `patch` 大的灰调补丁——低位深下唯一会崩的就是这块。
///
/// 补丁竖直方向 0→255，与 [`gradient`] 同一条斜坡，只是圈在一小块里。
/// 页上其余部分是白的：白在任何位深上都是格点，误差恒为零，
/// 这一页的判据于是完全由那一小块说了算。
pub fn tone_patch(size: Size, patch: Size) -> DynamicImage {
    let last = (patch.height - 1).max(1);
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        Luma([if x < patch.width && y < patch.height {
            (y * 255 / last) as u8
        } else {
            255
        }])
    }))
}

/// 纯色页：全图同一个灰度取值。
pub fn solid(size: Size, level: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |_, _| {
        Luma([level])
    }))
}

/// 彩页：`COLOR_BANDS` 自上而下的等高色带。
pub fn color_page(size: Size) -> DynamicImage {
    let bands = COLOR_BANDS.len() as u32;
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(size.width, size.height, |_, y| {
        let band = (y * bands / size.height).min(bands - 1) as usize;
        Rgb(COLOR_BANDS[band])
    }))
}

/// 截断的页：一张纯黑页的完整 PNG，只留前 `KEPT_FRACTION` 那一段。
///
/// 文件头与 IHDR 都在，IDAT 只剩一截：完整尺寸解得出来，像素解不全，但救得回一段。
/// 这是**部分救回页**——三种页状态里的第三种（04 号票）：它照常缩放、判定、写出，
/// 但不参与几何门与卷级上包络。
///
/// 纯黑是为了让两段一眼分得开：解回来的那一段是 0，救不回来的那一段是纸白 255。
pub fn truncated_page(size: Size) -> Vec<u8> {
    truncated(&solid(size, 0))
}

/// 同上，但页上画什么由调用方定：判定要跟着救回来的那一段走的用例需要它。
pub fn truncated(image: &DynamicImage) -> Vec<u8> {
    let bytes = encode_image(image, "png");
    bytes[..bytes.len() * KEPT_FRACTION / 100].to_vec()
}

/// 截断的页留下的那一段占原文件的百分之多少。
///
/// 取一半是为了让「解出来的那部分」与「缺的那一段」两侧都真的存在：
/// 留太多，整页都解得回来，留白那一段测不到；留太少，连一行都解不出来。
const KEPT_FRACTION: usize = 50;

/// 一行像素都救不回来的页：完整的文件头与 IHDR，第一个 IDAT 只剩块头。
///
/// 尺寸解得出来，救回那一趟一个像素都没写下——这正是 04 号票要拦的那一张：
/// 按 12 号票的界线（「解得出完整尺寸就照用」）它是一张正常页，写出去是一整张纸白，
/// 带着正常的判定元数据，卷还留在干净的去处。
///
/// 砍在第一个 IDAT 的块头上，而不是按比例砍：能不能救回取决于砍在哪里，不取决于比例
/// （见 measurements 的《截断页的解码容忍度》）。按比例砍出来的「零救回」会随
/// 编码器的输出长度漂，砍在块头上则是个定值。
pub fn salvages_nothing_page(size: Size) -> Vec<u8> {
    let bytes = encode_image(&solid(size, 0), "png");
    let mut at = PNG_SIGNATURE;
    loop {
        let length = u32::from_be_bytes(
            bytes[at..at + 4]
                .try_into()
                .expect("PNG 块头有四个字节的长度"),
        ) as usize;
        if &bytes[at + 4..at + 8] == b"IDAT" {
            // 留下块头、丢掉块身：解码器读得到「这里有一块数据」，读不到数据本身。
            return bytes[..at + 8].to_vec();
        }
        // 长度、类型、块身、CRC。
        at += 12 + length;
    }
}

/// PNG 文件签名的长度。块从它之后开始。
const PNG_SIGNATURE: usize = 8;

/// 尺寸大到解码器分配不出缓冲的页：只有文件头与 IHDR，横竖各 65535。
///
/// 像素缓冲要 4 GiB，越过解码器的分配上界。尺寸解得出来，这一页仍然算失败——
/// 分配不出来的页救不回任何像素（见 `decode::salvage`）。
pub fn oversized_page() -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(b"IHDR");
    header.extend_from_slice(&u32::from(u16::MAX).to_be_bytes());
    header.extend_from_slice(&u32::from(u16::MAX).to_be_bytes());
    // 位深 8、灰度、默认压缩与滤波、非隔行。
    header.extend_from_slice(&[8, 0, 0, 0, 0]);

    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend_from_slice(&(header.len() as u32 - 4).to_be_bytes());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&crc32fast::hash(&header).to_be_bytes());
    bytes
}

/// 带透明区的页：左半不透明黑，右半全透明——而 RGB 仍是黑。
/// 丢 alpha 会让右半出成黑，按纸白合成才出成白。
pub fn page_with_transparency(size: Size) -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(size.width, size.height, |x, _| {
        if x < size.width / 2 {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    }))
}

/// 色带 `band` 的纵向中心行，供测试取样。
pub fn band_center_row(size: Size, band: usize) -> u32 {
    let bands = COLOR_BANDS.len() as u32;
    (band as u32 * 2 + 1) * size.height / (bands * 2)
}

/// 一次测试的工作区：源卷与输出根目录分处两地，互不嵌套。
pub struct Workspace {
    tmp: tempfile::TempDir,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().expect("建临时目录"),
        }
    }

    /// 建一个空的目录卷。
    pub fn volume(&self, name: &str) -> Volume {
        Volume::new(self.tmp.path().join(name))
    }

    /// 建一个空的 CBZ 卷。加完成员要调 `Cbz::write` 才落盘。
    pub fn cbz(&self, name: &str) -> Cbz {
        Cbz::new(self.tmp.path().join(format!("{name}.cbz")))
    }

    /// 在工作区根下写一个不属于任何卷的文件。用来造「扩展名像卷、内容不是」的输入。
    pub fn stray_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.tmp.path().join(name);
        fs::write(&path, bytes).expect("写工作区文件");
        path
    }

    /// 输出根目录。此刻还不存在，由被测代码建出来。
    pub fn out(&self) -> PathBuf {
        self.tmp.path().join("out")
    }

    /// 另起一个输出根，与 [`out`](Self::out) 并列。同一个卷跑两趟、比两份输出的用例用它——
    /// 换个工作区跑第二趟就得把卷也生成两份，那时比的就不只是这两趟的差别了。
    pub fn out_named(&self, name: &str) -> PathBuf {
        assert_ne!(name, "out", "另起的输出根不该与默认那个撞名");
        self.tmp.path().join(name)
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

/// 同一档位深上不抖动的那个候选。几何门不成立的卷只有这一种候选可选。
pub const fn plain(bit_depth: BitDepth) -> tonefit::Candidate {
    tonefit::Candidate::new(bit_depth, Dither::Off)
}

/// 基准设备：`CONTEXT.md` 里阈值标定的那台。不点名 profile 的用例都用它。
pub const BASELINE_DEVICE: &str = "kobo-libra-2";

/// 按型号名取 profile。型号必须在内置表里，不在就是夹具写错了。
pub fn profile(device: &str) -> Profile {
    Profile::resolve(device).unwrap_or_else(|error| panic!("解析 profile {device}：{error}"))
}

/// 基准设备的 profile。自己拼 `Request` 的用例用它填 profile 那一项。
pub fn baseline_profile() -> Profile {
    profile(BASELINE_DEVICE)
}

/// 与基准面板等大的页尺寸：源即目标，不缩放，几何门两条边都贴住，
/// 也就是这台 profile 输出得到的**最大尺寸**。
///
/// 判据的分块聚合按**目标尺寸**铺开块数，「多小的损伤读得出来」因此只在真实输出尺寸上
/// 问得准（ADR 0002 的《第 3 条为什么改过》）。从 profile 推出而不写死：
/// 面板表改了，用它的夹具跟着走。
pub fn panel_sized() -> Size {
    baseline_profile().panel().resolution
}

/// 对一个目录卷调用被测的 `run`，用基准设备的 profile。
pub fn run_volume(space: &Workspace, volume: &Volume) -> tonefit::Report {
    run_volume_with(space, volume, baseline_profile())
}

/// 同上，但点名 profile。
pub fn run_volume_with(space: &Workspace, volume: &Volume, profile: Profile) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        profile,
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 把输出钉在 8bit 再跑一遍：量化成了恒等，写出的就是它之前那一步的结果。
///
/// 量重采样、解码或转灰的用例用这个。判定位深会把取值压到那一档的格点上，
/// 混进来就分不清一处差异出自哪一步——而那几条性质说的都不是量化。
///
/// 三个开关都是用户手上真有的：抬上界走 ADR 0003 给的 `--gray-levels`，点名位深走
/// `--bit-depth`，点名不抖动走 `--dither`。抖动在 8bit 上本来就是恒等（格点即工作精度），
/// 点名它是为了把候选裁到只剩一个——判定于是整个被顶掉，报告里那一项也就没有歧义。
pub fn run_volume_at_eight_bits(space: &Workspace, volume: &Volume) -> tonefit::Report {
    tonefit::run(&tonefit::Request {
        profile: baseline_profile().with_gray_levels(256).expect("全集可用"),
        bit_depth: Some(BitDepth::Eight),
        dither: Some(Dither::Off),
        ..request(space, [volume.path()])
    })
    .expect("处理应当成功")
}

/// 点名若干卷跑一遍，用基准设备的 profile。目录与归档都走这里。
pub fn run_paths<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> tonefit::Report {
    tonefit::run(&request(space, inputs)).expect("处理应当成功")
}

/// 同上，但预期失败，返回那个错误。
pub fn run_paths_expecting_failure<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> anyhow::Error {
    tonefit::run(&request(space, inputs)).expect_err("处理应当失败")
}

/// 拼一个用基准 profile、输出到工作区 `out/` 的 `Request`。
/// 换 profile 或要复用同一个 `Request` 的用例自己拼。
pub fn request<'a>(
    space: &Workspace,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> tonefit::Request {
    tonefit::Request {
        inputs: inputs.into_iter().map(Path::to_path_buf).collect(),
        output_root: space.out(),
        profile: baseline_profile(),
        filter: tonefit::Filter::default(),
        bit_depth: None,
        dither: None,
        per_page: false,
        cache_budget: tonefit::CacheBudget::default(),
        mode: tonefit::Mode::Process,
        io_mode: tonefit::IoMode::default(),
        progress: None,
        metadata: true,
    }
}

/// 一个卷：磁盘上装着页的目录。
pub struct Volume {
    root: PathBuf,
}

impl Volume {
    /// 建目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        fs::create_dir_all(&root).expect("建卷目录");
        Self { root }
    }

    /// 写一页，格式由 `name` 的扩展名决定。返回写出的路径。
    pub fn page(&self, name: &str, image: &DynamicImage) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建页所在目录");
        }
        write_image(&path, image);
        path
    }

    /// 写一个非图片文件。名字带目录就把目录一起建出来，与 [`page`](Self::page) 同形。
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建文件所在目录");
        }
        fs::write(&path, bytes).expect("写非图片文件");
        path
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

fn write_image(path: &Path, image: &DynamicImage) {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    fs::write(path, encode_image(image, &extension)).expect("写夹具页");
}

/// 把一张图编成 `extension` 指定的格式。归档夹具的成员也走这里，因此编出的是字节。
pub fn encode_image(image: &DynamicImage, extension: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    if extension == "avif" {
        // 默认的 speed 4 编一页要几秒；夹具只需要一个能解的 AVIF。
        let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut bytes, 10, 90);
        image.write_with_encoder(encoder).expect("编 AVIF");
        return bytes.into_inner();
    }
    let format = image::ImageFormat::from_extension(extension)
        .unwrap_or_else(|| panic!("不认识的夹具格式 {extension}"));
    if format == image::ImageFormat::Gif {
        // GIF 编码器只收 RGB/RGBA。灰度页转成 RGB 再写，解回来 r==g==b，灰度取值不变。
        image
            .to_rgb8()
            .write_to(&mut bytes, format)
            .expect("编 GIF 夹具页");
    } else {
        image.write_to(&mut bytes, format).expect("编夹具页");
    }
    bytes.into_inner()
}

/// 一页的判定。
///
/// 两种页没有判定：彩色分支上的（那条路径不量化，ADR 0005 决定第 4 条），
/// 以及失败页（它根本没解出来，12 号票）。取它就是用例写错了：
/// 要么点错了页，要么该断言的是 `verdict()` 为空。
pub fn verdict(page: &tonefit::PageReport) -> tonefit::Verdict {
    page.verdict().unwrap_or_else(|| {
        panic!(
            "{} 没有判定：它要么走的彩色分支，要么是失败页",
            page.source.display()
        )
    })
}

/// 解出的 PNG，供测试直接看编码结果而不经被测代码。
///
/// `color_type` 与 `bit_depth` 是文件里**写着**的那一对；`pixels` 一律摊回 8 位灰度，
/// 好让断言只谈灰度取值，不必跟着颜色类型分叉。
pub struct DecodedPng {
    pub size: Size,
    pub color_type: png::ColorType,
    pub bit_depth: png::BitDepth,
    pub pixels: Vec<u8>,
}

impl DecodedPng {
    pub fn pixel(&self, x: u32, y: u32) -> u8 {
        self.pixels[(y * self.size.width + x) as usize]
    }
}

/// 用 `png` crate 直接读回文件，绕开被测的编码路径。
pub fn read_png(path: &Path) -> DecodedPng {
    read_png_bytes(&fs::read(path).expect("读输出 PNG"))
}

/// 同上，直接从一页 PNG 的字节里读。归档卷的页没有文件系统路径，成员字节走这里——
/// 与 [`read_png_text`] 和 [`png_text`] 是同一种分工，读的是同一批字节。
pub fn read_png_bytes(bytes: &[u8]) -> DecodedPng {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    let header = decoder.read_header_info().expect("读 PNG 头").clone();
    // EXPAND 把低位深灰度按满量程摊回 8 位，把调色板摊成 RGB8。
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("读 PNG 信息");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
    let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
    pixels.truncate(info.buffer_size());
    if header.color_type == png::ColorType::Indexed {
        // 色板项恒是三个相等的分量（见被测的 `encode`），取一个就是灰度取值。
        pixels = pixels
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect();
    }
    DecodedPng {
        size: Size::new(info.width, info.height),
        color_type: header.color_type,
        bit_depth: header.bit_depth,
        pixels,
    }
}

/// 解出的彩色 PNG，供测试直接看彩色分支的编码结果。
///
/// 与 [`DecodedPng`] 分开：那一个把像素摊回灰度，专为「断言只谈灰度取值」；
/// 彩色这一侧要断言的恰恰是三个分量各是多少。
pub struct DecodedColorPng {
    pub size: Size,
    pub color_type: png::ColorType,
    pub bit_depth: png::BitDepth,
    /// 每像素三字节，RGB。调色板已摊开。
    pub pixels: Vec<u8>,
}

impl DecodedColorPng {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let start = ((y * self.size.width + x) * 3) as usize;
        [
            self.pixels[start],
            self.pixels[start + 1],
            self.pixels[start + 2],
        ]
    }
}

/// 用 `png` crate 直接读回一个彩色 PNG，绕开被测的编码路径。
pub fn read_color_png(path: &Path) -> DecodedColorPng {
    let file = fs::File::open(path).expect("打开输出 PNG");
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    let header = decoder.read_header_info().expect("读 PNG 头").clone();
    // EXPAND 把调色板摊成 RGB8。
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("读 PNG 信息");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
    let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
    pixels.truncate(info.buffer_size());
    DecodedColorPng {
        size: Size::new(info.width, info.height),
        color_type: header.color_type,
        bit_depth: header.bit_depth,
        pixels,
    }
}

/// PNG 头里写着的每像素比特数。`png::BitDepth` 的判别值就是比特数本身。
pub fn written_bits(depth: png::BitDepth) -> u32 {
    u32::from(depth as u8)
}

/// 一个目录容器里的成员名清单，按名字排序，分隔符归一成 `/`。
///
/// 归档那一侧的清单按**写入顺序**（见 `container.rs` 的 `member_names`），目录没有顺序，
/// 只有排序才比得出「输出里有什么」。断言「输出里只剩本趟的产物」用它。
pub fn directory_members(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|entry| !missing_root(entry))
        .map(|entry| entry.expect("遍历目录"))
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| relative_name(root, entry.path()))
        .collect();
    names.sort();
    names
}

/// 遍历的头一条就是「这个根不在」吗。
///
/// 目录还没建出来就是「里面什么都没有」，问「此刻输出里有什么」的用例正要这个答案。
/// **只放过这一种**：遍历途中的错误照旧当场炸（与 [`fingerprint`] 同一个口径），
/// 一律吞掉的话，一次 IO 抖动就能让「输出里只剩本趟的产物」假性通过。
fn missing_root(entry: &walkdir::Result<walkdir::DirEntry>) -> bool {
    let Err(error) = entry else { return false };
    error.depth() == 0
        && error
            .io_error()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
}

/// 目录内容的指纹：相对路径 + 每个文件的字节哈希。用来断言源目录未被改动。
pub fn fingerprint(root: &Path) -> Vec<(String, String)> {
    let mut entries: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.expect("遍历目录"))
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let relative = e
                .path()
                .strip_prefix(root)
                .expect("相对路径")
                .to_string_lossy()
                .into_owned();
            let bytes = fs::read(e.path()).expect("读文件");
            (relative, blake3::hash(&bytes).to_hex().to_string())
        })
        .collect();
    entries.sort();
    entries
}

/// 高频纹理页：`base` 上下各 `amplitude` 的逐像素交替。
///
/// 局部均值恒为 `base`，高频能量却拉满——判据的掩蔽加权该按后者放宽。
/// 取值不触顶不触底，加一个偏移上去不会被截断，加权方向因此可以单独测。
pub fn fine_texture(size: Size, base: u8, amplitude: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(ImageBuffer::from_fn(size.width, size.height, |x, y| {
        Luma([if (x + y) % 2 == 0 {
            base.saturating_add(amplitude)
        } else {
            base.saturating_sub(amplitude)
        }])
    }))
}

/// 把夹具页转成判据吃的 8 位灰度缓冲。夹具造的都是灰度页，取 luma 即可。
pub fn gray_image(image: &DynamicImage) -> GrayImage {
    let luma = image.to_luma8();
    GrayImage::new(Size::new(luma.width(), luma.height()), luma.into_raw())
}

/// 一页输出 PNG 里的 tEXt 记录：关键字到取值，按文件里的顺序。
///
/// 用 `png` 直接读，绕开被测的写入路径。归档卷的页没有文件系统路径，成员字节走
/// [`png_text`]——两者读的是同一批字节。
pub fn read_png_text(path: &Path) -> Vec<(String, String)> {
    png_text(&fs::read(path).expect("读输出 PNG"))
}

/// 同上，直接从一页 PNG 的字节里读。
pub fn png_text(bytes: &[u8]) -> Vec<(String, String)> {
    let reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .expect("读 PNG 信息");
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .map(|chunk| (chunk.keyword.clone(), chunk.text.clone()))
        .collect()
}

/// 一页 tEXt 记录里某个关键字的取值。没有这个关键字就是 `None`。
pub fn png_field(text: &[(String, String)], keyword: &str) -> Option<String> {
    text.iter()
        .find(|(listed, _)| listed == keyword)
        .map(|(_, value)| value.clone())
}

/// 一个成员相对某个容器根的名字，分隔符归一成 `/`。
///
/// 归一是因为**这个名字要被断言**：路径分隔符随平台而变，而黄金回归的快照要在哪台机器上
/// 都是同一份（15 号票）。`sink` 往归档里写成员名时用的也是这个算法，归档那一侧因此按它取得回来。
pub fn relative_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 一个卷的卷级判定，写成一行给人读的话。
///
/// 用词取自 `CONTEXT.md`：上包络定出的那一档叫**基准档**，站在分位秩上的那一页叫**驱动页**，
/// 因迟滞升上去的那些页叫**迟滞升档**。`Envelope` 自己的 `Display` 另有一份，那一份还带着
/// 「四者均未标定」那句注脚——快照要的是钉死的一行，注脚每趟都一样，摆进去只是噪声。
///
/// 黄金回归与真实素材冒烟共用这一份（15 号票）：两处说的是同一件事，各写一份迟早会走散。
pub fn volume_verdict(volume: &tonefit::VolumeReport) -> String {
    match volume.verdict {
        Some(tonefit::VolumeVerdict::Envelope(envelope)) => format!(
            "基准档 {} · 驱动页 {} · 主体 {} 页 · 离群 {} 页 · 迟滞升档 {} 页",
            envelope.base,
            page_at(volume, envelope.driver),
            envelope.body_pages,
            envelope.outlier_pages,
            envelope.raised_pages,
        ),
        Some(tonefit::VolumeVerdict::Override(candidate)) => format!("覆盖 {candidate}"),
        Some(tonefit::VolumeVerdict::PerPage) => "逐页".to_owned(),
        Some(tonefit::VolumeVerdict::Skipped { page_count }) => {
            format!("跳过 · 源 {page_count} 页")
        }
        None => "无 · 这一卷一页都没有".to_owned(),
    }
}

/// 卷内第 `index` 页在卷里的名字。驱动页与关掉几何门的那一页都靠它指人。
pub fn page_at(volume: &tonefit::VolumeReport, index: usize) -> String {
    volume
        .pages
        .get(index)
        .map(|page| relative_name(&volume.volume, &page.source))
        .unwrap_or_else(|| format!("第 {index} 页"))
}
