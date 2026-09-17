# 09 — 自动滚动、问题跳转与搜索

**What to build:** 大库挂着跑时的看法与找法（spec《状态》自动滚动、《卷列表》跳转）。

- **自动滚动**两档：**跟着**正在处理的那一卷（它的目录收着就停在目录行上，只滚不展），**暂停**。按键挪光标、
  `]d`／`[d`、搜索跳过去即暂停，框右端换成「已暂停自动滚动 ⋅ F 恢复」，屏底提 `F`；`F` 交回；每次开跑扳回跟着。
  它记的是光标，视口照旧由光标算；
- `]d`／`[d` 在转换失败的卷、进了隔离的卷、有需留意的页的卷、无法访问的地方之间跳；
- `/` 搜卷名或目录名（清点完之后才派），匹配处加下划线，`⏎` 跳到第一个，`n`／`N` 在结果之间跳，收着的目录自动展开到那一卷；
- 全部按键在转换中那一副（设计稿「全部按键」场景）。

滚轮与单击挪光标也要暂停自动滚动，那一半在 16。真会话仍进旧界面。

**Blocked by:** 07 — 输入行、补全框与全部按键；08 — 清点中与转换中

**Status:** resolved

- [x] 「搜索」「全部按键」各 120×36、80×24 逐格相等
- [x] 序列：`j` 暂停 → 推进几秒光标不动 → `F` 跟回；`]d` 连跳两次、`[d` 回跳；`/` → `⏎` → `n` → `N`（跳进收着的目录）；走完与期望屏相等
- [x] 用例：自动滚动暂停时只记光标，视口由光标算
- [x] 真会话仍进旧界面，旧用例照绿；设计快照未改

## 落地记录

**做了什么。** 自动滚动那两档连同它的三句回话、`]d`／`[d`、`/` 搜索那一整套在测试里长出来了；
真会话仍进旧界面（`src/main.rs` 的 `session::enter` 一字没动），旧的那一副与旧用例原样留着、照编照过。
**四屏逐格、24 串序列、八条纯逻辑用例**：

- **跳转与搜索**（`session::view`，`tui` 特性**外面**，闸门 2 照编照测）：`Hunt` 两种落点共用一套挑法
  （`Session::hunt`）——**次序按树上全部目录摊开那一副数**（`tree::Tree::every_row`），不按屏上此刻摆着的行：
  收着的目录里那几卷照样跳得到，跳过去才把那个目录展开。光标那一行**本身不算「下一个」**（两个方向都用严格
  不等号），一个都没有就绕回头一个。`]d`／`[d` 的落点是转换失败的卷 · 进了隔离的卷 · 有需留意的页的卷 ·
  无法访问的地方——前三种问那一趟（`Live::troubled_at`），末一种是树上的备注行。
  **搜索命中与搜索落点是两件事**：目录名自己就装着这一句时，它底下那几卷不再各算一个落点
  （搜「海贼」跳到那个目录行一次，不是十八卷各一次），而那十八行照旧加下划线。
- **此刻搜的是哪一句只有一处**（`Views::searching`）：搜索那一行开着时**就是它的缓冲**——打一个字、
  退一个字，下划线与框底边那一截当场跟着动，中间不存第二份；关掉之后是 `⏎` 定下来的那一句
  （`TaskView::search`，每次开跑清掉）。**空串不算在搜也判在这一处**（设计稿那几处问的都是
  `S.search && S.search.q`，两件事一个条件）。「那一行开着吗」是另一问，问 `Session::searching_line`。
- **画面**：`shell::draw` 逐格对 `search.120x36`／`search.80x24`／`help.120x36`／`help.80x24`。
  匹配的那一行**名字那一列加下划线**（树上三种行与开跑之前那一副四处共用 `list::name_look`，
  比的那一截字由树答：`Tree::searched_text`）；框底边左起那一截 `/海贼 ⋅ n N 跳到下一个 / 上一个`
  （黄斜体加灰斜体）；屏底那一件 `F` **只在暂停着时摆**。
- **交互**：`terminal::input` 又接了四支够得着那一趟的事——`F`（扳回那一格之后**紧接着再盯一眼**
  `watch_the_run`，光标这一帧就跟上）、搜索那一行上的 `⏎`、`]d`／`[d`、`n`／`N`。落点要问那一趟
  （哪几卷出了事），而状态机读不到它，与 08 立的 `Deed::Open` 落在卷行上那一支同一条分工。
