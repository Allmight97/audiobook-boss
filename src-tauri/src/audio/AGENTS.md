# Audio Pipeline Directives

## Scope

- Applies to audio-domain code under `src-tauri/src/audio/`.
- Nested `AGENTS.md` files own narrower rules for `processor/`; processing
  lifecycle rules live under `src-tauri/src/processing/`.
- This file owns audio integrity rules that cross local import discovery, stream
  probing, decoder setup, resampling, sample buffering, encoder setup, muxing,
  and output validation.
- Audio is the **Audio Engine Deep Module** Grey-Box Public API owner. Its
  allowed import surface lives at `crate::audio`; processor internals stay
  private under `src-tauri/src/audio/processor/`.

## Public API Strip

- Import from `crate::audio`, not private child modules such as
  `crate::audio::processor`, `crate::audio::settings_encoder`,
  `crate::audio::toolchain`, `crate::audio::path_validation`, or
  `crate::audio::cleanup`.
- Types: `AudioFile`, `DecoderSelection`, `SampleRateConfig`, `FileListInfo`,
  `SupportedAudioImportFormat`, `SupportedAudioImportMetadata`,
  `AacDecoderAvailability`, `EncoderSettings`, `EncoderType`, `BitrateMode`,
  `ChannelConfig`, `EncoderAvailability`, `EncoderCapabilitySource`.
- Functions: `get_file_list_info`, `apply_chapter_plans`, `validate_input_audio_path`,
  `validate_input_image_path`, `supported_audio_import_metadata`,
  `discover_audio_import_paths`, `validate_output_path`,
  `validate_sample_rate_config`, `validate_encoder_settings`,
  `validate_requested_encoder_available`,
  `encoder_settings_capabilities`,
  `resolve_encoder_type`, `resolve_encoder_name`,
  `detect_encoder_availability`, `detect_aac_decoder_availability`,
  `preferred_aac_decoder_order_labels`, `execute_audio_engine`,
  `validate_audio_engine_inputs`, `set_user_external_ffmpeg_path` (settings
  hydration/update ingress for the durable user FFmpeg path; validation stays
  toolchain-owned).
- Execution request type: `AudioExecutionRequest`.
- Capability types: `EncoderBitrateModeCapability`, `EncoderSettingsCapabilities`,
  `BitrateModeKind`.
- Constants: `VALID_ENCODER_BITRATES`.
- Crate-internal helper: `CleanupGuard`.
- Audio does not own lifecycle event names or progress math. Use
  `crate::processing` / `processing::progress` for queue/progress event
  vocabulary and operation lifecycle identity.

## Private Cluster

- Files: `buffer.rs`, `cleanup/`, `extensions.rs`,
  `constants.rs`, `file_list.rs`, `imports.rs`, `imports_tests.rs`, `metrics.rs`,
  `path_validation.rs`, `processor/`, `settings.rs`, `settings_capabilities.rs`,
  `settings_encoder.rs`, and `toolchain/` (`mod.rs` = platform-neutral
  resolution/validation; `platform.rs` = the per-OS probe seam — candidate
  enumeration, binary-arch acceptance, and platform paths live ONLY there,
  cfg-dispatched per the `src-tauri/src/remote_source/vault.rs` pattern with pure rules
  unit-testable on any host).
- The cluster owns local audio import metadata/discovery, decoder/toolchain
  selection, media inspection, decode/resample/encode/mux internals, staging,
  cleanup, and media execution facts. Processing owns lifecycle orchestration
  and terminal normalization; `output_artifact` owns final artifact commit truth.

## Test Placement

- Public API Strip behavior belongs in contract/integration tests that import
  only `crate::audio`.
- Private-cluster invariants may use source-tree unit tests, including sibling
  `*_tests.rs` files declared from the owning module with `#[cfg(test)]` and
  `#[path = "..._tests.rs"]`. Prefer source-tree tests over a separate
  integration-test directory when the move would widen the Public API Strip
  or force test-only exports.
- Keep small tests inline when they clarify the nearby code; move bulky private
  test blocks to sibling test files when readability is the real issue.

## Path Display Policy

