# 05 — `.7z`：整卷摊到临时目录

**What to build:** 点名一个 `.7z` 直接就能跑。开工前整卷摊到临时目录，之后**完全按目录卷走**；跑完临时目录收干净，中断也不留孤儿。

摊开不是图省事，是因为**固实归档取第 N 个成员要从块头解起**：随机取 O(N)、全卷 O(N²)，而源字节本来就要读两遍（幂等一遍、第一遍一遍）。`.zip` 家族不摊——每个成员各自 deflate，随机取 O(1)，摊开是白付一次全量写盘。读取形态因此**按格式分**，不逐卷探固实与否。

摊开借缓存溢写那套匿名临时文件机制，**不新开预算旋钮**：`--cache-budget` 限的是内存，磁盘今天没有旋钮，再加一个会让「限制占用」有两个说法。摊了多少字节印进那一卷的报告，让人看得出这笔账。

**Blocked by:** 02 — 归档扩展名归一（格式集与输出归一在那张票里立起来，ADR 0015 也在那里）。

**Status:** resolved

- [x] 点名一个 `.7z`，产物与同内容 `.cbz` **逐字节相同**
- [x] 跑到一半时临时目录里有东西；跑完之后它不在了；**中止之后也不在**
- [x] 摊了多少字节印进那一卷的报告
- [x] 磁盘不够是**卷级失败**，其余卷照做，退出码 `3`
- [x] `.cbz` / `.zip` 仍走随机取，产物与改动前逐字节相同
- [x] `CONTEXT.md`《I/O 与并发》加归档卷的读取形态：两种格式两条路
- [x] 三条闸门全绿

## 落地记录

### 选的是 `sevenz-rust2`

`sevenz-rust2 = "0.22.2"`，**Apache-2.0**，纯 Rust。ADR 0015 只写了「取纯 Rust 实现」，
点名哪一个由本票判。四项各说一句：

- **许可**：Apache-2.0，与仓库现有依赖同一档，没有 `.rar` 那边 UnRAR 那种分发约束
  （`THIRD-PARTY-NOTICES.md` 与 `licenses/` 因此**一个字都不用加**——那两处收的是
  需要随二进制附带的许可文本，Apache-2.0 走的是与仓库其余依赖同一条路）。
- **维护活跃度**：原 `sevenz-rust`（`dyz1990`）停在 `0.6.1` 不再动；`sevenz-rust2` 是接手的
  那一支，README 第一句写明「fork of the original, unmaintained sevenz-rust crate」，
  当前 `0.22.2`。作者同时是底下 `lzma-rust2` 的作者——LZMA/LZMA2 那一半与容器那一半同一个人维护。
- **`unsafe`**：整份 `src/` 一个 `unsafe` 块都没有（三处命中全是错误信息里的英文单词）。
- **解压之外用不用得着的东西**：**用不着，因此全关掉**。默认特性里 `compress`（写 7z）与
  `aes256`（带口令的包）一律不要——输出一律 `.cbz`（ADR 0015 决定第 2 条），
  而本工具没有问口令的地方。留下 `bzip2` / `deflate` / `ppmd` / `zstd` 四个解码器：
  它们各自那个包 **`zip 8` 早就拉进树里了**，收下它们不多一个包。

**依赖面上真正多出来的只有两个包**：`sevenz-rust2` 自己，加一份 `lzma-rust2 0.20.1`
（`zip 8` 钉的是 `0.16.5`，两个版本并存；树里本来就有 `hashbrown` 三份、`serde_spanned` 两份，
这不是一类新问题）。`bzip2` / `ppmd-rust` / `zstd` / `flate2` 一个都不是新的。

**没选的**：`sevenz-rust`（同许可，但已停更，`sevenz-rust2` 就是它的接班）；
`compress-tools` 一类 libarchive 绑定（是 C，ADR 0015 写死了纯 Rust）；
`unarc-rs` / `libarchive_oxide` 一类多格式新包（太年轻，且为了一个格式引一整套多格式抽象）。

**用到的两个 API**，各对着一件事：

- `Archive::open(path)`——只解归档头，**不留读取端**。预扫走它（`source::enumerate`），
  因此在 `.7z` 上同样一个句柄都不攥（`volume-discovery/01` 那条性质原样成立）。
- `ArchiveReader::for_each_entries`——按块**一条顺序扫**。固实归档的成员压在一条连续的流里，
  顺着解一遍是它唯一便宜的读法，这正是不按成员随机取的那条理由的反面。

### 摊开这一层长什么样

- `src/source.rs` 的格式集从一串扩展名变成一张表：`ARCHIVE_FORMATS: [(&str, ArchiveReading); 3]`。
  **格式集仍只有这一个出处**——拒绝那句话、`--help` 的「在哪里找卷」、读法，三处都从它出来。
- **`06` 接 `.rar` 要动的是哪几处，说准一点**（别读成「加一行就完」）：格式集加一行
  `("rar", ArchiveReading::Extracted)`，再写一个 `.rar` 版的 `open_solid_archive` /
  `list_solid_archive`，并把 `open_taking_solid_archives` 那一格从「固实就是 7z」
  改成按格式分派——`extract` 眼下的签名钉在 `sevenz_rust2::ArchiveReader<File>` 上。
  **能原样复用的**是摊开那一层本身：`Extraction` 的寿命与前缀、`write_extracted_member`
  的落盘与路径安全、`Reader::Unextracted`、成员表那一套（剥包装层、摘垃圾、相对路径）、
  报告那一格与它那一行、`cost` 的 `Stage::Extract`、CONTEXT.md 与 ADR 的措辞。
  **没有为 `.rar` 预留任何半成品代码**——`extract` 那个签名要等真有第二个格式时再泛化，
  现在泛化就是为一个不存在的调用方设计（票面明写不许预留）。
