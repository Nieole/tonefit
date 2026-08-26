//! 黄金回归：一组固定夹具的判定与输出体积记进快照，任何变动都要显式接受。
//!
//! 这里一条性质都不主张——它只钉住「今天这批夹具算出来的就是这些数」。存在的理由是
//! 15 号票那一句：防止调优在无人察觉时改变判定结果。判据的低通核、掩蔽加权、分块的
//! 边长与尾巴宽度、上包络的分位、迟滞页数、离群判据的立脚点分位与倍数、编码器在灰度与
//! 调色板之间的取舍，任何一处动一下都会在这里露出来。
//!
//! 非退化的上分位与迟滞升档要页数够多才走得到，因此有三个长卷专门喂它们；
//! 离群那一条不挑卷长，短卷夹具里就走得到（立脚点逐页各取一个，见 `envelope` 的 `outlying`）。
//! 判据自己那一层的分块聚合则要**局部**损伤才走得到，`local-damage` 专喂它。
//! 归档卷单列一个，让写进容器的页字节数也进快照。
//!
//! 与 `tests/metric.rs` 分工相反：那边测的是判据**该有的性质**，数值动了不算错；
//! 这边测的是**数值本身**，动了就要有人当场答一句「为什么」。
//!
//! 快照在 `tests/golden-snapshot.txt`，与本文件一起入版本库。夹具全部由代码生成，
//! 仓库里没有一张真实漫画页（真实素材那一路是 `tests/smoke.rs`）。

mod fixtures;

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use fixtures::{Volume, Workspace};
use tonefit::{
    CacheBudget, Filter, IoMode, Mode, PageBranch, PageColor, PageReport, Request, Size,
    VolumeReport,
};

/// 快照文件，相对仓库根。
const SNAPSHOT: &str = "tests/golden-snapshot.txt";

/// 接受快照变动的环境变量。设了它，这一趟把算出来的写回去，不再比对。
const ACCEPT: &str = "TONEFIT_ACCEPT_GOLDEN";

/// 差异最多印这么多行。整份对不上时（换了台机器、改了版式）印全了只会刷满终端，
/// 而头几行已经够说明问题。
const MAX_DIFF_LINES: usize = 40;

/// 彩色面板设备：彩页只有在它上面才走彩色分支（ADR 0010：彩色分支按面板分流）。
const COLOR_DEVICE: &str = "kobo-libra-colour";

/// 快照文件的抬头。它也进比对——说明与数据同一份出处，改了说法同样要显式接受。
const HEADER: &str = "\
# 黄金回归快照。由 `cargo test --test golden` 算出，夹具全部由代码生成。
#
# **任何一行变了都要显式接受**：
#
#     TONEFIT_ACCEPT_GOLDEN=1 cargo test --test golden
#
# 接受之前先答一句「为什么变」。这份快照存在的全部理由，就是不让判定在无人察觉时改动。
#
# 卷行：`[型号] 卷名 · 几何门 · 卷级判定`。
# 页行：页名 · 目标尺寸 · 判定候选 · 输出字节 · 理由。
# 透传行：非图片文件原样搬过去的那些，名字与字节数。
#
# 字节数**不含自描述元数据**（这一趟按 `--no-metadata` 跑）：tEXt 里写着工具版本，
# 收进来的话改一次版本号整份快照就跟着动，真正的判定变动会淹在里面。
# tEXt 那一批字段各是什么，由 `tests/idempotency.rs` 逐字段钉着。
";

#[test]
fn the_fixed_fixtures_still_decide_the_same_way() {
    let produced = snapshot();
    let path = snapshot_path();
    let committed = fs::read_to_string(&path).ok();

    if accepting() {
        fs::write(&path, &produced).expect("写快照");
        match committed.as_deref().map(normalize) {
            Some(before) if before == produced => eprintln!("快照没有变动：{SNAPSHOT}"),
            Some(_) => eprintln!("已接受快照变动：{SNAPSHOT}"),
            None => eprintln!("已写出快照：{SNAPSHOT}"),
        }
        return;
    }

    if let Err(difference) = compare(committed.as_deref(), &produced) {
        panic!("{difference}");
    }
}

