# 10 — 卷名只从清单来

**What to build:** 命令行进度条在开工那一条事件到时**收下清点清单上的「卷根 → 卷名」**，开一卷时按卷根查卷名——分卷序列那一卷叫 `第01卷`。之后「从卷根推名字」那个函数只剩页名一个读者，**改成页名那一头的名字**。场景与终端用例里卷名的期望值改成按卷根问清单。碰场景夹具（本轮的冲突热点）：与 11 不并行。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 进度条上分卷序列那一卷叫序列的名字，用例钉住（进度条那一侧的用例或起真进程比 stderr）
- [ ] 那个函数改名、读者只剩页名；断言 `第01卷.part1` 的那条用例改成说它管页名
- [ ] 场景与终端用例的卷名期望值读清单，不再拿那个函数推
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 不做会怎样

同一卷在命令行进度条与会话上两个名字（Q849 当初要避开的那一种）。

## 停车场结转

下面几条由停车场转来（`/settle` Q995–Q1388 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q1297 — 命令行进度条上分卷序列那一卷仍叫 `第01卷.part1`，会话那一侧已经叫 `第01卷`

- **From:** 票 `design-parity/12`
- **Kind:** 票面约束落地之后的后果（票面：「命令行进度条本票不动」）
- **Where:** `src/main.rs` 的 `Bar::start`（`render::volume_name(volume)`）；`src/render.rs` 的 `volume_name`
- **Why it did not block:** 不沾停线三条：命令行报告的字节不变，黄金快照原样过；进度条只是这一刻的那一行字。
- **What this ticket actually did:** 会话写卷名改读清点清单上的卷名（`SurveyedVolume::name`），命令行进度条照旧从开卷那一条报的卷根取（`render::volume_name`），分卷序列那一卷因此在两副界面上叫两个名字。`render::volume_name` 的文档、`Bar::start` 旁那句注释与 `tests/discovery.rs` 里那段说明改成如实说这一点，那条 `render` 用例的断言（`第01卷.part1`）留着、消息指到这一条。
- **Options:** ① 现状；② 进度条在 `RunStarted` 那一刻把清单上的「卷根 → 卷名」收下，`start` 按卷根查卷名（`Bar` 多一张表，`render::volume_name` 只剩页名一个读者）。
- **Recommend:** ②，另开一张小票。库已经把卷名交出来了，命令行那一路读它只差一张表；留着 ① 等于让「同一个卷在同一个程序的两副界面上两个名字」（Q849 当初要避开的那一种）在分卷序列上成真。
- **Whose call:** 协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：命令行进度条在开工那一刻收下清单上的「卷根 → 卷名」，按卷根查卷名。

#### Q1300 — `render::volume_name` 不再管会话里的卷名，名字没改

- **From:** 票 `design-parity/12`
- **Kind:** 命名
- **Where:** `src/render.rs` 的 `volume_name`；读者 `src/main.rs`（进度条）、`src/session/shell/pages.rs`（页名）、`src/session/shell/list.rs`（代表页）、`src/session/scene.rs` 与 `src/session/terminal.rs` 的用例
- **Why it did not block:** 不沾停线三条：改名是机械的，翻过来只动本票碰过的那几处与两处用例。
- **What this ticket actually did:** 名字留着，文档改成「一条路径在屏上叫什么」，写明会话里一卷叫什么不走它、读清单。
- **Options:** ① 现状；② 改名（如 `shown_name`／`last_segment`），把「这是卷名」的暗示拿掉。
- **Recommend:** ①，若 Q1297 走 ②（进度条也读清单）再改：那时它只剩页名一个读者，改成页名那一头的名字顺理成章；此刻改名要动 `scene.rs`（本轮的冲突热点）里的几处用例。
- **Whose call:** 协调人
- **处置：** 并入 Q1297，随它走（拷问定案 2026-10-02：进度条也读清单之后，`render::volume_name` 只剩页名一个读者，改成页名那一头的名字）。

#### Q1302 — 场景与终端那几条用例仍拿 `render::volume_name(卷根)` 推屏上卷名的期望值

- **From:** 票 `design-parity/12`
- **Kind:** 审查撞出来的（`/code-review` Standards 轴）
- **Where:** `src/session/scene.rs` 的 `on_the_grid` 里按卷名认行的那几处（`render::volume_name(&report.volume)`、`&failure.volume`、`&summarized.volume`）；`src/session/terminal.rs` 的 `v_while_deciding_opens_the_pages_of_that_volume_and_h_comes_back`
- **Why it did not block:** 不沾停线三条：场景数据里没有分卷序列，两种取法在每一卷上答的都是同一个名字；哪天设计稿添了一卷分卷序列，这几条是当场红，不是静默过。
- **What this ticket actually did:** 一处没动。产品代码里一卷叫什么已经一律读清单（本票那条屏上用例钉着），这几条用例只是拿 `render::volume_name` 认行、认卷。`scene.rs` 是本轮的冲突热点，`terminal.rs` 的用例此刻另一槽（`design-parity/05`）正在改。
- **Options:** ① 现状；② 期望值改读清单上的卷名（`on_the_grid` 添一个按卷根问 `live.roster()` 的小函数，`terminal.rs` 那一条比卷根或读 `Tree::name`）。
- **Recommend:** ②，等这两个文件没人在改的时候顺手做：期望值与产品代码同一个出处，用例才说得出「屏上写的是清单上那个名字」。
- **Whose call:** 协调人
- **处置：** 拷问定案（2026-10-02，`/grill-with-docs`）→ `honest-screen`，待 `/to-spec`／`/to-tickets`：场景与终端用例的期望值改读清单上的卷名。
