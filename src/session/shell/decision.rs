//! **确认条**：等待确认时钉在总览正下方的那一条黄色粗框
//! （`CONTEXT.md` 的《会话》：确认条、等待确认；spec《总览与确认条》）。
//!
//! ```text
//! ┏ 需要确认 ⋅ 这一卷分析完了，要写出吗？ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
//! ┃ ? 哆啦A梦/第05卷   灰阶分布 2bit+FS 187 ⋅ 4bit 32   需留意的页 1           ┃
//! ┃ x 写出这一卷（不用重新分析）     a 写出，后面的卷不再询问     s 不写出 …   ┃
//! ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 目前还没有写入任何文件 ━┛
//! ```
//!
//! **不弹窗、不盖住卷列表**：它占屏上自己那四行，底下那一块跟着矮四行——
//! 卷列表照样滚得动，`v` 进得去每页结果（[`super::pages`] 在这一档上照样画得出）。
//!
//! # 两行各说什么
//!
//! 头一行说**这一卷**：行首那个 `?` 与卷行上那个[记号](super::marks)同一个字，
//! 名字与总览「当前卷」那一格同一处（[`super::overview::current_name`]），
//! 灰阶分布与需留意几页出自这一卷**攒着的那一份报告**（`Live::summarized`）——
//! 与每页结果头一行报的是同一个数（[`Pages::notable_count`] 一处）。
//!
//! 第二行说**三种答法与 `v`**。这四句话是这一块自己写的（照设计稿 `drawDecision` 逐字），
//! 与按键表上那四行**撞着车**：`x`／`a`／`s` 三句与表上长的那一句逐字相同，
//! `v` 那一句短一截（表上是「查看这一卷的每页结果」）——停车场 **Q896**，
//! 与 Q874（屏上一块自己那几句提到键的话仍是手抄的）同一笔。
//!
//! # 摆不下时收成短句
//!
//! **「几列起收」那一档在 [`yielding`] 一处**（`CONTEXT.md` 的《让位》：确认条不到 110 列
//! 收成短句）——让位的次序与门槛都住在那里。收成什么样是这一块自己的事：卷名从中间省略到
//! 24 格、灰阶分布只报前两档、四句话各收短一截，三样照设计稿 `drawDecision` 那一支逐字。

use ratatui::layout::Rect;
use tonefit::VolumeReport;

use super::super::columns::elide;
use super::super::live::Live;
use super::super::look::{Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::view::Pages;
use super::canvas::{Border, Canvas};
use super::marks;
use super::overview::current_name;
use super::yielding;
use crate::render;

/// 这一条占几行：上下两条框线加两行正文（设计稿 `drawDecision` 的 `h = 4`）。
pub(super) const ROWS: u16 = 4;

/// 短句那一副卷名至多多宽，从中间省略（设计稿 `elide(volPath(v), 24)`）。
const NAME_WIDEST: usize = 24;

/// 灰阶分布报几档：宽那一副三档，短句那一副两档（设计稿 `tallySegs` 的两处调用）。
const TALLY_KEEPS: usize = 3;
const TALLY_KEEPS_SHORT: usize = 2;

/// 画确认条，从第 `y` 行起、占整宽；回它占了几行（与[总览](super::overview::draw)同一副）。
///
/// **答不出这一卷是哪一卷就一行都不占**（那一趟不在手上、攒着的那一份不在场）：
/// 摆一个空框出来比矮四行更坏——屏上会多一条什么都没说的黄框。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, live: Option<&Live>, y: u16) -> u16 {
    let Some(live) = live else {
        return 0;
    };
    // **这一卷攒着的那一份报告**：确认点上摆着的就是它（`Live::summarized`），
    // 每页结果那一屏读的也是它——两处报的灰阶分布与需留意几页因此是同一个数。
    let Some(report) = live.summarized() else {
        return 0;
    };
    // **让位那一问收的是整屏**（与 [`yielding::compact`]／[`single_column`](yielding::single_column)
    // 同一副）：这一条恒占整宽，而「窄不窄」问的是窗口，不是这一块摊到了几格。
    let screen = Rect::new(0, 0, canvas.width(), canvas.height());
    let short = yielding::short_decision(screen);
    let area = Rect::new(0, y, screen.width, ROWS);
    canvas.frame(
        area,
        &Border {
            thick: true,
            look: Look::tone(Tone::Caution),
            title: &[
                Segment::new("需要确认", Look::tone(Tone::Caution).bold()),
                Segment::plain(" ⋅ 这一卷分析完了，要写出吗？"),
            ],
            right: &[],
            bottom_left: &[],
            // 底边那一句答的是「按错了要紧吗」：这一刻盘上一个字节都还没有。
            bottom_right: &[Segment::faint("目前还没有写入任何文件")],
        },
    );
    let inner = area.width.saturating_sub(4);
    canvas.line(
        area.x + 2,
        area.y + 1,
        &this_volume(session, live, report, short),
        Some(inner),
    );
    canvas.line(area.x + 2, area.y + 2, &answers(short), Some(inner));
    ROWS
}

