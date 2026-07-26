# Developer Install

Use only when the owner wants the current checkout installed locally. This lane
does not version, tag, publish, or build a DMG.

1. Complete the shared fixed-point check in `SKILL.md`.
2. Build and replace the installed app:

   ```bash
   bun run app:install-local
   ```

3. Use `bun run app:install-local:existing` only when a fresh repo-local app
   already exists and no DMG build has cleaned it.
4. Report the installed version and whether launch/smoke verification ran.

For temporary development testing use `bun run app:dev:log`; do not create an
installed release.
