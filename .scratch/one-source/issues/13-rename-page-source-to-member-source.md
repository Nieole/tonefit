# 13 — `PageSource` 改名 `MemberSource`

**What to build:** 透传文件那一份哈希借用了 `PageSource` 这个名字，而它不是页。单独一次全仓改名：类型改叫 `MemberSource`，
《页级源哈希》那一条改成《成员源哈希 (MemberSource)》（一页或一个透传文件，一个成员一份；页上写的仍是这一份）；
tEXt 键不动，输出一个字节不变。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 类型与它的文档改名；旧名在仓库里一处不剩
- [ ] `CONTEXT.md` 那一条改成《成员源哈希 (MemberSource)》，引到它的几处跟着改
- [ ] tEXt 键一个字不变；幂等用例照旧绿
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q699 — 透传文件在页级那条路上的那一份哈希借用了 `PageSource` 这个类型：「页级源哈希」词条本义是「这一张来自的那个源成员」

- **From:** 票 `two-pass-rework/15`
- **Kind:** 命名／类型借用——code-review（Standards 轴）指出的 Mysterious Name
- **Where:** `src/metadata.rs` 的 `PageSources::extras: Vec<Option<PageSource>>` 与 `PageSource` 的类型文档（「『页级』说的是作用域——一个成员一份」那一句）；
  `src/lib.rs` 的 `nothing_else_changed`（输出里那一份读回来按 `PageSource::of` 重算再比）；`CONTEXT.md`《页级源哈希》末句
- **Why it did not block:** 算法与写法确实是同一个（`SourceHasher` 只喂这一个成员），透传文件那一份只拿去比、不进任何 tEXt，
  用户看不见它叫什么；给它另起一个类型就是同一段字节两个名字。词汇表《成员》本来就是「一页，或一个透传文件」，
  「一个成员一份」说得通，只是类型名里的 `Page` 与它对不上。
- **What this ticket actually did:** 借用 `PageSource`，在类型文档与《页级源哈希》词条各补一句「透传文件也各算一份，只拿去比、不进记录」。
- **Options:** ① 现状；② 类型改名 `MemberSource`、词条改成《成员源哈希 (MemberSource)》——tEXt 键 `tonefit:page-source` 不动
  （页上写的仍是页级那一份）；改的是一个已有词条的绑定与说法，按《改 CONTEXT.md 的规矩》先拍板，做起来是一次全仓改名；
  ③ 透传文件那一份换成 `SourceHasher` 直接收口的 `VolumeSource`（同一条规矩、另一个类型）——把「卷级」这个词借给透传文件，比 ① 更歪。
- **Recommend:** ②，单独派——一次改名，不该夹在这一票的语义改动里。
- **Whose call:** 拍板的人
- **处置：** 待处理。
