//! 预扫：开工之前**发现**这一趟有哪些卷，再把它们全枚举一遍，算出这一趟的
//! **全局总步数**（ADR 0011 决定第 3 条）。
//!
//! 两步，都在开工前：
//!
//! 1. **发现**——点名的每一个路径展开成一批卷（ADR 0014，见 [`crate::discover`]）。
//!    这一步不碰卷的内容，只看盘上的形状。
//! 2. **计数**——每一个卷开一次、列一遍成员、算出它这一趟要走多少步。
//!
//! 一个卷要走多少步，得先枚举它的成员才知道，而枚举原先发生在处理那一卷的时候——
//! 「整趟还要多久」因此在开工时无从算起，屏上那条横条只说得出当前这一卷走到哪儿了。
//! 几十卷跑下来唯一有人真想知道的数是**整趟**还剩多久，本模块把枚举提到开工之前，
//! 就为了给出那个数。发现本身不碰像素、不占步；慢盘上它要花时间，
//! 而命令行那条转轮已经在了（ADR 0011）。
//!
//! 只列成员，**不碰像素**：目录卷列一层目录，归档卷读归档头，一个像素都不解。
//! 走的因此是 [`source::enumerate`] 而不是 [`source::open`]——**固实归档在这一遍不摊开**
//! （ADR 0015）：摊开一整卷只为数几个成员是白付一次全量写盘，而这一遍要把这一趟每个卷
//! 都数一遍。摊开发生在处理那一卷的开头，见 `crate::process_volume`。
//! 数完就**把卷放掉**——[`Surveyed`] 里只剩这一卷的路径
//! 与几个数，处理那一卷时按那个路径再开一次。
//! 扫两遍买的不是省事，是**不攥着几千个句柄**：卷留在预扫手上的那一版里，N 个归档卷
//! 就整趟同时开着 N 个文件、常驻 N 份中央目录，那笔开销随**卷数**长，
//! 成百上千个卷就把一趟胀死——而发现落地之后，点名一个库就是几千个卷（ADR 0014）。
//! 买下来之后一趟同时开着几个，见 `source::Reader` 的《一趟同时开着几个句柄》——
//! 本模块是那笔账里「不是这一趟有几个卷」这一句的来处。
//!
//! 它顺带把「一卷点不开」提前到一页都没做之前：**点名的**那一种点不开在这里
//! **整趟拒绝**、逐条列出（见 [`refuse`]）。理由与「处理范围为空是错误」同一条
//! （ADR 0009 的《不要做的「简化」》）——范围层错了可能写到别人的目录里，
//! 而那一趟已经写出去的卷收不回来。
//! **发现出来的**那一种点不开不走这条路：归档记进非卷文件、目录记进走不进去的地方，
//! 其余照做（ADR 0014 决定第 5 条）——对推测出来的东西不用最重的处置，
//! 一个坏 zip 不该把几百卷挡在门外。
//! 预扫**之后**才出的卷级失败也不走这条路：那时其余卷照做，报告照出
//! （`CONTEXT.md` 的《失败》：卷级失败）。
//!
//! **一页都没有的东西不是卷**（ADR 0014 决定第 3 条）：开出来一页都没有的候选在这里
//! 就被丢掉，此后的每一层都不知道它存在过——输出里因此一个字节都没有。
//! 字体包、源码包、空目录、只装着别的卷的目录都落在这一支上。
//!
//! # 另两份产出：非卷文件，与走不进去的地方
//!
//! 同一遍开卷答的是三件事：**这是一个卷**、**这个不是、而且是为什么**、
//! 以及**这一块根本没看过**。
//!
//! 第二件攒成 [`Survey`] 的第二份，一路走到 [`crate::Report::non_volume_files`]——
//! 三类都在这一处出（见 [`nothing_took_it`] 与上面点不开的那一支）。
//! **它不是失败，退出码一格不动。**
//!
//! 第三件攒成第三份，走到 [`crate::Report::unreachable_places`]：一个**目录**列不出这一层，
//! 它底下有没有卷谁都不知道，整棵子树跳过（`p4-parking-lot/11`，收停车场 Q117）。
//! **这一份动退出码**——一棵没看过的子树不是「全都做成了」。
//! 它进不了上面那张表：那张列的是文件，而这是一个地方
//! （见 [`crate::report::UnreachablePlace`]）。
//!
//! 两份都非说不可，是因为「输出里一个字节都没有」自己会变成另一个毛病：东西没被转、
//! 地方没被走到，而报告里没有一处列得出它们（ADR 0014 的《背景》第 2 条）——
//! **不做**与**说得出**是同一条决定的两半。
//!
//! **不落盘。** 预扫的作用域是这一趟点名的卷，活在一次运行之内。把成员表存下来下趟再用
//! 就是一份全库索引，而那正是 ADR 0009 关掉的东西。
//!
//! **要认的代价**：归档卷的归档头读两遍，目录卷走两遍目录。这一笔随**一个卷**长，
//! 不随点名的卷数长，而且只在轮到那一卷时付、付完就还回去——正是上面那一笔换来的。
//! 两遍之间源变了的话，做的与报的都是**重开的那一份**：预扫这一遍只留下步数
//! （见 [`Surveyed::steps`]），成员数在处理那一卷时按重开的卷重新数一遍。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::discover::{self, Provenance};
use crate::report::{NonVolumeFile, NonVolumeReason, UnreachablePlace};
use crate::source::{self, Container};
use crate::{MemberCounts, Request, volume_steps};

