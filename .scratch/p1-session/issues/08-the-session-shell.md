# 08 — 会话骨架：无参数进入、三层配置、非终端退路

**What to build:** 不带任何参数敲 `tonefit` 就进入**会话**，左栏三层可改，按键退出。
带参数仍旧直接跑，现有行为一字不变。

三层的分界线画在**生命周期**上，不画在「哪几个 flag 长得像」上：
设备层错了是判定的依据错了，口味层错了只是这一趟不好看，范围层错了会写到别人的目录里。

会话画在 **stderr**（与现在的进度条同一个去处），**stdout 仍然只装报告**。
没有终端时给一条说得清的错误，而不是崩在 raw mode 里。

注意：`-p` **不需要**放宽必填。无参数在 clap 之前就被截住，带参数那一路的必填项照旧；
只有 `--preset` 供了 profile 那一种要放宽，而那归 07。

**Blocked by:** `page-geometry/01`–`05`

`page-geometry` 的四个开关（适配方式、裁边、拆分与阈值、阅读方向）要出现在**口味层**里。
这张票的口味层是**逐项列举**的，新开关不会自动流进来；页几何先于会话落地，因此写的时候
把它们一并列上即可，不必回头补。**阅读方向按口味层放**，不留逐卷口子——用户一批一批
处理，一批内同质，混批跑两趟。

**Status:** resolved

- [x] 无参数即会话；带参数即直接跑，现有行为一字不变
- [x] 左栏按设备层 / 口味层 / 范围层三块显示，各项可改
- [x] 范围层的路径输入**逐层补全**：只列打到的那一层，不递归、不建索引、不缓存（ADR 0009）
- [x] 范围层能勾掉已经打进去的卷
- [x] 会话画在 stderr，stdout 不被占用
- [x] 无参数但 stderr 不是终端 → 印「这里没有终端」加上现有那条必填项用法提示，退出码 `1`；
      clap 那条信息不被吃掉
- [x] 退出时终端恢复原状，不留在 raw mode 或 alternate screen 里
- [x] 终端库走 optional 依赖 + `tui` feature（默认开）；feature 关掉后仍能编译，
      无参数退回 clap 的必填项错误
- [x] 会话的状态机**脱离终端可测**：哪些键在哪个状态下有效有断言，不靠手点

## 落地记录

会话整件事在**二进制 crate 内**（`src/session.rs` 加 `src/session/` 三个模块），不进库——
spec 的《Seam》定死了这一条。库那一侧只多了一个访问器（`Profile::devices`，见下）。

### 入口：分岔在 clap 之前

```
main → execute
        ├─ without_arguments()  ← 无参数就走这条，排在 Cli::parse 之前
        │    ├─ stderr 是终端 → session::enter()
        │    └─ stderr 不是终端 → Err(no_terminal_error())  ⇒ 退出码 1
        └─ Cli::parse() → 一字未改的那一路
```

「有没有参数」问 `std::env::args_os().len() <= 1`，不问 clap——**要在它开口之前分岔**。
`-p` 因此不必放宽必填：带参数那一路根本走不到会话，三项必填的判定一动未动
（`the_volume_side_still_demands_an_output_root_a_profile_and_a_volume` 照旧钉着，
`tests/session.rs` 又在**进程**那一层验了一遍「参数只要有一个就不拐进会话」）。

关掉 `tui` 特性时 `without_arguments` 恒返回 `None`，无参数照旧落到 clap 的必填项错误上。

**没有终端那一条**印两件事：为什么没进会话（会话画在 stderr，而这一次 stderr 不是终端），
以及带参数那一路要什么——后半段**原样取自 clap**（`Cli::try_parse_from(["tonefit"])`
的 `error.render()`），不重抄一份。退出码 `1` 由既有的 `main` 那一支给出。

### 左栏：三层照 `07` 的分法，不另立一套

设备层与口味层**就是** `preset::DeviceLayer` 与 `preset::TasteLayer`——预设装的那两层，
不是另做的一份。`12`（会话里存预设）因此不必再做一次搬运。范围层只在会话里有
（`session::state::ScopeLayer`：输出根 + 卷清单），**不进预设**。

| 层 | 行 | 怎么改 |
|---|---|---|
| 设备层 | 型号 | ←→ 在内置表上转（`Profile::devices`） |
| | 感知可分辨级数、阈值 | ⏎ 打字。**要先挑型号**，否则没有面板可验（ADR 0002） |
| 口味层 | 适配方式、阅读方向、滤波器、位深、抖动、读取策略 | ←→ 转 |
| | 裁边、跨页拆分、逐页 | ←→ 转（没说 → 开 → 关） |
| | 拆分阈值、缓存预算 | ⏎ 打字 |
| 范围层 | 输出根 | ⏎ 打路径，⇥ **逐层补全** |
| | 每一个卷 | 空格 勾上／勾掉，`d` 整条删掉 |
| | ＋ 再打一个卷进来 | ⏎ 打路径，⇥ **逐层补全** |

