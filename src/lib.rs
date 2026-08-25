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

mod cache;
mod decide;
mod decode;
mod encode;
mod envelope;
mod geometry;
mod gray;
mod metric;
mod profile;
mod quantize;
mod report;
mod request;
mod resample;
mod sink;
mod source;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

pub use cache::{CacheBudget, CacheUsage};
pub use decide::{CandidateScore, Reason, Verdict};
pub use envelope::Envelope;
pub use geometry::{GeometryGate, Size};
pub use gray::GrayImage;
pub use metric::{Reference, Score, score};
pub use profile::{Panel, Profile, Threshold};
pub use quantize::{BitDepth, Candidate, Dither, quantize};
pub use report::{PageReport, Report, VolumeReport, VolumeVerdict};
pub use request::{Mode, Request};
pub use resample::{Filter, Scaling};

use sink::Sink;
use source::{Member, Volume};

/// 处理点名的若干卷，产出设备优化副本。源卷只读。
pub fn run(request: &Request) -> Result<Report> {
    if request.inputs.is_empty() {
        bail!("处理范围为空：至少点名一个卷（ADR 0009：处理点名的子集）");
    }
    ensure_the_overrides_leave_a_candidate(request)?;
    for input in &request.inputs {
        ensure_output_is_elsewhere(input, &request.output_root)?;
    }
    let volumes = request
        .inputs
        .iter()
        .map(|input| process_volume(input, request))
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        profile: request.profile.clone(),
        volumes,
    })
}

/// 处理一个卷：第一遍解码到判据，第二遍量化到写出，非图片成员原样搬过去。
///
/// 两遍之间隔着缓存（ADR 0005：解码一次，缓存缩放后的图）。第二遍的输入是第一遍存下的参照，
/// 源页因此只被解码一次——`VolumeReport::decodes` 是这条不变量看得见的形式。
///
/// dry-run 走同一条路，只是不建输出容器，第二遍也就没有可写的地方。
fn process_volume(input: &Path, request: &Request) -> Result<VolumeReport> {
    let mut volume = source::open(input)?;
    let output = volume.output_path(&request.output_root);
    let targets = page_targets(&volume);
    ensure_one_member_per_output(&volume, &targets)?;

    // dry-run 一个文件都不落盘，输出容器因此连建都不建（spec 的 story 6）。
    let sink = match request.mode {
        Mode::Process => Some(Sink::create(&output, volume.container)?),
        Mode::DryRun => None,
    };
    // dry-run 没有第二遍，缓存于是只记账不留页：用量照旧预告得出，临时文件一个不建。
    let retention = match request.mode {
        Mode::Process => cache::Retention::Keep,
        Mode::DryRun => cache::Retention::Account,
    };
    let mut cache = cache::PageCache::new(request.cache_budget, retention);
    let mut decoder = decode::Decoder::new();
    let scored = first_pass(
        &mut volume,
        request,
        &output,
        &targets,
        &mut cache,
        &mut decoder,
    )?;

    let (verdicts, verdict) = summarize_volume(&scored.pages, request);

    if let Some(mut sink) = sink {
        second_pass(&scored.pages, &verdicts, &targets, &mut cache, &mut sink)?;
        for extra in &volume.extras {
            let bytes = volume.reader.read(extra)?;
            sink.write_extra(&extra.relative, &bytes)?;
        }
        sink.finish()?;
    }

    Ok(VolumeReport {
        volume: volume.root,
        output,
        pages: scored
            .pages
            .into_iter()
            .zip(verdicts)
            .map(|(page, verdict)| page.into_report(verdict))
            .collect(),
        verdict,
        gate: scored.gate,
        cache: cache.usage(),
        decodes: decoder.decodes(),
    })
}

