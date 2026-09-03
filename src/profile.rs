//! Profile 与面板表。
//!
//! 面板是物理显示面板：分辨率 + PPI + 灰阶数 + 彩色，四项俱全才是 profile 的主键（`CONTEXT.md`）。
//! 设备只是面板的别名，多对一——表里面板是少数几个常量，型号一行一个。
//!
//! 阈值档位属于 profile、不属于面板：面板是物理事实，档位是标定出来的（ADR 0003）。
//! 同一面板可以有多个 profile，所以 profile 不是面板的同义词。

use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};

use crate::geometry::Size;
use crate::metric::Score;

/// 物理显示面板。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    /// 面板分辨率。目标尺寸由它 fit-inside 算出，两者不是一回事（`CONTEXT.md`）。
    pub resolution: Size,
    /// 每英寸像素数。判据的低通核由它推出（ADR 0002）。
    pub ppi: u32,
    /// 面板物理能显示的灰度级数。位深的硬上界（ADR 0003）。
    ///
    /// 彩色面板上它说的仍是黑白那一层：Kaleido 是黑白面板加一层彩色滤光片，
    /// 灰度页在它上面与在纯黑白面板上走同一条路。
    pub gray_levels: u32,
    /// 这块面板显不显示彩色。
    ///
    /// 彩页在彩色面板上走彩色分支、保留颜色，在黑白面板上转灰
    /// （ADR 0010：彩页按 profile 分流；ADR 0005 决定第 4 条）。
    /// 它在主键里：同分辨率同 PPI 的 Kaleido 与纯黑白 e-ink 输出不同，不是同一块面板。
    pub color: bool,
}

impl std::fmt::Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} · {} PPI · {} 级灰阶 · {}",
            self.resolution,
            self.ppi,
            self.gray_levels,
            if self.color { "彩色" } else { "黑白" }
        )
    }
}

/// 判据的可接受上限（`CONTEXT.md`：**判据是量，阈值是界**）。
///
/// 与判据同一把尺，单位是 8 位灰度级。跟着判据一起不可跨面板比较（ADR 0002）。
///
/// 数值从哪来，`Display` 一并写在数值旁边——报告与文档都得说出来（spec 的 Further Notes）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threshold {
    value: f32,
    source: ThresholdSource,
}

/// 一个阈值是怎么定出来的。判定只看数值，这一项只进 `Display`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThresholdSource {
    /// 内置值，由真实素材上的人工盲测定出（见 measurements 的《位深盲测》）。
    Calibrated,
    /// 用户**点名**的界。他在自己那台设备上数出来的那个数走这一条。
    ///
    /// 这一项分的是「这个数怎么定出来的」，不是「从哪个入口点的名」（`p1-session/12` 判的），
    /// `Display` 因此**不提入口**——入口有哪几个，见 [`Profile::with_threshold`]。
    Pinned,
}

impl Threshold {
    /// 这个判据值在界以内吗。
    pub fn admits(self, score: Score) -> bool {
        score.value() <= self.value
    }

    /// 这个判据值是不是**远在**界外——超出界的 `factor` 倍。
    ///
    /// 离群页判据要的就是它（ADR 0006 决定第 5 条：离群页不参与上包络，单独定档）：
    /// 不是刚过线，是远在界外。倍数由调用方给——那是个未标定的占位值，
    /// 属于上包络那一层（见 `crate::envelope`），不属于界。
    pub(crate) fn far_outside(self, score: Score, factor: f32) -> bool {
        score.value() > self.value * factor
    }

    /// 界的数值，8 位灰度级。
    ///
    /// 判定只经 [`admits`](Self::admits)。读出数值是给渲染与测试用的——用它相对地造判据值，
    /// 那些用例就不必抄下当前这个数字，标定把数字换掉时也不用跟着改。
    pub fn value(self) -> f32 {
        self.value
    }

    /// 这个界是怎么定出来的。
    pub fn source(self) -> ThresholdSource {
        self.source
    }
}

impl std::fmt::Display for Threshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = match self.source {
            ThresholdSource::Calibrated => "盲测标定于 boox-poke6，其余面板未复核",
            ThresholdSource::Pinned => "点名指定",
        };
        write!(f, "阈值 {:.3}（{source}）", self.value)
    }
}

