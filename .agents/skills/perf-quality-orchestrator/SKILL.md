---
name: perf-quality-orchestrator
description: Quality + benchmark orchestration for Audiobook Boss. Use for perf regression checks, baseline comparison, and concise decision-ready reports.
---

# Perf Quality Orchestrator

Use this skill when the user requests benchmark execution, baseline comparison, or performance health reporting.

## Inputs

- Mode: `synthetic | real | all`
- Scope: `all | single`
- Optional `bench_name` when `scope=single`
- Optional run count

If mode is omitted, default to `all`.

## Workflow

1. Run quality gate first:
```bash
scripts/checks.sh standard
```
2. Run requested benchmarks:
```bash
bun run perf:all
# or
bun run perf
# or
bun run perf:real
# or
bun scripts/perf/run.mjs --bench <name> --mode <synthetic|real> --runs <n> --compare-baseline --append-history
```
3. Compare against baselines and report deltas.
4. Produce concise report sections:
- `Health summary`
- `UX outcomes`
- `Engineering signal`
- `Benchmark deltas`
- `Trend snapshot`
- `Top 3 recs` (omit when healthy with no action)

## Pointers

- Results: `scripts/perf/results/latest.md`, `scripts/perf/results/latest.json`, `scripts/perf/results/history.ndjson`
- Baselines: `scripts/perf/baselines/*.json`

## Guardrails

- Report failed checks before perf conclusions.
- Do not skip requested benchmarks silently.
- Keep recommendations shippable and scoped.
- Do not auto-edit code unless user asks.

## Done Criteria

- Report includes exact benchmark names, run counts, and delta percentages.
- Regressions/improvements/missing baselines are explicit.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