- `open` 与 `enumerate` 分两条：前者摊开、备好读取端，后者只列成员。
  **非分不可**——`source::open` 从前预扫也在叫，若摊开发生在它里面，预扫就会把整个库的
  每一个 `.7z` 各摊开一遍再扔掉，开工之前先付两倍全量写盘。
  没摊开的读取端是 `Reader::Unextracted`，取字节回一句说得出为什么的错（不是恐慌）。
- 摊开之后 `Reader` 是 `Directory { root: 临时目录 }`，`Volume::container` 仍是 `Archive`
  （输出仍一律 `.cbz`），`Volume::root` 仍指着那个 `.7z`（报告、幂等的去处、成员身份都按它算）。
  下游没有一处认识「固实」这个词。
- 落到临时目录上的名字用的是**成员表里那条相对路径**——包装层已剥、垃圾成员已摘、
  `..` 与盘符在 `relative_path` 里就拒了，读取端于是与一个目录卷严丝合缝。
- `Extraction` 持着 `TempDir`，寿命就是那个卷的寿命；`VolumeReport::extracted` 带出摊了多少字节，
  报告里印成「摊开 3.0 MiB」，与缓存那一行共用库里那一份进位（`tonefit::format_bytes`）。
  **跳过的卷也印**：幂等那一道要把整卷读一遍，而读之前得先摊开。

### 验收那五条各由谁钉着

| 票面 | 用例 |
|---|---|
| 产物与同内容 `.cbz` 逐字节相同 | `tests/container.rs` 的 `a_seven_zip_comes_out_byte_for_byte_the_same_as_the_cbz_holding_the_same_pages` |
| 跑到一半在、跑完不在、中止之后也不在 | `tests/container.rs` 的 `a_seven_zip_leaves_no_temporary_directory_behind_even_when_the_run_is_aborted`（观察者跑到一半按包里一个独有的成员名认出那个目录）；寿命本身由 `src/source.rs` 的 `what_a_solid_archive_extracts_lives_exactly_as_long_as_the_volume` 钉 |
| 摊了多少字节印进报告 | `src/render.rs` 的 `a_volume_that_was_extracted_says_how_many_bytes_it_took`（含跳过的卷那一支） |
| 摊不下是卷级失败、退出码 `3` | `tests/exit_code.rs` 的 `a_volume_that_cannot_be_extracted_fails_alone_and_the_run_ends_with_three`（子进程的 `TMP`/`TEMP`/`TMPDIR` 指着一个不存在的目录）；库那一层由 `tests/container.rs` 的 `a_seven_zip_that_cannot_be_extracted_fails_only_its_own_volume` 钉 |
| `.cbz` / `.zip` 仍走随机取、产物不变 | 同上第一条里 `.cbz` 那一卷 `extracted == 0`；「与改动前逐字节相同」由 `tests/golden.rs` 那一整摞原样全绿担保——随机取那条路一个字符没动 |

**「磁盘不够」在用例里造不出真的**，用的是同一条路上的另一个 `Err`（临时目录根本不在）。
两者从 `source::extract` 出去时长得一样：那一卷记进 `failed_volumes`，其余卷照做。

### 数

按 1→2→3 的次序跑满。读的不是最后一行——末一格恒是 Doc-tests（本仓库没有文档用例，
恒是 0 通过）——而是 17 个测试二进制的 `test result` 行加起来。

| 闸门 | 数 |
|---|---|
| `cargo test` | **630 通过 0 失败**，17 个二进制。基线 618，多的 12 条即《验收那五条各由谁钉着》那张表点到的那些 |
| `cargo test --no-default-features` | **592 通过 0 失败**，17 个二进制。基线 580，多的同样是那 12 条——一条都不在 `tui` 后面 |
| `cargo check --features profiling` | 干净。本票真动了它盖的那一半：`cost::Stage` 多了一格 `Extract`（批 spec 的《闸门》点名了这一条） |

`cargo fmt --check` 与 `cargo clippy --all-targets`（默认与 `--no-default-features` 两种）
均零告警。**`cargo fmt` 顺手扫掉了一处与本票无关的格式漂移**：`src/lib.rs` 的
`normalises_an_extension`，在 `ac55bbf` 上就已经不合 rustfmt（另开一份工作树复核过）。
留着它闸门就不绿，因此收在本票里。

### 停车场

- **Q119**：摊开的卷两遍那一路仍按「归档卷是一条顺序扫」派（`container` 还是 `Archive`），
  而它的字节已经在一个目录里；介质也仍探的是那个 `.7z` 在哪块盘上，不是临时目录。
  本票一个字符没改——改它要动 `IoPlan::decide` 的入参或 ADR 0009 的探测边界。
- **Q120**：摊开那一份靠析构收（跑完、按停、中止都成立），但**硬杀会留下孤儿**，
  而缓存的溢写是匿名文件、由内核收。差别写进了 `Extraction` 的文档，临时目录一律
  `tonefit-` 打头好让人认得出；没有加清理——那是跨运行状态。
  **票面第三段那句「摊开借缓存溢写那套匿名临时文件机制」里的「匿名」二字做不到**，
  原因同上：成员要按名字读回来。ADR 0015 的措辞（「借溢写那套 `tempfile` 机制」）不受影响。
- **Q121**：摊开一整卷的**中途没有检查点**——ADR 0013 要的两个检查点一个在卷边界、
  一个在页边界，而摊开正落在两者之间。几百兆的 `.7z` 上按中止要等它解完才停得下来。
  票面第二条仍成立（「中止之后也不在」靠析构），不成立的是「立刻停」那半句；
  补它要让 `source::open` 收一个停止信号，那是跨模块的新依赖，超出本票。
