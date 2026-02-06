const WORKLOADS = {
  synthetic: {
    queue: 500,
    extras: 120,
    loops: 2000,
  },
  real: {
    queue: 900,
    extras: 260,
    loops: 2200,
  },
};

function resolveWorkload(mode, options = {}) {
  const defaults = WORKLOADS[mode] ?? WORKLOADS.synthetic;
  const queue = Number(options.queue ?? defaults.queue);
  const extras = Number(options.extras ?? defaults.extras);
  const loops = Number(options.loops ?? defaults.loops);

  if (!Number.isFinite(queue) || queue <= 0) {
    throw new Error(`Invalid queue size: ${queue}`);
  }
  if (!Number.isFinite(extras) || extras < 0) {
    throw new Error(`Invalid extras size: ${extras}`);
  }
  if (!Number.isFinite(loops) || loops <= 0) {
    throw new Error(`Invalid loops value: ${loops}`);
  }

  return {
    queue: Math.floor(queue),
    extras: Math.floor(extras),
    loops: Math.floor(loops),
  };
}

function runOldLookup(queueOrder, orderedKeys, loops) {
  let sum = 0;
  for (let i = 0; i < loops; i += 1) {
    for (const key of orderedKeys) {
      const position = queueOrder.length > 0 ? queueOrder.indexOf(key) + 1 : 0;
      if (position > 0) {
        sum += position;
      }
    }
  }
  return sum;
}

function runNewLookup(queueOrder, orderedKeys, loops) {
  let sum = 0;
  const queuePositions = new Map(queueOrder.map((key, index) => [key, index + 1]));
  for (let i = 0; i < loops; i += 1) {
    for (const key of orderedKeys) {
      const position = queuePositions.get(key) ?? 0;
      if (position > 0) {
        sum += position;
      }
    }
  }
  return sum;
}

export function runLookupBenchmark(options = {}) {
  const workload = resolveWorkload(options.mode ?? "synthetic", options);

  const queueOrder = Array.from({ length: workload.queue }, (_, i) => `idx:${i}`);
  const extras = Array.from({ length: workload.extras }, (_, i) => `extra:${i}`);
  const orderedKeys = [...queueOrder, ...extras];

  const oldStart = performance.now();
  const oldSum = runOldLookup(queueOrder, orderedKeys, workload.loops);
  const oldEnd = performance.now();

  const newStart = performance.now();
  const newSum = runNewLookup(queueOrder, orderedKeys, workload.loops);
  const newEnd = performance.now();

  const oldMs = oldEnd - oldStart;
  const newMs = newEnd - newStart;
  const speedup = newMs === 0 ? Infinity : oldMs / newMs;
  const equivalent = oldSum === newSum;

  return {
    queue: workload.queue,
    extras: workload.extras,
    loops: workload.loops,
    old_ms: Number(oldMs.toFixed(2)),
    new_ms: Number(newMs.toFixed(2)),
    speedup: Number(speedup.toFixed(2)),
    old_sum: oldSum,
    new_sum: newSum,
    equivalent,
  };
}

export const benchmark = {
  name: "statuspanel-render-lookup",
  description: "StatusPanel render ordering lookup hot-path.",
  phase: 1,
  metricType: "duration_ms",
  direction: "lower_is_better",
  warmupRuns: 2,
  async run({ mode }) {
    const result = runLookupBenchmark({ mode });
    if (!result.equivalent) {
      throw new Error("Lookup benchmark produced non-equivalent result.");
    }

    return {
      value: result.new_ms,
      details: result,
    };
  },
};
