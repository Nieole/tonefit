//! 错误处理与隔离目录，在 `run(Request) -> Report` 这个 seam 上测。
//!
//! 只断言外部可见的事实：`run` 返没返回错误、`Report` 里列着哪几页失败、
//! 卷写到了哪个目录、隔离目录里那一卷是不是完整的。
//!
//! 一条贯穿全篇的界线：**救得回像素就照用**（04 号票收紧了 12 号票的说法）。
//! 救回到像素的页照用，哪怕只回来一半——那是一张**部分救回页**；
//! 一个像素都救不回来、尺寸解不出来、或尺寸解得出来而缓冲分配不下，才算失败。

mod fixtures;

use fixtures::{Workspace, run_paths, run_volume};
use tonefit::{Mode, Size};

/// 隔离目录在输出根下的名字。测试把它写死，因为它是用户看得见的那个事实。
const ISOLATED: &str = "_isolated";

/// 一份透传文件的内容。故意带上非 ASCII 与换行——透传要逐字节一致。
const COMIC_INFO: &[u8] = "<?xml version=\"1.0\"?>
<ComicInfo><Title>卷一</Title></ComicInfo>
"
.as_bytes();

/// 基准面板的分辨率。一页好页都没有的卷退到它。
const PANEL: Size = Size::new(1264, 1680);

/// 一张坏图不再毁掉整卷：其余页照常判定、照常写出（spec 的 story 24）。
#[test]
fn one_page_that_cannot_be_decoded_does_not_take_the_volume_down() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");
    volume.page("003.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    // 失败页按阅读顺序占住自己那一格：页数不因为一页坏了就少一页。
    assert_eq!(reported.page_count(), 3);
    assert!(reported.pages[0].failure().is_none());
    assert!(reported.pages[1].failure().is_some(), "坏页没被认出来");
    assert!(reported.pages[2].failure().is_none());
    // 两张好页照常有判定，也照常落了盘。
    for index in [0, 2] {
        let page = &reported.pages[index];
        assert!(page.verdict().is_some(), "好页丢了判定");
        assert!(page.output.is_file(), "{} 没写出来", page.output.display());
    }
}

/// 失败页仍以卷内统一的尺寸产出：一页坏了，卷内尺寸不因此参差。
#[test]
fn a_failed_page_still_comes_out_at_the_size_the_rest_of_the_volume_uses() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");
    volume.page("003.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    let pages = &report.volumes[0].pages;
    let uniform = pages[0].size;
    assert_eq!(pages[2].size, uniform, "夹具不对：两张好页该是同一个尺寸");
    assert_eq!(pages[1].size, uniform, "失败页没被强制到卷内统一尺寸");

    let written = fixtures::read_png(&pages[1].output);
    assert_eq!(written.size, uniform, "写出的占位页尺寸对不上报告");
    // 占位页是纸白：它顶住位置，但不冒充内容。
    assert!(
        written.pixels.iter().all(|&pixel| pixel == 255),
        "占位页不是纸白"
    );
}

/// 含失败页的卷输出到隔离目录，并在报告里被标记（spec 的 story 25）。
#[test]
fn a_volume_with_a_failed_page_goes_to_the_isolation_directory() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert!(reported.isolated(), "含失败页的卷没被标记");
    assert_eq!(reported.output, space.out().join(ISOLATED).join("volume-a"));
    assert!(reported.output.is_dir());
    // 干净的那个去处不该同时有一份：一卷只有一个去处。
    assert!(
        !space.out().join("volume-a").exists(),
        "隔离的卷同时留在了干净的去处"
    );
    // 隔离目录里的卷是完整的：坏页占着位，好页照常在。
    for page in &reported.pages {
        assert!(page.output.is_file(), "{} 没写出来", page.output.display());
    }
}

/// 干净的卷不进隔离目录：隔离是标记，不是默认去处。
#[test]
fn a_volume_without_a_failed_page_stays_out_of_the_isolation_directory() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    assert!(!report.volumes[0].isolated());
    assert_eq!(report.volumes[0].output, space.out().join("volume-a"));
    assert!(!space.out().join(ISOLATED).exists());
}

