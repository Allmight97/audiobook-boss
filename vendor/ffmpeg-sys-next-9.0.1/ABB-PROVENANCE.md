# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.1 archive, SHA-256
`c6f932d8c1cede016c65d4f1435f323369c99c74df55143b18676aa10e7603f9`.
The upstream manifest declares WTFPL. 9.0.1 adds `-fno-builtin` to bindgen so
the generated bindings do not declare libc runtime symbols, and it parses
`libavutil/spherical.h`.

`ffmpeg-revision` holds the FFmpeg commit ABB builds: a `master` commit,
because Native AAC's NMR coder is not in a release. The Rust build fetches
that commit and checks the checkout. `scripts/setup.sh` names it again:
`ffmpeg_cli_build` is the BtbN describe id, and its `g` suffix is that commit;
`ffmpeg_cli_release` is the autobuild tag that publishes it. Those two move
with `ffmpeg-revision`. `scripts/setup-pins.test.ts` fails when the suffix is
not a prefix of the revision. ABB retains the CoreAudio framework link.

What ABB relies on in that source:

- NMR rate control, speed levels, and the public enum mappings. ABB selects
  NMR, CBR, target bitrate, and speed, and inherits automatic M/S and cutoff
  plus enabled intensity stereo, PNS, and TNS. ABR and quality-based VBR are
  not enabled.
- No FFmpeg seek call: every input decodes from its first packet, so priming
  arrives as first-packet skip-samples side data.
- The native HE-AAC decoder outputs its SBR delay untrimmed; ABB's HE window
  trims it.
- `discard_samples()` in `libavcodec/decode.c`, which the FAAD3 adapter
  mirrors (`faad_decoder_tests.rs` holds upstream's `api-skip-samples` table).
- `patches/mov-chapter-start.patch` (below).

## Moving the pin

1. Write the new commit to `ffmpeg-revision`, and move `ffmpeg_cli_release` and
   `ffmpeg_cli_build` in `scripts/setup.sh` to the BtbN master autobuild of
   that commit. `scripts/setup-pins.test.ts` enforces the pairing: the build
   id's `g` suffix must be a prefix of `ffmpeg-revision`, and the installer
   must download `ffmpeg_cli_release` when the tools folder holds a different
   FFmpeg 9. Then run `bash scripts/setup.sh rust` to fetch the source and
   that CLI.
2. Build the engine with its `bundled-ffmpeg` feature
   (`bash scripts/verify.sh engine`). A new upstream enum value fails the build
   in `../ffmpeg-next-9.0.0`; map it there and list it in that crate's
   provenance. The chapter patch must still apply.
3. Recheck each item in the list above against the upstream diff, and update
   the FAAD3 trim table if `api-skip-samples` changed.
4. Encode one fixed fixture with Native AAC at the old and the new pin, decode
   both with the same FFmpeg CLI, and compare `ffmpeg -i out.m4b -map 0:a -f md5 -`.
   Packet bytes can change while the decoded audio stays identical. A decoded
   difference needs the real-media probe in `crates/abb-engine/src/audio/AGENTS.md`.
5. Run `bash scripts/verify.sh engine media`.

FFmpeg is configured with `--disable-autodetect`; its enabled external codec
library is static libopus, with AudioToolbox supplied by the Apple SDK on macOS.
ABB uses libavcodec, libavformat, and libswresample, so the build disables
libavdevice, libavfilter, and libswscale. Resolved Rust dependency versions
live in `Cargo.lock`.

Bundled cache reuse requires the source/patch identity and the effective build
inputs: build-script contents, compiler/version, target, features, CPU flags,
SDK/sysroot and relevant compiler environment. Rebuild notifications are emitted
before deciding reuse. The C library is stored in `target/abb-ffmpeg-cache/<identity>`
rather than Cargo's per-unit `OUT_DIR`, because Cargo treats a build and a Clippy
run as different units and would otherwise each compile FFmpeg. Each build marks its
tree used and deletes sibling trees unused for 14 days that no build holds locked;
a build releases its lock before Cargo links against the tree, so only age makes a
tree safe to delete. Cargo reruns the build script when its tree's
`abb-build-inputs` stamp changes or disappears, so a profile whose cached build
output named a pruned tree rebuilds it instead of failing to link. The source clone comes from
`target/abb-ffmpeg-source/<revision>.git` when `scripts/setup.sh rust` has fetched
it, so a build after setup needs no network, and from GitHub otherwise. The build names the
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
source/patch contract. Linux fixture readback is the BtbN autobuild named by
`ffmpeg_cli_release` and `ffmpeg_cli_build`, the same commit as
`ffmpeg-revision`. macOS readback stays Homebrew's major-9 ffmpeg. `--check`
and `--print-env` accept that pinned build or a major-9 CLI; they do not
install.
