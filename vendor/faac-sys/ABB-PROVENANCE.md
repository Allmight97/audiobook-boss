# ABB bundled FAAC dependency

The bundled source is selected from [knik0/faac](https://github.com/knik0/faac)
at upstream revision `9edb7dbf32101cbb2b4aed66e3142501ca6385eb` (2026-09-21,
`frame: remove cold path attribute from doHEAACFrame (#216)`). This was upstream HEAD
when checked on 2026-09-21; builds use this immutable revision's source slice.

`upstream/` contains the portable scalar files listed by upstream
`libfaac/meson.build`, their headers (including `atomic.h`), the public
`include/faac.h`, and the LGPL license. The CLI, SIMD source, build outputs,
and unrelated documentation are omitted. The Rust build compiles the same
source list with `cc`, force-includes its generated configuration
(`MAX_CHANNELS=2`, `FAAC_SBR_DECIMATION=1`,
`PACKAGE_VERSION=2.1.0-dev.9edb7db`), and runs bindgen against the same public
header used by the C build. Compiled C and generated Rust bindings therefore
share one header/configuration contract.

## Local changes and upstream initialization

ABB carries no local changes to the selected upstream source files.
[Upstream #215](https://github.com/knik0/faac/pull/215) replaces ABB's former
quantizer initialization patch: upstream now initializes the shared quantizer,
FFT, and window tables once before an encoder uses them. Per-encoder handles
remain independent. Keep `FAAC_STATS` disabled because its optional global
counters are unsynchronized. Sys tests compare parallel and sequential packet
output; this is regression evidence, not an exhaustive race detector.

Upstream already includes the HE tail-drain and mono HE-AAC
AudioSpecificConfig signaling fixes. The shared-table change preserves full
SBR analysis density. [Upstream #217](https://github.com/knik0/faac/pull/217)
replaces the FFT with a Stockham implementation; upstream reports roughly 4%
better throughput with unchanged encoded output. #216 removes restrictive
compiler layout attributes from the HE path. ABB has not measured a throughput
or subjective quality improvement from these changes.

[Upstream #218](https://github.com/knik0/faac/pull/218) primes HE input with one
zero sample so its delay is exactly representable at the AAC core rate. HE's
reported delay becomes 3042 full-rate samples; LC remains at 1024. This changes
HE alignment, not the profile decision or rate-control policy.

## Adapter contract

The caller owns each encoder handle and closes it with `faac_encoder_close`.
AudioSpecificConfig pointers are library-owned until close and must be copied
before release. `FAAC_INPUT_FLOAT` is interleaved PCM scaled to signed-16 units;
`bit_rate` is per channel and encode input counts are total samples across
channels.

ABB sends the requested Auto/LC/HE profile directly to FAAC. ABR supplies the
total target divided by resolved channels; VBR supplies zero bitrate and the
selected quality. FAAC resolves Auto once at open. The adapter reads the
resolved profile, frame size, delay, rate control, bitrate, and quality before
creating mux parameters, and rejects a silently clamped request. Profile Auto
and rate-control Auto are independent upstream features; ABB exposes explicit
ABR/VBR choices with ABR as FAAC’s initial mode.

`src-tauri/src/audio/processor/faac_timing.rs` owns the HE-specific MP4 core
priming and native-decoder interval. LC uses its returned delay and a distinct
encoding-tool tag. New HE files use `AudioBook Boss FAAC HE-AAC timing-2`,
2080 samples of MP4 core priming, and 3042 samples of native decode trimming.
The original `AudioBook Boss FAAC HE-AAC` tag retains its 2079/3041 timing on
re-import. Real-media tests cover Apple playback and ABB re-import
alignment, sample count, and final-tail energy for both resolved profiles.
Apple HE playback can still round an odd full-rate duration by one sample
because its timeline uses the half-rate core clock.
Changes to this handoff need those regressions; an upstream timing fix alone
does not establish that ABB's MP4/decoder adaptation can be removed.

## License and distribution

The selected library source and license are LGPL-2.1-or-later; see
[upstream/COPYING](upstream/COPYING). Packaging must satisfy the LGPL
requirements for the linked library, including the obligations that apply to
static linking; this source dependency alone is not release proof.
