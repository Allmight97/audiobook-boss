#!/usr/bin/env bun

import os from "node:os";
import { resolve } from "node:path";

import {
  BENCHMARKS,
  BENCHMARKS_BY_NAME,
  CORE3_BENCH_NAMES,
  PHASE1_BENCH_NAMES,
  PHASE2_BENCH_NAMES,
} from "./benches/index.mjs";
import {
  applyBaselineComparison,
  loadBaseline,
  seedBaselineFromResults,
} from "./compare.mjs";
import {
  appendNdjson,
  ensureDir,
  REPO_ROOT,
  RESULTS_DIR,
  writeJson,
  writeText,
  readNdjson,
} from "./shared/io.mjs";
import { getGitInfo } from "./shared/git.mjs";
import { summarizeSamples } from "./shared/stats.mjs";
import { buildLatestMarkdown } from "./trends.mjs";

const HISTORY_PATH = resolve(RESULTS_DIR, "history.ndjson");
const LATEST_JSON_PATH = resolve(RESULTS_DIR, "latest.json");
const LATEST_MD_PATH = resolve(RESULTS_DIR, "latest.md");

function parseArgs(argv) {
  const parsed = {
    list: false,
    all: false,
    bench: null,
    mode: "synthetic",
    runs: 5,
    warmupRuns: null,
    compareBaseline: false,
    appendHistory: false,
    seedBaseline: false,
    includePhase2: false,
    benchScope: null,
    benchName: null,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--list") {
      parsed.list = true;
      continue;
    }
    if (arg === "--all") {
      parsed.all = true;
      continue;
    }
    if (arg === "--bench") {
      parsed.bench = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--mode") {
      parsed.mode = argv[i + 1] ?? parsed.mode;
      i += 1;
      continue;
    }
    if (arg === "--runs") {
      parsed.runs = Number.parseInt(argv[i + 1] ?? "", 10);
      i += 1;
      continue;
    }
    if (arg === "--warmup-runs") {
      parsed.warmupRuns = Number.parseInt(argv[i + 1] ?? "", 10);
      i += 1;
      continue;
    }
    if (arg === "--compare-baseline") {
      parsed.compareBaseline = true;
      continue;
    }
    if (arg === "--append-history") {
      parsed.appendHistory = true;
      continue;
    }
    if (arg === "--seed-baseline") {
      parsed.seedBaseline = true;
      continue;
    }
    if (arg === "--include-phase2") {
      parsed.includePhase2 = true;
      continue;
    }
    if (arg === "--bench-scope") {
      parsed.benchScope = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--bench-name") {
      parsed.benchName = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!["synthetic", "real"].includes(parsed.mode)) {
    throw new Error(`Invalid mode '${parsed.mode}'. Use 'synthetic' or 'real'.`);
  }
  if (!Number.isInteger(parsed.runs) || parsed.runs <= 0) {
    throw new Error(`Invalid --runs '${parsed.runs}'.`);
  }
  if (
    parsed.warmupRuns !== null &&
    (!Number.isInteger(parsed.warmupRuns) || parsed.warmupRuns < 0)
  ) {
    throw new Error(`Invalid --warmup-runs '${parsed.warmupRuns}'.`);
  }

  return parsed;
}

function printHelp() {
  console.log(`Usage:
  bun scripts/perf/run.mjs --list
  bun scripts/perf/run.mjs --bench <name> --mode synthetic --runs 9
  bun scripts/perf/run.mjs --all --mode synthetic --runs 9 --compare-baseline --append-history

Options:
  --list                    List available benchmarks.
  --bench <name>            Run a single benchmark by name.
  --all                     Run all phase-1 benchmarks.
  --mode <synthetic|real>   Benchmark mode.
  --runs <n>                Number of measured runs per benchmark.
  --warmup-runs <n>         Override warmup runs.
  --compare-baseline        Compare against scripts/perf/baselines/<mode>-main.json.
  --append-history          Append each benchmark summary to history.ndjson.
  --seed-baseline           Write baseline file from this run.
  --include-phase2          Include phase-2 benchmark entries.

Compatibility flags:
  --bench-scope <all|core3|single>
  --bench-name <name>
`);
}

