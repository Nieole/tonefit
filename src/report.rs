//! `run` 的输出。

use std::path::PathBuf;
use std::time::Duration;

use crate::cache::CacheUsage;
use crate::color::PageColor;
use crate::crop::Crop;
use crate::decide::{CandidateScore, Verdict};
use crate::decode::Salvage;
use crate::envelope::Envelope;
use crate::geometry::{FitMode, GeometryGate, Size};
use crate::interlock::Interlock;
use crate::medium::IoPlan;
use crate::profile::Profile;
use crate::quantize::{Candidate, Dither};
use crate::resample::Scaling;
use crate::spread::{Cut, SplitRule};

/// 一次处理调用的结果。
///
/// **失败页集**是 [`Report::failures`]：
/// 它不另占一个字段，因为失败页本来就在它那一卷的 [`VolumeReport::pages`] 里按阅读顺序占着位——
/// 再存一份就有两个出处，而两处早晚会走散（12 号票）。
#[derive(Debug, Clone)]
pub struct Report {
    /// 本次实际使用的 profile 与它的面板。这批输出该拿去哪台设备看，答案在这里。
    pub profile: Profile,
    /// 本次用的适配方式（页几何批 01 号票）。
    ///
    /// 它与 [`profile`](Self::profile) 并排，理由也一样：读的人要知道这批页的尺寸
    /// 是**照哪条规矩**算出来的。两种方式在普通漫画页上产出同一个尺寸
    /// （见 `crate::FitMode`），光看页尺寸分不出这一趟走的是哪一条。
    pub fit: FitMode,
    /// 这一趟裁不裁白边（页几何批 02 号票）。
    ///
    /// 与 [`fit`](Self::fit) 并排，理由也一样：页尺寸是照哪条规矩算出来的，读的人要知道。
    /// 逐页那一行只在**真裁掉了东西**时才说话，而一页都没裁与整趟没开是两件事——
    /// 分辨它们只有这一项。
    pub crop: bool,
    /// 这一趟怎么拆跨页（页几何批 04 号票）。
    ///
    /// 与 [`fit`](Self::fit)、[`crop`](Self::crop) 并排，理由是同一条：这一卷有几页、
    /// 每一页是哪一块，都照它算出来。逐页那一行只在**这一张真是切出来的一半**时才说话，
    /// 而「整卷没有一张跨页」与「整趟没开拆分」是两件事——分辨它们只有这一项。
    pub split: SplitRule,
    pub volumes: Vec<VolumeReport>,
    /// 整趟的墙钟耗时：`run` 从进到出（加固批 11 号票）。
    ///
    /// 它**装得下**各卷 [`VolumeTiming::elapsed`]，而不等于它们的和：开工前那几道检查
    /// （处理范围非空、输出不在源里、两个卷不撞同一个去处）与逐卷的介质探测都在卷外，
    /// 而这两样都要摸文件系统——慢盘与网络盘上探测尤其不便宜（ADR 0009）。
    ///
    /// **预扫**不在那一截里：它枚举各卷花掉的时间各自算进那一卷（见 `crate::survey`），
    /// 枚举挪到开工之前并没有让它变便宜，报出来的地方因此没变。
    ///
    /// 计时**只进结构，不进渲染出的文字**：印不印、印在哪、印成什么样由调用方定。
    /// 这与进度那一条是同一条规矩（见 `progress`：库只报到，样子由调用方定），
    /// 直接的好处是黄金快照不随机器快慢而变。
    pub elapsed: Duration,
}

impl Report {
    /// 这一趟的开关咬上了哪几条互锁（页几何批 05 号票）。
    ///
    /// 规则不在这里，在 [`Interlock`]：报告只把自己带着的那几项开关交给它。
    /// 哪几条落到报告上、落在报告的哪一段，界面层照 [`Interlock::voice`] 挑
    /// （见二进制侧的 `render::header`）——三条处置各不相同，报告不替它们挑。
    ///
    /// [`Interlock::DitherOutsideTheGate`] 出不来，而那不是漏：它的处置是**当场拒绝**，
    /// 咬上了 `run` 就返回 `Err`——这份结构存在本身，就是它没咬上的证据。
    pub fn interlocks(&self) -> impl Iterator<Item = Interlock> {
        Interlock::engaged(self.fit, self.crop, self.split)
    }

    /// 本次每一个失败页，按卷序、卷内按阅读顺序（spec 的 story 26）。
    ///
    /// 页自己说得出它是哪一卷的哪一页——`source` 是卷根接上成员的相对路径。
    pub fn failures(&self) -> impl Iterator<Item = &PageReport> {
        self.volumes.iter().flat_map(VolumeReport::failures)
    }