/// 比对这一手不是橡皮图章：缺快照、改一个数、多一行、少一行，四种都得报出来。
///
/// 「快照变动需显式接受，默认导致测试失败」是 15 号票的票面，而上面那一条用例
/// 自己证不了它——快照对得上时它照样通过。这一条把比对拿出来单独喂几组，
/// 那句票面才有一处真的断言。
#[test]
fn a_changed_snapshot_is_never_silently_accepted() {
    let produced = "卷 A\n  001.png 2bit+FS 100\n";

    assert!(
        compare(Some(produced), produced).is_ok(),
        "一模一样的两份该对得上"
    );
    assert!(compare(None, produced).is_err(), "快照不在时该报错");
    assert!(
        compare(Some("卷 A\n  001.png 2bit+FS 101\n"), produced).is_err(),
        "输出体积变了一个字节也该报错"
    );
    assert!(
        compare(Some("卷 A\n  001.png 4bit 100\n"), produced).is_err(),
        "判定候选变了该报错"
    );
    assert!(compare(Some("卷 A\n"), produced).is_err(), "少一行该报错");
    assert!(
        compare(
            Some("卷 A\n  001.png 2bit+FS 100\n  002.png 1bit 50\n"),
            produced
        )
        .is_err(),
        "多一行该报错"
    );
}

/// 仓库里那一份快照的路径。
fn snapshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT)
}

/// 这一趟是来接受变动的吗。
fn accepting() -> bool {
    std::env::var_os(ACCEPT).is_some_and(|value| !value.is_empty())
}

/// 换行归一。仓库检出时可能带上 CRLF，而快照比的是判定与体积，不是行尾。
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// 仓库里那一份与这一趟算出来的比。不同就给出能读的差异，并说清楚怎么接受。
fn compare(committed: Option<&str>, produced: &str) -> Result<(), String> {
    let Some(committed) = committed else {
        return Err(format!(
            "{SNAPSHOT} 不在。第一次立这份快照，就先看一遍算出来的数，\
             再用 `{ACCEPT}=1 cargo test --test golden` 写下去。"
        ));
    };
    let committed = normalize(committed);
    let produced = normalize(produced);
    if committed == produced {
        return Ok(());
    }

    let expected: Vec<&str> = committed.lines().collect();
    let actual: Vec<&str> = produced.lines().collect();
    let mut lines = Vec::new();
    let mut elided = 0;
    for index in 0..expected.len().max(actual.len()) {
        let (before, after) = (expected.get(index), actual.get(index));
        if before == after {
            continue;
        }
        if lines.len() >= MAX_DIFF_LINES {
            elided += 1;
            continue;
        }
        let number = index + 1;
        if let Some(before) = before {
            lines.push(format!("  {number:>4} - {before}"));
        }
        if let Some(after) = after {
            lines.push(format!("  {number:>4} + {after}"));
        }
    }
    if elided > 0 {
        // 数的是行号，不是上面印出来的行：一个行号最多印两条。
        lines.push(format!("  …另有 {elided} 处不同，全文见 {SNAPSHOT}"));
    }

    Err(format!(
        "黄金回归快照对不上：这批夹具的判定或输出体积变了。\n\
         - 是调优的预期结果 → 复核下面每一行，再用 `{ACCEPT}=1 cargo test --test golden` 接受。\n\
         - 不是 → 这就是一次回归，别接受。\n\
         - 判定一个都没变、体积却整片挪了几个字节 → 多半是换了机器或动了依赖版本\
         （重采样按 CPU 指令集分派），那也不是回归。\n\
         `-` 是仓库里那一份，`+` 是这一趟算出来的：\n{}",
        lines.join("\n")
    ))
}

