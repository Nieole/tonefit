//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外是三个 seam，其余全部是内部实现。
//!
//! [`run`] 是主 seam：所有模式走同一个入口，CLI 是它之上的薄层，只负责把命令行参数拼成
//! [`Request`]、把 [`Report`] 渲染成文字。
//!
//! [`score`] 是第二个 seam：判据的纯函数形态，数值与性质测试、标定工具直接调它。
//! 它周边的类型——[`Reference`]、[`Score`]、[`GrayImage`]、[`Candidate`]、[`quantize`]——
//! 一并公开，判据的调用方要拿它们拼出参照与候选。
//!
//! [`write_calibration_chart`] 是第三个：标定图。它不并进主入口——不读源、不走管线、
//! 不判定，只按一个 [`Profile`] 画出一张图并**无损写到点名的那个文件上**。
//! 量具与被处理的页走的不是同一条路。

mod cache;
mod calibrate;
mod color;
mod crop;
mod decide;
mod decode;
mod encode;
mod envelope;
mod geometry;
mod gray;
mod interlock;
mod medium;
mod metadata;
mod metric;
mod profile;
mod progress;
mod quantize;
mod read;
mod report;
mod request;
mod resample;
mod sink;
mod source;
mod spread;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use rayon::prelude::*;

pub use cache::{CacheBudget, CacheUsage};
pub use color::PageColor;
pub use crop::{Crop, InkRule, ink_rule};
pub use decide::{CandidateScore, Reason, Verdict};
pub use decode::Salvage;
pub use envelope::Envelope;
pub use geometry::{FitMode, GeometryGate, Size, max_target_pixels};
pub use gray::GrayImage;
pub use interlock::{Interlock, Voice};
pub use medium::{ChosenBy, IoMode, IoPlan, Medium};
pub use metric::{Aggregation, Composition, Reference, Score, aggregation, composition, score};
pub use profile::{Panel, Profile, Threshold, ThresholdSource};
pub use progress::{Event, Instruction, Pass, Progress, ProgressSink};
pub use quantize::{BitDepth, Candidate, Dither, quantize};
pub use report::{
    PageBranch, PageOutcome, PageReport, Processed, Report, VolumeReport, VolumeTiming,
    VolumeVerdict,
};
pub use request::{Mode, Request};
pub use resample::{Filter, Scaling};
pub use spread::{Cut, Gutter, ReadingOrder, Side, SplitRule, SplitThreshold};

use color::ColorImage;
use metadata::{Fingerprint, Origin, PageRecord, Record, Recorder};
use sink::Sink;
use source::{Member, Volume};
use spread::Split;

/// 画一张标定图并写到 `out`，父目录不在就建出来。
///
/// 尺寸恒等于面板分辨率：图要在真机上 1:1 显示才答得准。
/// 它一次上机答两件事，先后印在图内——那两件事与为什么合在一张图上，见 `calibrate` 的模块文档。
///
/// **图本身不经过位深判定**：它是量具，不是被处理的页——判据、上包络、抖动一概不碰它，
/// 像素以 8 位工作精度原样交给编码器，写出的是无损 PNG。自描述元数据也不写：
/// 记录说的是一页的判定与幂等依据（见 `metadata`），标定图两样都没有。
///
/// 落盘在库内完成，命令行与会话共用这一个调用（加固批 12 号票）：出图这件事从头到尾只有一份，
/// 界面层两边都不必自己建目录、自己写文件。写不出去时回的是 `Err`——盘满、
/// 父目录建不了都在里面，调用方接住它照自己的方式说，不必崩掉一整个会话。
///
/// 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`）。
pub fn write_calibration_chart(profile: &Profile, out: &Path) -> Result<()> {
    calibrate::write_chart(profile, out)
}

/// 处理点名的若干卷，产出设备优化副本。源卷只读。
pub fn run(request: &Request) -> Result<Report> {
    // 整趟的表从这里开始掐：开工前那几道检查也要摸文件系统，摊在计时之外
    // 只会让报出来的总耗时比调用方自己在外面掐的那个小一截（加固批 11 号票）。
    let started = Instant::now();
    if request.inputs.is_empty() {
        bail!("处理范围为空：至少点名一个卷（ADR 0009：处理点名的子集）");
    }
    ensure_the_overrides_leave_a_candidate(request)?;
    for input in &request.inputs {
        ensure_output_is_elsewhere(input, &request.output_root)?;
    }
    ensure_no_two_volumes_share_an_output(request)?;
    // 介质**按路径**探测，一次运行共用一份缓存（ADR 0009 决定第 2 条，见 `medium`）：
    // 同一趟里源卷可能在仓库盘上、输出在系统盘上，逐卷各判各的，互不影响。
    let mut probes = medium::Probes::new();
    // 这一趟的事件流。闩活在这里——一次运行一份，`Request` 复用不到它
    // （见 [`progress::Events`] 的 `standing`）。
    let standing = progress::Standing::default();
    let events = progress::Events::new(request.progress.as_ref(), &standing);
    // 开工前那几道检查排在它之前：那几种失败一条事件都不发，调用方拿到的是错误本身。
    // 全局总步数将来也落在这一条上（03 号票：预扫）。
    events.run_started(request.inputs.len());
    let mut volumes = Vec::with_capacity(request.inputs.len());
    for input in &request.inputs {
        // **卷边界上的检查点**（ADR 0013 决定第 1 条）：收尾让当前卷跑完就停，
        // 而「当前卷跑完」正是这里——盘上因此只有完整的卷，下一趟幂等接着走。
        // 中止在这一道上与收尾同样停下；它自己那个**页边界**检查点与丢弃 `partial`
        // 由 04 号票落地（见 [`Instruction::Abort`]）。
        if events.standing() != Instruction::Continue {
            break;
        }
        let medium = probes.medium(input);
        volumes.push(process_volume(input, request, medium, events)?);
    }
    events.run_finished();
    Ok(Report {
        profile: request.profile.clone(),
        fit: request.fit,
        crop: request.crop,
        split: request.split,
        volumes,
        elapsed: started.elapsed(),
    })
}

/// 掐一段的表：跑一遍 `work`，把这一段的墙钟耗时写进 `segment`。
///
/// 写成一个函数而不是在调用处各写三行，为的是让「哪几段掐了表」在 [`process_volume`] 里
/// 一眼数得清：段与段不许重叠，而重叠一旦发生，[`VolumeTiming`] 里三段之和就会大于总耗时。
fn timed<T>(segment: &mut Duration, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = work();
    *segment = started.elapsed();
    value
}

/// 这一卷要走多少步（spec 的 story 30）。
///
/// 三段：幂等这一道读全部**源**成员，第一遍走每一张**源页**，第二遍写全部**输出**成员。
/// 源那一侧与输出那一侧不是同一个数——一个源页产出一到多张输出页（页几何批 03 号票），
/// 而几张由内容决定（有没有装订沟，页几何批 04 号票）。三段里只有第二段按输出那一侧算：
/// 读源与解源页都发生在切开之前。
///
/// 各段自己可能不在——`--no-metadata` 关掉第一段（那时既没有记录可写也没有依据可比），
/// dry-run 没有第三段（一个文件都不落盘）。因此按**这一趟真要做的事**算，
/// 而不是按一个固定的倍数：不然进度条会停在某个百分比上再也不动。
///
/// 幂等命中的卷会提前收摊，那时走过的只有第一段——预告的步数是**上界**，不是承诺，
/// 剩下的由 [`Event::VolumeFinished`] 一次性了结。
///
/// 第二段那个数**也是上界**，理由与上面那条不同：一个源页产出几张要解了像素才知道，
/// 而这一步在解码之前。取的是[每个源页最多几张](MAX_OUTPUTS_PER_SOURCE_PAGE)——
/// 一卷里真被切开的页越少，走过的步就越少。取下界会让进度条冲过头，
/// 而「预告是上界」这条规矩本来就在。
fn volume_steps(members: MemberCounts, request: &Request) -> u64 {
    let MemberCounts {
        source_pages,
        output_pages,
        extras,
    } = members;
    let fingerprint = if request.metadata {
        source_pages + extras
    } else {
        0
    };
    let write = if request.mode == Mode::Process {
        output_pages + extras
    } else {
        0
    };
    (fingerprint + source_pages + write) as u64
}

/// 一个卷这一趟要碰的成员数，源那一侧与输出那一侧分开数（页几何批 03 号票）。
///
/// 三个数绑成一个类型而不是三个相邻的 `usize` 参数：它们总是一同算出、一同传下去，
/// 而三个同型的裸数换了位置编译器一句话都不会说，[`volume_steps`] 却会当场少报或多报一整段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemberCounts {
    /// 源页数。幂等这一道读它们，第一遍走它们。
    source_pages: usize,
    /// 输出页数的**上界**。第二遍写它们——一个源页产出一到多张，切开发生在第一遍之内，
    /// 而几张由内容决定（页几何批 04 号票），因此这一步只给得出上界。
    output_pages: usize,
    /// 透传文件数。它不经切开，两侧数的是同一批。
    extras: usize,
}

/// 锁上这一卷的缓存。
///
/// 中毒了照样用：里面是这一卷的账本，而一条计算线程恐慌不该让其余每一条跟着恐慌——
/// 那会把一处失败放大成整趟失败，真正的恐慌还被「锁中毒了」这句话盖住。
/// 与读取层那道闸同一条规矩（见 `read` 的 `Throttle::lock`）。
fn lock(cache: &Mutex<cache::PageCache>) -> MutexGuard<'_, cache::PageCache> {
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 计算层这一趟摊得开多少条。
///
/// 取核数：rayon 的默认线程池就是照这个数建的，读取层无谓比它派得更多——读得再快，
/// 也没有更多的核去消化（见 `read`）。
fn cores() -> usize {
    num_cpus::get().max(1)
}

/// 隔离目录在输出根下的名字（12 号票：含失败页的卷输出到隔离目录）。
///
/// 名字用 ASCII：输出常常要经 MTP 或 FAT 搬到阅读器上，目录名少一分编码上的赌注是一分。
/// 下划线前缀买两件事——它不至于撞上一个真叫这个名字的卷，列目录时也排在最前面。
const ISOLATED_DIRECTORY: &str = "_isolated";

