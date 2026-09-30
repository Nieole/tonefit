# 06 — 说不出话的那几种

**What to build:** 样张认**一张图**。点成别的东西、或者那张图根本解不开时，
当场收到一句说得清的话——而不是去翻一个空目录猜。

四种：点了一个目录或一个归档；点了解不开的图；点了型号表里没有的型号；去处写不进去。
**型号那一句与转换那条路是同一句**——`tests/single_source.rs` 那一批盯的正是这种抄件。

**Blocked by:** 02 — 一张普通页出一叠样张

**Status:** resolved

- [x] 点了一个目录或一个 `.cbz`／`.zip`／`.rar`／`.7z`：一句话说样张只认一张图
- [x] 点了解不开的图：一句话说它解不开，**去处里一个文件都没有**（不留半成品）
- [x] 点了表里没有的型号：与转换那条路**同一句**拒绝，只有一处出处
- [x] 去处写不进去（父目录建不了、盘满）：回 `Err`、说得出是写不出去，不崩掉调用方
- [x] 四种的退出码与 `calibrate` 那条路同一个形状
- [x] `tests/single_source.rs` 那一批加一条：型号那句话没有第二份抄件
- [x] `cargo xtask gate` 三条全绿——这台 macOS 上读作「除了基线就红的那一条，没有新增的红」，见《数》与 Q995

## 落地记录

**本票做了什么。** 库、命令行、用例各一件：

1. **库那一侧三句拒绝**（`src/proof.rs`）。`write` 的次序改成：覆盖项那一问（`Candidates::new`，挪到头一行，收 Q1042）→
   「只认一张图」那一问（新的 `ensure_one_image`）→ 读盘 → 解码 → 编 → 建去处 → 落盘。
   - **点成别的东西**：盘上真在的目录、归档（`is_archive`，与转换那一趟同一把尺子）、透传文件（`decode::is_page`，Q1055），
     一句「样张只认一张图，X 是……：……」，目录与归档的后半截说该怎么改（点名里面那一页；要看一卷先跑 `--dry-run`，
     取自 spec《Out of Scope》）。**路径不在的不走这一问**，由读盘那一步说读不到（评审 Spec 轴提的，见下）。
   - **解不开的图**：解码那三种坏页，一句「X 解不开，是一张坏页：样张一张都没出」，后面接解码器的原因。
     为了让这一句打头又不把路径说两遍，「解 X 这一页」那层上下文从 `open_source_page` 挪到转换那一趟的调用处
     （`Compute::split_and_branch`）——`open_source_page` 里会出错的只有解码那一个 `?`，转换那一趟坏页那一格的原因逐字节不变；
     `open_source_page` 因此不再收 `source`。
   - **写不出去**：建去处、写每一张两处的上下文改成「样张写不出去：去处 X 建不出来」「样张写不出去：X 写不进去」。
2. **命令行一行代码没改**。四种本来就经 `main` 那一行 `Err` 收场（退出码 `1`、stdout 空、stderr 一句），与 `calibrate` 同形；
   型号那一句本来就只出自 `Profile::resolve`，三条路都经 `target_profile`。本票补的是钉住这两件的用例与文档里的指路。
3. **文档**：`write_proof` 补《说不出话的那几种》一节，是这件事的唯一出处——`proof::write` 与二进制侧的 `proof` 只指过去；
   `target_profile` 的文档补一句 `proof` 也走它。`CONTEXT.md` 没动：本票没有新概念，「坏页」「透传文件」都是既有的词。

**去处本来不在时建不建空目录——判：不建。** 落盘之前的每一种拒绝（覆盖项、只认一张图、读不到、解不开、撞上门、编不出来）
都发生在 `create_dir_all` 之前，05 起就是这个次序，本票照旧、并在五条库用例上各断言一次 `!sheets().exists()`。
另一条路（先建去处、出错再删）换不来什么，还要多一步收拾。写到一半才写不进去的那一种不在「不留半成品」里，见 Q1056。

**产物一个字节没变。** `git diff 52db0af -- tests/golden.rs tests/golden-snapshot.txt tests/counters.rs` 为空；
快照 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，
两批用例都在最终那一趟里跑绿（见《数》）。`run` 那一侧的改动只有上面那一层上下文的挪位；`calibrate` 一行没动。

### 按反跑过的几遍（改完都还原了）

先红后绿写出来的六条：目录那一支红在「读 卷: Is a directory」，透传文件那一条红在样张照出了一整叠（`002.dat` 里是一张 PNG），
解不开那一条红在「解 X 这一页: 解码: …」（没说解不开），Q1042 那一条红在「样张说解不开、转换说越界」，
写不出去那一条红在「建样张的去处 X: Not a directory」，不存在的路径那一条红在「卷1 是转换那一趟不当页的文件」。
另外按反的：

