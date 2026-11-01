#!/usr/bin/env python3
"""Provision the same verified static dav1d for quality and delivery builds.

No Cargo source checksum covers this download. The source digest below does.
Only regular files/directories in the expected archive root are accepted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request

VERSION = "1.5.4"
ROOT = f"dav1d-{VERSION}"
URL = f"https://downloads.videolan.org/pub/videolan/dav1d/{VERSION}/{ROOT}.tar.xz"
SHA256 = "686616b7c69eb88d44459391ab25cac13b6647a3b288835c5784e71c1514a5c5"
TARGETS = {
    "x86_64-unknown-linux-gnu": ("Linux", "x86_64"),
    "x86_64-pc-windows-msvc": ("Windows", "x86_64"),
    "aarch64-apple-darwin": ("Darwin", "arm64"),
    "x86_64-apple-darwin": ("Darwin", "x86_64"),
}


def verify(path, digest):
    actual = hashlib.sha256(Path(path).read_bytes()).hexdigest()
    if actual != digest:
        raise ValueError(f"SHA256 mismatch: expected {digest}, got {actual}")


def extract(archive, destination):
    """Validate the entire archive before writing anything; never follow links."""
    destination = Path(destination)
    with tarfile.open(archive, "r:xz") as source:
        members = source.getmembers()
        seen = set()
        for member in members:
            name = PurePosixPath(member.name)
            if (name.is_absolute() or ".." in name.parts or "\\" in member.name
                    or not name.parts or name.parts[0] != ROOT
                    or not (member.isfile() or member.isdir())
                    or member.name in seen or ":" in member.name):
                raise ValueError(f"unsafe archive member: {member.name}")
            seen.add(member.name)
        for member in members:
            path = destination.joinpath(*PurePosixPath(member.name).parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as src, path.open("xb") as dst:
                    shutil.copyfileobj(src, dst)
                path.chmod(member.mode & 0o755)


def run(*args, **kwargs):
    print("+", *map(str, args), flush=True)
    subprocess.run(list(map(str, args)), check=True, **kwargs)


def output(*args):
    if args[0] == "cargo" and os.environ.get("TONEFIT_CARGO_SERIALIZER"):
        args = (sys.executable, os.environ["TONEFIT_CARGO_SERIALIZER"], *args)
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT, encoding="utf-8").strip()


def check_host(target):
    machine = platform.machine().lower()
    machine = {"amd64": "x86_64", "aarch64": "arm64"}.get(machine, machine)
    actual = (platform.system(), machine)
    if actual != TARGETS[target]:
        raise ValueError(f"native target {target} requires {TARGETS[target]}, got {actual}")
    return actual


LINUX_RUSTFLAGS = "-l static=stdc++"


def msvc_environment(env):
    """Never discover the MSVC compiler/linker through Git Bash's PATH."""
    root = env.get("VCToolsInstallDir")
    if not root:
        raise ValueError("VCToolsInstallDir missing; initialize the x64 MSVC toolchain")
    tools = Path(root) / "bin/Hostx64/x64"
    selected = {name: tools / f"{name}.exe" for name in ("cl", "link", "lib")}
    for name, path in selected.items():
        if not path.is_file():
            raise ValueError(f"missing MSVC tool: {path}")
        result = subprocess.run([str(path), "/?"], capture_output=True, text=True,
                                encoding="utf-8", errors="replace")
        banner = result.stdout + result.stderr
        expected = {"cl": "C/C++", "link": "Linker", "lib": "Library Manager"}[name]
        if "Microsoft" not in banner or expected not in banner:
            raise ValueError(f"unrecognized MSVC {name}: {banner}")
    return {"PATH": str(tools) + os.pathsep + env.get("PATH", ""),
            "CC": str(selected["cl"]), "CXX": str(selected["cl"]),
            "AR": str(selected["lib"]), "CC_LD": str(selected["link"]),
            "CXX_LD": str(selected["link"]),
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER": str(selected["link"])}


def probe_linux_runtime(work, env):
    """Compile the upstream link-attribute shape and prove static C++ ELF closure.

    A kind override is legal; a modifier override (-bundle) is not. Exercise real
    C++ runtime symbols, not an empty executable that could discard the library.
    The final package audit/container smoke remains independently mandatory.
    """
    probe = work / "cpp-runtime-probe"
    probe.mkdir()
    (probe / "shim.cc").write_text(
        '#include <string>\nextern "C" int tonefit_cpp_probe() {\n'
        'std::string s(256, \'x\'); return int(s.size()); }\n', encoding="utf-8")
    (probe / "main.rs").write_text(
        '#[link(name="tonefit_cpp_shim", kind="static")]\n'
        '#[link(name="stdc++")]\nunsafe extern "C" { fn tonefit_cpp_probe() -> i32; }\n'
        'fn main() { assert_eq!(unsafe { tonefit_cpp_probe() }, 256); }\n', encoding="utf-8")
    run("c++", "-c", probe / "shim.cc", "-o", probe / "shim.o", env=env)
    run("ar", "crs", probe / "libtonefit_cpp_shim.a", probe / "shim.o", env=env)
    run("rustc", probe / "main.rs", "-o", probe / "probe",
        "-L", f"native={probe}", *LINUX_RUSTFLAGS.split(), env=env)
    dynamic = output_env(["readelf", "-d", str(probe / "probe")], env)
    if "libstdc++" in dynamic:
        raise ValueError("C++ runtime probe unexpectedly requires dynamic libstdc++")
    run(probe / "probe", env=env)
    archive = Path(output_env(["c++", "-print-file-name=libstdc++.a"], env)).resolve()
    if not archive.is_file():
        raise ValueError("C++ compiler did not locate the static GNU runtime")
    return {"flags": LINUX_RUSTFLAGS, "elf_dynamic": dynamic,
            "cxx": output_env(["c++", "--version"], env), "archive": str(archive),
            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest()}


def build(work, target, archive=None, github_env=None):
    host = check_host(target)
    work = Path(work).resolve()
    # Refuse reuse: neither stale outputs nor attacker-controlled symlinks are inputs.
    work.mkdir(parents=True, exist_ok=False)
    source_archive = work / f"{ROOT}.tar.xz"
    if archive:
        shutil.copyfile(archive, source_archive)
    else:
        with urllib.request.urlopen(URL, timeout=120) as src, source_archive.open("xb") as dst:
            shutil.copyfileobj(src, dst)
    verify(source_archive, SHA256)
    extract(source_archive, work)
    prefix = work / "install"
    env = os.environ.copy()
    if host[0] == "Darwin":
        env["MACOSX_DEPLOYMENT_TARGET"] = "15.0"
    options = ["--buildtype=release", "--default-library=static", "--libdir=lib",
               f"--prefix={prefix}", "-Denable_tools=false", "-Denable_tests=false"]
    if host[0] == "Windows":
        options += ["-Db_vscrt=mt"]
        windows_tools = msvc_environment(env)
        env.update(windows_tools)
        # A native file avoids shell splitting absolute tool paths containing spaces.
        native_file = work / "msvc.ini"
        binaries = {"c": env["CC"], "cpp": env["CXX"], "ar": env["AR"],
                    "c_ld": env["CC_LD"], "cpp_ld": env["CXX_LD"]}
        entries = ["[binaries]\n"]
        for key, value in binaries.items():
            quoted = value.replace("\\", "/").replace("'", "\\'")
            entries.append(f"{key} = '{quoted}'\n")
        native_file.write_text("".join(entries), encoding="utf-8")
        options += [f"--native-file={native_file}"]
    run("meson", "setup", work / "build", work / ROOT, *options, env=env)
    run("meson", "compile", "-C", work / "build", env=env)
    run("meson", "install", "-C", work / "build", env=env)
    pc_env = env | {"PKG_CONFIG_PATH": str(prefix / "lib/pkgconfig")}
    version = output_env(["pkg-config", "--modversion", "dav1d"], pc_env)
    if version != VERSION:
        raise ValueError(f"unexpected dav1d pkg-config version: {version}")
    libraries = list((prefix / "lib").glob("*.a")) + list((prefix / "lib").glob("*.lib"))
    if not libraries or list((prefix / "lib").glob("*.dylib")) or list(prefix.rglob("*.dll")):
        raise ValueError("expected static dav1d only")
    versions = {tool: output(tool, "--version") for tool in ["meson", "ninja", "nasm", "pkg-config", "rustc", "cargo"]}
    # cl reports its version with a nonzero exit status when no source is supplied.
    compiler = subprocess.run([env["CC"]] if host[0] == "Windows" else ["cc", "--version"],
                              env=env,
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, encoding="utf-8")
    versions["cc"] = compiler.stdout.strip()
    provenance = {"source_url": URL, "source_sha256": SHA256, "dav1d": VERSION,
                  "target": target, "host": host, "platform": platform.platform(),
                  "tools": versions, "meson_options": options,
                  "commit": output("git", "rev-parse", "HEAD"),
                  "runner_image": {k: os.environ.get(k) for k in ["ImageOS", "ImageVersion"]}}
    exports = {"PKG_CONFIG_PATH": str(prefix / "lib/pkgconfig"),
               "SYSTEM_DEPS_DAV1D_LINK": "static", "SYSTEM_DEPS_DAV1D_BUILD_INTERNAL": "never"}
    if host[0] == "Darwin":
        exports["MACOSX_DEPLOYMENT_TARGET"] = "15.0"
    elif host[0] == "Linux":
        # unrar-ng-sys deliberately disables cc-rs's automatic C++ runtime link.
        provenance["cpp_runtime_probe"] = probe_linux_runtime(work, env)
        exports["RUSTFLAGS"] = LINUX_RUSTFLAGS
    else:
        exports.update(windows_tools)
        exports.update(CARGO_BUILD_TARGET=target,
                       CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-C target-feature=+crt-static")
    provenance["cargo_environment"] = exports
    provenance["compiler_environment"] = {key: env.get(key) for key in
                                           ["CC", "CXX", "CFLAGS", "CXXFLAGS", "MACOSX_DEPLOYMENT_TARGET"]}
    (work / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    if github_env:
        with open(github_env, "a", encoding="utf-8") as dst:
            for key, value in exports.items():
                if "\n" in value or "\r" in value:
                    raise ValueError("invalid environment value")
                dst.write(f"{key}={value}\n")
    print(json.dumps(exports, indent=2))


def output_env(args, env):
    return subprocess.check_output(args, env=env, text=True, encoding="utf-8").strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    check = sub.add_parser("host")
    check.add_argument("--target", required=True, choices=TARGETS)
    provision = sub.add_parser("build")
    provision.add_argument("--target", required=True, choices=TARGETS)
    provision.add_argument("--work", type=Path, required=True)
    provision.add_argument("--archive", type=Path, help="offline archive; digest is still mandatory")
    provision.add_argument("--github-env", type=Path)
    args = parser.parse_args()
    if args.command == "host":
        print(check_host(args.target))
    else:
        build(args.work, args.target, args.archive, args.github_env)


if __name__ == "__main__":
    main()
