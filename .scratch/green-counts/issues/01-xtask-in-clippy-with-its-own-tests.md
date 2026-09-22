# 01 — `xtask` 进 clippy、有自己的用例，逐条印结果，《数》带平台

**What to build:** 闸门照旧三条，多出来的检查进 `polish`：`clippy -p xtask` 与 `xtask` 自己的用例。`xtask` 里值得用例的是三个纯函数——
数 `test result:` 那一行（含带颜色转义的写法）、逐条那一行的措辞、平台那一行。
跑满三条闸门时**每跑完一条就当场印那一条的通过与失败数**，末尾照旧印整张《数》；《数》的表头印平台（操作系统与架构），
`gate.md`《读结果》写一句「数只与同一平台的上一趟比」。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `cargo xtask polish` 多出 `clippy -p xtask` 与 `xtask` 自己的用例两步，都过
- [ ] 三个纯函数各有用例；数那一行的用例覆盖带颜色转义的那一种
- [ ] 跑满三条时每跑完一条印一行；中途掐掉之后，已跑完的那几条的数照样留在屏上
- [ ] 《数》表头印平台
- [ ] `docs/agents/gate.md` 的 `polish` 那张表与《读结果》跟着改
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q208 — `xtask` 自己不在任何一条闸门、也不在两遍 clippy 的照射范围里

- **From:** 票 `p4-parking-lot/24`
- **Kind:** 我确实拿不准的单项
- **Where:** `Cargo.toml` 的 `default-members = ["."]`；`xtask/`
- **Why it did not block:** `default-members` 把它挡在 `cargo build`／`cargo test` 之外，
  那正是票面「新增的 workspace 成员不许进产物依赖」要的。代价是两处照不到它：
  `cargo clippy --all-targets` 走的也是 default members，扫不到 `xtask`；
  而它自己那个纯函数（`counts`，把 `test result:` 那一行数成两个数）值得一条用例，
  可任何一条闸门都跑不到它。编不过那一半倒是当场知道——`cargo xtask` 每次都先编它。
  排版那一条盖得住：`cargo fmt` 是全 workspace 的。
- **What this ticket actually did:** **两样都没加**：闸门仍是三条（票面写死「不多不少」），
  用例也没写——加一条跑不到的用例比不加更坏。`counts` 因此只有「跑一遍闸门、看那张《数》
  对不对得上」这一层验证，本票落地时对得上（796 / 670，与老三条各跑一遍逐格相同）。
  clippy 那一半本票**手动跑过一遍**（`cargo clippy -p xtask --all-targets`，零告警），
  而那一条不在 polish 里——下一个人不会自动跑到它。排版那一条当场验过盖得住：
  往 `xtask/src/main.rs` 里塞一行歪的，`cargo fmt --check` 当场回 `1`。
- **Whose call:** 拍板的人（要不要让 polish 那一条把 `xtask` 也扫进去，或者给它自己一条用例）
- **处置：** 待处理。

#### Q284 — `cargo xtask gate` 跑满三条时中途不印任何一条的结果，掐不准也切不动并发度

- **From:** 票 `p4-parking-lot/13`
- **Kind:** 落地途中撞见的一处工具缺口（不是本票改出来的）
- **Where:** `xtask/`（那张《数》的印法）、`docs/agents/gate.md` 的《一条命令跑满三条》
- **Why it did not block:** `cargo xtask gate` 印那张《数》是在**被点名的那几条全跑完之后**，
  中途一行「闸门 1 绿了 / 红了」都没有。两条线并排跑时这一点是要紧的：
  调度那一头临时要求「下一条起限到 6 个 job」，而唯一的办法是**等闸门 1 出结果再掐掉、
  换成 `gate 2 3`**——那一行永远不来，等了二十分钟才看明白闸门 2 早就在跑了。
  此刻掐的代价是**连闸门 1 的数一起丢**（那张表还没印，重跑等于闸门 1 全额再付）。
  不阻塞是因为**另一条路一样走得通**：分三条跑（`gate 1` → `gate 2` → `gate 3`），
  每条各印一张表，中间既切得动并发度、也随时掐得掉；`gate.md` 本来就写着
  「只跑其中几条、按点名的次序：`cargo xtask gate 2 1`」。
- **What this ticket actually did:** **让那一趟跑完，没有掐。**树在那一趟里一个字节没动，
  数因此作数。下一趟改成分条跑，办法写进了本票的《数》。
- **Whose call:** 拍板的人（`xtask` 要不要每跑完一条就印那一条的结果——
  那样跑满三条的输出会长一点，换来的是中途看得见、掐得准；
  `gate.md` 要不要把「多条线并排跑时按条跑」写成一句建议）
- **处置：** 待处理。

#### Q562 — 《数》跨平台不可逐格相比：同一份代码 Linux 916、Windows 915

- **From:** 票 `p4-parking-lot/25`
- **Kind:** 路过发现（本票要拿《数》证明「一格不变」，一比就撞上）
- **Where:** `docs/agents/gate.md` 的《读结果》与各票据的《数》；`tests/stop.rs` 的
  `#![cfg(unix)]`；`src/session/complete.rs` 里那条 `#[cfg(windows)]` 用例
- **Why it did not block:** 本票基底 `e1ceb2b` 与上一张落地票 `tone-alignment/05` 的
  `ee61c23` 之间 `src/`、`tests/`、`xtask/`、`Cargo.toml` **一个字节都不差**
  （`git diff --stat` 空），而两处的《数》对不上：ta/05 记的是 **915（lib 243 / bin 361）**、
  闸门 2 **784**，本票在这台 Linux 上量到 **916（243 / 360）**、闸门 2 **785**。
  差额逐条对得上，**全是平台条件编译**：`tests/stop.rs` 整份挂着 `#![cfg(unix)]`（两条，
  Windows 上不跑），而 `session::complete` 那条
  `the_separator_the_user_typed_is_the_one_that_comes_back` 挂着 `#[cfg(windows)]`
  （一条，落在 bin 里）——916 − 2 + 1 = 915，逐格闭合。不阻塞是因为它不是红，
  也不是谁改错了：两个数各自都对。
- **What this ticket actually did:** **自己跑一趟取数，不与别的票的数逐格比**
  （派活说明本来就写着「别抄别的票的」）。本票代码一行未动，「一格不变」因此由
  「`git diff` 里 `src/`、`tests/` 一个字节都没有」证，而不是由「与上一张票的数相等」证。
- **Options:** ① 记下这条读法（数只与**同一平台**的上一趟比）；
  ② 让 `cargo xtask gate` 把平台印进《数》那一张（一行的事，数从此自带口径）；
  ③ 把那三条平台相关的用例改成两平台都跑得到（`stop.rs` 那两条要一套跨平台的送信号法，
  贵得多）。
- **Recommend:** ②。它把口径钉在**数自己身上**，读的人不必先知道这一条；
  ① 只活在这一条记录里，下一个跨平台比数的人照样撞。③ 是另一件事，与本条无关。
- **Whose call:** 拍板的人（`xtask` 那件工具要不要在《数》里印平台）
- **处置：** 待处理。
