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
not a prefix of the revision. On macOS the build links only AudioToolbox,
CoreAudio, and CoreFoundation; upstream's list adds video, OpenGL, OpenCL,
and QTKit frameworks that the audio-only build never calls.

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

`bun run ffmpeg:upstream` lists the commits after the pin that touch the
sources of the enabled components (from FFmpeg's Makefiles, with their arch
variants and templates), the fixed list `ALWAYS_WATCHED` in the script
(`configure`, libavcodec's encode and decode cores, libavformat's demux, mux,
I/O and MP4 and RIFF helpers, the Native AAC encoder, the libavutil transform,
float DSP, AES and encryption-info files, libswresample), and the chapter
patch, and names the newest BtbN
autobuild with its master commit. Read it before choosing the new commit.
Blind spot: a helper that `configure` selects for a component and that no
Makefile line names for it, beyond the fixed list in the script, is not
watched.

1. Write the new commit to `ffmpeg-revision`, and move `ffmpeg_cli_release` and
   `ffmpeg_cli_build` in `scripts/setup.sh` to the BtbN master autobuild of
   that commit. `scripts/setup-pins.test.ts` enforces the pairing: the build
   id's `g` suffix must be a prefix of `ffmpeg-revision`, and the installer
   must download `ffmpeg_cli_release` when the tools folder holds a different
   FFmpeg 9. Then run `bash scripts/setup.sh rust` to fetch the source and
   that CLI.
2. Build the engine with its `bundled-ffmpeg` feature
   (`bash scripts/verify.sh engine`). `crates/abb-ffmpeg` is the only code
   that uses these bindings: an FFmpeg function or field it calls that
   upstream renamed or removed fails its build there, and a listed component
   `configure` no longer enables fails `build.rs`. The chapter patch must
   still apply.
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

## Enabled components

`build.rs` runs `configure` with `--disable-everything`, `--disable-network`,
and the lists in `ABB_COMPONENTS` (plus `libopus` and, on Apple targets,
`aac_at`). Why: ABB opens untrusted local files, and FFmpeg probes a file
against every compiled demuxer. It never uses FFmpeg for network I/O (HTTP is
ABB's own client). `configure` adds what its `select` rules require (the `mov`
muxer behind `mp4` and `ipod`; the `ac3` parser, `pgs_frame_merge` and
`vp9_superframe` filters behind the MP4 and Matroska muxers); those cannot be
turned off from the command line. `configure` only warns about a name it does
not know and quietly disables a component whose dependency is missing, so
`build.rs` then reads `config_components.h` and fails the build, naming each
listed component it does not find enabled.

