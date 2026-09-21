//! **配置视图**那一副的内容：设置栏上那几行、详情栏答的那几样、画质判定参数那五行
//! （`CONTEXT.md` 的《配置视图》《设置栏》《详情栏》《下钻》《画质判定参数》；
//! spec 的《配置视图》）。
//!
//! 画法在 [`super::shell`] 底下那三块（顶上一条预设、设置栏、详情栏），本模块答**屏上那几行
//! 各是什么**：三组各有哪几项（[`lines`]）、一项此刻印什么值（[`Item::shown`]）、详情栏列得出
//! 哪几格（[`choices`]）、套着的预设里这一项怎么设（[`preset_says`]），以及每一项底下
//! 那一段**长说明**（[`Item::about`]）——那是屏上唯一一段会话自己写的散文，出处只有这一处。
//!
//! # 行内那一句不在这里
//!
//! 画质判定参数那一组**行内那一句就是报告抬头里的那一行**（ADR 0016）：项名与数一个字都不
//! 另写，出处是 [`crate::render::judging`]，本模块只管「这一项此刻印哪一句」（[`premise`]）
//! 与「这一刻它还与抬头相同吗」（[`quotes_the_header`]）。详情栏里那一段**长说明**才是会话
//! 自己的字，它在本模块（[`Item::about`]）。
//!
//! # 取值环也不在这里
//!
//! 有取值环的那几项，环上有哪几格由**那一项自己的环**说了算（[`super::state::Session::ring`]）——
//! 环上加一个取值，详情栏当场跟着多一格。本模块不另列一份清单；型号那一项走的是另一路
//! （**下钻**：第一层是屏幕规格，[`Profile::devices_by_panel`] 那一份分组，
//! 与未知型号那条错误消息同一份）。
//!
//! # 它一个终端都不碰
//!
//! 因此摆在 `tui` 特性**外面**（见 `super` 的《终端库在哪一半》）：`--no-default-features`
//! 那一趟照编、照跑它自带的用例。

use tonefit::{Panel, Profile};

use super::state::{DEVICE_FIELDS, Field, Session, Shape, TASTE_FIELDS};
use crate::preset::Preset;
use crate::render::{self, Judging, Switches};

/// 还没挑型号时**设备配置**那一行印什么：抬头那一行到这一刻还出不来（界挂在 profile 上）。
const NO_MODEL: &str = "型号未选择";

/// **选项冲突**一条都没咬上时那一行印什么（`CONTEXT.md` 的《画质判定参数》：否则写「无」）。
const NO_INTERLOCK: &str = "无";

/// 设置栏上的一**组**（`CONTEXT.md` 的《设置栏》：三组）。
///
/// 分界线与三组设置画在同一处**生命周期**上（[`super::state::Layer`]），
/// 只是第三组换成了**画质判定参数**——路径与输出那一组不在配置视图里（`CONTEXT.md`
/// 的《路径与输出》：它跟着这一趟走，摆在任务视图开跑之前那一副里）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// **设备设置**：换设备时才需要改。
    Device,
    /// **处理选项**：每次转换都可以调。
    Taste,
    /// **画质判定参数**：只读。
    Judging,
}

impl Band {
    /// 三组，次序就是屏上的次序。
    pub const ALL: [Self; 3] = [Self::Device, Self::Taste, Self::Judging];

    /// 组名。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn name(self) -> &'static str {
        match self {
            Self::Device => "设备设置",
            Self::Taste => "处理选项",
            Self::Judging => "画质判定参数",
        }
    }

    /// 组名后面那半句：这一组什么时候要动。**它说的正是这三组为什么分成三组**
    /// ——换设备才动的、每趟都能调的、一个字都改不动的。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn note(self) -> &'static str {
        match self {
            Self::Device => "换设备时才需要改",
            Self::Taste => "每次转换都可以调",
            Self::Judging => "只读",
        }
    }

    /// 这一组里**停得住**的那几项，次序就是屏上的次序。
    pub fn items(self) -> Vec<Item> {
        match self {
            Self::Device => DEVICE_FIELDS.into_iter().map(Item::Setting).collect(),
            Self::Taste => TASTE_FIELDS.into_iter().map(Item::Setting).collect(),
            Self::Judging => Judging::ALL.into_iter().map(Item::Premise).collect(),
        }
    }
}

