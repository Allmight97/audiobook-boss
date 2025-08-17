## Audiobook Boss — Codebase Audit (2025-08-17)

This report summarizes duplicate code, consolidation opportunities, long/complex functions, oversized modules, deep nesting, code smells/dead code, and inline tests to relocate. Each finding includes a suggested resolution, risks, impact, and estimated effort. The audit covers Rust (src-tauri) and TypeScript (src) sources.

### Key
- Impact: Low / Medium / High
- Effort: Low / Medium / High
- Risk: Low / Medium / High (risk of regression/breaking changes)

---

### 1) Duplicate methods and near-duplicates

| Area                        | Files/Functions                                                                                                                      | Type                                   | Suggestion                                                                                                                                                | Impact                                                                     | Risk                       | Effort |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------------------------- | ------ |
| TS UI list movement         | `src/ui/fileList/actions.ts`: `moveFileUp`, `moveFileDown`                                                                           | Near-duplicate (index swaps)           | Unify into `moveFile(index: number, direction: 'up'                                                                                                       | 'down')` to reduce duplication; wire to `window.testCommands.testMoveFile` | Low                        | Low    | Low |
| TS cover art MIME detection | `src/ui/statusPanel/logic.ts`: `convertBytesToDataUrl` vs `src/ui/coverArt.ts`: inline MIME detection in `displayCoverArt`           | Duplicate logic                        | Extract shared `bytesToDataUrl(bytes: number[]): string` util (e.g., `src/ui/utils/image.ts`) and reuse                                                   | Low                                                                        | Low                        | Low    |
| Rust image format detection | `src-tauri/src/metadata/ffmpeg_bridge.rs`: `detect_cover_art_format` vs `src-tauri/src/metadata/writer.rs`: `detect_image_mime_type` | Overlapping detection                  | Introduce shared detector (e.g., `ImageFormat { Jpeg, Png, Gif, Webp, Unknown }`) with conversion adapters to `CoverFormat` and `MimeType` to avoid drift | Medium                                                                     | Medium (format heuristics) | Medium |
| Rust frame sizing           | `src-tauri/src/audio/buffer.rs`: `SampleAccumulator` vs `src-tauri/src/audio/frame_accumulator.rs`: `FrameAccumulator`               | Duplicate concepts; one appears unused | Standardize on one accumulator (keep `SampleAccumulator` which is used) and remove or deprecate `FrameAccumulator`                                        | Low                                                                        | Low                        | Low    |

Secondary risks (if consolidated): shared utility incorrectness, subtle differences in edge cases (e.g., tiny/placeholder images) — cover with unit tests for all formats/sizes.

---

### 2) Unification candidates (methods differing only by constants/params)

| Area                   | Files/Functions                                                         | Opportunity            | Suggestion                                                                                                                             | Impact | Risk | Effort |
| ---------------------- | ----------------------------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------ | ---- | ------ |
| TS/Progress            | `StatusPanel.startProcessing` includes validation, event hookup, invoke | Split responsibilities | Extract `validateInputs()`, `prepareProgressListener()`, `invokeProcessing()` helpers; keeps public method small                       | Low    | Low  | Low    |
| Rust progress emitters | `src-tauri/src/audio/progress/reporter.rs`: multiple `emit_*` wrappers  | Parameterize           | Optional: keep wrappers for readability; or add a single `emit_stage(stage, pct, msg, file, eta)` public API and make wrappers private | Low    | Low  | Low    |

---

### 3) Functions exceeding 55 LOC

