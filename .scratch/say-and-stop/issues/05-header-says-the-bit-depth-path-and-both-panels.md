# 05 — 抬头说出位深走的哪条路，标定来源写两块面板

**What to build:** **位深走的哪条路**：`Report` 带上这一趟是逐页判断还是整卷统一灰阶，抬头印一行，叫法与设置那一项同一个出处——
全卷都幂等命中的那一趟也说得出自己是哪条路。

**两道标定窗口**：标定来源的类型装得下几块面板；内置的那个印成「在 boox-poke6 与 kobo-libra-2 上实测，其他屏幕未验证」。点名的那一种不变。

会话配置视图里画质判定参数那一段取自报告抬头，跟着变：照 ADR 0019 决定第 13 条先改设计稿、重导，再让实现跟上。

**Blocked by:** `design-parity/01` — 设计稿追上实现已经对了的那几处（之后才重导）

**Status:** resolved

- [x] `render` 的用例：抬头的位深那一行两条路各一条；内置标定来源那一句说出两块面板
- [x] 设计稿里取自库的那几句改完、`npm run export` 重导，`npm run check` 绿；相关几串比整屏且绿
- [x] 黄金快照为本票的改动显式接受一次，diff 里只有本票那几行——**没有要接受的**：快照一个字节没动，理由见《落地记录》
- [x] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行（闸门 1、2 照旧只红基线那一条 Q995）

## 落地记录

**本票做了什么。**

1. **两道标定窗口（收 Q472）**（`src/profile.rs`）。`ThresholdSource::Calibrated` 从一个空变体改成 `Calibrated { devices: &'static [&'static str] }`，
   装得下几块面板；内置那个是 `["boox-poke6", "kobo-libra-2"]`，按窗口夹出的先后（外面那道、里面那道，measurements 的《位深盲测》《第十轮》）。
   `Threshold` 的 `Display` 把名字连成一串（`a 与 b`，三个以上 `a、b 与 c`），印成「画质门槛 5.123（在 boox-poke6 与 kobo-libra-2 上实测，其他屏幕未验证）」。
   点名的那一种（`Pinned`，「由你指定」）一字不变。型号名而不是 `Panel`、不带窗口与轮次，取舍见 Q1282。
2. **位深走的哪条路（收 Q637）**。
   - 库：`Report::envelope: bool`（`src/report.rs`），`run` 照 `Request::envelope` 原样填（`src/lib.rs`）；会话攒到一半的那一份同样填（`src/session/live.rs`）。
   - 措辞（`src/render.rs`）：抬头在选项冲突之后、画质分构成之前多一行（位置见 Q1280）——
     「整卷统一灰阶 关（逐页判断：每页各自选达标的最省空间档位）」／「整卷统一灰阶 开（每卷定一个统一档位，差异大的页单独定档）」。
     **项名与开、关两格只有一处出处**：`render::ENVELOPE_LABEL` 与 `render::envelope_value`，会话设置栏那一项（`Field::Envelope` 的项名与取值环那两格，`src/session/state.rs`）改读它们。
     括号里那半句是抬头自己的话，说的是开关（覆盖项顶死那一趟见 Q1279）。叫法的取舍见 Q1277。
   - 每一卷都幂等命中的那一趟照样有这一行：它读的是报告上那一格，不读卷级判定。
3. **会话屏跟着变的那一句**。照 ADR 0019 决定第 13 条先改设计稿：`design.html` 的 `thresholdText` 里内置那一句换成两块面板，`npm run export` 重导，
   `git diff --stat -- tests/fixtures/design` 只有 `config-narrow-h`、`config-narrow-h-l-h` 两串的 text 与 style 各一行（画质门槛那一行；窄屏上那一句截在「其」字，Q1283）。
   重导之后 Rust 那一侧 `under_ninety_columns_the_two_panes_take_turns` 先红，`profile.rs` 改完转绿。
   画质判定参数那一组的设备配置那一行读的就是 `Profile` 的 `Display`，`src/session/config.rs` 一字没动；位深那一行没进那一组（Q1278）。
