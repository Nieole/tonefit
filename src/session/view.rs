//! **视图 × 阶段 × 焦点**：新会话的界面状态（ADR 0019；spec《状态：视图 × 阶段 × 焦点》；
//! `CONTEXT.md` 的《会话》：视图、任务视图、配置视图、焦点、卷列表、屏底、退出会话）。
//!
//! 会话此刻在做什么由三样说：**视图**（任务 · 配置，[`View`]）、**阶段**（这一趟走到哪儿了，
//! 仍是 [`Stage`]，一格没动）、**焦点**（视图里的哪一块，[`Focus`]）。两个视图**各记各的**
//! 光标与所在的块（[`TaskView`]、[`ConfigView`]），切走再切回原样；盖在上面的输入行与覆盖层
//! 是焦点的另两个取值，掀掉之后底下原样回来。
//!
//! **旧界面那一维（[`super::state::Focus`]）原样留着**，与本模块并存：真会话仍进旧界面，
//! 切换在 `session-redesign/15`。本模块挂在 [`Session`] 上一格（[`Session::views`]），
//! 新界面的状态机是本模块里那几个 `impl Session`——三组设置与阶段仍在 [`Session`] 自己身上，
//! 两副界面读的是同一份。
//!
//! # 按键表在别处
//!
//! 「哪些键在哪个状态下有效」一处答完，按视图、阶段、焦点查——那张表在 [`super::keymap`]；
//! 本模块只做两件事：把一个输入认成表上的一件事（[`Session::deed_of`]，连击键的前半截在这里待着），
//! 与把那件事做掉（[`Session::perform`]）。屏底此刻摆哪几件、按什么轻重也在这里
//! （[`Session::hints`]），每一件的键与那一句仍从表上取。
//!
//! # 卷列表的光标记的是行的身份
//!
//! 输出目录、哪一条处理路径、「＋ 添加路径」（[`Cursor`]），不是第几行：删掉一条、列表从处理路径
//! 换成树时光标不乱跳。开跑之前那一副的行由 [`Session::lines`] 现算——处理路径有几条就有几行。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）。屏底那一句回话
//! （[`Reply`]）与连击键的待续记号（[`Pending`]）都读会话的[「此刻」](CONTEXT.md)——
//! 几秒后退回、多久算过期，两个时长照设计稿。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::cover::Overlay;
use super::keymap::{self, Chord, Deed, Hint, Phase, Want};
use super::look::{Look, Segment};
use super::state::{
    DEVICE_FIELDS, Exit, Field, Key, NamedPath, OUTPUT_UNSET, Session, TASTE_FIELDS,
};
use super::tone::Tone;
use super::typing::InputLine;
use crate::preset::Preset;

/// 回话在屏底占几秒（设计稿 `toast` 的默认时长）。
pub const REPLY_LINGERS: Duration = Duration::from_millis(2600);

/// 连击键按了前半截之后等后半截等多久（设计稿 `frame` 里那 900 毫秒）。
pub const COMBO_WAITS: Duration = Duration::from_millis(900);

/// 会话的顶层：两个视图，各占整屏（`CONTEXT.md` 的《会话》：视图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// **任务视图 (Task view)**：做事的那一屏——总览、确认条、卷列表或每页结果。
    #[default]
    Task,
    /// **配置视图 (Config view)**：调设置的那一屏——设置栏与详情栏。
    Config,
}

impl View {
    /// 顶栏上的写法：号与名。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn number(self) -> &'static str {
        match self {
            Self::Task => "1",
            Self::Config => "2",
        }
    }

    /// 顶栏上的名字。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn name(self) -> &'static str {
        match self {
            Self::Task => "任务",
            Self::Config => "配置",
        }
    }

    /// 另一个视图（`gt`／`gT` 轮换，只有两个，两个方向是同一个）。
    pub fn other(self) -> Self {
        match self {
            Self::Task => Self::Config,
            Self::Config => Self::Task,
        }
    }
}

/// **焦点**的取值：视图里的块，外加盖在上面的两种（`CONTEXT.md` 的《会话》：焦点；
/// ADR 0017 修订记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// **卷列表 (Volume list)**：任务视图的主体。
    VolumeList,
    /// **每页结果 (Pages)**：进了一卷时换掉卷列表的那一副。
    Pages,
    /// **设置栏 (Settings pane)**：配置视图左边那一栏。
    Settings,
    /// **详情栏 (Details pane)**：配置视图右边那一栏。
    Details,
    /// **预设栏 (Picker)**：`p` 掀开、替换详情栏的那一栏。
    Picker,
    /// **输入行 (Input line)**：打字时占住屏底的那一行。
    Input,
    /// **覆盖层 (Overlay)**：全部按键或说明卡。
    Overlay,
}

