---
name: release
description: Prepare ABB version/changelog changes, install a local build, package a DMG, or publish a GitHub Release when the user requests that release work or a release-scope decision.
---

# Release

State the accepted change's user-visible impact, the version or no-bump
decision, and the requested artifact before executing release work.

## Version Decision

Recommend a formal release for changes to shipped behavior, output files,
metadata, media processing, packaging, runtime safety, or dependency/security
posture. Guidance, tests, comments, and planning alone do not require a version
bump or changelog entry unless requested.

When choosing a version, use patch for fixes, minor for added capability, and
major for a breaking contract. Respect an explicit owner-selected version;
explain a material mismatch with the impact instead of silently changing it.

## Select The Requested Lane

| Requested outcome | Lane and effect |
| --- | --- |
| Developer/local install | Build for the compiling Apple Silicon host and replace `/Applications/AudioBook Boss.app`. |
| DMG/package only | Build and verify a portable Apple Silicon DMG. |
| Public/GitHub Release | Prepare metadata, build and verify the portable DMG, commit/tag/push the accepted release, and publish its verified asset. |
| Public plus local install | Public Release plus a separate native developer install. |
| Tag only | Create/push the requested tag; this does not create a GitHub Release. |

Use the whole request and prior authorization to select the lane. If “release”
leaves the destination unclear, inspect the accepted change and prepare the
version/lane recommendation, then resolve that choice before a build, install,
or publication that depends on it. Public release alone does not request a
local install. Temporary development testing uses `bun run app:dev:log`.

## Execute And Verify

Read the applicable sections of [execution.md](references/execution.md) for
the selected lane. Check commands against `scripts/AGENTS.md` and live package
scripts. That reference owns the portable-artifact and publication sequence;
owner guidance determines any additional code verification.

Complete the authorized lane, including artifact proof and remote verification
when publishing. Report only applicable outcomes: version/changelog, installed
app, DMG path, tag and commit, GitHub Release URL, asset verification, and
local/remote SHA parity. If a required step fails, state the completed state
and the exact remaining action.
