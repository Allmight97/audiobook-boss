import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import AppSettingsDialogIsland from './AppSettingsDialogIsland.svelte';
import {
	applyMaxConcurrentPreference,
	initJobControls,
	setJobControlsEnabled,
} from '../jobControls';
import { jobControlsState } from '../jobControls/state.svelte';
import {
	runtimeSettingsCapabilitiesState,
	setRuntimeSettingsCapabilities,
} from '../runtimeSettingsCapabilities.svelte';
import { tauriClient } from '../../lib/tauri/client';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRuntimeSettingsCapabilities: vi.fn(),
		setMaxConcurrentJobs: vi.fn().mockResolvedValue(4),
		updateAppSettings: vi.fn().mockResolvedValue({
			maxConcurrentJobs: { mode: 'auto' },
			encoderDefaults: {},
			outputDefaults: {},
		}),
		getMaxConcurrentJobs: vi.fn().mockResolvedValue(4),
	},
}));

vi.mock('../coverArt', () => ({ refreshCoverArtDisplay: vi.fn() }));
vi.mock('../outputPanel', () => ({ updateOutputPath: vi.fn() }));
vi.mock('../encoderPanel', () => ({
	readFdkAfterburner: () => true,
	setFdkAfterburner: vi.fn(),
}));

const setMaxConcurrentJobsMock = vi.mocked(tauriClient.setMaxConcurrentJobs);
const updateAppSettingsMock = vi.mocked(tauriClient.updateAppSettings);
const getMaxConcurrentJobsMock = vi.mocked(tauriClient.getMaxConcurrentJobs);
const getRuntimeSettingsCapabilitiesMock = vi.mocked(tauriClient.getRuntimeSettingsCapabilities);

async function flushAsync() {
	// Macrotask hops drain the full then/catch/finally chain behind
	// initJobControls plus the Svelte render flush (no onMount kick here,
	// unlike the merge-chip island).
	await new Promise((resolve) => setTimeout(resolve, 0));
	await new Promise((resolve) => setTimeout(resolve, 0));
}

function getMaxConcurrentSelect(): HTMLSelectElement {
	const select = document.getElementById('max-concurrent-select') as HTMLSelectElement | null;
	if (!select) {
		throw new Error('Expected max-concurrent select to be mounted');
	}
	return select;
}

function getMaxConcurrentIndicator(): HTMLElement {
	const indicator = document.getElementById('max-concurrent-effective');
	if (!indicator) {
		throw new Error('Expected max-concurrent indicator to be mounted');
	}
	return indicator;
}

function appSettingsFixture() {
	return {
		maxConcurrentJobs: { mode: 'auto' as const },
		encoderDefaults: {
			settings: {
				encoderType: 'auto' as const,
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr' as const, value: 3 },
				channels: 'auto' as const,
				afterburner: true,
			},
			sampleRate: 'auto' as const,
		},
		outputDefaults: {
			outputNaming: {
				preset: 'absDefault' as const,
				includeYear: false,
				customTemplate: undefined,
			},
			outputDirectory: undefined,
		},
		toolchain: {},
		startupBehavior: 'rememberLastState' as const,
		density: 'comfortable' as const,
		editSurface: 'rail' as const,
	};
}

