//! 屏上那一块：**卷表**——**展开一枝**之后那一副，一卷一行
//! （`CONTEXT.md` 的《会话》：卷表；`volume-discovery/08`）。
//!
//! ```text
//!  记号  卷名        页数  基准档   定档页   耗时
//!  ✓     棋魂 07      184  2bit+FS  087.png  1m12s
//!  !     哆啦 03      212  4bit+FS  011.png  2m03s  隔离
//!  -     名侦探 05    190  跳过              3s
//!  ✗     消失的那卷     -  没做成    卷根不在了
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
//! # 它列的是**一枝底下**那几卷
//!
//! 报告区默认那一副是[目录表](super::directories)——一个目录一行，一屏看得完；
//! 这一副是**展开一枝**之后摊出来的那几卷（`volume-discovery/08`）。
//! **哪几卷归哪一枝不在这里问**：分组只有 [`crate::render::grouped`] 一处出处，
//! 会话这一侧拿到的是算好的那几枝（[`Branch`]）。
//!
//! # 不重排
//!
//! 卷**按跑完的先后**添上去，出事的靠行首记号跳出来、不靠位置：重排会让刚跑完的那一卷
//! 在屏上跳位置，而「一卷跑完当场看得见」正是这一格存在的理由。
//! **分组一格不动这一条**：一枝底下那几卷仍按报告上的先后摆
//! （见 [`crate::render::grouped`]）。
//! 一处例外记在停车场 Q151：没做成的那几卷排在收摊了的那几卷**后面**——
//! `Report` 把两者分成两列存，先后无从复原。

use std::path::Path;

use tonefit::{VolumeFailure, VolumeReport};

use super::overview::{DECIDING, spell};
use super::paint::{Painted, Tone};
use crate::render::{self, Field, Row, RowKind};
use crate::session::columns::{self, Column, VolumeColumn, Widths};
use crate::session::live::{Branch, Live, Volume};

/// 一格不在场时那一列上写什么。
///
/// **只有页数用它**：它夹在卷名与档位中间，空着读起来像掉了一个数；
/// 而定档页与耗时排在末尾，空着就是「这一卷没有这件事」，不必再说一遍。
///
/// 取 `-` 而不是破折号 `—`：后者过不了 [`columns::width_is_stable`] 那一关
/// （停车场 Q154）。它夹在卷名与档位中间，右边还有三列。
const ABSENT: &str = "-";

