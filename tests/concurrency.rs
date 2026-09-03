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
    ChosenBy, Dither, Event, FitMode, GeometryGate, Instruction, IoMode, Mode, Progress,
    ProgressSink, Request, Size, Verdict, VolumeVerdict,
};

/// 一条贴住面板高边的窄页：几何门成立，而像素少到几十页连跑也不慢。
///
/// 高取基准面板的 1680（`fixtures::BASELINE_DEVICE`），宽远不到面板宽。这个尺寸是
/// **两种适配方式的公共不动点**：高已经等于面板高，以高为准原样输出，fit-inside
/// 也不放大——本文件因此两条路上都跑得快，而门在两条路上都开着。
///
/// 本文件要的正是门开着：候选集里带着抖动那一维，判据每页多求几个，
/// 乱序跑与顺着跑的差别才有地方露出来。
///
/// 拿它跑的页一律配 `fixtures::full_bleed_gradient` 那圈墨边（页几何批 09 号票）：
/// 裁边一裁，「原样输出」当场不成立——渐变下方那 21.6% 亮于墨阈的白边会被裁掉，
/// 页于是又要被放大回面板高，上面那句话就成了假话。
const TOUCHING: Size = Size::new(200, 1680);

/// 一个够长的卷：页数多到几条读取线程真的会互相错开。
fn long_volume(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    for index in 0..24 {
        volume.page(
            &format!("{index:03}.png"),
            &fixtures::full_bleed_gradient(TOUCHING),
        );
    }
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 一个第 3 页与第 9 页贴不住面板的卷，其余十页都贴住：**混排卷**（06 号票）。
///
/// 门逐页判，这一卷因此两套候选集都用得上：那两页只剩不抖的三个，另外十页六个都在。
///
/// **它只在 `--fit inside` 下是混排卷**（页几何批 01 号票）：以高为准会把那两页放大到
/// 面板高，门跟着成立，一卷十二页都拿满候选。用它的两条用例因此各自点名了适配方式。
fn volume_with_two_gate_breakers(space: &Workspace, name: &str) -> fixtures::Volume {
    let volume = space.volume(name);
    for index in 0..12 {
        let size = if index == 3 || index == 9 {
            SMALLER_THAN_TARGET
        } else {
            TOUCHING
        };
        // 四边顶着墨：这一条钉的是几何门，而裁边会改掉每一页的几何（页几何批 02 号票）。
        volume.page(
            &format!("{index:03}.png"),
            &fixtures::full_bleed_gradient(size),
        );
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

    assert_eq!(automatic.readers.chosen_by, ChosenBy::Probe);
    assert_eq!(serial.readers.chosen_by, ChosenBy::Named);
    assert_eq!(serial.readers.count, 1, "点名串行还派了不止一条读取");
    assert_eq!(concurrent.readers.chosen_by, ChosenBy::Named);
    // 断言的是一个**等号**，不是「不少于串行那一档」：串行恒为 1，那个不等号展开就是
    // 「不小于 1」，把并发悄悄退化成串行照样绿——一条永真的断言在 CI 日志里与真断言长得一样。
    // 点名并发派几条由核数定（`crate::cores`），本进程问到的是同一个数。
    // 单核机器上并发与串行本就分不开，这个等号在那里退成 `1 == 1`；
    // 「Concurrent 映射到核数」与主机无关的那一半钉在 `src/medium.rs` 的单元用例里。
    assert_eq!(
        concurrent.readers.count,
        num_cpus::get().max(1),
        "点名并发没有派满核数：{concurrent}"
    );
    // 目录卷两路恒相同：幂等那一道与两遍拿的是同一个数（11 号票不动目录卷这一路）。
    assert_eq!(concurrent.fingerprint, concurrent.readers, "{concurrent}");
    assert_eq!(serial.fingerprint, serial.readers, "{serial}");
    // 探到的介质与点名无关：三趟说的是同一块盘。
    assert_eq!(automatic.medium, serial.medium);
    assert_eq!(automatic.medium, concurrent.medium);
    // 报告那句话里点得出是谁定的这个数。
    assert!(serial.to_string().contains("--io-mode"), "{serial}");
    assert!(!automatic.to_string().contains("--io-mode"), "{automatic}");
}

/// 一次运行里，两个卷各拿各的读取计划——不是一趟只判一次（ADR 0009 决定第 2 条）。
///
/// 真实机器上只有一块盘，两个路径的**介质**必然相同；分得开的是另一维：**归档卷的两遍**
/// 恒串行（幂等那一道不在此列，11 号票）。点名并发之后两个卷仍给出不同的答案，
/// 「按卷判定」这件事因此在 seam 上看得见。
#[test]
fn two_volumes_in_one_run_each_carry_their_own_read_plan() {
    let space = Workspace::new();
    let directory = long_volume(&space, "volume-a");
    let mut archive = space.cbz("volume-b");
    for index in 0..8 {
        archive.page(
            &format!("{index:03}.png"),
            &fixtures::full_bleed_gradient(TOUCHING),
        );
    }
    let archive = archive.write();

    let report = tonefit::run(&Request {
        io_mode: IoMode::Concurrent,
        ..fixtures::request(&space, [directory.path(), archive.as_path()])
    })
    .expect("处理应当成功");

    assert_eq!(report.volumes.len(), 2);
    let (directory, archive) = (&report.volumes[0].io, &report.volumes[1].io);
    assert_eq!(directory.readers.chosen_by, ChosenBy::Named);
    // 归档卷的**两遍**点名并发也改不了：一个 ZipArchive 就是一个游标。
    assert_eq!(archive.readers.chosen_by, ChosenBy::ArchiveScan);
    assert_eq!(archive.readers.count, 1);
    assert!(archive.to_string().contains("顺序扫"), "{archive}");
    // 幂等那一道不吃这一条：它各开各的句柄，点名并发就真派得动（11 号票）。
    // 断言的是「与目录卷拿同一个数」，不是一个写死的数——派几条由核数定。
    assert_eq!(archive.fingerprint, directory.readers, "{archive}");
    // 报告一行里两路分得开。单核机器上并发与串行本就分不开，那里这一句退成「串行」。
    assert!(archive.to_string().contains("幂等那一道"), "{archive}");
}

/// 归档卷换一种读法重跑，这一卷**照旧被跳过**（11 号票）。
///
/// 指纹不进报告，在 seam 上量「两趟的指纹逐字节相同」因此只有这一种形式：
/// 跳过要四项依据全对，卷级源哈希是其中一项。两趟的幂等那一道一趟串行、一趟并发，
/// 而它仍然对得上——**并行的是解，不是喂**。两个方向各来一趟：
/// 串行写下的指纹并发读得回，并发写下的串行也读得回。
#[test]
fn an_archive_is_still_skipped_when_the_fingerprint_pass_changes_how_it_reads() {
    for (first, again) in [
        (IoMode::Serial, IoMode::Concurrent),
        (IoMode::Concurrent, IoMode::Serial),
    ] {
        let space = Workspace::new();
        let mut archive = space.cbz("volume-a");
        for index in 0..24 {
            archive.page(
                &format!("{index:03}.png"),
                &fixtures::full_bleed_gradient(TOUCHING),
            );
        }
        archive.file("ComicInfo.xml", b"<ComicInfo/>");
        let archive = archive.write();

        let plan = |mode: IoMode| Request {
            io_mode: mode,
            ..fixtures::request(&space, [archive.as_path()])
        };
        let done = tonefit::run(&plan(first)).expect("头一趟应当成功");
        let written = fixtures::fingerprint(&done.volumes[0].output);
        let rerun = tonefit::run(&plan(again)).expect("重跑应当成功");

        let skipped = &rerun.volumes[0];
        assert_eq!(
            skipped.verdict,
            Some(VolumeVerdict::Skipped { page_count: 24 }),
            "{first:?} 写下的指纹，{again:?} 读回来对不上"
        );
        // 跳过的依据一项不少：一页都没解码，输出一个字节都没动。
        assert_eq!(skipped.decodes, 0, "跳过的卷还是解码了");
        assert_eq!(
            fixtures::fingerprint(&skipped.output),
            written,
            "跳过的那一趟动了输出"
        );
    }
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
            // 门不成立那一支只在 fit-inside 上走得到（页几何批 01 号票）：
            // 以高为准让每一页的高都等于面板高，一条边永远贴着。
            fit: FitMode::Inside,
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
            // 同上：拒绝那条路只在 fit-inside 上打得着。
            fit: FitMode::Inside,
            ..fixtures::request(&space, [volume.path()])
        })
        .expect_err("门不成立时点名抖动该被拒");

        let said = format!("{error:#}");
        assert!(said.contains("003.png"), "第 {attempt} 趟：{said}");
        assert!(!said.contains("009.png"), "第 {attempt} 趟指错了页：{said}");
    }
}