/// 这一趟预扫出来的东西：**发现出来的**每一个卷一份，外加它们步数之和，
/// 再外加没被任何卷收下的那些[非卷文件](NonVolumeFile)，
/// 与[走不进去的那些地方](UnreachablePlace)。
pub(crate) struct Survey {
    volumes: Vec<Surveyed>,
    /// 各卷步数之和。这个数由 [`Surveyed::steps`] **加**出来，不是另算一遍——
    /// 「全局总步数等于各卷步数之和」因此是构造出来的，不是一条要靠人守住的约定。
    steps: u64,
    /// 发现走完之后没被任何卷收下的那些文件，按发现顺序（`volume-discovery/04`）。
    ///
    /// 它与 [`volumes`](Self::volumes) 是同一遍开卷的两份产出：一份答出「这是一个卷」，
    /// 这一份答出「这个不是，而且是为什么」。三类各自的来处见 [`nothing_took_it`] 与
    /// [`Survey::of`] 里点不开的那一支。
    non_volume_files: Vec<NonVolumeFile>,
    /// 发现的时候**走不进去**的那些地方，按发现顺序（`p4-parking-lot/11`）。
    ///
    /// 与上面那两份同一遍产出，而它答的是第三件事：「这一块**没看过**」。
    /// 来处只有一处——[`Survey::of`] 里点不开那一支的目录那一格。
    unreachable_places: Vec<UnreachablePlace>,
}

