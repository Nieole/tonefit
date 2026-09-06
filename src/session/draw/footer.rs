//! 屏上那一块：**屏底那几行**——上一行说这时按得动的键，下一行说按下去之后会怎样
//! （ADR 0013 立的形状），末几行是要说的那句话。
//!
//! **「此刻按什么」的家只有这里**（`p1-session/10` 立的那一条）：屏上别处不摆
//! 「这一刻该按哪个键」。哪一副摆出来随会话眼下是什么状态而定，各状态摆哪几件事
//! 在各自那个函数里，[`footer`] 那句 `match` 说的就是哪一副归哪一个。
//!
//! # 这一行与 `?` 那张表分的是哪一刀
//!
//! 屏上有两处摆键，而**它们答的不是同一个问题**：
//!
//! | | 屏底这一行 | [`super::overlay`] 那张表 |
//! |---|---|---|
//! | 答的是 | **此刻按什么**——最常用的四五件事，末尾恒是「全部键」 | **这个键做什么**——按焦点分组的全部键 |
//! | 键从哪来 | [`Session::keys_here`]：眼下这一块 | [`Session::key_table`]：每一块 |
//! | 措辞取 | [短的那一句](super::keys::Says::short) | [长的那一句](super::keys::Says::long) |
//!
//! **两处的键与措辞出自同一处**（`p4-parking-lot/07`，停车场 Q166、Q180）：
//! 这一行从前是各状态那几个函数里手写的字面串——同一个键因此有两句措辞
//! （这一行上是 `t 试算`，那一张上是 `t 试算：只算不写，报告照出`），
//! 改一个键位要动两处，而只动一处不会有任何一条用例红；取值栏那两层上三个键同义，
//! 而这一行只摆得出其中两个。**问出来之后那两笔都不存在**。
//!
//! **这一层挑的是动作，不是键**：「就在这一行上动手」「试算」「退出」——
//! 派得出它的是哪几个键、那一句怎么说，一律由 [`Asked`] 问出来。
//!
//! 这一格有多高不由本模块定：折出来几行就几行，上下限在 [`super::yielding::footer_height`]。
//! **每一行都按显示宽度折**（[`crate::wrap`]），摆不下时让位的次序见 [`footer`]。

use ratatui::text::Line;
use tonefit::Instruction;

use super::keys;
use super::report::expandable;
use super::yielding::FOOTER_HEIGHT;
use crate::session::complete;
use crate::session::live::{Live, Reach};
use crate::session::state::{
    Action, Covered, Edit, Focus, Follow, Key, KeyGroup, Listing, Overlay, Picker, Session, Stage,
    Step, Values,
};
use crate::session::viewport::Viewport;
use crate::wrap;

/// 屏底那一行**问按键表问出来的那一份**：[眼下这一块上派得出动作的每一个键](Session::keys_here)，
/// 连同[这是屏上哪一块](KeyGroup)与[这一趟走到哪个阶段](Stage)——措辞随这两样变。
///
/// 打成一个类型而不是三个参数：各状态那几个函数每一条提示都要问它，
/// 三个参数一路串下去，读的人得自己认出这三样是同一份东西。
pub(super) struct Asked<'a> {
    /// 眼下这一块上派得出动作的每一个键，连同它派什么。
    here: Vec<(Key, Action)>,
    /// 屏上这一块是哪一组：措辞随它变（同一个 `Esc` 在取值栏上是「一格不改地退一步」、
    /// 编辑到一半是「丢掉」）。
    group: KeyGroup,
    /// 这一趟走到哪个阶段：按停那个键与退出会话那一句随它变。
    stage: Stage,
    /// 那一趟攒着的那一份。**只给「屏上摆不摆这一条」用**——按键表答不出
    /// 「报告里此刻有没有卷」（见 [`report_prompt`]）。
    live: Option<&'a Live>,
}

impl<'a> Asked<'a> {
    fn new(session: &Session, live: Option<&'a Live>) -> Self {
        Self {
            here: session.keys_here(),
            group: KeyGroup::of(session.focus()),
            stage: session.stage(),
            live,
        }
    }

    /// **这一行上的一条**：派得出这件事的那几个键，加上短的那一句。
    /// 一个键都派不出来就没有这一条——屏上不摆按不动的键。
    fn on(&self, want: impl Fn(Action) -> bool) -> Option<String> {
        keys::prompt(self.group, self.stage, &self.here, want)
    }

    /// **就在这一行上动手**那一条：随光标停的那一行而变（摊开／改／打一个路径／勾上），
    /// 它是这一屏上唯一随行而变的那一件事——而屏底那一行的家就是「这一刻按什么」。
    fn hands_on(&self) -> Option<String> {
        self.on(|action| {
            matches!(
                action,
                Action::Unfold | Action::Edit | Action::Toggle | Action::Take
            )
        })
    }

    /// **退出会话**那一条。跑着与等答话时只剩 `Ctrl-C`（`q`／`Esc` 那时按不动，
    /// 停车场 Q63），而那一句还要说出盘上会剩下什么——两件事都由按键表与
    /// [措辞那一处](super::keys::says)答，这里不分岔。
    fn quit(&self) -> Option<String> {
        self.on(|action| action == Action::Quit)
    }

    /// **末尾那一条：全部键**。掀得开的每一块上恒在末尾（`p3-session-legibility/12`
    /// 票面第一条）——键变多之后这一行摆不下全部，而摆不下的那几个在屏上等于不存在。
    ///
    /// **打字那两块上它是 `F1`**（[`Key::F1`]，`p4-parking-lot/07` 票面第三条）：
    /// 那儿 `?` 是一个字，进的是缓冲。**掀着的那一张自己不摆**——那一刻那个键是「关掉」，
    /// 按键表因此答的是[退一步](Action::Cancel)，这一条自然就空了。
    fn all_keys(&self) -> Option<String> {
        self.on(|action| action == Action::Reveal(Overlay::Keys))
    }
}

