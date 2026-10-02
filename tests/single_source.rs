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
/// 本仓库的中文 doc comment 是手工折行的，`两样东西撞同一` 接着 `个去处`、
/// 以及 `**输出**落在源里`，都得算命中。
const REFUSAL_MARKS: [&str; 5] = [
    "输出落在源里",
    "输出不在源里",
    "两样东西撞同一个去处",
    "两样东西不撞同一个去处",
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

/// **按停止的两条规矩**的字样（`say-and-stop/01`）：升级那张表（继续 → 做完再停 → 立即停止
/// → 立即停止），与确认点上做完再停要让、立即停止不让。
///
/// 记号挑的是**那两个 `match` 的臂**：把规矩写下来只能写成这几支，而散文里说这件事用的是
/// 「做完再停」「立即停止」那几个词，一个都不带 `=>`。确认点那一条只挑**守卫的头**
/// （`Finish if`），不连着参数名——抄件换一个参数名就躲得过整句。
///
/// 两种写法各列一条：家里写的是 `Instruction::`，而这两条规矩最顺手的第二个去处是库里的
/// `impl Instruction`——在那儿写就是 `Self::`。那一处恰是它们不该去的地方（让不让是调用方的策略，
/// ADR 0012 决定第 3 条），抄件写哪一种都躲不过去。
const STOP_RULE_MARKS: [&str; 6] = [
    "Instruction::Continue => Instruction::Finish",
    "Self::Continue => Self::Finish",
    "Instruction::Finish | Instruction::Abort => Instruction::Abort",
    "Self::Finish | Self::Abort => Self::Abort",
    "Instruction::Finish if",
    "Self::Finish if",
];

/// 那两条规矩的家，相对仓库根。它在 bin 里而不在库里：让不让是调用方的策略
/// （ADR 0012 决定第 3 条）。
const STOP_RULE_HOME: &str = "src/stop.rs";

/// 调它的那两路，各在哪个文件里调哪几个——**命令行与会话都调的是它**。
///
/// 只问「别处没有第二份」的话，一路把规矩整个删掉（观察者把闩原样交给库、按一下直接跳到
/// 立即停止）这一条照绿；这张表拦的是那一手。问的是**调用的形状**（`stop::` 带着左括号），
/// 不是模块名——指路的散文不算调用。那一路调了之后真的照办没有，由两路各自的用例在一趟
/// 真跑上问（`src/main.rs` 与 `src/session/run.rs` 各一条：卷跑到一半按一次做完再停，
/// 那一卷仍旧整卷落盘）。
const STOP_RULE_CALLERS: [(&str, &[&str]); 3] = [
    (
        "src/main.rs",
        &[
            "stop::next(",
            "stop::at_the_decision_point(",
            "stop::answer(",
        ],
    ),
    (
        "src/session/run.rs",
        &["stop::at_the_decision_point(", "stop::answer("],
    ),
    ("src/session/state.rs", &["stop::next("]),
];

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

/// **型号认不出来时那一句拒绝**的字样（`proof-sheet/06`，spec 的 story 26）。
///
/// 三截各挑一截：抬头（带着那个占位符——**把这一句现拼出来**才写得出它，与
/// [`TRUNCATION_MARKS`] 同一条道理）、分组那一截、兜底办法那一截。抄件少抄哪一截都还剩另外两截。
///
/// **「未知型号」那四个字当不了记号**：`preset.rs`、会话那几处、`CONTEXT.md` 的《下钻》说的都是
/// 「与未知型号那条错误同一份」——那是**指着它**，不是又写了一份。
const DEVICE_REFUSAL_MARKS: [&str; 3] = [
    "未知型号「{",
    "内置型号按面板分组",
    "设备不在表里：挑一个面板相同的型号",
];

/// 那一句的家，相对仓库根。
const DEVICE_REFUSAL_HOME: &str = "src/profile.rs";

/// **翻大小写问盘那个探法**的字样（`one-source/03`，收停车场 Q241、Q300、Q366）。
///
/// 记号挑的是探法里**只有它写得出**的那两件：把一个名字整个翻到另一个大小写，
/// 与「一个字翻出来仍是一个字」那一问。补全从前自己攥着一份，撞名那一道另按编译平台折——
/// 「是不是同一处」三处三套，这一条拦的是第二份长回来。
///
/// 头一件连着参数一起挑：会话里另有一个 `flipped(self)`（`a` 键来回翻两档清单），
/// 那是另一件事，只挑函数名会把它也判成抄件。
const CASE_PROBE_MARKS: [&str; 2] = ["fn flipped(name: &str)", "fn one_other_case("];

/// 那个探法的家，相对仓库根：库里「同一处」那一把尺子（`CONTEXT.md` 的《同一处》）。
const CASE_PROBE_HOME: &str = "src/place.rs";

/// 调它的那几处，各在哪个文件里怎么调。问的是**调用的形状**（带着左括号），指路的散文不算。
///
/// 补全调探法本身、自己记一格；一趟开工时每条处理路径（源那一侧）与输出根（输出那一侧）
/// 各探一次，按那一侧取探不出时的那一边。
const CASE_PROBE_CALLERS: [(&str, &[&str]); 3] = [
    ("src/session/complete.rs", &["tonefit::case_sensitivity("]),
    ("src/survey.rs", &["Side::Source.probe("]),
    ("src/lib.rs", &["Side::Output.probe("]),
];

/// **屏上一句话里顺口提到的一个键**手抄时的字面（`design-parity/03`，收停车场 Q874、Q963）。
///
/// 一句一条，都是新界面屏上真出现过的字。记号取的是**键挨着措辞**的那一截——手抄成一个字面的
/// 那一句必然带着它，而从按键表取键的那一句在代码里写的是 `{key}`。
///
/// **记号写的是今天的键位**：换了键位再手抄一份，这几条记号就认不出它了。这一条守的是
/// 「这几句不许抄回去」，「换键位屏上一起变」由那几屏的设计快照守。
///
/// **键与措辞分两段写的那几处记号认不出**（确认条行首那四个键、「按 F 恢复」那一句回话、
/// 详情栏的 `[i → 修改]`）：分两段手抄的只是一个 `"x"`，代码里挨不着措辞。
/// 那几处由[读着按键表的那几个文件](KEY_READERS)上的下界拦——每处各调一次，退回一处手抄就少一次；
/// 确认条四个键共用一个闭包，那里数的是闭包的四次调用。
///
/// **「先按 s 停止，或按 C-c」不整截当记号**：按键表上 `q` 拒绝退出那一行长的那一句就是它
/// （表自己的话，在家里），回话那一句因此拆成两截各挑带着自己那几个字的一截。
const KEY_SENTENCE_MARKS: [&str; 20] = [
    "a → 全部页",
    "a → 只看需留意的页",
    "h → 回卷列表",
    "n N 跳到下一个",
    "F 恢复",
    "再按一次 dd 删除",
    "再按一次 ⏎ 覆盖",
    "p → 返回",
    "h → 返回",
    "h → 回到屏幕规格",
    "按 c 生成灰阶测试图",
    "Esc → 关闭",
    "x 写出这一卷",
    "a 写出，后面",
    "s 不写出",
    "v 查看每页结果",
    "q 不会退出",
    "请先按 s 停止",
    "或按 C-c 立即退出",
    "再按一次 s 立即停止",
];

/// 那几句问键的家：一件事的键怎么写，**不问阶段与块**（[`KEY_HOME_MARK`]）。
const KEY_HOME: &str = "src/session/keymap.rs";

/// 家里真住着的那一手：**只问名字，不问实现**（也不问它敞到哪一层）。
const KEY_HOME_MARK: &str = "fn spelt_for(";

/// 读着那一手的几个文件，各跟着它问的那一手与**代码里至少出现几次**（下界取实数，
/// 与 [`TRUNCATION_USED`] 同一条：多一处不该变红，少一处必须变红）。
///
/// 画法那几块之外还有两处：`view.rs`（屏底那几句回话是它说出口的）与 `config.rs`
/// （详情栏那几段长说明是它的字）。`decision.rs` 多问一手：四个键经同一个闭包取，
/// 调 `spelt_for` 只有一次，退回一个键手抄少的是闭包那一次调用。
/// **数的是代码里的**（[`code_only`]）：文档里点名它是指路，不算读。
const KEY_READERS: [(&str, &str, usize); 9] = [
    ("src/session/shell/pages.rs", "spelt_for(", 3),
    ("src/session/shell/list.rs", "spelt_for(", 3),
    ("src/session/shell/decision.rs", "spelt_for(", 1),
    ("src/session/shell/decision.rs", "key(Deed::", 8),
    ("src/session/shell/picker.rs", "spelt_for(", 2),
    ("src/session/shell/details.rs", "spelt_for(", 3),
    ("src/session/shell/overlay.rs", "spelt_for(", 1),
    ("src/session/view.rs", "spelt_for(", 5),
    ("src/session/config.rs", "spelt_for(", 1),
];

/// **转轮那一段算式**的字样（`one-source/06`，收停车场 Q864）：十格字形那一张表、
/// 「过了几格」那一除里的周期、「转满一圈从头来」那一模。
///
/// 三件各挑**算式里只有它写得出**的那一截：周期与字形表在家里是私有的，别处的抄件
/// 得自带一张表，那张表就是头一件；家里另写一段，就得再除一次周期、再模一次表长。
///
/// **问的是「代码里恰好一处、就在家里」**（[`code_only`]）：抄件可以落在别的文件，
/// 也可以落在家里另一个函数里——只问「别的文件没有」拦不住后一种。
const SPINNER_MARKS: [&str; 3] = [
    r#"["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]"#,
    "SPINS_EVERY.as_millis()",
    "% SPINNER.len()",
];

/// 那一段算式的家，相对仓库根：视图那一层摆在 `tui` 特性外面，
/// 顶栏、行首记号、总览上清点那一条都够得着它（`marks.rs` 整个在特性后面，反过来摆不成）。
const SPINNER_HOME: &str = "src/session/view.rs";

/// **会话的时钟起点**那一格，住在[家里](SPINNER_HOME)：只问名字与类型。
const CLOCK_HOME_MARK: &str = "pub clock: Option<Instant>";

/// 时钟起点从前另有的那一格：会话自己身上的「会话打开那一刻」。同一个值存两处，
/// 夹具就得摆两次——Q864 那次合并撞出来的次序问题出在这里。
///
/// **记号写的是从前那一格的名字**：换一个名字再长一格，这一条认不出它。可那一格要让转轮
/// 从它算，就得另写一段算式（家里那一段只读[那一格](CLOCK_HOME_MARK)），那一段由
/// [`SPINNER_MARKS`] 拦。
const CLOCK_COPY_MARK: &str = "opened_at";

/// 读转轮的那几处，各在哪个文件里怎么读：顶栏右端那一截、行首记号（卷列表那棵树）、
/// 总览上清点那一条。问的是**调用的形状**（带着左括号），指路的散文不算。
const SPINNER_READERS: [(&str, &str); 3] = [
    ("src/session/shell/topbar.rs", "views.spinning("),
    ("src/session/shell/list.rs", "views.spinning("),
    ("src/session/shell/overview.rs", "views.spinning("),
];

/// **隔离目录的名字**（`one-source/07`，收停车场 Q870）：盘上那一级目录叫什么。
///
/// **认的是一个独立的词**（[`standalone_count`]）：前面挨着字母、数字或下划线的不算
/// （`any_isolated` 那几个方法名），前面是引号、斜杠、空格的都算——`"_isolated"`、
/// `"out/_isolated"`、`"输出在 _isolated/"` 手抄的就是这几种。不归一：[`squashed`] 会把斜杠去掉，
/// `out/_isolated` 就粘成了一个词。
const ISOLATED_NAME: &str = "_isolated";

/// 那个名字的家：库那一格公开常量。会话住在另一个 crate 里，私有的那一格它够不着。
const ISOLATED_HOME: &str = "src/lib.rs";

/// 家里真住着的那一格：**只问名字与类型**，不问值——值是 [`ISOLATED_NAME`] 那一问的事。
const ISOLATED_HOME_MARK: &str = "pub const ISOLATED_DIRECTORY: &str";

/// 读那一格的那一处：每页结果头一行末尾那个短标签。问的是**读的形状**（带着 crate 名），
/// 指路的散文不算。场景数据那条「去处照库的镜像规则」的用例也读它，但用例不是出处
/// （[`code_only`] 把它砍掉了），不在这里问。
const ISOLATED_READER: (&str, &str) = ("src/session/shell/pages.rs", "tonefit::ISOLATED_DIRECTORY");

/// **一页都没判的卷在灰阶分布那一列上写的那两个词**（`one-source/07`，收停车场 Q961）：
/// 跳过、没做成。认的是**整个字面**（连引号）：「这一卷跳过了：」「 ⋅ 跳过 」那几句是别的话，
/// 不该被这一条判成抄件。
const WHY_NOTHING_MARKS: [&str; 2] = [r#""跳过""#, r#""没做成""#];

/// 那两个词的家：措辞那一层（`why_nothing_judged`）。
const WHY_NOTHING_HOME: &str = "src/render.rs";

/// 卷列表那一格读那两个词走的那一手：**问的是调用的形状**，指路的散文不算。
const WHY_NOTHING_READER: (&str, &str) = ("src/session/shell/list.rs", "render::tally_column(");

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

/// 只留**代码**：砍掉头一个**就地展开的**用例模块（`#[cfg(test)]` 下一行 `mod 名字 {`）起的整段，
/// 再去掉注释行。
///
/// 屏上那几句话在**用例**里出现是记录（期望屏逐字符比的就是它），在 **doc comment**
/// 里出现是引用——两处都不是第二个出处。**在代码里出现才是**：代码里写着 `h → 回卷列表`
/// 就是屏上多了一处手抄的键。本仓库就地展开的用例模块都收在文件末尾（`terminal.rs` 是两个连着：
/// `redesign` 接着 `tests`）。**只声明不展开的那几行不砍**（`session.rs` 中段的
/// `#[cfg(test)] mod scene;`）：它后面还是代码。
///
/// **整个文件都是用例的那几个照扫**（`scene.rs`、`shell/design.rs`、`terminal/measure.rs`）：
/// 它们自己不知道自己只在用例里编，这里也不替它们记一张名单。错的方向是**误红**，不是漏查——
/// 那几个文件里真写了一句记号，红了再看是哪一句。
fn code_only(text: &str) -> String {
    let opener = "#[cfg(test)]\nmod ";
    let cut = text
        .match_indices(opener)
        .map(|(at, _)| at)
        .find(|at| {
            let rest = &text[at + opener.len()..];
            rest.lines()
                .next()
                .is_some_and(|line| line.trim_end().ends_with('{'))
        })
        .unwrap_or(text.len());
    text[..cut]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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
    for mark in ["输出落在源里", "两样东西撞同一个去处", "覆盖项把候选集裁空"]
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

/// 按停止的两条规矩在 bin 里只有一份，命令行与会话都调它（`say-and-stop/01`）。
///
/// 从前两路各抄一份、名字逐字相同，靠两条逐字相同的用例各拴各的——谁也发现不了另一份
/// 跟自己分了家。现在规矩只在 [`STOP_RULE_HOME`]，两路只剩「谁按、记在哪儿、等不等人」。
///
/// **三件事一起问**，与[闩的编码那一条](the_latch_encoding_lives_in_one_place)同一个形状：
/// 别处没有第二份、[家里那几支](STOP_RULE_MARKS)真住着、[两路](STOP_RULE_CALLERS)真调它。
#[test]
fn the_stop_rules_live_in_one_place() {
    let home = root().join(STOP_RULE_HOME);
    let marks: Vec<String> = STOP_RULE_MARKS.iter().map(|mark| squashed(mark)).collect();

    let carrying: Vec<PathBuf> = delivered()
        .into_iter()
        .filter(|path| {
            let text = squashed(&read(path));
            marks.iter().any(|mark| text.contains(mark))
        })
        .collect();
    assert_eq!(carrying, vec![home.clone()], "按停止的规矩长出了第二份");

    let entry = squashed(&read(&home));
    for mark in STOP_RULE_MARKS
        .iter()
        .filter(|mark| mark.starts_with("Instruction::"))
    {
        assert!(
            entry.contains(&squashed(mark)),
            "{STOP_RULE_HOME} 里少了「{mark}」那一支"
        );
    }

    for (file, calls) in STOP_RULE_CALLERS {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：调它的那两路按文件路径记在 STOP_RULE_CALLERS 上，\
             模块挪了位置就把那张表跟着改"
        );
        let text = squashed(&read(&path));
        for call in calls {
            assert!(
                text.contains(&squashed(call)),
                "{file} 不再调「{call}」：那一路自己拿了主意，或者又抄了一份"
            );
        }
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

/// **型号认不出来时那一句拒绝只有一处出处**，抄出第二份就当场变红（`proof-sheet/06`）。
///
/// 转换那一趟、灰阶测试图那一趟、样张那一趟、预设里点的型号，四处认不出型号时说的都是它——
/// 样张 spec 的 story 26 要的正是「两条路上的说法不分家」。给样张另写一句，
/// 改了其中一句的人不会知道另一句也该改。
///
/// **问两件事**：别处没有第二份、家里真住着那一句（只问头一件的话，把那一句整个删掉这一条也是绿的）。
/// 前几条问的第三件——「指回来的路标还在不在」——在这里换成了一件运行时的事实：
/// 三条路**真都走到了它**，而不是哪一条自己另说了一句（那一句换了措辞，这里的记号一个都不命中）。
/// 那一件只有真进程答得出，在 `tests/exit_code.rs` 的
/// `an_unknown_device_is_refused_on_a_proof_in_the_words_run_and_calibrate_use`：三条路印到 stderr 上的字节逐个比。
#[test]
fn the_unknown_device_refusal_lives_in_one_place() {
    let home = root().join(DEVICE_REFUSAL_HOME);
    let marks: Vec<String> = DEVICE_REFUSAL_MARKS
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
    assert_eq!(
        carrying,
        vec![home.clone()],
        "型号认不出来时那一句拒绝长出了第二份"
    );

    let entry = squashed(&read(&home));
    for mark in DEVICE_REFUSAL_MARKS {
        assert!(
            entry.contains(&squashed(mark)),
            "{DEVICE_REFUSAL_HOME} 里少了「{mark}」那一截"
        );
    }
}

/// **翻大小写问盘那个探法只有一份**，补全与一趟开工时调的都是它（`one-source/03`）。
///
/// 从前补全自己攥着一份运行期的探法，撞名那一道按 `cfg!(windows)` 折——同一件事实两套判法，
/// macOS 上 `Abc.cbz` 与 `abc.cbz` 真会撞，撞名那一道却说它们是两个（停车场 Q366）。
///
/// **两件事一起问**：别处没有第二份、[调它的那几处](CASE_PROBE_CALLERS)真调它。
/// 只问头一件的话，补全把探法整个删掉、退回平台常量，这一条照绿。
#[test]
fn the_case_probe_lives_in_one_place() {
    let home = root().join(CASE_PROBE_HOME);
    let marks: Vec<String> = CASE_PROBE_MARKS.iter().map(|mark| squashed(mark)).collect();

    let carrying: Vec<PathBuf> = delivered()
        .into_iter()
        .filter(|path| {
            let text = squashed(&read(path));
            marks.iter().any(|mark| text.contains(mark))
        })
        .collect();
    assert_eq!(carrying, vec![home], "翻大小写问盘那个探法长出了第二份");

    for (file, calls) in CASE_PROBE_CALLERS {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：调它的那几处按文件路径记在 CASE_PROBE_CALLERS 上，\
             模块挪了位置就把那张表跟着改"
        );
        let text = squashed(&read(&path));
        for call in calls {
            assert!(
                text.contains(&squashed(call)),
                "{file} 不再调「{call}」：那一处自己拿了主意，或者又抄了一份"
            );
        }
    }
}

/// 屏上一句话里顺口提到的一个键，写法只有按键表一处出处；抄回一句就当场变红
/// （`design-parity/03`，收停车场 Q874、Q896、Q963）。
///
/// 屏底那一行与全部按键那一张的键早已从按键表派生（`keymap::hints`），而屏上还有十几句
/// **自己的话**提着一个键：「a → 全部页」「这一卷跳过了：… h → 回卷列表」「n N 跳到下一个」、
/// 确认条那四句、屏底那几句回话……它们**不随阶段改口**（等待确认那一档 `a` 让给答话、
/// 屏底不摆它，框底边那一句照样写着），因此不能从按阶段过滤的 `hints` 取，取的是
/// [不问阶段与块的那一手](KEY_HOME_MARK)；措辞仍是各自那一块自己的。从前那几个字母是手抄的
/// ——换一个键位，屏底跟着变、那几句不变，而没有一条用例红。
///
/// **三件事一起问**，与前几条同一个形状：**代码里**没有第二份（[`code_only`]，用例与文档里
/// 出现是记录不是出处）、[家里那一手](KEY_HOME_MARK)真住着、[读着它的那几个文件](KEY_READERS)
/// 还在读。只问头一件的话，把那几句的键各换一副别的手抄字，这一条也是绿的。
#[test]
fn the_keys_the_screen_mentions_come_from_the_key_table() {
    let marks: Vec<String> = KEY_SENTENCE_MARKS
        .iter()
        .map(|mark| squashed(mark))
        .collect();

    let mut code = Vec::new();
    collect(&root().join("src"), "rs", true, &mut code);
    // 一个文件连同它抄着的那几句一起报：红的时候一眼看得出该改哪几处。
    let carrying: Vec<(PathBuf, Vec<&str>)> = code
        .into_iter()
        .filter_map(|path| {
            let text = squashed(&code_only(&read(&path)));
            let copied: Vec<&str> = KEY_SENTENCE_MARKS
                .iter()
                .zip(&marks)
                .filter(|(_, mark)| text.contains(mark.as_str()))
                .map(|(said, _)| *said)
                .collect();
            (!copied.is_empty()).then_some((path, copied))
        })
        .collect();
    assert_eq!(
        carrying,
        Vec::<(PathBuf, Vec<&str>)>::new(),
        "屏上提到键的那几句话在代码里又手抄了一份"
    );

    assert!(
        squashed(&read(&root().join(KEY_HOME))).contains(&squashed(KEY_HOME_MARK)),
        "{KEY_HOME} 里少了「{KEY_HOME_MARK}」那一手"
    );

    for (file, reads, least) in KEY_READERS {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：读着按键表的几处按文件路径记在 KEY_READERS 上，\
             模块挪了位置就把那张表跟着改"
        );
        let found = squashed(&code_only(&read(&path)))
            .matches(&squashed(reads))
            .count();
        assert!(
            found >= least,
            "{file} 里从按键表取键的那一手「{reads}」从 {least} 处掉到了 {found} 处"
        );
    }
}

/// **转轮只有一段算式、会话的时钟只有一个起点**（`one-source/06`，收停车场 Q864）。
///
/// 从前顶栏那一截从视图那一格起点算、行首记号与总览从会话自己那一格起点算：同一个值存两处、
/// 算式写两段。夹具因此得摆两次，而合并 08 与 13 时那两次摆错了次序、差了整一格转轮——
/// 两棵树各自的闸门都是绿的。现在算式只在 [`SPINNER_HOME`]：从那一格起点算，
/// 再加一行自己的错相。
///
/// **四件事一起问**：[算式](SPINNER_MARKS)在代码里恰好一处、就在家里；
/// [时钟起点那一格](CLOCK_HOME_MARK)真住在家里；[另一格](CLOCK_COPY_MARK)哪儿都不在
/// （文档里也不在：说着一格不存在的东西的路标是一句假话）；[读转轮的那几处](SPINNER_READERS)
/// 真调它。只问头一件的话，一处读的地方退回手写一个字形，这一条照绿。
#[test]
fn the_spinner_and_its_clock_start_live_in_one_place() {
    let home = root().join(SPINNER_HOME);

    let mut code = Vec::new();
    collect(&root().join("src"), "rs", true, &mut code);
    let code: Vec<(PathBuf, String)> = code
        .into_iter()
        .map(|path| {
            let text = squashed(&code_only(&read(&path)));
            (path, text)
        })
        .collect();
    for mark in SPINNER_MARKS {
        let wanted = squashed(mark);
        // 一个文件连同它写了几遍一起报：家里写了两遍与别处抄了一遍都看得出来。
        let carrying: Vec<(PathBuf, usize)> = code
            .iter()
            .filter_map(|(path, text)| {
                let found = text.matches(&wanted).count();
                (found > 0).then(|| (path.clone(), found))
            })
            .collect();
        assert_eq!(
            carrying,
            vec![(home.clone(), 1)],
            "转轮那一段算式里的「{mark}」不止一处，或者不在家里"
        );
    }

    assert!(
        squashed(&read(&home)).contains(&squashed(CLOCK_HOME_MARK)),
        "{SPINNER_HOME} 里少了「{CLOCK_HOME_MARK}」那一格"
    );

    let copy = squashed(CLOCK_COPY_MARK);
    let carrying: Vec<PathBuf> = delivered()
        .into_iter()
        .filter(|path| squashed(&read(path)).contains(&copy))
        .collect();
    assert_eq!(
        carrying,
        Vec::<PathBuf>::new(),
        "会话的时钟起点又长出了第二格「{CLOCK_COPY_MARK}」"
    );

    for (file, call) in SPINNER_READERS {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：读转轮的那几处按文件路径记在 SPINNER_READERS 上，\
             模块挪了位置就把那张表跟着改"
        );
        assert!(
            squashed(&code_only(&read(&path))).contains(&squashed(call)),
            "{file} 不再调「{call}」：那一处自己拿了主意，或者又抄了一份"
        );
    }
}

/// 一个独立的词在这段文字里出现几次：**前面挨着字母、数字或下划线的不算**
/// （[`ISOLATED_NAME`] 说的那条）。
fn standalone_count(text: &str, word: &str) -> usize {
    text.match_indices(word)
        .filter(|(at, _)| {
            !text[..*at]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_ascii_alphanumeric() || before == '_')
        })
        .count()
}