/// 预扫过的一个卷：**只有数与路径**，没有卷本身。
///
/// 它是那个卷的一份摘要，不是那个卷：预扫数完就把卷放掉，处理那一卷时按
/// [`root`](Self::root) 再开一次（见本模块的模块文档）。
pub(crate) struct Surveyed {
    /// 卷根：目录路径，或归档文件路径（见 [`source::open`]）。处理这一卷时按它再开一次。
    ///
    /// 它**不一定是点名的那个路径**——发现出来的卷躺在点名的路径底下（ADR 0014）。
    pub(crate) root: PathBuf,
    /// 这一卷在**输出根之下**的去处，相对路径（见 [`discover::Candidate::output_relative`]）。
    ///
    /// 私有：外面要的是接好的那条路径，走 [`output_path`](Self::output_path)——
    /// 「输出根接上镜像出来的那几级」只有那一处会拼。
    output_relative: PathBuf,
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

impl Surveyed {
    /// 这一卷写到 `root` 之下的哪里：`root` 接上镜像出来的那几级。
    ///
    /// **接这条路径只有这一处**。它被叫三次，各喂一个不同的根：这一卷干净的去处
    /// （输出根）、隔离目录里那个去处（输出根 + `_isolated`）、以及开工前那道撞名校验
    /// 手上那个根。三处必须得出同一套算法——不然查出来的撞车与实际发生的撞车是两回事
    /// （见 `crate::ensure_no_two_volumes_share_an_output`）。
    pub(crate) fn output_path(&self, root: &Path) -> PathBuf {
        root.join(&self.output_relative)
    }
}

impl Survey {
    /// 发现这一趟有哪些卷，再把它们逐个枚举一遍。
    ///
    /// **点名的**路径里有一个点不开就整趟当场拒绝，一条事件都不发；发现出来的点不开的
    /// 归档进非卷文件那张表、点不开的目录进走不进去的地方那一张，其余照做。
    /// 开出来一页都没有的候选一并跳过——它不是卷，
    /// 它那一层里没被收下的东西同样进那张表（见 [`nothing_took_it`]）。
    pub(crate) fn of(request: &Request) -> Result<Self> {
        let mut volumes = Vec::new();
        let mut non_volume_files = Vec::new();
        let mut unreachable_places = Vec::new();
        // 坏路径**收齐了再报**，不是撞上第一个就返回：点名十个路径、其中三个写错了，
        // 一次说清三个才改得完一遍，逐个报要来回三趟。
        let mut refused = Vec::new();
        for input in &request.inputs {
            // 点名的路径**自己**点不开（不存在、既不是目录也不是认得的归档）在这里就定了：
            // 发现连一个候选都给不出来。
            let candidates = match discover::of(input) {
                Ok(candidates) => candidates,
                Err(error) => {
                    refused.push((input.as_path(), error));
                    continue;
                }
            };
            for candidate in candidates {
                let started = Instant::now();
                match source::enumerate(&candidate.root) {
                    Ok(volume) => {
                        // **一页都没有的东西不是卷**（ADR 0014 决定第 3 条）：只装着别的卷的目录、
                        // 空目录、字体包都落在这里，此后每一层都不知道它存在过。
                        // 走之前先把它没能收下的那些文件记进第三张表——「输出里一个字节都没有」
                        // 与「说得出什么没被转」是同一条决定的两半。
                        if volume.pages.is_empty() {
                            non_volume_files.extend(nothing_took_it(&volume));
                            continue;
                        }
                        let enumerating = started.elapsed();
                        volumes.push(Surveyed {
                            steps: volume_steps(MemberCounts::of(&volume, request), request),
                            root: volume.root,
                            output_relative: candidate.output_relative,
                            enumerating,
                        });
                        // 卷在这一格的末尾**放掉**：归档卷那个 `ZipArchive` 连同它的文件句柄
                        // 跟着析构，预扫因此不随卷数攥住句柄（见本模块的模块文档）。
                        // 固实归档这一遍**不留**句柄：`.7z` 连开都没开，`.rar` 开一次把
                        // 归档头读完、回来之前就放掉。两者的读取端都还是没摊开的那一格
                        // （`source::Reader::Unextracted`），而这一遍一个字节都不读。
                        // 上面那一句只留下了它的路径。
                    }
                    // **点名的 / 发现的**只决定这一件事（ADR 0014 决定第 5 条）。
                    // 点名的那个恒是候选里的头一个，因此这里报的路径就是 `input`。
                    Err(error) => match (candidate.provenance, candidate.container) {
                        (Provenance::Named, _) => refused.push((input.as_path(), error)),
                        // 发现出来的点不开的**归档**进非卷文件清单，其余照做：
                        // 退出码一格不动，而报告说得出是哪一个、为什么。
                        (Provenance::Discovered, Container::Archive) => {
                            non_volume_files.push(NonVolumeFile {
                                path: candidate.root,
                                reason: NonVolumeReason::Unopenable(format!("{error:#}")),
                            });
                        }
                        // 发现出来的点不开的**目录**不进那张表：那张表列的是**文件**
                        // （`CONTEXT.md` 的《处理对象》把三类都写成文件，spec 与 ADR 0014
                        // 决定第 5 条同样只说归档）。它整棵子树跳过，进的是**走不进去的
                        // 地方**那一张（`p4-parking-lot/11` 收的停车场 Q117）：
                        // 其余卷照常跑完，而退出码不再是「全都做成了」。
                        //
                        // `discover::push_children` 里「列不动这一层」那一句因此不再是
                        // 静默的——被它跳过的每一个目录，自己都是这里的一个候选
                        // （见那个函数的文档）。
                        //
                        // 这一格收的是 `enumerate` 在一个**目录**上失败的每一种，
                        // 而抬头那句话（见二进制侧的 `render::unreachable_tail`）按
                        // **压倒性的那一种**写：列不出这一层。另外几种都是两次系统调用
                        // 之间的竞态（目录没了、某一项问不出形态），那时子树其实走过了，
                        // 而抬头那句会说得重一点——记在停车场 Q201，那一条原因照旧
                        // 原样带出来，用户读得到真相。
                        (Provenance::Discovered, Container::Directory) => {
                            unreachable_places.push(UnreachablePlace {
                                path: candidate.root,
                                reason: format!("{error:#}"),
                            });
                        }
                    },
                }
            }
        }
        if !refused.is_empty() {
            return Err(refuse(&refused, request.inputs.len()));
        }
        Ok(Self {
            steps: volumes.iter().map(|surveyed| surveyed.steps).sum(),
            volumes,
            non_volume_files,
            unreachable_places,
        })
    }

