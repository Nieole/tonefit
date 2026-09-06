//! 界面文案：终端上印出来的那一套措辞，命令行与会话共用。
//!
//! 它在**界面层**，不进库：措辞是给人读的，库那一侧只出数据。位置在二进制 crate 内——
//! `lib.rs` 顶上那张模块清单之外的 `src/*.rs` 都是这一侧的。
//!
//! **一处有理由的例外**：互锁那几句话在库里（`tonefit::Interlock` 的 `Display`）。
//! 同一句要从三张嘴里出来——报告抬头、`--help`、以及那条拒绝的错误，
//! 而最后一张嘴在库内；挤在这里就给库里那一份留了第二个出处（理由写在那个模块的文档里）。
//! 本模块对它只做**呈现**：话落在哪一段、前面挂什么标签（见 [`interlock_lines`]）。
//!
//! 大头是把 [`Report`] 渲染成四段，调用方各取所需：[`header`] 一趟只出一次，
//! [`volume`] 与 [`pages`] 逐卷出，[`tail`] 收在末尾。会话攒到哪儿画到哪儿——
//! 卷级事件带着 `VolumeReport`，那一卷跑完就画得出它那一段（ADR 0011）。
//! 卷级与逐页分成两个函数，是因为会话的报告区**展开一枝**才逐卷、**展开一卷**才逐页。
//!
//! 它们上面还有一级：[目录那一行](directory)——卷按目录分组（[`grouped`]），
//! 一枝一行（`volume-discovery/08`）。**分组与聚合只有那两处出处**：
//! 命令行那一副的折叠与会话的目录表读的是同一份，两边不许各算各的。
//!
//! # 措辞一处，排版两处（ADR 0016）
//!
//! 抬头与末尾那两段**本来就是成句的**，出的仍是文字。卷级与逐页那两段出的是一组
//! [行](Row)——每一行带着[它是什么行](RowKind)，以及若干[格](Cell)。
//!
//! **在这里的是「一件事怎么说」**：一个值怎么写、成句的那几段怎么措辞。
//! **不在这里的是「摆在哪儿」**：缩进、前缀、分隔与单位跟着排版走，一副一份。
//! 纯文本那一副在 [`plain`]——命令行印出去的、以及会话眼下画的就是它；
//! 表那一副是会话自己的事（P3 的卷表与逐页表）。
//!
//! 两边拿的是同一份数据，措辞因此只有一套。同一个理由把[标定图那几行](calibration_note)
//! 也收在这里：它不从报告来，但它同样是界面文案，会话按键出图时印的也是它。
//!
//! # 折行不在这里
//!
//! 这里出的是**一段一段的话**，一行长到几百格也照旧是一行。**折到多宽由印它的那一头定**，
//! 折法只有一套（[`crate::wrap`]，各处折到多宽见那个模块）。收在这里的话，
//! 两个去处就得共用一个宽度，而它们一个不知道终端多宽、一个每帧都知道。
//!
//! 这里做的只有一件与折行有关的事：**说出哪一个空格是记号里面的**
//! （[`crate::wrap::HARD_SPACE`]，规矩在那一处）。

use std::path::{Path, PathBuf};

use tonefit::{
    CandidateScore, Mode, NonVolumeReason, PageBranch, PageColor, PageReport, Profile, Report,
    Voice, VolumeFailure, VolumeReport, VolumeVerdict, aggregation, composition,
};
// 收场那一句只有会话读得到（见 [`outcome`]），这两个类型因此跟着它一起挂在特性后面。
#[cfg(feature = "tui")]
use tonefit::{Instruction, RunOutcome};
// 「哪几页要紧」同理（见 [`Notable`]），只是它连 `--no-default-features` 那一趟的用例
// 一起要，因此挂的是 `any(feature = "tui", test)`。
#[cfg(any(feature = "tui", test))]
use tonefit::{GeometryGate, Panel, Reason};

use crate::wrap::HARD_SPACE;

/// **命令行那一侧的拼装**：把[行](Row)与[格](Cell)摆成纯文本那一副（ADR 0016）。
pub mod plain;

/// 报告上的**一行**：[它是什么行](RowKind)，加上若干[格](Cell)。
///
/// 卷级（[`volume`]）与逐页（[`pages`]）那两段出的就是它。一行对应印出来的一行，
/// 而**缩进、前缀与换行不在里面**——那些跟着排版走（ADR 0016）。
///
/// **成句的那几行只有一格**，装着整句话（[`Field::Sentence`]）：它们本来就是句子，
/// 拆开没有意义，画它的那一头把它当整段文字折行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 这是什么行。摆法、行首记号与颜色都从它来。
    pub kind: RowKind,
    /// 这一行的那几格，按印出来的先后。
    pub cells: Vec<Cell>,
}

impl Row {
    /// 一行，收下它那几格。
    fn new(kind: RowKind, cells: Vec<Cell>) -> Self {
        Self { kind, cells }
    }

    /// 只有一格的那种行，多半是[成句的那一种](Field::Sentence)。
    fn one(kind: RowKind, cell: Cell) -> Self {
        Self::new(kind, vec![cell])
    }

    /// 取一格的文字。**不在场就是 `None`**——一格在不在场本身就是一句话：
    /// 一张彩页都没有的卷没有彩页那一格，跳过的卷没有基准档那一格。
    pub fn cell(&self, field: Field) -> Option<&str> {
        self.cells
            .iter()
            .find(|cell| cell.field == field)
            .map(|cell| cell.text.as_str())
    }
}

/// 一**格**：它是[哪一格](Field)，以及那一格怎么说。
///
/// 装的是**值**：单位、列头与分隔符都不在里面，那些跟着排版走（ADR 0016）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// 这是哪一格。
    pub field: Field,
    /// 这一格怎么说。
    pub text: String,
}

impl Cell {
    /// 一格。
    fn new(field: Field, text: impl Into<String>) -> Self {
        Self {
            field,
            text: text.into(),
        }
    }
}

/// 一行是**什么行**。
///
/// 排版按它分派：纯文本那一副按它定缩进与前缀（见 [`plain`]），
/// 表那一副按它挑列、上色、给行首记号。
///
/// 一个变体对应报告上的一种行，**不多不少**：`--per-page` 与覆盖顶掉判定各是一种，
/// 而不是「卷级那一行的三种写法」——表要照它们各自的说法填基准档那一列，
/// 分不开就得回头去认字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// **目录那一行**：这一枝底下几卷、基准档怎么分布、几卷进了隔离
    /// （`volume-discovery/08`）。
    ///
    /// 它比[卷那一行](Self::Volume)高一级：报告区默认摆的是它，展开一枝才逐卷。
    /// 出处只有 [`directory`]。
    Directory,
    /// 卷那一行：去处与页数。
    Volume,
    /// 过期副本那一行（成句）。
    Superseded,
    /// 幂等命中而跳过那一行（成句）。
    Skipped,
    /// 隔离那一行（成句）。
    Isolated,
    /// 部分救回那一行（成句）。
    Salvaged,
    /// 几何门那一行：判定范围、不成立几页、本卷抖不抖。
    Gate,
    /// 几何门那一行底下的注解（成句）。
    GateNote,
    /// 卷级判定：上包络。
    Envelope,
    /// 上包络指出的定档页。
    Driver,
    /// 卷级判定：覆盖项顶掉了判定（成句）。
    Override,
    /// 卷级判定：`--per-page`，没有卷级基准档（成句）。
    PerPage,
    /// 这一趟怎么读的。
    Reading,
    /// 开工前摊到临时目录的那一笔。
    Extraction,
    /// 缓存用量。
    Cache,
    /// 一页的几何那一行：尺寸怎么来的、写到哪儿。
    PageGeometry,
    /// 一页的判定那一行：灰度路径。
    PageVerdict,
    /// 一页的判定那一行：彩色分支（成句）。
    PageColor,
    /// 一页失败那一行（成句）。
    PageFailure,
    /// **一整卷没做成**那一行（成句）：它连 [`VolumeReport`] 都没有，
    /// 报告正文里因此一行都没有（见 [`failed_volume_tail`]）。
    ///
    /// 它与[隔离](Self::Isolated)是两件事，`CONTEXT.md` 的《失败》分得很清楚：
    /// 那一种是卷交出来了、带着坏页，这一种是卷根本没交出来。
    FailedVolume,
}

/// 一格是**哪一格**。
///
/// 挑值靠它，不靠位置：一行上哪几格在场随这一卷、这一页而变
/// （见 [`Row::cell`]），按位置数格早晚数错。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// 成句的那一份：整句话，不再拆。
    Sentence,
    /// 源：卷的路径，或定档页的路径。
    Source,
    /// 去处：卷的输出目录，或一页的输出文件。
    Output,
    /// 输出页数。
    PageCount,
    /// **一枝底下几卷**（目录那一行）。没做成的那几卷也算在里面。
    VolumeCount,
    /// **基准档分布**：一枝底下各档各有几卷，排成一串（目录那一行）。
    /// **一卷都判不出档位就不在场。**
    Bases,
    /// **一枝底下几卷进了隔离**（目录那一行）。**一卷都没有就不在场。**
    Isolated,
    /// 彩页几张。**一张都没有就不在场。**
    ColorPages,
    /// 几何门的判定范围有几页。
    GateScope,
    /// 判定范围里几页不成立。
    GateBroken,
    /// 本卷抖不抖。
    Dither,
    /// 基准档。**表要的就是这一格**，而成句的那一格里说的是同一个数。
    Base,
    /// 上包络那一整句（四个数与「均未标定」那一句，出处在库里）。
    Envelope,
    /// 一个候选：覆盖顶掉判定时是那个覆盖值，逐页判定时是这一页判成的那一档。
    Candidate,
    /// 判成这一档的理由。
    Reason,
    /// 各候选的判据值排成一串。
    Scores,
    /// 这一趟怎么读的。
    Reading,
    /// 缓存用量。
    Cache,
    /// 一页的输出尺寸。
    Size,
    /// 裁边裁掉了多少。**一个像素都没裁就不在场。**
    Crop,
    /// 缩放怎么算的；失败页说的是它的尺寸从哪来。
    Scaling,
    /// 跨页切出来的哪一半。**不是切出来的就不在场。**
    Cut,
    /// 兜底上界退回过。**没退回过就不在场。**
    Backstop,
    /// 部分救回了多少。**不是部分救回的页就不在场。**
    Salvage,
    /// 这一页是彩页转灰。**不是就不在场。**
    ColorToGray,
}

/// 抬头：这批输出给哪台设备、页尺寸照哪种适配方式算出、判据是怎么聚合出来的，
/// 以及这一趟写不写盘。
///
/// 一趟只出一次。它吃的是整份报告而不是单独一个 profile——报告是**逐卷攒出来的**
/// （ADR 0011），攒到一半的那一份同样答得出这几件事。
pub fn header(report: &Report, mode: Mode) -> String {
    let mut text = format!("profile {}\n", report.profile);
    // 这一趟的页尺寸是照哪条规矩算出来的（页几何批 01 号票）。它自成一行、不接在 profile
    // 后面：适配方式是**读法偏好**，不是这块面板的物理事实（`CONTEXT.md` 的《几何》）。
    // 非说不可，是因为两种方式在普通漫画页上产出同一个尺寸——光看页尺寸分不出走的是哪一条。
    text.push_str(&format!("适配方式 {}\n", report.fit));
    // 裁边同理（页几何批 02 号票）：它改的是**适配之前**的页尺寸，而一卷里可能一页都没裁得动。
    // 「这一趟没开」与「这一卷没什么可裁」在逐页那几行上长得一样，只有这一行分得开。
    // 裁法那两个数跟着印出来，与判据聚合那一行同一条规矩：数摆出来，读的人自己判断。
    text.push_str(&format!("裁边 {}\n", crop_rule(report.crop)));
    // 拆分同理（页几何批 04 号票）：它改的是**这一卷有几页、每一页是哪一块**，
    // 而一卷里可能一张跨页都没有。「这一趟没开」与「这一卷没有跨页」在逐页那几行上
    // 长得一样，只有这一行分得开。阈值与阅读方向跟着印出来，与裁法那两个数同一条规矩。
    text.push_str(&format!("跨页拆分 {}\n", report.split));
    // 这一趟的开关咬上的那几条互锁（页几何批 05 号票）。它接在四个开关后面，
    // 因为它说的正是那四项凑在一起之后的事。
    text.push_str(&interlock_lines(report));
    // 逐页那一行的每个数由两项合成，其中颗粒项那道地板与阈值同一批盲测标定
    // （ADR 0002 决定第 5 条）。判据不再是单一个量，构成因此要说出来，
    // 否则读的人无从判断「1bit+FS 20.279」这样的数是从哪来的。
    text.push_str(&format!("判据构成 {}\n", composition()));
    // 逐页那些「判据 …」的数都是这套取法收出来的，而取法里的 K 还没标定。
    // 它与阈值同一个待遇：数摆出来，没标定这件事跟着摆出来（ADR 0002 决定第 3 条）。
    // 它自成一行、不接在 profile 后面——判据聚合眼下对所有 profile 都一样，不是这台设备的事。
    // 行首是「判据聚合」而不是「判据」：逐页那一行的「判据」说的是**量**，两者不许同名。
    text.push_str(&format!("判据聚合 {}\n", aggregation()));
    if mode == Mode::DryRun {
        text.push_str("dry-run：只算不写，下面的路径都还没落盘\n");
    }
    text
}

/// 抬头那几行里说互锁的那一段：这一趟咬上、**而且处置是「报告抬头提一次」**的那几条
/// （页几何批 05 号票的处置 ①）。
///
/// 一条都没有就一个字都不说，与末尾那几小结同一条规矩——默认那一套开关一条都不咬。
///
/// **筛的是 [`Voice::Header`]，不是「咬上的全部」。** 三条处置各不相同，而哪一条落到
/// 报告上由规则说了算（`tonefit::Interlock::voice`），不由这一层挑：裁边关着那一条
/// 咬上了也不在这里露面，它只进 `--help` 与文档。措辞同理不在这里——同一句话还要从
/// `--help` 与那条拒绝的错误里出来（见 `tonefit::Interlock`），这里只管挂标签与换行。
fn interlock_lines(report: &Report) -> String {
    report
        .interlocks()
        .filter(|interlock| interlock.voice() == Voice::Header)
        .map(|interlock| format!("互锁 {interlock}\n"))
        .collect()
}

/// 一个卷的**卷级**那几[行](Row)：去处与页数、过期副本、判定、这一趟怎么读的、缓存用量。
///
/// 逐页那些行不在里面，它们在 [`pages`]。分成两个函数是给会话用的：报告区默认只给卷级，
/// 展开某一卷才逐页（p1 spec 的《会话：布局与交互》）。命令行两段都要，接连摆下去
/// 就是从前那一整段（[`plain`]）。
///
/// **哪几行在场随这一卷而变**，摆的次序不变：跳过的卷没有几何门、没有卷级判定、
/// 也没有缓存用量——那三样它一样都没算过。
///
/// 那个页数是**输出**页数——用户打开的那本书里躺着几页。源页数是另一个数
/// （`VolumeReport::source_pages`，页几何批 03 号票），一个源页可以产出多张输出页；
/// 两者眼下相等，这一行因此还没有分开说的必要。
pub fn volume(volume: &VolumeReport) -> Vec<Row> {
    let mut rows = vec![Row::new(RowKind::Volume, volume_cells(volume))];
    rows.extend(superseded_row(volume));
    rows.extend(verdict_rows(volume));
    // 这一卷是怎么读的（13 号票）。它排在跳过那一支**之前**：幂等命中的卷同样把整卷的字节
    // 读了一遍，读法与做事的那一趟是同一个，而「跳过一卷为什么也要等这么久」正问在这里。
    rows.push(Row::one(
        RowKind::Reading,
        Cell::new(Field::Reading, volume.io.to_string()),
    ));
    // 开工前摊到临时目录的那一笔（ADR 0015）。它同样排在跳过那一支**之前**，
    // 理由与上一行同一条：幂等那一道要把整卷的字节读一遍，而读之前得先摊开——
    // 跳过的卷付了这笔磁盘，报告里就得说得出。
    rows.extend(extraction_row(volume));
    // 跳过的卷什么都没做：缓存用量与逐页结果无从谈起，`verdict_rows` 那一行已经说完了。
    if volume.skipped() {
        return rows;
    }
    // 卷成为不可分割的处理单元，峰值内存随卷大小走（ADR 0005）：这一行是那条代价的现场。
    rows.push(Row::one(
        RowKind::Cache,
        Cell::new(Field::Cache, volume.cache.to_string()),
    ));
    rows
}

/// 卷那一行的那几格：源、去处、页数，以及彩页几张。
fn volume_cells(volume: &VolumeReport) -> Vec<Cell> {
    let mut cells = vec![
        Cell::new(Field::Source, volume.volume.display().to_string()),
        Cell::new(Field::Output, volume.output.display().to_string()),
        Cell::new(Field::PageCount, volume.page_count().to_string()),
    ];
    cells.extend(color_pages(volume));
    cells
}

/// 一个卷的逐页那些[行](Row)：每页两行，一行[几何](RowKind::PageGeometry)、
/// 一行判定（[灰度](RowKind::PageVerdict)、[彩色分支](RowKind::PageColor)
/// 或[失败](RowKind::PageFailure)三者之一）。
///
/// 跳过的卷一行都不出：那一趟根本没算过逐页结果，摆出任何一项都是编的。
/// 这里问的是 [`VolumeReport::skipped`]——跳过在那个结构上由两处一起体现，
/// 而认哪一处只许有一个出处（见 `VolumeReport::skipped` 的文档）。
/// 拆分之前这道守卫与 [`volume`] 里那道是同一句 `continue`。
pub fn pages(volume: &VolumeReport) -> Vec<Row> {
    if volume.skipped() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for page in &volume.pages {
        rows.push(Row::new(RowKind::PageGeometry, geometry_cells(page)));
        rows.push(page_row(page));
    }
    rows
}

