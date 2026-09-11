//! 输出 PNG 的自描述元数据：判定与理由随文件走，同一批字段兼作幂等依据。
//!
//! 记录写进 tEXt（ADR 0006：判定与理由写进输出 PNG 的 tEXt）。**它就在文件里**，
//! 不在任何外部状态库：文件被移动、改名、重新打包都不会丢，而 tonefit 也就不必为了幂等
//! 去维护一份全库索引——那正是 ADR 0009 关掉的东西。
//!
//! 七个字段分三摞，外加一项只在默认路径上才有的：
//!
//! - **幂等依据**四项：工具版本、profile 名、参数哈希、源哈希。重跑时读回来逐项比，
//!   四项都对得上就不必重做（见 [`Fingerprint`]）。
//! - **来路**一项：这一张来自哪个源成员，以及它在那一族里排第几、一共几张（见 [`Origin`]）。
//!   它不是「要不要重做」的依据，是让幂等**问得出这个问题**的索引——一个源页产出几张
//!   由内容决定（跨页拆分，页几何批 04 号票），输出成员名因此在碰像素之前预告不出来。
//! - **判定记录**两项：判定与理由。两者由前四项推出来，因此不进比对；它们在这里，
//!   是为了让「这一页为什么是这一档」随文件走（spec 的 story 7）。
//! - **页级源哈希**一项（two-pass-rework/13）：只算这一张来自的那个源成员，与来路合起来
//!   是这一页**自己**的幂等依据（见 [`PageSource`]）。它摆在卷级那四项**旁边**，不替代它们：
//!   整卷齐着仍按卷跳；卷不齐时按它逐页留下没变的页（two-pass-rework/14，
//!   见 `crate::compare_with_the_prior_output`）。
//!
//! tEXt 的取值是 Latin-1，中文写不进去：本模块产出的字符串一律 ASCII。
//! 报告那一侧的中文说法（见 `crate::report` 与 `main`）不替代它，它也不替代报告——
//! 一份给人当场读，一份给几个月后打开这个文件的人读。

use std::fmt::Write as _;
use std::io::{BufRead, Seek};
use std::path::Path;

use crate::decide::{Reason, Verdict};
use crate::decode::Salvage;
use crate::geometry::Size;
use crate::metric::{Aggregation, Composition, Masking, aggregation, composition, masking};
use crate::quantize::Dither;
use crate::request::Request;

/// 工具版本那一项：包名加版本号。
const TOOL: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

/// 两个哈希写进 tEXt 的长度：blake3 的前 128 位，32 个十六进制字符。
///
/// 幂等依据问的是「变了没有」，不是抗构造碰撞：128 位远在偶然碰撞之外。截到这个长度，
/// 是因为这几行字段要给人读——两串 64 字符的十六进制摆在那里，谁也不会看第二眼。
const HASH_HEX: usize = 32;

/// tEXt 的关键字。
///
/// `Software` 是 PNG 规范登记过的那个，含义正是「哪个软件写出了这个文件」；其余四项按本工具
/// 的名字加前缀，免得与别的工具写进同一个文件的记录撞名。
const TOOL_KEYWORD: &str = "Software";
const PROFILE_KEYWORD: &str = "tonefit:profile";
const PARAMS_KEYWORD: &str = "tonefit:params";
const SOURCE_KEYWORD: &str = "tonefit:source";
const PAGE_SOURCE_KEYWORD: &str = "tonefit:page-source";
const ORIGIN_KEYWORD: &str = "tonefit:origin";
const VERDICT_KEYWORD: &str = "tonefit:verdict";
const REASON_KEYWORD: &str = "tonefit:reason";

/// 幂等依据：四项都对得上，这一卷就不必重做（ADR 0006）。
///
/// 判定与理由不在里面：它们是这四项推出来的结果，比它们等于比同一件事两遍。
///
/// 四项里有三项管像素，`profile` 那一项不管：同一块面板的两个别名
/// （`kobo-libra-2` 与 `kobo-libra-h2o`）输出逐字节相同，参数哈希因此不收型号名
/// （见 [`params_hash`]）。它仍然进比对，因为记录要说得出**这批输出该拿去哪台设备看**——
/// 那是 [`crate::Report::profile`] 存在的同一个理由。说错了型号的记录就是过期的记录，
/// 哪怕像素一个不差。换别名因此会重做一遍，这是明知故犯的交换。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    tool: String,
    profile: String,
    params: String,
    source: String,
}

impl Fingerprint {
    /// 本次调用在这一卷上的依据。`source` 是卷级源哈希，见 [`SourceHasher`]。
    pub fn new(request: &Request, source: String) -> Self {
        Self {
            tool: TOOL.to_owned(),
            profile: request.profile.device().to_owned(),
            params: params_hash(request),
            source,
        }
    }

    /// 除源哈希之外那三项——工具版本、profile 名、参数哈希——都对得上吗。
    ///
    /// **页级依据**要的正是这三项（two-pass-rework/14）：源那一项换成这一页自己的
    /// [`PageSource`]。卷级那一份说「这一卷变没变」，而按页跳过问的是「这一页变没变」
    /// ——卷里别的页改了，卷级源哈希必变，这一页却不该因此重做。
    pub fn same_but_for_the_source(&self, other: &Self) -> bool {
        self.tool == other.tool && self.profile == other.profile && self.params == other.params
    }
}

/// 一张输出页的**来路**：它来自哪个源成员，在那一族里排第几，那一族一共几张。
///
/// # 它为什么存在
///
/// 幂等要在**碰像素之前**答完（ADR 0006），而一个源页产出几张输出页由内容决定
/// ——有装订沟就切成两张，没有就一张（页几何批 04 号票）。输出成员名因此预告不出来，
/// 从前那条「按预告的名单逐个比指纹」的路走不下去了。这一项把方向反过来：
/// 名单从**上一趟写在输出里的记录**读回来，一张输出页自己说得出它来自哪个源页、
/// 那一族该有几张（见 `crate::compare_with_the_prior_output`）。
///
/// **「输出里少一张就该察觉」这条能力因此没有丢**，而且比从前更严：一族两张里删掉一张，
/// 剩下那一张仍写着「1/2」，第二张找不到，这一卷重做。缺了这个计数就只剩「至少有一张」，
/// 一张跨页被删掉半边会静默地留在输出里——`p0-hardening/03` 要拦的正是这一类。
///
/// # 取值写法
///
/// `<源成员相对路径> <第几>/<共几>`，序号从 1 数起，例如 `001.jpg 1/2`。
/// 分隔符按 `/` 归一（与 [`SourceHasher`] 同一条规矩：同一个卷在 Windows 与别处
/// 要给出同一份记录）。tEXt 只装得下 Latin-1，而成员名里中文与日文是常态，
/// 因此非 ASCII 的字节按 UTF-8 写成 `%XX`（见 [`escape`]）。
///
/// 比对只在**转义之后**的写法上做，没有反转义：读回来的是什么，就拿源成员名转义一遍去比。
/// 少一个方向的代码，也就少一处两个方向对不上的可能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// 源成员的相对路径，已转义成 ASCII。
    member: String,
    /// 这一张在那一族里排第几，从 0 数起。
    ordinal: usize,
    /// 那一族一共几张。
    count: usize,
}

impl Origin {
    /// 一个源成员切出来的第 `ordinal` 张（从 0 起），那一族共 `count` 张。
    pub fn new(relative: &Path, ordinal: usize, count: usize) -> Self {
        Self {
            member: escape(&normalized(relative)),
            ordinal,
            count,
        }
    }

    /// 这一族一共几张。幂等靠它知道还该去找几张（见 `crate::compare_with_the_prior_output`）。
    pub fn count(&self) -> usize {
        self.count
    }

