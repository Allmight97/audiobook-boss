# Bundled FAAC and FAAD3

## Scope

- Owns the vendored FAAC source slice (libfaac encoder, libfaad3 decoder, and
  their shared `common/`) and its raw FFI (`faac-sys`). The engine adapters,
  MP4 timing, and re-import trimming belong to
  `crates/abb-engine/src/audio/processor/AGENTS.md`.
- `ABB-PROVENANCE.md` ships in the app as the FAAC and FAAD source notice
  (`src-tauri/tauri.conf.json`). Keep it user-facing: source identity, build
  settings, license, and rebuild route.

## Invariants

- `upstream/` is one unmodified upstream commit: the latest `master` or a
  tagged release. Fix upstream defects by moving the pin, never by editing
  `upstream/`.
- `build.rs` compiles the scalar source lists from upstream's
  `libfaac/meson.build`, `libfaad/meson.build`, and `common/meson.build`, and
  generates bindings from the same `faac.h` and `faad3.h`, so the C libraries
  and Rust bindings share one ABI.
- Each library compiles `common/` under its own `COMMON_PREFIX` (`faac_`,
  `faad_`), as upstream does; one shared copy would link two libraries'
  tables against whichever archive the linker kept.
- The encoder is built for two channels (ABB writes mono or stereo) and
  without PS (HE-AAC v2 output): ABB's FAAC timing knows LC and HE v1 priming
  only. The decoder is built for eight channels, with `coupling.c` and PS, so
  surround and third-party HE v2 sources reach ABB's downmix. The
  `library_info` tests in `src/tests.rs` and `src/faad_tests.rs` pin both
  builds.
- `build.rs` defines every macro upstream's `config.h` would; libfaac reads
  its `FAAC_ENCODER_*` macros as C values, so a missing one fails to compile.
- Independent encoder and decoder handles may run concurrently because
  upstream initializes its shared tables once. Leave `FAAC_STATS` and
  `FAAD_STATS` undefined: their counters are unsynchronized globals.

## Moving the pin

- Replace `upstream/` with the commit's files for the same slice
  (`include/faac.h`, `include/faad3.h`, `common/`, `libfaac/`, `libfaad/`
  with `imdct_window.inc`, `COPYING`; no SIMD, tests, frontend, or build
  files) and confirm they match that commit byte for byte. Reconcile
  `build.rs`'s source lists and macros with its `meson.build` files and
  `meson_options.txt`.
- Set the `faac-sys` package version to the version the commit's
  `meson.build` declares; `build.rs` turns it into libfaac's runtime version.
  libfaad3 reports the version in its header. Update the commit and version
  in `ABB-PROVENANCE.md`.
- Proof: `bash scripts/verify.sh engine media` (the `faac-sys` tests run in
  `engine`, the FAAC and FAAD3 cases in `media`). A changed encoder delay
  fails the adapter's open check; follow the processor guidance before
  changing timing.
