# STAGE RELEASE configuration / helper proof

Configuration-only step: no release, tag, commit, push, index/history change or repository setting change was performed. The user's `.gitattributes` modification is untouched. The approved eventual delivery remains v0.1.0-alpha.1; no actual publication is claimed here.

## Implemented boundary

- `release.yml`: strict helper-validated alpha/beta/rc tag push; inline Git ancestry verification before executing tagged helper code; same-SHA reusable four-native-target quality/package run; explicit success-only publisher dependency barrier; per-tag concurrency; only publisher has contents:write.
- `quality.yml`: existing PR/all-branch/dispatch behavior retained, plus workflow_call with an exact commit input, explicit checkout/HEAD equality and same-SHA artifact names. Full retries replace only same-run temporary artifacts, never Release assets.
- `release.py`: fresh remote tag/main checks; exact current-run artifact whitelist and transport digest; safe artifact ZIP and final archive validation; internal clean SHA/tag/target/version/native/lock provenance; global checksums from the four unchanged final archives. No cross-architecture execution in the publisher.
- Draft ownership binds repository/tag/SHA and github-actions[bot]. Unknown/manual drafts and every already-published release fail closed. Owned drafts resume only identical assets; no deletion/replacement endpoint is used. Five remote assets are paginated, checked for exact metadata, downloaded and compared byte-for-byte before a final remote Git/release-state guard and the single visibility PATCH.
- Draft discovery uses write-token-visible `GET /releases`, paginating every page before selecting a unique same-tag release. The tag endpoint is published-only and its 404 must never authorize creation. Duplicate same-tag records fail closed, including published/draft duplicates across pages.
- Creation still supplies only the already-validated `refs/tags/<tag>` as target_commitish, never a SHA/main fallback. GitHub documents target_commitish as **unused when the tag exists**; returned `main`/`master`/SHA metadata is advisory, not tag identity. Actual remote tag SHA/main ancestry, artifact provenance and exact owner marker remain mandatory. No response target_commitish equality check remains. Existing-tag creation acceptance and ref-deletion races cannot be proved by local mocks (see operational limits).

## Actual local checks

All commands completed successfully on macOS ARM64; no Rust source changed, so no local Cargo rebuild was run for this configuration-only step.

- `/tmp/tonefit-ci-python/bin/python -m unittest discover -s scripts/ci/tests -v`: **39 passed**, including workflow contracts, offline CLI with all four synthetic archive architectures, missing/duplicate platforms, wrong report/internal identity, tampering, traversal, missing license, main ancestry, remote tag movement, partial upload/resume, manual drafts, published rerun rejection, exact five-asset roundtrip and publish-once policy. Public Git/REST command adapters exercise external state; a real local HTTP server checks REST status/JSON/binary/error handling. Redirect tests ensure credentials are removed before GitHub CDN/Azure artifact storage redirects and reject unknown hosts. Log: `/tmp/rc565-helpers.log`. The rc_565250ed counterexample changes the adapter to published-only tag lookup and real 100-item release-list pages: old code fails **7 assertions** (16 tests; `/tmp/rc565-red.log`), fixed code passes (16 tests; `/tmp/rc565-green.log`). Restoring old source after adding the last-state barrier test again fails **7 assertions** (17 tests; `/tmp/rc565-reversal-red.log`); restoring the fix yields the final **39/39** full helper suite. Checks explicitly assert one creation across partial-upload retry, exact resumed asset bytes, blocked manual/published/duplicate releases, advisory commitish acceptance with two Git guards, five downloads, and final release-state rejection before visibility.
- `actionlint`: exit 0; `/tmp/rc565-actionlint.log` (empty).
- `.scratch/session-redesign`, PATH prefixed `/tmp/node-v24.22.2-darwin-arm64/bin`: Node **v24.22.2**, `npm ci && npm run check && npm test`: export bytes match; **13 tests passed**, no skips. Log: `/tmp/rc565-js.log`. npm still reports the existing **two high-severity dependency audit findings**; no dependency churn or audit suppression.
- `git diff --check`: passed (`/tmp/rc565-diffcheck.log`, empty). Spec graph validation: passed, no issues.

## Primary API correction (rc_565250ed)

Read-only research: GitHub's official REST OpenAPI description downloaded from
`https://raw.githubusercontent.com/github/rest-api-description/main/descriptions/api.github.com/api.github.com.json`
(`/tmp/rc565-github-openapi.json`). Its operation documentation links are:

- https://docs.github.com/rest/releases/releases#list-releases — only users with push access receive draft listings.
- https://docs.github.com/rest/releases/releases#get-a-release-by-tag-name — “Get a published release with the specified tag.”
- https://docs.github.com/rest/releases/releases#create-a-release — target_commitish determines **tag creation**, “Unused if the Git tag already exists”; the official response example uses `master`.

