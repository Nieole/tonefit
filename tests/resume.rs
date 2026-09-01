//! 续做：试算与执行之间那一个决策点（会话批 06 号票，ADR 0012）。
//!
//! 断言全在 `run` 这个 seam 上：装一个在决策点上答话的观察者进去，看它答的那个字
//! 改变了什么——盘上有没有东西、报告出不出、第一遍走了几次。
//!
//! 决策点与两个**检查点**不是一回事，后者在 `tests/events.rs`：检查点问的是
//! 「这一趟还走不走」，答收尾要当前卷跑完；决策点问的是「这一卷的第二遍还做不做」，
//! 答收尾这一卷就不写了。两边共用同一个 [`Instruction`]，停出来的现场不同。
//!
//! 「等人的那段时间不算进计时」不在这里，在 `tests/timing.rs`：那是计时的性质，
//! 而那个文件是计时唯一的出处。

mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fixtures::{TINY, Volume, Workspace};
use tonefit::{CacheBudget, Event, Instruction, Mode, Pass, Progress, ProgressSink, Request};

/// 两页加一个透传文件的目录卷。
///
/// 透传成员是有意的：「答收尾就一个文件都不写」要连它一起管住——第二遍写的是**全部成员**，
/// 只看页的话，透传那个循环漏在决策点外面这条用例看不出来。
fn small_volume(space: &Workspace, name: &str) -> Volume {
    let volume = space.volume(name);
    volume.page("001.png", &fixtures::full_bleed_gradient(TINY));
    volume.page("002.png", &fixtures::full_bleed_gradient(TINY));
    volume.file("ComicInfo.xml", b"<ComicInfo/>");
    volume
}

/// 卷标识的末一段。事件与报告里的卷都是路径，用例按名字认它们。
fn named(volume: &Path) -> String {
    volume
        .file_name()
        .expect("卷路径有末一段")
        .to_string_lossy()
        .into_owned()
}

/// 在决策点上答一个字的观察者，其余每一条事件一律回继续。
///
/// 点名卷是为了让答的那个字**落得准**：一趟里每一卷各有一个决策点，不点名就只按得到
/// 头一卷那个。`None` 即每一卷都按。
#[derive(Clone)]
struct AtTheDecisionPoint {
    /// 输出根。每到一个决策点就看它一眼。
    out: PathBuf,
    /// 只在这个卷的决策点上答。`None` 即每一卷都答。
    volume: Option<String>,
    /// 答的是哪个字。
    instruction: Instruction,
    /// 观察者要交给库，用例又要读回它看见的那些，因此这一格共享。
    seen: Arc<Mutex<Seen>>,
}

#[derive(Default)]
struct Seen {
    /// 当前这一卷叫什么。
    current: String,
    /// 每一遍开工时是哪一卷的哪一遍，按到达顺序。决策点问了几次、问在哪儿，看它。
    passes: Vec<(String, Pass)>,
    /// 每到一个决策点时输出根下有哪些名字，按到达顺序。
    ///
    /// 「第二遍还没开始」只有这一眼看得出来：跑完再看只看得见最终的样子，
    /// 而「这一卷此刻还没落盘」与「它落过盘又被收走了」在那个形式上分不开
    /// （同一个手法见 `tests/events.rs` 的 `StopsAtAPageBoundary`）。
    at_each_decision_point: Vec<Vec<String>>,
    /// 报了「一卷跑完」的卷，按到达顺序。
    finished: Vec<String>,
}

impl AtTheDecisionPoint {
    fn new(space: &Workspace, volume: Option<&str>, instruction: Instruction) -> Self {
        Self {
            out: space.out(),
            volume: volume.map(str::to_owned),
            instruction,
            seen: Arc::new(Mutex::new(Seen::default())),
        }
    }

    /// 一趟里走过的那几遍，`(卷名, 遍)`，按到达顺序。
    fn passes(&self) -> Vec<(String, Pass)> {
        self.seen.lock().expect("记账没有中毒").passes.clone()
    }

