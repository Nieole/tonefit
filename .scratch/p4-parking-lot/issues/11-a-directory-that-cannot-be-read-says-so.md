# 11 — 读不动的目录说得出来，并进退出码

**What to build:** 发现的时候撞见一个**读不动的目录**（NAS 上权限没配好是最常见的一种），
今天一声不吭：那棵子树整个消失，报告一行不说，**退出码还是 `0`**。
用户拿到的是一份看起来成功、实际少了几十卷的输出。

报告多一栏：**「发现走不进去的地方」**。这一栏进退出码那条判据——有它就不是「全都做成了」。

**不改《非卷文件》那条词条**：它定的是三类**文件**，而走不进去的是一个**地方**，
塞进去要改词条的含义，那是另一回事。

收停车场的 **Q117**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 发现时读不动的地方逐条列在报告上，说得出是哪个路径、为什么走不进去
- [x] 有这一栏时退出码不是「全都做成了」
- [x] 其余卷照常跑完——一个读不动的目录不挡住整趟
- [x] **点名**一个读不动的路径仍是当场拒（与「点名的 / 发现的」那条既有分别一致）
- [x] 命令行与会话拿的是同一份数据
- [x] 三条闸门全绿

> **六条全做到，没有保留。** 四处**判断**（不是保留，但要写明读法）：
>
> - **退出码进的是 `3`，不是新开的第五个。** 票面把这一档交给实现拿主意，
>   而 `3` 那一档的文档自己写着用户下一步该查什么——「文件还在不在、盘还挂着没有、
>   权限变没变」——与走不进去的那一处逐字相同；交出的东西也一样（那一块一个字节都没有）。
>   分成两个数买不到任何一个脚本分得开的决定。理由写进了 `FAILED_VOLUME_EXIT` 的文档。
> - **报的地方只有一处，`push_children` 照旧不出声。** 那一句「列不动这一层就整棵子树跳过」
>   一个字符没动：被它跳过的每一个目录，自己都是预扫的一个候选，同一个 `read_dir`
>   在 `survey` 那一处再失败一次。在两处都报就是同一个目录两条。这条推理写在
>   `push_children` 的文档上——它成立的前提（每一层都先进 `found` 再走到这里）也写在那里。
> - **`CONTEXT.md` 只加了一个新词条，一个既有词条没动。** 加的是《处理对象》的
>   「走不进去的地方 (UnreachablePlace)」（`CLAUDE.md`《改 CONTEXT.md 的规矩》：
>   新词可以当场加，不加则「类型名一律取自它」执行不了）。
>   《失败》那段退出码说明写着「`3` 有卷没做成」，如今窄了一格——**没改**，补在 Q163 上
>   （第 17 条），归 28 号票。
> - **会话那一头多动了一格：总览块的出事行。** 票面那条验收（「命令行与会话拿的是同一份
>   数据」）只要求那一栏两边共用，而共用是靠 `render::tail` 做到的。评审指出：只有一处
>   走不进去的那一趟，屏上 `出事` 那一行**整行不出现**，而退出会话交出的是 `3`——
>   「屏上没事、脚本说出事了」正是那一行存在的理由（见 `trouble_row` 自己的文档）。
>   因此给它加了一格。这一格是**票面之外的一手**，写在这里，不藏在落地记录里。

## 落地记录

### 一、库那一侧：第四张表

**`Report` 多一列**（`src/report.rs`）：

```rust
pub struct Report {
    /* …… */
    pub non_volume_files: Vec<NonVolumeFile>,
    pub unreachable_places: Vec<UnreachablePlace>,
    /* …… */
}

pub struct UnreachablePlace {
    pub path: PathBuf,
    pub reason: String,
}
```

`reason` 装的是**由内到外的错误链**（`format!("{error:#}")`），与 `VolumeFailure::reason`、
`NonVolumeReason::Unopenable` 同一个待遇：报告里说一样东西没被处理，就得说得出该去修什么。

**判据是 `Report::any_place_unreachable`**，与 `any_volume_failed` 同一条规矩——
判据只有一条，与那一列不许分家。

