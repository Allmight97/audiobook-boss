### Refactor Plan (Phase 0-1)

Principles: small, behavior-preserving edits; keep modules <400 lines, functions <60 lines. Extract tests first, then split modules.

#### Test Extraction (Phase 0)
- Move inline `#[cfg(test)]` modules to `src-tauri/tests/**`:
  - `audio`: `file_list.rs`, `settings.rs`, `progress.rs`, `session.rs`, `cleanup.rs`
  - `ffmpeg`: `mod.rs`, `command.rs`
  - `metadata`: `reader.rs`, `writer.rs`
  - `commands/mod.rs` tests split into `unit/audio_commands.rs`, `unit/metadata_commands.rs`, `unit/basic_commands.rs`
  - `src-tauri/src/tests_integration.rs` → `src-tauri/tests/integration/*.rs`
- Replace `unwrap()/unwrap_err()` in tests with `expect()/expect_err()`; apply clippy format arg suggestions.

Directory structure:
```
src-tauri/tests/
  unit/
    audio/
    ffmpeg/
    metadata/
    commands/
  integration/
```

#### Module Splits (Phase 1)
- `audio/processor.rs` → `processor/{prepare.rs, execute.rs, finalize.rs, mod.rs}`
- `audio/progress.rs` → `progress/{reporter.rs, parser.rs, mod.rs}`
- `audio/cleanup.rs` → `cleanup/{guard.rs, ops.rs, mod.rs}`
- `commands/mod.rs` → `commands/{audio.rs, metadata.rs, system.rs, mod.rs}`
- UI TS:
  - `fileList.ts` → `ui/fileList/{state.ts, dom.ts, actions.ts}`
  - `statusPanel.ts` → `ui/status/{listener.ts, ui.ts, panel.ts}`
  - `outputPanel.ts` → `ui/output/{settingsForm.ts, errors.ts, panel.ts}`

#### FFmpeg Boundary (Phase 1.5)
- Define trait `MediaProcessor` with operations used by `processor`.
- Implement `ShellFFmpegProcessor` using current command-building flow.
- Add `safe-ffmpeg` feature flag with placeholder module.

#### Size Budget Automation
- Add `scripts/report/size_budget.sh` to report files >400 lines and flag functions >60 lines (best-effort via ripgrep).

#### Checklist (per edit)
- Tests: `cargo test`
- Lints: `cargo clippy --all-targets --all-features`
- Scope: confirm no behavior changes in Phase 0-1 edits


