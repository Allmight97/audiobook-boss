# ABB bundled FAAC and FAAD3

AudioBook Boss statically links the FAAC encoder (libfaac) and the FAAD3
decoder (libfaad3) from
[FreewareAdvancedAudio/faac](https://github.com/FreewareAdvancedAudio/faac),
`master` at commit `4d6d038ed164648e5a5924293f4e522820ddd137`, which declares
version 3.0.0 and is not yet a tagged release.

`upstream/` holds that commit's portable library sources listed in
`libfaac/meson.build`, `libfaad/meson.build`, and `common/meson.build`, their
headers, the public `include/faac.h` and `include/faad3.h`, and the license,
unmodified. The command-line tools, SIMD sources, tests, and upstream build
files are omitted. `build.rs` compiles these sources with HE-AAC (SBR) and
dynamic range control support in both libraries, parametric stereo in the
decoder only, the encoder limited to two channels and the decoder to eight,
and `PACKAGE_VERSION` set to 3.0.0.

## License and source

FAAC and FAAD3 are licensed under LGPL-2.1-or-later; see `upstream/COPYING`.
Each ABB release's source archive on
[GitHub Releases](https://github.com/Allmight97/audiobook-boss/releases)
contains this source and ABB's build scripts. To use a modified FAAC or FAAD3,
edit `vendor/faac-sys/upstream/` in that source and rebuild ABB as its README
describes.