    /// 每一个决策点到来的那一刻输出根下有哪些名字。
    fn at_each_decision_point(&self) -> Vec<Vec<String>> {
        self.seen
            .lock()
            .expect("记账没有中毒")
            .at_each_decision_point
            .clone()
    }

    /// 报了「一卷跑完」的那几卷。
    fn finished(&self) -> Vec<String> {
        self.seen.lock().expect("记账没有中毒").finished.clone()
    }
}

impl Progress for AtTheDecisionPoint {
    fn observe(&self, event: Event<'_>) -> Instruction {
        let mut seen = self.seen.lock().expect("记账没有中毒");
        match event {
            Event::VolumeStarted { volume, .. } => seen.current = named(volume),
            Event::PassStarted { pass, .. } => {
                let current = seen.current.clone();
                seen.passes.push((current.clone(), pass));
                if pass == Pass::Second {
                    let names = fixtures::names_in(&self.out);
                    seen.at_each_decision_point.push(names);
                    if self.volume.as_deref().is_none_or(|named| named == current) {
                        return self.instruction;
                    }
                }
            }
            Event::VolumeFinished { report, .. } => seen.finished.push(named(&report.volume)),
            _ => {}
        }
        Instruction::Continue
    }
}

/// 一趟带着观察者的处理。
fn run_with(
    space: &Workspace,
    volumes: &[Volume],
    watcher: &AtTheDecisionPoint,
) -> tonefit::Report {
    tonefit::run(&Request {
        progress: Some(ProgressSink::new(watcher.clone())),
        ..fixtures::request(space, volumes.iter().map(Volume::path))
    })
    .expect("在决策点上停下来不是失败")
}

