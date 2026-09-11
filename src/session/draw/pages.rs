//! 屏上那一块：**逐页表**——展开一卷之后报告区里一页一行的那张表
//! （`CONTEXT.md` 的《会话》：展开、要紧的页）。
//!
//! ```text
//!  记号  页名     尺寸       缩放                       判定     理由              判据
//!  ✓     001.jpg  1182x1680  缩放比 1.219 ⋅ 未预缩      4bit     阈值内最低的一档  1bit+FS 32.000 ⋅ ⋯
//!  *     087.jpg  1182x1680  缩放比 1.219 ⋅ 未预缩      2bit+FS  卷级上包络        ⋯                   定档页
//!  !     104.jpg  1182x1680  缩放比 1.219 ⋅ 未预缩      4bit     特例页单独定档    ⋯                   特例页
//!  ✗     017.jpg  1182x1680  失败页 ⋅ 卷内统一尺寸留白                                                 失败 解不出完整尺寸
//! ```
//!
//! 上面那一副是**砍剩的那几列**：这一卷一页都没裁过、没切过、没有彩页转灰，
//! 那三列因此连列头都不占，而去处那一列让给了宽度。
//! 十一列的全套与[砍列的次序](PageColumn)在 [`crate::session::columns`] 那一处。
//!
//! # 默认只列**要紧的页**
//!
//! 展开一卷的目的通常只有一个——**哪一页把整卷拉下来**——而两百页的卷里那几页不该由
//! 用户自己在四百行里找（`p3-session-legibility/11`）。「哪几页要紧」的判据**不在这里**，
//! 在 [`crate::render::notable`]：那一处与逐页那几行、与命令行印出去的那一份挨着，
//! 而这一层只把判出来的那几种摆成屏上的一个词与一个[记号](Mark)。
//! `a` 切到[全部页](Listing::All)。
//!
//! # 它与[卷表](super::table)是一对
//!
//! 同一批行（[`crate::render::pages`]）、同一套[砍列](crate::session::columns)、
//! 同一份[视口](crate::session::viewport::Viewport)、同一组[语义色](Tone)——
//! 差的只有列（[`PageColumn`]）与记号那几种。措辞照旧一个字都不在这里重写：
//! 表上那十列与失败页那一句全出自 [`crate::render`]。
//!
//! 长在这一层的只有**命令行上根本没有**的那三样：列头、[行首记号](Mark)、
//! 以及[要紧在哪儿那几个词](says)。
//!
//! # 逐页那几格**一格不少**
//!
//! 裁边、缩放、跨页哪一侧、彩页转灰、去处五格从前留在表外，`p4-parking-lot/10`
//! 把它们收了进来（停车场 Q162）。跟着回来的是失败页那一句
//! **它的尺寸是卷内统一尺寸**（[`Field::Scaling`] 那一格，`p1-session/11` 的验收）。
//!
//! **摆不下时它们仍是先让的那几列**（[`PageColumn::DROPPED_IN_TURN`](Column::DROPPED_IN_TURN)）：
//! 「哪一页把整卷拉下来」才是这一副要答的那一问，而**摆不下**与**不给**是两回事——
//! 屏一宽它们就回来。命令行印出来的那一份照旧一格不少（同一批格，另一副排版，ADR 0016）。
//!
//! **[兜底退回](Field::Backstop)那一格仍不进表**，而它不是漏掉的：那件事由
//! [行尾那个词](says)（`兜底上界`）说，两处都摆就是同一件事说两遍。

use tonefit::{Mode, PageReport, Panel, VolumeReport, WhiteAlignLimit};

use super::paint::Painted;
use super::table::{Table, driver};
use crate::render::{self, Field, Notable, Row, RowKind};
use crate::session::columns::{self, Column, PageColumn, Widths};
use crate::session::state::Listing;
use crate::session::tone::Tone;

/// 行首记号：**这一页要不要紧，一个字符说完。**
///
/// # 记号与语义色在这里绑成一对
///
/// 与[卷表那一头](super::table)同一个形状（停车场 Q153）：一页要紧在哪几处由
/// [`crate::render::notable`] 判一次，判出来的每一种[配一个记号](says)，
/// 而记号既定了行首那个字符（[`glyph`](Self::glyph)），也定了这一行的
/// [语义](Tone)（[`tone`](Self::tone)）。「颜色不是唯一载体」因此不靠人记着——
/// 添一种要紧法不配记号根本编不过去。
///
/// **一页可以同时要紧在好几处**（以高为准的跨页卷里，定档页多半也是一张宽溢出的页），
/// 而一行只有一种语义：取**最重的那一个**（[`Ord`] 从轻到重排，与 [`Tone`] 同一个做法）。
/// 具体要紧在哪几处不靠这一个字符说，靠行尾那几个词（[`says`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mark {
    /// 不要紧的一页：判定跟着卷级基准档走，几何与解码都没出过事。
    Fine,
    /// **定档页**：这一卷的基准档就是它判出来的。不是出了事，是这一卷的答案。
    Driver,
    /// 要留神的一页：特例、部分救回、几何门不成立、宽溢出、兜底上界，五者之一。
    Caution,
    /// 失败页：这一页根本没解出来。
    Failed,
}

