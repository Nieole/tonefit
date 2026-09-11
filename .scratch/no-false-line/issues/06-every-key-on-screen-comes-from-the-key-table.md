# 06: 屏上每一个键都出自按键表

**What to build:** 屏底那一行与 `?` 那张表的键早已收进一处（键从按键表出，长短两份措辞
只有一处出处）。屏上**还有六处**字面提到键（核于 2026-09-11）——票面原先只数到「第三处」，另四处是
`overlay.rs` 的 `NOT_RUN_YET`、`terminal.rs` 两处 `complain("还没跑过：先按 t 试算或 x 执行……")`、
`footer.rs` 的 `resuming_line`（「那时按 x……按 a……按 s 收尾」）。头两处：一趟都没跑过时，总览块与报告区各说的那一句
——「还没跑过。t 试算 · x 执行」「按 t 试算：只算不写，报告照出。按 x 执行：写到输出根。」

那两句里的 `t`／`x` 连同措辞都是**字面串**，而按键表里写着同样的字。
换一个键位要改两处，而屏上没有一处会红。

屏底那几句**说明**同理：`⇥` 在按键那一行上是「⇥ 补这一层」，在下一行的散文里是
「按 ⇥ 列出这一层」——同一个键两处措辞。

六处都改成从按键表取。那两句不是「此刻按什么」（它们是一趟都没跑过时那一格里的一句话），
因此取的是**键与写法**，不是屏底那一行的那份短措辞。

收停车场的 **Q190**。

**Blocked by:** 01 — 夹具喂全事件流那一半（同一批快照）

**Status:** resolved

- [x] 「还没跑过」那两句里的键与写法**从按键表取**，不再是字面串
- [x] 屏底那几句说明里提到的键，**写法**取自按键表
- [x] 换一个键位时**屏上每一处一起变**，有一条用例钉着（改一处措辞，别处当场红）
- [x] 屏上印出来的字**一个不多不少**——这一票换的是出处，不是措辞
- [x] 那两句仍只在「一趟都没跑过」那个阶段露面，露面条件一格没变
- [x] 三条闸门全绿

## 落地记录

基底 `a87f0f2`。**屏上的字一格没动**：`yielding.rs` 那批空闲屏快照、`overview`／`report`／`footer`
的成屏快照一张没重录。换的是**出处**——七处字面串改成问按键表，外加 Q190 点名而票面没数的第八处。

### 票面的「六处」核出来是七处，加 Q190 的一处是八处

票面数的六处：总览块 `START_KEYS`、报告区 `NOT_RUN_YET`、前提那一张 `NOT_RUN_YET`、
`terminal.rs` 两处 `complain`、屏底「续做」那一句；票面另提的「按 ⇥ 列出这一层」是第七处；
Q190 原文点名的「g 把它交回给最新那一卷」（屏底「跟随停了」那一句）是第八处。
**`report.rs:212` 那一段从未入过按键表**——派活时说的 predicted（p4/07 收了一半）不成立，
它是手抄的字面串，本票改了它。

### 接法：键与写法从表取，措辞留在各句

一处新家、三手读法，都在 `src/session/draw/keys.rs`（模块文档新开一节
《屏上顺口提到一个键的那几句散文》）：

| 读法 | 答的是 | 谁问 |
|---|---|---|
| `spelt_for(keys, want)` | 派得出这件事的那几个键怎么写，不带措辞；同义键一个不漏 | `footer::Asked::key_of`（`按 ⇥ 列出这一层`、`g 把它交回…`）、`footer::resuming_line`（`x`／`a`／`s`） |
| `Starters { dry, run }` + `starters(session)` | 起一趟的两个键怎么写；问的是 `key_table()` **每一块**，不是眼下这一块——取值栏摊着时那两句照旧在屏上 | `overview::idle`、`report::not_run_yet`、`overlay::not_run_yet`、`terminal::not_run_yet` |
| `Starters::named()` | 两个键各配上它起的那一趟叫什么（`t 试算`、`x 执行`），怎么接进句子归各句 | 上面四处里的三处（报告区那两句更长，自己拼） |

