//! 屏上那一块：**详情栏**——配置视图右边那一栏，答设置栏上光标那一项
//! （`CONTEXT.md` 的《详情栏》《下钻》《画质判定参数》）。
//!
//! 有取值环的那几项列出环上每一格（第一格恒是「没说」那一格，生效那一格带 `✓`、光标那一格带 `❯`）；
//! **型号那一项列的是屏幕规格**，`l` 进去才是那一块底下的型号（**下钻**）；自由填的那几项
//! 列当前值与 `i` 修改；**画质判定参数**那一组把行内那一句整句摊开——那一句与报告抬头逐字相同。
//! 每一项底下还有一段**说明**，那是会话自己的字。
//!
//! 套着预设时末尾注一句「预设「…」中：…」；跑着与等待确认时再注一句设置暂时锁定。
//! 不到 90 列退成单栏时框底边左端写一句 `h → 返回`。

use ratatui::layout::Rect;

use super::super::config::{self, Choices, Item};
use super::super::keymap::{self, Deed};
use super::super::look::{Hue, Kind, Look, Segment};
use super::super::state::{Field, Session};
use super::super::tone::Tone;
use super::super::view::{Focus, Target};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, hint};
use crate::session::columns::elide;

/// 抬头右端那一截「当前 …」至多多宽（设计稿 `drawCfgRight` 的 `elide(…, 22)`）。
const CURRENT_WIDEST: usize = 22;

/// 正文折到比框里的宽度再窄两格（设计稿 `drawCfgRight` 的 `iw - 2`）。
const FOLD_MARGIN: u16 = 2;

/// 屏上一行：那几截字，以及它是**第几格取值**（停不住的行是 `None`）。
struct Row {
    segments: Vec<Segment>,
    choice: Option<usize>,
}

impl Row {
    /// 停不住的一行：抬头那一句、说明、末尾那几句注。
    fn plain(segments: Vec<Segment>) -> Self {
        Self {
            segments,
            choice: None,
        }
    }

    fn blank() -> Self {
        Self::plain(Vec::new())
    }
}

/// 画详情栏，占 `area`。`narrow` 是这一屏退成了单栏——那时框底边左端提一句怎么回去。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, area: Rect, narrow: bool) {
    let focused = session.views.focus() == Focus::Details;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    let item = session.views.config.cursor;
    let inner = area.width.saturating_sub(4);
    let rows = rows(session, item, inner.saturating_sub(FOLD_MARGIN), focused);
    let title = title(item);
    let right = current(session, item);
    let back = [Segment::new(
        format!(
            "{} → 返回",
            keymap::spelt_for(Deed::ConfigBack).unwrap_or_default()
        ),
        Look::FAINT.italic(),
    )];
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &title,
            right: &right,
            bottom_left: if narrow { &back } else { &[] },
            bottom_right: &[],
        },
    );
    let shown = area.height.saturating_sub(2);
    // 视口跟着**光标那一格**走；一格都停不住的那两种从头画起。
    let at = rows
        .iter()
        .position(|row| row.choice == Some(session.views.config.choice))
        .unwrap_or(0);
    let viewport = Viewport::with_margin(rows.len(), usize::from(shown), at);
    let from = viewport.from();
    for (i, row) in rows.iter().enumerate().skip(from).take(usize::from(shown)) {
        canvas.line(
            area.x + 2,
            area.y + 1 + (i - from) as u16,
            &row.segments,
            Some(inner),
        );
        // 停得住的那几格点得中（设计稿 `l.choice` 那一笔）。
        if let Some(choice) = row.choice {
            canvas.hit_row(area, area.y + 1 + (i - from) as u16, Target::Choice(choice));
        }
    }
}

/// 抬头：项名，画质判定参数那一组再跟一句它归哪一组。
fn title(item: Item) -> Vec<Segment> {
    let mut title = vec![Segment::new(item.label(), Look::PLAIN.bold())];
    if matches!(item, Item::Premise(_)) {
        title.push(Segment::faint(" ⋅ 画质判定参数"));
    }
    title
}

