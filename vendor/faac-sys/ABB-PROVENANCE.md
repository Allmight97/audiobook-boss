# ABB bundled FAAC dependency

Upstream: [knik0/faac](https://github.com/knik0/faac), development commit
`3aa4c6d3b0d99223dfb8cab922f2db3ff9ce66b8` (25 commits after release 2.1).
The selected source files and COPYING were verified against that Git revision.
The build reports `2.1.0-dev.3aa4c6d` to distinguish it from the release.

`upstream/` contains the portable `common_src` files named by upstream
`libfaac/meson.build`, their required `stats.h`, the public `include/faac.h`,
and the LGPL license. The CLI, SIMD extension, build outputs, and unrelated
documentation are omitted. `build.rs` compiles these sources with `cc`,
using portable optimization without host CPU tuning or LTO. Configuration is
two channels maximum and full SBR analysis density (`FAAC_SBR_DECIMATION=1`).
Upstream now requires the generated configuration to be force-included in
every C translation unit. Bindgen generates the same public header that the
C compiler consumes; ABI 2 requires caller size in `faac_params_init`.

`timing.patch` records the two local corrections applied to `upstream/`.
To reproduce, copy the selected files from the recorded Git revision and
apply `patch -p1 < ../timing.patch` inside `upstream/`. Reversing the patch
restores the original files. It:

- drains HE FIR/QMF history through the existing zero-padded input path;
- declares that HEv1 ASC has no parametric stereo, preserving mono channels.

Upstream supplies short-input flushing and the additional HE drain packet;
the earlier local replacements were removed. The old local delay-info fields
were also removed. Upstream `encoder_delay` reports LC 1024 / HE 3041 samples;
the HE value includes the additional 962-sample decoder delay and must not
be used as an unconditional MP4 trim. Apple and native FFmpeg compensate it
differently. This remains a buildable dependency, with no selectable ABB
FAAC encoder or production mux/import timing policy yet.

Latest-source proof on macOS: documented flushing produced nonempty,
sufficient-capacity output in all 40 LC/HE rate/channel/length cases.
Unmodified upstream differed from its explicit zero-extension reference in
10 cases; the local patch restored exact decoded reference matches in all
40. Stock mono HE MP4 was reported as two channels; patched MP4 kept mono.
With retained trailing packets and 2079-sample core priming, Apple decoding
kept all 20 tested input lengths within one sample; native FFmpeg still
requires explicit demux/skip control and a 3041-sample crop to match the full
reference (20/20). Those research controls are not a general importer fix.

AAC-LC opens at 22050, 32000, 44100, and 48000 Hz. Upstream HE requires
at least 32000 Hz and rejects 22050 Hz; a future caller must expose that
constraint. The sys smoke test covers six HE and eight LC rate/channel
combinations and rejection of both 22050 Hz HE configurations.

The caller owns each encoder handle and closes it via `faac_encoder_close`.
ASC pointers are library-owned until close and must be copied before release.
`FAAC_INPUT_FLOAT` is interleaved PCM scaled to signed-16 units: `bit_rate` is
per channel and encode input counts are total samples across channels.

Library source and local changes are LGPL-2.1-or-later; see
[upstream/COPYING](upstream/COPYING). Packaging must satisfy the LGPL obligations
for the linked library; this source dependency alone is not release proof.
