---
name: audiobook-metadata
description: Canonical metadata strategy for Audiobook Boss. Use when changing tag mappings, metadata intent behavior, or ABS/Plex/Apple interoperability rules.
---

# Audiobook Metadata

## Interop Invariant

External compatibility with ABS/Plex/Apple Books is required. Compatibility
claims must match current code and product decisions.

ABB does not read or write Apple movement tags (`MVNM`/`MVIN`). Series uses
ffprobe-visible tags plus mirrored freeform atoms.

## Series Tag Strategy

For series metadata, write both:
- ffprobe-visible tags: `series`, `series-part`, plus mirrored freeform atoms `----:com.apple.iTunes:SERIES` and `----:com.apple.iTunes:SERIES-PART`

Intent semantics (`set | clear | noop`), the Outcome Plan, and naming-to-artifact
flow are owned by `src-tauri/src/metadata/AGENTS.md` and `src/lib/tauri/AGENTS.md`.

## Verification

1. Verify ffprobe-visible tags:
```bash
ffprobe -v quiet -print_format json -show_format output.m4b | jq '.format.tags'
```
2. Verify freeform atoms with mp4 tooling (for example `AtomicParsley`).
3. Validate behavior in ABS/Plex/Apple import workflows when modifying mappings.

## References

- Strategy and mappings: `references/tag-mapping.md`
- Folder conventions: `references/folder-conventions.md`
- Metadata model and outcome plan: `src-tauri/src/metadata/`
- Boundary commands: `src-tauri/src/commands/metadata.rs`
- Output artifact naming: `src-tauri/src/output_artifact/naming.rs`
