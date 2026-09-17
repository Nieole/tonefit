//! **输入行**与**补全框**：打字时占住屏底的那一行（`CONTEXT.md` 的《会话》：输入行、家目录、
//! 大小写敏感性；spec《输入行与路径》）。
//!
//! 提示词 · 缓冲 · 光标；`o` 添加处理路径，`i`／`⏎` 修改一条，输出目录那一行上修改输出目录
//! （[`Purpose`]）。缓冲里是**用户打的写法**：`~/` 开头照 [`Home`](super::home::Home) 认，
//! 落到盘上才展开；补全回来的候选也按打的写法拼回去（[`InputLine::fill`]）。
//!
//! # 补全仍逐层、不递归、不建索引
//!
//! 按 `Tab` 只列打到的那一层（ADR 0009；[`super::complete::level`]，大小写认不认在那里探）：
//! 候选多于一个时留着这一份、屏底上方弹出**补全框**，再按 `Tab` 轮到下一个（[`InputLine::cycle`]）；
//! 只有一个就直接补上；一个都没有屏底说一句。打一个字、退一个字、删一段都把这一份作废
//! ——下一次 `Tab` 重新列一遍。
//!
//! # 确定与取消
//!
//! `⏎` 收下：找不到的路径**当场说**（问一次盘上有没有它），输出目录换掉、处理路径添上或改掉，
//! 光标停到那一条上；`Esc` 丢掉这一步。两样都关掉输入行，底下的块原样回来。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》），与 [`super::complete`] 同一侧
//! ——那一侧读盘，这一侧也只在补全与确定时问一次盘。画它的是 `super::shell::footer`（输入行）
//! 与 `super::shell::completions`（补全框）。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::complete::{self, SEPARATORS};
use super::look::{Kind, Look, Segment};
use super::state::{Field, NamedPath, Session};
use super::tone::Tone;
use super::view::Cursor;

/// 「这里没有以「…」开头的项」在屏底占多久（设计稿 `complete` 里那 1600 毫秒）。
const NO_MATCH_LINGERS: Duration = Duration::from_millis(1600);

/// 补全框至多露几行（设计稿 `drawCompletions` 的 `vis`）。
#[cfg_attr(
    not(feature = "tui"),
    allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
pub const CANDIDATES_SHOWN: usize = 8;

/// 一项设置**打了个空串**进来时屏底那一句里写什么（设计稿 `submitInput` 的 `'未设置'`）。
const VALUE_UNSET: &str = "未设置";

/// 屏上路径的写法用 `/`：与 [`Home::abbreviate`](super::home::Home::abbreviate) 同一条、与设计稿一致。
/// 认用户敲的分隔符时两种都认（[`SEPARATORS`]）。
const SHOWN_SEPARATOR: char = '/';

/// 输入行用在哪一件事上；提示词随它。给预设起名、搜索随各自的票添。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// 添加一条处理路径。
    AddPath,
    /// 修改这一条处理路径（改之前它是哪一条）。
    EditPath(PathBuf),
    /// 改输出目录。
    Output,
    /// **改一项设置的值**：配置视图里自由填的那几项（`CONTEXT.md` 的《详情栏》：
    /// 「自由填的那几项列当前值与 `i` 修改」）。
    Setting(Field),
}

impl Purpose {
    /// 提示词（设计稿 `startInput`）。**末尾那两格空算在提示词里**：设计稿按种类定它，
    /// 搜索那一种是 `/`、后面不空（随搜索那一票添）。改一项设置的值那一种的提示词
    /// **就是那一项的名字**，不另写一份。
    #[cfg_attr(
        not(feature = "tui"),
        allow(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn prompt(&self) -> String {
        match self {
            Self::AddPath => "添加路径  ".to_owned(),
            Self::EditPath(_) => "修改路径  ".to_owned(),
            Self::Output => "输出目录  ".to_owned(),
            Self::Setting(field) => format!("{}  ", field.label()),
        }
    }

    /// `Tab` 在这一种输入行上补得出东西吗——**只有路径那几种补得出**
    /// （设计稿 `complete`：别的种类直接返回）。
    ///
    /// 屏底右端那一件照旧按表摆（`Complete` 只在还没开始那一档派得出）：表上没有
    /// 「输入行用在哪件事上」那一维，停车场 Q794 记着这一处两边对不上的由来。
    fn completes(&self) -> bool {
        matches!(self, Self::AddPath | Self::EditPath(_) | Self::Output)
    }
}

/// 补全框里的一条（补全项）：这一层里的那个名字，文件夹还是文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub name: String,
    pub directory: bool,
}

