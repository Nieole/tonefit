//! 一次性量具（跑完就挪走）：AVIF 无损在这条依赖链上站不站得住。
//!
//! 两问：
//! 1. **像素逐字节还原吗**——ravif（`image` 的 `avif` 特性）编、dav1d（`avif-native`）解，
//!    解回来的像素与 PNG 写出的那一份比。
//! 2. **比 PNG 大多少**——同一批像素，PNG 字节数与 AVIF 无损字节数并排。
//!
//! 口径：页走真管线（`run` 那条缝），候选由 `--bit-depth`／`--dither` 顶死，
//! 因此量的是 tonefit 真会写出的那些像素，不是另造的一张图。
//!
//! 编码器与版本：`image` 0.25.10 → `ravif` 0.13.0 → `rav1e` 0.8.1（asm 关着），
//! `quality = 100`（ravif 把它换成 quantizer 0）、`speed = 10`、`ColorSpace::Srgb`
//! （`ColorModel::RGB` → matrix Identity，不做 RGB→YCbCr 那一趟有损换算）。

mod fixtures;

use std::io::Cursor;
use std::path::Path;

use image::codecs::avif::{AvifEncoder, ColorSpace};
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, ImageReader};
use tonefit::{BitDepth, Dither};

fn decode_png(path: &Path) -> DynamicImage {
    let bytes = std::fs::read(path).expect("读输出页");
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
        .decode()
        .expect("解输出 PNG")
}

fn avif_lossless(image: &DynamicImage) -> Vec<u8> {
    let luma = image.to_luma8();
    let (width, height) = (luma.width(), luma.height());
    let mut bytes = Vec::new();
    let encoder =
        AvifEncoder::new_with_speed_quality(&mut bytes, 10, 100).with_colorspace(ColorSpace::Srgb);
    encoder
        .write_image(luma.as_raw(), width, height, ExtendedColorType::L8)
        .expect("编 AVIF");
    bytes
}

fn decode_avif(bytes: &[u8]) -> DynamicImage {
    ImageReader::with_format(Cursor::new(bytes.to_vec()), ImageFormat::Avif)
        .decode()
        .expect("解 AVIF")
}

fn levels(image: &DynamicImage) -> usize {
    let mut seen = [false; 256];
    for level in image.to_luma8().as_raw() {
        seen[*level as usize] = true;
    }
    seen.iter().filter(|seen| **seen).count()
}

#[test]
fn avif_lossless_probe() {
    let candidates = [
        ("1bit", BitDepth::One, Dither::Off),
        ("1bit+FS", BitDepth::One, Dither::FloydSteinberg),
        ("2bit", BitDepth::Two, Dither::Off),
        ("2bit+FS", BitDepth::Two, Dither::FloydSteinberg),
        ("4bit", BitDepth::Four, Dither::Off),
        ("4bit+FS", BitDepth::Four, Dither::FloydSteinberg),
        ("8bit", BitDepth::Eight, Dither::Off),
    ];

    let pages = [
        ("screentone", fixtures::screentone(fixtures::TYPICAL)),
        ("line_art", fixtures::line_art(fixtures::TYPICAL)),
        ("gradient", fixtures::gradient(fixtures::TYPICAL)),
    ];

    println!(
        "{:<11} {:<9} {:>9} {:>11} {:>7} {:>7} {:>6}",
        "page", "cand", "png", "avif", "ratio", "levels", "exact"
    );
    for (name, page) in &pages {
        for (candidate, depth, dither) in candidates {
            let space = fixtures::Workspace::new();
            let volume = space.volume("volume-a");
            volume.page("001.png", page);

            let report = tonefit::run(&tonefit::Request {
                profile: fixtures::baseline_profile()
                    .with_gray_levels(256)
                    .expect("全集可用"),
                bit_depth: Some(depth),
                dither: Some(dither),
                ..fixtures::request(&space, [volume.path()])
            })
            .expect("处理应当成功");

            let written = &report.volumes[0].pages[0];
            let decoded = decode_png(&written.output);
            let png_bytes = std::fs::metadata(&written.output).expect("量 PNG").len();

            let avif = avif_lossless(&decoded);
            let back = decode_avif(&avif);
            let exact = back.to_luma8().as_raw() == decoded.to_luma8().as_raw();

            println!(
                "{:<11} {:<9} {:>9} {:>11} {:>6.2}x {:>7} {:>6}",
                name,
                candidate,
                png_bytes,
                avif.len(),
                avif.len() as f64 / png_bytes as f64,
                levels(&decoded),
                if exact { "yes" } else { "NO" }
            );
        }
    }
}