/// 卷列表的光标停在哪一行——记的是**行的身份**（模块文档《卷列表的光标记的是行的身份》）。
/// 清点之后的目录、卷与备注行随树那一票添进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    /// 输出目录那一行。
    Output,
    /// 哪一条处理路径。
    Path(PathBuf),
    /// 「＋ 添加路径」那一行。
    Add,
}

/// 开跑之前卷列表上的一行（`CONTEXT.md` 的《卷列表》：开跑之前）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// 输出目录那一行。
    Output,
    /// 「处理路径 (N)」那一行标签。停不住。
    Heading,
    /// 第几条处理路径。
    Path(usize),
    /// 「＋ 添加路径」。
    Add,
}

impl Line {
    /// 这一行停得住的话，它的身份。
    pub fn stop(&self, paths: &[NamedPath]) -> Option<Cursor> {
        match self {
            Self::Output => Some(Cursor::Output),
            Self::Heading => None,
            Self::Path(at) => paths.get(*at).map(|named| Cursor::Path(named.path.clone())),
            Self::Add => Some(Cursor::Add),
        }
    }
}

/// 任务视图记着的：所在的块与卷列表的光标。展开着的目录、每页结果、自动滚动随后面的票添。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    /// 卷列表还是每页结果。
    pub focus: Focus,
    pub cursor: Cursor,
}

impl Default for TaskView {
    fn default() -> Self {
        Self {
            focus: Focus::VolumeList,
            cursor: Cursor::Output,
        }
    }
}

/// 套着的那一份预设：名字与它说的那两组，改了几项从它算。
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub name: String,
    pub preset: Preset,
}

/// 配置视图记着的：所在的块、设置栏的光标、套着的预设。详情栏与预设栏的光标随那几票添。
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigView {
    /// 设置栏、详情栏还是预设栏。
    pub focus: Focus,
    /// 设置栏的光标停在哪一项。
    pub cursor: Field,
    /// 当前套的是哪一份预设。
    pub applied: Option<Applied>,
}

impl Default for ConfigView {
    fn default() -> Self {
        Self {
            focus: Focus::Settings,
            cursor: Field::Profile,
            applied: None,
        }
    }
}

/// 连击键按了前半截：屏底右端留一个待续记号，等后半截等到 [`COMBO_WAITS`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub prefix: char,
    pub at: Instant,
}

/// 屏底那一句**回话**：按键之后临时占住屏底的几截字，到点退回按键提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub segments: Vec<Segment>,
    pub until: Instant,
}

/// **窗口**有多大：列 × 行。终端层每一帧问一次交进来（覆盖层滚到哪儿为止从它算；半屏与一屏那四个
/// 随各票也读它），用例给序列清单上的尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub cols: u16,
    pub rows: u16,
}

/// 新界面的状态：此刻在哪个视图、两个视图各自记着的、盖在上面的两样、屏底那两样临时的东西。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Views {
    pub view: View,
    pub task: TaskView,
    pub config: ConfigView,
    /// 打字时占住屏底的那一行（[`super::typing`]）。
    pub input: Option<InputLine>,
    /// 掀开着的那一张（[`super::cover`]）。盖住输入行，不替掉它。
    pub cover: Option<Overlay>,
    pending: Option<Pending>,
    reply: Option<Reply>,
}

impl Views {
    /// 焦点落在哪一块：盖在上面的先算——掀着覆盖层就是它，打着字就是输入行——都没有才是
    /// 此刻那个视图记着的块（[`block`](Self::block)）。
    pub fn focus(&self) -> Focus {
        if self.cover.is_some() {
            return Focus::Overlay;
        }
        if self.input.is_some() {
            return Focus::Input;
        }
        self.block()
    }

    /// 此刻那个视图记着的块——盖在上面的两样不算。屏上写在一块里的那几句顺口提的键
    /// （总览的 `t`／`x`、行上的 `i`／`o`）问的是这一块派什么，输入行开着时照样写着。
    pub fn block(&self) -> Focus {
        match self.view {
            View::Task => self.task.focus,
            View::Config => self.config.focus,
        }
    }

    /// 屏底右端此刻要不要待续记号：连击键的前半截还没过期。
    pub fn pending(&self, now: Instant) -> Option<char> {
        self.pending
            .filter(|pending| now.saturating_duration_since(pending.at) <= COMBO_WAITS)
            .map(|pending| pending.prefix)
    }