/// 设置栏上**停得住的一项**——光标记的是它，不是第几行（与卷列表那一条同一条规矩）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// 设备设置与处理选项那两组里可改的一行。
    Setting(Field),
    /// 画质判定参数那一组里只读的一行。
    Premise(Judging),
}

impl Item {
    /// 这一行的名字。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn label(self) -> &'static str {
        match self {
            Self::Setting(field) => field.label(),
            Self::Premise(which) => which.label(),
        }
    }

    /// 设置栏上**取值那一列**印的字。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn shown(self, session: &Session) -> String {
        match self {
            Self::Setting(field) => session.shown(field),
            Self::Premise(which) => premise(session, which),
        }
    }

    /// 这一行的取值**说出来了没有**——说了的一档亮着，跟着默认值走的那一档压成次要那一色
    /// （设计稿 `drawCfgLeft` 的 `set`）。型号那一行恒算说了，画质判定参数那一组恒算没说：
    /// 那一组印的不是一个取值，是这一趟照什么判定。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn said(self, session: &Session) -> bool {
        match self {
            Self::Setting(Field::Profile) => true,
            Self::Setting(field) => !session.unsaid(field),
            Self::Premise(_) => false,
        }
    }

    /// 详情栏里那一段**长说明**——**会话自己的字**（`CONTEXT.md` 的《画质判定参数》：
    /// 「详情栏里另有一段说明，那是界面层自己的解释文字，出处只在会话这一侧」）。
    ///
    /// 选项冲突那一项两句话分两种情形：咬上了说「代价是什么看上面那句」，
    /// 没咬上说「这一组之间没有冲突」——两句都指着 `tonefit --help` 的末尾那张全表。
    #[cfg_attr(
        not(feature = "tui"),
        expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
    )]
    pub fn about(self, session: &Session) -> &'static str {
        match self {
            Self::Setting(field) => about_setting(field),
            Self::Premise(Judging::Interlock) if !engaged(session) => {
                "当前这组设置之间没有互相冲突的组合。所有可能冲突的组合写在 tonefit --help 的末尾。"
            }
            Self::Premise(which) => about_premise(which),
        }
    }
}

/// 设置栏上的一行：组抬头**停不住**，其余两种停得住（[`Item`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(feature = "tui"),
    expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
pub enum Line {
    /// 一组的抬头。
    Band(Band),
    /// 停得住的一项。
    Item(Item),
}

/// 设置栏上那几行，次序就是屏上的次序：一组一个抬头，底下是它那几项。
#[cfg_attr(
    not(feature = "tui"),
    expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
pub fn lines() -> Vec<Line> {
    Band::ALL
        .into_iter()
        .flat_map(|band| {
            std::iter::once(Line::Band(band)).chain(band.items().into_iter().map(Line::Item))
        })
        .collect()
}

/// **停得住**的那几项，次序同上——框底边那句 `4 of 20` 数的就是它。
pub fn items() -> Vec<Item> {
    Band::ALL.into_iter().flat_map(Band::items).collect()
}

/// 画质判定参数那一行**行内那一句**（项名之后的全部）。
///
/// 出处是报告抬头那一处（[`crate::render::judging`]），会话一个字都不另写。
/// 还没挑型号时设备配置那一行答不出抬头（界挂在 profile 上），说的是会话自己的一句；
/// 选项冲突一条都没咬上时写「无」。
pub fn premise(session: &Session, which: Judging) -> String {
    render::judging(
        which,
        session.calibrated_profile().as_ref(),
        switches(session),
    )
    .unwrap_or_else(|| {
        match which {
            Judging::Interlock => NO_INTERLOCK,
            _ => NO_MODEL,
        }
        .to_owned()
    })
}

/// 详情栏里摊开的那**一整行**：项名 · 空格 · 行内那一句——
/// **与报告抬头里的那一行逐字相同**（`CONTEXT.md` 的《画质判定参数》）。
pub fn premise_line(session: &Session, which: Judging) -> String {
    format!("{} {}", which.label(), premise(session, which))
}

