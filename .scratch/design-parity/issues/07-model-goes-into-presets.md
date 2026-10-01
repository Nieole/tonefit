# 07 — 型号进预设

**What to build:** 存一份预设连型号一起存，套一份预设型号跟着换（2026-09-20 拍板：词汇表赢）。
套一份没写型号的预设，型号照《预设栏》那一条回到「没说」——跑之前要先挑型号，屏上照现成那一句说。
设计稿存与套两支都收型号，重导 `config-p-save-named`、`config-p-j-Enter`；「包含 N 项设置」、行尾的 `*`、
「改动了 N 项」三处读同一份单子，型号数在里面。

**Blocked by:** 01

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读
- [x] 存出去的预设包括型号；套用时型号跟着换
- [x] 套一份没写型号的预设，型号回到「没说」，屏上照现成那一句要求先挑型号
- [x] 三处计数读同一份单子；重导的两串比整屏且绿
- [x] 终端层预设读写那几条用例覆盖「连型号存、连型号套、没写型号的那一份」
- [x] `CONTEXT.md`《预设》《预设栏》不动（本来就这么说）
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q891 — 型号不进预设：设计稿的存与套都跳过它，而《预设》词条说预设装设备设置两组

- **From:** 票 `session-redesign/14`
- **Kind:** 词汇表与设计稿对不上（而且**两副界面对同一个词的理解也不一致**）
- **Where:** 设计稿 `design.html` 的 `configKey` 预设那一支
  （套用：`for (const item of CONFIG) if (item.kind === 'ring' || item.kind === 'text') …`，`model` 那一种不在）
  与 `submitInput` 的 `preset` 那一支（存出去的 `says` 同样只收 `ring`／`text`）；
  `CONTEXT.md` 的《预设》（「装设备设置与处理选项两组」）与《设备设置》（型号在里面）；
  实现那一侧是 `src/session/config.rs` 的 `stored_fields` 与 `src/session/view.rs` 的
  `apply_preset`／`preset_to_store`
- **Why it did not block:** **屏上把这一头钉死了**：`config-p-save-named` 那一串写着
  `插图    包含 2 项设置：缩放方式、抖动`（型号那一项没数进去），`config-p-j-Enter` 套完「画集」
  之后设置栏上型号仍是 `kobo-libra-2`、画质判定参数那一组照旧答得出设备配置。
  型号真跟着预设走的话，套一份没说型号的预设就等于把这一趟的型号清空——那一刻连跑都跑不起来
  （`Session::request` 第一句就是「先挑型号」）。
- **What this ticket actually did:** **照设计稿**（ADR 0019 决定第 13 条）：存与套都跳过型号，
  一处出处收在 `config::stored_fields`（设备设置与处理选项两组里除型号之外那十四项）——
  13 留下的「行尾那个 `*` 不数型号」（`config::starred`）改成读它，屏上四处
  （行尾的 `*`、顶上那句「改动了 N 项」、预设栏那句「包含 N 项设置」、存出去的那一份）
  从此数的是同一份单子。**《预设》那条词条一个字没动**（`CLAUDE.md`：改写已有词条的含义要先拍板）。
- **这一条比「词汇表对不上设计稿」更要紧的那半句：** **两副界面对同一个词的理解不一致。**
  旧界面存一份预设走的是 `Session::preset()`（`src/session/state.rs`），它**连型号一起存**；
  新界面走 `Session::preset_to_store()`，不存型号。同一份预设文件，两副界面写出去的形状不同：
  旧界面存的那一份拿到新界面里套，型号那一格套不进来（新界面根本不读它）。
  **15 号票让旧那一副退场时正撞上它**——那一刻要先定「预设到底收不收型号」。
- **Options:** ① 照设计稿（已落地）：预设只收十四项，型号是「这台机器上的设备」、不跟着一份配置走；
  ② 照词条：存与套都带上型号，设计稿的 `configKey` 与 `submitInput` 两支跟着改、
  `config-p-save-named` 与 `config-p-j-Enter` 两串重新导出，并另答「套一份没说型号的预设时型号怎么办」；
  ③ 认下两副界面各走各的，等 15 号票再判：不取——那意味着这中间存出去的预设文件形状取决于用户走了哪一副界面
