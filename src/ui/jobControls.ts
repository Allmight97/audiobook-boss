import { tauriClient } from '../lib/tauri/client';
import type { ConcurrencyPreference } from '../types/appSettings';
import type { JobType } from '../types/audio';
import { flushSync } from 'svelte';
import { jobControlsState } from './jobControls/state.svelte';
import { updateOutputPath } from './outputPanel';
import { updateStatusPanelConcurrencyStatus } from './statusPanel';
import { hydrateRuntimeSettingsCapabilities } from './runtimeSettingsCapabilities.svelte';

export function initJobControls(): void {
	void initializeRuntimeCapabilities();
	initializeMaxConcurrentControl();
}

async function initializeRuntimeCapabilities(): Promise<void> {
	const capabilities = await hydrateRuntimeSettingsCapabilities();
	jobControlsState.maxConcurrentCapabilities = capabilities?.maxConcurrentJobs ?? null;
}

function initializeMaxConcurrentControl(): void {
	void refreshMaxConcurrentIndicatorFromBackend();
}

export function handleMergeModeChange(checked: boolean): void {
	setJobType(checked ? 'merge' : 'batch', true);
}

export function handleMaxConcurrentSelectionChange(value: string): void {
	const previousSelection = jobControlsState.maxConcurrentSelection;
	jobControlsState.maxConcurrentSelection = value;
	void pushMaxConcurrentToBackend(value, true, previousSelection);
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

export async function applyMaxConcurrentPreference(
	preference: ConcurrencyPreference,
): Promise<void> {
	await initializeRuntimeCapabilities();
	const selection = preference.mode === 'fixed' ? String(preference.value) : 'auto';
	const previousSelection = jobControlsState.maxConcurrentSelection;
	jobControlsState.maxConcurrentSelection = selection;
	await pushMaxConcurrentToBackend(selection, false, previousSelection);
}

export function readMaxConcurrentPreferenceFromState(): ConcurrencyPreference {
	const effective = jobControlsState.effectiveMaxConcurrent;
	if (jobControlsState.maxConcurrentSelection === 'auto') {
		return { mode: 'auto' };
	}

	const parsed = parseInt(jobControlsState.maxConcurrentSelection, 10);
	return { mode: 'fixed', value: effective ?? parsed };
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

async function pushMaxConcurrentToBackend(
	value: string,
	persist: boolean,
	rollbackSelection?: string,
): Promise<void> {
	if (persist) {
		await pushPersistedMaxConcurrentToBackend(value, rollbackSelection);
		return;
	}

	try {
		const effective =
			value === 'auto' ? await tauriClient.setMaxConcurrentJobs(null) : await setFixedMax(value);
		applyEffectiveMaxConcurrent(effective);
	} catch (error) {
		if (rollbackSelection !== undefined) {
			jobControlsState.maxConcurrentSelection = rollbackSelection;
		}
		void refreshMaxConcurrentIndicatorFromBackend();
		console.warn('Failed to update max concurrency:', error);
	}
}

async function pushPersistedMaxConcurrentToBackend(
	value: string,
	rollbackSelection?: string,
): Promise<void> {
	const preference = preferenceFromSelection(value);
	if (preference.mode === 'fixed' && !Number.isFinite(preference.value)) {
		console.warn('Invalid max concurrency selection ignored:', value);
		return;
	}

	let acceptedPreference: ConcurrencyPreference;
	try {
		const settings = await tauriClient.updateAppSettings({
			maxConcurrentJobs: preference,
		});
		acceptedPreference = settings.maxConcurrentJobs;
		jobControlsState.maxConcurrentSelection = selectionFromPreference(acceptedPreference);
	} catch (error) {
		if (rollbackSelection !== undefined) {
			jobControlsState.maxConcurrentSelection = rollbackSelection;
		}
		void refreshMaxConcurrentIndicatorFromBackend();
		console.warn('Failed to update max concurrency:', error);
		return;
	}

	try {
		applyEffectiveMaxConcurrent(await tauriClient.getMaxConcurrentJobs());
	} catch (error) {
		applyAcceptedMaxConcurrentFallback(acceptedPreference);
		console.warn('Failed to refresh accepted max concurrency:', error);
	}
}

async function refreshMaxConcurrentIndicatorFromBackend(): Promise<void> {
	try {
		applyEffectiveMaxConcurrent(await tauriClient.getMaxConcurrentJobs());
	} catch (error) {
		console.warn('Failed to load max concurrency:', error);
	}
}

async function setFixedMax(value: string): Promise<number> {
	const parsed = parseInt(value, 10);
	if (!isAllowedFixedMaxConcurrent(parsed)) {
		throw new Error(`Invalid max concurrency selection ignored: ${value}`);
	}
	return tauriClient.setMaxConcurrentJobs(parsed);
}

function applyEffectiveMaxConcurrent(effective: number): void {
	if (!Number.isFinite(effective)) {
		return;
	}
	jobControlsState.effectiveMaxConcurrent = effective;
	updateMaxConcurrentIndicator();
}

function applyAcceptedMaxConcurrentFallback(preference: ConcurrencyPreference): void {
	jobControlsState.effectiveMaxConcurrent = preference.mode === 'fixed' ? preference.value : null;
	updateMaxConcurrentIndicator();
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	if (value === 'auto') {
		return { mode: 'auto' };
	}

	const parsed = parseInt(value, 10);
	if (!isAllowedFixedMaxConcurrent(parsed)) {
		return { mode: 'fixed', value: Number.NaN };
	}
	return { mode: 'fixed', value: parsed };
}

function isAllowedFixedMaxConcurrent(value: number): boolean {
	if (!Number.isFinite(value)) {
		return false;
	}

	const capabilities = jobControlsState.maxConcurrentCapabilities;
	return capabilities ? capabilities.fixedOptions.includes(value) : true;
}

function selectionFromPreference(preference: ConcurrencyPreference): string {
	return preference.mode === 'fixed' ? String(preference.value) : 'auto';
}