/// 内置的界，由真实素材上的人工盲测定出。
///
/// **标定只在 boox-poke6 上做过**：真机、逐组全排序，夹出窗口 [4.930, 6.022)——
/// 下界是被判可接受的最高判据值（画集 040 的 2bit+FS），上界是被判不可接受的最低判据值
/// （画集 056 的 4bit 不抖，目视明显色块）。**上界由 banding 定，不由 1bit 定**：
/// 判据补上颗粒项之后 1bit+FS 退到 21 以上，够不着界。
/// 5.5 落在窗口里，且在真实语料上不把任何一卷虚抬一档——界降到 5.0 才开始虚抬
/// （见 measurements 的《位深盲测》的卷级扫描）。
///
/// 判据跟着面板走、不可跨面板比较（ADR 0002），而这里是一个常数——**其余面板沿用这个数，
/// 没有复核**。`Display` 把这句话写在数值旁边，[`Profile::with_threshold`] 是出口。
///
/// **它仍然挡掉 2bit 不抖**——那一档真机排第 2、比 2bit+FS 省 18% 体积，而判据在棋魂两页上
/// 读它 8.250 与 18.878。那不是界的问题，是判据认为那两页的灰调塌陷值这么多；
/// 两侧都只有一份数据，复核之前判据说了算（见 `CONTEXT.md` 的《尚未确立》）。
///
/// **「候选上界恒过关」这条不再无条件成立。** 8.5 是 4bit 量化步长的一半，保证 4bit 恒在界内；
/// 5.5 不保证。几何门成立时上界是 4bit+FS，实测最大 1.770，远在界内；
/// 门不成立时上界降为 4bit 不抖，实测 66 页里有 2 页超过 5.5（最大 6.994），
/// 那两页走 [`crate::decide`] 的兜底。判定因此整体偏向更高的位深，文件更大。
const DEFAULT_THRESHOLD: Threshold = Threshold {
    value: 5.5,
    source: ThresholdSource::Calibrated,
};

/// 一次处理调用的目标设备。
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    device: &'static str,
    panel: Panel,
    threshold: Threshold,
}

impl Profile {
    /// 按型号名解析。大小写与分隔符先归一，`Kobo Libra 2` 与 `kobo-libra-2` 是同一个型号。
    pub fn resolve(device: &str) -> Result<Self> {
        let key = canonical(device);
        match DEVICES.iter().find(|(name, _)| *name == key) {
            Some((name, panel)) => Ok(Self {
                device: name,
                panel: *panel,
                threshold: DEFAULT_THRESHOLD,
            }),
            None => Err(unknown_device_error(device)),
        }
    }

    /// 覆盖面板灰阶数（`--gray-levels`）。
    ///
    /// 填的是在真机上数出来的实际可分辨级数，未收录的面板与实测修正都走这一条（ADR 0003）。
    /// 数出来的级数不必是 2 的幂：按它裁剪候选位深是 06 号票的事，这里只管记下。
    /// 界在 2 与 256 之间——低于 2 级的面板什么都显示不出，而阶梯图本身是 8 位灰度，
    /// 数不出 256 级以上。
    pub fn with_gray_levels(mut self, gray_levels: u32) -> Result<Self> {
        if !(2..=256).contains(&gray_levels) {
            bail!("灰阶数 {gray_levels} 数不出来：取值在 2 与 256 之间（e-ink 恒 16）");
        }
        self.panel.gray_levels = gray_levels;
        Ok(self)
    }

    /// 覆盖判定用的界——**点名那一种唯一的入口**。
    ///
    /// 点得动它的地方有三处：命令行 `--threshold`、预设的设备层、会话里那一行。
    /// 三处都经这里，出来的都是 [`ThresholdSource::Pinned`]（为什么是一种而不是三种，
    /// 见那个变体的文档）。
    ///
    /// 判据跟着面板走、不可跨面板比较（ADR 0002），而内置值是一个常数（见 [`DEFAULT_THRESHOLD`]）。
    /// 在自己那台设备上盲测出来的界走这一条：把同一页的各档输出拷进设备，
    /// 记下最低的那个**不可接受**档的判据值，界取在它之下。判据值由 `--dry-run` 逐页给出。
    ///
    /// 单位与判据同为 8 位灰度级，因此界落在 0 与 255 之间；取大了各档全部过关，界就不成其为界。
    pub fn with_threshold(mut self, threshold: f32) -> Result<Self> {
        if !(threshold.is_finite() && threshold > 0.0 && threshold <= 255.0) {
            bail!("阈值 {threshold} 不是一个界：取值在 0 与 255 之间，与判据同为 8 位灰度级");
        }
        self.threshold = Threshold {
            value: threshold,
            source: ThresholdSource::Pinned,
        };
        Ok(self)
    }