impl Completion {
    /// 屏上怎么写：文件夹带 `/`。
    pub fn shown(&self) -> String {
        if self.directory {
            format!("{}{SHOWN_SEPARATOR}", self.name)
        } else {
            self.name.clone()
        }
    }

    /// 从屏上的写法认回来（[`shown`](Self::shown) 的反面）：带 `/` 的是文件夹。场景数据里的候选就是这副写法。
    pub fn from_shown(shown: &str) -> Self {
        Self {
            name: shown.trim_end_matches(SEPARATORS).to_owned(),
            directory: shown.ends_with(SEPARATORS),
        }
    }

    /// 旁边那一句：是压缩包就说「压缩包」（措辞在 [`NamedPath::kind_of`]），文件夹与别的文件不说。
    /// 设计稿把每一个文件都标成压缩包（停车场 Q791），这里按扩展名认。
    pub fn label(&self) -> Option<&'static str> {
        let path = Path::new(&self.name);
        (!self.directory && tonefit::is_archive(path)).then(|| NamedPath::kind_of(path))
    }
}

/// **输入行**：提示词 · 缓冲 · 光标，外加上一次 `Tab` 列出来的那一层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLine {
    pub purpose: Purpose,
    /// 打到哪儿了，按用户打的写法。
    pub buffer: String,
    /// 上一次 `Tab` 列出来的这一层（多于一个时才留着）。**只有列出来的这一份，不留索引、不留缓存**
    /// （ADR 0009）——改一个字就作废，下一次 `Tab` 重新列一遍。
    pub candidates: Vec<Completion>,
    /// 轮到第几个。
    pub at: usize,
    /// 候选所在的那一层，按打的写法（到最后一个 `/` 为止）：轮到哪一个就把它拼在这后面。
    head: String,
}

impl InputLine {
    /// 开一行：缓冲里先摆着什么（添加时是 `~/`，修改时是那一条）。
    pub fn new(purpose: Purpose, buffer: impl Into<String>) -> Self {
        Self {
            purpose,
            buffer: buffer.into(),
            candidates: Vec::new(),
            at: 0,
            head: String::new(),
        }
    }

    /// 打一个字。
    pub fn type_in(&mut self, glyph: char) {
        self.buffer.push(glyph);
        self.candidates.clear();
    }

    /// 退一个字。
    pub fn erase(&mut self) {
        self.buffer.pop();
        self.candidates.clear();
    }

    /// 删一段：末尾那一层连同它后面的分隔符（设计稿 `C-w` 那一支：`~/下载/` → `~/`，`~/Comics` → `~/`）。
    pub fn delete_word(&mut self) {
        let kept = self.buffer.strip_suffix(SEPARATORS).unwrap_or(&self.buffer);
        let cut = kept
            .rfind(|c: char| SEPARATORS.contains(&c) || c.is_whitespace())
            .map_or(0, |at| {
                at + kept[at..].chars().next().map_or(1, char::len_utf8)
            });
        self.buffer.truncate(cut);
        self.candidates.clear();
    }

    /// 缓冲拆成「哪一层」与「这一层里打到一半的那一截」：分界是最后一个分隔符（与 [`complete`] 同一份表）。
    pub fn split(&self) -> (&str, &str) {
        match self.buffer.rfind(SEPARATORS) {
            Some(at) => self.buffer.split_at(at + 1),
            None => ("", self.buffer.as_str()),
        }
    }

