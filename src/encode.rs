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

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};

use crate::color::ColorImage;
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

/// 把一张彩色图编成 PNG，真彩色与调色板两种颜色类型取体积小者。
///
/// 彩色分支不量化（ADR 0005 决定第 4 条）：这里写出的是缩放后的像素本身，每分量 8 位。
/// 「取体积较小者」与灰度那一侧同一条规则，挑的依据同样只有体积——两种编法写出的像素完全相同。
pub fn color_png(image: &ColorImage) -> Result<Vec<u8>> {
    // 两种编法读的是同一份交织缓冲：一页 RGB 好几 MB，交织两遍是白付一次分配。
    let interleaved = image.interleaved();
    let truecolor = truecolor_png(image.size(), &interleaved)?;
    match palette_color_png(image.size(), &interleaved)? {
        Some(palette) if palette.len() < truecolor.len() => Ok(palette),
        _ => Ok(truecolor),
    }
}

/// 真彩色 PNG：每像素三字节，原样写出。
fn truecolor_png(size: Size, interleaved: &[u8]) -> Result<Vec<u8>> {
    write(
        size,
        png::BitDepth::Eight,
        png::ColorType::Rgb,
        None,
        interleaved,
    )
}

/// 调色板 PNG：这一页用到的颜色不超过 256 种时才编得出来，超过就是 `None`。
///
/// 色板按 RGB 三元组的字典序排。灰度那一侧排序买的是「索引跟着取值单调，行滤波器的差分才小」，
/// 彩色这一侧买不到同样的东西——三个分量上没有一个共同的序。排序在这里只为**定死次序**：
/// 同一页跑两遍要编出同一个文件（11 号票的幂等靠的是这一条）。
fn palette_color_png(size: Size, interleaved: &[u8]) -> Result<Option<Vec<u8>>> {
    let Some(colors) = distinct_colors(interleaved) else {
        return Ok(None);
    };
    let index_of: HashMap<[u8; 3], u8> = colors
        .iter()
        .enumerate()
        .map(|(index, &color)| (color, index as u8))
        .collect();
    let indices: Vec<u8> = interleaved
        .as_chunks::<3>()
        .0
        .iter()
        .map(|pixel| index_of[pixel])
        .collect();
    let palette: Vec<u8> = colors.concat();
    write_png(
        size,
        &indices,
        narrowest_depth(colors.len()),
        Some(&palette),
    )
    .map(Some)
}

/// 这一页用到的颜色，升序。超过 256 种就是 `None`——那时调色板编不出来。
///
/// 数到第 257 种就停：照片一类的页在头几行就越过它，把整页扫完是白扫。
fn distinct_colors(interleaved: &[u8]) -> Option<Vec<[u8; 3]>> {
    let mut seen: HashSet<[u8; 3]> = HashSet::new();
    for &pixel in interleaved.as_chunks::<3>().0 {
        if seen.insert(pixel) && seen.len() > 256 {
            return None;
        }
    }
    let mut colors: Vec<[u8; 3]> = seen.into_iter().collect();
    colors.sort_unstable();
    Some(colors)
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
    let palette: Vec<u8> = levels
        .iter()
        .flat_map(|&level| [level, level, level])
        .collect();
    write_png(
        image.size(),
        &indices,
        narrowest_depth(levels.len()),
        Some(&palette),
    )
}

/// 装得下这么多项色板的最小位宽。
///
/// 灰度与彩色两条调色板路径共用它：位宽由**色板项数**定，与判定位深无关。
/// PNG 没有 0 位，只有一项的色板（全图一色）落到 1 位。
fn narrowest_depth(entries: usize) -> BitDepth {
    BitDepth::ALL
        .into_iter()
        .find(|depth| depth.levels() as usize >= entries)
        .expect("256 项装得进 8bit")
}

/// 把每像素一个字节的索引写成 PNG。`palette` 给了就是调色板颜色类型，否则是灰度。
fn write_png(
    size: Size,
    indices: &[u8],
    depth: BitDepth,
    palette: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let color = match palette {
        Some(_) => png::ColorType::Indexed,
        None => png::ColorType::Grayscale,
    };
    write(
        size,
        png_depth(depth),
        color,
        palette,
        &pack(indices, size, depth.bits()),
    )
}