    /// 本次每一个部分救回页，按卷序、卷内按阅读顺序（04 号票）。
    ///
    /// 与 [`failures`](Self::failures) 并列而不是并进去：部分救回页**处理成了**——
    /// 它有自己的尺寸、判定与像素，卷也不因为它进隔离目录。两者混成一份清单，
    /// 「这一趟有几页根本没出来」就再也问不出来。
    pub fn salvaged(&self) -> impl Iterator<Item = &PageReport> {
        self.volumes.iter().flat_map(VolumeReport::salvaged)
    }

    /// 本次**输出宽超过面板宽**的页，按卷序、卷内按阅读顺序（页几何批 01 号票）。
    ///
    /// 这些页要求阅读器**平移、不缩放**才看得全——那比留边那一侧的要求更强
    /// （见 `crate::GeometryGate::of`），而用户翻它们时要横向翻动。
    /// 报告因此得点得出是哪几页：以高为准下跨页卷几乎整卷落在这里，
    /// 普通漫画卷一页都没有（实测棋魂 0%、哆啦A梦 91%，见 measurements 的《适配方式：fit-inside 与以高为准》）。
    ///
    /// 问的是 [`PageReport::size`]，因此**失败页也算**：它那张占位页按卷内统一尺寸写出，
    /// 那个尺寸真有那么宽，翻起来真要平移。几何门与它无关——门问的是「贴没贴住」，
    /// 溢出的页贴得好好的（见 `crate::GeometryGate`）。
    pub fn wider_than_the_panel(&self) -> impl Iterator<Item = &PageReport> {
        let panel = self.profile.panel().resolution.width;
        self.volumes
            .iter()
            .flat_map(|volume| volume.pages.iter())
            .filter(move |page| page.size.width > panel)
    }

    /// 本次目标尺寸被**兜底上界**改过的页，按卷序、卷内按阅读顺序（07 号票）。
    ///
    /// 这几页没按 [`fit`](Self::fit) 那条规矩出，而是退回了 fit-inside——点名那条路
    /// 算出的尺寸大到分配不下，照着走整趟都要停（见 `crate::FitMode::target`）。
    /// 用户点了一种适配方式，报告因此得点得出哪几页不是照它出的。
    ///
    /// 与 [`wider_than_the_panel`](Self::wider_than_the_panel) **不重叠**：退回之后的页
    /// 恒不超过面板宽，一页都不会同时落在两张清单里。
    pub fn backstopped(&self) -> impl Iterator<Item = &PageReport> {
        self.volumes
            .iter()
            .flat_map(|volume| volume.pages.iter())
            .filter(|page| page.backstopped())
    }

    /// 本次有没有卷被隔离。退出码要分得开「全部成功」与「有卷被隔离」，问的就是它。
    pub fn any_isolated(&self) -> bool {
        self.volumes.iter().any(VolumeReport::isolated)
    }
}

