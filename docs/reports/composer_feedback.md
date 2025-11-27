# UI Replacement Plan Audit: Composer Feedback

**Date**: 2025-01-27  
**Auditor**: Composer (AI Agent)  
**Plan Document**: `docs/planning/newUI_plan.md`  
**Mock Reference**: `docs/specs/UI_mock/new_UI_final.html`

## Executive Summary

The plan is generally well-structured with a clear 2-PR approach, but several critical gaps were identified that could cause implementation issues. The main concerns are around element ID mismatches, metadata field mapping inconsistencies, and missing implementation details for key features like the tag preview and preview duration dropdown.

---

## Critical Issues

### 1. Element ID Mismatches (CRITICAL)

**Issue**: The plan states "Preserve all existing element IDs (critical for TypeScript bindings)" but the mock uses different IDs than the current implementation.

**Conflicts Identified**:
- **Estimated size**: Mock uses `estimated-size`, current uses `output-estimated-size`
- **Series position**: Mock uses `meta-series-part`, current uses `meta-series-pos`
- **Metadata structure**: Mock uses 4-column grid layout, current uses 2-column grid

**Impact**: TypeScript modules (`outputPanel.ts`, `statusPanel/logic.ts`, `fileList/actions.ts`) reference these IDs directly. Changing IDs requires updating all references.

**Recommendation**: 
- **Option A**: Update mock to match current IDs (easier, less risk)
- **Option B**: Update all TypeScript references to match mock IDs (more work, but aligns with mock)
- **Decision needed**: Which approach should be taken?

**Files Affected**:
- `src/ui/outputPanel.ts` (line 286: `output-estimated-size`)
- `src/ui/statusPanel/logic.ts` (line 410-424: metadata field reads)
- `src/ui/fileList/actions.ts` (lines 127-128, 222-223: metadata population)

---

### 2. Metadata Field Mapping Mismatch (CRITICAL)

**Issue**: The mock's metadata structure differs significantly from current implementation, and the tag preview expects specific field mappings that aren't documented.

**Current HTML Structure**:
- `meta-album` (Album/ALBUM)
- `meta-series-pos` (Series Pos./TRACK)
- `meta-series-sort` (Series Sort/ALBUMSORT)