/// 汇总：先逐页定档，再把它们收成卷级的一个基准档（ADR 0006：位深按卷取上包络并加迟滞）。
///
/// 夹在两遍之间——要看完整卷才做得了，而第二遍此刻已经不必回头碰源页（ADR 0005）。
/// 返回的逐页判定与 `pages` 等长同序，第二遍读的就是它。
///
/// 逐页定档也落在这里，而不在第一遍：几何门要看完整卷才判得死（一页比目标小就整卷关掉抖动，
/// 见 [`first_pass`]），而候选集是判定的前提——在门还可能关上的时候定档，定的是一套
/// 随后可能被裁掉的候选。
///
/// 两条出口走不到上包络，各有各的道理：`--per-page` 是用户点名要逐页最优（决定第 6 条），
/// 覆盖项裁到只剩一个候选是判定整个被顶掉、逐页已全是 `Override`——后者不是「被关掉」，
/// 而是逐页结果里根本没有分布可聚合。两者在报告里各说各的，见 [`VolumeVerdict`]。
fn summarize_volume(
    pages: &[ScoredPage],
    request: &Request,
) -> (Vec<Verdict>, Option<VolumeVerdict>) {
    let Some(first) = pages.first() else {
        return (Vec::new(), None);
    };
    let threshold = request.profile.threshold();
    let pinned = pinned(request, &first.scores);
    let decided: Vec<Verdict> = pages
        .iter()
        .map(|page| decide::decide(&page.scores, threshold, pinned))
        .collect();

    if let Some(candidate) = pinned {
        return (decided, Some(VolumeVerdict::Override(candidate)));
    }
    if request.per_page {
        return (decided, Some(VolumeVerdict::PerPage));
    }
    let inputs: Vec<envelope::Page> = pages
        .iter()
        .zip(&decided)
        .map(|(page, verdict)| envelope::Page {
            scores: &page.scores,
            decided: verdict.candidate,
        })
        .collect();
    let summary = envelope::summarize(&inputs, threshold).expect("卷非空");
    (
        summary.verdicts,
        Some(VolumeVerdict::Envelope(summary.envelope)),
    )
}

/// 覆盖项裁到只剩一个候选时的那一个：判定被顶掉，判据说什么都不改变结果（spec 的 story 23）。
///
/// 「裁到只剩一个」与「有覆盖项」两条都要：`--gray-levels 2` 撞上几何门不成立同样只剩一个候选，
/// 但那一档是判出来的，不是被顶掉的——理由分得清，报告才解释得了它是怎么来的。
///
/// 反过来，只点了一维的覆盖项裁不到只剩一个：`--bit-depth 4` 而几何门开着时，
/// 抖动那一维还有得判，判据照旧说了算。
fn pinned(request: &Request, scores: &[CandidateScore]) -> Option<Candidate> {
    let overridden = request.bit_depth.is_some() || request.dither.is_some();
    match scores {
        [only] if overridden => Some(only.candidate),
        _ => None,
    }
}

/// 第一遍产出的一页：报告要的几何与判据曲线，加上它在缓存里的序号。
///
/// 序号跟着页走，不由第二遍数数补出来。眼下两者恰好重合，但 ADR 0005 决定第 4 条说
/// 彩页在彩色 profile 下第一遍即写出、**不进灰度缓存**——那一票（10 号）落地当天页序与缓存序
/// 就此分家，而重新数出来的序号会静默地把另一页的像素写到这一页的位置上。
struct ScoredPage {
    source: PathBuf,
    output: PathBuf,
    size: Size,
    scaling: resample::Scaling,
    scores: Vec<CandidateScore>,
    slot: usize,
}

impl ScoredPage {
    /// 补上汇总定下的那个判定，就是报告要的一页。缓存序号不进报告——它是管线内部的事。
    fn into_report(self, verdict: Verdict) -> PageReport {
        PageReport {
            source: self.source,
            output: self.output,
            size: self.size,
            scaling: self.scaling,
            scores: self.scores,
            verdict,
        }
    }
}

/// 第一遍产出的一卷：逐页的判据曲线，加上这一卷的几何门判定。
struct Scored {
    pages: Vec<ScoredPage>,
    gate: GeometryGate,
}

