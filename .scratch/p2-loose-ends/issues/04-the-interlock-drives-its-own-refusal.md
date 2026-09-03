# 04 — 互锁 ③ 只留一处判定

**What to build:** 「抖动开着而几何门不成立」这条拒绝，现在有两处形态：候选集那一侧
数「还剩几个候选」，互锁那一侧问「这一趟咬上 ③ 了吗」。两者答的不是同一个问题，
眼下靠一句 `debug_assert!` 把它们的等价关系拴着——调试构建上每趟都验，发布构建上没人验。

让互锁真的驱动那条拒绝，判定只留一处。

条目里另记着一件跟着它的事：拒绝那句话在**碰卷之前**就备好，那时没有页可量，
因此答不出「手上这一页换个适配方式解不解得了」。要按页分岔，得把这条拒绝挪到判门的地方。
本票要判这一步做不做，做不了要写明为什么。

收停车场的 **Q33**。

**Blocked by:** 03 — 兜底上界那道线只有一个说法（两票动同一片几何代码）

**Status:** resolved

- [x] 互锁 ③ 的判定只有一处，那句 `debug_assert!` 连同它拴的第二处形态一起消失
- [x] 位深那一维「裁空了」仍然判得出来——它不是互锁 ③，不要被一起收走
- [x] 拒绝的措辞仍只有一个出处
- [x] 「被兜底上界退回的页换过 `--fit` 仍被拒」那道例外照旧说得清，两条路都有用例
- [x] 按页分岔要么做了，要么写明为什么不做
- [x] 行为一格不变（哪些请求被拒、拒绝时说什么），黄金快照未变

## 落地记录

### 判定收在哪一处：Q33 之外的第三条路

Q33 摆的两条路都没走。它们各自的代价是真的：让 `candidates` 先问互锁再裁，
位深那一维仍得自己判空，等于多一道判定；让互锁去数候选集，它就得知道候选集怎么算。

走的是第三条——**两维各判各的，各只有一处判定，谁也不去数对方**：

```rust
fn candidates(request: &Request, gate: GeometryGate) -> Result<Vec<Candidate>> {
    if let Some(said) = why_nothing_is_left(request, gate) {
        return Err(Refusal(said).into());
    }
    ...  // 裁两道覆盖项，不再数还剩几个
}

fn why_nothing_is_left(request: &Request, gate: GeometryGate) -> Option<String> {
    // 位深那一维：点名的那一档，这块面板写不写得出（ADR 0003 的硬上界）
    if let Some(bit_depth) = request.bit_depth.filter(|depth| !depths.contains(depth)) { ... }
    // 抖动那一维：判定就是互锁 ③，这里只取它的答案
    Interlock::dither_outside_the_gate(request.dither, gate)
        .then(|| dither_outside_the_gate_error(request.fit))
}
```

原先那处形态——「两维一起裁完，数还剩几个」——**没有换个地方留着，是没了**。
`candidates` 现在一次都不数。这一步立得住靠两条，`candidates` 的文档把它们都写了出来：
候选集是两维的**全积**（`Candidate::all` 就是 `BitDepth::candidates` × `Dither::candidates`），
积空当且仅当有一维空；而**两维本身也空不了**——1bit 恒在位深那一维里
（面板灰阶数至少 2 级，`Profile::with_gray_levels` 卡着 `2..=256`），
`Dither::Off` 恒在抖动那一维里。第二条是外部不变量，不引它论证就少一环。

`nothing_left_error` 随之改名 `why_nothing_is_left`，返回 `Option<String>` 而不是
`anyhow::Error`——名字要说得出它回的是「为什么一个都不剩」这句话，不是一个是非。
戴 `Refusal` 仍只有 `candidates` 一处做，两支不会一支戴一支忘。

### 互锁那一处判定自己也收了一层

`Interlock::dither_outside_the_gate` 原先手写着 `dither == Some(FS) && !gate.holds()`——
那句话把「门拿走的是 FS」又抄了一遍，而那件事的出处是 `Dither::candidates`。改成问它：

```rust
dither.is_some_and(|named| !Dither::candidates(gate).contains(&named))
```

六格读数逐格不变（`None`、`Some(Off)` 在两种门上都还是 `false`，`Some(FS)` 只在门不成立时
`true`），但它现在问的是「**这一趟点名的那一档，门放不放行**」。这一收让本票敢把
`debug_assert!` 去掉：等价关系不再是两处手写式子碰巧相等，而是同一个出处的推论——
将来添一档抖动模式，`Dither::candidates` 一改，互锁的**读数**跟着变，不会出现
「候选集被裁空了而两道界一道都没拦」那种局面。

**跟着变的只有读数，措辞不会自己跟上**：判据现在对门拿走的任何一档都成立，而
`Interlock::DitherOutsideTheGate` 的 `Display` 仍写死 `--dither fs`。今天两档重合，
再添一档就得同一口气改那句话——这一条写进了 `dither_outside_the_gate` 的文档。

### 那句 `debug_assert!` 换成了一条两种构建都跑的用例

`src/interlock.rs` 的 `the_refusal_is_driven_by_this_interlock_alone`：扫两个覆盖项
（位深全集 + 不点名）× 抖动三种 × 两种门共 **30** 格，逐格问三件事——
两维一起裁完是不是空的、`why_nothing_is_left` 说不说得出话、以及位深那一维过得去时
说话的是不是本条判定。从前那句断言只在调试构建上验，这一条两种构建都跑。

