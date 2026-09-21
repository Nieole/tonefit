//! **每页结果**：从卷行进到页那一副，换掉卷列表、占整宽，一个框
//! （`CONTEXT.md` 的《会话》：每页结果、需留意的页；spec 的《每页结果》）。
//!
//! ```text
//! ┏ 任务 › ~/漫画库 › 集英社/海贼王 › 第07卷 ━━━━━━━━━━━━━━━━━━━━━━ [需留意的页] ━┓
//! ┃ 灰阶分布 2bit+FS 130 ⋅ 4bit 50 ⋅ 2bit 8   需留意 1/189 页   这一卷输出在 _isol ┃
//! ┃    页面      尺寸       缩放                    灰阶     原因     画质分   提示 ┃
//! ┃❯ ✗ 017.jpg   1182x1680  读不出 ⋅ 用空白页占位                    失败 JPEG ⋯   ┃
//! ┗ a → 全部页 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 1 of 1 ━┛
//! ```
//!
//! **抬头是面包屑**（任务 › 分区的路径 › 目录 › 卷），右端说此刻的[列法](Listing)；
//! 头一行是这一卷的灰阶分布与需留意几页；此后一页一行。框底边左起是 `a` 那一件、
//! 右端说光标停在第几页、共几页。
//!
//! # 每一格的字出自报告那一处
//!
//! **表里的每一格**都逐字来自 [`render::pages`]（ADR 0016）：尺寸 · 缩放 · 灰阶 ·
//! 原因 · 画质分五格是那几行上的格，提示那一列是要紧在哪几处那几个词
//! （[`marks::notable_word`] 一处）加上成句的那几格（残缺救回了多少、纸白与钳制、
//! 坏页那一句原因）。**表里一个字都不重写**，颜色也不回头去认那几格的字
//! （灰阶那一列的档位取自 [`Entry::depth`]）。
//!
//! **表外那几句是这一块自己说的**，逐字照设计稿：列头（[`PagesColumn::head`]）、
//! [行首记号](Mark)、框底边那一件、跳过的卷那一句、一页都不需留意那一句，
//! 以及灰阶分布那一行末尾「等待确认」「输出在隔离目录」两个短标签。
//! 它们在命令行上**根本没有**（那一路把同一批格摆成一段散文），因此不是第二份说法；
//! 唯一与库那一头撞车的是跳过那一句——停车场 **Q877**。
//!
//! # 一行的列
//!
//! [`PagesWidths`] 定，**砍列的次序只在 [`super::super::columns`] 一处**
//! （缩放 → 画质分 → 尺寸；灰阶恒在）。列宽**定死**：屏上一页一行要对齐在同一处。
//!
//! # 一页都列不出来时给的是一句话，不是一张空表
//!
//! 两种，各说各的：**跳过的卷**进得来却一页结果都没有（它这一趟一页都没重新分析），
//! **一张需留意的页都没有**是一句好消息——空表说不出这两件事，它读起来像画坏了。

use ratatui::layout::Rect;
use tonefit::{BitDepth, Mode, Panel, VolumeReport};

use super::super::columns::{self, Column, PAGES_MARKS, PagesColumn, PagesWidths};
use super::super::live::{Live, VolumeState};
use super::super::look::{Kind, Look, Segment};
use super::super::state::{Listing, Session};
use super::super::tone::Tone;
use super::super::view::{Focus, Pages};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, padded};
use super::marks;
use crate::render::{self, Field, Notable, Row, RowKind};

/// 行首记号：**这一页要不要紧，一个字符说完**（`CONTEXT.md` 的《语义色》：
/// 颜色不是唯一载体）。
///
/// 三档：**不给不要紧的页一个记号**（一屏 `✓` 说不出哪一页要紧），**代表页也不给**——它不是出了事，
/// 屏上说它的是提示那一列里的「代表页」那个词。
///
/// **一页可以同时要紧在好几处，而一行只有一种语义**：取最重的那一个
/// （[`Ord`] 从轻到重排，与 [`Tone`] 同一个做法）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum Mark {
    /// 不挂记号的一页：不要紧的，或者只是这一卷的代表页。
    #[default]
    None,
    /// 要留神的一页：特例 · 残缺 · 尺寸未贴合屏幕 · 页面超宽 · 兜底上界，五者之一。
    Caution,
    /// 坏页：这一页根本没解出来。
    Failed,
}

