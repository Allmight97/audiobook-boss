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

- `upstream/` is an unmodified formal upstream release. Fixes made after a
  release wait for the next release unless one corrects a defect ABB output
  actually shows.
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

## Updating to a new release

- Replace `upstream/` with the release's files for the same slice
  (`include/faac.h`, `include/faad3.h`, `common/`, `libfaac/`, `libfaad/`
  with `imdct_window.inc`, `COPYING`; no SIMD, tests, frontend, or build
  files) and confirm they match the tag byte for byte. Reconcile `build.rs`'s
  source lists and macros with the release's `meson.build` files and
  `meson_options.txt`.
- Set the `faac-sys` package version to the release version; `build.rs` turns
  it into libfaac's runtime version. libfaad3 reports the version in its
  header. Update the identity in `ABB-PROVENANCE.md`.
- Proof: `bash scripts/verify.sh engine media` (the `faac-sys` tests run in
  `engine`, the FAAC and FAAD3 cases in `media`). A changed encoder delay
  fails the adapter's open check; follow the processor guidance before
  changing timing.
