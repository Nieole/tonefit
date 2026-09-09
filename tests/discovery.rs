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
use std::sync::{Arc, Mutex};

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

/// **分卷 `.rar` 是一个卷，不是两个**（`p4-parking-lot/17`，ADR 0015 决定第 1 条的修订）。
///
/// 两份 `.part*.rar` 从前各自成卷，而 UnRAR 打开头一份就跨卷读完了——盘上因此出两份
/// 重名不同的产物，其中一份是重复的。折成一个卷之后只剩一个输出容器，
/// 而它的名字是**分卷序列**的名字：`.part1` 那一截不进产物。
#[test]
fn a_split_rar_comes_out_as_one_volume_named_after_the_sequence() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let parts = fixtures::rar::write_split(&library, "第01卷", 2);

    let started = StartedVolumes::default();
    let seen = Arc::clone(&started.0);
    let report = tonefit::run(&tonefit::Request {
        progress: Some(tonefit::ProgressSink::new(started)),
        ..fixtures::request(&space, [library.as_path()])
    })
    .expect("处理应当成功");

    assert_eq!(report.volumes.len(), 1, "两份分卷没折成一个卷");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/第01卷.cbz"],
        "卷名没取分卷序列的名字，或者出了两份产物"
    );
    // **报告与进度条印的是同一个**，而两边印的都是分卷序列的**头一份**。
    //
    // 屏上那个名字**不是**序列名：命令行与会话共用 `render::volume_name`，而它印的一直是
    // **源文件自己的名字**——一个 `第10话.zip` 也印成 `第10话.zip`，与它的去处
    // `第10话.cbz` 从来就不同。分卷这一卷照这条既有惯例印 `第01卷.part1.rar`，
    // 而序列名出现在**去处**上（上面那一条断言）。票面第 2 条后半句还有另一种读法
    // （两处印的都该是序列名），那要连四种格式一起改，记在停车场 Q331。
    //
    // 这一条钉的因此是**两处不许分道**：`Event::VolumeStarted` 的卷标识与
    // `VolumeReport::volume` 眼下同源，而「让进度条改印序列名」正是最容易只改一处的改法。
    assert_eq!(
        seen.lock().expect("读回开卷那几条事件").as_slice(),
        [parts[0].clone()],
        "进度条印的不是分卷序列的头一份"
    );
    assert_eq!(
        report.volumes[0].volume, parts[0],
        "报告印的卷与进度条印的不是同一个"
    );
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
/// 这一条只问**盘上有没有多出字节**；它们在报告上占哪一格，是
/// [`every_file_no_volume_took_is_listed_with_a_reason_of_its_own`] 那一条的事。
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

/// 非卷文件那张表：三类各带路径与一句为什么，**与输出树的形状逐条对得上**（`volume-discovery/04`）。
///
/// 一棵树一次问全 spec《非卷文件》点名的那几种形状：两层 cbz、混装目录、
/// 一页都没有的 zip、坏 zip、卷架上的 txt。清单与输出树是**互补**的两张表——
/// 清单上的在输出里一个字节都没有，输出里的在清单上一条都没有——这一条同时钉这两半。
/// 缺了任何一半这条用例都不成立：只看输出，「什么没被转」照旧说不出；只看清单，
/// 说不出它到底转没转。
///
/// 比的是**排过序的**清单而不是发现顺序：同层的次序由 `source::reading_order` 说了算，
/// 而那条顺序在 `src/discover.rs` 的单元用例里已经钉着，这里再钉一遍就成了第二个出处。
#[test]
fn every_file_no_volume_took_is_listed_with_a_reason_of_its_own() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    // ① 卷架上既不是页也不是归档的文件：`库` 这一层一页都没有，它因此不是卷，收不下它。
    std::fs::write(space.dir("库/答案.txt"), b"a note the owner left here").expect("摆一份 txt");
    // ② 一页都没有的归档——字体包就是这个样子。
    let mut fonts = space.archive("库/字体包.zip");
    fonts.file("readme.txt", b"no pages in here");
    fonts.write();
    // ③ 发现出来但点不开的归档：中央目录与尾记录都不见了，归档结构根本读不出来。
    let mut broken = space.archive("库/坏的.cbz");
    broken.page("001.png", &fixtures::cheap_page());
    broken.write_truncated();
    // 混装目录：封面自成一个一页的卷，两话各自成卷，而 `说明.txt` 是**封面那一卷的
    // 透传成员**——它躺的那一层有页，因此不在清单上，照旧搬进输出容器。
    let mixed = space.volume("库/N和S");
    mixed.page("cover.png", &fixtures::cheap_page());
    mixed.file("说明.txt", b"this volume's note");
    write_archive(&space, "库/N和S/第1话.cbz", 2);
    write_archive(&space, "库/N和S/第2话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    // 三类各一条，各说各的为什么。
    assert_eq!(
        listed(&space, &report),
        [
            "库/坏的.cbz · 点不开",
            "库/字体包.zip · 一页都没有的归档",
            "库/答案.txt · 既不是页也不是归档",
        ]
    );
    // 清单不为空，而这一趟一卷都没失败：非卷文件不是失败（`CONTEXT.md` 的《失败》）。
    assert!(report.failed_volumes.is_empty(), "非卷文件被记成了失败");
    assert_eq!(report.volumes.len(), 3, "封面与两话没各自成卷");
    // 输出树里**只有产物**：清单上那三个一个字节都没有，而卷内的透传文件照旧在。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        [
            "库/N和S/cover.png",
            "库/N和S/第1话.cbz",
            "库/N和S/第2话.cbz",
            "库/N和S/说明.txt",
        ]
    );
}

