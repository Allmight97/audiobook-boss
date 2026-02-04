---
name: mp4ameta-patterns
description: "mp4ameta Rust crate patterns for audiobook-boss. Use when reading/writing MP4/M4B/M4A metadata, handling series tags, cover art, or iTunes freeform atoms. Covers Tag API, FreeformIdent usage, dual-write strategy for ABS/Apple Books compatibility, and WriteConfig options."
---

# mp4ameta Patterns for audiobook-boss

This skill captures project-specific patterns for the `mp4ameta` crate (v0.13.0). Consult before modifying MP4/M4B metadata handling.

## Tool Cross-Check

This skill captures known patterns. If you need to verify, go deeper, or something seems
stale, use the `lib-research` skill (btca for source, Context7 for docs).

## Why mp4ameta?

ffmpeg-next cannot reliably write iTunes freeform atoms (`----:com.apple.iTunes:SERIES`) that Audiobookshelf requires. mp4ameta provides native Rust access to MP4 atom structure for reliable series tag writing.

## Crate Import Convention

```rust
use mp4ameta::{Data, FreeformIdent, Img, ImgFmt, MediaType, Tag, WriteConfig};
```

## Codebase Location

| Component | Location | Purpose |
|-----------|----------|---------|
| Bridge module | `src-tauri/src/metadata/mp4ameta_bridge.rs` | Read/write via mp4ameta |
| Integration | `src-tauri/src/metadata/reader.rs` | Route MP4 containers to mp4ameta |
| Test | `src-tauri/tests/mp4ameta_series_tags.rs` | Series tag verification |

## Container Detection

```rust
pub fn is_mp4_container(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    matches!(ext.to_ascii_lowercase().as_str(), "m4a" | "m4b" | "mp4")
}
```

Use this to route MP4 containers to mp4ameta, with ffmpeg-next as fallback.

## Reading Metadata

```rust
use mp4ameta::Tag;

let tag = Tag::read_from_path(path)
    .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

// Standard tags (direct accessors)
let title = tag.title().map(str::to_string);
let artist = tag.artist().or_else(|| tag.album_artist()).map(str::to_string);
let composer = tag.composer().map(str::to_string);
let album = tag.album().map(str::to_string);
let genre = tag.genre().map(str::to_string);
let year = tag.year().and_then(|y| y.parse::<u32>().ok());

// Cover art
let cover_bytes = tag.artwork().map(|img| img.data.to_vec());
```

## Reading Series Tags (Dual-Source)

Series data may exist in iTunes freeform atoms OR movement tags. Check both:

```rust
const ITUNES_MEAN: &str = "com.apple.iTunes";

fn read_series(tag: &Tag) -> Option<String> {
    // Primary: iTunes freeform atom
    let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
    let series = tag.strings_of(&ident).next().map(str::to_string);

    // Fallback: movement tag (Apple Books)
    series.or_else(|| tag.movement().map(str::to_string))
}

fn read_series_part(tag: &Tag) -> Option<String> {
    let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
    let part = tag.strings_of(&ident).next().map(str::to_string);

    // Fallback: movement index
    part.or_else(|| tag.movement_index().map(|idx| idx.to_string()))
}
```

## Writing Metadata

### Basic Pattern

```rust
let mut tag = Tag::read_from_path(path)?;

// Set standard fields
tag.set_title("Book Title");
tag.set_artist("Author Name");
tag.set_album_artist("Author Name");  // Mirror for compatibility
tag.set_composer("Narrator Name");
tag.set_album("Book Title");
tag.set_genre("Fiction");
tag.set_year("2024");
tag.set_description("Book synopsis...");

// Mark as audiobook (stik=2)
tag.set_media_type(MediaType::AudioBook);
```

### WriteConfig (Critical)

```rust
let config = WriteConfig {
    write_meta_items: true,  // Required for freeform atoms
    ..WriteConfig::NONE
};

tag.write_with_path(path, &config)?;
```

**Always use `write_meta_items: true`** — without it, iTunes freeform atoms won't be written.

## Series Tags: Dual-Write Strategy

Write BOTH formats for universal compatibility:

