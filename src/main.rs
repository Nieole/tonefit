//! CLI：把命令行参数拼成 `Request`，分派到库那两件事上，把进度画出来。此外不做别的事。
//!
//! 出来的文字长什么样不在这里，在 [`render`]——命令行与会话共用同一套措辞，
//! 那一套因此不该长在任何一个入口里面。
//!
//! **无参数即会话，带参数即直接跑**（`CONTEXT.md` 的《会话》）。分岔在
//! [`without_arguments`]，排在 clap **之前**：带参数那一路因此一字不变，
//! 连必填项的判定都没有被松动过。

mod preset;
mod render;
mod wrap;
// 会话的状态机那四个模块一个终端库都不 `use`，因此 `tui` 关掉的那一趟仍编译、仍跑它们
// 自带的用例（闸门的第二条，`docs/agents/gate.md`）。这两句 `cfg` 各自为什么必要——
// `test` 那一格与那笔 `dead_code` 放松——见 `session` 模块文档的《终端库在哪一半》。
#[cfg(any(feature = "tui", test))]
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
mod session;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::builder::{Resettable, StyledStr};
use clap::{CommandFactory, FromArgMatches, Parser};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tonefit::{
    BitDepth, CacheBudget, Dither, Event, Filter, FitMode, Instruction, Interlock, IoMode, Mode,
    Profile, Progress, ProgressSink, ReadingOrder, Report, Request, SplitRule, SplitThreshold,
};

use preset::Preset;

#[derive(Parser)]
// 不点子命令就是「处理点名的若干卷」这一件事，那是绝大多数时候要做的：
// `args_conflicts_with_subcommands` 把两种用法分开，`subcommand_negates_reqs`
// 让子命令不必再交出处理卷才要的那几个必填项。
#[command(
    about = "把漫画页适配到电子墨水阅读设备",
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    after_long_help = interlock_help()
)]
struct Cli {
    /// 另做一件事，而不是处理卷。
    #[command(subcommand)]
    command: Option<Command>,

    /// 在哪里找卷：一个归档，或一个目录——目录里直接躺着页就是一个卷，
    /// 底下的子目录与归档也各自成卷。源只读。
    ///
    /// 认得的扩展名**不写在这条注释里**：格式集由库那一份拼出来（见 [`inputs_help`]），
    /// 抄在这里就等于给它开第二个出处，而那一份加一项时这里不会跟着走。
    #[arg(
        required = true,
        value_name = "路径",
        help = inputs_help(),
        long_help = inputs_help()
    )]
    inputs: Vec<PathBuf>,

    /// 输出根目录。产物按源的结构镜像到它下面，点名路径自己的名字打头；
    /// 容器形态与输入一致，归档卷的扩展名一律写成 .cbz。
    #[arg(short, long, required = true, value_name = "目录")]
    out: Option<PathBuf>,

    /// 套用一份命名预设：**设备层**（型号、感知可分辨级数、阈值）与**口味层**（这一趟的立场）
    /// 从盘上那份配置里来。
    ///
    /// **只在点名它的时候读盘。** 不点名的那一趟命令行仍是全部输入，同一条命令在两台机器上
    /// 行为相同。
    ///
    /// **命令行上显式点到的那一项赢**：`--preset 漫画 --filter hamming` 就是「套那一份，
    /// 再改这一项」。三个关掉什么的开关（--no-crop、--no-split、--per-page）只说得出一个方向，
    /// 预设把裁边关掉之后这一趟没有再开回来的写法。
    ///
    /// 预设**不装处理范围与输出根**——那两样每趟都不同，混进去会让人套用预设时误写到上一次的
    /// 输出目录。`--out` 因此照旧必填，`--profile` 只在预设供了型号时才不必填。
    ///
    /// 一份文件装多个命名预设，落在用户配置目录下的 tonefit/presets.toml
    /// （Windows 取 %APPDATA%，其余平台取 $XDG_CONFIG_HOME、没设就是 ~/.config）。
    /// 每个预设两节：`[preset."名字".device]` 装型号、感知可分辨级数与阈值，
    /// `[preset."名字".taste]` 装这一趟的立场；键名就是去掉 `--` 的 flag 名
    /// （`gray-levels`、`split-threshold`、`cache-budget`……），
    /// 取值的写法与命令行上一模一样。名字带中文要按 TOML 的规矩加引号。
    ///
    /// 那份文件还不在时，`--preset` 会把一份可以照抄的样例连同位置一起印出来。
    ///
    /// 读不懂的预设（字段过时、型号已删、取值拼错）当场报错，不静默套默认值。
    #[arg(long, value_name = "名字")]
    preset: Option<String>,

    /// 目标设备型号。内置表覆盖 Kobo、BOOX、Kindle 的主力型号，型号名不区分大小写与分隔符。
    ///
    /// 点了 `--preset` 而那份预设的设备层供了型号时，这一项可以不填。
    #[arg(short, long, required_unless_present = "preset", value_name = "型号")]
    profile: Option<String>,

    /// 覆盖面板灰阶数。内置表没收录的设备、或在真机上数出的实际可分辨级数走这里。
    #[arg(long, value_name = "级数")]
    gray_levels: Option<u32>,

    /// 覆盖判定用的阈值。内置值由一块面板上的真机盲测定出，**没有逐面板复核**——
    /// 判据跟着面板走、不可跨面板比较（ADR 0002）。在自己那台设备上盲测出来的界填这里：
    /// 拿 `--dry-run` 逐页读出各档判据值，把同一页的各档拷进设备目视，
    /// 记下最低的那个**不可接受**档的判据值，界取在它之下。
    #[arg(long, value_name = "界")]
    threshold: Option<f32>,

    /// 这一趟怎么把页适配到面板：height（默认，以高为准）、inside（fit-inside）。
    ///
    /// **默认 height**：目标高恒等于面板高，宽按源宽高比算出、**允许超过面板宽**，
    /// 超出的部分靠阅读器横向平移。跨页因此从压扁状态变得可读，代价要认下来——
    /// **跨页卷的体积涨到约三四倍**，而比面板矮的页会被**放大**到面板高。
    ///
    /// 普通漫画页两种方式产出同一个尺寸：页比面板更瘦长、本来就受高度约束
    /// （实测棋魂 230 页、N和S 24 页 100% 一致）。
    ///
    /// inside 是从前那条路：整页放进面板，源比面板小时不放大，页两侧留边。
    /// 它对阅读器的要求更松——留边只要阅读器填背景不重采样，溢出要它平移不缩放。
    ///
    /// 它与别的开关咬在一起时会怎样，见 `--help` 末尾的《开关互锁》。
    #[arg(long, value_name = "方式")]
    fit: Option<String>,

    /// 不裁掉页面白边。**默认是裁的**：tonefit 自己按行列墨量占比逐页裁边。
    ///
    /// 裁边的要点不是省白边，是让你**关得掉阅读器那一侧的裁切**：
    /// tonefit 裁完之后好处已经烤进产物，阅读器那个开关变成空操作，1:1 恢复。
    /// 留着它要付什么，见下面那句指路。
    ///
    /// 裁法按**行列墨量占比**，不是内容外接框：白边里的孤立噪点不算内容，
    /// 边缘一个墨点不会让裁边整个失效。逐页各裁各的，**页与页的字号因此会跳动**——
    /// 那是要更大实际利用面积的代价，不是缺陷。整页空白的页原样通过。
    ///
    /// 它与别的开关咬在一起时会怎样，见 `--help` 末尾的《开关互锁》。
    #[arg(long)]
    no_crop: bool,

    /// 不把跨页拆成两页。**默认是拆的**：tonefit 在装订沟上把跨页切成两半，每半用满面板高。
    ///
    /// 拆开之后每半基本不必横向翻动（实测三卷双页片源的屏占比中位从 1.88–1.91 落到 0.88–0.92），
    /// 而**缩放系数完全相同**——拆分不是拿分辨率换的。
    ///
    /// 判定分两级，两级都只看几何与墨量，不看画面语义：先按宽高比与面板比挑出跨页候选
    /// （阈值走 `--split-threshold`），再在跨页候选里找装订沟定切点。**切点跟着装订沟走，
    /// 不切正中**——实测沟中心在 0.401–0.538 之间，按正中盲切最偏的一页会切进画面近一成页宽
    /// （8K 跨页上是 599 像素）。
    ///
    /// **找不到装订沟的是连续跨页，不切**：那是为视觉效果画满两页的一幅整画，切开就毁了。
    /// 不切的页照这一趟的适配方式出——默认 height 下宽溢出面板，靠阅读器横向平移看。
    /// 关掉本项，所有跨页都走那条路。
    ///
    /// 它与别的开关咬在一起时会怎样，见 `--help` 末尾的《开关互锁》。
    #[arg(long)]
    no_split: bool,

    /// 跨页候选的阈值：页宽高比要有面板宽高比的这么多倍才算候选，默认 1.5。
    ///
    /// 混排卷是常态——一卷里有些页已经拆好、有些还是连页。调低了收得进更多页
    /// （已经拆好的单页可能被再切一刀），调高了把真跨页也放过去（它退回横向平移）。
    #[arg(long, value_name = "比")]
    split_threshold: Option<String>,

    /// 拆开后两半的先后：rtl（默认，右开，右半在先）、ltr（左开，左半在先）。
    ///
    /// 日式漫画是右开，国漫与西方漫画是左开。不读 ComicInfo.xml——八卷素材里它出现 0 次。
    #[arg(long, value_name = "方向")]
    reading_order: Option<String>,

    /// 残差段的重采样滤波器：area（= box）、bilinear、hamming、bicubic、lanczos3，默认 lanczos3。
    /// 只作用于残差段——总缩放比 ≥ 2 时的整数倍预缩那一级恒为 box。
    #[arg(long, value_name = "滤波器")]
    filter: Option<String>,

    /// 覆盖自动判定的位深：1、2、4、8。面板灰阶数那道上界仍在，越界的覆盖会被拒绝。
    #[arg(long, value_name = "位深")]
    bit_depth: Option<u32>,

    /// 覆盖自动选择的抖动模式：off（= none）、fs（= floyd-steinberg）。
    /// 抖动只在输出不被下游缩放时才谈得上，而这是**每一页**各自的事实——
    /// 点名 fs 因此会撞上几何门，那条互锁见 `--help` 末尾的《开关互锁》。
    /// 错误指得出是哪一页，不会静默照抖。
    #[arg(long, value_name = "模式")]
    dither: Option<String>,

    /// 关闭卷级上包络与迟滞，位深回到逐页最优。体积最小，代价是**重新引入翻页跳变**：
    /// 相邻两页会落到不同档上，翻过去的一瞬间灰调的颗粒感换一种粗细。
    #[arg(long)]
    per_page: bool,

    /// 两遍之间的缓存最多在内存里留多少：纯字节数，或带 K/M/G 后缀，默认 512M。
    /// 超出的页溢写临时文件，运行结束即收走。
    #[arg(long, value_name = "字节数")]
    cache_budget: Option<String>,

    /// 读取策略：auto（按路径探测介质，默认）、serial（读取串行）、concurrent（读取并发）。
    /// auto 下有寻道惩罚的盘串行读、没有的并发读；网络路径与探不出来的一律按未知退到串行。
    #[arg(long, value_name = "模式")]
    io_mode: Option<String>,

    /// 只算不写：报告照出，逐页给出判定与各候选的判据值，一个文件都不落盘。
    #[arg(long)]
    dry_run: bool,

    /// 不把自描述元数据写进输出 PNG。**幂等能力随之关闭**：判定与理由不再随文件走，
    /// 重跑时也无从判断这一卷变没变，每一趟都整卷重做。
    #[arg(long)]
    no_metadata: bool,
}

