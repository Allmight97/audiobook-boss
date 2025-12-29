## Issue 109 scratchpad

Context: tag inventory for M4B writes so we can assess blast radius before bulk metadata apply.

### Cmd+S (metadata-only save on M4B)
Writer: `mp4ameta_bridge::write_metadata` in `src-tauri/src/metadata/mp4ameta_bridge.rs`.

Tags written when the field is present:
- `title` (©nam)
- `artist` (©ART) + `album_artist` (aART)
- `album` (©alb)
- `composer` (©wrt)
- `genre` (©gen)
- `year` (©day) from `date`
- `comment` (©cmt)
- `description` (desc) — removed if empty
- `album_sort_order` (soal) from `album_sort` (TSOA)
- `media_type` (stik=2)
- `SERIES` (freeform `----:com.apple.iTunes:SERIES`)
- `SERIES-PART` (freeform `----:com.apple.iTunes:SERIES-PART`)
- `movement` + `movement_index` + `show_movement` (Apple Books movement fields)
- `artwork` (covr) — removed if empty

Note: we do not write `show` or `episode_sort` here; if those exist they remain unless explicitly removed.

### Muxing/encoding path (new file output)
Phase 1: ffmpeg container metadata (during encoding).
Writer: `set_container_metadata` → `metadata_to_ffmpeg_dict` in `src-tauri/src/metadata/ffmpeg_dict.rs`.

Tags written:
- `title`, `artist`, `album_artist`, `album`, `composer`, `genre`
- `date` and `year`
- `comment`
- `description`
- `series`, `series-part`
- `----:com.apple.iTunes:SERIES`, `----:com.apple.iTunes:SERIES-PART`
- `MVNM`, `MVIN`
- `sort_album` (TSOA)
- `media_type=2`

Phase 2: post-mux for M4B only.
Writer: `write_metadata_stage` → `mp4ameta_bridge::write_metadata` in `src-tauri/src/audio/processor/finalize.rs`.
Same mp4ameta tags as the Cmd+S path.

### Read fallback (legacy compatibility)
Reader: `src-tauri/src/metadata/reader.rs`.
Reads `show` and `episode_sort` as fallback for series/series_part; we do not write them.