/// 一个卷的结果。
#[derive(Debug, Clone)]
pub struct VolumeReport {
    /// 卷标识：源目录路径，或源归档的文件路径。
    pub volume: PathBuf,
    /// 该卷的输出：目录卷是一个目录，归档卷是一个归档文件。
    ///
    /// 去处随「有没有失败页」在干净目录与隔离目录之间跳（12 号票），
    /// 上一趟落在另一处的那一份见 [`superseded`](Self::superseded)。
    pub output: PathBuf,
    /// 这一卷在**另一个**去处留着的上一趟输出，没有就是 `None`（12 号票的「过期副本」）。
    ///
    /// 一卷只有一个去处，而去处会跳：上一趟有页坏了写进了隔离目录，这一趟坏页修好了
    /// 写回干净目录——隔离目录里那一份不会被覆盖，也不会被删（源库只读那条规矩的同一个理由：
    /// 不替用户扔东西）。它于是可能是**一整卷白页**的占位输出，摆在文件管理器里
    /// 与一本正经的书没有分别。
    ///
    /// 报告因此得指出来。这是本票「问题可见且有界」那一句的直接后件：
    /// 隔离这套机制自己制造了一份会骗人的东西，藏起来就等于白做。
    pub superseded: Option<PathBuf>,
    /// 按阅读顺序排列的**输出页**。
    ///
    /// 一个源页产出一到多张输出页（页几何批 03 号票），同一源页切出来的那几张挨着排。
    /// 因此这里的条数是输出那一侧的数，源那一侧的数在 [`source_pages`](Self::source_pages)。
    /// 一张源页切成几张由跨页拆分说了算（页几何批 04 号票），上界是 `crate::MAX_OUTPUTS_PER_SOURCE_PAGE`。
    pub pages: Vec<PageReport>,
    /// 本卷的**源页数**：这一卷有几张待处理的图片（页几何批 03 号票）。
    ///
    /// 与输出页数分开说，因为两者从此不是同一个数：一个源页可以产出多张输出页。
    /// 输出页数走 [`page_count`](Self::page_count)。
    ///
    /// 跳过的卷也有一份：它是**源**那一侧的事实，源枚举就数得出来，不做工作也答得出。
    pub source_pages: usize,
    /// 本卷的卷级判定：这一卷的候选从哪来。抖动模式在这个候选里。
    ///
    /// 一页都没有的卷（只装着透传文件）是 `None`：那样的卷没有候选可判。
    pub verdict: Option<VolumeVerdict>,
    /// 本卷缓存的用量（ADR 0005）。
    pub cache: CacheUsage,
    /// 本卷这一趟怎么读：介质是什么，据此派了几条读取（13 号票）。
    ///
    /// 探测退到保守策略时**要说得出为什么**，那句话就在 [`IoPlan::medium`] 里
    /// （见 [`Medium::Unknown`](crate::Medium::Unknown)）。不说，用户看到的只是「跑得慢」。
    ///
    /// 跳过的卷也有一份：幂等那一道照样要把整卷的字节读一遍，读法与做事的那一趟同一个。
    pub io: IoPlan,
    /// 本卷解码源页的次数。跳过的卷是 0——「不重复工作」量得出来的形式就是它。
    ///
    /// 两遍管线的不变量是「每页只解码一次」（ADR 0005：解码一次，缓存缩放后的图），
    /// 这个数因此恒等于[源页数](Self::source_pages)——解码发生在切开之前，
    /// 一张源页产出几张输出页都只解一次。它在报告里，是为了让那条不变量在 `run` 这个 seam 上量得出来——
    /// 第二遍一旦回头碰源页，它立刻大于页数，而别的外部可见事实都察觉不到这件事。
    pub decodes: usize,
    /// 本卷这一趟的墙钟耗时，按管线的段分开（加固批 11 号票）。跳过的卷也有一份。
    pub timing: VolumeTiming,
}

/// 一个卷这一趟的墙钟耗时，按**管线的段**分开（加固批 11 号票）。
///
/// 段是 [`fingerprint`](Self::fingerprint)、[`first_pass`](Self::first_pass)、
/// [`second_pass`](Self::second_pass) 三个，与进度报到的那三段同一条分界线
/// （`CONTEXT.md` 的《进度》：幂等这一道读全部成员、第一遍走每一页、第二遍写全部成员）。
/// 前两段量的是**源**那一侧，第二段量的是**输出**那一侧——一个源页产出一到多张输出页
/// （页几何批 03 号票），而切开发生在第一遍之内。
/// 三段在 `crate::process_volume` 里依次首尾相接、互不重叠，一个卷总共只掐三次表——
/// **插桩点一个都不在热路径上**。
///
/// 页内那一层（解码、缩放、判据、量化、编码）不在这里，而且不该在：那几步在满核并行里交错跑，
/// 「解码耗时」是墙钟还是 CPU 时间说不清，聚合出来的数会骗人，而插桩点全落在热路径上。
/// 要那一层的数走事件流（ADR 0011）或 feature-gated 插桩（加固批 13 号票）。
///
/// 三段之和**不等于** [`elapsed`](Self::elapsed)：打开卷、枚举成员、汇总、定去处都在段外。
/// 那一截有名字，见 [`outside_the_segments`](Self::outside_the_segments)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VolumeTiming {
    /// 幂等这一道：算出本卷指纹，再与上一趟写在输出里的比。两半都在这一段里——
    /// 算的那一半把整卷源字节读一遍，比的那一半开输出容器、逐成员读回记录。
    ///
    /// 它**不是免费的**（`CONTEXT.md` 的《管线》），而幂等命中的卷付的正好只有这一笔——
    /// 「跳过一卷为什么也要等这么久」只有这个数答得出来，报告里因此不许报零。
    /// `--no-metadata` 下整道不在，那时才是 [`Duration::ZERO`]。
    pub fingerprint: Duration,
    /// 第一遍：解码、彩页识别、几何、缩放、算判据、进缓存。
    ///
    /// 幂等命中的卷提前收摊，一遍都不走，是 [`Duration::ZERO`]。
    pub first_pass: Duration,
    /// 第二遍：建输出容器、量化、编码、写页、搬透传文件、收尾改名。
    ///
    /// 写出的是**输出页**，一张一步（页几何批 03 号票）。
    /// 段界照**步**那一侧划（第二遍写全部成员），透传成员因此算在这里；建容器与收尾改名
    /// 同样算在这里——它们是写出这件事的两头，摊在段外只会让这个数假装比实际便宜。
    /// dry-run 一个文件都不落盘，幂等命中的卷也不走，两种情形都是 [`Duration::ZERO`]。
    pub second_pass: Duration,
    /// 这一卷的墙钟耗时：从打开卷到这份卷报告成型。
    pub elapsed: Duration,
}

