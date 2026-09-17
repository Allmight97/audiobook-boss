# ABB bundled FAAC dependency

The bundled source is selected from [knik0/faac](https://github.com/knik0/faac)
at upstream revision `4d9c9b6e099ac22c9de630d46b1f3c75fde499a2` (2026-09-14,
`ratecontrol: add a real bit reservoir for CBR behind --cbr (#199)`).

`upstream/` contains the portable scalar files listed by upstream
`libfaac/meson.build`, including the current `ratecontrol.c` module and its
header, the public `include/faac.h`, and the LGPL license. The CLI, SIMD source,
build outputs, and unrelated documentation are omitted. The Rust build compiles
the same source list with `cc`, force-includes its generated configuration
(`MAX_CHANNELS=2`, `FAAC_SBR_DECIMATION=1`, `PACKAGE_VERSION=2.1.0-dev.4d9c9b6`), and runs
bindgen against the same public header used by the C build. This keeps compiled
C and generated Rust bindings on one header/configuration contract.

The current upstream source already includes the HE tail-drain path and the
mono HE-AAC AudioSpecificConfig parametric-stereo signal, so no local timing
patch is carried in this dependency slice. ABB explicitly selects `FAAC_RC_ABR` and verifies the opened mode. The sys
smoke test also checks upstream AUTO resolution with a nonzero `bit_rate`.

ABB carries one concurrency patch in `libfaac/quantize.c`: `QuantizeInit`
publishes the shared lookup tables once with C11 acquire/release atomics.
Upstream initializes them on every encoder open, racing with other opens and
active encodes in a parallel batch. After publication the tables stay immutable;
encoder handles remain independent. Retire this patch when upstream supplies
equivalent thread-safe initialization. Keep `FAAC_STATS` disabled: its optional
global counters are not synchronized. The sys tests compare parallel and
sequential packet output; a standalone ThreadSanitizer probe reproduces the
upstream initialization race and checks this repair.

The caller owns each encoder handle and closes it with `faac_encoder_close`.
AudioSpecificConfig pointers are library-owned until close and must be copied
before release. `FAAC_INPUT_FLOAT` is interleaved PCM scaled to signed-16 units;
`bit_rate` is per channel and encode input counts are total samples across
channels.

The selected library source and license are LGPL-2.1-or-later; see
[upstream/COPYING](upstream/COPYING). Packaging must satisfy the LGPL
requirements for the linked library, including the obligations that apply to
static linking; this source dependency alone is not release proof.

ABB's `processor/faac_timing.rs` owns the MP4/core-priming and native-decoder
interval handoff. Changing FAAC timing requires the real-media Apple playback
and ABB re-import regressions to pass; do not retain a revision merely because
it was previously selected. This source is current upstream HEAD at adoption,
not the older 2.1 release that predates the tail and mono configuration fixes.
