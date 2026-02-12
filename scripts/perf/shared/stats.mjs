export function mean(values) {
	if (!Array.isArray(values) || values.length === 0) return null;
	return values.reduce((sum, value) => sum + value, 0) / values.length;
}

export function median(values) {
	if (!Array.isArray(values) || values.length === 0) return null;
	const sorted = [...values].sort((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	if (sorted.length % 2 === 0) {
		return (sorted[middle - 1] + sorted[middle]) / 2;
	}
	return sorted[middle];
}

export function percentile(values, p) {
	if (!Array.isArray(values) || values.length === 0) return null;
	if (p <= 0) return Math.min(...values);
	if (p >= 100) return Math.max(...values);

	const sorted = [...values].sort((a, b) => a - b);
	const rank = (p / 100) * (sorted.length - 1);
	const lower = Math.floor(rank);
	const upper = Math.ceil(rank);
	if (lower === upper) return sorted[lower];

	const weight = rank - lower;
	return sorted[lower] * (1 - weight) + sorted[upper] * weight;
}

export function stddev(values) {
	if (!Array.isArray(values) || values.length === 0) return null;
	const avg = mean(values);
	const variance = values.reduce((sum, value) => sum + (value - avg) ** 2, 0) / values.length;
	return Math.sqrt(variance);
}

export function summarizeSamples(values) {
	if (!Array.isArray(values) || values.length === 0) {
		return null;
	}

	const minValue = Math.min(...values);
	const maxValue = Math.max(...values);
	const avg = mean(values);
	const dev = stddev(values);

	return {
		count: values.length,
		min: minValue,
		max: maxValue,
		mean: avg,
		median: median(values),
		p95: percentile(values, 95),
		stddev: dev,
		cv: avg === 0 ? null : dev / avg,
	};
}

export function computeComparison({
	currentValue,
	baselineValue,
	lowerIsBetter,
	warnThresholdPct,
}) {
	if (!Number.isFinite(currentValue)) {
		return {
			status: 'invalid',
			baseline_value: baselineValue,
			delta_pct: null,
			threshold_pct: warnThresholdPct,
		};
	}

	if (!Number.isFinite(baselineValue) || baselineValue === 0) {
		return {
			status: 'missing',
			baseline_value: baselineValue,
			delta_pct: null,
			threshold_pct: warnThresholdPct,
		};
	}

	const deltaPct = ((currentValue - baselineValue) / baselineValue) * 100;
	const absDeltaPct = Math.abs(deltaPct);

	if (absDeltaPct <= warnThresholdPct) {
		return {
			status: 'ok',
			baseline_value: baselineValue,
			delta_pct: deltaPct,
			threshold_pct: warnThresholdPct,
		};
	}

	const improved = lowerIsBetter ? deltaPct < 0 : deltaPct > 0;
	return {
		status: improved ? 'improved' : 'warn',
		baseline_value: baselineValue,
		delta_pct: deltaPct,
		threshold_pct: warnThresholdPct,
	};
}

export function sparklineAscii(values) {
	if (!Array.isArray(values) || values.length === 0) return '';
	if (values.length === 1) return '=';

	const glyphs = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];
	const minValue = Math.min(...values);
	const maxValue = Math.max(...values);
	const range = maxValue - minValue;

	if (range === 0) {
		return '='.repeat(values.length);
	}

	return values
		.map((value) => {
			const normalized = (value - minValue) / range;
			const index = Math.min(glyphs.length - 1, Math.floor(normalized * glyphs.length));
			return glyphs[index];
		})
		.join('');
}

export function formatDelta(deltaPct) {
	if (!Number.isFinite(deltaPct)) return 'n/a';
	const sign = deltaPct > 0 ? '+' : '';
	return `${sign}${deltaPct.toFixed(2)}%`;
}
