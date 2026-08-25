//! `run` 的输出。

use std::path::PathBuf;

use crate::cache::CacheUsage;
use crate::color::PageColor;
use crate::decide::{CandidateScore, Verdict};
use crate::envelope::Envelope;
use crate::geometry::{GeometryGate, Size};
use crate::medium::IoPlan;
use crate::profile::Profile;
use crate::quantize::{Candidate, Dither};
use crate::resample::Scaling;

/// 一次处理调用的结果。
///
/// spec 固定的形状里还有计时，它随自己那张票落地。**失败页集**是 [`Report::failures`]：
/// 它不另占一个字段，因为失败页本来就在它那一卷的 [`VolumeReport::pages`] 里按阅读顺序占着位——
/// 再存一份就有两个出处，而两处早晚会走散（12 号票）。
#[derive(Debug, Clone)]
pub struct Report {
    /// 本次实际使用的 profile 与它的面板。这批输出该拿去哪台设备看，答案在这里。
    pub profile: Profile,
    pub volumes: Vec<VolumeReport>,
}

impl Report {
    /// 本次每一个失败页，按卷序、卷内按阅读顺序（spec 的 story 26）。
    ///
    /// 页自己说得出它是哪一卷的哪一页——`source` 是卷根接上成员的相对路径。
    pub fn failures(&self) -> impl Iterator<Item = &PageReport> {
        self.volumes.iter().flat_map(VolumeReport::failures)
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
    /// 按阅读顺序排列的页。
    pub pages: Vec<PageReport>,
    /// 本卷的卷级判定：这一卷的候选从哪来。抖动模式在这个候选里。
    ///
    /// 一页都没有的卷（只装着透传文件）是 `None`：那样的卷没有候选可判。
    pub verdict: Option<VolumeVerdict>,
    /// 本卷的几何门判定（ADR 0007：抖动仅在目标尺寸未被下游缩放时启用）。
    ///
    /// 它不在 [`verdict`](Self::verdict) 里：门是几何的、判定是内容的，门先判，
    /// 判定在门放行的那套候选上做。门关着时抖动整体关闭，`verdict` 里那个候选
    /// 于是必然不抖——不说门，报告就解释不了「为什么这一卷没抖」。
    ///
    /// 幂等命中而跳过的卷是 `None`：门是算出来的，而那一趟一页都没算
    /// （见 [`VolumeVerdict::Skipped`]）。一页都没有的卷不是 `None` 而是 `Holds`——
    /// 没有页去关它，那是真话，不是「不知道」。
    pub gate: Option<GeometryGate>,
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
    /// 这个数因此恒等于页数。它在报告里，是为了让那条不变量在 `run` 这个 seam 上量得出来——
    /// 第二遍一旦回头碰源页，它立刻大于页数，而别的外部可见事实都察觉不到这件事。
    pub decodes: usize,
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
    /// `page_count` 是这一卷的页数，数的是**源**那一侧——不做工作也数得出来。
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
    /// 跳过这件事在本结构里由三处一起体现——卷级判定是 [`VolumeVerdict::Skipped`]、
    /// `gate` 为空、`pages` 为空。**它们只有一个来源**，就是这里：读的那一端各自去认
    /// 其中一处，迟早会有人认错一处。
    pub fn skipped(&self) -> bool {
        matches!(self.verdict, Some(VolumeVerdict::Skipped { .. }))
    }

    /// 本卷的页数。失败页也算——它在输出里占着自己那一格。
    ///
    /// 跳过的卷没有逐页结果，页数从卷级判定里取——它是**源**那一侧的事实，
    /// 不做工作也数得出来，报告不该因为跳过就说这一卷是 0 页。
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
    pub size: Size,
    /// 这一页有没有处理成，处理成了的话它走了哪条分支。
    pub outcome: PageOutcome,
}

/// 一页的处理结果：成了，还是失败了（12 号票）。
///
/// 这一层与 [`PageBranch`] 问的不是同一件事：这里问「这一页解出来了吗」，那里问
/// 「解出来之后它走了哪条路」。合成一个枚举就得回答「失败页走的是哪条分支」，
/// 而那个问题没有答案——分支是解码之后才分的。
///
/// 缩放与彩页识别落在 [`Processed`](Self::Processed) 里而不在 [`PageReport`] 上，
/// 因为失败页两样都没有：它没被缩放过，也没人看得出它是不是彩页。
/// 摆一个 `PageColor::Gray` 上去是编的，报告不该有编出来的字段。
#[derive(Debug, Clone)]
pub enum PageOutcome {
    /// 处理成了的一页。
    Processed {
        /// 这一页实际走过的缩放：总缩放比、预缩倍数、残差比（ADR 0001）。
        ///
        /// 预缩这条路径在 B 类素材上从不触发（见 measurements 的《B 类素材普查》），
        /// 报告里说清楚它有没有触发，是这条路径在真实素材上唯一的现场证据。
        scaling: Scaling,
        /// 第一遍识别出这一页是彩页还是灰度页（ADR 0005 决定第 1 条：彩页识别在第一遍）。
        ///
        /// 这是**页的事实**，与它走了哪条分支不是一回事：黑白 profile 下彩页转灰、
        /// 走的是 [`PageBranch::Gray`]，但它仍然是一张彩页。两者都在报告里，
        /// 「这一页为什么没保留颜色」才答得出来。
        color: PageColor,
        /// 这一页走的那条分支，连同那条分支的产物。
        branch: PageBranch,
    },
    /// 失败页：字节读不出来，或完整尺寸解不出来，或尺寸解得出来而像素缓冲大到分配不下
    /// （12 号票，三种判据见 `decode`）。
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
    /// 这一页定下的候选与理由。彩色分支与失败页都没有——一个不量化，一个没解出来。
    pub fn verdict(&self) -> Option<Verdict> {
        match self.branch() {
            Some(PageBranch::Gray { verdict, .. }) => Some(*verdict),
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
        match &self.outcome {
            PageOutcome::Processed { branch, .. } => Some(branch),
            PageOutcome::Failed { .. } => None,
        }
    }

    /// 第一遍认出这一页是彩页还是灰度页。失败页没解出来，看不出。
    pub fn color(&self) -> Option<PageColor> {
        match &self.outcome {
            PageOutcome::Processed { color, .. } => Some(*color),
            PageOutcome::Failed { .. } => None,
        }
    }

    /// 这一页实际走过的缩放。失败页没被缩放过。
    pub fn scaling(&self) -> Option<Scaling> {
        match &self.outcome {
            PageOutcome::Processed { scaling, .. } => Some(*scaling),
            PageOutcome::Failed { .. } => None,
        }
    }

    /// 这一页失败的原因。处理成了的页是 `None`。
    pub fn failure(&self) -> Option<&str> {
        match &self.outcome {
            PageOutcome::Failed { reason } => Some(reason),
            PageOutcome::Processed { .. } => None,
        }
    }
}
