"""Parse the actual workflow and pin its security/quality contract."""
import ast
import copy
from pathlib import Path
import re
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[3]


def validate(workflow):
    assert workflow["permissions"] == {"contents": "read"}
    assert set(workflow["on"]) == {"push", "pull_request", "workflow_dispatch", "workflow_call"}
    assert workflow["on"]["push"]["branches"] == ["**"]
    assert workflow["on"]["workflow_call"]["inputs"]["commit"]["required"] == "true"
    jobs = workflow["jobs"]
    assert set(jobs) == {"native-quality", "quality-success"}
    quality = jobs["native-quality"]
    assert quality["strategy"]["fail-fast"] == "false"
    assert {(x["runner"], x["target"]) for x in quality["strategy"]["matrix"]["include"]} == {
        ("ubuntu-22.04", "x86_64-unknown-linux-gnu"),
        ("windows-2022", "x86_64-pc-windows-msvc"),
        ("macos-15", "aarch64-apple-darwin"),
        ("macos-15-intel", "x86_64-apple-darwin"),
    }
    assert len(quality["strategy"]["matrix"]["include"]) == 4
    assert quality["env"]["RUSTUP_TOOLCHAIN"] == "1.98.0"
    assert quality["env"]["CARGO_INCREMENTAL"] == "0"
    runs = []
    for job in jobs.values():
        assert 0 < int(job["timeout-minutes"]) <= (180 if job is quality else 5)
        assert "permissions" not in job
        assert "continue-on-error" not in job
        for step in job["steps"]:
            assert "continue-on-error" not in step
            if "uses" in step:
                assert re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", step["uses"])
                if step["uses"].startswith("actions/checkout@"):
                    assert step["with"]["persist-credentials"] == "false"
                if step["uses"].startswith("actions/setup-node@"):
                    assert step["with"]["node-version"] == "24.22.2"
            if "run" in step and job is quality:
                runs.append(step["run"])
                if step.get("name") in {"Three build gates", "Polish", "Design acceptance",
                                        "Helper and workflow contract tests", "Static dav1d",
                                        "Record native provenance"}:
                    assert "if" not in step
    steps = quality["steps"]
    assert steps[0]["with"]["ref"] == "${{ inputs.commit || github.sha }}"
    assert steps[0]["with"]["fetch-depth"] == "0"
    assert steps[1]["env"]["EXPECTED_SHA"] == "${{ inputs.commit || github.sha }}"
    assert 'git rev-parse HEAD' in steps[1]["run"]
    proof_name = "Native Windows safe-prefix guard proof"
    stop_name = "Native Windows stop preflight"
    preflight = ["Static dav1d", "Record native provenance", proof_name, stop_name, "Three build gates"]
    positions = [next(i for i, s in enumerate(steps) if s.get("name") == n) for n in preflight]
    assert positions == sorted(positions)
    for name, command in [
        (proof_name, 'python scripts/ci/windows_prefix_proof.py --logs "$RUNNER_TEMP/windows-prefix-proof"'),
        (stop_name, "cargo test --locked --test stop"),
    ]:
        selected = [s for s in steps if s.get("name") == name]
        assert len(selected) == 1
        assert selected[0]["if"] == "runner.os == 'Windows'"
        assert selected[0]["run"] == command
        assert "env" not in selected[0]
    delivery = ["Three build gates", "Polish", "Design acceptance", "Release binary",
                "Deterministic package fixtures", "Package, audit and extracted-archive smoke",
                "Upload verified package and audit"]
    indices = [next(i for i, s in enumerate(steps) if s.get("name") == n) for n in delivery]
    assert indices == sorted(indices)
    for i in indices:
        assert "if" not in steps[i]
    upload = steps[indices[-1]]
    assert upload["uses"] == "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
    assert upload["with"]["if-no-files-found"] == "error"
    assert upload["with"]["path"] == "dist/*"
    assert upload["with"]["name"] == "tonefit-${{ matrix.target }}-${{ inputs.commit || github.sha }}"
    assert upload["with"]["retention-days"] == "7"
    commands = "\n".join(runs)
    for required in ["cargo run --locked --quiet --package xtask -- gate",
                     "cargo run --locked --quiet --package xtask -- polish", "npm ci", "npm run check",
                     "npm test", "python -m unittest discover", "scripts/ci/native.py build",
                     "cargo build --locked --release --target", "cargo test --locked --test package",
                     "scripts/ci/package.py --target", "--fixtures", "--native-work", "--build-messages",
                     "--message-format=json-render-diagnostics"]:
        assert required in commands
    guard = jobs["quality-success"]
    assert guard["needs"] == ["native-quality"]
    assert guard["if"] == "always()"
    assert guard["steps"][0]["env"]["QUALITY_RESULT"] == "${{ needs.native-quality.result }}"
    assert guard["steps"][0]["run"] == 'test "$QUALITY_RESULT" = success'


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        # BaseLoader deliberately keeps GitHub's `on` and boolean-like values as strings.
        self.workflow = yaml.load((ROOT / ".github/workflows/quality.yml").read_text(encoding="utf-8"), Loader=yaml.BaseLoader)

    def test_actual_workflow_contract(self):
        validate(self.workflow)

    def test_guard_detects_privileged_or_incomplete_workflow(self):
        for mutate in [
            lambda w: w.update(permissions={"contents": "write"}),
            lambda w: w["jobs"]["native-quality"]["strategy"].update({"fail-fast": "true"}),
            lambda w: w["jobs"]["quality-success"].update({"if": "success()"}),
            lambda w: w["jobs"]["native-quality"]["steps"][0].update({"uses": "actions/checkout@v4"}),
            lambda w: w["jobs"]["native-quality"]["steps"][-1].update({"if": "always()"}),
            lambda w: w["jobs"]["native-quality"]["steps"].pop(-2),
        ]:
            changed = copy.deepcopy(self.workflow)
            mutate(changed)
            with self.assertRaises((AssertionError, StopIteration)):
                validate(changed)

    def test_windows_proof_cannot_move_after_gates_or_be_weakened(self):
        def step(w, name):
            return next(s for s in w["jobs"]["native-quality"]["steps"] if s.get("name") == name)
        for name in ["Native Windows safe-prefix guard proof", "Native Windows stop preflight"]:
            for field, value in [("if", "always()"), ("run", "echo skipped"),
                                 ("continue-on-error", "true"), ("env", {"RUSTUP_TOOLCHAIN": "stable"})]:
                changed = copy.deepcopy(self.workflow)
                step(changed, name)[field] = value
                with self.assertRaises(AssertionError):
                    validate(changed)
            changed = copy.deepcopy(self.workflow)
            steps = changed["jobs"]["native-quality"]["steps"]
            selected = step(changed, name)
            steps.remove(selected)
            steps.append(selected)
            with self.assertRaises(AssertionError):
                validate(changed)

    def test_windows_stop_helper_pins_the_native_linker(self):
        # Git Bash's GNU link.exe shadowed MSVC in the first real Windows stop
        # run; the helper must name the linker exported by the native step.
        def validate_helper(source):
            assert "CC_LD" in source
            assert '"linker=' in source
            assert "VCToolsInstallDir" in source
        source = (ROOT / "tests/fixtures/windows_stop.rs").read_text(encoding="utf-8")
        validate_helper(source)
        for broken in [source.replace("CC_LD", "PATH"),
                       source.replace('"linker=', '"unused='),
                       source.replace("VCToolsInstallDir", "PATH")]:
            with self.assertRaises(AssertionError):
                validate_helper(broken)

    def test_helpers_never_decode_utf8_using_the_host_locale(self):
        # Include tests: the first Windows CI failed reading Chinese Rust comments
        # before native compilation. Also guard future JSON/tool-output call sites.
        for path in (ROOT / "scripts/ci").rglob("*.py"):
            source = path.read_text(encoding="utf-8")
            for node in ast.walk(ast.parse(source)):
                if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
                    continue
                keywords = {k.arg: k.value for k in node.keywords}
                if (node.func.attr in {"read_text", "write_text"}
                        or node.func.attr in {"run", "check_output"} and "text" in keywords):
                    with self.subTest(path=path, line=node.lineno):
                        self.assertIn("encoding", keywords)
                        self.assertEqual(ast.literal_eval(keywords["encoding"]), "utf-8")

    def test_xtask_locks_all_dependency_resolving_steps(self):
        source = (ROOT / "xtask/src/main.rs").read_text(encoding="utf-8")
        arguments = re.findall(r'args: &\[(.*?)\]', source, flags=re.DOTALL)
        self.assertEqual(len(arguments), 7)
        for args in arguments:
            if '"fmt"' not in args:
                self.assertIn('"--locked"', args)


if __name__ == "__main__":
    unittest.main()
