# 02 — 默认路径逐页到底

**What to build:** 默认路径上，**开卷之前答得出**的那一种顶死（两维都点了，两组门的候选集裁到同一个）照旧是覆盖顶死、分析环节就把字节编完；
**答不出的那一角一律逐页判**，不再等整卷判完门再问——那一角的理由从「覆盖」变成逐页那两种、卷级那一行从「覆盖」变成「逐页」；
开滚动窗口、参照不进缓存、写页级依据、按页跳过成立。`--envelope` 那条路不变。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 集成用例：一卷两组门混着、只点了一维覆盖项——报告说「逐页」、每页理由是逐页那两种、缓存里没有参照、每页写了页级依据
- [ ] 同一卷改一页之后第二趟只重做那一页（按页跳过）
- [ ] 两维都点了的那一趟照旧是覆盖顶死；`--envelope` 那条路一格不变
- [ ] 第一遍那份档与汇总那份逐格相同那条断言照旧钉得住
- [ ] `CONTEXT.md`《覆盖顶死》加一句「默认路径上只有两维都点了的那一种」；ADR 0005《覆盖项那一档答不出来时让位》改写；`VolumeVerdict::Override` 的文档跟着改
- [ ] 黄金快照原样过（黄金里没有这一角）
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q424 — 覆盖项裁到只剩一个候选那一档，两组候选集一长一短时碰卷之前答不出来

- **From:** 票 `two-pass-rework/12`
- **Kind:** 落地时走的那条路（滚动窗口在一种参数组合上让位）
- **Where:** `src/lib.rs` 的 `pinned` 与新加的 `pinned_up_front`；`Window::open`
- **Why it did not block:** `pinned` 拿的是**其余页那一组**的候选集，而哪一组是其余页
  要等整卷判完几何门才知道（一页门成立的都没有时，门不成立的那些就当其余页）。
  滚动窗口要在第一遍里定档，因此要在碰卷之前答出这一问。两组答案相同时与分组无关，
  当场答得出；`--bit-depth 2` 这种一组剩两个（`2bit`/`2bit+FS`）、另一组剩一个（`2bit`）的
  参数上答不出，那一卷于是**不开窗口**，照旧攒整卷参照——与本票落地之前逐字节相同。
- **What this ticket actually did:** `pinned_up_front` 答不出就回 `None`，`Window::open` 跟着不开。
  写进 ADR 0005 新加的那一节《覆盖项那一档答不出来时让位》。
- **Options:** ① 照现在（那一种参数上让位）；② 把 `pinned` 改成只问 `Request` 与两套候选集，
  不再从「其余页那一组」的判据曲线上读——那样它与分组无关，碰卷之前恒答得出；
  ③ 窗口先按「不顶掉」走，撞见第一张门成立的灰度页再收口，一张都没有时整卷回退。
- **Recommend:** ②，但那是**领域决定**：`pinned` 今天那句注释明写「拿门不成立的页去问，
  答案会随卷里第一张灰度页碰巧是哪一种而变」，改它等于改「覆盖项顶掉判定」这句话的定义。
  ③做得到但要在窗口里留一条「回退成攒整卷」的路，为一种参数组合多养一条分支不划算。
- **Whose call:** 拍板的人
- **处置：** 待处理。

#### Q635 — 默认路径上「一组顶死、另一组没有」那一角仍攒整卷参照，等的只是理由那一个词

- **From:** 票 `two-pass-rework/11`
- **Kind:** 「默认路径上参照不再进缓存」的一处例外
- **Where:** `src/lib.rs` 的 `Settles::for_this_run`（`pinned_up_front` 答 `None` 那一支）与 `summarize_volume`
  里 `pinned(request, scores(first))`；ADR 0005《覆盖项那一档答不出来时让位》
- **Why it did not block:** 那一角的**字节**两条取法都一样：`--bit-depth 2` 而门不成立那一组只剩 `2bit` 时，
  每一页的候选无论怎么判都是 `2bit`；不同的只有 tEXt 里的理由（`override` 还是 `lowest candidate within threshold`）
  与卷级那一行（`覆盖 2bit` 还是 `逐页`），而理由取决于「哪一组是其余页」——要等整卷判完门才知道。
  06 号票为顶死那一趟定过同一条界，本票照搬，`debug_assert!`（第一遍那份档与汇总那份逐格相同）因此仍钉得住。
- **What this ticket actually did:** 默认路径在这一角退回 `Settles::AfterTheVolume`——照旧攒整卷参照、第二遍编。
  spec P-B 那句「默认路径上参照不再进缓存」在这一角不成立；只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **Options:** ① 保持（理由一处出处，那一角少见）；② 默认路径上 `summarize_volume` 不再问 `pinned`，
  一律 `decide(…, None)`——那一角的理由从 `override` 变成逐页那两种、卷级那一行从「覆盖」变成「逐页」，
  参照于是永不进缓存；代价是「覆盖顶死」这个词在默认路径上只剩两维都点了的那一种，`VolumeVerdict::Override`
  的文档要跟着改一句；③ 第一遍按 ② 编、汇总照旧——**不可取**，那正是 `debug_assert!` 挡的分家。