/// 一页**要紧在哪儿**（`CONTEXT.md` 的《会话》：要紧的页）。
///
/// 展开一卷的目的通常只有一个——**哪一页把整卷拉下来**——而两百页的卷里那几页不该由
/// 用户自己在四百行里找（`p3-session-legibility/11`）。这个枚举就是「哪几页」那一问的
/// **判据**，[`notable`] 是问它的唯一入口：屏上默认列的就是要紧在某一处的那些页
/// （`crate::session::draw::pages`）。
///
/// **七种，不多不少。** 前六种逐条对上 `CONTEXT.md` 的《语义色》在**页**这一级上列出的
/// 那几样——[失败页](Self::Failed)归「出事」，另外五样归「注意」；末一种
/// （[定档页](Self::Driver)）一档都不占，它不是出了什么事，而是**这一卷的答案**：
/// 基准档就是它判出来的，非在不可。
///
/// 一页可以同时要紧在**好几处**（[`notable`] 因此逐页出的是一列）：以高为准的跨页卷里，
/// 定档页多半也是一张[宽溢出的页](Self::Overflowed)。
///
/// **有几对凑不到一起**，而这里不去守着它们——那是库那一侧的事实，不是这一处的判据：
/// 退回过[兜底上界](Self::Backstopped)的页恒不超过面板宽
/// （[`Report::backstopped`](tonefit::Report::backstopped) 写着两张清单不重叠），
/// [特例页](Self::Outlier)与[定档页](Self::Driver)也碰不到面（定档页站在其余页那一组上，
/// 而特例页不在那一组里）。
///
/// 屏上那个词与那个行首记号在画法那一层配（`crate::session::draw::pages`）：
/// 这里只答「要不要紧」，不答「屏上写哪两个字」。
///
/// **读它的只有会话**（默认那一副只列要紧的页；命令行那一路一页不落地全印），
/// 它因此与 [`failing_pages`] 同一副写法挂在特性后面。挂的是 `any(feature = "tui", test)`
/// 而不是光 `feature = "tui"`：判据摆在**终端库外面**，`--no-default-features`
/// 那一趟照跑它自带的用例（闸门的第二条）——而 `tui` 关掉的非测试那一趟没人读它。
/// 同一副写法见 `src/main.rs` 那一句 `mod session`。
#[cfg(any(feature = "tui", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notable {
    /// 失败页：这一页根本没解出来，输出里躺着的是一张留白占位页。
    Failed,
    /// 部分救回页：解到哪个像素算哪个像素，缺的那一段留成纸白。
    Salvaged,
    /// 特例页：判据显著偏离卷内分布，不参与上包络、单独定档（ADR 0006 决定第 5 条）。
    Outlier,
    /// 几何门不成立的页：源比目标小，抖动被单独关掉（ADR 0007 决定第 2、3 条）。
    OutsideTheGate,
    /// 宽溢出的页：输出宽超过面板宽，翻它要阅读器横向平移。
    Overflowed,
    /// 目标尺寸被**兜底上界**改过的页：它没按这一趟点名的适配方式出，退回了 fit-inside。
    Backstopped,
    /// 定档页：卷级上包络站在它身上，**基准档就是它的判定**。
    Driver,
}

/// 这一卷逐页各**要紧在哪几处**：与 [`VolumeReport::pages`] **等长同序**，
/// 一页一列，空的那一列就是「这一页不要紧」。
///
/// **判据只有这一处**（见 [`Notable`]）：屏上默认列哪几页、行首挂什么记号、抬头上
/// 「要紧的页有几张」那个数，三处问的都是它。
///
/// 六种里五种问得到页自己身上（失败、部分救回、几何门、兜底、判定理由），另外两种要
/// **卷与面板**才答得出：宽溢出比的是这一页的输出宽与 `panel` 的宽（与
/// [`Report::wider_than_the_panel`](tonefit::Report::wider_than_the_panel) 同一条判据，
/// 那一头数的是整趟、这一头数的是一卷）；定档页是这一卷上包络指出的那一页
/// （[`Envelope::driver`](tonefit::Envelope::driver) 的序号指进 `volume.pages`）。
///
/// 跳过的卷出的是**空的一列**：它一页都没重做，逐页结果一条都没有
/// （与 [`pages`] 那道守卫同一件事）。
///
/// 它与 [`Notable`] 一起挂在特性后面，理由写在那一头。
#[cfg(any(feature = "tui", test))]
pub fn notable(volume: &VolumeReport, panel: Panel) -> Vec<Vec<Notable>> {
    // 定档页只有上包络定出来的卷才有：`--per-page` 与覆盖那两种卷级判定里没有一页
    // 站在分位秩上（见 `tonefit::VolumeVerdict`）。
    let driver = match volume.verdict {
        Some(VolumeVerdict::Envelope(envelope)) => Some(envelope.driver),
        _ => None,
    };
    volume
        .pages
        .iter()
        .enumerate()
        .map(|(at, page)| {
            let mut why = Vec::new();
            if page.failure().is_some() {
                why.push(Notable::Failed);
            }
            if page.salvage().is_some() {
                why.push(Notable::Salvaged);
            }
            if page.verdict().map(|verdict| verdict.reason) == Some(Reason::Outlier) {
                why.push(Notable::Outlier);
            }
            if page.gate() == Some(GeometryGate::Broken) {
                why.push(Notable::OutsideTheGate);
            }
            if page.size.width > panel.resolution.width {
                why.push(Notable::Overflowed);
            }
            if page.backstopped() {
                why.push(Notable::Backstopped);
            }
            if driver == Some(at) {
                why.push(Notable::Driver);
            }
            why
        })
        .collect()
}

/// **一整卷没做成**的那一[行](Row)（05 号票的那几卷，Q133）。
///
/// 它不从 [`VolumeReport`] 来——没做成的卷连一份卷报告都没有，只在
/// `Report::failed_volumes` 里躺着路径与一句原因。[`volume`] 因此够不着它，
/// 而**卷表要给它一行**（spec 的《卷表》），于是单出这一个函数。
///
/// 出的是[行](Row)而不是一段文字，为的是那一句原因只有一处出处：
/// 报告末尾那一小结（[`failed_volume_tail`]）摆的就是这一行，
/// 会话的卷表摆的也是它——两副排版，一套措辞（ADR 0016）。
pub fn failed_volume(failure: &VolumeFailure) -> Row {
    Row::new(
        RowKind::FailedVolume,
        vec![
            Cell::new(Field::Source, failure.volume.display().to_string()),
            Cell::new(Field::Sentence, failure.reason.clone()),
        ],
    )
}

/// 卷表**档位那一列**上写什么：这一卷判成的那一档，或者它**为什么没有一档**。
///
/// 收的是 [`volume`]（或 [`failed_volume`]）出的那几行：这一卷是哪一种判定已经由
/// [`RowKind`] 分好了，回头去认字符串就是第二个出处。一种都对不上时给 `None`——
/// 一张灰度页都没有的卷（整卷彩页、整卷失败）根本没有候选可判，那一格因此不在场。
///
/// **五种说法只有这一处**：它们与卷级那几行成句的那几句说的是同一件事
/// （「跳过 幂等命中……」「无（--per-page）……」「判定 X（覆盖项裁到只剩一个候选）」），
/// 表那一副只是把它压成一列摆得下的宽度，不编第二套说法（spec 的《卷表》）。
///
/// **两路都读它**：会话的卷表把它摆成一列，而目录那一行的[基准档分布](directory)
/// 逐卷问它一遍——命令行那一副的折叠与会话的目录表因此数的是同一批字
/// （`volume-discovery/08`）。卷级那几行摆成散文的那一路本身不分列，不读它。
pub fn base_column(rows: &[Row]) -> Option<String> {
    rows.iter().find_map(|row| match row.kind {
        // 判出了基准档的那一种：那一格就是它（[`Field::Base`] 的文档说的正是这一处）。
        RowKind::Envelope => row.cell(Field::Base).map(str::to_owned),
        RowKind::Override => row
            .cell(Field::Candidate)
            .map(|candidate| format!("覆盖 {candidate}")),
        RowKind::PerPage => Some("逐页".to_owned()),
        RowKind::Skipped => Some("跳过".to_owned()),
        RowKind::FailedVolume => Some("没做成".to_owned()),
        // 其余各行说的不是这一卷的档位。这里留 `_` 是对的：行的种类还会长
        // （每多一种页的行都要在这里加一条 `None`），而**说得出档位的只有上面那几种**——
        // 漏掉一种的后果是那一格不在场，屏上当场看得见，不是悄悄印错一个档。
        _ => None,
    })
}

/// 报告上**目录那一级要数的一条**：一个卷根，加上它这一趟交出了什么。
///
/// **两种，不多不少**（`CONTEXT.md` 的《失败》把它们分得很清楚）：
/// [收摊了的那几卷](Self::Settled)带着一份卷报告，[没做成的那几卷](Self::Failed)
/// 连一份都没有，只有一个路径与一句原因。目录那一行要数的两件事各要一种——
/// 卷数把两种都算进去，[基准档分布](directory)照 [`base_column`] 逐条问，
/// 而那一处对没做成的卷答的正是「没做成」。
///
/// **收的是引用，一条报告都不复制**：报告一趟能有几百卷，而分组只要认得出
/// 每一条挂在树上的哪一枝（[`grouped`]）。
#[derive(Debug, Clone, Copy)]
pub enum Listed<'a> {
    /// 收摊了的一卷。
    Settled(&'a VolumeReport),
    /// 一整卷没做成的那一卷。
    Failed(&'a VolumeFailure),
}

impl Listed<'_> {
    /// 这一条的**卷根**：目录路径，或归档文件路径。分组按它的父目录（见 [`grouped`]）。
    pub fn root(&self) -> &Path {
        match self {
            Self::Settled(volume) => &volume.volume,
            Self::Failed(failure) => &failure.volume,
        }
    }

    /// 这一条在[基准档那一列](base_column)上写什么。**目录那一行的分布逐条问它**，
    /// 不另编一套说法。
    ///
    /// **只建[判定那几行](verdict_rows)，不把整卷现一遍**：[`base_column`] 认的四种行
    /// （上包络 · 覆盖 · 逐页 · 跳过）全出自那一处，而 [`volume`] 还要把两条路径与
    /// 读法、缓存各摆一遍——目录那一行逐卷问它，几百卷的一趟每帧都要付那一笔。
    /// 认哪几种行仍旧只有 [`base_column`] 一处，这里只是少喂它几行它本来就不看的。
    fn base(&self) -> Option<String> {
        match self {
            Self::Settled(report) => base_column(&verdict_rows(report)),
            Self::Failed(failure) => {
                base_column(std::slice::from_ref(&self::failed_volume(failure)))
            }
        }
    }

    /// 这一条进没进隔离。没做成的那一卷**不算**：它连一份卷报告都没有，
    /// 也就没有一页失败可言（两者的分别见 `CONTEXT.md` 的《失败》）。
    fn isolated(&self) -> bool {
        match self {
            Self::Settled(report) => report.isolated(),
            Self::Failed(_) => false,
        }
    }
}

/// 报告上收得住的那几条，**按报告上的先后**：收摊了的那几卷，然后是没做成的那几卷。
///
/// 命令行那一路收的就是它。会话另拼一份（决策点上攒着的那一份也占一条，
/// 见 `crate::session::live::Live::branches`）——**拼法两处，分组只有一处**（[`grouped`]）。
pub fn listed(report: &Report) -> Vec<Listed<'_>> {
    report
        .volumes
        .iter()
        .map(Listed::Settled)
        .chain(report.failed_volumes.iter().map(Listed::Failed))
        .collect()
}

/// 一个**目录**：树上的那一枝，加上它收下了哪几条。
///
/// `at` 是下标，指回交给 [`grouped`] 的那一列——分组不搬运卷报告，
/// 调用方拿下标去取自己那一头的东西（会话要的是「表上第几卷」，命令行要的是那一份报告）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// 这一枝是哪个目录。
    pub directory: PathBuf,
    /// 它收下的那几条各是第几条，按传进来那一列的先后。
    pub at: Vec<usize>,
}

/// 卷按**目录**分组：一个目录一组（`volume-discovery/08`）。
///
/// **唯一出处**：命令行那一副的折叠（[`plain::report`]）与会话的目录表读的是同一份，
/// 两边不许各算各的。
///
/// 目录取的是卷根的**父目录**，那正是**发现出来那棵树**上它挂着的那一枝：
/// `crate::discover` 列一层目录时，列出来的每一个候选的父目录就是它站着的那一层。
/// 一个目录**既是卷又装着卷**时（ADR 0014），它自己归**它的父目录**那一组、
/// 它装着的那几卷归它自己那一组——层次因此与那棵树逐层对得上，
/// 而不是在这里照路径字符串另切一棵。
///
/// **不重排**（与卷表同一条，`CONTEXT.md` 的《会话》：卷表）：目录按它**头一条**
/// 在报告上的先后排，组内也按报告上的先后。发现走的是先序深度优先，
/// 同一枝底下那几卷因此**不一定在报告上挨着**（中间夹着子目录那几卷）——
/// 分组把它们收到一处，而收进来的次序仍是报告上的次序。
pub fn grouped(listed: &[Listed<'_>]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (at, one) in listed.iter().enumerate() {
        let directory = directory_of(one.root());
        match groups.iter_mut().find(|group| group.directory == directory) {
            Some(group) => group.at.push(at),
            None => groups.push(Group {
                directory: directory.to_path_buf(),
                at: vec![at],
            }),
        }
    }
    groups
}

