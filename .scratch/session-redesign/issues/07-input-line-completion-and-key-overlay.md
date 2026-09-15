# 07 — 输入行、补全框与全部按键

**What to build:** 新界面上打字与看帮助（spec《输入行与路径》《按键表、屏底与覆盖层》；停车场 Q715–Q717 的结论）。

- **输入行**占屏底：提示词 · 缓冲 · 光标，右端 `Tab` `C-w` `⏎` `Esc`。`o` 添加处理路径，`i`／`⏎` 修改一条，
  输出目录那一行上修改输出目录；
- 输入行认 `~/` 开头的路径；逐层补全，候选多于一个时屏底上方弹出**补全框**（能滚、不把屏底撑高），`Tab` 轮到下一个；
  `C-w` 删一段；找不到的路径当场说；
- **全部按键**：`?` 掀开，按用途分组，**只列此刻阶段派得出的键**，同一个键按阶段写它此刻做的事（`q` 跑着时说不退出）；
  宽时两栏；`j`／`k` 滚、底边说看到第几行；`?`、`Esc`、`q` 关掉；底下整屏压暗；掀着时除了停止与答话别的键不派；
- 打字时 `F1` 掀开全部按键，关掉之后输入行、缓冲与补全框原样回来。

真会话仍进旧界面。

**Blocked by:** 06 — 新会话的骨架与开跑之前那一副

**Status:** ready-for-agent

- [ ] 「添加路径」120×36、80×24 逐格相等
- [ ] 序列：`o` → 补全框 → `Tab` 轮换 → `C-w` → 打一个找不到的路径；`i` 修改一条处理路径；改输出目录；走完与期望屏相等
- [ ] 序列：打字时 `F1` → `j`／`k` → `Esc` 回到输入行；还没开始时 `?`；走完与期望屏相等
- [ ] 用例：全部按键与屏底出自同一张按键表（表里加一个键，两处都跟着出现）
- [ ] 真会话仍进旧界面，旧用例照绿；设计快照未改

## 落地记录

### 停车场结转

### Q774 — 序列 `fresh-q` 的期望屏是原型自己的话：屏底「退出（原型里不会真的退出）」，实现上 `q` 交出退出后不再画下一帧

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（原型无法真退出，只能印一句）
- **Where:** `design.html` 的 `onKey`（`q` 那一支的 `toast`）；`tests/fixtures/design/sequences/fresh-q.*`；`src/session/terminal.rs` 的 `redesign::q_before_the_run_hands_out_the_exit`
- **Why it did not block:** 票面写的验收是「还没开始时 `q` 交出退出」，那一支回 `Exit::Leave` 就够；期望屏上那句话是原型替代不了退出才印的，实现画它就是在屏上说一句假话
- **What this ticket actually did:** 用例只断言那一支交出退出，不拿走完那一屏比 `fresh-q`；那份期望屏留在盘上、清单里照旧有它（`every_exported_snapshot_scene_and_sequence_reads_back` 照读）
- **Options:** ① 照旧：这一串的用例断言去留，不比屏；② 设计稿里 `q` 退得出的那两档改成什么都不印、重导这一串，用例比「按了 `q` 之前那一屏」；③ 实现照印那句话——不行，屏上会有一句假话
- **Recommend:** ①——多一份永远比不上的期望屏没有坏处，改设计稿是为了一条不比屏的用例
- **Whose call:** 拍板的人（动设计稿才是 ②）
- **处置：** 待处理。

