---
name: audiobook-metadata
description: Canonical metadata strategy for Audiobook Boss. Use when changing tag mappings, metadata intent behavior, or ABS/Plex/Apple interoperability rules.
---

# Audiobook Metadata

This is the canonical owner for cross-ecosystem metadata strategy.

## Scope

Use this skill when changing:
- metadata field semantics,
- tag mapping policy,
- write/read interoperability behavior,
- output naming/folder conventions tied to metadata outcomes.

## Canonical Interop Invariant

External compatibility with ABS/Plex/Apple Books is required.

`ffprobe` does not expose movement tags (`MVNM`/`MVIN`). If series data is written only to movement tags, ABS/Plex scans miss it.

## Canonical Dual-Write Strategy

For series metadata, write both:
- ffprobe-visible tags: `series`, `series-part`, plus mirrored freeform atoms `----:com.apple.iTunes:SERIES` and `----:com.apple.iTunes:SERIES-PART`
- Apple movement tags: `MVNM`, `MVIN`

This preserves ABS/Plex discoverability and Apple Books compatibility in one write path.

## Metadata Intent Boundary

Honor explicit `set | clear | noop` intent semantics end-to-end.
- Empty sentinels (`''`, `0`, `[]`) are explicit clear commands.
- Do not collapse clear to noop via emptiness heuristics.

## Verification Path

1. Verify ffprobe-visible tags:
```bash
ffprobe -v quiet -print_format json -show_format output.m4b | jq '.format.tags'
```
2. Verify movement/freeform atoms with mp4 tooling (for example `AtomicParsley`).
3. Validate behavior in ABS/Plex/Apple import workflows when modifying mappings.

## Canonical References

- Strategy and mappings: `references/tag-mapping.md`
- Folder conventions: `references/folder-conventions.md`
- Metadata model: `src-tauri/src/metadata/mod.rs`
- Boundary commands: `src-tauri/src/commands/metadata.rs`

`mp4ameta-patterns` is implementation-focused and must reference this skill for strategy rationale.

## Done Criteria

- Series/narrator/core metadata remain discoverable across ABS/Plex/Apple.
- Clear intent behavior remains explicit and preserved.
- No internal legacy fallback assumptions added without explicit evidence.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
- For library/API uncertainty, invoke `lib-research`.
