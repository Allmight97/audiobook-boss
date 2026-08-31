# Metadata

A user edits book tags and cover art for the selected file, saves those
edits, or searches online databases and applies a result. The Metadata
Manager is the right-column editor.

## Sub-features

- `meta-edit`: type into a field such as Book Title
- `meta-save`: Save All Changes writes the draft
- `meta-lookup`: Find Metadata opens the online search dialog
- `meta-cover`: cover drop target loads or clears art

## How to get to it (user POV)

1. Import and select one scratch fixture (`file-import`, `file-list`).
2. The right column heading is `Metadata Manager`
   (`aria-label="Metadata Manager"`, `data-testid="metadata-manager"`).
3. Fields use ids `meta-title`, `meta-year`, `meta-author`,
   `meta-narrator`, `meta-series`, `meta-series-part`, `meta-subseries`,
   `meta-subseries-part`, `meta-genre`, `meta-description`.
4. Actions: `#metadata-lookup-btn` (`Find Metadata`),
   `#metadata-save-btn` (`Save All Changes`).
5. Cover art is `#cover-art-area` / `data-testid="cover-art-area"`.

## Driving it with osascript AX

Preconditions:

- Doctor passed or this is `--dry-run`.
- One valid file is selected so the form is single-file, not multi.
- `#metadata-form` does not have `data-multi-select="true"`.
- Online lookup talks to public catalog APIs (Audnexus / OpenLibrary).
  That is a production-boundary network call. Do not use Audible login.

- Edit title:
  User: types a title into Book Title.
  Command: `verify-abb drive metadata` focuses `#meta-title` and sets
  `ABB Verify Title`.
  Result: `#meta-title` shows that value and `data-dirty="true"`. Tags
  Preview (`aria-label="Basic metadata tags"`) title cell is no longer
  the empty em dash placeholder.

- Save:
  User: clicks `Save All Changes`.
  Command: AX click `#metadata-save-btn`.
  Result: `data-testid="metadata-status-message"` (`role="status"`)
  reports a save outcome. `#meta-title` is not dirty. The selected
  file still shows the new title after re-select.

- Lookup (optional, network):
  User: clicks `Find Metadata`.
  Command: AX click `#metadata-lookup-btn`.
  Result: `#metadata-lookup-modal` (`data-testid="metadata-lookup-modal"`)
  is open, `role="dialog"` labelled `Find Metadata Online`. `#metadata-lookup-title-query`
  is focused or visible. Close with `#metadata-lookup-close` if you will
  not apply a result.

`--dry-run` prints these steps and records that Tauri launch, AX typing,
save IPC, and any catalog search were skipped.

## Gotchas

- Multi-select shows Keep/Blank `<select>` widgets
  (`data-testid="meta-title-action"` and siblings). Drive single-file
  first.
- `Cmd+S` / `Ctrl+S` also saves. Prefer `#metadata-save-btn`.
- Lookup apply (`Use Metadata` on a result) stages metadata. It is not
  Audible acquire. Still skip lookup if you cannot accept a network call
  at that production boundary.
- Do not paste personal cover URLs that leak account material.
- `#cover-art-url-input` loads remote bytes through the Tauri cover
  loader. Prefer skip unless the proof needs cover.
