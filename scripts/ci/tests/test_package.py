"""Delivery guards exercise invalid data, not just successful helper invocations."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

HELPER = Path(__file__).resolve().parents[1] / "package.py"
sys.path.insert(0, str(HELPER.parent))
spec = importlib.util.spec_from_file_location("delivery_package", HELPER)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
MACHINE = package.native.platform.machine().lower()
HOST = (package.native.platform.system(), {"aarch64": "arm64", "amd64": "x86_64"}.get(MACHINE, MACHINE))
TARGET = next(t for t, host in package.native.TARGETS.items() if host == HOST)
VERSION = "0.1.0-alpha.1"


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def tree(self):
        stage = self.root / package.name(TARGET, VERSION)
        stage.mkdir()
        for p in ["LICENSE", "THIRD-PARTY-NOTICES.md", "README.txt", "licenses/UnRAR.txt",
                  "licenses/OFL-1.1.txt", "licenses/dav1d-BSD-2-Clause.txt", "licenses/dav1d-x86-ISC.asm",
                  "licenses/rust-stdlib/COPYRIGHT-library.html", "licenses/rust/example/LICENSE",
                  "licenses/rust-stdlib/licenses/MIT.txt", "licenses/rust-stdlib/licenses/Apache-2.0.txt",
                  "licenses/rust-stdlib/licenses/LLVM-exception.txt", "licenses/rust-stdlib/licenses/GCC-exception-3.1.txt",
                  "licenses/GCC-runtime-copyright.txt", "licenses/GPL-3.txt"]:
            path = stage / p
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("upstream attribution and license text", encoding="utf-8")
        (stage / "provenance.json").write_text(json.dumps(
            {"target": TARGET, "version": VERSION, "source_note": "中文丁"}, ensure_ascii=False), encoding="utf-8")
        (stage / "dependency-licenses.json").write_text(json.dumps([
            {"name": "example", "notices": ["licenses/rust/example/LICENSE"]}]), encoding="utf-8")
        data = bytearray(128)
        if "linux" in TARGET:
            data[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", data, 18, 62)
        elif "windows" in TARGET:
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 60, 64)
            data[64:70] = b"PE\0\0\x64\x86"
        else:
            data[:4] = b"\xcf\xfa\xed\xfe"
            struct.pack_into("<I", data, 4, 0x100000C if TARGET.startswith("aarch64") else 0x1000007)
        binary = stage / ("tonefit.exe" if "windows" in TARGET else "tonefit")
        binary.write_bytes(data)
        binary.chmod(0o755)
        self.manifest(stage)
        return stage

    def manifest(self, stage):
        (stage / "manifest.json").write_text(json.dumps({
            p.relative_to(stage).as_posix(): package.digest(p) for p in stage.rglob("*")
            if p.is_file() and p.relative_to(stage).as_posix() != "manifest.json"}), encoding="utf-8")

    def test_utf8_paths_inventory_and_tool_output_under_cp1252_defaults(self):
        # Model Windows' legacy default even on UTF-8 developer machines. U+4E2D's
        # UTF-8 includes 0xAD/0xB8, and U+4E01 includes the undefined cp1252 0x81.
        label = "中文丁"
        self.root = self.root / label
        self.root.mkdir()
        real_open = io.open

        def legacy_open(file, mode="r", buffering=-1, encoding=None, *args, **kwargs):
            if "b" not in mode and encoding in (None, "locale"):
                encoding = "cp1252"
            return real_open(file, mode, buffering, encoding, *args, **kwargs)

        script = "import os; os.write(1, " + repr(label.encode("utf-8")) + ")"
        with mock.patch("io.open", side_effect=legacy_open), mock.patch(
                "subprocess._text_encoding", return_value="cp1252"):
            # Exercise real license/JSON IO, archive names, extraction and validation.
            self.test_license_inventory_is_transitive_target_release_graph_not_lockfile_names()
            stage = self.tree()
            notice = stage / "licenses" / (label + ".txt")
            notice.write_text(label, encoding="utf-8")
            self.manifest(stage)
            archive = self.root / (stage.name + ".zip")
            package.archive(stage, archive)
            extracted = package.safe_extract(archive, self.root / "extracted", stage.name)
            package.validate_tree(extracted, TARGET, VERSION)
            self.assertEqual((extracted / "licenses" / notice.name).read_text(encoding="utf-8"), label)
            for output in [package.text(sys.executable, "-c", script),
                           package.native.output(sys.executable, "-c", script),
                           package.native.output_env([sys.executable, "-c", script], package.os.environ)]:
                self.assertEqual(output, label)

    def test_tar_and_zip_roundtrip_exact_contents(self):
        stage = self.tree()
        for extension in [".tar.gz", ".zip"]:
            archive = self.root / (stage.name + extension)
            package.archive(stage, archive)
            extracted = package.safe_extract(archive, self.root / extension[1:], stage.name)
            binary = package.validate_tree(extracted, TARGET, VERSION)
            original = stage / binary.name
            self.assertEqual(binary.read_bytes(), original.read_bytes())
            self.assertEqual(binary.stat().st_mode & 0o777, original.stat().st_mode & 0o777)

    def test_wrong_version_target_architecture_permissions_license_and_tamper(self):
        stage = self.tree()
        for target, ver in [(TARGET, "0.1.0"), (next(t for t in package.native.TARGETS if t != TARGET), VERSION)]:
            with self.subTest(target=target, ver=ver), self.assertRaisesRegex(ValueError, "target/version"):
                package.validate_tree(stage, target, ver)
        binary = stage / ("tonefit.exe" if "windows" in TARGET else "tonefit")
        data = binary.read_bytes()
        binary.write_bytes(bytes(4) + data[4:])
        self.manifest(stage)
        with self.assertRaisesRegex(ValueError, "architecture"):
            package.validate_tree(stage, TARGET, VERSION)
        binary.write_bytes(data)
        self.manifest(stage)
        # Windows has no Unix executable bit; test both mode contracts on every host.
        with self.assertRaisesRegex(ValueError, "executable"):
            package.validate_mode("x86_64-unknown-linux-gnu", 0o644)
        package.validate_mode("x86_64-pc-windows-msvc", 0o644)
        (stage / "LICENSE").unlink()
        self.manifest(stage)  # even a consistent manifest cannot omit required licenses
        with self.assertRaisesRegex(ValueError, "missing license"):
            package.validate_tree(stage, TARGET, VERSION)
        (stage / "LICENSE").write_text("license", encoding="utf-8")
        self.manifest(stage)
        binary.write_bytes(data + b"tampered")
        with self.assertRaisesRegex(ValueError, "tamper"):
            package.validate_tree(stage, TARGET, VERSION)
        binary.write_bytes(data)
        (stage / "licenses/manifest.json").write_text("unindexed extra file", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "tamper"):
            package.validate_tree(stage, TARGET, VERSION)

    def test_tar_rejects_links_traversal_duplicates_and_special_files_before_writes(self):
        base = package.name(TARGET, VERSION)
        for i, (name, kind) in enumerate([
                (base + "/../outside", tarfile.REGTYPE), ("/absolute", tarfile.REGTYPE),
                (base + "/a\\b", tarfile.REGTYPE), (base + "/C:/a", tarfile.REGTYPE),
                (base + "/link", tarfile.SYMTYPE), (base + "/hard", tarfile.LNKTYPE),
                (base + "/pipe", tarfile.FIFOTYPE), (base + "/./alias", tarfile.REGTYPE)]):
            archive = self.root / f"bad-{i}.tar.gz"
            with tarfile.open(archive, "w:gz") as out:
                m = tarfile.TarInfo(name)
                m.type = kind
                m.linkname = "../../outside"
                out.addfile(m)
            destination = self.root / f"out-{i}"
            with self.subTest(name=name, kind=kind), self.assertRaises(ValueError):
                package.safe_extract(archive, destination, base)
            self.assertFalse(destination.exists())
        archive = self.root / "duplicate.tar.gz"
        with tarfile.open(archive, "w:gz") as out:
            out.addfile(tarfile.TarInfo(base + "/LICENSE"))
            out.addfile(tarfile.TarInfo(base + "/license"))
        with self.assertRaises(ValueError):
            package.safe_extract(archive, self.root / "duplicate", base)
        for i, names in enumerate([(base + "/lic/a", base + "/LIC/b"),
                                   (base + "/é/a", base + "/e\u0301/b"),
                                   (base + "/file", base + "/file/child")]):
            with tarfile.open(archive, "w:gz") as out:
                for n in names:
                    out.addfile(tarfile.TarInfo(n))
            dest = self.root / f"collision-{i}"
            with self.assertRaises(ValueError):
                package.safe_extract(archive, dest, base)
            self.assertFalse(dest.exists())

    def test_zip_rejects_traversal_and_symlink(self):
        base = package.name(TARGET, VERSION)
        for i, member in enumerate([base + "/../escape", base + "/link"]):
            archive = self.root / f"bad-{i}.zip"
            with zipfile.ZipFile(archive, "w") as out:
                info = zipfile.ZipInfo(member)
                if i:
                    info.external_attr = 0o120777 << 16
                out.writestr(info, "../../escape")
            with self.assertRaises(ValueError):
                package.safe_extract(archive, self.root / f"zip-{i}", base)

    def test_dependency_allowlists_have_negative_controls(self):
        needed = " (NEEDED) Shared library: [libc.so.6]"
        package.elf_audit(needed, "GLIBC_2.35 GLIBC_2.2.5")
        for dynamic, symbols in [(needed, "GLIBC_2.36"), (needed, "GLIBC_ABI_DT_RELR"),
                                 (needed, "GLIBC_PRIVATE"),
                                 (needed + "\n (RELR) relocations", "GLIBC_2.35"),
                                 (needed + "\n (NEEDED) Shared library: [libdav1d.so.7]", "GLIBC_2.35"),
                                 (needed + "\n (NEEDED) Shared library: [libstdc++.so.6]", "GLIBC_2.35"),
                                 (needed + "\n (RUNPATH) [/build/lib]", "GLIBC_2.35")]:
            with self.assertRaises(ValueError):
                package.elf_audit(dynamic, symbols)
        package.pe_audit("    KERNEL32.dll\n    bcrypt.dll\n")
        for library in ["dav1d.dll", "VCRUNTIME140.dll", "MSVCP140.dll", "ucrtbase.dll", "unknown.dll"]:
            with self.assertRaises(ValueError):
                package.pe_audit("    KERNEL32.dll\n    " + library + "\n")
        loader = "cmd LC_LOAD_DYLINKER\ncmdsize 32\nname /usr/lib/dyld\n"
        package.macho_audit("binary:\n /usr/lib/libSystem.B.dylib (version)\n", loader + "minos 15.0", TARGET)
        for libraries, commands in [
                ("binary:\n /opt/homebrew/lib/libdav1d.dylib (version)", "minos 15.0"),
                ("binary:\n @rpath/libdav1d.dylib (version)", "minos 15.0"),
                ("binary:\n /usr/lib/libSystem.B.dylib (version)", "minos 16.0"),
                ("binary:\n /usr/lib/libSystem.B.dylib (version)", "minos 15.0\nLC_RPATH")]:
            with self.assertRaises(ValueError):
                package.macho_audit(libraries, loader + commands, TARGET)
        for commands in ["minos 15.0", loader.replace("/usr/lib/dyld", "/opt/homebrew/dyld") + "minos 15.0",
                         loader + "minos 15.0\nLC_DYLD_ENVIRONMENT"]:
            with self.assertRaisesRegex(ValueError, "loader/environment"):
                package.macho_audit("binary:\n /usr/lib/libSystem.B.dylib (version)", commands, TARGET)

    def test_architecture_reads_elf_pe_and_both_macho_headers(self):
        binary = self.root / "binary"
        data = bytearray(64)
        data[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", data, 18, 62)
        binary.write_bytes(data)
        package.architecture(binary, "x86_64-unknown-linux-gnu")
        struct.pack_into("<H", data, 18, 183)
        binary.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "architecture"):
            package.architecture(binary, "x86_64-unknown-linux-gnu")
        for target, cpu in [("aarch64-apple-darwin", 0x100000C), ("x86_64-apple-darwin", 0x1000007)]:
            binary.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", cpu) + bytes(56))
            package.architecture(binary, target)
            other = "x86_64-apple-darwin" if target.startswith("aarch64") else "aarch64-apple-darwin"
            with self.assertRaisesRegex(ValueError, "architecture"):
                package.architecture(binary, other)
        data = bytearray(128)
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 64)
        data[64:70] = b"PE\0\0\x64\x86"
        binary.write_bytes(data)
        package.architecture(binary, "x86_64-pc-windows-msvc")
        data[68:70] = b"\x4c\x01"
        binary.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "architecture"):
            package.architecture(binary, "x86_64-pc-windows-msvc")

    def test_license_inventory_is_transitive_target_release_graph_not_lockfile_names(self):
        source = self.root / "sources"
        packages = []
        for id_, crate in [("root", "tonefit"), ("a", "normal"), ("b", "transitive"),
                           ("c", "build"), ("d", "dev-feature-only"), ("x", "dev-only")]:
            path = source / crate
            path.mkdir(parents=True)
            (path / "LICENSE").write_text("copyright 中文丁 " + crate, encoding="utf-8")
            (path / "Cargo.toml").write_text("# upstream package metadata", encoding="utf-8")
            packages.append({"id": id_, "name": crate, "version": "1.0.0", "source": "registry+locked",
                             "manifest_path": str(path / "Cargo.toml"), "license": "MIT", "license_file": None})
        def edge(id_, kind=None):
            return {"pkg": id_, "dep_kinds": [{"kind": kind}]}
        nodes = [{"id": "root", "deps": [edge("a"), edge("c", "build"), edge("x", "dev")]},
                 {"id": "a", "deps": [edge("b"), edge("d")]},
                 *[{"id": p, "deps": []} for p in ["b", "c", "d", "x"]]]
        metadata = json.dumps({"packages": packages, "resolve": {"nodes": nodes}})
        messages = self.root / "build.json"
        messages.write_text("\n".join(json.dumps({"reason": "compiler-artifact", "package_id": p})
                                      for p in ["root", "a", "b", "c", "x"]), encoding="utf-8")
        native_work = self.root / "native"
        for relative in ["COPYING", "src/ext/x86/x86inc.asm"]:
            dest = native_work / package.native.ROOT / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text("native copyright and license", encoding="utf-8")
        sysroot = self.root / "sysroot"
        docs = sysroot / "share/doc/rust"
        (docs / "licenses").mkdir(parents=True)
        (docs / "COPYRIGHT-library.html").write_text("stdlib copyrights", encoding="utf-8")
        (docs / "licenses/MIT.txt").write_text("MIT", encoding="utf-8")
        stage = self.root / "inventory"
        with mock.patch.object(package, "text", side_effect=[metadata, str(sysroot)]):
            package.licenses(stage, native_work, "aarch64-apple-darwin", messages)
        inventory = json.loads((stage / "dependency-licenses.json").read_text(encoding="utf-8"))
        self.assertEqual({p["name"] for p in inventory}, {"normal", "transitive", "build"})
        self.assertEqual((stage / "licenses/rust/transitive-1.0.0/LICENSE").read_text(encoding="utf-8"), "copyright 中文丁 transitive")
        # A label alone is never accepted as the missing transitive license text.
        (source / "transitive/LICENSE").unlink()
        with mock.patch.object(package, "text", side_effect=[metadata, str(sysroot)]):
            with self.assertRaisesRegex(ValueError, "missing upstream license texts: transitive"):
                package.licenses(self.root / "invalid-inventory", native_work, "aarch64-apple-darwin", messages)

    def test_release_artifact_rejects_debug_non_tui_test_or_standby_binary(self):
        binary = self.root / "tonefit"
        artifact = {"reason": "compiler-artifact", "executable": str(binary),
                    "target": {"name": "tonefit", "kind": ["bin"]}, "features": ["default", "tui"],
                    "profile": {"opt_level": "3", "test": False, "debug_assertions": False}}
        self.assertEqual(package.release_artifact(binary, [artifact]), artifact)
        for mutate in [lambda a: a.update(features=[]), lambda a: a["profile"].update(test=True),
                       lambda a: a["profile"].update(opt_level="0"),
                       lambda a: a["profile"].update(debug_assertions=True),
                       lambda a: a.update(executable=str(self.root / "standby"))]:
            changed = copy.deepcopy(artifact)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "default-TUI release"):
                package.release_artifact(binary, [changed])
        with self.assertRaises(ValueError):
            package.release_artifact(binary, [])

    def test_checksum_negative_control(self):
        path = self.root / "archive"
        path.write_bytes(b"final archive")
        expected = package.digest(path)
        self.assertEqual(expected, hashlib.sha256(b"final archive").hexdigest())
        path.write_bytes(b"tampered archive")
        self.assertNotEqual(package.digest(path), expected)
        with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
            package.verify_archive(path, expected, TARGET, VERSION, self.root, self.root / "no-write")
        self.assertFalse((self.root / "no-write").exists())

    def test_cli_checksum_rejects_tampered_archive_before_extraction(self):
        machine = package.native.platform.machine().lower()
        host = (package.native.platform.system(), {"aarch64": "arm64", "amd64": "x86_64"}.get(machine, machine))
        target = next(t for t, value in package.native.TARGETS.items() if value == host)
        source = self.root / "tampered.tar.gz"
        source.write_bytes(b"not the verified archive")
        result = subprocess.run([sys.executable, str(HELPER), "--target", target,
                                 "--verify-archive", str(source), "--sha256", "0" * 64,
                                 "--fixtures", str(self.root), "--output", str(self.root / "out")],
                                text=True, capture_output=True, encoding="utf-8")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA256 mismatch", result.stderr)
        self.assertFalse((self.root / "out").exists())

    def test_cli_rejects_wrong_target_without_creating_output(self):
        result = subprocess.run([sys.executable, str(HELPER), "--target", "arm64-linux"],
                                text=True, capture_output=True, encoding="utf-8")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid choice", result.stderr)


if __name__ == "__main__":
    unittest.main()
