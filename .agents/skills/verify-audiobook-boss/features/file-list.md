# File list

A user sees imported audio as a selectable, reorderable list, inspects the
highlighted row, and can sort or clear the list. List membership and
selection stay in this column.

## Sub-features

- `list-select`: click a row to highlight it
- `list-reorder`: move a row with ▲ / ▼ or the reorder grip
- `list-clear`: Clear removes every row
- `list-inspect`: Selected File Properties updates with the row

## How to get to it (user POV)

1. Import at least two scratch fixtures (`file-import`).
2. The list is `role="listbox"` `aria-label="Audio files"` inside
   `aria-label="File list"`.
3. Toolbar: `#file-count-display`, `#sort-toggle-btn` (when shown),
   `#restore-import-order-btn` (when shown), `#clear-files-btn` (when
   shown).
4. The inspector under the list is `aria-label="Selected File Properties"`
   (`data-testid="file-inspector-panel"`).

## Driving it with osascript AX

Preconditions:

- Doctor passed or this is `--dry-run`.
- At least two valid rows are in `aria-label="Audio files"`.
- `#file-order-lock` is not visible.

- Select a row:
  User: clicks the first audio row.
  Command: `verify-abb drive file-list` (AX click `role="option"` whose
  `aria-label` is the first fixture basename).
  Result: that option is `aria-selected="true"`. The inspector context
  text is no longer the empty-state line. Bitrate / Sample Rate /
  Channels / Codec show values, not a blank inspector.

- Reorder:
  User: clicks ▼ on the first row (`.move-down-btn`).
  Command: AX click the `move-down` button on that row.
  Result: the former first basename is now the second `role="option"`.
  `#file-count-display` is unchanged.

- Clear:
  User: clicks `Clear`.
  Command: AX click `#clear-files-btn`.
  Result: `#file-count-display` reads `0 files`. The listbox has no
  options. `Clear` is no longer shown.

`--dry-run` prints these steps and records that Tauri launch, the window,
and AX clicks were skipped.

## Gotchas

- Keyboard Select all / Escape clear-highlight exist but are listbox-
  scoped. Prefer row click plus `#clear-files-btn` for the first drive.
- Reorder and Clear disable while `#file-order-lock` is visible.
- `Merge files into one audiobook` (`#merge-mode-toggle`) changes job
  type. Leave it off unless `encode-output` needs merge.
- PDF chips are remote supplemental assets. They should not appear for
  synthesized local wav fixtures.