| File                                        | Function                          | ~LOC | Why large                                    | Recommendation                                                                                               | Impact | Risk   | Effort |
| ------------------------------------------- | --------------------------------- | ---: | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ------ | ------ | ------ |
| `src-tauri/src/audio/media_pipeline.rs`     | `setup_encoder`                   | ~90+ | Muxer/encoder setup + metadata + cover art   | Extract helpers: `add_audio_stream`, `maybe_add_cover_art_pre_header`, `write_header_and_maybe_cover_packet` | Medium | Medium | Medium |
| `src-tauri/src/audio/media_pipeline.rs`     | `create_audio_encoder`            |  ~60 | Encoder config + feature toggles             | Extract FFI option tweaks to `configure_encoder_extras`                                                      | Low    | Low    | Low    |
| `src-tauri/src/audio/media_pipeline.rs`     | `setup_decoder_and_resampler`     | ~60+ | IO + stream selection + resampler            | Extract `open_input_and_best_stream`, `create_resampler_for_stream`                                          | Medium | Medium | Medium |
| `src-tauri/src/audio/media_pipeline.rs`     | `process_decoded_frames`          | ~60+ | Loop, resample, accumulate, encode, progress | Extract `resample_frame`, `accumulate_and_encode`, throttle progress emit                                    | Medium | Medium | Medium |
| `src-tauri/src/audio/media_pipeline.rs`     | `flush_decoder_frames`            | ~90+ | Complex flush logic                          | Consider simplified flush or reuse accumulator flush; extract inner loops to helpers                         | Medium | Medium | Medium |
| `src-tauri/src/audio/processor/finalize.rs` | `write_metadata_stage`            | ~60+ | Metadata writing + cover fallback check      | Extract `native_cover_present()` and `write_cover_art_fallback()`                                            | Low    | Low    | Low    |
| `src-tauri/src/metadata/ffmpeg_bridge.rs`   | `validate_metadata_compatibility` | ~90+ | Multi-branch validations                     | Split into `validate_cover_size`, `validate_cover_format`, `validate_dimensions`, `validate_codec_available` | Low    | Low    | Low    |
| `src/ui/statusPanel/logic.ts`               | `startProcessing`                 | ~70+ | Validation + UI + events + invoke            | Extract helpers listed above                                                                                 | Low    | Low    | Low    |

Note: No immediate functional issues, but splitting will improve readability, test seams, and reduce nesting.

---

### 4) Functions with >7 parameters

- Rust and TS: None detected (contexts and structs used effectively). No action required.

---

### 5) Modules >400 LOC

| File                                       | ~Lines | Notes                              | Recommendation                                                              | Impact | Risk   | Effort |
| ------------------------------------------ | -----: | ---------------------------------- | --------------------------------------------------------------------------- | ------ | ------ | ------ |
| `src-tauri/src/audio/media_pipeline.rs`    |   ~890 | Core engine implementation         | Consider submodules: `encoder.rs`, `decoder.rs`, `cover_art.rs`, `flush.rs` | Medium | Medium | High   |
| `src-tauri/src/metadata/ffmpeg_bridge.rs`  |   ~550 | Metadata embedding + image parsing | Extract `image_detect.rs`, `cover_embed.rs`, `container_meta.rs`            | Medium | Low    | Medium |
| `src-tauri/src/audio/progress/reporter.rs` |  ~400+ | Event/types/helpers + tests        | Move tests out; keep emitter/reporter focused                               | Low    | Low    | Low    |
| `src-tauri/src/audio/file_list.rs`         |  ~400+ | Validation + Lofty probes + tests  | Move tests out; consider `format_detect.rs` helper                          | Low    | Low    | Medium |

---

### 6) Nesting depth >4 (hotspots)

| File                                    | Function                 | Hot path                                             | Suggestion                                                      | Impact | Risk   | Effort |
| --------------------------------------- | ------------------------ | ---------------------------------------------------- | --------------------------------------------------------------- | ------ | ------ | ------ |
| `src-tauri/src/audio/media_pipeline.rs` | `flush_decoder_frames`   | while → match → if → while → if/else → for → if/else | Split copy/packetization into helpers; early-returns to flatten | Medium | Medium | Medium |
| `src-tauri/src/audio/media_pipeline.rs` | `process_decoded_frames` | loop → match → inner ops + for                       | Extract `resample_and_accumulate` helper; throttle progress     | Low    | Low    | Low    |

---

### 7) Code smells, dead code, unused

| Type                       | Location                                              | Details                                                     | Recommendation                                              | Impact | Risk | Effort |
| -------------------------- | ----------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------- | ------ | ---- | ------ |
| Dead code                  | `src-tauri/src/audio/frame_accumulator.rs`            | Unused type parallel to `SampleAccumulator`                 | Remove or mark deprecated; keep only `SampleAccumulator`    | Low    | Low  | Low    |
| Unused function            | `flush_decoder_frames` in `media_pipeline.rs`         | Defined but not used (explicitly skipped)                   | Remove or wire back with tests; if removing, document in PR | Low    | Low  | Low    |
| Possibly unused feature    | `parse_ffmpeg_progress` in `audio/progress/parser.rs` | Not referenced in engine path; used only in tests           | Consider keeping as fallback under a feature flag or remove | Low    | Low  | Low    |
| Log verbosity in hot loops | `media_pipeline.rs` packet/frame loops                | `log::debug/info!` per packet may be heavy                  | Gate debug logs behind `log::log_enabled!` or counters      | Low    | Low  | Low    |
| Duped success log          | `process_input_file`                                  | Two successive "✓ Decoder frames flushed successfully" logs | Remove duplicate log line                                   | None   | Low  | Low    |
| Dev-only globals           | `src/main.ts`: `window.testCommands`                  | Helpful in dev, noisy in prod                               | Consider gating by `NODE_ENV !== 'production'`              | Low    | Low  | Low    |

