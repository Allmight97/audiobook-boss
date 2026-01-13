# btca Resources

Configured resources for this project. See `btca.config.jsonc` for full configuration.

| Resource | Repository | Use Case |
|----------|------------|----------|
| `tauri` | tauri-apps/tauri (dev branch) | Command handlers, event emission, window management |
| `ffmpeg-next` | zmwangx/rust-ffmpeg | Audio encoding, decoding, resampling, frame/packet handling |
| `mp4ameta` | Saecki/rust-mp4ameta | M4B/MP4 metadata, Tag API, FreeformIdent atoms |
| `tokio` | tokio-rs/tokio | spawn_blocking, task cancellation, Semaphore patterns |
| `vitest` | vitest-dev/vitest | Mocking (vi.fn), async tests, JSDOM integration |
| `serde` | serde-rs/serde | Derive macros, field attributes, enum serialization |
| `serde-derive` | serde-rs/serde | Derive macro internals, attribute parsing, codegen |
| `typescript` | microsoft/TypeScript | Type inference, utility types, strict mode behavior |

## Adding New Resources

```bash
btca config resources add -n <name> -t git -u <repo-url> -b <branch>
```

Then update this file and `btca.config.jsonc`.