/// 这一行此刻**真与抬头里的那一行逐字相同吗**——详情栏那一句注解按它出不出。
///
/// 只有选项冲突答得出「不」：一条都没咬上时抬头一个字都不说，屏上那句「无」是界面自己的话
/// （设计稿 `drawCfgRight`）。**别的四项照旧注**——型号没挑时那一句同样是界面自己的，
/// 而那一刻整个配置视图都还没法判定，设计稿没为它分支，这里照设计稿。
pub fn quotes_the_header(session: &Session, which: Judging) -> bool {
    which != Judging::Interlock || engaged(session)
}

/// 这一趟的开关咬上了抬头会露面的那种选项冲突吗。
fn engaged(session: &Session) -> bool {
    render::interlocking(switches(session)).next().is_some()
}

/// 抬头那几行要问的**这一趟那三个开关**：处理选项那一组落到默认值之后的取值。
fn switches(session: &Session) -> Switches {
    Switches {
        fit: session.taste.fit(),
        crop: session.taste.crop(),
        split: session.taste.split_rule(),
    }
}

/// 这一项**记得进一份预设**吗——**型号不算，这一处一次判掉**。
///
/// 设计稿的存、套与那张单子（`configKey` 与 `changedKeys`）三处都只收取值环与自由填的那几项，
/// 型号那一行的取值是一块面板的名字，走的是它自己那一路（换型号还要清掉两个标定数）。
/// 屏上行尾那个 `*`、顶上那句「改动了 N 项」、预设栏那句「包含 N 项设置」、存出去的那一份，
/// 问的都是这一句。
///
/// （它与 `CONTEXT.md` 的《预设》对不上——那条词条说预设装设备设置与处理选项两组，
/// 而型号在设备设置里。照设计稿走，记在停车场。）
pub fn stored(field: Field) -> bool {
    field != Field::Profile
}

/// **一份预设记得下的那几项**，次序照设置栏：设备设置与处理选项两组里[记得进去](stored)
/// 的那十四项。
pub fn stored_fields() -> Vec<Field> {
    DEVICE_FIELDS
        .into_iter()
        .chain(TASTE_FIELDS)
        .filter(|field| stored(*field))
        .collect()
}

/// 与套着的那份预设**不同**的那几项，次序照设置栏（顶上一条预设写「改动了 N 项：…」，
/// 设置栏上这几项行尾带 `*`）。没套预设就一项都没有。
pub fn changed(session: &Session) -> Vec<Field> {
    stored_fields()
        .into_iter()
        .filter(|field| starred(session, *field))
        .collect()
}

/// 这一项**与套着的预设不同**吗——设置栏上行尾那个 `*` 就是它。没套预设时一项都不带。
/// 型号不算，理由在 [`stored`]。
pub fn starred(session: &Session, field: Field) -> bool {
    stored(field)
        && session
            .views
            .config
            .applied
            .as_ref()
            .is_some_and(|applied| session.differs_from(field, &applied.preset))
}

/// 这一份预设**说了哪几项**，次序照设置栏（预设栏上那一行写「包含 N 项设置：…」）。
///
/// 「没说」与「说了一个恰好等于默认值的值」是两件事（`CONTEXT.md` 的《预设》那一段），
/// 判它的是 [`Session::unsaid`] 一处：把这一份摆进一个探针会话里问一遍。
pub fn said_fields(session: &Session, preset: &Preset) -> Vec<Field> {
    let mut probe = session.clone();
    probe.device = preset.device.clone();
    probe.taste = preset.taste.clone();
    stored_fields()
        .into_iter()
        .filter(|field| !probe.unsaid(*field))
        .collect()
}

/// 套着的那份预设里这一项怎么设：**没设**就是 `None`（屏上写「未设置（使用默认值）」），
/// 设了就按**那一项自己的写法**印——与取值环上那一格同一份出处（[`Session::shown`]）。
#[cfg_attr(
    not(feature = "tui"),
    expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
pub fn preset_says(session: &Session, preset: &Preset, field: Field) -> Option<String> {
    let mut probe = session.clone();
    probe.device = preset.device.clone();
    probe.taste = preset.taste.clone();
    (!probe.unsaid(field)).then(|| probe.shown(field))
}

