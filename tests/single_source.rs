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

/// 「哪几种算拒绝开始」单子上那几项的字样，肯定式与否定式都算：耗时那一格与
/// 命令行的清点转轮从前抄的是「输出不在源里」这一副说法，那同样是一份抄件。
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
    "清点发现点名的路径点不开",
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

/// **闩的编码**那几格的字样，**读回来那一侧**（`p4-parking-lot/19`，收停车场 Q70）。
///
/// 记号挑在 `from_code` 那一侧，不挑 `code` 那一侧，理由是**编进去那一侧当不了记号**：
/// `Continue => 0` 这种写法随手就有第二个用处（按一级的**名次**说「推到这一级要按几次」，
/// 名次恰好就是编码那个数）——那是**另一件事**，不是闩的编码抄了一份；拿编进去那一侧当记号，
/// 会把它也判成抄件。**读回来那一侧没有第二个用处**：把一个字节还原成一级，只有编码本身要做。
///
/// 两种写法各列一条：家里那一份写 `Self::`（它就在 `Instruction` 的 `impl` 里），
/// 别的 crate 手抄一份只能写 `Instruction::`——两条都在，抄件写哪一种都躲不过去。
/// 挑的那两格一格是**头一格**、一格是**兜底那一格**（不认得的数按最强的算），
/// 抄件少抄哪一格都还剩另一格。
const LATCH_DECODE_MARKS: [&str; 4] = [
    "0=>Self::Continue",
    "0=>Instruction::Continue",
    "_=>Self::Abort",
    "_=>Instruction::Abort",
];

/// 编进去那一侧的三格：**只问家里在不在**，不问别处有没有（理由见 [`LATCH_DECODE_MARKS`]）。
const LATCH_CODE_MARKS: [&str; 3] = ["Continue=>0", "Finish=>1", "Abort=>2"];

/// 那份编码的家，相对仓库根。
const LATCH_CODE_HOME: &str = "src/progress.rs";

/// 另外两份闩留的那句路标——引用的是**那个公共方法的名字**而不是行号，
/// `progress` 怎么改都不使引用失效（`CLAUDE.md`《文档写作》第 5 条：稳定引用）。
const LATCH_CODE_SIGNPOST: &str = "Instruction::code";

/// **「列前几条，剩下的说个数」收口那一句**的字样（`p4-parking-lot/27`，
/// 收停车场 Q45／Q49／Q115／Q204）。
///
/// 记号挑的是**「另有」后面紧跟着一个占位符**，不是「另有」那两个字：那两个字在本仓库的
/// 散文里到处都是（「另有一处」「另有人」「另有去处」），拿它当记号会把真话全判成抄。
/// 而**把这一句现拼出来**只有一处要做——收口那一句报的是一个当场算出来的数，
/// 因此必然写成 `format!("另有 {..} ..")`。
///
/// 六处从前各写各的：报告末尾那三小结、清点那条拒绝、撞名那条拒绝，加上点名头几页那一句。
/// 现在形状只在 [`TRUNCATION_HOME`]，六处各自出的仍是自己那句抬头与自己那个量词。
const TRUNCATION_MARKS: [&str; 1] = ["另有 {"];

/// 那个形状的家，相对仓库根。
const TRUNCATION_HOME: &str = "src/listing.rs";

/// 家里真住着的那两格：**列几条**那个上限，与**剩下的怎么说**那一句。
///
/// 少问哪一格，把那一格删掉这一条都还是绿的——上限没了六处各挑各的数，
/// 收口那一句没了六处各说各的话，而两样都躲得过只问「别处有没有」的那一问。
///
/// **上限那一格只问名字，不问值**：把 `= 5` 一并抄进来，这个文件就成了仓库里第二处
/// 写着那个数的地方，而改值该改的是那一个常量，不是这一条用例。
const TRUNCATION_HOME_MARKS: [&str; 2] = ["pub const LIST_LIMIT: usize", "另有 {"];

/// 用着那个形状的三个文件，跟着的是**它在这个文件里至少出现几次**。
///
/// 问的是**下界**，而下界取的是**实数**：多一处用它不该变红，少一处必须变红。
/// `src/render.rs` 那九次是四处调用点（三小结加点名头几页那一句）连同 `use`、四句指路；
/// `src/survey.rs` 三次（`use`、一句指路、一处调用点）；
/// `src/lib.rs` **四**次（模块文档那句 `[FirstFew]`、`pub use`、一句指路、一处调用点）
/// ——少数了模块文档那一次的话，把撞名那一处拆回去自己数仍剩三次，这一问就漏得过去。
///
/// **这一条与[头一问](TRUNCATION_MARKS)问的不是同一件事**：那一问拦的是「又抄了一份」，
/// 这一问拦的是「某一处把它拆回去自己数」——拆回去的那一处若连收口那一句都一并省了
/// （只列前几条、剩下的一个字不说），头一问一声不吭。
const TRUNCATION_USED: [(&str, usize); 3] = [
    ("src/render.rs", 9),
    ("src/survey.rs", 3),
    ("src/lib.rs", 4),
];

