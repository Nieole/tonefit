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
//! # 按在哪一刻：盘上的记号，不是猜出来的时长
//!
//! 记号只有一个，就是**第一卷那格临时容器**（`src/sink.rs`：两种容器都先写到
//! `<名字>.partial`，收尾时才改名到最终位置）。它有两个看得见的相：
//!
//! | 记号 | 说明这一趟走到哪儿了 | 它开着多久 |
//! |---|---|---|
//! | `出/卷01.partial` 出现 | 第一卷的**第二遍**开始了 | 整个第二遍 |
//! | `出/卷01.partial` 里出现第一页 | 头一批页**编完了**，正在往里写 | 到下一批编完为止 |
//!
//! 两个相都是「一出现就恒在」的（直到那一卷改名或者被丢掉），因此**转到它成立为止**即可。
//!
//! **记号非得落在一卷的中段不可。**头一版这两条按在「第一卷改名到位」上——那一刻
//! 恰好是**卷边界**，而卷边界上就摆着收尾那个检查点（`src/lib.rs` 的逐卷循环）：
//! 信号落在检查点前后是一枚硬币，机器一忙就总落在前面，于是第二卷压根不开工。
//! 主仓那一趟两条因此全红（`["卷01"]` 而不是 `["卷01", "卷02"]`；另一条连
//! `卷02.partial` 都等不到）。**那是用例按错了地方，不是收尾把卷吃了。**
//!
//! # 为什么第一卷要那么多页
//!
//! 第二遍是**按批**走的：一批 `cores()` 张，整批并行编完再逐张写下去
//! （`src/lib.rs` 的 `second_pass`）。编那一段是秒级的，写那一段是毫秒级的——
//! 两下按停之间的空当、以及第二下按下之后到中止真停住之间的余地，靠的都是**下一批的编**。
//! [`pages`] 因此**照这台机器的核数算**，让第一卷必定走够两批；写死一个数的话，
//! 核多过它的机器上会退化成单批，那两个空当从「一整批的编」缩到「写完剩下几页」，
//! 两个半数量级。一趟量下来的形状（12 核）记在票据
//! `.scratch/p4-parking-lot/issues/18-*.md` 的《落地记录》第八节，这里不复述。
//!
//! 后面两卷各一页：它们只用来说明「下一卷不开工」，一页就够，不必陪着跑。

// `Ctrl-C` 在 Windows 上不是一个信号（那一头是 `SetConsoleCtrlHandler`，见 `Cargo.toml`
// 里 `ctrlc` 那一条的注释），送它要另一套 API。**装那个键**两个平台上是同一句
// `ctrlc::set_handler`，而**按它**这一半只有 unix 这一侧本仓验得到（停车场 Q263）。
#![cfg(unix)]

mod fixtures;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use fixtures::Workspace;

/// **按一次是收尾**：当前卷跑完才停，盘上只有完整的卷（ADR 0013 决定第 1 条）。
///
/// 按在第一卷**正写着**的时候，因此这一条同时说了两件事：那一卷**写完了**并改名到位
/// （盘上是完整的一卷，不是一格 `partial`），而第二卷**一页都没开工**。
/// 少一个说明按下去把当前卷也丢了（那是中止，不是收尾），多一个说明根本没停。
#[test]
fn one_ctrl_c_lets_the_volume_in_flight_finish_and_stops_there() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, "第一卷开始写第二遍", || {
        partial(&space).is_dir()
    });
    ctrl_c(&child);
    let code = child.wait().expect("等子进程").code();

    // **按停不是失败**：退出码照既有那四个走，这一趟做到的那一卷成了，交出的是 0
    // （`src/main.rs` 的 `exit_code`）。
    assert_eq!(code, Some(0), "按停停下来的一趟没交出「全部成功」那个数");
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["卷01"],
        "收尾没停在卷边界上：空的是当前卷被丢了，多一个是根本没停"
    );
    // **盘上只有完整的卷**：那一卷该有的页一页不少。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        page_names("卷01"),
        "留在盘上的那一卷不是完整的"
    );
}

/// **按两次是中止**：当前卷那格 `partial` 丢弃，最终位置上一个字节都没动过
/// （ADR 0013 决定第 2 条）。
///
/// 两下**各按在自己那个相上**，中间隔着头一批页的编（量出来是 1.25 秒）：
///
/// 1. `卷01.partial` 出现 → 按第一下（收尾）；
/// 2. 那格里出现第一页 → 按第二下（中止）。
///
/// 这个次序买下三件事。一是**那格 `partial` 真的建出来过、里面真的有页**，
/// 「丢弃」这才有东西可丢。二是**两个信号不会并成一个**：`SIGINT` 不排队，
/// 两下挨着送、第二下可能撞在第一下还没递到的时候被丢掉，而这里两下之间隔着一整批的编。
/// 三是第二下按下之后**还剩下一整批的编**，中止有足够的余地在下一张页写出去之前停住。
///
/// 断言输出根是**空的**：`卷01` 没有，`卷01.partial` 也没剩下——那一卷等于没做。
#[test]
fn two_ctrl_c_throws_the_volume_in_flight_away_and_the_final_place_is_untouched() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, "第一卷开始写第二遍", || {
        partial(&space).is_dir()
    });
    ctrl_c(&child);
    until(&mut child, "第一卷写出头一张页", || {
        !fixtures::names_in(&partial(&space)).is_empty()
    });
    ctrl_c(&child);
    let code = child.wait().expect("等子进程").code();

    assert_eq!(code, Some(0), "按停停下来的一趟没交出「全部成功」那个数");
    // **中止掉的那一卷等于没做**：最终位置上没有它，那格 `partial` 也没剩下——
    // 一格半成品都不留（`src/sink.rs` 的两个 `Drop`）。
    assert!(
        fixtures::names_in(&space.out()).is_empty(),
        "中止之后输出根里还剩着东西：{:?}",
        fixtures::names_in(&space.out())
    );
}