/// 处理一个卷：第一遍解码到判据，第二遍量化到写出，非图片成员原样搬过去。
///
/// 两遍之间隔着缓存（ADR 0005：解码一次，缓存缩放后的图）。第二遍的输入是第一遍存下的参照，
/// 源页因此只被解码一次——`VolumeReport::decodes` 是这条不变量看得见的形式。
///
/// 彩页在彩色 profile 下不走这条路：它在第一遍就缩放并编好，绕开缓存、判据与汇总
/// （ADR 0005 决定第 4 条，见 [`first_pass`]）。第二遍只把它按阅读顺序写出去。
///
/// dry-run 走同一条路，只是不建输出容器，第二遍也就没有可写的地方。
///
/// 两遍之前还有一道**幂等**：上一趟的输出还在、四项依据一项没变，这一卷就整个不做
/// （见 [`volume_fingerprint`]）。dry-run 也走这一道——它预告的是照做时会发生的事，
/// 而照做时会发生的正是「跳过」（spec 的 story 6、story 8）。
///
/// **卷的去处到第一遍走完才定得下来**（12 号票）：有失败页的卷整个进隔离目录，
/// 而哪一页失败要解过才知道。输出容器因此在第一遍之后才建——写出全在第二遍，
/// 早建一步只会让隔离的卷在干净的去处留下一个空壳。
///
/// **隔离的卷不被幂等跳过**：跳过只认干净的那个去处（见 [`can_skip`]）。这是有意的——
/// 那不是一份做完了的输出，而失败清单每一趟都要重新给得出来（spec 的 story 26）。
/// 代价是有坏页的卷每趟都重做一遍，直到坏页被修好。
///
/// `medium` 是这个源路径落在什么盘上（ADR 0009 决定第 2 条）。它在这里变成一份
/// [读取计划](IoPlan)：这一卷读几条、为什么是这个数，报告照它说。
fn process_volume(
    input: &Path,
    request: &Request,
    medium: Medium,
    events: progress::Events,
) -> Result<VolumeReport> {
    // 这一卷的表：总的从打开卷之前起算，三段各自掐（加固批 11 号票，见 [`VolumeTiming`]）。
    let started = Instant::now();
    let mut timing = VolumeTiming::default();
    let mut volume = source::open(input)?;
    // 这一卷的两个可能去处。哪一个作数要等第一遍走完才知道，另一个则可能留着上一趟的过期副本。
    let clean = volume.output_path(&request.output_root);
    let isolated = volume.output_path(&request.output_root.join(ISOLATED_DIRECTORY));
    // 这一卷的输出成员名此刻只预告得出**一对一那一套**：一个源页产出几张要解了像素才知道
    // （有没有装订沟，页几何批 04 号票），而这一步在解码之前。撞名因此查两遍——
    // 这一遍拦下与内容无关的那些（`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
    // 真正产出的那批名字等第一遍走完再查一遍。早查这一遍买的是**别白做一整卷**。
    ensure_one_member_per_output(&volume, &one_to_one_targets(&volume))?;
    let source_pages = volume.pages.len();
    let members = MemberCounts {
        source_pages,
        // 上界，不是承诺：一卷里真被切开的页越少，第二遍走过的步越少（见 [`volume_steps`]）。
        output_pages: source_pages * max_outputs_per_source_page(request),
        extras: volume.extras.len(),
    };

    let io = IoPlan::decide(medium, request.io_mode, volume.container, cores());
    let writes = request.mode == Mode::Process;
    events.volume_started(&volume.root, volume_steps(members, request));

    // `--no-metadata` 关掉记录，幂等的依据无处可写也无处可读，这一整道于是不在。
    //
    // 算指纹与拿它比是**同一段**（`CONTEXT.md` 的《管线》：算出本卷的指纹，与上一趟写在
    // 输出里的比）：比的那一半要开输出容器、逐成员读回记录，同样是真 I/O。摊到段外，
    // 「跳过一卷花在幂等上多久」就会少算一截，而那正是这个数存在的理由（加固批 11 号票）。
    // 命中时它是**上一趟写在输出里的那几张**的张数，不是这一步预告出来的数：
    // 一个源页产出几张由内容决定，而跳过的这一趟一个像素都不碰（见 [`can_skip`]）。
    let mut written_pages = None;
    let fingerprint = if request.metadata {
        events.pass_started(Pass::Fingerprint);
        timed(&mut timing.fingerprint, || -> Result<_> {
            let fingerprint = volume_fingerprint(&mut volume, request, &io, events)?;
            written_pages = can_skip(&clean, &volume, &fingerprint);
            Ok(Some(fingerprint))
        })?
    } else {
        // 掐表在这个 `if` 之内：整道不在时那一段是零，而不是一个「什么都没做」的很小的数。
        None
    };
    if let Some(output_pages) = written_pages {
        let report = VolumeReport {
            volume: volume.root,
            output: clean,
            // 跳过的卷是干净的，隔离目录里若还留着一份，那是上一趟坏页时写的。
            superseded: superseded(&isolated),
            pages: Vec::new(),
            source_pages,
            verdict: Some(VolumeVerdict::Skipped {
                page_count: output_pages,
            }),
            cache: CacheUsage::new(request.cache_budget),
            decodes: 0,
            io,
            // 两遍一遍都不走，三段里只有幂等那一段有数。
            timing: VolumeTiming {
                elapsed: started.elapsed(),
                ..timing
            },
        };
        // 跳过的卷照样报这一条：「跳过」在屏幕上不该长成「卡住」，
        // 而它带的那份报告与做了事的卷同形，攒报告的那一端不必分两种情形。
        events.volume_finished(&report);
        return Ok(report);
    }

    // dry-run 没有第二遍，缓存于是只记账不留页：用量照旧预告得出，临时文件一个不建。
    let retention = match request.mode {
        Mode::Process => cache::Retention::Keep,
        Mode::DryRun => cache::Retention::Account,
    };
    // 缓存与解码计数是计算层唯一共用的两样东西：一个要串起来（账本只有一本），
    // 一个是原子加。贵的那几步——解码、缩放、判据、压缩——全在锁外。
    let cache = Mutex::new(cache::PageCache::new(request.cache_budget, retention));
    let decoder = decode::Decoder::new();
    // 第一遍产出的是**输出页**：一个源页产出的那几张挨着排，卷内页序就是写出顺序。
    events.pass_started(Pass::First);
    let scored = timed(&mut timing.first_pass, || {
        first_pass(
            &mut volume,
            request,
            &cache,
            &decoder,
            fingerprint.as_ref(),
            &io,
            events,
        )
    })?;
    // 预告的张数与真产出的张数在这里第一次同时在手上。预告是**上界**（页几何批 04 号票：
    // 一个源页产出几张由内容决定），因此比的是区间而不是等号：下界是一张源页至少出一张，
    // 上界是每张都被切开。越出这个区间说明拆分与预告分了家，而那是一种静默的错——
    // 报告照出，进度条却要么冲过头、要么停在半路。
    debug_assert!(
        (members.source_pages..=members.output_pages).contains(&scored.len()),
        "第一遍产出 {} 张，而源页 {} 张、上界 {} 张",
        scored.len(),
        members.source_pages,
        members.output_pages
    );
    // 真正产出的那批成员名在这里第一次齐了：加了序号的名字可能撞上卷里本来就有的成员
    // （源里同时有 `001.jpg` 与 `001-1.png`），而那一撞要在写出第一个字节之前拦下。
    ensure_no_two_outputs_collide(&volume, &scored)?;

    let (verdicts, verdict) = summarize_volume(&scored, request);
    let uniform = uniform_size(&scored, request.profile.panel().resolution);
    // 有一页失败，整卷就去隔离目录；另一个去处留着的那一份这一趟碰都不碰。
    let (output, elsewhere) = if scored.iter().any(OutputPage::failed) {
        (isolated, clean)
    } else {
        (clean, isolated)
    };
    let superseded = superseded(&elsewhere);

    // dry-run 一个文件都不落盘，输出容器因此连建都不建（spec 的 story 6）。
    // 建容器与收尾改名一并掐在这一段里：它们是「写出」这件事的两头（加固批 11 号票）。
    if writes {
        // **续做的决策点就在这一句上**（ADR 0012 决定第 2 条）：汇总已经做完、
        // 第二遍还没开始。本票只把它报出去，「答收尾就停在这儿」由 06 号票落地。
        events.pass_started(Pass::Second);
        timed(&mut timing.second_pass, || -> Result<()> {
            let mut sink = Sink::create(&output, volume.container)?;
            let recorder = fingerprint
                .as_ref()
                .map(|fingerprint| Recorder::new(fingerprint, driver(verdict)));
            let encode = Encode {
                uniform,
                cache: &cache,
                recorder: recorder.as_ref(),
            };
            second_pass(&scored, &verdicts, &encode, &mut sink, events)?;
            for extra in &volume.extras {
                let bytes = volume.reader.read(extra)?;
                sink.write_extra(&extra.relative, &bytes)?;
                events.step();
            }
            sink.finish()
        })?;
    }

    let pages = scored
        .into_iter()
        .zip(verdicts)
        .map(|(page, verdict)| page.into_report(&output, verdict, uniform))
        .collect();
    // 缓存的用量在拼报告**之前**读回来：读它要抢那一把锁，而下一句就要把报告交给观察者，
    // 而观察者可能很久不返回（见 `progress` 的模块文档）。写在结构体字面量里的话，
    // 那个临时的 guard 要活到整条 `let` 语句结束——这一行把它掐在自己这一句里。
    let cache = lock(&cache).usage();
    let report = VolumeReport {
        volume: volume.root,
        output,
        superseded,
        pages,
        source_pages,
        verdict,
        cache,
        decodes: decoder.decodes(),
        io,
        timing: VolumeTiming {
            elapsed: started.elapsed(),
            ..timing
        },
    };
    events.volume_finished(&report);
    Ok(report)
}

/// 这一卷在另一个去处留着的上一趟输出，没有就是 `None`（12 号票的「过期副本」）。
///
/// 只问在不在，不去读它，也**不删它**：那是用户手上一份真实存在的输出，
/// 而 tonefit 在别处一律不做破坏性动作。删不删由用户定，报告负责让他知道有这么一份。
fn superseded(elsewhere: &Path) -> Option<PathBuf> {
    elsewhere.exists().then(|| elsewhere.to_path_buf())
}

/// 卷内统一的那个尺寸：失败页按它留白占位（12 号票：卷内尺寸保持一致）。
///
/// 取处理成了的那些页里**出现次数最多**的那个尺寸，并列时取先出现的。漫画卷内绝大多数页
/// 同一个尺寸，众数因此就是「这一卷看上去的样子」；取最大值会让一张跨页把整卷的占位页撑宽，
/// 取第一页则会被卷首的封面或彩页带偏。
///
/// 一页好页都没有的卷退到面板分辨率：卷内没有可参照的尺寸了，那就照这块面板的满幅出。
///
/// 数的是**输出页**：一个源页产出的那几张各有各的尺寸（页几何批 03 号票），众数因此在切开之后取。
fn uniform_size(pages: &[OutputPage], panel: Size) -> Size {
    let mut counted: Vec<(Size, usize)> = Vec::new();
    for size in pages.iter().filter_map(OutputPage::size) {
        match counted.iter_mut().find(|(seen, _)| *seen == size) {
            Some((_, count)) => *count += 1,
            None => counted.push((size, 1)),
        }
    }
    counted
        .into_iter()
        // 并列时留先出现的那个：`max_by_key` 留的是最后一个。
        .reduce(|best, next| if next.1 > best.1 { next } else { best })
        .map_or(panel, |(size, _)| size)
}

/// 一张纸白的页：失败页留在输出里的那个**占位页**。
///
/// 白而不是别的什么——占位页顶住页序与尺寸，但不冒充内容，也不该往页上添一笔本来没有的墨。
/// 它认得出来的地方在别处：这一卷在隔离目录里，这一页的记录写着 `failed`（见 `metadata`），
/// 报告里逐条列着原因。
fn placeholder(size: Size) -> GrayImage {
    let pixels = vec![u8::MAX; size.width as usize * size.height as usize];
    GrayImage::new(size, pixels)
}

/// 汇总：先逐页定档，再把它们收成卷级的一个基准档（ADR 0006：位深按卷取上包络并加迟滞）。
///
/// 夹在两遍之间——要看完整卷才做得了，而第二遍此刻已经不必回头碰源页（ADR 0005）。
/// 返回的逐页判定与 `pages` 等长同序，第二遍读的就是它。
///
/// **只有灰度路径上的页进来。**另外两种页没有判据曲线：彩色分支上的页不该有——ADR 0006
/// 决定第 5 条说彩页在彩色 profile 下「根本不进灰度上包络」；失败页则是没有可求判据的像素
/// （12 号票）。两者在返回的判定里都占位为 `None`，位置留着——第二遍与报告都按页序取。
///
/// **第一刀按几何门切**（ADR 0007 决定第 2 条）。门成立的页与门不成立的页候选集不是同一套：
/// 后者少了抖动那一维，而上包络取的是 (位深, 抖动模式) 这个组合——候选集不同的页
/// 排不进同一条序列。卷级那一层因此只在其中**一组**上做，主体取门成立的那一组；
/// 那一组一页都没有时才轮到另一组，摘一页是为了护着别人，而那时没有别人可护
/// （ADR 0007 决定第 5 条）。
///
/// 摘出去的那一组**不单独定档**：它们跟着卷级基准档的位深走、不低于它，抖动关掉
/// （ADR 0007 决定第 3 条）。门只拿走抖动，不拿走档次——让它们各按自己那条曲线定，
/// 位深那一维也会跟着逐页变，而它们并没有偏离卷内分布，摘它们的理由是几何。
/// 反过来只给基准档也不行：抖动被拿走之后同一档位深保真更差，那一页可能真的还要高一档。
///
/// **第二刀按页残缺切**（04 号票）。部分救回页有判据曲线，那条曲线却是在一页大半留白的图上
/// 求出来的，代表不了这一卷。它因此不进上包络，按自己那条曲线单独定档——与离群页同一个待遇
/// （ADR 0006 决定第 5 条），只是摘它的理由是页残缺，不是判据偏离。
/// 一页不剩地落在救回那一侧时一页都不摘：主体不能空着，与门那一刀同一条规矩
/// （也与 `envelope::summarize` 里「一页不剩地落到离群侧」同一条）。
///
/// **两刀落在同一页上时，门那一刀在外层**（ADR 0007 决定第 3 条）：既没解全、又贴不住面板的页
/// 拿的是「基准档的位深，不低于它，抖动关掉」，不是 04 号票那条「按自己那条曲线单独定档」。
/// 摘部分救回页的理由是它那条曲线不具代表性——一页大半留白，误差恒为零，判出来必偏低——
/// 而不具代表性的曲线更没有资格把这一页压到基准档以下。
///
/// 逐页定档也落在这里，而不在第一遍：摘出去的那两组都要拿卷级基准档当参照，
/// 而那一档要看完整卷才定得下来。
///
/// 两条出口走不到上包络，各有各的道理：`--per-page` 是用户点名要逐页最优（决定第 6 条），
/// 覆盖项裁到只剩一个候选是判定整个被顶掉、逐页已全是 `Override`——后者不是「被关掉」，
/// 而是逐页结果里根本没有分布可聚合。两者在报告里各说各的，见 [`VolumeVerdict`]。
///
/// 进来的是**输出页**，不是源页（页几何批 03 号票）：一个源页产出的那几张各有各的几何、各有各的
/// 判据曲线，卷级那一层因此在切开之后取——序号也都指进输出页那个序列，
/// 上包络的驱动页序号跟着（见 [`Envelope::driver`]）。
fn summarize_volume(
    pages: &[OutputPage],
    request: &Request,
) -> (Vec<Option<Verdict>>, Option<VolumeVerdict>) {
    // 灰度路径上那些页在 `pages` 里的序号。卷级的一切都只在它们身上做。
    //
    // 序号非带不可：卷级每一步都只在其中一部分页上做——彩页与失败页根本不在场，
    // 门不成立的页与部分救回页各自摘出去——手上那个序列与卷内页序早就不重合了。
    let gray: Vec<usize> = pages
        .iter()
        .enumerate()
        .filter(|(_, page)| page.scores().is_some())
        .map(|(index, _)| index)
        .collect();
    // 门先分组。两组的候选集不是同一套，混不得（见 [`Candidates`]）。
    let (holding, broken): (Vec<usize>, Vec<usize>) = gray
        .iter()
        .copied()
        .partition(|&index| pages[index].gate() == Some(GeometryGate::Holds));
    // 一页门成立的灰度页都没有时，不成立的那些页就是这一卷的主体，基准档由它们定出
    // ——那一档必然不抖（ADR 0007 决定第 5 条）。
    let (inside, outside) = if holding.is_empty() {
        (broken, Vec::new())
    } else {
        (holding, broken)
    };

    let mut verdicts: Vec<Option<Verdict>> = vec![None; pages.len()];
    // 一张灰度页都没有的卷没有候选可判：只装着彩页的、一页都没有的、整卷全失败的，都是这一支。
    let Some(&first) = inside.first() else {
        return (verdicts, None);
    };
    let scores = |index: usize| pages[index].scores().expect("灰度路径上必有判据曲线");

    let threshold = request.profile.threshold();
    // 「覆盖项裁到只剩一个候选」问的是**主体那一组**的候选集：门那两组不一样长，
    // 拿门不成立的页去问，答案会随卷里第一张灰度页碰巧是哪一种而变。
    let pinned = pinned(request, scores(first));
    // 逐页先各判各的。摘出去的两组都还用得上自己这一档：部分救回页直接用它，
    // 门不成立的页拿它跟基准档比出更严的那个（ADR 0007 决定第 3 条）。
    for &index in &gray {
        verdicts[index] = Some(decide::decide(scores(index), threshold, pinned));
    }

    if let Some(candidate) = pinned {
        return (verdicts, Some(VolumeVerdict::Override(candidate)));
    }
    if request.per_page {
        return (verdicts, Some(VolumeVerdict::PerPage));
    }

    // 上包络只在主体那一组的完好页上取（04 号票）。两条出口上不分这一刀：覆盖项顶掉了判定、
    // `--per-page` 关掉了卷级那一层，两种情形下都没有一个「卷级的档」可供谁去污染。
    // 摘出去的部分救回页留着逐页判定：`verdicts` 里已经是它了，不必再写一遍。
    let (body, salvaged): (Vec<usize>, Vec<usize>) = inside
        .iter()
        .copied()
        .partition(|&index| !pages[index].salvaged());
    // 一页不剩地落在救回那一侧时一页都不摘：主体不能空着。
    let body = if body.is_empty() { salvaged } else { body };

    let inputs: Vec<envelope::Page> = body
        .iter()
        .map(|&index| envelope::Page {
            scores: scores(index),
            decided: verdicts[index].expect("灰度页都判过了").candidate,
        })
        .collect();
    let envelope::Summary {
        envelope,
        verdicts: refined,
    } = envelope::summarize(&inputs, threshold).expect("主体非空");
    for (&index, verdict) in body.iter().zip(refined) {
        verdicts[index] = Some(verdict);
    }
    // 门不成立的页：跟着基准档的位深走、不低于它，抖动关掉（ADR 0007 决定第 3 条）。
    // 它们与 `body` 不相交，逐页那一档因此还在原处等着被读。
    for &index in &outside {
        let own = verdicts[index].expect("灰度页都判过了").candidate.bit_depth;
        verdicts[index] = Some(Verdict {
            candidate: Candidate::new(envelope.base.bit_depth.max(own), Dither::Off),
            reason: Reason::OutsideTheGate,
        });
    }
    // 驱动页的序号在上包络那一侧指进**主体页**的序列，报告里那个序号指进整卷的页。
    // 卷内混着彩页、门不成立的页或部分救回页时两者不重合，这一步把它换回去——不换，
    // 报告会指着另一页说「就是它定的档」。
    let envelope = Envelope {
        driver: body[envelope.driver],
        ..envelope
    };
    (verdicts, Some(VolumeVerdict::Envelope(envelope)))
}

