//! 样张（`CONTEXT.md` 的《样张》），在库的第四个 seam——[`tonefit::write_proof`]——上测。
//!
//! 只断言**外部看得见的事实**：交出来的数据、落到盘上的文件有什么性质、源文件有没有被动过。
//! 不断内部怎么走——与 `tests/pipeline.rs` 的模块文档同一句话（spec《Testing Decisions》）。

mod fixtures;

use std::fs;
use std::path::PathBuf;

use fixtures::Workspace;
use tonefit::{
    BitDepth, Candidate, Dither, PageBranch, PageColor, Proof, ProofPage, Reason, Request, Sheets,
    Verdict,
};

/// 一张**普通页**：单页、不拆、灰度路径、门成立——`proof-sheet/02` 的对象。
///
/// **每一步都得真在做事**，神谕那一条比的才不是一张「什么都没发生」的页：
///
/// - **裁白边**：内容四周一圈纯白边（[`MARGIN`]），裁完恰好剩下内容那一块；
/// - **缩放**：内容高 [`CONTENT`] 要缩到面板高，而页上有**硬边**（一竖条墨）——
///   换一个缩放算法，硬边两侧的振铃就不一样。只有一张线性渐变的页做不到这一条：
///   对称的核把线性斜坡原样复现，换算法一个字节都不变（本票落地时按反跑过一次，亲眼看见它照绿）；
/// - **纸色提白**：纸白是离格 2 级的 [`fixtures::OFF_GRID_PAPER_WHITE`]，[`Staged`] 点名的上限钳得动它；
/// - **画质分、量化、编码**：那一竖条从纯黑爬到纸白之下的灰调，六档各有各的样子。
///
/// 内容本身是 [`fixtures::page_with_paper_white`]（四边顶着墨，裁白边正好停在它的边上），
/// 内容那一块多大由 `content` 说（[`CONTENT`] 或 [`WIDE`]）。
fn plain_page(content: tonefit::Size) -> image::DynamicImage {
    let inked = fixtures::page_with_paper_white(content, fixtures::OFF_GRID_PAPER_WHITE).to_luma8();
    let mut page = image::GrayImage::from_pixel(
        content.width + 2 * MARGIN,
        content.height + 2 * MARGIN,
        image::Luma([255]),
    );
    image::imageops::replace(&mut page, &inked, i64::from(MARGIN), i64::from(MARGIN));
    image::DynamicImage::ImageLuma8(page)
}

/// 普通页里内容那一块：比面板高（1680）高一截，缩放因此真的在缩；宽高比远够不上跨页候选。
const CONTENT: tonefit::Size = tonefit::Size::new(1200, 1800);

/// **比面板更宽**的那一种普通页：宽高比 0.89 胜过面板的 0.75，而够不上跨页候选（面板比的 1.5 倍）。
///
/// 两种缩放方式在它身上**分得开**：以高为准缩到面板高（宽越过面板宽），fit-inside 让宽贴住面板宽、
/// 高落在面板高之下。[`CONTENT`] 那一张做不到——普通漫画页两种方式产出同一个尺寸
/// （`--fit` 的帮助里写着），`--fit inside` 在它身上是空操作。
const WIDE: tonefit::Size = tonefit::Size::new(1600, 1800);

/// 内容四周那一圈纯白边有多宽。
const MARGIN: u32 = 96;

/// 一张图摆在一个卷里（`卷/001.png`），外加一份**不写记录**的请求。
///
/// 卷级那几格（点名的卷、输出根）样张一格都不读（见 [`tonefit::write_proof`]），
/// 这里照 `run` 要的填：同一份交给 `run`，写出去的就是神谕那一条要比的那一张。
///
/// **处理选项那几格点名取值**，不借默认值（`docs/agents/testing.md`）：夹具让每一步都真在做事，
/// 靠的是这几个数——默认值哪天挪了（比如提白上限降到 2 以下），神谕那一条就会在一张
/// 什么都没提白的页上照绿。这几个数恰好是默认那一套；走非默认选项的用例各自改掉它点名的那几格。
struct Staged {
    space: Workspace,
    source: PathBuf,
    request: Request,
}

impl Staged {
    /// 一张普通页，内容那一块是 [`CONTENT`]。
    fn plain() -> Self {
        Self::plain_of(CONTENT)
    }

    /// 内容那一块是 `content` 那么大的一张普通页。
    fn plain_of(content: tonefit::Size) -> Self {
        Self::of(&plain_page(content))
    }

    /// 摆上 `image` 这一张。
    fn of(image: &image::DynamicImage) -> Self {
        Self::placing(|volume| volume.page("001.png", image))
    }