    /// 内置表里的全部型号规范名，按表里的次序。
    ///
    /// **给会话挑型号用的**（`p1-session/08`）：命令行上用户自己敲名字，认不出时
    /// [`resolve`](Self::resolve) 的错误把清单端出来；而会话里没有那条错误——
    /// 光标停在型号那一行、左右键换一个，那就要一份**枚举得出来**的清单。
    ///
    /// 它不是新 seam，是既有类型上的一个访问器：清单本来就从 `resolve` 的错误里出得来，
    /// 这里只是不必先制造一次失败。两处同一张内置表，次序也同一个：
    /// 按厂商分组、组内从小屏到大屏。
    pub fn devices() -> impl Iterator<Item = &'static str> {
        DEVICES.iter().map(|(device, _)| *device)
    }

    /// 解析到的型号规范名。
    pub fn device(&self) -> &'static str {
        self.device
    }

    /// 本次使用的面板。
    pub fn panel(&self) -> Panel {
        self.panel
    }

    /// 本次位深判定用的阈值。
    pub fn threshold(&self) -> Threshold {
        self.threshold
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}：{} · {}", self.device, self.panel, self.threshold)
    }
}

/// 未知型号的说法：把内置清单按面板分组端出来，再给出表外设备的兜底办法。
///
/// 面板相同的型号输出完全一致，所以「挑一个面板相同的型号顶上」是可行的一步；
/// 灰阶数是唯一挑不出来的那项，走 `--gray-levels`（ADR 0003）。
fn unknown_device_error(device: &str) -> anyhow::Error {
    let mut text = format!("未知型号「{device}」。内置型号按面板分组：\n");
    for (panel, devices) in devices_by_panel() {
        writeln!(text, "  {panel}\n    {}", devices.join(" ")).expect("写进 String 不会失败");
    }
    text.push_str("设备不在表里：挑一个面板相同的型号，再按实测用 --gray-levels 覆盖灰阶数。");
    anyhow!(text)
}

