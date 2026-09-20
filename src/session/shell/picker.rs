//! 屏上那一块：**预设栏**——`p` 掀开、**替换详情栏**的那一栏（`CONTEXT.md` 的《预设栏》）。
//!
//! 列的是**进这一栏那一刻**盘上有的那几份：一份一行，行首是光标那两格与「正在用的是它」那个
//! `✓`，接着是名字、这一份说了哪几项，正在用的那一份行尾再写一句「使用中」。
//! 末行是那一件**把当前设置保存为预设**——它不是一份预设，`dd` 删不动它。
//!
//! 那几行底下空一行，再是一段**说明**（套用、保存、删除各是怎么回事）；有一问等着第二下时
//! 说明底下再空一行、写那一句「再按一次」——**那一问写在这一栏里，不写在屏底**
//! （设计稿 `drawPresets`：屏底那一行照旧摆着这一栏的四件）。**存那一下撞上同名**的那一问
//! 同样摆在这里：那一刻屏底让给了输入行（[`super::footer`]），说给屏底等于一个字都没说。
//!
//! 设置栏仍在屏上、框细着：**存出去的就是它上面那两组**。

use ratatui::layout::Rect;

use super::super::config;
use super::super::keymap::{self, Deed};
use super::super::look::{Hue, Kind, Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::super::view::{ConfigView, Focus, NamedPreset};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, padded};

/// 名字那一列有多宽（设计稿 `drawPresets` 的 `pad(p.name, 8)`）。
const NAME_COLUMN: u16 = 8;

/// 正文折到比框里的宽度再窄两格（设计稿 `drawPresets` 的 `iw - 2`，与详情栏同一档）。
const FOLD_MARGIN: u16 = 2;

/// 这一栏那一段说明：套用、保存、删除各是怎么回事。**屏上唯一一处说这三件的地方。**
const HOW_IT_WORKS: &str = "使用预设会替换全部设备和处理选项：预设里没设置的项恢复默认，\
     路径不受影响。保存预设时只记录设置过的项。删除或覆盖预设需要按两次确认。";

/// 末行那一件：把当前设置存成一份新的预设。
const SAVE_ROW: &str = "＋ 把当前设置保存为预设";

/// 一份都没说的那一份行内写什么。
const SAYS_NOTHING: &str = "没有设置任何项（全部默认）";

/// 读不懂的那一份行内写什么（与 [`crate::preset::Presets::names`] 那一条同一个道理：
/// 一份字段过时的预设不该让别的几份列不出来，但屏上得说得出它读不懂）。
const UNREADABLE: &str = "读不懂：字段过时或取值拼错";

/// 画预设栏，占 `area`。
///
/// **框底边什么都不写**：详情栏在单栏那一档上提的那句 `h → 返回` 这一栏没有
/// （设计稿 `drawPresets` 自己画的框没有 `bottomLeft`），而 `p → 返回` 已经写在右上角。
pub(super) fn draw(canvas: &mut Canvas<'_>, session: &Session, area: Rect) {
    let focused = session.views.focus() == Focus::Picker;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    let config = &session.views.config;
    let inner = area.width.saturating_sub(4);
    let back = keymap::spelt_for(Deed::Presets).unwrap_or_default();
    let title = [
        Segment::new("预设", Look::PLAIN.bold()),
        Segment::faint(format!(" ⋅ {} 个", config.listed.len())),
    ];
    let right = [Segment::faint(format!("{back} → 返回"))];
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &title,
            right: &right,
            bottom_left: &[],
            bottom_right: &[],
        },
    );
    let shown = area.height.saturating_sub(2);
    let lines = rows(session, inner.saturating_sub(FOLD_MARGIN), focused);
    // 视口跟着**光标那一行**走（`CONTEXT.md` 的《视口》：滚动量是算出来的，不是记着的）
    // ——那几份多过这一栏装得下的行数时，光标照旧在屏上。滚动条不画，与详情栏同一档。
    let viewport = Viewport::with_margin(lines.len(), usize::from(shown), config.preset_cursor);
    let from = usize::from(viewport.from());
    for (at, row) in lines.iter().enumerate().skip(from).take(usize::from(shown)) {
        canvas.line(
            area.x + 2,
            area.y + 1 + (at - from) as u16,
            row,
            Some(inner),
        );
    }
}

/// 这一栏此刻的每一行：那几份、末行那一件、空一行、那一段说明，加上等着第二下时那一句。
fn rows(session: &Session, fold: u16, focused: bool) -> Vec<Vec<Segment>> {
    let config = &session.views.config;
    let applied = config.applied.as_ref().map(|applied| applied.name.as_str());
    let mut rows: Vec<Vec<Segment>> = config
        .listed
        .iter()
        .enumerate()
        .map(|(at, listed)| {
            one(
                session,
                listed,
                applied == Some(listed.name.as_str()),
                config.preset_cursor == at,
                focused,
            )
        })
        .collect();
    rows.push(save_row(
        config.preset_cursor == config.listed.len(),
        focused,
    ));
    rows.push(Vec::new());
    rows.extend(
        crate::wrap::fold(HOW_IT_WORKS, fold)
            .into_iter()
            .map(|line| vec![Segment::new(line, Look::of(Hue::Prose))]),
    );
    if let Some(asking) = asked(config) {
        rows.push(Vec::new());
        rows.push(asking);
    }
    rows
}

