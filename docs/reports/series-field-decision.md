# Decision: Series Field ID - `meta-series-part` vs `meta-series-pos`

**Date**: 2025-01-27  
**Decision**: Use `meta-series-part`  
**Status**: ✅ Approved

## Decision

**Use `meta-series-part` instead of `meta-series-pos`**

## Rationale

### 1. Current Field Not Used
- `meta-series-pos` exists in HTML but is **not referenced anywhere** in TypeScript code
- No breaking changes required
- Clean slate for implementation

### 2. Semantic Clarity
- **`part`**: Clearly means "book number in series" (Book 1, Book 2, etc.)
- **`pos`**: Generic "position" - could mean anything
- Mock label "Book #" aligns perfectly with "part"

### 3. Tag Preview Alignment
- Mock's tag preview expects `data-field="part"` for MVIN (Movement Number) tag
- Using `meta-series-part` creates consistent naming:
  - HTML: `meta-series-part`
  - Tag preview: `data-field="part"`
  - Future backend: `part` field

### 4. Audiobook Terminology
- "Book #" is standard audiobook/library terminology
- Aligns with Plex/Audiobookshelf conventions
- More intuitive for users

### 5. Future-Proofing
- Backend doesn't currently write MVIN tag, but when it does:
  - `part` is clearer than `pos`
  - Matches ID3/MP4 tag naming conventions

## Implementation Impact

### Files to Update (if switching from `meta-series-pos`)

**None currently** - field isn't used in code yet!

### Files to Create/Update (for new UI)

1. **`index.html`**: Use `meta-series-part` (already in mock)
2. **Tag preview module**: Use `data-field="part"` (already in mock)
3. **Metadata mapping**: Map `meta-series-part` → `part` field
4. **File list population**: Populate `meta-series-part` from metadata (if available)

## Consistency Check

Other field naming patterns:
- `meta-title` ✅ (noun)
- `meta-author` ✅ (noun)
- `meta-narrator` ✅ (noun)
- `meta-series` ✅ (noun)
- `meta-series-part` ✅ (noun + noun modifier) - **consistent**
- `meta-year` ✅ (noun)
- `meta-genre` ✅ (noun)

**Pattern**: All fields use noun-based naming. `part` fits this pattern better than `pos`.

## Alternative Considered

**`meta-series-pos`**:
- ❌ Less semantically clear
- ❌ Doesn't align with tag preview
- ❌ More generic term
- ✅ Currently in HTML (but unused)

## Conclusion

**`meta-series-part`** is the better choice because:
1. More semantically clear
2. Aligns with mock design
3. No existing code to break
4. Better user terminology
5. Future-proof for backend MVIN support

**Action**: Update plan to use `meta-series-part` throughout.

