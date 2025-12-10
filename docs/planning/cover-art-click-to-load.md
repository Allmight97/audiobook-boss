# Mock UI → Production Migration Plan

## Overview

Align all repository surfaces with the new UI design in `docs/specs/UI_mock/mock_ui.html`.

## Deliverables

1. **Archive current UI** → `/docs/IGNORE_ARCHIVE/old_ui.html`
2. **Implement UI changes** → `index.html`, `src/styles.css`, `src/ui/*.ts`
3. **Implement Job Type Support** → Backend + Frontend (see Phase 6)
4. **Create GitHub Issue** → [Issue #81](https://github.com/Allmight97/audiobook-boss/issues/81) (Created)

---

## Engineering Audit Summary (2025-12-10)

**Verdict: READY TO IMPLEMENT**

Key findings from audit:

- Metadata handling does NOT need to change for batch mode (KISS)
- Twoloop should be user-configurable (product owner decision)
- Cover art needs click + drag-drop handlers (not just button)
- UI selectors should be disabled during processing
- Batch mode requires explicit job type contract + per-file outputs to avoid collisions

### Issue #81 Contract Updates (authoritative for implementation)

**Backend**

- Add `JobType` enum (`merge`, `batch`) and `job_type: Option<JobType>` to `ProcessV2Payload` (default `merge`).
- Batch mode fan-out: one job per input file via `JobRegistry`; per-file output naming `<output_dir>/<stem>.m4b` with `-1`, `-2`, … suffix on collision.
- Metadata: reuse the same payload per job (no schema change).
- EncoderSettings: add `twoloop: bool` (default `true`, camelCase); respect env override `ABB_DISABLE_TWOLOOP`.
- Native AAC: set `aac_coder=twoloop` when `twoloop` is true and env override allows.

**Frontend**

- Job Type selector (merge/batch) in input header; disable Job Type + Max Concurrent while processing.
- Output preview: merge shows single path; batch shows directory + stem-based pattern (copy only).
- Native AAC twoloop checkbox (default checked; hide for other encoders).
- Cover art: click area opens picker; drag/drop with `dragenter/over/leave/drop`, stopPropagation to avoid bubbling to file import; toggle `.has-image` for overlay; clear overlay shown only when image exists.
- Progress throttling: aggregate/progress UI updates at ~500ms cadence to prevent flooding with 4+ jobs.

**Rust sketch**

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType { Merge, Batch }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>, // default merge
}
```

**TS throttling sketch**

```ts
const THROTTLE_MS = 500;
let last = 0;
function handleProgress(ev: ProgressEvent) {
  const now = Date.now();
  if (now - last < THROTTLE_MS) return;
  last = now;
  renderProgress(ev);
}
```

See full audit: `docs/reports/issue-81-engineering-audit.md`

---

## UI Changes Summary

| Component              | Current (Production)                                | New (Mock)                                                  |
| ---------------------- | --------------------------------------------------- | ----------------------------------------------------------- |
| **Section Headers**    | Mixed h2/h3, inconsistent sizing                    | All h3, 0.875rem, 500 weight                                |
| **Cover Art**          | Separate Load/Clear buttons                         | Clickable area + overlay clear                              |
| **Encoder Options**    | Separate boxed section                              | Inline in Row 2 with Channels                               |
| **Output Location**    | Dir input + subdirectory checkbox + filename radios | Live path preview + integrated Browse + collapsible options |
| **Preview Button**     | Bottom of metadata tags section                     | In section header row, right-aligned                        |
| **Button Styles**      | `button-primary/secondary`                          | `btn-pill` family                                           |
| **Browse Button**      | Secondary gray                                      | Soft primary blue, integrated in path box                   |
| **Input Panel Header** | "Input Audio Files" h2                              | "Input and File Order" h3 with job controls                 |
| **Job Type Selector**  | ❌ Not implemented                                  | Merge/Batch dropdown in header row                          |
| **Jobs Selector**      | ❌ Not wired to UI                                  | Number of Jobs dropdown in header row                       |

---

## Phase 1: Archive Current UI ✅ Completed

### [NEW] `/docs/IGNORE_ARCHIVE/old_ui.html`

- Copy current `index.html` with inlined CSS
- Preserve as single-file snapshot for A/B comparison

---

## Phase 2: HTML Changes

### [MODIFY] `index.html`

#### Input Panel

- Change h2 "Input Audio Files" → h3 "Input and File Order"
- Add Job Type selector and Jobs selector in header row
- Update button classes to `btn-pill btn-pill-secondary`

#### Metadata Section

- Rename "Metadata" → "Metadata Manager"
- Replace Load/Clear Cover Art buttons with clickable area + clear overlay
- Add `data-testid="cover-art-area"` attribute

#### Audio Encoder Settings

- Move encoder options (Afterburner/Twoloop) inline into Row 2
- Remove separate encoder-options-section box
- Shorten option descriptions

#### Output Location

- Replace directory input with live path preview box
- Move Browse button inside preview box
- Add collapsible "Path options" panel
- Change Browse to `btn-pill-primary-soft`

#### Metadata Tags Preview

- Move Preview Audio button to section header row
- Rename section header

---

## Phase 3: CSS Changes

### [MODIFY] `src/styles.css`

```css
/* Add: Pill button family */
.btn-pill {
  ...;
}
.btn-pill-primary {
  ...;
}
.btn-pill-secondary {
  ...;
}
.btn-pill-primary-soft {
  background: #6ba3f7;
  ...;
}