**它为什么另开一栏而不是当第四类非卷文件**，三处各写了一句、互相指着：
`NonVolumeReason::Unopenable`（那一句本来就在，只是从前指着 Q117）、
`UnreachablePlace` 自己、以及 `CONTEXT.md` 的新词条。

### 二、报的地方只有一处

`survey::Survey::of` 里点不开那一支的 `(Provenance::Discovered, Container::Directory)`
那一格，从前是空的 `{}`，如今推一条进 `unreachable_places`。

**这一处收得全**：一个目录列不出来时，`discover::push_children` 与
`source::open_directory` 撞的是同一个 `read_dir`，而**每一个被 `push_children` 走到的目录
自己都是一个候选**——`named` 是 `discover::of` 造的头一个，往下每一层都由 `expand`
先推进 `found` 再走到 `push_children`。于是那一层列不出来时，预扫开它必然也失败，
新那一栏必然收到它。`push_children` 因此一个字符没改，只加了这段推理的文档。

**「点名的 / 发现的」那条分别一格没动**：`(Provenance::Named, _)` 那一支照旧进
`refused`，整趟拒绝、`run` 返回 `Err`、没有报告（ADR 0014 决定第 5 条）。
两条用例各钉一头（见《数》）。

`Survey::into_volumes_and_non_volume_files` 改名成 `into_volumes_and_the_rest`，
交出的从两份变三份。

### 三、界面那一侧：末尾多一小结

`render::tail` 从六小结变七小结，新那一小结叫 **`RowKind::UnreachableTail`**，
措辞出自 `render::unreachable_tail`：

```
发现走不进去 2 处：那一层列不出来，底下有没有卷谁都不知道——整棵子树跳过。
这一趟因此**不是「全都做成了」**，退出码跟着变；别的卷该做的照做，上面那些就是做出来的。
要那底下的东西，先把下面这几处修好再重跑
  库/权限没配好的作品
    列出 库/权限没配好的作品 这一层: Permission denied (os error 13)
```

形状照另外两小结办：路径一行、原因一行，最多列五条，多了说「……另有 N 处」。

**压在末尾那几小结的最后**：那几小结按出的事有多重往下排，而这一种是里面唯一
**说不清少了多少**的——卷级失败点得出是哪几卷，这一种连那底下有没有卷都答不出来。

**命令行与会话拿的是同一份数据**：两边都走 `render::tail`，措辞只有 `render` 一处。
会话那一头给它 `Tone::Trouble`（`session::draw::report::tail_row`），与卷级失败同一档——
两者共用一个退出码。那个 `match` 不留 `_`，第八小结因此是编译错误而不是收场那一帧的恐慌。

### 四、退出码

```rust
fn exit_code(report: &Report) -> u8 {
    if report.any_volume_failed() || report.any_place_unreachable() {
        FAILED_VOLUME_EXIT
    } else if report.any_isolated() {
        ISOLATED_EXIT
    } else {
        SUCCESS_EXIT
    }
}
```

会话那一路走的是同一个函数（`session::live::Live::exit_code`），两条路一个数。

`README.md` 的 `3` 那一格跟着改成「有卷没做成，或有地方走不进去（那棵子树整个跳过）；
其余卷照常跑完」。**别的输出一个字节没动。**

### 数

三条闸门跑满，三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **796 通过 0 失败**；lib **213** / bin **328** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **670 通过 0 失败**；lib **213** / bin **202** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.98s`` | 干净，一条告警都没有 |

**闸门 1 从 789 涨到 796**（+7），**闸门 2 从 664 涨到 670**（+6）——
**lib 从 210 涨到 213**（本票动库，那三条都在 `src/survey.rs`，两趟都算），
bin 闸门 1 涨两条、闸门 2 涨一条（会话那一条挂在 `tui` 特性后面，闸门 2 上根本不编），
`tests/exit_code.rs` 涨两条。

涨的七条：

