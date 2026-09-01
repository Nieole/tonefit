//! 预设：一份命名的落盘配置，装**设备层**与**口味层**两层（`CONTEXT.md` 的《会话》）。
//!
//! 这个模块只答一个问题：**盘上那份预设写的是什么。**「这一趟到底用哪些值」是另一件事，
//! 在 [`crate::Cli`] 那一侧——那里才有命令行，而命令行与预设撞上时的规矩写在
//! `Cli` 的 `impl` 抬头。
//!
//! # 三件本模块不做的事
//!
//! **不装范围层。** 处理范围与输出根每趟都不同，混进预设会让人套用它时误写到上一次的
//! 输出目录（ADR 0009：处理范围是用户点名的子集）。这不是靠自觉——两层都写着
//! `deny_unknown_fields`，写进来的 `inputs` 或 `out` 当场是一条错误，不是被悄悄忽略的一行。
//!
//! **不读盘，除非被点名。** [`load`] 只在 `--preset` 出现时才被调到。不点名的那一趟
//! 命令行仍是全部输入，同一条命令在两台机器上因此行为相同（spec 的 story 13）。
//! 写盘同理，而且更紧一道：只有**会话里按下存**那一下写得动它（`p1-session/12`），
//! 而它盖不掉同名的那一份——覆盖是另一个动作（见 [`Presets::save`] 与 [`Presets::replace`]）。
//!
//! **不猜。** 读不懂的预设——字段过时、型号已删、取值拼错——当场报错，不静默套默认值
//! （spec 的 story 39）。字段怎么迁移是未决问题（`CONTEXT.md` 的《尚未确立》：预设的字段演进），
//! 在决定之前报错是保守的那一侧。
//!
//! # 盘上长什么样
//!
//! 一份文件装多个命名预设，各装两层：
//!
//! ```toml
//! [preset."漫画".device]
//! profile = "kobo-libra-2"
//! gray-levels = 12
//!
//! [preset."漫画".taste]
//! fit = "inside"
//! cache-budget = "1G"
//! ```
//!
//! 两层在文件里是**两节**，不是一堆平铺的键。那条分界线画在**生命周期**上
//! （`CONTEXT.md` 的《会话》），而分界线只写在文档里、不写进格式的话，
//! 把 `filter` 填进设备层这种事就没有人拦得住。
//!
//! # 取值的写法只有一份
//!
//! 每一项的写法就是命令行上那一项的写法，解析走的也是同一个函数
//! （`FitMode::resolve`、`CacheBudget::parse`、……）。**预设不是第二套语法**：
//! 有了第二套，「效果与把那几个 flag 逐个敲出来完全一致」这句话就只能靠人去核对。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tonefit::{
    BitDepth, CacheBudget, Dither, Filter, FitMode, IoMode, Profile, ReadingOrder, SplitRule,
    SplitThreshold,
};

/// 预设文件在用户配置目录下的位置。
const PRESET_FILE: &str = "tonefit/presets.toml";

/// 一份读进来、逐项验过的预设。
///
/// 每一项都是 `Option`：预设**只说它说到的那几项**，没说到的落到命令行的默认值上。
/// 「没写」与「写成默认值」在这里是同一个结果，两者都不该盖掉命令行上显式点到的那一项。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Preset {
    /// 设备层：绑面板，由标定决定，改一次管很久。
    pub device: DeviceLayer,
    /// 口味层：这一趟的立场。
    pub taste: TasteLayer,
}

/// 设备层：**型号 + 感知可分辨级数 + 阈值**。
///
/// 三项都是「判定的依据」那一类：错了不是这一趟不好看，是判定拿错了尺子。
///
/// **阈值在这一层，不在口味层。** 它跟着面板走、不可跨面板比较（ADR 0002），
/// 数值由真机盲测在一块面板上夹出（`Profile::threshold`）——与感知可分辨级数一样，
/// 是标定的产物、绑着那块面板、改一次管很久。口味层那一层的东西改了只是这一趟的取舍，
/// 而阈值改了，同一批页的判定整个换一套。三层的分界线画在生命周期上，它落在这一侧。
/// 后两项**要连同型号一起写**，理由见 [`no_panel_to_calibrate_against_error`]。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceLayer {
    /// 目标设备型号，已归一成内置表里的规范名。
    pub profile: Option<String>,
    /// 覆盖面板灰阶数（`--gray-levels`）：在真机上数出来的感知可分辨级数。
    pub gray_levels: Option<u32>,
    /// 覆盖判定用的阈值（`--threshold`）。
    pub threshold: Option<f32>,
}

/// 口味层：这一趟的立场，**逐项列举**。
///
/// 十一项：适配方式、裁边、拆分与它的阈值、阅读方向、滤波器、位深、抖动、逐页、
/// 缓存预算、读取策略。前五项是页几何那一批添的，后六项是 spec 的《会话：三层与预设》
/// 原本就列着的那几项。
///
/// **`--dry-run` 与 `--no-metadata` 不在里面**，按它们各自是什么判的：前者是这一趟做到
/// 哪一步（`Mode`），试算与执行是同一条回路的两半，不是一份存得住的立场；后者一开就把
/// 记录与幂等一起关掉，那是对**这一批输出**的处置——存进预设就意味着「这一套参数从此不留
/// 记录」，而那是每一趟各自要拿的主意。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TasteLayer {
    /// 适配方式（`--fit`）。
    pub fit: Option<FitMode>,
    /// 裁不裁边（`--no-crop` 关掉它）。
    pub crop: Option<bool>,
    /// 拆不拆跨页（`--no-split` 关掉它）。
    pub split: Option<bool>,
    /// 跨页候选的阈值（`--split-threshold`）。
    pub split_threshold: Option<SplitThreshold>,
    /// 拆开后两半的先后（`--reading-order`）。
    pub reading_order: Option<ReadingOrder>,
    /// 残差段的重采样滤波器（`--filter`）。
    pub filter: Option<Filter>,
    /// 覆盖自动判定的位深（`--bit-depth`）。
    pub bit_depth: Option<BitDepth>,
    /// 覆盖自动选择的抖动模式（`--dither`）。
    pub dither: Option<Dither>,
    /// 关掉卷级上包络与迟滞（`--per-page`）。
    pub per_page: Option<bool>,
    /// 缓存预算（`--cache-budget`）。
    pub cache_budget: Option<CacheBudget>,
    /// 读取策略（`--io-mode`）。
    pub io_mode: Option<IoMode>,
}

