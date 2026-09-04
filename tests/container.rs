//! 容器：卷从哪里读、写到哪里去。
//!
//! 目录与 CBZ 走同一个源抽象，因此这些用例大多成对出现：同一条性质，两种容器各测一次。

mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fixtures::{Workspace, run_paths, run_paths_expecting_failure, run_volume};
use tonefit::Size;

/// 透传要逐字节一致，因此这份夹具故意带上非 ASCII 与换行。
const COMIC_INFO: &str = "<?xml version=\"1.0\"?>\n<ComicInfo><Title>卷一</Title></ComicInfo>\n";

/// AppleDouble 边车文件的开头：魔数 0x00051607 加版本号。
/// macOS 打包时给每个成员配一份，扩展名照抄本体——当页解必然解不出图。
const APPLE_DOUBLE: &[u8] = &[0x00, 0x05, 0x16, 0x07, 0x00, 0x02, 0x00, 0x00];

#[test]
fn a_directory_volume_carries_its_non_page_files_across_byte_for_byte() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::cheap_page());
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());

    let report = run_volume(&space, &volume);

    let carried = std::fs::read(report.volumes[0].output.join("ComicInfo.xml"))
        .expect("非图片文件应当被带到输出");
    assert_eq!(carried, COMIC_INFO.as_bytes(), "透传的内容必须逐字节一致");
}

#[test]
fn a_cbz_volume_is_read_without_unpacking_and_comes_back_out_as_a_cbz() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 故意让字典序与阅读顺序分道扬镳：包内成员与目录成员走同一套排序。
    cbz.page("10.jpg", &page)
        .page("2.png", &page)
        .page("1.png", &page);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    // 输入是 CBZ，输出也是 CBZ——一个文件，不是一个目录。
    let output = &report.volumes[0].output;
    assert_eq!(output, &space.out().join("volume-a.cbz"));
    assert!(output.is_file(), "{} 不是一个文件", output.display());

    let members = fixtures::read_cbz(output);
    let names: Vec<_> = members.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["1.png", "2.png", "10.png"]);
    for (name, bytes) in &members {
        assert!(!bytes.is_empty(), "{name} 是空的");
    }
}

#[test]
fn an_archive_page_is_reported_as_the_volume_path_plus_its_member_name() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 两章并列，`ch1` 因此不是包装层，成员名里留得住一级目录。
    cbz.page("ch1/001.jpg", &page).page("ch2/001.jpg", &page);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    // 归档成员没有文件系统路径，报告给的是它在卷里的身份：卷路径接上成员名。
    let page = &report.volumes[0].pages[0];
    assert_eq!(page.source, path.join("ch1/001.jpg"));
    assert_eq!(
        page.output,
        space.out().join("volume-a.cbz").join("ch1/001.png")
    );
    assert!(!page.output.exists(), "这条身份不该是一个打得开的路径");
    // 打得开的是卷那一级。
    assert!(report.volumes[0].output.is_file());
}

#[test]
fn member_names_that_are_not_utf8_are_decoded_instead_of_mangled() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // GBK 名且不置 UTF-8 标志。按规范的 cp437 去解，「第」会变成一串乱码。
    cbz.gbk_page("第02话/001.png", &page)
        .gbk_page("第01话/001.png", &page)
        .gbk_file("第01话/说明.txt", "扉页".as_bytes());
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert_eq!(
        member_names(&report.volumes[0].output),
        ["第01话/001.png", "第02话/001.png", "第01话/说明.txt"]
    );
    // 报告里的身份同样是解好的名字，用户按它去归档里找得到。
    let pages = &report.volumes[0].pages;
    assert!(
        pages[0].source.ends_with("第01话/001.png"),
        "{:?}",
        pages[0].source
    );
}

#[test]
fn a_wrapper_directory_inside_the_archive_does_not_survive_into_the_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 打包工具惯常把整卷塞进一个同名目录，连目录项一起写出来。
    cbz.directory("volume-a")
        .directory("volume-a/ch1")
        .page("volume-a/ch1/001.jpg", &page)
        .page("volume-a/ch2/001.jpg", &page)
        .file("volume-a/ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    // 包装那一层没了，它下面的章节目录留着；目录项本身不是成员，不该出现在输出里。
    assert_eq!(
        member_names(&report.volumes[0].output),
        ["ch1/001.png", "ch2/001.png", "ComicInfo.xml"]
    );
}

#[test]
fn every_level_that_holds_the_whole_volume_is_stripped_not_just_the_first() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 嵌了两层，两层都装着整卷。没有兄弟的目录不承担顺序，一路剥到底。
    cbz.page("raw/第01话/001.png", &page)
        .page("raw/第01话/002.png", &page);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert_eq!(
        member_names(&report.volumes[0].output),
        ["001.png", "002.png"]
    );
}

#[test]
fn parallel_top_level_directories_are_not_a_wrapper_and_stay() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 两个并列的顶层目录开始承担顺序了，剥掉就把两章合并了。
    cbz.page("ch1/001.png", &page).page("ch2/001.png", &page);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert_eq!(
        member_names(&report.volumes[0].output),
        ["ch1/001.png", "ch2/001.png"]
    );
}

/// macOS 的「压缩」菜单打出来的包：整卷一个目录，外加一个 `__MACOSX` 兄弟目录，
/// 里面每个成员配一份 AppleDouble 边车，扩展名与页一模一样。
///
/// 边车当页解必然解不出图，整卷因此进隔离目录还被插上白页；`__MACOSX` 与 `.DS_Store`
/// 又都是包装那一层的兄弟，包装层于是一层都剥不掉。这一条把三件事一起钉住。
#[test]
fn a_mac_packed_archive_ignores_its_sidecars_and_stays_out_of_isolation() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    cbz.directory("volume-a")
        .page("volume-a/001.png", &page)
        .page("volume-a/002.png", &page)
        .file("volume-a/ComicInfo.xml", COMIC_INFO.as_bytes())
        .directory("__MACOSX")
        .directory("__MACOSX/volume-a")
        .file("__MACOSX/volume-a/._001.png", APPLE_DOUBLE)
        .file("__MACOSX/volume-a/._002.png", APPLE_DOUBLE)
        .file("__MACOSX/volume-a/._ComicInfo.xml", APPLE_DOUBLE)
        .file(".DS_Store", b"\x00\x00\x00\x01Bud1")
        .file("volume-a/.DS_Store", b"\x00\x00\x00\x01Bud1");
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    let volume = &report.volumes[0];
    assert!(!volume.isolated(), "边车被当成页，整卷进了隔离目录");
    assert_eq!(volume.output, space.out().join("volume-a.cbz"));
    assert_eq!(volume.page_count(), 2, "边车混进了页里");
    // 忽略掉这些之后包装那一层才没了兄弟，剥得掉。
    assert_eq!(
        member_names(&volume.output),
        ["001.png", "002.png", "ComicInfo.xml"]
    );
}

