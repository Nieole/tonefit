# 20 — 三个开关补上反面，口味层默认值只有一处出处

**What to build:** 两笔命令行表面的账：

- **裁边、拆分、逐页三个布尔开关的覆盖是单向的**：预设关得掉，命令行开不回来。
  补上 `--crop` / `--split` / `--no-per-page` 三个反面，让覆盖是双向的。
  `--help` 上每一项要说清「它和它的反面」；
- **口味层每一项的默认值只有一处出处。** 它们原本只写在命令行那一侧的参数上，
  会话因此拼不出同一份请求。收成一处，两边拼出来的是同一份。

收停车场的 **Q55**、**Q67**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 三个开关各有反面，套了一份关掉它的预设之后命令行开得回来
- [x] 三对开关互斥，同时给两个时当场说得清
- [x] `--help` 上每一项说得出它和它的反面
- [x] 口味层默认值只有一处出处，命令行与会话拼出来的请求逐格相同
- [x] 「没说」与「说了一个恰好等于默认的值」仍是两件事
- [x] 三条闸门全绿

## 六条验收开工那一刻各是什么状态

派活说明判「三个反面一个都不在」——**核下来一字不差地成立**。但票面第二笔账
（口味层默认值）**指错了地方**：Q67 说它们「只写在命令行那一侧的参数上」，
而那一半在基底 `93f26cb` 上早就收进 `TasteLayer` 了（`3618d5c`，p1-session 那一批）。
真正剩下的第二份出处在票面没提的地方。

| 验收 | 开工那一刻 | 这一票做了什么 |
|---|---|---|
| ① 三个开关各有反面 | **不成立**：全文没有 `ArgAction::SetFalse`、没有 `overrides_with`，字段只有 `no_crop` / `no_split` / `per_page` 三个单向的 | 补齐三个反面，覆盖改成双向 |
| ② 三对互斥，当场说得清 | **不成立**：一个反面都没有，也就无从互斥 | clap 的 `conflicts_with`，不自己写第二份说法 |
| ③ `--help` 说得出它和它的反面 | **不成立**：`--preset` 的长帮助反而白纸黑字写着「只说得出一个方向」 | 六项各写自己的反面，另改三处被推翻的旧说法 |
| ④ 默认值一处出处 + 两边逐格相同 | **一半成立**：`TasteLayer` 那几个方法与 `Session::request` 已经在，`Cli` 读的就是它们；**剩下的第二份在会话左栏** `spell_flag(self.taste.crop, true, …)` 那三个硬写的 `true`/`true`/`false` | 收掉那三个字面量；两边比对从逐字段改成整份 `Request` |
| ⑤ 「没说」≠「说了默认值」 | **已成立**：`TasteLayer` 每一项是 `Option`，`a_preset_that_says_nothing_reads_back_saying_nothing` 钉着 | **没拍平 `Option`**（硬约束），另补一条钉屏上那两句话 |
| ⑥ 三条闸门全绿 | —— | 见《数》 |

## 落地记录

### 三对开关补齐反面

`src/main.rs` 的 `Cli` 新出 `crop` / `split` / `no_per_page` 三个字段，各挨着自己的反面声明，
`--help` 上因此一对相邻。取值那三处从单向的布尔运算改成「显式点到的赢」：

| | 从前 | 现在 |
|---|---|---|
| `Cli::crop` | `!self.no_crop && preset.taste.crop()` | `said(self.crop, self.no_crop).unwrap_or_else(\|\| preset.taste.crop())` |
| `Cli::per_page` | `self.per_page \|\| preset.taste.per_page()` | `said(self.per_page, self.no_per_page).unwrap_or_else(\|\| preset.taste.per_page())` |
| `Cli::split_rule` | `on: !self.no_split && stored.on` | `on: said(self.split, self.no_split).unwrap_or(stored.on)` |

**一对折成「说了没有」这一步收在 `said` 一个自由函数里**，三对共用：点了正面 `Some(true)`、
点了反面 `Some(false)`、一个都没点 `None`。**返回 `Option` 而不是 `bool` 是验收第 5 条的要求**——
拍平成具体值，`--crop` 对着一份写着 `crop = false` 的预设就开不回来了。
`(true, true)` 走不到它：`conflicts_with` 挡在前面，错在哪、该怎么敲由 clap 说。

**互斥用 `conflicts_with` 而不是 `overrides_with`**：后者是「后点名的赢」，
而验收第 2 条要的是「同时给两个时**当场说得清**」。备选与理由记在 **Q591**。

### 票面指错了地方：真正的第二份出处在会话左栏

`src/session/state.rs` 的 `Session::shown` 里，三行布尔项各带一个**硬写的** fallback：