impl TasteLayer {
    /// 这一层每一项**落到默认值之后**的取值。
    ///
    /// 它是「没说」这一格的唯一去处：命令行拿它做「命令行没点、预设也没说」那一档
    /// （见 [`crate::Cli`] 各项那几个方法），会话直接拿它拼 [`Request`](tonefit::Request)
    /// （见 `session::state::Session::request`）。两处因此不会各写一份默认值——
    /// 写第二份，同一条命令与同一次会话就会在无人察觉时分家。
    ///
    /// 各项的默认值**仍在库那一侧**（那几个 `Default`），本方法一个都不复述；
    /// 复述的只有 [`crop`](Self::crop) 那一项，理由写在它自己身上。
    pub fn fit(&self) -> FitMode {
        self.fit.unwrap_or_default()
    }

    /// 裁不裁边。**默认裁**，而这个 `true` 在本仓库只有这一处：
    /// `Request::crop` 是个裸 `bool`，库那一侧没有一个 `Default` 说得出它。
    pub fn crop(&self) -> bool {
        self.crop.unwrap_or(true)
    }

    /// 关不关卷级上包络（ADR 0006 决定第 6 条）。**默认不关。**
    pub fn per_page(&self) -> bool {
        self.per_page.unwrap_or(false)
    }

    /// 怎么拆跨页。三项收成库那一侧的一份规矩，各自的默认在 [`SplitRule::default`]。
    pub fn split_rule(&self) -> SplitRule {
        let default = SplitRule::default();
        SplitRule {
            on: self.split.unwrap_or(default.on),
            threshold: self.split_threshold.unwrap_or(default.threshold),
            order: self.reading_order.unwrap_or(default.order),
        }
    }

    /// 残差段的重采样滤波器（ADR 0001）。
    pub fn filter(&self) -> Filter {
        self.filter.unwrap_or_default()
    }

    /// 缓存预算（ADR 0005）。
    pub fn cache_budget(&self) -> CacheBudget {
        self.cache_budget.unwrap_or_default()
    }

    /// 读取策略（ADR 0009）。
    pub fn io_mode(&self) -> IoMode {
        self.io_mode.unwrap_or_default()
    }
}

/// 按名字读一份预设。命令行那一路只用得到这一件事（`Cli::preset`）。
pub fn load(name: &str) -> Result<Preset> {
    Presets::found().read(name)
}

/// 盘上那份预设文件。**这是本仓库唯一一处为了预设去碰文件系统的地方。**
///
/// 位置在造出来那一刻定死，因此用例拿得到一份指向临时目录的它，**不必去改进程的环境变量**
/// （`tests/preset.rs` 已经说过为什么不改：edition 2024 里 `set_var` 是 `unsafe` 的，
/// 而一个进程里的用例并行跑，改全局环境会互相打架）。
///
/// **位置本身答不出来时不在造它那一刻报**（这台机器连 `APPDATA` 都没设）：
/// 会话不该因为一个环境变量没设就进不去，而按下存或取的那一刻它说得出为什么。
pub struct Presets {
    /// 那份文件在哪，或者**为什么答不出来**。
    at: Result<PathBuf, String>,
}

// 列出来、存、覆盖三件事**只有会话按得动**（`p1-session/12`），而会话整个在 `tui`
// 那个特性后面：关掉它这几个方法就没有调用方了。`cargo clippy --no-default-features`
// 那一趟因此要在这里放行一次——挪不走，命令行那一路本来就只读预设、不写。
#[cfg_attr(
    not(feature = "tui"),
    allow(
        dead_code,
        reason = "预设的存与列出只有会话调得到，而会话在 tui 特性后面"
    )
)]
impl Presets {
    /// 用户配置目录下的那一份（见 [`file`]）。
    pub fn found() -> Self {
        Self {
            at: file().map_err(|error| format!("{error:#}")),
        }
    }

