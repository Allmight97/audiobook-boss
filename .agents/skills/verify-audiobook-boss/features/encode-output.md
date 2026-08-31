# Encode and output

A user chooses how to encode, where the M4B should land, and starts
processing. Progress shows on the Status Panel and Work Center.

## Sub-features

- `encoder-select`: pick encoder and related settings
- `output-dir`: set a scratch output directory
- `output-naming`: ABS Default vs Custom Template
- `process-start`: Start Processing runs the job
- `process-status`: progress text, queue chips, Work Center rows

## How to get to it (user POV)

1. Import and select at least one valid scratch fixture. Metadata may be
   filled or left default.
2. The workbench is `aria-label="Encoding, output, and tags"`
   (`data-testid="encoding-workbench"`).
3. Encoder: `#encoder-settings-panel` / `data-testid="encoder-settings-panel"`.
   Controls: `#adv-encoder`, `#adv-bitrate-mode`, `#output-quality` or
   `#output-bitrate`, `#output-samplerate`, `#output-channels`.
4. Output: `data-testid="output-panel"`. Directory value
   `data-testid="output-directory-value"`. Browse is `#output-dir-browse`.
   Naming preset is `#output-naming-preset`.
5. Start is `#process-button` (`Start Processing`). Status text is
   `#status-text`. Work Center is `aria-label="Work Center"`.

## Driving it with osascript AX

Preconditions:

- Doctor passed or this is `--dry-run`.
- At least one valid imported file.
- Output directory is a scratch folder this run created, not a path from
  the user's `app-settings.json`.
- Encoder `#adv-encoder` is not stuck on `Loading…`.
- Do not enable FDK Afterburner or a user FFmpeg path.

- Set output directory:
  User: clicks `Browse…` under Output Directory.
  Command: `verify-abb drive encode-output` AX-clicks `#output-dir-browse`,
  then drive the native folder sheet at the scratch output directory.
  Result: `data-testid="output-directory-value"` (`#output-dir-text`)
  shows that scratch path. `data-testid="output-example"` previews a
  filename under it.

- Confirm encoder is selectable:
  User: looks at Encoder.
  Command: read AX value of `#adv-encoder` (`data-testid="encoder-select"`).
  Result: a real encoder option is selected (not `Loading…`).
  `#estimated-size` (`data-testid="estimated-size"`) shows a size or
  `---`.

- Start processing:
  User: clicks `Start Processing`.
  Command: AX click `#process-button`.
  Result: `#status-text` leaves `Ready to process audiobook` (or the idle
  line). Work Center is not `No background work.` A collision dialog
  (`#collision-dialog-modal`) may appear; Cancel (`#collision-dialog-close`)
  if the planned path is not the scratch directory. On success, a `.m4b`
  exists under the scratch output path and `.logs/tauri-dev.log` records
  the operation.

`--dry-run` prints these steps and records that Tauri launch, the output
sheet, Start Processing, and any encode were skipped.

## Gotchas

- `#process-button` submits a real encode. On `--dry-run` it must not
  run. On live drive, watch Work Center until a terminal status
  (Completed / Failed / Cancelled).
- Collision choices (`Overwrite Existing`, `Skip Existing`, `Rename`)
  write or skip real files. Use only against scratch output.
- `Preview Audio` (`#preview-button`) is a short encode. Do not use it as
  a silent substitute for Start Processing unless that is the feature
  under test.
- `#merge-mode-toggle` changes batch vs one merged book. Keep batch
  unless merge is the claim.
- Apple AAC (`aac_at`) is macOS-only. Linux cannot prove it and must not
  fake it.
- Never point output at the user's library or the installed app's last
  directory.