| 按反 | 结果 |
|---|---|
| 归档那一支关掉（`is_archive` 恒假） | 红：目录与归档那一条，`合集.cbz` 落到解码器上说「解 … 这一页: 解码: The image format could not be determined」 |
| 建去处挪到读图之前 | 红：解不开那一条，「解不开还建出了去处：[]」 |
| 残缺页也当解不开（`salvage.is_some()` 就拒绝） | 红：同一条的后半截，截断的那一张出不了样张 |
| 写每一张那一句换回「写样张 X」 | 红：写不出去那一条的第二种（参照那一张的名字被一个目录占着），「写样张 …/001.参照.png: Is a directory」 |
| 命令行上 `proof` 把 `Err` 吞成退出码 `3`、句子印到 stdout；型号那一句外面再包一层「样张」 | 红：两条进程用例都红——形状 `(Some(3), false, false)` 对不上灰阶测试图那一趟，stderr 比转换那一趟多出「样张 / Caused by」 |
| `src/main.rs` 里抄一份「未知型号「{}」」 | 红：`the_unknown_device_refusal_lives_in_one_place`，「长出了第二份」 |
| 家里那一句的「：」改成「，」 | 红：同一条，「少了『设备不在表里：挑一个面板相同的型号』那一截」 |

### 用例（两条闸门各多 9 条）

- `tests/proof.rs`（22 条到 28 条）：目录与四种归档（都是真卷）各说只认一张图、去处不建；透传文件（字节是 PNG，神谕先问实 `run` 原样搬了它）；
  解不开的三种坏页（神谕先问实三种在 `run` 里都是坏页）各说解不开、去处不建，残缺页照出；覆盖项越界配一张解不开的图，
  与 `run` 同一句（Q1042）；去处建不出来、一张写不进，各说写不出去与卡在哪儿；不存在的三个路径不许被说成「只认一张图」（反着钉）。
- `tests/exit_code.rs`（11 条到 13 条）：四种（目录、归档、解不开、型号、写不出去各跑一趟）收场与灰阶测试图写不出去那一趟同形
  （形状当场量，不抄）、各有自己那一句；型号认不出来时 `proof`、转换、`calibrate` 三趟 stderr 逐字节相同。