/// 决策点**每一卷各一个**，且落在「汇总之后、第二遍之前」（ADR 0012 决定第 2 条）。
///
/// 「第二遍之前」由**那一眼**钉住：第一个决策点到来时输出根还是空的，第二个到来时里面
/// 只有上一卷。决策点要是挪到了第二遍之内（哪怕只挪过建容器那一句），当前卷的那格
/// `partial` 就会出现在这一眼里。
///
/// 「每卷各一个」由遍的清单钉住：三个卷各走三遍，第二遍那一条一卷不多、一卷不少。
/// 少一条，那一卷根本没问过人；多一条，人会被同一卷问两遍。
#[test]
fn every_volume_gets_one_decision_point_before_a_byte_of_it_is_written() {
    let space = Workspace::new();
    let volumes: Vec<Volume> = ["volume-a", "volume-b", "volume-c"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    // 每一卷都答继续：这一条问的是决策点在哪儿、有几个，不是它能不能停下来。
    let watcher = AtTheDecisionPoint::new(&space, None, Instruction::Continue);

    let report = run_with(&space, &volumes, &watcher);

    assert_eq!(report.volumes.len(), 3, "三个卷没有都做完");
    assert_eq!(
        watcher.passes(),
        [
            ("volume-a".to_owned(), Pass::Fingerprint),
            ("volume-a".to_owned(), Pass::First),
            ("volume-a".to_owned(), Pass::Second),
            ("volume-b".to_owned(), Pass::Fingerprint),
            ("volume-b".to_owned(), Pass::First),
            ("volume-b".to_owned(), Pass::Second),
            ("volume-c".to_owned(), Pass::Fingerprint),
            ("volume-c".to_owned(), Pass::First),
            ("volume-c".to_owned(), Pass::Second),
        ],
        "决策点不是每一卷各一个"
    );
    assert_eq!(
        watcher.at_each_decision_point(),
        [
            Vec::<String>::new(),
            vec!["volume-a".to_owned()],
            vec!["volume-a".to_owned(), "volume-b".to_owned()],
        ],
        "决策点到来时当前卷已经在盘上了：它排到第二遍之内去了"
    );
}

/// 单卷答**收尾**：停在这儿，输出根一个文件都没有，而报告照出（ADR 0012 决定第 2 条）。
///
/// 那正是 dry-run 的效果，而这份报告正是试算要看的那份东西——判定、逐页结果、解码计数
/// 一样不少，只有第二遍那一段是零。三件事一起断言：`run` 回的是 `Ok`（停下来不是失败）、
/// 盘上什么都没有、报告是完整的。
#[test]
fn finishing_at_the_decision_point_writes_nothing_and_still_reports_the_volume() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let watcher = AtTheDecisionPoint::new(&space, None, Instruction::Finish);

    let report = run_with(&space, std::slice::from_ref(&volume), &watcher);

    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "答收尾之后输出根下还留着东西"
    );
    assert!(!space.out().exists(), "答收尾之后输出根被建了出来");

    assert_eq!(report.volumes.len(), 1, "答收尾把报告也一起停掉了");
    assert_eq!(
        watcher.finished(),
        ["volume-a"],
        "停在决策点上的那一卷没报「一卷跑完」"
    );
    let stopped = &report.volumes[0];
    assert!(!stopped.skipped(), "这一卷被幂等跳过了，测的就不是决策点了");
    assert_eq!(stopped.page_count(), 2, "报告里没有逐页结果");
    assert!(
        stopped.verdict.is_some(),
        "汇总没做就停下来了：报告没有卷级判定"
    );
    assert_eq!(stopped.decodes, stopped.source_pages, "第一遍没有走完");
    assert_eq!(
        stopped.timing.second_pass,
        std::time::Duration::ZERO,
        "答收尾之后第二遍还是走了"
    );
    // 报到那一侧只多出一条：决策点自己。第二遍一步都没走。
    assert_eq!(
        watcher.passes(),
        [
            ("volume-a".to_owned(), Pass::Fingerprint),
            ("volume-a".to_owned(), Pass::First),
            ("volume-a".to_owned(), Pass::Second),
        ],
        "遍的清单与「第二遍没走」对不上"
    );

    // 这一趟的收场：点名的卷进了报告，因此说的是**走到头**。
    // 它是停车场 Q53 记着的那一件——同一个手势，单卷收「走到头」、多卷收「停在半路」
    // （多卷那一半由本文件的
    // `finishing_at_one_volume_decision_point_leaves_the_earlier_volumes_whole_and_starts_no_more`
    // 钉着）。钉在这里不是主张它一定对，是为了让它**变的那一天有人看见**：
    // Q53 的答案落下来时，红的应该是这一行，而不是某个会话里画错的一屏。
    assert_eq!(
        report.outcome,
        tonefit::RunOutcome::Completed,
        "单卷停在决策点上，这一趟的收场变了（停车场 Q53）"
    );
}

