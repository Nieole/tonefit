//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外是两个 seam，其余全部是内部实现。
//!
//! [`run`] 是主 seam：所有模式走同一个入口，CLI 是它之上的薄层，只负责把命令行参数拼成
//! [`Request`]、把 [`Report`] 渲染成文字。
//!
//! [`score`] 是第二个 seam：判据的纯函数形态，数值与性质测试、标定工具直接调它。
//! 它周边的类型——[`Reference`]、[`Score`]、[`GrayImage`]、[`BitDepth`]、[`quantize`]——
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

use anyhow::{Context, Result, bail};

pub use cache::{CacheBudget, CacheUsage};
pub use decide::{CandidateScore, Reason, Verdict};
pub use envelope::Envelope;
pub use geometry::Size;
pub use gray::GrayImage;
pub use metric::{Reference, Score, score};
pub use profile::{Panel, Profile, Threshold};
pub use quantize::{BitDepth, quantize};
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
    ensure_the_override_fits_the_panel(request)?;
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
    let mut cached = first_pass(
        &mut volume,
        request,
        &output,
        &targets,
        &mut cache,
        &mut decoder,
    )?;

    let verdict = summarize_volume(&mut cached, request);

    if let Some(mut sink) = sink {
        second_pass(&cached, &targets, &mut cache, &mut sink)?;
        for extra in &volume.extras {
            let bytes = volume.reader.read(extra)?;
            sink.write_extra(&extra.relative, &bytes)?;
        }
        sink.finish()?;
    }

    Ok(VolumeReport {
        volume: volume.root,
        output,
        pages: cached.into_iter().map(|page| page.report).collect(),
        verdict,
        cache: cache.usage(),
        decodes: decoder.decodes(),
    })
}

/// 汇总：把逐页判定收成卷级的一个基准档（ADR 0006：位深按卷取上包络并加迟滞）。
///
/// 夹在两遍之间——要看完整卷才做得了，而第二遍此刻已经不必回头碰源页（ADR 0005）。
/// 重定过的判定写回每一页，第二遍读的就是它。
///
/// 两条出口走不到上包络，各有各的道理：`--per-page` 是用户点名要逐页最优（决定第 6 条），
/// `--bit-depth` 是判定整个被顶掉、逐页已全是 `Override`——后者不是「被关掉」，
/// 而是逐页结果里根本没有分布可聚合。两者在报告里各说各的，见 [`VolumeVerdict`]。
fn summarize_volume(pages: &mut [CachedPage], request: &Request) -> Option<VolumeVerdict> {
    if pages.is_empty() {
        return None;
    }
    if let Some(bit_depth) = request.bit_depth {
        return Some(VolumeVerdict::Override(bit_depth));
    }
    if request.per_page {
        return Some(VolumeVerdict::PerPage);
    }
    let summary = {
        let inputs: Vec<envelope::Page> = pages
            .iter()
            .map(|page| envelope::Page {
                scores: &page.report.scores,
                decided: page.report.verdict.bit_depth,
            })
            .collect();
        envelope::summarize(&inputs, request.profile.threshold())?
    };
    for (page, verdict) in pages.iter_mut().zip(summary.verdicts) {
        page.report.verdict = verdict;
    }
    Some(VolumeVerdict::Envelope(summary.envelope))
}

/// 第一遍产出的一页：给报告的那一份，加上它在缓存里的序号。
///
/// 序号跟着页走，不由第二遍数数补出来。眼下两者恰好重合，但 ADR 0005 决定第 4 条说
/// 彩页在彩色 profile 下第一遍即写出、**不进灰度缓存**——那一票（10 号）落地当天页序与缓存序
/// 就此分家，而重新数出来的序号会静默地把另一页的像素写到这一页的位置上。
struct CachedPage {
    report: PageReport,
    slot: usize,
}

