---
name: audiobook-metadata
description: Canonical metadata strategy for Audiobook Boss. Use when changing tag mappings, metadata intent behavior, or ABS/Plex/Apple interoperability rules.
---

# Audiobook Metadata

Use this skill when changing metadata field semantics, tag mapping policy, interoperability behavior, or output naming that depends on metadata.

## Interop Invariant

External compatibility with ABS/Plex/Apple Books is required, but compatibility
claims must be backed by current code and current product decisions.

`ffprobe` does not expose Apple movement tags (`MVNM`/`MVIN`). ABB does not
currently write or read movement tags as the canonical series mechanism.

## Series Tag Strategy

For series metadata, write both:
- ffprobe-visible tags: `series`, `series-part`, plus mirrored freeform atoms `----:com.apple.iTunes:SERIES` and `----:com.apple.iTunes:SERIES-PART`

This preserves ABS/Plex discoverability while keeping the code honest about the
specific MP4 atoms ABB currently supports. Movement tags were removed as
uncertain Apple-only compatibility support; reintroduce them only with manual
player evidence and an explicit product decision.

## Metadata Intent Boundary

Honor explicit `set | clear | noop` intent semantics end-to-end.
- Empty sentinels (`''`, `0`, `[]`) are explicit clear commands.
- Do not collapse clear to noop via emptiness heuristics.
- Metadata Intent Plan lives under `src-tauri/src/metadata/` and is governed by
  `src-tauri/src/metadata/AGENTS.md`.
- Output naming that depends on metadata flows through the Output Artifact
  boundary; keep naming policy in `src-tauri/src/output_artifact/naming.rs`.

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
- Metadata model and intent plan: `src-tauri/src/metadata/`
- Boundary commands: `src-tauri/src/commands/metadata.rs`
- Output artifact naming: `src-tauri/src/output_artifact/naming.rs`
