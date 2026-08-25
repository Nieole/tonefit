//! 编码：把一张已按目标位深量化的图编成 PNG 字节。
//!
//! 编出字节而不是写文件——页要么落到目录里，要么进归档，去处由输出容器决定（见 `sink`）。
//!
//! 位深与抖动模式在编码器接口以外，**调色板模式在接口以内**（ADR 0004）：判定位深说的是
//! 量化格点，颜色类型、以及最终写进文件的那个位深由这里挑，挑的依据只有体积。
//! 两者的差别在位宽由谁定：灰度按**格点数**留位宽，调色板按**页面实际用到的取值数**留。
//! 判定 4bit 而一页只有两个取值时，后者装进 1 位而像素一个不动，前者留满 4 位。
//!
//! ADR 0004 的编码器接口本身还没抽出来——P0 只有 PNG 一个实现，AVIF 输出不在范围内。

use anyhow::{Context, Result};

use crate::geometry::Size;
use crate::gray::GrayImage;
use crate::quantize::{BitDepth, grid_index};

/// 把一张按 `depth` 量化过的灰度图编成 PNG，灰度与调色板两种颜色类型取体积小者。
///
/// 两种编法写出的像素完全相同，差别只在颜色类型与位宽，因此「取小者」不牵动画质。
pub fn png(image: &GrayImage, depth: BitDepth) -> Result<Vec<u8>> {
    let grayscale = grayscale_png(image, depth)?;
    let palette = palette_png(image)?;
    // 一样大时留灰度：它不带 PLTE，读的那一端也少一层间接。
    Ok(if palette.len() < grayscale.len() {
        palette
    } else {
        grayscale
    })
}

/// 灰度 PNG：取值直接落在 `depth` 的格点序号上，位宽就是 `depth`。
fn grayscale_png(image: &GrayImage, depth: BitDepth) -> Result<Vec<u8>> {
    let indices: Vec<u8> = image
        .pixels()
        .iter()
        .map(|&level| grid_index(level, depth))
        .collect();
    write_png(image.size(), &indices, depth, None)
}

/// 调色板 PNG：色板只收这一页真正用到的取值，位宽因此可能低于判定位深。
///
/// 色板按灰度取值升序排，索引跟着单调——PNG 的行滤波器在索引上做差分，
/// 乱序的色板会把平缓过渡打成噪声。
fn palette_png(image: &GrayImage) -> Result<Vec<u8>> {
    let mut used = [false; 256];
    for &level in image.pixels() {
        used[level as usize] = true;
    }
    let levels: Vec<u8> = (0..=255u8).filter(|&level| used[level as usize]).collect();

    let mut index_of = [0u8; 256];
    for (index, &level) in levels.iter().enumerate() {
        index_of[level as usize] = index as u8;
    }
    let indices: Vec<u8> = image
        .pixels()
        .iter()
        .map(|&level| index_of[level as usize])
        .collect();

    // 装得下这块色板的最小位宽。全图同一个取值时色板只有一项，PNG 没有 0 位，落到 1 位。
    let depth = BitDepth::ALL
        .into_iter()
        .find(|depth| depth.levels() as usize >= levels.len())
        .expect("256 个取值装得进 8bit");
    let palette: Vec<u8> = levels
        .iter()
        .flat_map(|&level| [level, level, level])
        .collect();
    write_png(image.size(), &indices, depth, Some(&palette))
}

/// 把每像素一个字节的索引写成 PNG。`palette` 给了就是调色板颜色类型，否则是灰度。
fn write_png(
    size: Size,
    indices: &[u8],
    depth: BitDepth,
    palette: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size.width, size.height);
    encoder.set_depth(png_depth(depth));
    match palette {
        Some(palette) => {
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_palette(palette.to_vec());
        }
        None => encoder.set_color(png::ColorType::Grayscale),
    }
    let mut writer = encoder.write_header().context("写 PNG 头")?;
    writer
        .write_image_data(&pack(indices, size, depth.bits()))
        .context("写 PNG 像素")?;
    writer.finish().context("收尾 PNG")?;
    Ok(bytes)
}

/// 把索引按每像素 `bits` 位打包成 PNG 的扫描行：高位在前，每行另起一个字节。
fn pack(indices: &[u8], size: Size, bits: u32) -> Vec<u8> {
    if bits == 8 {
        return indices.to_vec();
    }
    let width = size.width as usize;
    let per_byte = 8 / bits as usize;
    let row_bytes = width.div_ceil(per_byte);
    let mut packed = vec![0u8; row_bytes * size.height as usize];
    for (y, row) in indices.chunks_exact(width).enumerate() {
        for (x, &index) in row.iter().enumerate() {
            let shift = 8 - bits as usize * (x % per_byte + 1);
            packed[y * row_bytes + x / per_byte] |= index << shift;
        }
    }
    packed
}

