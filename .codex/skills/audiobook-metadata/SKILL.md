---
name: audiobook-metadata
description: "Domain knowledge for audiobook-boss metadata and file handling. Use when modifying how M4B files are tagged, changing output folder structure, debugging metadata issues with Audiobookshelf/Plex/Apple Books, or adding new metadata fields. Critical - ffprobe cannot read MVNM/MVIN tags, dual-write strategy required."
---

# Audiobook Metadata Skill

Reference for audiobook-boss metadata implementation targeting Audiobookshelf (ABS), Plex, and Apple Books.

## Tool Cross-Check

- For exact mp4ameta API signatures, use `mcp__rust-docs__docs_rs_get_item` (e.g., `mp4ameta::Tag`). `docs_rs_readme`/`docs_rs_search_in_crate` may 404.
- For ffmpeg-next metadata dictionary details, defer to `ffmpeg-next-patterns` and confirm types via docs.rs (`ffmpeg_next::format`).

## Critical Constraint

**ffprobe cannot read `MVNM`/`MVIN` (movement) tags.** ABS uses ffprobe for scanning, so series info written only to movement tags is invisible to ABS.

Source: [ABS GitHub Discussion #1481](https://github.com/advplyr/audiobookshelf/discussions/1481)

## Dual-Write Strategy (Required)

Write BOTH tag formats for universal compatibility:

| Tags | Read By | Purpose |
|------|---------|---------|
| `series` + `series-part` | ABS, Plex (via ffprobe) | Primary series tags |
| `MVNM` + `MVIN` | Apple Books/iTunes | Movement tags for Apple ecosystem |

**Never write only MVNM/MVIN** — ABS will not see series info.

## Tag Reference

See [references/tag-mapping.md](references/tag-mapping.md) for complete M4B atom mappings.

Quick reference for most common tags:

```
title          → Book title
artist         → Author
composer       → Narrator (ABS maps this correctly)
series         → Series name (critical for ABS)
series-part    → Book number in series (critical for ABS)
MVNM           → Series name (for Apple Books)
MVIN           → Book number (for Apple Books)
```

## Folder Structure

Output structure compatible with all three platforms:

```
/Author Name
  /Series Name
    /Book Title
      book.m4b
      cover.jpg
```

See [references/folder-conventions.md](references/folder-conventions.md) for ABS naming patterns.

## Verification

After writing metadata, verify with:

```bash
# Confirm series/series-part readable by ABS
ffprobe -v quiet -print_format json -show_format output.m4b | jq '.format.tags'

# Confirm MVNM/MVIN present for Apple Books
# (ffprobe won't show these - use AtomicParsley or mp4info)
AtomicParsley output.m4b -t
```

Expected ffprobe output should include:
```json
{
  "series": "Series Name",
  "series-part": "1"
}
```

## Codebase Pointers

| Component | Location | Notes |
|-----------|----------|-------|
| Metadata writing | `src-tauri/src/metadata/ffmpeg_bridge.rs` (~L50-70) | `metadata_to_ffmpeg_dict()` - currently maps to `show`/`episode_sort` |
| Data model | `src-tauri/src/metadata/mod.rs` (~L20-55) | `AudiobookMetadata` struct with `series`/`series_part` fields |
| Metadata reading | `src-tauri/src/metadata/reader.rs` (~L30-40) | Reads `show` → `series`, `episode_sort` → `series_part` |
| Output path | `src-tauri/src/commands/audio.rs` (~L280-345) | `build_output_path()` with series folder logic |
| TSOA computation | `src-tauri/src/commands/metadata.rs` (~L40-65) | `compute_tsoa()` for album sort |

### Current Implementation Status

The current ffmpeg_bridge.rs maps:
- `series` → `show` (ffmpeg's TV show field)
- `series_part` → `episode_sort` (ffmpeg's episode sort field)

**This may not produce ABS-compatible output.** The correct approach is to write:
- `series` and `series-part` as iTunes freeform atoms (`----:com.apple.iTunes:SERIES`)

See references/tag-mapping.md for the correct ffmpeg `-metadata` keys.

## Known Issues

1. **ABS embed bug** ([#3547](https://github.com/advplyr/audiobookshelf/issues/3547)): ABS "Embed Metadata" may not write series tags back to files. Solution: Write correct tags during initial file creation in audiobook-boss.

2. **ffmpeg MVNM/MVIN**: Standard `-metadata` flag may not write movement tags correctly. May need stream-level metadata or alternative tool. Test output with AtomicParsley.

3. **Current show/episode_sort mapping**: Needs verification that ABS can read these fields. If not, must switch to `series`/`series-part` ffmetadata keys.

## Implementation Checklist

When modifying metadata handling:

- [ ] Write `series` and `series-part` atoms (ABS/Plex)
- [ ] Write `MVNM` and `MVIN` atoms (Apple Books)
- [ ] Use `composer` for narrator
- [ ] Verify with ffprobe (series/series-part visible)
- [ ] Verify with AtomicParsley (MVNM/MVIN visible)
- [ ] Test import into ABS (series auto-detected, no manual match needed)