### Q775 — 设计稿的视口是记着的（`S.from` 粘着、上下各留一行余量），`CONTEXT.md`《视口》与 `session::viewport::Viewport` 是从光标算出来的、不留余量——长列表上光标往回挪时两边画出来的起点不同

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与词汇表对不上（spec《状态》明说《视口》不改）
- **Where:** `design.html` 的 `viewport()`（`S.from[name]` 粘着，`margin = h > 8 ? 1 : 0`）；`src/session/viewport.rs` 的 `Viewport::new`（`from = cursor + 1 - height`，光标本来在格子里就从头画起）；`CONTEXT.md`《视口》「滚动量是算出来的，不是记着的」
- **Why it did not block:** 本票只画开跑之前那一副，15 行摆在 34 行里，两边起点都是零；撞上的是树那一票（`running` 那一景八十多卷）与每页结果那一票——光标从底下往上挪一行时设计稿不动、`Viewport` 立刻跳回从头画
- **What this ticket actually did:** 卷列表照《视口》用 `Viewport`；记下
- **Options:** ① 设计稿的视口改成从光标算（带那一行余量的话 `Viewport` 也加余量，两边同一个式子）并重导有滚动的那几串；② `Viewport` 记一个 `from`——改《视口》那一句，是领域决定；③ 各留各的，滚动的序列一律不比屏——不行，那正是要验的东西
- **Recommend:** ①，余量那一行随它一起定（`CONTEXT.md`《视口》本来就写着「格子高过 8 行时上下各留一行余量」，`Viewport::new` 今天没留）
- **Whose call:** 拍板的人（一边动设计稿、一边动 `Viewport` 的算法）
- **处置：** 待处理。

### Q776 — 从中间省略的截法两边不同：设计稿 `elide` 的尾巴预算是定死的 `w-1-left`，`columns::elide` 把头上没用完的那一格还给尾巴

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（潜在，本票没撞上）
- **Where:** `design.html` 的 `elide`；`src/session/columns.rs` 的 `elide`（「头上没用完的那几格还给尾巴」那一段）；新界面开跑之前那一副的路径那一列读的就是 `columns::elide`（`src/session/shell/list.rs`）
- **Why it did not block:** 一个宽字符跨在头上的预算边界上时才分得出：`w=8`、`a` 后面跟十个汉字，设计稿截成 `a汉⋯汉`（六格），实现截成 `a汉⋯汉汉`（八格）。11 个场景与本票认领的序列里没有一条路径或卷名要省略
- **What this ticket actually did:** 路径那一列照《让位》「截法只有一处出处」用 `columns::elide`；记下
- **Options:** ① 设计稿改成实现的截法并重导有省略的那几屏；② 实现改成设计稿的截法——旧界面的快照跟着变，而那一副本来就要退场；③ 各留各的——撞上那一格就红
- **Recommend:** ①：实现那一格填得更满，`CONTEXT.md`《砍列》「两头留着」要的正是它
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q777 — 窗口太小那一屏在哪一档上都写 `[q → 退出]`，而跑着与等待确认时 `q` 不退（《退出会话》）

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与词汇表对不上（spec 的 story 84 也写着「与 `q`」）
- **Where:** `design.html` 的 `drawTooSmall`（恒 `hint('q', '退出')`）；`tests/fixtures/design/snapshots/running.56x14.*`；`CONTEXT.md`《退出会话》《让位》；`src/session/shell/small.rs`
- **Why it did not block:** 本票只比还没开始那一份（`fresh.56x14`），那一档上 `q` 真退；跑着那一份归总览那一票
- **What this ticket actually did:** 那一句从按键表还没开始那一档的 `q` 行取写法与那一句，不随阶段换——照设计稿；记下
- **Options:** ① 设计稿跑着与等待确认时写 `[C-c → 退出]`（或 `[s → 停止]`），重导 `running.56x14`，实现按阶段从表上取；② 照旧，屏上写着 `q` 而按了没反应（窗口太小那一屏也没有屏底说为什么）
- **Recommend:** ①——屏上不摆按不动的键，是本仓库一贯的规矩
- **Whose call:** 拍板的人（动设计稿与 story 84 的措辞）
- **处置：** 待处理。

### Q778 — 等待确认时配置视图的屏底摆着 `[v → 每页结果]`，而 `v` 只在任务视图里派得出