    /// 点名一个位置。**只给用例用**：真会话与命令行读的都是用户配置目录下那一份。
    #[cfg(test)]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            at: Ok(path.into()),
        }
    }

    /// 那份文件在哪。
    pub fn path(&self) -> Result<&Path> {
        self.at.as_deref().map_err(|said| anyhow!("{said}"))
    }

    /// 文件里有的那几个预设的名字，按字典序。
    ///
    /// **只读名字，不读内容**——与 [`read`] 的「只有点名的那一个要读得懂」同一条：
    /// 一份字段过时的预设不该让别的几份列不出来。
    ///
    /// **文件还不在就是一个都没有**，不是错误：那正是要按下存的那一刻
    /// （`Presets::read` 那一侧不同，见 [`no_preset_file_error`]）。
    pub fn names(&self) -> Result<Vec<String>> {
        let path = self.path()?;
        let Some(text) = self.text()? else {
            return Ok(Vec::new());
        };
        names(&text).with_context(|| format!("预设文件是 {}", path.display()))
    }

    /// 按名字读一份预设。文件不在、或者点名的那一份读不懂，都是一条说得清的错误。
    pub fn read(&self, name: &str) -> Result<Preset> {
        let path = self.path()?;
        let Some(text) = self.text()? else {
            return Err(no_preset_file_error(path));
        };
        read(&text, name).with_context(|| format!("预设文件是 {}", path.display()))
    }

    /// 存一份预设。**盖不掉同名的那一份**——撞上了就是 [`Saved::Taken`]，一个字节都不写。
    ///
    /// 覆盖是**另一个动作**（[`replace`](Self::replace)），不是这一个动作的一个参数：
    /// 会话里那两下（撞名先说一句、再按一次才覆盖）因此不必靠调用方记得传对那个布尔。
    pub fn save(&self, name: &str, preset: &Preset) -> Result<Saved> {
        let text = self.text()?.unwrap_or_default();
        if names(&text)?.iter().any(|taken| taken == name) {
            return Ok(Saved::Taken);
        }
        self.put(&insert(&text, name, preset)?)?;
        Ok(Saved::Written)
    }

    /// 覆盖同名的那一份。名字还空着时它与 [`save`](Self::save) 做的是同一件事。
    ///
    /// **只有用户当场确认过才走得到这里**（见 `session::press`）：盖掉的可能是别人手写的
    /// 一份预设，而重排会连那份文件里的注释与排版一起丢掉（见 [`insert`]）。
    pub fn replace(&self, name: &str, preset: &Preset) -> Result<()> {
        let text = self.text()?.unwrap_or_default();
        self.put(&insert(&text, name, preset)?)
    }

    /// 那份文件的正文。**还不在就是 `None`**——那不是一条错误，各调用方自己判。
    fn text(&self) -> Result<Option<String>> {
        let path = self.path()?;
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(anyhow::Error::new(error).context(format!("读不出 {path:?}"))),
        }
    }

    /// 把正文写到位。**先写到同一层的临时文件、再改名**——与输出容器同一条规矩
    /// （`crate::sink`：最终位置上要么是上一份、要么是完整的这一份，中间那一份不出现）。
    /// 这里更该如此：盖的是用户自己手写的一份配置。
    ///
    /// 临时名字是**推得出来的**，因此有一个远角上的代价（与 `crate::sink` 那一处同一条）：
    /// 两个会话在同一秒里各存一份预设时，两者写的是同一个临时文件。换成随机名字能躲开，
    /// 代价是硬停留下的那一份就再也认不出、也没有下一趟去清它。存预设是用户按一下的事，
    /// 不是几十分钟的一趟，这一头因此照 `sink` 那一处的取舍来。
    fn put(&self, text: &str) -> Result<()> {
        let path = self.path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建配置目录 {}", parent.display()))?;
        }
        let partial = partial_path(path);
        std::fs::write(&partial, text).with_context(|| format!("写 {}", partial.display()))?;
        std::fs::rename(&partial, path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), path.display()))
    }
}

/// 存一份预设的结果。
///
/// **撞名不是一条错误，是一个答案**：存这个动作走到底之前要先问一句
/// （会话里屏上说一句，再按一次 `⏎` 才覆盖，见 `session::press`）。
/// 写成错误的话，调用方就得靠比对错误文本才分得出「名字占着」与「盘满了」。
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "存预设只有会话按得动，而会话在 tui 特性后面")
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// 写进去了。
    Written,
    /// 那个名字已经有人占着，一个字节都没动。
    Taken,
}

/// 临时文件的位置：在最终名字后面接一段固定后缀，与最终位置**同一层**，
/// 改名因此不跨卷（与 `crate::sink` 的那一个同一条理由，两处各自只此一用）。
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}

/// 从一份预设文件的正文里读出点名的那一个。
///
/// **只有点名的那一个要读得懂。** 同一个文件里另一个预设字段过时了，不该让今天要用的这个
/// 跑不起来——那正是「不点名就不读」这条规矩在文件内部的同一个道理。
/// 代价是行号：整份文件按类型解析时 toml 报得出「第几行第几列」，
/// 而这里先把各预设当成不透明的值收下来，再单独解析点名的那一个，错误里因此只有字段名。
pub fn read(text: &str, name: &str) -> Result<Preset> {
    let file: File<toml::Value> = toml::from_str(text).context("预设文件不是一份读得懂的 TOML")?;
    let Some(value) = file.preset.get(name) else {
        return Err(no_such_preset_error(name, &file.preset));
    };
    value
        .clone()
        .try_into()
        .map_err(anyhow::Error::new)
        .and_then(resolve)
        .with_context(|| format!("读不懂预设「{name}」"))
}

/// 一份预设文件里有的那几个名字，按字典序。**只读名字，不读内容。**
///
/// 会话里那一栏列的就是它（`p1-session/12`）：列一份清单不该因为其中一份的字段过时
/// 而整个列不出来——与 [`read`] 的「只有点名的那一个要读得懂」同一条。
pub fn names(text: &str) -> Result<Vec<String>> {
    let file: File<toml::Value> = toml::from_str(text).context("预设文件不是一份读得懂的 TOML")?;
    Ok(file.preset.into_keys().collect())
}

/// 把若干命名预设写成一份预设文件的正文。
///
/// 与 [`read`] 是一对：写出去的每一项都用它那个类型的**规范名**（`FitMode::name` 那一族），
/// 而规范名按定义读得回同一个值——往返因此是等价的，不是「差不多」。
///
/// 真往盘上写的那一路（会话里按下存，`p1-session/12`）走 [`insert`]，而它写出去的那一份
/// 就是这里给的——格式两侧同一份，往返由本模块的用例钉住（spec 的 story 46）。
pub fn write(presets: &BTreeMap<String, Preset>) -> Result<String> {
    let file = File {
        preset: presets
            .iter()
            .map(|(name, preset)| (name.clone(), OnDisk::from(preset)))
            .collect(),
    };
    toml::to_string_pretty(&file).context("预设写不成 TOML")
}

