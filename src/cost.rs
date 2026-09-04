//! 分阶段耗时剖面：feature-gated 的量具，默认整个不在。
//!
//! **它不是《卷级计时》。**卷级计时按**段**分（幂等这一道 / 第一遍 / 第二遍），是互不重叠的
//! 墙钟，进 [`crate::Report`]；本模块按**阶段**分——解码、转灰、裁边、缩放、判据、量化、
//! 编码这几步，只印到 stderr。两者一个是产品输出，一个是量具，不许合流
//! （`CONTEXT.md` 的《管线》，加固批 13 号票明禁把阶段计时塞进报告）。
//!
//! # 这个数是什么
//!
//! **各线程墙钟之和**，不是 CPU 时间。第一遍是满核并行的，一条计算线程在某个阶段里
//! 被调度出去的那一截也算在内。因此：
//!
//! - **占比可比**——各阶段一视同仁地吃这笔膨胀，排序与份额站得住，「前三大开销」答得出。
//! - **绝对值不可当 CPU 时间读**，也不可与整趟墙钟相加相减。
//!
//! 这正是 `CONTEXT.md` 说「聚合出来的数会骗人」的那件事：本模块的答法是**挑明一种口径**，
//! 而不是把它塞进报告让人当墙钟读。
//!
//! # 怎么用
//!
//! ```text
//! cargo build --release --features profiling
//! target/release/tonefit --profile boox-poke6 --out <目录> <卷>
//! ```
//!
//! 收场时 stderr 上多出一张表，一行一个阶段，按耗时降序。数落进
//! `docs/measurements.md` 的《分阶段耗时剖面》，别散到别处（`CLAUDE.md`）。

/// 管线上的一步。**与《段》不是一回事**，见模块文档。
///
/// 绝大多数是**页内**的一步（解码、转灰、缩放、判据……），一页走一次；
/// 末两格是**卷内**的（汇总、拼报告），一卷走一次。两者摆在同一张表上，
/// 因为表回答的是「这一趟的时间花在哪」，而那个问句不分层——
/// 一格贵不贵看的是它的总数，不是它一次多久。「次」那一列把两者分得开。
///
/// 排在这里的是花得起时间的那几步；判断一步该不该占一格，看的是「它单独摘出来
/// 会不会改变前三大开销的名单」。
///
/// **判别式就是它在两张表里的下标**（`#[repr(usize)]` 加声明次序），因此加一格
/// 不必再去别处对齐一个数组；[`ALL`] 只再管印出来的次序，而
/// [`the_stages_are_their_own_index`] 钉住两者不分家。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Stage {
    /// 摊开：固实归档开工前整卷解到临时目录（ADR 0015 决定第 3 条）。
    ///
    /// 一卷走一次，而且只有 `.7z` 那种卷走——排一格是因为它一次就吃掉整卷的解压加整卷的写盘，
    /// 单独摘出来必然改变前三大开销的名单（本类型开头那条判据）。
    /// 摊开之后那几步与目录卷一模一样，[读字节](Self::Read)因此照旧只记读，不记这一笔。
    Extract,
    /// 取源字节。目录卷上可能并发；归档卷上两遍是一条顺序扫，幂等那一道各开各的句柄
    /// （见 `crate::medium::IoPlan` 的《为什么是两路》）。
    Read,
    /// 喂源哈希（幂等这一道）。读字节那一截不在里面。
    Hash,
    /// 解码到像素缓冲。
    Decode,
    /// 彩页识别。
    Identify,
    /// 转灰。**只有灰度路径走它**——彩色分支不转灰（`CONTEXT.md` 的《管线》）。
    ToGray,
    /// 彩色分支上取彩色像素缓冲。与[转灰](Self::ToGray)分开记：
    /// 两条路做的不是同一件事，混进一格，彩色面板上那一行就说不出自己量的是什么。
    ToColor,
    /// 裁边：量墨 + 取窗口。切开之后每半再裁的那一趟也算在这里。
    Crop,
    /// 判跨页与切开。
    Split,
    /// 缩放到目标尺寸（含整数倍预缩）。
    Resize,
    /// 判据：建参照（低通、掩蔽加权、参照自己的高频起伏），
    /// 再在六个候选上各量化一遍、各求一次判据。
    Metric,
    /// 参照进缓存：压缩 + 记账（溢写的话还有那一次写盘）。
    CacheIn,
    /// 参照出缓存：取回 + 解压。
    CacheOut,
    /// 定下来那一档的量化。判据里那六次不在这里。
    Quantize,
    /// PNG 编码。彩色分支上的那一次也算在这里。
    Encode,
    /// 写进输出容器。
    Write,
    /// 汇总：上包络、迟滞、离群。
    Summarize,
    /// 拼卷报告。命令行那一趟每卷拼两次（停车场 Q77）。
    Assemble,
}