/// 决策点上答**中止**：那一卷等于没做，报告里没有它（ADR 0013 决定第 2 条）。
///
/// 它与答收尾在同一处按下，停出来的现场却相反——**分得开才有意义**：
/// 收尾留下一份报告，中止连报告都没有。
///
/// 「输出容器连建都不建」是本票带来的行为变化，这一条钉的就是它：从前决策点还不认这个字，
/// 第二遍照样开工，`Sink::create` 先建出那格 `partial` 再由析构丢掉。现在那一格压根不出现，
/// 而**在决策点上看的那一眼**是它唯一测得到的形式——跑完再看，「从没建过」与
/// 「建了又丢掉」分不开。
#[test]
fn aborting_at_the_decision_point_leaves_the_volume_out_of_the_report_entirely() {
    let space = Workspace::new();
    let volumes: Vec<Volume> = ["volume-a", "volume-b"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let watcher = AtTheDecisionPoint::new(&space, Some("volume-a"), Instruction::Abort);

    let report = run_with(&space, &volumes, &watcher);

    assert!(report.volumes.is_empty(), "被中止的那一卷进了报告");
    assert_eq!(
        watcher.finished(),
        Vec::<String>::new(),
        "它报了「一卷跑完」"
    );
    assert_eq!(
        fixtures::names_in(&space.out()),
        Vec::<String>::new(),
        "中止之后输出根下还留着东西"
    );
    assert_eq!(
        report.outcome,
        tonefit::RunOutcome::Stopped(Instruction::Abort),
        "这一趟没说自己是被中止的"
    );
    // 决策点到来的那一刻输出根还是空的，之后也没有第二个决策点：
    // 那一格 `partial` 从没建过，下一卷也没开工。
    assert_eq!(
        watcher.at_each_decision_point(),
        [Vec::<String>::new()],
        "中止之后还有卷走到了决策点，或者当前卷已经建出了 partial"
    );
    assert_eq!(
        watcher
            .passes()
            .iter()
            .filter(|(volume, _)| volume == "volume-b")
            .count(),
        0,
        "中止之后下一卷开工了"
    );
}

/// 单卷答**继续**：第一遍只走一次，而分成两趟跑要走两次（ADR 0012 的《背景》）。
///
/// 「贵的那一遍不白跑两次」是续做存在的理由，而 `decodes` 是它唯一量得出来的形式
/// （ADR 0005：解码一次，缓存缩放后的图）。对照的那一组是**没有续做时会发生的事**：
/// 先一趟 dry-run 看报告、满意了再一趟照做——两趟加起来解码两遍。
///
/// 只断言「续做那一趟解码了几次」的话，这条用例在管线退化成「每趟各解一遍」时照样绿：
/// 那时它量的只是一趟的开销。要有对照，省下的那一遍才说得出口。
#[test]
fn resuming_walks_the_expensive_pass_once_where_two_runs_walk_it_twice() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let watcher = AtTheDecisionPoint::new(&space, None, Instruction::Continue);

    let resumed = run_with(&space, std::slice::from_ref(&volume), &watcher);

    let done = &resumed.volumes[0];
    assert_eq!(done.decodes, done.source_pages, "续做那一趟解码了不止一遍");
    assert_eq!(
        watcher
            .passes()
            .iter()
            .filter(|(_, pass)| *pass == Pass::First)
            .count(),
        1,
        "第一遍走了不止一次"
    );
    assert_eq!(
        fixtures::directory_members(&done.output),
        ["001.png", "002.png", "ComicInfo.xml"],
        "答继续之后这一卷没有写全"
    );

    // 对照：不续做的那条路。试算一趟、照做一趟，同一批源页解码两遍。
    let apart = Workspace::new();
    let volume = small_volume(&apart, "volume-a");
    let trial = tonefit::run(&Request {
        mode: Mode::DryRun,
        ..fixtures::request(&apart, [volume.path()])
    })
    .expect("试算应当成功");
    let execution = fixtures::run_volume(&apart, &volume);
    assert_eq!(
        trial.volumes[0].decodes + execution.volumes[0].decodes,
        2 * done.source_pages,
        "分成两趟跑没有付两遍的价，这条用例的对照组不成立"
    );
}

/// 多卷：每一卷各问一次，答收尾的那一卷停在决策点上，**剩下的卷一个都不开工**。
///
/// 三件事一起断言，因为它们合起来才是「多卷不续做」的样子（ADR 0012 决定第 1 条）：
///
/// - 前一卷照旧写出去了——决策点是**这一卷**的事，不牵连已经做完的；
/// - 答收尾的那一卷有报告、盘上没有它；
/// - 后一卷连开卷都没有，收场说的是被按停停在半路（卷边界那个检查点接手）。
///
/// **缓存逐卷建、逐卷丢**也在这里量：每一卷报出来的缓存页数只有自己那几页。
/// 缓存要是跨卷活着，第二卷会报出两卷的量——而那正是 ADR 0012 决定第 1 条
/// 拿来否掉「多卷也续做」的那个理由（预算是**每卷**的）。
#[test]
fn finishing_at_one_volume_decision_point_leaves_the_earlier_volumes_whole_and_starts_no_more() {
    let space = Workspace::new();
    let volumes: Vec<Volume> = ["volume-a", "volume-b", "volume-c"]
        .into_iter()
        .map(|name| small_volume(&space, name))
        .collect();
    let watcher = AtTheDecisionPoint::new(&space, Some("volume-b"), Instruction::Finish);

    let report = run_with(&space, &volumes, &watcher);

    assert_eq!(
        fixtures::names_in(&space.out()),
        ["volume-a"],
        "盘上不是只有决策点之前做完的那一卷"
    );
    assert_eq!(
        report
            .volumes
            .iter()
            .map(|volume| named(&volume.volume))
            .collect::<Vec<_>>(),
        ["volume-a", "volume-b"],
        "报告里不是前两卷"
    );
    assert_eq!(
        watcher
            .passes()
            .iter()
            .filter(|(volume, _)| volume == "volume-c")
            .count(),
        0,
        "第三卷开工了：决策点上按下的那个字没有拦住卷边界"
    );
    assert_eq!(
        report.outcome,
        tonefit::RunOutcome::Stopped(Instruction::Finish),
        "后面还有卷没做，这一趟却说自己走到头了"
    );

    // 缓存逐卷建、逐卷丢：每一卷只报自己那两页。
    for volume in &report.volumes {
        assert_eq!(
            volume.cache.pages,
            2,
            "{} 报出来的缓存不是自己这一卷的",
            named(&volume.volume)
        );
    }
}