/// 边车不当页，也不当透传文件：它是打包环境的产物，不是卷的内容。
#[test]
fn a_sidecar_that_sits_next_to_the_pages_is_not_carried_across_either() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    // 这一份没有 `__MACOSX` 那一层：拷到 exFAT 上再打包就是这个样子。
    cbz.page("001.png", &page)
        .file("._001.png", APPLE_DOUBLE)
        .file("Thumbs.db", b"thumbnail cache")
        .file("desktop.ini", b"[.ShellClassInfo]\n")
        .file("@eaDir/001.png@SynoResource", b"nas index")
        .file(".DS_Store", b"\x00\x00\x00\x01Bud1");
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert!(!report.volumes[0].isolated());
    assert_eq!(member_names(&report.volumes[0].output), ["001.png"]);
}

/// 目录卷同形：同一批垃圾从目录里读进来也一样不算成员。
#[test]
fn a_directory_volume_ignores_the_same_system_junk() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::cheap_page());
    volume.file("._001.png", APPLE_DOUBLE);
    volume.file(".DS_Store", b"\x00\x00\x00\x01Bud1");
    volume.file("__MACOSX/._001.png", APPLE_DOUBLE);

    let report = run_volume(&space, &volume);

    assert!(
        !report.volumes[0].isolated(),
        "边车被当成页，整卷进了隔离目录"
    );
    assert_eq!(report.volumes[0].page_count(), 1);
    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        ["001.png"]
    );
}

/// 老式 Windows 打包工具把分隔符写成反斜杠。归一之后它与斜杠分隔的同一份包无从分别。
///
/// 归一之前它触发的是为路径穿越准备的那条拒绝，整卷被拒；而同一份归档在非 Windows 平台上
/// 连拒都不拒，反斜杠被当成文件名里的一个普通字符。
#[test]
fn backslash_separated_member_names_are_normalised_not_refused() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::cheap_page();
    cbz.page(r"volume-a\ch1\001.png", &page)
        .page(r"volume-a\ch2\001.png", &page)
        .file(r"volume-a\ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    let volume = &report.volumes[0];
    assert!(!volume.isolated());
    // 包装层照剥，两章留着，输出成员名一律用斜杠。
    assert_eq!(
        member_names(&volume.output),
        ["ch1/001.png", "ch2/001.png", "ComicInfo.xml"]
    );
    // 报告里的身份也是归一之后的那个名字。
    assert!(
        volume.pages[0].source.ends_with("ch1/001.png"),
        "{:?}",
        volume.pages[0].source
    );
}

