# 02 — bin 的文档检查进 `polish`，指不到的路径改对，写法进规矩

**What to build:** `cargo rustdoc --bin tonefit`（带私有项）收进 `polish`，告警条数与 `cargo doc` 那一条同一副做法：印出来、记进票据的《数》、只降不升，
不由工具写死阈值。落地这一张先把会话里两类指不到的文档路径改对——少写了 `terminal::` 那一族、`super::` 数少了一级那一族——
**用一条 `git grep` 数现存的**，不抄停车场里的数。基线从改完之后那个数起。

写法规矩进 `docs/agents/`（与闸门那一篇同处）：跨模块指路写 `crate::` 全路径、不数 `super::`；跨可见性写代码跨、不写方括号链接
（方括号链接按可见性解析，指向私有项的那一条本身就是一句假话）。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `cargo xtask polish` 多出 bin 的文档检查一步，印出告警条数；`gate.md` 那张表跟着改
- [ ] 那两类指不到的路径改对；本票记下改之前与之后的条数，以及数它的那条命令
- [ ] 写法规矩写进 `docs/agents/`
- [ ] 票据的《数》记下 bin 与 lib 两条文档告警数
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q511 — bin crate 的文档链接一条都没人验，一跑就是 61 条告警

- **From:** 票 `no-false-line/08`
- **Kind:** 工具缺口（这一整类假话能活到今天的原因）
- **Where:** `cargo doc` 跳过 `tonefit` 那个 bin（与 lib 同名，票面第一句就写着「一条告警都不会报」）。
  `src/session/` 整棵与 `src/main.rs` 都在那个 bin 里，**三条闸门与 `cargo xtask polish`
  一条都够不着它的 intra-doc link**。
- **Why it did not block:** 三条闸门与 polish 全绿，而绿**说明不了**那些链接指不指得到——
  本票收的三处假话正是这么活下来的。
- **What this ticket actually did:** 不改闸门口径，但跑了一趟**证明**：
  `cargo rustdoc --bin tonefit -- --document-private-items`（走默认 `target`，热的时候几秒）。
  落地前 **82 条**告警，落地后 **61 条**，差的 21 条正是本票改掉的那 21 处 `///` 注释
  （另 19 处在 `//` 普通注释或 `#[cfg(test)] mod tests` 里，rustdoc 本来就不检），
  **一条新增都没有**（两趟 warning 行逐条 diff 过）。剩下的 61 条：40 条 unresolved link、
  19 条「既是函数又是模块」的歧义链接、2 条「既是函数又是宏」。
- **Options:** ① 记下，靠人偶尔手跑；② 把 `cargo rustdoc --bin tonefit` 收进
  `cargo xtask polish`，把 **61** 记成基线、只准降不准升；③ 给那个 bin 改名，
  让 `cargo doc` 自然收得到它。
- **Recommend:** ②。它与 `cargo doc` 那 15 条基线是**同一副做法**，成本是一条命令
  （本机实测几秒，target 是热的），而它一次盯住 Q508、Q509 与本票收的那三处**一整类**。
  ③ 动的是 crate 布局，代价大且会牵到一大批 `crate::` 路径。
- **Whose call:** 拍板的人（改的是 polish 的口径，外加一条新基线）
- **处置：** 待处理。

#### Q508 — 会话那几个模块里另有十三处 `super::<终端那一层的函数>` 同样指不到

- **From:** 票 `no-false-line/08`
- **Kind:** 票面点名的两个名字之外，同一条毛病的其余副本
- **Where:** 共 **13** 处，数它的那条：
  `git grep -nE "(super::)+(resuming|drive|translate|store_preset|erase_preset)" -- src/session/`
  —— `src/session/live.rs:42`／`:46`；`src/session/run.rs:81`／`:1005`（`super::resuming`）、
  `:135`／`:423`（`super::drive`）、`:492`／`:547`（`super::super::drive`，在 `mod tests` 里）；
  `src/session/state.rs:52`（`super::translate`）、`:1161`／`:5273`（`super::erase_preset`）、
  `:1226`（`super::store_preset`）、`:1786`（`super::drive`）。
  按写法数：`super::resuming` 4、`super::drive` 3、`super::super::drive` 2、
  `super::erase_preset` 2、`super::translate` 1、`super::store_preset` 1。
- **Why it did not block:** 与票面第 2 条**同一条毛病、同一条修法**：那五个函数与
  `press`／`expand` 一样住在 `session::terminal` 里，从会话那一层看 `super::X` 就已经落空。
  票面第 2 条把范围写死在 `super::press`／`super::expand` 这两个**名字**上，
  而本票另写死「代码一行不动、三处一起订正」——扩出去就不是这一票了。
  bin crate 与 lib 同名、不进 `cargo doc`，一条告警都不会报，屏上也看不见。
- **What this ticket actually did:** 只改了票面点名的那两个名字（40 处，一律换成
  `crate::session::terminal::press`／`…::expand`）。这 13 处一字未动。
  **本条头一版把这个数写成 9**——漏了 `run.rs` 与 `state.rs` 用例模块里那 4 处，
  是 `/code-review` 的 Standards 轴查出来的。数改成 13，并在上面记下数它的那条命令：
  这个停车场自己也归「屏上没有一句假话」管，一个估出来的数不该留在里面（同 Q510）。
