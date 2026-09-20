//! **转轮 · 横条 · 行首记号**：屏上说「这一行此刻怎么样」的那几个字形
//! （`CONTEXT.md` 的《会话》：行首记号、语义色；spec《颜色》）。
//!
//! 三样各只有一处出处：总览、卷列表那棵树、窗口太小那一屏画的是同一个转轮、同一种横条、
//! 同一套记号——**颜色不是唯一载体**，每一处上色的地方旁边都靠这几个字形说话。
//!
//! 转轮那一样的**出处在 [`super::super::view`]**（`SPINNER` 与 `SPINS_EVERY`）：顶栏右端那一截
//! 读的是同一份，而那一份摆在 `tui` 特性外面——反过来摆不成，本模块整个在特性后面。
//! 本模块只多做一件它管不着的事：一行自己的**错相**（`offset`）。

use std::time::{Duration, Instant};

use tonefit::{Candidate, Dither, Pass};

use super::super::live::VolumeState;
use super::super::look::{Kind, Look, Segment};
use super::super::tone::Tone;
use super::super::view::{SPINNER, SPINS_EVERY};
use crate::render::Notable;

/// 横条的两个字形：走过的那一截与没走的那一截（盲文点阵，宽度稳）。
const FULL: &str = "⣿";
const EMPTY: &str = "⣀";

/// 转轮此刻转到第几格：从**会话打开那一刻**算（[`super::super::state::Session::opened_at`]），
/// 不从这一趟的计时算——清点中那一段一步都没走，转轮照样得转。
///
/// `offset` 是这一行自己的错相：清点中那一副的处理路径**一行一格地错开**（设计稿），
/// 让一串路径看着像在依次扫过去；别处一律传零，屏上那几个转轮同相。
pub(super) fn spinner(now: Instant, opened_at: Instant, offset: usize) -> &'static str {
    let frames = now.saturating_duration_since(opened_at).as_millis() / SPINS_EVERY.as_millis();
    let at = (frames as usize).wrapping_add(offset) % SPINNER.len();
    SPINNER[at]
}

/// 一页**要紧在哪一处，屏上那个词**（`CONTEXT.md` 的《语义色》在页那一级分出的那几样）。
///
/// **新界面的措辞只有这一处**：卷行行尾按种类报几页（[`super::list`]，数出自
/// [`Live::notable_at`](super::super::live::Live::notable_at)）与每页结果提示那一列写
/// 这一页要紧在哪几处（[`super::pages`]，判出自
/// [`render::notable`](crate::render::notable)）——两处读同一份，
/// 同一件事在屏上不会有两个叫法。
///
/// **[坏页](Notable::Failed)不给词**：它行尾跟着那一句原因，而那一句以「失败」开头——
/// 多加一个词是同一件事说两遍。哪几种在某一处**不写词**由那一处自己答
/// （每页结果另把[残缺](Notable::Salvaged)让给 `救回 62.0%` 那一格）。
pub(super) fn notable_word(what: Notable) -> Option<&'static str> {
    match what {
        Notable::Failed => None,
        Notable::Salvaged => Some("残缺"),
        Notable::Outlier => Some("差异大的页"),
        Notable::OutsideTheGate => Some("尺寸未贴合屏幕"),
        Notable::Overflowed => Some("页面超宽"),
        Notable::Backstopped => Some("兜底上界"),
        Notable::Driver => Some("代表页"),
    }
}

/// 一条横条画成什么样：走过的那一截与没走的那一截，合起来恰好 `width` 格。
///
/// 满格数**四舍五入**（设计稿的 `Math.round`）：一条八格的横条上，走了七分之一与
/// 走了八分之一该画成同一格。
pub(super) fn bar(fraction: f64, width: u16) -> (String, String) {
    let width = usize::from(width);
    let full = (fraction.clamp(0.0, 1.0) * width as f64).round() as usize;
    let full = full.min(width);
    (FULL.repeat(full), EMPTY.repeat(width - full))
}

/// 一条走过一截的横条，连同它那两截的样子：走过的那一截按**环节**上种类色，
/// 没走的那一截压暗。
pub(super) fn pass_bar(pass: Option<Pass>, fraction: f64, width: u16) -> Vec<Segment> {
    let (full, empty) = bar(fraction, width);
    vec![
        Segment::new(full, pass_look(pass)),
        Segment::new(empty, Look::FAINT.dim()),
    ]
}