// `test` 也开着：闸门第一条（`cargo test`）按**默认特性**走，而 `profiling` 不在 `default` 里。
// 不带上 test，下面那条「判别式即下标」的用例就没有 `ALL` 可比，枚举与数组分家一声不吭。
// 第三条闸门（`cargo check --features profiling`）验的是这一格之外的那一半，
// 三条各盖什么见 `docs/agents/gate.md`。
#[cfg(any(feature = "profiling", test))]
impl Stage {
    /// 印在表上的名字。
    const fn name(self) -> &'static str {
        match self {
            Self::Extract => "摊开",
            Self::Read => "读字节",
            Self::Hash => "源哈希",
            Self::Decode => "解码",
            Self::Identify => "彩页识别",
            Self::ToGray => "转灰",
            Self::ToColor => "取彩色像素",
            Self::Crop => "裁边",
            Self::Split => "判跨页与切开",
            Self::Resize => "缩放",
            Self::Metric => "判据（6 候选）",
            Self::CacheIn => "缓存写入",
            Self::CacheOut => "缓存取回",
            Self::Quantize => "量化",
            Self::Encode => "编码",
            Self::Write => "落盘",
            Self::Summarize => "汇总",
            Self::Assemble => "拼报告",
        }
    }
}

/// 全部阶段，**按声明次序**，也就是按判别式次序。表按耗时降序印，这一串只定次序与个数。
#[cfg(any(feature = "profiling", test))]
const ALL: &[Stage] = &[
    Stage::Extract,
    Stage::Read,
    Stage::Hash,
    Stage::Decode,
    Stage::Identify,
    Stage::ToGray,
    Stage::ToColor,
    Stage::Crop,
    Stage::Split,
    Stage::Resize,
    Stage::Metric,
    Stage::CacheIn,
    Stage::CacheOut,
    Stage::Quantize,
    Stage::Encode,
    Stage::Write,
    Stage::Summarize,
    Stage::Assemble,
];

/// 表上的那几行：**碰过的格子**（纳秒非零），按耗时降序。
///
/// 两串按判别式寻址，与两张计数表同一个下标（[`the_stages_are_their_own_index`] 钉着这一条）。
///
/// **纯函数，数从哪儿来它不管**：`profiling` 关着的那一趟也编译得到、验得到
/// （`cfg` 上那一格 `test`）。计数那一半因此不是只有「编译得过」这一条自动检查——
/// 挑哪几行、按什么排，闸门第一条就盖得住（`docs/agents/gate.md`）。
/// 留给 `profiling` 那一趟的只剩往原子表上加数与从它上面读回来。
#[cfg(any(feature = "profiling", test))]
fn rows(nanos: &[u64; ALL.len()], hits: &[u64; ALL.len()]) -> Vec<(Stage, u64, u64)> {
    let mut rows: Vec<(Stage, u64, u64)> = ALL
        .iter()
        .map(|&stage| {
            let slot = stage as usize;
            (stage, nanos[slot], hits[slot])
        })
        .filter(|&(_, nanos, _)| nanos > 0)
        .collect();
    rows.sort_by_key(|&(_, nanos, _)| std::cmp::Reverse(nanos));
    rows
}