/// `Report` 列出每一个失败页与原因（spec 的 story 26）。
#[test]
fn the_report_lists_every_failed_page_with_a_reason_that_names_it() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", b"");
    volume.page("002.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("003.png", b"not a png at all");

    let report = run_volume(&space, &volume);

    let failures: Vec<_> = report.failures().collect();
    assert_eq!(failures.len(), 2, "失败页集数不对");
    for page in failures {
        let name = page
            .source
            .file_name()
            .expect("失败页也有名字")
            .to_string_lossy()
            .into_owned();
        let reason = page.failure().expect("失败页必有原因");
        assert!(reason.contains(&name), "原因里没指名是哪一页：{reason}");
    }
    // 卷那一侧数出来的是同一批页。
    assert_eq!(report.volumes[0].failures().count(), 2);
}

/// 零字节文件、非图片文件、超大尺寸页都只是失败页，不中止进程。
#[test]
fn a_zero_byte_a_non_image_and_an_oversized_page_are_isolated_rather_than_fatal() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", b"");
    volume.file("002.png", b"not a png at all");
    volume.file("003.png", &fixtures::oversized_page());
    volume.page("004.png", &fixtures::gradient(fixtures::TYPICAL));

    // `run_volume` 里那句 expect 就是这条用例要的断言：进程没中止，调用也没返回错误。
    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert_eq!(reported.failures().count(), 3);
    assert!(reported.pages[3].verdict().is_some(), "唯一的好页也没了");
    // 四页一页不少地写了出去，坏页三张用的是那张好页的尺寸。
    assert_eq!(reported.page_count(), 4);
    for page in &reported.pages {
        assert_eq!(page.size, reported.pages[3].size);
        assert!(page.output.is_file());
    }
}

/// 救回到像素的截断页是**部分救回页**：它按自己的尺寸出，不把卷送进隔离目录，
/// 而报告说得出它救回了多少（04 号票）。
#[test]
fn a_truncated_page_that_still_gives_back_pixels_is_salvaged_rather_than_failed() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", &fixtures::truncated_page(fixtures::TYPICAL));

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert!(!reported.isolated(), "部分救回页不该把卷送进隔离目录");
    let page = &reported.pages[0];
    assert!(page.failure().is_none(), "部分救回页不该算失败");
    assert!(page.verdict().is_some(), "部分救回页照常有判定");
    // 尺寸按它自己的源尺寸算：完整尺寸解得出来，几何因此照常成立。
    assert_eq!(page.size, Size::new(1182, 1680));

    // 救回了多少说得出来——这正是「救回 99% 与救回 0 行是同一种结果」被拆开的地方。
    let salvage = page.salvage().expect("截断页该是一张部分救回页");
    assert!(
        (0.0..1.0).contains(&salvage.share()) && salvage.share() > 0.0,
        "救回的比例落在两端上：{}",
        salvage.share()
    );
    // 卷那一侧数出来的是同一张页。
    assert_eq!(reported.salvaged().count(), 1);
    assert_eq!(report.salvaged().count(), 1);

    let written = fixtures::read_png(&page.output);
    assert_eq!(written.size, page.size);
    // 解出来的那一段是真像素（源是纯黑），缺的那一段留白。
    assert_eq!(written.pixel(0, 0), 0, "解出来的那一段丢了");
    assert_eq!(
        written.pixel(0, written.size.height - 1),
        255,
        "缺的那一段没留白"
    );
    // 写出去那一段的比例与报告说的对得上：报告不是另算一份数。
    let ink = (0..written.size.height)
        .filter(|&y| written.pixel(0, y) != 255)
        .count() as f64
        / f64::from(written.size.height);
    assert!(
        (ink - salvage.share()).abs() < 0.05,
        "报告说救回 {}，写出来的却是 {ink}",
        salvage.share()
    );
}

