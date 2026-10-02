# Bundled FAAC

## Scope

- Owns the vendored FAAC source slice and its raw FFI (`faac-sys`). The engine
  adapter, MP4 timing, and re-import trimming belong to
  `crates/abb-engine/src/audio/processor/AGENTS.md`.
- `ABB-PROVENANCE.md` ships in the app as the FAAC source notice
  (`src-tauri/tauri.conf.json`). Keep it user-facing: source identity, build
  settings, license, and rebuild route.

## Invariants

- `upstream/` is an unmodified formal upstream release. Fixes made after a
  release wait for the next release unless one corrects a defect ABB output
  actually shows.
- `build.rs` compiles the scalar source list from upstream
  `libfaac/meson.build` and generates bindings from the same `faac.h`, so the
  C library and Rust bindings share one ABI.
- Independent encoder handles may encode concurrently because upstream
  initializes its shared tables once. Leave `FAAC_STATS` undefined: its
  counters are unsynchronized globals.

## Updating to a new release

- Replace `upstream/` with the release's files for the same slice and confirm
  they match the tag byte for byte. Reconcile `build.rs`'s source list and
  settings with the release's `meson.build` and `meson_options.txt`.
- Set the `faac-sys` package version to the release version; `build.rs` turns
  it into the library's runtime version. Update the identity in
  `ABB-PROVENANCE.md`.
- Proof: `cargo nextest run -p faac-sys`, plus the FAAC cases in the engine
  media lane (`scripts/AGENTS.md`). A changed encoder delay fails the
  adapter's open check; follow the processor guidance before changing timing.