- **Recommend:** ①，并**把《预设》那条词条改窄**（写成「装设备设置里除型号之外的那两项与处理选项」），
  同时把旧界面的 `Session::preset()` 收到 `preset_to_store` 上去。理由：型号是**这台机器连着哪块屏**，
  与「这一趟的立场」不是一回事；而一份跨机器传阅的预设带着别人的型号，套下来判定整个换一套尺子。
- **Whose call:** 拍板的人（动 `CONTEXT.md` 的《预设》词条，连带两副界面的收口）
- **处置：** **拍板（2026-09-20）：词汇表赢——型号要进预设。** `CONTEXT.md`《预设》那条词条不动（预设装设备设置与处理选项两组，型号在设备设置里）；**改的是设计稿与实现**：`submitInput` 的 `preset` 那一支与 `configKey` 的套用那一支都要收型号，照 ADR 0019 决定第 13 条先改设计稿、重新导出 `config-p-save-named`／`config-p-j-Enter` 那几串，再改 `config::stored_fields`。连带 `15` 号票让旧那一副退场时**不必再判两副界面写出的形状不同**——两边都存型号，冲突消失。

## 落地记录

**本票做了什么。** 先改设计稿、重导，再让实现跟上（ADR 0019 决定第 13 条）。结转的 Q891 照拍板（词汇表赢）收了。

| 处 | 设计稿（`.scratch/session-redesign/design.html`） | 实现 |
|---|---|---|
| 预设记得下的那几项 | 新一句 `stored`：型号、取值环、自由填三种（设备设置与处理选项两组） | `config::stored_fields` 就是那两组、一项不落、次序照设置栏；判型号不算的 `config::stored` 删掉 |
| 存 | `submitInput` 的 `preset` 那一支 `CONFIG.filter(stored)`，型号进 `says` | `Session::preset_to_store` 存会话上那两层整份，型号在里面 |
| 套 | `configKey` 的套用那一支 `if (stored(item))`，型号照 `says`，没写就回到 `null` | `Session::apply_preset` 先走 `set_device`（换型号、清旧的两个标定数），再摆上这一份的两个标定数与处理选项 |
| 三处计数 | `changedKeys` 与 `drawCfgLeft` 行尾 `*` 都改问 `stored`；「包含 N 项设置」数的是 `says`，存出去的 `says` 就是那几项 | `config::changed`／`starred`／`said_fields` 读同一份单子，型号在里面 |
| 假数据 | `PRESETS` 的「漫画」写了型号 `kobo-libra-2`（Q1147）；「画集」照旧没写 | 夹具照场景数据写预设文件（`profile` 进 `device` 那一节，本来就认） |

- **预设文件的读写格式一格没加**：`[preset."名字".device]` 的 `profile` 本来就在（`crate::preset` 的 `OnDisk`），命令行 `--preset` 一直从它取型号；
  没写型号的那一份照旧读得进、`profile` 是「没说」——「画集」就是这样一份。
- **重导之后**：`git diff --stat -- tests/fixtures/design` 读过，34 份：
  - `config-p-save-named`：「插图」那一行成了「包含 3 项设置：型号、缩放方式、抖动」；场景数据「插图」多 `profile`。
  - `config-p-j-Enter`：套「画集」之后型号行「未挑（跑起来之前必填）」、画质门槛「跟着型号走（先挑一个）」、设备配置「型号未选择」、顶栏「型号未选择」；场景数据 `profile: null`。
  - 本票牵到的：掀着预设栏的六串（`config-p`、`config-p-G`、`config-p-G-dd`、`config-p-dd`、`config-p-save`、`config-p-click-preset`）与 `running-2-p-Enter`，
    「漫画」那一行从「没有设置任何项（全部默认）」换成「包含 1 项设置：型号」；`config-model-drill-j-l` 换了型号，顶上成「改动了 3 项：型号、缩放方式、抖动」、
    型号行带 `*`。十二景场景数据的「漫画」各多一格 `profile`。别的快照一格没变。
  - node 24.16.0（`fnm exec`）；`npm run check` 逐字节相同，`npm test` 4 条全过。
