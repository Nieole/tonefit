# 06 — `.rar`

**What to build:** 点名一个 `.rar` 直接就能跑，复用 05 的摊开机制——固实与否都走同一条，不逐卷探。

这张票带**许可**：`.rar` 的依赖是 UnRAR，它禁止据其反推解压算法。这仓库维护着 `THIRD-PARTY-NOTICES.md` 与 `licenses/`，两处都要留字。这也是 ADR 0015 与 0014 分成两篇的理由——带依赖与许可的那一半将来最可能被单独推翻。

加密卷不另开一种结局，走「点名的 / 发现的」那条既有分别。

**Blocked by:** 05 — `.7z`（摊到临时目录的机制在那张票里立起来）。

**Status:** resolved

- [x] 点名一个 `.rar`，产物与同内容 `.cbz` **逐字节相同**
- [x] 固实压的 `.rar` 与 store 的 `.rar` 产物相同
- [x] 加密卷：**点名**它退出码 `1`；**发现**它进非卷文件清单，其余卷照做
- [x] UnRAR 许可进 `THIRD-PARTY-NOTICES.md` 与 `licenses/`
- [x] 三条闸门全绿

## 落地记录

### 选的是 `unrar-ng`

`unrar-ng = "0.7.7"`，`default-features = false`。四项各说一句：

- **许可**：两层，**要分开说**。`unrar-ng` 与底下 `unrar-ng-sys` 这两层 Rust 包装自身是
  MIT OR Apache-2.0；被 `unrar-ng-sys` 的 `build.rs` **编进 tonefit 二进制**的那份 UnRAR C++
  源码（7.21.1）不是——它走 UnRAR license。这与 `05` 那边正相反：`sevenz-rust2` 是
  Apache-2.0，`05` 因此在这两处**一个字都没加**；本票非加不可，处置见下面《许可怎么处置的》。
- **维护活跃度**：与 `05` 判 `sevenz-rust` → `sevenz-rust2` 同一条道理。原 `unrar`
  （`muja/unrar.rs`）停在 `0.5.8`（2025-02-19），随的是 UnRAR 7.1.0（2024-05）；
  `unrar-ng`（`ttys3/unrar.rs`）是接手的那一支，README 第一句写明
  「Actively maintained fork of `unrar`」，当前 `0.7.7`（2026-05-07），随的是 UnRAR 7.21.1
  （2026-03）。
- **要不要编 C 源码**：**要**，两个候选都要——`.rar` 只能走 UnRAR 那份 C++ 源码。
  这不是选型上的取舍，是 ADR 0015 把 `.rar` 单独成篇的那条理由本身：一个从零写起的纯 Rust
  rar 解码器要么就是 UnRAR 许可禁止的那件事，要么许可说不清楚。`unrar-rs` / `rarpar`
  那一类正落在这一档（crates.io 上 license 一栏是 `unknown`），**排除**。
- **Windows 上构建得起来没有**：**起得来**，本机 Windows 11 + MSVC 上一次编过，
  约 40 秒，之后进增量缓存。`unrar-ng-sys` 的 `build.rs` 靠 `cc` 找编译器，不需要预装
  libunrar。这条新前提没有落脚处，记进了 **Q123**。

**没选的**：`unrar 0.5.8`（同许可结构，但已慢一年半，且逐个成员各搜一遍块头——
README 里那张表上 94000 个文件慢 10 倍）；`unrar-rs`（纯 Rust 反推实现，见上）；
`compress-tools` 一类 libarchive 绑定（要一个系统库，且把一整套多格式抽象拖进来）。

**用到的三个 API**，各对着一件事：

- `Archive::open_for_listing`——只走归档头，一个内容字节都不解。预扫走它
  （`source::rar_headers`），**句柄在那一句里就放掉**。
- `read_header` + `read`——按归档里的次序**一条顺序扫**，一个成员一个成员往前走。
  固实归档的成员压在一条连续的流里，顺着解一遍是它唯一便宜的读法。
- `skip`——目录项与垃圾成员从这里过去，盘上不留。