- **07 留给本票那六串**（`help-*`）连同「全部按键」那两屏都接上了：那一景的底下是**转换中**那一副，
  07 摆不出来（那棵树归 08）。07 说的另两样（覆盖层上 `C-d`／`C-f`）它自己已经做完，本票只添用例。

### 自动滚动那一格：上游那半截写错了，而它没有声音

**08 票面之外提前做的「按键挪光标即暂停」判错了条件**：它写的是「光标**真挪了**才暂停」
（`place_cursor` 里 `if there != cursor`），而设计稿 `listGo` 那一支**不问光标有没有真挪**——
扳那一格与说那一句都在挪动之后无条件做。

**后果是一句没有声音的错**：自动滚动跟着的那一卷正停在列表两头时按 `j`（或到顶按 `k`、到底按 `G`），
光标挪不动 → 不暂停 → **下一帧 `watch_the_run` 把人拽回正在处理的那一卷**，而躲开那一下正是按这个键的
用意。屏上看着像「自动滚动在工作」，没有一个字说出「你那一下被吃掉了」。

**怎么发现的：读设计稿，不是跑用例。** 一条夹具都照不到它——24 串序列里挪光标那几串（`running-j`、
10 号票那六串滚动）落点都挪得动，`ended` 那几串阶段已结束、本来就不出声。这一条是把
`design.html` 的 `listGo` 与 `place_cursor` 并排逐句对出来的，`/code-review` 规范轴那一遍。
**下一个人照这个办法还找得出来**：设计稿那几个 `function` 是唯一说得出「这一下该不该出声」的地方。

修法：`place_cursor` 里那一下改成无条件 `pause_follow(now)`，两道闸仍在 `pause_follow` 自己身上
（跟得上东西的那两档、而且此刻真跟着）。补上专钉它的那一条——
`a_keypress_that_cannot_move_the_cursor_still_pauses_following`（光标摆到最后一行、按 `j`、
核光标没动而那一格停了、屏底说了那一句）。

### 一处验收打了折：Q844 那条已知的一格差

「搜索」那一景与「整卷统一灰阶」**同在 62%、当前卷同是 `灰原哀/第05卷`**，踩的是同一条根因：
设计稿那一头的模拟走**连续时间**（`done` 是 114.554，半页也占一格），而这一趟**一页一步**、走到的是
114/166——24 格那条横条满 16 不满 17，8 格那条满 5 不满 6。

照 10 号票在 `envelope` 上的同一手 `Expected::cell_like`（把那一格换成它右边那一格）：
`search.120x36` 与那两串各换两格（第 3 行第 46 格、第 16 行第 103 格），`search.80x24` 换一格
（第 14 行第 57 格）——**那一格与 10 号票 `envelope` 80×24 那一格逐字同坐标 `(14, 57)`**，
同一个根因、同一处坐标，**是同一笔账**（`/settle` 读 Q844 时会需要这一句）。
**换掉之后仍是一条断言**：实现在那一格上写别的照样红，左邻右舍照旧严格比。设计夹具一个字节没动
（`git diff --stat -- tests/fixtures/design` 为空）。

### 新添与改动的名单

| 哪儿 | 什么 |
|---|---|
| `session::view`（特性外） | `Hunt`／`Session::hunt`／`jump`／`confirm_search`／`searching_line`；`Views::searching`（此刻搜的是哪一句，一处答完）；`TaskView::search`；`pause_follow` 加那一句回话与 `following_matters`；`Deed::Follow` 那一句、`Deed::Search`、`Deed::ClearSearch`；屏底那两处（搜索那一行右端两件、`F` 只在暂停时摆）；`JUMP_LINGERS`／`FOLLOW_LINGERS` |
| `session::tree`（特性外） | `every_row`（全部目录摊开那一副，跳转按它数次序）、`every_directory`（私有）、`searched_text`（一行拿哪一截字给搜索比） |
| `session::live`（特性外） | `report_at`／`notable_at`／`troubled_at` 与 `NotableTally`：**「需留意几页」那一份判定从画法那一层搬了过来**（Q867），`list.rs` 原处那二十行连同 `panel` 那一格一起删掉 |
| `session::typing`（特性外） | `Purpose::Search`（提示词 `/`、不补全）；`cancel_typed` 连那一句一起丢；`confirm_typed` 把搜索那一种让给 `confirm_search` |
| `session::keymap`（特性外） | `Deed::ClearSearch` 一行（`Esc`／清点之后／卷列表，两句都空：屏底与全部按键都不摆它，Q869） |
| `session::shell::list` | 匹配处的下划线（`name_look` 一处，四处共用）、框底边那一截（`searching_chip`）、`Pointed`（行首记号 · 是不是光标那一行 · 匹配上没有，三样一路走） |
| `session::scene` | `views_of` 认 `session.search.query` 与 `input.kind == "search"` |
| `session::terminal` | `input` 接四支；测试里 `walked` 那条断言换法（Q865）、`assert_sequence_with`（四个 `assert_sequence*` 的同一份身子收成一处） |
| `CONTEXT.md` | 《卷列表》里 `/` 那一句补上**搜索 (Search)** 这个名字——不补，「类型名一律取自它」对 `Purpose::Search`／`TaskView::search` 执行不了 |

