//! 真实素材冒烟：opt-in，默认跳过。
//!
//! 仓库里不放真实漫画（15 号票最后一条，也是 spec 的 story 35）。素材放在本机的
//! `_samples/`（`.gitignore` 挡着，见 `.scratch/p0-core-pipeline/samples-wanted.md`），
//! 由环境变量指过来：
//!
//! ```text
//! TONEFIT_SAMPLES=_samples cargo test --test smoke -- --nocapture
//! ```
//!
//! **环境变量没设时这一条什么都不做、照样通过**：它不该是任何一次普通 `cargo test` 的负担。
//!
//! 断言只有两件事：**不崩溃**，以及**产出合法**——写出来的每一页都解得回来，尺寸与
//! 报告说的对得上，至少有一页真的处理成了。具体体积、具体判定一概不断言：真实素材各不
//! 相同，那样的断言换一批素材就得重写，而且没有一条说得出「本该是多少」。那些数由黄金
//! 回归在合成夹具上钉着（见 `tests/golden.rs`）。
//!
//! 这一条的价值全在合成夹具够不着的地方：真实的有损转码伪影、真实的成员名与编码、
//! 真实的页数与尺寸分布。它捞不出「判定得对不对」，只捞得出「跑不跑得下来」——
//! 而那正是 `CONTEXT.md` 的《尚未确立》列着的那几个洞目前唯一的现场反馈。

mod fixtures;

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use fixtures::Workspace;
use tonefit::{
    CacheBudget, Filter, IoMode, Mode, PageBranch, PageOutcome, PageReport, Report, Request,
    VolumeReport,
};

/// 指向本机素材目录的环境变量。没设就跳过。
const SAMPLES: &str = "TONEFIT_SAMPLES";

/// 归档卷的扩展名。素材目录里的 `.cbz` 各算一个卷。
const ARCHIVE: &str = "cbz";

#[test]
fn real_material_runs_through_the_pipeline() {
    let Some(root) = samples() else {
        eprintln!(
            "跳过真实素材冒烟：{SAMPLES} 没有指向任何目录。\
             要跑就把素材放到本机某处，再 `{SAMPLES}=<那个目录> cargo test --test smoke`。"
        );
        return;
    };

    let inputs = volumes(&root);
    eprintln!("真实素材冒烟：{} 下 {} 个卷", root.display(), inputs.len());

    let space = Workspace::new();
    let report = tonefit::run(&Request {
        inputs: inputs.clone(),
        output_root: space.out(),
        profile: fixtures::baseline_profile(),
        filter: Filter::default(),
        bit_depth: None,
        dither: None,
        per_page: false,
        cache_budget: CacheBudget::default(),
        mode: Mode::Process,
        io_mode: IoMode::default(),
        progress: None,
        metadata: true,
    })
    .unwrap_or_else(|error| panic!("真实素材没跑下来：{error:#}"));

    assert_eq!(
        report.volumes.len(),
        inputs.len(),
        "点名了 {} 个卷，报告里只有 {} 个",
        inputs.len(),
        report.volumes.len()
    );

    let processed = report.volumes.iter().map(check_volume).sum::<usize>();
    summarize(&report);

    // 一页都没处理成也可能一路 `Ok`：整批素材都读不出来时，每一卷都是清一色的占位页。
    // 那时这一趟什么都没验证到，而它看上去与验证过了一模一样。
    assert!(
        processed > 0,
        "{} 下一页都没有处理成：素材要么全坏了，要么根本不是图片",
        root.display()
    );
}

/// 素材目录。环境变量没设、或设成空串就是 `None`。
///
/// 指过来的路径不存在**不是**跳过：那是点名要跑却指错了地方，静悄悄通过等于骗人。
fn samples() -> Option<PathBuf> {
    let value = std::env::var_os(SAMPLES)?;
    if value.is_empty() {
        return None;
    }
    let root = PathBuf::from(value);
    assert!(
        root.is_dir(),
        "{SAMPLES} 指向 {}，而那不是一个目录",
        root.display()
    );
    Some(root)
}

/// 素材目录里的卷：直接子项中的目录与 `.cbz`，按路径排好。
///
/// 一个都没有时把指过来的这个目录本身当成一个卷——手头只有一卷样本的人会直接指它，
/// 那时报一句「这里没有卷」纯属添堵。
fn volumes(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("列 {}：{error}", root.display()))
        .map(|entry| entry.expect("读目录项").path())
        .filter(|path| path.is_dir() || is_archive(path))
        .collect();
    found.sort();
    if found.is_empty() {
        vec![root.to_owned()]
    } else {
        found
    }
}

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(ARCHIVE))
}

