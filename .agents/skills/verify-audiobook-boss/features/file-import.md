# File import

A user adds local audio into Input and File Order so the list, inspector,
and downstream panels have files to work on. Import validates paths and
analyzes audio before rows appear.

## Sub-features

- `import-files`: click the drop-zone header to pick files
- `import-folder`: click Add Folder to pick a directory
- `import-drop`: drop files or folders onto the file list
- `import-error`: unsupported paths show `#file-import-error`

## How to get to it (user POV)

1. Launch the verification instance. The left column heading is
   `Input and File Order` (`aria-label="Input and File Order"`,
   `data-testid="input-workflow-panel"`).
2. The drop-zone header reads `Drop files or folders here, click to
   choose files, or use Add Folder` (`aria-label="Add audio files"`).
3. `Add Folder` sits above the list (`#add-folder-btn`).
4. After a successful import, `#file-count-display` is not `0 files` and
   `role="listbox"` `aria-label="Audio files"` has options.

## Driving it with osascript AX

Preconditions:

- Doctor passed (`live_drive: possible`) or this is `--dry-run`.
- Scratch fixture exists (`verify-abb scaffold`).
- Order is not locked (`#file-order-lock` / `data-testid="file-order-lock"`
  is not visible).
- Do not use `Import from Library`.

- Open the file picker:
  User: clicks `Add audio files`.
  Command: `verify-abb drive file-import` (AX click the button
  `aria-label="Add audio files"`).
  Result: a native Tauri file sheet is frontmost, not a DOM modal.

- Choose the scratch fixture:
  User: selects the synthesized `.wav` in the helper scratch directory.
  Command: the same `drive` step sends osascript to that sheet (never a
  personal library).
  Result: the sheet closes. `#file-count-display` reads `1 file` (or more
  if several fixtures). A `role="option"` appears whose `aria-label` is
  the fixture basename. `#file-import-error` is absent.

- Folder import (optional second pass):
  User: clicks `Add Folder`.
  Command: AX click `#add-folder-btn`, then drive the folder sheet at the
  scratch directory.
  Result: `#file-count-display` increases only for new supported files.
  Duplicates show `#file-import-error` with
  `No new files added. All analyzed files were already in the list.`

`--dry-run` prints these steps and records that Tauri launch, the window,
and the native sheet were skipped.

## Gotchas

- `Import from Library` (`#acquire-audiobooks-btn`) opens Acquire
  Audiobooks (`#remote-source-modal`). Skip it. Real Audible auth is
  forbidden.
- Native pickers are `plugin-dialog`, not `#remote-source-modal` or
  `#metadata-lookup-modal`.
- Import is blocked while processing (`Order locked while processing` on
  `#file-order-lock`).
- Cover-art drops on `#cover-art-area` are not file import.
- Opening files via Finder on the installed `.app` is out of scope.
