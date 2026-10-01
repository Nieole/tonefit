# 03 — 「是不是同一处」一把尺子

**What to build:** 库里一个判等：**字面规整**（去掉 `.` 分量与尾分隔符，不解析 `..`），再按那个位置所在的文件系统分不分大小写决定折不折。
判等是纯函数：两条路径加一个「这一侧不分大小写吗」的答案进去。探法（只读地翻一个已有名字的大小写问盘）从补全那一处搬进库里。

**每一趟开工时，每条处理路径与输出根各探一次**，不跨趟记任何东西（ADR 0009）；输出根还不存在就探离它最近的已存在的上一级。
**探不出时两侧取相反的一边**：源那一侧（收编）按分大小写，输出那一侧（撞名、借住）按不分大小写。
收编、借住、撞名三处走它，撞名不再按编译平台折；补全照旧用它自己那一格进程内记忆，只是调库里这个探法。
软链不解析，两个名字当两个。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 判等纯函数的用例：规整（`.`、尾分隔符、`..` 不解析）× 折不折大小写 × 两侧探不出时各取哪一边，逐格钉住
- [x] 探法在跑用例的那个文件系统上验一次；不为测试开注入探法的口子
- [x] 集成用例（本机不分大小写）：`D:\库` 与 `d:\库\作品` 收编成一棵树、同一卷只做一遍；`库` 与 `./库` 同理；`Abc.cbz` 与 `abc.cbz` 要写进同一处时拒绝开始
- [x] 借住认得出大小写不同的去处
- [x] 补全照旧逐层补得出，调的是库里那一个探法
- [x] 报告里印的处理路径仍是用户点的那一条写法
- [x] `CONTEXT.md`《尚未确立》添一条：软链两个名字当两个
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 库里新添模块 `src/place.rs`，「是不是同一处」只有它一把尺子（`CONTEXT.md` 新词条《同一处》）：

1. **判等**：`same_place(a, b, case)` 纯函数；查表用它的键 `Place`（规整过、按那一侧折过的那串分量，两者是同一件事，
   一条用例钉住「键相等 ⟺ 判等」）。规整靠 `Path::components` 已去掉的中间 `.`、尾分隔符，再去掉打头那个 `./`；`..` 原样留着。
   折法逐个分量、逐字取小写，非 UTF-8 的分量不折。`Side::{Source, Output}::case` 是「探不出时两侧取相反的一边」。
2. **探法**：`tonefit::case_sensitivity`（公开，补全要用），从补全搬来——拿一个已有名字翻一次大小写问盘，只读，没有注入口子。
   从离那个位置最近的已存在的那一级问起，那一级翻不出名字就在**同一块盘上**往上问（Q1159）。
3. **每一趟开工时探**：`run` 在探写之后探输出根一次；清点里每条处理路径探一次（只探点得开的那几条）。答案只活在这一趟里。
4. **收编**（`discover::Found`）：卷根按 `Places` 认——两条处理路径的答案可以不同，字面相同恒是同一个，只差大小写的两边都不认才是（Q1158）。
   「哪个点名根更外层」数的是规整之后的级数，`./库` 与 `库` 一样深。卷根留头一回见到的写法，报告照印。
5. **借住**（`survey::find_the_lodgers`）：按输出那一侧的答案取键往上找祖先；记下的那一段取住户自己的写法。写出那一层不动（Q1162）。
6. **撞名**：`collision_key`（`cfg!(windows)` 折）删掉，按输出根的答案取 `Place` 当键；`normalises_an_extension` 用 `same_place` 比文件名。
7. **补全**：`CaseSensitivity` 与探法搬走，`remembered` 改调 `tonefit::case_sensitivity`，自己那一格进程内记忆照旧（Q1160）。
8. **文档**：`CONTEXT.md` 添《同一处》（新词）、《尚未确立》添「软链两个名字当两个」；《大小写敏感性》末一句不再成立，没改，记 Q1161。

**用例**（库内 +12、`tests/` +6 条，补全删 5 条注入探法的、改 1 条）：

- `src/place.rs`：规整两条（`.`／尾分隔符／`./`；`..` 不解析）、折不折一条、探不出两侧各取哪边一条、键与判等一致一条、
  `Places` 两侧答案不同一条、盘符一条（`#[cfg(windows)]`）；探法在真盘上五条——答的是这台机器（用例自己翻一次大小写问盘对照）且探完盘上不多东西、
  还不存在的地方问最近的那一级、一级翻不出名字问上一级（含文件）、两个只差大小写的名字同在一级（建不出来的盘上收工）、
  进不去问的那一级不是答案（Unix、root 底下收工）。