/// 盘符仍被拒，且两个平台上是同一句话——归一只管分隔符，管不到「这个名字写的是别处」。
#[test]
fn a_member_name_with_a_drive_letter_is_refused() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(r"C:\001.png", &fixtures::cheap_page());
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("不能当作输出路径"), "{message}");
    assert!(message.contains("盘符"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

/// 反斜杠写的路径穿越同样被拒，说的是穿越，不是分隔符。
#[test]
fn a_backslash_written_traversal_is_still_refused_as_a_traversal() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(r"..\..\001.png", &fixtures::cheap_page());
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("不能当作输出路径"), "{message}");
    assert!(message.contains("走出卷外"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

#[test]
fn an_archive_carries_its_non_page_members_across_byte_for_byte() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page("001.png", &fixtures::cheap_page())
        .file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();
    let before = std::fs::read(&path).expect("读源归档");

    let report = run_paths(&space, [path.as_path()]);

    let carried = fixtures::read_cbz(&report.volumes[0].output)
        .into_iter()
        .find(|(name, _)| name == "ComicInfo.xml")
        .expect("非图片成员应当被带到输出");
    assert_eq!(carried.1, COMIC_INFO.as_bytes(), "透传的内容必须逐字节一致");
    assert_eq!(
        std::fs::read(&path).expect("读源归档"),
        before,
        "源归档被改动了"
    );
}

#[test]
fn a_directory_and_an_archive_can_be_named_in_the_same_run() {
    let space = Workspace::new();
    // 高已经等于面板高：这样的页是两种适配方式的**公共不动点**（页几何批 01 号票），
    // 尺寸断言因此写得出一个字面值。本条问的是容器形态，不是几何。
    const PAGE: Size = Size::new(200, 1680);
    // 四边顶着墨：裁边在它身上是空操作（页几何批 02 号票），尺寸断言仍写得出字面值。
    let page = fixtures::full_bleed_gradient(PAGE);
    let loose = space.volume("volume-a");
    loose.page("001.png", &page);
    let mut packed = space.cbz("volume-b");
    packed.page("001.png", &page);
    let packed = packed.write();

    let report = run_paths(&space, [loose.path(), packed.as_path()]);

    // 同一次调用里两种容器各按自己的形态出去，页的处理没有分叉。
    assert_eq!(report.volumes[0].output, space.out().join("volume-a"));
    assert!(report.volumes[0].output.is_dir());
    assert_eq!(report.volumes[1].output, space.out().join("volume-b.cbz"));
    assert!(report.volumes[1].output.is_file());
    for volume in &report.volumes {
        assert_eq!(volume.pages.len(), 1);
        assert_eq!(volume.pages[0].size, PAGE);
    }
}

#[test]
fn an_archive_whose_structure_cannot_be_read_is_refused_without_leaving_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page("001.png", &fixtures::cheap_page());
    let path = cbz.write_truncated();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("volume-a.cbz"), "{message}");
    assert!(message.contains("可能已损坏"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

/// 透传文件的字节读不出来仍然是**卷级**的失败，而且要指名是哪个成员。
///
/// 页不再走这条路——坏页变成失败页，整卷进隔离目录（12 号票，见 `isolation.rs`）。
/// 透传文件没有那条出路：它逐字节照搬，搬不动就交不出这一卷，
/// 编一份空的 ComicInfo.xml 顶上去只会让阅读器读到假的书籍元信息。
///
/// **卷级失败不再毁掉整趟**（05 号票）：`run` 回的是 `Ok`，那一卷记在
/// `Report::failed_volumes` 里，指名与原因都在那一条上。整趟当场失败的老样子换掉了——
/// 那时前面几十卷的输出还在盘上，而说得清它们是什么的报告全丢了。
#[test]
fn a_pass_through_file_whose_bytes_are_corrupt_is_named_in_the_failed_volume() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    // 归档结构完好，坏的是这一个成员的字节——只有读到它才看得出来。
    cbz.page("001.png", &fixtures::cheap_page())
        .rotten_file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert!(
        report.volumes.is_empty(),
        "没做成的卷混进了做出东西的那一列"
    );
    let [failed] = &report.failed_volumes[..] else {
        panic!("这一卷没被记成卷级失败：{:?}", report.failed_volumes);
    };
    assert_eq!(failed.volume, path, "卷级失败指错了卷");
    assert!(failed.reason.contains("ComicInfo.xml"), "{}", failed.reason);
}

/// 认得的归档扩展名是一个**集合**，拒绝那句话要把这个集合报出来。
///
/// 四个格式全收下之后（`volume-discovery/06` 收下最后那个 `.rar`），这句话报的就是
/// 四个；不认得的扩展名照旧拒得跟一个 `.txt` 一样。
#[test]
fn a_file_that_is_neither_a_directory_nor_a_known_archive_is_refused() {
    let space = Workspace::new();
    let path = space.stray_file("volume-a.txt", b"just a note");

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(
        message.contains("一个卷是一个目录或一个归档（.cbz / .zip / .rar / .7z）"),
        "{message}"
    );
}

/// 扩展名**像**一个摊开那一档的归档、内容不是：那是「读不出归档结构」，
/// 不是「不认得这个扩展名」。
///
/// 两句话分得开才有用：前者说的是这个文件坏了，后者说的是这个格式没收。
/// 点名的这一种整趟拒绝（发现出来的那一种进非卷文件清单，见 `tests/exit_code.rs`）。
/// 两个格式各问一遍：那句话一格式一份（`source` 的 `seven_zip_is_unreadable` 与
/// `rar_is_unreadable`），少测一个就少守一句。
#[test]
fn a_file_that_only_looks_like_a_solid_archive_is_refused_for_being_unreadable() {
    for name in ["volume-a.7z", "volume-a.rar"] {
        let space = Workspace::new();
        let path = space.stray_file(name, b"just a note");

        let message = format!(
            "{:#}",
            run_paths_expecting_failure(&space, [path.as_path()])
        );

        assert!(
            message.contains("读不出") && message.contains("归档结构"),
            "{name}：{message}"
        );
    }
}

/// `.zip` 与 `.cbz` 是同一种字节：同内容的一对，产物逐字节相同，去处也是同一个名字。
///
/// 两个扩展名各跑一趟、各写一个输出根，比的才是「读法有没有分岔」这一件事——
/// 同一个输出根下这一对是撞车（见 `tests/pipeline.rs`），比不成。
#[test]
fn a_zip_is_read_as_a_cbz_and_comes_back_out_as_a_cbz() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();

    let mut as_cbz = space.archive("第10话.cbz");
    as_cbz.page("2.png", &page).page("1.png", &page);
    let as_cbz = as_cbz.write();
    let mut as_zip = space.archive("第10话.zip");
    as_zip.page("2.png", &page).page("1.png", &page);
    let as_zip = as_zip.write();

    let from_cbz = run_paths(&space, [as_cbz.as_path()]);
    let from_zip = tonefit::run(&tonefit::Request {
        output_root: space.out_named("out-zip"),
        ..fixtures::request(&space, [as_zip.as_path()])
    })
    .expect("点名 .zip 应当处理成功");

    // 输出扩展名归一：点名 `.zip`，写出来的仍是 `.cbz`——产物是给阅读器的。
    assert_eq!(from_cbz.volumes[0].output, space.out().join("第10话.cbz"));
    assert_eq!(
        from_zip.volumes[0].output,
        space.out_named("out-zip").join("第10话.cbz")
    );
    // 逐字节相同：同一种字节走的是同一条读法，扩展名一点都没进到产物里。
    //
    // 比的是**成员**，不是整个归档文件：归档的头里带着写出那一刻的时钟
    // （`zip` 的 `DateTime::default_for_write`），整文件的字节因此两趟之间本来就不同，
    // 与读的是 `.zip` 还是 `.cbz` 无关（停车场 Q109）。
    assert_eq!(
        fixtures::read_cbz(&from_zip.volumes[0].output),
        fixtures::read_cbz(&from_cbz.volumes[0].output),
        "同内容的 .zip 与 .cbz 产出了不同的成员"
    );
}

/// 大小写不敏感这一条管整个格式集，不只管 `.cbz`。
#[test]
fn an_uppercase_archive_extension_is_recognised_too() {
    let space = Workspace::new();
    let mut archive = space.archive("第10话.ZIP");
    archive.page("001.png", &fixtures::cheap_page());
    let path = archive.write();

    let report = run_paths(&space, [path.as_path()]);

    assert_eq!(report.volumes[0].output, space.out().join("第10话.cbz"));
}

#[test]
fn a_member_name_that_would_escape_the_volume_is_refused() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page("../001.png", &fixtures::cheap_page());
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("不能当作输出路径"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

/// 归档要么完整、要么不存在：不产出半成品。目录卷同形，见下面那一对用例。
///
/// 触发失败用的是一个**读不出字节的透传文件**：它排在页之后写，页因此已经进了临时归档，
/// 那一刻失败才真的是「写到一半」。坏页触发不了了——它现在被隔离，不再中止整卷（12 号票）。
#[test]
fn an_archive_that_fails_partway_leaves_no_half_written_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page("001.png", &fixtures::cheap_page())
        .rotten_file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    let message = &report.failed_volumes[0].reason;
    assert!(message.contains("ComicInfo.xml"), "{message}");
    let left_behind: Vec<_> = std::fs::read_dir(space.out())
        .expect("输出根目录")
        .map(|entry| {
            entry
                .expect("列输出")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(left_behind.is_empty(), "输出里留下了 {left_behind:?}");
}

#[test]
fn a_run_that_fails_leaves_the_previous_output_archive_intact() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();
    let request = fixtures::request(&space, [space.cbz("volume-a").path()]);

    let mut good = space.cbz("volume-a");
    good.page("001.png", &page).page("002.png", &page);
    good.write();
    let report = tonefit::run(&request).expect("第一次处理应当成功");
    let output = report.volumes[0].output.clone();

    // 同一个位置换成一份会中途失败的卷，再跑一次。触发方式的理由同上一个用例。
    let mut broken = space.cbz("volume-a");
    broken
        .page("001.png", &page)
        .page("002.png", &page)
        .rotten_file("ComicInfo.xml", COMIC_INFO.as_bytes());
    broken.write();
    let report = tonefit::run(&request).expect("卷级失败不该毁掉整趟");
    assert_eq!(report.failed_volumes.len(), 1, "这一卷没被记成卷级失败");

    assert_eq!(
        member_names(&output),
        ["001.png", "002.png"],
        "上一次的成品被这次失败毁掉了"
    );
}

/// 混排卷在归档里仍按阅读顺序排。
///
/// 彩页走的是彩色分支，第一遍就缩放并编好（ADR 0005 决定第 4 条），但它与灰度页
/// 一同在写出那一遍按页序落位。归档成员按**写入顺序**排，而页名的字典序与阅读顺序
/// 本来就对不上（`1` `2` `10`）——彩页要是在第一遍就写进归档，成员顺序会变成
/// 「先全部彩页、再全部灰度页」，按归档顺序翻页的阅读器于是跳着读。
#[test]
fn color_and_gray_pages_come_out_of_the_archive_in_reading_order() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let gray = fixtures::gradient(fixtures::TINY);
    let color = fixtures::color_page(fixtures::TINY);
    cbz.page("10.png", &gray)
        .page("2.png", &color)
        .page("11.png", &color)
        .page("1.png", &gray);
    let path = cbz.write();

    let report = tonefit::run(&tonefit::Request {
        profile: fixtures::profile("kobo-libra-colour"),
        ..fixtures::request(&space, [path.as_path()])
    })
    .expect("处理应当成功");

    // 两条分支各有两页，混着排。
    let volume = &report.volumes[0];
    assert_eq!(
        volume
            .pages
            .iter()
            .filter(|page| page.verdict().is_none())
            .count(),
        2,
        "夹具不对：彩页没走彩色分支，这条用例就什么都没钉住"
    );
    assert_eq!(
        member_names(&volume.output),
        ["1.png", "2.png", "10.png", "11.png"]
    );
}

/// 归档里每个成员装的是**它自己那一页**：尺寸与像素逐个对得上。
///
/// 目录卷那一侧不缺这道哨兵：`pipeline.rs` 与 `isolation.rs` 里一大批用例按
/// `page.output` 逐页读回文件，比尺寸、比像素、比记录。归档成员没有文件系统路径，
/// 读它只有 `fixtures::read_cbz` 一条路，而在这条用例之前没有一处用它解过页——
/// 成员名、成员顺序、成员非空、透传字节都测过，成员**里装着什么**没人问过。
/// 写出时让每个成员都写第一页的字节：目录那一侧红 8 条，归档这一侧只红黄金快照
/// 那一列字节数（变异实验见 08 号票的《落地记录》）。
///
/// 夹具让四页两两分得开：尺寸各不相同，图案也各不相同。三张灰度页只有纯黑与纯白
/// 两个取值（见 `fixtures::black_top_band`），因此断得起逐字节的等号；
/// 彩页走的是彩色分支，颜色原样出去，按色带取样。
#[test]
fn every_archive_member_carries_its_own_pixels_at_its_own_size() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    // 三张灰度页：尺寸与黑带高度都不同，任意两张既不同形也不同图。
    //
    // 高一律取面板高（1264×1680 那块，见 `fixtures::BASELINE_DEVICE`）：这样的页是
    // **两种适配方式的公共不动点**（页几何批 01 号票），写出的像素因此与源逐个相等，
    // 断言问得出「这一格装的是不是它自己的像素」。宽各不相同，串位照样红。
    let grays = [
        ("1.png", Size::new(160, 1680), 8),
        ("2.png", Size::new(120, 1680), 96),
        ("10.png", Size::new(200, 1680), 151),
    ];
    // 彩页夹在中间，且尺寸也与别人不同：两条分支之间串位一样要红。
    let color_size = Size::new(180, 1680);
    let color = fixtures::color_page(color_size);
    for (name, size, black_rows) in grays {
        cbz.page(name, &fixtures::black_top_band(size, black_rows));
    }
    cbz.page("3.png", &color)
        .file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let report = tonefit::run(&tonefit::Request {
        profile: fixtures::profile("kobo-libra-colour"),
        ..fixtures::request(&space, [path.as_path()])
    })
    .expect("处理应当成功");

    // 归档只读回一遍：顺序与内容问的是同一批成员。
    let in_order = fixtures::read_cbz(&report.volumes[0].output);
    let names: Vec<&str> = in_order.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["1.png", "2.png", "3.png", "10.png", "ComicInfo.xml"]
    );
    let members: std::collections::HashMap<&str, &[u8]> = in_order
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();

    for (name, size, black_rows) in grays {
        let written = fixtures::read_png_bytes(members[name]);
        assert_eq!(written.size, size, "{name} 的尺寸不是它自己的");
        assert_eq!(
            written.pixels,
            fixtures::luma_pixels(&fixtures::black_top_band(size, black_rows)),
            "{name} 装的不是它自己的像素"
        );
    }

    // 彩页那一格：色带一条不少，尺寸也是它自己的。
    let written = fixtures::read_color_png_bytes(members["3.png"]);
    assert_eq!(written.size, color_size, "彩页的尺寸不是它自己的");
    for (index, band) in fixtures::COLOR_BANDS.iter().enumerate() {
        assert_eq!(
            written.pixel(
                color_size.width / 2,
                fixtures::band_center_row(color_size, index)
            ),
            *band,
            "彩页第 {index} 条色带"
        );
    }

    assert_eq!(
        members["ComicInfo.xml"],
        COMIC_INFO.as_bytes(),
        "透传成员装的不是它自己的字节"
    );
}

