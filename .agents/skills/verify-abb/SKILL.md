---
name: verify-abb
description: Drive abb-dev headlessly through golden-path features, keep JSON plus ffprobe evidence, and never use stored credentials.
---

# Verify ABB

Prove the golden path without a window: import, metadata save, export,
collisions, cancel, chapters, output layout, and a killed export. Each feature is a script in
`features/` whose exit code is its verdict; `run.sh` runs them and keeps
evidence. The `media` lane in `scripts/verify.sh` runs the same scripts, so CI
proves them.

1. Doctor: `bash scripts/setup.sh --check`. If it fails, run the command it
   prints and check again. Do not continue until it is clean.
2. Run: `bash .agents/skills/verify-abb/run.sh` (or name features, such as
   `run.sh export chapters`).
3. Report the `PASS`/`FAIL` lines and the evidence folder it prints
   (`.logs/verify/<run-id>/`: each feature's log, abb-dev JSON, and
   `summary.txt`). A failed feature's log names the failing check.

Rules:

- Inputs are synthesized with the FFmpeg 9 CLI. Never point abb-dev at the
  owner's library or production state; every call gets its own `--state-dir`,
  except `crash`, which reuses one on purpose to relaunch after `kill -9`.
  `corpus` is the one feature that reads the owner's files, and only copies.
- Audible decrypt is proved by `bash scripts/verify.sh decrypt` (synthetic
  AAX/AAXC fixtures through the published helper), not by this skill.
- Remote Audible download is blocked: it needs a real account, and this skill
  never reads stored credentials. Report it as blocked.
- `corpus` is opt-in and not in `run.sh`'s default list: set
  `ABB_CORPUS_DIR=<folder of the owner's audiobooks>` and run
  `run.sh corpus`. It copies each file, so it never writes into the folder,
  and runs import, encode, and re-import against the copy, plus Preserve for
  MP3, M4B, and M4A, comparing title, chapters, and length. Unset, it fails
  and names `ABB_CORPUS_DIR`. Names are copied byte for byte: NFC and NFD
  forms stay distinct.
- A new feature is a new `features/<name>.sh` added to `run.sh`'s list; check
  the result with ffprobe or the JSON, not with abb-dev's exit code alone.