/// 印出来的那张表。**一格都没碰过就是 `None`**——那时一个字都不印。
///
/// 与 [`rows`] 同一条：纯函数，闸门第一条盖得住。
#[cfg(any(feature = "profiling", test))]
fn table(rows: &[(Stage, u64, u64)]) -> Option<String> {
    let total: u64 = rows.iter().map(|&(_, nanos, _)| nanos).sum();
    if total == 0 {
        return None;
    }
    let mut text = "分阶段耗时剖面（各线程墙钟之和，占比可比、绝对值不是 CPU 时间）\n".to_owned();
    text.push_str(&format!(
        "{:<16} {:>10} {:>8} {:>12}\n",
        "阶段", "秒", "占比", "次"
    ));
    for &(stage, nanos, hits) in rows {
        text.push_str(&format!(
            "{:<16} {:>10.3} {:>7.1}% {:>12}\n",
            stage.name(),
            nanos as f64 / 1e9,
            nanos as f64 * 100.0 / total as f64,
            hits,
        ));
    }
    text.push_str(&format!(
        "{:<16} {:>10.3} {:>7.1}%\n",
        "合计",
        total as f64 / 1e9,
        100.0
    ));
    Some(text)
}

/// 掐一个阶段：跑一遍 `work`，把这一次的耗时累加到 `stage` 那一格。
///
/// 特性关着时它就是 `work()` 本身——闭包内联掉，一条指令都不多。
#[inline(always)]
pub(crate) fn stage<T>(stage: Stage, work: impl FnOnce() -> T) -> T {
    #[cfg(feature = "profiling")]
    {
        let started = std::time::Instant::now();
        let out = work();
        tally::add(stage, started.elapsed());
        out
    }
    #[cfg(not(feature = "profiling"))]
    {
        let _ = stage;
        work()
    }
}

/// 开表：把各格清零。特性关着时什么都不做。
///
/// [`crate::run`] 进门就调它，因为表印出来说的是「**这一趟**」，而计数器活在进程里：
/// 一个会话可以跑很多趟（`CONTEXT.md` 的《会话》），不清零的话第二趟起印的是累计值。
///
/// 一次只跑一趟是这里的前提——会话把 `run` 放在一条工作线程上，同时只有一趟在走。
/// 真要并排跑两趟，两趟的账会记到一起；那时该改的是这个量具的作用域，不是这一句。
pub(crate) fn start() {
    #[cfg(feature = "profiling")]
    tally::start();
}

/// 把这一趟的剖面印到 stderr。特性关着时什么都不做。
///
/// 印在 stderr 而不是 stdout：stdout 上是报告，量具不该混进产品输出里去。
///
/// 走到这里的只有跑完与被按停两种收场。**拒绝执行那一路不印**——那一趟一页都没做
/// （`crate::run` 的文档），没有剖面可言。
pub(crate) fn print_profile() {
    #[cfg(feature = "profiling")]
    tally::print_profile();
}

#[cfg(feature = "profiling")]
mod tally {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::{ALL, Stage};

    /// 一格一个阶段，装纳秒。原子加：第一遍是满核并行的，每条计算线程都往里写。
    ///
    /// 一个卷一趟的量级是几十秒 × 几十条线程，离 `u64` 纳秒的上界（584 年）差着十个量级。
    static NANOS: [AtomicU64; ALL.len()] = [const { AtomicU64::new(0) }; ALL.len()];
    /// 同一格上进出过多少次。有了它，「贵在单次还是贵在次数多」分得开。
    static HITS: [AtomicU64; ALL.len()] = [const { AtomicU64::new(0) }; ALL.len()];

    pub(super) fn start() {
        for slot in 0..ALL.len() {
            NANOS[slot].store(0, Ordering::Relaxed);
            HITS[slot].store(0, Ordering::Relaxed);
        }
    }

    pub(super) fn add(stage: Stage, elapsed: Duration) {
        let slot = stage as usize;
        NANOS[slot].fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
        HITS[slot].fetch_add(1, Ordering::Relaxed);
    }