/// 详情栏上这一项**列得出的那几格**（`CONTEXT.md` 的《详情栏》《下钻》）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choices {
    /// 有取值环的那几项：环上每一格，**第一格恒是「没说」那一格**；`chosen` 是此刻生效的那一格。
    Ring { cells: Vec<String>, chosen: usize },
    /// 型号那一项的第一层：一块**屏幕规格**一行，带着它底下有几个型号。
    /// 型号停在内置表外的一个名字上时一格都不是生效的。
    Panels {
        cells: Vec<(String, usize)>,
        chosen: Option<usize>,
    },
    /// **下钻**进去那一层：这块屏幕规格底下的型号，**没有「没挑」那一格**（它在上一层的第一格）。
    Models {
        panel: Panel,
        cells: Vec<&'static str>,
        chosen: Option<usize>,
    },
    /// 自由填的那几项：当前值一句，`i` 改。一格都停不住。
    Filled(String),
    /// 画质判定参数：只读。一格都停不住。
    Premise,
}

impl Choices {
    /// 停得住几格——详情栏的光标夹在 `0..stops` 里。**一格都停不住的那两种答 1**：
    /// 光标因此恒停在第 0 格上，而屏上不画它（设计稿 `rightCount`）。
    pub fn stops(&self) -> usize {
        match self {
            Self::Ring { cells, .. } => cells.len(),
            Self::Panels { cells, .. } => cells.len(),
            Self::Models { cells, .. } => cells.len(),
            Self::Filled(_) | Self::Premise => 1,
        }
    }

    /// 刚进这一栏时光标落在第几格：**此刻生效的那一格**；这一层里一格都不生效
    /// （型号停在内置表外的一个名字上）就落在头一格。
    pub fn lands_on(&self) -> usize {
        match self {
            Self::Ring { chosen, .. } => *chosen,
            Self::Panels { chosen, .. } | Self::Models { chosen, .. } => chosen.unwrap_or(0),
            Self::Filled(_) | Self::Premise => 0,
        }
    }
}

/// 详情栏此刻列的那几格：光标停在设置栏哪一项、下钻进了哪一块屏幕规格说了算。
pub fn choices(session: &Session, item: Item, drill: Option<Panel>) -> Choices {
    let Item::Setting(field) = item else {
        return Choices::Premise;
    };
    if field.drills() {
        let groups = panels();
        let current = session.device.profile.as_deref();
        if let Some((panel, devices)) = drill.and_then(|wanted| under(&groups, wanted)) {
            return Choices::Models {
                panel,
                cells: devices.clone(),
                chosen: current.and_then(|name| devices.iter().position(|device| *device == name)),
            };
        }
        return Choices::Panels {
            chosen: current.and_then(|name| {
                groups
                    .iter()
                    .position(|(_, devices)| devices.contains(&name))
            }),
            cells: groups
                .iter()
                .map(|(panel, devices)| (panel.to_string(), devices.len()))
                .collect(),
        };
    }
    if field.shape() == Shape::Cycle {
        let (cells, chosen) = session.ring(field);
        return Choices::Ring { cells, chosen };
    }
    Choices::Filled(session.shown(field))
}

/// 这一块屏幕规格底下的那几个型号；内置表里没有这一块就是 `None`。
fn under<'a>(
    groups: &'a [(Panel, Vec<&'static str>)],
    wanted: Panel,
) -> Option<(Panel, &'a Vec<&'static str>)> {
    groups
        .iter()
        .find(|(panel, _)| *panel == wanted)
        .map(|(panel, devices)| (*panel, devices))
}

/// **屏幕规格分组**：详情栏第一层一块一行，第几格就是第几块（`CONTEXT.md` 的《下钻》）。
///
/// 分组走 [`Profile::devices_by_panel`]，与未知型号那条错误消息**同一份**：内置表里加一个型号、
/// 多一块屏幕规格，这一列当场跟着变。每一块印的是它自己的 `Display`，会话不另写一份格式。
pub fn panels() -> Vec<(Panel, Vec<&'static str>)> {
    Profile::devices_by_panel()
}

/// 处理选项与设备设置那几项的**长说明**（设计稿 `CONFIG` 的 `desc`）。
#[cfg_attr(
    not(feature = "tui"),
    expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
