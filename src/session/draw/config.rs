//! 屏上那一块：**左栏**——三层配置常驻的那一栏（`CONTEXT.md` 的《会话》：三层）。
//!
//! 一层一块、按生命周期从上到下，每一行是「标签 + 取值」。**跑起来之后整栏只读**，
//! 而那件事要在屏上看得出来，不能是按了没反应（见 [`config`]）。
//!
//! 这一栏在这一屏上占多宽、什么时候整个收起，归 [`super::yielding::config_width`]——
//! 那是布局的事；本模块只画格子里的东西。**折行走 [`crate::wrap`]**，与屏上其余各格同一套
//! （见 [`super::folded`]）：折出来有几行当场数得出，而[视口](Viewport)要的正是那个数。
//!
//! **卷打得多了这一栏就装不下**（行数是 21 加卷数，再加摊开那一列，**再加折出来的那几行**）：
//! 从第几行画起由 [`Viewport`] 算，滚动条与正文一起画（见 [`super::scrolling`]）。
//!
//! **取值栏就地摊在这一栏里**（`CONTEXT.md` 的《会话》：取值栏）——摊在那一行下面，
//! 左栏其余各行还在场。预设栏占的是主区，那是另一个状态、另一块（[`super::picker`]）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::Styled;
use super::paint::Tone;
use crate::session::state::{Field, Focus, Layer, Session, Stage, Values};
use crate::session::viewport::Viewport;

/// 跑起来之后左栏的抬头。**「只读」要看得出来**，不能是按了没反应
/// （`CONTEXT.md` 的《会话》：一趟跑起来之后三层都只读）。
const READ_ONLY_TITLE: &str = "配置 · 跑着，三层都只读";

/// 取值栏上**此刻生效的那一格**行首那个记号，以及别的那几格行首那个。
///
/// 两个记号而不是「有记号／没记号」：一列里只有一格是实心的，扫一眼就找得到它，
/// 而空着的那一格会与缩进糊成一片。**它与光标那一格分得开**（票面第二条）——
/// 光标靠反白，这一个靠记号，两者可以落在同一格上，也可以不落在同一格上，
/// 而那正是「我在看的」与「我选的」的分别。
const CHOSEN: char = '●';
const UNCHOSEN: char = '○';

/// **下钻进去的那一块面板**行首那个记号（`CONTEXT.md` 的《会话》：下钻）。
///
/// 与上面那两个不是一回事，因此不是第三种「选中」：那一行不是这一列上的一格
/// ——它是**抬头**，光标落不到它上面，也没有「生效不生效」可言。
/// 一个朝里的箭头说的正是「这几格在它底下」。
const INSIDE: char = '▸';

/// 取值栏那几格的缩进：摊开那一列比它挂着的那一行再缩进一层，
/// 下钻进去那一层（抬头是 [`INSIDE`]）再缩进一层。
///
/// **缩进是屏上唯一说得出「这几格是上面那一行的」的东西**——这一栏没有第二级框线，
/// 而两层各深一级正是「它在哪一层」在屏上的样子。两个数摆在一处定，
/// 一个改了另一个忘了跟着改的话，两层就对不齐。
const UNFOLDED_INDENT: &str = "    ";
const DRILLED_INDENT: &str = "      ";

