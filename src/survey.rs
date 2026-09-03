//! 预扫：开工之前把点名的卷全枚举一遍，算出这一趟的**全局总步数**（ADR 0011 决定第 3 条）。
//!
//! 一个卷要走多少步，得先枚举它的成员才知道，而枚举原先发生在处理那一卷的时候——
//! 「整趟还要多久」因此在开工时无从算起，屏上那条横条只说得出当前这一卷走到哪儿了。
//! 几十卷跑下来唯一有人真想知道的数是**整趟**还剩多久，本模块把枚举提到开工之前，
//! 就为了给出那个数。
//!
//! 只列成员，**不碰像素**：目录卷走一遍目录，归档卷读中央目录，两样都是 [`source::open`]
//! 本来就要做的事，一个像素都不解。数完就**把卷放掉**——[`Surveyed`] 里只剩这一卷的路径
//! 与几个数，处理那一卷时按那个路径再开一次。
//! 扫两遍买的不是省事，是**不攥着几千个句柄**：卷留在预扫手上的那一版里，点名 N 个归档卷
//! 就整趟同时开着 N 个文件、常驻 N 份中央目录，那笔开销随**点名的卷数**长，
//! 成百上千个卷就把一趟胀死。买下来之后一趟同时开着几个，见 `source::Reader` 的
//! 《一趟同时开着几个句柄》——本模块是那笔账里「不是点名的卷数」这一句的来处。
//!
//! 它顺带把「一卷点不开」提前到一页都没做之前：这里发现的坏路径**整趟拒绝**、逐条列出
//! （见 [`refuse`]）。理由与「处理范围为空是错误」同一条（ADR 0009 的《不要做的「简化」》）——
//! 范围层错了可能写到别人的目录里，而那一趟已经写出去的卷收不回来。
//! 预扫**之后**才出的卷级失败不走这条路：那时其余卷照做，报告照出
//! （`CONTEXT.md` 的《失败》：卷级失败）。
//!
//! **不落盘。** 预扫的作用域是这一趟点名的卷，活在一次运行之内。把成员表存下来下趟再用
//! 就是一份全库索引，而那正是 ADR 0009 关掉的东西。
//!
//! **要认的代价**：归档卷的中央目录读两遍，目录卷走两遍目录。这一笔随**一个卷**长，
//! 不随点名的卷数长，而且只在轮到那一卷时付、付完就还回去——正是上面那一笔换来的。
//! 两遍之间源变了的话，做的与报的都是**重开的那一份**：预扫这一遍只留下步数
//! （见 [`Surveyed::steps`]），成员数在处理那一卷时按重开的卷重新数一遍。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::source;
use crate::{MemberCounts, Request, volume_steps};

/// 这一趟预扫出来的东西：点名的每一个卷一份，外加它们步数之和。
pub(crate) struct Survey {
    volumes: Vec<Surveyed>,
    /// 各卷步数之和。这个数由 [`Surveyed::steps`] **加**出来，不是另算一遍——
    /// 「全局总步数等于各卷步数之和」因此是构造出来的，不是一条要靠人守住的约定。
    steps: u64,
}

/// 预扫过的一个卷：**只有数与路径**，没有卷本身。
///
/// 它是那个卷的一份摘要，不是那个卷：预扫数完就把卷放掉，处理那一卷时按
/// [`root`](Self::root) 再开一次（见本模块的模块文档）。
pub(crate) struct Surveyed {
    /// 卷根，也就是点名的那个路径（见 [`source::open`]）。处理这一卷时按它再开一次。
    pub(crate) root: PathBuf,
    /// 这一卷这一趟最多走多少步。开卷那条事件报的就是它。
    ///
    /// 它算在**预扫这一遍**数出来的成员上。重开之后成员数可能与它对不上（两遍之间源变了），
    /// 那时报的步数仍是这一个：它已经加进开工那条事件报出去的全局总步数里，
    /// 而预告的步数本来就是上界（见 `crate::volume_steps`）。
    pub(crate) steps: u64,
    /// **预扫这一遍**枚举这一卷花了多久。
    ///
    /// 它进这一卷的 [`VolumeTiming::elapsed`](crate::VolumeTiming::elapsed)——那个数的定义是
    /// 「从打开卷到这份卷报告成型」，而预扫这一遍也是打开卷。不带上它，慢盘上那一截会
    /// 静默消失，而 `outside_the_segments` 的文档正指着它说「少掉的那一截恰恰是枚举」。
    ///
    /// 重开那一遍**不在这里**：它发生在处理那一卷之内，本来就落在那一卷的墙钟里
    /// （见 `crate::process_volume`）。那一格因此装着两遍枚举——一遍由这里加进去，
    /// 一遍自己就在里面。
    pub(crate) enumerating: Duration,
}

