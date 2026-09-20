//! **场景夹具**：按设计稿导出的[场景数据](CONTEXT.md)摆出那一趟与三组设置
//! （`session-redesign/05`；spec《夹具》）。
//!
//! 读 `tests/fixtures/design/scenes/<场景>.json`（或一串交互走完那一刻的
//! `sequences/<名>.scene.json`），按它造各卷的 `VolumeReport`，沿真跑那条路把事件喂进
//! [`Live`]（开工带清单、开卷、环节、步、收摊……），给定「此刻」；三组设置摆进 [`Session`]，
//! 预设文件放临时目录（`Presets::at`，与 `terminal` 的用例同一招）；临时目录里照场景数据
//! 提到的每一处建出那棵树，作家目录。**数据只从场景数据来**：不在 Rust 里重写设计稿的
//! 伪随机与模拟（Q718）；逐页结果只有屏上开着的那一卷有整份，其余各卷照灰阶分布补页（Q736）。
//!
//! 会话的**界面状态**（视图、光标、展开、覆盖层、输入行……）按 [`Data::session`] 摆进
//! [`Session::views`]，**各票接上自己那一块**：本票（`session-redesign/06`）摆的是视图、
//! 开跑之前卷列表的光标与套着的预设（[`views_of`]）；树上的光标、展开、每页结果、覆盖层、
//! 输入行随各票补（都在 [`views_of`] 那一处，认一条备注行要树，那一步在
//! [`stand_on_a_note`]）。这里另摆那一趟（[`Scene::live`]）、三组设置
//! （[`Scene::session`] 上的 `device`／`taste`／`scope`，加上阶段）、家目录与预设文件。
//!
//! 夹具**不读写用户配置目录、不改进程的环境变量**：家目录与预设文件都在临时目录里，
//! 由 [`Scene`] 的字段交出去、由调用方往下传（spec《输入行与路径》：家目录由会话入口问一次往下传，
//! 用例给临时目录）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;
use tempfile::TempDir;
use tonefit::{
    BitDepth, Candidate, CandidateScore, Crop, Dither, Envelope, GeometryGate, Instruction,
    Mode as RunMode, NonVolumeFile, NonVolumeReason, PageBranch, PageColor, PageOutcome,
    PageReport, Pass, Processed, Reason, RunOutcome, Salvage, Scaling, Score, Size, SurveyedVolume,
    UnreachablePlace, Verdict, VolumeReport, VolumeVerdict, WhiteAlignment,
};

use super::config::Item;
use super::cover::Overlay;
use super::home::Home;
use super::live::{Live, Reach, Resuming, fixture};
use super::state::{DEVICE_FIELDS, Field, Key, Listing, NamedPath, Session, TASTE_FIELDS};
use super::typing::{Completion, InputLine, Purpose};
use super::view::{Applied, Cursor, Focus, Input, Pages, View, Views};
use crate::preset::{self, Preset, Presets};
use crate::render;

/// 导出的产物住在哪儿（相对仓库根）。与 `super::draw::design::FIXTURES` 同一个目录，
/// 各写各的：那一份在 `tui` 后面，这一份两趟都编。
const FIXTURES: &str = "tests/fixtures/design";

/// 设备设置在预设文件里的三个键（`crate::preset::OnDiskDevice`，kebab-case）。
/// 场景数据的设置是一张平的表，写成 TOML 时按这三个分进 `device` 那一节，其余归 `taste`。
const DEVICE_KEYS: [&str; 3] = ["profile", "gray-levels", "threshold"];

/// 三遍按设计稿的编号：`pass` 0 是幂等那一道、1 是分析环节、2 是写出环节，3 是三遍都走完。
const PASSES: [Pass; 3] = [Pass::Fingerprint, Pass::First, Pass::Second];

// ───────────────────────── 场景数据的形状 ─────────────────────────
//
// 字段以导出脚本的 `sceneData` 为准（`.scratch/session-redesign/export.js`）。
// 这里只读夹具要的那几样；`session` 那一段整个留成 JSON，由后面各票按需读。

/// 一份场景数据。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Data {
    /// 场景名。
    pub(crate) scene: String,
    /// 设计稿那只表冻在第几毫秒（`render.js` 把 `performance.now` 冻在它上）。
    /// **转轮转到第几格从它算**：夹具照它往回推出会话的时钟起点
    /// （`super::state::Session::opened_at`：会话打开那一刻）——转轮与这一趟跑了多久无关，
    /// 清点中那一段一步都没走，它照样得转。
    #[serde(default)]
    pub(crate) now_ms: u64,
    /// 输出目录（`~/` 写法）。
    pub(crate) output: String,
    /// 处理路径与勾选。
    pub(crate) paths: Vec<Named>,
    /// 三组设置里进预设的那两组，键照预设文件（kebab-case），取值照那一项的类型；`null` 是「没说」。
    pub(crate) settings: BTreeMap<String, Value>,
    /// 套的是哪一份预设。
    pub(crate) applied_preset: Option<String>,
    /// 预设文件里的那几份。
    pub(crate) presets: Vec<PresetData>,
    /// 这一趟。还没开跑的场景是 `None`。
    pub(crate) run: Option<Run>,
    /// 会话的界面状态，原样留着：视图、光标、展开、每页结果、覆盖层、输入行、搜索、回话、
    /// 连击键、配置视图那几格。**本模块不照它摆任何东西**（06 起各票按需读）；只读两格：
    /// 建假盘时看输入行补全到了哪儿（[`disk`]），核字网格时看每页结果开着哪一卷。
    pub(crate) session: Value,
}

/// 一条处理路径。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Named {
    pub(crate) path: String,
    /// `directory` 或 `archive`。
    pub(crate) kind: String,
    pub(crate) checked: bool,
}

/// 一份预设：名字，与它说到的那几项。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PresetData {
    pub(crate) name: String,
    pub(crate) says: BTreeMap<String, Value>,
}

/// 这一趟。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Run {
    /// `process` 或 `dry-run`。
    pub(crate) mode: String,
    /// `surveying` · `running` · `deciding` · `ended`。
    pub(crate) stage: String,
    /// 结束了才有：`done` · `stopped` · `aborted`。
    pub(crate) outcome: Option<String>,
    /// 整卷统一灰阶开没开。
    pub(crate) envelope: bool,
    /// 开工到此刻几秒（已乘设计稿的时间倍数）。
    pub(crate) elapsed_s: f64,
    /// 走了几步（设计稿的模拟按连续时间推进，因此带小数）。
    pub(crate) steps: f64,
    /// 共几步。
    pub(crate) total_steps: u64,
    /// 当前卷的卷根。
    pub(crate) current: Option<String>,
    /// 按停止到第几级。
    pub(crate) stop_level: u8,
    /// 答过「后面的卷都写出」没有。
    pub(crate) for_the_rest: bool,
    /// 到此刻为止真写出过东西没有。
    pub(crate) wrote: bool,
    /// 清点清单。
    pub(crate) survey: Survey,
    /// 每卷此刻怎么样，与 [`Survey::volumes`] 同序。
    pub(crate) volumes: Vec<VolumeData>,
}

/// 清点清单：树的顶层、全部卷、没勾的几条。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Survey {
    pub(crate) entries: Vec<Entry>,
    pub(crate) volumes: Vec<Listed>,
    pub(crate) unchecked: usize,
}

/// 卷列表顶层的一项：分区，或顶格的目录行。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum Entry {
    #[serde(rename = "section")]
    Section {
        path: String,
        directories: Vec<String>,
        notes: Vec<Note>,
    },
    #[serde(rename = "directory")]
    Directory { root: String, notes: Vec<Note> },
}

impl Entry {
    fn notes(&self) -> &[Note] {
        match self {
            Entry::Section { notes, .. } | Entry::Directory { notes, .. } => notes,
        }
    }
}

/// 一条备注：无法访问的地方一处一条，非漫画文件一组一条。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Note {
    pub(crate) kind: NoteKind,
    pub(crate) entries: Vec<NoteEntry>,
}

/// 备注的两种：报告末尾那两小结拆散后挂回来的那一条（`CONTEXT.md` 的《备注行》）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) enum NoteKind {
    #[serde(rename = "unreachable")]
    Unreachable,
    #[serde(rename = "non-volume")]
    NonVolume,
}

/// 备注里的一条：路径，与那句原因（渲染后的整句）。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct NoteEntry {
    pub(crate) path: String,
    pub(crate) reason: String,
}

/// 清单上的一卷。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Listed {
    pub(crate) root: String,
    pub(crate) directory: String,
    pub(crate) name: String,
    pub(crate) source_pages: usize,
    pub(crate) steps: u64,
}

/// 一卷此刻怎么样。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VolumeData {
    pub(crate) root: String,
    /// `queued` · `running` · `deciding` · `done` · `isolated` · `skipped` · `failed` · `aborted` · `trialed`。
    pub(crate) state: String,
    /// 走到哪一遍（见 [`PASSES`]）；还没轮到是 -1。
    pub(crate) pass: i8,
    /// 这一遍走到第几页（连续时间推进，带小数）。
    pub(crate) done: f64,
    pub(crate) elapsed_s: f64,
    /// 灰阶分布：档位与页数，页多的在前。一页判定都没有是 `None`。
    pub(crate) tally: Option<Vec<(String, usize)>>,
    pub(crate) page_count: Option<usize>,
    /// 需留意的那几页（没开着的卷给这个）。
    pub(crate) notable_pages: Option<Vec<PageData>>,
    /// 整份逐页（屏上开着的那一卷才有，Q736）。
    pub(crate) pages: Option<Vec<PageData>>,
    /// 没做成的原因。
    pub(crate) failure: Option<String>,
    /// 进了隔离时它的去处。
    pub(crate) isolated_output: Option<String>,
}

/// 一页。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PageData {
    pub(crate) name: String,
    pub(crate) output_size: (u32, u32),
    /// 需留意在哪几处：`failed` · `outlier` · `wide` · `gate` · `salvage` · `driver`。
    pub(crate) kinds: Vec<String>,
    pub(crate) source_height: Option<u32>,
    pub(crate) verdict: Option<String>,
    /// 库 `Reason` 的种类名（kebab-case）。
    pub(crate) reason: Option<String>,
    pub(crate) score: Option<f32>,
    pub(crate) failure: Option<String>,
    pub(crate) salvaged_percent: Option<f64>,
    #[serde(default)]
    pub(crate) driver: bool,
}

// ───────────────────────── 读 ─────────────────────────

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES)
}

