# 02 — 归档扩展名归一

**What to build:** 点名一个 `.zip` 直接就能跑——它与 `.cbz` 本来就是同一种字节。归档卷的输出**一律写成 `.cbz`**：产物是给阅读器的，而那是阅读器认的名字。

扩展名归一带出一种新的撞车：同一目录下的 `第10话.zip` 与 `第10话.cbz` 落到同一个去处。拒绝执行本身已经成立，但那句「输出名取自卷名，同名的卷因此撞在一起」说不出**是扩展名归一造成的**，措辞要改。

这张票同时把 **ADR 0015** 一次立全——格式集、输出归一、`.rar`/`.7z` 摊到临时目录三条决定都已拍板，05 与 06 实现它，不再回头补记。

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] 点名一个 `.zip`，产物与同内容 `.cbz` 逐字节相同
- [x] 归档卷的输出恒为 `.cbz`，输入扩展名不影响输出名
- [x] 同名的 `.zip` 与 `.cbz` 撞车时整趟拒绝、退出码 `1`，那句话说得出是扩展名归一造成的
- [x] 幂等仍认得出上一趟的输出：去处按源卷名算，扩展名归一之后仍对得上
- [x] 新增 `docs/adr/0015-*`，装格式集、输出归一、`.rar`/`.7z` 摊到临时目录三条决定
- [x] `CONTEXT.md`《输出》交代扩展名归一对幂等去处的影响
- [x] 三条闸门全绿

## 落地记录

### 一个常量分成两个：认输入的那一串，拼输出名的那一个

`src/source.rs` 原先只有 `pub const ARCHIVE_EXTENSION: &str = "cbz"`，一个常量同时承担
两件事——**认输入**与**拼输出名**。归一之后这两件事分了家：

```rust
const ARCHIVE_EXTENSIONS: [&str; 2] = ["cbz", "zip"];   // 认输入，大小写不敏感
pub const OUTPUT_ARCHIVE_EXTENSION: &str = "cbz";       // 拼输出名，恒是它
```

`is_archive` 比整个集合，`output_path_of` 只取后一个。**归一因此落在
「卷名 + 容器形态 → 输出位置」那一步**——`open` 与 `planned_output` 共用它，
算去处的那一趟与真去写的那一趟仍得出同一个名字，撞车查得准这条不变量一个字没动。
`Container`、`Volume`、`Reader` 的形状与 `open_archive` 那条读法一行都没改：
`.zip` 走的就是今天 `zip` 那一条。

`.rar` / `.7z` **一个字节都没放进来**：`ARCHIVE_EXTENSIONS` 里只有已经读得了的那两个，
点名它们仍撞上 `identity_of` 那条拒绝。拒绝那句话由 `listed_archive_extensions()` 拼出——

```
X 既不是目录，也不是认得的归档：一个卷是一个目录或一个归档（.cbz / .zip）
```

——格式集与措辞因此只有一个出处，将来 05／06 往集合里加一项，这句话自己跟着走。

### 撞车那句话点出的是**规则**，不是「撞了」

`ensure_no_two_volumes_share_an_output`（spec 的《撞车的措辞》写成 `ensure_distinct_outputs`，
那是同文件里另一个函数——**成员级**的那道，见下面《票面》）的末句从

> 输出名取自卷名，同名的卷因此撞在一起。分批处理，每批给一个自己的输出根。

改成**按撞上的那几组现拼**：共有的一句加上按来由各出一条的出路。

> 输出名取自卷名，同名的卷因此撞在一起。分批处理，每批给一个自己的输出根。

> 输出名取自卷名，同名的卷因此撞在一起。
> 上面有一对只差扩展名：归档卷的输出扩展名一律归一成 .cbz，源那一头叫什么扩展名都不
> 带过来，两份包因此指着同一个去处。这一对换输出根分不开——卷名本来就相同——
> 只点名其中一份。

两种来由共用一道拒绝，但**出路不同**：卷名撞车换个输出根就分得开，扩展名归一撞的
这一对分不开。一句把两条出路都念出来，对其中一种必然是错的指引——原话对后一种是错的，
而「两条一起念」对两个同名**目录**撞车的人同样是错的。判据是 `normalises_an_extension`：
**文件名不同**。卷名撞车的两个源文件名必然相同（`甲部/第1话` 与 `乙部/第1话` 都叫
`第1话`），文件名不同还撞得上，只可能是扩展名在算去处那一步被归一掉了。
比文件名用的是 `collision_key`——与比去处同一把尺子，不然 Windows 上 `第1话.CBZ` 与
`第1话.cbz` 会被报成扩展名归一，而它撞的其实是大小写。

那句话里也**不写死 `.zip`**：归一成什么取自 `OUTPUT_ARCHIVE_EXTENSION`，
撞的是哪一对由上面那几行 `←` 自己报，05／06 把 `.rar`／`.7z` 加进格式集时这句话不必改。

### 幂等：去处按源卷名算，因此本来就对得上

去处只取决于卷名与容器形态，源的扩展名在 `output_path_of` 那一步就不见了；
而源哈希收的是成员的相对路径与字节（`volume_fingerprint`），也不含卷的文件名。
两条合起来：**把 `第10话.cbz` 换成 `第10话.zip` 重打一遍，下一趟仍跳过整卷。**
这不是新写的行为，是归一之后仍然成立的行为——用例把它钉住，因为断了的话症状是静默的
（换个扩展名重打一次包，整库无声地重跑一遍）。

### ADR 0015 一次立全三条