/// 覆盖项裁到只剩一个候选时的那一个：判定被顶掉，判据说什么都不改变结果（spec 的 story 23）。
///
/// 「裁到只剩一个」与「有覆盖项」两条都要：`--gray-levels 2` 撞上几何门不成立同样只剩一个候选，
/// 但那一档是判出来的，不是被顶掉的——理由分得清，报告才解释得了它是怎么来的。
///
/// 反过来，只点了一维的覆盖项裁不到只剩一个：`--bit-depth 4` 而主体那一组的门开着时，
/// 抖动那一维还有得判，判据照旧说了算。
///
/// `scores` 取的是**主体那一组**里的一页（见 [`summarize_volume`]）。裁到只剩一个的
/// 覆盖项落在门不成立那一组上时，那一组的候选集必然也只剩同一个——门只拿走抖动，
/// 而剩下的那一个既然过得了门，它本来就不抖。
fn pinned(request: &Request, scores: &[CandidateScore]) -> Option<Candidate> {
    let overridden = request.bit_depth.is_some() || request.dither.is_some();
    match scores {
        [only] if overridden => Some(only.candidate),
        _ => None,
    }
}

/// 第一遍产出的一张**输出页**：它从哪一个源页来、写到哪儿去，有没有处理成，
/// 以及处理成了的话留下了什么。
///
/// 一个源页产出**一到多张**（见 [`split`]）。因此 `source` 会重复
/// ——同一源页切出来的几张都指着它——而 `target` 一定不重复
/// （见 [`ensure_one_member_per_output`]）。第一遍之后管线上处处按输出页数事：
/// 汇总、上包络的序号、第二遍的写出、报告里的逐页结果，一律以它为单位。
struct OutputPage {
    /// 这一张来自哪一个源页：卷根接上成员相对路径，报告与错误信息用它指人。
    source: PathBuf,
    /// 它在输出容器里的相对位置（见 [`output_name`]）。
    target: PathBuf,
    /// 它的**来路**：来自哪个源成员、在那一族里排第几、那一族共几张（页几何批 04 号票）。
    ///
    /// 与 [`source`](Self::source) 不是重复：那一项是**给人读的身份**（卷根接上相对路径，
    /// 报告与错误信息指人用它），这一项是**写进 tEXt 的索引**（卷内相对路径，转义成 ASCII，
    /// 带着那一族的位次）。幂等靠它把输出页反查回源页——一个源页产出几张由内容决定，
    /// 输出成员名因此在碰像素之前预告不出来（见 [`Origin`] 与 [`can_skip`]）。
    origin: Origin,
    outcome: Outcome,
}

/// 一页在第一遍的结局。
///
/// 与报告那一侧的 [`PageOutcome`] 同形而不同物：这里装的是**第二遍要用的东西**
/// （缓存序号、编好的字节），那里装的是报告要读的东西。两者各留各的，
/// 内部产物才不会跟着报告一路公开出去。
enum Outcome {
    /// 处理成了的一页：完好的，或者救回来一段的。
    Processed {
        size: Size,
        /// 这一页裁掉了多少白边（页几何批 02 号票）。裁边在适配之前，`size` 由裁完的尺寸算出。
        crop: Crop,
        /// `size` 是不是**兜底上界**改出来的（07 号票，见 [`FitMode::target`]）。
        ///
        /// 它跟着页走，不由报告那一侧按尺寸倒推：算目标尺寸的地方只有一处，
        /// 倒推一遍就是第二处——与几何门那一条同一个理由（见 [`Branch::Gray`] 的 `gate`）。
        backstopped: bool,
        /// 这一张是那一刀的产物：切在哪条装订沟上、是哪一侧。整页出的是 `None`（04 号票）。
        cut: Option<spread::Cut>,
        /// 这一张所属的源页够得上**跨页候选**吗（04 号票）。与 `cut` 一起才说得全
        /// 拆分那两级，见 [`PageReport::spread_candidate`]。
        spread_candidate: bool,
        scaling: resample::Scaling,
        color: PageColor,
        branch: Branch,
        /// 这一页救回了多少。整解出来的完好页是 `None`（04 号票，见 `decode`）。
        salvage: Option<Salvage>,
    },
    /// 失败页：字节读不出来，或者连完整尺寸都解不出来（12 号票）。
    ///
    /// 它在这里仍然占着自己那一格——页序不因为一页坏了就错位，
    /// 第二遍照样给它写一张卷内统一尺寸的白页。
    Failed { reason: String },
}

/// 一页在第一遍里走的那条分支，连同它留给第二遍的东西。
///
/// 两条分支留下的不是同一套：灰度路径留判据曲线与缓存序号，彩色分支留编好的字节
/// （ADR 0005 决定第 4 条）。
enum Branch {
    /// 灰度路径。
    Gray {
        scores: Vec<CandidateScore>,
        /// 这一页的几何门判定（ADR 0007 决定第 1 条：门逐页判）。
        ///
        /// 它跟着页走，不由汇总那一步按尺寸重算：判定几何门的地方只有一处
        /// （[`GeometryGate::of`]，ADR 0003 要求灰阶硬上界与抖动判定同源），
        /// 而重算一遍就是第二处。
        gate: GeometryGate,
        /// 这一页在缓存里的序号。
        ///
        /// 序号跟着页走，不由第二遍数数补出来：彩页在彩色 profile 下不进灰度缓存
        /// （ADR 0005 决定第 4 条）、失败页也不进，页序与缓存序因此不重合，
        /// 而重新数出来的序号会静默地把另一页的像素写到这一页的位置上。
        slot: usize,
    },
    /// 彩色分支：第一遍缩放并编好的 PNG 字节，等写出那一遍按阅读顺序落位。
    ///
    /// dry-run 没有写出那一遍，也就不编——一个字节都不留（spec 的 story 6）。
    Color { encoded: Option<Vec<u8>> },
}

impl OutputPage {
    /// 这一页的判据曲线。彩色分支与失败页上都没有——一条不量化，一条没解出来。
    fn scores(&self) -> Option<&[CandidateScore]> {
        match &self.outcome {
            Outcome::Processed {
                branch: Branch::Gray { scores, .. },
                ..
            } => Some(scores),
            _ => None,
        }
    }

    /// 这一页写出的尺寸。失败页没有自己的尺寸——它按卷内统一尺寸出，而那个数
    /// 恰恰是从这个函数的结果里算出来的（见 [`uniform_size`]）。
    fn size(&self) -> Option<Size> {
        match &self.outcome {
            Outcome::Processed { size, .. } => Some(*size),
            Outcome::Failed { .. } => None,
        }
    }

    /// 这一页为什么失败，没失败就是 `None`。
    ///
    /// 事件流报「一页失败了」那一条用它取原因（见 [`Compute::page`]）。
    fn failure(&self) -> Option<&str> {
        match &self.outcome {
            Outcome::Failed { reason } => Some(reason),
            Outcome::Processed { .. } => None,
        }
    }

    /// 这一页失败了吗。一卷里只要有一页答是，整卷就进隔离目录。
    ///
    /// 问的是 [`failure`](Self::failure)：「失败了吗」与「为什么失败」只有一个出处，
    /// 两处各自 `matches!` 一遍，将来多一种失败就会有一处忘了改。
    fn failed(&self) -> bool {
        self.failure().is_some()
    }

    /// 这一页的几何门判定。判定范围之外的页没有——彩色分支上的页不在范围内
    /// （ADR 0010 决定第 4 条），失败页连几何都没有。部分救回页在范围内（ADR 0007 决定第 1 条）。
    fn gate(&self) -> Option<GeometryGate> {
        match &self.outcome {
            Outcome::Processed {
                branch: Branch::Gray { gate, .. },
                ..
            } => Some(*gate),
            _ => None,
        }
    }

    /// 这一页是救回来的吗（04 号票）。答是的页不参与卷级上包络。
    fn salvaged(&self) -> bool {
        matches!(
            self.outcome,
            Outcome::Processed {
                salvage: Some(_),
                ..
            }
        )
    }

    /// 补上汇总定下的那个判定，就是报告要的一页。缓存序号与编好的字节都不进报告——
    /// 它们是管线内部的事。
    ///
    /// `output` 是这一卷的去处，接上这一张的成员名就是它写出去的位置。
    /// `uniform` 只对失败页说话：它写出去用的就是这个尺寸。
    fn into_report(self, output: &Path, verdict: Option<Verdict>, uniform: Size) -> PageReport {
        let output = output.join(&self.target);
        let (size, outcome) = match self.outcome {
            Outcome::Processed {
                size,
                crop,
                backstopped,
                cut,
                spread_candidate,
                scaling,
                color,
                branch,
                salvage,
            } => {
                let processed = Processed {
                    crop,
                    backstopped,
                    cut,
                    spread_candidate,
                    scaling,
                    color,
                    branch: match branch {
                        Branch::Gray { scores, gate, .. } => PageBranch::Gray {
                            scores,
                            verdict: verdict.expect("灰度路径上必有判定"),
                            gate,
                        },
                        Branch::Color { .. } => PageBranch::Color,
                    },
                };
                let outcome = match salvage {
                    Some(salvage) => PageOutcome::Salvaged {
                        page: processed,
                        salvage,
                    },
                    None => PageOutcome::Whole(processed),
                };
                (size, outcome)
            }
            Outcome::Failed { reason } => (uniform, PageOutcome::Failed { reason }),
        };
        PageReport {
            source: self.source,
            output,
            size,
            outcome,
        }
    }
}

/// 本次的两套候选：几何门成立的那一套，与门不成立的那一套。
///
/// 两套在碰卷之前就备好，页判出门之后现取一套（[`Candidates::for_gate`]）。
/// 门逐页判，一卷里两套都用得上——混排卷正是 06 号票要收的那个形态（ADR 0007 决定第 1 条）。
///
/// 门不成立那一套是成立那一套的**子集**：同样的位深，少了抖动那一维。
/// 「候选集全卷同一套」因此只在**一组之内**成立，而卷级那一层只在其中一组上做
/// （见 [`summarize_volume`]）。
struct Candidates {
    /// 门成立时的候选集。它非空——覆盖项把它裁空的话，整趟在碰卷之前就被拒了
    /// （见 [`ensure_the_overrides_leave_a_candidate`]）。
    holds: Vec<Candidate>,
    /// 门不成立时的候选集。覆盖项把它裁空时是 `Err`：`--dither fs` 撞上一页贴不住面板
    /// 就是这个局面。错误留到真撞上那一页时才报——门是**页**的事实，一卷里可能一页都不撞。
    broken: Result<Vec<Candidate>>,
}