impl VolumeTiming {
    /// 三段之外的那一截：打开卷、枚举成员、查重、汇总、定去处、拼报告。
    ///
    /// 它是 [`elapsed`](Self::elapsed) 减去三段，饱和到零。有名字是为了让「三段不求和」
    /// 这件事说得出口：读的那一端自己去加那三个数，得出的是一个偏小的总耗时，
    /// 而少掉的那一截恰恰是枚举——慢盘上它不小。
    ///
    /// 名字不叫 `elsewhere`：那个词在 crate 里已经指着**卷的另一个去处**
    /// （见 `crate::superseded`），一个词两个意思，读的人迟早认错一处。
    ///
    /// 饱和是**断言的形式**，不是遮丑：三段是 `elapsed` 里的一部分，差额不可能为负，
    /// 真为负说明段与段重叠了，那时这个数为零，而三段之和会大于 `elapsed`。
    pub fn outside_the_segments(&self) -> Duration {
        self.elapsed
            .saturating_sub(self.fingerprint + self.first_pass + self.second_pass)
    }
}

/// 卷级判定：这一卷的候选从哪来（spec 的卷报告形状：判定位深、抖动模式、判定理由、驱动页）。
///
/// 几项绑成一个枚举而不是各占一个字段，因为它们不是每一种情形下都同时成立：
/// `--per-page` 一开，卷级根本没有候选，也就没有驱动页可指。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeVerdict {
    /// 卷级上包络定的基准档（ADR 0006：位深按卷取上包络并加迟滞）。
    /// 抖动那一维跟着位深一起定在这里（ADR 0007：上包络取的是这个组合）。
    Envelope(Envelope),
    /// 覆盖项把候选裁到只剩一个：判定被顶掉，卷级基准档无从谈起（spec 的 story 23）。
    ///
    /// 裁到只剩一个之后每一页的判定都是覆盖，逐页结果里没有分布可聚合——上包络于是不是
    /// 「被关掉」，而是无从谈起。裁不到只剩一个的覆盖不走这里：`--bit-depth` 点了一档位深、
    /// 而几何门开着时，抖动那一维还有得判，卷级仍是一次上包络。
    Override(Candidate),
    /// `--per-page` 关掉了上包络与迟滞：候选逐页最优，卷内没有基准档
    /// （ADR 0006 决定第 6 条）。翻页跳变随之回来，抖动模式也跟着逐页可变。
    PerPage,
    /// 幂等命中：输出已经在，且工具版本、profile、参数、源四项都没变，本卷一页都没有重做
    /// （ADR 0006：同一批 tEXt 字段兼作幂等依据）。
    ///
    /// `page_count` 是这一卷的**输出**页数——上一趟写在那儿、这一趟逐个比过指纹的那些页
    /// （见 `crate::can_skip`）。不做工作也数得出来：那份名单在碰像素之前就给得出
    /// （`crate::page_targets`）。源那一侧的数在 [`VolumeReport::source_pages`]。
    ///
    /// 它不叫 `pages`：[`VolumeReport::pages`] 是逐页结果，而跳过的那一趟一份都没有。
    /// 读页数一律走 [`VolumeReport::page_count`]。
    ///
    /// 与另外三种并列而不是另起一个字段，因为它和它们是同一个问题的答案：
    /// 「这一卷的候选从哪来」——这一卷的候选哪儿也没来，上一趟写的还在。
    Skipped { page_count: usize },
}

impl VolumeVerdict {
    /// 这一卷定下的抖动模式（09 号票：`Report` 要标明本卷的抖动模式）。
    ///
    /// `--per-page` 下没有卷级的那一个：抖动跟着位深一起逐页可变
    /// （ADR 0006 决定第 6 条）。
    pub fn dither(&self) -> Option<Dither> {
        match self {
            VolumeVerdict::Envelope(envelope) => Some(envelope.base.dither),
            VolumeVerdict::Override(candidate) => Some(candidate.dither),
            VolumeVerdict::PerPage | VolumeVerdict::Skipped { .. } => None,
        }
    }
}

impl VolumeReport {
    /// 这一卷被幂等命中跳过了吗。
    ///
    /// 跳过这件事在本结构里由两处一起体现——卷级判定是 [`VolumeVerdict::Skipped`]、
    /// `pages` 为空。**它们只有一个来源**，就是这里：读的那一端各自去认
    /// 其中一处，迟早会有人认错一处。
    pub fn skipped(&self) -> bool {
        matches!(self.verdict, Some(VolumeVerdict::Skipped { .. }))
    }

