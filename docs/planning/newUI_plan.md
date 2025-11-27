# UI Replacement Plan: Audiobook Boss

## Overview
Replace the existing UI with the new mock design (`docs/specs/UI_mock/new_UI_final.html`) while maintaining all existing functionality and addressing identified gaps.

---

## Critical Gaps Identified

### GAP 1: Cover Art Optimization (MISSING)
**Requirement**: Standardize all cover art to 800×800 JPEG at 85% quality
**Current State**: No optimization logic exists. Cover art is loaded and written as-is.
**Impact**: Large cover art causes slow metadata writes; inconsistent formats/sizes.

**Optimization Spec**:
- Max resolution: **800×800** (maintains aspect ratio, fits within bounds)
- Format: **JPEG 85%** quality
- Transparency: **Flatten to white background** before JPEG conversion
- Trigger: Always optimize (standardize all covers)

**Changes Required**:
- **Backend** (`src-tauri/src/commands/metadata.rs`): Add `optimize_cover_art()` function
  - Resize to max 800×800 (preserve aspect ratio)
  - Flatten any transparency to white background
  - Convert to JPEG at 85% quality
  - Use `image` crate for operations
- **Backend** (`src-tauri/src/metadata/reader.rs`): Optimize cover art from `read_audio_metadata`
  - Auto-loaded cover art from source files ALSO gets optimized (Codex feedback)
- **Frontend**: No changes needed (optimization happens transparently)

### GAP 2: Bitrate Options Mismatch
**Requirement**: 48-128k in 8-step increments (48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128)
**Current Backend Whitelist**: `[48, 56, 64, 72, 80, 88, 96, 112, 128]` - missing 104, 120
**Impact**: UI cannot offer full range without backend validation failure.

**Changes Required**:
- **Backend** (`src-tauri/src/audio/settings_encoder.rs`): Add 104, 120 to `VALID_BITRATES`
- **Frontend** (`src/types/audio.ts`): Update `VALID_ENCODER_BITRATES` tuple
- **New UI**: Dropdown options 48-128 in steps of 8

### GAP 3: Preview Duration Dropdown
**Requirement**: Primary button = 30s, dropdown = 15s, 45s, 60s
**Current State**: Backend supports `preview_seconds` parameter; UI hardcodes 30s
**Impact**: Minor - just needs UI wiring.

**Changes Required**:
- **Frontend only**: Add split button with dropdown, pass selected duration to `process_audiobook_files_v2`

### GAP 4: Element ID Mismatches (Composer Feedback)
**Issue**: Mock uses different IDs than current TypeScript code.
**Decision**: Use mock IDs (more semantic, future-proof).

**ID Changes Required**:
| Current ID | Mock ID | Files Affected |
|------------|---------|----------------|
| `meta-series-pos` | `meta-series-part` | `fileList/actions.ts`, `statusPanel/logic.ts` |
| `output-estimated-size` | `estimated-size` | `outputPanel.ts` |

### GAP 5: Tag Preview Implementation Details (Composer Feedback)
**Issue**: Plan lacked TSOA calculation and field mapping specifics.

**Metadata Field Mapping** (Form → Tag):
| Form Field | Tag Name | Notes |
|------------|----------|-------|
| `meta-title` | Title, Album | Same value for both |
| `meta-author` | Artist, Album Artist | Same value for both |
| `meta-narrator` | Composer | Narrator stored here |
| `meta-series` | MVNM | Movement Name tag |
| `meta-series-part` | MVIN | Movement Number tag |
| `meta-year` | TYER | Release year |
| `meta-genre` | TCON | Genre tag |
| (calculated) | TSOA | Title Sort Order (Plex hack) |

**TSOA Calculation Logic**:
```typescript
function padPart(num: string): string {
  const n = parseInt(num, 10);
  if (isNaN(n)) return '00';
  return n < 10 ? `0${n}` : `${n}`;
}

function calculateTSOA(series: string, part: string, title: string): string {
  if (!series || !title) return '';
  return `${series} ${padPart(part)} - ${title}`;
}
```

---

## Tailwind CSS: CDN vs npm Integration

### Option A: CDN (Quick Start)
**Pros**:
- Immediate compatibility with mock
- Zero build configuration
- Faster initial development

**Cons**:
- **Requires internet** on first load (cached after)
- **Breaks offline** in packaged app (critical issue)
- Larger payload (full Tailwind vs tree-shaken)
- Runtime CSS parsing overhead

### Option B: npm Integration (Recommended)
**Pros**:
- **Works offline** in packaged desktop app
- Tree-shaking removes unused CSS (~3KB vs ~300KB)
- Better performance (build-time CSS)
- Proper version control

