import type { JobType } from '../../types/audio';
import type { MaxConcurrentJobsCapabilities } from '../../types/audio';

export const jobControlsState = $state({
	jobType: 'batch' as JobType,
	controlsEnabled: true,
	maxConcurrentCapabilities: null as MaxConcurrentJobsCapabilities | null,
	maxConcurrentSelection: 'auto',
	effectiveMaxConcurrent: null as number | null,
	effectiveLabel: '',
});