    /// 屏底此刻占着的那句回话，到点就没有了。
    pub fn reply(&self, now: Instant) -> Option<&[Segment]> {
        self.reply
            .as_ref()
            .filter(|reply| now < reply.until)
            .map(|reply| reply.segments.as_slice())
    }

    /// 说一句回话，占屏底 [`REPLY_LINGERS`]。
    pub(super) fn say(&mut self, segments: Vec<Segment>, now: Instant) {
        self.say_for(segments, REPLY_LINGERS, now);
    }

    /// 说一句回话，占屏底多久由这一句自己定（设计稿 `toast` 的第二个参数）。
    pub(super) fn say_for(&mut self, segments: Vec<Segment>, lingers: Duration, now: Instant) {
        self.reply = Some(Reply {
            segments,
            until: now + lingers,
        });
    }

    /// 取走待着的前半截（不管过没过期都取走：新按下的键要么接上它、要么从头算）。
    fn take_pending(&mut self, now: Instant) -> Option<char> {
        let prefix = self.pending(now);
        self.pending = None;
        prefix
    }
}

/// 终端层交给新会话的一个输入：键或鼠标（spec《缝》：收的东西从「键」扩成「键或鼠标」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    /// Ctrl 加一个字母（`C-c` 不在这里，它是 [`Key::Interrupt`]）。
    Ctrl(char),
    /// 滚轮：往下几格（负数往上）。
    Wheel(i16),
    /// 单击屏上第几列第几行。
    Click {
        x: u16,
        y: u16,
    },
}

impl Input {
    fn chord(self) -> Chord {
        match self {
            Self::Key(key) => Chord::Key(key),
            Self::Ctrl(letter) => Chord::Ctrl(letter),
            Self::Wheel(_) => Chord::Wheel,
            Self::Click { .. } => Chord::Click,
        }
    }
}

impl Session {
    /// 开跑之前卷列表上的行：输出目录 · 「处理路径 (N)」· 每条处理路径 · 「＋ 添加路径」。
    pub fn lines(&self) -> Vec<Line> {
        let mut lines = vec![Line::Output, Line::Heading];
        lines.extend((0..self.scope.paths.len()).map(Line::Path));
        lines.push(Line::Add);
        lines
    }

    /// 光标此刻停在开跑之前那一副的第几行。记着的身份找不到了（那一条删掉了）就停到头一行停得住的。
    pub fn cursor_line(&self) -> usize {
        let lines = self.lines();
        let wanted = &self.views.task.cursor;
        lines
            .iter()
            .position(|line| line.stop(&self.scope.paths).as_ref() == Some(wanted))
            .or_else(|| {
                lines
                    .iter()
                    .position(|line| line.stop(&self.scope.paths).is_some())
            })
            .unwrap_or(0)
    }

    /// 光标停在第几条停得住的行上（从 1 起），与停得住的行共几条——框底边那句 `2 of 15`。
    pub fn cursor_position(&self) -> (usize, usize) {
        let stops: Vec<usize> = self
            .lines()
            .iter()
            .enumerate()
            .filter(|(_, line)| line.stop(&self.scope.paths).is_some())
            .map(|(at, _)| at)
            .collect();
        let here = self.cursor_line();
        let position = stops
            .iter()
            .position(|at| *at == here)
            .map_or(0, |at| at + 1);
        (position, stops.len())
    }

    /// 光标停在哪一条处理路径上（停在别的行上就是 `None`）。
    pub fn path_under_cursor(&self) -> Option<usize> {
        match &self.views.task.cursor {
            Cursor::Path(path) => self
                .scope
                .paths
                .iter()
                .position(|named| named.path == *path),
            _ => None,
        }
    }

    /// 这一条处理路径被另一条**勾着的文件夹**包含着的话，是哪一条（按路径前缀认，不碰盘）。
    pub fn nested_in(&self, at: usize) -> Option<&Path> {
        let inner = &self.scope.paths[at];
        self.scope
            .paths
            .iter()
            .enumerate()
            .filter(|(other, named)| *other != at && named.on && !named.is_archive())
            .map(|(_, named)| named.path.as_path())
            .find(|outer| inner.path.starts_with(outer) && inner.path != *outer)
    }

    /// 勾着几条、没勾几条。
    pub fn checked_paths(&self) -> (usize, usize) {
        let on = self.scope.paths.iter().filter(|named| named.on).count();
        (on, self.scope.paths.len() - on)
    }

    /// 与套着的预设不同的有几项（型号不算：设计稿只数取值环与自由填的那几项）。没套预设就是零。
    pub fn changed_from_preset(&self) -> usize {
        let Some(applied) = &self.views.config.applied else {
            return 0;
        };
        DEVICE_FIELDS
            .into_iter()
            .chain(TASTE_FIELDS)
            .filter(|field| *field != Field::Profile)
            .filter(|field| self.differs_from(*field, &applied.preset))
            .count()
    }

