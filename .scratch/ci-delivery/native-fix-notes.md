
## Native blockers — 2026-11-01 local fix (uncommitted)

Observed red: run 41268907511, source 699aa67 (logs supplied at
`/tmp/ci-package-windows-native-failed.log` and `/tmp/ci-package-linux-gate-failed.log`).
Windows Meson rejected `C:\Program Files\Git\usr\bin\link.EXE` as GNU link, after
MSVC initialization but before dav1d build completion. Linux gate 1 failed at
unrar-ng-sys 0.7.7 `src/lib.rs:325`: “overriding linking modifiers from command line
is not supported”. Its Linux `#[link(name="stdc++")]` coexists with build.rs
`.cpp_link_stdlib(None)`; the bad global flag was `-l static:-bundle=stdc++`.

Proposed fix:
- Resolve cl/link/lib from initialized `VCToolsInstallDir/bin/Hostx64/x64`, require
  their files and Microsoft tool identity; missing/unknown tools fail closed.
  Prepend that directory only in the native build child environment and export
  absolute compiler/archiver/linker paths to Cargo/cc. Meson gets an explicit
  native file so paths with spaces never depend on Bash tokenization. No SDK
  version, MinGW, feature removal, or assembly disablement introduced.
- Linux uses `-l static=stdc++`: kind-only override, no modifier override.
  Real Rust 1.98 metadata compilation on macOS proves this is accepted with the
  original link-attribute shape; the original bad flag is a required failing
  counterexample. Linux provisioning now additionally compiles/executes a C++
  string allocation through the same Rust attribute, rejects dynamic libstdc++
  NEEDED, and records compiler, static runtime archive path/hash and ELF output.
  Final package ELF allowlist and pristine pinned Ubuntu smoke remain unchanged;
  an explicit libstdc++.so.6 negative control was added. Existing package
  licensing already copies Ubuntu gcc-11-base copyright (including GCC runtime
  exception/attributions) and GPL-3; this remains required. No vendor fork or
  decoder change, no external libstdc++ allowance, no release helper edit.

Proof so far:
- Synthetic Git-link shadowing, unknown/missing tools, and mocked full Windows
  Meson-to-Cargo environment handoff regressions added, alongside the real rustc
  good-kind/bad-modifier source fixture. Full helper suite: 43 tests (original
  39 + 4), macOS Rust 1.98.0; final log `/tmp/ci-native-fix-helpers.log`.
- actionlint exits 0: `/tmp/ci-native-fix-actionlint.log`.
- Docker CLI exists but daemon is unavailable (`orbstack` socket missing), logged
  in `/tmp/ci-native-fix-docker.log`. Therefore no local real Linux compiler/link
  or clean Ubuntu execution proof is claimed. Linux native CI must run both the
  new runtime probe and full final package audit/container smoke.
- Native macOS static provisioning, three gates/polish, release/package audit
  launched with `/tmp/tonefit-rust198` (actual Rust 1.98), CARGO_INCREMENTAL=0;
  every Cargo invocation serialized through `/tmp/tonefit-honest-screen-cargo.py`.
  Completion results follow below. All logs use `/tmp/ci-native-fix-*`.

Remaining: parent must push and obtain full four-platform native CI green on the
final tree. Windows Rust builds and stop/console tests have not yet executed past
the observed dav1d blocker; no Windows green or Linux closure is inferred from
synthetic/local macOS results. User `.gitattributes` and the docs worker's README
and delivery documentation edits are untouched. No commit/push/tag/release.

Completion / retry-resume audit:
- Inspected the completed exit files: gates, polish and package all exit 0.
  `/tmp/ci-native-fix-gates.log`: default 1192 passed, no-default 1034 passed,
  profiling check green. `/tmp/ci-native-fix-polish.log`: fmt, both clippy modes
  and docs green; docs retain 15 existing private-link warnings.
- `/tmp/ci-native-fix-provision.log`, `-release.log`, `-package.log` and
  `/tmp/ci-native-fix-dist/verification.json` confirm real ARM macOS static dav1d
  provisioning, release construction, Mach-O system-only dependency audit,
  deployment 15.0, signing validation and extracted-archive version/help plus
  AVIF/PNG/JPEG/BMP/TIFF/WebP/GIF/solid-RAR conversion smoke. Archive SHA256
  rechecked against SHA256SUMS and verification.json:
  `1078318e0810cf40a88a419e265d1a06bc4842c9535424f9f67ba3e38d83b75d`.
  These are local dirty-tree results, not trusted release artifacts.
- No implementation or Rust inputs changed during resume; reused those completed
  native gates/package checks rather than rerunning them for this notes append.
- Reran all 43 Python helper tests successfully with actual rustc 1.98.0,
  CARGO_INCREMENTAL=0 and the configured Cargo serializer:
  `/tmp/ci-native-fix-final-helpers.log`. This includes Git-link shadowing,
  missing/unrecognized tools, Windows absolute paths containing spaces and
  Meson-to-Cargo exports, real Rust kind-only acceptance/bad-modifier rejection,
  and dynamic libstdc++ rejection in both provisioning and final ELF audit.
  Existing `/tmp/ci-native-fix-negative-controls.log` records intentionally red
  regressions when restoring the old unsupported modifier or PATH ambiguity.
