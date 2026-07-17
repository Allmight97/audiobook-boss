# Metadata Boundary

For ABS/Plex/Apple tag-mapping, series-tag strategy, and folder conventions, use the `audiobook-metadata` skill.

## Public API Strip
- Import metadata boundary symbols from `crate::metadata`, not private child modules.
- Intent symbols: `MetadataIntentPatch`, `PatchOp`, `AlbumSortPatchOp`,
  `MetadataIntentValidationResult`, `validate_metadata_intent_patch`.
- Outcome symbols: `MetadataOutcomeRequest`, `MetadataOutcomePlan`,
  `NamingMetadata`, `CoverArtPassthroughPolicy`, `plan_metadata_outcome`,
  plus the test-only write-plan contract helper `plan_metadata_write`.
- Thumbnail symbol: `read_audio_cover_thumbnail` (reads embedded art without adapting metadata
  intent; re-exported at the crate root for the media-execution integration proof).
- Read/write symbols: `read_metadata`, `save_metadata_intent`,
  `finalize_artifact_metadata` (all also re-exported at the crate root for
  external integration tests, e.g. the media-execution lane's tag round-trip
  and artifact-finalize proofs; the lane also uses the crate-root re-exports
  of `extract_passthrough_metadata` + `PassthroughSource` to assert chapter
  truth on real artifacts). `finalize_artifact_metadata` is the
  container-aware external-adapter handoff: remux carries chapters/cover, then
  MP4-family tag truth is rewritten via mp4ameta. The FFmpeg mov muxer
  silently drops dict keys outside its known-atom table (series, series-part,
  freeform mirrors, sort_album), so MP4-family tag truth must not depend on a
  bare remux.
- Passthrough symbols: `PassthroughSource`, `PassthroughMetadata`,
  `extract_passthrough_metadata`, `add_chapters_to_output`. Private modules:
  `passthrough`, `mp4ameta_bridge`. Audio maps `AudioFile` → `PassthroughSource`
  at call sites; metadata must not import `crate::audio::AudioFile`.
- Crate-local write-plan support: `MetadataWritePlan`, `AlbumSortWriteAction`.
- Pure intent, validation, naming, and write-plan facts are packaged in
  `abb-metadata-core`; `src-tauri/src/metadata` owns container adapters and
  runtime file behavior.
- Series-family validation is effective-metadata aware: inherited odd tags from
  external files do not block unrelated intent, but touched series/subseries
  intent must produce a round-trippable shape before save or processing.
  Real file saves route through `save_metadata_intent`, not raw write-plan
  construction, so this validation can see source metadata.

## Private Cluster
- Files: `intent_plan.rs`, `contract_tests.rs`, `field_schema.rs`, `metadata_ops.rs`,
  `metadata_sinks.rs`; `mod.rs` owns the public re-export strip.
- `field_schema` + `metadata_ops` own container-neutral tag mapping, read aliases,
  clear groups, and field op planning (fan-outs, track/disk tuples). Container
  adapters (`ffmpeg_dict`, `mp4ameta_bridge`, `reader`) apply ops only.
- `cover_art` and `media_type` stay sink-owned outside the neutral field-op loop
  (encode-path embed vs mp4 artwork; unconditional audiobook media type).
- `AlbumSortWriteAction` stays on `MetadataWritePlan`; do not fold album_sort into
  generic field set/clear ops.
- Track/disk are read-compatible passthrough fields: excluded from
  `MetadataIntentPatch`, but written when present on full `AudiobookMetadata`.
- The broader metadata boundary owns passthrough, cover-art handling, remux helpers,
  and container routing. `tag_registry.rs` retains series constants folded by
  `field_schema`.

## Allowed Agent Edits Without Escalation
- Change pure intent internals when `cargo nextest run -p abb-metadata-core` stays green.
- Change runtime/container adapters when targeted `audiobook-boss` Nextest and
  Public API Strip checks stay green.
- Preserve `set | clear | noop` semantics across save, processing projection,
  naming projection, write plans, validation/normalization, and cover-art
  handling.
- Preserve external audiobook tag interoperability.
- Drop FFmpeg probe/remux contexts before calling mp4ameta on the same path or replacing the source file.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Making clear intent partial, lossy, or dependent on sentinel frontend values.
- Moving publication-date or series/subseries sequence validation out of the
  metadata boundary.
- Changing canonical/mirrored/compatibility tag precedence, provider-degradation
  contract, or container routing without explicit evidence and doc updates.