function resolveBenchSelection(parsed) {
  if (parsed.benchScope) {
    if (parsed.benchScope === "single") {
      const name = parsed.benchName ?? parsed.bench;
      if (!name) {
        throw new Error("bench_scope=single requires --bench-name or --bench.");
      }
      return [name];
    }
    if (parsed.benchScope === "core3") {
      return CORE3_BENCH_NAMES;
    }
    if (parsed.benchScope === "all") {
      return parsed.includePhase2
        ? [...PHASE1_BENCH_NAMES, ...PHASE2_BENCH_NAMES]
        : PHASE1_BENCH_NAMES;
    }
    throw new Error(`Unsupported --bench-scope '${parsed.benchScope}'.`);
  }

  if (parsed.bench) {
    return [parsed.bench];
  }

  if (parsed.all) {
    return parsed.includePhase2
      ? [...PHASE1_BENCH_NAMES, ...PHASE2_BENCH_NAMES]
      : PHASE1_BENCH_NAMES;
  }

  throw new Error("Select benchmarks with --bench <name> or --all.");
}

function listBenchmarks() {
  console.log("Available benchmarks:");
  BENCHMARKS.forEach((bench) => {
    console.log(
      `- ${bench.name} (phase ${bench.phase}) :: ${bench.metricType}, ${bench.direction}`
    );
  });
}

async function runBenchmark(bench, config) {
  const warmupRuns =
    config.warmupRuns !== null ? config.warmupRuns : bench.warmupRuns ?? 1;

  for (let i = 0; i < warmupRuns; i += 1) {
    await bench.run({ mode: config.mode, repoRoot: REPO_ROOT });
  }

  const samples = [];
  const details = [];

  for (let i = 0; i < config.runs; i += 1) {
    const output = await bench.run({ mode: config.mode, repoRoot: REPO_ROOT });

    if (output?.skipped) {
      return {
        samples: [],
        details: [],
        warmupRuns,
        skipped: true,
        skipReason: output.reason ?? "Skipped by benchmark.",
      };
    }

    const value = typeof output === "number" ? output : output?.value;
    if (!Number.isFinite(value)) {
      throw new Error(`${bench.name} produced non-numeric value on run ${i + 1}.`);
    }

    samples.push(value);
    if (output?.details) {
      details.push(output.details);
    }
  }

  return {
    samples,
    details,
    warmupRuns,
    skipped: false,
    skipReason: null,
  };
}

function toResultRow({ bench, summary, env, mode, runs, warmupRuns, details }) {
  return {
    bench_name: bench.name,
    mode,
    timestamp: summary.timestamp,
    git_sha: env.git.commit,
    git_branch: env.git.branch,
    host_os: env.host.os,
    cpu_info: env.host.cpu,
    runs,
    warmup_runs: warmupRuns,
    metric_type: bench.metricType,
    direction: bench.direction,
    median: Number.isFinite(summary.stats?.median)
      ? Number(summary.stats.median.toFixed(3))
      : null,
    p95: Number.isFinite(summary.stats?.p95)
      ? Number(summary.stats.p95.toFixed(3))
      : null,
    stddev: Number.isFinite(summary.stats?.stddev)
      ? Number(summary.stats.stddev.toFixed(3))
      : null,
    baseline_median: null,
    delta_pct: null,
    status: "ok",
    threshold_pct: 15,
    details,
  };
}

function toSkippedRow({ bench, env, mode, runs, warmupRuns, reason, timestamp }) {
  return {
    bench_name: bench.name,
    mode,
    timestamp,
    git_sha: env.git.commit,
    git_branch: env.git.branch,
    host_os: env.host.os,
    cpu_info: env.host.cpu,
    runs,
    warmup_runs: warmupRuns,
    metric_type: bench.metricType,
    direction: bench.direction,
    median: null,
    p95: null,
    stddev: null,
    baseline_median: null,
    delta_pct: null,
    status: "skipped",
    threshold_pct: 15,
    details: { reason },
  };
}

