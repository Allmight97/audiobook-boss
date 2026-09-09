# ABB bundled FAAC dependency

Upstream: [knik0/faac](https://github.com/knik0/faac), release 2.1.0,
commit `028707109fe62da278870138e152ac664f9c6a4f`.
The exact commit archive SHA-256 is
`3c5c0a6fc082f25e1c9edc863d6f31873e84a2beeca203b08da8a4fbcca589bc`.
The copied C sources, headers, and COPYING were verified against that archive.

`upstream/` contains the portable `common_src` files named by upstream
`libfaac/meson.build`, the public `include/faac.h`, and the LGPL license.
The CLI, SIMD extension, build outputs, and unrelated documentation are omitted.
`build.rs` compiles these sources with `cc`, using portable optimization without
host CPU tuning or LTO. Configuration is two channels maximum and full SBR
analysis density (`FAAC_SBR_DECIMATION=1`). Bindgen generates the public
`faac_*` API from the same header that the C compiler consumes.

`timing.patch` records the local changes already applied to `upstream/`.
To reproduce, copy the selected files from the pinned archive and apply
`patch -p1 < ../timing.patch` inside `upstream/`. Reversing that patch restores
those files to the original revision. The patch:

- makes zero-input flushing advance through internal priming before reporting
  EOF for short sources;
- drains the HE FIR/QMF history through the existing zero-padded input path and
  permits the additional HE drain frame;
- declares that HEv1 ASC has no parametric stereo, preserving mono channels;
- appends separate encoder-priming and decoder-delay facts to encoder info.

AAC-LC opens at 22050, 32000, 44100, and 48000 Hz. Upstream HE requires
at least 32000 Hz and rejects 22050 Hz; the caller must expose that constraint.
The smoke test covers the six HE and eight LC rate/channel combinations and
checks explicit rejection of both 22050 Hz HE configurations.

The delay fields do not prescribe MP4 trimming. LC reports 1024 encoder-priming
samples and no extra decoder delay. HE reports 2079 core-compatible priming
samples and 962 additional SBR decoder-delay samples at the full sample rate.
Decoder compensation differs; do not add these fields unconditionally. This
patch and the sys smoke tests do not qualify HE playback or presentation timing.

The caller owns each encoder handle and closes it via `faac_encoder_close`.
ASC pointers are library-owned until close and must be copied before release.
`FAAC_INPUT_FLOAT` is interleaved PCM scaled to signed-16 units; `bit_rate` is
per channel and encode input counts are total samples across channels.

Library source and local changes are LGPL-2.1-or-later; see
[upstream/COPYING](upstream/COPYING). Packaging must satisfy the LGPL obligations
for the linked library; this source dependency alone is not release proof.
