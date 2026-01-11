# Issue #139 — ABS Output Naming Plan

## Goals

- Default output naming aligns with Audiobookshelf (ABS) parsing rules.
- Omit year from folder/filename unless explicitly enabled.
- Preserve full title; sanitize only for filesystem safety.
- Maintain reliable metadata tags for ABS/Plex.
- Provide clear, minimal UX controls for naming.

## Defaults (ABS-Compatible)

```
Author/
  Series/
    Book # - Title/
      Book # - Title.m4b
```

- `ABS-compatible structure` toggle ON by default.
- `Include year` toggle OFF by default.
- Year only appears if metadata date is present.

## UX Plan

- Rename toggle to `ABS-compatible structure` with tooltip.
- Show warning if `Series` is set and `Book #` is empty.
  - Warning is non-blocking.
  - User can proceed or fill in the series number and retry.
- Preview path updates live via existing output preview panel.

## Technical Plan

### Prep
- Refactor `src/ui/outputPanel.ts` into submodules.

### Rust
- Replace `FilenamePattern` with `OutputNamingConfig`.
- Update `build_output_path()` to use full title and ABS formatting.
- Sanitization: `:` and `,` → ` - ` for title/series; invalid path chars → space; preserve commas in author.

### TypeScript
- Add `OutputNamingConfig` to types.
- Update `ProcessV2Payload` to send `outputNaming`.
- Update output panel state, preview builder, and event handlers.

### HTML/CSS
- ABS toggle + include-year option.
- Add warning text styling for missing `Book #`.

### Tests
- Update output path unit tests for new naming config.
- Add tests for full-title preservation and author comma handling.

## Deferred (Issue #140)

- Manual naming options (author/year/series toggles).
- Subtitle toggle
- Narrator toggle
- Custom filename template editor

## Notes

- ABS parsing rules are applied to the **title folder**, not filename.
- Embedded metadata remains the primary source of year/series for ABS/Plex.