### 代价

三处：① Q844 那处折扣（上面那一节）；② 「需留意几页」那一份判定从此住在 `Live` 上，
而那个模块答的本是「事件流折成几个数」——两个界面层（一个在 `tui` 后面、一个在外面）都要问它，
`Live` 是唯一两边都够得着的地方（Q867）；③ 搜索那一行上 `C-w` **按得动而屏底不摆它**
（照设计稿逐格；Q868，与 Q794 是同一处「屏底与按键表对不上」）。

### review 之后改的

`/code-review` 两轴（标准 · 规范）各一遍，**接了九条、驳回两条**。

标准轴那几条：① **`Live` 上那一份不再带屏上的词**——头一版把四个词一起搬了过去，而
`troubled_at` 为了答一个 `bool` 要先拼一串词，且 `live.rs` 模块文档头一句写着「画成什么样是 `draw` 的事」，
一搬就不成立；改成 `NotableTally`（四个具名计数加 `pages()`／`any()`），**数在 `Live`、词在 `list.rs` 一处**，
模块文档一个字都不用改——回头去读设计稿，`problemsOf(v)` 数、`drawRow` 造词，**它本来就是这么分的**
（Q867 已按真实落地重写，不留那句已经不成立的推荐）；② 「空串不算在搜」从四处收进 `Views::searching()` 一处，
`confirm_search` 不再先存一个空串；③ `confirm_typed` 改调 `searching_line()`，不再逐字重写那一句；
④ 「加粗＋下划线」三份收成 `list::name_look`；⑤ `(cursor, at_cursor, matched)` 打成 `Pointed`
（照 08 立的 `Spot` 那条理由）；⑥ 四个 `assert_sequence*` 的同一份身子收成 `assert_sequence_with`
（收 `impl FnOnce(Expected) -> Expected`）；⑦ `Tree::every_directory` 收成私有（唯一调用者在同一个 `impl` 里）；
⑧ `CONTEXT.md` 补上**搜索 (Search)** 那个英文名。
规范轴那一条：⑨ 上面《自动滚动那一格》整节。

**驳回两条，理由在这里：**

- **「`hunt` 该走 `pause_follow` 那两道闸」——不。** 设计稿 `reveal` 与 `jump` 都是**无条件**扳那一格；
  而那两道闸的用处是**不让第二句回话抢掉「问题 4/4」那一行**（两句抢同一行，说出口的只能是人刚按下那件事）。
  结束之后 `follow` 留假**没有外显**：框右端那一枚只在转换中与等待确认两档画（`list::follow_chip`）、
  `watch_the_run` 遇上 `live.ended()` 先返回、下次开跑 `TaskView::start_a_run` 扳回开着。
- **「`following_matters` 读 `surveyed` 而 `follow_chip` 读 `Phase`，同一条界两份数据」——不是两份。**
  `TaskView::surveyed` 存在的理由**正是**让会话不问那一趟也答得出「清点完了没有」（08 评审第 ⑫ 条立的：
  原先拿「树上几卷 vs 清单上几卷」比，一卷都没清点出来的那一趟会永远停在开跑之前那一副），
  而 `pause_follow` 跑在状态机里、手上没有 `live`。两处问的是同一件事，只是一处够得着那一趟、一处够不着。