- `tests/discovery.rs`：`Lib` 与 `lib/作品`（本机不认大小写：一棵树、只做一遍、镜像以最外层为准、报告印的是先点的 `lib/作品/第1话.cbz`；
  认大小写的盘上摆一份真不同的、两卷各做各的）；`库/作品 ./库` 起真进程（Q1164）；盘符翻大小写那一例（`#[cfg(windows)]`，Q1157）。
- `tests/pipeline.rs`：`甲部/Abc.cbz` 与 `乙部/abc.cbz`——不认的盘上拒绝开始、两卷都点名、说分批处理不说扩展名、输出目录不建；认的盘上两卷各写各的。
- `tests/container.rs`：`甲/N和S`（封面一卷）与 `乙/n和s/第1话.cbz`——第二趟只改封面，那一话整卷跳过（认得出借住）。
- `tests/single_source.rs`：翻大小写那个探法只有 `src/place.rs` 一份，补全、清点、`run` 三处真调它。
- 认不认大小写那三条集成用例都由 `fixtures::the_disk_folds_case` 自己问一次盘，分两支各断言各的。

**按反跑过的**（`docs/agents/testing.md`）：

- 探法 `Ok` 答成「认」：探法那三条真盘用例红。
- 问盘被拒答成「认」：`a_question_the_disk_refuses_is_not_an_answer` 红（`Some(Sensitive)` 对 `Some(Insensitive)`）。
- 收编的级数照字面数（`named.components().count()`）：`./库` 那条红，输出树成了 `作品/第1话.cbz`。
- 源那一侧恒按认：`Lib`／`lib` 那条红在「同一卷做了不止一遍」（2 对 1）。
- 输出那一侧恒按认：`Abc.cbz` 那条红在「该在开工前被拒」。
- 找住户恒按认：借住那条红在那一话 `PerPage` 而不是整卷跳过。
- 动手前四条集成用例、单一出处那条与库内那几条都各红过一次（同样的位置）。
- 没能按反的：两个只差大小写的名字同在一级那条（这台盘建不出那一对，Linux 上才咬人）；探法不跨盘那一条（要挂一块盘，用例里造不出）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff ae578d6` 加未跟踪的 `src/place.rs`。Spec 轴没找到缺口、没找到错的实现。

**收下的**：

- 探法一直往上走会跨过挂载点：一个空的、不认大小写的 U 盘挂在认大小写的目录底下时上一级答「认」，正是输出那一侧最怕的那一边（Spec）。
  改成只在同一块盘上往上走（Unix 按设备号），Q1159 跟着改。
- 《同一处》照抄了「探离它最近的已存在的上一级」「库里只有一把尺子」，与实现对不上（Standards，文档写作第 1 条）——改写成实际的走法，
  并点明「输出落在源里」那一道不用这把尺子（Q1165）。
- 盘符那条用例的文档写「由上一条钉着」（稳定引用）——改成点用例名。
- `Found` 拆出一个与 `candidates` 一格对一格的 `outermost: Vec`（Data Clumps）——收成一格一个 `Seat`。
- 「翻一个大小写去问盘」在三条集成用例里各写一份（Duplicated Code）——收进 `fixtures::the_disk_folds_case`。库内那一份留着：库内用例够不着 `tests/fixtures`。
- 「探不出时两侧取哪边、各是什么代价」在词条与 `Side::case` 各写一整遍、「每趟开工时各探一次」写了五处（单一出处）——
  代价只留在词条里，`Side::case`、`run`、清点那几句收成一句加指路。
- `Place::first`／`after` 说不出交回的是什么（Mysterious Name）——改名 `ancestor`／`below`。
- 问盘被拒那一支的用例随注入口子一起删了（Standards 旁注）——补了一条真盘上的（Unix 上改目录权限）。
- Q1164 推荐没写理由、Q1165 的选项②近乎稻草人——补了理由、换了选项。

**驳回的**：

- `Places::find`／`insert` 与 `fold` 各判一次 `Insensitive`（Repeated Switches）：三处做的是三件不同的事，并成一张表反倒要多一层。
- `output_root` 与 `output_case` 同行（Data Clumps）：只有撞名那一处调用。
- `Places`、`at` 的名字：`at` 是既有字段名；`Places` 文档写清了它是「按同一处认、各带一侧答案」的那张表。
- 补全行为变了与「照旧」对不上（Spec）：票面那句「照旧」说的是补全自己那一格进程内记忆，那一格照旧；
  这一层翻不出名字时由同一块盘的上一级答，是改调库里探法的直接后果，记在 Q1160。
- 停车场条目缺 **Why it matters**：那一栏不是必填，每条的代价已写在 What this ticket actually did 里。

### 停车场

本票用了 Q1157–Q1166 全部十个：

- **Q1157**：盘符那一例本机造不出，钉的是只差大小写的一级目录；两条 `#[cfg(windows)]` 没编过、没跑过。
- **Q1158**：两条处理路径答案不同时，只差大小写的卷根按「认」。
- **Q1159**：探法在同一块盘上往上走到答得出为止。
- **Q1160**：补全改调库里探法，这一层翻不出名字时由上一级答、照样记。
- **Q1161**：《大小写敏感性》末一句不再成立，没改。
- **Q1162**：写出那一层比盘上名字仍逐字节。
- **Q1163**：折法不认 Unicode 规范化与多字折叠。
- **Q1164**：`./库` 那条起真进程。
- **Q1165**：「输出落在源里」那一道仍用它自己那把解析软链的尺子。
- **Q1166**：相对写法与绝对写法不互相认。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后跑的。日志是 `os-03.gate1.log`、`os-03.gate2.log`、`os-03.gate3.log`、`os-03.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1079 通过 1 失败**；lib 250 / bin 435（1 ignored）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.90s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **961 通过 1 失败**；lib 250 / bin 317；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.89s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.70s` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**两道钉子**：`tests/golden.rs` 2 条全过（闸门 1 上 147.76 秒、闸门 2 上 158.80 秒），黄金快照没动。

