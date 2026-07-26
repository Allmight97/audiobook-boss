# Public Release

Use only with explicit authority to publish.

1. Complete the shared fixed-point check in `SKILL.md`, including intended
   version, branch, SHA, remote parity, dirty-path classification, and proof
   that the tag and GitHub Release do not already exist.
2. Update the matching `CHANGELOG.md` section and run:

   ```bash
   bun scripts/bump-version.ts <x.y.z>
   ```

3. Review the exact version-surface diff and run owner-scoped validation.
4. Build and verify the distributable:

   ```bash
   bun run app:build:dmg
   bun scripts/resolve-release-dmg.ts --version <x.y.z>
   hdiutil verify "<resolved-dmg-path>"
   ```

5. Recheck worktree status. Enumerate and review the exact accepted paths, then
   stage those paths explicitly, for example:

   ```bash
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml Cargo.lock CHANGELOG.md <accepted-code-paths>
   git diff --cached --check
   git diff --cached --stat
   git commit -m "rel: release v<x.y.z>"
   ```

   Stop if the staged diff contains an unclassified path.
6. Confirm the release commit and tag target, then create the tag:

   ```bash
   git tag v<x.y.z>
   ```

7. Reconfirm publish authority and push intentionally:

   ```bash
   git push origin main
   git push origin v<x.y.z>
   ```

8. Use the matching changelog section as release notes and attach the verified
   DMG:

   ```bash
   gh release create v<x.y.z> "<resolved-dmg-path>" --title "AudioBook Boss v<x.y.z>" --notes-file <notes-file>
   gh release view v<x.y.z>
   ```

9. Prove local/remote branch and tag SHA parity and verify the expected asset on
   the GitHub Release. Report any failed or skipped proof as incomplete.

If Developer Install was also requested, load `developer-install.md` after the
public release completes. A DMG build may clean the repo-local app, so use the
normal fresh install command rather than `app:install-local:existing`.
