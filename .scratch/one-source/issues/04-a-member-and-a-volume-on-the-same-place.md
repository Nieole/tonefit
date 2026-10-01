# 04 — 成员与卷撞同一个去处

**What to build:** 撞名那一道多比一种：**一个卷的成员去处**与**住在它去处里的另一个卷（借住的卷）的去处**。一对一那一套输出名
（`001.jpg` → `001.png`）清点时就比——借住关系在清点时已经算出，住户所在那一卷的成员清点时列过；撞上即**拒绝开始**，
那句话说出撞的是哪一张页、哪一卷。只有解了像素才知道的那一半（跨页切开之后的名字）照旧在它那一卷里查、撞上走卷级失败。

**Blocked by:** 03

**Status:** resolved

- [x] 集成用例：混装目录里 `001.jpg` 与目录卷 `001.png/` 并存时拒绝开始，一个字节都不写；那句话点得出那一张页与那一卷
- [x] 比较走 03 那一把判等（输出那一侧的折法）
- [x] 切开那一半撞上时照旧卷级失败（既有用例照旧绿）
- [x] `CONTEXT.md`《拒绝开始》那一种改成「两样东西撞同一个去处」
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 落地记录

**本票做了什么。** 撞名那一道多比一种：**一个卷的成员**与**住在它去处里的另一个卷（借住的卷）**撞同一个去处（收停车场 Q301）。

1. **一对一那一套名字在清点里比**（`src/survey.rs`）：列成员之前，从候选的镜像路径认出哪几个去处住得下别的候选
   （`places_with_room_for_a_lodger`，Q1180），只有这几个目录卷一张都不切时的输出名留到借住算出来。`find_the_lodgers` 改成只认、交回那几对
   （`Lodging`），`clashes` 拿成员名比住户那一段的**头一级**（Q1177），比完 `record_the_lodgers` 记进 `Surveyed::lodgers`。比法是 03 那一把：
   `Place`（`same_place` 的键），按输出根这一趟探出来的答案。两处数祖先走同一个 `Place::into_ancestors`。撞上的那几对攒成 `Survey::clashes`
   （`Clash`：成员身份、住户卷根、相对输出目录的那一处）。
2. **拒绝开始**（`src/lib.rs` 的 `ensure_no_member_clashes_with_a_lodger`）：排在卷与卷那一道之后、开工那条事件之前（Q1178），
   那句话逐条印那一处、`← 成员（成员）`、`← 住户卷根（借住的卷）`，出路只给改名（Q1179）。输出目录根本不建。
3. **切开之后的名字在那一卷里比**（`ensure_no_page_clashes_with_a_lodger`）：与 `ensure_no_two_outputs_collide` 同一刻、同一个待遇——
   卷转换失败，住户照做（Q1181）。比法同上，`process_volume` 多收一个 `output_case`。只问目录卷（归档卷的成员写在包里，Q1183）。
4. **一对一那一套名字一处出处**：`one_to_one_targets` 换成 `one_to_one_names`（成员, 输出名，页与透传文件），卷内那一道
   `ensure_one_member_per_output` 与清点那一道都读它。
5. `src/sink.rs`：`Lodgers` 添 `heads`（去处直接那一层通往住户的那几级），`leads_to_one` 改调它、**仍逐字节比**（Q1162 没碰）；
   `swap_members` 里指着 Q301 的那句注释改写成「挡路的同名目录不该是借住的卷」，指着两道新比较。
6. **文档**：`CONTEXT.md`《拒绝开始》「两个卷撞同一个去处」改成「两样东西撞同一个去处（两个卷，或一个卷的成员与借住在它去处里的卷）」，
   `tests/single_source.rs` 守那张单子的记号跟着换；《只列前几条》与 `src/listing.rs` 的使用者清单从六处添成七处、量词添「对」
   （使用者清单是事实，不是词条含义）。《卷转换失败》的举例没改（Q1182）。`src/place.rs` 模块文档里「撞名」那一句点到两种比法。

**代价**：一张会被切开的 `001.jpg` 清点时仍按 `001.png` 比——它与 `001.png/` 那一卷同在时照样拒，哪怕切开之后其实不写 `001.png`
（spec 认下的「一对一那一套清点时就比」，与卷内那一道 `ensure_one_member_per_output` 同一个口径）。

