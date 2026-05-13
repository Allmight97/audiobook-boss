---
name: release
description: Audiobook Boss release workflow for deciding whether changes need a formal version/changelog release, preparing release metadata, validating the local app or DMG artifact, tagging, and publishing manually. Use when the user mentions release, version bump, changelog, tag, GitHub Release, DMG, publish, ship, release notes, or asks whether a change should be released.
---

# Release

## Overview

Use this skill to keep release work consistent without spreading release logic across repo scripts. Prefer human-readable `CHANGELOG.md` plus one mechanical version bump script; do not add changelog generators, release orchestrators, or GitHub release automation unless the owner explicitly chooses that tradeoff.

## Decision Rule

Treat a formal release as needed when an accepted change affects user-visible behavior, output files, metadata, audio processing, packaging, runtime safety, supported fixtures, or dependency/security posture.

Use an internal changelog note without a version bump when the change is only repo guidance, tests, comments, local agent hooks, docs, cleanup, or planning state and does not need a shipped build.

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
4. Run `scripts/checks.sh standard`.
5. For a local app smoke build, run `bun run app:build`.
6. For a DMG release, run:
```bash
bun run app:build:dmg
bun scripts/resolve-release-dmg.ts --version <x.y.z>
hdiutil verify "<resolved-dmg-path>"
```
7. Commit release metadata and code together when they are part of the same accepted release:
```bash
git add -A
git commit -m "rel: release v<x.y.z>"
git tag v<x.y.z>
```
8. Push intentionally:
```bash
git push origin main
git push origin v<x.y.z>
```

GitHub Release publishing is manual unless repo automation is deliberately reintroduced. Use the matching `CHANGELOG.md` section as the release notes and attach the verified DMG.

## Final Proof

End release work by reporting the version, tag, changelog entry, validation commands, artifact path if built, and local/remote SHA parity.