impl Mark {
    /// 屏上那两格：记号加一个空格，不挂记号的两格空。
    fn glyph(self) -> &'static str {
        match self {
            Self::None => "  ",
            Self::Caution => "! ",
            Self::Failed => "✗ ",
        }
    }

    /// 那两格的样子：挂着记号的按它那一种[语义](Tone)加粗，不挂记号的两格是默认色
    /// ——**空白不上色**（那两格什么都没说）。
    fn look(self) -> Look {
        match self {
            Self::None => Look::PLAIN,
            Self::Caution | Self::Failed => Look::tone(self.tone()).bold(),
        }
    }

    /// 这一种记号是哪一种[语义](Tone)——**这一行的原因与提示两格按它上色**。
    fn tone(self) -> Tone {
        match self {
            Self::None => Tone::Plain,
            Self::Caution => Tone::Caution,
            Self::Failed => Tone::Trouble,
        }
    }
}

/// 一页**要紧在这一处**挂哪一个[记号](Mark)。
///
/// 逐条对上 `CONTEXT.md` 的《语义色》：坏页是「出事」，那五样是「注意」，
/// **代表页一档都不占**——它不是一件要留神的事，是这一卷的答案。
fn mark_of(what: Notable) -> Mark {
    match what {
        Notable::Failed => Mark::Failed,
        Notable::Salvaged
        | Notable::Outlier
        | Notable::OutsideTheGate
        | Notable::Overflowed
        | Notable::Backstopped => Mark::Caution,
        Notable::Driver => Mark::None,
    }
}

/// 一页要紧在这一处，**提示那一列上写哪个词**。词在 [`marks::notable_word`] 一处，
/// 这里只答「这一副写不写它」。
///
/// **这一副只让掉一种**：[残缺](Notable::Salvaged)跟着 `Field::Salvage` 那一格
/// （`救回 62.0%`），而它比一个词多说了**救回了多少**——救回 5% 与救回 95% 是两回事，
/// 一个「残缺」把那个数抹平了。多加一个词是同一件事说两遍。
///
/// **[坏页](Notable::Failed)不在这里让**：出处那一份上它本来就没有词
/// （行尾跟着那一句以「失败」开头的原因），照它答就是了——在这里再写一次 `None`
/// 是把同一件事说两遍。
fn word_of(what: Notable) -> Option<&'static str> {
    match what {
        Notable::Salvaged => None,
        Notable::Failed
        | Notable::Outlier
        | Notable::OutsideTheGate
        | Notable::Overflowed
        | Notable::Backstopped
        | Notable::Driver => marks::notable_word(what),
    }
}

/// 表上的一行：一页。
///
/// 不在场的格是空串——**一格在不在场本身就是一句话**（`CONTEXT.md` 的《格》）：
/// 坏页没有灰阶、没有原因、没有画质分。
struct Entry {
    mark: Mark,
    /// 页面：**源那一侧的成员名**，只印最后那一段（[`render::volume_name`]，
    /// 与面包屑末一截同一处出处）。
    name: String,
    size: String,
    scaling: String,
    verdict: String,
    reason: String,
    scores: String,
    /// 提示：要紧在哪几处那几个词，加上成句的那几格，中间用「 ⋅ 」隔开。
    notes: String,
    /// 这一页**页面超宽**吗：尺寸那一格按它上注意色（设计稿 `drawPages` 那一支）。
    overflowed: bool,
    /// 这一页判成的**档位**：灰阶那一列按它上种类色。
    ///
    /// **不是从[灰阶那一格的字](Self::verdict)上认回来的**——那一格是措辞那一层写下的一句，
    /// 而颜色要的是数（ADR 0016《后果》：表不许回头去认字符串）。出处是报告本身
    /// （`PageReport::verdict`）；彩色分支与坏页都没有，那时这一格退回次要那一灰。
    depth: Option<BitDepth>,
}