**用例**（`tests/` +3、库内 +2；`tests/single_source.rs` 改记号）：

- `tests/pipeline.rs`：`N和S/001.jpg` 与 `N和S/001.png/`——拒绝开始、那句话点得出 `← 页`、`← 那一卷`与去处、不念「分批处理」、输出目录不建；
  `001.jpg` 与 `001.PNG/`——按 `fixtures::the_disk_folds_case` 分两支（不认大小写的盘上拒，认的盘上两卷各写各的）；
  跨页 `001.png` 切出的 `001-1.png` 撞上 `001-1.png/` 那一卷——住户点名在前、先写出来，混装目录那一卷记一笔卷转换失败、住户的输出一个字节不动。
- `src/survey.rs`：住户躺在更深处（`001.png/第1话/`）同样算撞；透传文件 `info.txt` 撞上两条处理路径镜像进来的 `info.txt/` 那一卷。

**按反跑过的**（`docs/agents/testing.md`）：

- 动手前三条集成用例各红一次：前两条红在「处理应当失败」（混装目录那一卷先写，住户腾不出位置、记一笔卷转换失败——没有一处拒），
  切开那一条红在「撞上借住的卷没被拦下」（`failed_volumes` 为空：混装目录那一卷收尾把住户的整卷输出清掉了）。
- 清点那一比按认大小写比：大小写那一条红在「该在开工前被拒」，另两条照绿。
- 比住户那一段的整段而不是头一级：`a_member_standing_on_the_way_to_a_lodger_clashes_with_it` 红（0 对），整段相同的那条集成用例照绿。
- 预筛反过来（只给住不下别的卷的去处留名字）：库内那一条与两条拒绝开始的集成用例红。
- `one_to_one_names` 不带透传文件：`a_pass_through_file_clashes_with_a_lodger_too` 红。
- 没能按反的：卷内那一比的大小写折法（没有用例造切开的名字与住户只差大小写的一对）；预筛按超集认、多留了的那一种（结果相同，只差内存）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 2f84ad8`。Spec 轴：四条验收都有着落、没找到错的实现，Q1177–Q1183 都判为合理读法。

**收下的**：

- 透传文件那一侧没有用例钉着（Spec）——补了 `a_pass_through_file_clashes_with_a_lodger_too`，按反跑过。
- 《只列前几条》写着「用它的有六处」、量词没有「对」，与新添的那条拒绝对不上（Standards，记进停车场或改）——使用者清单是事实，
  `CONTEXT.md` 与 `src/listing.rs` 两处一起改成七处、添「对」。
- `tests/single_source.rs` 那条记号的折行举例还是「两个卷撞同一」（Standards，结果）——改了。
- `houses` 与 `find_the_lodgers` 各写一遍祖先遍历，「超集、漏不了」靠两份手抄一致（Duplicated Code）——收进 `Place::into_ancestors`，两处都调它。
- `houses`、`take_in`、与函数同名的局部 `one_to_one`、丢了「targets」的 `one_to_one`（Mysterious Name）——改名
  `places_with_room_for_a_lodger`、`record_the_lodgers`、`named`、`one_to_one_names`。
- 落地记录与按反跑的记录还没写（Standards）——就是这一节。

**驳回的**：

- 「头一级按 `Place` 取键」清点与卷内各写一次（Duplicated Code）、`ensure_no_page_clashes_with_a_lodger` 伸手进 `Lodgers` 建索引（Feature Envy）：
  收进 `Lodgers` 就得让它带着输出那一侧的答案，那正是 Q1162 待拍板的那一手；两处共用的是 `Lodgers::heads` 与 `Place`，剩下的是一行取键。记在 Q1181。
- 「只有目录卷才比」判了两次（Repeated Switches）：两处守的是两份不同的数据（清点留不留名字、那一卷比不比真产出的名字），各一行。
- `Vec<(usize, Vec<(PathBuf, PathBuf)>)>`（Primitive Obsession）：只在 `Survey::of` 与 `clashes` 之间走一次，造一个类型不值。
- `Clash` 没进词汇表：它是《拒绝开始》里「两样东西撞同一个去处」那一种的一条记录，库内类型（`Slot`、`Seat`）一向不进词汇表。
- 外面那一卷叫法不一（混装目录那一卷／外面那一卷／成员那一卷）：各在各的上下文里点的是不同的侧面，词汇表里没有它的词条。
- 停车场条目缺 **Why it matters**：不是必填，代价已写在 What this ticket actually did 里。

### 停车场

本票用了 Q1177–Q1183：

- **Q1177**：成员挡在住户去处的祖先上（住户在更深处）也算撞——比头一级。
- **Q1178**：两种撞车各一道、各一句话，没并成一条拒绝。
- **Q1179**：那句话只给「改名」一条出路。
- **Q1180**：成员名只给去处住得下别的候选的目录卷留（预筛）。
- **Q1181**：切开之后的名字撞上借住的卷，在那一卷里补比；那句话点不出住户卷根；`output_case` 递参数、不塞进 `Lodgers`。
- **Q1182**：《卷转换失败》的举例没添新那一种。
- **Q1183**：归档卷的去处挡在另一个卷的去处上（祖先），照旧没人查——两种结局都是卷转换失败、不毁东西。

**Q1162 的现状**：`Lodgers::leads_to_one`／`spoken_for` 仍逐字节比住户那一段与盘上的名字；本票只把「取头一级」收进 `heads`，比法没动。

### 数

最终状态跑的那一趟：评审收完、`cargo fmt` 过之后，四条顺序跑。日志是 `os-04.gate1.log`、`os-04.gate2.log`、`os-04.gate3.log`、`os-04.polish.log`，
都在树外，每份末尾记着退出码。这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录；闸门 3 走 `cargo xtask gate 3`。本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1088 通过 1 失败**（1 ignored）；lib 252 / bin 439；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 76.73s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **967 通过 1 失败**；lib 252 / bin 318；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.12s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `全绿。`（检查那一步 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 9.36s`） |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；末行 `全绿。`；两道 clippy 零告警；`cargo doc` 告警 15 条（`warning: \`tonefit\` (lib doc) generated 15 warnings`），与基线同数 |