/// 命令行与预设合起来定出这一趟的每一项（p1-session 的 07 号票）。
///
/// 规矩只有一条：**命令行上显式点到的那一项赢。** 预设是存着的立场，命令行是这一趟当场的
/// 指令——「套一份预设，再改其中一项」因此写作 `--preset 漫画 --filter hamming`。
/// 命令行没点到的落到预设上，预设也没说的落到默认值上，而那些默认值仍在原来的地方
/// （库那一侧的 `Default`），这里一个都不复述。
///
/// **不点名预设时行为一字不变**：那一趟拿到的是一份 [`Preset::default`]——每一项都是「没说」，
/// 于是每一项都落到默认值上，与本票之前逐字相同。这不是靠比对来保证的，是同一段代码走过去
/// 的结果。
///
/// **三个布尔开关的覆盖是单向的。** `--no-crop`、`--no-split`、`--per-page` 在命令行上
/// 只说得出一个方向，说不出它们的反面；预设把裁边关掉之后，这一趟没有再把它开回来的写法
/// （要开回来就改预设，或者这一趟不套它）。取值一致性因此仍然成立——预设里的 `crop = false`
/// 与命令行上的 `--no-crop` 是同一件事——只是覆盖不对称。
impl Cli {
    /// 这一趟要套用的预设。**只在显式点名时读盘。**
    fn preset(&self) -> Result<Preset> {
        match &self.preset {
            Some(name) => preset::load(name),
            None => Ok(Preset::default()),
        }
    }

    /// 本次做到哪一步。
    ///
    /// 不收预设：`--dry-run` 说的是这一趟做到哪一步，不是一份存得住的立场
    /// （见 `preset::TasteLayer`）。
    fn mode(&self) -> Mode {
        if self.dry_run {
            Mode::DryRun
        } else {
            Mode::Process
        }
    }

    /// 本次的适配方式。不点名就是默认的以高为准（01 号票）。
    fn fit_mode(&self, preset: &Preset) -> Result<FitMode> {
        match &self.fit {
            Some(name) => FitMode::resolve(name),
            None => Ok(preset.taste.fit()),
        }
    }

    /// 本次裁不裁边（02 号票）。**默认裁**，`--no-crop` 关掉它。
    fn crop(&self, preset: &Preset) -> bool {
        // 默认值不在这里：它在 `TasteLayer::crop`，会话拼 `Request` 时读的是同一个
        // （`Request::crop` 是个裸 `bool`，库那一侧没有一个 `Default` 说得出它）。
        !self.no_crop && preset.taste.crop()
    }

    /// 本次关不关卷级上包络（ADR 0006 决定第 6 条）。**默认不关**，`--per-page` 打开它。
    fn per_page(&self, preset: &Preset) -> bool {
        self.per_page || preset.taste.per_page()
    }

    /// 本次怎么拆跨页（04 号票）。三项收成一份规矩交给库，见 [`SplitRule`]。
    ///
    /// 不点名就是默认那一套：拆、阈值 1.5、右开。三项各自的默认在库那一侧
    /// （`SplitRule::default`），这里只把命令行点到的那几项盖上去——写死一份就是第二个出处。
    fn split_rule(&self, preset: &Preset) -> Result<SplitRule> {
        let stored = preset.taste.split_rule();
        Ok(SplitRule {
            on: !self.no_split && stored.on,
            threshold: match &self.split_threshold {
                Some(text) => SplitThreshold::parse(text)?,
                None => stored.threshold,
            },
            order: match &self.reading_order {
                Some(name) => ReadingOrder::resolve(name)?,
                None => stored.order,
            },
        })
    }

    /// 本次残差段用哪个滤波器。不点名就是默认的 lanczos3（ADR 0001）。
    fn residual_filter(&self, preset: &Preset) -> Result<Filter> {
        match &self.filter {
            Some(name) => Filter::resolve(name),
            None => Ok(preset.taste.filter()),
        }
    }

    /// 本次要不要点名位深。不点名就由判据说了算。
    fn bit_depth_override(&self, preset: &Preset) -> Result<Option<BitDepth>> {
        match self.bit_depth {
            Some(bits) => BitDepth::from_bits(bits).map(Some),
            None => Ok(preset.taste.bit_depth),
        }
    }

    /// 本次要不要点名抖动模式。不点名就由判据在几何门放行的那几种里选。
    fn dither_override(&self, preset: &Preset) -> Result<Option<Dither>> {
        match self.dither.as_deref() {
            Some(name) => Dither::resolve(name).map(Some),
            None => Ok(preset.taste.dither),
        }
    }

    /// 本次的缓存预算。不点名就是默认的那一档。
    fn cache_budget(&self, preset: &Preset) -> Result<CacheBudget> {
        match &self.cache_budget {
            Some(text) => CacheBudget::parse(text),
            None => Ok(preset.taste.cache_budget()),
        }
    }

    /// 本次的读取策略。不点名就按路径探测介质（ADR 0009）。
    fn io_mode(&self, preset: &Preset) -> Result<IoMode> {
        match &self.io_mode {
            Some(text) => IoMode::resolve(text),
            None => Ok(preset.taste.io_mode()),
        }
    }

    /// 把 `--profile`、`--gray-levels` 与 `--threshold` 合成本次要用的 profile。
    ///
    /// 三项都在**设备层**，因此预设供得出它们中的任何一项。型号两处都没有时当场停下：
    /// clap 那道必填只放行「点了 `--preset`」这一种，而那份预设有没有型号只有读了才知道。
    fn target_profile(&self, preset: &Preset) -> Result<Profile> {
        let device = self
            .profile
            .as_deref()
            .or(preset.device.profile.as_deref())
            .ok_or_else(|| self.no_device_error())?;
        target_profile(
            device,
            self.gray_levels.or(preset.device.gray_levels),
            self.threshold.or(preset.device.threshold),
        )
    }

    /// 预设点了名却没供出型号时的说法。
    ///
    /// 走到这里就一定点过 `--preset`：命令行上没有型号时，clap 的
    /// `required_unless_present = "preset"` 只放行那一种。
    fn no_device_error(&self) -> anyhow::Error {
        let name = self.preset.as_deref().expect(NAMED_A_PRESET);
        anyhow!(
            "预设「{name}」的设备层没有型号，`--profile` 因此仍然必填。\
             要么在命令行上点名，要么往那份预设的 [preset.\"{name}\".device] 里写一行 profile = \"…\"。"
        )
    }

    /// 把命令行与这一趟的预设合成 [`Request`]。
    ///
    /// 进度观察者不在这里装：那是界面的事，而这个方法答的是「这一趟的参数是什么」。
    /// **参数哈希收的正是这里算出来的这些值**——它由 `Request` 求出（见 `tonefit` 的
    /// `metadata`），而预设的名字一个字都没进来。改了预设的内容而名字没变，
    /// 下一趟因此照样重做。
    fn request(self, preset: &Preset) -> Result<Request> {
        Ok(Request {
            profile: self.target_profile(preset)?,
            fit: self.fit_mode(preset)?,
            crop: self.crop(preset),
            split: self.split_rule(preset)?,
            filter: self.residual_filter(preset)?,
            bit_depth: self.bit_depth_override(preset)?,
            dither: self.dither_override(preset)?,
            per_page: self.per_page(preset),
            cache_budget: self.cache_budget(preset)?,
            mode: self.mode(),
            io_mode: self.io_mode(preset)?,
            metadata: !self.no_metadata,
            progress: None,
            output_root: self.out.expect(REQUIRED_BY_CLAP),
            inputs: self.inputs,
        })
    }
}

/// `--help` 末尾那一节：互锁**逐条列全**（页几何批 05 号票）。
///
/// 这是互锁在文档这一侧的唯一落点。处置 ② 明写着「只进文档与 `--help`」，
/// 而另外两条也不该各自散回自己那个开关的帮助里——互锁说的是**几项凑在一起**之后的事，
/// 挂在任何一项名下都缺了另一半。各开关那一侧留的是指路，不是第二份说法。
///
/// **逐条都列，与各自的处置无关。** 处置说的是咬上之后**这一趟**怎么交代
/// （`tonefit::Voice`），这一节是说明书：拒绝那一条要让人事先躲得开，
/// 一趟不吭声的那一条更是只剩这里说得到。
///
/// 措辞不在这里，在 `tonefit::Interlock`——同一句还要从报告抬头与那条拒绝的错误里出来。
/// 这一层只管排版：一条一行，挂个圆点。
///
/// 只进长帮助（`--help`），不进 `-h`：短帮助一项只印一行，塞不下也不该塞。
/// `--help` 里说「在哪里找卷」那一句。
///
/// 认得的归档扩展名**从库那一份取**（[`tonefit::listed_archive_extensions`]）：
/// 格式集只有一个出处（`source::ARCHIVE_FORMATS`），加一项时这句话与那条拒绝
/// 一起跟着走。抄一串字面量在这里，`.7z` 落地那天帮助里就写着假话——它真发生过。
///
/// 与 [`interlock_help`] 同一条道理：clap 的 `help` 收得下一个运行期算出来的串，
/// 而文档注释收不下。
fn inputs_help() -> String {
    format!(
        "在哪里找卷：一个归档（{}），或一个目录——目录里直接躺着页就是一个卷，         底下的子目录与归档也各自成卷。源只读。",
        tonefit::listed_archive_extensions()
    )
}

