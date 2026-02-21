import type { JobType } from '../../types/audio';

export const jobControlsState = $state({
	jobType: 'batch' as JobType,
	controlsEnabled: true,
	maxConcurrentSelection: 'auto',
	effectiveMaxConcurrent: null as number | null,
	effectiveLabel: '',
});
