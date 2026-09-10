# 08: 三处模块文档指对

**What to build:** 三处文档在说假话，而 bin crate 不进 `cargo doc`（与 lib 同名，cargo 跳过它），
**一条告警都不会报**：

1. **画法那一层的模块文档三处指着一个改过名的模块**——指的东西读得懂
   （「摆不下时谁让位」只有一处），错的只是名字：那个模块多半改过名，文档没跟着走；
2. **`super::press`／`super::expand` 那十来处指的其实是 `terminal::press`／`terminal::expand`**
   （会话那几个模块与画法那一层共十来处）。它们**本来就指不到东西**——
   那两样住在终端那个模块里，从会话那一层看 `super::press` 就已经落空；
3. **预设栏那句「为什么摆在这一格」给的理由是假的**：它写着
   「这句话摆在这一格而不是屏底那一句里，是因为**屏底那一格不折行**，一条长路径会被切掉」
   ——而屏底早就折行了，还会按折出来的行数往下长。**结论仍成立**（文件位置该摆在预设那一格），
   假的是理由：真理由是屏底那一格是**三样争一格**的地方。

三处一起订正。**代码一行不动，闸门数一格不变**——与本批别的几件分开落，
不把一次订正混进一次结构改动的 diff（这正是当初记下它们时不顺手改的那条理由）。

收停车场的 **Q128**、**Q177**、**Q178**。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 画法那三处指对那个模块此刻真正的名字
- [x] `super::press`／`super::expand` 那十来处改成指得到的写法；**一处不漏**
- [x] 预设栏那句理由改写成真的：屏底折得下来，但它是三样争一格的地方
- [x] **代码一行不动**：`git diff` 里只有文档注释
- [x] **闸门数一格不变**（三条各自的通过数与落地前逐个相同）
- [x] 三条闸门全绿

## 落地记录

**代码一行没动。**`git diff` 里只有 `///`、`//!` 与 `//` 三种注释——自查那条命令与它的输出在下面
《代码一行不动，怎么验的》。

### 一、画法那三处：`[`room`]` → `[`yielding`]`

`src/session/draw.rs` 的模块文档第 10、15、37 行。那个模块此刻叫 `yielding`
（`draw.rs:79` 的 `mod yielding;`），同一份文档第 39 行早就写着 ``[`yielding::title`]``、
`draw.rs:125` 写着 ``[`yielding::config_width`]``——**改过名的是模块，落下的是这三处**。

全库 ``[`room`]`` 只此三处（`git grep -n '\[`room`\]' -- src/`）。别处那十来个 `room`
是**函数参数名**（`draw/table.rs`、`draw/pages.rs`、`draw/directories.rs`、`draw/footer.rs`、
`draw/yielding.rs`、`session/columns.rs`），一个没动。

### 二、`super::press`／`super::expand`：**40 处**，不是「十来处」

票面与 spec 都写「那十来处」。**实测 40 处**，口径是这一条：

```
$ git grep -oE "(super::)+(press|expand)" -- src/ | wc -l
40
```

`press` 36、`expand` 4；分布 `state.rs` 32、`draw/report.rs` 4、`run.rs` 2、`live.rs` 1、
`draw/overlay.rs` 1。**「一处不漏」是这条 grep 判的，不是那个数判的**——落地后同一条数出 `0`。
票面那个数记停车场 **Q510**。

**为什么取 `crate::` 全路径，而不是把 `super::` 补够。**那 40 处里有 5 处在
`#[cfg(test)] mod tests` 里（`state.rs` 两处、`run.rs` 两处、`draw/report.rs` 一处），
`super::` 要数几级**随所在模块而变**：`draw/report.rs` 的用例里要三级，`state.rs` 的用例里要两级，
模块层面又是另一个数。补 `super::` 等于当场造出**第二种数错的写法**——而这一票收的三条停车场里
有两条正是这么坏的。全路径一处一个样，搬家也不错。

**为什么最后落成代码跨，而不是方括号链接。**头一趟落的是 ``[`crate::session::terminal::press`]``，
路径对了，**rustdoc 照旧判它 unresolved**：`press` 与 `expand` **私有于 `terminal`**
（`terminal.rs:132`、`:412`，都没有 `pub`），而 intra-doc link 按编译器那套**可见性**解析——
从 `session::state` 看不见 `terminal` 里的私有项。要让方括号真的成立只有一条路：把那两个函数
抬成 `pub(super)`，而那是**改代码**，票面第 4 条明令禁止。

因此 40 处一律落成**代码跨** `` `crate::session::terminal::press` ``：指路，不是超链接。
一条解析不了的方括号链接本身就是一句假话，而这一效力的名字就叫「屏上没有一句假话」。
这一层张力记停车场 **Q512**。

### 三、预设栏那句理由：结论没动，理由换成真的

`src/session/draw/picker.rs` 的 `presets` 文档。**结论一格没动**——那份文件的位置照旧摆在
预设那一格。换掉的是理由：

| | 原来 | 现在 |
|---|---|---|
| 理由 | 屏底那一格**不折行**，一条长路径会被切掉 | 屏底那一格是**三样争一格** |

