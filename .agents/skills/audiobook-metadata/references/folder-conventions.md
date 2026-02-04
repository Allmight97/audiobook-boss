# Folder Structure Conventions

ABS and Plex folder naming patterns for automatic metadata parsing.

## Table of Contents

1. [Recommended Structure](#recommended-structure)
2. [ABS Title Folder Patterns](#abs-title-folder-patterns)
3. [ABS Author Folder Patterns](#abs-author-folder-patterns)
4. [Plex Considerations](#plex-considerations)
5. [Filename Sanitization](#filename-sanitization)
6. [audiobook-boss Implementation](#audiobook-boss-implementation)

---

## Recommended Structure

Universal structure that works for ABS, Plex, and general file organization:

```
/Audiobooks
  /Author Name
    /Series Name
      /Book Title
        book.m4b
        cover.jpg
```

For standalone books (no series):

```
/Audiobooks
  /Author Name
    /Book Title
      book.m4b
      cover.jpg
```

## ABS Title Folder Patterns

ABS parses metadata from folder names. Supported patterns:

| Pattern | Example |
|---------|---------|
| Title only | `Wizards First Rule` |
| With narrator | `Wizards First Rule {Sam Tsoutsouvas}` |
| Year prefix | `1994 - Wizards First Rule` |
| Sequence prefix | `Book 1 - Wizards First Rule` |
| Sequence + year | `Vol 1 - 1994 - Wizards First Rule` |
| With subtitle | `1994 - Wizards First Rule - A Subtitle` |

### Parsing Rules

- **Narrator**: Wrap in `{curly braces}`
- **Year**: 4 digits, separated by ` - ` (space-dash-space)
- **Sequence**: Prefix with `Book`, `Vol`, `Vol.`, or `Volume`
- **Subtitle**: Separated by ` - ` (requires ABS server setting)

### Sequence Prefix Examples

All valid:
```
Book 1 - Title
Book 1. Title
Vol 1 - Title
Vol. 1 - Title
Volume 1 - Title
```

## ABS Author Folder Patterns

Multiple formats supported:

| Format | Example |
|--------|---------|
| First Last | `Terry Goodkind` |
| Last, First | `Goodkind, Terry` |
| Multiple (comma) | `Terry Goodkind, Brandon Sanderson` |
| Multiple (ampersand) | `Terry Goodkind & Brandon Sanderson` |
| Multiple (and) | `Terry Goodkind and Brandon Sanderson` |

## Plex Considerations

Plex treats audiobooks as music albums in a Music library:

1. **Library type**: Music with "Store Track Progress" enabled
2. **Disable online matching**: Plex will misidentify audiobooks
3. **Preferred structure**: `Author/Book Title/audiobook.m4b`
4. **Series**: No native support—relies on folder structure or custom agents

For Plex compatibility, the Author folder is most important. Series organization is secondary.

## Filename Sanitization

Characters to remove or replace in folder/file names:

| Character | Replacement | Reason |
|-----------|-------------|--------|
| `:` | ` -` or remove | Invalid on Windows |
| `?` | remove | Invalid on Windows |
| `*` | remove | Invalid on Windows |
| `"` | `'` or remove | Invalid on Windows |
| `<` | remove | Invalid on Windows |
| `>` | remove | Invalid on Windows |
| `\|` | ` -` or remove | Invalid on Windows |
| `/` | ` -` or remove | Path separator |
| `\` | ` -` or remove | Path separator (Windows) |

### Sanitization Function (Pseudocode)

```
sanitize(name):
  replace ":" with " -"
  replace "," with " - " (except author)
  replace "/" "\" with " "
  remove "?" "*" "<" ">" "|"
  collapse multiple spaces to single space
  trim leading/trailing whitespace
  return name
```

## audiobook-boss Implementation

The output path generation is in `src-tauri/src/audio/output_path.rs`:

```rust
// build_output_path()
// Structure (ABS-compatible): output_dir / author / series (optional) / Book # - Title / Book # - Title.m4b
```

Current behavior:
- ABS-compatible structure is the default.
- Author folder: Uses `artist` from metadata (falls back to "Unknown Author").
- Series folder: Uses `series` from metadata (skipped if empty).
- Title folder + filename: Uses full `title` from metadata, sanitized for filesystem safety.
- Author folder preserves commas; other components replace commas with ` - `.
- Year appears only when explicitly enabled and metadata date is present.

To modify folder structure behavior, edit `build_output_path()` in `audio/output_path.rs`.

---

Sources:
- [ABS Docs - Title Folder Naming](https://www.audiobookshelf.org/docs/#book-title-folder-naming)
- [ABS Docs - Author Folder Naming](https://www.audiobookshelf.org/docs/#book-author-folder-naming)
- [Plex Audiobook Guide](https://github.com/seanap/Plex-Audiobook-Guide)
