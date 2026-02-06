#!/usr/bin/env bun

/**
 * Benchmarks queue-position lookup strategies used by statusPanel render flow.
 *
 * This is a micro-benchmark for algorithmic hot-path comparison:
 * - old: repeated Array#indexOf lookups
 * - new: precomputed Map lookup
 *
 * Usage:
 *   bun scripts/bench-statuspanel-render-lookup.mjs
 *   bun scripts/bench-statuspanel-render-lookup.mjs --queue 500 --extras 120 --loops 2000 --json
 */

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

const queueSize = readNumericArg("--queue", 500);
const extraSize = readNumericArg("--extras", 120);
const loops = readNumericArg("--loops", 2000);
const asJson = args.includes("--json");

const queueOrder = Array.from({ length: queueSize }, (_, i) => `idx:${i}`);
const extras = Array.from({ length: extraSize }, (_, i) => `extra:${i}`);
const orderedKeys = [...queueOrder, ...extras];

function oldLookup() {
  let sum = 0;
  for (let i = 0; i < loops; i += 1) {
    for (const key of orderedKeys) {
      const position = queueOrder.length > 0 ? queueOrder.indexOf(key) + 1 : 0;
      if (position > 0) sum += position;
    }
  }
  return sum;
}

function newLookup() {
  let sum = 0;
  const queuePositions = new Map(
    queueOrder.map((key, index) => [key, index + 1])
  );
  for (let i = 0; i < loops; i += 1) {
    for (const key of orderedKeys) {
      const position = queuePositions.get(key) ?? 0;
      if (position > 0) sum += position;
    }
  }
  return sum;
}

const t1 = performance.now();
const oldSum = oldLookup();
const t2 = performance.now();
const newSum = newLookup();
const t3 = performance.now();

const oldMs = t2 - t1;
const newMs = t3 - t2;
const speedup = newMs === 0 ? Infinity : oldMs / newMs;
const equivalent = oldSum === newSum;

const result = {
  queue: queueSize,
  extras: extraSize,
  loops,
  old_ms: Number(oldMs.toFixed(2)),
  new_ms: Number(newMs.toFixed(2)),
  speedup: Number(speedup.toFixed(2)),
  old_sum: oldSum,
  new_sum: newSum,
  equivalent,
};

if (asJson) {
  console.log(JSON.stringify(result));
} else {
  console.log("StatusPanel render lookup benchmark");
  console.log(`queue=${queueSize}, extras=${extraSize}, loops=${loops}`);
  console.log(`old(indexOf): ${result.old_ms}ms`);
  console.log(`new(Map):     ${result.new_ms}ms`);
  console.log(`speedup:      ${result.speedup}x`);
  console.log(`equivalent:   ${equivalent ? "yes" : "no"}`);
  if (!equivalent) {
    process.exitCode = 1;
  }
}