`docs/adr/0015-archive-formats-and-how-they-are-read.md`，三条决定：格式集四个扩展名、
输出一律 `.cbz`、`.rar`/`.7z` 开工前整卷摊到临时目录（含那张两格式两条路的表、
固实归档 O(N²) 那笔账、不新开磁盘预算旋钮、UnRAR 许可要在 `THIRD-PARTY-NOTICES.md`
与 `licenses/` 各留一份）。状态行写明：决定第 1 条的后两个扩展名与决定第 3 条的第二行
**尚未实现**，落在 `volume-discovery/05`、`06`。

`CONTEXT.md`《输出》加一段：归档卷的去处再归一一道扩展名，`第10话.zip` 与 `第10话.cbz`
指着同一份输出，幂等因此仍认得出；代价在撞车那一侧。

### 花了多少

改了四个源文件：`src/source.rs`（常量拆开、`is_archive`、`output_path_of`、拒绝那句话）、
`src/lib.rs`（撞车那句话按来由现拼，加 `normalises_an_extension`）、`src/main.rs`（两处帮助文案）、
`src/sink.rs`（模块抬头一句）；两份文档：`CONTEXT.md`（《输出》一段）与
`docs/adr/0015-*`（新增）。`src/session/draw.rs` 一个字没动，也没给任何列表加滚动。

测试那一侧：`tests/fixtures/mod.rs` 加 `Workspace::archive`（扩展名自己带，`cbz()` 改成
经它走），`tests/smoke.rs` 把「认输入」与「认输出」两个用途分开——`ARCHIVES` 认
`.cbz`／`.zip`，`OUTPUT_ARCHIVE` 只认 `.cbz`（输出恒是它；一个叫 `第10话.zip` 的
**目录**卷的输出仍是目录，两者混用会认错）。

新增 `#[test]` **四条**：

- `a_zip_is_read_as_a_cbz_and_comes_back_out_as_a_cbz`（`tests/container.rs`）——
  同内容的一对各跑一趟、各写一个输出根，去处都是 `第10话.cbz`，成员逐字节相同；
- `an_uppercase_archive_extension_is_recognised_too`（同上）——大小写不敏感管整个集合；
- `a_zip_and_a_cbz_of_the_same_name_collide_and_the_message_says_why`（`tests/pipeline.rs`）——
  整趟拒绝、两个卷都点名、那句话说得出是归一造成的且**不给**「分批处理」那条出路、
  输出根一个字节都没建；
- `the_same_volume_packed_as_zip_or_cbz_lands_on_the_same_output_and_still_skips`
  （`tests/idempotency.rs`）——换个扩展名重打，下一趟仍是 `Skipped`。

另有两处加在**既有**用例上，不是新增：
`a_file_that_is_neither_a_directory_nor_an_archive_is_refused`
→ `..._nor_a_known_archive_is_refused`，从一个 `.txt` 扩成 `.txt` / `.rar` / `.7z` 三种，
断言换成新的那串扩展名——`.rar` / `.7z` **仍然点名即拒**这一条因此有用例钉着；
`two_volumes_that_would_write_to_the_same_place_are_refused` 多两句断言——
两个同名**目录**撞车时给的是「分批处理」且**不提**扩展名归一，与上面那条正反成对。

退出码 `1` 不另起用例：拒绝执行恒是 `REFUSED_EXIT`，那条映射由 `tests/exit_code.rs`
（真进程那一条，那份文件自己写着「整份用例只此一条」）与 `src/render.rs` 的
`exit_code` 用例钉着，与撞车是哪一种无关。

### 数

按 1→2→3 的次序跑满。读的不是最后一行——末一格恒是 Doc-tests（本仓库没有文档用例，
恒是 0 通过）——而是 17 个测试二进制的 `test result` 行加起来。

| 闸门 | 数 |
|---|---|
| `cargo test` | **595 通过 0 失败**，17 个二进制。基线 591，多的 4 条即上面那四条 |
| `cargo test --no-default-features` | **557 通过 0 失败**，17 个二进制。基线 553，多的同样是那 4 条——四条一条都不在 `tui` 后面 |
| `cargo check --features profiling` | 干净 |

### 停车场

新增 **Q109**——输出归档的每个成员带着写出那一刻的时钟（`zip` 8 的
`SimpleFileOptions::default()` → `DateTime::default_for_write()`），整个归档文件的字节
因此两趟之间就不同。票面第一条那句「逐字节相同」于是测在**成员**这一层：
名字与字节两趟完全相同，而这正是「读法有没有分岔」要证的东西。没有去动 `sink`——
把时间戳钉死是改产物的字节，落在这张票之外。

新增 **Q110**——`CONTEXT.md`《处理对象》的「源」与「输出容器」两条词条还按「CBZ」写，
其中「输出容器」那半句（「输入是 CBZ，输出也是 CBZ」）在归一之后**说不全**了。
按 `CLAUDE.md`《改 CONTEXT.md 的规矩》没有顺手改：那是改已有词条的含义，要先拍板；
而且 spec 的《要改的领域文档》已经把《处理对象》那几条派给了 `volume-discovery/03`，
在这里先改一手等于同一段话改两次。归一这条事实的权威位置是《输出》，本票写在那里。

### 票面

票面本身没有错。**spec 的《撞车的措辞》把那道拒绝写成 `ensure_distinct_outputs`**，
而实际叫 `ensure_no_two_volumes_share_an_output`；`ensure_distinct_outputs` 是同一文件里
**成员级**的那一道（一个卷之内两个源成员认领同一个输出成员）。改的是前者。
