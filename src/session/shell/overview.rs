//! **总览**：任务视图最上面钉住的那一块（`CONTEXT.md` 的《会话》：总览）。
//!
//! 框的抬头答「此刻在做什么」，正文随阶段换（`CONTEXT.md` 的《总览》）：
//!
//! - **还没开始**：输出目录 · 勾了几条路径 · 型号 · 处理选项套的哪一份预设、改了几项；`t`／`x` 各做什么。
//! - **清点中**：只说正在清点、**不报卷数**（清点途中库一条事件都不报，停车场 Q720）。
//! - **跑起来之后**：总进度（横条 · 百分比 · 步数）· 当前卷（卷名 · 环节 · 这一环节的横条）·
//!   结论行 · 问题行。**结论行与问题行答的是「这一趟至今真写出过东西没有」**，一趟之内翻一次、
//!   翻过不翻回（[`Live::has_written`]）。
//! - **结束之后**：抬头整条换成结束那句话（完成 · 已停止 · 已中断，加用时），右端换成输出目录。
//!
//! **不到 30 行高正文收成两行**（`CONTEXT.md` 的《让位》）；横条先收窄，窄到看不出比例就让掉。

use std::time::Instant;

use ratatui::layout::Rect;
use tonefit::{Candidate, Instruction, RunOutcome};

use super::super::draw::overview::{pass_name, spell};
use super::super::keymap::{self, Deed, Phase, Want};
use super::super::live::{Live, VolumeState};
use super::super::look::{Kind, Look, Segment};
use super::super::state::Session;
use super::super::tone::Tone;
use super::canvas::{Border, Canvas, hint};
use super::marks;
use super::topbar::model;
use super::yielding;
use crate::render;

/// 画总览，从第 `y` 行起、占整宽；回它占了几行（正文加上下两条框线）。
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
    y: u16,
) -> u16 {
    let screen = Rect::new(0, 0, canvas.width(), canvas.height());
    let inner = screen.width.saturating_sub(4);
    let lines = lines(session, live, phase, now, inner, yielding::compact(screen));
    let height = lines.len() as u16 + 2;
    canvas.frame(
        Rect::new(0, y, screen.width, height),
        &Border {
            thick: false,
            look: Look::FAINT,
            title: &title(session, live, phase),
            right: &right(session, live, phase),
            bottom_left: &[],
            bottom_right: &[],
        },
    );
    for (i, line) in lines.iter().enumerate() {
        canvas.line(2, y + 1 + i as u16, line, Some(inner));
    }
    height
}

/// 框的抬头：**答的是「此刻在做什么」**（`CONTEXT.md` 的《总览》）。
fn title(session: &Session, live: Option<&Live>, phase: Phase) -> Vec<Segment> {
    // **问的是阶段，不是「有没有那一趟」**：结束之后按 `o` 回到开跑之前那一副时，
    // 那一趟还攒在手上（退出时仍要印它的报告），而屏上这一条该说「还没开始」。
    let (Some(live), false) = (live, phase == Phase::Fresh) else {
        return vec![Segment::new("还没开始", Look::PLAIN.bold())];
    };
    if phase == Phase::Ended {
        return ended_title(live);
    }
    let mut segments = vec![mode_word(live)];
    match phase {
        // 清点中那一段**不接「正在停止」**：那时连第几卷都还说不出（停车场 Q720），
        // 屏底那一句已经答过按下去发生了什么。
        Phase::Surveying => {
            segments.push(Segment::plain(" ⋅ 清点中"));
            return segments;
        }
        Phase::Deciding => {
            let overall = live.overall();
            segments.push(Segment::plain(format!(
                " ⋅ 第 {}/{} 卷 ⋅ ",
                overall.volume, overall.volumes
            )));
            segments.push(Segment::new("等待确认", Look::tone(Tone::Caution).bold()));
        }
        _ => {
            let overall = live.overall();
            segments.push(Segment::plain(format!(
                " ⋅ 第 {}/{} 卷",
                overall.volume, overall.volumes
            )));
            segments.push(Segment::plain(match overall.left {
                Some(left) => format!(" ⋅ 预计还要 {}", spell(left)),
                None => " ⋅ 正在估算剩余时间".to_owned(),
            }));
        }
    }
    // 按了一次停止：抬头接一句（按到的那一级记在会话的阶段上，`Session::stopping`）。
    if session.stopping() == Instruction::Finish {
        segments.push(Segment::new(
            " ⋅ 正在停止：做完当前卷就停",
            Look::tone(Tone::Caution).bold(),
        ));
    }
    segments
}

