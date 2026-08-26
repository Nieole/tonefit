//! CLI：把命令行参数拼成 `Request`，把 `Report` 渲染成文字。此外不做别的事。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tonefit::{
    BitDepth, CacheBudget, CandidateScore, Dither, Filter, IoMode, Mode, PageBranch, PageColor,
    PageReport, Profile, Progress, ProgressSink, Report, Request, VolumeReport, VolumeVerdict,
    aggregation,
};

#[derive(Parser)]
// 不点子命令就是「处理点名的若干卷」这一件事，那是绝大多数时候要做的：
// `args_conflicts_with_subcommands` 把两种用法分开，`subcommand_negates_reqs`
// 让子命令不必再交出处理卷才要的那几个必填项。
#[command(
    about = "把漫画页适配到电子墨水阅读设备",
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    /// 另做一件事，而不是处理卷。
    #[command(subcommand)]
    command: Option<Command>,

    /// 要处理的卷：一个目录，或一个 CBZ。源只读。
    #[arg(required = true, value_name = "卷")]
    inputs: Vec<PathBuf>,

    /// 输出根目录。每个卷在它下面得到一份同名副本，容器形态与输入一致。
    #[arg(short, long, required = true, value_name = "目录")]
    out: Option<PathBuf>,

    /// 目标设备型号。内置表覆盖 Kobo、BOOX、Kindle 的主力型号，型号名不区分大小写与分隔符。
    #[arg(short, long, required = true, value_name = "型号")]
    profile: Option<String>,

    /// 覆盖面板灰阶数。内置表没收录的设备、或在真机上数出的实际可分辨级数走这里。
    #[arg(long, value_name = "级数")]
    gray_levels: Option<u32>,

    /// 覆盖判定用的阈值。内置值由两块面板上的盲测定出，**没有逐面板复核**——
    /// 判据跟着面板走、不可跨面板比较（ADR 0002）。在自己那台设备上盲测出来的界填这里：
    /// 拿 `--dry-run` 逐页读出各档判据值，把同一页的各档拷进设备目视，
    /// 记下最低的那个**不可接受**档的判据值，界取在它之下。
    #[arg(long, value_name = "界")]
    threshold: Option<f32>,

    /// 残差段的重采样滤波器：area（= box）、bilinear、hamming、bicubic、lanczos3，默认 lanczos3。
    /// 只作用于残差段——总缩放比 ≥ 2 时的整数倍预缩那一级恒为 box。
    #[arg(long, value_name = "滤波器")]
    filter: Option<String>,

    /// 覆盖自动判定的位深：1、2、4、8。面板灰阶数那道上界仍在，越界的覆盖会被拒绝。
    #[arg(long, value_name = "位深")]
    bit_depth: Option<u32>,

    /// 覆盖自动选择的抖动模式：off（= none）、fs（= floyd-steinberg）。
    /// 抖动只在输出不被下游缩放时才谈得上，而这是**每一页**各自的事实：
    /// 卷里只要有一页源比目标尺寸小，点名 fs 就会被**拒绝**——错误指得出是哪一页，
    /// 不会静默照抖。
    #[arg(long, value_name = "模式")]
    dither: Option<String>,

    /// 关闭卷级上包络与迟滞，位深回到逐页最优。体积最小，代价是**重新引入翻页跳变**：
    /// 相邻两页会落到不同档上，翻过去的一瞬间灰调的颗粒感换一种粗细。
    #[arg(long)]
    per_page: bool,

    /// 两遍之间的缓存最多在内存里留多少：纯字节数，或带 K/M/G 后缀，默认 512M。
    /// 超出的页溢写临时文件，运行结束即收走。
    #[arg(long, value_name = "字节数")]
    cache_budget: Option<String>,

    /// 读取策略：auto（按路径探测介质，默认）、serial（读取串行）、concurrent（读取并发）。
    /// auto 下有寻道惩罚的盘串行读、没有的并发读；网络路径与探不出来的一律按未知退到串行。
    #[arg(long, value_name = "模式")]
    io_mode: Option<String>,

    /// 只算不写：报告照出，逐页给出判定与各候选的判据值，一个文件都不落盘。
    #[arg(long)]
    dry_run: bool,

    /// 不把自描述元数据写进输出 PNG。**幂等能力随之关闭**：判定与理由不再随文件走，
    /// 重跑时也无从判断这一卷变没变，每一趟都整卷重做。
    #[arg(long)]
    no_metadata: bool,
}

impl Cli {
    /// 本次做到哪一步。
    fn mode(&self) -> Mode {
        if self.dry_run {
            Mode::DryRun
        } else {
            Mode::Process
        }
    }

    /// 本次残差段用哪个滤波器。不点名就是默认的 lanczos3（ADR 0001）。
    fn residual_filter(&self) -> Result<Filter> {
        match &self.filter {
            Some(name) => Filter::resolve(name),
            None => Ok(Filter::default()),
        }
    }

    /// 本次要不要点名位深。不点名就由判据说了算。
    fn bit_depth_override(&self) -> Result<Option<BitDepth>> {
        self.bit_depth.map(BitDepth::from_bits).transpose()
    }

    /// 本次要不要点名抖动模式。不点名就由判据在几何门放行的那几种里选。
    fn dither_override(&self) -> Result<Option<Dither>> {
        self.dither.as_deref().map(Dither::resolve).transpose()
    }

    /// 本次的缓存预算。不点名就是默认的那一档。
    fn cache_budget(&self) -> Result<CacheBudget> {
        match &self.cache_budget {
            Some(text) => CacheBudget::parse(text),
            None => Ok(CacheBudget::default()),
        }
    }

    /// 本次的读取策略。不点名就按路径探测介质（ADR 0009）。
    fn io_mode(&self) -> Result<IoMode> {
        match &self.io_mode {
            Some(text) => IoMode::resolve(text),
            None => Ok(IoMode::default()),
        }
    }

    /// 把 `--profile`、`--gray-levels` 与 `--threshold` 合成本次要用的 profile。
    fn target_profile(&self) -> Result<Profile> {
        target_profile(
            self.profile.as_deref().expect(REQUIRED_BY_CLAP),
            self.gray_levels,
            self.threshold,
        )
    }
}

/// 处理卷那一路的必填项走到这里就一定有值：`required = true` 挡在 clap 那一层。
///
/// 字段类型仍是 `Option`——`subcommand_negates_reqs` 让子命令那一路不必交出它们
/// （`calibrate` 根本不收输出根与卷），字段就得容得下「没有」。
/// 必填这道关因此留在 clap：错在哪、该怎么敲，它说得比这里好。
const REQUIRED_BY_CLAP: &str = "clap 的 required = true 已经挡在前面";

