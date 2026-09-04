//! 屏上那一块：**屏底那几行**——上一行说这时按得动的键，下一行说按下去之后会怎样
//! （ADR 0013 立的形状），末几行是要说的那句话。
//!
//! **按键提示的家只有这里**（`p1-session/10` 立的那一条）：屏上别处不摆键，
//! 一个键摆两处就是两份措辞。哪一副摆出来随会话眼下是什么状态而定，各状态的措辞
//! 在各自那个函数里，[`footer`] 那句 `match` 说的就是哪一副归哪一个。
//!
//! 这一格有多高不由本模块定：折出来几行就几行，上下限在 [`super::footer_height`]。
//! **每一行都按显示宽度折**（[`crate::wrap`]），摆不下时让位的次序见 [`footer`]。

use ratatui::text::Line;
use tonefit::Instruction;

use super::FOOTER_HEIGHT;
use super::report::expandable;
use crate::session::complete;
use crate::session::live::Live;
use crate::session::state::{Edit, Layer, Mode, Picker, Session, Shape};
use crate::session::viewport::Viewport;
use crate::wrap;

/// 试算与执行那两个键。屏上提到它们的地方都用这一句——
/// 键位改了只改这里，不必去找第二处、第三处。
///
/// 它们各自**做什么**只在报告区那一段说（见 [`super::report::report_pane`]）：
/// 那里有地方把话说完整，而这里是提示条，长了反而读不出重点。
pub(super) const START_KEYS: &str = "t 试算 · x 执行";

/// 屏底那两行：**上一行说这时按得动的键，下一行说按下去之后会怎样**（ADR 0013 立的形状）。
/// 各状态的措辞在各自那个函数里。
///
/// 打成一个类型而不是一对裸串：这一格摆不下时两半的**待遇不同**——按键那一半一行不让
/// （`q 退出` 在里面），说明那一半先让（见 [`footer`]）。一对裸串说不出这件事，
/// 调用处也看不出哪一格是哪一半。
pub(super) struct Prompt {
    /// 这时按得动的那几个键。
    pub(super) keys: String,
    /// 按下去之后它在等什么，或者这一副样子与默认那一副的差。没什么可说就是空的。
    pub(super) what: String,
}

impl Prompt {
    fn new(keys: impl Into<String>, what: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            what: what.into(),
        }
    }
}