/// 行首记号：**这一卷怎么样，一个字符说完。**
///
/// 四种一一对应 `CONTEXT.md` 的《失败》分出来的那几种处境，**不多不少**：
/// 隔离是卷交出来了、带着坏页；没做成是卷根本没交出来。
///
/// # 记号与语义色在这里绑成一对（停车场 Q153）
///
/// **「这一卷属于哪一种」只判一次**（[`Mark::of`]），判出来的那一个既定了行首那个字符
/// （[`glyph`](Self::glyph)），也定了这一行的[语义](Tone)（[`tone`](Self::tone)）。
/// 「颜色不是唯一载体」这条因此**不靠人记着**：一行有颜色就必有一个记号，
/// 两者出自同一个取值，添一种记号不配语义（或反过来）根本编不过去。
///
/// **接住颜色的是这个记号，不是档位那一列上的那几个字。** 那几个字（「跳过」「没做成」）
/// 出自 [`crate::render::base_column`]——它是**措辞**，命令行那一路读的也是它，
/// 而命令行不上色（spec 的《Out of Scope》）。它们说的恰好是同一件事，因此屏上一行
/// 常常有两个载体；但**靠得住的那一个是记号**：砍列砍到只剩两列时它与卷名仍在
/// （[`crate::session::columns`]），而档位那一列是砍得掉的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
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
    ///
    /// 四个字形逐个过得了 [`columns::width_is_stable`] 那一关：跳过那一个从短横
    /// `–`（U+2013）换成了 `-`（停车场 Q154），`✓`／`!`／`✗` 三个本来就过得了，一个字没换。
    pub(super) fn glyph(self) -> char {
        match self {
            Self::Done => '✓',
            Self::Isolated => '!',
            Self::Skipped => '-',
            Self::Failed => '✗',
        }
    }

    /// 这一种记号是哪一种[语义](Tone)——**这一行整行按它上色**。
    ///
    /// 四种一一对上（spec 的《语义色》那张表）：正常跑完的卷平常、隔离要注意、
    /// 跳过的卷不要紧、一整卷没做成是出事。
    ///
    /// **整行上色而不是只染那一个记号**：出事的卷要在几十行里跳出来，
    /// 而一个字符的红点在一屏灰字里找不着——「一眼看出重点」问的是行，不是格。
    pub(super) fn tone(self) -> Tone {
        match self {
            Self::Done => Tone::Plain,
            Self::Isolated => Tone::Caution,
            Self::Skipped => Tone::Muted,
            Self::Failed => Tone::Trouble,
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

    /// **一枝该挂哪一个**（[`super::directories`]）：它底下那几卷里**最重的那一种**。
    ///
    /// 次序就是四种的轻重（与 `CONTEXT.md` 的《语义色》逐条对得上）：
    /// 有一卷没做成就是出事，有一卷进隔离就是要注意，全跳过了才是不要紧，
    /// 其余平常。**一枝的记号因此不会比它底下最坏的那一卷轻**——
    /// 收成一行的代价只许落在明细上，不许落在「这一枝出没出事」上。
    pub(super) fn of_branch(live: &Live, branch: &Branch) -> Self {
        // **最重的那一档先问**：它只看 `failures` 那一列，一卷都不必开。
        if !branch.failures.is_empty() {
            return Self::Failed;
        }
        let marks = || {
            branch
                .volumes
                .iter()
                .filter_map(|at| live.volume(*at))
                .map(Self::of)
        };
        if marks().any(|mark| mark == Self::Isolated) {
            return Self::Isolated;
        }
        // `all` 对空的一列答 `true`，而一卷都收不住的那一枝不是「跳过」——先问一句有没有。
        if marks().next().is_some() && marks().all(|mark| mark == Self::Skipped) {
            return Self::Skipped;
        }
        Self::Done
    }
}

/// 被隔离那一卷行尾标的那个词。
///
/// 整句话（几页失败、去了哪儿、坏页在输出里什么样）在卷级那一行上，出自
/// [`crate::render`]；这里只留那个**跳得出来的词**——表上一行摆不下一整句，
/// 而不上色的终端上非有一个字不可（spec 的《语义色》）。
const ISOLATED: &str = "隔离";

/// 摆在一卷那一行**底下**的那几句缩进几格（见 [`under`]）。
///
/// 比行首那一格再深两格：那一格是全表共有的（表离开框线用的），而这两格说的是
/// 「这一句是上面那一行的」——缩进是屏上唯一说得出这件事的东西。
const UNDER_INDENT: &str = "   ";

/// 表上的一行：一卷。
///
/// 不在场的格是 `None`——**一格在不在场本身就是一句话**（`CONTEXT.md` 的《格》）：
/// 跳过的卷没有定档页，没做成的卷连页数都没有。
struct Entry {
    /// **表上的光标停得上去吗，停上去指的是哪一卷**（`p3-session-legibility/10`）。
    ///
    /// 没做成的那几卷是 `None`：它们连一份卷报告都没有，逐页那几行无从谈起
    /// （见 [`Volume`]）。它们照旧占一行——那一行要说的话就在它自己的行尾。
    at: Option<Volume>,
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
    ///
    /// 它们各有各的语义，**不跟这一行走**：一卷可以正常跑完（平常）而其中几页是部分救回的
    /// （要注意），两件事各说各的。
    under: Vec<Painted>,
}

impl Entry {
    /// 这一列上那一格。
    fn cell(&self, column: VolumeColumn) -> Option<&str> {
        match column {
            VolumeColumn::Mark => None,
            VolumeColumn::Name => Some(&self.name),
            VolumeColumn::Pages => self.pages.as_deref(),
            VolumeColumn::Base => self.base.as_deref(),
            VolumeColumn::Driver => self.driver.as_deref(),
            VolumeColumn::Elapsed => self.elapsed.as_deref(),
        }
    }

    /// 这一列上写什么：不在场的那几格里，只有页数留一个 [`ABSENT`]。
    ///
    /// 记号那一列自己答：它恒在，而它不是一格字，是一个[记号](Mark)。
    fn text(&self, column: VolumeColumn) -> String {
        if column == VolumeColumn::Mark {
            return self.mark.glyph().to_string();
        }
        self.cell(column).map_or_else(
            || {
                if column == VolumeColumn::Pages {
                    ABSENT.to_owned()
                } else {
                    String::new()
                }
            },
            str::to_owned,
        )
    }

    /// 一卷跑完（或跑到一半）的那一行。
    ///
    /// `at` 是**光标停上去指的是哪一卷**：收摊了的那几卷各是自己的下标，
    /// 决策点上那一份是 [`Volume::Summarized`]——它不在收摊了的那几卷里
    /// （`p2-loose-ends/08`：不许摊开上一卷冒充它）。
    fn of_volume(at: Volume, volume: &VolumeReport, waiting: bool) -> Self {
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
            at: Some(at),
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
            // 光标停不上去：没有报告，也就没有第二层可看。
            at: None,
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
///
/// **两种的语义不同**：部分救回在 spec 的《语义色》里列在「注意」那一档
/// （源文件不全，这一卷交出来的页不全是完整解出来的）；过期副本**四档里一档都不占**
/// ——盘上多躺着一份上一趟的输出既不是失败也不是要留神的判定，它只是一件要说出口的事。
/// 不在表上编一档给它：编出来的那一档在 [`Tone`] 上没有对应的语义（停车场 Q156）。
fn under(rows: &[Row]) -> Vec<Painted> {
    rows.iter()
        .filter_map(|row| match row.kind {
            RowKind::Superseded => Some((row, Tone::Plain)),
            RowKind::Salvaged => Some((row, Tone::Caution)),
            _ => None,
        })
        .filter_map(|(row, tone)| {
            row.cell(Field::Sentence)
                .map(|said| Painted::new(said.to_owned(), tone))
        })
        .collect()
}

/// 定档页那一列：[那一行](RowKind::Driver)上的路径，只印最后那一段。
///
/// 只印最后一段，与卷名同一条规矩（[`crate::render::volume_name`]）：
/// 一整条路径在这一列上摆不下，而定档页要答的是「是哪一页」。
///
/// **逐页那张表的抬头也读它**（[`super::pages`]）：展开一卷之后钉在顶上的那一行要说
/// 「这一卷的档是哪一页定的」，而那与这一列说的是同一件事，不许各取各的。
pub(super) fn driver(rows: &[Row]) -> Option<String> {
    rows.iter()
        .find(|row| row.kind == RowKind::Driver)
        .and_then(|row| row.cell(Field::Source))
        .map(|path| render::volume_name(Path::new(path)))
}

/// **这一枝底下**到此刻为止的那几卷，**按跑完的先后**（例外见模块文档《不重排》）。
///
/// `only` 是展开着的那一枝：收摊了的那几卷按 [`Volume`] 认，没做成的那几卷按它们
/// 在 `Report::failed_volumes` 里第几条认——两处都由[分组](crate::render::grouped)
/// 算好摆在 [`Branch`] 上，这里一个路径都不再切一遍。
fn entries(live: &Live, only: &Branch) -> Vec<Entry> {
    let report = live.report();
    let mut entries: Vec<Entry> = report
        .volumes
        .iter()
        .enumerate()
        .filter(|(at, _)| only.volumes.contains(&Volume::Settled(*at)))
        .map(|(at, volume)| Entry::of_volume(Volume::Settled(at), volume, false))
        .collect();
    entries.extend(
        report
            .failed_volumes
            .iter()
            .enumerate()
            .filter(|(at, _)| only.failures.contains(at))
            .map(|(_, failure)| Entry::of_failure(failure)),
    );
    // 决策点上那一卷**到此刻为止**的那一份（停车场 Q52）：它还没收摊，不在报告那一列里，
    // 而它同样占一行——「不许摊开上一卷冒充它」（`p2-loose-ends/08` 的硬约束）。
    // 光标停得上去、也展得开，指的是 [`Volume::Summarized`]（`p3-session-legibility/10`）。
    // 那个 `after` 是**它的身份**（见 [`Volume::Summarized`]）：光标停在它上面之后
    // 它收了摊，靠这个数才认得出「它此刻是收摊了的第几卷」。
    let after = report.volumes.len();
    entries.extend(
        live.summarized()
            .filter(|_| only.volumes.contains(&Volume::Summarized { after }))
            .map(|volume| Entry::of_volume(Volume::Summarized { after }, volume, true)),
    );
    entries
}

/// 卷表画出来的那几行，外加**光标停在第几行**。
///
/// 两样装在一个类型里而不是一对裸值：它们是同一次摆出来的，而调用处看不出
/// 「第二个 `usize` 是什么」（与 [`super::pages::Opened`] 同一条理由）。
pub(super) struct Table {
    /// 表上那几行：列头一行，此后一行一项（外加摆在一卷底下的那几句）。
    pub(super) rows: Vec<Painted>,
    /// 光标停在 [`rows`](Self::rows) 的第几行。**没有光标可画时是 `None`**——
    /// 一卷都没有、或者光标指着的那一卷此刻不在表上。
    pub(super) cursor: Option<usize>,
}

/// **卷表**：列头一行，此后一卷一行。一卷都还没有就一行都不出。
///
/// `room` 是这一格里正文摆得下几列。行首恒留一格空白——那一格既让表离开框线，
/// 也是行尾那句话折下来时的**悬挂缩进**（[`crate::wrap`]：缩进跟着折下来的每一行走）。
///
/// `at` 是**报告区那个光标停在哪一卷上**（`p3-session-legibility/10`）：出来的
/// [`Table::cursor`] 是它落在第几行。**指着的那一卷不在表上时不报错**——
/// 那时一行都不反白，与一卷都没有时一个待遇（光标越界不算错，与
/// `crate::session::viewport::Viewport` 同一条）。
///
/// 每一行带着它是哪一种[语义](Tone)（[`Painted`]），画它的那一头照那一种上色——
/// 一行的语义由行首那个[记号](Mark)说了算，本模块因此一个颜色名都不写
/// （见 [`Mark`] 的《记号与语义色在这里绑成一对》）。**反白不在这里上**：
/// 它不是语义色（见 [`super::paint`]），落哪一行由 [`Table::cursor`] 说了算。
pub(super) fn table(live: &Live, room: u16, at: Option<Volume>, only: &Branch) -> Table {
    let entries = entries(live, only);
    if entries.is_empty() {
        return Table {
            rows: Vec::new(),
            cursor: None,
        };
    }
    let room = usize::from(room).saturating_sub(1);
    let mut widths: Widths<VolumeColumn> = Widths::new();
    for entry in &entries {
        for column in VolumeColumn::ALL {
            widths.widen(*column, &entry.text(*column));
        }
    }
    // 砍列，砍无可砍再把卷名收窄（摆不下的那几个字从中间省略）——两步一处出处
    // （[`columns::plan`]），逐页那张表走的是同一处。
    let kept = columns::plan(room, &mut widths);
    let mut lines = vec![Painted::plain(columns::lay(
        &kept,
        &widths,
        |column| column.head().to_owned(),
        &[],
    ))];
    let mut cursor = None;
    for entry in &entries {
        // 光标停在这一卷上：记下它落在第几行。`at` 是 `None`（一卷都没选）时
        // 恒不相等——`Option::==` 那一头的 `None` 不与任何一卷相等。
        if entry.at.is_some() && entry.at == at {
            cursor = Some(lines.len());
        }
        lines.push(Painted::new(
            columns::lay(&kept, &widths, |column| entry.text(column), &entry.notes),
            entry.mark.tone(),
        ));
        // 摆在它底下、缩进一格的那几句（[`under`]）：整段文字，画它的那一头折行。
        lines.extend(entry.under.iter().map(|said| said.indented(UNDER_INDENT)));
    }
    Table {
        rows: lines,
        cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **卷表自己造的那几个字形在哪种终端上都占一格**（停车场 Q154）。
    ///
    /// 行首记号是这张表的头一列，右边还有五列：它宽一格，整行就跟着错一格。
    /// 判据在 [`columns::width_is_stable`]。这张表画成什么样在 [`super::report`] 那一头问，
    /// 本模块只钉这一条自己说了算的事。
    #[test]
    fn every_glyph_this_table_makes_is_the_same_width_on_any_terminal() {
        for mark in [Mark::Done, Mark::Isolated, Mark::Skipped, Mark::Failed] {
            let glyph = mark.glyph();
            assert!(
                columns::width_is_stable(glyph),
                "{mark:?} 那个记号 {glyph} 是东亚歧义宽度"
            );
        }
        for glyph in ABSENT.chars() {
            assert!(columns::width_is_stable(glyph), "{glyph} 是东亚歧义宽度");
        }
    }
}
