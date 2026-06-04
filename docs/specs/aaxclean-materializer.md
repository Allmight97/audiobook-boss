# AAXClean Materializer

## Outcome

ABB materializes Audible AAX/AAXC acquisitions into validated local M4B files
before FileList handoff. Protected provider files, license material, activation
bytes, keys, IVs, raw license JSON, and helper diagnostics stay behind
`RemoteSourceRuntime`.

## Decisions

- `RemoteSourceRuntime` owns acquisition lifecycle, per-item staging, helper
  process lifetime, cancellation, protected-artifact cleanup, and final import
  validation.
- The first materializer is a bundled `.NET 8` AAXClean sidecar for macOS Apple
  Silicon. ABB's top-level Apache license remains unchanged in this workblock;
  helper source and GPL notices are carried under `tools/abb-aaxclean-helper/`.
- AAX/AAXC are in scope. Dash/Widevine stay unsupported because they require a
  separate CDM, MPD/PSSH, challenge/response, and key-selection subsystem.
- The helper protocol is backend-private. It is not generated into TypeScript
  and does not add a frontend command.
- Supplemental PDFs resolve through the provider-private authenticated
  companion-file route. Audible API `pdf_url` fields are presence hints only;
  ABB must not expose or directly download them as frontend or FileList facts.

## Implementation Targets

- `tools/abb-aaxclean-helper/`: stdin JSON request, NDJSON progress/result/error,
  AAXClean `3.0.2`, `net8.0`, self-contained `osx-arm64` publish.
- `src-tauri/src/remote_source/materializer/`: helper path resolution, child
  process registry, request serialization, stdout parsing, redaction, and
  cancellation cleanup.
- `src-tauri/src/remote_source/providers/audible/`: provider-private license
  material extraction, AAX/AAXC materialization path, and authenticated
  Supplemental PDF resolution/download.
- `scripts/build-app.ts` / helper publish command: generate
  `src-tauri/binaries/abb-aaxclean-helper-aarch64-apple-darwin` before app
  packaging and local install verification.

## Verification

- `dotnet test tools/abb-aaxclean-helper`
- helper publish smoke check for `osx-arm64`
- `cargo nextest run -p abb-media-core`
- `cargo nextest run -p abb-remote-source-core`
- `cargo nextest run -p audiobook-boss --lib`
- `bun run build`
- `bun run app:install-local`

Delete this spec or distill only enduring rules into canon after the PR is
implemented, reviewed, and manually validated.