/// 把一份预设放进一份预设文件的正文里，**其余的东西一样都不动**。
///
/// 两条路，差别在「别人手写的那些字节活不活得下来」：
///
/// - **名字还空着**：那两节**追加**在末尾，原文一个字节都不改——注释、排版、
///   连本模块读不懂的那几份预设，全部原样留着。
/// - **名字已经有了**：整份**重排**。那一份要换掉，而 `toml` 1.1 给不出「只改这一段、
///   别的字节不动」的写法（它没有保留格式的编辑端；`toml_edit` 是另一个包，
///   而引一个依赖是一条依赖面上的决定，不是本票该顺手拿的——停车场 Q73）。
///   重排之后每一份预设的**内容**都还在（读不懂的那几份按不透明的 [`toml::Value`]
///   原样搬过去），丢掉的是注释与手写的排版。**会话在覆盖之前把这句话说出口**
///   （见 `session::state` 那句「再按一次 ⏎ 覆盖」）。
///
/// **读不懂的整份文件当场报错、一个字节都不写**：拿一份自己都没读懂的文件去覆盖，
/// 是这一路上最不该做的事。
pub fn insert(text: &str, name: &str, preset: &Preset) -> Result<String> {
    let file: File<toml::Value> = toml::from_str(text).context("预设文件不是一份读得懂的 TOML")?;
    let added = write(&BTreeMap::from([(name.to_owned(), preset.clone())]))?;
    if !file.preset.contains_key(name) {
        let appended = append(text, &added);
        // 追加完仍要**验一遍**：`preset` 那张表写成内联表时（`preset = { 漫画 = … }`），
        // 后面再摆一节 `[preset."名字".device]` 是接不上去的。验不过就走重排那一条。
        if appends_cleanly(&appended, name, preset, &file.preset) {
            return Ok(appended);
        }
    }
    let mut sections: BTreeMap<&str, String> = BTreeMap::new();
    for (other, value) in &file.preset {
        sections.insert(other, section(other, value)?);
    }
    sections.insert(name, added);
    Ok(sections.into_values().collect::<Vec<_>>().join("\n"))
}

/// 一份预设在文件里的那两节，连同它的名字。
///
/// 只有[重排](insert)那一条用得到它，而且**收的是不透明的 [`toml::Value`]**：
/// 那一路要把本模块读不懂的几份原样搬过去。点名的那一份走的仍是 [`write`]——
/// 写出去的形状因此只有一处出处。
fn section<P: Serialize>(name: &str, preset: P) -> Result<String> {
    toml::to_string_pretty(&File {
        preset: BTreeMap::from([(name.to_owned(), preset)]),
    })
    .context("预设写不成 TOML")
}

/// 把新的那两节接在原文后面。原文那一截**一个字节都不改**。
fn append(text: &str, added: &str) -> String {
    let mut appended = text.to_owned();
    if !appended.is_empty() {
        if !appended.ends_with('\n') {
            appended.push('\n');
        }
        appended.push('\n');
    }
    appended.push_str(added);
    appended
}

/// 追加出来的那一份读得回去吗：点名的那一份如数读得回来，原来那几份一个都没变样。
///
/// 验它而不是信它：TOML 里同一张表有好几种写法，而追加只对其中一种成立。
fn appends_cleanly(
    appended: &str,
    name: &str,
    preset: &Preset,
    before: &BTreeMap<String, toml::Value>,
) -> bool {
    let Ok(after) = toml::from_str::<File<toml::Value>>(appended) else {
        return false;
    };
    after.preset.len() == before.len() + 1
        && before
            .iter()
            .all(|(other, value)| after.preset.get(other) == Some(value))
        && read(appended, name).is_ok_and(|read_back| read_back == *preset)
}

/// 预设文件的位置：用户配置目录下的 `tonefit/presets.toml`。
///
/// Windows 取 `%APPDATA%`，其余平台取 `$XDG_CONFIG_HOME`、没设就是 `~/.config`。
/// 不引一个专管目录的包：要问的只有这两个环境变量，而多一个依赖要多一份许可与一次审计。
pub fn file() -> Result<PathBuf> {
    Ok(config_dir()?.join(PRESET_FILE))
}

#[cfg(windows)]
fn config_dir() -> Result<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("找不到用户配置目录：环境变量 APPDATA 没有设"))
}

#[cfg(not(windows))]
fn config_dir() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config"))
        .ok_or_else(|| anyhow!("找不到用户配置目录：环境变量 XDG_CONFIG_HOME 与 HOME 都没有设"))
}

/// 一份预设文件：顶层只有 `preset` 这一张表，键是预设的名字。
///
/// 顶层留出这一层而不是让预设名直接当顶层键，是给「预设的字段演进」留的位置
/// （`CONTEXT.md` 的《尚未确立》）：那件事真要落地时，第一个要加的多半是一个文件级的
/// 版本号，而它得有地方放。
///
/// 泛型的那一格是读与写的差别：读的时候先按 [`toml::Value`] 原样收下（见 [`read`]），
/// 写的时候直接摆上 [`OnDisk`]。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File<P> {
    // 点名的是 `BTreeMap::new` 而不是 `#[serde(default)]`：后者会让 derive 给 `P` 加一条
    // `Default` 约束，而读的时候 `P` 是 `toml::Value`，它没有默认值。
    #[serde(default = "BTreeMap::new")]
    preset: BTreeMap<String, P>,
}

/// 一份预设在盘上的形状。取值一律是命令行上那一项的写法。
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OnDisk {
    #[serde(default)]
    device: OnDiskDevice,
    #[serde(default)]
    taste: OnDiskTaste,
}

/// 设备层在盘上的形状。
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct OnDiskDevice {
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gray_levels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    threshold: Option<f64>,
}