**真理由是代码里核实的，不是照抄票面。**`src/session/draw/footer.rs` 的 `footer` 文档写着
让位次序，从让得最早的数起：① 说明那一行——**整句丢，不截前缀**；② 要说的那句话贴着底，一行不让；
③ 按键那几行一行不让（退出会话在里面）。屏底**早就折行**，还会按折出来的行数往下长
（`draw/yielding.rs:157` 的 `footer_height`，上限 `footer_max_rows`）——所以一条长路径搁在那里，
坏的不是「被切掉」，是**整句随时会没掉**。而预设这一格里它是常驻的头一行，没有第二样东西跟它争。

### 代码一行不动，怎么验的

```
$ git diff -U0 -- src/ | grep -E "^[+-]" | grep -v "^[+-][+-]" | grep -vE "^[+-]\s*(///|//!|//)"
（无输出）
```

左边那一半取出 diff 里的增删行，右边那一半滤掉三种注释——**剩下的就是代码行，一行都没有**。

### 「指得到」这一条，闸门验不了，所以由记录来验

`cargo doc` 跳过 `tonefit` 那个 bin（与 lib 同名），而 `src/session/` 整棵都在那个 bin 里——
**三条闸门与 `cargo xtask polish` 一条都够不着这些 intra-doc link**（票面第一句就是这么说的）。
三处假话能活到今天，靠的正是这个缺口。因此证明摆在这里，四条：

**① 链上每一级的声明在哪一行**

```
$ git grep -n "^mod session;" -- src/main.rs
src/main.rs:22:mod session;
$ git grep -n -B1 "^mod terminal;" -- src/session.rs
src/session.rs-68-#[cfg(feature = "tui")]
src/session.rs:69:mod terminal;
```

**② `press` 与 `expand` 真的在 `terminal` 里**（列首无缩进 ＝ 文件顶层项）

```
$ git grep -n "^fn press\|^fn expand" -- src/session/terminal.rs
src/session/terminal.rs:132:fn press(
src/session/terminal.rs:412:fn expand(session: &mut Session, running: &Running, action: Action) {
```

`terminal.rs` 里唯一那个内层模块是 `mod tests`（`:643`），排在两者**之后**——因此这两个函数
是 `terminal` 的直接子项，不被任何内层模块挡着。

**③ 从 `crate::session::` 这一级出发，`terminal` 看得见**

```
$ git grep -n "mod terminal" -- src/
src/session.rs:69:mod terminal;
```

全库只此一处，没有第二个 `terminal` 把它遮住。`src/session.rs` 就是 `crate::session` 那一级的
模块文件，`src/session/terminal.rs` 是它的子模块文件（edition 2024 路径）。
`mod terminal;` 是 `session` 的**直接子项**，`state`／`live`／`run` 都是 `session` 的后代，
因此 `terminal` 这个**模块**对它们可见——不可见的只是模块**里面**那两个私有函数（见上面第二节）。

**④ 拿 rustdoc 真跑一趟**（这一条是前三条证不到的）

```
$ cargo rustdoc --bin tonefit -- --document-private-items
```

走默认 `target`，热的时候几秒。两趟逐条 diff：

| | bin 告警 | `unresolved link to crate::session::terminal::*` |
|---|---|---|
| 落地前（方括号链接那一版） | **82** | **21** |
| 落地后（代码跨） | **61** | **0** |
| 评审两条修正落进去之后 | **61** | **0** |

消掉 21 条，**新增 0 条**（三趟的 warning 行整表比对过，后两趟逐条相同）。40 处里另 19 处在 `//` 普通注释或
`#[cfg(test)] mod tests` 里，rustdoc 本来就不检——这一层也是这趟才量清楚的。
`[`yielding`]` 那三处与 picker 里新写的 ``[`super::footer::footer`]`` 一条告警都没添。
**这个缺口本身**（bin 那 61 条没人看）记停车场 **Q511**。

### 数

**闸门与 polish 都在最终代码上跑过，一共跑了三趟。**注释改一次就重跑一次——
**凭一份对不上代码的绿收票是假绿**（`no-false-line/07` 立的那一条）。三趟各是：
① 方括号链接那一版；② rustdoc 探针证出方括号解析不了、改成代码跨之后；
③ `/code-review` 那两条修正（`picker.rs` 那段重写）落进去之后。
**三趟的数逐字相同**，下表是第三趟、也就是最终代码上的那一趟。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **884 通过 0 失败**；lib **243** / bin **349** |
| `cargo test --no-default-features` | 同上（Doc-tests 那一格） | **753 通过 0 失败**；lib **243** / bin **218** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s`` | 干净，一条告警都没有 |

**三个数与落地前逐个相同**（884 / 753 / 干净，lib 243 + bin 349、lib 243 + bin 218）——
本票一条用例都没加没减没改，改的全是注释。

**闸门之外那一遍**（`cargo xtask polish`，跑在最终代码上）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**（``warning: `tonefit` (lib doc) generated 15 warnings``），
**一条没多**——本票一个 `src/` 库文件都没碰，改的全在 bin 那一侧的 `src/session/`。

