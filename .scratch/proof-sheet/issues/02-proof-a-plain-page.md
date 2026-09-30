# 02 — 一张普通页出一叠样张（搬家 + tracer bullet）

**What to build:** 两件事，先后做（Q922 的拍板，2026-09-20）。

**一、搬家：把前半截也提成两边共用的一段。**零行为改变，照 01 号票的办法。
spec 第二条列的十一步里，01 提了后六步（几何与尺寸贴合检查 → 缩放 → 纸色提白 → 建参照 →
求画质分，加量化与编码）；前面那几步——**解码 → 彩页识别 → 分流 → 裁白边 → 判跨页 → 拆分 →
每半再裁**——仍在 `Compute` 上，缠着 `decoder` 与 `events` 两格。样张要走满管线，就得先把它们
也提出来：进去的是一张源页的字节加这一趟的处理选项与一个解码器，出来的是这一页的彩页识别结果、
救回了多少、以及切好裁好的那几块。它不认识事件流、不认识指纹、不认识缓存。
`Compute` 改成调它；坏页那一支（解不开就占一格白页）与报到那两句仍留在 `Compute` 上——
那是转换那一趟才有的事。

**二、tracer bullet：**一条完整的路走通：`tonefit proof <一张图> --profile <型号> --out <目录>`
拿一张**普通页**（单页、不拆、灰度路径、门成立）走一遍真管线，
把这块面板上这一页派得出的**每一个候选**各编一张，连同《参照》落进去处，
并把每一张的画质分、这一页的《判定》与《理由》、《画质门槛》连同它的**标定来源**印到 stdout。

这一张立起三样东西，后面四张都站在它上面：

- **库的第四个 seam**，与 `write_calibration_chart` 并列（spec《Implementation Decisions》第一条）。
  不并进 `run`，不给 `Mode` 加第三个取值。落盘在库内完成。
- **那条神谕用例**：样张里判定那一档的那一张，与 `run`（`--no-metadata`）对同一页写出的那一张
  **逐字节相同**。样张不许从管线上漂开，靠的就是它。
- **《样张》这个词**进 `CONTEXT.md`。

处理选项这一张只走默认那一套，`--preset` 与其余九项归 03。

**Blocked by:** 01 — 把「一页走到参照与画质分曲线」提成两边共用的一段

**Status:** resolved

搬家那一半的三条（钉法照 01 号票的《零行为改变是怎么验的》：**比文件，不比一次运行**）：

- [x] 解码 → 彩页识别 → 分流 → 裁白边 → 判跨页 → 拆分 → 每半再裁那一段提成不依赖 `Compute` 的一段；`Compute` 与样张两边都调它，文档说清它是哪两条路共用的、为什么不含 `Compute` 的状态
- [x] **黄金回归一格没动**：`tests/golden-snapshot.txt` 的 sha256 动手前后同一个数，`tests/golden.rs` 全过——快照一个字符都不许为变绿而改
- [x] **窄计数器读数一格没动**：`tests/counters.rs` 的断言一条不改、全过——搬家没把解码、缩放、参照进缓存哪一段做多或做少

tracer bullet 那一半：

- [x] `tonefit proof <一张图> --profile <型号> --out <目录>` 走通；去处不在时自己建出来
- [x] 去处里的文件数 = 候选数 + 1（《参照》）；每个文件名说得出它是哪一页、哪一个候选
- [x] 交出来的候选集恰好等于 `Candidate::all(可见灰阶数, 这一页的门)`
- [x] **神谕用例**：同一张图、同一套选项，`run --no-metadata` 写出的那一张与样张里判定那一档的那一张**逐字节相同**
- [x] 《参照》那一张解回来是 8 位灰度，灰调级数多于最高那一档的格点数——它没被量化过
- [x] 每一张候选图**落格**（`CONTEXT.md`《量化》）：解回来的取值都落在它那一档的格点上
- [x] 样张里**一个 tEXt 块都没有**
- [x] 跑完之后源文件的字节与 mtime 一格没动
- [x] stdout 印出：逐候选一行（候选 · 画质分 · 字节数 · 落到哪个文件）、判定一行（候选 · 理由）、画质门槛一行（数值**加标定来源**）、这一页的几何与提白（裁前／裁后尺寸、门成不成立、纸白与钳制宽度）
- [x] 印出来的措辞**从既有出处取**：`Reason` 的 `Display`、报告里画质门槛那一行、`format_bytes`。一处都不新写
- [x] `CONTEXT.md`《量化》加《样张 (Proof)》，紧挨《灰阶测试图》；两条各自点明分工与那一处相反（一个不走管线，一个必须走满），并写明样张眼下只在命令行上
- [x] `lib.rs` 的模块文档从「对外是三个 seam」改成四个，第四条写清它为什么不并进 `run`
- [x] `cargo xtask gate` 三条全绿；黄金回归一格没动（产物字节一个都没变）——这台 macOS 上读作「除了基线就红的那一条，没有新增的红」，见《数》与 Q995

