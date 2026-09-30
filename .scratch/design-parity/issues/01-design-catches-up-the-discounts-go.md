# 01 — 设计稿追上实现已经对了的那几处，四副打折手法退场

**What to build:** 逐格比对从此没有例外。下面十处**实现本来就是对的**，错在设计稿或导出——把设计稿改对、重导一次，
比对器上为它们长出来的四副打折手法（一段抹成空白、一格换成右邻、一段往右推、一整行换掉）连同每一处用法删掉，
那几串改比整屏。

- 清点中输出目录那一行不摆 `i → 修改`（Q807）；
- 环节横条的分子取屏上印的那个整数页数（Q844），导出时总进度的步数向下取整（Q899）；
- 设计稿的折行照《折行》带悬挂缩进（Q845）；
- 灰阶测试图那句回话照实现说出写到了哪里（假盘上的一条路径），不再说「原型不写文件」；
  「拷进设备，用原始尺寸打开」那半句不上屏底，命令行那一路照旧印它（Q890）；
- 补全候选按名字排（Q790）；「添加路径」那一景走真的补全动作，缓冲换成头一个候选（Q797）；
- 隔离那一卷的去处带上处理路径那一级（Q765）；`parentHint` 改成真正的父目录、导出直接读它（Q738）；
  预设文件那条路径导出进场景数据（Q824）。

**本票不改实现在屏上的任何行为**——改的是设计稿追上它，外加导出、夹具与比对器。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 比对器上那四副打折手法删掉，用到它们的每一串比整屏且绿
- [x] `fresh-o-Tab`、`fresh-o-Tab-Tab` 两串比整屏且绿
- [x] 夹具里「先收下候选再把缓冲改回去」与「给预设文件摆一条家目录下的路径」两手删掉，改读场景数据
- [x] 场景数据里隔离那一卷的去处与库的镜像规则一致；目录的根只有设计稿一处算法
- [x] 实现的画法与终端层一行不动（`git diff` 只落在设计稿、导出、夹具与比对器上）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q807 — 清点中那一副的输出目录行上，设计稿仍写着 `[i → 修改]`，而那一档 `i` 派不出去

- **From:** 票 `session-redesign/08`
- **Kind:** 路过发现的设计稿缺陷（与 Q777、Q783 同一族：屏上摆着按不动的键）
- **Where:** `design.html` 的 `drawRow`（`out` 那一支恒 `...hint('i', '修改')`，不问阶段）与 `taskKey`（`if (!r || r.stage === 'surveying') { if (r) return false; … }` ——跑起来之后 `i` 一件事都不做）；`tests/fixtures/design/snapshots/survey.120x36.*`、`survey.80x24.*`、`sequences/survey-s.*`；`src/session/keymap.rs` 的 `EditPath` 三行标 `FRESH`
- **Why it did not block:** 设计稿自己的 `taskKey` 就不派它——屏上那一句是 `drawRow` 漏分了一档，不是有意为之；实现照 06 立的那条（行上顺口提的键一律从按键表取，派不出就不提）不写它
- **What this ticket actually did:** 照按键表办，屏上不写那十格；逐格比对时把**期望屏上那十格抹成空白**（新加的 `Expected::blanked`，抹掉之后仍要求实现在那儿一个字都不写），用例的文档里指着本条。**设计稿一个字节没改**
- **Options:** ① 设计稿的 `drawRow` 在 `out` 那一支分一档，跑起来之后不摆 `[i → 修改]`，重导 `survey.*` 与 `survey-s`，实现这一侧把 `blanked` 那一手拿掉 ② 表上把 `EditPath` 放宽到清点中——按下去改的是**下一趟**的输出目录，而这一趟的已经拼进 `Request` 了，屏上那句话会骗人 ③ 照旧：抹掉那十格，留着一处「快照与实现说不一样」
- **Recommend:** ①（与 Q777、Q783 同一批一起改，它们是同一条毛病的三处）
- **Whose call:** 拍板的人（动设计稿）。顺带核到：**Q783 已不成立**——`survey.120x36` 的屏底就是 `[s → 停止] [j/k → 选择] [? → 全部按键]`，`]d` 与 `/` 都不在上面；**Q777 也已不成立**——`running.56x14` 写的是 `[C-c → 退出]`
- **处置：** **拍板（2026-09-20）：先把设计稿改对，再同步改实现。** 四副折扣手法（本条的 `blanked`、Q844 的 `cell_like`、Q845 的 `shifted`、Q890 的 `instead`）全是设计稿与实现对不上逃出来的——照 ADR 0019 决定第 13 条改设计稿、重新导出、再改实现，四副都不必存在。**不收成统一记法**：那等于把病固化成工具。
  **落地（design-parity/01，2026-10-01）：** `drawRow` 那一行清点中不摆 `[i → 修改]`，重导 `survey.*`、`survey-s`、`fresh-t`、`fresh-x`，`blanked` 删了。