/// 口味层在盘上的形状。
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct OnDiskTaste {
    #[serde(skip_serializing_if = "Option::is_none")]
    fit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    split: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    split_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reading_order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bit_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dither: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    per_page: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_budget: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    io_mode: Option<String>,
}

/// 把盘上那份验成一组类型好的值。**点名的那个预设整份都要读得懂**——
/// 与这一趟有没有拿命令行盖掉其中某一项无关：一项一项当场解析出来，解析不出来就报错。
///
/// 「整份」这句话之所以成立，靠的是[设备层那两个覆盖项要连同型号一起写](no_panel_to_calibrate_against_error)：
/// 它们的界挂在 profile 上（`Profile::with_gray_levels` 与 `with_threshold` 是那两个界唯一的
/// 出处），没有型号就没有面板可验，那一支会静默地放过一个越界的数。
fn resolve(raw: OnDisk) -> Result<Preset> {
    let threshold = raw.device.threshold.map(|value| value as f32);
    let profile = match &raw.device.profile {
        Some(name) => {
            let mut profile = Profile::resolve(name)?;
            if let Some(levels) = raw.device.gray_levels {
                profile = profile.with_gray_levels(levels)?;
            }
            if let Some(threshold) = threshold {
                profile = profile.with_threshold(threshold)?;
            }
            Some(profile.device().to_owned())
        }
        None if raw.device.gray_levels.is_some() || threshold.is_some() => {
            return Err(no_panel_to_calibrate_against_error());
        }
        None => None,
    };
    Ok(Preset {
        device: DeviceLayer {
            profile,
            gray_levels: raw.device.gray_levels,
            threshold,
        },
        taste: TasteLayer {
            fit: raw.taste.fit.as_deref().map(FitMode::resolve).transpose()?,
            crop: raw.taste.crop,
            split: raw.taste.split,
            split_threshold: raw
                .taste
                .split_threshold
                .map(|value| SplitThreshold::parse(&value.to_string()))
                .transpose()?,
            reading_order: raw
                .taste
                .reading_order
                .as_deref()
                .map(ReadingOrder::resolve)
                .transpose()?,
            filter: raw
                .taste
                .filter
                .as_deref()
                .map(Filter::resolve)
                .transpose()?,
            bit_depth: raw.taste.bit_depth.map(BitDepth::from_bits).transpose()?,
            dither: raw
                .taste
                .dither
                .as_deref()
                .map(Dither::resolve)
                .transpose()?,
            per_page: raw.taste.per_page,
            cache_budget: raw
                .taste
                .cache_budget
                .as_deref()
                .map(CacheBudget::parse)
                .transpose()?,
            io_mode: raw
                .taste
                .io_mode
                .as_deref()
                .map(IoMode::resolve)
                .transpose()?,
        },
    })
}

impl From<&Preset> for OnDisk {
    fn from(preset: &Preset) -> Self {
        Self {
            device: OnDiskDevice {
                profile: preset.device.profile.clone(),
                gray_levels: preset.device.gray_levels,
                threshold: preset.device.threshold.map(f64::from),
            },
            taste: OnDiskTaste {
                fit: preset.taste.fit.map(|fit| fit.name().to_owned()),
                crop: preset.taste.crop,
                split: preset.taste.split,
                // 拆分阈值在盘上是个数，而它的界只有 `SplitThreshold::parse` 一处出处，
                // 那一处收的是文本。`f64` 的 `Display` 是能原样读回来的最短写法，
                // 转一道文本因此不丢精度——往返用例钉着这一点。
                split_threshold: preset.taste.split_threshold.map(SplitThreshold::value),
                reading_order: preset
                    .taste
                    .reading_order
                    .map(|order| order.name().to_owned()),
                filter: preset.taste.filter.map(|filter| filter.name().to_owned()),
                bit_depth: preset.taste.bit_depth.map(BitDepth::bits),
                dither: preset.taste.dither.map(|dither| dither.name().to_owned()),
                per_page: preset.taste.per_page,
                cache_budget: preset.taste.cache_budget.map(spell_budget),
                io_mode: preset.taste.io_mode.map(|mode| mode.name().to_owned()),
            },
        }
    }
}

/// 把缓存预算写成 `CacheBudget::parse` 认得的写法：整 G/M/K 就带后缀，其余原样写字节数。
///
/// `CacheBudget` 的 `Display` 顶不了这件事——那一份是给人看的（`512 MiB`），
/// 而 `parse` 读不回来。往返用例逐个验 `parse(spell(x)) == x`。
///
/// 会话也拿它（`p1-session/08`）：光标停在缓存预算那一行按下回车时，缓冲里摆的
/// 必须是**改一个字就能再收下**的写法，而那与写进预设的写法是同一件事。
pub fn spell_budget(budget: CacheBudget) -> String {
    const UNITS: [(u64, &str); 3] = [(1024 * 1024 * 1024, "G"), (1024 * 1024, "M"), (1024, "K")];
    let bytes = budget.bytes();
    for (scale, suffix) in UNITS {
        if bytes >= scale && bytes.is_multiple_of(scale) {
            return format!("{}{suffix}", bytes / scale);
        }
    }
    bytes.to_string()
}