**「续做」那一句要预告一个还没到的阶段**：跑着时屏底就说「那时按 x／a／s」，而按键表三个公共读法
都只问 `self.stage`。`state.rs` 因此加了一条**只读**的 `pub fn stage_keys(stage)`（`every_key()`
逐个问 `stage_action(key, stage)`），按键表本身一格没改；footer 以 `Stage::Deciding(pressed)` 问它。
这一手放在表旁边而不是开可见性，理由在 **Q641**。

**`overview()` 从 `Option<&Live>` 改收 `&Live`**：一趟都没跑过那一支拆成 `overview::idle(&Starters, width)`，
由 `draw::main_pane` 按 `live` 分派——`overview()` 拿不到会话，问不了按键表。牵动 `overview.rs`
七处测试调用点去掉 `Some(..)`（tpr/01 在同一文件里改 `pass_name` 与快照，两处相隔上百行；
`overview.rs`／`overlay.rs` 里它那两块我一行没动）。

**派不出键时那一截不说**：`Starters` 两格都是 `Option`，照 `footer::listed` 补不动时不说「按 ⇥」的先例——
overlay 与 terminal 各多一句从未上屏的兜底措辞（**Q644**）。`terminal.rs` 那两支本身就是「到不了但留着说一句」
的兜底，只动了两处 `complain` 与所需的 `use`，外加一条私有 `not_run_yet` 与一条用例。

**可见性**：`draw.rs` 把 `mod keys` 开成 `pub(super)`，`Starters`／`starters`／`named` 是 `pub`
——terminal 那一句是画法之外唯一一处读这一份的地方；`spelt_for` 仍 `pub(super)`。

### 钉住「换一个键位屏上每一处一起变」的用例怎么写

按键表在用例里换不了键位（`Session::action` 不可注入，spec 也说一处 seam 都不新增），因此分成三半：

1. **不抄的那一半**——各句的拼法**喂一副假键**（`Starters::faked(Some("r"), Some("w"))`、
   `[(Key::Char('r'), Answer(..))]`），句子里得是假键。写死 `t`／`x` 的话当场红；真按键表上恰好就是
   `t`／`x`，拿它喂进去分不出「问出来的」与「抄上去的」。七个模块各一条。
2. **一起变的那一半**——`draw.rs` 的 `every_mention_of_the_starting_keys_is_the_key_tables_own`：
   拿 `keys::starters(&session)` 答的键去问整屏，总览块那一句、报告区那两句都得挨着它。
   今天它必然绿；换键位那一刻，哪一处仍是手抄的它就红。评审指出头一版「`{key} 试算` 次数 == `试算` 次数」
   耦合了无关措辞（兄弟票印一个不挨键的「试算」就误红），改成只问那三句。
3. **不许抄回去的那一半**——spec《Testing Decisions》点名的 `tests/single_source.rs`：
   `the_keys_the_screen_mentions_come_from_the_key_table`，与前三条同一个三问形状。新添 `code_only`
   ——砍掉 `#[cfg(test)] mod tests` 起的整段、去掉注释行——**快照与 doc comment 里出现那几句是记录不是出处**，
   代码里出现才是。六个记号在基底代码上各中一处（`overview`、`report`、`overlay`、`terminal`、`footer` 三处），
   改后一处不中。这一条在 `tui` 特性外面，闸门 1、2 都跑得到。

### 数

三条闸门在评审之后的**最终状态**上跑的（`cargo xtask gate`，随后 `cargo xtask polish`，
同一 detached 脚本，两份日志各自 `EXIT=0`）：

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **944 通过 0 失败**；lib **249** / bin **378** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **805 通过 0 失败**；lib **249** / bin **239** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.54s`` | 干净，一条告警都没有 |

