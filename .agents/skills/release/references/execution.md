# Release Execution

Read the sections for the selected lane after stating the version or no-bump
decision. Before running a command, check it against
`scripts/AGENTS.md` and the live package scripts; the repository environment
owns command truth.

## Version And Changelog

During explicit version work, keep these synchronized:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`

Use `bun scripts/bump-version.ts <x.y.z>`. Add a concise, user-facing section:

```markdown
## [x.y.z] - YYYY-MM-DD

### Added

### Changed

### Fixed

### Removed
```

Omit empty categories. Do not generate release notes from commit history by
default.

## Developer Install

Run:

```bash
bun run app:install-local
```

This builds and installs `/Applications/AudioBook Boss.app`, signs it ad hoc,
registers it, refreshes Spotlight metadata, and removes the repo-local install
artifact. The source build targets the compiling Apple Silicon host natively.
Use `bun run app:install-local:existing` only when a fresh repo-local app
already exists and only reinstalling is needed.

Report the installed version and any launch/smoke proof. Do not tag, publish, or
build a DMG in this lane.

## Artifact Only

Run:

```bash
bun run app:build:dmg
bun scripts/resolve-release-dmg.ts --version <x.y.z>
hdiutil verify "<resolved-dmg-path>"
```

The build must stay noninteractive and use the portable Apple Silicon FFmpeg
feature; a distributable DMG must not inherit the build host's native CPU
tuning. Report the resolved path and verification result, then stop before
tagging or publishing.

## Public Release

1. Use the selected version and accepted change's impact.
2. Write the matching `CHANGELOG.md` section and run the version bump script.
3. Run `bun run audit` so both Rust and JavaScript dependency graphs report
   before public packaging; any failure blocks the release. Run additional
   validation selected by the changed owner. For release-only metadata,
   `git diff --check` plus the dependency audit and artifact proof is sufficient
   unless a concrete safety, data, or contract invariant requires more.
4. Build and verify the DMG:

   ```bash
   bun run app:build:dmg
   bun scripts/resolve-release-dmg.ts --version <x.y.z>
   hdiutil verify "<resolved-dmg-path>"
   ```

   Use `bun run app:build` only for explicit repo-local app validation; a normal
   DMG release already builds the app.
5. If a local install was also requested, run `bun run app:install-local` after
   DMG verification. This separate native rebuild is expected; the DMG lane
   may remove the intermediate portable app. Do not substitute
   `app:install-local:existing`.
6. Commit and tag the accepted release:

   ```bash
   git add <accepted-code-and-release-metadata>
   git commit -m "rel: release v<x.y.z>"
   git tag v<x.y.z>
   ```

7. Verify the release commit and tag identify the accepted work on the intended
   branch. For a release from `main`, push:

   ```bash
   git push origin main
   git push origin v<x.y.z>
   ```

8. Publish the verified artifact with the
   matching changelog section as release notes:

   ```bash
   gh release create v<x.y.z> "<resolved-dmg-path>" --title "AudioBook Boss v<x.y.z>" --notes-file <notes-file>
   gh release verify-asset v<x.y.z> "<resolved-dmg-path>"
   gh release view v<x.y.z>
   gh release list --limit 5
   ```

   `verify-asset` must succeed for the exact local DMG that passed `hdiutil
   verify`; it confirms the GitHub asset is byte-for-byte the tested file.

Check local/remote tag and branch SHA parity before reporting publication
complete. If a push or publish result is uncertain, inspect remote state before
retrying. An existing tag/release with conflicting content requires resolving
the mismatch; do not force-move tags or overwrite assets to make a retry pass.

## Tag Only

Verify the requested commit, create the tag, and push that tag to the intended
remote. Inspect the remote tag SHA before reporting completion. Build, install,
and GitHub Release creation apply only if separately requested.