/// **预览还是转换**：抬头答的是「此刻在写没写」——答出第一个继续之后它就是转换了。
fn mode_word(live: &Live) -> Segment {
    if writing(live) {
        Segment::new("转换", Look::kind(Kind::Convert).bold())
    } else {
        Segment::new("预览", Look::kind(Kind::Preview).bold())
    }
}

/// 抬头那一格的「在写没写」：这一趟起手就是转换、已经写出过东西，或者当前这一卷
/// **此刻正走在写出环节上**——最后那一条比结论行早翻一拍，抬头因此先说转换。
fn writing(live: &Live) -> bool {
    live.started_as() == tonefit::Mode::Process
        || live.has_written()
        || live.walking().is_some_and(|walking| walking.writes)
}

/// 结束之后那一条：完成 · 已停止 · 已中断，加用时。
fn ended_title(live: &Live) -> Vec<Segment> {
    let overall = live.overall();
    let (word, look, why) = match live.report().outcome {
        RunOutcome::Stopped(Instruction::Abort) => (
            "已中断 ",
            Look::tone(Tone::Caution).bold(),
            // 被立即停止掉的那一卷**照它自己在清单上的序号**说，不按「收摊了几卷」说：
            // 它没收摊，而屏上那一句问的正是「哪一卷没保存」。
            format!("⋅ 第 {} 卷未保存，不会留下半成品", aborted_at(live)),
        ),
        RunOutcome::Stopped(_) => (
            "已停止 ",
            Look::tone(Tone::Caution).bold(),
            format!("⋅ 处理到第 {} 卷", overall.volume),
        ),
        _ => (
            "完成 ",
            Look::kind(Kind::Done).bold(),
            format!("⋅ {} 卷全部处理完", overall.volumes),
        ),
    };
    vec![
        Segment::new(word, look),
        Segment::plain(why),
        Segment::faint(format!(" ⋅ 用时 {}", spell(overall.elapsed))),
    ]
}

/// 被立即停止掉的是清单上第几卷（从 1 起）。一卷都没开就落回收摊了几卷。
fn aborted_at(live: &Live) -> usize {
    live.states()
        .iter()
        .position(|state| *state == VolumeState::Aborted)
        .map_or_else(|| live.overall().volume, |at| at + 1)
}

/// 框右端那一段：跑着时是已用多久，结束之后换成输出目录。
fn right(session: &Session, live: Option<&Live>, phase: Phase) -> Vec<Segment> {
    // 同上：开跑之前那一副右端一个字都不写（见 [`title`]）。
    let (Some(live), false) = (live, phase == Phase::Fresh) else {
        return Vec::new();
    };
    if phase == Phase::Ended {
        return vec![Segment::faint(format!(
            "输出目录 {}",
            session.output_shown()
        ))];
    }
    vec![Segment::faint(format!(
        "已用 {}",
        spell(live.overall().elapsed)
    ))]
}

/// 正文那几行。`compact` 是不到 30 行高那一档。
fn lines(
    session: &Session,
    live: Option<&Live>,
    phase: Phase,
    now: Instant,
    inner: u16,
    compact: bool,
) -> Vec<Vec<Segment>> {
    match (phase, live) {
        (Phase::Fresh, _) | (_, None) => before_the_run(session, compact),
        (Phase::Surveying, Some(_)) => surveying(session, now),
        (_, Some(live)) => in_a_run(session, live, phase, inner, compact),
    }
}

/// **清点中**那两行：只说正在清点，**不报卷数**（停车场 Q720）。
fn surveying(session: &Session, now: Instant) -> Vec<Vec<Segment>> {
    vec![
        vec![
            Segment::new(
                format!("{} 清点 ", marks::spinner(now, session.opened_at, 0)),
                Look::kind(Kind::Surveying).bold(),
            ),
            Segment::plain("正在找出每个路径里的卷"),
            Segment::faint("  ⋅ 先数清总量，剩余时间才估得准"),
        ],
        vec![
            Segment::faint("当前卷 "),
            Segment::new("还没开始", Look::FAINT.dim()),
        ],
    ]
}

