---
name: mp4ameta-patterns
description: Implementation patterns for mp4ameta usage in Audiobook Boss. Use for MP4/M4B tag IO, freeform atoms, movement tags, and cover-art writes.
---

# mp4ameta Patterns

Use this skill for `mp4ameta` implementation details only.

Strategy ownership for cross-ecosystem behavior lives in `audiobook-metadata`.

## Scope

Use when editing:
- `src-tauri/src/metadata/mp4ameta_bridge.rs`
- MP4/M4B read/write routing in metadata readers
- freeform atom or movement-tag implementation logic
- cover-art embedding via mp4ameta

## Preferred Path

1. Open tag with `Tag::read_from_path`.
2. Apply standard fields via typed setters.
3. Apply series atoms/movement mirrors per `audiobook-metadata` strategy.
4. Write using `WriteConfig { write_meta_items: true, ..WriteConfig::NONE }`.
5. Re-read and verify expected atoms/fields in tests.

## Hard Invariants

- `write_meta_items: true` is required for freeform atom persistence.
- MP4 container routing stays explicit (`m4a|m4b|mp4`).
- `movement_index` mirrors only positive `u16` values.
- Cover art writes use supported formats (JPEG/PNG).

## Minimal Pattern

```rust
use mp4ameta::{FreeformIdent, Tag, WriteConfig};

let mut tag = Tag::read_from_path(path)?;
let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
tag.set_data(series_ident, mp4ameta::Data::Utf8(series_name.to_string()));

let cfg = WriteConfig { write_meta_items: true, ..WriteConfig::NONE };
tag.write_with_path(path, &cfg)?;
```

## Verification

- Add/update metadata integration tests in `src-tauri/tests/integration_metadata_tests.rs`.
- Verify freeform + movement mirrors where series fields are present.
- Preserve compatibility assertions owned by `audiobook-metadata`.

## Pointers

- `src-tauri/src/metadata/mp4ameta_bridge.rs`
- `src-tauri/src/metadata/reader.rs`
- `src-tauri/tests/integration_metadata_tests.rs`
- `../audiobook-metadata/SKILL.md`
- `docs/external-apis/mp4ameta.md`

## Done Criteria

- MP4 metadata writes are stable and test-covered.
- Implementation stays consistent with canonical metadata strategy.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
- For library/API uncertainty, invoke `lib-research`.