/// 一个**环节**在屏上是哪一色（种类色：查重品红 · 分析蓝 · 写出青）。
/// 还没进环节的那一段跟着分析走——屏上那一格写的是「开卷」，它是分析之前的那一截。
pub(super) fn pass_look(pass: Option<Pass>) -> Look {
    Look::kind(Kind::Pass(pass.unwrap_or(Pass::First)))
}

/// 一个**行首记号**：一个字符，连同它的样子。
pub(super) struct Mark {
    pub(super) glyph: &'static str,
    pub(super) look: Look,
}

impl Mark {
    const fn of(glyph: &'static str, look: Look) -> Self {
        Self { glyph, look }
    }

    /// 等待中那一个：`⋅`，压暗。
    pub(super) fn queued() -> Self {
        Self::of("⋅", Look::FAINT)
    }

    /// 出错那一个：`✗`，出事色加粗。
    pub(super) fn broken() -> Self {
        Self::of("✗", Look::tone(Tone::Trouble).bold())
    }

    /// 已跳过那一个：`-`，压暗。
    pub(super) fn skipped() -> Self {
        Self::of("-", Look::FAINT)
    }

    /// 这一个记号写成一截字（后面跟一个空格，与名字之间恒隔一格）。
    pub(super) fn segment(&self) -> Segment {
        Segment::new(format!("{} ", self.glyph), self.look)
    }
}

/// **一卷此刻的行首记号**（`CONTEXT.md` 的《行首记号》《卷状态》）：
/// `✓` 完成 · 转轮 处理中 · `?` 等待确认 · `!` 需留意 · `✗` 出错 · `-` 已跳过 ·
/// `⋅` 等待中。`notable` 是这一卷有没有需留意的页——收摊了的卷靠它分 `✓` 与 `!`。
pub(super) fn volume_mark(state: VolumeState, notable: bool, spin: &'static str) -> Mark {
    match state {
        VolumeState::Running { .. } => Mark::of(spin, Look::kind(Kind::Working).bold()),
        VolumeState::Deciding => Mark::of("?", Look::tone(Tone::Caution).bold()),
        VolumeState::Skipped => Mark::skipped(),
        VolumeState::Failed => Mark::broken(),
        VolumeState::Isolated => Mark::of("!", Look::tone(Tone::Caution).bold()),
        VolumeState::Aborted | VolumeState::Queued => Mark::queued(),
        VolumeState::Done if notable => Mark::of("!", Look::tone(Tone::Caution).bold()),
        VolumeState::Done => Mark::of("✓", Look::kind(Kind::Done)),
    }
}

/// **一个目录行或一条分区的行首记号**：底下那几卷**一行要紧在好几处时取最重的那一个**
/// （`CONTEXT.md` 的《行首记号》），外加它自己那一档「做了一部分」。
///
/// 次序即轻重：等待确认 → 处理中 → 出错 → 需留意 → 全做完 → 做了一部分 → 等待中。
pub(super) fn branch_mark(tally: &BranchTally, spin: &'static str) -> Mark {
    if tally.deciding.is_some() {
        return Mark::of("?", Look::tone(Tone::Caution).bold());
    }
    if tally.running.is_some() {
        return Mark::of(spin, Look::kind(Kind::Working).bold());
    }
    if tally.bad > 0 {
        return Mark::broken();
    }
    if tally.warn > 0 {
        return Mark::of("!", Look::tone(Tone::Caution).bold());
    }
    if tally.total > 0 && tally.finished == tally.total {
        return if tally.skipped == tally.total {
            Mark::skipped()
        } else {
            Mark::of("✓", Look::kind(Kind::Done))
        };
    }
    if tally.finished > 0 {
        return Mark::of("◌", Look::FAINT);
    }
    Mark::queued()
}