/// **跑起来之后**那几行：总进度 · 当前卷 · 结论行 · 问题行。
fn in_a_run(
    session: &Session,
    live: &Live,
    phase: Phase,
    inner: u16,
    compact: bool,
) -> Vec<Vec<Segment>> {
    let overall = live.overall();
    let ended = phase == Phase::Ended;
    let counted = Counted::of(live);
    if compact {
        return compact_lines(session, live, &counted, ended);
    }
    let mut total = walked_so_far(
        overall.walked,
        overall.steps,
        inner.saturating_sub(60).clamp(10, 40),
        ended,
    );
    total.push(Segment::faint(format!(
        "  {}/{} 步",
        overall.walked, overall.steps
    )));
    let mut here = vec![
        Segment::faint("当前卷 "),
        Segment::new("—", Look::FAINT.dim()),
    ];
    if !ended && let Some(walking) = live.walking() {
        let (pass, done, pages) = walked(session, live);
        here = vec![
            Segment::faint("当前卷 "),
            Segment::new(
                super::super::columns::elide(&current_name(session, live), 34),
                Look::PLAIN.bold(),
            ),
            Segment::faint(" ⋅ "),
            Segment::new(pass_name(pass), marks::pass_look(pass)),
            Segment::plain(" "),
        ];
        let width = inner.saturating_sub(80).clamp(8, 24);
        let fraction = if pages > 0 {
            done as f64 / pages as f64
        } else {
            0.0
        };
        here.extend(marks::pass_bar(pass, fraction, width));
        here.push(Segment::faint(format!(" {done}/{pages}")));
        let _ = walking;
    }
    let mut lines = vec![total, here, counted.conclusion()];
    let trouble = counted.trouble();
    if !trouble.is_empty() {
        let mut line = vec![Segment::new("问题 ", Look::tone(Tone::Trouble).bold())];
        line.extend(trouble);
        line.push(Segment::plain("    "));
        line.extend(
            keymap::hints(
                phase,
                session.views.block(),
                &[Want::saying(Deed::NextProblem, "跳到下一个")],
            )
            .first()
            .map(|said| hint(&said.spelt(), said.what))
            .unwrap_or_default(),
        );
        lines.push(line);
    }
    lines
}

/// **总进度那一截**：标签 · 横条 · 百分比。宽窄两副共用（窄的那一副横条收成十格）。
fn walked_so_far(walked: u64, steps: u64, width: u16, ended: bool) -> Vec<Segment> {
    let fraction = if steps > 0 {
        walked as f64 / steps as f64
    } else {
        0.0
    };
    let (full, empty) = marks::bar(fraction, width);
    vec![
        Segment::faint("总进度 "),
        Segment::new(
            full,
            Look::kind(if ended { Kind::Done } else { Kind::Working }),
        ),
        Segment::new(empty, Look::FAINT.dim()),
        Segment::new(
            format!(" {}%", marks::percent(walked, steps)),
            Look::PLAIN.bold(),
        ),
    ]
}

/// 不到 30 行那两行：总进度与当前卷挤在头一行，结论与问题挤在第二行。
fn compact_lines(
    session: &Session,
    live: &Live,
    counted: &Counted,
    ended: bool,
) -> Vec<Vec<Segment>> {
    let overall = live.overall();
    let mut first = walked_so_far(overall.walked, overall.steps, 10, ended);
    if !ended && live.walking().is_some() {
        let (pass, done, pages) = walked(session, live);
        first.extend([
            Segment::faint("   当前卷 "),
            Segment::new(
                super::super::columns::elide(&current_name(session, live), 22),
                Look::PLAIN.bold(),
            ),
            Segment::new(format!(" {}", pass_name(pass)), marks::pass_look(pass)),
            Segment::faint(format!(" {done}/{pages}")),
        ]);
    }
    let mut second = counted.short_conclusion();
    let trouble = counted.short_trouble();
    if !trouble.is_empty() {
        second.push(Segment::new("   问题 ", Look::tone(Tone::Trouble).bold()));
        second.extend(trouble);
    }
    vec![first, second]
}

/// 当前卷屏上怎么写：它那个目录加卷名（`集英社/海贼王/第15卷`）。
fn current_name(session: &Session, live: &Live) -> String {
    let Some(walking) = live.walking() else {
        return String::new();
    };
    let tree = &session.views.task.tree;
    let name = render::volume_name(&walking.volume);
    match tree
        .index_of(&walking.volume)
        .and_then(|at| tree.directory_of(at))
    {
        Some(directory) => format!("{}/{name}", directory.label),
        None => name,
    }
}

