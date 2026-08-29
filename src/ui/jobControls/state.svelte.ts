import type { JobType } from '../../types/audio';
import type { MaxConcurrentJobsCapabilities } from '../../types/audio';

export const jobControlsState = {
	jobType: 'batch' as JobType,
	controlsEnabled: true,
	maxConcurrentCapabilities: null as MaxConcurrentJobsCapabilities | null,
	maxConcurrentSelection: 'auto',
	effectiveMaxConcurrent: null as number | null,
	effectiveLabel: '',
};

const listeners = new Set<() => void>();

export function subscribeJobControls(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

export function notifyJobControls(): void {
	for (const listener of listeners) listener();
}
