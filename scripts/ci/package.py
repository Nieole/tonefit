#!/usr/bin/env python3
"""Build once, audit and smoke the final archive, then expose CI artifacts (not a publisher)."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import struct
import sys
import subprocess
import tarfile
import tempfile
import tomllib
import unicodedata
import zipfile

import native

ROOT = Path(__file__).resolve().parents[2]
UBUNTU = "ubuntu:22.04@sha256:076e7bf82319619f28dd0aa9c02ab34b055ad37100395b4f49a2ffba04fb5e11"
SYSTEM_ELF = {"libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1",
              "libgcc_s.so.1", "ld-linux-x86-64.so.2"}
SYSTEM_PE = {"kernel32.dll", "ntdll.dll", "advapi32.dll", "bcrypt.dll", "userenv.dll",
             "ws2_32.dll", "shell32.dll", "ole32.dll", "oleaut32.dll", "user32.dll",
             "gdi32.dll", "comdlg32.dll", "crypt32.dll", "secur32.dll", "rpcrt4.dll",
             "normaliz.dll", "psapi.dll", "synchronization.dll", "bcryptprimitives.dll",
             "dbghelp.dll", "version.dll", "winmm.dll"}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def text(*args):
    return subprocess.check_output(list(map(str, args)), text=True, stderr=subprocess.STDOUT, encoding="utf-8")


def version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


def name(target, ver):
    if target not in native.TARGETS or not re.fullmatch(r"\d+\.\d+\.\d+(?:-[a-z]+\.\d+)?", ver):
        raise ValueError("invalid target/version")
    return f"tonefit-{ver}-{target}"


def architecture(binary, target):
    data = Path(binary).read_bytes()
    if target.endswith("linux-gnu"):
        good = data[:6] == b"\x7fELF\x02\x01" and struct.unpack_from("<H", data, 18)[0] == 62
    elif target.endswith("windows-msvc"):
        offset = struct.unpack_from("<I", data, 60)[0]
        good = data[:2] == b"MZ" and data[offset:offset+6] == b"PE\0\0\x64\x86"
    else:
        cpu = 0x100000C if target.startswith("aarch64") else 0x1000007
        good = data[:4] == b"\xcf\xfa\xed\xfe" and struct.unpack_from("<I", data, 4)[0] == cpu
    if not good:
        raise ValueError("wrong binary architecture: " + target)


def elf_audit(dynamic, symbols):
    needed = set(re.findall(r"\(NEEDED\).*?\[(.*?)\]", dynamic))
    if not needed or needed - SYSTEM_ELF or re.search(r"\((?:RPATH|RUNPATH|RELR|RELRSZ|RELRENT|AUDIT|DEPAUDIT|FILTER|AUXILIARY)\)", dynamic):
        raise ValueError(f"forbidden ELF dependencies/search paths: {needed}")
    if any(not re.fullmatch(r"GLIBC_\d+\.\d+(?:\.\d+)?", v) for v in re.findall(r"\bGLIBC_[\w.]+", symbols)):
        raise ValueError("unsupported GLIBC ABI requirement")
    versions = re.findall(r"\bGLIBC_(\d+)\.(\d+)(?:\.(\d+))?", symbols)
    if not versions or any(tuple(int(x or 0) for x in v) > (2, 35, 0) for v in versions):
        raise ValueError("GLIBC requirement exceeds 2.35 or absent")
    return sorted(needed)


def macho_audit(libraries, commands, target):
    deps = [line.strip().split(" (", 1)[0] for line in libraries.splitlines()[1:] if line.strip()]
    if not deps or any(not p.startswith(("/usr/lib/", "/System/Library/")) or ".." in PurePosixPath(p).parts for p in deps):
        raise ValueError(f"forbidden Mach-O dependency: {deps}")
    minimums = re.findall(r"(?:minos|version)\s+(\d+\.\d+(?:\.\d+)?)", commands)
    # LC_BUILD_VERSION is expected for the fixed modern toolchain; don't confuse SDK versions.
    minos = re.findall(r"\bminos\s+(\d+\.\d+(?:\.\d+)?)", commands)
    if not minos or any(v not in {"15.0", "15.0.0"} for v in minos) or "LC_RPATH" in commands:
        raise ValueError(f"unexpected Mach-O deployment target/rpath: {minimums}")
    loader = re.findall(r"cmd LC_LOAD_DYLINKER\s+cmdsize \d+\s+name (\S+)", commands)
    if loader != ["/usr/lib/dyld"] or "LC_DYLD_ENVIRONMENT" in commands:
        raise ValueError("unexpected Mach-O loader/environment")
    return deps


def pe_audit(imports):
    deps = {p.lower() for p in re.findall(r"^\s+([\w.-]+\.dll)\s*$", imports, re.MULTILINE | re.IGNORECASE)}
    forbidden = {p for p in deps - SYSTEM_PE if not re.fullmatch(r"api-ms-win-core-[a-z0-9-]+\.dll", p)}
    if not deps or forbidden:
        raise ValueError(f"forbidden PE imports (static CRT required): {forbidden}")
    return sorted(deps)


def audit(binary, target):
    architecture(binary, target)
    if target.endswith("linux-gnu"):
        dynamic = text("readelf", "-d", binary)
        symbols = text("readelf", "--version-info", binary)
        header = text("readelf", "-l", binary)
        interpreter = re.findall(r"Requesting program interpreter:\s*([^\]]+)\]", header)
        if interpreter != ["/lib64/ld-linux-x86-64.so.2"]:
            raise ValueError("unexpected ELF interpreter")
        return {"dependencies": elf_audit(dynamic, symbols), "dynamic": dynamic, "symbols": symbols}
    if target.endswith("windows-msvc"):
        imports = text("dumpbin", "/dependents", binary)
        return {"dependencies": pe_audit(imports), "imports": imports}
    libraries = text("otool", "-L", binary)
    commands = text("otool", "-l", binary)
    result = {"dependencies": macho_audit(libraries, commands, target), "load_commands": commands}
    native.run("codesign", "--verify", "--strict", binary)
    return result


def copy_file(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def licenses(stage, native_work, target, build_messages=None):
    """Resolve the target's transitive normal/build graph, retain upstream notices, including nested native code.

    Build dependencies are conservatively included (proc macros may generate linked code).
    No dev-only edge is traversed. The inventory records expressions, sources and selected files;
    a missing upstream license is a hard error, never silently replaced by an SPDX label.
    """
    driver = [sys.executable, os.environ["TONEFIT_CARGO_SERIALIZER"]] if os.environ.get("TONEFIT_CARGO_SERIALIZER") else []
    metadata = json.loads(text(*driver, "cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = next(p["id"] for p in packages.values() if p["name"] == "tonefit")
    pending, selected = [root], set()
    while pending:
        item = pending.pop()
        if item in selected:
            continue
        selected.add(item)
        pending += [d["pkg"] for d in nodes[item]["deps"]
                    if any(k["kind"] != "dev" for k in d["dep_kinds"])]
    if build_messages is not None:
        artifacts = [json.loads(line) for line in build_messages.read_text(encoding="utf-8").splitlines() if line.strip()]
        compiled = {m["package_id"] for m in artifacts if m.get("reason") == "compiler-artifact"}
        if root not in compiled or compiled - packages.keys():
            raise ValueError("release build messages do not match locked metadata")
        # metadata unifies dev features; compiler artifacts identify the actual release graph.
        selected &= compiled
    inventory = []
    native_versions = {}
    for item in sorted(selected):
        p = packages[item]
        if p["name"] == "tonefit":
            continue
        source = Path(p["manifest_path"]).parent
        files = [f for f in source.rglob("*") if f.is_file() and
                 re.match(r"(?:licen[cs]e|copying|copyright|notice)(?:[._-]|$)", f.name, re.I)]
        if p.get("license_file"):
            files.append(source / p["license_file"])
        relative = []
        if not files:
            supplements = ROOT / "licenses/rust-supplemental"
            key = f"{p['name']}-{p['version']}"
            reviewed = json.loads((supplements / "sources.json").read_text(encoding="utf-8")).get(key)
            vcs = source / ".cargo_vcs_info.json"
            if (not reviewed or not vcs.exists() or p["license"] != reviewed["declared_license"]
                    or json.loads(vcs.read_text(encoding="utf-8"))["git"]["sha1"] != reviewed["revision"]):
                raise ValueError(f"missing upstream license texts: {p['name']} {p['version']}")
            declaration = supplements / key / "UPSTREAM-LICENSE.md"
            native.verify(declaration, reviewed["upstream_sha256"])
            for filename in ["LICENSE-MIT.txt", "UPSTREAM-LICENSE.md"]:
                dest = Path("licenses/rust") / key / filename
                copy_file(supplements / key / filename, stage / dest)
                relative.append(dest.as_posix())
        metadata_path = Path("licenses/rust") / f"{p['name']}-{p['version']}" / "Cargo.toml"
        copy_file(source / "Cargo.toml", stage / metadata_path)
        relative.append(metadata_path.as_posix())
        for f in sorted(set(files)):
            if not f.stat().st_size:
                raise ValueError(f"empty upstream license: {f}")
            dest = Path("licenses/rust") / f"{p['name']}-{p['version']}" / f.relative_to(source)
            copy_file(f, stage / dest)
            relative.append(dest.as_posix())
        if p["name"] == "unrar-ng-sys":
            header = source / "vendor/unrar/version.hpp"
            native_versions["unrar_version_header"] = header.read_text(encoding="utf-8")
        inventory.append({"name": p["name"], "version": p["version"], "source": p["source"],
                          "license": p["license"], "notices": relative})
    copy_file(native_work / native.ROOT / "COPYING", stage / "licenses/dav1d-BSD-2-Clause.txt")
    copy_file(native_work / native.ROOT / "src/ext/x86/x86inc.asm", stage / "licenses/dav1d-x86-ISC.asm")
    sysroot = Path(text("rustc", "--print", "sysroot").strip()) / "share/doc/rust"
    # rust-docs is installed explicitly: standard-library third-party code is not in cargo metadata.
    copy_file(sysroot / "COPYRIGHT-library.html", stage / "licenses/rust-stdlib/COPYRIGHT-library.html")
    shutil.copytree(sysroot / "licenses", stage / "licenses/rust-stdlib/licenses")
    if target.endswith("linux-gnu"):
        # Ubuntu's GCC copyright file includes the runtime exception and all runtime attribution.
        source = Path("/usr/share/doc/gcc-11-base/copyright")
        copy_file(source, stage / "licenses/GCC-runtime-copyright.txt")
        copy_file(Path("/usr/share/common-licenses/GPL-3"), stage / "licenses/GPL-3.txt")
    (stage / "dependency-licenses.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
    return native_versions


def archive(stage, destination):
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as out:
            for f in sorted(stage.rglob("*")):
                if f.is_file():
                    out.write(f, f.relative_to(stage.parent).as_posix())
    else:
        with tarfile.open(destination, "w:gz") as out:
            out.add(stage, arcname=stage.name)


def safe_extract(source, destination, expected_root):
    """Validate all entries before writing; no symlinks, special files, aliases or collisions."""
    zipped = source.suffix == ".zip"
    with (zipfile.ZipFile(source) if zipped else tarfile.open(source, "r:gz")) as src:
        members = src.infolist() if zipped else src.getmembers()
        if len(members) > 100000 or sum(m.file_size if zipped else m.size for m in members) > 1024**3:
            raise ValueError("archive exceeds delivery size limit")
        entries, seen, prefixes = [], set(), {}
        for m in members:
            raw = m.filename if zipped else m.name
            path = PurePosixPath(raw)
            mode = (m.external_attr >> 16) if zipped else m.mode
            directory = m.is_dir() if zipped else m.isdir()
            regular = (stat.S_IFMT(mode) in (0, stat.S_IFREG, stat.S_IFDIR)) if zipped else (m.isfile() or m.isdir())
            canonical = unicodedata.normalize("NFC", path.as_posix()).casefold()
            if (not regular or mode & 0o7000 or not path.parts or path.parts[0] != expected_root or path.is_absolute()
                    or any(re.fullmatch(r"(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?", p, re.I) for p in path.parts)
                    or ".." in path.parts or "\\" in raw or any(c in raw for c in ":<>|?*") or any(ord(c) < 32 for c in raw)
                    or canonical in seen or any(p.endswith((".", " ")) for p in path.parts)
                    or raw.rstrip("/") != path.as_posix()):
                raise ValueError(f"unsafe archive member: {raw}")
            # TAR metadata, unlike NTFS st_mode, is authoritative on every host.
            if not zipped and path == PurePosixPath(expected_root, "tonefit") and not directory:
                validate_mode("unix", mode)
            for prefix in [path, *path.parents]:
                key = unicodedata.normalize("NFC", prefix.as_posix()).casefold()
                if key in prefixes and prefixes[key] != prefix.as_posix():
                    raise ValueError("archive path alias: " + raw)
                prefixes[key] = prefix.as_posix()
            seen.add(canonical)
            entries.append((m, path, mode, directory))
        files = {unicodedata.normalize("NFC", p.as_posix()).casefold() for _, p, _, d in entries if not d}
        if any(unicodedata.normalize("NFC", parent.as_posix()).casefold() in files
               for _, p, _, _ in entries for parent in p.parents):
            raise ValueError("file/directory collision")
        destination.mkdir(parents=True, exist_ok=False)
        for m, path, mode, directory in entries:
            dest = destination.joinpath(*path.parts)
            if directory:
                dest.mkdir(parents=True, exist_ok=True)
            else:
                dest.parent.mkdir(parents=True, exist_ok=True)
                with (src.open(m) if zipped else src.extractfile(m)) as inp, dest.open("xb") as out:
                    shutil.copyfileobj(inp, out)
                dest.chmod(mode & 0o777)
    return destination / expected_root


def validate_tree(stage, target, ver):
    provenance = json.loads((stage / "provenance.json").read_text(encoding="utf-8"))
    if provenance["target"] != target or provenance["version"] != ver:
        raise ValueError("wrong target/version provenance")
    required = {"LICENSE", "THIRD-PARTY-NOTICES.md", "README.txt", "dependency-licenses.json",
                "licenses/UnRAR.txt", "licenses/OFL-1.1.txt", "licenses/dav1d-BSD-2-Clause.txt",
                "licenses/dav1d-x86-ISC.asm", "licenses/rust-stdlib/COPYRIGHT-library.html"}
    required |= {"licenses/rust-stdlib/licenses/" + p for p in
                 ["MIT.txt", "Apache-2.0.txt", "LLVM-exception.txt", "GCC-exception-3.1.txt"]}
    if "linux" in target:
        required |= {"licenses/GCC-runtime-copyright.txt", "licenses/GPL-3.txt"}
    manifest = json.loads((stage / "manifest.json").read_text(encoding="utf-8"))
    actual = {f.relative_to(stage).as_posix(): digest(f) for f in stage.rglob("*")
              if f.is_file() and f.relative_to(stage).as_posix() != "manifest.json"}
    if not required <= actual.keys() or not all((stage / p).stat().st_size for p in required):
        raise ValueError("missing license/documentation")
    if actual != manifest:
        raise ValueError("package manifest tamper/missing file")
    inventory = json.loads((stage / "dependency-licenses.json").read_text(encoding="utf-8"))
    if not inventory or any(not p["notices"] or any(n not in actual or not (stage / n).stat().st_size for n in p["notices"]) for p in inventory):
        raise ValueError("missing dependency license")
    binary = stage / ("tonefit.exe" if "windows" in target else "tonefit")
    architecture(binary, target)
    # NTFS cannot represent Unix execution bits; safe_extract checks TAR modes
    # before writing. Keep the independent filesystem check on Unix hosts.
    if os.name != "nt":
        validate_mode(target, binary.stat().st_mode)
    return binary


def validate_mode(target, mode):
    if "windows" not in target and mode & 0o111 != 0o111:
        raise ValueError("missing executable permission")


def show(*parts):
    # Windows runners attach a cp1252 console; tonefit reports are Chinese.
    # Write UTF-8 bytes so echoing a successful smoke can never fail the step.
    text = " ".join(parts) + "\n"
    sys.stdout.buffer.write(text.encode("utf-8", "backslashreplace"))
    sys.stdout.buffer.flush()


def smoke(binary, fixtures, work, target, ver):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("DYLD_", "LD_", "PKG_CONFIG", "SYSTEM_DEPS"))}
    env["PATH"] = os.pathsep.join([str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32")]) if "windows" in target else "/usr/bin:/bin:/usr/sbin:/sbin"
    env["HOME"] = str(work)
    commands = [["--version"], ["--help"]] + [
        [str(fixtures / item), "--profile", "kobo-libra-2", "--out", str(work / ("out-" + item))]
        for item in ["images.cbz", "comic.rar"]]
    if "linux" in target:
        # Only an unmodified minimum userspace image; no apt, Python, decode or build packages.
        mounts = ["-v", f"{binary.parent}:/package:ro", "-v", f"{fixtures}:/fixtures:ro", "-v", f"{work}:/work"]
        script = 'set -eu; test "$(/package/tonefit --version)" = "tonefit ' + ver + '"; /package/tonefit --help > /work/help.txt; '
        for item in ["images.cbz", "comic.rar"]:
            script += f"/package/tonefit /fixtures/{item} --profile kobo-libra-2 --out /work/out-{item}; "
        native.run("docker", "run", "--rm", "--network=none", "--platform=linux/amd64",
                   "--user", f"{os.getuid()}:{os.getgid()}", "--env", "HOME=/work", *mounts, UBUNTU,
                   "/bin/sh", "-c", script)
    else:
        for args in commands:
            result = subprocess.run([str(binary), *args], check=True, env=env, cwd=work,
                                    capture_output=True, text=True, encoding="utf-8")
            show(result.stdout, result.stderr)
            if args == ["--version"] and result.stdout.strip() != f"tonefit {ver}":
                raise ValueError("wrong binary version")
            if args == ["--help"]:
                (work / "help.txt").write_text(result.stdout, encoding="utf-8")
    help_text = (work / "help.txt").read_text(encoding="utf-8")
    if not all(token in help_text for token in ["Usage: tonefit", "--profile", "--out", "--version"]):
        raise ValueError("missing packaged CLI help contract")
    # Decode outputs structurally, not merely successful exit codes / presence of a report.
    for item, count in [("images.cbz", 7), ("comic.rar", 2)]:
        outputs = list((work / ("out-" + item)).rglob("*.cbz"))
        if len(outputs) != 1:
            raise ValueError(f"missing conversion output: {item}: {outputs}")
        with zipfile.ZipFile(outputs[0]) as src:
            pages = [n for n in src.namelist() if n.endswith(".png")]
            if len(pages) != count or any(not src.read(n).startswith(b"\x89PNG\r\n\x1a\n") for n in pages):
                raise ValueError("conversion did not produce expected PNG pages")


def release_artifact(binary, messages):
    artifacts = [m for m in messages if m.get("reason") == "compiler-artifact" and m.get("executable")
                 and m["target"]["name"] == "tonefit" and "bin" in m["target"]["kind"]]
    if ([Path(m["executable"]).resolve() for m in artifacts] != [binary.resolve()]
            or set(artifacts[0]["features"]) != {"default", "tui"}
            or artifacts[0]["profile"]["test"] or artifacts[0]["profile"]["opt_level"] != "3"
            or artifacts[0]["profile"]["debug_assertions"]):
        raise ValueError("binary is not the recorded default-TUI release compiler artifact")
    return artifacts[0]


def build(args):
    native.check_host(args.target)
    ver = version()
    base = name(args.target, ver)
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="tonefit-package-") as tmp:
        stage = Path(tmp) / base
        stage.mkdir()
        binary = stage / ("tonefit.exe" if "windows" in args.target else "tonefit")
        architecture(args.binary, args.target)
        messages = [json.loads(line) for line in args.build_messages.read_text(encoding="utf-8").splitlines() if line.strip()]
        app_artifact = release_artifact(args.binary, messages)
        copy_file(args.binary, binary)
        binary.chmod(0o755)
        if "apple" in args.target:
            native.run("strip", "-x", binary)
            native.run("codesign", "--force", "--sign", "-", binary)
        for p in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            copy_file(ROOT / p, stage / p)
        shutil.copytree(ROOT / "licenses", stage / "licenses")
        native_versions = licenses(stage, args.native_work, args.target, args.build_messages)
        copy_file(ROOT / "scripts/ci/package-readme.txt", stage / "README.txt")
        provenance = json.loads((args.native_work / "provenance.json").read_text(encoding="utf-8"))
        if (provenance["target"] != args.target or provenance["dav1d"] != native.VERSION
                or provenance["source_sha256"] != native.SHA256
                or provenance["commit"] != text("git", "rev-parse", "HEAD").strip()):
            raise ValueError("native provenance does not match this build")
        provenance.update(release_features=app_artifact["features"], release_profile=app_artifact["profile"], native_versions=native_versions, dirty=bool(text("git", "status", "--porcelain").strip()), version=ver, target=args.target, commit=text("git", "rev-parse", "HEAD").strip(),
                          tags=text("git", "tag", "--points-at", "HEAD").splitlines(),
                          rust=text("rustc", "-vV"), release_messages_sha256=digest(args.build_messages), cargo_lock_sha256=digest(ROOT / "Cargo.lock"),
                          signature="ad-hoc; not Developer ID or notarized" if "apple" in args.target else "unsigned")
        (stage / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
        manifest = {f.relative_to(stage).as_posix(): digest(f) for f in stage.rglob("*") if f.is_file()}
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        final = args.output / (base + (".zip" if "windows" in args.target else ".tar.gz"))
        archive(stage, final)  # The only archive creation; everything below consumes these bytes.
        sha = digest(final)
        (args.output / "SHA256SUMS").write_text(f"{sha}  {final.name}\n", encoding="utf-8")
        report = verify_archive(final, sha, args.target, ver, args.fixtures.resolve(), Path(tmp) / "verification")
        (args.output / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def verify_archive(source, sha, target, ver, fixtures, work):
    source, fixtures, work = source.resolve(), fixtures.resolve(), work.resolve()
    native.verify(source, sha)  # Before parsing/extracting; checksum belongs to these final bytes.
    expected_filename = name(target, ver) + (".zip" if "windows" in target else ".tar.gz")
    if source.name != expected_filename:
        raise ValueError("wrong final archive filename/format")
    work.mkdir(parents=True, exist_ok=False)
    extracted = safe_extract(source, work / "extracted", name(target, ver))
    executable = validate_tree(extracted, target, ver)
    report = audit(executable, target)
    runtime = work / "runtime"
    runtime.mkdir()
    smoke(executable, fixtures.resolve(), runtime, target, ver)
    native.verify(source, sha)
    report.update(archive=source.name, sha256=sha, target=target, version=ver,
                  runtime_image=UBUNTU if "linux" in target else native.platform.platform(),
                  smoke="extracted archive: version/help, AVIF+PNG+JPEG+BMP+TIFF+WebP+GIF, solid RAR")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=native.TARGETS, required=True)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--binary", type=Path)
    inputs.add_argument("--verify-archive", type=Path, help="reverify existing final bytes without rebuilding")
    parser.add_argument("--sha256", help="required trusted checksum for --verify-archive")
    parser.add_argument("--native-work", type=Path)
    parser.add_argument("--build-messages", type=Path, help="Cargo release --message-format=json-render-diagnostics output")
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.verify_archive:
        if not args.sha256 or not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
            parser.error("--verify-archive requires --sha256")
        native.check_host(args.target)
        report = verify_archive(args.verify_archive, args.sha256, args.target, version(), args.fixtures, args.output)
        (args.output / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    else:
        if not args.native_work or not args.build_messages:
            parser.error("--binary requires --native-work and --build-messages")
        build(args)


if __name__ == "__main__":
    main()