4. **用例**（取值一律点名）。
   - `render` 3 条：`the_builtin_threshold_names_both_panels_it_was_calibrated_on`（反着钉：只点名 boox-poke6 的那一副不许回来）、
     `the_header_says_the_per_page_path_even_when_every_volume_was_skipped`、`the_header_says_the_envelope_path_even_when_every_volume_was_skipped`
     （夹具是每一卷都幂等命中的那一种趟，各自先钉住卷级一句判定都没有）。既有两条数行数的用例各多一行（18 → 19、11 → 12），两条写着旧句的断言换成新句。
   - `session::state` 1 条：`the_envelope_row_is_named_as_the_report_header_names_it`——`running`（没开）与 `envelope`（开着）两景各拿真的那一份抬头，
     设置栏那一行说出口的样子（项名加开、关）是抬头那一行的开头。
   - `profile` 1 条（lib）：`every_calibration_device_is_in_the_built_in_table`——标定来源里每个名字都是内置表里的规范名、各落在不同面板上。
   - `tests/idempotency.rs` 1 条，起真进程：`the_printed_report_of_a_run_that_skipped_every_volume_says_which_path_it_took`，
     `--no-envelope` 与 `--envelope` 各跑两趟，第二趟整卷跳过，stdout 上抬头那一行照样说出走的哪条路、不说另一条。
5. **黄金快照没有要接受的**：`tests/golden.rs` 链的是库、自己拼卷行与页行，读的是 `Candidate` 与 `Reason` 的 `Display`，
   不读 `Threshold` 的 `Display`、也够不着命令行抬头（`render::header` 在二进制里）。`tests/golden-snapshot.txt` 的 sha256 动手前后同为
   `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，`TONEFIT_ACCEPT_GOLDEN` 没设过。
6. **没改的**：`CONTEXT.md`（spec《词条随票落地》「其余几件是报告的措辞，词汇表不动」）；`src/session/config.rs` 与 `src/session/scene.rs` 一字没动；
   顶栏那一份「逐页判断／整卷统一灰阶」（Q1285）；详情栏两段长说明里的 boox-poke6（Q1284）；预览那一句缓存说明（Q1281）。

**按反跑过的**（`docs/agents/testing.md` 第一条）：

- 抬头那一行不读报告那一格（`envelope_line(false)`）：`--envelope` 那条 `render` 用例红。
- `run` 不填那一格（`envelope: false`）：起进程那一条红在 `--envelope` 那一趟（「没说自己走的哪条路」）。
- 会话攒的报告不填那一格：`the_envelope_row_is_named_as_the_report_header_names_it` 红在 `envelope` 那一景。
- 设置栏那一项另写一份叫法（「整卷统一档位」）：同一条红在 `running` 那一景。
- 标定来源拼错一个名字（`kobo-libra2`）、换成同面板的 `boox-poke5`：`every_calibration_device_is_in_the_built_in_table` 各红一次。
- 新用例落地前各自先红：两块面板那一条（旧句）、两条路那两条（编译不过，报告上还没有那一格；补上之后红在抬头没有那一行）、
  窄屏两串（设计稿重导之后、`profile.rs` 改之前）。

### 评审收了什么、驳了什么

两轴各一个只读子代理，看 `git diff 91298d0`（未提交的工作树）。

**收下的**：

- Standards + Spec：「每页各自选达标的最省空间档位」在抬头那一行与卷级「逐页」那一句各写一份（故事 35「只在措辞那一层写一次」）。收下：抽成 `PER_PAGE_RULE`，两处共用，卷级那一句逐字节不变。
- Standards：Q1278 标题用了旧称「位深」。收下：改成「灰阶档位走哪条路那一行」。
- Standards：同一样东西字段叫 `devices`、用例名叫 model。收下：用例改名 `every_calibration_device_is_in_the_built_in_table`。
- Spec：宽屏（120 列）设置栏那一行截在「（在 boox-poke6 」之后，与旧句截出来的一模一样。收下：写进 Q1283（标题、Where、做了什么），推荐改成另开「截断带省略号」的口子。
- Standards：按反那几遍要写进《落地记录》。收下：见上一节（评审时票还没填）。

**驳回的**：

- 缩放方式、裁白边、拆分跨页三行的项名照旧各写一份（Shotgun Surgery）：票面的硬约束点名的是位深那一行；那三行的字面在本票之前就各写一份，收拢它们不在本票。
- `Report` 又从 `Request` 抄一格（Data Clumps）：`fit`、`crop`、`split`、`white_align_limit` 都是这个写法，本票照它多一格。
- `Calibrated` 装字符串（Primitive Obsession）：取舍记在 Q1282，用例钉着名字都在内置表里。
- 起真进程那一条（Spec 记作轻度蔓延）：留着。派活说明的硬约束「全卷都幂等命中的那一趟也说得出」要 `run` 填那一格、`main` 一路印到 stdout，`render` 用例够不着这两段；按反跑过（`run` 不填就红）。
- `enumerated` 接得住三块以上、「各在不同面板上」那一句断言（Spec 记作小幅超出）：票面要的就是「装得下几块面板」；后一句是那句话说得对的前提。
- 两维都点死那一趟抬头与卷级两句对不上（Spec 记作像是错的）：照旧，记在 Q1279；顶栏那两个词同样只看开关。
- 故事 34「画质判定参数那一段……说出位深走哪条路」（Spec 记作只做了一部分）：照旧，Q1278 待拍板。

### 数

最终状态跑的**那一趟**（评审收完、`cargo fmt` 过之后，依次闸门 1 → 闸门 2 → 闸门 3 → polish；日志 `ss-05.gate1.log`、`ss-05.gate2.log`、
`ss-05.gate3.log`、`ss-05.polish.log`，都在树外）。这台机器是 macOS，闸门 1、2 在基线上就各红一条：
`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`（Q995，平台带来的，本票没碰它）。
本栏读作：**除了这一条基线红，没有新增的红。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | `GATE1_EXIT=101`；合计 **1137 通过 1 失败**；lib 253 / bin 473（1 条 ignored）；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.14s`（`concurrency`，红的是 Q995 那一条） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | `GATE2_EXIT=101`；合计 **999 通过 1 失败**；lib 253 / bin 335；末行 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 48.06s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 7.34s` |

**本票添 6 条用例**，两条闸门各多 6 条：lib 里 `profile` 1 条（252 → 253）；bin 里 `render` 3 条、`session::state` 1 条；
`tests/idempotency.rs` 1 条（45 → 46）。

**两道钉子逐格没动**：

- `tests/golden.rs` 2 条全过（闸门 1 上 174.52 秒、闸门 2 上 150.60 秒）；`tests/counters.rs` 14 条全过。
- `tests/golden-snapshot.txt` 的 sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`。
- 设计快照只动了窄屏那两串的画质门槛那一行（text、style 各一行）；`npm run check` 绿，闸门 1 里比整屏那几条全绿。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：

