## 2026-02-16 — ffmpeg-next Core with CLI Escape-Hatch Policy
Context: Issue #192 requested a durable decision record for engine strategy. The runtime codebase already enforces `FfmpegNextProcessor` as the production path while CLI `ffmpeg` remains in perf tooling for attribution.
Decision: Accept the existing implementation as policy: `ffmpeg-next` is the core production engine; CLI usage is an explicit escape hatch for benchmarking/diagnostics, not a parallel runtime engine.
Consequences:
- Locks in typed progress + structured error UX for unattended batch processing.
- Reduces architectural drift by avoiding dual-engine runtime behavior.
- Preserves CLI benchmarking value without expanding core failure surface.
Links: docs/decisions/004-ffmpeg-next-core-cli-escape-hatch-policy.md, src-tauri/src/audio/processor/selection.rs, src-tauri/src/audio/processor/execute.rs, scripts/perf/benches/audio-processing-throughput.mjs, #192