/// 目录卷是**整目录重写**：源里删掉一页，输出里那一页跟着消失。
///
/// 只覆盖本趟写出的文件的话，那一页会原地留着、还带着上一趟的记录，下一趟又被幂等跳过，
/// 从此永久留在输出里——在阅读器里与真页毫无分别。
#[test]
fn a_page_deleted_from_the_source_disappears_from_the_directory_output() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::TINY);
    volume.page("001.png", &page);
    volume.page("002.png", &page);
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());

    let first = run_volume(&space, &volume);
    assert_eq!(
        fixtures::directory_members(&first.volumes[0].output),
        ["001.png", "002.png", "ComicInfo.xml"],
        "夹具不对：头一趟就没写全"
    );

    std::fs::remove_file(volume.path().join("002.png")).expect("从源里删掉一页");
    let second = run_volume(&space, &volume);

    assert_eq!(
        fixtures::directory_members(&second.volumes[0].output),
        ["001.png", "ComicInfo.xml"]
    );
}

/// 写完之后输出里只剩本趟的产物与透传文件：上一趟留下的东西一概不留。
///
/// 用一个陈旧产物钉这条，而不是再钉一次「删掉的页」：整目录重写管的是**所有**
/// 不该再存在的旧成员，删页只是其中最要命的那一种。
#[test]
fn a_directory_volume_keeps_nothing_that_this_run_did_not_write() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());

    let output = run_volume(&space, &volume).volumes[0].output.clone();
    std::fs::write(output.join("陈旧产物.png"), "上一趟的".as_bytes()).expect("放一个陈旧产物");
    // 源多了一页，这一卷因此要重做——陈旧产物本身不是重做的理由。
    volume.page("002.png", &fixtures::solid(fixtures::TINY, 40));
    let second = run_volume(&space, &volume);

    assert_eq!(
        fixtures::directory_members(&second.volumes[0].output),
        ["001.png", "002.png", "ComicInfo.xml"]
    );
}

