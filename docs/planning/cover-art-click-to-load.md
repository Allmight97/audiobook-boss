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
- Twoloop should be user-configurable (default ON, product owner confirmed)
- Cover art needs click + drag-drop handlers (not just button)
- UI selectors should be disabled during processing
- Batch mode requires explicit job type contract + per-file outputs to avoid collisions
- Output path contract: users choose a destination folder; app generates paths from metadata-based patterns with collision suffixing

> [!IMPORTANT]
> **Pre-Implementation Task (done in PR 83)**: Added support for `ABB_DISABLE_TWOLOOP` while keeping `ABB_DISABLE_TWOOLOOP` as a backward‑compatible alias.

**PR Strategy (planned vs actual)**:
- **Planned**: PR A for Phases 2‑4 (frontend migration), PR B for Phase 6 (JobType + twoloop backend/contract).
- **Actual**: PR 83 combined Phases 2‑4 **and** most Phase 6 work. Remaining Phase 6 gaps are now tracked at the end of this doc for a follow‑up PR.

### Issue #81 Contract Updates (authoritative for implementation)

**Backend**

- Add `JobType` enum (`merge`, `batch`) and `job_type: Option<JobType>` to `ProcessV2Payload` (default `merge`).
- Treat `output_dir` as destination folder (no manual filename entry). Generate per-job paths from metadata using the default pattern `[Author]/[Series]/[Title]/` + filename `Title (Year).m4b` (configurable in UI) and append `-1`, `-2`, … before `.m4b` on collision. Apply the same collision guard for merge and batch.
- Metadata: reuse the same payload per job (no schema change).
- EncoderSettings: add `twoloop: bool` (default `true`, camelCase); respect env override `ABB_DISABLE_TWOLOOP`.
- Native AAC: set `aac_coder=twoloop` when `twoloop` is true and env override allows.

**Frontend**

- Job Type selector (merge/batch) in input header; disable Job Type + Max Concurrent while processing.
- Output preview: read-only path built from the chosen folder + metadata-based directory pattern (default `[Author]/[Series]/[Title]/`) and filename pattern (default `Title (Year).m4b`). Batch preview shows the pattern rather than enumerating every file.
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
| **Output Location**    | Dir input + subdirectory checkbox + filename radios | Live path preview (folder-based) + integrated Browse + collapsible options |
| **Preview Button**     | Bottom of metadata tags section                     | In section header row, right-aligned                        |
| **Button Styles**      | `button-primary/secondary`                          | `btn-pill` family                                           |
| **Browse Button**      | Secondary gray                                      | Soft primary blue, integrated in path box                   |
| **Input Panel Header** | "Input Audio Files" h2                              | "Input and File Order" h3 with job controls                 |
| **Job Type Selector**  | ✅ Implemented in PR 83                             | Merge/Batch dropdown in header row                          |
| **Jobs Selector**      | ✅ Implemented in PR 83                             | Number of Jobs dropdown in header row                       |

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

- Replace directory input with live path preview box (read-only text)
- Move Browse button inside preview box
- Add collapsible "Path options" panel
- Change Browse to `btn-pill-primary-soft`
- Default directory pattern `[Author]/[Series]/[Title]/` (was `[Author]/[Series]/[Year-Title]/`)
- Default filename pattern `Title (Year).m4b`; keep radio to switch patterns; no manual typing

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

- Compute and display live path preview (read-only) from selected folder + metadata-based patterns
- Implement path options toggle
- Update path preview on metadata/settings change
- Update subdirectory builder default to `[Author]/[Series]/[Title]/`; keep filename default `Title (Year).m4b`; allow toggles but no freeform filename entry

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

## Manual UI Validation Checklist (PR 83 pre‑merge)

### 0. Setup
- [ ] Run the app via `bun run tauri dev` on macOS.
- [ ] Open DevTools console and keep it visible for errors.

### 1. File Import + File List
- [ ] Drag‑drop supported audio files into the main drop zone → list populates; invalid files show errors.
- [ ] Click the drop zone → picker accepts `mp3/m4a/m4b/aac` → list populates.
- [ ] Drop an image file into the main drop zone → shows “No supported audio files dropped…” and imports nothing.
- [ ] Select different files in the list → properties + metadata form update.
- [ ] Reorder (move up/down) and sort toggle → order updates without losing selection.

### 2. Metadata + Tag Preview
- [ ] Edit Title/Author/Series/Year → tag preview grid updates.
- [ ] Press Cmd+S/Ctrl+S while idle → metadata saves; status text flashes “Metadata saved!”.

