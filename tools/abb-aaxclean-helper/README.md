# ABB AAXClean Helper

Backend-only helper for materializing Audible AAX/AAXC files into M4B files for
AudioBook Boss. ABB invokes this helper from `RemoteSourceRuntime`; the frontend
must never call it directly.

## Protocol

- Request: one JSON object on stdin.
- Response: newline-delimited JSON on stdout.
- Secrets are accepted only through stdin. Do not pass activation bytes, keys,
  IVs, vouchers, license blobs, signed URLs, or raw provider responses in argv,
  environment variables, filenames, stderr, or logs.

## Build

```bash
dotnet test tools/abb-aaxclean-helper
bun run aaxclean-helper:publish
```

`global.json` pins SDK 10.0.401. The publish script picks the RID and Rust
triple from the host (`osx-arm64` / `linux-x64` / `linux-arm64`) and writes
`src-tauri/binaries/abb-aaxclean-helper-<triple>`.

`bun run tauri dev`, Tauri builds, and `scripts/build-app.ts` publish the helper
into `src-tauri/binaries/` before the app resolves or packages the sidecar.

## Licensing

This helper depends on AAXClean `3.1.0`, licensed GPL-3.0. See
`THIRD-PARTY-NOTICES.md`. ABB's top-level license is not changed by this helper
source directory.