**闸门 1 涨 9**：bin 涨 8（370 → 378，全在 `tui` 里面——`keys` 两条、`overview`／`report`／`overlay`／
`footer`／`draw`／`terminal` 各一条），`tests/single_source.rs` 涨 1。**闸门 2 只涨那 1**（bin 239 一格不动：
新添的用例都在画法与终端那一侧，`state.rs` 的 `stage_keys` 在特性外面但本票没给它单独的用例——
它由 footer 那一条透过真按键表问到）。基底那一趟本票没有单跑，370／804 是按新添用例数推的。
lib 249 两条都没动。

**闸门之外那一遍**（`cargo xtask polish`）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**（``warning: `tonefit` (lib doc) generated 15 warnings``），
**一条没多**。

**头一趟红过两条**：`overlay`／`terminal` 那两句「或」的拼接丢了空格、`才有` 前多了一格——
两句屏上到不了、快照抓不到，是各自那条「喂假键」的用例抓的。修掉之后没再红过。

### 停车场结转

**Q190 了结**：屏上手抄的键——「还没跑过」那两句里的 `t`／`x`、屏底说明里的 `⇥`／`x a s`／`g`——
八处都从按键表取。《已了结》索引表那一行原样留着。

**新记四条**（本票 id 块 Q641–Q648，用掉四个），都在《待处理》：

- **Q641** — 「续做」那一句要问一个还没到的阶段，读表那一手 `stage_keys` 放进了 `state.rs`（vs 开两个可见性让 `keys.rs` 自己拼）。推荐现状。
- **Q642** — 屏上还有三处散文提着键，票面没数进去：`overlay()` 抬头 `Esc 关`、`report::GONE`（`⇥ 换一卷，Esc 收起回卷表`）、`terminal::open` 的 `Esc 回目录表`。两处在兄弟票正改的区域，推荐合流后另立小票。
- **Q643** — 那几句里的「试算／执行」与报告区那两句长话与 `says` 逐字相同，票面只让取键；推荐作为一张措辞票（与 Q626 同类）连 `overview::run_name` 一起收。
- **Q644** — 派不出起一趟的键时那几句怎么说：选了「不提键」，两句兜底措辞从未上屏；vs `expect`。推荐现状。

### 评审

`/code-review` 跑过一遍，两轴并行（**只读，没碰工作区，一条 cargo 都没跑**），worktree 路径与基底
`a87f0f2` 一起交出去的。Standards 提 3 硬 + 6 判断，Spec 提 5；**收 7 条**：

- **Standards 硬①「`not_run_yet` 插进了 `expand` 的 doc comment 中间」**——真的，`expand` 丢了摘要行。挪到它上面，`expand` 的文档复原。
- **Standards 判断「三处同形的键配词」**——`overlay`／`terminal`／`overview` 各拼一遍 `[dry 试算, run 执行]`。收成 `Starters::named()`。
- **Standards 判断「四处测试夹具各抄一份 `Starters{r,w}`」**——收成 `#[cfg(test)] Starters::faked(dry, run)`。
- **Standards 判断「`following_line` 两处调用各问一遍 `key_of(Follow)`」**——改成 `following_line(asked, stopped)` 自己问。
- **Standards 判断「`spelled` 与 `spelt` 同词两拼」**——后者改名 `spelt_for`，文档说清两者差在问的对象。
- **Standards 判断「同一段理由在七处 doc 复述」**——各处收成引 `keys` 模块文档那一节的小节名。
- **Spec (a)「spec 点名的 `tests/single_source.rs` 没动」**——补上，见上第 3 半。
- **Spec (c)「成屏用例耦合无关措辞」**——收，见上第 2 半。

**没收的**：Standards 硬③「`Starters` 未进 CONTEXT.md」——`Asked`／`Says`／`Wording` 都没进，画法内部类型免登有先例；
Spec (b) 两条（`None` 分支兜底措辞、`overview()` 拆 `idle`）——都是必要的一步，前者已记 Q644。
Standards 判断「`Asked::key_of` 是 Middle Man」——与既有 `Asked::on` 同形，留。
