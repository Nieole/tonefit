# 10 — 已结束、备注行与整卷统一灰阶

**What to build:** 一趟跑完之后在新界面上看结果，以及整卷统一灰阶那一趟多出来的那一列。

- 结束之后总览抬头换成**完成 / 已停止 / 已中断**加用时，右端写输出目录；结论行是转换那一副；
- 转换失败的卷是它目录里的 `✗` 卷行，行尾是那句原因；
- **备注行**挂在它所在那条处理路径的分区或顶格目录行末尾：无法访问的地方一处一行（`✗`），
  非漫画文件合成一行（`-`）；
  `⏎` 掀开**说明卡**看全文，`Esc` 关；
- 结束之后 `o`／`i` 回到开跑之前那一副，`t`／`x` 再开一趟；
- 整卷统一灰阶那一趟卷行在灰阶分布之后多一列**代表页**，砍列时排在耗时之后；默认逐页那一趟整列不在场、列头也不占（Q712）。

真会话仍进旧界面。

**Blocked by:** 08 — 清点中与转换中

**Status:** resolved

- [x] 「已结束」「整卷统一灰阶」各 120×36、80×24 逐格相等
      —— **「整卷统一灰阶」那两屏各打了折**：环节横条上三格，见《落地记录》的《两处打了折的验收》
- [x] 序列：备注行 `⏎` → 说明卡 → `Esc`；结束之后 `o` 回到路径列表；`s` 按一次推进到停下、`s` 按两次立即停下，两种结束抬头；走完与期望屏相等
      —— `ended-nonvolume-Enter` 那一串**打了折**（正文折下来的两行往右推四格），同上
- [x] 用例：默认逐页那一趟代表页那一列连列头都不在场
- [x] 真会话仍进旧界面，旧用例照绿；设计快照未改

## 落地记录

**做了什么。** 一趟跑完之后那一副在新界面上齐了：四屏逐格、十八串序列、五处纯逻辑各一条红→绿。
真会话仍进旧界面（`src/main.rs` 一字没动），旧的焦点、画法与用例原样留着、照编照过；
`tests/fixtures/design/` 一个字节没动（`git diff --stat -- tests/fixtures/design` 为空）。

**08 交下来的那几样全都能用，一处都没返工**：备注行的骨架（`Tree::rows` 出的备注行、
`render::non_volume_heading` 出的行尾那一句）、代表页那一列（`Notable::Driver` 那一页的名字）、
`marks::*`、`TaskView::surveyed`、`Tree::index_of`、`Spot`、`VolumeState::settled`——
接上去就对。**四屏里 08 已经画对的部分多得出乎意料**：`ended` 那两屏第一趟红只差屏底一件
（`[l → 每页结果]`），树上每一行、结束之后的总览抬头与结论行、备注行全都是对的。

### 新添与改动的名单