| Group | Components | The ABB path that needs it |
| --- | --- | --- |
| Demuxers | `mov`, `mp3`, `aac` (raw ADTS), `wav`, `flac`, `matroska` | One per import extension (`audio/extensions.rs`: m4a, m4b, mp3, aac, wav, flac, mka); `mov` also reads Audible AAX and AAXC with the license keys, and every ABB export |
| Muxers | `mp4`, `ipod`, `matroska` | M4B, M4A Opus, MKA Opus output; `ipod` is what FFmpeg guesses from an `.m4b` or `.m4a` name (`Output::create_for_file_name`: a Preserve merge, always AAC). No `.mka` path reaches a guessed name, because a merge needs AAC sources and MKA output is Opus only |
| Muxers | `mp3`, `flac`, `adts`, `wav` | A tag Save on an MP3, FLAC, raw AAC or WAV remuxes with the file's own container (`metadata/container.rs`) |
| Decoders | `aac`, `aac_at` (Apple) | AAC import and verification; `aac_at` is an import candidate on macOS |
| Decoders | `mp3float`, `mp2float`, `mp1float` | MP3 import (an `.mp3` may hold Layer I or II); `mp3float` is the decoder FFmpeg picks for MP3 |
| Decoders | `flac`, `opus`, `vorbis`, `alac` | FLAC import; Opus, Vorbis, FLAC, ALAC inside MKA or M4A |
| Decoders | `ac3`, `eac3` | AC-3 and E-AC-3 audiobooks in an M4A or M4B (owner decision: they stay importable) |
| Decoders | `pcm_u8`, `pcm_s16le`, `pcm_s24le`, `pcm_s32le`, `pcm_f32le`, `pcm_f64le`, `pcm_s16be`, `pcm_s24be`, `pcm_s32be`, `pcm_alaw`, `pcm_mulaw`, `adpcm_ima_wav`, `adpcm_ms` | WAV import (the payloads its mainstream format tags name) and PCM in MKA or M4A |
| Decoders | `mjpeg` | `avformat_find_stream_info` decodes a JPEG cover. Built without it, every file with a JPEG cover logged "Could not find codec parameters" and a FLAC tag Save wrote 0 instead of 24 as the picture's colour depth (checked on a FLAC with a JPEG cover) |
| Encoders | `aac` (Native NMR), `aac_at` (Apple), `libopus` | The three encoders; FAAC is the separate `faac-sys` library |
| Parsers | `aac`, `mpegaudio`, `flac`, `opus`, `vorbis`, `mjpeg` | Packet durations and timing for each decoded codec |
| Bitstream filters | `aac_adtstoasc` | Raw ADTS AAC muxed into MP4 or Matroska |
| Protocols | `file` | `Input::open` and `Output::create` take local paths only |

Dropped on purpose, so a file with a supported extension but another real
format stops importing (it shows as invalid): Ogg, AIFF, CAF, ASF/WMA, and raw
AC-3. Audible requests AAC-LC only (`mp4a.40.2`). Of the 98 audio codecs named
in the WAV, MOV and Matroska tag tables, 66 have no decoder now; the ones a
real file could hold, so listed as unsure: `aac_latm`,
`mp3on4`, `pcm_s8`, `pcm_f32be`, `pcm_f64be`, `pcm_s64le`, `adpcm_ima_qt`
(MOV `ima4`), `adpcm_g722`, `adpcm_g726`, `gsm`, `gsm_ms`, `ilbc`, `speex`,
`wavpack`, `tta`, `truehd`, `mlp`, the `wma*` family, and the legacy
RealAudio, ATRAC, Cook, Sipr and Nellymoser decoders. No PNG, GIF, BMP, TIFF,
WebP, or JPEG XL decoder or parser is built (PNG never was:
`--disable-autodetect` removes zlib); FFmpeg logs a warning for such a cover,
the import still works, and a tag Save reads the picture's size from its header
(`metadata/remux.rs`). A JPEG XL cover blocks a Save: ABB reads no JPEG XL
header, an ID3 picture carries no size, and every muxer refuses a picture
without one, so a Save of an MP3 that carries one fails and keeps the original.

No image encoder is built. ABB writes a JPEG or PNG cover by filling the
attached-picture stream's codec parameters and copying the bytes
(`abb-ffmpeg` `Output::add_attached_picture_stream`); libavformat looks up no
encoder for it. Cover conversion uses the `image` crate.

A change that adds an import extension, an output format or an encoder
extends `ABB_COMPONENTS` in the same change.
`integration_media_import_formats_tests.rs` fails when a supported extension
has no fixture recipe, and when a recipe no longer imports, decodes and
encodes.

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
passes against unpatched upstream. Advance the source, the calls in
`crates/abb-ffmpeg`, and header/runtime verification together. Unbundled builds must satisfy the same
source/patch contract. Linux fixture readback is the BtbN autobuild named by
`ffmpeg_cli_release` and `ffmpeg_cli_build`, the same commit as
`ffmpeg-revision`. macOS readback stays Homebrew's major-9 ffmpeg. `--check`
and `--print-env` accept that pinned build or a major-9 CLI; they do not
install.