impl Candidates {
    fn new(request: &Request) -> Result<Self> {
        Ok(Self {
            holds: candidates(request, GeometryGate::Holds)?,
            broken: candidates(request, GeometryGate::Broken),
        })
    }

    /// 门是这个结果的页该拿哪一套。
    ///
    /// 裁空那一支上**重说一遍**错误，而不是把它搬走：撞上门的页可能有好几张，
    /// 每一张都要指得出自己，而 `anyhow::Error` 复制不了。
    fn for_gate(&self, gate: GeometryGate) -> Result<&[Candidate]> {
        match gate {
            GeometryGate::Holds => Ok(&self.holds),
            GeometryGate::Broken => self.broken.as_deref().map_err(|error| anyhow!("{error:#}")),
        }
    }
}

/// 第一遍：读 → 解码 → **切开** → 逐张彩页识别 → 分流。
///
/// 灰度路径：转灰 → 几何与几何门 → 缩放 → 判据曲线，同时把参照存进缓存。
/// 彩色分支：几何 → 缩放 → 编码，不进缓存、不求判据（ADR 0005 决定第 4 条）。
///
/// **产出的是输出页，不是源页**（页几何批 03 号票）：一张源页读一次、解一次，切成一到多张
/// （见 [`split`]），此后每一张各走各的分支、各占一个缓存序号、
/// 各占报告里的一格。同一源页切出来的那几张挨着排，卷内的输出页序因此仍是阅读顺序。
///
/// **识别排在转灰之前**，因为转过之后就没有颜色可看了；也排在汇总之前，
/// 因为分流决定了哪些页进得了上包络（ADR 0006 决定第 5 条）。
/// 走哪条分支由**面板与页**共同决定：只有彩色面板上的彩页走彩色分支，
/// 黑白面板上的彩页转灰、和其它页走同一条路。
///
/// 判据两种模式都求值，dry-run 预告的就是照做时的那一档（spec 的 story 6）。
/// 覆盖了判定也照求：`--dry-run --bit-depth 2` 要说得清「你点的这一档判据是多少」。
/// 彩色分支上没有这回事——那条路径不量化，dry-run 因此连编码都省了。
///
/// **几何门在这一遍上逐页收口。**门是几何的、一页看得出来，而它只决定这一页
/// （ADR 0007 决定第 1 条）：算到哪一页就判哪一页的门，候选集随之定下，判据只在那一套上求。
/// 一页贴不住面板不再改变别的页求几个候选，收尾处因此不必回头统一裁一遍——
/// 而从前那一裁，正是「一页否决整卷」在实现上的落点。
///
/// **彩色分支上的页不在判定范围内。**门撑的是抖动与面板灰阶那道硬上界（ADR 0007、ADR 0003），
/// 两者都只作用在灰度路径上；彩页既不量化也不抖动，它的几何事实对那两件事没有说话的资格。
///
/// **部分救回页在范围内**：它的尺寸是文件头里的真尺寸，答得出「这一页会不会被下游再缩一次」，
/// 而它答的只是自己那一页。04 号票把它摘出去，是因为那时门对整卷只有一个结果；
/// 门改成逐页判之后那条理由不在了，口径把它收了回来（ADR 0007 决定第 1 条）。
///
/// **读不出、解不出的页在这里变成失败页**（12 号票），而不是让整卷的调用返回 `Err`。
/// 它同样不在判定范围内，理由比彩页还直白：它连尺寸都没有。判据与缓存也一样绕开——
/// 没有像素可求判据，也没有像素可缓存。它留下的只有一条原因，等第二遍给它留一张白页。
///
/// **读取与计算在这里分成两层**（13 号票，见 `read` 与 `medium`）：读取按介质定并发度，
/// 计算走 rayon 满核，两层之间是一道按在途字节背压的有界通道。页因此**乱序算完**，
/// 页序在收尾处按序号归位——除此之外，这一遍的产物与一页一页顺着做完全相同。
fn first_pass(
    volume: &mut Volume,
    request: &Request,
    cache: &Mutex<cache::PageCache>,
    decoder: &decode::Decoder,
    fingerprint: Option<&Fingerprint>,
    io: &IoPlan,
    events: progress::Events,
) -> Result<Vec<OutputPage>> {
    // 两套候选在碰卷之前备好，页判出门之后现取一套（见 [`Candidates`]）。
    let candidates = Candidates::new(request)?;
    // 页的身份先取出来：读取层要借走 `reader`，此后就没有一个完整的 `Volume` 可问了。
    let sources: Vec<PathBuf> = volume
        .pages
        .iter()
        .map(|page| volume.identity(page))
        .collect();
    let Volume { pages, reader, .. } = volume;
    let members: Vec<&Member> = pages.iter().collect();

    let compute = Compute {
        request,
        decoder,
        cache,
        fingerprint,
        candidates: &candidates,
        events,
    };
    let mut scored: Vec<(usize, Result<Vec<OutputPage>>)> =
        read::reads(reader, &members, io.readers, read::BUDGET)
            .par_bridge()
            .map(|read| {
                let index = read.index;
                // 成员表在这一层照旧读得到：读取层借的也是共享引用。
                // 输出成员名由相对路径推出（见 [`output_name`]）。
                let relative = &members[index].relative;
                (index, compute.page(&sources[index], relative, read.bytes))
            })
            .collect();
    // 计算层乱序完成，页序在这里归位。往后每一处「第 n 页」都指得回同一页。
    scored.sort_by_key(|(index, _)| *index);
    // 归位**之后**才短路取错，因此报出来的是序号最小的那一页出的错，不是最先撞上的那一页：
    // 换一次调度就换一句错误的报告等于没有报告。`--dither fs` 撞上几何门那一支尤其吃这一条,
    // 那一支上没有报告可看，错误里指的那一页是唯一的线索。
    // 代价是一页出错时整卷仍会算完，而这一支上整卷本来就要作废，省下的那点算力买不到什么。
    //
    // 摊平放在最后：一个源页产出的那几张挨着排（页几何批 03 号票），卷内的输出页序因此仍然是阅读顺序。
    scored
        .into_iter()
        .map(|(_, page)| page)
        .collect::<Result<Vec<_>>>()
        .map(|pages| pages.into_iter().flatten().collect())
}

/// 第一遍上每条计算线程共用的那一摊。
///
/// 装成一个结构体而不是一串参数，是因为它要整个被闭包借走：拆成六个参数，
/// 闭包的捕获清单就得逐个写一遍，而漏掉一个的报错在 rayon 那一层读起来毫无线索。
struct Compute<'a> {
    request: &'a Request,
    decoder: &'a decode::Decoder,
    /// 缓存的账本只有一本，因此非串起来不可。压缩在锁外做（见 `cache::compress`）。
    cache: &'a Mutex<cache::PageCache>,
    fingerprint: Option<&'a Fingerprint>,
    /// 两套候选集。这一页判出门之后现取一套（见 [`Candidates::for_gate`]）。
    candidates: &'a Candidates,
    events: progress::Events<'a>,
}