/// 另外两份闩各在哪个文件里，跟着的是**那句路标在这个文件里至少出现几次**。
///
/// 问的是**下界**：多一句指路不该变红，少一句必须变红。两处各两句——闩自己的文档一句、
/// 钉着它的那条用例一句，两句说的是两件事（这一份不自己编 / 这一份存的就是它给的字节）。
const LATCH_CODE_SIGNPOSTED: [(&str, usize); 2] = [("src/session/run.rs", 2), ("src/main.rs", 2)];

/// **《落格》那条不变量**的字样（`encoder-gate/01`，收停车场 Q936）。
///
/// 记号挑的是那一条词条里**只有它说得出**的三句：这条不变量本身、它给出的那个上界、
/// 以及它对编码器立下的那道闸。三句各挑一句，抄件少抄哪一句都还剩另外两句。
///
/// **「落格」那两个字当不了记号**：`tests/pipeline.rs` 那道闸、`docs/adr/0009` 那句路标，
/// 说的都是**指着它**，不是又写了一份。拿那两个字当记号会把真话也判成抄。
const ON_GRID_MARKS: [&str; 3] = [
    "把写出去的字节解回来，每一个取值都还落在它那一档判定的格点上",
    "灰调级数因此不超过 `2^灰阶档位`",
    "一个编码器进不了这个项目，除非它保得住这一条",
];

/// 那条不变量的家，相对仓库根。
const ON_GRID_HOME: &str = "CONTEXT.md";

/// 家里真住着的那三句：**哪几种页不在这条不变量里，各自凭什么**。
///
/// `tests/pipeline.rs` 那道闸断的是一条双向对应——没有判定的页必须说得出自己是这三种里的
/// 哪一种——而「是哪三种」这句话只在词条里。把这三句从词条里删掉，那道闸就成了一份
/// 没有依据的名单，**这一条因此要拦住那一手**。
const ON_GRID_HOME_MARKS: [&str; 3] = [
    "彩页不量化",
    "透传文件原样拷",
    "坏页的空白占位页只有一个取值",
];

/// 指回来的那句路标——**term 与它的家一并写着**，因此换了小节标题会变红，
/// 而词条在文件里挪位置不会（`CLAUDE.md`《文档写作》第 5 条：稳定引用）。
const ON_GRID_SIGNPOST: &str = "《落格》，见 `CONTEXT.md` 的《量化》";

