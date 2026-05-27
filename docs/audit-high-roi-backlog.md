# High-ROI Audit Backlog

Repo-grounded audit of first-party code (`src/`, `src-tauri/src/`). Ordered by impact if resolved before new features or UI redesign. Generated 2026-05-26.

## Priority stack (top 6)

| # | Item | Why first |
| --- | --- | --- |
| 1 | Metadata lookup unregistered fallbacks | Silent product-truth degradation; violates fallback hard invariant |
| 2 | Duplicate TS↔Rust domain rules | Two-source-of-truth trap across Decide/Preflight/Process (expanded below) |
| 3 | Processing terminal-outcome concentration | False success/failure in Status Panel |
| 4 | `external_fdk.rs` monolith | Highest-risk encode path; change blast radius |
| 5 | Adaptive preview chapter markers | Known incomplete preview behavior |
| 6 | `fileList/actions.ts` orchestration hub | File-list + metadata coupling blocks safe UI iteration |

---

## 1. Metadata lookup unregistered fallbacks

**Evidence:** `src-tauri/src/commands/metadata_lookup/service.rs` silently falls through on ASIN failure and substitutes Audible scrape data when Audnexus fails. `docs/fallbacks.md` has no entries for these paths. No Rust tests for `search_online_metadata`.

**Done looks like:** Register with trigger/observe/sunset or fail visibly; add backend tests for ASIN, provider failure, and merge ordering.

---

## 2. Duplicate TS↔Rust domain rules (two-source-of-truth)

Parent issue: UI preflight and backend execution each re-implement the same constraints with separate tests and no cross-layer parity contract (except one bitrate whitelist test).

### 2a. Encoder type ↔ bitrate mode matrix

**Impact:** Highest among rule duplicates — invalid combos fail at process time after UI appeared to accept them.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/encoderPanel/logic.ts` (`enforceBitrateModeCompatibility`), `src/types/encoder.ts` (`DEFAULT_BITRATE_MODE_BY_ENCODER`) |
| Backend | `src-tauri/src/audio/settings_encoder.rs` (`validate_encoder_mode_combo`) |

**Drift risk:** Auto/FDK→VBR, AAC-AT→CVBR, Native→CBR must stay aligned when encoder UX changes.

### 2b. Auto encoder resolution order

**Impact:** UI labels and availability hints can misrepresent what the backend will actually run.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/encoderPanel/logic.ts` (`resolveEffectiveEncoder`: FDK → AAC-AT → Native) |
| Backend | `src-tauri/src/audio/settings_encoder.rs` (`resolve_encoder_type`) |

**Drift risk:** Stale frontend availability vs fresh backend availability at process start.

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

### 2e. Max concurrent jobs allowed range

**Impact:** Settings/backend accept fixed values **1–8**; UI select exposes only **1–4**.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/jobControls/JobControlsIsland.svelte` (options 1–4 + auto) |
| Backend | `src-tauri/src/app_settings/types.rs` (`ConcurrencyPreference::validate`: 1..=8) |

**Drift risk:** Hydrated settings or patches with 5–8 have no matching UI option; `preferenceFromSelection` does not clamp before IPC.

### 2f. Explicit sample rate allowlist

**Impact:** Invalid rate can be typed/sent if UI options expand without backend sync.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/encoderPanel/EncoderPanelIsland.svelte` (22050, 32000, 44100, 48000) |
| Backend | `src-tauri/src/audio/settings.rs` (`validate_explicit_sample_rate`) |

**Drift risk:** No parity test (unlike bitrates).

### 2g. Thread count range (1–1024)

**Impact:** Frontend may sanitize to values backend rejects, or vice versa after rule change.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/encoder.ts` (`sanitizeThreads`) |
| Backend | `src-tauri/src/audio/settings_encoder.rs` (`VALID_THREAD_COUNT_RANGE`, `validate_threads`) |

**Drift risk:** No cross-layer parity test.

### 2h. Encoder bitrate whitelist

**Impact:** Medium — mitigated but still a manual mirror.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/audio.ts` (`VALID_ENCODER_BITRATES`, comment: "Frontend mirror of Rust") |
| Backend | `src-tauri/src/audio/settings_encoder.rs` |
| Mitigation | `src/types/audio-defaults.test.ts` parses Rust source for parity |

**Drift risk:** Lower than other items because of existing parity test; still not generated from one source.

### 2i. VBR quality level clamp (1–5)

**Impact:** Invalid VBR levels clamped in UI but validated strictly on backend.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/encoder.ts` (`sanitizeBitrateMode`), `src/ui/encoderPanel/state.svelte.ts` |
| Backend | `src-tauri/src/audio/settings_encoder.rs` (`validate_bitrate_mode`) |

### 2j. Default bitrate mode per encoder

**Impact:** UI defaults and coercions can diverge from backend combo rules.

| Layer | Location |
| --- | --- |
| Frontend | `src/types/encoder.ts` (`DEFAULT_BITRATE_MODE_BY_ENCODER`), `logic.ts` coercion |
| Backend | `validate_encoder_mode_combo` + `src-tauri/src/app_settings/types.rs` defaults |

### 2k. Cover art file extension allowlist

**Impact:** User can pass picker/drag paths frontend accepts but backend image loader rejects.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/coverArt.ts` (picker extensions + `/\.(jpg|jpeg|png|webp)$/i`) |
| Backend | `src-tauri/src/audio/constants.rs` (`ALLOWED_IMAGE_EXTENSIONS`), `path_validation.rs`, `commands/metadata.rs` (content-type check) |

**Drift risk:** Three surfaces (picker, drag regex, Rust path + HTTP content-type).

### 2l. Cover art URL HTTPS-only policy

**Impact:** Lower — defense in depth, but duplicated UX/error strings.

| Layer | Location |
| --- | --- |
| Frontend | `src/ui/coverArt.ts` (`parsed.protocol !== 'https:'`) |
| Backend | `src-tauri/src/commands/metadata.rs` (`load_cover_art_from_url`) |

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

## 3. Processing terminal-outcome concentration

**Evidence:** `terminal_outcomes.rs` (823 LOC) + `run.rs` (760 LOC) own batch skip, cancel vs fail classification, and Status Panel terminal truth.

**Done looks like:** Split classification modules; integration tests for merge/batch/cancel edge cases.

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
1 → metadata lookup fallbacks + tests
2a–2d → core Decide/Process rule parity (encoder matrix, auto resolve, dates, series parts)
2e–2g → settings-range parity (concurrency UI, sample rates, threads)
3 → terminal outcome split
4 → external_fdk decomposition
2h–2m → remaining mirrors as part of contract-hardening pass
5–6 → preview chapters + fileList split
```

## Clean areas (not backlog)

- IPC boundary: runtime calls centralized in `tauriClient`; no UI `invoke()` bypass.
- Local audio import formats: backend-owned metadata; frontend does not maintain a parallel allowlist.
- Encoder bitrate whitelist: partial parity test exists (`audio-defaults.test.ts`).
