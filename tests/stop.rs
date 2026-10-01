//! 命令行上的两级停止，在**真进程**上测（ADR 0013 决定第 3 条）。
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
//! | `出/卷01.partial` 出现 | 第一卷的**写出环节**开始了 | 整个写出环节 |
//! | `出/卷01.partial` 里出现第一页 | 头一批页**编完了**，正在往里写 | 到下一批编完为止 |
//!
//! 两个相都是「一出现就恒在」的（直到那一卷改名或者被丢掉），因此**转到它成立为止**即可。
//!
//! **记号非得落在一卷的中段不可。**头一版这两条按在「第一卷改名到位」上——那一刻
//! 恰好是**卷边界**，而卷边界上就摆着做完再停那个检查点（`src/lib.rs` 的逐卷循环）：
//! 信号落在检查点前后是一枚硬币，机器一忙就总落在前面，于是第二卷压根不开工。
//! 主仓那一趟两条因此全红（`["卷01"]` 而不是 `["卷01", "卷02"]`；另一条连
//! `卷02.partial` 都等不到）。**那是用例按错了地方，不是做完再停把卷吃了。**
//!
//! # 为什么第一卷要那么多页
//!
//! 写出环节是**按批**走的：一批 `cores()` 张，整批并行编完再逐张写下去
//! （`src/lib.rs` 的 `second_pass`）。编那一段是秒级的，写那一段是毫秒级的——
//! 两下按停止之间的空当、以及第二下按下之后到立即停止真停住之间的余地，靠的都是**下一批的编**。
//! [`pages`] 因此**照这台机器的核数算**，让第一卷必定走够两批；写死一个数的话，
//! 核多过它的机器上会退化成单批，那两个空当从「一整批的编」缩到「写完剩下几页」，
//! 两个半数量级。一趟量下来的形状（12 核）记在票据
//! `.scratch/p4-parking-lot/issues/18-*.md` 的《落地记录》第八节，这里不复述。
//!
//! 后面两卷各一页：它们只用来说明「下一卷不开工」，一页就够，不必陪着跑。
//!
//! # 为什么开着 `--envelope`
//!
//! 上面两个空当都靠**写出环节里还有编**。默认路径自 ADR 0018 起一页判完当场编码
//! （`two-pass-rework/11`），写出环节只剩写——「一批的编」那个秒级空当整个没了，
//! 三个相之间只隔毫秒，第二下 `SIGINT` 赶不赶得上成了一枚硬币（五趟两红，停车场 Q670）。
//! 整卷统一灰阶那条路上写出环节照旧按批编（`src/envelope.rs`），[`spawn`] 因此显式带上
//! `--envelope`：这两条问的是「`Ctrl-C` 到底有没有变成那两个字」，两级停止的检查点
//! 两条路共用，走哪条路不改结论——改的只是记号之间隔得够不够宽。

//! # 报告末尾那一句结束方式（`say-and-stop/02`，收停车场 Q260）
//!
//! 按停之后屏上那一句走 stderr，对面不是终端时一个字节都不写；**留得下来的是 stdout 上那份报告**。
//! 两级各一条都顺带读 stdout，问报告末尾有没有那一句（措辞的出处在 `src/render.rs` 的 `outcome`，
//! 这里只认它印出来的样子）。报告按 100 格折过（对面不是终端，`src/wrap.rs`），
//! 长的那一句会被折开——比之前两边的空白都去掉（见 [`says`]）。
//!
//! # 清点途中按停：盘上没有记号，拿一串命名管道当记号
//!
//! 清点只列归档头、一个字节都不写，《按在哪一刻》那张表里
//! 一个记号都没有。记号换成**点名的命名管道**（扩展名 `.cbz`）：清点开到它时卡在 `open` 上，
//! 等一个写端；用例**非阻塞地**去开写端——开得动，说明清点正卡在那儿（键因此早已装上：
//! 它装在 `run` 之前），那一个归档头随即读到空的、点不开，清点接着走向下一个。
//! **写端要攥到下一个管道放行为止**，不能开完就关：XNU 上读端被叫醒之后要再看一眼
//! 「有没有写端」，写端在它醒来之前就关掉的话它接着睡，清点就此卡死在那个管道上
//! （头一版十趟里卡死过一趟）。
//! 按下 `Ctrl-C` 之后再一个个放行，直到进程退出：信号那条线程把那一下记进闩要一点功夫，
//! 那一点功夫由**下一个归档头之前那一问**接住（`tonefit::Event::Surveying`）。
//! 放行之前各歇几毫秒，给那条线程留出余地（量级与出处见 [`HOLES`]）——**那不是记号**，
//! 记号仍是「清点卡在第几个管道上」；
//! 余地不够的话管道会被放完，清点走到头、这一趟被拒，用例是**红**不是假绿。
//!
//! 管道是**点名的**而不是躺在一个目录里：发现只收普通文件（`src/discover.rs` 的 `push_children`），
//! 管道连候选都当不上。点名的坏归档撞上停止时**停止赢**（`tonefit::Event::Surveying` 的文档），
//! 这一趟因此交回报告而不是拒绝。

