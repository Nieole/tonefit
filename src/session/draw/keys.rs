//! 屏上摆键的那两处共用的一份：**一个键怎么写**，以及**它派的那件事怎么说**。
//!
//! 屏上有两处摆键，而它们答的不是同一个问题——
//!
//! | | [屏底那一行](super::footer) | [`?` 那张表](super::overlay) |
//! |---|---|---|
//! | 答的是 | **此刻按什么**：最常用的四五件事 | **这个键做什么**：按焦点分组的全部键 |
//! | 措辞取 | [短的那一句](Says::short) | [长的那一句](Says::long) |
//!
//! **两份措辞只有这一处出处**（`p4-parking-lot/07` 票面第二条，停车场 Q166）：
//! 从前屏底那一行的键是各状态那几个函数里手写的字面串，同一个键因此有两句措辞
//! （屏底 `t 试算`，覆盖层 `t 试算：只算不写，报告照出`），改一个键位要动两处，
//! 而只动一处不会有任何一条用例红。
//!
//! **键同样不在这里列**：两边都把[按键表](crate::session::state::Session::action)
//! 问出来的 `(Key, Action)` 交到 [`merged`]——屏底问的是
//! [眼下这一块](crate::session::state::Session::keys_here)，
//! `?` 那张表问的是[每一块](crate::session::state::Session::key_table)。
//! **画法这一层因此一个键都不自己列。**

use crate::session::state::{Action, Key, KeyGroup, Listing, Pane, Stage, Step};
use tonefit::{Instruction, Mode as RunMode};

use crate::session::live::Reach;

/// 一件事在屏上的**两句措辞**：短的给屏底那一行，长的给 `?` 那张表。
///
/// 打成一个类型而不是一对裸串：两句在调用处看不出哪一句是哪一句，而它们
/// **摆的地方不同、长度的理由也不同**——屏底那一格摆不下解释，
/// 而一张只有提示的表读不出所以然。
pub(super) struct Says {
    /// 屏底那一行上那一句：**提示**。一行摆四五件事，长了就把出路挤下去。
    pub(super) short: &'static str,
    /// `?` 那张表上那一句：**解释**。那一张是拿来扫的，一行一件事，摆得下。
    pub(super) long: &'static str,
}

impl Says {
    /// 长短两份不同的那几件事。
    fn two(short: &'static str, long: &'static str) -> Self {
        Self { short, long }
    }

    /// **长短一样**的那几件事：一句话本来就短，硬拆成两份只会有一处忘了跟着改。
    fn same(both: &'static str) -> Self {
        Self {
            short: both,
            long: both,
        }
    }
}

/// 取哪一句。**不是一个裸 `bool`**：调用处一个 `true` 说不出取的是哪一份
/// （与 [`Listing`] 同一条理由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Wording {
    /// 屏底那一行取的那一句。
    Short,
    /// `?` 那张表取的那一句。
    Long,
}

/// **派得出同一句话的那几个键并成一行**，次序照它们在按键表上被问到的次序。
///
/// 并的依据是**屏上那句话**，不是动作本身：`↑` 派的是「往上挪一格」、`↓` 派的是
/// 「往下挪一格」，两个动作，而屏上它们是同一件事（`↑ ↓ j k 在三层上挪一行`）。
/// 照动作并的话，屏上一半的行是同一句话的两半。
///
/// **同义的那几个键因此一个都不漏**：取值栏上 `⏎`／空格／`→` 三个键同义，
/// 而屏底那一行从前只摆得出其中两个——空格按得动却不在屏上（停车场 Q180）。
/// 摆哪几个由这一处答，不再手抄。
pub(super) fn merged(
    group: KeyGroup,
    stage: Stage,
    keys: &[(Key, Action)],
    wording: Wording,
) -> Vec<(String, &'static str)> {
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    for (key, action) in keys {
        let said = says(group, stage, *action);
        let what = match wording {
            Wording::Short => said.short,
            Wording::Long => said.long,
        };
        match rows.iter_mut().find(|(_, already)| *already == what) {
            Some((spelt, _)) => {
                spelt.push(' ');
                spelt.push_str(&spelled(*key));
            }
            None => rows.push((spelled(*key), what)),
        }
    }
    rows
}

/// **屏底那一行上的一条**：派得出这件事的那几个键，加上短的那一句。
///
/// `here` 是[眼下这一块上派得出动作的每一个键](crate::session::state::Session::keys_here)，
/// `want` 是屏底那一层挑的**动作**——屏底只摆此刻最常用的几件事，而挑的是
/// 「就在这一行上动手」「试算」「退出」这种事，不是键。**一个键都派不出来就没有这一条**
/// （`None`）：屏上不摆按不动的键，而那正是「按了没反应」的来源。
pub(super) fn prompt(
    group: KeyGroup,
    stage: Stage,
    here: &[(Key, Action)],
    want: impl Fn(Action) -> bool,
) -> Option<String> {
    let picked: Vec<(Key, Action)> = here
        .iter()
        .copied()
        .filter(|(_, action)| want(*action))
        .collect();
    let rows = merged(group, stage, &picked, Wording::Short);
    match rows.is_empty() {
        true => None,
        false => Some(
            rows.iter()
                .map(|(spelt, what)| format!("{spelt} {what}"))
                .collect::<Vec<_>>()
                .join(" · "),
        ),
    }
}