/// 一个像素都救不回来的页是**失败页**（04 号票）：它进隔离目录、进失败清单，
/// 退出码跟着变。
///
/// 这是本票的缺陷本身：尺寸解得出来买不到任何像素，而按 12 号票的界线它是一张正常页——
/// 输出是一整张纸白，带着正常的判定元数据，卷还留在干净的去处，退出码 0。
#[test]
fn a_page_that_gives_back_no_pixels_at_all_is_a_failed_page() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    // 尺寸与那张好页不同：占位页要是按自己的几何出，这一条当场看得出来。
    volume.file(
        "002.png",
        &fixtures::salvages_nothing_page(fixtures::SMALLER_THAN_TARGET),
    );

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    let page = &reported.pages[1];
    assert!(
        page.salvage().is_none(),
        "一个像素都没救回来的页被当成了部分救回页"
    );
    let reason = page.failure().expect("它该是一张失败页");
    assert!(reason.contains("002.png"), "原因里没指名是哪一页：{reason}");
    assert!(
        reason.contains("一个像素都没解出来"),
        "原因没说清是救回那一步空手而归：{reason}"
    );

    // 三条后果一条不少：进隔离目录、进失败清单、退出码那一侧看得见。
    assert!(reported.isolated(), "含失败页的卷没进隔离目录");
    assert_eq!(reported.output, space.out().join(ISOLATED).join("volume-a"));
    assert_eq!(reported.failures().count(), 1);
    assert!(
        report.any_isolated(),
        "退出码读的就是它：`exit_code` 靠它把「有卷被隔离」与「全部成功」分开"
    );

    // 它按卷内统一尺寸留白占位，不按自己那个几何——失败页没有自己的几何可用。
    assert_eq!(page.size, reported.pages[0].size);
    let written = fixtures::read_png(&page.output);
    assert_eq!(written.size, reported.pages[0].size);
    assert!(
        written.pixels.iter().all(|&pixel| pixel == 255),
        "占位页不是纸白"
    );
}

/// 归档卷同样进隔离目录，形态仍是归档，成员按阅读顺序一个不少。
#[test]
fn an_archive_volume_with_a_failed_page_is_isolated_as_an_archive() {
    let space = Workspace::new();
    let mut cbz = space.cbz("volume-a");
    let page = fixtures::gradient(fixtures::TYPICAL);
    cbz.page("001.png", &page)
        // 归档结构完好，坏的是这一个成员的字节：读得到它，读不出它。
        .rotten_page("002.png", &page)
        .page("003.png", &page)
        .file("ComicInfo.xml", COMIC_INFO);
    let path = cbz.write();

    let report = run_paths(&space, [path.as_path()]);

    let reported = &report.volumes[0];
    assert!(reported.isolated());
    assert_eq!(
        reported.output,
        space.out().join(ISOLATED).join("volume-a.cbz")
    );
    assert!(reported.output.is_file());
    // 进隔离目录的是**整卷**：三张页一张不少，透传文件也跟着走，而且逐字节一致。
    let members = fixtures::read_cbz(&reported.output);
    let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["001.png", "002.png", "003.png", "ComicInfo.xml"]);
    let carried = &members
        .iter()
        .find(|(name, _)| name == "ComicInfo.xml")
        .expect("透传文件应当在隔离目录里的那一卷内")
        .1;
    assert_eq!(carried.as_slice(), COMIC_INFO, "透传的内容必须逐字节一致");
}