    /// 本卷的**输出**页数：这一卷在输出容器里躺着几页。失败页也算——它在输出里
    /// 占着自己那一格。源那一侧的数在 [`source_pages`](Self::source_pages)，
    /// 一个源页可以产出多张输出页（页几何批 03 号票）。
    ///
    /// 跳过的卷没有逐页结果，页数从卷级判定里取——那份名单在碰像素之前就给得出，
    /// 报告不该因为跳过就说这一卷是 0 页。
    pub fn page_count(&self) -> usize {
        match self.verdict {
            Some(VolumeVerdict::Skipped { page_count }) => page_count,
            _ => self.pages.len(),
        }
    }

    /// 本卷的失败页，按阅读顺序（spec 的 story 26）。
    pub fn failures(&self) -> impl Iterator<Item = &PageReport> {
        self.pages
            .iter()
            .filter(|page| matches!(page.outcome, PageOutcome::Failed { .. }))
    }

    /// 本卷落在几何门**判定范围**里的页，按阅读顺序（06 号票）。
    ///
    /// 范围是灰度路径上的每一页（ADR 0007 决定第 1 条）：彩色分支上的页不在里面——
    /// 那条路径既不量化也不抖动（ADR 0010 决定第 4 条）；失败页连几何都没有。
    ///
    /// 报告要说得出这个范围，否则「几何门成立」这句话读不出它替多少页说了话：
    /// 一卷全是彩页时门同样成立，而那是「无人可关」，不是「每一页都贴住了面板」。
    pub fn judged_by_the_gate(&self) -> impl Iterator<Item = &PageReport> {
        self.pages.iter().filter(|page| page.gate().is_some())
    }

    /// 本卷几何门不成立的页，按阅读顺序（06 号票）。
    ///
    /// 它们**只被排除在抖动之外**：位深仍跟着卷级基准档走、不低于它（ADR 0007 决定第 3 条）。
    /// 一页门成立的灰度页都没有时它们就是主体，那时这个清单等于整个判定范围，
    /// 而卷级基准档本身就不抖。
    pub fn outside_the_gate(&self) -> impl Iterator<Item = &PageReport> {
        self.pages
            .iter()
            .filter(|page| page.gate() == Some(GeometryGate::Broken))
    }

    /// 本卷的部分救回页，按阅读顺序（04 号票）。
    ///
    /// 它们**不**把这一卷送进隔离目录：救回来的页有自己的尺寸与像素，是处理成了的页。
    /// 报告仍要数得出来——源文件不全这件事只此一处说得出口，而它没有退出码替它喊。
    pub fn salvaged(&self) -> impl Iterator<Item = &PageReport> {
        self.pages.iter().filter(|page| page.salvage().is_some())
    }

    /// 本卷被隔离了吗（spec 的 story 25）。
    ///
    /// **判据只有一条：有没有失败页。**隔离目录那个去处是它的**结果**，
    /// 不是另一个可以各说各话的事实——[`output`](Self::output) 已经指着隔离目录了，
    /// 再存一个布尔量就是第二个出处。
    pub fn isolated(&self) -> bool {
        self.failures().next().is_some()
    }
}

/// 一页的结果。
///
/// 归档卷的页没有文件系统路径，`source` 与 `output` 因此是**卷路径接上成员的相对路径**，
/// 长成 `卷.cbz/001.png`。这是页在卷里的身份，不是一个打得开的路径——
/// 归档卷真正打得开的那个路径是 [`VolumeReport::output`]。
#[derive(Debug, Clone)]
pub struct PageReport {
    pub source: PathBuf,
    pub output: PathBuf,
    /// 目标尺寸：实际写出的像素尺寸。
    ///
    /// 失败页也有一个，而且是真的——它按**卷内统一尺寸**留白占位写了出去（12 号票）。
    /// 部分救回页用的是它自己的尺寸：文件头里那个尺寸一点没缺（04 号票）。
    pub size: Size,
    /// 这一页有没有处理成，处理成了的话它走了哪条分支。
    pub outcome: PageOutcome,
}