fn interlock_help() -> String {
    let mut text = format!("{INTERLOCK_HEADING}\n");
    for interlock in Interlock::ALL {
        text.push_str(&format!("  · {interlock}\n"));
    }
    text
}

/// **长**帮助折到多宽：[`wrap::TERMINAL_WIDTH`] 减去 clap 给 `--help` 那一档缩进（10 格）。
///
/// 折的是原文，缩进由 clap 加在外面，因此要先扣掉。
const LONG_HELP_WIDTH: u16 = wrap::TERMINAL_WIDTH - 10;

/// **短**帮助折到多宽。
///
/// `-h` 把每一项的短帮助摆在**开关那一列后面**，缩进因此比长帮助深得多——它随这份命令行上
/// 最长的那个开关走，眼下是 31 格。clap 不交出这个数，只能照它排出来的帮助**数格子**
/// （与 `docs/measurements.md` 那种实测数字无关，那里装的是图像处理量出来的东西）：
/// [`tests::the_help_folds_every_line_into_the_terminal`] 钉着「`-h` 没有一行过
/// [`wrap::TERMINAL_WIDTH`]」，添一个更长的开关时它当场变红。
const SHORT_HELP_WIDTH: u16 = wrap::TERMINAL_WIDTH - 31;

/// 交给 clap 之前，把每一条帮助按显示宽度折一遍（[`wrap`]）。
///
/// **clap 一行都不折**：折行挂在它的 `wrap_help` 特性后面，本仓库没开。
/// 开了也治不了中文——它按**空格**断，而中文长句里一个空格都没有，整句仍是「一个词」。
/// 落地这一票之前 `--help` 里最长的一行有 412 格、`-h` 有 300 格（停车场 Q32）。
/// 折的是**原文**——clap 认原文里的换行，折好的每一行都短过它的排版，它一行都不必再动。
///
/// 只在真要跑的那一趟折得着：用例问的是 [`Cli::command`] 那一份**原文**
/// （见 [`tests::the_help_lists_every_interlock_in_one_place`]），
/// 折过的文字里 `contains` 一条长句会被折行折断。
fn folded_help(command: clap::Command) -> clap::Command {
    // `about` 是**短**的那一份：它还要摆进上一级那张子命令表里，与短帮助同一档。
    // 长的那三份只印在第 0 列上，长帮助那一档够宽。
    let about = fold_help(command.get_about(), SHORT_HELP_WIDTH);
    let long_about = fold_help(command.get_long_about(), LONG_HELP_WIDTH);
    let after_help = fold_help(command.get_after_help(), LONG_HELP_WIDTH);
    let after_long_help = fold_help(command.get_after_long_help(), LONG_HELP_WIDTH);
    let arguments: Vec<clap::Id> = command
        .get_arguments()
        .map(|argument| argument.get_id().clone())
        .collect();
    let subcommands: Vec<String> = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .collect();

    let mut command = command
        .about(about)
        .long_about(long_about)
        .after_help(after_help)
        .after_long_help(after_long_help);
    for argument in arguments {
        command = command.mut_arg(argument, |argument| {
            let help = fold_help(argument.get_help(), SHORT_HELP_WIDTH);
            let long_help = fold_help(argument.get_long_help(), LONG_HELP_WIDTH);
            argument.help(help).long_help(long_help)
        });
    }
    for subcommand in subcommands {
        command = command.mut_subcommand(subcommand, folded_help);
    }
    command
}

/// 一条帮助折好之后的样子。没有这一条就还是没有——`Reset` 落在本来就空着的那一格上
/// 什么都不改。
fn fold_help(text: Option<&StyledStr>, width: u16) -> Resettable<StyledStr> {
    text.map(|text| StyledStr::from(wrap::fold(&text.to_string(), width).join("\n")))
        .into()
}

/// 《开关互锁》那一节的标题。
///
/// 它自成一个常量，是为了让用例分得开**那一节本体**与各开关帮助里指向它的那句路标——
/// 两者都带着「开关互锁」四个字，而短帮助里该有的只有路标。
const INTERLOCK_HEADING: &str = "开关互锁（几项凑在一起会互相削弱的那几种组合）:";

/// 输出根走到这里就一定有值：`required = true` 挡在 clap 那一层。
///
/// 字段类型仍是 `Option`——`subcommand_negates_reqs` 让子命令那一路不必交出它
/// （`calibrate` 根本不收输出根与卷），字段就得容得下「没有」。
/// 必填这道关因此留在 clap：错在哪、该怎么敲，它说得比这里好。
///
/// **型号不走这一条**：它的必填是有条件的（`required_unless_present = "preset"`），
/// 而「那份预设到底有没有型号」只有读了盘才知道——那一头的说法在
/// [`Cli::no_device_error`]。
const REQUIRED_BY_CLAP: &str = "clap 的 required = true 已经挡在前面";

/// 命令行上没有型号却走到了拼 profile 那一步，只可能是因为点了 `--preset`：
/// `required_unless_present = "preset"` 放行的就是这一种（见 [`Cli::no_device_error`]）。
const NAMED_A_PRESET: &str = "clap 的 required_unless_present = \"preset\" 已经挡在前面";

/// 处理卷之外的那些事，各占一个子命令。
#[derive(clap::Subcommand)]
enum Command {
    /// 生成标定图：一次上机答两件事——像素有没有原样贴上、还分得开几级灰。数出的是感知可分辨级数，不等于面板的物理灰阶数。
    ///
    /// 两件事与那句话都挤在头一行，为的是 `-h` 也说得到：短帮助只印这一行，
    /// 而「这张图不止用来数灰阶」与「数出来的不是规格表上那个数」正是最容易漏掉的两条。
    ///
    /// 图按目标面板的分辨率排布，判读说明中英两份都印在图内——图拷进设备就能用，
    /// 不必对着文档看。上半是像素完整性：抖动块与**同均值**实心块并置、1 像素周期光栅四种、
    /// 四角压在第 0 行列与末行列上的直角标记。下半是并排的各候选位深阶梯，每一级标着自己的号。
    ///
    /// 用法：把图拷进设备，以**原尺寸**打开（关掉缩放、适配屏幕与白边裁切）。
    ///
    /// **一、先看像素完整性。** 每一对方块左边抖动、右边实心，两边的均值严格相等：
    /// 分得开就说明像素原样贴上了，糊成一片就说明阅读器自己重采样过一遍。
    /// 光栅要呈细密纹理而不是平灰，四角标记要四个都在（少一个说明加了边距或裁了图）。
    ///
    /// **这一件不过就别数灰阶**——阶梯那时也被重采样过，数出来的不是面板能显示的级数。
    /// 抖动在那种阅读器上做了等于没做，而 tonefit 探不到它：阅读器的显示管线在视野之外
    /// （ADR 0007）。用户只有这张图能知道自己该关哪个开关。
    ///
    /// **二、再数最右那条阶梯**里你还分得开几级——它最细，其余几条只作对照；
    /// 把那个数回填给 `--gray-levels`（ADR 0003：面板灰阶数是位深的硬上界，
    /// 它在判据之前裁掉候选位深）。
    ///
    /// **数出来的是感知可分辨级数，不等于面板的物理灰阶数。** 两者不必相等——
    /// 显示固件的处理、环境光、观看距离都会改变你数得出几级，而 `--gray-levels`
    /// 填的正是前者：判定要贴合你**实际看到**的效果，不是贴合规格表。
    ///
    /// 标定图本身不经过位深判定：它是量具，不是被处理的页。像素以 8 位工作精度画出，
    /// 无损写出，不带自描述元数据。
    Calibrate {
        /// 目标设备型号。与处理卷时同一张内置表，型号名不区分大小写与分隔符。
        #[arg(short, long, value_name = "型号")]
        profile: String,

        /// 覆盖面板灰阶数。上一趟数出来的级数填这里，图跟着只排这台设备真会用到的那几档。
        #[arg(long, value_name = "级数")]
        gray_levels: Option<u32>,

        /// 标定图写到哪个文件。父目录不在就建出来。
        #[arg(short, long, value_name = "文件")]
        out: PathBuf,
    },
}

/// 把型号名与各覆盖项合成一个 profile。
///
/// 处理卷与 `calibrate` 共用它：两边解析出的必须是同一块面板，
/// 不然标定图量的是一块、判定用的是另一块。
///
/// 阈值是这里唯一一项 `calibrate` 用不上的：标定图是量具，不经判定（它恒传 `None`）。
fn target_profile(
    device: &str,
    gray_levels: Option<u32>,
    threshold: Option<f32>,
) -> Result<Profile> {
    let mut profile = Profile::resolve(device)?;
    if let Some(gray_levels) = gray_levels {
        profile = profile.with_gray_levels(gray_levels)?;
    }
    if let Some(threshold) = threshold {
        profile = profile.with_threshold(threshold)?;
    }
    Ok(profile)
}

/// 有卷被隔离时的退出码。
///
/// 与「拒绝执行」分成两个数（12 号票：退出码要分得开「全部成功」与「有卷被隔离」）：
/// 隔离过的那一趟**做完了**——输出齐着、报告齐着，只是其中几卷带着坏页。
/// 脚本据此可以选择忽略，也可以停下来看一眼，而 `1` 只说得出「这一趟没做成」。
const ISOLATED_EXIT: u8 = 2;