```rust
Field::Crop    => spell_flag(self.taste.crop,     true,  "裁", "不裁"),
Field::Split   => spell_flag(self.taste.split,    true,  "拆", "不拆"),
Field::PerPage => spell_flag(self.taste.per_page, false, "开", "关"),
```

那三个字面量就是口味层默认值的第二份。**把 `TasteLayer::crop()` 的默认翻成 `false`，
bin 全套 365 条一条都不红**，而屏上那句「默认（裁）」此刻正在说谎——这一趟已经不裁了。
三处改成 `self.taste.crop()` / `self.taste.split_rule().on` / `self.taste.per_page()`。

其余几行本来就没有这个毛病：`SplitThreshold::default()`、`CacheBudget::default()`、
`spell_name` 的 `T::default()` 都指着库那一侧同一个 `Default`，而 `TasteLayer` 读的正是它。

### `spell_flag` 的签名——类型一个字没变，含义变了

**这一处合并时编译器与测试都不会报，因此在这里留一句。**

```rust
fn spell_flag(value: Option<bool>, taken: bool, yes: &str, no: &str) -> String
```

第二个参数原本叫 `fallback`、要的是「**默认值是什么**」；现在叫 `taken`、要的是
「这一格**落到默认值之后取到什么**」。**两个都是 `bool`**，调用点从字面量换成
`self.taste.*()` 之后类型签名一模一样——git 自动合并会一声不吭地把旧调用点合进来，
`cargo build` 与全套用例都不会说一个字，症状只是屏上那句「默认（裁）」重新开始说谎。

**判据（机械可查）**：`Session::shown` 那个 `match` 的 `Field::Crop` / `Field::Split` /
`Field::PerPage` 三行，**只要还是 `bool` 字面量就是合错了**。

其余签名改动都是编译器拦得住的：`src/main.rs` 新增自由函数 `said(bool, bool) -> Option<bool>`
（纯新增），`Cli` 新增三个字段（字段增删 git 会正常报冲突）；
`Cli::crop` / `per_page` / `split_rule` / `request` 的签名一个都没动。

### 验收第 4 条的前半没有用例钉得住，说清楚为什么

**我先写了一条钉它的用例，`/code-review` 的 Spec 轴判它恒真——判对了。**
两头都从 `TasteLayer` 那几个方法取之后，「屏上说的等于真做的」就是同义反复：
一条断言的两边落到同一个函数上，把 `taken` 换回字面量它照样绿
（字面量此刻恰好等于默认值）。**这是这一批第四条「永远绿」，而且是自己写的。**

我起初以为验过：变异 `TasteLayer::crop()` 时它红了——但那次变异跑在**改之前**的实现上。
桥架好之后同一个变异不再红。**变异要在打算提交的那一版上做，不是在改之前那一版上。**

删掉它，换成一条钉验收第 5 条的
（`saying_the_default_out_loud_still_reads_differently_from_saying_nothing`）：
屏上「没说」印「默认（裁）」、「说了 `Some(true)`」印「裁」，两句话不能是一句。
把 `Option` 拍平成 `value.unwrap_or(taken)` → 当场红。

拦住第二份出处的因此**不是一条断言，是那个参数的名字与 `spell_flag` 上的那段文档**。
理由写在函数身上，下一个人改到那里看得见。

验收第 4 条的**后半**（两边逐格相同）有真锁：
`an_untouched_session_asks_for_what_a_bare_command_line_asks_for` 从逐字段比改成
**整份 `Request` 的 `Debug` 比**，与命令行那侧的 `request_line` 同一手法。
它红得起来：把会话那格 `white_align_limit` 填成非默认 → 整份比红，
而**逐字段那种写法绿**（它那张清单里根本没有这一格）——漏掉新字段正是它防的事。

### 三处被这张票推翻的旧说法

三处都在说「三个开关的覆盖是单向的」，都改成当前成立的事实：

- `src/main.rs` 的 `impl Cli` 抬头与 `--preset` 的长帮助——那两段是 Q55 的现场记录；
- `CONTEXT.md`「裁边、跨页拆分、以高为准三项默认全开，各有一个关掉它的开关」——
  补上「前两项那两个开关各有反面」。**这是加一条当前成立的事实，不是改写词条含义**，
  词汇表一格没动；
- `src/white.rs` 拿「三个开关的覆盖是单向的」当「`--white-align-limit` 不另设布尔开关」
  的**理由**——这张票把那个理由推翻了。换成真正站得住的那条：取 0 时产物逐字节相同，
  多一个开关就是同一件事的第二个出处。（`/code-review` 的 Spec 轴抓的，我自己漏了。）

