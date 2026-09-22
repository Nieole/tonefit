# 12 — `is_junk` 改名

**What to build:** `is_junk` 判的是卷内成员两件事：躺在不看的地方里的，与打包环境留下的那几个文件。名字只说得出后一半。
改成说得出两半的名字（与目录那一侧的「不看的地方」同一个词），三处文档、五个调用点与那条跟着它取名的用例一起改。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 成员那一侧的谓词名字里有「不看」这个词，与目录那一侧对上
- [ ] 三处文档、调用点、那条用例名一起改；旧名在仓库里一处不剩
- [ ] 全部用例照旧绿，黄金快照一个字节不动；设计快照照旧绿
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q477 — 名单与目录那一侧的谓词改口了，成员那一侧的 `is_junk` 名字没跟着改

- **From:** 票 `no-false-line/07`
- **Kind:** 落地时认下的（一次改名只做了一半）
- **Where:** `src/source.rs` 的 `is_junk`，三处指着它的文档（`solid_members`、
  `strip_wrapper_directory`、`src/survey.rs` 的 `nothing_took_it`），
  以及跟着它取名的那条用例 `tests/container.rs` 的
  `a_directory_volume_ignores_the_same_system_junk`（连同它那句「同一批垃圾」）
- **Why it did not block:** 名单与**目录**那一侧已按词汇表改成 `IGNORED_DIRECTORIES`／
  `is_ignored_directory`（词条《不看的地方 (IgnoredPlace)》）。`is_junk` 判的是**卷内成员**，
  它今天收两样：躺在不看的地方里的，与打包环境留下的那几个文件（`JUNK_FILES`、AppleDouble
  边车）。名字只说得出后一半。`CLAUDE.md` 那条「类型名一律取自 `CONTEXT.md`」管的是
  类型名、模块名、测试名与 issue 标题，私有函数名不在其中，因此不算破规矩。
- **What this ticket actually did:** 文档改口了（第一句从「这个成员是不是打包环境留下的垃圾」
  改成「这个成员是不是不看的东西：躺在不看的地方里，或者本身就是打包环境留下的那几个文件」），
  名字一个字没动——票面写的是名单与判据，没写改名，而三处引用今天各自那句话都还成立。
- **Options:** ① 原样留着，靠第一句说清两半；② 改成 `is_ignored_member`，连同三处文档
  与五个调用点一起；③ 拆成两个谓词（不看的地方一条、打包环境留下的文件一条），
  调用点各按各的意思拼。
- **Recommend:** ②。一次改名，diff 落在三处文档、五个调用点与一条用例名上，
  而它把「不看」这个词从目录那一侧贯到成员那一侧——今天读代码的人会以为卷内那条判据
  只管打包垃圾，于是往名单里加系统目录时不会想到它在**两处**同时作数
  （Q478 说的正是这一半）。本票的 `/code-review` 两轴都独立指到了这一处
  （Standards 轴判为 Mysterious Name 并直接点名方案 ②）。
  ③ 不选：两条判据在**每一个**调用点都同时要，拆开只是把一个 `||` 从函数里搬到五处去。
- **Whose call:** 落地的人
- **处置：** 待处理。
