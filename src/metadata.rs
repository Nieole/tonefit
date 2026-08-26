//! 输出 PNG 的自描述元数据：判定与理由随文件走，同一批字段兼作幂等依据。
//!
//! 记录写进 tEXt（ADR 0006：判定与理由写进输出 PNG 的 tEXt）。**它就在文件里**，
//! 不在任何外部状态库：文件被移动、改名、重新打包都不会丢，而 tonefit 也就不必为了幂等
//! 去维护一份全库索引——那正是 ADR 0009 关掉的东西。
//!
//! 六个字段分两摞：
//!
//! - **幂等依据**四项：工具版本、profile 名、参数哈希、源哈希。重跑时读回来逐项比，
//!   四项都对得上就不必重做（见 [`Fingerprint`]）。
//! - **判定记录**两项：判定与理由。两者由前四项推出来，因此不进比对；它们在这里，
//!   是为了让「这一页为什么是这一档」随文件走（spec 的 story 7）。
//!
//! tEXt 的取值是 Latin-1，中文写不进去：本模块产出的字符串一律 ASCII。
//! 报告那一侧的中文说法（见 `crate::report` 与 `main`）不替代它，它也不替代报告——
//! 一份给人当场读，一份给几个月后打开这个文件的人读。

use std::fmt::Write as _;
use std::io::{BufRead, Seek};
use std::path::Path;

use crate::decide::{Reason, Verdict};
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

    /// 从一页输出 PNG 里读回依据。四项缺一项就是 `None`——没有记录的输出
    /// （`--no-metadata` 写出的，或别的工具写的）不构成幂等的依据。
    ///
    /// 只读到第一个 IDAT 为止，一个像素都不解：ADR 0006 认下的「读回 tEXt 比对」就是这一步，
    /// 它的成本要停在这里，否则跳过一卷比重做一卷还贵。
    pub fn read(source: impl BufRead + Seek) -> Option<Self> {
        let reader = png::Decoder::new(source).read_info().ok()?;
        let text = &reader.info().uncompressed_latin1_text;
        let field = |keyword: &str| {
            text.iter()
                .find(|chunk| chunk.keyword == keyword)
                .map(|chunk| chunk.text.clone())
        };
        Some(Self {
            tool: field(TOOL_KEYWORD)?,
            profile: field(PROFILE_KEYWORD)?,
            params: field(PARAMS_KEYWORD)?,
            source: field(SOURCE_KEYWORD)?,
        })
    }
}

/// 记录落在文件开头这么多字节以内。
///
/// 写出来的 PNG 是 IHDR、色板、六个 tEXt、IDAT 这个次序（见 `crate::encode`），
/// 记录因此紧挨着文件头。读的那一端顺着这个事实只取开头一截：一页 PNG 有好几 MB，
/// 为了六行记录把它整个搬进内存不值当。
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
        // 分隔符按 `/` 归一：同一个卷在 Windows 与别处要算出同一个哈希。
        let name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
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

/// 一页要写进 tEXt 的全部字段：幂等那四项由整卷共用，判定与理由逐页各一份。
pub struct Record<'a> {
    fingerprint: &'a Fingerprint,
    verdict: String,
    reason: String,
}

impl<'a> Record<'a> {
    /// 彩色分支上的一页：只缩放、不量化，因此没有判定位深可写（ADR 0005 决定第 4 条）。
    /// 幂等那四项一项不少——它同样要能被跳过。
    ///
    /// 这一支不经 [`Recorder`]：彩页在第一遍就编好写出（ADR 0010），那时驱动页还没定下来，
    /// 而它本来也用不上——`volume-p95, driven by page …` 是灰度那一侧的理由。
    pub fn color(fingerprint: &'a Fingerprint) -> Self {
        Self {
            fingerprint,
            verdict: COLOR_VERDICT.to_owned(),
            reason: COLOR_REASON.to_owned(),
        }
    }

