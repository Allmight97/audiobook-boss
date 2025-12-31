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

---

## Implementation plan (finalized)

Recommendation: implement multi-select + bulk metadata apply in the frontend using
a selection set with a primary selection, dirty-field tracking, and a Cmd+S loop
that reuses `save_metadata_to_file`. Outcome: faster series imports with minimal
backend risk if single-select remains the primary path.

Tri-order impact:
- 1st: Bulk edits speed up metadata entry.
- 2nd: Cmd+S expands to multi-file save loop; avoid single-select regressions.
- 3rd: Establishes pattern for future bulk ops (cover art/settings).

Blast radius (likely files):
- `src/ui/fileList/state.ts`
- `src/ui/fileList/actions.ts`
- `src/ui/fileList/events.ts`
- `src/ui/fileList/dom.ts`
- `src/ui/fileList/index.ts`
- `src/main.ts`
- `src/ui/statusPanel/processing.ts`
- `src/ui/metadataState.ts`
- `src/ui/metadataForm.ts`
- `index.html`
- `src/styles.css`
- New: `src/ui/fileList/selection.ts`
- Tests: `src/ui/__tests__/*`

Plan:
1) Confirm current selection + metadata save flows and touchpoints.
2) Add selection set + primary selection; keyboard range/select-all handling.
3) Add dirty-field tracking + multi-select form behavior.
4) Cmd+S applies to all selected valid files via `save_metadata_to_file`.
5) Add tests + manual QA checklist.

UX intent (basic):
- Keep metadata form editable for all fields during multi-select.
- Add minimal awareness UI (selection count + inline hint + status text).
- Optional "Apply to Selected" button mirrors Cmd+S behavior.

Dirty field behavior (decided):
- Empty fields = "no change" (avoids accidental data loss).
- If user clears a field, treat it the same as untouched—no wipe.

Invalid files:
- If files are invalid (`isValid === false`), keep them selectable for visibility,
  but skip them during bulk apply/save and report a "skipped invalid" count.

Confirmation dialog (decided):
- Skip confirmation for Cmd+S (keyboard shortcut implies intent).
- Toast summary after save is sufficient feedback.