/// 屏底：正在打字就显示缓冲与这一层列出来的候选，否则显示按键提示。末几行是要说的那句话。
///
/// **每一行都按显示宽度折**（[`crate::wrap`]）。从前这一格不折行，窄终端上从行尾切掉，
/// 而尾巴上摆的是退出——每多一个键，`q 退出` 就少露一截（停车场 Q75）。
///
/// 摆不下时**让位的次序**，从让得最早的数起：
///
/// 1. **说明那一行**（下面那一行）——它解释按下去会怎样，摆不下就等于没说，与 [`listed`]
///    让位给要说的那句话同一条规矩；
/// 2. **要说的那句话**贴着底，一行不让；
/// 3. **按键那一行折出来的几行一行不让**——`q 退出` 在里面，而不知道怎么退出是最难受的
///    一种卡住（本票的目的）。
///
/// 让完仍摆不下，这一格就往下长（见 [`super::footer_height`]）。
///
/// **屏矮到这一格也长不动时，裁的是底下**——按键那几行留在上面，要说的那句话跟着屏一起没了。
/// 那一刻这一层不再挑：屏上已经没有地方，而三样里最不能没有的是出路。
///
/// 收 `live` 只为一件事：**没有报告可展开的时候不摆展开那个键**
/// （见 [`browsing_keys`]）——屏上不摆按不动的键，那正是「按了没反应」的来源。
pub(super) fn footer(session: &Session, live: Option<&Live>, width: u16) -> Vec<Line<'static>> {
    let Prompt { keys, what } = match session.mode() {
        // 编辑一行时说明那一半摆的是**补全候选**，而「列得下几条」要等这一格分给它
        // 几行才算得出来——它因此不在这张表里拼，在下面 `room` 出来之后才补上。
        Mode::Editing(edit) => Prompt::new(editing_keys(edit), ""),
        Mode::Browsing => Prompt::new(browsing_keys(session, live), ""),
        Mode::Running(pressed) => running_prompt(*pressed, live),
        Mode::Deciding(_) => deciding_prompt(),
        Mode::Expanded(_) => expanded_prompt(),
        Mode::Picking(picker) => picking_prompt(picker),
    };
    let said = wrap::fold(session.notice().unwrap_or(""), width);
    let mut rows = wrap::fold(&keys, width);
    // 说明那一半分得到几行：按键那几行与要说的那句话先占（让位的次序见上）。
    // **只算这一次**：补全候选列得下几条按的是同一个数（见 [`listed`]）。
    let room = usize::from(FOOTER_HEIGHT).saturating_sub(rows.len().saturating_add(said.len()));
    let what = match session.mode() {
        Mode::Editing(edit) => listed(session, edit, width, room),
        _ => what,
    };
    rows.extend(wrap::fold(&what, width).into_iter().take(room));
    // 要说的那句话贴着底：中间垫空行。没有话要说时垫到 [`FOOTER_HEIGHT`] 为止，
    // 与从前那一格逐格相同。
    while rows.len() + said.len() < usize::from(FOOTER_HEIGHT) {
        rows.push(String::new());
    }
    rows.extend(said);
    rows.into_iter().map(Line::from).collect()
}

/// 跑起来之后屏底那两行：**上一行说这时按得动的键，下一行说按下去之后它在等什么**
/// （ADR 0013）。
///
/// 一张表而不是两个函数：两行随的是**同一个**取值，而屏底那一格本来就是一起画的——
/// 分成两处，改一级的措辞就要在两处对着改。
///
/// 上一行：配置这时只读（spec 的《会话：布局与交互》），因此一个改动键都不提；
/// 「只读」那件事本身写在左栏抬头上（见 [`super::config::config`]）。按到中止之后 `s` 也不提了——
/// 闩到了顶，再按一次没有更强的一级可去（`super::super::state::running_action` 在那一级上
/// 派的是「没有意义」）。**屏上不摆按不动的键**，那正是「按了没反应」的来源。
///
/// 下一行：收尾那一句非说不可——按下去之后屏上一切照旧地往前走，几千页的卷还要跑几分钟，
/// 不说清「在等当前卷跑完」，看上去就像那一下没按上。中止那一句说的是**盘上会剩下什么**。
/// 没按过时它是空的，与浏览时那一行同一个样子（那一行也是空的）。
///
/// 措辞与报告里那两句（`crate::render::outcome` 的「按停」）说的是同一件事，
/// 但时态不同：那两句是收场之后的结果，这两句是此刻在等的事。
pub(super) fn running_prompt(pressed: Instruction, live: Option<&Live>) -> Prompt {
    let [keys, waiting] = match pressed {
        Instruction::Continue => [
            "s 停（按一次收尾，再按一次中止）· Ctrl-C 退出会话（当前卷中止，盘上不留半卷）",
            resuming_line(live),
        ],
        Instruction::Finish => [
            "再按一次 s 中止 · Ctrl-C 退出会话",
            "收尾：等当前卷跑完就停，剩下的卷一个都不开工；盘上只留完整的卷，下一趟幂等接着走",
        ],
        Instruction::Abort => [
            "Ctrl-C 退出会话",
            "中止：当前卷停在这一页上，它那格 partial 丢掉——那一卷等于没做，最终位置上一个字节都没动过",
        ],
    };
    Prompt::new(
        // 行首那一截与全局条那一格的抬头同一个出处（见 [`stopping_name`]）。
        // 没按过时它是「跑着」——那不是按停的一级，因此不在那张表里。
        format!(" {}…… · {keys}", stopping_name(pressed).unwrap_or("跑着")),
        if waiting.is_empty() {
            String::new()
        } else {
            format!(" {waiting}")
        },
    )
}

