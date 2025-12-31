**Findings**
- Critical — Cmd+S bulk/single saves ignore edits because dirty tracking never activates (no `initMetadataFormEvents` call + selector targets missing `#metadata-form`), so `readMetadataForm({ onlyDirty: true })` yields `{}`; likely the reported “Save Failure.” `src/main.ts:134`, `src/main.ts:221`, `src/ui/metadataForm.ts:6`, `index.html:214`
- High — Selection state can drift: deselecting the anchor leaves `selectedFileIndex` invalid and list mutations don’t update `selectedFileIndices`, so bulk apply can target the wrong files. `src/ui/fileList/selection.ts:58`, `src/ui/fileList/actions.ts:140`, `src/ui/fileList/events.ts:193`, `src/main.ts:195`
- High — Cover art edits are dropped in Cmd+S because dirty-mode skips cover art entirely, so even single-file cover art changes never persist. `src/main.ts:221`, `src/ui/metadataForm.ts:102`
- Medium — `metadataState` is no longer refreshed on selection change or after bulk save; batch processing can emit stale tags even after a successful save. `src/ui/fileList/actions.ts:61`, `src/main.ts:239`, `src/ui/statusPanel/processing.ts:159`
- Medium — Dirty mode can’t clear numeric fields and several Issue #109 UX items are missing (Cmd+A/Escape, Apply to Selected button, selection count/placeholder), leaving bulk editing incomplete. `src/ui/metadataForm.ts:69`, `src/ui/fileList/selection.ts:94`, `src/ui/fileList/events.ts:97`, `index.html:214`
- Low — `src/ui/fileList/actions.ts` remains 518 LOC (over the 400 guideline), so the refactor doesn’t meet the maintainability target. `src/ui/fileList/actions.ts:1`

Tri-order impact: 1st‑order, edits appear saved but aren’t (and bulk apply can hit wrong files); 2nd‑order, users may unknowingly overwrite metadata and processing outputs can carry stale tags; 3rd‑order, trust erosion and higher cost for future bulk‑edit features.

**User Clarifications (current session)**
- Bulk cover art is out of scope for multi-select (no bulk cover art apply).
- Multi-select intent: apply common metadata (author/series/narrator/etc) across selected files via Cmd+S; blank fields stay blank unless edited.
- Mixed-value display desired: when multiple files selected, identical values show, blank stays blank, and mixed values expose a third-state control (e.g., "<keep>" / "<blank>") while still allowing edits (mp3tag-inspired).
- Mixed-value UI should use explicit per-field dropdowns (keep/blank) plus normal editable inputs; dropdown choice defines what happens on Cmd+S or encoding.
- If the year field is empty, it should be treated as empty on save/encode.
- Selection change behavior: preserve cached edits in single-select; reset pending edits for multi-select when the selection set changes.
- Multi-select Cmd+S with no dirty fields should be a no-op (single-select keeps current behavior).
- Mixed-value display should read metadata for all selected files (use cache where available; async fetch for missing).

**Engineering Decisions (locked)**
- Cmd+S writes to disk and updates `metadataState` for selected files; Apply to Selected updates `metadataState` only (no disk writes).
- Multi-select uses explicit per-field dropdowns for Keep/Blank plus normal editable inputs; dropdown choice defines apply behavior.
- Multi-select Cmd+S is a no-op if no dirty fields; single-select keeps current behavior.
- Mixed-value display reads metadata for all selected files (use cache when available; async fetch for missing).
- Single-select preserves cached edits on selection change; multi-select resets pending edits when selection set changes.
- Cover art is single-select only; multi-select ignores cover art changes.

**Scope (Minimum Viable for this PR)**
- Fix dirty-field tracking (initialize events + ensure selectors exist).
- Selection integrity fixes (anchor updates, selection set updates on remove/reorder/sort/clear).
- Cmd+A selects all; Escape clears selection.
- Mixed-value computation for selected files (same/blank/mixed) with Keep/Blank dropdowns + editable inputs.
- Apply to Selected button (metadataState only) + Cmd+S (disk + metadataState).
- Selection count label in UI near metadata context header.
- Backend support for clearing year (mp4ameta remove_year + ffmpeg dict removal path).

**UI Placement (locked)**
- Apply to Selected lives in the metadata section directly under the Description field, right-aligned, matching the Output “Browse” button style and color.

**Implementation Status (post-fix)**
- Metadata form now supports multi-select mode with Keep/Blank dropdowns, mixed-value placeholders, and a selection count label.
- Added Apply to Selected (metadataState-only) and Cmd+S persists to disk + metadataState (single file or multi-select).
- Selection logic refreshed (anchor handling, reorder/remove reindex, select-all/Cmd+A, Escape clear) and metadata panel split for clarity.
- Multi-select reads metadata for all selected files (cache-first, async fetch for missing), cover art remains single-select.
- Year clearing implemented in backend (mp4ameta `remove_year` + ffmpeg dict removal for `date`/`year`).

**Open Questions (remaining)**
- None.

**Applied Changes**
- Wired dirty tracking (`#metadata-form`, `initMetadataFormEvents`, reset after save/apply).
- Kept selection state consistent across remove/reorder/sort/clear and maintained anchor.
- Added mixed-value display with Keep/Blank controls plus editable inputs for multi-select.
- Preserved cover art as single-select only; multi-select ignores cover art.
- Implemented numeric clearing (year = 0) with mp4ameta + ffmpeg dict removal, synced `metadataState` after apply/save.
- Added UX affordances and tests (Cmd+A/Escape, Apply button, selection count, selection unit tests).

**Quality Rating**
- Original PR: Design 2/5 · Practice 2/5 · Code & Solution Quality 2/5 — core flows were incomplete, state drifted, and tests/guardrails were missing.
- Post-fix: Design 4/5 · Practice 4/5 · Code & Solution Quality 4/5 — core flows implemented with tests; remaining gaps are minor polish.

**Tests & Checks**
- `scripts/quick-checks.sh` (cargo fmt/clippy, ensure-contract, tsc)
- `bun run test`
- `cargo test` (from `src-tauri/`)
  - Note: ffmpeg/codec warnings observed in test logs; tests still passed.
