//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外只有一个入口：[`run`]。CLI 是它之上的薄层，只负责把命令行参数拼成 [`Request`]、
//! 把 [`Report`] 渲染成文字。第二个 seam `score` 随判据落地（04 号票）。

mod decode;
mod encode;
mod geometry;
mod gray;
mod profile;
mod report;
mod request;
mod resample;
mod source;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub use geometry::Size;
pub use profile::{Panel, Profile};
pub use report::{PageReport, Report, VolumeReport};
pub use request::Request;

/// 处理点名的若干卷，产出设备优化副本。源卷只读。
pub fn run(request: &Request) -> Result<Report> {
    if request.inputs.is_empty() {
        bail!("处理范围为空：至少点名一个卷（ADR 0009：处理点名的子集）");
    }
    for input in &request.inputs {
        ensure_output_is_elsewhere(input, &request.output_root)?;
    }
    let panel = request.profile.panel();
    let volumes = request
        .inputs
        .iter()
        .map(|input| process_volume(input, &request.output_root, panel))
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        profile: request.profile.clone(),
        volumes,
    })
}

/// 逐页读、算、写一遍。
///
/// ADR 0005 要的是「解码一次 + 缓存缩放后的图」的两遍管线；那要等判据出现才有意义，
/// 见 07 号票。在此之前这里是单遍，卷级决策与缓存都还不存在。
fn process_volume(input: &Path, output_root: &Path, panel: Panel) -> Result<VolumeReport> {
    let volume = source::open(input)?;
    let output = output_root.join(&volume.name);
    let targets: Vec<PathBuf> = volume
        .pages
        .iter()
        .map(|page| output.join(page.relative.with_extension("png")))
        .collect();
    ensure_one_output_per_page(&volume.pages, &targets)?;

    let mut pages = Vec::with_capacity(volume.pages.len());
    for (page, target_path) in volume.pages.iter().zip(targets) {
        let decoded = decode::decode(&page.path)?;
        let gray = gray::to_gray(&decoded);
        let size = geometry::fit_inside(gray.size(), panel.resolution);
        let scaled = resample::resize(&gray, size)?;
        encode::write_gray8_png(&target_path, &scaled)?;
        pages.push(PageReport {
            source: page.path.clone(),
            output: target_path,
            size,
        });
    }
    Ok(VolumeReport {
        volume: volume.root,
        output,
        pages,
    })
}

/// 扩展名一律换成 png，`001.jpg` 与 `001.png` 于是撞在同一个输出上。
/// 撞了就报错——静默覆盖会让 `Report` 里两页指向同一个文件。
fn ensure_one_output_per_page(pages: &[source::SourcePage], targets: &[PathBuf]) -> Result<()> {
    let mut taken: HashMap<&Path, &Path> = HashMap::with_capacity(targets.len());
    for (page, target) in pages.iter().zip(targets) {
        if let Some(previous) = taken.insert(target, page.path.as_path()) {
            bail!(
                "{} 与 {} 都要写到 {}：请让同一卷内的页名互不冲突",
                previous.display(),
                page.path.display(),
                target.display()
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
