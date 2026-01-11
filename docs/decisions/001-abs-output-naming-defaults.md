# ADR-001: ABS Output Naming Defaults

**Status:** accepted  
**Date:** 2026-01-10  
**Issue:** #139

## Context

Audiobook Boss outputs M4B files to a user-specified directory. The folder/file naming structure affects compatibility with audiobook servers (Audiobookshelf, Plex) and user organization preferences.

Previously, the app used a flat structure with configurable filename patterns. Users requested ABS-compatible folder hierarchies as the default.

## Decision

1. **ABS-compatible structure ON by default**
   - Output: `Author/Series/Book ## - Title/Book ## - Title.m4b`
   - Non-series: `Author/Title/Title.m4b`

2. **Year OFF by default** (toggle available)
   - When enabled (series): `Author/Series/Book ## - YYYY - Title/...`
   - When enabled (no series): `Author/YYYY - Title/...`
   - Rationale: ABS parses year only when first or directly after series sequence

3. **Full title preserved** (no short-title extraction)
   - Sanitize only for filesystem safety (`:` → ` - `)
   - Subtitles and edition info retained
   - Rationale: Information loss outweighs path brevity

4. **Commas preserved in author field**
   - "Last, First" format maintained for ABS compatibility
   - Other fields sanitize `,` → ` - `

5. **Manual mode deferred** to #140
   - Toggles for author/series/year in flat output
   - Keeps #139 scope focused on ABS defaults

## Consequences

### Pros
- Drop-in compatibility with Audiobookshelf
- Sensible defaults reduce user configuration
- No information loss in filenames
- Preserves author name formatting for ABS

### Cons
- Longer paths than short-title approach would have created
- Year requires explicit opt-in (may surprise some users expecting it by default)

## Alternatives Considered

| Alternative | Why Not Chosen |
|-------------|----------------|
| Short-title extraction (strip after `:` or `,`) | Loses subtitle/edition info (e.g., "Special Edition", "Unabridged") |
| Year ON by default | ABS doesn't require it; user preferred off by default |
| Ship manual mode in #139 | Scope creep; deferred to #140 for focused implementation |
| Remove year toggle entirely | User wanted the option available even if off by default |
| Sanitize commas everywhere | Breaks ABS "Last, First" author format convention |
