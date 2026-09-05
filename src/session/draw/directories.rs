//! 屏上那一块：**目录表**——报告区默认那一副，一个目录一行
//! （`volume-discovery/08`，`CONTEXT.md` 的《会话》：展开）。
//!
//! ```text
//!  记号  目录        卷数  基准档分布
//!  ✓     网络资源      12  2bit+FS 9 · 4bit+FS 3
//!  !     原盘扫描       4  4bit+FS 3 · 跳过 1      隔离 1 卷
//!  ✗     旧库           2  没做成 2
//! ```
//!
//! # 一趟几百卷时，先看得完的是它
//!
//! 卷表一卷一行，几百卷就是几百行——「一屏看得完」在那一副上根本不成立。
//! 这一副把一枝收成一行：**几卷 · 判成哪几档 · 几卷进了隔离**，
//! 追下去按 `⏎` 展开那一枝（[`super::table`]），再按一下展开那一卷的逐页
//! （[`super::pages`]）。
//!
//! # 分组与聚合都不在这里
//!
//! **一处出处在 [`crate::render`]**：按目录分组是 [`crate::render::grouped`]，
//! 一枝那一行上说什么是 [`crate::render::directory`]——命令行印出去的那一份
//! 读的是同一批格（[`crate::render::plain`]），两边不许各算各的（ADR 0016）。
//! 会话这一侧一棵树都不另切：层次与**发现出来的那棵树**一致
//! （`CONTEXT.md` 的《发现》）。
//!
//! 长在这一层的仍旧只有**命令行上根本没有**的那两样：列头
//! （[`DirectoryColumn::head`](crate::session::columns::Column::head)）与
//! [行首记号](super::table::Mark)——那一路一个列都不分，也就不需要它们。
//!
//! # 哪几列、砍哪几列不在这里
//!
//! 在 [`crate::session::columns`]，与另外两张表同一套（那一块在终端库外面）。

use super::paint::Painted;
use super::table::{Mark, Table};
use crate::render::{self, Field, Row};
use crate::session::columns::{self, Column, DirectoryColumn, Widths};
use crate::session::live::{Branch, Live, Volume};

/// 表上的一行：一枝。
struct Entry {
    /// 这一枝底下**停得住的那几卷**（`CONTEXT.md` 的《会话》：跟随——光标恒是一卷）。
    ///
    /// **光标停在其中任何一卷上，反白的都是这一行**：这一副只是把那个光标归到一行上，
    /// 它自己不另记一个。
    ///
    /// **可以是空的**：一枝底下的卷全没做成时它一卷都收不住，光标因此停不上去——
    /// 与卷表上没做成那几行停不上去是同一条规矩。它照旧占一行。
    inside: Vec<Volume>,
    /// 行首记号。
    mark: Mark,
    /// 这一枝是哪个目录，**印全路径**。
    ///
    /// **不与卷名同一条规矩**（那一条只印最后那一段）：卷名的两头是书名与第几卷，
    /// 而两枝的末一段常常一模一样（`库/漫画/连载` 与 `库/图集/连载`）——
    /// 「哪个目录出的事」正是这一副要答的那件事，答成两行同名就等于没答。
    /// 命令行那一副印的也是全路径（[`Field::Source`]），两边因此说的是同一个身份。
    /// 摆不下时[从中间省略](crate::session::columns::elide)，两头留着。
    name: String,
    /// 这一枝底下几卷。
    volumes: Option<String>,
    /// 基准档分布。**一卷都判不出档位就不在场。**
    bases: Option<String>,
    /// 跟在行尾的那几句：成句，不塞进格。
    notes: Vec<String>,
}

impl Entry {
    /// 这一列上那一格。
    fn cell(&self, column: DirectoryColumn) -> Option<&str> {
        match column {
            DirectoryColumn::Mark => None,
            DirectoryColumn::Name => Some(&self.name),
            DirectoryColumn::Volumes => self.volumes.as_deref(),
            DirectoryColumn::Bases => self.bases.as_deref(),
        }
    }

