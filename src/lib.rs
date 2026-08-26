//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外是两个 seam，其余全部是内部实现。
//!
//! [`run`] 是主 seam：所有模式走同一个入口，CLI 是它之上的薄层，只负责把命令行参数拼成
//! [`Request`]、把 [`Report`] 渲染成文字。
//!
//! [`score`] 是第二个 seam：判据的纯函数形态，数值与性质测试、标定工具直接调它。
//! 它周边的类型——[`Reference`]、[`Score`]、[`GrayImage`]、[`Candidate`]、[`quantize`]——
//! 一并公开，判据的调用方要拿它们拼出参照与候选。
//!
//! [`calibration_chart`] 是第三个：灰阶阶梯标定图。它不在管线上——不读源、不写文件，
//! 只按一个 [`Profile`] 画出一张图的字节，`--gray-levels` 填的那个数由它量出来（ADR 0003）。

mod cache;
mod calibrate;
mod color;
mod decide;
mod decode;
mod encode;
mod envelope;
mod geometry;
mod gray;
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

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result, anyhow, bail};
use rayon::prelude::*;

pub use cache::{CacheBudget, CacheUsage};
pub use color::PageColor;
pub use decide::{CandidateScore, Reason, Verdict};
pub use decode::Salvage;
pub use envelope::Envelope;
pub use geometry::{GeometryGate, Size};
pub use gray::GrayImage;
pub use medium::{ChosenBy, IoMode, IoPlan, Medium};
pub use metric::{Aggregation, Reference, Score, aggregation, score};
pub use profile::{Panel, Profile, Threshold, ThresholdSource};
pub use progress::{Progress, ProgressSink};
pub use quantize::{BitDepth, Candidate, Dither, quantize};
pub use report::{
    PageBranch, PageOutcome, PageReport, Processed, Report, VolumeReport, VolumeVerdict,
};
pub use request::{Mode, Request};
pub use resample::{Filter, Scaling};

use metadata::{Fingerprint, Record, Recorder};
use sink::Sink;
use source::{Member, Volume};

/// 画一张灰阶阶梯标定图，编成 PNG 字节。
///
/// 尺寸恒等于面板分辨率：图要在真机上 1:1 显示才数得准（见 `calibrate`）。
///
/// **图本身不经过位深判定**：它是量具，不是被处理的页——判据、上包络、抖动一概不碰它，
/// 像素以 8 位工作精度原样交给编码器，写出的是无损 PNG。自描述元数据也不写：
/// 记录说的是一页的判定与幂等依据（见 `metadata`），标定图两样都没有。
pub fn calibration_chart(profile: &Profile) -> Result<Vec<u8>> {
    encode::png(&calibrate::chart(profile), BitDepth::Eight, None)
}

/// 处理点名的若干卷，产出设备优化副本。源卷只读。
pub fn run(request: &Request) -> Result<Report> {
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
    let volumes = request
        .inputs
        .iter()
        .map(|input| {
            let medium = probes.medium(input);
            process_volume(input, request, medium)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        profile: request.profile.clone(),
        volumes,
    })
}

