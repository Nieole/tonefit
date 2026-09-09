# 17 — 分卷 `.rar` 折成一个卷

**What to build:** 分卷 `.rar`（`x.part1.rar` / `x.part2.rar`）今天被当成**两个卷**，
而 UnRAR 打开头一份就跨卷读完了——盘上因此出**两份重名不同的产物**
（`第01卷.part1.cbz` 与 `.part2.cbz`），其中一份是重复的。

开卷前读一次归档头里的分卷标志，把它们折成一个卷，卷名取分卷序列的名字。

**这一票推翻 `ADR 0015` 决定第 1 条的一半**：格式的**识别**仍只看扩展名，
但「这一份是不是另一份的续」要看内容。修订那一篇，写清松的是哪一半、为什么。

**固实与否仍不探**——那只影响快慢、不影响产物对不对（那一半在 25 号票里只订正文字）。

收停车场的 **Q124**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 两份 `.part*.rar` 出**一个**输出容器，内容与同内容的单份 `.rar` 逐字节相同
- [x] 卷名取分卷序列的名字，报告与进度条印的是同一个
- [x] 单份 `.rar`、`.7z`、`.zip`、`.cbz` 四种一格不变
- [x] 缺了中间一份的分卷序列当场说得出，不静默出半个卷
- [x] `ADR 0015` 决定第 1 条就地修订，写明松的是「续不续」这一半、不是「是哪种格式」
- [x] 三条闸门全绿

## 落地记录

分卷 `.rar` 从此是**一个**卷。改动分两半：一半在 tonefit 里（发现那一层认得出「续的那一份」），
另一半在**依赖里**——`unrar-ng` 0.7.7 的跨卷回调有一句越界读，不修就没有一条跨卷的路走得通。

### 先说依赖那一半：仓库里从此躺着一份打了补丁的 `unrar-ng`

上一轮跑这张票的 agent 在这里停了线，诊断成立：`unrar-ng` 0.7.7 的跨卷回调
（`Internal::<M>::callback` 的 `UCM_CHANGEVOLUMEW` 那一支）里有一句越界读。
**病在哪、怎么改、什么条件下撤掉，唯一出处是 `vendor/unrar-ng/PATCH.md`**，这里不复述。

要在这里说的只有一句：**这一支只有跨卷时进得去**，所以单份 `.rar` 一辈子踩不到——
仓库里此前也从没有一条用例走过跨卷这条路，洞因此一直没被踩到。而 `.rar` 那两条路
（`rar_headers` 列成员、`spread_rar` 摊开）都要从卷边界上走过去：**列成员那一遍也踩**，
因为 `OpenArchive<List, _>` 的 `Iterator` 每读一条头都走一次
`Internal::<Skip>::process_file_raw`，回调就是在那里挂上去的。

**处置：本地 `[patch.crates-io]`。**

| | |
|---|---|
| 放在哪 | `vendor/unrar-ng/`，0.7.7 整包 244 KB |
| 差哪一行 | `src/open_archive.rs`：`from_ptr_truncate(p1, 2048)` → `from_ptr_str(p1)`（先扫 NUL、只复制到那里），连同它上面那两行说 "2048 seems to be the buffer size" 的注释——留着就是一句假话 |
| 别的 | **与上游逐字节相同**。`diff -r` 只报得出这一处、被摘掉的 `.cargo-ok`、以及新加的 `PATCH.md` |
| 许可 | `LICENSE-MIT`、`LICENSE-APACHE` 跟着躺在同目录（MIT OR Apache-2.0）。被 `unrar-ng-sys` 编进二进制的那份 UnRAR C++ **不在这里**，照旧从 crates.io 上来，约束一格没变 |
| 什么条件撤掉 | **上游发出带这个修的版本（0.7.8 或更新）**，这个目录与 `[patch.crates-io]` 那一节一起删掉，依赖版本往上抬。不需要别的条件 |
| 说明在哪 | `vendor/unrar-ng/PATCH.md`；`Cargo.toml` 那一节与 `THIRD-PARTY-NOTICES.md` 的《UnRAR》各指过去 |

`[workspace] exclude = ["vendor"]` 把它挡在成员之外——**不挡的话 `cargo fmt` 与两遍 clippy
会去管别人的源码按不按本仓的规矩写**，那正是「跟上游对不上」的开始。
三个 target 目录的安排一格没动。

**给上游的 bug 报告草稿**写在 `.scratch/p4-parking-lot/upstream-bug-unrar-ng.md`
（英文，照 GitHub issue 的形状）。它跟着本次提交进仓库，但**本轮没有把它发给上游**
——往仓库外发东西是拍板的人的事。它**故意自成一份、不引仓库里的路径**：
它要出门，而读它的人手上没有 tonefit。

