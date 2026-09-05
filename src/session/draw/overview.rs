//! 屏上那一块：**总览块**——主区最上面那一块，一个框，抬头加三到四行正文
//! （`CONTEXT.md` 的《会话》：总览）。
//!
//! ```text
//! ┌执行 · 第 3/3 卷 · 还剩约 3m20s ─────────────────      ← 抬头
//! │ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s    ← 全局那一行
//! │ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步 ← 当前卷那一行
//! │ 完成 1 卷 · 跳过 1 卷                                             ← 结论行
//! │ 出事 隔离 1 卷 · 失败 1 页                                        ← 出事行，没事就不出现
//! └─────────────────────────────────────────────────
//! ```
//!
//! （右边那条框线在这张示意图上省掉了；一格不差的四张在本模块的 `mod tests` 里。）
//!
//! **整块钉住**：它与报告区各占主区的一格（[`super::main_pane`] 分），报告在它自己那一格里
//! 滚，一行都推不动这一块。`p1-session/09` 那条「三段各占一格，免得报告长起来把进度条
//! 顶出屏外」由这一条接住。
//!
//! 合成一块之前是**两个框六行**（全局条一个、当前卷条一个），而屏上没有一处答得出
//! 「**这一趟到底怎么样**」——那两件由[结论行](settled_row)与[出事行](trouble_row)答，
//! 两行的内容随这一趟是[什么](Live::mode)而变。票面写的是「三个框九行」，与屏上对不上，
//! 停车场 Q145 记着那一条。
//!
//! 只读那一趟边跑边攒的那一份（[`Live`]），一个字都不在这里重编：卷名走
//! [`crate::render::volume_name`]、收场那一句走 [`crate::render::outcome`]、
//! 按停按到的那一级走 [`super::footer::stopping_name`]。横条画多宽与命令行那两条
//! 同一个出处（[`BAR_WIDTH`]）。
//!
//! 主区第二块是报告区，在 [`super::report`]。
//!
//! 长在本模块的只有**命令行上根本没有**的那一样：这一块的排版
//! （命令行那两条横条是 indicatif 的模板，见 `crate::bar_style`）。

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use tonefit::{Instruction, Mode as RunMode, Pass, Report, VolumeReport, VolumeVerdict};

use super::footer::{START_KEYS, stopping_name};
use super::paint::{Painted, Tone};
use crate::session::live::{Live, Walking};

/// 总览块**最高**几行：四行正文加上下两条边（跑着、而且出了事的那一副）。
///
/// 矮下去的两条见 [`Overview::height`]：出事行不在场少一行，收场之后当前卷那一行也不在。这个数只用来给主区留位子
/// （[`super::MAIN_MIN_HEIGHT`]）——**屏矮下来时先让报告区，总览块不砍**
/// （spec 的《窄终端》：宁可少画表，不少画总览）。
pub(super) const OVERVIEW_HEIGHT: u16 = 6;

/// 一条横条画多宽。**与命令行那两条同一个出处**（`crate::BAR_WIDTH`）：
/// 两处的横条长得一样，读的人不必重新认一遍。
const BAR_WIDTH: u64 = crate::BAR_WIDTH as u64;

/// **停在决策点上等人拿主意**那一句。
///
/// 两处说的是同一件事，措辞因此只有这一处：这一块的[抬头](title)上顶掉「还剩多久」的
/// 那一截，以及卷表上那一卷行尾标着的那一句（`super::table`）。
pub(super) const DECIDING: &str = "等你拿主意";

/// 总览块：**一个框，抬头一行加一到四行正文**。
///
/// 先算出来再画，是因为**它有几行要在分格子之前答得出**：出事行不在场时那一行让给报告区
/// （[`super::main_pane`] 按 [`height`](Self::height) 分格）。画出来的那一份
/// 与算出来的这一份因此不许各算各的。
///
/// 抬头加正文那几行，一行答一件事：
///
/// | 行 | 答的是 |
/// |---|---|
/// | 抬头 | 这一趟是什么 · 走到哪儿 · 还剩多久（外加按停按到哪一级、等答话那一句） |
/// | 全局那一行 | 整趟走了几步、已用多久 |
/// | 当前卷那一行 | 在走哪一卷的哪一遍 |
/// | 结论行 | **这一趟到底怎么样**：试算给判定分布，执行给完成与跳过 |
/// | 出事行 | 要注意的那几件，**一条都没有时整行不出现** |
pub(super) struct Overview {
    /// 边框上那一行。**没做成那一趟它是红的**（见 [`ended_title`]）。
    title: Painted,
    /// 框里那一到四行。各行的[语义](Tone)各自算（出事行见 [`trouble_row`]）。
    rows: Vec<Painted>,
}

impl Overview {
    /// 这一块占几行：正文那几行加上下两条边。
    ///
    /// **让得出去的有两行**，让出去的都归报告区：出事行一条都没有时不画
    /// （与报告末尾那几小结同一条规矩——一条都没有就一个字都不说），
    /// 收场之后当前卷那一行也不画（那时再没有「本卷」可说）。
    pub(super) fn height(&self) -> u16 {
        u16::try_from(self.rows.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
    }

    /// 画出来。**上色按语义要**，一个颜色名都不在这一块里（见 [`super::paint`]）。
    pub(super) fn draw(self) -> Paragraph<'static> {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.line());
        let rows: Vec<Line<'static>> = self.rows.iter().map(Painted::line).collect();
        Paragraph::new(Text::from(rows)).block(block)
    }
}