    /// 把两张原子表读成两串数，交给[挑行](super::rows)与[排版](super::table)那两个纯函数。
    ///
    /// **这一层只做「读回来」这一件事**：挑哪几行、按什么排、印成什么样都在特性外面，
    /// 默认那一趟的用例验得到（`docs/agents/gate.md`）。
    pub(super) fn print_profile() {
        let nanos = std::array::from_fn(|slot| NANOS[slot].load(Ordering::Relaxed));
        let hits = std::array::from_fn(|slot| HITS[slot].load(Ordering::Relaxed));
        if let Some(table) = super::table(&super::rows(&nanos, &hits)) {
            eprint!("{table}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ALL, Stage, rows, table};

    /// **判别式就是下标**，而 [`ALL`] 按同一个次序排开。
    ///
    /// 两张计数表按判别式寻址，`ALL` 只管印出来的次序与个数——两者一分家，
    /// 新加的那一格要么把账记到邻居头上，要么根本印不出来，而两种都不报错。
    /// 这条用例在**默认特性**下也跑得到（`ALL` 与 `Stage::name` 对 `test` 开着），
    /// 因此闸门盖得住它：`profiling` 不在 `default` 里，别的检查都够不着这半个模块。
    #[test]
    fn the_stages_are_their_own_index() {
        for (index, &stage) in ALL.iter().enumerate() {
            assert_eq!(
                stage as usize,
                index,
                "{} 这一格的判别式与下标分家了",
                stage.name()
            );
        }
    }

    /// 每一格都有一个自己的名字：表是按名字读的，两格同名就并成了一行。
    #[test]
    fn every_stage_has_a_name_of_its_own() {
        let mut names: Vec<&str> = ALL.iter().map(|&stage| stage.name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "有两格重名");
        assert!(names.iter().all(|name| !name.is_empty()), "有一格没有名字");
    }

    /// 一格都没碰过就**一个字都不印**。
    ///
    /// 拒绝执行那一路走到的正是这一种（`crate::run` 一页都没做）：那时印一张全是零的表，
    /// 读的人会以为每一步都花了零秒，而事实是一步都没走过。
    #[test]
    fn a_table_nobody_touched_prints_nothing() {
        let untouched = [0; ALL.len()];

        assert_eq!(rows(&untouched, &untouched), vec![]);
        assert_eq!(table(&[]), None);
    }

    /// 表上**只有碰过的那几格**，按耗时降序，各格的数落在自己那一行上。
    ///
    /// 三件事一条用例：挑行（没碰过的不占一行）、排序（最贵的排头）、
    /// 以及两串数按判别式各归各位（[`the_stages_are_their_own_index`] 钉的是同一个下标）。
    #[test]
    fn the_table_lists_what_was_touched_worst_first() {
        let mut nanos = [0; ALL.len()];
        let mut hits = [0; ALL.len()];
        for (stage, spent, times) in [
            (Stage::Encode, 3_000_000_000, 7),
            (Stage::Decode, 9_000_000_000, 4),
            (Stage::Crop, 1, 1),
        ] {
            nanos[stage as usize] = spent;
            hits[stage as usize] = times;
        }

        assert_eq!(
            rows(&nanos, &hits),
            vec![
                (Stage::Decode, 9_000_000_000, 4),
                (Stage::Encode, 3_000_000_000, 7),
                (Stage::Crop, 1, 1),
            ]
        );
    }

    /// 印出来的那张表：两行抬头、碰过的那几格各一行、末尾一行合计，占比按总数折算。
    #[test]
    fn the_table_names_each_stage_and_totals_the_shares() {
        let mut nanos = [0; ALL.len()];
        let hits = [1; ALL.len()];
        nanos[Stage::Decode as usize] = 9_000_000_000;
        nanos[Stage::Encode as usize] = 3_000_000_000;

        let printed = table(&rows(&nanos, &hits)).expect("碰过两格");
        let lines: Vec<&str> = printed.lines().collect();

        assert!(printed.ends_with('\n'), "末行没有换行：{printed:?}");
        assert_eq!(lines.len(), 5, "两行抬头 + 两行 + 合计：{printed}");
        assert!(lines[2].starts_with("解码"), "{printed}");
        assert!(lines[2].contains("75.0%"), "{printed}");
        assert!(lines[3].starts_with("编码"), "{printed}");
        assert!(lines[3].contains("25.0%"), "{printed}");
        assert!(lines[4].starts_with("合计"), "{printed}");
        // 没碰过的格子一行都没有——`hits` 那一串全是 1，靠的不是它。
        assert!(!printed.contains("量化"), "{printed}");
    }
}