impl Compute<'_> {
    /// 算一张**源页**：解码 → 切开 → 每一张各自彩页识别、分流。
    /// 语义与顺着做时逐字相同，见 [`first_pass`]。
    ///
    /// 报到一步，不是几步：步按**源页**数（`CONTEXT.md` 的《进度》：第一遍走每一页），
    /// 而一张源页读一次、解一次，切成几张不改变这一遍的工作量。
    ///
    /// 失败页在这里就报出去，不等整卷跑完（09 号票：失败页出现的当场就在主区可见）。
    /// 报在这一层而不在造出它的那一层：坏字节与解不出来的图各从各的地方回来，
    /// 而两条路最后都汇到这个返回值上——报在汇合处，以后多一种失败也不会漏报。
    ///
    /// 两条报到都在解码与切分**之后**：这条线程此刻一把锁都没拿着（缓存那把在
    /// [`Compute::gray_page`] 里进出），而观察者可能很久不返回（见 `progress` 的模块文档）。
    fn page(
        &self,
        source: &Path,
        relative: &Path,
        bytes: Result<Vec<u8>>,
    ) -> Result<Vec<OutputPage>> {
        let pages = self.split_and_branch(source, relative, bytes)?;
        for page in &pages {
            if let Some(reason) = page.failure() {
                self.events.page_failed(&page.source, reason);
            }
        }
        self.events.step();
        Ok(pages)
    }

    /// 解一张源页，**分流**，再按裁边 → 判跨页 → 拆分 → 每半再裁 → 适配走下去。
    ///
    /// 分流排在切开**之前**，也只问一次（ADR 0005 决定第 1 条：读 → 解码 → 彩页识别 →
    /// 拆分/裁边）：彩不彩是**源页**的事实，一幅跨页画不会因为从中间切开就有一半不再是彩页。
    /// 走哪条分支由**面板与页**共同决定——只有彩色面板上的彩页走彩色分支。
    fn split_and_branch(
        &self,
        source: &Path,
        relative: &Path,
        bytes: Result<Vec<u8>>,
    ) -> Result<Vec<OutputPage>> {
        let read = bytes.and_then(|bytes| {
            self.decoder
                .decode(&bytes)
                .with_context(|| format!("解 {} 这一页", source.display()))
        });
        let (decoded, salvage) = match read {
            Ok(decoded) => (decoded.image, decoded.salvage),
            // 一张坏图不毁掉整卷（spec 的 story 24）：记下原因就走，
            // 第二遍拿卷内统一尺寸给它留一张白页，整卷进隔离目录。
            //
            // 失败页**恒产出一张**占位页：没有像素可切，切不出第二张来。
            Err(error) => {
                let placement = Placement::new(relative, 0, OUTPUTS_PER_FAILED_PAGE);
                return Ok(vec![placement.into_page(
                    source,
                    Outcome::Failed {
                        reason: format!("{error:#}"),
                    },
                )]);
            }
        };
        let color = color::identify(&decoded);
        let panel = self.request.profile.panel();
        if panel.color && color.is_color() {
            self.color_pages(source, relative, color::to_color(&decoded), color, salvage)
        } else {
            self.gray_pages(source, relative, gray::to_gray(&decoded), color, salvage)
        }
    }

    /// 灰度路径上一张源页产出的那几张输出页。
    ///
    /// 次序是**裁边 → 判跨页 → 拆分 → 每半再裁**（`crate::spread` 的模块文档）：
    /// 先裁再判，因为白边过宽的单页在裁之前宽高比会像跨页；每半再裁，因为装订沟那一侧的
    /// 白边是切开之后才露出来的。三段窗口叠成源页上的一块，报告只印那一个
    /// （见 [`Crop::then`]）。
    ///
    /// **没切开的那一支一个像素都不多搬**：整页那一张原样往下走，既不复制一遍，
    /// 也不白裁第二遍——一对一那条老路因此与本票落地之前逐字节相同。
    fn gray_pages(
        &self,
        source: &Path,
        relative: &Path,
        image: GrayImage,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<Vec<OutputPage>> {
        let request = self.request;
        let panel = request.profile.panel().resolution;
        let crop = Crop::of_gray(&image, request.crop, salvage);
        let image = crop.apply_gray(image);
        let split = Split::of_gray(&image, panel, request.split, salvage);
        let pieces: Vec<(GrayImage, Piece)> = match split.halves() {
            None => vec![(image, Piece::whole(crop, split))],
            Some(halves) => halves
                .iter()
                .map(|half| {
                    let piece = half.window().take_gray(&image);
                    let inner = Crop::of_gray(&piece, request.crop, salvage);
                    (inner.apply_gray(piece), Piece::half(crop, *half, inner))
                })
                .collect(),
        };
        let count = pieces.len();
        pieces
            .into_iter()
            .enumerate()
            .map(|(ordinal, (image, piece))| {
                self.gray_page(
                    source,
                    Placement::new(relative, ordinal, count),
                    image,
                    piece,
                    color,
                    salvage,
                )
            })
            .collect()
    }

    /// 彩色分支上一张源页产出的那几张输出页。次序与灰度那一侧逐字相同，
    /// 见 [`gray_pages`](Self::gray_pages)。
    ///
    /// 两条路各写一遍而不是收成一个泛型：收起来要给「一张页的像素」立一个 trait，
    /// 而两条路真正共用的只有那五行次序——次序本身的出处在 `crate::spread` 的模块文档里，
    /// 那是文字，不是代码。多一层抽象换回来的是同一句话说三遍。
    /// 裁边那一侧早已是这个形状（`Crop::of_gray` 与 `Crop::of_color` 两支）。
    fn color_pages(
        &self,
        source: &Path,
        relative: &Path,
        image: ColorImage,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<Vec<OutputPage>> {
        let request = self.request;
        let panel = request.profile.panel().resolution;
        let crop = Crop::of_color(&image, request.crop, salvage);
        let image = crop.apply_color(image);
        let split = Split::of_color(&image, panel, request.split, salvage);
        let pieces: Vec<(ColorImage, Piece)> = match split.halves() {
            None => vec![(image, Piece::whole(crop, split))],
            Some(halves) => halves
                .iter()
                .map(|half| {
                    let piece = half.window().take_color(&image);
                    let inner = Crop::of_color(&piece, request.crop, salvage);
                    (inner.apply_color(piece), Piece::half(crop, *half, inner))
                })
                .collect(),
        };
        let count = pieces.len();
        pieces
            .into_iter()
            .enumerate()
            .map(|(ordinal, (image, piece))| {
                self.color_page(
                    source,
                    Placement::new(relative, ordinal, count),
                    &image,
                    piece,
                    color,
                    salvage,
                )
            })
            .collect()
    }

    /// 彩色分支上的一张：几何 → 缩放 → 编码，不进缓存、不求判据（ADR 0005 决定第 4 条）。
    ///
    /// 进来的 `image` 已经裁过、可能切过（见 [`color_pages`](Self::color_pages)），
    /// `crop` 是那几段窗口叠起来的**源页上的一块**，报告印的就是它。
    fn color_page(
        &self,
        source: &Path,
        placement: Placement,
        image: &ColorImage,
        piece: Piece,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<OutputPage> {
        let request = self.request;
        // 兜底上界在 `FitMode::target` 里，两条分支因此共用同一道（07 号票）：
        // 彩色分支上一个目标像素更贵，越界的页在这条路上先撑不住。
        let fit = request
            .fit
            .target(image.size(), request.profile.panel().resolution);
        let size = fit.size();
        let (scaled, scaling) = resample::resize_color(image, size, request.filter)?;
        // dry-run 一个文件都不落盘，编出来的字节没人要。
        let record = self
            .fingerprint
            .map(|fingerprint| Record::color(fingerprint, &placement.origin, salvage));
        let encoded = match request.mode {
            Mode::Process => Some(
                encode::color_png(&scaled, record.as_ref())
                    .with_context(|| format!("编 {} 这一页", source.display()))?,
            ),
            Mode::DryRun => None,
        };
        Ok(placement.into_page(
            source,
            Outcome::Processed {
                size,
                crop: piece.crop,
                backstopped: fit.backstopped(),
                cut: piece.cut,
                spread_candidate: piece.candidate,
                scaling,
                color,
                branch: Branch::Color { encoded },
                salvage,
            },
        ))
    }

    /// 灰度路径上的一张：几何与几何门 → 缩放 → 判据曲线，同时把参照存进缓存。
    /// 进来的东西同 [`color_page`](Self::color_page)。
    fn gray_page(
        &self,
        source: &Path,
        placement: Placement,
        image: GrayImage,
        piece: Piece,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<OutputPage> {
        let request = self.request;
        let panel = request.profile.panel();
        let fit = request.fit.target(image.size(), panel.resolution);
        let size = fit.size();
        // 门在这里判，也只在这里判：这一页的候选集当场定下，判据只在那一套上求。
        // 门只决定这一页——同一卷里贴住面板的页照旧拿得到抖动那一维（ADR 0007 决定第 1 条）。
        let gate = GeometryGate::of(size, panel.resolution);
        let allowed = self
            .candidates
            .for_gate(gate)
            .with_context(|| format!("{} 这一页关上了几何门", source.display()))?;
        let (scaled, scaling) = resample::resize(&image, size, request.filter)?;
        let reference = Reference::new(panel, scaled);
        let scores = candidate_scores(&reference, allowed);
        let block = cache::compress(reference.image());
        let slot = lock(self.cache)
            .insert(block)
            .with_context(|| format!("缓存 {} 这一页", source.display()))?;
        Ok(placement.into_page(
            source,
            Outcome::Processed {
                size,
                crop: piece.crop,
                backstopped: fit.backstopped(),
                cut: piece.cut,
                spread_candidate: piece.candidate,
                scaling,
                color,
                branch: Branch::Gray { scores, gate, slot },
                salvage,
            },
        ))
    }
}

/// 一张输出页在**源页上是哪一块**：留下的那个窗口，加上拆分那两级各自的结果。
///
/// 三项绑成一个类型而不是各占一个参数：它们由同一段（裁边 → 判跨页 → 拆分 → 每半再裁）
/// 一起算出，一起传下去，一起落进报告。摊成三个参数之后，两个可空的同型参数换了位置
/// 编译器一句话都不会说，而报告里会静默地把一张页说成另一张的形状。
#[derive(Debug, Clone, Copy)]
struct Piece {
    /// 这一张在源页上留下的那一块——裁边、切开、每半再裁三段窗口叠起来的结果
    /// （见 [`Crop::then`]）。
    crop: Crop,
    /// 这一张是那一刀的产物吗；是的话，切在哪条装订沟上、是哪一侧。整页出的是 `None`。
    cut: Option<spread::Cut>,
    /// 这一张所属的源页够得上**跨页候选**吗（拆分两级判定的第一级）。
    candidate: bool,
}

impl Piece {
    /// 没切开的那一张：整页就是一块。它仍然可能是候选——**候选而没切开就是连续跨页**。
    fn whole(crop: Crop, split: Split) -> Self {
        Self {
            crop,
            cut: None,
            candidate: split.candidate(),
        }
    }

    /// 切出来的一半：外层裁边、这一刀、每半再裁，三段窗口叠成源页上的一块。
    /// 切得开的必然是候选。
    fn half(outer: Crop, half: spread::Half, inner: Crop) -> Self {
        Self {
            crop: outer.then(half.window()).then(inner),
            cut: Some(half.cut()),
            candidate: true,
        }
    }
}

/// 一张输出页在输出容器里的位置与它的**来路**。
///
/// 两者由同一组 (源成员, 第几张, 共几张) 算出，因此一同算出、一同传下去：
/// 分开算就是两个出处，而两处一旦对不上，幂等去找的名字与真写出的名字就错开了
/// ——报告照出，输出里却少了成员（页几何批 04 号票）。
struct Placement {
    /// 它在输出容器里的相对位置（见 [`output_name`]）。
    target: PathBuf,
    /// 它的来路，写进 tEXt（见 [`Origin`]）。
    origin: Origin,
}

impl Placement {
    fn new(relative: &Path, ordinal: usize, count: usize) -> Self {
        Self {
            target: output_name(relative, ordinal, count),
            origin: Origin::new(relative, ordinal, count),
        }
    }

    /// 配上这一张的结局，就是第一遍产出的一张输出页。
    fn into_page(self, source: &Path, outcome: Outcome) -> OutputPage {
        OutputPage {
            source: source.to_path_buf(),
            target: self.target,
            origin: self.origin,
            outcome,
        }
    }
}

/// 第二遍：灰度页从缓存读 → 量化 → 编码，彩页取第一遍编好的字节，失败页留一张白页，
/// 三者一同写出。不再碰源页（ADR 0005）。
///
/// **写出按阅读顺序**，彩页也在这一遍落位。ADR 0005 决定第 4 条原话是「第一遍即写出」，
/// 那一句管的是彩页**离开灰度管线的时刻**——不进缓存、不求判据、不进上包络，这三条这里都成立。
/// 写出的时刻另有一条约束压着它：归档卷的成员按写入顺序排，而页名的字典序与阅读顺序
/// 本来就对不上（`1.png` `2.png` `10.png`）。彩页在第一遍就写进归档，混排卷的成员顺序
/// 就变成「先全部彩页、再全部灰度页」，按归档顺序翻页的阅读器会跳着读。
/// 代价认下：编好的字节要在内存里等到这一遍，且不受 `--cache-budget` 约束
/// （详见 ADR 0010）——那是编码后的 PNG，比参照小。
///
/// **失败页也在这一遍占住自己那一格**（12 号票）：一张 `uniform` 尺寸的纸白页。
/// 少写一页会让页序错位、页数对不上，而那正是「一张坏图毁掉整卷」的另一种形态。
///
/// 这一遍出的错仍然是卷级的错，不再变成失败页：它们不是坏图，是磁盘、内存与输出容器出了事，
/// 换一页重试也躲不过去。
/// **量化与编码满核跑，写出仍按阅读顺序**（13 号票）。两件事之所以分得开：编一页是纯计算、
/// 每页各编各的，而写出有次序——归档卷的成员按写入顺序排，乱一位就得让阅读器跳着读
/// （理由与彩页为什么不在第一遍写出是同一条）。
///
/// 一批编完再写一批，批量取核数：编好的字节要等到轮到它才写得出去，这一批就是它们在内存里
/// 排队的长度，因此**有界**——一页 PNG 中位不到 1 MB（measurements 的《B 类位深实测》），
/// 满核也就十几 MB。不分批而是一口气全编，那一摊就随卷长，正是有界通道要拦的东西。
///
/// 走的是**输出页**那个序列（页几何批 03 号票）：一步一张输出页，成员名跟着页走
/// （[`OutputPage::target`]），不由这一遍按源页序数出来——一个源页产出几张时，
/// 数出来的那个序号会静默地把另一张的字节写到这一张的位置上。
fn second_pass(
    pages: &[OutputPage],
    verdicts: &[Option<Verdict>],
    encode: &Encode,
    sink: &mut Sink,
    events: progress::Events,
) -> Result<()> {
    let work: Vec<(&OutputPage, Option<Verdict>)> = pages
        .iter()
        .zip(verdicts)
        .map(|(page, verdict)| (page, *verdict))
        .collect();
    for batch in work.chunks(cores()) {
        let encoded: Vec<Cow<'_, [u8]>> = batch
            .par_iter()
            .map(|(page, verdict)| encode.page(page, *verdict))
            .collect::<Result<Vec<_>>>()?;
        for ((page, _), bytes) in batch.iter().zip(&encoded) {
            sink.write_page(&page.target, bytes)?;
            events.step();
        }
    }
    Ok(())
}

/// 第二遍上每条计算线程共用的那一摊，与第一遍的 [`Compute`] 同一个用意。
struct Encode<'a> {
    /// 失败页按它出（12 号票的卷内统一尺寸）。
    uniform: Size,
    cache: &'a Mutex<cache::PageCache>,
    recorder: Option<&'a Recorder<'a>>,
}

impl Encode<'_> {
    /// 一页写出去的那串字节。三种页各有各的来路，但出来的都是一页 PNG。
    ///
    /// 出的是 [`Cow`]：彩页的字节第一遍就编好了，这里**借**它而不是复制一份。
    /// 那一摊本来就不受 `--cache-budget` 约束（ADR 0010），再翻一倍不合适。
    fn page<'p>(&self, page: &'p OutputPage, verdict: Option<Verdict>) -> Result<Cow<'p, [u8]>> {
        let Self {
            uniform,
            cache,
            recorder,
        } = *self;
        let source = page.source.display();
        match &page.outcome {
            Outcome::Failed { .. } => {
                // 占位页按 1bit 编，不跟卷级基准档走。它不是一个**判定**——它没进过候选集、
                // 没求过判据，卷级那一档说的是「这一卷的内容要几档灰」，而这一页没有内容。
                // 位深是编码属性（`CONTEXT.md`），而整页只有一个取值时 1bit 恰好装得下它；
                // 换个更宽的档也写不出别的字节，编码器那一层照旧会挑最窄的（ADR 0004）。
                let record = recorder.map(|recorder| recorder.failed(&page.origin));
                encode::png(&placeholder(uniform), BitDepth::One, record.as_ref())
                    .map(Cow::Owned)
                    .with_context(|| format!("编 {source} 这一页的占位页"))
            }
            Outcome::Processed {
                branch: Branch::Color { encoded },
                ..
            } => Ok(Cow::Borrowed(
                encoded.as_deref().expect("照做的那一遍第一遍就编过彩页"),
            )),
            Outcome::Processed {
                branch: Branch::Gray { slot, .. },
                salvage,
                ..
            } => {
                let verdict = verdict.expect("灰度路径上必有判定");
                // 取页要动缓存那本账，因此在锁里；量化与编码在锁外——贵的是后两件。
                let reference = lock(cache)
                    .load(*slot)
                    .with_context(|| format!("从缓存取 {source} 这一页"))?;
                let quantized = quantize::quantize(&reference, verdict.candidate);
                let record =
                    recorder.map(|recorder| recorder.gray(&page.origin, verdict, *salvage));
                encode::png(&quantized, verdict.candidate.bit_depth, record.as_ref())
                    .map(Cow::Owned)
                    .with_context(|| format!("编 {source} 这一页"))
            }
        }
    }
}

/// 本次调用在这一卷上的幂等依据（ADR 0006：同一批 tEXt 字段兼作幂等依据）。
///
/// 源哈希在这里算，作用域是卷（为什么，见 ADR 0006 的《决定》末段）。
///
/// 这一遍**把源字节多读一遍**——它在第一遍解码之前，而第一遍还要再读一次。这笔成本换不掉：
/// 彩页在第一遍就编好并写进 tEXt（ADR 0010），那一刻卷级哈希必须已经齐了。
/// 换来的是命中时一趟都不用做——多读一遍字节，省掉的是整卷的解码、缩放、判据与编码。
/// `--no-metadata` 连这一遍都不读：那时既没有记录可写，也没有依据可比。
///
/// 它走的是与第一遍同一个[读取层](read)，因此在没有寻道惩罚的盘上这一遍也是并发读的。
/// 喂哈希那一端仍**严格按成员次序**——源哈希是有序的，乱一位整卷的指纹就变了，
/// 而读取层交付本来就有序（见 `read` 的模块头）。
fn volume_fingerprint(
    volume: &mut Volume,
    request: &Request,
    io: &IoPlan,
    events: progress::Events,
) -> Result<Fingerprint> {
    let Volume {
        pages,
        extras,
        reader,
        ..
    } = volume;
    let members: Vec<&Member> = pages.iter().chain(extras.iter()).collect();
    let mut hasher = metadata::SourceHasher::new();
    for read in read::reads(reader, &members, io.readers, read::BUDGET) {
        let relative = &members[read.index].relative;
        // 读不出字节的成员在这一遍不算失败：它在第一遍里才变成失败页（12 号票），
        // 而这一遍排在第一遍之前。这里把它记成「读不出来」照样喂进哈希——
        // 拦在这里，一个坏成员就会毁掉整卷，正是本票要拆掉的那件事。
        match &read.bytes {
            Ok(bytes) => hasher.member(relative, bytes),
            Err(_) => hasher.unreadable(relative),
        }
        events.step();
    }
    Ok(Fingerprint::new(request, hasher.finish()))
}

