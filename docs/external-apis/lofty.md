## Lofty integration guide (MP4/M4B tags and cover art)

Scope: M4B (MP4 container). Other formats are out of scope for writes per product direction.

### Creating/ensuring a primary MP4 tag

- Freshly muxed MP4/M4B can have no tag. Ensure one exists before writing:

```rust
let mut tagged_file = Probe::open(path)?.read()?;
if tagged_file.primary_tag().is_none() {
    tagged_file.insert_tag(Tag::new(TagType::Mp4Ilst));
}
let tag = tagged_file.primary_tag_mut().ok_or_else(|| AppError::Metadata(...))?;
```

### Basic field mapping used here

- title → `set_title`
- author → `set_artist`
- album → `set_album`
- narrator → `AlbumArtist` (stored as item)
- date/year → `set_year`
- genre → `set_genre`
- description → `set_comment`

```rust
tag.clear();
// set fields based on provided metadata
tagged_file.save_to_path(path, Default::default())?;
```

### Cover art embedding (fallback path)

- If native FFmpeg attached_pic embedding wasn’t detected, Lofty fallback writes a `Picture`:

```rust
let picture = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, cover_bytes.to_vec());
tag.push_picture(picture);
tagged_file.save_to_path(path, Default::default())?;
```

MIME detection: simple magic-bytes detection for JPEG/PNG/GIF; WebP is treated as JPEG for compatibility.

### Safe/atomic write strategies (recommendations)

Lofty’s `save_to_path` writes in place. For higher safety we recommend:

Option A (temp file swap):
- Copy original → temp
- Apply Lofty writes on temp
- fsync temp; atomically rename temp over original (POSIX rename is atomic within same filesystem)
- On failure, keep original intact; clean temp

Option B (backup + in-place):
- Copy original → backup
- Apply writes on original
- On failure, log and restore from backup

Trade-offs:
- A increases peak disk usage (≈ 2x file size) but minimizes corruption risk
- B uses less peak space but risks short window where original is partially updated

We generally recommend Option A for M4B outputs if disk space allows.

### References

- Lofty (Rust): [docs.rs – lofty](https://docs.rs/lofty/latest/lofty/)
- MP4/M4A tag model (iTunes list): use `TagType::Mp4Ilst`