/* Add: Cover art click-to-load */
.cover-art-area {
  cursor: pointer;
}
.cover-art-clear-btn {
  position: absolute;
  top: 4px;
  ...;
}
.cover-art-area.has-image .cover-art-clear-btn {
  display: flex;
}

/* Add: Output path preview */
.output-preview-box {
  display: flex;
  ...;
}
.output-path-text {
  flex: 1;
  overflow: hidden;
  ...;
}
.path-segment {
  font-weight: 500;
}
.path-options-toggle {
  ...;
}
.path-options-panel {
  display: none;
}
.path-options-panel.expanded {
  display: block;
}

/* Fix: Unified h3 sizing */
h3 {
  font-size: 0.875rem;
  font-weight: 500;
  margin: 0;
}

/* Fix: Profile display height */
.profile-display {
  padding: 0.35rem 0.5rem;
  font-size: 0.8125rem;
}
```

---

## Phase 4: TypeScript Changes

### [MODIFY] `src/ui/coverArt.ts`

- Add click handler to `#cover-art-area` → opens file picker
- Add click handler to clear overlay → clears cover art
- Implement `.has-image` class toggle
- **[AUDIT]** Add drag-drop handlers (`dragenter`, `dragover`, `dragleave`, `drop`)
- **[AUDIT]** Prevent event bubbling to audio file import drop zone

```typescript
// Implementation pattern for drag-drop (from audit)
const coverArtArea = document.getElementById("cover-art-area");
coverArtArea?.addEventListener("dragover", (e) => {
  e.preventDefault();
  e.stopPropagation(); // Prevent bubbling to file import
  coverArtArea.classList.add("drag-over");
});
coverArtArea?.addEventListener("drop", async (e) => {
  e.preventDefault();
  e.stopPropagation();
  coverArtArea.classList.remove("drag-over");
  const file = e.dataTransfer?.files[0];
  if (file && isImageFile(file)) {
    await loadCoverArtFromFile(file.path);
  }
});
```

### [MODIFY] `src/ui/outputPanel.ts`

- Compute and display live path preview
- Implement path options toggle
- Update path preview on metadata/settings change

### [NEW] `src/ui/jobControls.ts` (if needed)

- Handle Job Type selector
- Handle Max Concurrent Jobs selector
- **[AUDIT]** Disable both selectors while `isProcessing === true`
- **[AUDIT]** Backend `set_max_concurrent_jobs` requires idle state - frontend must enforce

```typescript
// Implementation pattern (from audit)
export function setJobControlsEnabled(enabled: boolean): void {
  const jobTypeSelect = document.getElementById(
    "job-type-select"
  ) as HTMLSelectElement;
  const maxConcurrentSelect = document.getElementById(
    "max-concurrent-select"
  ) as HTMLSelectElement;
  if (jobTypeSelect) jobTypeSelect.disabled = !enabled;
  if (maxConcurrentSelect) maxConcurrentSelect.disabled = !enabled;
}
```

---

## Phase 5: Documentation

### [MODIFY] `CHANGELOG.md` or create release notes

- Document UI changes for users

---

## Testing Checklist

- [ ] Cover art: click opens picker, clear overlay works
- [ ] Encoder options: display inline correctly
- [ ] Output preview: path computes from metadata
- [ ] Path options: toggle expand/collapse works
- [ ] Browse button: opens directory picker
- [ ] Preview Audio: button in header row works
- [ ] All buttons: consistent pill styling
- [ ] Light/dark theme: both render correctly
- [ ] No console errors

---

## Backend API Audit