## 停车场结转

下面四条由停车场转来（`/settle` Q206–Q994），归这张票收。**Q922 已拍板（2026-09-20）**：本票连前半截（解码到判跨页那几步）一起提，不另插票——票面要照它重写，动手之前先读。

#### Q916 — 提出来那一段吃的是整份 `Request`，而样张手上只有「一份处理选项」

- **From:** 票 `proof-sheet/01`
- **Kind:** 票面没说到的第三种情形（形状由 02 号票定，而 02 还没落地）
- **Where:** `src/lib.rs` 的 `examine_gray_page` 那个 `request: &Request` 参数；
  `src/request.rs` 的 `Request`；spec《Implementation Decisions》第八条（样张吃哪十项、不吃哪六项）
- **Why it did not block:** 票面写的是「进去的是一张解好的灰度页加**这一趟的处理选项**与面板」，
  而 `Request` 的文档头一句就是「一次处理调用的全部输入」——它就是这一趟的处理选项那一份。
  收窄成一个只装五项的新类型也走得通，但那是**替 02 号票拍板**：
  样张那条路上「处理选项」到底是个什么类型，要等它把解码、裁白边、判跨页那前半截也走通才看得清
  （那几段同样读 `request.crop` 与 `request.split`）。
- **What this ticket actually did:** 收 `&Request`。它**不是** `Compute` 的状态——
  `Compute` 只是借着它，而票面点名的四格（`counters`、`cache`、`fingerprint`、`settles`）一格都没进来。
  代价写在那一段的文档里：样张那一趟要造一份 `Request`，其中卷级的那七格
  （`inputs`、`output_root`、`progress`、`cache_budget`、`io_mode`、`metadata`、`envelope`）
  对它无从谈起。
- **Options:** ① 照旧收 `&Request`（已落地），02 造一份、卷级那几格填默认；
  ② 02 落地时提一个只装那十项处理选项的类型，`Request` 与样张两边都装它
  （`Request` 因此变成「那十项 + 卷级那几项」）；③ 把五个用得到的字段摊成五个裸参数
- **Recommend:** ② 留给 02 号票去判，但**别急着在 01 上做**：②的价值要等 02 把前半截走通才看得出来，
  而 ③ 是往回走——五个同型可空的裸参数正是 `Piece` 与 `SplitRule` 两处文档反复说不要的形状。
- **Whose call:** 02 号票的实现者（它是第一个调用方，形状合不合用由它说了算）
- **处置：** **02 号票判（2026-09-30）：照 ① 走——样张那个 seam 收整份 `Request`，不另立类型。** 前半截走通之后看得清了：样张那一路读的是型号、缩放方式、裁白边、拆分、缩放算法、提白上限、两道覆盖项（`open_source_page`、`examine_gray_page`、`Candidates::new` 三处读的并集），不读的是卷级那几格加整卷统一灰阶——这一刀**不落在任何一条既有词条上**（《处理选项》那一组里有整卷统一灰阶、内存上限、读盘方式，而型号在设备设置那一组），② 因此要先造一个新词，再让 `Request` 所有的构造处（命令行、会话、几十条用例）跟着改形。收 `Request` 还有一条正面的理由：**同一份交给 `run`，写出去的就是判定那一张**——神谕那一条用例比的正是这一句，签名本身就把它说了出来。代价写在 `write_proof` 的文档里（《`request` 读哪几格》）：卷级那几格样张一格都不读，命令行那一侧照实填（`proof_request`）。③ 仍不取。翻过来只改 `write_proof` 的签名与 `proof_request` 一处。