```rust
const ITUNES_MEAN: &str = "com.apple.iTunes";

// For ABS/Plex (ffprobe-readable)
if let Some(ref series) = metadata.series {
    let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
    tag.set_data(ident, Data::Utf8(series.to_string()));

    // Also write movement for Apple Books
    tag.set_movement(series);
    tag.set_show_movement();
}

if let Some(ref series_part) = metadata.series_part {
    let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
    tag.set_data(ident, Data::Utf8(series_part.to_string()));

    // Movement index (must be u16, non-zero)
    if let Some(index) = parse_series_part(series_part) {
        tag.set_movement_index(index);
        tag.set_show_movement();
    }
}

fn parse_series_part(value: &str) -> Option<u16> {
    let raw = value.split('/').next()?.trim();  // Handle "1/5" format
    let parsed = raw.parse::<u16>().ok()?;
    if parsed == 0 { None } else { Some(parsed) }
}
```

### Tag Mapping Summary

| Field | ABS/Plex (freeform) | Apple Books (movement) |
|-------|---------------------|------------------------|
| Series name | `SERIES` atom | `set_movement()` |
| Book number | `SERIES-PART` atom | `set_movement_index()` |

## Cover Art Embedding

```rust
use mp4ameta::{Img, ImgFmt};

fn cover_art_to_img(bytes: &[u8]) -> Option<Img<Vec<u8>>> {
    // Detect format from magic bytes
    let format = detect_cover_art_format(bytes)?;

    match format {
        CoverFormat::Jpeg => Some(Img::new(ImgFmt::Jpeg, bytes.to_vec())),
        CoverFormat::Png => Some(Img::new(ImgFmt::Png, bytes.to_vec())),
    }
}

// Apply to tag
if let Some(image) = cover_art_to_img(cover_bytes)? {
    tag.set_artwork(image);
}
```

**Note**: mp4ameta only supports JPEG and PNG. Other formats must be converted first.

## FreeformIdent API

For custom iTunes atoms:

```rust
use mp4ameta::{Data, FreeformIdent};

// Create identifier
let ident = FreeformIdent::new_static("com.apple.iTunes", "CUSTOM_TAG");

// Write string data
tag.set_data(ident.clone(), Data::Utf8("value".to_string()));

// Read string data
let value = tag.strings_of(&ident).next();

// Read raw data
let data = tag.data_of(&ident).next();
```

### Common iTunes Atoms

| Atom Name | Purpose |
|-----------|---------|
| `SERIES` | Series name (ABS primary) |
| `SERIES-PART` | Book number (ABS primary) |
| `NARRATOR` | Narrator (redundant with composer) |
| `ASIN` | Amazon identifier |
| `ISBN` | Book ISBN |

## Integration with ffmpeg-next

mp4ameta handles post-encoding metadata. The workflow is:

1. **ffmpeg-next**: Creates M4B container with audio stream
2. **mp4ameta**: Writes metadata to existing file

```rust
// In reader.rs - routing logic
if mp4ameta_bridge::is_mp4_container(path) {
    match mp4ameta_bridge::read_metadata(path) {
        Ok(metadata) => {
            // Use mp4ameta result, fall back to ffmpeg for missing fields
            // (series, series_part, cover_art may need ffmpeg fallback)
        }
        Err(_) => read_metadata_with_ffmpeg(path),
    }
}
```

## Error Handling

```rust
use crate::errors::{AppError, Result};

Tag::read_from_path(path)
    .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

tag.write_with_path(path, &config)
    .map_err(|e| AppError::General(format!("mp4ameta write failed: {e}")))?;
```

## Verification

After writing, verify with both tools:

```bash
# Verify ABS-compatible tags (series/series-part visible)
ffprobe -v quiet -print_format json -show_format output.m4b | jq '.format.tags'

# Verify movement tags for Apple Books
# (ffprobe won't show these)
```

Or in Rust tests:

```rust
let tag = Tag::read_from_path(&output).expect("read tag");

// Check freeform atoms
let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
assert_eq!(tag.strings_of(&series_ident).next(), Some("Series Name"));

// Check movement tags
assert_eq!(tag.movement(), Some("Series Name"));
assert_eq!(tag.movement_index(), Some(1));
```

## Tag API Quick Reference

### String Metadata (getter/setter/remove pattern)

