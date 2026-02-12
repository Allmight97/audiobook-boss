export const benchmark = {
	name: 'cancel-latency',
	description: 'Phase 2 benchmark placeholder for job cancellation responsiveness.',
	phase: 2,
	metricType: 'duration_ms',
	direction: 'lower_is_better',
	warmupRuns: 0,
	async run() {
		return {
			skipped: true,
			reason:
				'Phase 2 benchmark not implemented yet. Track cancellation path perf after Phase 1 baselines stabilize.',
		};
	},
};