/// 这一趟算出来的整份快照。
fn snapshot() -> String {
    let space = Workspace::new();
    let mut text = String::from(HEADER);
    render(&mut text, &space, fixtures::BASELINE_DEVICE, &mono_cases());
    render(&mut text, &space, COLOR_DEVICE, &color_cases());
    text
}

/// 一个夹具卷：卷名，加上往里放页的那一手。
///
/// `summary_only` 是给长卷的：非退化的上分位与迟滞升档要页数够多才走得到（见本文件抬头，
/// 离群那一条不挑卷长），而几十行页行会把快照撑爆，真正的判定变动反而淹在里面。
/// 长卷因此只记卷级摘要——那一行里有基准档、驱动页、主体/离群/升档三个计数，
/// 本来就是卷级那几条路径的全部观测点。
struct Case {
    name: &'static str,
    build: Build,
    summary_only: bool,
}

/// 夹具卷的两种形态。归档那一支存在的理由：写进归档的页，除了成员名之外
/// 一条断言都没有——变异实验里让每个成员都写第一页的字节，全套测试一项没红。
/// 页字节数进了快照，那种串位就再也藏不住。
enum Build {
    Directory(fn(&Volume)),
    Archive(fn(&mut fixtures::Cbz)),
}

impl Case {
    const fn new(name: &'static str, build: fn(&Volume)) -> Self {
        Self {
            name,
            build: Build::Directory(build),
            summary_only: false,
        }
    }

    /// 长卷：只记卷级摘要，不逐页展开。
    const fn summary(name: &'static str, build: fn(&Volume)) -> Self {
        Self {
            name,
            build: Build::Directory(build),
            summary_only: true,
        }
    }

    /// 归档卷：输入是一个 CBZ，输出也是。
    const fn archive(name: &'static str, build: fn(&mut fixtures::Cbz)) -> Self {
        Self {
            name,
            build: Build::Archive(build),
            summary_only: false,
        }
    }
}

/// 铺一个长卷：`count` 页，第 `index` 页画什么由 `page` 说了算（下标从 1 起）。
///
/// 长卷用的都是小页。卷级那三条路径只看逐页判定的分布，不看页画的是什么，而小页把
/// 一卷几十页的代价压到可接受——代价是源比面板小，几何门在**每一页**上都不成立
/// （ADR 0007：抖动仅在目标尺寸未被下游缩放时启用）。一页成立的都没有，那些页于是
/// 自己就是主体，卷级基准档由它们定出、必然不抖（ADR 0007 决定第 5 条）。
/// 那是这几卷有意接受的形态：它们考的是上包络、离群与迟滞，不是几何。
///
/// **由此这几卷的基线踩在一件尚未想清楚的事情上**：几何门不成立时该用哪个位深集合，
/// 是 `CONTEXT.md` 的《尚未确立》里明确挂着的一条——对齐消失，灰阶硬上界的依据随之失效，
/// P0 只是暂且仍照 {1,2,4} 裁。那件事定下来的那一天，这几卷的位深与抖动会整片变动，
/// 而它与上包络、离群、迟滞一点关系都没有。**那不是回归。**
fn long_volume(volume: &Volume, count: usize, page: impl Fn(usize) -> image::DynamicImage) {
    for index in 1..=count {
        volume.page(&format!("{index:03}.png"), &page(index));
    }
}