| 用例 | 在哪 | 问的是 |
|---|---|---|
| `a_directory_that_cannot_be_read_says_so` | `survey`（闸门 1、2） | 发现出来的一个读不动的目录上了表、说得出是哪个路径与为什么，而其余卷照常收下 |
| `a_named_directory_that_cannot_be_read_is_still_refused` | `survey`（闸门 1、2） | 同一个目录换成**点名的**，`Survey::of` 回 `Err`、那句话说得出「整趟不做」 |
| `a_broken_archive_and_a_closed_door_land_on_different_lists` | `survey`（闸门 1、2） | 坏归档进非卷文件、读不动的目录进新那一栏，两张表分得开（本票那条边界） |
| `a_place_that_cannot_be_entered_is_named_and_shows_up_in_the_exit_code` | `render`（闸门 1、2） | 报告说得出两处各自的路径与错误链；一卷没失败、一卷没隔离而退出码已是 `3`；那一小结压在末尾 |
| `a_place_that_cannot_be_entered_shows_up_on_the_trouble_row` | `session::draw::overview`（只进闸门 1） | 会话那一头的出事行说得出那一处、给的是「出事」那一档，而 `Live::exit_code` 真是 `3`（评审收的） |
| `a_place_that_cannot_be_entered_ends_the_run_with_three` | `tests/exit_code`（闸门 1、2） | 真进程上收在 `3` 上，而好的那一卷真在盘上 |
| `a_named_place_that_cannot_be_entered_is_still_refused` | `tests/exit_code`（闸门 1、2） | 真进程上点名一个读不动的目录仍是 `1` |

**四条走权限的用例在弄不出「读不动的目录」的机器上当场收工**（Windows 没有这一手，
root 底下权限位不作数）：`shut_the_door` 真去 `read_dir` 一遍，列得动就**把门打回去**、
回 `false`，用例直接返回。在那种机器上它们恒不成立、误报不了；
在这台机器上，「一声不吭」那一版当场红。

**两条既有用例跟着改**：
`every_tail_subsection_gets_its_own_tone_without_moving_a_glyph` 从六小结变七小结
（那一条要的正是「几小结凑齐才问得出分属三档」）；
`the_tail_comes_out_one_row_per_subsection_and_prints_the_same_bytes` 的文档改准了
（评审收的：它点的两小结是**头一条与卷级失败那一条**，不是「中间那两小结」）。

**闸门之外那一遍**：`cargo fmt --check` 干净；`cargo clippy --all-targets` 与
`--all-targets --no-default-features` 两遍都零告警；`cargo doc --no-deps` 仍是
**15 条告警**（`tonefit (lib doc) generated 15 warnings`），一条没多。

**另外在真库上跑了一趟**（不是用例，是眼看一遍）：一个装着一卷两页与一个 `chmod 000`
目录的库，命令行印出来的末尾是

```
发现走不进去 1 处：那一层列不出来，底下有没有卷谁都不知道——整棵子树跳过。这一趟因此**不是「全都做成
了」**，退出码跟着变；别的卷该做的照做，上面那些就是做出来的。要那底下的东西，先把下面这几处修好再重
跑
  demo/库/权限没配好的作品
    列出 demo/库/权限没配好的作品 这一层: Permission denied (os error 13)
```

而退出码是 `3`，那一卷两页照常写出。

### 评审提的十五条

**六条是真的，各配一手**：

- **会话的出事行对它一个字都不说。** 一趟卷卷都成、只有一处走不进去的运行，屏上
  `出事` 那一行整行不出现（三个数全是零），而退出会话交出的是 `3`。
  「屏上没事、脚本说出事了」正是那一行存在的理由（它自己的文档写着
  「一趟试算里坏了三页、废了一卷，这一行一个字都不说——而那正是这一块存在的理由」）。
  **加了一格**：`发现走不进去 N 处`，`Tone::Trouble`，与卷级失败并排；
  配一条用例 `a_place_that_cannot_be_entered_shows_up_on_the_trouble_row`
  （两头一起问：那一行说得出，而 `Live::exit_code` 真是 `3`）。