#### Q844 — 设计稿的环节横条走**连续时间**（`done` 带小数），实现一页一步：同一条横条差一格

- **From:** 票 `session-redesign/10`
- **Kind:** 票面没想到的第三种情形（夹具与实现之间的**分辨率**差，不是谁算错了）
- **Where:** `design.html` 的 `drawOverview`（`pf = cs.done / cur.pages`）与 `list.rs` 的 `walking_segments`；夹具那一头 `scene.rs` 的 `step(&mut live, volume.done.floor() as usize)`
- **Why it did not block:** 设计稿的模拟按连续时间推进，`灰原哀/第05卷` 的 `done` 是 **114.554**——半页也占横条一格，24 格的横条因此满 17 格、8 格的满 6 格；而屏上那个数印的是 `Math.floor(done)` = 114。实现一页一步，`Live` 只数得出 114（一个「步」就是一页，没有半页这回事），同一条横条满 16 与 5。**两边的数相同（114/166），横条差一格。** `running.*` 那两屏当初绿是撞上了：`done` 是 13.679，floor 与它 round 到同一格
- **What this ticket actually did:** 「整卷统一灰阶」那两屏上**换掉三格**（120×36 的第 3 行第 46 格与第 28 行第 111 格、80×24 的第 20 行第 57 格），各换成它右边那一格——新添 `Expected::cell_like`，换完之后**仍是一条断言**（实现在那一格上写别的照样红）。与 Q807 的 `blanked` 同一副做法、同一条理由
- **Options:** ① 照现状：打三格折扣，记在这里 ② 让夹具把 `done` 进到 115：那个数印出来就是 115，与快照的 114 对不上，换一处红 ③ 按 ADR 0019 决定第 13 条改设计稿——把 `pf` 的分子改成 `Math.floor(cs.done)`（屏上那个数本来就是它），重新导出 `envelope.*`，这三格折扣跟着拿掉
- **Recommend:** ③。这一处**设计稿自己不自洽**：同一行上那个数取 floor、横条取原值，而真程序拿不出半页
- **Whose call:** 拍板的人（改设计稿、重新导出是他的事）
- **处置：** **拍板（2026-09-20）：先把设计稿改对，再同步改实现**（与 Q807、Q845、Q890 同一判，四条一起做）。本条的根是设计稿那头的模拟走连续时间而实现一页一步——改设计稿、重导，`cell_like` 随之退场。
  **落地（design-parity/01，2026-10-01）：** 三条环节横条的分子取 `Math.floor(done)`，重导 `envelope.*`、`search.*` 与两串，`cell_like` 删了。

#### Q899 — `deciding-x-advance` 总进度那一行的末一位：夹具与期望屏把**同一个数**印成了两副

- **From:** 票 `session-redesign/12`
- **Kind:** 设计稿与这一趟对不上的一格（**与 Q844 同一类、不同根**）
- **Where:** `tests/fixtures/design/sequences/deciding-x-advance.scene.json` 的
  `run.steps`（`3799`）对着 `deciding-x-advance.text.txt` 第 2 行的 `3798/46809 步`
- **Why it did not block:** 设计稿攒出来的 `r.steps` 是个**浮点数**（`design.html` 的
  `run.steps += Math.max(0, stepsOf(v) - walked)` 一路累加），导出那一步 `num()` 是
  `Math.round(x * 1000) / 1000` → 写成 `3799`，而屏上那一格是 `Math.floor(r.steps)`
  → 印成 `3798`：真值差一丝不到 3799。**这一趟一页一步**，走出来的是整数 **3799**，
  与场景数据那一格逐格相同（`Scene` 的自检 `overall.walked == run.steps.floor()` 按它核过），
  因此实现这一头没有第二种答案可选
- **What this ticket actually did:** `cell_like(2, 57, 63)` 把期望屏那一格换成同一行
  `46809` 里那个 `9`（同色同修饰），理由写在用例上。**换完仍是一条断言**：
  实现在那一格上写别的照样红
