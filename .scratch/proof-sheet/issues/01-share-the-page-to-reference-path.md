# 01 — 把「一页走到参照与画质分曲线」提成两边共用的一段

**What to build:** 零行为改变的搬家，为样张开路。样张要走的那几段今天全在 `gray_page` 前半截与
`gray_bytes` 里，而它们缠着 `Compute` 的状态（`counters`、`cache`、`fingerprint`、`settles`）——
样张一样都没有。**先把路铺平，再走上去**（spec《Implementation Decisions》第二条）。

**两处提取：**

- **「几何与尺寸贴合检查 → 缩放 → 纸色提白 → 建参照 → 求候选画质分」那一段。**
  进去的是一张解好的灰度页加这一趟的处理选项与面板，出来的是参照、各候选的画质分、
  这一页的门与几何事实、以及提白的结果。它不认识缓存、不认识指纹、不认识 `Settles`。
- **「按一个候选量化并编码」这件事**，从 `gray_bytes` 那个吃 `Verdict` 的形状里拿出来。
  样张要为**每一个**候选各编一张，而它手上没有判定。

`gray_page` 改成调这两段。**一行行为都不许变**——这张票的全部价值就是下一张票能直接调它们。

**Blocked by:** 无——可以立刻开始

**Status:** resolved

- [x] `gray_page` 改成调提出来的那两段；两段都不依赖 `Compute` 的状态
- [x] **黄金回归（`tests/golden.rs`）一格没动**——每一卷的判定与输出体积逐个相同，快照一个字符都不许为变绿而改
- [x] **窄计数器读数一格没动**（解码次数、缩放次数、参照进缓存次数）：搬家没把哪一段工作做多或做少
- [x] 提出来的两段各带一句文档，说清它是哪两条路共用的、以及为什么不能再含 `Compute` 的状态
- [x] `cargo xtask gate` 三条全绿；`cargo xtask polish` 四条过，`cargo doc` 告警条数不多于落地前

## 落地记录

