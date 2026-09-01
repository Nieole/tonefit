//! 会话的画法：左窄配置常驻 + 右宽主区（spec 的《会话：布局与交互》）。
//!
//! 主区自上而下三段：**全局条**（卷数 · 剩余时间）、**当前卷条**（在走哪一遍 · 步数）、
//! **报告区**（边跑边攒）。逐页展开与左栏收起归 `p1-session/11`，这里不替它们决定形状。
//!
//! # 措辞在这里长第二份的只有两样
//!
//! 报告区画的是 [`crate::render`] 那几个函数——命令行与会话共用，一个字都不在这里重写。
//! 「边跑边攒」因此不必另有一套说法：一卷跑完那条事件带着那一卷的报告（ADR 0011），
//! [`crate::render::volume`] 收下它就画得出判定、驱动页、隔离与这一趟怎么读的。
//!
//! 长在本模块的只有**命令行上根本没有**的那两样：左栏那几行配置的标签与按键提示，
//! 以及两条进度条的排版（命令行那两条是 indicatif 的模板，见 `crate::bar_style`）。

use std::cmp::Ordering;
use std::time::Duration;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use tonefit::{Instruction, Pass};

use super::complete;
use super::live::{Live, Walking};
use super::state::{Edit, Field, Layer, Mode, Session, Shape};

/// 左栏的宽度。配置一直在场，改一下就能在右边看到影响。
///
/// 固定列数而不是按比例：这一栏装的是**标签加取值**，两边都不随终端变宽而变长，
/// 按比例分只会在宽终端上留下一栏空白。
const CONFIG_WIDTH: u16 = 52;

/// 主区无论如何要留下的列数。
///
/// **终端窄到放不下两栏时，让的是左栏。** 报告区挤到十几列就一个字都读不出来，
/// 而左栏那几行本来就折着行（见 [`config`]），窄一点仍看得懂。
///
/// 这不是「左栏收起」——那是 `p1-session/11` 的一个开关，用户按得动、也按得回来；
/// 这里是放不下时的退化，没有开关。
const MAIN_MIN_WIDTH: u16 = 30;

/// 屏底那几行：编辑条、补全候选、要说的那句话。
const FOOTER_HEIGHT: u16 = 3;

/// 全局条与当前卷条各占几行：一行正文加上下两条边。
const BAR_HEIGHT: u16 = 3;

/// 一条横条画多宽。**与命令行那两条同一个出处**（`crate::BAR_WIDTH`）：
/// 两处的横条长得一样，读的人不必重新认一遍。
const BAR_WIDTH: u64 = crate::BAR_WIDTH as u64;

/// 试算与执行那两个键。屏上提到它们的地方都用这一句——
/// 键位改了只改这里，不必去找第二处、第三处。
///
/// 它们各自**做什么**只在报告区那一段说（见 [`report_pane`]）：那里有地方把话说完整，
/// 而这里是提示条，长了反而读不出重点。
const START_KEYS: &str = "t 试算 · x 执行";

/// 跑起来之后左栏的抬头。**「只读」要看得出来**，不能是按了没反应
/// （`CONTEXT.md` 的《会话》：一趟跑起来之后三层都只读）。
const READ_ONLY_TITLE: &str = "配置 · 跑着，三层都只读";

/// 左栏在这一屏上占多宽。装得下就是 [`CONFIG_WIDTH`]，装不下就让给主区。
fn config_width(total: u16) -> u16 {
    CONFIG_WIDTH.min(total.saturating_sub(MAIN_MIN_WIDTH))
}

/// 把一屏画出来。
pub fn shell(frame: &mut Frame, session: &Session, live: Option<&Live>) {
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(FOOTER_HEIGHT)])
        .areas(frame.area());
    let [left, main] = Layout::horizontal([
        Constraint::Length(config_width(body.width)),
        Constraint::Min(0),
    ])
    .areas(body);

    frame.render_widget(config(session), left);
    main_pane(frame, main, live);
    frame.render_widget(self::footer(session), footer);
}

/// 左栏：三层，各占一块，按生命周期从上到下。
///
/// **跑起来之后整栏只读**，而这一条要在屏上**看得出来**，不能是「按了没反应」：
/// 抬头改口（[`READ_ONLY_TITLE`]），光标不再反白，各行压暗。
/// 真正拦住按键的不是这里——是状态机在那个状态下一个改动键都不派
/// （见 `super::state::running_action`）；这里只把那件事说出来。
fn config(session: &Session) -> Paragraph<'static> {
    let running = matches!(session.mode(), Mode::Running(_));
    let focus = session.focus();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut drawn: Option<Layer> = None;
    for field in session.rows() {
        let layer = field.layer();
        if drawn != Some(layer) {
            if drawn.is_some() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                layer.title().to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            drawn = Some(layer);
        }
        // 跑着时光标不反白：那一格反白说的是「就在这一行上动手」，而这时按不动。
        let style = match (running, field == focus) {
            (true, _) => Style::default().add_modifier(Modifier::DIM),
            (false, true) => Style::default().add_modifier(Modifier::REVERSED),
            (false, false) => Style::default(),
        };
        lines.push(row(session, field, style));
    }
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(if running {
            READ_ONLY_TITLE
        } else {
            "配置"
        }))
        // 折行而不是切掉：阈值那一行要把**标定来源**原样带上来（spec 的 Further Notes），
        // 而那句话比这一栏宽；路径也一样，切掉尾巴的路径看不出是哪一个。
        // `trim: false` 让折下来的那一截保留缩进，读得出它还是上一行的。
        .wrap(Wrap { trim: false })
}