- **From:** 票 `session-redesign/06`
- **Kind:** 路过发现的设计稿缺陷
- **Where:** `design.html` 的 `stageHints`（`S.pages ? [] : [hint('v', '每页结果')]`，配置视图也走它）与 `onKey`（`v` 只在 `taskKey` 里认）；`src/session/keymap.rs` 的 `ViewPages` 那一行只在卷列表上派
- **Why it did not block:** 11 个场景里没有「配置视图 × 等待确认」那一屏；按键表照《焦点》只在卷列表上派 `v`，屏底照表不摆——真导出那一屏时那一格会对不上
- **What this ticket actually did:** 记下
- **Options:** ① 设计稿的配置视图屏底不摆 `v`；② `v` 在配置视图也派得出——切回任务视图并进那一卷的每页结果，表上那一行放宽到配置那几块
- **Recommend:** ①——答话那三个键在哪一块上都按得动是《焦点》定的，`v` 不是答话
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q779 — 连击键的前半截按表待着，设计稿按写死的几个字待着：`d` 在光标不停在处理路径上时一边待一边不待，`]`／`[` 在还没开始时正相反

- **From:** 票 `session-redesign/06`
- **Kind:** 设计稿与实现对不上（潜在，本票认领的序列没撞上）
- **Where:** `design.html` 的 `onKey`（`k === 'g' || k === ']' || k === '[' || (k === 'd' && canDelete())` 才 `pending`，不问阶段）；`src/session/view.rs` 的 `Session::deed_of`（`keymap::starts_a_combo`：这一档、这一块上表里有以它开头的连击键才待）
- **Why it did not block:** 两个方向各差一下：光标停在输出目录或「＋ 添加路径」上按 `d`，实现右端多一个 `d…`、`dd` 落到空处，设计稿什么都不做；还没开始时按 `]` 或 `[`，设计稿右端留 `]…`，实现不待（表上 `]d`／`[d` 只在清点完之后派）。`fresh-d` 与 `fresh-dd` 的光标都停在处理路径上，两边逐格相等；`]`／`[` 在还没开始时没有一串序列按过。屏底那一件 `[dd → 删除]` 设计稿本来也不随光标换
- **What this ticket actually did:** 一律按表待着，不问光标那一行是什么、不写死哪几个字；记下
- **Options:** ① 设计稿改成与表一致：`d` 一律待着、`]`／`[` 只在清点完之后待着，重导——本票认领的序列一格不变；② 实现照设计稿写死那几个字外加问光标那一行——按键表之外多一维、多一份名单
- **Recommend:** ①（屏上摆着 `[dd → 删除]` 按下去就该待着；`]d` 派不出的档上留一个 `]…` 是在等一个不存在的后半截）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。

### Q783 — 清点中那一景的屏底摆着 `[]d → 下一个问题] [/ → 搜索]`，而设计稿自己的 `KEYMAP` 与实现的按键表都把这两个键标成清点完之后才派得出

- **From:** 票 `session-redesign/06`
- **Kind:** 路过发现的设计稿缺陷（`footerHints` 的跑着那一副不分清点中）
- **Where:** `design.html` 的 `footerHints`（`if (!r) … else [...stageHints(), hint(']d', …), …, hint('/', …), …]`，`surveying` 也走这一支）与 `KEYMAP`（`/`、`]d [d` 标 `RDE`）；`tests/fixtures/design/snapshots/survey.120x36.*`；`src/session/keymap.rs` 那两行（`AFTER_SURVEY`）；`CONTEXT.md`《按键表》「清点完之前 `/` 那几个键派不出去」
- **Why it did not block:** 本票不画清点中那一景；实现照「屏上不摆按不动的键」从表上挑，清点中那一档屏底不会有这两件——总览那一票（08）逐格比 `survey.120x36` 时那一行会对不上
- **What this ticket actually did:** 记下
- **Options:** ① 设计稿的清点中屏底不摆这两件，重导 `survey.*`；② 表上把 `/` 与 `]d`／`[d` 放宽到清点中——按下去没有树可搜、没有问题可跳，屏底摆着按不动的键
- **Recommend:** ①（设计稿的 `KEYMAP` 自己也是这么标的，`footerHints` 只是漏分了一档）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。
