const SYNTHETIC_WORKLOAD = {
	queryCount: 200,
	loops: 100,
	itemCount: 120,
};

function extractAsin(query) {
	let current = '';
	for (const ch of query) {
		if (/[a-z0-9]/i.test(ch)) {
			current += ch.toUpperCase();
		} else {
			if (current.length === 10) return current;
			current = '';
		}
	}
	return current.length === 10 ? current : null;
}

function extractRegionOverride(query) {
	for (let i = 0; i <= query.length - 4; i += 1) {
		if (query[i] !== '[' || query[i + 3] !== ']') continue;
		const region = query.slice(i + 1, i + 3).toLowerCase();
		if (['au', 'ca', 'de', 'es', 'fr', 'in', 'it', 'jp', 'us', 'uk'].includes(region)) {
			return region;
		}
	}
	return null;
}

function stripRegionOverrides(query) {
	return query
		.replace(/\[(au|ca|de|es|fr|in|it|jp|us|uk)\]/gi, ' ')
		.split(/\s+/)
		.filter(Boolean)
		.join(' ');
}

function parseYear(value) {
	if (!value || value.length < 4) return null;
	const parsed = Number.parseInt(value.slice(0, 4), 10);
	return Number.isFinite(parsed) ? parsed : null;
}

function normalizeHttpsUrl(raw) {
	if (!raw) return null;
	try {
		const parsed = new URL(raw);
		return parsed.protocol === 'https:' ? parsed.toString() : null;
	} catch {
		return null;
	}
}

function sampleQueries(count) {
	return Array.from({ length: count }, (_, index) => {
		const region = index % 2 === 0 ? '[us]' : '[uk]';
		const asin = `B0${String(index).padStart(8, '0')}`;
		return `${region} Project Book ${index} ${asin}`;
	});
}

function sampleAudibleItems(count) {
	return Array.from({ length: count }, (_, index) => ({
		asin: `B0${String(index).padStart(8, '0')}`,
		title: `Synthetic Title ${index}`,
		subtitle: index % 3 === 0 ? `Subtitle ${index}` : null,
		authors: [{ name: `Author ${index % 17}` }],
		narrators: [{ name: `Narrator ${index % 13}` }],
		release_date: `20${(10 + (index % 15)).toString().padStart(2, '0')}-01-01`,
		runtime_length_min: 420 + index,
		publisher_summary: `Summary ${index}`,
		merchandising_summary: null,
		product_images: {
			500: 'https://images.example.com/cover-500.jpg',
			1024: 'https://images.example.com/cover-1024.jpg',
		},
	}));
}

function mapAudibleItem(item) {
	const baseTitle = (item.title ?? item.asin).trim();
	const title = item.subtitle?.trim() ? `${baseTitle}: ${item.subtitle.trim()}` : baseTitle;

	return {
		source: 'audnexus',
		source_id: item.asin,
		title,
		authors: (item.authors ?? []).map((author) => author.name),
		narrators: (item.narrators ?? []).map((narrator) => narrator.name),
		description: item.publisher_summary ?? item.merchandising_summary,
		published_year: parseYear(item.release_date),
		duration_seconds: item.runtime_length_min ? Math.round(item.runtime_length_min * 60) : null,
		cover_url:
			normalizeHttpsUrl(item.product_images?.['1024']) ??
			normalizeHttpsUrl(item.product_images?.['500']),
	};
}

function runSyntheticMetadataBench() {
	const queries = sampleQueries(SYNTHETIC_WORKLOAD.queryCount);
	const items = sampleAudibleItems(SYNTHETIC_WORKLOAD.itemCount);

	let checksum = 0;
	const start = performance.now();

	for (let loop = 0; loop < SYNTHETIC_WORKLOAD.loops; loop += 1) {
		for (const query of queries) {
			const asin = extractAsin(query);
			const region = extractRegionOverride(query) ?? 'us';
			const cleaned = stripRegionOverrides(query);
			checksum += (asin?.length ?? 0) + region.length + cleaned.length;
		}

		for (const item of items) {
			const mapped = mapAudibleItem(item);
			checksum += mapped.title.length + (mapped.published_year ?? 0);
		}
	}

	const elapsedMs = performance.now() - start;
	return {
		value: elapsedMs,
		details: {
			mode: 'synthetic',
			query_count: SYNTHETIC_WORKLOAD.queryCount,
			item_count: SYNTHETIC_WORKLOAD.itemCount,
			loops: SYNTHETIC_WORKLOAD.loops,
			operations:
				SYNTHETIC_WORKLOAD.loops * (SYNTHETIC_WORKLOAD.queryCount + SYNTHETIC_WORKLOAD.itemCount),
			checksum,
			elapsed_ms: Number(elapsedMs.toFixed(3)),
		},
	};
}

export const benchmark = {
	name: 'metadata-lookup-latency',
	description: 'Deterministic metadata lookup pipeline latency for local parsing and mapping work.',
	userImpact: 'Metadata lookup pipeline bookkeeping stays fast before any network request happens',
	phase: 1,
	metricType: 'duration_ms',
	direction: 'lower_is_better',
	supportedModes: ['synthetic'],
	async run({ mode }) {
		if (mode === 'real') {
			return {
				skipped: true,
				reason:
					'metadata-lookup-latency is synthetic-only; use metadata-lookup-network-probe for external latency.',
			};
		}
		return runSyntheticMetadataBench();
	},
};