/// 算出这一块：抬头一行、正文一到四行。
///
/// **按停按到哪一级、以及等答话那一句都挂在抬头上**（停车场 Q71、`p1-session/14`）：
/// 按下收尾之后横条照旧往前走，而「它在等什么」只写在屏底——眼睛盯着横条的人不会往下
/// 扫一行。抬头摆在**边框**上，一列正文都不占。措辞与屏底那一行同一个出处
/// （[`stopping_name`]）。
///
/// **等答话排在按停那一级之前**：横条这时一动不动，而「它为什么不动」是眼睛盯着这一块的人
/// 第一眼要看到的（按过的停要等答完话才继续作数）。
pub(super) fn overview(live: Option<&Live>, pressed: Instruction, deciding: bool) -> Overview {
    let Some(live) = live else {
        return Overview {
            title: Painted::plain("总览".to_owned()),
            rows: vec![Painted::plain(format!(" 还没跑过。{START_KEYS}"))],
        };
    };
    let mut rows = vec![Painted::plain(overall_row(live))];
    rows.extend(volume_row(live).map(Painted::plain));
    rows.push(Painted::plain(settled_row(live)));
    rows.extend(trouble_row(live));
    Overview {
        title: title(live, pressed, deciding),
        rows,
    }
}

/// 抬头：**这一趟是什么 · 走到哪儿 · 还剩多久**，收场之后换成收场那句话。
///
/// 末一截随此刻在等什么而变：
///
/// | 此刻 | 末一截 |
/// |---|---|
/// | 跑着 | 还剩多久 |
/// | 按过停 | 还剩多久 · 按到哪一级 |
/// | 等答话 | **等你拿主意**（顶掉「还剩多久」） |
///
/// **等答话时不说「还剩多久」**：那一刻横条一动不动，剩下的时间由用户拿主意的快慢决定，
/// 报一个数出来说的就成了「用户还要想多久」。它同样顶掉按停那一级——等答话是此刻更要紧的
/// 那一件（按过的停要等答完话才继续作数），与从前那一格逐字相同。
fn title(live: &Live, pressed: Instruction, deciding: bool) -> Painted {
    if live.ended() {
        return ended_title(live);
    }
    let overall = live.overall();
    let tail = match (deciding, stopping_name(pressed)) {
        (true, _) => DECIDING.to_owned(),
        (false, Some(name)) => format!("{} · {name}", left_clause(overall.left)),
        (false, None) => left_clause(overall.left),
    };
    Painted::plain(format!(
        "{} · 第 {}/{} 卷 · {tail}",
        run_name(live.mode()),
        overall.volume,
        overall.volumes,
    ))
}

/// 这一趟是什么。两个词与屏底那两个键同一批（[`START_KEYS`]，`CONTEXT.md` 的《会话》：试算）。
///
/// **照 [`Live::mode`] 说的走，不另开一个开关**：试算答出第一个继续之后它就是执行了——
/// 那一卷真写了出去，而结论行与出事行说的正是「写出来的是什么样」。
fn run_name(mode: RunMode) -> &'static str {
    match mode {
        RunMode::DryRun => "试算",
        RunMode::Process => "执行",
    }
}

/// 「还剩多久」那一截。一步都还没走时答不出来（见 `Overall::left`），那时不编一个数。
fn left_clause(left: Option<Duration>) -> String {
    left.map_or_else(
        || "还剩 —".to_owned(),
        |left| format!("还剩约 {}", spell(left)),
    )
}

/// 收场之后的抬头。
///
/// 没做成那一句照库那一侧的原话（拒绝执行是一种，那条线程恐慌了是另一种）；
/// 做成了那一种照 [`crate::render::outcome`]——「按停停在半路」与「点名的卷都走过了」
/// 的分别在 `Report::outcome` 上，措辞跟报告那一套走，会话不另编一句。
///
/// 「用了」那个数收场之后就定住了（见 [`Live::overall`]）：它是库交出来的那一个，
/// 扣掉了在决策点上等人的那几分钟。
fn ended_title(live: &Live) -> Painted {
    match live.undone() {
        // **拒绝执行是「出事」那一档**（spec 的《语义色》）：错在这一趟的参数上，
        // 换一个卷不会变好，而这一句是屏上唯一说得出它的地方。
        // 「没做成」三个字就在这一句里——颜色不是唯一载体（见 [`super::paint`]）。
        Some(said) => Painted::new(format!("这一趟没做成：{said}"), Tone::Trouble),
        None => Painted::plain(format!(
            "收场 {} · {} 卷 · 用了 {}",
            crate::render::outcome(live.report().outcome),
            live.report().volumes.len(),
            spell(live.overall().elapsed),
        )),
    }
}

