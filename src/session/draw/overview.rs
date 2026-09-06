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
//! 两行的内容随这一趟是[什么](Live::started_as)而变。票面写的是「三个框九行」，与屏上对不上，
//! 停车场 Q145 记着那一条。
//!
//! 只读那一趟边跑边攒的那一份（[`Live`]），一个字都不在这里重编：卷名走
//! [`crate::render::volume_name`]、收场那一句走 [`crate::render::outcome`]、
//! 按停按到的那一级走 [`super::footer::stopping_name`]。横条画多宽与命令行那两条
//! 同一个出处（[`BAR_WIDTH`]），这一格摆不下时让到几格由让位那一处答
//! （[`super::yielding::bar_width`]）。
//!
//! # 这一块答「此刻」，报告末尾那几小结答「已定案」
//!
//! 两处的数**在一卷跑到一半时故意不一样，而各自都对**——它们答的不是同一个问题：
//!
//! | 屏上那一处 | 答的是 | 数的是 |
//! |---|---|---|
//! | [出事行](trouble_row) | **此刻**出了多少事 | 收摊了的那几卷，加上当前这一卷已经报过的那几条（[`Live::failures_so_far`]） |
//! | 报告末尾那几小结（`crate::render::tail`） | **已定案**的那几卷出了多少事 | `Report` 上那一列，一卷不收摊就一个字不算 |
//!
//! 一卷收摊之后两个数又相等。**这不是一个数摆两处**：出事行钉在屏上是为了答
//! 「这一趟此刻怎么样」，而它从前跟着报告走，比同一屏的报告区晚一整卷——
//! 屏上已经写着「失败页……JPEG 数据截断」，专门答这件事的那一行却一个字都不说
//! （停车场 Q148）。
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

use super::footer::stopping_name;
use super::paint::{Painted, Tone};
use crate::session::live::{Live, Walking};

/// 总览块**最高**几行：四行正文加上下两条边（跑着、而且出了事的那一副）。
///
/// 矮下去的两条见 [`Overview::height`]：出事行不在场少一行，收场之后当前卷那一行也不在。这个数只用来给主区留位子
/// （[`super::yielding::MAIN_MIN_HEIGHT`]）——**屏矮下来时先让报告区，总览块不砍**
/// （spec 的《窄终端》：宁可少画表，不少画总览）。
pub(super) const OVERVIEW_HEIGHT: u16 = 6;

/// 一条横条**摆得开时**画多宽。**与命令行那两条同一个出处**（`crate::BAR_WIDTH`）：
/// 两处的横条长得一样，读的人不必重新认一遍。
///
/// 这一格摆不下它时收窄到几格**不在这里判**（[`super::yielding::bar_width`]）：
/// 收窄是让位，而让位的次序只有那一处。这一层因此仍旧只有这一个宽度。
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
/// | 出事行 | **此刻**出了多少事，**一条都没有时整行不出现** |
pub(super) struct Overview {
    /// 边框上那一行。**没做成那一趟它是红的**（见 [`ended_title`]）。
    title: Painted,
    /// 框里那一到四行。各行的[语义](Tone)各自算（出事行见 [`trouble_row`]）。
    rows: Vec<Painted>,
    /// 这一块占多宽。**算与画同一个数**：横条要照它收窄（[`with_a_bar`]），
    /// 而摆不下的那几行要照它省略（[`Overview::draw`]）——两处各拿一个宽度的话，
    /// 算出来的横条会摆进另一副宽度的格子里。
    width: u16,
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
    ///
    /// **摆不下的一律从中间省略，不由终端库硬截**（截法只有 [`super::yielding`] 一处，
    /// 停车场 Q147、Q169）：抬头那一头 `还剩约 3m20s` 硬截成 `还剩约 3m` 读起来是一个
    /// 完整而偏小的估计，正文那一头 `已用 5m00s` 硬截成 `已用 5m` 一模一样，
    /// 而屏上都没有一处痕迹说它被截过。抬头走 [`title`](super::yielding::title)
    /// （两个角各占一格），正文那几行走 [`pinned`](super::yielding::pinned)（一格边框都不占）。
    ///
    /// 正文里的横条**不会被省略号截着**：它在算的时候就照这一格收窄过了
    /// （[`with_a_bar`]），摆不下的那一档整条不画。
    pub(super) fn draw(self) -> Paragraph<'static> {
        let title = Painted::new(
            super::yielding::title(&self.title.text, self.width),
            self.title.tone,
        );
        let block = Block::default().borders(Borders::ALL).title(title.line());
        let inside = self.width.saturating_sub(2);
        let rows: Vec<Line<'static>> = self
            .rows
            .iter()
            .map(|row| Painted::new(super::yielding::pinned(&row.text, inside), row.tone).line())
            .collect();
        Paragraph::new(Text::from(rows)).block(block)
    }
}

