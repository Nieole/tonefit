//! 样张（`CONTEXT.md` 的《样张》），在库的第四个 seam——[`tonefit::write_proof`]——上测。
//!
//! 只断言**外部看得见的事实**：交出来的数据、落到盘上的文件有什么性质、源文件有没有被动过。
//! 不断内部怎么走——与 `tests/pipeline.rs` 的模块文档同一句话（spec《Testing Decisions》）。

mod fixtures;

use std::fs;
use std::path::PathBuf;

use fixtures::Workspace;
use tonefit::{Candidate, Proof, ProofPage, Request};

/// 一张**普通页**：单页、不拆、灰度路径、门成立——本票的对象。
///
/// **每一步都得真在做事**，神谕那一条比的才不是一张「什么都没发生」的页：
///
/// - **裁白边**：内容四周一圈纯白边（[`MARGIN`]），裁完恰好剩下内容那一块；
/// - **缩放**：内容高 [`CONTENT`] 要缩到面板高，而页上有**硬边**（一竖条墨）——
///   换一个缩放算法，硬边两侧的振铃就不一样。只有一张线性渐变的页做不到这一条：
///   对称的核把线性斜坡原样复现，换算法一个字节都不变（本票落地时按反跑过一次，亲眼看见它照绿）；
/// - **纸色提白**：纸白是离格 2 级的 [`fixtures::OFF_GRID_PAPER_WHITE`]，[`Plain`] 点名的上限钳得动它；
/// - **画质分、量化、编码**：那一竖条从纯黑爬到纸白之下的灰调，六档各有各的样子。
///
/// 内容本身是 [`fixtures::page_with_paper_white`]（四边顶着墨，裁白边正好停在它的边上）。
fn plain_page() -> image::DynamicImage {
    let content =
        fixtures::page_with_paper_white(CONTENT, fixtures::OFF_GRID_PAPER_WHITE).to_luma8();
    let mut page = image::GrayImage::from_pixel(
        CONTENT.width + 2 * MARGIN,
        CONTENT.height + 2 * MARGIN,
        image::Luma([255]),
    );
    image::imageops::replace(&mut page, &content, i64::from(MARGIN), i64::from(MARGIN));
    image::DynamicImage::ImageLuma8(page)
}

/// 普通页里内容那一块：比面板高（1680）高一截，缩放因此真的在缩；宽高比远够不上跨页候选。
const CONTENT: tonefit::Size = tonefit::Size::new(1200, 1800);

/// 内容四周那一圈纯白边有多宽。
const MARGIN: u32 = 96;

/// 一张普通页摆在一个卷里（`卷/001.png`），外加一份**不写记录**的请求。
///
/// 卷级那几格（点名的卷、输出根）样张一格都不读（见 [`tonefit::write_proof`]），
/// 这里照 `run` 要的填：同一份交给 `run`，写出去的就是神谕那一条要比的那一张。
///
/// **处理选项那几格点名取值**，不借默认值（`docs/agents/testing.md`）：夹具让每一步都真在做事，
/// 靠的是这几个数——默认值哪天挪了（比如提白上限降到 2 以下），神谕那一条就会在一张
/// 什么都没提白的页上照绿。取值与眼下的默认值相同，是因为本票只走默认那一套。
struct Plain {
    space: Workspace,
    source: PathBuf,
    request: Request,
}

impl Plain {
    fn new() -> Self {
        let space = Workspace::new();
        let volume = space.volume("卷");
        let source = volume.page("001.png", &plain_page());
        let request = Request {
            fit: tonefit::FitMode::Height,
            crop: true,
            split: tonefit::SplitRule {
                on: true,
                ..tonefit::SplitRule::default()
            },
            filter: tonefit::Filter::Lanczos3,
            white_align_limit: fixtures::ALIGNING_LIMIT,
            bit_depth: None,
            dither: None,
            metadata: false,
            ..fixtures::request(&space, [volume.path()])
        };
        Self {
            space,
            source,
            request,
        }
    }

