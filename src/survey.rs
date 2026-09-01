//! 预扫：开工之前把点名的卷全枚举一遍，算出这一趟的**全局总步数**（ADR 0011 决定第 3 条）。
//!
//! 一个卷要走多少步，得先枚举它的成员才知道，而枚举原先发生在处理那一卷的时候——
//! 「整趟还要多久」因此在开工时无从算起，屏上那条横条只说得出当前这一卷走到哪儿了。
//! 几十卷跑下来唯一有人真想知道的数是**整趟**还剩多久，本模块把枚举提到开工之前，
//! 就为了给出那个数。
//!
//! 只列成员，**不碰像素**：目录卷走一遍目录，归档卷读中央目录，两样都是 [`source::open`]
//! 本来就要做的事，一个像素都不解。枚举结果原样交给管线——[`Surveyed`] 里装着那个
//! **已经打开了的卷**，`process_volume` 接着用它，同一个卷不被枚举两次。
//! 「枚举很便宜，扫两遍也无妨」是不成立的：慢盘上枚举正是贵的那一头，而预扫是**每趟都要付**
//! 的成本，付两遍就等于把它翻倍。
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
//! **要认的代价**：点名的每一个归档卷在整趟里都占着一个打开的文件句柄与一份中央目录
//! （目录卷不占——它的读取端只是一个路径）。这笔代价是「不扫两次」的必然结果：
//! 一个 `ZipArchive` 就是那次枚举本身，放掉它就等于要再读一遍中央目录。

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::source::{self, Volume};
use crate::{MemberCounts, Request, max_outputs_per_source_page, volume_steps};

/// 这一趟预扫出来的东西：点名的每一个卷一份，外加它们步数之和。
pub(crate) struct Survey {
    volumes: Vec<Surveyed>,
    /// 各卷步数之和。这个数由 [`Surveyed::steps`] **加**出来，不是另算一遍——
    /// 「全局总步数等于各卷步数之和」因此是构造出来的，不是一条要靠人守住的约定。
    steps: u64,
}

/// 预扫过的一个卷。
///
/// 装的是那个**打开了的卷**本身，不是它的一份摘要：管线接着就用它，不再开第二次
/// （见本模块的模块文档）。
pub(crate) struct Surveyed {
    /// 打开了的卷，成员表已经排好序。
    pub(crate) volume: Volume,
    /// 这一卷这一趟要碰的成员数。
    pub(crate) members: MemberCounts,
    /// 这一卷这一趟最多走多少步。开卷那条事件报的就是它。
    pub(crate) steps: u64,
    /// 枚举这一卷花了多久。
    ///
    /// 它进这一卷的 [`VolumeTiming::elapsed`](crate::VolumeTiming::elapsed)——那个数的定义是
    /// 「从打开卷到这份卷报告成型」，而打开卷这件事只是挪到了预扫里，并没有变便宜。
    /// 不带上它，慢盘上那一截会静默消失，而 `outside_the_segments` 的文档正指着它说
    /// 「少掉的那一截恰恰是枚举」。
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
                    let source_pages = volume.pages.len();
                    let members = MemberCounts {
                        source_pages,
                        // 上界，不是承诺：一卷里真被切开的页越少，第二遍走过的步越少
                        // （见 [`volume_steps`]）。
                        output_pages: source_pages * max_outputs_per_source_page(request),
                        extras: volume.extras.len(),
                    };
                    volumes.push(Surveyed {
                        steps: volume_steps(members, request),
                        volume,
                        members,
                        enumerating,
                    });
                }
                Err(error) => refused.push((input.as_path(), error)),
            }
        }
        if !refused.is_empty() {
            // 已经打开的那些卷在这里一起放掉：整趟不做，句柄没有留着的理由。
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
}
