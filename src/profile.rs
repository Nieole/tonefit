//! Profile 与面板表。
//!
//! 面板是物理显示面板：分辨率 + PPI + 灰阶数，三项俱全才是 profile 的主键（`CONTEXT.md`）。
//! 设备只是面板的别名，多对一——表里面板是少数几个常量，型号一行一个。
//!
//! 阈值档位属于 profile、不属于面板，它随位深判定落地（06 号票）；此刻 profile 只有面板。
//! 同一面板可以有多个 profile，所以 profile 不是面板的同义词。

use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};

use crate::geometry::Size;

/// 物理显示面板。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    /// 面板分辨率。目标尺寸由它 fit-inside 算出，两者不是一回事（`CONTEXT.md`）。
    pub resolution: Size,
    /// 每英寸像素数。判据的低通核由它推出（ADR 0002）。
    pub ppi: u32,
    /// 面板物理能显示的灰度级数。位深的硬上界（ADR 0003）。
    pub gray_levels: u32,
}

impl std::fmt::Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} · {} PPI · {} 级灰阶",
            self.resolution, self.ppi, self.gray_levels
        )
    }
}

/// 一次处理调用的目标设备。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    device: &'static str,
    panel: Panel,
}

impl Profile {
    /// 按型号名解析。大小写与分隔符先归一，`Kobo Libra 2` 与 `kobo-libra-2` 是同一个型号。
    pub fn resolve(device: &str) -> Result<Self> {
        let key = canonical(device);
        match DEVICES.iter().find(|(name, _)| *name == key) {
            Some((name, panel)) => Ok(Self {
                device: name,
                panel: *panel,
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

    /// 解析到的型号规范名。
    pub fn device(&self) -> &'static str {
        self.device
    }

    /// 本次使用的面板。
    pub fn panel(&self) -> Panel {
        self.panel
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}：{}", self.device, self.panel)
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

/// 内置清单按面板归并。面板按分辨率从小到大排，同分辨率再按 PPI——替身是照分辨率挑的。
fn devices_by_panel() -> Vec<(Panel, Vec<&'static str>)> {
    let mut groups: Vec<(Panel, Vec<&'static str>)> = Vec::new();
    for (device, panel) in DEVICES {
        match groups.iter_mut().find(|(seen, _)| seen == panel) {
            Some((_, devices)) => devices.push(device),
            None => groups.push((*panel, vec![device])),
        }
    }
    groups.sort_by_key(|(panel, _)| (panel.resolution.height, panel.resolution.width, panel.ppi));
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

/// 一块 e-ink 面板。
const fn eink(width: u32, height: u32, ppi: u32) -> Panel {
    Panel {
        resolution: Size::new(width, height),
        ppi,
        gray_levels: EINK_GRAY_LEVELS,
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

/// 型号 → 面板，多对一。新增型号只需在这里加一行。
///
/// 只收黑白 e-ink 型号：Kaleido 彩色面板要走自己的分支，P0 不在范围内。
/// 名字一律小写连字符，解析前会把用户敲的形式归一到这里。
///
/// 同一条产品线换代换过面板的（Paperwhite、Oasis），型号名必须带代次：
/// 少写一位就会静默给出错的目标尺寸，而那正是这张表要挡掉的事。
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
    // BOOX
    ("boox-palma", EINK_824X1648_300),
    ("boox-palma-2", EINK_824X1648_300),
    ("boox-poke5", EINK_1072X1448_300),
    ("boox-leaf2", EINK_1264X1680_300),
    ("boox-page", EINK_1264X1680_300),
    ("boox-note-air3", EINK_1404X1872_227),
    ("boox-tab-ultra", EINK_1404X1872_227),
    ("boox-go-10-3", EINK_1404X1872_227),
    ("boox-max-lumi2", EINK_1650X2200_207),
    ("boox-tab-x", EINK_1650X2200_207),
    // Kindle
    ("kindle-voyage", EINK_1072X1448_300),
    ("kindle-11", EINK_1072X1448_300),
    ("kindle-oasis-2", EINK_1264X1680_300),
    ("kindle-oasis-3", EINK_1264X1680_300),
    ("kindle-paperwhite-11", EINK_1236X1648_300),
    ("kindle-paperwhite-12", EINK_1264X1680_300),
    ("kindle-scribe", EINK_1860X2480_300),
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
            // 内置表当前只收黑白 e-ink。哪天进了 LCD 面板，它得连同「未经标定」的标注
            // 一起落地（ADR 0003），这条断言随之改写。
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

    /// 清单要全：用户挑替身是从这段文字里挑的，漏掉的型号等于不存在。
    #[test]
    fn the_unknown_model_error_lists_every_model_in_the_table() {
        let message = unknown_device_error("没这个型号").to_string();
        for (device, _) in DEVICES {
            assert!(message.contains(device), "清单里少了 {device}：{message}");
        }
    }
}