    /// 这一项此刻的值与那份预设说的不同吗。
    fn differs_from(&self, field: Field, preset: &Preset) -> bool {
        match field {
            Field::Profile => self.device.profile != preset.device.profile,
            Field::GrayLevels => self.device.gray_levels != preset.device.gray_levels,
            Field::Threshold => self.device.threshold != preset.device.threshold,
            Field::Fit => self.taste.fit != preset.taste.fit,
            Field::Crop => self.taste.crop != preset.taste.crop,
            Field::Split => self.taste.split != preset.taste.split,
            Field::SplitThreshold => self.taste.split_threshold != preset.taste.split_threshold,
            Field::ReadingOrder => self.taste.reading_order != preset.taste.reading_order,
            Field::Filter => self.taste.filter != preset.taste.filter,
            Field::WhiteAlignLimit => {
                self.taste.white_align_limit != preset.taste.white_align_limit
            }
            Field::BitDepth => self.taste.bit_depth != preset.taste.bit_depth,
            Field::Dither => self.taste.dither != preset.taste.dither,
            Field::Envelope => self.taste.envelope != preset.taste.envelope,
            Field::CacheBudget => self.taste.cache_budget != preset.taste.cache_budget,
            Field::IoMode => self.taste.io_mode != preset.taste.io_mode,
            Field::Out | Field::Path(_) | Field::AddPath => false,
        }
    }

    /// 把一个输入认成按键表上的一件事。认不出（这个键此刻没有意义）就是 `None`。
    ///
    /// **连击键在这里待着**：前半截按下去先记下（屏底右端留待续记号），后半截来了合成一个连击键
    /// 去查表；合不上就把后半截当一个单独的键从头认。过了 [`COMBO_WAITS`] 前半截作废。
    pub fn deed_of(&mut self, input: Input, phase: Phase, now: Instant) -> Option<Deed> {
        let focus = self.views.focus();
        let chord = input.chord();
        if let Some(prefix) = self.views.take_pending(now)
            && let Chord::Key(Key::Char(second)) = chord
            && let Some(deed) = keymap::deed(phase, focus, Chord::Combo(prefix, second))
        {
            return Some(deed);
        }
        if let Chord::Key(Key::Char(first)) = chord
            && keymap::starts_a_combo(phase, focus, first)
        {
            self.views.pending = Some(Pending {
                prefix: first,
                at: now,
            });
            return None;
        }
        if let Some(deed) = keymap::deed(phase, focus, chord) {
            return Some(deed);
        }
        // 打字：输入行上表派不出的每一个字符都是一个字（`?`、`q`、`j` 也是）。
        if focus == Focus::Input {
            return match chord {
                Chord::Key(Key::Char(glyph)) => Some(Deed::Typed(glyph)),
                Chord::Key(Key::Space) => Some(Deed::Typed(' ')),
                _ => None,
            };
        }
        None
    }

    /// 把一件事做掉——**状态机够得着的那几件**：挪光标、勾选、删一条、换视图、退出、
    /// 掀开与关掉全部按键、打字与添改路径（[`super::typing`]：补全与确定那两下问一次盘）。
    ///
    /// 起一趟、按停止、答话、预设那几支、灰阶测试图，都要够着那一趟，归终端层那一支
    /// （`super::terminal`）；交到这里的那几件当作没有意义，原地不动。
    /// 覆盖层上滚动要知道窗口有多大，同样在终端层那一支（[`super::cover::Sheet`] 与 `Views::scroll_cover`）；
    /// 半屏与一屏那四个随每页结果与树那几票接上，眼下原地不动。
    pub fn perform(&mut self, deed: Deed, now: Instant) -> Exit {
        let focus = self.views.focus();
        match deed {
            Deed::Quit | Deed::Interrupt => return Exit::Leave,
            Deed::Help | Deed::HelpWhileTyping => self.views.lift_keys(),
            Deed::CloseOverlay => self.views.drop_cover(),
            Deed::AddPath => self.open_adding(),
            Deed::EditPath => self.open_editing(),
            Deed::Typed(glyph) => {
                if let Some(line) = &mut self.views.input {
                    line.type_in(glyph);
                }
            }
            Deed::Erase => {
                if let Some(line) = &mut self.views.input {
                    line.erase();
                }
            }
            Deed::DeleteWord => {
                if let Some(line) = &mut self.views.input {
                    line.delete_word();
                }
            }
            Deed::Complete => self.complete_typed(now),
            Deed::Confirm => self.confirm_typed(now),
            Deed::Cancel => self.cancel_typed(),
            Deed::Down | Deed::Up if focus == Focus::Input => {
                if let Some(line) = &mut self.views.input {
                    line.step(if deed == Deed::Down { 1 } else { -1 });
                }
            }
            Deed::QuitRefused => self.views.say(
                vec![
                    Segment::new("正在转换：", Look::tone(Tone::Caution).bold()),
                    Segment::plain("q 不会退出，请先按 s 停止，或按 C-c 立即退出"),
                ],
                now,
            ),
            Deed::TaskView => self.views.view = View::Task,
            Deed::ConfigView => self.views.view = View::Config,
            Deed::NextView | Deed::PrevView => self.views.view = self.views.view.other(),
            Deed::Down => self.place_cursor(|here, last| (here + 1).min(last)),
            Deed::Up => self.place_cursor(|here, _| here.saturating_sub(1)),
            Deed::Bottom => self.place_cursor(|_, last| last),
            Deed::Top => self.place_cursor(|_, _| 0),
            Deed::TogglePath => {
                if let Some(at) = self.path_under_cursor() {
                    self.scope.paths[at].on = !self.scope.paths[at].on;
                }
            }
            Deed::DeletePath => self.delete_path(now),
            _ => {}
        }
        Exit::Stay
    }

