//! CLI：把命令行参数拼成 `Request`，把 `Report` 渲染成文字。此外不做别的事。

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tonefit::{BitDepth, CandidateScore, Filter, Mode, PageReport, Profile, Report, Request};

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

    /// 只算不写：报告照出，逐页给出判定与各候选位深的判据值，一个文件都不落盘。
    #[arg(long)]
    dry_run: bool,
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

    /// 本次要不要顶掉自动判定。不点名就由判据说了算。
    fn bit_depth_override(&self) -> Result<Option<BitDepth>> {
        self.bit_depth.map(BitDepth::from_bits).transpose()
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
    let mode = cli.mode();
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out,
        profile,
        filter,
        bit_depth,
        mode,
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
            "{} → {}（{} 页）\n",
            volume.volume.display(),
            volume.output.display(),
            volume.pages.len()
        ));
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

/// 一页的判定：定下的那一档、定它的理由，后面跟上各候选的判据值。
///
/// 判据是量、阈值是界：判定从两者的比较来，因此两者都得摆在同一行上，判定才是可解释的
/// （spec 的 story 7）。阈值在头一行的 profile 里，它对整份报告只有一个。
fn verdict_line(page: &PageReport) -> String {
    format!(
        "位深 {}（{}）  判据 {}",
        page.verdict.bit_depth,
        page.verdict.reason,
        score_line(&page.scores)
    )
}

/// 一页各候选的判据值排成一行，位深由小到大。
fn score_line(scores: &[CandidateScore]) -> String {
    scores
        .iter()
        .map(|scored| format!("{} {}", scored.bit_depth, scored.score))
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use tonefit::{GrayImage, Reason, Reference, Scaling, Size, Verdict, VolumeReport};

    /// B 类中位页缩到基准面板：总缩放比 1.219，不触发预缩（见 measurements 的《B 类素材普查》）。
    fn typical_scaling() -> Scaling {
        Scaling::plan(Size::new(1441, 2048), Size::new(1182, 1680))
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
        let one_bit = tonefit::score(
            &reference,
            &tonefit::quantize(reference.image(), BitDepth::One),
        );
        let report = Report {
            profile: profile.clone(),
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                pages: vec![PageReport {
                    source: PathBuf::from("library/volume-a/001.jpg"),
                    output: PathBuf::from("out/volume-a/001.png"),
                    size: Size::new(1264, 1680),
                    scaling: typical_scaling(),
                    scores: vec![CandidateScore {
                        bit_depth: BitDepth::One,
                        score: one_bit,
                    }],
                    verdict: Verdict {
                        bit_depth: BitDepth::One,
                        reason: Reason::LowestWithinThreshold,
                    },
                }],
            }],
        };

        let text = render(&report, Mode::DryRun);

        assert!(text.contains("dry-run"), "{text}");
        assert!(text.contains("还没落盘"), "{text}");
        // 比值 < 2 的一页：报告要说出它没预缩，残差段就是全部。
        assert!(text.contains("缩放比 1.219 · 未预缩"), "{text}");
        assert!(text.contains(&format!("判据 1bit {one_bit}")), "{text}");
        // dry-run 也给判定：预告的就是照做时会写出的那一档。
        assert!(text.contains("位深 1bit"), "{text}");
    }

    #[test]
    fn the_report_renders_the_profile_then_one_line_per_volume_and_per_page() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        // 判据值从公开 seam 上真算一个：整页偏 8 级，判据读出的就是 8.000。
        let four_bit = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                pages: vec![PageReport {
                    source: PathBuf::from("library/volume-a/001.jpg"),
                    output: PathBuf::from("out/volume-a/001.png"),
                    size: Size::new(1264, 1680),
                    // 正好两倍面板的一页：报告要说出它预缩过。
                    scaling: Scaling::plan(Size::new(2528, 3360), Size::new(1264, 1680)),
                    scores: vec![CandidateScore {
                        bit_depth: BitDepth::Four,
                        score: four_bit,
                    }],
                    verdict: Verdict {
                        bit_depth: BitDepth::Four,
                        reason: Reason::LowestWithinThreshold,
                    },
                }],
            }],
        };

        let text = render(&report, Mode::Process);

        // profile 一行、卷一行，页两行：一行几何，一行判定。
        assert_eq!(text.lines().count(), 4);
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
        // 判定、它的理由，以及判定所依据的那个量：位深判定要可解释（spec 的 story 7）。
        assert!(
            text.contains(&format!(
                "位深 4bit（阈值内最低的一档）  判据 4bit {four_bit}"
            )),
            "{text}"
        );
        // 阈值对整份报告只有一个，写在头一行的 profile 里，并标明它还没标定。
        assert!(text.contains("阈值 8.500（未标定占位值）"), "{text}");
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
}
