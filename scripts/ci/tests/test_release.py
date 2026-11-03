"""Offline public-seam checks: untrusted archives, Git command adapter, REST adapter."""
import contextlib
import copy
import http.server
import hashlib
import io
import json
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import threading
from types import SimpleNamespace
import urllib.error
import unittest
from unittest.mock import patch
import zipfile

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release

SHA = "a" * 40
VERSION = "0.1.0-alpha.1"
TAG = "v" + VERSION
REPO = "Nieole/tonefit"


class GitAdapter(release.Git):
    def __init__(self):
        self.remote = SHA
        self.ancestor = True
        self.fetches = 0
        self.change_after_stage = False

    def run(self, *args):
        if args[0] == "fetch":
            assert args[1:] == ("--no-tags", "origin", "+refs/heads/main:refs/remotes/origin/main",
                               f"+refs/tags/{TAG}:refs/remotes/origin/release-candidate")
            self.fetches += 1
            if self.change_after_stage and self.fetches > 1:
                self.remote = "b" * 40
            return ""
        if args[0] == "merge-base":
            assert args == ("merge-base", "--is-ancestor", SHA, "refs/remotes/origin/main")
            if not self.ancestor:
                raise subprocess.CalledProcessError(1, args)
            return ""
        if args == ("rev-parse", "--verify", "refs/remotes/origin/release-candidate^{commit}"):
            return self.remote
        if args == ("rev-parse", "HEAD"):
            return SHA
        if args == ("status", "--porcelain", "--untracked-files=no"):
            return ""
        raise AssertionError(args)


class RestAdapter:
    """Models remote state, not publisher internals. Every call goes through request()."""
    def __init__(self):
        self.release = None
        self.other_releases = []
        self.creations = 0
        self.entries = []
        self.bytes = {}
        self.calls = []
        self.fail_upload = None
        self.corrupt = False
        self.publishes = 0

    def request(self, method, path, data=None, binary=False, upload=False):
        self.calls.append((method, path))
        if path.startswith("/releases/tags/"):
            # GitHub documents this endpoint as published-only. Drafts are 404.
            return copy.deepcopy(self.release) if self.release and not self.release["draft"] else None
        if method == "GET" and path.startswith("/releases?per_page=100&page="):
            entries = self.other_releases + ([self.release] if self.release else [])
            page = int(path.rsplit("=", 1)[1])
            return copy.deepcopy(entries[(page - 1) * 100:page * 100])
        if method == "POST" and path == "/releases":
            # Real GitHub: target_commitish takes a commit SHA or branch name;
            # `refs/tags/<tag>` is a 422 Validation Failed (the first real stage run).
            commitish = data.get("target_commitish", "")
            if commitish.startswith("refs/") or not re.fullmatch(r"[0-9a-f]{40}", commitish):
                raise urllib.error.HTTPError(path, 422, "Unprocessable Entity", {}, None)
            self.creations += 1
            self.release = dict(data, id=1, author={"login": "github-actions[bot]"})
            return copy.deepcopy(self.release)
        if method == "POST" and "/assets?name=" in path:
            name = path.split("name=")[1]
            if name == self.fail_upload:
                raise OSError("upload transport failed")
            asset = {"id": len(self.entries) + 10, "name": name, "size": len(data), "state": "uploaded"}
            self.entries.append(asset)
            self.bytes[asset["id"]] = data
            return copy.deepcopy(asset)
        if method == "GET" and path.startswith("/releases/1/assets?"):
            # Five assets fit on the first page; release-list pagination is tested separately.
            return copy.deepcopy(self.entries) if path.endswith("&page=1") else []
        if method == "GET" and path.startswith("/releases/assets/"):
            return b"tamper" if self.corrupt else self.bytes[int(path.rsplit("/", 1)[1])]
        if method == "GET" and path == "/releases/1":
            return copy.deepcopy(self.release)
        if method == "PATCH" and path == "/releases/1":
            self.publishes += 1
            self.release.update(data)
            return copy.deepcopy(self.release)
        raise AssertionError((method, path))


