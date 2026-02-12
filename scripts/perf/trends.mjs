import { sparklineAscii, formatDelta } from './shared/stats.mjs';
import { BENCHMARKS_BY_NAME } from './benches/index.mjs';

const BENCH_UX_META = {
	'statuspanel-render-lookup': {
		area: 'UI Rendering',
		humanize: (median) => {
			if (!Number.isFinite(median)) return 'n/a';
			return `${median.toFixed(2)} ms (well under 16ms frame budget)`;
		},
	},
	'statuspanel-event-throughput': {
		area: 'Event Pipeline',
		humanize: (median) => {
			if (!Number.isFinite(median)) return 'n/a';
			const k = Math.round(median / 1000);
			return `${k}K events/sec`;
		},
	},
	'metadata-lookup-latency': {
		area: 'Search',
		humanize: (median) => {
			if (!Number.isFinite(median)) return 'n/a';
			return `${median.toFixed(1)} ms`;
		},
	},
	'audio-processing-throughput': {
		area: 'Audio Encoding',
		humanize: (median, mode, details) => {
			if (!Number.isFinite(median)) return 'n/a';
			if (mode === 'synthetic') {
				return `${Math.round(median)} MiB/s`;
			}
			// real mode: median is realtime_factor, calculate encoding time for 33-min book
			const bookMinutes = 33;
			const encodeSeconds = (bookMinutes * 60) / median;
			return `${median.toFixed(1)}x realtime (~${Math.round(encodeSeconds)}s for a ${bookMinutes}-min book)`;
		},
	},
	'audio-processing-app-e2e': {
		area: 'Audio Encoding (App E2E)',
		humanize: (median) => {
			if (!Number.isFinite(median)) return 'n/a';
			return `${median.toFixed(1)}x realtime`;
		},
	},
};

function keyFor(row) {
	return `${row.bench_name}::${row.mode}`;
}

function formatNumber(value, digits = 3) {
	if (!Number.isFinite(value)) return 'n/a';
	return value.toFixed(digits);
}

function statusEmoji(status) {
	if (status === 'warn') return 'WARN';
	if (status === 'improved') return 'IMPROVED';
	if (status === 'ok') return 'OK';
	if (status === 'missing') return 'MISSING_BASELINE';
	if (status === 'skipped') return 'SKIPPED';
	return status?.toUpperCase() ?? 'UNKNOWN';
}

function buildSparklineRows(historyRows, latestRows) {
	const grouped = new Map();

	for (const row of historyRows) {
		const key = keyFor(row);
		if (!grouped.has(key)) {
			grouped.set(key, []);
		}
		grouped.get(key).push(row);
	}

	for (const rows of grouped.values()) {
		rows.sort((a, b) => a.timestamp.localeCompare(b.timestamp));
	}

	return latestRows.map((row) => {
		const key = keyFor(row);
		const history = grouped.get(key) ?? [];
		const values = history.map((entry) => entry.median).filter((value) => Number.isFinite(value));
		const recentValues = values.slice(-12);

		return {
			bench_name: row.bench_name,
			mode: row.mode,
			points: recentValues.length,
			sparkline: sparklineAscii(recentValues),
			recent: recentValues.map((value) => Number(value.toFixed(3))),
		};
	});
}

function buildEncoderBreakdown(latestRows) {
	const audioRow = latestRows.find((row) => row.bench_name === 'audio-processing-throughput');
	if (!audioRow || audioRow.mode !== 'real') return null;

	const encoderRuns = audioRow.details?.latest?.encoder_runs;
	if (!Array.isArray(encoderRuns) || encoderRuns.length === 0) return null;

	const lines = [];
	lines.push('');
	lines.push('### Encoder Breakdown (audio-processing-throughput, real mode)');
	lines.push('');
	lines.push('| Encoder | Speed | Throughput | Wall Time | What This Means |');
	lines.push('| --- | ---: | ---: | ---: | --- |');

	const interpretations = {
		aac_at: 'Fastest — native macOS hardware path',
		fdk_he_aac: 'Good quality-to-speed for HE-AAC',
		native_aac: 'Slowest — software-only fallback',
	};

	for (const run of encoderRuns) {
		const encoder = run.encoder ?? 'unknown';
		const label = encoder === 'aac_at' ? 'aac_at (Apple)' : encoder;
		const speed = Number.isFinite(run.realtime_factor)
			? `${run.realtime_factor.toFixed(1)}x realtime`
			: 'n/a';
		const throughput = Number.isFinite(run.throughput_mib_per_s)
			? `${run.throughput_mib_per_s.toFixed(1)} MiB/s`
			: 'n/a';
		const wall = Number.isFinite(run.elapsed_ms) ? `${(run.elapsed_ms / 1000).toFixed(1)}s` : 'n/a';
		const meaning = interpretations[encoder] ?? '';

		lines.push(`| ${label} | ${speed} | ${throughput} | ${wall} | ${meaning} |`);
	}

	return lines.join('\n');
}