/// 还没按过停的时候，屏底第二行说的那件事：**这一趟在决策点上怎么走**
/// （ADR 0012 决定第 3 条，`p1-session/14`、`volume-discovery/07`）。
///
/// 两句都要在**跑起来的当口**说，不能等到停下来才说：
///
/// - **续做那一趟**要预告它会停：每一卷跑到第二遍之前都不走了，不预告的话，
///   横条停住看上去与卡住没有分别。答话那三个键连同「一卷一次」一起预告出来——
///   几十卷的一趟里，「还要按几下」是用户当场就想知道的那件事。
/// - **答过「剩下的卷都这样」之后**要说清它不再问了：往下的决策点当场照那个默认答案答掉
///   （`super::super::run::Gate`），横条从此一路走到底。不说的话，「它怎么不问了」
///   与「它忘了问」在屏上没有分别。
///
/// 执行那一趟与还没跑过时这一行是空的，与从前逐格相同：那两种没有「续不续做」可言。
fn resuming_line(live: Option<&Live>) -> &'static str {
    let Some(live) = live else {
        return "";
    };
    if !live.resumes() {
        return "";
    }
    if live.for_the_rest().is_some() {
        return "剩下的卷都这样：往下的决策点不再停下来问，这一趟一路做到底";
    }
    "续做：每一卷第一遍走完都会停下来等你拿主意——那时按 x 接着做第二遍（第一遍不重算），按 a 剩下的卷都这样，按 s 收尾"
}

/// **停在决策点上等人拿主意**时屏底那两行（`p1-session/14`、`volume-discovery/07`，
/// ADR 0012）。
///
/// 上一行是这时按得动的四个键，下一行说**此刻这一卷是什么样**——决策点问的是
/// 「这一卷的第二遍还做不做」，而答这一问要知道的正是「这一卷现在还什么都没写」。
///
/// **说的是这一卷，不是输出根**：一趟里每一卷各停一次，答过继续的那几卷早就写出去了
/// （`volume-discovery/07`）。说成「输出根一个字节都没有」的话，
/// 第二卷停下来的那一刻它就是一句假话。
///
/// 三个答话键各带一句它买的东西：`x` 那一句是**第一遍不重算**（续做整件事就是为了它），
/// `a` 那一句是**往下不再问**（几十卷的一趟按一下就挂得住），
/// `s` 那一句是**等价于 dry-run**（`CONTEXT.md` 的《会话》：决策点）。
/// 措辞里不提「收尾」那一级的定义——那是按停的第一级，说的是「当前卷跑完才停」，
/// 与这里停出来的现场恰好相反（见 `super::super::state::deciding_action`）。
///
/// **`s` 那一句还得说出「剩下的卷也不开工」**：决策点上答的字照样进库那一侧的闩
/// （`CONTEXT.md` 的《会话》：「那一卷停在这儿之后，剩下的卷也不必开工」，
/// 用例见 `tests/resume.rs` 的
/// `finishing_at_one_volume_decision_point_leaves_the_earlier_volumes_whole_and_starts_no_more`）。
/// 一卷的时候那件事说不说都一样；五十卷的时候它是这个键**最大的后果**，
/// 而只说「这一卷不写」的话，它读起来像是「跳过这一卷」。
fn deciding_prompt() -> Prompt {
    Prompt::new(
        " 等你拿主意…… · x 接着做第二遍（第一遍不重算）· a 剩下的卷都这样（往下不再问）· s 收尾（这一卷不写，等价 dry-run；剩下的卷也不开工）· Ctrl-C 退出会话",
        " 上面那份报告是真的：判定、逐页结果、缓存用量都算出来了，只有第二遍一步没走——这一卷此刻一个字节都没写",
    )
}

