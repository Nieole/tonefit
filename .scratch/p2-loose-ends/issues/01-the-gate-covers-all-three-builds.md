# 01 — 闸门盖住三种构建，状态机搬出终端库

**What to build:** 改这个仓库的人跑一遍闸门，就知道三种构建都还站得住：默认那一趟、
关掉终端库那一趟、开着量具那一趟。眼下只有第一趟有人跑——会话那六十几条用例整个在
`tui` 后面，关掉之后一条都编译不到；量具那一半连编译都没人验。

闸门因此从一条命令变成三条。第二条要成立，会话的**状态机**得搬到 feature 之外：
它本来就不 `use` 任何终端库，搬出去是把这个事实变成结构，而不是靠下一个人记得。
绘制与键码翻译留在 feature 后面——那两样真的要终端库。

收停车场的 **Q61**、**Q68**、**Q79**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 闸门是三条命令，三条都绿，且在一处写明它们是闸门（下一个人照着跑，不必问）
- [x] 关掉终端库那一趟能跑到会话状态机的全部用例，不只是「编译得过」
- [x] 开着量具那一趟至少编译得过，计数那一半不再一条自动检查都没有
- [x] 状态机模块在 feature 之外，且仍旧不 `use` 任何终端库
- [x] 绘制与键码翻译仍在 feature 后面，关掉之后不留 `dead_code` 告警
- [x] 默认那一趟的用例数不减少，黄金快照未变

## 落地记录

**闸门写在 `docs/agents/gate.md`**，`CLAUDE.md` 的《闸门》给三行命令加一个指路
（渐进披露：入口给骨架，细节下沉一层）。那份文档写清三条各盖住什么、
三条为什么互相说明不了对方、哪种改动必须跑满三条、以及每条的结果怎么读。
**数不在那儿**——它指回各票据的《数》，免得两处各说一个。

**状态机搬家**。原来的 `src/session.rs` 是模块根加一整层终端胶水，整个挂在 `tui` 后面。
现在拆成两半：

- `src/session.rs` 只剩几行 `mod` 与一段模块文档《终端库在哪一半》。
  `state`、`live`、`run`、`complete` 摆在**特性外面**；`draw` 与新的 `terminal` 挂 `tui`。
- `src/session/terminal.rs` 是原来那一层：进出终端（`Screen`）、crossterm 键码翻译
  （`translate`）、那条循环（`drive`／`enter`），加上状态机够不着的那几支（`press` 一层）。

**整个 `mod session` 挂在 `any(feature = "tui", test)` 上**，不是无条件——`tui` 关掉之后
会话没有非测试的用户（`without_arguments` 恒不接手），不挂 `test` 就是一片 `dead_code`。
同一副写法在 `src/cost.rs` 上已经有一份。spec 那句「搬到 `tui` 之外」按字面读是「无条件」，
差的正是这一格 `test`：**非测试的 `--no-default-features` 构建里状态机仍然不编译**，
而闸门第二条要的「跑得到它的用例」成立。只有画法读得到的那九处取值器，
在关掉的那一趟上整模块放开 `dead_code`（停车场 Q84 记着这笔放松与它的宽度），
默认那一趟一格不放松。

**胶水没搬**：`press` 那一层只有一处真要终端库——`expand` 要 `draw::opens_at`
让 ratatui 画一遍数出滚动量。给那个数开缝是改形状，而折行那一套归 `p2-loose-ends/07`
定形，本票不抢（停车场 Q83）。

`render::calibration_notice` 的 `cfg` 从 `feature = "tui"` 改成 `any(feature = "tui", test)`：
读它的是状态机，而状态机现在在特性外面。`render::failing_pages` 与 `render::outcome`
照旧只挂 `tui`——读它们的是画法。

**量具那一半**。`tally` 里原先「挑哪几行、按什么排、印成什么样」与原子表搅在一起，
`profiling` 关掉就整个不编译。摘出两个纯函数 `cost::rows` 与 `cost::table`
（`cfg` 同为 `any(feature = "profiling", test)`），三条新用例在**默认**那一趟就跑得到；
`tally` 里剩下的只有加数与读回来，由第三条闸门编译到。印出来的字节一个没变
（原先几条 `eprintln!`，现在一条 `eprint!` 印同一份文本——顺带让那张表在
多趟并发的 stderr 上不再被别的行劈开，但没有人要求这一条）。
**`profiling` 仍不在 `default` 里**——量具存在的前提没松动。

