---
name: audiobook-metadata
description: Canonical metadata and output-naming strategy for Audiobook Boss. Use when changing tag mappings, metadata intent behavior, folder or filename conventions, or ABS/Plex/Apple interoperability rules.
---

# Audiobook Metadata

## Interop Invariant

External compatibility with ABS/Plex/Apple Books is required. Compatibility
claims must match current code and product decisions.

ABB does not read or write Apple movement tags (`MVNM`/`MVIN`). Series uses
ffprobe-visible tags plus mirrored mp4ameta freeform atoms.

## Series Tag Strategy

For series metadata, write both:
- ffprobe-visible tags: `series`, `series-part`
- mp4ameta freeforms: canonical `----:com.apple.iTunes:series` /
  `----:com.apple.iTunes:series-part` plus uppercase iTunes mirrors
  `----:com.apple.iTunes:SERIES` / `----:com.apple.iTunes:SERIES-PART`

Intent semantics (`set | clear | noop`), the Outcome Plan, and naming-to-artifact
flow are owned by `src-tauri/src/metadata/AGENTS.md` and `src/lib/tauri/AGENTS.md`.

## Proof Matrix

Choose proof by observer; one observer cannot substitute for another.

| Claim changed | Required proof |
| --- | --- |
| Pure intent, mapping, validation, or naming rule | Focused tests in `abb-metadata-core` or `abb-output-artifact-core` |
| MP4 sink, case-distinct freeforms, clear behavior, or container routing | Focused runtime/sink tests that inspect the exact atoms or sink operations |
| Real artifact tag/chapter/cover truth | Media-execution artifact plus `ffprobe`; use MP4-aware inspection for freeform atoms |
| ABS, Plex, or Apple importer compatibility | Manual importer evidence when the change affects a compatibility claim |

`ffprobe -show_format` cannot prove that lowercase canonical and uppercase
mirror freeform atoms both exist because it collapses names that differ only by
case. Do not treat a bare FFmpeg remux as MP4 metadata proof; the MOV muxer drops
keys outside its known atom table.

## References

- Strategy and mappings: `references/tag-mapping.md`
- Folder conventions: `references/folder-conventions.md`
- Metadata model and outcome plan: `src-tauri/src/metadata/`
- Boundary commands: `src-tauri/src/commands/metadata.rs`
- Output artifact naming policy: `crates/abb-output-artifact-core/src/lib.rs`
- Output artifact naming adapter: `src-tauri/src/output_artifact/naming.rs`