    /// 写进 tEXt 的那串字。
    fn text(&self) -> String {
        format!("{} {}/{}", self.member, self.ordinal + 1, self.count)
    }

    /// 从 tEXt 里读回来。写法对不上、序号越界都是 `None`——那不是本工具写的记录。
    fn parse(text: &str) -> Option<Self> {
        let (member, position) = text.rsplit_once(' ')?;
        let (ordinal, count) = position.split_once('/')?;
        let ordinal: usize = ordinal.parse().ok()?;
        let count: usize = count.parse().ok()?;
        if ordinal == 0 || ordinal > count {
            return None;
        }
        Some(Self {
            member: member.to_owned(),
            ordinal: ordinal - 1,
            count,
        })
    }
}

/// 页级源哈希：一张输出页**自己那一份**幂等依据里代表源的那一项（two-pass-rework/13）。
///
/// 卷级那一份（[`Fingerprint`] 的 `source`）说的是「这一卷变没变」；这一份说的是
/// 「这一张来自的那个源成员变没变」。默认路径上一页的档只取决于它自己
/// （ADR 0018 决定第 2 条：不做迟滞），这一问因此才有答案——卷级上包络那条路上答不了，
/// 一页的档由全卷定。
///
/// # 为什么是多一个键，而不是 `tonefit:source` 那一格多一层形态
///
/// 旧记录只有卷级那一份，而它们必须**照旧读得懂、照旧按卷比**：卷级那一问一个字不改
/// （[`PageRecord::matches`]）。多一个键，旧读法碰都不碰它；改 `source` 的写法则每一个读它的地方
/// 都得先拆再比，[`Fingerprint`] 的相等也就不再是「四项逐字相同」。缺了这个键的记录
/// 读回来是 `page_source: None`——它是老形态的记录，不是坏记录：整卷齐着照旧整卷跳，
/// 只是卷不齐时它留不下来。
///
/// # 喂什么
///
/// 与 [`SourceHasher`] 喂一个成员时**逐字节相同**（名字带长度前缀、字节带长度前缀），只是只喂
/// 这一个成员：同一条规矩，两个作用域。名字照喂，理由与卷级那一份相同——两页对调名字，
/// 输出整个错位。
///
/// # 跨页拆分下它指得回哪一半
///
/// 一个源页切出两张输出页，两张的页级源哈希**相同**：它们来自同一个成员的同一批字节。
/// 哪一半由 [`Origin`] 说（`001.jpg 1/2` 对 `001.jpg 2/2`）——两项合在一起才是页级依据，
/// 这一项单独不成立。
///
/// # 哪些页有
///
/// 只在**这一页的字节只取决于它自己**时写：默认路径与覆盖顶死那一趟（`crate::Settles`
/// 答「第一遍就编好」的那两条）。上包络那条路一页的档由全卷定，失败页的占位尺寸由全卷定
/// （卷内统一尺寸），两处都不写；`--no-metadata` 那一趟连算都不算。
///
/// # 谁读它
///
/// 按页跳过（two-pass-rework/14，见 `crate::compare_with_the_prior_output`）：幂等那一道
/// 给每个源页各算一份，拿去与上一趟写在输出里的这一项比——对得上、来路也对得上、
/// 其余三项依据没变，这一页就**留下**，不解码、不判、不编。写它与读它用的是同一个
/// [`PageSource::of`]，两侧因此不会各算各的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSource(String);

impl PageSource {
    /// 一个源成员的页级源哈希：名字与字节都算，规矩见类型文档。
    pub fn of(relative: &Path, bytes: &[u8]) -> Self {
        let mut hasher = SourceHasher::new();
        hasher.member(relative, bytes);
        Self(hasher.finish())
    }

    /// 写进 tEXt 的那串字。
    fn text(&self) -> &str {
        &self.0
    }

    /// 从 tEXt 里读回来。不是 [`HASH_HEX`] 个十六进制字符就是 `None`——那不是本工具写的记录。
    fn parse(text: &str) -> Option<Self> {
        (text.len() == HASH_HEX && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| Self(text.to_owned()))
    }
}

/// 一页输出 PNG 里读回来的记录，幂等要的那两摞，加上页级那一项。
///
/// 两摞一趟读回来而不是各读一次：读一页记录要开文件、解 PNG 头，那笔成本
/// 「跳过一卷比重做一卷便宜」全靠它压着（见 [`RECORD_PREFIX`]）。
pub struct PageRecord {
    /// 幂等依据那四项。
    pub fingerprint: Fingerprint,
    /// 这一张写出时的像素尺寸，读自 IHDR——读记录本来就要解到那一块，白拿的。
    ///
    /// 留下的页要它（two-pass-rework/14）：占位页按卷内统一尺寸出，而那个尺寸是全卷的众数，
    /// 留下的页也在分母里——不数它们，一卷只重做一页时众数就由那一页说了算。
    pub size: Size,
    /// 这一张的来路。**旧记录没有这一项**——它是页几何批 04 号票加的。
    ///
    /// 缺了它幂等**不命中**：一张说不出自己那一族有几张的页，证不了「输出里没少东西」，
    /// 而幂等从不该给一个证不出来的命中。代价是升级之后每一卷重做一趟，
    /// 那一趟之后每一页都带着它。
    pub origin: Option<Origin>,
    /// 这一张自己那一份源哈希（two-pass-rework/13）。**旧记录没有这一项**，上包络那条路
    /// 与失败页也没有（见 [`PageSource`]）——缺了它是「页级答不了」，不是坏记录；
    /// 卷级那四项照旧比、照旧命中，只是这一页留不下来、整卷不齐时跟着重做
    /// （见 [`PageRecord::matches_by_page`]）。
    pub page_source: Option<PageSource>,
}

impl PageRecord {
    /// 从一页输出 PNG 里读回记录。幂等那四项缺一项就是 `None`——没有记录的输出
    /// （`--no-metadata` 写出的，或别的工具写的）不构成幂等的依据。
    ///
    /// 只读到第一个 IDAT 为止，一个像素都不解：ADR 0006 认下的「读回 tEXt 比对」就是这一步，
    /// 它的成本要停在这里，否则跳过一卷比重做一卷还贵。
    pub fn read(source: impl BufRead + Seek) -> Option<Self> {
        let reader = png::Decoder::new(source).read_info().ok()?;
        let info = reader.info();
        let text = &info.uncompressed_latin1_text;
        let field = |keyword: &str| {
            text.iter()
                .find(|chunk| chunk.keyword == keyword)
                .map(|chunk| chunk.text.clone())
        };
        Some(Self {
            fingerprint: Fingerprint {
                tool: field(TOOL_KEYWORD)?,
                profile: field(PROFILE_KEYWORD)?,
                params: field(PARAMS_KEYWORD)?,
                source: field(SOURCE_KEYWORD)?,
            },
            size: Size::new(info.width, info.height),
            origin: field(ORIGIN_KEYWORD).as_deref().and_then(Origin::parse),
            page_source: field(PAGE_SOURCE_KEYWORD)
                .as_deref()
                .and_then(PageSource::parse),
        })
    }

    /// 这一页记的是「那个源成员切出来的第几张」吗——来路对得上。
    ///
    /// 来路缺项即不是，理由见 [`PageRecord::origin`]。它只答这一张**属于哪一格**，
    /// 不答该不该重做：那一问分两个作用域，见下面两个方法。
    pub fn is_the_page(&self, relative: &Path, ordinal: usize, count: usize) -> bool {
        self.origin.as_ref() == Some(&Origin::new(relative, ordinal, count))
    }