// `Ctrl-C` 在 Windows 上不是一个信号（那一头是 `SetConsoleCtrlHandler`，见 `Cargo.toml`
// 里 `ctrlc` 那一条的注释），送它要另一套 API。**装那个键**两个平台上是同一句
// `ctrlc::set_handler`，而**按它**这一半只有 unix 这一侧本仓验得到（停车场 Q263）。
#![cfg(unix)]

mod fixtures;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use fixtures::Workspace;

/// **按一次是做完再停**：当前卷跑完才停，盘上只有完整的卷（ADR 0013 决定第 1 条）。
///
/// 按在第一卷**正写着**的时候，因此这一条同时说了两件事：那一卷**写完了**并改名到位
/// （盘上是完整的一卷，不是一格 `partial`），而第二卷**一页都没开工**。
/// 少一个说明按下去把当前卷也丢了（那是立即停止，不是做完再停），多一个说明根本没停。
#[test]
fn one_ctrl_c_lets_the_volume_in_flight_finish_and_stops_there() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, "第一卷开始进写出环节", || {
        partial(&space).is_dir()
    });
    ctrl_c(&child);
    let (code, printed) = finished(child);

    // **按停止不是失败**：退出码照既有那四个走，这一趟做到的那一卷成了，交出的是 0
    // （`src/main.rs` 的 `exit_code`）。
    assert_eq!(code, Some(0), "按停止停下来的一趟没交出「全部成功」那个数");
    // **报告说得出它被按停过**，剩下哪一截（`say-and-stop/02`）：当前卷做完，后面两卷没开工。
    assert!(
        says(&printed, "按停止停下（做完再停）：剩下 2 卷没开工"),
        "stdout 上的报告没说这一趟是按停止停下的：{printed}"
    );
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["卷01"],
        "做完再停没停在卷边界上：空的是当前卷被丢了，多一个是根本没停"
    );
    // **盘上只有完整的卷**：那一卷该有的页一页不少。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        page_names("卷01"),
        "留在盘上的那一卷不是完整的"
    );
}