Direct docs-page fetching was blocked by the local resolver; the official GitHub-maintained OpenAPI was fetched successfully instead. Tests do not assume GitHub echoes `refs/tags/<tag>` as authoritative metadata. Documentation promises branch/SHA creation inputs, not the precise normalization of existing-tag response metadata; actual acceptance remains remote proof, not a mocked guarantee. There is no fallback to main or SHA on any REST error.

## Open proof / operational limits

These are local configuration/helper tests, not real four-platform execution or a GitHub release. Parent must run the final changed workflow tree remotely; any independent Quality run of 699aa67 does not certify this tree. Actual native build/audit/runtime provenance, current-run artifact digest/download behavior, GitHub draft/upload/list/download/PATCH responses, token permissions, and eventual published asset downloads remain unproved remotely. Do not publish until the approved final tree is on main and the tagged same-SHA full matrix passes.

Workflow/main/tag writers are trusted configuration owners; no repository protection/settings were changed. There is no atomic Git-ref-plus-Release transaction in GitHub REST: the helper rechecks immediately before visibility, but cannot prevent a privileged concurrent ref mutation after that final check. A connection loss after GitHub accepts the visibility PATCH can leave outcome uncertain; subsequent attempts refuse published releases rather than replacing anything. Platform minimum-system/client/quarantine and external-material caveats remain those in package-notes.md. Known Node 20 Action-runtime warnings forced to Node 24 are not counted as native failures; existing fully pinned action SHAs were not churned.

## Artifact Accept negotiation correction (before first tag)

The parent observed a real read-only GitHub failure: `GET
repos/Nieole/tonefit/actions/artifacts/13080274440/zip` with
`Accept: application/octet-stream` returned HTTP 415 (Unsupported Accept; JSON
required). The same `gh api --allow-escape-sequences` GET with
`Accept: application/vnd.github+json` downloaded an actual ZIP whose SHA-256 was
`6bffa57135d36356ce767fdb28f8c0ebeca50c35ba6f00d5d03a67ed8a21d1d1`, matching
GitHub artifact metadata. This was a real helper bug, not a CLI-only problem:
`API.request(binary=True)` previously selected octet-stream globally.

- `scripts/ci/release.py`: binary response decoding is now independent of Accept
  negotiation. Only binary GET `/releases/assets/<numeric-id>` selects
  octet-stream; Actions artifact ZIP requests retain GitHub JSON Accept. No
  status handling, digest/ZIP validation, redirect allowlist, or redirect
  Authorization stripping was weakened; HTTP 415 still raises.
- `scripts/ci/tests/test_release.py`: a real local HTTP server captures requests
  from the actual API opener, requires JSON for Actions ZIP and octet-stream for
  Release asset downloads, and checks that 415 is propagated. Existing digest,
  unsafe ZIP, corrupt asset roundtrip, status and redirect tests remain green;
  an additional assertion rejects a JSON metadata body even when its transport
  digest matches.
- TDD: old source failed the new endpoint-header test with HTTP 415 (20 release
  tests, one error; `/tmp/artifact-accept-red.log`). Fixed source passed all
  **55 helper tests**, including Windows proof tests (previous 54 + one new test;
  `/tmp/artifact-accept-green.log`). Reversing only the header selection produced
  the same failure (`/tmp/artifact-accept-reversal-red.log`); restoring the fix
  passed **55/55** again (`/tmp/artifact-accept-restored-green.log`).
- `actionlint` and `git diff --check`: exit 0
  (`/tmp/artifact-accept-actionlint.log`, `/tmp/artifact-accept-diffcheck.log`).
  Spec graph validation: valid, no issues. Existing spec contracts are unchanged.
  This correction changes only Python/tests/these notes; no Rust or JS checks
  were rerun, and earlier results are not newly claimed.

Actual helper integration proof (not merely gh CLI): the fixed `API` instance
used its real `SafeRedirect` opener to GET artifact metadata, run metadata, and
that artifact ZIP from GitHub. Token acquisition was in memory via `gh auth token`, never printed or written
to a token file. No POST/PATCH/DELETE, tag,
Release creation, commit, push, or index mutation occurred. All requests used
GitHub API plus its allowlisted HTTPS download redirect. The resulting ZIP
passed CRC checks, had exactly these three entries, and matched both the
expected SHA-256 above and the live artifact digest:

- `SHA256SUMS`
- `tonefit-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz`
- `verification.json`

Log: `/tmp/artifact-accept-live-readonly.log` (safe metadata only). Run
`41295011993` has SHA `49fb856b5912802155dd81b22002de9526c0f7b6`, event `push`,
and workflow `.github/workflows/quality.yml`. This proves the real artifact
transport/header/redirect/digest boundary for that existing Quality artifact;
it does **not** bypass or prove `download_artifacts`' current release-workflow
run gate, four-platform collection, or real five-asset Release roundtrip.
Those publication proofs remain gated on the approved same-SHA trusted release
matrix and five-asset verification; no actual publication is claimed.