- **Options:** ① 照现状 ② 导出那一步把 `steps` 也改成 `Math.floor` 再重导这一串
  ——ADR 0019 决定第 13 条 ③ 屏上那一格改成四舍五入：不取，那是让实现去迁就一个导出脚本的取整
- **Recommend:** ②，与 **Q844** 一起看（两条都是「设计稿那一头的数与这一趟差一格」，
  根不同：那一条是连续时间对一页一步，这一条是同一个量印了两副）
- **Whose call:** 拍板的人
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** 导出向下取整，外加环节走满时结清 `run.steps`（Q1057），重导 `deciding-x-advance`。

#### Q845 — 说明卡正文：设计稿的折行**不带悬挂缩进**，而屏上《折行》那一条带

- **From:** 票 `session-redesign/10`
- **Kind:** 票面没想到的第三种情形（撞的是一条**已经拍过板**的规矩）
- **Where:** `design.html` 的 `wrap()` 与 `crate::wrap::fold_line`（`hanging = leading_spaces(line)`）；用例在 `src/session/terminal.rs` 的 `the_card_of_the_ignored_files_says_what_the_report_says`
- **Why it did not block:** 说明卡的正文是报告末尾那一小结（`  路径` 一行、`    原因` 一行）。原因那一行折下来的第二行，设计稿顶格写（`卷。如果它和图片…`），而 `crate::wrap` 那一条**行首缩进跟着折下来的每一行走**（停车场 Q32／Q114 立的就是它，理由是「折下来的那一截缩不回去，条目的边界就没了」）。`CONTEXT.md` 的《折行》明写着「会话的详情栏、**说明卡**与预设栏里的说明共用这一套」「屏上没有第二套折行规矩」——照设计稿要在 `wrap` 里给这一处开一个不缩的旁支。两套算法比过一遍，**除了这一截缩进，断在哪儿、折出几行逐行相同**（无法访问那一张全等）
- **What this ticket actually did:** 照《折行》那一条缩，`ended-nonvolume-Enter` 的第 20、23 两行**把期望屏那一段往右推四格**——新添 `Expected::shifted`：推开前先断言被挤掉的那四格本来就是空白，因此**一个字都没丢**，仍是一条断言
- **Options:** ① 照现状 ② 给 `crate::wrap` 开一个「折下来的不跟缩进」的旁支，只给说明卡用——《折行》那句「屏上没有第二套」跟着要改 ③ 按 ADR 0019 决定第 13 条改设计稿的 `wrap()`：让它也缩，重新导出 `ended-nonvolume-Enter`，这一手折扣跟着拿掉
- **Recommend:** ③。顶格那一行读起来像下一条的开头，而那正是 Q32 当初要治的毛病
- **Whose call:** 拍板的人
- **处置：** **拍板（2026-09-20）：先把设计稿改对，再同步改实现**（与 Q807、Q844、Q890 同一判）。设计稿的折行不带悬挂缩进而屏上《折行》带——改设计稿、重导，`shifted` 随之退场。
  **落地（design-parity/01，2026-10-01）：** `wrap` 带悬挂缩进，重导 `ended-nonvolume-Enter`，`shifted` 删了。

#### Q890 — 灰阶测试图那一句：设计稿说的是「原型不写文件」，而这一副真写得出文件

- **From:** 票 `session-redesign/14`
- **Kind:** 票面写错了的反面——**设计稿写不出真话**（设计稿与票面互相打架，这一次票面对）
- **Where:** 设计稿 `design.html` 的 `configKey` 的 `c` 那一支
  （`toast([['✓ 已生成灰阶测试图','c-green b'], ['（1264x1680）：拷进设备，用原始尺寸打开（原型不写文件）','c-fg']], 3200)`）
  与它导出的 `tests/fixtures/design/sequences/config-c.{text,style}.txt` 末一行；
  实现那一侧是 `src/session/terminal.rs` 的 `draw_a_chart`，折扣那一手是
  `src/session/draw/design.rs` 新添的 `Expected::instead`
- **Why it did not block:** 屏上那一行只差**末尾那一截**，别的 35 行一格不差；
  而票面第五条明写着「`c` 出灰阶测试图，**回话说写到了哪里**」——原型说得出「不写文件」，
  这一副说不出，它真写。ADR 0019 决定第 13 条是「设计稿是权威，对不上先改设计稿、重新导出」，
  而那是拍板的人的事，因此这一票走折扣、不改快照（`git diff -- tests/fixtures/design` 为空）。
