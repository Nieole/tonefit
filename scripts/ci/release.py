#!/usr/bin/env python3
"""Fail-closed stage release. `check` is offline; `stage`/`publish` require CI.

Public seams: Git.guard(), API.request(), collect(), publish(). No binary is executed.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile

import native
import package

STAGE = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)-(alpha|beta|rc)\.([1-9][0-9]*)")
SHA = re.compile(r"[0-9a-f]{40}")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def identity(tag, sha, version):
    require(bool(STAGE.fullmatch(tag)), "not a strict stage tag")
    require(tag[1:] == version, "tag/Cargo version mismatch")
    require(bool(SHA.fullmatch(sha)), "not a full commit SHA")


class Git:
    def __init__(self, directory="."):
        self.directory = directory

    def run(self, *args):
        return subprocess.check_output(["git", "-C", str(self.directory), *args], text=True,
                                       encoding="utf-8", stderr=subprocess.STDOUT).strip()

    def guard(self, tag, sha, version):
        identity(tag, sha, version)
        # Fetch explicit refs, never create/move a tag; no reliance on stale local main.
        self.run("fetch", "--no-tags", "origin", "+refs/heads/main:refs/remotes/origin/main",
                 f"+refs/tags/{tag}:refs/remotes/origin/release-candidate")
        resolved = self.run("rev-parse", "--verify", "refs/remotes/origin/release-candidate^{commit}")
        require(resolved == sha, "remote tag moved/wrong commit")
        self.run("merge-base", "--is-ancestor", sha, "refs/remotes/origin/main")
        require(self.run("rev-parse", "HEAD") == sha, "checkout is not tagged SHA")
        require(self.run("status", "--porcelain", "--untracked-files=no") == "", "dirty source checkout")


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        url = urllib.parse.urlparse(newurl)
        require(url.scheme == "https" and (url.hostname == "github.com" or
                (url.hostname or "").endswith((".githubusercontent.com", ".blob.core.windows.net"))),
                "untrusted download redirect")
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        redirected.remove_header("Authorization")
        return redirected


class API:
    def __init__(self, repo, token):
        require(bool(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo)), "invalid repository")
        self.base = f"https://api.github.com/repos/{repo}"
        self.upload = f"https://uploads.github.com/repos/{repo}"
        self.token = token
        self.opener = urllib.request.build_opener(SafeRedirect())

    def request(self, method, path, data=None, binary=False, upload=False):
        # Raw response bytes do not imply octet-stream negotiation: Actions ZIP
        # redirects require JSON Accept; only Release asset downloads use octet-stream.
        asset_download = binary and method == "GET" and re.fullmatch(r"/releases/assets/[0-9]+", path)
        headers = {"Authorization": "Bearer " + self.token, "X-GitHub-Api-Version": "2022-11-28",
                   "Accept": "application/octet-stream" if asset_download else "application/vnd.github+json"}
        if isinstance(data, dict):
            data = json.dumps(data).encode("utf-8")
            headers["Content-Type"] = "application/json"
        elif data is not None:
            headers["Content-Type"] = "application/octet-stream"
        request = urllib.request.Request((self.upload if upload else self.base) + path,
                                         data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=120) as response:
                status, body = response.status, response.read()
        except urllib.error.HTTPError as error:
            error.close()
            if error.code == 404 and method == "GET" and path.startswith("/releases/tags/"):
                return None
            raise
        require(status == (201 if method == "POST" else 200), "unexpected REST status")
        return body if binary else json.loads(body)


def pages(api, path, key=None):
    result = []
    for page in range(1, 1001):
        response = api.request("GET", f"{path}{'&' if '?' in path else '?'}per_page=100&page={page}")
        items = response[key] if key else response
        require(isinstance(items, list) and len(items) <= 100, "invalid paginated REST response")
        result.extend(items)
        if len(items) < 100:
            return result
    raise ValueError("pagination limit")


def artifact_name(target, sha):
    return f"tonefit-{target}-{sha}"


def archive_name(target, version):
    return package.name(target, version) + (".zip" if "windows" in target else ".tar.gz")


def collect(directories, tag, sha, version, output):
    """Validate exactly four artifact directories; write only final bytes + global sums."""
    identity(tag, sha, version)
    expected = {artifact_name(t, sha): t for t in native.TARGETS}
    require({p.name for p in directories.iterdir()} == set(expected), "missing/extra/duplicate platform artifact")
    files = {}
    for artifact, target in expected.items():
        directory = directories / artifact
        filename = archive_name(target, version)
        require(directory.is_dir() and not directory.is_symlink(), "invalid artifact directory")
        require({p.name for p in directory.iterdir()} == {filename, "SHA256SUMS", "verification.json"},
                "unexpected artifact files")
        require(all(p.is_file() and not p.is_symlink() for p in directory.iterdir()), "invalid artifact file")
        source = directory / filename
        sha256 = package.digest(source)
        require((directory / "SHA256SUMS").read_text(encoding="utf-8") == f"{sha256}  {filename}\n", "checksum mismatch")
        report = json.loads((directory / "verification.json").read_text(encoding="utf-8"))
        require(all(report.get(k) == v for k, v in {"archive": filename, "sha256": sha256,
                "target": target, "version": version}.items()), "wrong verification identity")
        require(bool(report.get("smoke")) and bool(report.get("dependencies")), "missing native audit/smoke")
        with tempfile.TemporaryDirectory(prefix="tonefit-release-") as tmp:
            stage = package.safe_extract(source, Path(tmp) / "extracted", package.name(target, version))
            package.validate_tree(stage, target, version)
            provenance = json.loads((stage / "provenance.json").read_text(encoding="utf-8"))
            require(provenance.get("commit") == sha and provenance.get("dirty") is False,
                    "wrong commit/dirty provenance")
            require(tag in provenance.get("tags", []), "missing stage tag provenance")
            require(provenance.get("cargo_lock_sha256") == package.digest(package.ROOT / "Cargo.lock"), "wrong lock provenance")
            require(provenance.get("dav1d") == native.VERSION and provenance.get("source_sha256") == native.SHA256,
                    "wrong native provenance")
            require(set(provenance.get("release_features", [])) == {"default", "tui"}, "wrong release features")
        data = source.read_bytes()
        require(hashlib.sha256(data).hexdigest() == sha256, "archive changed during validation")
        files[filename] = data
    files["SHA256SUMS"] = "".join(f"{hashlib.sha256(data).hexdigest()}  {n}\n" for n, data in sorted(files.items())).encode("utf-8")
    output.mkdir(parents=True, exist_ok=False)
    for filename, data in files.items():
        (output / filename).write_bytes(data)
    return files


def download_artifacts(api, run, sha, directory):
    metadata = api.request("GET", f"/actions/runs/{run}")
    require(metadata.get("id") == int(run) and metadata.get("head_sha") == sha and
            metadata.get("event") == "push" and metadata.get("path") == ".github/workflows/release.yml",
            "not current trusted tag workflow run")
    wanted = {artifact_name(t, sha) for t in native.TARGETS}
    artifacts = pages(api, f"/actions/runs/{run}/artifacts", "artifacts")
    require(len(artifacts) == 4 and {a["name"] for a in artifacts} == wanted, "wrong run artifact set")
    directory.mkdir(parents=True, exist_ok=False)
    ids = set()
    for artifact in artifacts:
        aid = artifact["id"]
        require(type(aid) is int and aid > 0 and aid not in ids and artifact.get("expired") is False,
                "invalid/duplicate/expired artifact")
        ids.add(aid)
        data = api.request("GET", f"/actions/artifacts/{aid}/zip", binary=True)
        require(artifact.get("digest") == "sha256:" + hashlib.sha256(data).hexdigest(), "artifact transport digest mismatch")
        target = next(t for t in native.TARGETS if artifact_name(t, sha) == artifact["name"])
        names = {archive_name(target, package.version()), "SHA256SUMS", "verification.json"}
        destination = directory / artifact["name"]
        destination.mkdir()
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            require(len(archive.infolist()) == 3 and set(archive.namelist()) == names, "unsafe artifact ZIP entries")
            for entry in archive.infolist():
                require(entry.file_size <= 1024**3 and not entry.is_dir() and
                        (entry.external_attr >> 16) & 0o170000 in {0, 0o100000}, "unsafe artifact ZIP mode/size")
                (destination / entry.filename).write_bytes(archive.read(entry))


def release_ok(release, tag, sha, marker):
    # target_commitish is unused for an existing tag, not its authoritative SHA.
    # Git.guard verifies the actual remote ref; the exact owner marker binds SHA.
    require(type(release.get("id")) is int and release["id"] > 0 and release.get("tag_name") == tag and
            release.get("prerelease") is True and
            release.get("draft") is True and release.get("body") == marker and
            release.get("author", {}).get("login") == "github-actions[bot]", "published or unknown/manual draft: refusing replacement")


def assets(api, rid):
    result = pages(api, f"/releases/{rid}/assets")
    require(len({a["name"] for a in result}) == len(result) and
            len({a["id"] for a in result}) == len(result), "duplicate release assets")
    require(all(type(a.get("id")) is int and a["id"] > 0 and a.get("state") == "uploaded" and
                type(a.get("size")) is int and a["size"] > 0 for a in result), "invalid asset metadata")
    return result


def publish(api, git, files, tag, sha, version, repository):
    """Resume only our exact draft; published releases always fail closed, never delete/replace."""
    identity(tag, sha, version)
    expected = {archive_name(t, version) for t in native.TARGETS} | {"SHA256SUMS"}
    require(set(files) == expected and all(isinstance(v, bytes) and v for v in files.values()), "incomplete final asset set")
    sums = "".join(f"{hashlib.sha256(files[n]).hexdigest()}  {n}\n" for n in sorted(expected - {"SHA256SUMS"})).encode("utf-8")
    require(files["SHA256SUMS"] == sums, "invalid global checksum")
    marker = f"tonefit-stage-release-v1\nrepository={repository}\ntag={tag}\ncommit={sha}\n"
    git.guard(tag, sha, version)
    # The tag endpoint returns published releases only. With the publisher's write
    # token, the paginated list includes drafts; never create on a tag-endpoint 404.
    matches = [r for r in pages(api, "/releases") if r.get("tag_name") == tag]
    require(len(matches) <= 1, "duplicate tag releases: refusing replacement")
    release = matches[0] if matches else None
    if release is None:
        # A SHA here would authorize GitHub to recreate a concurrently deleted tag.
        # Resolve only the existing tag itself; absent tag => unresolvable commitish.
        release = api.request("POST", "/releases", {"tag_name": tag, "target_commitish": "refs/tags/" + tag,
                              "name": tag, "body": marker, "draft": True, "prerelease": True})
    release_ok(release, tag, sha, marker)
    rid = release["id"]
    existing = assets(api, rid)
    require({a["name"] for a in existing} <= expected, "unexpected draft assets")
    # No deletions, even in owned drafts. Existing bytes must be identical.
    for asset in existing:
        data = files[asset["name"]]
        require(asset["size"] == len(data) and api.request("GET", f"/releases/assets/{asset['id']}", binary=True) == data,
                "existing draft asset differs")
    for name in sorted(expected - {a["name"] for a in existing}):
        result = api.request("POST", f"/releases/{rid}/assets?name=" + urllib.parse.quote(name), files[name], upload=True)
        require(result.get("name") == name and result.get("size") == len(files[name]) and
                result.get("state") == "uploaded" and type(result.get("id")) is int, "upload response mismatch")
    final = assets(api, rid)
    require(len(final) == 5 and {a["name"] for a in final} == expected, "incomplete remote asset set")
    for asset in final:
        require(asset["size"] == len(files[asset["name"]]) and
                api.request("GET", f"/releases/assets/{asset['id']}", binary=True) == files[asset["name"]],
                "remote asset roundtrip mismatch")
    # Re-read release ownership/state and fresh tag/main immediately before visibility.
    release_ok(api.request("GET", f"/releases/{rid}"), tag, sha, marker)
    git.guard(tag, sha, version)
    result = api.request("PATCH", f"/releases/{rid}", {"draft": False, "prerelease": True})
    require(result.get("draft") is False and result.get("prerelease") is True and
            result.get("id") == rid and result.get("tag_name") == tag, "publish response mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "stage", "publish"])
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    version = package.version()
    identity(args.tag, args.sha, version)
    if args.command == "check":
        require(args.artifacts is not None and args.output is not None, "check requires --artifacts/--output")
        collect(args.artifacts, args.tag, args.sha, version, args.output)
        return
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("GITHUB_EVENT_NAME") == "push" and
            os.environ.get("GITHUB_REF") == "refs/tags/" + args.tag and os.environ.get("GITHUB_SHA") == args.sha and
            os.environ.get("GITHUB_REPOSITORY") == "Nieole/tonefit", "not trusted stage CI context")
    git = Git()
    git.guard(args.tag, args.sha, version)
    if args.command == "stage":
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"sha={args.sha}\n")
        return
    api = API(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_TOKEN"])
    with tempfile.TemporaryDirectory(prefix="tonefit-publish-") as tmp:
        directory = Path(tmp) / "artifacts"
        download_artifacts(api, os.environ["GITHUB_RUN_ID"], args.sha, directory)
        files = collect(directory, args.tag, args.sha, version, Path(tmp) / "final")
        publish(api, git, files, args.tag, args.sha, version, os.environ["GITHUB_REPOSITORY"])


if __name__ == "__main__":
    main()