/// 一趟都还没跑过时这一块里那一句提到的两个键。
///
/// **屏底那一行不用它**：那一行的键出自按键表（`Session::keys_here`），措辞出自
/// [`super::keys::says`]。这一处是「还没跑过」那一句里的一截提示，仍旧手抄——
/// 这一笔连同报告区那一段同样的一句记在停车场 Q190。
const START_KEYS: &str = "t 试算 · x 执行";

/// 算出这一块：抬头一行、正文一到四行。
///
/// **按停按到哪一级、以及等答话那一句都挂在抬头上**（停车场 Q71、`p1-session/14`）：
/// 按下收尾之后横条照旧往前走，而「它在等什么」只写在屏底——眼睛盯着横条的人不会往下
/// 扫一行。抬头摆在**边框**上，一列正文都不占。措辞与屏底那一行同一个出处
/// （[`stopping_name`]）。
///
/// **等答话排在按停那一级之前**：横条这时一动不动，而「它为什么不动」是眼睛盯着这一块的人
/// 第一眼要看到的（按过的停要等答完话才继续作数）。
pub(super) fn overview(
    live: Option<&Live>,
    pressed: Instruction,
    deciding: bool,
    width: u16,
) -> Overview {
    let Some(live) = live else {
        return Overview {
            title: Painted::plain("总览".to_owned()),
            rows: vec![Painted::plain(format!(" 还没跑过。{START_KEYS}"))],
            width,
        };
    };
    let room = width.saturating_sub(2);
    let mut rows = vec![Painted::plain(overall_row(live, room))];
    rows.extend(volume_row(live, room).map(Painted::plain));
    rows.push(Painted::plain(settled_row(live)));
    rows.extend(trouble_row(live));
    Overview {
        title: title(live, pressed, deciding),
        rows,
        width,
    }
}

