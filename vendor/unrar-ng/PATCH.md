# 这份 fork 是干什么的

**这一处的唯一出处就是本文。** `Cargo.toml` 的 `[patch.crates-io]`、
`THIRD-PARTY-NOTICES.md` 的《UnRAR》、票 `p4-parking-lot/17` 的《落地记录》
都只留一句指过来，不复述下面这套机制。

`unrar-ng` 0.7.7 **原样**一份，加**一处**改动：跨卷回调里那一句越界读。
上游发布带着这个修的版本（0.7.8 或更新）的那天，这个目录整个删掉、
`Cargo.toml` 里那节 `[patch.crates-io]` 一并删掉、依赖版本往上抬 —— 撤掉它不需要别的条件。

- **上游**：<https://github.com/ttys3/unrar.rs>，crates.io 上的 `unrar-ng` 0.7.7
  （`.cargo_vcs_info.json` 记着打包那一刻的 `0971923085c66fa663b1081dd31c5e1150be12cb`）
- **许可**：MIT OR Apache-2.0，两份全文跟着一起躺在这个目录里
  （`LICENSE-MIT`、`LICENSE-APACHE`）。被 `unrar-ng-sys` 编进二进制的那份 UnRAR C++ 源码
  **不在这里**，它照旧从 crates.io 上来，那一份的约束见 `THIRD-PARTY-NOTICES.md` 的《UnRAR》。
- **除这一处以外逐字节相同**：
  `diff -r ~/.cargo/registry/src/index.crates.io-*/unrar-ng-0.7.7 vendor/unrar-ng`
  只该报出 `src/open_archive.rs` 这一处，加上被摘掉的 `.cargo-ok` 与新加的这份 `PATCH.md`。

## 改的是哪一句

`src/open_archive.rs`，`Internal::<M>::callback` 的 `UCM_CHANGEVOLUMEW` 那一支：

```rust
// 改前
let next = unsafe { widestring::WideCString::from_ptr_truncate(p1 as *const _, 2048) };
// 改后
let next = unsafe { widestring::WideCString::from_ptr_str(p1 as *const _) };
```

## 为什么非改不可

`from_ptr_truncate(p, len)` 的 `len` 是**元素个数**，而它**先建切片再找 NUL**
（`widestring` 1.2.1 的 `UCString::from_ptr_truncate`：`slice::from_raw_parts(p, len)`
之后才 `from_vec_truncate`）。
Linux 上 `wchar_t` 是 4 字节，`2048` 因此是 **8 KiB**；而 UnRAR 那侧交过来的缓冲有多大，
上游自己的注释也只敢说 "2048 *seems to be* the buffer size"。

`from_raw_parts` 这个调用**本身**就要求整段可读——一个字节都不读也已经是 UB。
实测：`-C debug-assertions=on` 下 `copy_nonoverlapping` 判出重叠、`SIGABRT`（退出码 134）；
`off` 下不炸，但那 8 KiB 真的被读了。

这一支只在**跨卷**时进得去，因此单份 `.rar` 一辈子踩不到它；tonefit 收下分卷 `.rar`
（`p4-parking-lot/17`）之后，`.rar` 那两条路（`source::rar_headers` 列成员、
`source::spread_rar` 摊开）都从卷边界上走过去，两条都会踩。

`from_ptr_str` 是**先扫 NUL、只复制到那里**的那一个，读的字节不多于 UnRAR 真写进去的那些。
空指针那一格两者同形（都 panic），因此这一句换掉不改变任何别的行为。

## 报给上游的那一份

草稿在 `.scratch/p4-parking-lot/upstream-bug-unrar-ng.md`。它在仓库里，
但**没有发给上游**——往仓库外发东西是拍板的人的事。
