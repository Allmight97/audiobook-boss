---
name: release
description: Audiobook Boss release workflow for choosing a release lane, preparing version and changelog metadata, validating a local app or DMG artifact, and publishing an intentional tag and GitHub Release. Use when the user mentions release, version bump, changelog, tag, GitHub Release, DMG, publish, ship, release notes, or asks whether a change should be released.
---

# Release

Choose exactly one lane before changing state:

| User intent | Lane | Procedure |
| --- | --- | --- |
| Install the latest build locally | Developer Install | `references/developer-install.md` |
| Build and verify a DMG without publishing | Artifact-Only | `references/artifact-only.md` |
| Version, tag, and publish | Public Release | `references/public-release.md` |
| Publish and also update `/Applications` | Public Release, then Developer Install | Load both procedures in that order. |

If “release” is ambiguous, confirm the lane. Do not infer permission to tag,
push, publish, or replace the installed app.

## Release Decision

A formal release is warranted when accepted work changes user-visible behavior,
output files, metadata, audio processing, packaging, runtime safety, supported
fixtures, or dependency/security posture.

Repo guidance, tests, comments, cleanup, and planning state normally need only
an internal changelog note, if any. Choose the smallest honest scope.

## Fixed Point

Before any mutation, record:

- selected lane and intended version, when applicable
- current branch, local SHA, worktree status, and configured remote
- remote parity for the branch being published
- whether the intended version tag and GitHub Release already exist
- every dirty path, classified as release-owned, accepted product work, or
  unrelated/user-owned work

Stop on a wrong branch, unexpected divergence, existing tag/release, ambiguous
dirty path, or overlap with unrelated work. Do not hide these conditions with a
force flag or broad staging command.

## Shared Rules

- Synchronize `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md` during explicit
  versioned release work.
- Bump versions with `bun scripts/bump-version.ts <x.y.z>`.
- Copy the current changelog pattern. Keep entries concise and
  user/outcome-facing; omit empty categories.
- Choose validation from the changed owner and concrete risk surface. For
  release-only metadata changes, `git diff --check` plus artifact proof is
  sufficient unless an invariant requires more.
- Stage only enumerated, reviewed paths. Never use `git add -A`, `git add .`, or
  an equivalent broad staging command in a release workflow.
- Treat tag, push, GitHub Release creation, and local app replacement as
  separate state changes. Confirm authority before the first requested one.

## Completion Proof

Report the selected lane, version, validation performed, artifact path when
applicable, and all state changes. For a public release, also prove:

- release commit equals the intended local commit
- remote branch SHA equals the local release commit
- remote tag resolves to that same commit
- GitHub Release exists at the intended tag
- verified DMG is attached under the expected asset name

If any proof is missing, report the release as incomplete. A tag-only publish is
not a GitHub Release and will not appear as the latest release.
