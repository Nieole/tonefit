//! 一次性探针（票 `encoder-gate/02`）：AVIF 在**本仓这条依赖链**上有没有页内元数据载体。
//!
//! 跑完即删——它不进仓库，产出是 `docs/research/` 里那一篇。
//! 口径不是「AVIF 规范支不支持」，是「手上这套依赖编得进去、读得回来吗」。

use std::io::Cursor;

use image::codecs::avif::{AvifDecoder, AvifEncoder};
use image::{ExtendedColorType, ImageDecoder, ImageEncoder};

const W: u32 = 16;
const H: u32 = 16;

fn pixels() -> Vec<u8> {
    let mut v = Vec::with_capacity((W * H * 3) as usize);
    for y in 0..H {
        for x in 0..W {
            let g = ((x + y) * 8) as u8;
            v.extend_from_slice(&[g, g, g]);
        }
    }
    v
}

/// 编一张 AVIF，带或不带一段元数据。返回 (文件字节, set_exif 的回话, set_icc 的回话)。
fn encode(exif: Option<&[u8]>) -> (Vec<u8>, String, String) {
    let mut out = Vec::new();
    let mut enc = AvifEncoder::new_with_speed_quality(&mut out, 10, 90);
    let exif_say = match exif {
        None => "(没调用)".to_owned(),
        Some(p) => match enc.set_exif_metadata(p.to_vec()) {
            Ok(()) => "Ok".to_owned(),
            Err(e) => format!("Err: {e}"),
        },
    };
    let icc_say = match enc.set_icc_profile(vec![0u8; 8]) {
        Ok(()) => "Ok".to_owned(),
        Err(e) => format!("Err: {e}"),
    };
    enc.write_image(&pixels(), W, H, ExtendedColorType::Rgb8)
        .expect("AVIF 编码");
    (out, exif_say, icc_say)
}

// ---------- 自己走一遍 ISOBMFF，不经任何依赖 ----------

/// 一层箱子：(四字码, 载荷起, 载荷止)。
fn walk(data: &[u8], mut pos: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    while pos + 8 <= end {
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let typ: [u8; 4] = data[pos + 4..pos + 8].try_into().unwrap();
        let (body, box_end) = match size {
            0 => (pos + 8, end),
            1 => {
                if pos + 16 > end {
                    break;
                }
                let big = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().unwrap()) as usize;
                (pos + 16, (pos + big).min(end))
            }
            n if n >= 8 => (pos + 8, (pos + n).min(end)),
            _ => break,
        };
        out.push((typ, body, box_end));
        if box_end <= pos {
            break;
        }
        pos = box_end;
    }
    out
}

fn find<'a>(boxes: &'a [([u8; 4], usize, usize)], tag: &[u8; 4]) -> Option<&'a ([u8; 4], usize, usize)> {
    boxes.iter().find(|b| &b.0 == tag)
}

/// `iinf` → item_id 到四字码的对应（avif-serialize 写的是 iinf v0 + infe v2）。
fn item_types(data: &[u8], body: usize, end: usize) -> Vec<(u16, [u8; 4])> {
    let mut out = Vec::new();
    if body >= end {
        return out;
    }
    // `iinf` 是 FullBox：version 0 的 entry_count 是 u16，version 1 起是 u32。
    // avif-serialize 0.8.9 写的是 v0（`boxes.rs` 的 `impl MpegBox for IinfBox`）。
    // **硬断言它**：上游哪天改成 v1，这里按 u16 读会读出高半截 0，
    // 于是一条 infe 都取不到、`stored_exif` 答 `None`，探针会印「没找到」——
    // 那正是本轮在 `docs/research/` 里点名的「失败没有声音」。宁可当场炸。
    assert_eq!(data[body], 0, "iinf 的 FullBox version 不是 0，本探针的解析对不上了");
    let p = body + 4; // FullBox 的 version/flags
    if p + 2 > end {
        return out;
    }
    let count = u16::from_be_bytes(data[p..p + 2].try_into().unwrap()) as usize;
    let mut entries = walk(data, p + 2, end);
    entries.retain(|b| &b.0 == b"infe");
    for (_, ib, ie) in entries.into_iter().take(count) {
        // FullBox(version=2) 之后：item_ID u16、protection u16、item_type 四字码
        if ib + 12 > ie {
            continue;
        }
        let version = data[ib];
        if version != 2 {
            continue;
        }
        let id = u16::from_be_bytes(data[ib + 4..ib + 6].try_into().unwrap());
        let typ: [u8; 4] = data[ib + 8..ib + 12].try_into().unwrap();
        out.push((id, typ));
    }
    out
}