| 哪儿 | 什么 |
|---|---|
| `render` | `volume_name` **归档卷去掉扩展名**（Q849）；`non_volume_stack`／`unreachable_stack`（末尾那两小结摞成一块的形状，「一条路径怎么写」交给调用方），两条 `*_tail` 改成读它，命令行那一段逐字节不变 |
| `session::tree` | `Note::said`：这一条装着的那几处（路径与那一句为什么），说明卡的全文从它拼；`Tree::locate_note`（光标记着的身份 → 树上的位置） |
| `session::cover`（特性外） | `Overlay::Note { node, at }`；**`Card`**：说明卡的全文与几何（居中、宽至多 76、高随正文，正文走 `crate::wrap::fold`）；`Views::lift_note`；`scroll_cover` 补上半屏与一屏那四个（这一张上都挪一整屏），说明卡不滚 |
| `session::view`（特性外） | `Window::page`（设计稿的 `pageH`）；`Session::scroll_list`（`C-d`／`C-u`／`C-f`／`C-b`）；`Session::open_want`（屏底那一件「展开／每页结果／查看」随光标那一行换）；`Session::volume_state`；`hints` 多收一个 `Option<&Live>`；`Deed::Open` 在备注行上掀卡、`Deed::BackToPaths` 接上 |
| `session::live` | `VolumeState::opens_the_pages`：**展不展得开的判据一处**——屏底摆不摆 `l` 与按下去换不换屏读同一份 |
| `session::state` | `Session::back_to_paths`：阶段退回 `Stage::Fresh`，那一趟仍攥在手上（Q851） |
| `session::shell::overlay` | 两张分开画：全部按键那一张原样，**说明卡**照 `drawNote`（框、抬头、「是哪几处」、正文；`Esc → 关闭` 从按键表取） |
| `session::shell::canvas` | 滚动条的滑块**照设计稿那两条式子自己算**，不再过终端库那个 widget（收掉 Q780 那一格） |
| `session::shell::overview` | 抬头与右端从问「有没有那一趟」改成问**阶段**（`o` 回到开跑之前那一副时那一趟还在手上） |
| `session::shell::footer` | `draw` 多收那一趟（屏底那一件要问光标那一卷展不展得开） |
| `session::look`／`draw::paint` | 种类色加一种：**说明**（灰）——说明卡的全文、详情栏的长说明、预设栏那一段；`CONTEXT.md` 的《语义色》跟着补 |
| `session::scene` | `views_of` 认光标那两种的键名修了（Q846），备注行那一种挪成一趟后置（`stand_on_a_note`，树拼出来之后按「是哪几处」认）；`Scene::advance_to`：序列里「推进几秒」那一步 |
| `session::draw::design` | `Expected::shifted`（往右推几格，被挤掉的必须本来是空白）、`Expected::cell_like`（一格换成同一行另一格）——两手都**换完仍是断言**，与 `blanked` 同一副做法 |
| `session::viewport` | `Scrollbar` 那份文档跟着改：怎么画滑块**两副界面各一处**（旧那一副仍走 widget），Q850 |

### 两处打了折的验收

**「整卷统一灰阶」那两屏与 `ended-nonvolume-Enter` 那一串各打了折**，两处都是**设计稿与实现
对不上**，而对不上的那一侧各有一条**已经拍过板**的规矩在实现这边：

1. **环节横条差一格**（`envelope.120x36` 两格、`envelope.80x24` 一格，`Expected::cell_like`）：
   设计稿那一头的模拟走**连续时间**（`灰原哀/第05卷` 的 `done` 是 114.554，半页也占横条一格），
   而这一趟**一页一步**——屏上那个数两边都是 114，横条差一格。停车场 **Q844**。
2. **说明卡正文折下来的两行往右推四格**（`Expected::shifted`）：设计稿的 `wrap()` 不带悬挂缩进，
   而 `CONTEXT.md` 的《折行》明写着「会话的详情栏、**说明卡**与预设栏里的说明共用这一套」
   「屏上没有第二套折行规矩」，那一套**行首缩进跟着折下来的每一行走**（Q32／Q114）。
   两套算法比过一遍：**除了这一截缩进，断在哪儿、折出几行逐行相同**。停车场 **Q845**。

**两手都不是放过。** `cell_like` 换完那一格仍要求实现写下的与它一致；`shifted` 推开之前
先断言被挤掉的那几格本来就是空白，**一个字都没丢**。要收干净只有一条路——按 ADR 0019
决定第 13 条先改设计稿、重新导出，那是拍板的人的事。与 08 的 Q807 是同一类的第二、第三处。

**另有一串不比屏**：`ended-q`。那一屏的屏底写着「退出（原型里不会真的退出）」——设计稿自己
点明那是原型的话，真程序在这一档退出、画不出下一帧。照 `fresh-q` 那条已经判过的先例
（**Q774**）断的是那一支交出 `Exit::Leave`。记在 **Q847**。

