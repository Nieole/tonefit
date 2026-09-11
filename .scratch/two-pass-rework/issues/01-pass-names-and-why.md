# 01: 遍名换成说得出在做什么的名字

**What to build:** 跑起来之后，横条上那个词说得出这一遍在**做什么**——`对指纹`、`读图定档`、`按档写出`，而不是「幂等这一道 / 第一遍 / 第二遍」。按 `?` 调出来的那张表多一节，三个遍各配一句**为什么非做不可**。词汇表里 `段` 与 `遍` 枚举的是同一批三样东西，并成一条。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 横条上三个遍名换成语义名；开卷那一支不变
- [x] `?` 那张表从「键位表」扩成「键位 + 这一趟在做什么」，三遍各一句为什么
- [x] 屏底一个字不动——它的单一职责是按键提示，不摆常驻散文
- [x] 横条那一行在主区最窄那一档上仍摆得下
- [x] **不显示活的当前阶段**：第一遍满核并行，同一刻不同线程在不同阶段，没有单一答案
- [x] `CONTEXT.md` 里 `段` 与 `遍` 并成一条，留 `遍`，把「步按它划、卷级计时按它分」并进去
- [x] 成屏文字的快照用例跟着改，逐字符对得上

## Comments

**派活前核过（2026-09-11）：票面现状与代码一致，可按票面派。** 一条 Facts 给实现者：
`src/session/draw/overlay.rs` 约 602 行有既有用例断言 `!screen.contains('z')`——`?` 表加「为什么」那一节散文时
会撞上它，那条断言的意图要先读懂再动。

## 落地记录

### 横条上那个词（`src/session/draw/overview.rs`）

`pass_name` 仍是那个词的**唯一出处**，三个字面量换成 `对指纹` / `读图定档` / `按档写出`，`None => "开卷"` 与
`Some(_) => "这一遍"` 两支一字不动，枚举名 `Fingerprint`/`First`/`Second` 不动。函数升成 `pub(super)`——`?` 那张表
从这里取词，不另抄。宽度：对指纹 10 → 6 格（幂等这一道 → 对指纹），读图定档与按档写出各 6 → 8 格（第一遍 / 第二遍 → 四个字），最宽的那个因此从 10 缩到 8；当前卷那一行的
横条因此比从前**早两列**让位（`with_a_bar` 的规则没动），让位用例的阈值跟着挪：`[9, 8]@45` → `[11, 8]@47`、
让掉当前卷那一条从 44 列挪到 46 列，全局那一条仍是 44 在、43 让。

**最窄那一档（30 列）**：正文本来就比它长（32 → 34 格），横条让完仍从中间省略——头上「 本卷 卷三 · 」13 格、
尾上「 1000/3000 步」13 格，两头都在，被吃掉的只有中间那个词。**新旧词切出来的屏一模一样**
（`yielding.rs` 那两张 `⋯` 快照一格没动），40 列那一档 `本卷 卷三 · 按档写出 1000/3000 步` 仍整条摆得下。

两条新用例：`the_pass_on_the_bar_says_what_it_does`（三遍 + 开卷各问一遍成屏文字），
`the_narrowest_tier_keeps_both_ends_of_the_volume_row_for_every_pass`（30 列上三遍各留两头、不画横条）。
两处用例的夹具是 `probe::a_run_walking(failures, pass)`——`a_run_in_flight` 从此是它 `Some(Pass::Second)` 那一支，
`?` 表那一侧同一份（code-review 收的：从前两个模块各抄了一份尾巴）。

**「摆得下」读成「两头在、词从中间省略」，与基底那一屏逐字相同**——不是词可见。30 列上这一行的固定部分
（「 本卷 」6 + 卷名 ≥ 4 + 「 · 」3 + 空格 1 + 步数 12）已经 ≥ 26 格，留给那个词的至多两格，任何措辞都放不下；
要词可见只能让掉卷名。是不是该那样，记 Q628。

### `?` 那张表（`src/session/draw/overlay.rs`）

`Overlay::Keys` 的正文从 `keys(session)` 变成 `keys_and_passes(session)`：键位那几组照旧，末尾空一行接一节

```
 三遍 · 本卷那一行上那个词各在做什么
   对指纹     不读一遍源字节，答不出这一卷上一趟做过没有；对得上整卷跳过
   读图定档   看过像素才知道哪一档够用：解码、缩放、算判据，一个字节都不写
   按档写出   等全卷读完才写：一页失败整卷进隔离，而哪页失败要解过才知道
```