describe('App Settings concurrency control', () => {
	beforeEach(() => {
		setMaxConcurrentJobsMock.mockReset();
		getRuntimeSettingsCapabilitiesMock.mockReset();
		getRuntimeSettingsCapabilitiesMock.mockResolvedValue(runtimeSettingsCapabilitiesFixture());
		updateAppSettingsMock.mockReset();
		updateAppSettingsMock.mockResolvedValue(appSettingsFixture());
		getMaxConcurrentJobsMock.mockReset();
		getMaxConcurrentJobsMock.mockResolvedValue(4);
		setJobControlsEnabled(true);
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.maxConcurrentCapabilities = null;
		jobControlsState.effectiveMaxConcurrent = null;
		jobControlsState.effectiveLabel = '';
		setRuntimeSettingsCapabilities(null);
		runtimeSettingsCapabilitiesState.loading = false;
		render(AppSettingsDialogIsland);
	});

	it('renders backend-owned fixed concurrency options', async () => {
		initJobControls();
		await flushAsync();

		const values = Array.from(getMaxConcurrentSelect().options).map((option) => option.value);

		expect(values).toEqual(['auto', '1', '2', '3', '4', '5', '6', '7', '8']);
	});

	it('routes selection changes through the job controls strip', async () => {
		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();

		const select = getMaxConcurrentSelect();
		select.value = '3';
		updateAppSettingsMock.mockResolvedValueOnce({
			...appSettingsFixture(),
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
		getMaxConcurrentJobsMock.mockResolvedValueOnce(3);
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushAsync();

		expect(setMaxConcurrentJobsMock).not.toHaveBeenCalled();
		expect(updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
	});

	it('applies a hydrated fixed max concurrency preference and updates the indicator', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(3);

		await applyMaxConcurrentPreference(
			{ mode: 'fixed', value: 3 },
			runtimeSettingsCapabilitiesFixture().maxConcurrentJobs,
		);
		await flushAsync();

		expect(getMaxConcurrentSelect().value).toBe('3');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
		expect(getMaxConcurrentIndicator().textContent).toBe('Max 3');
	});

	it('keeps the in-memory selection and pushes the new auto payload', async () => {
		jobControlsState.maxConcurrentSelection = '2';

		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();
		getMaxConcurrentJobsMock.mockClear();

		const select = getMaxConcurrentSelect();
		select.value = 'auto';
		getMaxConcurrentJobsMock.mockResolvedValueOnce(6);
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushAsync();

		expect(setMaxConcurrentJobsMock).not.toHaveBeenCalled();
		expect(updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'auto' },
		});
		expect(getMaxConcurrentJobsMock).toHaveBeenCalledTimes(1);
		expect(getMaxConcurrentIndicator().textContent).toBe('Auto → 6');
	});

	it('rolls back the visible concurrency selection when backend acceptance fails', async () => {
		jobControlsState.maxConcurrentCapabilities =
			runtimeSettingsCapabilitiesFixture().maxConcurrentJobs;
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.effectiveMaxConcurrent = 4;
		jobControlsState.effectiveLabel = 'Auto → 4';
		await flushAsync();
		updateAppSettingsMock.mockRejectedValueOnce(new Error('jobs active'));
		getMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		getMaxConcurrentJobsMock.mockClear();

		const select = getMaxConcurrentSelect();
		select.value = '3';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushAsync();

		expect(updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
		expect(getMaxConcurrentJobsMock).toHaveBeenCalledTimes(1);
		expect(jobControlsState.maxConcurrentSelection).toBe('auto');
		expect(getMaxConcurrentIndicator().textContent).toBe('Auto → 4');
	});

	it('keeps accepted concurrency selection when accepted follow-up read fails', async () => {
		jobControlsState.maxConcurrentCapabilities =
			runtimeSettingsCapabilitiesFixture().maxConcurrentJobs;
		await flushAsync();
		updateAppSettingsMock.mockResolvedValueOnce({
			...appSettingsFixture(),
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
		getMaxConcurrentJobsMock.mockRejectedValueOnce(new Error('read failed'));
		getMaxConcurrentJobsMock.mockClear();

		const select = getMaxConcurrentSelect();
		select.value = '3';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushAsync();

		expect(updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
		expect(getMaxConcurrentJobsMock).toHaveBeenCalledTimes(1);
		expect(jobControlsState.maxConcurrentSelection).toBe('3');
		expect(getMaxConcurrentIndicator().textContent).toBe('Max 3');
	});

	it('locks the select while job controls are disabled', async () => {
		initJobControls();
		await flushAsync();

		setJobControlsEnabled(false);
		await flushAsync();
		expect(getMaxConcurrentSelect().disabled).toBe(true);

		setJobControlsEnabled(true);
		await flushAsync();
		expect(getMaxConcurrentSelect().disabled).toBe(false);
	});
});