- **上色那张表还写着「六小结」，而它是那张表的唯一出处。** `tail_row` 的文档表里
  没有新那一小结的行，而它下面的 `match` 已经给了 `Trouble`——下一个动语义色的人
  照表改就会漏掉一个变体。表补上了那一格；连同 `render`、`render::plain`、
  `draw::paint`、`draw::report` 里另外五处「六小结」一并改口。
- **`Live::exit_code` 的文档没跟着改。** 命令行那一路的同一句话改了，会话这一句还写着
  「有卷没做成 `3`」。改了。
- **我改坏了一条既有用例的文档。** `the_tail_comes_out_one_row_per_subsection_and_prints_the_same_bytes`
  原本写着「头一条与末一条特意各点一小结」，添了第七小结之后末一条不再是卷级失败，
  我把它改成了「中间那两小结」——而非卷文件是**头一条**，这句话是假的。
  改成说得准的那一句，并把它买到的东西写回去（空在头上与空在尾上两者都要与从前一样）。
- **停车场 Q163 那第 17 点插错了地方。** 它落在了 `- **Whose call:**` 那一条下面，
  编号表看上去停在 16。挪进编号表末尾，并把它要拍的那一板补进 `Whose call`。
- **试不出「读不动的目录」时不还原权限位。** `shut_the_door` 在 root 底下会走到
  「`set_permissions` 成了、`read_dir` 照样读得动」那一支，那时目录已经是 `0o000`
  而用例直接返回。改成**当场把门打回去**再回 `false`（库与集成两份都改）。

**四条认下、记进停车场**：

| 提的 | 去处 | 为什么不在本票动 |
|---|---|---|
| `System Volume Information` 这类**永远**读不动的系统目录会把点名盘根那一趟钉死在 `3` 上 | **Q203** | `JUNK_DIRECTORIES` 收的是「打包环境留下的目录」，往里加系统目录是改那条判据的含义；NAS 上最常撞见的那两个回收站（`#recycle`／`@Recycle`）本来就在名单上 |
| 新那一格收的是 `enumerate` 在目录上失败的**每一种**，而抬头那句只对「列不出这一层」严格成立 | **Q201**（并补了 `survey` 那一格的注释） | 另外几种都是两次系统调用之间的竞态，而**那一条原因照旧原样带出来**，用户读得到真相；为竞态把常见那一句写虚是净损失 |
| ADR 0014 决定第 5 条只列了归档那一支 | **Q202** | 票面写死不许动那一条 |
| 末尾三小结是同一副骨架抄了三遍 | **Q204** | 抽公共的那一下要动另外两小结印出去的那一段，而票面写死「既有输出一字不改」 |

**一条已经不成立**：《数》那一节里的模板占位符——评审读到的是填进去之前那一版。

**一条按票面办**：`CONTEXT.md` 自己两处对不上（《处理对象》那条新词条说它进退出码，
《失败》那段说明写着「`3` 有卷没做成」）。票面写死「已有词条一个字不许动，
撞见对不上记 Q163」，已记（第 17 点）。

**三条没动，各有理由**：

| 提的 | 为什么没动 |
|---|---|
| 每条打印两遍路径（路径一行、原因一行而原因里也带路径） | 那是这仓库定死的形状：`survey::refuse` 的文档写着「不拼成一行是因为两者不一定互相包含」，非卷文件与卷级失败两小结都这么摆 |
| `shut_the_door` 在库与集成测试里各一份 | 两份跨 crate（一份在 `src/`，一份在 `tests/`），搬不到同一处；集成那一份只有一个消费者 |
| 重叠的点名路径会让同一处上表两次 | 非卷文件那一列一模一样，是既有形状，不是本票造的 |

## 停车场结转

**了结一条**（原文连同处置挪到这里，`## 待处理` 里已删掉，`## 已了结` 索引表加了一行），
**新记四条**（留在《待处理》里，后两条是评审撞出来的）：