    /// 光标挪到停得住的行里的哪一条：`to` 收「此刻在第几条、最后一条是第几条」，答挪到第几条。
    /// 只在任务视图的卷列表上挪；别的块随各票接上。
    fn place_cursor(&mut self, to: impl Fn(usize, usize) -> usize) {
        if self.views.view != View::Task || self.views.task.focus != Focus::VolumeList {
            return;
        }
        let stops: Vec<Cursor> = self
            .lines()
            .iter()
            .filter_map(|line| line.stop(&self.scope.paths))
            .collect();
        let Some(last) = stops.len().checked_sub(1) else {
            return;
        };
        let here = stops
            .iter()
            .position(|stop| *stop == self.views.task.cursor)
            .unwrap_or(0);
        self.views.task.cursor = stops[to(here, last).min(last)].clone();
    }

    /// 删掉光标那一条处理路径；光标停到它下一条上，没有下一条就停到「＋ 添加路径」；屏底说一句。
    fn delete_path(&mut self, now: Instant) {
        let Some(at) = self.path_under_cursor() else {
            return;
        };
        let gone = self.scope.paths.remove(at);
        self.views.task.cursor = match self.scope.paths.get(at) {
            Some(next) => Cursor::Path(next.path.clone()),
            None => Cursor::Add,
        };
        self.views.say(
            vec![
                Segment::new("已删除 ", Look::tone(Tone::Caution)),
                Segment::plain(self.home_shown(&gone.path)),
            ],
            now,
        );
    }

    /// 屏上这一条路径怎么写（家目录缩写成 `~`）。家目录由会话入口问一次、摆在这里往下传。
    pub fn home_shown(&self, path: &Path) -> String {
        self.home.abbreviate(path)
    }