- **用例**：
  - `config`：`a_preset_records_every_setting_of_the_two_bands_the_model_too`（单子就是设置栏那两组、次序照屏上；换了型号那一行带 `*`、数进「改动了」），
    `a_preset_says_only_the_settings_it_was_saved_with`（「漫画」只说型号、「画集」四项、空的那一份一项都不说）。
  - `view`：`changed_items_are_counted_against_the_applied_preset_the_model_too`、`using_a_preset_replaces_both_bands_the_model_too_and_leaves_the_paths_alone`、
    新 `using_a_preset_that_names_a_model_brings_the_model_and_its_numbers_along`、`the_preset_a_save_would_store_carries_the_model`。
  - `terminal`（终端层预设读写）：连型号存——`saving_a_named_preset_writes_it_into_the_preset_file_on_disk` 从盘上读回型号；
    `a_preset_saved_in_the_session_is_the_one_the_command_line_takes` 命令行不再点 `--profile`，型号从那份预设里来。
    连型号套——新 `a_preset_that_names_a_model_brings_it_along_when_used`（盘上手写一节带 `profile` 与 `gray-levels` 的，`p` 读进来、`⏎` 套下）。
    没写型号的那一份——`enter_uses_the_preset_under_the_cursor_and_replaces_both_bands`（`config-p-j-Enter` 比整屏，型号回到「没说」），
    新 `a_preset_that_names_no_model_leaves_the_run_asking_for_one`（接着按 `t`，屏底那一句取自 `Session::request`，那一趟没起来）。
    新 `a_preset_that_says_nothing_is_listed_as_saying_nothing`：「没有设置任何项（全部默认）」不再有期望屏钉着（Q1147），由它钉。
  - `an_existing_name_takes_two_presses_before_it_overwrites`：撞名的「漫画」盘上那一份改成先读下来再比（它写了型号，不再是空的）。
- **`CONTEXT.md` 没改**：《预设》《预设栏》本来就这么说。
- **代价**：「漫画」从一项都没说换成写了型号（Q1147）；详情栏型号那一项照设计稿仍不说「预设「X」中：…」（Q1148）；
  设计稿 `startRun` 不认型号没挑，「套了没写型号的那一份再按 `t`」没有期望屏，只有终端层用例（Q1149）。

### 按反跑过的几遍（实现落地之后按反、跑 `cargo test --bin tonefit session`、还原）

| 按反 | 结果 |
|---|---|
| `apply_preset` 不碰型号（本票之前的样子） | 红 5 条：`using_a_preset_replaces_both_bands_…`、`using_a_preset_that_names_a_model_…`（view），`enter_uses_the_preset_…`、`a_preset_that_names_no_model_…`、`a_preset_that_names_a_model_brings_it_along_…`（terminal） |
| `apply_preset` 先摆两个标定数、后 `set_device` | 红 2 条：`using_a_preset_that_names_a_model_…`、`a_preset_that_names_a_model_brings_it_along_…`——可见灰阶数被换型号那一下抹掉 |
| `preset_to_store` 把型号写成 `None`（本票之前的样子） | 红 3 条：`the_preset_a_save_would_store_carries_the_model`、`saving_a_named_preset_…`（`config-p-save-named` 比屏）、`a_preset_saved_in_the_session_is_the_one_the_command_line_takes`（命令行拼不出型号） |
| `stored_fields` 滤掉型号 | 红 11 条：`config` 两条、`changed_items_…`，以及掀着预设栏与换型号那几串比屏（`p_lifts_the_picker_…`、`dd_twice_…`、`dd_on_the_save_row_…`、`saving_a_named_preset_…`、`enter_uses_…`、`during_a_run_the_picker_…`、`the_model_drills_…`、`the_wheel_the_click_…`） |
| 预设栏 `says` 没有「一项都没说」那一支 | 红 1 条：`a_preset_that_says_nothing_is_listed_as_saying_nothing` |

