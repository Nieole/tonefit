# 02 — 输入行让得开，滚轮在输入行上不乱跑

**What to build:** 输入行摆不下时右端那几件**从前往后让**——先 `⇥`、再 `C-w`，`⏎` 与 `Esc` 最后——缓冲与光标恒在；全让光了缓冲才截，**前面放 `⋯`**、露出末尾（01 那一种）。输入行开着时滚轮只管补全框：有候选挪候选，**没候选一件事都不做**，不再落到底下那一块。设计稿先改、重导，再改实现（ADR 0019 决定第 13 条）。

**Blocked by:** 01 — 三种截法收在一处

**Status:** resolved

- [x] 设计稿改完、`npm run export` 重导，`npm run check` 绿；快照对实现只读；重导之后既有快照的变动只落在本票牵到的那几处
- [x] 80 列「添加路径」那一屏：缓冲末尾与光标看得见，右端让掉的是 `⇥`（必要时再让 `C-w`），比整屏且绿
- [x] 长搜索词那一串：同一副让法，比整屏且绿
- [x] 缓冲长到右端全让光仍放不下：前面 `⋯`、露出末尾与光标，导一串钉住
- [x] 输入行开着、补全框没候选时滚一格：底下那一块一格不动，导一串钉住；有候选时照旧挪候选
- [x] 留一条反着钉的断言：80 列上缓冲末尾不再被右端那几件盖住
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（已分开跑完；1、2 仅在范围外基线 Q995 红，3 绿，见《数》）

## 不做会怎样

窄终端上添加长路径时看不见自己打到哪儿；滚轮在输入行上挪走底下那一块的光标，而屏底被输入行占着、那句回话看不见。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1061 — 「添加路径」那一景改成按过 `Tab` 之后，80 列那一屏的缓冲被右端那几件盖住：`~/Comics/大友克洋` 之后的 `/` 与光标都看不见

- **From:** 票 `design-parity/01`
- **Kind:** 路过发现的设计稿缺陷（本票重导之后才露出来）
- **Where:** `tests/fixtures/design/snapshots/add.80x24.text.txt` 末行（` 添加路径  ~/Comics/大友克洋[Tab → 补全] [C-w → 删一段] [⏎ → 确定] [Esc → 取消] `）；
  `design.html` 画输入行那一段（右端那几件压在缓冲上）；实现那一侧的输入行同一副（`the_add_scene_matches_its_design_snapshot_wide_and_narrow` 逐格相等）；
  `CONTEXT.md` 的《让位》《输入行》
- **Why it did not block:** 设计稿与实现两边一样，比对照样逐格相等；翻过来是输入行那一段的摆法，不碰本票的十处。
- **What this ticket actually did:** 照 Q797 的处置让那一景走真的补全，缓冲从 `~/Comics/`（10 格）变成 `~/Comics/大友克洋/`（18 格），
  80 列上摆不下右端那四件之前的全部，设计稿与实现都让右端那几件压住了缓冲的末尾。没动摆法。
- **Why it matters:** 输入行里用户正在打的那一句被截掉、光标看不见，而《让位》只写了卷列表、确认条、总览怎么让，输入行摆不下时谁先让没有写。
- **Options:** ① 照旧（已落地）；② 输入行摆不下时右端那几件从前往后让掉（缓冲与光标恒在）——设计稿与实现一起改，词汇表《让位》添一句；
  ③ 「添加路径」那一景换一层短一点的目录，让 80 列摆得下——只是把这一屏躲过去。
- **Recommend:** ②，并进 `design-parity/02`：那一票本来就要把输入行右端那几件改成按用途从按键表派下来，同一段摆法。
- **Whose call:** 拍板的人（设计稿与《让位》）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：输入行摆不下时右端从前往后让（先 `⇥`、再 `C-w`），缓冲与光标恒在，全让光了缓冲前面放 `⋯`（《让位》已写）。

#### Q1092 — 搜索那一行右端多了 `C-w` 一件，80 列上右端从第 37 格起摆：搜索词宽过 35 格就被盖住（Q1061 同一个毛病）

- **From:** 票 `design-parity/02`
- **Kind:** 本票带出来的边角（设计稿与实现一样）
- **Where:** `design.html` 的 `drawFooter` 打字那一支（缓冲按 `W - 40` 裁、右端贴右摆）；`src/session/shell/footer.rs` 的 `BUFFER_KEEPS_CLEAR`；
  `snapshots/search.80x24.text.txt` 末行；停车场 Q1061
- **Why it did not block:** 票面明写搜索那一行摆 `C-w`；导出的几屏搜的是「海贼」，一格都没盖。翻过来是输入行那一段的摆法，与 Q1061 一起定。
- **What this ticket actually did:** 右端从 27 格变成 42 格（`[C-w → 删一段] [⏎ → 跳到结果] [Esc → 取消]`），80 列上起点从第 52 格挪到第 37 格；
  缓冲从第 2 格起、按 40 格裁，搜索词宽过 35 格（十八个汉字上下）时末尾与光标被右端盖住。120 列上起点在第 77 格，缓冲裁到 80 格，同样会撞，只是要更长。