/// 读产物目录里的一份 JSON。
fn json(file: &str) -> Value {
    let path = fixtures().join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到 {}：{error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{file} 不是 JSON：{error}"))
}

/// 清单上有场景数据的那几个场景，照清单的次序、不重。
pub(crate) fn scenes() -> Vec<String> {
    let manifest = json("manifest.json");
    let mut names = Vec::new();
    for entry in manifest["snapshots"]
        .as_array()
        .expect("清单上有快照那一列")
    {
        if entry.get("data").is_some() {
            let name = entry["scene"].as_str().expect("每一份快照记着场景名");
            if !names.iter().any(|seen| seen == name) {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// 清单上的全部交互序列名。
pub(crate) fn sequences() -> Vec<String> {
    let manifest = json("manifest.json");
    manifest["sequences"]
        .as_array()
        .expect("清单上有序列那一列")
        .iter()
        .map(|entry| entry["name"].as_str().expect("每一串记着名字").to_owned())
        .collect()
}

/// **交互序列**的一步（`CONTEXT.md` 的《会话》：交互序列；`manifest.json` 的 `steps`）。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Step {
    /// 按一个键，名字照清单上的写法：`j`、`Space`、`Enter`、`Escape`、`Tab`、`F1`、`C-w`。
    Key(String),
    /// 打一串字。
    Type(String),
    /// 单击第几列第几行。
    Click(u16, u16),
    /// 双击第几列第几行。
    DoubleClick(u16, u16),
    /// 滚轮几格（负数往上）。
    Wheel(i16),
    /// 推进几秒（屏上的秒）。
    Advance(f64),
    /// 换尺寸：列 × 行。
    Resize(u16, u16),
}

impl Step {
    /// 这一步交给新会话的输入。打字那一步是一个一个字符（各自一个输入）；推进与换尺寸不是输入。
    pub(crate) fn inputs(&self) -> Vec<Input> {
        match self {
            Self::Key(name) => vec![key_named(name)],
            Self::Type(text) => text.chars().map(|c| Input::Key(Key::Char(c))).collect(),
            Self::Click(x, y) => vec![Input::Click { x: *x, y: *y }],
            // 双击等于 `⏎`：终端层按阈值内的第二下认出来（随鼠标那一票），这里先给两下。
            Self::DoubleClick(x, y) => vec![Input::Click { x: *x, y: *y }; 2],
            Self::Wheel(notches) => vec![Input::Wheel(*notches)],
            Self::Advance(_) | Self::Resize(_, _) => Vec::new(),
        }
    }
}

/// 清单上一个键的名字 → 输入。设计稿的键名照浏览器的 `KeyboardEvent.key`，几个记号另有名字。
fn key_named(name: &str) -> Input {
    match name {
        "Space" => Input::Key(Key::Space),
        "Enter" => Input::Key(Key::Enter),
        "Escape" => Input::Key(Key::Esc),
        "Tab" => Input::Key(Key::Tab),
        "Backspace" => Input::Key(Key::Backspace),
        "F1" => Input::Key(Key::F1),
        "C-c" => Input::Key(Key::Interrupt),
        _ => match name.strip_prefix("C-") {
            Some(letter) if letter.chars().count() == 1 => {
                Input::Ctrl(letter.chars().next().expect("一个字母"))
            }
            _ => {
                let mut chars = name.chars();
                let (Some(c), None) = (chars.next(), chars.next()) else {
                    panic!("清单上的键名认不出：{name}");
                };
                Input::Key(Key::Char(c))
            }
        },
    }
}

/// 一串交互序列：从哪个场景起、多大的屏、逐步输入。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sequence {
    pub(crate) name: String,
    pub(crate) scene: String,
    pub(crate) size: (u16, u16),
    pub(crate) steps: Vec<Step>,
}

/// 清单上这一串序列。
pub(crate) fn sequence(name: &str) -> Sequence {
    let manifest = json("manifest.json");
    let entry = manifest["sequences"]
        .as_array()
        .expect("清单上有序列那一列")
        .iter()
        .find(|entry| entry["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("清单上没有「{name}」这一串"));
    let pair = |value: &Value| -> (u16, u16) {
        let both = value.as_array().expect("两个数");
        (
            both[0].as_u64().expect("列数") as u16,
            both[1].as_u64().expect("行数") as u16,
        )
    };
    let steps = entry["steps"]
        .as_array()
        .expect("这一串有步")
        .iter()
        .map(|step| {
            if let Some(key) = step["key"].as_str() {
                Step::Key(key.to_owned())
            } else if let Some(text) = step["type"].as_str() {
                Step::Type(text.to_owned())
            } else if step.get("click").is_some() {
                let (x, y) = pair(&step["click"]);
                Step::Click(x, y)
            } else if step.get("dblclick").is_some() {
                let (x, y) = pair(&step["dblclick"]);
                Step::DoubleClick(x, y)
            } else if let Some(notches) = step["wheel"].as_i64() {
                Step::Wheel(notches as i16)
            } else if let Some(seconds) = step["advance"].as_f64() {
                Step::Advance(seconds)
            } else if step.get("resize").is_some() {
                let (cols, rows) = pair(&step["resize"]);
                Step::Resize(cols, rows)
            } else {
                panic!("「{name}」里这一步认不出：{step}")
            }
        })
        .collect();
    Sequence {
        name: name.to_owned(),
        scene: entry["scene"].as_str().expect("从哪个场景起").to_owned(),
        size: pair(&entry["size"]),
        steps,
    }
}

fn parse(value: Value) -> Data {
    serde_json::from_value(value)
        .unwrap_or_else(|error| panic!("场景数据读不成夹具的形状：{error}"))
}

/// 一个场景的场景数据。
pub(crate) fn scene_data(scene: &str) -> Data {
    parse(json(&format!("scenes/{scene}.json")))
}

/// 一串交互走完那一刻的场景数据：与起点场景一样的那几样导出时写成 `"unchanged"`，
/// 这里从起点场景那一份补回来。
pub(crate) fn sequence_data(sequence: &str) -> Data {
    let mut after = json(&format!("sequences/{sequence}.scene.json"));
    let scene = after["scene"]
        .as_str()
        .expect("每一串记着起点场景")
        .to_owned();
    let base = json(&format!("scenes/{scene}.json"));
    let fields = after.as_object_mut().expect("场景数据是一张表");
    for (key, value) in base.as_object().expect("场景数据是一张表") {
        if fields.get(key).and_then(Value::as_str) == Some("unchanged") {
            fields.insert(key.clone(), value.clone());
        }
    }
    parse(after)
}

// ───────────────────────── 摆出来 ─────────────────────────

/// 一个场景摆出来的样子。
pub(crate) struct Scene {
    /// 它是哪一份：场景名，或者序列名（对不上时报出来）。
    pub(crate) label: String,
    /// 它照的那份场景数据。
    pub(crate) data: Data,
    /// 临时目录：家目录与预设文件都在它底下。拿着它，那棵树才活着。
    space: TempDir,
    /// **家目录**：场景数据里的 `~`。照设计稿的假盘建出来的那棵树就是它。
    pub(crate) home: PathBuf,
    /// 预设文件，在临时目录里（不在家目录底下：家目录底下只有假盘上那几样）。
    pub(crate) presets: Presets,
    /// 三组设置摆好了的会话，阶段照这一趟，家目录与界面状态里本票那几格也摆好了（见模块文档）。
    pub(crate) session: Session,
    /// 那一趟。还没开跑的场景是 `None`。
    pub(crate) live: Option<Live>,
    /// 开工那一刻。「此刻」是它加上场景数据里的已用秒数（[`Scene::now`]）。
    pub(crate) epoch: Instant,
}

impl Scene {
    /// 摆出一个场景：`scenes/<场景>.json`。
    pub(crate) fn named(scene: &str) -> Self {
        Self::from_data(scene, scene_data(scene))
    }

    /// 摆出一串交互走完那一刻：`sequences/<名>.scene.json`。
    pub(crate) fn after(sequence: &str) -> Self {
        Self::from_data(sequence, sequence_data(sequence))
    }

    fn from_data(label: &str, data: Data) -> Self {
        let space = tempfile::tempdir().expect("建得出临时目录");
        let home = space.path().join("home");
        build_disk(&home);
        let presets = write_presets(space.path(), &data.presets);
        let mut session = Session::new();
        let Preset { device, taste } = preset_of(&data.settings);
        session.device = device;
        session.taste = taste;
        session.scope.out = Some(expand(&home, &data.output));
        session.scope.paths = data
            .paths
            .iter()
            .map(|named| NamedPath {
                path: expand(&home, &named.path),
                on: named.checked,
            })
            .collect();
        session.home = Home::at(&home);
        let epoch = Instant::now();
        let live = data
            .run
            .as_ref()
            .map(|run| replay(run, &home, &data.output, epoch, &mut session));
        // **界面状态摆在回放之后**：起一趟那一下会把树、展开与自动滚动扳回开跑那一刻的样子
        // （`Session::run_started`），场景数据说的那几格要压在它上面。
        session.views = views_of(&data, &home, &presets);
        // **会话的时钟起点，两格都摆在这一句之后**：设计稿那只表冻在 `now_ms` 上，照它往回推,
        // 屏上那几个转轮因此都停在设计稿导出那一刻的那一格（顶栏那一截读
        // [`super::view::Views::spinning`]，行首记号与总览读 `shell::marks::spinner`）。
        //
        // 摆在上面那一句**之前**会被它抹掉——`views_of` 整份换掉 `session.views`，
        // 而这正是 08 与 13 两张票各设一格、合起来差了一格转轮的那道坑（停车场 Q864）。
        let back = Duration::from_millis(data.now_ms);
        let origin = epoch + elapsed(data.run.as_ref());
        session.views.clock = origin.checked_sub(back);
        session.opened_at = origin - back;
        // 清点的产出到了就把树拼出来，自动滚动开着时光标跟到正在处理的那一卷——
        // 与真会话里那一层做的是同一件事（`super::terminal::input`）。
        if let Some(live) = &live {
            session.watch_the_run(live);
        }
        // **树拼出来之后才认得出备注行**：光标与说明卡都按那一条备注的「是哪几处」认。
        stand_on_a_note(&mut session, &data);
        Self {
            label: label.to_owned(),
            data,
            space,
            home,
            presets,
            session,
            live,
            epoch,
        }
    }

    /// 那一趟。没开跑的场景上问它是用例写错了景，当场说出来。
    pub(crate) fn live(&self) -> &Live {
        self.live
            .as_ref()
            .unwrap_or_else(|| panic!("「{}」还没开跑，没有那一趟", self.label))
    }

    /// 场景数据里的一条路径落到盘上是哪儿：`~/` 换成家目录。
    pub(crate) fn path(&self, tilde: &str) -> PathBuf {
        expand(&self.home, tilde)
    }

    /// 这一景的「此刻」：开工那一刻加上已用的秒数。没开跑就是开工那一刻。
    pub(crate) fn now(&self) -> Instant {
        self.epoch + elapsed(self.data.run.as_ref())
    }

    /// **序列里「推进几秒」那一步**：把手上那一趟换成 `data` 说的那一趟，「此刻」跟着走
    /// （[`Scene::now`] 之后答的就是新的那一刻），新的那一份 [`Live`] 摆回 [`Scene::live`]
    /// ——与[摆一个场景](Scene::from_data)交出来的形状相同，调用方照旧 `take` 它。
    ///
    /// **夹具没有线程**（停车场 Q805 记的是同一条）：推进那几秒里那条线程做了什么，
    /// 只有**这一串自己的场景数据**说得出——`s` 按一次之后再推进 30 秒，当前那一卷做完、
    /// 那一趟收了场，而这一头没有东西去把那一卷跑完。照它在**同一个家目录**上重放一遍，
    /// 盘、预设、三组设置都不动。
    ///
    /// **界面状态一格不动**：视图、光标、展开、掀着的那一张都是前面那几步输入摆出来的，
    /// 而那正是这一串要看的东西——[`replay`] 起手要走一遍「起一趟」
    /// （[`Session::run_started`]，那一下会把它们扳回开跑那一刻），因此前后各存回一次。
    pub(crate) fn advance_to(&mut self, data: Data) {
        let run = data.run.as_ref().expect("推进之后那一串仍在一趟里");
        let views = self.session.views.clone();
        let live = replay(run, &self.home, &data.output, self.epoch, &mut self.session);
        self.session.views = views;
        // 会话打开那一刻：往回推设计稿那一头的钟（与 [`Scene::from_data`] 同一条式子）。
        self.session.opened_at =
            self.epoch + elapsed(data.run.as_ref()) - Duration::from_millis(data.now_ms);
        self.data = data;
        // 与真会话里那一层每一下做的是同一件事：清点的产出已经在了，树不必重拼；
        // 结束了的那一趟自动滚动不再跟（`Session::watch_the_run`）。
        self.session.watch_the_run(&live);
        self.live = Some(live);
    }
}

/// 场景数据 `session` 那一段里认得的几格：视图、开跑之前卷列表的光标、套着的预设（06）；
/// 输入行连同它列着的候选、全部按键那一张与它从第几行画起（07）；配置视图那几格
/// （在哪一栏、两栏各自的光标、下钻进了哪一块，加上改一项设置的值那种输入行，13）；
/// 树上的光标、展开与自动滚动（08）；搜索那一句连同搜索那一种输入行（09）；
/// 每页结果开着哪一卷、光标停在第几页、列的是哪几页（11）。
/// 认不得的先停在输出目录那一行上；给预设起名那一种输入行随那一票认。
fn views_of(data: &Data, home: &Path, presets: &Presets) -> Views {
    let mut views = Views::default();
    views.view = match data.session["view"].as_str() {
        Some("config") => View::Config,
        _ => View::Task,
    };
    let cursor = &data.session["cursor"];
    let at = |key: &str| {
        expand(
            home,
            cursor[key]
                .as_str()
                .unwrap_or_else(|| panic!("光标那一{key}")),
        )
    };
    views.task.cursor = match cursor["kind"].as_str() {
        Some("out") => Cursor::Output,
        Some("add") => Cursor::Add,
        Some("path") => Cursor::Path(at("path")),
        Some("directory") => Cursor::Directory(at("root")),
        Some("volume") => Cursor::Volume(at("root")),
        // **备注行那一种在这里认不出**：场景数据记的是那一条备注的「是哪几处」，
        // 而认它要树，树在这一步之后才拼出来（见 [`stand_on_a_note`]）。
        Some("note") => Cursor::Output,
        _ => Cursor::Output,
    };
    // 展开着的那几个目录，与自动滚动开着没有。
    if let Some(expanded) = data.session["expanded"].as_array() {
        views.task.expanded = expanded
            .iter()
            .filter_map(Value::as_str)
            .map(|path| expand(home, path))
            .collect();
    }
    views.task.follow = data.session["follow"].as_bool().unwrap_or(true);
    // **每页结果**：`pages` 那一段在场就是进了一卷（场景数据的 `S.pages`）。
    // 它只记卷根、第几页与列哪几页——列出来的是哪几页由那一趟的报告现算
    // （[`super::view::Pages::listed`]），夹具这一头不存第二份。
    let pages = &data.session["pages"];
    views.task.pages = pages["volume"].as_str().map(|root| Pages {
        volume: expand(home, root),
        at: pages["cursor"].as_u64().unwrap_or(0) as usize,
        listing: if pages["all"].as_bool().unwrap_or(false) {
            Listing::All
        } else {
            Listing::Notable
        },
    });
    // **搜索那一句**：`⏎` 定下来的那一份。搜索那一行开着时缓冲本身就是此刻搜的那一句
    // （`Views::searching`），两头对得上（设计稿那一景 `S.input.buf` 与 `S.search.q` 同值）。
    views.task.search = data.session["search"]["query"].as_str().map(str::to_owned);
    views.config.applied = data.applied_preset.as_ref().map(|name| Applied {
        name: name.clone(),
        preset: presets
            .read(name)
            .unwrap_or_else(|error| panic!("套着的预设「{name}」读不出：{error:#}")),
    });
    config_of(&data.session["config"], &mut views);
    // **预设文件那一条路径设计稿是写死的**（`drawConfig`），它不是场景数据——夹具因此照它
    // 摆一条家目录底下的路径。真文件仍在临时目录里（[`write_presets`]）：家目录底下只有
    // 假盘那几样，补全那几串数的正是它（停车场 Q824）。
    views.presets = Some(home.join(".config").join("tonefit").join("presets.toml"));
    let input = &data.session["input"];
    let purpose = match input["kind"].as_str() {
        Some("add") => Some(Purpose::AddPath),
        Some("out") => Some(Purpose::Output),
        // 改哪一条设计稿没导出：光标停在的那一条就是（改着的时候光标不挪）。
        Some("edit") => match &views.task.cursor {
            Cursor::Path(path) => Some(Purpose::EditPath(path.clone())),
            _ => None,
        },
        // 改的是配置视图光标停着的那一项（改着的时候光标不挪）。
        Some("value") => match views.config.cursor {
            Item::Setting(field) => Some(Purpose::Setting(field)),
            Item::Premise(_) => None,
        },
        Some("search") => Some(Purpose::Search),
        _ => None,
    };
    if let Some(purpose) = purpose {
        let buffer = input["buffer"].as_str().expect("输入行的缓冲").to_owned();
        let mut line = InputLine::new(purpose, buffer.clone());
        if let Some(candidates) = input["candidates"].as_array() {
            let listed: Vec<Completion> = candidates
                .iter()
                .filter_map(Value::as_str)
                .map(Completion::from_shown)
                .collect();
            let head = line.split().0.to_owned();
            line.offer(&head, listed);
            let at = input["candidate"].as_u64().unwrap_or(0) as usize;
            line.step(at as isize);
            // 设计稿的「添加路径」那一景把候选直接摆上去、缓冲没跟着换：照它的缓冲。
            line.buffer = buffer;
        }
        views.input = Some(line);
    }
    let overlay = &data.session["overlay"];
    if overlay["kind"].as_str() == Some("help") {
        views.cover = Some(Overlay::Keys {
            from: overlay["from"].as_u64().unwrap_or(0) as usize,
        });
    }
    views
}

/// 场景数据 `session.config` 那一段：在哪一栏、两栏各自的光标、下钻进了哪一块屏幕规格。
///
/// 预设栏那三格（`presets`、`preset_cursor`、`armed_delete`）随 `session-redesign/14` 认。
fn config_of(said: &Value, views: &mut Views) {
    views.config.focus = match said["pane"].as_str() {
        Some("right") => Focus::Details,
        _ => Focus::Settings,
    };
    if let Some(item) = said["cursor"].as_str().and_then(item_named) {
        views.config.cursor = item;
    }
    views.config.choice = said["choice_index"].as_u64().unwrap_or(0) as usize;
    // 下钻那一格导出的是**那一块屏幕规格的写法**（它自己的 `Display`），照它认回那一块。
    views.config.drill = said["drill"].as_str().and_then(|shown| {
        crate::session::config::panels()
            .into_iter()
            .find(|(panel, _)| panel.to_string() == shown)
            .map(|(panel, _)| panel)
    });
}

/// 设计稿给设置栏每一行起的名字认回一项（`design.html` 的 `CONFIG` 的 `key`）。
///
/// **这张对照表只在夹具这一侧**：那几个名字是设计稿自己的，实现那一头的名字取自
/// `CONTEXT.md`（[`Field`] 与 [`crate::render::Judging`]）。
fn item_named(key: &str) -> Option<Item> {
    let field = match key {
        "model" => Field::Profile,
        "levels" => Field::GrayLevels,
        "threshold" => Field::Threshold,
        "fit" => Field::Fit,
        "crop" => Field::Crop,
        "split" => Field::Split,
        "splitAt" => Field::SplitThreshold,
        "order" => Field::ReadingOrder,
        "filter" => Field::Filter,
        "white" => Field::WhiteAlignLimit,
        "depth" => Field::BitDepth,
        "dither" => Field::Dither,
        "envelope" => Field::Envelope,
        "cache" => Field::CacheBudget,
        "io" => Field::IoMode,
        premise => {
            let at = ["p1", "p2", "p3", "p4", "p5"]
                .iter()
                .position(|named| *named == premise)?;
            return Some(Item::Premise(render::Judging::ALL[at]));
        }
    };
    debug_assert!(
        DEVICE_FIELDS.contains(&field) || TASTE_FIELDS.contains(&field),
        "设计稿的 {key} 认成了配置视图外面的一项"
    );
    Some(Item::Setting(field))
}

/// 场景数据里**光标停在一条备注行上**、或者**说明卡掀着**的那两种，摆在树拼出来之后：
/// 认一条备注靠的是它的「是哪几处」（场景数据的 `cursor.what` 与那张卡自己的 `entries`），
/// 而树上那一条的身份是一条路径（[`super::tree::Note::at`]）——两头对得上要先有树。
///
/// 卡掀着的时候光标就停在那一条上（设计稿掀开它的那一下不挪光标），因此两样认同一条。
fn stand_on_a_note(session: &mut Session, data: &Data) {
    let wanted = data.session["cursor"]["what"].as_str();
    let lifted = data.session["overlay"]["kind"].as_str() == Some("note");
    if data.session["cursor"]["kind"].as_str() != Some("note") {
        assert!(!lifted, "掀着说明卡而光标不在那一条备注上");
        return;
    }
    let what = wanted.expect("光标停在备注行上时记着它是哪几处");
    let tree = &session.views.task.tree;
    let (node, at) = tree
        .locate(|note| note.what == what)
        .unwrap_or_else(|| panic!("树上没有「{what}」那一条备注"));
    session.views.task.cursor =
        Cursor::Note(tree.note(node, at).expect("刚找到的那一条").at.clone());
    if lifted {
        session.views.lift_note(node, at);
    }
}

/// `~/` 换成家目录。
fn expand(home: &Path, tilde: &str) -> PathBuf {
    match tilde.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if tilde == "~" => home.to_path_buf(),
        None => PathBuf::from(tilde),
    }
}

/// 场景数据里这一趟已用了多久。
fn elapsed(run: Option<&Run>) -> Duration {
    run.map_or(Duration::ZERO, |run| Duration::from_secs_f64(run.elapsed_s))
}

// ───────────────────────── 假盘 ─────────────────────────

/// 盘上一处是什么形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Directory,
    File,
}

/// **假盘上有什么**：11 个场景的场景数据提到的每一处的并集，`~/` 写法。
///
/// 设计稿的假盘本身没有导出（停车场 Q764），能从场景数据认出来的是：处理路径（文件夹还是压缩包）、
/// 清点清单上的分区、目录与卷根、备注里的路径（无法访问的地方是目录，非漫画文件是文件）、
/// 输出目录，以及输入行补全框里列出的候选。一趟只算一次，每个场景各自建一棵。
fn disk() -> &'static [(String, Shape)] {
    static DISK: OnceLock<Vec<(String, Shape)>> = OnceLock::new();
    DISK.get_or_init(|| {
        let mut places: BTreeMap<String, Shape> = BTreeMap::new();
        let mut put = |path: &str, shape: Shape| {
            if path.starts_with('~') {
                places.entry(path.to_owned()).or_insert(shape);
            }
        };
        for scene in scenes() {
            let data = scene_data(&scene);
            put(&data.output, Shape::Directory);
            for named in &data.paths {
                put(&named.path, shape_of_named(&named.kind));
            }
            if let Some(run) = &data.run {
                for entry in &run.survey.entries {
                    match entry {
                        Entry::Section {
                            path, directories, ..
                        } => {
                            put(path, Shape::Directory);
                            for directory in directories {
                                put(directory, Shape::Directory);
                            }
                        }
                        Entry::Directory { root, .. } => put(root, Shape::Directory),
                    }
                    for note in entry.notes() {
                        let shape = match note.kind {
                            NoteKind::Unreachable => Shape::Directory,
                            NoteKind::NonVolume => Shape::File,
                        };
                        for said in &note.entries {
                            put(&said.path, shape);
                        }
                    }
                }
                for listed in &run.survey.volumes {
                    put(&listed.directory, Shape::Directory);
                    put(&listed.root, shape_of_root(&listed.root));
                }
            }
            // 输入行里补全到一半的路径与它列出的候选。
            let input = &data.session["input"];
            if let (Some(buffer), Some(candidates)) =
                (input["buffer"].as_str(), input["candidates"].as_array())
            {
                for candidate in candidates.iter().filter_map(Value::as_str) {
                    let path = format!("{buffer}{candidate}");
                    match path.strip_suffix('/') {
                        Some(directory) => put(directory, Shape::Directory),
                        None => put(&path, Shape::File),
                    }
                }
            }
        }
        places.into_iter().collect()
    })
}

/// 一条处理路径是目录还是文件：按它说的种类。
fn shape_of_named(kind: &str) -> Shape {
    match kind {
        "directory" => Shape::Directory,
        "archive" => Shape::File,
        other => panic!("不认识的处理路径种类：{other}"),
    }
}

/// 一个卷根是目录还是文件：归档卷带着扩展名（`第01卷.cbz`），目录卷没有。
fn shape_of_root(root: &str) -> Shape {
    if Path::new(root).extension().is_some() {
        Shape::File
    } else {
        Shape::Directory
    }
}

/// 照[假盘](disk)在 `home` 底下建出那棵树。文件都是空的：夹具不碰盘上的内容，只问形状。
fn build_disk(home: &Path) {
    for (place, shape) in disk() {
        let at = expand(home, place);
        match shape {
            Shape::Directory => std::fs::create_dir_all(&at),
            Shape::File => std::fs::create_dir_all(at.parent().expect("文件有上一层"))
                .and_then(|()| std::fs::write(&at, b"")),
        }
        .unwrap_or_else(|error| panic!("建不出 {}：{error}", at.display()));
    }
}

// ───────────────────────── 三组设置与预设 ─────────────────────────

/// 场景数据里的一组设置（平的一张表，kebab-case）写成预设文件里的一份。
///
/// 走的是盘上那份 TOML 的形状（`crate::preset::OnDisk`）：`null` 是「没说」，整项不写；
/// 设备设置那三个键进 `device`，其余进 `taste`。读回来的那一步交给 [`preset::read`]——
/// 取值怎么解析、越界怎么报错，与真读预设文件同一处。
fn preset_table(says: &BTreeMap<String, Value>) -> toml::Table {
    let mut device = toml::Table::new();
    let mut taste = toml::Table::new();
    for (key, value) in says {
        if value.is_null() {
            continue;
        }
        let value = toml::Value::try_from(value)
            .unwrap_or_else(|error| panic!("设置「{key}」写不成 TOML：{error}"));
        if DEVICE_KEYS.contains(&key.as_str()) {
            device.insert(key.clone(), value);
        } else {
            taste.insert(key.clone(), value);
        }
    }
    let mut one = toml::Table::new();
    one.insert("device".to_owned(), toml::Value::Table(device));
    one.insert("taste".to_owned(), toml::Value::Table(taste));
    one
}

/// 几份预设写成一整份预设文件的文字。
fn presets_toml(presets: &[(&str, &BTreeMap<String, Value>)]) -> String {
    let mut all = toml::Table::new();
    for (name, says) in presets {
        all.insert((*name).to_owned(), toml::Value::Table(preset_table(says)));
    }
    let mut file = toml::Table::new();
    file.insert("preset".to_owned(), toml::Value::Table(all));
    toml::to_string(&file).expect("预设写得成 TOML")
}

/// 场景数据里的一组设置读成一份 [`Preset`]。
fn preset_of(says: &BTreeMap<String, Value>) -> Preset {
    const NAME: &str = "此刻";
    preset::read(&presets_toml(&[(NAME, says)]), NAME).expect("场景数据的设置读得成预设")
}

/// 预设文件写进临时目录，交出指着它的 [`Presets`]。
fn write_presets(space: &Path, presets: &[PresetData]) -> Presets {
    let path = space.join("config").join("tonefit").join("presets.toml");
    std::fs::create_dir_all(path.parent().expect("预设文件有上一层")).expect("建得出配置目录");
    let listed: Vec<(&str, &BTreeMap<String, Value>)> = presets
        .iter()
        .map(|said| (said.name.as_str(), &said.says))
        .collect();
    std::fs::write(&path, presets_toml(&listed)).expect("写得出预设文件");
    Presets::at(path)
}

// ───────────────────────── 那一趟 ─────────────────────────

/// 沿真跑那条路把场景数据里的那一趟喂进 [`Live`]，会话的阶段跟着走。
fn replay(run: &Run, home: &Path, output: &str, epoch: Instant, session: &mut Session) -> Live {
    // 起手按的是哪个键：预览走的也是 `Mode::Process`（参照要留着），只是在确认点上等人
    // （`super::terminal::resuming`）。
    let resumes = match run.mode.as_str() {
        "process" => Resuming::GoesOn,
        "dry-run" => Resuming::Waits,
        other => panic!("不认识的模式：{other}"),
    };
    let request = session
        .request(RunMode::Process)
        .expect("场景数据的三组设置拼得出一份 Request");
    let mut live = fixture::live_for(epoch, &request, resumes);
    session.run_started();
    let now = epoch + Duration::from_secs_f64(run.elapsed_s);

    // 清点中：线程起了，开工那一条还没到——清单还是空的（`CONTEXT.md` 的《总览》：不报卷数）。
    if run.stage == "surveying" {
        press_stop(session, run);
        live.tick(now);
        return live;
    }

    let roster: Vec<SurveyedVolume> = run
        .survey
        .volumes
        .iter()
        .map(|listed| SurveyedVolume {
            root: expand(home, &listed.root),
            steps: listed.steps,
            source_pages: listed.source_pages,
        })
        .collect();
    let (non_volume_files, unreachable_places) = tables(run, home);
    live.run_started(roster.len(), run.total_steps);
    live.surveyed(&roster, &non_volume_files, &unreachable_places);

    let out = expand(home, output);
    let disk = Disk { home, out: &out };
    let typical = typical_size();
    // **没有报告的那几卷，耗时只有会话这一头记得住**（`Live::elapsed_at`）：把「此刻」
    // 推到它开卷与收手那两刻，量出来的就是场景数据说的那个数。收摊了的卷不必——
    // 它们那个数在自己那份报告的 `VolumeTiming` 上。
    let mut clock = epoch;
    for (listed, volume) in run.survey.volumes.iter().zip(&run.volumes) {
        assert_eq!(listed.root, volume.root, "每卷状态那一列与清单同序");
        let root = expand(home, &listed.root);
        let pages = listed.source_pages;
        let step = |live: &mut Live, times: usize| {
            for _ in 0..times {
                live.stepped();
            }
        };
        match volume.state.as_str() {
            "queued" => {}
            "skipped" => {
                live.volume_started(&root, listed.steps);
                live.pass_started(Pass::Fingerprint, None);
                step(&mut live, pages);
                live.volume_finished(&skipped_report(listed, volume, disk, run));
            }
            "failed" => {
                live.tick(clock);
                live.volume_started(&root, listed.steps);
                live.pass_started(Pass::Fingerprint, None);
                step(&mut live, pages);
                clock += Duration::from_secs_f64(volume.elapsed_s);
                live.tick(clock);
                live.volume_failed(
                    &root,
                    volume.failure.as_deref().expect("没做成的卷带着原因"),
                );
            }
            "done" | "isolated" | "trialed" => {
                let report = volume_report(listed, volume, disk, run, typical);
                live.volume_started(&root, listed.steps);
                // 前两遍走满；写出那一遍在确认点上答的字：答了「不写出」的那一卷（`trialed`）
                // 写出环节一步不走，紧跟着收摊（`tonefit::Pass::Second` 的文档）。
                let said = if volume.state == "trialed" {
                    Instruction::Finish
                } else {
                    Instruction::Continue
                };
                for pass in PASSES {
                    begin_pass(&mut live, pass, resumes, run, Some(&report), said);
                    if pass != Pass::Second || said == Instruction::Continue {
                        step(&mut live, pages);
                    }
                }
                fixture::volume_finished_with_its_failures(&mut live, &report);
            }
            "running" | "deciding" | "aborted" => {
                // 还没收摊的那一卷：把「此刻」退回它开卷那一刻，末尾那一次 `tick` 因此
                // 正好量出它做了多久（等待确认那一卷有攒着的那一份报告，不必退）。
                if volume.state != "deciding" {
                    clock = epoch
                        + Duration::from_secs_f64((run.elapsed_s - volume.elapsed_s).max(0.0));
                    live.tick(clock);
                }
                // 分析环节走完的卷才有到此刻为止的报告（灰阶分布）；还在前两遍上的卷没有。
                let so_far = volume
                    .tally
                    .is_some()
                    .then(|| volume_report(listed, volume, disk, run, typical));
                live.volume_started(&root, listed.steps);
                let pass = usize::try_from(volume.pass).expect("开了卷的卷走到了某一遍");
                // 走完了的那几遍走满，正在走的这一遍走到第几页；坏页在分析环节里当场报
                // （`Event::PageFailed`：出现的当场一条，收摊时报告里再一次）。
                for (at, walking) in PASSES.iter().enumerate().take(pass + 1) {
                    begin_pass(
                        &mut live,
                        *walking,
                        resumes,
                        run,
                        so_far.as_ref(),
                        Instruction::Continue,
                    );
                    step(
                        &mut live,
                        if at < pass {
                            pages
                        } else {
                            volume.done.floor() as usize
                        },
                    );
                    if *walking == Pass::First
                        && let Some(so_far) = &so_far
                    {
                        for page in so_far.failures() {
                            if let PageOutcome::Failed { reason } = &page.outcome {
                                live.page_failed(&page.source, reason);
                            }
                        }
                    }
                }
                if volume.state == "deciding" {
                    // 等待确认：分析环节走完，停在写出那一遍的确认点上。「此刻」要在确认点那一条
                    // **之前**给——等人那一截从那一条起算，给在它之后会把跑过的那一段减光。
                    assert_eq!(
                        (pass, volume.done as usize),
                        (1, pages),
                        "等待确认的卷停在分析环节走完之后"
                    );
                    live.tick(now);
                    second_pass(&mut live, resumes, run, so_far.as_ref(), None);
                }
            }
            other => panic!("不认识的卷状态：{other}"),
        }
    }

    match run.stage.as_str() {
        "running" => {
            press_stop(session, run);
            live.tick(now);
        }
        "deciding" => {
            assert_eq!(run.stop_level, 0, "夹具还不认得在确认点上按过停止的那一趟");
            session.at_the_decision_point(true);
        }
        "ended" => {
            // 按停止按到哪一级不再按：结束之后会话的阶段上没有那一格，结局带着它。
            live.tick(now);
            live.run_finished(outcome_of(run));
            // 那条线程回来了：库交出来的那一份与攒着的只差计时与两张表。
            let mut report = live.report().clone();
            report.elapsed = Duration::from_secs_f64(run.elapsed_s);
            report.non_volume_files = non_volume_files;
            report.unreachable_places = unreachable_places;
            live.returned(Ok(report));
            session.run_finished();
        }
        other => panic!("不认识的阶段：{other}"),
    }
    live
}

/// 结束了的那一趟是怎么收的场：`done` · `stopped`（做完再停）· `aborted`（立即停止）。
fn outcome_of(run: &Run) -> RunOutcome {
    match run.outcome.as_deref() {
        Some("done") => RunOutcome::Completed,
        Some("stopped") => RunOutcome::Stopped(Instruction::Finish),
        Some("aborted") => RunOutcome::Stopped(Instruction::Abort),
        other => panic!("结束了的那一趟结局认不出：{other:?}"),
    }
}

/// 某一遍开工：写出那一遍走确认点那一支（[`second_pass`]，照 `said` 答话），另两遍直接开。
fn begin_pass(
    live: &mut Live,
    pass: Pass,
    resumes: Resuming,
    run: &Run,
    so_far: Option<&VolumeReport>,
    said: Instruction,
) {
    match pass {
        Pass::Second => second_pass(live, resumes, run, so_far, Some(said)),
        other => live.pass_started(other, None),
    }
}

/// 写出那一遍开工，带着这一卷到此刻为止的报告（会话的观察者读它，几种趟都一样——
/// `Progress::reads_the_report_at_the_decision_point` 默认读）。等人的那一趟停在确认点上，
/// 再照 `said` 答话：`None` 是还停着（等待确认那一景）。答过「后面的卷都写出」之后不再问，
/// 走到这一遍就是处理中。
///
/// 场景数据只说答过「后面的卷都写出」没有，没说在哪一卷上答的：按 `a` 的那一次是
/// **头一个**停下来问的确认点——两串带它的序列都是这么走的。
fn second_pass(
    live: &mut Live,
    resumes: Resuming,
    run: &Run,
    so_far: Option<&VolumeReport>,
    said: Option<Instruction>,
) {
    let so_far = so_far.expect("走到写出那一遍的卷带着到此刻为止的报告（分析环节走完了）");
    let asked = resumes == Resuming::Waits && live.for_the_rest().is_none();
    live.pass_started(Pass::Second, Some(so_far));
    if !asked {
        return;
    }
    if let Some(said) = said {
        let reach = if run.for_the_rest {
            Reach::ForTheRest
        } else {
            Reach::ThisVolume
        };
        live.decide(said, reach);
    }
}

/// 按停止按到场景数据说的那一级。`Live` 不记它——记的是会话的阶段（`Session::stopping`），
/// 真会话里那个字由 `Running::stop` 交给线程，而夹具没有线程。
fn press_stop(session: &mut Session, run: &Run) {
    for _ in 0..run.stop_level {
        session.press(Key::Char('s'));
    }
}

/// 开工那一条带的两张表：非漫画文件与无法访问的地方，从备注里来。
///
/// 无法访问的地方那句原因**原样带**（场景数据里就是渲染后的整句）；非漫画文件的原因得反查回
/// [`NonVolumeReason`] 的哪一类——措辞只在 [`render::non_volume_reason`] 一处，这里拿三类各说一遍去比。
fn tables(run: &Run, home: &Path) -> (Vec<NonVolumeFile>, Vec<UnreachablePlace>) {
    let mut non_volume_files = Vec::new();
    let mut unreachable_places = Vec::new();
    for entry in &run.survey.entries {
        for note in entry.notes() {
            for said in &note.entries {
                match note.kind {
                    NoteKind::Unreachable => unreachable_places.push(UnreachablePlace {
                        path: expand(home, &said.path),
                        reason: said.reason.clone(),
                    }),
                    NoteKind::NonVolume => non_volume_files.push(NonVolumeFile {
                        path: expand(home, &said.path),
                        reason: non_volume_reason_of(&said.reason),
                    }),
                }
            }
        }
    }
    (non_volume_files, unreachable_places)
}

/// 渲染后的那一句反查回它是哪一类非漫画文件（正向的那一处是 [`render::non_volume_reason`]）。
fn non_volume_reason_of(sentence: &str) -> NonVolumeReason {
    for reason in [
        NonVolumeReason::NeitherPageNorArchive,
        NonVolumeReason::ArchiveWithoutAPage,
    ] {
        if render::non_volume_reason(&reason) == sentence {
            return reason;
        }
    }
    // 点不开的那一类带着错误链：拿一个记号占住那一格，再从句子两头把记号前后那两截剥掉。
    const MARK: &str = "\u{0}";
    let template = render::non_volume_reason(&NonVolumeReason::Unopenable(MARK.to_owned()));
    let (head, tail) = template.split_once(MARK).expect("模板里有那个记号");
    match sentence
        .strip_prefix(head)
        .and_then(|rest| rest.strip_suffix(tail))
    {
        Some(chain) => NonVolumeReason::Unopenable(chain.to_owned()),
        None => panic!("这一句不是三类非漫画文件里的任何一类：{sentence}"),
    }
}

/// **普通一页的输出尺寸**：11 个场景的场景数据里头一张不超宽的页说的那个。
/// 没开着的卷补的页都按它（与[假盘](disk)同一个道理：一趟只算一次，取自全部场景的并集——
/// 单看一个场景，等待确认那一景只有一张超宽的页可查）。
fn typical_size() -> Size {
    static TYPICAL: OnceLock<Size> = OnceLock::new();
    *TYPICAL.get_or_init(|| {
        scenes()
            .iter()
            .map(|scene| scene_data(scene))
            .filter_map(|data| data.run)
            .flat_map(|run| run.volumes)
            .flat_map(|volume| {
                volume
                    .notable_pages
                    .into_iter()
                    .chain(volume.pages)
                    .flatten()
            })
            .find(|page| !page.kinds.iter().any(|kind| kind == "wide"))
            .map(|page| Size::new(page.output_size.0, page.output_size.1))
            .expect("场景数据里有一页说得出普通页的输出尺寸")
    })
}

/// 这一卷干净的去处：输出目录接上卷根在它那条分区（或顶格目录的上一层）之下的那一截——
/// 与场景数据给出的隔离去处同一个写法（设计稿的假数据；库自己的镜像规则见停车场 Q765）。
fn output_of(listed: &Listed, out: &Path, run: &Run) -> PathBuf {
    let base = run
        .survey
        .entries
        .iter()
        .find_map(|entry| match entry {
            Entry::Section {
                path, directories, ..
            } if directories.contains(&listed.directory) => Some(path.as_str()),
            Entry::Directory { root, .. } if *root == listed.directory => {
                Some(root.rsplit_once('/').map_or("", |(parent, _)| parent))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("清单上找不到 {} 所在的那一项", listed.root));
    let inside = listed
        .root
        .strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or_else(|| panic!("{} 不在 {base} 底下", listed.root));
    out.join(inside)
}

/// 家目录与输出目录，造报告时一起传。
#[derive(Clone, Copy)]
struct Disk<'a> {
    home: &'a Path,
    out: &'a Path,
}

/// 一份**幂等命中**的卷报告。
fn skipped_report(listed: &Listed, volume: &VolumeData, disk: Disk<'_>, run: &Run) -> VolumeReport {
    VolumeReport {
        volume: expand(disk.home, &listed.root),
        output: output_of(listed, disk.out, run),
        superseded: None,
        pages: Vec::new(),
        retained_pages: 0,
        source_pages: listed.source_pages,
        verdict: Some(VolumeVerdict::Skipped {
            page_count: listed.source_pages,
        }),
        cache: fixture::cache_usage(),
        extracted: 0,
        io: fixture::io_plan(),
        decodes: 0,
        resizes: 0,
        cached_references: 0,
        timing: fixture::took(Duration::from_secs_f64(volume.elapsed_s)),
    }
}

/// 一份**做过事**的卷报告：逐页照场景数据（开着的那一卷整份，其余照分布补），
/// 卷级判定照这一趟开没开整卷统一灰阶。
fn volume_report(
    listed: &Listed,
    volume: &VolumeData,
    disk: Disk<'_>,
    run: &Run,
    typical: Size,
) -> VolumeReport {
    let Disk { home, out } = disk;
    let root = expand(home, &listed.root);
    // 去处：进了隔离的卷场景数据直接给（`~/` 写法），其余照同一个写法接出来。
    let output = match &volume.isolated_output {
        Some(isolated) => expand(home, isolated),
        None => output_of(listed, out, run),
    };
    let pages_data = pages_of(volume, run.envelope, typical);
    let pages: Vec<PageReport> = pages_data
        .iter()
        .map(|page| page_report(page, &root, &output, typical))
        .collect();
    let verdict = if run.envelope {
        let driver = pages_data
            .iter()
            .position(|page| page.driver)
            .unwrap_or_else(|| panic!("{} 开着整卷统一灰阶却没有代表页", listed.root));
        let base = candidate_of(pages_data[driver].verdict.as_deref().expect("代表页有判定"));
        let reasons = |reason: Reason| {
            pages
                .iter()
                .filter(|page| page.verdict().map(|verdict| verdict.reason) == Some(reason))
                .count()
        };
        Some(VolumeVerdict::Envelope(Envelope {
            base,
            driver,
            body_pages: reasons(Reason::VolumeEnvelope),
            outlier_pages: reasons(Reason::Outlier),
            raised_pages: 0,
        }))
    } else {
        Some(VolumeVerdict::PerPage)
    };
    let decoded = pages.len();
    VolumeReport {
        volume: root,
        output,
        superseded: None,
        pages,
        retained_pages: 0,
        source_pages: listed.source_pages,
        verdict,
        cache: fixture::cache_usage(),
        extracted: 0,
        io: fixture::io_plan(),
        decodes: decoded,
        resizes: decoded,
        cached_references: decoded,
        timing: fixture::took(Duration::from_secs_f64(volume.elapsed_s)),
    }
}

/// 这一卷逐页的数据：开着的那一卷整份照抄；其余各卷**照灰阶分布补页**（Q736）——
/// 需留意的那几页按名字里的序号落位，剩下的位置按分布的次序填满。补的页没有画质分与源页高
/// 可说（屏上从它们身上读得到的只有灰阶分布），画质分填零、源页高取输出高。
fn pages_of(volume: &VolumeData, envelope: bool, typical: Size) -> Vec<PageData> {
    if let Some(pages) = &volume.pages {
        return pages.clone();
    }
    let count = volume.page_count.expect("做过事的卷有页数");
    let notable = volume.notable_pages.as_deref().unwrap_or(&[]);
    let mut slots: Vec<Option<PageData>> = vec![None; count];
    for page in notable {
        let at = index_of(&page.name);
        assert!(
            at < count && slots[at].is_none(),
            "需留意的那一页 {} 落不进 {} 的第 {} 格",
            page.name,
            volume.root,
            at + 1
        );
        slots[at] = Some(page.clone());
    }
    // 分布里扣掉需留意的那几页已经占掉的，剩下的按分布的次序排成一列。
    let mut remaining: Vec<(String, usize)> = volume.tally.clone().unwrap_or_default();
    for page in notable.iter().filter(|page| page.failure.is_none()) {
        let verdict = page.verdict.as_deref().expect("没坏的页有判定");
        let slot = remaining
            .iter_mut()
            .find(|(candidate, _)| candidate == verdict)
            .unwrap_or_else(|| panic!("{} 的分布里没有 {verdict}", volume.root));
        slot.1 = slot.1.checked_sub(1).expect("分布里数得下需留意的那几页");
    }
    let mut pool = remaining
        .iter()
        .flat_map(|(candidate, count)| std::iter::repeat_n(candidate.clone(), *count));
    let extension = notable
        .first()
        .and_then(|page| page.name.rsplit_once('.'))
        .map_or("jpg", |(_, extension)| extension);
    let reason = if envelope {
        "volume-envelope"
    } else {
        "lowest-within-threshold"
    };
    let pages: Vec<PageData> = slots
        .into_iter()
        .enumerate()
        .map(|(at, slot)| {
            slot.unwrap_or_else(|| PageData {
                name: format!("{:03}.{extension}", at + 1),
                output_size: (typical.width, typical.height),
                kinds: Vec::new(),
                source_height: Some(typical.height),
                verdict: Some(
                    pool.next()
                        .unwrap_or_else(|| panic!("{} 的分布比页数少", volume.root)),
                ),
                reason: Some(reason.to_owned()),
                score: Some(0.0),
                failure: None,
                salvaged_percent: None,
                driver: false,
            })
        })
        .collect();
    assert!(pool.next().is_none(), "{} 的分布比页数多", volume.root);
    pages
}

/// 页名里的序号落在第几格：`062.jpg` 是第 62 页。
fn index_of(name: &str) -> usize {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
        .unwrap_or_else(|| panic!("页名 {name} 不以序号开头"))
}

/// 一页的报告。
fn page_report(page: &PageData, root: &Path, output: &Path, typical: Size) -> PageReport {
    let source = root.join(&page.name);
    let written = output.join(Path::new(&page.name).with_extension("png"));
    let size = Size::new(page.output_size.0, page.output_size.1);
    if let Some(reason) = &page.failure {
        return PageReport {
            source,
            output: written,
            size,
            outcome: PageOutcome::Failed {
                reason: reason.clone(),
            },
        };
    }
    let candidate = candidate_of(page.verdict.as_deref().expect("没坏的页有判定"));
    let reason = reason_of(page.reason.as_deref().expect("没坏的页有理由"));
    let source_height = page.source_height.unwrap_or(typical.height);
    // 源页宽只有比例可循：缩放那一格只读高（`Scaling::plan`），宽按同一比例放大就够。
    let ratio = f64::from(source_height) / f64::from(size.height);
    let source_size = Size::new(
        (f64::from(size.width) * ratio).round() as u32,
        source_height,
    );
    let processed = Processed {
        crop: Crop::keeping_all(source_size),
        backstopped: false,
        cut: None,
        spread_candidate: false,
        scaling: Scaling::plan(source_size, size),
        color: PageColor::Gray,
        branch: PageBranch::Gray {
            white: WhiteAlignment::Off,
            gate: if reason == Reason::OutsideTheGate {
                GeometryGate::Broken
            } else {
                GeometryGate::Holds
            },
            scores: vec![CandidateScore {
                candidate,
                score: Score::from_value(page.score.expect("没坏的页有画质分")),
            }],
            verdict: Verdict { candidate, reason },
        },
    };
    let outcome = match page.salvaged_percent {
        Some(percent) => PageOutcome::Salvaged {
            page: processed,
            salvage: Salvage::from_share(percent / 100.0),
        },
        None => PageOutcome::Whole(processed),
    };
    PageReport {
        source,
        output: written,
        size,
        outcome,
    }
}

/// 灰阶那一格的字（`2bit+FS`）认回候选：拿每个候选的 `Display` 去比，写法只有库那一处。
fn candidate_of(text: &str) -> Candidate {
    BitDepth::ALL
        .into_iter()
        .flat_map(|depth| {
            [Dither::Off, Dither::FloydSteinberg]
                .into_iter()
                .map(move |dither| Candidate::new(depth, dither))
        })
        .find(|candidate| candidate.to_string() == text)
        .unwrap_or_else(|| panic!("认不出的候选：{text}"))
}

/// 理由的种类名（导出脚本的 `REASONS`）认回 [`Reason`]。
fn reason_of(kind: &str) -> Reason {
    match kind {
        "lowest-within-threshold" => Reason::LowestWithinThreshold,
        "volume-envelope" => Reason::VolumeEnvelope,
        "outlier" => Reason::Outlier,
        "outside-the-gate" => Reason::OutsideTheGate,
        other => panic!("认不出的理由种类：{other}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::session::live::VolumeState;

    /// 一个卷根（`~/` 写法）在场景数据里对应的清单项与状态。
    fn volume_of<'a>(run: &'a Run, root: &str) -> (&'a Listed, &'a VolumeData) {
        let at = run
            .survey
            .volumes
            .iter()
            .position(|listed| listed.root == root)
            .unwrap_or_else(|| panic!("清单上没有 {root}"));
        (&run.survey.volumes[at], &run.volumes[at])
    }

    /// 场景数据里一卷此刻怎么样，翻成 [`VolumeState`]：七种各一档，外加设计稿多出来的
    /// `trialed`（确认点上答了「不写出」的那一卷——库那一侧它收摊成完成，只是没写）。
    fn state_of(volume: &VolumeData) -> VolumeState {
        match volume.state.as_str() {
            "queued" => VolumeState::Queued,
            "running" => VolumeState::Running {
                pass: Some(PASSES[volume.pass as usize]),
            },
            "deciding" => VolumeState::Deciding,
            "done" | "trialed" => VolumeState::Done,
            "isolated" => VolumeState::Isolated,
            "skipped" => VolumeState::Skipped,
            "failed" => VolumeState::Failed,
            "aborted" => VolumeState::Aborted,
            other => panic!("不认识的卷状态：{other}"),
        }
    }

    /// 场景数据里一卷的页各需留意在哪几处，数成一张表（`failed` · `outlier` · `wide` · `gate` ·
    /// `salvage` · `driver` 各几页）。
    fn kinds_of(volume: &VolumeData) -> BTreeMap<&str, usize> {
        let mut counted = BTreeMap::new();
        for page in volume
            .pages
            .iter()
            .chain(volume.notable_pages.iter())
            .flatten()
        {
            for kind in &page.kinds {
                *counted.entry(kind.as_str()).or_insert(0) += 1;
            }
        }
        counted
    }

    /// 库那一头一卷的页各需留意在哪几处，数成同一张表（[`render::notable`]）。
    fn notable_of(report: &VolumeReport, panel: tonefit::Panel) -> BTreeMap<&'static str, usize> {
        let mut counted = BTreeMap::new();
        for why in render::notable(report, panel).into_iter().flatten() {
            let kind = match why {
                render::Notable::Failed => "failed",
                render::Notable::Salvaged => "salvage",
                render::Notable::Outlier => "outlier",
                render::Notable::OutsideTheGate => "gate",
                render::Notable::Overflowed => "wide",
                render::Notable::Backstopped => "backstopped",
                render::Notable::Driver => "driver",
            };
            *counted.entry(kind).or_insert(0) += 1;
        }
        counted
    }

    /// 一份卷报告的灰阶分布：各档几页（[`render::tally_row`] 数的正是这个）。
    fn tally_of(report: &VolumeReport) -> BTreeMap<String, usize> {
        let mut counted = BTreeMap::new();
        for verdict in report.pages.iter().filter_map(PageReport::verdict) {
            *counted.entry(verdict.candidate.to_string()).or_insert(0) += 1;
        }
        counted
    }

    /// 一份预设说到的那几项，写成盘上那一份的样子再读回一张平的表：与场景数据同一副形状，
    /// 转换只有 [`preset::write`] 一处。
    fn said_by(preset: &Preset) -> BTreeMap<String, toml::Value> {
        let text = preset::write(&BTreeMap::from([("它".to_owned(), preset.clone())]))
            .expect("预设写得成 TOML");
        let file: toml::Table = toml::from_str(&text).expect("写出来的是 TOML");
        let mut flat = BTreeMap::new();
        for layer in ["device", "taste"] {
            if let Some(table) = file["preset"]["它"][layer].as_table() {
                for (key, value) in table {
                    flat.insert(key.clone(), value.clone());
                }
            }
        }
        flat
    }

    /// 场景数据里说到的那几项（`null` 是没说），同一副形状。
    fn said_in(says: &BTreeMap<String, Value>) -> BTreeMap<String, toml::Value> {
        says.iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(key, value)| {
                (
                    key.clone(),
                    toml::Value::try_from(value).expect("场景数据的取值写得成 TOML"),
                )
            })
            .collect()
    }

    /// **摆出来的那一景与它的场景数据逐项相同**（票面第二、四条）：那一趟（总进度、当前卷与环节、
    /// 每卷状态、问题计数、灰阶分布、需留意的页、写出过没有、答话与停止、结局）、三组设置与预设、
    /// 假盘上那棵树，以及夹具没碰用户配置目录。不经画法。
    fn agrees_with_its_data(scene: &Scene) {
        let data = &scene.data;
        let name = &scene.label;
        assert!(name.starts_with(&data.scene), "{name} 起自 {}", data.scene);

        // 三组设置：设备设置与处理选项照场景数据说到的那几项，路径与输出照处理路径与勾选。
        assert_eq!(
            said_by(&scene.session.preset()),
            said_in(&data.settings),
            "{name}"
        );
        assert_eq!(
            scene.session.scope.out,
            Some(scene.path(&data.output)),
            "{name}"
        );
        assert_eq!(
            scene.session.scope.paths,
            data.paths
                .iter()
                .map(|named| NamedPath {
                    path: scene.path(&named.path),
                    on: named.checked,
                })
                .collect::<Vec<_>>(),
            "{name}"
        );

        // 预设文件：那几份都在，各说各的那几项；文件在临时目录里、不在家目录底下。
        let file = scene.presets.path().expect("预设文件的位置是定死的");
        assert!(!file.starts_with(&scene.home), "{name}：预设文件不在假盘上");
        assert!(
            file.starts_with(scene.space.path()),
            "{name}：预设文件在临时目录里"
        );
        let mut names = scene.presets.names().expect("预设文件读得出名字");
        names.sort();
        let mut expected: Vec<String> = data.presets.iter().map(|said| said.name.clone()).collect();
        expected.sort();
        assert_eq!(names, expected, "{name}");
        if let Some(applied) = &data.applied_preset {
            assert!(
                names.contains(applied),
                "{name}：套的那一份「{applied}」在文件里"
            );
        }
        for said in &data.presets {
            let preset = scene.presets.read(&said.name).expect("每一份都读得出");
            assert_eq!(
                said_by(&preset),
                said_in(&said.says),
                "{name}：预设「{}」",
                said.name
            );
        }

        // 假盘：处理路径各是它说的那种形状，清单上的每一处都在，输出目录在。
        for named in &data.paths {
            let at = scene.path(&named.path);
            assert!(at.exists(), "{name}：{} 不在盘上", named.path);
            assert_eq!(
                at.is_dir(),
                named.kind == "directory",
                "{name}：{}",
                named.path
            );
        }
        assert!(scene.path(&data.output).is_dir(), "{name}：输出目录在盘上");

        let Some(run) = &data.run else {
            assert!(scene.live.is_none(), "{name}：还没开跑就没有那一趟");
            assert_eq!(
                scene.session.stage(),
                super::super::state::Stage::Fresh,
                "{name}"
            );
            return;
        };
        let live = scene.live();
        let panel = live.report().profile.panel();

        // 清点中：清单还是空的（开工那一条还没到）。
        if run.stage == "surveying" {
            assert!(live.roster().is_empty(), "{name}：清点中清单还没到");
            assert!(!live.ended(), "{name}");
            assert_eq!(live.overall().walked, 0, "{name}");
            return;
        }

        // 清单。
        assert_eq!(
            run.survey.unchecked,
            data.paths.iter().filter(|named| !named.checked).count(),
            "{name}"
        );
        for (listed, surveyed) in run.survey.volumes.iter().zip(live.roster()) {
            assert_eq!(surveyed.root, scene.path(&listed.root), "{name}");
            assert_eq!(
                Some(listed.name.as_str()),
                Path::new(&listed.root)
                    .file_stem()
                    .and_then(|stem| stem.to_str()),
                "{name}：清单上的卷名是卷根的末一级"
            );
            assert_eq!(surveyed.steps, listed.steps, "{name}：{}", listed.root);
            assert_eq!(
                surveyed.source_pages, listed.source_pages,
                "{name}：{}",
                listed.root
            );
            assert!(surveyed.root.exists(), "{name}：{} 不在盘上", listed.root);
        }
        assert_eq!(live.roster().len(), run.survey.volumes.len(), "{name}");

        // 总进度。
        let overall = live.overall();
        assert_eq!(overall.steps, run.total_steps, "{name}");
        assert_eq!(overall.walked, run.steps.floor() as u64, "{name}");
        assert_eq!(overall.volumes, run.survey.volumes.len(), "{name}");
        assert_eq!(
            overall.elapsed,
            Duration::from_secs_f64(run.elapsed_s),
            "{name}"
        );
        assert_eq!(
            scene.now() - scene.epoch,
            overall.elapsed,
            "{name}：此刻就是开工加已用"
        );

        // 当前卷与环节。结束了就没有当前卷（被立即停止掉的那一卷也已经关上）。
        if run.stage == "ended" {
            assert!(live.walking().is_none(), "{name}");
        } else {
            let current = run.current.as_deref().expect("跑着与等待确认时有当前卷");
            let walking = live.walking().expect("当前卷在走");
            assert_eq!(walking.volume, scene.path(current), "{name}");
            let (listed, state) = volume_of(run, current);
            assert_eq!(walking.steps, listed.steps, "{name}");
            let pass = usize::try_from(state.pass).expect("当前卷走到了某一遍");
            let walked = pass as u64 * listed.source_pages as u64 + state.done.floor() as u64;
            assert_eq!(walking.walked, walked, "{name}");
            if state.state == "deciding" {
                assert_eq!(
                    walking.pass,
                    Some(Pass::Second),
                    "{name}：等待确认停在写出那一遍前"
                );
            } else {
                assert_eq!(walking.pass, Some(PASSES[pass]), "{name}");
            }
        }

        // 每卷状态。
        for (volume, state) in run.volumes.iter().zip(live.states()) {
            assert_eq!(*state, state_of(volume), "{name}：{}", volume.root);
        }

        // 问题计数：坏页、转换失败的卷、无法访问的地方、非漫画文件。坏页数的是此刻——
        // 收摊了的那几卷加上当前这一卷已经报过的；被立即停止掉的那一卷等于没做，它的不算
        // （设计稿 `runTotals` 同一个数法；场景数据里那一卷眼下没有坏页，两边这一格因此还没被验过）。
        let failed_pages: usize = run
            .volumes
            .iter()
            .filter(|volume| volume.state != "aborted")
            .map(|volume| kinds_of(volume).get("failed").copied().unwrap_or(0))
            .sum();
        assert_eq!(live.failures_so_far(), failed_pages, "{name}");
        let failed_volumes = run
            .volumes
            .iter()
            .filter(|volume| volume.state == "failed")
            .count();
        assert_eq!(live.report().failed_volumes.len(), failed_volumes, "{name}");
        let notes = |kind: NoteKind| -> usize {
            run.survey
                .entries
                .iter()
                .flat_map(Entry::notes)
                .filter(|note| note.kind == kind)
                .map(|note| note.entries.len())
                .sum()
        };
        assert_eq!(
            live.unreachable_places().len(),
            notes(NoteKind::Unreachable),
            "{name}"
        );
        assert_eq!(
            live.non_volume_files().len(),
            notes(NoteKind::NonVolume),
            "{name}"
        );
        for (place, said) in live.unreachable_places().iter().zip(
            run.survey
                .entries
                .iter()
                .flat_map(Entry::notes)
                .filter(|note| note.kind == NoteKind::Unreachable)
                .flat_map(|note| &note.entries),
        ) {
            assert_eq!(place.reason, said.reason, "{name}");
        }

        // 灰阶分布与需留意的页：收摊了的卷在报告上，等待确认的那一卷在攒着的那一份上。
        for (listed, volume) in run.survey.volumes.iter().zip(&run.volumes) {
            let Some(tally) = &volume.tally else { continue };
            let root = scene.path(&listed.root);
            let report = live
                .report()
                .volumes
                .iter()
                .find(|report| report.volume == root)
                .or_else(|| live.summarized().filter(|report| report.volume == root))
                .unwrap_or_else(|| panic!("{name}：{} 有分布却没有报告", listed.root));
            let expected: BTreeMap<String, usize> = tally.iter().cloned().collect();
            assert_eq!(tally_of(report), expected, "{name}：{}", listed.root);
            assert_eq!(
                report.page_count(),
                volume.page_count.expect("有分布就有页数"),
                "{name}"
            );
            let expected = kinds_of(volume);
            assert_eq!(
                notable_of(report, panel),
                expected,
                "{name}：{}",
                listed.root
            );
            assert_eq!(
                report.isolated(),
                volume.state == "isolated",
                "{name}：{}",
                listed.root
            );
            if let Some(isolated) = &volume.isolated_output {
                assert_eq!(report.output, scene.path(isolated), "{name}");
            }
            assert_eq!(
                report.timing.elapsed,
                Duration::from_secs_f64(volume.elapsed_s),
                "{name}"
            );
        }
        // 没做成的卷：那句原因。
        for volume in run.volumes.iter().filter(|volume| volume.state == "failed") {
            let root = scene.path(&volume.root);
            let failure = live
                .report()
                .failed_volumes
                .iter()
                .find(|failure| failure.volume == root)
                .unwrap_or_else(|| panic!("{name}：{} 没做成却不在报告上", volume.root));
            assert_eq!(
                Some(failure.reason.as_str()),
                volume.failure.as_deref(),
                "{name}"
            );
        }

        // 写出过没有、答话、停止、结局、阶段。
        assert_eq!(live.has_written(), run.wrote, "{name}");
        assert_eq!(live.for_the_rest().is_some(), run.for_the_rest, "{name}");
        // 按停止按到哪一级记在会话的阶段上，结束之后那一格就没了（结局带着它）。
        // 设计稿的 `latch` 数的正是闩的那三格（`Instruction::code`），读回来走它的反面。
        if run.stage != "ended" {
            assert_eq!(
                scene.session.stopping(),
                Instruction::from_code(run.stop_level),
                "{name}"
            );
        }
        match run.stage.as_str() {
            "running" => {
                assert!(!live.ended() && !scene.session.deciding(), "{name}");
                assert!(scene.session.stage().read_only(), "{name}");
            }
            "deciding" => {
                assert!(scene.session.deciding(), "{name}");
                assert!(live.summarized().is_some(), "{name}：确认点上攒着那一份");
            }
            "ended" => {
                assert!(live.ended(), "{name}");
                assert_eq!(
                    scene.session.stage(),
                    super::super::state::Stage::Ended,
                    "{name}"
                );
                assert_eq!(live.report().outcome, outcome_of(run), "{name}");
                assert_eq!(
                    live.report().elapsed,
                    Duration::from_secs_f64(run.elapsed_s),
                    "{name}"
                );
                assert_eq!(
                    live.report().unreachable_places.len(),
                    notes(NoteKind::Unreachable),
                    "{name}"
                );
                assert_eq!(
                    live.report().non_volume_files.len(),
                    notes(NoteKind::NonVolume),
                    "{name}"
                );
            }
            other => panic!("{name}：不认识的阶段 {other}"),
        }
    }

    /// **非漫画文件的三类原因，从渲染后的那一句都反查得回来**——包括场景数据里眼下没有的
    /// 点不开那一类（它带着错误链）。措辞变了这一条先红，而不是等到某一份场景数据读不进来。
    #[test]
    fn every_kind_of_non_volume_reason_is_read_back_from_its_sentence() {
        for reason in [
            NonVolumeReason::NeitherPageNorArchive,
            NonVolumeReason::ArchiveWithoutAPage,
            NonVolumeReason::Unopenable("打开 字体包.zip: 不是 zip".to_owned()),
        ] {
            let sentence = render::non_volume_reason(&reason);
            let back = non_volume_reason_of(&sentence);
            assert_eq!(render::non_volume_reason(&back), sentence);
            assert_eq!(
                std::mem::discriminant(&back),
                std::mem::discriminant(&reason)
            );
        }
    }

    /// **`running.json` 摆出来的那一趟与场景数据逐项相同**（票面第二条）：总进度、当前卷与环节、
    /// 每卷状态、问题计数、灰阶分布。不经画法。这一条是头一道接缝；全部场景在下一条。
    #[test]
    fn the_running_scene_is_the_run_its_data_describes() {
        let scene = Scene::named("running");
        agrees_with_its_data(&scene);
        let live = scene.live();
        // 设计快照抬头上的几个数，直接对一遍。
        assert_eq!(live.overall().volume, 27, "第 27/84 卷");
        assert_eq!(live.overall().walked, 15933, "15933/46809 步");
        assert_eq!(live.failures_so_far(), 1, "坏页 1 页");
        assert_eq!(live.report().failed_volumes.len(), 1, "转换失败 1 卷");
        assert_eq!(live.unreachable_places().len(), 1, "无法访问 1 处");
    }

    /// **「推进几秒」那一步换掉的是那一趟，不是界面状态**（`session-redesign/10`）：
    /// `running-s-advance` 从「转换中」起，推进之后那一趟收了场，而视图、光标、
    /// 展开着的那几个目录一格不动——那是前面那几步输入摆出来的，正是那一串要看的东西。
    /// 家目录也不换：换进来的那一趟报回来的卷根与树上的仍对得上。
    ///
    /// 换进来的那一趟**与那一串自己的场景数据逐项相同**（走的是同一条 `agrees_with_its_data`）。
    #[test]
    fn advancing_swaps_the_run_and_leaves_the_interface_alone() {
        let mut scene = Scene::named("running");
        let before = scene.session.views.clone();
        let home = scene.home.clone();
        assert!(!scene.live().ended(), "起点那一趟还在跑");
        scene.advance_to(sequence_data("running-s-advance"));
        assert!(scene.live().ended(), "推进之后那一趟收了场");
        assert_eq!(scene.session.views, before, "界面状态一格都没动");
        assert_eq!(scene.home, home, "家目录没换");
        assert_eq!(
            scene.live().roster().len(),
            scene.session.views.task.tree.roots.len(),
            "清单与树上的卷根仍对得上"
        );
        agrees_with_its_data(&scene);
    }

    /// **清单上每一串交互序列都读得成步**，每一步都翻得成输入（`session-redesign/06`）：
    /// 键名一个都不认不出，打字那一步一个字一个输入，推进与换尺寸不是输入。
    #[test]
    fn every_sequence_in_the_manifest_reads_into_steps() {
        let names = sequences();
        assert!(!names.is_empty());
        for name in &names {
            let sequence = sequence(name);
            assert_eq!(sequence.name, *name);
            assert!(!sequence.steps.is_empty(), "「{name}」没有步");
            assert!(sequence.size.0 > 0 && sequence.size.1 > 0);
            for step in &sequence.steps {
                let inputs = step.inputs();
                match step {
                    Step::Advance(_) | Step::Resize(_, _) => assert!(inputs.is_empty()),
                    Step::Type(text) => assert_eq!(inputs.len(), text.chars().count()),
                    Step::DoubleClick(_, _) => assert_eq!(inputs.len(), 2),
                    _ => assert_eq!(inputs.len(), 1, "「{name}」的 {step:?}"),
                }
            }
        }
        assert_eq!(
            sequence("fresh-dd").steps,
            [Step::Key("d".to_owned()), Step::Key("d".to_owned())]
        );
        assert_eq!(key_named("Space"), Input::Key(Key::Space));
        assert_eq!(key_named("C-w"), Input::Ctrl('w'));
        assert_eq!(key_named("C-c"), Input::Key(Key::Interrupt));
        assert_eq!(key_named("?"), Input::Key(Key::Char('?')));
    }

    /// **11 个场景的那一趟与设置都摆得出来**，各与自己的场景数据逐项相同（票面第一、二条）。
    #[test]
    fn every_scene_is_what_its_data_describes() {
        let scenes = scenes();
        assert_eq!(scenes.len(), 11, "清单上有场景数据的场景：{scenes:?}");
        for name in scenes {
            agrees_with_its_data(&Scene::named(&name));
        }
    }

    /// **交互走完那一刻那一趟变了的每一串也摆得出来**：答话、按停止、推进几秒、立即停止……
    /// 后面各票按序列认领用例时，起点就在这里。序列的场景数据与起点场景一样的那几样写成
    /// `"unchanged"`，读的时候补回来；那一趟没变的那一百多串与起点场景是同一趟，上一条已经摆过。
    #[test]
    fn every_sequence_that_moves_the_run_ends_where_its_data_says() {
        let moved: Vec<String> = sequences()
            .into_iter()
            .filter(|name| json(&format!("sequences/{name}.scene.json"))["run"] != "unchanged")
            .collect();
        assert!(moved.len() >= 20, "那一趟变了的序列：{moved:?}");
        for name in moved {
            agrees_with_its_data(&Scene::after(&name));
        }
    }
}

/// **报告那一处对场景数据说出来的字，在设计快照的字网格里找得到**（票面第三条）。
///
/// 屏上的字由库照语义字段说出来，再与设计快照逐字比——Q718 要保住的「一处出处」就是在这里被验的
/// （spec《夹具》）。这几条要读设计快照，因此挂在 `tui` 后面（读法在 `super::draw::design`）；
/// 夹具本身两趟都编。
#[cfg(all(test, feature = "tui"))]
mod on_the_grid {
    use super::*;
    use crate::render::{Field, RowKind};
    use crate::session::draw::design;

    /// 一句话核它头几个字：行尾那一句常被屏宽截掉（`…这一卷整卷写到隔`），整句不在屏上。
    const HEAD: usize = 12;

    /// 一句话的头几个字。
    fn head(text: &str) -> String {
        text.chars().take(HEAD).collect()
    }

    /// 字网格里带着 `marker` 的头一行。
    fn line_with<'a>(lines: &'a [String], marker: &str) -> &'a str {
        line_with_all(lines, &[marker])
    }

    /// 字网格里同时带着这几个记号的头一行（卷名会在总览的当前卷那一行上先出现一次）。
    fn line_with_all<'a>(lines: &'a [String], markers: &[&str]) -> &'a str {
        lines
            .iter()
            .find(|line| markers.iter().all(|marker| line.contains(marker)))
            .unwrap_or_else(|| {
                panic!(
                    "字网格里没有一行同时带着 {markers:?}：\n{}",
                    lines.join("\n")
                )
            })
    }

    /// 一份报告里某一卷的报告。
    fn report_of<'a>(scene: &'a Scene, root: &str) -> &'a VolumeReport {
        let live = scene.live();
        let root = scene.path(root);
        live.report()
            .volumes
            .iter()
            .find(|report| report.volume == root)
            .unwrap_or_else(|| panic!("报告上没有 {}", root.display()))
    }

    /// 一行里某一格的字。
    fn cell(row: &render::Row, field: Field) -> &str {
        row.cell(field)
            .unwrap_or_else(|| panic!("{:?} 那一行上没有 {field:?} 那一格", row.kind))
    }

    /// 卷级那几行里某一种行的整句。
    fn sentence(rows: &[render::Row], kind: RowKind) -> String {
        rows.iter()
            .find(|row| row.kind == kind)
            .map(|row| cell(row, Field::Sentence).to_owned())
            .unwrap_or_else(|| panic!("卷级那几行里没有 {kind:?}"))
    }

    /// **逐页各格**：每页结果那一景开着的那一卷，尺寸、缩放、灰阶、原因、画质分、坏页那一句，
    /// 逐格出自 [`render::pages`]，在设计快照（只列需留意的页）与 `a` 之后那一屏（全部页）上找得到；
    /// 抬头那一行的灰阶分布出自 [`render::volume`]。
    #[test]
    fn the_pages_of_the_open_volume_are_on_the_grid() {
        let scene = Scene::named("pages");
        let live = scene.live();
        let opened = scene.data.session["pages"]["volume"]
            .as_str()
            .expect("每页结果那一景开着一卷");
        let report = report_of(&scene, opened);
        let rows = render::pages(report, live.mode());
        let notable = design::snapshot("pages", 120, 36).lines();
        let all = design::sequence("pages-a").lines();

        // 抬头：灰阶分布那一格。
        let tally = tally_text(report, live);
        assert!(line_with(&notable, "灰阶分布").contains(&tally), "{tally}");

        // 坏页：尺寸、缩放（它的尺寸从哪来）、那一句原因，三格在同一行上。
        let failed = report
            .pages
            .iter()
            .position(|page| page.failure().is_some())
            .expect("开着的那一卷有一张坏页");
        let (geometry, verdict) = (&rows[failed * 2], &rows[failed * 2 + 1]);
        let name = render::volume_name(&report.pages[failed].source);
        let line = line_with(&notable, &name);
        for text in [
            cell(geometry, Field::Size),
            cell(geometry, Field::Scaling),
            cell(verdict, Field::Sentence),
        ] {
            assert!(line.contains(text), "「{text}」不在这一行上：{line}");
        }

        // 全部页那一屏的头几页：尺寸、缩放、灰阶、原因、画质分。
        for at in 0..3 {
            let (geometry, verdict) = (&rows[at * 2], &rows[at * 2 + 1]);
            let name = render::volume_name(&report.pages[at].source);
            let line = line_with(&all, &name);
            for text in [
                cell(geometry, Field::Size),
                cell(geometry, Field::Scaling),
                cell(verdict, Field::Candidate),
                cell(verdict, Field::Reason),
                cell(verdict, Field::VerdictScore),
            ] {
                assert!(line.contains(text), "「{text}」不在这一行上：{line}");
            }
        }
    }

    /// 一卷的灰阶分布那一格的字（卷级那几行里的那一格，`render::tally_column`）。
    fn tally_text(report: &VolumeReport, live: &Live) -> String {
        let rows = render::volume(report, live.report().white_align_limit);
        render::tally_column(&rows).expect("做过事的卷有灰阶分布")
    }

    /// **卷行行尾与备注行**（转换中那一景）：隔离那一句、没做成的原因、无法访问的地方那条错误链、
    /// 非漫画文件那一小结的抬头，各出自报告那一处，在设计快照上找得到（行尾被屏宽截掉的只核头几个字）。
    #[test]
    fn row_tails_and_notes_of_the_running_scene_are_on_the_grid() {
        let scene = Scene::named("running");
        let live = scene.live();
        let lines = design::snapshot("running", 120, 36).lines();
        let limit = live.report().white_align_limit;

        // 隔离那一卷：行尾是隔离那一句；灰阶分布那一格也在同一行。
        let isolated = scene
            .data
            .run
            .as_ref()
            .and_then(|run| run.volumes.iter().find(|volume| volume.state == "isolated"))
            .expect("转换中那一景有一卷进了隔离");
        let report = report_of(&scene, &isolated.root);
        let rows = render::volume(report, limit);
        let line = line_with(&lines, &render::volume_name(&report.volume));
        let said = sentence(&rows, RowKind::Isolated);
        assert!(
            line.contains(&head(&said)),
            "「{said}」不在这一行上：{line}"
        );
        let tally = tally_text(report, live);
        assert!(
            line.contains(&head(&tally)),
            "「{tally}」不在这一行上：{line}"
        );

        // 没做成的那一卷：行尾是那句原因。
        let failure = &live.report().failed_volumes[0];
        let row = render::failed_volume(failure);
        let line = line_with(&lines, &render::volume_name(&failure.volume));
        let said = cell(&row, Field::Sentence);
        assert!(line.contains(said), "「{said}」不在这一行上：{line}");

        // 备注行：无法访问的地方给那条错误链（原样），非漫画文件给那一小结抬头那一句。
        let place = &live.unreachable_places()[0];
        let line = line_with(&lines, "无法访问  ");
        assert!(
            line.contains(&place.reason),
            "「{}」不在这一行上：{line}",
            place.reason
        );
        let mut with_tables = live.report().clone();
        with_tables.non_volume_files = live.non_volume_files().to_vec();
        let tail = render::tail(&with_tables);
        let said = sentence(&tail, RowKind::NonVolumeTail);
        let first = said.lines().next().expect("那一小结有抬头");
        let line = line_with(&lines, "已忽略");
        assert!(
            line.contains(&head(first)),
            "「{first}」不在这一行上：{line}"
        );
    }

    /// **整卷统一灰阶那一景**：跳过那一句、代表页、灰阶分布，各出自报告那一处，在设计快照上找得到；
    /// **等待确认那一景**：确认条上的灰阶分布出自攒着的那一份。
    #[test]
    fn the_envelope_and_deciding_scenes_are_on_the_grid() {
        let scene = Scene::named("envelope");
        let live = scene.live();
        let lines = design::snapshot("envelope", 120, 36).lines();
        let limit = live.report().white_align_limit;
        let run = scene.data.run.as_ref().expect("有一趟");

        let skipped = run
            .volumes
            .iter()
            .find(|volume| volume.state == "skipped")
            .expect("有跳过的卷");
        let report = report_of(&scene, &skipped.root);
        let said = sentence(&render::volume(report, limit), RowKind::Skipped);
        let line = line_with(&lines, &render::volume_name(&report.volume));
        assert!(
            line.contains(&head(&said)),
            "「{said}」不在这一行上：{line}"
        );

        let with_driver = run
            .volumes
            .iter()
            .find(|volume| {
                volume.state == "done"
                    && volume
                        .notable_pages
                        .iter()
                        .flatten()
                        .any(|page| page.kinds.iter().any(|kind| kind == "outlier"))
            })
            .expect("有一卷带差异大的页");
        let report = report_of(&scene, &with_driver.root);
        let rows = render::volume(report, limit);
        let line = line_with_all(&lines, &[&render::volume_name(&report.volume), "代表页"]);
        let driver = rows
            .iter()
            .find(|row| row.kind == RowKind::Driver)
            .map(|row| render::volume_name(Path::new(cell(row, Field::Source))))
            .expect("整卷统一灰阶那一趟每一卷有代表页");
        assert!(
            line.contains(&driver),
            "代表页「{driver}」不在这一行上：{line}"
        );
        let tally = tally_text(report, live);
        assert!(line.contains(&tally), "「{tally}」不在这一行上：{line}");

        let scene = Scene::named("deciding");
        let live = scene.live();
        let lines = design::snapshot("deciding", 120, 36).lines();
        let summarized = live.summarized().expect("停在确认点上");
        let tally = tally_text(summarized, live);
        let line = line_with_all(
            &lines,
            &[&render::volume_name(&summarized.volume), "灰阶分布"],
        );
        assert!(line.contains(&tally), "「{tally}」不在确认条上：{line}");
    }
}
