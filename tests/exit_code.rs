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

/// **发现走不进去的一个目录让这一趟收在 `3` 上，其余卷照常跑完**
/// （`p4-parking-lot/11`，收停车场 Q117）。
///
/// 从前这一趟收在 `0` 上：那棵子树整个消失，报告一行不说——用户拿到的是一份看起来成功、
/// 实际少了几十卷的输出。NAS 上一个权限没配好的作品目录正是这个样子。
///
/// **它与卷级失败共用那个数**，不新开第五个：两者交出的东西一样（那一块一个字节都没有）、
/// 用户下一步该查的也一样（文件还在不在、盘还挂着没有、权限变没变），
/// 见二进制侧的 `FAILED_VOLUME_EXIT`。
///
/// 非在真进程上问不可，理由与本文件其余几条同一个：退出码只在进程那一层观察得到。
/// 弄不出一个读不动的目录的机器上（Windows 没有这一手，root 底下权限位不作数）当场收工
/// ——它在那里恒不成立，误报不了；而「一声不吭」那一版在弄得出的机器上当场红。
#[test]
fn a_place_that_cannot_be_entered_ends_the_run_with_three() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();
    let closed = library.join("权限没配好的作品");
    std::fs::create_dir(&closed).expect("建读不动的那一层");
    if !shut_the_door(&closed) {
        return;
    }

    let code = tonefit(&space, &[library.as_path()]);
    let members = fixtures::directory_members(&space.out());
    // 断言之前先把门打开：断言红了也不至于留下一个删不掉的临时目录。
    open_the_door(&closed);

    assert_eq!(code, Some(3), "走不进去的那一处没让这一趟离开 `0`");
    // 「其余卷照常跑完」不只是退出码：好的那一卷真在盘上。
    assert_eq!(members, ["库/好的.cbz"], "其余卷没照常跑完");
}

/// **点名一个读不动的目录仍是 `1`**（ADR 0014 决定第 5 条）。
///
/// 「点名的 / 发现的」那条既有分别一格没动：上一条那个目录换成点名的，这一趟整个不做。
/// 与本文件那条坏 zip、那条加密 rar 是同一副骨架——换的只是「点不开」的来处。
#[test]
fn a_named_place_that_cannot_be_entered_is_still_refused() {
    let space = Workspace::new();
    let closed = space.dir("权限没配好的作品");
    std::fs::create_dir_all(&closed).expect("建读不动的那一层");
    if !shut_the_door(&closed) {
        return;
    }

    let code = tonefit(&space, &[closed.as_path()]);
    open_the_door(&closed);

    assert_eq!(code, Some(1), "点名一个读不动的目录没被整趟拒");
}

/// **一棵含操作系统目录、卷卷都成的树收在 `0` 上**（本票，收停车场 Q203）。
///
/// 从前这一趟恒收在 `3` 上：`System Volume Information` 这一类**永远**读不动，
/// 于是每一趟都进走不进去的地方那一栏、每一趟都把退出码从 `0` 拖走，重跑一百遍都一样。
/// 点名一个盘根或共享根的人因此永远拿不到 `0`，而那一栏劝他做的事对这一类一件都做不了
/// （为什么做不了，见 `CONTEXT.md` 的《不看的地方 (IgnoredPlace)》）。
///
/// 这一条在两种机器上问的不是同一件事，两边都不恒真：
///
/// - **关得上门的机器**上，那个 `System Volume Information` 真读不动——这正是 Q203
///   那个场景的复现：不看的地方不进那一栏，退出码因此回到 `0`。
/// - **关不上门的机器**上（Windows 没有这一手，root 底下权限位不作数），它读得动，
///   而它底下那一卷**没有**出现在输出树上——按名字绕过在发现那一层就作数，
///   与读不读得动无关。
///
/// 反过来那一半在 `a_place_that_cannot_be_entered_ends_the_run_with_three`：
/// 名字**不在**那份名单上的目录真读不动时，照旧上那一栏、照旧收在 `3` 上。
#[test]
fn a_tree_with_places_we_never_look_at_still_ends_the_run_with_zero() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();
    // 四个都摆上：名字是各平台上固定的那几个，一个都不该成为候选。
    for ignored in [
        "System Volume Information",
        "$RECYCLE.BIN",
        "lost+found",
        ".Trash-1000",
    ] {
        let place = library.join(ignored);
        std::fs::create_dir(&place).expect("建不看的那一层");
        // 里面躺着一个真卷：门关不上的机器上，走进去了就会在输出树上露馅。
        let mut inside = fixtures::Cbz::new(place.join("删掉的第1话.cbz"));
        inside.page("001.png", &fixtures::cheap_page());
        inside.write();
    }
    // 头一个再关上门：关得上的机器上，它就是 Q203 里那个永远读不动的地方。
    let never_readable = library.join("System Volume Information");
    shut_the_door(&never_readable);

    let code = tonefit(&space, &[library.as_path()]);
    let members = fixtures::directory_members(&space.out());
    // 断言之前先把门打开：断言红了也不至于留下一个删不掉的工作区。
    open_the_door(&never_readable);

    assert_eq!(code, Some(0), "不看的地方把这一趟从 `0` 上拖走了");
    assert_eq!(
        members,
        ["库/好的.cbz"],
        "走进了不看的地方，它底下的卷进了输出"
    );
}

