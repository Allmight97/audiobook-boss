# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.0 archive, SHA-256
`9b939bf79dd5949412a4b81cfe21a07f48ea21b47fcbb5f57816c8c2de5ae30b`.
The upstream manifest declares WTFPL.

`ffmpeg-revision` selects the immutable FFmpeg development source required by
Native AAC's NMR coder. The Rust build and Linux setup consume this file and
verify the fetched commit. ABB retains the CoreAudio framework link.

The September 18, 2026 selection is
`be387f252de068a38e3b0f5f40404ec55c14578c`. Compared with the previous selection,
NMR's `libavcodec/aaccoder_nmr.h` is unchanged. The refresh includes MP4 metadata
allocation/read validation fixes and shared media I/O changes; it is not an NMR
algorithm update. Existing feature selection and external dependencies remain
unchanged. The wrapper represents the new ASTC codec identifier explicitly.

On September 23, upstream HEAD `4a511dc8eba349b43817e49661b28d521c6b604a`
contained no changes to the selected AAC/NMR encoder sources. ABB retains this
pin rather than taking unrelated decoder, platform, and muxer changes solely
to advance the revision label.

Bundled cache reuse requires the source/patch identity and the effective build
inputs: build-script contents, compiler/version, target, features, CPU flags,
SDK/sysroot and relevant compiler environment. Rebuild notifications are emitted
before deciding reuse. Linux setup records its source/patch identity separately.
Native and portable CPU mechanisms come from upstream sys 9.0.0; ABB selects
native builds for development and portable builds for distribution.

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
