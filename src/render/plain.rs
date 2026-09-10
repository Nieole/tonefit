//! **纯文本那一副**：把 [`super`] 出的[行](super::Row)与[格](super::Cell)摆成
//! 命令行印出去的那一段。
//!
//! 措辞不在这里（ADR 0016）。这里只有**摆法**：一行缩进几格、哪一格前面挂什么词、
//! 格与格之间拿什么隔开、一个数后面跟什么单位。同一批行摆成一张表是**另一副**，
//! 那一副是会话自己的事（目录表、卷表与逐页表）。
//!
//! **分界线画在「这几个字换一副排版还成不成立」上**：列头、前缀、单位与把格串起来的
//! 那几个连接词换一副就得重写，它们在这里；一句**解释**（「开工前整卷解到临时目录，
//! 跑完就收」这种）换一副仍是同一句话，它在 [`super`]，整句装在一格里
//! （[`super::Field::Sentence`]）。搬错边的代价是单向的：解释落到这里，
//! 表那一副要么抄一遍、要么把它丢了。
//!
//! # 目录那一级也在这一副里（`volume-discovery/08`）
//!
//! [`report`] 按**目录**分组（[`super::grouped`]），每一枝那几卷前面摆一行
//! [目录行](directory)：几卷 · 基准档分布 · 几卷进了隔离。
//! **分组与聚合都不在这里**——它们在 [`super`]，会话的目录表读的是同一份。
//!
//! 命令行这一副**三级一并摆出来**：它没有一个键可按，藏起来的那两级在屏上就再也
//! 没有第二个地方看得到了（停车场 Q171 记着这一笔）。会话那一副折得起来：
//! 默认只给目录那一级，按 `⏎` 才往下摊。
//!
//! # 谁在读这一副
//!
//! - **命令行**：跑完一次性把四段拼起来印出去（[`report`]，`crate::execute`）。
//! - **会话退出时**：`stdout` 上留下的那份报告照的是命令行那一路的原格式
//!   （`crate::session::run` 的 `Running::report`），走的也是 [`report`]。
//!   最后那一趟没做成时它后面还跟着一段 [`undone`]——那一句为什么没做成，
//!   与报告正文分得开（21 号票）。
//! - **会话的报告区**：眼下画的就是这一副（`crate::session::draw::report`）。
//!   它会换成表——换的是排版，措辞一个字都不会跟着动，那正是 ADR 0016 买到的东西。
//!
//! # 一行摆成什么样，只有这里说得出
//!
//! [`line`] 那个 `match` 就是「拼装的规矩」的全部：一种[行](RowKind)一条。
//! 缺一格当场恐慌（见 [`cell`]）——那是这一层拼错了，不是数据的事。
//!
//! **末尾那一小结也走它**：卷级失败那几卷不在报告正文里，[`super::failed_volume_tail`]
//! 把它们逐条摆下来，摆法照的是 [`line`] 里 [`RowKind::FailedVolume`] 那一条——
//! 那一句原因因此在会话的卷表与命令行的末尾小结之间只有一处出处（Q133）。
//!
//! # 末尾那几小结拼回一段的规矩住在这里（停车场 Q155）
//!
//! [`super::tail`] 交出来的是**一小结一行**——七小结分属三档语义，拼成一段就只上得了
//! 一种色。**拼回一段文字是排版**，因此在这一副（[`tail`]）：措辞那一层只有一种输出，
//! 而命令行印出去的那一段与拆之前**逐字节相同**。
//!
//! 折行不在这里，与 [`super`] 同一条：折到多宽由印它的那一头定（见 [`crate::wrap`]）。

use tonefit::{Mode, Report, VolumeReport, WhiteAlignLimit};

use super::{Field, Listed, Row, RowKind};

/// 整份报告：命令行跑完在最后一次性渲染出来的就是它。
///
/// 四段按顺序拼起来，中间不加任何东西——会话逐段画出来的与这里拼出来的逐字节相同。
pub fn report(report: &Report, mode: Mode) -> String {
    let mut text = super::header(report, mode);
    let listed = super::listed(report);
    for group in super::grouped(&listed) {
        text.push_str(&directory(&group, &listed));
        for at in &group.at {
            if let Some(Listed::Settled(volume)) = listed.get(*at) {
                text.push_str(&self::volume(volume, report.white_align_limit));
                text.push_str(&self::pages(volume, mode));
            }
        }
    }
    text.push_str(&self::tail(report));
    text
}