/// 抬头右端那一截：这一项此刻是什么。**画质判定参数那一组没有它**——那一组印的不是一个取值。
fn current(session: &Session, item: Item) -> Vec<Segment> {
    match item {
        Item::Premise(_) => Vec::new(),
        Item::Setting(_) => vec![
            Segment::faint("当前 "),
            Segment::plain(elide(&item.shown(session), CURRENT_WIDEST)),
        ],
    }
}

/// 这一栏此刻的每一行。
fn rows(session: &Session, item: Item, fold: u16, focused: bool) -> Vec<Row> {
    let locked = session.settings_locked();
    let mut rows = head(session, item, fold, focused, locked);
    rows.push(Row::blank());
    rows.extend(folded(item.about(session), fold, Look::of(Hue::Prose)));
    if matches!(item, Item::Premise(_)) {
        rows.push(Row::blank());
        rows.extend(folded(NOW_NOT_LAST_TIME, fold, Look::FAINT));
    }
    if let Item::Setting(field) = item
        && field != Field::Profile
        && let Some(applied) = &session.views.config.applied
    {
        rows.push(Row::blank());
        rows.push(Row::plain(preset_says(
            session,
            &applied.name,
            &applied.preset,
            field,
        )));
    }
    if locked {
        rows.push(Row::blank());
        rows.push(Row::plain(vec![Segment::new(
            "正在转换，设置暂时锁定，结束后才能修改。",
            Look::tone(Tone::Caution),
        )]));
    }
    rows
}

/// 这一栏上头那几行：取值环的每一格、屏幕规格的每一块、下钻之后的每一个型号、
/// 自由填那一项的当前值，或者画质判定参数那一整句。
fn head(session: &Session, item: Item, fold: u16, focused: bool, locked: bool) -> Vec<Row> {
    // 一格取值摆成一行：**这一格是不是光标那一格**由这一栏聚不聚焦与它排第几说了算，
    // 三种取值（环、屏幕规格、型号）因此不各问一遍。
    let cell = |at: usize, label: &str, chosen: bool, note: Option<String>| {
        let here = focused && session.views.config.choice == at;
        choice(at, label, chosen, note, here, locked)
    };
    match config::choices(session, item, session.views.config.drill) {
        Choices::Ring { cells, chosen } => cells
            .iter()
            .enumerate()
            .map(|(at, said)| {
                cell(
                    at,
                    said,
                    chosen == at,
                    (at == 0).then(|| UNSAID_NOTE.to_owned()),
                )
            })
            .collect(),
        Choices::Panels { cells, chosen } => {
            let mut rows = vec![Row::plain(vec![
                Segment::faint("先选屏幕规格，再选型号 ⋅ "),
                Segment::new("同一种屏幕的型号效果完全一样", Look::FAINT.dim()),
            ])];
            rows.extend(cells.iter().enumerate().map(|(at, (label, models))| {
                cell(
                    at,
                    label,
                    chosen == Some(at),
                    Some(format!("{models} 个型号")),
                )
            }));
            rows
        }
        Choices::Models {
            panel,
            cells,
            chosen,
        } => {
            let back = keymap::spelt_for(Deed::ConfigBack).unwrap_or_default();
            let mut rows = vec![Row::plain(vec![
                Segment::faint("屏幕规格 › "),
                Segment::new(panel.to_string(), Look::PLAIN.bold()),
                Segment::new(format!("   {back} → 回到屏幕规格"), Look::FAINT.dim()),
            ])];
            rows.extend(
                cells
                    .iter()
                    .enumerate()
                    .map(|(at, name)| cell(at, name, chosen == Some(at), None)),
            );
            rows
        }
        Choices::Filled(value) => {
            let mut rows = vec![Row::plain(vec![
                Segment::faint("当前  "),
                Segment::new(value, Look::PLAIN.bold()),
            ])];
            // 改不动的时候不提 `i`：按键表在只读那几档上根本不派它（屏上不摆按不动的键）。
            if !locked {
                let mut said = hint(
                    keymap::spelt_for(Deed::EditValue).unwrap_or_default(),
                    "修改",
                );
                said.push(Segment::new("   留空 = 使用默认值", Look::FAINT.dim()));
                rows.push(Row::plain(said));
            }
            rows
        }
        Choices::Premise => {
            let Item::Premise(which) = item else {
                return Vec::new();
            };
            let mut rows = folded(
                &config::premise_line(session, which),
                fold,
                Look::PLAIN.bold(),
            );
            // 一条互锁都没咬上时抬头一个字都不说：那一刻屏上那句「无」是界面自己的话。
            if config::quotes_the_header(session, which) {
                rows.push(Row::plain(vec![Segment::new(
                    "与报告抬头里的这一行逐字相同",
                    Look::FAINT.dim(),
                )]));
            }
            rows
        }
    }
}

