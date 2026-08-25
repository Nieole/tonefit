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

mod decode;
mod encode;
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

pub use geometry::Size;
pub use gray::GrayImage;
pub use metric::{Reference, Score, score};
pub use profile::{Panel, Profile};
pub use quantize::{BitDepth, quantize};
pub use report::{CandidateScore, PageReport, Report, VolumeReport};
pub use request::{Mode, Request};

use sink::Sink;
use source::{Member, Volume};

/// 处理点名的若干卷，产出设备优化副本。源卷只读。
pub fn run(request: &Request) -> Result<Report> {
    if request.inputs.is_empty() {
        bail!("处理范围为空：至少点名一个卷（ADR 0009：处理点名的子集）");
    }
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

/// 逐页读、算、写一遍，非图片成员原样搬过去。dry-run 走同一条路，只是不建输出容器。
///
/// ADR 0005 要的是「解码一次 + 缓存缩放后的图」的两遍管线；那要等卷级决策出现才有意义，
/// 见 07 号票。在此之前这里是单遍，卷级决策与缓存都还不存在。
fn process_volume(input: &Path, request: &Request) -> Result<VolumeReport> {
    let panel = request.profile.panel();
    let mut volume = source::open(input)?;
    let output = volume.output_path(&request.output_root);
    let targets = page_targets(&volume);
    ensure_one_member_per_output(&volume, &targets)?;

    // dry-run 一个文件都不落盘，输出容器因此连建都不建（spec 的 story 6）。
    let mut sink = match request.mode {
        Mode::Process => Some(Sink::create(&output, volume.container)?),
        Mode::DryRun => None,
    };
    let mut pages = Vec::with_capacity(volume.pages.len());
    for (page, relative) in volume.pages.iter().zip(&targets) {
        let source = volume.identity(page);
        let bytes = volume.reader.read(page)?;
        let decoded =
            decode::decode(&bytes).with_context(|| format!("解 {} 这一页", source.display()))?;
        let gray = gray::to_gray(&decoded);
        let size = geometry::fit_inside(gray.size(), panel.resolution);
        let scaled = resample::resize(&gray, size)?;
        // 两种模式在这一步分道：处理模式编码写出，dry-run 改为把判据求出来给报告。
        let scores = match &mut sink {
            Some(sink) => {
                let encoded = encode::gray8_png(&scaled)
                    .with_context(|| format!("编 {} 这一页", source.display()))?;
                sink.write_page(relative, &encoded)?;
                Vec::new()
            }
            None => candidate_scores(&Reference::new(panel, scaled), panel),
        };
        pages.push(PageReport {
            source,
            output: output.join(relative),
            size,
            scores,
        });
    }
    if let Some(mut sink) = sink {
        for extra in &volume.extras {
            let bytes = volume.reader.read(extra)?;
            sink.write_extra(&extra.relative, &bytes)?;
        }
        sink.finish()?;
    }

    Ok(VolumeReport {
        volume: volume.root,
        output,
        pages,
    })
}

/// 把这一页的参照与每个候选各比一遍。
///
/// 候选先按面板灰阶数裁一轮再求值，顺序是 ADR 0003 定的：被裁掉的位深不进入候选，
/// 也就不进入判据。据判据选出一档是 06 号票——这里只出量，不选。
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
