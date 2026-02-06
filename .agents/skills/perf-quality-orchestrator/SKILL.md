---
name: perf-quality-orchestrator
description: Orchestrate quality + performance validation for Audiobook Boss. Trigger when the user asks for benchmark runs, perf regression checks, baseline comparison, or a concise performance report with recommendations. Runs quick/standard checks, executes selected/all perf benches, compares to baselines, and produces a markdown report. Does not auto-edit code by default.
---

# Perf Quality Orchestrator

Use this skill when the user asks to validate performance health, investigate regressions, or generate a benchmark summary with recommendations.

## Trigger Intents

Trigger this skill for requests like:
- "run perf"
- "benchmark core paths"
- "compare against baseline"
- "is performance getting worse?"
- "generate a perf report"

Do not trigger for unrelated feature implementation unless perf validation is explicitly requested.

## Workflow

1. Confirm scope and mode before execution.
- Inputs: `mode`, `bench_scope` (`all|core3|single`), optional `bench_name`, and `runs`.
- If `bench_scope=single`, require `bench_name`.

2. Run quality gates first.
- Run `scripts/standard-checks.sh` (the default check command).

3. Run performance benchmarks.
- Preferred orchestrator: `bun scripts/perf/run.mjs --mode <mode> --bench-scope <scope> --runs <n> [--bench-name <name>]`.
- `bench_scope=all`: run full suite.
- `bench_scope=core3`: run the core three benchmarks.
- `bench_scope=single`: run only the requested benchmark.

4. Compare to baselines.
- Read generated results from `scripts/perf/results`.
- Compare current metrics to baseline artifacts in the results set (or the team’s declared baseline source).
- Call out significant regressions and improvements explicitly.

5. Generate markdown report with required sections.
- `## Health summary`
- `## Benchmark deltas`
- `## Trend snapshot`
- `## Top 3 recs`

6. Recommend optimizations only when regressions exist.
- Prioritize highest-impact regressions first.
- Give concrete, outcome-focused recommendations (what to change, why it helps, expected impact, validation step).
- No automatic code edits by default. Only edit code if the user explicitly asks.

## Reporting Rules

- Keep report concise, outcome-first, and decision-ready.
- Include exact benchmark names, run counts, and delta percentages.
- If baseline data is missing, state that clearly and provide a "next run" baseline bootstrap recommendation.
- Separate facts from inference.

## Guardrails

- Avoid broad refactors during perf triage unless requested.
- Keep recommendations shippable and scoped.
- If checks fail, report failures first before perf conclusions.
- Do not silently skip requested benchmarks.