/// 一枝底下**那几卷合起来怎么样**：目录行、分区标题与总览的问题行数的都是它。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct BranchTally {
    pub(super) total: usize,
    pub(super) finished: usize,
    pub(super) skipped: usize,
    /// 有一卷正在处理——是清单里第几卷。
    pub(super) running: Option<usize>,
    /// 有一卷停在确认点上——是清单里第几卷。
    pub(super) deciding: Option<usize>,
    /// 出事的有几件（没做成的卷、无法访问的地方）。
    pub(super) bad: usize,
    /// 要留意的有几件（进了隔离、有需留意的页）。
    pub(super) warn: usize,
    pub(super) isolated: usize,
    pub(super) failed_volumes: usize,
    pub(super) failed_pages: usize,
    /// 灰阶分布：档位与页数，按清单上撞见的先后。
    pub(super) tally: Vec<(Candidate, usize)>,
    pub(super) elapsed: Duration,
}

impl BranchTally {
    /// 把另一枝并进来（分区的汇总是它底下那几个目录行加起来）。
    pub(super) fn absorb(&mut self, other: &Self) {
        self.total += other.total;
        self.finished += other.finished;
        self.skipped += other.skipped;
        self.running = self.running.or(other.running);
        self.deciding = self.deciding.or(other.deciding);
        self.bad += other.bad;
        self.warn += other.warn;
        self.isolated += other.isolated;
        self.failed_volumes += other.failed_volumes;
        self.failed_pages += other.failed_pages;
        self.elapsed += other.elapsed;
        for (candidate, pages) in &other.tally {
            add_to(&mut self.tally, *candidate, *pages);
        }
    }
}

/// **一笔灰阶分布记进来**：同一个候选就加上去，没有就添一条。
///
/// 收的是[候选](Candidate)本身、不是印好的那一串——**表不许回头去认字符串**
/// （ADR 0016 的《后果》）。一卷那几笔出自 [`crate::render::tally_pairs`]，
/// 目录行那一级把底下几卷的加起来，这里就是加的那一手。
pub(super) fn add_to(tally: &mut Vec<(Candidate, usize)>, candidate: Candidate, pages: usize) {
    match tally.iter_mut().find(|(one, _)| *one == candidate) {
        Some((_, already)) => *already += pages,
        None => tally.push((candidate, pages)),
    }
}

/// **灰阶分布那一格**：页多的在前，最多 `most` 笔，灰阶档位上**种类色**、`+FS` 那半截压暗
/// （`CONTEXT.md` 的《语义色》：灰阶档位 1bit 品红 · 2bit 青 · 4bit 蓝）。
///
/// **屏上那几个字出自候选自己的写法**（`4bit`／`4bit+FS`，`Candidate` 的 `Display`），
/// 这一层一个字都不另编：它只挑色，而挑色要的是两半分开——整串上不了两种色。
pub(super) fn tally_segments(tally: &[(Candidate, usize)], most: usize) -> Vec<Segment> {
    let mut sorted: Vec<&(Candidate, usize)> = tally.iter().collect();
    // 几卷加起来之后要再排一次：一卷那几笔从 `render` 出来时就排好了，加完的次序会变。
    sorted.sort_by_key(|(_, pages)| std::cmp::Reverse(*pages));
    let mut segments = Vec::new();
    for (at, (candidate, pages)) in sorted.into_iter().take(most).enumerate() {
        if at > 0 {
            segments.push(Segment::faint(" ⋅ "));
        }
        let look = Look::kind(Kind::Depth(candidate.bit_depth));
        segments.push(Segment::new(candidate.bit_depth.to_string(), look));
        if candidate.dither != Dither::Off {
            // `Candidate` 的写法就是「档位 + 抖动那半截」：拿整串减掉档位那一半，不另写一份。
            let whole = candidate.to_string();
            let tail = whole
                .strip_prefix(&candidate.bit_depth.to_string())
                .unwrap_or_default()
                .to_owned();
            segments.push(Segment::new(tail, look.dim()));
        }
        segments.push(Segment::plain(format!(" {pages}")));
    }
    segments
}

/// **几件事串成一行**：中间用「 ⋅ 」隔开，空的那几件不占位。
///
/// 屏上三处摆的是同一副（目录行与分区的问题计数、总览的问题行）：各写一遍，
/// 迟早有一处的分隔符跟别处分了家。
pub(super) fn dotted(parts: Vec<Segment>) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    for part in parts {
        if !segments.is_empty() {
            segments.push(Segment::faint(" ⋅ "));
        }
        segments.push(part);
    }
    segments
}

