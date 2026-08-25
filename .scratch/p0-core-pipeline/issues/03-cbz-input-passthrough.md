# 03 — CBZ 输入与非图片透传

**What to build:** 直接处理 CBZ，不必先解包。包内的非图片文件（如 `ComicInfo.xml`）原样带到输出，阅读器仍能读到书籍元信息。

ZIP 成员名可能是 GBK 编码且没有 UTF-8 标志，需要按启发式解码而不是假定 UTF-8。

**Blocked by:** 01 — 骨架：单页贯通与合成夹具

**Status:** resolved

- [x] CBZ 与目录走同一个源抽象，调用方无需区分
- [x] 成员名非 UTF-8 时按启发式解码，中文名不出现乱码
- [x] 包内目录前缀被正确处理，输出不带多余层级
- [x] 非图片文件原样透传，内容逐字节一致
- [x] 输入是 CBZ 时输出也是 CBZ
- [x] 损坏或无法读取的归档给出可操作的错误，不产出半成品

## Comments

源在 `src/source.rs`，输出容器在 `src/sink.rs`；容器行为的用例在 `tests/container.rs`，
CBZ 夹具在 `tests/fixtures/cbz.rs`。`CONTEXT.md` 因此多了「输出容器」与「成员」两个词条。

本票内定下的四件事，各自的理由写在实现处，这里只记结论与出处：

- **成员名按「合法 UTF-8 → GBK → cp437」的顺序解**，不看 UTF-8 标志位。见 `source::decode_name`。
- **包内目录前缀的判据是「没有兄弟」，且一剥到底**，与目录名叫什么无关；目录卷不剥。
  见 `source::strip_wrapper_directory`。
- **归档输出先写临时文件，收尾时才改名到位**，中途失败因此既不留半个归档、也不毁掉上一次的成品。
  见 `sink::ArchiveSink`。
- **成员名里的 `..`、绝对路径与盘符一律拒绝**，不就地修正——修正会静默改变成员的去处。
  见 `source::relative_path`。

`PageReport` 的 `source` 与 `output` 对归档卷是「卷路径接上成员相对路径」，不是打得开的路径；
这条约定写在 `report::PageReport` 上，由 `an_archive_page_is_reported_as_the_volume_path_plus_its_member_name` 钉住。

认下的两处限制：

- **非 UTF-8 的名字一律当 GBK，这是假定不是判别。** Shift-JIS 的名字多半也能被 GBK 解出来，
  解成汉字乱码且没有下一档去兜。日文片源要正确，得先有编码判别——那是另一件事。
- **`.zip`、CBR 与 PDF 输入都不认**，`source::open` 的错误信息直说卷是目录或 CBZ。

推迟的：单页失败仍会中止整卷——错误隔离是 12 号票。
`an_archive_that_fails_partway_leaves_no_half_written_output` 与
`a_run_that_fails_leaves_the_previous_output_archive_intact` 用坏图当触发方式，12 号票要换掉它，
两个用例真正钉的是「归档要么完整、要么不存在」。
透传文件不进 `Report`，那份形状随后面的票再定。

夹具按字节手搓 ZIP，不用 `zip` 的写入端；为什么见 `Cargo.toml` 里 `crc32fast` 那条注释。