/// 卷的去处会跳，而上一趟写在另一处的那一份不会被覆盖也不会被删——报告要指出来
/// （12 号票的「过期副本」）。
///
/// 这条盯的是隔离这套机制自己造出来的坑：整卷白页的占位输出留在隔离目录里，
/// 摆在文件管理器里与一本正经的书没有分别，藏起来就等于白做。
#[test]
fn the_copy_left_in_the_other_place_is_named_in_the_report() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    let good = fixtures::gradient(fixtures::TYPICAL);
    volume.page("001.png", &good);
    let broken = volume.file("002.png", b"not a png at all");

    // 头一趟有坏页：整卷进隔离目录，干净那一处还空着。
    let first = run_volume(&space, &volume);
    assert!(first.volumes[0].isolated());
    assert_eq!(first.volumes[0].superseded, None, "头一趟没有上一份可指");

    // 修好坏页再跑：这一趟干净，写回 out/volume-a，而隔离目录里那一份原地留着。
    std::fs::write(&broken, fixtures::encode_image(&good, "png")).expect("修好坏页");
    let second = run_volume(&space, &volume);

    let redone = &second.volumes[0];
    assert!(!redone.isolated());
    assert_eq!(redone.output, space.out().join("volume-a"));
    assert_eq!(
        redone.superseded,
        Some(space.out().join(ISOLATED).join("volume-a")),
        "隔离目录里那一整卷白页没被指出来"
    );
    // 没被删：tonefit 不替用户扔东西，只负责让他知道有这么一份。
    assert!(space.out().join(ISOLATED).join("volume-a").is_dir());
}

/// 一页好页都没有的卷仍然出得来：卷内统一尺寸退到面板分辨率。
#[test]
fn a_volume_whose_every_page_fails_still_comes_out_whole() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.file("001.png", b"not a png at all");
    volume.file("002.png", b"");

    let report = run_volume(&space, &volume);

    let reported = &report.volumes[0];
    assert!(reported.isolated());
    assert_eq!(reported.failures().count(), 2);
    for page in &reported.pages {
        // 没有一页好页可参照，统一尺寸只能是这块面板本身。
        assert_eq!(page.size, PANEL);
        assert_eq!(fixtures::read_png(&page.output).size, PANEL);
    }
}

/// 隔离的卷每一趟都重做，不被幂等跳过：它不是一份做完了的输出，
/// 而失败清单要能重新给得出来（11 号票的跳过只认干净的那个去处）。
#[test]
fn an_isolated_volume_is_redone_on_every_run() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");

    let first = run_volume(&space, &volume);
    let second = run_volume(&space, &volume);

    assert!(first.volumes[0].isolated());
    let redone = &second.volumes[0];
    assert!(!redone.skipped(), "隔离的卷被跳过了，失败清单跟着没了");
    assert!(redone.isolated());
    assert_eq!(redone.failures().count(), 1);
    assert_eq!(redone.decodes, 2, "重做那一趟没有真的重做");
}

/// dry-run 预告的是照做时会发生的事：隔离的去处照说，一个文件都不落盘。
#[test]
fn a_dry_run_names_the_isolation_directory_without_writing_anything() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");

    let report = tonefit::run(&tonefit::Request {
        mode: Mode::DryRun,
        ..fixtures::request(&space, [volume.path()])
    })
    .expect("处理应当成功");

    let reported = &report.volumes[0];
    assert!(reported.isolated());
    assert_eq!(reported.output, space.out().join(ISOLATED).join("volume-a"));
    assert_eq!(reported.failures().count(), 1);
    assert!(!space.out().exists(), "dry-run 落了盘");
}