另外评审建议把 `Hunt` 也加进词汇表，**驳回**：`Want`／`Hint`／`Chord`／`Deed`（07）、`Spot`／`LinedRow`／
`BranchTally`（08）一个都不在词汇表里——那张表管**领域概念**，不管一个两变体的私有枚举。
补**搜索 (Search)** 是另一回事：那是给一个已经写在词条里的概念补名字，照《改 CONTEXT.md 的规矩》
「新词可以当场加」当场加的（13 在《组 (Band)》《画质判定参数 (Judging)》上做过同一件事）。

### 票面有没有说全根因

**验收四框全勾之后，「大库挂着跑时的看法与找法」仍缺两样**，两样都在别的票里：

- **看法那一半**：`]d` 把人送到出事那一卷跟前之后，**按下去还是原地不动**——每页结果那一屏归 11。
  而「到底哪一页把整卷拉下来」正是进一卷的唯一目的（`CONTEXT.md` 的《需留意的页》自己这么写着）：
  跳得到、看不进去，这一票交出来的是半条路。
- **找法那一半**：**滚轮与单击挪光标仍不暂停自动滚动**（归 16）。触控板用户一滚就被下一帧拽回去
  ——与本票修掉的那个上游 bug 是**同一个形状、同一句没有声音的错**，而 spec《卡顿的根因》点名的
  大库用法正是「触控板连续滚动」。16 号票接那一支时读一眼本票的 `pause_follow`：那一处已经备好，
  只差把滚轮与单击那一路接上去。

另外两处折扣仍活着，收干净都要先改设计稿、重新导出（ADR 0019 决定第 13 条，拍板的人的事）：
Q844（横条那一格，本票与 10 号票同一笔账）与 Q807（清点中那两屏输出目录行行尾十格，08 号票记的）。

### 没做的（按票面归别的票）

滚轮与单击挪光标之后暂停，连同 `running-wheel-*`／`running-click-row`／`running-dblclick-dir`（16）；
`⏎` 在展开着的目录行上收起（Q806／Q853，16）；每页结果换屏与 `ended-l*`／`ended-Enter`（11）；
确认条与 `deciding-*`（12）；预设栏（14）；真会话切过来（15）。

### 数

review 收完、改完之后跑的**同一趟**（`cargo xtask gate` 接 `cargo xtask polish`，一条命令连着跑完）
就是最终状态：三条闸门全绿（`GATE=0`），polish 四条全绿（`POLISH=0`）。

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1124 通过 0 失败；lib 238 / bin 545；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 897 通过 0 失败；lib 238 / bin 318；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 36.18s` |

上一趟（08 收尾之后、10 与 13 并进来的 `main` 顶端 `f0af011`）是 **1103 / 889**：
**闸门 1 多 21 条，闸门 2 多 8 条，lib 一条没动**（238 → 238：这一票一行库代码都没加用例）。

- 闸门 1 那 21 条：`shell` 2（「搜索」与「全部按键」各宽窄两屏）、`terminal::redesign` 11
  （挪光标暂停 · 推进几秒光标不动 · `F` 跟回 · `]d` 两串加 `[d` · 结束之后 `]d`／`[d` ·
  `/` 开那一行 · `⏎` 与 `n`／`N` · `Esc` 丢掉那一句 · 搜进收着的目录 · 一个都没找到 ·
  `help-*` 六串）、`view` 8（下一条）。
- 闸门 2 那 8 条：`view` 自带的那八条——自动滚动暂停只记光标／视口由光标算 · 挪不动也暂停 ·
  开跑扳回跟着并清掉上一句 · 清点中挪光标不出声 · 搜索的落点与自动展开 · 一个都没找到时各说一句 ·
  搜索那一行右端只两件 · `F` 只在暂停时上屏底。**跳转与搜索整套都在特性外面**，这一趟连它们一起跑。

`cargo xtask polish` 四条全绿：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check` | 过 |
| 2 | `cargo clippy --all-targets` | `Finished`，**零告警** |
| 3 | `cargo clippy --all-targets --no-default-features` | `Finished`，**零告警** |
| 4 | `cargo doc --no-deps` | `warning: \`tonefit\` (lib doc) generated 15 warnings`——**仍是 15 条，一条没多**（与 06／07／08 同数；那 15 条全是库里公共文档指着私有项的老账，本票几十条 doc 链接一条新告警都没加） |

黄金快照 `tests/golden-snapshot.txt` 原样过；`tests/single_source.rs` 那一族四条全过；
**设计快照与序列一格没动**（`git diff --stat -- tests/fixtures/design` 为空）。