**Mock HTML Structure**:
- No separate `meta-album` field (title is used for both title and album)
- `meta-series-part` (Book #)
- Different field layout (4-column grid vs 2-column)

**Tag Preview Requirements**:
The mock's tag preview expects:
- `title` → Title (Book Title)
- `album` → Album (Book Title) - same as title
- `artist` → Artist (Author)
- `albumArtist` → Album Artist (Author) - same as artist
- `composer` → Composer (Narrator)
- `series` → MVNM (Series)
- `part` → MVIN (Book #)
- `tsoa` → TSOA (Title Sort Order) - calculated field
- `year` → TYER (Release Year)
- `genre` → TCON (Genre)

**Current Code Mapping**:
- `meta-author` → `artist` (in `statusPanel/logic.ts`)
- `meta-narrator` → `composer`
- `meta-series` → appended to `album` (temporary hack)

**Impact**: Tag preview logic won't work without proper field mapping. Current code has a temporary series hack that needs to be replaced.

**Recommendation**: Document explicit field mapping strategy:
- `meta-title` → both `title` AND `album` (same value)
- `meta-author` → both `artist` AND `albumArtist` (same value)
- `meta-narrator` → `composer`
- `meta-series` → `series` (MVNM tag)
- `meta-series-part` → `part` (MVIN tag)
- Calculate `tsoa` from series + part + title
- `meta-year` → `year` (TYER tag)
- `meta-genre` → `genre` (TCON tag)

**Files Affected**:
- `src/ui/outputPanel.ts` (getCurrentMetadata function)
- `src/ui/statusPanel/logic.ts` (getCurrentMetadata function)
- New tag preview module (to be created)

---

### 3. Tag Preview TSOA Calculation Missing Details

**Issue**: Plan mentions calculating TSOA but doesn't specify the exact logic.

**Mock JavaScript Logic**:
```javascript
function padPart(num) {
    const n = parseInt(num, 10);
    if (isNaN(n)) return '';
    return n < 10 ? `0${n}` : `${n}`;
}

// Calculate TSOA
let tsoa = '';
if (series && title) {
    const paddedPart = padPart(part) || '00';
    tsoa = `${series} ${paddedPart} - ${title}`;
}
```

**Recommendation**: Add to Phase 3.3:
- Implement `padPart()` helper function (pads to 2 digits: "01", "02", etc.)
- Calculate TSOA as: `{series} {paddedPart} - {title}` when both series and title exist
- Handle edge cases: empty series, empty title, missing part (default to "00")

---

### 4. Preview Duration Dropdown Missing Implementation Details

**Issue**: Plan mentions adding preview duration state but doesn't specify the DOM structure changes needed.

**Current Implementation**:
- Single button: `preview-button` (hardcoded to 30s)
- Handler in `statusPanel/logic.ts` line 81-83

**Mock Structure**:
- Split button: `preview-button` (main) + `preview-dropdown-toggle` (caret)
- Dropdown: `preview-dropdown` with options (15s, 30s, 60s)
- Dropdown options: `data-duration` attributes

**Missing Details**:
- Need to replace single button with split button structure
- Need dropdown toggle handler
- Need click-outside handler to close dropdown
- Need to update button text to show selected duration
- Need to pass selected duration to `startProcessing({ previewSeconds: selectedDuration })`

**Recommendation**: Expand Phase 3.2:
- Replace `<button id="preview-button">` with split button structure from mock
- Add `preview-dropdown-toggle` click handler
- Add click-outside handler (use event delegation)
- Update `statusPanel/logic.ts` to read duration from state, not hardcode 30
- Update button text dynamically: `Preview Audio (${duration}s)`

---

### 5. Advanced Settings Panel Integration Unclear

**Issue**: Plan mentions wiring encoder type/profile dropdowns but doesn't address existing `src/ui/encoderPanel/` module.

**Current State**:
- `src/ui/encoderPanel/` module exists with `initEncoderPanel()` function
- Called from `main.ts` line 99
- Has feature flags (`ENABLE_FDK`, `ENABLE_VBR`)

**Mock Structure**:
- Advanced settings panel with encoder type dropdown
- Profile dropdown (LC, HE v1, HE v2)
- VBR checkbox (disabled)

**Questions**:
- Is encoder panel separate from advanced settings panel?
- Should encoder panel be integrated into advanced settings?
- How does `initEncoderPanel()` relate to advanced settings toggle?

**Recommendation**: Clarify in Phase 3.4:
- Document whether encoder panel is separate or integrated
- If separate: document how they interact
- If integrated: note that `initEncoderPanel()` may need refactoring
- Ensure advanced panel toggle works with encoder panel logic

---

## Moderate Issues

### 6. Cover Art Optimization Scope Error

**Issue**: Plan says to call optimization from both `load_cover_art_file` and `write_cover_art`, but `write_cover_art` writes to an existing file and shouldn't optimize.

**Current Commands**:
- `load_cover_art_file`: Loads image from disk → returns bytes
- `write_cover_art`: Writes bytes to existing M4B file

**Correct Behavior**:
- Optimization should happen at **load time** only (`load_cover_art_file`)
- `write_cover_art` should write bytes as-is (already optimized)

**Recommendation**: Update Phase 1.1:
- Remove `write_cover_art` from optimization section
- Only optimize in `load_cover_art_file` command
- Clarify that optimization happens transparently when user loads cover art

---

### 7. Estimated Size Element ID Mismatch

**Issue**: Mock uses `estimated-size`, current uses `output-estimated-size`.

**Impact**: `updateEstimatedSize()` in `outputPanel.ts` line 286 references `output-estimated-size`.

**Recommendation**: 
- If keeping current IDs: Update mock to use `output-estimated-size`
- If switching to mock IDs: Update `outputPanel.ts` to use `estimated-size`
- Document decision in plan

---

### 8. Tailwind CDN Removal Missing Step

**Issue**: Plan says to remove CDN script but doesn't specify when/how.

**Current State**: `index.html` line 9 has:
```html
<script src="https://cdn.tailwindcss.com"></script>
```

**Recommendation**: Add explicit step to Phase 2.1:
- Remove CDN script tag from `index.html` after npm Tailwind is confirmed working
- Add verification step: "Verify npm Tailwind compiles before removing CDN"

---

### 9. Bitrate Dropdown Options Incomplete

**Issue**: Plan says "48-128 in steps of 8" but doesn't list all options.

**Current Dropdown**: 48, 56, 64, 72, 80, 88, 96, 112, 128 (missing 104, 120)  
**Mock Shows**: 64, 72, 80, 88, 96 (incomplete list in mock)

**Full List Should Be**: 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128 (11 options total)

**Recommendation**: Clarify in Phase 3.1:
- List all 11 bitrate options explicitly
- Note that backend validation already supports these (after PR 1)
- Mock dropdown should show all options (or at least indicate full range)

---

### 10. Metadata Field Population Missing Detail

**Issue**: Plan doesn't specify how metadata populates from selected files.

**Current Code**: `src/ui/fileList/actions.ts` populates:
- `meta-title`, `meta-author`, `meta-album`, `meta-narrator`, `meta-year`, `meta-genre`, `meta-series`, `meta-series-pos`

**Mock Uses**: `meta-series-part` instead of `meta-series-pos`

**Recommendation**: Add to Phase 3.5:
- Update file list population logic if switching to mock IDs
- Document which fields are populated from file metadata
- Ensure `meta-series-part` is populated if using mock structure

---

## Minor Issues / Suggestions

### 11. PostCSS Configuration Missing

**Issue**: Plan mentions creating `postcss.config.js` but doesn't specify contents.

**Recommendation**: Add example config:
```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

---

### 12. Tag Preview Update Trigger Missing

**Issue**: Plan says to wire tag preview to metadata input changes but doesn't specify which inputs.

**Mock JavaScript**: Binds to `#meta-title, #meta-author, #meta-narrator, #meta-series, #meta-series-part, #meta-year, #meta-genre`

**Recommendation**: Document exact input IDs to bind in Phase 3.3.

---

### 13. Advanced Settings Keyboard Shortcut

**Issue**: Plan says to remove keyboard shortcut, but mock still shows "Press A to toggle" hint (line 492).

**Recommendation**: Clarify:
- Remove keyboard shortcut handler (mock has it but plan says remove)
- Remove hint text from HTML
- Or: Keep shortcut but remove hint text?

---

## Summary of Critical Gaps

1. **Element ID Strategy**: Need decision on preserve vs. change IDs
2. **Metadata Field Mapping**: Need explicit mapping documentation
3. **Tag Preview TSOA**: Need calculation logic details
4. **Preview Button Split**: Need DOM structure change details
5. **Encoder Panel Integration**: Need clarification on existing module

---

## Recommendations for Plan Update

1. **Add Element ID Decision Section**: Document which IDs to use (current vs. mock)
2. **Expand Metadata Mapping**: Add explicit field mapping table
3. **Detail Tag Preview Logic**: Include TSOA calculation and `padPart()` helper
4. **Expand Preview Duration**: Add DOM structure changes and event handlers
5. **Clarify Encoder Panel**: Document integration approach with existing module
6. **Fix Cover Art Optimization**: Remove `write_cover_art` from optimization scope
7. **Add Verification Steps**: Include checks for Tailwind, element IDs, field mappings

---

## Files Requiring Updates (If Using Mock IDs)

If decision is made to use mock IDs instead of current IDs:

- `src/ui/outputPanel.ts`: Update `output-estimated-size` → `estimated-size`
- `src/ui/statusPanel/logic.ts`: Update metadata field reads
- `src/ui/fileList/actions.ts`: Update `meta-series-pos` → `meta-series-part`
- `index.html`: Update all element IDs to match mock

---

## Conclusion

The plan provides a solid foundation but needs clarification on element IDs, metadata mappings, and implementation details for tag preview and preview duration features. The 2-PR approach is sound, but PR 2 will need more detailed specifications to avoid breaking existing functionality.

**Priority Actions**:
1. Decide on element ID strategy (preserve vs. change)
2. Document metadata field mapping explicitly
3. Add tag preview TSOA calculation details
4. Expand preview duration dropdown implementation
5. Clarify encoder panel integration approach