/// 左栏：三层，各占一块，按生命周期从上到下。
///
/// **跑起来之后整栏只读**，而这一条要在屏上**看得出来**，不能是「按了没反应」：
/// 抬头改口（[`READ_ONLY_TITLE`]），光标不再反白，各行压暗（[`Tone::Muted`]）。
/// 真正拦住按键的不是这里——是状态机在那个阶段一个改动键都不派
/// （见 `super::super::state::stage_action`）；这里只把那件事说出来。
///
/// **反白只在左栏就是眼下动手的地方时才给**：那一格反白说的是「就在这一行上动手」。
/// 跑着与等答话时按不动（上面那一条），**焦点切到报告区时按键全归那一块**
/// （`⇥`，ADR 0017），预设那一栏开着时同理——左栏此刻在屏上只为让人对照着看
/// （见 [`super::shell`]），反白它就是在指一个按不动的地方。
/// 压暗仍只给跑着那一种：那一种是「这一趟没跑完都改不动」，而焦点与预设那一栏
/// 都是一个键就回来的事。
///
/// **取值栏摊着时那一列摊在那一行下面**（`CONTEXT.md` 的《会话》：取值栏），
/// 而**反白落在那一列上、不落在那一行上**：屏上只有一处反白，它说的恒是
/// 「就在这一格上动手」，而此刻动手的地方是那一格取值。生效着的那一格另有一个记号
/// （[`CHOSEN`]），两者因此分得开。
///
/// **打进来的卷多过这一格装得下的行数时，视口跟着光标走**（见 [`Viewport`]）：
/// 从前这一栏一点滚动都没有——行数是 21 加卷数，24 行的终端上打进三个卷，
/// 光标就走到屏外，而屏上看不出它去哪儿了。滚动量**算出来、不记着**：
/// 光标在哪一行，这一栏就跟到哪一行。
pub(super) fn config(frame: &mut Frame, area: Rect, session: &Session) {
    // **压暗只给「跑着」那一种，而反白按「焦点 + 只读」给**——两个判断，两件事。
    // 等答话时三层同样只读（`Stage::read_only`），而这一栏此刻不压暗、抬头也不改口：
    // 那一句写死了「跑着」，改口先要定下等答话时它写什么。停车场 Q160 记着这一笔。
    let running = matches!(session.stage(), Stage::Running(_));
    // **反白只在焦点真落在这一栏上、而且三层此刻改得动时才给**：
    // 焦点在报告区（或展开着、预设栏开着）时它归那一块，跑着与等答话时它一个键都不派
    // ——两条各归两维中的一维（ADR 0017），而屏上只有一处反白。
    let acting = matches!(session.focus(), Focus::Config | Focus::Editing(_))
        && !session.stage().read_only();
    let focus = session.field();
    // 一行字加这一行的样式（[`Styled`]）：折行只认字，样式折完由 [`super::folded`] 逐行重挂。
    let mut rows: Vec<Styled> = Vec::new();
    let mut drawn: Option<Layer> = None;
    // 光标落在**折行之前**的第几行：层与层之间还垫着抬头与空行，行号数不出来
    // （见 [`Viewport::new`]）。折完落在第几行由 [`super::folded`] 换算——
    // 交给 [`Viewport`] 的那个数说的恒是**屏上的行**。
    // **跑着与预设那一栏开着时它照旧跟着 `focus` 走**：那两种只是不反白，
    // 光标本身没有挪窝，视口跟丢了才是屏上说不通的事。
    let mut cursor = 0;
    for field in session.rows() {
        let layer = field.layer();
        if drawn != Some(layer) {
            // 层与层之间空一行。**摆的是一个空格而不是空串**：空文字折出零行
            // （[`crate::wrap::fold`]），而这里要的正是一行——屏上早有这个写法
            // （`super::overlay` 里那几组之间空的那一行）。
            if drawn.is_some() {
                rows.push(Styled::plain(" ".to_owned()));
            }
            rows.push(Styled::new(
                layer.title().to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            drawn = Some(layer);
        }
        let style = match (running, acting && field == focus) {
            // **跑着时这一栏是「不要紧」那一档**（spec 的《语义色》：只读时的左栏）——
            // 压暗这件事因此与卷表上跳过的那几行同一个出处（[`Tone`]），
            // 这一块自己一个颜色都不挑。**接住这个颜色的是抬头**（[`READ_ONLY_TITLE`]）：
            // 不上色的终端上「按不动」照旧写在那里。
            (true, _) => Tone::Muted.style(),
            (false, true) => Style::default().add_modifier(Modifier::REVERSED),
            (false, false) => Style::default(),
        };
        if field == focus {
            cursor = rows.len();
        }
        rows.push(Styled::new(row(session, field), style));
        // 取值栏就摊在这一行下面。视口要跟的因此是**那一列上的光标**，不是这一行——
        // 摊开之后左栏长出十来行，跟着这一行走的话，摊开那一列的末几格照旧掉在屏外。
        if let Some(values) = session.valuing()
            && values.field() == field
        {
            // 下钻进去那一层的抬头：**进的是哪一块面板**。这一列此刻列的是型号名，
            // 不印它就没有一处答得出这几个型号是哪块屏的——而屏窄到左栏让出宽度那一档上
            // 屏底那一行也不在场（见 [`super::footer::valuing_prompt`]）。
            // 字面走 `tonefit::Panel` 自己的写法，会话这一侧不另编一份。
            if let Some(panel) = values.panel() {
                rows.push(Styled::plain(format!("{UNFOLDED_INDENT}{INSIDE} {panel}")));
            }
            for at in 0..values.cells().len() {
                if at == values.at() {
                    cursor = rows.len();
                }
                rows.push(choice(values, at));
            }
        }
    }
    // **折行而不是切掉**：阈值那一行要把**标定来源**原样带上来（spec 的 Further Notes），
    // 而那句话比这一栏宽；路径也一样，切掉尾巴的路径看不出是哪一个。
    // **折到多宽从骨架来**：这一格的宽度由 [`super::yielding::config_width`] 定，
    // 两条框线各吃一格——这一栏自己不猜第二个数。
    let (lines, cursor) = super::folded(rows, cursor, area.width.saturating_sub(2));
    let view = Viewport::new(
        lines.len(),
        usize::from(area.height.saturating_sub(2)),
        cursor,
    );
    let body = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(
        super::yielding::title(if running { READ_ONLY_TITLE } else { "配置" }, area.width),
    ));
    super::scrolling(frame, area, body, &view);
}

/// 取值栏上的第 `at` 格：记号 + 取值。
///
/// 比那一行的取值再缩进一层：它摊在那一行**下面**，而缩进是屏上唯一说得出
/// 「这几格是上面那一行的」的东西——这一栏没有第二级框线。
/// **下钻进去那一层再缩进一层**：那几格挂在面板那一行底下，缩进说的是同一件事。
///
/// **两个记号在这一处一起定**：实心的那个说的是「生效的是它」，反白说的是
/// 「光标停在它上面」——两者读的是 [`Values`] 上两个各不相干的数，
/// 拆成两个参数递进来就等于让调用方替它们各判一次。
///
/// **一格都不实心是有的**：型号停在内置表外的一个名字上时那一列里没有一格生效着
/// （见 `super::super::state::Values::chosen`），这一列因此全是空心的——
/// 那正是屏上该说的话，随便挑一格点实了就是在指一个用户没挑过的型号。
fn choice(values: &Values, at: usize) -> Styled {
    let mark = if values.chosen() == Some(at) {
        CHOSEN
    } else {
        UNCHOSEN
    };
    let style = if at == values.at() {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let indent = if values.panel().is_some() {
        DRILLED_INDENT
    } else {
        UNFOLDED_INDENT
    };
    Styled::new(format!("{indent}{mark} {}", values.cells()[at]), style)
}

/// 左栏上的一行：名字 + 取值。怎么标（反白、压暗、还是原样）由 [`config`] 定。
fn row(session: &Session, field: Field) -> String {
    match field {
        // 卷那一行的取值里已经带着勾与路径，再挂一个「卷」字是废话。
        Field::Volume(_) => format!("  {}", session.shown(field)),
        Field::AddVolume => format!("  {}", field.label()),
        _ => format!("  {:　<8}{}", field.label(), session.shown(field)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::probe::{
        reversed_cells, reversed_rows, same_screen, screen, snapshot, tight,
    };
    use super::*;
    use crate::session::state::Key;

    /// 打进来几个卷的一个会话。走的是真会话那条路（停在「＋ 再打一个卷进来」上打字），
    /// 而不是往里塞一份状态：卷打进来之后光标停在哪一行是本票要问的事，
    /// 塞进去就把那一半跳过了。
    fn with_volumes(count: usize) -> Session {
        let mut session = Session::new();
        for at in 1..=count {
            session.go_to(Field::AddVolume);
            session.press(Key::Enter);
            for glyph in format!("库/卷{at}").chars() {
                session.press(Key::Char(glyph));
            }
            session.press(Key::Enter);
        }
        session
    }

    /// 把左栏单独画进一格里。
    fn config_pane(session: &Session, width: u16, height: u16) -> String {
        snapshot(|frame| config(frame, frame.area(), session), width, height)
    }

    /// **跑起来之后：三层只读这件事在屏上看得出来**，而不是按了没反应（本票的验收）。
    ///
    /// 三样各说一遍同一件事：抬头改口说「只读」、光标那一行不再反白、改一行的那几个键
    /// 一个都不提。反白那一格非验不可——它说的是「就在这一行上动手」，而这时按不动。
    #[test]
    fn a_run_in_progress_says_on_screen_that_the_three_layers_are_read_only() {
        let mut session = Session::new();
        let before = tight(&screen(&mut session, None, 120, 40));
        assert!(before.contains(&tight("⏎ 摊开取值")), "{before}");
        assert!(reversed_rows(&mut session) > 0, "浏览时光标那一行该反白");

        session.run_started();
        assert_eq!(reversed_rows(&mut session), 0, "跑着时光标还反白着");
        let running = tight(&screen(&mut session, None, 120, 40));

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
        // **取值栏那个键也不提**：跑起来之后摊不开，而三层只读那一条不因它松动
        // （`CONTEXT.md` 的《会话》；按键表那一头见 `super::super::state` 的
        // `which_keys_do_what_in_which_state` 第六段）。
        for keys in ["⏎ 摊开取值", "⏎ 改", "空格 勾上"] {
            assert!(
                !running.contains(&tight(keys)),
                "{keys} 还在屏上：{running}"
            );
        }
    }

    /// **左栏装不下时光标仍在屏上，上下走到头会跟着滚**（本票的验收头一条）。
    ///
    /// 行数是 21 加卷数：24 行的终端上打进三个卷正好 24 行，而那一格里面只有 22 行。
    /// 从前这一栏一点滚动都没有——光标停在「＋ 再打一个卷进来」上，屏上却看不到它，
    /// 也看不出它去哪儿了。
    ///
    /// 两头各问一次：光标在末尾时开头那几行让出去，走回开头时反过来。
    /// **反白那一格非验不可**——逐格拼回来的文字看不出光标停在哪一行，
    /// 而「光标仍在屏上」问的正是它（数反白用 `super::super::probe::reversed_cells`：
    /// 这几条只画左栏一格，`reversed_rows` 那个按列切的办法在这里用不上）。
    ///
    /// **这一条问的是 52 列那一档**（[`super::yielding::CONFIG_WIDTH`]，这一栏装得下的
    /// 正常宽度）：那里一行都不折。窄档上行行都折，那一档由
    /// [`the_narrow_config_column_counts_the_rows_it_folds_into`] 问
    /// （`p4-parking-lot/02` 收的 Q104／Q136）。
    #[test]
    fn a_config_column_taller_than_its_box_scrolls_with_the_cursor() {
        let mut session = with_volumes(3);
        assert_eq!(
            session.rows().len() + 5,
            24,
            "21 行加卷数，加的是三块抬头与两个空行"
        );

        // 光标停在末尾那一行上：它在屏上，而开头那几行让了出去。
        let bottom = tight(&config_pane(&session, 52, 24));
        assert!(
            bottom.contains(&tight(Field::AddVolume.label())),
            "光标那一行掉出屏外了：{bottom}"
        );
        assert!(
            !bottom.contains(&tight(Layer::Device.title())),
            "视口没跟着光标走：{bottom}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 52, 24) > 0,
            "光标那一行不在屏上：{bottom}"
        );

        // 一路走回开头：这一回让出去的是末尾那几行。
        for _ in 0..session.rows().len() - 1 {
            session.press(Key::Up);
        }
        let top = tight(&config_pane(&session, 52, 24));
        assert!(
            top.contains(&tight(Layer::Device.title())),
            "翻不回顶上：{top}"
        );
        assert!(
            !top.contains(&tight(Field::AddVolume.label())),
            "开头那几行在屏上，末尾那一行也在：{top}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 52, 24) > 0,
            "光标那一行不在屏上：{top}"
        );
    }

    /// **快照：左栏装不下的那一张**（本票的验收「快照：左栏装不下时的一张」）。
    ///
    /// 钉的是两件事：开头那两行让了出去（正文从「型号」下面那一行画起），
    /// 以及**滚动条画在右边那条框线上**——`▲`／`▼` 两头加中间那一截滑块说的是
    /// 「上面还有、下面还有」。滚动条走终端库自带的那个 widget，一列正文都不吃
    /// （见 [`super::scrolling`]）。
    ///
    /// **装得下时一条都不画**：那一格因此与没有这一段时逐格相同。
    /// 「装不装得下」按的是**折出来的行**，与屏上的行一一对应（见
    /// [`the_narrow_config_column_counts_the_rows_it_folds_into`]）。
    #[test]
    fn the_config_column_that_does_not_fit() {
        same_screen(
            &config_pane(&with_volumes(3), 52, 24),
            THE_CONFIG_COLUMN_THAT_DOES_NOT_FIT,
        );

        // 一个卷都没打进来时是 21 行，22 行的格子装得下——滚动条一格都不画。
        let fits = config_pane(&Session::new(), 52, 24);
        assert!(
            !fits.contains('▲') && !fits.contains('▼'),
            "装得下还画了滚动条：{fits}"
        );
    }

    /// **窄档上视口数的是屏上的行，不是逻辑行**（票面第四条，收的是停车场 Q104／Q136）。
    ///
    /// 屏 62–81 列那一段上这一栏 32–51 列（见 [`super::yielding::config_width`]），
    /// 行行折得起来：**一个卷都没打进来的二十一个逻辑行，34 列上折成三十行**。
    /// 折行从前走终端库自己的 `Wrap`，折出来几行这一头数不出来，视口因此数的是逻辑行——
    /// 那一段上于是有两个毛病：**内容已经被折掉、滚动条却不画**，而光标走到格子外面去。
    /// 折行搬进 [`crate::wrap`] 之后折出来有几行当场数得出，两个数从此是同一个。
    ///
    /// **26 行的一格恰好夹在两个数中间**（正文 24 行）：21 装得下、30 装不下，
    /// 两档因此各答各的——只问一档的话，「滚动条画不画」那一半就没有对照。
    #[test]
    fn the_narrow_config_column_counts_the_rows_it_folds_into() {
        let mut session = Session::new();

        // 52 列：一行不折，二十一行摆得进正文那 24 行——没有可滚的东西，一条都不画。
        let wide = config_pane(&session, 52, 26);
        assert!(
            !wide.contains('▲') && !wide.contains('▼'),
            "装得下还画了滚动条：{wide}"
        );

        // 34 列：同一份内容折成三十行，摆不进去——**滚动条画得出来**。
        let narrow = config_pane(&session, 34, 26);
        for edge in ['▲', '▼'] {
            assert!(
                narrow.contains(edge),
                "内容已经被折掉，滚动条却不画：{narrow}"
            );
        }

        // **光标不掉出格子**：走到末尾那一行上，那一行连同它的反白都还在屏上。
        // 反白那一格非验不可——逐格拼回来的文字看不出光标停在哪一行。
        session.go_to(Field::AddVolume);
        let bottom = tight(&config_pane(&session, 34, 26));
        assert!(
            bottom.contains(&tight(Field::AddVolume.label())),
            "光标那一行掉出格子了：{bottom}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 34, 26) > 0,
            "光标那一行不在屏上：{bottom}"
        );

        // **折下来的那一截跟着上同一份样式**（票面第二条）：把光标停在一行真折得起来的行上
        // （型号那一行 40 格，这一格只有 32 格），再与**同一行不折时**比反白的格数。
        // 折下来那一截要是没跟着反白，这个数只会更小；它反而多两格——多的正是
        // `crate::wrap` 给折下来的那一行补上的行首缩进。
        session.go_to(Field::Profile);
        let folded = reversed_cells(|frame| config(frame, frame.area(), &session), 34, 26);
        let unfolded = reversed_cells(|frame| config(frame, frame.area(), &session), 52, 26);
        assert!(
            folded > unfolded,
            "光标那一行折成了两行，反白只有 {folded} 格（不折时 {unfolded} 格）——没跟下来"
        );
    }

    /// 摊开一行的取值栏，把左栏画出来。
    fn unfolding(session: &mut Session, field: Field) {
        session.go_to(field);
        session.press(Key::Enter);
        assert!(session.valuing().is_some(), "{field:?} 没摊开");
    }

    /// **快照：取值栏摊开着的一张**（本票的验收第八条前一半）。
    ///
    /// 钉的是三件事：那一列摊在**那一行下面**、比它再缩进一层（读得出这几格是谁的）；
    /// **第一格是「没说」那一格**（`默认（lanczos3）`）；**此刻生效的那一格前面是实心
    /// 记号**，别的几格是空心的。左栏其余各行还在场——改一个值时看得见它在整份配置里
    /// 的位置（spec 的 story 10）。
    ///
    /// 光标那一格靠反白，快照读不出来，另有一条问它（见
    /// [`the_unfolded_values_keep_the_cursor_on_screen`]）。
    #[test]
    fn the_unfolded_values() {
        let mut session = Session::new();
        // 先说一个值，记号与第一格才分得开：不然两者落在同一格上，
        // 这张快照就说不出「记号指的是生效的那一格」。
        session.go_to(Field::Filter);
        session.press(Key::Right);
        unfolding(&mut session, Field::Filter);
        same_screen(&config_pane(&session, 52, 30), THE_UNFOLDED_VALUES);
    }

    /// 见 [`the_unfolded_values`]。
    const THE_UNFOLDED_VALUES: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         │"
"│  型号　　　　　　未挑（跑起来之前必填）          │"
"│  感知可分辨级数　默认（跟随面板）                │"
"│  阈值　　　　　　跟着型号走（先挑一个）          │"
"│                                                  │"
"│口味层 · 这一趟的立场                             │"
"│  适配方式　　　　默认（height）                  │"
"│  裁边　　　　　　默认（裁）                      │"
"│  跨页拆分　　　　默认（拆）                      │"
"│  拆分阈值　　　　默认（1.5）                     │"
"│  阅读方向　　　　默认（rtl）                     │"
"│  滤波器　　　　　area                            │"
"│    ○ 默认（lanczos3）                            │"
"│    ● area                                        │"
"│    ○ bilinear                                    │"
"│    ○ hamming                                     │"
"│    ○ bicubic                                     │"
"│    ○ lanczos3                                    │"
"│  位深　　　　　　自动（判据说了算）              │"
"│  抖动　　　　　　自动（判据说了算）              │"
"│  逐页　　　　　　默认（关）                      │"
"│  缓存预算　　　　默认（512.0 MiB）               │"
"│  读取策略　　　　默认（auto）                    │"
"│                                                  │"
"│范围层 · 每趟都不同，不进预设                     │"
"│  输出根　　　　　未填（跑起来之前必填）          │"
"│  ＋ 再打一个卷进来                               │"
"│                                                  │"
"└──────────────────────────────────────────────────┘"
"#;

    /// **快照：摊开到左栏装不下的一张**（本票的验收第八条后一半）。
    ///
    /// 摊开之后左栏纵向变长——21 行加卷数，再加摊开那一列的格数。
    /// 装不下的那一刻滚动条画出来（`▲`／`▼` 两头加中间那一截滑块），
    /// 而**摊开那一列一格不少地跟着正文滚**：它是左栏正文的一部分，不另占一格。
    ///
    /// 光标停在那一列的**末一格**上，视口因此跟到了那儿：开头那几行让了出去
    /// （设备层整块不在屏上），而那一格在屏上。这张钉的正是「摊开之后视口接得住」
    /// （本票的验收第四条）画出来是什么样。
    #[test]
    fn the_unfolded_values_that_do_not_fit() {
        let mut session = with_volumes(3);
        unfolding(&mut session, Field::Filter);
        // 走到那一列的末一格上（`↑` 两头绕回去）：视口要跟到的是它。
        session.press(Key::Up);
        same_screen(
            &config_pane(&session, 52, 16),
            THE_UNFOLDED_VALUES_THAT_DO_NOT_FIT,
        );
    }

    /// 见 [`the_unfolded_values_that_do_not_fit`]。
    const THE_UNFOLDED_VALUES_THAT_DO_NOT_FIT: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│                                                  ▲"
"│口味层 · 这一趟的立场                             ║"
"│  适配方式　　　　默认（height）                  █"
"│  裁边　　　　　　默认（裁）                      █"
"│  跨页拆分　　　　默认（拆）                      █"
"│  拆分阈值　　　　默认（1.5）                     █"
"│  阅读方向　　　　默认（rtl）                     ║"
"│  滤波器　　　　　默认（lanczos3）                ║"
"│    ● 默认（lanczos3）                            ║"
"│    ○ area                                        ║"
"│    ○ bilinear                                    ║"
"│    ○ hamming                                     ║"
"│    ○ bicubic                                     ║"
"│    ○ lanczos3                                    ▼"
"└──────────────────────────────────────────────────┘"
"#;

    /// **摊开之后左栏变长，而视口接得住：光标不掉出屏**（本票的验收第四条）。
    ///
    /// 视口跟的是**那一列上的光标**，不是摊开的那一行——跟着那一行走的话，
    /// 摊开那一列的末几格照旧掉在格子下面，而那正是本票要修的毛病的另一面。
    ///
    /// 两头各问一次：光标走到那一列的末一格时开头那几行让出去，回到第一格时反过来。
    /// **反白那一格非验不可**——逐格拼回来的文字看不出光标停在哪一格。
    ///
    /// **只问一行都不折的宽度**，与 [`a_config_column_taller_than_its_box_scrolls_with_the_cursor`]
    /// 同一条（停车场 Q136）。
    #[test]
    fn the_unfolded_values_keep_the_cursor_on_screen() {
        let mut session = with_volumes(3);
        unfolding(&mut session, Field::Filter);
        session.press(Key::Up);

        // 光标停在那一列的末一格上：它在屏上，而开头那几行让了出去。
        // **比的是带着记号的那一整格**：光比 `lanczos3` 的话，第一格
        // 「● 默认（lanczos3）」也含着它，末一格掉出屏外这一条照样绿。
        let bottom = tight(&config_pane(&session, 52, 16));
        assert!(
            bottom.contains(&tight("○ lanczos3")),
            "光标那一格掉出屏外了：{bottom}"
        );
        assert!(
            !bottom.contains(&tight(Layer::Device.title())),
            "视口没跟着光标走：{bottom}"
        );
        let cells = |session: &Session| {
            reversed_cells(|frame| config(frame, frame.area(), session), 52, 16)
        };
        assert!(cells(&session) > 0, "光标那一格不在屏上：{bottom}");

        // 走回那一列的第一格：这一回让出去的是末尾那几行。
        session.press(Key::Down);
        let top = tight(&config_pane(&session, 52, 16));
        assert!(
            top.contains(&tight("● 默认（lanczos3）")),
            "翻不回去：{top}"
        );
        assert!(
            !top.contains(&tight(Field::AddVolume.label())),
            "末尾那一行也在屏上：{top}"
        );
        assert!(cells(&session) > 0, "光标那一格不在屏上：{top}");
    }

    /// **快照：型号那一行摊开的第一层——面板**（本票的验收第八条前一半）。
    ///
    /// 钉的是三件事：那一列摊在型号那一行**下面**、比它再缩进一层；**第一格是「没挑」**；
    /// **每一行是一块面板**，带着分辨率 · PPI · 灰阶数 · 黑白／彩色——字面走
    /// `tonefit::Panel` 自己的写法，会话这一侧不另写一份格式。
    ///
    /// **高度按实现给的面板数取**，不照票面那个「八块」（停车场 Q141）：
    /// 这一张要把整层摆出来看，而面板不止八块。
    ///
    /// 后半段问的正是**「八行放得下」那条布局假设不成立**：同一层画进 24 行的终端里，
    /// 滚动条画得出来（`▲`／`▼`），末几块面板走到跟前时视口跟得上去——
    /// 接住它的仍是 `p3/04` 那**一份**视口，本票一行实现都没新造。
    #[test]
    fn the_unfolded_panels() {
        let mut session = Session::new();
        unfolding(&mut session, Field::Profile);
        same_screen(&config_pane(&session, 52, 36), THE_UNFOLDED_PANELS);

        // 24 行的终端上这一层装不下：滚动条画出来，走到末一块面板时它在屏上。
        let squeezed = config_pane(&session, 52, 24);
        assert!(
            squeezed.contains('▲') || squeezed.contains('▼'),
            "面板那一层在 24 行里装不下，却没画滚动条：{squeezed}"
        );
        session.press(Key::Up);
        let last = session
            .valuing()
            .expect("没摊开")
            .cells()
            .last()
            .expect("那一列不是空的")
            .clone();
        let bottom = tight(&config_pane(&session, 52, 24));
        assert!(
            bottom.contains(&tight(&format!("{UNCHOSEN} {last}"))),
            "末一块面板掉出屏外了：{bottom}"
        );
        assert!(
            reversed_cells(|frame| config(frame, frame.area(), &session), 52, 24) > 0,
            "光标那一格不在屏上：{bottom}"
        );
    }

    /// 见 [`the_unfolded_panels`]。
    const THE_UNFOLDED_PANELS: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         │"
"│  型号　　　　　　未挑（跑起来之前必填）          │"
"│    ● 未挑（跑起来之前必填）                      │"
"│    ○ 1072×1448 · 300 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1072×1448 · 300 PPI · 16 级灰阶 · 彩色      │"
"│    ○ 824×1648 · 300 PPI · 16 级灰阶 · 黑白       │"
"│    ○ 1236×1648 · 300 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1264×1680 · 300 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1264×1680 · 300 PPI · 16 级灰阶 · 彩色      │"
"│    ○ 1404×1872 · 227 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1404×1872 · 227 PPI · 16 级灰阶 · 彩色      │"
"│    ○ 1404×1872 · 300 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1440×1920 · 300 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1650×2200 · 207 PPI · 16 级灰阶 · 黑白      │"
"│    ○ 1860×2480 · 300 PPI · 16 级灰阶 · 黑白      │"
"│  感知可分辨级数　默认（跟随面板）                │"
"│  阈值　　　　　　跟着型号走（先挑一个）          │"
"│                                                  │"
"│口味层 · 这一趟的立场                             │"
"│  适配方式　　　　默认（height）                  │"
"│  裁边　　　　　　默认（裁）                      │"
"│  跨页拆分　　　　默认（拆）                      │"
"│  拆分阈值　　　　默认（1.5）                     │"
"│  阅读方向　　　　默认（rtl）                     │"
"│  滤波器　　　　　默认（lanczos3）                │"
"│  位深　　　　　　自动（判据说了算）              │"
"│  抖动　　　　　　自动（判据说了算）              │"
"│  逐页　　　　　　默认（关）                      │"
"│  缓存预算　　　　默认（512.0 MiB）               │"
"│  读取策略　　　　默认（auto）                    │"
"│                                                  │"
"│范围层 · 每趟都不同，不进预设                     │"
"│  输出根　　　　　未填（跑起来之前必填）          │"
"│  ＋ 再打一个卷进来                               │"
"└──────────────────────────────────────────────────┘"
"#;

    /// **快照：下钻进一块面板之后的那一层——型号**（本票的验收第八条后一半）。
    ///
    /// 钉的是三件事：**行首那一行印着进的是哪一块面板**（`▸`，它是抬头，不是这一列上的
    /// 一格）；型号那几格比它**再缩进一层**；**记号画在当前型号前面**，光标也停在它上面
    /// （`p3-session-legibility/06` 票面第四条）。左栏其余各行还在场。
    #[test]
    fn the_unfolded_models_under_one_panel() {
        let mut session = Session::new();
        session.device.profile = Some("kobo-libra-2".to_owned());
        unfolding(&mut session, Field::Profile);
        session.press(Key::Right);
        assert!(
            session.valuing().expect("没摊开").panel().is_some(),
            "没下钻"
        );
        same_screen(
            &config_pane(&session, 52, 32),
            THE_UNFOLDED_MODELS_UNDER_ONE_PANEL,
        );
    }

    /// 见 [`the_unfolded_models_under_one_panel`]。
    const THE_UNFOLDED_MODELS_UNDER_ONE_PANEL: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│设备层 · 判定的依据，绑面板，改一次管很久         │"
"│  型号　　　　　　kobo-libra-2                    │"
"│    ▸ 1264×1680 · 300 PPI · 16 级灰阶 · 黑白      │"
"│      ○ kobo-libra-h2o                            │"
"│      ● kobo-libra-2                              │"
"│      ○ boox-leaf2                                │"
"│      ○ boox-page                                 │"
"│      ○ kindle-oasis-2                            │"
"│      ○ kindle-oasis-3                            │"
"│      ○ kindle-paperwhite-12                      │"
"│  感知可分辨级数　默认（跟随面板）                │"
"│  阈值　　　　　　阈值 5.500（盲测标定于          │"
"│  boox-poke6，其余面板未复核）                    │"
"│                                                  │"
"│口味层 · 这一趟的立场                             │"
"│  适配方式　　　　默认（height）                  │"
"│  裁边　　　　　　默认（裁）                      │"
"│  跨页拆分　　　　默认（拆）                      │"
"│  拆分阈值　　　　默认（1.5）                     │"
"│  阅读方向　　　　默认（rtl）                     │"
"│  滤波器　　　　　默认（lanczos3）                │"
"│  位深　　　　　　自动（判据说了算）              │"
"│  抖动　　　　　　自动（判据说了算）              │"
"│  逐页　　　　　　默认（关）                      │"
"│  缓存预算　　　　默认（512.0 MiB）               │"
"│  读取策略　　　　默认（auto）                    │"
"│                                                  │"
"│范围层 · 每趟都不同，不进预设                     │"
"│  输出根　　　　　未填（跑起来之前必填）          │"
"│  ＋ 再打一个卷进来                               │"
"└──────────────────────────────────────────────────┘"
"#;

    /// 见 [`the_config_column_that_does_not_fit`]。
    const THE_CONFIG_COLUMN_THAT_DOES_NOT_FIT: &str = r#"
"┌配置──────────────────────────────────────────────┐"
"│  感知可分辨级数　默认（跟随面板）                ▲"
"│  阈值　　　　　　跟着型号走（先挑一个）          ║"
"│                                                  █"
"│口味层 · 这一趟的立场                             █"
"│  适配方式　　　　默认（height）                  █"
"│  裁边　　　　　　默认（裁）                      █"
"│  跨页拆分　　　　默认（拆）                      █"
"│  拆分阈值　　　　默认（1.5）                     █"
"│  阅读方向　　　　默认（rtl）                     █"
"│  滤波器　　　　　默认（lanczos3）                █"
"│  位深　　　　　　自动（判据说了算）              █"
"│  抖动　　　　　　自动（判据说了算）              █"
"│  逐页　　　　　　默认（关）                      ║"
"│  缓存预算　　　　默认（512.0 MiB）               ║"
"│  读取策略　　　　默认（auto）                    ║"
"│                                                  ║"
"│范围层 · 每趟都不同，不进预设                     ║"
"│  输出根　　　　　未填（跑起来之前必填）          ║"
"│  [x] 库/卷1                                      ║"
"│  [x] 库/卷2                                      ║"
"│  [x] 库/卷3                                      ║"
"│  ＋ 再打一个卷进来                               ▼"
"└──────────────────────────────────────────────────┘"
"#;
}