/// 内置清单按面板归并。面板按分辨率从小到大排，同分辨率再按 PPI、再按彩色——替身是照分辨率挑的。
fn devices_by_panel() -> Vec<(Panel, Vec<&'static str>)> {
    let mut groups: Vec<(Panel, Vec<&'static str>)> = Vec::new();
    for (device, panel) in DEVICES {
        match groups.iter_mut().find(|(seen, _)| seen == panel) {
            Some((_, devices)) => devices.push(device),
            None => groups.push((*panel, vec![device])),
        }
    }
    groups.sort_by_key(|(panel, _)| {
        (
            panel.resolution.height,
            panel.resolution.width,
            panel.ppi,
            panel.color,
        )
    });
    groups
}

/// 型号名归一：全小写、连字符分隔。用户怎么敲都落到表里的规范名上。
fn canonical(device: &str) -> String {
    let mut key = String::with_capacity(device.len());
    for character in device.chars() {
        if character.is_ascii_alphanumeric() {
            key.push(character.to_ascii_lowercase());
        } else if !key.ends_with('-') {
            key.push('-');
        }
    }
    key.trim_matches('-').to_owned()
}

/// e-ink 面板的灰阶数恒为 16（`CONTEXT.md`）。
const EINK_GRAY_LEVELS: u32 = 16;

/// 一块纯黑白 e-ink 面板。
const fn eink(width: u32, height: u32, ppi: u32) -> Panel {
    Panel {
        resolution: Size::new(width, height),
        ppi,
        gray_levels: EINK_GRAY_LEVELS,
        color: false,
    }
}

/// 一块 Kaleido 彩色 e-ink 面板：同一块黑白面板加一层彩色滤光片。
///
/// 分辨率与 PPI 填的是**黑白那一层**的规格——面板的主键、目标尺寸与低通核都由它推出。
/// 彩色滤光片让彩色内容的实际分辨率降到黑白层之下（各家口径是一半），
/// 但 P0 的彩色分支只做缩放、不做判据，这个折减到不了任何判定上；
/// 真要利用它得先测（`CONTEXT.md` 的《尚未确立》）。
const fn kaleido(width: u32, height: u32, ppi: u32) -> Panel {
    Panel {
        resolution: Size::new(width, height),
        ppi,
        gray_levels: EINK_GRAY_LEVELS,
        color: true,
    }
}

// 面板按「分辨率 + PPI」命名，不按型号：设备是面板的别名，反过来不成立。
// 分辨率相同而 PPI 不同的是两块面板——低通核由 PPI 推出，它们不能共用一份 profile。
const EINK_824X1648_300: Panel = eink(824, 1648, 300);
const EINK_1072X1448_300: Panel = eink(1072, 1448, 300);
const EINK_1236X1648_300: Panel = eink(1236, 1648, 300);
const EINK_1264X1680_300: Panel = eink(1264, 1680, 300);
const EINK_1404X1872_227: Panel = eink(1404, 1872, 227);
const EINK_1404X1872_300: Panel = eink(1404, 1872, 300);
const EINK_1440X1920_300: Panel = eink(1440, 1920, 300);
const EINK_1650X2200_207: Panel = eink(1650, 2200, 207);
const EINK_1860X2480_300: Panel = eink(1860, 2480, 300);
const KALEIDO_1072X1448_300: Panel = kaleido(1072, 1448, 300);
const KALEIDO_1264X1680_300: Panel = kaleido(1264, 1680, 300);
const KALEIDO_1404X1872_227: Panel = kaleido(1404, 1872, 227);

/// 型号 → 面板，多对一。新增型号只需在这里加一行。
///
/// 黑白与 Kaleido 彩色两类 e-ink 型号都收。彩色那几个的面板与同代黑白型号同分辨率同 PPI，
/// 区分它们的是彩色那一项——它在主键里，两者不是同一块面板（ADR 0010：彩页按 profile 分流）。
/// 名字一律小写连字符，解析前会把用户敲的形式归一到这里。
///
/// 同一条产品线换代换过面板的（Paperwhite、Oasis），型号名必须带代次：
/// 少写一位就会静默给出错的目标尺寸，而那正是这张表要挡掉的事。
///
/// **拼写跟各家自己的产品名走**，不统一：Kobo 写作 Libra/Clara **Colour**，
/// BOOX 写作 Go **Color** 7，Amazon 写作 **Color**soft。[`canonical`] 归一的是大小写与分隔符，
/// 不管拼写——把 `colour` 改成 `color`，用户敲真实产品名就解析不出来。
///
/// 分辨率与 PPI 取各家公布的规格，不是实测——实测数字见 measurements 的《B 类素材普查》，
/// 那一节的三块面板都在本表内。
const DEVICES: &[(&str, Panel)] = &[
    // Kobo
    ("kobo-clara-hd", EINK_1072X1448_300),
    ("kobo-clara-2e", EINK_1072X1448_300),
    ("kobo-clara-bw", EINK_1072X1448_300),
    ("kobo-libra-h2o", EINK_1264X1680_300),
    ("kobo-libra-2", EINK_1264X1680_300),
    ("kobo-forma", EINK_1440X1920_300),
    ("kobo-sage", EINK_1440X1920_300),
    ("kobo-aura-one", EINK_1404X1872_300),
    ("kobo-elipsa", EINK_1404X1872_227),
    ("kobo-elipsa-2e", EINK_1404X1872_227),
    ("kobo-clara-colour", KALEIDO_1072X1448_300),
    ("kobo-libra-colour", KALEIDO_1264X1680_300),
    // BOOX
    ("boox-palma", EINK_824X1648_300),
    ("boox-palma-2", EINK_824X1648_300),
    ("boox-poke5", EINK_1072X1448_300),
    ("boox-poke6", EINK_1072X1448_300),
    ("boox-leaf2", EINK_1264X1680_300),
    ("boox-page", EINK_1264X1680_300),
    ("boox-note-air3", EINK_1404X1872_227),
    ("boox-tab-ultra", EINK_1404X1872_227),
    ("boox-go-10-3", EINK_1404X1872_227),
    ("boox-max-lumi2", EINK_1650X2200_207),
    ("boox-tab-x", EINK_1650X2200_207),
    ("boox-go-color-7", KALEIDO_1264X1680_300),
    ("boox-note-air3-c", KALEIDO_1404X1872_227),
    ("boox-tab-ultra-c", KALEIDO_1404X1872_227),
    // Kindle
    ("kindle-voyage", EINK_1072X1448_300),
    ("kindle-11", EINK_1072X1448_300),
    ("kindle-oasis-2", EINK_1264X1680_300),
    ("kindle-oasis-3", EINK_1264X1680_300),
    ("kindle-paperwhite-11", EINK_1236X1648_300),
    ("kindle-paperwhite-12", EINK_1264X1680_300),
    ("kindle-scribe", EINK_1860X2480_300),
    ("kindle-colorsoft", KALEIDO_1264X1680_300),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// 表的完整性：名字不重复、已是规范名、解析出来的面板就是表里写的那块。
    #[test]
    fn every_listed_model_resolves_to_the_panel_it_is_listed_with() {
        let mut seen: Vec<&str> = Vec::new();
        for (device, panel) in DEVICES {
            assert!(!seen.contains(device), "型号 {device} 在表里出现了两次");
            seen.push(device);
            let profile =
                Profile::resolve(device).unwrap_or_else(|error| panic!("{device}：{error}"));
            assert_eq!(profile.device(), *device, "表里的名字必须已经是规范名");
            assert_eq!(profile.panel(), *panel);
            // 内置表收的全是 e-ink，黑白与 Kaleido 都是——Kaleido 的黑白那一层同样 16 级。
            // 哪天进了 LCD 面板，它得连同「未经标定」的标注一起落地（ADR 0003），
            // 这条断言随之改写。
            assert_eq!(
                panel.gray_levels, EINK_GRAY_LEVELS,
                "{device} 不是 e-ink 面板"
            );
        }
    }

    /// 表的形状是「少量面板 + 型号别名」：新增型号只加一行别名，不新增面板。
    #[test]
    fn the_table_is_a_few_panels_with_many_aliases() {
        let panels = devices_by_panel();
        assert!(
            DEVICES.len() >= panels.len() * 2,
            "{} 个型号只归并出 {} 块面板，别名表退化成了设备清单",
            DEVICES.len(),
            panels.len()
        );
    }

    /// 彩色面板与黑白面板是**两块**面板：彩色那一项进主键，因此同分辨率同 PPI 的
    /// Kaleido 与纯黑白 e-ink 各占一行（ADR 0010：彩页按 profile 分流）。
    #[test]
    fn a_kaleido_model_resolves_to_a_color_panel() {
        let color = Profile::resolve("kobo-libra-colour").expect("内置型号");
        let monochrome = Profile::resolve("kobo-libra-2").expect("内置型号");

        assert!(color.panel().color, "Kaleido 面板显示彩色");
        assert!(!monochrome.panel().color, "纯黑白 e-ink 面板不显示彩色");
        // 两者的分辨率与 PPI 相同，区分它们的只有彩色那一项——它必须在主键里。
        assert_eq!(color.panel().resolution, monochrome.panel().resolution);
        assert_eq!(color.panel().ppi, monochrome.panel().ppi);
        assert_ne!(color.panel(), monochrome.panel());
    }

    /// 枚举出来的清单与那条错误里的清单是**同一份**，每一项都解析得回它自己。
    ///
    /// 会话挑型号靠前者、命令行报错靠后者（见 [`Profile::devices`]）：
    /// 两处走散的话，屏上挑得到的型号会有敲不出来的，或者反过来。
    #[test]
    fn the_enumerated_models_are_the_ones_the_table_lists() {
        let listed: Vec<&str> = Profile::devices().collect();

        assert_eq!(listed.len(), DEVICES.len());
        let message = unknown_device_error("没这个型号").to_string();
        for device in listed {
            assert_eq!(
                Profile::resolve(device)
                    .expect("枚举出来的就是规范名")
                    .device(),
                device
            );
            assert!(message.contains(device), "错误里少了 {device}");
        }
    }

    /// 清单要全：用户挑替身是从这段文字里挑的，漏掉的型号等于不存在。
    #[test]
    fn the_unknown_model_error_lists_every_model_in_the_table() {
        let message = unknown_device_error("没这个型号").to_string();
        for (device, _) in DEVICES {
            assert!(message.contains(device), "清单里少了 {device}：{message}");
        }
    }
}