### 3. Cover Art
- [ ] Auto‑loads cover art from first valid file (if present) unless custom art already set.
- [ ] Click cover art area → picker accepts `jpg/png/webp` → image displays; clear overlay appears on hover.
- [ ] Click clear overlay → image removed; placeholder returns; next Cmd+S removes art in file.
- [ ] Drag an image over cover art area → drag‑over visual appears; drop loads image.
- [ ] Drag audio near cover art area → not treated as cover art; still imports when dropped on main zone.

### 4. Output Panel
- [ ] Browse… selects output directory → preview box shows full computed path.
- [ ] Toggle “Use subdirectory pattern” and filename radios → preview updates.
- [ ] Expand/collapse “Path options” panel → layout stable.
- [ ] Change metadata fields → preview recomputes live.
- [ ] Switch Job Type:
  - Merge → preview shows concrete path.
  - Batch → preview switches to pattern hint. **Known gap:** hint doesn’t yet reflect user‑chosen filename/subdir options.

### 5. Encoder Panel
- [ ] Advanced settings accordion open/close works.
- [ ] Encoder selection restricts bitrate‑mode options correctly (Apple=CVBR only, Native=CBR only, FDK/Auto=VBR only).
- [ ] Twoloop checkbox visible only for Native AAC, default checked, persists after reload.
- [ ] Afterburner visible only for FDK; disabled if FDK unavailable.

### 6. Job Controls + Processing
- [ ] Max Concurrent selector persists across restart and backend accepts changes when idle.
- [ ] Start processing → Job Type + Max Concurrent selectors disable while running.
- [ ] Merge mode with multiple files → single output at previewed path.
- [ ] Batch mode with 2–3 files → multiple outputs; job list shows per‑job IDs; respects max concurrency.
- [ ] Cancel button cancels running jobs and UI returns to idle.
- [ ] Preview Audio dropdown durations create previews of roughly selected length.

---

## Pre‑merge Actions for PR 83
1. Run full quality gates:
   - `scripts/quick-checks.sh`
   - `scripts/ensure-contract.sh`
   - `cargo test` (from `src-tauri/`)
   - `bun run build`
2. Re‑scan PR head for prior review findings:
   - Template literal syntax error ✅ fixed in `f8c1e6d`.
   - WebP magic‑byte detection ✅ fixed in `f8c1e6d`.
   - Misleading HTML comment ✅ no longer present at head.
3. Decide polish vs defer:
   - Batch preview accuracy + disabled‑opacity CSS are safe follow‑ups (see Phase 6 remainder).
   - Archive correctness for `docs/IGNORE_ARCHIVE/old_ui.html` can be fixed post‑merge unless you want a perfect historical snapshot pre‑merge.

---

## Phase 6 Remaining Work (Follow‑up PR after PR 83 merges)

### Goal
Close the small gaps between the Phase 6 spec and PR 83, without re‑opening broad UI migration diffs.

### Implementation Steps
1. **Batch output preview reflects user choices**
   - Update the batch branch of `calculateOutputPath()` in `src/ui/outputPanel.ts` to:
     - Respect `currentState.useSubdirPattern` (include/exclude `[Author]/[Series]/[Title]`).
     - Use `buildFilename(metadata)` so batch preview matches selected filename pattern.
     - Remove or clarify the artificial `/(Batch Output Folder)` suffix if backend doesn’t create it.
   - Verify with a focused manual test (Section 4 above).
2. **Move disabled styling to CSS (Separation of Concerns)**
   - Remove inline `style.opacity` toggles from `setJobControlsEnabled()` in `src/ui/jobControls.ts`.
   - Add a CSS rule like `select:disabled { opacity: 0.5; cursor: not-allowed; }` to `src/styles.css`.
3. **Clean up contract comments**
   - Update `src/types/audio.ts` comments (JobType/twoloop are no longer “pending backend support”).
4. **Docs: batch path contract**
   - Create `docs/planning/batch-processing-paths.md` documenting current `build_output_path` + collision rules and expected UX.
5. **Optional tech‑debt reduction**
   - Split `src/ui/statusPanel/logic.ts` if you need to add more logic there in the follow‑up PR.

### Success Criteria
- Batch preview matches merge preview logic + selected patterns.
- `scripts/ensure-contract.sh` stays green.
- No regressions in the PR 83 manual UI checklist.