    /// 这一趟最多走多少步。开工那条事件报的就是它。
    pub(crate) fn steps(&self) -> u64 {
        self.steps
    }

    /// 发现出来的那些卷。开工前那道撞名校验按它查
    /// （见 `crate::ensure_no_two_volumes_share_an_output`）——撞车要在发现之后才查得准：
    /// 点名的是**在哪里找**，撞在一起的是**找到的那些**。
    pub(crate) fn volumes(&self) -> &[Surveyed] {
        &self.volumes
    }

    /// 按发现顺序交出预扫的**三份产出**：那些卷、那些非卷文件、那些走不进去的地方。
    ///
    /// 一次交出而不是分三个取数：后两份要跟着卷一路走到 [`crate::Report`] 上，
    /// 而卷这一份是被吃掉的（处理一卷就消费一份摘要）——分几次取就得给这一份留一份克隆。
    pub(crate) fn into_volumes_and_the_rest(
        self,
    ) -> (Vec<Surveyed>, Vec<NonVolumeFile>, Vec<UnreachablePlace>) {
        (self.volumes, self.non_volume_files, self.unreachable_places)
    }
}

/// 一个候选**开出来一页都没有**时，它在非卷文件那张表上留下的那几条。
///
/// 两种容器留下的不是一回事：
///
/// - **归档**留下它自己一条（[`NonVolumeReason::ArchiveWithoutAPage`]）——包是一个整体，
///   用户要知道的是「这个包里没有页」，不是包里那几个文件分别叫什么。
/// - **目录**留下的是它这一层里那些既不是页也不是归档的文件，逐条列
///   （[`NonVolumeReason::NeitherPageNorArchive`]）。**目录自己不上表**：一个只装着别的卷的
///   目录是库的正常形状，不是一件要报的事（ADR 0014 决定第 1 条：不给「装卷的目录」造词条）。
///   空目录因此一条都不留——它没有落下任何东西。
///
/// 收下与没收下的分界由 [`source`] 一处说了算：躺在这一层的归档是卷不是成员、打包环境
/// 留下的边车与索引文件根本不算成员（见 `source::open_directory` 与 `source::is_junk`）。
/// 这里读的是它分好的那两摞，不另立一套「什么算页」。
fn nothing_took_it(volume: &source::Volume) -> Vec<NonVolumeFile> {
    match volume.container {
        Container::Archive => vec![NonVolumeFile {
            path: volume.root.clone(),
            reason: NonVolumeReason::ArchiveWithoutAPage,
        }],
        Container::Directory => volume
            .extras
            .iter()
            .map(|member| NonVolumeFile {
                path: volume.identity(member),
                reason: NonVolumeReason::NeitherPageNorArchive,
            })
            .collect(),
    }
}

/// **点名的**那几条路径里点不开的逐条列出，整趟拒绝。
///
/// 只有点名的进得来。发现出来的点不开的归档不在这张清单上——它进的是非卷文件那一张
/// （ADR 0014 决定第 5 条），退出码一格不动。
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
        "点名的 {named} 个路径里有 {} 个点不开，整趟不做。点不开的是：\n",
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