/// **按两次是立即停止**：当前卷那格 `partial` 丢弃，最终位置上一个字节都没动过
/// （ADR 0013 决定第 2 条）。
///
/// 两下**各按在自己那个相上**，中间隔着头一批页的编（量出来是 1.25 秒）：
///
/// 1. `卷01.partial` 出现 → 按第一下（做完再停）；
/// 2. 那格里出现第一页 → 按第二下（立即停止）。
///
/// 这个次序买下三件事。一是**那格 `partial` 真的建出来过、里面真的有页**，
/// 「丢弃」这才有东西可丢。二是**两个信号不会并成一个**：`SIGINT` 不排队，
/// 两下挨着送、第二下可能撞在第一下还没递到的时候被丢掉，而这里两下之间隔着一整批的编。
/// 三是第二下按下之后**还剩下一整批的编**，立即停止有足够的余地在下一张页写出去之前停住。
///
/// 断言输出目录是**空的**：`卷01` 没有，`卷01.partial` 也没剩下——那一卷等于没做。
#[test]
fn two_ctrl_c_throws_the_volume_in_flight_away_and_the_final_place_is_untouched() {
    let space = Workspace::new();
    let volumes = three_volumes(&space);
    let mut child = spawn(&space, &volumes);

    until(&mut child, "第一卷开始进写出环节", || {
        partial(&space).is_dir()
    });
    ctrl_c(&child);
    until(&mut child, "第一卷写出头一张页", || {
        !fixtures::names_in(&partial(&space)).is_empty()
    });
    ctrl_c(&child);
    let (code, printed) = finished(child);

    assert_eq!(code, Some(0), "按停止停下来的一趟没交出「全部成功」那个数");
    // **报告说得出丢掉的是哪一卷**、最终位置没动过、后面两卷没开工（`say-and-stop/02`）。
    let said = format!(
        "按停止停下（立即停止）：{} 做到一半丢掉了，最终位置上一个字节都没动过；剩下 2 卷没开工",
        volumes[0].display()
    );
    assert!(
        says(&printed, &said),
        "stdout 上的报告没说当前那一卷丢掉了：{printed}"
    );
    // **立即停止掉的那一卷等于没做**：最终位置上没有它，那格 `partial` 也没剩下——
    // 一格半成品都不留（`src/sink.rs` 的两个 `Drop`）。
    assert!(
        fixtures::names_in(&space.out()).is_empty(),
        "立即停止之后输出目录里还剩着东西：{:?}",
        fixtures::names_in(&space.out())
    );
}

/// **清点途中按停当场停**，报告说一卷都没开工（`say-and-stop/02`，收停车场 Q261）。
///
/// 从前清点那一段按下去停不下来：闩记住了，而第一个检查点在清点之后，几十个归档头要列完
/// 才停得住。现在每开一个归档头之前问一句，这一趟当场收手——排在管道后面的那个真卷
/// 一页都没做，stdout 上的报告以「清点途中按停止停下」收尾，退出码照按停止那一条走（0）。
///
/// 记号与余地怎么来的，见模块文档《清点途中按停》。
#[test]
fn ctrl_c_while_surveying_stops_there_and_the_report_says_no_volume_started() {
    let space = Workspace::new();
    let holes: Vec<PathBuf> = (1..=HOLES)
        .map(|n| pipe(&space, &format!("坑{n:02}.cbz")))
        .collect();
    let volume = space.volume("卷01");
    volume.page(&page_name(1), &fixtures::cheap_page());
    let mut inputs = holes.clone();
    inputs.push(volume.path().to_path_buf());
    let mut child = spawn(&space, &inputs);

    // 清点开到头一个管道上了：键早已装好。
    let mut held =
        let_through(&mut child, &holes[0]).expect("清点还没开到头一个归档头，进程就退了");
    ctrl_c(&child);
    for hole in &holes[1..] {
        std::thread::sleep(BREATHER);
        // 下一个放行了，上一个的写端才放手（模块文档《清点途中按停》）。
        match let_through(&mut child, hole) {
            Some(next) => held = next,
            None => break,
        }
    }
    drop(held);
    let (code, printed) = finished(child);

    assert_eq!(
        code,
        Some(0),
        "清点途中按停止没交出按停止那个数（管道放完了、清点走到头被拒？）：{printed}"
    );
    assert!(
        says(&printed, "清点途中按停止停下：一卷都没开工"),
        "stdout 上的报告没说清点途中停下：{printed}"
    );
    assert!(
        !space.out().join("卷01").exists(),
        "清点途中按了停，排在后面的卷照样做了"
    );
}

/// 清点途中那条用例摆几个管道。头一个当记号，按下之后剩下的那些每放行一个先歇 [`BREATHER`]，
/// 合起来是信号那条线程把那一下记进闩的余地：39 × 5 毫秒，约 0.2 秒。
///
/// **余地的量级**：`ctrlc` 的回调跑在它自己那条线程上，信号处理函数往管道里写一个字节把它叫醒
/// ——那是一次线程唤醒，空闲的机器上是微秒级；闸门那一趟所有核都在编页，排到它要等调度，
/// 量级是毫秒到几十毫秒。0.2 秒高出后者一个数量级。正常的一趟里那条线程在第二、三个管道之前
/// 就记上了，后面那些管道一个都不开（连跑二十趟，每趟整条用例约 0.7 秒，见票据《落地记录》）。
/// 余地不够时这条用例**红**（管道放完、清点走到头被拒），不会假绿。
const HOLES: usize = 40;

