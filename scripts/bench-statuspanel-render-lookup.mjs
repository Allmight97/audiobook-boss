#!/usr/bin/env bun

/**
 * Back-compat shim for the migrated perf benchmark.
 *
 * Prefer:
 *   bun scripts/perf/run.mjs --bench statuspanel-render-lookup --mode synthetic --runs 9
 */

import { runLookupBenchmark } from "./perf/benches/statuspanel-render-lookup.mjs";

const args = process.argv.slice(2);

function readNumericArg(flag, defaultValue) {
  const idx = args.indexOf(flag);
  if (idx === -1) return defaultValue;
  const raw = args[idx + 1];
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`Invalid value for ${flag}: ${raw}`);
  }
  return Math.floor(parsed);
}

const queue = readNumericArg("--queue", 500);
const extras = readNumericArg("--extras", 120);
const loops = readNumericArg("--loops", 2000);
const asJson = args.includes("--json");

const result = runLookupBenchmark({ mode: "synthetic", queue, extras, loops });

if (asJson) {
  console.log(JSON.stringify(result));
} else {
  console.log("StatusPanel render lookup benchmark");
  console.log(`queue=${result.queue}, extras=${result.extras}, loops=${result.loops}`);
  console.log(`old(indexOf): ${result.old_ms}ms`);
  console.log(`new(Map):     ${result.new_ms}ms`);
  console.log(`speedup:      ${result.speedup}x`);
  console.log(`equivalent:   ${result.equivalent ? "yes" : "no"}`);
}

if (!result.equivalent) {
  process.exitCode = 1;
}
