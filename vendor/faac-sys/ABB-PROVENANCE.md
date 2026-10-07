# ABB bundled FAAC and FAAD

AudioBook Boss statically links the FAAC encoder and the FAAD3 decoder from
[FreewareAdvancedAudio/faac](https://github.com/FreewareAdvancedAudio/faac),
pull request 29 (`faad-upstream`), commit
`66a7c2d2943f699a23ebf37e25e8b34d6bdbf513`. This commit is not a release; it
follows release `faac-2.2`.

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
