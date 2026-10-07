# Bundled FAAC

## Scope

- Owns the vendored FAAC source slice (encoder `libfaac`, decoder `libfaad`,
  shared `common`) and its raw FFI (`faac-sys`). The engine adapters, MP4
  timing, and re-import trimming belong to
  `crates/abb-engine/src/audio/processor/AGENTS.md`.
- `ABB-PROVENANCE.md` ships in the app as the FAAC and FAAD source notice
  (`src-tauri/tauri.conf.json`). Keep it user-facing: source identity, build
  settings, license, and rebuild route.

## Invariants

- `upstream/` is unmodified upstream source from one commit. The `FAAD3` branch
  tracks FAAC pull request 29 to review FAAD3 before upstream tags it; `main`
  takes only formal releases. Fixes made after a release wait for the next
  release unless one corrects a defect ABB output actually shows.
- Encoder and decoder come from the same commit and compile `common/` once: a
  static link of two copies keeps only one, so they must not drift apart.
- `build.rs` compiles the scalar source lists from upstream
  `libfaac/meson.build`, `libfaad/meson.build` and `common/meson.build`, and
  generates bindings from the same `faac.h` and `faad.h`, so the C library and
  Rust bindings share one ABI. It defines `WORDS_BIGENDIAN`: the sources have
  no default for it.
- Independent encoder handles may encode concurrently because upstream
  initializes its shared tables once. Leave `FAAC_STATS` undefined: its
  counters are unsynchronized globals. The same holds for FAAD handles and
  `FAAD_STATS`.

## Updating to a new release

- Replace `upstream/` with the release's files for the same slice and confirm
  they match the tag byte for byte. Reconcile `build.rs`'s source list and
  settings with the release's `libfaac/meson.build` and `meson_options.txt` in
  the upstream repository (`upstream/` omits both).
- Set the `faac-sys` package version to the release version; `build.rs` turns
  it into the library's runtime version. Update the identity in
  `ABB-PROVENANCE.md`.
- Proof: `cargo test --locked -p faac-sys` (`faad_tests.rs` drives FAAD with
  this crate's FAAC output, including ABB's HE timing constants), plus the
  FAAC cases in the engine media lane (`scripts/AGENTS.md`). A changed encoder delay fails the
  adapter's open check; follow the processor guidance before changing timing.
