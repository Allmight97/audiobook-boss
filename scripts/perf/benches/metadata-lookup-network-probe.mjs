const REAL_WORKLOAD = {
	requests: [
		'https://api.audible.com/1.0/catalog/products?response_groups=contributors,product_desc,product_attrs,product_extended_attrs,media,product_details,series&products_sort_by=Relevance&num_results=3&image_sizes=500,1024&keywords=project%20hail%20mary',
		'https://api.audnex.us/books/B08G9PRS1K?region=us',
	],
	timeoutMs: 12000,
};

async function fetchWithTiming(url, timeoutMs) {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);

	const started = performance.now();
	try {
		const response = await fetch(url, {
			signal: controller.signal,
			headers: {
				'user-agent': 'audiobook-boss-perf/1.0',
			},
		});

		const text = await response.text();
		if (!response.ok) {
			throw new Error(`HTTP ${response.status}`);
		}

		return {
			ok: true,
			elapsedMs: performance.now() - started,
			bytes: text.length,
		};
	} catch (error) {
		return {
			ok: false,
			elapsedMs: performance.now() - started,
			error: error instanceof Error ? error.message : String(error),
		};
	} finally {
		clearTimeout(timer);
	}
}

async function runNetworkProbe() {
	if (process.env.ABB_PERF_ALLOW_NETWORK !== '1') {
		return {
			skipped: true,
			reason: 'metadata-lookup-network-probe requires ABB_PERF_ALLOW_NETWORK=1.',
		};
	}

	const samples = [];
	for (const url of REAL_WORKLOAD.requests) {
		const sample = await fetchWithTiming(url, REAL_WORKLOAD.timeoutMs);
		if (!sample.ok) {
			return {
				skipped: true,
				reason: `metadata-lookup-network-probe skipped after request failure for ${url}: ${sample.error}`,
			};
		}
		samples.push({
			url,
			elapsedMs: sample.elapsedMs,
			bytes: sample.bytes,
		});
	}

	const elapsed = samples.map((sample) => sample.elapsedMs);
	const avgElapsed = elapsed.reduce((sum, value) => sum + value, 0) / elapsed.length;

	return {
		value: avgElapsed,
		details: {
			mode: 'real',
			requests: samples.length,
			request_latencies_ms: elapsed.map((value) => Number(value.toFixed(3))),
			bytes: samples.reduce((sum, sample) => sum + (sample.bytes ?? 0), 0),
			elapsed_avg_ms: Number(avgElapsed.toFixed(3)),
			urls: samples.map((sample) => sample.url),
		},
	};
}

export const benchmark = {
	name: 'metadata-lookup-network-probe',
	description: 'Optional external metadata lookup network probe for remote service latency.',
	userImpact: 'Optional check that remote metadata providers are reachable with sane latency',
	phase: 1,
	metricType: 'duration_ms',
	direction: 'lower_is_better',
	warmupRuns: 0,
	supportedModes: ['real'],
	async run({ mode }) {
		if (mode !== 'real') {
			return {
				skipped: true,
				reason: 'metadata-lookup-network-probe is real-mode only.',
			};
		}
		return runNetworkProbe();
	},
};