/// 第一遍：读 → 解码 → 转灰 → 几何与几何门 → 缩放 → 判据曲线，同时把参照存进缓存。
///
/// 判据两种模式都求值，dry-run 预告的就是照做时的那一档（spec 的 story 6）。
/// 覆盖了判定也照求：`--dry-run --bit-depth 2` 要说得清「你点的这一档判据是多少」。
///
/// **几何门在这一遍上收口。**门是几何的、逐页看得出来，但它对整卷只有一个结果
/// （ADR 0007：条件不成立时抖动整体关闭），因此一页关上门，抖动那一维就当场
/// 从候选集里去掉，已经求过的抖动候选一并丢掉——「候选集全卷同一套」在任何时刻都成立，
/// 上包络与迟滞靠的正是这一条。门关得越早，白求的判据越少。
fn first_pass(
    volume: &mut Volume,
    request: &Request,
    output: &Path,
    targets: &[PathBuf],
    cache: &mut cache::PageCache,
    decoder: &mut decode::Decoder,
) -> Result<Scored> {
    let panel = request.profile.panel();
    let mut gate = GeometryGate::Holds;
    let mut allowed = candidates(request, gate)?;
    let mut pages: Vec<ScoredPage> = Vec::with_capacity(volume.pages.len());
    for (index, (page, relative)) in volume.pages.iter().zip(targets).enumerate() {
        let source = volume.identity(page);
        let bytes = volume.reader.read(page)?;
        let decoded = decoder
            .decode(&bytes)
            .with_context(|| format!("解 {} 这一页", source.display()))?;
        let gray = gray::to_gray(&decoded);
        let size = geometry::fit_inside(gray.size(), panel.resolution);
        if gate.holds() && !geometry::one_to_one(size, panel.resolution) {
            gate = GeometryGate::Broken { page: index };
            allowed = candidates(request, gate)
                .with_context(|| format!("{} 这一页关上了几何门", source.display()))?;
            for earlier in &mut pages {
                earlier
                    .scores
                    .retain(|scored| allowed.contains(&scored.candidate));
            }
        }
        let (scaled, scaling) = resample::resize(&gray, size, request.filter)?;
        let reference = Reference::new(panel, scaled);
        let scores = candidate_scores(&reference, &allowed);
        let slot = cache
            .store(reference.image())
            .with_context(|| format!("缓存 {} 这一页", source.display()))?;
        pages.push(ScoredPage {
            source,
            output: output.join(relative),
            size,
            scaling,
            scores,
            slot,
        });
    }
    Ok(Scored { pages, gate })
}

/// 第二遍：从缓存读 → 量化 → 编码 → 写出。不再碰源页（ADR 0005）。
fn second_pass(
    pages: &[ScoredPage],
    verdicts: &[Verdict],
    targets: &[PathBuf],
    cache: &mut cache::PageCache,
    sink: &mut Sink,
) -> Result<()> {
    for ((page, verdict), relative) in pages.iter().zip(verdicts).zip(targets) {
        let source = page.source.display();
        let reference = cache
            .load(page.slot)
            .with_context(|| format!("从缓存取 {source} 这一页"))?;
        let quantized = quantize::quantize(&reference, verdict.candidate);
        let encoded = encode::png(&quantized, verdict.candidate.bit_depth)
            .with_context(|| format!("编 {source} 这一页"))?;
        sink.write_page(relative, &encoded)?;
    }
    Ok(())
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

/// 本次可用的候选集，由小到大。
///
/// 四道裁剪，全部发生在判据求值之前：位深按面板灰阶数裁（ADR 0003），抖动模式按几何门裁
/// （ADR 0007），`--bit-depth` 与 `--dither` 各再裁自己那一维。前两道是界，后两道是覆盖项，
/// 但作用方式是同一个——都只从候选集里拿走东西，谁都放不回被拿走的。
///
/// 裁空了就报错：面板显示不出来、或几何上到不了眼睛的那些候选，写出去也是白写，
/// 宁可当场拒绝也不静默照写。
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
/// 几何门此刻还没有卷可判，先当它成立：门那一侧裁空的候选集只有等到第一遍里
/// 那一页出现才拦得住（见 [`first_pass`]）。
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
            "点名的抖动模式越过了几何门：这一卷有页源比目标尺寸还小，按不放大原样输出，\
             阅读器显示时还要再缩一次，抖动推到高频的误差会被折回低频。\
             几何门不成立时抖动整体关闭，--dither 覆盖不了它（ADR 0007）"
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
