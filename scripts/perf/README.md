# Perf System

## How It Works (mental model)

You have 4 benchmarks. Each measures something users actually feel:

| Benchmark | What It Answers |
|---|---|
| `statuspanel-render-lookup` | Does the progress panel stay snappy as queues grow? |
| `statuspanel-event-throughput` | Can the UI keep up with rapid progress events? |
| `metadata-lookup-latency` | How fast does metadata resolve when adding books? |
| `audio-processing-throughput` | How fast do audiobooks actually encode? |

Each benchmark runs in two modes:
- **synthetic** — fast, deterministic, pure-JS. Great for quick checks.
- **real** — uses actual files and encoders. Shows true user-facing performance.

Results compare against committed baselines (15% threshold):
- **OK** = users won't notice a difference
- **WARN** = users would feel this regression
- **IMPROVED** = ship it, things got faster

---

## Quick Start

```bash
bun run perf            # Full synthetic sweep (the default go-to)
bun run perf:audio      # Real audio encode test
bun run perf:real       # All benchmarks, real mode
bun run perf:quick      # Fast 3-run gut check
bun run perf:list       # What benchmarks exist + what they measure
```

---

## Modes

### Synthetic Mode

Deterministic local workloads that run quickly and consistently. Pure CPU/memory ops, no external dependencies (no ffmpeg, no network).

Use synthetic mode for:
- Pre-commit checks
- Iteration loops during development
- Fast baseline comparisons

### Real Mode

Workload-shaped runs using real files/endpoints when available.

**Notes:**
- `metadata-lookup-latency` uses network only if `ABB_PERF_ALLOW_NETWORK=1`; otherwise it falls back to synthetic and records fallback reason.
- `audio-processing-throughput` benchmarks encoder-path transcodes (`aac`, `aac_at`, `libfdk_aac`) and reports per-encoder realtime factors.
- `audio-processing-throughput` requires `media/Feedback.m4b` by default and fails fast if missing.
- `audio-processing-throughput` defaults to a 300-second clip for stable and faster comparisons.
- To run full-file encode, set `ABB_PERF_AUDIO_MAX_SECONDS` to the input duration in seconds.

---

## Results

- **Latest markdown summary**: `scripts/perf/results/latest.md`
- **Latest machine-readable summary**: `scripts/perf/results/latest.json`
- **Historical run rows**: `scripts/perf/results/history.ndjson`

The markdown summary includes:
- **Performance Matrix** — translates raw metrics into user-facing outcomes
- **Encoder Breakdown** (real audio mode only) — per-encoder speed/throughput
- **Technical Detail** — raw bench/metric/median/p95 numbers
- **Trend Snapshot** — ASCII sparklines from the last 12 history entries

---

## Baselines

- `scripts/perf/baselines/synthetic-main.json`
- `scripts/perf/baselines/real-main.json`

Comparison status semantics:
- `ok`: within threshold.
- `warn`: >15% regression versus baseline.
- `improved`: >15% improvement.
- `missing`: no baseline value.
- `skipped`: benchmark intentionally skipped (phase 2 stubs).

---

## Advanced Usage

### Override Real-Mode Audio Settings

Environment variables for `audio-processing-throughput`:
- `ABB_PERF_AUDIO_INPUT` (relative path from repo root, default: `media/Feedback.m4b`)
- `ABB_PERF_AUDIO_MAX_SECONDS` (clip duration cap, default: `300`)
- `ABB_PERF_AAC_BITRATE_KBPS` (default: `64`)
- `ABB_PERF_NATIVE_TWOOLOOP` (`1` default, set `0` to disable)
- `ABB_PERF_FDK_VBR` (`1..5`, default: `3`)
- `ABB_PERF_FDK_AFTERBURNER` (`1` default, set `0` to disable)

Example full-file run for `Feedback.m4b` (~1985s):

```bash
ABB_PERF_AUDIO_MAX_SECONDS=1985 bun scripts/perf/run.mjs --bench audio-processing-throughput --mode real --runs 3
```

### Manual Invocation

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

---

## Benchmark Catalog

### Phase 1 (Active)

**`statuspanel-render-lookup`**
- **What it measures**: Progress panel render ordering lookup hot-path (O(n) vs O(1))
- **User impact**: Progress panel stays smooth when processing large queues
- **Metric**: `duration_ms` (lower is better)
- **Target**: Under 16ms frame budget

**`statuspanel-event-throughput`**
- **What it measures**: End-to-end progress event handling throughput
- **User impact**: UI handles rapid progress updates without stuttering during batch jobs
- **Metric**: `events_per_second` (higher is better)

**`metadata-lookup-latency`**
- **What it measures**: Metadata lookup pipeline latency
- **User impact**: Book metadata resolves near-instantly when adding files to the library
- **Metric**: `duration_ms` (lower is better)

**`audio-processing-throughput`**
- **What it measures**: Audio DSP (synthetic) and encoder throughput (real)
- **User impact**: Audiobooks encode fast — a 33-min book should finish in seconds, not minutes
- **Metric**: `realtime_factor` (real) / `throughput_mib_per_s` (synthetic) — higher is better

### Phase 2 (Stubs)

- `cancel-latency` — Job cancellation responsiveness (not implemented)
- `cover-art-path` — Cover art read/render path (not implemented)

---

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

---

## Troubleshooting

- **Unknown benchmark**: run `bun run perf:list` and use one of the printed names.
- **Missing baseline**: run with `--seed-baseline` intentionally after validating run quality.
- **Network errors in metadata real mode**: use `ABB_PERF_ALLOW_NETWORK=1` and verify connectivity.
- **FFmpeg failures in real audio mode**: ensure `ffmpeg` and `ffprobe` are installed and on PATH.