/// 全局那一行：**走了几步、已用多久**。
///
/// 卷数与剩余时间在抬头上，这里不再说第二遍——两条横条与摘要合成一块要修的毛病之一
/// 就是同一个数在顶上出现两次。**收场之后「已用」也让给抬头**：那时它在抬头上叫
/// 「用了」，是同一个数（见 [`ended_title`]），摆两处就是两份措辞。
///
/// 步数出自开工那条事件（`RunStarted`），而它是**预扫**算出来的（03 号票）。
/// 预告的步数是**上界**不是承诺（`CONTEXT.md` 的《进度》）：幂等命中的卷提前收摊，
/// 那一截由 [`Live::finish_volume`] 结清。
fn overall_row(live: &Live) -> String {
    let overall = live.overall();
    let walked = format!(
        " 总体 {} {}/{} 步",
        bar(overall.walked, overall.steps),
        overall.walked,
        overall.steps,
    );
    if live.ended() {
        return walked;
    }
    format!("{walked} · 已用 {}", spell(overall.elapsed))
}

/// 当前卷那一行：**在走哪一卷的哪一遍**，以及这一遍走到第几步。
///
/// 「在走哪一遍」只有 `PassStarted` 答得出（命令行那一路当下没有去处，见 `crate::Bar`）。
/// 非说不可，是因为三遍的性质完全不同：幂等那一道只读不写，第一遍碰像素，
/// 第二遍才往盘上写字节——「跑到一半停下来会留下什么」全看它停在哪一遍。
///
/// **卷与卷之间这一行是空的，行不撤**：编一条横条上去只会让人以为它卡住了，
/// 而撤掉那一行会让下面的报告每过一卷跳一格。
///
/// **收场之后整行不在**：那时再也没有「本卷」可说，那一行让给报告区
/// （与出事行同一条规矩）。这一撤不会让屏跳——这一趟已经走完，这一块不会再变。
fn volume_row(live: &Live) -> Option<String> {
    if live.ended() {
        return None;
    }
    Some(live.walking().map_or_else(String::new, walking_line))
}

/// **卷名与在走哪一遍摆在横条前面**，与从前那一格逐字同序。
///
/// 横条有 [`BAR_WIDTH`] 加两个方括号那么宽，摆在前面就会把这两样顶到 80 列的屏外——
/// 那一档上主区只有 30 列（`super::MAIN_MIN_WIDTH`），而「在跑哪一卷的哪一遍」
/// 屏上再没有第二处说得出（`p1-session/09` 的验收）。两行的横条因此对不齐，
/// 那是认下的代价：对齐是好看，这两样是内容。
fn walking_line(walking: &Walking) -> String {
    // 卷名怎么取只有一处：命令行那条横条印的是同一个（`crate::Bar::start`）。
    let name = crate::render::volume_name(&walking.volume);
    format!(
        " 本卷 {name} · {} {} {}/{} 步",
        pass_name(walking.pass),
        bar(walking.walked, walking.steps),
        walking.walked,
        walking.steps,
    )
}

/// 在走哪一遍。三段与 `VolumeTiming` 的三段是同一条分界线（`CONTEXT.md` 的《进度》）。
///
/// `_` 那一支不是遗漏：[`Pass`] 非穷尽，多一遍不该逼着这里跟着改。
fn pass_name(pass: Option<Pass>) -> &'static str {
    match pass {
        // 开卷之后、第一条 `PassStarted` 到达之前：打开容器、列成员，还没走进任何一遍。
        None => "开卷",
        Some(Pass::Fingerprint) => "幂等这一道",
        Some(Pass::First) => "第一遍",
        Some(Pass::Second) => "第二遍",
        Some(_) => "这一遍",
    }
}

/// 结论行：**这一趟到底怎么样**——屏上从前没有一处答得出它。
///
/// **内容随这一趟是什么而变，而「是什么」手上已经有**（[`Live::mode`]），不必多一个开关：
///
/// - **试算**只算不写，交出来的是一份判定：这一行因此给**判定分布**
///   （`6 卷 2bit+FS · 2 卷 4bit+FS`）。
/// - **执行**真写了出去：这一行因此给**完成与跳过各几卷**。
///
/// 数的是**收摊了的卷**（[`Live::report`] 上那一列），与报告末尾那几小结同一份数据。
/// 隔离的卷算进「完成」——它是**处理过**的卷，交出来了，只是带着坏页；
/// 「它出了事」由[出事行](trouble_row)说，两处不许各说各的。
///
/// **一卷收摊的都没有时两支都给一个破折号**：那时没有分布、也没有完成与跳过可说，
/// 而编一个「0 卷」是在说一件没发生的事。拒绝执行的那一趟走的正是这一支——
/// 它一步都没开工，而抬头已经说了它没做成。
fn settled_row(live: &Live) -> String {
    let report = live.report();
    let (label, said) = match live.mode() {
        RunMode::DryRun => ("判定", verdict_spread(report)),
        RunMode::Process => ("完成", finished_and_skipped(report)),
    };
    format!(" {label} {said}")
}

/// 完成与跳过各几卷。隔离的卷算进「完成」（见 [`settled_row`]）。
fn finished_and_skipped(report: &Report) -> String {
    if report.volumes.is_empty() {
        return NOTHING_SETTLED.to_owned();
    }
    let skipped = report
        .volumes
        .iter()
        .filter(|volume| volume.skipped())
        .count();
    format!("{} 卷 · 跳过 {skipped} 卷", report.volumes.len() - skipped)
}