/// 全部成功的退出码。
const SUCCESS_EXIT: u8 = 0;

/// 有卷**没做成**时的退出码（05 号票：卷级失败）。
///
/// 与 [`ISOLATED_EXIT`] 分成两个数，理由与当初把它从「拒绝执行」里分出来的是同一条：
/// 脚本要分得开。隔离过的那一卷**交出来了**——输出齐着、页数齐着，只是其中几页是占位页，
/// 修好坏页重跑一趟就好；没做成的那一卷**一个字节都没交出来**，盘上根本没有它，
/// 该去查的是文件还在不在、盘还挂着没有、权限变没变。
///
/// 它压过 `ISOLATED_EXIT`：两件事同时成立时报更重的那一件（本票的验收）。
/// 一个数只说得出一件事，而「有卷根本没做成」是那两件里更该让人停下来看的。
const FAILED_VOLUME_EXIT: u8 = 3;

/// **这一趟没做成**时的退出码：拒绝执行，以及别的任何一种当场返回 `Err` 的收场
/// （`CONTEXT.md` 的《失败》）。
///
/// 它就是 `ExitCode::FAILURE` 那个数，写成常量是为了让会话那一路取得到它——
/// 会话在拒绝执行之后**不退出**（把话画出来当场改，见 spec 的《卷级失败与退出码》），
/// 那个 `1` 因此要等到用户退出会话时才交出去，而那时早已不在这个 `match` 里了。
const REFUSED_EXIT: u8 = 1;

fn main() -> ExitCode {
    match execute() {
        Ok(code) => ExitCode::from(code),
        // `Result<()>` 那个 main 印的就是这一行，退出码 1。自己拿 `ExitCode` 之后照印。
        //
        // **这一处不折行，因此得自己把标注换回来**（[`wrap::printed`]）：拒绝那句话里
        // 劝人换的那条命令带着[不许断的空格](wrap::HARD_SPACE)，原样落到 stderr 上，
        // 用户照着抄一遍 clap 就认不出那个开关。折行那几处由 `wrap::fold` 顺手做了，
        // 这一路一格都没折——报告与帮助折到多宽有出处，一条错误没有。
        Err(error) => {
            eprintln!("Error: {}", wrap::printed(&format!("{error:?}")));
            ExitCode::from(REFUSED_EXIT)
        }
    }
}

/// 这一趟的退出码：全部成功是 [`SUCCESS_EXIT`]，有卷没做成是 [`FAILED_VOLUME_EXIT`]，
/// 有卷被隔离是 [`ISOLATED_EXIT`]。
///
/// 拒绝执行那一种走不到这里——它在 `run` 里就返回了 `Err`，退出码是 `ExitCode::FAILURE`（1）。
/// 四个数因此各说一件事：做完了、做完了但有卷带着坏页、做完了但有卷根本没做成、
/// 这一趟没做成（`CONTEXT.md` 的《失败》）。
///
/// **次序就是取舍**：一个进程只交得出一个数，两件事同时成立时报更重的那一件
/// （05 号票的验收）。为什么那两件是两个不同的决定，见 [`FAILED_VOLUME_EXIT`]。
///
/// 按停停下来的那一趟**不在这里露面**：它是用户自己的决定，不是失败，退出码照旧
/// （`Report::outcome` 说得出它，见 `tonefit::RunOutcome`）。命令行这一路眼下也按不出来。
///
/// 出的是 `u8` 而不是 `ExitCode`：后者不可比较，这条规则也就测不了，
/// 而「退出码分得开这几种」正是本票要钉住的那一条。
fn exit_code(report: &Report) -> u8 {
    if report.any_volume_failed() {
        FAILED_VOLUME_EXIT
    } else if report.any_isolated() {
        ISOLATED_EXIT
    } else {
        SUCCESS_EXIT
    }
}

/// 不带任何参数敲 `tonefit` 时走的那一条：**会话**。
///
/// 它排在 [`Cli::parse`] **之前**，这是本票唯一一处动到入口的地方。
/// 为什么非得在 clap 之前：`--out`、`--profile`、卷三项必填，无参数交到 clap 手上
/// 就是一条「缺了什么」的错误，而那正是要拦下的那一趟。反过来，
/// **带参数的命令行一个字都没改**——参数只要有一个，这里立刻让路
/// （`-p` 因此不必放宽必填，见 [`REQUIRED_BY_CLAP`]）。
///
/// 「有没有参数」问的是 `args_os`：程序名之外一个都没有才算数。
/// 不问 clap，因为要在它开口之前就分岔。
///
/// 关掉 `tui` 特性时它恒不接手，无参数照旧落到 clap 的必填项错误上
/// （spec 的《依赖》：库使用者 `default-features = false` 即可甩掉终端库）。
#[cfg(feature = "tui")]
fn without_arguments() -> Option<Result<u8>> {
    (std::env::args_os().len() <= 1).then(session::enter)
}

#[cfg(not(feature = "tui"))]
fn without_arguments() -> Option<Result<u8>> {
    None
}

fn execute() -> Result<u8> {
    if let Some(session) = without_arguments() {
        return session;
    }
    let cli = Cli::from_arg_matches(&folded_help(Cli::command()).get_matches())
        .unwrap_or_else(|error| error.exit());
    if let Some(Command::Calibrate {
        profile,
        gray_levels,
        out,
    }) = &cli.command
    {
        return calibrate(profile, *gray_levels, out);
    }
    // 预设先读：它供得出型号，而下面每一项都可能落到它身上。**不点名就一个字节都不读盘。**
    let preset = cli.preset()?;
    let bar = Bar::new(cli.inputs.len());
    let mut request = cli.request(&preset)?;
    let mode = request.mode;
    request.progress = Some(ProgressSink::new(bar));
    let report = tonefit::run(&request)?;
    // 印出去之前折一遍行。措辞归 [`render`]，**印在多宽的地方上归这里**：
    // 报告里那几句长的（末尾几小结、互锁那几行）一句里一个空格都没有，
    // 不折就是一行几百格（见 [`wrap`]）。
    print!(
        "{}",
        wrap::folded_text(&render::plain::report(&report, mode), wrap::TERMINAL_WIDTH)
    );
    Ok(exit_code(&report))
}

/// 把命令行点名的那台设备与去处交给库里的出图入口（14 号票、加固批 12 号票）。
///
/// 这一趟不读源、不写输出根、不判定任何东西：标定图是量具，管线一整套都不在场。
/// 因此也没有「有卷被隔离」那种结局——写成了就是 [`SUCCESS_EXIT`]，写不成是 `Err`。
///
/// 出图整件事在 [`tonefit::write_calibration_chart`] 里。这一层剩下两件命令行自己的事：
/// 把型号名与 `--gray-levels` 合成 profile，以及印出[那几行文案](render::calibration_note)。
fn calibrate(device: &str, gray_levels: Option<u32>, out: &Path) -> Result<u8> {
    let profile = target_profile(device, gray_levels, None)?;
    tonefit::write_calibration_chart(&profile, out)?;
    print!("{}", render::calibration_note(&profile, out));
    Ok(SUCCESS_EXIT)
}

/// 进度条：把管线报到的那些步画出来（spec 的 story 30）。
///
/// 画在 **stderr** 上——报告走 stdout，`tonefit … > 报告.txt` 那种用法下进度条不该混进文件里。
/// indicatif 自己认得出对面不是终端，那时它一个字节都不写，管道与用例因此干净
/// （见 `ProgressDrawTarget::term`）。
///
/// 屏上最多三行，按这一趟走到哪儿依次登场：**预扫**那一条转轮、**整趟**那一条、
/// **当前卷**那一条。一卷走完抹掉那一卷的，整趟走完全抹掉——几十卷跑下来，
/// 屏幕上留下的只有报告。留着一排走完的进度条是另一种噪声，
/// 而「这一趟做了什么」报告说得更清楚。
struct Bar {
    /// 几条一起画。两条 `ProgressBar` 各画各的会互相抹掉，`MultiProgress` 是 indicatif
    /// 那一侧把它们排成上下几行的办法。
    frame: MultiProgress,
    /// 开工之前那一段的转轮（03 号票）。
    ///
    /// 它在 `run` **之前**就起来了，`RunStarted` 一到就收掉：预扫排在那条事件之前
    /// （见 `tonefit::Event::RunStarted`），因此库这一侧报不出「预扫开始了」——
    /// 那一段的交代只能由这一层自己给。几十个归档卷在慢盘上要列一阵归档头，
    /// 而静默的空白与卡死在屏幕上分不开。
    ///
    /// 它盖住的**不止预扫**：开工前那几道检查（**拒绝执行**里**预扫之外、要摸文件系统**
    /// 的那几道，单子在 `CONTEXT.md` 的《失败》）也在这条事件之前。措辞因此把两件事都说了
    /// ——只说「预扫」的话，被那几道检查拒掉的那一趟屏上就写着一件根本没发生的事。
    survey: Mutex<Option<ProgressBar>>,
    /// 整趟那一条：全局进度与**剩余时间**，长任务里唯一有人真想知道的数（ADR 0011）。
    global: Mutex<Option<ProgressBar>>,
    /// 当下这一卷的那一条。卷与卷之间是空的。
    ///
    /// 每卷新起一条而不是复用同一条：走完的那一条已经收了尾，再往它身上设长度、加位置，
    /// indicatif 那一侧不保证还画得出来。
    volume: Mutex<Option<ProgressBar>>,
}

/// 一条横条的样子：名字、格子、走了几步、已用多久、还剩多久。
///
/// 整趟那条与当前卷那条共用它——两条长得一样，靠 `{msg}` 分辨是哪一条。
/// 模板编不出来时退回 indicatif 的默认样式：进度条画得难看不该让这一趟跑不成。
fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template(&format!(
        "{{msg}} [{{bar:{BAR_WIDTH}}}] {{pos}}/{{len}} 步 · 已用 {{elapsed}} · 剩 {{eta}}"
    ))
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("=> ")
}

/// 一条横条画多宽。**命令行与会话共用这一个数**：两处的横条长得一样，
/// 读的人不必重新认一遍（会话那一份见 `session::draw`）。
const BAR_WIDTH: usize = 30;

