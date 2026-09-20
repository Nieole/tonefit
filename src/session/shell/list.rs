//! **卷列表**：任务视图的主体，一张整宽的列表，一个框（`CONTEXT.md` 的《会话》：卷列表、
//! 停得住 / 展得开、焦点——聚焦框）。
//!
//! **形状随阶段换。** 清点完之前是**开跑之前**那一副：输出目录那一行 · 「处理路径 (N)」·
//! 一条条处理路径（勾选框 · 路径 · 文件夹还是压缩包；被另一条包含着的标一句，没勾的压暗）·
//! 末行「＋ 添加路径」。**这一副一次都不碰盘**：文件夹还是压缩包按扩展名认
//! （[`NamedPath::kind_of`]），被包含按路径前缀认（[`Session::nested_in`]）。
//! **清点中**同一副，只把勾选框换成**一行一格错开的转轮**，末行「＋ 添加路径」不在——
//! 跑着的时候加不进路径。
//!
//! **清点完之后**同一张列表重排成[那棵树](super::super::tree)：分区 · 目录行 · 卷行 ·
//! 备注行，末行说一句没勾的有几条。拼法摆在 `tui` 特性外面，这一层只管画。
//! 一行的列由 [`TreeWidths`] 定，**砍列的次序只在 [`super::super::columns`] 一处**。
//!
//! 焦点在这一块上时框换成粗线、上聚焦色，光标那一行行首是 `❯`；别处聚焦时细线、光标行首是暗的 `›`。
//! 框底边右端说光标停在第几条、共几条，右边框线上是滚动条；跑着时框的右端说**自动滚动**
//! 开着还是暂停了。

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use tonefit::{Candidate, FirstFew, VolumeReport};

use super::super::columns::{self, TreeColumn, TreeWidths};
use super::super::draw::overview::spell;
use super::super::draw::table::driver;
use super::super::keymap::{self, Deed, Phase, Want};
use super::super::live::{Live, NotableTally, VolumeState};
use super::super::look::{Kind, Look, Segment};
use super::super::state::{NamedPath, Session};
use super::super::tone::Tone;
use super::super::tree::{self, Directory, NoteKind, Shape};
use super::super::view::{Focus, Line};
use super::super::viewport::Viewport;
use super::canvas::{Border, Canvas, hint, padded};
use super::marks::{self, BranchTally, Mark};
use super::yielding;
use crate::render::{self, Field, Notable, RowKind};

/// 缩进一级占几格。
const A_LEVEL: u16 = 2;

/// 一行画在哪儿：内容那一截的**左端**、第几行、有多宽。
///
/// 三个数一起走：它们是一整套（框里那一截的几何），拆成三个参数传下去，
/// 每一层都得再拼一遍。
#[derive(Debug, Clone, Copy)]
struct Spot {
    x: u16,
    y: u16,
    inner: u16,
}

/// 画卷列表，占 `area`。
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
    area: Rect,
) {
    let focused = session.views.focus() == Focus::VolumeList;
    let look = if focused {
        Look::kind(Kind::Focus)
    } else {
        Look::FAINT
    };
    let (position, stops) = session.cursor_position();
    let lines = session.lines();
    let cursor = session.cursor_line();
    let shown = area.height.saturating_sub(2);
    let inner = area.width.saturating_sub(4);
    let viewport = Viewport::with_margin(lines.len(), usize::from(shown), cursor);
    canvas.frame(
        area,
        &Border {
            thick: focused,
            look,
            title: &title(session, phase),
            right: &follow_chip(session, phase),
            bottom_left: &searching_chip(session),
            bottom_right: &[Segment::new(format!("{position} of {stops}"), look)],
        },
    );
    if let Some(bar) = viewport.scrollbar() {
        canvas.scrollbar(area, &bar);
    }
    let painter = Painter {
        session,
        live,
        phase,
        now,
        widths: TreeWidths::of(inner, session.taste.envelope.unwrap_or(false)),
    };
    let from = usize::from(viewport.from());
    for (at, line) in lines.iter().enumerate().skip(from).take(usize::from(shown)) {
        let spot = Spot {
            x: area.x + 2,
            y: area.y + 1 + (at - from) as u16,
            inner,
        };
        painter.row(canvas, spot, line, at == cursor, focused);
    }
}