    /// 这一页记的正是「这份指纹下、那个源成员切出来的第几张」吗——**卷级**依据。
    ///
    /// 两样都要对上：指纹（这一卷变没变）、来路（这一张属于哪个源页的哪一格）。
    pub fn matches(
        &self,
        fingerprint: &Fingerprint,
        relative: &Path,
        ordinal: usize,
        count: usize,
    ) -> bool {
        self.fingerprint == *fingerprint && self.is_the_page(relative, ordinal, count)
    }

    /// 这一页记的正是「这一趟的工具、profile 与参数下，字节没变的那个源成员切出来的第几张」吗
    /// ——**页级**依据（two-pass-rework/14）。
    ///
    /// 与 [`matches`](Self::matches) 只差源那一项：卷级源哈希换成这一页自己的
    /// [`PageSource`]。旧记录没有那一项，答的恒是否——它证不了「这一页没变」，
    /// 而幂等从不该给一个证不出来的命中。
    pub fn matches_by_page(
        &self,
        fingerprint: &Fingerprint,
        page_source: &PageSource,
        relative: &Path,
        ordinal: usize,
        count: usize,
    ) -> bool {
        self.fingerprint.same_but_for_the_source(fingerprint)
            && self.page_source.as_ref() == Some(page_source)
            && self.is_the_page(relative, ordinal, count)
    }
}

/// 把一页输出 PNG 里**卷级源哈希那一项**改写成 `fingerprint` 的，其余字节一个不动
/// （two-pass-rework/14：留下的页）。
///
/// 留下的页是上一趟写的，它那一项记着上一趟的卷级源哈希；卷里别的页改了，那个数就过期了。
/// 改写它，这一趟的输出才与整卷重做那一趟**逐字节相同**——下一趟卷级那四项每一页都对得上，
/// 整卷跳过，不必再逐页比；不改写，留下的页会一直记着旧数，那一卷从此每趟都要按页比、
/// 按页搬。像素、页级源哈希、来路、判定与理由在这一页上都只取决于它自己，
/// 因此都是对的，不必碰。
///
/// 只改那一块：tEXt 的数据是「关键字 `\0` 取值」，取值恒是 [`HASH_HEX`] 个字符，
/// 新旧等长，原地覆盖再重算那一块的 CRC（PNG 的 CRC-32 盖的是块类型加数据）即可，
/// 一个像素都不解、不重编。找不到那一块、长度对不上，就是这一页不是本工具写的
/// ——那样的页留不下来（见 [`PageRecord::read`]），走到这里是调用方的错，当场报。
pub fn restamp_source(png: &mut [u8], fingerprint: &Fingerprint) -> anyhow::Result<()> {
    const SIGNATURE: usize = 8;
    let source = fingerprint.source.as_bytes();
    // 新值与旧值都得恰好是那么长——原地覆盖靠的正是等长。指纹里那一项由 [`SourceHasher::finish`]
    // 写出，恒是这个长度；这里再问一遍，是不让一个长度不对的指纹变成一次越界。
    anyhow::ensure!(
        source.len() == HASH_HEX,
        "这一趟的卷级源哈希长 {} 字节，不是 {HASH_HEX}",
        source.len()
    );
    let mut at = SIGNATURE;
    let mut leader = SOURCE_KEYWORD.as_bytes().to_vec();
    leader.push(0);
    while at + 8 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().expect("4 字节")) as usize;
        let (kind, data) = (at + 4, at + 8);
        // 长度是文件里读来的数，先问一句再拿它算位置：一个坏文件不该变成一次溢出。
        let Some(crc) = data.checked_add(length).filter(|crc| crc + 4 <= png.len()) else {
            break;
        };
        if &png[kind..data] == b"tEXt" && png[data..crc].starts_with(&leader) {
            let value = data + leader.len();
            anyhow::ensure!(
                crc - value == HASH_HEX,
                "卷级源哈希那一项长 {} 字节，不是 {HASH_HEX}",
                crc - value
            );
            png[value..crc].copy_from_slice(source);
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&png[kind..crc]);
            png[crc..crc + 4].copy_from_slice(&hasher.finalize().to_be_bytes());
            return Ok(());
        }
        at = crc + 4;
    }
    anyhow::bail!("这一页里没有卷级源哈希那一项，不是本工具写的记录")
}

/// 一个相对路径写成一串字：分隔符按 `/` 归一。
///
/// 与 [`SourceHasher::feed`] 那一处同一条规矩，也同一个理由：同一个卷在 Windows 与别处
/// 要算出同一份记录。
fn normalized(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 把一串字转义成 ASCII：非 ASCII 的字节按 UTF-8 写成 `%XX`，`%` 自己与空格也转。
///
/// tEXt 只装得下 Latin-1，而本模块产出的字符串一律 ASCII（见模块文档）。成员名里
/// 中文与日文是常态，直接写进去编码器会当场拒绝。
///
/// `%` 要转，否则一个真叫 `%41.jpg` 的成员与转义出来的 `A` 撞在一起；
/// 空格要转，因为它是 [`Origin::text`] 里名字与序号之间的分隔符。
fn escape(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    for byte in name.bytes() {
        if byte.is_ascii() && byte != b'%' && byte != b' ' && !byte.is_ascii_control() {
            text.push(byte as char);
        } else {
            let _ = write!(text, "%{byte:02X}");
        }
    }
    text
}

/// 记录落在文件开头这么多字节以内。
///
/// 写出来的 PNG 是 IHDR、色板、那几个 tEXt、IDAT 这个次序（见 `crate::encode`），
/// 记录因此紧挨着文件头。读的那一端顺着这个事实只取开头一截：一页 PNG 有好几 MB，
/// 为了几行记录把它整个搬进内存不值当。
///
/// 截短了只会读不出记录——那时这一卷重做，不会得出错的结论。
pub const RECORD_PREFIX: u64 = 64 * 1024;

/// 卷级源哈希的累加器：按阅读顺序把每个成员的相对路径与字节喂进去，页与透传文件都算。
///
/// 作用域为什么是卷而不是页，见 ADR 0006 的《决定》末段。
pub struct SourceHasher(blake3::Hasher);

/// 读不出字节的成员在源哈希里占的那个长度前缀。真实成员到不了这个长度。
const UNREADABLE: u64 = u64::MAX;

impl SourceHasher {
    pub fn new() -> Self {
        Self(blake3::Hasher::new())
    }

    /// 喂进一个成员：名字与字节都算。
    ///
    /// 只算字节的话，两页对调名字看不出来，而输出会整个错位。
    /// 名字与字节各自带上长度前缀，两个成员的拼接才不会与另一种切法撞上。
    pub fn member(&mut self, relative: &Path, bytes: &[u8]) {
        self.feed(relative, bytes.len() as u64, bytes);
    }

    /// 喂进一个**字节读不出来**的成员：名字照算，字节那一半换成一个固定的记号。
    ///
    /// 这样的成员在第一遍里会变成失败页（12 号票），但它在这一遍**不能被跳过**：
    /// 跳过它，把一个坏成员从卷里删掉之后哈希纹丝不动，那一卷会被静默地跳过，
    /// 而它明明少了一页。
    ///
    /// 记号是长度前缀写成 [`UNREADABLE`]，与任何真实字节串都撞不上——
    /// 那个长度是任何一段真实字节都到不了的。
    pub fn unreadable(&mut self, relative: &Path) {
        self.feed(relative, UNREADABLE, &[]);
    }

    fn feed(&mut self, relative: &Path, length: u64, bytes: &[u8]) {
        // 分隔符按 `/` 归一：同一个卷在 Windows 与别处要算出同一个哈希（见 [`normalized`]）。
        let name = normalized(relative);
        self.0.update(&(name.len() as u64).to_le_bytes());
        self.0.update(name.as_bytes());
        self.0.update(&length.to_le_bytes());
        self.0.update(bytes);
    }

    /// 收口成写进 tEXt 的那串十六进制。
    pub fn finish(self) -> String {
        hex(self.0.finalize())
    }
}

/// 彩色分支那两项的取值。那条路径不量化，没有判定位深可写（ADR 0005 决定第 4 条）。
const COLOR_VERDICT: &str = "color";
const COLOR_REASON: &str = "color branch, scaled only";

/// 失败页那两项的取值。那一页没解出来，也就没有判定位深可写（12 号票）。
///
/// 报告那一侧的原因具体到「哪个成员、卡在哪一步」，这一句钉死不变：tEXt 只装得下 Latin-1，
/// 而原因里有成员名与中文。占位页因此自己说得出「我是个占位页」——
/// 隔离目录之外，这是它随身带着的第二处标记。
const FAILED_VERDICT: &str = "failed";
const FAILED_REASON: &str = "page could not be decoded, blank placeholder";

/// 一页要写进 tEXt 的全部字段：幂等那四项由整卷共用，来路、判定与理由逐页各一份，
/// 页级源哈希逐页各一份而且**不一定在**（见 [`PageSource`] 的《哪些页有》）。
pub struct Record<'a> {
    fingerprint: &'a Fingerprint,
    origin: String,
    page_source: Option<String>,
    verdict: String,
    reason: String,
}

