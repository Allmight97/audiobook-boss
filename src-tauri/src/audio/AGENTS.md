# Audio Pipeline Directives

## Scope

- Applies to audio-domain code under `src-tauri/src/audio/`.
- Nested `AGENTS.md` files own narrower rules for `processor/`; processing
  lifecycle rules live under `src-tauri/src/processing/`.
- This file owns audio integrity rules that cross stream probing, decoder setup, resampling, sample buffering, encoder setup, muxing, and output validation.
- Audio is the **Audio Engine Deep Module** Grey-Box Public API owner. Its
  public strip lives at `crate::audio`; processor internals stay private under
  `src-tauri/src/audio/processor/`.

## Public API Strip

- Import from `crate::audio`, not private child modules such as
  `crate::audio::processor`, `crate::audio::settings_encoder`,
  `crate::audio::toolchain`, `crate::audio::path_validation`, or
  `crate::audio::cleanup`.
- Modules: `constants` is crate-visible for existing processing event names and
  progress math until those constants move to the processing/status owner.
- Types: `AudioFile`, `DecoderSelection`, `SampleRateConfig`, `FileListInfo`,
  `AacDecoderAvailability`, `EncoderSettings`, `EncoderType`, `BitrateMode`,
  `ChannelConfig`, `ThreadSetting`, `EncoderAvailability`,
  `EncoderCapabilitySource`, `ExternalToolchainPreference`.
- Functions: `get_file_list_info`, `validate_input_audio_path`,
  `validate_input_image_path`, `validate_output_path`,
  `validate_sample_rate_config`, `validate_encoder_settings`,
  `validate_requested_encoder_available`, `validate_threads`,
  `resolve_encoder_type`, `resolve_encoder_name`, `detect_encoder_availability`,
  `detect_aac_decoder_availability`, `preferred_aac_decoder_order_labels`,
  `execute_audio_engine`, `validate_audio_engine_inputs`.
- Execution request type: `AudioExecutionRequest`.
- Constants: `VALID_ENCODER_BITRATES`, `VALID_THREAD_COUNT_RANGE`.
- Crate-internal helper: `CleanupGuard`.

## Private Cluster

- Files: `buffer.rs`, `cleanup/`, `extensions.rs`, `file_list.rs`,
  `metrics.rs`, `path_validation.rs`, `processor/`, `settings.rs`,
  `settings_encoder.rs`, and `toolchain.rs`.
- The cluster owns decoder/toolchain selection, media inspection,
  decode/resample/encode/mux internals, staging, cleanup, and media execution
  facts. Processing owns lifecycle orchestration and terminal normalization;
  `output_artifact` owns final artifact commit truth.

## Allowed Agent Edits Without Escalation

- Change private implementation files when focused audio/processing tests,
  `scripts/check-public-api-strips.sh`, and `scripts/check-no-bridge-imports.sh`
  stay green.
- Narrow accidental visibility when callers can use the public strip without
  losing contract truth.
- Keep Native AAC, Apple AAC/AAC-AT, and external FDK adapter differences inside
  the private cluster unless a caller needs a stable capability fact.

## Breaking-Change Triggers

- Adding, removing, or renaming any Public API Strip symbol.
- Moving job lifecycle ownership out of processing, final artifact commit truth
  out of `output_artifact`, or metadata policy out of metadata-owned APIs.
- Changing user-visible progress, cancellation, or terminal success/failure
  semantics.

## Preferred Path

- Treat audio processing as a boundary chain: validate paths -> inspect inputs -> choose decoder/toolchain -> decode/resample -> accumulate exact encoder frames -> encode/mux -> finalize artifact -> verify output truth.
- Keep sample format, channel layout, sample rate, frame size, and encoder selection explicit at the boundary where they are chosen.
- Prefer real media probes and small targeted regression tests over codec speculation when audio quality, channel shape, duration, or output validity changes.
- Keep Native AAC, Apple AAC/AAC-AT, and external FDK behavior distinct. They are different encoder/toolchain targets with different sample formats and quality profiles.
- Treat Native AAC as a compatibility path. Do not hide quality limitations behind silent fallback behavior.

## Hard Invariants

- For planar audio, use typed plane access such as `frame.plane::<T>(ch)` and `frame.plane_mut::<T>(ch)`, or an explicit FFmpeg `extended_data`-aware helper. Do not use `data(ch)` or `data_mut(ch)` as channel-presence or sample-copy truth for planar audio.
- Byte `linesize` is not per-channel audio truth for planar frames. A zero byte linesize on channel index `> 0` must not be interpreted as a missing channel without typed-plane or raw-plane confirmation.
- Silence padding is allowed only for a deliberate short-frame/tail policy or a verified missing-plane condition, and the behavior must have regression coverage.
- Sample sanitization may repair NaN/Inf or clamp out-of-range floats before encoding, but it must not mask channel-layout, frame-size, or format mismatches.
- Encoder option changes must include evidence for the affected encoder path: targeted tests, real-file `ffprobe`/`ffmpeg` diagnostics, or documented external encoder behavior.

## Canary Trigger

Trigger Canary when any of these appear:

- repeated frame-plane warnings
- preview/full divergence for the same encoder path
- missing or silent output channels
- distorted audio with structurally normal progress reporting
- mismatched source/output duration beyond expected preview boundaries
- wrapper API behavior that disagrees with FFmpeg frame/layout semantics

Include the affected boundary, the current assumption used to continue, and the smallest doc/test/code guard that would prevent recurrence.

## Done Criteria

- Sample-buffer, resampler, or encoder-boundary changes include focused regression coverage for channel preservation and tail/frame behavior.
- Native AAC changes include a real-media probe when feasible: output codec/profile, sample rate, channels, duration, and at least one channel-level sanity check such as RMS/peak parity.
- Audio correctness fixes run `scripts/checks.sh standard` before being presented as done.
- Final notes distinguish structural correctness from subjective encoder quality when discussing Native AAC artifacts.
