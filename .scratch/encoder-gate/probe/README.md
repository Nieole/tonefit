# 02 号票的探针：AVIF 的页内元数据载体

`02` 要答「**我们手上这套依赖，编得进去、读得回来吗**」。这个目录装的是答它用的那一次性量具。

**结论在 `docs/research/avif-in-page-metadata-carrier.md`**，不在这里。这里只有跑它的办法。

## 为什么不在 `tests/` 里

两条理由与三个备选，**唯一出处是 `.scratch/非阻塞问题.md` 的 Q956**。
一句话：它钉的是**上游今天的能力边界**，不是本仓的行为。

## 怎么跑

拷回 `tests/` 再跑，跑完拿走：

```
cp .scratch/encoder-gate/probe/avif_metadata_probe.rs tests/
cargo test --test avif_metadata_probe -- --nocapture
rm tests/avif_metadata_probe.rs
```

它只要 `[dev-dependencies]` 里那份 `image`（`avif` 特性 → ravif）与 `[dependencies]` 里的
`avif-native`、`png`，**一个新依赖都不加**。本机 2026-09-20、`CARGO_BUILD_JOBS=4`：
冷构建约 4 分 15 秒，跑一趟 0.75 秒。

## 它答的是哪几问

1. `set_exif_metadata` / `set_icc_profile` 收不收；
2. **自己走一遍 ISOBMFF**（`meta` → `iinf`/`infe` 找 `Exif` 项 → `iloc` 取段 → 按绝对偏移切字节），
   与写进去的载荷逐字节比——这一步绕开了「链上没有读回来那一头」这个死结；
3. 运行时那一侧的 `AvifDecoder` 四个元数据访问器各答什么；
4. 不调 `set_exif_metadata` 时文件里还剩什么；
5. 对照组：`png` 0.18.1 的 `tEXt` 在同样几段字节上说什么——
   **注意必须真把 PNG 写出去**，`add_text_chunk` 恒 `Ok`，只调它什么都测不到。

四种载荷：90 字节 ASCII 记录、47 字节非 Latin-1（UTF-8＋内嵌 NUL＋`0x80~0xFF`）、空、64 KiB。
