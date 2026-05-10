# Audio Pipeline Directives

## Scope

- Applies to audio-domain code under `src-tauri/src/audio/`.
- Nested `AGENTS.md` files own narrower rules for `processor/` and `job_registry/`.
- This file owns audio integrity rules that cross stream probing, decoder setup, resampling, sample buffering, encoder setup, muxing, and output validation.

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