**这一刀比票面要的多**：第三条 checkbox 只说「至少编译得过」，而第三条闸门本身就够。
摘纯函数是为了让「计数那一半不再一条自动检查都没有」名副其实——
只被编译一遍算不上一条检查。多出来的是三条用例与一次搬家，没有新行为。

**行为一字未改**：命令行与会话两路一步没动，退出码照旧，黄金快照未变、未重录。

### 复审

两轴各跑一遍（Standards / Spec）。收了这几条：

- **停车场的处置在复述 `01` 的《落地记录》**（Standards 轴，单一出处）。Q61 与 Q79 的
  《处置》各砍掉大半，只留「答没答上本条问的那件事」，细节指回这一份记录——
  Q68 本来就是这么写的。
- **`Q84` 的 Where 指着已经不存在的代码**（Spec 轴）。复审途中那四行 `cfg_attr` 已经并成
  `mod session` 上的一句，条目跟着改；顺带把「放开的面比那九处宽」写进去。
- **四行重复的 `cfg_attr`**（Standards 轴，Duplicated Code）。并成 `mod session` 上一句——
  `tui` 关掉时 `session` 下面本来就只有那四个模块，两种写法等价。
- **三条命令在 `gate.md` 里列了两遍**（Standards 轴，单一出处）。删掉开头那个代码块，
  只留表格；`CLAUDE.md` 那一份是入口骨架，按《渐进披露》留着。
- **「闸门」与领域术语《几何门》撞词**（Standards 轴）。`gate.md` 开头点明两者无关。
- **`print_profile` 手搬两张表**（Standards 轴）。换成 `std::array::from_fn`。

几条没收，有理由：

- **`(Stage, u64, u64)` 该有个类型**（Standards 轴，Data Clumps / Primitive Obsession）。
  这个三元组只在相邻的两个私有函数之间走，与 `p1-session/09` 判过的那对
  「失败页 `(路径, 原因)`」同一形状、同一理由。
- **`table` 里三份列宽格式串**（Standards 轴，Duplicated Code）。格式串必须是字面量，
  抽不成一个常量；而且这三份**搬家之前就在**（原来是三条 `eprintln!`），不是本票引进的。
- **`cfg(any(feature = X, test))` 撒在四处**（Standards 轴，Shotgun Surgery）。
  那正是本票要做的事：闸门盖住哪一趟是**每个模块自己**的性质，收进一处等于把它藏起来。
- **闸门没有脚本，只有文档**（Spec 轴，spec 说断言该写在 CI 脚本或 `xtask` 上）。
  仓库里 CI 与 `xtask` 都不存在，立一套只为跑三条 `cargo` 命令，能保证的事情
  与照着文档敲一样多。记成停车场 Q82，等拍板要不要 CI。

### 数

| 闸门 | 结果 | 最后一行 |
|---|---|---|
| `cargo test` | **552 通过 0 失败**（基线 549，多的三条是 `cost` 的新用例），17 个测试二进制 | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（doc-test 那一格） |
| `cargo test --no-default-features` | **517 通过 0 失败**（此前 460），同样 17 个二进制——会话那 54 条（`state` 24、`run` 15、`live` 9、`complete` 6）从「编译不到」变成跑得到 | 同上（doc-test 那一格） |
| `cargo check --features profiling` | 干净 | ``Finished `dev` profile [unoptimized + debuginfo] target(s)`` |

前两条的最后一行都是 doc-test 那一格的 `0 passed`：库里没有 doc-test，
真正的结论在它上面那十六行——十六个二进制**一个 FAILED 都没有**。

留在 `tui` 后面的是 35 条：`draw` 23、`terminal` 12（含 `press` 那 10 条）。

`cargo fmt --check` 干净；`cargo clippy --all-targets` 三种特性组合
（默认、`--no-default-features`、`--features profiling`）均干净；
`cargo doc --no-deps` 与 HEAD 同为 **14** 条既有告警。
**黄金快照未变、未重录。**

停车场了结 Q61（→ `p1-session/08`）、Q68（→ `p1-session/09`）、Q79（→ `p0-hardening/13`），
新增 Q82、Q83、Q84；待处理 34 → 34。