/// `iloc` → item_id 到它那几段 (绝对偏移, 长度)。只认 avif-serialize 写的那一种：
/// version 0、offset_size = length_size = 4、base_offset_size = 0。
fn item_extents(data: &[u8], body: usize, end: usize) -> Vec<(u16, Vec<(usize, usize)>)> {
    let mut out = Vec::new();
    let mut p = body + 4;
    if p + 4 > end {
        return out;
    }
    let sizes = data[p];
    let base = data[p + 1] >> 4;
    assert_eq!(sizes, 0x44, "iloc 的 offset/length 宽度不是 4/4");
    assert_eq!(base, 0, "iloc 的 base_offset_size 不是 0");
    let count = u16::from_be_bytes(data[p + 2..p + 4].try_into().unwrap()) as usize;
    p += 4;
    for _ in 0..count {
        if p + 6 > end {
            break;
        }
        let id = u16::from_be_bytes(data[p..p + 2].try_into().unwrap());
        let n = u16::from_be_bytes(data[p + 4..p + 6].try_into().unwrap()) as usize;
        p += 6;
        let mut ext = Vec::new();
        for _ in 0..n {
            if p + 8 > end {
                break;
            }
            let off = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(data[p + 4..p + 8].try_into().unwrap()) as usize;
            p += 8;
            ext.push((off, len));
        }
        out.push((id, ext));
    }
    out
}