/// 处理卷之外的那些事，各占一个子命令。
#[derive(clap::Subcommand)]
enum Command {
    /// 生成灰阶阶梯标定图，在设备上目视数出感知可分辨级数——它不等于面板的物理灰阶数。
    ///
    /// 那句话在头一行，为的是 `-h` 也说得到：短帮助只印这一行，而它正是最容易被误读的一条。
    ///
    /// 图按目标面板的分辨率排布，并排给出各候选位深的阶梯，每一级标着自己的号，
    /// 判读说明印在图内——图拷进设备就能用，不必对着文档看。
    ///
    /// 用法：把图拷进设备，以**原尺寸**打开（关掉缩放与适配屏幕），
    /// 数出**最右**那条阶梯里你还分得开几级——它最细，其余几条只作对照；
    /// 再把那个数回填给 `--gray-levels`（ADR 0003：面板灰阶数是位深的硬上界，
    /// 它在判据之前裁掉候选位深）。
    ///
    /// **数出来的是感知可分辨级数，不等于面板的物理灰阶数。** 两者不必相等——
    /// 显示固件的处理、环境光、观看距离都会改变你数得出几级，而 `--gray-levels`
    /// 填的正是前者：判定要贴合你**实际看到**的效果，不是贴合规格表。
    ///
    /// 标定图本身不经过位深判定：它是量具，不是被处理的页。像素以 8 位工作精度画出，
    /// 无损写出，不带自描述元数据。
    Calibrate {
        /// 目标设备型号。与处理卷时同一张内置表，型号名不区分大小写与分隔符。
        #[arg(short, long, value_name = "型号")]
        profile: String,

        /// 覆盖面板灰阶数。上一趟数出来的级数填这里，图跟着只排这台设备真会用到的那几档。
        #[arg(long, value_name = "级数")]
        gray_levels: Option<u32>,

        /// 标定图写到哪个文件。父目录不在就建出来。
        #[arg(short, long, value_name = "文件")]
        out: PathBuf,
    },
}

/// 把型号名与各覆盖项合成一个 profile。
///
/// 处理卷与 `calibrate` 共用它：两边解析出的必须是同一块面板，
/// 不然标定图量的是一块、判定用的是另一块。
///
/// 阈值是这里唯一一项 `calibrate` 用不上的：标定图是量具，不经判定（它恒传 `None`）。
fn target_profile(
    device: &str,
    gray_levels: Option<u32>,
    threshold: Option<f32>,
) -> Result<Profile> {
    let mut profile = Profile::resolve(device)?;
    if let Some(gray_levels) = gray_levels {
        profile = profile.with_gray_levels(gray_levels)?;
    }
    if let Some(threshold) = threshold {
        profile = profile.with_threshold(threshold)?;
    }
    Ok(profile)
}

/// 有卷被隔离时的退出码。
///
/// 与「拒绝执行」分成两个数（12 号票：退出码要分得开「全部成功」与「有卷被隔离」）：
/// 隔离过的那一趟**做完了**——输出齐着、报告齐着，只是其中几卷带着坏页。
/// 脚本据此可以选择忽略，也可以停下来看一眼，而 `1` 只说得出「这一趟没做成」。
const ISOLATED_EXIT: u8 = 2;

/// 全部成功的退出码。
const SUCCESS_EXIT: u8 = 0;

fn main() -> ExitCode {
    match execute() {
        Ok(code) => ExitCode::from(code),
        // `Result<()>` 那个 main 印的就是这一行，退出码 1。自己拿 `ExitCode` 之后照印。
        Err(error) => {
            eprintln!("Error: {error:?}");
            ExitCode::FAILURE
        }
    }
}

/// 这一趟的退出码：全部成功是 [`SUCCESS_EXIT`]，有卷被隔离是 [`ISOLATED_EXIT`]。
///
/// 拒绝执行那一种走不到这里——它在 `run` 里就返回了 `Err`，退出码是 `ExitCode::FAILURE`（1）。
/// 三个数因此各说一件事：做完了、做完了但有卷带着坏页、没做成。
///
/// 出的是 `u8` 而不是 `ExitCode`：后者不可比较，这条规则也就测不了，
/// 而「退出码分得开这两种」正是本票要钉住的那一条。
fn exit_code(report: &Report) -> u8 {
    if report.any_isolated() {
        ISOLATED_EXIT
    } else {
        SUCCESS_EXIT
    }
}

fn execute() -> Result<u8> {
    let cli = Cli::parse();
    if let Some(Command::Calibrate {
        profile,
        gray_levels,
        out,
    }) = &cli.command
    {
        return calibrate(profile, *gray_levels, out);
    }
    let profile = cli.target_profile()?;
    let filter = cli.residual_filter()?;
    let bit_depth = cli.bit_depth_override()?;
    let dither = cli.dither_override()?;
    let cache_budget = cli.cache_budget()?;
    let io_mode = cli.io_mode()?;
    let mode = cli.mode();
    let bar = Bar::new();
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out.expect(REQUIRED_BY_CLAP),
        profile,
        filter,
        bit_depth,
        dither,
        per_page: cli.per_page,
        cache_budget,
        mode,
        io_mode,
        progress: Some(ProgressSink::new(bar)),
        metadata: !cli.no_metadata,
    })?;
    print!("{}", render(&report, mode));
    Ok(exit_code(&report))
}

/// 画一张灰阶阶梯标定图并写到 `out`（14 号票）。
///
/// 这一趟不读源、不写输出根、不判定任何东西：标定图是量具，管线一整套都不在场。
/// 因此也没有「有卷被隔离」那种结局——写成了就是 [`SUCCESS_EXIT`]，写不成是 `Err`。
fn calibrate(device: &str, gray_levels: Option<u32>, out: &Path) -> Result<u8> {
    let profile = target_profile(device, gray_levels, None)?;
    let chart = tonefit::calibration_chart(&profile)?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("建标定图的去处 {}", parent.display()))?;
    }
    std::fs::write(out, &chart).with_context(|| format!("写标定图 {}", out.display()))?;
    print!("{}", calibration_note(&profile, out));
    Ok(SUCCESS_EXIT)
}

/// 标定图写出去之后印的那几行：图在哪儿，以及**此刻**要做对的那一件事。
///
/// 只说这一件。怎么数、数出来的数是什么意思，图内印着，`--help` 里也写着——
/// 同一套说法在终端上再抄一遍，改的时候就得记着改三处。
/// 留下的那一条之所以在这里，是因为它在别处已经来不及：图一旦被缩着显示过，
/// 数出来的就不是这块面板了，而用户正是在这一刻决定怎么打开它。
///
/// 面板规格不重复——头一行的 `profile` 里已经有了。
fn calibration_note(profile: &Profile, out: &Path) -> String {
    format!(
        "profile {profile}\n\
         标定图 {}\n  \
         拷进设备，以原尺寸打开：关掉缩放，也关掉适配屏幕——\
         图被缩过一次，数出来的就不是这块面板了\n  \
         怎么数印在图内（大写英文，中文字模装不进一张位图）；\
         完整说法见 tonefit calibrate --help\n",
        out.display(),
    )
}

