//! CLI：把命令行参数拼成 `Request`，把 `Report` 渲染成文字。此外不做别的事。

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tonefit::{
    BitDepth, CacheBudget, CandidateScore, Dither, Filter, GeometryGate, Mode, PageBranch,
    PageColor, PageReport, Profile, Report, Request, VolumeReport, VolumeVerdict,
};

#[derive(Parser)]
#[command(about = "把漫画页适配到电子墨水阅读设备", version)]
struct Cli {
    /// 要处理的卷：一个目录，或一个 CBZ。源只读。
    #[arg(required = true, value_name = "卷")]
    inputs: Vec<PathBuf>,

    /// 输出根目录。每个卷在它下面得到一份同名副本，容器形态与输入一致。
    #[arg(short, long, value_name = "目录")]
    out: PathBuf,

    /// 目标设备型号。内置表覆盖 Kobo、BOOX、Kindle 的主力型号，型号名不区分大小写与分隔符。
    #[arg(short, long, value_name = "型号")]
    profile: String,

    /// 覆盖面板灰阶数。内置表没收录的设备、或在真机上数出的实际可分辨级数走这里。
    #[arg(long, value_name = "级数")]
    gray_levels: Option<u32>,

    /// 残差段的重采样滤波器：area（= box）、bilinear、hamming、bicubic、lanczos3，默认 lanczos3。
    /// 只作用于残差段——总缩放比 ≥ 2 时的整数倍预缩那一级恒为 box。
    #[arg(long, value_name = "滤波器")]
    filter: Option<String>,

    /// 覆盖自动判定的位深：1、2、4、8。面板灰阶数那道上界仍在，越界的覆盖会被拒绝。
    #[arg(long, value_name = "位深")]
    bit_depth: Option<u32>,

    /// 覆盖自动选择的抖动模式：off（= none）、fs（= floyd-steinberg）。
    /// 抖动只在输出不被下游缩放时才谈得上：卷内有页源比目标尺寸小时几何门不成立，
    /// 那时点名 fs 会被**拒绝**，不会静默照抖。
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

    /// 把 `--profile` 与 `--gray-levels` 合成本次要用的 profile。
    fn target_profile(&self) -> Result<Profile> {
        let profile = Profile::resolve(&self.profile)?;
        match self.gray_levels {
            Some(gray_levels) => profile.with_gray_levels(gray_levels),
            None => Ok(profile),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let profile = cli.target_profile()?;
    let filter = cli.residual_filter()?;
    let bit_depth = cli.bit_depth_override()?;
    let dither = cli.dither_override()?;
    let cache_budget = cli.cache_budget()?;
    let mode = cli.mode();
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out,
        profile,
        filter,
        bit_depth,
        dither,
        per_page: cli.per_page,
        cache_budget,
        mode,
        metadata: !cli.no_metadata,
    })?;
    print!("{}", render(&report, mode));
    Ok(())
}

fn render(report: &Report, mode: Mode) -> String {
    let mut text = format!("profile {}\n", report.profile);
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
        text.push_str(&volume_lines(volume));
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
                page.scaling,
                page.output.display()
            ));
            text.push_str(&format!("    {}\n", verdict_line(page)));
        }
    }
    text
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
        .filter(|page| page.color == PageColor::Color)
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
    let mut text = gate_line(volume, verdict);
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

