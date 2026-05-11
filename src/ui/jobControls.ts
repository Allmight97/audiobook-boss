import { tauriClient } from '../lib/tauri/client';
import type { JobType } from '../types/audio';
import { flushSync } from 'svelte';
import { jobControlsState } from './jobControls/state.svelte';
import { updateOutputPath } from './outputPanel';
import { updateStatusPanelConcurrencyStatus } from './statusPanel';

export function initJobControls(): void {
	initializeMaxConcurrentControl();
}

function initializeMaxConcurrentControl(): void {
	void pushMaxConcurrentToBackend(jobControlsState.maxConcurrentSelection);
}

export function handleMergeModeChange(checked: boolean): void {
	setJobType(checked ? 'merge' : 'batch', true);
}

export function handleMaxConcurrentSelectionChange(value: string): void {
	jobControlsState.maxConcurrentSelection = value;
	void pushMaxConcurrentToBackend(value);
}

function setJobType(jobType: JobType, emitChangeEvent: boolean): void {
	jobControlsState.jobType = jobType;
	if (emitChangeEvent) {
		updateOutputPath('final');
	}
}

export function setJobTypeSelection(jobType: JobType): void {
	setJobType(jobType, false);
}

// Read current value
export function getJobType(): JobType {
	return jobControlsState.jobType;
}

export function getMaxConcurrentStatus(): {
	effective: number | null;
	selection: string;
} {
	return {
		effective: jobControlsState.effectiveMaxConcurrent,
		selection: jobControlsState.maxConcurrentSelection,
	};
}

export function setJobControlsEnabled(enabled: boolean): void {
	flushSync(() => {
		jobControlsState.controlsEnabled = enabled;
	});
}

function updateMaxConcurrentIndicator(): void {
	if (jobControlsState.effectiveMaxConcurrent === null) {
		jobControlsState.effectiveLabel = '';
		updateStatusPanelConcurrencyStatus('Max jobs: —');
		return;
	}

	if (jobControlsState.maxConcurrentSelection === 'auto') {
		jobControlsState.effectiveLabel = `Auto → ${jobControlsState.effectiveMaxConcurrent}`;
	} else {
		jobControlsState.effectiveLabel = `Max ${jobControlsState.effectiveMaxConcurrent}`;
	}

	const suffix = jobControlsState.maxConcurrentSelection === 'auto' ? ' (Auto)' : '';
	updateStatusPanelConcurrencyStatus(
		`Max jobs: ${jobControlsState.effectiveMaxConcurrent}${suffix}`,
	);
}

async function pushMaxConcurrentToBackend(value: string): Promise<void> {
	try {
		let effective: number;
		if (value === 'auto') {
			effective = await tauriClient.setMaxConcurrentJobs(null);
		} else {
			const parsed = parseInt(value, 10);
			if (!Number.isFinite(parsed)) {
				console.warn('Invalid max concurrency selection ignored:', value);
				return;
			}
			effective = await tauriClient.setMaxConcurrentJobs(parsed);
		}
		if (!Number.isFinite(effective)) {
			return;
		}
		jobControlsState.effectiveMaxConcurrent = effective;
		updateMaxConcurrentIndicator();
	} catch (error) {
		console.warn('Failed to update max concurrency:', error);
	}
}
