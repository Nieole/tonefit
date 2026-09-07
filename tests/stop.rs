//! 命令行上的两级停，在**真进程**上测（ADR 0013 决定第 3 条）。
//!
//! 非在进程那一层不可：这两条问的是「按下 `Ctrl-C` 之后盘上留下什么」，而 `Ctrl-C` 是一个
//! **送给进程**的信号——库那一侧的用例送不出它，也观察不到 `main` 有没有把那一级
//! 交给 `run`（`tests/exit_code.rs` 的模块文档是同一句话，那一份问的是退出码）。
//!
//! **两级各自停在哪一道边界上不由这两条断言**——那是库那一对检查点的事，
//! 在 `tests/events.rs`、`tests/resume.rs` 与 `tests/container.rs` 上早就测过了
//! （那几条是自己造一个观察者答 `Finish`／`Abort`）。这一份补的是它们够不着的那一截：
//! **`Ctrl-C` 到底有没有变成那两个字**。
//!
//! # 两条用例怎么按在准地方上
//!
//! 三个卷、每卷四页。按在哪一刻不靠睡一段猜出来的时长，靠**盘上的记号**
//! （`src/sink.rs`：两种容器都先写到 `<名字>.partial`，收尾时才改名到最终位置）：
//!
//! | 记号 | 说明这一趟走到哪儿了 |
//! |---|---|
//! | `出/卷01` 出现 | 第一卷改名到位了，第二卷正在走**第一遍** |
//! | `出/卷02.partial` 出现 | 第二卷的**第二遍**开始了，它正在往那一格里写页 |
//!
//! 两个记号都是「一出现就恒在」的（后者直到它自己改名或者被丢掉），因此**转到它成立为止**
//! 即可，转多少圈不影响结论。`sleep` 反过来是猜：短了还没走到、长了已经走过，
//! 而两种都让那一下按在错的地方。
//!
//! 转圈的时候每一圈都问一次子进程还在不在：整趟跑完了记号还没出现，说明这一趟的形状
//! 与用例设的前提不一样了——那时当场断言失败，而不是在这里转到天荒地老。

// `Ctrl-C` 在 Windows 上不是一个信号（那一头是 `SetConsoleCtrlHandler`，见 `Cargo.toml`
// 里 `ctrlc` 那一条的注释），送它要另一套 API。**装那个键**两个平台上是同一句
// `ctrlc::set_handler`，而**按它**这一半只有 unix 这一侧本仓验得到（停车场 Q263）。
#![cfg(unix)]

mod fixtures;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use fixtures::Workspace;

/// **按一次是收尾**：当前卷跑完才停，盘上多一个完整的卷（ADR 0013 决定第 1 条）。
///
/// 按在**第二卷的第一遍**里，那正是收尾唯一会被弄错的地方：那一下也要经过第二卷的
/// 决策点，而决策点问的是「这一卷的第二遍还做不做」——闩去答它的话，第二卷等于走了
/// 一次试算，盘上少的正是收尾说好要留下的那一卷（`src/main.rs` 的 `answer`）。
/// 因此这一条断言的是 `["卷01", "卷02"]`：少一个说明那一让没有让，多一个说明根本没停。
///
/// **它留着一道微秒级的缝**：第一卷改名到位与卷边界那个检查点之间要是被这一下撞上，
/// 这一趟会停在 `["卷01"]` 上而红。缝比第二卷那一整遍（秒级）小四五个数量级——
/// 记号是**改名之后**才看得见的，而看见它、再走一次 `kill` 的工夫里，那条线程早就
/// 迈过了那个检查点。而且**撞上是红、不是假绿**：这一条说不了谎，最坏是白响一次。
#[test]
fn one_ctrl_c_lets_the_volume_in_flight_finish_and_stops_there() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, &space.out().join("卷01"), "第一卷改名到位");
    ctrl_c(&child);
    let code = child.wait().expect("等子进程").code();

    // **按停不是失败**：退出码照既有那四个走，这一趟做到的每一卷都成了，交出的是 0
    // （`src/main.rs` 的 `exit_code`）。
    assert_eq!(code, Some(0), "按停停下来的一趟没交出「全部成功」那个数");
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["卷01", "卷02"],
        "收尾没停在卷边界上：少一个是当前卷被吃掉了，多一个是根本没停"
    );
    // **盘上只有完整的卷**：两卷各四页，一页不少。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        [page_names("卷01"), page_names("卷02")].concat(),
        "留在盘上的卷不是完整的"
    );
}

