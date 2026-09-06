//! 退出码，在**真进程**上测。
//!
//! 这一条只有在进程那一层才成立：`exit_code` 那个纯函数说得出该返回几，说不出 `main`
//! 有没有把它交出去。spec 的 story 33 要的是「测试不必启动子进程」，不是「一律不许」——
//! 退出码本身就是进程那一层的事实，别处观察不到。为退出码启动子进程的用例只在这一份里。
//!
//! **印出去的那几个字节同样只有真进程看得见**，两条也在这一份里：拒绝那句话落到 stderr
//! 上时记号中间是不是一个普通空格，以及 `--help` 重定向出去之后折到多宽。
//! 两者都是「`main` 交出去的到底是什么」，纯函数那一层问不出来。

mod fixtures;

use std::path::Path;
use std::process::{Command, Stdio};

use fixtures::Workspace;

/// 四种结局各有各的退出码：做完了、做完了但有卷被隔离、做完了但有卷没做成、
/// 这一趟没做成（12 号票立的规矩，05 号票加的第四个数）。
#[test]
fn the_exit_code_tells_the_four_ways_a_run_can_end_apart() {
    let space = Workspace::new();
    let clean = space.volume("volume-a");
    clean.page("001.png", &fixtures::gradient(fixtures::TINY));
    let isolated = space.volume("volume-b");
    isolated.page("001.png", &fixtures::gradient(fixtures::TINY));
    isolated.file("002.png", b"not a png at all");
    // 一个像素都救不回来的页同样是失败页（04 号票）：它单独一卷，退出码要跟着变。
    // 它此前是一张「正常页」——整趟做完、退出码 0，脚本什么都察觉不到。
    let salvages_nothing = space.volume("volume-c");
    salvages_nothing.page("001.png", &fixtures::gradient(fixtures::TINY));
    salvages_nothing.file("002.png", &fixtures::salvages_nothing_page(fixtures::TINY));

    // 预扫时打得开、轮到它时做不成的卷（05 号票）。造它的是一个**读不出字节的透传成员**：
    // 归档结构完好，中央目录列得出它，坏的是那一个成员——只有真去读才看得出来。
    // 透传文件没有页那条出路，搬不动就交不出这一卷（`CONTEXT.md` 的《失败》）。
    let mut failed = space.cbz("volume-d");
    failed
        .page("001.png", &fixtures::gradient(fixtures::TINY))
        .rotten_file("ComicInfo.xml", b"<?xml version=\"1.0\"?>");
    let failed = failed.write();

    assert_eq!(
        tonefit(&space, &[clean.path()]),
        Some(0),
        "干净的一趟不是 0"
    );
    assert_eq!(
        tonefit(&space, &[isolated.path()]),
        Some(2),
        "有卷被隔离的一趟没和干净的那一趟分开"
    );
    assert_eq!(
        tonefit(&space, &[salvages_nothing.path()]),
        Some(2),
        "一个像素都没救回来的页没让退出码反映失败"
    );
    // 有卷没做成是第三个数：那一趟**做完了**——其余卷照做、报告照出——只是有卷没交出来。
    // 它与隔离那一个分开，因为脚本据此做的是两个不同的决定：带着坏页的卷可以先收下，
    // 根本没做成的卷得先去查文件还在不在。
    assert_eq!(
        tonefit(&space, &[failed.as_path()]),
        Some(3),
        "有卷没做成的一趟没和有卷被隔离的那一趟分开"
    );
    // **两件事同时成立时取 3**：一个进程只交得出一个数，报更重的那一件。
    assert_eq!(
        tonefit(&space, &[isolated.path(), failed.as_path()]),
        Some(3),
        "有卷被隔离又有卷没做成时报的不是更重的那一个"
    );
    // 拒绝执行是第四个数，不能和上面几个混在一起：那一趟根本没做成，一页都没做。
    // 点一个不存在的卷——它落在源那一侧，不会先撞上「输出与源卷相互嵌套」那道拒绝。
    assert_eq!(
        tonefit(&space, &[&clean.path().join("根本不存在的卷")]),
        Some(1),
        "拒绝执行的一趟不是 1"
    );
}