impl<'a> Record<'a> {
    /// 彩色分支上的一页：只缩放、不量化，因此没有判定位深可写（ADR 0005 决定第 4 条）。
    /// 幂等那四项与来路一项不少——它同样要能被跳过，也同样可能是切出来的一半。
    ///
    /// 这一支不经 [`Recorder`]：彩页在第一遍就编好写出（ADR 0010），那时定档页还没定下来，
    /// 而它本来也用不上——`volume-p95, driven by page …` 是灰度那一侧的理由。
    ///
    /// `salvage` 是这一页救回了多少，完好页是 `None`（04 号票，见 [`salvaged_text`]）。
    /// `page_source` 是这一张自己那一份源哈希，写不写由调用方按 [`PageSource`] 的《哪些页有》定。
    pub fn color(
        fingerprint: &'a Fingerprint,
        origin: &Origin,
        page_source: Option<&PageSource>,
        salvage: Option<Salvage>,
    ) -> Self {
        Self {
            fingerprint,
            origin: origin.text(),
            page_source: page_source.map(|source| source.text().to_owned()),
            verdict: COLOR_VERDICT.to_owned(),
            reason: salvaged_text(salvage, COLOR_REASON.to_owned()),
        }
    }

    /// 全部字段，按写进文件的顺序：七项，页级源哈希在场时是八项，紧跟在卷级那一份之后。
    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        let mut fields = vec![
            (TOOL_KEYWORD, self.fingerprint.tool.as_str()),
            (PROFILE_KEYWORD, &self.fingerprint.profile),
            (PARAMS_KEYWORD, &self.fingerprint.params),
            (SOURCE_KEYWORD, &self.fingerprint.source),
        ];
        if let Some(page_source) = &self.page_source {
            fields.push((PAGE_SOURCE_KEYWORD, page_source));
        }
        fields.extend([
            (ORIGIN_KEYWORD, self.origin.as_str()),
            (VERDICT_KEYWORD, &self.verdict),
            (REASON_KEYWORD, &self.reason),
        ]);
        fields
    }
}

/// 给灰度路径逐页盖记录的那一套：全卷共用的指纹，加上定档页序号。
///
/// 两者绑成一个类型，因为盖记录处处要它们成对：指纹填前四项，定档页把上包络那句
/// `volume-p95, driven by page 087` 写全，缺一项都盖不出一份完整的记录。
///
/// `driver` 指进 [`crate::VolumeReport::pages`]。上包络不在场（没开 `--envelope`、
/// 覆盖项顶掉判定）时没有定档页可指，那时是 `None`。
pub struct Recorder<'a> {
    fingerprint: &'a Fingerprint,
    driver: Option<usize>,
}

impl<'a> Recorder<'a> {
    pub fn new(fingerprint: &'a Fingerprint, driver: Option<usize>) -> Self {
        Self {
            fingerprint,
            driver,
        }
    }

    /// 灰度路径上的一页：判定与理由都有。
    ///
    /// `salvage` 是这一页救回了多少，完好页是 `None`（04 号票，见 [`salvaged_text`]）。
    /// `page_source` 是这一张自己那一份源哈希——默认路径与顶死那一趟第一遍盖记录时有，
    /// 上包络那条路第二遍盖记录时没有（见 [`PageSource`] 的《哪些页有》）。
    pub fn gray(
        &self,
        origin: &Origin,
        page_source: Option<&PageSource>,
        verdict: Verdict,
        salvage: Option<Salvage>,
    ) -> Record<'a> {
        Record {
            fingerprint: self.fingerprint,
            origin: origin.text(),
            page_source: page_source.map(|source| source.text().to_owned()),
            verdict: verdict.candidate.to_string(),
            reason: salvaged_text(salvage, reason_text(verdict.reason, self.driver)),
        }
    }

    /// 失败页留下的那张占位页：没有判定，只说明自己是什么（12 号票）。
    ///
    /// 这一句是占位页**随身带着**的那处标记，而它得随身带着：白页一旦离开报告的上下文
    /// ——被拷进阅读器、从隔离目录里单拎出来——就再没有别的地方说得出它是个占位页，
    /// 而 12 号票要的正是「问题不会藏起来」。
    ///
    /// 幂等那四项与来路照填，但幂等在这里买不到什么：隔离的卷每一趟都重做
    /// （见 `crate::process_volume`）。填它们只是因为记录本身是七项一套的。
    ///
    /// 失败页那一族恒只有一张（`crate::OUTPUTS_PER_FAILED_PAGE`）：它没有像素可切。
    ///
    /// 页级源哈希**不写**：占位页的尺寸是卷内统一尺寸，由全卷定，这一页的字节因此不只
    /// 取决于它自己——与上包络那条路同一条理由（见 [`PageSource`] 的《哪些页有》）。
    pub fn failed(&self, origin: &Origin) -> Record<'a> {
        Record {
            fingerprint: self.fingerprint,
            origin: origin.text(),
            page_source: None,
            verdict: FAILED_VERDICT.to_owned(),
            reason: FAILED_REASON.to_owned(),
        }
    }
}

/// 部分救回页的理由前面添一句：这一页只救回了这么多（04 号票）。
///
/// 与占位页那句自证同一个道理（见 [`Recorder::failed`]）：一张只救回了一半的页
/// 一旦离开报告的上下文——被拷进阅读器、被单拎出来——就再没有别的地方说得出它不全，
/// 而它看上去与一张下半截是留白的正常页毫无分别。
///
/// 添在理由上而不另占一个 tEXt 关键字：救回了多少与「这一档是怎么来的」是同一句话的两半，
/// 而拆成两个字段会让只读其中一个的工具看漏。它也**不进** [`Fingerprint`]——
/// 那四项管的是「要不要重做」，而重做与否只看源字节，源字节没变，救回的比例也不会变。
///
/// 取值仍是 ASCII：百分数只用得上数字与小数点。
fn salvaged_text(salvage: Option<Salvage>, reason: String) -> String {
    match salvage {
        Some(salvage) => format!(
            "salvaged {:.1}% recovered, {reason}",
            salvage.share() * 100.0
        ),
        None => reason,
    }
}