impl Mark {
    /// 屏上那一个字符。
    ///
    /// 定档页取 `*` 而不是另一个记号：它与 `✓` 一样不是坏事，
    /// 而一颗星在一屏 `✓` 里一眼看得出来——「这一卷的档是它定的」正是要一眼看见的那件事。
    fn glyph(self) -> char {
        match self {
            Self::Fine => '✓',
            Self::Driver => '*',
            Self::Caution => '!',
            Self::Failed => '✗',
        }
    }

    /// 这一种记号是哪一种[语义](Tone)——**这一行整行按它上色**。
    ///
    /// 逐条对上 `CONTEXT.md` 的《语义色》：失败页是「出事」，那五样是「注意」，
    /// 定档页与普通页都是「平常」——定档页在四档里一档都不占，它不是一件要留神的事。
    fn tone(self) -> Tone {
        match self {
            Self::Fine | Self::Driver => Tone::Plain,
            Self::Caution => Tone::Caution,
            Self::Failed => Tone::Trouble,
        }
    }
}

/// 一页**要紧在哪儿**在屏上写成什么样：行尾那个词，加上它挂哪一个[记号](Mark)。
///
/// **判据不在这里**（在 [`crate::render::notable`]）：这一处只答「屏上怎么说」。
/// 那几个词逐字取自 `CONTEXT.md` 的《语义色》那一档里列的名字——
/// 它们在 spec、在报告末尾那几小结、在这里说的是同一批事。
///
/// **两种不给词**，理由同一条：屏上本来就跟着 [`crate::render`] 出的那一格，
/// 而那一格自己就说得出这件事——多加一个词是同一件事说两遍。
///
/// - [失败页](Notable::Failed)行尾跟着那一句原因，它以「失败」开头；
/// - [部分救回](Notable::Salvaged)行尾跟着 `Field::Salvage` 那一格（`救回 62.0%`），
///   而它比一个词多说了**救回了多少**——救回 5% 与救回 95% 是两回事，
///   一个「部分救回」把那个数抹平了。
fn says(what: Notable) -> (Option<&'static str>, Mark) {
    match what {
        Notable::Failed => (None, Mark::Failed),
        Notable::Salvaged => (None, Mark::Caution),
        Notable::Outlier => (Some("特例页"), Mark::Caution),
        Notable::OutsideTheGate => (Some("几何门不成立"), Mark::Caution),
        Notable::Overflowed => (Some("宽溢出"), Mark::Caution),
        Notable::Backstopped => (Some("兜底上界"), Mark::Caution),
        Notable::Driver => (Some("定档页"), Mark::Driver),
    }
}

/// 表上的一行：一页。
///
/// 不在场的格是 `None`——**一格在不在场本身就是一句话**（`CONTEXT.md` 的《格》）：
/// 失败页没有判定，走彩色分支的页没有判据。
struct Entry {
    /// 行首记号：这一页要紧的那几处里最重的那一个。
    mark: Mark,
    /// 页名：**源那一侧的成员名**，只印最后那一段。
    ///
    /// 走 [`crate::render::volume_name`]，与卷表定档页那一列、与这一副的
    /// [抬头](heading)印的是同一个——三处要是各取各的，抬头说 `001.jpg`
    /// 而表上写 `001.png`，读的人会以为那是两页。
    name: String,
    /// 这一页的输出尺寸。
    size: Option<String>,
    /// 裁边裁掉了多少。一个像素都没裁就不在场。
    crop: Option<String>,
    /// 缩放怎么算的；失败页说的是它的尺寸从哪来。
    scaling: Option<String>,
    /// 跨页切出来的哪一半。不是切出来的就不在场。
    cut: Option<String>,
    /// 这一页是彩页转灰。不是就不在场。
    color_to_gray: Option<String>,
    /// 这一页判成的那一档。
    verdict: Option<String>,
    /// 判成这一档的理由。
    reason: Option<String>,
    /// 各候选的判据值排成一串。
    scores: Option<String>,
    /// 去处：这一页写到哪个文件。
    output: Option<String>,
    /// 跟在行尾的那几句：**成句或成词，不塞进格**——要紧在哪几处那几个词，
    /// 外加成句的那一行（失败页那一句原因、彩色分支那一句）。
    ///
    /// 摆不下时整行折下去，走 [`crate::wrap`]，与卷表那一头同一条。
    notes: Vec<String>,
}

impl Entry {
    /// 一页开头那一行（[几何](RowKind::PageGeometry)）：尺寸、裁边、缩放、跨页与去处
    /// 五格从这里来。
    ///
    /// **名字不从这里来**——那一行上只有去处（一条输出路径），而这一副印的是源那一侧的
    /// 成员名（见 [`name`](Self::name)）：它由 [`named`](Self::named) 补上。
    /// 去处那一格照旧收下，它自己占一列。
    fn opened(row: &Row) -> Self {
        Self {
            mark: Mark::Fine,
            name: String::new(),
            size: row.cell(Field::Size).map(str::to_owned),
            crop: row.cell(Field::Crop).map(str::to_owned),
            scaling: row.cell(Field::Scaling).map(str::to_owned),
            cut: row.cell(Field::Cut).map(str::to_owned),
            color_to_gray: None,
            verdict: None,
            reason: None,
            scores: None,
            output: row.cell(Field::Output).map(str::to_owned),
            notes: Vec::new(),
        }
    }

