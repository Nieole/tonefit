//! 真实素材冒烟：opt-in，没有素材时**明确跳过**。
//!
//! 仓库里不放真实漫画（15 号票最后一条，也是 spec 的 story 35）。素材放在本机的
//! `_samples/`（`.gitignore` 挡着，见 `.scratch/p0-core-pipeline/samples-wanted.md`），
//! 由环境变量指过来：
//!
//! ```text
//! TONEFIT_SAMPLES=_samples cargo test --test smoke
//! ```
//!
//! **这一条自带 harness**（`Cargo.toml` 里 `[[test]] harness = false`），只为一件事：
//! 跳过要说得出口。内建 harness 认得的跳过只有 `#[ignore]` 那个**静态**标记，而这里的条件
//! 是运行时的——环境变量指没指过来，编译期答不出。挂在内建 harness 上，没素材的那一趟印出来
//! 是 `test ... ok`，与真跑过一模一样，读 CI 日志的人会把它当证据。自带 harness 之后跳过
//! 印的是跳过，这个二进制一个「通过」都不计入总数，本次跑没跑一眼看得出。
//!
//! 素材指过来了却指错地方**不是跳过**：那是点名要跑，当场红。
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

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use fixtures::Workspace;
use tonefit::{
    CacheBudget, Filter, FitMode, Gutter, IoMode, Mode, PageBranch, PageReport, Report, Request,
    VolumeReport,
};

/// 指向本机素材目录的环境变量。没设就跳过。
const SAMPLES: &str = "TONEFIT_SAMPLES";

/// 素材目录里点得动的归档扩展名（ADR 0015 的格式集里已经开了的那两个）。
/// 直接子项里带这些扩展名的各算一个卷。
const ARCHIVES: [&str; 2] = ["cbz", "zip"];

/// 归档卷的**输出**扩展名。输入是哪一个都归一成它（ADR 0015），
/// 因此认输出是不是归档只比这一个——一个叫 `第10话.zip` 的**目录**卷的输出仍是目录。
const OUTPUT_ARCHIVE: &str = "cbz";

/// 这一条用例的名字。自带 harness 之后，命令行的过滤词要自己拿它比对，落款也用它。
const NAME: &str = "real_material_runs_through_the_pipeline";

/// 三种结局各印一行，头两个字就分得开：跑过了、跳过了，红了那一路走 panic。
fn main() {
    if !selected() {
        println!("跳过 {NAME}：命令行的过滤词点的是别的用例。");
        return;
    }
    let Some(root) = samples() else {
        println!(
            "跳过 {NAME}：{SAMPLES} 没有指向任何目录，本次一个卷都没跑。\n\
             要跑就把素材放到本机某处，再 `{SAMPLES}=<那个目录> cargo test --test smoke`。"
        );
        return;
    };

    let processed = real_material_runs_through_the_pipeline(&root);

    println!("跑过 {NAME}：{} 下 {processed} 页处理成。", root.display());
}

/// 命令行点的是这一条吗。
///
/// 自带 harness 就得自己认这件事：`cargo test <过滤词>` 把过滤词发给**每一个**测试二进制，
/// 内建 harness 拿它对测试名做子串匹配。这里照同一条规矩办，不然 `cargo test golden`
/// 会顺带把真实素材跑一遍。`--nocapture` 一类的 flag 不是过滤词，不看。
///
/// 一个过滤词都没有就跑；有几个则**任一命中即跑**——内建 harness 的多过滤词是或，不是与。
///
/// **拿不准就跑**：命令行上但凡出现一个 flag，就不按过滤词跳过。libtest 有好几个分离取值的
/// 选项（`--test-threads 4`、`--skip foo`），它们的值不以 `-` 开头，照过滤词认会把 `4`
/// 当成一个谁都不命中的过滤词，于是本该跑的这一趟静悄悄跳过——「本该跑却没跑、还印着一行跳过」
/// 正是这张票要收掉的那种谎。多跑一趟只是慢，跳错一趟是假的证据，两者的代价不对称。
///
/// `--exact`、`--skip` 那几个不认。要单点这一条，用 `cargo test --test smoke`。
fn selected() -> bool {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument.starts_with('-')) {
        return true;
    }
    arguments.is_empty() || arguments.iter().any(|filter| NAME.contains(filter))
}