- **What this ticket actually did:** 屏底那一句**到冒号为止照设计稿一字不差**
  （`✓ 已生成灰阶测试图`、`（1264x1680）：`，连 3200 毫秒那个时长也照它），
  冒号之后整截换成**图落在哪儿**（家目录缩写成 `~`）。
  **一并让掉的还有设计稿那半句使用说明**（「拷进设备，用原始尺寸打开」）：屏底恒一行，
  而那一句与一条绝对路径同时摆不下（`CONTEXT.md` 的《让位》：行尾那一句从尾部截断），
  票面第五条要的是路径。命令行那一路照旧两行都印，那一句的出处仍是
  `render::calibration_notice` 的 `OPEN_IT_AT_NATIVE_SIZE`——新界面这一句因此**比旧界面少说一件事**。`config-c` 那一串因此**整行换掉**再比：
  新添 `Expected::instead(row, &[Segment])`——那一行连同每一格的字、前景色与修饰由用例自己写出来、
  行尾补到同宽，**换完仍是一条断言**（实现说别的、多写一个字都红）。
- **Options:** ① 照本票：实现说真话，那一行走折扣，那半句使用说明让掉；
  ② 改设计稿那一句（原型也点一条假路径出来）、重新导出 `config-c` 两张网格，实现照它——
  那时要一并答「路径与那半句使用说明在一行里怎么摆得下」（另开一张说明卡是一条路）；
  ③ 实现照设计稿原样说「原型不写文件」：不取——那是屏上一句假话，而票面第五条正好要的是那条路径
- **Recommend:** ②（设计稿那一句里「原型」两个字本来就只对原型成立；它是**这一副界面唯一一句
  自认是原型的话**）。在它落地之前 ① 是对的。
- **另有一问，一并记在这里：** `Expected` 上「打折但仍是断言」的手法**这是第四副**——
  **Q807** 用 `blanked`（一段抹成空白）、**Q844** 用 `cell_like`（一格换成右邻那一格）、
  **Q845** 用 `shifted`（一段往右推几格），本条用 `instead`（一行整个换掉）。
  四副守的是同一条规矩（换完仍要红得起来），眼下却是**四个各自独立的口子**：
  四个函数、四处各自靠用例上一句话点明是哪一条停车场条目，而「这一处为什么豁免」
  在代码里没有一个共同的落点。**要不要收成一副**（比如一处统一的
  「这一格／这一行／这一段按什么理由豁免、指着哪一条」的记法）归拍板的人；
  **Q807、Q844、Q845 三条本身一个字都没动**。
- **Whose call:** 拍板的人（动设计稿那一侧；四副折扣手法要不要收口同样归他）
- **处置：** **拍板（2026-09-20）：先把设计稿改对，再同步改实现**（与 Q807、Q844、Q845 同一判）。设计稿那句「原型不写文件」与真程序写得出文件对不上——改设计稿、重导，`instead` 随之退场。**四副手法要不要收成一副：不收**，根治是让它们没有存在的理由。
  **落地（design-parity/01，2026-10-01）：** 回话照实现说写到了哪里（`~/tonefit-calibration-kobo-libra-2-16-levels.png`，落点导成场景数据 `cwd`，Q1059），重导 `config-c`，`instead` 删了。四副不收成一副。

#### Q790 — 补全候选的次序两边不同：设计稿按假盘 `FS` 的写法次序列（`漫画库 下载 Comics 转好的`、`棋魂 大友克洋 火之鸟 寄生兽`），实现按名字排——`fresh-o-Tab`、`fresh-o-Tab-Tab` 两串比不了屏

- **From:** 票 `session-redesign/07`
- **Kind:** 设计稿与实现对不上（假盘的次序是设计稿手写的，盘上 `read_dir` 的次序本来就不定）
- **Where:** `design.html` 的 `fsList`（`Object.keys` 的写法次序）；`src/session/complete.rs` 的 `level`（`listed.sort()`，按码点）；`tests/fixtures/design/sequences/fresh-o-Tab*`、`snapshots/add.*`、`sequences/add-F1*`、`add-narrow-*`
- **Why it did not block:** 实现只能有一个定得下来的次序，那就是名字的次序（`complete::level` 早就这么排）；`add` 那一景与 `add-F1*` 那几串的候选是场景数据里摆好的，夹具照它的次序摆、逐格相等；`fresh-o-Tab-Tab-C-w` 之后候选没了、逐格相等
- **What this ticket actually did:** `fresh-o-Tab` 与 `fresh-o-Tab-Tab` 两串不比屏，只断言候选四条、都是文件夹、轮到第几个、缓冲是头上那一层加轮到的那一条（`terminal.rs` 的 `tab_lists_the_level_cycles_through_it_and_ctrl_w_deletes_a_segment`）
- **Options:** ① 设计稿的 `fsList` 按名字排（JS 的默认 `sort()` 按码元，与 Rust 字符串的字节序在基本平面上同序），重导 `add.*`、`add-F1*`、`add-narrow-*`、`fresh-o-Tab*`，两串改成比屏；② 实现照盘上的次序——不定，用例不稳；③ 照旧，两串永远不比屏
- **Recommend:** ①
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** `fsList` 按名字排，`fresh-o-Tab`、`fresh-o-Tab-Tab` 比整屏。