/// 一页的处理结果：完好、部分救回，还是失败（04 号票）。
///
/// **三种，不是两种。** 12 号票只问「解出来了吗」，而救回 99% 与救回 0 行在那一层是同一个
/// 答案。中间那一种单列出来，是因为它在管线上真的与另外两种都不同：完好页什么都参加，
/// 部分救回页**不参与几何门与卷级上包络**（见 `crate::first_pass` 与
/// `crate::summarize_volume`），失败页连像素都没有。
///
/// 这一层与 [`PageBranch`] 问的不是同一件事：这里问「这一页解出来了多少」，那里问
/// 「解出来之后它走了哪条路」。合成一个枚举就得回答「失败页走的是哪条分支」，
/// 而那个问题没有答案——分支是解码之后才分的。
#[derive(Debug, Clone)]
pub enum PageOutcome {
    /// 完好页：整解出来的一页。
    Whole(Processed),
    /// 部分救回页：整解失败，按文件头里的尺寸救回了其中一段（04 号票）。
    ///
    /// 它照常缩放、判定、写出——几何一点没缺，缺的那一段留成纸白。但它**不替整卷说话**：
    /// 几何门与卷级上包络都没有它，一页没解全的页不该定另外一百多页的档。
    Salvaged {
        page: Processed,
        /// 这一页救回了多少。
        salvage: Salvage,
    },
    /// 失败页：字节读不出来，或完整尺寸解不出来，或尺寸解得出来而像素缓冲大到分配不下，
    /// 或救回了却一个像素都没解出来（12 号票的三种，加上 04 号票补的第四种，判据见 `decode`）。
    ///
    /// 它仍然写了出去——以卷内统一尺寸留白占位，页序因此不断、卷内尺寸因此一致。
    /// 含失败页的卷整卷进隔离目录（见 [`VolumeReport::isolated`]）。
    Failed {
        /// 这一页为什么失败，给人当场读的那一句：由内到外的错误链，指得出是哪一页。
        ///
        /// 与记录那一侧那句钉死的英文不是重复（见 `metadata`）：tEXt 只装得下 Latin-1，
        /// 而这一句要说得出具体是哪个成员、卡在哪一步。
        reason: String,
    },
}

/// 处理成了的一页留下的东西。完好页与部分救回页共用它——两者在这几项上一模一样，
/// 差别只有「救回了多少」，而那一项挂在 [`PageOutcome::Salvaged`] 上。
///
/// 缩放与彩页识别落在这里而不在 [`PageReport`] 上，因为失败页两样都没有：
/// 它没被缩放过，也没人看得出它是不是彩页。摆一个 `PageColor::Gray` 上去是编的，
/// 报告不该有编出来的字段。
#[derive(Debug, Clone)]
pub struct Processed {
    /// 这一页裁掉了多少白边（页几何批 02 号票）。
    ///
    /// 裁边发生在**适配之前**：目标尺寸由 [`Crop::after`] 算出，而不是由源尺寸算出。
    /// 因此它排在 [`scaling`](Self::scaling) 前面——报告里两者也是这个顺序，
    /// 读的人顺着「解出来多大 → 裁完多大 → 缩了多少 → 写出多大」一路读下来。
    ///
    /// 裁边关着的那一趟是一个原样通过的窗口，与「这一页没什么可裁」长得一样：
    /// 分辨它们靠报告抬头那一行。
    pub crop: Crop,
    /// 这一页的目标尺寸是**兜底上界**改出来的吗（07 号票）。
    ///
    /// 为真的页没按抬头那条适配方式出，而是退回了 fit-inside：点名那条路算出的尺寸
    /// 大到分配不下，照着走整趟都要停（见 `crate::FitMode::target`）。
    /// 读的口子在 [`PageReport::backstopped`]，整趟的清单在 [`Report::backstopped`]。
    pub backstopped: bool,
    /// 这一张是那一刀的产物：切在哪条装订沟上、这一张是哪一侧。
    /// 整页出的是 `None`（页几何批 04 号票）。
    ///
    /// 哪一侧说的是**这张图原来长在页的哪边**，不是它排第几：先读哪一侧由阅读方向定
    /// （见 [`ReadingOrder`](crate::ReadingOrder)），而反过阅读方向之后同一张图仍是那一侧。
    ///
    /// 「切没切」在报告里另有两处痕迹——[`VolumeReport::source_pages`] 与输出页数分家、
    /// 同一个 `source` 出现两次。两处都要读的人自己去推，而这一项直说；
    /// 三处对不上是不可能的：它们全由第一遍那一次拆分定下。
    pub cut: Option<Cut>,
    /// 这一张所属的源页够得上**跨页候选**吗（页几何批 04 号票）。
    ///
    /// 它与 [`cut`](Self::cut) 一起才说得全拆分那两级：`cut` 有值的必然是候选；
    /// **候选而 `cut` 为空的，就是连续跨页**——够得上宽高比那一关，却找不到装订沟，
    /// 因此不切、退回这一趟的适配方式。两项都为假的页根本没进过拆分。
    ///
    /// 报告非说得出这一项不可，否则「拆分把这一页放过去了」有两种读法而分不开，
    /// 而真实素材冒烟数的**误报率**问的正是其中一种：单页卷上这一项为真的页，
    /// 就是候选那一关漏下来的（04 号票的验收，见 `tests/smoke.rs`）。
    pub spread_candidate: bool,
    /// 这一页实际走过的缩放：总缩放比、预缩倍数、残差比（ADR 0001）。
    ///
    /// 预缩这条路径在 B 类素材上从不触发（见 measurements 的《B 类素材普查》），
    /// 报告里说清楚它有没有触发，是这条路径在真实素材上唯一的现场证据。
    pub scaling: Scaling,
    /// 第一遍识别出这一页是彩页还是灰度页（ADR 0005 决定第 1 条：彩页识别在第一遍）。
    ///
    /// 这是**页的事实**，与它走了哪条分支不是一回事：黑白 profile 下彩页转灰、
    /// 走的是 [`PageBranch::Gray`]，但它仍然是一张彩页。两者都在报告里，
    /// 「这一页为什么没保留颜色」才答得出来。
    pub color: PageColor,
    /// 这一页走的那条分支，连同那条分支的产物。
    pub branch: PageBranch,
}