/// 框的抬头：开跑之前列的是路径，清点中说一句马上就来，清点完之后说共几卷。
fn title(session: &Session, phase: Phase) -> Vec<Segment> {
    let rest = match phase {
        Phase::Fresh => Segment::faint(" ⋅ 路径"),
        Phase::Surveying => Segment::faint(" ⋅ 正在清点，马上显示卷列表"),
        _ => Segment::faint(format!(" ⋅ {} 卷", session.views.task.tree.roots.len())),
    };
    vec![Segment::new("任务", Look::PLAIN.bold()), rest]
}

/// 框右端那一枚：**自动滚动**开着还是暂停了（`CONTEXT.md` 的《自动滚动》）。
/// 还没开跑、清点中与结束之后不摆它——那几档没有「正在处理的那一卷」可跟。
fn follow_chip(session: &Session, phase: Phase) -> Vec<Segment> {
    if !matches!(phase, Phase::Running | Phase::Deciding) {
        return Vec::new();
    }
    if session.views.task.follow {
        vec![Segment::new("[自动滚动]", Look::kind(Kind::Done).bold())]
    } else {
        vec![Segment::new(
            "[已暂停自动滚动 ⋅ F 恢复]",
            Look::tone(Tone::Caution),
        )]
    }
}

/// 框**底边左起**那一截：**此刻搜的那一句**连同 `n`／`N`（`CONTEXT.md` 的《卷列表》：
/// `n`／`N` 在结果之间跳）。
///
/// 一个字都还没打（刚按下 `/`）时**它自己就不在**——「空串不算在搜」判在
/// [`super::super::view::Views::searching`] 一处，匹配处那一道下划线读的是同一份。
fn searching_chip(session: &Session) -> Vec<Segment> {
    let Some(query) = session.views.searching() else {
        return Vec::new();
    };
    vec![
        Segment::new(format!("/{query}"), Look::tone(Tone::Caution).italic()),
        Segment::new(" ⋅ n N 跳到下一个 / 上一个", Look::FAINT.italic()),
    ]
}

/// 一行**此刻被怎么点出来**：行首那两格光标记号、它是不是光标那一行、
/// 它匹配着搜索那一句吗。
///
/// 三样一路走到名字那一列（[`name_look`]），拆成三个参数传下去每一层都得再拼一遍
/// ——与 [`Spot`] 同一条理由。
#[derive(Debug, Clone)]
struct Pointed {
    /// 行首那两格：`❯ `（这一块聚焦着）· `› `（焦点在别处）· 两格空。
    cursor: Segment,
    at_cursor: bool,
    matched: bool,
}

/// 名字那一列的样子：**光标那一行加粗，匹配着搜索那一句的加下划线**
/// （`CONTEXT.md` 的《语义色》：光标行加粗与搜索匹配加下划线两样都不归 `NO_COLOR` 管）。
///
/// 树上的目录行与卷行、备注行的名头、开跑之前那一副的每一行，**四处共用这一份**。
fn name_look(look: Look, pointed: &Pointed) -> Look {
    let look = if pointed.at_cursor { look.bold() } else { look };
    if pointed.matched {
        look.underlined()
    } else {
        look
    }
}

/// 画一行要的那几样：会话、那一趟、此刻，加上这一屏的列宽。
struct Painter<'a> {
    session: &'a Session,
    live: Option<&'a Live>,
    phase: Phase,
    now: Instant,
    widths: TreeWidths,
}

