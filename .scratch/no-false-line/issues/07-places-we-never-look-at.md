# 07: 不看的地方

**What to build:** 点名一个盘根或共享根，`System Volume Information`（Windows 上管理员都进不去）、
`$RECYCLE.BIN`、`lost+found`、`.Trash-1000` 这类**永远**读不动的目录会进
[走不进去的地方](../../../CONTEXT.md)那一栏，于是**一趟卷卷都成的运行恒收在 `3` 上**，
重跑一百遍都一样。而那一栏劝人做的三件事（文件还在不在、盘还挂着没有、权限变没变）
对它们一件都做不了。

词汇表已经把这两种分开了：**走不进去的地方**收窄成「本该走得进去、**这一趟**走不进去」
（权限没配好、盘掉了——换一趟可能就好），新词
[不看的地方 (IgnoredPlace)](../../../CONTEXT.md)收「**本来就不看**」的那一类。
分得开它们的是「换一趟会不会好」。

落地：**两份名单并成一份**。仓库里已有一份收「打包环境留下的目录」的名单，
它与系统目录**行为逐字相同**（默默跳过、不进退出码、不进任何一栏），分成两份只是历史。
那份名单的判据改成「本来就不看的地方」，系统目录一并收进去。

绕过发生在**发现**那一层：它们根本不成为候选，因此预扫看不见、那一栏收不到、
退出码一格不动，**报告上一个字都没有**。

这与「东西不该无声消失」那条线不冲突：那条线针对的是**本该处理而没处理**，
而这一类本来就不处理。

收停车场的 **Q203**。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 两份名单**并成一份**，判据从「打包环境留下的目录」改成「本来就不看的地方」
- [x] 那几个系统目录**按名字**认得出来（各平台上名字固定，不随语言变）
- [x] 点名一棵含系统目录、卷卷都成的树，退出码是 **`0`**；那几个目录**报告上一个字都没有**
- [x] **反向那条**：权限真的没配好的目录**照旧**进走不进去的地方那一栏、**照旧**进退出码
      ——收窄不许把这一半一起收掉（**本机验不到那一半**，见《落地记录》与停车场 Q479）
- [x] 绕过发生在发现那一层：它们**根本不成为候选**，不是进了候选再被滤掉
      （**这一条是守住、不是做出**——见《落地记录》与停车场 Q480）
- [x] `CONTEXT.md` 一个字不动——两条词条这一轮已经写好了
- [x] 三条闸门全绿

## 落地记录

### 一份名单，判据从「打包环境留下的」改成「本来就不看的」

`src/source.rs` 的 `JUNK_DIRECTORIES: [&str; 10]` 改名成 `IGNORED_DIRECTORIES: [&str; 14]`，
末尾添上操作系统留下的那四个：

```rust
"System Volume Information",
"$RECYCLE.BIN",
"lost+found",
".Trash-1000",
```

`is_junk_directory` 跟着改名成 `is_ignored_directory`——名字取自 `CONTEXT.md` 这一轮新写的
词条《不看的地方 (IgnoredPlace)》，`CLAUDE.md`《写代码前》要求的正是这件事。
比法一格没动：`is_one_of` 照旧**整名**、大小写不敏感地比，`$Recycle.Bin` 与 `$RECYCLE.BIN`
因此是同一个名字。

名单的文档不复述词条：这一类是什么、它与走不进去的地方分在哪里、为什么报告上一个字都没有，
权威位置是 `CONTEXT.md`，名单这里只说**它自己**答得出的那两件——两种来处为什么共一份名单，
以及判据为什么是名字而不是「读不读得动」（按读不动认，会把真正走不进去的那一半一起吞掉）。

### 绕过那一条是**守住**，不是做出

票面第五个复选框写着「绕过发生在发现那一层：它们**根本不成为候选**」。
**这条形状在本票开工前就已经在位**——`src/discover.rs` 的 `push_children` 里那个
`continue` 从 `volume-discovery/03` 起就排在 `children.push(...)` 之前
（`git show b6106fb:src/discover.rs` 看得到）。本票只把名单与判据搬过去，一格结构没动，
候选集上**没有**新增任何过滤。

读成「要做的事」的人会去候选集上加一道过滤，而那正是同一张票面明令禁止的形状。
因此这一票在 `push_children` 的文档上补了一句，把它写在**它成立的地方**：

