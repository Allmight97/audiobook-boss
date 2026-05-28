# Metadata Ingress Trust Policy - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose

Outcome: metadata-adjacent ingress stays truthful at the boundary where ABB
accepts outside data. Cover-art file/URL intake and online metadata lookup must
not rely on duplicated UI rules or silent provider substitution.

Acceptance signal: backend validation remains authoritative for cover-art files
and URLs; frontend checks are either derived from a backend-owned fact or tested
as non-authoritative hints. Online lookup fallbacks are retained only when they
are explicit, observable, registered, and tested; otherwise they become
user-visible failures or degraded results.

## Current Evidence

- `src/ui/coverArt.ts` owns picker extensions, HTTPS precheck, and drag/drop
  image matching.
- `src-tauri/src/audio/constants.rs` and
  `src-tauri/src/audio/path_validation.rs` own backend image extension
  validation for local cover-art paths.
- `src-tauri/src/commands/metadata.rs` owns backend cover-art URL validation,
  redirect policy, bogon-IP defense, content-type validation, size limits, and
  optimization.
- `src-tauri/src/commands/metadata_lookup/service.rs` currently falls through
  from failed ASIN lookup to text search, tolerates partial provider failure,
  and maps failed Audnexus detail lookups into Audible-only results.
- `src-tauri/src/commands/metadata_lookup/mapping.rs` marks Audible-only mapped
  results with `audible_only: Some(true)`.
- `docs/fallbacks.md` currently registers no metadata lookup fallback.

## Decision Log

- Decision: combine cover-art intake and metadata lookup fallback into one
  metadata ingress trust workblock.
  Rationale: both paths accept outside metadata-adjacent data before the
  Metadata Outcome Plan writes durable tags. The shared design problem is
  accepted/degraded input truth, not final metadata write policy.
  Date: 2026-05-28.
- Decision: backend validation remains the authority for cover-art path and URL
  safety. Frontend checks are UX affordances only unless a backend-owned
  capability fact is deliberately exposed.
  Date: 2026-05-28.

## Scope

In scope:

- Cover-art file extension alignment across picker, drag/drop, and backend
  image path validation.
- Cover-art HTTPS URL precheck alignment across UI and backend URL validation.
- Metadata lookup ASIN/direct lookup fallback to text search.
- Metadata lookup selected-provider failure tolerance.
- Audible-only result mapping when Audnexus detail lookup fails.
- User-visible diagnostics, degraded-result markers, tests, and fallback
  register/source markers for retained behavior.

Out of scope:

- Metadata intent validation or write-plan behavior.
- Remote acquisition.
- Broad provider rewrite unless required to make fallback truth observable.
- Image optimization redesign beyond error/result alignment.
- Weakening backend path, URL, host, redirect, or content-type validation.

Constraints:

- Do not relax backend HTTPS-only URL validation, bogon-IP rejection, redirect
  validation, content-type validation, image-size limits, or local path
  validation.
- Do not expose sensitive absolute path details in user-facing errors.
- Any retained fallback requires explicit trigger, observable signal, source
  marker metadata where applicable, `docs/fallbacks.md` entry, and sunset or
  renewal condition.
- Do not present provider-substituted data as canonical provider truth.

## Plan Of Work

- Inventory current cover-art and lookup ingress rules against code before
  editing.
- Decide per metadata lookup fallback whether to retain/register, expose as a
  degraded result, or convert to a hard failure.
- If retaining lookup fallbacks, add source-adjacent markers, result diagnostics
  where needed, and compact `docs/fallbacks.md` rows.
- Align cover-art frontend checks by either consuming a backend-owned fact or
  centralizing them as non-authoritative hints with tests that backend authority
  remains.
- Align user-facing error messages for unsupported local images and URL scheme
  failures without leaking sensitive paths.
- Add focused Rust tests for metadata lookup fallback/degraded paths, cover-art
  URL validation, and image path validation.
- Add focused TS tests for cover-art frontend affordance behavior.
- Regenerate/check bindings if lookup result shapes or command contracts change.

## Proof Path

- Focused Rust metadata lookup tests.
- Focused Rust metadata command/path-validation tests.
- Focused TS cover-art workflow tests.
- `scripts/check-fallback-policy.sh` if any fallback is retained or removed.
- `bun scripts/proof/runner.ts focus runtime` if IPC/result shapes change.
- `bun scripts/proof/runner.ts review` before handoff.

## Cleanup Trigger

When implemented, reviewed, validated, docs-aligned, and synced:

- Delete this spec.
- Keep only active fallback rows in `docs/fallbacks.md`.
- Distill enduring path-security or metadata lookup ownership guidance into the
  nearest `AGENTS.md` only if future agents need a stable rule.
