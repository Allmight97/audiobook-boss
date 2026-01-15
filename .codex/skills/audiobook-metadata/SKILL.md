---
name: audiobook-metadata
description: "Domain knowledge for audiobook-boss metadata and file handling. Use when modifying how M4B files are tagged, changing output folder structure, debugging metadata issues with Audiobookshelf/Plex/Apple Books, or adding new metadata fields. Critical - ffprobe cannot read MVNM/MVIN tags, dual-write strategy required."
---

# Audiobook Metadata Skill

Reference for audiobook-boss metadata implementation targeting Audiobookshelf (ABS), Plex, and Apple Books.

## Tool Cross-Check

This skill captures known patterns. If you need to verify, go deeper, or something seems
stale, use the `lib-research` skill (btca for source, Context7 for docs).

## Internal Docs

- `docs/external-apis/README.md` (navigation)
- `docs/external-apis/mp4ameta.md`
- `docs/external-apis/ffmpeg-next.md`

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
| Metadata writing | `src-tauri/src/metadata/ffmpeg_bridge.rs` | `metadata_to_ffmpeg_dict()` for tag mapping |
| Data model | `src-tauri/src/metadata/mod.rs` | `AudiobookMetadata` fields (`series`, `series_part`) |
| Metadata reading | `src-tauri/src/metadata/reader.rs` | `read_metadata()` routing (ffmpeg vs mp4ameta) |
| Output path | `src-tauri/src/audio/output_path.rs` | `build_output_path()` for folder structure |
| TSOA computation | `src-tauri/src/commands/metadata.rs` | `compute_tsoa()` for album sort |

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