/// 按停按到的那一级**叫什么**。没按过就没有名字——那不是按停的一级。
///
/// **屏上提到它的两处都用这一个**：屏底那一行的行首（[`running_prompt`]），
/// 与全局条那一格的抬头（[`super::bars::overall_bar`]，停车场 Q71）。
/// 两处说的是同一件事，措辞因此只有这一处。
pub(super) fn stopping_name(pressed: Instruction) -> Option<&'static str> {
    match pressed {
        Instruction::Continue => None,
        Instruction::Finish => Some("收尾中"),
        Instruction::Abort => Some("中止中"),
    }
}

/// 展开之后屏底那两行：**上一行说这时按得动的键，下一行说这一副样子与默认那一副的差**。
///
/// 收起那个键要一直摆着：左栏此刻不在屏上，而「收起来的东西回得来」只有它说得出
/// （票面的验收：收起后能一键回到配置）。
///
/// **展开的是第几卷不在这里说**，那个数在报告区那一格的抬头上
/// （见 [`super::report::report_title`]）——挨着它说的那一卷，而这里是按键提示的家。
/// 一个数摆两处就是两份措辞，与按停那一级同一条规矩（见 [`stopping_name`]）。
///
/// 下一行说的是**不折行**这件事：屏窄的时候行尾会被切掉，
/// 不说清「横着滚得动」，看上去就是报告缺了半截。
fn expanded_prompt() -> Prompt {
    Prompt::new(
        " ↑↓ 翻一行 · ←→ 横着滚 · ⇥／⇧⇥ 换下一卷／上一卷 · e／Esc 收起，左栏回来 · q 退出",
        " 逐页那两行不折行：屏窄时行尾被切掉，往右滚就看得到——页面不会跟着整体错位",
    )
}

/// 预设那一栏屏底那两行：**上一行说这时按得动的键**，下一行说这一栏与三层的关系。
///
/// 上一行随光标停在哪一行而变，与浏览时同一条（见 [`browsing_keys`]）：停在一份预设上
/// 是套用它——**把名字摆进那句话里**，因为套上去之后两层整个换掉，而那不可撤销；
/// 停在末尾那一行上是打一个名字存下来。
///
/// **`d` 只在停着一份预设时摆出来**：那一行不是预设时它按不动（见
/// `super::super::state::listing_action`），而屏上不摆按不动的键。删要按两下，
/// 而第一下问的那句话走的是屏底那句要说的话（[`Session::ask_before_erasing`]）——
/// 与撞名那一问同一条路，按键这一行因此不必为它改口。
///
/// 打名字那一副照编辑一行的样子（见 [`editing_prompt`]）：缓冲加一句按键提示。
/// 下一行这时说的是**存出去的是哪两层**——范围层不进预设是这一栏最要紧的一条性质
/// （票面第三条），而用户按下 `⏎` 之前唯一会读的就是屏底这两行。
fn picking_prompt(picker: &Picker) -> Prompt {
    let Some(naming) = picker.naming() else {
        let [keys, what] = match picker.picked() {
            Some(name) => [
                format!(" ↑↓ 选 · ⏎ 套用「{name}」 · d 删掉 · p／Esc 回配置 · q 退出"),
                // 套用把两层**整个**换掉，包括眼下配好的那几项——那一下不可撤销，
                // 因此在按下去之前说，与覆盖那一句同一条规矩。
                " 套用把设备层与口味层整个换成那一份（它没说的那几项跟着回到「默认」），\
                 眼下配好的两层随之丢掉；范围层不动"
                    .to_owned(),
            ],
            None => [
                " ↑↓ 选 · ⏎ 打个名字存下来 · p／Esc 回配置 · q 退出".to_owned(),
                " 存的是设备层与口味层。范围层（输出根与卷）不进预设".to_owned(),
            ],
        };
        return Prompt::new(keys, what);
    };
    Prompt::new(
        format!(" 预设名 {}▏   ⏎ 存下 · Esc 回列表", naming.buffer),
        " 存的是设备层与口味层。范围层（输出根与卷）不进预设，套用时因此写不到上一次的目录去",
    )
}