/// 卷内的透传文件**照旧搬**，而且不出现在清单里（`volume-discovery/04`）。
///
/// 与上一条的分界只有一条：`ComicInfo.xml` 躺的那一层**有页**。阅读器靠它读作者、
/// 卷号与阅读方向，砍掉是净损失——非卷文件那张表收的是没有任何卷要的那些，不是它。
#[test]
fn a_pass_through_member_of_a_real_volume_is_never_listed() {
    let space = Workspace::new();
    let volume = space.volume("第10话");
    volume.page("001.png", &fixtures::cheap_page());
    volume.file(
        "ComicInfo.xml",
        b"<ComicInfo><Number>10</Number></ComicInfo>",
    );

    let report = fixtures::run_paths(&space, [volume.path()]);

    assert!(
        report.non_volume_files.is_empty(),
        "卷内的透传文件进了非卷文件清单：{:?}",
        listed(&space, &report)
    );
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["第10话/001.png", "第10话/ComicInfo.xml"]
    );
}

/// 点名两个互相嵌套的路径：同一个卷只做一遍，输出树与只点名最外层那个一模一样
/// （`p4-parking-lot/15`，收停车场 Q111）。
///
/// 从前 `库/作品/第1话.cbz` 被发现两遍，各写一份：一份在 `out/库/作品/` 下、一份在
/// `out/作品/` 下。去处不同，撞名那一道因此拦不住——盘上两份同内容不同位置的产物。
#[test]
fn naming_a_library_and_a_directory_inside_it_does_the_work_once() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let works = directory(&space, "库/作品");
    write_archive(&space, "库/作品/第1话.cbz", 2);
    write_archive(&space, "库/作品/第2话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path(), works.as_path()]);

    assert_eq!(report.volumes.len(), 2, "同一个卷做了不止一遍");
    // 镜像路径以**最外层**那个点名根为准：点名 `库 库/作品` 与只点名 `库` 同一棵输出树。
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/作品/第1话.cbz", "库/作品/第2话.cbz"]
    );
}

/// 同一个路径点两遍与点名嵌套路径走**同一条**收编：卷一份都不多。
#[test]
fn naming_the_same_path_twice_does_the_work_once() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    write_archive(&space, "库/作品/第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path(), library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "同一个卷做了不止一遍");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/作品/第1话.cbz"]
    );
}

