# Engineering Audit: GitHub Issue #81

**Issue**: "Implement Job Type Support and UI Refinements"
**Related Plan**: `docs/planning/cover-art-click-to-load.md`
**Audit Date**: 2025-12-10
**Auditor**: Claude (Senior Engineering Review)

---

## Executive Summary

Issue #81 is a comprehensive UI migration + parallel batch processing feature.

**Verdict: READY TO IMPLEMENT**

The original plan is well-structured. Key findings:
- Metadata handling **does not need to change** for batch mode (KISS principle)
- Twoloop should be **user-configurable** (product owner decision)
- Cover art needs **click + drag-drop handlers** (not just button)
- UI selectors should be **disabled during processing**

---

## Gaps Identified

### Gap 1: Job Type Field Missing from Payload (HIGH)

**Location**: `src-tauri/src/commands/audio.rs:86-92`

```rust
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub sample_rate: Option<audio::SampleRateConfig>,
    // MISSING: job_type field
}
```

**Required**: Add `job_type: Option<JobType>` with forking logic in `process_audiobook_files_v2`.

**Clarification on Batch Mode**:
- **Merge**: N chapter files -> 1 merged audiobook (current behavior)
- **Batch**: N complete audiobooks -> N converted outputs (processed in parallel)
- **Metadata**: Existing flow unchanged - each batch job uses user-provided metadata

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

**Decision**: Make user-configurable per product owner.

**Required Changes**:
1. Add `twoloop: bool` to `EncoderSettings` (`settings_encoder.rs`)
2. Wire checkbox in frontend encoder panel
3. Update native AAC encoder to respect `settings.twoloop`
4. Tech debt: Fix env var typo `ABB_DISABLE_TWOOLOOP` -> `ABB_DISABLE_TWOLOOP`

---

### Gap 4: UI Selector State During Processing (MEDIUM)

**Problem**: No specification for behavior when user changes selectors during active jobs.

**Backend constraint** (`audio.rs:76-82`):
```rust
pub async fn set_max_concurrent_jobs(...) -> Result<usize> {
    registry.update_max_concurrent(desired).await  // Requires idle state!
}
```

**Recommendation**: Disable Job Type and Max Concurrent selectors while `isProcessing === true`.

---

### Gap 5: Progress Event Throttling (LOW)

**Problem**: 4+ parallel batch jobs may flood UI with progress events.

**Recommendation**: Throttle aggregate progress updates to 500ms intervals.

---

## What The Plan Got Right

- Phase structure is well-organized
- Backend API audit correctly identified gaps
- UI component mapping is complete
- Testing checklists are comprehensive
- Phased approach minimizes risk

---

## Implementation Order (Recommended)

### Sprint 1: Foundation (Parallelizable)
- Phase 2: HTML structure migration
- Phase 3: CSS styling migration
- Phase 6.1: Add `JobType` enum + payload field

### Sprint 2: Backend Logic (Sequential)
- Phase 6.2: Batch processing in `process_audiobook_files_v2`
- Phase 6.3: Twoloop configuration

### Sprint 3: Frontend Logic
- Phase 4.1: Cover art click + drag-drop
- Phase 4.2: Output path live preview
- Phase 4.3: Job Controls wiring

### Sprint 4: Polish
- Phase 5: Documentation
- Contract verification (`scripts/ensure-contract.sh`)
- Integration testing

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Batch output file collisions | Medium | High | Pre-validate paths before processing |
| Progress UI flooding | Medium | Medium | Throttle to 500ms |
| Cover art drag conflicts | Medium | Medium | Prevent event bubbling |
| Selector change during processing | Medium | Low | Disable while processing |

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