/// 一个卷的产出合法吗。返回这一卷真正处理成了的页数。
fn check_volume(volume: &VolumeReport) -> usize {
    let mut written = Written::open(volume);
    let mut processed = 0;
    for page in &volume.pages {
        let bytes = written.page(volume, page);
        let png = fixtures::read_png_bytes(&bytes);
        assert_eq!(
            png.size,
            page.size,
            "{} 写出来是 {}，报告说的是 {}",
            page.output.display(),
            png.size,
            page.size
        );
        // 灰度路径写出的位宽不该超过判定那一档：调色板可以更窄（一页用不满格点时），
        // 但更宽就意味着判定与写出的对不上（ADR 0004：颜色类型在编码器接口以内）。
        if let Some(verdict) = page.verdict() {
            assert!(
                fixtures::written_bits(png.bit_depth) <= verdict.candidate.bit_depth.bits(),
                "{} 判定 {} 却按 {} 位写出",
                page.output.display(),
                verdict.candidate,
                fixtures::written_bits(png.bit_depth)
            );
        }
        if page.failure().is_none() {
            processed += 1;
        }
    }
    processed
}

/// 一个卷写出来的页从哪儿取：目录卷从文件里，归档卷从归档里。
///
/// 归档卷的 `PageReport::output` 不是一个打得开的路径（它是 `卷.cbz/001.png` 这个身份），
/// 因此归档这一支按成员名取——名字就是它相对卷输出的那一段，分隔符归一成 `/`
/// （`fixtures::relative_name`，与 `sink` 写进去时的算法同一个）。
///
/// 不走 `fixtures::read_cbz`：那一个把整个归档摊进内存，而这里的卷是**真实素材**，
/// 一卷几百兆很正常。归档在这里开一次、按名字逐页取。
enum Written {
    Directory,
    Archive(zip::ZipArchive<BufReader<File>>),
}

impl Written {
    fn open(volume: &VolumeReport) -> Self {
        if !is_archive(&volume.output) {
            return Written::Directory;
        }
        let file = File::open(&volume.output)
            .unwrap_or_else(|error| panic!("打开 {}：{error}", volume.output.display()));
        let archive = zip::ZipArchive::new(BufReader::new(file))
            .unwrap_or_else(|error| panic!("读 {} 的结构：{error}", volume.output.display()));
        Written::Archive(archive)
    }

    fn page(&mut self, volume: &VolumeReport, page: &PageReport) -> Vec<u8> {
        match self {
            Written::Directory => std::fs::read(&page.output)
                .unwrap_or_else(|error| panic!("读 {}：{error}", page.output.display())),
            Written::Archive(archive) => {
                let name = fixtures::relative_name(&volume.output, &page.output);
                let mut member = archive
                    .by_name(&name)
                    .unwrap_or_else(|error| panic!("取归档成员 {name}：{error}"));
                let mut bytes = Vec::new();
                member.read_to_end(&mut bytes).expect("读归档成员");
                bytes
            }
        }
    }
}

/// 这一趟都发生了什么，印给跑它的人看（`-- --nocapture`）。
///
/// 冒烟不断言判定，可判定恰恰是跑它的人要看的东西——真实素材上的档位分布，
/// 是 `CONTEXT.md` 的《尚未确立》里那几条目前唯一的现场数据来源。
fn summarize(report: &Report) {
    eprintln!("profile：{}", report.profile);
    for volume in &report.volumes {
        eprintln!(
            "  {} · {} 页{}\n    {}",
            volume
                .volume
                .file_name()
                .unwrap_or(volume.volume.as_os_str())
                .to_string_lossy(),
            volume.page_count(),
            if volume.isolated() { " · 隔离" } else { "" },
            // 与黄金回归共用同一份说法（`CONTEXT.md` 的用词），两处不会走散。
            fixtures::volume_verdict(volume),
        );
        let color = volume
            .pages
            .iter()
            .filter(|page| matches!(page.branch(), Some(PageBranch::Color)))
            .count();
        if color > 0 {
            eprintln!("    彩色分支 {color} 页");
        }
        for page in volume.failures() {
            eprintln!("    失败页 {}：{}", page.source.display(), reason(page));
        }
    }
}

fn reason(page: &PageReport) -> &str {
    match &page.outcome {
        PageOutcome::Failed { reason } => reason,
        PageOutcome::Processed { .. } => "",
    }
}
