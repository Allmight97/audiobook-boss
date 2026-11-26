# Media pipeline

- This crate owns audio, metadata, processing, execution, and output
  artifacts. It knows nothing of sessions, settings, providers, HTTP, or
  credentials: `abb-engine` passes it facts and paths, and
  `check-rust-tiers.sh` keeps those dependencies out. Hosts reach it only
  through `abb-engine`'s re-exports.
- The logging, lock-poisoning, and test-helper rules in
  `crates/abb-engine/AGENTS.md` hold in this crate too.
- Callers request a metadata outcome; the metadata owner picks MP4 or FFmpeg
  handling from the actual media, never the caller.
- Diagnostic records never change an operation result, and artifact IDs
  correlate paths without exposing parent folders.
- `power::PowerManager` holds idle sleep off through a macOS power assertion
  or a Linux logind inhibitor; without D-Bus it logs and work continues.
  Encoding, metadata saves, Audible acquisition, and Indexer handoff hold an
  `ActiveWork` guard after their scheduler wait and through final writes and
  cleanup; opening ABB and browsing do not.
- Proofs that need private media boundaries live under `src/test_cases`.
  `#[cfg(test)]` items do not cross crates, so `abb-engine`'s tests build
  their media fixtures from this crate's public API instead.

## Outside facts

- `errors.rs` `log_label` values are parsed from captured dev sessions by
  `scripts/dev-log-analysis.ts`; keep them stable.