/// 屏底那两行：**上一行说这时按得动的键，下一行说按下去之后会怎样**（ADR 0013 立的形状）。
/// 各状态摆哪几件事在各自那个函数里。
///
/// 打成一个类型而不是一对裸串：这一格摆不下时两半的**待遇不同**——按键那一半一行不让
/// （退出会话在里面），说明那一半先让（见 [`footer`]）。一对裸串说不出这件事，
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

    /// 按键那一行：**摆得出来的那几条挨着拼**，中间一个 `·`。
    /// 摆不出来的那几条（[`Asked::on`] 给的 `None`）连同它们的分隔符一起没有——
    /// 屏上不摆按不动的键，那正是「按了没反应」的来源。
    fn listing(parts: impl IntoIterator<Item = Option<String>>, what: impl Into<String>) -> Self {
        let listed: Vec<String> = parts.into_iter().flatten().collect();
        Self::new(format!(" {}", listed.join(" · ")), what)
    }
}

/// 屏底：正在打字就显示缓冲与这一层列出来的候选，否则显示按键提示。末几行是要说的那句话。
///
/// **每一行都按显示宽度折**（[`crate::wrap`]）。从前这一格不折行，窄终端上从行尾切掉，
/// 而尾巴上摆的是退出——每多一个键，退出那一条就少露一截（停车场 Q75）。
///
/// 摆不下时**让位的次序**，从让得最早的数起：
///
/// 1. **说明那一行**（下面那一行）——它解释按下去会怎样，摆不下就等于没说，与 [`listed`]
///    让位给要说的那句话同一条规矩；
/// 2. **要说的那句话**贴着底，一行不让；
/// 3. **按键那一行折出来的几行一行不让**——退出会话在里面，而不知道怎么退出是最难受的
///    一种卡住（`p1-session/10` 的目的）。
///
/// 让完仍摆不下，这一格就往下长（见 [`super::yielding::footer_height`]）。
///
/// **屏矮到这一格也长不动时，裁的是底下**——按键那几行留在上面，要说的那句话跟着屏一起没了。
/// 那一刻这一层不再挑：屏上已经没有地方，而三样里最不能没有的是出路。
///
/// 收 `live` 只为一件事：**报告里此刻有没有卷**（见 [`report_prompt`]）——
/// 按键表答不出那一问，而屏上不摆按不动的键。
///
/// # 这一行只摆最常用的几件事，其余在 `?` 那张表里
///
/// **末尾恒是全部键那一条**（见 [`Asked::all_keys`]）：**那一条在这里接**，
/// 不在下面各状态那几个函数里各接一遍——一处接完，「恒在末尾」才是结构上成立的。
pub(super) fn footer(session: &Session, live: Option<&Live>, width: u16) -> Vec<Line<'static>> {
    let asked = Asked::new(session, live);
    let Prompt { keys, what } = match session.focus() {
        // 编辑一行时说明那一半摆的是**补全候选**，而「列得下几条」要等这一格分给它
        // 几行才算得出来——它因此不在这里拼，在下面 `room` 出来之后才补上。
        Focus::Editing(edit) => Prompt::new(editing_keys(&asked, edit), ""),
        Focus::Config => config_prompt(&asked),
        Focus::Report => report_prompt(&asked, session),
        Focus::Opened(_) => opened_prompt(&asked, session),
        Focus::Expanded(_) => expanded_prompt(&asked, session),
        Focus::Picking(picker) => picking_prompt(&asked, picker),
        Focus::Valuing(values) => valuing_prompt(&asked, values),
        Focus::Overlaid(covered) => overlaid_prompt(&asked, covered),
    };
    let keys = match asked.all_keys() {
        Some(all) => format!("{keys} · {all}"),
        None => keys,
    };
    let said = wrap::fold(session.notice().unwrap_or(""), width);
    let mut rows = wrap::fold(&keys, width);
    // 说明那一半分得到几行：按键那几行与要说的那句话先占（让位的次序见上）。
    // **只算这一次**：补全候选列得下几条按的是同一个数（见 [`listed`]）。
    let room = usize::from(FOOTER_HEIGHT).saturating_sub(rows.len().saturating_add(said.len()));
    let what = match session.focus() {
        Focus::Editing(edit) => listed(&asked, session, edit, width, room),
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

/// **阶段那一维此刻摆得出的那几条**（ADR 0017），连同下一行说的那句话：
/// 跑着是按停那一副，等答话是答话那一副。没跑过与收场了这一维一个键都不派，因此是 `None`。
///
/// 焦点那一维上摆得下它的那几块都拼它——**按停与答话那三个键在哪一块上都按得动**
/// （`super::super::state::stage_action`），而屏上不摆按不动的键的另一半是
/// **按得动的键要摆出来**。
///
/// **退出会话那一条不在这里**：它在每一块上都摆（见 [`Asked::quit`]），
/// 而这一维只管「这一趟此刻在等什么」。
fn stage_parts(asked: &Asked) -> Option<(Vec<Option<String>>, String)> {
    match asked.stage {
        Stage::Running(pressed) => Some(running_parts(asked, pressed)),
        Stage::Deciding(_) => Some(deciding_parts(asked)),
        Stage::Fresh | Stage::Ended => None,
    }
}

/// 这一块自己那几条，加上[阶段那一维那几条](stage_parts)、退出会话与全部键。
///
/// **次序是一处定的**：这一块自己的键在前，阶段那几个跟在后面，出路收尾——
/// 六块因此长一个样子，读的人不必在每一块上重新找退出在哪儿。
fn with_stage(
    asked: &Asked,
    mut parts: Vec<Option<String>>,
    otherwise: impl Into<String>,
) -> Prompt {
    let what = match stage_parts(asked) {
        Some((stage, what)) => {
            parts.extend(stage);
            what
        }
        None => otherwise.into(),
    };
    parts.push(asked.quit());
    Prompt::listing(parts, what)
}

/// **焦点落在左栏时**屏底那两行。
///
/// 跑着与等答话时这一块自己只剩一个键：`⇥`——它不改三层里的任何一格，
/// 而几十分钟的一趟里回头看第一卷正是这一下（ADR 0017）。**那一条不必在这里分岔**：
/// 三层只读的时候改动那几件事的键根本不派，问出来就是空的。
///
/// 留下的这几件事各有理由：
///
/// - **就在这一行上动手**（[`Asked::hands_on`]）是这一屏上唯一随行而变的那一件；
/// - **试算与执行**与光标停在哪儿无关，而「配好了之后按哪个键」是这一屏上最该
///   一直看得见的事；
/// - **`⇥ 报告区` 只在上一趟收场之后才摆**：一趟都没跑过时那个键根本不派动作
///   （`super::super::state::Session::browsing_action`），问出来就没有这一条；
/// - **退出会话一行不让**（停车场 Q75）。
///
/// 就地转一格那一副（`←→`）`p3-session-legibility/12` 之后归 `?` 那张表——
/// 这一行瘦身成最常用的几件事，而两条路改的是同一格。
fn config_prompt(asked: &Asked) -> Prompt {
    with_stage(
        asked,
        vec![
            asked.hands_on(),
            asked.on(|action| matches!(action, Action::Start(_))),
            asked.on(|action| matches!(action, Action::Focus(_))),
        ],
        String::new(),
    )
}

/// **焦点落在报告区时**屏底那两行（`p3-session-legibility/10`）。
///
/// 行首是**这一块的名字**：焦点在哪儿屏上要看得出来，而卷表上那一行反白说得出这件事的
/// 前提是**表上真有一卷**——一趟跑到第一卷收摊之前它一行都没有（见 [`super::table`]），
/// 那时说得出焦点在哪的只剩这一行。
///
/// **选一枝与展开那几条只在有卷可选时才摆**：一卷都没有时按下去无处可去，
/// 而屏上不摆按不动的键。**这一问按键表答不出来**（它读不到那一趟攒下来的东西），
/// 因此由这一层拿 `live` 挡一道——挡的是「摆不摆这一条」，键与措辞照旧问出来。
///
/// **`g` 只在跟随停了时摆**：跟随着的时候按它一格不变（`CONTEXT.md` 的《会话》：跟随），
/// 而「跟随此刻停没停」屏上另有一处说（报告区那一格的抬头，见
/// [`super::report::report_title`]）——这里只摆那个键。
///
/// **[这一趟的前提](Overlay::Premises)那一条摆在这一块上**
/// （`p3-session-legibility/12` 票面第三条）：那几行从前就摆在卷表**上方**
/// （`p3-session-legibility/08`），读报告的人正是在这一块上找它们的。别的几块上它不摆
/// ——屏底那一行只摆此刻最常用的几件事，而 `?` 那张表上它照旧在「任何时候」那一组里。
/// **一趟都没跑过时它根本不派**（停车场 Q167），这一层因此不必再挡一道。
fn report_prompt(asked: &Asked, session: &Session) -> Prompt {
    let stopped = matches!(session.follow(), Follow::Stopped(_));
    let mut parts = vec![Some("报告区".to_owned())];
    if expandable(asked.live) {
        parts.push(asked.on(|action| matches!(action, Action::Select(_))));
        parts.push(asked.on(|action| action == Action::Open));
        parts.push(asked.on(|action| action == Action::Expand));
    }
    if stopped {
        parts.push(asked.on(|action| action == Action::Follow));
    }
    parts.push(asked.on(|action| action == Action::Reveal(Overlay::Premises)));
    parts.push(asked.on(|action| matches!(action, Action::Focus(_))));
    with_stage(asked, parts, following_line(stopped))
}

/// **展开着一枝时**屏底那两行（`volume-discovery/08`）。
///
/// 与[目录表那一副](report_prompt)同一个形状，差的只有三处：这一块列的是**一卷一行**，
/// 因此那两条说的是「选一卷」与「展开逐页」；出路多一个 `Esc`——
/// 这一级是展开进来的，退一步该退到刚才那一级去（按键表那一头见 [`Focus::Opened`]）。
///
/// [这一趟的前提](Overlay::Premises)那一条照旧摆在这一块上：读报告的人在这两块上
/// 找的是同一件事。
fn opened_prompt(asked: &Asked, session: &Session) -> Prompt {
    let stopped = matches!(session.follow(), Follow::Stopped(_));
    let mut parts = vec![Some("卷表".to_owned())];
    if expandable(asked.live) {
        parts.push(asked.on(|action| matches!(action, Action::Select(_))));
        parts.push(asked.on(|action| action == Action::Expand));
    }
    if stopped {
        parts.push(asked.on(|action| action == Action::Follow));
    }
    parts.push(asked.on(|action| action == Action::Reveal(Overlay::Premises)));
    parts.push(asked.on(|action| action == Action::Collapse));
    parts.push(asked.on(|action| matches!(action, Action::Focus(_))));
    with_stage(asked, parts, following_line(stopped))
}

/// 报告区那一行底下说的那件事：**跟随此刻是什么样**（`CONTEXT.md` 的《会话》：跟随）。
///
/// 跑着与等答话时它让位给阶段那一维那一句（见 [`stage_parts`]）：那一句说的是此刻在等
/// 什么，比这一句急。**「跟随停了」屏上因此另有一处常驻**——报告区那一格的抬头，
/// 那一处不随阶段让位。
fn following_line(stopped: bool) -> &'static str {
    match stopped {
        true => " 跟随停了：报告再长，光标也不动——g 把它交回给最新那一卷",
        false => " 跟随着最新的那一卷：一卷收摊，光标就落到它上面",
    }
}