/// 半卷不出现在最终位置：**头一趟**整卷写完之前，那个目录里一个成员都没有。
///
/// 钉的是 `most == 0`，因此夹具必须是头一趟——最终位置上此前有一份输出的话，
/// 整趟里它都合法地摆在那儿，数出来的就不是半卷而是上一趟那一卷了。
/// 逐页直写的话，最后一页写完的那一刻最终位置上已经躺着大半卷散页——观察者恰好在
/// 那一刻被叫到，这条用例因此站得住。
#[test]
fn a_directory_volume_does_not_appear_at_its_final_path_until_it_is_complete() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::TINY);
    for name in ["001.png", "002.png", "003.png", "004.png"] {
        volume.page(name, &page);
    }
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let watch = WatchDuringRun::new(&space.out().join("volume-a"));

    let report = tonefit::run(&tonefit::Request {
        progress: Some(tonefit::ProgressSink::new(watch.clone())),
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    assert!(watch.steps() > 4, "夹具不对：观察者没被叫到几次");
    assert_eq!(
        watch.most_it_ever_held(),
        0,
        "跑到一半时最终位置上已经有成员了"
    );
    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        ["001.png", "002.png", "003.png", "004.png", "ComicInfo.xml"]
    );
}

/// 目录卷要么完整、要么不存在：中途失败不留半成品目录，临时容器也不留。
///
/// 触发失败的是一个**读不出字节的透传文件**——透传文件排在页之后写，页因此已经进了
/// 临时目录，那一刻失败才真的是「写到一半」。理由与归档那条用例同一条。
#[test]
fn a_directory_volume_that_fails_partway_leaves_no_half_written_output() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::TINY);
    volume.page("001.png", &page);
    volume.page("002.png", &page);
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());

    let report = run_losing_the_extra(&space, &volume);

    let message = &report.failed_volumes[0].reason;
    assert!(message.contains("ComicInfo.xml"), "{message}");
    let left = fixtures::names_in(&space.out());
    assert!(left.is_empty(), "输出里留下了 {left:?}");
}

/// 一趟失败不毁掉上一趟的成品：整目录重写只在收尾那一刻才碰最终位置。
#[test]
fn a_run_that_fails_leaves_the_previous_output_directory_intact() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let page = fixtures::gradient(fixtures::TINY);
    volume.page("001.png", &page);
    volume.page("002.png", &page);
    volume.file("ComicInfo.xml", COMIC_INFO.as_bytes());

    let output = run_volume(&space, &volume).volumes[0].output.clone();
    let before = fixtures::fingerprint(&output);

    // 同一个卷，这一趟中途失败。触发方式的理由同上一个用例。
    run_losing_the_extra(&space, &volume);

    assert_eq!(
        fixtures::fingerprint(&output),
        before,
        "上一次的成品被这次失败毁掉了"
    );
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["volume-a"],
        "输出根里多了东西"
    );
}

/// 上一趟**异常死亡**留下的那格临时容器，这一趟认得出来，也清得掉。
///
/// 正常收场时它由析构收走：跑完改名到位（本文件上面那几条），或者中途失败、被中止
/// （`tests/events.rs` 里中止那几条）。进程被硬杀掉时析构跑不到，盘上就留着一格
/// 改不了名的临时容器——而临时名字是**推得出来**的，下一趟因此认得出它。
/// 那正是它不取随机名字的理由（见 `tonefit` 的 `sink` 里 `partial_path` 那一段）。
///
/// 它落在本文件而不是中止那几条旁边，因为问的是**容器怎么收尾**，不是按停：
/// 清残留这条能力是 `p0-hardening/03` 落地的，本条只是补上它一直没有的用例
/// （会话批 04 号票要求「异常死亡留下的残留，下一趟能识别并清理」，而那件事在这一层）。
///
/// 两种形态各摆一格：目录卷的是目录，归档卷的是文件。里面都塞着上一趟的垃圾——
/// 断言的不只是「残留不见了」，还有「它带的成员没有混进这一趟的输出」。
#[test]
fn a_partial_left_behind_by_a_hard_kill_is_cleaned_up_by_the_next_run() {
    let space = Workspace::new();
    let page = fixtures::gradient(fixtures::TINY);
    let loose = space.volume("volume-a");
    loose.page("001.png", &page);
    let mut cbz = space.cbz("volume-b");
    cbz.page("001.png", &page);
    let packed = cbz.write();

    let stale_directory = space.out().join("volume-a.partial");
    std::fs::create_dir_all(&stale_directory).expect("摆一格残留的临时目录");
    std::fs::write(stale_directory.join("999.png"), b"junk from a killed run")
        .expect("往残留里塞一个成员");
    let stale_file = space.out().join("volume-b.cbz.partial");
    std::fs::write(&stale_file, b"half an archive, no central directory")
        .expect("摆一格残留的临时文件");

    let report = run_paths(&space, [loose.path(), packed.as_path()]);

    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        ["001.png"],
        "残留临时目录里的成员混进了这一趟的输出"
    );
    assert_eq!(member_names(&report.volumes[1].output), ["001.png"]);
    assert_eq!(
        fixtures::names_in(&space.out()),
        ["volume-a", "volume-b.cbz"],
        "残留的那两格临时容器还在"
    );
}

/// 同一批输入在两种容器形态上给出**一致的输出成员**——源里删掉一页之后也一致。
///
/// 归档卷整个重写，删掉的页本来就不会留下；目录卷此前只覆盖本趟写出的文件，
/// 同一条标准于是在两种形态上给出不同答案。
#[test]
fn both_container_shapes_hold_the_same_members_after_a_page_is_deleted() {
    let space = Workspace::new();
    let page = fixtures::gradient(fixtures::TINY);
    let loose = space.volume("volume-a");
    let pack = |names: &[&str]| {
        let mut cbz = space.cbz("volume-b");
        for name in names {
            cbz.page(name, &page);
        }
        cbz.file("ComicInfo.xml", COMIC_INFO.as_bytes());
        cbz.write()
    };

    loose.page("001.png", &page);
    loose.page("002.png", &page);
    loose.file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let packed = pack(&["001.png", "002.png"]);
    run_paths(&space, [loose.path(), packed.as_path()]);

    std::fs::remove_file(loose.path().join("002.png")).expect("从目录卷里删掉一页");
    let packed = pack(&["001.png"]);
    let report = run_paths(&space, [loose.path(), packed.as_path()]);

    let mut packed_members = member_names(&report.volumes[1].output);
    packed_members.sort();
    assert_eq!(
        fixtures::directory_members(&report.volumes[0].output),
        packed_members
    );
    assert_eq!(packed_members, ["001.png", "ComicInfo.xml"]);
}