    /// 这一列上写什么。
    ///
    /// **不在场的那几格一个字都不占**：这张表上真会不在场的只有分布那一格
    /// （一枝底下一卷都判不出档位），而它排在末尾——空着就是「这一枝没有这件事」，
    /// 不必再拿一个记号说一遍（卷表那一处的 `ABSENT` 治的是**夹在中间**的页数）。
    /// 卷数那一格恒在场（[`crate::render::directory`] 无条件摆下它）。
    fn text(&self, column: DirectoryColumn) -> String {
        if column == DirectoryColumn::Mark {
            return self.mark.glyph().to_string();
        }
        self.cell(column).map_or_else(String::new, str::to_owned)
    }

    /// 一枝那一行。措辞与聚合都出自 [`crate::render::directory`]
    /// （`row` 由 [`Live::branch_rows`] 一次算齐）。
    fn of_branch(live: &Live, branch: &Branch, row: &Row) -> Self {
        let mut notes = Vec::new();
        if let Some(count) = row.cell(Field::Isolated) {
            notes.push(render::isolated_note(count));
        }
        Self {
            inside: branch.volumes.clone(),
            mark: Mark::of_branch(live, branch),
            name: branch.directory.display().to_string(),
            volumes: row.cell(Field::VolumeCount).map(str::to_owned),
            bases: row.cell(Field::Bases).map(str::to_owned),
            notes,
        }
    }
}

