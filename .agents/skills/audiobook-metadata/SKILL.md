---
name: audiobook-metadata
description: Canonical metadata strategy for Audiobook Boss. Use when changing tag mappings, metadata intent behavior, or ABS/Plex/Apple interoperability rules.
---

# Audiobook Metadata

Use this skill when changing metadata field semantics, tag mapping policy, interoperability behavior, or output naming that depends on metadata.

## Interop Invariant

External compatibility with ABS/Plex/Apple Books is required.

`ffprobe` does not expose movement tags (`MVNM`/`MVIN`). If series data is written only to movement tags, ABS/Plex scans miss it.

## Dual-Write Strategy

For series metadata, write both:
- ffprobe-visible tags: `series`, `series-part`, plus mirrored freeform atoms `----:com.apple.iTunes:SERIES` and `----:com.apple.iTunes:SERIES-PART`
- Apple movement tags: `MVNM`, `MVIN`

This preserves ABS/Plex discoverability and Apple Books compatibility in one write path.

## Metadata Intent Boundary

Honor explicit `set | clear | noop` intent semantics end-to-end.
- Empty sentinels (`''`, `0`, `[]`) are explicit clear commands.
- Do not collapse clear to noop via emptiness heuristics.

## Verification

1. Verify ffprobe-visible tags:
```bash
ffprobe -v quiet -print_format json -show_format output.m4b | jq '.format.tags'
```
2. Verify movement/freeform atoms with mp4 tooling (for example `AtomicParsley`).
3. Validate behavior in ABS/Plex/Apple import workflows when modifying mappings.

## References

- Strategy and mappings: `references/tag-mapping.md`
- Folder conventions: `references/folder-conventions.md`
- Metadata model: `src-tauri/src/metadata/mod.rs`
- Boundary commands: `src-tauri/src/commands/metadata.rs`