**没用 `extract_all`**（`unrar-ng` 那条批量入口）：它让 libunrar 自己按包内的名字写盘，
等于把路径那一道交给 C++ 那一侧，而包装层剥到哪一级、垃圾成员摘不摘、`..` 与盘符收不收，
都是这一侧的规矩（`source::relative_path` / `is_junk` / `strip_wrapper_directory`）。
代价是一个成员先整个进内存再落盘——而一个成员本来就整个进内存（`Reader::read`）。

**依赖面上多出来的是三个包**：`unrar-ng`、`unrar-ng-sys`，加一份 `widestring 1.2.1`
（UnRAR 的宽字符成员名要经它转成 `PathBuf`）。`cc` / `libc` / `winapi` / `bitflags` / `regex`
一个都不是新的——树里本来就有。**默认特性关掉了**，但那一格省不掉 `libc`：
它是 `unrar-ng-sys` 的**非可选**依赖，默认特性管的只是 `extract_all` 在 Linux 上写盘时的
文件名编码，而本仓不走那条路。

### 接上去动了哪几处

`05` 的《落地记录》把「`06` 要动哪几处」说准了，照着来的：

- **格式集加一行**：`ARCHIVE_FORMATS` 从三项变四项，次序照 ADR 0015 决定第 1 条写的
  `.cbz / .zip / .rar / .7z`。拒绝那句话与 `--help` 的「在哪里找卷」自己跟着走了——
  两处都从 `listed_archive_extensions` 取，一个字面量都没改。
- **`ArchiveReading::Extracted` 带上了那个格式的两个入口**（新的私有
  `struct SolidFormat { open, list }`，两格都是 `type OpenVolume = fn(&Path) -> Result<Volume>`）。
  **没有第二处 `match`**：格式集那一行直接写着去哪个解码器要字节，
  `open_taking_solid_archives` 收的是「在一个格式的两个入口里挑哪一个」
  （`|format| format.open` / `|format| format.list`），与格式无关。
  「格式集加一项就只改这一处」那句话因此仍是真的——**这一条是评审提出来的**：
  头一版立的是 `enum Solid { Rar, SevenZip }` 加两个分派函数，那会让那句文档说假话，
  也多出一对同构的 `match`。
- **`extract` 泛化了**：`05` 那个签名钉在 `sevenz_rust2::ArchiveReader<File>` 上，
  现在收一个 `impl FnOnce(&Path) -> Result<u64>`。临时目录、它的前缀、
  「把 X 摊到临时目录」那句话、摊不下就整份收走，都留在 `extract` 里；一格式一份的
  只有「按什么次序把成员的字节交出来」（`spread_seven_zip` / `spread_rar`）。
- **摊开那一档的两个格式共用三处**，各自只剩「问自己那个库要什么」：
  `extracted_volume` / `unextracted_volume`（交卷那一下的三条不变量——`root` 仍指着归档文件、
  `container` 仍是 `Archive`、读取端是目录还是没摊开——只写一处），
  `solid_members`（目录项不算成员、名字要能当相对路径、垃圾摘掉、包装层剥掉——
  两个库各自那种条目摘成「名字 + 字节 + 是不是目录项」三样交进来）。
- **原样复用的**：`Extraction` 的寿命、`write_extracted_member` 的落盘与路径安全、
  `Reader::Unextracted`、报告那一格与它那一行、`cost::Stage::Extract`。
  `.rar` 卷的 `Volume::root` 仍指着那个 `.rar`，`container` 仍是 `Archive`，
  下游一处都不知道它原来是个 rar。
- **`.cbz` / `.zip` / `.7z` 一个字节没变**：随机取那条路一个字符没动，`.7z` 那一支只被
  改了名（`open_solid_archive` → `open_seven_zip` 等）与摊开那一句的调用形。
  黄金回归那一整摞原样全绿。

**归档头要读两遍**，这是 `.rar` 与 `.7z` 唯一实打实的不同：UnRAR 交出来的是一个只能往前走
的游标，而摊开的去处要等成员表齐了才定得下来（包装层剥几层，得看全卷共有的前缀是什么）。
两个句柄不重叠——列成员那一个在 `rar_headers` 返回时就放掉了，`Reader`
的《一趟同时开着几个句柄》那一格因此一个数都没变。

