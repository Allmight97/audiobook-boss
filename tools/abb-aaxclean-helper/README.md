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
dotnet publish tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/AbbAaxcleanHelper.csproj \
  -c Release -f net8.0 -r osx-arm64 --self-contained true \
  -p:PublishSingleFile=true -p:PublishTrimmed=false
```

`bun run tauri dev`, Tauri builds, and `scripts/build-app.ts` publish the helper
into `src-tauri/binaries/` before the app resolves or packages the sidecar.

## Licensing

This helper depends on AAXClean `3.0.2`, licensed GPL-3.0. See
`THIRD-PARTY-NOTICES.md`. ABB's top-level license is not changed by this helper
source directory.