**Cons**:
- Requires `tailwind.config.js` setup
- Build pipeline integration needed
- Initial setup time (~30 min)

### Recommendation: **npm Integration**
Given the app will be packaged for macOS/Windows/Linux distribution, offline capability is essential. The CDN approach would break in bundled apps without network access.

**Setup Steps**:
1. `npm install -D tailwindcss postcss autoprefixer`
2. Create `tailwind.config.js` with custom theme (matching mock's CSS variables)
3. Add Tailwind directives to `src/styles.css`
4. Update `vite.config.ts` for PostCSS

---

## Implementation Plan

### Phase 1: Foundation (Backend + Build Setup)
**Goal**: Prepare backend and build system before UI changes

#### 1.1 Add Cover Art Optimization (Backend)
- Add `image` crate dependency to `Cargo.toml`
- Create `optimize_cover_art(bytes: Vec<u8>) -> Result<Vec<u8>>` in metadata module:
  - Resize to max 800×800 (preserve aspect ratio)
  - Flatten transparency to white background
  - Convert to JPEG at 85% quality
- Call from `load_cover_art_file` command (user-selected art)
- Call from `read_audio_metadata` (auto-loaded art from source files - Codex feedback)
- **Files**: `src-tauri/Cargo.toml`, `src-tauri/src/commands/metadata.rs`, `src-tauri/src/metadata/reader.rs`

#### 1.2 Update Bitrate Whitelist
- Add 104, 120 to backend validation
- Update TypeScript constant
- **Files**: `src-tauri/src/audio/settings_encoder.rs`, `src/types/audio.ts`

#### 1.3 Tailwind npm Setup
- Install dependencies
- Create config with mock's color palette
- Integrate with Vite build
- **Files**: `package.json`, `tailwind.config.js`, `postcss.config.js`, `src/styles.css`

### Phase 2: HTML Structure Replacement
**Goal**: Replace `index.html` with new layout

#### 2.1 Replace index.html
- Copy structure from mock
- Preserve all existing element IDs (critical for TypeScript bindings)
- Remove CDN script, use npm Tailwind classes
- Remove keyboard shortcut hint (`Press A to toggle`)
- **Files**: `index.html`

#### 2.2 Update CSS Variables
- Merge mock's CSS variables with existing styles
- Preserve dark mode support
- Update component styles for new layout
- **Files**: `src/styles.css`

### Phase 3: TypeScript Module Updates
**Goal**: Wire existing modules to new DOM structure

#### 3.1 Update Output Panel (`src/ui/outputPanel.ts`)
- Update dropdown options for bitrate (48-128, step 8)
- Add sample rate "Auto (Pass-through)" option
- Wire existing functionality to new element structure
- **Files**: `src/ui/outputPanel.ts`

#### 3.2 Add Preview Duration State (`src/ui/statusPanel/`)
- Add `previewDuration` state variable (default: 30)
- Wire split button dropdown to update state
- Pass duration to `process_audiobook_files_v2` call
- **Files**: `src/ui/statusPanel/logic.ts`, `src/ui/statusPanel/dom.ts`

#### 3.3 Add Tag Preview Grid Logic
- Create `updateTagPreview()` function (port from mock's JavaScript)
- Wire to metadata input change events
- Calculate TSOA (Title Sort Order) for Plex compatibility
- **Files**: `src/ui/outputPanel.ts` or new `src/ui/tagPreview.ts`

#### 3.4 Update Advanced Settings Panel
- Wire encoder type dropdown to existing `EncoderSettings`
- Wire profile dropdown (LC, HE v1, HE v2)
- Keep VBR checkbox disabled (per mock)
- Remove keyboard shortcut handler (and hint text from HTML)
- Note: Encoder panel is split module (`src/ui/encoderPanel/*` not single file - per Codex)
- **Files**: `src/ui/encoderPanel/*` (index.ts, state.ts, dom.ts, actions.ts)

#### 3.5 Preserve File List Functionality
- Ensure drag-to-reorder still works
- Verify file properties display
- Test file selection → metadata population
- **Files**: `src/ui/fileList/*.ts` (minimal changes expected)

#### 3.6 Preserve Cover Art Functionality
- Verify drag-drop on new smaller cover area
- Test load/clear buttons
- Verify optimization triggers on load
- **Files**: `src/ui/coverArt.ts` (minimal changes expected)

### Phase 4: Update Mocks
**Goal**: Keep dev environment functional

#### 4.1 Update Mock Implementations
- Ensure `load_cover_art_file` mock reflects optimization behavior
- **Files**: `src/lib/mocks.ts`

### Phase 5: Verification & Testing (Optional Addon)
**Goal**: Ensure all functionality works

#### 5.1 Manual Testing Checklist
- [ ] File drag-drop and selection
- [ ] Metadata form population from file
- [ ] Cover art load/clear/auto-load
- [ ] Tag preview real-time updates
- [ ] Bitrate dropdown (all 11 options)
- [ ] Sample rate dropdown (Auto + explicit values)
- [ ] Advanced settings expand/collapse
- [ ] Preview button with duration dropdown
- [ ] Full processing workflow
- [ ] Cancel processing
- [ ] Progress bar updates

#### 5.2 Optional: Add Vitest Tests
- Test tag preview calculation logic
- Test bitrate validation
- Test cover art optimization (mock image crate)
- **Files**: `src/**/*.test.ts`

---

## File Change Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src-tauri/Cargo.toml` | Modify | Add `image` crate dependency |
| `src-tauri/src/commands/metadata.rs` | Modify | Add cover art optimization |
| `src-tauri/src/metadata/reader.rs` | Modify | Optimize auto-loaded cover art |
| `src-tauri/src/audio/settings_encoder.rs` | Modify | Add bitrates 104, 120 |
| `src/types/audio.ts` | Modify | Update bitrate constant |
| `package.json` | Modify | Add Tailwind dependencies |
| `tailwind.config.js` | Create | Tailwind configuration |
| `postcss.config.js` | Create | PostCSS configuration |
| `index.html` | Replace | New layout structure with mock IDs |
| `src/styles.css` | Modify | Merge mock styles, CSS variables |
| `src/ui/outputPanel.ts` | Modify | Bitrate/sample rate dropdowns, `estimated-size` ID |
| `src/ui/statusPanel/logic.ts` | Modify | Preview duration state, `meta-series-part` ID |
| `src/ui/statusPanel/dom.ts` | Modify | Split button DOM handling |
| `src/ui/fileList/actions.ts` | Modify | Update `meta-series-part` ID reference |
| `src/ui/encoderPanel/*` | Modify | Wire advanced settings panel |
| `src/lib/mocks.ts` | Modify | Update for new behaviors |

---

## Decisions Confirmed

| Item | Decision |
|------|----------|
| Keyboard 'A' shortcut | Remove entirely |
| Bitrate options | 48-128k in 8-step increments (11 options) |
| Preview durations | Primary = 30s, Dropdown = 15s, 45s, 60s |
| Cover art optimization | 800×800 max, JPEG 85%, white background for transparency |
| Cover art paths | Optimize BOTH user-selected AND auto-loaded from source files |
| Sample rate Auto | Pass-through (no resampling) |
| Tailwind | npm integration (offline support for packaged app) |
| Old UI | Replace directly (no archive) |
| PR Strategy | 2 PRs: Backend foundation first, then UI replacement |
| Element IDs | Use mock IDs (`meta-series-part`, `estimated-size`) |

---

## Execution Strategy: 2 PRs

### PR 1: Backend Foundation
**Branch**: `feature/backend-ui-prep`

**Executive Summary**: Prepares backend infrastructure for new UI capabilities. Adds cover art optimization (auto-resize/convert large images) and expands bitrate whitelist. These changes are backward-compatible with the existing UI and can be merged independently.

**Why This PR Exists**: The new UI requires backend support for cover art optimization and additional bitrate options (104k, 120k). Making these changes first ensures the UI replacement PR has a stable foundation without introducing backend changes alongside UI changes.

**Scope** (~150 LOC):
| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `image` crate dependency |
| `src-tauri/src/commands/metadata.rs` | Add `optimize_cover_art()` function, call from `load_cover_art_file` |
| `src-tauri/src/metadata/reader.rs` | Call `optimize_cover_art()` for auto-loaded cover art |
| `src-tauri/src/audio/settings_encoder.rs` | Add 104, 120 to valid bitrate list |
| `src/types/audio.ts` | Update `VALID_ENCODER_BITRATES` constant |

**Testing**: Existing UI continues working; new bitrates available; cover art auto-optimized on both load paths.

---

### PR 2: UI Replacement
**Branch**: `feature/new-ui` (depends on PR 1 merged)

**Executive Summary**: Replaces existing UI with new compact design from mock. Integrates Tailwind CSS via npm for offline-capable desktop builds. Adds tag preview grid, preview duration selector, and advanced encoder settings panel. All existing functionality preserved.

**Why This PR Exists**: Complete UI overhaul to match new design specifications. Separated from backend changes to isolate UI concerns and enable focused review.

**Scope** (~650 LOC):
| File | Change |
|------|--------|
| `package.json` | Add Tailwind dependencies |
| `tailwind.config.js` | Create with theme matching mock |
| `postcss.config.js` | Create for build integration |
| `index.html` | Replace with new layout structure (use mock IDs) |
| `src/styles.css` | Merge mock CSS variables, update components |
| `src/ui/outputPanel.ts` | Update dropdowns, add tag preview logic, update `estimated-size` ID |
| `src/ui/statusPanel/logic.ts` | Add preview duration state, update `meta-series-part` ID |
| `src/ui/statusPanel/dom.ts` | Wire split button dropdown |
| `src/ui/fileList/actions.ts` | Update `meta-series-part` ID reference |
| `src/ui/encoderPanel/*` | Wire advanced settings (Codex: module is split, not single file) |
| `src/lib/mocks.ts` | Update for new behaviors |

**Testing**: Full manual testing checklist (see Phase 5 below).

---

## Implementation Phases

### Phase 1 (PR 1): Backend Foundation

#### 1.1 Add Cover Art Optimization
- Add `image` crate to `Cargo.toml`
- Create `optimize_cover_art(bytes: Vec<u8>) -> Result<Vec<u8>>`:
  - Check size > 500KB OR format == PNG
  - If triggered: resize to max 600×600, convert to JPEG 80% quality
  - Return optimized bytes (or original if no optimization needed)
- Call from `load_cover_art_file` command (optimization at load time)

#### 1.2 Update Bitrate Whitelist
- Add 104, 120 to `VALID_BITRATES` in `settings_encoder.rs`
- Update `VALID_ENCODER_BITRATES` in `src/types/audio.ts`

#### 1.3 Verify & Merge PR 1
- Run `scripts/quick-checks.sh`
- Test cover art optimization with PNG > 500KB
- Test new bitrate values accepted

---

### Phase 2 (PR 2): UI Replacement

#### 2.1 Tailwind npm Setup
- `npm install -D tailwindcss postcss autoprefixer`
- Create `tailwind.config.js` with mock's color palette
- Create `postcss.config.js`
- Add Tailwind directives to `src/styles.css`

#### 2.2 HTML Structure Replacement
- Replace `index.html` with mock structure
- Preserve all element IDs for TypeScript bindings
- Remove CDN script reference
- Remove keyboard shortcut hint

#### 2.3 CSS Migration
- Merge mock's CSS variables into `src/styles.css`
- Preserve dark mode support
- Update component styles for new 2-column layout

#### 2.4 TypeScript Module Updates
- **outputPanel.ts**: Update bitrate dropdown (48-128 step 8), sample rate options, change `output-estimated-size` → `estimated-size`
- **statusPanel/logic.ts**: Add `previewDuration` state, wire split button, change `meta-series-pos` → `meta-series-part`
- **fileList/actions.ts**: Update `meta-series-pos` → `meta-series-part` reference
- **outputPanel.ts** or new **tagPreview.ts**: Add tag preview grid logic with TSOA calculation
- **encoderPanel/***: Wire advanced settings (encoder type, profile, VBR disabled)

#### 2.5 Preserve Existing Functionality
- File list drag-to-reorder
- Cover art drag-drop and load/clear
- Metadata form population
- Processing workflow and cancel

#### 2.6 Update Mocks
- Update `src/lib/mocks.ts` for any changed behaviors

#### 2.7 Verify & Merge PR 2
- Full manual testing checklist
- Run `scripts/quick-checks.sh`

---

## Risk Mitigation

- **Preserve element IDs**: All TypeScript modules rely on specific IDs; changing them breaks bindings
- **Test incrementally**: Verify each phase before proceeding
- **Backend-first approach**: Ensures UI can use new features immediately
- **Keep existing module structure**: Minimize refactoring risk

---

## Estimated Scope

- **Backend changes**: ~150 LOC (cover art optimization in two paths + bitrate)
- **Build setup**: ~50 LOC (configs)
- **HTML/CSS**: ~400 LOC (mostly replacement, not new)
- **TypeScript**: ~200 LOC (preview duration, tag preview, ID updates, encoder panel wiring)
- **Total**: ~800 LOC changed/added

---

## Agent Feedback Incorporated

| Source | Issue | Resolution |
|--------|-------|------------|
| Composer | Element ID mismatches | Use mock IDs, update TypeScript references |
| Composer | TSOA calculation missing | Added explicit calculation logic |
| Composer | Preview split button details | Expanded in PR2 scope |
| Gemini | PNG transparency handling | Flatten to white background |
| Gemini | Cover art size concern | Updated to 800×800 @ 85% JPEG |
| Codex | Auto-loaded cover art bypasses optimization | Added optimization in `read_audio_metadata` |
| Codex | Encoder panel module structure wrong | Corrected to `src/ui/encoderPanel/*` |
