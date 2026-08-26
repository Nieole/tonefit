//! 退出码，在**真进程**上测。
//!
//! 这一条只有在进程那一层才成立：`exit_code` 那个纯函数说得出该返回几，说不出 `main`
//! 有没有把它交出去。spec 的 story 33 要的是「测试不必启动子进程」，不是「一律不许」——
//! 退出码本身就是进程那一层的事实，别处观察不到。整份用例只此一条。

mod fixtures;

use std::path::Path;
use std::process::{Command, Stdio};

use fixtures::Workspace;

/// 三种结局各有各的退出码：做完了、做完了但有卷被隔离、没做成（12 号票）。
#[test]
fn the_exit_code_tells_a_clean_run_an_isolated_one_and_a_refusal_apart() {
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

    assert_eq!(tonefit(&space, clean.path()), Some(0), "干净的一趟不是 0");
    assert_eq!(
        tonefit(&space, isolated.path()),
        Some(2),
        "有卷被隔离的一趟没和干净的那一趟分开"
    );
    assert_eq!(
        tonefit(&space, salvages_nothing.path()),
        Some(2),
        "一个像素都没救回来的页没让退出码反映失败"
    );
    // 拒绝执行是第三个数，不能和上面两个混在一起：那一趟根本没做成。
    // 点一个不存在的卷——它落在源那一侧，不会先撞上「输出与源卷相互嵌套」那道拒绝。
    assert_eq!(
        tonefit(&space, &clean.path().join("根本不存在的卷")),
        Some(1),
        "拒绝执行的一趟不是 1"
    );
}

/// 跑一趟 tonefit，返回它的退出码。进程被信号打断时是 `None`。
fn tonefit(space: &Workspace, input: &Path) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", fixtures::BASELINE_DEVICE])
        .arg(input)
        // 报告与错误都不进测试日志：这条用例只看退出码，那两样别处已经测过了。
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("启动 tonefit")
        .code()
}
