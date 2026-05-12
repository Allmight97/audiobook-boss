# Unsafe Code Register

This is a small learning and guardrail surface for Rust `unsafe` in Audiobook
Boss. It is not a ban list. It exists so unsafe code remains easy to find,
easy to explain, and hard to widen casually.

Use this register when production Rust unsafe code is added, removed, or
materially changed. Keep entries concise: purpose, owner, blast radius, and the
invariant that keeps the unsafe operation contained.

## How To Read This

In this repo, current production `unsafe` is concentrated around FFmpeg and
FFmpeg-backed wrappers:

- raw FFmpeg pointers not exposed by `ffmpeg-next`
- allocated FFmpeg audio frame buffers
- byte-to-sample slice views for packed audio
- encoder context fields and AVOptions
- copied metadata or codec side data

The important question is not "is unsafe bad?" The useful question is: what
library boundary forced it, what must stay true around it, and what user-visible
behavior can break if that invariant is wrong?

## Agent Rules

- Before changing production `unsafe`, identify the owning boundary in this
  file and the nearest nested `AGENTS.md`.
- Prefer a safe wrapper API when `ffmpeg-next` or a local helper already exposes
  one.
- Keep unsafe blocks as small as practical and close to pointer/slice creation.
- Add or preserve a `SAFETY:` comment when the invariant is not obvious from the
  surrounding code.
- Update this register when production unsafe scope, purpose, or blast radius
  changes.
- Do not expand unsafe code as a compatibility shim without the normal fallback
  evidence, signal, and sunset discipline from `AGENTS.md`.

## Current Production Unsafe

| Area | Representative locations | Purpose | Blast radius | Containing invariant |
| --- | --- | --- | --- | --- |
| Metadata attached-picture reads | `src-tauri/src/metadata/reader.rs` `extract_attached_pic`; `src-tauri/src/metadata/passthrough.rs` `extract_attached_pic` | Read FFmpeg `AVStream.attached_pic` bytes and immediately copy them into owned `Vec<u8>`. | Cover-art readback, passthrough cover art, metadata inspection accuracy. | Only read streams marked `ATTACHED_PIC`; require non-null data and positive size; copy before returning. |
| Metadata stream disposition writes | `src-tauri/src/metadata/ffi.rs` `set_attached_pic_disposition`; `set_stream_disposition_and_clear_codec_tag` | Set `ATTACHED_PIC`, stream disposition bits, and clear codec tags not exposed by safe `ffmpeg-next` APIs. | Cover-art embedding compatibility and container metadata writes. | Validate format/stream pointers and stream index before mutation; preserve or deliberately set the expected disposition. |
| Audio frame allocation and sample slices | `src-tauri/src/audio/buffer.rs`; `src-tauri/src/audio/processor/frame_pipeline.rs` | Allocate FFmpeg audio frames and create typed views over packed audio buffers. | Channel order, sample integrity, frame/tail handling, resampling correctness. | Frame format, channel layout, rate, and sample count are set before allocation; slice lengths are derived from frame/sample counts and format width. |
| Encoder registry and AVOptions | `src-tauri/src/audio/settings_encoder.rs`; `src-tauri/src/audio/processor/encoder/common.rs`; encoder option modules | Query FFmpeg encoders by C name and set encoder context options such as `strict`, `threads`, and bitrate fields. | Encoder availability detection, AAC mode behavior, bitrate behavior, encode startup. | C strings are validated; null codec/context pointers are checked where applicable; failures are converted to errors or logged fallbacks according to the owning encoder path. |
| Encoder context observation | `src-tauri/src/audio/processor/encoder/context.rs` | Read raw FFmpeg encoder context fields for logging/diagnostics. | Diagnostics only unless later code starts depending on logged values. | The context has been opened before the raw field read; keep this read diagnostic, not behavioral. |
| Codec extradata copy | `src-tauri/src/audio/processor/streams.rs` `read_codec_extradata` | Copy codec extradata for AAC profile/probe decisions when safe wrapper coverage is insufficient. | Decoder selection and xHE-AAC/AAC routing. | Read only during the lifetime of codec parameters; require non-null data and positive size; copy into owned memory immediately. |

## Non-Production Sightings

These are tracked at summary level so the register stays useful:

- `src-tauri/tests/**` and inline `#[cfg(test)]` modules under `src-tauri/src/**`:
  unsafe frame allocation and raw sample slice helpers are used to build FFmpeg
  test frames and assert sample contents.
- `vendor/ffmpeg-sys-next-8.1.0/**`: unsafe C wrapper functions belong to the
  vendored FFmpeg sys layer. Treat this as dependency/sys surface unless local
  production code calls a wrapper directly.
- `src/AGENTS.md`: "unsafe `any` propagation" is TypeScript lint language, not
  Rust unsafe code.

## Refresh Command

Use this from the repo root when refreshing the register:

```bash
rg -n "\bunsafe\b" src-tauri/src src-tauri/tests vendor/ffmpeg-sys-next-8.1.0/src src/AGENTS.md --glob '!target'
```

Then classify hits as:

- production Rust unsafe
- test helper unsafe
- vendored/sys unsafe
- non-Rust or false-positive language

## Current Posture

The current unsafe surface is expected for a desktop audio app that touches
FFmpeg directly. The risk to watch is not that unsafe exists; the risk is unsafe
logic spreading upward into product orchestration, UI contracts, fallback policy,
or path/output decisions where ABB already has owned safe boundaries.