/// 一个键在屏上怎么写。**屏底那一行与 `?` 那张表写的是同一批记号**
/// （`⏎`、`⇥`、`⇧⇥`、`Esc`、`Ctrl-C`、`F1`）：同一个键在两处长得不一样的话，
/// 读的人要先认出它们是一个。
pub(super) fn spelled(key: Key) -> String {
    match key {
        Key::Up => "↑".to_owned(),
        Key::Down => "↓".to_owned(),
        Key::Left => "←".to_owned(),
        Key::Right => "→".to_owned(),
        Key::Enter => "⏎".to_owned(),
        Key::Space => "空格".to_owned(),
        Key::Tab => "⇥".to_owned(),
        Key::BackTab => "⇧⇥".to_owned(),
        Key::Backspace => "⌫".to_owned(),
        Key::Esc => "Esc".to_owned(),
        Key::Interrupt => "Ctrl-C".to_owned(),
        Key::F1 => "F1".to_owned(),
        Key::Char(letter) => letter.to_string(),
    }
}

/// 一个动作在屏上怎么说，**长短两份**。
///
/// **哪些键派得出它由按键表答**（[`crate::session::state::Session::action`]）：
/// 这一层只管措辞。
///
/// **几支随[屏上这一块](KeyGroup)而变**：同一个 [`Action::Move`] 在左栏上挪的是一行配置、
/// 在取值栏上挪的是一格取值、在逐页表上挪的是一页——动作相同，说的不是同一件事。
/// **两支随[阶段](Stage)而变**：按停按过一次之后那个键说的是「再按一次就中止」，
/// 而跑着时 `Ctrl-C` 退出会话的后果与浏览时不是一件事。别的几支与两者都无关。
///
/// **打字那几支只有屏底读得到**（[`Action::Insert`]、[`Action::Backspace`]）：
/// 编辑一行与打预设名两块不在 `?` 那张表上（见 [`KeyGroup`]）。这一张表照旧列全——
/// 少列一支，往后添一个新动作时这里不会红。
pub(super) fn says(group: KeyGroup, stage: Stage, action: Action) -> Says {
    match action {
        Action::Move(_) => match group {
            KeyGroup::Valuing => Says::two("选", "在这一列取值上挪一格"),
            KeyGroup::Expanded => Says::two("选一页", "在逐页表上挪一页"),
            KeyGroup::Picking => Says::two("选", "在这一栏上挪一份"),
            // 覆盖层是**读物**：这一下挪的是从第几行画起，不是一个光标。
            KeyGroup::Overlaid => Says::two("读", "往下读"),
            _ => Says::two("挪一行", "在三层上挪一行"),
        },
        Action::Select(_) => match group {
            KeyGroup::Report => Says::two("选一枝", "在目录表上挪一枝"),
            _ => Says::two("选一卷", "在卷表上挪一卷"),
        },
        Action::Cycle(_) => Says::two("换一个", "就地换一个取值（不摊开）"),
        Action::Unfold => Says::two("摊开取值", "摊开这一行的取值"),
        Action::Drill => Says::two("看这块面板底下的型号", "进去看这块面板底下的型号"),
        Action::Choose => Says::two("定", "把停着的这一格定下来"),
        Action::Toggle => Says::two("勾上／勾掉", "把这一卷勾上／勾掉"),
        Action::Edit => match group {
            KeyGroup::Picking => Says::two("打个名字存下来", "打一个名字，存成一份预设"),
            _ => Says::two("改", "打字改这一行"),
        },
        Action::Remove => Says::two("删掉这一条", "把这一条卷删掉"),
        Action::Insert(_) => Says::same("把这个字添进缓冲"),
        Action::Backspace => Says::same("退掉一个字"),
        Action::Complete => Says::two("补这一层", "把这一层补出来"),
        Action::Commit => Says::two("收下", "收下打的东西"),
        Action::Cancel => match group {
            // **一句话盖住两层**：型号那一行下钻进去之后退回的是面板那一层，
            // 不是左栏（`p3-session-legibility/06` 票面第三条）——摊在屏上的是哪一层
            // 由行首那一截说（见 `super::footer::valuing_prompt`）。
            // 「一格不改」两层上都成立：这一支根本没有写取值的路子。
            KeyGroup::Valuing => Says::two(
                "一格不改地退一步",
                "一格不改地退一步（下钻进去之后回的是面板那一层）",
            ),
            KeyGroup::Picking => Says::two("回配置", "退一步，回配置"),
            // 打名字是这一栏里的一步，退一步该退到上一步：再按一次才出这一栏。
            KeyGroup::Naming => Says::two("回列表", "退一步，回这一栏的列表"),
            KeyGroup::Editing => Says::two("丢掉", "丢掉打的东西，回浏览"),
            // 覆盖层**盖住**一块焦点、不替掉它，而「刚才那一块」此刻不在屏上——
            // 不说一句，屏上没有一处答得出关掉之后会到哪儿。
            KeyGroup::Overlaid => Says::same("关（回到刚才那一块）"),
            _ => Says::two("回配置", "退一步，回配置"),
        },
        Action::Start(RunMode::DryRun) => Says::two("试算", "试算：只算不写，报告照出"),
        Action::Start(RunMode::Process) => Says::two("执行", "执行：写到输出根"),
        // **按停按到哪一级说的不是同一句话**（ADR 0013）：没按过时它是两级停的说明，
        // 按过一次之后闩已经在收尾上，这个键剩下的只有中止那一级。
        Action::Stop => match stage {
            Stage::Running(Instruction::Continue) => Says::two(
                "停（按一次收尾，再按一次中止）",
                "停：按一次收尾，再按一次中止",
            ),
            _ => Says::two("再按一次就中止", "再按一次就中止：当前卷停在这一页上"),
        },
        // **三个答话键各带一句它买的东西**：`x` 那一句是第一遍不重算（续做整件事
        // 就是为了它），`a` 那一句是往下不再问（几十卷的一趟按一下就挂得住）。
        // 两句都短，长短一份就够。
        Action::Answer(Instruction::Continue, Reach::ThisVolume) => {
            Says::same("接着做第二遍（第一遍不重算）")
        }
        Action::Answer(Instruction::Continue, Reach::ForTheRest) => {
            Says::same("剩下的卷都这样（往下不再问）")
        }
        // **「剩下的卷也不开工」非说不可**：一卷的时候那件事说不说都一样，
        // 五十卷的时候它是这个键最大的后果，而只说「这一卷不写」的话，
        // 它读起来像是「跳过这一卷」。长短两份都带着它。
        //
        // **短的那一份让掉「等价 dry-run」**：等答话时屏底摆的是七件事
        // （逐页表那几个键也在场），而这一行再长一截就把**下一行那句话**挤掉了
        // ——那一句说的正是「这一卷此刻一个字节都没写」，答这一问要知道的就是它。
        // 让掉的这一句在 `?` 那张表上照旧读得到。
        Action::Answer(..) => Says::two(
            "收尾（这一卷不写，剩下的卷也不开工）",
            "收尾（这一卷不写，等价 dry-run；剩下的卷也不开工）",
        ),
        Action::Focus(Pane::Report) => Says::two("报告区", "把焦点切到报告区"),
        Action::Focus(Pane::Config) => Says::two("回配置", "把焦点切回左栏"),
        Action::Follow => Says::two("回到跟随", "回到跟随：光标交回给最新那一卷"),
        Action::Open => Says::two("展开这一枝", "展开这一枝：摊出它底下那几卷"),
        Action::Expand => Says::two("展开逐页", "把这一卷的逐页摊开"),
        Action::Turn(Step::Next) => Says::same("换下一卷"),
        Action::Turn(_) => Says::same("换上一卷"),
        Action::Collapse => match group {
            KeyGroup::Expanded => Says::two("收起，左栏回来", "收起，回这一枝的卷表（左栏回来）"),
            _ => Says::two("回目录表", "收起，回目录表"),
        },
        Action::List(Listing::All) => Says::same("列全部页"),
        Action::List(_) => Says::same("只列要紧的页"),
        Action::Pick => Says::two("预设", "开预设那一栏"),
        Action::Take => Says::two("套用这一份", "套用停着的那一份"),
        Action::Store => Says::two("存下", "存下来"),
        Action::Erase => Says::two("删掉", "删掉停着的那一份（按两下）"),
        Action::Chart => Says::two("出标定图", "按这块面板出一张标定图"),
        // 两张各叫什么只有 [`Overlay::what`] 一处，屏底那一行与这一格的抬头共用它。
        Action::Reveal(overlay) => Says::same(overlay.what()),
        // **跑着与等答话时退出会话的后果要说出来**（停车场 Q63）：那一下走的是中止，
        // 当前卷停在这一页上、那一格 partial 丢掉——盘上不留半卷。
        Action::Quit => match stage.read_only() {
            true => Says::same("退出会话（当前卷中止，盘上不留半卷）"),
            false => Says::two("退出", "退出会话"),
        },
        // 派不出动作的键根本不进这两处（[`Session::keys_here`] 与
        // [`Session::keys_of`] 各先滤了一道）。
        Action::Ignored => Says::same("在这里没有意义"),
    }
}