    /// 收下列出来的这一层——候选在 `head` 那一层里（按打的写法）：一个都没有交回 `false`；
    /// 只有一个直接补上；多于一个留着、轮到头一个。
    pub fn offer(&mut self, head: &str, candidates: Vec<Completion>) -> bool {
        if candidates.is_empty() {
            return false;
        }
        self.head = head.to_owned();
        if let [only] = candidates.as_slice() {
            self.buffer = format!("{}{}", self.head, only.shown());
            self.candidates.clear();
            return true;
        }
        self.candidates = candidates;
        self.at = 0;
        self.fill();
        true
    }

    /// 轮到下一个（到末尾绕回头一个）。
    pub fn cycle(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        self.at = (self.at + 1) % self.candidates.len();
        self.fill();
    }

    /// 在候选里挪几个（两头到底为止），缓冲跟着换。
    pub fn step(&mut self, delta: isize) {
        if self.candidates.is_empty() {
            return;
        }
        let last = self.candidates.len() - 1;
        self.at = self.at.saturating_add_signed(delta).min(last);
        self.fill();
    }

    /// 轮到的那一个拼进缓冲，按打的写法。
    fn fill(&mut self) {
        if let Some(candidate) = self.candidates.get(self.at) {
            self.buffer = format!("{}{}", self.head, candidate.shown());
        }
    }
}

impl Session {
    /// 打开输入行：添加一条（缓冲先摆 `~/`；问不出家目录时空着）。
    pub fn open_adding(&mut self) {
        let start = if self.home.is_known() { "~/" } else { "" };
        self.views.input = Some(InputLine::new(Purpose::AddPath, start));
    }

    /// 打开输入行改光标那一行：输出目录那一行上改输出目录，处理路径那一行上改那一条，
    /// 「＋ 添加路径」上添一条。
    pub fn open_editing(&mut self) {
        match self.views.task.cursor.clone() {
            Cursor::Output => {
                let start = match &self.scope.out {
                    Some(out) => format!("{}/", self.home_shown(out)),
                    None => String::new(),
                };
                self.views.input = Some(InputLine::new(Purpose::Output, start));
            }
            Cursor::Path(path) => {
                if self.path_under_cursor().is_some() {
                    let shown = self.home_shown(&path);
                    self.views.input = Some(InputLine::new(Purpose::EditPath(path), shown));
                }
            }
            Cursor::Add => self.open_adding(),
        }
    }

    /// 按 `Tab`：候选多于一个就轮到下一个，否则列打到的那一层（模块文档《补全仍逐层、不递归、不建索引》）。
    pub fn complete_typed(&mut self, now: Instant) {
        let Some(line) = &mut self.views.input else {
            return;
        };
        // 补全只对路径那几种有意义（[`Purpose::completes`]）：改一项设置的值按下去一个字都不动。
        if !line.purpose.completes() {
            return;
        }
        if line.candidates.len() > 1 {
            line.cycle();
            return;
        }
        let (head, prefix) = line.split();
        let prefix = prefix.to_owned();
        // 一个分隔符都没有的那一截在家目录底下找（设计稿 `complete` 的 `base = '~'`），补回来的也带上 `~/`。
        let head = if head.is_empty() && self.home.is_known() {
            format!("~{SHOWN_SEPARATOR}")
        } else {
            head.to_owned()
        };
        let on_disk = if head.is_empty() {
            String::new()
        } else {
            let expanded = self.home.expand(&head).display().to_string();
            if expanded.ends_with(SEPARATORS) {
                expanded
            } else {
                format!("{expanded}{SHOWN_SEPARATOR}")
            }
        };
        let listed: Vec<Completion> = complete::level(&format!("{on_disk}{prefix}"))
            .iter()
            .map(|hit| Completion {
                name: complete::name(hit).to_owned(),
                directory: hit.ends_with(SEPARATORS),
            })
            .collect();
        let Some(line) = &mut self.views.input else {
            return;
        };
        if !line.offer(&head, listed) {
            self.views.say_for(
                vec![Segment::new(
                    format!("这里没有以「{prefix}」开头的项"),
                    Look::tone(Tone::Caution),
                )],
                NO_MATCH_LINGERS,
                now,
            );
        }
    }

