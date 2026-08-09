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
| "all", "public and dev", "public plus local" | Public Release + Developer Install | Publish the verified DMG, then run a separate developer install so `/Applications/AudioBook Boss.app` matches the release version. Do not run Artifact-Only separately because Public Release already builds the DMG. |

Never build or open a DMG for Developer Install. Use `bun run app:dev:log` for
temporary development testing instead of creating a local installed release.

## Decision Rule

Treat a formal release as needed when an accepted change affects user-visible behavior, output files, metadata, audio processing, packaging, runtime safety, supported fixtures, or dependency/security posture.

Use an internal changelog note without a version bump when the change is only repo guidance, tests, comments, docs, cleanup, or planning state and does not need a shipped build.

When in doubt, name the impact and choose the smallest honest release scope. Do not bump version just to make a PR look complete.

## Execute The Lane

After choosing a lane, read
[`references/execution.md`](references/execution.md) for the current version,
changelog, build, install, tag, and publication sequence. Reconcile it with
`scripts/AGENTS.md` and the live package scripts before running commands.

## Final Verification

End release work by reporting the version, tag, changelog entry, validation commands, DMG path, GitHub Release URL, attached asset name, and local/remote SHA parity. If publishing was intentionally tag-only, state that GitHub will not show it as the latest release.