### 「屏上不摆按不动的键」在屏底也立住了

屏底那一件「展开／每页结果／查看」随**光标那一行**换（设计稿 `openHint`）：目录行 `l → 展开`、
卷行 `l → 每页结果`、备注行 `⏎ → 查看`，而**卷行展不开就一件都不摆**。末一问要问那一趟
「这一卷此刻怎么样」——`Session::hints` 因此多收一个 `Option<&Live>`，判据落在
`VolumeState::opens_the_pages` 一处：**屏底摆不摆它与按下去换不换屏读的是同一份**。
从前那两处各有一份名单（`open_a_volume` 里一个 `match` 的四个分支），而屏底那一句压根没摆。

### 序列：哪几串归这一票

**走完并逐格相等的十八串**：`ended-note`／`-Enter`／`-Enter-Escape`、`ended-nonvolume-Enter`
（打了折）、`ended-o`、`ended-help`、`ended-G`／`-G-gg`、`ended-C-d`／`-C-d-C-u`／
`ended-C-f`／`-C-f-C-b`、`ended-h`／`-h-l`／`-h-Enter`、`ended-failed-l`、
`running-s-advance`；`ended-q` 只断 `Exit::Leave`、不比屏（Q847）。

**票面把 `ended-h*` 与 `ended-failed-l` 划给了 11，那是划错的**（票面标的是 `predicted`，
而它自己说「真正的出处是 `sequences/` 这个目录，按它判」）：那五串走完停在**卷列表**上、
**不换屏进每页结果**——`ended-h*` 三串问的正是这一票的屏底那一件（`l → 展开` ⇄
`l → 每页结果`）与 08 的 `h` 收起，`ended-failed-l` 问的是票面自己那一句「转换失败的卷…
行尾是那句原因」。

> **给 11 号票：`ended-h`、`ended-h-l`、`ended-h-Enter`、`ended-failed-l` 这四串归 10，
> 已经走完并逐格相等（`terminal` 的 `h_collapses_the_directory_of_the_volume_and_l_opens_it_again`
> 与 `a_failed_volume_says_the_reason_from_its_own_row`），不要重做。**
> 11 那边的是 `ended-l`、`ended-l-a`、`ended-l-a-j`、`ended-l-a-j-h`、`ended-Enter`、
> `ended-skipped-l`——那六串**都要先换屏进每页结果**（末一串 `ended-l-a-j-h` 再按 `h`
> 回卷列表，回得对不对同样要那一屏先在）。

第五串 `ended-h-Enter-Enter` 要 `⏎` 在展开着的目录行上**收起**它，而表上 `l` 与 `⏎` 同义
——那是 **Q806**，它当初判的依据是「没有一串序列踩到」，而这一串从**键盘**那一头踩到了：
记进 **Q853**（Q806 的续），归 16。

### 没做的（按票面归别的票）

每页结果换屏（11——`ended-Enter` 的 `says` 是 `[需留意的页]`，**那一串归 11，不是说明卡**；
`ended-l`／`ended-l-a*`／`ended-skipped-l` 同归）、`]d`／`[d` 与搜索（09，连同 `ended-]d`／
`ended-[d`／`ended-search-*`）、确认条（12）、配置视图那几屏（13／14）、鼠标（16）、
真会话切过来（15）。

**半屏与一屏那四个键**（`C-d`／`C-u`／`C-f`／`C-b`）这一票做了，而它们**只在卷列表上挪**
（`Session::scroll_list` 头一道守卫）：每页结果与配置视图接上之后各自那一块的滚动归 11／13
——那四个键在表上派给没被盖着的每一块，认下来却什么都不做的话，按下去会是一片静默。
全部按键那一张上它们挪一整屏，这一票接了（设计稿覆盖层那一支）。

### 数