    /// 按 `⏎`：关掉输入行、收下打的那条路径（模块文档《确定与取消》）。
    pub fn confirm_typed(&mut self, now: Instant) {
        // 改一项设置的值不问盘：它收的是一个数、一个界、一个字节数，不是一条路径。
        if let Some(line) = &self.views.input
            && let Purpose::Setting(field) = line.purpose
        {
            let typed = line.buffer.trim().to_owned();
            self.settle_typed(field, &typed, now);
            return;
        }
        let Some(line) = self.views.input.take() else {
            return;
        };
        let typed = line.buffer.trim_end_matches(SEPARATORS).to_owned();
        if typed.is_empty() {
            return;
        }
        let on_disk = self.home.expand(&typed);
        if std::fs::metadata(&on_disk).is_err() {
            self.views.say(
                vec![
                    Segment::new("✗ ", Look::tone(Tone::Trouble).bold()),
                    Segment::new(format!("找不到「{typed}」"), Look::tone(Tone::Trouble)),
                ],
                now,
            );
            return;
        }
        let shown = self.home_shown(&on_disk);
        let done = || Segment::new("✓ ", Look::kind(Kind::Done).bold());
        match line.purpose {
            // 改一项设置的值上面那道岔路已经收走了，走不到这里。
            Purpose::Setting(_) => {}
            Purpose::Output => {
                self.scope.out = Some(on_disk);
                self.views.say(
                    vec![done(), Segment::plain(format!("输出目录 → {shown}"))],
                    now,
                );
            }
            Purpose::EditPath(before) => {
                if let Some(named) = self
                    .scope
                    .paths
                    .iter_mut()
                    .find(|named| named.path == before)
                {
                    named.path = on_disk.clone();
                    self.views.task.cursor = Cursor::Path(on_disk);
                }
            }
            Purpose::AddPath => {
                if self.scope.paths.iter().any(|named| named.path == on_disk) {
                    self.views.say(
                        vec![Segment::new(
                            "这个路径已经在列表里了",
                            Look::tone(Tone::Caution),
                        )],
                        now,
                    );
                    return;
                }
                let named = NamedPath {
                    path: on_disk.clone(),
                    on: true,
                };
                let kind = named.kind();
                self.scope.paths.push(named);
                self.views.task.cursor = Cursor::Path(on_disk);
                self.views.say(
                    vec![
                        done(),
                        Segment::plain(format!("已添加 {shown}")),
                        Segment::faint(format!(" ⋅ {kind}")),
                    ],
                    now,
                );
            }
        }
    }

    /// 打开输入行**改配置视图里光标那一项的值**：缓冲里先摆着它此刻的可编辑写法
    /// （空串代表「没说」）。光标停在别的项上、或者这一趟已经锁住设置时什么都不做。
    pub fn open_valuing(&mut self) {
        if self.settings_locked() {
            return;
        }
        let Some(field) = self.filled_item() else {
            return;
        };
        let start = self.typed(field);
        self.views.input = Some(InputLine::new(Purpose::Setting(field), start));
    }

    /// 收下改一项设置打出来的东西：空串是「没说」，落回默认值。
    ///
    /// **验的是那一项自己的界**（[`Session::take`]，与命令行、预设那两头同一处）；
    /// **解析不过就留在输入行上**——把用户打的东西丢掉再让他重打一遍是最差的那一种处置。
    fn settle_typed(&mut self, field: Field, typed: &str, now: Instant) {
        match self.take(field, typed) {
            Ok(()) => {
                self.views.input = None;
                let said = match typed.is_empty() {
                    true => VALUE_UNSET.to_owned(),
                    false => typed.to_owned(),
                };
                self.views.say(
                    vec![
                        Segment::new("✓ ", Look::kind(Kind::Done).bold()),
                        Segment::plain(format!("{} → {said}", field.label())),
                    ],
                    now,
                );
            }
            Err(error) => self.views.say(
                vec![
                    Segment::new("✗ ", Look::tone(Tone::Trouble).bold()),
                    Segment::new(format!("{error}"), Look::tone(Tone::Trouble)),
                ],
                now,
            ),
        }
    }