function printRunSummary(results) {
  console.log("\nPerformance summary:\n");
  for (const row of results) {
    const median = Number.isFinite(row.median) ? row.median.toFixed(3) : "n/a";
    const p95 = Number.isFinite(row.p95) ? row.p95.toFixed(3) : "n/a";
    const delta = Number.isFinite(row.delta_pct) ? `${row.delta_pct.toFixed(2)}%` : "n/a";
    console.log(
      `${row.bench_name} | median=${median} | p95=${p95} | delta=${delta} | status=${row.status}`
    );
  }
}

async function main() {
  const parsed = parseArgs(process.argv.slice(2));

  if (parsed.list) {
    listBenchmarks();
    return;
  }

  await ensureDir(RESULTS_DIR);

  const selectedNames = resolveBenchSelection(parsed);
  const selectedBenches = selectedNames.map((name) => {
    const bench = BENCHMARKS_BY_NAME.get(name);
    if (!bench) {
      throw new Error(`Unknown benchmark '${name}'. Use --list to inspect available names.`);
    }
    return bench;
  });

  const git = getGitInfo(REPO_ROOT);
  const host = {
    os: `${os.platform()} ${os.release()}`,
    cpu: os.cpus()?.[0]?.model ?? "unknown cpu",
  };

  const results = [];
  const timestamp = new Date().toISOString();

  for (const bench of selectedBenches) {
    console.log(`Running ${bench.name} (${parsed.mode}) ...`);

    const runData = await runBenchmark(bench, parsed);

    if (runData.skipped) {
      results.push(
        toSkippedRow({
          bench,
          env: { git, host },
          mode: parsed.mode,
          runs: parsed.runs,
          warmupRuns: runData.warmupRuns,
          reason: runData.skipReason,
          timestamp,
        })
      );
      console.log(`  skipped: ${runData.skipReason}`);
      continue;
    }

    const stats = summarizeSamples(runData.samples);
    const row = toResultRow({
      bench,
      summary: { timestamp, stats },
      env: { git, host },
      mode: parsed.mode,
      runs: parsed.runs,
      warmupRuns: runData.warmupRuns,
      details: {
        samples: runData.samples.map((value) => Number(value.toFixed(6))),
        latest: runData.details.at(-1) ?? null,
      },
    });

    results.push(row);
    console.log(
      `  median=${row.median?.toFixed(3) ?? "n/a"} ${row.metric_type} | p95=${
        row.p95?.toFixed(3) ?? "n/a"
      }`
    );
  }

  const warnThresholdPct = 15;
  let finalResults = results;

  if (parsed.compareBaseline) {
    const baseline = await loadBaseline(parsed.mode);
    finalResults = applyBaselineComparison(results, baseline, warnThresholdPct);
  }

  if (parsed.seedBaseline) {
    const seeded = await seedBaselineFromResults({
      mode: parsed.mode,
      results: finalResults,
      warnThresholdPct,
    });
    console.log(`Seeded baseline: ${seeded.path}`);
  }

  const payload = {
    version: 1,
    timestamp,
    mode: parsed.mode,
    runs: parsed.runs,
    warmup_runs: parsed.warmupRuns,
    git_branch: git.branch,
    git_sha: git.commit,
    git_dirty: git.dirty,
    host_os: host.os,
    cpu_info: host.cpu,
    warn_threshold_pct: warnThresholdPct,
    results: finalResults,
  };

  await writeJson(LATEST_JSON_PATH, payload);

  if (parsed.appendHistory) {
    for (const row of finalResults) {
      await appendNdjson(HISTORY_PATH, row);
    }
  }

  const historyRows = await readNdjson(HISTORY_PATH);
  const latestMd = buildLatestMarkdown({
    summary: payload,
    latestRows: finalResults,
    historyRows,
  });
  await writeText(LATEST_MD_PATH, latestMd);

  printRunSummary(finalResults);

  const hasWarnings = finalResults.some((result) => result.status === "warn");
  process.exitCode = hasWarnings ? 1 : 0;
}

main().catch((error) => {
  console.error(`Perf runner failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