/// 这一卷要走多少步（spec 的 story 30）。
///
/// 三段：幂等这一道读全部成员，第一遍走每一页，第二遍写全部成员。各自可能不在——
/// `--no-metadata` 关掉第一段（那时既没有记录可写也没有依据可比），dry-run 没有第三段
/// （一个文件都不落盘）。因此按**这一趟真要做的事**算，而不是按一个固定的倍数：
/// 不然进度条会停在某个百分比上再也不动。
///
/// 幂等命中的卷会提前收摊，那时走过的只有第一段——预告的步数是**上界**，不是承诺，
/// 剩下的由 `Progress::volume_finished` 一次性了结。
fn volume_steps(volume: &Volume, request: &Request) -> u64 {
    let members = (volume.pages.len() + volume.extras.len()) as u64;
    let fingerprint = if request.metadata { members } else { 0 };
    let write = if request.mode == Mode::Process {
        members
    } else {
        0
    };
    fingerprint + volume.pages.len() as u64 + write
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
fn process_volume(input: &Path, request: &Request, medium: Medium) -> Result<VolumeReport> {
    let mut volume = source::open(input)?;
    // 这一卷的两个可能去处。哪一个作数要等第一遍走完才知道，另一个则可能留着上一趟的过期副本。
    let clean = volume.output_path(&request.output_root);
    let isolated = volume.output_path(&request.output_root.join(ISOLATED_DIRECTORY));
    let targets = page_targets(&volume);
    ensure_one_member_per_output(&volume, &targets)?;

    let io = IoPlan::decide(medium, request.io_mode, volume.container, cores());
    let writes = request.mode == Mode::Process;
    let steps = progress::Steps::new(request.progress.as_ref());
    steps.started(&volume.root, volume_steps(&volume, request));

    // `--no-metadata` 关掉记录，幂等的依据无处可写也无处可读，这一整道于是不在。
    let fingerprint = request
        .metadata
        .then(|| volume_fingerprint(&mut volume, request, &io, steps))
        .transpose()?;
    if let Some(fingerprint) = &fingerprint
        && can_skip(&clean, &volume, &targets, fingerprint)
    {
        steps.finished();
        return Ok(VolumeReport {
            volume: volume.root,
            output: clean,
            // 跳过的卷是干净的，隔离目录里若还留着一份，那是上一趟坏页时写的。
            superseded: superseded(&isolated),
            pages: Vec::new(),
            verdict: Some(VolumeVerdict::Skipped {
                page_count: targets.len(),
            }),
            cache: CacheUsage::new(request.cache_budget),
            decodes: 0,
            io,
        });
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
    let scored = first_pass(
        &mut volume,
        request,
        &cache,
        &decoder,
        fingerprint.as_ref(),
        &io,
        steps,
    )?;

    let (verdicts, verdict) = summarize_volume(&scored, request);
    let uniform = uniform_size(&scored, request.profile.panel().resolution);
    // 有一页失败，整卷就去隔离目录；另一个去处留着的那一份这一趟碰都不碰。
    let (output, elsewhere) = if scored.iter().any(ScoredPage::failed) {
        (isolated, clean)
    } else {
        (clean, isolated)
    };
    let superseded = superseded(&elsewhere);

    // dry-run 一个文件都不落盘，输出容器因此连建都不建（spec 的 story 6）。
    if writes {
        let mut sink = Sink::create(&output, volume.container)?;
        let recorder = fingerprint
            .as_ref()
            .map(|fingerprint| Recorder::new(fingerprint, driver(verdict)));
        let encode = Encode {
            uniform,
            cache: &cache,
            recorder: recorder.as_ref(),
        };
        second_pass(&scored, &verdicts, &targets, &encode, &mut sink, steps)?;
        for extra in &volume.extras {
            let bytes = volume.reader.read(extra)?;
            sink.write_extra(&extra.relative, &bytes)?;
            steps.step();
        }
        sink.finish()?;
    }
    steps.finished();

    let pages = scored
        .into_iter()
        .zip(verdicts)
        .zip(&targets)
        .map(|((page, verdict), relative)| {
            page.into_report(output.join(relative), verdict, uniform)
        })
        .collect();
    Ok(VolumeReport {
        volume: volume.root,
        output,
        superseded,
        pages,
        verdict,
        cache: lock(&cache).usage(),
        decodes: decoder.decodes(),
        io,
    })
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
fn uniform_size(pages: &[ScoredPage], panel: Size) -> Size {
    let mut counted: Vec<(Size, usize)> = Vec::new();
    for size in pages.iter().filter_map(ScoredPage::size) {
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
fn summarize_volume(
    pages: &[ScoredPage],
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

/// 第一遍产出的一页：这一页有没有处理成，以及处理成了的话留下了什么。
struct ScoredPage {
    source: PathBuf,
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

impl ScoredPage {
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

    /// 这一页失败了吗。一卷里只要有一页答是，整卷就进隔离目录。
    fn failed(&self) -> bool {
        matches!(self.outcome, Outcome::Failed { .. })
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
    /// `uniform` 只对失败页说话：它写出去用的就是这个尺寸。
    fn into_report(self, output: PathBuf, verdict: Option<Verdict>, uniform: Size) -> PageReport {
        let (size, outcome) = match self.outcome {
            Outcome::Processed {
                size,
                scaling,
                color,
                branch,
                salvage,
            } => {
                let processed = Processed {
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

/// 第一遍：读 → 解码 → **彩页识别** → 分流。
///
/// 灰度路径：转灰 → 几何与几何门 → 缩放 → 判据曲线，同时把参照存进缓存。
/// 彩色分支：几何 → 缩放 → 编码，不进缓存、不求判据（ADR 0005 决定第 4 条）。
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
    steps: progress::Steps,
) -> Result<Vec<ScoredPage>> {
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
        steps,
    };
    let mut scored: Vec<(usize, Result<ScoredPage>)> =
        read::reads(reader, &members, io.readers, read::BUDGET)
            .par_bridge()
            .map(|read| {
                let index = read.index;
                (index, compute.page(&sources[index], read.bytes))
            })
            .collect();
    // 计算层乱序完成，页序在这里归位。往后每一处「第 n 页」都指得回同一页。
    scored.sort_by_key(|(index, _)| *index);
    // 归位**之后**才短路取错，因此报出来的是序号最小的那一页出的错，不是最先撞上的那一页：
    // 换一次调度就换一句错误的报告等于没有报告。`--dither fs` 撞上几何门那一支尤其吃这一条,
    // 那一支上没有报告可看，错误里指的那一页是唯一的线索。
    // 代价是一页出错时整卷仍会算完，而这一支上整卷本来就要作废，省下的那点算力买不到什么。
    scored
        .into_iter()
        .map(|(_, page)| page)
        .collect::<Result<Vec<_>>>()
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
    steps: progress::Steps<'a>,
}

impl Compute<'_> {
    /// 算一页：解码 → 彩页识别 → 分流。语义与顺着做时逐字相同，见 [`first_pass`]。
    fn page(&self, source: &Path, bytes: Result<Vec<u8>>) -> Result<ScoredPage> {
        let page = self.branch(source, bytes)?;
        self.steps.step();
        Ok(page)
    }

    fn branch(&self, source: &Path, bytes: Result<Vec<u8>>) -> Result<ScoredPage> {
        let request = self.request;
        let panel = request.profile.panel();
        let read = bytes.and_then(|bytes| {
            self.decoder
                .decode(&bytes)
                .with_context(|| format!("解 {} 这一页", source.display()))
        });
        let (decoded, salvage) = match read {
            Ok(decoded) => (decoded.image, decoded.salvage),
            // 一张坏图不毁掉整卷（spec 的 story 24）：记下原因就走，
            // 第二遍拿卷内统一尺寸给它留一张白页，整卷进隔离目录。
            Err(error) => {
                return Ok(ScoredPage {
                    source: source.to_path_buf(),
                    outcome: Outcome::Failed {
                        reason: format!("{error:#}"),
                    },
                });
            }
        };
        let color = color::identify(&decoded);

        if panel.color && color.is_color() {
            let image = color::to_color(&decoded);
            let size = geometry::fit_inside(image.size(), panel.resolution);
            let (scaled, scaling) = resample::resize_color(&image, size, request.filter)?;
            // dry-run 一个文件都不落盘，编出来的字节没人要。
            let record = self
                .fingerprint
                .map(|fingerprint| Record::color(fingerprint, salvage));
            let encoded = match request.mode {
                Mode::Process => Some(
                    encode::color_png(&scaled, record.as_ref())
                        .with_context(|| format!("编 {} 这一页", source.display()))?,
                ),
                Mode::DryRun => None,
            };
            return Ok(ScoredPage {
                source: source.to_path_buf(),
                outcome: Outcome::Processed {
                    size,
                    scaling,
                    color,
                    branch: Branch::Color { encoded },
                    salvage,
                },
            });
        }

        let gray = gray::to_gray(&decoded);
        let size = geometry::fit_inside(gray.size(), panel.resolution);
        // 门在这里判，也只在这里判：这一页的候选集当场定下，判据只在那一套上求。
        // 门只决定这一页——同一卷里贴住面板的页照旧拿得到抖动那一维（ADR 0007 决定第 1 条）。
        let gate = GeometryGate::of(size, panel.resolution);
        let allowed = self
            .candidates
            .for_gate(gate)
            .with_context(|| format!("{} 这一页关上了几何门", source.display()))?;
        let (scaled, scaling) = resample::resize(&gray, size, request.filter)?;
        let reference = Reference::new(panel, scaled);
        let scores = candidate_scores(&reference, allowed);
        let block = cache::compress(reference.image());
        let slot = lock(self.cache)
            .insert(block)
            .with_context(|| format!("缓存 {} 这一页", source.display()))?;
        Ok(ScoredPage {
            source: source.to_path_buf(),
            outcome: Outcome::Processed {
                size,
                scaling,
                color,
                branch: Branch::Gray { scores, gate, slot },
                salvage,
            },
        })
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
fn second_pass(
    pages: &[ScoredPage],
    verdicts: &[Option<Verdict>],
    targets: &[PathBuf],
    encode: &Encode,
    sink: &mut Sink,
    steps: progress::Steps,
) -> Result<()> {
    let work: Vec<(&ScoredPage, Option<Verdict>, &PathBuf)> = pages
        .iter()
        .zip(verdicts)
        .zip(targets)
        .map(|((page, verdict), relative)| (page, *verdict, relative))
        .collect();
    for batch in work.chunks(cores()) {
        let encoded: Vec<Cow<'_, [u8]>> = batch
            .par_iter()
            .map(|(page, verdict, _)| encode.page(page, *verdict))
            .collect::<Result<Vec<_>>>()?;
        for ((_, _, relative), bytes) in batch.iter().zip(&encoded) {
            sink.write_page(relative, bytes)?;
            steps.step();
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
    fn page<'p>(&self, page: &'p ScoredPage, verdict: Option<Verdict>) -> Result<Cow<'p, [u8]>> {
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
                let record = recorder.map(Recorder::failed);
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
                let record = recorder.map(|recorder| recorder.gray(verdict, *salvage));
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
    steps: progress::Steps,
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
        steps.step();
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
fn can_skip(
    output: &Path,
    volume: &Volume,
    targets: &[PathBuf],
    fingerprint: &Fingerprint,
) -> bool {
    if targets.is_empty() {
        return false;
    }
    let Some(mut written) = sink::Written::open(output, volume.container) else {
        return false;
    };
    targets
        .iter()
        .all(|relative| written.fingerprint_of(relative).as_ref() == Some(fingerprint))
        && volume
            .extras
            .iter()
            .all(|extra| written.holds(&extra.relative))
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
        return Err(nothing_left_error(request));
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
fn nothing_left_error(request: &Request) -> anyhow::Error {
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
        _ => anyhow!(
            "点名的抖动模式越过了几何门：这一页源比目标尺寸还小，按不放大原样输出，\
             阅读器显示时还要再缩一次，抖动推到高频的误差会被折回低频。\
             门在这一页上不成立，抖动因此关闭，--dither 覆盖不了它——它是页的几何事实，\
             不是一个可以放宽的档位（ADR 0007）"
        ),
    }
}

/// 每页在输出容器里的相对位置：扩展名一律换成 png。透传文件原名不动，不必单列一份。
fn page_targets(volume: &Volume) -> Vec<PathBuf> {
    volume
        .pages
        .iter()
        .map(|page| page.relative.with_extension("png"))
        .collect()
}

/// 扩展名一律换成 png，`001.jpg` 与 `001.png` 于是撞在同一个输出上；归档里还可能有同名成员。
/// 撞了就报错——静默覆盖会让 `Report` 里两页指向同一个文件。
fn ensure_one_member_per_output(volume: &Volume, targets: &[PathBuf]) -> Result<()> {
    let members = volume.pages.iter().chain(&volume.extras);
    let relatives = targets
        .iter()
        .chain(volume.extras.iter().map(|extra| &extra.relative));

    let mut taken: HashMap<&Path, &Member> = HashMap::new();
    for (member, relative) in members.zip(relatives) {
        if let Some(previous) = taken.insert(relative, member) {
            bail!(
                "{} 与 {} 都要写到 {}：请让同一卷内的成员名互不冲突",
                volume.identity(previous).display(),
                volume.identity(member).display(),
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