    /// 按 `Esc`：丢掉这一步，底下的块原样回来。
    pub fn cancel_typed(&mut self) {
        self.views.input = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::home::Home;
    use crate::session::keymap::{Deed, Phase};
    use crate::session::state::{Exit, Key};
    use crate::session::view::{Focus, Input};

    fn key(letter: char) -> Input {
        Input::Key(Key::Char(letter))
    }

    /// 临时目录作家目录，里面照设计稿假盘的一角：`Comics/` 底下三个文件夹一个压缩包、`漫画库/`。
    fn at_home() -> (tempfile::TempDir, Session) {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let home = space.path().join("home");
        for directory in [
            "Comics/棋魂",
            "Comics/大友克洋",
            "Comics/火之鸟",
            "漫画库",
            "转好的",
        ] {
            std::fs::create_dir_all(home.join(directory)).expect("建得出");
        }
        std::fs::write(home.join("Comics/合集.cbz"), b"").expect("写得出");
        let mut session = Session::new();
        session.home = Home::at(&home);
        session.scope.out = Some(home.join("转好的"));
        session.scope.paths.push(NamedPath {
            path: home.join("漫画库"),
            on: true,
        });
        session.views.task.cursor = Cursor::Path(home.join("漫画库"));
        (space, session)
    }

    fn typed(session: &Session) -> &str {
        session
            .views
            .input
            .as_ref()
            .map_or("", |line| line.buffer.as_str())
    }

    /// 补全项旁边那一句：压缩包说「压缩包」，文件夹与别的文件不说；屏上的写法与认回来是一对。
    #[test]
    fn a_completion_labels_only_archives_and_reads_back_from_its_shown_form() {
        let archive = Completion::from_shown("第01卷.cbz");
        assert_eq!(archive.label(), Some("压缩包"));
        assert_eq!(Completion::from_shown("棋魂/").label(), None);
        assert_eq!(Completion::from_shown("答案.txt").label(), None);
        for shown in ["棋魂/", "答案.txt", "第01卷.cbz"] {
            assert_eq!(Completion::from_shown(shown).shown(), shown);
        }
    }

    /// 缓冲上打字、退字、删一段：`C-w` 删的是末尾那一层连同它后面的 `/`。
    #[test]
    fn typing_erasing_and_deleting_a_word_edit_the_buffer_by_the_typed_spelling() {
        let mut line = InputLine::new(Purpose::AddPath, "~/");
        for glyph in "下载/灰原哀".chars() {
            line.type_in(glyph);
        }
        assert_eq!(line.buffer, "~/下载/灰原哀");
        line.erase();
        assert_eq!(line.buffer, "~/下载/灰原");
        line.delete_word();
        assert_eq!(line.buffer, "~/下载/");
        line.delete_word();
        assert_eq!(line.buffer, "~/");
        line.delete_word();
        assert_eq!(line.buffer, "", "`~/` 再删一段就空了");
        let mut line = InputLine::new(Purpose::Output, "~/Comics");
        line.delete_word();
        assert_eq!(line.buffer, "~/");
        assert_eq!(line.split(), ("~/", ""));
        line.type_in('C');
        assert_eq!(line.split(), ("~/", "C"));
    }

    /// `Tab` 列打到的那一层：候选多于一个时留着、缓冲先摆头一个，再按轮到下一个、到末尾绕回来；
    /// `↓` 到底为止；打一个字就作废。认 `~/`：列的是家目录底下那一层，拼回去的仍是 `~/` 写法。
    #[test]
    fn tab_lists_the_level_under_the_tilde_and_cycles_through_the_candidates() {
        let (_space, mut session) = at_home();
        let now = Instant::now();
        session.open_adding();
        assert_eq!(typed(&session), "~/");
        assert_eq!(session.views.focus(), Focus::Input);
        for glyph in "Comics/".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        session.perform(Deed::Complete, now);
        let line = session.views.input.as_ref().expect("输入行还开着");
        let listed: Vec<String> = line.candidates.iter().map(Completion::shown).collect();
        assert_eq!(
            listed,
            ["合集.cbz", "大友克洋/", "棋魂/", "火之鸟/"],
            "这一层按名字排"
        );
        assert_eq!((line.at, line.buffer.as_str()), (0, "~/Comics/合集.cbz"));
        session.perform(Deed::Complete, now);
        assert_eq!(typed(&session), "~/Comics/大友克洋/");
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        session.perform(Deed::Down, now);
        assert_eq!(typed(&session), "~/Comics/火之鸟/", "到底为止");
        session.perform(Deed::Complete, now);
        assert_eq!(typed(&session), "~/Comics/合集.cbz", "到末尾绕回头一个");
        session.perform(Deed::Typed('x'), now);
        assert!(
            session
                .views
                .input
                .as_ref()
                .expect("还开着")
                .candidates
                .is_empty(),
            "打一个字候选作废"
        );
        // 只有一个对得上就直接补上，不留候选；一个都没有屏底说一句、缓冲不动。
        session.perform(Deed::DeleteWord, now);
        for glyph in "火".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        session.perform(Deed::Complete, now);
        assert_eq!(typed(&session), "~/Comics/火之鸟/");
        assert!(
            session
                .views
                .input
                .as_ref()
                .expect("还开着")
                .candidates
                .is_empty()
        );
        session.perform(Deed::Typed('z'), now);
        session.perform(Deed::Complete, now);
        assert_eq!(typed(&session), "~/Comics/火之鸟/z");
        let reply = session.views.reply(now).expect("屏底说了一句");
        assert_eq!(reply[0].text, "这里没有以「z」开头的项");
        assert_eq!(session.views.reply(now + NO_MATCH_LINGERS), None);
        // 一个分隔符都没有的那一截在家目录底下找，补回来的带上 `~/`（设计稿 `base = '~'`）。
        session.perform(Deed::Cancel, now);
        session.open_adding();
        session.perform(Deed::DeleteWord, now);
        assert_eq!(typed(&session), "");
        session.perform(Deed::Typed('C'), now);
        session.perform(Deed::Complete, now);
        assert_eq!(typed(&session), "~/Comics/");
    }

    /// `⏎` 收下：找不到的路径当场说、一个字都不改；找得到的添上（勾着、光标停到它上面、屏底说一句）；
    /// 同一条不添第二次；`Esc` 丢掉这一步。两样都关掉输入行。
    #[test]
    fn enter_adds_the_path_when_it_exists_and_says_so_when_it_does_not() {
        let (_space, mut session) = at_home();
        let now = Instant::now();
        session.open_adding();
        for glyph in "没有这个".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        assert_eq!(session.perform(Deed::Confirm, now), Exit::Stay);
        assert_eq!(session.views.input, None, "输入行关了");
        assert_eq!(session.views.focus(), Focus::VolumeList);
        assert_eq!(session.scope.paths.len(), 1);
        let reply = session.views.reply(now).expect("屏底说了一句");
        assert_eq!(
            (reply[0].text.as_str(), reply[1].text.as_str()),
            ("✗ ", "找不到「~/没有这个」")
        );
        session.open_adding();
        for glyph in "Comics/火之鸟/".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        session.perform(Deed::Confirm, now);
        assert_eq!(session.scope.paths.len(), 2);
        assert!(session.scope.paths[1].on);
        assert_eq!(session.cursor_position(), (3, 4), "光标停到新添的那一条上");
        let reply = session.views.reply(now).expect("屏底说了一句");
        let said: Vec<&str> = reply.iter().map(|segment| segment.text.as_str()).collect();
        assert_eq!(said, ["✓ ", "已添加 ~/Comics/火之鸟", " ⋅ 文件夹"]);
        session.open_adding();
        for glyph in "Comics/火之鸟".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        session.perform(Deed::Confirm, now);
        assert_eq!(session.scope.paths.len(), 2, "同一条不添第二次");
        assert_eq!(
            session.views.reply(now).expect("屏底说了一句")[0].text,
            "这个路径已经在列表里了"
        );
        session.open_adding();
        session.perform(Deed::Typed('x'), now);
        session.perform(Deed::Cancel, now);
        assert_eq!(session.views.input, None);
        assert_eq!(session.scope.paths.len(), 2);
    }

    /// `i` 在输出目录那一行上改输出目录（缓冲先摆 `~/转好的/`），在处理路径那一行上改那一条
    /// （缓冲先摆它），改完光标仍停在那一条上、勾选不变；压缩包按扩展名认。
    #[test]
    fn editing_changes_the_output_directory_or_the_path_under_the_cursor() {
        let (_space, mut session) = at_home();
        let now = Instant::now();
        session.perform(Deed::Up, now);
        session.open_editing();
        assert_eq!(typed(&session), "~/转好的/");
        session.perform(Deed::DeleteWord, now);
        for glyph in "Comics".chars() {
            session.perform(Deed::Typed(glyph), now);
        }
        session.perform(Deed::Confirm, now);
        assert_eq!(session.output_shown(), "~/Comics");
        let said: Vec<String> = session
            .views
            .reply(now)
            .expect("屏底说了一句")
            .iter()
            .map(|segment| segment.text.clone())
            .collect();
        assert_eq!(said, ["✓ ", "输出目录 → ~/Comics"]);
        // 上一句退回之后再改一条：改一条不说话。
        let later = now + crate::session::view::REPLY_LINGERS;
        session.perform(Deed::Down, later);
        session.scope.paths[0].on = false;
        session.open_editing();
        assert_eq!(typed(&session), "~/漫画库");
        for _ in 0..3 {
            session.perform(Deed::Erase, later);
        }
        for glyph in "Comics/合集.cbz".chars() {
            session.perform(Deed::Typed(glyph), later);
        }
        session.perform(Deed::Confirm, later);
        assert_eq!(session.scope.paths.len(), 1);
        assert_eq!(
            session.home_shown(&session.scope.paths[0].path),
            "~/Comics/合集.cbz"
        );
        assert!(!session.scope.paths[0].on, "勾选不变");
        assert_eq!(session.scope.paths[0].kind(), "压缩包");
        assert_eq!(session.cursor_position(), (2, 3), "光标仍停在那一条上");
        assert_eq!(session.views.reply(later), None, "改一条不说话");
    }

    /// 打字时每一个字符都是一个字：`?`、`q`、`j`、空格都进缓冲；`Tab`、`⏎`、`Esc`、`F1`、`C-w`、`⌫` 照表派。
    #[test]
    fn while_typing_every_character_is_a_character_and_the_input_keys_come_from_the_table() {
        let (_space, mut session) = at_home();
        let now = Instant::now();
        session.open_adding();
        for (input, deed) in [
            (key('?'), Deed::Typed('?')),
            (key('q'), Deed::Typed('q')),
            (key('j'), Deed::Typed('j')),
            (key('d'), Deed::Typed('d')),
            (Input::Key(Key::Space), Deed::Typed(' ')),
            (Input::Key(Key::Tab), Deed::Complete),
            (Input::Key(Key::Enter), Deed::Confirm),
            (Input::Key(Key::Esc), Deed::Cancel),
            (Input::Key(Key::F1), Deed::HelpWhileTyping),
            (Input::Ctrl('w'), Deed::DeleteWord),
            (Input::Key(Key::Backspace), Deed::Erase),
            (Input::Key(Key::Interrupt), Deed::Interrupt),
        ] {
            assert_eq!(
                session.deed_of(input, Phase::Fresh, now),
                Some(deed),
                "{input:?}"
            );
            assert_eq!(session.views.pending(now), None, "打字时没有连击键");
        }
        assert_eq!(session.deed_of(Input::Ctrl('d'), Phase::Fresh, now), None);
    }
}
