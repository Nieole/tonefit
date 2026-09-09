//! 四条真机判读结论立成的护栏：判据合不合**人眼**。
//!
//! 与 `tests/metric.rs` 是**两类东西**。那一篇验的是「代码做了它该做的」，出处是 ADR 0002；
//! 这一篇验的是「代码合不合人眼」，出处是 `.scratch/metric-recalibration/judgements/`
//! 底下那几轮真机判读。判据的下一次改动撞到这四条会**当场红**，
//! 而不是要再上一次机才发现。
//!
//! **只认真机那几轮。**`judgements/README.md` 写着口径：第四、五轮是模型判读局部放大块，
//! **不作标定依据**；第六轮起是人在 Kobo Libra 2 上整页 1:1 看，那几轮才作数。
//! 这四条一条都不引模型那两轮，每一条各指回它那一轮的逐对数据。
//!
//! **写性质，不写读数。**四条断的都是「这两样东西之间的关系」——谁更干净、撒不撒点、
//! 三格结论同不同——一条都不断具体读数。换面板即换低通核、换夹具即换页，
//! 读数会动而关系不该动（ADR 0002：判据数值不可跨面板比较）。
//!
//! **前三条用合成夹具，第四条走 opt-in 真实素材。**真实素材不入库（spec 的
//! 《Testing Decisions》），第四条那四页是真实版面，合成造不出来。
//!
//! **自带 harness**（`Cargo.toml` 里 `[[test]] harness = false`）：理由与 `tests/smoke.rs`
//! 是同一条，写在那一篇的模块文档里，这里不复述。这一篇要它只为第四条——**跳过要说得出口**。
//!
//! ```text
//! cargo test --test perceptual                            # 前三条，第四条印一行跳过
//! TONEFIT_SAMPLES=<素材目录> cargo test --test perceptual   # 四条全跑
//! ```
//!
//! 素材指过来了却指错地方**不是跳过**：那是点名要跑，当场红（同 `tests/smoke.rs`）。

mod fixtures;

use std::path::{Path, PathBuf};

use tonefit::{BitDepth, Candidate, GrayImage, Reference, Score, Size, quantize, score};

/// 一条用例：它的名字，加它自己。名字给命令行的过滤词比对，也给落款用。
type Case = (&'static str, fn() -> Outcome);

/// 这一篇的四条，按票面的次序。
const CASES: &[Case] = &[
    (
        "near_white_flat_tones_never_favour_dithering",
        near_white_flat_tones_never_favour_dithering,
    ),
    (
        "on_grid_paper_white_takes_no_dither_dots",
        on_grid_paper_white_takes_no_dither_dots,
    ),
    (
        "the_metric_answers_the_same_at_every_background_brightness",
        the_metric_answers_the_same_at_every_background_brightness,
    ),
    (
        "off_grid_by_one_real_pages_still_favour_four_bit_plain",
        off_grid_by_one_real_pages_still_favour_four_bit_plain,
    ),
];

/// 一条用例的结局。红了那一路走 panic——与 `tests/smoke.rs` 同一条。
enum Outcome {
    Ran(String),
    Skipped(String),
}

/// 三种结局各印一行，头两个字就分得开：跑过了、跳过了，红了那一路走 panic。
///
/// **被过滤词滤掉的也印一行。**自带 harness 的整个理由就是「本次跑没跑一眼看得出」，
/// 而静悄悄跳过的那几条与从来不存在的那几条，在日志上长得一模一样。
fn main() {
    let (mut ran, mut skipped) = (0usize, 0usize);
    for &(name, case) in CASES {
        if !selected(name) {
            skipped += 1;
            println!("跳过 {name}：命令行的过滤词点的是别的用例。");
            continue;
        }
        match case() {
            Outcome::Ran(note) => {
                ran += 1;
                println!("跑过 {name}：{note}");
            }
            Outcome::Skipped(note) => {
                skipped += 1;
                println!("跳过 {name}：{note}");
            }
        }
    }
    println!("真机护栏：跑过 {ran} 条，跳过 {skipped} 条。");
}

/// 命令行点的是这一条吗。
///
/// 自带 harness 就得自己认这件事：`cargo test <过滤词>` 把过滤词发给**每一个**测试二进制，
/// 内建 harness 拿它对测试名做子串匹配，这里照同一条规矩办。**拿不准就跑**——
/// 理由与写法同 `tests/smoke.rs` 的 `selected`，那一篇写着为什么多跑一趟只是慢、
/// 跳错一趟是假的证据。
fn selected(name: &str) -> bool {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument.starts_with('-')) {
        return true;
    }
    arguments.is_empty() || arguments.iter().any(|filter| name.contains(filter))
}

