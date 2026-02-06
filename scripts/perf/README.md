# Perf System

This directory contains repo-native performance benchmarks for Audiobook Boss.

## Goals

- Provide repeatable local/CI perf signals.
- Support single benchmark and run-all workflows.
- Compare against committed baselines with a 15% warning threshold.
- Track trend history over time in machine + human readable formats.

## Commands

```bash
# List benches
bun scripts/perf/run.mjs --list

# Single benchmark
bun scripts/perf/run.mjs --bench statuspanel-render-lookup --mode synthetic --runs 9

# Run all phase-1 benchmarks, compare to baseline, append history
bun scripts/perf/run.mjs --all --mode synthetic --runs 9 --compare-baseline --append-history

# Compatibility mode for workflow inputs
bun scripts/perf/run.mjs --mode synthetic --bench-scope core3 --runs 5
bun scripts/perf/run.mjs --mode synthetic --bench-scope single --bench-name statuspanel-render-lookup --runs 5

# Seed baseline JSON from current run (intentional use only)
bun scripts/perf/run.mjs --all --mode synthetic --runs 9 --seed-baseline
```

## Modes

- `synthetic`: deterministic local workloads that run quickly and consistently.
- `real`: workload-shaped runs using real files/endpoints when available.

Real mode notes:
- `metadata-lookup-latency` uses network only if `ABB_PERF_ALLOW_NETWORK=1`; otherwise it falls back to synthetic and records fallback reason.
- `audio-processing-throughput` benchmarks encoder-path transcodes (`aac`, `aac_at`, `libfdk_aac`) and reports per-encoder realtime factors.
- `audio-processing-throughput` requires `media/Feedback.m4b` by default and fails fast if missing.
- `audio-processing-throughput` defaults to a 300-second clip for stable and faster comparisons.
- To run full-file encode, set `ABB_PERF_AUDIO_MAX_SECONDS` to the input duration in seconds (example below).
- Real-mode override env vars for `audio-processing-throughput`:
  - `ABB_PERF_AUDIO_INPUT` (relative path from repo root)
  - `ABB_PERF_AUDIO_MAX_SECONDS` (clip duration cap, default `300`)
  - `ABB_PERF_AAC_BITRATE_KBPS` (default `64`)
  - `ABB_PERF_NATIVE_TWOOLOOP` (`1` default, set `0` to disable)
  - `ABB_PERF_FDK_VBR` (`1..5`, default `3`)
  - `ABB_PERF_FDK_AFTERBURNER` (`1` default, set `0` to disable)

Example full-file run for `Feedback.m4b` (~1985s):

```bash
ABB_PERF_AUDIO_MAX_SECONDS=1985 bun scripts/perf/run.mjs --bench audio-processing-throughput --mode real --runs 3
```

## Results

- Latest machine-readable summary: `scripts/perf/results/latest.json`
- Latest markdown summary: `scripts/perf/results/latest.md`
- Historical run rows: `scripts/perf/results/history.ndjson`

## Baselines

- `scripts/perf/baselines/synthetic-main.json`
- `scripts/perf/baselines/real-main.json`

Comparison status semantics:
- `ok`: within threshold.
- `warn`: >15% regression versus baseline.
- `improved`: >15% improvement.
- `missing`: no baseline value.
- `skipped`: benchmark intentionally skipped (phase 2 stubs).

## Result Row Schema

Each benchmark result row includes:

- `bench_name`
- `mode`
- `timestamp`
- `git_sha`, `git_branch`
- `host_os`, `cpu_info`
- `runs`, `warmup_runs`
- `metric_type`, `direction`
- `median`, `p95`, `stddev`
- `baseline_median`, `delta_pct`, `status`
- `details`

## Bench Catalog

Phase 1:
- `statuspanel-render-lookup`
- `statuspanel-event-throughput`
- `metadata-lookup-latency`
- `audio-processing-throughput`

Phase 2 stubs:
- `cancel-latency`
- `cover-art-path`

## Troubleshooting

- Unknown benchmark: run `--list` and use one of the printed names.
- Missing baseline: run with `--seed-baseline` intentionally after validating run quality.
- Network errors in metadata real mode: use `ABB_PERF_ALLOW_NETWORK=1` and verify connectivity.
- FFmpeg failures in real audio mode: ensure `ffmpeg` and `ffprobe` are installed.