impl Entry {
    /// 这一列上写什么。记号那一列自己答：它不是一格字，是一个[记号](Mark)；
    /// 提示那一列不在这里——它不补空，吃剩下的宽度。
    fn text(&self, column: PagesColumn) -> &str {
        match column {
            PagesColumn::Mark => self.mark.glyph(),
            PagesColumn::Name => &self.name,
            PagesColumn::Size => &self.size,
            PagesColumn::Scaling => &self.scaling,
            PagesColumn::Verdict => &self.verdict,
            PagesColumn::Reason => &self.reason,
            PagesColumn::Scores => &self.scores,
            PagesColumn::Notes => &self.notes,
        }
    }
}

/// **一页那一行比框里别的几行靠左一格**：它从光标记号那一格起笔（设计稿
/// `scr.line(x + 1, …, iw + 1)`），而抬头那几行（灰阶分布、列头、一页都列不出来时那一句）
/// 从 `x + 2` 起笔。**那一格之差只有这一个数**：行摆得下几格、列头前面留几格空、
/// 行画在哪一列，三处都从它算。
const ROW_STARTS_EARLIER: u16 = 1;

/// 一格字，不在场就是空串。
fn cell(row: &Row, field: Field) -> String {
    row.cell(field).unwrap_or_default().to_owned()
}

/// 这一行上有这一格就收下，**不在场就不动**（`CONTEXT.md` 的《格》：
/// 一格在不在场本身就是一句话）。
fn take_cell(into: &mut String, row: &Row, field: Field) {
    if let Some(said) = row.cell(field) {
        *into = said.to_owned();
    }
}

/// 这一卷逐页各一行，**外加它要紧在哪几处**。
///
/// 行从 [`render::pages`] 来（一页两行：几何一行、判定一行），要紧在哪几处从
/// [`render::notable`] 来——两处都是逐页、同序，这里只把它们并到一行上。
/// **跳过的卷两处都是空的**：它一页都没重做。
fn entries(report: &VolumeReport, panel: Panel, mode: Mode) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    // 按「几何那一行起一页」分组，而不是按「每两行一页」切：后者把 `render::pages`
    // 的行数写死在这里，添一行就悄悄错位。
    for row in render::pages(report, mode) {
        if row.kind == RowKind::PageGeometry {
            entries.push(Entry {
                mark: Mark::default(),
                name: String::new(),
                size: cell(&row, Field::Size),
                scaling: cell(&row, Field::Scaling),
                verdict: String::new(),
                reason: String::new(),
                scores: String::new(),
                notes: String::new(),
                overflowed: false,
                depth: None,
            });
            continue;
        }
        let Some(entry) = entries.last_mut() else {
            continue;
        };
        // **一页的第二行不止一种**（判定 · 彩色分支 · 失败）：**在场的格才收**，
        // 不拿一行上没有的那一格把上一行收下的抹掉。
        take_cell(&mut entry.verdict, &row, Field::Candidate);
        take_cell(&mut entry.reason, &row, Field::Reason);
        // **画质分那一格是「判成那一档在这一页上的分」**（`2bit+FS 3.515`），
        // 不是六个候选那一整串（`Field::Scores`，那是命令行那一副，停车场 Q725）。
        take_cell(&mut entry.scores, &row, Field::VerdictScore);
        // 成句或成格、**不塞进列**的那几样跟在提示那一列上：救回了多少、
        // 这一页的纸白与钳制（只在预览那一副出）、成句的那一行（坏页、彩色分支）。
        for field in [Field::Salvage, Field::PaperWhite, Field::Sentence] {
            let Some(said) = row.cell(field) else {
                continue;
            };
            if !entry.notes.is_empty() {
                entry.notes.push_str(" ⋅ ");
            }
            entry.notes.push_str(said);
        }
    }
    // 名字与「要紧在哪几处」都按**页序**并上去：两者与 `report.pages` 等长同序。
    for ((entry, page), why) in entries
        .iter_mut()
        .zip(&report.pages)
        .zip(render::notable(report, panel))
    {
        entry.name = render::volume_name(&page.source);
        entry.overflowed = why.contains(&Notable::Overflowed);
        entry.depth = page.verdict().map(|verdict| verdict.candidate.bit_depth);
        let mut words: Vec<String> = Vec::new();
        for what in why {
            entry.mark = entry.mark.max(mark_of(what));
            words.extend(word_of(what).map(str::to_owned));
        }
        // 那几个词排在成句的那一句**前面**：句子长，而这几个词是扫一眼就要看见的东西。
        if !entry.notes.is_empty() {
            words.push(std::mem::take(&mut entry.notes));
        }
        entry.notes = words.join(" ⋅ ");
    }
    entries
}

