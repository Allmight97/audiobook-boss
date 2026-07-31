---
name: resource-lifetime-audit
description: Read-only audit for Audiobook Boss resource-lifetime foot-guns with high bug potential. Use when reviewing or planning ABB changes across file handles, external processes, temp artifacts, replacement, cleanup ownership, validation-to-write, or FFmpeg/mp4ameta reopen boundaries, and when the user asks to hunt for lifecycle or cross-platform hazards. Apply fixes only with explicit implementation authority.
---

# Resource Lifetime Audit

## Scan Scope

Scan the highest-risk boundaries first: audio processing, metadata read/write/remux, output artifact commit, path validation, cleanup, cancellation, and external process handling.

## Hunt Pattern

Search for code that crosses one lifecycle phase into another:

- probe/open/read -> reopen same path
- FFmpeg context -> mp4ameta read/write on same path
- write temp -> rename/copy/replace final path
- spawn process -> read pipes -> cancel/kill/wait
- register cleanup -> remove path from cleanup ownership
- validate path -> persist or write path

For each candidate, ask:

1. What resource is still alive?
2. What opens, mutates, deletes, renames, or replaces the same resource next?
3. Does this rely on macOS/POSIX behavior that may fail on Windows?
4. Would a failure cause false success, file loss, residue, stuck jobs, or noisy retries?
5. Is the fix local and testable?

For cleanup candidates, trace exact path values through registration, removal,
draining, overlapping guards, and startup or other backstop owners. Evaluate
terminal classification separately from residue retry or recovery. A cleanup
removal call is not proof of ownership: prove that the registered and removed
values match and identify every remaining owner.

Do not close a post-commit cleanup candidate until both questions are answered:

1. If the durable operation succeeded and cleanup fails, what terminal outcome
   reaches the caller or user?
2. Which exact owner retains or reacquires the failed path for retry?

A retry or startup backstop can limit residue without making a false terminal
failure truthful.

Before concluding that no retry owner remains, enumerate every caller of the
transition and every guard that registered the same concrete resource value.

## Priority

Prioritize findings that touch:

- source audiobook files
- final output artifacts
- metadata writes and cover art
- FFmpeg input/output contexts
- external FFmpeg processes
- cleanup/cancellation terminal paths

Defer low-impact style cleanup unless it prevents repeated lifecycle mistakes.

## Fix Shape

Prefer explicit ownership transitions:

- Drop FFmpeg probe contexts before reopening the same path.
- Keep replacement behavior in the owning boundary.
- Use backup/rollback or create-new semantics instead of assuming rename-over-existing is portable.
- Remove cleanup ownership only after the durable artifact is committed.
- Map file/path errors through existing `AppError` patterns.

Add focused tests when behavior is deterministic without real media. Use the
owner-scoped command menu from `README.md` / `scripts/AGENTS.md` for code
changes.

## Report Shape

Lead with the category name, affected boundary, impact, and fix. Use "resource-lifetime foot-gun" for hazards that are not always active bugs but have credible cross-platform or edge-case failure paths.
