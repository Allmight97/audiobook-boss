# ABB bundled FAAC and FAAD

AudioBook Boss statically links the FAAC encoder and the FAAD3 decoder from
[FreewareAdvancedAudio/faac](https://github.com/FreewareAdvancedAudio/faac),
the open stack of pull requests 37 (`0bbb491d845032b413c5111125b014e642112825`,
libfaam), 38 (`a03656f5879e769135a550f24d85158b3fc1d08c`, MP4 through libfaam),
and 39 (`333fe311d0eb64165efd62f94e87c33960b442fc`, libfaad). This commit is
not a release; it follows release `faac-2.2`.

`upstream/` holds that commit's portable sources listed in
`libfaac/meson.build`, `libfaad/meson.build` and `common/meson.build`, their
headers, the public `include/faac.h` and `include/faad.h`, and the license,
unmodified. The command-line frontends, SIMD sources, and upstream build files
are omitted. `build.rs` compiles these sources with `MAX_CHANNELS=2`,
`FAAC_SBR_DECIMATION=1`, `WORDS_BIGENDIAN` from the target, and
`PACKAGE_VERSION` set to the package version.

## License and source

FAAC and FAAD are licensed under LGPL-2.1-or-later; see `upstream/COPYING`.
Each ABB release's source archive on
[GitHub Releases](https://github.com/Allmight97/audiobook-boss/releases)
contains this source and ABB's build scripts. To use a modified FAAC or FAAD,
edit `vendor/faac-sys/upstream/` in that source and rebuild ABB as its README
describes.