| UI Feature              | Required Backend                        | Status                |
| ----------------------- | --------------------------------------- | --------------------- |
| Cover Art Click-to-Load | `commands::load_cover_art_file`         | ✅ Exists             |
| Jobs Selector           | `commands::get/set_max_concurrent_jobs` | ✅ Exists             |
| Browse Directory        | `dialog.open()` (Tauri built-in)        | ✅ Built-in           |
| Live Path Preview       | Frontend-only computation               | ✅ N/A                |
| Encoder Options         | No change (same settings)               | ✅ N/A                |
| **Job Type Selector**   | `ProcessV2Payload.job_type`             | ⚠️ **Gap identified** |
| **Twoloop Option**      | `EncoderSettings.twoloop`               | ⚠️ **Gap identified** |

> [!WARNING] > **Gap: Job Type Support**
>
> Current `ProcessV2Payload` has no `job_type` field. Backend (`process_audiobook_files_v2`)
> always merges all input files into a single output. For "batch single" mode (process each
> file as a separate .m4b), backend needs new logic.

> [!NOTE] > **Gap: Twoloop Configuration**
>
> Backend currently enables `aac_coder=twoloop` by default for native AAC but does not expose
> it in `EncoderSettings`. Mock UI shows it as a toggle.

---

## Phase 6: Job Type & Encoder Support (Backend + Frontend)

### Gap Analysis

**Current behavior:**

- `process_audiobook_files_v2` always merges files (❌ no batch mode)
- `twoloop` is hardcoded on (❌ not user configurable)

**Required changes:**

#### Backend: `src-tauri/src/commands/audio.rs` & `settings_encoder.rs`

```rust
// Add to EncoderSettings (settings_encoder.rs)
pub struct EncoderSettings {
    pub encoder_type: EncoderType,
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,
    pub channels: ChannelConfig,
    pub afterburner: bool,
    pub threads: ThreadSetting,
    pub twoloop: bool,  // NEW - native AAC only, defaults to true
}

// Add JobType enum (audio.rs)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Merge,  // N files → 1 output (current behavior)
    Batch,  // N files → N outputs (parallel via JobRegistry)
}

// Add to ProcessV2Payload (audio.rs)
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,  // NEW - defaults to Merge for backward compat
}
```

**Logic changes:**

1. **Job Type:** Implement fork logic in `process_audiobook_files_v2` (Merge vs Batch).
2. **Twoloop:** Update `native.rs` to use `settings.twoloop` instead of hardcoded default.

**[AUDIT] Batch Mode Clarification:**

- **Merge**: User drops chapter files → App merges into 1 audiobook
- **Batch**: User drops complete audiobooks → App converts each in parallel
- **Metadata**: Existing flow unchanged - each batch job uses user-provided metadata (KISS)
- **Output paths**: Frontend generates per-file paths using original filename stem

**[AUDIT] Tech Debt:** Fix env var typo `ABB_DISABLE_TWOOLOOP` → `ABB_DISABLE_TWOLOOP`

#### Frontend: `src/ui/jobControls.ts` & `src/ui/encoderPanel` (new)

```typescript
// Wire #job-type-select to processing payload
// Wire #twoloop-toggle to encoder settings
```

#[derive(serde::Deserialize)]
pub enum JobType { #[serde(rename = "merge")]
Merge, // Merge all files → 1 output #[serde(rename = "batch")]  
 BatchSingle, // Each file → separate output (uses JobRegistry concurrency)
}

````

**Logic change in `process_audiobook_files_v2`:**
- If `job_type == Merge`: current behavior (process all files together)
- If `job_type == BatchSingle`: spawn N separate jobs via `JobRegistry` (one per input file)

#### Frontend: `src/ui/jobControls.ts` (new)

```typescript
// Wire #job-type-select to processing payload
// Wire #max-concurrent-select to set_max_concurrent_jobs command
````

### Testing Checklist (Phase 6)

**Batch Processing:**

- [ ] Merge mode: multiple files → single .m4b
- [ ] Batch mode: 3 files → 3 separate .m4b
- [ ] Batch mode respects max concurrent jobs
- [ ] Per-job cancellation works in batch mode
- [ ] Progress aggregation works for batch jobs

**Cover Art:**

- [ ] Click on cover art area opens file picker
- [ ] Drag-drop image onto cover art area loads it
- [ ] Clear overlay button removes cover art
- [ ] Drag-drop doesn't bubble to audio file import

**UI State:**

- [ ] Job Type selector disabled during processing
- [ ] Max Concurrent selector disabled during processing
- [ ] Twoloop checkbox toggles correctly for native AAC

**Regression:**

- [ ] Output path still updates and works same as 'main' branch
- [ ] Preview Audio button still works same as 'main' branch
- [ ] Light/dark theme both render correctly
- [ ] No console errors