    /// 一页第二行（判定／彩色分支／失败）：彩页转灰、判定那三格与跟在行尾的那两格
    /// 从这里来。
    fn closed(&mut self, row: &Row) {
        self.color_to_gray = row.cell(Field::ColorToGray).map(str::to_owned);
        self.verdict = row.cell(Field::Candidate).map(str::to_owned);
        self.reason = row.cell(Field::Reason).map(str::to_owned);
        self.scores = row.cell(Field::Scores).map(str::to_owned);
        // **救回了多少**跟在行尾（`救回 62.0%`）：它是[部分救回](Notable::Salvaged)
        // 那一档在屏上的载体，而且比一个词多说了一个数（见 [`says`]）。
        self.notes
            .extend(row.cell(Field::Salvage).map(str::to_owned));
        // **这一页的纸白与钳制宽度**同样跟在行尾（`纸白 253 ⋅ 钳制 2 级`）：它只在试算那一副
        // 出（见 [`crate::render::pages`]），一列专门留给它就意味着执行那一趟整列空着。
        // 措辞连同「纸白」「钳制」那几个字都在格里，这一层一个字都没添。
        self.notes
            .extend(row.cell(Field::PaperWhite).map(str::to_owned));
        // 成句的那一行（失败页、彩色分支）整句跟在行尾：它本来就是句子，拆成格没有意义。
        self.notes
            .extend(row.cell(Field::Sentence).map(str::to_owned));
    }

    /// 这一页叫什么：源那一侧的成员名，只留最后那一段（见 [`name`](Self::name)）。
    fn named(&mut self, page: &PageReport) {
        self.name = render::volume_name(&page.source);
    }

    /// **这一页要紧吗**——默认那一副列的就是它为真的那几页。
    ///
    /// 问的是记号而不是另存一个布尔量：[每一种要紧法都配着一个比
    /// `Fine` 重的记号](says)，两者因此逐条等价，而多存一个数就多一处对不上的可能。
    fn matters(&self) -> bool {
        self.mark > Mark::Fine
    }

    /// 这一页要紧在哪几处：记号取最重的那一个，那几个词按判据给的次序跟在行尾。
    ///
    /// 那几个词排在成句的那一句**前面**：句子摆不下时整行折下去，而这几个词是扫一眼
    /// 就要看见的东西。
    fn notable(&mut self, why: &[Notable]) {
        let mut words = Vec::new();
        for what in why {
            let (word, mark) = says(*what);
            self.mark = self.mark.max(mark);
            words.extend(word.map(str::to_owned));
        }
        words.append(&mut self.notes);
        self.notes = words;
    }

    /// 这一列上写什么。记号那一列自己答：它恒在，而它不是一格字，是一个[记号](Mark)。
    ///
    /// **不在场的那几格一个字都不占**（`CONTEXT.md` 的《格》：一格在不在场本身就是
    /// 一句话）——卷表那一处的 `ABSENT` 治的是**夹在中间**的页数，而这张表上真会不在场的
    /// 那几格（裁边、跨页、彩页转灰、判定那三格）空着就是「这一页没有这件事」。
    fn text(&self, column: PageColumn) -> String {
        match column {
            PageColumn::Mark => self.mark.glyph().to_string(),
            PageColumn::Name => self.name.clone(),
            PageColumn::Size => self.size.clone().unwrap_or_default(),
            PageColumn::Crop => self.crop.clone().unwrap_or_default(),
            PageColumn::Scaling => self.scaling.clone().unwrap_or_default(),
            PageColumn::Cut => self.cut.clone().unwrap_or_default(),
            PageColumn::ColorToGray => self.color_to_gray.clone().unwrap_or_default(),
            PageColumn::Verdict => self.verdict.clone().unwrap_or_default(),
            PageColumn::Reason => self.reason.clone().unwrap_or_default(),
            PageColumn::Scores => self.scores.clone().unwrap_or_default(),
            PageColumn::Output => self.output.clone().unwrap_or_default(),
        }
    }
}

/// 这一卷逐页各一行，**外加它要紧在哪几处**。
///
/// 行从 [`crate::render::pages`] 来（一页两行：几何一行、判定一行），
/// 要紧在哪几处从 [`crate::render::notable`] 来——两处都是逐页、同序，
/// 这里只把它们并到一行上。**跳过的卷两处都是空的**：它一页都没重做。
fn entries(volume: &VolumeReport, panel: Panel, mode: Mode) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    // 按「几何那一行起一页」分组，而不是按「每两行一页」切：后者把
    // `render::pages` 的行数写死在这里，添一行就悄悄错位。
    for row in render::pages(volume, mode) {
        match row.kind {
            RowKind::PageGeometry => entries.push(Entry::opened(&row)),
            _ => {
                if let Some(entry) = entries.last_mut() {
                    entry.closed(&row);
                }
            }
        }
    }
    // 名字与「要紧在哪几处」都按**页序**并上去：两者与 `volume.pages` 等长同序
    // （见 [`crate::render::notable`]），而上面那一圈也是照那个次序摆出来的。
    for ((entry, page), why) in entries
        .iter_mut()
        .zip(&volume.pages)
        .zip(render::notable(volume, panel))
    {
        entry.named(page);
        entry.notable(&why);
    }
    entries
}

