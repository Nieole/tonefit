//! 说过一次的话，仓库里只有一处说得算。
//!
//! 这一批断言问的都是同一个形状：某件事**只有一份出处**，抄出第二份就当场变红。
//! 它们扫的是**文件本身**，不是某个运行时的取值——文档没有常量拴得住
//! （`CLAUDE.md`《文档写作》第 4 条：单一出处）。
//!
//! 摆在这里而不是各自的模块里，正是因为它们要扫的东西横跨库、二进制与交付出去的文档：
//! 摆进任何一处都得在扫描里给自己挖一个洞——这一支自己就攥着那几个记号。

use std::fs;
use std::path::{Path, PathBuf};

/// 「哪几种算拒绝执行」单子上那几项的字样，肯定式与否定式都算：耗时那一格与
/// 命令行的预扫转轮从前抄的是「输出不在源里」这一副说法，那同样是一份抄件。
///
/// 挑的是**只在这张单子上出现**的那几项，别的两项都当不了记号：
/// 「处理范围为空」另有运行时的出处（`run` 那句 `bail!`）；
/// 「覆盖项把候选集裁空」在库内的 `Refusal` 上被单点了名，而那是一句本地的事实
/// （眼下只有那一种戴这个标记），是**引用单子上的一项**，不是把单子抄了一遍
/// （停车场 Q174）。拿它们当记号会把真话也判成抄。
///
/// 记号与被扫的文字**两头都归一**（见 [`squashed`]）：折行与行内加粗因此躲不过去——
/// 本仓库的中文 doc comment 是手工折行的，`两个卷撞同一` 接着 `个去处`、
/// 以及 `**输出**落在源里`，都得算命中。
const REFUSAL_MARKS: [&str; 5] = [
    "输出落在源里",
    "输出不在源里",
    "两个卷撞同一个去处",
    "两个卷不撞同一个去处",
    "预扫发现点名的路径点不开",
];

/// 那张单子的家，相对仓库根。
const REFUSAL_HOME: &str = "CONTEXT.md";

/// 从前抄着单子的那五处，如今留的是这句路标——引用的是**小节标题**而不是行号，
/// 那五个文件怎么改都不使引用失效（`CLAUDE.md`《文档写作》第 5 条：稳定引用）。
const REFUSAL_SIGNPOST: &str = "`CONTEXT.md` 的《失败》";

/// 那五处摊在这四个文件里，跟着的是**那句路标在这个文件里至少出现几次**。
///
/// 数不全是本票新添的：`progress.rs` 那两次（`RunStarted` 与 `RunFinished`）都是，
/// 另外三个文件本来就各有一两句指着《失败》的路标（`Refusal` 的抬头、
/// `RunOutcome::Refused`、退出码那几条），本票各往上加一句。
/// **问的是下界**：多一句指路不该变红，少一句必须变红——只问「有没有」的话，
/// 那三个文件上这一条就是空转的，把新添的路标全删掉它照样绿。
const REFUSAL_SIGNPOSTED: [(&str, usize); 4] = [
    ("src/lib.rs", 3),
    ("src/progress.rs", 2),
    ("src/report.rs", 3),
    ("src/main.rs", 3),
];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|why| panic!("读 {} 失败：{why}", path.display()))
}

/// 归一：把空白、`*`、`/` 与反引号全去掉。
///
/// 手工折行的 doc comment、行内加粗、把一个词打散在两行——归一之后都还原成同一串，
/// 记号因此躲不开。两头都过这一道，比较的才是同一副形状。
fn squashed(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '*' | '/' | '`'))
        .collect()
}

/// 该扫的那几处：仓库根上那几份交付文档（词条自己也在里面）、全部实现、`docs/`。
///
/// `tests/` 不在里面——这一支自己就攥着那几个记号；`.scratch/` 也不在——
/// 停车场与票据照抄词条原文是它们的本分，那是记录，不是第二个出处。
fn delivered() -> Vec<PathBuf> {
    let mut files = Vec::new();
    // 根这一层只列直接的 `.md`：`README.md` 的退出码表离「顺手把五种展开一遍」最近，
    // 而它从前落在扫描面外。
    collect(root(), "md", false, &mut files);
    collect(&root().join("src"), "rs", true, &mut files);
    collect(&root().join("docs"), "md", true, &mut files);
    files
}

fn collect(dir: &Path, extension: &str, descend: bool, into: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|why| panic!("读 {} 失败：{why}", dir.display()));
    // 目录序不定，排一遍：断言里那张清单才不随文件系统变。
    let mut here: Vec<PathBuf> = entries.map(|entry| entry.expect("目录项").path()).collect();
    here.sort();
    for path in here {
        if path.is_dir() {
            if descend {
                collect(&path, extension, descend, into);
            }
        } else if path.extension().is_some_and(|ext| ext == extension) {
            into.push(path);
        }
    }
}

/// 加第六种拒绝执行只改一处——这一条是那句话的闸门（P4 01 号票，收停车场 Q91）。
///
/// 从前那张单子在实现文档里抄了五份，加一种要五处一起改（Shotgun Surgery）。
/// 现在单子只在 `CONTEXT.md` 的《失败》，五处只留指路。
///
/// **三件事一起问**，少一件这一条就问不出话来：别处没有第二份、家里真住着那张单子、
/// 五处的路标还在。只问头一件的话，把词条整个删掉这一条也是绿的。
#[test]
fn the_refusal_list_lives_in_one_place() {
    let home = root().join(REFUSAL_HOME);
    let marks: Vec<String> = REFUSAL_MARKS.iter().map(|mark| squashed(mark)).collect();

    let carrying: Vec<PathBuf> = delivered()
        .into_iter()
        .filter(|path| {
            let text = squashed(&read(path));
            marks.iter().any(|mark| text.contains(mark))
        })
        .collect();
    assert_eq!(
        carrying,
        vec![home.clone()],
        "「哪几种算拒绝执行」那张单子长出了第二份"
    );

    let entry = squashed(&read(&home));
    for mark in ["输出落在源里", "两个卷撞同一个去处", "覆盖项把候选集裁空"]
    {
        assert!(
            entry.contains(&squashed(mark)),
            "《失败》的**拒绝执行**里少了「{mark}」"
        );
    }

    let signpost = squashed(REFUSAL_SIGNPOST);
    for (file, least) in REFUSAL_SIGNPOSTED {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：那五处的路标按文件路径记在 REFUSAL_SIGNPOSTED 上，\
             模块挪了位置就把那张表跟着改"
        );
        let found = squashed(&read(&path)).matches(&signpost).count();
        assert!(
            found >= least,
            "{file} 里指回《失败》的路标从 {least} 句掉到了 {found} 句"
        );
    }
}