- `cargo fmt --check` 绿；
- `cargo clippy --all-targets` 绿、零告警；
- `cargo clippy --all-targets --no-default-features` 绿、零告警；
- `cargo doc --no-deps` **告警 15 条**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数）。

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q637 — 报告抬头不说位深走的哪条路，全卷幂等命中那一趟因此说不出自己是 `--envelope` 还是逐页

- **From:** 票 `two-pass-rework/11`
- **Kind:** story 29 的一个边角
- **Where:** `src/render.rs` 的 `header`（抬头列 profile、适配方式、裁边、跨页拆分、互锁、判据三行）；`src/report.rs` 的 `Report`
  （抬头那几项都是它身上的字段）；`src/session/draw/overlay.rs` 的「这一趟的前提」覆盖层
- **Why it did not block:** 处理过的每一卷都说得出：卷级那一行 `VolumeVerdict::Envelope`／`PerPage` 只在各自那条路上出现，
  两趟混不到一起比（spec 的 story 29）。说不出的只有一种趟：**每一卷都幂等命中**——那时没有一卷有卷级判定，
  抬头又不带这个开关。那一趟本来就一页没做，读的人要的多半是「为什么全跳过」而不是「走的哪条路」。
- **What this ticket actually did:** 卷级那一行改说法（「无（默认逐页）……要卷级齐整开 --envelope」），抬头没动。
- **Options:** ① 抬头加一行「位深 逐页」／「位深 上包络」——要给 `Report` 加一个字段、`run` 填它、`header` 印它，
  会话的前提覆盖层跟着多一行（那在 `src/session/draw/`，本票不进）；② 不加，全跳过的那一趟本来无卷级判定可读；
  ③ 只在全部卷都跳过时补一句。