**本票做了什么。** 零行为改变的搬家：`Compute::gray_page` 前半截与 `gray_bytes` 里的一段
各提成一个自由函数，`gray_page` 与 `gray_bytes` 改成调它们。**一行行为都没变**——
黄金快照 `tests/golden-snapshot.txt` 一个字节没动
（sha256 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`，动手前后同一个数，
`git diff -- tests/` 为空），`tests/counters.rs` 那三个窄计数器的断言一条没改、14 条全过。

### 零行为改变是怎么验的：**比文件，不比一次运行**

**这一段写给下一个做搬家票的人。** 票面那两道钉子——黄金快照与窄计数器——
**都是入库的文件，不是一次运行**：快照是 `tests/golden-snapshot.txt`，
窄计数器的期望是写死在 `tests/counters.rs` 里的字面量。所以「落地前的基线」
不必跑一趟去取，**动手之前把那两份文件的状态记下来**就是基线。

本票取的是 `sha256sum tests/golden-snapshot.txt`，动手前算一次、收尾后再算一次，
两次同为 `2a6aabc0…`；`git diff --stat -- tests/` 为空。

**这比「跑一趟、结果一样」更硬**：跑出来一样，仍可能是两处错互相抵消；
而文件一个字节没变 + 用例全绿，说的是「判据与期望都没动过，绿是它们自己绿的」。
省下的那个冷构建在这台机器上是四十分钟起。

**反过来也要说清它验不到什么**：文件没动只保证**没人为了变绿去改钉子**，
真正说「行为没变」的仍是那两批用例跑绿（本票 `tests/golden.rs` 2 条、
`tests/counters.rs` 14 条，收尾那一趟全过）。两件事要一起摆出来才够。

### 提出来的两段各是什么形状（给 02 号票）

**① `examine_gray_page` —— 一页走到参照与画质分曲线。**

```rust
fn examine_gray_page(
    source: &Path,                    // 只进那一句拒绝的措辞（撞上门的页要指得出是哪一张）
    image: &GrayImage,                // 一张解好的灰度页（裁过、可能切过）
    request: &Request,                // 这一趟的处理选项：读 fit / filter / white_align_limit / mode / profile 五项
    candidates: &Candidates,          // 两套候选，门判出来现取一套
    resampler: &resample::Resampler,  // 缩放那一笔记在调用方的账本上
) -> Result<Examined>
```

走的五步与从前逐字相同：**几何与尺寸贴合检查 → 缩放 → 纸色提白 → 建参照 → 求候选画质分**。
交出来的是

```rust
struct Examined {
    reference: Reference,          // 《参照》
    scores: Vec<CandidateScore>,   // 这一页那套候选各一格
    gate: GeometryGate,            // 这一页的尺寸贴合检查
    fit: geometry::Fit,            // 目标尺寸 + 兜底上界退没退过
    scaling: Scaling,              // 这一趟怎么缩的
    white: WhiteAlignment,         // 纸色提白做了什么
}
```

**02 号票怎么调它**：`Candidates::new(&request)` 造一套、`Resampler::default()` 现开一个，
喂进去就拿得到整条曲线。`examined.scores` 里每一格的 `candidate` 恰好等于
`Candidate::all(panel.gray_levels, examined.gate)`——02 第三条验收要的就是这一句，
新添的那条用例已经钉着它（`Candidates::new` 在没点覆盖项时那两道 `filter` 都是空操作）。
判定要的话自己叫 `decide::decide(&examined.scores, threshold, pinned)`。

**② `candidate_bytes` —— 按一个候选量化并编码。**

```rust
fn candidate_bytes(
    reference: &GrayImage,
    candidate: Candidate,       // 不是 Verdict：样张手上没有判定
    record: Option<&Record>,    // 样张恒 None（不写《记录》）
) -> Result<Vec<u8>>
```

`gray_bytes` 如今只剩盖记录那一句，量化与编码两步连同两格掐表都在这里。
**神谕那一条（02 第四条验收）因此是结构上成立的**：`run --no-metadata` 那一趟
`recorder` 是 `None`、`record` 跟着是 `None`，走的正是
`candidate_bytes(reference, verdict.candidate, None)`；样张为判定那一档调同一句、传同一个 `None`。
两边逐字节相同不是巧合，是**同一个函数同一组入参**。

### 两段都不收 `Compute` 那一摊

票面点名的四格里，缓存、指纹、`Settles` 三格一个字都没提，`ComputeCounters` 那个结构体也不进来。
留在 `gray_page` 里的正是它们：那一格装参照还是装字节由 `Settles` 定，记录器由这一卷的指纹派生，
缓存那把锁进出也在那里。各自的理由写在两段自己的文档上（票面第四条验收）。

**唯一从那一摊旁边进来的是缩放器，而它不是那一摊的一格**：签名收的是 `&resample::Resampler`，
不是 `ComputeCounters`——《窄计数器》要的是「记在动作本身上」，而账本是谁的由调用方说了算。
转换那一趟交这一卷的那一个（数要进报告），样张现开一个、一眼都不看。细节在停车场 **Q917**。

### 新添一条用例（lib 的数因此多 1）

`a_page_reaches_its_reference_and_every_candidate_without_a_compute`：
一张解好的灰度页 + 一份处理选项 + 一套候选 + 一个现开的缩放器，走到参照与曲线，
再拿曲线上每一个候选各编一张——**全程手上没有 `Compute`**。
它不断任何数值（那一半由黄金回归与 `tests/counters.rs` 钉着），断的是形状；
**编译得过本身就是它的一半**：签名里再混进一格 `Compute` 的状态，02 号票就得为它造一份假的。
它也是「02 直接调得动吗」这一问的答案——那条用例做的就是 02 要做的事。

摆在 `src/lib.rs` 的 `mod tests` 里，因为那两个符号**库内私有**，`tests/` 那一批经 `run` 进来碰不到。
模块文档跟着补了一句判据（不是一张名单，见 Q920）。

### 有一处顺序确实变了，判它不是行为（Q919）

`gray_bytes` 从前是 `量化 → 盖记录 → 编码`，如今是 `盖记录 → 量化 → 编码`——
提取逼出来的（记录要在调用 `candidate_bytes` 之前备好）。判它安全的三条：
`Recorder::gray` 是纯函数（拿指纹、来路、判定、救回量拼几个字符串，一个像素都不碰）；
盖记录那一句**两种顺序下都在两格掐表之外**，`Quantize` 与 `Encode` 逐格没动；
黄金回归与窄计数器两道钉子都没响。
`gray_bytes` 的抬头跟着改成新顺序——文档跟着实现走，不留第二份真相。

### `CONTEXT.md` 一个字都没动

`Examined` 不是新概念，六个字段全是既有词条；同类的搬运结构（`Piece`、`Placement`、
`Candidates`、`Branch`、`ComputeCounters`）在词汇表里一个都没有。判断过程记在 **Q921**。

### 评审收了三条、驳了三条

**收下的三条**（都出自标准轴）：

- **那句加粗的话与签名对不上**（硬）。原文写「它一格 `Compute` 的状态都不认识（`counters`、
  `cache`、`fingerprint`、`settles`）」，而两行之下收的就是 `&self.counters.resampler`。
  `CLAUDE.md`《文档写作》第 1 条要的是**当前成立的事实**——改成「缓存、指纹、`Settles`
  一个字都不提，`ComputeCounters` 那个结构体也不进来」，缩放器另起一段说清它为什么不是那一格。
- **两句写成了变更史**（「量化与编码那两步**下沉**到…」「掐表**跟着下沉**」）。同一条规矩，
  改成当前成立的事实。
- **`mod tests` 的模块文档列了一张名单**（锁、哨兵、本票这一条），添一条就得改它。改成判据。

**驳回的三条，各写理由**：

- **`Examined` 该按《分析环节》取名**（判断题）。驳：《分析环节》盖的是解码到进缓存那一整遍，
  而这一段只有其中五步——拿那个词命名它，词汇表里那一条就多出一个不成立的用法。
- **`gray_bytes` 成了 Middle Man**（判断题）。驳：它剩下的不是转发，是**盖记录**这一件真活，
  而「写出的字节不因为在哪一遍编的而不同」靠的正是它只有一处（单一出处）。
- **盖记录那五样是 Data Clump**（判断题）。收下事实、驳回在本票动它：收拢要改两个调用处的形状，
  那是设计不是搬家，而这张票写死了零行为改变。记进 **Q923**。

### 停车场

本票记 **Q916–Q923** 八条。**两条直接落在 02 号票头上**：

- **Q916** —— `examine_gray_page` 吃的是整份 `Request`，而样张只有 spec 第八条那十项处理选项；
  卷级那七格它无从谈起。收窄是 02 的活。
- **Q922** —— **票面那句前提不全**。它写着「样张要走的那几段**今天全在** `gray_page` 前半截与
  `gray_bytes` 里」，而 spec 第二条列的十一步里这张票只够得着后六步：
  解码、转灰、裁白边、判跨页与切开仍在 `Compute` 上，缠的是 `decoder` 与 `events` 两格
  （不是本票点名的那四格）。**02 号票动手前先读这一条。**

其余六条：缩放器那个参数（Q917）、纸白 `judge` 认 `Mode::DryRun` 而样张两种模式都不是（Q918）、
盖记录与量化换序（Q919）、`mod tests` 模块文档的范围（Q920）、`Examined` 不进词汇表（Q921）、
盖记录那五样的 Data Clump（Q923）。

### 数

review 收完、改完、`cargo fmt --check` 过之后跑的**那一趟**就是最终状态
（日志 `ps-01-closing.log`，三条闸门与四条收尾同出一趟）：`cargo xtask gate` 三条全绿（`GATE_EXIT=0`）：

| | 命令 | 末行 |
|---|---|---|
| 1 | `cargo test`（目录 `target`） | 合计 1156 通过 0 失败；lib 239 / bin 576；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 2 | `cargo test --no-default-features`（目录 `target/gate/no-default-features`） | 合计 917 通过 0 失败；lib 239 / bin 337；`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| 3 | `cargo check --features profiling`（目录 `target/gate/profiling`） | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2m 15s` |

基线 `5d406be` 是 **1155 / 916**，lib 238 / bin 576 与 lib 238 / bin 337：
**两条闸门各多 1 条，两条都在 lib，bin 两个数一格没动。**
多的那一条就是本票新添的 `a_page_reaches_its_reference_and_every_candidate_without_a_compute`
——它在特性外面，闸门 2 因此照样跑得到它。
**过去六张票 lib 238 一格没动，这张票动的是库，238 → 239 是预期内的。**

**两道钉子逐格没动**：`tests/golden.rs` 2 条全过（`the_fixed_fixtures_still_decide_the_same_way`
在这一趟跑了 1236 秒——机器上同时挂着另外三棵树），`tests/golden-snapshot.txt`
sha256 落地前后同为 `2a6aabc07627323b7ddd6539bc1c3a8450ebe8a234265fd7912fe0d48751602d`；
`tests/counters.rs` 14 条全过。`git diff --stat -- tests/` **为空**——
两份钉子一个字节都没改过，绿是它们自己绿的。

`cargo xtask polish` 四条全绿（`POLISH_EXIT=0`）：

| | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --check`（目录 `target`） | 绿 |
| 2 | `cargo clippy --all-targets`（目录 `target`） | **告警 0 条**（末行 `Finished \`dev\` profile … in 1m 33s`） |
| 3 | `cargo clippy --all-targets --no-default-features`（目录 `target/gate/no-default-features`） | **告警 0 条**（末行 `Finished \`dev\` profile … in 47.99s`） |
| 4 | `cargo doc --no-deps`（目录 `target`） | **告警 15 条**（`warning: \`tonefit\` (lib doc) generated 15 warnings`，与基线同数） |

**自己数过一遍，不只看退出码**（`polish` 有告警也退 0，这一轮栽在这条上五次）：

```
grep -c '^warning' ps-01-closing.log                          → 16（基线 16）
grep '^warning' ps-01-closing.log | grep -v 'links to private' → 1 行
                                     warning: `tonefit` (lib doc) generated 15 warnings
```

16 条全部出自第 4 条：15 条 `links to private item` 加那一行合计，两趟 clippy 一条都没有。
**本票新添的那几处文档链接一条告警都没添**：`examine_gray_page`、`Examined`、`candidate_bytes`
三者都是**私有**项，私有项的文档不进公开文档，因此指着私有项也不报
（那 15 条报的是**公开**项指私有项）。
