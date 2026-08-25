//! 源：按阅读顺序吐出页。
//!
//! 本版本只有目录源。归档（CBZ）与非图片文件透传是 03 号票的事。

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::decode;

/// 一个卷：一次处理调用的作用域。
pub struct Volume {
    /// 卷标识：源目录路径。
    pub root: PathBuf,
    /// 卷名，用作输出目录名。
    pub name: String,
    /// 按阅读顺序排好的页。
    pub pages: Vec<SourcePage>,
}

/// 卷内的一页。
pub struct SourcePage {
    /// 页文件的完整路径。
    pub path: PathBuf,
    /// 相对卷根的路径，输出按它镜像出目录结构。
    pub relative: PathBuf,
}

/// 打开一个目录卷。目录只读，这里不写任何东西。
pub fn open(root: &Path) -> Result<Volume> {
    if !root.is_dir() {
        bail!("{} 不是目录：本版本只处理目录形式的卷", root.display());
    }
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("{} 没有目录名，无法决定输出位置", root.display()))?;

    let mut pages = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.with_context(|| format!("遍历 {}", root.display()))?;
        if !entry.file_type().is_file() || !decode::is_page(entry.path()) {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("遍历结果恒在卷根之下")
            .to_path_buf();
        pages.push(SourcePage {
            path: entry.path().to_path_buf(),
            relative,
        });
    }
    pages.sort_by(|a, b| reading_order(&a.relative, &b.relative));

    Ok(Volume {
        root: root.to_path_buf(),
        name,
        pages,
    })
}

/// 阅读顺序：逐层比路径分量，分量内数字段按数值比。
///
/// 字典序会把 `10.png` 排到 `2.png` 前面，而页号正是漫画的阅读顺序本身。
fn reading_order(a: &Path, b: &Path) -> Ordering {
    let mut a = a.components();
    let mut b = b.components();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left), Some(right)) => {
                let left = left.as_os_str().to_string_lossy();
                let right = right.as_os_str().to_string_lossy();
                match natural(&left, &right) {
                    Ordering::Equal => continue,
                    ordering => return ordering,
                }
            }
        }
    }
}

/// 自然序：数字段按数值比，其余按大小写不敏感的字典序比。
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        if a.is_empty() || b.is_empty() {
            return a.len().cmp(&b.len());
        }
        let ordering = match (starts_with_digit(a), starts_with_digit(b)) {
            (true, true) => {
                let left = take_run(&mut a, true);
                let right = take_run(&mut b, true);
                compare_numbers(left, right)
            }
            (false, false) => {
                let left = take_run(&mut a, false);
                let right = take_run(&mut b, false);
                left.to_lowercase()
                    .cmp(&right.to_lowercase())
                    .then_with(|| left.cmp(right))
            }
            // 一边是数字一边不是，首字符已经能定序。
            _ => return a.cmp(b),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn starts_with_digit(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_digit())
}

/// 切下开头一段同类字符（`digits` 为真取数字段，否则取非数字段），
/// 返回它并让 `text` 指向剩下的部分。
fn take_run<'a>(text: &mut &'a str, digits: bool) -> &'a str {
    let end = text
        .find(|c: char| c.is_ascii_digit() != digits)
        .unwrap_or(text.len());
    let (run, rest) = text.split_at(end);
    *text = rest;
    run
}

/// 数字段按数值比：去掉前导零后先比位数再比字典序，位数不设上限；数值相同则前导零少的在前。
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let left = a.trim_start_matches('0');
    let right = b.trim_start_matches('0');
    left.len()
        .cmp(&right.len())
        .then_with(|| left.cmp(right))
        .then_with(|| a.len().cmp(&b.len()))
}
