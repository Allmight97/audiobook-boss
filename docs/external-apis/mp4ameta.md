## mp4ameta Integration Guide (iTunes-style MPEG-4 metadata)

This guide documents how `audiobook-boss` uses the `mp4ameta` crate to handle M4B/M4A metadata specifically for audiobook-compatible tagging (Apple Books, Audiobookshelf).

### Where used
- `src-tauri/src/metadata/mp4ameta_bridge.rs` (exclusive handler for MP4 containers)

### Core Patterns

#### Reading Tags
```rust
let tag = Tag::read_from_path(path).map_err(|e| AppError::General(format!("read failed: {e}")))?;

// Standard fields
let title = tag.title();
let artwork = tag.artwork(); // Img<Vec<u8>>
```

#### Writing Tags (The "Configured" Way)
We use a specific `WriteConfig` to ensure `meta` items are written (important for series tags).

```rust
let config = WriteConfig {
    write_meta_items: true,
    ..WriteConfig::NONE
};

tag.write_with_path(path, &config)?;
```

### Custom Audiobook Fields (Series)
MPEG-4 doesn't have native "Series" fields, so we use `com.apple.iTunes` freeform fields and "Movement" (used by Apple Books).

- **Series Name**: Freeform `SERIES` + `Movement` name.
- **Series Part**: Freeform `SERIES-PART` + `Movement Index`.

```rust
const ITUNES_MEAN: &str = "com.apple.iTunes";

// Writing Series
let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
tag.set_data(ident, Data::Utf8(series_name));
tag.set_movement(series_name); // Apple Books visible

// Writing Series Part
let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
tag.set_data(ident, Data::Utf8(part_string));
tag.set_movement_index(index_u16);
tag.set_show_movement(); // Required flag
```

### Description vs Comment
`description` maps to the long-form synopsis (`desc` atom). `comment` is a shorter free-form note (`©cmt`).
Audiobook Boss only surfaces `description`; we intentionally do not write `comment` today.

### Image Formats
`mp4ameta` is picky about image formats. We must detect and map them correctly.
- Support: **JPEG**, **PNG**.
- No support: **WebP** (at the container tag level).

```rust
let img = match format {
    CoverFormat::Jpeg => Img::new(ImgFmt::Jpeg, bytes),
    CoverFormat::Png => Img::new(ImgFmt::Png, bytes),
};
tag.set_artwork(img);
```

### References
- [docs.rs – mp4ameta](https://docs.rs/mp4ameta/latest/mp4ameta/)
- [GitHub – mp4ameta](https://github.com/SlightlySane/mp4ameta)