/// 这一卷可以跳过吗：上一趟的输出还齐着，且每一页都记着这份指纹（spec 的 story 8）。
///
/// **两件事都要问。**指纹只随页走，透传文件不带记录——只问指纹的话，
/// 有人从输出里删掉 ComicInfo.xml 之后这一卷会永远跳过，那个文件再也补不回来。
///
/// 一页读不出记录就整卷重做，不逐页续做：判定是卷级的（ADR 0006 决定第 3 条），
/// 补写的那几页会拿到一个由**当前**全卷算出的基准档，与旁边幸存的旧页对不上。
///
/// 一页都没有的卷永远不命中：记录随页走，没有页就没有地方放它。那样的卷每一趟
/// 都把透传文件重写一遍——它们本来就是逐字节照搬，重写一遍与跳过没有可观察的差别。
///
/// 一个源页产出的那几张输出页**每一张**都要带着这份指纹，少一张就重做。
///
/// # 名单从哪儿来（页几何批 04 号票）
///
/// 从前它由 `page_targets` 在碰像素之前预告出来，逐个去比。跨页拆分落地之后那条预告
/// 不成立了——一个源页产出几张由内容决定（有没有装订沟），而幂等的全部意义是在解码之前答完。
///
/// 方向因此反过来：名单从**上一趟写在输出里的记录**读回来。一张输出页自己说得出
/// 它来自哪个源成员、那一族该有几张（[`Origin`]），于是按源页逐族去探——
/// 先试一对一那个名字，不在就试切开的那一族，头一张说出总共几张，再把余下的逐个对上。
///
/// **「输出里少一张就该察觉」这条能力因此没有丢，而且比从前更严**：
/// 一族两张里删掉一张，剩下那一张仍写着「1/2」，第二张找不到，整卷重做
/// （`p0-hardening/03` 靠的正是这条能力）。缺了那个计数就只剩「至少有一张」，
/// 一张跨页被删掉半边会静默地留在输出里。
///
/// 命中时答的是**上一趟写在那儿的输出页数**，不是这一趟预告出来的数：
/// 那个数眼下只给得出上界（见 [`MemberCounts`]），而报告里印的那个要是真数。
fn can_skip(output: &Path, volume: &Volume, fingerprint: &Fingerprint) -> Option<usize> {
    if volume.pages.is_empty() {
        return None;
    }
    let mut written = sink::Written::open(output, volume.container)?;
    let mut pages = 0;
    for page in &volume.pages {
        pages += written_family(&mut written, &page.relative, fingerprint)?;
    }
    volume
        .extras
        .iter()
        .all(|extra| written.holds(&extra.relative))
        .then_some(pages)
}

/// 一个源页那一族输出页在上一趟的输出里齐不齐；齐就答它有几张，缺一张就是 `None`。
///
/// 两支：一对一那个名字在，就只此一张（记录自己也得说是 `1/1`——名字对上而记录说
/// 「共两张」的话，另一张要么被删了、要么是别的参数跑出来的）；不在，就按切开那一族探，
/// 头一张（`…-1.png`）的记录说出总共几张，剩下的逐个对上。
///
/// 名字怎么拼只有一个出处（[`output_name`]），两支拼的都是它。
fn written_family(
    written: &mut sink::Written,
    relative: &Path,
    fingerprint: &Fingerprint,
) -> Option<usize> {
    let matched = |record: Option<PageRecord>, ordinal: usize, count: usize| {
        record.is_some_and(|record| record.matches(fingerprint, relative, ordinal, count))
    };
    if let Some(record) = written.record_of(&output_name(relative, 0, 1)) {
        return record.matches(fingerprint, relative, 0, 1).then_some(1);
    }
    // 一对一那个名字不在。那这一族要么是切开的，要么根本没写出来——头一张说了算：
    // 它记着自己那一族共几张，而余下几张的名字由那个数推得出来。
    let first_of_many = output_name(relative, 0, MORE_THAN_ONE);
    let first = written.record_of(&first_of_many)?;
    let count = first.origin.as_ref()?.count();
    if count < MORE_THAN_ONE || !first.matches(fingerprint, relative, 0, count) {
        return None;
    }
    (1..count)
        .all(|ordinal| {
            matched(
                written.record_of(&output_name(relative, ordinal, count)),
                ordinal,
                count,
            )
        })
        .then_some(count)
}

/// 卷级上包络的驱动页序号，写进 tEXt 那句 `volume-p95, driven by page 087` 用它。
///
/// 另外三种卷级判定没有驱动页可指：覆盖项顶掉了判定，`--per-page` 关掉了卷级那一层，
/// 而跳过的卷根本走不到写出这一步。
fn driver(verdict: Option<VolumeVerdict>) -> Option<usize> {
    match verdict {
        Some(VolumeVerdict::Envelope(envelope)) => Some(envelope.driver),
        _ => None,
    }
}

/// 把这一页的参照与每个候选各比一遍。
///
/// 候选先裁再求值，顺序是 ADR 0003 定的：被裁掉的候选不进入判据。
/// 这里只出量，拿量去和阈值比在 `decide`。
fn candidate_scores(reference: &Reference, allowed: &[Candidate]) -> Vec<CandidateScore> {
    allowed
        .iter()
        .map(|&candidate| CandidateScore {
            candidate,
            score: metric::score(reference, &quantize::quantize(reference.image(), candidate)),
        })
        .collect()
}

/// 门是 `gate` 的页可用的候选集，由小到大。
///
/// 四道裁剪，全部发生在判据求值之前：位深按面板灰阶数裁（ADR 0003），抖动模式按几何门裁
/// （ADR 0007），`--bit-depth` 与 `--dither` 各再裁自己那一维。前两道是界，后两道是覆盖项，
/// 但作用方式是同一个——都只从候选集里拿走东西，谁都放不回被拿走的。
///
/// 裁空了就报错：面板显示不出来、或几何上到不了眼睛的那些候选，写出去也是白写，
/// 宁可当场拒绝也不静默照写。门那一维裁空的时候，拒绝的报出的是**哪一页**撞上的门
/// （见 [`Candidates::for_gate`]）。
fn candidates(request: &Request, gate: GeometryGate) -> Result<Vec<Candidate>> {
    let panel = request.profile.panel();
    let picked: Vec<Candidate> = Candidate::all(panel.gray_levels, gate)
        .into_iter()
        .filter(|candidate| {
            request
                .bit_depth
                .is_none_or(|bit_depth| candidate.bit_depth == bit_depth)
        })
        .filter(|candidate| {
            request
                .dither
                .is_none_or(|dither| candidate.dither == dither)
        })
        .collect();
    if picked.is_empty() {
        return Err(nothing_left_error(request, gate));
    }
    Ok(picked)
}

/// 覆盖项与面板对不对得上，在碰卷之前先问一次。
///
/// 几何门此刻还没有页可判，先当它成立：门那一侧裁空的候选集只有等到第一遍里
/// 真撞上那一页才拦得住（见 [`Candidates::for_gate`]）。
fn ensure_the_overrides_leave_a_candidate(request: &Request) -> Result<()> {
    candidates(request, GeometryGate::Holds).map(|_| ())
}

/// 覆盖项裁空了候选集的说法：指出是哪道界拦下的，以及那道界本身还有没有得动。
///
/// 两道界只有一道动得了：面板灰阶数走 `--gray-levels`（ADR 0003），几何门动不了——
/// 它是页的几何事实，不是一个可以放宽的档位。
fn nothing_left_error(request: &Request, gate: GeometryGate) -> anyhow::Error {
    let panel = request.profile.panel();
    let depths = BitDepth::candidates(panel.gray_levels);
    match request.bit_depth {
        Some(bit_depth) if !depths.contains(&bit_depth) => {
            let listed = depths
                .iter()
                .map(BitDepth::to_string)
                .collect::<Vec<_>>()
                .join("、");
            anyhow!(
                "{bit_depth} 越过了面板的 {} 级灰阶：这块面板上写得出的是 {listed}。\
                 真要写 {bit_depth}，先按实测用 --gray-levels 抬高上界",
                panel.gray_levels
            )
        }
        // 位深那一维过得去，裁空的只能是抖动那一维：几何门不成立，而 `--dither` 点了抖动。
        // 那正是互锁 ③，处置是维持拒绝（页几何批 05 号票）。
        _ => {
            debug_assert!(
                Interlock::dither_outside_the_gate(request.dither, gate),
                "候选集被裁空了，而两道界一道都没拦"
            );
            dither_outside_the_gate_error(request.fit)
        }
    }
}

/// 互锁 ③ 咬上时那条拒绝的说法（05 号票的处置 ③：**维持拒绝**）。
///
/// 规则那一句由 [`Interlock`] 自己说——同一句还要从 `--help` 里出来，措辞只有那一份。
/// 这里补的是**这一趟**才知道的那件事：适配方式那一侧还有没有出路。撞上的是哪一页
/// 由错误链外层带着（见 [`Candidates::broken`]）。
///
/// 两条路上说法不同，而且**两边都不许把话说满**——把话说满正是本票要改掉的毛病：
///
/// - **fit-inside 上**，`--fit height` 把这一页放大到面板高，门跟着成立（页几何批 01 号票）。
///   **但不是每一页都够得着这条出路**：宽高比极端到以高为准算出的目标尺寸越过
///   [兜底上界](FitMode::target)的页会被退回 fit-inside，换过去仍是这条拒绝（07 号票）。
///   那道例外要跟着说出来，不然用户照着敲一遍只会撞第二次。
/// - **以高为准上**根本没有出路：那条路上每一页的高都等于面板高，门恒成立；
///   走得到这里的只能是被兜底上界退回去的页——它已经是一张 fit-inside 的页了。
///   那时劝人换 `--fit height` 是**假话**，改说剩下的那两条路。
///
/// 这里判不出手上这一页是哪一种：错误在碰卷之前就备好（[`Candidates::new`]），
/// 那时没有页可量。**能做的是把话说全**——真要按页分岔，得把这条拒绝挪到判门的地方，
/// 那是另一件事（停车场 Q33）。
fn dither_outside_the_gate_error(fit: FitMode) -> anyhow::Error {
    let way_out = match fit {
        FitMode::Inside => format!(
            "改得动的是几何：--fit height 把这一页放大到面板高，门跟着成立。\
             够不着这条出路的只有一种页——宽高比极端到以高为准算出的目标尺寸越过 {} 像素、\
             会被兜底上界退回 fit-inside 的那种（07 号票）；那种页换过去仍是这条拒绝，\
             走下面那两条",
            max_target_pixels()
        ),
        FitMode::Height => format!(
            "适配方式这一侧已经没有出路：以高为准让每一页都贴住面板高，\
             走到这里的只能是目标尺寸越过 {} 像素、被兜底上界退回 fit-inside 出的那种页\
             （07 号票），门是在那张退回来的页上判的",
            max_target_pixels()
        ),
    };
    anyhow!(
        "{}。{way_out}。剩下两条路——不点 --dither fs（判据自己会替这一页把抖动关掉），\
         或换一张宽高比没这么极端的源页",
        Interlock::DitherOutsideTheGate
    )
}

/// 一个源页**最多**产出几张输出页：跨页从装订沟上切一刀，因此是两张（页几何批 04 号票）。
///
/// 真产出几张由内容决定，解完像素才知道（见 `crate::spread`）。这个数是碰像素之前
/// 给得出来的那个**上界**：进度步数照它预告（[`volume_steps`]），第一遍走完拿它对一次区间
/// （[`process_volume`]）。
///
/// 它不是「切几刀」的配置项：切点只有一个（装订沟），两半就是两张。
const MAX_OUTPUTS_PER_SOURCE_PAGE: usize = 2;

/// 拼「切开那一族里的第一张」这个名字时传的张数。
///
/// [`output_name`] 只分「一张」与「不止一张」两种写法，具体几张不进名字——
/// `001-1.png` 无论那一族有两张还是三张都是这个名字。幂等按名字探那一族时因此随便传一个
/// ≥2 的数就够（见 [`written_family`]）；名字里那个 `-1` 不是「共两张」的意思。
const MORE_THAN_ONE: usize = 2;

/// 这一趟一个源页最多产出几张。拆分关着时恒 1——那时预告是精确的，不是上界。
fn max_outputs_per_source_page(request: &Request) -> usize {
    if request.split.on {
        MAX_OUTPUTS_PER_SOURCE_PAGE
    } else {
        1
    }
}

/// 一张失败页产出几张占位页。**恒 1，而且与 [`MAX_OUTPUTS_PER_SOURCE_PAGE`] 无关**：
/// 它没有像素可切，切不出第二张来（12 号票：失败页以卷内统一尺寸留白占位，页序不断）。
///
/// 两者已经分家：拆分让好页的 N 随内容而变（页几何批 04 号票），而解不出像素的那一页
/// 仍然只出一张——预告那个上界因此对它偏大一张，那正是[区间断言](process_volume)容得下的。
const OUTPUTS_PER_FAILED_PAGE: usize = 1;

