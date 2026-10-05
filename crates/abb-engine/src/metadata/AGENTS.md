# Metadata Boundary

For ABS/Plex/Apple tag-mapping, series-tag strategy, and folder conventions, use the `audiobook-metadata` skill.

## Public API Strip

- Import metadata boundary symbols from `crate::metadata`; the strip is the
  `pub use` list in `metadata/mod.rs`. `passthrough` and `mp4ameta_bridge` stay
  private.
- The app reads tags from session snapshots and changes them through session
  intents; `abb-dev` reads through `Engine::read_audio_metadata`. Real file
  saves route through `save_metadata_intent`, so series validation can see
  source metadata.
- Audio maps `AudioFile` to `PassthroughSource` at call sites; metadata never
  imports `crate::audio::AudioFile`.
- `finish_artifact_tags` is the one last step after any mux or remux (native or
  preserve): MP4-family tag rewrite through mp4ameta, then accepted-chapter
  verification. Its pre-write callback lets Audio report progress, so Audio
  never asks which container strategy applies. The FFmpeg mov muxer silently
  drops dict keys outside its known-atom table (series, series-part, freeform
  mirrors, sort_album), so MP4-family tag truth never depends on a bare remux.
  `finalize_artifact_metadata` is the same finish for a preserved merge, after
  a remux that carries chapters and cover.
- Remux owns its sibling `.abb_meta_*` output from creation until it replaces
  the source; any failure removes it after FFmpeg's handles close.
- Pure intent, validation, naming, and write-plan facts live in
  `abb-metadata-core`; this directory owns container adapters and runtime file
  behavior.
- Series-family validation is effective-metadata aware: inherited odd tags from
  external files do not block unrelated intent, but touched series/subseries
  intent must produce a round-trippable shape before save or processing.

## Private Cluster

- `field_schema` + `metadata_ops` own container-neutral tag mapping, read
  aliases, clear groups, and field op planning (fan-outs, track/disk tuples).
  Container adapters (`ffmpeg_dict`, `mp4ameta_bridge`, `reader`) apply ops
  only.
- `cover_art` and `media_type` stay sink-owned outside the neutral field-op loop
  (encode-path embed vs mp4 artwork; unconditional audiobook media type).
  `prepare_output_cover_art` prepares write-ready cover bytes once after merge:
  JPEG already at the write target (JPEG, 800px or smaller) stays as is; other
  covers go through `optimize_cover_art` before native mux, remux, or
  mp4ameta. A cover is converted in exactly two places: here, and in
  `cover_source.rs` when the user loads a file or URL cover. Covers embedded in
  source files stay unconverted until write-prep, so displays show the source's
  real bytes.
- Cover intent comes only from cover actions (load, drop, clear). A text edit
  or Lookup apply that keeps the cover leaves cover intent absent, so a save
  does not rewrite unchanged art.
- Processed outputs carry the album sort (TSOA) from `processing_album_sort`:
  derived from series, book # and title when possible (fractions kept, so
  novellas sort between books), else the source's own; explicit album-sort
  intent wins. Every processing route takes it from `plan_metadata_outcome`
  (effective metadata, or `write_intent` for single-file preserve). Metadata
  saves preserve TSOA unless explicitly changed, because a save edits the user's
  own source file.
- Keep `AlbumSortWriteAction` on `MetadataWritePlan` as its own action, apart
  from generic field set/clear ops.
- Track/disk are read-compatible passthrough fields: the form never edits them,
  `MetadataIntentPatch` carries them only as explicit artifact set/clear intent,
  and full `AudiobookMetadata` writes them when present.
- Thumbnail ingestion stays allocation-bounded before decode or demux:
  `mp4_covr` bounds MP4 payload, nesting, and atom traversal; `embedded_cover`
  reads bounded ID3/FLAC/WAVE picture records without opening FFmpeg.
  `matroska_cover` bounds EBML traversal and cover payload allocation; its
  recognized cover filenames/MIME types also govern attachment replacement.
  Other attachments survive metadata edits. Unknown non-MP4 tag containers
  return no thumbnail; no unbounded demux fallback exists.

## Edit Rules

- When metadata policy crosses planning and writing, prove the changed handoff
  through the production processing planner and read tags from actual output
  files on each affected route. Direct engine tests with supplied metadata
  cannot prove planner forwarding. Also prove source tags stay untouched by
  processing and retain Save's distinct policy.
- Keep `set | clear | absent` field semantics (absent keeps the source value)
  across save, processing projection, naming projection, write plans,
  validation/normalization, and cover-art handling. Clear intent is the
  explicit blank field action, never a sentinel value.
- Publication-date and series/subseries sequence validation stay in this
  boundary. Changes to tag precedence (canonical, mirrored, compatibility),
  the provider-degradation contract, or container routing carry evidence and
  update this file.
- FFmpeg COMM reads prefer `comment`, then undescribed language keys, then
  described comments, with lexical key order breaking ties. Explicit comment
  set/clear removes those user-comment aliases; an absent field preserves them.
  `iTun*` COMM descriptors are technical passthrough records, never display or
  clear candidates. This policy lives in `field_schema::comment_key_rank`.
- MP4 artist and composer reads join repeated values with `;`, matching
  FFmpeg's text projection. Artist values take precedence over album-artist
  fallback. Unrelated save intent preserves repeated source values; processing
  and explicit contributor writes use the singular text model (artist also fans
  out to album-artist). Treat each name list as one text value; it carries no
  structural or distinct album-artist preservation.
- Drop FFmpeg probe/remux contexts before calling mp4ameta on the same path or
  replacing the source file.

## CUE And Accepted Chapters

- `abb-metadata-core` owns `parse_cue`, `validate_chapters`, `ChapterSpec`,
  `CueInterpretation`, and `CueSheet`. CUE conversion rounds the checked
  rational timestamp once to the nearest millisecond. Runtime `cue.rs` owns
  bounded sibling discovery, diagnostics, and source fingerprints; FILE text
  never chooses another audio path.
- `ChapterPlan`, `CueSource`, and `CueStatus` carry intake facts. Audio consumes
  crate-local `inspect_chapter_source`, `validate_chapter_plan`, and
  `validate_source_fingerprint`; the last compares metadata from an already-open
  source handle with the fingerprint captured during inspection. ABB's own tag
  write moves a source's fingerprint forward only when the file still matched
  it just before the write (`metadata_save.rs` reports both), so a change made
  outside ABB is still refused.
- `PassthroughSource.chapters` carries accepted data when present; `None` is
  container discovery for artifact readers. `verify_chapters` checks names,
  starts, ends, and count after final metadata writes and before artifact
  commit. Selected chapter write/readback failures propagate.

## Opus Container Timing

MP4 output uses the `mp4` muxer explicitly (`ipod` rejects Opus). Matroska covers
are attachments with a filename and MIME type. Retagging reuses the selected
cover bytes, preserving explicit-clear intent. For Opus Matroska→MP4 copies,
packet framing restores the 48 kHz timeline lost to millisecond container
rounding; discontinuities fail rather than discard audio. MP4 frame-size metadata
must allow the muxer to honor trailing discard padding. MP3 sources without
trim metadata stay free of an invented Xing decoder delay during remux.