/// 第一遍：读 → 解码 → 转灰 → 几何与缩放 → 判据曲线 → 判定，同时把参照存进缓存。
///
/// 判据两种模式都求值，dry-run 预告的就是照做时的那一档（spec 的 story 6）。
/// 覆盖了判定也照求：`--dry-run --bit-depth 2` 要说得清「你点的这一档判据是多少」。
///
/// 这里给出的判定是**逐页**的那一档，汇总（[`summarize`]）会把它重定一遍——
/// 上包络与离群页判据要的正是每页都有的这条判据曲线。
fn first_pass(
    volume: &mut Volume,
    request: &Request,
    output: &Path,
    targets: &[PathBuf],
    cache: &mut cache::PageCache,
    decoder: &mut decode::Decoder,
) -> Result<Vec<CachedPage>> {
    let panel = request.profile.panel();
    let mut pages = Vec::with_capacity(volume.pages.len());
    for (page, relative) in volume.pages.iter().zip(targets) {
        let source = volume.identity(page);
        let bytes = volume.reader.read(page)?;
        let decoded = decoder
            .decode(&bytes)
            .with_context(|| format!("解 {} 这一页", source.display()))?;
        let gray = gray::to_gray(&decoded);
        let size = geometry::fit_inside(gray.size(), panel.resolution);
        let (scaled, scaling) = resample::resize(&gray, size, request.filter)?;
        let reference = Reference::new(panel, scaled);
        let scores = candidate_scores(&reference, panel);
        let verdict = decide::decide(&scores, request.profile.threshold(), request.bit_depth);
        let slot = cache
            .store(reference.image())
            .with_context(|| format!("缓存 {} 这一页", source.display()))?;
        pages.push(CachedPage {
            report: PageReport {
                source,
                output: output.join(relative),
                size,
                scaling,
                scores,
                verdict,
            },
            slot,
        });
    }
    Ok(pages)
}

/// 第二遍：从缓存读 → 量化 → 编码 → 写出。不再碰源页（ADR 0005）。
fn second_pass(
    pages: &[CachedPage],
    targets: &[PathBuf],
    cache: &mut cache::PageCache,
    sink: &mut Sink,
) -> Result<()> {
    for (page, relative) in pages.iter().zip(targets) {
        let source = page.report.source.display();
        let reference = cache
            .load(page.slot)
            .with_context(|| format!("从缓存取 {source} 这一页"))?;
        let bit_depth = page.report.verdict.bit_depth;
        let quantized = quantize::quantize(&reference, bit_depth);
        let encoded =
            encode::png(&quantized, bit_depth).with_context(|| format!("编 {source} 这一页"))?;
        sink.write_page(relative, &encoded)?;
    }
    Ok(())
}

/// 把这一页的参照与每个候选各比一遍。
///
/// 候选先按面板灰阶数裁一轮再求值，顺序是 ADR 0003 定的：被裁掉的位深不进入候选，
/// 也就不进入判据。这里只出量，拿量去和阈值比在 `decide`。
/// 候选此刻只有位深这一维，抖动模式那一维随 09 号票加进来。
fn candidate_scores(reference: &Reference, panel: Panel) -> Vec<CandidateScore> {
    BitDepth::candidates(panel.gray_levels)
        .into_iter()
        .map(|bit_depth| CandidateScore {
            bit_depth,
            score: metric::score(reference, &quantize::quantize(reference.image(), bit_depth)),
        })
        .collect()
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

/// `--bit-depth` 覆盖的是自动判定，不是面板灰阶数那道硬上界（ADR 0003）。
///
/// 面板显示不出来的位深，写出去也到不了眼睛，因此宁可当场拒绝也不静默照写。
/// 上界本身只有 `--gray-levels` 动得了，错误信息于是指向它。
fn ensure_the_override_fits_the_panel(request: &Request) -> Result<()> {
    let Some(bit_depth) = request.bit_depth else {
        return Ok(());
    };
    let panel = request.profile.panel();
    let candidates = BitDepth::candidates(panel.gray_levels);
    if !candidates.contains(&bit_depth) {
        let listed = candidates
            .iter()
            .map(BitDepth::to_string)
            .collect::<Vec<_>>()
            .join("、");
        bail!(
            "{bit_depth} 越过了面板的 {} 级灰阶：这块面板上写得出的是 {listed}。\
             真要写 {bit_depth}，先按实测用 --gray-levels 抬高上界",
            panel.gray_levels
        );
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