/// 指回来的那句路标在哪几个文件里，跟着的是**至少出现几次**（下界取实数，
/// 与 [`TRUNCATION_USED`] 同一条：多一句指路不该变红，少一句必须变红）。
///
/// 两处：ADR 0009 的《不要做的「简化」》把《落格》列成第二个编码器要过的另一道闸；
/// `tests/pipeline.rs` 那道闸自己**只指过去、不复述词条**，四句路标各在一处
/// （枚举、逐页那一问，加两条用例的抬头）。`encoder-gate/04` 处置 ADR 0004 之后
/// 那一篇也会指回来，届时这张表跟着加一行。
///
/// **`tests/` 在这张表里，而不在[被扫的那几处](delivered)**：那一批扫的是
/// 「别处有没有抄第二份」，用例不在其中（这一支自己就攥着记号）；这张表问的是
/// 「指回来的那句话还在不在」，按文件路径问得着任何一个文件。
const ON_GRID_SIGNPOSTED: [(&str, usize); 2] = [
    ("docs/adr/0009-scope-is-a-named-subset.md", 1),
    ("tests/pipeline.rs", 4),
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

/// 加第六种拒绝开始只改一处——这一条是那句话的闸门（P4 01 号票，收停车场 Q91）。
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
        "「哪几种算拒绝开始」那张单子长出了第二份"
    );

    let entry = squashed(&read(&home));
    for mark in ["输出落在源里", "两个卷撞同一个去处", "覆盖项把候选集裁空"]
    {
        assert!(
            entry.contains(&squashed(mark)),
            "《失败》的**拒绝开始**里少了「{mark}」"
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

/// 闩的编码只有一处，抄出第二份就当场变红（P4 19 号票，收停车场 Q70）。
///
/// 从前这三格在**两个 crate 里各有一份**（库、会话），18 号票给命令行接上两级停止之后
/// 成了三份，靠三条用例分别拴着——而那三条各拴各的，谁也发现不了另外两份跟自己
/// 分了家。现在编码只在 [`LATCH_CODE_HOME`] 的 `Instruction::code`／`from_code`，
/// 另外两份闩只剩「存在哪儿、由谁往上推」。
///
/// **三件事一起问**，与[拒绝开始那一条](the_refusal_list_lives_in_one_place)同一个形状：
/// 别处没有第二份（问的是[读回来那一侧](LATCH_DECODE_MARKS)）、家里[编进去那三格](LATCH_CODE_MARKS)
/// 真住着、另外两份闩指回来的路标还在。只问头一件的话，把 `code` 整个删掉这一条也是绿的。
#[test]
fn the_latch_encoding_lives_in_one_place() {
    let home = root().join(LATCH_CODE_HOME);
    let marks: Vec<String> = LATCH_DECODE_MARKS
        .iter()
        .map(|mark| squashed(mark))
        .collect();

    let carrying: Vec<PathBuf> = delivered()
        .into_iter()
        .filter(|path| {
            let text = squashed(&read(path));
            marks.iter().any(|mark| text.contains(mark))
        })
        .collect();
    assert_eq!(carrying, vec![home.clone()], "闩的编码长出了第二份");

    let entry = squashed(&read(&home));
    for mark in LATCH_CODE_MARKS {
        assert!(
            entry.contains(&squashed(mark)),
            "`Instruction::code` 里少了「{mark}」那一格"
        );
    }

    let signpost = squashed(LATCH_CODE_SIGNPOST);
    for (file, least) in LATCH_CODE_SIGNPOSTED {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：另外两份闩按文件路径记在 LATCH_CODE_SIGNPOSTED 上，\
             模块挪了位置就把那张表跟着改"
        );
        let found = squashed(&read(&path)).matches(&signpost).count();
        assert!(
            found >= least,
            "{file} 里指回那一份公共编码的路标从 {least} 句掉到了 {found} 句"
        );
    }
}

/// 「列前几条，剩下的说个数」只有一处出处，抄出第二份就当场变红
/// （P4 27 号票，收停车场 Q45／Q49／Q115／Q204）。
///
/// 这个形状从前在仓库里有**六处**，各写各的——停车场为它记了三次（三处 → 四处 → 五处），
/// 每一次都只是在数。六处各有一个自己的上限、各拼一遍收口那一句，
/// 而它们谁也发现不了另外五处跟自己分了家。
///
/// **三件事一起问**，与[拒绝开始那一条](the_refusal_list_lives_in_one_place)、
/// [闩的编码那一条](the_latch_encoding_lives_in_one_place)同一个形状：
/// 别处没有第二份、[家里那两格](TRUNCATION_HOME_MARKS)真住着、
/// [用着它的那三个文件](TRUNCATION_USED)还在用。
#[test]
fn the_way_to_truncate_a_list_lives_in_one_place() {
    let home = root().join(TRUNCATION_HOME);
    let marks: Vec<String> = TRUNCATION_MARKS.iter().map(|mark| squashed(mark)).collect();

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
        "「列前几条，剩下的说个数」长出了第二份"
    );

    let entry = squashed(&read(&home));
    for mark in TRUNCATION_HOME_MARKS {
        assert!(
            entry.contains(&squashed(mark)),
            "{TRUNCATION_HOME} 里少了「{mark}」那一格"
        );
    }

    let signpost = squashed("FirstFew");
    for (file, least) in TRUNCATION_USED {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：用着那个形状的几处按文件路径记在 TRUNCATION_USED 上，\
             模块挪了位置就把那张表跟着改"
        );
        let found = squashed(&read(&path)).matches(&signpost).count();
        assert!(
            found >= least,
            "{file} 里用那一处公共件的地方从 {least} 处掉到了 {found} 处"
        );
    }
}

/// 《落格》那条不变量只有一处出处，抄出第二份就当场变红（`encoder-gate/01`，收停车场 Q936）。
///
/// 这一条钉的是**那道闸的依据**：`tests/pipeline.rs` 那几条问的是「写出去的页落不落格」，
/// 而**落格是什么**它们一个字都不说、只指过去。说法一旦长出第二份，改了其中一处的人
/// 不会知道另一处也该改——下一个想接编码器的人于是各按各的说法过关，
/// 而这条不变量存在的全部意义就是拦住那一手。
///
/// **三件事一起问**，与前四条同一个形状：别处没有第二份、
/// [家里那三句例外](ON_GRID_HOME_MARKS)真住着、[指回来的那句路标](ON_GRID_SIGNPOST)还在。
/// 只问头一件的话，把词条整个删掉这一条也是绿的。
#[test]
fn the_on_grid_invariant_lives_in_one_place() {
    let home = root().join(ON_GRID_HOME);
    let marks: Vec<String> = ON_GRID_MARKS.iter().map(|mark| squashed(mark)).collect();

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
        "《落格》那条不变量长出了第二份"
    );

    let entry = squashed(&read(&home));
    for mark in ON_GRID_HOME_MARKS {
        assert!(
            entry.contains(&squashed(mark)),
            "《落格》里少了「{mark}」那一句——不在这条不变量里的页从此说不出自己凭什么不在"
        );
    }

    let signpost = squashed(ON_GRID_SIGNPOST);
    for (file, least) in ON_GRID_SIGNPOSTED {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：指回《落格》的那几处按文件路径记在 ON_GRID_SIGNPOSTED 上，\
             文件挪了位置就把那张表跟着改"
        );
        let found = squashed(&read(&path)).matches(&signpost).count();
        assert!(
            found >= least,
            "{file} 里指回《落格》的路标从 {least} 句掉到了 {found} 句"
        );
    }
}
