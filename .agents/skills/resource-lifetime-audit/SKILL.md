---
name: resource-lifetime-audit
description: Read-only audit for Audiobook Boss resource-lifetime foot-guns with high bug potential. Use when the user asks to hunt for lifecycle bugs, file-handle contention, Windows or cross-platform file behavior, temp-file cleanup, process lifetime, FFmpeg/mp4ameta reopen issues, rename/replace semantics, or similar in-kind high-ROI hazards. Apply fixes only when the user explicitly asks.
---

# Resource Lifetime Audit

Default to a read-only audit. Do not edit code merely because a credible hazard
was found. Enter fix mode only when the user explicitly requests implementation.

## Scan Scope

Scan the highest-risk boundaries first: audio processing, metadata
read/write/remux, output artifact commit, path validation, cleanup,
cancellation, and external process handling.

Look for transitions where one owner may still hold a resource while the next
operation conflicts with it:

- probe/open/read → reopen the same path
- FFmpeg context → mp4ameta read/write on the same path
- write temp → rename/copy/replace final path
- spawn process → read pipes → cancel/kill/wait
- register cleanup → remove path from cleanup ownership
- validate path → persist or write path

## Scan Ledger

Record every investigated transition, including rejected candidates:

| Field | Required content |
| --- | --- |
| Owner / path | Owning module and concrete code path |
| Resource / lifetime | Handle, process, temp artifact, cleanup token, or path assumption and when it ends |
| Next conflicting operation | Reopen, mutate, delete, rename, replace, kill, or persist |
| Status | Confirmed bug, credible hazard, or rejected |
| Impact / platform | False success, loss, residue, stuck job, retry noise; platform sensitivity |
| Proof | Reproduction, code evidence, focused test, or reason rejected |
| Residual uncertainty | What remains unproven |

For each candidate ask: what is still alive, what conflicts next, whether the
behavior relies on macOS/POSIX semantics, what the failure costs, and whether a
local owner-scoped fix is testable.

## Priority

Prioritize source files, final artifacts, metadata/cover writes, FFmpeg
contexts and processes, and cleanup/cancellation terminal paths. Defer style
cleanup unless it prevents repeated ownership mistakes.

## Fix Mode

When explicitly authorized, prefer explicit ownership transitions:

- drop probe/remux contexts before reopening or replacing the same path
- keep replacement behavior in the owning boundary
- use rollback or create-new semantics instead of assuming portable
  rename-over-existing behavior
- remove cleanup ownership only after durable commit
- map file/path failures through existing `AppError` patterns

Every fix needs proof that can go red before the fix and green after it: a
reproduction, focused regression test, or portability contract test. Use the
owner-scoped command menu in `README.md` or `scripts/AGENTS.md`. If an observed
failure needs iterative diagnosis rather than a bounded lifecycle correction,
route to `diagnose`.

## Report

Lead with status, category, affected boundary, impact, and proof. Use
“resource-lifetime foot-gun” for a credible hazard that is not confirmed as an
active bug. Include the complete scan ledger and residual uncertainty; do not
turn absence of reproduction into certainty.