/// 一卷都还没收摊时结论行说的那一个字（见 [`settled_row`]）。
const NOTHING_SETTLED: &str = "—";

/// 判定分布：**哪一档定了几卷**，多的排在前面。
fn verdict_spread(report: &Report) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for volume in &report.volumes {
        *counts.entry(base_name(volume)).or_default() += 1;
    }
    if counts.is_empty() {
        return NOTHING_SETTLED.to_owned();
    }
    let mut spread: Vec<(String, usize)> = counts.into_iter().collect();
    // 多的排在前面；一样多的按名字排，屏上因此不会因为收摊次序而跳位置。
    spread.sort_by(|(left, one), (right, two)| two.cmp(one).then_with(|| left.cmp(right)));
    spread
        .into_iter()
        .map(|(name, count)| format!("{count} 卷 {name}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 一卷的**基准档**该怎么称呼。
///
/// 四支照卷级判定说的写，不编第二套说法（spec 的《卷表》：`--per-page` 与覆盖顶掉判定的
/// 那两种写「逐页」「覆盖 4bit」）。一张灰度页都没有的卷（只装着彩页的、整卷全失败的）
/// 连候选都没有，那一支自成一档——把它并进别的哪一档都是在说一件没发生的事。
fn base_name(volume: &VolumeReport) -> String {
    match volume.verdict {
        None => "无判定".to_owned(),
        Some(VolumeVerdict::Skipped { .. }) => "跳过".to_owned(),
        Some(VolumeVerdict::PerPage) => "逐页".to_owned(),
        Some(VolumeVerdict::Override(candidate)) => format!("覆盖 {candidate}"),
        Some(VolumeVerdict::Envelope(envelope)) => envelope.base.to_string(),
    }
}

/// 出事行：**要注意的那几件**，一条都没有时整行不出现——那一行让给报告区。
///
/// 与结论行同一条：内容随这一趟是什么而变。
///
/// - **试算**给的是判定上要注意的：特例页几张 · 宽溢出几页 · 几何门不成立几卷。
/// - **执行**给的是盘上出的事：隔离几卷 · 失败几页 · 卷级失败几卷。
///
/// 数与措辞**与报告末尾那几小结对得上**（`crate::render::tail` 的隔离与卷级失败两小结）：
/// 同一份报告，两处的数与那几个词一模一样，
/// `the_trouble_row_counts_what_the_report_tail_counts` 两头对着问。
/// 它们没有合成一个函数——那两小结是**成句的**（隔离那一句还要说清失败页在输出里是什么样），
/// 而这一行是一串数；共用的话得先把那句话拆成词，而措辞只许有一处出处（ADR 0016）。
///
/// **失败页数的是收摊了的卷**（`Report::failures`），因此比报告区晚一整卷——
/// 出现的当场那几条在报告区（`crate::render::failing_pages`），停车场 Q148 记着这一笔。
///
/// **这一行只有一种[语义](Tone)**：它列着的那几件分属两档（隔离要注意、失败页与卷级失败
/// 是出事），而取的是最重的那一个——理由见函数里那条注释。
fn trouble_row(live: &Live) -> Option<Painted> {
    let report = live.report();
    let said: Vec<Painted> = match live.mode() {
        RunMode::DryRun => vec![
            count(outliers(report), "特例页", "张", Tone::Caution),
            count(
                report.wider_than_the_panel().count(),
                "宽溢出",
                "页",
                Tone::Caution,
            ),
            count(broken_gates(report), "几何门不成立", "卷", Tone::Caution),
        ],
        RunMode::Process => vec![
            count(isolated(report), "隔离", "卷", Tone::Caution),
            count(report.failures().count(), "失败", "页", Tone::Trouble),
            count(report.failed_volumes.len(), "卷级失败", "卷", Tone::Trouble),
        ],
    }
    .into_iter()
    .flatten()
    .collect();
    // **这一行只有一种颜色，取列着的那几件里最重的那一种**（[`Tone`] 的 `Ord` 就是为它派生的）：
    // 隔离要注意、失败页与卷级失败是出事，三件同时在场时这一行是红的。
    // 分成三段各上各的色也行得通，但「一眼看出这一趟出没出事」问的是**有没有红**，
    // 而一行里掺着黄的红读不出重点。行首「出事」两个字接住这个颜色。
    //
    // 一件都没有时它答 `None`——那正是「整行不出现」，与从前那一格逐字同义。
    let tone = said.iter().map(|one| one.tone).max()?;
    let listed: Vec<&str> = said.iter().map(|one| one.text.as_str()).collect();
    Some(Painted::new(format!(" 出事 {}", listed.join(" · ")), tone))
}

/// 「几件什么」那一小截，连同它是哪一档[语义](Tone)。**零就一个字都不说**——
/// 出事行只列真出了的事。
///
/// 交出来的是 [`Painted`] 而不是一对裸值：那一对里哪一半是哪一半在调用处看不出来
/// （理由与 [`Painted`] 自己的文档同一条）。这里的一「行」是行上的一小截，
/// 而语义正是逐小截给的——整行取它们里面最重的那一个。
fn count(many: usize, what: &str, unit: &str, tone: Tone) -> Option<Painted> {
    (many > 0).then(|| Painted::new(format!("{what} {many} {unit}"), tone))
}

/// 这一趟摘出去单独定档的特例页共几张（`Envelope::outlier_pages` 逐卷相加）。
fn outliers(report: &Report) -> usize {
    report
        .volumes
        .iter()
        .filter_map(|volume| match volume.verdict {
            Some(VolumeVerdict::Envelope(envelope)) => Some(envelope.outlier_pages),
            _ => None,
        })
        .sum()
}

/// 这一趟有几卷**出现过几何门不成立的页**。
///
/// 数的是**卷**不是页（spec 的《总览块》：几何门不成立几卷）：门逐页判，而「这一卷该不该
/// 换个 profile」是一卷一卷问的。
fn broken_gates(report: &Report) -> usize {
    report
        .volumes
        .iter()
        .filter(|volume| volume.outside_the_gate().next().is_some())
        .count()
}

/// 这一趟有几卷被**隔离**。判据只有一条：有没有失败页（`VolumeReport::isolated`）。
fn isolated(report: &Report) -> usize {
    report
        .volumes
        .iter()
        .filter(|volume| volume.isolated())
        .count()
}

/// 一条横条。样子与命令行那两条一致：`=` 是走过的，`>` 是当前这一格，空白是还没走的。
///
/// 预告的步数是零（还没开工、或者这一卷一步都不走）时整条是空的：那时没有比例可画，
/// 而画一个「刚起步」的箭头是编的。
fn bar(done: u64, total: u64) -> String {
    let filled = (total > 0).then(|| {
        // 先乘后除：先除的话，步数比条格数少的小卷会被整个抹成 0。
        done.min(total) * BAR_WIDTH / total
    });
    let mut text = String::with_capacity(BAR_WIDTH as usize + 2);
    text.push('[');
    for at in 0..BAR_WIDTH {
        text.push(match filled.map(|filled| (at.cmp(&filled), filled)) {
            Some((Ordering::Less, _)) => '=',
            Some((Ordering::Equal, filled)) if filled < BAR_WIDTH => '>',
            _ => ' ',
        });
    }
    text.push(']');
    text
}

/// 一段时长：`42s`、`6m40s`、`1h06m`。
///
/// 只留两级：秒以下在一趟几十分钟的任务里没有意义，而三级读起来要数位数。
///
/// **卷表耗时那一列走的也是它**（`super::table`）：同一屏上两个时长长得不一样，
/// 读的人就得先分辨一遍这是哪一种写法。
pub(super) fn spell(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    match (seconds / 3600, (seconds % 3600) / 60, seconds % 60) {
        (0, 0, second) => format!("{second}s"),
        (0, minute, second) => format!("{minute}m{second:02}s"),
        (hour, minute, _) => format!("{hour}h{minute:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::super::footer::running_prompt;
    use super::super::main_pane;
    use super::super::probe::{
        a_run_in_flight, main_snapshot, same_screen, screen, snapshot, tight,
    };
    use super::*;
    use crate::session::live::{Resuming, Volume, fixture};
    use crate::session::state::{Expansion, Key, Session};
    use tonefit::{
        BitDepth, Candidate, Dither, Envelope, GeometryGate, PageBranch, PageOutcome, RunOutcome,
        Size,
    };

    /// 总览块**单独**一张快照：主区最上面那一块，一个框。
    ///
    /// 只钉这一块，是因为本票做的就是它——把报告区一起钉进来，改一句报告措辞就要
    /// 重录这四张（`main_snapshot` 那两张钉的正是主区整块，两者分工不同）。
    ///
    /// 高度照它自己说的取（[`Overview::height`]）：出事行在不在场，快照上一眼看得出。
    fn block(live: &Live, pressed: Instruction, deciding: bool) -> String {
        const WIDE: u16 = 96;

        let top = overview(Some(live), pressed, deciding);
        let height = top.height();
        snapshot(
            |frame| frame.render_widget(top.draw(), frame.area()),
            WIDE,
            height,
        )
    }

    /// 一趟**试算跑完了**：三卷各是一档——幂等命中一卷、4bit 一卷、2bit+FS 一卷，
    /// 而末一卷有两张特例页、一张宽溢出的页、一页几何门不成立。
    ///
    /// 判定分布与试算那一副的出事行都要它：结论行按档分组，出事行数的是
    /// 特例页 · 宽溢出 · 几何门不成立三样。
    fn a_dry_run_that_finished() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::GoesOn);
        live.run_started(3, 3000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 180));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.volume_finished(&fixture::processed_volume("卷二", None));
        live.volume_started(Path::new("库/卷三"), 1000);
        live.volume_finished(&a_volume_worth_a_second_look("卷三"));
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Completed;
        report.elapsed = Duration::from_secs(400);
        live.returned(Ok(report));
        live
    }

    /// 一份**每一样都要注意一下**的卷报告：两张特例页、一张宽溢出的页、一页几何门不成立。
    ///
    /// 照 [`fixture::processed_volume`] 改出来而不是另搓一份：变的只有这三样，
    /// 别的一格不动，快照上因此看得出这一行说的是哪几个数。
    fn a_volume_worth_a_second_look(name: &str) -> VolumeReport {
        let mut volume = fixture::processed_volume(name, None);
        volume.verdict = Some(VolumeVerdict::Envelope(Envelope {
            base: Candidate::new(BitDepth::Two, Dither::FloydSteinberg),
            driver: 0,
            body_pages: 8,
            outlier_pages: 2,
            raised_pages: 0,
        }));
        // 宽出面板（`kobo-libra-2` 是 1264 列宽）：那一页要横着平移才读得全。
        volume.pages[0].size = Size::new(1400, 1680);
        if let PageOutcome::Whole(page) = &mut volume.pages[0].outcome
            && let PageBranch::Gray { gate, .. } = &mut page.branch
        {
            *gate = GeometryGate::Broken;
        }
        volume
    }

    /// 一趟**执行跑完了，一件事都没出**：两卷都收了摊，横条走到头。
    fn a_run_that_finished_clean() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 180));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.volume_finished(&fixture::processed_volume("卷二", None));
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Completed;
        report.elapsed = Duration::from_secs(400);
        live.returned(Ok(report));
        live
    }

    /// **快照：试算跑完。** 抬头是收场那句话，结论行给判定分布，出事行给要注意的三样。
    #[test]
    fn the_overview_of_a_dry_run_that_finished() {
        same_screen(
            &block(&a_dry_run_that_finished(), Instruction::Continue, false),
            A_DRY_RUN_THAT_FINISHED,
        );
    }

    /// 见 [`the_overview_of_a_dry_run_that_finished`]。
    const A_DRY_RUN_THAT_FINISHED: &str = r#"