fn about_setting(field: Field) -> &'static str {
    match field {
        Field::Profile => {
            "先选屏幕规格，再选型号：同一种屏幕的型号，转出来的效果完全一样。表里没有你的设备时，挑一个屏幕规格相同的型号，再按实际情况填可见灰阶数。换型号会清空之前填的可见灰阶数和画质门槛。"
        }
        Field::GrayLevels => {
            "你在这台设备上用肉眼能分清几级灰。按 c 生成灰阶测试图，拷进设备里数一数；不填就用屏幕标称的灰阶数。它和标称值不一定相同：显示固件、环境光、看的距离都会影响。"
        }
        Field::Threshold => {
            "画质分低于这个数，就算看不出和原图的差别。这个数是在 boox-poke6 上让人盲看对比定出来的：像素密度（PPI）相同的屏幕可以直接沿用，密度不同的屏幕还没验证过。"
        }
        Field::Fit => {
            "按高度铺满：页面高度等于屏幕高度，宽度按比例缩放，比屏幕宽的页（多半是跨页）要左右平移着看。整页放进屏幕：整页缩进屏幕里，四周可能留边。普通漫画页两种方式结果一样，区别只出现在跨页上。"
        }
        Field::Crop => "自动裁掉扫描时留下的白边。每页各裁各的，所以翻页时字的大小可能略有变化。",
        Field::Split => {
            "把横跨两页的大图从中缝切成两页；找不到中缝的（一幅画横跨两页）不切。开着时，有跨页的卷体积会变大不少。"
        }
        Field::SplitThreshold => "页面的宽高比超过屏幕宽高比的多少倍，才当成跨页处理。",
        Field::ReadingOrder => "跨页拆开之后，左右两半谁排在前面。日漫是从右往左。",
        Field::Filter => {
            "缩小图片时用的算法。lanczos3 最清晰，一般不用改；area 会偏糊，只用来对比。"
        }
        Field::WhiteAlignLimit => {
            "扫描出来的纸色常常略微发灰，加抖动时白底上会冒出零星噪点。开着时把接近纯白的纸色提成纯白，数字是最多提几级灰度；0 = 关闭，结果和没有这个功能时完全一样。"
        }
        Field::BitDepth => {
            "指定整卷用几级灰，不再逐页自动判断。电子墨水屏最多显示 16 级灰，所以只有这三档。表格里的写法：1bit = 2 级灰，2bit = 4 级灰，4bit = 16 级灰；+FS = 加抖动。"
        }
        Field::Dither => {
            "用细小的点模拟中间灰度，看起来更细腻，但点子可能显眼。尺寸没有贴合屏幕的页一律不加抖动：阅读器再缩放一次，会把点子糊成脏灰。"
        }
        Field::Envelope => {
            "关（默认）：每页各自选最合适的档位，不会为了少数几页让整卷变大。开：整卷用同一个档位，和其他页差异特别大的页单独处理。"
        }
        Field::CacheBudget => {
            "转换时最多占用多少内存，超出的部分临时写到硬盘上。它限制的是内存占用，不限制卷的大小。"
        }
        Field::IoMode => {
            "自动：按硬盘类型自己选。逐个读：适合机械硬盘和网络盘。同时读：适合固态硬盘。"
        }
    }
}