/// **正在处理那一段**：环节那个词 · 这一环节的横条 · 走到第几页。
///
/// 总览的当前卷那一行与树上在跑的那几行摆的是同一副（`CONTEXT.md` 的《目录行 / 卷行》：
/// 行尾那一句带的是**这一环节**的进度）。
pub(super) fn pass_segments(
    pass: Option<Pass>,
    done: usize,
    pages: usize,
    width: u16,
) -> Vec<Segment> {
    let fraction = if pages > 0 {
        done as f64 / pages as f64
    } else {
        0.0
    };
    let mut segments = vec![Segment::new(
        format!("{} ", super::super::draw::overview::pass_name(pass)),
        pass_look(pass),
    )];
    segments.extend(pass_bar(pass, fraction, width));
    segments.push(Segment::faint(format!(" {done}/{pages}")));
    segments
}

/// 当前卷**走到哪个环节、这一环节走到第几页、这一卷共几页**。
///
/// 走的步数减掉这一环节开工那一刻的读数（[`super::super::live::Walking::pass_from`]）：
/// 屏上问的是这一环节的进度，不是这一卷累计的步数。
pub(super) fn at_this_pass(live: &super::super::live::Live, pages: usize) -> (Option<Pass>, usize) {
    let Some(walking) = live.walking() else {
        return (None, 0);
    };
    let walked = walking.walked.saturating_sub(walking.pass_from);
    (
        walking.pass,
        usize::try_from(walked).unwrap_or(usize::MAX).min(pages),
    )
}

/// **走了百分之几**（向下取整）：总览那两副与窗口太小那一屏印的是同一个数。
pub(super) fn percent(walked: u64, steps: u64) -> u64 {
    if steps == 0 {
        return 0;
    }
    (walked as f64 / steps as f64 * 100.0).floor() as u64
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// **转轮十格一圈、一格 90 毫秒**，从会话打开那一刻算；一行自己的错相往后挪一格。
    #[test]
    fn the_spinner_turns_one_frame_every_ninety_milliseconds_and_wraps_at_ten() {
        let opened = Instant::now();
        assert_eq!(spinner(opened, opened, 0), "⠋");
        assert_eq!(spinner(opened + Duration::from_millis(90), opened, 0), "⠙");
        assert_eq!(spinner(opened + Duration::from_millis(900), opened, 0), "⠋");
        assert_eq!(spinner(opened, opened, 1), "⠙", "错开一格就是下一格");
    }

    /// **横条满格数四舍五入**，两截加起来恰好那么宽。
    #[test]
    fn the_bar_rounds_to_the_nearest_cell_and_always_fills_its_width() {
        let (full, empty) = bar(0.34, 10);
        assert_eq!((full.chars().count(), empty.chars().count()), (3, 7));
        let (full, empty) = bar(1.5, 4);
        assert_eq!((full.chars().count(), empty.chars().count()), (4, 0));
        let (full, empty) = bar(-1.0, 4);
        assert_eq!((full.chars().count(), empty.chars().count()), (0, 4));
    }

    /// **一行要紧在好几处时取最重的那一个**：出错压过需留意，处理中压过出错。
    #[test]
    fn a_branch_mark_takes_the_heaviest_thing_on_the_row() {
        let heavy = BranchTally {
            total: 4,
            finished: 2,
            bad: 1,
            warn: 1,
            ..BranchTally::default()
        };
        assert_eq!(branch_mark(&heavy, "⠋").glyph, "✗");
        let running = BranchTally {
            running: Some(0),
            ..heavy.clone()
        };
        assert_eq!(branch_mark(&running, "⠋").glyph, "⠋");
        let part = BranchTally {
            total: 4,
            finished: 2,
            ..BranchTally::default()
        };
        assert_eq!(branch_mark(&part, "⠋").glyph, "◌");
    }

    /// **灰阶分布页多的在前**，只摆得下几档就摆几档。
    #[test]
    fn the_tally_puts_the_busiest_depth_first() {
        let tally = vec![
            (Candidate::new(tonefit::BitDepth::Four, Dither::Off), 59),
            (
                Candidate::new(tonefit::BitDepth::Two, Dither::FloydSteinberg),
                123,
            ),
        ];
        let text: String = tally_segments(&tally, 2)
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert_eq!(text, "2bit+FS 123 ⋅ 4bit 59");
    }
}