fn render(report: &Report, mode: Mode) -> String {
    let mut text = format!("profile {}\n", report.profile);
    // 逐页那些「判据 …」的数都是这套取法收出来的，而取法里的 K 还没标定。
    // 它与阈值同一个待遇：数摆出来，没标定这件事跟着摆出来（ADR 0002 决定第 3 条）。
    // 它自成一行、不接在 profile 后面——判据聚合眼下对所有 profile 都一样，不是这台设备的事。
    // 行首是「判据聚合」而不是「判据」：逐页那一行的「判据」说的是**量**，两者不许同名。
    text.push_str(&format!("判据聚合 {}\n", aggregation()));
    if mode == Mode::DryRun {
        text.push_str("dry-run：只算不写，下面的路径都还没落盘\n");
    }
    for volume in &report.volumes {
        text.push_str(&format!(
            "{} → {}（{} 页{}）\n",
            volume.volume.display(),
            volume.output.display(),
            volume.page_count(),
            color_page_note(volume)
        ));
        text.push_str(&superseded_line(volume));
        text.push_str(&volume_lines(volume));
        // 这一卷是怎么读的（13 号票）。它排在跳过那一支**之前**：幂等命中的卷同样把整卷的字节
        // 读了一遍，读法与做事的那一趟是同一个，而「跳过一卷为什么也要等这么久」正问在这里。
        text.push_str(&format!("  {}\n", volume.io));
        // 跳过的卷什么都没做：缓存用量与逐页结果无从谈起，`volume_lines` 那一行已经说完了。
        if volume.skipped() {
            continue;
        }
        // 卷成为不可分割的处理单元，峰值内存随卷大小走（ADR 0005）：这一行是那条代价的现场。
        text.push_str(&format!("  缓存 {}\n", volume.cache));
        for page in &volume.pages {
            text.push_str(&format!(
                "  {}  {}  {}\n",
                page.size,
                scaling_note(page),
                page.output.display()
            ));
            text.push_str(&format!("    {}\n", page_line(page)));
        }
    }
    text.push_str(&salvage_tail(report));
    text.push_str(&isolation_tail(report));
    text
}

/// 隔离那一小结，摆在整份报告的末尾。
///
/// 逐页那几行已经把每一个失败页与原因说过一遍了；这一行是给长任务备的：几十卷跑下来，
/// 失败页早滚出屏幕了，而「这一趟到底有没有出事」得有一个不用往回翻的答案。
/// 退出码说的是同一件事（见 [`exit_code`]），只是那一个给脚本读、这一行给人读。
/// 一卷都没被隔离就一个字都不说。
fn isolation_tail(report: &Report) -> String {
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.isolated())
        .count();
    if volumes == 0 {
        return String::new();
    }
    format!(
        "隔离 {volumes} 卷 · 失败 {} 页：失败页以卷内统一尺寸留白占位，原因逐条列在上面\n",
        report.failures().count()
    )
}

/// 部分救回那一小结，与隔离那一小结并排摆在报告末尾（04 号票）。
///
/// 它比隔离那一行更需要这个位置：含失败页的卷有退出码替它喊，也有一个隔离目录摆在那儿；
/// 部分救回页两样都没有——卷照常落在干净的去处，退出码是 0，而源文件确实不全。
/// 几十卷跑下来，逐页那几行早滚出屏幕了，不在末尾说一句就等于没说。
///
/// 这一行只报数，不重复「它们没参与卷级的哪两件事」——那句话在卷级那一行上，
/// 而这一行与它出现在同一份报告里（见 [`salvaged_line`]）。
///
/// 一页都没有就一个字都不说，与隔离那一行同一条规矩。
fn salvage_tail(report: &Report) -> String {
    let pages = report.salvaged().count();
    if pages == 0 {
        return String::new();
    }
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.salvaged().next().is_some())
        .count();
    format!("部分救回 {volumes} 卷 · {pages} 页：源文件不全，缺的那一段留成纸白\n")
}

/// 过期副本那一行（12 号票）。
///
/// 卷的去处随「有没有失败页」在干净目录与隔离目录之间跳，而这一趟写不到的那一份不会被覆盖、
/// 也不会被删。它可能是**一整卷白页**的占位输出——摆在文件管理器里与一本正经的书没有分别。
/// 报告因此要指名道姓地说出它在哪儿，删不删由用户定。
///
/// 这一行排在卷级各行之前：它说的不是这一趟做了什么，而是上一趟留下了什么。
fn superseded_line(volume: &VolumeReport) -> String {
    match &volume.superseded {
        Some(path) => format!(
            "  过期副本 {}：上一趟写在那儿，这一趟没有覆盖它。\
             那一份当初若是被隔离过的，它整卷都是白页——删不删由你\n",
            path.display()
        ),
        None => String::new(),
    }
}

/// 卷级那一段里说部分救回的那一行，排在隔离那一行之后（04 号票）。
///
/// 隔离那一行说的是「这一卷有页根本没出来」，这一行说的是「有页出来了，但不全」。
/// 两句分开，因为后果不同：前者整卷换了去处，后者没有——这一卷仍在干净的去处，
/// 而卷级的档是在**没有**这几页的情况下定出来的，那正是这一行要交代的事。
fn salvaged_line(volume: &VolumeReport) -> String {
    let pages = volume.salvaged().count();
    if pages == 0 {
        return String::new();
    }
    format!(
        "  部分救回 {pages} 页：整解失败，按文件头的尺寸救回了一段，缺的那一段留成纸白。\
         它们不参与卷级上包络，各自单独定档。几何门照旧问它们——那是文件头里的真尺寸；\
         门在哪一页上也不成立，那一页就改按门那一条来（见上）\n"
    )
}

/// 一页那一行里说缩放的那一小截。
///
/// 失败页没有缩放可说——它没被缩放过（ADR 0001 那三个数一个都不成立）。
/// 那一格于是改说它的尺寸是从哪来的：卷内统一，不是它自己的。
fn scaling_note(page: &PageReport) -> String {
    match page.scaling() {
        Some(scaling) => scaling.to_string(),
        None => "失败页 · 卷内统一尺寸留白".to_owned(),
    }
}

/// 幂等命中而跳过的卷那一行。
///
/// 「跳过」本身不够——用户要能分清「这一卷没变」与「工具没做事」。四项依据点名摆出来，
/// 改了其中哪一项会让它重做，一眼看得见（spec 的 story 8、story 9）。
const SKIPPED_LINE: &str =
    "  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做\n";

/// 卷那一行里说彩页有几张的那一小截。
///
/// 一张都没有就不说——绝大多数卷是这个样子（见 measurements 的《B 类素材普查》：97% 近灰度），
/// 每卷都挂一句「彩页 0 页」只是噪声。数的是**彩页**，与它走了哪条分支无关。
fn color_page_note(volume: &VolumeReport) -> String {
    let count = volume
        .pages
        .iter()
        .filter(|page| page.color() == Some(PageColor::Color))
        .count();
    if count == 0 {
        String::new()
    } else {
        format!("，其中彩页 {count} 页")
    }
}