#### Q918 — 纸白那道 `judge` 认的是 `Mode::DryRun`，而样张既不是预览也不是照做

- **From:** 票 `proof-sheet/01`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/lib.rs` 的 `examine_gray_page` 里 `WhiteAlignment::Off if request.mode == Mode::DryRun`
  那一支（原样从 `Compute::gray_page` 搬来）；`src/request.rs` 的 `Mode`（只有两个取值）；
  spec 的 story 12「我想知道这一页的《纸白》量出来是多少、提白钳掉了多宽」
- **Why it did not block:** 这张票是零行为改变的搬家，那一支**原样跟着搬**，
  两条既有路径上的读数因此一格没动。样张撞得上它只有一种情形——用户点了
  `--white-align-limit 0` 又想看纸白读数——而样张那条路这张票上还不存在。
- **What this ticket actually did:** 原样搬，一个字没改。`Mode` 也没加第三个取值
  （spec《Implementation Decisions》第一条写死了样张**不并进** `Mode`：加取值是改写
  《模式》与《预览》两条词条的含义，按 `CLAUDE.md` 那要先拍板）。
- **Options:** ① 02 落地时样张传 `Mode::DryRun`（它确实一个字节都不写出去，语义对得上，
  但那个名字在报告里另有含义）；② 02 传 `Mode::Process`，上限取 0 的样张就读不到纸白，
  story 12 在那一角落空；③ 把那一支的条件从 `mode` 换成一个显式的入参
  （「上限取 0 时还判不判一遍」），三条路各自说得出自己要哪一种
- **Recommend:** ③，但**归 02 号票**：它是唯一知道样张要不要那个读数的人，
  而在 01 上换条件就是拿零行为改变去赌一件还没有调用方的事。
- **Whose call:** 02 号票的实现者
- **处置：** **02 号票判（2026-09-30）：照 ③ 走。** `examine_gray_page` 那一支的条件从 `request.mode == Mode::DryRun` 换成一个显式的入参 `WhiteWhenOff`（`LeaveIt`／`Foresee` 两个取值），转换那一趟由 `WhiteWhenOff::of(mode)` 推出——照做 `LeaveIt`、预览 `Foresee`，与从前逐字相同（黄金回归与窄计数器两道钉子都没响，见《落地记录》）；样张恒交 `Foresee`：上限取 0 时纸白照样读一遍（story 12）。这样样张那一路**一格 `Mode` 都不读**，`write_proof` 的文档才说得出「卷级那几格一格都不读」这一句。`Mode` 没加取值。

#### Q921 — `Examined` 没进 `CONTEXT.md` 的词汇表

- **From:** 票 `proof-sheet/01`
- **Kind:** 我确实拿不准的单项
- **Where:** `src/lib.rs` 新添的 `struct Examined`；`CLAUDE.md`《改 CONTEXT.md 的规矩》
  头一条「新词可以当场加：实现引入了一个新概念（新类型、新开关、新状态），加进词汇表是落地的一部分」
- **Why it did not block:** `Examined` **不引入新概念**：六个字段全是既有词条
  （《参照》《画质分》《尺寸贴合检查》《纸色提白》，加目标尺寸与缩放两样几何事实），
  它只是把「同一段一起算出来的那几样」捆成一个搬运用的形状。
  同类的搬运结构在这个仓库里一个都没进词汇表——`Piece`、`Placement`、`Candidates`、
  `ComputeCounters`、`Branch` 全都不在（`Settles` 在词汇表里只以《缓存》那条的转述出现，
  类型名本身也不是词条）。
- **What this ticket actually did:** 不加词条，在类型自己的文档里把六个字段各指回它的出处。
  这张票一个字都没动 `CONTEXT.md`——它是零行为改变的搬家。
- **Options:** ① 不加（已落地）；② 加进《管线》，与《缓存》《汇总》并列；
  ③ 等 02 号票落地时连《样张 (Proof)》一起判（那一条是**真**新词，spec 第十条点名要加）
- **Recommend:** ①，理由是上面那条「同类一个都不在」。要改这条惯例的话该整批改，
  而那不是这张票的地界。
- **Whose call:** 记下即可；02 号票加《样张》那一条时可以顺手再看一眼
- **处置：** **02 号票判（2026-09-30）：维持 ①。** 加《样张 (Proof)》那一条时再看了一眼：本票新添的搬运结构 `Opened`、`Pieces` 与 `Examined` 同一类，照同一条惯例都不进词条，各自在类型文档里指回出处。**进了词条的是两类**：公开的 `ProofPage`（一叠）与 `Sheet`（一张）写在《样张》那一条里——它们是库对外交出去的形状，调用方要叫得出名字；`WhiteWhenOff` 不是搬运结构，是一个**状态**（上限取 0 时量不量纸白），按《改 CONTEXT.md 的规矩》头一条补进《提白上限》那一条（评审 Standards 轴提的）。

#### Q922 — 票面说「样张要走的那几段今天全在 `gray_page` 前半截与 `gray_bytes` 里」，而前半截还有一截没提出来

- **From:** 票 `proof-sheet/01`
- **Kind:** 票面写错了
- **Where:** `src/lib.rs` 的 `Compute::page`／`split_and_branch`／`gray_pages`
  （解码 → 切开 → 逐张彩页识别 → 裁白边 → 分流）；
  票 `proof-sheet/01` 的《What to build》头一句；
  spec《Implementation Decisions》第二条列的那一串
  （「解码、转灰、裁白边、判跨页与切开、几何与尺寸贴合检查、缩放、纸色提白、构造参照、求画质分、量化、编码」）
- **Why it did not block:** 票面点名要提的是**两段**，两段都提了（`examine_gray_page` 与 `candidate_bytes`），
  五条验收一条不少。错的是那句**前提**，不是那两条指令：
  spec 第二条列的十一步里，这张票只够得着后六步——前五步（解码、转灰、裁白边、判跨页与切开）
  仍在 `Compute` 上，而且真的缠着它：解码那一次记在 `self.counters.decoder` 上、
  `Placement::new` 要 `self.fingerprint`、坏页报到要 `self.events`。
- **What this ticket actually did:** 照票面提那两段，**前五步一个字没动**——
  把它们一起提出来是第二张票的量，而且那几段缠的是另外两格（`decoder` 与 `events`），
  与这张票点名的四格不是同一批。这一条记下来，是因为 02 号票会**当场撞上它**：
  它要走满管线，就得自己把那五步再走一遍，或者先把它们也提出来。
- **Options:** ① 02 号票自己判（已把事实记在这里）；
  ② 在 02 之前再插一张「把解码到切开那一截也提成共用」的票；
  ③ 02 只走「一张图、不拆跨页」那一条最窄的路，前五步手写几行绕过去
  （但 spec 第六条要跨页出两叠，那条路 04 号票就要还回来）
- **Recommend:** ①，并让 02 在动手前先读这一条。真要插票的话是 ②，但那会把
  「tracer bullet」这张票的价值推后一轮——而 02 的验收里只有普通页，撞不撞得上要它自己量。
- **Whose call:** 02 号票的实现者；要插票的话是拍板的人
- **处置：** **拍板（2026-09-20）：`02` 自己连前半截一起提，不另插票。** 解码、转灰、裁白边、判跨页那前四五步（缠着 `Compute` 的 `decoder` 与 `events` 两格）由 `02` 一并提出来。`02` 的票面因此要重写：它从一张 tracer bullet 变成「搬家 + tracer bullet」，验收要把前半截那一次搬家的两道钉子（黄金回归、窄计数器）也写进去——照本票（`proof-sheet/01`）的办法，比文件不比一次运行。

## 落地记录

**本票做了什么。** 两件事，先后做：

1. **搬家（零行为改变）**：`Compute::split_and_branch` 里「解码 → 彩页识别 → 分流 → 裁白边 → 判跨页 → 拆分 →
   每半再裁」那一截提成自由函数 `open_source_page`（加 `gray_pieces`／`color_pieces` 两支、交出 `Opened`／`Pieces`），
   `Compute::gray_pages`／`color_pages` 两个方法并进去、不再存在；`split_and_branch` 只剩转换那一趟独有的两件——
   解不开的一页占一格白页（要指纹造来路）、每一块的 `Placement`（同样问指纹）。报到那两句仍在 `Compute::page`。
   另外 `examine_gray_page` 那一支 `request.mode == Mode::DryRun` 换成显式入参 `WhiteWhenOff`（Q918 的处置），
   `OutputPage::to_report` 里「救回没救回 → 完好页／残缺页」那一段提成 `PageOutcome::of`，两处共用。
2. **tracer bullet**：库的第四个 seam `tonefit::write_proof`（`src/proof.rs`，交出 `Proof`／`ProofPage`／`Sheet`），
   命令行 `tonefit proof <图> --profile <型号> --out <目录>`，印出来那几行在 `render::proof_note`
   （行由 `render` 出、摆法在 `render::plain`，新添 `RowKind::ProofSheet` 与 `Field::Sheet`／`Field::Bytes`）。
   `CONTEXT.md`《量化》添《样张 (Proof)》、改《灰阶测试图》那一条与底下那段引文，《提白上限》补 `WhiteWhenOff` 那一句；
   `lib.rs` 模块文档改成四个 seam。

### 零行为改变是怎么验的：比文件，不比一次运行

照 01 号票那一段的办法。两道钉子都是入库的文件：

- `tests/golden-snapshot.txt` sha256 动手前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`；
  `tests/golden.rs`（`cdc6f0dc…`）、`tests/counters.rs`（`f9fac0d5…`）两个文件的 sha256 也动手前后相同；
  `git diff 11a2370 -- tests/golden-snapshot.txt tests/golden.rs tests/counters.rs` 为空。