---

### 8) Inline tests to move to `src-tauri/tests/...`

Inline tests are great for private internals. For public APIs, prefer external test files per repo standards.

| File (inline tests)                       | Public surface?                           | External tests exist?                              | Action                                                            | Impact | Risk | Effort |
| ----------------------------------------- | ----------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------- | ------ | ---- | ------ |
| `src-tauri/src/metadata/writer.rs`        | Yes (`write_metadata`, `write_cover_art`) | Yes (`tests/unit/metadata/writer_tests.rs`)        | Remove/relocate inline tests; keep only tests for private helpers | Low    | Low  | Low    |
| `src-tauri/src/metadata/reader.rs`        | Yes (`read_metadata`)                     | Yes (`tests/unit/metadata/reader_tests.rs`)        | Same as above                                                     | Low    | Low  | Low    |
| `src-tauri/src/metadata/ffmpeg_bridge.rs` | Yes (public funcs)                        | Yes (`tests/unit/metadata/ffmpeg_bridge_tests.rs`) | Same                                                              | Low    | Low  | Low    |
| `src-tauri/src/audio/file_list.rs`        | Yes (`get_file_list_info`)                | Yes (multiple unit tests)                          | Same                                                              | Low    | Low  | Low    |
| `src-tauri/src/audio/path_validation.rs`  | Yes (`validate_input_audio_path`)         | Yes (`tests/unit/audio/path_validation_tests.rs`)  | Same                                                              | Low    | Low  | Low    |
| `src-tauri/src/audio/settings.rs`         | Mixed (many private validators)           | Partial (integration tests)                        | Keep inline tests for private functions                           | N/A    | N/A  | N/A    |
| `src-tauri/src/audio/progress/*.rs`       | Mixed                                     | Partial                                            | Keep inline for private math/helpers; move public behavior tests  | Low    | Low  | Low    |

Potential risk: adjusting import paths and slightly different behavior under `#[cfg(test)]`. Keep test coverage equivalent when moving.

---

### Secondary audit: dependency/regression risks for suggested edits

- Consolidating image detection (Rust): ensure identical behavior across JPEG/PNG/GIF/WebP and tiny placeholder images; update unit tests in `tests/unit/metadata/*` accordingly. Risk: Medium.
- Splitting long functions in engine: validate audio equivalence by running existing integration tests (`ffmpegnext_integration.rs`, `p41_core_pipeline_tests.rs`). Risk: Medium.
- Removing dead code (`FrameAccumulator`, `flush_decoder_frames`): if any downstream plans relied on them, note in PR. Risk: Low.
- TS refactors (movement unification, shared image util): manual UI testing plus quick smoke run `npm run tauri dev`. Risk: Low.

---

### Positive findings

- Robust error handling via `AppError` with no `unwrap()`/`expect()` in production paths; tests only use unwraps.
- Strong path validation (`validate_input_audio_path`) with canonicalization and symlink handling; comprehensive tests.
- Clean architectural split: `prepare` → `execute` → `finalize`, with context types reducing parameter count.
- Progress system centralized with clear TS event contracts in `src/types/events.ts` and Rust `ProgressEmitter`/`ProgressReporter`.
- TS code uses typed interfaces and modular UI structure (`fileList`, `statusPanel`, `outputPanel`) with DOM caching.
- Single-engine architecture (`FfmpegNextProcessor`) improves maintainability; feature flags removed.

---

### Recommended next steps (P0/P1/P2)

- P0: Remove dead code (`frame_accumulator.rs`), relocate duplicate inline tests to `src-tauri/tests/`, unify TS `moveFile` and image util.
- P1: Split long functions in `media_pipeline.rs` and `ffmpeg_bridge.rs` into focused helpers/submodules; reduce nesting.
- P2: Consider gating dev-only `window.testCommands` and throttling high-volume logs.
