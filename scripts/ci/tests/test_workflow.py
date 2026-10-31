"""Parse the actual workflow and pin its security/quality contract."""
import copy
from pathlib import Path
import re
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[3]


def validate(workflow):
    assert workflow["permissions"] == {"contents": "read"}
    assert set(workflow["on"]) == {"push", "pull_request", "workflow_dispatch"}
    assert workflow["on"]["push"]["branches"] == ["**"]
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
        assert 0 < int(job["timeout-minutes"]) <= 90
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
    commands = "\n".join(runs)
    for required in ["cargo run --locked --quiet --package xtask -- gate",
                     "cargo run --locked --quiet --package xtask -- polish", "npm ci", "npm run check",
                     "npm test", "python -m unittest discover", "scripts/ci/native.py build"]:
        assert required in commands
    guard = jobs["quality-success"]
    assert guard["needs"] == ["native-quality"]
    assert guard["if"] == "always()"
    assert guard["steps"][0]["env"]["QUALITY_RESULT"] == "${{ needs.native-quality.result }}"
    assert guard["steps"][0]["run"] == 'test "$QUALITY_RESULT" = success'


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        # BaseLoader deliberately keeps GitHub's `on` and boolean-like values as strings.
        self.workflow = yaml.load((ROOT / ".github/workflows/quality.yml").read_text(), Loader=yaml.BaseLoader)

    def test_actual_workflow_contract(self):
        validate(self.workflow)

    def test_guard_detects_privileged_or_incomplete_workflow(self):
        for mutate in [
            lambda w: w.update(permissions={"contents": "write"}),
            lambda w: w["jobs"]["native-quality"]["strategy"].update({"fail-fast": "true"}),
            lambda w: w["jobs"]["quality-success"].update({"if": "success()"}),
            lambda w: w["jobs"]["native-quality"]["steps"][0].update({"uses": "actions/checkout@v4"}),
        ]:
            changed = copy.deepcopy(self.workflow)
            mutate(changed)
            with self.assertRaises(AssertionError):
                validate(changed)

    def test_xtask_locks_all_dependency_resolving_steps(self):
        source = (ROOT / "xtask/src/main.rs").read_text()
        arguments = re.findall(r'args: &\[(.*?)\]', source, flags=re.DOTALL)
        self.assertEqual(len(arguments), 7)
        for args in arguments:
            if '"fmt"' not in args:
                self.assertIn('"--locked"', args)


if __name__ == "__main__":
    unittest.main()