**跑完之后改过一处**：polish 绿，但两道 clippy 都在 `tests/container.rs` 本票那条借住用例上报了一条 `clone_on_copy`
（`Option<VolumeVerdict>` 是 `Copy`，多写了一个 `.clone()`）。删了那个 `.clone()` 之后只窄跑过：两种特性组合各一遍
`cargo clippy --test container`（无告警）、`cargo test --test container -- a_lodger_is_recognised`（过）、`cargo fmt --check`（过）。
四条宽的没有为这一个字重跑。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q241 — 收编按**字面路径**认「同一个卷根」：大小写、`./`、软链，三种写法都折不到一起

- **From:** 票 `p4-parking-lot/15`（大小写那一半由评审补上）
- **Kind:** 本票造出来的新形状（收编的边界）
- **Where:** `src/discover.rs` 的 `Found` 那张 `HashMap<PathBuf, …>`；对照
  `src/lib.rs` 的 `collision_key`（撞名那一道按平台折大小写）
- **Why it did not block:** 收编按 `PathBuf` 本身查，而 `Path` 的相等**逐字节**比，
  在哪个平台上都一样。三种写法因此认不出是同一个卷根，Q111 那个形状原样回来
  ——同一卷做两遍、写两份，去处还不同，撞名那一道也拦不住（它比的是**去处**）：
  1. **大小写**（Windows／macOS 上最容易撞见的一种）：`tonefit --out D:\out D:\库 d:\库\作品`
     ——盘符大小写不同就够了，一个是手敲的、一个是补全或粘贴来的。
     `Path` 不折大小写，两边各展一遍。
  2. **`.` 与相对写法**：`tonefit 库 ./库`。
  3. **软链**：点名一条软链与它指向的那棵树。
  第 2、3 种要同一条命令行上把同一个库写成两种写法，场合很窄；**第 1 种不窄**——
  在不区分大小写的文件系统上，两种写法指的就是同一个文件。
- **What this ticket actually did:** **没有规范化。** 本票的派活说明写死了可动的文件
  （`src/discover.rs`、`src/survey.rs`、它们的用例、`tests/discovery.rs`，加两份 markdown），
  而这三种都要一把「文件系统认不认成同一个东西」的尺子，那把尺子已经在
  `src/lib.rs` 的 `collision_key` 上（撞名那一道用它，`normalises_an_extension`
  的文档明写「与比去处同一把尺子」）——在 `discover` 里另抄一把就是第二个出处，
  而把它开成 `pub(crate)` 要动 `src/lib.rs`，落在本票之外。
  **评审驳掉的一条理由记在这里**：本条初稿写的是「规范化会把报告里印的那条路径换掉」，
  那不成立——`at` 那张表的**键**可以取规范化过的形式，而 `Candidate::root`
  照旧留用户点的那一条。第 1 种（大小写）尤其便宜：`collision_key` 一个平台折叠就够，
  一次系统调用都不必付。第 2、3 种要 `crate::resolve`（`std::path::absolute` 加
  `canonicalize`），那是逐个候选一次系统调用，另一笔账。
- **Whose call:** 拍板的人（收编按哪把尺子认「同一个卷根」——字面、按平台折大小写、
  还是解析到规范路径）
