//! CLI：把命令行参数拼成 `Request`，把 `Report` 渲染成文字。此外不做别的事。

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tonefit::{Report, Request};

#[derive(Parser)]
#[command(about = "把漫画页适配到电子墨水阅读设备", version)]
struct Cli {
    /// 要处理的卷（目录）。源目录只读。
    #[arg(required = true, value_name = "卷")]
    inputs: Vec<PathBuf>,

    /// 输出根目录。每个卷在它下面得到一个同名子目录。
    #[arg(short, long, value_name = "目录")]
    out: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out,
    })?;
    print!("{}", render(&report));
    Ok(())
}

fn render(report: &Report) -> String {
    let mut text = String::new();
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
    use tonefit::{PageReport, Size, VolumeReport};

    #[test]
    fn the_report_renders_one_line_per_volume_and_per_page() {
        let report = Report {
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

        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("library/volume-a"), "{text}");
        assert!(text.contains("1 页"), "{text}");
        assert!(text.contains("1264×1680"), "{text}");
        assert!(text.contains("out/volume-a/001.png"), "{text}");
    }
}