/// 设备层写了覆盖项却没写型号时的说法。
///
/// 这条规矩不是为了让校验好写，是因为那两个数**本来就绑着一块面板**：
/// 感知可分辨级数是在某一台真机上数出来的，阈值是在某一块面板上盲测夹出来的，
/// 而判据跟着面板走、不可跨面板比较（ADR 0002）。一份不说是哪块面板的
/// 「12 级 / 阈值 5.2」，套到下一台设备上就是一次无声的跨面板搬运。
///
/// 校验因此顺带完整了：设备层要么一项覆盖都没有，要么连型号一起在场，
/// 于是[读进来那一刻](resolve)每一项都验得动——「整份都要读得懂」不必再带个例外。
fn no_panel_to_calibrate_against_error() -> anyhow::Error {
    anyhow!(
        "预设的设备层写了 gray-levels 或 threshold，却没有写 profile。\
         那两个数都是在**某一块面板上**标定出来的——感知可分辨级数是在真机上数出来的，\
         阈值是在一块面板上盲测夹出来的，而判据跟着面板走、不可跨面板比较（ADR 0002）。\
         把型号一起写进设备层，或者把这两项交给命令行。"
    )
}

/// 还没有预设文件时的说法：把位置说出来，再把格式当场教一遍。
///
/// 教格式而不是只报「文件不在」：预设是用户自己手写的一份文件，而写它的人手上
/// 只有这条错误——`--help` 那一段也说得到，但那要他先想到去看。
fn no_preset_file_error(path: &Path) -> anyhow::Error {
    anyhow!(
        "还没有预设文件：{} 不在。\n\
         一份文件装多个命名预设，各装设备层与口味层两层：\n\
         \n\
         \x20 [preset.\"漫画\".device]\n\
         \x20 profile = \"kobo-libra-2\"\n\
         \x20 gray-levels = 12\n\
         \n\
         \x20 [preset.\"漫画\".taste]\n\
         \x20 fit = \"inside\"\n\
         \x20 cache-budget = \"1G\"\n\
         \n\
         处理范围与输出根不进预设——那两样每趟都不同（ADR 0009）。",
        path.display()
    )
}

/// 文件在、点名的那个不在：把这份文件里有的那几个端出来。
///
/// 与未知型号那条错误同一个形状（`Profile::resolve`）：认不出用户给的那个词时，
/// 把认得的全列出来，用户从清单里挑，而不是回去翻文档。
fn no_such_preset_error<P>(name: &str, presets: &BTreeMap<String, P>) -> anyhow::Error {
    if presets.is_empty() {
        return anyhow!("预设文件里一个预设都没有，找不到「{name}」。");
    }
    let names: Vec<&str> = presets.keys().map(String::as_str).collect();
    anyhow!("预设文件里没有「{name}」。有的是：{}。", names.join(" "))
}