/// **预扫报出来的那两个数不因收编而多数一遍。**
///
/// 两头一起问：开工那条事件报出来的卷数与**这一趟实际做的卷数**对得上；
/// 而点名 `库 库/作品`、点名 `库 库`、只点名 `库` 三趟报出来的卷数与全局总步数逐个相同。
///
/// 看开工那条事件而不是只看报告：多数出来的那一份正是屏上那条横条的**分母**——
/// 它多数了一遍，进度条就永远走不到头（ADR 0011 决定第 3 条）。
#[test]
fn the_run_announces_each_volume_once_however_the_named_paths_overlap() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let works = directory(&space, "库/作品");
    write_archive(&space, "库/作品/第1话.cbz", 2);
    write_archive(&space, "库/作品/第2话.cbz", 2);

    let (alone, _) = announced(&space, &[library.as_path()]);
    let (nested, report) = announced(&space, &[library.as_path(), works.as_path()]);
    let (twice, _) = announced(&space, &[library.as_path(), library.as_path()]);

    // 两话各自成卷，这棵树上就这么多——数取自夹具，不是照实现算一遍。
    assert_eq!(alone.0, 2, "点名一个库该发现两个卷，夹具搭错了");
    assert!(alone.1 > 0, "总步数是 0，那这条用例什么都没问出来");
    assert_eq!(
        nested.0,
        report.volumes.len(),
        "预告的卷数与这一趟实际做的卷数对不上"
    );
    assert_eq!(nested, alone, "嵌套的点名把卷数或总步数多数了一遍");
    assert_eq!(twice, alone, "同一个路径点两遍把卷数或总步数多数了一遍");
}

/// 里外哪个先点名都一样：收编到的恒是**最外层**那个根，输出树因此确定且可预测。
#[test]
fn the_outermost_named_root_wins_whichever_path_comes_first() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let works = directory(&space, "库/作品");
    write_archive(&space, "库/作品/第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [works.as_path(), library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "同一个卷做了不止一遍");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/作品/第1话.cbz"],
        "先点里层就按里层镜像了"
    );
}

/// 点名的两个路径**不重叠**时收编一个卷都不动：两边各展各的。
#[test]
fn two_named_paths_that_do_not_overlap_both_come_out() {
    let space = Workspace::new();
    let first = directory(&space, "甲部");
    let second = directory(&space, "乙部");
    write_archive(&space, "甲部/第1话.cbz", 2);
    write_archive(&space, "乙部/第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [first.as_path(), second.as_path()]);

    assert_eq!(report.volumes.len(), 2, "不重叠的两个点名被折掉了一个");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["乙部/第1话.cbz", "甲部/第1话.cbz"]
    );
}

/// 单独点名一个**不看的地方**（回收站），上面那个库点名了也折不掉它。
///
/// 折的是**卷根**，不是点名路径之间的前缀关系：`库` 与 `库/#recycle` 是嵌套的两条路径，
/// 而点名 `库` 那一趟一个回收站里的卷根都没走到（见
/// [`discovery_does_not_walk_into_the_places_we_never_look_at`]），
/// 两边因此不重叠。按前缀折的话，用户明说要的那个回收站会连同它底下的卷一起消失。
///
/// 「不看」只管**发现**：用户自己点名它就是明说了要，那一条一格没动。
#[test]
fn an_ignored_place_named_on_its_own_is_not_folded_away() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let recycle = directory(&space, "库/#recycle");
    write_archive(&space, "库/留着的作品/第1话.cbz", 2);
    write_archive(&space, "库/#recycle/删掉的作品/第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path(), recycle.as_path()]);

    assert_eq!(report.volumes.len(), 2, "点名的那个回收站被折掉了");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["#recycle/删掉的作品/第1话.cbz", "库/留着的作品/第1话.cbz"]
    );
}

/// 收编换的是去处，不是**「点名的 / 发现的」**那条分别：被外层收编掉的那个点名路径
/// 点不开时仍是整趟拒绝（ADR 0014 决定第 5 条）。
///
/// `库/坏的.cbz` 同时是点名的（用户自己点了它）与发现出来的（点名 `库` 走到了它）。
/// 收编要是顺手把那顶帽子摘了，它就变成「发现出来的点不开的归档」——进非卷文件、
/// 其余照做，而用户明说了要处理它。
#[test]
fn a_named_path_swallowed_by_an_outer_one_is_still_refused_when_it_cannot_be_opened() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    write_archive(&space, "库/第1话.cbz", 2);
    let mut broken = space.archive("库/坏的.cbz");
    broken.page("001.png", &fixtures::cheap_page());
    broken.write_truncated();

    let error = fixtures::run_paths_expecting_failure(
        &space,
        [library.as_path(), space.dir("库/坏的.cbz").as_path()],
    );

    let said = format!("{error:#}");
    assert!(said.contains("坏的.cbz"), "没说是哪个路径点不开：{said}");
    assert!(said.contains("整趟不做"), "没整趟拒绝：{said}");
}