/// 前三条用的页尺寸。判据只吃像素与面板 PPI，不要求尺寸恰好是目标尺寸；
/// 取得比面板小是为了让这一组跑得快（同 `tests/metric.rs`）。
///
/// 缩小不影响这三条：一、三条的页是平坦调，每一块的读数本来就一样；第二条数的是
/// 白底上有几个像素被撒了点，那是**逐像素**的事，与分块聚合无关。
/// 第四条不走它——真实页有多大就是多大。
const PAGE: Size = Size::new(640, 832);

/// 第七轮 L 组那十五格：《离格量》u × 三种背景亮度，逐格的目标灰度。
///
/// 逐字取自 `judgements/第七轮真机包04.json` 的《逐格》，三列依次是
/// [`BRIGHTNESS`] 的近白 / 中灰 / 偏暗。原材料是武器原档 MHZ01_090 的一块 512×512
/// 无文字白底**只动直流**平移出来的；这里用纯色合成同一个形状——
/// 那一块的块内起伏只有 5 级，平坦到判据在它身上读的就是这道阶梯本身。
const LADDER: [(u32, [u8; 3]); 5] = [
    (1, [254, 171, 86]),
    (2, [253, 172, 87]),
    (8, [247, 178, 93]),
    (21, [234, 191, 106]),
    (42, [213, 128, 43]),
];

/// [`LADDER`] 三列各是什么背景亮度，次序与那三个灰度一一对应。
const BRIGHTNESS: [&str; 3] = ["近白", "中灰", "偏暗"];

/// 近白是 [`LADDER`] 每一格的第几列。第一条只看这一列；
/// 第三条拿它当「与另外两列比对的那一列」。
const NEAR_WHITE: usize = 0;

/// 近白那一列上，今天的判据仍与真机同向的《离格量》。
///
/// **真机在整列五格上 5/5 判「2bit 不抖更干净」，无一例外**——第五格（u = 42）也在内，
/// 而今天的判据在那一格反着说 FS 更好。那是本票量出来**唯一不绿的一格**：读数、
/// 判据与真机各说了什么，只在票 `grain-floor-absolute/02` 的《落地记录》二写一处；
/// 停车场 **Q447** 记的是「为什么收窄，而不是留一条今天就红的用例」。
///
/// **收窄不等于事实变小。**u = 42 的白底在真机上仍是不抖更干净；这一行只说
/// 「护栏今天守得住的是前四格」。那一格并非没人看着：
/// [`the_metric_answers_the_same_at_every_background_brightness`] 在另一个维度上照旧扫到它。
const NEAR_WHITE_AGREEING: [u32; 4] = [1, 2, 8, 21];

/// **第一条：近白平坦调上 FS 不占优。**
///
/// 出处：第七轮 L 组**近白那一列 5/5 判「2bit 不抖更干净」**
/// （`judgements/第七轮真机包04.json`，对 01/02/03/06/09；第八轮的四格锚 4/4 复现，
/// 其中就有 `u01_近白`）。
///
/// 性质：平坦调的背景够亮时，`2bit+FS` **永远不比** `2bit 不抖` 干净。
/// 断的是这两个候选之间的关系，不是任何一个读数。
///
/// 守的是什么：颗粒项的可见度地板一旦抬到吞掉 2bit 上的 FS 颗粒，
/// 这一列会整列翻向 FS——而真机说那是错的。地板改动撞上来会在这里红。
///
/// 今天守得住的范围见 [`NEAR_WHITE_AGREEING`]。
fn near_white_flat_tones_never_favour_dithering() -> Outcome {
    let mut checked = 0;
    for (offset, levels) in LADDER {
        if !NEAR_WHITE_AGREEING.contains(&offset) {
            continue;
        }
        let level = levels[NEAR_WHITE];
        let reference = flat_reference(level);
        let dithered = reading(&reference, fixtures::dithered(BitDepth::Two));
        let plain = reading(&reference, fixtures::plain(BitDepth::Two));
        println!("  近白 u={offset:<2} 灰度 {level}：2bit 不抖 {plain} · 2bit+FS {dithered}");
        assert!(
            dithered >= plain,
            "近白 u={offset}（灰度 {level}）上 2bit+FS 读成 {dithered}，比 2bit 不抖的 {plain} 还干净：\
             真机在这一列 5/5 判不抖更干净（第七轮 L 组）"
        );
        checked += 1;
    }
    Outcome::Ran(format!(
        "近白平坦调 {checked} 格，2bit+FS 一格都没赢过 2bit 不抖"
    ))
}