- 真正说「行为没变」的仍是那两批用例跑绿：见《数》。

**搬家里唯一变了的是一样对象的寿命**：解码出来的那张整图从前活到 `split_and_branch` 收尾（跟着每一块走完
`gray_page`），如今在 `open_source_page` 返回时就释放；源字节反过来多活到切完那一刻。两样都不进产物、不进窄计数器，
峰值内存只会更低。

### 神谕那一条：夹具要让每一步都真在做事（按反跑过）

`the_verdict_sheet_is_byte_for_byte_what_run_writes_without_metadata`。**头一版夹具是 B 类中位尺寸那张线性渐变，按反跑的时候
它失灵了**：把样张那一路的缩放算法换成 hamming，神谕照绿——对称的核把线性斜坡原样复现，换算法一个字节都不变。
夹具因此换成「一圈纯白边 + 纸白 253 的内容（`page_with_paper_white`，带一竖条硬边墨）」，并在用例里先断三条前提
（白边裁掉恰好剩内容、内容真被缩放过、纸白真被提成 `Aligned { 253 }`），再比字节。

按反那几遍各看见了什么（改完都还原了）：

| 按反 | 结果 |
|---|---|
| 样张那一路换缩放算法（`Filter::Hamming`） | 红：样张 71758 字节，转换 75493 字节 |
| 样张那一路不裁白边 | 红：66472 对 75493 |
| 样张那一路提白上限取 0 | 红：174011 对 75493 |
| 参照先按 4bit 量化再写 | 红：参照那一条（解回来是调色板、级数不够） |
| 候选那几张写成未量化的 8 位图 | 红：落格那一条（`001.1bit.png` 写着格点外的 2）与神谕那一条 |
| 出样张时把源文件原样写回一遍 | 红：mtime 那一条 |
| 样张交 `WhiteWhenOff::LeaveIt` | 红：上限取 0 那一条（读到 `Off`，要的是 `OverTheLimit { 253 }`） |
| `proof_note` 的判定那一行不出纸白那一格 | 红：措辞那一条（「纸白与钳制宽度不在」） |