**黄金快照**：`tests/golden.rs` 2 条全过（闸门 1 上 220.00 秒、闸门 2 上 164.78 秒），快照没动。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q301 — 本趟成员的去处与一个**借住的卷**的去处撞名时，没有一处查得出来

- **From:** 票 `p4-parking-lot/16`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/sink.rs` 的 `DirectorySink::swap_members`（腾位置那一句）；
  `src/lib.rs` 的 `ensure_no_two_volumes_share_an_output`、`ensure_one_member_per_output`、
  `ensure_no_two_outputs_collide`
- **Why it did not block:** 源里同时有 `N和S/001.jpg` 这一页与 `N和S/001.png/` 这个目录卷
  时，前者的输出成员名归一成 `001.png`，后者的去处也叫 `001.png`——**同一个去处**。
  三道撞名校验一道都拦不住：头一道比的是卷与卷，后两道比的是同一卷内的成员与成员，
  而这一撞是**一个成员**与**一个卷**。造得出来，只是要用户在同一个目录里摆一个与某张页
  同名（换过扩展名之后）的子目录卷，真实漫画库里没见过。
- **What this ticket actually did:** **与「整个换掉」那一支同一个结果：本趟的成员让它让位。**
  `swap_members` 搬成员之前把挡路的同名目录清掉，那一句因此可能清掉的是一个借住的卷。
  这与本票之前的行为**逐字节相同**（那时整个去处都被删掉，那一卷自然也没了），
  所以不是本票造出来的回归；改成「不清、让改名失败、报一笔卷级失败」是**新长一种失败**，
  而它的口径属于撞名那一道，不在本票范围里。`swap_members` 里那一句的注释照实写着
  这两种可能，并指着本条。
- **Whose call:** 拍板的人（撞名那一道要不要把「一个卷的去处」与「另一个卷的成员去处」
  也比进去）
- **处置：** **`one-source/04` 落地（2026-10-01）：照票面了结。**撞名那一道比得出成员与借住的卷：一对一那一套名字在清点里比、撞上拒绝开始；切开之后的名字在那一卷里比、撞上卷转换失败（Q1181）。`swap_members` 那句注释改写，不再指着本条。