/// 判定理由的英文说法。
///
/// 与报告那一侧的中文说法（`Reason` 的 `Display`）是两份，不是重复：tEXt 只装得下 Latin-1，
/// 而这一份还要在几个月后被别的工具读出来，写法因此得钉死。
/// 上包络那一条照 ADR 0006 的原话写：`volume-p95, driven by page 087`。
fn reason_text(reason: Reason, driver: Option<usize>) -> String {
    match reason {
        Reason::LowestWithinThreshold => "lowest candidate within threshold".to_owned(),
        Reason::NoneWithinThreshold => "none within threshold, top candidate".to_owned(),
        Reason::Override => "override".to_owned(),
        Reason::VolumeEnvelope => match driver {
            Some(page) => format!("volume-p95, driven by page {:03}", page + 1),
            None => "volume-p95".to_owned(),
        },
        Reason::Hysteresis => "hysteresis raise".to_owned(),
        Reason::Outlier => "outlier, decided on its own".to_owned(),
        Reason::OutsideTheGate => "outside the geometry gate, dither off".to_owned(),
    }
}

/// 参数哈希：这一次调用里**会改变输出**的每一项。
///
/// 收进来的是面板四项、阈值、适配方式、裁边、拆分那三项、残差段滤波器与三个覆盖项，
/// 连同**判据那三个结构体的全部字段**（[`Composition`]、[`Aggregation`]、[`Masking`]）。
/// 判据出的是量、阈值划的是界，收一半漏一半，改了判据参数的那一趟会被幂等静默跳过
/// （ADR 0002 的《后果》）。
///
/// 型号名不收：设备只是面板的别名，多对一（`CONTEXT.md`），同一块面板的两个别名输出
/// 逐字节相同。它另有去处——[`Fingerprint`] 单独记着它，也单独比它，理由见那里。
/// 其余不收的几项各有理由：`--cache-budget` 限的是峰值内存、不动写出的像素（ADR 0005），
/// 收了它，改一次预算就要整库重做；`--dry-run` 不写输出，没有输出可作废；
/// 输入路径与输出根目录同理——收了它们，输出一搬家记录就全部失效，
/// 而「记录随文件走」要的正是反面（ADR 0009）。
///
/// 喂进哈希的是一段按名写死的文本，不是 `Debug`。这串字节要落进文件、几个月后还要比对，
/// 而 `Debug` 的写法没有任何稳定承诺：它一变，全库的输出会静默地一起过期。
/// 判据那三件同样按名写死——`Debug` 能自动跟上新字段，代价是把全库的过期时机交给一个
/// 没有承诺的写法。**穷尽解构**顶上那一格：加了字段这里当场编译不过，而写法仍由本函数说了算。
fn params_hash(request: &Request) -> String {
    hash(params_text(request, composition(), aggregation(), masking()).as_bytes())
}

/// 喂进哈希的那段文本。
///
/// 判据那三件东西**由调用方传进来**，本函数不去取：生产路径上只有 [`params_hash`]
/// 一个调用点，取的一律是本次判据在用的那一套；而用例换得动它们，
/// 「判据参数一改哈希就变」这条性质因此断言得出来——判据参数是编译期常数，
/// 不这样它就只能靠人眼看。
fn params_text(
    request: &Request,
    composition: Composition,
    aggregation: Aggregation,
    masking: Masking,
) -> String {
    let panel = request.profile.panel();
    let mut text = String::new();
    let mut line = |name: &str, value: &dyn std::fmt::Display| {
        writeln!(text, "{name} {value}").expect("写进 String 不会失败");
    };
    line(
        "panel",
        &format!(
            "{}x{} {}ppi {}levels {}",
            panel.resolution.width,
            panel.resolution.height,
            panel.ppi,
            panel.gray_levels,
            if panel.color { "color" } else { "mono" }
        ),
    );
    line(
        "threshold",
        &format!("{:.3}", request.profile.threshold().value()),
    );
    // 判据那三个结构体的全部字段：为什么收它们、为什么按名写死而不用 `Debug`，
    // 见 `params_hash` 的文档。三处的解构一律穷尽，那是「不必记得回来改这里」的落点。
    //
    // 取值按 `Display` 原样写，**不截精度**：浮点的 `Display` 给的是能还原原值的最短写法，
    // 而上一行 `threshold` 用的 `{:.3}` 在这里是陷阱——小数点后第四位的一次改动会被它整个藏起来。
    let Composition { grain_ratio } = composition;
    line("metric-composition", &format!("grain-ratio {grain_ratio}"));
    let Aggregation {
        tile,
        quantile,
        tail_tiles,
    } = aggregation;
    line(
        "metric-aggregation",
        &format!("tile {tile} quantile {quantile} tail-tiles {tail_tiles}"),
    );
    let Masking { floor, knee } = masking;
    line("metric-masking", &format!("floor {floor} knee {knee}"));
    // 适配方式改的是目标尺寸本身（页几何批 01 号票）：换了它，这一卷每一页的尺寸、
    // 几何门、判据参照与判定都要重算，上一趟的输出一张都不能留。
    line("fit", &request.fit.name());
    // 裁边改的是**适配之前**的页尺寸（页几何批 02 号票）：换了它，目标尺寸、几何门、
    // 判据参照与判定整卷重算，上一趟的输出一张都不能留。
    line("crop", &request.crop);
    // 拆分那三项改的是**这一卷有几页、每一页是哪一块**（页几何批 04 号票）：
    // 换了其中任何一项，成员名与页尺寸都要重算。阅读方向只换两半的先后，
    // 但那正是成员名的次序——`001-1.png` 从右半变成左半，字节整个换了一张。
    line("split", &request.split.on);
    line(
        "split-threshold",
        &format!("{:.3}", request.split.threshold.value()),
    );
    line("reading-order", &request.split.order.name());
    line("filter", &request.filter.name());
    // 纸白对齐的上限改的是缩放之后那一步的像素（纸白对齐批 01 号票）：参照与其后一切量化
    // 跟着变，上一趟的输出一张都不能留。**取值 0 也照样写进来**——从「关」改到「开」
    // 与从 4 改到 2 是同一类改动，而 ADR 0002 的《后果》里记过一次漏收的同型事故
    // （「地板不在里面……旧输出会被静默跳过」）。
    line("white-align-limit", &request.white_align_limit);
    line(
        "bit-depth",
        &request
            .bit_depth
            .map_or_else(|| "auto".to_owned(), |depth| depth.to_string()),
    );
    line("dither", &request.dither.map_or("auto", Dither::name));
    // 走的是哪条路——上包络开着还是逐页各判各的——改的是每一页的档（ADR 0018）：
    // 翻默认那一趟从没点过这个开关的用户全部不命中，ADR 0018 的《后果》认下了它。
    line("envelope", &request.envelope);
    text
}

/// 一段字节的哈希，截到 [`HASH_HEX`] 个十六进制字符。
fn hash(bytes: &[u8]) -> String {
    hex(blake3::hash(bytes))
}