/// **隔离目录的名字与「跳过」「没做成」两个词，代码里各只有一处**（`one-source/07`，
/// 收停车场 Q870、Q961）。
///
/// 从前会话那一头各手写着一份：每页结果头一行末尾「这一卷输出在 _isolated/」写死了目录名，
/// 而库那一格是私有的、会话够不着；卷列表灰阶分布那一格的两个词在画法那一层另写了一份，
/// 措辞那一层那一处（`render::tally_column`）只剩用例在读。两处今天写的都是同一串，
/// 屏上一个字不差——改名、换词那一天才分家，而那一天没有一条用例会红。
///
/// **三件事一起问**，与前几条同一个形状：**代码里**只在家里出现一次（[`code_only`]：
/// 用例与文档里出现是记录与引用，不是出处）、家里那一格真住着、读它的那几处真读它。
/// 只问头一件的话，那几处把字换一副写法手抄回去（`format!` 拼出来、换个引号），这一条照绿。
#[test]
fn the_isolated_directory_name_and_the_why_nothing_words_live_in_one_place() {
    let mut code = Vec::new();
    collect(&root().join("src"), "rs", true, &mut code);
    let code: Vec<(PathBuf, String)> = code
        .into_iter()
        .map(|path| {
            let text = code_only(&read(&path));
            (path, text)
        })
        .collect();

    // 一个文件连同它写了几遍一起报：家里写了两遍与别处抄了一遍都看得出来。
    let carrying = |count: &dyn Fn(&str) -> usize| -> Vec<(PathBuf, usize)> {
        code.iter()
            .filter_map(|(path, text)| {
                let found = count(text);
                (found > 0).then(|| (path.clone(), found))
            })
            .collect()
    };

    assert_eq!(
        carrying(&|text| standalone_count(text, ISOLATED_NAME)),
        vec![(root().join(ISOLATED_HOME), 1)],
        "隔离目录的名字「{ISOLATED_NAME}」不止一处，或者不在家里"
    );
    assert!(
        read(&root().join(ISOLATED_HOME)).contains(ISOLATED_HOME_MARK),
        "{ISOLATED_HOME} 里少了「{ISOLATED_HOME_MARK}」那一格"
    );

    for mark in WHY_NOTHING_MARKS {
        assert_eq!(
            carrying(&|text| text.matches(mark).count()),
            vec![(root().join(WHY_NOTHING_HOME), 1)],
            "{mark} 不止一处，或者不在家里"
        );
    }

    for (file, reads) in [ISOLATED_READER, WHY_NOTHING_READER] {
        let path = root().join(file);
        assert!(
            path.is_file(),
            "{file} 不在了：读的那一处按文件路径记在 ISOLATED_READER、WHY_NOTHING_READER 上，\
             模块挪了位置就把那两格跟着改"
        );
        assert!(
            code_only(&read(&path)).contains(reads),
            "{file} 不再读「{reads}」：那一处又手写了一份，或者自己拿了主意"
        );
    }
}