/// 按下之后每放行一个管道之前歇多久。见 [`HOLES`]。
const BREATHER: std::time::Duration = std::time::Duration::from_millis(5);

/// 在工作区根下造一个**命名管道**，名字照 `name`（带 `.cbz`，清点才把它当归档头去开）。
fn pipe(space: &Workspace, name: &str) -> PathBuf {
    let path = space.root().join(name);
    let raw = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).expect("路径里没有 NUL");
    // SAFETY: 一个以 NUL 收尾的路径、一个权限位；`mkfifo` 不碰别的内存。
    let made = unsafe { libc::mkfifo(raw.as_ptr(), 0o644) };
    assert_eq!(made, 0, "造不出命名管道 {}", path.display());
    path
}

/// **放行一个管道**：等到清点卡在它的 `open` 上（非阻塞地开写端开得动），交回那个写端——
/// 那一个归档头读到的是空的。写端由调用方攥着，攥到下一个放行为止（见模块文档）。
/// 进程先退了就回 `None`，一个字节都不等。
///
/// 等一分钟都等不到读端就把子进程杀掉再红：那是清点卡在了别处，转下去只会把整趟闸门挂死。
fn let_through(child: &mut Child, hole: &Path) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if child.try_wait().expect("问子进程还在不在").is_some() {
            return None;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("一分钟都没等到清点开 {}：它卡在了别处", hole.display());
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(hole)
        {
            Ok(writer) => return Some(writer),
            // 读端还没来：清点没开到这一个。歇一毫秒再问（理由同 [`until`]）。
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(error) => panic!("开 {} 的写端：{error}", hole.display()),
        }
    }
}

/// 这一趟：三个卷。第一卷 [`pages`] 张（两条用例都停在它身上，见模块文档
/// 《为什么第一卷要那么多页》），后面两卷各一页——它们只用来说明「下一卷不开工」。
///
/// **三个而不是两个**：做完再停要看得出「当前卷跑完、下一卷不开工」，而两个卷上
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
/// 页数跟着核数长不亏——分析环节与每一批的编都是并行的，核多的机器上多出来的页
/// 摊在多出来的核上，墙钟大致不动。
///
/// 问的是 [`available_parallelism`](std::thread::available_parallelism)，而库那一侧
/// 分批用的是 `num_cpus::get()`（`src/lib.rs` 的 `cores`）。两者答得出的数在本机相同；
/// 真要差一点也不怕——两倍那个余量吃得下，而万一退化成单批，
/// 这两条是**红**不是假绿（立即停止赶不上就会看见输出目录里剩着东西）。
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
/// 进度条不进测试日志；报告接成管道，由 [`finished`] 读回来（只读末尾那一句结束方式）。
/// 进度条那一头本来也不会写——对面不是终端时 indicatif 一个字节都不写
/// （`src/main.rs` 的 `Bar`）。
fn spawn(space: &Workspace, inputs: &[PathBuf]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        // 写出环节要有编，两个记号之间才隔得开——模块文档《为什么开着 `--envelope`》。
        .arg("--envelope")
        .args(inputs)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("启动 tonefit")
}

/// 等它退出，交回退出码与 stdout 上的报告。
///
/// stdout 接成管道而不是丢掉：报告末尾那一句结束方式是这几条要读的（模块文档《报告末尾那一句结束方式》）。
/// 报告不长，管道装得下，等完再读不会把子进程憋住。
fn finished(child: Child) -> (Option<i32>, String) {
    let output = child.wait_with_output().expect("等子进程");
    let printed = String::from_utf8(output.stdout).expect("报告是 UTF-8");
    (output.status.code(), printed)
}

/// 报告里**说了**这一句没有：两边的空白都去掉再比。
///
/// 报告按 100 格折过，长的那一句会被折开，折口落在空格上就吃掉那个空格、落在汉字之间就多一个换行；
/// 两边都把空白去掉，折在哪儿都比得上。
fn says(printed: &str, sentence: &str) -> bool {
    let squashed = |text: &str| text.split_whitespace().collect::<String>();
    squashed(printed).contains(&squashed(sentence))
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