/// **同一个坏归档，点名它得 `1`，发现它得 `0`**（ADR 0014 决定第 5 条）。
///
/// 「点名的 / 发现的」只决定这一件事——点不开时的处置——而处置的差别就是这个数：
/// 点名的整趟拒绝（他明说了要处理它），发现的记下来、其余卷照常跑完。
/// 对推测出来的东西不用最重的处置，一个坏 zip 不该把整座库挡在门外。
///
/// 两趟点的是**同一个文件**，差别只在点名的是它自己还是装着它的那个目录。
#[test]
fn a_broken_archive_is_refused_when_named_and_skipped_when_discovered() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();
    let mut broken = fixtures::Cbz::new(library.join("坏的.cbz"));
    broken.page("001.png", &fixtures::cheap_page());
    // 中央目录与尾记录都不见了：归档结构根本读不出来。
    let broken = broken.write_truncated();

    assert_eq!(
        tonefit(&space, &[broken.as_path()]),
        Some(1),
        "点名一个点不开的归档，这一趟该整个被拒"
    );
    assert_eq!(
        tonefit(&space, &[library.as_path()]),
        Some(0),
        "发现出来的一个坏归档把整趟拖下了水"
    );
    // 「其余卷照常跑完」不只是退出码：好的那一卷真在盘上。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照常跑完"
    );
}

/// **非卷文件一整张表也不动退出码**（ADR 0014 决定第 3、5 条，`volume-discovery/04`）。
///
/// 三类一次全摆上：卷架上的 txt、一页都没有的归档、发现出来但点不开的归档。
/// 这一趟因此有一份不空的清单，而脚本那一侧看到的与全部成功一模一样——
/// 它们既没被转，也不是「没做成」，只是不属于这一趟的产物（`CONTEXT.md` 的《失败》）。
///
/// 非在真进程上问不可：清单不空这件事在库那一侧断言得了（见 `tests/discovery.rs`），
/// 而「四个码一格不动」只有退出码说得出，退出码只在进程那一层观察得到。
#[test]
fn a_list_of_non_volume_files_never_changes_the_exit_code() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();
    // ① 卷架上既不是页也不是归档的文件。
    std::fs::write(library.join("答案.txt"), b"a note the owner left here").expect("摆一份 txt");
    // ② 一页都没有的归档。
    let mut fonts = fixtures::Cbz::new(library.join("字体包.zip"));
    fonts.file("readme.txt", b"no pages in here");
    fonts.write();
    // ③ 发现出来但点不开的归档。
    let mut broken = fixtures::Cbz::new(library.join("坏的.cbz"));
    broken.page("001.png", &fixtures::cheap_page());
    broken.write_truncated();

    assert_eq!(
        tonefit(&space, &[library.as_path()]),
        Some(0),
        "一份不空的非卷文件清单改了退出码"
    );
    // 「其余照做」不只是退出码：好的那一卷真在盘上，那三个一个字节都没有。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "非卷文件跟着进了输出，或者好的那一卷没做"
    );
}

/// **摊不下就是卷级失败，其余卷照做，退出码 `3`**（`volume-discovery/05`，ADR 0015）。
///
/// 固实归档开工前要整卷摊到系统临时目录，而这一趟的临时目录**根本不在**——
/// 子进程的 `TMP` / `TEMP` / `TMPDIR` 指着一个没建出来的路径。「磁盘不够」在用例里造不出来，
/// 而它与这一种走的是同一条路（`source::extract` 的每一个 `Err`）：摊开这一步失败，
/// 那一卷交不出来。
///
/// 非在真进程上问不可，理由与本文件其余几条同一个：退出码只在进程那一层观察得到，
/// 而 `TMP` 是进程级的东西——在库那一侧改它会波及同时在跑的别的用例。
#[test]
fn a_volume_that_cannot_be_extracted_fails_alone_and_the_run_ends_with_three() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let mut solid = fixtures::SevenZip::new(library.join("固实的.7z"));
    solid.page("001.png", &fixtures::gradient(fixtures::TINY));
    solid.write();
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::gradient(fixtures::TINY));
    good.write();
    // 建都没建出来：`tempfile` 在它底下建不出目录，摊开那一步于是当场失败。
    let nowhere = space.dir("没有这个临时目录");

    assert_eq!(
        tonefit_with_temp(&space, &[library.as_path()], Some(nowhere.as_path())),
        Some(3),
        "摊不开的那一卷没被记成卷级失败"
    );
    // 「其余卷照做」不只是退出码：好的那一卷真在盘上，摊不开的那一卷一个字节都没有。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照做，或者摊不开的那一卷也写出了东西"
    );
}