/// 一个源页产出的第 `ordinal` 张输出页（从 0 起）在输出容器里的相对位置，
/// `count` 是这一源页总共产出几张。
///
/// 扩展名一律换成 png。**只产出一张时名字就是源页名换扩展名**——一对一那条老路
/// 一个字符都不多，升级的人手上的输出因此不会有成员被改名。产出多张时在名字后面接一个
/// 从 1 起的序号（`001.jpg` → `001-1.png`、`001-2.png`），序号顺序就是阅读顺序。
///
/// 加了序号的名字可能撞上卷里本来就有的另一个成员（源里同时有 `001.jpg` 与 `001-1.png`），
/// 那一撞由 [`ensure_one_member_per_output`] 当场拦下，不静默覆盖。
fn output_name(relative: &Path, ordinal: usize, count: usize) -> PathBuf {
    if count <= 1 {
        return relative.with_extension("png");
    }
    let mut name = relative.file_stem().unwrap_or_default().to_os_string();
    name.push(format!("-{}.png", ordinal + 1));
    relative.with_file_name(name)
}

/// 一个源页产出的那几张输出页的成员名，按阅读顺序。规则见 [`output_name`]。
fn output_names(relative: &Path, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|ordinal| output_name(relative, ordinal, count))
        .collect()
}

/// 这一卷每个源页**当它一张都不切时**的输出成员名：外层按源页序，内层按阅读顺序。
///
/// 这一份是碰像素之前唯一给得出来的名单：一个源页产出几张由内容决定（页几何批 04 号票），
/// 而这一步在解码之前。它只喂开工前那道撞名校验——拦下与内容无关的那些
/// （`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
/// 买的是**别白做一整卷**。真正产出的那批名字等第一遍走完再查一遍
/// （见 [`ensure_no_two_outputs_collide`]）。
///
/// 幂等不再问它：名单改从上一趟写在输出里的记录读回来（见 [`can_skip`]）。
///
/// 透传文件原名不动，不必单列一份。
fn one_to_one_targets(volume: &Volume) -> Vec<Vec<PathBuf>> {
    volume
        .pages
        .iter()
        .map(|page| output_names(&page.relative, 1))
        .collect()
}

/// 每个输出成员只对一个源成员。
///
/// 扩展名一律换成 png，`001.jpg` 与 `001.png` 于是撞在同一个输出上；一个源页产出多张时
/// 加的那个序号也可能撞上卷里本来就有的成员；归档里还可能有同名成员。
/// 撞了就报错——静默覆盖会让 `Report` 里两页指向同一个文件。
fn ensure_one_member_per_output(volume: &Volume, targets: &[Vec<PathBuf>]) -> Result<()> {
    let pages = volume
        .pages
        .iter()
        .zip(targets)
        .flat_map(|(page, names)| names.iter().map(move |name| (page, name.as_path())));
    let extras = volume
        .extras
        .iter()
        .map(|extra| (extra, extra.relative.as_path()));
    ensure_distinct_outputs(pages.chain(extras), |member| {
        volume.identity(member).display().to_string()
    })
}

/// 第一遍**真产出**的那批成员名互不冲突（页几何批 04 号票）。
///
/// 开工前那一遍（[`ensure_one_member_per_output`]）只查得了一对一那套名字：切开之后
/// 加的那个序号可能撞上卷里本来就有的成员——源里同时有 `001.jpg` 与 `001-1.png`，
/// 前者被切成两张，`001-1.png` 就有两个主人。那一撞要在**写出第一个字节之前**拦下，
/// 而这里正是两批名字第一次同时在手上的地方（第二遍还没开始，输出容器还没建）。
///
/// 报错指得出是哪两个源成员：输出页自己记着它来自哪一张（[`OutputPage::source`]），
/// 而透传成员按原名占着位。
fn ensure_no_two_outputs_collide(volume: &Volume, pages: &[OutputPage]) -> Result<()> {
    let written = pages
        .iter()
        .map(|page| (page.source.as_path(), page.target.as_path()));
    let extras = volume
        .extras
        .iter()
        .map(|extra| (volume.identity(extra), extra.relative.clone()))
        .collect::<Vec<_>>();
    let extras = extras
        .iter()
        .map(|(identity, relative)| (identity.as_path(), relative.as_path()));
    ensure_distinct_outputs(written.chain(extras), |source| source.display().to_string())
}

/// 一批 (源成员, 它要写到的输出成员) 里有没有两个源成员认领同一个输出成员。
///
/// 同一个源成员出现几次是合法的——它产出几张输出页就出现几次（页几何批 03 号票）；
/// 同一个输出成员被两个源成员认领则不行。
///
/// `identity` 只在真撞上时才被叫到：报错要指得出是哪两个源成员，而拼那两个名字
/// 不该让每一个卷都白付一遍。源成员做成类型参数而不是钉死 `&Member`，买的是这道校验
/// **单独测得了**——`Member` 的归档序号是 `source` 模块的私有字段，卷外造不出一个来，
/// 而一对多的撞名恰恰要在卷外喂进去才试得到（见本文件的用例）。
fn ensure_distinct_outputs<'a, M: Copy>(
    pairs: impl IntoIterator<Item = (M, &'a Path)>,
    identity: impl Fn(M) -> String,
) -> Result<()> {
    let mut taken: HashMap<&Path, M> = HashMap::new();
    for (member, relative) in pairs {
        if let Some(previous) = taken.insert(relative, member) {
            bail!(
                "{} 与 {} 都要写到 {}：请让同一卷内的成员名互不冲突",
                identity(previous),
                identity(member),
                relative.display()
            );
        }
    }
    Ok(())
}

/// 源库只读（ADR 0009）：输出与源卷互相嵌套时直接拒绝，不去猜用户的意思。
/// 两个卷不能写到同一个地方。
///
/// 输出名取自卷名，而卷名重复得很自然：一部漫画一个目录，每部里都有「第 1 话」。
/// 一次点名多部，后到的会把先到的**整卷盖掉**——一句告警都没有，在阅读器里也与真卷
/// 毫无分别。因此开工前就查：撞车要在写出第一个字节之前说。
///
/// 不替用户改名。「输出名就是卷名」这条约定要能反着用——看着输出得认得出是哪一卷——
/// 自动加后缀会让它失效，而失效的方式还是静默的。
fn ensure_no_two_volumes_share_an_output(request: &Request) -> Result<()> {
    let mut by_target: HashMap<String, Vec<&Path>> = HashMap::new();
    for input in &request.inputs {
        let target = source::planned_output(input, &request.output_root)?;
        by_target
            .entry(collision_key(&target))
            .or_default()
            .push(input.as_path());
    }
    let mut collisions: Vec<_> = by_target.into_values().filter(|by| by.len() > 1).collect();
    if collisions.is_empty() {
        return Ok(());
    }
    // 顺序取自第一个卷的路径：报错要可复现，而 `HashMap` 的遍历序不是。
    collisions.sort_by(|a, b| a[0].cmp(b[0]));

    let total: usize = collisions.iter().map(|by| by.len()).sum();
    let mut said =
        format!("{total} 个卷要写到同一批去处，后到的会把先到的整卷盖掉。撞在一起的是：\n");
    const SHOWN: usize = 5;
    for group in collisions.iter().take(SHOWN) {
        let target = source::planned_output(group[0], &request.output_root)?;
        said.push_str(&format!("  {}\n", target.display()));
        for input in group {
            said.push_str(&format!("    ← {}\n", input.display()));
        }
    }
    if collisions.len() > SHOWN {
        said.push_str(&format!("  ……另有 {} 处\n", collisions.len() - SHOWN));
    }
    said.push_str("输出名取自卷名，同名的卷因此撞在一起。分批处理，每批给一个自己的输出根。");
    bail!(said)
}

/// 撞车比的是文件系统认不认成同一个去处。
///
/// Windows 上大小写不区分，`Abc.cbz` 与 `abc.cbz` 是同一个文件；别的平台上是两个。
/// 按平台折叠，查出来的撞车才与真会发生的撞车一致。
fn collision_key(target: &Path) -> String {
    let text = target.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn ensure_output_is_elsewhere(input: &Path, output_root: &Path) -> Result<()> {
    let input_path = resolve(input)?;
    let output_path = resolve(output_root)?;
    if output_path.starts_with(&input_path) || input_path.starts_with(&output_path) {
        bail!(
            "输出目录 {} 与源卷 {} 相互嵌套：源库只读，请把输出写到别处",
            output_root.display(),
            input.display()
        );
    }
    Ok(())
}

/// 规范化到可比较的绝对路径。输出目录还不存在，因此上溯到最近的已存在祖先再接回剩下的分量。
fn resolve(path: &Path) -> Result<PathBuf> {
    let absolute =
        std::path::absolute(path).with_context(|| format!("解析路径 {}", path.display()))?;
    let mut suffix = Vec::new();
    let mut current = absolute.as_path();
    loop {
        if let Ok(canonical) = current.canonicalize() {
            let mut resolved = canonical;
            resolved.extend(suffix.iter().rev());
            return Ok(resolved);
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                suffix.push(name.to_os_string());
                current = parent;
            }
            // 一个已存在的祖先都没有，退回词法绝对路径。
            _ => return Ok(absolute),
        }
    }
}

#[cfg(test)]
mod tests {
    //! 一个源页产出**两张**输出页时，下游还跟不跟得上（页几何批 03 号票）。
    //!
    //! 命令行造不出这个形态：此刻每一源页恒产出一张（[`OUTPUTS_PER_SOURCE_PAGE`]），
    //! 决定 N 是多少的那套逻辑要等页几何批 04 号票。因此这几条直接喂一份 N=2 的第一遍产物，
    //! 问的是从成员命名到进度步数、从汇总到写出，这条管线认不认这个形状。
    //!
    //! 与集成用例分工不同：`tests/` 那一批只经 `run` 这个 seam，而 N=2 眼下走不到那个 seam。

    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    use crate::cache::Retention;
    use crate::source::Container;

    /// 基准 profile 上门成立的那一页拿得到的候选，由小到大。
    const CANDIDATES: [(BitDepth, Dither); 3] = [
        (BitDepth::One, Dither::Off),
        (BitDepth::Two, Dither::Off),
        (BitDepth::Four, Dither::Off),
    ];

    /// 一份最小的请求。各用例只改自己那一处。
    fn request() -> Request {
        Request {
            inputs: vec![PathBuf::from("library/volume-a")],
            output_root: PathBuf::from("out"),
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            filter: Filter::default(),
            bit_depth: None,
            dither: None,
            per_page: false,
            cache_budget: CacheBudget::default(),
            mode: Mode::Process,
            io_mode: IoMode::default(),
            progress: None,
            metadata: true,
        }
    }

    /// 一张走灰度路径的输出页。`values` 是三个候选各自的判据值。
    fn gray(source: &str, target: &str, size: Size, values: [f32; 3], slot: usize) -> OutputPage {
        let scores = CANDIDATES
            .into_iter()
            .zip(values)
            .map(|((bit_depth, dither), value)| CandidateScore {
                candidate: Candidate::new(bit_depth, dither),
                score: Score::from_value(value),
            })
            .collect();
        OutputPage {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
            origin: Origin::new(Path::new(source), 0, 1),
            outcome: Outcome::Processed {
                size,
                crop: Crop::keeping_all(size),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(size, size),
                color: PageColor::Gray,
                branch: Branch::Gray {
                    scores,
                    gate: GeometryGate::Holds,
                    slot,
                },
                salvage: None,
            },
        }
    }

    /// 一张走彩色分支的输出页：没有判据曲线，也没有几何门。
    fn color(source: &str, target: &str, size: Size) -> OutputPage {
        OutputPage {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
            origin: Origin::new(Path::new(source), 0, 1),
            outcome: Outcome::Processed {
                size,
                crop: Crop::keeping_all(size),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(size, size),
                color: PageColor::Color,
                branch: Branch::Color { encoded: None },
                salvage: None,
            },
        }
    }

    /// 一对一那条老路一个字符都不改，一对多才加序号（页几何批 03 号票的成员命名）。
    ///
    /// 前半句是这张票「什么都没变」那条验收在命名这一侧的形式：产出一张时名字
    /// 就是源页名换扩展名，升级的人手上的输出不会有成员被改名。
    #[test]
    fn one_output_keeps_the_source_name_and_several_get_ordered_suffixes() {
        assert_eq!(
            output_names(Path::new("001.jpg"), 1),
            [PathBuf::from("001.png")]
        );
        assert_eq!(
            output_names(Path::new("ch1/001.jpg"), 2),
            [
                PathBuf::from("ch1/001-1.png"),
                PathBuf::from("ch1/001-2.png")
            ]
        );
        // 序号从 1 起，序号顺序就是阅读顺序。
        assert_eq!(
            output_names(Path::new("001.png"), 3),
            [
                PathBuf::from("001-1.png"),
                PathBuf::from("001-2.png"),
                PathBuf::from("001-3.png")
            ]
        );
    }