> 头一样是在**这里**挡的，不是攒完候选再滤：不看的地方连同它底下的一切**根本不成为候选**，
> 预扫因此看不见它、走不进去的地方那一栏收不到它、退出码一格不动、报告上一个字都没有。

那四件后果从前只写在票面与 spec 里，**代码上一个字都没有**。记停车场 **Q480**
（票面那一条建议改写成「守住」）。

### 反向那半：主证据换成平台无关的一条

票面要「权限真的没配好的目录**照旧**进那一栏、**照旧**进退出码 `3`」。
既有那两条用例（`src/survey.rs` 的 `a_directory_that_cannot_be_read_says_so`、
`tests/exit_code.rs` 的 `a_place_that_cannot_be_entered_ends_the_run_with_three`）
本票一格没动、跑绿——**但它们在本机（Windows）验不到**：`shut_the_door` 在
`#[cfg(not(unix))]` 上恒回 `false`，两条当场 `return`。

刀口在于：判据要是从「名字」滑成「读不动」，本机三条闸门会**全绿**。
因此反向那半的主证据换成一条平台无关的，`tests/discovery.rs` 的
`a_name_that_merely_looks_like_one_we_never_look_at_is_still_walked_into`：
`System Volume Information 备份`、`.Trash-10000`、`$RECYCLE.BIN.old` 照旧走进去、
底下的卷照旧落在输出树上，而只有大小写不同的 `$Recycle.Bin` 照旧不看。
**实测**把 `is_one_of` 放宽成前缀匹配，这一条当场红（卷数 3 → 0）。
本机验不到的是哪一半，记停车场 **Q479**。

新添的那条退出码用例**在本机也不是恒真**：它调 `shut_the_door` 但**不跟着 `return`**——
关不上门的机器上那四个目录读得动，走进去了它们底下的卷就会露在输出树上，
`assert_eq!(members, ["库/好的.cbz"])` 照旧咬人。

### 四个红绿片段，每一个都实测过红

| 片段 | 红的样子（实测） |
|---|---|
| `src/discover.rs` 的 `an_ignored_place_is_never_walked_into` | `System Volume Information` 整棵子树都成了候选 |
| `tests/discovery.rs` 的 `discovery_does_not_walk_into_the_places_we_never_look_at` | 摘掉四个名字，卷数 5 ≠ 1 |
| `tests/exit_code.rs` 的 `a_tree_with_places_we_never_look_at_still_ends_the_run_with_zero` | 摘掉四个名字，输出树上多出四卷 |
| `tests/discovery.rs` 的 `a_name_that_merely_looks_like_one_we_never_look_at_is_still_walked_into` | `is_one_of` 放宽成前缀匹配，卷数 3 → 0 |

三条改了名的用例（另两条是 `an_ignored_place_named_on_its_own_is_not_folded_away`
与那条集成用例）连同名单改名，在别人票的《落地记录》里留下十处悬空引用——
按 `docs/agents/issue-tracker.md`「历史记录不被重写」一处没动，清单记在停车场 **Q482**。

### `/code-review` 两轴，照办四条

Spec 轴：七条验收条实质全中，无 scope creep，无实现错误；亲自核实了
`git diff b6106fb -- CONTEXT.md` **为空**，以及「绕过在发现那一层」那条形状在基点上就已在位。

两轴合起来指出五处，四处当场改了：

1. **单一出处**（`CLAUDE.md`《文档写作》第 4 条）——名单的文档把词条抄了一遍、
   退出码那条用例的文档再抄第三遍。都压回引用词条名。
2. **名单文档漏了四个名字**——改写时把覆盖全单的那句「其余几个是各家文件管理器、
   版本控制与 NAS 自己的索引与回收站目录」丢了，`.Spotlight-V100`、`.Trashes`、
   `.fseventsd`、`.git` 从此无人解释，而「版本控制」四个字还留着。补回。
3. **稳定引用**（第 5 条）——Q482 原本用行号定位十处引用，改成按小节标题与术语。
4. **Q482 的「零残留」口径**——它只管这次改掉的那几个名字；`is_junk`、`JUNK_FILES`
   与 `tests/container.rs` 那条用例照旧带着 `junk`，那是**成员**那一侧，见 Q477。当场限定。