/// **按两次是中止**：当前卷那格 `partial` 丢弃，最终位置上一个字节都没动过
/// （ADR 0013 决定第 2 条）。
///
/// 两下**各按在自己的记号上**，中间隔着第二卷的一整遍：
///
/// 1. 第一卷改名到位 → 按第一下（收尾）；
/// 2. `卷02.partial` 出现 → 第二卷的第二遍开始了，按第二下（中止）。
///
/// 这个次序买下三件事。一是**那格 `partial` 真的建出来过**，「丢弃」这才有东西可丢——
/// 按在第一遍里的话，中止在决策点上就返回了，一格 `partial` 都还没有。
/// 二是**两个信号不会并成一个**：`SIGINT` 不排队，两下挨着送、第二下可能撞在第一下
/// 还没递到的时候被丢掉，而这里两下之间隔着一整遍。
/// 三是那个记号自己就说明第一下**收到了而且让对了**：没让的话第二卷压根走不到第二遍，
/// `卷02.partial` 永远不出现。
#[test]
fn two_ctrl_c_throws_the_volume_in_flight_away_and_the_final_place_is_untouched() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, &space.out().join("卷01"), "第一卷改名到位");
    ctrl_c(&child);
    until(
        &mut child,
        &space.out().join("卷02.partial"),
        "第二卷开始写第二遍",
    );
    ctrl_c(&child);
    let code = child.wait().expect("等子进程").code();

    assert_eq!(code, Some(0), "按停停下来的一趟没交出「全部成功」那个数");
    // **中止掉的那一卷等于没做**：最终位置上没有它，那格 `partial` 也没剩下——
    // 一格半成品都不留（`src/sink.rs` 的两个 `Drop`）。
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["卷01"],
        "中止没把当前卷那一格丢掉，或者它压根没中止"
    );
    assert_eq!(
        fixtures::directory_members(&space.out()),
        page_names("卷01"),
        "留在盘上的卷不是完整的"
    );
}

/// 这一趟：三个卷，每卷四页。
///
/// **三个而不是两个**：收尾要看得出「当前卷跑完、下一卷不开工」，而两个卷上
/// 「当前卷跑完」与「全跑完」是同一件事，那一条断言便什么都说不出。
///
/// **每卷四页而不是一页**：两条用例都要在第二卷正走着的时候按下去，而按下去的那一刻
/// 到它撞上下一个检查点之间要够走完一页。页取[最便宜的那一张](fixtures::cheap_page)——
/// 它原样穿过去，一卷仍有秒级的功夫可按，而这一份是三份夹具里最省的。
///
/// 名字带序号而不是「卷一卷二卷三」：断言比的是[排过序的清单](fixtures::names_in)，
/// 而中文数字那三个字的码位次序是一、三、二。
fn three_volumes(space: &Workspace) -> Vec<PathBuf> {
    ["卷01", "卷02", "卷03"]
        .into_iter()
        .map(|name| {
            let volume = space.volume(name);
            for page in PAGES {
                volume.page(page, &fixtures::cheap_page());
            }
            volume.path().to_path_buf()
        })
        .collect()
}

/// 一个卷有哪几页。源与输出同名，因此两处共用它。
const PAGES: [&str; 4] = ["001.png", "002.png", "003.png", "004.png"];

/// 一个卷完整地落在输出里长什么样：四页，一页不少
/// （[`fixtures::directory_members`] 那一份的形状）。
fn page_names(volume: &str) -> Vec<String> {
    PAGES
        .iter()
        .map(|page| format!("{volume}/{page}"))
        .collect()
}

/// 起一趟，**不等它**。
///
/// 报告与进度条都不进测试日志：这两条只看盘上留下什么与退出码是几。
/// 进度条那一头本来也不会写——对面不是终端时 indicatif 一个字节都不写
/// （`src/main.rs` 的 `Bar`）。
fn spawn(space: &Workspace, inputs: &[PathBuf]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        .args(inputs)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("启动 tonefit")
}

/// 转到 `mark` 出现为止（见本模块文档《两条用例怎么按在准地方上》）。
///
/// 每一圈问一次子进程还在不在：整趟已经跑完而记号没出现，说明这一趟的形状与用例设的
/// 前提不一样了——当场断言失败，而不是在这里一直转下去。
fn until(child: &mut Child, mark: &Path, what: &str) {
    while !mark.exists() {
        assert!(
            child.try_wait().expect("问子进程还在不在").is_none(),
            "整趟都跑完了也没等到「{what}」（{}）",
            mark.display()
        );
        std::thread::yield_now();
    }
}

/// 往子进程上按一下 `Ctrl-C`。
fn ctrl_c(child: &Child) {
    // SAFETY: 只是往一个还在的子进程上送一个信号。`kill` 对一个已经退掉、但还没被
    // `wait` 收走的进程同样是安全的（它还占着 pid），而下面那一句断言会把那种情形说出来。
    let sent = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(sent, 0, "SIGINT 没送出去");
}