| 新记的 | 一句话 |
|---|---|
| **Q201** | 发现往下走时，一个**问不出形态**的目录项（`entry.file_type()` 失败）照旧无声消失——它根本没成为候选，新那一栏收不到它；同一条还记着新那一栏抬头那句话只对「列不出这一层」那一种严格成立 |
| **Q202** | ADR 0014 决定第 5 条只列了「发现的**归档**点不开」那一支，如今那里有三支，而末一句「退出码一格不动」对新那一支不成立（票面写死不许动那一条） |
| **Q203** | `System Volume Information` 这类**永远**读不动的系统目录不在 `JUNK_DIRECTORIES` 里，点名一个盘根从此恒收在 `3` 上 |
| **Q204** | 末尾三小结（非卷文件 · 卷级失败 · 走不进去）是同一副骨架抄了三遍，改 `SHOWN` 或改「……另有」的措辞要在三处各改一手 |

另有**一条补在 Q163 上**（`CONTEXT.md` 那一类，归 28 号票）：《失败》那段退出码说明
写着「`3` 有卷没做成」，而本票把走不进去的地方也并进了那个数——`CONTEXT.md`
因此自己两处对不上（《处理对象》那条新词条说它进退出码）。

### Q117 — 发现出来的一个目录**读不动**时，至今一声不吭

- **From:** 票 `volume-discovery/04`
- **Kind:** 既有缺口（`03` 就在，本票把它的边界划清了但没有补上）
- **Where:** `src/discover.rs` 的 `push_children`（`read_dir` 出错就整棵子树跳过）、
  `src/survey.rs` 里点不开那一支的 `(Provenance::Discovered, Container::Directory)` 一格
- **Why it did not block:** 权限不足、盘拔了、路径太长——发现走不进的那个目录底下有没有卷
  谁都不知道，而这一趟从头到尾一个字都不说。本票新加的那张表**收不下它**：
  spec 的第三类写的是「发现出来但点不开的**归档**」，`CONTEXT.md` 的词条把三类都写成
  **文件**，ADR 0014 决定第 5 条也只说归档——而一个目录不是文件。
  本票因此把这一格明确成「跳过」，与 `push_children` 那句「列不动这一层就整棵子树跳过」
  同一条处置；改之前它是**碰巧**会进那张表的（`source::open` 对目录出错也走同一支），
  那才是真不一致：一个目录路径会印在一张自称只列文件的表上。
  咬人的场景是真的：NAS 上一个权限没配好的作品目录，整棵子树静默消失，
  而退出码是 `0`、报告上一行都没有。
- **What this ticket actually did:** **只划清了边界，没有补上那句话。** 补它要么给非卷文件
  加第四类（那要改 `CONTEXT.md` 的词条——拍板级），要么另开一栏「发现走不进去的地方」，
  两条都超出本票。本票在 `NonVolumeReason::Unopenable` 与 `survey` 那一格的文档里
  各指了一句到这里。
- **Whose call:** 拍板的人（读不动的目录该进非卷文件、另立一栏，还是照旧不说）
- **处置：** **另开一栏收掉**（本票）。走的是那两条出路里的第二条——**不改
  《非卷文件》那条词条**，而在报告上并列出第四张表：`Report::unreachable_places`
  （`CONTEXT.md` 的《处理对象》新词条「走不进去的地方 (UnreachablePlace)」）。
  代码自己早论证过这一条：`NonVolumeReason::Unopenable` 的文档写着「这张表列的是文件，
  而一个读不动的目录不是文件」。
  **报的地方只有一处**：`survey::Survey::of` 里 `(Discovered, Directory)` 那一格。
  `discover::push_children` 那句「列不动这一层就整棵子树跳过」照旧不出声，
  而这一趟不会因此不出声——被它跳过的每一个目录，自己都是那一处的一个候选，
  同一个 `read_dir` 在那里再失败一次（那条推理写在 `push_children` 的文档上）。
  **它进退出码 `3`**，不新开第五个：交出的东西与卷级失败一样（一个字节都没有），
  用户下一步该查的也一样（文件还在不在、盘还挂着没有、权限变没变）——
  理由写在 `FAILED_VOLUME_EXIT` 上。
  ADR 0014 决定第 3 条里指着本条说「等拍板」那一句就地改成了指向新那一栏；
  决定第 5 条一个字没动（票面写死不许动），那一条如今不全，记在 **Q202**。