- **处置：** **`one-source/03` 落地（2026-10-01）：照票面了结。**收编按《同一处》认卷根：字面规整（`./库` 与 `库`）加按处理路径探出来的大小写敏感性折（`D:\库` 与 `d:\库`）；软链两个名字当两个，记进《尚未确立》。

#### Q300 — 「谁住在谁的去处里」按**镜像路径逐字节**认，与撞名那一道用的不是同一把尺子

- **From:** 票 `p4-parking-lot/16`
- **Kind:** 路过发现
- **Where:** `src/survey.rs` 的 `lodge`（`HashMap<PathBuf, usize>` 按 `output_relative` 查）；
  `src/lib.rs` 的 `collision_key`（撞名那一道那把会折大小写的尺子）；停车场 **Q241**
- **Why it did not block:** `lodge` 认「这个卷的去处是不是那个卷去处的祖先」靠 `Path` 的
  分量相等，而那在哪个平台上都逐字节比。在不区分大小写的文件系统上，`out\N和S` 与
  `out\n和s` 是同一个目录而这里认不出——借住的卷因此认漏，那一卷退回「整个换掉」，
  Q113 的形状原样回来。**这是 Q241 记的那把尺子的第二个受害点**：那一条说的是收编按字面
  路径认卷根，这一条说的是借住按字面路径认去处。造不出来的一半是：两条镜像路径的分量
  出自同一批源名字，同一趟里写法不会自己变——要撞上得是用户在**源**那一侧给了两种写法
  （软链、`./`、盘符大小写），也就是 Q241 举的那三种。
- **What this ticket actually did:** **按字面比，没有去引那把尺子。** `collision_key` 是
  `src/lib.rs` 的私有件，开成 `pub(crate)` 要动那个文件的对外面，而票面写死了本票只该动
  写出与发现这条线；在 `survey` 里另抄一把就是第二个出处（Q241 自己已经写过这条理由）。
  认漏与认多的后果都写在 `lodge` 的文档里：**认漏是退回从前的行为，认多是少清一件陈旧
  产物，两种都不写坏东西**——这是选它的理由，不是它没有代价。
- **Whose call:** 拍板的人（「文件系统认不认成同一个东西」该不该有一把全仓共用的尺子
  ——与 Q241 同一个问题）
- **处置：** **`one-source/03` 落地（2026-10-01）：照票面了结。**找住户按输出根探出来的答案取《同一处》的键往上找祖先；写出那一层比盘上名字没动（Q1162）。

#### Q366 — 撞名那条判据仍按 `cfg!(windows)` 折大小写，而补全已经改成运行期探

- **From:** 票 `p4-parking-lot/23`
- **Kind:** 路过发现（同一个病，另一处）
- **Where:** `src/lib.rs` 的 `collision_key`：`if cfg!(windows) { text.to_lowercase() } else { text }`，
  连同它上面那句「Windows 上大小写不区分，`Abc.cbz` 与 `abc.cbz` 是同一个文件；别的平台上是两个」
- **Why it did not block:** 本票治的是**补全**那一处，票面从头到尾说的是「逐层补全认不认大小写」；
  撞名是**拒绝执行**的一条判据（`CONTEXT.md`《失败》），改它要探的是**输出根**那一层、
  动的是 `src/lib.rs`，与补全那条路一个函数都不共用。补全这一处改完之后，
  仓库里对同一件事实有了两套判法——但两套各自自洽，没有一条现成用例因此变红。
- **What this ticket actually did:** `collision_key` **一个字没动**，并把新立的词条**限在补全上**
  （`CONTEXT.md`《会话》那条写的是「逐层补全按前缀筛时认不认大小写」，末尾点明「只管补全这一处」），
  不去替撞名那一处发言。
- **Options:** ① 照现在（两处两套判法，词条只管补全）；② 撞名那一处也改成运行期探——
  探输出根所在的文件系统，判据合成一处；③ 反过来让补全也退回平台常量，两处一致地错。
- **Recommend:** ②，但**不该由本票做**。它改的是一条拒绝执行的判据（判宽了让两卷写进同一个去处，
  判严了拦下本来跑得成的一趟），那是领域决定不是实现细节；而且 macOS 上 `Abc.cbz` 与 `abc.cbz`
  **真的**会撞，今天那一支说它们是两个——这是一个真 bug，值得一张自己的票。
  ③ 只为一致而把已经修好的一处退回去，不成立。
- **Whose call:** 拍板的人
- **处置：** **`one-source/03` 落地（2026-10-01）：照 ② 了结。**撞名不再按编译平台折，按输出根这一趟开工时探出来的答案折；补全与一趟开工时调的是同一个探法。《大小写敏感性》末一句没改（Q1161）。
