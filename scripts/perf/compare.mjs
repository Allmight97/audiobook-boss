import { resolve } from "node:path";

import { BASELINES_DIR, readJson, writeJson } from "./shared/io.mjs";
import { computeComparison } from "./shared/stats.mjs";

export function baselinePathForMode(mode) {
  return resolve(BASELINES_DIR, `${mode}-main.json`);
}

export async function loadBaseline(mode) {
  return readJson(baselinePathForMode(mode), null);
}

export function applyBaselineComparison(results, baseline, warnThresholdPct = 15) {
  return results.map((result) => {
    if (result.status === "skipped") {
      return {
        ...result,
        baseline_median: null,
        delta_pct: null,
      };
    }

    const baselineEntry = baseline?.benchmarks?.[result.bench_name];
    const baselineMedian = baselineEntry?.median;
    const lowerIsBetter = result.direction === "lower_is_better";

    const comparison = computeComparison({
      currentValue: result.median,
      baselineValue: baselineMedian,
      lowerIsBetter,
      warnThresholdPct,
    });

    return {
      ...result,
      baseline_median:
        Number.isFinite(comparison.baseline_value) ? comparison.baseline_value : null,
      delta_pct: Number.isFinite(comparison.delta_pct)
        ? Number(comparison.delta_pct.toFixed(3))
        : null,
      status: comparison.status,
      threshold_pct: warnThresholdPct,
    };
  });
}

export async function seedBaselineFromResults({ mode, results, warnThresholdPct = 15 }) {
  const benchmarks = {};
  for (const result of results) {
    if (!Number.isFinite(result.median)) continue;

    benchmarks[result.bench_name] = {
      metric_type: result.metric_type,
      direction: result.direction,
      median: Number(result.median.toFixed(3)),
      p95: Number.isFinite(result.p95) ? Number(result.p95.toFixed(3)) : null,
      stddev: Number.isFinite(result.stddev) ? Number(result.stddev.toFixed(3)) : null,
      updated_at: result.timestamp,
      git_sha: result.git_sha,
    };
  }

  const payload = {
    version: 1,
    mode,
    warn_threshold_pct: warnThresholdPct,
    updated_at: new Date().toISOString(),
    benchmarks,
  };

  const path = baselinePathForMode(mode);
  await writeJson(path, payload);
  return { path, payload };
}
