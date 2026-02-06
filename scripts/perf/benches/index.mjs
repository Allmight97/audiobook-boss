import { benchmark as statuspanelRenderLookup } from "./statuspanel-render-lookup.mjs";
import { benchmark as statuspanelEventThroughput } from "./statuspanel-event-throughput.mjs";
import { benchmark as metadataLookupLatency } from "./metadata-lookup-latency.mjs";
import { benchmark as audioProcessingThroughput } from "./audio-processing-throughput.mjs";
import { benchmark as cancelLatency } from "./cancel-latency.mjs";
import { benchmark as coverArtPath } from "./cover-art-path.mjs";

export const BENCHMARKS = [
  statuspanelRenderLookup,
  statuspanelEventThroughput,
  metadataLookupLatency,
  audioProcessingThroughput,
  cancelLatency,
  coverArtPath,
];

export const BENCHMARKS_BY_NAME = BENCHMARKS.reduce((acc, bench) => {
  acc.set(bench.name, bench);
  return acc;
}, new Map());

export const CORE3_BENCH_NAMES = [
  "statuspanel-render-lookup",
  "statuspanel-event-throughput",
  "metadata-lookup-latency",
];

export const PHASE1_BENCH_NAMES = [
  "statuspanel-render-lookup",
  "statuspanel-event-throughput",
  "metadata-lookup-latency",
  "audio-processing-throughput",
];

export const PHASE2_BENCH_NAMES = [
  "cancel-latency",
  "cover-art-path",
];
