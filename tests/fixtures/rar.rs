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

/// 造一组**分卷序列**：`name.part1.rar` … `name.partN.rar`，落在 `dir` 下，按顺序返回。
///
/// 上面三份非签进仓不可，因为票面要的是「真压过」与「真加密」——那两样只有 RARLAB 的
/// 写入端给得出。分卷这一份不同：它要的是**归档头里那一位**（`p4-parking-lot/17`），
/// 而成员一律**存储不压**——存储那一档的成员字节原样躺在包里，一个压缩算法都不碰，
/// 因此这一份用不着 UnRAR 许可挡着的那一半（见本模块抬头）。
///
/// 装的是 [`members`] 那三个成员，与另外三种格式同一批内容——「同内容的单份 `.rar`
/// 与这一组出同一份产物」那一条因此断得起逐字节的等号。
///
/// **每一道卷边界都落在一个成员当中**（那一条成员被劈成两半，前一半带「后面还有」、
/// 后一半带「前面未完」）。这不是为了逼真：UnRAR 认「这一份是不是头一份」时，
/// 归档头里那个卷号会被**头一条成员头上那一位**改写（`archive.cpp` 的 `IsArchive`），
/// 边界正好落在两个成员之间的包，续的那几份会被认成头一份。因此 `parts` 不能超过成员数。
pub fn write_split(dir: &Path, name: &str, parts: usize) -> Vec<PathBuf> {
    split_volumes(parts)
        .into_iter()
        .enumerate()
        .map(|(index, bytes)| write(dir.join(format!("{name}.part{}.rar", index + 1)), &bytes))
        .collect()
}

/// RAR5 的签名。
const SIGNATURE: &[u8] = b"Rar!\x1a\x07\x01\x00";

/// 块头上那三位：这个块后面跟着数据 / 这一条成员前面未完 / 这一条成员后面还有。
const HAS_DATA: u64 = 0x0002;
const SPLIT_BEFORE: u64 = 0x0008;
const SPLIT_AFTER: u64 = 0x0010;

/// [`write_split`] 那几份的字节。分块见该函数的文档。
fn split_volumes(parts: usize) -> Vec<Vec<u8>> {
    let members = members();
    assert!(
        (1..=members.len()).contains(&parts),
        "分卷数要落在 1..={}：每一道边界都得劈开一条成员",
        members.len()
    );
    (0..parts)
        .map(|index| {
            let mut bytes = SIGNATURE.to_vec();
            // 卷号只有续的那几份带着，头一份不带——UnRAR 按「带没带」认头一份。
            bytes.extend_from_slice(&archive_block(
                parts > 1,
                (index > 0).then_some(index as u64),
            ));
            for fragment in fragments(&members, index, parts) {
                bytes.extend_from_slice(&file_block(&fragment));
            }
            bytes.extend_from_slice(&end_block(index + 1 < parts));
            bytes
        })
        .collect()
}

/// 第 `index` 份里躺着哪几段：上一条成员的后一半、加这一条成员的前一半；
/// 末一份把剩下的成员整条装完。
fn fragments<'a>(
    members: &'a [(&'a str, Vec<u8>)],
    index: usize,
    parts: usize,
) -> Vec<Fragment<'a>> {
    let mut fragments = Vec::new();
    if index > 0 {
        let (name, whole) = &members[index - 1];
        fragments.push(Fragment::half(name, whole, Half::Tail));
    }
    if index + 1 < parts {
        let (name, whole) = &members[index];
        fragments.push(Fragment::half(name, whole, Half::Head));
    } else {
        fragments.extend(
            members[index..]
                .iter()
                .map(|(name, whole)| Fragment::whole(name, whole)),
        );
    }
    fragments
}

/// 一条成员在**这一份**里的那一段。
struct Fragment<'a> {
    name: &'a str,
    /// 整条解开有多少字节。劈成两半的那一条上，两半写的都是整条的数。
    unpacked: u64,
    /// 整条的 CRC32。同上：两半写的都是整条的。
    crc: u32,
    /// 这一段的字节。
    data: &'a [u8],
    /// 前面未完 / 后面还有。
    before: bool,
    after: bool,
}

enum Half {
    Head,
    Tail,
}

impl<'a> Fragment<'a> {
    fn whole(name: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            name,
            unpacked: bytes.len() as u64,
            crc: crc32(bytes),
            data: bytes,
            before: false,
            after: false,
        }
    }

    fn half(name: &'a str, bytes: &'a [u8], half: Half) -> Self {
        let at = bytes.len() / 2;
        let (data, before, after) = match half {
            Half::Head => (&bytes[..at], false, true),
            Half::Tail => (&bytes[at..], true, false),
        };
        Self {
            name,
            unpacked: bytes.len() as u64,
            crc: crc32(bytes),
            data,
            before,
            after,
        }
    }
}

/// 归档头（类型 1）：这一份是不是分卷序列里的一员，是第几份。
fn archive_block(volume: bool, number: Option<u64>) -> Vec<u8> {
    let mut body = Vec::new();
    vint(
        u64::from(volume) | if number.is_some() { 0x0002 } else { 0 },
        &mut body,
    );
    if let Some(number) = number {
        vint(number, &mut body);
    }
    block(1, 0, &body, &[])
}

/// 成员头（类型 2）加它那一段字节。压缩方式一律**存储不压**。
fn file_block(fragment: &Fragment<'_>) -> Vec<u8> {
    let mut body = Vec::new();
    vint(0x0004, &mut body); // 只带 CRC32，不带时间戳——字节因此稳定
    vint(fragment.unpacked, &mut body);
    vint(0, &mut body); // 属性
    body.extend_from_slice(&fragment.crc.to_le_bytes());
    vint(0, &mut body); // 压缩信息：版本 0、不固实、存储不压、最小字典
    vint(0, &mut body); // 打包的那台机器
    vint(fragment.name.len() as u64, &mut body);
    body.extend_from_slice(fragment.name.as_bytes());
    let flags = HAS_DATA
        | if fragment.before { SPLIT_BEFORE } else { 0 }
        | if fragment.after { SPLIT_AFTER } else { 0 };
    block(2, flags, &body, fragment.data)
}

/// 收尾头（类型 5）：后面还有没有下一份。
fn end_block(more: bool) -> Vec<u8> {
    let mut body = Vec::new();
    vint(u64::from(more), &mut body);
    block(5, 0, &body, &[])
}

/// 一个块：`[头 CRC32][头长][类型][标志][数据长][块自己那几项][数据]`。
/// 头 CRC32 盖的是「头长」那一格起、到头末为止的那一串，数据不在里面。
fn block(kind: u64, flags: u64, body: &[u8], data: &[u8]) -> Vec<u8> {
    let mut header = Vec::new();
    vint(kind, &mut header);
    vint(flags, &mut header);
    if flags & HAS_DATA != 0 {
        vint(data.len() as u64, &mut header);
    }
    header.extend_from_slice(body);

    let mut sized = Vec::new();
    vint(header.len() as u64, &mut sized);
    sized.extend_from_slice(&header);

    let mut out = crc32(&sized).to_le_bytes().to_vec();
    out.extend_from_slice(&sized);
    out.extend_from_slice(data);
    out
}

/// RAR5 的变长整数：一字节七位，低位在前，最高位置 1 表示后面还有。
fn vint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}