- Final actionlint and git diff --check both exit 0:
  `/tmp/ci-native-fix-final-actionlint.log`, `-final-diffcheck.log`.
  spec_validate reports a valid graph with no issues.
- Node 24.22.2 design export check is byte-identical
  (`/tmp/ci-native-fix-final-design.log`); the first combined command exceeded
  its 60-second tool timeout during tests. Reran npm test with sufficient time:
  all 13 pass, zero skipped/cancelled, exit 0 in
  `/tmp/ci-native-fix-final-js.log` and `-final-js.exit`.
- No README.md, docs/delivery.md or .gitattributes edits made by this resume;
  all other workers' uncommitted changes preserved. Release helper and workflow
  security/quality barriers unchanged. No commits or publishing actions.
- Remaining proof is unchanged: parent push must establish all four native CI
  jobs green. Windows actual MSVC builds/console tests and Linux true ELF probe,
  final closure and pristine pinned Ubuntu runtime smoke are not locally proven;
  Docker daemon remains unavailable. Synthetic/macOS results do not replace them.

## Candidate 238f350 — observed CI run 41294275077 repair

Observed errors (not inferred): `/tmp/ci-candidate-linux-failed.log` shows the
binary-only native probe succeeded, then cfg-if/unicode-ident RLIB creation
failed “could not find native static library stdc++, perhaps -L missing”.
`/tmp/ci-candidate-windows-failed.log` shows the helper's copied uppercase
VCTOOLSINSTALLDIR was missed, plus valid synthetic Unix archives failed the
NTFS filesystem executable-bit check, masking five identity negative cases.

Changes:
- Resolve and SHA256 c++'s actual libstdc++.a before probing; export its parent
  as explicit rustc native search path alongside the kind-only static flag via
  CARGO_ENCODED_RUSTFLAGS (unit-separated arguments preserve spaces). Native
  provenance retains argument array, archive/hash, and missing-L diagnostic.
  The actual Linux CI probe now first requires the old missing-L RLIB compile
  to fail with that precise diagnostic, then builds the upstream link-attribute
  RLIB with explicit -L, links/runs its consumer and rejects dynamic libstdc++.
  Final ELF allowlist, pristine pinned Ubuntu container smoke and GCC licensing
  are unchanged and mandatory.
- Normalize Windows copied environment keys before SDK lookup/build handoff;
  regressions exercise uppercase SDK keys, mixed-case Path, Git shadowing and
  unchanged exact Microsoft cl/link/lib file/banner identity checks.
- safe_extract validates Unix executable TAR entry mode before writing on all
  hosts. validate_tree additionally enforces filesystem mode on Unix, not NTFS.
  Synthetic four-target release fixtures explicitly assign Unix TAR modes,
  including rearchives used by identity/tamper tests. Bad 0644 executable TAR
  is rejected before tree validation on every host. A modeled NTFS collect
  positive control and normal four-target offline CLI both pass. No release.py
  changes or archive-path/manifest/provenance/architecture relaxation; actual
  Windows .exe ZIP packaging is unchanged.

Verification (actual commands/logs under `/tmp/ci-candidate-fix-*`):
- `PATH=/tmp/tonefit-rust198/bin:$PATH CARGO_INCREMENTAL=0
  TONEFIT_CARGO_SERIALIZER=/tmp/tonefit-honest-screen-cargo.py python3 -m unittest
  discover -s scripts/ci/tests -v`: all 46 pass, zero skipped, helper log.
  Initial run exposed two test path expectations lacking macOS /private path
  resolution; corrected expectations and reran green.
- `python3 /tmp/ci-candidate-fix-negative-controls.py`: exit 0; in-memory old
  case-sensitive lookup, missing -L flags and disabled TAR mode enforcement
  each produce genuine expected-red targeted tests. No worktree rollback or
  git index mutation used. Actual unsupported-modifier Rust metadata negative
  and dynamic-libstdc++ boundary negatives also pass in the helper suite.
- `python3 scripts/ci/package.py --target aarch64-apple-darwin --verify-archive
  /tmp/ci-native-fix-dist/tonefit-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz
  --sha256 1078318e0810cf40a88a419e265d1a06bc4842c9535424f9f67ba3e38d83b75d
  --fixtures /tmp/ci-package-resume-fixtures --output
  /tmp/ci-candidate-fix-arm-reverify`: exit 0, revalidated actual final bytes,
  newly extracted executable, Mach-O closure/signature and all real conversions
  after package semantics changed. Log and verification.json retained there.
- actionlint and git diff --check exit 0; spec_validate valid. No workflow,
  JS/config or Rust inputs changed: reuse prior actual 1.98 three gates/polish
  and 13 Node 24 JS results above, not rerun or claimed as new results.
- `docker info`: exit 1, daemon still unavailable. No actual Linux Rust RLIB
  execution or Windows native tool execution claimed locally. Real runner must
  establish missing-L red / explicit-L green, full Linux closure/container smoke,
  actual Windows build/tests, and all four native jobs green on parent's push.
  Existing run's macOS-in-progress status is not a final-tree CI result.

All changes remain uncommitted. User .gitattributes edit preserved. No commit,
index/history edits, push, tag, release, or concurrent code writers used.