/// 把一个目录弄成**列不出来**的样子，成了才回 `true`。
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

/// 把门再打开，好让工作区收得掉。
#[cfg(unix)]
fn open_the_door(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn open_the_door(_dir: &Path) {}

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
/// （库那条公共 API，`tonefit::HARD_SPACE`）。折行那几处顺手把它换回一个普通空格，
/// **而拒绝这一路一格都不折**——`main` 里那一行 `eprintln!` 直接落到 stderr 上，
/// 换回来的是 `wrap::printed`。漏了那一步，用户照着抄出来的命令里带着一个
/// clap 认不出的字符，Q106 要买的东西正好反了。
///
/// **只有真进程看得见这一条**：库那一侧的用例断言的是**带标注的原文**
/// （`tests/pipeline.rs` 的 `fit_height`／`dither_fs`），印出去的字节别处观察不到。
#[test]
fn the_refusal_on_stderr_spells_its_commands_with_a_plain_space() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 两边都比面板小的页：fit-inside 上按不放大原样输出，一条边都贴不住，
    // 几何门因此不成立，而这一趟点了抖动。
    //
    // **换成以高为准它就贴住面板高了**，那条出路对它当真——拒绝按页分岔之后
    // （21 号票），只有这样的页才听得见 `--fit height`，两条命令因此一次都问得到。
    volume.page(
        "001.png",
        &fixtures::full_bleed_gradient(fixtures::SMALLER_THAN_TARGET),
    );

    let refused = Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        // fit-inside 上这一页贴不住面板，而以高为准够得着——那一支才劝人换
        // `--fit height`；两条命令因此一次都问得到。
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

/// **`--brief` 印出去的那一份真的少了底下两级**（`p4-parking-lot/22` 票面第一、二条）。
///
/// **只有真进程看得见这一条**：`Cli::report_fold` 说得出该挑哪一副、
/// `render::plain::report` 说得出那一副长什么样，两处各有各的用例；说不出的是 `main`
/// 有没有把前者交给后者——把那一行换回不折的那一副，二进制那一侧一条都不会红。
/// 与退出码那几条同一个道理：接线本身是进程那一层的事实。
///
/// **两趟共用一个工作区，跑之前把上一趟的产物清掉**。共用是必须的：两份报告要逐行比，
/// 而报告里印着源与去处的**真路径**——换个工作区跑第二趟，那几行连折都没折就已经不同了。
/// 清产物也是必须的：留着的话第二趟整卷幂等命中，卷级那一行改口说「跳过」，
/// 比出来的就不是折没折了。
///
/// **断言一个字面记号都不认**：印出去之前那一份还要过一遍折行（`main` 里那一句
/// `folded_text`，折到 100 格），而这个夹具的卷根是一条临时目录路径——卷级那一行
/// 铁定折断，`contains(" → ")` 会红得莫名其妙。这里问的因此全是**行与行的关系**：
/// 折起那一副更短、它的每一行都在不折那一副里逐字出现过、两头那两行一格没动。
/// 前两条钉住「藏掉了正文」，后两条钉住「抬头与末尾那几小结没跟着藏」。
#[test]
fn brief_folds_the_report_down_to_one_row_per_directory() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    // 一张读不出的页把这一卷送进隔离：末尾那几小结因此非空，折起那一副得留着它们。
    volume.file("002.png", b"not a png at all");

    let printed = |brief: bool| {
        let _ = std::fs::remove_dir_all(space.out());
        let mut command = Command::new(env!("CARGO_BIN_EXE_tonefit"));
        command
            .arg("--out")
            .arg(space.out())
            .args(["--profile", fixtures::BASELINE_DEVICE])
            .arg(volume.path());
        if brief {
            command.arg("--brief");
        }
        let ran = command.output().expect("启动 tonefit");
        let said = String::from_utf8_lossy(&ran.stdout).into_owned();
        assert_eq!(ran.status.code(), Some(2), "这一趟该有卷被隔离：{said}");
        said
    };

    let whole = printed(false);
    let folded = printed(true);
    let rows = |said: &str| said.lines().map(str::to_owned).collect::<Vec<_>>();
    let (whole, folded) = (rows(&whole), rows(&folded));

    // 折起那一副更短——`main` 真把开关交给了渲染那一头。
    assert!(
        folded.len() < whole.len(),
        "--brief 没折掉任何东西：{folded:?}"
    );
    // 而它没另编一套说法：留下的每一行都在不折那一副里逐字出现过。
    for row in &folded {
        assert!(
            whole.contains(row),
            "折起那一副多出了一行不折那一副没有的：{row}"
        );
    }
    // 两头那两行一格没动：抬头在，末尾那几小结也在。
    assert_eq!(folded.first(), whole.first(), "抬头跟着折没了");
    assert_eq!(
        folded.last(),
        whole.last(),
        "末尾那几小结跟着折没了——折起那一副因此说不出这一趟出了什么事"
    );
}