/// **这一趟没做成**时，退出会话在 stdout 上留下的那一份（21 号票，收停车场 Q66）。
///
/// 三段按次序接起来：`earlier`（先前那一份做成了的报告，一趟都没做成过就没有）、
/// `attempt`（这一趟攒下来的那一份，说得出已经做完的哪几卷）、以及这一趟为什么没做成。
///
/// **两条缝两种待遇，各有各的理由。**报告与报告之间**不加任何东西**——两份各自都以换行
/// 收尾，接下去仍是一段报告（与 [`report`] 那四段同一条）。那句话前面**空一行**：
/// 它不是报告的一部分，而是它为什么没算完，读的人要分得开。**两条缝都只有这一处说了算**
/// ——`stdout` 上那一份是纯文本这一副（ADR 0016 决定第 3 条），摆法不该散到会话那一层去。
///
/// 措辞在 [`super::undone`]。**那一句在这里过一遍 [`crate::wrap::printed`]**：
/// 它劝人换一条命令（`--dither fs`、`--fit height`），而记号里那个空格带着
/// [不许断的标注](tonefit::HARD_SPACE)——标注是给折行看的，不是印出去的东西，
/// 而这一路不走折行（`CONTEXT.md` 的**字形约定**：印出去之前换回普通空格）。
/// **只过这一段**：报告正文那两份仍与本票落地之前逐字节相同，那一半的账记在停车场 Q584。
///
/// **唯一的读者是会话**（`crate::session::run::Running::report`），而会话整个挂在 `tui`
/// 后面：关掉那个特性的**非测试**构建里它一个读者都没有——**那不是死代码，是那一趟的前提**
/// （规矩见 `crate::session` 的模块文档：逐处挂，不整块放开）。
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "只有会话读得到，而它整个在 tui 特性后面")
)]
pub fn undone(earlier: Option<&str>, attempt: &str, said: &str) -> String {
    let mut text = earlier.unwrap_or_default().to_owned();
    text.push_str(attempt);
    text.push('\n');
    text.push_str(&crate::wrap::printed(&super::undone(said)));
    text.push('\n');
    text
}

/// **末尾那几小结拼回一段文字**（[`super::tail`] 出的那几行）。
///
/// 措辞那一层此刻交出来的是**一小结一行**（停车场 Q155：七小结分属三档语义，
/// 拼成一段就只上得了一种色）。**拼回去的规矩住在这一副**，不在措辞那一层——
/// 那一层因此只有一种输出，而命令行印出去的这一段与拆之前**逐字节相同**：
/// 每一小结那一段本来就以换行收尾，一行一行接下去，中间不加任何东西
/// （与 [`text`] 同一条）。
///
/// 会话那一副读的是同一批行，只是逐小结上色（`crate::session::draw` 的报告区）——
/// 同样的行、同样的格，摆法两副。**这一副一个颜色都不加**（spec 的《Out of Scope》），
/// 哪一小结挂哪一档因此不在这一层，也不在措辞那一层。
pub fn tail(report: &Report) -> String {
    text(&super::tail(report))
}

