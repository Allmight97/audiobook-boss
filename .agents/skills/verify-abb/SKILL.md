---
name: verify-abb
description: "Prove ABB's golden path on real output files without a window, from import through export, collisions, cancel, a save during an export, chapters, layout, a killed export, a copy of the owner's library, or a real Audible download. Use to verify a change end to end."
---

# Verify ABB

Prove the golden path without a window: import, metadata save, export,
collisions, cancel, a save made during an export, chapters, output layout, and
a killed export. Each feature is a script in
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
- Audible decrypt is proved by `bash scripts/verify.sh engine` (synthetic
  AAX/AAXC fixtures through the bundled FFmpeg), not by this skill.
- A real Audible download and decrypt runs only when the owner asks for it,
  through `bash scripts/abb-dev.sh audible` (its `--help` lists the commands).
  The owner runs `sign-in` once; the sign-in lives in abb-dev's own keychain
  entry, never the app's. Then `acquire` into a fresh `--state-dir` the
  fewest titles the question needs, newest first is fine: each one spends the
  owner's account and time. Check each landed file with ffprobe, and delete
  the state folder after, because it holds decrypted audiobooks. With no
  sign-in, `library` and `acquire` fail and say so: report the download as
  blocked on the owner's sign-in.
- `corpus` is opt-in and not in `run.sh`'s default list: set
  `ABB_CORPUS_DIR=<folder of the owner's audiobooks>` and run `run.sh corpus`.
  It reads the owner's files but works only on copies, and compares title,
  chapters, and length after encode and Preserve, and diffs the full
  `ffprobe -show_format` tags: Preserve fails on a tag the output lost, and an
  encode lists the tags it did not carry (`features/corpus.sh`).
- To prove a code change kept an output (a port, a refactor, an FFmpeg
  update), compare before and after with `abb-dev parity`:
  1. On the build before the change, export the input and record it:
     `bash scripts/abb-dev.sh <input> --format m4b --intent encode --template '{title}' --out <before> --export`,
     then `bash scripts/abb-dev.sh parity record <before>/*.m4b --reference <ref.json>`.
  2. On the changed build, export the same input the same way into `<after>`
     and run `bash scripts/abb-dev.sh parity check <after>/*.m4b --reference <ref.json> --bytes`.

  `SAME` means equal; `DIFFERENT` names the parts (audio, covers, container,
  streams, chapters, bytes). Use `--bytes` only when both builds link the same
  FFmpeg and the output is not Matroska; without it the check compares
  decoded audio, covers, tags, streams, and chapters, which is what an FFmpeg
  update must keep. The command reads files with the `ffmpeg` and `ffprobe`
  commands, not with the engine. `features/parity.sh` proves two exports of
  one input agree.
- A new feature is a new `features/<name>.sh` added to `run.sh`'s list; check
  the result with ffprobe or the JSON, not with abb-dev's exit code alone.
