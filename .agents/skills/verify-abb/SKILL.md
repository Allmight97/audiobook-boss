---
name: verify-abb
description: Drive abb-dev headlessly through golden-path features, keep JSON plus ffprobe evidence, and never use stored credentials.
---

# Verify ABB

Prove the golden path without a window: import, metadata save, export,
collisions, cancel, and chapters. Each feature is a script in `features/`
whose exit code is its verdict; `run.sh` runs them and keeps evidence. The
`media` lane in `scripts/verify.sh` runs the same scripts, so CI proves them.

1. Doctor: `bash scripts/setup.sh --check`. If it fails, run the command it
   prints and check again. Do not continue until it is clean.
2. Run: `bash .agents/skills/verify-abb/run.sh` (or name features, such as
   `run.sh export chapters`).
3. Report the `PASS`/`FAIL` lines and the evidence folder it prints
   (`.logs/verify/<run-id>/`: each feature's log, abb-dev JSON, and
   `summary.txt`). A failed feature's log names the failing check.

Rules:

- Inputs are synthesized with the FFmpeg 9 CLI. Never point abb-dev at the
  owner's library or production state; every call gets its own `--state-dir`.
- Audible decrypt is proved by `bash scripts/verify.sh decrypt` (synthetic
  AAX/AAXC fixtures through the published helper), not by this skill.
- Remote Audible download is blocked: it needs a real account, and this skill
  never reads stored credentials. Report it as blocked.
- A new feature is a new `features/<name>.sh` added to `run.sh`'s list; check
  the result with ffprobe or the JSON, not with abb-dev's exit code alone.
