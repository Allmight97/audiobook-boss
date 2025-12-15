---
name: Align File Properties Section with Mock UI Design
overview: Update the Selected File Properties section in production to match the visual design and structure from the mock UI, improving visual hierarchy, information density, and user experience while maintaining all existing functionality.
todos:
  - id: add-inspector-css
    content: Add inspector footer CSS classes to src/styles.css (inspector-footer, inspector-header, inspector-context, context variants)
    status: pending
  - id: update-html-structure
    content: Update index.html Selected File Properties section to match mock structure (add inspector-footer class, restructure header, remove title/helper text)
    status: pending
  - id: refactor-context-function
    content: Refactor updatePropertiesContext() in src/ui/fileList/actions.ts to build structured DOM with badges and truncation
    status: pending
  - id: update-clear-function
    content: Update clearFileProperties() to use structured context display
    status: pending
  - id: update-property-format
    content: Update property value formatting to match mock (remove inline units, use --- instead of N/A)
    status: pending
---

# Align File Properties Section with Mock UI Design

## Analysis: Mock vs Production Differences

### Visual Design Gaps

**Mock UI Design (Target State):**

- **Inspector Footer**: Uses `inspector-footer` class with explicit `border-top` for visual separation
- **Header Structure**: Dedicated `inspector-header` div with `border-bottom`, creating clear visual hierarchy
- **Context Display**: Structured with:
  - Filename in bold (`context-filename`) with truncation (max-width: 180px)
  - Position badge (`context-position`) styled as a pill/badge with background and border
  - Empty state (`context-empty`) with italic styling
  - Multi-select state (`context-multiselect`) with accent color
- **Content**: Removes "Selected File Properties" title and helper text for cleaner, more focused UI
- **Spacing**: Better visual separation with `margin-bottom: 0.5rem` and `padding-bottom: 0.375rem` on header

**Production (Current State):**

- Uses `file-properties-pinned` class (functional but lacks visual polish)
- Simple flex header with title and plain text context span
- Context is plain text: `"filename.mp3 (1 of 5)"`
- Includes title "Selected File Properties" and helper text
- Less visual hierarchy and separation

### Engineering Considerations

**Design Principles:**

1. **Visual Hierarchy**: Mock uses borders and spacing to create clear information layers
2. **Information Density**: Mock removes redundant text, letting visual structure communicate purpose
3. **Progressive Disclosure**: Badge styling makes position information scannable
4. **Consistency**: Inspector pattern aligns with common UI patterns (VS Code, Xcode, etc.)

**Implementation Strategy:**

- **Incremental Enhancement**: Add new classes alongside existing ones for backward compatibility
- **Semantic HTML**: Maintain accessibility with proper structure
- **TypeScript Updates**: Refactor `updatePropertiesContext()` to build structured DOM
- **CSS Consolidation**: Add inspector styles without breaking existing functionality

## Implementation Plan

### Task 1: Add Inspector Footer CSS Classes

**File**: `src/styles.css`

**Changes**: Add inspector footer styles after `.file-properties-pinned` rule

```css
/* Inspector Footer Styles - Enhanced visual hierarchy */
.inspector-footer {
  background-color: var(--bg-panel);
  border-top: 1px solid var(--border-primary);
}

.inspector-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
  padding-bottom: 0.375rem;
  border-bottom: 1px solid var(--border-primary);
}

.inspector-context {
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.context-empty {
  color: var(--text-muted);
  font-style: italic;
  font-weight: normal;
}

.context-filename {
  color: var(--text-primary);
  font-weight: 600;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.context-position {
  color: var(--text-muted);
  font-weight: normal;
  font-size: 0.7rem;
  background: var(--bg-input);
  padding: 0.125rem 0.375rem;
  border-radius: 0.25rem;
  border: 1px solid var(--border-primary);
}

.context-multiselect {
  color: var(--accent-primary);
  font-weight: 500;
}
```

