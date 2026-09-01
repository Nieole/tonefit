//! CLI：把命令行参数拼成 `Request`，分派到库那两件事上，把进度画出来。此外不做别的事。
//!
//! 出来的文字长什么样不在这里，在 [`render`]——命令行与会话共用同一套措辞，
//! 那一套因此不该长在任何一个入口里面。

mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard};

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tonefit::{
    BitDepth, CacheBudget, Dither, Filter, FitMode, Interlock, IoMode, Mode, Profile, Progress,
    ProgressSink, ReadingOrder, Report, Request, SplitRule, SplitThreshold,
};

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

    /// 要处理的卷：一个目录，或一个 CBZ。源只读。
    #[arg(required = true, value_name = "卷")]
    inputs: Vec<PathBuf>,

    /// 输出根目录。每个卷在它下面得到一份同名副本，容器形态与输入一致。
    #[arg(short, long, required = true, value_name = "目录")]
    out: Option<PathBuf>,

    /// 目标设备型号。内置表覆盖 Kobo、BOOX、Kindle 的主力型号，型号名不区分大小写与分隔符。
    #[arg(short, long, required = true, value_name = "型号")]
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

impl Cli {
    /// 本次做到哪一步。
    fn mode(&self) -> Mode {
        if self.dry_run {
            Mode::DryRun
        } else {
            Mode::Process
        }
    }

    /// 本次的适配方式。不点名就是默认的以高为准（01 号票）。
    fn fit_mode(&self) -> Result<FitMode> {
        match &self.fit {
            Some(name) => FitMode::resolve(name),
            None => Ok(FitMode::default()),
        }
    }

    /// 本次怎么拆跨页（04 号票）。三项收成一份规矩交给库，见 [`SplitRule`]。
    ///
    /// 不点名就是默认那一套：拆、阈值 1.5、右开。三项各自的默认在库那一侧
    /// （`SplitRule::default`），这里只把命令行点到的那几项盖上去——写死一份就是第二个出处。
    fn split_rule(&self) -> Result<SplitRule> {
        let default = SplitRule::default();
        Ok(SplitRule {
            on: !self.no_split,
            threshold: match &self.split_threshold {
                Some(text) => SplitThreshold::parse(text)?,
                None => default.threshold,
            },
            order: match &self.reading_order {
                Some(name) => ReadingOrder::resolve(name)?,
                None => default.order,
            },
        })
    }

    /// 本次残差段用哪个滤波器。不点名就是默认的 lanczos3（ADR 0001）。
    fn residual_filter(&self) -> Result<Filter> {
        match &self.filter {
            Some(name) => Filter::resolve(name),
            None => Ok(Filter::default()),
        }
    }

    /// 本次要不要点名位深。不点名就由判据说了算。
    fn bit_depth_override(&self) -> Result<Option<BitDepth>> {
        self.bit_depth.map(BitDepth::from_bits).transpose()
    }

    /// 本次要不要点名抖动模式。不点名就由判据在几何门放行的那几种里选。
    fn dither_override(&self) -> Result<Option<Dither>> {
        self.dither.as_deref().map(Dither::resolve).transpose()
    }

    /// 本次的缓存预算。不点名就是默认的那一档。
    fn cache_budget(&self) -> Result<CacheBudget> {
        match &self.cache_budget {
            Some(text) => CacheBudget::parse(text),
            None => Ok(CacheBudget::default()),
        }
    }

    /// 本次的读取策略。不点名就按路径探测介质（ADR 0009）。
    fn io_mode(&self) -> Result<IoMode> {
        match &self.io_mode {
            Some(text) => IoMode::resolve(text),
            None => Ok(IoMode::default()),
        }
    }

