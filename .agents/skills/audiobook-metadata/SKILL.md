---
name: audiobook-metadata
description: Evaluate or change ABB tag mappings and folder conventions for Audiobookshelf, Plex, or Apple Books interoperability. Ordinary metadata intent and validation work follows its owning AGENTS.md.
---

# Audiobook Metadata

Preserve real-file interoperability while keeping compatibility claims tied to
the writer, reader, and player behavior actually verified.

## Choose The Evidence

- For tag mapping or series compatibility, read
  [tag-mapping.md](references/tag-mapping.md). It owns the retained interop
  strategy and routes to the live field schema and container adapters.
- For output organization or scanner folder parsing, read
  [folder-conventions.md](references/folder-conventions.md). It routes to
  ABB's naming owner and external scanner documentation.

Repository paths in these references are relative to the ABB root. Metadata
intent, validation, write planning, and runtime adaptation stay under
`src-tauri/src/metadata/AGENTS.md` and `src/lib/tauri/AGENTS.md`. Output path
policy stays under `src-tauri/src/output_artifact/AGENTS.md`.

## Verify The Changed Handoff

Use owner tests for mapping, clear/noop behavior, and atom precedence. For a
changed writer or finalization path, inspect a generated M4B with:

```bash
ffprobe -v quiet -print_format json -show_format output.m4b
```

Use atom-aware readback for freeform mirrors that ffprobe cannot distinguish.
When the claim is player/scanner compatibility, verify the affected import
workflow and record the consumer version and relevant library settings. If
that consumer is unavailable, report the artifact proof and the unverified
compatibility claim separately.

Finish with the changed mapping or convention, its owner, artifact/test
evidence, and any remaining consumer-specific uncertainty.