### 再说 tonefit 这一半：两问分开

ADR 0015 决定第 1 条原先是一句话：「判定只看扩展名，不去嗅内容」。本票把它劈成两问：

- **是哪一种格式** —— 照旧只看扩展名，一个字节都不读。这半句一格没动。
- **这一份是不是另一份的续** —— **看内容**：读一次归档头里的分卷标志。

第二问名字上答不了：`第01卷.part2.rar` 与一份恰好起了这个名字的**单份**包逐字相同，
而前者不是一个卷、后者是。按名字猜，猜错哪一边都要付代价——把单份包当成续就把它整个丢掉，
把续当成单份包就出两份重名不同的产物（那正是修订前盘上发生的事）。

落成三样东西，都在 `src/source.rs`：

| | |
|---|---|
| `SplitPart` | 一份归档在分卷序列里是哪一份：`First` / `Continuation`，不分卷是 `None` |
| `split_part_of` | 那一问的入口。挂在[格式集](`ARCHIVE_FORMATS`)的新一格 `ArchiveFormat::split` 上，**四个格式里只有 `.rar` 有值**——加一项仍然只改那一处 |
| `sequence_name` | 分卷序列的名字：卷名去掉末尾那一截 `.partN`。**只看名字**，而它只在内容那一问已经答出「头一份」之后才轮得到 |

发现那一层（`src/discover.rs`）改动只有一行代码：`expand` 从 `name_of` 改走
`volume_name_of`——续的那一份在那里回 `None`，与「末级分量取不出名字」走同一支
`continue`。模块文档那句「本模块**不打开任何东西**」改成「不**解**任何内容」，
并把这唯一一处例外连同理由写在下面。

点名一份**续的**分卷（`x.part2.rar`）是**整趟拒绝**（`identity_of`），那句话指得出该点哪一个。
票面没要求这一条，但 `identity_of` 的文档写着「它认『这是不是一个卷』」，
不落就成了两套说法：点名目录得到一个卷，点名它里面那份 `part2` 得到半个卷。走的是既有的
「点名的那一个点不开 → 整趟拒绝」（ADR 0014 决定第 5 条），**不新开一种结局**（记 Q325）。

**多开的那一次归档头只有一次。** 落地途中收敛过一处：`identity_of` 原先被
`open_taking_solid_archives` 拿来问容器形态，那样 `open` 与 `enumerate` 会**每卷各白开一次**
归档头。抽出 `container_of`（只答容器形态、不问分卷），两条开卷的路改走它——
分卷那一问因此只在发现那一头问，一个候选一次。
赚回来的比付出去的大：续的那几份从此连预扫都不开，而从前每一份都要被 UnRAR **整套列一遍**
（跨卷，一份要把整个序列读完）。

### 「缺了一份」那句话认的是哪个错误码——推的是什么，实测是什么

票面第 4 条要「缺了中间一份的分卷序列当场说得出」。

**动手时推的是**：UnRAR 走到卷边界、要接的那一份不在时回调答 `-1`，DLL 回 `ERAR_EOPEN`；
`When` 猜的是 `Read`（`read_header` 里 `RARReadHeaderEx` 出错走的是 `When::Read`），
保险起见写成 `When::Read | When::Process`。**这一段是推断，动手时没有实测。**

**实测是 `(Code::EOpen, When::Process)`，不是 `Read`。** 把那一支收成只有 `Read` 再跑，
用例当场红，报出来的是 `Could not open next volume`。原因：卷边界上那一下切换发生在
**读完头之后那一次 `RARProcessFile`**——列成员那一遍也走它（UnRAR 用它把当前成员跳过去）。

而且这一对不必靠推：**上游 `unrar_ng::error` 自己的那张表就把 `(EOpen, Process)` 写成
"Could not open next volume"**，别的 `EOpen` 才是 "Could not open archive"
（`vendor/unrar-ng/src/error.rs:106`）。认哪一对与上游认的是同一对，这才是那句话的出处。
判据的精度另记 **Q326**。

### 用例