    /// 把 `--profile`、`--gray-levels` 与 `--threshold` 合成本次要用的 profile。
    fn target_profile(&self) -> Result<Profile> {
        target_profile(
            self.profile.as_deref().expect(REQUIRED_BY_CLAP),
            self.gray_levels,
            self.threshold,
        )
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
fn interlock_help() -> String {
    let mut text = format!("{INTERLOCK_HEADING}\n");
    for interlock in Interlock::ALL {
        text.push_str(&format!("  · {interlock}\n"));
    }
    text
}

/// 《开关互锁》那一节的标题。
///
/// 它自成一个常量，是为了让用例分得开**那一节本体**与各开关帮助里指向它的那句路标——
/// 两者都带着「开关互锁」四个字，而短帮助里该有的只有路标。
const INTERLOCK_HEADING: &str = "开关互锁（几项凑在一起会互相削弱的那几种组合）:";

/// 处理卷那一路的必填项走到这里就一定有值：`required = true` 挡在 clap 那一层。
///
/// 字段类型仍是 `Option`——`subcommand_negates_reqs` 让子命令那一路不必交出它们
/// （`calibrate` 根本不收输出根与卷），字段就得容得下「没有」。
/// 必填这道关因此留在 clap：错在哪、该怎么敲，它说得比这里好。
const REQUIRED_BY_CLAP: &str = "clap 的 required = true 已经挡在前面";

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

fn main() -> ExitCode {
    match execute() {
        Ok(code) => ExitCode::from(code),
        // `Result<()>` 那个 main 印的就是这一行，退出码 1。自己拿 `ExitCode` 之后照印。
        Err(error) => {
            eprintln!("Error: {error:?}");
            ExitCode::FAILURE
        }
    }
}

/// 这一趟的退出码：全部成功是 [`SUCCESS_EXIT`]，有卷被隔离是 [`ISOLATED_EXIT`]。
///
/// 拒绝执行那一种走不到这里——它在 `run` 里就返回了 `Err`，退出码是 `ExitCode::FAILURE`（1）。
/// 三个数因此各说一件事：做完了、做完了但有卷带着坏页、没做成。
///
/// 出的是 `u8` 而不是 `ExitCode`：后者不可比较，这条规则也就测不了，
/// 而「退出码分得开这两种」正是本票要钉住的那一条。
fn exit_code(report: &Report) -> u8 {
    if report.any_isolated() {
        ISOLATED_EXIT
    } else {
        SUCCESS_EXIT
    }
}

fn execute() -> Result<u8> {
    let cli = Cli::parse();
    if let Some(Command::Calibrate {
        profile,
        gray_levels,
        out,
    }) = &cli.command
    {
        return calibrate(profile, *gray_levels, out);
    }
    let profile = cli.target_profile()?;
    let fit = cli.fit_mode()?;
    let split = cli.split_rule()?;
    let filter = cli.residual_filter()?;
    let bit_depth = cli.bit_depth_override()?;
    let dither = cli.dither_override()?;
    let cache_budget = cli.cache_budget()?;
    let io_mode = cli.io_mode()?;
    let mode = cli.mode();
    let bar = Bar::new();
    let report = tonefit::run(&Request {
        inputs: cli.inputs,
        output_root: cli.out.expect(REQUIRED_BY_CLAP),
        profile,
        fit,
        crop: !cli.no_crop,
        split,
        filter,
        bit_depth,
        dither,
        per_page: cli.per_page,
        cache_budget,
        mode,
        io_mode,
        progress: Some(ProgressSink::new(bar)),
        metadata: !cli.no_metadata,
    })?;
    print!("{}", render::report(&report, mode));
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
/// 一卷一条，走完即抹掉：几十卷跑下来，屏幕上留下的只有报告。留着一排走完的进度条
/// 是另一种噪声，而「这一趟做了什么」报告说得更清楚。
struct Bar {
    /// 当下这一卷的那一条。卷与卷之间是空的。
    ///
    /// 每卷新起一条而不是复用同一条：走完的那一条已经收了尾，再往它身上设长度、加位置，
    /// indicatif 那一侧不保证还画得出来。
    volume: Mutex<Option<ProgressBar>>,
}

impl Bar {
    fn new() -> Self {
        Self {
            volume: Mutex::new(None),
        }
    }

    fn current(&self) -> MutexGuard<'_, Option<ProgressBar>> {
        self.volume
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Progress for Bar {
    fn volume_started(&self, volume: &Path, steps: u64) {
        let name = volume.file_name().map_or_else(
            || volume.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let bar = ProgressBar::new(steps);
        bar.set_style(
            ProgressStyle::with_template(
                "{msg} [{bar:30}] {pos}/{len} 步 · 已用 {elapsed} · 剩 {eta}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
        );
        bar.set_message(name);
        *self.current() = Some(bar);
    }

    fn stepped(&self) {
        if let Some(bar) = self.current().as_ref() {
            bar.inc(1);
        }
    }

    fn volume_finished(&self) {
        if let Some(bar) = self.current().take() {
            bar.finish_and_clear();
        }
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

        let profile = cli.target_profile().expect("profile 应当解析成功");

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
                .fit_mode()
                .expect("inside 应当认得"),
            FitMode::Inside
        );
        assert_eq!(
            parse(&["--fit", "height"])
                .fit_mode()
                .expect("height 应当认得"),
            FitMode::Height
        );
        // 不点名就是以高为准。
        assert_eq!(parse(&[]).fit_mode().expect("默认值"), FitMode::Height);
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--fit", "stretch"]).fit_mode().is_err());
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

        let default = parse(&[]).split_rule().expect("默认值");
        assert!(default.on, "不点名就该拆");
        assert_eq!(default.threshold, SplitThreshold::default());
        assert_eq!(default.order, ReadingOrder::RightToLeft);

        assert!(!parse(&["--no-split"]).split_rule().expect("认得").on);
        assert_eq!(
            parse(&["--split-threshold", "2.5"])
                .split_rule()
                .expect("2.5 应当认得")
                .threshold,
            SplitThreshold::parse("2.5").expect("正数")
        );
        assert_eq!(
            parse(&["--reading-order", "LTR"])
                .split_rule()
                .expect("ltr 应当认得")
                .order,
            ReadingOrder::LeftToRight
        );
        // 认不出的取值在拼 Request 之前就被挡下。
        assert!(
            parse(&["--reading-order", "japanese"])
                .split_rule()
                .is_err()
        );
        assert!(parse(&["--split-threshold", "0"]).split_rule().is_err());
        assert!(parse(&["--split-threshold", "很宽"]).split_rule().is_err());
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
                .residual_filter()
                .expect("box 应当认得"),
            Filter::Area
        );
        // 不点名就是 ADR 0001 定的默认。
        assert_eq!(
            parse(&[]).residual_filter().expect("默认值"),
            Filter::Lanczos3
        );
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--filter", "mitchell"]).residual_filter().is_err());
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
                .bit_depth_override()
                .expect("2 应当认得"),
            Some(BitDepth::Two)
        );
        // 不点名就由判据说了算。
        assert_eq!(parse(&[]).bit_depth_override().expect("默认值"), None);
        // 全集之外的比特数在拼 Request 之前就被挡下。
        assert!(parse(&["--bit-depth", "3"]).bit_depth_override().is_err());
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
                .dither_override()
                .expect("fs 应当认得"),
            Some(Dither::FloydSteinberg)
        );
        assert_eq!(
            parse(&["--dither", "none"])
                .dither_override()
                .expect("none 应当认得"),
            Some(Dither::Off)
        );
        // 不点名就由判据在几何门放行的那几种里选。
        assert_eq!(parse(&[]).dither_override().expect("默认值"), None);
        // 认不出的名字在拼 Request 之前就被挡下。
        assert!(parse(&["--dither", "bayer"]).dither_override().is_err());
    }
}