    /// 屏上输出目录怎么写：缩写成 `~/…`；没填就说没填。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn output_shown(&self) -> String {
        match &self.scope.out {
            Some(out) => self.home_shown(out),
            None => OUTPUT_UNSET.to_owned(),
        }
    }

    /// 屏底此刻按轻重要摆哪几件（设计稿 `footerHints` 的次序），`?` 恒在末尾。
    /// 每一件的键怎么写、那一句怎么说从按键表取（[`keymap::hints`]），派不出的不摆。
    pub fn hints(&self, phase: Phase) -> Vec<Hint> {
        let focus = self.views.focus();
        let mut wants: Vec<Want> = Vec::new();
        match (self.views.view, focus) {
            // 覆盖层掀着时屏底让给它自己的两件（设计稿 `footerHints` 头一支）：`?` 不另摆——它在抬头上。
            (_, Focus::Overlay) => {
                return keymap::hints(
                    phase,
                    focus,
                    &[
                        Want::of(Deed::Down),
                        Want::of(Deed::Up),
                        Want::of(Deed::CloseOverlay),
                    ],
                );
            }
            // 输入行右端那几件（设计稿 `drawFooter` 打字那一支）。
            (_, Focus::Input) => {
                return keymap::hints(
                    phase,
                    focus,
                    &[
                        Want::of(Deed::Complete),
                        Want::of(Deed::DeleteWord),
                        Want::of(Deed::Confirm),
                        Want::of(Deed::Cancel),
                    ],
                );
            }
            (View::Config, Focus::Picker) => wants.extend([
                Want::of(Deed::UsePreset),
                Want::of(Deed::DeletePreset),
                Want::saying(Deed::Presets, "返回"),
                Want::of(Deed::Down),
                Want::of(Deed::Up),
            ]),
            (View::Config, _) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.extend([
                    Want::saying(Deed::ConfigOpen, "展开"),
                    Want::saying(Deed::ConfigBack, "返回"),
                    Want::saying(Deed::Presets, "预设"),
                    Want::of(Deed::TaskView),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
            (View::Task, Focus::Pages) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.extend([
                    Want::saying(Deed::ListAll, "全部页"),
                    Want::of(Deed::BackToList),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
            (View::Task, _) if phase == Phase::Fresh => wants.extend([
                Want::of(Deed::Preview),
                Want::of(Deed::Convert),
                Want::of(Deed::AddPath),
                Want::of(Deed::TogglePath),
                Want::of(Deed::DeletePath),
                Want::of(Deed::Down),
                Want::of(Deed::Up),
                Want::of(Deed::ConfigView),
            ]),
            (View::Task, _) => {
                wants.extend(self.stage_wants(phase, focus));
                wants.extend([
                    Want::of(Deed::NextProblem),
                    Want::of(Deed::Search),
                    Want::of(Deed::Down),
                    Want::of(Deed::Up),
                ]);
            }
        }
        wants.push(Want::of(Deed::Help));
        keymap::hints(phase, focus, &wants)
    }

    /// 阶段那一维派的几件：等待确认时答话那三个与 `v`，跑着时停止，结束了再来一趟。
    fn stage_wants(&self, phase: Phase, focus: Focus) -> Vec<Want> {
        match phase {
            Phase::Deciding => {
                let mut wants = vec![
                    Want::of(Deed::Write),
                    Want::of(Deed::WriteAll),
                    Want::of(Deed::End),
                ];
                if focus != Focus::Pages {
                    wants.push(Want::of(Deed::ViewPages));
                }
                wants
            }
            Phase::Surveying | Phase::Running => vec![Want::saying(
                Deed::Stop,
                if Phase::latched(self.stage()) {
                    "再按一次立即停"
                } else {
                    "停止"
                },
            )],
            Phase::Ended if self.views.view == View::Task => {
                vec![Want::of(Deed::Preview), Want::of(Deed::Convert)]
            }
            Phase::Ended | Phase::Fresh => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::home::Home;
    use crate::session::keymap::TABLE;
    use crate::session::state::Stage;

    fn key(letter: char) -> Input {
        Input::Key(Key::Char(letter))
    }

    /// 三条处理路径、一个输出目录，光标停在头一条上。
    fn three_paths() -> Session {
        let mut session = Session::new();
        session.home = Home::at("/home/me");
        session.scope.out = Some(PathBuf::from("/home/me/转好的"));
        for (path, on) in [
            ("/home/me/漫画库", true),
            ("/home/me/漫画库/银河", true),
            ("/home/me/下载/第01卷.cbz", false),
        ] {
            session.scope.paths.push(NamedPath {
                path: PathBuf::from(path),
                on,
            });
        }
        session.views.task.cursor = Cursor::Path(PathBuf::from("/home/me/漫画库"));
        session
    }

    /// **屏底每一件都出自那张按键表**（票面第三条）：键的写法与那一句合起来是表上的一行，
    /// 而且那一行在此刻的阶段与块上派得出。还没开始、跑着、等待确认、已结束各问一遍。
    #[test]
    fn every_hint_on_the_footer_is_a_row_of_the_key_table() {
        let mut session = three_paths();
        for (phase, stage) in [
            (Phase::Fresh, Stage::Fresh),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
            (Phase::Ended, Stage::Ended),
        ] {
            session.set_stage(stage);
            for view in [View::Task, View::Config] {
                session.views.view = view;
                let focus = session.views.focus();
                let hints = session.hints(phase);
                assert!(!hints.is_empty(), "{phase:?} 的 {view:?} 上屏底空着");
                for hint in &hints {
                    for spelt in &hint.keys {
                        assert!(
                            TABLE.iter().any(|row| row.spelt == *spelt
                                && row.short == hint.what
                                && row.applies(phase, focus)),
                            "{phase:?} 的 {view:?} 上屏底那件「{spelt} → {}」不出自按键表",
                            hint.what
                        );
                    }
                }
                assert_eq!(
                    hints.last().map(|hint| hint.what),
                    Some("全部按键"),
                    "`?` 恒在末尾"
                );
            }
        }
    }

    /// 还没开始时屏底那几件照设计稿的轻重次序。
    #[test]
    fn before_the_run_the_footer_lists_the_eight_things_in_order() {
        let session = three_paths();
        let said: Vec<String> = session
            .hints(Phase::Fresh)
            .iter()
            .map(|hint| format!("{} → {}", hint.spelt(), hint.what))
            .collect();
        assert_eq!(
            said,
            [
                "t → 预览",
                "x → 转换",
                "o → 添加路径",
                "空格 → 勾选",
                "dd → 删除",
                "j/k → 选择",
                "2 → 配置",
                "? → 全部按键",
            ]
        );
    }

    /// `j`／`k` 在停得住的行上挪，标签那一行跳过，两头到底为止；`gg`／`G` 到顶到底。
    /// 问的是框底边那句「第几条 of 几条」，不问光标那个枚举。
    #[test]
    fn the_cursor_moves_over_the_rows_that_can_be_stopped_on() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.cursor_position(), (2, 5));
        session.perform(Deed::Up, now);
        assert_eq!(session.cursor_position(), (1, 5));
        session.perform(Deed::Up, now);
        assert_eq!(session.cursor_position(), (1, 5), "到顶为止");
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        assert_eq!(session.cursor_position(), (3, 5), "标签那一行跳过");
        for _ in 0..10 {
            session.perform(Deed::Down, now);
        }
        assert_eq!(session.cursor_position(), (5, 5), "到底为止");
        session.perform(Deed::Top, now);
        assert_eq!(session.cursor_position(), (1, 5));
        session.perform(Deed::Bottom, now);
        assert_eq!(session.cursor_position(), (5, 5));
        // 光标记的是身份：删掉光标前面那一条，它仍停在同一条路径上（位置往前挪一位）。
        session.perform(Deed::Top, now);
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        session.scope.paths.remove(0);
        assert_eq!(session.cursor_position(), (2, 4));
    }

    /// 空格勾选、再按一次取消；被包含按路径前缀认，勾掉外层那一条里层就不算被包含。
    #[test]
    fn space_toggles_the_path_and_nesting_follows_the_checked_outer_folder() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.checked_paths(), (2, 1));
        assert_eq!(session.nested_in(1), Some(Path::new("/home/me/漫画库")));
        assert_eq!(session.nested_in(0), None);
        session.perform(Deed::TogglePath, now);
        assert!(!session.scope.paths[0].on);
        assert_eq!(session.checked_paths(), (1, 2));
        assert_eq!(session.nested_in(1), None, "外层没勾就不算被包含");
        session.perform(Deed::TogglePath, now);
        assert!(session.scope.paths[0].on);
    }

    /// `dd` 删掉光标那一条：光标停到下一条上，屏底说一句、路径缩写成 `~`；删末一条停到「＋ 添加路径」。
    #[test]
    fn dd_deletes_the_path_under_the_cursor_and_says_so() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.deed_of(key('d'), Phase::Fresh, now), None);
        assert_eq!(session.views.pending(now), Some('d'), "前半截待着");
        assert_eq!(
            session.deed_of(key('d'), Phase::Fresh, now),
            Some(Deed::DeletePath)
        );
        assert_eq!(session.views.pending(now), None);
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.scope.paths.len(), 2);
        assert_eq!(session.cursor_position(), (2, 4), "停到下一条上");
        let reply = session.views.reply(now).expect("屏底说了一句");
        assert_eq!(reply[0].text, "已删除 ");
        assert_eq!(reply[1].text, "~/漫画库");
        assert_eq!(session.views.reply(now + REPLY_LINGERS), None, "到点退回");
        // 删末一条：光标掉到「＋ 添加路径」上，那正是接着要做的事。
        session.perform(Deed::Bottom, now);
        session.perform(Deed::Up, now);
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.cursor_position(), (3, 3));
        // 光标不在处理路径上时 `dd` 什么都不删。
        session.perform(Deed::DeletePath, now);
        assert_eq!(session.scope.paths.len(), 1);
    }

    /// 连击键的前半截过了 [`COMBO_WAITS`] 作废；合不上的后半截当一个单独的键认。
    #[test]
    fn a_pending_prefix_expires_and_an_unmatched_second_key_stands_alone() {
        let mut session = three_paths();
        let now = Instant::now();
        assert_eq!(session.deed_of(key('d'), Phase::Fresh, now), None);
        assert_eq!(
            session
                .views
                .pending(now + COMBO_WAITS + Duration::from_millis(1)),
            None
        );
        assert_eq!(
            session.deed_of(key('d'), Phase::Fresh, now + COMBO_WAITS * 2),
            None,
            "过期之后这一下又是前半截"
        );
        assert_eq!(
            session.deed_of(key('j'), Phase::Fresh, now + COMBO_WAITS * 2),
            Some(Deed::Down),
            "`dj` 合不上，`j` 自己算"
        );
        assert_eq!(session.deed_of(key('g'), Phase::Fresh, now), None);
        assert_eq!(
            session.deed_of(key('t'), Phase::Fresh, now),
            Some(Deed::NextView)
        );
    }

    /// **退出规则**（`CONTEXT.md` 的《退出会话》）：`q` 只在还没开始与结束了时退出，跑着与等待确认时
    /// 屏底说先按 `s` 或按 `C-c`；`C-c` 任何时候都退；`Esc` 从不退出。
    #[test]
    fn q_quits_only_before_and_after_a_run_ctrl_c_always_and_esc_never() {
        let mut session = three_paths();
        let now = Instant::now();
        let q = key('q');
        let esc = Input::Key(Key::Esc);
        let interrupt = Input::Key(Key::Interrupt);
        for (phase, stage) in [(Phase::Fresh, Stage::Fresh), (Phase::Ended, Stage::Ended)] {
            session.set_stage(stage);
            let deed = session.deed_of(q, phase, now).expect("q 派得出");
            assert_eq!(session.perform(deed, now), Exit::Leave, "{phase:?}");
        }
        for (phase, stage) in [
            (
                Phase::Surveying,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
        ] {
            session.set_stage(stage);
            let deed = session.deed_of(q, phase, now).expect("q 派得出");
            assert_eq!(session.perform(deed, now), Exit::Stay, "{phase:?}");
            let reply = session.views.reply(now).expect("屏底说为什么不退");
            assert!(reply[1].text.contains("先按 s 停止，或按 C-c"), "{reply:?}");
        }
        for (phase, stage) in [
            (Phase::Fresh, Stage::Fresh),
            (
                Phase::Running,
                Stage::Running(tonefit::Instruction::Continue),
            ),
            (
                Phase::Deciding,
                Stage::Deciding(tonefit::Instruction::Continue),
            ),
            (Phase::Ended, Stage::Ended),
        ] {
            session.set_stage(stage);
            let deed = session.deed_of(interrupt, phase, now).expect("C-c 派得出");
            assert_eq!(session.perform(deed, now), Exit::Leave, "{phase:?}");
            match session.deed_of(esc, phase, now) {
                None => {}
                Some(deed) => assert_eq!(session.perform(deed, now), Exit::Stay, "{phase:?}"),
            }
        }
    }

    /// `1`／`2` 换视图，`gt` 轮换。问的是屏底：在任务视图上摆着 `2 → 配置`，
    /// 切过去之后摆着 `1 → 任务`，切回来原样。
    #[test]
    fn the_two_views_switch_and_the_footer_follows() {
        let mut session = three_paths();
        let now = Instant::now();
        let footer = |session: &Session| -> Vec<String> {
            session
                .hints(Phase::Fresh)
                .iter()
                .map(|hint| format!("{} → {}", hint.spelt(), hint.what))
                .collect()
        };
        let task = footer(&session);
        assert!(task.contains(&"2 → 配置".to_owned()), "{task:?}");
        session.perform(Deed::ConfigView, now);
        let config = footer(&session);
        assert!(config.contains(&"1 → 任务".to_owned()), "{config:?}");
        assert!(!config.contains(&"2 → 配置".to_owned()), "{config:?}");
        session.perform(Deed::NextView, now);
        assert_eq!(footer(&session), task, "轮换回来原样");
        session.perform(Deed::PrevView, now);
        assert_eq!(footer(&session), config);
        session.perform(Deed::TaskView, now);
        assert_eq!(footer(&session), task);
    }

    /// 改了几项从套着的预设算：型号不算，取值环与自由填的项算。
    #[test]
    fn changed_items_are_counted_against_the_applied_preset_without_the_model() {
        let mut session = three_paths();
        assert_eq!(session.changed_from_preset(), 0, "没套预设");
        session.views.config.applied = Some(Applied {
            name: "漫画".to_owned(),
            preset: Preset::default(),
        });
        assert_eq!(session.changed_from_preset(), 0);
        session.device.profile = Some("kobo-libra-2".to_owned());
        assert_eq!(session.changed_from_preset(), 0, "型号不算");
        session.taste.fit = Some(tonefit::FitMode::Inside);
        session.taste.dither = Some(tonefit::Dither::FloydSteinberg);
        session.device.gray_levels = Some(12);
        assert_eq!(session.changed_from_preset(), 3);
    }
}
