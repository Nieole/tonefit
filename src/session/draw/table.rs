//! 屏上那一块：**卷表**——报告区里一卷一行的那张表（`CONTEXT.md` 的《会话》：卷表）。
//!
//! ```text
//!  记号  卷名        页数  基准档   定档页   耗时
//!  ✓     棋魂 07      184  2bit+FS  087.png  1m12s
//!  !     哆啦 03      212  4bit+FS  011.png  2m03s  隔离
//!  –     名侦探 05    190  跳过              3s
//!  ✗     消失的那卷     —  没做成    卷根不在了
//! ```
//!
//! # 它吃的是同一批行
//!
//! 卷级那几行由 [`crate::render::volume`] 出（ADR 0016：一行带着它是什么行与若干格），
//! 命令行那一路把同一批行摆成一段散文（[`crate::render::plain`]）。
//! **这里是第二副排版，不是第二份数据**：措辞一个字都不在这里重写——
//! 档位那一列写什么问 [`crate::render::base_column`]，卷名与定档页走
//! [`crate::render::volume_name`]，耗时走 [`super::overview::spell`]，
//! 没做成那一卷的原因出自 [`crate::render::failed_volume`]。
//!
//! 长在这一层的只有**命令行上根本没有**的那两样：列头（[`Column::head`]）与
//! [行首记号](Mark)——那一路一个列都不分，也就不需要它们。
//!
//! # 表上没有列的那几句
//!
//! 成句的那几行**不塞进格**。表答得出的（跳过、逐页、覆盖、隔离）由档位那一列与
//! 记号／行尾那个词说；表与报告别处都答不出的那两种（过期副本、部分救回）
//! **摆在那一卷那一行底下**，整段折行——见 [`under`]。
//!
//! # 哪几列、砍哪几列不在这里
//!
//! 列的次序、砍列的次序、卷名怎么省略都在 [`crate::session::columns`]——**它在终端库
//! 外面**，`--no-default-features` 那一趟照跑它的用例。本模块只把那几个数摆成字。
//!
//! # 不重排
//!
//! 卷**按跑完的先后**添上去，出事的靠行首记号跳出来、不靠位置：重排会让刚跑完的那一卷
//! 在屏上跳位置，而「一卷跑完当场看得见」正是这一格存在的理由。
//! 一处例外记在停车场 Q151：没做成的那几卷排在收摊了的那几卷**后面**——
//! `Report` 把两者分成两列存，先后无从复原。

use std::path::Path;

use tonefit::{VolumeFailure, VolumeReport};

use super::overview::{DECIDING, spell};
use crate::render::{self, Field, Row, RowKind};
use crate::session::columns::{self, Column, Widths};
use crate::session::live::Live;
use crate::wrap;

/// 一格不在场时那一列上写什么。
///
/// **只有页数用它**：它夹在卷名与档位中间，空着读起来像掉了一个数；
/// 而定档页与耗时排在末尾，空着就是「这一卷没有这件事」，不必再说一遍。
const ABSENT: &str = "—";

/// 行首记号：**这一卷怎么样，一个字符说完。**
///
/// **颜色不是唯一载体**（spec 的《语义色》；上色本身归 `p3/09`，本模块一个颜色都不加）：
/// 这一格与档位那一列上的那几个字（「跳过」「没做成」）、行尾那一句（「隔离」）一起，
/// 在不上色的终端上、以及色盲眼里把话说全。
///
/// 四种一一对应 `CONTEXT.md` 的《失败》分出来的那几种处境，**不多不少**：
/// 隔离是卷交出来了、带着坏页；没做成是卷根本没交出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// 正常跑完。
    Done,
    /// 有失败页，整卷进了隔离目录。
    Isolated,
    /// 幂等命中，一页都没重做。
    Skipped,
    /// 一整卷没做成。
    Failed,
}

impl Mark {
    /// 屏上那一个字符。
    fn glyph(self) -> char {
        match self {
            Self::Done => '✓',
            Self::Isolated => '!',
            Self::Skipped => '–',
            Self::Failed => '✗',
        }
    }

    /// 这一卷该挂哪一个。
    fn of(volume: &VolumeReport) -> Self {
        if volume.skipped() {
            Self::Skipped
        } else if volume.failures().next().is_some() {
            Self::Isolated
        } else {
            Self::Done
        }
    }
}

/// 被隔离那一卷行尾标的那个词。
///
/// 整句话（几页失败、去了哪儿、坏页在输出里什么样）在卷级那一行上，出自
/// [`crate::render`]；这里只留那个**跳得出来的词**——表上一行摆不下一整句，
/// 而不上色的终端上非有一个字不可（spec 的《语义色》）。
const ISOLATED: &str = "隔离";