/// 跑一趟，**这一卷刚被重开那一刻**把源里的透传文件抽走，返回那份报告。
///
/// 造「写到一半才失败」用它：成员在那一刻就枚举完了，读到它是第二遍的事，那时页已经写进
/// 临时容器。抽走那一刻与判别式的理由都在 `fixtures::RemoveOnceTheVolumeIsOpen` 上。
///
/// 回的是报告而不是错误：**预扫之后才出的卷级失败不毁掉整趟**（05 号票），
/// 那一卷记在 `Report::failed_volumes` 里，原因也在那一条上。
fn run_losing_the_extra(space: &Workspace, volume: &fixtures::Volume) -> tonefit::Report {
    let removal = fixtures::RemoveOnceTheVolumeIsOpen::member(volume.path(), "ComicInfo.xml");
    let report = tonefit::run(&tonefit::Request {
        progress: Some(tonefit::ProgressSink::new(removal)),
        ..fixtures::request(space, [volume.path()])
    })
    .expect("卷级失败不该毁掉整趟");
    assert_eq!(
        report.failed_volumes.len(),
        1,
        "抽走透传文件之后这一卷没被记成卷级失败"
    );
    report
}

/// 跑到一半时最终位置上有多少个成员——每报到一步问一次，留下见过的最大值。
///
/// 观察者是库唯一向外开的、跑到一半还插得上话的口子（见 `tonefit::Progress`）：
/// 「半卷不出现在最终位置」是一条**过程中**的性质，跑完再看是看不出来的。
#[derive(Clone)]
struct WatchDuringRun {
    path: PathBuf,
    /// 观察者要交给库、用例又要读回它记下的数，因此这一格共享——
    /// `ProgressSink` 收的是所有权，留不下第二个把手。
    seen: Arc<Mutex<Seen>>,
}

/// 一趟跑下来记住的两个数。
#[derive(Default)]
struct Seen {
    steps: usize,
    most: usize,
}

impl WatchDuringRun {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            seen: Arc::new(Mutex::new(Seen::default())),
        }
    }

    fn look(&self) {
        let held = fixtures::directory_members(&self.path).len();
        let mut seen = self.seen.lock().expect("看一眼最终位置");
        seen.most = seen.most.max(held);
    }

    fn most_it_ever_held(&self) -> usize {
        self.seen.lock().expect("读回见过的最大值").most
    }

    fn steps(&self) -> usize {
        self.seen.lock().expect("读回报到次数").steps
    }
}

impl tonefit::Progress for WatchDuringRun {
    /// 开卷与走完一步这两条上各看一眼。
    ///
    /// 一卷跑完那一条上**不看**：收尾改名排在它之前，此刻最终位置上应该已经是一整卷了。
    /// 别的事件同理不看——它们与「最终位置上此刻有几个成员」无关。
    fn observe(&self, event: tonefit::Event<'_>) -> tonefit::Instruction {
        match event {
            tonefit::Event::VolumeStarted { .. } => self.look(),
            tonefit::Event::Stepped { .. } => {
                self.seen.lock().expect("记一步").steps += 1;
                self.look();
            }
            _ => {}
        }
        tonefit::Instruction::Continue
    }
}