### 加密卷那两条是怎么测的

夹具是 `rar a -hptonefit`：**连归档头一起加密**。于是列成员的第一条就回
`MissingPassword`，那一卷**点不开**——之后走的完全是既有的那条分别，没有新代码：

| | 用例 |
|---|---|
| 点名它退出码 `1` | `tests/exit_code.rs` 的 `an_encrypted_rar_is_refused_when_named_and_skipped_when_discovered`（同一条里一并问发现它是 `0`） |
| 发现它进非卷文件清单、其余卷照做 | `tests/container.rs` 的 `an_encrypted_rar_is_refused_when_named_and_listed_as_a_non_volume_file_when_discovered`（理由那一格是 `NonVolumeReason::Unopenable`，好的那一卷真在盘上） |

**那句话要说得出为什么**：`rar_is_unreadable` 把带口令的包与坏掉的包分开说
（`MissingPassword` / `BadPassword` 一支，其余一支）。合成一句的话，用户会被支去修一份
好好的包。这是本票唯一为加密加的代码，**不是一种新结局**。

只加密数据、不加密头的那一种（`rar a -p…`）列得出成员、摊开时才失败，落在**卷级失败**
那一格（退出码 `3`），与 `05` 那个压缩流被打坏的 `.7z` 同一条路。同样不是新结局，没另写用例。

### 验收那四条各由谁钉着

| 票面 | 用例 |
|---|---|
| 产物与同内容 `.cbz` 逐字节相同 | `tests/container.rs` 的 `every_archive_format_turns_the_same_pages_into_the_same_product` |
| 固实压的与 store 的产物相同 | `tests/container.rs` 的 `a_solid_rar_and_a_stored_one_come_out_the_same`（两份夹具成员逐一相同，差的只有 `-s -m5` 与 `-s- -m0`；两卷都断言摊开过——不逐卷探这一条正体现在这里） |
| 加密卷点名 `1` / 发现进清单 | 见上一节那张表 |
| 许可两处留字 | 见下一节 |

**头一条落成了四个格式一起比**，而不是 `.rar` 与 `.cbz` 两两比：批 spec 的
《Testing Decisions》要的是「同一批页从 `.cbz` / `.zip` / `.rar` / `.7z` 进去，
出来**逐字节相同**」，而格式集要到本票才齐，这一条至此才摆得出来（**评审指出来的**：
头一版只写了 `.rar` 对 `.cbz`，靠 `.cbz` 传递才成立）。同一条里一并断言输出扩展名四份都归一到
`.cbz`，以及摊开只发生在该发生的两个格式上（`.cbz` / `.zip` 那两卷摊了 0 字节）。

另外两条不在票面上、但属于新代码该有的守卫：

- `tests/container.rs` 的 `a_rar_that_cannot_be_extracted_fails_only_its_own_volume`——
  成员字节被打坏的 `.rar`，归档头完好、预扫列得出成员，只有真去摊开才失败。
  它钉的是 `spread_rar` 那条错误路径（**评审指出来的**：ADR 0015 那句「磁盘不够是卷级失败」
  在 `.rar` 上原本一条用例都没有）。「磁盘不够」本身在用例里造不出来，
  退出码那一格由 `tests/exit_code.rs` 那条钉着，而它对格式无所谓——
  它断在临时目录根本不在上，摊开那一层是两个格式共用的。
- `src/source.rs` 的 `each_format_carries_its_own_way_of_being_read`——四个格式
  （含大小写各一）走哪一条读法，一次问全。它**只问走哪一条，不问接的是哪个解码器**：
  后者现在是格式集里那两个函数指针，在单元用例里比等于比函数地址；
  「`.rar` 真接上了 rar 那一支」由上面那几条拿真包答。

### 许可怎么处置的

- **`licenses/UnRAR.txt`**：UnRAR 许可全文，逐字取自 `unrar-ng-sys` 随附的
  `vendor/unrar/license.txt`（与 `muja` 那一支随的那份**逐字节相同**，比对过）。