/// 白底那一片占页高的几分之几。剩下的是底下那条灰调带。
const WHITE_FIELD_SHARE: u32 = 4;

/// 底下那条灰调带从纸白往下压多少级。压到 2bit 的格点之间，FS 在那里非抖不可。
const BAND_DEPTH: u8 = 200;

/// **第二条：离格 0 的页上 FS 够用。**
///
/// 出处：第六轮 C 组八页（`judgements/第六轮真机四组.json`）——那八页**纸白全是 255**，
/// 逐页的《到2bit格点》是 0、《FS在白底上要撒的点》是 0.0，真机上 6 页分不出、2 页偏
/// 4bit 不抖；同一轮 B 组那四页纸白 253、离格 2，白底上撒点，真机 4/4 判 4bit 不抖。
/// **分界线就在纸白落不落在格点上这一列**，`CONTEXT.md` 的《离格量》词条写的就是它。
///
/// 性质：纸白正落在格点上时，FS 在白底上**一个点都不撒**；同一页把纸白平移到离格 2，
/// 同一档的 FS 在同一片白底上就撒得出点来。断的是「撒没撒点」，不是撒了多少。
///
/// 页是白底在上、灰调带在下：FS 的误差只往右和往下传（`src/quantize.rs`），
/// 白底那一片因此收不到内容那一块漏过来的误差，「一个点都不撒」问得干净。
/// 带子不能省——没有它整页都落在格点上，量化成了恒等映射，这一条就什么都没验到。
fn on_grid_paper_white_takes_no_dither_dots() -> Outcome {
    let field_pixels = white_field_rows(PAGE) as usize * PAGE.width as usize;
    let dots = |paper_white: u8, candidate: Candidate| {
        let page = page_with_a_white_field_above_a_tone_band(PAGE, paper_white);
        quantize(&page, candidate).pixels()[..field_pixels]
            .iter()
            .filter(|&&level| level != 255)
            .count()
    };

    let on_grid = dots(255, fixtures::dithered(BitDepth::Two));
    assert_eq!(
        on_grid, 0,
        "纸白 255 正落在 2bit 格点上，白底那 {field_pixels} 个像素里仍有 {on_grid} 个被 FS 撒了点：\
         第六轮 C 组那八页的前提就是白底上一个点都不撒"
    );

    // 离格 2 那一侧：撒点的是 FS，不是量化本身——同一片白底上不抖动照旧一个点都不撒。
    let off_grid_plain = dots(253, fixtures::plain(BitDepth::Two));
    assert_eq!(
        off_grid_plain, 0,
        "纸白 253 上 2bit 不抖在白底撒了 {off_grid_plain} 个点：那一档的就近取整该把整片白底归到 255"
    );
    let off_grid = dots(253, fixtures::dithered(BitDepth::Two));
    assert!(
        off_grid > 0,
        "纸白 253 离格 2，白底那 {field_pixels} 个像素上 FS 一个点都没撒：\
         第六轮 B 组那四页判得差正是因为它撒了（夹具不对，这一条没在验它要验的东西）"
    );

    Outcome::Ran(format!(
        "离格 0 的白底上 FS 撒 0 个点，同一片白底离格 2 时撒 {off_grid} 个"
    ))
}