/// 一个归档里成员名的清单，按归档里的顺序。
fn member_names(archive: &std::path::Path) -> Vec<String> {
    fixtures::read_cbz(archive)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// **点名一个 `.7z` 直接就能跑，产物与同内容的 `.cbz` 逐字节相同**
/// （`volume-discovery/05`，ADR 0015 决定第 3 条）。
///
/// 两种格式两条路——`.7z` 开工前整卷摊到临时目录、之后按目录卷走，`.cbz` 随机取——
/// 而这一条钉的正是**下游看不出分别**：读法的差异被关在源那一层里，
/// 判定、几何、量化、编码、透传一件都不知道「固实」这个词。
///
/// 报告那一格一并比：`.cbz` 那一卷摊了 0 字节，也就是它**没走摊开那一条**。
#[test]
fn a_seven_zip_comes_out_byte_for_byte_the_same_as_the_cbz_holding_the_same_pages() {
    let space = Workspace::new();
    let page = fixtures::cheap_page();
    let other = fixtures::gradient(fixtures::TINY);

    let mut cbz = space.cbz("volume-a");
    cbz.page("ch1/002.jpg", &other)
        .page("ch1/001.png", &page)
        .file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let packed = cbz.write();

    let mut sevenz = space.sevenz("volume-a");
    sevenz
        .page("ch1/002.jpg", &other)
        .page("ch1/001.png", &page)
        .file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let solid = sevenz.write();

    // 两趟各写各的输出根：两个卷同名，落到同一处会撞车（ADR 0015 认下的那种归一撞车）。
    let from_cbz = run_paths(&space, [packed.as_path()]);
    let from_seven_zip = tonefit::run(&tonefit::Request {
        output_root: space.out_named("out-7z"),
        ..fixtures::request(&space, [solid.as_path()])
    })
    .expect("点名一个 .7z 该跑得起来");

    // 输出仍一律 `.cbz`：输入是哪一个扩展名都不带过来（ADR 0015 决定第 2 条）。
    assert_eq!(
        from_seven_zip.volumes[0].output,
        space.out_named("out-7z").join("volume-a.cbz")
    );
    assert_eq!(
        fixtures::read_cbz(&from_seven_zip.volumes[0].output),
        fixtures::read_cbz(&from_cbz.volumes[0].output),
        "两种格式的同一卷，产物不是同一串字节"
    );

    // 摊了多少字节进了报告：那笔磁盘账没有旋钮，这个数是它唯一可见的形式。
    assert!(
        from_seven_zip.volumes[0].extracted > 0,
        "点名一个 .7z，报告里却说一个字节都没摊开"
    );
    assert_eq!(
        from_cbz.volumes[0].extracted, 0,
        "`.cbz` 走了摊开那一条：它该是随机取"
    );
}

/// **摊开的临时目录跑到一半在、跑完不在、中止之后也不在**（`volume-discovery/05`）。
///
/// 三问一次答完，因为它们是同一条寿命的三个时刻：那一份活在卷上，卷一放掉就收
/// （见 `source::Extraction`）。中止走的是既有的两级停——那一卷当场返回，卷跟着析构
/// （ADR 0013 决定第 2 条）。
///
/// 「跑到一半」只有**跑到一半**才看得见，因此从观察者那一侧看（与本文件
/// `WatchDuringRun` 同一条道理）。认哪个目录是这一卷摊开的那一份，靠的是包里一个
/// 别处不会出现的成员名：系统临时目录是公共的，光按名字前缀筛会把别的用例正在用的
/// 那一份也筛进来。
///
/// **它证的不是「摊开途中按得停」。**观察者要先**看见**那个目录才改口答中止，
/// 也就是说中止落在摊开**之后**。摊开那一整段里没有检查点——按下中止要等它解完——
/// 那是停车场 Q121 记着的一个空档，这条用例够不着它，别把它当成有人守着。
#[test]
fn a_seven_zip_leaves_no_temporary_directory_behind_even_when_the_run_is_aborted() {
    /// 这个成员只在本用例的包里出现，认摊开的那个目录靠它。
    const MARKER: &str = "只此一份的页.png";

    let space = Workspace::new();
    let mut sevenz = space.sevenz("volume-a");
    sevenz.page(MARKER, &fixtures::cheap_page());
    let solid = sevenz.write();

    for (name, answer) in [
        ("out-完整", tonefit::Instruction::Continue),
        ("out-中止", tonefit::Instruction::Abort),
    ] {
        let watcher = WatchTheExtraction::answering(MARKER, answer);
        let report = tonefit::run(&tonefit::Request {
            output_root: space.out_named(name),
            progress: Some(tonefit::ProgressSink::new(watcher.clone())),
            ..fixtures::request(&space, [solid.as_path()])
        })
        .expect("摊开这件事本身不该让整趟失败");
        if answer == tonefit::Instruction::Abort {
            assert!(report.volumes.is_empty(), "中止掉的那一卷进了报告");
        }

        let seen = watcher.seen();
        assert!(
            !seen.is_empty(),
            "跑到一半临时目录里一个摊开的目录都没有（{name}）"
        );
        for dir in seen {
            assert!(!dir.exists(), "跑完之后 {} 还在（{name}）", dir.display());
        }
    }
}

/// **摊不开是卷级失败，其余卷照做**（`volume-discovery/05`，ADR 0015）。
///
/// 造它的是一个**压缩流被打坏的** `.7z`：归档头完好，预扫列得出成员——那一卷因此
/// 既不是「点名的点不开」也不是非卷文件；坏的是那一段字节，只有真去摊开才看得出来。
/// 磁盘不够走的是同一条路（[`source::extract`] 的每一个 `Err`），
/// 而那一种在用例里造不出来。退出码那一格由 `tests/exit_code.rs` 钉着。
#[test]
fn a_seven_zip_that_cannot_be_extracted_fails_only_its_own_volume() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");

    let mut broken = fixtures::SevenZip::new(library.join("坏的.7z"));
    broken.page("001.png", &fixtures::cheap_page());
    let broken = broken.write_with_a_broken_stream();

    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();

    let report = run_paths(&space, [library.as_path()]);

    assert_eq!(
        report.failed_volumes.len(),
        1,
        "摊不开的那一卷没被记成卷级失败"
    );
    assert_eq!(report.failed_volumes[0].volume, broken);
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照做，或者摊不开的那一卷也写出了东西"
    );
}

/// **四个格式装同一批页，出来是同一串字节**（批 spec 的《Testing Decisions》，
/// ADR 0015 决定第 1、2、3 条）。
///
/// `volume-discovery/06` 收下最后那个 `.rar`，格式集因此齐了，这一条才摆得出来。
/// 它一次钉住三件事：
///
/// - **读法的差异被关在源那一层里**——两条读法（随机取、摊开）在这里各走两个格式，
///   而判定、几何、量化、编码、透传一件都不知道这一卷原来是什么包。
/// - **输出扩展名归一**：四份产物都叫 `.cbz`，输入是哪一个都不带过来（决定第 2 条）。
/// - **摊开只发生在该发生的两个格式上**：`.cbz` / `.zip` 那两卷摊了 0 字节。
///
/// 四个包装的是同一份内容，那个前提的出处只有一处：`.rar` 那一侧是签进仓的字节
/// （造它的命令行见 `fixtures::rar` 抬头），另外三个照 `fixtures::rar::members()` 灌。
/// 四卷各写各的输出根：卷名都一样，落到同一处会撞车（ADR 0015 认下的那种归一撞车）。
#[test]
fn every_archive_format_turns_the_same_pages_into_the_same_product() {
    let space = Workspace::new();
    let members = fixtures::rar::members();

    let mut cbz = space.cbz("volume-a");
    let mut zip = space.archive("volume-a.zip");
    let mut sevenz = space.sevenz("volume-a");
    for (name, bytes) in &members {
        cbz.file(name, bytes);
        zip.file(name, bytes);
        sevenz.file(name, bytes);
    }

    let packed = [
        ("out-cbz", cbz.write(), 0),
        ("out-zip", zip.write(), 0),
        ("out-rar", space.rar("volume-a", fixtures::rar::SOLID), 1),
        ("out-7z", sevenz.write(), 1),
    ];

    let mut products = Vec::new();
    for (out, volume, extracts) in &packed {
        let report = tonefit::run(&tonefit::Request {
            output_root: space.out_named(out),
            ..fixtures::request(&space, [volume.as_path()])
        })
        .unwrap_or_else(|error| panic!("点名 {} 该跑得起来：{error:#}", volume.display()));

        assert_eq!(
            report.volumes[0].output,
            space.out_named(out).join("volume-a.cbz"),
            "{} 的产物没归一到 .cbz",
            volume.display()
        );
        // 摊了多少字节进了报告：那笔磁盘账没有旋钮，这个数是它唯一可见的形式。
        assert_eq!(
            report.volumes[0].extracted > 0,
            *extracts == 1,
            "{} 走岔了读取形态：摊开了 {} 字节",
            volume.display(),
            report.volumes[0].extracted
        );
        products.push((
            volume.clone(),
            fixtures::read_cbz(&report.volumes[0].output),
        ));
    }

    let (first, expected) = &products[0];
    for (volume, product) in &products[1..] {
        assert_eq!(
            product,
            expected,
            "{} 与 {} 装着同一批页，产物却不是同一串字节",
            volume.display(),
            first.display()
        );
    }
}

