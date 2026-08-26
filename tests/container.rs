//! 容器：卷从哪里读、写到哪里去。
//!
//! 目录与 CBZ 走同一个源抽象，因此这些用例大多成对出现：同一条性质，两种容器各测一次。

mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// 透传文件的字节读不出来仍然是**卷级**的失败，而且要指名是哪个成员。
///
/// 页不再走这条路——坏页变成失败页，整卷进隔离目录（12 号票，见 `isolation.rs`）。
/// 透传文件没有那条出路：它逐字节照搬，搬不动就交不出这一卷，
/// 编一份空的 ComicInfo.xml 顶上去只会让阅读器读到假的书籍元信息。
#[test]
fn a_pass_through_file_whose_bytes_are_corrupt_is_named_in_the_error() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    // 归档结构完好，坏的是这一个成员的字节——只有读到它才看得出来。
    cbz.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    )
    .rotten_file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
    assert!(message.contains("ComicInfo.xml"), "{message}");
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

/// 归档要么完整、要么不存在：不产出半成品。目录卷同形，见下面那一对用例。
///
/// 触发失败用的是一个**读不出字节的透传文件**：它排在页之后写，页因此已经进了临时归档，
/// 那一刻失败才真的是「写到一半」。坏页触发不了了——它现在被隔离，不再中止整卷（12 号票）。
#[test]
fn an_archive_that_fails_partway_leaves_no_half_written_output() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    cbz.page(
        "001.png",
        &fixtures::gradient(fixtures::SMALLER_THAN_TARGET),
    )
    .rotten_file("ComicInfo.xml", COMIC_INFO.as_bytes());
    let path = cbz.write();

    let error = run_paths_expecting_failure(&space, [path.as_path()]);

    let message = format!("{error:#}");
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
    let page = fixtures::gradient(fixtures::SMALLER_THAN_TARGET);
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
    tonefit::run(&request).expect_err("第二次处理应当失败");

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

/// 半卷永远不出现在最终位置：整卷写完之前，那个目录里一个成员都没有。
///
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

    let error = run_losing_the_extra(&space, &volume);

    let message = format!("{error:#}");
    assert!(message.contains("ComicInfo.xml"), "{message}");
    let left = left_in(&space.out());
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
    assert_eq!(left_in(&space.out()), ["volume-a"], "输出根里多了东西");
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

/// 跑一趟，开工那一刻把源里的透传文件抽走，返回那个错误。
///
/// 造「写到一半才失败」用它：成员在 `volume_started` 之前就枚举完了，读到它是第二遍的事，
/// 那时页已经写进临时容器。目录卷没有归档那种坏 CRC 可造——文件系统上一个文件
/// 要么读得出来，要么根本不在。
fn run_losing_the_extra(space: &Workspace, volume: &fixtures::Volume) -> anyhow::Error {
    let removal = RemoveAtStart::new(&volume.path().join("ComicInfo.xml"));
    tonefit::run(&tonefit::Request {
        progress: Some(tonefit::ProgressSink::new(removal)),
        ..fixtures::request(space, [volume.path()])
    })
    .expect_err("处理应当失败")
}

/// 输出根下留着的那些名字，按名字排序。半成品也在这个清单里——正是要断言它不在的东西。
fn left_in(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| {
            entry
                .expect("列输出")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
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
    fn volume_started(&self, _volume: &Path, _steps: u64) {
        self.look();
    }

    fn stepped(&self) {
        self.seen.lock().expect("记一步").steps += 1;
        self.look();
    }

    /// 收尾排在这条报到之前，此刻最终位置上**应该**已经是一整卷了——不看。
    fn volume_finished(&self) {}
}

/// 开工那一刻把源里的一个文件抽走：成员已经枚举过，读到它时就读不出来了。
struct RemoveAtStart {
    path: PathBuf,
}

impl RemoveAtStart {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl tonefit::Progress for RemoveAtStart {
    fn volume_started(&self, _volume: &Path, _steps: u64) {
        let _ = std::fs::remove_file(&self.path);
    }

    fn stepped(&self) {}

    fn volume_finished(&self) {}
}

/// 一个归档里成员名的清单，按归档里的顺序。
fn member_names(archive: &std::path::Path) -> Vec<String> {
    fixtures::read_cbz(archive)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}