/// 把文件里 `Exif` 那一项存着的字节原样取出来。
fn stored_exif(data: &[u8]) -> Option<Vec<u8>> {
    let top = walk(data, 0, data.len());
    let &(_, mb, me) = find(&top, b"meta")?;
    // `meta` 也是 FullBox，跳它的 version/flags 4 字节。同上：断言 version，
    // 不然跳错 4 字节之后走出来的是一堆垃圾箱子，而且不会报错。
    assert_eq!(data[mb], 0, "meta 的 FullBox version 不是 0，本探针的解析对不上了");
    let meta_children = walk(data, mb + 4, me);
    let &(_, ib, ie) = find(&meta_children, b"iinf")?;
    let &(_, lb, le) = find(&meta_children, b"iloc")?;
    let types = item_types(data, ib, ie);
    let exif_id = types.iter().find(|(_, t)| t == b"Exif").map(|(id, _)| *id)?;
    let extents = item_extents(data, lb, le);
    let (_, ext) = extents.into_iter().find(|(id, _)| *id == exif_id)?;
    let mut bytes = Vec::new();
    for (off, len) in ext {
        bytes.extend_from_slice(data.get(off..off + len)?);
    }
    Some(bytes)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn decode(data: &[u8]) -> (String, String, String, String, String) {
    match AvifDecoder::new(Cursor::new(data.to_vec())) {
        Err(e) => {
            let s = format!("解码失败: {e}");
            (s.clone(), s.clone(), s.clone(), s.clone(), s)
        }
        Ok(mut d) => {
            let dims = format!("{:?}", d.dimensions());
            let say = |r: image::ImageResult<Option<Vec<u8>>>| match r {
                Ok(None) => "Ok(None)".to_owned(),
                Ok(Some(v)) => format!("Ok(Some({} 字节))", v.len()),
                Err(e) => format!("Err: {e}"),
            };
            let exif = say(d.exif_metadata());
            let xmp = say(d.xmp_metadata());
            let iptc = say(d.iptc_metadata());
            let icc = say(d.icc_profile());
            (dims, exif, xmp, iptc, icc)
        }
    }
}

fn pixels_back(data: &[u8]) -> Vec<u8> {
    let d = AvifDecoder::new(Cursor::new(data.to_vec())).expect("解 AVIF");
    let mut buf = vec![0u8; d.total_bytes() as usize];
    d.read_image(&mut buf).expect("读像素");
    buf
}

/// 对照组用：往 PNG 里塞一段 tEXt 并**真写出去**，答 (add_text_chunk 说什么, 写出时说什么)。
fn png_with_text(text: &str) -> (String, String) {
    let mut buf = Vec::new();
    let added;
    let result;
    {
        let mut e = png::Encoder::new(Cursor::new(&mut buf), 1, 1);
        e.set_depth(png::BitDepth::Eight);
        e.set_color(png::ColorType::Grayscale);
        added = match e.add_text_chunk("tonefit:probe".to_owned(), text.to_owned()) {
            Ok(()) => "Ok".to_owned(),
            Err(err) => format!("Err: {err}"),
        };
        result = (|| -> Result<(), png::EncodingError> {
            let mut w = e.write_header()?;
            w.write_image_data(&[0u8])?;
            w.finish()
        })();
    }
    let written = match result {
        Ok(()) => format!("Ok，写出 {} 字节", buf.len()),
        Err(err) => format!("Err: {err}"),
    };
    (added, written)
}

fn report(name: &str, payload: &[u8]) {
    println!("\n---------- {name}（{} 字节载荷） ----------", payload.len());
    let (file, exif_say, icc_say) = encode(Some(payload));
    println!("  set_exif_metadata   → {exif_say}");
    println!("  set_icc_profile     → {icc_say}");
    println!("  文件               → {} 字节", file.len());
    println!("  文件里有 `Exif` 四字码 → {}", contains(&file, b"Exif"));
    println!(
        "  载荷字节整段在文件里   → {}",
        if payload.is_empty() {
            "n/a（空载荷，这一问问不出来）".to_owned()
        } else {
            contains(&file, payload).to_string()
        }
    );
    match stored_exif(&file) {
        None => println!("  自己走 ISOBMFF 取 Exif 项 → 没找到"),
        Some(stored) => {
            println!("  自己走 ISOBMFF 取 Exif 项 → {} 字节", stored.len());
            let framed = {
                let mut v = vec![0u8, 0, 0, 0];
                v.extend_from_slice(payload);
                v
            };
            println!("  == [4 字节零偏移] ++ 载荷？ → {}", stored == framed);
            println!("  == 载荷本身？             → {}", stored == payload);
            let tail_same = stored.len() >= payload.len()
                && &stored[stored.len() - payload.len()..] == payload;
            println!("  末尾 {} 字节逐字节相同？  → {tail_same}", payload.len());
        }
    }
    let (dims, exif, xmp, iptc, icc) = decode(&file);
    println!("  AvifDecoder::dimensions → {dims}");
    println!("  AvifDecoder::exif_metadata → {exif}");
    println!("  AvifDecoder::xmp_metadata  → {xmp}");
    println!("  AvifDecoder::iptc_metadata → {iptc}");
    println!("  AvifDecoder::icc_profile   → {icc}");
}

#[test]
fn avif_in_page_metadata_probe() {
    println!("\n================ 探针：AVIF 页内元数据载体 ================");
    println!("版本（取自 Cargo.lock）：image 0.25.10、ravif 0.13.0、avif-serialize 0.8.9、");
    println!("                          rav1e 0.8.1、mp4parse 0.17.0、dav1d(crate) 0.11.1");

    // ① 一段平常的 ASCII 记录，形状照 PNG tEXt 那一份
    let ascii = b"tonefit 0.1.0|profile=kobo-libra-2|params=0123456789abcdef\
0123456789abcdef|verdict=2bit+fs";
    report("① ASCII 记录", ascii);

    // ② 非 Latin-1 的字节：UTF-8 中文 + 内嵌 NUL + 0x80..0xFF
    let mut wild: Vec<u8> = "记录：改革之獸／第 001 卷".as_bytes().to_vec();
    wild.extend_from_slice(&[0x00, 0x01, 0x7f, 0x80, 0xfe, 0xff]);
    wild.extend_from_slice("尾巴".as_bytes());
    report("② 非 Latin-1 字节（UTF-8 + NUL + 0x80~0xFF）", &wild);

    // ③ 空载荷
    report("③ 空载荷", b"");

    // ④ 64 KiB
    let big: Vec<u8> = (0..65536u32).map(|i| (i % 251) as u8).collect();
    report("④ 64 KiB 载荷", &big);

    // ⑤ 关得掉吗：同样的像素、不调 set_exif_metadata
    println!("\n---------- ⑤ 不调 set_exif_metadata（`--no-metadata` 那条路） ----------");
    let (bare, exif_say, icc_say) = encode(None);
    println!("  set_exif_metadata   → {exif_say}");
    println!("  set_icc_profile     → {icc_say}");
    println!("  文件               → {} 字节", bare.len());
    println!("  文件里有 `Exif` 四字码 → {}", contains(&bare, b"Exif"));
    println!("  自己走 ISOBMFF 取 Exif 项 → {:?}", stored_exif(&bare).map(|v| v.len()));
    let (dims, exif, xmp, iptc, icc) = decode(&bare);
    println!("  AvifDecoder::dimensions → {dims}");
    println!("  AvifDecoder::exif_metadata → {exif}");
    println!("  AvifDecoder::xmp_metadata  → {xmp}");
    println!("  AvifDecoder::iptc_metadata → {iptc}");
    println!("  AvifDecoder::icc_profile   → {icc}");

    let (with, _, _) = encode(Some(ascii));
    println!("\n  带记录与不带记录，解出来的像素逐字节相同？ → {}",
        pixels_back(&with) == pixels_back(&bare));
    println!("  两份文件的字节数        → 带 {} / 不带 {}", with.len(), bare.len());

    // ⑥ 对照组：PNG tEXt 在同一段字节上说什么
    //
    // 注意：`add_text_chunk` **恒返回 Ok**（png 0.18.1 `src/encoder.rs:416`，
    // 它只是 push 进 Info，一个字都不校验）。Latin-1 的拒绝出自**写出那一刻**
    // ——`TEXtChunk::encode` 里的 `encode_iso_8859_1`（`src/text_metadata.rs:161`）。
    // 所以对照组必须真把文件写出来，只调 add_text_chunk 什么都测不到。
    println!("\n---------- ⑥ 对照组：PNG tEXt（png 0.18.1） ----------");
    let wild_text = String::from_utf8_lossy(&wild).into_owned();
    let ascii_text = String::from_utf8_lossy(ascii).into_owned();
    let latin1_only = "caf\u{e9} \u{ff} \u{a0}".to_owned(); // 全在 U+00FF 以内
    let cjk = "改革之獸".to_owned();
    for (name, text) in [
        ("ASCII 记录", &ascii_text),
        ("非 Latin-1 那一段（UTF-8 中文+NUL+高位）", &wild_text),
        ("纯 Latin-1（U+00FF 以内）", &latin1_only),
        ("中文", &cjk),
    ] {
        let (added, written) = png_with_text(text);
        println!("  {name}");
        println!("      add_text_chunk → {added}");
        println!("      真写出去       → {written}");
    }

    println!("\n================ 探针结束 ================\n");
}