/// 一份每一项都写满的预设。往返用例要的是「每一项都验过」，不是「随便挑几项」。
///
/// 会话那一侧的用例也拿它（`crate::session::state`）：屏上的两层与盘上的两层格数对不对得上，
/// 靠的正是这一份「说满了」的预设——它**没有 `..Default::default()`**，
/// 往任何一层加一个字段，这里当场编译不过；补完之后盘上那一节就多一个键，
/// 而屏上的行数没跟着变，那一条断言随之变红。
#[cfg(test)]
pub fn every_field() -> Preset {
    Preset {
        device: DeviceLayer {
            profile: Some("boox-poke6".to_owned()),
            gray_levels: Some(12),
            threshold: Some(4.75),
        },
        taste: TasteLayer {
            fit: Some(FitMode::Inside),
            crop: Some(false),
            split: Some(false),
            split_threshold: Some(SplitThreshold::parse("1.75").expect("是个正数")),
            reading_order: Some(ReadingOrder::LeftToRight),
            filter: Some(Filter::Hamming),
            bit_depth: Some(BitDepth::Two),
            dither: Some(Dither::FloydSteinberg),
            per_page: Some(true),
            cache_budget: Some(CacheBudget::parse("1G").expect("认得的写法")),
            io_mode: Some(IoMode::Concurrent),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份预设文件的正文，装着 `name` 这一个预设。
    fn one(name: &str, preset: &Preset) -> String {
        write(&BTreeMap::from([(name.to_owned(), preset.clone())])).expect("写得出来")
    }

    /// 往返：存出来再读回去等价（07 号票的验收）。
    ///
    /// 「等价」比的是**值**，不是文本：写出去的是各类型的规范名，用户原来手写的可能是
    /// 另一个别名（`floyd-steinberg` 之于 `fs`），文本因此本来就不必逐字相同。
    #[test]
    fn a_preset_written_out_reads_back_the_same() {
        let preset = every_field();

        let text = one("漫画", &preset);
        let read_back = read(&text, "漫画").expect("读得回来");

        assert_eq!(read_back, preset, "写出去的是：\n{text}");
    }

    /// 空预设也往返：一项都没写的那份读回来仍是一项都没写，不是一份默认值。
    ///
    /// 这条钉的是 `Option` 那一层——预设**只说它说到的那几项**。收成默认值的话，
    /// 「预设里没提的项落到命令行上」就成了「预设把每一项都盖了一遍」。
    #[test]
    fn a_preset_that_says_nothing_reads_back_saying_nothing() {
        let text = one("空的", &Preset::default());

        assert_eq!(read(&text, "空的").expect("读得回来"), Preset::default());
    }

    /// 每一种缓存预算的写法都读得回同一个预算。
    #[test]
    fn every_budget_spelling_reads_back_as_the_same_budget() {
        for text in ["1", "1023", "1K", "5M", "512M", "1G", "3G"] {
            let budget = CacheBudget::parse(text).expect("认得的写法");
            let spelled = spell_budget(budget);
            assert_eq!(
                CacheBudget::parse(&spelled).expect("写出去的也认得"),
                budget,
                "{text} 写成了 {spelled}"
            );
        }
        // 整数倍的那几档挑最大的那个后缀，人读得出来。
        assert_eq!(spell_budget(CacheBudget::default()), "512M");
        assert_eq!(spell_budget(CacheBudget::new(1023)), "1023");
    }

    /// 字段过时：认不出的键当场是一条错误，不是被悄悄忽略的一行（07 号票的验收）。
    #[test]
    fn a_field_the_tool_no_longer_knows_is_an_error() {
        let text = "\
[preset.\"漫画\".taste]
fit = \"inside\"
sharpen = true
";

        let error = read(text, "漫画")
            .expect_err("认不出的字段该报错")
            .to_string();

        assert!(error.contains("漫画"), "{error}");
    }

    /// 型号已删：内置表里没有的型号当场报错，并把有的那几个列出来。
    #[test]
    fn a_model_that_is_not_in_the_table_is_an_error() {
        let text = "[preset.\"漫画\".device]\nprofile = \"kobo-libra-9\"\n";

        let error = read(text, "漫画").expect_err("未知型号该报错").to_string();

        assert!(error.contains("漫画"), "{error}");
    }

    /// 预设不装范围层：处理范围与输出根写进来是错误，不是被忽略的两行（07 号票的验收）。
    ///
    /// 靠的是两层各自的 `deny_unknown_fields`——「预设不含范围层」因此是格式的性质，
    /// 不是「我们没去读它」这种自觉。
    #[test]
    fn the_scope_layer_has_no_place_in_a_preset() {
        for text in [
            "[preset.\"漫画\".device]\nout = \"D:/out\"\n",
            "[preset.\"漫画\".taste]\ninputs = [\"D:/库/卷一\"]\n",
            "[preset.\"漫画\"]\nout = \"D:/out\"\n",
            "[preset.\"漫画\".scope]\nout = \"D:/out\"\n",
        ] {
            assert!(read(text, "漫画").is_err(), "范围层不该被收下：{text}");
        }
    }

    /// 取值拼错：每一项都当场解析，认不出就报错，不静默套默认值。
    #[test]
    fn a_value_that_does_not_resolve_is_an_error() {
        for text in [
            "[preset.\"漫画\".taste]\nfit = \"cover\"\n",
            "[preset.\"漫画\".taste]\nfilter = \"mitchell\"\n",
            "[preset.\"漫画\".taste]\ndither = \"bayer\"\n",
            "[preset.\"漫画\".taste]\nreading-order = \"ttb\"\n",
            "[preset.\"漫画\".taste]\nio-mode = \"parallel\"\n",
            "[preset.\"漫画\".taste]\nbit-depth = 3\n",
            "[preset.\"漫画\".taste]\nsplit-threshold = 0.0\n",
            "[preset.\"漫画\".taste]\ncache-budget = \"512T\"\n",
            "[preset.\"漫画\".device]\nprofile = \"boox-poke6\"\ngray-levels = 0\n",
            "[preset.\"漫画\".device]\nprofile = \"boox-poke6\"\nthreshold = 0.0\n",
        ] {
            assert!(read(text, "漫画").is_err(), "该被挡下：{text}");
        }
    }

    /// 设备层那两个覆盖项**要连同型号一起写**，否则当场报错。
    ///
    /// 两件事一条规矩管住：那两个数绑着一块面板（ADR 0002），不说是哪块就是一次
    /// 无声的跨面板搬运；而它们的界也挂在 profile 上——不要求型号在场，
    /// `gray-levels = 0` 这种越界值就会在「这一趟拿命令行盖掉了它」的时候被静默放过，
    /// 「点名的那个预设整份都要读得懂」随之带上一个例外。
    #[test]
    fn a_calibrated_override_must_name_the_panel_it_was_measured_on() {
        for text in [
            "[preset.\"漫画\".device]\ngray-levels = 12\n",
            "[preset.\"漫画\".device]\nthreshold = 5.2\n",
        ] {
            // 按 `{:?}` 取——`to_string` 只给最外那一层上下文，说清是哪一项的那句在因由里。
            let error = format!(
                "{:?}",
                read(text, "漫画").expect_err("没有型号的覆盖项该被挡下")
            );
            assert!(error.contains("profile"), "{error}");
        }

        // 界也因此每一次都验得动：越界的数不再依赖「这一趟有没有盖掉它」。
        assert!(read("[preset.\"漫画\".device]\ngray-levels = 0\n", "漫画").is_err());

        // 反过来，一项覆盖都没有的设备层照旧收下——口味层单独成一份预设是正当的。
        assert_eq!(
            read("[preset.\"漫画\".taste]\nfit = \"inside\"\n", "漫画")
                .expect("读得懂")
                .device,
            DeviceLayer::default()
        );
    }

    /// **只有点名的那一个要读得懂。** 同一份文件里另一个预设字段过时了，
    /// 不该让今天要用的这个跑不起来。
    #[test]
    fn only_the_named_preset_has_to_be_readable() {
        let text = "\
[preset.\"旧的\".taste]
sharpen = true

[preset.\"漫画\".taste]
fit = \"inside\"
";

        assert_eq!(
            read(text, "漫画").expect("点名的这个读得懂").taste.fit,
            Some(FitMode::Inside)
        );
        assert!(read(text, "旧的").is_err(), "点名过时的那个仍要报错");
    }

    /// 点名的预设不在文件里：把文件里有的那几个端出来。
    #[test]
    fn a_preset_that_is_not_in_the_file_names_the_ones_that_are() {
        let text = "[preset.\"漫画\".taste]\nfit = \"inside\"\n\n[preset.\"画集\".taste]\nper-page = true\n";

        let error = read(text, "小说").expect_err("没有这个预设").to_string();

        assert!(error.contains("小说"), "{error}");
        assert!(error.contains("漫画") && error.contains("画集"), "{error}");
    }

    /// 手写的预设按用户敲的写法读，与命令行同一套：`fs` 与 `floyd-steinberg` 是同一个。
    ///
    /// 钉的是「预设不是第二套语法」——解析走的就是命令行那几个函数。
    #[test]
    fn a_preset_reads_the_same_spellings_the_command_line_takes() {
        let text = "\
[preset.\"漫画\".device]
profile = \"Kobo Libra 2\"

[preset.\"漫画\".taste]
filter = \"box\"
dither = \"floyd-steinberg\"
reading-order = \"left-to-right\"
";

        let preset = read(text, "漫画").expect("读得懂");

        // 型号归一到内置表里的规范名，与 `--profile` 走的是同一个 `Profile::resolve`。
        assert_eq!(preset.device.profile.as_deref(), Some("kobo-libra-2"));
        assert_eq!(preset.taste.filter, Some(Filter::Area));
        assert_eq!(preset.taste.dither, Some(Dither::FloydSteinberg));
        assert_eq!(preset.taste.reading_order, Some(ReadingOrder::LeftToRight));
    }

    /// **存一份新的：原文一个字节都不改。**
    ///
    /// 注释、排版、连本模块读不懂的那几份预设，全部原样留在原处（[`insert`] 的追加那一条）。
    #[test]
    fn saving_a_new_preset_leaves_every_byte_of_the_file_where_it_was() {
        let by_hand = "\
# 我手写的
[preset.\"旧的\".taste]
# 这一项本模块读不懂
sharpen = true
";

        let after = insert(by_hand, "漫画", &every_field()).expect("存得进去");

        assert!(after.starts_with(by_hand), "原文那一截被改动了：\n{after}");
        assert_eq!(read(&after, "漫画").expect("读得回来"), every_field());
        assert!(after.contains("# 这一项本模块读不懂"), "{after}");
        assert!(read(&after, "旧的").is_err(), "读不懂的那一份被悄悄改写了");
        assert_eq!(names(&after).expect("列得出来"), ["旧的", "漫画"]);
    }

    /// **覆盖同名的那一份：其余几份一个都不少**，包括本模块读不懂的那几份。
    ///
    /// 代价一并钉在这里：整份**重排**，注释与手写的排版留不下来。会话在按下去之前
    /// 把这句话说出口（`session::state::Session::name_is_taken`）——`toml` 1.1
    /// 给不出保留格式的编辑端（停车场 Q73）。
    #[test]
    fn overwriting_one_preset_keeps_the_others_but_not_the_comments() {
        let by_hand = "\
# 我手写的
[preset.\"漫画\".taste]
filter = \"box\"

[preset.\"旧的\".taste]
sharpen = true
";

        let after = insert(by_hand, "漫画", &every_field()).expect("覆盖得了");

        assert_eq!(read(&after, "漫画").expect("读得回来"), every_field());
        assert!(
            after.contains("sharpen = true"),
            "另一份预设丢了：\n{after}"
        );
        assert_eq!(names(&after).expect("列得出来"), ["旧的", "漫画"]);
        assert!(
            !after.contains("# 我手写的"),
            "重排之后注释还在——那说明格式其实保得住，这一条与它的文档都该改：\n{after}"
        );
    }

    /// **读不懂的整份文件当场报错，一个字节都不写。**
    ///
    /// 拿一份自己都没读懂的文件去覆盖，是这一路上最不该做的事。
    #[test]
    fn a_file_that_does_not_parse_is_never_written_over() {
        for text in ["这不是一份 TOML", "[preset.\"漫画\"\n", "另有一节 = 1\n"] {
            assert!(
                insert(text, "漫画", &every_field()).is_err(),
                "读不懂却写了：{text}"
            );
        }
    }

    /// 盘上那一份：**列、读、存、覆盖**四件事在一个真文件上走一遍。
    ///
    /// 文件摆在临时目录下（[`Presets::at`]），一个用户的东西都不碰。
    #[test]
    fn the_file_on_disk_takes_a_preset_and_gives_it_back() {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let home = space.path().join("配置").join("tonefit");
        let presets = Presets::at(home.join("presets.toml"));

        // 文件还不在：列出来是一个都没有（不是错误），而读一份是一条说得清的错误。
        assert!(presets.names().expect("列得出来").is_empty());
        assert!(presets.read("漫画").is_err());

        // 存：上一层目录跟着建出来。
        assert_eq!(
            presets.save("漫画", &every_field()).expect("存得进去"),
            Saved::Written
        );
        assert_eq!(presets.names().expect("列得出来"), ["漫画"]);
        assert_eq!(presets.read("漫画").expect("读得回来"), every_field());
        // 改名到位之后不留临时文件。
        let left: Vec<_> = std::fs::read_dir(&home)
            .expect("读得出配置目录")
            .filter_map(|entry| Some(entry.ok()?.file_name()))
            .collect();
        assert_eq!(left, ["presets.toml"], "写完留下了临时文件：{left:?}");

        // 同一个名字再存：**盖不掉**，一个字节都没动。
        let before = std::fs::read_to_string(home.join("presets.toml")).expect("读得出来");
        assert_eq!(
            presets.save("漫画", &Preset::default()).expect("问得出来"),
            Saved::Taken
        );
        assert_eq!(
            std::fs::read_to_string(home.join("presets.toml")).expect("读得出来"),
            before,
            "存那一下把同名的那一份盖了"
        );

        // 覆盖是另一个动作，走到底才换得掉。
        presets
            .replace("漫画", &Preset::default())
            .expect("覆盖得了");
        assert_eq!(presets.read("漫画").expect("读得回来"), Preset::default());
    }

    /// 预设文件的位置在用户配置目录之下，文件名是 `presets.toml`。
    ///
    /// 不断言全路径——那随这台机器的环境变量而变。断言的是尾巴那两截：
    /// 用例把配置目录指到临时目录上时，靠的正是这两截。
    #[test]
    fn the_preset_file_sits_under_the_user_config_directory() {
        let path = file().expect("这台机器答得出用户配置目录");

        assert!(path.ends_with(Path::new(PRESET_FILE)), "{path:?}");
    }
}