/// 画质判定参数那几项的**长说明**（设计稿 `CONFIG` 的 `desc`）。
///
/// 选项冲突那一项咬上时说这一句；没咬上那一句在 [`Item::about`] 里分出去。
#[cfg_attr(
    not(feature = "tui"),
    expect(dead_code, reason = "只有画法读得到，而它在 tui 特性后面")
)]
fn about_premise(which: Judging) -> &'static str {
    match which {
        Judging::Device => {
            "下一趟照这份设备配置判定：型号、屏幕规格、画质门槛，门槛是在哪块屏幕上实测的也写着。像素密度（PPI）相同的屏幕可以直接沿用这个门槛，密度不同的还没验证过。"
        }
        Judging::Composition => {
            "画质分由两部分相加。① 整体走样：把原图和转换后的图都稍微模糊一下（相当于在正常阅读距离看），比较灰度差多少，色带、灰调发闷都算在这里。② 抖动颗粒：抖动的点子本身有多扎眼；起伏不到「相邻两级灰度差的 21.6%」就当看不见，不扣分。21.6% 这个比例是在 boox-poke6 上实测的。"
        }
        Judging::Masking => {
            "线稿密、细节多的地方，瑕疵不容易被看出来，所以扣分打折；大片平涂的地方不打折。最低折扣 0.50：再怎么花的地方，扣分也至少保留一半。中点细节度 8.0：一块区域的细节度（像素偏离周围平均值的程度）到 8.0 时，正好打七五折，也就是不打折和五折的中间。这两个数目前是暂定值，还没用真机校准。"
        }
        Judging::Aggregation => {
            "把页面切成 32x32 像素的小块分别打分。一页的分数取最差那 1% 的小块，但最多只看最差的 8 块。不取平均，免得大片留白把局部的崩坏冲淡。8 这个数是暂定值。"
        }
        Judging::Interlock => {
            "这组设置里有两项凑在一起会互相削弱：上面那句写着是哪两项、代价是什么。组合本身能用，只是拿到的是部分收益。所有可能冲突的组合写在 tonefit --help 的末尾。"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::scene::Scene;
    use crate::session::state::Stage;
    use crate::session::view::Input;

    /// **画质判定参数那五行与报告抬头逐字相同**（`session-redesign/13` 票面第三条）。
    ///
    /// 比的是**真的那一份抬头**：`running` 那一景的三组设置拼出一份 `Request` 跑起来，
    /// 攒到此刻的那份报告交给 [`crate::render::header`]，屏上那五行一行不落地在它里面。
    /// 项名与数一个字都不另写，这一条因此会在有人在会话这一侧另抄一份的那一天变红。
    #[test]
    fn every_judging_row_is_a_line_of_the_report_header() {
        let scene = Scene::named("running");
        let live = scene.live();
        let header = crate::render::header(&live.report(), live.mode());
        let lines: Vec<&str> = header.lines().collect();
        for which in Judging::ALL {
            let line = premise_line(&scene.session, which);
            assert!(
                lines.contains(&line.as_str()),
                "「{line}」不在报告抬头里：\n{header}"
            );
            assert!(
                quotes_the_header(&scene.session, which),
                "{which:?} 这一行明明与抬头相同，却说自己不是"
            );
        }
    }

    /// **选项冲突一条都没咬上时写「无」**，而且那一刻不说自己与抬头相同——
    /// 抬头那一刻一个字都不说（`CONTEXT.md` 的《画质判定参数》）。
    #[test]
    fn with_no_interlock_engaged_the_row_says_none_and_claims_nothing() {
        let mut scene = Scene::named("config");
        // `config` 那一景咬着的是「拆分开着、缩放方式却是 fit-inside」那一条：关掉拆分就散了。
        assert!(engaged(&scene.session));
        scene.session.taste.split = Some(false);
        assert!(!engaged(&scene.session));
        assert_eq!(premise(&scene.session, Judging::Interlock), "无");
        assert!(!quotes_the_header(&scene.session, Judging::Interlock));
    }

    /// **跑着时一个改动都定不下，而拦下它的是阶段那一维**（票面第四条）。
    ///
    /// 走的是同一副键（`l` 进详情栏、挪一格、`l` 定下来）：还没开始那一档设备设置与处理选项
    /// 那十五项**一项不落**都改得动，跑着与等待确认那两档一项都改不动——**两趟差的只有阶段**，
    /// 焦点、光标、按的键一格不差。自由填的那几项连输入行都开不起来。
    /// 画质判定参数那五项两档都不动：它们只读，与阶段无关。
    #[test]
    fn during_a_run_not_one_setting_settles_and_the_stage_is_what_stops_it() {
        let key = |letter: char| Input::Key(crate::session::state::Key::Char(letter));
        // 挪一格试两个方向：取值停在环的末一格上时（这一景的缩放方式与抖动）`j` 挪不动，
        // 得往回挪才落到另一格上。两个方向都定不下来，才是「一个改动都定不下」。
        let settles = |stage: Stage, item: Item, mover: char| -> bool {
            let scene = Scene::named("config");
            let mut session = scene.session.clone();
            session.set_stage(stage);
            let before = (session.device.clone(), session.taste.clone());
            session.views.config.cursor = item;
            session.views.config.pane = super::super::view::Pane::Settings;
            let now = scene.now();
            // 进详情栏、挪一格、定下来（型号那一项多一层下钻，因此按两下）；
            // 自由填的那几项另按一次 `i` 试着改。
            for input in [key('l'), key(mover), key('l'), key('l'), key('i')] {
                if let Some(deed) = session.deed_of(input, phase_of(stage), now) {
                    session.perform(deed, now);
                }
            }
            (session.device.clone(), session.taste.clone()) != before
                || session.views.input.is_some()
        };
        let settled = |stage: Stage| -> Vec<Item> {
            items()
                .into_iter()
                .filter(|item| settles(stage, *item, 'j') || settles(stage, *item, 'k'))
                .collect()
        };
        let settable: Vec<Item> = items()
            .into_iter()
            .filter(|item| matches!(item, Item::Setting(_)))
            .collect();
        assert_eq!(
            settled(Stage::Fresh),
            settable,
            "还没开始那一档该动的没全动"
        );
        let running = settled(Stage::Running(tonefit::Instruction::Continue));
        assert!(running.is_empty(), "跑着时这几项还是定下来了：{running:?}");
        let deciding = settled(Stage::Deciding(tonefit::Instruction::Continue));
        assert!(
            deciding.is_empty(),
            "等待确认时这几项还是定下来了：{deciding:?}"
        );
    }

    /// 这一档阶段在按键表上是哪一档。这几条用例问的是**阶段那一维**，
    /// 清点中与转换中在设置改不改得动上是同一件事，取后者。
    fn phase_of(stage: Stage) -> crate::session::keymap::Phase {
        use crate::session::keymap::Phase;
        match stage {
            Stage::Fresh => Phase::Fresh,
            Stage::Running(_) => Phase::Running,
            Stage::Deciding(_) => Phase::Deciding,
            Stage::Ended => Phase::Ended,
        }
    }

    /// **一份预设记得下的是设置栏那两组里除型号之外的那十四项**
    /// （`session-redesign/14`）：屏上行尾那个 `*`、顶上那句「改动了 N 项」、
    /// 预设栏那句「包含 N 项设置」、存出去的那一份，读的都是这一份单子。
    ///
    /// 断的**不是** `stored_fields().len() == 14`（那个数从两张单子上现数出来，
    /// 写死它等于抄第二份）：断的是**型号不在里面，而另外两组一项不落都在**，
    /// 以及那份单子与[那一句](stored)对得上（一份单子、一句判据，不许各说各的）。
    #[test]
    fn a_preset_records_every_setting_of_the_two_bands_but_the_model() {
        let said = stored_fields();
        assert!(!said.contains(&Field::Profile), "型号不在里面");
        for field in DEVICE_FIELDS.into_iter().chain(TASTE_FIELDS) {
            assert_eq!(
                said.contains(&field),
                field != Field::Profile,
                "{} 在不在这份单子上错了",
                field.label()
            );
            assert_eq!(said.contains(&field), stored(field), "单子与那一句对不上");
        }
        // 屏上行尾那个 `*` 与这份单子同一处出处：型号一辈子不带 `*`。
        let scene = Scene::named("config");
        assert!(!starred(&scene.session, Field::Profile));
    }

    /// **一份预设说了哪几项**（预设栏那一行写的「包含 N 项设置：…」）：
    /// 「没说」与「说了一个恰好等于默认值的值」是两件事，判它的是 `Session::unsaid` 一处。
    ///
    /// 拿的是 `config` 那一景盘上真有的那两份：「漫画」一项都没说，「画集」说了四项。
    #[test]
    fn a_preset_says_only_the_settings_it_was_saved_with() {
        let scene = Scene::named("config");
        let nothing = scene.presets.read("漫画").expect("读得出「漫画」");
        assert!(said_fields(&scene.session, &nothing).is_empty());
        let four = scene.presets.read("画集").expect("读得出「画集」");
        let names: Vec<&str> = said_fields(&scene.session, &four)
            .iter()
            .map(|field| field.label())
            .collect();
        assert_eq!(names, ["缩放方式", "裁白边", "拆分跨页", "灰阶档位"]);
    }
}
