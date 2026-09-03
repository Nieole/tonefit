//! 发现：点名的一个路径展开成一批卷（ADR 0014）。
//!
//! 外部行为是**盘上的字节**：造一棵树、跑一趟、看输出树的形状。这一份里的每一条都这么问，
//! 只有「各自一份上包络」那一条另看报告——那是发现改动卷边界之后**唯一**从盘上看不出来的
//! 后果（同一批页，分成两卷与合成一卷写出的字节可以相同，定档却不同）。
//!
//! 退出码那一条不在这里：「点名的 / 发现的」只决定点不开时的处置，而处置的差别是
//! **退出码**，退出码只在真进程上观察得到（见 `tests/exit_code.rs`）。

mod fixtures;

use std::path::{Path, PathBuf};

use fixtures::Workspace;
use tonefit::VolumeVerdict;

/// 点名一个两层库：每一话各自成卷，输出按源的结构镜像。
///
/// 这是触发整张票的那个形状——`网络资源/<作品>/<第N话>.cbz`。从前点名 `库` 得到的是
/// 一个「0 页」的卷加两个被原样拷过去的 cbz。
#[test]
fn every_chapter_in_a_two_level_library_becomes_its_own_volume() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    write_archive(&space, "库/作品/第1话.cbz", 2);
    write_archive(&space, "库/作品/第2话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 2, "两话没各自成卷");
    // 点名路径自己的名字打头，其下按源的结构镜像——基准点是点名路径的**父目录**。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/作品/第1话.cbz", "库/作品/第2话.cbz"]
    );
    assert!(library.is_dir(), "源库被动了");
}

/// 点名一个目录卷（直接躺着页）：产物落在它一直落的那个地方。
///
/// 「与改动前逐字节相同」由黄金回归钉着（`tests/golden.rs` 比的是产物的哈希，
/// 那一批夹具正是这个形状）；这一条问的是**去处**——发现给输出加了一层镜像，
/// 而点名一个卷时那层镜像必须退化成「就是它自己的名字」，否则页会直接撒进输出根。
#[test]
fn a_named_directory_volume_still_lands_under_its_own_name() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::cheap_page());
    volume.page("002.png", &fixtures::cheap_page());

    let report = fixtures::run_paths(&space, [volume.path()]);

    assert_eq!(report.volumes.len(), 1);
    assert_eq!(report.volumes[0].output, space.out().join("volume-a"));
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["volume-a/001.png", "volume-a/002.png"]
    );
}

/// 点名一个归档卷：同一条规则，扩展名归一成 `.cbz`。
#[test]
fn a_named_archive_still_lands_under_its_own_name() {
    let space = Workspace::new();
    let archive = write_archive(&space, "第10话.zip", 2);

    let report = fixtures::run_paths(&space, [archive.as_path()]);

    assert_eq!(report.volumes.len(), 1);
    assert_eq!(fixtures::directory_members(&space.out()), ["第10话.cbz"]);
}

/// **一个目录可以既是卷又装着卷**：一张孤立封面加几个 cbz，封面自成一个一页的卷，
/// 每个 cbz 各自成卷。这个场景正是 ADR 0014 决定第 1 条的来由，理由写在那里。
///
/// 这一条钉的是**一页都不被处理两遍**：三个卷的源页两两不同，加起来正好是树上那几张。
#[test]
fn a_lone_cover_next_to_the_archives_becomes_a_one_page_volume_of_its_own() {
    let space = Workspace::new();
    let cover = space.volume("N和S");
    cover.page("cover.png", &fixtures::cheap_page());
    write_archive(&space, "N和S/第1话.cbz", 2);
    write_archive(&space, "N和S/第2话.cbz", 2);

    let report = fixtures::run_paths(&space, [cover.path()]);

    assert_eq!(report.volumes.len(), 3, "封面与两话没各自成卷");
    assert_eq!(report.volumes[0].source_pages, 1, "封面那一卷不是一页");
    // 输出里**没有那两个 cbz 的副本**：躺在这一层的归档是卷，不是这一卷的透传文件。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["N和S/cover.png", "N和S/第1话.cbz", "N和S/第2话.cbz"]
    );
    // 一页都不被处理两遍：五张源页，五个两两不同的源成员。
    let sources: Vec<PathBuf> = report
        .volumes
        .iter()
        .flat_map(|volume| &volume.pages)
        .map(|page| page.source.clone())
        .collect();
    let mut distinct = sources.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(sources.len(), 5, "源页总数不对");
    assert_eq!(
        distinct.len(),
        sources.len(),
        "有页被处理了两遍：{sources:?}"
    );
}

/// 分目录装的**归档**一页不少：归档的边界由打包者定死了，内部结构照收。
#[test]
fn an_archive_that_keeps_its_chapters_in_folders_loses_no_page() {
    let space = Workspace::new();
    let mut cbz = space.cbz("合订本");
    let page = fixtures::cheap_page();
    cbz.page("第01话/001.png", &page)
        .page("第01话/002.png", &page)
        .page("第02话/001.png", &page);
    let path = cbz.write();

    let report = fixtures::run_paths(&space, [path.as_path()]);

    assert_eq!(report.volumes.len(), 1, "归档被拆开了");
    assert_eq!(report.volumes[0].source_pages, 3, "归档里有页没被收下");
}

