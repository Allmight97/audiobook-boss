---
name: verify-abb
description: Drive abb-dev headlessly through golden-path features, keep JSON plus ffprobe evidence, and never use stored credentials.
---

# Verify ABB

Prove the golden path without a window: import, edit, save, export, collisions,
cancel, chapters, and synthetic Audible decrypt. Drive `abb-dev` the same way in
every environment (decrypt uses `verify.sh decrypt`).

## Launch

Use a throwaway state directory so the run never touches the app's settings or
credentials:

```bash
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
EVIDENCE=".logs/verify/${RUN_ID}"
STATE="$(mktemp -d "${TMPDIR:-/tmp}/abb-verify-state.XXXXXX")"
INPUTS="$(mktemp -d "${TMPDIR:-/tmp}/abb-verify-in.XXXXXX")"
mkdir -p "${EVIDENCE}"
eval "$(scripts/setup.sh --print-env)"
ABB_DEV=(bash scripts/abb-dev.sh)
```

`abb-dev` already uses its own identity (`com.audiobook-boss.devtool`). Always
pass `--state-dir "${STATE}"` and `--json`.

## Doctor

```bash
bash scripts/setup.sh --check
```

If that fails, run the command it prints, then `--check` again. Do not continue
until Doctor is clean.

## Drive

Synthesize inputs with the FFmpeg 9 CLI (`-f lavfi` sine tones, as the media
tests already do). After each ffmpeg command, check the exit code; a nonzero
exit is a failed feature. Treat an import whose JSON has `files: []` or
`noSupportedFiles` as a failure — an empty session is not a pass. Then run
one feature at a time from `features/`. Live Audible download is blocked: it
needs a real account, and this skill never reads stored credentials. Synthetic
decrypt uses the published helper and the fixtures from `verify.sh decrypt`.

## Evidence

For each feature, write:

- the `--json` session snapshot to `${EVIDENCE}/<feature>.json`
- an `ffprobe` readback of every output to `${EVIDENCE}/<feature>.ffprobe.json`
- a one-line verdict to `${EVIDENCE}/summary.txt`

Keep going through the unblocked features after a failure. The run is not green
if any unblocked feature failed.

## Cleanup

Remove `${STATE}` and `${INPUTS}`. Keep `${EVIDENCE}`.

```bash
rm -rf "${STATE}" "${INPUTS}"
```

## Feature map

| Feature | File | Status |
| --- | --- | --- |
| Import | [features/import.md](features/import.md) | Drive |
| Metadata edit and save | [features/metadata.md](features/metadata.md) | Drive |
| Export formats and encoders | [features/export.md](features/export.md) | Drive |
| Collisions and cancel | [features/collisions-cancel.md](features/collisions-cancel.md) | Drive |
| Chapters | [features/chapters.md](features/chapters.md) | Drive |
| Audible decrypt | [features/decrypt.md](features/decrypt.md) | Drive |
| Remote / Audible download | [features/remote-audible.md](features/remote-audible.md) | Blocked |
