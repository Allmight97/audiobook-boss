# High-ROI Audit Backlog

Repo-grounded audit of first-party code (`src/`, `src-tauri/src/`). Ordered by impact if resolved before new features or UI redesign. Generated 2026-05-26.

## Priority stack (open)

| # | Item | Why first |
| --- | --- | --- |
| 1 | Duplicate TS↔Rust domain rules | Two-source-of-truth trap across Decide/Preflight/Process (expanded below) |
| 2 | `external_fdk.rs` monolith | Highest-risk encode path; change blast radius |
| 3 | Adaptive preview chapter markers | Known incomplete preview behavior |
| 4 | `fileList/actions.ts` orchestration hub | File-list + metadata coupling blocks safe UI iteration |

---

## Resolved 2026-05-28: metadata ingress trust policy

Metadata lookup fallbacks are now explicit through typed diagnostics, source
markers, focused tests, and `docs/fallbacks.md` rows `FB-019` through `FB-021`.
Cover-art picker/drop checks are centralized as frontend hints while backend
path and URL validation remains authoritative.

## Resolved 2026-05-28: processing terminal-outcome concentration

Processing terminal truth is split behind the private `terminal_outcomes/`
module family. `run.rs` keeps orchestration while classification, skip/no-write
entry construction, batch aggregation/repair, and terminal event helpers are
owned by named private modules with focused tests.

---

## 2. Duplicate TS↔Rust domain rules (two-source-of-truth)

Parent issue: UI preflight and backend execution each re-implement the same constraints with separate tests and no cross-layer parity contract (except one bitrate whitelist test).

### Resolved 2026-05-27: encoder runtime capabilities (`2a`-`2b`)

Encoder type ↔ bitrate-mode compatibility and Auto encoder display now use
backend-owned capability/availability payloads. The frontend still applies UI
state transitions, labels, and disabled states, but it no longer owns the
encoder-mode matrix or Auto resolution order.

### 2c. Publication date normalization

**Impact:** Form can silently skip invalid dates while backend rejects or normalizes differently on save/process.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/metadataIntent.ts` (`normalizePublicationDate`), `src/ui/metadataForm.ts` (invalid parse → no write) |
| Backend | `src-tauri/src/metadata/mod.rs` (`normalize_publication_date`), used in intent apply + readers |

**Drift risk:** Regex vs byte-parser edge cases (`2024-13`, ISO timestamps). Separate unit tests only.

### 2d. Series / subseries part slash ban

**Impact:** User blocked or warned in UI at 4+ call sites; backend re-validates on naming/save/process.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/metadataValidation.ts` (`getSeriesPartValidationError`, `getSubseriesPartValidationError`) |
| Backend | `src-tauri/src/metadata/mod.rs` (`validate_series_part`), `output_artifact/naming.rs` |

**Drift risk:** Error message wording already differs slightly between layers.

### Resolved 2026-05-27: runtime settings capability contract (`2e`-`2j`)

Max-concurrent-job options, explicit sample rates, thread range, encoder bitrate
options, VBR range/default, and default bitrate mode per encoder now flow
through `get_runtime_settings_capabilities`. Audio owns encoder capability
facts; JobRegistry owns concurrency capability facts; App Settings validates
durable preferences against the runtime owner instead of keeping its own range
table; frontend controls render capability payloads instead of local
accept/reject matrices.

### Resolved 2026-05-28: cover art file extension allowlist

Frontend picker/drop extensions are centralized as non-authoritative hints in
`src/ui/coverArt.ts`; backend image path and HTTP content-type validation remain
the authority.

### Resolved 2026-05-28: cover art URL HTTPS-only policy

Frontend HTTPS checks remain user-facing hints; backend URL validation still
enforces HTTPS-only, redirect, host, bogon-IP, content-type, and size rules.

### 2m. Metadata intent enriched type + compile adapter

**Impact:** Contract drift surface — not duplicate validation logic, but duplicate *ownership* of intent shape.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/metadataIntent.ts` (enriched patch, `noop`, `compileMetadataIntentPatch`) |
| Wire | `src/lib/generated/tauri.ts` (`MetadataIntentPatch`) |
| Backend | `src-tauri/src/metadata/intent.rs` |
| Adapter | `src/lib/tauri/commands.ts` (`metadataIntent` → `metadataPatch`) |

**Mitigation:** `metadataIntent.test.ts`, `tauri-client.test.ts`. **Drift risk:** New Specta fields/ops without TS compile failures if casts stay loose.

### 2n. Import duplicate detection (frontend-only twin logic)

**Impact:** Lower — same path-set rule and user message duplicated in two UI modules.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/fileImport/importAnalysisWorkflow.ts`, `src/ui/fileList/actions.ts` (`collectUniqueFiles`) |

**Note:** Audio format allowlist is correctly backend-owned (`get_supported_audio_import_metadata`); this is not a violation.

### 2o. Output size estimate heuristic (frontend-only advisory)

**Impact:** Lowest in this family — not a hard validation duplicate, but Decide-phase *advisory truth* lives only in TS.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/outputPanel/preview.ts` (`calculateEstimatedSize`: bitrate × duration × stereo fudge × 1.03) |
| Backend | No equivalent estimate |

**Drift risk:** Misleading size preview vs actual encoded output; acceptable if labeled advisory, problematic if treated as preflight truth.

---

## 4. `external_fdk.rs` monolith

**Evidence:** 1,310 LOC (~2.7× backend threshold). Combines spawn/monitor/kill, args, progress, cancellation, metadata passthrough, and large inline tests.

**Done looks like:** Extract spawn/monitor, arg builder, and test fixtures before next encoder/toolchain change.

---

## 5. Adaptive preview chapter markers

**Evidence:** `src-tauri/src/audio/processor/preview_state.rs` TODO; markers collected in pipeline but only logged, not emitted.

**Done looks like:** Wire FFMETADATA chapter emission or remove dead collection.

---

## 6. `fileList/actions.ts` orchestration hub

**Evidence:** 601 LOC; couples metadata drafts, output refresh, selection, order lock; no dedicated unit test file.

**Done looks like:** Split by behavior; add focused tests for intent staging and output refresh triggers.

---

## Recommended sequencing

```text
2c–2d → remaining metadata rule parity
3 → terminal outcome split
4 → external_fdk decomposition
5–6 → preview chapters + fileList split
```

## Clean areas (not backlog)

- IPC boundary: runtime calls centralized in `tauriClient`; no UI `invoke()` bypass.
- Local audio import formats: backend-owned metadata; frontend does not maintain a parallel allowlist.
- Encoder bitrate whitelist: partial parity test exists (`audio-defaults.test.ts`).