/// **第三条：亮度分歧——这一条是缺陷记录，不是希望的行为。**
///
/// 出处：第七轮 L 组十五格（`judgements/第七轮真机包04.json`），连同**第八轮复判**
/// （`judgements/第八轮复判04R.json`）对其中两格的订正。
///
/// 断言：同一个《离格量》的**三种背景亮度**上，判据给出的**结论相同**——
/// 三格要么都说不抖更好，要么都说 FS 更好。
///
/// **真机在其中两行上给的答案相反。**第七轮当场读出来是三行（u = 2 / 21 / 42），
/// 第八轮把可疑的那两格各判两次、左右顺序相反：`u02_偏暗` 两次一致，**真值是 2bit 不抖，
/// 第七轮那个 FS 是噪声**——u = 2 那一行因此不再分歧；`u42_中灰` 两次相反，
/// 落在判读边界上、记成「平」，而同一行的近白判不抖、偏暗判 FS，那一行照旧分歧。
/// **今天站得住的是两行：u = 21 与 u = 42。**（第八轮的四格锚 4/4 与第七轮一致，
/// 那一趟判读是稳的。）
///
/// 判据对背景亮度是**盲的**——同一个 u 的三格读数逐位几乎相同——因此这一条钉住的
/// 不是「判据判对了」，是**判据缺哪一项**。
///
/// **将来加了亮度维之后它该红。红了就把它换成新的断言，那不是回归。**
/// 换的时候要连这段文档一起换：留着一条写着「结论必须相同」的用例，
/// 下一个人会把它当成要守的行为去守，而它从来不是。
///
/// 断的是「三格结论相同」，**不是**「三格读数极差小于某值」：不依赖任何具体读数，
/// 换面板、换夹具都不会假红。
fn the_metric_answers_the_same_at_every_background_brightness() -> Outcome {
    for (offset, levels) in LADDER {
        let verdicts: Vec<bool> = levels
            .iter()
            .zip(BRIGHTNESS)
            .map(|(&level, brightness)| {
                let reference = flat_reference(level);
                let dithered = reading(&reference, fixtures::dithered(BitDepth::Two));
                let plain = reading(&reference, fixtures::plain(BitDepth::Two));
                let favours_dithering = dithered < plain;
                println!(
                    "  u={offset:<2} {brightness} 灰度 {level:<3}：\
                     2bit 不抖 {plain} · 2bit+FS {dithered} → {}",
                    if favours_dithering {
                        "FS 更好"
                    } else {
                        "不抖更好"
                    }
                );
                favours_dithering
            })
            .collect();
        let first = verdicts[NEAR_WHITE];
        assert!(
            verdicts.iter().all(|&favours| favours == first),
            "u={offset} 的三种背景亮度上判据给了不同的结论：{:?}。\
             判据今天对亮度是盲的，这一条正是钉着那件事——**它红了多半是好事**：\
             判据认得出亮度了，那就把这一条换成新的断言（见本条文档）",
            verdicts
                .iter()
                .zip(BRIGHTNESS)
                .map(|(&favours, brightness)| format!(
                    "{brightness}:{}",
                    if favours { "FS" } else { "不抖" }
                ))
                .collect::<Vec<_>>()
        );
    }
    Outcome::Ran(format!(
        "{} 个《离格量》各三种背景亮度，判据逐个 u 结论相同（缺陷记录：真机在两行上分歧）",
        LADDER.len()
    ))
}

/// 指向本机素材目录的环境变量，与 `tests/smoke.rs` 同一个。没设就跳过。
const SAMPLES: &str = "TONEFIT_SAMPLES";

/// 第七轮 M 组那四页的**参照 8bit**在素材目录里的位置。
///
/// 是参照那一侧、未经目标位深量化的图，判据要的正是它；怎么渲出来的见
/// `.scratch/metric-recalibration/calibration/怎么重跑.md`。
const OFF_GRID_BY_ONE_DIRECTORY: &str = "_实验-判据重标定/真机包04/参照8bit/离格量1参照";

/// M 组逐对那四页，页名取自 `judgements/第七轮真机包04.json`。
const OFF_GRID_BY_ONE_PAGES: [&str; 4] = ["014.png", "016.png", "018.png", "024.png"];

/// **第四条：离格 1 的真实页上 `4bit 不抖` 更干净。**
///
/// 出处：第七轮 M 组 **4/4 判 4bit 不抖更干净**（`judgements/第七轮真机包04.json`），
/// 材料是 N和S 第 23 话的一手 B 类发布版，纸白 254、《离格量》1、全 24 页皆同。
/// 判读者备注「这组差距很小」——因此断的是方向，不是幅度。
///
/// **这一条合成不出来**：它要的是真实版面上白底、网点与文字的那个混合，
/// 而离格 1 时 FS 在白底上只撒 1.2% 的点，合成的平坦白底读不出那点差别。
/// 于是走 opt-in 真实素材，照 `tests/smoke.rs` 那一套（口径见模块文档）。
///
/// 性质：那四页上 `4bit 不抖` 的读数**低于** `2bit+FS`。
fn off_grid_by_one_real_pages_still_favour_four_bit_plain() -> Outcome {
    let Some(root) = samples() else {
        return Outcome::Skipped(format!(
            "{SAMPLES} 没有指向任何目录，第七轮 M 组那四页这一趟一页都没验。\n\
             要跑就 `{SAMPLES}=<素材目录> cargo test --test perceptual`；\
             那四页在素材目录下的 {OFF_GRID_BY_ONE_DIRECTORY}/。"
        ));
    };
    let directory = root.join(OFF_GRID_BY_ONE_DIRECTORY);
    assert!(
        directory.is_dir(),
        "{SAMPLES} 指向 {}，而 {OFF_GRID_BY_ONE_DIRECTORY}/ 不在它底下：\
         点名要跑真实素材却指错了地方，静悄悄通过等于骗人",
        root.display()
    );

    for name in OFF_GRID_BY_ONE_PAGES {
        let page = directory.join(name);
        let reference = baseline_reference(read_gray_page(&page));
        let plain = reading(&reference, fixtures::plain(BitDepth::Four));
        let dithered = reading(&reference, fixtures::dithered(BitDepth::Two));
        println!("  {name}：4bit 不抖 {plain} · 2bit+FS {dithered}");
        assert!(
            plain < dithered,
            "{name} 上 4bit 不抖读成 {plain}，没有比 2bit+FS 的 {dithered} 干净：\
             真机在这四页上 4/4 判 4bit 不抖更干净（第七轮 M 组）"
        );
    }

    Outcome::Ran(format!(
        "{} 下 {} 页离格 1 的真实页，4bit 不抖逐页干净过 2bit+FS",
        directory.display(),
        OFF_GRID_BY_ONE_PAGES.len()
    ))
}

