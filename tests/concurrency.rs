//! 介质探测、读取并发与进度（13 号票）。
//!
//! 断言全在 `run` 这个 seam 上：报告说这一卷是怎么读的，输出说读法没有改变结果。
//! 读取层自己的性质——有界通道按字节预算背压、峰值不随并发度长——在 `src/read.rs` 的
//! 单元用例里量，那里量得到在途字节，这里量不到。
//!
//! 探测本身的性质（同一次运行里不同路径可得不同结论、同一条通道只探一次、探不出来退到保守）
//! 同理在 `src/medium.rs` 里：那些要换一对假的平台探测进来才测得了，而真实机器上
//! 只有一块盘可探。

mod fixtures;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use fixtures::{SMALLER_THAN_TARGET, Workspace};
use tonefit::{
    ChosenBy, Dither, GeometryGate, IoMode, Mode, Progress, ProgressSink, Request, Size, Verdict,
};

/// 一条贴住面板宽边的窄页：几何门成立，而像素少到几十页连跑也不慢。
///
/// 宽取基准面板的 1264（`fixtures::BASELINE_DEVICE`），高远不到面板高——fit-inside 于是
/// 原样输出，而宽那条边正贴着面板，门照样成立。本文件要的正是这个：门开着，
/// 候选集里带着抖动那一维，判据每页多求几个，乱序跑与顺着跑的差别才有地方露出来。
const TOUCHING: Size = Size::new(1264, 200);