/// 一行正文：`head` + 横条 + `tail`，**横条摆不下就整条不画**，连它前面那个空格一起。
///
/// 先量不带横条那一副，剩下的格才轮到横条，收窄到几格由让位那一处答
/// （[`super::yielding::bar_width`]）。整条让掉的那一档一个空格都不留——那一行照旧成句，
/// 两个数之间不会多出一格没人解释的空白。
///
/// **量的与画的是同一份文字**：`head` 与 `tail` 拼一次、量一次、用一次，中途不再问第二遍
/// 那一趟走到哪儿了。各问各的话，`59s` 涨成 `1m00s` 的那一瞬画出来的那一行会比量的时候
/// 宽两格，而宽出去的那两格由省略号收——省略号正好落在横条身上，切出半条没有 `]` 的横条，
/// 也就是本票要拦的那一副（同一条道理见 `Live::overall` 那段「一次读表算两个数」，
/// 停车场 Q118）。
///
/// **两行各算各的**：这一块里两条横条一条量整趟、一条量这一卷，本来就是两把尺。
/// 合成一个数要取两行里更紧的那一个，而当前卷那一行的长短随**卷名**变、卷与卷之间
/// 整行还是空的——全局那一条的刻度会因此在换卷的那一刻跳一下，同一个「走了六成」
/// 上一刻十八格、下一刻六格，看着像整趟往回退了（评审提的）。
fn with_a_bar(head: &str, tail: &str, done: u64, total: u64, room: u16) -> String {
    let bare = crate::wrap::width(head).saturating_add(crate::wrap::width(tail));
    // 让给横条的那几格：这一行剩下的，再扣掉它后面那一个空格。
    let left = room.saturating_sub(bare).saturating_sub(1);
    super::yielding::bar_width(BAR_WIDTH, left).map_or_else(
        || format!("{head}{tail}"),
        |width| format!("{head}{} {tail}", bar(done, total, width)),
    )
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

/// 这一趟是什么。两个词与「还没跑过」那一句里那两个键同一批（[`START_KEYS`]，
/// `CONTEXT.md` 的《会话》：试算）。
///
/// **抬头照 [`Live::mode`] 说的走**：试算答出第一个继续之后它就是执行了——那一卷真写了
/// 出去，而这一行答的正是「**此刻**在写没写」，报告抬头（`crate::render::header`）跟的
/// 也是它。
///
/// **底下那两行不跟它走**（[`settled_row`] 与 [`trouble_row`] 问的是 [`Live::started_as`]）：
/// 那两行答的是另一个问题——「这一趟交出来的是什么」，而那件事在决策点上答话前后是同一件。
/// 跟着这一条走的话，答出继续的那一帧屏上会换掉整副内容、并可能矮一行（停车场 Q149）。
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
fn overall_row(live: &Live, room: u16) -> String {
    // **一次读表**：底下那几截共用同一份 `overall`（见 [`with_a_bar`]）。
    let overall = live.overall();
    let steps = format!("{}/{} 步", overall.walked, overall.steps);
    let tail = if live.ended() {
        steps
    } else {
        format!("{steps} · 已用 {}", spell(overall.elapsed))
    };
    with_a_bar(" 总体 ", &tail, overall.walked, overall.steps, room)
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
fn volume_row(live: &Live, room: u16) -> Option<String> {
    if live.ended() {
        return None;
    }
    Some(
        live.walking()
            .map_or_else(String::new, |walking| walking_line(walking, room)),
    )
}

/// **卷名与在走哪一遍摆在横条前面**，与从前那一格逐字同序。
///
/// 横条有 [`BAR_WIDTH`] 加两个方括号那么宽，摆在前面就会把这两样顶到 80 列的屏外——
/// 那一档上主区只有 30 列（`super::yielding::MAIN_MIN_WIDTH`），而「在跑哪一卷的哪一遍」
/// 屏上再没有第二处说得出（`p1-session/09` 的验收）。两行的横条因此对不齐，
/// 那是认下的代价：对齐是好看，这两样是内容。
///
/// **这一行的横条按它自己剩下的格算**（[`with_a_bar`]）：卷名与在走哪一遍长短不定，
/// 而全局那一条的刻度不该跟着换卷跳一下。
fn walking_line(walking: &Walking, room: u16) -> String {
    // 卷名怎么取只有一处：命令行那条横条印的是同一个（`crate::Bar::start`）。
    let name = crate::render::volume_name(&walking.volume);
    let head = format!(" 本卷 {name} · {} ", pass_name(walking.pass));
    let tail = format!("{}/{} 步", walking.walked, walking.steps);
    with_a_bar(&head, &tail, walking.walked, walking.steps, room)
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
/// **内容随这一趟是什么而变，而「是什么」手上已经有**（[`Live::started_as`]），
/// 不必多一个开关：
///
/// - **试算**只算不写，交出来的是一份判定：这一行因此给**判定分布**
///   （`6 卷 2bit+FS · 2 卷 4bit+FS`）。
/// - **执行**真写了出去：这一行因此给**完成与跳过各几卷**。
///
/// **问的是「起手按的哪一个键」，不是「此刻落过盘没有」**（[`Live::mode`]，
/// 抬头那一行走的是它）：后者在决策点上答出第一个继续的那一刻翻面，而这一行与
/// [出事行](trouble_row)会跟着**同一帧里换掉整副内容**——用户刚看完判定、按下 `x`，
/// 他据以拿主意的那份判定分布当场就没了，出事行还可能整行消失、这一块矮一行、
/// 下面的报告整块上移（停车场 Q149）。**这一块因此一趟之内一格不变**。
/// 两处答的不是同一个问题：抬头答「此刻在写没写」，这两行答「这一趟交出来的是什么」。
///
/// 数的是**收摊了的卷**（[`Live::report`] 上那一列），与报告末尾那几小结同一份数据——
/// 出事行那一行不是（它答的是「此刻」，见本模块开头那张表）。隔离的卷算进「完成」：
/// 它是**处理过**的卷，交出来了，只是带着坏页；「它出了事」由出事行说，
/// 这一行一个字都不重复。
///
/// **一卷收摊的都没有时两支都给一个破折号**：那时没有分布、也没有完成与跳过可说，
/// 而编一个「0 卷」是在说一件没发生的事。拒绝执行的那一趟走的正是这一支——
/// 它一步都没开工，而抬头已经说了它没做成。
fn settled_row(live: &Live) -> String {
    let report = live.report();
    let (label, said) = match live.started_as() {
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

/// 出事行：**此刻出了多少事**，一条都没有时整行不出现——那一行让给报告区。
///
/// 与结论行同一条：**要注意的那几件**随这一趟是什么而变，问的同样是
/// [`Live::started_as`]（一趟之内一格不变，理由见 [`settled_row`]）。
///
/// - **试算**给的是判定上要注意的：特例页几张 · 宽溢出几页 · 几何门不成立几卷。
/// - **执行**给的是盘上出的事：隔离几卷。
///
/// **失败页与卷级失败两副都给**（停车场 Q146）：解不出尺寸的页在第一遍就失败，
/// 而试算只是不走第二遍；卷根被删掉的卷同样记一笔卷级失败。照从前那样按副二选一的话，
/// 一趟试算里坏了三页、废了一卷，这一行**一个字都不说**——而那正是这一块存在的理由。
///
/// **数的是此刻**：失败页走 [`Live::failures_so_far`]（收摊了的那几卷加上当前这一卷
/// 已经报过的那几条），卷级失败与隔离那两样出现的当场就进报告。它与报告末尾那几小结
/// 因此在一卷跑到一半时**故意不一样**，两处答的是两个不同的问题——见本模块开头那张表，
/// `the_trouble_row_says_now_while_the_report_tail_says_what_is_settled` 两头对着问。
///
/// 措辞仍与那几小结逐字相同，而两处没有合成一个函数：那几小结是**成句的**
/// （隔离那一句还要说清失败页在输出里是什么样），这一行是一串数；共用的话得先把那句话
/// 拆成词，而措辞只许有一处出处（ADR 0016）。
///
/// **这一行只有一种[语义](Tone)**：它列着的那几件分属两档（要注意的那几样是「注意」、
/// 失败页与卷级失败是「出事」），而取的是最重的那一个——理由见函数里那条注释。
fn trouble_row(live: &Live) -> Option<Painted> {
    let report = live.report();
    let mut listed = match live.started_as() {
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
        RunMode::Process => vec![count(isolated(report), "隔离", "卷", Tone::Caution)],
    };
    // **盘上出的那两样两副都有**（Q146），因此摆在分岔外面：试算同样解不出尺寸、
    // 同样撞得上卷根不在了。压在末尾是照重轻排——前面那几样是「注意」，这两样是「出事」。
    listed.extend([
        count(live.failures_so_far(), "失败", "页", Tone::Trouble),
        count(report.failed_volumes.len(), "卷级失败", "卷", Tone::Trouble),
    ]);
    let said: Vec<Painted> = listed.into_iter().flatten().collect();
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

/// 一条横条，`width` 格宽。样子与命令行那两条一致：`=` 是走过的，`>` 是当前这一格，
/// 空白是还没走的。
///
/// 宽度由调用方给：摆得开时是 [`BAR_WIDTH`]，这一格摆不开时是让位那一处收窄过的那个数
/// （[`fitted_bar`]）。**样子一格不随宽度变**——收窄的是格数，不是画法。
///
/// 预告的步数是零（还没开工、或者这一卷一步都不走）时整条是空的：那时没有比例可画，
/// 而画一个「刚起步」的箭头是编的。
fn bar(done: u64, total: u64, width: u64) -> String {
    let filled = (total > 0).then(|| {
        // 先乘后除：先除的话，步数比条格数少的小卷会被整个抹成 0。
        done.min(total) * width / total
    });
    let mut text = String::with_capacity(width as usize + 2);
    text.push('[');
    for at in 0..width {
        text.push(match filled.map(|filled| (at.cmp(&filled), filled)) {
            Some((Ordering::Less, _)) => '=',
            Some((Ordering::Equal, filled)) if filled < width => '>',
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
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::super::footer::running_prompt;
    use super::super::main_pane;
    use super::super::probe::{
        a_run_in_flight, main_snapshot, same_screen, screen, snapshot, tight,
    };
    use super::super::yielding::MAIN_MIN_WIDTH;
    use super::*;
    use crate::session::live::{Reach, Resuming, Volume, fixture};
    use crate::session::state::{Expansion, Key, Session};
    use tonefit::{
        BitDepth, Candidate, Dither, Envelope, GeometryGate, PageBranch, PageOutcome, RunOutcome,
        Size,
    };

    /// **耗时那一格在哪种终端上都占同一格**（判据见 [`crate::wrap::width_is_stable`]）。
    ///
    /// 它是卷表的一列（`crate::session::columns::VolumeColumn::Elapsed`），
    /// 而写法由 [`spell`] 一处造出来——与省略号、行首记号同一条规矩：
    /// **画法这一层自己造的字形一个都不许是歧义宽度**。三种写法各问一遍。
    #[test]
    fn the_elapsed_this_module_spells_is_the_same_width_on_any_terminal() {
        for seconds in [0, 9, 59, 60, 400, 3599, 3600, 5 * 3600 + 120] {
            let said = spell(Duration::from_secs(seconds));
            for glyph in said.chars() {
                assert!(
                    crate::wrap::width_is_stable(glyph),
                    "{glyph} 是东亚歧义宽度：{seconds}s 写成「{said}」"
                );
            }
        }
    }

    /// 总览块**单独**一张快照：主区最上面那一块，一个框。
    ///
    /// 只钉这一块，是因为本票做的就是它——把报告区一起钉进来，改一句报告措辞就要
    /// 重录这四张（`main_snapshot` 那两张钉的正是主区整块，两者分工不同）。
    ///
    /// 高度照它自己说的取（[`Overview::height`]）：出事行在不在场，快照上一眼看得出。
    fn block(live: &Live, pressed: Instruction, deciding: bool) -> String {
        block_at(live, pressed, deciding, WIDE)
    }

    /// 宽终端那一档：横条摆得开，一行都不省略。
    const WIDE: u16 = 96;

    /// 同上，**摆在指定宽度的一格里**：窄档那几条要它（见
    /// [`the_bars_give_way_before_the_numbers_do`]）。
    fn block_at(live: &Live, pressed: Instruction, deciding: bool, width: u16) -> String {
        let top = overview(Some(live), pressed, deciding, width);
        let height = top.height();
        snapshot(
            |frame| frame.render_widget(top.draw(), frame.area()),
            width,
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
    /// （[`Live::started_as`]），不必多一个开关。
    ///
    /// 两副各问一遍：试算那一副给判定分布与要注意的三样，执行那一副给完成／跳过与隔离。
    /// 这几个字**互不出现在对方身上**——同一行两种内容，混了就等于没分。
    ///
    /// **盘上出的那两样（失败页 · 卷级失败）不在这张单子上**：Q146 之后两副都给，
    /// 各是哪一副由 [`a_dry_run_says_what_broke_too`] 问。
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
        for said in ["完成 1 卷", "隔离 1 卷"] {
            assert!(real.contains(said), "执行那一副少了「{said}」：{real}");
            assert!(!dry.contains(said), "试算那一副不该有「{said}」：{dry}");
        }
    }

    /// **试算那一副的出事行也带失败页与卷级失败**（停车场 Q146）：试算同样解不出尺寸、
    /// 同样撞得上卷根不在了，而从前那一行按副二选一——一趟试算里坏了页、废了卷，
    /// 屏上一个字都不说。
    ///
    /// **「一条都没有时整行不出现」一格没松**：末一问走的是一趟干干净净的试算。
    #[test]
    fn a_dry_run_says_what_broke_too() {
        let mut live = a_dry_run_with_a_broken_page_and_a_lost_volume();
        let row = trouble_row(&live).expect("这一趟出了事");

        for said in ["失败 1 页", "卷级失败 1 卷"] {
            assert!(
                row.text.contains(said),
                "试算那一副少了「{said}」：{}",
                row.text
            );
        }
        // **隔离仍旧只在执行那一副。**理由不是「试算一个字节都没写」——按 `t` 起的那一趟
        // 在决策点上答过继续之后真写盘、真隔离；那是收 Q149 认下的折扣，记在 Q196。
        // 屏上不至于一个字都不说：隔离的判据就是有没有失败页，而失败页这一行数得出来。
        assert!(
            !row.text.contains("隔离"),
            "试算那一副说了隔离：{}",
            row.text
        );
        // 出了事的那一档压过要注意的那一档。
        assert_eq!(row.tone, Tone::Trouble);

        // 一条都没有的那一趟：整行不出现，那一行让给报告区。
        live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(1, 1000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::processed_volume("卷一", None));
        assert!(trouble_row(&live).is_none(), "干净的一趟还画着出事行");
    }

    /// 一趟**试算**，坏了一页、废了一卷：Q146 那两样各一份。
    fn a_dry_run_with_a_broken_page_and_a_lost_volume() -> Live {
        const BROKEN: &str = "解不出完整尺寸：JPEG 数据截断";

        let mut live = Live::new(&fixture::request(RunMode::DryRun), Resuming::Waits);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.page_failed(Path::new("库/卷一/017.jpg"), BROKEN);
        live.volume_finished(&fixture::processed_volume("卷一", Some(BROKEN)));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.volume_failed(Path::new("库/卷二"), "卷根不在了");
        live
    }

    /// **出事行答「此刻」，报告末尾那几小结答「已定案」**（停车场 Q148）：
    /// 一卷跑到一半时两个数**故意**不一样，而各自都对。
    ///
    /// 从前这一行数的是报告那一份（只含收摊了的卷），因此比同一屏的报告区**晚一整卷**——
    /// 报告区已经写着「失败页（出现的当场……）」，钉在它上面、专门答「这一趟到底怎么样」
    /// 的那一行却一个字都不说。
    ///
    /// **措辞两头仍旧逐字相同**（ADR 0016：措辞只有一处出处）：一卷收摊之后两个数又相等，
    /// 那一刻两头对着问得出同一句话。改了一头，这一条当场红。
    #[test]
    fn the_trouble_row_says_now_while_the_report_tail_says_what_is_settled() {
        const BROKEN: &str = "解不出完整尺寸：JPEG 数据截断";

        let mut live = a_run_in_flight(true);
        live.volume_failed(Path::new("库/卷四"), "卷根不在了");
        // 当前这一卷又坏了一页：事件当场就到，而那一卷还没收摊。
        live.page_failed(Path::new("库/卷三/004.jpg"), BROKEN);

        let now = trouble_row(&live).expect("这一趟出了事");
        let tail = crate::render::tail(live.report());
        assert!(
            now.text.contains("失败 2 页"),
            "此刻坏的没数上：{}",
            now.text
        );
        assert!(
            tail.contains("失败 1 页"),
            "末尾那几小结数的不再是已定案的那几卷：{tail}"
        );

        // 那一卷收摊之后两个数又相等，措辞也逐字相同。
        live.volume_finished(&fixture::processed_volume("卷三", Some(BROKEN)));
        let settled = trouble_row(&live).expect("这一趟出了事");
        let tail = crate::render::tail(live.report());
        for said in ["隔离 2 卷", "失败 2 页", "卷级失败 1 卷"] {
            assert!(
                settled.text.contains(said),
                "出事行少了「{said}」：{}",
                settled.text
            );
            assert!(tail.contains(said), "报告末尾那几小结不这么说了：{tail}");
        }
    }

    /// **决策点上答出继续的那一帧，这一块不换内容、也不矮一行**（停车场 Q149）。
    ///
    /// 那一刻 [`Live::mode`] 从试算翻成执行（那一卷真写了出去），而结论行与出事行
    /// 跟着它走的话：判定分布当场换成完成／跳过——用户刚据以拿主意的那一份没了；
    /// 出事行从「特例页 · 宽溢出 · 几何门不成立」换成「隔离」，此刻多半全是零，
    /// 于是整行消失、这一块矮一行、下面的报告整块上移。
    ///
    /// 两行因此问 [`Live::started_as`]。**抬头照旧改口**：它答的是另一个问题
    /// （此刻在写没写），而它摆在边框上，一行正文都不占。
    #[test]
    fn answering_go_on_does_not_move_the_overview() {
        let mut live = a_dry_run_stopped_at_a_decision_point();
        let before = overview(Some(&live), Instruction::Continue, true, WIDE);
        let (settled, trouble, height) = (settled_row(&live), trouble_row(&live), before.height());
        assert!(
            settled.contains("判定 "),
            "拿主意那一刻没给判定分布：{settled}"
        );

        live.decide(Instruction::Continue, Reach::ThisVolume);

        assert_eq!(settled_row(&live), settled, "答了继续，判定分布就没了");
        assert_eq!(
            trouble_row(&live).map(|row| row.text),
            trouble.map(|row| row.text),
            "答了继续，出事行换了一副内容"
        );
        assert_eq!(
            overview(Some(&live), Instruction::Continue, false, WIDE).height(),
            height,
            "答了继续，这一块矮了一行"
        );
        // 抬头是另一件事：那一卷真写了出去，它当场改口。
        assert_eq!(live.mode(), RunMode::Process);
        assert!(
            block(&live, Instruction::Continue, false).contains("执行"),
            "抬头没跟着落盘那件事改口"
        );
    }

    /// 一趟**续做**的试算，停在第二卷的决策点上：头一卷已经收了摊，
    /// 而它带着两张特例页——出事行因此在场。
    fn a_dry_run_stopped_at_a_decision_point() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        live.run_started(2, 2000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&a_volume_worth_a_second_look("卷一"));
        live.volume_started(Path::new("库/卷二"), 1000);
        live.pass_started(Pass::Second, Some(&fixture::processed_volume("卷二", None)));
        live
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
            overview(Some(&quiet), Instruction::Continue, false, WIDE).height(),
            OVERVIEW_HEIGHT - 1,
            "没出事还画着出事行"
        );
        assert_eq!(
            overview(Some(&noisy), Instruction::Continue, false, WIDE).height(),
            OVERVIEW_HEIGHT
        );
        assert!(!block(&quiet, Instruction::Continue, false).contains("出事"));

        // **收场之后当前卷那一行也让出去**：那时再没有「本卷」可说。
        assert_eq!(
            overview(
                Some(&a_run_that_finished_clean()),
                Instruction::Continue,
                false,
                WIDE
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
    /// 翻的是**展开**那一副——默认那一副的滚动量由光标算出来（跟随着的时候恒停在底上），
    /// 而按得动的只有展开着的那一份。
    ///
    /// 屏高按这一块自己的高度加五行取：逐页那张表因此**一定**摆不下
    /// （见 `super::pages`），翻下去才真有东西在动。
    #[test]
    fn the_overview_block_stays_put_while_the_report_scrolls() {
        let live = a_run_in_flight(true);
        let alone = block(&live, Instruction::Continue, false);
        let rows = alone.lines().count();
        let height = u16::try_from(rows + 5).expect("这一块没有六万行");
        let mut session = Session::new();
        // 第二卷才有逐页那几行：头一卷是幂等命中的，一页都没重做。
        session.expand(Expansion::new(PathBuf::from("库"), Volume::Settled(1)));

        let mut seen: Vec<String> = Vec::new();
        for _ in 0..4 {
            let shot = snapshot(
                |frame| main_pane(frame, frame.area(), &mut session, Some(&live)),
                96,
                height,
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
    /// 收窄那一档画法一格不变，只是格数少了（末两句）。
    #[test]
    fn the_bar_fills_from_empty_to_full() {
        assert_eq!(BAR_WIDTH, crate::BAR_WIDTH as u64, "横条宽度长出了第二份");
        assert_eq!(bar(0, 100, BAR_WIDTH), format!("[>{}]", " ".repeat(29)));
        assert_eq!(bar(100, 100, BAR_WIDTH), format!("[{}]", "=".repeat(30)));
        assert_eq!(bar(0, 0, BAR_WIDTH), format!("[{}]", " ".repeat(30)));
        // 步数比条格数少的小卷不该被抹成 0。
        assert!(bar(1, 3, BAR_WIDTH).starts_with("[========="));
        // 收窄到八格：两头照旧，箭头照旧在走过的那一格上。
        assert_eq!(bar(0, 100, 8), format!("[>{}]", " ".repeat(7)));
        assert_eq!(bar(100, 100, 8), format!("[{}]", "=".repeat(8)));
        assert_eq!(bar(50, 100, 8), "[====>   ]");
    }

    /// **窄档上横条先收窄，收不下去就整条让掉**（停车场 Q169）。
    ///
    /// 从前它一格不让：80×24 上主区只有 30 列（[`MAIN_MIN_WIDTH`]），而这两行各要
    /// 六十几格，屏上因此是 `│ 总体 [==================>  │`——右边那个方括号、步数与
    /// 已用一起在框外。切在半路的横条比截断的抬头更坏：没有收尾的 `]`，
    /// 走过的比例看着比实际大。
    ///
    /// **两行各按自己剩下的格算**（见 [`with_a_bar`]），末一段问的正是这一条的理由：
    /// 合成一个数的话，全局那一条的刻度会在换卷那一刻跳一下。
    #[test]
    fn the_bars_give_way_before_the_numbers_do() {
        let live = a_run_in_flight(true);
        // 卷与卷之间那一副：当前卷那一行是空的（见 [`volume_row`]）。
        let mut between = a_run_in_flight(true);
        between.volume_finished(&fixture::processed_volume("卷三", None));

        // 这一副屏上那几条横条各有几格；没画横条的行不进这张单子。
        let bars = |live: &Live, width: u16| -> Vec<usize> {
            block_at(live, Instruction::Continue, false, width)
                .lines()
                .filter_map(|row| {
                    let opened = row.find('[')?;
                    let closed = row[opened..].find(']').expect("画出来的横条都有收尾那一格");
                    Some(closed - 1)
                })
                .collect()
        };

        // 摆得开：两条都是满宽，与命令行那两条一样长。
        assert_eq!(
            bars(&live, WIDE),
            vec![BAR_WIDTH as usize; 2],
            "宽终端上横条缩了"
        );
        // 收窄了：当前卷那一行的字更多（卷名与在走哪一遍），它那一条因此先短一格。
        assert_eq!(bars(&live, 45), vec![9, 8], "没按各自剩下的格收窄");
        // 再窄一格，当前卷那一条整条让掉——而它那两个数还在。
        assert_eq!(bars(&live, 44), vec![8], "该让的是横条，不是数");
        assert!(
            block_at(&live, Instruction::Continue, false, 44).contains("1000/3000 步"),
            "让掉横条之后连数也没了"
        );
        // 再窄一格，全局那一条也让掉，两行的数与「已用」照旧整条在屏上。
        assert!(bars(&live, 43).is_empty(), "窄到摆不下还画着横条");
        let bare = block_at(&live, Instruction::Continue, false, 43);
        for said in ["3000/5000 步", "已用 5m00s", "1000/3000 步"] {
            assert!(bare.contains(said), "让掉横条之后「{said}」也没了：{bare}");
        }

        // **全局那一条不跟着换卷跳**：卷与卷之间当前卷那一行是空的，而它一格不动。
        // 两行合用一个数的话，它的刻度会在换卷那一刻涨一截——同一个「走了六成」
        // 上一刻十八格、下一刻六格，看着像整趟往回退了。
        for width in [WIDE, 50, 46, 45, 44] {
            assert_eq!(
                bars(&between, width).first(),
                bars(&live, width).first(),
                "{width} 列上换卷那一刻全局那一条的刻度跳了"
            );
        }

        // 80×24 那一档：主区只有这么宽，而正文本身就比它长——横条让完仍摆不下。
        // 摆不下的那一截**从中间省略**，不由终端库从行尾硬截（Q147 立的那一条）：
        // 屏上因此有一处痕迹说它被截过，而末一截仍旧整条在屏上。
        let narrow = block_at(&live, Instruction::Continue, false, MAIN_MIN_WIDTH);
        assert!(
            bars(&live, MAIN_MIN_WIDTH).is_empty(),
            "最窄那一档还画着横条"
        );
        assert!(narrow.contains('⋯'), "从行尾硬截了：{narrow}");
        assert!(narrow.contains("已用 5m00s"), "末一截被截掉了：{narrow}");
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
    /// [`super::super::yielding::MAIN_MIN_HEIGHT`] 留的正是这一块最高那几行加报告区最少那三行。
    #[test]
    fn the_overview_is_the_last_thing_the_main_pane_gives_up() {
        assert_eq!(
            OVERVIEW_HEIGHT,
            overview(
                Some(&a_run_in_flight(true)),
                Instruction::Continue,
                false,
                WIDE
            )
            .height(),
            "最高那一副与留位子的那个数对不上"
        );
    }
}