### 数

三条闸门跑满，三条都绿，一条失败都没有（`cargo xtask gate`，基底 `93f26cb`）。
**告警一条都没有**——三条日志逐条读过，不限行首、不分大小写地找过 `warn`，一条命中都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **925 通过 0 失败**；lib **244** / bin **366** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **794 通过 0 失败**；lib **244** / bin **235** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.23s`` | 干净，一条告警都没有 |

**lib 两趟都是 244，一格没动**：这一票唯一碰到的库文件是 `src/white.rs`，
改的只是一段文档注释里那条已经不成立的理由。

**bin 净 +3**（闸门 1 从 363 到 366，闸门 2 从 232 到 235）：**新增 4 条、删 1 条**。

| 用例 | 问的是 | 改坏哪一处会红 |
|---|---|---|
| `a_switch_a_preset_closed_opens_back_up_on_the_command_line` | 预设关掉的三个开关命令行开得回来，反过来也压得过 | `crop()` 退回 `!no_crop && taste.crop()` |
| `a_switch_and_its_opposite_in_the_same_run_is_an_error` | 三对同时点名当场说得清是哪两个 | 拿掉任一对的 `conflicts_with` |
| `the_help_says_each_switch_and_its_opposite` | 六项各自的长帮助里都有自己的反面 | 删掉 `--no-split` 里指向 `--split` 的半句 |
| `saying_the_default_out_loud_still_reads_differently_from_saying_nothing` | 屏上「没说」与「说了默认值」是两句话 | 把 `Option` 拍平成 `value.unwrap_or(taken)` |
| ~~`the_switches_that_only_say_one_thing_stay_one_way`~~ | 删掉：它钉的正是这张票推翻的那条单向规矩 | —— |

**四条新断言逐条做过变异验证**，上表右栏就是各自的变异。另两条改写的也验过：
`an_explicit_flag_beats_the_preset` 收进三对开关之后，「命令行赢」在它们身上才分辨得出
（从前预设与命令行说的是同一侧，无从分辨）；整份 `Request` 比对那条见上一节。

**闸门之外那一遍**（`cargo xtask polish`，四条全绿）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**，一条没多。

### 评审

`/code-review` 两轴各跑一遍（**只读，没碰工作区**），基底 `93f26cb`。

**Spec 轴收两条，两条都是我漏判的**：

- **那条钉验收第 4 条的用例是恒真的**——见上面《验收第 4 条的前半没有用例钉得住》。
- **`src/white.rs` 说假话**——它拿「三个开关的覆盖是单向的」当不另设布尔开关的理由，
  而这张票正是来推翻它的。

**Standards 轴收四条**：`CONTEXT.md` 那句只说了一半面（当场补）；
**Q592 引的《卷级上包络》不是词条名**，词汇表里那一条是《上包络 (Envelope)》（改）；
`the_help_says_each_switch_and_its_opposite` 末行
`assert!(rendered.contains("--{one}"))` 恒真——`get_arguments().find(id)` 找不到已经 panic，
找得到就必然渲染得出长名，而 `--split` 还被 `--split-threshold` 包着；
**它更被自己上一句文档否掉**（「整份里六个长名都在，`contains` 说不出反面写在哪一项名下」），
删掉，只为它存在的 `rendered` 一并删；票据未收尾（本节）。
外加 `let said = …render()` 遮蔽模块级 `fn said`，改叫 `complaint`。

**判为不收四条**：

- **六个 `bool` 收成一个小类型**（Data Clumps）：`said` 已经把「一对 → `Option<bool>`」
  收在一处，再造类型要在 clap derive 那侧多一层 flatten——为不存在的需要造抽象。
  命令行**表面**那一半已由 Q591 停进停车场。
- **三对开关的帮助里那两句逐字三遍**（Duplicated Code）：一对开关的帮助必须**自足**——
  用户只读他点名的那一项，抽出去的公共件抽不进 `--help`。
- **`self.taste.split_rule().on`**（Message Chain）：绕开 `split_rule()` 单独算 `on`
  就是给「拆分那三项落到默认之后」开第二条路径，而它正是本票要收的那件事；
  三个 `Copy` 字段的代价可忽略。
- **`unwrap_or` 与 `unwrap_or_else` 不齐**：`stored.on` 是字段访问，`unwrap_or` 正确；
  换成 lazy 会撞 clippy 的 `unnecessary_lazy_evaluations`。

### 停车场

用掉 **Q591**、**Q592**（配额 Q591–Q600，剩下八个留下空洞）。
收掉票面点名的 **Q55**、**Q67**——两条早已在《已了结》的索引表里指着本票，
条目原文的去处就是这份《落地记录》。