/// 编辑一行时按键那一行：缓冲加这时按得动的几个键。
///
/// 下面那一行（这一层列出来的候选）不在这里拼——它要等 [`footer`] 算出这一格
/// 分给它几行（见 [`listed`]）。
fn editing_keys(edit: &Edit) -> String {
    let keys = match edit.field.shape() {
        Shape::Path => "⇥ 补这一层 · ⏎ 收下 · Esc 丢掉",
        _ => "⏎ 收下 · Esc 丢掉",
    };
    format!(" {} {}▏   {keys}", edit.field.label(), edit.buffer)
}

/// 补全列出来的那一层，摆在屏底。空着就说一句这一层还没列过。
///
/// **列得下几条列几条，剩下几条说出来。** 从前这里硬性只列 12 条，第 13 条起
/// 没有任何交代——一层里有三十个目录时，屏上说的是「这一层有十二个东西」。
///
/// 「列得下几条」按**这一格真有多宽、分得到几行**（`room`，[`footer`] 算的那一个）：
/// 一条一条往上加，加到摆不下为止。加不进去的那几条由 [`Viewport`] 数出来
/// （`hidden`），与别处的「还有多少没露面」是同一份实现。
///
/// **这一处没有光标、也没有滚动条**，理由与别处的分别见 [`Viewport`] 那张表——
/// 「还有 N 条」就是它说这件事的方式。
///
/// 只列打到的那一层，且**只是列出来**：不留索引、不留缓存（ADR 0009）。
fn listed(session: &Session, edit: &Edit, width: u16, room: usize) -> String {
    if edit.candidates.is_empty() {
        // 有话要说时这一行让位——那句话就印在下一行。
        return match session.notice() {
            Some(_) => String::new(),
            None => " 按 ⇥ 列出这一层".to_owned(),
        };
    }
    // 只留这一层里的那个名字，切法在 `complete` 那一侧——分隔符表只有一份。
    let names: Vec<&str> = edit
        .candidates
        .iter()
        .map(|hit| complete::name(hit))
        .collect();
    let view = Viewport::new(names.len(), fitting(&names, width, room), 0);
    spelled(&names[..view.shown()], view.hidden())
}

/// 这一格摆得下几条候选：**一条一条往上加，加到折出来的行数超出 `room` 为止**。
///
/// 折的是 [`spelled`] 拼出来的那一整行，走的是 [`crate::wrap`]——屏底那一格真正折行
/// 的也是它（见 [`footer`]），两处因此不会一处说摆得下、另一处画不出来。
///
/// **先砍一刀再逐条试**：一条候选最少占三格（一个字加两个空格的间隔），
/// 这一格顶天摆得下 `宽 × 行 / 3` 条。一层里有上千个名字是常事，
/// 而逐条试一遍是平方的——砍掉之后每一帧最多试几十次。
fn fitting(names: &[&str], width: u16, room: usize) -> usize {
    let ceiling = usize::from(width).saturating_mul(room) / 3 + 1;
    let mut fits = 0;
    for take in 1..=names.len().min(ceiling) {
        if wrap::fold(&spelled(&names[..take], names.len() - take), width).len() > room {
            break;
        }
        fits = take;
    }
    fits
}

/// 候选那一行的写法：列出来的那几条，外加**没露面的还剩几条**。
///
/// 「还有 N 条」只在真有剩的时候说：一层里就那么几个东西时多这么一句是噪音。
fn spelled(names: &[&str], left: usize) -> String {
    let listed = names.join("  ");
    match left {
        0 => format!(" 这一层：{listed}"),
        left => format!(" 这一层：{listed}  …还有 {left} 条"),
    }
}

