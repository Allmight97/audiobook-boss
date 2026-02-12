export const benchmark = {
	name: 'cover-art-path',
	description: 'Phase 2 benchmark placeholder for cover-art read/render path.',
	phase: 2,
	metricType: 'duration_ms',
	direction: 'lower_is_better',
	warmupRuns: 0,
	async run() {
		return {
			skipped: true,
			reason:
				'Phase 2 benchmark not implemented yet. Introduce after stable metadata/art fixture strategy.',
		};
	},
};