- **`THIRD-PARTY-NOTICES.md`**：新开《UnRAR》一节。抬头那句也改了——从前只说「一份随程序
  分发的第三方素材」，现在是两样：一份签在仓库里的字模，一份编进二进制的第三方源码。
  节里点明三件事：**两层许可要分开说**（Rust 包装是 MIT OR Apache-2.0，被包进去的 C++ 不是）；
  **第 2 条是真约束**（不得用于开发 RAR 兼容的打包器、不得重建 RAR 压缩算法，
  且分发修改过的源码时该条全文必须随行）；**它跟着二进制走**，谁分发 tonefit 的可执行文件
  谁就要一并带上 `licenses/UnRAR.txt`。

`.rar` 只读不写，输出仍一律 `.cbz`（ADR 0015 决定第 2 条）——这也是本仓离那条禁止最远的
一个位置：tonefit 一处都不产出 rar。

### 夹具是签进仓的，不是生成的

这是本票与全仓惯例唯一分道的地方，因此单说一段（另记 **Q122**）。

`tests/fixtures/rar/` 下三份二进制（`solid.rar` 640 B、`stored.rar` 12.6 KB、
`encrypted.rar` 1.2 KB，共约 14 KB），由 WinRAR 的 `Rar.exe` 造好签进仓，
`tests/fixtures/rar.rs` 用 `include_bytes!` 拿它们。**造不出来**：Rust 这边没有 rar 的写入端，
将来也不会有——那正是 UnRAR 许可禁止的那件事。而票面第二条要真压过的固实包、
第三条要真加密的包，两条都非真包不可。

三条命令行逐字记在 `tests/fixtures/rar.rs` 抬头，谁手上有 WinRAR 谁就重造得出。
包里那两页是 `fixtures::cheap_page()` 与 `fixtures::gradient(TINY)` 编成的 PNG，
另外三个格式那一侧由 `rar::members()` 灌出同一份内容——
**四个包装着同一份内容**这个前提，两侧各有且只有一个出处。
**只用 PNG、不用 JPEG**：有损格式的编码结果随 `image` 换版本而变，签进仓的字节不跟着变，
两边一分道，用例比的就不再是这一趟的差别。生成器一改，用例**当场红**，不会静默地过。

### 数

按 1→2→3 的次序跑满。读的不是最后一行——末一格恒是 Doc-tests（本仓库没有文档用例，
恒是 0 通过）——而是 17 个测试二进制的 `test result` 行加起来。

| 闸门 | 数 |
|---|---|
| `cargo test` | **635 通过 0 失败**，17 个二进制。基线 630，多的 5 条即上面两张表点到的那些 |
| `cargo test --no-default-features` | **597 通过 0 失败**，17 个二进制。基线 592，多的同样是那 5 条——一条都不在 `tui` 后面 |
| `cargo check --features profiling` | 干净 |

多出来的 5 条：`tests/container.rs` 四条（四格式同批页、固实对存储、摊不开、加密卷），
`tests/exit_code.rs` 一条（加密卷的两个退出码）。原有那条
`a_file_that_only_looks_like_a_seven_zip_is_refused_for_being_unreadable` 改名成
`..._solid_archive_...` 并改成两个格式各问一遍，条数不变。

`cargo fmt --check` 与 `cargo clippy --all-targets`（默认与 `--no-default-features` 两种）
均零告警。

### 停车场

- **Q122**：`.rar` 的夹具是造好签进仓的二进制，页的生成器一改就要有 WinRAR 的人重造一遍。
- **Q123**：收下 `.rar` 之后构建 tonefit 多了一个 C++ 工具链的前提，仓库里没有一处说得出。
- **Q124**：分卷 `.rar`（`x.part1.rar` / `x.part2.rar`）会被当成两个卷，而 UnRAR 打开头一份
  就跨卷读完了，产物重一遍。**评审指出来的**；本票一处没读、一个字没写——
  「一套分卷算不算一个卷」是拍板的事，而认出它要读归档头，撞 ADR 0015 决定第 1 条。

`05` 开的 **Q119 / Q120 / Q121** 三条，`.rar` 原样继承——摊开的卷仍按「归档卷是一条顺序扫」
派读取、摊开那一份靠析构收硬杀会留孤儿、摊开一整卷中途没有检查点。三条一个字没改，
也没有为 `.rar` 另记一份。