/// 这一趟：三个卷。第一卷 [`pages`] 张（两条用例都停在它身上，见模块文档
/// 《为什么第一卷要那么多页》），后面两卷各一页——它们只用来说明「下一卷不开工」。
///
/// **三个而不是两个**：收尾要看得出「当前卷跑完、下一卷不开工」，而两个卷上
/// 「当前卷跑完」与「全跑完」是同一件事，那一条断言便什么都说不出。
///
/// 页取[最便宜的那一张](fixtures::cheap_page)：它原样穿过去，是这批夹具里最省的一张，
/// 而两条用例问的都不是几何。
///
/// 名字带序号而不是「卷一卷二卷三」：断言比的是[排过序的清单](fixtures::names_in)，
/// 而中文数字那三个字的码位次序是一、三、二。
fn three_volumes(space: &Workspace) -> Vec<PathBuf> {
    ["卷01", "卷02", "卷03"]
        .into_iter()
        .map(|name| {
            let volume = space.volume(name);
            let pages = if name == "卷01" { pages() } else { 1 };
            for page in 1..=pages {
                volume.page(&page_name(page), &fixtures::cheap_page());
            }
            volume.path().to_path_buf()
        })
        .collect()
}

/// 第一卷有几页：**这台机器核数的两倍**，至少八张。
///
/// 为什么非得多过核数，见模块文档《为什么第一卷要那么多页》。取两倍而不是「核数 + 1」：
/// 那样第二批只有一张，而两个空当要的是**那一批编得够久**，一批只剩一张就短掉了。
/// 页数跟着核数长不亏——第一遍与每一批的编都是并行的，核多的机器上多出来的页
/// 摊在多出来的核上，墙钟大致不动。
///
/// 问的是 [`available_parallelism`](std::thread::available_parallelism)，而库那一侧
/// 分批用的是 `num_cpus::get()`（`src/lib.rs` 的 `cores`）。两者答得出的数在本机相同；
/// 真要差一点也不怕——两倍那个余量吃得下，而万一退化成单批，
/// 这两条是**红**不是假绿（中止赶不上就会看见输出根里剩着东西）。
fn pages() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    (cores * 2).max(8)
}

/// 第几页叫什么。源与输出同名，因此两处共用它。
fn page_name(page: usize) -> String {
    format!("{page:03}.png")
}

/// 第一卷完整地落在输出里长什么样：[`pages`] 张，一页不少
/// （[`fixtures::directory_members`] 那一份的形状）。
fn page_names(volume: &str) -> Vec<String> {
    (1..=pages())
        .map(|page| format!("{volume}/{}", page_name(page)))
        .collect()
}

/// 第一卷那格临时容器（`src/sink.rs` 的 `partial_path`）。
fn partial(space: &Workspace) -> PathBuf {
    space.out().join("卷01.partial")
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

/// 转到 `mark` 成立为止（见模块文档《按在哪一刻》）。
///
/// 每一圈问一次子进程还在不在：整趟已经跑完而记号没出现，说明这一趟的形状与用例设的
/// 前提不一样了——当场断言失败，而不是在这里一直转下去。
///
/// 圈与圈之间歇一毫秒。**那不是「睡一段猜出来的时长」**——等的仍旧是那个记号，
/// 歇多久都不改变结论；歇一下只为**不把一个核空转掉**：子进程正想拿满所有核编页，
/// 而这两条用例是并排跑的。一毫秒对两个秒级的窗口来说是零头。
fn until(child: &mut Child, what: &str, mark: impl Fn() -> bool) {
    while !mark() {
        assert!(
            child.try_wait().expect("问子进程还在不在").is_none(),
            "整趟都跑完了也没等到「{what}」"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// 往子进程上按一下 `Ctrl-C`。
fn ctrl_c(child: &Child) {
    // SAFETY: 只是往一个还在的子进程上送一个信号。`kill` 对一个已经退掉、但还没被
    // `wait` 收走的进程同样是安全的（它还占着 pid），而下面那一句断言会把那种情形说出来。
    let sent = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(sent, 0, "SIGINT 没送出去");
}