"┌收场 点名的卷都走过了 · 3 卷 · 用了 6m40s─────────────────────────────────────────────────────┐"
"│ 总体 [==============================] 3000/3000 步                                           │"
"│ 判定 1 卷 2bit+FS · 1 卷 4bit · 1 卷 跳过                                                    │"
"│ 出事 特例页 2 张 · 宽溢出 1 页 · 几何门不成立 1 卷                                           │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：执行跑着。** 抬头给「这一趟是什么 · 走到哪儿 · 还剩多久」，
    /// 结论行给完成与跳过，出事行给隔离与失败页。
    #[test]
    fn the_overview_of_a_run_in_flight() {
        same_screen(
            &block(&a_run_in_flight(true), Instruction::Continue, false),
            A_RUN_IN_FLIGHT,
        );
    }

    /// 见 [`the_overview_of_a_run_in_flight`]。
    const A_RUN_IN_FLIGHT: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s───────────────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                              │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                             │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"│ 出事 隔离 1 卷 · 失败 1 页                                                                   │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：执行完了，一件事都没出。** 出事行整行不出现——这一块因此只有五行。
    #[test]
    fn the_overview_of_a_run_that_finished_clean() {
        same_screen(
            &block(&a_run_that_finished_clean(), Instruction::Continue, false),
            A_RUN_THAT_FINISHED_CLEAN,
        );
    }

    /// 见 [`the_overview_of_a_run_that_finished_clean`]。
    const A_RUN_THAT_FINISHED_CLEAN: &str = r#"