/// **等着第二下的那一问**：删一份，或者存那一下撞上了一份同名的。
///
/// 两样同一刻只有一样在场（光标一挪 `dd` 那一格就作废，而起名那一行只在末行上开得起来），
/// 因此这一栏底下恒只多一行。**键怎么写从按键表取**（`CONTEXT.md` 的《屏底》：
/// 屏上顺口提到一个键的那几句不手抄）。
fn asked(config: &ConfigView) -> Option<Vec<Segment>> {
    // 删与覆盖**同一副骨架**：再按一次哪个键、对哪一份做哪件事，后半句说那件事的两半
    // ——「那一份的内容没了、撤不回来」与「文件里其余几份照旧留着」（`CONTEXT.md` 的《预设》）。
    let (deed, what, name) = match (&config.armed_delete, &config.armed_save) {
        (Some(name), _) => (Deed::DeletePreset, "删除", name),
        (None, Some(name)) => (Deed::Confirm, "覆盖", name),
        (None, None) => return None,
    };
    let key = keymap::spelt_for(deed).unwrap_or_default();
    Some(vec![
        Segment::new(
            format!("再按一次 {key} {what}「{name}」："),
            Look::tone(Tone::Caution).bold(),
        ),
        Segment::new(
            format!("{what}后无法恢复，其他预设不受影响"),
            Look::tone(Tone::Caution),
        ),
    ])
}

/// 一份预设那一行：光标那两格、「正在用的是它」那个 `✓`、名字、说了哪几项，
/// 正在用的那一份行尾再一句「使用中」。
///
/// **`✓` 与 `❯` 分得开**：一个说「此刻用的是它」，一个说「我在看的是它」——与详情栏那一栏同一条。
///
/// **行首那个 `❯` 与名字加不加粗问的不是同一件事**（设计稿 `drawPresets`）：
/// `❯` 问的是 `cur && focused`——这一栏不聚焦时它不露面；名字加粗问的只是 `cur`
/// ——打字时输入行盖在屏底上，这一栏里光标停在哪一行照旧看得出来。
fn one(
    session: &Session,
    listed: &NamedPreset,
    applied: bool,
    at_cursor: bool,
    focused: bool,
) -> Vec<Segment> {
    let name = if at_cursor {
        Look::PLAIN.bold()
    } else {
        Look::PLAIN
    };
    let mut segments = vec![
        cursor_cell(at_cursor, focused),
        Segment::new(
            if applied { "✓ " } else { "  " },
            Look::kind(Kind::Done).bold(),
        ),
        Segment::new(padded(&listed.name, NAME_COLUMN), name),
        Segment::faint(says(session, listed)),
    ];
    if applied {
        segments.push(Segment::new("   使用中", Look::kind(Kind::Done)));
    }
    segments
}

/// 这一份说了哪几项那一句：**项名取自设置栏那一份单子**（[`config::said_fields`]），
/// 这里不另列一份。一项都没说的那一份另说一句——「包含 0 项设置：」读不出意思。
fn says(session: &Session, listed: &NamedPreset) -> String {
    let Some(preset) = &listed.preset else {
        return UNREADABLE.to_owned();
    };
    let said = config::said_fields(session, preset);
    if said.is_empty() {
        return SAYS_NOTHING.to_owned();
    }
    let names: Vec<&str> = said.iter().map(|field| field.label()).collect();
    format!("包含 {} 项设置：{}", said.len(), names.join("、"))
}

/// 末行那一件：**把当前设置保存为预设**。
///
/// 中间那两格是空的、**不上色**——它不是一份预设，没有「正在用的是它」那个 `✓`
/// （设计稿 `drawPresets` 那一行的第二截样式为空）。
///
/// 行首那个 `❯` 与这一句加不加粗同样问的不是同一件事，见 [`one`]——按下 `⏎` 之后
/// 输入行占住屏底，这一行照旧加粗着（`config-p-save` 那一串钉的正是这一格）。
fn save_row(at_cursor: bool, focused: bool) -> Vec<Segment> {
    let what = if at_cursor {
        Look::kind(Kind::Done).bold()
    } else {
        Look::kind(Kind::Done)
    };
    vec![
        cursor_cell(at_cursor, focused),
        Segment::plain("  "),
        Segment::new(SAVE_ROW, what),
    ]
}

/// 行首那两格：光标停在这一行、**而且这一栏聚焦着**的时候才是 `❯`
/// （设计稿 `drawPresets` 的 `cur && focused`）。那两格恒是聚焦色——空着的时候也是它，
/// 屏上因此一格不挪。
fn cursor_cell(at_cursor: bool, focused: bool) -> Segment {
    Segment::new(
        if at_cursor && focused { "❯ " } else { "  " },
        Look::kind(Kind::Focus).bold(),
    )
}