/// 卷级那一段：几何门的判定结果，加上这一卷的候选从哪来。
///
/// 「这卷为什么是这个候选」要有一个指得出驱动页的答案（ADR 0006），这几行就是它。
/// 上包络不在场时说清是为什么不在场——那正是翻页跳变回来的时候，报告不能看起来还是一样。
fn volume_lines(volume: &VolumeReport) -> String {
    // 一页都没有的卷只装着透传文件，没有候选可判，几何门也就无从谈起。
    let Some(verdict) = &volume.verdict else {
        return String::new();
    };
    // 跳过的卷同样没有几何门可说——它一页都没算。这一支要排在 `gate_line` 之前：
    // 那里读的 `volume.gate` 只有算过的卷才有。
    if volume.skipped() {
        return SKIPPED_LINE.to_owned();
    }
    let mut text = isolated_line(volume);
    text.push_str(&salvaged_line(volume));
    text.push_str(&gate_line(volume, verdict));
    text.push_str(&match verdict {
        VolumeVerdict::Envelope(envelope) => format!(
            "  卷级 {envelope}\n    驱动页 {}\n",
            volume.pages[envelope.driver].source.display()
        ),
        VolumeVerdict::Override(candidate) => format!(
            "  卷级 判定 {candidate}（覆盖项裁到只剩一个候选）：判定被顶掉，卷级基准档无从谈起\n"
        ),
        VolumeVerdict::PerPage => {
            "  卷级 无（--per-page）：上包络与迟滞关着，候选逐页最优，翻页处会换档\n".to_owned()
        }
        // 上面那一支已经把跳过的卷送走了。
        VolumeVerdict::Skipped { .. } => String::new(),
    });
    text
}

/// 被隔离的卷那一行，排在卷级各行之首（12 号票：含失败页的卷被标记）。
///
/// 卷那一行里的去处已经指着隔离目录了，但那要用户认得出那个目录名才读得懂。
/// 这一行把话说完：几页失败、这一卷因此去了哪儿、坏页在输出里是什么样子。
/// 后面几行照常——隔离的卷是**处理过**的卷，几何门、卷级判定、逐页结果一样不少。
fn isolated_line(volume: &VolumeReport) -> String {
    let failed = volume.failures().count();
    if failed == 0 {
        return String::new();
    }
    format!(
        "  隔离 {failed} 页失败：本卷整卷写到隔离目录 {}，\
         失败页以卷内统一尺寸留白占位，页序不断\n",
        volume.output.display()
    )
}

/// 几何门那一段：门的**判定范围**、范围里有几页不成立，加上本卷最终抖不抖。
///
/// 三件事写在一起，因为只有并排才解释得了对方。门逐页判（ADR 0007 决定第 1 条），
/// 「成立」这句话因此得连着范围一起读——一卷全是彩页时门同样成立，而那是「无人可关」，
/// 不是「每一页都贴住了面板」。本卷那个抖动模式同理：门在主体那一组上开着时它才是判据选的，
/// 主体一页都不成立时它只是被关掉的结果。
///
/// **被排除的页要指得出来**，与上包络指出驱动页同一个做法：不指名，用户就无从判断
/// 这一卷该不该换个 profile。逐页那几行各自标着理由（`几何门不成立，本页不抖动`），
/// 这里只给个抓手——页数多起来时全列一遍只会把卷级那几行淹掉。
fn gate_line(volume: &VolumeReport, verdict: &VolumeVerdict) -> String {
    let judged = volume.judged_by_the_gate().count();
    let broken: Vec<&PageReport> = volume.outside_the_gate().collect();
    // `--per-page` 一开就没有卷级的抖动模式：它跟着位深一起逐页可变。
    let dither = verdict
        .dither()
        .map_or_else(|| "逐页".to_owned(), |dither| dither.to_string());
    let mut text = format!(
        "  几何门 判定范围 灰度页 {judged} 页 · 不成立 {} 页 · 本卷 {dither}\n",
        broken.len()
    );
    if broken.is_empty() {
        return text;
    }
    if broken.len() == judged {
        // 一页成立的都没有：没有别人可护，这些页自己就是主体，卷级那一档由它们定出
        // ——那一档必然不抖（ADR 0007 决定第 5 条）。
        text.push_str(
            "    范围内一页都不成立：每一页源都比目标小，按不放大原样输出，\
             阅读器还要再缩一次。没有别人可护，卷级基准档由它们自己定出，抖动因此整卷关闭\n",
        );
    } else {
        text.push_str(&format!("    不成立：{}\n", first_few_names(&broken)));
        text.push_str(
            "    这几页源比目标小，原样输出，阅读器还要再缩一次：它们不进卷级上包络，\
             抖动单独关掉，位深仍跟着卷级基准档、不低于它\n",
        );
    }
    // 同一道门也撑着面板灰阶那道硬上界：像素与灰阶不再对齐，「多出来的级到不了眼睛」
    // 就不再成立。ADR 0003 说了不得沿用，也说了该用哪个集合尚未测量——P0 仍照它裁，
    // 报告因此得把这句话说出来，而不是让它烂在一句注释里。
    text.push_str(
        "    面板灰阶上界的依据在这几页上随门一起失效，\
         P0 仍按它裁候选位深（ADR 0003：该用哪个集合尚未测量）\n",
    );
    text
}

/// 头几页的名字排成一句，剩下的报个数收口。
///
/// 上界取三：这一句是给人抓手用的，不是清单——真要逐页看，逐页那几行一页不落地列着。
fn first_few_names(pages: &[&PageReport]) -> String {
    const SHOWN: usize = 3;
    let listed: Vec<String> = pages
        .iter()
        .take(SHOWN)
        .map(|page| page.source.display().to_string())
        .collect();
    match pages.len().checked_sub(SHOWN) {
        Some(rest) if rest > 0 => format!("{}，另有 {rest} 页", listed.join("、")),
        _ => listed.join("、"),
    }
}

/// 一页那一行：它走的分支，以及那条分支得出的结果。
///
/// 灰度路径给的是判定与判据。判据是量、阈值是界：判定从两者的比较来，因此两者都得摆在
/// 同一行上，判定才是可解释的（spec 的 story 7）。阈值在头一行的 profile 里，
/// 它对整份报告只有一个。
///
/// 彩色分支上没有判定可说，那一行说的是它为什么没有：那条路径只缩放（ADR 0005 决定第 4 条）。
/// 彩页转灰走的是灰度路径，行首标出来——不标，用户就看不出这一档位深是替一张彩页定的，
/// 也看不出这台设备为什么没留住颜色。
///
/// 失败页那一行说的是**原因**（spec 的 story 26）：报告要让用户知道该去修哪几张。
/// 原因是由内到外的整条错误链，最外一环指得出是哪一页、卡在哪一步。
fn page_line(page: &PageReport) -> String {
    let Some(branch) = page.branch() else {
        return format!("失败 {}", page.failure().expect("没有分支的页必是失败页"));
    };
    // 部分救回页标在行首（04 号票）：它有判定、有判据、有自己的尺寸，逐页那一行因此
    // 与一张完好页长得一模一样，而它的判据是在一页大半留白的图上求出来的。
    let salvaged = match page.salvage() {
        Some(salvage) => format!("{salvage} · "),
        None => String::new(),
    };
    match branch {
        PageBranch::Gray {
            scores, verdict, ..
        } => format!(
            "{salvaged}{}判定 {}（{}）  判据 {}",
            if page.color() == Some(PageColor::Color) {
                "彩页转灰 · "
            } else {
                ""
            },
            verdict.candidate,
            verdict.reason,
            score_line(scores)
        ),
        PageBranch::Color => {
            format!("{salvaged}彩页 · 彩色分支：只缩放，不量化，不进灰度缓存也不进卷级上包络")
        }
    }
}