| 票面 | 用例 |
|---|---|
| 两份 `.part*.rar` 出一个输出容器 | `tests/discovery.rs` 的 `a_split_rar_comes_out_as_one_volume_named_after_the_sequence`（点名装着两份分卷的目录，报告一卷、盘上一个 `库/第01卷.cbz`） |
| 内容与同内容的单份 `.rar` 逐字节相同 | `tests/container.rs` 的 `a_split_rar_reads_across_its_volumes_and_comes_out_like_a_single_one`（分卷那一卷与 `rar::SOLID` 那一卷 `read_cbz` 逐字节相等；两卷都断言摊开过） |
| 卷名取分卷序列的名字，报告与进度条印的是同一个 | 同上第一条：去处是 `第01卷.cbz`（`.part1` 那一截不进产物），而 `Event::VolumeStarted` 的卷标识与 `VolumeReport::volume` **拿观察者比过**，两边都指着头一份。**后半句取的是「两处不许分道」这一读法，不是「两处印的都该是序列名」**——见下一节 |
| 四种单份格式一格不变 | 既有那几条原样绿：`every_archive_format_turns_the_same_pages_into_the_same_product`、`a_solid_rar_and_a_stored_one_come_out_the_same`、`each_format_carries_its_own_way_of_being_read` |
| 缺了中间一份当场说得出 | `tests/container.rs` 的 `a_split_rar_missing_one_of_its_parts_says_so_instead_of_making_half_a_volume`（三份拿掉中间那一份：头一份进非卷文件、那句话说得出「下一份不在」、末一份**不另报一条**、别的卷照做）。加 `naming_a_continuation_of_a_split_rar_refuses_the_whole_run`：点名 `part2` 是整趟拒绝，那句话指得出该点 `.part1`（**评审指出来的**：新增的用户可见拒绝原本一条用例都没有） |
| `ADR 0015` 决定第 1 条就地修订 | `docs/adr/0015-...md`：决定第 1 条劈成两问、写明松的是哪半句；《后果》加一条代价、《备选方案》加一条「续不续也按名字认」 |

另加两条本票新代码该有的守卫（`src/source.rs` 的 `mod tests`）：

- `only_rar_is_asked_where_it_sits_in_a_split_sequence`——三个格式一格都没有、问下去恒是
  `None`（**一个字节都不读**），`.rar` 那一格有值。与 `each_format_carries_its_own_way_of_being_read`
  同一张表、同一把尺子。
- `a_split_sequence_is_named_without_its_part_number`——`.partN` 那一截去得对不对：
  多位数、大小写、`v1.2.part3` 只去末尾那一截、`part` 后面没数不算、数后面还有字不算、
  去掉之后什么都不剩（`.part1`）就不去。

### 屏上印的是**头一份的文件名**，不是序列名——票面第 2 条后半句取的是哪一读法

票面第 2 条后半句「报告与进度条印的是同一个」有两种读法（**评审指出来的**）：
两处印的是**同一个东西**（不许分道），还是两处印的都该是前半句那个**序列名**。

**取的是前者。** 理由是 `render::volume_name`（命令行与会话共用的那一处）印的**一直是
源文件自己的名字**：一个 `第10话.zip` 也印成 `第10话.zip`，与它的去处 `第10话.cbz`
从来就不同。要让分卷这一卷印 `第01卷`，那个函数就得改印「卷名」而不是文件名——
四种格式屏上的样子一起变，还丢掉「这一卷是从哪个文件读的」这条信息，
而且渲染层拿不到「这一份是不是分卷」那一位（它不许读内容），只能按名字猜，
那正是决定第 1 条修订之后不许干的事。

因此**序列名出现在去处上**（`第01卷.cbz`），**屏上出现的是入口那个文件**
（`第01卷.part1.rar`）。用例钉的是两处不许分道，并在它自己的文档里写明它**没有**钉另一种
读法。这个岔口记 **Q331**。

### 夹具：手搓的 RAR5 分卷写入端

`tests/fixtures/rar.rs` 的 `write_split`。仓库里那三份 `.rar` 是拿 `Rar.exe` 造好签进仓的——
票面要的「真压过」「真加密」只有 RARLAB 的写入端给得出。**分卷这一份不同**：
它要的是归档头里那一位，成员一律**存储不压**，一个压缩算法都不碰，因此手搓得出来，
也用不着 UnRAR 许可挡着的那一半。

**每一道卷边界都必须劈开一条成员**——这一条非记不可：UnRAR 认「这一份是不是头一份」时，
会拿**续那一份头一条成员头上的 `SPLIT_BEFORE`** 改写归档头里的卷号
（`archive.cpp` 的 `IsArchive`）。边界正好落在两个成员之间的包，续的那几份会被认成头一份，
折不成一个卷。因此 `parts` 不能超过成员数（三条）。真 WinRAR 打的分卷包一份都没验过，
那一半（跨卷的压缩流接得上）是 UnRAR 的事、与本票改的那一行无关——记 **Q329**。

### 顺带把一条被改成假话的理由订正了

ADR 0015 决定第 3 条「不逐卷探固实与否」的理由之一原先写着「逐卷探要**先读一遍归档头**，
而那正是这条决定想省掉的那一次」。本票为了认分卷，`.rar` 那一头**每个候选都开一次头**了
——而固实那一位（`ArchiveFlags::SOLID`）就在同一个头里，一分钱不多花。**那条理由塌了半边**
（**评审指出来的**）。