/// 一个够长的卷：页数多到几条读取线程真的会互相错开。
fn long_volume(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    for index in 0..24 {
        volume.page(&format!("{index:03}.png"), &fixtures::gradient(TOUCHING));
    }
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 一个第 3 页与第 9 页贴不住面板的卷，其余十页都贴住：**混排卷**（06 号票）。
///
/// 门逐页判，这一卷因此两套候选集都用得上：那两页只剩不抖的三个，另外十页六个都在。
fn volume_with_two_gate_breakers(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    for index in 0..12 {
        let size = if index == 3 || index == 9 {
            SMALLER_THAN_TARGET
        } else {
            TOUCHING
        };
        volume.page(&format!("{index:03}.png"), &fixtures::gradient(size));
    }
    volume
}

/// 一个目录树下的全部文件，按相对路径排好。两份输出比对用它。
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).expect("读输出目录") {
            let path = entry.expect("读一项").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let relative = path.strip_prefix(root).expect("在根之下").to_path_buf();
                into.insert(relative, std::fs::read(&path).expect("读输出文件"));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// **读法不改变结果。**串行读一趟、并发读一趟，写出的每一个字节都一样。
///
/// 这是本票最要紧的那一条：第一遍从一页一页顺着做改成了乱序满核跑（判据、缓存序号、
/// 几何门都在那条路上），而这些改动对外应当完全不可见。
#[test]
fn reading_serially_or_concurrently_writes_the_very_same_bytes() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");
    let run = |mode: IoMode, out: &str| {
        tonefit::run(&Request {
            io_mode: mode,
            output_root: space.out_named(out),
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功")
    };

    let serial = run(IoMode::Serial, "serial");
    let concurrent = run(IoMode::Concurrent, "concurrent");

    assert_eq!(
        tree(&space.out_named("serial"))
            .values()
            .collect::<Vec<_>>(),
        tree(&space.out_named("concurrent"))
            .values()
            .collect::<Vec<_>>(),
        "两种读法写出的字节不一样"
    );
    // 报告里那些算出来的事实也得一样：卷级判定、缓存用量、解码次数。
    // 几何门不在这一排里——它跟着页走（06 号票），逐页那一处比得更细。
    let (one, other) = (&serial.volumes[0], &concurrent.volumes[0]);
    assert_eq!(format!("{:?}", one.verdict), format!("{:?}", other.verdict));
    assert_eq!(one.decodes, other.decodes);
    // 缓存的**总量**与顺序无关，因此两趟必须一样；常驻与溢写的分法则随存入顺序而变，
    // 而第一遍是乱序满核跑的（见 `cache::PageCache::insert`）——那两个数不在这里断言。
    assert_eq!(one.cache.pages, other.cache.pages);
    assert_eq!(one.cache.raw, other.cache.raw);
    assert_eq!(one.cache.stored, other.cache.stored);
    // 逐页的门与判定同样逐页相等——卷级相等掩盖得了逐页的错位。
    let decided = |report: &tonefit::Report| {
        report.volumes[0]
            .pages
            .iter()
            .map(|page| {
                (
                    page.source.file_name().map(ToOwned::to_owned),
                    page.gate(),
                    page.verdict(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(decided(&serial), decided(&concurrent));
}

/// `--io-mode` 覆盖自动探测，而报告说得出这个数是点名来的（13 号票）。
///
/// 覆盖的是**策略**不是事实：探到的介质两趟都照实说，变的只是派几条读取。
#[test]
fn io_mode_overrides_the_probe_and_the_report_says_where_the_number_came_from() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");
    let plan = |mode: IoMode, out: &str| {
        tonefit::run(&Request {
            io_mode: mode,
            output_root: space.out_named(out),
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功")
        .volumes[0]
            .io
            .clone()
    };

    let automatic = plan(IoMode::Auto, "auto");
    let serial = plan(IoMode::Serial, "serial");
    let concurrent = plan(IoMode::Concurrent, "concurrent");

    assert_eq!(automatic.chosen_by, ChosenBy::Probe);
    assert_eq!(serial.chosen_by, ChosenBy::Named);
    assert_eq!(serial.readers, 1, "点名串行还派了不止一条读取");
    assert_eq!(concurrent.chosen_by, ChosenBy::Named);
    assert!(
        concurrent.readers >= serial.readers,
        "点名并发派得比串行还少：{concurrent}"
    );
    // 探到的介质与点名无关：三趟说的是同一块盘。
    assert_eq!(automatic.medium, serial.medium);
    assert_eq!(automatic.medium, concurrent.medium);
    // 报告那句话里点得出是谁定的这个数。
    assert!(serial.to_string().contains("--io-mode"), "{serial}");
    assert!(!automatic.to_string().contains("--io-mode"), "{automatic}");
}

/// 一次运行里，两个卷各拿各的读取计划——不是一趟只判一次（ADR 0009 决定第 2 条）。
///
/// 真实机器上只有一块盘，两个路径的**介质**必然相同；分得开的是另一维：归档卷恒串行。
/// 点名并发之后两个卷仍给出不同的答案，「按卷判定」这件事因此在 seam 上看得见。
#[test]
fn two_volumes_in_one_run_each_carry_their_own_read_plan() {
    let space = Workspace::new();
    let directory = long_volume(&space, "volume-a");
    let mut archive = space.cbz("volume-b");
    for index in 0..8 {
        archive.page(&format!("{index:03}.png"), &fixtures::gradient(TOUCHING));
    }
    let archive = archive.write();

    let report = tonefit::run(&Request {
        io_mode: IoMode::Concurrent,
        ..fixtures::request(&space, [directory.path(), archive.as_path()])
    })
    .expect("处理应当成功");

    assert_eq!(report.volumes.len(), 2);
    let (directory, archive) = (&report.volumes[0].io, &report.volumes[1].io);
    assert_eq!(directory.chosen_by, ChosenBy::Named);
    // 归档卷点名并发也改不了：一个 ZipArchive 就是一个游标。
    assert_eq!(archive.chosen_by, ChosenBy::ArchiveScan);
    assert_eq!(archive.readers, 1);
    assert!(archive.to_string().contains("顺序扫"), "{archive}");
}

/// 并发读之下每页仍然只解码一次（ADR 0005 的那条不变量）。
#[test]
fn every_page_is_still_decoded_exactly_once_when_reading_concurrently() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");

    let report = tonefit::run(&Request {
        io_mode: IoMode::Concurrent,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let volume = &report.volumes[0];
    assert_eq!(volume.decodes, volume.pages.len());
}

/// 混排卷里几何门不成立的仍然只有那两页，逐页判定也一趟一个样地不变。
///
/// 并发下页乱序算完，而几何门与候选集都在那条路上。门逐页判（ADR 0007 决定第 1 条）之后
/// 每一页自己判自己，答案本就与调度无关——这一条钉的是它真的无关：从前那一套要在收尾处
/// 按最小页序定出一个卷级的门，而那时**换一次调度就可能换一个答案**。
///
/// 顺带钉住 06 号票的票面：那十页贴住面板的页照旧抖得动，没有被那两页连坐。
#[test]
fn a_mixed_volume_gates_the_same_pages_every_time_under_concurrency() {
    let space = Workspace::new();
    let volume = volume_with_two_gate_breakers(&space, "volume-a");

    // dry-run：一个文件都不落盘，因此同一个输出根跑几趟都互不干扰。
    let mut before: Option<Vec<(Option<GeometryGate>, Option<Verdict>)>> = None;
    for attempt in 0..4 {
        let report = tonefit::run(&Request {
            io_mode: IoMode::Concurrent,
            mode: Mode::DryRun,
            ..fixtures::request(&space, [volume.path()])
        })
        .expect("处理应当成功");

        let pages = &report.volumes[0].pages;
        let broken: Vec<usize> = pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.gate() == Some(GeometryGate::Broken))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(broken, vec![3, 9], "第 {attempt} 趟摘错了页");
        // 另外十页照旧抖得动：一张贴不住面板的页不否决整卷（06 号票）。
        for (index, page) in pages.iter().enumerate() {
            let dither = fixtures::verdict(page).candidate.dither;
            let expected = if broken.contains(&index) {
                Dither::Off
            } else {
                Dither::FloydSteinberg
            };
            assert_eq!(dither, expected, "第 {attempt} 趟第 {index} 页抖得不对");
        }

        let decided: Vec<_> = pages
            .iter()
            .map(|page| (page.gate(), page.verdict()))
            .collect();
        match &before {
            Some(first) => assert_eq!(*first, decided, "第 {attempt} 趟与头一趟不一样"),
            None => before = Some(decided),
        }
    }
}

/// 门不成立而 `--dither fs` 点了抖动：并发之下报的仍是同一页。
///
/// 这一支上没有报告可看——整卷的调用返回 `Err`。那句话里指的那一页因此是唯一的线索，
/// 它必须与顺着做时是同一页。
#[test]
fn a_dither_the_gate_forbids_is_refused_naming_the_same_page_every_time() {
    let space = Workspace::new();
    let volume = volume_with_two_gate_breakers(&space, "volume-a");

    for attempt in 0..4 {
        let error = tonefit::run(&Request {
            io_mode: IoMode::Concurrent,
            mode: Mode::DryRun,
            dither: Some(Dither::FloydSteinberg),
            ..fixtures::request(&space, [volume.path()])
        })
        .expect_err("门不成立时点名抖动该被拒");

        let said = format!("{error:#}");
        assert!(said.contains("003.png"), "第 {attempt} 趟：{said}");
        assert!(!said.contains("009.png"), "第 {attempt} 趟指错了页：{said}");
    }
}

/// 记账用的进度观察者：管线报的每一步都落在这里。
#[derive(Clone, Default)]
struct Tally(Arc<Counts>);

#[derive(Default)]
struct Counts {
    /// 每个卷开始时预告的步数，按开始顺序。
    started: Mutex<Vec<u64>>,
    /// 实际走过的步数总和。
    advanced: AtomicU64,
    /// 收摊过几次。
    finished: AtomicUsize,
}

impl Progress for Tally {
    fn volume_started(&self, _volume: &Path, steps: u64) {
        self.0.started.lock().expect("记账没有中毒").push(steps);
    }

    fn stepped(&self) {
        self.0.advanced.fetch_add(1, Ordering::Relaxed);
    }

    fn volume_finished(&self) {
        self.0.finished.fetch_add(1, Ordering::Relaxed);
    }
}

impl Tally {
    fn started(&self) -> Vec<u64> {
        self.0.started.lock().expect("记账没有中毒").clone()
    }

    fn advanced(&self) -> u64 {
        self.0.advanced.load(Ordering::Relaxed)
    }

    fn finished(&self) -> usize {
        self.0.finished.load(Ordering::Relaxed)
    }
}

/// 长任务报得出进度（spec 的 story 30），而且预告的步数与真走的步数对得上。
///
/// 三段各占多少：幂等那一道读全部成员、第一遍走每一页、第二遍写全部成员。
/// 24 页 + 1 个透传文件，因此 25 + 24 + 25 = 74 步。这个数钉在这里，
/// 是为了让进度条不会「停在某个百分比上再也不动」——那正是预告与实际走散的样子。
#[test]
fn a_long_run_reports_every_step_it_announced() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");
    let tally = Tally::default();

    tonefit::run(&Request {
        progress: Some(ProgressSink::new(tally.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(tally.started(), vec![25 + 24 + 25]);
    assert_eq!(tally.advanced(), 25 + 24 + 25);
    assert_eq!(tally.finished(), 1);
}

/// dry-run 没有第二遍，预告的步数就少那一段——不预告一段永远走不到的路。
#[test]
fn a_dry_run_announces_only_the_passes_it_will_make() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");
    let tally = Tally::default();

    tonefit::run(&Request {
        mode: Mode::DryRun,
        progress: Some(ProgressSink::new(tally.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert_eq!(tally.started(), vec![25 + 24]);
    assert_eq!(tally.advanced(), 25 + 24);
    assert_eq!(tally.finished(), 1);
}

/// 幂等命中的卷提前收摊：预告的是上界，走过的只有幂等那一道，而它照样收得了尾。
///
/// 不收尾的话进度条会停在三分之一处等下一个卷——「跳过」在屏幕上就成了「卡住」。
#[test]
fn a_skipped_volume_stops_early_and_still_finishes_its_bar() {
    let space = Workspace::new();
    let volume = long_volume(&space, "volume-a");
    let request = fixtures::request(&space, [volume.path()]);
    tonefit::run(&request).expect("头一趟应当成功");

    let tally = Tally::default();
    let report = tonefit::run(&Request {
        progress: Some(ProgressSink::new(tally.clone())),
        ..request
    })
    .expect("第二趟应当成功");

    assert!(report.volumes[0].skipped(), "第二趟没有被幂等跳过");
    assert_eq!(tally.started(), vec![25 + 24 + 25], "预告的是上界");
    assert_eq!(tally.advanced(), 25, "跳过的卷只该走幂等那一道");
    assert_eq!(tally.finished(), 1, "跳过的卷没有收尾");
}