/// 非卷文件那张表也不因收编而重复列出：那一层被点名两遍，`说明.txt` 仍只有一条。
///
/// 它与卷数是同一遍开卷的两份产出（见 `src/survey.rs` 的《另两份产出》）——
/// 收编排在开卷之前，两份因此一起只数一遍。
#[test]
fn a_file_no_volume_took_is_listed_once_however_often_its_directory_is_named() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    let works = directory(&space, "库/作品");
    write_archive(&space, "库/作品/第1话.cbz", 2);
    std::fs::write(space.dir("库/作品/说明.txt"), "读我").expect("摆一个非卷文件");

    let report = fixtures::run_paths(&space, [library.as_path(), works.as_path()]);

    assert_eq!(
        listed(&space, &report),
        ["库/作品/说明.txt · 既不是页也不是归档"]
    );
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

/// 不看的地方整棵子树都不进去，而且**报告上一个字都没有**。
///
/// 打包环境留下的那几个与操作系统留下的那几个（`System Volume Information` 一类，
/// **永远**读不动）共一份名单、共一条判据——名单与「哪几个是发现才撞得到的」
/// 都写在 `src/source.rs` 的 `IGNORED_DIRECTORIES` 上。
///
/// 三句话：**卷数只有那一个**、**报告的两栏都空着**、**输出树上只有留着的那一卷**。
/// 中间那句是这一条的要害：绕过发生在发现那一层，它们根本不成为候选，
/// 因此走不进去的地方那一栏收不到它们——收到了就是把「不看」说成了「看不了」。
#[test]
fn discovery_does_not_walk_into_the_places_we_never_look_at() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    for ignored in [
        ".git",
        "#recycle",
        "@Recycle",
        ".@__thumb",
        "__MACOSX",
        "System Volume Information",
        "$RECYCLE.BIN",
        "lost+found",
        ".Trash-1000",
    ] {
        write_archive(&space, &format!("库/{ignored}/删掉的第1话.cbz"), 2);
    }
    write_archive(&space, "库/留着的第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 1, "走进了不该走的目录");
    assert!(
        report.unreachable_places.is_empty(),
        "不看的地方上了走不进去的地方那一栏：{:?}",
        report.unreachable_places
    );
    assert_eq!(
        listed(&space, &report),
        Vec::<String>::new(),
        "不看的地方上了非卷文件那一栏"
    );
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/留着的第1话.cbz"]
    );
}