票面写着「固实与否仍不探」，因此**决定一格没动**，动的只有那句话：从原来那一条里摘掉，
改写进本票新加的那条代价底下，并说清 `.rar` 上剩下的理由只有《备选方案》里那一条
——两条读取路径都要留着、都要测。`src/source.rs` 的 `ArchiveReading` 同一句一并改，
`SolidFormat` 那一处改成指过去、不再各写一遍。`.7z` 那一头一格没变：它至今连开都不开。
要不要就此逐卷探固实，记 **Q332**（与 Q125 同一本账，地界在 25 号票）。

### 词汇表

`CONTEXT.md` 的《处理对象》加一条新词**《分卷序列 (split sequence)》**，
摆在《发现》与《点名的 / 发现的》之间。它写清三件事：这一组合起来是**一个**卷、
卷名取序列的名字、续的那几份**不是**非卷文件。末一件那句话写成**带条件**的
（**评审指出来的**）：它们属于头一份那一卷——那一卷做成了，字节一个不少地进产物；
那一卷做不成，说得出为什么的是**头一份**那一条，而其余那几份在报告里一个字都没有
（那个缺口记在 Q326 的后半）。
《卷 (Volume)》那一行仍写着「一个目录或一个归档」，**一个字没动**——改写已有词条的含义
是领域决定（`CLAUDE.md`），记 **Q327**。

### 停车场

| id | 一句话 |
|---|---|
| **Q323** | 仓库里从此躺着一份别人的源码（`vendor/unrar-ng/`）：怎么放、怎么撤、bug 报告谁去提 |
| **Q324** | 「续不续」这一问摆进了**发现**，而发现原先一个字节都不读——那条不变量就地放宽了 |
| **Q325** | 点名一份续的分卷从此是整趟拒绝，票面没要求这一条 |
| **Q326** | 一个**断掉的**分卷序列在报告里长什么样：那句话认的是哪个码，以及末一份凭空消失 |
| **Q327** | 《卷》那一行仍写着「一个归档」，而一个分卷序列是好几个文件 |
| **Q328** | 只认 `.partN.rar` 一种打法，`.7z.001` / `.z01` / 老式 `.r00` 一概不认 |
| **Q329** | 分卷夹具是手搓的 RAR5 存储不压包，真 WinRAR 打的分卷包一份都没验过 |
| **Q330** | 基线上带着一条 `unused import` 的告警，不是本票弄出来的（**已了结**：`ead9574`） |
| **Q331** | 票面第 2 条后半句两种读法，取的是「两处不许分道」，不是「两处都印序列名」 |
| **Q332** | 决定第 3 条「不逐卷探固实」原先给的理由，在 `.rar` 上被本票推翻了 |

收停车场 **Q124**。

### 数

按 1→2→3 的次序跑满，`TMPDIR` 指到本工作树自己的 `.tmp`
（四棵 worktree 并排跑时，`tests/container.rs` 那几条看摊开的用例扫的是公共的
`std::env::temp_dir()`，兄弟树正在摊的目录会被认领；停车场 Q340 记着这件事）。
读的不是末一行——末一格恒是 Doc-tests（本仓库没有文档用例，恒是 0 通过）。

| 闸门 | 数 |
|---|---|
| `cargo test`（目录 `target`） | **829 通过 0 失败**；lib 220 / bin 334。基线 823（lib 218 / bin 334） |
| `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | **701 通过 0 失败**；lib 220 / bin 206。基线 695（lib 218 / bin 206） |
| `cargo check --features profiling`（目录 `target/gate/profiling`） | 干净 |

**两条各多 6，而且是同样的 6**——本票新增的用例一条都不在 `tui` 后面：

- lib +2（`src/source.rs` 的 `mod tests`）：`only_rar_is_asked_where_it_sits_in_a_split_sequence`、
  `a_split_sequence_is_named_without_its_part_number`
- 集成 +4：`tests/discovery.rs` 一条（分卷折成一个卷），`tests/container.rs` 三条
  （跨卷读完、缺了一份、点名续的那一份）
- bin 一格没动：本票一行界面代码都没改

`cargo xtask polish` 四条全绿（`cargo fmt --check`、两遍 `cargo clippy --all-targets`、
`cargo doc --no-deps`）。**`cargo doc` 仍是 15 条告警，一条没多**——本票新加的那些
文档链接（`SplitPart`、`split_part_of`、`split_sequence_name`、`name_in_sequence`、
`container_of`、`ArchiveFormat::split`）一条都没落进那 15 条里。

分支基线 `cf09df7`。**基线自带一条 `unused import: VolumeReport`
（`src/session/draw/probe.rs`）的 clippy 告警，不是本票弄出来的**——它随 `nfl/01`
那次合流进来，集成分支上已由拍板的人在 `ead9574` 收掉；本票一个字没动它
（只提交这张票动过的文件）。