- **Recommend:** ①，等 tpr/01、tpr/02 合完再做——适配方式、裁边、拆分三项都在抬头上，第四个改产物的开关不在，
  是抬头那张单子漏了一项；③ 是为一种趟另开一条规则，不值。
- **Whose call:** 拍板的人（可并进 tpr/02，它正在动会话那几副的措辞）
- **处置：** **`say-and-stop/05` 落地（2026-10-02）：照票面收，走选项 ①。**`Report::envelope` 由 `run` 照 `Request::envelope` 填，抬头在选项冲突之后、画质分构成之前多一行「整卷统一灰阶 关（逐页判断：…）」／「整卷统一灰阶 开（…）」，项名与开、关两格与会话设置栏那一项同一处（`render::ENVELOPE_LABEL`、`render::envelope_value`）；每一卷都幂等命中的那一趟照样有它（`render` 两条、起真进程一条）。会话那一侧没另添覆盖层的行：「这一趟的前提」覆盖层已退场（ADR 0019 决定第 7 条），这一行也没进画质判定参数那一组（Q1278）。另见本票《落地记录》与 Q1277–Q1281、Q1285。

#### Q472 — `Threshold` 的 `Display` 仍只说 boox-poke6，而今天钉住取值的是 kobo-libra-2 与本仓夹具

- **From:** `metric-recalibration/07`（阈值 5.5 → 5.123）
- **Kind:** 路过发现（屏上与报告上那一句话，兑不了 `CONTEXT.md` 给的承诺）
- **Where:** `src/profile.rs` 的 `impl Display for Threshold`——`ThresholdSource::Calibrated`
  那一支印的是「盲测标定于 boox-poke6，其余面板未复核」
- **Why it did not block:** 那句话**没有说错**：5.123 确实落在 boox-poke6 那次盲测夹出的
  [4.930, 6.022) 里，「其余面板未复核」也照旧成立。
- **What this ticket actually did:** 一个字没动那句话——改措辞超出「只改一个数」。
  但它现在**说得不全**：今天真正把取值钉死的是**另外两样**，一样都不在那句话里——
  ① 第十轮真机在 **kobo-libra-2** 上夹出的 [5.079, 5.134)（那才是窄的那一道）；
  ② 本仓那张满幅渐变夹具的 5.113（取点的下界，一个回归证人）。
  `CONTEXT.md` 的《阈值》写着「标定来源因此是它的一部分，报告里与数值一同印出」，
  《尚未确立》又写着「报告里印出的是数值**加标定来源**，读的人能自己判断它对手上那块板成不成立」——
  **读的人今天判断不了**：他看到的来源是 boox-poke6，而收窄它的是另一块面板上的判读。
- **Options:** ① 记下；② `ThresholdSource::Calibrated` 带上面板与轮次
  （`Calibrated { panel, round }`），`Display` 印全——**那是给类型加一格，不是换措辞**；
  ③ 只把 `Display` 那句话改长，来源仍是一个枚举格。
- **Recommend:** ②。这一票量出来的事实是「窗口有两道、各在各的面板上」，
  而 `ThresholdSource` 今天只装得下一道。③ 改完那句话还是编的——枚举里没有第二道的位置，
  下一次再多一道又要重写一遍。②要动对外形状，因此不在本票。
- **Whose call:** 拍板的人（`ThresholdSource` 要不要装得下不止一道窗口）
- **处置：** **`say-and-stop/05` 落地（2026-10-02）：照票面收，走选项 ② 的「装得下几块面板」那一半。**`ThresholdSource::Calibrated { devices }`，内置的是 `["boox-poke6", "kobo-libra-2"]`，印成「在 boox-poke6 与 kobo-libra-2 上实测，其他屏幕未验证」；不带轮次与窗口（Q1282）。本仓那张满幅渐变夹具的 5.113 是取点的回归证人、不是一道窗口，照旧只写在 `DEFAULT_THRESHOLD` 的文档上。设计稿那一句先改、重导（Q1283：设置栏那一行截断）；详情栏两段长说明仍只说 boox-poke6（Q1284）。
