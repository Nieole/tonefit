# Rust license supplements

These exact locked crates omit full license texts from their crates.io tarballs.
`sources.json` pins their own VCS revision and upstream licensing declaration;
`UPSTREAM-LICENSE.md` preserves those exact bytes. All explicitly permit MIT.
`LICENSE-MIT.txt` retains the upstream authors/copyright attribution and the full
standard MIT terms selected for distribution. No copyright year is invented for
objc2 packages (their published metadata names authors but no copyright year).

The packaging helper accepts these supplements only for the exact package
version, declared license and `.cargo_vcs_info.json` revision. Unknown missing
texts remain errors. The generated inventory also retains upstream Cargo package
metadata to preserve attribution and source identifiers.