/// **加密卷点名得 `1`、发现得 `0`**（`volume-discovery/06`，ADR 0014 决定第 5 条）。
///
/// 加密**不另开一种结局**：头是密的就列不出成员，那一卷点不开，而点不开的处置早就定死了。
/// 这一条与本文件那条坏 zip 是同一副骨架——换的只是「点不开」的来处，
/// 而这正是它要钉住的：两个数一格不动。
///
/// 库那一侧的两半（拒绝那句话说得出是口令、发现的进非卷文件清单）由 `tests/container.rs`
/// 钉着；这里只问进程那一层看得见的那个数。
#[test]
fn an_encrypted_rar_is_refused_when_named_and_skipped_when_discovered() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let locked = fixtures::rar::write(library.join("加密的.rar"), fixtures::rar::ENCRYPTED);
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::gradient(fixtures::TINY));
    good.write();

    assert_eq!(
        tonefit(&space, &[locked.as_path()]),
        Some(1),
        "点名一个加密卷，这一趟该整个被拒"
    );
    assert_eq!(
        tonefit(&space, &[library.as_path()]),
        Some(0),
        "发现出来的一个加密卷把整趟拖下了水"
    );
    // 「其余卷照常跑完」不只是退出码：好的那一卷真在盘上。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照常跑完"
    );
}

/// 跑一趟 tonefit，返回它的退出码。进程被信号打断时是 `None`。
fn tonefit(space: &Workspace, inputs: &[&Path]) -> Option<i32> {
    tonefit_with_temp(space, inputs, None)
}

/// 同上，但把子进程的**系统临时目录**指到别处。
///
/// 三个变量一起设：`std::env::temp_dir()` 在 Windows 上看 `TMP` / `TEMP`，
/// 在别的平台上看 `TMPDIR`，而这条用例两个平台都要成立。
fn tonefit_with_temp(space: &Workspace, inputs: &[&Path], temp: Option<&Path>) -> Option<i32> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tonefit"));
    command
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        .args(inputs)
        // 报告与错误都不进测试日志：这条用例只看退出码，那两样别处已经测过了。
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(temp) = temp {
        command
            .env("TMP", temp)
            .env("TEMP", temp)
            .env("TMPDIR", temp);
    }
    command.status().expect("启动 tonefit").code()
}

/// **拒绝那句话印到 stderr 上时，记号里那个空格是一个普通空格**（停车场 Q106／Q183）。
///
/// 那句话劝人换一条命令，而它的原文里记号中间那个空格带着「不许断」的标注
/// （`src/wrap.rs` 的 `HARD_SPACE`）。折行那几处顺手把它换回一个普通空格，
/// **而拒绝这一路一格都不折**——`main` 里那一行 `eprintln!` 直接落到 stderr 上，
/// 换回来的是 `wrap::printed`。漏了那一步，用户照着抄出来的命令里带着一个
/// clap 认不出的字符，Q106 要买的东西正好反了。
///
/// **只有真进程看得见这一条**：库那一侧的用例断言的是**带标注的原文**
/// （`tests/pipeline.rs` 的 `FIT_HEIGHT`／`DITHER_FS`），印出去的字节别处观察不到。
#[test]
fn the_refusal_on_stderr_spells_its_commands_with_a_plain_space() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 一条又扁又小的纯墨页：两边都贴不住面板，几何门因此不成立，而这一趟点了抖动。
    volume.page(
        "001.png",
        &fixtures::solid(fixtures::DEGENERATE_STRIP_SMALLER_THAN_PANEL, 0),
    );

    let refused = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        // fit-inside 那一支才劝人换 `--fit height`；两条命令因此一次都问得到。
        .args(["--fit", "inside", "--dither", "fs"])
        .arg(volume.path())
        .output()
        .expect("启动 tonefit");
    let said = String::from_utf8_lossy(&refused.stderr);

    assert_eq!(refused.status.code(), Some(1), "这一趟该被拒：{said}");
    assert!(
        said.contains("--fit height"),
        "劝人换的那条命令断了：{said}"
    );
    assert!(said.contains("不点 --dither fs"), "另一条也断了：{said}");
    // 标注一个都没漏出去：漏了的话上面两条照旧成立不了，这一条说的是为什么。
    assert!(
        !said.contains('\u{a0}'),
        "标注原样落到 stderr 上了：{said:?}"
    );
}

