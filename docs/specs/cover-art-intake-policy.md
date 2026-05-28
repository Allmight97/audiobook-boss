# Cover Art Intake Policy - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: cover-art file and URL intake keeps backend path/security validation
authoritative while the frontend provides aligned affordances and messages.

Acceptance signal: extension filters, drag/drop checks, HTTPS-only URL checks,
backend path validation, and backend URL validation cannot drift silently.
Frontend prechecks are either derived from backend-owned capability facts or
clearly treated as non-authoritative hints.

## Progress

- [x] 2026-05-26: Audit validated items `2k` and `2l` as a coherent
  cover-art intake workblock.
- [ ] Decide capability-export versus hint-only frontend policy.
- [ ] Implement aligned frontend/backend validation behavior and tests.

## Surprises & Discoveries

- Observation: frontend picker and drag/drop maintain their own local image
  extension allowlist.
  Evidence: `src/ui/coverArt.ts`.
- Observation: backend separately validates local cover-art image paths and
  extensions through Audio Engine path validation.
  Evidence: `src-tauri/src/audio/constants.rs` and
  `src-tauri/src/audio/path_validation.rs`.
- Observation: frontend HTTPS-only URL precheck duplicates part of backend URL
  defense, but defense-in-depth is appropriate for URL/path security.
  Evidence: `src/ui/coverArt.ts` and
  `src-tauri/src/commands/metadata.rs`.

## Three-Order Trace / Blast Radius

- Order 1, duplicated rule facts:
  cover-art file extensions and HTTPS-only URL acceptance are checked in the UI
  and again in Rust command/path-validation code.
- Order 2, immediate blast radius:
  picker filters, drag/drop prechecks, URL form validation, metadata commands,
  image decode/content-type checks, and user-facing error mapping.
- Order 3, downstream effects:
  a drift can create misleading UX, false rejection before backend defense,
  inconsistent error messages, or pressure to weaken backend path/URL
  validation in later cover-art work.

## Decision Log

- Decision: Treat backend cover-art path and URL validation as the authority;
  frontend checks are UX affordances only.
  Rationale: path and URL inputs are security-sensitive, and root policy says
  path validation guarantees cannot be bypassed.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Path validation owner:
    `src-tauri/src/audio/path_validation.rs`.
  - Metadata command boundary:
    `src-tauri/src/commands/metadata.rs`.
  - UI cover-art workflow:
    `src/ui/coverArt.ts`.
- Canon surfaces this spec must not redefine:
  - Path validation remains active for file inputs.
  - Command ingress validates unsafe external input.
  - Frontend filters do not replace backend validation.

## Scope And Constraints

In scope:

- `2k`: cover-art extension allowlist mirror.
- `2l`: cover-art HTTPS-only URL mirror.
- Aligned error mapping and tests for file and URL intake.

Out of scope:

- Metadata lookup provider fallback behavior.
- Metadata intent validation.
- Image optimization redesign.
- Broad network security policy beyond the existing cover-art URL loader.

Constraints:

- Do not relax backend HTTPS-only URL validation, host validation, bogon-IP
  rejection, or local path validation.
- Do not expose sensitive absolute path details in user-facing errors.
- Any fallback behavior must be registered and sunset-bound.

## Plan Of Work

- Edits:
  - Add a backend-owned cover-art intake capability result, or explicitly mark
    frontend filters as affordance-only and test that backend authority remains.
  - Update `src/ui/coverArt.ts` to consume capability facts or centralize
    affordance constants behind a non-authoritative adapter.
  - Align user-facing error strings for unsupported extensions and URL scheme
    failures.
  - Add tests for picker/drag/drop prechecks and backend rejection matrix.
- Proof steps:
  - Rust tests for `validate_input_image_path` and cover-art URL validation.
  - TS tests for cover-art frontend affordance behavior.
  - `mise run proof` if runtime IPC changes.
- Expected repo-visible outcome:
  - UI and backend agree on supported cover-art intake behavior without making
    frontend security-authoritative.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/audio/path_validation.rs`
  - `src-tauri/src/audio/constants.rs`
  - `src-tauri/src/commands/metadata.rs`
  - `src/ui/coverArt.ts`
- Libraries/external behavior:
  - Existing image decoding/content-type checks remain authoritative.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust metadata command tests.
  - Rust audio path-validation tests.
  - TS cover-art workflow tests.
- Full gate:
  - `mise run proof` if commands/types change.
  - Otherwise run focused Rust/TS checks plus `git diff --check`.
- Manual or visual evidence:
  - Only needed if cover-art UI text/control layout changes materially.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill only enduring path-security ownership guidance into the nearest
  `AGENTS.md` or `docs/system-map.md` if future agents need it.
