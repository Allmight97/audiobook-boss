import { benchmark as statuspanelRenderLookup } from './statuspanel-render-lookup.mjs';
import { benchmark as statuspanelEventThroughput } from './statuspanel-event-throughput.mjs';
import { benchmark as metadataLookupLatency } from './metadata-lookup-latency.mjs';
import { benchmark as metadataLookupNetworkProbe } from './metadata-lookup-network-probe.mjs';
import { benchmark as audioProcessingThroughput } from './audio-processing-throughput.mjs';
import { benchmark as audioProcessingAppE2E } from './audio-processing-app-e2e.mjs';
import { benchmark as cancelLatency } from './cancel-latency.mjs';
import { benchmark as coverArtPath } from './cover-art-path.mjs';

export const BENCHMARKS = [
	statuspanelRenderLookup,
	statuspanelEventThroughput,
	metadataLookupLatency,
	metadataLookupNetworkProbe,
	audioProcessingThroughput,
	audioProcessingAppE2E,
	cancelLatency,
	coverArtPath,
];

export const BENCHMARKS_BY_NAME = BENCHMARKS.reduce((acc, bench) => {
	acc.set(bench.name, bench);
	return acc;
}, new Map());

export const CORE3_BENCH_NAMES = [
	'statuspanel-render-lookup',
	'statuspanel-event-throughput',
	'metadata-lookup-latency',
	'metadata-lookup-network-probe',
];

export const PHASE1_BENCH_NAMES = [
	'statuspanel-render-lookup',
	'statuspanel-event-throughput',
	'metadata-lookup-latency',
	'metadata-lookup-network-probe',
	'audio-processing-throughput',
	'audio-processing-app-e2e',
];

export const PHASE2_BENCH_NAMES = ['cancel-latency', 'cover-art-path'];
