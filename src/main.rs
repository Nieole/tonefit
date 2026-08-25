//! CLI：把命令行参数拼成 `Request`，把 `Report` 渲染成文字。此外不做别的事。

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tonefit::{Profile, Report, Request};

#[derive(Parser)]
#[command(about = "把漫画页适配到电子墨水阅读设备", version)]
struct Cli {
    /// 要处理的卷（目录）。源目录只读。
    #[arg(required = true, value_name = "卷")]
    inputs: Vec<PathBuf>,

    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    #[arg(short, long, value_name = "目录")]
    out: PathBuf,

    /// 目标设备型号。内置表覆盖 Kobo、BOOX、Kindle 的主力型号，型号名不区分大小写与分隔符。
    #[arg(short, long, value_name = "型号")]
    profile: String,

    /// 覆盖面板灰阶数。内置表没收录的设备、或在真机上数出的实际可分辨级数走这里。
    #[arg(long, value_name = "级数")]
    gray_levels: Option<u32>,
}

impl Cli {
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
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out,
        profile,
    })?;
    print!("{}", render(&report));
    Ok(())
}

fn render(report: &Report) -> String {
    let mut text = format!("profile {}\n", report.profile);
    for volume in &report.volumes {
        text.push_str(&format!(
            "{} → {}（{} 页）\n",
            volume.volume.display(),
            volume.output.display(),
            volume.pages.len()
        ));
        for page in &volume.pages {
            text.push_str(&format!("  {}  {}\n", page.size, page.output.display()));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use tonefit::{PageReport, Size, VolumeReport};

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
    fn the_report_renders_the_profile_then_one_line_per_volume_and_per_page() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                pages: vec![PageReport {
                    source: PathBuf::from("library/volume-a/001.jpg"),
                    output: PathBuf::from("out/volume-a/001.png"),
                    size: Size::new(1264, 1680),
                }],
            }],
        };

        let text = render(&report);

        assert_eq!(text.lines().count(), 3);
        // 头一行说明这份输出是给哪台设备的，以及本次用的面板。
        assert!(text.contains("kobo-libra-2"), "{text}");
        assert!(text.contains("300 PPI"), "{text}");
        assert!(text.contains("16 级灰阶"), "{text}");
        assert!(text.contains("library/volume-a"), "{text}");
        assert!(text.contains("1 页"), "{text}");
        assert!(text.contains("1264×1680"), "{text}");
        assert!(text.contains("out/volume-a/001.png"), "{text}");
    }
}