/// **重定向出去时折到一个定值，不随窗口大小变**（`p4-parking-lot/05` 票面第二条）。
///
/// 折到多宽此刻**问终端**（`src/wrap.rs` 的 `terminal_width`），而**输出不是终端就取那个
/// 定值**：`tonefit --help > 说明.txt` 与 `tonefit … > 报告.txt` 的产出因此与跑它的那块屏无关。
///
/// **只有真进程看得见这一条**：二进制那一侧的用例是拿一个宽度去调 `folded_help`，
/// 说不出「这一趟到底问出了几格」。这里 stdout 接的是管道，跑用例的终端有多宽都不算数。
///
/// **命令行那两处各问一遍**——帮助与报告折的是同一个数，而它们在两条路上各自取用。
/// 帮助那一头两头都问：**一行都不过那个定值**（宽终端上没把 200 格当真），
/// 而**总有一行宽过它减一档缩进**（窄终端上也没把 60 格当真）；那一档余量取的正是
/// `LONG_HELP_INDENT`，`--help` 最宽的一行恰好是折满的正文加上它。
/// 报告那一头只问得出前一半：夹具是一卷一页，长到顶着那个定值的句子摆不出来。
///
/// 100 这个数写在 `src/wrap.rs` 的 `OFF_TERMINAL_WIDTH` 上，这里只能抄一遍：
/// 集成测试是另一个 crate，够不着二进制侧的常量（同一条道理见 `tests/golden.rs`
/// 的 `fit_key`）。两处真要分家，红的是这一条。
#[test]
fn what_is_redirected_out_of_a_terminal_folds_to_one_fixed_width() {
    /// 输出不是终端时折到多宽（`src/wrap.rs` 的 `OFF_TERMINAL_WIDTH`）。
    const OFF_TERMINAL_WIDTH: usize = 100;
    /// clap 给长帮助那一档缩进（`src/main.rs` 的 `LONG_HELP_INDENT`）。
    const LONG_HELP_INDENT: usize = 10;

    let widest = |said: &str| {
        said.lines()
            .map(unicode_width::UnicodeWidthStr::width)
            .max()
            .unwrap_or_default()
    };

    let helped = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--help")
        .output()
        .expect("启动 tonefit");
    let help = String::from_utf8_lossy(&helped.stdout);
    assert!(
        widest(&help) <= OFF_TERMINAL_WIDTH,
        "帮助最宽那一行 {} 格，宽过了那个定值：{help}",
        widest(&help)
    );
    // 折窄了同样不对：那说明它把跑用例的那块屏当了真。
    assert!(
        widest(&help) > OFF_TERMINAL_WIDTH - LONG_HELP_INDENT,
        "帮助最宽那一行只有 {} 格，定值没用满：{help}",
        widest(&help)
    );

    // **报告那一路走的是另一个出口**（`main` 里那一句 `folded_text`），同样只有真进程看得见。
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    let ran = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        .arg(volume.path())
        .output()
        .expect("启动 tonefit");
    let report = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(ran.status.code(), Some(0), "这一趟该干净跑完：{report}");
    assert!(
        widest(&report) <= OFF_TERMINAL_WIDTH,
        "报告最宽那一行 {} 格，宽过了那个定值：{report}",
        widest(&report)
    );
}
