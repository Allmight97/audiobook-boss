---
name: resource-lifetime-audit
description: Audit and fix Audiobook Boss resource-lifetime foot-guns with high bug potential. Use when the user asks to hunt for lifecycle bugs, file-handle contention, Windows/cross-platform file behavior, temp-file cleanup, process lifetime, FFmpeg/mp4ameta reopen issues, rename/replace semantics, or "similar in-kind" high-ROI issues.
---

# Resource Lifetime Audit

## Goal

Find small, high-impact lifecycle hazards before they become user-visible bugs. Focus on important boundaries first: audio processing, metadata read/write/remux, output artifact commit, path validation, cleanup, cancellation, and external process handling.

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

Add focused tests when behavior is deterministic without real media. Use `scripts/proof.sh standard` for code changes.

## Report Shape

Lead with the category name, affected boundary, impact, and fix. Use "resource-lifetime foot-gun" for hazards that are not always active bugs but have credible cross-platform or edge-case failure paths.