设计稿改完、重导、实现还没动的那一趟：`cargo test --bin tonefit -- preset config_p model_drill picker design` 红 9 条（掀着预设栏与换型号那几串比屏、撞名、存、套）；
`config` 两条新写的用例在实现之前各红一次。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff a9d2978`（未提交的工作树）。Spec 轴：票面六条都有改动与用例撑着，没有缺的。

**收下的**：

- **注释说的不是事实**（Standards，CLAUDE.md《文档写作》第 1 条）：设计稿 `stored` 那句注释说「包含 N 项设置」读它，`drawPresets` 读的是 `says`；
  `stored_fields` 与 `preset_to_store` 的文档说存出去的那一份「数」这份单子，它整份抄两层。三处改成实情。
- **拍过板的结论旁不指条目号**（Standards，停车场抬头 Q493 那条规矩）：`stored_fields`、`apply_preset`、`preset_to_store` 与设计稿 `stored` 旁添「停车场 Q891 拍板」。
- **一条用例里同一个数写了两遍**（Standards，testing.md「一条用例里同一个数只有一个出处」）：`boox-poke6`／`12`／`kobo-libra-2` 在三条用例里各收成一处。
- **「先挑型号：」是三句共用的前缀**（Spec）：`a_preset_that_names_no_model_…` 改成从 `Session::request` 取那一句整句来比，不抄字。
- **注释里的「如今」**（Standards，「结果」）：改掉。

**驳回的**：

- **「漫画」写型号超出「只改存与套两支」**（Spec）：不写的话六十份快照「改了 2 项」跟着变成 3 项；两条路都说得通，记 Q1147 待拍板。
- **`starred` 不读 `stored_fields`，一处出处名存实亡**（Spec）：`*` 挂在设置栏那两组的每一行上，那两组就是那份单子；给 `starred` 再套一层 `contains` 恒为真。文档写明了这一层关系。
- **view 与 terminal 两条「写了型号」用例断同一组事实**（Standards，Duplicated Code）：两个接缝——状态机那一条在 `--no-default-features` 那一趟也跑，终端层那一条走盘上那份文件的读路径（票面要的就是终端层）。
- **测试里的 `views.config.applied.as_ref().map(..)` 链**（Standards，Message Chains）：既有用例的写法，只在测试里。

### 停车场

本票用了 Q1147–Q1156 里的三个：

- **Q1147**：设计稿「漫画」写了型号，预设栏那一行跟着变；推荐照现在。
- **Q1148**：详情栏型号那一项不说「预设「X」中：…」，而型号行会带 `*`；推荐设计稿补上。
- **Q1149**：设计稿 `startRun` 不认型号没挑；推荐设计稿先问型号、导一串期望屏。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态，四条顺序跑
（日志 `dp-07.gate1.log`、`dp-07.gate2.log`、`dp-07.gate3.log`、`dp-07.polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
本票这一栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1067 通过 1 失败**；lib 239 / bin 443；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.56s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **946 通过 1 失败**；lib 239 / bin 322；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 56.40s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`全绿。` |
| polish | `cargo xtask polish`（fmt、两道 clippy、doc） | 绿，`POLISH_EXIT=0`；`全绿。`，`cargo doc` 告警 15 条（与基线同数） |

**基线**是 `a9d2978`，没在它上面重跑：`design-parity/13`（1060／942）记的是并进 `ss/01` 之前那一趟，`ss/01`（两趟各多 3 条）先进了 main，
推得闸门 1 **1063 通过 1 失败**、闸门 2 **945 通过 1 失败**。**闸门 1 多 4 条、闸门 2 多 1 条，都在预期里**：
`using_a_preset_that_names_a_model_brings_the_model_and_its_numbers_along`（view）两趟都编；
`a_preset_that_names_no_model_leaves_the_run_asking_for_one`、`a_preset_that_names_a_model_brings_it_along_when_used`、
`a_preset_that_says_nothing_is_listed_as_saying_nothing`（terminal）只在默认那一趟。其余是改写，不增不减。

**黄金快照逐格没动**：`git diff a9d2978 -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；快照 sha256 仍为 `2a6aabc0…`。