| Method | Atom | Notes |
|--------|------|-------|
| `title()` / `set_title()` | `©nam` | Book title |
| `artist()` / `set_artist()` | `©ART` | Author |
| `album_artist()` / `set_album_artist()` | `aART` | Author (mirror) |
| `album()` / `set_album()` | `©alb` | Album/book title |
| `composer()` / `set_composer()` | `©wrt` | Narrator |
| `genre()` / `set_genre()` | `©gen` | Genre |
| `year()` / `set_year()` | `©day` | Publication year |
| `comment()` / `set_comment()` | `©cmt` | Short comment |
| `description()` / `set_description()` | `desc` | Synopsis |
| `movement()` / `set_movement()` | `©mvn` | Series (Apple Books) |
| `album_sort_order()` / `set_album_sort_order()` | `soal` | TSOA sort field |

### Numeric/Flag Methods

| Method | Type | Notes |
|--------|------|-------|
| `movement_index()` / `set_movement_index()` | `u16` | Book # (1-65535) |
| `movement_count()` / `set_movement_count()` | `u16` | Total in series |
| `show_movement()` / `set_show_movement()` | `bool` | Show movement info |
| `media_type()` / `set_media_type()` | `MediaType` | Use `AudioBook` |
| `track_number()` / `set_track_number()` | `u16` | Track in album |
| `disc_number()` / `set_disc_number()` | `u16` | Disc number |

### Data Access Methods

| Method | Purpose |
|--------|---------|
| `strings_of(&ident)` | Iterator over string values for identifier |
| `data_of(&ident)` | Iterator over raw Data values |
| `set_data(ident, data)` | Set single value for identifier |
| `add_data(ident, data)` | Append value for identifier |
| `remove_data_of(&ident)` | Remove all values for identifier |

### MediaType Enum Values

```rust
pub enum MediaType {
    Movie,
    Normal,      // Music
    AudioBook,   // stik=2 - USE THIS
    MusicVideo,
    TVShow,
    Booklet,
    Ringtone,
}
```

### ReadConfig / WriteConfig

```rust
// Selective reading (skip image data for speed)
let read_cfg = ReadConfig {
    read_meta_items: true,
    read_image_data: false,
    ..ReadConfig::NONE
};

// Selective writing (only metadata, preserve chapters)
let write_cfg = WriteConfig {
    write_meta_items: true,
    ..WriteConfig::NONE
};
```

## Common Gotchas

1. **WriteConfig::NONE by default**: Must explicitly set `write_meta_items: true` for freeform atoms.

2. **Movement index is u16**: Values must be 1-65535. Zero is treated as "not set".

3. **Series-part parsing**: Handle "1/5" format by splitting on `/` before parsing.

4. **File must exist**: `Tag::read_from_path` fails on non-existent files. Create container first with ffmpeg-next.

5. **No WebP support**: Cover art must be JPEG or PNG.

6. **set_show_movement()**: Call after setting movement or movement_index to ensure visibility.

7. **year() returns String**: Parse with `.and_then(|y| y.parse::<u32>().ok())`.

## References

### External Documentation
- `docs/external-apis/mp4ameta.md`
- [docs.rs: mp4ameta 0.13.0](https://docs.rs/mp4ameta/0.13.0/mp4ameta/) - Full API reference
- [docs.rs: Tag struct](https://docs.rs/mp4ameta/0.13.0/mp4ameta/struct.Tag.html) - All Tag methods
- [GitHub: Saecki/mp4ameta](https://github.com/Saecki/mp4ameta) - Source and README
- [crates.io: mp4ameta](https://crates.io/crates/mp4ameta) - Crate info

### Related Documentation
- [AtomicParsley Doc](http://atomicparsley.sourceforge.net/mpeg-4files.html) - MP4 atom reference
- [Mutagen Doc](https://mutagen.readthedocs.io/en/latest/api/mp4.html) - Python MP4 handling (reference)
- [Apple QuickTime Metadata](https://developer.apple.com/library/archive/documentation/QuickTime/QTFF/Metadata/Metadata.html) - Official atom spec

### Codebase
- [mp4ameta_bridge.rs](src-tauri/src/metadata/mp4ameta_bridge.rs) - Bridge implementation
- [reader.rs](src-tauri/src/metadata/reader.rs) - Integration with ffmpeg fallback
- [mp4ameta_series_tags.rs](src-tauri/tests/mp4ameta_series_tags.rs) - Series tag test
- [Skill: audiobook-metadata](../audiobook-metadata/SKILL.md) - Tag mapping and ABS compatibility
