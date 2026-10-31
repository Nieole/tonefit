"""Exercise provisioning inputs without network access or a native compiler."""
import hashlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

HELPER = Path(__file__).resolve().parents[1] / "native.py"
spec = importlib.util.spec_from_file_location("native", HELPER)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class NativeInputTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

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
                                 "--github-env", str(env)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA256 mismatch", result.stderr)
        self.assertFalse((work / native.ROOT).exists())
        self.assertFalse(env.exists())

    def test_cli_rejects_incorrect_native_target(self):
        target = next(t for t, h in native.TARGETS.items() if h[0] != native.platform.system())
        result = subprocess.run([sys.executable, str(HELPER), "host", "--target", target],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requires", result.stderr)


if __name__ == "__main__":
    unittest.main()