- Filesystem and process identity uses `Path`, `OsStr`, or `OsString`.
- Diagnostics and logs use sanitized display strings.
- Lossy strings are allowed only for display ordering, never identity or command argv.

## Edit Rules

- Change private implementation files when focused audio/processing tests stay
  green for the touched boundary.
- Narrow accidental visibility when callers can use the Public API Strip without
  losing contract truth.
- Keep Native AAC, Apple AAC/AAC-AT, and external FDK adapter differences inside
  the private cluster unless a caller needs a stable capability fact.

## Boundary Changes

- Adding, removing, or renaming any Public API Strip symbol.
- Moving job lifecycle ownership out of processing, final artifact commit truth
  out of `output_artifact`, or metadata policy out of metadata-owned APIs.
- Changing user-visible progress, cancellation, or terminal success/failure
  semantics.

## Preferred Path

- Treat audio processing as a boundary chain: discover supported local paths ->
  validate paths -> inspect inputs -> choose decoder/toolchain ->
  decode/resample -> accumulate exact encoder frames -> encode/mux -> finalize
  artifact -> verify output truth.
- Keep sample format, channel layout, sample rate, frame size, and encoder selection explicit at the boundary where they are chosen.
- Prefer real media probes and small targeted regression tests over codec speculation when audio quality, channel shape, duration, or output validity changes.
- Keep Native AAC, Apple AAC/AAC-AT, and external FDK behavior distinct. They are different encoder/toolchain targets with different sample formats and quality profiles.
- Treat Native AAC as a compatibility path. Do not hide quality limitations behind silent downgrade behavior.

## Hard Invariants

- For planar audio, use typed plane access such as `frame.plane::<T>(ch)` and `frame.plane_mut::<T>(ch)`, or an explicit FFmpeg `extended_data`-aware helper. Do not use `data(ch)` or `data_mut(ch)` as channel-presence or sample-copy truth for planar audio.
- Byte `linesize` is not per-channel audio truth for planar frames. A zero byte linesize on channel index `> 0` must not be interpreted as a missing channel without typed-plane or raw-plane confirmation.
- Silence padding is allowed only for a deliberate short-frame/tail policy or a verified missing-plane condition, and the behavior must have regression coverage.
- Sample sanitization may repair NaN/Inf or clamp out-of-range floats before encoding, but it must not mask channel-layout, frame-size, or format mismatches.
- Resampler output buffers must account for pending swr delay plus input samples
  scaled to the output rate; EOF drains must stream flushed frames through the
  accumulator/encoder instead of collecting the whole drain.
- Encoder option changes must include evidence for the affected encoder path: targeted tests, real-file `ffprobe`/`ffmpeg` diagnostics, or documented external encoder behavior.

## Audio Integrity Traps

Treat these as evidence of an audio-boundary assumption to investigate, not as cosmetic warnings:

- repeated frame-plane warnings
- preview/full divergence for the same encoder path
- missing or silent output channels
- distorted audio with structurally normal progress reporting
- mismatched source/output duration beyond expected preview boundaries
- wrapper API behavior that disagrees with FFmpeg frame/layout semantics

When one appears, name the affected boundary, state the current assumption used to continue, and add or propose the smallest regression test, invariant, or doc guard that would prevent recurrence.

## Done Criteria

- Sample-buffer, resampler, or encoder-boundary changes include focused regression coverage for channel preservation and tail/frame behavior.
- Native AAC changes include a real-media probe when feasible: output codec/profile, sample rate, channels, duration, and at least one channel-level sanity check such as RMS/peak parity.
- Audio correctness fixes run the focused audio/runtime checks warranted by the touched boundary before being presented as done; escalate only when the change crosses owners or uses real-media behavior the focused checks cannot prove.
- Final notes distinguish structural correctness from subjective encoder quality when discussing Native AAC artifacts.

## Chapter Intake

Analysis attaches sibling CUE diagnostics and a source-fingerprinted candidate
chapter plan to each MP3. `apply_chapter_plans` validates accepted payload plans
against the audio identity and duration before dispatch. CUE confirmation or
Ignore is explicit; multi-source CUE merging is rejected. Encoder adapters
consume accepted chapters, while passthrough source probing remains for cover
art and external artifact readers. Preview continues to omit chapters.
