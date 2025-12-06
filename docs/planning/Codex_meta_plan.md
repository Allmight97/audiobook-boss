# Codex Metadata Migration Plan (ffmpeg-next only)

## Bugs / Problems in scope
- **#66 Chapters missing**: Timed Text/chapters were never copied into the muxed output; outputs keep 0–1 chapters. Preview intentionally remains chapter-free.
- **#67 Duplicate covers**: Dual writers (ffmpeg-next + Lofty fallback) double-embed cover art, producing 2+ images.
- **#68 Chroma shift**: Cover art re-encoded to 4:4:4 instead of preserving source (likely caused by re-encoding/dual writes).
- **#32 Redundant cover validation**: Extension + guessed-format double validation; extension check is noisy and redundant.
- **New directive**: Remove Lofty entirely (read/write/tests/docs); ffmpeg-next becomes the single metadata reader/writer, including metadata-only “save” flows.

## Approach & rationale
- **Single writer/reader (ffmpeg-next)**: Stream-copy/remux for metadata edits; embed container metadata + chapters + attached_pic in one pass. Eliminates dual-writer race/duplication and keeps chapter handling in one place.
- **Cover art**: If user supplies art, skip source attached_pic and embed the new one once; otherwise copy the existing attached_pic stream. Unsupported formats are skipped (JPEG/PNG only).
- **Metadata-only edits**: Remux with stream copy (no audio re-encode), copy chapters, set container metadata, preserve attached_pic unless replaced.
- **Input analysis**: Use ffmpeg-next for format/duration/sample rate/channels/cover extraction; no Lofty probing anywhere.
- **Preview mode**: Still no chapters by design; keep skip flag.

## Implementation checklist
- [x] Remove Lofty dependency surface (Cargo, errors, mods, writer/reader).
- [x] ffmpeg-based metadata reader (dict + attached_pic extraction).
- [x] ffmpeg remux helper `rewrite_metadata_with_ffmpeg`: copy streams (skip attached_pic when replacing), copy chapters, set metadata (merge with existing), optional new cover stream, atomic replace.
- [x] Replace Lofty usages in validation (file_list/prepare/passthrough).
- [x] Update commands to use ffmpeg remux for Cmd+S and cover writes.
- [x] Clean/remove Lofty-based tests; convert cover art validation to ffmpeg attached_pic checks.
- [ ] Re-run quick checks (fmt/clippy/tests/contract/ts) after cleanup.
- [ ] Manual validation (mediainfo/ffprobe) on Flybot.m4b + merge cases to confirm: chapters preserved, single cover, no accumulation.

## Test plan (to execute post-build)
- `scripts/quick-checks.sh` (fmt, clippy, contract, tsc when available).
- Manual: process Flybot.m4b and multi-file merge; verify with `mediainfo`/`ffprobe -show_chapters -show_streams`:
  - Chapters count/title preserved.
  - Single attached_pic stream; no duplication after metadata-only save.
  - Cover chroma matches source (no unintended 4:4:4 re-encode).