/// 浏览时的按键提示，随光标停的那一行而变——按不动的键不该印在屏上。
///
/// 试算与执行两个键**每一行上都在**：它们与光标停在哪儿无关，
/// 而「配好了之后按哪个键」是这一屏上最该一直看得见的事。
///
/// **展开那个键只在有卷可展开时才摆**（见 [`expandable`]）：一趟都没跑过时按下去
/// 只换来一句话，而摆一个只会说「还没跑过」的键与「屏上不摆按不动的键」相左。
///
/// **预设那个键每一行上都在**，与试算和执行同一条：存的是整两层，与光标停在哪儿无关。
/// 它挤进来时展开那个键从「展开逐页」缩成「展开」：这一行长起来之后在窄终端上要折成两行
/// （见 [`footer`]），而键少一个就少折一截。缩写不是为了「摆得下」——摆不下的那一半
/// 从前是从行尾切掉的，而尾巴上摆的正是退出（停车场 Q75），眼下折得开了。
/// 「展开」与报告区抬头上那句「展开 卷二（第 2/2 卷）」是同一个词，缩了也认得出。
///
/// **出标定图那个键只在设备层那三行上摆**（会话批的 13 号票）：它在别的层上根本不派动作
/// （`super::super::state::Session::browsing_action`），摆出来就是一个按不动的键。
/// 它挨着行内那个动作、排在通用的那几个键之前，因为它与它们是同一类——
/// **这一行上按得动什么**，而不是「这一屏上按得动什么」。
///
/// 型号还没挑时它照样摆着：按下去说的是「先挑型号」，与 `t`／`x` 那时的待遇一样。
/// 那不是按不动，是**按了有话说**。
fn browsing_keys(session: &Session, live: Option<&Live>) -> String {
    let focus = session.focus();
    let expand = if expandable(live) { " · e 展开" } else { "" };
    let chart = if focus.layer() == Layer::Device {
        " · c 出标定图"
    } else {
        ""
    };
    let common = format!("↑↓ 选 · {START_KEYS}{expand} · p 预设 · q 退出");
    match focus.shape() {
        // 只有这两副落得到设备层上（`Field::shape` 与 `Field::layer`：型号是环，
        // 灰阶数与阈值是打字改的），标定图那个键因此只插在这两支里。
        Shape::Cycle => format!(" ←→ 换一个{chart} · {common}"),
        Shape::Text => format!(" ⏎ 改{chart} · {common}"),
        // 底下两副恒落在范围层上（输出根、「＋ 再打一个卷进来」、卷行），
        // 插进去也永远是空的——摆一个点不着的洞，改的人迟早当它是活的。
        Shape::Path => format!(" ⏎ 打一个路径进来（⇥ 逐层补全）· {common}"),
        Shape::Volume => format!(" 空格 勾上／勾掉 · d 删掉这一条 · {common}"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::probe::{screen, tight};
    use super::*;
    use crate::session::live::{Reach, Resuming, fixture};
    use crate::session::state::{Field, Key};
    use tonefit::Mode as RunMode;

    /// **两级停按下去之后屏上说清它在等什么**（本票的验收）。
    ///
    /// 收尾那一句非说不可：按下去之后进度条照旧往前走，不说清「在等当前卷跑完」，
    /// 看上去就像那一下没按上。中止那一句说的是盘上会剩下什么。
    #[test]
    fn pressing_stop_says_what_it_is_waiting_for() {
        let mut session = Session::new();
        session.run_started();

        // 没按过：提示条上摆着那个键，按一次是收尾、再一次是中止，两级都写着。
        let idle = tight(&screen(&mut session, None, 120, 40));
        assert!(
            idle.contains(&tight("s 停（按一次收尾，再按一次中止）")),
            "{idle}"
        );

        // 按一次：收尾。屏上说清它在等当前卷跑完，也说清下一次按下去会怎样。
        session.press(Key::Char('s'));
        let finishing = tight(&screen(&mut session, None, 120, 40));
        assert!(finishing.contains(&tight("收尾中")), "{finishing}");
        assert!(
            finishing.contains(&tight("等当前卷跑完就停")),
            "{finishing}"
        );
        assert!(finishing.contains(&tight("再按一次 s 中止")), "{finishing}");

        // 再按一次：中止。说的是盘上会剩下什么——那一卷等于没做。
        session.press(Key::Char('s'));
        let aborting = tight(&screen(&mut session, None, 120, 40));
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
        .map(|pressed| running_prompt(pressed, None).keys)
        .collect();
        assert_eq!(keys.len(), 3, "三级里有两级说了同一句：{keys:?}");
        assert_eq!(
            running_prompt(Instruction::Continue, None).what,
            "",
            "没按过时不该有话说"
        );
    }

    /// **试算在跑起来的当口就预告它会逐卷停下来**（`p1-session/14` 票面第四条，
    /// `volume-discovery/07`）。
    ///
    /// 非说不可：横条会在每一卷的第二遍之前停住，而停住与卡住在屏上没有分别。
    /// 「一卷一次」与答话那三个键一起预告出来——几十卷的一趟里，
    /// 「还要按几下」是用户当场就想知道的那件事。
    ///
    /// **答过「剩下的卷都这样」之后换一句**：往下不再问了，而「它怎么不问了」
    /// 与「它忘了问」在屏上同样没有分别。
    ///
    /// 执行那一趟这一行仍旧是空的：它没有「续不续做」可言，与从前逐格相同。
    #[test]
    fn a_trial_says_it_will_stop_at_every_volume_while_it_runs() {
        // 试算：预告它会停下来，一卷一次，三个键都摆出来。
        let mut resuming = Live::new(&fixture::request(RunMode::Process), Resuming::Waits);
        resuming.run_started(20, 20_000);
        let said = running_prompt(Instruction::Continue, Some(&resuming)).what;
        assert!(said.contains("续做"), "{said}");
        assert!(said.contains("每一卷"), "{said}");
        for key in ["x 接着做第二遍", "a 剩下的卷都这样", "s 收尾"] {
            assert!(said.contains(key), "{key}：{said}");
        }

        // 答过「剩下的卷都这样」：换成「往下不再问」那一句。
        resuming.decide(Instruction::Continue, Reach::ForTheRest);
        let said = running_prompt(Instruction::Continue, Some(&resuming)).what;
        assert!(said.contains("剩下的卷都这样"), "{said}");
        assert!(said.contains("不再停下来问"), "{said}");
        assert!(!said.contains("等你拿主意"), "{said}");

        // 执行：这一行空着。
        let processing = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        assert_eq!(
            running_prompt(Instruction::Continue, Some(&processing)).what,
            "",
            "执行那一趟不该多说一句"
        );
    }

    /// **出标定图那个键只摆在设备层那三行上，而它说的那两行屏上都在**（13 号票）。
    ///
    /// 两半各是一条性质：**摆不摆**（屏上不摆按不动的键）与**说得下说不下**
    /// （屏底那一格恒三行，说两行就让掉一行提示）。后者非验不可——那两行里一行是路径，
    /// 挤成一行就会被切掉，而「图在哪儿」正是用户此刻唯一要读的东西。
    #[test]
    fn the_chart_key_sits_on_the_device_layer_and_what_it_says_fits() {
        let mut session = Session::new();

        // 设备层那三行上都摆着它。
        for field in [Field::Profile, Field::GrayLevels, Field::Threshold] {
            session.focus_on(field);
            let screen = tight(&screen(&mut session, None, 120, 40));
            assert!(screen.contains(&tight("c 出标定图")), "{field:?}：{screen}");
        }
        // 别的两层上不摆：它在那儿根本不派动作。
        for field in [Field::Filter, Field::Out] {
            session.focus_on(field);
            let screen = tight(&screen(&mut session, None, 120, 40));
            assert!(
                !screen.contains(&tight("c 出标定图")),
                "{field:?}：{screen}"
            );
        }

        // 出完图说的那两行**都在屏上**：图在哪儿，以及此刻要做对的那一件事。
        session.focus_on(Field::Profile);
        session.charted(Path::new("图/tonefit-calibration-boox-poke6-16-levels.png"));
        let screen = tight(&screen(&mut session, None, 120, 40));
        assert!(
            screen.contains(&tight("tonefit-calibration-boox-poke6-16-levels.png")),
            "{screen}"
        );
        assert!(screen.contains(&tight("以原尺寸打开")), "{screen}");
        // 让掉的是提示那一行里的空行，按得动的那几个键仍在。
        assert!(screen.contains(&tight("c 出标定图")), "{screen}");
    }

    /// **补全候选：列得下几条列几条，剩下多少条说得出来**（本票的验收第三条）。
    ///
    /// 从前这里硬性只列 12 条，第 13 条起没有任何交代——一层下面有四十个目录时，
    /// 屏上说的是「这一层有十二个东西」。眼下列的是这一格真摆得下的那几条，
    /// 没露面的那些由那一套视口数出来（见 [`listed`]）。
    ///
    /// **这一处没有滚动条**：它列而不选，一个键都不派，「还有 N 条」就是它说这件事的方式。
    #[test]
    fn the_completion_candidates_fill_the_room_and_say_how_many_are_left() {
        let session = Session::new();
        let edit = Edit {
            field: Field::Out,
            buffer: "库/".to_owned(),
            candidates: (1..=40).map(|at| format!("库/第{at:02}卷/")).collect(),
        };
        let names = |line: &str| line.matches("卷").count();

        // 宽终端上一行摆得下的比十二条多——那条硬上限撤掉之后它就列得出来。
        let wide = listed(&session, &edit, 120, 2);
        assert!(names(&wide) > 12, "还卡在十二条上：{wide}");
        assert!(wide.contains("还有"), "没说还剩多少条：{wide}");

        // 说得出的那个数与列出来的那几条对得上：两者加起来就是这一层的全部。
        let left: usize = wide
            .rsplit_once("还有 ")
            .and_then(|(_, tail)| tail.trim_end_matches(" 条").parse().ok())
            .unwrap_or_else(|| panic!("「还有 N 条」没说出一个数来：{wide}"));
        assert_eq!(
            names(&wide) + left,
            40,
            "列出来的加上剩下的不是全部：{wide}"
        );

        // 窄终端上列得少——这一格真摆得下几条就是几条，而剩下的照旧说得出来。
        let narrow = listed(&session, &edit, 40, 2);
        assert!(
            names(&narrow) < names(&wide),
            "窄终端上列得一样多：{narrow}"
        );
        assert!(narrow.contains("还有"), "没说还剩多少条：{narrow}");

        // 一层下面就那么几个东西时不多说一句：没有剩下的，「还有」二字就是噪音。
        let few = Edit {
            candidates: vec!["库/第01卷/".to_owned(), "库/第02卷/".to_owned()],
            ..edit.clone()
        };
        let all = listed(&session, &few, 120, 2);
        assert!(!all.contains("还有"), "全列出来了还说剩下几条：{all}");

        // 屏底那一格一行都匀不出来时：一条都不列，而这一句仍旧算得出来、不恐慌。
        let none = listed(&session, &edit, 120, 0);
        assert!(none.contains("还有 40 条"), "{none}");
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

        let screen = tight(&screen(&mut session, None, 120, 40));

        assert!(screen.contains("输出根库"), "{screen}");
        assert!(screen.contains("补这一层"), "{screen}");
    }
}