/// 一枝的[目录那一行](super::directory)，摆成纯文本。
///
/// 它摆在这一枝那几卷**前面**：命令行印出来的那一份因此与会话读的是同一个层次
/// （目录 → 卷 → 页），只是命令行三级一并摆出来——那一路没有一个键可按，
/// 藏起来的那两级在屏上就再也没有第二个地方看得到了。
pub fn directory(group: &super::Group, listed: &[Listed<'_>]) -> String {
    line(&super::directory(group, listed))
}

/// 一个卷的卷级那几行，摆成纯文本（[`super::volume`] 出的行）。
///
/// `limit` 是这一趟的《纸白对齐上限》：卷级那一行要照它说话，
/// 而它是**这一趟**的事实，不在 [`VolumeReport`] 上（见 [`super::volume`]）。
pub fn volume(volume: &VolumeReport, limit: WhiteAlignLimit) -> String {
    text(&super::volume(volume, limit))
}

/// 一个卷的逐页那几行，摆成纯文本（[`super::pages`] 出的行）。
///
/// 跳过的卷一行都没有，出来的就是空串。
/// `mode` 定的是逐页那几行说不说纸白——只在 `--dry-run` 说（见 [`super::pages`]）。
pub fn pages(volume: &VolumeReport, mode: Mode) -> String {
    text(&super::pages(volume, mode))
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
pub(super) fn line(row: &Row) -> String {
    match row.kind {
        // **目录那一行摆在它那几卷前面**（`volume-discovery/08`）：一枝一行，
        // 顶格摆——它比卷那一行高一级，而卷那一行本来就顶格，缩进说不出这件事。
        // 分布与隔离那两格**在场才说**：一格在不在场本身就是一句话。
        RowKind::Directory => format!(
            "{}  {} 卷{}{}\n",
            cell(row, Field::Source),
            cell(row, Field::VolumeCount),
            row.cell(Field::Bases)
                .map_or_else(String::new, |spread| format!(" · {spread}")),
            row.cell(Field::Isolated).map_or_else(String::new, |count| {
                format!(" · {}", super::isolated_note(count))
            }),
        ),
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
        // **上限那一格恒在，三个数不恒在**（见 [`super::white_align_rows`]）：
        // 照做那一趟上限取 0 时一页都没量过，三个 0 摆出去是编的。
        // 「上限」是列头、「页」是单位，两样都在这一层；「级」与那句「没开」在格里，
        // 因为这一格摆到哪一副排版上都得自带它们。
        RowKind::WhiteAlign => format!(
            "  纸白对齐 上限 {}{}\n",
            cell(row, Field::WhiteAlignLimit),
            row.cell(Field::WhiteAligned).map_or_else(String::new, |_| {
                format!(
                    " · 对齐 {} 页 · 超限 {} 页 · 量不出纸白 {} 页",
                    cell(row, Field::WhiteAligned),
                    cell(row, Field::WhiteOverTheLimit),
                    cell(row, Field::WhiteNoPaperWhite),
                )
            }),
        ),
        // 纸白对齐底下那一句与几何门底下那几句同一个摆法：它说的是上一行那几个数。
        RowKind::WhiteAlignNote => format!("    {}\n", cell(row, Field::Sentence)),
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
        // 纸白那一格跟在行尾，**在场才说**：它只在 `--dry-run` 出（见 [`super::pages`]），
        // 摆法与部分救回那一格同一条——不占一列，跟着这一行走。
        RowKind::PageVerdict => format!(
            "    {}{}判定 {}（{}）  判据 {}{}\n",
            marked(row, Field::Salvage),
            marked(row, Field::ColorToGray),
            cell(row, Field::Candidate),
            cell(row, Field::Reason),
            cell(row, Field::Scores),
            row.cell(Field::PaperWhite)
                .map_or_else(String::new, |said| format!("  {said}")),
        ),
        RowKind::PageColor => format!(
            "    {}{}\n",
            marked(row, Field::Salvage),
            cell(row, Field::Sentence)
        ),
        RowKind::PageFailure => format!("    {}\n", cell(row, Field::Sentence)),
        // 卷级失败那一卷在报告**正文**里一行都没有：它连一份卷报告都没有
        // （见 [`super::failed_volume`]）。这一副摆的是它在**末尾那一小结**里的两行——
        // 路径一行、原因一行，形状照预扫那条拒绝办。[`super::failed_volume_tail`]
        // 走的就是这一条，报告里印出去的那两行因此与卷表读的是同一行。
        RowKind::FailedVolume => format!(
            "  {}\n    {}\n",
            cell(row, Field::Source),
            cell(row, Field::Sentence)
        ),
        // **末尾那七小结原样摆下去**：一小结那一段本来就是排好版的一整段
        // （抬头一行、逐条那几行各自带着自己的缩进，末尾一个换行），这一副一格都不再动它。
        // 七种摆法相同而**分成七种**，理由与卷级那三种判定同一条：会话那一副要照它们
        // 各自的语义上色（停车场 Q155），认字符串是认不出来的。
        RowKind::NonVolumeTail
        | RowKind::OverflowTail
        | RowKind::BackstopTail
        | RowKind::SalvageTail
        | RowKind::IsolationTail
        | RowKind::FailedVolumeTail
        | RowKind::UnreachableTail => cell(row, Field::Sentence).to_owned(),
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