### 复选框逐条

- 第 2 条的「那十来处」**实际是 40 处**（Q510）；「指得到的写法」取的是**指路**不是超链接，
  理由与那条张力见 Q512。两条都勾了，注在这里。
- 第 5 条「闸门数一格不变」按上面《数》那张表对账，三条逐个相同。

## 停车场结转

**收的三条**（`Q128`、`Q177`、`Q178`）本票全部了结，索引表那三行改指到这里：

| 收的 | 它说的 | 落在本票哪一节 |
|---|---|---|
| **Q128** | 画法里那几处 `super::press`／`super::expand` 指的其实是 `terminal::press`／`terminal::expand` | 《二、`super::press`／`super::expand`：**40 处**》 |
| **Q177** | 预设栏的模块文档写着「屏底那一格不折行」，而屏底那一格折行 | 《三、预设栏那句理由》 |
| **Q178** | `src/session/draw.rs` 的模块文档三处指着 ``[`room`]``，而那个模块叫 `yielding` | 《一、画法那三处》 |

Q128 与 Q178 原文都说「连同仓库里那十来处」——**那个数是估的，实测 40 处**，见 Q510。

新记**六条**（`Q507`–`Q512`，都留在《待处理》里，每条都带 `- **处置：**` 那一行）：

| 新记的 | 一句话 |
|---|---|
| **Q507** | 「屏底那一格不折行」那句假前提，票面收的那一处之外另有四处（`render.rs` 一处、`state.rs` 三处） |
| **Q508** | 会话那几个模块里另有 **13** 处 `super::resuming`／`drive`／`translate`／`erase_preset`／`store_preset` 同病 |
| **Q509** | 画法那一层的用例模块里，6 处 `super::super::…` 少数了一级 |
| **Q510** | 票面把那一类写成「十来处」，实际 40 处 |
| **Q511** | bin crate 的文档链接一条都没人验，`cargo rustdoc --bin tonefit` 一跑就是 61 条告警 |
| **Q512** | 「指得到」在私有项上做不到：票面第 2 条与第 4 条互相顶着 |

**Q507／Q508／Q509 是撞见了没顺手改的**——票面写死「三处一起订正」，理由是不把一次订正
混进一次结构改动的 diff。三条都是同一类「指不到」，都够开一张自己的票。

## 评审

`/code-review` 跑过一遍，两轴并行（**只读，没碰工作区，一条 cargo 都没跑**），
worktree 路径与基点 `440f6d7` 一起交出去的。两轴共提 5 条，**收 3 条、驳 1 条、留 1 条**：

- **Standards ①「单一出处」（硬，收）**：`picker.rs` 新写的那段把 `footer.rs` 已有的让位次序
  逐字抄了一份（`CLAUDE.md`《文档写作》第 4 条）——**本票自己的 Q507 推荐的正是「立成单一出处」，
  而落地又添了第五份抄件**。改法：`picker.rs` 只留结论与指路，三样各是什么、次序为什么这么排
  一律引 `[`super::footer::footer`]`，不抄第二份。
- **Standards ②「Q508 那个数是错的」（收）**：本条头一版写「9 处」，实际 **13 处**——
  漏了 `run.rs:492`／`:547`（`super::super::drive`）、`run.rs:1005`（`super::resuming`）、
  `state.rs:5273`（`super::erase_preset`），四处都在 `mod tests` 里，
  正好落在我头一趟那两条扫描（`src/session/*.rs` 模块层 + `src/session/draw/*.rs`）中间的缝里。
  **停车场自己也归「屏上没有一句假话」管**：数改成 13，并把数它的那条命令记进 Q508，
  连同 Q508／Q509 的分界（少一级 vs 少一段）写清。这一条与 Q510 是同一个毛病的两次犯——
  **一个估出来的数配一条要求穷尽的话**。
- **Spec ②「假前提被删掉，不是被反驳」（收）**：验收第 3 条写的是
  「**屏底折得下来**，但它是三样争一格的地方」，而头一版只落了后半句。
  改写后那一句里带上「那一格折得下长路径」，前半句在场。
- **Standards ③「索引表三行去掉了反引号」（驳）**：索引表《去处》那一栏里，
  指向 `X《停车场结转》` 的行**本来就不带反引号**（Q64、Q72、Q77、Q61、Q68、Q79、Q40、Q36
  都是这个样子），带反引号的是指向一张票、不带《停车场结转》的那种写法。本票三行照的是前者。
- **Spec ①「`state.rs:1096` 是同一句假理由的第二处」（留）**：属实，已记 **Q507**。
  票面写死「三处一起订正」，理由是不把一次订正混进一次结构改动的 diff——**不顺手改是票面要的形状**。
  评审自己也写「倾向可接受，但口径请拍板人确认」，这一句原样转给拍板的人。

两轴都独立复核过「代码一行不动」与「40 处一处不漏」，结论与本票一致。
评审复核不了的是要跑 cargo 的那几个数（闸门、`cargo doc`、rustdoc 那两趟）——那几个在《数》里。