#### Q797 — 设计稿「添加路径」那一景的状态按键到不了：候选摆上了、缓冲没跟着换（`~/Comics/`，按过 `Tab` 该是 `~/Comics/棋魂/`）

- **From:** 票 `session-redesign/07`
- **Kind:** 路过发现的设计稿缺陷（`scene('add')` 直接塞 `cands`，不走 `complete()`）
- **Where:** `design.html` 的 `scene`（`case 'add'`）；`tests/fixtures/design/snapshots/add.*`、`sequences/add-F1*`、`add-narrow-*`；`src/session/scene.rs` 的 `views_of`（照场景数据的缓冲摆）
- **Why it did not block:** 快照就是那副样子，夹具照它摆、逐格相等；从这一景再按 `Tab` 两边都轮到第二个
- **What this ticket actually did:** 夹具先收下候选再把缓冲改回场景数据里的那一句
- **Options:** ① 设计稿 `scene('add')` 走 `complete()`（缓冲变成头一个候选），重导 `add.*` 与那六串，夹具那一句改回去；② 照旧
- **Recommend:** ①（一景该是按键到得了的状态；与 Q790 一起改一次）
- **Whose call:** 拍板的人（动设计稿）
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** `scene('add')` 走 `complete()`，夹具不再把缓冲改回去（新的 `offered` 从缓冲认出那一层）。

#### Q765 — 设计稿里卷的去处少了处理路径自己那一级：`~/转好的/_isolated/集英社/海贼王/第07卷`，库的镜像规则是 `…/_isolated/漫画库/集英社/海贼王/第07卷`

- **From:** 票 `session-redesign/05`
- **Kind:** 路过发现的设计稿假数据与库规则不一致
- **Where:** `design.html` 的 `isolatedOutput`（`${outRoot}/_isolated/${dir.label}/${name}`）；场景数据各卷的 `isolated_output`；`src/discover.rs` 的 `mirrored`（基准点是**处理路径的父目录**，处理路径自己的名字恒出现在输出目录下）；`src/session/scene.rs` 的 `output_of`
- **Why it did not block:** 24 份快照上没有一处露出去处：隔离那一句在 120 列上截在路径之前（`…这一卷整卷写到隔`），每页结果抬头那句「这一卷输出在 _isolated/」是定死的字。夹具照场景数据给的写法接其余各卷的去处（分区路径或顶格目录的上一层之下那一截），隔离那一卷直接取给的值，整趟内部一致
- **What this ticket actually did:** 照场景数据，不照库的规则
- **Options:** ① 照旧；② 设计稿 `isolatedOutput` 改成带处理路径名那一级并重导（只动场景数据的一个字段，快照一格不变）；③ 夹具改用库的规则，隔离那一卷与场景数据不一致
- **Recommend:** ②，下一次重导时顺手改——说明卡或更宽的屏哪天露出这条路径，它就该是库真会写出的那一条
- **Whose call:** 拍板的人（设计稿的假数据）
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** `isolatedOutput` 照库的镜像规则；新用例 `an_isolated_volume_lands_where_the_library_mirrors_it` 钉住。干净去处另记 Q1058。

#### Q738 — 设计稿里 `parentHint` 对分区底下的目录记的是处理路径的父目录，没人读它；导出自己按树算根