/// 画每页结果，占 `area`。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, live: Option<&Live>, area: Rect) {
    let Some(pages) = &session.views.task.pages else {
        return;
    };
    let focused = session.views.focus() == Focus::Pages;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    // 框里正文那一截有多宽、摆得下几行（设计稿的 `iw`／`ih`）。
    let inner = area.width.saturating_sub(4);
    let shown = area.height.saturating_sub(4);
    let report = session.pages_report(live);
    let listed = session.listed_pages(live).unwrap_or_default();
    let at = pages.settled(listed.len());
    let viewport = Viewport::with_margin(listed.len(), usize::from(shown), at);
    let position = if listed.is_empty() {
        "0 of 0".to_owned()
    } else {
        format!("{} of {}", at + 1, listed.len())
    };
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &crumbs(session),
            right: &[listing_chip(pages.listing)],
            // **屏上这一块自己的开关**：它写的是**按下去会到的那一副**，
            // 而且不随阶段改口——等待确认那一档 `a` 让给答话、屏底不摆它，
            // 这一句照样写着（设计稿 `drawPages` 的 `bottomLeft`）。
            bottom_left: &[Segment::new(
                format!("a → {}", session.listing_key_says()),
                Look::FAINT.italic(),
            )],
            bottom_right: &[Segment::new(position, look)],
        },
    );
    if let Some(bar) = viewport.scrollbar() {
        canvas.scrollbar(area, &bar);
    }
    // 报告只有那一趟给得出（[`Session::pages_report`]），因此有报告就有那一趟。
    let (Some(report), Some(live)) = (report, live) else {
        return;
    };
    // **进得来却一页结果都没有的只有跳过的卷**（`CONTEXT.md` 的《停得住 / 展得开》）：
    // 它这一趟一页都没重新分析，连灰阶分布与列头都没有可写的。
    if report.skipped() {
        canvas.line(
            area.x + 2,
            area.y + 2,
            &[
                Segment::new("这一卷跳过了：", Look::FAINT.bold()),
                Segment::plain(
                    "之前转换过，源文件和设置都没变，这一趟没有重新分析，所以没有每页结果。",
                ),
                Segment::faint("  h → 回卷列表"),
            ],
            Some(inner),
        );
        return;
    }
    let panel = live.report().profile.panel();
    canvas.line(
        area.x + 2,
        area.y + 1,
        &tally_line(session, report, live, panel),
        Some(inner),
    );
    let widths = PagesWidths::of(inner);
    let kept = widths.kept();
    // 一行比抬头那几行[靠左一格](ROW_STARTS_EARLIER)，因此也宽一格。
    let row_width = inner + ROW_STARTS_EARLIER;
    canvas.line(area.x + 2, area.y + 2, &heads(&kept, &widths), Some(inner));
    if listed.is_empty() {
        canvas.line(
            area.x + 2,
            area.y + 3,
            &[
                Segment::plain("这一卷没有需留意的页 ⋅ "),
                Segment::faint("a → 全部页"),
            ],
            Some(inner),
        );
        return;
    }
    let all = entries(report, panel, live.mode());
    let from = viewport.from();
    for (row, index) in listed
        .iter()
        .enumerate()
        .skip(from)
        .take(usize::from(shown))
    {
        let Some(entry) = all.get(*index) else {
            continue;
        };
        canvas.line(
            area.x + 2 - ROW_STARTS_EARLIER,
            area.y + 3 + (row - from) as u16,
            &row_segments(entry, &kept, &widths, row == at, row_width),
            Some(row_width),
        );
    }
}

