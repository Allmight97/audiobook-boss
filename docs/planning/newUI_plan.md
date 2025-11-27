# UI Replacement Plan: Audiobook Boss

## Overview
Replace the existing UI with the new mock design (`docs/specs/UI_mock/new_UI_final.html`) while maintaining all existing functionality and addressing identified gaps.

---

## Critical Gaps Identified

### GAP 1: Cover Art Optimization (MISSING)
**Requirement**: If cover > 500KB OR format == PNG → Convert to JPG (80% quality) + Resize to max 600×600
**Current State**: No optimization logic exists. Cover art is loaded and written as-is.
**Impact**: Large cover art causes slow metadata writes; PNG format is less efficient for embedded art.

**Changes Required**:
- **Backend** (`src-tauri/src/commands/metadata.rs`): Add `optimize_cover_art()` function
  - Check file size (> 500KB threshold)
  - Check format (PNG detection via magic bytes)
  - Use `image` crate for resize/convert operations
  - Return optimized JPEG bytes
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
- Create `optimize_cover_art(bytes: Vec<u8>) -> Result<Vec<u8>>` in metadata module
- Logic: Check size/format → resize to 600×600 max → convert to JPEG 80%
- Call from `load_cover_art_file` and `write_cover_art` commands
- **Files**: `src-tauri/Cargo.toml`, `src-tauri/src/commands/metadata.rs`

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
- Remove keyboard shortcut handler
- **Files**: `src/ui/outputPanel.ts` or `src/ui/encoderPanel.ts`

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
| `src-tauri/src/audio/settings_encoder.rs` | Modify | Add bitrates 104, 120 |
| `src/types/audio.ts` | Modify | Update bitrate constant |
| `package.json` | Modify | Add Tailwind dependencies |
| `tailwind.config.js` | Create | Tailwind configuration |
| `postcss.config.js` | Create | PostCSS configuration |
| `index.html` | Replace | New layout structure |
| `src/styles.css` | Modify | Merge mock styles, CSS variables |
| `src/ui/outputPanel.ts` | Modify | Bitrate/sample rate dropdowns |
| `src/ui/statusPanel/logic.ts` | Modify | Preview duration state |
| `src/ui/statusPanel/dom.ts` | Modify | Split button DOM handling |
| `src/lib/mocks.ts` | Modify | Update for new behaviors |

---

## Decisions Confirmed

| Item | Decision |
|------|----------|
| Keyboard 'A' shortcut | Remove entirely |
| Bitrate options | 48-128k in 8-step increments (11 options) |
| Preview durations | Primary = 30s, Dropdown = 15s, 45s, 60s |
| Cover art optimization | At load time: 500KB/PNG→JPG 80% @ 600×600 max |
| Sample rate Auto | Pass-through (no resampling) |
| Tailwind | npm integration (offline support for packaged app) |
| Old UI | Replace directly (no archive) |
| PR Strategy | 2 PRs: Backend foundation first, then UI replacement |

---

## Execution Strategy: 2 PRs

### PR 1: Backend Foundation
**Branch**: `feature/backend-ui-prep`

**Executive Summary**: Prepares backend infrastructure for new UI capabilities. Adds cover art optimization (auto-resize/convert large images) and expands bitrate whitelist. These changes are backward-compatible with the existing UI and can be merged independently.

**Why This PR Exists**: The new UI requires backend support for cover art optimization and additional bitrate options (104k, 120k). Making these changes first ensures the UI replacement PR has a stable foundation without introducing backend changes alongside UI changes.

**Scope** (~100 LOC):
| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `image` crate dependency |
| `src-tauri/src/commands/metadata.rs` | Add `optimize_cover_art()` function, integrate at load time |
| `src-tauri/src/audio/settings_encoder.rs` | Add 104, 120 to valid bitrate list |
| `src/types/audio.ts` | Update `VALID_ENCODER_BITRATES` constant |

**Testing**: Existing UI continues working; new bitrates available in dropdown; cover art auto-optimized on load.

---

### PR 2: UI Replacement
**Branch**: `feature/new-ui` (depends on PR 1 merged)

**Executive Summary**: Replaces existing UI with new compact design from mock. Integrates Tailwind CSS via npm for offline-capable desktop builds. Adds tag preview grid, preview duration selector, and advanced encoder settings panel. All existing functionality preserved.

**Why This PR Exists**: Complete UI overhaul to match new design specifications. Separated from backend changes to isolate UI concerns and enable focused review.

**Scope** (~600 LOC):
| File | Change |
|------|--------|
| `package.json` | Add Tailwind dependencies |
| `tailwind.config.js` | Create with theme matching mock |
| `postcss.config.js` | Create for build integration |
| `index.html` | Replace with new layout structure |
| `src/styles.css` | Merge mock CSS variables, update components |
| `src/ui/outputPanel.ts` | Update dropdowns, add tag preview logic |
| `src/ui/statusPanel/logic.ts` | Add preview duration state |
| `src/ui/statusPanel/dom.ts` | Wire split button dropdown |
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
- **outputPanel.ts**: Update bitrate dropdown (48-128 step 8), sample rate options
- **statusPanel/logic.ts**: Add `previewDuration` state, wire split button
- **outputPanel.ts** or new **tagPreview.ts**: Add tag preview grid logic
- **encoderPanel.ts**: Wire advanced settings (encoder type, profile, VBR disabled)

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

- **Backend changes**: ~100 LOC (cover art optimization + bitrate)
- **Build setup**: ~50 LOC (configs)
- **HTML/CSS**: ~400 LOC (mostly replacement, not new)
- **TypeScript**: ~150 LOC (preview duration, tag preview, wiring)
- **Total**: ~700 LOC changed/added
