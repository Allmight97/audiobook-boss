import { tauriClient } from '../lib/tauri/client';
import type { JobType } from '../types/audio';
import { flushSync } from 'svelte';
import { jobControlsState } from './jobControls/state.svelte';

const MAX_CONCURRENT_STORAGE_KEY = 'abb:maxConcurrentJobs';

export function initJobControls(): void {
	initializeMaxConcurrentControl();
}

function initializeMaxConcurrentControl(): void {
	const saved = readMaxConcurrentPreference();
	jobControlsState.maxConcurrentSelection = saved;

	// Push initial selection
	void pushMaxConcurrentToBackend(saved);
}

export function handleMergeModeChange(checked: boolean): void {
	setJobType(checked ? 'merge' : 'batch', true);
}

export function handleMaxConcurrentSelectionChange(value: string): void {
	jobControlsState.maxConcurrentSelection = value;
	writeMaxConcurrentPreference(value);
	void pushMaxConcurrentToBackend(value);
}

function setJobType(jobType: JobType, emitChangeEvent: boolean): void {
	jobControlsState.jobType = jobType;
	if (emitChangeEvent) {
		document.dispatchEvent(new Event('abb:job-type-changed'));
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
		return;
	}

	if (jobControlsState.maxConcurrentSelection === 'auto') {
		jobControlsState.effectiveLabel = `Auto → ${jobControlsState.effectiveMaxConcurrent}`;
	} else {
		jobControlsState.effectiveLabel = `Max ${jobControlsState.effectiveMaxConcurrent}`;
	}
}

function readMaxConcurrentPreference(): string {
	// FALLBACK[FB-004]: trigger=localStorage unavailable/blocked (privacy mode, restricted contexts)
	// observe=console.warn markers on read/write/parse fallback paths
	// sunset=2026-04-30 issue=#199
	if (typeof localStorage === 'undefined' || typeof localStorage.getItem !== 'function') {
		console.warn('FALLBACK[FB-004] localStorage.getItem unavailable; using auto max concurrency');
		return 'auto';
	}
	try {
		return localStorage.getItem(MAX_CONCURRENT_STORAGE_KEY) ?? 'auto';
	} catch (error) {
		console.warn('FALLBACK[FB-004] failed to read max concurrency preference; using auto', error);
		return 'auto';
	}
}

function writeMaxConcurrentPreference(value: string): void {
	if (typeof localStorage === 'undefined' || typeof localStorage.setItem !== 'function') {
		return;
	}
	try {
		localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, value);
	} catch (error) {
		// localStorage may be unavailable in private browsing; non-critical
		console.warn('FALLBACK[FB-004] failed to persist max concurrency preference', error);
	}
}

async function pushMaxConcurrentToBackend(value: string): Promise<void> {
	try {
		let effective: number;
		if (value === 'auto') {
			effective = await tauriClient.setMaxConcurrentJobs(null);
		} else {
			const parsed = parseInt(value, 10);
			if (!Number.isFinite(parsed)) {
				console.warn('FALLBACK[FB-004] invalid max concurrency selection ignored:', value);
				return;
			}
			effective = await tauriClient.setMaxConcurrentJobs(parsed);
		}
		if (!Number.isFinite(effective)) {
			return;
		}
		jobControlsState.effectiveMaxConcurrent = effective;
		updateMaxConcurrentIndicator();
		document.dispatchEvent(
			new CustomEvent('abb:max-concurrent-updated', {
				detail: { effective, selection: jobControlsState.maxConcurrentSelection },
			}),
		);
	} catch (error) {
		console.warn('Failed to update max concurrency:', error);
	}
}