    /// 六个字段，按写进文件的顺序。
    pub fn fields(&self) -> [(&'static str, &str); 6] {
        [
            (TOOL_KEYWORD, &self.fingerprint.tool),
            (PROFILE_KEYWORD, &self.fingerprint.profile),
            (PARAMS_KEYWORD, &self.fingerprint.params),
            (SOURCE_KEYWORD, &self.fingerprint.source),
            (VERDICT_KEYWORD, &self.verdict),
            (REASON_KEYWORD, &self.reason),
        ]
    }
}

/// 给灰度路径逐页盖记录的那一套：全卷共用的指纹，加上驱动页序号。
///
/// 两者绑成一个类型，因为盖记录处处要它们成对：指纹填前四项，驱动页把上包络那句
/// `volume-p95, driven by page 087` 写全，缺一项都盖不出一份完整的记录。
///
/// `driver` 指进 [`crate::VolumeReport::pages`]。上包络不在场（`--per-page`、
/// 覆盖项顶掉判定）时没有驱动页可指，那时是 `None`。
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
    pub fn gray(&self, verdict: Verdict) -> Record<'a> {
        Record {
            fingerprint: self.fingerprint,
            verdict: verdict.candidate.to_string(),
            reason: reason_text(verdict.reason, self.driver),
        }
    }

    /// 失败页留下的那张占位页：没有判定，只说明自己是什么（12 号票）。
    ///
    /// 这一句是占位页**随身带着**的那处标记，而它得随身带着：白页一旦离开报告的上下文
    /// ——被拷进阅读器、从隔离目录里单拎出来——就再没有别的地方说得出它是个占位页，
    /// 而 12 号票要的正是「问题不会藏起来」。
    ///
    /// 幂等那四项照填，但幂等在这里买不到什么：隔离的卷每一趟都重做
    /// （见 `crate::process_volume`）。填它只是因为记录本身是六项一套的。
    pub fn failed(&self) -> Record<'a> {
        Record {
            fingerprint: self.fingerprint,
            verdict: FAILED_VERDICT.to_owned(),
            reason: FAILED_REASON.to_owned(),
        }
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
    }
}

/// 参数哈希：这一次调用里**会改变输出**的每一项。
///
/// 收进来的是面板四项、阈值、残差段滤波器与三个覆盖项。
///
/// 型号名不收：设备只是面板的别名，多对一（`CONTEXT.md`），同一块面板的两个别名输出
/// 逐字节相同。它另有去处——[`Fingerprint`] 单独记着它，也单独比它，理由见那里。
/// 其余不收的几项各有理由：`--cache-budget` 限的是峰值内存、不动写出的像素（ADR 0005），
/// 收了它，改一次预算就要整库重做；`--dry-run` 一个文件都不写，没有输出可作废；
/// 输入路径与输出根目录同理——收了它们，输出一搬家记录就全部失效，
/// 而「记录随文件走」要的正是反面（ADR 0009）。
///
/// 喂进哈希的是一段按名写死的文本，不是 `Debug`。这串字节要落进文件、几个月后还要比对，
/// 而 `Debug` 的写法没有任何稳定承诺：它一变，全库的输出会静默地一起过期。
fn params_hash(request: &Request) -> String {
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
    line("filter", &request.filter.name());
    line(
        "bit-depth",
        &request
            .bit_depth
            .map_or_else(|| "auto".to_owned(), |depth| depth.to_string()),
    );
    line("dither", &request.dither.map_or("auto", Dither::name));
    line("per-page", &request.per_page);
    hash(text.as_bytes())
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
    use crate::medium::IoMode;
    use crate::profile::Profile;
    use crate::quantize::{BitDepth, Candidate};
    use crate::request::Mode;
    use crate::resample::Filter;
    use std::path::PathBuf;

    /// 一处参数改动，连同它在断言里的说法。
    type Change = (&'static str, fn(&mut Request));

    /// 一份默认参数的请求。各用例只改自己那一项。
    fn request() -> Request {
        Request {
            inputs: vec![PathBuf::from("library/volume-a")],
            output_root: PathBuf::from("out"),
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            filter: Filter::default(),
            bit_depth: None,
            dither: None,
            per_page: false,
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
        let changes: [Change; 6] = [
            ("面板", |request| {
                request.profile = Profile::resolve("kobo-clara-hd").expect("内置型号")
            }),
            ("面板灰阶数", |request| {
                request.profile = request
                    .profile
                    .clone()
                    .with_gray_levels(4)
                    .expect("4 级灰阶")
            }),
            ("滤波器", |request| request.filter = Filter::Bicubic),
            ("位深覆盖", |request| {
                request.bit_depth = Some(BitDepth::Four)
            }),
            ("抖动覆盖", |request| request.dither = Some(Dither::Off)),
            ("逐页", |request| request.per_page = true),
        ];

        for (what, change) in changes {
            let mut changed = request();
            change(&mut changed);
            assert_ne!(params_hash(&changed), baseline, "改了{what}，哈希没变");
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
        let records = [
            Recorder::new(&fingerprint, Some(86)).gray(verdict),
            Record::color(&fingerprint),
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
            "驱动页那一句与 ADR 0006 对不上"
        );
    }

    /// 六条理由的英文说法各钉一次。
    ///
    /// 这一份要落进文件、几个月后由别的工具读出来，写法因此得钉死（见 [`reason_text`]）。
    /// 「钉死」从前只是一句声称：全仓只有上包络那一条被断言过，另外五条改成任何字面量，
    /// 全套测试一条都不会红——黄金回归按 `--no-metadata` 跑，根本不看 tEXt。
    ///
    /// 离群页那一条尤其要钉：它现在真的随普通卷落盘（黄金夹具里 `mixed` 与 `archive.cbz`
    /// 各出离群页），而 ADR 0006 认下「可指认」时，报告那一侧有快照钉着，
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