/// 展开一卷之后这一格里的东西：**钉住的那一行抬头**，加上**表**。
///
/// 两样装在一个类型里而不是一对裸值，与 [`Table`] 同一条理由：它们是同一次拼出来的，
/// 而「第二个 `String` 是什么」在调用处看不出来。
pub(super) struct Opened {
    /// 钉在这一格顶上的那一行：这一卷的档位分布、定档页、这一副列着几页。
    ///
    /// **它不随表滚**（见 [`super::report`]）：逐页翻到第三屏时「这一卷判成哪一档」
    /// 还得答得出来，而那正是翻这几页要比的东西。
    pub(super) heading: String,
    /// 表那几行，外加光标停在第几行。
    pub(super) table: Table,
}

/// **逐页表**：列头一行，此后一页一行。
///
/// `room` 是这一格里正文摆得下几列；`listing` 是这一副列[要紧的页](Listing::Notable)
/// 还是[全部页](Listing::All)；`at` 是光标停在列出来的第几页上
/// （越界不算错，就近收到最后一页上——与 [`Viewport`](crate::session::viewport::Viewport)
/// 那条同一个规矩）。
///
/// **一页都列不出来时给的是一句话，不是一张空表**（`p3-session-legibility/11` 票面
/// 第三条）：跳过的卷根本没有逐页结果，而一张要紧的页都没有的卷是**一句好消息**——
/// 空表说不出这件事，它读起来像画坏了。
///
/// `mode` 与 `limit` 是**这一趟的前提**，两样都只为纸白对齐那一项递进来
/// （纸白对齐批 02 号票）：逐页那一格纸白只在 `--dry-run` 出，
/// 而抬头读的那几行卷级行里有一行要照本次上限说话。
pub(super) fn pages(
    volume: &VolumeReport,
    panel: Panel,
    mode: Mode,
    limit: WhiteAlignLimit,
    room: u16,
    listing: Listing,
    at: usize,
) -> Opened {
    let all = entries(volume, panel, mode);
    let notable = all.iter().filter(|entry| entry.matters()).count();
    let heading = heading(volume, limit, listing, notable, all.len());
    let shown: Vec<&Entry> = match listing {
        Listing::Notable => all.iter().filter(|entry| entry.matters()).collect(),
        Listing::All => all.iter().collect(),
    };
    let table = match nothing_to_list(volume, listing, shown.is_empty()) {
        Some(said) => Table {
            rows: vec![said],
            cursor: None,
        },
        None => laid_out(&shown, &all, room, at),
    };
    Opened { heading, table }
}

/// 钉在这一格顶上那一行：**这一卷的档位分布 · 定档页 · 这一副列着几页**（票面：抬头钉住）。
///
/// 前两格与卷表上那两列**同一个出处**（[`crate::render::tally_column`] 与
/// [`driver`]）：展开着的时候卷表不在屏上，而「这一卷各页写成了哪几档、是哪一页定的」
/// 正是逐页那几行要比的东西。不在场就不出：跳过的卷没有定档页，
/// 默认逐页那一趟也没有（`two-pass-rework/02`——定档页只在 `--envelope` 那条路上在场）。
///
/// 末一格说的是[这一副列的是哪几页](Listing)——**切换状态屏上看得出**就落在它身上
/// （屏底那一行摆的是那个键，见 [`super::footer`]：按键提示的家是屏底，状态的家是抬头）。
///
/// 「它是第几卷」不在这里：那个数在报告区那一格的抬头上（[`super::report::report_title`]），
/// 一个数不摆两处。
fn heading(
    volume: &VolumeReport,
    limit: WhiteAlignLimit,
    listing: Listing,
    notable: usize,
    total: usize,
) -> String {
    let rows = render::volume(volume, limit);
    let mut said = Vec::new();
    if let Some(tally) = render::tally_column(&rows) {
        // 有分布的卷挂上列头；一页都没判的卷（跳过）只剩那一个词，列头不挂——
        // 「档位分布 跳过」不是一句话，表上那一列有列头撑着，抬头没有。
        said.push(if rows.iter().any(|row| row.kind == RowKind::Tally) {
            format!("档位分布 {tally}")
        } else {
            tally
        });
    }
    if let Some(driver) = driver(&rows) {
        said.push(format!("定档页 {driver}"));
    }
    // 一页逐页结果都没有的卷（跳过）不说这一句：那一格里的那句话已经说清为什么。
    if total > 0 {
        said.push(match listing {
            Listing::Notable => format!("要紧的页 {notable}/{total}"),
            Listing::All => format!("全部 {total} 页（要紧的 {notable} 页）"),
        });
    }
    format!(" {}", said.join(" · "))
}

