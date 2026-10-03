# ABB bundled FAAC

AudioBook Boss statically links FAAC 2.2 from
[FreewareAdvancedAudio/faac](https://github.com/FreewareAdvancedAudio/faac),
release tag `faac-2.2`, commit `6acbe23ac9318d8f731ef38e0a56365f70af4797`.

`upstream/` holds that release's portable library sources listed in
`libfaac/meson.build`, their headers, the public `include/faac.h`, and the
license, unmodified. The command-line frontend, SIMD sources, and upstream
build files are omitted. `build.rs` compiles these sources with
`MAX_CHANNELS=2`, `FAAC_SBR_DECIMATION=1`, and `PACKAGE_VERSION` set to the
release version.

## License and source

FAAC is licensed under LGPL-2.1-or-later; see `upstream/COPYING`. Each ABB
release's source archive on
[GitHub Releases](https://github.com/Allmight97/audiobook-boss/releases)
contains this FAAC source and ABB's build scripts. To use a modified FAAC,
edit `vendor/faac-sys/upstream/` in that source and rebuild ABB as its README
describes.