    /// 卷里摆上 `put` 放进去的东西，样张要出的就是它交回的那个路径——
    /// 一张解不开的图、一个转换那一趟不当页的文件，都得先摆进卷里，神谕那一侧的 `run` 才问得着。
    fn placing(put: impl FnOnce(&fixtures::Volume) -> PathBuf) -> Self {
        let space = Workspace::new();
        let volume = space.volume("卷");
        let source = put(&volume);
        let request = Request {
            fit: tonefit::FitMode::Height,
            crop: true,
            // 拆分那三格也点名：跨页那几条靠判定宽度把夹具认成跨页候选，靠阅读方向排两半的先后。
            split: tonefit::SplitRule {
                on: true,
                threshold: tonefit::SplitThreshold::parse("1.5").expect("判定宽度"),
                order: tonefit::ReadingOrder::RightToLeft,
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

/// 这一张图的那一叠。没切开的一张图就是一叠：普通页、关掉拆分的跨页、连续跨页。
fn only_page(proof: &Proof) -> &ProofPage {
    match proof.pages.as_slice() {
        [only] => only,
        pages => panic!("没切开的一张图该出一叠，出了 {} 叠", pages.len()),
    }
}

/// **一张普通页出一叠**：去处不在就建出来，里面一个候选一张、外加《参照》一张；
/// 交出来的候选集恰好是这块面板上这一页的门派得出的那一整套；
/// 每个文件名说得出它是哪一页、哪一个候选。
#[test]
fn a_plain_page_gets_one_sheet_for_every_candidate_and_one_reference() {
    let plain = Staged::plain();
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
        .sheets
        .iter()
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
    let plain = Staged::plain();

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

/// **神谕在非默认选项上再跑一遍**：`--fit inside` 加一个非默认的缩放算法，两边吃同一套，
/// 判定那一张照旧与 `run`（`--no-metadata`）写出的那一张逐字节相同（`proof-sheet/03`）。
///
/// 默认那一套上的神谕钉不住「样张跟着我给的选项走」（spec 的 story 13、15）：样张要是把
/// 缩放方式或缩放算法写死成默认值，默认那一趟照样两边相同。两件都先在**转换那一趟**上断言
/// 真在起作用——fit-inside 让这一页的宽贴住面板、高落在面板高之下（以高为准的话高恰是面板高）；
/// 换回默认算法，写出去的就不是同一串字节。前提只问神谕那一侧，样张哪一格没跟上，
/// 红在下面那个等号上，不红在前提上。
#[test]
fn the_verdict_sheet_follows_a_non_default_fit_and_filter_byte_for_byte() {
    let wide = Staged::plain_of(WIDE);
    let request = Request {
        fit: tonefit::FitMode::Inside,
        filter: tonefit::Filter::Hamming,
        ..wide.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    let panel = request.profile.panel().resolution;
    assert!(
        ran.size.width == panel.width && ran.size.height < panel.height,
        "夹具的前提：fit-inside 让宽贴住面板（{panel:?}），这一页却是 {:?}",
        ran.size
    );
    let by_default_filter = tonefit::run(&Request {
        filter: tonefit::Filter::Lanczos3,
        output_root: wide.space.dir("默认算法"),
        ..request.clone()
    })
    .expect("换回默认算法转换一趟");
    assert!(
        fs::read(&ran.output).expect("读转换那一趟写出的那一张")
            != fs::read(&by_default_filter.volumes[0].pages[0].output)
                .expect("读默认算法写出的那一张"),
        "夹具的前提：这一页换缩放算法，写出去的就不是同一串字节"
    );

    let proof = tonefit::write_proof(&wide.source, &request, &wide.sheets()).expect("出样张");
    let page = only_page(&proof);
    assert_eq!(
        page.page.verdict(),
        ran.verdict(),
        "样张与转换那一趟定下的不是同一档、同一个理由"
    );
    let written = fs::read(&ran.output).expect("读转换那一趟写出的那一张");
    let proofed = fs::read(&page.page.output).expect("读样张里判定那一档的那一张");
    assert!(
        written == proofed,
        "判定那一张与转换那一趟写出的不是同一串字节（样张 {} 字节，转换 {} 字节）",
        proofed.len(),
        written.len()
    );
}

/// **两道覆盖项把判定顶死，却不裁样张的候选集**（样张 spec《Implementation Decisions》第三条；
/// `CONTEXT.md` 的《覆盖顶死》）。
///
/// `--bit-depth` 与 `--dither` 两维都点名：转换那一趟的候选集裁到只剩一个，判定被顶掉、理由是覆盖。
/// 样张照出整套——覆盖项裁掉的是「这一趟不要」，不是「这一页不可能」，而样张存在的理由正是并排看——
/// 判定那一格说的是被顶死成了哪一档，那一张仍与 `run` 同一套选项写出的那一张逐字节相同。
///
/// 顶死的那一档取**画质分说它不达标**的那一档：判定落到它身上只可能是被顶死的，
/// 不是画质分恰好判到了同一档（夹具的前提，先断言）。
#[test]
fn an_override_pins_the_verdict_and_leaves_every_candidate_on_the_sheets() {
    let plain = Staged::plain();
    let pinned = Candidate::new(BitDepth::One, Dither::Off);
    let request = Request {
        bit_depth: Some(pinned.bit_depth),
        dither: Some(pinned.dither),
        ..plain.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&plain.source, &request, &plain.sheets()).expect("出样张");

    let page = only_page(&proof);
    let gate = page.page.gate().expect("普通页走灰度路径，有门");
    let proofed: Vec<Candidate> = page.scored().map(|(scored, _)| scored.candidate).collect();
    assert_eq!(
        proofed,
        Candidate::all(request.profile.panel().gray_levels, gate),
        "覆盖项裁了样张的候选集"
    );
    assert_eq!(
        fixtures::directory_members(&plain.sheets()).len(),
        proofed.len() + 1,
        "去处里该是整套候选各一张、外加参照一张"
    );
    let (scored, _) = page
        .scored()
        .find(|(scored, _)| scored.candidate == pinned)
        .expect("顶死的那一档在整套里");
    assert!(
        !request.profile.threshold().admits(scored.score),
        "夹具的前提：画质分说 {pinned} 不达标（{:?}）",
        scored.score
    );
    assert_eq!(
        page.page.verdict(),
        Some(Verdict {
            candidate: pinned,
            reason: Reason::Override,
        }),
        "判定那一格没说被顶死成了哪一档"
    );

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert_eq!(
        page.page.verdict(),
        ran.verdict(),
        "样张与转换那一趟顶死的不是同一档"
    );
    let written = fs::read(&ran.output).expect("读转换那一趟写出的那一张");
    let proofed = fs::read(&page.page.output).expect("读样张里判定那一档的那一张");
    assert!(
        written == proofed,
        "顶死的那一张与转换那一趟写出的不是同一串字节（样张 {} 字节，转换 {} 字节）",
        proofed.len(),
        written.len()
    );
}

/// **只点一维覆盖项时判定没被顶死**：它只收窄判定从哪几个里挑，样张照旧出整套。
///
/// `--bit-depth` 单点一维、门成立时，转换那一趟的候选集剩下那一档的抖动与不抖两个——
/// 画质分照旧说了算，理由是判出来的那一种，不是覆盖（`CONTEXT.md` 的《覆盖顶死》：
/// 顶死说的是**裁到只剩一个**）。把「点了覆盖项」读成「判定被顶死」，这一条就红。
///
/// 点名的那一档取**整套判下来不会落到**的那一档（夹具的前提，另出一叠先断言）：
/// 不然覆盖项有没有被读进判定，这一条分不出来。
#[test]
fn a_single_override_narrows_the_verdict_but_not_the_sheets() {
    let plain = Staged::plain();
    let named = BitDepth::Two;
    let request = Request {
        bit_depth: Some(named),
        dither: None,
        ..plain.request.clone()
    };

    let unpinned = tonefit::write_proof(&plain.source, &plain.request, &plain.space.dir("整套判"))
        .expect("不点覆盖项出一叠");
    let judged_whole = only_page(&unpinned).page.verdict().expect("灰度页有判定");
    assert_ne!(
        judged_whole.candidate.bit_depth, named,
        "夹具的前提：整套判下来本来就不是 {named}"
    );

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&plain.source, &request, &plain.sheets()).expect("出样张");

    let page = only_page(&proof);
    let gate = page.page.gate().expect("普通页走灰度路径，有门");
    let proofed: Vec<Candidate> = page.scored().map(|(scored, _)| scored.candidate).collect();
    assert_eq!(
        proofed,
        Candidate::all(request.profile.panel().gray_levels, gate),
        "覆盖项裁了样张的候选集"
    );
    let verdict = page.page.verdict().expect("灰度页有判定");
    assert_eq!(
        verdict.candidate.bit_depth, named,
        "判定没落在点名的那一档上"
    );
    assert_ne!(
        verdict.reason,
        Reason::Override,
        "只点一维、还剩两个可挑，判定不该说被顶死"
    );

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert_eq!(
        Some(verdict),
        ran.verdict(),
        "样张与转换那一趟判的不是同一档"
    );
    assert!(
        fs::read(&ran.output).expect("读转换那一趟写出的那一张")
            == fs::read(&page.page.output).expect("读样张里判定那一档的那一张"),
        "判定那一张与转换那一趟写出的不是同一串字节"
    );
}

/// **`--dither fs` 撞上一页没贴合屏幕：样张与转换那一趟说同一句拒绝，去处里一张都没有**（互锁 ③）。
///
/// 覆盖项不裁样张的候选集，门那一侧的整套因此照样编得出来；可转换那一趟在这一页上一个字节都不写，
/// 样张就没有「判定那一张」可给——它照转换那一趟拒绝，不自己挑一档顶上。
///
/// 页比面板小、fit-inside 不放大：目标尺寸哪条边都贴不住面板，门不成立。
#[test]
fn a_dither_override_the_geometry_gate_shuts_is_refused_as_run_refuses_it() {
    let small = Staged::plain_of(SMALL);
    let request = Request {
        fit: tonefit::FitMode::Inside,
        dither: Some(Dither::FloydSteinberg),
        ..small.request.clone()
    };

    let ran = tonefit::run(&request).expect_err("转换那一趟该拒绝");
    let proofed = tonefit::write_proof(&small.source, &request, &small.sheets())
        .expect_err("样张该照转换那一趟拒绝");

    let refused = tonefit::Interlock::DitherOutsideTheGate.to_string();
    for said in [format!("{ran:#}"), format!("{proofed:#}")] {
        assert!(said.contains(&refused), "说的不是互锁 ③ 那一句：{said}");
        assert!(
            said.contains(&small.source.display().to_string()),
            "没指出撞上的是哪一页：{said}"
        );
    }
    assert!(
        format!("{ran:#}").contains(&format!("{proofed:#}")),
        "样张与转换那一趟说的不是同一句：\n样张 {proofed:#}\n转换 {ran:#}"
    );
    assert!(
        !small.sheets().exists(),
        "拒绝了还在去处里留了东西：{:?}",
        fixtures::directory_members(&small.sheets())
    );
}

/// 比面板小的那一种普通页：fit-inside 不放大，它原样出，门不成立。
const SMALL: tonefit::Size = tonefit::Size::new(600, 900);

/// 《参照》那一张解回来是 **8 位灰度**，而且灰调级数**多于最高那一档的格点数**——它没被量化过
/// （spec《Implementation Decisions》第四条：对照本身不许带自己的损伤）。
///
/// 后一问才是要害：参照要是被哪一档量化过（哪怕是最高那一档），解回来的级数就被那一档的格点数
/// 压住了——8 位的容器装一张 16 级的图，头一问照样答得出「8 位」。
#[test]
fn the_reference_sheet_is_eight_bit_gray_and_was_never_quantized() {
    let plain = Staged::plain();
    let proof = plain.proof();
    let page = only_page(&proof);

    let Sheets::Gray { reference, .. } = &page.sheets else {
        panic!("普通页走灰度路径，那一叠有参照");
    };
    let reference = fixtures::read_png(&reference.file);
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
    let plain = Staged::plain();
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
                grid.image().pixels().contains(&level),
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
    let plain = Staged::plain();
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
    for sheet in page.sheets.iter() {
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
    let plain = Staged::plain();
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
    let plain = Staged::plain();
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

// ── 跨页（`proof-sheet/04`）────────────────────────────────────────────────
//
// 样张按**输出页**出，不按源页（样张 spec《Implementation Decisions》第六条）：拆开的跨页
// 真会被写出去的是那两半。这一段的每一条都拿**同一份请求交给 `run`** 当对照——
// 名字、次序、裁切窗口、判定、字节，问的都是「与转换那一趟切出来的那一张是不是同一张」。

/// 一张**两半不一样的跨页**：中缝一条，两半各是一块纸白离格的内容（[`framed`]），
/// 而**只有左半上下还留着白边**——右半顶天立地，左半矮一截、竖着摆在正中。
///
/// 这样摆买三件事：
///
/// - **拆得开**：中缝是一条贯穿全高的纯白，落在页宽正中；整页宽高比 1.24，
///   够得上跨页候选（基准面板 0.75 乘上 [`Staged`] 点名的判定宽度 1.5，是 1.13）。
/// - **每半各裁各的看得出来**：整页那一道裁白边一行都拿不走（右半顶天立地，每一行都有墨），
///   左半上下那两截白边要切开之后「每半再裁」才收得走——两半的裁切窗口因此不一样高，
///   拿一个两半共用的裁切框就对不上（`crate::crop` 的模块文档：不取卷级裁切框）。
/// - **门可以一半成立一半不成立**：左半裁完 [`LEFT`] 比面板小，fit-inside 不放大、
///   哪条边都贴不住；右半 [`RIGHT`] 比面板高，缩下来贴住面板高。以高为准时两半都贴得住。
fn lopsided_spread() -> image::DynamicImage {
    let mut page = image::GrayImage::from_pixel(
        LEFT.width + GUTTER + RIGHT.width,
        RIGHT.height,
        image::Luma([255]),
    );
    let top = (RIGHT.height - LEFT.height) / 2;
    image::imageops::replace(&mut page, &framed(LEFT), 0, i64::from(top));
    image::imageops::replace(&mut page, &framed(RIGHT), i64::from(LEFT.width + GUTTER), 0);
    image::DynamicImage::ImageLuma8(page)
}

/// 跨页左半那块内容：比面板（1264×1680）两条边都小，高又留得住左半窗口高的一半以上
/// （裁白边留不到一半就整块原样通过，见 `crate::crop` 的 `MIN_KEPT`）。
const LEFT: tonefit::Size = tonefit::Size::new(1100, 1000);

/// 跨页右半那块内容：顶天立地，比面板高。与左半一样宽，中缝因此落在页宽正中。
const RIGHT: tonefit::Size = tonefit::Size::new(1100, 1800);

/// 中缝多宽，单位是列。占页宽 1.8%，落在实测的 0.17%–12.47% 之间（measurements 的《跨页拆分》）。
const GUTTER: u32 = 40;

/// 一半的内容：纸白离格的那一页（[`fixtures::page_with_paper_white`]），外面再压一圈 [`FRAME`] 宽的墨。
///
/// 那一圈是给**中缝检测**看的：那一页纸白那两条竖条上，每一列只有上下边框那 8 行是墨，
/// 而一列要有页高 0.5% 的墨（1800 高的页上是 9 行）才不算空白列。不压这一圈，
/// 贴着中缝的那两条纸白会被量进中缝里——沟宽越过上限，整页判成连续跨页。
fn framed(content: tonefit::Size) -> image::GrayImage {
    let mut half =
        fixtures::page_with_paper_white(content, fixtures::OFF_GRID_PAPER_WHITE).to_luma8();
    for (x, y, pixel) in half.enumerate_pixels_mut() {
        if x < FRAME || y < FRAME || x + FRAME >= content.width || y + FRAME >= content.height {
            *pixel = image::Luma([0]);
        }
    }
    half
}

/// [`framed`] 那一圈墨多宽。
const FRAME: u32 = 16;

/// 一卷输出页的成员名，按阅读顺序——这就是 `run` 给的输出页名。
fn run_names(volume: &tonefit::VolumeReport) -> Vec<String> {
    volume
        .pages
        .iter()
        .map(|page| fixtures::relative_name(&volume.output, &page.output))
        .collect()
}

/// 一叠样张里每一张的文件名，候选那几张在前、参照收尾。
fn sheet_names(stack: &ProofPage) -> Vec<String> {
    stack
        .sheets
        .iter()
        .map(|sheet| {
            sheet
                .file
                .file_name()
                .expect("有文件名")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// `run` 写出的那一页（`001-1.png`）配上这一叠，每一张**该叫**什么：页那一截照搬那个成员名，
/// 接上是哪一个候选，参照那一张接词条名。
fn expected_sheet_names(ran: &tonefit::PageReport, stack: &ProofPage) -> Vec<String> {
    let page = ran.output.file_stem().expect("有文件名").to_string_lossy();
    stack
        .scored()
        .map(|(scored, _)| format!("{page}.{}.png", scored.candidate))
        .chain([format!("{page}.参照.png")])
        .collect()
}

/// **一张跨页出两叠，各自的输出页名与 `run` 给的同一套**：带着那一族的第几张，次序是阅读顺序
/// （样张 spec 的 story 17、《Implementation Decisions》第六条）。
///
/// 名字不在这里手写第二份：每一叠该叫什么从 `run` 写出的那个成员名推（[`expected_sheet_names`]）。
/// 手写的只有前提那一句——转换那一趟真把它切成了 `001-1`、`001-2` 两张。
#[test]
fn a_spread_gets_one_proof_page_per_half_under_the_names_run_gives_them() {
    let spread = Staged::of(&lopsided_spread());

    let report = tonefit::run(&spread.request).expect("转换那一趟");
    let proof = spread.proof();

    let ran = &report.volumes[0];
    assert_eq!(
        run_names(ran),
        ["001-1.png", "001-2.png"],
        "夹具的前提：转换那一趟把它切成两张"
    );
    assert_eq!(proof.pages.len(), 2, "一张跨页该出两叠");
    for (stack, ran) in proof.pages.iter().zip(&ran.pages) {
        assert_eq!(
            sheet_names(stack),
            expected_sheet_names(ran, stack),
            "这一叠的名字不是 `run` 给这一半的那一个"
        );
        assert_eq!(
            stack.page.cut(),
            ran.cut(),
            "这一叠不是转换那一趟排在同一位上的那一半"
        );
    }
    let on_disk = fixtures::directory_members(&spread.sheets());
    let mut named: Vec<String> = proof.pages.iter().flat_map(sheet_names).collect();
    named.sort();
    assert_eq!(named, on_disk, "交出来的那两叠就是盘上那几张");
}

/// **关掉拆分，同一张跨页出一叠**：整页那一张，名字就是 `run` 给整页的那一个
/// （样张 spec 的 story 18：它仍旧和我那一趟的产物对得上）。判定那一张也一并比字节。
#[test]
fn with_splitting_off_a_spread_gets_one_proof_page() {
    let spread = Staged::of(&lopsided_spread());
    let request = Request {
        split: tonefit::SplitRule {
            on: false,
            ..spread.request.split
        },
        ..spread.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&spread.source, &request, &spread.sheets()).expect("出样张");

    let ran = &report.volumes[0];
    assert_eq!(
        run_names(ran),
        ["001.png"],
        "夹具的前提：关掉拆分，整页一张"
    );
    let stack = only_page(&proof);
    assert_eq!(stack.page.cut(), None, "关掉拆分还切开了");
    assert_eq!(
        sheet_names(stack),
        expected_sheet_names(&ran.pages[0], stack),
        "整页那一叠的名字不是 `run` 给整页的那一个"
    );
    assert!(
        fs::read(&ran.pages[0].output).expect("读转换那一趟写出的那一张")
            == fs::read(&stack.page.output).expect("读样张里判定那一档的那一张"),
        "判定那一张与转换那一趟写出的不是同一串字节"
    );
}

/// **找不到中缝的连续跨页出一叠**，而交出来的数据说得出它**没被切开**：它够得上跨页候选
/// （不是没判成候选），挡下它的是「没有沟」那一关（`CONTEXT.md` 的《连续跨页》）。
///
/// 两格一起问：候选那一格为真、切口那一格为空。只问后一格，一张够不上候选的普通页也答得一样。
#[test]
fn a_continuous_spread_gets_one_proof_page_that_says_it_was_not_cut() {
    // 竖直渐变、四边顶着墨：每一列长得一样，一条空白列都挑不出来；裁白边不改它的宽高比。
    let spread = Staged::of(&fixtures::full_bleed_gradient(CONTINUOUS));

    let report = tonefit::run(&spread.request).expect("转换那一趟");
    let proof = spread.proof();

    let ran = &report.volumes[0];
    assert_eq!(run_names(ran), ["001.png"], "夹具的前提：转换那一趟没切它");
    let stack = only_page(&proof);
    assert!(
        stack.page.spread_candidate(),
        "交出来的数据说不出它是跨页候选：一张连续跨页被说成了没判成候选的普通页"
    );
    assert_eq!(stack.page.cut(), None, "连续跨页被切开了");
    assert_eq!(
        (stack.page.spread_candidate(), stack.page.cut()),
        (ran.pages[0].spread_candidate(), ran.pages[0].cut()),
        "拆分那两级与转换那一趟答得不一样"
    );
    assert_eq!(
        sheet_names(stack),
        expected_sheet_names(&ran.pages[0], stack),
        "这一叠的名字不是 `run` 给整页的那一个"
    );
}

/// 连续跨页：宽高比 1.43，够得上跨页候选（[`Staged`] 点名的判定宽度下是 1.13）；
/// 高恰是面板高，以高为准一步都不缩。
const CONTINUOUS: tonefit::Size = tonefit::Size::new(2400, 1680);

/// **每一半各自裁过白边**：两半的裁切窗口各是各的，与 `run` 给那一半的窗口逐格相同
/// （样张 spec 的 story 19；`crate::crop` 的模块文档：逐页各裁各的，不取卷级裁切框）。
///
/// 左半上下那两截白边只有「每半再裁」收得走（见 [`lopsided_spread`]）：两半窗口不一样高，
/// 各自恰好是自己那块内容的高。拿一个两半共用的框去裁，两半就一样高。
#[test]
fn each_half_of_a_spread_is_cropped_on_its_own() {
    let spread = Staged::of(&lopsided_spread());

    let report = tonefit::run(&spread.request).expect("转换那一趟");
    let proof = spread.proof();

    let window = |page: &tonefit::PageReport| page.crop().expect("处理成了的页有裁白边那一格");
    // 右开：右半在先（[`Staged`] 点名的阅读方向）。
    let [right, left] = proof.pages.as_slice() else {
        panic!("一张跨页该出两叠，出了 {} 叠", proof.pages.len());
    };
    assert_eq!(
        (window(&right.page).after(), window(&left.page).after()),
        (RIGHT, LEFT),
        "两半没各自裁到自己那块内容"
    );
    for (stack, ran) in proof.pages.iter().zip(&report.volumes[0].pages) {
        assert_eq!(
            window(&stack.page),
            window(ran),
            "这一半的裁切窗口与转换那一趟的不一样"
        );
    }
}

/// **神谕在跨页上跑一遍**：两半各比一次，各自与 `run --no-metadata` 写出的那一半**逐字节相同**，
/// 判定也是同一档、同一个理由（spec《Testing Decisions》第一条：「跨页那一张再来一遍，两半各比一次」）。
///
/// 前提照普通页那一条的规矩先问：两半都真的各自裁到了自己那块内容、缩放过、提过白——
/// 任何一条不成立，下面那个等号就是在一张「什么都没发生」的半页上成立的。
/// 裁白边那一问比的是裁完的尺寸，不问「裁没裁」：半页的窗口叠在整张源页上，
/// 光是切开那一刀就让「裁没裁」答是（`Crop::then`）。
#[test]
fn both_halves_of_a_spread_are_byte_for_byte_what_run_writes_without_metadata() {
    let spread = Staged::of(&lopsided_spread());

    let report = tonefit::run(&spread.request).expect("转换那一趟");
    let proof = spread.proof();

    let halves = &report.volumes[0].pages;
    assert_eq!(halves.len(), 2, "夹具的前提：转换那一趟把它切成两张");
    assert_eq!(proof.pages.len(), 2, "一张跨页该出两叠");
    // 右开：右半在先（[`Staged`] 点名的阅读方向）。
    for ((stack, ran), content) in proof.pages.iter().zip(halves).zip([RIGHT, LEFT]) {
        let page = &stack.page;
        let crop = page.crop().expect("处理成了的页有裁白边那一格");
        assert_eq!(
            crop.after(),
            content,
            "夹具的前提：这一半裁到了自己那块内容"
        );
        assert_ne!(page.size, crop.after(), "夹具的前提：这一半真的被缩放过");
        assert!(
            matches!(
                page.white_alignment(),
                Some(tonefit::WhiteAlignment::Aligned { paper_white }) if paper_white == fixtures::OFF_GRID_PAPER_WHITE
            ),
            "夹具的前提：这一半的纸白真的被提过：{:?}",
            page.white_alignment()
        );
        assert_eq!(
            page.verdict(),
            ran.verdict(),
            "{:?} 那一半：样张与转换那一趟定下的不是同一档、同一个理由",
            page.cut()
        );
        let written = fs::read(&ran.output).expect("读转换那一趟写出的那一半");
        let proofed = fs::read(&page.output).expect("读样张里判定那一档的那一张");
        assert!(
            written == proofed,
            "{:?} 那一半：判定那一张与转换那一趟写出的不是同一串字节（样张 {} 字节，转换 {} 字节）",
            page.cut(),
            proofed.len(),
            written.len()
        );
    }
}

/// **两半的门分了家、只点灰阶档位时，「顶死没有」照转换那一趟问——不拿某一块剩下几个候选去问**
/// （停车场 Q1014；`CONTEXT.md` 的《覆盖顶死》）。
///
/// `--fit inside` 上 [`lopsided_spread`] 的左半贴不住面板（门不成立，候选里没有抖动那一维），
/// 右半贴得住。只点 `--bit-depth` 一维：左半那一套只剩一个候选，右半那一套还剩抖与不抖两个。
/// 转换那一趟判定没被顶死，左半那一档是它自己那条曲线判出来的。一块一块问的话，左半那一块只剩一个候选，
/// 会被说成顶死：**字节相同，理由不同**。所以这一条比的是整个判定，不只是字节。
/// 门处处不成立的那一种见 `a_single_override_on_a_page_the_geometry_gate_shuts_is_judged_as_run_judges_it`。
///
/// 前提问的是转换那一趟：两半的门真的分了家，左半的理由真的不是覆盖。
#[test]
fn a_single_override_on_a_spread_whose_halves_part_at_the_gate_is_judged_as_run_judges_it() {
    let spread = Staged::of(&lopsided_spread());
    let request = Request {
        fit: tonefit::FitMode::Inside,
        bit_depth: Some(BitDepth::Two),
        dither: None,
        ..spread.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let halves = &report.volumes[0].pages;
    let holds: Vec<bool> = halves
        .iter()
        .map(|page| page.gate().expect("灰度路径上有门").holds())
        .collect();
    assert_eq!(
        holds,
        [true, false],
        "夹具的前提：右半贴得住面板、左半贴不住"
    );
    assert_ne!(
        halves[1].verdict().expect("灰度页有判定").reason,
        Reason::Override,
        "夹具的前提：转换那一趟没把左半说成被顶死"
    );

    let proof = tonefit::write_proof(&spread.source, &request, &spread.sheets()).expect("出样张");
    assert_eq!(proof.pages.len(), 2, "一张跨页该出两叠");
    for (stack, ran) in proof.pages.iter().zip(halves) {
        assert_eq!(
            stack.page.verdict(),
            ran.verdict(),
            "{:?} 那一半：样张与转换那一趟定下的不是同一档、同一个理由",
            stack.page.cut()
        );
        assert!(
            fs::read(&ran.output).expect("读转换那一趟写出的那一半")
                == fs::read(&stack.page.output).expect("读样张里判定那一档的那一张"),
            "{:?} 那一半：判定那一张与转换那一趟写出的不是同一串字节",
            stack.page.cut()
        );
    }
}

/// **一张门不成立的图只点灰阶档位：候选只剩一个，判定照转换那一趟逐页判，不说被顶死**
/// （one-source/02；`CONTEXT.md` 的《覆盖顶死》）。
///
/// 页比面板小、fit-inside 不放大（[`SMALL`]）：门不成立，候选里没有抖动那一维，`--bit-depth 2`
/// 一点名就只剩 `2bit` 一个。默认那条路上顶死只认开卷之前答得出的那一种，这一角答不出——
/// 门成立的页若在卷里，它们还剩抖与不抖两个——转换那一趟因此逐页判。样张问的是同一句：
/// 按「这张图的其余页那一组只剩一个」去问的话，它会说被顶死，**字节相同，理由不同**。
#[test]
fn a_single_override_on_a_page_the_geometry_gate_shuts_is_judged_as_run_judges_it() {
    let small = Staged::plain_of(SMALL);
    let request = Request {
        fit: tonefit::FitMode::Inside,
        bit_depth: Some(BitDepth::Two),
        dither: None,
        envelope: false,
        ..small.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&small.source, &request, &small.sheets()).expect("出样张");

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert_eq!(
        ran.gate(),
        Some(tonefit::GeometryGate::Broken),
        "夹具的前提：转换那一趟在这一页上门不成立"
    );
    assert_eq!(
        ran.scores().len(),
        1,
        "夹具的前提：转换那一趟在这一页上只剩一个候选"
    );
    let page = only_page(&proof);
    let verdict = page.page.verdict().expect("灰度页有判定");
    assert_ne!(
        verdict.reason,
        Reason::Override,
        "开卷之前答不出的那一角，样张说成了被顶死"
    );
    assert_eq!(
        Some(verdict),
        ran.verdict(),
        "样张与转换那一趟定下的不是同一档、同一个理由"
    );
    assert!(
        fs::read(&ran.output).expect("读转换那一趟写出的那一张")
            == fs::read(&page.page.output).expect("读样张里判定那一档的那一张"),
        "判定那一张与转换那一趟写出的不是同一串字节"
    );
}

// ── 彩页与尺寸未贴合屏幕（`proof-sheet/05`）──────────────────────────────────
//
// 两种页拿不到整叠样张，理由各不相同：彩色面板上的彩页走彩色分支、不量化，没有候选可比；
// 尺寸未贴合屏幕的页候选里没有抖动那一维。两种都拿**同一份请求交给 `run`** 当对照。

/// 彩色面板：与 [`Staged`] 点名的基准面板同分辨率、同 PPI，差的只有彩色那一项（ADR 0010 决定第 1 条）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

/// 一张**彩页**：普通页（[`plain_page`]）染成真彩色（[`fixtures::colorize`]），白边仍是纯白。
///
/// 染过之后裁白边、缩放两步照样真在做事：四周那圈白边还在，内容那一块仍比面板高。
fn color_plain_page() -> image::DynamicImage {
    fixtures::colorize(&plain_page(CONTENT))
}

/// **彩色面板上的彩页只出一张**：没有候选、没有画质分、没有参照，交出来的数据说得出
/// 它走的是彩色分支（ADR 0005 决定第 4 条；样张 spec《Implementation Decisions》第七条）。
///
/// 那一张的名字照别的样张的规矩起：页那一截是 `run` 的成员名，接上它是哪一张——这里接的是
/// 词条名《彩色分支》（参照那一张接的是《参照》）。
#[test]
fn a_color_page_on_a_color_panel_gets_one_sheet_and_no_candidates() {
    let staged = Staged::of(&color_plain_page());
    let request = Request {
        profile: fixtures::profile(COLOR_DEVICE),
        ..staged.request.clone()
    };

    let proof = tonefit::write_proof(&staged.source, &request, &staged.sheets()).expect("出样张");
    let page = only_page(&proof);

    assert_eq!(
        page.page.color(),
        Some(PageColor::Color),
        "夹具的前提：这是一张彩页"
    );
    assert!(
        matches!(page.page.branch(), Some(PageBranch::Color)),
        "交出来的数据说不出它走的是彩色分支：{:?}",
        page.page.branch()
    );
    assert_eq!(page.page.verdict(), None, "彩色分支上没有判定");
    assert!(page.page.scores().is_empty(), "彩色分支上没有画质分");
    assert_eq!(page.scored().count(), 0, "彩色分支上没有候选");
    let Sheets::Color(sheet) = &page.sheets else {
        panic!("彩色分支那一叠该只有一张：{:?}", page.sheets);
    };
    assert_eq!(
        page.page.output, sheet.file,
        "那一张就是 `run` 会写出的那一张"
    );
    assert_eq!(
        fixtures::directory_members(&staged.sheets()),
        ["001.彩色分支.png"],
        "去处里只该有那一张"
    );
    assert_eq!(
        fs::metadata(&sheet.file).expect("那一张在盘上").len(),
        sheet.bytes,
        "交出来的字节数就是盘上那一张的大小"
    );
    assert_ne!(
        fixtures::read_color_png(&sheet.file).color_type,
        png::ColorType::Grayscale,
        "彩色分支上那一张留着颜色"
    );
}

/// **神谕在彩页上跑一遍**：彩色面板上那唯一一张与 `run --no-metadata` 写出的那一张**逐字节相同**
/// （样张 spec《Testing Decisions》第一条）。
///
/// 前提照普通页那一条的规矩先问：白边真的裁掉了、内容真的被缩放过，那一张真的留着颜色——
/// 任何一条不成立，下面那个等号就是在一张「什么都没发生」的页上成立的。
#[test]
fn the_color_sheet_is_byte_for_byte_what_run_writes_without_metadata() {
    let staged = Staged::of(&color_plain_page());
    let request = Request {
        profile: fixtures::profile(COLOR_DEVICE),
        ..staged.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&staged.source, &request, &staged.sheets()).expect("出样张");

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert!(
        matches!(ran.branch(), Some(PageBranch::Color)),
        "夹具的前提：转换那一趟走的是彩色分支"
    );
    let page = only_page(&proof);
    let crop = page.page.crop().expect("处理成了的页有裁白边那一格");
    assert!(crop.trimmed(), "夹具的前提：白边真的裁掉了");
    assert_ne!(page.page.size, crop.after(), "夹具的前提：内容真的被缩放过");
    assert_ne!(
        fixtures::read_color_png(&ran.output).color_type,
        png::ColorType::Grayscale,
        "夹具的前提：转换那一趟写出的那一张留着颜色"
    );
    assert!(
        page.page.output.starts_with(staged.sheets()),
        "那一张指着样张的去处，不是转换那一趟的输出"
    );
    let written = fs::read(&ran.output).expect("读转换那一趟写出的那一张");
    let proofed = fs::read(&page.page.output).expect("读样张里彩色分支那一张");
    assert!(
        written == proofed,
        "彩色分支那一张与转换那一趟写出的不是同一串字节（样张 {} 字节，转换 {} 字节）",
        proofed.len(),
        written.len()
    );
}

/// **黑白面板上同一张彩页转灰，出整叠**，与灰度路径上的普通页同形：每一个候选一张、参照一张，
/// 名字一张不差（ADR 0005 决定第 4 条；样张 spec 的 story 23）。
///
/// 走哪条分支由**面板与页**共同决定：同一张彩页换一块黑白面板，就不再走彩色分支——
/// 它仍然是一张彩页（交出来的数据说得出），只是转了灰。判定那一张也照神谕比一次字节。
#[test]
fn the_same_color_page_on_a_monochrome_panel_gets_a_full_gray_proof_page() {
    let staged = Staged::of(&color_plain_page());
    assert!(
        !staged.request.profile.panel().color,
        "夹具的前提：[`Staged`] 点名的是一块黑白面板"
    );

    let report = tonefit::run(&staged.request).expect("转换那一趟");
    let proof = staged.proof();
    let page = only_page(&proof);

    assert_eq!(
        page.page.color(),
        Some(PageColor::Color),
        "转了灰的彩页仍然说得出自己是彩页"
    );
    let gate = page.page.gate().expect("转灰之后走灰度路径，有门");
    let proofed: Vec<Candidate> = page.scored().map(|(scored, _)| scored.candidate).collect();
    assert_eq!(
        proofed,
        Candidate::all(staged.request.profile.panel().gray_levels, gate),
        "交出来的候选集不是这一页的门派得出的那一整套"
    );
    assert!(
        matches!(page.sheets, Sheets::Gray { .. }),
        "黑白面板上那一叠该有参照"
    );

    // 同形：与一张普通页那一叠的名字一张不差。
    let plain = Staged::plain();
    plain.proof();
    assert_eq!(
        fixtures::directory_members(&staged.sheets()),
        fixtures::directory_members(&plain.sheets()),
        "转灰的彩页那一叠与普通页那一叠不同形"
    );

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert_eq!(
        page.page.verdict(),
        ran.verdict(),
        "样张与转换那一趟定下的不是同一档、同一个理由"
    );
    assert!(
        fs::read(&ran.output).expect("读转换那一趟写出的那一张")
            == fs::read(&page.page.output).expect("读样张里判定那一档的那一张"),
        "判定那一张与转换那一趟写出的不是同一串字节"
    );
}

/// **尺寸未贴合屏幕的页：候选里没有抖动那一维，交出来的数据说得出门不成立**；
/// 那一套就是 `run` 在同一页上用的那一套（神谕；ADR 0007 决定第 2 条）。
///
/// 页比面板小、fit-inside 不放大（[`SMALL`]）：目标尺寸哪条边都贴不住面板。
/// 前提先问两件：门成立时那一套比这一套多（门真的拿走了东西），转换那一趟在这一页上也是门不成立。
/// 判定那一张照神谕比一次字节。
#[test]
fn a_page_the_geometry_gate_shuts_gets_no_dithered_sheets_and_the_candidates_run_uses() {
    let small = Staged::plain_of(SMALL);
    let request = Request {
        fit: tonefit::FitMode::Inside,
        ..small.request.clone()
    };

    let report = tonefit::run(&request).expect("转换那一趟");
    let proof = tonefit::write_proof(&small.source, &request, &small.sheets()).expect("出样张");

    let [ran] = report.volumes[0].pages.as_slice() else {
        panic!("一页的卷，报告里该有一页");
    };
    assert_eq!(
        ran.gate(),
        Some(tonefit::GeometryGate::Broken),
        "夹具的前提：转换那一趟在这一页上门不成立"
    );
    let page = only_page(&proof);
    assert_eq!(
        page.page.gate(),
        Some(tonefit::GeometryGate::Broken),
        "交出来的数据说不出门不成立"
    );
    let proofed: Vec<Candidate> = page.scored().map(|(scored, _)| scored.candidate).collect();
    assert!(
        proofed
            .iter()
            .all(|candidate| candidate.dither == Dither::Off),
        "门不成立的页候选里有抖动那一维：{proofed:?}"
    );
    assert!(
        proofed.len()
            < Candidate::all(
                request.profile.panel().gray_levels,
                tonefit::GeometryGate::Holds
            )
            .len(),
        "夹具的前提：门成立时那一套比这一套多"
    );
    let used: Vec<Candidate> = ran.scores().iter().map(|scored| scored.candidate).collect();
    assert_eq!(
        proofed, used,
        "样张的候选集不是转换那一趟在同一页上用的那一套"
    );
    assert_eq!(
        fixtures::directory_members(&small.sheets()).len(),
        proofed.len() + 1,
        "去处里该是这一套候选各一张、外加参照一张"
    );
    assert!(
        fs::read(&ran.output).expect("读转换那一趟写出的那一张")
            == fs::read(&page.page.output).expect("读样张里判定那一档的那一张"),
        "判定那一张与转换那一趟写出的不是同一串字节"
    );
}

/// **点名一档这块面板写不出的灰阶档位，彩色分支上的页也照转换那一趟拒绝**，去处里一张都没有。
///
/// 转换那一趟碰卷之前就说这一句——一卷里只有彩页也一样，那一趟里它是这一卷的一页。
/// 样张要是只在灰度路径上问覆盖项，一张彩页就会在转换那一趟拒绝的请求上照出一张。
#[test]
fn an_override_the_panel_cannot_write_is_refused_on_a_color_page_as_run_refuses_it() {
    let staged = Staged::of(&color_plain_page());
    let request = Request {
        profile: fixtures::profile(COLOR_DEVICE),
        bit_depth: Some(BitDepth::Eight),
        ..staged.request.clone()
    };
    assert!(
        BitDepth::Eight.levels() > request.profile.panel().gray_levels,
        "夹具的前提：这块面板写不出 8bit"
    );

    let ran = tonefit::run(&request).expect_err("转换那一趟该拒绝");
    let proofed = tonefit::write_proof(&staged.source, &request, &staged.sheets())
        .expect_err("样张该照转换那一趟拒绝");

    assert_eq!(
        format!("{proofed:#}"),
        format!("{ran:#}"),
        "样张与转换那一趟说的不是同一句"
    );
    assert!(
        !staged.sheets().exists(),
        "拒绝了还在去处里留了东西：{:?}",
        fixtures::directory_members(&staged.sheets())
    );
}

// ---- 说不出话的那几种（`proof-sheet/06`） ----

/// **点成一个目录或一个归档，当场一句话说样张只认一张图**（spec 的 story 25），去处一个都不建。
///
/// 五个都是**真卷**：目录里躺着一页，四种归档里各装着一页——转换那一趟拿它们当卷照做，
/// 样张不认的是「一卷」这个形状，不是它们打不开。归档认哪几个扩展名与转换那一趟同一把尺子
/// （`tonefit::is_archive`）。句子里点得出是哪一个、是目录还是归档，用户才知道该怎么改。
#[test]
fn a_directory_or_an_archive_is_refused_with_one_sentence_that_a_proof_takes_one_image() {
    let staged = Staged::plain();
    let page = plain_page(fixtures::TINY);
    let directory = staged
        .source
        .parent()
        .expect("那一页躺在卷里")
        .to_path_buf();
    let cbz = staged.space.cbz("合集").page("001.png", &page).write();
    let zip = staged
        .space
        .archive("合集.zip")
        .page("001.png", &page)
        .write();
    let rar = staged.space.rar("合集", fixtures::rar::STORED);
    let sevenz = staged.space.sevenz("合集").page("001.png", &page).write();

    for (named, kind) in [
        (&directory, "目录"),
        (&cbz, "归档"),
        (&zip, "归档"),
        (&rar, "归档"),
        (&sevenz, "归档"),
    ] {
        let said = format!(
            "{:#}",
            tonefit::write_proof(named, &staged.request, &staged.sheets())
                .expect_err("样张该拒绝不是一张图的东西")
        );

        assert!(
            said.contains("样张只认一张图"),
            "没说样张只认一张图：{said}"
        );
        assert!(
            said.contains(&named.display().to_string()),
            "没指出点的是哪一个：{said}"
        );
        assert!(said.contains(kind), "没说它是{kind}：{said}");
        assert!(
            !staged.sheets().exists(),
            "拒绝了还建出了去处：{:?}",
            fixtures::directory_members(&staged.sheets())
        );
    }
}

/// **点了一个不存在的路径，说的是它不在——不是「样张只认一张图」**。
///
/// 那一问只看路径的形状（目录、归档扩展名、页扩展名），对一个根本不在的路径它会说错话：
/// 敲错了的 `卷1` 被说成透传文件、不在的 `合集.cbz` 被说成归档，用户照着那句去改只会越改越远。
/// 这条反着钉那一句不许再出现；不在的那一种交给读盘那一步说，它说得出是哪个路径、为什么读不到。
#[test]
fn a_path_that_does_not_exist_is_not_called_something_other_than_an_image() {
    let staged = Staged::plain();
    for missing in [
        staged.space.dir("卷1"),
        staged.space.dir("合集.cbz"),
        staged.space.dir("002.png"),
    ] {
        let said = format!(
            "{:#}",
            tonefit::write_proof(&missing, &staged.request, &staged.sheets())
                .expect_err("不存在的路径出不了样张")
        );

        assert!(
            !said.contains("样张只认一张图"),
            "把一个不存在的路径说成了别的东西：{said}"
        );
        assert!(
            said.contains(&missing.display().to_string()),
            "没指出是哪个路径：{said}"
        );
        assert!(
            !staged.sheets().exists(),
            "出不了还建出了去处：{:?}",
            fixtures::directory_members(&staged.sheets())
        );
    }
}

/// **透传文件，样张也不认**：它在卷里是转换那一趟原样拷过去的成员，一张都不会被编出去，
/// 样张出一叠就是在回答一个转换那一趟从来不问的问题。
///
/// 夹具挑的是最刁的那一种：**字节是一张好好的 PNG**，扩展名却不是页（`002.dat`）——
/// 解码器解得开它，拦它的只能是「转换那一趟认不认它是一页」那一问。
/// 神谕那一侧先问实：`run` 把它原样搬了过去。
#[test]
fn a_file_run_passes_through_is_refused_with_the_sentence_that_a_proof_takes_one_image() {
    let staged = Staged::placing(|volume| {
        volume.page("001.png", &plain_page(fixtures::TINY));
        volume.file(
            "002.dat",
            &fixtures::encode_image(&plain_page(fixtures::TINY), "png"),
        )
    });

    let ran = tonefit::run(&staged.request).expect("转换那一趟照做");
    let [volume] = ran.volumes.as_slice() else {
        panic!("该是一卷");
    };
    assert_eq!(
        fs::read(
            volume
                .output
                .join(staged.source.file_name().expect("透传文件有名字"))
        )
        .expect("读转换那一趟搬过去的那一份"),
        fs::read(&staged.source).expect("读源"),
        "夹具的前提：转换那一趟把它原样透传"
    );

    let said = format!(
        "{:#}",
        tonefit::write_proof(&staged.source, &staged.request, &staged.sheets())
            .expect_err("样张该拒绝透传文件")
    );
    assert!(
        said.contains("样张只认一张图"),
        "没说样张只认一张图：{said}"
    );
    assert!(
        said.contains(&staged.source.display().to_string()),
        "没指出点的是哪一个：{said}"
    );
    assert!(said.contains("透传"), "没说转换那一趟拿它怎么办：{said}");
    assert!(
        !staged.sheets().exists(),
        "拒绝了还建出了去处：{:?}",
        fixtures::directory_members(&staged.sheets())
    );
}

/// **点了一张解不开的图，当场一句话说它解不开，去处里一个文件都没有**（spec 的 story 24）——
/// 去处本来不在的话，连一个空目录都不留：用户不必去翻一个空目录猜。
///
/// 「解不开」照转换那一趟认：就是那一趟的**坏页**（`CONTEXT.md` 的《失败》）。神谕那一侧先问实：
/// 三种夹具在 `run` 里都是坏页——尺寸都解不出来的、尺寸解得出来而缓冲分配不下的、
/// 救回却一个像素都没解出来的。**残缺页不在里面**：它救回到了像素，那一趟照常写出，
/// 样张也照出（界画宽了，残缺页就被说成解不开）。
#[test]
fn an_image_that_cannot_be_decoded_is_refused_in_one_sentence_and_leaves_nothing_behind() {
    for (what, bytes) in [
        ("不是图的字节", b"not a page".to_vec()),
        ("缓冲分配不下", fixtures::oversized_page()),
        (
            "一个像素都救不回",
            fixtures::salvages_nothing_page(fixtures::TINY),
        ),
    ] {
        let staged = Staged::placing(|volume| volume.file("001.png", &bytes));
        let ran = tonefit::run(&staged.request).expect("转换那一趟照做");
        assert!(
            ran.volumes[0].pages[0].failure().is_some(),
            "夹具的前提：{what} 在转换那一趟是坏页"
        );

        let said = format!(
            "{:#}",
            tonefit::write_proof(&staged.source, &staged.request, &staged.sheets())
                .expect_err("样张该拒绝一张解不开的图")
        );

        assert!(said.contains("解不开"), "{what}：没说它解不开：{said}");
        assert!(
            said.contains(&staged.source.display().to_string()),
            "{what}：没指出是哪一张：{said}"
        );
        assert!(
            !staged.sheets().exists(),
            "{what}：解不开还建出了去处：{:?}",
            fixtures::directory_members(&staged.sheets())
        );
    }

    let salvaged = Staged::placing(|volume| {
        volume.file("001.png", &fixtures::truncated(&plain_page(fixtures::TINY)))
    });
    let page = salvaged.proof();
    assert!(
        only_page(&page).page.salvage().is_some(),
        "夹具的前提：截断的那一张是残缺页"
    );
    assert!(salvaged.sheets().exists(), "残缺页照出样张，不是解不开");
}

/// **覆盖项越界又点了一张解不开的图，样张先说越界——与转换那一趟同一个次序、同一句**
/// （停车场 Q1042）。
///
/// 转换那一趟碰卷之前就问覆盖项，那一问排在读任何一页之前；样张先解码的话，
/// 同一份两处都错的请求上，转换那一趟说越界、样张说解不开。
#[test]
fn an_override_the_panel_cannot_write_is_refused_before_the_image_is_decoded_as_run_refuses_it() {
    let staged = Staged::placing(|volume| volume.file("001.png", b"not a page"));
    let request = Request {
        bit_depth: Some(BitDepth::Eight),
        ..staged.request.clone()
    };
    assert!(
        BitDepth::Eight.levels() > request.profile.panel().gray_levels,
        "夹具的前提：这块面板写不出 8bit"
    );

    let ran = tonefit::run(&request).expect_err("转换那一趟该拒绝");
    let proofed = tonefit::write_proof(&staged.source, &request, &staged.sheets())
        .expect_err("样张该照转换那一趟拒绝");

    assert_eq!(
        format!("{proofed:#}"),
        format!("{ran:#}"),
        "样张与转换那一趟说的不是同一句"
    );
    assert!(
        !staged.sheets().exists(),
        "拒绝了还建出了去处：{:?}",
        fixtures::directory_members(&staged.sheets())
    );
}

/// **去处写不进去时回 `Err`，说得出是写不出去、卡在哪个路径上**——不恐慌，调用方接得住
/// （与灰阶测试图那一路同一条：`write_calibration_chart`）。
///
/// 两种各造一个，都不靠哪一个平台才有的权限语义：
///
/// - **去处建不出来**：拿一个文件挡在它的父目录上；
/// - **去处建得出、一张写不进**（盘满那一类）：去处先在，参照那一张的名字上先摆一个目录——
///   落盘时头一张就撞上它。
#[test]
fn a_destination_that_cannot_take_the_sheets_comes_back_as_an_error_that_says_so() {
    let staged = Staged::plain_of(SMALL);
    // 比面板小的页配 fit-inside：一步都不放大，门不成立，三个候选——这两条只问落盘那一步，编得快些。
    let request = Request {
        fit: tonefit::FitMode::Inside,
        ..staged.request.clone()
    };

    let blocker = staged.space.stray_file("挡路的文件", b"");
    let unmakeable = blocker.join("样张");
    let occupied = staged.sheets();
    // 参照那一张的名字：这一页在 `run` 那一侧的成员名（`001.png`）接上词条名。
    let taken = occupied.join(
        staged
            .source
            .with_extension("参照.png")
            .file_name()
            .expect("有名字"),
    );
    fs::create_dir_all(&taken).expect("在参照那一张的名字上摆一个目录");

    for (out, stuck) in [(&unmakeable, &unmakeable), (&occupied, &taken)] {
        let said = format!(
            "{:#}",
            tonefit::write_proof(&staged.source, &request, out).expect_err("写不进去该回 Err")
        );

        assert!(said.contains("写不出去"), "没说是写不出去：{said}");
        assert!(
            said.contains(&stuck.display().to_string()),
            "没说卡在哪儿（{}）：{said}",
            stuck.display()
        );
    }
}