/// 一个卷根挂在树上的**哪一枝**：它的父目录。
///
/// **取不出父目录时落到当前目录（`.`）**：点名一个裸文件名（`tonefit 第1话.cbz`）
/// 那一卷躺的正是当前目录。取它自己当一枝的话，报告上会连着两行同一个名字——
/// 一行当目录、一行当卷，而那两行说的不是同一件东西。
fn directory_of(root: &Path) -> &Path {
    root.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// **目录那一行**：这一枝底下几卷、基准档怎么分布、几卷进了隔离。
///
/// 一趟几百卷时报告区默认摆的就是它，一个目录一行（`volume-discovery/08` 票面第一条）；
/// 命令行那一副把它摆在这一枝那几卷**前面**（[`plain::report`]）。
/// **两边拿的是同一份**：聚合与措辞都在这一处。
///
/// **哪几格在场随这一枝而变**：一卷都没判出档位的目录没有分布那一格，
/// 一卷都没进隔离的目录没有隔离那一格——一格在不在场本身就是一句话（见 [`Row::cell`]）。
///
/// 卷数把**没做成的那几卷也算进去**：它们同样是这一枝底下点到过的卷，
/// 而「几卷没做成」由分布那一格说（[`base_column`] 对它们答的是「没做成」）。
pub fn directory(group: &Group, listed: &[Listed<'_>]) -> Row {
    let inside: Vec<Listed<'_>> = group
        .at
        .iter()
        .filter_map(|at| listed.get(*at).copied())
        .collect();
    let mut cells = vec![
        Cell::new(Field::Source, group.directory.display().to_string()),
        Cell::new(Field::VolumeCount, inside.len().to_string()),
    ];
    let spread = base_spread(&inside);
    if !spread.is_empty() {
        cells.push(Cell::new(Field::Bases, spread));
    }
    let isolated = inside.iter().filter(|one| one.isolated()).count();
    if isolated > 0 {
        cells.push(Cell::new(Field::Isolated, isolated.to_string()));
    }
    Row::new(RowKind::Directory, cells)
}

/// 一枝底下**几卷进了隔离**那一句：`隔离 2 卷`。
///
/// **一处出处**：命令行那一副接在目录那一行的行尾，会话的目录表把它当行尾那一句
/// （两处摆的是同一串字，而不是同一个值的两种写法——后者才归排版，见 ADR 0016）。
/// 那个数出自[隔离那一格](Field::Isolated)。
pub fn isolated_note(count: &str) -> String {
    format!("隔离 {count} 卷")
}

/// **基准档分布**：这一枝底下各档各有几卷，多的排在前面。
///
/// 各档怎么写照 [`base_column`]——卷表那一列写的是同一批字，跳过与没做成也在里面。
/// **串起来的那个 `·` 是措辞**，与[判据那一串](score_line)同一条（ADR 0016 决定第 2 条）。
///
/// 排法：**卷多的在前**，一样多的按头一次出现的先后（`sort_by` 是稳定的）。
/// 按报告先后原样排的话，一枝里最常见的那一档常常落在末尾——而扫一眼要看出来的
/// 正是「这一枝多半判成哪一档」。
fn base_spread(inside: &[Listed<'_>]) -> String {
    let mut counted: Vec<(String, usize)> = Vec::new();
    for one in inside {
        let Some(base) = one.base() else {
            continue;
        };
        match counted.iter_mut().find(|(said, _)| *said == base) {
            Some((_, count)) => *count += 1,
            None => counted.push((base, 1)),
        }
    }
    counted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counted
        .iter()
        .map(|(said, count)| format!("{said} {count}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 末尾那六小结：非卷文件、输出宽超过面板、兜底上界退回、部分救回、隔离、卷级失败。
/// 各自一个（一页、一卷）都没有就一个字都不说。
///
/// 它们要看完整趟才给得出来，因此不进 [`volume`]：那几行数的是**这一趟**有几卷几页，
/// 而不是这一卷。
///
/// 次序按**这一趟出的事有多重**往下排，最重的压在末尾——终端上它离提示符最近，
/// 也是几十卷跑下来最不该被往回翻的那一条。非卷文件因此打头：它是这一列里唯一
/// **连失败都不是**的一种（`CONTEXT.md` 的《失败》：非卷文件不是失败），
/// 退出码一格都不动它。
pub fn tail(report: &Report) -> String {
    let mut text = non_volume_tail(report);
    text.push_str(&overflow_tail(report));
    text.push_str(&backstop_tail(report));
    text.push_str(&salvage_tail(report));
    text.push_str(&isolation_tail(report));
    text.push_str(&failed_volume_tail(report));
    text
}

/// 非卷文件那一小结，打头摆在末尾那几小结的最前面（`volume-discovery/04`）。
///
/// 这几个文件在报告正文里**一行都没有**，与卷级失败那一小结同一个处境：正文逐卷那几段
/// 来自 `Report::volumes`，而它们连卷都不是。这一小结因此是它们在报告里唯一的位置——
/// 少了它，用户看到的是「输出目录里没有这些东西」，却不知道是漏了还是本来就不该有。
/// 那正是发现落地之前的老毛病：没被当成页的一律透传，产物里混着源库的杂物，
/// 而报告里没有一处列得出它们（ADR 0014 的《背景》）。
///
/// **打头而不是压尾**：末尾那几小结按出的事有多重往下排，而它是唯一连失败都不是的一种——
/// 退出码一格不动（`CONTEXT.md` 的《失败》）。
///
/// **逐条带上路径与一句为什么**，三类各说各的（见 [`non_volume_reason`]）：只说个数的话，
/// 用户还得自己去源库里对一遍才知道是哪几个。形状照卷级失败那一小结办
/// （见 [`failed_volume_tail`]）：路径一行、原因一行，多了只列前几条并说还有多少——
/// 一屏放不下的清单等于没有清单。截断只发生在**这一层**，`Report::non_volume_files`
/// 一条不少。
///
/// **命令行与会话印的是这一段**，不是两套：报告末尾那几小结两边都走 [`tail`]
/// （见 `crate::session::draw` 的报告区），而数据只有 `Report` 上那一列。
fn non_volume_tail(report: &Report) -> String {
    /// 最多列几条。
    const SHOWN: usize = 5;

    if report.non_volume_files.is_empty() {
        return String::new();
    }
    let mut text = format!(
        "非卷文件 {} 个：发现走完之后没被任何卷收下，既没转也没拷——输出里一个字节都没有。\
         这**不是失败**，退出码一格没变；源库一个字节没动，要它们的话去源里自己拿\n",
        report.non_volume_files.len()
    );
    for file in report.non_volume_files.iter().take(SHOWN) {
        text.push_str(&format!(
            "  {}\n    {}\n",
            file.path.display(),
            non_volume_reason(&file.reason)
        ));
    }
    let rest = report.non_volume_files.len().saturating_sub(SHOWN);
    if rest > 0 {
        text.push_str(&format!("  ……另有 {rest} 个\n"));
    }
    text
}

/// 一个非卷文件**为什么**没被任何卷收下，那一句话。
///
/// 三类分得开，各说各的来由——三者的处置一模一样（不转、不拷），而用户要做的事不一样：
/// 头一种是源库里的杂物，第二种多半是拿错了包，第三种是**真有东西坏了**，
/// 那一条还带着由内到外的错误链。
///
/// 逐个变体都列出来、不留 `_`：[`NonVolumeReason`] 不是非穷尽的，
/// 多一类该怎么说是个要当场拿的主意（同一条规矩见 [`outcome`]）。
fn non_volume_reason(reason: &NonVolumeReason) -> String {
    match reason {
        NonVolumeReason::NeitherPageNorArchive => {
            "它躺着的那一层一页都没有、不是卷，而它自己既不是页也不是归档——没有卷收得下它。\
             同一个文件躺在一个有页的目录里则是那一卷的透传成员，照旧搬进输出容器"
                .to_owned()
        }
        NonVolumeReason::ArchiveWithoutAPage => {
            "一页都没有的归档：一页都没有的东西不是卷，输出里因此没有一个空容器".to_owned()
        }
        NonVolumeReason::Unopenable(reason) => format!(
            "发现出来却点不开：{reason}。点名的路径点不开是整趟拒绝，\
             而它是发现出来的——其余卷照做"
        ),
    }
}

/// 输出宽超过面板的那些页，摆在报告末尾（页几何批 01 号票）。
///
/// 它们要求阅读器**平移、不缩放**才看得全，而用户翻它们时要横向翻动——那是以高为准
/// 认下的代价，用户得知道是哪几页。留边那一侧要不到这句话：填背景不重采样是绝大多数
/// 阅读器的默认行为，而平移不缩放不是。
///
/// 一页都没有就一个字都不说，与另外两小结同一条规矩——普通漫画卷正是这个样子
/// （实测棋魂 0%，见 measurements 的《适配方式：fit-inside 与以高为准》）。页多时只点名头几页：
/// 跨页卷几乎整卷落在这里（哆啦A梦 91%），全列出来只会把报告刷满。
fn overflow_tail(report: &Report) -> String {
    let pages: Vec<&PageReport> = report.wider_than_the_panel().collect();
    let Some(widest) = pages
        .iter()
        .map(|page| page.size)
        .max_by_key(|size| size.width)
    else {
        return String::new();
    };
    let panel = report.profile.panel().resolution.width;
    format!(
        "输出宽超过面板 {} 页：最宽 {widest}，是面板宽 {panel} 的 {:.2} 倍。\
         这些页要阅读器平移着看才读得全——留边那一侧只要它别重采样，这一侧的要求更强。\
         换 --fit{HARD_SPACE}inside 能把它们压回面板以内，代价是跨页重新被压扁\n  {}\n",
        pages.len(),
        f64::from(widest.width) / f64::from(panel),
        first_few_names(&pages)
    )
}

/// **兜底上界**退回去的那些页，紧挨着上一小结（07 号票）。
///
/// 排在「输出宽超过面板」后面，因为两者说的是同一件事的两头：那一头是宽出去了但仍然出得来，
/// 这一头是宽到再走下去整趟都要停。两张清单**不重叠**——退回之后的页恒在面板宽以内。
///
/// 这不是一个开关，用户没得选，报告因此只在真发生时说话，抬头一个字都不提
/// （与裁边那条互锁同一个待遇，见 05 号票的处置 ②）。上界那个数跟着印出来，
/// 与裁法那两个数同一条规矩：数摆出来，读的人自己判断。
///
/// 一页都没有就一个字都不说——真实素材整批都是这个样子（实测最宽的一页只有面板宽的
/// 3.22 倍，离这道线还有 13 倍，见 measurements 的《适配方式：fit-inside 与以高为准》）。
fn backstop_tail(report: &Report) -> String {
    let pages: Vec<&PageReport> = report.backstopped().collect();
    if pages.is_empty() {
        return String::new();
    }
    format!(
        "兜底上界 {} 页：按 {} 算出的目标尺寸越过 {} 像素，这几页改按 fit-inside 出——\
         再照原样算下去那块缓冲分配不下，整趟都要停。够得着这道线的是宽高比极端的源页，\
         不是页上画着什么\n  {}\n",
        pages.len(),
        report.fit,
        tonefit::max_target_pixels(),
        first_few_names(&pages)
    )
}

/// 隔离那一小结，摆在整份报告的末尾。
///
/// 逐页那几行已经把每一个失败页与原因说过一遍了；这一行是给长任务备的：几十卷跑下来，
/// 失败页早滚出屏幕了，而「这一趟到底有没有出事」得有一个不用往回翻的答案。
/// 退出码说的是同一件事（见 `crate::exit_code`），只是那一个给脚本读、这一行给人读。
/// 一卷都没被隔离就一个字都不说。
fn isolation_tail(report: &Report) -> String {
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.isolated())
        .count();
    if volumes == 0 {
        return String::new();
    }
    format!(
        "隔离 {volumes} 卷 · 失败 {} 页：失败页以卷内统一尺寸留白占位，原因逐条列在上面\n",
        report.failures().count()
    )
}

/// 卷级失败那一小结，压在整份报告的最末尾（05 号票）。
///
/// 这几卷在报告正文里**一行都没有**：正文逐卷那几段来自 `Report::volumes`，
/// 而没做成的卷不在那一列里——它没有去处、没有页数、没有判定可印。
/// 这一小结因此不是「再说一遍」，它是这几卷在报告里唯一的位置：
/// 少了它，用户看到的是一份少了几卷而不说为什么的报告，比报错更糟。
///
/// **逐条带上原因**，形状照预扫那条拒绝办（见 `tonefit` 的 `survey`）：路径一行、
/// 原因一行，多了只列前几条并说还有多少。一屏放不下的清单等于没有清单。
/// 截断只发生在**这一层**：`Report::failed_volumes` 一卷不少、每一卷都带着自己那句原因，
/// 要全部的调用方读那一列。
///
/// 排在隔离那一小结之后：两者是同一件事的两个轻重——那一头是卷交出来了、带着坏页，
/// 这一头是卷根本没交出来。退出码上同样是后者压过前者（见 `crate::exit_code`）。
fn failed_volume_tail(report: &Report) -> String {
    /// 最多列几条。
    const SHOWN: usize = 5;

    if report.failed_volumes.is_empty() {
        return String::new();
    }
    let mut text = format!(
        "卷级失败 {} 卷：预扫时打得开，轮到它们时没做成，一个字节都没交出来。\
         这一趟没有因此停下——别的卷该做的照做，上面那些就是做出来的\n",
        report.failed_volumes.len()
    );
    for failure in report.failed_volumes.iter().take(SHOWN) {
        text.push_str(&plain::line(&failed_volume(failure)));
    }
    let rest = report.failed_volumes.len().saturating_sub(SHOWN);
    if rest > 0 {
        text.push_str(&format!("  ……另有 {rest} 卷\n"));
    }
    text
}

/// 部分救回那一小结，与隔离那一小结并排摆在报告末尾（04 号票）。
///
/// 它比隔离那一行更需要这个位置：含失败页的卷有退出码替它喊，也有一个隔离目录摆在那儿；
/// 部分救回页两样都没有——卷照常落在干净的去处，退出码是 0，而源文件确实不全。
/// 几十卷跑下来，逐页那几行早滚出屏幕了，不在末尾说一句就等于没说。
///
/// 这一行只报数，不重复「它们没参与卷级的哪两件事」——那句话在卷级那一行上，
/// 而这一行与它出现在同一份报告里（见 [`salvaged_row`]）。
///
/// 一页都没有就一个字都不说，与隔离那一行同一条规矩。
fn salvage_tail(report: &Report) -> String {
    let pages = report.salvaged().count();
    if pages == 0 {
        return String::new();
    }
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.salvaged().next().is_some())
        .count();
    format!("部分救回 {volumes} 卷 · {pages} 页：源文件不全，缺的那一段留成纸白\n")
}

/// 过期副本那一行（12 号票）。
///
/// 卷的去处随「有没有失败页」在干净目录与隔离目录之间跳，而这一趟写不到的那一份不会被覆盖、
/// 也不会被删。它可能是**一整卷白页**的占位输出——摆在文件管理器里与一本正经的书没有分别。
/// 报告因此要指名道姓地说出它在哪儿，删不删由用户定。
///
/// 这一行排在卷级各行之前：它说的不是这一趟做了什么，而是上一趟留下了什么。
fn superseded_row(volume: &VolumeReport) -> Option<Row> {
    let path = volume.superseded.as_ref()?;
    Some(sentence_row(
        RowKind::Superseded,
        format!(
            "过期副本 {}：上一趟写在那儿，这一趟没有覆盖它。\
             那一份当初若是被隔离过的，它整卷都是白页——删不删由你",
            path.display()
        ),
    ))
}

/// 摊开那一行（ADR 0015 决定第 3 条）。**只有摊开过的卷才有它**。
///
/// 这个数为什么非说不可，写在它那一格上（`tonefit::VolumeReport` 的 `extracted`），
/// 这里不复述。本函数只管**呈现**：
///
/// 恒是 0 的那些卷（`.cbz` / `.zip` 与目录卷）一个字都不说——那一行在它们身上没有信息，
/// 与末尾那几小结同一条规矩。字节数走库那一份进位（[`tonefit::format_bytes`]），
/// 好让这一行与上一行的缓存用量在同一屏上是同一种单位。
fn extraction_row(volume: &VolumeReport) -> Option<Row> {
    if volume.extracted == 0 {
        return None;
    }
    Some(sentence_row(
        RowKind::Extraction,
        format!(
            "摊开 {}（开工前整卷解到临时目录，跑完就收）",
            tonefit::format_bytes(volume.extracted)
        ),
    ))
}

/// 卷级那一段里说部分救回的那一行，排在隔离那一行之后（04 号票）。
///
/// 隔离那一行说的是「这一卷有页根本没出来」，这一行说的是「有页出来了，但不全」。
/// 两句分开，因为后果不同：前者整卷换了去处，后者没有——这一卷仍在干净的去处，
/// 而卷级的档是在**没有**这几页的情况下定出来的，那正是这一行要交代的事。
fn salvaged_row(volume: &VolumeReport) -> Option<Row> {
    let pages = volume.salvaged().count();
    if pages == 0 {
        return None;
    }
    Some(sentence_row(
        RowKind::Salvaged,
        format!(
            "部分救回 {pages} 页：整解失败，按文件头的尺寸救回了一段，缺的那一段留成纸白。\
             它们不参与卷级上包络，各自单独定档。几何门照旧问它们——那是文件头里的真尺寸；\
             门在哪一页上也不成立，那一页就改按门那一条来（见上）"
        ),
    ))
}

/// 抬头那一行里说裁边的那一小截：开着就把裁法那两个数一并印出来。
///
/// 开着的那一趟不多说一个字的利害——那归 `--help` 与文档（页几何批 05 号票的处置 ②）：
/// 抖动被阅读器抹平只在用户的阅读器会裁时才发生，而 tonefit 看不到那一层，逐卷提醒等于噪音。
fn crop_rule(on: bool) -> String {
    if on {
        tonefit::ink_rule().to_string()
    } else {
        "关（页按解出来的原尺寸适配）".to_owned()
    }
}

/// 一页几何那一行的那几格：这一页的尺寸是怎么来的，以及它写到哪儿。
///
/// 那一行顺着「解出来多大 → 裁完多大 → 缩了多少 → 写出多大」读下来，
/// 这几格因此按那个次序摆：裁边在缩放**之前**（裁边发生在适配之前），
/// 跨页与兜底在它**之后**（兜底改的是最后那一步的规矩，07 号票）。
///
/// **四格各自只在真发生过时在场**（见 [`Field`] 上那几条）：
///
/// - **裁边**——一个像素都没裁的页不占这一格。这一趟开没开裁边由抬头那一行说，
///   两件事分得开。失败页也没有：它没有像素可裁。
/// - **缩放**——恒在场。失败页没有缩放可说（ADR 0001 那三个数一个都不成立），
///   那一格于是改说它的尺寸是从哪来的：卷内统一，不是它自己的。
/// - **跨页**——说的是**哪一侧**，不是排第几：排第几看成员名上那个序号就行，
///   而「这张图原来长在页的哪边」没有第二处说得出来（见 `tonefit::Side`）。
///   「这一卷没有跨页」与「整趟没开拆分」在逐页那几行上长得一样，后者由抬头那一行说。
/// - **兜底退回**——末尾那一小结数的是**这一趟**有几页，而且只点名头几页；
///   「哪一页」要逐页翻得到，靠的就是这一格（07 号票：退回这件事逐页可指认）。
fn geometry_cells(page: &PageReport) -> Vec<Cell> {
    let mut cells = vec![Cell::new(Field::Size, page.size.to_string())];
    if let Some(crop) = page.crop().filter(|crop| crop.trimmed()) {
        cells.push(Cell::new(Field::Crop, crop.to_string()));
    }
    cells.push(Cell::new(
        Field::Scaling,
        match page.scaling() {
            Some(scaling) => scaling.to_string(),
            None => "失败页 · 卷内统一尺寸留白".to_owned(),
        },
    ));
    if let Some(cut) = page.cut() {
        cells.push(Cell::new(Field::Cut, cut.to_string()));
    }
    if page.backstopped() {
        cells.push(Cell::new(Field::Backstop, "兜底退回 fit-inside"));
    }
    cells.push(Cell::new(Field::Output, page.output.display().to_string()));
    cells
}

/// 幂等命中而跳过的卷那一句。
///
/// 「跳过」本身不够——用户要能分清「这一卷没变」与「工具没做事」。四项依据点名摆出来，
/// 改了其中哪一项会让它重做，一眼看得见（spec 的 story 8、story 9）。
const SKIPPED: &str =
    "跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做";

/// 卷那一行里说彩页有几张的那一格。
///
/// 一张都没有就**不在场**——绝大多数卷是这个样子（见 measurements 的《B 类素材普查》：
/// 97% 近灰度），每卷都挂一句「彩页 0 页」只是噪声。数的是**彩页**，与它走了哪条分支无关。
fn color_pages(volume: &VolumeReport) -> Option<Cell> {
    let count = volume
        .pages
        .iter()
        .filter(|page| page.color() == Some(PageColor::Color))
        .count();
    (count > 0).then(|| Cell::new(Field::ColorPages, count.to_string()))
}

/// 卷级那几行里说判定的那一段：几何门的判定结果，加上这一卷的候选从哪来。
///
/// 「这卷为什么是这个候选」要有一个指得出定档页的答案（ADR 0006），这几行就是它。
/// 上包络不在场时说清是为什么不在场——那正是翻页跳变回来的时候，报告不能看起来还是一样。
fn verdict_rows(volume: &VolumeReport) -> Vec<Row> {
    // 一张灰度页都没有的卷（只装着彩页的、整卷全失败的）没有候选可判，几何门也就无从谈起。
    let Some(verdict) = &volume.verdict else {
        return Vec::new();
    };
    // 跳过的卷同样没有几何门可说——它一页都没算。这一支要排在 `gate_rows` 之前：
    // 那里读的 `volume.gate` 只有算过的卷才有。
    if volume.skipped() {
        return vec![sentence_row(RowKind::Skipped, SKIPPED)];
    }
    let mut rows: Vec<Row> = Vec::new();
    rows.extend(isolated_row(volume));
    rows.extend(salvaged_row(volume));
    rows.extend(gate_rows(volume, verdict));
    rows.extend(match verdict {
        // 上包络与它指出的定档页是**两行**：定档页是一页的名字，而上包络那一句是这一卷的判定。
        // 表要的基准档另占一格——它与那一句里说的是同一个数，取值不必回头认字符串。
        VolumeVerdict::Envelope(envelope) => vec![
            Row::new(
                RowKind::Envelope,
                vec![
                    Cell::new(Field::Base, envelope.base.to_string()),
                    Cell::new(Field::Envelope, envelope.to_string()),
                ],
            ),
            Row::one(
                RowKind::Driver,
                Cell::new(
                    Field::Source,
                    volume.pages[envelope.driver].source.display().to_string(),
                ),
            ),
        ],
        VolumeVerdict::Override(candidate) => vec![Row::new(
            RowKind::Override,
            vec![
                Cell::new(Field::Candidate, candidate.to_string()),
                Cell::new(
                    Field::Sentence,
                    format!(
                        "判定 {candidate}（覆盖项裁到只剩一个候选）：判定被顶掉，卷级基准档无从谈起"
                    ),
                ),
            ],
        )],
        VolumeVerdict::PerPage => vec![sentence_row(
            RowKind::PerPage,
            "无（--per-page）：上包络与迟滞关着，候选逐页最优，翻页处会换档",
        )],
        // 上面那一支已经把跳过的卷送走了。
        VolumeVerdict::Skipped { .. } => Vec::new(),
    });
    rows
}

/// 被隔离的卷那一行，排在卷级各行之首（12 号票：含失败页的卷被标记）。
///
/// 卷那一行里的去处已经指着隔离目录了，但那要用户认得出那个目录名才读得懂。
/// 这一行把话说完：几页失败、这一卷因此去了哪儿、坏页在输出里是什么样子。
/// 后面几行照常——隔离的卷是**处理过**的卷，几何门、卷级判定、逐页结果一样不少。
fn isolated_row(volume: &VolumeReport) -> Option<Row> {
    let failed = volume.failures().count();
    if failed == 0 {
        return None;
    }
    Some(sentence_row(
        RowKind::Isolated,
        format!(
            "隔离 {failed} 页失败：本卷整卷写到隔离目录 {}，\
             失败页以卷内统一尺寸留白占位，页序不断",
            volume.output.display()
        ),
    ))
}

/// 几何门那一段：门的**判定范围**、范围里有几页不成立，加上本卷最终抖不抖。
///
/// 三件事写在一起，因为只有并排才解释得了对方。门逐页判（ADR 0007 决定第 1 条），
/// 「成立」这句话因此得连着范围一起读——一卷全是彩页时门同样成立，而那是「无人可关」，
/// 不是「每一页都贴住了面板」。本卷那个抖动模式同理：门在其余页那一组上开着时它才是判据选的，
/// 其余页一页都不成立时它只是被关掉的结果。
///
/// **被排除的页要指得出来**，与上包络指出定档页同一个做法：不指名，用户就无从判断
/// 这一卷该不该换个 profile。逐页那几行各自标着理由（`几何门不成立，本页不抖动`），
/// 这里只给个抓手——页数多起来时全列一遍只会把卷级那几行淹掉。
fn gate_rows(volume: &VolumeReport, verdict: &VolumeVerdict) -> Vec<Row> {
    let judged = volume.judged_by_the_gate().count();
    let broken: Vec<&PageReport> = volume.outside_the_gate().collect();
    // `--per-page` 一开就没有卷级的抖动模式：它跟着位深一起逐页可变。
    let dither = verdict
        .dither()
        .map_or_else(|| "逐页".to_owned(), |dither| dither.to_string());
    let mut rows = vec![Row::new(
        RowKind::Gate,
        vec![
            Cell::new(Field::GateScope, judged.to_string()),
            Cell::new(Field::GateBroken, broken.len().to_string()),
            Cell::new(Field::Dither, dither),
        ],
    )];
    if broken.is_empty() {
        return rows;
    }
    if broken.len() == judged {
        // 一页成立的都没有：没有别人可护，这些页自己就当其余页，卷级那一档由它们定出
        // ——那一档必然不抖（ADR 0007 决定第 5 条）。
        rows.push(gate_note(
            "范围内一页都不成立：每一页源都比目标小，按不放大原样输出，\
             阅读器还要再缩一次。没有别人可护，卷级基准档由它们自己定出，抖动因此整卷关闭",
        ));
    } else {
        rows.push(gate_note(format!("不成立：{}", first_few_names(&broken))));
        rows.push(gate_note(
            "这几页源比目标小，原样输出，阅读器还要再缩一次：它们不进卷级上包络，\
             抖动单独关掉，位深仍跟着卷级基准档、不低于它",
        ));
    }
    // 同一道门也撑着面板灰阶那道硬上界：像素与灰阶不再对齐，「多出来的级到不了眼睛」
    // 就不再成立。ADR 0003 说了不得沿用，也说了该用哪个集合尚未测量——P0 仍照它裁，
    // 报告因此得把这句话说出来，而不是让它烂在一句注释里。
    rows.push(gate_note(
        "面板灰阶上界的依据在这几页上随门一起失效，\
         P0 仍按它裁候选位深（ADR 0003：该用哪个集合尚未测量）",
    ));
    rows
}

/// 几何门那一行底下的一句注解。
fn gate_note(sentence: impl Into<String>) -> Row {
    sentence_row(RowKind::GateNote, sentence)
}

/// **成句的那一种行**：一行只有一格，装着整句话（[`Field::Sentence`]）。
///
/// 它们本来就是句子，拆开没有意义——拆的那一刀会把措辞挪到排版那一层去，
/// 而措辞只许有一处出处（ADR 0016）。画它的那一头把这样的一行当整段文字折行。
fn sentence_row(kind: RowKind, sentence: impl Into<String>) -> Row {
    Row::one(kind, Cell::new(Field::Sentence, sentence))
}

/// 头几页的名字排成一句，剩下的报个数收口。
///
/// 上界取三：这一句是给人抓手用的，不是清单——真要逐页看，逐页那几行一页不落地列着。
fn first_few_names(pages: &[&PageReport]) -> String {
    const SHOWN: usize = 3;
    let listed: Vec<String> = pages
        .iter()
        .take(SHOWN)
        .map(|page| page.source.display().to_string())
        .collect();
    match pages.len().checked_sub(SHOWN) {
        Some(rest) if rest > 0 => format!("{}，另有 {rest} 页", listed.join("、")),
        _ => listed.join("、"),
    }
}

/// 一页那一行：它走的分支，以及那条分支得出的结果。
///
/// 灰度路径给的是判定与判据。判据是量、阈值是界：判定从两者的比较来，因此两者都得摆在
/// 同一行上，判定才是可解释的（spec 的 story 7）。阈值在头一行的 profile 里，
/// 它对整份报告只有一个。
///
/// 彩色分支上没有判定可说，那一行说的是它为什么没有：那条路径只缩放（ADR 0005 决定第 4 条）。
/// 彩页转灰走的是灰度路径，行首标出来——不标，用户就看不出这一档位深是替一张彩页定的，
/// 也看不出这台设备为什么没留住颜色。
///
/// 失败页那一行说的是**原因**（spec 的 story 26）：报告要让用户知道该去修哪几张。
/// 原因是由内到外的整条错误链，最外一环指得出是哪一页、卡在哪一步。
fn page_row(page: &PageReport) -> Row {
    let Some(branch) = page.branch() else {
        return sentence_row(
            RowKind::PageFailure,
            failure_line(page.failure().expect("没有分支的页必是失败页")),
        );
    };
    // 部分救回页标在行首（04 号票）：它有判定、有判据、有自己的尺寸，逐页那一行因此
    // 与一张完好页长得一模一样，而它的判据是在一页大半留白的图上求出来的。
    let mut cells: Vec<Cell> = page
        .salvage()
        .map(|salvage| Cell::new(Field::Salvage, salvage.to_string()))
        .into_iter()
        .collect();
    match branch {
        PageBranch::Gray {
            scores, verdict, ..
        } => {
            if page.color() == Some(PageColor::Color) {
                cells.push(Cell::new(Field::ColorToGray, "彩页转灰"));
            }
            cells.push(Cell::new(Field::Candidate, verdict.candidate.to_string()));
            cells.push(Cell::new(Field::Reason, verdict.reason.to_string()));
            cells.push(Cell::new(Field::Scores, score_line(scores)));
            Row::new(RowKind::PageVerdict, cells)
        }
        PageBranch::Color => {
            cells.push(Cell::new(
                Field::Sentence,
                "彩页 · 彩色分支：只缩放，不量化，不进灰度缓存也不进卷级上包络",
            ));
            Row::new(RowKind::PageColor, cells)
        }
    }
}

/// 失败页那一句：**原因原样带上**（spec 的 story 26）。
///
/// 逐页那一行（[`pages`]）与会话「出现的当场」那一段（[`failing_pages`]）说的是
/// 同一句话——一份是结果，一份是增量，而措辞只有这一处。
fn failure_line(reason: &str) -> String {
    format!("失败 {reason}")
}

/// 一页各候选的判据值排成一串，候选由小到大——[判据那一格](Field::Scores)装的就是它。
///
/// **串起来的那个 `·` 是措辞，不是排版**：一格装一个值，而这一格的值本身是一串东西，
/// 「一串东西怎么说」与「一个数怎么写」同属措辞（ADR 0016 决定第 2 条）。
fn score_line(scores: &[CandidateScore]) -> String {
    scores
        .iter()
        .map(|scored| format!("{} {}", scored.candidate, scored.score))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 这一趟**至今为止**失败的那些页，出现一条画一条（09 号票的会话主区）。
///
/// 一页都没有就一个字都不说，与末尾那几小结同一条规矩。
///
/// 它与逐页那一行（[`pages`] 里失败页那一支）说的是同一件事、同一份原因——
/// 那一份是结果，这一段是**增量**（见 `tonefit::Event::PageFailed`）。
/// 会话非要它不可，是因为报告区默认只给卷级，而卷级那几行只说得出「几页失败」、
/// 说不出为什么（见 [`isolated_row`]），何况那一行要等整卷跑完才有。
///
/// 命令行不画它：那一路攒完才印，那时逐页那几行已经把话说全了。
/// 措辞仍然只有这一处——会话画的是它，不是自己另编的一句。
///
/// 它因此挂着特性开关：关掉 `tui` 就没有会话，也就没有人读它
/// （[`outcome`] 与 [`calibration_notice`] 同理）。措辞留在这里而不是搬进会话，
/// 是为了让它与 [`pages`] 里那一句挨着——两句说的是同一件事，走散了没人发现。
#[cfg(feature = "tui")]
pub fn failing_pages<'a>(pages: impl Iterator<Item = (&'a Path, &'a str)>) -> String {
    let mut text = String::new();
    for (page, reason) in pages {
        if text.is_empty() {
            text.push_str("失败页（出现的当场，逐页那几行在整卷跑完后才有）\n");
        }
        // 页名一行、原因一行，缩进与那一句都与逐页那两行一样（见 [`pages`] 与
        // [`failure_line`]）：同一件事在屏上不该长成两个样子。
        text.push_str(&format!(
            "  {}\n    {}\n",
            page.display(),
            failure_line(reason)
        ));
    }
    text
}

/// 这一趟是怎么**收的场**（`CONTEXT.md` 的《进度》：收场）。
///
/// 与 [`failing_pages`] 同一条：会话是它眼下唯一的读者（命令行那一路不印收场），
/// 措辞却仍旧留在这里——它说的是报告上的一格（`Report::outcome`），
/// 而报告的措辞只有本模块一处。会话直接印 `{:?}` 的话，中文界面上会冒出
/// `Stopped(Abort)` 这种 Rust 标识符。
///
/// 逐个变体都列出来、不留 `_`：[`RunOutcome`] 与 [`Instruction`] 都**不是**非穷尽的
/// （后者的文档写着为什么：它是库外要造出来的东西），多一种收场该怎么说，
/// 是个要当场拿的主意。
#[cfg(feature = "tui")]
pub fn outcome(outcome: RunOutcome) -> String {
    match outcome {
        RunOutcome::Completed => "点名的卷都走过了".to_owned(),
        RunOutcome::Stopped(Instruction::Finish) => {
            "按停（收尾）：当前卷跑完就停了，剩下的卷一个都没开工".to_owned()
        }
        RunOutcome::Stopped(Instruction::Abort) => {
            "按停（中止）：当前卷丢掉了，它等于没做，剩下的卷一个都没开工".to_owned()
        }
        // `Stopped` 里恒不是「继续」（见 `RunOutcome::of`）。真到了这里说明库那一侧
        // 改了那条性质，而这一句至少不撒谎。
        RunOutcome::Stopped(Instruction::Continue) => "按停停在半路".to_owned(),
        // 报告上出不来（那一趟返回的是错误本身），事件流上到得了。
        RunOutcome::Refused => "拒绝执行".to_owned(),
    }
}

/// 一个卷在屏上叫什么：路径的最后一段，取不出就整条路径。
///
/// 命令行的进度条（`crate::Bar::start`）与会话的当前卷条（会话批的 09 号票）
/// 印的是同一个名字，因此只有这一处。会话的**卷表**（卷名那一列与定档页那一列）走的
/// 也是它——「屏上叫什么」在这个仓库里只有这一条规矩，一页与一卷同样只印最后那一段。
pub fn volume_name(volume: &Path) -> String {
    volume.file_name().map_or_else(
        || volume.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// 标定图写出去之后**此刻**要做对的那一件事：以原尺寸打开它。
///
/// 命令行印的那几行与会话屏底那两行**共用这一句**（[`calibration_note`] 与
/// [`calibration_notice`]）：同一件事从两张嘴里出来，措辞只能有一处出处。
///
/// 只说这一件。判读顺序、怎么数、数出来的数是什么意思，图内中英两份都印着，
/// `--help` 里也写着——同一套说法在终端上再抄一遍，改的时候就得记着改三处。
/// 留下的那一条之所以在这里，是因为它在别处已经来不及：图一旦被缩着显示过，
/// 抖动块与光栅先糊掉、阶梯也跟着被重采样，它答的两件事一件都不作数了，
/// 而用户正是在这一刻决定怎么打开它。
///
/// **白边裁切**要单独点名，不并进笼统的一句「缩放」：糊的来源实测就是它
/// （measurements 的《真机像素完整性》），而它在阅读器里通常另占一个开关。
const OPEN_IT_AT_NATIVE_SIZE: &str = "拷进设备，以原尺寸打开：关掉缩放、适配屏幕与白边裁切——\
     图被缩过一次，它答的两件事一件都不作数了";

/// 那几句完整的说法在哪儿。**怎么数不在这一行里**——图内已印，`--help` 里也写着。
///
/// 只有命令行那一路印它：会话屏底那一格总共三行，多说一行就少一行按键提示，
/// 而此刻非说不可的只有[以原尺寸打开](OPEN_IT_AT_NATIVE_SIZE)那一句。
const WHERE_THE_FULL_STORY_IS: &str = "判读说明中英两份都印在图内，先看像素完整性再数灰阶；\
     完整说法见 tonefit calibrate --help";

/// 图写到哪儿了。命令行那几行与会话屏底那两行都从这一行起头，
/// 「标定图」三个字与路径怎么排因此只有这一处。
fn chart_landed_at(out: &Path) -> String {
    format!("标定图 {}", out.display())
}

/// 标定图写出去之后**命令行**印的那几行：图在哪儿，此刻要做对的那一件事，以及去哪儿看全套说法。
///
/// 它不从报告来，却与报告同属界面文案。会话屏底那两行是 [`calibration_notice`]——
/// 同一件事，格子不同。
///
/// 面板规格不重复——头一行的 `profile` 里已经有了。
pub fn calibration_note(profile: &Profile, out: &Path) -> String {
    format!(
        "profile {profile}\n{}\n  {OPEN_IT_AT_NATIVE_SIZE}\n  {WHERE_THE_FULL_STORY_IS}\n",
        chart_landed_at(out),
    )
}

/// 标定图写出去之后**会话屏底**那两行（会话批的 13 号票）。
///
/// 与命令行那几行共用要紧的那一句（[`OPEN_IT_AT_NATIVE_SIZE`]），少的是两样：
///
/// - **`profile` 那一行**——左栏上正摆着设备层，屏上已经有了。
/// - **[指路那一行](WHERE_THE_FULL_STORY_IS)**——屏底那一格总共三行，
///   说的话多占一行，按键提示就少一行。
///
/// 两行而不是一行：屏底那一格不折行，一条绝对路径接上那句话必被切掉，
/// 而路径就是「图在哪儿」的全部内容。存预设那一句避得开路径（`Session::saved`），
/// 靠的是预设那一栏自己摆着文件位置；标定图没有那样一个去处。
///
/// 关掉 `tui` 就没有会话，也就没有人读它——与 [`outcome`] 同一条。**只多一格 `test`**：
/// 读它的是状态机（`session::state`），而状态机摆在特性外面，关掉终端库那一趟仍要跑它的用例
/// （`docs/agents/gate.md` 的第二条闸门）。
#[cfg(any(feature = "tui", test))]
pub fn calibration_notice(out: &Path) -> String {
    format!("{}\n{OPEN_IT_AT_NATIVE_SIZE}", chart_landed_at(out))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use tonefit::SplitRule;
    // 退出码不是渲染的事，用例却要问它：「报告说得出隔离」与「退出码分得开隔离」是同一条
    // 验收的两半（12 号票），拆到两个用例就得把那一大摊报告再拼一遍。
    // 够得着命令行那一侧的只有这个 `#[cfg(test)]` 块：上面那些渲染函数一个符号都不碰它。
    use crate::{FAILED_VOLUME_EXIT, ISOLATED_EXIT, SUCCESS_EXIT, exit_code};
    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, ChosenBy, Dither, Envelope, FitMode,
        GeometryGate, GrayImage, Interlock, IoPlan, Medium, NonVolumeFile, PageOutcome, Processed,
        Readers, Reason, Reference, RunOutcome, Salvage, Scaling, Size, Verdict, VolumeFailure,
        VolumeTiming,
    };

    /// 一份卷级上包络。渲染这一侧只关心它有没有被说出来，一页的卷取那一页作定档页。
    fn envelope(base: Candidate) -> Envelope {
        Envelope {
            base,
            driver: 0,
            body_pages: 1,
            outlier_pages: 0,
            raised_pages: 0,
        }
    }

    /// 一份读取计划。渲染这一侧只关心它有没有被说出来，取「探到固态盘、并发读」那一种。
    fn io_plan() -> IoPlan {
        IoPlan {
            medium: Medium::Solid,
            readers: probed(8),
            fingerprint: probed(8),
        }
    }

    /// 探出来的一路读取，派 `count` 条。
    fn probed(count: usize) -> Readers {
        Readers {
            count,
            chosen_by: ChosenBy::Probe,
        }
    }

    /// 一份缓存用量。渲染这一侧只关心它有没有被说出来，数值取整好读的。
    fn cache_usage() -> CacheUsage {
        CacheUsage {
            budget: CacheBudget::default(),
            pages: 1,
            raw: 4 * 1024 * 1024,
            stored: 1024 * 1024,
            resident: 1024 * 1024,
            spilled: 0,
        }
    }

    use tonefit::Crop;

    /// 一个什么都没裁掉的裁边窗口。逐页那一行只在真裁掉了东西时才说裁边，
    /// 不问裁边的用例因此一律拿它填这一格（页几何批 02 号票）。
    fn nothing_trimmed() -> Crop {
        Crop::keeping_all(Size::new(1441, 2048))
    }

    /// B 类中位页缩到基准面板：总缩放比 1.219，不触发预缩（见 measurements 的《B 类素材普查》）。
    fn typical_scaling() -> Scaling {
        Scaling::plan(Size::new(1441, 2048), Size::new(1182, 1680))
    }

    /// 一份一页的报告。各用例只改自己那一处，别处照抄默认。
    ///
    /// 几何门不在参数里：它跟着那一页走（`PageBranch::Gray` 的 `gate`），
    /// 卷级那一段是从页数出来的（06 号票）。
    fn one_page_report(profile: Profile, verdict: VolumeVerdict, page: PageReport) -> Report {
        Report {
            profile,
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(verdict),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 1,
                timing: VolumeTiming::default(),
                pages: vec![page],
                source_pages: 1,
            }],
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn a_dry_run_says_nothing_was_written_and_gives_the_metric_for_every_candidate() {
        // 判据值从公开 seam 上真算一个：报告要显示的就是它。
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let one_bit_dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        let score = tonefit::score(
            &reference,
            &tonefit::quantize(reference.image(), one_bit_dithered),
        );
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(one_bit_dithered)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate: one_bit_dithered,
                            score,
                        }],
                        verdict: Verdict {
                            candidate: one_bit_dithered,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::DryRun);

        assert!(text.contains("dry-run"), "{text}");
        assert!(text.contains("还没落盘"), "{text}");
        // 比值 < 2 的一页：报告要说出它没预缩，残差段就是全部。
        assert!(text.contains("缩放比 1.219 · 未预缩"), "{text}");
        assert!(text.contains(&format!("判据 1bit+FS {score}")), "{text}");
        // dry-run 也给判定：预告的就是照做时会写出的那一个候选。
        assert!(text.contains("判定 1bit+FS"), "{text}");
    }

    #[test]
    fn the_report_renders_the_profile_then_one_line_per_volume_and_per_page() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        // 判据值从公开 seam 上真算一个：整页偏 8 级，判据读出的就是 8.000。
        let four_bit = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    // 正好两倍面板的一页：报告要说出它预缩过。
                    scaling: Scaling::plan(Size::new(2528, 3360), Size::new(1264, 1680)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate,
                            score: four_bit,
                        }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        // profile 一行、适配方式一行、裁边一行、跨页拆分一行、判据形状两行（构成与聚合）、
        // **目录一行**（`volume-discovery/08`：命令行那一副把这一枝摆在它那几卷前面）、
        // 卷六行（去处、几何门、卷级、定档页、读取、缓存），页两行：一行几何，一行判定。
        assert_eq!(text.lines().count(), 15);
        // 这一趟的页尺寸照哪三条规矩算出来的，抬头都说得出（页几何批 01、02、04 号票）。
        assert!(text.contains("适配方式 以高为准"), "{text}");
        assert!(text.contains("裁边 按行列墨量占比"), "{text}");
        assert!(text.contains("跨页拆分 跨页候选阈值 1.50"), "{text}");
        // 这一页不是切出来的，逐页那一行因此一个字都不说拆分。
        assert!(
            !text.contains("跨页右半") && !text.contains("跨页左半"),
            "{text}"
        );
        // 这一页一个像素都没裁，逐页那一行因此一个字都不说裁边。
        assert!(!text.contains("裁边 1441×2048"), "{text}");
        // 一页都没有超出面板宽，末尾那一小结因此一个字都不说。
        assert!(!text.contains("输出宽超过面板"), "{text}");
        // 头一行说明这份输出是给哪台设备的，以及本次用的面板。
        assert!(text.contains("kobo-libra-2"), "{text}");
        assert!(text.contains("300 PPI"), "{text}");
        assert!(text.contains("16 级灰阶"), "{text}");
        assert!(text.contains("library/volume-a"), "{text}");
        assert!(text.contains("1 页"), "{text}");
        assert!(text.contains("1264×1680"), "{text}");
        // 每页的缩放三件套：总缩放比、有没有预缩、残差比。
        assert!(text.contains("缩放比 2.000"), "{text}");
        assert!(text.contains("预缩 2×"), "{text}");
        assert!(text.contains("残差比 1.000"), "{text}");
        assert!(text.contains("out/volume-a/001.png"), "{text}");
        // 判定、它的理由，以及判定所依据的那个量：判定要可解释（spec 的 story 7）。
        assert!(
            text.contains(&format!(
                "判定 4bit（阈值内最低的一档）  判据 4bit {four_bit}"
            )),
            "{text}"
        );
        // 阈值对整份报告只有一个，写在头一行的 profile 里，并标明它是怎么定出来的。
        assert!(
            text.contains("阈值 5.500（盲测标定于 boox-poke6，其余面板未复核）"),
            "{text}"
        );
        // 判据那一栏的每个数都是分块聚合收出来的，而聚合里的 K 同样没标定——
        // 不说出来，读的人无从判断这一栏该信到什么程度（02 号票，ADR 0002 决定第 3 条）。
        // 块边长是 ADR 定死的数，直接写；K 是占位值，从 `aggregation()` 取——
        // 标定把它换掉时这一条不该跟着改。
        assert!(text.contains("判据聚合 分块 32×32"), "{text}");
        // 判据由两项合成，其中颗粒项那道地板与阈值同一批盲测标定：数与来源一并摆出来，
        // 否则逐页那一行的数是从哪来的没人答得出（ADR 0002 决定第 5 条）。
        assert!(text.contains("判据构成 低通后的局部均值误差"), "{text}");
        assert!(
            text.contains(&format!("颗粒超出 {:.1} 灰度级", composition().grain_floor)),
            "{text}"
        );
        assert!(text.contains("地板盲测标定于 boox-poke6"), "{text}");
        assert!(
            text.contains(&format!("不宽于 {} 块", aggregation().tail_tiles)),
            "{text}"
        );
        assert!(text.contains("K 未标定占位值"), "{text}");
        // 卷成为不可分割的处理单元是 ADR 0005 认下的代价：用量与有没有溢写都要说出来。
        assert!(text.contains("缓存 1 页 1.0 MiB"), "{text}");
        assert!(text.contains("未溢写"), "{text}");
        // 「这卷为什么是这个候选」要有一个指得出定档页的答案（ADR 0006）。
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        assert!(text.contains("定档页 library/volume-a/001.jpg"), "{text}");
        // 上包络不承诺卷内绝对一致：特例与迟滞升档各出了多少页，报告要说出来。
        // 特例那一处还带着占比——「一页都没摘出来」要在报告里看得见，而光看计数分不清
        // 「本来就没有特例页」与「特例判定整个失灵」（加固批 01 号票）。
        assert!(text.contains("特例 0 页（0.0%）"), "{text}");
        assert!(text.contains("迟滞升档 0 页"), "{text}");
        // 上包络的分位、迟滞页数、特例页判据的立脚点分位与倍数，四者均未标定，
        // 报告显式标注（ADR 0006）。
        assert!(text.contains("四者均未标定"), "{text}");
        // 几何门的判定范围与本卷的抖动模式都要报出来（ADR 0007、06 号票）：
        // 这一页在范围内、门开着，那个「不抖动」因此是判据选的。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动"),
            "{text}"
        );
    }

    /// 一份最省事的一页报告，只为问抬头：判定与判据在这几条用例里一个字都不看。
    ///
    /// 它与 [`one_page_report`] 分开，是因为那一个每次都要现算一个判据值——
    /// 互锁问的是**开关**，与页上有什么无关。
    fn switches_report(fit: FitMode, crop: bool, split: SplitRule) -> Report {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::Off);
        let mut report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: Vec::new(),
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );
        report.fit = fit;
        report.crop = crop;
        report.split = split;
        report
    }

    /// 互锁 ① 咬上时抬头**提一次**：拆分开着，适配方式却是 fit-inside
    /// （页几何批 05 号票的处置 ①）。
    ///
    /// 「一次」是字面的：那句话不逐卷、不逐页重复——组合本身成立，说清楚就够，
    /// 每一页再喊一遍就成了噪音。措辞不在这一层，用例因此拿 `Interlock` 自己那句话去比。
    #[test]
    fn the_header_says_once_that_splitting_meets_fit_inside() {
        let report = switches_report(FitMode::Inside, true, SplitRule::default());

        let text = plain::report(&report, Mode::Process);

        assert!(
            text.contains(&Interlock::SpreadsStayFlattened.to_string()),
            "{text}"
        );
        assert_eq!(text.matches("互锁 ").count(), 1, "{text}");
        // 它落在抬头，不在卷级也不在逐页：卷那一行之前就说完了。
        let header = super::header(&report, Mode::Process);
        assert!(
            header.contains(&Interlock::SpreadsStayFlattened.to_string()),
            "{header}"
        );
        // 拦不住任何东西：这一趟照常写，报告的其余部分一个字都不改。
        assert!(text.contains("out/volume-a/001.png"), "{text}");
    }

    /// ① 只在那一格上咬：另外三种开关组合的报告里一个「互锁」都没有。
    ///
    /// 默认那一套（拆、以高为准）尤其要钉住——绝大多数趟走的是它，
    /// 抬头多出一行就是每个人每一趟都要读一遍的噪音。
    #[test]
    fn the_other_three_combinations_of_fit_and_splitting_say_nothing() {
        let split_off = SplitRule {
            on: false,
            ..SplitRule::default()
        };
        for (fit, split) in [
            (FitMode::Height, SplitRule::default()),
            (FitMode::Height, split_off),
            (FitMode::Inside, split_off),
        ] {
            let text = plain::report(&switches_report(fit, true, split), Mode::Process);
            assert!(!text.contains("互锁"), "{fit:?} {split:?}\n{text}");
        }
    }

    /// 互锁 ② 咬上了报告仍然**一个字都不说**（页几何批 05 号票的处置 ②）。
    ///
    /// 关掉裁边完全合法，而抖动被抹平只在用户的阅读器会裁时才发生——tonefit 看不到那一层，
    /// 逐卷提醒等于噪音。那句话在 `--help` 与文档里（见命令行的 `interlock_help`）。
    ///
    /// 抬头那一行仍照实说「裁边 关」：那是**事实**，与那句提醒不是一回事。
    #[test]
    fn turning_crop_off_engages_an_interlock_that_the_report_never_mentions() {
        let report = switches_report(FitMode::Height, false, SplitRule::default());
        // 规则那一侧确实咬上了——报告不说，不是因为它没发生。
        assert!(
            report
                .interlocks()
                .any(|interlock| interlock == Interlock::ReaderCropWipesTheDither),
            "裁边关着却没咬上"
        );

        let text = plain::report(&report, Mode::Process);

        assert!(!text.contains("互锁"), "{text}");
        assert!(
            !text.contains(&Interlock::ReaderCropWipesTheDither.to_string()),
            "{text}"
        );
        // 事实照说：这一趟没裁，逐页那几行分不出来，只有这一行分得开（02 号票）。
        assert!(text.contains("裁边 关（页按解出来的原尺寸适配）"), "{text}");
    }

    /// 报告上露不露面，**由处置说了算**：露面的恰好是处置为 [`Voice::Header`] 的那些
    /// （页几何批 05 号票）。
    ///
    /// 这一趟两条一起咬上——拆分开着配 fit-inside（抬头提一次）与裁边关着（一趟不吭声），
    /// 用例因此不是空转：一条该在、一条该不在。渲染这一层不记着谁该露面，
    /// 它只照 `voice` 筛；处置改了，报告跟着改口。
    ///
    /// 处置是「当场拒绝」的那一条永远进不了任何报告：它咬上时 `run` 返回的是 `Err`，
    /// 压根没有报告可渲染（那条路由 `tests/pipeline.rs` 的
    /// `a_dither_the_geometry_gate_forbids_is_refused` 钉着）。
    #[test]
    fn only_the_interlocks_whose_voice_is_the_header_reach_the_report() {
        let report = switches_report(FitMode::Inside, false, SplitRule::default());
        let engaged: Vec<Interlock> = report.interlocks().collect();
        assert_eq!(engaged.len(), 2, "{engaged:?}");

        let text = plain::report(&report, Mode::Process);

        for interlock in engaged {
            assert_eq!(
                text.contains(&interlock.to_string()),
                interlock.voice() == Voice::Header,
                "{interlock:?}\n{text}"
            );
        }
    }

    /// 几何门那一段要说出**判定范围**与**被排除的页**（06 号票）：门逐页判，
    /// 「不成立」这句话得连着「范围里有几页、是哪几页」一起读，用户才判断得了这一卷该怎么办。
    #[test]
    fn a_broken_geometry_gate_names_its_scope_and_the_pages_it_left_out() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(800, 1000),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    // 源比目标小：按不放大原样输出，一条边都贴不住面板。
                    scaling: Scaling::plan(Size::new(800, 1000), Size::new(800, 1000)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Broken,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        // 判定范围与不成立的页数并排：一卷全是彩页时门同样成立，那是「无人可关」，
        // 不是「每一页都贴住了面板」。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 1 页 · 本卷 不抖动"),
            "{text}"
        );
        // 这一卷范围内一页都不成立：没有别人可护，卷级那一档由它们自己定出。
        assert!(text.contains("范围内一页都不成立"), "{text}");
        // 同一道门也撑着面板灰阶那道硬上界（ADR 0003），它跟着失效这件事不能只留在注释里。
        assert!(
            text.contains("面板灰阶上界的依据在这几页上随门一起失效"),
            "{text}"
        );
        assert!(text.contains("ADR 0003"), "{text}");
    }

    /// 输出宽超过面板的页要在末尾**点名**（页几何批 01 号票）。
    ///
    /// 用户翻它们时要横向翻动，而那对阅读器的要求比留边更强——留边只要它别重采样，
    /// 溢出要它平移不缩放。几何门在这些页上照旧成立（高那条边贴着面板），
    /// 光看门那一行看不出这件事，所以它得自己有一行。
    #[test]
    fn the_pages_wider_than_the_panel_are_named_at_the_end() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::FloydSteinberg);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        // 跨页：以高为准之后高贴住面板、宽是面板宽的 4 倍（5056 ÷ 1264）。
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(5056, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: Scaling::plan(Size::new(5056, 1680), Size::new(5056, 1680)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        // 门照旧成立：溢出的页贴得好好的。
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        assert!(text.contains("输出宽超过面板 1 页"), "{text}");
        assert!(text.contains("最宽 5056×1680"), "{text}");
        // 倍数要说出来：那是「要横向翻多少」的唯一线索。
        assert!(text.contains("面板宽 1264 的 4.00 倍"), "{text}");
        // 是哪一页要点名——报告里翻不回去就等于没说。点名用的是**源**那一侧的名字，
        // 与几何门那一行同一个出处（见 `first_few_names`）。
        assert!(text.contains("library/volume-a/001.jpg"), "{text}");
        // 出路也要给：换回 fit-inside 就压得回面板以内，代价一并说清。
        // 中间那个空格带着[标注](crate::wrap::HARD_SPACE)——折行不许在它上面断，
        // 断开了这条命令就抄不出来。印出去时它换回一个普通空格（`crate::wrap::fold`）。
        assert!(text.contains(&format!("--fit{HARD_SPACE}inside")), "{text}");
        // 门在这一页上照旧成立，两件事不许混为一谈。
        assert!(text.contains("不成立 0 页"), "{text}");
    }

    /// **兜底上界退回去的页要在末尾点名**（07 号票）。
    ///
    /// 用户点了一种适配方式，这几页却不是照它出的——报告不说，就只剩输出里几张莫名其妙
    /// 小一号的页。它自成一小结、不并进「输出宽超过面板」那一段：退回之后的页恒在面板宽
    /// 以内，压根不会落在那张清单里，两处说的是同一件事的两头。
    #[test]
    fn the_pages_the_backstop_pulled_back_are_named_at_the_end() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::FloydSteinberg);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        // 一根 3000×100 的长条：以高为准算出 50400×1680，退回 fit-inside 之后是 1264×42。
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 42),
                outcome: PageOutcome::Whole(Processed {
                    crop: Crop::keeping_all(Size::new(3000, 100)),
                    backstopped: true,
                    cut: None,
                    spread_candidate: false,
                    scaling: Scaling::plan(Size::new(3000, 100), Size::new(1264, 42)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        assert!(text.contains("兜底上界 1 页"), "{text}");
        // 是哪一页要点名——报告里翻不回去就等于没说。
        assert!(text.contains("library/volume-a/001.jpg"), "{text}");
        // 逐页那一行自己也说得出：末尾那一小结只点名头几页，翻得到的是这一小截。
        assert!(text.contains("兜底退回 fit-inside"), "{text}");
        // 上界那个数摆出来，读的人自己判断它对手上这批素材成不成立。
        assert!(
            text.contains(&tonefit::max_target_pixels().to_string()),
            "{text}"
        );
        // 退回的目的地也要说：用户点的是以高为准，出来的却是 fit-inside 的尺寸。
        assert!(text.contains("fit-inside"), "{text}");
        // 这一页在面板宽以内，不该同时出现在「输出宽超过面板」那一段里。
        assert!(!text.contains("输出宽超过面板"), "{text}");
    }

    /// 一页都没退回时**一个字都不说**：真实素材整批都是这个样子（07 号票）。
    ///
    /// 与另外三小结同一条规矩。这条用例与上一条一起把「只在真发生时说话」钉死——
    /// 少了它，一个恒真的小结也能让上一条通过。
    #[test]
    fn a_run_where_the_backstop_never_fired_says_nothing_about_it() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1182, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        assert!(!text.contains("兜底"), "{text}");
    }

    /// **报告逐页说得出这一页裁掉了多少**（页几何批 02 号票），而没裁的页一个字不说。
    ///
    /// 裁边那一小截排在缩放**之前**：裁边发生在适配之前，读的人顺着
    /// 「解出来多大 → 裁完多大 → 缩了多少 → 写出多大」一路读下来。
    ///
    /// 一页都没裁时那一格空着，与「这一趟根本没开裁边」长得一样——分辨两者的是抬头那一行，
    /// 这条用例把两处一起钉住。
    #[test]
    fn the_report_says_how_much_came_off_each_page_and_nothing_when_none_did() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let page = |crop: Crop| PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1260, 1680),
            outcome: PageOutcome::Whole(Processed {
                crop,
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: typical_scaling(),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    scores: vec![CandidateScore { candidate, score }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::LowestWithinThreshold,
                    },
                },
            }),
        };
        let trimmed = Crop::new(Size::new(1441, 2048), (120, 100), Size::new(1200, 1600));

        let text = plain::report(
            &one_page_report(
                profile.clone(),
                VolumeVerdict::Envelope(envelope(candidate)),
                page(trimmed),
            ),
            Mode::Process,
        );

        // 裁前裁后两个尺寸都在，裁掉了多少一眼看得出。**四边各去了多少不进这行文字**——
        // 读的人要的是「裁没裁、裁了多少」，不是左右上下怎么分；要那个数走 `PageReport::crop()`。
        assert!(text.contains("裁边 1441×2048 → 1200×1600"), "{text}");
        // 它排在缩放之前。
        let crop_at = text.find("裁边 1441×2048").expect("裁边那一小截");
        let scaling_at = text.find("缩放比").expect("缩放那一小截");
        assert!(crop_at < scaling_at, "裁边排到了缩放后面：{text}");

        // 一个像素都没裁的那一页：逐页那一行一个字不说，抬头那一行照旧说得出裁边开着。
        let untouched = plain::report(
            &one_page_report(
                profile,
                VolumeVerdict::Envelope(envelope(candidate)),
                page(nothing_trimmed()),
            ),
            Mode::Process,
        );
        assert!(!untouched.contains("裁边 1441×2048"), "{untouched}");
        assert!(untouched.contains("裁边 按行列墨量占比"), "{untouched}");
    }

    /// 报告要区分彩页与灰度页，也要区分它们走了哪条分支（10 号票）。
    ///
    /// 三页各占一种情形：彩页走彩色分支、彩页转灰走灰度路径、灰度页走灰度路径。
    /// 中间那一种是最容易被藏起来的——它有判定，看上去与灰度页毫无二致，
    /// 而用户想知道的恰恰是「这台设备为什么没留住颜色」。
    #[test]
    fn the_report_tells_a_color_page_apart_from_a_gray_one() {
        let profile = Profile::resolve("kobo-libra-colour").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let page = |name: &str, color, branch| PageReport {
            source: PathBuf::from(format!("library/volume-a/{name}.png")),
            output: PathBuf::from(format!("out/volume-a/{name}.png")),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                crop: nothing_trimmed(),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: typical_scaling(),
                color,
                branch,
            }),
        };
        let gray_branch = || PageBranch::Gray {
            gate: GeometryGate::Holds,
            scores: vec![CandidateScore { candidate, score }],
            verdict: Verdict {
                candidate,
                reason: Reason::LowestWithinThreshold,
            },
        };
        let report = Report {
            profile,
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                // 定档页必须是一张灰度页：彩页不进上包络，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(Envelope {
                    base: candidate,
                    driver: 2,
                    body_pages: 2,
                    outlier_pages: 0,
                    raised_pages: 0,
                })),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 3,
                timing: VolumeTiming::default(),
                source_pages: 3,
                pages: vec![
                    page("001", PageColor::Color, PageBranch::Color),
                    page("002", PageColor::Color, gray_branch()),
                    page("003", PageColor::Gray, gray_branch()),
                ],
            }],
            elapsed: Duration::ZERO,
        };

        let text = plain::report(&report, Mode::Process);

        // 卷那一行数得出彩页有几张：走哪条分支不影响它是不是彩页。
        assert!(text.contains("3 页，其中彩页 2 页"), "{text}");
        // 彩色分支那一页说得出它为什么没有判定。
        assert!(text.contains("彩页 · 彩色分支：只缩放"), "{text}");
        assert!(text.contains("不进灰度缓存也不进卷级上包络"), "{text}");
        // 转灰的那一页有判定，行首标着它的来路。
        assert!(text.contains("彩页转灰 · 判定 4bit"), "{text}");
        // 灰度页那一行不多带任何标记：四个空格之后直接是判定。
        assert!(text.contains("    判定 4bit"), "{text}");
        assert!(text.contains("定档页 library/volume-a/003.png"), "{text}");
    }

    /// 跳过的卷只占两行：去处那一行，加上说清它为什么什么都没有的那一行。
    ///
    /// 几何门、卷级判定、缓存用量、逐页结果一个都不出现——那一趟根本没算过它们，
    /// 报告摆出任何一项都是编的。页数照旧要说出来：它是源那一侧的事实。
    #[test]
    fn a_skipped_volume_says_so_and_says_nothing_it_did_not_compute() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                source_pages: 12,
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 0,
                timing: VolumeTiming::default(),
            }],
            elapsed: Duration::ZERO,
        };

        let text = plain::report(&report, Mode::Process);

        // profile 一行、适配方式一行、裁边一行、跨页拆分一行、判据形状两行、**目录一行**、
        // 卷两行，加上读取那一行——跳过的卷同样把整卷读了一遍。
        assert_eq!(text.lines().count(), 10);
        assert!(
            text.contains("library/volume-a → out/volume-a（12 页）"),
            "{text}"
        );
        assert!(text.contains("跳过 幂等命中"), "{text}");
        // 改哪一项会让它重做，用户得看得见（spec 的 story 9）。
        assert!(text.contains("工具版本、profile、参数、源均未变"), "{text}");
        assert!(!text.contains("几何门"), "{text}");
        assert!(!text.contains("缓存"), "{text}");
    }

    /// 介质探不出来的卷：报告说得出它退到了串行，也说得出**为什么**探不出来（13 号票）。
    ///
    /// 不说那句话，退到保守策略这件事对用户就只表现为「这一卷跑得慢」——
    /// 而那正是他没法据以决定要不要 `--io-mode concurrent` 的样子。
    #[test]
    fn a_volume_whose_medium_is_unknown_says_why_it_fell_back_to_serial() {
        let mut report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from(r"\\nas\share\volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                source_pages: 12,
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 0,
                timing: VolumeTiming::default(),
            }],
            elapsed: Duration::ZERO,
        };
        report.volumes[0].io = IoPlan {
            medium: Medium::Unknown {
                reason: r"\\nas\share\ 是网络路径，介质无从探测".to_owned(),
            },
            readers: probed(1),
            fingerprint: probed(1),
        };

        let text = plain::report(&report, Mode::Process);

        assert!(text.contains("读取串行"), "{text}");
        assert!(text.contains("是网络路径"), "{text}");
    }

    /// 被隔离的卷要说清三件事：几页失败、整卷去了哪儿、每一页各是为什么
    /// （spec 的 story 25、story 26）。退出码跟着分开——脚本读的是那个数。
    #[test]
    fn an_isolated_volume_names_its_failed_pages_and_gets_its_own_exit_code() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let good = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                crop: nothing_trimmed(),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: typical_scaling(),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    scores: vec![CandidateScore { candidate, score }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::VolumeEnvelope,
                    },
                },
            }),
        };
        let failed = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/002.png"),
            // 失败页照卷内统一尺寸出：与上面那张好页一模一样。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Failed {
                reason: "解 library/volume-a/002.jpg 这一页: 判定格式".to_owned(),
            },
        };
        let report = Report {
            profile,
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/_isolated/volume-a"),
                // 上一趟这一卷是干净的，那一份还在 out/volume-a 留着。
                superseded: Some(PathBuf::from("out/volume-a")),
                // 定档页必须是一张好页：失败页没有判据曲线，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 2,
                timing: VolumeTiming::default(),
                source_pages: 2,
                pages: vec![good, failed],
            }],
            elapsed: Duration::ZERO,
        };

        let text = plain::report(&report, Mode::Process);

        // 卷级那一行说得出几页失败、整卷去了哪儿。
        assert!(text.contains("隔离 1 页失败"), "{text}");
        assert!(text.contains("out/_isolated/volume-a"), "{text}");
        // 隔离的卷仍是**处理过**的卷：几何门、卷级判定、缓存一样不少。
        // 失败页不在几何门的判定范围内（它连尺寸都没有），范围因此只有那一张好页。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页"),
            "{text}"
        );
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        // 失败页那两行：尺寸从哪来，以及它为什么失败。
        assert!(
            text.contains("1264×1680  失败页 · 卷内统一尺寸留白"),
            "{text}"
        );
        assert!(
            text.contains("失败 解 library/volume-a/002.jpg 这一页: 判定格式"),
            "{text}"
        );
        // 末尾那一行：几十卷跑下来不用往回翻也知道这一趟出过事。
        assert!(text.contains("隔离 1 卷 · 失败 1 页"), "{text}");
        // 上一趟写在干净去处的那一份还在，这一趟没覆盖它——报告要指名道姓说出来。
        assert!(text.contains("过期副本 out/volume-a"), "{text}");
        assert!(text.contains("删不删由你"), "{text}");
        // 退出码分得开「全部成功」与「有卷被隔离」。
        assert_eq!(exit_code(&report), ISOLATED_EXIT);

        // **两件事同时成立时报更重的那一件**（05 号票）：同一份报告上再添一卷没做成的，
        // 退出码就得让给 `3`。隔离的卷交出来了、只是带着坏页，没做成的卷一个字节都没交出来——
        // 脚本据此做的是两个不同的决定，而一个进程只交得出一个数。
        let mut also_a_failed_volume = report.clone();
        also_a_failed_volume.failed_volumes.push(VolumeFailure {
            volume: PathBuf::from("library/volume-b"),
            reason: "打开 library/volume-b: 拒绝访问".to_owned(),
        });
        assert_eq!(exit_code(&also_a_failed_volume), FAILED_VOLUME_EXIT);
    }

    /// 没做成的卷在报告里**有自己的位置**，而且说得出为什么（05 号票）。
    ///
    /// 正文逐卷那几段只画得出做出了东西的卷——没做成的卷没有去处、没有页数、没有判定可印，
    /// 末尾这一小结是它在报告里唯一的位置。少了它，用户拿到的是一份少了几卷
    /// 却不说为什么的报告，那比当场报错更糟。
    ///
    /// 退出码那一半同在这里，与隔离那一条同一个理由：「报告说得出」与「脚本分得开」
    /// 是同一条验收的两半，拆到两个用例就得把那一摊报告再拼一遍。
    #[test]
    fn the_report_names_every_volume_that_never_got_done_and_why() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            // 点名一个卷、它没做成：做出了东西的卷一个都没有。
            volumes: Vec::new(),
            failed_volumes: vec![VolumeFailure {
                volume: PathBuf::from("library/volume-b"),
                reason: "读 library/volume-b/ComicInfo.xml: 系统找不到指定的文件".to_owned(),
            }],
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            elapsed: Duration::ZERO,
        };

        let text = plain::report(&report, Mode::Process);

        assert!(text.contains("卷级失败 1 卷"), "{text}");
        assert!(text.contains("library/volume-b"), "{text}");
        // 为什么没做成要逐条说出来：只报个数，用户不知道该去修什么。
        assert!(text.contains("ComicInfo.xml"), "{text}");
        // 「这一趟没有因此停下」也要说：报告少了一卷，读的人得知道别的卷没跟着遭殃。
        // 这句话**不许断言别的卷做成了**——这份报告里一卷都没做成，那样说就是假话。
        assert!(text.contains("这一趟没有因此停下"), "{text}");
        assert_eq!(exit_code(&report), FAILED_VOLUME_EXIT);
    }

    /// 非卷文件在报告里**有自己的位置**，三类各印各的那句为什么（`volume-discovery/04`）。
    ///
    /// 与卷级失败那一小结同一个处境：这几个文件在正文里一行都没有，末尾这一小结是它们
    /// 在报告里唯一的位置。三句话要**分得开**——三者的处置一样，用户要做的事却不一样：
    /// 一种是源库的杂物，一种多半是拿错了包，一种是真有东西坏了。
    ///
    /// 退出码那一半同在这里，与卷级失败那一条同一个理由：「报告说得出」与「脚本一格不动」
    /// 是同一条验收的两半。
    ///
    /// 这一趟**真做成了一卷**，清单却不空：退出码因此仍是「全部成功」那一个。
    /// 拿一份空报告去问这个数是问不出东西的——那种报告本来就是 `0`，
    /// 断言会在清单被记成失败的那天照旧通过。
    #[test]
    fn the_report_names_every_file_no_volume_took_and_why() {
        let mut report = one_page_report(
            Profile::resolve("kobo-libra-2").expect("内置型号"),
            VolumeVerdict::PerPage,
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Color,
                    branch: PageBranch::Color,
                }),
            },
        );
        report.non_volume_files = vec![
            NonVolumeFile {
                path: PathBuf::from("library/答案.txt"),
                reason: NonVolumeReason::NeitherPageNorArchive,
            },
            NonVolumeFile {
                path: PathBuf::from("library/字体包.zip"),
                reason: NonVolumeReason::ArchiveWithoutAPage,
            },
            NonVolumeFile {
                path: PathBuf::from("library/坏的.cbz"),
                reason: NonVolumeReason::Unopenable("读不出归档结构".to_owned()),
            },
        ];

        let text = plain::report(&report, Mode::Process);

        assert!(text.contains("非卷文件 3 个"), "{text}");
        // 逐条带路径：只报个数的话，用户还得自己回源库里对一遍才知道是哪几个。
        for path in ["library/答案.txt", "library/字体包.zip", "library/坏的.cbz"] {
            assert!(text.contains(path), "清单里没有 {path}：{text}");
        }
        // 点不开那一条把错误链原样带上来——那是三类里唯一「真有东西坏了」的一种。
        assert!(text.contains("读不出归档结构"), "{text}");
        // 三句为什么两两不同：印成同一句就等于这张表只有一类。
        let reasons: Vec<String> = report
            .non_volume_files
            .iter()
            .map(|file| super::non_volume_reason(&file.reason))
            .collect();
        let mut distinct = reasons.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), reasons.len(), "三类没分开：{reasons:?}");
        // **不是失败**：这一列不空，退出码照旧是「全部成功」那一个。
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 一个都没有就一个字都不说，与末尾那几小结同一条规矩——绝大多数一趟是这个样子。
    #[test]
    fn a_run_that_left_nothing_behind_says_nothing_about_non_volume_files() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            volumes: Vec::new(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            elapsed: Duration::ZERO,
        };

        let text = super::tail(&report);

        assert!(text.is_empty(), "什么都没出事的一趟还说了话：{text}");
    }

    /// 部分救回页在报告里认得出来，而且**只有报告认得出来**（04 号票）。
    ///
    /// 它没有退出码替它喊，卷也照旧落在干净的去处：这一趟从进程那一侧看与全部成功
    /// 一模一样。三处各说一遍——逐页那一行说这一页救回了多少，卷级那一行说它没参与
    /// 卷级的判定，末尾那一行让几十卷跑下来的人不用往回翻。
    #[test]
    fn the_report_marks_a_salvaged_page_and_says_it_stayed_out_of_the_volume_decision() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let processed = |reason| Processed {
            crop: nothing_trimmed(),
            backstopped: false,
            cut: None,
            spread_candidate: false,
            scaling: typical_scaling(),
            color: PageColor::Gray,
            branch: PageBranch::Gray {
                gate: GeometryGate::Holds,
                scores: vec![CandidateScore { candidate, score }],
                verdict: Verdict { candidate, reason },
            },
        };
        let whole = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(processed(Reason::VolumeEnvelope)),
        };
        let salvaged = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/volume-a/002.png"),
            // 它按**自己**的尺寸出：文件头里那个尺寸一点没缺。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Salvaged {
                // 它没进上包络，判定因此是它自己那条判据曲线定的。
                page: processed(Reason::LowestWithinThreshold),
                salvage: Salvage::from_share(0.625),
            },
        };
        let report = Report {
            profile,
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            failed_volumes: Vec::new(),
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                extracted: 0,
                io: io_plan(),
                decodes: 2,
                timing: VolumeTiming::default(),
                source_pages: 2,
                pages: vec![whole, salvaged],
            }],
            elapsed: Duration::ZERO,
        };

        let text = plain::report(&report, Mode::Process);

        // 逐页那一行：救回了多少，摆在判定前面。
        assert!(text.contains("救回 62.5% · 判定 4bit"), "{text}");
        // 卷级那一行：这一卷有几页不全，以及它没参与卷级的哪一件事。
        // 几何门不在那句话里了——门逐页判之后照旧问它（ADR 0007 决定第 1 条，06 号票）。
        assert!(text.contains("部分救回 1 页"), "{text}");
        assert!(text.contains("不参与卷级上包络"), "{text}");
        assert!(text.contains("几何门照旧问它们"), "{text}");
        // 末尾那一行：几十卷跑下来不用往回翻。
        assert!(text.contains("部分救回 1 卷 · 1 页"), "{text}");
        // 完好的那一页一个字都不多说：它那一行以判定开头，前面没有救回那一截。
        assert!(
            text.contains(
                "
    判定 4bit（卷级上包络）"
            ),
            "{text}"
        );
        // 卷没被隔离，退出码因此仍是 0——报告是唯一说得出这件事的地方。
        assert!(!text.contains("隔离"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 一卷都没被隔离时，隔离那几行一个字都不出现，退出码是 0。
    ///
    /// 「没出事」与「出了事」在报告与退出码上都得分得开，而分得开要两侧各测一遍。
    #[test]
    fn a_run_without_a_failed_page_says_nothing_about_isolation() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = plain::report(&report, Mode::Process);

        assert!(!text.contains("隔离"), "{text}");
        assert!(!text.contains("失败"), "{text}");
        assert!(!text.contains("过期副本"), "{text}");
        // 一页都没救回过的一趟同样一个字都不说（04 号票）。
        assert!(!text.contains("救回"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 一个说得上话的卷：过期副本、摊开、隔离、上包络与定档页，一张好页加一张失败页。
    ///
    /// 行与格那几条用它——一份报告上摆得出的行有大半在这里。
    fn a_volume_worth_a_row_of_each_kind() -> VolumeReport {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        VolumeReport {
            volume: PathBuf::from("library/volume-a"),
            output: PathBuf::from("out/_isolated/volume-a"),
            superseded: Some(PathBuf::from("out/volume-a")),
            verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
            cache: cache_usage(),
            extracted: 3 * 1024 * 1024,
            io: io_plan(),
            decodes: 2,
            timing: VolumeTiming::default(),
            source_pages: 2,
            pages: vec![
                PageReport {
                    source: PathBuf::from("library/volume-a/001.jpg"),
                    output: PathBuf::from("out/_isolated/volume-a/001.png"),
                    size: Size::new(1264, 1680),
                    outcome: PageOutcome::Whole(Processed {
                        crop: nothing_trimmed(),
                        backstopped: false,
                        cut: None,
                        spread_candidate: false,
                        scaling: typical_scaling(),
                        color: PageColor::Gray,
                        branch: PageBranch::Gray {
                            gate: GeometryGate::Holds,
                            scores: vec![CandidateScore { candidate, score }],
                            verdict: Verdict {
                                candidate,
                                reason: Reason::VolumeEnvelope,
                            },
                        },
                    }),
                },
                PageReport {
                    source: PathBuf::from("library/volume-a/002.jpg"),
                    output: PathBuf::from("out/_isolated/volume-a/002.png"),
                    // 失败页照卷内统一尺寸出：与上面那张好页一模一样。
                    size: Size::new(1264, 1680),
                    outcome: PageOutcome::Failed {
                        reason: "解 library/volume-a/002.jpg 这一页: 判定格式".to_owned(),
                    },
                },
            ],
        }
    }

    /// **哪几页要紧，判据只有这一处**（[`notable`]，`p3-session-legibility/11`）。
    ///
    /// 七种逐条问一遍，外加两条边界：一页可以同时要紧在**好几处**，
    /// 而**跳过的卷一页都不要紧**（它连逐页结果都没有）。
    ///
    /// 屏上那个词与那个记号不在这里问（那是画法那一层的事，
    /// 见 `crate::session::draw::pages`）：这一条问的是「要不要紧」本身。
    #[test]
    fn which_pages_matter_is_judged_in_exactly_one_place() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let panel = profile.panel();
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        // 定档页那一张加失败页那一张：夹具本来就是这两张。
        let isolated = a_volume_worth_a_row_of_each_kind();
        assert_eq!(
            notable(&isolated, panel),
            vec![vec![Notable::Driver], vec![Notable::Failed]]
        );

        // 换掉头一页，逐种问一遍。定档页那一格跟着它走（`Envelope::driver` 指着第 0 页）。
        let processed = |gate: GeometryGate, reason: Reason, backstopped: bool| Processed {
            crop: nothing_trimmed(),
            backstopped,
            cut: None,
            spread_candidate: false,
            scaling: typical_scaling(),
            color: PageColor::Gray,
            branch: PageBranch::Gray {
                gate,
                scores: Vec::new(),
                verdict: Verdict { candidate, reason },
            },
        };
        let judged = |size: Size, outcome: PageOutcome| {
            let mut volume = a_volume_worth_a_row_of_each_kind();
            volume.pages[0].size = size;
            volume.pages[0].outcome = outcome;
            notable(&volume, panel).swap_remove(0)
        };
        let ordinary = Size::new(1264, 1680);

        // 特例页：判定的理由说得出它单独定了档。
        assert_eq!(
            judged(
                ordinary,
                PageOutcome::Whole(processed(GeometryGate::Holds, Reason::Outlier, false))
            ),
            vec![Notable::Outlier, Notable::Driver]
        );
        // 几何门不成立。
        assert_eq!(
            judged(
                ordinary,
                PageOutcome::Whole(processed(
                    GeometryGate::Broken,
                    Reason::OutsideTheGate,
                    false
                ))
            ),
            vec![Notable::OutsideTheGate, Notable::Driver]
        );
        // 部分救回。
        assert_eq!(
            judged(
                ordinary,
                PageOutcome::Salvaged {
                    page: processed(GeometryGate::Holds, Reason::VolumeEnvelope, false),
                    salvage: Salvage::from_share(0.62),
                }
            ),
            vec![Notable::Salvaged, Notable::Driver]
        );
        // 兜底上界退回过。**退回之后恒不超过面板宽**（见 [`Report::backstopped`]），
        // 因此这一张拿的是普通尺寸——那一对凑不到一起，不拿它当「好几处」的例子。
        assert_eq!(
            judged(
                ordinary,
                PageOutcome::Whole(processed(GeometryGate::Holds, Reason::VolumeEnvelope, true))
            ),
            vec![Notable::Backstopped, Notable::Driver]
        );
        // **一页要紧在好几处**：以高为准的跨页卷里，定档页多半也是一张宽溢出的页。
        assert_eq!(
            judged(
                Size::new(panel.resolution.width + 1, 1680),
                PageOutcome::Whole(processed(
                    GeometryGate::Holds,
                    Reason::VolumeEnvelope,
                    false
                ))
            ),
            vec![Notable::Overflowed, Notable::Driver]
        );
        // 贴着面板宽的那一页**不算**宽溢出：判据是「超过」，不是「够到」。
        assert_eq!(
            judged(
                ordinary,
                PageOutcome::Whole(processed(
                    GeometryGate::Holds,
                    Reason::VolumeEnvelope,
                    false
                ))
            ),
            vec![Notable::Driver]
        );

        // 定档页只有上包络定出来的卷才有：`--per-page` 那一种一页都不站在分位秩上。
        let mut per_page = a_volume_worth_a_row_of_each_kind();
        per_page.verdict = Some(VolumeVerdict::PerPage);
        assert_eq!(notable(&per_page, panel)[0], Vec::new());

        // 跳过的卷一页都没有：逐页结果一条都没有，这一列因此是空的
        // （与 [`pages`] 那道守卫同一件事）。
        let mut skipped = a_volume_worth_a_row_of_each_kind();
        skipped.pages = Vec::new();
        skipped.verdict = Some(VolumeVerdict::Skipped { page_count: 184 });
        assert!(notable(&skipped, panel).is_empty());
    }

    /// **卷级与逐页出的是行，每一行说得出它是什么行**（ADR 0016）。
    ///
    /// 断言的是次序与种类，不是拼出来的那段文字——那一段由 [`plain`] 那一副钉着
    /// （见 `drawing_the_four_parts_one_by_one_gives_the_same_bytes_as_one_shot`）。
    /// 两者分开问，正是「措辞一处、排版两处」那条分界在用例上的样子。
    #[test]
    fn the_volume_and_page_lines_are_rows_that_each_say_what_they_are() {
        let isolated = a_volume_worth_a_row_of_each_kind();

        assert_eq!(
            volume(&isolated)
                .iter()
                .map(|row| row.kind)
                .collect::<Vec<_>>(),
            vec![
                RowKind::Volume,
                RowKind::Superseded,
                RowKind::Isolated,
                RowKind::Gate,
                RowKind::Envelope,
                RowKind::Driver,
                RowKind::Reading,
                RowKind::Extraction,
                RowKind::Cache,
            ]
        );
        // 一页两行：一行几何、一行判定，而失败那一页的判定是**另一种行**。
        assert_eq!(
            pages(&isolated)
                .iter()
                .map(|row| row.kind)
                .collect::<Vec<_>>(),
            vec![
                RowKind::PageGeometry,
                RowKind::PageVerdict,
                RowKind::PageGeometry,
                RowKind::PageFailure,
            ]
        );

        let rows = volume(&isolated);
        // 卷那一行三格：源、去处、页数。一张彩页都没有，彩页那一格因此不在场。
        assert_eq!(rows[0].cells.len(), 3);
        assert_eq!(rows[0].cell(Field::PageCount), Some("2"));
        assert_eq!(rows[0].cell(Field::ColorPages), None);
        // 几何门那一行三格，三个数各占一格。
        assert_eq!(rows[3].cells.len(), 3);
        assert_eq!(rows[3].cell(Field::GateScope), Some("1"));
        assert_eq!(rows[3].cell(Field::GateBroken), Some("0"));
        // 基准档单占一格：表要的就是这一个值，不必回头去认那一整句话。
        assert_eq!(rows[4].cell(Field::Base), Some("4bit"));
        assert!(
            rows[4]
                .cell(Field::Envelope)
                .expect("上包络那一格")
                .starts_with("基准档 4bit"),
            "{:?}",
            rows[4]
        );
    }

    /// **跳过的卷少的是格，不是话**：它没算过的那几项一格都不在场。
    ///
    /// 「跳过」本身仍是完整的一句（[`Field::Sentence`]），而基准档、几何门那三个数、
    /// 缓存用量一个都取不出来——取不出来正是对的，那一趟根本没算过它们。
    #[test]
    fn a_skipped_volume_is_short_of_the_cells_it_never_computed() {
        let mut skipped = extracted_by(512 * 1024);
        skipped.verdict = Some(VolumeVerdict::Skipped { page_count: 12 });

        let rows = volume(&skipped);

        assert_eq!(
            rows.iter().map(|row| row.kind).collect::<Vec<_>>(),
            // 摊开那一行在场：幂等那一道要把整卷读一遍，而读之前得先摊开。
            vec![
                RowKind::Volume,
                RowKind::Skipped,
                RowKind::Reading,
                RowKind::Extraction,
            ]
        );
        for field in [
            Field::Base,
            Field::GateScope,
            Field::GateBroken,
            Field::Dither,
            Field::Cache,
        ] {
            assert!(
                rows.iter().all(|row| row.cell(field).is_none()),
                "跳过的卷不该有 {field:?} 那一格"
            );
        }
        // 页数照旧在场：它是源那一侧的事实。
        assert_eq!(rows[0].cell(Field::PageCount), Some("12"));
        // 跳过那一行成句，只有一格。
        assert_eq!(rows[1].cells.len(), 1);
        assert!(
            rows[1]
                .cell(Field::Sentence)
                .expect("跳过那一句")
                .starts_with("跳过 幂等命中"),
            "{:?}",
            rows[1]
        );
        // 逐页一行都没有：那一趟根本没算过逐页结果。
        assert!(pages(&skipped).is_empty());
    }

    /// **出了事的那一页与被它拖进隔离的那一卷各是一种行**，失败原因仍是整段文字。
    ///
    /// 「失败的卷是哪一种行」问的是卷级那一段：有页失败的卷多出一行
    /// [`RowKind::Isolated`]，而不是在卷那一行上多一格——排版据此上色、给行首记号，
    /// 不必回头去认那一句话里有没有「隔离」两个字。
    ///
    /// **卷级失败**（一个字节都没交出来的那种卷）不在这里：它连 `VolumeReport` 都没有，
    /// [`volume`] 够不着它。它自成一种行（[`failed_volume`]），
    /// 见 [`a_volume_that_never_got_done_is_its_own_kind_of_row`]（Q133 收在那里）。
    ///
    /// 失败原因是句子，拆开没有意义——会话把这样的一行当整段文字折行画。
    /// 失败页那一行因此只有一格，而那一格装的是[逐页与增量共用的那一句](failure_line)。
    #[test]
    fn a_failed_page_and_the_volume_it_isolates_are_each_their_own_kind_of_row() {
        let isolated = a_volume_worth_a_row_of_each_kind();
        // 失败的卷是这一种行：卷级那一段里多出来的那一行，成句、只有一格。
        let volume_rows = volume(&isolated);
        let marked = volume_rows
            .iter()
            .find(|row| row.kind == RowKind::Isolated)
            .expect("有页失败的卷该有隔离那一行");
        assert_eq!(marked.cells.len(), 1);
        assert!(
            marked
                .cell(Field::Sentence)
                .expect("隔离那一句")
                .starts_with("隔离 1 页失败"),
            "{marked:?}"
        );
        // 一页都没失败的卷没有这一行——一行在不在场本身就是一句话。
        let mut skipped = extracted_by(0);
        skipped.verdict = Some(VolumeVerdict::Skipped { page_count: 12 });
        assert!(
            volume(&skipped)
                .iter()
                .all(|row| row.kind != RowKind::Isolated)
        );

        let rows = pages(&isolated);

        let failure = rows.last().expect("末一行是失败那一页的判定");
        assert_eq!(failure.kind, RowKind::PageFailure);
        assert_eq!(failure.cells.len(), 1);
        assert_eq!(
            failure.cell(Field::Sentence),
            Some("失败 解 library/volume-a/002.jpg 这一页: 判定格式")
        );
        // 失败页的几何那一行说得出它的尺寸是**卷内统一**的，不是它自己缩出来的。
        assert_eq!(
            rows[2].cell(Field::Scaling),
            Some("失败页 · 卷内统一尺寸留白")
        );
        // 没裁过、不是跨页、没退回过——那三格一个都不在场。
        for field in [Field::Crop, Field::Cut, Field::Backstop] {
            assert_eq!(rows[2].cell(field), None, "{field:?} 那一格不该在场");
        }
    }

    /// **没做成的那一卷自成一种行**，而那一句原因只有一处出处（Q133）。
    ///
    /// 它连 `VolumeReport` 都没有——[`volume`] 够不着它，[`failed_volume`] 单出这一行。
    /// 卷表读的是它，末尾那一小结（[`failed_volume_tail`]）摆的也是它：
    /// 一套措辞，两副排版。这一条钉的正是那个「一处」——报告里印出去的两行，
    /// 逐字来自这一行的两格。
    #[test]
    fn a_volume_that_never_got_done_is_its_own_kind_of_row() {
        let failure = VolumeFailure {
            volume: PathBuf::from("library/volume-b"),
            reason: "读 library/volume-b/ComicInfo.xml: 系统找不到指定的文件".to_owned(),
        };

        let row = failed_volume(&failure);

        assert_eq!(row.kind, RowKind::FailedVolume);
        // 两格：路径一格，那句原因整句装在一格里（成句，不拆）。
        assert_eq!(row.cells.len(), 2);
        assert_eq!(row.cell(Field::Source), Some("library/volume-b"));
        assert_eq!(row.cell(Field::Sentence), Some(failure.reason.as_str()));

        // 末尾那一小结摆的就是这一行：报告里那两行逐字来自这两格。
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            volumes: Vec::new(),
            failed_volumes: vec![failure],
            non_volume_files: Vec::new(),
            outcome: RunOutcome::Completed,
            elapsed: Duration::ZERO,
        };
        assert!(
            tail(&report).contains(&plain::line(&row)),
            "{}",
            tail(&report)
        );
    }

    /// **档位那一列写什么，五种说法只有一处**（[`base_column`]，P3 的卷表）。
    ///
    /// 判出了基准档的写那一档；剩下四种写的是**它为什么没有一档**，
    /// 与卷级那几行成句的那几句说的是同一件事。一张灰度页都没有的卷一种都对不上，
    /// 那时那一格不在场——表上留白，不编一个档出来。
    #[test]
    fn the_base_column_says_which_one_it_is_or_why_there_is_none() {
        let isolated = a_volume_worth_a_row_of_each_kind();
        assert_eq!(base_column(&volume(&isolated)), Some("4bit".to_owned()));

        let mut each = extracted_by(0);
        each.verdict = Some(VolumeVerdict::Skipped { page_count: 12 });
        assert_eq!(base_column(&volume(&each)), Some("跳过".to_owned()));

        each = a_volume_worth_a_row_of_each_kind();
        each.verdict = Some(VolumeVerdict::PerPage);
        assert_eq!(base_column(&volume(&each)), Some("逐页".to_owned()));

        each.verdict = Some(VolumeVerdict::Override(Candidate::new(
            BitDepth::Two,
            Dither::FloydSteinberg,
        )));
        assert_eq!(base_column(&volume(&each)), Some("覆盖 2bit+FS".to_owned()));

        let failure = VolumeFailure {
            volume: PathBuf::from("library/volume-b"),
            reason: "卷根不在了".to_owned(),
        };
        let gone = failed_volume(&failure);
        assert_eq!(
            base_column(std::slice::from_ref(&gone)),
            Some("没做成".to_owned())
        );

        // 一张灰度页都没有的卷（判定那一格不在场）：一种都对不上，那一格因此不在场。
        each.verdict = None;
        assert_eq!(base_column(&volume(&each)), None);
    }

    /// **格里装的是值，不是摆法**（ADR 0016）：缩进、换行与行尾那些分隔符一个都不在里面。
    ///
    /// 这一条挡的是把排版偷偷搬回措辞那一层的改动：真有人把那两个空格写进某一格，
    /// 纯文本那一副看不出任何异样，而表那一副会莫名其妙多出两格。
    #[test]
    fn a_cell_carries_a_value_and_never_an_indent_or_a_line_break() {
        let isolated = a_volume_worth_a_row_of_each_kind();
        let mut skipped = extracted_by(512 * 1024);
        skipped.verdict = Some(VolumeVerdict::Skipped { page_count: 12 });

        let rows: Vec<Row> = [&isolated, &skipped]
            .into_iter()
            .flat_map(|each| volume(each).into_iter().chain(pages(each)))
            .collect();

        assert!(rows.len() > 10, "问这一条要先有足够多的行：{}", rows.len());
        for row in &rows {
            for cell in &row.cells {
                assert!(
                    !cell.text.contains('\n'),
                    "{:?} 的 {:?} 那一格里有换行：{}",
                    row.kind,
                    cell.field,
                    cell.text
                );
                assert_eq!(
                    cell.text.trim(),
                    cell.text,
                    "{:?} 的 {:?} 那一格前后带着空白",
                    row.kind,
                    cell.field
                );
            }
        }
    }

    /// 四段各画各的，拼起来与一次性渲染出的**逐字节相同**（会话批的 02、09 号票）。
    ///
    /// 会话就是这么画的：抬头一次，**目录一枝一行**、卷级与逐页逐卷出，末尾收口。
    /// 这一条钉住的是「两边措辞只有一套」——真有人在 [`plain::report`] 里插了一行
    /// 别处没有的东西，这里当场红。
    ///
    /// **分组那一级也在里面**（`volume-discovery/08`）：命令行那一副与会话的目录表
    /// 读的是同一份（[`grouped`] 与 [`directory`]），这一条因此照着那两处逐枝拼一遍。
    #[test]
    fn drawing_the_four_parts_one_by_one_gives_the_same_bytes_as_one_shot() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let salvaged = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Salvaged {
                page: Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                },
                salvage: Salvage::from_share(0.625),
            },
        };
        // 两卷：一卷带着部分救回页（末尾那一小结因此在场），一卷是跳过的（它没有逐页那一段）。
        let mut report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            salvaged,
        );
        report.volumes.push(VolumeReport {
            volume: PathBuf::from("library/volume-b"),
            output: PathBuf::from("out/volume-b"),
            superseded: None,
            pages: Vec::new(),
            source_pages: 12,
            verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
            cache: cache_usage(),
            extracted: 0,
            io: io_plan(),
            decodes: 0,
            timing: VolumeTiming::default(),
        });

        let mut drawn = header(&report, Mode::Process);
        let listed = self::listed(&report);
        for group in grouped(&listed) {
            drawn.push_str(&plain::directory(&group, &listed));
            for at in &group.at {
                let Listed::Settled(each) = listed[*at] else {
                    continue;
                };
                drawn.push_str(&plain::volume(each));
                drawn.push_str(&plain::pages(each));
            }
        }
        drawn.push_str(&tail(&report));

        assert_eq!(drawn, plain::report(&report, Mode::Process));
    }

    /// 计时**不进渲染出的文字**：印不印、印在哪由调用方定（加固批 11 号票）。
    ///
    /// 断言写成「同一份报告只改计时，画出来逐字节相同」，而不是「文字里找不到某个数」：
    /// 后者只挡得住恰好那一个写法，而这一条挡得住任何一处把耗时印出去的改动——
    /// 真有人加了一行「耗时 3.2 秒」，两份当场分家。
    ///
    /// 黄金快照不因机器快慢而 flaky，靠的就是这一条：渲染一个计时字段都不读。
    #[test]
    fn the_rendered_text_says_nothing_about_how_long_it_took() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let quick = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        // 同一份报告，慢了一小时：每一个计时字段都改掉，别的一个字节都不动。
        let mut slow = quick.clone();
        slow.elapsed = Duration::from_secs(3600);
        for each in &mut slow.volumes {
            each.timing = VolumeTiming {
                fingerprint: Duration::from_secs(11),
                first_pass: Duration::from_secs(222),
                second_pass: Duration::from_secs(3333),
                elapsed: Duration::from_secs(3600),
            };
        }

        // 四段逐段比，而不只比拼起来的那一份：会话画的是这四段，
        // 只比总的，某一段说了而另一段抵消掉的情形就漏过去了。
        assert_eq!(header(&slow, Mode::Process), header(&quick, Mode::Process));
        for (slower, quicker) in slow.volumes.iter().zip(&quick.volumes) {
            assert_eq!(volume(slower), volume(quicker));
            assert_eq!(pages(slower), pages(quicker));
        }
        assert_eq!(tail(&slow), tail(&quick));
        assert_eq!(
            plain::report(&slow, Mode::Process),
            plain::report(&quick, Mode::Process)
        );
    }

    /// **摊了多少字节印进那一卷的报告**，而没摊开的卷一个字都不说
    /// （`volume-discovery/05`，ADR 0015 决定第 3 条）。
    ///
    /// 那笔代价落在磁盘上，而磁盘这一侧没有旋钮——这一行是它唯一可见的形式。
    /// 恒是 0 的那些卷（`.cbz` / `.zip` 与目录卷）印出来只是噪声，与末尾那几小结同一条规矩。
    ///
    /// **跳过的卷也说**：幂等那一道要把整卷的字节读一遍，而读之前得先摊开。
    /// 那一支排在跳过那道守卫**之前**，与「这一趟怎么读的」那一行同一条理由——
    /// 「跳过一卷为什么也占着几百兆临时目录」只有这一行答得出。
    #[test]
    fn a_volume_that_was_extracted_says_how_many_bytes_it_took() {
        let untouched = extracted_by(0);
        assert!(
            !plain::volume(&untouched).contains("摊开"),
            "没摊开的卷也印了那一行：{}",
            plain::volume(&untouched)
        );

        let said = plain::volume(&extracted_by(3 * 1024 * 1024));
        assert!(said.contains("摊开 3.0 MiB"), "{said}");
        // 字节数走库那一份进位：同一屏上它与缓存那一行得是同一种单位。
        assert!(
            said.contains(&tonefit::format_bytes(3 * 1024 * 1024)),
            "{said}"
        );

        let mut skipped = extracted_by(512 * 1024);
        skipped.verdict = Some(VolumeVerdict::Skipped { page_count: 1 });
        let said = plain::volume(&skipped);
        assert!(
            said.contains("摊开 512.0 KiB"),
            "跳过的卷没说摊了多少：{said}"
        );
    }

    /// 一份摊开了 `extracted` 字节的卷报告，逐页那一摞是空的——这一条只问卷级那几行。
    fn extracted_by(extracted: u64) -> VolumeReport {
        VolumeReport {
            volume: PathBuf::from("library/volume-a.7z"),
            output: PathBuf::from("out/volume-a.cbz"),
            superseded: None,
            verdict: None,
            cache: cache_usage(),
            extracted,
            io: io_plan(),
            decodes: 0,
            timing: VolumeTiming::default(),
            pages: Vec::new(),
            source_pages: 0,
        }
    }

    /// 一份**只答得出档位**的卷报告：这几条问的是分组与目录那一行，不是逐页。
    ///
    /// `verdict` 是 `None` 时它一档都判不出来（整卷彩页、整卷失败那一档），
    /// [`base_column`] 对它答的正是「不在场」。
    fn a_volume(root: &str, verdict: Option<VolumeVerdict>, broken: bool) -> VolumeReport {
        let page = PageReport {
            source: PathBuf::from(format!("{root}/001.jpg")),
            output: PathBuf::from("out/001.png"),
            size: Size::new(1264, 1680),
            outcome: match broken {
                true => PageOutcome::Failed {
                    reason: "解不出完整尺寸：JPEG 数据截断".to_owned(),
                },
                false => PageOutcome::Whole(Processed {
                    crop: nothing_trimmed(),
                    backstopped: false,
                    cut: None,
                    spread_candidate: false,
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: Vec::new(),
                        verdict: Verdict {
                            candidate: Candidate::new(BitDepth::Four, Dither::Off),
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        };
        VolumeReport {
            volume: PathBuf::from(root),
            output: PathBuf::from("out"),
            superseded: None,
            verdict,
            cache: cache_usage(),
            extracted: 0,
            io: io_plan(),
            decodes: 1,
            timing: VolumeTiming::default(),
            pages: vec![page],
            source_pages: 1,
        }
    }

    /// **分组的层次就是发现出来那棵树的层次**（`volume-discovery/08` 票面第二条）。
    ///
    /// 一个目录**既是卷又装着卷**（ADR 0014）时两条各归各的：那个目录卷自己归
    /// **它的父目录**那一组，它装着的那几卷归**它自己**那一组——`crate::discover`
    /// 列一层目录时，列出来的每个候选的父目录就是它站着的那一层，这里问的正是那条边。
    ///
    /// **不重排**：目录按它头一条在报告上的先后排，组内也按报告上的先后。
    /// 发现走的是先序深度优先，同一枝底下那几卷在报告上**本来就不挨着**——
    /// 这一条特意把子目录那一卷夹在中间，收出来的两组仍各自成序。
    #[test]
    fn the_directories_are_the_branches_of_the_tree_discovery_walked() {
        let base = Some(VolumeVerdict::Envelope(envelope(Candidate::new(
            BitDepth::Four,
            Dither::Off,
        ))));
        let volumes = [
            a_volume("库/作品", base, false),
            a_volume("库/作品/子集", base, false),
            a_volume("库/作品/子集/第1话.cbz", base, false),
            a_volume("库/作品/第10话.cbz", base, false),
        ];
        let listed: Vec<Listed<'_>> = volumes.iter().map(Listed::Settled).collect();

        let groups = grouped(&listed);

        let names: Vec<String> = groups
            .iter()
            .map(|group| group.directory.display().to_string())
            .collect();
        assert_eq!(names, ["库", "库/作品", "库/作品/子集"], "分的不是那棵树");
        // 组内按报告上的先后：`库/作品` 那一枝底下的两卷在报告上隔着两条。
        assert_eq!(groups[0].at, [0], "目录卷自己归它的父目录");
        assert_eq!(groups[1].at, [1, 3], "它装着的那几卷归它自己那一组");
        assert_eq!(groups[2].at, [2]);
    }

    /// **目录那一行说的是几卷 · 基准档怎么分布 · 几卷进了隔离**（票面第一条）。
    ///
    /// 分布逐条问 [`base_column`]——卷表那一列写的是同一批字，跳过与没做成也在里面；
    /// **卷多的排在前**，一样多的按头一次出现的先后。
    /// 隔离那一格**一卷都没有就不在场**（一格在不在场本身就是一句话）。
    #[test]
    fn a_directory_row_counts_its_volumes_and_spreads_their_bases() {
        let four = Some(VolumeVerdict::Envelope(envelope(Candidate::new(
            BitDepth::Four,
            Dither::Off,
        ))));
        let volumes = [
            a_volume(
                "库/第1话",
                Some(VolumeVerdict::Skipped { page_count: 1 }),
                false,
            ),
            a_volume("库/第2话", four, false),
            a_volume("库/第3话", four, true),
        ];
        let failures = [VolumeFailure {
            volume: PathBuf::from("库/第4话"),
            reason: "卷根不在了".to_owned(),
        }];
        let listed: Vec<Listed<'_>> = volumes
            .iter()
            .map(Listed::Settled)
            .chain(failures.iter().map(Listed::Failed))
            .collect();
        let groups = grouped(&listed);
        assert_eq!(groups.len(), 1, "四条都在同一枝上");

        let row = directory(&groups[0], &listed);

        assert_eq!(row.kind, RowKind::Directory);
        assert_eq!(row.cell(Field::Source), Some("库"));
        // 没做成的那一卷也算一卷：它同样是这一枝底下点到过的卷。
        assert_eq!(row.cell(Field::VolumeCount), Some("4"));
        // 多的在前：4bit 两卷，跳过与没做成各一卷（后两者按头一次出现的先后）。
        assert_eq!(row.cell(Field::Bases), Some("4bit 2 · 跳过 1 · 没做成 1"));
        // 带失败页那一卷进了隔离，没做成那一卷不算（它连一份卷报告都没有）。
        assert_eq!(row.cell(Field::Isolated), Some("1"));

        // 一卷都没进隔离时那一格不在场。
        let clean = [a_volume("库/第1话", four, false)];
        let listed: Vec<Listed<'_>> = clean.iter().map(Listed::Settled).collect();
        let row = directory(&grouped(&listed)[0], &listed);
        assert_eq!(row.cell(Field::Isolated), None);
        assert_eq!(row.cell(Field::VolumeCount), Some("1"));

        // 一卷都判不出档位时分布那一格也不在场（整卷彩页、整卷失败那一档）。
        let unjudged = [a_volume("库/第1话", None, true)];
        let listed: Vec<Listed<'_>> = unjudged.iter().map(Listed::Settled).collect();
        let row = directory(&grouped(&listed)[0], &listed);
        assert_eq!(row.cell(Field::Bases), None);
    }

    /// **命令行那一副把目录那一行摆在它那几卷前面**（票面第三条）。
    ///
    /// 「两边拿的是同一份报告数据」在这里问得出来：印出去的那一行逐字就是
    /// [`directory`] 出的那一行摆出来的样子，会话的目录表读的是同一批格。
    #[test]
    fn the_command_line_folds_the_report_by_directory() {
        let base = Some(VolumeVerdict::Envelope(envelope(Candidate::new(
            BitDepth::Four,
            Dither::Off,
        ))));
        let mut report = one_page_report(
            Profile::resolve("kobo-libra-2").expect("内置型号"),
            VolumeVerdict::Skipped { page_count: 1 },
            a_volume("库/第1话", base, false).pages.remove(0),
        );
        report.volumes[0] = a_volume("库/第1话", base, false);
        report
            .volumes
            .push(a_volume("库/别的作品/第1话", base, false));

        let text = plain::report(&report, Mode::Process);

        let listed = self::listed(&report);
        for group in grouped(&listed) {
            let said = plain::directory(&group, &listed);
            assert!(text.contains(said.trim_end()), "{said} 不在报告里：{text}");
        }
        // 两枝各一行，摆在它那一卷**前面**。
        let at = |said: &str| text.find(said).unwrap_or_else(|| panic!("{said}：{text}"));
        assert!(
            at("库  1 卷") < at("库/第1话 → "),
            "目录那一行没摆在前面：{text}"
        );
        assert!(
            at("库/别的作品  1 卷") < at("库/别的作品/第1话 → "),
            "{text}"
        );
    }
}