- **Options:** ① 照旧，与 Q1061 一并定；② 输入行摆不下时右端那几件从前往后让（Q1061 的 ②），缓冲与光标恒在。
- **Recommend:** ②，并进 Q1061 一起拍板——两条是同一处摆法，分开定会摆出两套规矩。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 并入 Q1061，随它走（拷问定案 2026-10-02：同一处输入行摆法）。

#### Q1107 — 输入行开着而补全框没开时，滚轮落到底下那一块上（设计稿 `moveBy` 一路落下来），与 `↓`／`↑` 在那儿一件事都不做不一致

- **From:** 票 `design-parity/06`
- **Kind:** 照设计稿走的那条路（票面只说补全框开着时挪候选）
- **Where:** `design.html` 的 `moveBy`（第 1582 行起：输入行那一支只在 `S.input.cands` 有东西时收走，否则落到配置视图／每页结果／`listGo`）；
  `src/session/view.rs` 的 `Session::wheel`（第 1188 行，文档《滚轮在哪一块上做哪一件》）；`keymap::TABLE` 滚轮那一行（块是 `ANY_BLOCK`）
- **Why it did not block:** spec 只说「输入行开着时挪补全框里的候选」，补全框没开时怎样没说；票面结转的 Q979 ② 写的是
  「在输入行上走 `InputLine::step`」（照它读，没有候选时不动），设计稿则一路落下来。两种读法里设计稿是权威（ADR 0019 第 13 条），照它走；
  翻过来是设计稿 `moveBy` 加一行、`Session::wheel` 改一行、重导一串。
- **What this ticket actually did:** 照设计稿：补全框有候选时 `InputLine::step` 一格三个；没有候选时这一格交给 `place_cursor`，
  挪的是输入行底下那一块的光标——搜索那一行底下的卷列表（并暂停自动滚动，而屏底被输入行占着，那句回话看不见）、
  改一项设置的值那一行底下的设置栏、起名那一行底下的预设栏。没有导出一串钉它（钉住的是一个多半是顺手落下来的行为）；
  `typing.rs` 的 `the_wheel_steps_through_the_completion_box` 末段钉着它，翻过来时那几行跟着翻。
- **Why it matters:** 改值那一行上滚一下，设置栏的光标挪到别的项上，输入行却仍在改原来那一项（`Purpose::Setting` 带着那一项）——屏上两处说的不是同一项。
- **Options:** ① 照设计稿：落到底下那一块；② 输入行开着时滚轮只管补全框，没开就一件事都不做——与 `↓`／`↑` 在输入行上一样
  （滚轮一格就是那两个键按三下）：设计稿 `moveBy` 输入行那一支改成 `if (S.input) { if (cands) …; return; }`，重导添一串，
  `Session::wheel` 那一支去掉 `!line.candidates.is_empty()` 那半句。
- **Recommend:** ②。输入行开着时焦点在它身上，打字的键、`↓`／`↑` 都不碰底下那一块，只有滚轮碰，说不出道理。
- **Whose call:** 拍板的人（设计稿）
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：输入行开着、补全框没候选时滚轮一件事都不做，设计稿 `moveBy` 先改。


## 落地记录

起点与交付 HEAD 均为 `hs-02` / `d252f9a`（已含批准的 01）；保留并完成上次中断留下的认领与 `export.js` 序列草稿。改动全部未提交，供协调者转移。

- 设计先行：`design.html` 输入行按实际显示宽度给缓冲、光标与间隔留位，右端按 `Tab`、`C-w`、`⏎`、`Esc` 次序让；全让光仍放不下才从前面放 `⋯`。回话仍占右端的位置。`moveBy` 在输入行上收走滚轮，没有候选也不落到底下的块。
- `export.js` 导出 15 条本票 80×24 序列：路径六屏（从 `G`、`o`、打 `Comics/`、实际 `Tab` 补全起步）、搜索四屏、无候选滚轮四屏（路径列表、运行中搜索、设置栏、预设栏）、有候选滚轮一屏。路径与搜索都覆盖逐件让位、全让光、不够时保留末尾。序列的正负自检同时核光标、末尾与不该再占位的提示。
- `npm run export` 重导，`npm run check` 逐字节相同，`npm test` 四条通过。已有导出只改变 `add.80x24` 与 `add-narrow-F1-j-k-Escape` 的屏底文字/样式；没有手改导出、黄金快照或 `.gitattributes`。
- Rust `footer.rs` 删除固定 `W - 40` 裁剪预算，提示照按键表的用途次序让位；缓冲调 01 的 `columns::elide_at(..., Some(Keep::End))`，不另写裁剪循环。`view.rs` 的滚轮在输入行上统一交给 `InputLine::step`，空候选不碰底下的块，也不暂停自动滚动。
- `terminal.rs` 三条新用例从终端输入入口走完上述序列，比整屏的字、色与修饰；另留末尾 `/▏` / `画▏` / `卷▏` 不许消失、已让掉的提示不许再出现的负断言。`typing.rs` 旧滚轮用例改为打字清空候选后底下列表仍不动，有候选时每格三项的断言保留。
- 代价：设计多一个保留末尾的省略 helper，画法按实际宽度量提示；没有依赖、特性、边界变更或新抑制。词汇表与模块 spec 已覆盖这副规矩，不改它们。