/// 头一行：这一卷 · 灰阶分布 · 需留意几页（宽那一副另报差异大的页）。
fn this_volume(session: &Session, live: &Live, report: &VolumeReport, short: bool) -> Vec<Segment> {
    let name = current_name(session, live);
    let notable = Pages::notable_count(report, live.report().profile.panel());
    // 有需留意的页就上注意色——**一个都没有的那一卷这一格是默认色**，
    // 「0」不该长得像一件要留神的事（设计稿 `drawDecision` 那一格同样分两档）。
    let count = Segment::new(
        notable.to_string(),
        if notable > 0 {
            Look::tone(Tone::Caution).bold()
        } else {
            Look::PLAIN
        },
    );
    let mark = Segment::new("? ", Look::tone(Tone::Caution).bold());
    let pairs = render::tally_pairs(report);
    if short {
        let mut line = vec![
            mark,
            Segment::new(elide(&name, NAME_WIDEST), Look::PLAIN.bold()),
            Segment::plain("  "),
        ];
        line.extend(marks::tally_segments(&pairs, TALLY_KEEPS_SHORT));
        line.push(Segment::faint("  需留意的页 "));
        line.push(count);
        return line;
    }
    let mut line = vec![
        mark,
        Segment::new(name, Look::PLAIN.bold()),
        Segment::faint("   灰阶分布 "),
    ];
    line.extend(marks::tally_segments(&pairs, TALLY_KEEPS));
    line.push(Segment::faint("   需留意的页 "));
    line.push(count);
    // **差异大的页另报一个数**（设计稿 `drawDecision` 那一截）：整卷统一灰阶那一趟
    // 才有这几页，而它同样停得到确认点上。数出自 [`Live::notable_at`] 一处——
    // 卷行行尾报的是同一份（`CONTEXT.md` 的《需留意的页》）。
    let outlier = session
        .views
        .task
        .tree
        .index_of(&report.volume)
        .map_or(0, |at| live.notable_at(at).outlier);
    if outlier > 0 {
        line.push(Segment::new(
            format!(" ⋅ 与其他页差异大 {outlier}"),
            Look::tone(Tone::Caution),
        ));
    }
    line
}

/// 第二行：三种答法与 `v`，一件一个键（模块文档《两行各说什么》：这四句是这一块自己写的）。
fn answers(short: bool) -> Vec<Segment> {
    let key = |glyph: &'static str| Segment::new(glyph, Look::tone(Tone::Caution).bold());
    if short {
        return vec![
            key("x"),
            Segment::plain(" 写出   "),
            key("a"),
            Segment::plain(" 写出，后面不再问   "),
            key("s"),
            Segment::plain(" 不写出，结束   "),
            key("v"),
            Segment::plain(" 查看每页结果"),
        ];
    }
    vec![
        key("x"),
        Segment::plain(" 写出这一卷"),
        Segment::faint("（不用重新分析）     "),
        key("a"),
        Segment::plain(" 写出"),
        Segment::faint("，后面的卷不再询问     "),
        key("s"),
        Segment::plain(" 不写出"),
        Segment::faint("，结束预览     "),
        key("v"),
        Segment::plain(" 查看每页结果"),
    ]
}