**Rationale**: These styles create the visual hierarchy and badge styling seen in the mock.

### Task 2: Update HTML Structure

**File**: `index.html`

**Current Structure** (lines 89-108):

```html
<div class="section-divider file-properties-pinned">
  <div class="flex items-center justify-between mb-1">
    <h3 class="text-sm font-medium section-subtitle">
      Selected File Properties
    </h3>
    <span id="prop-selected-context" class="text-xs muted-text italic">No file selected</span>
  </div>
  <div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
    <!-- properties -->
  </div>
  <p class="text-xs muted-text italic mt-1">
    (Properties shown for single selected file.)
  </p>
</div>
```

**New Structure**:

```html
<div class="section-divider file-properties-pinned inspector-footer">
  <!-- Context Anchor Header - prevents "what am I looking at?" confusion -->
  <div class="inspector-header">
    <span class="inspector-context" id="prop-selected-context">
      <!-- Dynamic: shows filename + position OR empty state -->
      <span class="context-empty">No file selected</span>
    </span>
  </div>

  <div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
    <span class="property-label">Bitrate:</span><span class="property-value" id="prop-bitrate">--- kb/s</span>
    <span class="property-label">Sample Rate:</span><span class="property-value" id="prop-samplerate">--- Hz</span>
    <span class="property-label">Channels:</span><span class="property-value" id="prop-channels">---</span>
    <span class="property-label">File Size:</span><span class="property-value" id="prop-filesize">--- MB</span>
    <span class="property-label">Combined Size:</span><span class="property-value" id="prop-combinedsize">--- MB</span>
  </div>
</div>
```

**Key Changes**:

- Add `inspector-footer` class alongside `file-properties-pinned`
- Remove title "Selected File Properties" (visual structure communicates purpose)
- Remove helper text "(Properties shown for single selected file.)"
- Wrap context in `inspector-header` div for visual separation
- Change context container to `inspector-context` class
- Use nested span structure for context display

### Task 3: Refactor TypeScript Context Update Function

**File**: `src/ui/fileList/actions.ts`

**Current Function** (lines 129-141):

```typescript
function updatePropertiesContext(file: AudioFile, index: number): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl) return;

  if (!currentFileList || index < 0 || index >= currentFileList.files.length) {
    contextEl.textContent = "No file selected";
    return;
  }

  const fileName = file.path.split(/[\\\/]/).pop() || file.path;
  const totalFiles = currentFileList.files.length;
  contextEl.textContent = `${fileName} (${index + 1} of ${totalFiles})`;
}
```

**New Function**:

```typescript
function updatePropertiesContext(file: AudioFile, index: number): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl) return;

  // Clear existing content
  contextEl.innerHTML = "";

  if (!currentFileList || index < 0 || index >= currentFileList.files.length) {
    // Empty state
    const emptySpan = document.createElement("span");
    emptySpan.className = "context-empty";
    emptySpan.textContent = "No file selected";
    contextEl.appendChild(emptySpan);
    return;
  }

  // File selected state - build structured display
  const fileName = file.path.split(/[\\\/]/).pop() || file.path;
  const totalFiles = currentFileList.files.length;

  // Filename span with truncation
  const filenameSpan = document.createElement("span");
  filenameSpan.className = "context-filename";
  filenameSpan.title = fileName; // Full name on hover
  filenameSpan.textContent = fileName;

  // Position badge
  const posSpan = document.createElement("span");
  posSpan.className = "context-position";
  posSpan.textContent = `${index + 1} of ${totalFiles}`;

  contextEl.appendChild(filenameSpan);
  contextEl.appendChild(posSpan);
}
```

**Also Update**: `clearFileProperties()` to use the same structure:

```typescript
export function clearFileProperties(): void {
  // ... existing property clearing ...

  const contextEl = document.getElementById("prop-selected-context");
  if (contextEl) {
    contextEl.innerHTML = "";
    const emptySpan = document.createElement("span");
    emptySpan.className = "context-empty";
    emptySpan.textContent = "No file selected";
    contextEl.appendChild(emptySpan);
  }
}
```