/// 左栏上的一行：名字 + 取值。怎么标（反白、压暗、还是原样）由 [`config`] 定。
fn row(session: &Session, field: Field, style: Style) -> Line<'static> {
    let text = match field {
        // 卷那一行的取值里已经带着勾与路径，再挂一个「卷」字是废话。
        Field::Volume(_) => format!("  {}", session.shown(field)),
        Field::AddVolume => format!("  {}", field.label()),
        _ => format!("  {:　<8}{}", field.label(), session.shown(field)),
    };
    Line::from(Span::styled(text, style))
}

/// 右边那一大格，自上而下三段。
///
/// 三段各占一格而不是挤成一段文字：前两段是**此刻**的事（步数一直在动），
/// 第三段是**攒下来**的事（只增不改）。挤在一起的话，报告长起来之后进度条就被顶出屏外，
/// 而那正是长任务里唯一有人真想看的东西。
pub fn main_pane(frame: &mut Frame, area: Rect, live: Option<&Live>) {
    let [overall, current, report] = Layout::vertical([
        Constraint::Length(BAR_HEIGHT),
        Constraint::Length(BAR_HEIGHT),
        Constraint::Min(0),
    ])
    .areas(area);

    frame.render_widget(overall_bar(live), overall);
    frame.render_widget(volume_bar(live), current);
    frame.render_widget(report_pane(live, report), report);
}

/// 全局条：**卷数与剩余时间**——长任务里唯一有人真想知道的两个数（ADR 0011）。
///
/// 两个数都出自开工那条事件（`RunStarted`），而它是**预扫**算出来的（03 号票）。
/// 预告的步数是**上界**不是承诺（`CONTEXT.md` 的《进度》），剩余时间因此也是估计：
/// 幂等命中的卷提前收摊，剩下的那一截会突然缩短。
fn overall_bar(live: Option<&Live>) -> Paragraph<'static> {
    let block = Block::default().borders(Borders::ALL).title("整趟");
    let Some(live) = live else {
        return Paragraph::new(format!(" 还没跑过。{START_KEYS}")).block(block);
    };
    let overall = live.overall();
    let line = if live.ended() {
        ended_line(live)
    } else {
        format!(
            " {}/{} 卷 {} {}/{} 步 · 已用 {} · 剩 {}",
            overall.volume,
            overall.volumes,
            bar(overall.walked, overall.steps),
            overall.walked,
            overall.steps,
            spell(overall.elapsed),
            overall.left.map_or_else(|| "—".to_owned(), spell),
        )
    };
    Paragraph::new(line).block(block)
}

/// 收场那一行。
///
/// 没做成那一句照库那一侧的原话（拒绝执行是一种，那条线程恐慌了是另一种）；
/// 做成了那一种照 [`crate::render::outcome`]——「按停停在半路」与「点名的卷都走过了」
/// 的分别在 `Report::outcome` 上，措辞跟报告那一套走，会话不另编一句。
///
/// 「用了」那个数收场之后就定住了（见 [`Live::overall`]）：它是库交出来的那一个，
/// 扣掉了在决策点上等人的那几分钟。
fn ended_line(live: &Live) -> String {
    match live.undone() {
        Some(said) => format!(" 这一趟没做成：{said}"),
        None => format!(
            " 收场 {} · {} 卷 · 用了 {}",
            crate::render::outcome(live.report().outcome),
            live.report().volumes.len(),
            spell(live.overall().elapsed),
        ),
    }
}

/// 当前卷条：**在走哪一遍**，以及这一遍走到第几步。
///
/// 「在走哪一遍」只有 `PassStarted` 答得出（命令行那一路当下没有去处，见 `crate::Bar`）。
/// 非说不可，是因为三遍的性质完全不同：幂等那一道只读不写，第一遍碰像素，
/// 第二遍才往盘上写字节——「跑到一半停下来会留下什么」全看它停在哪一遍。
fn volume_bar(live: Option<&Live>) -> Paragraph<'static> {
    let block = Block::default().borders(Borders::ALL).title("当前卷");
    let line = match live.and_then(Live::walking) {
        Some(walking) => walking_line(walking),
        // 卷与卷之间、以及还没跑过时都是空的：编一条横条上去只会让人以为它卡住了。
        None => String::new(),
    };
    Paragraph::new(line).block(block)
}