/// **反过来那一半：名字只是挨得近的目录照旧走进去。**
///
/// 判据是**整个名字**（大小写不敏感），不是前缀、不是包含，更不是「读不读得动」。
/// 三种放宽各有一个用户会真撞上的样子：`System Volume Information 备份` 是有人手动
/// 拷出来的一份，`.Trash-10000` 是另一个 uid 的回收站以外的普通目录，
/// `$RECYCLE.BIN.old` 是换盘时留下的。按前缀或包含认，这三处连同它们底下的卷一起静默消失
/// ——那正是这张票要治的病换了个方向再犯一遍。
///
/// 大小写那一格反着钉一下：`$Recycle.Bin` 与 `$RECYCLE.BIN` 是同一个名字
/// （Windows 上两种写法都见得到），它**照旧**不看。
///
/// 权限那一半的反向在 `tests/exit_code.rs` 的
/// `a_place_that_cannot_be_entered_ends_the_run_with_three` 与 `src/survey.rs` 的
/// `a_directory_that_cannot_be_read_says_so`：名字不在名单上而真读不动的目录，
/// 照旧进走不进去的地方那一栏、照旧进退出码。那两条要关得上门的机器才问得出来，
/// 这一条平台无关。
#[test]
fn a_name_that_merely_looks_like_one_we_never_look_at_is_still_walked_into() {
    let space = Workspace::new();
    let library = directory(&space, "库");
    for near in [
        "System Volume Information 备份",
        ".Trash-10000",
        "$RECYCLE.BIN.old",
    ] {
        write_archive(&space, &format!("库/{near}/第1话.cbz"), 2);
    }
    write_archive(&space, "库/$Recycle.Bin/删掉的第1话.cbz", 2);

    let report = fixtures::run_paths(&space, [library.as_path()]);

    assert_eq!(report.volumes.len(), 3, "挨得近的名字被当成了不看的地方");
    assert_eq!(
        fixtures::directory_members(&space.out()),
        [
            "库/$RECYCLE.BIN.old/第1话.cbz",
            "库/.Trash-10000/第1话.cbz",
            "库/System Volume Information 备份/第1话.cbz",
        ]
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

/// 跑一趟，交出**开工那条事件报出来的两个数**（这一趟有几个卷、最多走多少步）
/// 与这一趟的报告。
///
/// 走**试算**（`Mode::DryRun`）：这两个数由预扫算出、一个文件都不落盘，同一个工作区
/// 因此跑得了好几趟而互不干扰——落盘那一路第二趟会撞上第一趟的产物，比的就不只是
/// 点名方式的差别了。
fn announced(space: &Workspace, inputs: &[&Path]) -> ((usize, u64), tonefit::Report) {
    let seen = Arc::new(Mutex::new(None));
    let report = tonefit::run(&tonefit::Request {
        mode: tonefit::Mode::DryRun,
        progress: Some(tonefit::ProgressSink::new(Announced(Arc::clone(&seen)))),
        ..fixtures::request(space, inputs.iter().copied())
    })
    .expect("处理应当成功");
    let announced = *seen.lock().expect("读开工那条事件带的两个数");
    (announced.expect("这一趟没发出开工那条事件"), report)
}

/// 只留开工那条事件带的两个数，别的一律不看。
struct Announced(Arc<Mutex<Option<(usize, u64)>>>);

impl tonefit::Progress for Announced {
    fn observe(&self, event: tonefit::Event<'_>) -> tonefit::Instruction {
        if let tonefit::Event::RunStarted { volumes, steps, .. } = event {
            *self.0.lock().expect("记下开工那条事件") = Some((volumes, steps));
        }
        tonefit::Instruction::Continue
    }
}

/// 只留每条**开卷**事件带的那个卷标识，别的一律不看。
///
/// 进度条印的卷从这里来（命令行那一路的 `Bar::start`、会话那一路的
/// `Live::volume_started` 都取 `Event::VolumeStarted` 的 `volume`），
/// 而报告里那一格是 `VolumeReport::volume`——「报告与进度条印的是同一个」
/// 只有把两边比一次才钉得住。
#[derive(Default)]
struct StartedVolumes(Arc<Mutex<Vec<PathBuf>>>);

impl tonefit::Progress for StartedVolumes {
    fn observe(&self, event: tonefit::Event<'_>) -> tonefit::Instruction {
        if let tonefit::Event::VolumeStarted { volume, .. } = event {
            self.0
                .lock()
                .expect("记下开卷那条事件")
                .push(volume.to_path_buf());
        }
        tonefit::Instruction::Continue
    }
}

/// 非卷文件那张表，摊成一行一条的 `工作区相对路径 · 哪一类`，按名字排序。
///
/// 类那一半只取一个短标签，不取界面上那句完整的话：措辞归界面层
/// （`src/render.rs` 的 `non_volume_reason`，那一句在那里钉着），
/// 这一份要分得开的是**哪三类**。
fn listed(space: &Workspace, report: &tonefit::Report) -> Vec<String> {
    let mut lines: Vec<String> = report
        .non_volume_files
        .iter()
        .map(|file| {
            let kind = match file.reason {
                tonefit::NonVolumeReason::NeitherPageNorArchive => "既不是页也不是归档",
                tonefit::NonVolumeReason::ArchiveWithoutAPage => "一页都没有的归档",
                tonefit::NonVolumeReason::Unopenable(_) => "点不开",
            };
            format!(
                "{} · {kind}",
                fixtures::relative_name(space.root(), &file.path)
            )
        })
        .collect();
    lines.sort();
    lines
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