/// 一页都列不出来时那一句话。列得出来就是 `None`。
///
/// 两种，各说各的：**跳过的卷**根本没有逐页结果（不要紧那一档：它不是坏消息，
/// 也不是这一趟做的事）；**一张要紧的页都没有**是一句好消息，照实说出来。
fn nothing_to_list(volume: &VolumeReport, listing: Listing, empty: bool) -> Option<Painted> {
    if !empty {
        return None;
    }
    if volume.skipped() {
        return Some(Painted::new(
            " 这一卷幂等命中，一页都没有重做：没有逐页结果可看。".to_owned(),
            Tone::Muted,
        ));
    }
    Some(match listing {
        Listing::Notable => Painted::plain(format!(
            " 这一卷没有要紧的页：{} 页里没有一页出事、也没有一页被摘出去，卷级判定也不是哪一页定出来的。",
            volume.pages.len()
        )),
        // 全部页那一副也空：这一卷连一页都没有。真实素材上到不了（一页都没有的东西不是卷），
        // 摆在这里是为了不给一张连列头都没有的空表。
        Listing::All => Painted::plain(" 这一卷一页都没有。".to_owned()),
    })
}

/// 把列出来的那几页摆成表：列头一行，此后一页一行。
///
/// `shown` 是这一副真列出来的那几页，`all` 是**整卷**那几页——两样都要，而且
/// [各答一半](Widths::note)：
///
/// - **哪几列在场按整卷算**（`all`）：`a` 换的是列哪几页，屏上不该跟着换一副表。
///   彩页转灰与跨页都不在[要紧的页](crate::render::Notable)那七种里，
///   只按 `shown` 量的话，一卷里那几页恰好都不要紧时整列会随 `a` 一开一合。
/// - **每一列多宽按真列出来的那几行算**（`shown`）：列不出来的那几行一个像素都不占。
///
/// 每一行带着它是哪一种[语义](Tone)（[`Painted`]），画它的那一头照那一种上色——
/// 一行的语义由行首那个[记号](Mark)说了算，本模块因此一个颜色名都不写。
/// **反白不在这里上**：它不是语义色（见 [`super::paint`]），落哪一行由
/// [`Table::cursor`] 说了算。
fn laid_out(shown: &[&Entry], all: &[Entry], room: u16, at: usize) -> Table {
    let room = usize::from(room).saturating_sub(1);
    let mut widths: Widths<PageColumn> = Widths::new();
    for entry in all {
        for column in PageColumn::ALL {
            widths.note(*column, &entry.text(*column));
        }
    }
    for entry in shown {
        for column in PageColumn::ALL {
            widths.widen(*column, &entry.text(*column));
        }
    }
    // 砍列，砍无可砍再把页名收窄——与卷表同一处出处（[`columns::plan`]）。
    let kept = columns::plan(room, &mut widths);
    let mut rows = vec![Painted::plain(columns::lay(
        &kept,
        &widths,
        |column| column.head().to_owned(),
        &[],
    ))];
    for entry in shown {
        rows.push(Painted::new(
            columns::lay(&kept, &widths, |column| entry.text(column), &entry.notes),
            entry.mark.tone(),
        ));
    }
    Table {
        // 光标停在第 `at` 页上，也就是表上的第 `at + 1` 行——列头占着第零行。
        // **越界就近收到最后一页上**：列的东西刚换过一副时它会越界，而那一帧仍要画得出来。
        cursor: (!shown.is_empty()).then(|| at.min(shown.len() - 1) + 1),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **逐页表自己造的那几个字形在哪种终端上都占一格**（判据见
    /// [`tonefit::width_is_stable`]，停车场 Q154）。
    ///
    /// 与卷表那一条同一件事，两张表因此各问各的：添一种记号不过这一关就红。
    #[test]
    fn every_glyph_this_table_makes_is_the_same_width_on_any_terminal() {
        for mark in [Mark::Fine, Mark::Driver, Mark::Caution, Mark::Failed] {
            let glyph = mark.glyph();
            assert!(
                tonefit::width_is_stable(glyph),
                "{mark:?} 那个记号 {glyph} 是东亚歧义宽度"
            );
        }
    }
    use crate::render::Cell;
    use crate::session::live::fixture;

    /// 这一趟那块面板。宽溢出比的就是它的宽。
    fn panel() -> Panel {
        fixture::request(tonefit::Mode::DryRun).profile.panel()
    }

    /// 表上那几行的字（列头不算）。
    fn body(opened: &Opened) -> Vec<String> {
        opened
            .table
            .rows
            .iter()
            .skip(1)
            .map(|row| row.text.clone())
            .collect()
    }

    /// **默认只列要紧的页，`a` 切到全部页**（票面第二条）。
    ///
    /// 夹具那一卷八页，要紧的六页——普通那两页只在全部页那一副上。
    /// 每一种要紧法在屏上都另有一个词（「颜色不是唯一载体」），失败页那一句原因照旧成句。
    #[test]
    fn the_default_listing_is_the_pages_that_matter_and_a_shows_them_all() {
        let volume = fixture::a_page_of_every_kind("卷二");

        let notable = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            200,
            Listing::Notable,
            0,
        );
        let all = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            200,
            Listing::All,
            0,
        );

        assert_eq!(body(&notable).len(), 6, "{:?}", body(&notable));
        assert_eq!(body(&all).len(), 8, "{:?}", body(&all));
        // 六种要紧法各带一个词（失败页那一句本来就以「失败」开头，不再多一个词）。
        let said = body(&notable).join("\n");
        for word in [
            "定档页",
            "特例页",
            "几何门不成立",
            "宽溢出",
            "兜底上界",
            // 部分救回与失败两档**不给词**：行尾跟着的那一格自己就说得出，
            // 而它比一个词多说了一个数（见 [`says`]）。
            "救回 62.0%",
            "失败 解不出完整尺寸",
        ] {
            assert!(said.contains(word), "{word} 没说出来：{said}");
        }
        // **一页要紧在好几处时，那几个词一个不少**（`004` 是特例页，而且它宽溢出）。
        let both = body(&notable)
            .into_iter()
            .find(|row| row.contains("004.jpg"))
            .expect("特例那一页在");
        assert!(both.contains("特例页") && both.contains("宽溢出"), "{both}");
        // 普通那两页只在全部页那一副上，而它们身上一个要紧的词都没有。
        // 彩色分支那一句也在那一副上——它只有逐页那几行说得出。
        let only_all = body(&all).join("\n");
        assert!(only_all.contains("001.jpg"), "{only_all}");
        assert!(only_all.contains("彩色分支：只缩放，不量化"), "{only_all}");
        assert!(!said.contains("001.jpg"), "普通页混进了要紧那一副：{said}");
        assert!(!said.contains("002.jpg"), "彩页混进了要紧那一副：{said}");
    }

    /// **一张要紧的页都没有的卷说一句话，不给一张空表**（票面第三条）。
    ///
    /// 跳过的卷另说一句：它根本没有逐页结果，而那与「没有要紧的页」不是一回事。
    #[test]
    fn a_volume_with_nothing_worth_listing_says_so_instead_of_showing_an_empty_table() {
        // 默认逐页那一卷没有定档页，一页也没出过事：要紧的页因此一张都没有。
        let plain = fixture::per_page_volume("名侦探 05");
        let opened = pages(
            &plain,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            120,
            Listing::Notable,
            0,
        );
        assert_eq!(opened.table.rows.len(), 1, "给了一张表");
        assert!(opened.table.rows[0].text.contains("没有要紧的页"));
        assert_eq!(opened.table.cursor, None, "一行都没有却有光标");
        // 抬头照旧说得出这一副列着几页——那个数就是「零张要紧的」。
        assert!(
            opened.heading.contains("要紧的页 0/1"),
            "{}",
            opened.heading
        );
        // 切到全部页就有表了：那一页在。
        let all = pages(
            &plain,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            120,
            Listing::All,
            0,
        );
        assert_eq!(body(&all).len(), 1);

        // 跳过的卷两副都是那一句，而它是「不要紧」那一档。
        let skipped = fixture::skipped_volume("棋魂 07", 184);
        for listing in [Listing::Notable, Listing::All] {
            let opened = pages(
                &skipped,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                120,
                listing,
                0,
            );
            assert_eq!(opened.table.rows.len(), 1);
            assert!(opened.table.rows[0].text.contains("一页都没有重做"));
            assert_eq!(opened.table.rows[0].tone, Tone::Muted);
            // 抬头只剩「跳过」那一个词：没有分布可挂列头，也没有定档页、没有页数可说。
            assert_eq!(opened.heading, " 跳过", "{}", opened.heading);
        }
    }

    /// **抬头钉住这一卷的档位分布与定档页，两格与卷表那两列同一个出处**（票面：抬头；
    /// 基准档那一格换成分布是 `two-pass-rework/02`）。
    #[test]
    fn the_heading_pins_the_tally_and_the_driver_of_this_volume() {
        let volume = fixture::a_page_of_every_kind("卷二");

        let heading = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            200,
            Listing::Notable,
            0,
        )
        .heading;

        let rows = render::volume(&volume, WhiteAlignLimit::default());
        assert!(heading.contains(&render::tally_column(&rows).expect("有档位分布")));
        assert!(heading.contains(&driver(&rows).expect("有定档页")));
        // 这一卷八页里六页有判定：基准档那五页加特例那一页，分布把两档各摆出来。
        assert!(
            heading.starts_with(" 档位分布 4bit 5 ⋅ 8bit 1 · 定档页 003.jpg"),
            "{heading}"
        );
        // 列着几页也在：切到全部页之后这一格换一种说法——屏上看得出切没切过去。
        assert!(heading.contains("要紧的页 6/8"), "{heading}");
        let all = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            200,
            Listing::All,
            0,
        )
        .heading;
        assert!(all.contains("全部 8 页（要紧的 6 页）"), "{all}");
    }

    /// **光标停在第几页上算得出来，越界就近收住**（与视口那条同一个规矩）。
    #[test]
    fn the_cursor_lands_on_the_page_it_points_at_and_never_falls_off_the_table() {
        let volume = fixture::a_page_of_every_kind("卷二");

        // 列头占第零行：第 0 页是表上第 1 行。
        assert_eq!(
            pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                200,
                Listing::All,
                0
            )
            .table
            .cursor,
            Some(1)
        );
        assert_eq!(
            pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                200,
                Listing::All,
                3
            )
            .table
            .cursor,
            Some(4)
        );
        // 越界不算错：八页的卷上第 99 页收到最后一页。
        assert_eq!(
            pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                200,
                Listing::All,
                99
            )
            .table
            .cursor,
            Some(8)
        );
        // 只列要紧的那一副上只有六页，同一个数落在别处——两副列的不是同一批页。
        assert_eq!(
            pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                200,
                Listing::Notable,
                99
            )
            .table
            .cursor,
            Some(6)
        );
    }

    /// **窄下来按那个固定次序砍列，记号、页名与判定仍在**（票面第一条：与卷表同一套砍列）。
    ///
    /// 砍列那个次序的用例在 [`crate::session::columns`]（纯函数，终端库外面）；
    /// 这一条问的是**摆出来之后**：判据那一串先让掉，而「哪一页判成哪一档」一直在。
    #[test]
    fn a_narrow_pane_drops_the_scores_first_and_keeps_the_verdict() {
        let volume = fixture::a_page_of_every_kind("卷二");

        let wide = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            200,
            Listing::All,
            0,
        );
        let narrow = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            44,
            Listing::All,
            0,
        );

        assert!(
            wide.table.rows[0].text.contains("判据"),
            "宽了也没有判据那一列"
        );
        assert!(
            !narrow.table.rows[0].text.contains("判据"),
            "窄了还留着判据"
        );
        assert!(narrow.table.rows[0].text.contains("判定"), "判定被砍掉了");
        assert!(narrow.table.rows[0].text.contains("页名"), "页名被砍掉了");
        // 砍无可砍时页名从中间省略，两头留着——不恐慌、不错位。
        let sliver = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            6,
            Listing::All,
            0,
        );
        assert_eq!(body(&sliver).len(), 8, "砍无可砍时行也还在");
    }

    /// **几何那五格各落进自己那一列**：裁边 · 缩放 · 跨页 · 彩页转灰 · 去处
    /// （`p4-parking-lot/10`，收停车场 Q162）。
    ///
    /// 喂的是一行手搓的[几何行](Row)与一行判定行——**这一条问的是「哪一格落进哪一列」**，
    /// 而不是整表摆出来什么样（那一条是
    /// [`the_five_cells_that_used_to_sit_outside_the_table_are_laid_out_in_it`]）。
    /// 手搓是为了[跨页那一格](Field::Cut)：`tonefit::Cut` 在库外造不出来，
    /// 而这一格上本层做的事只有「把 `Field::Cut` 摆进 [`PageColumn::Cut`]」。
    #[test]
    fn each_of_the_five_cells_lands_in_its_own_column() {
        let on_the_geometry_row = [
            (Field::Crop, PageColumn::Crop, "裁边 1441x2048 ⟶ 1400x2000"),
            (Field::Scaling, PageColumn::Scaling, "缩放比 1.219 ⋅ 未预缩"),
            (Field::Cut, PageColumn::Cut, "跨页右半"),
            (Field::Output, PageColumn::Output, "出/隔离/卷三/001.png"),
        ];
        let cell = |field: Field, text: &str| Cell {
            field,
            text: text.to_owned(),
        };
        let geometry = Row {
            kind: RowKind::PageGeometry,
            cells: on_the_geometry_row
                .iter()
                .map(|(field, _, text)| cell(*field, text))
                .collect(),
        };
        // 彩页转灰在**判定那一行**上，不在几何那一行上（见 `render::page_row`）。
        let verdict = Row {
            kind: RowKind::PageVerdict,
            cells: vec![cell(Field::ColorToGray, "彩页转灰")],
        };

        let mut entry = Entry::opened(&geometry);
        entry.closed(&verdict);

        for (_, column, text) in on_the_geometry_row {
            assert_eq!(entry.text(column), text, "{column:?} 那一格落错了列");
        }
        assert_eq!(entry.text(PageColumn::ColorToGray), "彩页转灰");
    }

    /// **五格摆进表里，失败页那一行因此说得出它的尺寸是卷内统一的**
    /// （`p4-parking-lot/10` 收停车场 Q162；那一句是 `p1-session/11` 的验收）。
    ///
    /// 与上一条是一对：那一条问「哪一格落进哪一列」，这一条问**整表摆出来**——
    /// 列头在、格在、措辞逐字来自 [`crate::render`]（这一层一个字都没重写）。
    #[test]
    fn the_five_cells_that_used_to_sit_outside_the_table_are_laid_out_in_it() {
        let volume = fixture::a_page_with_every_geometry_cell("卷三");
        let cell = |field: Field| {
            render::pages(&volume, Mode::Process)
                .into_iter()
                .find_map(|row| row.cell(field).map(str::to_owned))
                .unwrap_or_else(|| panic!("{field:?} 那一格不在这一卷的行上"))
        };
        let opened = pages(
            &volume,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            240,
            Listing::All,
            0,
        );

        let said = body(&opened).join("\n");
        for field in [
            Field::Crop,
            Field::Scaling,
            Field::ColorToGray,
            Field::Output,
        ] {
            assert!(said.contains(&cell(field)), "少了 {field:?}：{said}");
        }
        // 列头也在：那几格各占自己一列，不是塞进行尾那几句里。
        let head = &opened.table.rows[0].text;
        for column in [
            PageColumn::Crop,
            PageColumn::Scaling,
            PageColumn::ColorToGray,
            PageColumn::Output,
        ] {
            assert!(head.contains(column.head()), "{column:?} 没有列头：{head}");
        }
        // **失败页那一行说得出它的尺寸是卷内统一的**：那一句就在缩放那一格里。
        let failed = body(&opened)
            .into_iter()
            .find(|row| row.contains("017.jpg"))
            .expect("失败那一页在");
        assert!(failed.contains("卷内统一尺寸"), "{failed}");
    }

    /// **一整列都不在场时那一列连列头都不占**（[`columns::fit`] 的第零步）。
    ///
    /// 多数卷一页都没裁过、没切过、没有彩页转灰——三个空列头挤掉的是判据那一串，
    /// 而那正是这一副要答的那一问的证据。[`fixture::a_page_of_every_kind`] 那一卷
    /// 就是这一档：几何那三格一格都不在场。
    #[test]
    fn a_column_no_page_has_anything_in_does_not_even_take_its_head() {
        let bare = fixture::a_page_of_every_kind("卷二");
        let full = fixture::a_page_with_every_geometry_cell("卷三");

        let without = pages(
            &bare,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            240,
            Listing::All,
            0,
        )
        .table
        .rows[0]
            .text
            .clone();
        let with = pages(
            &full,
            panel(),
            Mode::Process,
            WhiteAlignLimit::default(),
            240,
            Listing::All,
            0,
        )
        .table
        .rows[0]
            .text
            .clone();

        for head in ["裁边", "彩页"] {
            assert!(
                !without.contains(head),
                "空着的 {head} 那一列还占着：{without}"
            );
            assert!(with.contains(head), "{head} 那一列不见了：{with}");
        }
        // 跨页那一列两卷都空着——两份夹具都没有切开的页（见
        // [`fixture::a_page_with_every_geometry_cell`]）。
        assert!(!with.contains("跨页"), "{with}");
        // 恒在的那几列空着也留着：判定那一格在失败页上不在场，而它是这一副要答的那件事。
        assert!(without.contains("判定"), "{without}");
        // 缩放那一列每一页都有：它一格不空，因此这一档上照旧在。
        assert!(without.contains("缩放"), "{without}");
    }

    /// **按 `a` 切列法，表的那几列一列不多一列不少**（[`laid_out`] 的《哪几列在场按整卷算》）。
    ///
    /// 「一整列都不在场就不占地方」那一步问的若是**这一副列出来的那几页**，
    /// 一个只管「列哪几页」的键就会换掉整张表的列——而彩页转灰与跨页都不在
    /// [要紧的页](crate::render::notable)那七种里，一卷里带着它们的页恰好都不要紧
    /// 时那一列会随 `a` 一开一合。
    ///
    /// 这一卷就是那一档：几何全齐的那一页判定被覆盖顶掉、因此**不要紧**
    /// （只列要紧的页那一副上只剩失败那一张）。
    #[test]
    fn pressing_a_changes_which_pages_are_listed_and_not_which_columns_there_are() {
        let mut volume = fixture::a_page_with_every_geometry_cell("卷三");
        // 覆盖顶掉判定：这一卷没有定档页，几何全齐的那一页因此一处都不要紧。
        volume.verdict = Some(tonefit::VolumeVerdict::Override(tonefit::Candidate::new(
            tonefit::BitDepth::Four,
            tonefit::Dither::Off,
        )));

        // 留下哪几列：列头那一行上出现了哪几个词。**比的是列，不是宽度**——
        // 每一列多宽照旧按真列出来的那几行算，切一副列法列宽跟着变是应当的。
        let kept = |listing| {
            let head = pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                240,
                listing,
                0,
            )
            .table
            .rows[0]
                .text
                .clone();
            PageColumn::ALL
                .iter()
                .filter(|column| head.contains(column.head()))
                .copied()
                .collect::<Vec<_>>()
        };

        assert_eq!(
            body(&pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                240,
                Listing::Notable,
                0
            ))
            .len(),
            1,
            "只列要紧的页那一副该只剩失败那一张"
        );
        assert_eq!(
            body(&pages(
                &volume,
                panel(),
                Mode::Process,
                WhiteAlignLimit::default(),
                240,
                Listing::All,
                0
            ))
            .len(),
            2
        );
        assert_eq!(
            kept(Listing::Notable),
            kept(Listing::All),
            "按 `a` 换掉了表的列"
        );
        // 那两列确实在——两副都在，而不是两副都没有。
        for column in [PageColumn::Crop, PageColumn::ColorToGray] {
            assert!(
                kept(Listing::Notable).contains(&column),
                "{column:?} 那一列不在"
            );
        }
    }
}