/// 当前卷走到哪个环节、这一环节走到第几页、这一卷共几页。
fn walked(session: &Session, live: &Live) -> (Option<tonefit::Pass>, usize, usize) {
    let Some(walking) = live.walking() else {
        return (None, 0, 0);
    };
    let pages = session
        .views
        .task
        .tree
        .index_of(&walking.volume)
        .and_then(|at| live.roster().get(at))
        .map_or(0, |listed| listed.source_pages);
    let (pass, done) = marks::at_this_pass(live, pages);
    (pass, done, pages)
}

/// 这一趟到此刻为止各样各几件：结论行与问题行数的都是它。
struct Counted {
    wrote: bool,
    done: usize,
    skipped: usize,
    waiting: usize,
    judged: usize,
    tally: Vec<(Candidate, usize)>,
    failed_pages: usize,
    failed_volumes: usize,
    unreachable: usize,
    /// **判定上要留意的几样**：差异大的页与页面超宽的页。**只有预览那一副给**
    /// （`CONTEXT.md` 的《总览》：问题行——预览那一副另给判定上要留意的几样）。
    outlier: usize,
    wide: usize,
}

impl Counted {
    fn of(live: &Live) -> Self {
        let mut counted = Self {
            wrote: live.has_written(),
            done: 0,
            skipped: 0,
            waiting: 0,
            judged: 0,
            tally: Vec::new(),
            failed_pages: live.failures_so_far(),
            failed_volumes: 0,
            unreachable: live.unreachable_places().len(),
            outlier: 0,
            wide: 0,
        };
        let panel = live.report().profile.panel();
        for (at, state) in live.states().iter().enumerate() {
            match state {
                VolumeState::Done | VolumeState::Isolated => counted.done += 1,
                VolumeState::Skipped => counted.skipped += 1,
                VolumeState::Failed => counted.failed_volumes += 1,
                _ => {}
            }
            if !state.settled() {
                counted.waiting += 1;
            }
            if *state == VolumeState::Aborted {
                continue;
            }
            let Some(report) = live.listed_at(at).and_then(|which| live.volume(which)) else {
                continue;
            };
            let pairs = render::tally_pairs(report);
            if pairs.is_empty() {
                continue;
            }
            counted.judged += 1;
            for (candidate, pages) in pairs {
                marks::add_to(&mut counted.tally, candidate, pages);
            }
            // 判定上要留意的那两样**判在一处**（`render::notable`），与每页结果、
            // 与命令行印出去的那一份同一份判定。
            for page in render::notable(report, panel) {
                counted.outlier += usize::from(page.contains(&render::Notable::Outlier));
                counted.wide += usize::from(page.contains(&render::Notable::Overflowed));
            }
        }
        counted
    }

    /// **结论行**：转换那一副给完成 · 跳过 · 等待，预览那一副给已分析几卷与灰阶分布。
    fn conclusion(&self) -> Vec<Segment> {
        if self.wrote {
            return vec![
                Segment::faint("完成 "),
                Segment::new(format!("{} 卷", self.done), Look::PLAIN.bold()),
                Segment::faint(" ⋅ 跳过 "),
                Segment::plain(format!("{} 卷", self.skipped)),
                Segment::faint(format!(" ⋅ 等待 {} 卷", self.waiting)),
            ];
        }
        let mut segments = vec![
            Segment::faint("已分析 "),
            Segment::new(format!("{} 卷", self.judged), Look::PLAIN.bold()),
            Segment::faint("   灰阶分布 "),
        ];
        segments.extend(marks::tally_segments(&self.tally, 3));
        segments
    }

    /// 窄屏那一档的结论行。
    fn short_conclusion(&self) -> Vec<Segment> {
        if self.wrote {
            return vec![
                Segment::faint("完成 "),
                Segment::new(self.done.to_string(), Look::PLAIN.bold()),
                Segment::faint(format!(" ⋅ 跳过 {} ⋅ 等待 {}", self.skipped, self.waiting)),
            ];
        }
        vec![
            Segment::faint("已分析 "),
            Segment::new(format!("{} 卷", self.judged), Look::PLAIN.bold()),
        ]
    }