function buildAttributionMatrix(latestRows) {
	const cliRow = latestRows.find(
		(row) => row.bench_name === 'audio-processing-throughput' && row.mode === 'real',
	);
	const appRow = latestRows.find(
		(row) => row.bench_name === 'audio-processing-app-e2e' && row.mode === 'real',
	);
	if (!cliRow || !appRow) return null;

	const cliRuns = cliRow.details?.latest?.encoder_runs;
	const appRuns = appRow.details?.latest?.encoder_runs;
	if (!Array.isArray(cliRuns) || !Array.isArray(appRuns)) return null;
	if (cliRuns.length === 0 || appRuns.length === 0) return null;

	const appByEncoder = new Map();
	for (const run of appRuns) {
		if (!run?.encoder) continue;
		appByEncoder.set(run.encoder, run);
	}

	const rows = [];
	for (const cli of cliRuns) {
		const encoder = cli?.encoder;
		if (!encoder) continue;
		const app = appByEncoder.get(encoder);
		if (!app) continue;
		if (!Number.isFinite(cli.realtime_factor) || !Number.isFinite(app.realtime_factor)) continue;

		const rtfCli = Number(cli.realtime_factor);
		const rtfApp = Number(app.realtime_factor);
		const overheadRatio = rtfCli > 0 ? (rtfCli - rtfApp) / rtfCli : null;
		rows.push({
			encoder,
			rtfCli,
			rtfApp,
			overheadRatio,
		});
	}

	if (rows.length === 0) return null;

	const lines = [];
	lines.push('');
	lines.push('### Attribution Matrix (App vs Encoder, real mode)');
	lines.push('');
	lines.push('| Encoder | rtf_cli | rtf_app | overhead_ratio | Interpretation |');
	lines.push('| --- | ---: | ---: | ---: | --- |');

	for (const row of rows) {
		const overheadPct = Number.isFinite(row.overheadRatio)
			? `${(row.overheadRatio * 100).toFixed(1)}%`
			: 'n/a';
		const interpretation = !Number.isFinite(row.overheadRatio)
			? 'n/a'
			: row.overheadRatio > 0.3
				? 'High app-side overhead opportunity'
				: row.overheadRatio > 0.15
					? 'Moderate app-side overhead'
					: row.overheadRatio > 0.05
						? 'Low app-side overhead'
						: 'Near encoder baseline';
		lines.push(
			`| ${row.encoder} | ${row.rtfCli.toFixed(1)}x | ${row.rtfApp.toFixed(1)}x | ${overheadPct} | ${interpretation} |`,
		);
	}

	return lines.join('\n');
}

export function buildLatestMarkdown({ summary, latestRows, historyRows }) {
	const lines = [];
	lines.push('# Performance Results');
	lines.push('');

	// Group by mode to show timestamps
	const modeGroups = new Map();
	for (const row of latestRows) {
		if (!modeGroups.has(row.mode)) {
			modeGroups.set(row.mode, row.timestamp);
		}
	}

	if (modeGroups.size === 1) {
		lines.push(`- Timestamp: ${summary.timestamp}`);
		lines.push(`- Mode: ${summary.mode}`);
	} else {
		const syntheticTs = modeGroups.get('synthetic');
		const realTs = modeGroups.get('real');
		if (syntheticTs) lines.push(`- Synthetic: ${syntheticTs}`);
		if (realTs) lines.push(`- Real: ${realTs}`);
	}

	lines.push(`- Git: ${summary.git_branch} (${summary.git_sha})`);
	lines.push(`- Host: ${summary.host_os} | ${summary.cpu_info}`);
	lines.push(`- Runs: ${summary.runs} (warmup ${summary.warmup_runs ?? 'benchmark defaults'})`);
	lines.push('');

	// UX/Outcomes Matrix
	lines.push('## Performance Matrix');
	lines.push('');
	lines.push('| What Users Feel | Area | Mode | Result | vs Baseline | Health |');
	lines.push('| --- | --- | --- | ---: | ---: | --- |');

	for (const row of latestRows) {
		const bench = BENCHMARKS_BY_NAME.get(row.bench_name);
		const userImpact = bench?.userImpact ?? row.bench_name;
		const meta = BENCH_UX_META[row.bench_name];
		const area = meta?.area ?? 'System';
		const result = meta?.humanize
			? meta.humanize(row.median, row.mode, row.details?.latest)
			: formatNumber(row.median);
		const delta = formatDelta(row.delta_pct);
		const health = statusEmoji(row.status);

		lines.push(`| ${userImpact} | ${area} | ${row.mode} | ${result} | ${delta} | ${health} |`);
	}

	// Encoder breakdown sub-table if applicable
	const encoderBreakdown = buildEncoderBreakdown(latestRows);
	if (encoderBreakdown) {
		lines.push(encoderBreakdown);
	}
	const attributionMatrix = buildAttributionMatrix(latestRows);
	if (attributionMatrix) {
		lines.push(attributionMatrix);
	}

	lines.push('');
	lines.push('## Technical Detail');
	lines.push('');
	lines.push('| Bench | Mode | Metric | Median | P95 | Delta vs Baseline | Status |');
	lines.push('| --- | --- | --- | ---: | ---: | ---: | --- |');

	for (const row of latestRows) {
		lines.push(
			`| ${row.bench_name} | ${row.mode} | ${row.metric_type} | ${formatNumber(row.median)} | ${formatNumber(row.p95)} | ${formatDelta(row.delta_pct)} | ${statusEmoji(row.status)} |`,
		);
	}

	const sparkRows = buildSparklineRows(historyRows, latestRows);

	lines.push('');
	lines.push('## Trend Snapshot (Last 12)');
	lines.push('');
	lines.push('| Bench | Mode | Points | Trend | Recent Medians |');
	lines.push('| --- | --- | ---: | --- | --- |');

	for (const row of sparkRows) {
		lines.push(
			`| ${row.bench_name} | ${row.mode} | ${row.points} | ${row.sparkline || 'n/a'} | ${row.recent.length > 0 ? row.recent.join(', ') : 'n/a'} |`,
		);
	}

	lines.push('');
	lines.push('## Notes');
	lines.push('');
	lines.push('- `warn` indicates >15% regression versus baseline in the wrong direction.');
	lines.push('- `improved` indicates >15% improvement versus baseline.');
	lines.push('- `missing` means no baseline entry exists for that bench/mode.');

	return `${lines.join('\n')}\n`;
}
