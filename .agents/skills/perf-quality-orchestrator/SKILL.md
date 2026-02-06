---
name: perf-quality-orchestrator
description: Orchestrate quality + performance validation for Audiobook Boss. Trigger when the user asks for benchmark runs, perf regression checks, baseline comparison, or a concise performance report with recommendations. Runs standard checks, executes selected/all perf benches, compares to baselines, and produces a markdown report that includes both UX outcomes and engineering signals. Does not auto-edit code by default.
---

# Perf Quality Orchestrator

Use this skill when the user asks to validate performance health, investigate regressions, or generate a benchmark summary with recommendations.

## Trigger Intents

Trigger this skill for requests like:
- "run perf"
- "run perf:all"
- "benchmark core paths"
- "compare against baseline"
- "is performance getting worse?"
- "generate a perf report"
- "show engineering signal"
- "show matrix + technical detail"

Do not trigger for unrelated feature implementation unless perf validation is explicitly requested.

## Workflow

1. Confirm scope and mode before execution.
- Inputs: `mode` (`synthetic|real|all`), `scope` (`all|single`), optional `bench_name`, and `runs`.
- If `scope=single`, require `bench_name`.
- If mode is not explicitly specified, default to `all` by running `perf:all`.

2. Run quality gates first.
- Run `scripts/standard-checks.sh` (the default check command).

3. Run performance benchmarks.
- Preferred default:
  - `bun run perf:all` (canonical all-mode path: synthetic + real in one flow; still runs real even if synthetic warns).
- Single-benchmark path:
  - `bun scripts/perf/run.mjs --bench <name> --mode <synthetic|real> --runs <n> --compare-baseline --append-history`
- Full synthetic only:
  - `bun run perf`
- Full real only:
  - `bun run perf:real`

4. Compare to baselines.
- Read generated results from `scripts/perf/results`.
- Compare current metrics to baseline artifacts in the results set (or the team’s declared baseline source).
- Call out significant regressions and improvements explicitly.

5. Generate markdown report with required sections.
- `## Health summary`
- `## UX outcomes`
- `## Engineering signal`
- `## Benchmark deltas`
- `## Trend snapshot`
- `## Top 3 recs`

6. Assess and recommend.
- All `ok` / `improved`: state "Performance healthy — no action needed." Note any `improved` results. Skip `## Top 3 recs`.
- Any `warn`: prioritize highest-impact regressions. Give concrete recommendations (what to change, why, expected impact, validation step).
- Any `missing`: recommend baseline bootstrap (`--seed-baseline`).
- No automatic code edits by default. Only edit if user explicitly asks.

## Reporting Rules

- Keep report concise, outcome-first, and decision-ready.
- Include exact benchmark names, run counts, and delta percentages.
- Metric definitions for attribution:
  - `rtf_cli`: encoder-only realtime factor from `audio-processing-throughput` (real mode).
  - `rtf_app`: app end-to-end realtime factor from `audio-processing-app-e2e` (real mode).
  - `overhead_ratio`: `(rtf_cli - rtf_app) / rtf_cli`.
    - `>30%` high app-side overhead, `>15%` moderate, `>5%` low, `<=5%` near encoder baseline.
- Data sources (use the right one for the task):
  - `scripts/perf/results/latest.md` — Performance Matrix (UX outcomes), Technical Detail table (engineering signal), trend sparklines. Start here for reports.
  - `scripts/perf/results/latest.json` — Structured results: `median`, `p95`, `stddev`, `delta_pct`, `status`, raw `samples[]`, encoder `details`. Use for programmatic analysis and regression root-cause.
  - `scripts/perf/results/latest-{mode}.json` — Per-mode snapshots (e.g. `latest-synthetic.json`). Use when isolating one mode.
  - `scripts/perf/results/history.ndjson` — Historical rows for trend analysis and drift detection.
  - `scripts/perf/baselines/{mode}-main.json` — Committed baselines (comparison source for deltas).
- Status semantics (15% threshold):
  - `ok` — within threshold, users won't notice
  - `warn` — >15% regression, users would feel this
  - `improved` — >15% improvement, ship it
  - `missing` — no baseline for this bench/mode
  - `skipped` — benchmark intentionally skipped
- If baseline data is missing, state that clearly and provide a "next run" baseline bootstrap recommendation.
- Separate facts from inference.

## Guardrails

- Avoid broad refactors during perf triage unless requested.
- Keep recommendations shippable and scoped.
- If checks fail, report failures first before perf conclusions.
- Do not silently skip requested benchmarks.
