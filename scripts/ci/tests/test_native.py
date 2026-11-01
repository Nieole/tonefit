"""Network-free provisioning checks plus a real rustc link-attribute regression."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

HELPER = Path(__file__).resolve().parents[1] / "native.py"
spec = importlib.util.spec_from_file_location("native", HELPER)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class NativeInputTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_msvc_ignores_git_link_shadow_and_exports_absolute_tools(self):
        tools = self.root / "MSVC/14.current/bin/Hostx64/x64"
        tools.mkdir(parents=True)
        for name in ("cl", "link", "lib"):
            (tools / f"{name}.exe").touch()
        git = self.root / "Git/usr/bin"
        git.mkdir(parents=True)
        (git / "link.EXE").write_text("GNU coreutils", encoding="utf-8")
        env = {"VCToolsInstallDir": str(self.root / "MSVC/14.current"), "PATH": str(git)}
        def identify(args, **kwargs):
            self.assertEqual(Path(args[0]).parent, tools)
            banners = {"cl.exe": "Microsoft C/C++ for x64", "link.exe": "Microsoft Linker",
                       "lib.exe": "Microsoft Library Manager"}
            return subprocess.CompletedProcess(args, 0, banners[Path(args[0]).name], "")
        with patch.object(native.subprocess, "run", side_effect=identify):
            selected = native.msvc_environment(env)
        self.assertEqual(selected["PATH"].split(os.pathsep)[0], str(tools))
        for key in ("CC_LD", "CXX_LD", "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"):
            self.assertEqual(selected[key], str(tools / "link.exe"))
        self.assertEqual(selected["CC"], str(tools / "cl.exe"))
        with patch.object(native.subprocess, "run", return_value=
                          subprocess.CompletedProcess([], 0, "GNU link", "")):
            with self.assertRaisesRegex(ValueError, "unrecognized MSVC"):
                native.msvc_environment(env)
        (tools / "link.exe").unlink()
        with patch.object(native.subprocess, "run", side_effect=identify):
            with self.assertRaisesRegex(ValueError, "missing MSVC"):
                native.msvc_environment(env)
        with self.assertRaisesRegex(ValueError, "VCToolsInstallDir missing"):
            native.msvc_environment({"PATH": str(git)})

    def test_windows_build_passes_toolchain_to_meson_and_cargo(self):
        tools = self.root / "Visual Studio/bin/Hostx64/x64"
        tools.mkdir(parents=True)
        for name in ("cl", "link", "lib"):
            (tools / f"{name}.exe").touch()
        env = {"VCToolsInstallDir": str(self.root / "Visual Studio"),
               "PATH": str(self.root / "Git/usr/bin")}
        work = (self.root / "build-work").resolve()
        exported = self.root / "github-env"
        def build_tool(*args, **kwargs):
            self.assertEqual(kwargs["env"]["PATH"].split(os.pathsep)[0], str(tools))
            self.assertEqual(kwargs["env"]["CC_LD"], str(tools / "link.exe"))
            if args[1] == "setup":
                self.assertIn(f"--native-file={work / 'msvc.ini'}", args)
            if args[1] == "install":
                lib = work / "install/lib"
                lib.mkdir(parents=True)
                (lib / "dav1d.lib").touch()
        banner = subprocess.CompletedProcess([], 0, "Microsoft C/C++ Linker Library Manager", "")
        with patch.dict(os.environ, env, clear=True), \
                patch.object(native, "check_host", return_value=("Windows", "x86_64")), \
                patch.object(native.platform, "platform", return_value="Windows synthetic"), \
                patch.object(native, "verify"), patch.object(native, "extract"), \
                patch.object(native, "run", side_effect=build_tool), \
                patch.object(native, "output", return_value="test version"), \
                patch.object(native, "output_env", return_value=native.VERSION), \
                patch.object(native.subprocess, "run", return_value=banner):
            native.build(work, "x86_64-pc-windows-msvc",
                         self.archive(f"{native.ROOT}/COPYING"), exported)
        settings = exported.read_text(encoding="utf-8")
        self.assertIn(f"CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER={tools / 'link.exe'}", settings)
        self.assertIn(f"CC={tools / 'cl.exe'}", settings)
        self.assertIn("target-feature=+crt-static", settings)
        self.assertIn((tools / "link.exe").as_posix(), (work / "msvc.ini").read_text(encoding="utf-8"))

    def test_rust_kind_override_and_bad_modifier_counterexample(self):
        fixture = Path(__file__).parent / "fixtures/cpp-link.rs"
        command = ["rustc", "--crate-type=rlib", "--emit=metadata", str(fixture),
                   "-o", str(self.root / "probe.rmeta")]
        good = subprocess.run(command + native.LINUX_RUSTFLAGS.split(), capture_output=True,
                              text=True, encoding="utf-8")
        self.assertEqual(good.returncode, 0, good.stderr)
        bad = subprocess.run(command + ["-l", "static:-bundle=stdc++"], capture_output=True,
                             text=True, encoding="utf-8")
        self.assertNotEqual(bad.returncode, 0)
        self.assertIn("overriding linking modifiers", bad.stderr)

    def test_linux_runtime_probe_rejects_shared_cpp_runtime(self):
        # Boundary counterexample on every host; the real Linux build runs the probe.
        with patch.object(native, "run") as run, patch.object(native, "output_env", return_value=
                " (NEEDED) Shared library: [libstdc++.so.6]"):
            with self.assertRaisesRegex(ValueError, "unexpectedly requires dynamic libstdc"):
                native.probe_linux_runtime(self.root, os.environ.copy())
        rustc = next(call for call in run.call_args_list if call.args[0] == "rustc")
        self.assertEqual(rustc.args[-2:], ("-l", "static=stdc++"))

    def archive(self, name, kind=tarfile.REGTYPE):
        archive = self.root / "input.tar.xz"
        with tarfile.open(archive, "w:xz") as tar:
            member = tarfile.TarInfo(name)
            member.type = kind
            if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                member.linkname = "../../outside"
            member.size = 3 if kind == tarfile.REGTYPE else 0
            tar.addfile(member, io.BytesIO(b"abc") if member.size else None)
        return archive

    def test_regular_archive_extracts_exact_bytes(self):
        archive = self.archive(f"{native.ROOT}/COPYING")
        native.extract(archive, self.root / "out")
        self.assertEqual((self.root / "out" / native.ROOT / "COPYING").read_bytes(), b"abc")

    def test_rejects_traversal_absolute_foreign_and_windows_paths(self):
        for name in [f"{native.ROOT}/../../outside", "/tmp/outside", "other/file",
                     f"{native.ROOT}/C:/outside", f"{native.ROOT}/..\\outside"]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                native.extract(self.archive(name), self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_rejects_links_and_special_files(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE]:
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                native.extract(self.archive(f"{native.ROOT}/bad", kind), self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_hash_accepts_only_matching_bytes(self):
        path = self.root / "bytes"
        path.write_bytes(b"known source")
        digest = hashlib.sha256(b"known source").hexdigest()
        native.verify(path, digest)
        path.write_bytes(b"altered source")
        with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
            native.verify(path, digest)

    def test_cli_corrupt_offline_archive_fails_before_extraction_or_environment(self):
        host = (native.platform.system(), native.platform.machine().lower())
        target = next(t for t in native.TARGETS if native.TARGETS[t] ==
                      (host[0], {"amd64": "x86_64", "aarch64": "arm64"}.get(host[1], host[1])))
        archive = self.archive(f"{native.ROOT}/COPYING")
        work = self.root / "work"
        env = self.root / "github-env"
        result = subprocess.run([sys.executable, str(HELPER), "build", "--target", target,
                                 "--archive", str(archive), "--work", str(work),
                                 "--github-env", str(env)], capture_output=True, text=True, encoding="utf-8")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA256 mismatch", result.stderr)
        self.assertFalse((work / native.ROOT).exists())
        self.assertFalse(env.exists())

    def test_cli_rejects_incorrect_native_target(self):
        target = next(t for t, h in native.TARGETS.items() if h[0] != native.platform.system())
        result = subprocess.run([sys.executable, str(HELPER), "host", "--target", target],
                                capture_output=True, text=True, encoding="utf-8")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requires", result.stderr)


if __name__ == "__main__":
    unittest.main()
