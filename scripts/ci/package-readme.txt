tonefit — portable command-line converter with the default terminal UI

Run tonefit --help (Windows: tonefit.exe --help). Example:
  tonefit comic.rar --profile kobo-libra-2 --out converted
No Rust, Python, Homebrew or dav1d installation is needed to run this package.
Keep the accompanying licenses, notices and provenance when redistributing.

Compatibility boundary: Linux x64 glibc 2.35+ (Linux CI verification uses an
unmodified Ubuntu 22.04 userspace container); macOS deployment target 15.0+ on
the architecture in the filename;
Windows 10/11 x64 is the intended boundary, with automated execution on Windows
Server 2022, not a Windows 10/11 client certification. Older OSes are unverified.
See the accompanying verification.json CI artifact for the actual test system.

This alpha is not publisher-signed or notarized. macOS uses local ad-hoc signing,
which does NOT authenticate a Developer ID. Gatekeeper / SmartScreen may warn.
Only proceed if you trust the source: on macOS use System Settings > Privacy &
Security > Open Anyway after attempting to open; on Windows review SmartScreen's
More info before choosing Run anyway. Do not disable OS protection globally.
SHA256SUMS checks byte integrity, not publisher identity or safety.

provenance.json records source commit/tag (if present), target and build tools.
dependency-licenses.json indexes the target's transitive Rust license material;
licenses/ also contains native dav1d (including x86 ISC), UnRAR, font and Rust
standard-library notices. UnRAR's license prohibits using it to reconstruct RAR
compression or to develop a compatible RAR archiver; tonefit only reads RAR.
PR/branch Actions artifacts are temporary validation outputs, not a release.
