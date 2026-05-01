# Metadata Boundary Directives

## Scope

- Owns audiobook metadata read/write behavior under `src-tauri/src/metadata/`.
- Source of truth for external tag interoperability and fallback discipline.

## Preferred Path

- Use mp4ameta bridge paths as the probed MP4-family atom reader and writer when the file can be read directly.
- Use ffmpeg as the generic reader/prober. Route metadata engines from actual container classification, not filename suffix.
- Keep read/write behavior aligned through shared tag registry expectations.
- Preserve canonical and mirrored tag writes only when they have proven downstream ecosystem value.
- Introduce fallback paths only when fail-fast would break user outcomes on real files.

## Hard Invariants

- Maintain interoperability with external audiobook tag variants in the wild.
- New fallback behavior must include explicit marker metadata, observability, and fallback-register entry per root policy.
- Clear intent must fully remove related legacy/canonical atoms and cover-art artifacts, not partially clear fields.
- Metadata behavior must stay consistent with boundary intent semantics from `src/lib/tauri/AGENTS.md`.

## Canary Trigger

- Trigger Canary when metadata correctness depends on undocumented precedence between canonical, mirrored, and legacy tags.
- Report the ambiguous precedence, working assumption, and minimal rule update proposal.
- Continue unless ambiguity risks incorrect user-visible metadata or data loss.

## Done Criteria

- Metadata edits preserve external interoperability and clear-intent correctness.
- Any fallback addition is explicit, observable, and registered.
- Read/probe and write ownership remain coherent across canonical and compatibility tags.