/// 几何门那一行：门的判定结果，加上本卷最终抖不抖。
///
/// 两件事写在一行上，因为只有并排才解释得了对方：门关着时抖动整体关闭，那个「不抖动」
/// 不是判据选的；门开着时它才是判据选出来的（ADR 0007：不成立时整体关闭，
/// 通过时抖不抖跟着位深一起按卷决定）。
/// 门被哪一页关上也要说出来——门关掉的是**整卷**的抖动，不指名，用户就无从下手。
fn gate_line(volume: &VolumeReport, verdict: &VolumeVerdict) -> String {
    // 走到这里的卷都真算过一遍：跳过的那一种在 `volume_lines` 里已经走掉了。
    let gate = match volume.gate.expect("算过的卷必有几何门判定") {
        GeometryGate::Holds => "成立".to_owned(),
        GeometryGate::Broken { page } => format!(
            "不成立（{} 源比目标小，原样输出，阅读器还要再缩一次）",
            volume.pages[page].source.display()
        ),
    };
    // `--per-page` 一开就没有卷级的抖动模式：它跟着位深一起逐页可变。
    let dither = verdict
        .dither()
        .map_or_else(|| "逐页".to_owned(), |dither| dither.to_string());
    let mut text = format!("  几何门 {gate} · 本卷 {dither}\n");
    if !volume.gate.is_some_and(GeometryGate::holds) {
        // 同一道门也撑着面板灰阶那道硬上界：像素与灰阶不再对齐，「多出来的级到不了眼睛」
        // 就不再成立。ADR 0003 说了不得沿用，也说了该用哪个集合尚未测量——P0 仍照它裁，
        // 报告因此得把这句话说出来，而不是让它烂在一句注释里。
        text.push_str(
            "    面板灰阶上界的依据随门一起失效，\
             P0 仍按它裁候选位深（ADR 0003：该用哪个集合尚未测量）\n",
        );
    }
    text
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
fn verdict_line(page: &PageReport) -> String {
    match &page.branch {
        PageBranch::Gray { scores, verdict } => format!(
            "{}判定 {}（{}）  判据 {}",
            if page.color == PageColor::Color {
                "彩页转灰 · "
            } else {
                ""
            },
            verdict.candidate,
            verdict.reason,
            score_line(scores)
        ),
        PageBranch::Color => {
            "彩页 · 彩色分支：只缩放，不量化，不进灰度缓存也不进卷级上包络".to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use tonefit::{
        CacheUsage, Candidate, Envelope, GrayImage, Reason, Reference, Scaling, Size, Verdict,
        VolumeReport,
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
    fn one_page_report(
        profile: Profile,
        gate: GeometryGate,
        verdict: VolumeVerdict,
        page: PageReport,
    ) -> Report {
        Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                verdict: Some(verdict),
                gate: Some(gate),
                cache: cache_usage(),
                decodes: 1,
                pages: vec![page],
            }],
        }
    }

    #[test]
    fn the_command_line_is_wired_up() {
        Cli::command().debug_assert();
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
            GeometryGate::Holds,
            VolumeVerdict::Envelope(envelope(one_bit_dithered)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                scaling: typical_scaling(),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    scores: vec![CandidateScore {
                        candidate: one_bit_dithered,
                        score,
                    }],
                    verdict: Verdict {
                        candidate: one_bit_dithered,
                        reason: Reason::LowestWithinThreshold,
                    },
                },
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
            GeometryGate::Holds,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                // 正好两倍面板的一页：报告要说出它预缩过。
                scaling: Scaling::plan(Size::new(2528, 3360), Size::new(1264, 1680)),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    scores: vec![CandidateScore {
                        candidate,
                        score: four_bit,
                    }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::LowestWithinThreshold,
                    },
                },
            },
        );

        let text = render(&report, Mode::Process);

        // profile 一行、卷五行（去处、几何门、卷级、驱动页、缓存），页两行：一行几何，一行判定。
        assert_eq!(text.lines().count(), 8);
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
        // 阈值对整份报告只有一个，写在头一行的 profile 里，并标明它还没标定。
        assert!(text.contains("阈值 8.500（未标定占位值）"), "{text}");
        // 卷成为不可分割的处理单元是 ADR 0005 认下的代价：用量与有没有溢写都要说出来。
        assert!(text.contains("缓存 1 页 1.0 MiB"), "{text}");
        assert!(text.contains("未溢写"), "{text}");
        // 「这卷为什么是这个候选」要有一个指得出驱动页的答案（ADR 0006）。
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        assert!(text.contains("驱动页 library/volume-a/001.jpg"), "{text}");
        // 上包络不承诺卷内绝对一致：离群与迟滞升档各出了多少页，报告要说出来。
        assert!(text.contains("离群 0 页"), "{text}");
        assert!(text.contains("迟滞升档 0 页"), "{text}");
        // 上包络的分位、迟滞页数、离群页判据三者均未标定，报告显式标注（ADR 0006）。
        assert!(text.contains("三者均未标定"), "{text}");
        // 几何门与本卷的抖动模式都要报出来（ADR 0007）：门开着而判据选了不抖。
        assert!(text.contains("几何门 成立 · 本卷 不抖动"), "{text}");
    }

    /// 几何门不成立时报告要说出**是哪一页**关的门：门关掉的是整卷的抖动，
    /// 不指名，用户就无从判断这一卷该怎么办（ADR 0007）。
    #[test]
    fn a_broken_geometry_gate_names_the_page_that_broke_it() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            GeometryGate::Broken { page: 0 },
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(800, 1000),
                // 源比目标小：按不放大原样输出，一条边都贴不住面板。
                scaling: Scaling::plan(Size::new(800, 1000), Size::new(800, 1000)),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    scores: vec![CandidateScore { candidate, score }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::VolumeEnvelope,
                    },
                },
            },
        );

        let text = render(&report, Mode::Process);

        assert!(text.contains("几何门 不成立"), "{text}");
        assert!(
            text.contains("library/volume-a/001.jpg 源比目标小"),
            "{text}"
        );
        assert!(text.contains("本卷 不抖动"), "{text}");
        // 同一道门也撑着面板灰阶那道硬上界（ADR 0003），它跟着失效这件事不能只留在注释里。
        assert!(text.contains("面板灰阶上界的依据随门一起失效"), "{text}");
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
            scaling: typical_scaling(),
            color,
            branch,
        };
        let gray_branch = || PageBranch::Gray {
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
                // 驱动页必须是一张灰度页：彩页不进上包络，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(Envelope {
                    base: candidate,
                    driver: 2,
                    body_pages: 2,
                    outlier_pages: 0,
                    raised_pages: 0,
                })),
                gate: Some(GeometryGate::Holds),
                cache: cache_usage(),
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
                pages: Vec::new(),
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                gate: None,
                cache: cache_usage(),
                decodes: 0,
            }],
        };

        let text = render(&report, Mode::Process);

        // profile 一行、卷两行。
        assert_eq!(text.lines().count(), 3);
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