impl Survey {
    /// 预扫点名的那些卷。一个都点不开的话整趟当场拒绝，一条事件都不发。
    pub(crate) fn of(request: &Request) -> Result<Self> {
        let mut volumes = Vec::with_capacity(request.inputs.len());
        // 坏路径**收齐了再报**，不是撞上第一个就返回：点名十个卷、其中三个路径写错了，
        // 一次说清三个才改得完一遍，逐个报要来回三趟。
        let mut refused = Vec::new();
        for input in &request.inputs {
            let started = Instant::now();
            match source::open(input) {
                Ok(volume) => {
                    let enumerating = started.elapsed();
                    volumes.push(Surveyed {
                        steps: volume_steps(MemberCounts::of(&volume, request), request),
                        root: volume.root,
                        enumerating,
                    });
                    // 卷在这一格的末尾**放掉**：归档卷那个 `ZipArchive` 连同它的文件句柄
                    // 跟着析构，预扫因此不随点名的卷数攥住句柄（见本模块的模块文档）。
                    // 上面那一句只留下了它的路径。
                }
                Err(error) => refused.push((input.as_path(), error)),
            }
        }
        if !refused.is_empty() {
            return Err(refuse(&refused, request.inputs.len()));
        }
        Ok(Self {
            steps: volumes.iter().map(|surveyed| surveyed.steps).sum(),
            volumes,
        })
    }

    /// 这一趟最多走多少步。开工那条事件报的就是它。
    pub(crate) fn steps(&self) -> u64 {
        self.steps
    }

    /// 按点名顺序交出预扫过的那些卷。
    pub(crate) fn into_volumes(self) -> Vec<Surveyed> {
        self.volumes
    }
}