- **From:** 票 `session-redesign/02`
- **Kind:** 路过发现的无关缺陷
- **Where:** `design.html` 的 `discover`（分区底下的目录 `mkDir(label, np.path.slice(0, lastIndexOf('/') + 1), mix)`，记成 `~/` 而不是 `~/漫画库/`）与 `drawRow`（那个三目两边一样）；`export.js` 的 `roots`
- **Why it did not block:** 屏上没读它；导出的目录根按「分区的路径 + 目录名」算，与屏上的面包屑一致（`~/漫画库/集英社/海贼王/第07卷`），点名的压缩包带回 `.cbz`
- **What this ticket actually did:** 没动设计稿，导出自己算
- **Options:** ① 设计稿删掉 `parentHint`（或改成真正的父目录，`export.js` 的 `roots` 随之去掉）；② 照旧
- **Recommend:** ①，下次动设计稿时顺手
- **Whose call:** 协调人
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** `parentHint` 是真父目录，设计稿立 `dirRoot`、卷带 `root`，导出直接读，`roots()` 删了。

#### Q824 — 夹具里预设文件那条路径是**摆出来的**：设计稿把它写死了，而真文件不能摆进假家目录

- **From:** 票 `session-redesign/13`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/session/scene.rs` 的 `views_of`（`views.presets = Some(home/.config/tonefit/presets.toml)`）
  与 `write_presets`（真文件在 `<临时目录>/config/tonefit/presets.toml`）；设计稿 `drawConfig` 那一段
  `right: [['~/.config/tonefit/presets.toml', 'c-gray d']]`
- **Why it did not block:** 配置视图顶上那一条右端要印这条路径，而**它不是场景数据**——导出脚本没导它，
  设计稿直接写死。真文件挪不进假家目录：`~/` 底下多一个 `.config` 会让补全那几串多出一项，
  而 `fresh-o-Tab` 的设计快照钉着「`~/` 底下四项」。
- **What this ticket actually did:** 夹具给 `Views::presets` 摆一条家目录底下的路径（缩写出来正是设计稿那一条），
  真文件仍在临时目录里、`Presets` 照旧指着它。两处因此不是同一条路径，而屏上只看得见前一条。
- **Options:** ① 导出脚本把这条路径导进场景数据（`presets_file`），夹具照它摆，两处合成一条；
  ② 假盘建到家目录的一个子目录里（`~/` 不再是家目录本身），预设文件就摆得进家目录底下；
  ③ 照本票这样摆着，并在夹具那一处写清为什么
- **Recommend:** ①（那条路径本来就该是场景数据的一格：它是屏上的字）
- **Whose call:** 拍板的人（动导出脚本＝动设计稿那一侧）
- **处置：** 待处理。
  **落地（design-parity/01，2026-10-01）：** `presets_file` 导进场景数据，夹具照它摆；真文件仍在临时目录（Q1060）。

## 落地记录

**本票做了什么。** 设计稿追上实现本来就对了的十处，重导一次，比对器上的四副打折手法连同每一处用法删掉，那几串改比整屏。
每一处都是同一个次序：先把用例上的打折拿掉、看见红（红在设计稿那一格），再改设计稿、`npm run export`、看见绿。

| 条目 | 设计稿（`.scratch/session-redesign/design.html`） | 重导牵到的屏 |
|---|---|---|
| Q807 | `drawRow` 输出目录那一行：`S.run` 在（清点中）就不摆 `[i → 修改]` | `survey.*`、`survey-s`、`fresh-t`、`fresh-x` |
| Q844 | 三条环节横条（总览当前卷、目录行、卷行）的分子取 `Math.floor(done)`，与旁边那个数同源 | `envelope.*`、`search.*`、`search-Escape`、`search-Enter-Escape` |
| Q899 | 导出 `steps: Math.floor(r.steps)`；**另在一个环节走满时把 `run.steps` 结清成整数**（Q1057） | `deciding-x-advance`（3798 → 3799） |
| Q845 | `wrap` 带悬挂缩进：行首空格窄过这一格时跟着折下来的每一行走 | `ended-nonvolume-Enter` |
| Q890 | `c` 的回话说「写到 ~/tonefit-calibration-kobo-libra-2-16-levels.png」（`chartPath` 照 `terminal::chart_file` 拼，会话从 `CWD` 敲起来），那半句使用说明不上屏底 | `config-c` |
| Q790 | `fsList` 按名字排（与 `complete::level` 同一个键） | `fresh-o-Tab`、`fresh-o-Tab-Tab`、`add.*`、`add-F1*` |
| Q797 | `scene('add')` 走 `complete()`：候选摆上、缓冲换成头一条 `~/Comics/大友克洋/` | 同上 |
| Q765 | `isolatedOutput` 照库的镜像规则：`~/转好的/_isolated/漫画库/集英社/海贼王/第07卷` | 只动场景数据，屏一格没变 |
| Q738 | `parentHint` 改成真父目录，新增 `dirRoot`；卷带上 `named` 与 `root`；`drawRow` 里两边一样的三目收掉 | 只动导出：`export.js` 的 `roots()` 删了，直接读设计稿；目录根与卷根一格没变 |
| Q824 | 预设文件那条路径成了常量 `PRESETS_FILE`，导进场景数据 `presets_file` | 屏一格没变 |

- **导出**（`export.js`、`render.js`）：场景数据多两格 `cwd`、`presets_file`（序列里多半是 `"unchanged"`）；`steps` 向下取整；
  目录根读设计稿的 `dirRoot`、卷根读 `v.root`，导出自己不再算。
- **比对器**（`src/session/shell/design.rs`）：`blanked`、`shifted`、`cell_like`、`instead` 四个删了，
  `terminal.rs` 测试里的 `assert_sequence_with`／`_shifting`／`_like`／`_blanking` 四个包装一起删，`assert_sequence` 就是整屏比。
  `grep -rnE '\.(blanked|shifted|cell_like|instead)\(' src/session` 为空。
- **夹具**（`src/session/scene.rs`）：
  - 「先收下候选再把缓冲改回去」删了：新的 `offered` 从缓冲减去轮到的那一条认出候选在哪一层，`views_of` 与假盘 `disk` 共用它；
    收下候选、轮到那一条之后当场断言缓冲拼回场景数据那一句。
  - 「给预设文件摆一条家目录下的路径」删了：`Views::presets` 照 `presets_file` 摆（真文件仍在临时目录，Q1060）。
  - `Data` 多 `cwd`、`presets_file`；`cwd` 也建进假盘；`Run::steps` 改成 `u64`，自检 `overall.walked == run.steps`。
  - 新用例 `an_isolated_volume_lands_where_the_library_mirrors_it`：每一份场景数据（场景与序列）里进了隔离的卷，
    去处都是「输出目录 / `_isolated` / 卷根在处理路径的父目录之下那一截」。
  - `output_of` 的算法没动、文档改成当前成立的事实（它不照库的规则，Q1058）。
- **用例**：`shell.rs` 的清点中、整卷统一灰阶、搜索三景，`terminal.rs` 的 `fresh-t`、`fresh-x`、`survey-s`、`search-Escape`、
  `search-Enter-Escape`、`deciding-x-advance`、`ended-nonvolume-Enter`、`config-c`、`fresh-o-Tab`、`fresh-o-Tab-Tab` 都比整屏。
  `config-c` 那条比完整屏再核「屏上说的那条路径上真有这一张、只有这一张」（图落在场景数据说的 `cwd`，`charts_land_in` 读它）。
  `f1_while_typing_…` 那一句缓冲跟着 Q797 改成 `~/Comics/大友克洋/`。
- **实现的画法与终端层一行不动**：`terminal.rs`、`shell.rs` 的改动全在 `#[cfg(test)]` 模块里（逐段核过 `git diff -U0` 的行号）。
  `draw_a_chart` 的文档末段还说「屏底那一句与设计稿不同」，已经不对了——终端层本票一行不动，留给改的正是这个函数的
  `design-parity/11`（Q968）换成当前成立的事实。

