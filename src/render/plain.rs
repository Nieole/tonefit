//! **纯文本那一副**：把 [`super`] 出的[行](super::Row)与[格](super::Cell)摆成
//! 命令行印出去的那一段。
//!
//! 措辞不在这里（ADR 0016）。这里只有**摆法**：一行缩进几格、哪一格前面挂什么词、
//! 格与格之间拿什么隔开、一个数后面跟什么单位。同一批行摆成一张表是**另一副**，
//! 那一副是会话自己的事（P3 的卷表与逐页表）。
//!
//! **分界线画在「这几个字换一副排版还成不成立」上**：列头、前缀、单位与把格串起来的
//! 那几个连接词换一副就得重写，它们在这里；一句**解释**（「开工前整卷解到临时目录，
//! 跑完就收」这种）换一副仍是同一句话，它在 [`super`]，整句装在一格里
//! （[`super::Field::Sentence`]）。搬错边的代价是单向的：解释落到这里，
//! 表那一副要么抄一遍、要么把它丢了。
//!
//! # 谁在读这一副
//!
//! - **命令行**：跑完一次性把四段拼起来印出去（[`report`]，`crate::execute`）。
//! - **会话退出时**：`stdout` 上留下的那份报告照的是命令行那一路的原格式
//!   （`crate::session::run` 的 `Running::report`），走的也是 [`report`]。
//! - **会话的报告区**：眼下画的就是这一副（`crate::session::draw::report`）。
//!   它会换成表——换的是排版，措辞一个字都不会跟着动，那正是 ADR 0016 买到的东西。
//!
//! # 一行摆成什么样，只有这里说得出
//!
//! [`line`] 那个 `match` 就是「拼装的规矩」的全部：一种[行](RowKind)一条。
//! 缺一格当场恐慌（见 [`cell`]）——那是这一层拼错了，不是数据的事。
//!
//! 折行不在这里，与 [`super`] 同一条：折到多宽由印它的那一头定（见 [`crate::wrap`]）。

use tonefit::{Mode, Report, VolumeReport};

use super::{Field, Row, RowKind};

/// 整份报告：命令行跑完在最后一次性渲染出来的就是它。
///
/// 四段按顺序拼起来，中间不加任何东西——会话逐段画出来的与这里拼出来的逐字节相同。
pub fn report(report: &Report, mode: Mode) -> String {
    let mut text = super::header(report, mode);
    for volume in &report.volumes {
        text.push_str(&self::volume(volume));
        text.push_str(&self::pages(volume));
    }
    text.push_str(&super::tail(report));
    text
}

/// 一个卷的卷级那几行，摆成纯文本（[`super::volume`] 出的行）。
pub fn volume(volume: &VolumeReport) -> String {
    text(&super::volume(volume))
}

/// 一个卷的逐页那几行，摆成纯文本（[`super::pages`] 出的行）。
///
/// 跳过的卷一行都没有，出来的就是空串。
pub fn pages(volume: &VolumeReport) -> String {
    text(&super::pages(volume))
}

/// 一摞行摆成一段：一行一行接下去，中间不加任何东西。
fn text(rows: &[Row]) -> String {
    rows.iter().map(line).collect()
}

/// **一行摆成什么样**：缩进、前缀、分隔与单位全在这里。
///
/// 一种[行](RowKind)一条，不留 `_`：多一种行该怎么摆是个要当场拿的主意，
/// 而漏掉一种只会在屏上少一行、没人报错（同一条规矩见 [`super::outcome`]）。
///
/// 成句的那几行摆法都一样——缩进两格、把那句话原样放下去。它们**在这里也不拆**：
/// 拆开没有意义，而拆的那一刀会把措辞挪到这一层来。
fn line(row: &Row) -> String {
    match row.kind {
        RowKind::Volume => format!(
            "{} → {}（{} 页{}）\n",
            cell(row, Field::Source),
            cell(row, Field::Output),
            cell(row, Field::PageCount),
            row.cell(Field::ColorPages)
                .map_or_else(String::new, |count| format!("，其中彩页 {count} 页")),
        ),
        RowKind::Superseded
        | RowKind::Skipped
        | RowKind::Isolated
        | RowKind::Salvaged
        | RowKind::Extraction => format!("  {}\n", cell(row, Field::Sentence)),
        RowKind::Gate => format!(
            "  几何门 判定范围 灰度页 {} 页 · 不成立 {} 页 · 本卷 {}\n",
            cell(row, Field::GateScope),
            cell(row, Field::GateBroken),
            cell(row, Field::Dither),
        ),
        // 几何门底下那几句缩到第四格：它们说的是上一行那两个数，不是并列的另一件事。
        RowKind::GateNote => format!("    {}\n", cell(row, Field::Sentence)),
        RowKind::Envelope => format!("  卷级 {}\n", cell(row, Field::Envelope)),
        // 覆盖与逐页那两种同样挂在「卷级」后面：三种判定在纸上是同一行的三种说法。
        RowKind::Override | RowKind::PerPage => format!("  卷级 {}\n", cell(row, Field::Sentence)),
        RowKind::Driver => format!("    定档页 {}\n", cell(row, Field::Source)),
        RowKind::Reading => format!("  {}\n", cell(row, Field::Reading)),
        RowKind::Cache => format!("  缓存 {}\n", cell(row, Field::Cache)),
        // 「解出来多大 → 裁完多大 → 缩了多少 → 写出多大」一行读下来，
        // 中间那几格用 `·` 串起来，去处与它们之间空两格分开。
        RowKind::PageGeometry => format!(
            "  {}  {}{}{}{}  {}\n",
            cell(row, Field::Size),
            marked(row, Field::Crop),
            cell(row, Field::Scaling),
            row.cell(Field::Cut)
                .map_or_else(String::new, |cut| format!(" · {cut}")),
            row.cell(Field::Backstop)
                .map_or_else(String::new, |note| format!(" · {note}")),
            cell(row, Field::Output),
        ),
        RowKind::PageVerdict => format!(
            "    {}{}判定 {}（{}）  判据 {}\n",
            marked(row, Field::Salvage),
            marked(row, Field::ColorToGray),
            cell(row, Field::Candidate),
            cell(row, Field::Reason),
            cell(row, Field::Scores),
        ),
        RowKind::PageColor => format!(
            "    {}{}\n",
            marked(row, Field::Salvage),
            cell(row, Field::Sentence)
        ),
        RowKind::PageFailure => format!("    {}\n", cell(row, Field::Sentence)),
    }
}

/// 非在不可的那一格。
///
/// **不在场就恐慌**：一种行该有哪几格由 [`super`] 那一侧定死，这一层照着摆。
/// 少一格是这两处对不上了，而悄悄印出一行少一段的话没人会发现。
fn cell(row: &Row, field: Field) -> &str {
    row.cell(field)
        .unwrap_or_else(|| panic!("{:?} 那一行少了 {field:?} 那一格", row.kind))
}

/// 在场就带一个 `·` 接在后面的那种格：部分救回、彩页转灰都是这个样子。
///
/// 不在场就一个字都不占——那正是「一格在不在场本身就是一句话」（见 [`Row::cell`]）。
fn marked(row: &Row, field: Field) -> String {
    row.cell(field)
        .map_or_else(String::new, |text| format!("{text} · "))
}