键位：`↑↓`／`kj` 选，`←→` 换取值，**空格与 `⏎` 同义**（都是「就在这一行上动手」——
转得动的行上转一格，打字改的行上进编辑，卷行上勾／取消勾），`d` 删卷，
`q`／`Esc`／`Ctrl-C` 退出；编辑时 `⇥` 补全、`⏎` 收下、`Esc` 丢掉、`Ctrl-C` 照样退得出去。

**每一项都有一个「没说」的位置**，屏上印成 `默认（lanczos3）`／`自动（判据说了算）`，
转一圈回得到它。那是预设那一层 `Option` 的形状：「没说」与「说了一个恰好等于默认值的值」
在这一趟的 `Request` 上完全一样，差别只在存成预设时落地（停车场 Q58）。

**取值环是穷尽的 `match`，不是手抄的清单**（`next_fit`、`next_filter`、`next_dither`……）：
库那一侧给某个枚举加一个变体，这里当场编译不过。倒着转由 `back` 顺着走一圈算出来，
反向的表因此一份都不必写。

**换型号把设备层那两个覆盖项一起清掉**：它们是在上一块面板上量出来的（ADR 0002）。

### 状态机怎么做到脱离终端可测

不是「这批用例碰巧没开终端」，是**结构上开不起来**：

- `session::state` 不 `use` 任何终端库。按键在它自己那个 `Key` 枚举上说，
  crossterm 的键码翻译在 `session::translate`——**全仓库唯一认得 crossterm 键码的地方**，
  十几行一张对照表，自己有一条用例。
- 「哪些键在哪个状态下有效」由一个**纯函数** `Session::action(key) -> Action` 一处答完，
  用例直接问它，不必去数「按下去之后屏幕变成什么样」。
  `Action::Ignored` 是一个取值，不是遗漏——「编辑路径时上下键不动光标」这种规矩正是靠它说出来的。
- `Session::press` 只是「问一次 `action`，再把它做掉」。

按键那张表由 `which_keys_do_what_in_which_state` 分五段逐条问：转得动的行、打字改的行、
卷行、编辑路径时、编辑非路径时。另有 `only_the_quit_keys_leave_the_session`
（别的键按到底都退不出去）与 `an_interrupt_leaves_even_in_the_middle_of_typing`。

### 逐层补全

`session::complete::level` 只做一件事：**照打到的那一层 `read_dir` 一次，用完就扔**
（ADR 0009：不递归、不建索引、不缓存，源库只读）。分界是最后一个分隔符，
列出来的东西按用户打的写法拼回去（分隔符照他敲的那一个），目录带一个分隔符、
再按一次 `⇥` 就下到那一层；`⇥` 先补到**共同的那一段**为止——补到分岔口是补全该做的事，
替用户从几项里挑一项不是。

四条用例分别钉：只列打到的那一层（下一层一个都不出现）、前缀筛得动且不替用户挑、
**两次补全之间新建的东西第二次就列得到**（这一条钉的正是「不缓存」）、点不开的那一层是空清单。

### 终端进出

画在 **stderr**（`CrosstermBackend::new(stderr())`），stdout 一个字节都不写——
退出会话时把报告印到 stdout 归 `09`。

「退出时终端恢复原状」由 `Screen` 的 `Drop` 一处实现：正常退出、`?` 半路返回、恐慌展开，
三条路都经过它。恐慌那一条另挂一个钩子（只挂一次），让恐慌信息印在**还原之后**的屏幕上——
只靠 `Drop` 的话那几行会印进 alternate screen，然后随它一起消失。
进了 raw mode 之后建终端失败的那一支自己收（那时 `Screen` 还没造出来，`Drop` 顶不上）。

### 依赖

`ratatui = { version = "0.30.2", optional = true }`，`[features] default = ["tui"]`、
`tui = ["dep:ratatui"]`。库使用者 `default-features = false` 即可甩掉终端库；
`cargo build --no-default-features` 与 `cargo clippy --all-targets --no-default-features`
都干净。代价是关掉之后会话那批用例也跟着不编译（默认 83 条 vs 关掉 58 条）；
无参数那条用例按特性分成两半，关掉那一趟验的是「退回 clap 的必填项错误」（停车场 Q61）。

`09` 要的「终端库的测试后端快照」在 ratatui 这一侧是 `TestBackend`，本票已经用上了
（三条屏幕用例，含窄终端不恐慌那一条）。

### 库那一侧动的一行

`Profile::devices()` —— 内置表里的全部型号规范名。命令行上用户自己敲名字、认不出时
`resolve` 的错误把清单端出来；会话里没有那条错误，光标停在型号那一行左右键换一个，
那就要一份**枚举得出来**的清单。它不是新 seam，是既有类型上的一个访问器
（清单本来就从 `resolve` 的错误里出得来，这里只是不必先制造一次失败），
`the_enumerated_models_are_the_ones_the_table_lists` 把它与那条错误里的那一份拴在一起。

### 没做的

- **主区**（试算、执行、边跑边攒的报告）归 `09`。眼下那一格里只有一句「画在这里」，
  形状原样留着。
- **两级停与跑起来之后的只读**归 `10`：状态机里因此**没有**「跑起来了」那个状态，
  一个占位都没留。