/// **固实压的与存储的走同一条，产物相同**（`volume-discovery/06`，ADR 0015 决定第 3 条）。
///
/// 读取形态**按格式分，不逐卷探固实与否**：逐卷探要先读一遍归档头，而那正是这条决定
/// 想省掉的那一次。因此一份打包时关掉了固实的 `.rar` 也照样摊开——白付一次全量写盘，
/// 那是那条决定认下的代价，而这一条钉的是**代价之外什么都没变**。
///
/// 两份夹具的成员逐一相同，差的只有 `-s -m5` 与 `-s- -m0`（见 `fixtures::rar` 抬头）。
#[test]
fn a_solid_rar_and_a_stored_one_come_out_the_same() {
    let space = Workspace::new();
    let solid = space.rar("固实", fixtures::rar::SOLID);
    let stored = space.rar("存储", fixtures::rar::STORED);

    let report = run_paths(&space, [solid.as_path(), stored.as_path()]);

    assert_eq!(report.volumes.len(), 2, "两卷没都跑起来");
    assert_eq!(
        fixtures::read_cbz(&report.volumes[0].output),
        fixtures::read_cbz(&report.volumes[1].output),
        "固实压的与存储的出来不是同一串字节"
    );
    for volume in &report.volumes {
        assert!(
            volume.extracted > 0,
            "{} 没走摊开那一条：固实与否不该改读法",
            volume.volume.display()
        );
    }
}

/// **摊不开的 `.rar` 是卷级失败，其余卷照做**（ADR 0015 决定第 3 条，
/// 与 `.7z` 那一条 `a_seven_zip_that_cannot_be_extracted_fails_only_its_own_volume` 并列）。
///
/// 造它的是一份**成员字节被打坏的** `.rar`：归档头完好，预扫列得出成员——那一卷因此
/// 既不是「点名的点不开」也不是非卷文件；坏的是那一段字节，只有真去摊开才看得出来。
/// 「磁盘不够」走的是同一条路（`source::extract` 的每一个 `Err`），而那一种在用例里
/// 造不出来；退出码那一格由 `tests/exit_code.rs` 钉着，且那一条对格式无所谓——
/// 它断在临时目录根本不在上，摊开那一层是两个格式共用的。
#[test]
fn a_rar_that_cannot_be_extracted_fails_only_its_own_volume() {
    /// 存储不压的那份夹具里，这个位置落在头一页的字节中段：两头的归档头都碰不到。
    const INSIDE_THE_FIRST_PAGE: usize = 2000;

    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let broken =
        fixtures::rar::write_with_a_broken_member(library.join("坏的.rar"), INSIDE_THE_FIRST_PAGE);
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();

    let report = run_paths(&space, [library.as_path()]);

    assert_eq!(
        report.failed_volumes.len(),
        1,
        "摊不开的那一卷没被记成卷级失败"
    );
    assert_eq!(report.failed_volumes[0].volume, broken);
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照做，或者摊不开的那一卷也写出了东西"
    );
}

/// **加密卷不另开一种结局**：走「点名的 / 发现的」那条既有分别（`volume-discovery/06`，
/// ADR 0014 决定第 5 条）。
///
/// 头都是密的，列成员就要口令，而 tonefit 一处都问不出口令——这一卷因此**点不开**。
/// 点不开的处置早就定死了：点名的整趟拒绝（他明说了要处理它），发现的进非卷文件那张表、
/// 其余卷照做。加密只是「点不开」的又一个来处，不是第五种结局。
///
/// 那句话要说得出**为什么**：一个带口令的包没有坏，说成「可能已损坏」会把用户支去修
/// 一份好好的包（见 `source` 的 `rar_is_unreadable`）。退出码那一格由 `tests/exit_code.rs` 钉着。
#[test]
fn an_encrypted_rar_is_refused_when_named_and_listed_as_a_non_volume_file_when_discovered() {
    let space = Workspace::new();
    let library = space.dir("库");
    std::fs::create_dir_all(&library).expect("建库目录");
    let locked = fixtures::rar::write(library.join("加密的.rar"), fixtures::rar::ENCRYPTED);
    let mut good = fixtures::Cbz::new(library.join("好的.cbz"));
    good.page("001.png", &fixtures::cheap_page());
    good.write();

    // 点名它：整趟拒绝，那句话说得出是口令。
    let message = format!(
        "{:#}",
        run_paths_expecting_failure(&space, [locked.as_path()])
    );
    assert!(
        message.contains("口令"),
        "拒绝那句话没说是口令的事：{message}"
    );

    // 发现它：进非卷文件那张表，其余卷照做。
    let report = run_paths(&space, [library.as_path()]);
    let listed: Vec<&std::path::Path> = report
        .non_volume_files
        .iter()
        .map(|file| file.path.as_path())
        .collect();
    assert_eq!(listed, [locked.as_path()], "加密卷没进非卷文件清单");
    assert!(
        matches!(
            report.non_volume_files[0].reason,
            tonefit::NonVolumeReason::Unopenable(_)
        ),
        "加密卷进清单的理由不是「点不开」"
    );
    assert_eq!(
        fixtures::directory_members(&space.out()),
        ["库/好的.cbz"],
        "其余卷没照做，或者加密卷也写出了东西"
    );
}

/// 跑到一半时**摊开的那些临时目录**是哪几个——每报到一步问一次，见过的都留下。
///
/// 认它靠的是包里那个别处不会出现的成员名：系统临时目录是公共的，光按名字前缀筛
/// 会把别的用例正在用的那一份也筛进来。前缀那一道仍在，为的是不去 stat 一整个临时目录。
#[derive(Clone)]
struct WatchTheExtraction {
    marker: &'static str,
    answer: tonefit::Instruction,
    seen: Arc<Mutex<Vec<PathBuf>>>,
}

impl WatchTheExtraction {
    fn answering(marker: &'static str, answer: tonefit::Instruction) -> Self {
        Self {
            marker,
            answer,
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 见过的那些目录，去重。
    fn seen(&self) -> Vec<PathBuf> {
        let mut seen = self.seen.lock().expect("读回见过的目录").clone();
        seen.sort();
        seen.dedup();
        seen
    }

    fn look(&self) {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `source::EXTRACTION_PREFIX`。前缀不对就不必再去 stat 里面那个成员。
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("tonefit-"))
            {
                continue;
            }
            if path.join(self.marker).is_file() {
                self.seen.lock().expect("记一个目录").push(path);
            }
        }
    }
}

impl tonefit::Progress for WatchTheExtraction {
    /// 每条事件都看一眼，而那个字**要等摊开真的发生了才答**。
    ///
    /// 开工那一条上就答中止的话，卷边界那个检查点当场停下，摊开压根没发生——
    /// 这条用例要问的正是「摊开之后中止，那一份收不收得走」。
    fn observe(&self, _event: tonefit::Event<'_>) -> tonefit::Instruction {
        self.look();
        if self.seen.lock().expect("读回见过的目录").is_empty() {
            return tonefit::Instruction::Continue;
        }
        self.answer
    }
}