/// 表上的一行：一卷。
///
/// 不在场的格是 `None`——**一格在不在场本身就是一句话**（`CONTEXT.md` 的《格》）：
/// 跳过的卷没有定档页，没做成的卷连页数都没有。
struct Entry {
    /// 行首记号。
    mark: Mark,
    /// 卷名。走 [`crate::render::volume_name`]，与进度条印的是同一个。
    name: String,
    /// 输出页数。
    pages: Option<String>,
    /// 基准档，或者这一卷为什么没有一档。
    base: Option<String>,
    /// 定档页，只印最后那一段。
    driver: Option<String>,
    /// 这一卷做了多久。
    elapsed: Option<String>,
    /// 跟在行尾的那几句：**成句，不塞进格**（隔离、等你拿主意、没做成的那一句原因）。
    ///
    /// 它们本来就是句子，拆成格没有意义；摆不下时整行折下去，走 [`crate::wrap`]。
    notes: Vec<String>,
    /// 摆在这一行**底下**的那几句（见 [`under`]）：表上没有它们的列，别处也没有它们的位置。
    under: Vec<String>,
}

impl Entry {
    /// 这一列上那一格。
    fn cell(&self, column: Column) -> Option<&str> {
        match column {
            Column::Mark => None,
            Column::Name => Some(&self.name),
            Column::Pages => self.pages.as_deref(),
            Column::Base => self.base.as_deref(),
            Column::Driver => self.driver.as_deref(),
            Column::Elapsed => self.elapsed.as_deref(),
        }
    }

    /// 这一列上写什么：不在场的那几格里，只有页数留一个 [`ABSENT`]。
    ///
    /// 记号那一列自己答：它恒在，而它不是一格字，是一个[记号](Mark)。
    fn text(&self, column: Column) -> String {
        if column == Column::Mark {
            return self.mark.glyph().to_string();
        }
        self.cell(column).map_or_else(
            || {
                if column == Column::Pages {
                    ABSENT.to_owned()
                } else {
                    String::new()
                }
            },
            str::to_owned,
        )
    }

    /// 一卷跑完（或跑到一半）的那一行。
    fn of_volume(volume: &VolumeReport, waiting: bool) -> Self {
        let rows = render::volume(volume);
        let mark = Mark::of(volume);
        let mut notes = Vec::new();
        if mark == Mark::Isolated {
            notes.push(ISOLATED.to_owned());
        }
        if waiting {
            notes.push(DECIDING.to_owned());
        }
        Self {
            mark,
            name: render::volume_name(&volume.volume),
            // 页数走**卷那一行上那一格**，不回头去问 `VolumeReport`：这一副与命令行那一副
            // 读的是同一批格，另取一份就是第二个出处（`render::volume` 的文档记着，
            // 输出页数与源页数眼下相等、往后不一定）。
            pages: rows
                .iter()
                .find(|row| row.kind == RowKind::Volume)
                .and_then(|row| row.cell(Field::PageCount))
                .map(str::to_owned),
            base: render::base_column(&rows),
            driver: driver(&rows),
            elapsed: Some(spell(volume.timing.elapsed)),
            notes,
            under: under(&rows),
        }
    }

    /// 一整卷没做成的那一行（Q133）。
    ///
    /// 它连一份卷报告都没有——页数、定档页、耗时一样都答不出来，
    /// 而那正是「没做成」的意思。原因跟在行尾，成句。
    fn of_failure(failure: &VolumeFailure) -> Self {
        let row = render::failed_volume(failure);
        let rows = std::slice::from_ref(&row);
        Self {
            mark: Mark::Failed,
            name: render::volume_name(&failure.volume),
            pages: None,
            base: render::base_column(rows),
            driver: None,
            elapsed: None,
            notes: row
                .cell(Field::Sentence)
                .map(str::to_owned)
                .into_iter()
                .collect(),
            under: Vec::new(),
        }
    }
}

/// 摆在一卷那一行**底下**的那几句：表上没有它们的列，报告别处也没有它们的位置。
///
/// **两种，不多不少**：
///
/// - [过期副本](RowKind::Superseded)——盘上还躺着**上一趟**写下的那一份，
///   而这一趟没有覆盖它（当初被隔离过的话，那一份整卷都是白页）。
///   它不在末尾那几小结里，也不在总览块的出事行上：**少了这几行，屏上一个字都没有**。
/// - [部分救回](RowKind::Salvaged)——末尾那一小结说得出它，但那一段**收场之后才画**
///   （见 [`super::report`]），而跑着的那几十分钟里源文件不全这件事就没人说。
///
/// 别的成句的那几行表上都答得出：跳过、逐页、覆盖在档位那一列上
/// （[`crate::render::base_column`]），隔离在行首记号与行尾那个词上。
/// 几何门那几句注解不在这里——表**有意**不带几何，追下去要展开那一卷
/// （`p3-session-legibility/11` 的逐页表），而把它们摆回来就是把这一票要消灭的
/// 那四五行长句原样搬回屏上。
fn under(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|row| matches!(row.kind, RowKind::Superseded | RowKind::Salvaged))
        .filter_map(|row| row.cell(Field::Sentence))
        .map(str::to_owned)
        .collect()
}

