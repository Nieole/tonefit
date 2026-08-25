//! 容器：卷从哪里读、写到哪里去。
//!
//! 目录与 CBZ 走同一个源抽象，因此这些用例大多成对出现：同一条性质，两种容器各测一次。

mod fixtures;

use fixtures::{Workspace, run_paths, run_paths_expecting_failure, run_volume};

/// 透传要逐字节一致，因此这份夹具故意带上非 ASCII 与换行。
const COMIC_INFO: &str = "<?xml version=\"1.0\"?>\n<ComicInfo><Title>卷一</Title></ComicInfo>\n";

#[test]
fn a_directory_volume_carries_its_non_page_files_across_byte_for_byte() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    // 两个并列的顶层目录开始承担顺序了，剥掉就把两章合并了。
    cbz.page("ch1/001.png", &page).page("ch2/001.png", &page);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    assert_eq!(
        member_names(&report.volumes[0].output),
        ["ch1/001.png", "ch2/001.png"]
    );
}

#[test]
fn an_archive_carries_its_non_page_members_across_byte_for_byte() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    )
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
        assert_eq!(volume.pages[0].size, fixtures::SMALLER_THAN_TARGET);
    }
}

#[test]
fn an_archive_whose_structure_cannot_be_read_is_refused_without_leaving_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    let path = cbz.write_truncated();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("volume-a.cbz"), "{message}");
    assert!(message.contains("可能已损坏"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

#[test]
fn a_member_whose_bytes_are_corrupt_is_named_in_the_error() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    // 归档结构完好，坏的是这一个成员的字节——只有读到它才看得出来。
    cbz.rotten_page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("001.png"), "{message}");
}

#[test]
fn a_file_that_is_neither_a_directory_nor_an_archive_is_refused() {
    let space = Workspace::new();
    let path = space.stray_file("volume-a.txt", b"just a note");

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(
        message.contains("一个卷是一个目录或一个 CBZ 归档"),
        "{message}"
    );
}

#[test]
fn a_member_name_that_would_escape_the_volume_is_refused() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(
        "../001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    );
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("不能当作输出路径"), "{message}");
    assert!(!space.out().exists(), "被拒绝之后仍然写了输出");
}

#[test]
fn an_archive_that_fails_partway_leaves_no_half_written_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    )
    // 触发失败用的是一张坏图，只因为现在没有别的办法在写到一半时失败。
    // 12 号票会把坏图改成隔离而不是中止，届时这里要换一个触发方式——
    // 要钉住的性质是「归档要么完整、要么不存在」，不是「坏图让整卷失败」。
    .file("002.png", b"not a png at all");
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("002.png"), "{message}");
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
    let request = fixtures::request(&space, [space.cbz("volume-a").path()]);

    let mut good = space.cbz("volume-a");
    good.page("001.png", &page).page("002.png", &page);
    good.write();
    let report = tonefit::run(&request).expect("第一次处理应当成功");
    let output = report.volumes[0].output.clone();

    // 同一个位置换成一份会中途失败的卷，再跑一次。触发方式的注意事项同上一个用例。
    let mut broken = space.cbz("volume-a");
    broken
        .page("001.png", &page)
        .file("002.png", b"not a png at all");
    broken.write();
    tonefit::run(&request).expect_err("第二次处理应当失败");

    assert_eq!(
        member_names(&output),
        ["001.png", "002.png"],
        "上一次的成品被这次失败毁掉了"
    );
}

/// 一个归档里成员名的清单，按归档里的顺序。
fn member_names(archive: &std::path::Path) -> Vec<String> {
    fixtures::read_cbz(archive)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}
