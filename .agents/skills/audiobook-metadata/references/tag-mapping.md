# Tag Interoperability

Read for changes to ABB's tag mapping or consumer compatibility. Paths below
are relative to the ABB root.

## Live Mapping Owners

| Question | Source to inspect |
| --- | --- |
| Supported fields; per backend, read aliases, write keys, keys a Set or Clear drops; series key families and precedence | `crates/abb-engine/src/metadata/field_schema.rs` |
| Field fan-outs, tuples, series/subseries projection | `crates/abb-engine/src/metadata/metadata_ops.rs` |
| Actual container writes and atom-level tests | `crates/abb-engine/src/metadata/metadata_sinks.rs` |
| MP4 reads, movement cleanup, artwork | `crates/abb-engine/src/metadata/mp4ameta_bridge.rs` |
| FFmpeg album-sort keys (`sort_album`, `album-sort`) | `crates/abb-engine/src/metadata/field_schema.rs`; which one a write keeps: `ffmpeg_dict.rs` |
| Date and series validation/normalization | `crates/abb-metadata-core/src/lib.rs` |

The schema describes ABB's supported fields. A field recognized by an external
scanner is not evidence that ABB exposes or writes it. Preserve normalized
publication dates through the owning sink; do not infer year-only storage from
a container library method named `set_year`.

## Retained Series Strategy

Write ffprobe-visible `series` / `series-part` keys. MP4 files also get both
freeform families under `com.apple.iTunes`:

- canonical lowercase `series` / `series-part`;
- uppercase interoperability mirrors `SERIES` / `SERIES-PART`.

FFmpeg-written containers (MP3 ID3, FLAC/Opus comments, Matroska) carry only
`series` / `series-part`; an iTunes freeform name there is a stray tag (an ID3
TXXX frame, a Vorbis comment) no reader maps.

Album sort: MP4-family files carry the sort-album atom (FFmpeg key
`sort_album`) and Matroska carries a `SORT_ALBUM` tag (the same key; FFmpeg and
ABB compare tag keys ignoring case, and Matroska reports them upper case). MP3
carries the ID3v2.4 `TSOA` frame (FFmpeg key `album-sort`); `sort_album` there
would be a stray TXXX frame. ABB reads `album-sort` first, then `sort_album`,
so an MP3 tagged by an older ABB still reads. A Save that sets or
clears the sort replaces both keys on MP3; one that leaves the sort alone
keeps whatever keys the file has. Clearing a field removes its tag whatever
case the container reports.

Read precedence favors lowercase canonical values, then uppercase mirrors,
then legacy `show` / `episode_sort` values. Preserve legacy read compatibility;
new writes use the canonical/mirrored families. Series/subseries projection and
effective-metadata validation stay in their code owners above.

Apple movement tags (`MVNM`/`MVIN`) are not ABB's series read/write mechanism.
The MP4 bridge removes movement fields when series-family writes apply.
Adopting movement tags as a series mechanism requires player evidence and an
explicit product decision.

For MP4 finalization, follow `crates/abb-engine/src/metadata/AGENTS.md`: its
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
