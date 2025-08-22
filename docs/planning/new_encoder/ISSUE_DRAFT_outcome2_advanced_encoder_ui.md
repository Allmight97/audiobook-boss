# Epic: Outcome 2 — Advanced Encoder Options + UI Redesign

Labels: enhancement, epic, audio, ui, tauri, ffmpeg

## Summary
Break down the advanced encoder/UI redesign into phases with clear contracts, validation, and test coverage. macOS-first with AAC-AT default; native AAC HEv1/HEv2 supported elsewhere. Single engine: ffmpeg-next.

Related docs:
- Dependency Map: `docs/planning/new_encoder/advanced_encoder_dependency_map.md`
- Expanded Plan: `docs/planning/new_encoder/outcome2_advanced_encoder_plan.md`
- Test Gaps Report: `docs/planning/new_encoder/encoding_test_gaps.md`
- Original design: `docs/planning/new_encoder/outcome2_advanced_encoder_ui.md`

## Goals
- Expose encoder options (AAC-AT/HE-AAC v1/v2, bitrate, channels) and advanced toggles (coder, afterburner, threads) with validation and logs
- Redesign UI panel to host these controls and a future profile selector placeholder

## Phases & Tasks

### Phase 0 — Scaffolding and analysis
- [x] Add dependency map
- [x] Add expanded implementation plan
- [x] Add test gaps report
- [x] Confirm API strategy: in-place upgrade of existing command/types in one PR; update all call sites; no v2 shim

### Phase 1 — Types and validation (no UI yet)
Backend
- [ ] Add `EncoderType`, `AacCoder`, `ThreadSetting`, `EncoderSettings` types (serde derive)
- [ ] Add validation: bitrate whitelist; HE-AAC v2 stereo enforcement; threads bounds
- [ ] FFI helper: encoder-by-name (`aac_at`) with fallback
- [ ] New tauri command: `validate_encoder_settings`
 - [ ] Feature flag plumbing for optional FDK afterburner (commented-out stubs now; no-op unless available in future)

Frontend
- [ ] TS types for encoder settings and defaults per platform

### Phase 2 — Command upgrade and wiring
Backend
- [ ] Upgrade `process_audiobook_files` to accept extended payload with `EncoderSettings`
- [ ] Build `MediaProcessingPlan` from upgraded payload

Frontend
- [ ] Update `main.ts` to send extended payload

### Phase 3 — Encoder mapping in ffmpeg-next
- [ ] Choose encoder (`aac_at` on macOS) or native `aac`
- [ ] Apply profile for HE-AAC v1/v2 (best-effort) via `av_opt_set_int(profile, ...)`
- [ ] Apply `aac_coder` twoloop/fast for native AAC
- [ ] Apply threads (auto/off/fixed) best-effort
- [ ] Afterburner: only if `libfdk_aac` encoder active (feature-flagged); otherwise log ignored
- [ ] INFO summary log with resolved params; DEBUG logs for each set/ignore

### Phase 4 — UI redesign
- [ ] New Encoder panel with controls and tooltips; disabled states for unsupported combos
- [ ] macOS warning when switching away from AAC-AT
- [ ] HE-AAC v2 locks channels=2 and shows a toast: "HE-AAC v2 is stereo-only (Parametric Stereo). For mono, use HE-AAC v1"

### Phase 5 — Tests & docs
- [ ] Add unit/integration tests per report
- [ ] Update external docs with specifics (profiles, threads, encoder selection)

## Acceptance criteria
- Validation errors for invalid combos (e.g., HEv2 + mono)
- Logs summarize encoder params and note ignored toggles per encoder
- macOS: AAC-AT path exercised successfully; native AAC paths remain functional on other OSes

## Risks / Notes
- "Afterburner" requires libfdk_aac; not available with native AAC or AAC-AT — confirm build capabilities and gate UI accordingly
- Encoder-by-name support may require minimal FFI wrapper if ffmpeg-next API lacks it
- Keep backward compatibility for existing UI until v2 path stabilizes

---

Use this issue to track the epic. Subtasks can be created from the phase checklists.
