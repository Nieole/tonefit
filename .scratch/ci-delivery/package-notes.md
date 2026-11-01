# PACKAGE step 4 delivery notes

## Boundary

Packaging is in each existing native quality job, after all three gates, polish,
design export checking and JS tests. No publisher, tag mutation, release or git
index/history changes. `.gitattributes` is untouched. App and root lock entry only
are 0.1.0-alpha.1; no dependency updates.

The owning contract is `.github/SPEC.md`. Helpers and archive checks are deliberately
fail-closed: unknown dynamic libraries / missing license texts fail CI, rather than
silently copying a runner's development libraries into a portable package.

## Version-dependent test expectation

An honest first gate with alpha.1 failed 109 bin tests because the immutable design
fixture loader asserted that the full application version had the same width as
`#.#.#`. No golden/design export was changed. The test-only loader now substitutes
the full version and keeps the entire banner right-anchored by consuming only the
adjacent blank gap. It still compares every cell's text/style, and has a negative
control forbidding consumption of nonblank content. This is not a runtime UI change.

## License inputs

Release compiler-artifact JSON is intersected with the target normal/build graph:
Cargo metadata alone unifies dev features (including AVIF fixture encoders), so
it cannot identify actual release code by itself. Transitive copyright/license/
notice files and upstream Cargo metadata are retained, plus Rust stdlib copyright
and accompanying license texts. Native UnRAR's version header is in provenance.

Five exact locked crates omit full licenses in their crate archives: block2,
dispatch2, objc2, objc2-encode, zune-inflate. Checked-in supplements pin the published
crate VCS revision, preserve the exact upstream licensing declaration and its hash,
and supply the full MIT terms with upstream attribution. Unknown omissions or a
changed version/license/revision still fail. No dependency source/version was changed.

## Verification log locations

All execution logs and local artifacts are outside the repository in `/tmp/ci-package-*`.
The initial alpha width failure is `/tmp/ci-package-gates.log`; the initial polish
failure was formatting while the new fixture was still being authored. Final
checks use the `*-final.log` files. An accidental first JS invocation used global
Node 25 (unsupported by jsdom); it is not acceptance proof. Acceptance uses the
fixed `/tmp/node-v24.22.2-darwin-arm64/bin` Node installation.

## Remote-only proof still required

Four-target CI execution: Windows MSVC/static CRT and PE allowlist, Linux static
C++ linkage/ELF/GLIBC audit plus digest-pinned Ubuntu 22.04 userspace execution,
and Intel macOS Mach-O/deployment/signature/runtime. Native Windows/macOS runner
execution does not certify Windows 10/11 client devices or browser quarantine /
SmartScreen flows. Linux container tests only userspace, not an old kernel.
No remote result or release is claimed by local helper tests.

## Windows UTF-8 correction

The first remote quality run, https://github.com/Nieole/tonefit/actions/runs/41226007956,
failed on Windows in `test_workflow.py` before native provisioning/build: the
implicit cp1252 read of `xtask/src/main.rs` encountered byte 0x81. Its Linux and
two macOS quality/design results do not prove this packaging tree. The failed log
is `/tmp/ci-quality-first-failed.log`; no assertion was removed or made optional.

All CI Python helpers/tests now explicitly read/write known UTF-8 source/JSON/text
and decode captured tool streams as UTF-8. The helper contract test scans these
call sites. A behavioral regression forces cp1252 defaults for both file IO and
subprocess capture, with a `中文丁` directory, UTF-8 provenance JSON, copyright
content, archive roundtrip and real child-process output. It covers the undefined
0x81 byte, not just Latin-1 characters. Running the same regression against an
in-memory copy of `package.py` with explicit encodings removed fails with the
expected `UnicodeDecodeError` while reading extracted provenance. No source files
were edited for that negative control.

## Resumed verification commands and logs

Current acceptance logs have the prefix `/tmp/ci-package-resume-`; older
`/tmp/ci-package-*-final.log` files are interrupted-worker checkpoints, not the
resumed acceptance run. The first resumed foreground Rust run was terminated by
the tool's 600-second timeout; it was restarted detached, with the same full
commands, rather than counting that partial run as green.

Environment for Rust commands:

```sh
source /tmp/ci-package-env.sh
export PATH=/tmp/tonefit-ci-python/bin:$PATH
export TONEFIT_CARGO_SERIALIZER=/tmp/tonefit-honest-screen-cargo.py
set -a; source /tmp/ci-package-native.env; set +a
```

This uses actual `/tmp/tonefit-rust198` Rust 1.98.0, `CARGO_INCREMENTAL=0`,
`MACOSX_DEPLOYMENT_TARGET=15.0` and static dav1d 1.5.4. Every Cargo invocation,
including helper metadata/provenance, goes through the serialized runner; no global
toolchain/config change. `/tmp/ci-package-resume-rust.sh` records the complete
Rust/release/fixture/archive commands; `/tmp/ci-package-resume-js.sh` records the
Node commands. The checks do not edit immutable goldens/design exports.

Completed independent commands:

- `python -m unittest discover -s scripts/ci/tests -v`: 22 passed, including
  wrong version/target/architecture/mode, missing license, tampering, unsafe tar/zip
  extraction, dependency allowlist counterexamples, release artifact identity and
  cp1252-default Unicode regression. Log: `resume-helpers-final.log`.
- `python /tmp/ci-package-resume-utf8-negative.py`: expected-red mutation confirmed
  (`UnicodeDecodeError` in `validate_tree`); wrapper exits 0 only when the negative
  control fails as intended. Log: `resume-utf8-negative-final.log`.