/// 黑白面板上的夹具卷。spec 点名的每一类页都在这里出现一次。
fn mono_cases() -> Vec<Case> {
    vec![
        // B 类中位页：总缩放比 1.219 < 2，预缩退化为恒等，连续灰调直接考判据。
        Case::new("b-class-gradient", |volume| {
            volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
        }),
        // A 类代理：正好两倍面板，整数倍预缩真的走一遍（ADR 0001）。
        Case::new("a-class-gradient", |volume| {
            volume.page("001.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
        }),
        // 二值网点页。网点是因、灰调是果：预缩解析出来的那一档才是判据看到的东西。
        Case::new("screentone", |volume| {
            volume.page("001.png", &fixtures::screentone(fixtures::DOUBLE_PANEL));
        }),
        // 线稿页：判据对它相对不敏感，档位该压得下来。
        Case::new("line-art", |volume| {
            volume.page("001.png", &fixtures::line_art(fixtures::TYPICAL));
        }),
        // 纯色页：一个取值，编码器那一侧的下限。
        Case::new("solid", |volume| {
            volume.page("001.png", &fixtures::solid(fixtures::TYPICAL, 128));
        }),
        // 一页留白里一小块灰调崩坏，页与面板等大——**判据这一侧的分块聚合只在这里走得到**。
        // 别的夹具页画的都是铺满整页的东西，块与块之间读数相近，尾巴取第几块都一样；
        // 只有这一页的损伤是局部的，尾巴宽一格窄一格当场换一档位深（02 号票，ADR 0002 决定第 3 条）。
        // 补丁 128×128 = 16 块：够 K 那条绝对下限圈住，又不到 p99 在 2120 块上圈住的 22 块。
        Case::new("local-damage", |volume| {
            volume.page(
                "001.png",
                &fixtures::tone_patch(fixtures::panel_sized(), Size::new(128, 128)),
            );
        }),
        // 彩页落在黑白面板上：转灰后走灰度路径，是离群页的主要来源（`CONTEXT.md`）。
        Case::new("color-on-mono", |volume| {
            volume.page("001.png", &fixtures::color_page(fixtures::TYPICAL));
        }),
        // 跨页宽幅：fit-inside 由宽边定夺，贴住的是宽那条边，门仍成立。
        Case::new("spread", |volume| {
            volume.page("001.png", &fixtures::gradient(fixtures::SPREAD));
        }),
        // 源比目标小：不放大，一条边都贴不住，几何门在这一页上不成立。
        // 全卷只此一页，一页成立的都没有——它自己就是主体，抖动因此关着（ADR 0007 决定第 5 条）。
        Case::new("undersized", |volume| {
            volume.page(
                "001.png",
                &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
            );
        }),
        // **混合尺寸卷：06 号票的现场。**四页正片贴住面板，一张封面比目标还小。
        // 门逐页判（ADR 0007 决定第 1 条）：那一张封面不再否决另外四页的抖动，
        // 它自己拿卷级基准档的位深、抖动关掉。这一卷因此是快照里唯一两套候选集
        // 都用得上的那个——一张封面否决整卷那件事回来的话，四页正片的候选会整片变短。
        Case::new("mixed-size", |volume| {
            volume.page(
                "001.png",
                &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
            );
            volume.page("002.png", &fixtures::gradient(fixtures::TYPICAL));
            volume.page("003.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("004.png", &fixtures::gradient(fixtures::DOUBLE_PANEL));
            volume.page("005.png", &fixtures::screentone(fixtures::TYPICAL));
        }),
        // 多页混排：卷内档位一致与驱动页落在哪一页，只有多页才看得出来（ADR 0006）。
        // 三页线稿，另两页（渐变与纯色）在线稿过得去的那一档上远在界外，
        // 五页的卷因此摘得出离群页。**短卷上离群页那一层是唯一还挡着的防线**——上分位的秩在 20 页以内
        // 就是页数本身，基准档那一层已经退化了（见 `envelope` 的 `envelope`）。
        // 迟滞升档要页数够多才走得到，那一条由下面的长卷喂。
        Case::new("mixed", |volume| {
            volume.page("001.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("002.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("003.png", &fixtures::gradient(fixtures::TYPICAL));
            volume.page("004.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("005.png", &fixtures::solid(fixtures::TYPICAL, 200));
        }),
        // 以下三卷至少二十页。二十是**基准档**那一层的下限，不是随手取的：
        // 上分位的秩是 ceil(0.95n)，n≤19 时它等于 n，秩落在排序末位——基准档成了
        // 判定最高的那一页自己，上分位整个退化。n=20 时秩是 19，它第一次真的是个分位。
        // 离群页那一层不受这个下限约束：立脚点逐页各取一个、不含被判的这一页，
        // 短卷照样摘得出来（上面的 `mixed` 就是五页的卷，加固批 01 号票）。
        //
        // 填充页一律用 `solid`：卷级那三条路径只看判据的分布，不看页画的是什么，
        // 而纯色页的生成、编码、解码、判据全是这批夹具里最便宜的。
        //
        // 三卷都只记卷级摘要——那一行里的基准档、驱动页、主体/离群/升档三个计数，
        // 就是这几条路径的全部观测点，二十行页行只会把真正的变动淹掉。

        // 非退化的上分位 + 一页离群：基准档由分位定出，那一页另行摘走。
        Case::summary("envelope-outlier", |volume| {
            long_volume(volume, 20, |index| {
                if index == 20 {
                    fixtures::solid(fixtures::TINY, 128)
                } else {
                    fixtures::line_art(fixtures::TINY)
                }
            });
        }),
        // 离群占比 10%，越过上分位那 5% 的线。**这一卷正是加固批 01 号票的现场**：
        // 立脚点从前由全卷聚合而来，秩 19 落进那两页自己里，它们合力把立脚点抬上去，
        // 检验又在那一档上做，于是谁都不显著偏离——摘出 0 页，十八页主体被拖高到 4bit。
        // 立脚点改成逐页各取一个、不含被判的这一页之后，这两页摘了出去，主体落回 1bit。
        // 快照里那一行记的就是修完之后的样子。
        Case::summary("envelope-outlier-heavy", |volume| {
            long_volume(volume, 20, |index| {
                if index >= 19 {
                    fixtures::solid(fixtures::TINY, 128)
                } else {
                    fixtures::line_art(fixtures::TINY)
                }
            });
        }),
        // 迟滞升档：连续三页够不上基准档、又够不上离群，整段一起升。
        // 三页是 `HYSTERESIS_PAGES` 的取值。卷长 60 而不是 20，是因为这三页要落在
        // 上分位的秩之外，基准档才留得住填充页那一档——20 页时秩是 19，那三页排在
        // 17、18、19 位上，基准档跟着被它们抬上去，迟滞也就无档可升。
        Case::summary("envelope-hysteresis", |volume| {
            long_volume(volume, 60, |index| {
                if (8..=10).contains(&index) {
                    // 判据在基准档上过线、又够不上离群那三倍线的那一档。
                    fixtures::solid(fixtures::TINY, 15)
                } else {
                    fixtures::line_art(fixtures::TINY)
                }
            });
        }),
        // 归档卷：输入是 CBZ，输出也是。两页画得不一样，字节数因此各不相同——
        // 归档写出把成员串位的话，这两个数会当场对不上。透传文件一并进来，
        // 「原样搬过去」在归档那一侧同样是一条会被改坏的性质。
        Case::archive("archive", |cbz| {
            cbz.page("001.png", &fixtures::line_art(fixtures::TYPICAL))
                .page("002.png", &fixtures::gradient(fixtures::TYPICAL))
                .file("ComicInfo.xml", b"<ComicInfo/>");
        }),
        // 三种页状态与透传文件各占一格（04 号票）：完好的那一页、部分救回的那一页、
        // 失败的那两页——一张连尺寸都分配不下，一张尺寸有而一个像素都没救回来。
        // 这一卷整个进隔离目录（12 号票），占位页按卷内统一尺寸出；
        // 部分救回的那一页按自己的尺寸出，也不进卷级上包络。
        Case::new("damaged", |volume| {
            volume.page("001.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.file("002.png", &fixtures::truncated_page(fixtures::TYPICAL));
            volume.file("003.png", &fixtures::oversized_page());
            volume.file(
                "004.png",
                &fixtures::salvages_nothing_page(fixtures::TYPICAL),
            );
            volume.file("ComicInfo.xml", b"<ComicInfo/>");
        }),
    ]
}

/// 彩色面板上的夹具卷。彩页在这里保留颜色，同卷的灰度页照走灰度路径。
fn color_cases() -> Vec<Case> {
    vec![Case::new("color-on-color", |volume| {
        volume.page("001.png", &fixtures::color_page(fixtures::TYPICAL));
        volume.page("002.png", &fixtures::gradient(fixtures::TYPICAL));
    })]
}

/// 把一组夹具卷跑一遍，结果接在快照后面。
///
/// 一趟 `run` 收全部卷，不是一卷一趟：多卷同趟本来就是常态，而卷级判定各卷各判，
/// 合在一起不改变任何一卷的结果。
fn render(text: &mut String, space: &Workspace, device: &str, cases: &[Case]) {
    // 目录卷要活到 `run` 之后（`Volume` 一落地就把临时目录收走），归档卷写完即成文件。
    let mut directories: Vec<Volume> = Vec::new();
    let mut inputs: Vec<PathBuf> = Vec::new();
    for case in cases {
        match case.build {
            Build::Directory(build) => {
                let volume = space.volume(case.name);
                build(&volume);
                inputs.push(volume.path().to_owned());
                directories.push(volume);
            }
            Build::Archive(build) => {
                let mut archive = space.cbz(case.name);
                build(&mut archive);
                inputs.push(archive.write());
            }
        }
    }

    let report = tonefit::run(&Request {
        inputs,
        // 各设备一个输出根：同一个工作区里两趟并列，互不覆盖。
        output_root: space.out_named(&format!("out-{device}")),
        profile: fixtures::profile(device),
        filter: Filter::default(),
        bit_depth: None,
        dither: None,
        per_page: false,
        cache_budget: CacheBudget::default(),
        mode: Mode::Process,
        io_mode: IoMode::default(),
        progress: None,
        // 见抬头：记录里的工具版本会让体积跟着版本号动。
        metadata: false,
    })
    .expect("黄金回归的夹具都该处理得下来");

    let summary_only: HashSet<&str> = cases
        .iter()
        .filter(|case| case.summary_only)
        .map(|case| case.name)
        .collect();

    for volume in &report.volumes {
        let name = volume_name(volume);
        let mut block = vec![
            format!(
                "[{device}] {name}{}",
                if volume.isolated() { " · 隔离" } else { "" }
            ),
            format!("  几何门 {}", gate(volume)),
            format!("  卷级   {}", fixtures::volume_verdict(volume)),
        ];
        if summary_only.contains(name.as_str()) {
            // 长卷到此为止：卷级那一行已经载着这几条路径的全部观测点。
            text.push('\n');
            text.push_str(&block.join("\n"));
            text.push('\n');
            continue;
        }
        let sizes = output_sizes(volume);
        block.extend(
            volume
                .pages
                .iter()
                .map(|page| format!("  {}", page_line(volume, page, &sizes))),
        );
        block.extend(
            passthrough(volume, &sizes)
                .into_iter()
                .map(|(name, bytes)| format!("  透传 {name} · {bytes} 字节")),
        );
        text.push('\n');
        text.push_str(&block.join("\n"));
        text.push('\n');
    }
}

/// 卷名：源路径的最后一段。快照不记全路径——那是临时目录，每趟都不一样。
fn volume_name(volume: &VolumeReport) -> String {
    volume
        .volume
        .file_name()
        .expect("夹具卷有名字")
        .to_string_lossy()
        .into_owned()
}

/// 这一卷的几何门（ADR 0007）：判定范围有几页，其中几页不成立。
///
/// 门逐页判（06 号票），卷级这一行因此记的是两个计数，不是一页的名字。
/// 是**哪几页**不成立，逐页那几行自己说得出来：它们的理由写着「几何门不成立」。
fn gate(volume: &VolumeReport) -> String {
    format!(
        "判定范围 {} 页 · 不成立 {} 页",
        volume.judged_by_the_gate().count(),
        volume.outside_the_gate().count()
    )
}

/// 一页的那一行：名字、目标尺寸、判定候选、输出字节、理由。
///
/// 前四列一律 ASCII 且定宽，理由排在最后——中文的显示宽度对不齐，夹在中间会把整列拧歪。
fn page_line(volume: &VolumeReport, page: &PageReport, sizes: &BTreeMap<String, u64>) -> String {
    let (candidate, mut reason) = match page.branch() {
        Some(PageBranch::Gray { verdict, .. }) => {
            (verdict.candidate.to_string(), verdict.reason.to_string())
        }
        Some(PageBranch::Color) => ("color".to_owned(), "彩色分支 · 只缩放不量化".to_owned()),
        // 失败的那一句里有成员名与平台相关的分隔符，进不了快照（原因见 tests/isolation.rs）。
        None => ("failed".to_owned(), "失败页 · 占位页".to_owned()),
    };
    // 彩页在黑白面板上转灰、走灰度路径，报告里两件事都在（见 `PageOutcome`）。
    if page.color() == Some(PageColor::Color)
        && matches!(page.branch(), Some(PageBranch::Gray { .. }))
    {
        reason.push_str(" · 彩页转灰");
    }
    // 救回了多少进快照（04 号票）：它决定这一页算不算数——救回到 0 就是一张失败页，
    // 而这个比例还决定了它进不进上包络。夹具固定，它因此也是个定值。
    if let Some(salvage) = page.salvage() {
        reason.push_str(&format!(" · {salvage}"));
    }
    let name = fixtures::relative_name(&volume.output, &page.output);
    let bytes = sizes
        .get(&name)
        .copied()
        .unwrap_or_else(|| panic!("输出里没有成员 {name}"));
    format!(
        "{:<14} {:<10} {:<8} {:>7}  {reason}",
        fixtures::relative_name(&volume.volume, &page.source),
        format!("{}x{}", page.size.width, page.size.height),
        candidate,
        bytes,
    )
}

/// 这一卷输出里每个成员的字节数，按成员名索引。
///
/// 目录卷从文件系统读，归档卷从容器里读——归档成员不是磁盘上的文件，`fs::metadata`
/// 够不着它们。两种形态因此走同一个索引，下游不必再分辨自己面对的是哪一种。
fn output_sizes(volume: &VolumeReport) -> BTreeMap<String, u64> {
    if volume.output.is_dir() {
        walkdir::WalkDir::new(&volume.output)
            .into_iter()
            .map(|entry| entry.expect("遍历输出容器"))
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                (
                    fixtures::relative_name(&volume.output, entry.path()),
                    entry.metadata().expect("读输出成员的大小").len(),
                )
            })
            .collect()
    } else {
        fixtures::read_cbz(&volume.output)
            .into_iter()
            .map(|(name, bytes)| (name, bytes.len() as u64))
            .collect()
    }
}

/// 这一卷原样透传的非图片文件：名字与字节数，按名字排好。
///
/// `VolumeReport` 不单列它们（页才有逐个的报告），因此从输出成员里数：除页之外
/// 剩下的就是透传过来的那些。它们进快照，是因为「原样透传」也是一条会被改坏的性质——
/// 少一个文件、多压一遍，页那几行都不会动。
fn passthrough(volume: &VolumeReport, sizes: &BTreeMap<String, u64>) -> Vec<(String, u64)> {
    let pages: HashSet<String> = volume
        .pages
        .iter()
        .map(|page| fixtures::relative_name(&volume.output, &page.output))
        .collect();
    sizes
        .iter()
        .filter(|(name, _)| !pages.contains(name.as_str()))
        .map(|(name, bytes)| (name.clone(), *bytes))
        .collect()
}
