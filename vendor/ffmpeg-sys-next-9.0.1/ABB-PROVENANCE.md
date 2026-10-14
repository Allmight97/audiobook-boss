# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.1 archive, SHA-256
`c6f932d8c1cede016c65d4f1435f323369c99c74df55143b18676aa10e7603f9`.
The upstream manifest declares WTFPL. 9.0.1 adds `-fno-builtin` to bindgen so
the generated bindings do not declare libc runtime symbols, and it parses
`libavutil/spherical.h`.

`ffmpeg-revision` selects the immutable FFmpeg development source required by
Native AAC's NMR coder. The Rust build consumes this file and verifies the
fetched commit. ABB retains the CoreAudio framework link.

The October 14, 2026 selection is
`2ec8998c3daf6aa31a4afec963bff9b5ecbfdee7`, upstream master HEAD that day.
It keeps the October 7 pin's NMR encoder (`17ec99894249`, including
`98afaae587` and `aac_rc` default `cbr`) and takes the later mov demuxer
bounds and edit-list fixes plus the USAC decoder fixes on master since that
pin. The wrapper maps `AV_CODEC_ID_RW2` and `AV_PKT_DATA_GAIN_MAP_PARAMS`.
Feature selection and external dependencies are unchanged.

Bundled cache reuse requires the source/patch identity and the effective build
inputs: build-script contents, compiler/version, target, features, CPU flags,
SDK/sysroot and relevant compiler environment. Rebuild notifications are emitted
before deciding reuse. The C library is stored in `target/abb-ffmpeg-cache/<identity>`
rather than Cargo's per-unit `OUT_DIR`, because `cargo build` and `cargo clippy`
are different units and would otherwise each compile FFmpeg. The build names the
static libopus archive's directory for the linker, since Linux pkg-config omits
system library directories.
Native and portable CPU mechanisms come from upstream sys 9.0.1; ABB's
`bundled-ffmpeg` feature always enables `build-portable`. Upstream lets
`FFMPEG_MARCH`/`FFMPEG_MTUNE` override `build-portable`; ABB refuses either
variable under `build-portable`, so an inherited `FFMPEG_MARCH=native` cannot
make a distributable build native.

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
