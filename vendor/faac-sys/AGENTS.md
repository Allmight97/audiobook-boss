# Bundled FAAC and FAAD3

- `ABB-PROVENANCE.md` ships in the app as the FAAC and FAAD source notice
  (`src-tauri/tauri.conf.json`); keep it user-facing: source identity, build
  settings, license, and rebuild route.
- `upstream/` is one unmodified upstream commit, the latest `master` or a
  tagged release. Fix an upstream defect by moving the pin, never by editing
  `upstream/`. Nothing checks this: the commit is recorded without a checksum.
- `build.rs` takes its source lists from upstream's `meson.build` files and
  its bindings from the same `faac.h` and `faad3.h`, so the C libraries and
  Rust bindings share one ABI.
- Each library compiles `common/` under its own `COMMON_PREFIX` (`faac_`,
  `faad_`), as upstream does, because one shared copy would link both
  libraries against whichever archive's tables the linker kept.
- The encoder is built for two channels and without PS, because ABB's FAAC
  timing knows LC and HE v1 priming only. The decoder keeps eight channels,
  `coupling.c`, and PS, so surround and third-party HE v2 sources reach ABB's
  downmix.
- Leave `FAAC_STATS` and `FAAD_STATS` undefined: their counters are
  unsynchronized globals, and independent handles run concurrently.

## Moving the pin

- Replace `upstream/` with the commit's files for the same slice
  (`include/faac.h`, `include/faad3.h`, `common/`, `libfaac/`, `libfaad/` with
  `imdct_window.inc`, `COPYING`; no SIMD, tests, frontend, or build files) and
  confirm they match that commit byte for byte. Reconcile `build.rs`'s source
  lists and macros with its `meson.build` files and `meson_options.txt`.
- Set the `faac-sys` package version to the version the commit's
  `meson.build` declares, and update the commit and version in
  `ABB-PROVENANCE.md`.
- A changed encoder delay fails the FAAC adapter's open check; read
  `crates/abb-engine/src/audio/processor/AGENTS.md` before changing timing.