    /// 样张的去处。**此刻还不在**：出样张的那一趟自己建出来。
    fn sheets(&self) -> PathBuf {
        self.space.dir("样张")
    }

    fn proof(&self) -> Proof {
        tonefit::write_proof(&self.source, &self.request, &self.sheets()).expect("出样张")
    }
}

/// 这一张图的那一叠。普通页不拆，一张图就是一叠。
fn only_page(proof: &Proof) -> &ProofPage {
    match proof.pages.as_slice() {
        [only] => only,
        pages => panic!("一张普通页该出一叠，出了 {} 叠", pages.len()),
    }
}

/// **一张普通页出一叠**：去处不在就建出来，里面一个候选一张、外加《参照》一张；
/// 交出来的候选集恰好是这块面板上这一页的门派得出的那一整套；
/// 每个文件名说得出它是哪一页、哪一个候选。
#[test]
fn a_plain_page_gets_one_sheet_for_every_candidate_and_one_reference() {
    let plain = Plain::new();
    assert!(!plain.sheets().exists(), "夹具的前提：去处此刻还不在");

    let proof = plain.proof();
    let page = only_page(&proof);

    let gate = page.page.gate().expect("普通页走灰度路径，有门");
    assert!(gate.holds(), "普通页的门成立：高缩到面板高，贴住了一条边");
    let expected = Candidate::all(plain.request.profile.panel().gray_levels, gate);
    assert_eq!(
        expected.len(),
        6,
        "夹具的前提：e-ink 面板、门成立，六个候选"
    );
    let proofed: Vec<Candidate> = page.scored().map(|(scored, _)| scored.candidate).collect();
    assert_eq!(
        proofed, expected,
        "交出来的候选集就是这一页的门派得出的那一整套"
    );

    let on_disk = fixtures::directory_members(&plain.sheets());
    assert_eq!(
        on_disk.len(),
        expected.len() + 1,
        "去处里一个候选一张、外加参照一张：{on_disk:?}"
    );
    let mut named: Vec<String> = page
        .candidates
        .iter()
        .chain([&page.reference])
        .map(|sheet| {
            assert_eq!(
                fs::metadata(&sheet.file)
                    .expect("交出来的那一张在盘上")
                    .len(),
                sheet.bytes,
                "交出来的字节数就是盘上那一张的大小"
            );
            sheet
                .file
                .strip_prefix(plain.sheets())
                .expect("每一张都落在点名的去处里")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    named.sort();
    assert_eq!(named, on_disk, "交出来的那几张就是盘上那几张");
    // 名字的写法：这一页在 `run` 那一侧的成员名（`001`），接上是哪一个候选。
    for (scored, sheet) in page.scored() {
        assert_eq!(
            sheet.file.file_name().expect("有文件名").to_string_lossy(),
            format!("001.{}.png", scored.candidate),
            "那一张的名字说不出它是哪一档"
        );
    }
    assert!(
        on_disk.contains(&"001.参照.png".to_owned()),
        "参照那一张的名字说不出它是参照：{on_disk:?}"
    );
}

/// **神谕**：同一张图、同一套选项，样张里判定那一档的那一张与 `run`（`--no-metadata`）
/// 写出的那一张**逐字节相同**（spec《Testing Decisions》第一条）。
///
/// 它钉的是「样张不许从管线上漂开」：样张要是另走一条平行的路，哪怕只漏掉一步——
/// 裁白边、纸色提白、换一个缩放算法——这里当场就红。判定也一并比：
/// 两边定下的是同一档、同一个理由。
#[test]
fn the_verdict_sheet_is_byte_for_byte_what_run_writes_without_metadata() {
    let plain = Plain::new();

    let report = tonefit::run(&plain.request).expect("转换那一趟");
    let proof = plain.proof();

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    let page = only_page(&proof);
    // 夹具的前提：每一步都真在做事（见 [`plain_page`]）。任何一条不成立，
    // 下面那个等号就是在一张「什么都没发生」的页上成立的，钉不住那一步。
    let crop = page.page.crop().expect("处理成了的页有裁白边那一格");
    assert_eq!(
        (crop.trimmed(), crop.after()),
        (true, CONTENT),
        "夹具的前提：白边裁掉、恰好剩下内容那一块"
    );
    assert_ne!(page.page.size, CONTENT, "夹具的前提：内容真的被缩放过");
    assert!(
        matches!(
            page.page.white_alignment(),
            Some(tonefit::WhiteAlignment::Aligned { paper_white }) if paper_white == fixtures::OFF_GRID_PAPER_WHITE
        ),
        "夹具的前提：纸白真的被提过：{:?}",
        page.page.white_alignment()
    );
    assert!(
        page.page.output.starts_with(plain.sheets()),
        "判定那一张指着样张的去处，不是转换那一趟的输出"
    );
    let written = fs::read(&ran.output).expect("读转换那一趟写出的那一张");
    let proofed = fs::read(&page.page.output).expect("读样张里判定那一档的那一张");
    assert!(
        written == proofed,
        "判定那一张与转换那一趟写出的不是同一串字节（样张 {} 字节，转换 {} 字节）",
        proofed.len(),
        written.len()
    );
    assert_eq!(
        page.page.verdict(),
        ran.verdict(),
        "样张与转换那一趟定下的不是同一档、同一个理由"
    );
}

/// 《参照》那一张解回来是 **8 位灰度**，而且灰调级数**多于最高那一档的格点数**——它没被量化过
/// （spec《Implementation Decisions》第四条：对照本身不许带自己的损伤）。
///
/// 后一问才是要害：参照要是被哪一档量化过（哪怕是最高那一档），解回来的级数就被那一档的格点数
/// 压住了——8 位的容器装一张 16 级的图，头一问照样答得出「8 位」。
#[test]
fn the_reference_sheet_is_eight_bit_gray_and_was_never_quantized() {
    let plain = Plain::new();
    let proof = plain.proof();
    let page = only_page(&proof);

    let reference = fixtures::read_png(&page.reference.file);
    assert_eq!(reference.color_type, png::ColorType::Grayscale);
    assert_eq!(reference.bit_depth, png::BitDepth::Eight);
    assert_eq!(
        reference.size, page.page.size,
        "参照就是缩放到目标尺寸的那一张"
    );

    let top = page
        .scored()
        .map(|(scored, _)| scored.candidate.bit_depth)
        .max()
        .expect("有候选");
    let levels = distinct_levels(&reference.pixels);
    assert!(
        levels > top.levels() as usize,
        "参照解回来只有 {levels} 级灰，没多过最高那一档 {top} 的格点数"
    );
}

/// 每一张候选图**落格**：解回来的取值都在它那一档的格点上
/// （**《落格》，见 `CONTEXT.md` 的《量化》**）。
///
/// 与 `tests/pipeline.rs` 那道闸问的是同一个性质的**另一半**：那一批问 `run` 写出的页，
/// 这一条问样张（spec《Testing Decisions》第二条）。格点不在这里现算——
/// 拿被测的 `quantize` 把 0..=255 打一遍，落下来的那一份就是格点集。
#[test]
fn every_candidate_sheet_is_on_the_grid_of_its_bit_depth() {
    let plain = Plain::new();
    let proof = plain.proof();

    for (scored, sheet) in only_page(&proof).scored() {
        let candidate = scored.candidate;
        let grid = tonefit::quantize(
            &tonefit::GrayImage::new(tonefit::Size::new(256, 1), (0..=255u8).collect()),
            Candidate::new(candidate.bit_depth, tonefit::Dither::Off),
        );
        let written = fixtures::read_png(&sheet.file);
        for &level in &written.pixels {
            assert!(
                grid.pixels().contains(&level),
                "{} 是 {candidate}，却写着格点外的 {level}",
                sheet.file.display()
            );
        }
    }
}

/// 样张里**一个 tEXt 块都没有**：它不写《记录》——写了，下一趟幂等会把样张读回来当上一趟的输出
/// （spec《Implementation Decisions》第五条）。
///
/// **读 tEXt 的那一手先问一遍阳性对照**：同一页带着记录照做一趟，那一张读得出 tEXt。
/// 少了它，读法哪天失灵（换了个读不到文本块的解法），这一条照样一片安静。
#[test]
fn no_sheet_carries_a_text_chunk() {
    let plain = Plain::new();
    let recorded = tonefit::run(&Request {
        metadata: true,
        ..plain.request.clone()
    })
    .expect("带着记录照做一趟");
    assert!(
        !fixtures::read_png_text(&recorded.volumes[0].pages[0].output).is_empty(),
        "阳性对照：带着记录写出的那一张该读得出 tEXt"
    );

    let proof = plain.proof();
    let page = only_page(&proof);
    for sheet in page.candidates.iter().chain([&page.reference]) {
        assert_eq!(
            fixtures::read_png_text(&sheet.file),
            Vec::new(),
            "{} 带着 tEXt",
            sheet.file.display()
        );
    }
}

/// 跑完之后**源文件的字节与 mtime 一格没动**：样张是量具，源库只读（spec 的 story 28）。
#[test]
fn proofing_leaves_the_source_bytes_and_mtime_as_they_were() {
    let plain = Plain::new();
    let bytes = fs::read(&plain.source).expect("读源");
    let mtime = modified(&plain.source);

    plain.proof();

    assert!(
        fs::read(&plain.source).expect("读源") == bytes,
        "源文件的字节变了"
    );
    assert_eq!(modified(&plain.source), mtime, "源文件的 mtime 变了");
}

/// 一个文件此刻的 mtime。
fn modified(path: &std::path::Path) -> std::time::SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .expect("读 mtime")
}

/// 一串像素里出现了几种取值（灰调级数）。
fn distinct_levels(pixels: &[u8]) -> usize {
    let mut seen = [false; 256];
    for &level in pixels {
        seen[level as usize] = true;
    }
    seen.iter().filter(|&&hit| hit).count()
}

/// **提白上限取 0 时纸白照样量一遍**：样张读得出「这一页的纸白是多少、钳掉多宽」，
/// 哪怕这一趟点名关掉了提白（样张 spec 的 story 12；停车场 Q918 的处置）。
///
/// 照做那一趟在这里一个数都不给——上限取 0 时它连纸白都不量，报告说的是「做过什么」。
/// 样张说的是「开了会怎样」，与预览同一边；而它不是预览，这一格因此不由 `Mode` 推
/// （库内的 `WhiteWhenOff`）。**两边同时问**：照做那一趟那一张读到的是「没开」，
/// 样张那一张读到的是真的纸白——少了前一问，这一条在「谁都量」的实现上也照绿。
#[test]
fn with_the_limit_at_zero_a_proof_still_reads_the_paper_white() {
    let plain = Plain::new();
    let off = Request {
        white_align_limit: tonefit::WhiteAlignLimit::new(0),
        ..plain.request.clone()
    };

    let ran = tonefit::run(&off).expect("转换那一趟");
    let proof = tonefit::write_proof(&plain.source, &off, &plain.sheets()).expect("出样张");

    assert_eq!(
        ran.volumes[0].pages[0].white_alignment(),
        Some(tonefit::WhiteAlignment::Off),
        "照做那一趟上限取 0 就不量"
    );
    assert_eq!(
        only_page(&proof).page.white_alignment(),
        Some(tonefit::WhiteAlignment::OverTheLimit {
            paper_white: fixtures::OFF_GRID_PAPER_WHITE
        }),
        "样张上限取 0 也读得出纸白，而且说得出它超过了这一趟的上限"
    );
}
