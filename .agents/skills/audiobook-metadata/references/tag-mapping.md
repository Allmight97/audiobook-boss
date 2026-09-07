# Tag Interoperability

Read for changes to ABB's tag mapping or consumer compatibility. Paths below
are relative to the ABB root.

## Live Mapping Owners

| Question | Source to inspect |
| --- | --- |
| Supported fields, read aliases, clear groups, FFmpeg keys | `src-tauri/src/metadata/field_schema.rs` |
| Series key families and precedence | `src-tauri/src/metadata/tag_registry.rs` |
| Field fan-outs, tuples, series/subseries projection | `src-tauri/src/metadata/metadata_ops.rs` |
| Actual container writes and atom-level tests | `src-tauri/src/metadata/metadata_sinks.rs` |
| MP4 reads, album sort, movement cleanup, artwork | `src-tauri/src/metadata/mp4ameta_bridge.rs` |
| Date and series validation/normalization | `crates/abb-metadata-core/src/lib.rs` |

The schema describes ABB's supported fields. A field recognized by an external
scanner is not evidence that ABB exposes or writes it. Preserve normalized
publication dates through the owning sink; do not infer year-only storage from
a container library method named `set_year`.

## Retained Series Strategy

Write ffprobe-visible `series` / `series-part` keys and both MP4 freeform
families under `com.apple.iTunes`:

- canonical lowercase `series` / `series-part`;
- uppercase interoperability mirrors `SERIES` / `SERIES-PART`.

Read precedence favors lowercase canonical values, then uppercase mirrors,
then legacy `show` / `episode_sort` values. Preserve legacy read compatibility;
new writes use the canonical/mirrored families. Series/subseries projection and
effective-metadata validation stay in their code owners above.

Apple movement tags (`MVNM`/`MVIN`) are not ABB's series read/write mechanism.
The MP4 bridge removes movement fields when series-family writes apply.
Adopting movement tags as a series mechanism requires player evidence and an
explicit product decision.

For MP4 finalization, follow `src-tauri/src/metadata/AGENTS.md`: its
container-aware handoff owns preservation of tags the mov muxer drops. A bare
FFmpeg command setting arbitrary dictionary keys is not equivalent evidence.

## Consumer Evidence

For ABS mapping, consult the current
[File Metadata documentation](https://audiobookshelf.org/docs/documentation/libraries/book-library/directory-structure/#file-metadata).
For disagreements between disk tags and an existing library entry, inspect the
[metadata priority settings](https://audiobookshelf.org/docs/documentation/libraries/book-library/book-metadata/#local-metadata-priority-order).
Folder and sidecar precedence can override audio tags.

Plex and Apple Books claims need the affected player's import evidence.
Distinguish ABB's chosen compatibility strategy from demonstrated support for
a particular consumer version.

`ffprobe -show_format` can collapse case-only freeform names. The atom-level
sink tests/readback establish whether both families exist; a single visible
series key does not. Report artifact evidence separately from player-import
evidence.