/// **目录表**：列头一行，此后一枝一行。一卷都还没有就一行都不出。
///
/// `room` 是这一格里正文摆得下几列，行首恒留一格空白——与[卷表](super::table::table)
/// 逐字同一条。
///
/// `branches` 是[此刻摆得出的那几枝](Live::branches)，由调用方**一次算出来**：
/// 这一格里算它的地方不止一处（抬头也要说「第几枝」），各算各的就是逐帧多走一遍。
///
/// `at` 是**报告区那个光标停在哪一卷上**：反白的是**它所在的那一枝**
/// （`CONTEXT.md` 的《会话》：跟随——屏上那个光标恒是一卷，这一副只是把它归到一行上）。
/// 指着的那一卷此刻不在表上时不报错，与卷表同一条。
///
/// 每一行带着它是哪一种[语义](super::paint::Tone)，出处仍是行首那个
/// [记号](Mark)——本模块一个颜色名都不写。
pub(super) fn directories(
    live: &Live,
    branches: &[Branch],
    room: u16,
    at: Option<Volume>,
) -> Table {
    // 两处**同序**（都是 `render::grouped` 出的那几组）：`zip` 因此对得上。
    let entries: Vec<Entry> = branches
        .iter()
        .zip(live.branch_rows())
        .map(|(branch, row)| Entry::of_branch(live, branch, &row))
        .collect();
    if entries.is_empty() {
        return Table {
            rows: Vec::new(),
            cursor: None,
        };
    }
    let room = usize::from(room).saturating_sub(1);
    let mut widths: Widths<DirectoryColumn> = Widths::new();
    for entry in &entries {
        for column in DirectoryColumn::ALL {
            widths.widen(*column, &entry.text(*column));
        }
    }
    let kept = columns::plan(room, &mut widths);
    let mut lines = vec![Painted::plain(columns::lay(
        &kept,
        &widths,
        |column| column.head().to_owned(),
        &[],
    ))];
    let mut cursor = None;
    for entry in &entries {
        // 光标停在这一枝底下任何一卷上，反白的都是这一行。`at` 是 `None`
        // （一卷都没选）时恒不相等。
        if at.is_some_and(|at| entry.inside.contains(&at)) {
            cursor = Some(lines.len());
        }
        lines.push(Painted::new(
            columns::lay(&kept, &widths, |column| entry.text(column), &entry.notes),
            entry.mark.tone(),
        ));
    }
    Table {
        rows: lines,
        cursor,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tonefit::Mode as RunMode;

    use super::super::probe::{screen, tight};
    use super::*;
    use crate::session::live::{Resuming, fixture};
    use crate::session::state::Session;

    /// 一趟**两枝**：甲两卷（一卷幂等命中、一卷带失败页），乙一卷。
    ///
    /// 夹具那几份卷报告的卷根恒是 `库/{name}`，名字里带一层目录就分得出两枝来
    /// （见 `fixture::skipped_volume`）。三卷的**末一段各不相同**：屏上印的是
    /// 那一段（`render::volume_name`），重名的话「另一枝的卷摊进来了」就问不出来。
    fn a_run_across_two_directories() -> Live {
        let mut live = Live::new(&fixture::request(RunMode::Process), Resuming::GoesOn);
        live.run_started(3, 3000);
        live.volume_started(Path::new("库/甲/棋魂 07"), 1000);
        live.volume_finished(&fixture::skipped_volume("甲/棋魂 07", 184));
        live.volume_started(Path::new("库/甲/棋魂 08"), 1000);
        live.volume_finished(&fixture::processed_volume(
            "甲/棋魂 08",
            Some("解不出完整尺寸：JPEG 数据截断"),
        ));
        live.volume_started(Path::new("库/乙/哆啦 03"), 1000);
        live.volume_finished(&fixture::processed_volume("乙/哆啦 03", None));
        live
    }

    /// **一枝一行，说得出几卷 · 判成哪几档 · 几卷进了隔离**（`volume-discovery/08`
    /// 票面第一条）。
    ///
    /// 一屏看得完问的正是这一条：三卷收成两行，而每一行都答得出「这一枝怎么样」。
    /// **记号不比它底下最坏的那一卷轻**：甲那一枝有一卷进了隔离，行首因此是 `!`。
    #[test]
    fn each_directory_gets_one_row_that_sums_up_its_volumes() {
        let live = a_run_across_two_directories();

        let table = directories(&live, &live.branches(), 120, None);
        let body: Vec<&str> = table
            .rows
            .iter()
            .skip(1)
            .map(|row| row.text.as_str())
            .collect();

        assert_eq!(body.len(), 2, "两枝该是两行：{body:?}");
        assert!(body[0].starts_with(" !"), "甲那一枝的记号不对：{}", body[0]);
        // 印的是**全路径**：两枝的末一段一样时，只印末一段就分不出是哪一枝。
        assert!(
            body[0].contains("库/甲") && body[0].contains('2'),
            "{}",
            body[0]
        );
        assert!(body[0].ends_with("隔离 1 卷"), "{}", body[0]);
        // 分布逐条问 `render::base_column`——跳过那一卷也在里面。
        assert!(body[0].contains("跳过 1"), "{}", body[0]);
        assert!(
            body[1].starts_with(" ✓") && body[1].contains("库/乙"),
            "{}",
            body[1]
        );
        // 逐页那几行一行都不在这一级上：那是再往下两级的事。
        assert!(
            !body.iter().any(|row| row.contains("001.jpg")),
            "目录那一级摊出了逐页：{body:?}"
        );
    }

    /// **展开一枝摊出来的只有这一枝底下那几卷**（票面第二条）。
    ///
    /// 层次与发现出来的那棵树一致：甲那一枝摊开之后，乙那一卷一行都不在。
    #[test]
    fn opening_a_branch_lists_only_the_volumes_under_it() {
        let live = a_run_across_two_directories();
        let mut session = Session::new();

        // 默认那一副：两枝各一行，一卷都没摊开。
        let folded = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(folded.contains(&tight("记号  目录")), "{folded}");
        for volume in ["棋魂 07", "棋魂 08", "哆啦 03"] {
            assert!(
                !folded.contains(&tight(volume)),
                "默认那一副摊出了卷：{folded}"
            );
        }

        // 展开甲那一枝：它底下那两卷在，乙那一卷不在。
        session.open(PathBuf::from("库/甲"));
        let opened = tight(&screen(&mut session, Some(&live), 120, 40));
        assert!(opened.contains(&tight("记号  卷名")), "{opened}");
        assert!(opened.contains(&tight("棋魂 07")), "{opened}");
        assert!(opened.contains(&tight("棋魂 08")), "{opened}");
        assert!(
            !opened.contains(&tight("哆啦 03")),
            "另一枝的卷摊进来了：{opened}"
        );
        // 抬头说清摊开的是哪一枝、它是第几枝——**印的是全路径**，
        // 与目录表那一列同一条：两枝的末一段常常一模一样。
        assert!(
            opened.contains(&tight("展开 库/甲（第 1/2 个目录）")),
            "{opened}"
        );
    }
}
