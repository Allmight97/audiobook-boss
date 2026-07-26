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

## Writing with ffmpeg

### Basic metadata via ffmetadata file

```ini
;FFMETADATA1
title=Book Title
artist=Author Name
composer=Narrator Name
series=Series Name
series-part=1
genre=Science Fiction/Fantasy
```

Apply with:
```bash
ffmpeg -i input.m4b -i metadata.txt -map_metadata 1 -c copy output.m4b
```

### Direct command line

```bash
ffmpeg -i input.m4b \
  -metadata title="Book Title" \
  -metadata artist="Author Name" \
  -metadata composer="Narrator Name" \
  -metadata series="Series Name" \
  -metadata series-part="1" \
  -c copy output.m4b
```

## Writing with ffmpeg-next (Rust)

audiobook-boss writes canonical ffprobe-visible keys plus iTunes freeform mirrors
for series metadata. These are the series keys ABB currently owns in the FFmpeg
dictionary path.

```rust
// Primary + mirrored keys written together
dict.set("series", series_name);
dict.set("----:com.apple.iTunes:SERIES", series_name);

dict.set("series-part", book_number);
dict.set("----:com.apple.iTunes:SERIES-PART", book_number);
```

The mp4ameta path writes the same logical values to lowercase canonical
freeforms and uppercase iTunes mirrors. Keep both families unless player
evidence and an explicit product decision replace this compatibility strategy.

Series-part values should stay scanner-friendly. Slash-form values like `1/5`
are rejected by validation.

Validation checklist:
1. Creating an M4B with these tags
2. Running `ffprobe -show_format output.m4b`
3. Confirming `series` and `series-part` appear in the tags (plus the freeform atom names)

Legacy read compatibility remains (`show` / `episode_sort`) for older files,
but write paths should use the canonical and freeform keys above.

---

Source: [Audiobookshelf Docs - Audio Metadata](https://www.audiobookshelf.org/docs/#book-audio-metadata)