/// 分目录装的**目录**裂成几个卷，**各自一份上包络**。
///
/// 这是本票买下的那笔代价，正面写在这里：同一批页从前合成一卷取一个上包络，
/// 如今两个章节各定各的档。夹具让两边的判定落在不同的档上，两份上包络因此分得开——
/// 合成一卷时它们只会有一个数。
#[test]
fn a_directory_that_keeps_its_chapters_in_folders_splits_into_one_volume_each() {
    let space = Workspace::new();
    let works = directory(&space, "作品");
    let first = space.volume("作品/第01话");
    let second = space.volume("作品/第02话");
    for page in ["001.png", "002.png", "003.png"] {
        first.page(
            page,
            &fixtures::solid(fixtures::TINY, fixtures::NEEDS_TWO_BITS),
        );
        second.page(
            page,
            &fixtures::solid(fixtures::TINY, fixtures::FAR_OUTSIDE),
        );
    }

    // 这两个取值分得开那一档要在**门不成立**那条路上读，因此跑 fit-inside
    // （见夹具里 `FAR_OUTSIDE` 与 `TINY` 各自的说明）。
    let report = tonefit::run(&tonefit::Request {
        fit: tonefit::FitMode::Inside,
        ..fixtures::request(&space, [works.as_path()])
    })
    .expect("处理应当成功");

    assert_eq!(report.volumes.len(), 2, "两个章节目录没各自成卷");
    let depths: Vec<tonefit::BitDepth> = report.volumes.iter().map(base_depth).collect();
    assert_eq!(
        depths,
        [tonefit::BitDepth::Two, tonefit::BitDepth::Four],
        "两卷共用了一份上包络"
    );
}

/// 一页都没有的东西不是卷：输出里一个字节都没有。
///
/// 四种形状一次问全：一页都没有的归档（字体包就是这个样子）、空目录、
/// 底下一个卷都没有的目录、卷旁边那份既不是页也不是归档的 txt、以及点名的那个目录自己。
/// 清单本身是 `volume-discovery/04` 的事，这一条只问**盘上有没有多出字节**。
#[test]
fn nothing_without_a_page_writes_a_single_byte() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let mut fonts = space.archive("库/字体包.cbz");
    fonts.file("readme.txt", b"no pages in here");
    fonts.write();
    directory(&space, "库/空目录");
    directory(&space, "库/只装着别的目录/里面也是空的");
    std::fs::write(space.dir("库/答案.txt"), b"a note the owner left here").expect("摆一份 txt");
    write_archive(&space, "库/第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "不是卷的东西成了卷");
    assert!(report.failed_volumes.is_empty(), "非卷文件被记成了失败");
    assert_eq!(fixtures::directory_members(&space.out()), ["库/第1话.cbz"]);
}

/// 输出根落在点名路径底下仍然当场拒绝——发现因此不会把上一趟的产物当成源。
#[test]
fn an_output_root_inside_a_named_directory_is_still_refused() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    write_archive(&space, "库/第1话.cbz", 2);

    let error = tonefit::run(&tonefit::Request {
        output_root: library.join("out"),
        ..fixtures::request(&space, [library.as_path()])
    })
    .expect_err("输出根落在点名路径底下该拒绝");

    assert!(error.to_string().contains("相互嵌套"), "{error}");
}

/// 打包环境留下的目录整棵子树都不进去。名单与「后四个为什么是发现才撞得到的」
/// 都写在 `src/source.rs` 的 `JUNK_DIRECTORIES` 上。
#[test]
fn discovery_does_not_walk_into_the_directories_packing_tools_leave_behind() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    for junk in [".git", "#recycle", "@Recycle", ".@__thumb", "__MACOSX"] {
        write_archive(&space, &format!("库/{junk}/删掉的第1话.cbz"), 2);
    }
    write_archive(&space, "库/留着的第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "走进了不该走的目录");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/留着的第1话.cbz"]
    );
}

/// 符号链接与 junction **不跟进**：环因此进不来，发现的深度不必设上界。
///
/// 建不出符号链接的机器上（Windows 默认要管理员或开发者模式）这条问不出来，
/// 当场收工——它在那种机器上恒成立，误报不了，而在建得出来的机器上，跟进的那一版当场红：
/// 链接指回它自己的上一级，跟进就是无穷递归。
#[test]
fn discovery_does_not_follow_a_symlink() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    write_archive(&space, "库/第1话.cbz", 2);
    if !link_directory(&library, &library.join("回到自己")) {
        return;
    }

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "跟进了符号链接");
    assert_eq!(fixtures::directory_members(&space.out()), ["库/第1话.cbz"]);
}

/// 建一级目录（父目录一起建出来），返回它的路径。
fn directory(space: &Workspace, name: &str) -> PathBuf {
    let path = space.dir(name);
    std::fs::create_dir_all(&path).expect("建目录");
    path
}

/// 在 `name` 处写一个装着 `pages` 张页的归档，父目录一起建出来。
fn write_archive(space: &Workspace, name: &str, pages: usize) -> PathBuf {
    let path = space.dir(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("建归档所在目录");
    }
    let mut archive = space.archive(name);
    let page = fixtures::cheap_page();
    for index in 1..=pages {
        archive.page(&format!("{index:03}.png"), &page);
    }
    archive.write()
}

/// 一个卷的基准档。
fn base_depth(volume: &tonefit::VolumeReport) -> tonefit::BitDepth {
    match volume.verdict {
        Some(VolumeVerdict::Envelope(envelope)) => envelope.base.bit_depth,
        ref other => panic!("这一卷该由上包络定档，实际是 {other:?}"),
    }
}

/// 建一个指向 `target` 的目录符号链接。这台机器不许建就回 `false`。
fn link_directory(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (target, link);
        false
    }
}