/// 记账用的进度观察者：管线报的每一步都落在这里。
///
/// 本文件只问「预告了多少步、真走了多少步、收没收尾」这三件事，别的事件一概不记——
/// 事件流本身的形状在 `tests/events.rs` 里测。
#[derive(Clone, Default)]
struct Tally(Arc<Counts>);

#[derive(Default)]
struct Counts {
    /// 这一趟开始时预告的**全局**总步数（预扫算出来的那个数）。
    named: AtomicU64,
    /// 每个卷开始时预告的步数，按开始顺序。
    started: Mutex<Vec<u64>>,
    /// 实际走过的步数总和。
    advanced: AtomicU64,
    /// 收摊过几次。
    finished: AtomicUsize,
}

impl Progress for Tally {
    fn observe(&self, event: Event<'_>) -> Instruction {
        match event {
            Event::RunStarted { steps, .. } => {
                self.0.named.store(steps, Ordering::Relaxed);
            }
            Event::VolumeStarted { steps, .. } => {
                self.0.started.lock().expect("记账没有中毒").push(steps);
            }
            Event::Stepped { .. } => {
                self.0.advanced.fetch_add(1, Ordering::Relaxed);
            }
            Event::VolumeFinished { .. } => {
                self.0.finished.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        Instruction::Continue
    }
}

impl Tally {
    /// 开工那条事件预告的全局总步数。
    fn named(&self) -> u64 {
        self.0.named.load(Ordering::Relaxed)
    }

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

/// 长任务报得出进度（spec 的 story 30），而真走的步数不越过预告。
///
/// 三段各占多少：幂等那一道读全部成员、第一遍走每一页、第二遍写全部**输出**成员。
/// 24 页 + 1 个透传文件。
///
/// **预告是上界，不是承诺**（`CONTEXT.md` 的《进度》）：拆分开着时一张源页最多产出两张
/// 输出页（页几何批 04 号票），而几张要解了像素才知道——第二段因此按 24×2 + 1 预告。
/// 这一卷一张跨页都没有，真走的是 25 + 24 + 25 = 74。
///
/// 拆分**关着**时预告仍然精确：N 恒 1，第二段数得准。那一半钉的正是「进度条不会
/// 停在某个百分比上再也不动」——上界那一侧钉的只能是「不越过」。
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

    assert_eq!(
        tally.started(),
        vec![25 + 24 + (24 * 2 + 1)],
        "预告的是上界"
    );
    assert_eq!(tally.advanced(), 25 + 24 + 25);
    assert!(
        tally.advanced() <= tally.started()[0],
        "走过的步越过了预告的上界"
    );
    assert_eq!(tally.finished(), 1);

    // 拆分关着：预告与真走的对得上，一步不差。
    let exact = Workspace::new();
    let volume = long_volume(&exact, "volume-a");
    let tally = Tally::default();
    tonefit::run(&Request {
        split: tonefit::SplitRule {
            on: false,
            ..tonefit::SplitRule::default()
        },
        progress: Some(ProgressSink::new(tally.clone())),
        ..fixtures::request(&exact, [volume.path()])
    })
    .expect("处理应当成功");
    assert_eq!(tally.started(), vec![25 + 24 + 25]);
    assert_eq!(tally.advanced(), 25 + 24 + 25);
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
///
/// 全局那个数**照样按上界预告**（会话批 03 号票）：预扫只列成员，判不出这一卷会不会命中幂等——
/// 那要读回上一趟写在输出里的记录，是幂等那一道自己的事。差额归谁处理，
/// 见 `tonefit::Event::RunStarted` 的 `steps`。
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
    assert_eq!(
        tally.started(),
        vec![25 + 24 + (24 * 2 + 1)],
        "预告的是上界"
    );
    assert_eq!(tally.advanced(), 25, "跳过的卷只该走幂等那一道");
    assert_eq!(tally.finished(), 1, "跳过的卷没有收尾");
    assert_eq!(
        tally.named(),
        tally.started()[0],
        "全局那个数没按上界预告：预扫判不出这一卷会命中幂等"
    );
}

/// 点名很多归档卷跑一整趟：**跑得动**，而同时开着的句柄不随点名的卷数长
/// （`p2-loose-ends/12`）。
///
/// 整笔句柄账在 `source::Reader` 的《一趟同时开着几个句柄》里，这一条钉的是里面最容易
/// 失守的一格——**正在处理的那一卷是 1，不是点名的卷数**。另两格各有自己的钉子：
/// 带乘数的那一格是 `src/read.rs` 的 `a_concurrent_read_holds_one_reader_per_worker`，
/// 预扫那一半是 `src/survey.rs` 的 `a_survey_keeps_no_archive_open`。
///
/// 问的时刻是**每一卷开工那一条事件**：那一刻上一卷已经收摊、这一卷还没按路径重开，
/// 因此整趟任何一处攥住某个归档不放，都会在这里露馅。
///
/// 「还攥着没有」照 `a_survey_keeps_no_archive_open` 的办法两个平台各答一半，两句都在：
///
/// - Linux 上 `/proc/self/fd` 直接数得出来（见 [`open_files_under`]）。
/// - Windows 上问不出句柄数，改问一件等价的事：**装着这些归档的目录还改不改得动名**。
///   那个平台上目录里只要还开着一个文件，重命名就会被拒。改完当场改回来，
///   这一卷接着按原路重开。
///
/// 两句在对方的平台上都恒成立，因此谁也不会误报。
///
/// **同一对问题在「开遍」那条事件上再问一遍，那是阳性对照**：那一刻这一卷正开着，
/// 两句都该报「还开着」。少了它，这条用例会在「开工那条事件挪到重开之后」这类改动下
/// **静默失去检出力**——问的时刻不对了，而两句照旧一片安静。
///
/// 两个时刻都出自 `run` 那条线程（开工与开遍都由 `process_volume` 自己发）。
/// **不问「走完一步」那一条**：它从计算线程上报出来、同一卷内可能并发到达
/// （`tonefit::Event::Stepped`），在那里改名会与自己撞车。
#[test]
fn many_archive_volumes_never_hold_more_than_the_one_being_processed() {
    /// 点名这么多卷。这条性质两个卷就问得出来（第二卷开工时露馅），取几十个是为了让
    /// 「不随卷数长」这句话在读的人眼里也站得住——处理范围本来就是用户点名的子集
    /// （ADR 0009 决定第 1 条）。
    const VOLUMES: usize = 32;

    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir(&library).expect("建库目录");
    let inputs: Vec<PathBuf> = (0..VOLUMES)
        .map(|n| {
            let mut cbz = fixtures::Cbz::new(library.join(format!("第{n:02}话.cbz")));
            cbz.page("001.png", &fixtures::full_bleed_gradient(fixtures::TINY));
            cbz.write()
        })
        .collect();

    let watch = Handles::new(&library);
    let report = tonefit::run(&Request {
        // 几十卷各走一整趟管线，因此拿这批夹具里最便宜的一张页：`TINY` 配 fit-inside
        // 不放大、`full_bleed_gradient` 那圈墨边让裁边也不起作用，每一卷于是只剩
        // 「开卷、读、写出」这几笔——本条问的正是它们，不是像素。
        fit: FitMode::Inside,
        progress: Some(ProgressSink::new(watch.clone())),
        ..fixtures::request(&space, inputs.iter().map(PathBuf::as_path))
    })
    .expect("几十个归档卷该跑得动");

    assert_eq!(report.volumes.len(), VOLUMES, "点名的卷没有全跑完");

    let between = &watch.0.between;
    assert_eq!(between.asked(), VOLUMES, "不是每一卷开工时都问过一次");
    assert_eq!(between.open(), 0, "开工那一刻还开着别的卷的归档");
    assert_eq!(
        between.refused(),
        0,
        "开工那一刻装着这些归档的目录改不动名，说明还开着其中某个文件"
    );

    // 阳性对照：开一遍那一刻这一卷正开着，两句各在自己的平台上都该报「还开着」。
    let during = &watch.0.during;
    assert!(during.asked() >= VOLUMES, "每卷至少开一遍，怎么会没问到");
    if open_files_under(&library).is_some() {
        assert!(
            during.open() >= 1,
            "开一遍那一刻也数不出开着的归档：`/proc/self/fd` 那一句已经失去检出力"
        );
    } else {
        assert_eq!(
            during.refused(),
            during.asked(),
            "开一遍那一刻目录竟改得动名：重命名那一句已经失去检出力"
        );
    }
}

/// 数一数这个进程还开着几个落在 `root` 底下的文件。
///
/// 与 `src/survey.rs` 的同名函数逐字相同，而两者分处两个 crate（那一份在库的
/// `#[cfg(test)]` 里），共享不了——这一份重复是认下来的。
///
/// 只有 Linux 答得出（`/proc/self/fd`），别的平台回 `None`——那边由重命名那一句去问。
/// 按路径过滤，因此同一个测试二进制里别的用例开着的文件干扰不到它。
fn open_files_under(root: &Path) -> Option<usize> {
    let held = std::fs::read_dir("/proc/self/fd").ok()?;
    Some(
        held.filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
            .filter(|target| target.starts_with(root))
            .count(),
    )
}

/// 在两个时刻各问一次「这些归档还有开着的没有」：每一卷开工那一条事件，与走一步那一条。
#[derive(Clone)]
struct Handles(Arc<Watch>);

struct Watch {
    /// 装着点名的那些归档的目录。
    library: PathBuf,
    /// 开工那一刻的答案。这一趟要的就是它：一个都不该开着。
    between: Answers,
    /// 开一遍那一刻的答案。阳性对照：那一刻这一卷正开着。
    during: Answers,
}

/// 一个时刻上两个平台各答的那一半。
#[derive(Default)]
struct Answers {
    /// 问过几次。
    asked: AtomicUsize,
    /// Linux 上数出来的峰值。数不出来的平台上恒 0，见用例文档。
    open: AtomicUsize,
    /// 目录改不动名的次数。改得动名的平台上恒 0。
    refused: AtomicUsize,
}

impl Answers {
    /// 问一次，两句都问，答案记进自己这三格。
    fn ask(&self, library: &Path) {
        self.asked.fetch_add(1, Ordering::Relaxed);
        if let Some(held) = open_files_under(library) {
            self.open.fetch_max(held, Ordering::Relaxed);
        }
        // 改名与改回来夹在这一条事件里：观察者不返回，被测的那一趟就走不下去。
        // 这两条事件都出自 `run` 那条线程，因此这两句之间没有第二个观察者在跑
        // （「走完一步」那一条不是，见用例文档）。
        let moved = library.with_file_name("改过名");
        match std::fs::rename(library, &moved) {
            Ok(()) => std::fs::rename(&moved, library).expect("改回原名"),
            Err(_) => {
                self.refused.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::Relaxed)
    }

    fn open(&self) -> usize {
        self.open.load(Ordering::Relaxed)
    }

    fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }
}

impl Handles {
    fn new(library: &Path) -> Self {
        Self(Arc::new(Watch {
            library: library.to_path_buf(),
            between: Answers::default(),
            during: Answers::default(),
        }))
    }
}

impl Progress for Handles {
    fn observe(&self, event: Event<'_>) -> Instruction {
        match event {
            Event::VolumeStarted { .. } => self.0.between.ask(&self.0.library),
            Event::PassStarted { .. } => self.0.during.ask(&self.0.library),
            _ => {}
        }
        Instruction::Continue
    }
}
