//! 黄金回归：一组固定夹具的判定与输出体积记进快照，任何变动都要显式接受。
//!
//! 这里一条性质都不主张——它只钉住「今天这批夹具算出来的就是这些数」。存在的理由是
//! 15 号票那一句：防止调优在无人察觉时改变判定结果。判据的低通核、掩蔽加权、上包络的
//! 分位、迟滞页数、编码器在灰度与调色板之间的取舍，任何一处动一下都会在这里露出来。
//!
//! 与 `tests/metric.rs` 分工相反：那边测的是判据**该有的性质**，数值动了不算错；
//! 这边测的是**数值本身**，动了就要有人当场答一句「为什么」。
//!
//! 快照在 `tests/golden-snapshot.txt`，与本文件一起入版本库。夹具全部由代码生成，
//! 仓库里没有一张真实漫画页（真实素材那一路是 `tests/smoke.rs`）。

mod fixtures;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use fixtures::{Volume, Workspace};
use tonefit::{
    CacheBudget, Filter, GeometryGate, IoMode, Mode, PageBranch, PageColor, PageOutcome,
    PageReport, Request, VolumeReport,
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
struct Case {
    name: &'static str,
    build: fn(&Volume),
}

impl Case {
    const fn new(name: &'static str, build: fn(&Volume)) -> Self {
        Self { name, build }
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
        // 彩页落在黑白面板上：转灰后走灰度路径，是离群页的主要来源（`CONTEXT.md`）。
        Case::new("color-on-mono", |volume| {
            volume.page("001.png", &fixtures::color_page(fixtures::TYPICAL));
        }),
        // 跨页宽幅：fit-inside 由宽边定夺，贴住的是宽那条边，门仍成立。
        Case::new("spread", |volume| {
            volume.page("001.png", &fixtures::gradient(fixtures::SPREAD));
        }),
        // 源比目标小：不放大，一条边都贴不住，几何门不成立 → 抖动整卷关闭（ADR 0007）。
        Case::new("undersized", |volume| {
            volume.page(
                "001.png",
                &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
            );
        }),
        // 多页混排：卷内档位一致与驱动页落在哪一页，只有多页才看得出来（ADR 0006）。
        // 这几页驱不出离群与迟滞升档——那两条在 `tests/pipeline.rs` 里有专门的用例，
        // 这里的 0 只是钉住「这批页不该触发它们」。
        Case::new("mixed", |volume| {
            volume.page("001.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("002.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("003.png", &fixtures::gradient(fixtures::TYPICAL));
            volume.page("004.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.page("005.png", &fixtures::solid(fixtures::TYPICAL, 200));
        }),
        // 坏页与透传文件：救回的那一页、失败的那一页、原样搬过去的那一个文件。
        // 这一卷整个进隔离目录（12 号票），占位页按卷内统一尺寸出。
        Case::new("damaged", |volume| {
            volume.page("001.png", &fixtures::line_art(fixtures::TYPICAL));
            volume.file("002.png", &fixtures::truncated_page(fixtures::TYPICAL));
            volume.file("003.png", &fixtures::oversized_page());
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
    let volumes: Vec<Volume> = cases
        .iter()
        .map(|case| {
            let volume = space.volume(case.name);
            (case.build)(&volume);
            volume
        })
        .collect();

    let report = tonefit::run(&Request {
        inputs: volumes
            .iter()
            .map(|volume| volume.path().to_owned())
            .collect(),
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

    for volume in &report.volumes {
        let mut block = vec![
            format!(
                "[{device}] {}{}",
                volume_name(volume),
                if volume.isolated() { " · 隔离" } else { "" }
            ),
            format!("  几何门 {}", gate(volume)),
            format!("  卷级   {}", fixtures::volume_verdict(volume)),
        ];
        block.extend(
            volume
                .pages
                .iter()
                .map(|page| format!("  {}", page_line(volume, page))),
        );
        block.extend(
            passthrough(volume)
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

/// 这一卷的几何门（ADR 0007）。
fn gate(volume: &VolumeReport) -> String {
    match volume.gate {
        Some(GeometryGate::Holds) => "成立".to_owned(),
        Some(GeometryGate::Broken { page }) => {
            format!("不成立 · {} 贴不住面板", fixtures::page_at(volume, page))
        }
        None => "—".to_owned(),
    }
}

/// 一页的那一行：名字、目标尺寸、判定候选、输出字节、理由。
///
/// 前四列一律 ASCII 且定宽，理由排在最后——中文的显示宽度对不齐，夹在中间会把整列拧歪。
fn page_line(volume: &VolumeReport, page: &PageReport) -> String {
    let (candidate, mut reason) = match &page.outcome {
        PageOutcome::Processed {
            branch: PageBranch::Gray { verdict, .. },
            ..
        } => (verdict.candidate.to_string(), verdict.reason.to_string()),
        PageOutcome::Processed {
            branch: PageBranch::Color,
            ..
        } => ("color".to_owned(), "彩色分支 · 只缩放不量化".to_owned()),
        // 失败的那一句里有成员名与平台相关的分隔符，进不了快照（原因见 tests/isolation.rs）。
        PageOutcome::Failed { .. } => ("failed".to_owned(), "失败页 · 占位页".to_owned()),
    };
    // 彩页在黑白面板上转灰、走灰度路径，报告里两件事都在（见 `PageOutcome`）。
    if page.color() == Some(PageColor::Color)
        && matches!(page.branch(), Some(PageBranch::Gray { .. }))
    {
        reason.push_str(" · 彩页转灰");
    }
    let bytes = fs::metadata(&page.output)
        .unwrap_or_else(|error| panic!("读 {} 的大小：{error}", page.output.display()))
        .len();
    format!(
        "{:<14} {:<10} {:<8} {:>7}  {reason}",
        fixtures::relative_name(&volume.volume, &page.source),
        format!("{}x{}", page.size.width, page.size.height),
        candidate,
        bytes,
    )
}

/// 这一卷原样透传的非图片文件：名字与字节数，按名字排好。
///
/// `VolumeReport` 不单列它们（页才有逐个的报告），因此从输出容器里数：里面除页之外
/// 剩下的就是透传过来的那些。它们进快照，是因为「原样透传」也是一条会被改坏的性质——
/// 少一个文件、多压一遍，页那几行都不会动。
fn passthrough(volume: &VolumeReport) -> Vec<(String, u64)> {
    let pages: HashSet<&Path> = volume
        .pages
        .iter()
        .map(|page| page.output.as_path())
        .collect();
    let mut extras: Vec<(String, u64)> = walkdir::WalkDir::new(&volume.output)
        .into_iter()
        .map(|entry| entry.expect("遍历输出容器"))
        .filter(|entry| entry.file_type().is_file() && !pages.contains(entry.path()))
        .map(|entry| {
            (
                fixtures::relative_name(&volume.output, entry.path()),
                entry.metadata().expect("读透传文件的大小").len(),
            )
        })
        .collect();
    extras.sort();
    extras
}