### 实际红 → 绿

1. 原设计跑已有草稿序列，`npm run export` 红于 `input-path-yield-0` 缺 `~/Comics/大友克洋/`：旧屏底在 `大友克洋` 后直接画 `[Tab → 补全]`，`/` 与光标都不在（`/tmp/hs-02-design-red.log`）。先修设计、重导并 check 绿，再动 Rust。
2. 新路径整屏用例在旧 Rust 上实际 0 通过 / 1 失败：第 23 行第 28 格实际 `[`、期望 `/`（`/tmp/hs-02-path-red.log`）；启用让位与共享末尾裁剪后 1 通过（`/tmp/hs-02-path-green.log`）。搜索同一副实现随后整屏及负断言通过。
3. 新空候选滚轮用例在旧守卫上红于 `input-add-empty-wheel`：第 6 行第 2 格实际 `›`、期望空格，光标被滚到了输出目录（`/tmp/hs-02-input-red.log`）。输入行无条件收走滚轮后，该组 13 通过（`/tmp/hs-02-input-green.log`）；候选每格三项的旧用例单独 1 通过（`/tmp/hs-02-candidates-green.log`）。
4. 首跑整仓闸门发现草稿 `input-path-yield-0` 没有步，两个夹具完整性检查红；没有放松检查，改为从 `fresh` 喂真实添加与补全步骤，再重导。最终两条夹具检查随闸门的 bin 套件通过。

守卫按反与反转实跑，三次均实际 0 通过 / 1 失败、退出 101，随后全部还原：

| 改坏哪一处 | 实际红证据 |
| --- | --- |
| 提示摆不下 `>` 改 `<` | `input-path-yield-0` 屏底出现过早的 `⋯`，期望 `~`；`/tmp/hs-02-yield-mutation.log` |
| 缓冲保留末尾 `Keep::End` 改 `Keep::Start` | `input-search-yield-45` 第 23 行第 2 格实际 `漫`、期望 `⋯`；`/tmp/hs-02-tail-mutation.log` |
| 恢复「有候选才收走滚轮」旧守卫 | `input-add-empty-wheel` 底下光标又挪到输出目录；`/tmp/hs-02-wheel-mutation.log` |

最终整组会话验收 **329 通过、0 失败、1 个既有忽略**（`/tmp/hs-02-session-green.log`）；默认 bin 全组 **486 通过、0 失败、1 个既有忽略**。三次反转之后均回到实际绿的实现。

所有 Cargo 命令都经 `python3 /tmp/tonefit-honest-screen-cargo.py cargo ...`，且设 `RUSTC_WORKSPACE_WRAPPER=/tmp/hs-02-rustc-workspace.sh`（仅 `exec "$@"`）。共享 target 的 workspace 指纹与其他票隔离。

闸门 1、2 只剩已批准排除的 Q995：macOS 上 `tests/concurrency.rs:755` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，重命名检出力断言实际 `0`、期望 `96`，与 01 交付记录同一失败。未修改、弱化或跳过该例；闸门在它处停下，后续集成二进制没有跑。按这项已知基线例外结票，「三条全绿」保持未勾。

## 数

日志均在仓库外。三条闸门分别调用，没有用一条遇红早停的命令冒充跑完三条。

- 闸门 1（`/tmp/hs-02-gate1.log`）：红，合计 **760 通过 1 失败；lib 255 / bin 486**。
  - `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.58s`
- 闸门 2（`/tmp/hs-02-gate2.log`）：红，合计 **616 通过 1 失败；lib 255 / bin 342**。
  - `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.31s`
- 闸门 3（`/tmp/hs-02-gate3.log`）：绿。
  - `Finished dev profile [unoptimized + debuginfo] target(s) in 2.38s`
- polish（`/tmp/hs-02-polish.log`）：`fmt --check`、两种构建 clippy、`doc --no-deps` 四条全绿。
  - 默认 clippy：`Finished dev profile [unoptimized + debuginfo] target(s) in 5.86s`
  - 无默认特性 clippy：`Finished dev profile [unoptimized + debuginfo] target(s) in 4.90s`
  - 文档：`warning: tonefit (lib doc) generated 15 warnings`（既有私有链接告警，未增加）；`Generated /private/tmp/tonefit-honest-screen/02/target/doc/tonefit/index.html`
  - 编译 / clippy 的既有 `src/main.rs:1193` `fetch_update` 弃用告警未动。初次 fmt check 提示两处换行，按 rustfmt 调整后 polish 通过。
- `npm run export`：30 份快照、188 串期望屏；`npm run check` 逐字节相同；`npm test` **4 通过**（最终日志 `/tmp/hs-02-design-export.log`、`/tmp/hs-02-design-check.log`、`/tmp/hs-02-npm-test.log`）。
- `git diff --check` 通过。
