---
name: release
description: Audiobook Boss release workflow for deciding whether changes need a formal version/changelog release, preparing release metadata, validating the local app or DMG artifact, tagging, and publishing manually. Use when the user mentions release, version bump, changelog, tag, GitHub Release, DMG, publish, ship, release notes, or asks whether a change should be released.
---

# Release

## Overview

- Prefer human-readable `CHANGELOG.md` plus one mechanical version bump script.
- A complete public release includes a Git tag, GitHub Release, and verified DMG
  attachment.
- A tag alone is tag-only; GitHub will not show it as the latest release.
- This skill owns release metadata and artifact proof. It does not impose a
  broad test matrix by default; choose code validation from the touched owner
  and concrete risk surface before release work, or skip it when the accepted
  change has already been verified and the owner asks to avoid rerunning tests.

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
scripts/bump-version.sh <x.y.z>
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

## Release Workflow

1. Confirm the intended version and impact category.
2. Update `CHANGELOG.md` with `## [x.y.z] - YYYY-MM-DD`.
3. Run `scripts/bump-version.sh <x.y.z>`.
4. Run only the validation warranted by the changed owner or release metadata.
   For release-only version/changelog edits, `git diff --check` plus the
   artifact commands below is sufficient unless a concrete safety, data, or
   contract invariant says otherwise.
5. For a local launcher-visible app smoke build, run:
```bash
bun run app:install-local
```
This builds the `.app`, installs a real `/Applications/AudioBook Boss.app`, signs it ad-hoc for local execution, registers it with LaunchServices, refreshes Spotlight metadata, and removes the repo-local `.app` install artifact. Use `bun run app:install-local:existing` only when a fresh repo-local `.app` already exists and you only need to reinstall it locally.
6. For repo-local `.app` artifact validation without touching `/Applications`, run:
```bash
bun run app:build
```
7. For a DMG release, run:
```bash
bun run app:build:dmg
bun scripts/resolve-release-dmg.ts --version <x.y.z>
hdiutil verify "<resolved-dmg-path>"
```
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