/// 抬头那条**面包屑**：任务 › 分区的路径 › 目录 › 卷。
///
/// **分区那一截只在这一卷归一条分区时在场**（顶格目录行本身就是顶层，
/// `CONTEXT.md` 的《分区》）。末一截是卷名（[`render::volume_name`]，归档卷去掉扩展名，
/// 停车场 Q849），与表上页名同一处出处。
fn crumbs(session: &Session) -> Vec<Segment> {
    let mut segments = vec![Segment::faint("任务")];
    let Some(pages) = &session.views.task.pages else {
        return segments;
    };
    let tree = &session.views.task.tree;
    let at = tree.index_of(&pages.volume);
    if let Some(path) = at.and_then(|at| tree.section_of(at)) {
        segments.push(Segment::faint(" › "));
        segments.push(Segment::faint(session.home_shown(path)));
    }
    if let Some(directory) = at.and_then(|at| tree.directory_of(at)) {
        segments.push(Segment::faint(" › "));
        segments.push(Segment::plain(directory.label.clone()));
    }
    segments.push(Segment::faint(" › "));
    segments.push(Segment::new(
        render::volume_name(&pages.volume),
        Look::PLAIN.bold(),
    ));
    segments
}

/// 抬头右端那一枚：此刻列的是哪几页。**需留意的页那一档上注意色**——
/// 屏上少列着东西是一件要知道的事。
fn listing_chip(listing: Listing) -> Segment {
    match listing {
        Listing::Notable => Segment::new("[需留意的页]", Look::tone(Tone::Caution).bold()),
        Listing::All => Segment::new("[全部页]", Look::PLAIN.bold()),
    }
}

/// 框里头一行：**这一卷的灰阶分布 · 需留意几页 · 这一卷此刻还有一句什么**。
///
/// 灰阶分布与卷行那一列同一处出处（[`render::tally_pairs`] 与
/// [`marks::tally_segments`]）：进了一卷之后卷列表不在屏上，而「这一卷各页写成了哪几档」
/// 正是逐页那几行要比的东西。
fn tally_line(session: &Session, report: &VolumeReport, live: &Live, panel: Panel) -> Vec<Segment> {
    let pages = report.page_count();
    let notable = Pages::notable_count(report, panel);
    let mut segments = vec![Segment::faint("灰阶分布 ")];
    segments.extend(marks::tally_segments(&render::tally_pairs(report), 3));
    segments.push(Segment::faint("   需留意 "));
    segments.push(Segment::new(
        format!("{notable}/{pages} 页"),
        Look::PLAIN.bold(),
    ));
    segments.push(Segment::plain("   "));
    // 末一句说的是**这一卷此刻还有一件什么事**：等待确认的那一份一个字节都没写，
    // 进了隔离的那一卷整卷去了隔离目录。都不是就不摆。
    let said = match session.volume_state(Some(live), &report.volume) {
        VolumeState::Deciding => "等待确认：还没写入任何文件",
        VolumeState::Isolated => "这一卷输出在 _isolated/",
        _ => "",
    };
    if !said.is_empty() {
        segments.push(Segment::new(said, Look::tone(Tone::Caution)));
    }
    segments
}

/// 列头那一行：行首那两截留空，此后每一列补空到它那么宽。
///
/// 前面留的是 [`PAGES_MARKS`] 减掉这一行[比一页那一行靠右的那一格](ROW_STARTS_EARLIER)
/// ——两行的页面那一列因此对齐在同一处。
fn heads(kept: &[PagesColumn], widths: &PagesWidths) -> Vec<Segment> {
    let blanks = usize::from(PAGES_MARKS - ROW_STARTS_EARLIER);
    let mut segments = vec![Segment::plain(" ".repeat(blanks))];
    for column in kept {
        if *column == PagesColumn::Mark {
            continue;
        }
        let text = match column {
            // 提示那一列吃剩下的，列头照它自己那么长就好。
            PagesColumn::Notes => column.head().to_owned(),
            _ => padded(column.head(), widths.of_column(*column)),
        };
        segments.push(Segment::new(text, Look::FAINT.bold()));
    }
    segments
}