/// 一页各候选的判据值排成一行，候选由小到大。
fn score_line(scores: &[CandidateScore]) -> String {
    scores
        .iter()
        .map(|scored| format!("{} {}", scored.candidate, scored.score))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 进度条：把管线报到的那些步画出来（spec 的 story 30）。
///
/// 画在 **stderr** 上——报告走 stdout，`tonefit … > 报告.txt` 那种用法下进度条不该混进文件里。
/// indicatif 自己认得出对面不是终端，那时它一个字节都不写，管道与用例因此干净
/// （见 `ProgressDrawTarget::term`）。
///
/// 一卷一条，走完即抹掉：几十卷跑下来，屏幕上留下的只有报告。留着一排走完的进度条
/// 是另一种噪声，而「这一趟做了什么」报告说得更清楚。
struct Bar {
    /// 当下这一卷的那一条。卷与卷之间是空的。
    ///
    /// 每卷新起一条而不是复用同一条：走完的那一条已经收了尾，再往它身上设长度、加位置，
    /// indicatif 那一侧不保证还画得出来。
    volume: Mutex<Option<ProgressBar>>,
}

impl Bar {
    fn new() -> Self {
        Self {
            volume: Mutex::new(None),
        }
    }

    fn current(&self) -> MutexGuard<'_, Option<ProgressBar>> {
        self.volume
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Progress for Bar {
    fn volume_started(&self, volume: &Path, steps: u64) {
        let name = volume.file_name().map_or_else(
            || volume.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let bar = ProgressBar::new(steps);
        bar.set_style(
            ProgressStyle::with_template(
                "{msg} [{bar:30}] {pos}/{len} 步 · 已用 {elapsed} · 剩 {eta}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
        );
        bar.set_message(name);
        *self.current() = Some(bar);
    }

    fn stepped(&self) {
        if let Some(bar) = self.current().as_ref() {
            bar.inc(1);
        }
    }

    fn volume_finished(&self) {
        if let Some(bar) = self.current().take() {
            bar.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use tonefit::{
        CacheUsage, Candidate, ChosenBy, Envelope, GeometryGate, GrayImage, IoPlan, Medium,
        PageOutcome, Processed, Reason, Reference, Salvage, Scaling, Size, Verdict, VolumeReport,
    };

    /// 一份卷级上包络。渲染这一侧只关心它有没有被说出来，一页的卷取那一页作驱动页。
    fn envelope(base: Candidate) -> Envelope {
        Envelope {
            base,
            driver: 0,
            body_pages: 1,
            outlier_pages: 0,
            raised_pages: 0,
        }
    }

    /// 一份读取计划。渲染这一侧只关心它有没有被说出来，取「探到固态盘、并发读」那一种。
    fn io_plan() -> IoPlan {
        IoPlan {
            medium: Medium::Solid,
            readers: 8,
            chosen_by: ChosenBy::Probe,
        }
    }

    /// 一份缓存用量。渲染这一侧只关心它有没有被说出来，数值取整好读的。
    fn cache_usage() -> CacheUsage {
        CacheUsage {
            budget: CacheBudget::default(),
            pages: 1,
            raw: 4 * 1024 * 1024,
            stored: 1024 * 1024,
            resident: 1024 * 1024,
            spilled: 0,
        }
    }

    /// B 类中位页缩到基准面板：总缩放比 1.219，不触发预缩（见 measurements 的《B 类素材普查》）。
    fn typical_scaling() -> Scaling {
        Scaling::plan(Size::new(1441, 2048), Size::new(1182, 1680))
    }

    /// 一份一页的报告。各用例只改自己那一处，别处照抄默认。
    ///
    /// 几何门不在参数里：它跟着那一页走（`PageBranch::Gray` 的 `gate`），
    /// 卷级那一段是从页数出来的（06 号票）。
    fn one_page_report(profile: Profile, verdict: VolumeVerdict, page: PageReport) -> Report {
        Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(verdict),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 1,
                pages: vec![page],
            }],
        }
    }

    #[test]
    fn the_command_line_is_wired_up() {
        Cli::command().debug_assert();
    }

    /// `calibrate` 点名一台设备与一个去处，此外什么都不要（14 号票）。
    ///
    /// 处理卷要的那一整排参数它一个都不收——标定图不读源、不落进输出根，
    /// 缩放、位深、抖动、缓存、并发在它身上一件都不发生。
    #[test]
    fn the_calibrate_subcommand_names_a_device_and_where_to_write_the_chart() {
        let cli = Cli::try_parse_from([
            "tonefit",
            "calibrate",
            "--profile",
            "Kobo Libra 2",
            "--out",
            "标定图.png",
        ])
        .expect("参数应当可解析");

        let Some(Command::Calibrate {
            profile,
            gray_levels,
            out,
        }) = &cli.command
        else {
            panic!("没解析成 calibrate 子命令");
        };
        assert_eq!(
            target_profile(profile, *gray_levels, None)
                .expect("内置型号")
                .device(),
            "kobo-libra-2"
        );
        assert_eq!(out, &PathBuf::from("标定图.png"));

        // 不点子命令仍是「处理点名的若干卷」那一件事：子命令没有把默认用法顶掉。
        let plain =
            Cli::try_parse_from(["tonefit", "--out", "out", "--profile", "kobo-libra-2", "卷"])
                .expect("参数应当可解析");
        assert!(plain.command.is_none());
    }

    /// 处理卷那一路的必填项一项都没松：`--out`、`--profile`、卷，缺一样都不许往下走。
    ///
    /// 这三项的字段类型是 `Option`，为的是让 `calibrate` 那一路不必交出它们
    /// （见 [`REQUIRED_BY_CLAP`]）。类型放松了，**必填这道关不许跟着放松**——
    /// 松掉的话，敲错的命令会一路走到 `expect` 上恐慌，而不是被 clap 拦下并告诉用户缺了什么。
    #[test]
    fn the_volume_side_still_demands_an_output_root_a_profile_and_a_volume() {
        for line in [
            vec!["tonefit", "--profile", "kobo-libra-2", "卷"],
            vec!["tonefit", "--out", "out", "卷"],
            vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"],
        ] {
            let kind = Cli::try_parse_from(&line)
                .map(|_| ())
                .map_err(|error| error.kind());
            assert_eq!(
                kind,
                Err(clap::error::ErrorKind::MissingRequiredArgument),
                "{line:?} 不该解析得出来"
            );
        }
    }

    /// 数出来的级数回填给 `--gray-levels`，标定图跟着只排这台设备真会用到的那几档
    /// （ADR 0003：面板灰阶数是位深的硬上界）。
    #[test]
    fn gray_levels_on_calibrate_fold_into_the_profile_the_chart_is_drawn_for() {
        let cli = Cli::try_parse_from([
            "tonefit",
            "calibrate",
            "-p",
            "kobo-libra-2",
            "--gray-levels",
            "4",
            "-o",
            "chart.png",
        ])
        .expect("参数应当可解析");

        let Some(Command::Calibrate {
            profile,
            gray_levels,
            ..
        }) = &cli.command
        else {
            panic!("没解析成 calibrate 子命令");
        };

        assert_eq!(
            target_profile(profile, *gray_levels, None)
                .expect("内置型号")
                .panel()
                .gray_levels,
            4
        );
    }

    /// 帮助文本必须说清：目视数出的是**感知可分辨级数**，不等于面板的物理灰阶数
    /// （14 号票的最后一条；ADR 0003 的《后果》）。
    ///
    /// 两者不必相等，而 `--gray-levels` 填的是前者。这句话不说出来，用户会拿数出的 12
    /// 去质疑「厂商标的明明是 16」，然后什么都不填。
    #[test]
    fn the_calibrate_help_says_what_is_counted_is_perceived_not_physical() {
        let mut command = Cli::command();
        let calibrate = command
            .find_subcommand_mut("calibrate")
            .expect("calibrate 子命令");

        // `-h` 与 `--help` 两份都要说到：短的那份只印头一行，而用户多半只敲 `-h`。
        for help in [
            calibrate.render_help().to_string(),
            calibrate.render_long_help().to_string(),
        ] {
            assert!(help.contains("感知可分辨级数"), "{help}");
            assert!(help.contains("不等于面板的物理灰阶数"), "{help}");
        }

        let long = calibrate.render_long_help().to_string();
        assert!(long.contains("--gray-levels"), "{long}");
        // 数哪一条要点名：几条阶梯就有几个数。
        assert!(long.contains("最右"), "{long}");
        // 子命令在总帮助里露得出来，不然没人找得到它。
        assert!(
            Cli::command()
                .render_long_help()
                .to_string()
                .contains("calibrate"),
            "总帮助里没有 calibrate"
        );
    }

    /// `calibrate` 把图写到点名的那个文件上，父目录不在就建出来（14 号票）。
    ///
    /// 落不落盘、落到哪儿是 CLI 这一层的事：库那侧出的是字节，够不着这一条。
    /// 断言比的是**文件里的字节**与库出的那一份——中间这一段不许对图动手。
    #[test]
    fn calibrate_writes_the_chart_to_the_named_file_and_makes_its_parent() {
        let workspace = tempfile::tempdir().expect("建临时目录");
        let out = workspace.path().join("还不存在的目录").join("标定图.png");

        let code = calibrate("Kobo Libra 2", None, &out).expect("写标定图");

        assert_eq!(code, SUCCESS_EXIT, "写成了就该是全部成功那个数");
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        assert_eq!(
            std::fs::read(&out).expect("读回标定图"),
            tonefit::calibration_chart(&profile).expect("画标定图"),
            "落盘的字节与库出的不是同一份"
        );
    }

    #[test]
    fn gray_levels_from_the_command_line_fold_into_the_profile() {
        let cli = Cli::try_parse_from([
            "tonefit",
            "--out",
            "out",
            "--profile",
            "Kobo Libra 2",
            "--gray-levels",
            "8",
            "volume-a",
        ])
        .expect("参数应当可解析");

        let profile = cli.target_profile().expect("profile 应当解析成功");

        assert_eq!(profile.device(), "kobo-libra-2");
        assert_eq!(profile.panel().gray_levels, 8);
    }

    #[test]
    fn the_filter_from_the_command_line_names_the_residual_filter() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        // `box` 与 `area` 是同一个滤波器，大小写不论。
        assert_eq!(
            parse(&["--filter", "BOX"])
                .residual_filter()
                .expect("box 应当认得"),
            Filter::Area
        );
        // 不点名就是 ADR 0001 定的默认。
        assert_eq!(
            parse(&[]).residual_filter().expect("默认值"),
            Filter::Lanczos3
        );
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--filter", "mitchell"]).residual_filter().is_err());
    }

    #[test]
    fn a_dry_run_says_nothing_was_written_and_gives_the_metric_for_every_candidate() {
        // 判据值从公开 seam 上真算一个：报告要显示的就是它。
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let one_bit_dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        let score = tonefit::score(
            &reference,
            &tonefit::quantize(reference.image(), one_bit_dithered),
        );
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(one_bit_dithered)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate: one_bit_dithered,
                            score,
                        }],
                        verdict: Verdict {
                            candidate: one_bit_dithered,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = render(&report, Mode::DryRun);

        assert!(text.contains("dry-run"), "{text}");
        assert!(text.contains("还没落盘"), "{text}");
        // 比值 < 2 的一页：报告要说出它没预缩，残差段就是全部。
        assert!(text.contains("缩放比 1.219 · 未预缩"), "{text}");
        assert!(text.contains(&format!("判据 1bit+FS {score}")), "{text}");
        // dry-run 也给判定：预告的就是照做时会写出的那一个候选。
        assert!(text.contains("判定 1bit+FS"), "{text}");
    }

    #[test]
    fn the_report_renders_the_profile_then_one_line_per_volume_and_per_page() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        // 判据值从公开 seam 上真算一个：整页偏 8 级，判据读出的就是 8.000。
        let four_bit = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    // 正好两倍面板的一页：报告要说出它预缩过。
                    scaling: Scaling::plan(Size::new(2528, 3360), Size::new(1264, 1680)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate,
                            score: four_bit,
                        }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = render(&report, Mode::Process);

        // profile 一行、判据形状一行、卷六行（去处、几何门、卷级、驱动页、读取、缓存），
        // 页两行：一行几何，一行判定。
        assert_eq!(text.lines().count(), 10);
        // 头一行说明这份输出是给哪台设备的，以及本次用的面板。
        assert!(text.contains("kobo-libra-2"), "{text}");
        assert!(text.contains("300 PPI"), "{text}");
        assert!(text.contains("16 级灰阶"), "{text}");
        assert!(text.contains("library/volume-a"), "{text}");
        assert!(text.contains("1 页"), "{text}");
        assert!(text.contains("1264×1680"), "{text}");
        // 每页的缩放三件套：总缩放比、有没有预缩、残差比。
        assert!(text.contains("缩放比 2.000"), "{text}");
        assert!(text.contains("预缩 2×"), "{text}");
        assert!(text.contains("残差比 1.000"), "{text}");
        assert!(text.contains("out/volume-a/001.png"), "{text}");
        // 判定、它的理由，以及判定所依据的那个量：判定要可解释（spec 的 story 7）。
        assert!(
            text.contains(&format!(
                "判定 4bit（阈值内最低的一档）  判据 4bit {four_bit}"
            )),
            "{text}"
        );
        // 阈值对整份报告只有一个，写在头一行的 profile 里，并标明它是怎么定出来的。
        assert!(
            text.contains("阈值 5.500（盲测标定于 boox-poke6，其余面板未复核）"),
            "{text}"
        );
        // 判据那一栏的每个数都是分块聚合收出来的，而聚合里的 K 同样没标定——
        // 不说出来，读的人无从判断这一栏该信到什么程度（02 号票，ADR 0002 决定第 3 条）。
        // 块边长是 ADR 定死的数，直接写；K 是占位值，从 `aggregation()` 取——
        // 标定把它换掉时这一条不该跟着改。
        assert!(text.contains("判据聚合 分块 32×32"), "{text}");
        assert!(
            text.contains(&format!("不宽于 {} 块", aggregation().tail_tiles)),
            "{text}"
        );
        assert!(text.contains("K 未标定占位值"), "{text}");
        // 卷成为不可分割的处理单元是 ADR 0005 认下的代价：用量与有没有溢写都要说出来。
        assert!(text.contains("缓存 1 页 1.0 MiB"), "{text}");
        assert!(text.contains("未溢写"), "{text}");
        // 「这卷为什么是这个候选」要有一个指得出驱动页的答案（ADR 0006）。
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        assert!(text.contains("驱动页 library/volume-a/001.jpg"), "{text}");
        // 上包络不承诺卷内绝对一致：离群与迟滞升档各出了多少页，报告要说出来。
        // 离群那一处还带着占比——「一页都没摘出来」要在报告里看得见，而光看计数分不清
        // 「本来就没有离群页」与「离群判定整个失灵」（加固批 01 号票）。
        assert!(text.contains("离群 0 页（0.0%）"), "{text}");
        assert!(text.contains("迟滞升档 0 页"), "{text}");
        // 上包络的分位、迟滞页数、离群页判据的立脚点分位与倍数，四者均未标定，
        // 报告显式标注（ADR 0006）。
        assert!(text.contains("四者均未标定"), "{text}");
        // 几何门的判定范围与本卷的抖动模式都要报出来（ADR 0007、06 号票）：
        // 这一页在范围内、门开着，那个「不抖动」因此是判据选的。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动"),
            "{text}"
        );
    }

    /// 几何门那一段要说出**判定范围**与**被排除的页**（06 号票）：门逐页判，
    /// 「不成立」这句话得连着「范围里有几页、是哪几页」一起读，用户才判断得了这一卷该怎么办。
    #[test]
    fn a_broken_geometry_gate_names_its_scope_and_the_pages_it_left_out() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(800, 1000),
                outcome: PageOutcome::Whole(Processed {
                    // 源比目标小：按不放大原样输出，一条边都贴不住面板。
                    scaling: Scaling::plan(Size::new(800, 1000), Size::new(800, 1000)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Broken,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = render(&report, Mode::Process);

        // 判定范围与不成立的页数并排：一卷全是彩页时门同样成立，那是「无人可关」，
        // 不是「每一页都贴住了面板」。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 1 页 · 本卷 不抖动"),
            "{text}"
        );
        // 这一卷范围内一页都不成立：没有别人可护，卷级那一档由它们自己定出。
        assert!(text.contains("范围内一页都不成立"), "{text}");
        // 同一道门也撑着面板灰阶那道硬上界（ADR 0003），它跟着失效这件事不能只留在注释里。
        assert!(
            text.contains("面板灰阶上界的依据在这几页上随门一起失效"),
            "{text}"
        );
        assert!(text.contains("ADR 0003"), "{text}");
    }

    /// 报告要区分彩页与灰度页，也要区分它们走了哪条分支（10 号票）。
    ///
    /// 三页各占一种情形：彩页走彩色分支、彩页转灰走灰度路径、灰度页走灰度路径。
    /// 中间那一种是最容易被藏起来的——它有判定，看上去与灰度页毫无二致，
    /// 而用户想知道的恰恰是「这台设备为什么没留住颜色」。
    #[test]
    fn the_report_tells_a_color_page_apart_from_a_gray_one() {
        let profile = Profile::resolve("kobo-libra-colour").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let page = |name: &str, color, branch| PageReport {
            source: PathBuf::from(format!("library/volume-a/{name}.png")),
            output: PathBuf::from(format!("out/volume-a/{name}.png")),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                scaling: typical_scaling(),
                color,
                branch,
            }),
        };
        let gray_branch = || PageBranch::Gray {
            gate: GeometryGate::Holds,
            scores: vec![CandidateScore { candidate, score }],
            verdict: Verdict {
                candidate,
                reason: Reason::LowestWithinThreshold,
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                // 驱动页必须是一张灰度页：彩页不进上包络，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(Envelope {
                    base: candidate,
                    driver: 2,
                    body_pages: 2,
                    outlier_pages: 0,
                    raised_pages: 0,
                })),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 3,
                pages: vec![
                    page("001", PageColor::Color, PageBranch::Color),
                    page("002", PageColor::Color, gray_branch()),
                    page("003", PageColor::Gray, gray_branch()),
                ],
            }],
        };

        let text = render(&report, Mode::Process);

        // 卷那一行数得出彩页有几张：走哪条分支不影响它是不是彩页。
        assert!(text.contains("3 页，其中彩页 2 页"), "{text}");
        // 彩色分支那一页说得出它为什么没有判定。
        assert!(text.contains("彩页 · 彩色分支：只缩放"), "{text}");
        assert!(text.contains("不进灰度缓存也不进卷级上包络"), "{text}");
        // 转灰的那一页有判定，行首标着它的来路。
        assert!(text.contains("彩页转灰 · 判定 4bit"), "{text}");
        // 灰度页那一行不多带任何标记：四个空格之后直接是判定。
        assert!(text.contains("    判定 4bit"), "{text}");
        assert!(text.contains("驱动页 library/volume-a/003.png"), "{text}");
    }

    /// 跳过的卷只占两行：去处那一行，加上说清它为什么什么都没有的那一行。
    ///
    /// 几何门、卷级判定、缓存用量、逐页结果一个都不出现——那一趟根本没算过它们，
    /// 报告摆出任何一项都是编的。页数照旧要说出来：它是源那一侧的事实。
    #[test]
    fn a_skipped_volume_says_so_and_says_nothing_it_did_not_compute() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 0,
            }],
        };

        let text = render(&report, Mode::Process);

        // profile 一行、判据形状一行、卷两行，加上读取那一行——跳过的卷同样把整卷读了一遍。
        assert_eq!(text.lines().count(), 5);
        assert!(
            text.contains("library/volume-a → out/volume-a（12 页）"),
            "{text}"
        );
        assert!(text.contains("跳过 幂等命中"), "{text}");
        // 改哪一项会让它重做，用户得看得见（spec 的 story 9）。
        assert!(text.contains("工具版本、profile、参数、源均未变"), "{text}");
        assert!(!text.contains("几何门"), "{text}");
        assert!(!text.contains("缓存"), "{text}");
    }

    /// 介质探不出来的卷：报告说得出它退到了串行，也说得出**为什么**探不出来（13 号票）。
    ///
    /// 不说那句话，退到保守策略这件事对用户就只表现为「这一卷跑得慢」——
    /// 而那正是他没法据以决定要不要 `--io-mode concurrent` 的样子。
    #[test]
    fn a_volume_whose_medium_is_unknown_says_why_it_fell_back_to_serial() {
        let mut report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            volumes: vec![VolumeReport {
                volume: PathBuf::from(r"\\nas\share\volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 0,
            }],
        };
        report.volumes[0].io = IoPlan {
            medium: Medium::Unknown {
                reason: r"\\nas\share\ 是网络路径，介质无从探测".to_owned(),
            },
            readers: 1,
            chosen_by: ChosenBy::Probe,
        };

        let text = render(&report, Mode::Process);

        assert!(text.contains("读取串行"), "{text}");
        assert!(text.contains("是网络路径"), "{text}");
    }

    /// 被隔离的卷要说清三件事：几页失败、整卷去了哪儿、每一页各是为什么
    /// （spec 的 story 25、story 26）。退出码跟着分开——脚本读的是那个数。
    #[test]
    fn an_isolated_volume_names_its_failed_pages_and_gets_its_own_exit_code() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let good = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                scaling: typical_scaling(),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    scores: vec![CandidateScore { candidate, score }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::VolumeEnvelope,
                    },
                },
            }),
        };
        let failed = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/002.png"),
            // 失败页照卷内统一尺寸出：与上面那张好页一模一样。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Failed {
                reason: "解 library/volume-a/002.jpg 这一页: 判定格式".to_owned(),
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/_isolated/volume-a"),
                // 上一趟这一卷是干净的，那一份还在 out/volume-a 留着。
                superseded: Some(PathBuf::from("out/volume-a")),
                // 驱动页必须是一张好页：失败页没有判据曲线，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 2,
                pages: vec![good, failed],
            }],
        };

        let text = render(&report, Mode::Process);

        // 卷级那一行说得出几页失败、整卷去了哪儿。
        assert!(text.contains("隔离 1 页失败"), "{text}");
        assert!(text.contains("out/_isolated/volume-a"), "{text}");
        // 隔离的卷仍是**处理过**的卷：几何门、卷级判定、缓存一样不少。
        // 失败页不在几何门的判定范围内（它连尺寸都没有），范围因此只有那一张好页。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页"),
            "{text}"
        );
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        // 失败页那两行：尺寸从哪来，以及它为什么失败。
        assert!(
            text.contains("1264×1680  失败页 · 卷内统一尺寸留白"),
            "{text}"
        );
        assert!(
            text.contains("失败 解 library/volume-a/002.jpg 这一页: 判定格式"),
            "{text}"
        );
        // 末尾那一行：几十卷跑下来不用往回翻也知道这一趟出过事。
        assert!(text.contains("隔离 1 卷 · 失败 1 页"), "{text}");
        // 上一趟写在干净去处的那一份还在，这一趟没覆盖它——报告要指名道姓说出来。
        assert!(text.contains("过期副本 out/volume-a"), "{text}");
        assert!(text.contains("删不删由你"), "{text}");
        // 退出码分得开「全部成功」与「有卷被隔离」。
        assert_eq!(exit_code(&report), ISOLATED_EXIT);
    }

    /// 部分救回页在报告里认得出来，而且**只有报告认得出来**（04 号票）。
    ///
    /// 它没有退出码替它喊，卷也照旧落在干净的去处：这一趟从进程那一侧看与全部成功
    /// 一模一样。三处各说一遍——逐页那一行说这一页救回了多少，卷级那一行说它没参与
    /// 卷级的判定，末尾那一行让几十卷跑下来的人不用往回翻。
    #[test]
    fn the_report_marks_a_salvaged_page_and_says_it_stayed_out_of_the_volume_decision() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let processed = |reason| Processed {
            scaling: typical_scaling(),
            color: PageColor::Gray,
            branch: PageBranch::Gray {
                gate: GeometryGate::Holds,
                scores: vec![CandidateScore { candidate, score }],
                verdict: Verdict { candidate, reason },
            },
        };
        let whole = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(processed(Reason::VolumeEnvelope)),
        };
        let salvaged = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/volume-a/002.png"),
            // 它按**自己**的尺寸出：文件头里那个尺寸一点没缺。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Salvaged {
                // 它没进上包络，判定因此是它自己那条判据曲线定的。
                page: processed(Reason::LowestWithinThreshold),
                salvage: Salvage::from_share(0.625),
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 2,
                pages: vec![whole, salvaged],
            }],
        };

        let text = render(&report, Mode::Process);

        // 逐页那一行：救回了多少，摆在判定前面。
        assert!(text.contains("救回 62.5% · 判定 4bit"), "{text}");
        // 卷级那一行：这一卷有几页不全，以及它没参与卷级的哪一件事。
        // 几何门不在那句话里了——门逐页判之后照旧问它（ADR 0007 决定第 1 条，06 号票）。
        assert!(text.contains("部分救回 1 页"), "{text}");
        assert!(text.contains("不参与卷级上包络"), "{text}");
        assert!(text.contains("几何门照旧问它们"), "{text}");
        // 末尾那一行：几十卷跑下来不用往回翻。
        assert!(text.contains("部分救回 1 卷 · 1 页"), "{text}");
        // 完好的那一页一个字都不多说：它那一行以判定开头，前面没有救回那一截。
        assert!(
            text.contains(
                "
    判定 4bit（卷级上包络）"
            ),
            "{text}"
        );
        // 卷没被隔离，退出码因此仍是 0——报告是唯一说得出这件事的地方。
        assert!(!text.contains("隔离"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 一卷都没被隔离时，隔离那几行一个字都不出现，退出码是 0。
    ///
    /// 「没出事」与「出了事」在报告与退出码上都得分得开，而分得开要两侧各测一遍。
    #[test]
    fn a_run_without_a_failed_page_says_nothing_about_isolation() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = render(&report, Mode::Process);

        assert!(!text.contains("隔离"), "{text}");
        assert!(!text.contains("失败"), "{text}");
        assert!(!text.contains("过期副本"), "{text}");
        // 一页都没救回过的一趟同样一个字都不说（04 号票）。
        assert!(!text.contains("救回"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    #[test]
    fn the_bit_depth_from_the_command_line_overrides_the_verdict() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        assert_eq!(
            parse(&["--bit-depth", "2"])
                .bit_depth_override()
                .expect("2 应当认得"),
            Some(BitDepth::Two)
        );
        // 不点名就由判据说了算。
        assert_eq!(parse(&[]).bit_depth_override().expect("默认值"), None);
        // 全集之外的比特数在拼 Request 之前就被挡下。
        assert!(parse(&["--bit-depth", "3"]).bit_depth_override().is_err());
    }

    /// `--no-metadata` 关掉记录，幂等能力随之关闭——两件事是同一个开关，
    /// 帮助文本里必须并排说出来，否则用户会以为自己只是少写了几行 tEXt。
    #[test]
    fn no_metadata_says_in_the_help_text_that_it_also_turns_idempotency_off() {
        let line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
        let mut with_flag = line.clone();
        with_flag.extend(["--no-metadata", "volume-a"]);
        let mut without = line;
        without.push("volume-a");

        assert!(
            Cli::try_parse_from(with_flag)
                .expect("参数应当可解析")
                .no_metadata
        );
        // 不点名就照写：记录随文件走是默认行为。
        assert!(
            !Cli::try_parse_from(without)
                .expect("参数应当可解析")
                .no_metadata
        );

        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--no-metadata"), "{help}");
        assert!(help.contains("幂等能力随之关闭"), "{help}");
        assert!(help.contains("每一趟都整卷重做"), "{help}");
    }

    #[test]
    fn the_dither_mode_from_the_command_line_overrides_the_automatic_choice() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        assert_eq!(
            parse(&["--dither", "FS"])
                .dither_override()
                .expect("fs 应当认得"),
            Some(Dither::FloydSteinberg)
        );
        assert_eq!(
            parse(&["--dither", "none"])
                .dither_override()
                .expect("none 应当认得"),
            Some(Dither::Off)
        );
        // 不点名就由判据在几何门放行的那几种里选。
        assert_eq!(parse(&[]).dither_override().expect("默认值"), None);
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--dither", "bayer"]).dither_override().is_err());
    }
}