/// 三条路径混在一卷里也不串位：彩色分支、灰度路径、失败页。
///
/// 彩页在彩色 profile 下不进灰度缓存（ADR 0005 决定第 4 条），失败页也不进——页序与缓存序
/// 因此在**两个**地方脱钩。缓存序号跟着页走而不是第二遍数出来，钉的正是这件事：
/// 数错一格，就会静默地把另一页的像素写到这一页的位置上。
///
/// **四张好页两两分得开**（页几何批 09 号票）：两张彩页各用一个宽，两张灰度页同宽而
/// 黑带高度不同——任意两格对调都当场红。从前五页同尺寸，断言只有三句：五页都写出来、
/// 缓存里两页、以及 003 与 005 这两张**同一夹具**逐字节相同；「写出时每页都写第一页的
/// 字节」那个变异套上去照样绿——名字承诺的三条路径里，真被比过像素的一条都没有。
///
/// 高一律取面板高（`PANEL`）：这样的页两种适配方式下都原样输出（页几何批 01 号票）。
/// 裁边在这两类页上也都是空操作——灰度那一张四边顶着墨（见 `fixtures::black_top_band`），
/// 彩页最下面那条色带是纯黑，每一列都有墨。写出的像素因此与源逐个相等，等号写得起。
/// 灰度页只有纯黑与纯白两个取值，在任何一档位深上都是格点，量化与抖动对它们都是恒等；
/// 彩页不量化（ADR 0005 决定第 4 条），按色带取样。
///
/// 失败页那一格按**卷内众数**尺寸留白占位（见 `tonefit` 的 `uniform_size`）：
/// 两张灰度页共用一个宽，两张彩页各用一个，众数因此是灰度页那个尺寸，写得出字面值。
#[test]
fn a_color_page_a_gray_page_and_a_failed_page_keep_their_own_pixels() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    // 灰度页那一对：同宽（好让它成为卷内众数），黑带高度不同。
    let gray_size = Size::new(200, PANEL.height);
    let grays = [(2, "003.png", 8u32), (4, "005.png", 151u32)];
    // 彩页那一对：宽各不相同，与灰度页也不同——两条分支之间串位一样要红。
    let colors = [
        (0, "001.png", Size::new(120, PANEL.height)),
        (3, "004.png", Size::new(180, PANEL.height)),
    ];

    // 写下去的与读回来比的是同一批名字与尺寸：两处各写一份就会各自漂。
    for (_, name, size) in colors {
        volume.page(name, &fixtures::color_page(size));
    }
    for (_, name, black_rows) in grays {
        volume.page(name, &fixtures::black_top_band(gray_size, black_rows));
    }
    volume.file("002.png", b"not a png at all");

    let report = fixtures::run_volume_with(&space, &volume, fixtures::profile("kobo-libra-colour"));

    let reported = &report.volumes[0];
    assert!(reported.isolated());
    assert_eq!(reported.page_count(), 5);
    // 只有两张灰度页进得了缓存：彩页与失败页都不进。
    assert_eq!(reported.cache.pages, 2, "缓存里的页数不对");
    for page in &reported.pages {
        assert!(page.output.is_file(), "{} 没写出来", page.output.display());
    }

    // 灰度路径那两格：尺寸是它自己的，像素与源逐个相等。
    for (index, name, black_rows) in grays {
        let written = fixtures::read_png(&reported.pages[index].output);
        assert_eq!(written.size, gray_size, "{name} 的尺寸不是它自己的");
        assert_eq!(
            written.pixels,
            fixtures::luma_pixels(&fixtures::black_top_band(gray_size, black_rows)),
            "{name} 装的不是它自己的像素"
        );
    }

    // 彩色分支那两格：尺寸各是自己的，色带一条不少。
    for (index, name, size) in colors {
        let written = fixtures::read_color_png(&reported.pages[index].output);
        assert_eq!(written.size, size, "{name} 的尺寸不是它自己的");
        for (band, expected) in fixtures::COLOR_BANDS.iter().enumerate() {
            assert_eq!(
                written.pixel(size.width / 2, fixtures::band_center_row(size, band)),
                *expected,
                "{name} 的第 {band} 条色带"
            );
        }
    }

    // 失败页那一格：卷内众数尺寸的一整张纸白——它顶住位置，但不冒充内容，
    // 也没有拿到别人的像素。
    let placeholder = fixtures::read_png(&reported.pages[1].output);
    assert_eq!(placeholder.size, gray_size, "占位页不是卷内众数尺寸");
    assert!(
        placeholder.pixels.iter().all(|&pixel| pixel == 255),
        "占位页装的不是纸白"
    );
}

/// 源库只读：坏页与好页一样，源那一侧一个字节都不动（spec 的 story 10）。
#[test]
fn isolating_a_volume_leaves_the_source_untouched() {
    let space = Workspace::new();
    let volume = space.volume("volume-a");
    volume.page("001.png", &fixtures::gradient(fixtures::TYPICAL));
    volume.file("002.png", b"not a png at all");
    let before = fixtures::fingerprint(volume.path());

    run_volume(&space, &volume);

    assert_eq!(fixtures::fingerprint(volume.path()), before);
}
