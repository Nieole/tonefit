//! 退出码，在**真进程**上测。
//!
//! 这一条只有在进程那一层才成立：`exit_code` 那个纯函数说得出该返回几，说不出 `main`
//! 有没有把它交出去。spec 的 story 33 要的是「测试不必启动子进程」，不是「一律不许」——
//! 退出码本身就是进程那一层的事实，别处观察不到。整份用例只此一条。

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

/// 跑一趟 tonefit，返回它的退出码。进程被信号打断时是 `None`。
fn tonefit(space: &Workspace, inputs: &[&Path]) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        .args(inputs)
        // 报告与错误都不进测试日志：这条用例只看退出码，那两样别处已经测过了。
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("启动 tonefit")
        .code()
}
