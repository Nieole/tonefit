# avif-output — 想加 AVIF 输出，量完不做了

**Status:** landed（2026-10-27）；产物一个字节没改

## 结论

**不加 `--codec=avif`。PNG 是唯一的输出编码器**——决定在 `docs/adr/0021-png-is-the-only-output-encoder.md`，
数在 `docs/measurements.md` 的《AVIF 无损输出：这条依赖链上做不到，而近似无损在 1~4bit 内容上大 9~702 倍》。

## 为什么是这一条

三条，各自独立的理由：

1. **这条链编不出无损 AVIF。** `ravif` 把 `quality = 100` 换成 quantizer 0，而 `rav1e` 0.8.1
   不做无损（`src/rate.rs` 的 `min_qi` 一路 `.max(1)`；`src/segmentation.rs` 的
   「never bringing qidx below 1」）。实测 21 组里 17 组解回来的像素与写出去的**不相等**。
   ⇒ 《落格》过不去。
2. **近似无损在 1~4bit 内容上恰好最坏**：`1bit` 的 1264×1680 页编成 7.8 MB（PNG 11 KB 的 702 倍）。
3. **页内记录读不回来**（`docs/research/avif-in-page-metadata-carrier.md`，Q958 拍板按
   「链上现成 API 读得回」读），⇒ 元数据那道闸也过不去。

另有一笔不是闸、但一样贵的：**目标阅读器读不了**——四台自带阅读器不认，KOReader 到今天也不认
（koreader#10973 开着，koreader-base 的 `feat: add AVIF support` PR #2383 未合并）。

## probe/

`probe/avif_lossless_probe.rs` 是一次性量具，产出就是 measurements 那一节。跑法：

```
cp .scratch/avif-output/probe/avif_lossless_probe.rs tests/
cargo test --test avif_lossless_probe -- --nocapture
rm tests/avif_lossless_probe.rs
```

它只要 `[dev-dependencies]` 里那份 `image`（`avif` 特性 → ravif）与 `[dependencies]` 里的
`avif-native`，一个新依赖都不加。本机 2026-10-27（macOS arm64 8 核）：冷构建约 9 分钟，
跑一趟 625 秒。

**它不留在 `tests/`**：它钉的是上游编码器的能力边界，不是本仓的行为——与 `encoder-gate/02`
的探针同一条理由（停车场 Q956）。