fn png_depth(depth: BitDepth) -> png::BitDepth {
    match depth {
        BitDepth::One => png::BitDepth::One,
        BitDepth::Two => png::BitDepth::Two,
        BitDepth::Four => png::BitDepth::Four,
        BitDepth::Eight => png::BitDepth::Eight,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::{Candidate, Dither, quantize};

    /// 一页竖直渐变，每行一个取值。
    fn gradient(size: Size) -> GrayImage {
        let last = (size.height - 1).max(1);
        let pixels = (0..size.height)
            .flat_map(|y| std::iter::repeat_n((y * 255 / last) as u8, size.width as usize))
            .collect();
        GrayImage::new(size, pixels)
    }

    /// 解回来看：颜色类型、位宽，以及摊回 8 位的像素。
    fn read(bytes: &[u8]) -> (png::ColorType, png::BitDepth, Vec<u8>) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let header = decoder.read_header_info().expect("读 PNG 头").clone();
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().expect("读 PNG 信息");
        let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
        let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
        pixels.truncate(info.buffer_size());
        // EXPAND 把调色板摊成 RGB8；色板项恒是三个相等的分量，取一个即可。
        if header.color_type == png::ColorType::Indexed {
            pixels = pixels
                .as_chunks::<3>()
                .0
                .iter()
                .map(|pixel| pixel[0])
                .collect();
        }
        (header.color_type, header.bit_depth, pixels)
    }

    /// 每一档位深都要能编、能解回原样：写出去的像素与量化结果逐字节相同。
    #[test]
    fn every_bit_depth_round_trips_the_quantized_pixels() {
        // 宽度取 7：1/2/4 位每行都除不尽，末字节留着空位，正好压住打包的边界。
        let size = Size::new(7, 5);
        for depth in BitDepth::ALL {
            let quantized = quantize(&gradient(size), Candidate::new(depth, Dither::Off));
            let (_, _, read_back) = read(&png(&quantized, depth).expect("编 PNG"));
            assert_eq!(read_back, quantized.pixels(), "{depth} 没有原样解回来");
        }
    }

    /// 取值铺满格点时灰度胜出：调色板要为同样的位宽再背一个 PLTE。
    #[test]
    fn a_page_that_fills_the_grid_comes_out_as_grayscale() {
        let quantized = quantize(
            &gradient(Size::new(64, 64)),
            Candidate::new(BitDepth::Four, Dither::Off),
        );
        let (color_type, bit_depth, _) = read(&png(&quantized, BitDepth::Four).expect("编 PNG"));
        assert_eq!(color_type, png::ColorType::Grayscale);
        assert_eq!(bit_depth, png::BitDepth::Four);
    }

    /// 取值远少于格点时调色板胜出，且把位宽压到装得下色板的那一档——
    /// 判定位深是 4bit，文件里却只用 1 位，而像素一个没动。
    #[test]
    fn a_page_with_two_levels_comes_out_as_a_palette_at_a_lower_width() {
        let size = Size::new(64, 64);
        let pixels = (0..size.height)
            .flat_map(|y| {
                std::iter::repeat_n(if y % 2 == 0 { 0 } else { 255 }, size.width as usize)
            })
            .collect();
        let image = GrayImage::new(size, pixels);

        let bytes = png(&image, BitDepth::Four).expect("编 PNG");

        let (color_type, bit_depth, read_back) = read(&bytes);
        assert_eq!(color_type, png::ColorType::Indexed);
        assert_eq!(bit_depth, png::BitDepth::One);
        assert_eq!(read_back, image.pixels());
        assert!(
            bytes.len()
                < grayscale_png(&image, BitDepth::Four)
                    .expect("编灰度 PNG")
                    .len(),
            "调色板没有比灰度小"
        );
    }

    /// 全图一个取值：色板只有一项，而 PNG 没有 0 位，位宽落到 1 位。
    ///
    /// 直接问调色板那一路——纯色页压到几十字节，两路谁小取决于 PLTE 那点开销，
    /// 而这里要钉的是「一项色板编得出来」，不是它赢不赢。
    #[test]
    fn a_single_entry_palette_still_takes_one_bit() {
        let size = Size::new(16, 16);
        let image = GrayImage::new(size, vec![204; 256]);

        let (color_type, bit_depth, read_back) = read(&palette_png(&image).expect("编调色板 PNG"));

        assert_eq!(color_type, png::ColorType::Indexed);
        assert_eq!(bit_depth, png::BitDepth::One);
        assert!(read_back.iter().all(|&level| level == 204));
    }
}