- **Recommend:** ②，但不在本票做：它改的是《覆盖顶死》词条的射程（`CONTEXT.md`「一个覆盖项都没点而只剩一个时不算」
  那一条旁边要再加一句），按《改 CONTEXT.md 的规矩》先拍板。做起来是 `summarize_volume` 里两行加一条用例。
- **Whose call:** 拍板的人
- **处置：** 待处理。

#### Q683 — Q635 那一角（默认路径、一维覆盖、两组门混着）没有页级依据，按页跳过在那一角不成立：改一页整卷重做

- **From:** 票 `two-pass-rework/14`
- **Kind:** 票面「改一页只重做那一页」的射程差——Q635 的直接后件，不是收 Q635
- **Where:** `src/lib.rs` 的 `Settles::if_processing`（`pinned_up_front` 答 `None` → `AfterTheVolume`）与 `process_volume`
  里 `by_page` 的谓词；13 号票在同一谓词上不写页级依据（Q665）
- **Why it did not block:** 谓词一处出处：写不写页级依据与按不按页跳过问的是同一句
  `Settles::if_processing(..).encodes_in_the_first_pass()`——那一角像素只取决于页，tEXt 里的理由那一句
  （`override` 还是 `lowest candidate within threshold`）却由全卷定，留一页就是留一句可能过期的理由，
  13 号票判不写、本票顺着判不留。那一角只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **What this ticket actually did:** 那一角走 `Prior::Nothing`／`Prior::Whole` 两种，与 `--envelope` 同一个待遇：
  整卷跳或整卷重做。没有用例专门钉它——钉它要一卷两组门混着加一维覆盖，与 Q635 的处置绑在一起。
- **Options:** ① 现状，等 Q635 按它的推荐 ②（默认路径不再问 `pinned`）落地，那一角自然消失、页级依据自然成立；
  ② 本票把那一角的谓词单独放开（像素确实只取决于页）——留下的页理由那一句可能与整卷重做写出的不同，
  产物不再逐字节相同，破 Q681 那条性质。
- **Recommend:** ①。
- **Whose call:** 拍板的人（随 Q635）
- **处置：** 待处理。

#### Q697 — 依据的作用域由 `Settles` 定，不由 `--envelope` 定：默认路径上 Q635 那一角仍算、仍记卷级依据

- **From:** 票 `two-pass-rework/15`
- **Kind:** 票面「默认路径不再算、不再记卷级依据」的一处例外——Q635／Q683 那一角的直接后件
- **Where:** `src/lib.rs` 的 `Settles::if_processing`（`pinned_up_front` 答 `None` → `AfterTheVolume`）与 `process_volume`
  里 `by_page = walks.encodes_in_the_first_pass()`；`src/metadata.rs` 的 `SourceHash`；`CONTEXT.md`《源哈希》
- **Why it did not block:** 谓词一处出处：13 号票写不写页级依据、14 号票按不按页跳、本票按哪种作用域算记比，
  问的都是同一句 `Settles::if_processing(..).encodes_in_the_first_pass()`——「依据的作用域 = 这一页的字节取决于什么」。
  那一角像素只取决于页，tEXt 里的理由那一句（`override` 还是 `lowest candidate within threshold`）却由全卷定，
  留一页就是留一句可能过期的理由（Q683 说过），页级依据在那一角不成立；不给它卷级依据它就永远不跳
  （今天它整卷跳），给它页级依据就破「留下的页与整卷重做逐字节相同」。两条路的依据仍不重叠、不互相兜底：
  一份记录只带一种，谓词一处。那一角只在点了恰好一维覆盖项、且卷里门那两组混着时撞得上。
- **What this ticket actually did:** 那一角走卷级那条路（`SourceHash::Volume`）：算、记 `tonefit:source`，整卷跳或整卷重做，
  与 `--envelope` 同一个待遇。票面那一句在这一角不成立，ADR 0006 改写那一段与《源哈希》词条都写明了「问的是字节取决于什么，
  不是开关的名字」。
- **Options:** ① 现状——等 Q635 按它的推荐 ②（默认路径不再问 `pinned`）落地，那一角自然消失，票面那一句随之全部成立；
  ② 谓词改成 `request.envelope`，那一角哪一种依据都不写、永远不跳——多一种「没有依据」的记录形态，且是一处静默回退；
  ③ 那一角放开页级依据（Q683 的 ②）——破逐字节相同。
- **Recommend:** ①。
- **Whose call:** 拍板的人（随 Q635）
- **处置：** 待处理。
