# M4B Tag Mapping Reference

Current mapping of ABB-written MP4 metadata to Audiobookshelf, Plex, and Apple Books fields.

## Standard Tags

| MP4 Atom | ffmetadata key | ABS Field | Plex Field | Notes |
|----------|----------------|-----------|------------|-------|
| `©nam` | `title` | Title | Track Name | Book title |
| `©ART` | `artist` | Author | Artist | Primary author |
| `aART` | `album_artist` | Author | Album Artist | Fallback for author |
| `©alb` | `album` | Title | Album | Fallback for title |
| `©wrt` | `composer` | Narrator | Composer | **Use for narrator** |
| `©day` | `date` | Publish Year | Year | Year only |
| `©gen` | `genre` | Genres | Genre | Separate multiple with `/` or `;` |
| `desc` | `description` | Description | — | Synopsis |
| `©pub` | `publisher` | Publisher | — | Publisher name |
| `©cmt` | `comment` | — | — | General comments |

## Series Tags (Critical)

These are the tags that matter most for series detection.

### For ABS and Plex

| Storage | Key / Atom | Purpose |
|---------|------------|---------|
| ffprobe / ffmetadata | `series` | Series name |
| ffprobe / ffmetadata | `series-part` | Book number |
| mp4ameta canonical freeform | `----:com.apple.iTunes:series` | Series name |
| mp4ameta canonical freeform | `----:com.apple.iTunes:series-part` | Book number |
| iTunes compatibility freeform | `----:com.apple.iTunes:SERIES` | Series name |
| iTunes compatibility freeform | `----:com.apple.iTunes:SERIES-PART` | Book number |

Read precedence prefers canonical lowercase `series` / `series-part`, then the
uppercase iTunes mirrors, then legacy `show` / `episode_sort` fallbacks.
`ffprobe -show_format` collapses freeform names that differ only by case, so
unit tests around the mp4ameta sink are the proof that both lowercase canonical
and uppercase mirror atoms exist.

Apple movement tags (`MVNM`/`MVIN`) are not currently written or read as ABB's
series mechanism. Reintroduce them only with manual player evidence and an
explicit product decision.

## iTunes Custom Atoms

Additional freeform atoms recognized by ABS:

| Atom | ffmetadata key | ABS Field |
|------|----------------|-----------|
| `----:com.apple.iTunes:ASIN` | `asin` | ASIN |
| `----:com.apple.iTunes:AUDIBLE_ASIN` | `audible_asin` | ASIN |
| `----:com.apple.iTunes:ISBN` | `isbn` | ISBN |

## ABB Write Ownership

Container-neutral mappings and clear groups are owned by
`src-tauri/src/metadata/field_schema.rs` and
`src-tauri/src/metadata/metadata_ops.rs`. Container adapters apply those
operations. MP4-family artifact finalization is owned by
`finalize_artifact_metadata` and the mp4ameta sink.

Do not teach or use a bare `ffmpeg -c copy` metadata recipe as ABB's MP4 write
path. FFmpeg's MOV muxer drops dictionary keys outside its known atom table, so
series freeforms, mirrors, and other MP4 tag truth must be finalized through the
owning adapter.

Keep lowercase canonical freeforms and uppercase iTunes mirrors unless player
evidence and an explicit product decision replace this compatibility strategy.

Series-part values should stay scanner-friendly. Slash-form values like `1/5`
are rejected by validation.

Proof must match the observer-specific matrix in `../SKILL.md`. In particular,
`ffprobe` proves visible tags but cannot prove both case-distinct freeform atom
families.

Legacy read compatibility remains (`show` / `episode_sort`) for older files,
but write paths should use the canonical and freeform keys above.

---

Source: [Audiobookshelf Docs - Audio Metadata](https://www.audiobookshelf.org/docs/#book-audio-metadata)