- **Options:** ① 记下；② 顺着本票同一副写法，9 处一次改完；
  ③ 让它自己红：给 bin crate 开一条进 `cargo doc` 的口子，把 `broken_intra_doc_links`
  变成闸门上的一条。
- **Recommend:** ② 先，③ 是根治。② 是一次 sed，风险与本票逐字相同；
  ③ 能把这一整类（连同 Q509）一次变成红的，但要动 crate 布局或闸门口径，够开一张自己的票。
- **Whose call:** 落地的人（②）；③ 是拍板的人（改的是闸门口径）
- **处置：** 待处理。

#### Q509 — 画法那一层的用例模块里，六处 `super::super::…` 少数了一级

- **From:** 票 `no-false-line/08`
- **Kind:** 同一类「指不到」的第三种形状
- **Where:** `src/session/draw/config.rs:279`、`draw/footer.rs:1226`、
  `draw/overlay.rs:271`／`:413`、`draw/report.rs:1759`（写的是 `super::super::state`），
  以及 `draw/report.rs:1021`（`super::super::terminal`）——六处都在各自文件的 `mod tests` 里。
  **与 Q508 的分界**：这一条是 `super::` **数少了一级**（路径形状错），Q508 那一条是
  级数对了、**少了 `terminal::` 那一段**。`run.rs:492`／`:547` 的 `super::super::drive`
  级数是对的，因此归 Q508 不归这里。
- **Why it did not block:** 这六处**在模块层面本来是对的**，搬进 `mod tests` 之后少了一级：
  从 `session::draw::<那一块>::tests` 数起，`super::super` 只到 `session::draw`，
  要三级才够得着 `session::state`。屏上看不见，`cargo doc` 也不报（bin crate 不进）。
  票面第 2 条按**名字**划范围，这一类不在里面。
- **What this ticket actually did:** 没动。本票改的 40 处里有 5 处正落在这一类里
  （`state.rs` 用例两处、`run.rs` 用例两处、`draw/report.rs` 用例一处），
  改法是换成 `crate::session::…` 全路径——**同一副写法对这六处照样成立**。
- **Options:** ① 记下；② 六处一律换成 `crate::session::state::…`／`crate::session::terminal::…`；
  ③ 连同 Q508 一起，把「跨模块指路一律写 `crate::` 全路径，不数 `super::`」写进 `docs/agents/`。
- **Recommend:** ②，③ 是它的规矩那一半。数 `super::` 的写法一搬家就错，
  而本票收的三条停车场里有两条正是这么坏的。
- **Whose call:** 落地的人（②）；③ 要不要立成规矩是拍板的人
- **处置：** 待处理。

#### Q512 — 「指得到」在私有项上做不到：票面第 2 条与第 4 条互相顶着

- **From:** 票 `no-false-line/08`
- **Kind:** 票面两条验收互相顶着（不是哪一处代码的毛病）
- **Where:** 票面第 2 条「改成**指得到的写法**」与第 4 条「**代码一行不动**」；
  对象是 `src/session/terminal.rs:132` 的 `fn press` 与 `:412` 的 `fn expand`
- **Why it did not block:** 那两个函数**私有于 `terminal`**。rustdoc 的 intra-doc link
  按编译器那套**可见性**解析：从 `session::state` 看不见 `terminal` 里的私有项，
  因此 ``[`crate::session::terminal::press`]`` 这样的**方括号链接**照旧 unresolved——
  路径对了，可见性不对。要让链接真的成立只有一条路：把那两个函数抬成
  `pub(super)`／`pub(crate)`，而那是**改代码**，第 4 条明令禁止。
  同一条也解释了 `terminal::TICK`、`terminal::no_terminal_error`、`super::draw::table`、
  `super::yielding::footer_height` 那一批为什么一直 unresolved：目标都是私有项。
  对照组 `super::run::Running::stop` 解析得到——`Running` 与 `stop` 都是 `pub`。
- **What this ticket actually did:** 取「指得到」的另一层意思——**指路，不是超链接**：
  40 处一律写成代码跨 `` `crate::session::terminal::press` ``，不加方括号。
  路径逐段查得到（`main.rs:22` 的 `mod session;` → `session.rs:69` 的 `mod terminal;`
  → `terminal.rs:132`／`:412`），而一条解析不了的方括号链接**本身就是一句假话**。
  实测：改成代码跨之后，bin 那一趟的 unresolved 从 21 条降到 **0**（见 Q511）。
- **Options:** ① 照本票的做法——跨可见性一律写代码跨、不写方括号链接，并把这条写进
  `docs/agents/`；② 把 `press`／`expand`（连同 Q508 那五个同类）抬成 `pub(super)`，
  链接就真的成立；③ 什么都不做，留着解析不了的链接。
- **Recommend:** ①。② 为了一个链接放宽可见性，是让文档反过来定代码的形状；
  `terminal` 那几个函数私有是**有意的**（它们是那条循环的内部分工，
  `src/session.rs`《终端库在哪一半》说的就是这件事）。③ 与本效力的名字对着干。
- **Whose call:** 落地的人（①）；② 要放宽可见性是拍板的人
- **处置：** 待处理。
