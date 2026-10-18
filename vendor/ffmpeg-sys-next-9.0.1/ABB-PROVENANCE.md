# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.1 archive, SHA-256
`c6f932d8c1cede016c65d4f1435f323369c99c74df55143b18676aa10e7603f9`.
The upstream manifest declares WTFPL. 9.0.1 adds `-fno-builtin` to bindgen so
the generated bindings do not declare libc runtime symbols, and it parses
`libavutil/spherical.h`.

`ffmpeg-revision` selects the immutable FFmpeg development source required by
Native AAC's NMR coder. The Rust build consumes this file and verifies the
fetched commit. ABB retains the CoreAudio framework link.

The October 18, 2026 selection is
`7045cc9b8020ebaae156f4947baa752625056f90`, verified as master HEAD at both
`git.ffmpeg.org` and its GitHub mirror. Compared with ABB's October 14 pin
`2ec8998c3daf6aa31a4afec963bff9b5ecbfdee7`, it includes:

- [Short-window spread-energy indexing fix](https://github.com/FFmpeg/FFmpeg/commit/89d34d66a638b52d56023d547e785291f67efb2f)
  in AAC psychoacoustic hole avoidance. Upstream reports bit-identical output
  on its tested music and transient samples; this is not a listening-quality claim.
- [Intensity-stereo loop optimization](https://github.com/FFmpeg/FFmpeg/commit/b60ab6c9a4583cd4bbdc010b412d598945919da8),
  with no intended output change.
- MOV sample-rate validation (`3be6c2d943`) and float-cast overflow prevention
  (`c705686a33`), ADTS packet keyframe marking (`a58779b2e7`), and Vorbis seek
  timestamp correction (`b863390a8d`).

NMR rate control, speed levels, public enum mappings, and swresample are unchanged
from the previous pin. ABB explicitly selects NMR, CBR, target bitrate, and speed;
it inherits automatic M/S and cutoff plus enabled intensity stereo, PNS, and TNS.
The existing ABR and quality-based VBR options change the bitrate contract and
are not enabled by this update. No additional NMR tuning option was introduced
in this interval.

FFmpeg is configured with `--disable-autodetect`; its enabled external codec
library is static libopus, with AudioToolbox supplied by the Apple SDK on macOS.
Opus 1.6.1 remains the latest stable upstream release and the locally verified
archive version. No FFmpeg wrapper release supersedes sys 9.0.1 / next 9.0.0.
The FFmpeg Rust dependency closure is refreshed in `Cargo.lock`: bitflags
2.13.2, cfg-if 1.0.5, hermit-abi 0.5.3, libc 0.2.191, syn 3.0.9, and
unicode-ident 1.0.27. Other resolved dependencies in that closure are unchanged.

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
