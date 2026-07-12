# Output Folder and Filename Conventions

Current policy is owned by
`crates/abb-output-artifact-core/src/lib.rs`; the Tauri adapter is
`src-tauri/src/output_artifact/naming.rs`. Read those surfaces and their tests
before changing or describing naming behavior.

## Current ABB Shape

```text
<output>/<author>/<series-when-present>/<title-folder>/<title-file>.m4b
```

- ABS-compatible structure is the default.
- Author comes from metadata artist, with the product's current fallback.
- Series folder is omitted when series is empty.
- Title folder and filename use the core naming policy and filesystem-safe
  normalization.
- Author commas and commas in other components intentionally have different
  handling.
- Year appears only when explicitly enabled and a usable metadata date exists.
- Series sequence and display-title rules belong to the core policy, not to
  ad-hoc path construction in callers.

## Change Rule

Change naming in the core owner first, keep the adapter thin, and update focused
core tests for Windows-invalid characters, empty/fallback fields, series/no-
series branches, sequence display, and optional year behavior. When claiming
ABS, Plex, or Apple compatibility, add manual importer evidence if the changed
folder shape is observable by that importer.

External scanner examples are evidence, not an alternate naming algorithm. Do
not copy generic Plex setup guidance or pseudocode sanitizers into this
reference; they drift from the product owner and create a second policy.

Primary external reference when scanner behavior is the question:
[Audiobookshelf folder naming](https://www.audiobookshelf.org/docs/#book-title-folder-naming).