/// 一页那一行：光标记号 · 行首记号 · 各列。整行的语义由行首那个[记号](Mark)说了算。
///
/// **提示那一列不补空**：它吃剩下的宽度，补空只会把一行的尾巴填满空格。
fn row_segments(
    entry: &Entry,
    kept: &[PagesColumn],
    widths: &PagesWidths,
    at_cursor: bool,
    row_width: u16,
) -> Vec<Segment> {
    let tone = entry.mark.tone();
    // **原因那一列只在坏页上变色**（设计稿 `drawPages` 的 `p.tone === 'bad' ? 'c-red' : 'c-fg'`）：
    // 那一格写的是**判定给的理由**，一句平常话；「这一页要留神」由行首记号与提示那一列说。
    // 两列一起上注意色的话，屏上就分不出这一格说的是哪件事——页面超宽那一页的原因仍是
    // 「达标的最省空间档位」，一句好消息。**坏页另当别论**：它那一格写的正是没解出来那一句本身。
    let reason = if entry.mark == Mark::Failed {
        tone
    } else {
        Tone::Plain
    };
    let mut segments = vec![
        Segment::new(
            if at_cursor { "❯ " } else { "  " },
            Look::kind(Kind::Focus).bold(),
        ),
        Segment::new(entry.mark.glyph(), entry.mark.look()),
    ];
    for column in kept {
        let width = widths.of_column(*column);
        let look = match column {
            // 页面那一列：**光标那一行加粗**（`CONTEXT.md` 的《语义色》：
            // 光标行加粗不归 `NO_COLOR` 管）。
            PagesColumn::Name if at_cursor => Look::PLAIN.bold(),
            PagesColumn::Name => Look::PLAIN,
            // 尺寸那一列：**页面超宽的那几页上注意色**——那一格本身就是那件事的载体。
            PagesColumn::Size if entry.overflowed => Look::tone(Tone::Caution),
            PagesColumn::Size | PagesColumn::Scaling | PagesColumn::Scores => Look::FAINT,
            PagesColumn::Verdict => depth_look(entry.depth),
            PagesColumn::Reason => Look::tone(reason),
            PagesColumn::Notes => Look::tone(tone),
            // 记号那两格上面已经摆过了。
            PagesColumn::Mark => continue,
        };
        let text = match column {
            // 页面那一列摆不下时**从中间省略**（它是[原样那一档](columns::Provenance)）。
            PagesColumn::Name => padded(&columns::elide(&entry.name, usize::from(width)), width),
            // **原因那一列省略到比列宽窄一格**（设计稿 `pad(elide(p.reason, C.reason - 1), …)`）：
            // 恰好占满整列的那一句（`没有档位达标，取最高档` 正是 22 格）会贴上右边那一列，
            // 两列的字挤在一起读起来像一句。省一格，中间那一道空白就保得住。
            PagesColumn::Reason => padded(
                &columns::elide(entry.text(*column), usize::from(width.saturating_sub(1))),
                width,
            ),
            PagesColumn::Notes => {
                columns::elide(&entry.notes, usize::from(widths.notes(row_width)))
            }
            _ => padded(entry.text(*column), width),
        };
        segments.push(Segment::new(text, look));
    }
    segments
}

/// 灰阶那一列的样子：**按档位上种类色**（1bit 品红、2bit 青、4bit 蓝）。
///
/// **`+FS` 那半截不另压暗**：这一列一格就是一个判定，整格一个颜色；
/// 压暗那一手在灰阶分布那一格上（[`marks::tally_segments`]），那里一行挤着好几档，
/// 分出主次才读得清。一格都没判的页（坏页、彩色分支）退回次要那一灰。
fn depth_look(depth: Option<BitDepth>) -> Look {
    depth.map_or(Look::FAINT, |depth| Look::kind(Kind::Depth(depth)))
}