**Rationale**: Builds structured DOM matching mock UI, enabling badge styling and truncation.

### Task 4: Update Property Value Display

**File**: `src/ui/fileList/actions.ts`

**Change**: Remove " kb/s" suffix from bitrate (mock shows just the number)

**Current** (line 105):

```typescript
bitrateEl.textContent = file.bitrate ? `${file.bitrate} kb/s` : "N/A";
```

**New**:

```typescript
bitrateEl.textContent = file.bitrate ? `${file.bitrate}` : "---";
```

**Also**: Ensure all property values match mock format (no units in values, use "---" instead of "N/A")

**Rationale**: Mock shows cleaner property values without inline units (units are in labels).

## Design Rationale

### Why These Changes Matter

**1. Visual Hierarchy (1st-order UX)**

- Border separation creates clear information layers
- Badge styling makes position scannable at a glance
- Removed redundant text reduces cognitive load

**2. Information Density (2nd-order UX)**

- Truncated filename with hover tooltip balances space vs. information
- Badge format "(1 of 5)" is more scannable than inline text
- Removed title/helper text lets visual structure communicate purpose

**3. Consistency (3rd-order DX)**

- Inspector pattern aligns with industry standards (VS Code, Xcode, Chrome DevTools)
- Structured classes enable future enhancements (multi-select, scroll-to-file button)
- Semantic HTML maintains accessibility

### Trade-offs Considered

**Removed Title**:

- **Pro**: Cleaner, more focused UI; visual structure communicates purpose
- **Con**: Less explicit labeling (mitigated by context header)
- **Decision**: Remove title (mock design choice)

**Structured Context Display**:

- **Pro**: Better visual hierarchy, enables badges and truncation
- **Con**: More complex DOM manipulation
- **Decision**: Implement structured display (better UX)

**Property Value Format**:

- **Pro**: Cleaner values, units in labels
- **Con**: Requires checking mock for exact format
- **Decision**: Match mock format (units in labels)

## Files to Modify

1. **`src/styles.css`**

   - Add inspector footer CSS classes (after `.file-properties-pinned`)

2. **`index.html`**

   - Update Selected File Properties structure (lines 89-108)
   - Add `inspector-footer` class
   - Restructure header and context display
   - Remove title and helper text

3. **`src/ui/fileList/actions.ts`**

   - Refactor `updatePropertiesContext()` to build structured DOM
   - Update `clearFileProperties()` to use structured display
   - Update property value formatting (remove inline units)

## Testing Checklist

1. **Visual Verification**

   - Verify border-top on inspector footer
   - Verify border-bottom on inspector header
   - Verify badge styling on position indicator
   - Verify filename truncation (test with long filenames)
   - Verify hover tooltip shows full filename

2. **Functionality**

   - Verify context updates when file selected
   - Verify context shows "No file selected" when cleared
   - Verify property values display correctly
   - Verify all existing functionality preserved

3. **Accessibility**

   - Verify keyboard navigation still works
   - Verify screen reader compatibility
   - Verify color contrast meets WCAG standards

4. **Edge Cases**

   - Test with very long filenames (>180px)
   - Test with single file (should show "1 of 1")
   - Test with no files loaded
   - Test property updates when selection changes

## Risk Assessment

**Low Risk Changes**:

- CSS additions: No breaking changes, additive only
- HTML structure: Maintains same IDs, backward compatible
- TypeScript refactor: Same functionality, improved structure

**Potential Issues**:

- Property value format change might need verification against mock
- Truncation width (180px) may need adjustment for different screen sizes
- DOM manipulation complexity increases (mitigated by clear function structure)

**Mitigation**:

- Test with various filename lengths
- Verify property value format matches mock exactly
- Ensure all existing tests still pass