---
name: release
description: "Cut a release, package the DMG, or install the app locally."
---

# Release

`bun scripts/release.ts <x.y.z> <lane>` runs the release; its header says
what each lane does. Settle the four decisions below first, and state them
before running it.

## Version

Patch for fixes, minor for added capability, major for a breaking contract.
Changes to guidance, tests, or plans alone need no release. An owner-chosen
version stands; when it does not match the change's impact, say so and why.

## Changelog

Write the `## [x.y.z] - YYYY-MM-DD` section of `CHANGELOG.md` by hand, for
users, in the existing format: `### Added`, `### Changed`, `### Fixed`,
`### Removed`, empty ones left out. Write it from the change's user-visible
effect, not from the commit list. The script refuses a release without it
and publishes it as the GitHub Release notes.

## Lane

| The owner wants | Lane |
| --- | --- |
| The app installed on this Mac | `local` |
| A portable DMG only | `dmg` |
| A public GitHub Release | `github` |
| A public release and the app installed | `github+local` |

When "release" leaves the destination open, recommend a lane and settle it
before running. A tag without a GitHub Release is not a lane; do it by hand
only on request. Temporary development testing uses `bun run app:dev:log`.

## LGPL

The app statically links FAAC and FAAD3 and FFmpeg under LGPL-2.1-or-later.
The script checks that the app ships every license file in
`src-tauri/tauri.conf.json` `bundle.resources` and that the FFmpeg notice
names the pinned commit. Before a public lane, judge what it cannot: that a
recipient can get the matching source and build materials, change the
library, and rebuild the app, as each `vendor/*/ABB-PROVENANCE.md` describes.
A new Apache-2.0 dependency with a NOTICE file ships that text too.