tEXt 那一条自带阳性对照（同一页带着记录照做一趟，那一张读得出 tEXt），不必另按反。

### 用例（新添 11 条）

- `tests/proof.rs`（新文件，7 条）：一叠的形状（文件数 = 候选数 + 1、候选集 = `Candidate::all(可见灰阶数, 门)`、文件名）、神谕、参照没被量化过、
  每张候选落格、没有 tEXt、源的字节与 mtime 不动、上限取 0 时样张照样读出纸白（照做那一趟读到的是「没开」）。
- `src/main.rs`（3 条，都在 bin 的数里）：`proof` 子命令解析出的那一块面板与转换那条路**整份 `Profile` 相同**；
  样张吃的处理选项与一个 flag 都不点的转换那一趟逐项相同；走一遍命令行这一层、去处不在时建出来、退出码是全部成功那个数。
- `src/render.rs`（1 条，在 bin 的数里）：印出来那几行每一句的出处——画质门槛那一行拿**真跑一趟的报告抬头**比
  （不拿现拼的一句比，那样等号恒成立），判定用 `Reason` 的 `Display`，逐张那一行用 `scored_line` 与 `format_bytes`。
- lib 那一条（01 号票的 `a_page_reaches_its_reference_and_every_candidate_without_a_compute`）只多交了一格 `WhiteWhenOff::Foresee`，数不变。

