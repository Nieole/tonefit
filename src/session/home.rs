//! **家目录**：屏上的路径把家目录那一截缩写成 `~`，输入行认 `~/` 开头的路径
//! （spec《输入行与路径》；`CONTEXT.md` 的《会话》：视图——路径与输出）。
//!
//! 家目录**由会话入口问一次往下传**（问不出来就不缩写、不认 `~`），用例给临时目录。
//! 问的那一下在 [`Home::found`]（一趟会话只问一次，与预设文件找配置目录同一副问法）；
//! 此后本模块不读环境变量、不读盘，只认交进来的那一个目录。
//!
//! 一个终端都不碰，因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）。

use std::path::{Path, PathBuf};

/// 家目录。`None` 是问不出来：那时一条路径原样印、`~` 也只是一个字。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Home(Option<PathBuf>);

impl Home {
    /// 认这一个目录作家目录。
    pub fn at(home: impl Into<PathBuf>) -> Self {
        Self(Some(home.into()))
    }

    /// **会话入口问那一次**：环境变量说家目录在哪儿就是哪儿（Windows 上是 `USERPROFILE`，
    /// 别处是 `HOME`，与 `crate::preset` 找配置目录同一副问法），没设就是问不出来。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有会话入口问它，而那条循环在 tui 特性后面")
    )]
    pub fn found() -> Self {
        let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        Self(std::env::var_os(variable).map(PathBuf::from))
    }

    /// 问不出家目录的那一种。
    pub fn unknown() -> Self {
        Self(None)
    }

    /// 问得出家目录没有：问不出时输入行不从 `~/` 起。
    pub fn is_known(&self) -> bool {
        self.0.is_some()
    }

    /// 屏上怎么写这一条路径：家目录那一截缩写成 `~`，别的原样。
    ///
    /// **只缩写整一截**：`/home/a` 是家目录时 `/home/ab/x` 不缩写——`~b/x` 是另一条路径。
    /// 家目录本身写成 `~`。分隔符照屏上的写法用 `/`，与设计稿一致。
    pub fn abbreviate(&self, path: &Path) -> String {
        let shown = |path: &Path| path.display().to_string();
        let Some(home) = &self.0 else {
            return shown(path);
        };
        match path.strip_prefix(home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", shown(rest).replace(std::path::MAIN_SEPARATOR, "/")),
            Err(_) => shown(path),
        }
    }

    /// 输入行里打进来的一条路径落到盘上是哪儿：`~/` 开头换成家目录，`~` 一个字就是家目录。
    /// 问不出家目录时 `~` 只是一个字，原样交回。
    pub fn expand(&self, typed: &str) -> PathBuf {
        match (&self.0, typed.strip_prefix("~/")) {
            (Some(home), Some(rest)) => home.join(rest),
            (Some(home), None) if typed == "~" => home.clone(),
            _ => PathBuf::from(typed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 家目录底下的路径缩写成 `~/…`，家目录本身是 `~`，外面的原样。
    #[test]
    fn a_path_under_home_is_abbreviated_with_a_tilde() {
        let home = Home::at("/home/alex");
        assert_eq!(home.abbreviate(Path::new("/home/alex/漫画库")), "~/漫画库");
        assert_eq!(
            home.abbreviate(Path::new("/home/alex/下载/第01卷.cbz")),
            "~/下载/第01卷.cbz"
        );
        assert_eq!(home.abbreviate(Path::new("/home/alex")), "~");
        assert_eq!(home.abbreviate(Path::new("/srv/库")), "/srv/库");
        // 只缩写整一截：`/home/alexander` 不是家目录底下的。
        assert_eq!(
            home.abbreviate(Path::new("/home/alexander/x")),
            "/home/alexander/x"
        );
    }

    /// 问不出家目录时一个字都不缩写，`~` 也只是一个字。
    #[test]
    fn without_a_home_nothing_is_abbreviated_and_the_tilde_is_a_character() {
        let home = Home::unknown();
        assert_eq!(
            home.abbreviate(Path::new("/home/alex/漫画库")),
            "/home/alex/漫画库"
        );
        assert_eq!(home.expand("~/漫画库"), PathBuf::from("~/漫画库"));
    }

    /// 输入行认 `~/`：展开成家目录底下的那条路径；缩写再展开是一次往返。
    #[test]
    fn a_typed_tilde_path_expands_to_the_home_and_round_trips() {
        let home = Home::at("/home/alex");
        assert_eq!(home.expand("~/漫画库"), PathBuf::from("/home/alex/漫画库"));
        assert_eq!(home.expand("~"), PathBuf::from("/home/alex"));
        assert_eq!(home.expand("/srv/库"), PathBuf::from("/srv/库"));
        let path = Path::new("/home/alex/Comics/棋魂");
        assert_eq!(home.expand(&home.abbreviate(path)), path);
    }
}