- **逐页展开与左栏收起**归 `11`、**预设的存取**归 `12`、**一键出标定图**归 `13`。
- **会话跑不了任何东西**，因此没有「把三层拼成 `Request`」那个方法——那是 `09` 第一件要做的事。
- **`CONTEXT.md` 一个字没动。** 《会话》的口味层那一行仍列着六项；判成「改写已有词条」
  那一类，理由逐条写在停车场 Q57 的《`08` 撞上之后》。

### review 之后改的

`/code-review` 两轴各出了几条，收了这些：

- **阈值那一行会话另编了一套说法**（Spec 轴）。spec 的 Further Notes 写着「会话里显示时
  照报告的写法把来源原样带上来，不自己另编一套说法」，而屏上印的是
  `默认（内置，标定于 boox-poke6）`——数值没了，「其余面板未复核」也没了，
  而标定来源是阈值的一部分（ADR 0002）。改成照 `Threshold` 自己的 `Display` 印，
  与报告那一行逐字相同；合 profile 那一步也换成命令行与 `calibrate` 用的那个
  `target_profile`，三处解析出来的是同一块面板。
  `the_threshold_row_prints_the_source_the_report_prints` 钉住两处不走散。
  连带把左栏改成**折行**而不是切掉——那句话比这一栏宽，切掉等于把来源丢了。
  库里那句「命令行指定」在会话里读着不对，但改它会动到黄金基线，记进停车场 Q62。
- **`restore()` 拿 `?` 把两件事串成一条**（Spec 轴）。退不出 alternate screen 的那一次，
  `disable_raw_mode` 根本不执行，而验收要的是「不留在 raw mode **或** alternate screen 里」。
  改成两件都做完再把先出的那个错误交出去。
- **一条名不副实的断言**（Standards 轴）。`assert_eq!(TASTE_FIELDS.len(), 11)` 断的是
  `[Field; 11]` 这个类型本身，永远红不了，而那张单子的文档偏说它拴着
  「与 `TasteLayer` 一一对应」。换成真的交叉验：`preset::write` 把一份**说满了**的预设
  （`preset::every_field`，从测试模块提到模块层，没有 `..Default::default()`）写成 TOML，
  那两节里各有几个键，就断言两层各有几行。往 `TasteLayer` 加一个字段，这一条当场变红。
- **`layer()` 与 `shape()` 留着 `_`**（Standards 轴）。同一个模块刚写下「取值环是穷尽 match，
  加一个变体当场编译不过」，这两处却让新 `Field` 静默落进口味层、静默当成转得动的行。
  连同 `cycle()` 与 `take()` 的 `_ => {}` 一起展开成逐个变体。
- **两个类型名与 `CONTEXT.md` 撞了**（Standards 轴）。`Outcome`（Stay/Quit）与词条
  **收场**（`RunOutcome`）同名异义 → 改叫 `Exit`（`Stay`/`Leave`）；
  `Volume`（路径 + 勾）与词条**卷**（库里 `source::Volume`）同名异义 → 改叫 `Picked`：
  它装的是「用户点了它」这件事，连点不点得开都还没问过。
- **分隔符切分写了两遍**（Standards 轴，Duplicated Code）。`draw::tail` 收进
  `complete::name`，与 `split` 共用同一份分隔符表。
- **用例里 `cursor = rows().position(…)` 重复七次**（Standards 轴）。收成一个
  `#[cfg(test)] Session::focus_on(Field)`。
- **`Profile::devices` 承诺了用不上的 `ExactSizeIterator`**（Standards 轴，
  Speculative Generality）；`next_device` 每按一次键 collect 一遍内置表。两处都收掉。
- **`tests/session.rs` 在关掉 `tui` 的那一趟必挂**（自查）。那条无参数的用例按特性分成两半：
  开着时验「这里没有终端」加退出码 1，关掉时验退回 clap 的必填项错误。
  关掉特性那一趟因此不再是零行为断言，停车场 Q61 跟着改小。

一条没收，有理由：

- **`Field` 上七处 `match`（Repeated Switches / 一点 Shotgun Surgery）**（Standards 轴）。
  收成一张 `(Field, Layer, 名字, Shape)` 的表能省掉三处，但那张表是**手抄的**——
  而上面刚把 `_` 全部展开，图的正是「加一个变体当场编译不过」。另外四处
  （`cycle`／`typed`／`take`／`shown`）每一格的类型与默认值各不相同，表装不下。
  用编译期的穷尽性换掉手抄的表，这笔交易本模块认。

### 数

全量 **474 通过 0 失败**（基线 446），**17** 个测试二进制（多的是 `tests/session.rs`）。
`cargo fmt --check`、`cargo clippy --all-targets`（含 `--no-default-features`）干净，
`cargo doc --no-deps` 与 HEAD 同为 **14** 条既有告警。
**黄金快照未变、未重录**——会话不改变任何一趟的默认行为，命令行那一路一字未动。
停车场新增 Q58、Q59、Q60、Q61、Q62，并给 Q57 追记了《`08` 撞上之后》；一条都没了结。