impl Painter<'_> {
    /// 画一行。`x` 是内容那一截的左端，`inner` 是它有多宽。
    fn row(
        &self,
        canvas: &mut Canvas<'_>,
        spot: Spot,
        line: &Line,
        at_cursor: bool,
        focused: bool,
    ) {
        let pointed = Pointed {
            cursor: match (at_cursor, focused) {
                (true, true) => Segment::new("❯ ", Look::kind(Kind::Focus).bold()),
                (true, false) => Segment::faint("› "),
                (false, _) => Segment::plain("  "),
            },
            at_cursor,
            // **匹配上没有一处算出来**：名字那一列的下划线（树上那三种行与开跑之前
            // 那一副）读的是同一份。
            matched: self.matched(line),
        };
        match line {
            Line::Tree(row) => self.tree_row(canvas, spot, *row, pointed),
            _ => {
                let segments = self.before_the_run(line, spot.inner, &pointed);
                canvas.line(spot.x, spot.y, &segments, Some(spot.inner));
            }
        }
    }

    /// 屏上这一行的转轮转到第几格；`offset` 是这一行自己的错相。
    fn spin(&self, offset: usize) -> &'static str {
        marks::spinner(self.now, self.session.opened_at, offset)
    }

    /// **这一行匹配着搜索那一句吗**——匹配的那一行**名字那一列加下划线**
    /// （`CONTEXT.md` 的《卷列表》：匹配处加下划线；《语义色》：搜索匹配加下划线，
    /// 不归 `NO_COLOR` 管）。
    ///
    /// 比的是哪一截字：清点之后那棵树上的行由树答（[`tree::Tree::searched_text`]），
    /// 开跑之前那一副比的是**屏上那条路径**（家目录已缩写成 `~`，与屏上写的一样）。
    /// 输出目录那一行与「＋ 添加路径」不参与——那两行上没有名字可搜。
    /// 没在搜（空串也算）时一行都不匹配，判在 [`Views::searching`] 一处。
    fn matched(&self, line: &Line) -> bool {
        let Some(query) = self.session.views.searching() else {
            return false;
        };
        let text = match line {
            Line::Tree(row) => self.session.views.task.tree.searched_text(*row),
            Line::Path(at) => self
                .session
                .scope
                .paths
                .get(*at)
                .map(|named| self.session.home_shown(&named.path)),
            Line::Output | Line::Heading | Line::Add => None,
        };
        text.is_some_and(|text| text.contains(query))
    }

    // ───────────────────────── 开跑之前与清点中 ─────────────────────────

    /// 开跑之前那一副的一行：输出目录 · 「处理路径 (N)」· 一条处理路径 · 「＋ 添加路径」。
    fn before_the_run(&self, line: &Line, inner: u16, pointed: &Pointed) -> Vec<Segment> {
        let session = self.session;
        // 行上顺口提的那两个键（`[i → 修改]`、`[o → 添加]`）连同那一句都从按键表取；派不出就不提。
        let mentioned = |want: Want| {
            keymap::hints(self.phase, session.views.block(), &[want])
                .first()
                .map(|said| hint(&said.spelt(), said.what))
                .unwrap_or_default()
        };
        let named = |look: Look| name_look(look, pointed);
        let cursor = || pointed.cursor.clone();
        match line {
            Line::Output => {
                let mut segments = vec![
                    cursor(),
                    Segment::faint("输出目录   "),
                    Segment::new(session.output_shown(), named(Look::PLAIN)),
                    Segment::plain("   "),
                ];
                segments.extend(mentioned(Want::of(Deed::EditPath)));
                segments
            }
            Line::Heading => vec![
                Segment::plain("  "),
                Segment::new("处理路径", Look::PLAIN.bold()),
                Segment::faint(format!(" ({})", session.scope.paths.len())),
                Segment::new("   ⋅ 开始前不会读取里面的内容", Look::tone(Tone::Muted)),
            ],
            Line::Add => {
                let mut segments = vec![
                    cursor(),
                    Segment::new("＋ 添加路径", named(Look::kind(Kind::Done))),
                    Segment::plain("   "),
                ];
                segments.extend(mentioned(Want::saying(Deed::AddPath, "添加")));
                segments
            }
            Line::Path(at) => self.path_row(*at, inner, cursor(), &named),
            Line::Tree(_) => Vec::new(),
        }
    }

    /// 一条处理路径那一行。**清点中**把勾选框换成转轮（一行一格错开），没勾的那一条空着——
    /// 它这一趟不处理，屏上不该让它看着像在动。
    fn path_row(
        &self,
        at: usize,
        inner: u16,
        cursor: Segment,
        named: &dyn Fn(Look) -> Look,
    ) -> Vec<Segment> {
        let session = self.session;
        let named_path: &NamedPath = &session.scope.paths[at];
        let box_of_it = if self.phase == Phase::Surveying {
            // 没勾的那一条空着：它这一趟不处理，屏上不该让它看着像在动。
            let glyph = if named_path.on {
                format!("{}   ", self.spin(at))
            } else {
                "    ".to_owned()
            };
            Segment::new(glyph, Look::kind(Kind::Surveying))
        } else if named_path.on {
            Segment::new("[x] ", Look::kind(Kind::Done))
        } else {
            Segment::faint("[ ] ")
        };
        let name_look = if named_path.on {
            Look::PLAIN
        } else {
            Look::tone(Tone::Muted)
        };
        let shown = session.home_shown(&named_path.path);
        // 路径那一列有多宽，整张表一个数：最长那条路径说了算。
        let longest = session
            .scope
            .paths
            .iter()
            .map(|one| crate::wrap::width(&session.home_shown(&one.path)))
            .max()
            .unwrap_or(0);
        let column = yielding::path_column(inner, longest);
        let mut segments = vec![
            cursor,
            box_of_it,
            Segment::new(
                padded(&columns::elide(&shown, usize::from(column)), column),
                named(name_look),
            ),
            Segment::faint(format!(" {}", NamedPath::kind_of(&named_path.path))),
        ];
        if named_path.on
            && let Some(outer) = session.nested_in(at)
        {
            segments.push(Segment::new(
                format!(" ⋅ 已包含在 {} 中，不会重复处理", session.home_shown(outer)),
                Look::tone(Tone::Caution),
            ));
        }
        if !named_path.on {
            segments.push(Segment::new(
                " ⋅ 未勾选，本次不处理",
                Look::tone(Tone::Muted),
            ));
        }
        segments
    }

    // ───────────────────────── 清点之后那棵树 ─────────────────────────

    /// 树上的一行：分区标题 · 目录行 · 卷行 · 备注行 · 空行 · 末行那一句。
    fn tree_row(&self, canvas: &mut Canvas<'_>, spot: Spot, row: tree::Row, pointed: Pointed) {
        let Spot { x, y, inner } = spot;
        let tree = &self.session.views.task.tree;
        match row {
            tree::Row::Gap => {}
            tree::Row::Foot => {
                // 「剩下的说个数」那一句只有一处出处（`crate::listing`）：这一行列的是**零条**
                // ——没勾的处理路径一条都不进树，末行只说个数。
                let off: Vec<&NamedPath> = self
                    .session
                    .scope
                    .paths
                    .iter()
                    .filter(|named| !named.on)
                    .collect();
                let Some(rest) = FirstFew::at_most(&off, 0).only_the_rest("个") else {
                    return;
                };
                let segments = vec![
                    Segment::plain("  "),
                    Segment::new(
                        format!("{rest}路径未勾选，本次不处理"),
                        Look::tone(Tone::Muted),
                    ),
                ];
                canvas.line(x, y, &segments, Some(inner));
            }
            tree::Row::Section { node } => self.section_row(canvas, spot, node),
            tree::Row::Note { node, at, indent } => {
                self.note_row(canvas, spot, (node, at, indent), &pointed);
            }
            tree::Row::Directory { node, at, indent } => {
                let Some(directory) = tree.directory(node, at) else {
                    return;
                };
                let tally = self.branch(directory);
                let expanded = self.session.views.task.expanded.contains(&directory.path);
                let running = tally.running.or(tally.deciding).is_some();
                self.lined_row(
                    canvas,
                    spot,
                    LinedRow {
                        pointed,
                        indent,
                        chevron: if expanded { "▾ " } else { "▸ " },
                        mark: marks::branch_mark(&tally, self.spin(0)),
                        name: directory.label.clone(),
                        name_look: if running {
                            Look::kind(Kind::Working)
                        } else {
                            Look::PLAIN
                        },
                        count: format!("{}/{} 卷", tally.finished, tally.total),
                        count_look: if tally.total > 0 && tally.finished == tally.total {
                            Look::PLAIN
                        } else {
                            Look::FAINT
                        },
                        tally: tally.tally.clone(),
                        why_nothing: Vec::new(),
                        driver: None,
                        elapsed: tally.elapsed,
                        tail: self.directory_tail(&tally),
                    },
                );
            }
            tree::Row::Volume { at, indent } => {
                self.volume_row(canvas, spot, (at, indent), pointed);
            }
        }
    }

    /// **分区标题**：一条横线，带汇总（行首记号 · 路径 · 做完几卷／共几卷 · 问题计数）。
    fn section_row(&self, canvas: &mut Canvas<'_>, spot: Spot, node: usize) {
        let Spot { x, y, inner } = spot;
        let tree = &self.session.views.task.tree;
        let Some(one) = tree.nodes.get(node) else {
            return;
        };
        let Shape::Section { path, directories } = &one.shape else {
            return;
        };
        let mut tally = BranchTally::default();
        for directory in directories {
            tally.absorb(&self.branch(directory));
        }
        let unreachable = one
            .notes
            .iter()
            .filter(|note| note.kind == NoteKind::Unreachable)
            .count();
        tally.bad += unreachable;
        let mut segments = vec![
            Segment::faint("── "),
            marks::branch_mark(&tally, self.spin(0)).segment(),
            Segment::new(self.session.home_shown(path), Look::PLAIN.bold()),
            Segment::faint(format!(" ⋅ {}/{} 卷", tally.finished, tally.total)),
        ];
        let problems = problem_parts(&tally, unreachable);
        if !problems.is_empty() {
            segments.push(Segment::faint(" ⋅ "));
            segments.extend(problems);
        }
        segments.push(Segment::plain(" "));
        let used = canvas.line(x, y, &segments, Some(inner));
        if used < inner {
            canvas.put(
                x + used,
                y,
                &"─".repeat(usize::from(inner - used)),
                Look::FAINT,
            );
        }
    }

    /// **备注行**：名头 · 是哪几处 · 行尾那一句（`CONTEXT.md` 的《备注行》）。
    fn note_row(
        &self,
        canvas: &mut Canvas<'_>,
        spot: Spot,
        (node, at, indent): (usize, usize, u16),
        pointed: &Pointed,
    ) {
        let Some(note) = self.session.views.task.tree.note(node, at) else {
            return;
        };
        let bad = note.kind == NoteKind::Unreachable;
        let (glyph, look) = if bad {
            ("✗ ", Look::tone(Tone::Trouble).bold())
        } else {
            ("- ", Look::FAINT)
        };
        let label_look = if bad {
            Look::tone(Tone::Trouble)
        } else {
            Look::PLAIN
        };
        let segments = vec![
            pointed.cursor.clone(),
            Segment::plain(" ".repeat(usize::from(indent * A_LEVEL))),
            Segment::plain("  "),
            Segment::new(glyph, look),
            Segment::new(format!("{}  ", note.label), name_look(label_look, pointed)),
            Segment::plain(note.what.clone()),
            Segment::new(format!("  {}", note.brief), Look::tone(Tone::Muted)),
        ];
        canvas.line(spot.x, spot.y, &segments, Some(spot.inner));
    }

    /// 一枝底下那几卷合起来怎么样。
    fn branch(&self, directory: &Directory) -> BranchTally {
        let mut tally = BranchTally {
            total: directory.volumes.len(),
            ..BranchTally::default()
        };
        let Some(live) = self.live else {
            return tally;
        };
        for at in &directory.volumes {
            let Some(state) = live.states().get(*at).copied() else {
                continue;
            };
            tally.elapsed += self.elapsed_of(*at);
            if state.settled() {
                tally.finished += 1;
            }
            match state {
                VolumeState::Running { .. } => tally.running = tally.running.or(Some(*at)),
                VolumeState::Deciding => tally.deciding = tally.deciding.or(Some(*at)),
                VolumeState::Skipped => tally.skipped += 1,
                VolumeState::Failed => {
                    tally.bad += 1;
                    tally.failed_volumes += 1;
                }
                VolumeState::Isolated => {
                    tally.warn += 1;
                    tally.isolated += 1;
                }
                VolumeState::Done | VolumeState::Queued | VolumeState::Aborted => {}
            }
            if state == VolumeState::Aborted {
                continue;
            }
            let Some(report) = self.report_of(*at) else {
                continue;
            };
            for (candidate, pages) in render::tally_pairs(report) {
                marks::add_to(&mut tally.tally, candidate, pages);
            }
            tally.failed_pages += report.failures().count();
            if state != VolumeState::Isolated && self.notable(*at) > 0 {
                tally.warn += 1;
            }
        }
        tally
    }

    /// 清单里第几卷那一份报告。**出处在那一趟上**（[`Live::report_at`]）：
    /// 跳转的落点读的是同一份。
    fn report_of(&self, at: usize) -> Option<&VolumeReport> {
        self.live?.report_at(at)
    }

    /// 这一卷做了多久：**收摊了的那几卷走报告那一份**（`CONTEXT.md` 的《卷级计时》：
    /// 只扣掉在确认点上等人的那一截），没有报告的那几卷（没做成、还在跑、被立即停止掉）
    /// 走会话这一头自己记的那一份（[`Live::elapsed_at`]）。
    fn elapsed_of(&self, at: usize) -> Duration {
        let Some(live) = self.live else {
            return Duration::ZERO;
        };
        self.report_of(at).map_or_else(
            || live.elapsed_at(at).unwrap_or_default(),
            |report| report.timing.elapsed,
        )
    }

    /// 这一卷有几页**需留意**（`CONTEXT.md` 的《需留意的页》）。
    fn notable(&self, at: usize) -> usize {
        self.notable_tally(at).pages()
    }

    /// 这一卷需留意的页按种类各几页。**数出自那一趟一处**（[`Live::notable_at`]）：
    /// 行首那个 `!` 与 `]d` 的落点读的是同一份。
    fn notable_tally(&self, at: usize) -> NotableTally {
        self.live
            .map(|live| live.notable_at(at))
            .unwrap_or_default()
    }

    /// 需留意那几样**在屏上各怎么写**，照屏上的次序（`CONTEXT.md` 的《语义色》在页
    /// 那一级分出的那几样）。数由那一趟折出来，**词在 [`marks::notable_word`] 一处**
    /// ——每页结果提示那一列读的是同一份。
    fn notable_bits(&self, at: usize) -> Vec<(&'static str, usize)> {
        let tally = self.notable_tally(at);
        [
            (Notable::Outlier, tally.outlier),
            (Notable::Overflowed, tally.overflowed),
            (Notable::OutsideTheGate, tally.outside_the_gate),
            (Notable::Salvaged, tally.salvaged),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .filter_map(|(what, count)| Some((marks::notable_word(what)?, count)))
        .collect()
    }

    /// 目录行行尾那一句：在跑的带着当前那一卷与它的进度，出事的报个数，
    /// 一卷都没做完就是「等待中」，整枝跳过就是跳过那一句。
    fn directory_tail(&self, tally: &BranchTally) -> Vec<Segment> {
        if let Some(at) = tally.deciding {
            return vec![
                Segment::plain(format!("{} ⋅ ", self.volume_name(at))),
                Segment::new("等待确认", Look::tone(Tone::Caution).bold()),
            ];
        }
        if let Some(at) = tally.running {
            let mut segments = vec![Segment::plain(format!("{} ⋅ ", self.volume_name(at)))];
            segments.extend(self.walking_segments(at, 8));
            return segments;
        }
        let problems = problem_segments(tally);
        if !problems.is_empty() {
            return problems;
        }
        if tally.finished == 0 {
            return vec![Segment::new("等待中", Look::FAINT.dim())];
        }
        Vec::new()
    }

    /// 清单里第几卷叫什么（屏上那个名字与进度条印的是同一个）。
    fn volume_name(&self, at: usize) -> String {
        self.session
            .views
            .task
            .tree
            .root(at)
            .map_or_else(String::new, render::volume_name)
    }

    /// 正在处理那一段：环节 · 横条 · 走到第几页（摆法与总览那一行同一副，在 [`marks`] 一处）。
    ///
    /// **`at` 必须就是此刻在跑的那一卷**：走到哪个环节、这一环节走到第几步，只有那一卷
    /// 答得出（`Live::walking`）——三个调用处给的都是 `BranchTally` 认出来的那一卷。
    fn walking_segments(&self, at: usize, width: u16) -> Vec<Segment> {
        let Some(live) = self.live else {
            return Vec::new();
        };
        let pages = live
            .roster()
            .get(at)
            .map_or(0, |listed| listed.source_pages);
        let (pass, done) = marks::at_this_pass(live, pages);
        marks::pass_segments(pass, done, pages, width)
    }

    /// 报告那一侧一句成句的话（跳过、隔离……）：**字只有一处出处**（ADR 0016）。
    fn sentence(&self, at: usize, kind: RowKind) -> Option<String> {
        let report = self.report_of(at)?;
        render::volume(
            report,
            self.session.taste.white_align_limit.unwrap_or_default(),
        )
        .iter()
        .find(|row| row.kind == kind)
        .and_then(|row| row.cell(Field::Sentence))
        .map(str::to_owned)
    }

    /// 一卷那一行。
    fn volume_row(
        &self,
        canvas: &mut Canvas<'_>,
        spot: Spot,
        (at, indent): (usize, u16),
        pointed: Pointed,
    ) {
        let state = self
            .live
            .and_then(|live| live.states().get(at).copied())
            .unwrap_or(VolumeState::Queued);
        let notable = self.notable(at);
        let report = self.report_of(at);
        let pages = report.map_or_else(
            || {
                self.live
                    .and_then(|live| live.roster().get(at))
                    .map_or(0, |listed| listed.source_pages)
            },
            VolumeReport::page_count,
        );
        let tally = report
            .filter(|_| state != VolumeState::Aborted)
            .map(render::tally_pairs)
            .unwrap_or_default();
        // 一页都没判的卷在灰阶分布那一列上写的是**为什么**（`render::tally_column` 那两个词）。
        let why_nothing = match state {
            VolumeState::Skipped => vec![Segment::new("跳过", Look::FAINT.dim())],
            VolumeState::Failed => vec![Segment::new("没做成", Look::tone(Tone::Trouble))],
            _ => Vec::new(),
        };
        let running = matches!(state, VolumeState::Running { .. } | VolumeState::Deciding);
        self.lined_row(
            canvas,
            spot,
            LinedRow {
                pointed,
                indent,
                chevron: "  ",
                mark: marks::volume_mark(state, notable > 0, self.spin(0)),
                name: self.volume_name(at),
                name_look: if running {
                    Look::kind(Kind::Working)
                } else {
                    Look::kind(Kind::Volume)
                },
                count: format!("{pages} 页"),
                count_look: Look::FAINT,
                tally,
                why_nothing,
                driver: self.driver_of(at),
                elapsed: self.elapsed_of(at),
                tail: self.volume_tail(at, state, notable),
            },
        );
    }

    /// **代表页那一列**：这一卷的档位是哪一页定出来的。只有整卷统一灰阶判出来的卷有
    /// （默认逐页那一趟这一列整个不在场，停车场 Q712）。字与逐页表抬头、与旧卷表那一列
    /// 出自同一处（`draw::table::driver`）。
    fn driver_of(&self, at: usize) -> Option<String> {
        let report = self.report_of(at)?;
        driver(&render::volume(
            report,
            self.session.taste.white_align_limit.unwrap_or_default(),
        ))
    }

    /// 卷行行尾那一句：**这一行此刻最要紧的事**（`CONTEXT.md` 的《目录行 / 卷行》）。
    fn volume_tail(&self, at: usize, state: VolumeState, notable: usize) -> Vec<Segment> {
        match state {
            VolumeState::Running { .. } => self.walking_segments(at, 10),
            VolumeState::Deciding => vec![Segment::new(
                "等待确认 ⋅ 已分析，还没写入任何文件",
                Look::tone(Tone::Caution),
            )],
            VolumeState::Failed => self
                .live
                .and_then(|live| live.undone_at(at))
                .map(|reason| vec![Segment::new(reason, Look::tone(Tone::Trouble))])
                .unwrap_or_default(),
            VolumeState::Skipped => self
                .sentence(at, RowKind::Skipped)
                .map(|said| vec![Segment::new(said, Look::FAINT.dim())])
                .unwrap_or_default(),
            VolumeState::Aborted => vec![Segment::new("已中断，未保存", Look::FAINT.dim())],
            VolumeState::Isolated => self
                .sentence(at, RowKind::Isolated)
                .map(|said| vec![Segment::new(said, Look::tone(Tone::Caution))])
                .unwrap_or_default(),
            VolumeState::Queued => vec![Segment::new("等待中", Look::FAINT.dim())],
            VolumeState::Done if notable > 0 => {
                let said: Vec<String> = self
                    .notable_bits(at)
                    .into_iter()
                    .map(|(what, count)| format!("{what} {count}"))
                    .collect();
                vec![Segment::new(said.join(" ⋅ "), Look::tone(Tone::Caution))]
            }
            VolumeState::Done => Vec::new(),
        }
    }

    /// **目录行与卷行共用的那一套列**：光标 · 缩进 · 展开记号 · 行首记号 · 名字 · 卷数 ·
    /// 灰阶分布 · 代表页 · 耗时 · 行尾那一句。哪几列在场由 [`TreeWidths`] 说了算。
    fn lined_row(&self, canvas: &mut Canvas<'_>, spot: Spot, row: LinedRow) {
        let Spot { x: left, y, inner } = spot;
        let widths = &self.widths;
        let mut pen = canvas.put(left, y, &row.pointed.cursor.text, row.pointed.cursor.look);
        pen = canvas.put(
            pen,
            y,
            &" ".repeat(usize::from(row.indent * A_LEVEL)),
            Look::PLAIN,
        );
        pen = canvas.put(pen, y, row.chevron, Look::FAINT);
        pen = canvas.put(pen, y, &format!("{} ", row.mark.glyph), row.mark.look);
        let name_width = widths.name.saturating_sub(row.indent * A_LEVEL);
        let name_look = name_look(row.name_look, &row.pointed);
        pen = canvas.put(
            pen,
            y,
            &padded(
                &columns::elide(&row.name, usize::from(name_width)),
                name_width,
            ),
            name_look,
        );
        pen += 1;
        pen = canvas.put(
            pen,
            y,
            &to_the_right(&row.count, widths.count),
            row.count_look,
        );
        pen += 2;
        // 剩下那几列**在不在场由砍列说了算**（次序在 `columns` 一处）：恒在的三列
        // （记号 · 名字 · 卷数）上面已经画过。
        let kept = widths.kept();
        if kept.contains(&TreeColumn::Tally) {
            let segments = if row.tally.is_empty() {
                row.why_nothing.clone()
            } else {
                marks::tally_segments(&row.tally, 2)
            };
            canvas.line(pen, y, &segments, Some(widths.tally));
            pen += widths.tally + 2;
        }
        if kept.contains(&TreeColumn::Driver) {
            if let Some(driver) = &row.driver {
                canvas.line(
                    pen,
                    y,
                    &[Segment::faint("代表页 "), Segment::plain(driver.clone())],
                    Some(widths.driver),
                );
            }
            pen += widths.driver + 2;
        }
        if kept.contains(&TreeColumn::Elapsed) {
            let said = if row.elapsed.is_zero() {
                String::new()
            } else {
                spell(row.elapsed)
            };
            pen = canvas.put(pen, y, &to_the_right(&said, widths.elapsed), Look::FAINT);
            pen += 2;
        }
        // 行尾那一句吃剩下的；剩不下四格就整句不摆——半句话比没有话更坏。
        let room = (left + inner).saturating_sub(pen);
        if room > 4 {
            canvas.line(pen, y, &row.tail, Some(room));
        }
    }
}