def final_files():
    files = {release.archive_name(t, VERSION): ("final-" + t).encode("utf-8") for t in release.native.TARGETS}
    files["SHA256SUMS"] = "".join(f"{hashlib.sha256(v).hexdigest()}  {n}\n" for n, v in sorted(files.items())).encode("utf-8")
    return files


class ReleaseTests(unittest.TestCase):
    def test_identity_and_ancestry(self):
        release.identity(TAG, SHA, VERSION)
        for tag, sha, version in [("v0.1.0", SHA, "0.1.0"), ("v0.1.0-preview.1", SHA, VERSION),
                                  ("v0.1.0-alpha.01", SHA, VERSION), (TAG, "aaa", VERSION),
                                  (TAG, SHA, "0.1.0"), ("v00.1.0-alpha.1", SHA, VERSION),
                                  ("v0.1.0-alpha.1;echo bad", SHA, VERSION)]:
            with self.assertRaises(ValueError):
                release.identity(tag, sha, version)
        for phase in ["alpha", "beta", "rc"]:
            release.identity("v1.2.3-" + phase + ".2", SHA, "1.2.3-" + phase + ".2")
        git = GitAdapter()
        git.guard(TAG, SHA, VERSION)
        git.ancestor = False
        with self.assertRaises(subprocess.CalledProcessError):
            git.guard(TAG, SHA, VERSION)
        git.ancestor = True
        git.remote = "b" * 40
        with self.assertRaisesRegex(ValueError, "remote tag"):
            git.guard(TAG, SHA, VERSION)

    def publish(self, api, git=None):
        release.publish(api, git or GitAdapter(), final_files(), TAG, SHA, VERSION, REPO)

    def test_complete_roundtrip_publishes_once_and_rerun_never_replaces(self):
        api = RestAdapter()
        self.publish(api)
        self.assertEqual(api.publishes, 1)
        self.assertEqual(len(api.entries), 5)
        self.assertEqual(api.release["target_commitish"], SHA)
        self.assertEqual(sum(1 for m, p in api.calls if m == "GET" and p.startswith("/releases/assets/")), 5)
        before = copy.deepcopy(api.entries)
        with self.assertRaisesRegex(ValueError, "published or unknown"):
            self.publish(api)
        self.assertEqual(api.entries, before)
        self.assertEqual(api.publishes, 1)
        self.assertFalse(any(m in {"DELETE", "PUT"} for m, _ in api.calls))

    def test_partial_upload_stays_draft_and_owned_rerun_resumes_identical_bytes(self):
        api = RestAdapter()
        api.fail_upload = sorted(final_files())[2]
        with self.assertRaises(OSError):
            self.publish(api)
        self.assertTrue(api.release["draft"])
        self.assertEqual(len(api.entries), 2)
        self.assertEqual(api.publishes, 0)
        api.fail_upload = None
        self.publish(api)
        self.assertEqual(len(api.entries), 5)
        self.assertEqual(api.publishes, 1)
        self.assertEqual(api.creations, 1)
        self.assertEqual(api.bytes, {a["id"]: final_files()[a["name"]] for a in api.entries})

    def test_manual_draft_or_unknown_owner_rejected(self):
        for mutation in [{"body": "manual"}, {"author": {"login": "maintainer"}},
                         {"body": "wrong commit marker"}, {"prerelease": False}]:
            api = RestAdapter()
            api.fail_upload = sorted(final_files())[0]
            with self.assertRaises(OSError):
                self.publish(api)
            api.release.update(mutation)
            api.fail_upload = None
            with self.assertRaisesRegex(ValueError, "unknown/manual"):
                self.publish(api)
            self.assertEqual(api.entries, [])
            self.assertEqual(api.creations, 1)
            self.assertEqual(api.publishes, 0)

    def test_release_discovery_paginates_and_rejects_duplicate_tags(self):
        for duplicate in [False, True]:
            with self.subTest(duplicate=duplicate):
                api = RestAdapter()
                api.fail_upload = sorted(final_files())[2]
                with self.assertRaises(OSError):
                    self.publish(api)
                api.other_releases = [dict(id=i + 100, tag_name=f"v-other-{i}", draft=False)
                                      for i in range(100)]
                if duplicate:
                    # Duplicate may be on a different page, published or draft.
                    api.other_releases[0] = dict(api.release, id=99, draft=False)
                api.fail_upload = None
                if duplicate:
                    with self.assertRaisesRegex(ValueError, "duplicate.*release"):
                        self.publish(api)
                    self.assertEqual(api.publishes, 0)
                    self.assertEqual(len(api.entries), 2)
                else:
                    self.publish(api)
                    self.assertIn(("GET", "/releases?per_page=100&page=2"), api.calls)
                    self.assertEqual(api.publishes, 1)
                self.assertEqual(api.creations, 1)
                self.assertFalse(any(p.startswith("/releases/tags/") for _, p in api.calls))

    def test_existing_tag_target_commitish_is_advisory_not_identity(self):
        # Docs: target_commitish is unused when tag exists; examples return main/master.
        for commitish in ["main", "master", SHA]:
            with self.subTest(commitish=commitish):
                api = RestAdapter()
                api.fail_upload = sorted(final_files())[0]
                with self.assertRaises(OSError):
                    self.publish(api)
                api.release["target_commitish"] = commitish
                api.fail_upload = None
                git = GitAdapter()
                self.publish(api, git)
                self.assertEqual(api.creations, 1)
                self.assertEqual(git.fetches, 2)
                self.assertEqual(api.publishes, 1)

    def test_last_release_state_guard_blocks_visibility_patch(self):
        class ChangedRelease(RestAdapter):
            def request(self, method, path, *args, **kwargs):
                result = super().request(method, path, *args, **kwargs)
                if method == "GET" and path == "/releases/1":
                    result["body"] = "changed externally after uploads"
                return result
        api = ChangedRelease()
        with self.assertRaisesRegex(ValueError, "unknown/manual"):
            self.publish(api)
        self.assertEqual(len(api.entries), 5)
        self.assertEqual(sum(m == "GET" and p.startswith("/releases/assets/") for m, p in api.calls), 5)
        self.assertEqual(api.publishes, 0)

    def test_changed_remote_tag_before_publish_or_corrupt_roundtrip_stays_draft(self):
        for corrupt in [False, True]:
            api, git = RestAdapter(), GitAdapter()
            api.corrupt = corrupt
            git.change_after_stage = not corrupt
            with self.assertRaisesRegex(ValueError, "roundtrip|remote tag"):
                self.publish(api, git)
            self.assertTrue(api.release["draft"])
            self.assertEqual(api.publishes, 0)

    def test_asset_listing_wrong_size_extra_duplicate_and_missing_rejected(self):
        for kind in ["size", "extra", "duplicate", "missing"]:
            api = RestAdapter()
            api.fail_upload = sorted(final_files())[-1]
            with self.assertRaises(OSError):
                self.publish(api)
            if kind == "size":
                api.entries[0]["size"] += 1
            elif kind == "extra":
                api.entries[0]["name"] = "extra"
            elif kind == "duplicate":
                api.entries.append(copy.deepcopy(api.entries[0]))
            else:
                api.corrupt = True
            api.fail_upload = None
            with self.assertRaises(ValueError):
                self.publish(api)
            self.assertEqual(api.publishes, 0)

    def test_pagination_reads_all_pages(self):
        class Paginated:
            def request(self, method, path):
                return list(range(100)) if path.endswith("&page=1") else [100]
        self.assertEqual(len(release.pages(Paginated(), "/assets")), 101)

    def fixture(self, root):
        artifacts = root / "artifacts"
        artifacts.mkdir()
        for target in release.native.TARGETS:
            directory = artifacts / release.artifact_name(target, SHA)
            directory.mkdir()
            stage = root / release.package.name(target, VERSION)
            stage.mkdir()
            required = ["LICENSE", "THIRD-PARTY-NOTICES.md", "README.txt", "licenses/UnRAR.txt",
                        "licenses/OFL-1.1.txt", "licenses/dav1d-BSD-2-Clause.txt", "licenses/dav1d-x86-ISC.asm",
                        "licenses/rust-stdlib/COPYRIGHT-library.html", "licenses/GCC-runtime-copyright.txt", "licenses/GPL-3.txt"]
            required += ["licenses/rust-stdlib/licenses/" + n for n in
                         ["MIT.txt", "Apache-2.0.txt", "LLVM-exception.txt", "GCC-exception-3.1.txt"]]
            for name in required:
                path = stage / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("license fixture", encoding="utf-8")
            provenance = dict(target=target, version=VERSION, commit=SHA, dirty=False, tags=[TAG],
                              cargo_lock_sha256=release.package.digest(release.package.ROOT / "Cargo.lock"),
                              dav1d=release.native.VERSION, source_sha256=release.native.SHA256,
                              release_features=["default", "tui"])
            (stage / "provenance.json").write_text(json.dumps(provenance), encoding="utf-8")
            (stage / "dependency-licenses.json").write_text(json.dumps([{"notices": ["LICENSE"]}]), encoding="utf-8")
            data = bytearray(128)
            if "linux" in target:
                data[:6] = b"\x7fELF\x02\x01"
                struct.pack_into("<H", data, 18, 62)
            elif "windows" in target:
                data[:2] = b"MZ"
                struct.pack_into("<I", data, 60, 64)
                data[64:70] = b"PE\0\0\x64\x86"
            else:
                data[:4] = b"\xcf\xfa\xed\xfe"
                struct.pack_into("<I", data, 4, 0x100000C if target.startswith("aarch64") else 0x1000007)
            binary = stage / ("tonefit.exe" if "windows" in target else "tonefit")
            binary.write_bytes(data)
            binary.chmod(0o755)
            self.manifest(stage)
            source = directory / release.archive_name(target, VERSION)
            self.fixture_archive(stage, source)
            self.report(directory, target)
        return artifacts

    def fixture_archive(self, stage, source, executable_mode=0o755):
        # Synthetic Unix archives must encode Unix modes even when built on NTFS.
        if source.suffix == ".zip":
            release.package.archive(stage, source)
        else:
            def modes(member):
                member.mode = executable_mode if member.name == stage.name + "/tonefit" else (0o755 if member.isdir() else 0o644)
                return member
            with release.package.tarfile.open(source, "w:gz") as out:
                out.add(stage, arcname=stage.name, filter=modes)

    def test_collect_checks_tar_modes_without_unix_filesystem_bits(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifacts = self.fixture(root)
            # Model only package's host capability, without changing pathlib's OS.
            with patch.object(release.package, "os", SimpleNamespace(name="nt")):
                release.collect(artifacts, TAG, SHA, VERSION, root / "final")
            self.assertEqual(len(list((root / "final").iterdir())), 5)

    def test_collect_rejects_bad_unix_tar_executable_mode_on_every_host(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifacts = self.fixture(root)
            target = "x86_64-unknown-linux-gnu"
            directory = artifacts / release.artifact_name(target, SHA)
            source = directory / release.archive_name(target, VERSION)
            self.fixture_archive(root / release.package.name(target, VERSION), source, 0o644)
            self.report(directory, target)
            # Model NTFS too: security must come from actual TAR entry metadata.
            with patch.object(release.package, "validate_tree", side_effect=AssertionError("must reject before tree validation")):
                with self.assertRaisesRegex(ValueError, "missing executable permission"):
                    release.collect(artifacts, TAG, SHA, VERSION, root / "final")
            self.assertFalse((root / "final").exists())

    def manifest(self, stage):
        (stage / "manifest.json").write_text(json.dumps({p.relative_to(stage).as_posix(): release.package.digest(p)
            for p in stage.rglob("*") if p.is_file() and p.name != "manifest.json"}), encoding="utf-8")

    def report(self, directory, target):
        source = directory / release.archive_name(target, VERSION)
        digest = release.package.digest(source)
        (directory / "SHA256SUMS").write_text(f"{digest}  {source.name}\n", encoding="utf-8")
        (directory / "verification.json").write_text(json.dumps(dict(archive=source.name, sha256=digest,
            target=target, version=VERSION, dependencies=["system"], smoke="native verified")), encoding="utf-8")

    def test_collect_four_targets_and_offline_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifacts = self.fixture(root)
            subprocess.run([sys.executable, str(Path(release.__file__)), "check", "--tag", TAG, "--sha", SHA,
                            "--artifacts", str(artifacts), "--output", str(root / "final")], check=True)
            self.assertEqual(len(list((root / "final").iterdir())), 5)

    def test_collect_rejects_missing_duplicate_wrong_target_commit_tamper_and_traversal(self):
        for kind in ["missing", "duplicate", "target", "commit", "dirty", "tamper", "traversal", "version", "license"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                artifacts = self.fixture(root)
                target = next(iter(release.native.TARGETS))
                directory = artifacts / release.artifact_name(target, SHA)
                source = directory / release.archive_name(target, VERSION)
                stage = root / release.package.name(target, VERSION)
                if kind == "missing":
                    source.unlink()
                elif kind == "duplicate":
                    (artifacts / "duplicate").mkdir()
                elif kind == "tamper":
                    source.write_bytes(source.read_bytes() + b"tamper")
                elif kind == "traversal":
                    # Build a valid-checksum archive whose internal paths are malicious.
                    if source.suffix == ".zip":
                        with zipfile.ZipFile(source, "w") as archive:
                            archive.writestr("../escape", "bad")
                    else:
                        with release.package.tarfile.open(source, "w:gz") as archive:
                            entry = release.package.tarfile.TarInfo("../escape")
                            entry.size = 3
                            archive.addfile(entry, io.BytesIO(b"bad"))
                    self.report(directory, target)
                else:
                    provenance = json.loads((stage / "provenance.json").read_text(encoding="utf-8"))
                    if kind == "target":
                        provenance["target"] = "wrong"
                    elif kind == "commit":
                        provenance["commit"] = "b" * 40
                    elif kind == "dirty":
                        provenance["dirty"] = True
                    elif kind == "version":
                        provenance["version"] = "0.1.0"
                    else:
                        (stage / "licenses/UnRAR.txt").unlink()
                    (stage / "provenance.json").write_text(json.dumps(provenance), encoding="utf-8")
                    self.manifest(stage)
                    self.fixture_archive(stage, source)
                    self.report(directory, target)
                with self.assertRaises((ValueError, FileNotFoundError, release.package.tarfile.ReadError)):
                    release.collect(artifacts, TAG, SHA, VERSION, root / "final")

    def test_verification_json_identity_and_exact_artifact_file_set(self):
        for mutation in ["target", "version", "sha256", "archive", "extra-file"]:
            with tempfile.TemporaryDirectory() as tmp, self.subTest(mutation=mutation):
                root = Path(tmp)
                artifacts = self.fixture(root)
                directory = next(artifacts.iterdir())
                if mutation == "extra-file":
                    (directory / "extra").write_bytes(b"unexpected")
                else:
                    path = directory / "verification.json"
                    report = json.loads(path.read_text(encoding="utf-8"))
                    report[mutation] = "wrong"
                    path.write_text(json.dumps(report), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "verification identity|unexpected artifact"):
                    release.collect(artifacts, TAG, SHA, VERSION, root / "final")

    def test_download_only_current_run_exact_four_names(self):
        class ArtifactAPI:
            def __init__(self, metadata, artifacts):
                self.metadata, self.artifacts = metadata, artifacts
            def request(self, method, path, **kwargs):
                if path == "/actions/runs/9":
                    return self.metadata
                if path.startswith("/actions/runs/9/artifacts?"):
                    return {"artifacts": self.artifacts}
                raise AssertionError("must reject before download")
        metadata = dict(id=9, head_sha=SHA, event="push", path=".github/workflows/release.yml")
        expected = [dict(name=release.artifact_name(t, SHA), id=i + 1, expired=False) for i, t in enumerate(release.native.TARGETS)]
        for field, value in [("head_sha", "b" * 40), ("event", "pull_request"), ("path", ".github/workflows/quality.yml")]:
            changed = dict(metadata, **{field: value})
            with self.assertRaises(ValueError):
                release.download_artifacts(ArtifactAPI(changed, expected), "9", SHA, Path("unused"))
        for entries in [expected[:-1], expected + [expected[0]], [expected[0]] * 4]:
            with self.assertRaises(ValueError):
                release.download_artifacts(ArtifactAPI(metadata, entries), "9", SHA, Path("unused"))

    def test_artifact_transport_digest_and_zip_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            directories = self.fixture(root)
            class Downloads:
                def __init__(self):
                    self.artifacts = []
                    self.data = {}
                    for i, directory in enumerate(directories.iterdir()):
                        buffer = io.BytesIO()
                        with zipfile.ZipFile(buffer, "w") as archive:
                            for path in directory.iterdir():
                                archive.writestr(path.name, path.read_bytes())
                        data = buffer.getvalue()
                        self.data[i + 1] = data
                        self.artifacts.append(dict(id=i + 1, name=directory.name, expired=False,
                            digest="sha256:" + hashlib.sha256(data).hexdigest()))
                def request(self, method, path, **kwargs):
                    if path == "/actions/runs/9":
                        return dict(id=9, head_sha=SHA, event="push", path=".github/workflows/release.yml")
                    if path.startswith("/actions/runs/9/artifacts?"):
                        return {"artifacts": self.artifacts}
                    if path.startswith("/actions/artifacts/"):
                        return self.data[int(path.split("/")[3])]
                    raise AssertionError(path)
            api = Downloads()
            release.download_artifacts(api, "9", SHA, root / "downloaded")
            release.collect(root / "downloaded", TAG, SHA, VERSION, root / "final")
            # A JSON metadata body must not pass as an artifact, even with a matching digest.
            original = api.data[1]
            api.data[1] = b'{"id": 1, "archive_download_url": "not an archive"}'
            api.artifacts[0]["digest"] = "sha256:" + hashlib.sha256(api.data[1]).hexdigest()
            with self.assertRaises(zipfile.BadZipFile):
                release.download_artifacts(api, "9", SHA, root / "json-metadata")
            api.data[1] = original
            api.artifacts[0]["digest"] = "sha256:" + hashlib.sha256(original).hexdigest()
            api.data[1] += b"tampered"
            with self.assertRaisesRegex(ValueError, "transport digest"):
                release.download_artifacts(api, "9", SHA, root / "bad-digest")
            for entries in [{"../escape": b"bad"}, {"SHA256SUMS": b"bad", "verification.json": b"bad", "extra": b"bad"}]:
                buffer = io.BytesIO()
                with zipfile.ZipFile(buffer, "w") as archive:
                    for name, data in entries.items():
                        archive.writestr(name, data)
                api.data[1] = buffer.getvalue()
                api.artifacts[0]["digest"] = "sha256:" + hashlib.sha256(api.data[1]).hexdigest()
                with tempfile.TemporaryDirectory() as bad, self.assertRaisesRegex(ValueError, "ZIP entries"):
                    release.download_artifacts(api, "9", SHA, Path(bad) / "download")

    def test_real_download_endpoint_accept_headers(self):
        calls = []
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_GET(self):
                accept = self.headers.get("Accept")
                calls.append((self.path, accept))
                expected = ("application/octet-stream" if self.path == "/releases/assets/10"
                            else "application/vnd.github+json")
                status = 415 if accept != expected or self.path == "/unsupported" else 200
                self.send_response(status)
                self.end_headers()
                self.wfile.write(b'actual download bytes' if status == 200 else b'{"message":"Unsupported Accept"}')
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            api = release.API(REPO, "offline-test-token")
            api.base = f"http://127.0.0.1:{server.server_port}"
            for path, accept in [("/actions/artifacts/123/zip", "application/vnd.github+json"),
                                 ("/releases/assets/10", "application/octet-stream")]:
                with self.subTest(path=path):
                    self.assertEqual(api.request("GET", path, binary=True), b"actual download bytes")
                    self.assertEqual(calls[-1], (path, accept))
            with self.assertRaises(urllib.error.HTTPError) as error:
                api.request("GET", "/unsupported", binary=True)
            self.assertEqual(error.exception.code, 415)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_real_rest_transport_checks_status_json_bytes_and_credentials(self):
        calls = []
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_GET(self):
                calls.append((self.path, self.headers.get("Authorization")))
                if self.path == "/ok":
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(b'{"id": 1}')
                elif self.path == "/binary":
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(b"actual bytes")
                else:
                    status = 404 if self.path.startswith("/releases/tags/") else 201 if self.path == "/wrong-status" else 500
                    self.send_response(status)
                    self.end_headers()
                    self.wfile.write(b'{"id": 1}')
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            api = release.API(REPO, "offline-test-token")
            # Real HTTP against a local server, never a real GitHub token or network write.
            api.base = f"http://127.0.0.1:{server.server_port}"
            self.assertEqual(api.request("GET", "/ok"), {"id": 1})
            self.assertEqual(api.request("GET", "/binary", binary=True), b"actual bytes")
            self.assertIsNone(api.request("GET", "/releases/tags/absent"))
            with self.assertRaisesRegex(ValueError, "REST status"):
                api.request("GET", "/wrong-status")
            with self.assertRaises(urllib.error.HTTPError), \
                    contextlib.redirect_stderr(io.StringIO()) as logged:
                api.request("GET", "/failure")
            # The validation body names the rejected field: keep it in the CI log.
            self.assertIn('HTTP 500: {"id": 1}', logged.getvalue())
            self.assertTrue(all(token == "Bearer offline-test-token" for _, token in calls))
            request = release.urllib.request.Request("https://api.github.com/test", headers={"Authorization": "secret"})
            redirected = release.SafeRedirect().redirect_request(request, None, 302, "", {},
                "https://objects.githubusercontent.com/test")
            self.assertIsNone(redirected.get_header("Authorization"))
            azure = release.SafeRedirect().redirect_request(request, None, 302, "", {},
                "https://productionresultssa0.blob.core.windows.net/actions-results/test")
            self.assertIsNone(azure.get_header("Authorization"))
            with self.assertRaisesRegex(ValueError, "redirect"):
                release.SafeRedirect().redirect_request(request, None, 302, "", {}, "https://attacker.example/test")
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_release_workflow_security_and_barrier(self):
        path = release.package.ROOT / ".github/workflows/release.yml"
        workflow = yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
        self.assertEqual(workflow["on"], {"push": {"tags": ["v*-alpha.*", "v*-beta.*", "v*-rc.*"]}})
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertEqual(workflow["concurrency"]["cancel-in-progress"], "false")
        jobs = workflow["jobs"]
        self.assertEqual(set(jobs), {"stage", "quality", "publish"})
        self.assertEqual(jobs["quality"]["uses"], "./.github/workflows/quality.yml")
        self.assertEqual(jobs["quality"]["with"]["commit"], "${{ needs.stage.outputs.sha }}")
        self.assertEqual(jobs["quality"]["needs"], ["stage"])
        self.assertEqual(jobs["publish"]["needs"], ["stage", "quality"])
        self.assertEqual(jobs["publish"]["if"], "${{ !cancelled() && needs.stage.result == 'success' && needs.quality.result == 'success' }}")
        for name, job in jobs.items():
            self.assertNotIn("continue-on-error", job)
            self.assertEqual(job.get("permissions", {"contents": "read"}),
                             {"contents": "write", "actions": "read"} if name == "publish" else {"contents": "read"})
            for step in job.get("steps", []):
                self.assertNotIn("continue-on-error", step)
                if "uses" in step:
                    self.assertRegex(step["uses"], r"@[0-9a-f]{40}$")
                if step.get("uses", "").startswith("actions/checkout@"):
                    self.assertEqual(step["with"]["persist-credentials"], "false")
                if "run" in step:
                    self.assertNotIn("${{", step["run"])


if __name__ == "__main__":
    unittest.main()