- `tests/single_source.rs`（4 条到 5 条）：型号那一句三截记号只在 `src/profile.rs`，家里三截都在。
- lib 与 bin 一条没多。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 52db0af`。

**收下的**：

- **不存在的路径被说成别的东西**（Spec）：`ensure_one_image` 只看路径的形状，敲错的 `卷1` 被说成透传文件、不在的 `合集.cbz` 被说成归档——
  比基点那句「读 X: No such file…」还差。改成只问盘上真在的东西，不在的交给读盘那一步说；补一条反着钉的用例（先红后绿，见上）。
- **「先全部编好再落盘」那张单子三处各写一遍、已经对不上**（Standards）：`write_proof` 的《说不出话的那几种》当唯一出处，
  `proof::write` 与二进制侧的 `proof` 只指过去。
- **「转换那一趟不当页的文件」有现成的词**（Standards）：拒绝那一句、文档、Q1055 都改成《成员》里的「透传文件」。
- **「解不开就是坏页」说宽了**（Standards）：坏页的第四种（字节读不出来）在样张上走读盘那一句。文档收窄成「坏页里解码那三种」。
- **一条用例里同一个名字写了两遍**（Standards，testing.md 第三条）：`002.dat` 与参照那一张的名字改从 `staged.source` 推出。
- **`Ending::shape` 交一个 `(Option<i32>, bool, bool)`**（Standards）：换成带名字的 `Shape`，比不上时看得出是哪一格。
  **`ONE_IMAGE`／`UNWRITABLE` 读着不像半句话**：改名 `TAKES_ONE_IMAGE`／`CANNOT_WRITE`。
- **用例文档里一句「样张从前先解码」是变更史**（Standards）：改成条件句。
- 自己收的一条：第一版把夹具的新构造器叫 `Staged::holding`——04 的评审正因为它撞上 `GateGroups` 里的 `holding` 才改掉过，
  这次又撞回去了。改 `Staged::placing`。

**驳回的**，各写理由：

- **Q1055 是范围外的新行为**（Spec）。驳：它就是票面没想到的第三种情形，按规矩记了停车场、走了推荐的那条；
  不拦的话它要么说「解不开」（没说到点子上），要么对一张改了扩展名的图照出一叠转换那一趟根本不编的样张。交接时点名告知。
- **四条新用例各自 `format!("{:#}", write_proof(..).expect_err(..))` 再查句子、路径、去处，提一个帮手**（Standards）。
  驳：与 05 驳回的那一条同一个理由——既有几条（互锁 ③、彩页越界）都是这个写法，每一处的失败措辞各说各的情形；抽帮手要么丢措辞，要么多参数。
- **`single_source.rs` 里 `carrying` 那段过滤是第五份，提成 `assert_only_home_carries`**（Standards）。驳：那四条既有用例是另几张票落的，
  本票照它们的形状加第五条；把四条一起改写是那个文件自己的事，不在本票。
- **票据没收尾**（两轴都提了）：评审时本票还在收尾之前，本节与《数》就是补上的那一份。

### 停车场

本票用了 Q1055–Q1069 里的两个：

- **Q1055**：透传文件也不是一张图，样张照「只认一张图」拒绝（票面只列了目录与归档）。
- **Q1056**：写到一半写不进去，已经落下的那几张留在去处里、不回头收；推荐等 Q998 拍板再看。

结转一条：**Q1042**（见《停车场结转》，照 ② 了结）。**Q998 看过、没碰**：它问的是去处里已有旧样张怎么办，Whose call 写的是拍板的人，不是本票；
Q1056 的推荐与它挂钩。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `ps-06.gate1.log`、`ps-06.gate2.log`、`ps-06.gate3-polish.log`，都在树外）。

这台机器是 macOS，闸门 1、2 在基线上就各红一条：`tests/concurrency.rs:685` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`，平台带来的，本票没碰它（Q995）。
因此照 02–05 的跑法：闸门 1、2 各加 `--no-fail-fast`、各用自己那个 target 目录，闸门 3 走 `cargo xtask gate 3`。
本票这一栏读作：**除了这一条基线红，没有新增的红；这一条在最终那一趟里照旧只红它自己。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_NFF_EXIT=101`；合计 **1035 通过 1 失败**；lib 239 / bin 423；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.22s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_NFF_EXIT=101`；合计 **920 通过 1 失败**；lib 239 / bin 308；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.48s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.85s` |

**基线**是 `proof-sheet/05` 落地那一刻（`31c63e1`，`52db0af` 是它的合并提交，树相同）：闸门 1 **1026 通过 1 失败**
（lib 239 / bin 423），闸门 2 **911 通过 1 失败**（lib 239 / bin 308），闸门 3 绿，红的是同一条。
**两条闸门各多 9 条，都在预期里**：`tests/proof.rs` 从 22 条到 28 条，`tests/exit_code.rs` 从 11 条到 13 条，
`tests/single_source.rs` 从 4 条到 5 条。lib 239、bin 423／308 一格没动。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（闸门 1 上 153.84 秒），`tests/counters.rs` 14 条全过；
`tests/golden-snapshot.txt` sha256 同上。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：`cargo fmt --check` 绿；`cargo clippy --all-targets` **告警 0 条**；
`cargo clippy --all-targets --no-default-features` **告警 0 条**；`cargo doc --no-deps` **告警 15 条**
（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 05 同数，逐条都是既有的 `links to private item`，本票没添一条）。

## 停车场结转

下面这一条由停车场转来（`proof-sheet/05` 记的，指明归本票的实现者判）。

#### Q1042 — 覆盖项越界又点了一张解不开的图：样张先解码、说「解不开」，转换那一趟先问覆盖项、说越界

- **From:** 票 `proof-sheet/05`
- **Kind:** 路过发现（`proof-sheet/02` 起就是这个次序）
- **Where:** `src/proof.rs` 的 `write`（`Candidates::new` 排在 `open_source_page` 之后）；
  `src/lib.rs` 的 `run`（`ensure_the_overrides_leave_a_candidate` 排在清点之前）；票 `proof-sheet/06`
- **Why it did not block:** 只在一份两处都错的请求上，两句拒绝谁先说；两条路都一个字节不写。
  本票要的那一半（彩色分支上的页也照转换那一趟拒绝越界的档位）不受这个次序影响。
- **What this ticket actually did:** 次序没动。`Candidates::new` 从灰度那一支里挪到分流之前，
  彩色面板上的彩页点了 `--bit-depth 8` 也照转换那一趟拒绝、一个字不差
  （`an_override_the_panel_cannot_write_is_refused_on_a_color_page_as_run_refuses_it`，按反跑过）。
- **Why it matters:** 「与转换那一趟说同一句」在这一角不成立：同一份请求，转换说越界，样张说解不开。
- **Options:** ① 照旧；② 覆盖项那一问挪到 `write` 的头一行、读图之前，与 `run` 同一个次序。
- **Recommend:** ②，归 06——它正在写「解不开的图」那一句，也要动 `write` 的开头；挪一行，外加一条两处都错的用例。
- **Whose call:** 06 号票的实现者
- **处置：** **06 号票判（2026-10-01）：照 ② 走。**`Candidates::new` 挪到 `proof::write` 的头一行，
  排在「只认一张图」那一问、读图、解码之前——与 `run` 同一个次序（覆盖项先于清点）。
  钉着的是 `an_override_the_panel_cannot_write_is_refused_before_the_image_is_decoded_as_run_refuses_it`
  （基准面板点 `--bit-depth 8`、卷里那一页是解不开的字节）：改之前红在「样张说解不开、转换说越界」，
  改之后两句逐字相同、去处没建。那一行旁边的注释指回本条。
