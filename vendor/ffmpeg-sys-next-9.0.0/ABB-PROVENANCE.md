# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.0 archive, SHA-256
`9b939bf79dd5949412a4b81cfe21a07f48ea21b47fcbb5f57816c8c2de5ae30b`.
The upstream manifest declares WTFPL.

`ffmpeg-revision` selects the immutable FFmpeg development source required by
Native AAC's NMR coder. The Rust build consumes this file and verifies the
fetched commit. ABB retains the CoreAudio framework link.

The October 7, 2026 selection is
`17ec99894249a68444d145ea8870713a4fb7f342`, upstream master HEAD that day. It
takes the NMR encoder series merged October 4 (`72d8fb25b6`, `77506207d2`,
`a86dc8bd2a`, `98afaae587`, `03ce7af84b`, psy `f68ccd1b39` and `88e8cfc801`):
allocation retune and bandwidth ladder, pressure-independent intensity stereo
for correlated long-window bands above 8 kHz (`aac_is` default unchanged at 1),
and the new `aac_rc` rate-control option (default `cbr`). It also takes AAC
decoder fixes (`9cee5ae719`, `23316a12ac`, `e383b7d532`) and the swresample
rematrix series (`70e948e2e5`, `4bab8327f9`, `09f879b074`, `a17a624069`).
Feature selection and external dependencies are unchanged. The wrapper maps the
new codec, pixel-format and frame side-data identifiers explicitly.

Bundled cache reuse requires the source/patch identity and the effective build
inputs: build-script contents, compiler/version, target, features, CPU flags,
SDK/sysroot and relevant compiler environment. Rebuild notifications are emitted
before deciding reuse. The C library is stored in `target/abb-ffmpeg-cache/<identity>`
rather than Cargo's per-unit `OUT_DIR`, because `cargo build` and `cargo clippy`
are different units and would otherwise each compile FFmpeg. The build names the
static libopus archive's directory for the linker, since Linux pkg-config omits
system library directories.
Native and portable CPU mechanisms come from upstream sys 9.0.0; ABB's
`bundled-ffmpeg` feature always enables `build-portable`.

The header-only CUDA shim adds `CUarray` and `CUDA_ARRAY3D_DESCRIPTOR` so bindgen
can parse the selected headers. Layout follows nv-codec-headers revision
`eddcea9e27f6b772057c9b3f87de2cc1737faffc`; on arm64 the descriptor is 40 bytes,
alignment 8, with offsets 0/8/16/24/28/32. No CUDA implementation is bundled.
Retire these declarations when upstream headers/bindings no longer require them.

`patches/mov-chapter-start.patch` initializes a generated QuickTime chapter
track's first DTS as unknown, matching ordinary tracks. Otherwise the muxer
resets a nonzero first chapter start. The media regression prepends chapterless
audio to a chaptered M4B and verifies accepted chapter names and positions,
plus the QuickTime edit and sample timing tables.

Retire the chapter patch only when the same nonzero-first-chapter regression
passes against unpatched upstream. Advance source, wrapper mappings and
header/runtime verification together. Unbundled builds must satisfy the same
source/patch contract; fixture/readback CLI binaries are a separate toolchain.