impl Bar {
    /// 起一份进度显示，预扫那条转轮当场登场。
    ///
    /// `named` 是这一趟点名了几个**路径**——不是几个卷：一个路径底下有几个卷，
    /// 要等预扫发现完才知道（ADR 0014）。它取自命令行而不是等库来报：
    /// 转轮要在 `run` 之前起来，而那时一条事件都还没有。
    fn new(named: usize) -> Self {
        let frame = MultiProgress::new();
        let survey = frame.add(ProgressBar::new_spinner());
        survey.set_message(format!("点名 {named} 个路径：发现卷、预扫成员……"));
        // 转轮自己转起来：预扫期间这一层收不到任何事件，没有人来推它。
        survey.enable_steady_tick(Duration::from_millis(120));
        Self {
            frame,
            survey: Mutex::new(Some(survey)),
            global: Mutex::new(None),
            volume: Mutex::new(None),
        }
    }

    /// 中毒了照样用：里面是一个画横条的句柄，一条线程恐慌不该让进度显示从此哑掉
    /// （与 `tonefit` 那一侧锁缓存同一条规矩）。
    fn held(slot: &Mutex<Option<ProgressBar>>) -> MutexGuard<'_, Option<ProgressBar>> {
        slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn current(&self) -> MutexGuard<'_, Option<ProgressBar>> {
        Self::held(&self.volume)
    }

    /// 预扫完了：收掉转轮，把整趟那一条摆上。
    fn start_run(&self, volumes: usize, steps: u64) {
        if let Some(spinner) = Self::held(&self.survey).take() {
            spinner.finish_and_clear();
        }
        let bar = self.frame.add(ProgressBar::new(steps));
        bar.set_style(bar_style());
        bar.set_message(format!("整趟 {volumes} 卷"));
        *Self::held(&self.global) = Some(bar);
    }

    /// 起一条新的：这一卷叫什么、这一趟最多走多少步。
    fn start(&self, volume: &Path, steps: u64) {
        let bar = self.frame.add(ProgressBar::new(steps));
        bar.set_style(bar_style());
        // 卷名怎么取只有一处（`render::volume_name`）：会话的当前卷条印的是同一个。
        bar.set_message(render::volume_name(volume));
        *self.current() = Some(bar);
    }

    /// 又走完一步：当前卷那条与整趟那条各进一格。
    ///
    /// 两把锁**依次取、不嵌套**：这个方法是从计算线程上被调到的，嵌套持有两把锁就得开始
    /// 讲取锁次序，而这里根本不需要同时持有。
    fn step(&self) {
        if let Some(bar) = self.current().as_ref() {
            bar.inc(1);
        }
        if let Some(bar) = Self::held(&self.global).as_ref() {
            bar.inc(1);
        }
    }

    /// 一卷收摊：抹掉那一条，并把它**预告了却没走**的那几步结清到整趟那一条上。
    ///
    /// 为什么非结清不可，见 `tonefit::Event::RunStarted` 的 `steps`——那一条要求每个
    /// 画全局进度的实现方都这么做。这里只说**怎么**做：差额从那一卷自己那条横条上读，
    /// 它的长度是预告、位置是真走过的步数，两个数都已经在手上。
    fn finish_volume(&self) {
        let Some(bar) = self.current().take() else {
            return;
        };
        let left = bar.length().unwrap_or(0).saturating_sub(bar.position());
        bar.finish_and_clear();
        if let Some(global) = Self::held(&self.global).as_ref() {
            global.inc(left);
        }
    }

    /// 每一条都收掉，一行不留。
    ///
    /// 也挂在 [`Drop`] 上：**开工之前**被拒的那一趟（预扫发现坏路径就是其中一种）
    /// 一条事件都不发，收场那一条因此根本不来，而屏上那条转轮还转着。
    ///
    /// 开工**之后**才撞上的那一种拒绝照发收场（见 `tonefit::Event::RunFinished`），
    /// 这里因此会被调到两次——`take()` 让第二次是空操作。
    fn clear(&self) {
        for slot in [&self.survey, &self.global, &self.volume] {
            if let Some(bar) = Self::held(slot).take() {
                bar.finish_and_clear();
            }
        }
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Progress for Bar {
    /// 事件流里它认六条：开工摆上整趟那条、开卷起一条、走一步两条各进一格、
    /// 一卷收摊结清差额、一卷**没做成**同样收摊结清、收场全抹掉。
    ///
    /// 没做成那一条与跑完那一条在这一层做同一件事，而两条都得接：一卷开过头就一定要收摊，
    /// 不然那条横条留在屏上不走，整趟那条也少了它预告的那几步、永远走不到头
    /// （见 `tonefit::Event::VolumeFailed`）。
    ///
    /// 其余的事件命令行这一路当下没有去处——「在走哪一遍」与「哪一页失败了」
    /// 报告里说得更全。`_` 那一支不是遗漏：[`Event`] 非穷尽，多一个变体不该逼着这里跟着改
    /// （ADR 0011 的《后果》）。
    ///
    /// 回的恒是[继续](Instruction::Continue)：两级停要有人按，而命令行这一路
    /// 还没有那个键——它是会话那一头的事（ADR 0013 决定第 3 条说命令行同样用得上，
    /// 接线留给按停那几张票）。
    fn observe(&self, event: Event<'_>) -> Instruction {
        match event {
            Event::RunStarted { volumes, steps, .. } => self.start_run(volumes, steps),
            Event::VolumeStarted { volume, steps, .. } => self.start(volume, steps),
            Event::Stepped { .. } => self.step(),
            Event::VolumeFinished { .. } | Event::VolumeFailed { .. } => self.finish_volume(),
            Event::RunFinished { .. } => self.clear(),
            _ => {}
        }
        Instruction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_wired_up() {
        Cli::command().debug_assert();
    }

    /// 没点名 `--preset` 那一趟拿到的东西：每一项都是「没说」。
    ///
    /// 各项默认值的用例都拿它——它们问的是「命令行不点名时落到哪里」，
    /// 而预设不点名正是那个前提的另一半。
    fn no_preset() -> Preset {
        Preset::default()
    }

    /// 一条完整的处理卷命令行，卷与输出根固定，其余由调用方补。
    fn parse(arguments: &[&str]) -> Cli {
        let mut line = vec!["tonefit", "--out", "out"];
        line.extend_from_slice(arguments);
        line.push("volume-a");
        Cli::try_parse_from(line).expect("参数应当可解析")
    }

    /// 这一条命令行加这一份预设，合出来的 [`Request`] 长什么样。
    ///
    /// 比的是**整份 `Request` 的 `Debug`**，不是挑几个字段：参数哈希收的是这里头
    /// 会改变输出的每一项（`tonefit` 的 `metadata`），挑着比就等于自己重列一遍那张单子，
    /// 而漏掉一项恰恰是这批用例要防的事。观察者两边都是 `None`，比得起来。
    fn request_line(arguments: &[&str], preset: &Preset) -> String {
        format!(
            "{:?}",
            parse(arguments).request(preset).expect("合得出 Request")
        )
    }

    /// 一份把每一项都说满的预设，连同把它逐项敲出来的那条命令行。
    ///
    /// 两者是同一套值的两种写法——本票的验收「效果与把那几个 flag 逐个敲出来完全一致」
    /// 说的就是它们该合出同一个 `Request`。
    fn every_field() -> (Preset, Vec<&'static str>) {
        let text = "\
[preset.\"漫画\".device]
profile = \"boox-poke6\"
gray-levels = 12
threshold = 4.75

[preset.\"漫画\".taste]
fit = \"inside\"
crop = false
split = false
split-threshold = 1.75
reading-order = \"ltr\"
filter = \"hamming\"
bit-depth = 2
dither = \"fs\"
per-page = true
cache-budget = \"1G\"
io-mode = \"concurrent\"
";
        let flags = vec![
            "--profile",
            "boox-poke6",
            "--gray-levels",
            "12",
            "--threshold",
            "4.75",
            "--fit",
            "inside",
            "--no-crop",
            "--no-split",
            "--split-threshold",
            "1.75",
            "--reading-order",
            "ltr",
            "--filter",
            "hamming",
            "--bit-depth",
            "2",
            "--dither",
            "fs",
            "--per-page",
            "--cache-budget",
            "1G",
            "--io-mode",
            "concurrent",
        ];
        (preset::read(text, "漫画").expect("读得懂"), flags)
    }

    /// **套一份预设，与把那几个 flag 逐个敲出来，合出同一个 `Request`**（07 号票的验收）。
    ///
    /// 这一条同时是「参数哈希收的是展开后的值」那一条的一半：哈希由 `Request` 求出，
    /// 而两条路交出的是同一个 `Request`，预设的名字一个字都没进去。另一半是
    /// [`changing_what_a_preset_says_changes_the_run`]——改了内容就换一个 `Request`，
    /// 于是换一个哈希、下一趟重做（那一步在 `tests/idempotency.rs` 的
    /// `a_changed_parameter_redoes_the_volume` 上）。
    #[test]
    fn a_preset_expands_to_the_same_run_as_typing_the_flags_out() {
        let (preset, flags) = every_field();

        assert_eq!(
            request_line(&["--preset", "漫画"], &preset),
            request_line(&flags, &no_preset()),
            "套预设与逐个敲 flag 合出来的不是同一趟"
        );
    }

    /// **预设点的阈值与命令行点的印出同一句话**（`p2-loose-ends/10` 的验收：
    /// 三个入口下印出来的话都对）。
    ///
    /// 第三个入口（会话）由 `session::state` 的
    /// `the_threshold_row_prints_the_source_the_report_prints` 问同一件事。
    /// 三处共用 [`Profile::with_threshold`]，来源因此只有一种——那一句不提入口
    /// （`p1-session/12` 判的：来源分的是这个数怎么定出来的，不是从哪个入口点的名）。
    #[test]
    fn a_threshold_from_a_preset_says_what_the_command_line_says() {
        let (preset, _) = every_field();

        let from_preset = parse(&["--preset", "漫画"])
            .target_profile(&preset)
            .expect("合得出 profile");
        let from_flags = parse(&["--profile", "boox-poke6", "--threshold", "4.75"])
            .target_profile(&no_preset())
            .expect("合得出 profile");

        assert_eq!(
            from_preset.threshold().to_string(),
            from_flags.threshold().to_string(),
            "同一个数，两个入口印出两句话"
        );
        assert!(
            from_preset.threshold().to_string().contains("点名指定"),
            "{}",
            from_preset.threshold()
        );
    }

    /// 改了预设的内容而名字没变，这一趟的 `Request` 就跟着变。
    ///
    /// 与上一条合起来钉住「参数哈希收的是**展开后的值**」：名字进不去哈希，内容进得去。
    #[test]
    fn changing_what_a_preset_says_changes_the_run() {
        let before = preset::read("[preset.\"漫画\".taste]\nfilter = \"hamming\"\n", "漫画")
            .expect("读得懂");
        let after = preset::read("[preset.\"漫画\".taste]\nfilter = \"bicubic\"\n", "漫画")
            .expect("读得懂");

        assert_ne!(
            request_line(&["--preset", "漫画", "--profile", "kobo-libra-2"], &before),
            request_line(&["--preset", "漫画", "--profile", "kobo-libra-2"], &after),
            "预设的内容换了，这一趟却一模一样"
        );
    }

    /// 显式 flag 与预设撞上时**命令行赢**（07 号票的验收）。
    ///
    /// 逐项都验：每一项各被预设与命令行各说一次，合出来的必须与只敲命令行那一趟相同。
    #[test]
    fn an_explicit_flag_beats_the_preset() {
        let (preset, _) = every_field();
        // 与预设里那一份处处不同的另一套值。
        let flags = [
            "--profile",
            "kobo-libra-2",
            "--gray-levels",
            "8",
            "--threshold",
            "6.25",
            "--fit",
            "height",
            "--split-threshold",
            "2.5",
            "--reading-order",
            "rtl",
            "--filter",
            "bicubic",
            "--bit-depth",
            "4",
            "--dither",
            "off",
            "--cache-budget",
            "64M",
            "--io-mode",
            "serial",
        ];
        let mut with_preset = vec!["--preset", "漫画"];
        with_preset.extend_from_slice(&flags);

        // 裁边、拆分、逐页三项不在这里：命令行只说得出一个方向，而预设把前两项已经关到了
        // 命令行说得出的那一侧、把第三项开到了那一侧——「命令行赢」在它们身上无从分辨。
        // 单向那条规矩由 `the_switches_that_only_say_one_thing_stay_one_way` 钉。
        let mut typed_out = flags.to_vec();
        typed_out.extend(["--no-crop", "--no-split", "--per-page"]);

        assert_eq!(
            request_line(&with_preset, &preset),
            request_line(&typed_out, &no_preset()),
            "预设盖过了命令行上显式点到的那一项"
        );
    }

    /// 三个只说得出一个方向的开关：预设关得掉，命令行关不回来。
    ///
    /// 钉的是那条**单向**的规矩本身（见 `Cli` 那个 `impl` 的抬头）——它是有意的取舍，
    /// 不是漏了三个 flag，因此得有一行说得出它当下是什么样。
    #[test]
    fn the_switches_that_only_say_one_thing_stay_one_way() {
        let off = preset::read(
            "[preset.\"漫画\".taste]\ncrop = false\nsplit = false\nper-page = true\n",
            "漫画",
        )
        .expect("读得懂");
        let on = preset::read(
            "[preset.\"漫画\".taste]\ncrop = true\nsplit = true\nper-page = false\n",
            "漫画",
        )
        .expect("读得懂");
        let line = ["--preset", "漫画", "--profile", "kobo-libra-2"];

        // 预设关得掉，命令行上没有把它开回来的写法。
        let cli = parse(&line);
        assert!(!cli.crop(&off), "预设关不掉裁边");
        assert!(!cli.split_rule(&off).expect("合得出").on, "预设关不掉拆分");
        assert!(cli.per_page(&off), "预设开不了逐页");

        // 反过来，命令行说得出的那一个方向压得过预设。
        let mut relented = line.to_vec();
        relented.extend(["--no-crop", "--no-split", "--per-page"]);
        let cli = parse(&relented);
        assert!(!cli.crop(&on), "--no-crop 没压过预设");
        assert!(
            !cli.split_rule(&on).expect("合得出").on,
            "--no-split 没压过预设"
        );
        assert!(cli.per_page(&on), "--per-page 没压过预设");
    }

    /// 不点名 `--preset` 时命令行行为一字不变（07 号票的验收）。
    ///
    /// 钉的是「空预设 == 从前那一趟」：每一项都落到默认值上，一项都不落到预设上。
    #[test]
    fn a_run_that_names_no_preset_is_the_run_it_always_was() {
        let plain = parse(&["--profile", "kobo-libra-2"]);
        let preset = plain.preset().expect("不点名不读盘，因此不会失败");

        assert_eq!(preset, no_preset(), "不点名却拿到了一份有内容的预设");
        assert_eq!(
            request_line(&["--profile", "kobo-libra-2"], &preset),
            request_line(&["--profile", "kobo-libra-2"], &no_preset())
        );
    }

    /// `--preset` 供了型号时 `-p` 不再必填；其余情况必填照旧（07 号票的验收）。
    #[test]
    fn a_preset_can_stand_in_for_the_required_profile() {
        // clap 那一层：点了 `--preset` 就放行，没点就照旧拦下。
        assert!(
            Cli::try_parse_from(["tonefit", "--out", "out", "--preset", "漫画", "volume-a"])
                .is_ok(),
            "点了 --preset 仍被 clap 拦下"
        );
        let missing = match Cli::try_parse_from(["tonefit", "--out", "out", "volume-a"]) {
            Ok(_) => panic!("没点 --preset 也没点 --profile，该被 clap 拦下"),
            Err(error) => error.to_string(),
        };
        assert!(missing.contains("--profile"), "{missing}");

        // 放行之后由预设自己交出型号。
        let supplies = preset::read(
            "[preset.\"漫画\".device]\nprofile = \"boox-poke6\"\n",
            "漫画",
        )
        .expect("读得懂");
        let profile = parse(&["--preset", "漫画"])
            .target_profile(&supplies)
            .expect("预设供得出型号");
        assert_eq!(profile.device(), "boox-poke6");

        // 预设没有型号那一种：clap 放行了，这一层得说得出话来，不能崩。
        let silent =
            preset::read("[preset.\"漫画\".taste]\nfit = \"inside\"\n", "漫画").expect("读得懂");
        let error = parse(&["--preset", "漫画"])
            .target_profile(&silent)
            .expect_err("两处都没有型号")
            .to_string();
        assert!(
            error.contains("漫画") && error.contains("--profile"),
            "{error}"
        );
    }

    /// 全局条走到哪儿了。用例问的就是它——`ProgressBar` 记着位置，画不画得出来是另一回事。
    fn global_position(bar: &Bar) -> (u64, u64) {
        let held = Bar::held(&bar.global);
        let global = held.as_ref().expect("整趟那一条该在场");
        (global.position(), global.length().unwrap_or(0))
    }

    /// 开工之前那一段有一条转轮顶着，开工那条事件一到就换成整趟那一条（03 号票）。
    ///
    /// 钉的是「那一段不是静默的空白」：预扫在慢盘上要花时间，而它排在开工事件之前，
    /// 库这一侧一句话都说不出来——屏上有没有东西，全看这一层在 `run` 之前有没有先摆一条。
    #[test]
    fn something_is_on_screen_before_the_run_even_starts() {
        let bar = Bar::new(3);
        assert!(
            Bar::held(&bar.survey).is_some(),
            "`run` 还没开始，屏上一条都没有"
        );

        bar.start_run(3, 100);
        assert!(
            Bar::held(&bar.survey).is_none(),
            "开工了，开工前那条转轮还留着"
        );
        assert!(
            Bar::held(&bar.global).is_some(),
            "开工了，整趟那一条没摆上来"
        );
    }

    /// 一卷提前收摊，全局条**结清**它预告剩下的那几步（03 号票，见 [`Bar::finish_volume`]）。
    ///
    /// 钉的是那笔算术的结果：无论哪一卷走了几步，全局条最终恰好收在预告的总数上。
    ///
    /// 调的是这一层自己那几个方法，不是喂一串事件：[`Event`] 的每一个变体都非穷尽，
    /// 库外根本造不出一条来（ADR 0011 的《后果》）。`observe` 因此只是一层分派，
    /// 真的算术在这几个方法里，而这条用例问的正是算术。管线报得对不对是另一侧的事，
    /// 在 `tests/concurrency.rs`。
    #[test]
    fn a_volume_that_stops_early_settles_up_on_the_global_bar() {
        let bar = Bar::new(2);
        bar.start_run(2, 100);

        // 头一个卷预告 70 步，只走了 5 步就收摊——幂等命中就是这个样子。
        bar.start(Path::new("卷一"), 70);
        for _ in 0..5 {
            bar.step();
        }
        assert_eq!(global_position(&bar).0, 5, "走过的步没进全局条");
        bar.finish_volume();
        assert_eq!(
            global_position(&bar).0,
            70,
            "提前收摊的卷没有把预告剩下的步结清"
        );

        // 第二个卷预告 30 步，一步不差地走完：全局条恰好收在预告的总数上。
        bar.start(Path::new("卷二"), 30);
        for _ in 0..30 {
            bar.step();
        }
        bar.finish_volume();
        assert_eq!(global_position(&bar), (100, 100), "全局条没有走到头");
    }

    /// `calibrate` 点名一台设备与一个去处，此外什么都不要（14 号票）。
    ///
    /// 处理卷要的那一整排参数它一个都不收——标定图不读源、不落进输出根，
    /// 缩放、位深、抖动、缓存、并发在它身上一件都不发生。
    #[test]
    fn the_calibrate_subcommand_names_a_device_and_where_to_write_the_chart() {
        let cli = Cli::try_parse_from([
            "tonefit",
            "calibrate",
            "--profile",
            "Kobo Libra 2",
            "--out",
            "标定图.png",
        ])
        .expect("参数应当可解析");

        let Some(Command::Calibrate {
            profile,
            gray_levels,
            out,
        }) = &cli.command
        else {
            panic!("没解析成 calibrate 子命令");
        };
        assert_eq!(
            target_profile(profile, *gray_levels, None)
                .expect("内置型号")
                .device(),
            "kobo-libra-2"
        );
        assert_eq!(out, &PathBuf::from("标定图.png"));

        // 不点子命令仍是「处理点名的若干卷」那一件事：子命令没有把默认用法顶掉。
        let plain =
            Cli::try_parse_from(["tonefit", "--out", "out", "--profile", "kobo-libra-2", "卷"])
                .expect("参数应当可解析");
        assert!(plain.command.is_none());
    }

    /// 处理卷那一路的必填项一项都没松：`--out`、`--profile`、卷，缺一样都不许往下走。
    ///
    /// 这三项的字段类型是 `Option`，为的是让 `calibrate` 那一路不必交出它们
    /// （见 [`REQUIRED_BY_CLAP`]）。类型放松了，**必填这道关不许跟着放松**——
    /// 松掉的话，敲错的命令会一路走到 `expect` 上恐慌，而不是被 clap 拦下并告诉用户缺了什么。
    #[test]
    fn the_volume_side_still_demands_an_output_root_a_profile_and_a_volume() {
        for line in [
            vec!["tonefit", "--profile", "kobo-libra-2", "卷"],
            vec!["tonefit", "--out", "out", "卷"],
            vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"],
        ] {
            let kind = Cli::try_parse_from(&line)
                .map(|_| ())
                .map_err(|error| error.kind());
            assert_eq!(
                kind,
                Err(clap::error::ErrorKind::MissingRequiredArgument),
                "{line:?} 不该解析得出来"
            );
        }
    }

    /// 数出来的级数回填给 `--gray-levels`，标定图跟着只排这台设备真会用到的那几档
    /// （ADR 0003：面板灰阶数是位深的硬上界）。
    #[test]
    fn gray_levels_on_calibrate_fold_into_the_profile_the_chart_is_drawn_for() {
        let cli = Cli::try_parse_from([
            "tonefit",
            "calibrate",
            "-p",
            "kobo-libra-2",
            "--gray-levels",
            "4",
            "-o",
            "chart.png",
        ])
        .expect("参数应当可解析");

        let Some(Command::Calibrate {
            profile,
            gray_levels,
            ..
        }) = &cli.command
        else {
            panic!("没解析成 calibrate 子命令");
        };

        assert_eq!(
            target_profile(profile, *gray_levels, None)
                .expect("内置型号")
                .panel()
                .gray_levels,
            4
        );
    }

    /// 短帮助必须说得出**这张图现在回答两件事**（标定图批 01 号票），
    /// 以及数出的是**感知可分辨级数**、不等于面板的物理灰阶数（14 号票的最后一条；
    /// ADR 0003 的《后果》）。
    ///
    /// 两条都往头一行挤，是因为短帮助只印那一行，而用户多半只敲 `-h`。
    /// 少了前一条，这张图会被当成只用来数灰阶的，像素完整性那一半就没人看；
    /// 少了后一条，用户会拿数出的 12 去质疑「厂商标的明明是 16」，然后什么都不填。
    #[test]
    fn the_calibrate_help_says_the_chart_answers_two_things_and_what_the_count_means() {
        let mut command = Cli::command();
        let calibrate = command
            .find_subcommand_mut("calibrate")
            .expect("calibrate 子命令");

        // `-h` 与 `--help` 两份都要说到：短的那份只印头一行，而用户多半只敲 `-h`。
        for help in [
            calibrate.render_help().to_string(),
            calibrate.render_long_help().to_string(),
        ] {
            assert!(help.contains("两件事"), "{help}");
            assert!(help.contains("原样贴上"), "{help}");
            assert!(help.contains("感知可分辨级数"), "{help}");
            assert!(help.contains("不等于面板的物理灰阶数"), "{help}");
        }

        let long = calibrate.render_long_help().to_string();
        assert!(long.contains("--gray-levels"), "{long}");
        // 数哪一条要点名：几条阶梯就有几个数。
        assert!(long.contains("最右"), "{long}");
        // 两件事的**先后**：第一件不过，第二件数出来的不是这块面板。
        assert!(long.contains("先看像素完整性"), "{long}");
        assert!(long.contains("不过就别数灰阶"), "{long}");
        // 子命令在总帮助里露得出来，不然没人找得到它。
        assert!(
            Cli::command()
                .render_long_help()
                .to_string()
                .contains("calibrate"),
            "总帮助里没有 calibrate"
        );
    }

    /// `calibrate` 这一层只剩两件事：把型号名与 `--gray-levels` 合成 profile 交给库出图，
    /// 以及写成了给出 [`SUCCESS_EXIT`]（14 号票、加固批 12 号票）。
    ///
    /// 断言照这两件事写。**不比「文件里的字节 == 库出的字节」**——落盘整个在库里，
    /// 两边跑的是同一段代码，那个等号恒成立，钉不住任何东西。
    /// 「父目录不在就建出来」同理归库，钉在 `calibrate` 模块的用例里；
    /// 这里仍点名一个不存在的父目录，只为让命令行这条路真的走过它。
    ///
    /// 钉得住的是**交出去的是哪个 profile**：图恒等于面板分辨率，写出的 PNG 头里因此看得见
    /// 型号名解成了哪块面板；覆盖了灰阶数的那一趟排的阶梯条数不同，字节跟着不同。
    #[test]
    fn calibrate_folds_the_device_and_the_override_into_the_profile_it_hands_the_library() {
        let workspace = tempfile::tempdir().expect("建临时目录");
        let out = workspace.path().join("还不存在的目录").join("标定图.png");
        let two_levels = workspace.path().join("两级灰阶.png");

        let code = calibrate("Kobo Libra 2", None, &out).expect("写标定图");
        calibrate("Kobo Libra 2", Some(2), &two_levels).expect("写标定图");

        assert_eq!(code, SUCCESS_EXIT, "写成了就该是全部成功那个数");
        let panel = Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .panel()
            .resolution;
        let written = std::fs::read(&out).expect("读回标定图");
        let header = png::Decoder::new(std::io::Cursor::new(&written))
            .read_header_info()
            .expect("读 PNG 头")
            .clone();
        assert_eq!(
            (header.width, header.height),
            (panel.width, panel.height),
            "交给库的不是这台设备那块面板"
        );
        assert_ne!(
            written,
            std::fs::read(&two_levels).expect("读回标定图"),
            "--gray-levels 没跟着走到库那一侧"
        );
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

        let profile = cli
            .target_profile(&no_preset())
            .expect("profile 应当解析成功");

        assert_eq!(profile.device(), "kobo-libra-2");
        assert_eq!(profile.panel().gray_levels, 8);
    }

    /// `--fit` 两种都点得到，**不点名就是以高为准**（01 号票：默认换了）。
    ///
    /// 默认这一条要在命令行这一层单独钉住：库那一侧的 `FitMode::default()` 换了向，
    /// 而命令行完全可以自己写死另一个，那时用户敲出来的与文档说的对不上。
    #[test]
    fn the_fit_mode_from_the_command_line_names_how_pages_meet_the_panel() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        assert_eq!(
            parse(&["--fit", "INSIDE"])
                .fit_mode(&no_preset())
                .expect("inside 应当认得"),
            FitMode::Inside
        );
        assert_eq!(
            parse(&["--fit", "height"])
                .fit_mode(&no_preset())
                .expect("height 应当认得"),
            FitMode::Height
        );
        // 不点名就是以高为准。
        assert_eq!(
            parse(&[]).fit_mode(&no_preset()).expect("默认值"),
            FitMode::Height
        );
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--fit", "stretch"]).fit_mode(&no_preset()).is_err());
    }

    /// 帮助里要把这一趟的**行为变化与代价**说出来：默认是以高为准、跨页卷体积涨、
    /// 比面板矮的页被放大（01 号票的票面）。少了这几句，升级的人只会看到输出突然变样。
    #[test]
    fn the_fit_help_says_the_default_changed_and_what_it_costs() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--fit"), "{help}");
        assert!(help.contains("默认 height"), "{help}");
        assert!(help.contains("允许超过面板宽"), "{help}");
        assert!(help.contains("体积涨到约三四倍"), "{help}");
        assert!(help.contains("放大"), "{help}");
        // 「两种方式在普通漫画页上同尺寸」也要说：不然用户以为开关处处生效。
        assert!(help.contains("同一个尺寸"), "{help}");
    }

    /// `--no-crop` 关得掉裁边，**不点名就是裁**（页几何批 02 号票：默认打开）。
    ///
    /// 默认这一条要在命令行这一层单独钉住，与 `--fit` 那一条同一个理由：
    /// 库那一侧默认裁着，而命令行完全可以自己写反一个布尔，那时用户敲出来的与文档说的对不上。
    #[test]
    fn crop_is_on_unless_the_command_line_turns_it_off() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        assert!(!parse(&[]).no_crop, "不点名就该裁");
        assert!(parse(&["--no-crop"]).no_crop);
    }

    /// 帮助里要说清**默认是裁的**，以及裁法与它认下的那两件事。
    ///
    /// 关掉它要付的那笔账**不在这里断言**：那是互锁 ②，与另外两条一起写在
    /// `--help` 末尾的《开关互锁》里，由 [`the_help_lists_every_interlock_in_one_place`]
    /// 钉着（页几何批 05 号票：三条互锁只写一处）。这一条留下的只是一句指路。
    #[test]
    fn the_crop_help_says_it_is_on_by_default_and_how_the_margins_come_off() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--no-crop"), "{help}");
        assert!(help.contains("默认是裁的"), "{help}");
        // 裁法与它认下的两件事：孤立噪点不算内容，页间字号会跳动。
        assert!(help.contains("行列墨量占比"), "{help}");
        assert!(help.contains("孤立噪点"), "{help}");
        assert!(help.contains("字号因此会跳动"), "{help}");
        // 指路要在：互锁那一节是关掉裁边这件事唯一说得到代价的地方。
        assert!(help.contains("开关互锁"), "{help}");
    }

    /// 互锁**逐条**列在 `--help` 末尾那一节里，一条都不少（页几何批 05 号票）。
    ///
    /// 断言问的是 `after_long_help` 那一段原文，不是渲染出来的整份帮助：
    /// 渲染那一步会按终端宽度折行，而互锁每一句都长，`contains` 会被折行折断。
    /// 拿原文比，`Interlock::ALL` 加一条却忘了写进帮助时这条用例当场变红——
    /// 那正是「只写一处」要防的漂移。
    ///
    /// **处置不筛。** 一趟不吭声的那一条（裁边关着）与当场拒绝的那一条
    /// （`--dither fs` 撞上门）都在里面：处置说的是咬上之后这一趟怎么交代，
    /// 而这一节是说明书——躲得开的前提是事先看得到。
    ///
    /// 只进长帮助：`-h` 一项只印一行，那一节塞不进去。
    #[test]
    fn the_help_lists_every_interlock_in_one_place() {
        let command = Cli::command();
        let section = command
            .get_after_long_help()
            .expect("--help 末尾要有《开关互锁》那一节")
            .to_string();

        assert!(section.contains(INTERLOCK_HEADING), "{section}");
        for interlock in Interlock::ALL {
            assert!(section.contains(&interlock.to_string()), "{section}");
        }

        // 短帮助不带那一节：`-h` 一项只印一行。
        // 各开关那句路标仍在，路标不是第二份说法。
        assert!(
            !Cli::command()
                .render_help()
                .to_string()
                .contains(INTERLOCK_HEADING),
            "短帮助里不该有那一节"
        );
        // 长帮助真的印得出来——用户找得到它才谈得上「写在一处」。
        assert!(
            Cli::command()
                .render_long_help()
                .to_string()
                .contains(INTERLOCK_HEADING),
            "长帮助里没有那一节"
        );
    }

    /// **印出去的帮助没有一行过 [`wrap::TERMINAL_WIDTH`]**（票面第一条的帮助那一半）。
    ///
    /// 从前一行都不折：`--help` 里最长的那一行有 412 格，`-h` 有 300 格——
    /// clap 按空格折行，而中文长句里一个空格都没有（停车场 Q32）。
    ///
    /// 四份都问：`-h` 与 `--help` 缩进不同一档（见 [`SHORT_HELP_WIDTH`] 与
    /// [`LONG_HELP_WIDTH`]），子命令那一份还要再套一层。
    /// [`SHORT_HELP_WIDTH`] 里那个 31 是量出来的——添一个更长的开关时这一条当场变红。
    #[test]
    fn the_help_folds_every_line_into_the_terminal() {
        let folded = folded_help(Cli::command());
        let printed = [
            folded.clone().render_help().to_string(),
            folded.clone().render_long_help().to_string(),
            folded
                .clone()
                .find_subcommand_mut("calibrate")
                .expect("calibrate 子命令在")
                .render_help()
                .to_string(),
            folded
                .clone()
                .find_subcommand_mut("calibrate")
                .expect("calibrate 子命令在")
                .render_long_help()
                .to_string(),
        ];

        for help in &printed {
            for line in help.lines() {
                assert!(
                    wrap::width(line) <= wrap::TERMINAL_WIDTH,
                    "这一行 {} 格：{line}",
                    wrap::width(line)
                );
            }
        }

        // 折行不吃字：互锁那一节逐条都还在（折过之后按行找，整句已经被折断了）。
        // 两侧都把空白去掉——原文里[不许断的那个空格](wrap::HARD_SPACE)是给折行看的标注，
        // 印出去的是一个普通空格。
        let long = printed[1].replace(['\n', ' '], "");
        for interlock in Interlock::ALL {
            let said = interlock.to_string().replace([' ', wrap::HARD_SPACE], "");
            assert!(long.contains(&said), "折没了：{said}");
        }

        // **带空格的记号不断在中间**（停车场 Q106）：那两条命令各自整个留在某一行上，
        // 断开了照着抄就抄不出一条能用的命令。原文里它们中间那个空格带着标注
        // （见 `Interlock` 的 `Display`）。
        for token in ["--fit height", "--dither fs"] {
            assert!(
                printed[1].lines().any(|line| line.contains(token)),
                "{token} 被折断了：{}",
                printed[1]
            );
        }
    }

    /// `--no-split` 关得掉拆分，阈值与阅读方向点得动，**不点名就是拆、1.5、右开**
    /// （页几何批 04 号票：默认打开，阈值可调）。
    ///
    /// 默认这三项要在命令行这一层单独钉住，与 `--fit`、`--no-crop` 那两条同一个理由：
    /// 库那一侧的默认在 `SplitRule::default()`，而命令行完全可以自己写死另一套，
    /// 那时用户敲出来的与文档说的对不上。
    #[test]
    fn splitting_is_on_unless_the_command_line_turns_it_off() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        let default = parse(&[]).split_rule(&no_preset()).expect("默认值");
        assert!(default.on, "不点名就该拆");
        assert_eq!(default.threshold, SplitThreshold::default());
        assert_eq!(default.order, ReadingOrder::RightToLeft);

        assert!(
            !parse(&["--no-split"])
                .split_rule(&no_preset())
                .expect("认得")
                .on
        );
        assert_eq!(
            parse(&["--split-threshold", "2.5"])
                .split_rule(&no_preset())
                .expect("2.5 应当认得")
                .threshold,
            SplitThreshold::parse("2.5").expect("正数")
        );
        assert_eq!(
            parse(&["--reading-order", "LTR"])
                .split_rule(&no_preset())
                .expect("ltr 应当认得")
                .order,
            ReadingOrder::LeftToRight
        );
        // 认不出的取值在拼 Request 之前就被挡下。
        assert!(
            parse(&["--reading-order", "japanese"])
                .split_rule(&no_preset())
                .is_err()
        );
        assert!(
            parse(&["--split-threshold", "0"])
                .split_rule(&no_preset())
                .is_err()
        );
        assert!(
            parse(&["--split-threshold", "很宽"])
                .split_rule(&no_preset())
                .is_err()
        );
    }

    /// 帮助里要说清**默认是拆的**、切点由什么定、以及不切的那一种是什么
    /// （页几何批 04 号票的票面）。
    ///
    /// 少了「默认是拆的」，升级的人只会看到页数突然翻倍；少了「装订沟定切点」，
    /// 他会以为工具从正中盲切；少了「连续跨页不切」，他会以为整幅跨页也被切开了——
    /// 而那正是这张票明确不做的事。
    #[test]
    fn the_split_help_says_it_is_on_by_default_and_how_the_cut_point_is_found() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--no-split"), "{help}");
        assert!(help.contains("--split-threshold"), "{help}");
        assert!(help.contains("--reading-order"), "{help}");
        assert!(help.contains("默认是拆的"), "{help}");
        // 切点由装订沟定，不切正中。
        assert!(help.contains("装订沟"), "{help}");
        assert!(help.contains("不切正中"), "{help}");
        // 找不到沟的那一种不切，退回横向平移。
        assert!(help.contains("连续跨页"), "{help}");
        assert!(help.contains("横向平移"), "{help}");
        // 拆开的收益与它不换什么：不必横向翻动，而缩放系数不变。
        assert!(help.contains("缩放系数完全相同"), "{help}");
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
                .residual_filter(&no_preset())
                .expect("box 应当认得"),
            Filter::Area
        );
        // 不点名就是 ADR 0001 定的默认。
        assert_eq!(
            parse(&[]).residual_filter(&no_preset()).expect("默认值"),
            Filter::Lanczos3
        );
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(
            parse(&["--filter", "mitchell"])
                .residual_filter(&no_preset())
                .is_err()
        );
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
                .bit_depth_override(&no_preset())
                .expect("2 应当认得"),
            Some(BitDepth::Two)
        );
        // 不点名就由判据说了算。
        assert_eq!(
            parse(&[]).bit_depth_override(&no_preset()).expect("默认值"),
            None
        );
        // 全集之外的比特数在拼 Request 之前就被挡下。
        assert!(
            parse(&["--bit-depth", "3"])
                .bit_depth_override(&no_preset())
                .is_err()
        );
    }

    /// `--no-metadata` 关掉记录，幂等能力随之关闭——两件事是同一个开关，
    /// 帮助文本里必须并排说出来，否则用户会以为自己只是少写了几行 tEXt。
    #[test]
    fn no_metadata_says_in_the_help_text_that_it_also_turns_idempotency_off() {
        let line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
        let mut with_flag = line.clone();
        with_flag.extend(["--no-metadata", "volume-a"]);
        let mut without = line;
        without.push("volume-a");

        assert!(
            Cli::try_parse_from(with_flag)
                .expect("参数应当可解析")
                .no_metadata
        );
        // 不点名就照写：记录随文件走是默认行为。
        assert!(
            !Cli::try_parse_from(without)
                .expect("参数应当可解析")
                .no_metadata
        );

        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--no-metadata"), "{help}");
        assert!(help.contains("幂等能力随之关闭"), "{help}");
        assert!(help.contains("每一趟都整卷重做"), "{help}");
    }

    #[test]
    fn the_dither_mode_from_the_command_line_overrides_the_automatic_choice() {
        let parse = |arguments: &[&str]| {
            let mut line = vec!["tonefit", "--out", "out", "--profile", "kobo-libra-2"];
            line.extend_from_slice(arguments);
            line.push("volume-a");
            Cli::try_parse_from(line).expect("参数应当可解析")
        };

        assert_eq!(
            parse(&["--dither", "FS"])
                .dither_override(&no_preset())
                .expect("fs 应当认得"),
            Some(Dither::FloydSteinberg)
        );
        assert_eq!(
            parse(&["--dither", "none"])
                .dither_override(&no_preset())
                .expect("none 应当认得"),
            Some(Dither::Off)
        );
        // 不点名就由判据在几何门放行的那几种里选。
        assert_eq!(
            parse(&[]).dither_override(&no_preset()).expect("默认值"),
            None
        );
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(
            parse(&["--dither", "bayer"])
                .dither_override(&no_preset())
                .is_err()
        );
    }
}
