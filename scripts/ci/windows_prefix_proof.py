"""Native Windows guard fault injection; never use the mutant for delivery.

Intentionally coupled to the raw-name eligibility guard in src/place.rs. If that
guard or the public Place table names change, update this proof and its tests;
never add a production test hook. Adapter tests are not native Windows evidence.
"""
import argparse
import hashlib
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

POSITIVE = "place::tests::safe_windows_prefixes_share_keys_without_changing_the_tree"
NEGATIVE = "place::tests::windows_namespace_semantics_are_not_erased_by_prefix_equivalence"
GUARD = b"if !safe_windows_name(name) {"
MUTANT = b"if false && !safe_windows_name(name) {"

def show(text):
    """Print UTF-8 cargo output without the Windows console code page."""
    sys.stdout.buffer.write(text.encode("utf-8", "backslashreplace"))
    sys.stdout.buffer.flush()


def execute(args, root):
    env = dict(os.environ, CARGO_TERM_COLOR="never")
    result = subprocess.run(args, cwd=root, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, encoding="utf-8")
    show("+ " + " ".join(args) + "\n")
    show(result.stdout + "\n")
    show(f"exit: {result.returncode}\n")
    return result


def require_clean(root, run):
    result = run(["git", "status", "--porcelain", "--untracked-files=no"], root)
    if result.returncode != 0 or result.stdout.strip():
        raise RuntimeError("proof requires clean tracked source")


def verify_host(root, run):
    # No command-line/environment OS override: cross-typechecking is not proof.
    if platform.system() != "Windows" or platform.machine().lower() not in {"amd64", "x86_64"}:
        raise RuntimeError("proof requires actual native Windows x64")
    rust = run(["rustc", "-vV"], root)
    cargo = run(["cargo", "-V"], root)
    if (rust.returncode or cargo.returncode
            or not re.search(r"^release: 1\.98\.0$", rust.stdout, re.M)
            or not re.search(r"^host: x86_64-pc-windows-msvc$", rust.stdout, re.M)
            or not cargo.stdout.startswith("cargo 1.98.0 ")):
        raise RuntimeError("proof requires native MSVC Rust/Cargo 1.98.0")


def table(root, run, logs, label, name, mutant=False):
    args = ["cargo", "test", "--locked", "--lib", name, "--", "--exact", "--nocapture", "--color", "never"]
    result = run(args, root)
    output = result.stdout
    (logs / (label + ".log")).write_text(
        "+ " + " ".join(args) + f"\nexit: {result.returncode}\n" + output, encoding="utf-8")
    state = "FAILED" if mutant else "ok"
    passed, failed = (0, 1) if mutant else (1, 0)
    summary = rf"^test result: {'FAILED' if mutant else 'ok'}\. {passed} passed; {failed} failed; 0 ignored; 0 measured; \d+ filtered out;"
    if (result.returncode != (101 if mutant else 0)
            or not re.search(rf"^test {re.escape(name)} \.\.\. {state}$", output, re.M)
            or not re.search(r"^running 1 test$", output, re.M)
            or not re.search(summary, output, re.M)
            or mutant and not re.search(rf"^thread '{re.escape(name)}'( \(\d+\))? panicked at", output, re.M)):
        raise RuntimeError(f"{label}: not the required actual table result (exit {result.returncode})")


def prove(root, logs, run=execute):
    """Orchestration seam for external-process adapter tests, not an OS override."""
    logs.mkdir(parents=True, exist_ok=True)
    require_clean(root, run)
    source = root / "src/place.rs"
    original = source.read_bytes()
    before = hashlib.sha256(original).hexdigest()
    show(f"source SHA256 before: {before}\n")
    if original.count(GUARD) != 1 or MUTANT in original:
        raise RuntimeError("raw eligibility guard changed: update the proof coupling")
    try:
        table(root, run, logs, "before-positive", POSITIVE)
        table(root, run, logs, "before-negative", NEGATIVE)
        source.write_bytes(original.replace(GUARD, MUTANT, 1))
        table(root, run, logs, "mutant-negative", NEGATIVE, mutant=True)
    finally:
        source.write_bytes(original)
        after = hashlib.sha256(source.read_bytes()).hexdigest()
        print(f"source SHA256 restored: {after}", flush=True)
        (logs / "source-sha256.txt").write_text(f"before {before}\nafter {after}\n", encoding="utf-8")
        if source.read_bytes() != original or after != before:
            raise RuntimeError("source restoration failed")
        require_clean(root, run)
    table(root, run, logs, "restored-positive", POSITIVE)
    table(root, run, logs, "restored-negative", NEGATIVE)
    require_clean(root, run)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logs", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    verify_host(root, execute)
    prove(root, args.logs)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError) as error:
        sys.exit(str(error))
