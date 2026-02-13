# M4B Tag Mapping Reference

Complete mapping of MP4 atoms to Audiobookshelf, Plex, and Apple Books fields.

## Table of Contents

1. [Standard Tags](#standard-tags)
2. [Series Tags (Critical)](#series-tags-critical)
3. [iTunes Custom Atoms](#itunes-custom-atoms)
4. [Writing with ffmpeg](#writing-with-ffmpeg)
5. [Writing with ffmpeg-next (Rust)](#writing-with-ffmpeg-next-rust)

---

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

### For ABS and Plex (ffprobe-readable)

| Atom | ffmetadata key | Purpose |
|------|----------------|---------|
| `----:com.apple.iTunes:SERIES` | `series` | Series name |
| `----:com.apple.iTunes:SERIES-PART` | `series-part` | Book number |

### For Apple Books (movement tags)

| Atom | ffmetadata key | Purpose |
|------|----------------|---------|
| `©mvn` | `MVNM` | Movement name (series) |
| `©mvi` | `MVIN` | Movement number (book #) |

**Both sets must be written for universal compatibility.**

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

### Movement tags (MVNM/MVIN)

**Warning**: ffmpeg's handling of movement tags is inconsistent. The standard `-metadata` approach may not work. Options:

1. Use AtomicParsley as a post-processing step
2. Write via ffmpeg-next with custom atom handling

## Writing with ffmpeg-next (Rust)

audiobook-boss uses ffmpeg-next bindings with a dual-write strategy so ABS/Plex and Apple Books both resolve series metadata.

```rust
// Primary + mirrored keys written together
dict.set("series", series_name);
dict.set("----:com.apple.iTunes:SERIES", series_name);
dict.set("MVNM", series_name);

dict.set("series-part", book_number);
dict.set("----:com.apple.iTunes:SERIES-PART", book_number);
dict.set("MVIN", book_number);
```

`MVIN` should be a plain positive integer string. Slash-form values like `1/5` are rejected by validation and are not mirrored to movement index.

Validation checklist:
1. Creating an M4B with these tags
2. Running `ffprobe -show_format output.m4b`
3. Confirming `series` and `series-part` appear in the tags (plus the freeform atom names)

Legacy read compatibility remains (`show` / `episode_sort`) for older files, but write paths should use the dual-write keys above.

---

Source: [Audiobookshelf Docs - Audio Metadata](https://www.audiobookshelf.org/docs/#book-audio-metadata)