/// 素材目录。环境变量没设、或设成空串就是 `None`；指过来了却不是目录就 panic（见模块文档）。
fn samples() -> Option<PathBuf> {
    let value = std::env::var_os(SAMPLES)?;
    if value.is_empty() {
        return None;
    }
    let root = PathBuf::from(value);
    assert!(
        root.is_dir(),
        "{SAMPLES} 指向 {}，而那不是一个目录",
        root.display()
    );
    Some(root)
}

/// 读一页真实参照图，转成判据吃的 8 位灰度缓冲。
fn read_gray_page(path: &Path) -> GrayImage {
    let image = image::open(path).unwrap_or_else(|error| panic!("读 {}：{error}", path.display()));
    fixtures::gray_image(&image)
}

/// 白底那一片有多少行。两处要它：算白底那一段有多少像素，与铺出这一页——
/// 各写一遍就是走散的开始，而走散了不报错，只让断言少看或多看几行。
const fn white_field_rows(size: Size) -> u32 {
    size.height / WHITE_FIELD_SHARE * (WHITE_FIELD_SHARE - 1)
}

/// 白底在上、灰调带在下的一页：上面 [`white_field_rows`] 那么多行是 `paper_white`，
/// 底下那一条从纸白往下压到 `paper_white − `[`BAND_DEPTH`]。
///
/// 白底放在上边是要紧的：FS 的误差只往右和往下传，白底那一片因此收不到带子漏过来的误差
/// （见 `src/quantize.rs` 的 `floyd_steinberg`），「白底上一个点都不撒」才问得干净。
fn page_with_a_white_field_above_a_tone_band(size: Size, paper_white: u8) -> GrayImage {
    let field = white_field_rows(size);
    let band = (size.height - field).max(1);
    let pixels = (0..size.height)
        .flat_map(|y| {
            let level = if y < field {
                paper_white
            } else {
                paper_white.saturating_sub(((y - field) * u32::from(BAND_DEPTH) / band) as u8)
            };
            std::iter::repeat_n(level, size.width as usize)
        })
        .collect();
    GrayImage::new(size, pixels)
}

/// 一张平坦调页的参照，灰度是 `level`。头一条与第三条的阶梯全走它。
fn flat_reference(level: u8) -> Reference {
    baseline_reference(fixtures::gray_image(&fixtures::solid(PAGE, level)))
}

/// 基准设备的面板上的参照。
///
/// 与 `tests/metric.rs` 的同名助手**只差收什么图**（那边收 `DynamicImage`，这边收
/// `GrayImage`，因为第二条的页是直接铺出来的）。[`reading`] 与 [`PAGE`] 那两项则与那一篇
/// 逐字相同——把三者一并下沉进 `tests/fixtures/` 要连着改那一篇，而那一篇是 `04` 的地界，
/// 记在停车场 **Q450**。
fn baseline_reference(image: GrayImage) -> Reference {
    Reference::new(fixtures::baseline_profile().panel(), image)
}

/// 把 `candidate` 量化出来，量它离参照有多远。位深从候选身上取——
/// 判据要它算颗粒项那道地板，而候选正是量化这张图的那一档（同 `tests/metric.rs`）。
fn reading(reference: &Reference, candidate: Candidate) -> Score {
    score(
        reference,
        &quantize(reference.image(), candidate),
        candidate.bit_depth,
    )
}
