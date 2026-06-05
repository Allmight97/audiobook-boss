---
name: release
description: Audiobook Boss release workflow for deciding whether changes need a formal version/changelog release, preparing release metadata, validating the local app or DMG artifact, tagging, and publishing manually. Use when the user mentions release, version bump, changelog, tag, GitHub Release, DMG, publish, ship, release notes, or asks whether a change should be released.
---

# Release

## Overview

- Prefer human-readable `CHANGELOG.md` plus one mechanical version bump script.
- Choose a release lane before running commands. Do not let "release" silently
  mean every possible lane.
- Developer install is local-machine truth: build and replace
  `/Applications/AudioBook Boss.app` without a DMG.
- Public release is distribution truth: version/changelog, tag, GitHub Release,
  and verified DMG attachment.
- Artifact-only is packaging truth: build and verify a DMG without publishing.
- A tag alone is tag-only; GitHub will not show it as the latest release.
- This skill owns release metadata and artifact proof. It does not impose a
  broad test matrix by default; choose code validation from the touched owner
  and concrete risk surface before release work, or skip it when the accepted
  change has already been verified and the owner asks to avoid rerunning tests.

## Lane Selection

Use the user's wording to choose exactly one lane unless they explicitly ask for
a combined lane.

| User wording | Lane | Meaning |
| --- | --- | --- |
| "dev release", "developer release", "local release", "install local" | Developer Install | Build current repo and silently replace `/Applications/AudioBook Boss.app`. |
| "public release", "GitHub Release", "tag and publish", "ship DMG" | Public Release | Prepare metadata, build/verify DMG, tag, push, and publish GitHub Release. |
| "artifact release", "DMG only", "package only" | Artifact-Only | Build/verify DMG and stop before tag/publish. |
| "release" with no qualifier | Confirm lane if the request is interactive; if the owner is asking to ship a new version, run Public Release plus Developer Install. |
| "all", "public and dev", "public plus local" | Public Release + Developer Install | Install local app and publish the matching public version. Do not run Artifact-Only separately because Public Release already builds the DMG. |

Never build or open a DMG for Developer Install. Use `bun run app:dev:log` for
temporary development testing instead of creating a local installed release.

## Decision Rule

Treat a formal release as needed when an accepted change affects user-visible behavior, output files, metadata, audio processing, packaging, runtime safety, supported fixtures, or dependency/security posture.

Use an internal changelog note without a version bump when the change is only repo guidance, tests, comments, docs, cleanup, or planning state and does not need a shipped build.

When in doubt, name the impact and choose the smallest honest release scope. Do not bump version just to make a PR look complete.

## Version Surfaces

Keep these synchronized during explicit release work:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`

Use:

```bash
bun scripts/bump-version.ts <x.y.z>
```

## Changelog Format

Keep entries concise and user/outcome-facing. Copy the current file pattern; do not generate from commit history by default.

```markdown
## [x.y.z] - YYYY-MM-DD

### Added

### Changed

### Fixed

### Removed
```

Omit empty categories in a release section. Keep `[Unreleased]` as the staging area only when useful; it is acceptable for release entries to be written directly during release prep.

## Developer Install Lane

Use this lane when the owner wants the latest local app with minimal friction.

```bash
bun run app:install-local
```

This builds the `.app`, installs a real `/Applications/AudioBook Boss.app`,
signs it ad-hoc for local execution, registers it with LaunchServices, refreshes
Spotlight metadata, and removes the repo-local `.app` install artifact. Use
`bun run app:install-local:existing` only when a fresh repo-local `.app` already
exists and the owner only needs to reinstall it locally.

Report the installed app version and whether launch/smoke verification was run.
Do not tag, publish, or build a DMG in this lane.

## Artifact-Only Lane

Use this lane when packaging needs proof but public release should not be
published.

```bash
bun run app:build:dmg
bun scripts/resolve-release-dmg.ts --version <x.y.z>
hdiutil verify "<resolved-dmg-path>"
```

`app:build:dmg` is expected to be noninteractive and must not require Finder or
manual drag-to-Applications behavior. Report the DMG path and verification
result. Do not tag or publish.

## Public Release Lane

1. Confirm the intended version and impact category.
2. Update `CHANGELOG.md` with `## [x.y.z] - YYYY-MM-DD`.
3. Run `bun scripts/bump-version.ts <x.y.z>`.
4. Run only the validation warranted by the changed owner or release metadata.
   For release-only version/changelog edits, `git diff --check` plus the
   artifact commands below is sufficient unless a concrete safety, data, or
   contract invariant says otherwise.
5. For repo-local `.app` artifact validation without touching `/Applications`
   and without building a DMG, run:
```bash
bun run app:build
```
Do not run this as a separate pre-step for normal public releases; the DMG
command builds the app.
6. For a DMG release, run:
```bash
bun run app:build:dmg
bun scripts/resolve-release-dmg.ts --version <x.y.z>
hdiutil verify "<resolved-dmg-path>"
```
7. If the requested lane is Public Release + Developer Install, install the
   already-built app so the local app matches the public artifact without a
   second build:
```bash
bun run app:install-local:existing
```
If the repo-local `.app` is missing, fall back to `bun run app:install-local`
and state that the local install rebuilt.
8. Commit release metadata and code together when they are part of the same accepted release:
```bash
git add -A
git commit -m "rel: release v<x.y.z>"
git tag v<x.y.z>
```
9. Push intentionally:
```bash
git push origin main
git push origin v<x.y.z>
```
10. Publish the GitHub Release unless the owner explicitly asks for tag-only:
```bash
gh release create v<x.y.z> "<resolved-dmg-path>" --title "AudioBook Boss v<x.y.z>" --notes-file <notes-file>
gh release view v<x.y.z>
gh release list --limit 5
```
Use the matching `CHANGELOG.md` section as the release notes and attach the verified DMG.

## Final Verification

End release work by reporting the version, tag, changelog entry, validation commands, DMG path, GitHub Release URL, attached asset name, and local/remote SHA parity. If publishing was intentionally tag-only, state that GitHub will not show it as the latest release.