/// 把一整幅扫描行写成 PNG 字节。
///
/// 三种编法——灰度、调色板、真彩色——只在位宽、颜色类型与带不带色板这三项上分岔，
/// 编码器这一段的样板与出错说法因此只此一份。
fn write(
    size: Size,
    depth: png::BitDepth,
    color: png::ColorType,
    palette: Option<&[u8]>,
    scanlines: &[u8],
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size.width, size.height);
    encoder.set_depth(depth);
    encoder.set_color(color);
    if let Some(palette) = palette {
        encoder.set_palette(palette.to_vec());
    }
    let mut writer = encoder.write_header().context("写 PNG 头")?;
    writer.write_image_data(scanlines).context("写 PNG 像素")?;
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

    /// 解回一张彩色 PNG：颜色类型、位宽，以及摊成 RGB8 的像素。
    fn read_color(bytes: &[u8]) -> (png::ColorType, png::BitDepth, Vec<u8>) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let header = decoder.read_header_info().expect("读 PNG 头").clone();
        // EXPAND 把调色板摊成 RGB8，两种颜色类型于是解成同一种形态。
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().expect("读 PNG 信息");
        let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
        let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
        pixels.truncate(info.buffer_size());
        (header.color_type, header.bit_depth, pixels)
    }

    /// 一张彩色图，`pixel` 给出每个像素的三个分量。
    fn color_image(size: Size, pixel: impl Fn(u32, u32) -> [u8; 3]) -> ColorImage {
        let mut planes = [Vec::new(), Vec::new(), Vec::new()];
        for y in 0..size.height {
            for x in 0..size.width {
                for (plane, channel) in planes.iter_mut().zip(pixel(x, y)) {
                    plane.push(channel);
                }
            }
        }
        ColorImage::new(size, planes.map(|pixels| GrayImage::new(size, pixels)))
    }

    /// 彩色分支不量化：写出去的像素与缩放结果逐字节相同，两种颜色类型都是。
    #[test]
    fn a_color_page_round_trips_whichever_color_type_wins() {
        // 宽度取 7：调色板那一路每行都除不尽，末字节留着空位，压住打包的边界。
        let size = Size::new(7, 5);
        let flat = color_image(size, |x, y| [(x * 17) as u8, (y * 51) as u8, 0]);
        let varied = color_image(size, |x, y| {
            [(x * 37 + y) as u8, (y * 13) as u8, (x + y) as u8]
        });

        for image in [&flat, &varied] {
            let (_, _, read_back) = read_color(&color_png(image).expect("编彩色 PNG"));
            assert_eq!(read_back, image.interleaved(), "彩色像素没有原样解回来");
        }
    }

    /// 颜色少于 256 种时调色板胜出，位宽压到装得下色板的那一档；多于 256 种只能走真彩色。
    ///
    /// 与灰度那一侧同一条规则：两种编法写出的像素完全相同，挑的依据只有体积。
    #[test]
    fn a_page_with_few_colors_comes_out_as_a_palette_and_a_photograph_does_not() {
        let size = Size::new(64, 64);
        // 六种颜色：色板装得下，4 位就够（PNG 没有 3 位）。
        let banded = color_image(size, |_, y| {
            [
                [0, 0, 255],
                [18, 18, 18],
                [255, 0, 0],
                [0, 255, 0],
                [255, 255, 255],
                [0, 0, 0],
            ][(y % 6) as usize]
        });
        let (color_type, bit_depth, _) = read_color(&color_png(&banded).expect("编彩色 PNG"));
        assert_eq!(color_type, png::ColorType::Indexed);
        assert_eq!(bit_depth, png::BitDepth::Four);
        assert!(
            color_png(&banded).expect("编彩色 PNG").len()
                < truecolor_png(banded.size(), &banded.interleaved())
                    .expect("编真彩色 PNG")
                    .len(),
            "调色板没有比真彩色小"
        );

        // 每个像素一种颜色：4096 种，色板装不下。
        let photograph = color_image(size, |x, y| [x as u8, y as u8, (x * y) as u8]);
        assert!(
            palette_color_png(photograph.size(), &photograph.interleaved())
                .expect("编调色板 PNG")
                .is_none()
        );
        let (color_type, bit_depth, _) = read_color(&color_png(&photograph).expect("编彩色 PNG"));
        assert_eq!(color_type, png::ColorType::Rgb);
        assert_eq!(bit_depth, png::BitDepth::Eight);
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
