# 09 — 收掉夹具与用例里四处会给假绿灯的地方

**What to build:** 测试说出它真正验过的事。四处都不改产品行为，但都会让后来的人拿到假的信心。

**Blocked by:** `page-geometry/04`（跨页拆分会再动一批夹具，现在整理等于整两遍）

**Status:** ready-for-agent

- [ ] `fixtures::gradient` 在默认适配方式下会被裁掉下面 21.6%（那一片 200–255 的浅灰按行列
      墨量占比就是白边），而它有 **101 处**调用。断言真踩在几何上的已经换成 `full_bleed_gradient`，
      其余原样留着——**风险是后来的人给它们加一条尺寸断言时会撞上一个解释不了的数**
      （1441×2048 的页出成 1507×1680）。逐处判一遍「这一条问不问几何」
- [ ] `SMALLER_THAN_TARGET`（800×1000）在默认路径上被放大到 1344×1680、像素多 2.8 倍，
      而 **53 处**调用仍拿它当「一张便宜的页」。全套测试因此从约 12 秒涨到约 19 秒
      （`tests/container.rs` 0.22 → 2.05 秒是大头）。只要便宜页的那些换成 `fixtures::PASSES_THROUGH`
      （800×1680，两种适配方式下都恒等通过）
- [ ] `tests/isolation.rs` 的 `a_color_page_a_gray_page_and_a_failed_page_keep_their_own_pixels`
      名不副实：名字承诺三条路径各保住自己的像素，实际只断言了五页都写出来、缓存里两页、
      以及 003 与 005 **这两张同一夹具**逐字节相同。五页同尺寸，其余三页的像素一个都没比，
      套上「写出时每页都写第一页的字节」这个变异照样绿——它真正钉住的是缓存序号错格。
      补断言或改名，二选一
- [ ] `[dev-dependencies]` 里 `encoding_rs` 与 `zip` 两行冗余。原本的解释「集成测试拿不到本包的
      普通依赖」不成立（`tests/concurrency.rs` 现在就直接用着只列在 `[dependencies]` 里的 `num_cpus`），
      注释已订正，剩下的是删不删。**`image` 那一处不在此列**——dev 侧要的是 `avif` 编码特性，
      与运行时的 `avif-native` 不是一回事，重列有实义
- [ ] 闸门（`cargo test`）通过数不减；`cargo fmt --check` 与 `cargo clippy --all-targets` 保持干净

## 不要做的

- 不要顺手改产品行为。这张票只让测试说实话。
- 不要把 `full_bleed_gradient` 换回 `gradient`——四边顶着墨正是它存在的理由。