fn hex(digest: blake3::Hash) -> String {
    let mut text = digest.to_hex().to_string();
    text.truncate(HASH_HEX);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheBudget;
    use crate::geometry::FitMode;
    use crate::medium::IoMode;
    use crate::profile::Profile;
    use crate::quantize::{BitDepth, Candidate};
    use crate::request::Mode;
    use crate::resample::Filter;
    use crate::spread::SplitRule;
    use crate::white::WhiteAlignLimit;
    use std::path::PathBuf;

    /// 一处参数改动，连同它在断言里的说法。
    type Change = (&'static str, fn(&mut Request));

    /// 一份默认参数的请求。各用例只改自己那一项。
    fn request() -> Request {
        Request {
            inputs: vec![PathBuf::from("library/volume-a")],
            output_root: PathBuf::from("out"),
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            filter: Filter::default(),
            white_align_limit: WhiteAlignLimit::default(),
            bit_depth: None,
            dither: None,
            envelope: false,
            cache_budget: CacheBudget::default(),
            mode: Mode::Process,
            io_mode: IoMode::default(),
            progress: None,
            metadata: true,
        }
    }

    /// 会改变输出的每一项都得改变参数哈希：漏掉一项，改了它的那一趟会被静默跳过，
    /// 用户拿到的是上一套参数的输出。
    #[test]
    fn every_parameter_that_changes_the_output_changes_the_hash() {
        let baseline = params_hash(&request());
        let changes: [Change; 10] = [
            ("面板", |request| {
                request.profile = Profile::resolve("kobo-clara-hd").expect("内置型号")
            }),
            // 界挪一格，逐页判定就可能落到另一档上。它与面板不是同一项：
            // 内置表里每一块面板共用同一个界，换面板那一行因此一次都没动过它。
            ("阈值", |request| {
                let doubled = request.profile.threshold().value() * 2.0;
                request.profile = request
                    .profile
                    .clone()
                    .with_threshold(doubled)
                    .expect("两倍仍在 0 与 255 之间")
            }),
            ("面板灰阶数", |request| {
                request.profile = request
                    .profile
                    .clone()
                    .with_gray_levels(4)
                    .expect("4 级灰阶")
            }),
            // 适配方式换掉，目标尺寸整卷重算（页几何批 01 号票）。
            ("适配方式", |request| request.fit = FitMode::Inside),
            // 裁边换掉，适配之前的页尺寸就变了（页几何批 02 号票）。
            ("裁边", |request| request.crop = false),
            ("滤波器", |request| request.filter = Filter::Bicubic),
            // 从**默认的 4**（开）改到 0（关）：默认值抬上去之后（05 号票），
            // 那才是这一项最要紧的一次改动——不放心的人关掉它，上一趟的输出必须整卷过期，
            // 而 0 恰好是「取值 0 也照样进哈希」不成立时唯一漏得掉的那一个。
            ("纸白对齐上限", |request| {
                request.white_align_limit = WhiteAlignLimit::OFF
            }),
            ("位深覆盖", |request| {
                request.bit_depth = Some(BitDepth::Four)
            }),
            ("抖动覆盖", |request| request.dither = Some(Dither::Off)),
            ("上包络", |request| request.envelope = true),
        ];

        for (what, change) in changes {
            let mut changed = request();
            change(&mut changed);
            assert_ne!(params_hash(&changed), baseline, "改了{what}，哈希没变");
        }
    }

    /// 判据那三个结构体的**每一个字段**都得改变参数哈希：漏掉一个，改了它的那一趟
    /// 会被幂等静默跳过，用户拿到的是上一套判据算出来的判定（ADR 0002 的《后果》）。
    ///
    /// 改动一律**相对当前取值**（翻倍、加一），一个当前的数都不写死：标定把哪一个换掉，
    /// 这一条都不必跟着改（与 `Aggregation` 的 K 同一条规矩）。
    #[test]
    fn every_metric_parameter_changes_the_hash() {
        type MetricChange = (
            &'static str,
            fn(&mut Composition, &mut Aggregation, &mut Masking),
        );
        let hash_of = |composition, aggregation, masking| {
            hash(params_text(&request(), composition, aggregation, masking).as_bytes())
        };
        let baseline = hash_of(composition(), aggregation(), masking());
        // 这一串**就是**生产路径写进记录的那一串：`params_hash` 喂给哈希的是本次判据在用的
        // 那三件，不是另抄的一份。穷尽解构管得住「将来加了一个字段」，管不住
        // 「有人在调用点传了别的一份进来」——那道缝由这一句钉着（同 Q306 那一类）。
        assert_eq!(
            params_hash(&request()),
            baseline,
            "生产路径喂给哈希的不是本次判据在用的那三件"
        );
        let changes: [MetricChange; 6] = [
            ("颗粒可见度地板", |composition, _, _| {
                composition.grain_ratio *= 2.0
            }),
            ("分块边长", |_, aggregation, _| aggregation.tile += 1),
            ("上分位", |_, aggregation, _| aggregation.quantile *= 0.9),
            ("尾巴块数 K", |_, aggregation, _| {
                aggregation.tail_tiles += 1
            }),
            ("掩蔽地板", |_, _, masking| masking.floor *= 0.5),
            ("掩蔽拐点", |_, _, masking| masking.knee += 1.0),
        ];

        for (what, change) in changes {
            let (mut composition, mut aggregation, mut masking) =
                (composition(), aggregation(), masking());
            change(&mut composition, &mut aggregation, &mut masking);
            assert_ne!(
                hash_of(composition, aggregation, masking),
                baseline,
                "改了{what}，哈希没变"
            );
        }
    }

    /// 不改变输出的那几项不该改变参数哈希：收了它们，改一次预算就要整库重做，
    /// 输出一搬家记录就全部失效。
    #[test]
    fn what_does_not_change_the_output_does_not_change_the_hash() {
        let baseline = params_hash(&request());
        let changes: [Change; 5] = [
            ("缓存预算", |request| {
                request.cache_budget = CacheBudget::new(4096)
            }),
            ("模式", |request| request.mode = Mode::DryRun),
            // 读取策略只改这一趟怎么把字节取进来，一个像素都不动（13 号票）。
            // 收了它，同一批卷换台机器跑就要整库重做——而输出逐字节相同。
            ("读取策略", |request| request.io_mode = IoMode::Serial),
            ("输入路径", |request| {
                request.inputs = vec![PathBuf::from("elsewhere/volume-a")]
            }),
            ("输出根目录", |request| {
                request.output_root = PathBuf::from("elsewhere")
            }),
        ];

        for (what, change) in changes {
            let mut changed = request();
            change(&mut changed);
            assert_eq!(params_hash(&changed), baseline, "改了{what}，哈希跟着变了");
        }
    }

    /// 同一块面板的两个别名：参数哈希一模一样——输出逐字节相同，它收的是像素那一侧。
    /// 但指纹不同：记录要说得出这批输出该拿去哪台设备看，型号名说错了就是过期的记录。
    ///
    /// 两件事分在两个字段上，各自的作用因此都说得清；合成一个，就得在「别名换了要不要重做」
    /// 上二选一，而两边都有道理。
    #[test]
    fn two_aliases_of_one_panel_share_a_params_hash_but_not_a_fingerprint() {
        let alias_of = |device: &str| {
            let mut request = request();
            request.profile = Profile::resolve(device).expect("内置型号");
            request
        };
        let libra_2 = alias_of("kobo-libra-2");
        let libra_h2o = alias_of("kobo-libra-h2o");
        assert_eq!(
            libra_2.profile.panel(),
            libra_h2o.profile.panel(),
            "夹具选错了：这两个型号该指向同一块面板"
        );

        assert_eq!(params_hash(&libra_2), params_hash(&libra_h2o));
        let source = SourceHasher::new().finish();
        assert_ne!(
            Fingerprint::new(&libra_2, source.clone()),
            Fingerprint::new(&libra_h2o, source)
        );
    }

    /// 卷级源哈希看得见成员的名字，不只是字节：两页对调名字，输出整个错位，哈希必须变。
    #[test]
    fn the_source_hash_covers_the_member_names_too() {
        let hash_of = |members: [(&str, &[u8]); 2]| {
            let mut hasher = SourceHasher::new();
            for (name, bytes) in members {
                hasher.member(Path::new(name), bytes);
            }
            hasher.finish()
        };

        let original = hash_of([("001.png", b"one"), ("002.png", b"two")]);

        assert_ne!(
            hash_of([("002.png", b"one"), ("001.png", b"two")]),
            original,
            "两页对调了名字，哈希却没变"
        );
        assert_ne!(
            hash_of([("001.png", b"one"), ("002.png", b"three")]),
            original,
            "一页的字节变了，哈希却没变"
        );
    }

    /// 字段一律 ASCII：tEXt 只装得下 Latin-1，中文写进去会被编码器当场拒绝。
    #[test]
    fn every_field_is_writable_as_latin1() {
        let fingerprint = Fingerprint::new(&request(), SourceHasher::new().finish());
        let verdict = Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
            reason: Reason::VolumeEnvelope,
        };
        // 成员名取一个**带中文的**：那是这批素材的常态，而 tEXt 只装得下 Latin-1。
        let origin = Origin::new(Path::new("第 1 话/001.jpg"), 0, 2);
        let page_source = PageSource::of(Path::new("第 1 话/001.jpg"), b"jpeg bytes");
        let records = [
            Recorder::new(&fingerprint, Some(86)).gray(&origin, None, verdict, None),
            Record::color(&fingerprint, &origin, None, None),
            Recorder::new(&fingerprint, Some(86)).gray(&origin, None, verdict, Some(half())),
            Record::color(&fingerprint, &origin, None, Some(half())),
            Recorder::new(&fingerprint, Some(86)).failed(&origin),
            // 页级那一项在场的两种：字段多一项，同样得是 ASCII。
            Recorder::new(&fingerprint, None).gray(&origin, Some(&page_source), verdict, None),
            Record::color(&fingerprint, &origin, Some(&page_source), None),
        ];

        for record in &records {
            for (keyword, value) in record.fields() {
                assert!(keyword.is_ascii() && (1..=79).contains(&keyword.len()));
                assert!(value.is_ascii(), "{keyword} 的取值不是 ASCII：{value}");
            }
        }
        // ADR 0006 要的那一句，页号从 1 数起、补到三位。
        assert_eq!(
            records[0].reason, "volume-p95, driven by page 087",
            "定档页那一句与 ADR 0006 对不上"
        );
    }

    /// **旧输出里 `hysteresis pull-back` 那一句仍认得**（two-pass-rework/11 的验收）。
    ///
    /// 段式迟滞随 ADR 0018 退场，这一句不再产出；但它写在一批真实输出的 tEXt 里，
    /// 读回来时要当作一份**正常的记录**——判为不命中（参数哈希变了，走的路不同），
    /// 不判为错、不当成别的工具写的。理由那一项本来就不进比对（[`PageRecord::read`]
    /// 只读幂等那四项与来路），这一条把「不读」钉成一句断言：换了理由的写法，
    /// 旧记录照样读得回、照样按指纹比。
    #[test]
    fn a_record_carrying_the_retired_pull_back_reason_still_reads_back_as_a_miss() {
        let today = Fingerprint::new(&request(), SourceHasher::new().finish());
        let yesterday = Fingerprint {
            params: "0123456789abcdef0123456789abcdef".to_owned(),
            ..today.clone()
        };
        let origin = Origin::new(Path::new("001.jpg"), 0, 1);
        let old_record = Record {
            fingerprint: &yesterday,
            origin: origin.text(),
            page_source: None,
            verdict: "2bit".to_owned(),
            reason: "hysteresis pull-back".to_owned(),
        };
        let read = PageRecord::read(std::io::Cursor::new(one_pixel_png(&old_record)))
            .expect("旧记录该读得回来");

        assert_eq!(
            read.fingerprint, yesterday,
            "读回来的指纹不是写进去的那一份"
        );
        assert!(
            read.matches(&yesterday, Path::new("001.jpg"), 0, 1),
            "同一份指纹该命中——理由那一句不进比对"
        );
        assert!(
            !read.matches(&today, Path::new("001.jpg"), 0, 1),
            "参数哈希变了却命中了：翻默认那一趟从没点过开关的用户本该全部重做"
        );
    }

    /// 一张只有一个像素、盖着 `record` 的 PNG：读回记录的用例要的只是 tEXt，像素越少越好。
    fn one_pixel_png(record: &Record) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(png::ColorType::Grayscale);
        for (keyword, value) in record.fields() {
            encoder
                .add_text_chunk(keyword.to_owned(), value.to_owned())
                .expect("写得进 tEXt");
        }
        let mut writer = encoder.write_header().expect("写 PNG 头");
        writer.write_image_data(&[0]).expect("写像素");
        writer.finish().expect("收尾");
        bytes
    }

    /// **页级源哈希写下去、读得回来；旧记录读回来是「没有」，不是「坏了」**（two-pass-rework/13）。
    ///
    /// 两份记录只差那一项：新的带着它，读回来逐字相同；老的没有，读回来 `page_source` 是 `None`，
    /// 而卷级那四项与来路照旧读得回、照旧命中——幂等的判据一个字没变。
    /// 写法不对的取值（不是 32 个十六进制字符）同样读成 `None`：那不是本工具写的记录。
    #[test]
    fn the_page_level_basis_reads_back_and_an_old_record_reads_back_without_it() {
        let fingerprint = Fingerprint::new(&request(), SourceHasher::new().finish());
        let origin = Origin::new(Path::new("001.jpg"), 1, 2);
        let page_source = PageSource::of(Path::new("001.jpg"), b"jpeg bytes");

        let today = Record::color(&fingerprint, &origin, Some(&page_source), None);
        let read = PageRecord::read(std::io::Cursor::new(one_pixel_png(&today)))
            .expect("新记录该读得回来");
        assert_eq!(read.page_source, Some(page_source.clone()));
        assert!(read.matches(&fingerprint, Path::new("001.jpg"), 1, 2));

        let yesterday = Record::color(&fingerprint, &origin, None, None);
        let read = PageRecord::read(std::io::Cursor::new(one_pixel_png(&yesterday)))
            .expect("旧记录该读得回来");
        assert_eq!(read.page_source, None, "旧记录读出了一个不存在的页级源哈希");
        assert!(
            read.matches(&fingerprint, Path::new("001.jpg"), 1, 2),
            "旧记录在卷级那四项上本该照旧命中"
        );

        let malformed = Record {
            page_source: Some("not a hash".to_owned()),
            ..Record::color(&fingerprint, &origin, None, None)
        };
        let read = PageRecord::read(std::io::Cursor::new(one_pixel_png(&malformed)))
            .expect("其余几项齐着，记录该读得回来");
        assert_eq!(read.page_source, None, "写法不对的取值被当成了页级源哈希");
    }

    /// **改写卷级源哈希那一项，其余字节一个不动**（two-pass-rework/14：留下的页）。
    ///
    /// 改过的页与一开始就按新指纹写出的那一页**逐字节相同**——那正是留下的页要买的东西：
    /// 产物与整卷重做的一样，不是「像素一样、记录旧着」。页级那一问改前改后都命中
    /// （它不看卷级那一项），卷级那一问改后按新指纹命中、按旧指纹不再命中。
    /// 没有那一项的页（`--no-metadata` 写的）改不了，报错——那样的页留不下来。
    #[test]
    fn restamping_the_volume_source_rewrites_that_one_field_and_nothing_else() {
        let yesterday = Fingerprint::new(&request(), "0".repeat(HASH_HEX));
        let today = Fingerprint::new(&request(), "f".repeat(HASH_HEX));
        let origin = Origin::new(Path::new("001.jpg"), 0, 1);
        let page_source = PageSource::of(Path::new("001.jpg"), b"jpeg bytes");
        let written = |fingerprint: &Fingerprint| {
            one_pixel_png(&Record::color(
                fingerprint,
                &origin,
                Some(&page_source),
                None,
            ))
        };

        let mut page = written(&yesterday);
        restamp_source(&mut page, &today).expect("改得了");
        assert_eq!(
            page,
            written(&today),
            "改过的页与按新指纹写的那一页不是同一份字节"
        );

        let read = PageRecord::read(std::io::Cursor::new(page)).expect("改过的页该读得回来");
        assert!(read.matches(&today, Path::new("001.jpg"), 0, 1));
        assert!(!read.matches(&yesterday, Path::new("001.jpg"), 0, 1));
        assert!(read.matches_by_page(&today, &page_source, Path::new("001.jpg"), 0, 1));

        let mut bare = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bare, 1, 1);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_color(png::ColorType::Grayscale);
            let mut writer = encoder.write_header().expect("写 PNG 头");
            writer.write_image_data(&[0]).expect("写像素");
            writer.finish().expect("收尾");
        }
        assert!(
            restamp_source(&mut bare, &today).is_err(),
            "没有那一项的页也被「改」成功了"
        );
    }

    /// **页级那一问与卷级那一问只差源那一项**（two-pass-rework/14）：卷里别的页改了，卷级源哈希变，
    /// 卷级不命中，页级照旧命中；这一页自己改了，页级源哈希变，页级不命中。
    /// 工具版本、profile、参数三项任何一项变了，两问一起不命中。旧记录（没有页级那一项）页级恒不命中。
    #[test]
    fn the_page_level_question_differs_from_the_volume_level_one_only_in_the_source() {
        let then = Fingerprint::new(&request(), "0".repeat(HASH_HEX));
        let origin = Origin::new(Path::new("001.jpg"), 0, 1);
        let page_source = PageSource::of(Path::new("001.jpg"), b"jpeg bytes");
        let read = PageRecord::read(std::io::Cursor::new(one_pixel_png(&Record::color(
            &then,
            &origin,
            Some(&page_source),
            None,
        ))))
        .expect("记录该读得回来");
        let page = Path::new("001.jpg");

        // 卷里别的页改了：卷级变、页级不变。
        let volume_changed = Fingerprint::new(&request(), "1".repeat(HASH_HEX));
        assert!(!read.matches(&volume_changed, page, 0, 1));
        assert!(read.matches_by_page(&volume_changed, &page_source, page, 0, 1));
        // 这一页自己改了：页级不命中。
        let edited = PageSource::of(page, b"other bytes");
        assert!(!read.matches_by_page(&volume_changed, &edited, page, 0, 1));
        // 参数变了：两问一起不命中。
        let mut other = request();
        other.crop = false;
        let reparameterised = Fingerprint::new(&other, "0".repeat(HASH_HEX));
        assert!(!read.matches(&reparameterised, page, 0, 1));
        assert!(!read.matches_by_page(&reparameterised, &page_source, page, 0, 1));
        // 来路对不上：两问一起不命中。
        assert!(!read.matches_by_page(&then, &page_source, page, 0, 2));
        // 旧记录：页级恒不命中，卷级照旧。
        let old = PageRecord::read(std::io::Cursor::new(one_pixel_png(&Record::color(
            &then, &origin, None, None,
        ))))
        .expect("旧记录该读得回来");
        assert!(old.matches(&then, page, 0, 1));
        assert!(!old.matches_by_page(&then, &page_source, page, 0, 1));
    }

    /// 页级源哈希的**写法钉死**（two-pass-rework/13）：它要落进真实输出，按页跳过那一票
    /// 落地之后改一次定义就是全库页级不命中一趟（停车场 Q667）。字面量是本票落地那一刻
    /// 算出来的数——规矩是「只喂这一个成员的 [`SourceHasher`]」，名字与字节都算，
    /// 两页对调名字，输出整个错位，页级也得看得见。
    #[test]
    fn the_page_source_has_a_frozen_definition() {
        let page = PageSource::of(Path::new("ch1/001.jpg"), b"one");

        assert_eq!(
            page.text(),
            "20ec99fd09b7c84efdf8e48d87fd3e8b",
            "页级源哈希的定义变了"
        );
        assert_ne!(
            PageSource::of(Path::new("ch1/002.jpg"), b"one"),
            page,
            "换了名字，页级源哈希却没变"
        );
        assert_ne!(
            PageSource::of(Path::new("ch1/001.jpg"), b"two"),
            page,
            "换了字节，页级源哈希却没变"
        );
        // 一族两张共用同一份：它算的是源成员，不是切出来的哪一半。
        assert_eq!(PageSource::of(Path::new("ch1/001.jpg"), b"one"), page);
    }

    /// 救回了多少这一半的页数，`Salvage` 造不出来——它只在 `decode` 里量得出。
    fn half() -> Salvage {
        Salvage::from_share(0.5)
    }

    /// 部分救回页的记录**自己说得出它不全**（04 号票）。
    ///
    /// 两条分支各钉一次：灰度路径与彩色分支写记录的是两段代码，只测一条，
    /// 另一条上的页离开报告之后就再没有地方说得出它救回了多少。
    #[test]
    fn a_salvaged_page_says_so_in_its_own_record() {
        let fingerprint = Fingerprint::new(&request(), SourceHasher::new().finish());
        let verdict = Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
            reason: Reason::VolumeEnvelope,
        };

        let origin = Origin::new(Path::new("001.jpg"), 0, 1);
        assert_eq!(
            Recorder::new(&fingerprint, Some(86))
                .gray(&origin, None, verdict, Some(half()))
                .reason,
            "salvaged 50.0% recovered, volume-p95, driven by page 087"
        );
        assert_eq!(
            Record::color(&fingerprint, &origin, None, Some(half())).reason,
            "salvaged 50.0% recovered, color branch, scaled only"
        );
        // 完好页那一句一个字都不动：添的这一句只属于救回来的页。
        assert_eq!(
            Record::color(&fingerprint, &origin, None, None).reason,
            COLOR_REASON,
            "完好页的理由被救回那一句污染了"
        );
    }

    /// 六条理由的英文说法各钉一次。
    ///
    /// 这一份要落进文件、几个月后由别的工具读出来，写法因此得钉死（见 [`reason_text`]）。
    /// 「钉死」从前只是一句声称：全仓只有上包络那一条被断言过，另外五条改成任何字面量，
    /// 全套测试一条都不会红——黄金回归按 `--no-metadata` 跑，根本不看 tEXt。
    ///
    /// 特例页那一条尤其要钉：它现在真的随普通卷落盘（黄金夹具里 `mixed` 与 `archive.cbz`
    /// 各出特例页），而 ADR 0006 认下「可指认」时，报告那一侧有快照钉着，
    /// 随文件走的这一侧此前没有。
    #[test]
    fn every_reason_has_a_frozen_english_wording() {
        for (reason, driver, text) in [
            (
                Reason::LowestWithinThreshold,
                None,
                "lowest candidate within threshold",
            ),
            (
                Reason::NoneWithinThreshold,
                None,
                "none within threshold, top candidate",
            ),
            (Reason::Override, None, "override"),
            (Reason::VolumeEnvelope, None, "volume-p95"),
            (
                Reason::VolumeEnvelope,
                Some(86),
                "volume-p95, driven by page 087",
            ),
            (Reason::Hysteresis, None, "hysteresis raise"),
            (Reason::Outlier, None, "outlier, decided on its own"),
        ] {
            assert_eq!(reason_text(reason, driver), text, "{reason:?} 的说法变了");
            // tEXt 只装得下 Latin-1，这一份还得是 ASCII。
            assert!(text.is_ascii(), "{text}");
        }
    }
}
