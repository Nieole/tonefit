//! `.rar` 夹具：三份**造好了签进仓**的归档。
//!
//! 与 CBZ（`super::cbz`，按字节手搓）和 `.7z`（`super::sevenz`，借编码端现造）都不同，
//! 这一份是**跑不出来的**：`.rar` 的写入端只有 RARLAB 自己有——UnRAR 许可明写着不许据它
//! 反推 RAR 的压缩算法（见 `THIRD-PARTY-NOTICES.md`），因此 Rust 这边没有、将来也不会有
//! 一个造 `.rar` 的库。而票面要的两条恰恰非真包不可：**固实压的**那一份要真压过，
//! **加密的**那一份要真加密。所以这三份是拿 WinRAR 的 `Rar.exe` 造好、当成字节签进仓的。
//!
//! # 它们是怎么造的
//!
//! 先把三个成员摆成一棵树（`ch1/001.png` 是 [`super::cheap_page`]、
//! `ch1/002.png` 是 [`super::gradient`] 配 [`super::TINY`]，两张都编成 PNG；
//! `ComicInfo.xml` 是下面那个 [`COMIC_INFO`]），再在那棵树的根上各跑一条：
//!
//! ```text
//! Rar.exe a -ma5 -r -s  -m5 -tsm- -tsc- -tsa- solid.rar     ch1 ComicInfo.xml
//! Rar.exe a -ma5 -r -s- -m0 -tsm- -tsc- -tsa- stored.rar    ch1 ComicInfo.xml
//! Rar.exe a -ma5 -r     -m5 -hptonefit -tsm- -tsc- -tsa- encrypted.rar ch1 ComicInfo.xml
//! ```
//!
//! `-s` 固实、`-s-` 不固实、`-m0` 存储不压、`-hp` 连**归档头**一起加密（口令 `tonefit`），
//! `-ts*-` 不存时间戳好让字节稳定。
//!
//! **页是生成出来的，不是随手找的图**：动了 [`super::cheap_page`] 或 [`super::gradient`]，
//! 这三份夹具就与用例对不上了，那时「两种格式一份内容」那一条会**当场红**，不会静默地过。
//!
//! # 只用 PNG，为什么
//!
//! 包里那两页都是 PNG，没有 `.7z` 那份夹具里的 JPEG。有损格式的编码结果随 `image` 的版本
//! 变，而这三份归档的字节是签进仓的、不跟着变——两边一旦分道，用例比的就不再是这一趟的差别。
//! PNG 无损，重编一遍解出来的像素还是同一批，产物因此照旧逐字节相同。

use std::path::{Path, PathBuf};

/// **固实**压的那一份：整批成员压成一条连续的流，取第 N 个要从块头解起。
/// 「开工前整卷摊到临时目录」这条决定（ADR 0015 决定第 3 条）针对的就是它。
pub const SOLID: &[u8] = include_bytes!("rar/solid.rar");

/// **存储、不固实**的那一份，成员与 [`SOLID`] 逐一相同。
///
/// 与 [`SOLID`] 成对：读取形态**按格式分、不逐卷探固实与否**，因此这两份该出同一个产物。
pub const STORED: &[u8] = include_bytes!("rar/stored.rar");

/// **连归档头一起加密**的那一份：列成员就要口令，而 tonefit 没有问口令的地方。
pub const ENCRYPTED: &[u8] = include_bytes!("rar/encrypted.rar");

/// [`SOLID`] 与 [`STORED`] 里那个透传成员的字节。
///
/// 与 `tests/container.rs` 里那一份同字：透传要逐字节一致，因此故意带上非 ASCII 与换行。
pub const COMIC_INFO: &str =
    "<?xml version=\"1.0\"?>\n<ComicInfo><Title>卷一</Title></ComicInfo>\n";

/// 把一份夹具落到 `path` 上，返回它。
pub fn write(path: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
    let path = path.as_ref();
    std::fs::write(path, bytes).expect("写 .rar 夹具");
    path.to_path_buf()
}

/// [`SOLID`] 与 [`STORED`] 里装着的那三个成员：包里的名字，配它那一串字节。
///
/// 批 spec 的《Testing Decisions》要「同一批页从 `.cbz` / `.zip` / `.rar` / `.7z` 进去，
/// 出来逐字节相同」，而**同一批页**这个前提得有个出处：`.rar` 那一侧由本模块抬头那几条
/// 命令钉着，另外三个格式由这一句钉着——各家搭建器的 `file` 都收「名字 + 字节」，
/// 照这张表灌一遍，四个包装的就是同一份内容。
///
/// 页也走 `file` 而不是 `page`：这里交的是**已经编好的** PNG 字节，
/// 而是不是一页只看扩展名。
pub fn members() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "ch1/001.png",
            super::encode_image(&super::cheap_page(), "png"),
        ),
        (
            "ch1/002.png",
            super::encode_image(&super::gradient(super::TINY), "png"),
        ),
        ("ComicInfo.xml", COMIC_INFO.as_bytes().to_vec()),
    ]
}

/// 把一份夹具落到 `path` 上，但把某个字节**打坏**：归档头完好、列得出成员，
/// 真去解那一段才发现解不开。
///
/// 只对 [`STORED`] 有意义——它存储不压，成员的字节原样躺在文件中段，
/// 动它就是动那一页本身，而两头的归档头一个字节都碰不到。
pub fn write_with_a_broken_member(path: impl AsRef<Path>, at: usize) -> PathBuf {
    let mut bytes = STORED.to_vec();
    assert!(at < bytes.len(), "打坏的位置落在夹具之外");
    bytes[at] = !bytes[at];
    write(path, &bytes)
}