/// 真实素材跑得下来，产出也合法。返回这一趟真正处理成了的页数。
fn real_material_runs_through_the_pipeline(root: &Path) -> usize {
    let inputs = volumes(root);
    println!("真实素材冒烟：{} 下 {} 个卷", root.display(), inputs.len());

    let space = Workspace::new();
    let report = tonefit::run(&Request {
        inputs: inputs.clone(),
        output_root: space.out(),
        profile: fixtures::baseline_profile(),
        fit: FitMode::default(),
        crop: true,
        split: tonefit::SplitRule::default(),
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
    processed
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

/// 素材目录里的卷：直接子项中的目录与归档，按路径排好。
///
/// 一个都没有时把指过来的这个目录本身当成一个卷——手头只有一卷样本的人会直接指它，
/// 那时报一句「这里没有卷」纯属添堵。
fn volumes(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("列 {}：{error}", root.display()))
        .map(|entry| entry.expect("读目录项").path())
        .filter(|path| path.is_dir() || has_extension(path, &ARCHIVES))
        .collect();
    found.sort();
    if found.is_empty() {
        vec![root.to_owned()]
    } else {
        found
    }
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
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

/// 一卷的**跨页拆分**这一行：跨页候选几页、真切开几页、装订沟长什么样（04 号票的验收）。
///
/// 误报率那条基线说的是**跨页候选那一关**该把单页挡在外面：实测哆啦A梦 15/16、
/// 改革之獸 16/16 命中；棋魂 2/16、画集 1/16 误报，而误报的沟宽 21–24%、真装订沟窄得多
/// （见 measurements 的《跨页拆分》）。这一行把两个数并排印出来——
/// **单页卷上跨页候选那一栏不是 0，就是这道防线漏了**，而漏下去的是不是被沟那一关接住了，
/// 「切开」那一栏答得出来。
///
/// 两个数都从**报告里读**，不在这里拿同一套判据重算一遍：spec 的《Testing Decisions》
/// 定死「Seam 只用 `run(Request) -> Report` 这一个……不新开公开 seam」。
/// 重算一遍的话，这一行量的是它自己，不是管线。
///
/// 沟本身的两个比例也印出来：中心落在页宽的哪儿（实测 0.401–0.538）、占页宽多少
/// （实测 0.17%–12.47%）。这一行是那两个数唯一的现场数据来源。
///
/// 冒烟不断言这些数（见 [`summarize`]）：真实素材各不相同，而「本该是多少」没有一条说得出。
fn spread_line(volume: &VolumeReport) -> String {
    let mut candidates = 0;
    let mut split = 0;
    let mut gutters: Vec<Gutter> = Vec::new();
    // 同一源页切出来的那几张挨着排（页几何批 03 号票），因此按源成员名分组就是按源页分组。
    let mut groups: Vec<(&Path, Vec<&PageReport>)> = Vec::new();
    for page in &volume.pages {
        match groups.last_mut() {
            Some((source, pages)) if *source == page.source.as_path() => pages.push(page),
            _ => groups.push((page.source.as_path(), vec![page])),
        }
    }
    for (_, pages) in &groups {
        if pages[0].spread_candidate() {
            candidates += 1;
        }
        if let Some(cut) = pages[0].cut() {
            split += 1;
            gutters.push(cut.gutter());
        }
    }
    let mut line = format!(
        "跨页 源页 {} · 跨页候选 {candidates} · 切开 {split}",
        groups.len()
    );
    if !gutters.is_empty() {
        let span = |values: Vec<f64>| {
            let low = values.iter().copied().fold(f64::INFINITY, f64::min);
            let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (low, high)
        };
        let (low, high) = span(gutters.iter().map(|gutter| gutter.center()).collect());
        let (thin, thick) = span(gutters.iter().map(|gutter| gutter.share()).collect());
        let _ = write!(
            line,
            " · 沟中心 {low:.3}–{high:.3} · 沟宽 {:.2}%–{:.2}%",
            thin * 100.0,
            thick * 100.0
        );
    }
    line
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
        if !has_extension(&volume.output, &[OUTPUT_ARCHIVE]) {
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

/// 这一趟都发生了什么，印给跑它的人看。
///
/// 自带 harness 不捕获输出，因此不必再 `-- --nocapture`：印了就看得见。
///
/// 冒烟不断言判定，可判定恰恰是跑它的人要看的东西——真实素材上的档位分布，
/// 是 `CONTEXT.md` 的《尚未确立》里那几条目前唯一的现场数据来源。
fn summarize(report: &Report) {
    println!("profile：{}", report.profile);
    println!("跨页拆分：{}", report.split);
    for volume in &report.volumes {
        println!(
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
            println!("    彩色分支 {color} 页");
        }
        println!("    {}", spread_line(volume));
        // 部分救回页在真实素材上是「这份片源下歪了」的现场证据（04 号票），
        // 而它不进隔离目录、也没有退出码替它喊：不印出来，跑冒烟的人无从知道。
        for page in volume.salvaged() {
            println!(
                "    部分救回 {}：{}",
                page.source.display(),
                page.salvage().expect("这是一张部分救回页"),
            );
        }
        for page in volume.failures() {
            println!(
                "    失败页 {}：{}",
                page.source.display(),
                page.failure().expect("这是一张失败页"),
            );
        }
    }
}