/// 一行要画的那几格。
struct LinedRow {
    pointed: Pointed,
    indent: u16,
    chevron: &'static str,
    mark: Mark,
    name: String,
    /// 名字那一列的**底色**：加粗与下划线由 [`name_look`] 按 [`Pointed`] 叠上去。
    name_look: Look,
    count: String,
    count_look: Look,
    tally: Vec<(Candidate, usize)>,
    /// 一页都没判的卷在灰阶分布那一列上写的是**为什么**（跳过、没做成），不空着。
    why_nothing: Vec<Segment>,
    driver: Option<String>,
    elapsed: Duration,
    tail: Vec<Segment>,
}

/// 靠右摆到 `width` 格宽。
fn to_the_right(text: &str, width: u16) -> String {
    let used = crate::wrap::width(text);
    format!(
        "{}{}",
        " ".repeat(usize::from(width.saturating_sub(used))),
        text
    )
}

/// 一枝上**出了什么事**那几段：隔离几卷、转换失败几卷。
fn problem_segments(tally: &BranchTally) -> Vec<Segment> {
    problem_parts(tally, 0)
}

/// 同上，另外数进这一枝上**无法访问**几处（那是备注行的事，不是卷的事，因此单独带进来）。
fn problem_parts(tally: &BranchTally, unreachable: usize) -> Vec<Segment> {
    let mut parts: Vec<Segment> = Vec::new();
    if tally.isolated > 0 {
        parts.push(Segment::new(
            render::isolated_note(&tally.isolated.to_string()),
            Look::tone(Tone::Caution),
        ));
    }
    if tally.failed_volumes > 0 {
        parts.push(Segment::new(
            format!("转换失败 {}", tally.failed_volumes),
            Look::tone(Tone::Trouble),
        ));
    }
    if unreachable > 0 {
        parts.push(Segment::new(
            format!("无法访问 {unreachable}"),
            Look::tone(Tone::Trouble),
        ));
    }
    marks::dotted(parts)
}