/// 取值那一格的一行：光标那两格、生效那个 `✓`、这一格的写法，以及跟在后面的那一小截。
///
/// **`✓` 与 `❯` 分得开**：一个说「此刻生效的是它」，一个说「我在看的是它」——
/// 两者可以落在同一格上，也可以不落在同一格上，而那正是这一栏要摆出来的分别。
fn choice(
    at: usize,
    label: &str,
    chosen: bool,
    note: Option<String>,
    at_cursor: bool,
    locked: bool,
) -> Row {
    let mut value = if chosen {
        Look::PLAIN.bold()
    } else {
        Look::PLAIN
    };
    if locked {
        value = value.dim();
    }
    let mut segments = vec![
        Segment::new(
            if at_cursor { "❯ " } else { "  " },
            Look::kind(Kind::Focus).bold(),
        ),
        Segment::new(
            if chosen { "✓ " } else { "  " },
            Look::kind(Kind::Done).bold(),
        ),
        Segment::new(label, value),
    ];
    if let Some(note) = note {
        segments.push(Segment::new(format!("  {note}"), Look::FAINT.dim()));
    }
    Row {
        segments,
        choice: Some(at),
    }
}

/// 一段字折成几行，每一行一个样子。
fn folded(text: &str, width: u16, look: Look) -> Vec<Row> {
    crate::wrap::fold(text, width)
        .into_iter()
        .map(|line| Row::plain(vec![Segment::new(line, look)]))
        .collect()
}

/// 套着的那份预设里这一项怎么设，与此刻不同时注一句。
fn preset_says(
    session: &Session,
    name: &str,
    preset: &crate::preset::Preset,
    field: Field,
) -> Vec<Segment> {
    let said = config::preset_says(session, preset, field);
    let mut segments = vec![
        Segment::faint(format!("预设「{name}」中：")),
        Segment::plain(said.clone().unwrap_or_else(|| PRESET_UNSAID.to_owned())),
    ];
    if config::starred(session, field) {
        segments.push(Segment::new("   ← 与预设不同", Look::tone(Tone::Caution)));
    }
    segments
}

/// 取值环第一格后面那一小截：**「没说」与「说了一个恰好等于默认的值」的分别**
/// 只有存成预设时才看得见，屏上因此非说一句不可。
const UNSAID_NOTE: &str = "未设置：使用默认值，保存预设时不记录";

/// 套着的预设里这一项没设时那一句。
const PRESET_UNSAID: &str = "未设置（使用默认值）";

/// 画质判定参数那一组末尾那一句：**印的是此刻的设置**（`CONTEXT.md` 的那一条词条）。
const NOW_NOT_LAST_TIME: &str =
    "这里是此刻的设置，下一趟照它判定；上一趟用的是什么，看退出时印出的那份报告的抬头。";