    /// 第二段按**输出**成员数，前两段按源那一侧（页几何批 03 号票的进度步数）。
    ///
    /// 分得开才要紧：读源与解源页都发生在切开之前，只有写出那一段跟着切完的张数走。
    /// 混成一个数的话，切开的卷进度条会在第二遍里走过头或者停下不动。
    #[test]
    fn the_write_segment_counts_output_pages_and_the_read_segments_count_source_pages() {
        // 三张源页切成五张输出页，外加一个透传文件：幂等读 3+1、第一遍走 3、第二遍写 5+1。
        let split = MemberCounts {
            source_pages: 3,
            output_pages: 5,
            extras: 1,
        };
        assert_eq!(volume_steps(split, &request()), 4 + 3 + 6);
        // 一对一时与从前逐字相同。
        let intact = MemberCounts {
            output_pages: 3,
            ..split
        };
        assert_eq!(volume_steps(intact, &request()), 4 + 3 + 4);
        // dry-run 没有第二段，切成几张都不改变步数。
        let dry = Request {
            mode: Mode::DryRun,
            ..request()
        };
        assert_eq!(volume_steps(split, &dry), 4 + 3);
        // `--no-metadata` 关掉幂等那一段，第二段照旧按输出算。
        let bare = Request {
            metadata: false,
            ..request()
        };
        assert_eq!(volume_steps(split, &bare), 3 + 6);
    }

    /// 一个输出成员只对一个源成员：同一源成员出现几次是合法的，两个源成员撞在一起不行。
    ///
    /// 加了序号的名字可能撞上卷里本来就叫那个名字的成员——`001.jpg` 切出来的 `001-1.png`
    /// 与一张真叫 `001-1.jpg` 的页就是这个局面。撞了要当场指名道姓，不静默覆盖。
    #[test]
    fn two_source_members_may_not_claim_the_same_output_member() {
        // 同一个源成员产出两张，各占一个名字：合法。
        assert!(
            distinct(&[("001.jpg", "001-1.png"), ("001.jpg", "001-2.png")]).is_ok(),
            "一个源页产出多张不该被当成撞名"
        );

        let error = distinct(&[
            ("001.jpg", "001-1.png"),
            ("001.jpg", "001-2.png"),
            ("001-1.jpg", "001-1.png"),
        ])
        .expect_err("撞名该被拦下");
        let said = format!("{error:#}");
        for named in ["001.jpg", "001-1.jpg", "001-1.png"] {
            assert!(said.contains(named), "错误里没指出 {named}：{said}");
        }
    }

    /// 把 (源成员, 输出成员) 对喂给那道校验。源成员在这里就是一个名字。
    fn distinct(pairs: &[(&'static str, &'static str)]) -> Result<()> {
        ensure_distinct_outputs(
            pairs
                .iter()
                .map(|(source, target)| (*source, Path::new(*target))),
            |source: &str| source.to_owned(),
        )
    }

    /// 幂等要对上一个源页产出的**每一张**输出页，少一张就重做（页几何批 04 号票）。
    ///
    /// 名单不再是预告出来的：一个源页产出几张由内容决定，而这一道在解码之前。
    /// 它改由**上一趟写在输出里的记录**说出来——头一张写着「1/2」，第二张就非在不可
    /// （见 [`Origin`] 与 [`written_family`]）。
    ///
    /// 「输出里少一张就该察觉」这条能力是 `p0-hardening/03` 的地基，这一条钉的正是它：
    /// 只认「至少有一张」的实现在下半段会答「跳过」，而那一半再也补不回来。
    #[test]
    fn a_skip_needs_the_fingerprint_on_every_output_page_of_a_source_page() {
        let space = tempfile::tempdir().expect("建临时目录");
        // 一个真卷：一张源页，好让 `can_skip` 拿得到容器形态与透传清单。
        let root = space.path().join("volume-a");
        fs::create_dir_all(&root).expect("建源卷");
        let page = GrayImage::new(Size::new(4, 4), vec![128; 16]);
        fs::write(
            root.join("001.png"),
            encode::png(&page, BitDepth::One, None).expect("编一张源页"),
        )
        .expect("写源页");
        let volume = source::open(&root).expect("打开源卷");

        // 这一张源页切成了两半：两张输出页各记着自己是那一族的第几张。
        let names = output_names(Path::new("001.png"), 2);
        let fingerprint = Fingerprint::new(&request(), "0".repeat(32));
        let written = |ordinal: usize, count: usize| {
            let origin = Origin::new(Path::new("001.png"), ordinal, count);
            let record = Record::color(&fingerprint, &origin, None);
            encode::png(&page, BitDepth::One, Some(&record)).expect("编一张带记录的页")
        };

        // 两半都在、都带着这份指纹与自己那一格：跳得过，而且数得出是两张。
        let output = space.path().join("out-both");
        fs::create_dir_all(&output).expect("建输出容器");
        for (ordinal, name) in names.iter().enumerate() {
            fs::write(output.join(name), written(ordinal, 2)).expect("写一张输出页");
        }
        assert_eq!(
            can_skip(&output, &volume, &fingerprint),
            Some(2),
            "两半都齐着还是重做了"
        );

        // 后一半被删掉：整卷重做。剩下那一张仍写着「1/2」，缺口因此看得见。
        fs::remove_file(output.join(&names[1])).expect("删掉后一半");
        assert_eq!(
            can_skip(&output, &volume, &fingerprint),
            None,
            "输出里少了一张，这一卷仍然被跳过了"
        );

        // 旧记录（没有来路那一项）不命中：它说不出自己那一族有几张，
        // 证不了「输出里没少东西」，而幂等从不该给一个证不出来的命中。
        let old = space.path().join("out-old");
        fs::create_dir_all(&old).expect("建输出容器");
        let stale = encode::png(&page, BitDepth::One, None).expect("编一张不带记录的页");
        fs::write(old.join(&names[0]), &stale).expect("写一张输出页");
        fs::write(old.join(&names[1]), &stale).expect("写一张输出页");
        assert_eq!(can_skip(&old, &volume, &fingerprint), None);
    }

    /// 没切开的那一族只有一张，而且它得**自己说是一张**（页几何批 04 号票）。
    ///
    /// 名字对上、指纹也对上，记录却写着「共两张」——那说明另一张要么被删了、
    /// 要么是别的参数跑出来的。这一族因此不齐，整卷重做。
    #[test]
    fn a_one_to_one_output_must_say_it_is_the_only_one() {
        let space = tempfile::tempdir().expect("建临时目录");
        let root = space.path().join("volume-a");
        fs::create_dir_all(&root).expect("建源卷");
        let page = GrayImage::new(Size::new(4, 4), vec![128; 16]);
        fs::write(
            root.join("001.png"),
            encode::png(&page, BitDepth::One, None).expect("编一张源页"),
        )
        .expect("写源页");
        let volume = source::open(&root).expect("打开源卷");
        let fingerprint = Fingerprint::new(&request(), "0".repeat(32));
        let written = |count: usize| {
            let origin = Origin::new(Path::new("001.png"), 0, count);
            let record = Record::color(&fingerprint, &origin, None);
            encode::png(&page, BitDepth::One, Some(&record)).expect("编一张带记录的页")
        };

        let honest = space.path().join("out-one");
        fs::create_dir_all(&honest).expect("建输出容器");
        fs::write(honest.join("001.png"), written(1)).expect("写一张输出页");
        assert_eq!(can_skip(&honest, &volume, &fingerprint), Some(1));

        let lying = space.path().join("out-claims-two");
        fs::create_dir_all(&lying).expect("建输出容器");
        fs::write(lying.join("001.png"), written(2)).expect("写一张输出页");
        assert_eq!(can_skip(&lying, &volume, &fingerprint), None);
    }

    /// 卷内统一尺寸的众数在**切开之后**取：同一源页的两半各算一张（页几何批 03 号票）。
    ///
    /// 按源页数，两个尺寸各一票、并列；按输出页数，切开的那个尺寸两票。
    /// 失败页照它留白占位，数错了整卷的占位页就换一个尺寸。
    #[test]
    fn the_uniform_size_counts_output_pages() {
        let narrow = Size::new(500, 800);
        let wide = Size::new(600, 800);
        let pages = vec![
            gray("001.jpg", "001-1.png", narrow, [9.0, 4.0, 1.0], 0),
            gray("001.jpg", "001-2.png", narrow, [9.0, 4.0, 1.0], 1),
            gray("002.jpg", "002.png", wide, [9.0, 4.0, 1.0], 2),
        ];

        assert_eq!(uniform_size(&pages, Size::new(1264, 1680)), narrow);
    }

    /// 汇总那一层的序号指进**输出页**那个序列，不是源页那个（页几何批 03 号票）。
    ///
    /// 头一张源页的两半走彩色分支、不进上包络，灰度页因此排在第 2、3 位上——
    /// 按源页数的话驱动页会指到 0 或 1，而那是另一张页。
    #[test]
    fn the_volume_level_summary_indexes_output_pages_not_source_pages() {
        let size = Size::new(600, 800);
        let pages = vec![
            color("001.jpg", "001-1.png", size),
            color("001.jpg", "001-2.png", size),
            gray("002.jpg", "002-1.png", size, [9.0, 4.0, 1.0], 0),
            gray("002.jpg", "002-2.png", size, [8.0, 3.0, 0.5], 1),
        ];

        let (verdicts, verdict) = summarize_volume(&pages, &request());

        // 判定与输出页一一对应，一张一格。
        assert_eq!(verdicts.len(), 4);
        assert!(
            verdicts[0].is_none() && verdicts[1].is_none(),
            "彩色分支上没有判定"
        );
        assert!(verdicts[2].is_some() && verdicts[3].is_some());
        let Some(VolumeVerdict::Envelope(envelope)) = verdict else {
            panic!("这一卷该由上包络定档，实际是 {verdict:?}");
        };
        assert!(
            matches!(envelope.driver, 2 | 3),
            "驱动页指到了第 {} 张，那不是一张灰度页",
            envelope.driver
        );
    }

    /// 同一源页切出来的两张各写各的成员，源那一格指着同一张源页（页几何批 03 号票）。
    #[test]
    fn both_halves_of_one_source_page_report_their_own_output_and_the_same_source() {
        let size = Size::new(600, 800);
        let verdict = Some(Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::Off),
            reason: Reason::VolumeEnvelope,
        });
        let out = Path::new("out/volume-a");

        let reports: Vec<PageReport> = vec![
            gray(
                "library/volume-a/001.jpg",
                "001-1.png",
                size,
                [9.0, 4.0, 1.0],
                0,
            ),
            gray(
                "library/volume-a/001.jpg",
                "001-2.png",
                size,
                [9.0, 4.0, 1.0],
                1,
            ),
        ]
        .into_iter()
        .map(|page| page.into_report(out, verdict, size))
        .collect();

        assert_eq!(reports[0].output, PathBuf::from("out/volume-a/001-1.png"));
        assert_eq!(reports[1].output, PathBuf::from("out/volume-a/001-2.png"));
        assert_eq!(
            reports[0].source, reports[1].source,
            "两半来自同一张源页，报告里那一格该指着同一个成员"
        );
    }

    /// 第二遍按输出页写出：一个源页的两半各落一个成员，各带各的字节，各报到一步（页几何批 03 号票）。
    ///
    /// 成员名跟着页走。由第二遍按源页序数出来的话，两半会写到同一个名字上——
    /// 一张覆盖另一张，而报告里两页仍各说各的。
    #[test]
    fn the_second_pass_writes_every_output_page_of_a_split_source_page() {
        let space = tempfile::tempdir().expect("建临时目录");
        let out = space.path().join("volume-a");
        let size = Size::new(4, 4);

        // 两半在缓存里各占一个序号，像素刻意不同：串位当场看得出来。
        let cache = Mutex::new(cache::PageCache::new(
            CacheBudget::default(),
            Retention::Keep,
        ));
        let slots: Vec<usize> = [0u8, u8::MAX]
            .into_iter()
            .map(|level| {
                let image = GrayImage::new(size, vec![level; 16]);
                lock(&cache)
                    .insert(cache::compress(&image))
                    .expect("存进缓存")
            })
            .collect();

        let pages = vec![
            gray(
                "volume-a/001.jpg",
                "001-1.png",
                size,
                [9.0, 4.0, 1.0],
                slots[0],
            ),
            gray(
                "volume-a/001.jpg",
                "001-2.png",
                size,
                [9.0, 4.0, 1.0],
                slots[1],
            ),
        ];
        let verdict = Some(Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::Off),
            reason: Reason::VolumeEnvelope,
        });
        let encode = Encode {
            uniform: size,
            cache: &cache,
            recorder: None,
        };
        let tally = Tally::default();
        let watching = ProgressSink::new(tally.clone());
        let standing = progress::Standing::default();

        let mut sink = Sink::create(&out, Container::Directory).expect("建输出容器");
        second_pass(
            &pages,
            &[verdict, verdict],
            &encode,
            &mut sink,
            progress::Events::new(Some(&watching), &standing),
        )
        .expect("写出这两张");
        sink.finish().expect("收尾");

        let mut members: Vec<String> = fs::read_dir(&out)
            .expect("列输出容器")
            .map(|entry| {
                entry
                    .expect("读目录项")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        members.sort();
        assert_eq!(members, ["001-1.png", "001-2.png"]);
        assert_ne!(
            fs::read(out.join("001-1.png")).expect("读回头一半"),
            fs::read(out.join("001-2.png")).expect("读回后一半"),
            "两半写出了同样的字节：缓存序号串位了"
        );
        // 一张输出页一步，不是一张源页一步。
        assert_eq!(tally.steps(), 2);
    }

    /// 数第二遍报到了几步。观察者那一端只关心「走完一步」这一种事件，
    /// 别的事件长什么样在 `progress` 与 `tests/events.rs` 里测。
    #[derive(Clone, Default)]
    struct Tally(Arc<AtomicUsize>);

    impl Tally {
        fn steps(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }
    }

    impl Progress for Tally {
        fn observe(&self, event: Event<'_>) -> Instruction {
            if matches!(event, Event::Stepped { .. }) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            Instruction::Continue
        }
    }
}
