# Artifact-Only Release

Use when packaging needs proof but nothing should be tagged or published.

1. Complete the shared fixed-point check in `SKILL.md`.
2. Build and resolve the expected DMG:

   ```bash
   bun run app:build:dmg
   bun scripts/resolve-release-dmg.ts --version <x.y.z>
   hdiutil verify "<resolved-dmg-path>"
   ```

3. Confirm the build was noninteractive and report the resolved path and
   verification result.
4. Stop before commit, tag, push, GitHub Release creation, or local app
   replacement.