fn walking_line(walking: &Walking) -> String {
    // 卷名怎么取只有一处：命令行那条横条印的是同一个（`crate::Bar::start`）。
    let name = crate::render::volume_name(&walking.volume);
    format!(
        " {name} · {} {} {}/{} 步",
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
fn spell(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    match (seconds / 3600, (seconds % 3600) / 60, seconds % 60) {
        (0, 0, second) => format!("{second}s"),
        (0, minute, second) => format!("{minute}m{second:02}s"),
        (hour, minute, _) => format!("{hour}h{minute:02}m"),
    }
}

/// 报告区：**边跑边攒**的那一份，措辞出自 [`crate::render`]。
///
/// 卷级那几行一卷跑完就画得出来（一卷跑完那条事件带着那一卷的报告，ADR 0011）：
/// 判定、驱动页、隔离、这一趟怎么读的、幂等命中说清哪四项依据没变，全在里面。
/// 逐页那几行**默认不给**——它归 `p1-session/11`（展开时左栏收起、主区吃满宽度），
/// 而失败页要在出现的当场看得见，那一段走 [`crate::render::failing_pages`]。
///
/// **长过一格就只画最后那几行**（见 [`last_lines`]）：报告只增不减，
/// 而「一卷跑完当场看得见」说的正是刚添上去的那几行。
fn report_pane(live: Option<&Live>, area: Rect) -> Paragraph<'static> {
    let block = Block::default().borders(Borders::ALL).title("报告");
    let Some(live) = live else {
        return Paragraph::new(
            "
 按 t 试算：只算不写，报告照出。
              按 x 执行：写到输出根。
              跑起来之前必填的两项是型号与输出根。",
        )
        .block(block)
        .wrap(Wrap { trim: false });
    };
    let text = report_text(live);
    // 边框各占一格，正文因此只剩这么大。
    let inside = Rect::new(
        0,
        0,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let past = past_the_top(&text, inside);
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((past, 0))
        .block(block)
}

/// 报告折行之后有几行掉在这一格**上面**——把它当滚动量，格子里留下的就是最后那几行。
///
/// 报告只增不减：从头画的话，跑到第十几卷时新添的那几行全掉在格子外面，
/// 而这一票要的正是「不必等全部跑完才发现参数错了」。收场之后留最后那几行同样是对的：
/// 末尾那几小结按「这一趟出的事有多重」往下排，最重的压在最后（见 [`crate::render::tail`]）。
///
/// 翻回去看前面几卷要等 `p1-session/11`：报告区的展开与收起归它，本票不替它决定形状。
///
/// **折行有几行是量出来的，不是估出来的**：让 ratatui 自己往一块够高的临时缓冲上画一遍，
/// 再数底下空着几行。它那个 `Paragraph::line_count` 眼下还挂着 unstable 的门，
/// 而自己写一套「中文两列、按词断行」的估算是停车场 Q32 的地界——
/// 估错一行就会把最新的那几行挤出格子。
fn past_the_top(text: &str, inside: Rect) -> u16 {
    if inside.width == 0 || inside.height == 0 {
        return 0;
    }
    let scratch = Rect::new(0, 0, inside.width, tall_enough(text, inside.width));
    let mut buffer = Buffer::empty(scratch);
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .render(scratch, &mut buffer);
    let used = (0..scratch.height)
        .rev()
        .find(|row| (0..inside.width).any(|at| buffer[(at, *row)].symbol() != " "))
        .map_or(0, |row| row + 1);
    used.saturating_sub(inside.height)
}

/// 一块**一定装得下**这份东西的临时缓冲有多高。
///
/// 上界，不是估计：按词断行时每一行至少放得下一个词，因此行数不会超过
/// 「每个词单独占一行时要的行数之和」。量出来的那个数由 [`past_the_top`] 现算，
/// 这里只管别让缓冲太小。
fn tall_enough(text: &str, width: u16) -> u16 {
    let mut tall: u32 = 0;
    for line in text.lines() {
        let mut rows: u32 = 0;
        for word in line.split(' ') {
            let wide = u32::try_from(Span::raw(word).width()).unwrap_or(u32::from(u16::MAX));
            rows += wide.div_ceil(u32::from(width)).max(1);
        }
        tall = tall.saturating_add(rows.max(1));
    }
    u16::try_from(tall).unwrap_or(u16::MAX)
}

/// 报告区的正文。**与命令行印出来的那一份同源**：同样的段、同样的函数。
///
/// 与那一份的差只有两处，各有理由：逐页那几行留给 `11`；失败页那一段是命令行没有的
/// **增量**（命令行攒完才印，那时逐页那几行已经把话说全了）。
/// 末尾那几小结要看完整趟才给得出来（见 [`crate::render::tail`]），因此只在收场之后画。
fn report_text(live: &Live) -> String {
    let report = live.report();
    let mut text = crate::render::header(report, live.mode());
    for volume in &report.volumes {
        text.push_str(&crate::render::volume(volume));
    }
    text.push_str(&crate::render::failing_pages(live.failed_pages()));
    if live.ended() {
        text.push_str(&crate::render::tail(report));
    }
    text
}

/// 屏底：正在打字就显示缓冲与这一层列出来的候选，否则显示按键提示。
fn footer(session: &Session) -> Paragraph<'static> {
    let mut lines = match session.mode() {
        Mode::Editing(edit) => editing_lines(session, edit),
        Mode::Browsing => vec![Line::from(browsing_keys(session)), Line::from("")],
        Mode::Running(pressed) => running_lines(*pressed),
    };
    lines.push(Line::from(session.notice().unwrap_or("").to_owned()));
    Paragraph::new(lines)
}

/// 跑起来之后屏底那两行：**上一行说这时按得动的键，下一行说按下去之后它在等什么**
/// （ADR 0013）。
///
/// 一张表而不是两个函数：两行随的是**同一个**取值，而屏底那一格本来就是一起画的——
/// 分成两处，改一级的措辞就要在两处对着改。
///
/// 上一行：配置这时只读（spec 的《会话：布局与交互》），因此一个改动键都不提；
/// 「只读」那件事本身写在左栏抬头上（见 [`config`]）。按到中止之后 `s` 也不提了——
/// 闩到了顶，再按一次没有更强的一级可去（`super::state::running_action` 在那一级上
/// 派的是「没有意义」）。**屏上不摆按不动的键**，那正是「按了没反应」的来源。
///
/// 下一行：收尾那一句非说不可——按下去之后屏上一切照旧地往前走，几千页的卷还要跑几分钟，
/// 不说清「在等当前卷跑完」，看上去就像那一下没按上。中止那一句说的是**盘上会剩下什么**。
/// 没按过时它是空的，与浏览时那一行同一个样子（那一行也是空的）。
///
/// 措辞与报告里那两句（`crate::render::outcome` 的「按停」）说的是同一件事，
/// 但时态不同：那两句是收场之后的结果，这两句是此刻在等的事。
fn running_lines(pressed: Instruction) -> Vec<Line<'static>> {
    let [keys, waiting] = match pressed {
        Instruction::Continue => [
            " 跑着…… · s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷）",
            "",
        ],
        Instruction::Finish => [
            " 收尾中…… · 再按一次 s 中止 · Ctrl-C 退出会话",
            " 收尾：等当前卷跑完就停，剩下的卷一个都不开工；盘上只留完整的卷，下一趟幂等接着走",
        ],
        Instruction::Abort => [
            " 中止中…… · Ctrl-C 退出会话",
            " 中止：当前卷停在这一页上，它那格 partial 丢掉——那一卷等于没做，最终位置上一个字节都没动过",
        ],
    };
    vec![Line::from(keys), Line::from(waiting)]
}