### 位深那一维没有被一起收走

`why_nothing_is_left` 的第一支照旧只问位深，措辞一字未改；`decide` 那条
「裁到只剩一个而**没有**覆盖项的面板（`--gray-levels 2` 撞上几何门不成立）不走
`Reason::Override`」也一字未动——本票动的是「裁空了怎么拒」，不是「剩一个怎么判」。
两维一起对不上时报的仍是位深那一句（它指得出一道动得了的界），
次序在 `why_nothing_is_left` 的文档里写了下来。

### 按页分岔：**不做**

**不是做不到。**判门的地方（`Compute::gray_page`）手上有这一页的源尺寸与 `Fit::backstopped`，
`FitMode::Height.target(源尺寸, 面板).backstopped()` 就答得出「这一页换 `--fit height`
解不解得了」，挪得动。

不做的理由是那一步改的**不是这句话摆在哪里，是这句话说什么**：分岔之后，fit-inside 上
够得着出路的页只听见前半句，够不着的只听见后半句——而本票的硬约束是行为一格不变、
拒绝时说什么逐字不变、黄金快照未变，两件事直接顶上。对用户说什么是领域决定，
不是这一处的实现细节。挪得动、落点在哪、为什么不挪，三件事写进了
`dither_outside_the_gate_error` 的文档；仍然要拍的那一半记成 **Q102**。

### 行为一格不变

`Refusal` 的两句措辞逐字未动（位深那一句只是缩进跟着变了，`\` 续行把行首空白吃掉，
出来的字节相同）；`Interlock` 的 `Display` 一字未改，措辞的出处仍只有它那一份。
哪些请求被拒也未变——`the_refusal_is_driven_by_this_interlock_alone` 那 30 格正是这句话的证据。
`p2-loose-ends/03` 留下的
`a_page_barely_over_the_backstop_can_break_the_gate_after_falling_back_to_fit_inside`
两块面板照旧绿，`tests/pipeline.rs` 里两条路各一张的
`a_dither_the_geometry_gate_forbids_is_refused` 与
`on_the_default_fit_the_refusal_stops_offering_a_fit_mode_that_changes_nothing` 也照旧绿。
**黄金快照未变、未重录。**

### 数

| 闸门 | 结果 | 二进制 |
|---|---|---|
| `cargo test` | **569 通过 0 失败**（基线 568，多的一条是新增的那条用例） | 17 |
| `cargo test --no-default-features` | **534 通过 0 失败**（基线 533，同那一条） | 17 |
| `cargo check --features profiling` | 干净 | — |

闸门之外：`cargo fmt --check` 干净；`cargo clippy --all-targets` 干净；
`cargo doc --no-deps` 与 HEAD 同为 **14** 条既有告警。

### 停车场

了结 **Q33**（原文连同处置 → 本节，索引留一行指过去）；新增 **Q102**（按页分岔仍待拍板）。
待处理 41 → 41。

#### Q33 原文（了结）

> ### Q33 — 互锁 ③ 的判定在代码里有两处形态，靠一句 `debug_assert!` 拴着
>
> - **From:** 票 `05`
> - **Kind:** 路过发现
> - **Where:** `src/lib.rs` 的 `candidates`、`nothing_left_error` 与
>   `dither_outside_the_gate_error`；`src/interlock.rs` 的 `Interlock::dither_outside_the_gate`
> - **Why it did not block:** 两者答的不是同一个问题。`candidates` 答「这一套候选还剩几个」——
>   位深与抖动两维一起裁的结果；互锁那一条答「这一趟咬上了 ③ 吗」——只看抖动那一维加几何门。
>   把拒绝改成由互锁驱动，要么让 `candidates` 先问互锁再裁（多一道判定，而位深那一维仍得
>   自己判空），要么让互锁去数候选集（它就得知道候选集怎么算）。两条都是把一个模块的知识
>   搬进另一个，而本票要的是**处置**只写一处，那一条已经做到了。
>   还有一件跟着它：拒绝那句话在**碰卷之前**就备好（`Candidates::new`），那时没有页可量，
>   因此答不出「手上这一页换个适配方式解不解得了」。真要按页分岔，得把这条拒绝挪到判门的地方（`gray_page`）。
> - **What this ticket actually did:** 保留 `candidates` 的裁空判定；`nothing_left_error` 收了
>   `gate`，那一支加 `debug_assert!(Interlock::dither_outside_the_gate(request.dither, gate))`
>   与一句注释点名它就是互锁 ③。用例跑在调试构建上，等价关系因此每一趟都验。
>   错误措辞改由 `Interlock` 自己说，措辞那一侧的出处只有一个。
>   按页分岔那一步没做，换成**把话说全**：fit-inside 那一侧仍指 `--fit height`，
>   但把「被兜底上界退回的页换过去仍被拒」那道例外一并写进去，用例两条路一起钉着。
> - **Whose call:** 拍板的人（要不要让互锁真的驱动那条拒绝）

**处置：** 走 Q33 摆的两条之外的第三条路——两维各判各的、抖动那一维的判定就是互锁 ③，
候选集那一侧的裁空判定连同 `debug_assert!` 一起消失（见本节前四小节）。
跟着它的那一半（按页分岔）判定**不做**，仍待拍板，转记 **Q102**。