/// 命令行那一路的 `--dry-run` 一字不变：没有第二遍，也就**没有决策点**。
///
/// dry-run 走 `Retention::Account`，留下的字节没人取，也就没有下一步可续
/// （ADR 0012 决定第 5 条）。在它那条路上报一个决策点出去，会话就得替一个续不了的问题
/// 想一个答案，而命令行这一路那个问题根本不存在。
///
/// 用量照旧预告得出：预算为零时它照说会溢写多少——那是 `--cache-budget` 要确认的东西，
/// 与建不建临时文件是两件事（后者在 `src/cache.rs` 的
/// `an_accounting_only_cache_measures_everything_and_keeps_nothing`）。
#[test]
fn a_dry_run_has_no_second_pass_and_therefore_no_decision_point() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let watcher = AtTheDecisionPoint::new(&space, None, Instruction::Finish);

    let report = tonefit::run(&Request {
        mode: Mode::DryRun,
        cache_budget: CacheBudget::new(0),
        progress: Some(ProgressSink::new(watcher.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("试算应当成功");

    assert_eq!(
        watcher.passes(),
        [
            ("volume-a".to_owned(), Pass::Fingerprint),
            ("volume-a".to_owned(), Pass::First),
        ],
        "dry-run 报了第二遍：决策点漏到命令行这一路上了"
    );
    assert!(!space.out().exists(), "dry-run 在输出根下留了东西");
    assert!(
        report.volumes[0].cache.spilled > 0,
        "预算为零，用量却没预告出溢写"
    );
}

/// 试算那条路上「不写**输出**」成立，而越过预算的页照样溢写（ADR 0012 决定第 5 条）。
///
/// 会话单卷那条路走的是 `Mode::Process`（`Retention::Keep`）——参照要留着，人答继续时
/// 第二遍从缓存里取。dry-run 那句「一个文件都不落盘」在这条路上因此重述为
/// **「不写输出」**：越过预算的页仍建溢写临时文件，运行结束即收走
/// （收走那一半在 `src/cache.rs` 的 `a_spilled_cache_leaves_no_file_behind`）。
///
/// 预算取零是为了逼出溢写：它让每一页都留不住，这条路上「有临时文件、没有输出」
/// 两件事因此同时在场。
#[test]
fn the_trial_path_spills_over_budget_pages_and_still_writes_no_output() {
    let space = Workspace::new();
    let volume = small_volume(&space, "volume-a");
    let watcher = AtTheDecisionPoint::new(&space, None, Instruction::Finish);

    let report = tonefit::run(&Request {
        cache_budget: CacheBudget::new(0),
        progress: Some(ProgressSink::new(watcher.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("在决策点上停下来不是失败");

    let stopped = &report.volumes[0];
    assert!(stopped.cache.spilled > 0, "预算为零却一页都没溢写");
    assert_eq!(stopped.cache.resident, 0, "预算为零却还有页留在内存里");
    assert!(!space.out().exists(), "试算那条路写了输出");
}
