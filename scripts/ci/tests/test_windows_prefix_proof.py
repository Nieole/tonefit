"""Fake process adapters prove orchestration only, never native Windows behavior."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("windows_prefix_proof", Path(__file__).resolve().parents[1] / "windows_prefix_proof.py")
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


def output(name, mutant=False):
    state = "FAILED" if mutant else "ok"
    return (f"running 1 test\ntest {name} ... {state}\n"
            + (f"thread '{name}' panicked at src/place.rs:600: assertion failed\n" if mutant else "")
            + f"test result: {state}. {0 if mutant else 1} passed; {1 if mutant else 0} failed; 0 ignored; 0 measured; 258 filtered out; finished in 0.00s\n")


class PrefixProofTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "src/place.rs"
        self.source.parent.mkdir()
        self.original = b"// UTF-8 \xe5\xba\x93\r\n" + proof.GUARD + b"\r\n}\r\n"
        self.source.write_bytes(self.original)
        self.logs = self.root / "logs"
        self.calls = []
        self.bad = None

    def run_fake(self, args, root):
        self.assertEqual(root, self.root)
        self.calls.append(args)
        if args[0] == "git":
            return subprocess.CompletedProcess(args, 0, "")
        self.assertEqual(args[:4], ["cargo", "test", "--locked", "--lib"])
        self.assertEqual(args[5:], ["--", "--exact", "--nocapture", "--color", "never"])
        mutant = proof.MUTANT in self.source.read_bytes()
        if mutant:
            self.assertEqual(args[4], proof.NEGATIVE)
            if isinstance(self.bad, Exception):
                raise self.bad
            if self.bad is not None:
                return subprocess.CompletedProcess(args, *self.bad)
        return subprocess.CompletedProcess(args, 101 if mutant else 0, output(args[4], mutant))

    def test_green_red_restore_green_exact_tables(self):
        proof.prove(self.root, self.logs, self.run_fake)
        self.assertEqual(self.source.read_bytes(), self.original)
        tables = [a[4] for a in self.calls if a[0] == "cargo"]
        self.assertEqual(tables, [proof.POSITIVE, proof.NEGATIVE, proof.NEGATIVE, proof.POSITIVE, proof.NEGATIVE])
        self.assertEqual(len(list(self.logs.glob("*.log"))), 5)
        hashes = (self.logs / "source-sha256.txt").read_text(encoding="utf-8").splitlines()
        self.assertEqual(hashes[0].split()[1], hashes[1].split()[1])

    def test_wrong_failures_and_exceptions_restore_bytes_and_reject(self):
        for bad in [(101, "error: could not compile"), (1, "infrastructure failure"),
                    (0, output(proof.NEGATIVE)), (101, output(proof.POSITIVE, True)),
                    (101, output(proof.NEGATIVE, True).replace("0 passed; 1 failed", "0 passed; 2 failed")),
                    (101, output(proof.NEGATIVE, True).replace("0 ignored", "1 ignored")),
                    OSError("process launch failed")]:
            with self.subTest(bad=bad):
                self.bad = bad
                self.calls = []
                with self.assertRaises((RuntimeError, OSError)):
                    proof.prove(self.root, self.logs, self.run_fake)
                self.assertEqual(self.source.read_bytes(), self.original)
                self.assertEqual(self.calls[-1][0], "git")
                self.assertEqual(len([a for a in self.calls if a[0] == "cargo"]), 3)

    def test_missing_or_duplicate_guard_fails_without_editing(self):
        for content in [b"renamed guard", self.original * 2]:
            self.source.write_bytes(content)
            with self.assertRaisesRegex(RuntimeError, "coupling"):
                proof.prove(self.root, self.logs, self.run_fake)
            self.assertEqual(self.source.read_bytes(), content)

    def test_dirty_tracked_source_rejected(self):
        def dirty(args, root):
            return subprocess.CompletedProcess(args, 0, " M src/place.rs\n")
        with self.assertRaisesRegex(RuntimeError, "clean tracked"):
            proof.prove(self.root, self.logs, dirty)
        self.assertEqual(self.source.read_bytes(), self.original)

    def test_initial_green_failure_restores_and_rejects(self):
        def fail(args, root):
            if args[0] == "git":
                return self.run_fake(args, root)
            return subprocess.CompletedProcess(args, 101, "compile error")
        with self.assertRaises(RuntimeError):
            proof.prove(self.root, self.logs, fail)
        self.assertEqual(self.source.read_bytes(), self.original)

    def test_host_check_rejects_non_windows_without_processes(self):
        with patch.object(proof.platform, "system", return_value="Darwin"):
            with self.assertRaisesRegex(RuntimeError, "actual native Windows"):
                proof.verify_host(self.root, lambda *args: self.fail("must reject before Cargo"))

    def test_actual_process_adapter_uses_utf8_and_colorless_output(self):
        with patch.object(proof.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "库")) as run:
            proof.execute(["cargo", "-V"], self.root)
        self.assertEqual(run.call_args.kwargs["encoding"], "utf-8")
        self.assertEqual(run.call_args.kwargs["env"]["CARGO_TERM_COLOR"], "never")


if __name__ == "__main__":
    unittest.main()