/// 点不开的那几条路径**逐条列出**，整趟拒绝。
///
/// 形状照开工前那道撞名校验办（见 `crate::ensure_no_two_volumes_share_an_output`）：
/// 先说有几个、总共点名了几个，再逐条缩进列出，多了只列前几条并说还有多少。
/// 一屏放不下的清单等于没有清单。
///
/// 每条占两行——路径一行，错误链一行。不拼成一行是因为两者不一定互相包含：
/// 「X 不存在」自己带着路径，「读不出归档结构」也带，而将来多一种错法未必带。
fn refuse(refused: &[(&Path, anyhow::Error)], named: usize) -> anyhow::Error {
    /// 最多列几条。
    const SHOWN: usize = 5;

    let mut said = format!(
        "点名的 {named} 个卷里有 {} 个点不开，整趟不做。点不开的是：\n",
        refused.len()
    );
    for (input, error) in refused.iter().take(SHOWN) {
        said.push_str(&format!("  {}\n    {error:#}\n", input.display()));
    }
    if refused.len() > SHOWN {
        said.push_str(&format!("  ……另有 {} 个\n", refused.len() - SHOWN));
    }
    said.push_str(
        "整趟拒绝的理由与「处理范围为空是错误」同一条：范围层错了可能写到别人的目录里\
         （ADR 0009）。因此一页都没做，输出根下一个文件都没有——把点名的路径改对再重跑。",
    );
    anyhow!(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 坏路径逐条列出，且说得出总共点名了几个。
    ///
    /// 只报第一条的话，点名十个卷、写错三个路径的人要来回改三趟——而这三条本来一次就说得完。
    #[test]
    fn every_path_that_cannot_be_opened_is_named() {
        let refused = [
            (Path::new("库/第1话"), anyhow!("库/第1话 不存在")),
            (Path::new("库/第2话.cbz"), anyhow!("读不出归档结构")),
        ];
        let said = format!("{:#}", refuse(&refused, 5));

        for named in ["库/第1话", "库/第2话.cbz", "不存在", "读不出归档结构"] {
            assert!(said.contains(named), "拒绝的那句话里没有 {named}：{said}");
        }
        assert!(said.contains('5'), "没说总共点名了几个：{said}");
    }

    /// 坏路径多到一屏放不下时只列前几条，剩下的说个数。
    #[test]
    fn a_long_list_of_bad_paths_is_cut_short() {
        let paths: Vec<std::path::PathBuf> = (0..9)
            .map(|n| std::path::PathBuf::from(format!("库/第{n}话")))
            .collect();
        let refused: Vec<(&Path, anyhow::Error)> = paths
            .iter()
            .map(|path| (path.as_path(), anyhow!("{} 不存在", path.display())))
            .collect();
        let said = format!("{:#}", refuse(&refused, 9));

        assert!(said.contains("库/第0话"), "头一条没列出来：{said}");
        assert!(!said.contains("库/第8话"), "第九条也列出来了：{said}");
        assert!(said.contains("另有 4 个"), "没说还剩多少条：{said}");
    }

    /// 点名的归档卷数。「成百上千个卷」正是本票要拆掉的那个上限，用例照几百这一档造。
    const MANY: usize = 300;

    /// 一个**空归档**：ZIP 的中央目录结束记录那 22 个字节，一个成员都没有。
    ///
    /// 手搓而不是拿 `zip` 的写入端拼：这一条要几百个归档卷，而它问的只有
    /// 「预扫开完还攥不攥着它们」——成员一个都不需要，页更不需要。
    const EMPTY_ARCHIVE: [u8; 22] = [
        b'P', b'K', 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    /// 这个进程此刻开着的、落在 `root` 之下的文件有几个。问不出来就回 `None`。
    ///
    /// `/proc/self/fd` 上问得出，别的平台上没有这个东西。只数落在点名那棵树下的，
    /// 因此同一个测试二进制里别的用例开着的文件干扰不到它。
    fn open_files_under(root: &Path) -> Option<usize> {
        let held = std::fs::read_dir("/proc/self/fd").ok()?;
        Some(
            held.filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
                .filter(|target| target.starts_with(root))
                .count(),
        )
    }

    /// 预扫**开完就放**：点名几百个归档卷，一趟下来一个句柄都不攥着
    /// （`volume-discovery/01`）。
    ///
    /// 「还攥着没有」两个平台各答一半，两句都在：
    ///
    /// - Linux 上 `/proc/self/fd` 直接数得出来（见 [`open_files_under`]）。
    /// - Windows 上问不出句柄数，改问一件等价的事：**装着这些归档的目录还改不改得动名**。
    ///   那个平台上目录里只要还开着一个文件，重命名就会被拒。
    ///
    /// 两句在对方的平台上都恒成立，因此谁也不会误报；而把卷留在预扫手上的那一版，
    /// 两句各在自己的平台上当场红。
    #[test]
    fn a_survey_keeps_no_archive_open() {
        let space = tempfile::tempdir().expect("建临时目录");
        let library = space.path().join("库");
        std::fs::create_dir(&library).expect("建库目录");
        let inputs: Vec<PathBuf> = (0..MANY)
            .map(|n| {
                let path = library.join(format!("第{n:03}话.cbz"));
                std::fs::write(&path, EMPTY_ARCHIVE).expect("写空归档");
                path
            })
            .collect();

        // 点名的卷与输出根之外，用的是 crate 里那一份最小请求（`crate::tests::request`）——
        // 本模块只关心点名了哪几个卷，别的一格都不改，也不该再抄一份 `Request` 字面量。
        let survey = Survey::of(&Request {
            inputs,
            output_root: space.path().join("out"),
            ..crate::tests::request()
        })
        .expect("几百个空归档都该点得开");

        if let Some(held) = open_files_under(&library) {
            assert_eq!(held, 0, "预扫开完还攥着 {held} 个句柄");
        }
        std::fs::rename(&library, space.path().join("改过名"))
            .expect("装着这些归档的目录改不动名：预扫还攥着其中某个文件");

        // 断言排在放句柄那两句**之后**：`survey` 因此活到这里，
        // 上面问的是「攥着这一份摘要的同时」还开不开着文件。
        assert_eq!(survey.into_volumes().len(), MANY, "点名的卷没全数进来");
    }
}