impl PageOutcome {
    /// 处理成了的那一页留下的东西。失败页没有。
    ///
    /// 完好页与部分救回页在这里合流：读的那一端只有在**问的正是那点差别**时才该分辨两者，
    /// 而分辨的口子只有一个，见 [`PageReport::salvage`]。
    fn processed(&self) -> Option<&Processed> {
        match self {
            PageOutcome::Whole(page) | PageOutcome::Salvaged { page, .. } => Some(page),
            PageOutcome::Failed { .. } => None,
        }
    }
}

/// 一页走过的那条分支（ADR 0010：彩页按 profile 分流）。
///
/// 两条分支的产物不是同一套，所以它们是一个枚举而不是几个各自可空的字段：
/// 彩色分支上没有判据曲线、也没有判定——那条路径根本不量化，说「判定为空」
/// 会读成「判定丢了」。
#[derive(Debug, Clone)]
pub enum PageBranch {
    /// 灰度路径：算判据、进缓存、第二遍照判定量化写出。
    /// 黑白 profile 下的彩页转灰后也走这里（ADR 0005 决定第 4 条）。
    Gray {
        /// 这一页的几何门判定（ADR 0007 决定第 1 条：门逐页判）。
        ///
        /// 它落在灰度路径这一支上而不在 [`PageReport`] 上，因为判定范围就是这条路径：
        /// 彩色分支上没有门可判（ADR 0010 决定第 4 条），失败页连几何都没有。
        /// 摆一个 `Holds` 上去是编的，报告不该有编出来的字段。
        ///
        /// 卷级那一层的「判定范围」与「被排除的页」都从这里数出来
        /// （[`VolumeReport::judged_by_the_gate`]、[`VolumeReport::outside_the_gate`]）：
        /// 门的结果只有一个出处，就是页。
        gate: GeometryGate,
        /// 各候选的判据值，由小到大。候选已按面板灰阶数与几何门裁过（ADR 0003、ADR 0007）。
        ///
        /// 两种模式都求值：判据现在决定输出的候选，dry-run 因此预告的就是照做时的那一个。
        scores: Vec<CandidateScore>,
        /// 这一页最终定下的候选，以及定它的理由（spec 的 story 7）。
        ///
        /// 卷级那一层开着时，这里是上包络重定过的结果，不是逐页判定的原始输出——
        /// 写出去的就是它。逐页判定要的那一档仍可从 `scores` 与阈值读出来。
        ///
        /// 判定说的是**量化格点**。文件里写着的那个位深可能更低——一页只用得上几个取值时，
        /// 调色板装得下同样的像素而位宽更窄，那是编码器接口以内的事（ADR 0004，见 `encode`）。
        verdict: Verdict,
    },
    /// 彩色分支：只做缩放，不量化、不进灰度缓存、不进卷级上包络
    /// （ADR 0005 决定第 4 条）。彩色 profile 下的彩页走这里。
    Color,
}

impl PageReport {
    /// 这一页救回了多少。完好页与失败页都没有——一个整解出来了，一个一个像素都没有。
    ///
    /// **三种页状态的唯一分辨口**（04 号票）：[`failure`](Self::failure) 有值的是失败页，
    /// 这里有值的是部分救回页，两处都没有的才是完好页。各处自己去认 [`PageOutcome`]
    /// 的变体，迟早有人少认一种。
    pub fn salvage(&self) -> Option<Salvage> {
        match &self.outcome {
            PageOutcome::Salvaged { salvage, .. } => Some(*salvage),
            PageOutcome::Whole(_) | PageOutcome::Failed { .. } => None,
        }
    }

