# Release Execution

Read this reference only after selecting the release lane. Before running a
command, confirm it against `scripts/AGENTS.md` and the live package scripts;
the repository environment owns command truth.

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

1. Confirm the intended version and impact category.
2. Write the matching `CHANGELOG.md` section and run the version bump script.
3. Run validation selected by the changed owner. For release-only metadata,
   `git diff --check` plus artifact proof is sufficient unless a concrete
   safety, data, or contract invariant requires more.
4. Build and verify the DMG:

   ```bash
   bun run app:build:dmg
   bun scripts/resolve-release-dmg.ts --version <x.y.z>
   hdiutil verify "<resolved-dmg-path>"
   ```

   Use `bun run app:build` only for explicit repo-local app validation; a normal
   DMG release already builds the app.
5. For Public Release + Developer Install, run `bun run app:install-local` after
   DMG verification. This native rebuild is expected: the published DMG stays
   portable while the owner's installed app targets the local Mac. The DMG
   lane may also remove the intermediate app; do not substitute
   `app:install-local:existing`.
6. Commit and tag the accepted release:

   ```bash
   git add <accepted-code-and-release-metadata>
   git commit -m "rel: release v<x.y.z>"
   git tag v<x.y.z>
   ```

7. Push intentionally:

   ```bash
   git push origin main
   git push origin v<x.y.z>
   ```

8. Unless the owner requested tag-only, publish the verified artifact with the
   matching changelog section as release notes:

   ```bash
   gh release create v<x.y.z> "<resolved-dmg-path>" --title "AudioBook Boss v<x.y.z>" --notes-file <notes-file>
   gh release verify-asset v<x.y.z> "<resolved-dmg-path>"
   gh release view v<x.y.z>
   gh release list --limit 5
   ```

   `verify-asset` must succeed for the exact local DMG that passed `hdiutil
   verify`; it confirms the GitHub asset is byte-for-byte the tested file.