fn editing_lines(session: &Session, edit: &Edit) -> Vec<Line<'static>> {
    let keys = match edit.field.shape() {
        Shape::Path => "⇥ 补这一层 · ⏎ 收下 · Esc 丢掉",
        _ => "⏎ 收下 · Esc 丢掉",
    };
    vec![
        Line::from(format!(" {} {}▏   {keys}", edit.field.label(), edit.buffer)),
        // 只列打到的那一层，且**只是列出来**：不留索引、不留缓存（ADR 0009）。
        Line::from(format!(" {}", listed(session, edit))),
    ]
}

/// 补全列出来的那一层，摆成一行。空着就说一句这一层还没列过。
fn listed(session: &Session, edit: &Edit) -> String {
    if edit.candidates.is_empty() {
        // 有话要说时这一行让位——那句话就印在下一行。
        return match session.notice() {
            Some(_) => String::new(),
            None => "按 ⇥ 列出这一层".to_owned(),
        };
    }
    // 只留这一层里的那个名字，切法在 `complete` 那一侧——分隔符表只有一份。
    let names: Vec<&str> = edit
        .candidates
        .iter()
        .map(|hit| complete::name(hit))
        .take(12)
        .collect();
    format!("这一层：{}", names.join("  "))
}

/// 浏览时的按键提示，随光标停的那一行而变——按不动的键不该印在屏上。
///
/// 试算与执行两个键**每一行上都在**：它们与光标停在哪儿无关，
/// 而「配好了之后按哪个键」是这一屏上最该一直看得见的事。
fn browsing_keys(session: &Session) -> String {
    let common = format!("↑↓ 选 · {START_KEYS} · q 退出");
    match session.focus().shape() {
        Shape::Cycle => format!(" ←→ 换一个 · {common}"),
        Shape::Text => format!(" ⏎ 改 · {common}"),
        Shape::Path => format!(" ⏎ 打一个路径进来（⇥ 逐层补全）· {common}"),
        Shape::Volume => format!(" 空格 勾上／勾掉 · d 删掉这一条 · {common}"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::session::live::fixture;
    use crate::session::state::Key;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tonefit::{Mode as RunMode, RunOutcome};

    /// 屏上的文字，**空白全去掉**再比。
    ///
    /// 宽字符在缓冲里占两格，第二格被 ratatui 重置成一个空格——逐格读回来的文字
    /// 因此在每个汉字之间多一个空格（停车场 Q60）。要问的是「这几个字在不在屏上」，
    /// 两边都去掉空白最省事，也不会把断言比松：这些标签没有一个靠空格分辨。
    ///
    /// **快照那几条不走这里**：它们比的是 [`snapshot`]，而那一条路上宽字符那一格
    /// 根本不在场。
    fn tight(text: &str) -> String {
        text.chars()
            .filter(|glyph| !glyph.is_whitespace())
            .collect()
    }

    /// 把一屏画出来，取回屏上的文字（逐格拼，宽字符后面多一个空格）。
    fn screen(session: &Session, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        terminal
            .draw(|frame| shell(frame, session, None))
            .expect("画得出来");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    /// 左栏上有几格是反白的。**光标停在哪一行只有它看得出来**——
    /// 逐格拼回来的文字里一点痕迹都没有，而「跑起来之后按不动」正要靠这一格说话。
    fn reversed_rows(session: &Session) -> usize {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("测试后端起得来");
        terminal
            .draw(|frame| shell(frame, session, None))
            .expect("画得出来");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .filter(|cell| cell.modifier.contains(Modifier::REVERSED))
            .count()
    }

    /// 一屏的**快照**：一行一行，每行两侧加引号，一格都不多一格不少。
    ///
    /// 走的是 `TestBackend` 自己的 `Display`——它按 `cell_width` 跳过被宽字符盖住的那一格
    /// （停车场 Q60 说的正是那一格）。自己逐格拼的话，每个汉字后面都会多出一个空格来，
    /// 而快照要的恰恰是「屏上一模一样」。
    fn snapshot(draw: impl FnOnce(&mut Frame), width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("测试后端起得来");
        terminal.draw(draw).expect("画得出来");
        terminal
            .backend()
            .to_string()
            .lines()
            // 它每行是 `"……"`，后面还可能跟一句「被宽字符盖住的是哪几格」——
            // 那一句是给人看的诊断，不进快照。引号留着：不留的话，行尾那几个空格
            // 在编辑器里会被吃掉，快照下一次就对不上了。
            .filter_map(|row| row.split('"').nth(1))
            .map(|row| format!("\"{row}\""))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 主区单独一格的快照。快照只钉主区，是因为本票做的就是它——
    /// 把左栏一起钉进来，改一行配置标签就要重录一次这几段。
    fn main_snapshot(live: &Live, width: u16, height: u16) -> String {
        snapshot(
            |frame| main_pane(frame, frame.area(), Some(live)),
            width,
            height,
        )
    }

    /// 一趟跑到一半：两卷跑完（一卷幂等命中、一卷带失败页），第三卷正走第二遍。
    ///
    /// 时钟往回拨一段固定的量，快照因此不随机器快慢而变——与黄金快照同一条规矩
    /// （`tonefit::Report::elapsed`：计时只进结构，不进渲染出的文字）。
    fn a_run_in_flight(failures: bool) -> Live {
        let mut live = Live::new(&fixture::request(RunMode::Process));
        live.run_started(3, 5000);
        live.volume_started(Path::new("库/卷一"), 1000);
        live.volume_finished(&fixture::skipped_volume("卷一", 180));
        live.volume_started(Path::new("库/卷二"), 1000);
        let broken = failures.then_some("解不出完整尺寸：JPEG 数据截断");
        if let Some(reason) = broken {
            live.page_failed(Path::new("库/卷二/017.jpg"), reason);
        }
        live.volume_finished(&fixture::processed_volume("卷二", broken));
        live.volume_started(Path::new("库/卷三"), 3000);
        live.pass_started(tonefit::Pass::Second);
        for _ in 0..1000 {
            live.stepped();
        }
        live.rewind(Duration::from_secs(300));
        live
    }

    /// 左栏按三块显示，各项都在屏上；主区三段都在。
    #[test]
    fn the_shell_draws_three_layers_and_the_three_sections_of_the_main_pane() {
        let session = Session::new();

        let screen = tight(&screen(&session, 120, 40));

        for layer in [Layer::Device, Layer::Taste, Layer::Scope] {
            assert!(
                screen.contains(&tight(layer.title())),
                "{layer:?} 那一块没画出来"
            );
        }
        for field in session.rows() {
            assert!(
                screen.contains(&tight(field.label())),
                "{field:?} 那一行没画出来：{screen}"
            );
        }
        for section in ["整趟", "当前卷", "报告"] {
            assert!(screen.contains(section), "主区少了「{section}」那一段");
        }
        // 一趟都没跑过时，主区说的是「按哪个键跑起来」。
        assert!(screen.contains("t试算"), "{screen}");
        assert!(screen.contains("x执行"), "{screen}");
    }

    /// **跑起来之后：三层只读这件事在屏上看得出来**，而不是按了没反应（本票的验收）。
    ///
    /// 三样各说一遍同一件事：抬头改口说「只读」、光标那一行不再反白、改一行的那几个键
    /// 一个都不提。反白那一格非验不可——它说的是「就在这一行上动手」，而这时按不动。
    #[test]
    fn a_run_in_progress_says_on_screen_that_the_three_layers_are_read_only() {
        let mut session = Session::new();
        let before = tight(&screen(&session, 120, 40));
        assert!(before.contains(&tight("←→ 换一个")), "{before}");
        assert!(reversed_rows(&session) > 0, "浏览时光标那一行该反白");

        session.run_started();
        assert_eq!(reversed_rows(&session), 0, "跑着时光标还反白着");
        let running = tight(&screen(&session, 120, 40));

        assert!(running.contains(&tight(READ_ONLY_TITLE)), "{running}");
        // 三层还在屏上（看得见配的是什么），只是改不动了。
        for layer in [Layer::Device, Layer::Taste, Layer::Scope] {
            assert!(running.contains(&tight(layer.title())), "{running}");
        }
        // 改一行的那几个键一个都不提。试算与执行那两个键不在这张单子上，
        // 是因为它们还印在报告区那段「还没跑过」的说明里——真会话里那一段这时早换成了
        // 攒着的报告（`live` 一起线程就有了），这里 `screen` 传的是 `None`。
        // 「跑着时按不动 t 与 x」由按键表那一条钉住（`super::state` 的
        // `which_keys_do_what_in_which_state` 第六段）。
        for keys in ["←→ 换一个", "⏎ 改", "空格 勾上"] {
            assert!(
                !running.contains(&tight(keys)),
                "{keys} 还在屏上：{running}"
            );
        }
    }

    /// **两级停按下去之后屏上说清它在等什么**（本票的验收）。
    ///
    /// 收尾那一句非说不可：按下去之后进度条照旧往前走，不说清「在等当前卷跑完」，
    /// 看上去就像那一下没按上。中止那一句说的是盘上会剩下什么。
    #[test]
    fn pressing_stop_says_what_it_is_waiting_for() {
        let mut session = Session::new();
        session.run_started();

        // 没按过：提示条上摆着那个键，按一次是收尾、再一次是中止，两级都写着。
        let idle = tight(&screen(&session, 120, 40));
        assert!(
            idle.contains(&tight("s 停（按一次收尾，再按一次中止）")),
            "{idle}"
        );

        // 按一次：收尾。屏上说清它在等当前卷跑完，也说清下一次按下去会怎样。
        session.press(Key::Char('s'));
        let finishing = tight(&screen(&session, 120, 40));
        assert!(finishing.contains(&tight("收尾中")), "{finishing}");
        assert!(
            finishing.contains(&tight("等当前卷跑完就停")),
            "{finishing}"
        );
        assert!(finishing.contains(&tight("再按一次 s 中止")), "{finishing}");

        // 再按一次：中止。说的是盘上会剩下什么——那一卷等于没做。
        session.press(Key::Char('s'));
        let aborting = tight(&screen(&session, 120, 40));
        assert!(aborting.contains(&tight("中止中")), "{aborting}");
        assert!(aborting.contains(&tight("partial 丢掉")), "{aborting}");
        // 闩到了顶，那个键从此按不动——屏上因此也不再摆它。
        assert!(!aborting.contains(&tight("再按一次 s")), "{aborting}");

        // 三级各说各的，上一行一句都不重样；没按过时下一行是空的。
        let keys: std::collections::BTreeSet<String> = [
            Instruction::Continue,
            Instruction::Finish,
            Instruction::Abort,
        ]
        .into_iter()
        .map(|pressed| running_lines(pressed)[0].to_string())
        .collect();
        assert_eq!(keys.len(), 3, "三级里有两级说了同一句：{keys:?}");
        assert_eq!(
            running_lines(Instruction::Continue)[1].to_string(),
            "",
            "没按过时不该有话说"
        );
    }

    /// 打字时屏底摆着缓冲与这一层列出来的候选。
    #[test]
    fn typing_a_path_shows_the_buffer_and_the_level_underneath() {
        let mut session = Session::new();
        session.focus_on(Field::Out);
        session.press(Key::Enter);
        for character in "库".chars() {
            session.press(Key::Char(character));
        }

        let screen = tight(&screen(&session, 120, 40));

        assert!(screen.contains("输出根库"), "{screen}");
        assert!(screen.contains("补这一层"), "{screen}");
    }

    /// **快照：一趟跑到一半，没有失败页。**
    ///
    /// 钉住的是这一票的九条验收里画得出来的那几条：全局条给出卷数与剩余时间、
    /// 当前卷条说得出在走哪一遍、一卷跑完当场显示它的判定、幂等命中说清是哪四项依据没变、
    /// 这一趟怎么读的在卷级行里看得见。
    #[test]
    fn the_main_pane_without_a_failed_page() {
        let snapshot = main_snapshot(&a_run_in_flight(false), 96, 30);

        same_screen(&snapshot, WITHOUT_A_FAILED_PAGE);
    }

    /// 见 [`the_main_pane_without_a_failed_page`]。
    const WITHOUT_A_FAILED_PAGE: &str = r#"
"┌整趟──────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 3/3 卷 [==================>           ] 3000/5000 步 · 已用 5m00s · 剩 3m20s                 │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌当前卷────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 卷三 · 第二遍 [==========>                   ] 1000/3000 步                                  │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于         │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                               │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                     │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于               │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                             │"
"│库/卷一 → 出/卷一（180 页）                                                                   │"
"│  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│库/卷二 → 出/卷二（1 页）                                                                     │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                     │"
"│  卷级 基准档 4bit · 主体 1 页 · 离群 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 ·   │"
"│离群判据 p75 立脚点、3.0× 阈值，四者均未标定）                                                │"
"│    驱动页 库/卷二/001.jpg                                                                    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│  缓存 1 页 1.0 MiB（压缩前 4.0 MiB），未溢写（预算 512.0 MiB）                               │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：同一趟，其中一卷有失败页。**
    ///
    /// 「失败页出现的当场就在主区可见，带原因」——那一段与整卷跑完之后的隔离行
    /// 并排出现，两者说的是同一份原因（一份是增量，一份是结果）。
    #[test]
    fn the_main_pane_with_a_failed_page() {
        let snapshot = main_snapshot(&a_run_in_flight(true), 96, 36);

        same_screen(&snapshot, WITH_A_FAILED_PAGE);
    }

    /// 见 [`the_main_pane_with_a_failed_page`]。
    const WITH_A_FAILED_PAGE: &str = r#"
"┌整趟──────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 3/3 卷 [==================>           ] 3000/5000 步 · 已用 5m00s · 剩 3m20s                 │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌当前卷────────────────────────────────────────────────────────────────────────────────────────┐"
"│ 卷三 · 第二遍 [==========>                   ] 1000/3000 步                                  │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"┌报告──────────────────────────────────────────────────────────────────────────────────────────┐"
"│profile kobo-libra-2：1264×1680 · 300 PPI · 16 级灰阶 · 黑白 · 阈值 5.500（盲测标定于         │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│适配方式 以高为准（宽随源比例，允许超出面板宽）                                               │"
"│裁边 按行列墨量占比 · 墨阈 200 · 行列占比 0.5%                                                │"
"│跨页拆分 跨页候选阈值 1.50 × 面板宽高比 · 装订沟定切点 · 右开（右半在先）                     │"
"│判据构成 低通后的局部均值误差 ＋ 颗粒超出 55.0 灰度级的那一部分（地板盲测标定于               │"
"│boox-poke6，其余面板未复核）                                                                  │"
"│判据聚合 分块 32×32 · 尾巴取 p99，但不宽于 8 块（K 未标定占位值）                             │"
"│库/卷一 → 出/卷一（180 页）                                                                   │"
"│  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│库/卷二 → 出/隔离/卷二（2 页）                                                                │"
"│  隔离 1 页失败：本卷整卷写到隔离目录 出/隔离/卷二，失败页以卷内统一尺寸留白占位，页序不断    │"
"│  几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动                                     │"
"│  卷级 基准档 4bit · 主体 1 页 · 离群 0 页（0.0%）· 迟滞升档 0 页（上包络 p95 · 迟滞 3 页 ·   │"
"│离群判据 p75 立脚点、3.0× 阈值，四者均未标定）                                                │"
"│    驱动页 库/卷二/001.jpg                                                                    │"
"│  介质 无寻道惩罚（固态盘） · 读取并发 8                                                      │"
"│  缓存 1 页 1.0 MiB（压缩前 4.0 MiB），未溢写（预算 512.0 MiB）                               │"
"│失败页（出现的当场，逐页那几行在整卷跑完后才有）                                              │"
"│  库/卷二/017.jpg                                                                             │"
"│    失败 解不出完整尺寸：JPEG 数据截断                                                        │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"│                                                                                              │"
"└──────────────────────────────────────────────────────────────────────────────────────────────┘"
"#;

    /// **快照：终端窄到放不下两栏时的退化。**
    ///
    /// 让的是左栏（见 [`MAIN_MIN_WIDTH`]）：主区仍留得下 [`MAIN_MIN_WIDTH`] 列，
    /// 三段一段不少。再窄到连主区都放不下时**不恐慌**——画得难看是一回事，崩掉是另一回事。
    #[test]
    fn a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane() {
        let session = Session::new();
        let live = a_run_in_flight(false);

        assert_eq!(config_width(120), CONFIG_WIDTH, "宽终端上左栏不缩");
        assert_eq!(config_width(60), 30, "窄终端上左栏让出去");
        assert_eq!(config_width(20), 0, "再窄就整个让掉");

        // 快照钉的是**整屏**：左栏让到 30 列、主区拿到 34 列，三段一段不少，
        // 而报告区那几行按显示宽度折了行。
        same_screen(
            &snapshot(|frame| shell(frame, &session, Some(&live)), 64, 18),
            TOO_NARROW_FOR_TWO_COLUMNS,
        );

        // 比左栏还窄、且高度只够画个边框：一屏都摆不下，照样不恐慌。
        snapshot(|frame| shell(frame, &session, Some(&live)), 20, 6);
        snapshot(|frame| shell(frame, &session, None), 1, 1);
    }

    /// 见 [`a_terminal_too_narrow_for_two_columns_gives_the_width_to_the_main_pane`]。
    const TOO_NARROW_FOR_TWO_COLUMNS: &str = r#"
"┌配置────────────────────────────┐┌整趟────────────────────────┐"
"│设备层 ·                        ││ 3/3 卷 [==================>│"
"│判定的依据，绑面板，改一次管很久│└────────────────────────────┘"
"│  型号                          │┌当前卷──────────────────────┐"
"│未挑（跑起来之前必填）          ││ 卷三 · 第二遍 [==========> │"
"│  感知可分辨级数                │└────────────────────────────┘"
"│默认（跟随面板）                │┌报告────────────────────────┐"
"│  阈值                          ││阈值，四者均未标定）        │"
"│跟着型号走（先挑一个）          ││    驱动页 库/卷二/001.jpg  │"
"│                                ││  介质 无寻道惩罚（固态盘） │"
"│口味层 · 这一趟的立场           ││· 读取并发 8                │"
"│  适配方式　　　　默认（height）││  缓存 1 页 1.0 MiB（压缩前 │"
"│  裁边　　　　　　默认（裁）    ││4.0 MiB），未溢写（预算     │"
"│  跨页拆分　　　　默认（拆）    ││512.0 MiB）                 │"
"└────────────────────────────────┘└────────────────────────────┘"
" ←→ 换一个 · ↑↓ 选 · t 试算 · x 执行 · q 退出                   "
"                                                                "
"                                                                "
"#;

    /// 收场之后全局条说得出这一趟是怎么收的，报告末尾那几小结也补上了。
    #[test]
    fn the_overall_bar_says_how_the_run_ended() {
        let mut live = a_run_in_flight(false);
        let mut report = live.report().clone();
        report.outcome = RunOutcome::Completed;
        live.returned(Ok(report));

        let snapshot = main_snapshot(&live, 78, 18);

        assert!(snapshot.contains("收场"), "{snapshot}");
        assert!(snapshot.contains("点名的卷都走过了"), "{snapshot}");
    }

    /// 拒绝执行的那一趟：会话不退出，把那句话画在全局条上，用户当场改。
    #[test]
    fn a_refused_run_says_why_on_the_overall_bar() {
        let mut live = Live::new(&fixture::request(RunMode::Process));
        live.returned(Err(anyhow::anyhow!("处理范围为空：至少点名一个卷")));

        let snapshot = main_snapshot(&live, 78, 10);

        assert!(snapshot.contains("没做成"), "{snapshot}");
        assert!(snapshot.contains("处理范围为空"), "{snapshot}");
    }

    /// 报告长过一格时，留下的是**最后**那几行——新添上去的那几行不该掉到格子外面。
    ///
    /// 这一条与那两张快照是一对：快照里格子够高、一行都不少，这里问格子不够高时留谁。
    #[test]
    fn a_report_taller_than_the_pane_keeps_its_last_lines() {
        let live = a_run_in_flight(true);
        let full = report_text(&live);
        let last = full.lines().next_back().expect("报告不是空的");

        // 只给四行的格子：最后那一行仍在，头一行已经让位。
        let squeezed = main_snapshot(&live, 96, 4 + BAR_HEIGHT * 2);

        assert!(squeezed.contains(last), "最新的那一行掉出去了：{squeezed}");
        assert!(
            !squeezed.contains("适配方式 以高为准"),
            "四行的格子装不下抬头，它却还在：{squeezed}"
        );
        // 一格都不剩的格子问不出滚动量，也不恐慌。
        assert_eq!(past_the_top(&full, Rect::new(0, 0, 96, 0)), 0);
        assert_eq!(past_the_top(&full, Rect::new(0, 0, 0, 10)), 0);
    }

    /// 横条的两头：一步没走是空的，走完是满的，总步数为零时不画比例。
    #[test]
    fn the_bar_fills_from_empty_to_full() {
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

    /// 快照比对。对不上就把**实际那一屏**整个印出来——改的人照着它逐行确认，
    /// 确认过了再抄回本文件。与黄金快照同一条规矩：任何一行变了都要有人当场答一句为什么。
    fn same_screen(actual: &str, expected: &str) {
        assert_eq!(
            actual,
            expected.trim_matches('\n'),
            "\n实际画出来的是：\n{actual}"
        );
    }
}