**重导的漂移核过**：`git diff --stat -- tests/fixtures/design` 里网格变了的恰是上表那 22 份；场景数据除新增的两行之外，
只动了 `isolated_output`、`buffer`／`candidates`、`steps` 三格（`steps` 那 10 份只是千分位小数变整数）。`manifest.json` 没变。
工具链 node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。

### 按反跑过的几遍（改完都还原了）

四副打折各拿掉一次，都先红在设计稿那一格：清点中「第 6 行第 26 格 实际「 」期望「[」」、横条「第 3 行第 46 格 实际「⣀」期望「⣿」」、
总进度「第 2 行第 57 格 实际「9」期望「8」」、说明卡「第 20 行第 25 格 实际「 」期望「卷」」、灰阶测试图「第 35 行第 34 格 实际「写」期望「拷」」、
补全「第 30 行第 5 格 实际「C」期望「漫」」；预设文件那一格先红在「missing field `presets_file`」，隔离去处那条先红在
`~/转好的/_isolated/集英社/海贼王/第07卷` 对 `…/_isolated/漫画库/…`。新守卫另按反一次：

| 按反 | 结果 |
|---|---|
| `views_of` 收下候选后轮到下一条（`at + 1`） | 红：`the_add_scene_…`，「收下候选、轮到第 0 条之后的缓冲」`~/Comics/寄生兽/` 对 `~/Comics/大友克洋/` |
| `offered` 拿错那一条去减缓冲（`at + 1`） | 红：同一条，「缓冲「~/Comics/大友克洋/」不是那一层接上轮到的「寄生兽/」」 |
| `draw_a_chart` 不真写图、照样说写到了 | 红：`c_draws_…`，「只写出一张，就在屏上说的那儿」`[]` 对那条路径 |
| `deciding-x-advance.scene.json` 的 `steps` 改回 3798 | 红：`every_sequence_that_moves_…`，`3799` 对 `3798` |

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff c598cbe`。

**收下的**：

- **目录根的算法仍有两处**（Standards）：`mkVol` 的默认参数与 `export.js` 的 `roots().dirRoot` 各拼一遍 `${parentHint}${label}`。
  设计稿立一个 `dirRoot`，`mkVol` 用它，导出经 `render.js` 的把手读它；`roots()` 那个只转交的小工厂一并删掉。
- **`assert_sequence` 的文档说「那一套只有这一处」**（Standards）：`assert_sequence_with_every_page_of_the_open_volume` 还有一份，那句删掉。
- **按反那一遍没写**（Standards，testing.md 第一条）：补跑、写进上一节。
- **Q1058 推荐 ② 却落了 ③、而且新用例自己也在 Rust 里写了一遍规则**（Standards）：条目里补了两句——用例里那一份是拿来核数的神谕，
  不是拿来造数的；② 要在设计稿里添一个十处之外的函数，派活说明不许。
- **Q1061 原先那一条（`draw_a_chart` 文档）过不了「另一条路也说得通」**（Standards）：删掉、改记在本节上面；
  空出来的号给了下一条。
- **80 列「添加路径」那一屏缓冲被右端那几件盖住**（Spec）：Q797 之后才露出来的设计稿缺陷，记 Q1061。

**驳回的**：

- **`Math.floor(x.done)` 七处，提一个函数**（Standards）：设计稿是拍板的人的，本票只动十处；那几处数字本来就这么写，本票只让横条跟上。
- **`cwd` 与终端层的 `here` 两个名字**（Standards）：`cwd` 是场景数据的一格（与 `output` 同一副 `~/` 写法），两处文档都写明是「会话是从哪儿敲起来的」；
  词汇表里没有这个概念，它也不是新的领域概念（终端层早就有 `here`），不加词条。
- **`tick` 里结清步数超出票面字面**（Spec 提了、也认同必需）：见 Q1057。

### 停车场

本票用了 Q1057–Q1066 里的六个：

- **Q1057**：票面「导出时向下取整」单这一手收不了 Q899，另在环节走满时结清 `run.steps`。
- **Q1058**：夹具的干净去处（`output_of`）仍不照库的镜像规则；推荐设计稿把干净去处也导进场景数据。
- **Q1059**：灰阶测试图的落点导成场景数据 `cwd`，而不是夹具认定家目录。
- **Q1060**：预设文件路径读场景数据了，真文件仍在临时目录。
- **Q1061**：80 列「添加路径」那一屏缓冲被右端那几件盖住；推荐并进 `design-parity/02`。
- **Q1062**：设计稿的折行只补了悬挂缩进，与 `crate::wrap` 还有三处边角不同。

结转的十条（Q807、Q844、Q899、Q845、Q890、Q790、Q797、Q765、Q738、Q824）照上面的处置都收了。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `dp-01.gate1.log`、`dp-01.gate2.log`、`dp-01.gate3.log`、`dp-01.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
因此照前几票的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1036 通过 1 失败**；lib 239 / bin 424；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 48.29s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **921 通过 1 失败**；lib 239 / bin 309；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.91s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 11.34s` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；四项全绿，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `c598cbe`：闸门 1 **1035 通过 1 失败**（lib 239 / bin 423），闸门 2 **920 通过 1 失败**（lib 239 / bin 308），闸门 3 绿，红的是同一条。
**两条闸门各多 1 条，都在预期里**：bin 里新添的 `an_isolated_volume_lands_where_the_library_mirrors_it`（夹具两趟都编）。
删掉的四副打折手法没有自己的用例，比对器上那几条用例原样在，只是改成比整屏。

**黄金快照逐格没动**：`git diff c598cbe -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`；`tests/golden.rs` 2 条全过（闸门 1 上 189.89 秒），
`tests/counters.rs` 14 条全过。