- `actionlint`: exit 0, `resume-actionlint-final.log` (empty).
- In `.scratch/session-redesign`, PATH prefixed with
  `/tmp/node-v24.22.2-darwin-arm64/bin`: `node --version` reports v24.22.2;
  `npm ci && npm run check && npm test` passes byte-for-byte export checking and
  all 13 JS tests, zero skipped. Log: `resume-js-final.log`, exit marker
  `resume-js.exit` = 0. npm reports two high-severity existing dependency audit
  findings; no dependency update was attempted in this package-only change.

Log basenames above are relative to `/tmp/ci-package-` (for example
`resume-helpers-final.log` means `/tmp/ci-package-resume-helpers-final.log`).

Local execution host is macOS 15.7.7 (24G720), ARM64; helper tests used the existing
`/tmp/tonefit-ci-python` Python 3.14.2 environment (CI remains pinned to 3.13.7).
Rust is `1.98.0 (88d9e12ae 2026-08-18)`, LLVM 22.1.8. The nonblocking Node 20
Action-runtime deprecation warnings from the first remote run are not treated as
packaging failures; all Action references retain full commit SHAs.

Completed serialized Rust/native/archive commands (all current exit markers are 0):

- `python /tmp/tonefit-honest-screen-cargo.py cargo run --locked --quiet --package xtask -- gate`:
  default 1192 passed / 0 failed (lib 257, bin 509), no-default 1034 / 0
  (lib 257, bin 351), profiling check passed. Log: `resume-gates-final.log`.
  Existing `TONEFIT_SAMPLES`-dependent perceptual and real-material smoke cases
  explicitly reported missing external samples; those materials were not tested.
- `python /tmp/tonefit-honest-screen-cargo.py cargo run --locked --quiet --package xtask -- polish`:
  fmt, both all-target clippy configurations and docs passed; the existing 15
  rustdoc warnings remain. Log: `resume-polish-final.log`.
- `python /tmp/tonefit-honest-screen-cargo.py cargo build --locked --release --target aarch64-apple-darwin --message-format=json-render-diagnostics > /tmp/ci-package-resume-release.json`:
  passed; `resume-release-final.log`. The partial local `/tmp` Rust installation
  replays a nonfatal rust-objcopy warning about missing libLLVM.dylib. Package
  preparation independently performs system `strip -x` and ad-hoc signing, and
  final Mach-O/runtime validation passed; the delivered executable does not
  import libLLVM. No warning was hidden or gate weakened.
- `TONEFIT_PACKAGE_FIXTURES=/tmp/ci-package-resume-fixtures python /tmp/tonefit-honest-screen-cargo.py cargo test --locked --test package`:
  passed, `resume-fixtures-final.log`.
- `python scripts/ci/native.py build --target aarch64-apple-darwin --work /tmp/ci-package-resume-native-中文丁 --archive /tmp/ci-package-native/dav1d-1.5.4.tar.xz --github-env /tmp/ci-package-resume-native.env`:
  verified the pinned source hash, built/installed static 1.5.4 under a Unicode
  path, recorded current helper provenance and exports; `resume-native-final.log`,
  `resume-native.exit` = 0. Meson is 1.9.1; the pinned Ninja 1.13.0 wheel reports
  `1.13.0.git.kitware.jobserver-pipe-1`. This independent
  provisioning check uses the same pinned source as the release build's existing
  `/tmp/ci-package-native` installation, not an unverified system dav1d.
- `python scripts/ci/package.py --target aarch64-apple-darwin --binary target/aarch64-apple-darwin/release/tonefit --native-work /tmp/ci-package-native --build-messages /tmp/ci-package-resume-release.json --fixtures /tmp/ci-package-resume-fixtures --output /tmp/ci-package-resume-dist`:
  passed, `resume-archive-final.log`. The newly generated final tar was safely
  extracted, checked for architecture/version/licenses/manifest/executable bits,
  audited for deployment 15.0/system-only dependencies/no extra rpath, signature
  verified, then actually run with cleaned search paths for version/help and all
  seven image formats plus solid RAR. PNG output page counts were verified.
- The exact same tar bytes were reverified using `--verify-archive <archive>
  --sha256 <SHA256SUMS digest> --fixtures /tmp/ci-package-resume-fixtures` into
  both `/tmp/ci-package-resume-reverified` and `/tmp/ci-package-resume-中文丁`:
  passed, `resume-reverify-final.log` and `resume-unicode-smoke-final.log`.
  `/tmp/ci-package-resume-counterexamples.sh` contains the exact commands.
- `package.smoke` on that extracted alpha executable with expected version
  `0.1.0` rejects it (`wrong binary version`), `resume-wrongversion-final.log`.
  `TONEFIT_TEST_BINARY=/usr/bin/false python /tmp/tonefit-honest-screen-cargo.py cargo test --locked --test package`
  fails as intended (101); the same command with the actual Unicode-path
  extracted executable passes. Logs: `resume-binary-negative-final.log` and
  `resume-binary-positive-final.log`. This demonstrates no fallback to Cargo's
  binary when an external binary is requested.

The final local archive is
`/tmp/ci-package-resume-dist/tonefit-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz`;
SHA256 `867b389d16685a6f29de8dac86a881449d5a2483002d23f1650bfdb700eb9640`.
Its sibling `SHA256SUMS` and `verification.json` are the final byte audit. Local
provenance honestly records the current dirty worktree; this is not a published
release. `resume-rust.exit`, `resume-native.exit`, `resume-counterexamples.exit`
and `resume-js.exit` are all 0. `git diff --check` and spec-graph validation pass.

Windows native Rust/tests/static CRT/PE, Linux ELF/glibc 2.35/container smoke and
Intel macOS packaging remain unverified for this uncommitted tree: the parent
must run the four-target remote quality workflow after pushing it. In particular,
the early Windows Python failure must not be described as a native build result.
No release workflow, publishing, commit, push or tag was performed in this step.