    /// **问题行**：坏页 · 转换失败的卷 · 无法访问的地方。一条都没有时整行不出现。
    fn trouble(&self) -> Vec<Segment> {
        let mut bits = vec![
            (self.failed_pages, "坏页", "页", Tone::Trouble),
            (self.failed_volumes, "转换失败", "卷", Tone::Trouble),
            (self.unreachable, "无法访问", "处", Tone::Trouble),
        ];
        if !self.wrote {
            bits.push((self.outlier, "与其他页差异大", "页", Tone::Caution));
            bits.push((self.wide, "页面超宽", "页", Tone::Caution));
        }
        self.bits(&bits)
    }

    /// 窄屏那一档的问题行：不带量词。
    fn short_trouble(&self) -> Vec<Segment> {
        let mut bits = vec![
            (self.failed_pages, "坏页", "", Tone::Trouble),
            (self.failed_volumes, "转换失败", "", Tone::Trouble),
            (self.unreachable, "无法访问", "", Tone::Trouble),
        ];
        if !self.wrote {
            bits.push((self.outlier, "差异大", "", Tone::Caution));
        }
        self.bits(&bits)
    }

    /// 几件事各报个数、串成一行：一件都没有的不占位（串法在 [`marks::dotted`] 一处）。
    fn bits(&self, of: &[(usize, &str, &str, Tone)]) -> Vec<Segment> {
        let parts: Vec<Segment> = of
            .iter()
            .filter(|(count, ..)| *count > 0)
            .map(|(count, what, unit, tone)| {
                let unit = if unit.is_empty() {
                    String::new()
                } else {
                    format!(" {unit}")
                };
                Segment::new(format!("{what} {count}{unit}"), Look::tone(*tone))
            })
            .collect();
        marks::dotted(parts)
    }
}

/// 还没开始那两行。
fn before_the_run(session: &Session, compact: bool) -> Vec<Vec<Segment>> {
    let (on, off) = session.checked_paths();
    let changed = session.changed_from_preset();
    let applied = session
        .views
        .config
        .applied
        .as_ref()
        .map(|applied| &applied.name);
    // 起一趟那两个键与它们那一句都从按键表取（屏底摆的正是同一份）。
    let mentioned = |deed: Deed| {
        keymap::hints(Phase::Fresh, session.views.block(), &[Want::of(deed)])
            .first()
            .map(|said| hint(&said.spelt(), said.what))
            .unwrap_or_default()
    };
    let output = Segment::new(session.output_shown(), Look::PLAIN.bold());
    if compact {
        let preset = match applied {
            Some(name) => format!("预设「{name}」（改了 {changed} 项）"),
            None => "未使用预设".to_owned(),
        };
        let mut second = mentioned(Deed::Preview);
        second.push(Segment::plain(" "));
        second.extend(mentioned(Deed::Convert));
        second.push(Segment::faint(" ⋅ "));
        second.push(Segment::new(preset, Look::tone(Tone::Caution)));
        return vec![
            vec![
                Segment::faint("输出目录 "),
                output,
                Segment::faint(" ⋅ 路径 "),
                Segment::plain(format!("已勾选 {on} 个")),
                Segment::faint(format!(" ⋅ {}", model(session))),
            ],
            second,
        ];
    }
    let preset = match applied {
        Some(name) if changed > 0 => Segment::new(
            format!("预设「{name}」（改了 {changed} 项）"),
            Look::tone(Tone::Caution),
        ),
        Some(name) => Segment::faint(format!("预设「{name}」")),
        None => Segment::faint("未使用预设"),
    };
    let mut first = vec![
        Segment::faint("输出目录 "),
        output,
        Segment::faint("   路径 "),
        Segment::plain(format!("已勾选 {on} 个")),
    ];
    if off > 0 {
        first.push(Segment::faint(format!(" ⋅ {off} 个未勾选")));
    }
    first.extend([
        Segment::faint("   设备 "),
        Segment::plain(model(session)),
        Segment::faint("   处理选项 "),
        preset,
    ]);
    let mut second = mentioned(Deed::Preview);
    second.push(Segment::faint("  只分析，不写文件"));
    second.push(Segment::plain("     "));
    second.extend(mentioned(Deed::Convert));
    second.push(Segment::faint("  写到输出目录"));
    second.push(Segment::new(
        "     开始后会扫描每个路径里的卷",
        Look::tone(Tone::Muted),
    ));
    vec![first, second]
}