`cargo fmt --check` 过之后跑的那一趟就是最终状态：`cargo xtask gate` 三条全绿（`EXIT=0`）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1092 通过 0 失败；lib 238 / bin 513；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 886 通过 0 失败；lib 238 / bin 307；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 31.70s` |

上一趟（08 收尾）是 1074 / 880：**闸门 1 多 18 条，闸门 2 多 6 条，lib 一条没动**
（238 → 238：这一票没往库里加用例，`render` 那几处改动读的是既有那几条，外加自己那一条新的）。

- 闸门 1 那 18 条：`shell` 2（已结束两屏 · 整卷统一灰阶两屏）、`columns` 1（默认逐页那一趟
  代表页整列不在场）、`cover` 2（说明卡的全文出自报告那一处 · 卡居中且高随正文）、
  `view` 1（半屏与一屏各挪几行）、`scene` 1（推进那一步换的是那一趟、不是界面状态）、
  `render` 1（归档卷去掉扩展名、页名不动）、`terminal` 10（滚动六串 · 说明卡三串 ·
  非漫画文件那一张 · `h` 收起三串 · 没做成那一卷说原因 · 结束之后 `o` ·
  结束了那一档的全部按键 · 结束了 `q` 退出 · `o`／`i`／`t`／`x` 各派什么 · `s` 一次加推进）。
- 闸门 2 那 6 条：特性外面那几个模块自带的（`cover` 2、`view` 1、`columns` 1、`scene` 1、
  `render` 1）——`shell` 与 `terminal` 那 12 条在 `tui` 后面，那一趟不编。
- `tree` 那一条是**扩了既有的那一条**（备注行挂位那一条上补 `said` 与 `locate_note`），
  因此不进上面的加数。

黄金快照 `tests/golden-snapshot.txt` 原样过；**设计快照与序列一格没动**
（`git diff --stat -- tests/fixtures/design` 为空）。

`cargo xtask polish` 四条全绿（`EXIT=0`），**按告警条数读的**：`cargo fmt --check` 过；
clippy 两趟各 `Finished`、**各零告警**（默认那一趟 6.64s、甩掉终端库那一趟 16.35s，
两段里一个 `warning` 字都没有）；`cargo doc --no-deps` **15 条告警**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 06／07／08 同数，一条没多）。

### review 之后改的

标准轴与规范轴各一批，逐条：

- **`pub type Said = (PathBuf, String)` 拿掉了**：那是一个新开的公共类型名而
  `CONTEXT.md` 上没有它（`CLAUDE.md`《写代码前》「类型名一律取自它」）——而它也不该进词汇表：
  那不是一个领域概念，是那两小结逐条的**渲染形状**，两族东西自己的类型是库上的
  `NonVolumeFile` 与 `UnreachablePlace`。两处 `*_stack` 因此收 `&[(PathBuf, String)]`，
  逐条那一对在文档里写明。
- **滚动条那两句文档**：`viewport.rs` 的 `Scrollbar` 说着「本仓库不自己画一条」、
  `CONTEXT.md` 的《视口》末一句说着「走终端库自带的那个 widget」。前一句是**代码文档**、
  说的是实现，改了（《文档写作》第 1 条）；后一句是**词汇表**，按《改 CONTEXT.md 的规矩》
  记进 **Q850**、没有顺手改。两副界面过渡期有两处滑块画法，与 Q804 同一笔代价，写进了
  两处文档。
- **`Card` 带着 `NoteKind`，不折成 `bad: bool`**：「是哪一种」在树上已经有名字，
  折一次画法那一头就得再认回来（原先还多一个 `look_of_label(bad: bool)` 的旗标参数）。
- **走一遍全部备注只有一处**：`Tree::locate(is_it)`；`locate_note`（按那一条的身份问）
  与场景夹具的 `stand_on_a_note`（按「是哪几处」问）都读它。
- **`Session::scroll_list` 加了头一道守卫**（不在卷列表上就一件都不认）：那四个键在表上
  派给没被盖着的每一块，认下来却什么都不做的话，11／13 接上之后按下去是一片静默。
  同一道守卫收成 `Session::on_the_volume_list`，`place_cursor` 也读它。
- **`Step::Advance` 当场断言它是最后一步**：夹具摆得出的只有「走完那一刻」，
  推进之后还有输入的那三串（09／12）拿它当中间态是错的——**Q852**。
- **`canvas::scrollbar` 的文档补上头一道守卫**（共几行不多过露出几行就一格都不画）：
  原先写着「两条式子」而代码有三条。
- **`Kind::Prose` 的文档改成只说当前事实**：眼下只有说明卡的全文读它；详情栏与预设栏
  那两块是设计稿的同一色，随 13／14 接进来。
- **`render::volume_name` 补了一条钉住它的用例**：旧那几条用例一格都没钉住归档卷名
  （改之前也不会红），Q849 跟着写清这一点，连同 `.partN` 那一截仍对不上。
- **`ended-h`／`-h-l`／`-h-Enter`／`ended-failed-l` 四串接上了**（票面把它们划给了 11，
  划错了，见上一节）。

**没改的两条**：① `non_volume_stack` 与 `unreachable_stack` 两个函数体只差抬头与量词、
条目那一行逐字相同——**这一票没有新添这份重复**：那一行（`  路径\n    原因\n`）改之前就在
`non_volume_tail`、`unreachable_tail` 与 `failed_volume_tail` 三处各写一遍，这一票只是把
前两处搬进了 `*_stack`，份数一格没变；而 `crate::listing` 的模块文档明写着「条目怎么渲染
**不共用**，是一个闭包参数」，收成一份是那一处的决定，不是这一票的。② `Cursor` 上那两处
`match`（`open_under_cursor` 做事、`open_want` 说屏底怎么写）：两处答的是两个问题，
真要合得造一个「这一行展得开什么」的中间值，而 `open_under_cursor` 还要那一行的载荷。

### 票面有没有说全根因

**「一趟跑完之后在新界面上看结果」这件事，除了下面这一样，好了。** 仍活着的是
**每页结果那一屏**（11）：结束之后卷行上按 `l` 眼下**原地不动、一句话都不说**——
屏底摆着 `[l → 每页结果]`（这一票让它摆得对了），按下去却什么都不发生。
那不是这一票漏了，是 11 号票那一屏还没有；而这一票把「按得动就摆、按不动就不摆」
那条规矩立在了同一份判据上（`VolumeState::opens_the_pages`），11 接上换屏那一下即可，
屏底一个字都不必再动。

**这一票记下的折扣与岔路，一条都不是活口**（十条，Q844–Q853）：

| | 是什么 | 要谁收 |
|---|---|---|
| Q844 | 环节横条那三格（设计稿走连续时间） | 先改设计稿、重导 |
| Q845 | 说明卡正文那两行的悬挂缩进 | 先改设计稿、重导 |
| Q846 | `views_of` 认光标那两处读错（08 的缺陷，已修）；还缺一条「认不出就出声」的用例 | 15 或 `/settle` |
| Q847 | `ended-q` 不比屏（照 Q774 的先例） | 与 Q774 同批 |
| Q848 | 设计稿说明卡上 `⏎` 也关卡（没有夹具按得到） | 协调人 |
| Q849 | `volume_name` 动的是共用件；`.partN` 那一截仍对不上 | 协调人 / 15 |
| Q850 | 滑块改成自己算，《视口》末一句没跟着改；过渡期两处 | 拍板的人 / 15 |
| Q851 | `o` 回到开跑之前把阶段退回 `Fresh`，旧界面那一维跟着退 | 15 收摊时核一遍 |
| Q852 | 序列里「推进几秒」只摆得出走完那一刻 | 09 / 12 |
| Q853 | `ended-h-Enter-Enter` 要 `⏎` 收起（撞上 Q806） | 16 |
