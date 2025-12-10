# Engineering Audit: GitHub Issue #81

**Issue**: "Implement Job Type Support and UI Refinements"
**Related Plan**: `docs/planning/cover-art-click-to-load.md`
**Audit Date**: 2025-12-10
**Auditor**: Claude (Senior Engineering Review)

---

## Executive Summary

Issue #81 is a UI migration + parallel batch processing feature.

**Verdict: READY TO IMPLEMENT (with contract updates below)**

Key confirmations:

- Metadata handling stays the same; reuse the existing payload per job.
- Twoloop is user-configurable for Native AAC; default ON.
- Cover art needs click + drag-drop (area-level) plus overlay toggle.
- Job selectors must be disabled while processing.
- Batch mode needs an explicit job type contract and per-file output naming to avoid collisions.

---

## Gaps Identified

### Gap 1: Job Type & Batch Fan-Out (HIGH)

**Problem**: Payload has no `job_type`; backend always runs a single job and writes one output path. Batch mode needs N jobs and N outputs with collision-safe naming.

**Required**:

- Add `JobType` enum (`merge`, `batch`) and `job_type: Option<JobType>` (default `merge`) to `ProcessV2Payload`.
- In `process_audiobook_files_v2`, if `batch`, spawn one job per validated input, each with its own output path. Keep merge behavior unchanged.
- Per-file output naming for batch: `<output_dir>/<input_stem>.m4b`; on collision append `-1`, `-2`, … (fail fast if still colliding).
- Reuse the same metadata payload per job (no schema change).

**Rust sketch**:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType { Merge, Batch }

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>, // default merge
}
```

```rust
match payload.job_type.unwrap_or(JobType::Merge) {
    JobType::Merge => run_single_merge_job(...),
    JobType::Batch => {
        for input in validated_inputs {
            let out = next_available_path(&payload.output_dir, &input)?;
            spawn_batch_job(input, out, metadata.clone(), registry.clone(), ...).await?;
        }
    }
}
```

---

### Gap 2: Cover Art Click-to-Load + Drag-Drop (HIGH)

**Location**: `src/ui/coverArt.ts:14-28`

Current implementation only has button-click loading. Mock UI shows:

- Click on `#cover-art-area` opens file picker
- Drag-drop on area loads image
- Clear overlay button (absolute positioned)

**Missing Implementation**:

1. Click handler on area itself
2. Drag-drop events (`dragenter`, `dragover`, `dragleave`, `drop`)
3. `.has-image` class toggle for overlay visibility

---

### Gap 3: Twoloop Configuration (MEDIUM)

**Decision**: User-configurable, default ON.

**Required Changes**:

1. Add `twoloop: bool` to `EncoderSettings` (`settings_encoder.rs`), serde camelCase.
2. Respect in native AAC options; when true set `aac_coder=twoloop`.
3. Keep env override but fix typo to `ABB_DISABLE_TWOLOOP`.
4. Frontend checkbox under native AAC only; default checked.

**Rust sketch**:

```rust
pub struct EncoderSettings {
    pub encoder_type: EncoderType,
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,
    pub channels: ChannelConfig,
    pub afterburner: bool,
    pub threads: ThreadSetting,
    pub twoloop: bool, // NEW default true
}
```

---

### Gap 4: UI Selector State During Processing (MEDIUM)

**Problem**: Selectors can change mid-flight; backend requires idle for `set_max_concurrent_jobs`.

**Recommendation**: Disable Job Type and Max Concurrent selectors while `isProcessing === true`; surface backend errors if idle check fails.

---

### Gap 5: Progress Event Throttling (LOW)

**Problem**: 4+ parallel batch jobs may flood the UI.

**Recommendation**: Throttle aggregate progress updates to ~500ms.

---

## What The Plan Got Right

- Phase structure is well-organized
- Backend API audit correctly identified gaps
- UI component mapping is complete
- Testing checklists are comprehensive
- Phased approach minimizes risk

---

## Implementation Order (Recommended)

### Sprinting Plan (Recommended)

- Sprint 1: HTML/CSS migration; add `JobType` enum/payload.
- Sprint 2: Backend batch fan-out + collision-safe naming; twoloop wiring (Rust + env typo fix).
- Sprint 3: Frontend logic — cover art click/drag, job selectors disable, output preview copy for batch, twoloop toggle.
- Sprint 4: Progress throttling; docs; contract check; integration tests.

---

## Risk Assessment

| Risk                              | Likelihood | Impact | Mitigation                           |
| --------------------------------- | ---------- | ------ | ------------------------------------ |
| Batch output file collisions      | Medium     | High   | Pre-validate paths before processing |
| Progress UI flooding              | Medium     | Medium | Throttle to 500ms                    |
| Cover art drag conflicts          | Medium     | Medium | Prevent event bubbling               |
| Selector change during processing | Medium     | Low    | Disable while processing             |

---

## Critical Files

**Backend**:

- `src-tauri/src/commands/audio.rs` - ProcessV2Payload, batch logic
- `src-tauri/src/audio/settings_encoder.rs` - EncoderSettings (twoloop)

**Frontend**:

- `index.html` - HTML structure migration
- `src/styles.css` - CSS migration
- `src/ui/coverArt.ts` - Click/drag-drop handlers
- `src/ui/statusPanel/logic.ts` - Progress aggregation

**Reference**:

- `docs/specs/UI_mock/mock_ui.html` - Target design
- `docs/planning/cover-art-click-to-load.md` - Implementation plan