    /// 这一页定下的候选与理由。彩色分支与失败页都没有——一个不量化，一个没解出来。
    pub fn verdict(&self) -> Option<Verdict> {
        match self.branch() {
            Some(PageBranch::Gray { verdict, .. }) => Some(*verdict),
            _ => None,
        }
    }

    /// 这一页的几何门判定。彩色分支与失败页上都没有——那是判定范围之外
    /// （见 [`VolumeReport::judged_by_the_gate`]）。
    pub fn gate(&self) -> Option<GeometryGate> {
        match self.branch() {
            Some(PageBranch::Gray { gate, .. }) => Some(*gate),
            _ => None,
        }
    }

    /// 这一页各候选的判据值，由小到大。彩色分支与失败页上都是空的。
    pub fn scores(&self) -> &[CandidateScore] {
        match self.branch() {
            Some(PageBranch::Gray { scores, .. }) => scores,
            _ => &[],
        }
    }

    /// 这一页走的那条分支。失败页没有——分支是解码之后才分的。
    pub fn branch(&self) -> Option<&PageBranch> {
        self.outcome.processed().map(|page| &page.branch)
    }

    /// 第一遍认出这一页是彩页还是灰度页。失败页没解出来，看不出。
    pub fn color(&self) -> Option<PageColor> {
        self.outcome.processed().map(|page| page.color)
    }

    /// 这一页实际走过的缩放。失败页没被缩放过。
    pub fn scaling(&self) -> Option<Scaling> {
        self.outcome.processed().map(|page| page.scaling)
    }

    /// 这一页裁掉了多少白边（页几何批 02 号票）。失败页没有——它没有像素可裁。
    pub fn crop(&self) -> Option<Crop> {
        self.outcome.processed().map(|page| page.crop)
    }

    /// 这一张是那一刀的产物吗；是的话，切在哪条装订沟上、这一张是哪一侧
    /// （页几何批 04 号票）。
    ///
    /// 两种页都没有：整页出的那些（它们不是切出来的），以及失败页（它没有像素可切，
    /// 恒出一张占位页）。
    pub fn cut(&self) -> Option<Cut> {
        self.outcome.processed().and_then(|page| page.cut)
    }

    /// 这一张所属的源页够得上**跨页候选**吗（页几何批 04 号票）。
    ///
    /// 失败页答否，而那是真话不是缺省：它连尺寸都没解出来，宽高比那一关无从问起。
    /// 「候选而没切开」怎么读，见 [`Processed::spread_candidate`]。
    pub fn spread_candidate(&self) -> bool {
        self.outcome
            .processed()
            .is_some_and(|page| page.spread_candidate)
    }

    /// 这一页的目标尺寸是**兜底上界**改出来的吗（07 号票）。
    ///
    /// 失败页答否，而那是真话不是缺省：它连目标尺寸都没算过，那张占位页按卷内统一尺寸出
    /// （见 [`PageReport::size`]）。这里因此不是 `Option`——「这一页退回过吗」
    /// 对每一页都问得出口，而另外几项（缩放、裁边、判定）在失败页上根本不存在。
    pub fn backstopped(&self) -> bool {
        self.outcome
            .processed()
            .is_some_and(|page| page.backstopped)
    }

    /// 这一页失败的原因。处理成了的页是 `None`。
    pub fn failure(&self) -> Option<&str> {
        match &self.outcome {
            PageOutcome::Failed { reason } => Some(reason),
            PageOutcome::Whole(_) | PageOutcome::Salvaged { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 段外那一截是**减出来的**，而且减不出负数（加固批 11 号票）。
    ///
    /// 前半句在集成用例里量得到（`tests/timing.rs`），后半句量不到：那要一份段与段重叠的计时，
    /// 而管线不产出这种东西。它却是 [`VolumeTiming::outside_the_segments`] 的实义所在——
    /// `Duration` 的减法在下溢时恐慌，报告里一处掐表出岔子不该让整趟当场炸掉。
    #[test]
    fn what_falls_outside_the_three_segments_is_the_remainder_and_never_negative() {
        let timing = VolumeTiming {
            fingerprint: Duration::from_secs(1),
            first_pass: Duration::from_secs(2),
            second_pass: Duration::from_secs(3),
            elapsed: Duration::from_secs(10),
        };
        assert_eq!(timing.outside_the_segments(), Duration::from_secs(4));

        // 三段之和大于总耗时：段与段重叠了。答一个零，不恐慌——
        // 而三段之和大于 `elapsed` 这件事，调用方自己加一遍就看得出来。
        let overlapping = VolumeTiming {
            elapsed: Duration::from_secs(1),
            ..timing
        };
        assert_eq!(overlapping.outside_the_segments(), Duration::ZERO);
    }
}