第五处（`is_junk` 这个名字如今只说得出它判据的一半，Standards 轴判为 Mysterious Name）
不在本票动：票面没要求，而改名的爆炸半径本票刚吃过一次。记停车场 **Q477**，推荐方案 ②。

### 改名之后的全仓扫描

`git grep` 覆盖 `*.md` `*.rs` `*.toml`，扫 `JUNK_DIRECTORIES`、`is_junk_directory`、
「打包环境留下的目录」与三个旧用例名：

| 扫的范围 | 结果 |
|---|---|
| `src/`、`tests/`、`xtask/` | **零命中** |
| `CONTEXT.md`、`docs/`（含 `docs/adr/`） | **零命中**——票面「一个字不动」那条没被这次改名逼到 |
| `git diff b6106fb -- CONTEXT.md docs/` | **空**（Spec 轴独立核过一遍） |
| 别人票的票面与《落地记录》 | **十处**，一处没动，清单在停车场 Q482 |

### 数

**闸门 1 在最终代码上重跑过一遍。** 头一趟跑到一半时，`/code-review` 两轴的四条修正
落进了 `src/source.rs` 与 `tests/exit_code.rs`（都是文档注释），那一趟的闸门 1
因此跑在改前的二进制上。凭一份对不上代码的绿收票是假绿，故补跑闸门 1，
两趟的数**逐字相同**。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test`（**重跑**） | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **884 通过 0 失败**；lib **243** / bin **349** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **753 通过 0 失败**；lib **243** / bin **218** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.36s`` | 干净，一条告警都没有 |

**两条闸门各涨 2**（884 ← 882、753 ← 751），正是新添的那两条集成用例
（`tests/discovery.rs` 与 `tests/exit_code.rs` 各一条）。lib 两条都是 **243**，一格没动——
本票在库里只**改**了一条既有单元用例的名字与内容，没添新的。两条新用例都在 `tests/` 下、
不挂 `tui`，因此闸门 1 与闸门 2 同涨。

**闸门 2 与闸门 3 没有重跑，够用，理由是编译时刻**（本机 mtime 实测）：
`src/source.rs` 最后改于 `00:02:53`，而闸门 2 产出的测试可执行文件是 `00:03:02`～`00:03:04`、
闸门 3 是 `00:09:02`——**两条都编在那次改动之后**。唯一没被它们收进去的是
`tests/exit_code.rs` 那次 `00:03:12` 的改动，而那是一段 `///` 文档注释，
不改任何可执行代码，且**闸门 1 已在含它的那一版上跑绿**（同一个文件、同一批断言）。
本票不碰 `src/session/`、`src/cost.rs` 与 `[features]`，`docs/agents/gate.md`
《什么时候必须跑满三条》那三条一条都不沾。

**闸门之外那一遍**（`cargo xtask polish`，跑在最终代码上）：`cargo fmt --check` 干净；
`cargo clippy --all-targets` 与 `--all-targets --no-default-features` 两遍都零告警；
`cargo doc --no-deps` 仍是 **15 条告警**（``warning: `tonefit` (lib doc) generated 15 warnings``），
**一条没多**——本票新写的 intra-doc link 只有一条（`crate::UnreachablePlace`），
走的是公开重导出路径，不是 `crate::report::UnreachablePlace` 那条私有模块路径。

### 停车场

新记**六条**（`Q477`–`Q482`，都留在《待处理》里，每条都带 `- **处置：**` 那一行）：

| 新记的 | 一句话 |
|---|---|
| **Q477** | 名单与目录那一侧改口成「不看的地方」了，成员那一侧的 `is_junk` 名字没跟着改——它今天收两样，名字只说得出后一半 |
| **Q478** | 名单在**两处**同时作数，因此归档**成员名**那一侧也跟着扩了四个名字，而票面只说了发现那一侧 |
| **Q479** | 反向那半在本机（Windows）验不到——`shut_the_door` 恒 `false`，主证据因此换成平台无关的那一条 |
| **Q480** | 票面把「绕过发生在发现那一层」写成了要**做**的事，而它开工前就已经成立——那是要**守**的 |
| **Q481** | `lost+found` 与 `.Trash-1000` 在大小写敏感的文件系统上也被大小写不敏感地认 |
| **Q482** | 改名的爆炸半径：五份别人票的记录里十处指着旧名，按「历史记录不被重写」一处没动 |

本票收的 **Q203** 就此落地：点名一个盘根不再恒收在 `3` 上。