"┌收场 点名的卷都走过了 · 2 卷 · 用了 6m40s─────────────────────────────────────────────────────┐"
"│ 总体 [==============================] 2000/2000 步                                           │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：按了收尾。** 按到哪一级挂在抬头上，横条照旧往前走。
    #[test]
    fn the_overview_of_a_run_that_was_asked_to_finish() {
        same_screen(
            &block(&a_run_in_flight(false), Instruction::Finish, false),
            A_RUN_ASKED_TO_FINISH,
        );
    }

    /// 见 [`the_overview_of_a_run_that_was_asked_to_finish`]。
    const A_RUN_ASKED_TO_FINISH: &str = r#"
"┌执行 · 第 3/3 卷 · 还剩约 3m20s · 收尾中──────────────────────────────────────────────────────┐"
"│ 总体 [==================>           ] 3000/5000 步 · 已用 5m00s                              │"
"│ 本卷 卷三 · 第二遍 [==========>                   ] 1000/3000 步                             │"
"│ 完成 1 卷 · 跳过 1 卷                                                                        │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **结论行与出事行的内容随这一趟是什么而变**（票面第二条），而「是什么」手上已经有
    /// （[`Live::mode`]），不必多一个开关。
    ///
    /// 两副各问一遍：试算那一副给判定分布与要注意的三样，执行那一副给完成／跳过
    /// 与盘上出的三样。两副的字**互不出现在对方身上**——同一行两种内容，
    /// 混了就等于没分。
    #[test]
    fn the_settled_and_trouble_rows_say_different_things_in_each_mode() {
        let dry = block(&a_dry_run_that_finished(), Instruction::Continue, false);
        let real = block(&a_run_in_flight(true), Instruction::Continue, false);

        for said in [
            "判定 ",
            "2bit+FS",
            "特例页 2 张",
            "宽溢出 1 页",
            "几何门不成立 1 卷",
        ] {
            assert!(dry.contains(said), "试算那一副少了「{said}」：{dry}");
            assert!(!real.contains(said), "执行那一副不该有「{said}」：{real}");
        }
        for said in ["完成 1 卷", "隔离 1 卷", "失败 1 页"] {
            assert!(real.contains(said), "执行那一副少了「{said}」：{real}");
            assert!(!dry.contains(said), "试算那一副不该有「{said}」：{dry}");
        }
    }

    /// 出事行那几个数与措辞，**与报告末尾那几小结说的是同一件事**：同一份报告，
    /// 两处的数与那几个词一模一样。
    ///
    /// 两头对着问而不是合成一个函数：那两小结是**成句的**（隔离那一句还要说清失败页在
    /// 输出里是什么样），而这一行是一串数——共用得先把句子拆成词，而措辞只许有一处出处
    /// （ADR 0016）。这一条是那一处出处的**闸门**：改了一头，它当场红。
    #[test]
    fn the_trouble_row_counts_what_the_report_tail_counts() {
        let mut live = a_run_in_flight(true);
        live.volume_failed(Path::new("库/卷四"), "卷根不在了");
        let row = trouble_row(&live).expect("这一趟出了事");
        let tail = crate::render::tail(live.report());

        for said in ["隔离 1 卷", "失败 1 页", "卷级失败 1 卷"] {
            assert!(
                row.text.contains(said),
                "出事行少了「{said}」：{}",
                row.text
            );
            assert!(tail.contains(said), "报告末尾那几小结不这么说了：{tail}");
        }
    }

    /// **出事行只有一种颜色，取它列着的那几件里最重的那一种**（spec 的《语义色》）。
    ///
    /// 试算那一副列的三样（特例页 · 宽溢出 · 几何门不成立）都是「注意」；
    /// 执行那一副里隔离是「注意」而失败页是「出事」，一行只上得了一种色，取重的那一个。
    /// 行首「出事」两个字接住这个颜色——颜色不是唯一载体（见 [`super::paint`]）。
    #[test]
    fn the_trouble_row_takes_the_most_serious_tone_it_lists() {
        assert_eq!(
            trouble_row(&a_dry_run_that_finished())
                .expect("这一趟有要注意的")
                .tone,
            Tone::Caution
        );
        assert_eq!(
            trouble_row(&a_run_in_flight(true))
                .expect("这一趟出了事")
                .tone,
            Tone::Trouble
        );
    }

    /// **出事行一条都没有时整行不出现，那一行让给下面的报告**（票面第三条，
    /// 与报告末尾那几小结同一条规矩）。
    #[test]
    fn the_trouble_row_is_gone_when_nothing_went_wrong_and_the_report_takes_that_row() {
        let quiet = a_run_in_flight(false);
        let noisy = a_run_in_flight(true);

        assert_eq!(
            overview(Some(&quiet), Instruction::Continue, false).height(),
            OVERVIEW_HEIGHT - 1,
            "没出事还画着出事行"
        );
        assert_eq!(
            overview(Some(&noisy), Instruction::Continue, false).height(),
            OVERVIEW_HEIGHT
        );
        assert!(!block(&quiet, Instruction::Continue, false).contains("出事"));

        // **收场之后当前卷那一行也让出去**：那时再没有「本卷」可说。
        assert_eq!(
            overview(
                Some(&a_run_that_finished_clean()),
                Instruction::Continue,
                false
            )
            .height(),
            OVERVIEW_HEIGHT - 2,
            "跑完了还占着「本卷」那一行"
        );

        // 让出来的那一行**归报告区**：同一屏上，没出事那一副的报告那一格高一行。
        let opens_at = |live: &Live| {
            main_snapshot(live, 96, 30)
                .lines()
                .position(|row| row.contains("┌报告"))
                .expect("报告那一格在屏上")
        };
        assert_eq!(opens_at(&quiet) + 1, opens_at(&noisy), "那一行没让出去");
    }

    /// **总览块钉住：下面的报告怎么滚，它一行都不动**（票面第一条）。
    ///
    /// `p1-session/09` 那条「三段各占一格，免得报告长起来把进度条顶出屏外」由这一条接住：
    /// 两块各占主区的一格（[`main_pane`]），报告在它自己那一格里滚。
    ///
    /// 翻的是**展开**那一副——默认那一副的滚动量是算出来的（恒停在底上），
    /// 而按得动的只有展开着的那一份。
    #[test]
    fn the_overview_block_stays_put_while_the_report_scrolls() {
        let live = a_run_in_flight(true);
        let alone = block(&live, Instruction::Continue, false);
        let rows = alone.lines().count();
        let mut session = Session::new();
        session.expand(Expansion::new(Volume::Settled(0), 0));

        let mut seen: Vec<String> = Vec::new();
        for _ in 0..4 {
            let shot = snapshot(
                |frame| main_pane(frame, frame.area(), &mut session, Some(&live)),
                96,
                20,
            );
            let (top, report) = shot.split_at(
                shot.match_indices('\n')
                    .nth(rows - 1)
                    .expect("屏比这一块高")
                    .0
                    + 1,
            );
            same_screen(top.trim_end_matches('\n'), &alone);
            seen.push(report.to_owned());
            session.press(Key::Down);
        }
        assert!(
            seen.iter().any(|report| report != &seen[0]),
            "报告根本没滚，这一条什么都没证明"
        );
    }

    /// 收场之后**抬头改成收场那句话**，报告末尾那几小结也补上了（票面第五条）。
    ///
    /// 「用了」那个数是**库交出来的那一个**——它扣掉了在决策点上等人的那几分钟
    /// （停车场 Q41），而不是会话接着读自己那块表（那一条钉在 `Live` 那一侧的
    /// `the_elapsed_time_stops_moving_once_the_run_is_over` 上）。
    #[test]
    fn the_overview_title_says_how_the_run_ended() {
        let mut live = a_run_in_flight(false);
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Completed;
        report.elapsed = Duration::from_secs(400);
        live.returned(Ok(report));

        let snapshot = main_snapshot(&live, 78, 18);

        assert!(snapshot.contains("收场"), "{snapshot}");
        assert!(snapshot.contains("点名的卷都走过了"), "{snapshot}");
        // 走完了的那一趟抬头不上色：**四种里有一种是「不上色」**，而它是屏上多数。
        assert_eq!(ended_title(&live).tone, Tone::Plain);
        // 库交出来的那一个，不是会话自己那块表上的五分钟。
        assert!(snapshot.contains("用了 6m40s"), "{snapshot}");
        // 收场之后不再说「还剩多久」：这一趟已经走完了。
        assert!(!snapshot.contains("还剩"), "{snapshot}");
    }

    /// 拒绝执行的那一趟：会话不退出，把那句话画在总览块的抬头上，用户当场改。
    #[test]
    fn a_refused_run_says_why_on_the_overview_title() {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.returned(Err(anyhow::anyhow!("处理范围为空：至少点名一个卷")));

        let snapshot = main_snapshot(&live, 78, 10);

        assert!(snapshot.contains("没做成"), "{snapshot}");
        assert!(snapshot.contains("处理范围为空"), "{snapshot}");
        // **拒绝执行是「出事」那一档**（spec 的《语义色》），而「没做成」三个字
        // 就在同一句里——颜色不是唯一载体。
        assert_eq!(ended_title(&live).tone, Tone::Trouble);
        // 一步都没开工的那一趟不编一个「完成 0 卷」：结论行给的是那个破折号，
        // 与试算那一支同一条规矩（见 [`settled_row`]）。
        assert!(snapshot.contains("完成 —"), "{snapshot}");
        assert!(!snapshot.contains("完成 0 卷"), "{snapshot}");
    }

    /// **按停按到哪一级，总览块的抬头上就看得出来**（停车场 Q71）。
    ///
    /// 屏底那两行说的是同一件事，措辞同一个出处（[`stopping_name`]）；
    /// 摆在抬头上是因为眼睛盯着横条的人不会往下扫一行。
    #[test]
    fn the_overview_title_says_that_the_run_is_stopping() {
        let mut session = Session::new();
        session.run_started();
        assert!(
            tight(&screen(&mut session, None, 120, 40)).contains("┌总览"),
            "没跑过时抬头就是这一块的名字"
        );

        let live = a_run_in_flight(false);
        session.press(Key::Char('s'));
        let finishing = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(
            finishing.contains(&tight("还剩约 3m20s · 收尾中")),
            "{finishing}"
        );

        session.press(Key::Char('s'));
        let aborting = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(
            aborting.contains(&tight("还剩约 3m20s · 中止中")),
            "{aborting}"
        );

        // 抬头与屏底那一行说的是同一个词：措辞只有一处。
        for pressed in [Instruction::Finish, Instruction::Abort] {
            let name = stopping_name(pressed).expect("按过的那两级都有名字");
            assert!(
                running_prompt(pressed, None).keys.contains(name),
                "屏底那一行没用 stopping_name：{pressed:?}"
            );
        }
        assert_eq!(stopping_name(Instruction::Continue), None, "没按过没有名字");
    }

    /// 横条的两头：一步没走是空的，走完是满的，总步数为零时不画比例。
    ///
    /// **宽度与命令行那两条同一个出处**（票面第六条）：这一层不许再长一个字面量出来。
    #[test]
    fn the_bar_fills_from_empty_to_full() {
        assert_eq!(BAR_WIDTH, crate::BAR_WIDTH as u64, "横条宽度长出了第二份");
        assert_eq!(bar(0, 100), format!("[>{}]", " ".repeat(29)));
        assert_eq!(bar(100, 100), format!("[{}]", "=".repeat(30)));
        assert_eq!(bar(0, 0), format!("[{}]", " ".repeat(30)));
        // 步数比条格数少的小卷不该被抹成 0。
        assert!(bar(1, 3).starts_with("[========="));
    }

    /// 时长两级就够：秒、分秒、时分。
    #[test]
    fn a_duration_is_spelled_with_two_units() {
        assert_eq!(spell(Duration::from_secs(0)), "0s");
        assert_eq!(spell(Duration::from_secs(42)), "42s");
        assert_eq!(spell(Duration::from_secs(400)), "6m40s");
        assert_eq!(spell(Duration::from_secs(3960)), "1h06m");
    }

    /// 屏矮下来时**先让报告区，总览块不砍**（spec 的《窄终端》）：
    /// [`super::super::MAIN_MIN_HEIGHT`] 留的正是这一块最高那几行加报告区最少那三行。
    #[test]
    fn the_overview_is_the_last_thing_the_main_pane_gives_up() {
        assert_eq!(
            OVERVIEW_HEIGHT,
            overview(Some(&a_run_in_flight(true)), Instruction::Continue, false).height(),
            "最高那一副与留位子的那个数对不上"
        );
    }
}