/// 定档页那一列：[那一行](RowKind::Driver)上的路径，只印最后那一段。
///
/// 只印最后一段，与卷名同一条规矩（[`crate::render::volume_name`]）：
/// 一整条路径在这一列上摆不下，而定档页要答的是「是哪一页」。
fn driver(rows: &[Row]) -> Option<String> {
    rows.iter()
        .find(|row| row.kind == RowKind::Driver)
        .and_then(|row| row.cell(Field::Source))
        .map(|path| render::volume_name(Path::new(path)))
}

/// 这一趟到此刻为止的那几卷，**按跑完的先后**（例外见模块文档《不重排》）。
fn entries(live: &Live) -> Vec<Entry> {
    let report = live.report();
    let mut entries: Vec<Entry> = report
        .volumes
        .iter()
        .map(|volume| Entry::of_volume(volume, false))
        .collect();
    entries.extend(report.failed_volumes.iter().map(Entry::of_failure));
    // 决策点上那一卷**到此刻为止**的那一份（停车场 Q52）：它还没收摊，不在报告那一列里，
    // 而它同样占一行——「不许摊开上一卷冒充它」（`p2-loose-ends/08` 的硬约束）。
    entries.extend(
        live.summarized()
            .map(|volume| Entry::of_volume(volume, true)),
    );
    entries
}

/// **卷表**：列头一行，此后一卷一行。一卷都还没有就一行都不出。
///
/// `room` 是这一格里正文摆得下几列。行首恒留一格空白——那一格既让表离开框线，
/// 也是行尾那句话折下来时的**悬挂缩进**（[`crate::wrap`]：缩进跟着折下来的每一行走）。
pub(super) fn table(live: &Live, room: u16) -> Vec<String> {
    let entries = entries(live);
    if entries.is_empty() {
        return Vec::new();
    }
    let room = usize::from(room).saturating_sub(1);
    let mut widths = Widths::new();
    for entry in &entries {
        for column in Column::ALL {
            widths.widen(column, &entry.text(column));
        }
    }
    let kept = columns::fit(room, &widths);
    // 砍无可砍仍摆不下：卷名收窄，摆不下的那几个字从中间省略（[`columns::elide`]）。
    // 记号与卷名恒在，因此没有「一列都不剩」那一档。
    let over = columns::line_width(&kept, &widths).saturating_sub(room);
    if over > 0 {
        widths.narrow(
            Column::Name,
            widths.of(Column::Name).saturating_sub(over).max(1),
        );
    }
    let mut lines = vec![lay(&kept, &widths, |column| column.head().to_owned(), &[])];
    for entry in &entries {
        lines.push(lay(
            &kept,
            &widths,
            |column| entry.text(column),
            &entry.notes,
        ));
        // 摆在它底下、缩进一格的那几句（[`under`]）：整段文字，画它的那一头折行。
        lines.extend(entry.under.iter().map(|said| format!("   {said}")));
    }
    lines
}

/// 一行摆出来：留哪几列由 `kept` 说了算，每一列占几格由 `widths` 说了算。
///
/// 靠左还是靠右问 [`Column::to_the_right`]。行尾那几句**不占格**，也不参与对齐——
/// 它们是句子，摆不下时跟着整行折下去。
fn lay(
    kept: &[Column],
    widths: &Widths,
    mut cell: impl FnMut(Column) -> String,
    notes: &[String],
) -> String {
    let mut line = String::from(" ");
    for (at, column) in kept.iter().enumerate() {
        if at > 0 {
            line.push_str(&" ".repeat(columns::GAP));
        }
        let room = widths.of(*column);
        let text = columns::elide(&cell(*column), room);
        let pad = " ".repeat(room.saturating_sub(usize::from(wrap::width(&text))));
        if column.to_the_right() {
            line.push_str(&pad);
            line.push_str(&text);
        } else {
            line.push_str(&text);
            line.push_str(&pad);
        }
    }
    for note in notes {
        line.push_str(&" ".repeat(columns::GAP));
        line.push_str(note);
    }
    // 行尾那几格空白留着没有意义：折行那一头本来也要去掉它们（[`crate::wrap::fold`]）。
    line.trim_end().to_owned()
}