行形与键位那几组**出自同一处** `section(title, column, rows)`（code-review 收的：从前两截各写一遍，「同一副」只是
承诺），但两截各对各的列。词出自 `overview::pass_name`，「为什么」是 `WHY: [(Pass, &str); 3]` 一张表——三遍是哪三个
只在这里点名（`Pass` 非穷尽）。
**恒在**（一趟都没跑过时也在，停车场 Q627），**不随当前遍变**：`the_key_table_does_not_follow_the_pass_being_walked`
钉了走读图定档与走按档写出时整屏逐字相同。80 列上三行各一行，不折。

第 602 行那条 `!screen.contains('z')`：它防的是「按键表派不出的键出现在表上」，`z` 是哨兵字母。三遍那一节没有
键那一列，但也在同一屏上——注释里点明，那三句全是汉字，一个字母都不许溜进去。断言本身没动。

### 快照（手改，逐字符）

- `overview.rs` 2 张、`report.rs` 10 张、`yielding.rs` 1 张：`第二遍` → `按档写出`，行尾少两格。
- `overlay.rs` 4 张 80×24：只差滚动条一格（表长了五行，滑块短一格，`█` → `║`）；新增 `THE_KEY_TABLE_ENDS_WITH_THE_PASSES`
  一张滚到底的。
- 屏底（`footer.rs`、`keys.rs`）一个字没动；决策点 `x` 那句「接着做第二遍（第一遍不重算）」因此仍是旧名——Q626。

### `CONTEXT.md`

《管线》里的《段 (Segment)》删掉，并进《进度》的《遍 (Pass)》：三个新名各括注《管线》旧名词条，
「步按它划、卷级计时也按它分」并入，加一句「报的是这一卷此刻走进了哪一遍，不是哪条线程此刻在哪个《阶段》」。
《步》换新名；《卷级计时》「按段分开」→「按《遍》分开」、《阶段》「与《段》不是一回事」→「与《遍》」，
《管线》表下那段引文「计时按段分……汇总不占一个段」→「按遍分……不占一遍」，《进度》引文「预扫算进去的那几段」
→「那几遍」（并词条的必然后果，引用不改就悬空——后两处是 code-review 找出来的）；《进度》引文里
「第二段那一截按每张源页两张输出页预告」→「按档写出那一截」（核过 `lib.rs::volume_steps`：那个数是写出那一遍的）。
第 128–133、178 行一字未碰——**其中第 133 行《段式迟滞》末句「与《段 (Segment)》说的那三样不是一回事」因此悬空**，
那一条归 tpr/11（它正在按 ADR 0018 改写《段式迟滞》），交接记在 Q625。

### 停车场

Q625（《管线》三条旧名词条、第 133 行悬空的《段》引用、`report.rs`/`progress.rs` 注释里的旧名与「段」、《覆盖层》
没提新那一节——建议 tpr/11 合并后一次扫齐）、Q626（决策点 `x` 那句与横条同屏两套词——建议随 Q625 一起改）、
Q627（三遍那节跑前也在——建议保持）、Q628（30 列上「摆得下」读成两头在、词省略——建议保持，不让卷名）。

### code-review 之后

标准轴一条硬违规（本记录里那句宽度写错，已订正）、四条气味（两截行形合成一处 `section`、两份夹具合成
`probe::a_run_walking`、两条冗余 `use`、一条用例拆成两条）全收；spec 轴找出的两处「段」残留改掉、第 133 行交接。
改动全在 `tui` 后面的 `src/session/draw/{overlay,overview,probe}.rs`：闸门 2 不编译它们；收完重跑
`cargo test --bin tonefit` **374 通过**（拆出一条用例，370 → 374）、`cargo xtask gate 3` 绿、
`cargo clippy --bin tonefit --tests` 干净、`cargo fmt --check` 过。下面《数》那张表是 review 之前那趟闸门的，
bin 那一格当时是 373。

### 数

三条闸门跑满，三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **938 通过 0 失败**；lib **249** / bin **373** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **804 通过 0 失败**；lib **249** / bin **239** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.89s`` | 干净 |

闸门 1 的 bin 涨三格（370 → 373）：本票那三条新用例，全在 `tui` 后面；闸门 2 的 bin（239）与 lib（249）一格没动。

`cargo xtask polish`：

| | 结果 |
|---|---|
| `cargo fmt --check` | 绿 |
| `cargo clippy --all-targets` | 绿，``Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.32s`` |
| `cargo clippy --all-targets --no-default-features` | 绿，``Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.89s`` |
| `cargo doc --no-deps` | 绿，`warning: \`tonefit\` (lib doc) generated 15 warnings`——与基线的 15 条一条不多 |