/// 跑起来之后阶段那一维摆出来的那几条：**上一行说这时按得动的键，下一行说它在等什么**
/// （ADR 0013）。
///
/// 一处出两半而不是两个函数：两行随的是**同一个**取值，而屏底那一格本来就是一起画的——
/// 分成两处，改一级的措辞就要在两处对着改。
///
/// 上一行：配置这时只读（spec 的《会话：布局与交互》），因此一个改动键都不派；
/// 「只读」那件事本身写在左栏抬头上（见 [`super::config::config`]）。按到中止之后
/// 按停那个键也不摆了——闩到了顶，再按一次没有更强的一级可去
/// （`super::super::state::running_action` 在那一级上派的是「没有意义」），
/// 而**屏上不摆按不动的键**：这一条不必在这里判，问出来就是空的。
///
/// 下一行：收尾那一句非说不可——按下去之后屏上一切照旧地往前走，几千页的卷还要跑几分钟，
/// 不说清「在等当前卷跑完」，看上去就像那一下没按上。中止那一句说的是**盘上会剩下什么**。
/// 没按过时它是空的，与浏览时那一行同一个样子（那一行也是空的）。
///
/// 措辞与报告里那两句（`crate::render::outcome` 的「按停」）说的是同一件事，
/// 但时态不同：那两句是收场之后的结果，这两句是此刻在等的事。
fn running_parts(asked: &Asked, pressed: Instruction) -> (Vec<Option<String>>, String) {
    let waiting = match pressed {
        Instruction::Continue => resuming_line(asked.live),
        Instruction::Finish => {
            "收尾：等当前卷跑完就停，剩下的卷一个都不开工；盘上只留完整的卷，下一趟幂等接着走"
        }
        Instruction::Abort => {
            "中止：当前卷停在这一页上，它那格 partial 丢掉——那一卷等于没做，最终位置上一个字节都没动过"
        }
    };
    (
        vec![
            // 行首那一截与总览块那一格的抬头同一个出处（见 [`stopping_name`]）。
            // 没按过时它是「跑着」——那不是按停的一级，因此不在那张表里。
            Some(format!("{}……", stopping_name(pressed).unwrap_or("跑着"))),
            asked.on(|action| action == Action::Stop),
        ],
        match waiting.is_empty() {
            true => String::new(),
            false => format!(" {waiting}"),
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

/// **停在决策点上等人拿主意**时阶段那一维摆出来的那几条（`p1-session/14`、
/// `volume-discovery/07`，ADR 0012）。
///
/// 上一行是这时按得动的那三个键，下一行说**此刻这一卷是什么样**——决策点问的是
/// 「这一卷的第二遍还做不做」，而答这一问要知道的正是「这一卷现在还什么都没写」。
///
/// **说的是这一卷，不是输出根**：一趟里每一卷各停一次，答过继续的那几卷早就写出去了
/// （`volume-discovery/07`）。说成「输出根一个字节都没有」的话，
/// 第二卷停下来的那一刻它就是一句假话。
///
/// 三个答话键各带一句它买的东西（措辞见 [`super::keys::says`]）：`x` 那一句是
/// **第一遍不重算**（续做整件事就是为了它），`a` 那一句是**往下不再问**，
/// `s` 那一句是**等价于 dry-run**（`CONTEXT.md` 的《会话》：决策点）。
fn deciding_parts(asked: &Asked) -> (Vec<Option<String>>, String) {
    (
        vec![
            Some("等你拿主意……".to_owned()),
            // **三个各摆一条，次序是这一层挑的**：`x` 在先——续做整件事就是为了它，
            // 而按键表问出来的次序是**字母序**（`a` `s` `x`），把主路那一个摆到了末尾。
            // 挑的是动作，键与措辞照旧问出来。
            asked.on(|action| {
                matches!(action, Action::Answer(Instruction::Continue, Reach::ThisVolume))
            }),
            asked.on(|action| {
                matches!(
                    action,
                    Action::Answer(Instruction::Continue, Reach::ForTheRest)
                )
            }),
            asked.on(|action| matches!(action, Action::Answer(Instruction::Finish, _))),
        ],
        " 上面那份报告是真的：判定、逐页结果、缓存用量都算出来了，只有第二遍一步没走——这一卷此刻一个字节都没写"
            .to_owned(),
    )
}

/// 跑着的那一副屏底两行。**只给用例用**——真会话里它由 [`footer`] 从
/// [`Asked`] 拼出来，而用例问的是「按停按到这一级时那两行说什么」，
/// 拼一个会话出来那几步不是它要说的事。
///
/// 按到哪一级由 `s` 按几次说了算（两级停是同一个键按两次，ADR 0013）：
/// 这里照那条规矩把会话推到那一级去，不另拼一个状态。
#[cfg(test)]
pub(super) fn running_prompt(pressed: Instruction, live: Option<&Live>) -> Prompt {
    let mut session = Session::new();
    session.run_started();
    for _ in 0..match pressed {
        Instruction::Continue => 0,
        Instruction::Finish => 1,
        Instruction::Abort => 2,
    } {
        session.press(Key::Char('s'));
    }
    assert_eq!(session.stage(), Stage::Running(pressed), "没推到那一级上");
    let asked = Asked::new(&session, live);
    let (mut parts, what) = running_parts(&asked, pressed);
    parts.push(asked.quit());
    Prompt::listing(parts, what)
}

/// 按停按到的那一级**叫什么**。没按过就没有名字——那不是按停的一级。
///
/// **屏上提到它的两处都用这一个**：屏底那一行的行首（[`running_parts`]），
/// 与总览块那一格的抬头（[`super::overview::overview`]，停车场 Q71）。
/// 两处说的是同一件事，措辞因此只有这一处。
pub(super) fn stopping_name(pressed: Instruction) -> Option<&'static str> {
    match pressed {
        Instruction::Continue => None,
        Instruction::Finish => Some("收尾中"),
        Instruction::Abort => Some("中止中"),
    }
}

/// 展开之后屏底那两行：**上一行说这时按得动的键，下一行说这一副此刻列着哪几页**。
///
/// 收起那一条要一直摆着：左栏此刻不在屏上，而「收起来的东西回得来」只有它说得出。
/// **它回的是报告区**——展开是从那一块进去的（光标停着的那一卷，ADR 0017），
/// 而左栏跟着回到屏上，再一个 `⇥` 就站得上去。
///
/// **展开的是第几卷不在这里说**，那个数在报告区那一格的抬头上
/// （见 [`super::report::report_title`]）；这一副列着几页同理，那一句钉在这一格顶上
/// （见 [`super::pages::pages`]）。这里是按键提示的家，一个数都不摆第二遍——
/// 与按停那一级同一条规矩（见 [`stopping_name`]）。
///
/// **换一副列法那一条等答话时不摆**：那一刻 `a` 是[「剩下的卷都这样」](Action::Answer)
/// （见 `super::super::state::expanded_action`），而屏上不摆按不动的键——
/// 这一条同样不必在这里判，问出来就是空的。摆出来的那一句说的是**按过去是哪一副**，
/// 不是「切换」：一个 toggle 说不出去哪儿。
///
/// **换一卷这里只摆 `⇥`**（`p3-session-legibility/12` 的瘦身）：`⇧⇥` 是它的另一头，
/// 而这一行摆的是最常用的那几件事——两头都在 `?` 那张表上。
fn expanded_prompt(asked: &Asked, session: &Session) -> Prompt {
    let listing = session
        .expansion()
        .map(|expansion| expansion.listing)
        .unwrap_or_default();
    with_stage(
        asked,
        vec![
            asked.on(|action| matches!(action, Action::Move(_))),
            asked.on(|action| matches!(action, Action::List(_))),
            asked.on(|action| action == Action::Turn(Step::Next)),
            asked.on(|action| action == Action::Collapse),
        ],
        listed_pages(listing),
    )
}

/// 展开那一副底下说的那件事：**这一副列的是哪几页**（`CONTEXT.md` 的《会话》：要紧的页）。
///
/// 只列要紧的那一档要把那六种数出来：屏上一页一个词说得出它要紧在哪儿，
/// 但「一共有哪几种算要紧」在别处一个字都没有。判据的出处是
/// [`crate::render::notable`]，这一句是它在屏上的说法。
fn listed_pages(listing: Listing) -> &'static str {
    match listing {
        Listing::Notable => {
            " 只列要紧的页：特例 · 失败 · 部分救回 · 几何门不成立 · 宽溢出 · 兜底上界，加上定档页"
        }
        Listing::All => " 列着全部页：要紧的那几页照旧靠行首记号跳出来",
    }
}

/// 预设那一栏屏底那两行：**上一行说这时按得动的键**，下一行说这一栏与三层的关系。
///
/// 上一行随光标停在哪一行而变，与浏览时同一条（见 [`Asked::hands_on`]）：停在一份预设上
/// 是套用它，停在末尾那一行上是打一个名字存下来。**`d` 只在停着一份预设时摆出来**：
/// 那一行不是预设时它按不动（见 `super::super::state::listing_action`），
/// 而屏上不摆按不动的键——这一条问出来就是空的。
///
/// **套的是哪一份写在下一行上**：套上去之后两层整个换掉，而那不可撤销——
/// 按下去之前要读到的是那一份的名字连同这一下的后果，两样摆在一起才说得清。
/// 删要按两下，而第一下问的那句话走的是屏底那句要说的话
/// （[`Session::ask_before_erasing`]）——与撞名那一问同一条路，按键这一行因此不必为它改口。
///
/// 打名字那一副照编辑一行的样子（见 [`editing_keys`]）：缓冲加一句按键提示。
/// 下一行这时说的是**存出去的是哪两层**——范围层不进预设是这一栏最要紧的一条性质，
/// 而用户按下 `⏎` 之前唯一会读的就是屏底这两行。
fn picking_prompt(asked: &Asked, picker: &Picker) -> Prompt {
    let Some(naming) = picker.naming() else {
        let what = match picker.picked() {
            Some(name) => format!(
                " 套用「{name}」：设备层与口味层整个换成那一份（它没说的那几项跟着回到「默认」），\
                 眼下配好的两层随之丢掉；范围层不动"
            ),
            None => " 存的是设备层与口味层。范围层（输出根与卷）不进预设".to_owned(),
        };
        return Prompt::listing(
            vec![
                asked.on(|action| matches!(action, Action::Move(_))),
                asked.hands_on(),
                asked.on(|action| action == Action::Erase),
                asked.on(|action| action == Action::Cancel),
                asked.quit(),
            ],
            what,
        );
    };
    let Prompt { keys, .. } = Prompt::listing(
        vec![
            asked.on(|action| action == Action::Store),
            asked.on(|action| action == Action::Cancel),
        ],
        "",
    );
    Prompt::new(
        format!(" 预设名 {}▏  {keys}", naming.buffer),
        " 存的是设备层与口味层。范围层（输出根与卷）不进预设，套用时因此写不到上一次的目录去",
    )
}

/// **取值栏摊着时**屏底那两行（`CONTEXT.md` 的《会话》：取值栏）。
///
/// 上一行的行首是**摊开的是哪一行**：那一列摊在左栏里，而屏窄到左栏让出宽度那一档上
/// 它整个不在屏上——不说一句，屏上就没有一处答得出「这一列是谁的」。
/// 措辞取那一行自己的标签（`Field::label`），不另编一份。
///
/// **`←→` 摆出来**：它们在这个状态下归这一列（`→` 与 `⏎` 同义、`←` 与 `Esc` 同义，
/// 见 `super::super::state::valuing_action`），而屏上不摆按不动的键的另一半是
/// **按得动的键要摆出来**——不摆的话，习惯了 `←→` 就地转一格的人在这里会以为它们没了。
/// **空格也在里面**：那三个键同义，摆哪几个由按键表答，不再手抄（停车场 Q180）。
///
/// 下一行说的是**第一格那件事**：「没说」与「说了一个恰好等于默认的值」在屏上长得像，
/// 而两者的差别要到存成预设那一刻才看得见（`CONTEXT.md` 的《会话》：
/// 存出去的只有「说了的那几项」）。看不见的差别用户改不动，所以在这儿说。
///
/// **型号那一行的两层各有一副**（`CONTEXT.md` 的《会话》：下钻）：
///
/// - **面板那一层**上停在一块面板上时那一下是**进去看**，不是定——面板不是型号
///   那一行的一个取值（`super::super::state::Values::at_a_panel`）；停在第一格「没挑」上时
///   它照旧是定，措辞因此随光标停的那一格换（按键表那一头就是这么分岔的），
///   屏上不摆一个这一格上按不动的键。下一行说的是**为什么摊开的是面板**：
///   设备只是面板的别名，面板相同的型号输出完全一致。
/// - **下钻那一层**上行首多印**进的是哪一块面板**（`tonefit::Panel` 自己的写法，
///   会话不另编一份）：这一列此刻列的是型号名，不说一句就没有一处答得出这几个是哪块屏的。
///   **退一步在这一层上退回的是面板那一层**，而那一句由[措辞那一处](super::keys::says)
///   一句话盖住两层——摊在屏上的是哪一层，行首这一截已经说了。
fn valuing_prompt(asked: &Asked, values: &Values) -> Prompt {
    let label = values.field().label();
    let head = match values.panel() {
        Some(panel) => format!("{label} · {panel}"),
        None => label.to_owned(),
    };
    let what = match (values.panel().is_some(), values.at_a_panel()) {
        (true, _) => {
            " 这块面板底下的型号输出完全一致，挑哪一个都一样；换掉型号会把标定出来的灰阶数与阈值清空"
        }
        (false, true) => {
            " 设备只是面板的别名，面板相同的型号输出完全一致——\
             内置表里没有你那台设备时，挑一个面板相同的顶上，再按实测填感知可分辨级数"
        }
        (false, false) => {
            " 第一格是「没说」：它跟着默认值走，存成预设时那一项不写进去——\
             与「说了一个恰好等于默认的值」是两件事，后者往后默认改了也仍是那个值"
        }
    };
    Prompt::listing(
        vec![
            Some(head),
            asked.on(|action| matches!(action, Action::Move(_))),
            asked.on(|action| matches!(action, Action::Choose | Action::Drill)),
            asked.on(|action| action == Action::Cancel),
            asked.quit(),
        ],
        what,
    )
}

/// 编辑一行时按键那一行：缓冲加这时按得动的那几件事。
///
/// **`⇥ 补这一层` 只有路径项摆得出来**：别的行没有「下一层」可补，那个键在那儿不派动作
/// （见 `super::super::state::editing_action`）——这一条问出来就是空的。
///
/// 下面那一行（这一层列出来的候选）不在这里拼——它要等 [`footer`] 算出这一格
/// 分给它几行（见 [`listed`]）。
fn editing_keys(asked: &Asked, edit: &Edit) -> String {
    let Prompt { keys, .. } = Prompt::listing(
        vec![
            asked.on(|action| action == Action::Complete),
            asked.on(|action| action == Action::Commit),
            asked.on(|action| action == Action::Cancel),
        ],
        "",
    );
    format!(" {} {}▏  {keys}", edit.field.label(), edit.buffer)
}

/// 补全列出来的那一层，摆在屏底。空着就说一句这一层还没列过——
/// **而只在那个键真派得出动作的行上说**：`⇥` 只有[路径项](crate::session::state::Shape::Path)
/// 补得动（见 `super::super::state::editing_action`），改一个文本项时按它一个动作都不派。
/// 那一句从前无条件摆着，屏底上一行不摆 `⇥`、下一行却劝人按它——
/// 而屏上不摆按不动的键（`p4-parking-lot/07`，评审提的）。
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
fn listed(asked: &Asked, session: &Session, edit: &Edit, width: u16, room: usize) -> String {
    if edit.candidates.is_empty() {
        // 有话要说时这一行让位——那句话就印在下一行。
        // 补不动的行上同样不说（见上）：问的是按键表，与按键那一行同一处。
        return match (
            session.notice(),
            asked.on(|action| action == Action::Complete),
        ) {
            (Some(_), _) | (None, None) => String::new(),
            (None, Some(_)) => " 按 ⇥ 列出这一层".to_owned(),
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

/// **一张覆盖层掀着时**屏底那两行（`p3-session-legibility/12`）。
///
/// 上一行是这一块上派得出的**全部**键：这一块自己那几个（`↑↓` 读、`Esc` 关、
/// 另一张那个键换过去），加上**阶段那一维那几个**——按停与答话在覆盖层掀着时
/// 照样按得动（`p4-parking-lot/06`，见 `super::super::state::overlay_action`）。
///
/// **`Esc 关` 说清它回哪儿去**：覆盖层**盖住**一块焦点、不替掉它，而「刚才那一块」
/// 此刻不在屏上——不说一句，屏上没有一处答得出关掉之后会到哪儿。
///
/// **另一张那个键摆在这里**：两张是同一副形状，换一张不必先关掉这一张。
/// **一趟都没跑过时[前提那一张](Overlay::Premises)不摆**——那时它一个字都印不出来，
/// 而按键表在那个阶段上根本不派它（停车场 Q167）。
/// **掀着的那一张自己那个键也不摆**（[`Asked::all_keys`]）：那一刻它是「关掉」。
///
/// **`q` 不在这一行上**：这一块上它不派动作（那一块的「退一步」是 `Esc`），
/// 而屏上不摆按不动的键。出路一个不少——`Esc` 关掉就回到刚才那一块，
/// `Ctrl-C` 照旧在每一个状态下都是退出，而它由[退出那一条](Asked::quit)摆出来。
///
/// 下一行说的是**这一张是什么**：`?` 那一张要说清它只列此刻这个阶段派得出的键
/// （屏上不摆按不动的键在这一张上也成立），前提那一张要说清它为什么不在卷表上方。
/// **跑着与等答话时让给阶段那一维那一句**：按停买的是什么、答话那三个各答什么，
/// 是那一刻屏上最要紧的一句——与[展开着那一副](expanded_prompt)同一条让法。
fn overlaid_prompt(asked: &Asked, covered: &Covered) -> Prompt {
    with_stage(
        asked,
        vec![
            asked.on(|action| matches!(action, Action::Move(_))),
            asked.on(|action| action == Action::Cancel),
            asked.on(|action| action == Action::Reveal(Overlay::Premises)),
        ],
        what_is_on(covered.overlay),
    )
}

/// 覆盖层那一格底下说的那件事：**这一张是什么**。
fn what_is_on(overlay: Overlay) -> &'static str {
    match overlay {
        Overlay::Keys => {
            " 只列此刻这个阶段派得出的键，按焦点分组——屏底那一行摆的是最常用的几个，这里是全部"
        }
        Overlay::Premises => {
            " 这一趟的前提：一趟只说一次，因此不占卷表那几行——它们说的是这一份报告是照哪几条算出来的"
        }
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

    /// **取值栏摊着时屏底摆的是这一列的键，摊得开的行上摆的是「⏎ 摊开」**。
    ///
    /// 三件事：
    ///
    /// - 转得动的行上屏底摆的是**摊开那一条**；就地转一格那一条（`←→`）
    ///   `p3-session-legibility/12` 之后归 `?` 那张表——屏底瘦身成最常用的几个，
    ///   而两条路改的是同一格（那一条性质由按键表钉着，不靠屏底这一行说）；
    /// - 摊开之后屏底换成这一列的键，**同义的那几个一个不漏**（`⏎`／空格／`→`
    ///   三个键同义，而从前这一行只摆得出其中两个——停车场 Q180），
    ///   而**「一格不改」写在按键那一行上**：`Esc` 买的是什么，用户按下去之前就该读得到；
    /// - 下一行说清**第一格那件事**——「没说」与「说了一个恰好等于默认的值」的分别
    ///   只有存成预设时才看得见，而看不见的差别用户改不动。
    #[test]
    fn the_unfolded_values_put_their_own_keys_on_the_bottom_row() {
        let mut session = Session::new();
        session.go_to(Field::Filter);
        let browsing = tight(&screen(&mut session, None, 120, 40));
        assert!(browsing.contains(&tight("⏎ 空格 摊开取值")), "{browsing}");
        // 屏底瘦身之后 `←→` 不在这一行上，而它照旧按得动——`?` 那张表列着它
        // （见 `super::super::overlay`）。
        assert!(!browsing.contains(&tight("←→ 换一个")), "{browsing}");

        session.press(Key::Enter);
        let unfolded = tight(&screen(&mut session, None, 120, 40));
        assert!(
            unfolded.contains(&tight(Field::Filter.label())),
            "{unfolded}"
        );
        assert!(unfolded.contains(&tight("↑ ↓ j k 选")), "{unfolded}");
        assert!(unfolded.contains(&tight("→ ⏎ 空格 定")), "{unfolded}");
        assert!(
            unfolded.contains(&tight("← Esc 一格不改地退一步")),
            "{unfolded}"
        );
        assert!(unfolded.contains(&tight("第一格是「没说」")), "{unfolded}");

        // **型号那一行也摊得开**：转得动的行一律摊得开，而它多一层下钻
        // （见 [`the_two_levels_of_the_model_row_each_put_their_own_keys_up`]）。
        session.press(Key::Esc);
        session.go_to(Field::Profile);
        let profile = tight(&screen(&mut session, None, 120, 40));
        assert!(profile.contains(&tight("⏎ 空格 摊开取值")), "{profile}");
        // 末尾恒是它（`p3-session-legibility/12` 票面第一条）：屏底摆的是最常用的几个，
        // 全部键在一个键之外。**`F1` 与它并成一行**——两个键派的是同一件事
        // （`p4-parking-lot/07` 票面第三条）。
        assert!(profile.contains(&tight("? F1 全部键")), "{profile}");
    }

    /// **型号那两层各把自己的键摆出来**（`CONTEXT.md` 的《会话》：下钻）。
    ///
    /// 屏上不摆按不动的键，另一半是**这一格上按下去会怎样，按之前就该读得到**：
    /// 同一个 `⏎` 停在一块面板上是进去看，停在「没挑」上是定，下钻那一层上是定。
    /// 三副措辞因此不同。
    ///
    /// 下钻那一层还多印一样：**进的是哪一块面板**。那一列此刻列的是型号名，
    /// 而屏窄到左栏让出宽度那一档上左栏整个不在场——不说一句，屏上就没有一处答得出
    /// 这几个型号是哪块屏的。**退一步那一句两层共用**（`p4-parking-lot/07`）：
    /// 一句话盖住两层（下钻进去之后退回的是面板那一层），而摊在屏上的是哪一层
    /// 由行首那一截说。
    #[test]
    fn the_two_levels_of_the_model_row_each_put_their_own_keys_up() {
        let mut session = Session::new();
        session.go_to(Field::Profile);
        session.press(Key::Enter);

        // 面板那一层，光标停在第一格「没挑」上：`⏎` 是定。
        let unsaid = tight(&screen(&mut session, None, 120, 40));
        assert!(unsaid.contains(&tight("→ ⏎ 空格 定")), "{unsaid}");
        assert!(
            unsaid.contains(&tight("← Esc 一格不改地退一步")),
            "{unsaid}"
        );

        // 挪到一块面板上：同一个键换了意思，措辞跟着换。
        session.press(Key::Down);
        let panels = tight(&screen(&mut session, None, 120, 40));
        assert!(
            panels.contains(&tight("→ ⏎ 空格 看这块面板底下的型号")),
            "{panels}"
        );
        assert!(!panels.contains(&tight("→ ⏎ 空格 定")), "{panels}");
        assert!(panels.contains(&tight("设备只是面板的别名")), "{panels}");

        // 下钻进去：`⏎` 是定，而 `Esc` 退回的是面板那一层——照实写。
        session.press(Key::Right);
        let panel = session.valuing().expect("没下钻").panel().expect("没进去");
        let inside = tight(&screen(&mut session, None, 120, 40));
        assert!(inside.contains(&tight(&panel.to_string())), "{inside}");
        assert!(inside.contains(&tight("→ ⏎ 空格 定")), "{inside}");
        assert!(
            inside.contains(&tight("← Esc 一格不改地退一步")),
            "{inside}"
        );
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
        assert!(
            finishing.contains(&tight("s 再按一次就中止")),
            "{finishing}"
        );

        // 再按一次：中止。说的是盘上会剩下什么——那一卷等于没做。
        session.press(Key::Char('s'));
        let aborting = tight(&screen(&mut session, None, 120, 40));
        assert!(aborting.contains(&tight("中止中")), "{aborting}");
        assert!(aborting.contains(&tight("partial 丢掉")), "{aborting}");
        // 闩到了顶，那个键从此按不动——屏上因此也不再摆它。
        assert!(!aborting.contains(&tight("再按一次就中止")), "{aborting}");

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

    /// **出标定图那个键只在设备层那三行上列得出来，而它说的那两行屏上都在**（13 号票）。
    ///
    /// 两半各是一条性质：**列不列**（屏上不摆按不动的键）与**说得下说不下**
    /// （屏底那一格恒三行，说两行就让掉一行提示）。后者非验不可——那两行里一行是路径，
    /// 挤成一行就会被切掉，而「图在哪儿」正是用户此刻唯一要读的东西。
    ///
    /// **前一半此刻问的是 `?` 那张表**（`p3-session-legibility/12`）：屏底那一行瘦身之后
    /// 只摆最常用的四五个，这个键归覆盖层那一张——而那一张列的是按键表当场问出来的，
    /// 「它在别的层上不派动作」因此在屏上照旧读得出来。
    #[test]
    fn the_chart_key_sits_on_the_device_layer_and_what_it_says_fits() {
        let mut session = Session::new();

        // 设备层那三行上都列得出它。
        for field in [Field::Profile, Field::GrayLevels, Field::Threshold] {
            session.go_to(field);
            session.press(Key::Char('?'));
            let screen = tight(&screen(&mut session, None, 120, 60));
            assert!(
                screen.contains(&tight("按这块面板出一张标定图")),
                "{field:?}：{screen}"
            );
            session.press(Key::Esc);
        }
        // 别的两层上一处都没有：它在那儿根本不派动作。
        for field in [Field::Filter, Field::Out] {
            session.go_to(field);
            session.press(Key::Char('?'));
            let screen = tight(&screen(&mut session, None, 120, 60));
            assert!(
                !screen.contains(&tight("按这块面板出一张标定图")),
                "{field:?}：{screen}"
            );
            session.press(Key::Esc);
        }

        // 出完图说的那两行**都在屏上**：图在哪儿，以及此刻要做对的那一件事。
        session.go_to(Field::Profile);
        session.charted(Path::new("图/tonefit-calibration-boox-poke6-16-levels.png"));
        let screen = tight(&screen(&mut session, None, 120, 40));
        assert!(
            screen.contains(&tight("tonefit-calibration-boox-poke6-16-levels.png")),
            "{screen}"
        );
        assert!(screen.contains(&tight("以原尺寸打开")), "{screen}");
        // 让掉的是提示那一行里的空行，按键那一行仍在（退出会话一行不让，停车场 Q75）。
        assert!(screen.contains(&tight("q 退出")), "{screen}");
        assert!(screen.contains(&tight("? F1 全部键")), "{screen}");
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
        let mut session = Session::new();
        // 真在编辑那一行上：这一格空着时说不说那一句要问按键表
        // （`⇥` 只有路径项补得动，见 [`listed`]）。
        session.go_to(Field::Out);
        session.press(Key::Enter);
        let asked = Asked::new(&session, None);
        let edit = Edit {
            field: Field::Out,
            buffer: "库/".to_owned(),
            candidates: (1..=40).map(|at| format!("库/第{at:02}卷/")).collect(),
        };
        let names = |line: &str| line.matches("卷").count();

        // 宽终端上一行摆得下的比十二条多——那条硬上限撤掉之后它就列得出来。
        let wide = listed(&asked, &session, &edit, 120, 2);
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
        let narrow = listed(&asked, &session, &edit, 40, 2);
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
        let all = listed(&asked, &session, &few, 120, 2);
        assert!(!all.contains("还有"), "全列出来了还说剩下几条：{all}");

        // 屏底那一格一行都匀不出来时：一条都不列，而这一句仍旧算得出来、不恐慌。
        let none = listed(&asked, &session, &edit, 120, 0);
        assert!(none.contains("还有 40 条"), "{none}");
    }

    /// 打字时屏底摆着缓冲与这一层列出来的候选。
    ///
    /// **补全那个键只在路径项上摆**（`p4-parking-lot/07`，评审提的）：`⇥` 在别的行上
    /// 一个动作都不派（`super::super::state::editing_action`），而屏上不摆按不动的键
    /// ——按键那一行与底下那句「按 ⇥ 列出这一层」问的是同一处，两处一起不摆。
    #[test]
    fn typing_a_path_shows_the_buffer_and_the_level_underneath() {
        let mut session = Session::new();
        session.go_to(Field::Out);
        session.press(Key::Enter);
        for character in "库".chars() {
            session.press(Key::Char(character));
        }

        let path = tight(&screen(&mut session, None, 120, 40));

        assert!(path.contains("输出根库"), "{path}");
        assert!(path.contains("补这一层"), "{path}");
        assert!(path.contains(&tight("按 ⇥ 列出这一层")), "{path}");

        // 打字改的行：那个键补不动，两处一起不摆。
        session.press(Key::Esc);
        session.go_to(Field::CacheBudget);
        session.press(Key::Enter);
        let text = tight(&screen(&mut session, None, 120, 40));
        assert!(text.contains("缓存预算"), "{text}");
        assert!(!text.contains("补这一层"), "{text}");
        assert!(!text.contains(&tight("按 ⇥ 列出这一层")), "{text}");
    }
}