    /// 一个**最小的卷**：装着一个叫 `001.png` 的空成员的 ZIP。
    ///
    /// 成员一个都不能少：一页都没有的东西不是卷（ADR 0014 决定第 3 条），
    /// 空归档在预扫里当场被丢掉，这条用例就一个卷都数不到了。
    /// 页是不是解得出来这里不问——`decode::is_page` 只看扩展名，而这条用例问的只有
    /// 「预扫开完还攥不攥着它们」。
    ///
    /// 字节拼一次、几百个文件共用：拼它比写它贵得多。
    fn one_page_archive() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file::<_, ()>("001.png", zip::write::SimpleFileOptions::default())
            .expect("起一个成员");
        writer.finish().expect("收尾").into_inner()
    }

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
        let archive = one_page_archive();
        let inputs: Vec<PathBuf> = (0..MANY)
            .map(|n| {
                let path = library.join(format!("第{n:03}话.cbz"));
                std::fs::write(&path, &archive).expect("写归档");
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
        .expect("几百个归档都该点得开");

        if let Some(held) = open_files_under(&library) {
            assert_eq!(held, 0, "预扫开完还攥着 {held} 个句柄");
        }
        std::fs::rename(&library, space.path().join("改过名"))
            .expect("装着这些归档的目录改不动名：预扫还攥着其中某个文件");

        // 断言排在放句柄那两句**之后**：`survey` 因此活到这里，
        // 上面问的是「攥着这一份摘要的同时」还开不开着文件。
        let (volumes, _, _) = survey.into_volumes_and_the_rest();
        assert_eq!(volumes.len(), MANY, "点名的卷没全数进来");
    }

    /// 把一个目录弄成**列不出来**的样子，成了才回 `true`。
    ///
    /// 弄不成的机器上（Windows 没有这一手，root 底下权限位不作数）走它的那几条用例当场
    /// 收工——它们在那种机器上恒不成立、问不出来，因此误报不了；而在问得出来的机器上，
    /// 「一声不吭」那一版当场红。这与发现那一条符号链接用例同一个办法
    /// （见 `tests/discovery.rs` 的 `discovery_does_not_follow_a_symlink`）。
    ///
    /// **真去列一遍**才算数：`set_permissions` 在 root 底下也回 `Ok`，而权限位拦不住 root。
    #[cfg(unix)]
    fn shut_the_door(dir: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;

        if std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o000)).is_err() {
            return false;
        }
        if std::fs::read_dir(dir).is_ok() {
            // 这一手没作数（root 绕过权限位）。**门要打回去**：不然临时目录里留下一个
            // `0o000` 的目录，而收场那一手删不掉它。
            open_the_door(dir);
            return false;
        }
        true
    }

    #[cfg(not(unix))]
    fn shut_the_door(_dir: &Path) -> bool {
        false
    }

    /// 把门再打开，好让临时目录收得掉。
    #[cfg(unix)]
    fn open_the_door(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    }

    #[cfg(not(unix))]
    fn open_the_door(_dir: &Path) {}

    /// 一个库，装着一个真卷、一个**列不出来**的目录。门开不上就回 `None`。
    fn a_library_with_a_closed_door(space: &Path) -> Option<(PathBuf, PathBuf)> {
        let library = space.join("库");
        std::fs::create_dir(&library).expect("建库目录");
        std::fs::write(library.join("好的.cbz"), one_page_archive()).expect("写归档");
        let closed = library.join("读不动的");
        std::fs::create_dir(&closed).expect("建读不动的那一层");
        shut_the_door(&closed).then_some((library, closed))
    }

    /// **发现出来的一个读不动的目录说得出来**，而其余卷照常收下（`p4-parking-lot/11`，
    /// 收停车场 Q117）。
    ///
    /// 从前这一格是空的：那棵子树整个消失，报告一行不说，退出码还是 `0`——
    /// 用户拿到的是一份看起来成功、实际少了几十卷的输出。
    ///
    /// 三句话：那一处**上了表**、说得出**是哪个路径与为什么**、而**别的卷一个没少**。
    #[test]
    fn a_directory_that_cannot_be_read_says_so() {
        let space = tempfile::tempdir().expect("建临时目录");
        let Some((library, closed)) = a_library_with_a_closed_door(space.path()) else {
            return;
        };

        let surveyed = Survey::of(&Request {
            inputs: vec![library],
            output_root: space.path().join("out"),
            ..crate::tests::request()
        });
        // 断言之前先把门打开：断言红了也不至于留下一个删不掉的临时目录。
        open_the_door(&closed);

        let (volumes, _, unreachable) = surveyed
            .expect("点名的那个目录读得动，这一趟不该被拒")
            .into_volumes_and_the_rest();
        assert_eq!(volumes.len(), 1, "其余卷没照常收下");
        assert_eq!(
            unreachable.len(),
            1,
            "读不动的那一层没上表：{unreachable:?}"
        );
        assert_eq!(unreachable[0].path, closed, "上表的不是那个路径");
        assert!(
            unreachable[0].reason.contains("读不动的"),
            "那一句没说是哪个地方：{}",
            unreachable[0].reason
        );
    }

    /// **点名一个读不动的目录仍是当场拒**（ADR 0014 决定第 5 条）。
    ///
    /// 「点名的 / 发现的」那条分别一格没动：上一条那个目录换成点名的，这一趟就整个不做，
    /// `Survey::of` 回的是 `Err`、一份报告都没有。少了这一条，新那一栏会悄悄把
    /// 「点名的当场拒」降级成「记一笔、接着跑」。
    #[test]
    fn a_named_directory_that_cannot_be_read_is_still_refused() {
        let space = tempfile::tempdir().expect("建临时目录");
        let closed = space.path().join("读不动的");
        std::fs::create_dir(&closed).expect("建读不动的那一层");
        if !shut_the_door(&closed) {
            return;
        }

        let surveyed = Survey::of(&Request {
            inputs: vec![closed.clone()],
            output_root: space.path().join("out"),
            ..crate::tests::request()
        });
        open_the_door(&closed);

        let error = surveyed.err().expect("点名一个读不动的目录该整趟拒");
        let said = format!("{error:#}");
        assert!(said.contains("整趟不做"), "拒的那句话不对：{said}");
    }

    /// **两张表分得开**：读不动的**归档**进非卷文件，读不动的**目录**进走不进去的地方
    /// （`p4-parking-lot/11`）。
    ///
    /// 这一条钉的是本票那条边界：非卷文件那三类列的是**文件**，而一个目录不是文件——
    /// 塞进去要改 `CONTEXT.md` 的词条，那是另一回事。两样东西摆在同一层里一次问清楚，
    /// 混成一张表当场红。
    #[test]
    fn a_broken_archive_and_a_closed_door_land_on_different_lists() {
        let space = tempfile::tempdir().expect("建临时目录");
        let Some((library, closed)) = a_library_with_a_closed_door(space.path()) else {
            return;
        };
        // 中央目录与尾记录都没有：这个归档结构根本读不出来。
        std::fs::write(library.join("坏的.cbz"), b"not a zip at all").expect("写坏归档");

        let surveyed = Survey::of(&Request {
            inputs: vec![library.clone()],
            output_root: space.path().join("out"),
            ..crate::tests::request()
        });
        open_the_door(&closed);

        let (_, non_volume_files, unreachable) = surveyed
            .expect("点名的那个目录读得动")
            .into_volumes_and_the_rest();
        assert_eq!(
            non_volume_files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            [library.join("坏的.cbz")],
            "点不开的归档没进非卷文件那张表，或者读不动的目录混了进去"
        );
        assert!(
            matches!(
                non_volume_files[0].reason,
                NonVolumeReason::Unopenable(ref _said)
            ),
            "点不开的归档记成了别的一类"
        );
        assert_eq!(
            unreachable
                .iter()
                .map(|place| place.path.clone())
                .collect::<Vec<_>>(),
            [closed],
            "走不进去那张表上的不是那一处"
        );
    }
}
