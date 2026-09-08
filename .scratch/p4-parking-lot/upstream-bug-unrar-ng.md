# 给上游的 bug 报告（草稿，**本仓没有提交它**）

- 上游：<https://github.com/ttys3/unrar.rs>（crates.io 上的 `unrar-ng`）
- 版本：0.7.7
- 本仓的处置：`[patch.crates-io]` 指到 `vendor/unrar-ng/`，只改那一行。撤掉的条件见
  `vendor/unrar-ng/PATCH.md`。
- **往仓库外发这份东西是拍板的人的事**，实现者不提交。

**这一份故意自成一份、不引仓库里的任何路径**：它是要发到仓库外面去的，
读它的人手上没有 tonefit。仓库里那份说明（机制、怎么改、什么条件撤掉）的唯一出处是
`vendor/unrar-ng/PATCH.md`——本文与它重复的那几段，是「要出门」这一条买下来的。

下面是照 GitHub issue 的形状写好的正文，英文。

---

## Title

`UCM_CHANGEVOLUMEW` callback reads out of bounds — `WideCString::from_ptr_truncate(p, 2048)` builds the slice before scanning for NUL

## Summary

`Internal::<M>::callback`'s `UCM_CHANGEVOLUMEW` arm
(`src/open_archive.rs`, v0.7.7) does:

```rust
// 2048 seems to be the buffer size in unrar,
// also it's the maximum path length since 5.00.
let next = unsafe { widestring::WideCString::from_ptr_truncate(p1 as *const _, 2048) };
```

`from_ptr_truncate(p, len)` does **not** scan for the NUL first. It builds the slice up
front and only then truncates (`widestring` 1.2.1, `src/ucstring.rs:233`):

```rust
pub unsafe fn from_ptr_truncate(p: *const $uchar, len: usize) -> Self {
    if len == 0 { return Self::default(); }
    assert!(!p.is_null());
    let slice = slice::from_raw_parts(p, len);   // <-- requires 2048 readable elements
    Self::from_vec_truncate(slice)
}
```

`len` is an **element** count, so on platforms where `wchar_t` is 32-bit (Linux, macOS)
this asserts that **8 KiB** starting at `p1` is a valid, initialized, readable object.
The comment in the crate itself only claims 2048 "*seems to be*" the buffer size, and
nothing in the unrar callback contract guarantees it — `UCM_CHANGEVOLUME[W]` just hands
you a pointer to the name unrar is about to use.

`slice::from_raw_parts` is UB the moment it is called, independently of how many elements
are subsequently read.

## Impact

- This arm is only reached when unrar crosses a **volume boundary**, so single-volume
  archives never hit it. Any consumer that lists or extracts a multi-volume archive does.
- Under `-C debug-assertions=on` the copy inside `from_vec_truncate` trips the standard
  library's overlap check in `copy_nonoverlapping` and the process aborts (`SIGABRT`,
  exit code 134).
- Under a release profile it does not abort, but the 8 KiB read still happens.
- Both `OpenArchive<List, _>`'s `Iterator` and the `Process` path go through
  `Internal::process_file_raw`, so listing a multi-volume archive is affected too, not
  just extraction.

## Reproduction

Any multi-volume RAR is enough; the first `RAR_VOL_NOTIFY` fires as soon as unrar opens
the second volume.

```rust
// debug profile (debug-assertions on)
let headers: Vec<_> = unrar_ng::Archive::new("x.part1.rar")
    .open_for_listing()
    .unwrap()
    .collect();
```

Observed: process aborts with `SIGABRT` (exit code 134) once the iterator reaches the
first member that is split across `x.part1.rar` / `x.part2.rar`.

The same abort reproduces with no RAR file at all, which isolates it to the
`from_ptr_truncate` call rather than to anything in the FFI layer:

```rust
// A short, NUL-terminated buffer standing in for what unrar hands the callback.
let name: Vec<u32> = "x.part2.rar\0".chars().map(|c| c as u32).collect();
let _ = unsafe { widestring::WideCString::from_ptr_truncate(name.as_ptr(), 2048) };
```

## Suggested fix

Scan for the NUL terminator first, i.e. use `from_ptr_str`, which is
`WideCStr::from_ptr_str(p).to_ucstring()` and never builds an oversized slice:

```rust
let next = unsafe { widestring::WideCString::from_ptr_str(p1 as *const _) };
```

Null-pointer behaviour is unchanged (both variants panic), so nothing else in the arm
needs to move.

Note that adding a bounds check *after* the current call does not help — the
`from_raw_parts` call is itself the unsound step.

## Not affected

`impl From<native::HeaderDataEx> for FileHeader` also calls
`from_ptr_truncate(filename_w_ptr, 1024)`, but there the pointee really is
`[wchar_t; 1024]` (`unrar-ng-sys` `src/lib.rs:193`), so that one is in bounds.

The manual NUL scan in `extract_callback`'s `read_filename` helper is fine as well — it
reads one element at a time and stops at the terminator.