### 评审收了什么、驳了什么

两轴各一个只读的子代理，看 `git diff 11a2370`。

**收下的**：

- **渲染层拿 `Mode::DryRun` 冒充样张**（两轴都提）。`page_row` 改收一个 `PaperWhite`（`Shown`／`Left`），报告那一副由 `PaperWhite::of(mode)` 推，
  样张恒交 `Shown`——与库那一侧 `WhiteWhenOff` 同一个理由。
- **逐张那一行在 `render` 里自己排版**（ADR 0016）。改成出 `RowKind::ProofSheet` 那一种行，摆法挪进 `plain::line`。
- **`Sheet.candidate: Option<Candidate>` 与四处 `find`**。`Sheet` 只剩落在哪、多大；「第几张是哪一档」由 `ProofPage::scored`
  一处配（曲线与那几张逐格同序），`render` 与用例都走它，画质分不记第二份。
- **用例借了默认值**（testing.md 第三条）。`tests/proof.rs` 的 `Plain` 把处理选项那几格点名写出，render 那一条点名提白上限 4。
- **神谕的前提没有被断言**。补上三条前提（见上一节）。
- **`WhiteWhenOff` 是新状态，该进词汇表**。补进《提白上限》（Q921 的处置跟着改）。原名 `WhenOff` 看不出说的是提白，改名。
- **`write_proof` 的文档把「两道覆盖项照裁候选集」写成了设计**。改口：眼下照裁，而 spec 第三条要的是不裁、只顶死判定，归 03。
- **命令行那条解析用例自称「与转换那条路是同一块面板」却只比了型号名**。改成比整份 `Profile`。
- **上限取 0 时样张照样量纸白没有用例**。补一条（见上表末两行之一）。

**驳回的，各写理由**：

- **《灰阶测试图》与《样张》两条都写了分工与那一处相反（单一出处）**。驳：本票验收与 spec 第十条都点名要**两条各自点明**；
  两句各从自己那一侧说，理由（「写出去会是什么样」）只在《样张》那一条。
- **「参照」这个词两处出处（`proof::REFERENCE` 与 `render::PROOF_REFERENCE`）**。驳：一个是文件叫什么（库），一个是屏上怎么说（界面层），
  哪天文件名为设备改成 ASCII，屏上照旧说「参照」——两件事恰好用同一个词条名。理由写在 `PROOF_REFERENCE` 的文档上。
- **`proof_request` 是「处理选项 → `Request`」的第三份映射**。收下事实、不在本票收：03 要给 `proof` 接 `--preset` 与九项，整个改写它。记进 **Q999**。
- **`proof_gate_row` 的「抖动」一格与报告里同一格读法不同、stdout 上没裁白边的页不印裁前裁后**。记进 **Q997**（验收条的读法由拍板的人定）。
- **「读哪几格」那张清单写了三处**（`write_proof` 文档、main.rs 用例的元组、Q916 处置）。驳：用例那一份是断言本身，票据是记录。

### 停车场

本票记 **Q995–Q999** 五条：

- **Q995** 这台 macOS 上闸门 1、2 在基线上就红（`tests/concurrency.rs` 的 `many_archive_volumes_never_hold_more_than_the_one_being_processed`，
  阳性对照只认 Linux 与 Windows），闸门的跑法因此改成 `--no-fail-fast`（见《数》）。
- **Q996** 样张对切出来的每一块都出一叠，跨页在本票上已出两叠（用例与措辞归 04）。
- **Q997** stdout 上几何与门两件照搬报告那几行的读法。
- **Q998** 去处里已有旧样张时静默混放（story 27 没落在任何一张票上）。
- **Q999** `proof_request` 那份映射的第三份。

