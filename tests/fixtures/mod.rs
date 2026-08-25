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
use tonefit::{Profile, Size};

pub use cbz::{Cbz, read_cbz};

/// B 类素材的中位尺寸（见 measurements 的《B 类素材普查》）。缩放比含小数。
pub const TYPICAL: Size = Size::new(1441, 2048);

/// 正好两倍面板，缩放后的尺寸没有取整歧义。
pub const DOUBLE_PANEL: Size = Size::new(2528, 3360);

/// 宽幅跨页：宽高比远超面板，fit-inside 由宽边定夺。
pub const SPREAD: Size = Size::new(5056, 1680);

/// 两边都小于面板：不该被放大。
pub const SMALLER_THAN_TARGET: Size = Size::new(800, 1000);

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
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
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

    /// 写一个非图片文件。
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
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

/// 解出的 PNG，供测试直接看编码结果而不经被测代码。
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
    let file = fs::File::open(path).expect("打开输出 PNG");
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().expect("读 PNG 头");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
    let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
    pixels.truncate(info.buffer_size());
    DecodedPng {
        size: Size::new(info.width, info.height),
        color_type: info.color_type,
        bit_depth: info.bit_depth,
        pixels,
    }
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