结转的四条：**Q916** 判 ①（收整份 `Request`）、**Q918** 判 ③（`WhiteWhenOff`）、**Q921** 维持 ①（补一句：`WhiteWhenOff` 进了《提白上限》）、
**Q922** 照拍板做了（前半截一并提出来）。各自的《处置》写在上面结转那一节。

### 数

review 收完、改完、`cargo fmt` 过之后跑的**那一趟**就是最终状态（日志 `ps-02.closing.log`，放在树外；三条闸门与四条收尾同出一趟）。

**这台机器是 macOS，闸门 1、2 在没改过的树（`11a2370`）上就各红一条**：`tests/concurrency.rs:685` 的
`many_archive_volumes_never_hold_more_than_the_one_being_processed`（「开一遍那一刻目录竟改得动名」，left 0／right 96），
平台带来的，本票没碰它（Q995）。`cargo xtask gate` 头一条红了就停、`cargo test` 在头一个红的测试二进制上就不往下跑，
照那样跑本票要的两道钉子根本跑不到——**闸门 1、2 因此各跑同一条命令加 `--no-fail-fast`、各用自己那个 target 目录，
闸门 3 走 `cargo xtask gate 3`**。本票这一栏读作：**除了这一条基线红，没有新增的红；这一条在最终那一趟里照旧只红它自己。**

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test --no-fail-fast`（目录 `target`） | 合计 1005 通过 1 失败；lib 239 / bin 417；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 66.33s`（`concurrency`） |
| 2 | `cargo test --no-default-features --no-fail-fast --target-dir target/gate/no-default-features` | 合计 890 通过 1 失败；lib 239 / bin 302；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`；红的那一个二进制：`test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 81.85s`（`concurrency`，同一条） |
| 3 | `cargo xtask gate 3`（`cargo check --features profiling`，目录 `target/gate/profiling`） | 绿，`GATE3_EXIT=0`；`Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 3.39s` |

**基线**（`11a2370`，同样的跑法，日志 `ps-02.gate1-nff-baseline.log`、`ps-02.gate2-nff-baseline.log`、`ps-02.gate3-baseline.log`）：
闸门 1 是 **994 通过 1 失败**，lib 239 / bin 413；闸门 2 是 **879 通过 1 失败**，lib 239 / bin 298；闸门 3 绿。红的是同一条。

**两条闸门各多 11 条，都在预期里**：`tests/proof.rs` 新文件 7 条、bin 多 4 条（`src/main.rs` 3 条、`src/render.rs` 1 条）；
lib 239 一格没动（`WhiteWhenOff` 那一格只让 01 号票那一条多交一个参数）。失败的一条两头都是那一条。

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（两趟分别跑了 150.92 秒与 282.82 秒，机器上同时挂着别的项目的测试），
`tests/golden-snapshot.txt` sha256 落地前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`；
`tests/counters.rs` 14 条全过。`git diff 11a2370 -- tests/golden-snapshot.txt tests/golden.rs tests/counters.rs` **为空**。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check`（目录 `target`） | 绿 |
| 2 | `cargo clippy --all-targets`（目录 `target`） | **告警 0 条**（末行 `Finished \`dev\` profile … in 0.47s`） |
| 3 | `cargo clippy --all-targets --no-default-features`（目录 `target/gate/no-default-features`） | **告警 0 条**（末行 `Finished \`dev\` profile … in 27.92s`） |
| 4 | `cargo doc --no-deps`（目录 `target`） | **告警 15 条**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与 01 号票记的同数） |

自己数过一遍：`^warning` 16 行，去掉 15 条 `links to private item` 只剩合计那一行；本票新添的文档链接一条告警都没添
（`open_source_page`、`Opened`、`Pieces`、`WhiteWhenOff` 是私有项；`write_proof`、`Proof`、`ProofPage`、`Sheet` 的文档只指公开项或用反引号）。

**命令行真跑过一遍**（生成的 900×1400 页，`--profile "Kobo Libra 2"`）：去处建出来，七张（六档 + 参照），退出码 0；
stdout 头一行是报告抬头那一行（`画质门槛 5.123（在 boox-poke6 上实测，其他屏幕未验证）`），接着几何（`裁白边 900x1400 ⟶ 800x1300`）、
判定（`4bit+FS（达标的最省空间档位）`，行尾 `纸色 253 ⋅ 提了 2 级`）、尺寸贴合那一句、逐张七行。
