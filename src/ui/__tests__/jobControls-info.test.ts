import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../jobControls/JobControlsIsland.svelte';
import {
	applyMaxConcurrentPreference,
	getJobType,
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
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

const mockedDependencies = vi.hoisted(() => ({
	updateOutputPathMock: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	updateOutputPath: mockedDependencies.updateOutputPathMock,
}));

const setMaxConcurrentJobsMock = vi.mocked(tauriClient.setMaxConcurrentJobs);
const updateAppSettingsMock = vi.mocked(tauriClient.updateAppSettings);
const getMaxConcurrentJobsMock = vi.mocked(tauriClient.getMaxConcurrentJobs);
const getRuntimeSettingsCapabilitiesMock = vi.mocked(tauriClient.getRuntimeSettingsCapabilities);

function setupDomRoot() {
	render(JobControlsIsland, {
		onMergeModeChange: handleMergeModeChange,
		onMaxConcurrentSelectionChange: handleMaxConcurrentSelectionChange,
	});
}

async function flushAsync() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

function getMergeToggle(): HTMLInputElement {
	const toggle = document.getElementById('merge-mode-toggle') as HTMLInputElement | null;
	if (!toggle) {
		throw new Error('Expected merge toggle to be mounted');
	}
	return toggle;
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
	};
}

describe('Job controls merge toggle', () => {
	beforeEach(() => {
		setMaxConcurrentJobsMock.mockReset();
		getRuntimeSettingsCapabilitiesMock.mockReset();
		getRuntimeSettingsCapabilitiesMock.mockResolvedValue(runtimeSettingsCapabilitiesFixture());
		updateAppSettingsMock.mockReset();
		updateAppSettingsMock.mockResolvedValue(appSettingsFixture());
		getMaxConcurrentJobsMock.mockReset();
		getMaxConcurrentJobsMock.mockResolvedValue(4);
		mockedDependencies.updateOutputPathMock.mockReset();
		setJobTypeSelection('batch');
		setJobControlsEnabled(true);
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.maxConcurrentCapabilities = null;
		jobControlsState.effectiveMaxConcurrent = null;
		jobControlsState.effectiveLabel = '';
		setRuntimeSettingsCapabilities(null);
		runtimeSettingsCapabilitiesState.loading = false;
		setupDomRoot();
	});

	it('renders backend-owned fixed concurrency options', async () => {
		initJobControls();
		await flushAsync();

		const values = Array.from(getMaxConcurrentSelect().options).map((option) => option.value);

		expect(values).toEqual(['auto', '1', '2', '3', '4', '5', '6', '7', '8']);
	});

	it('updates job type and refreshes output preview when merge mode changes', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		initJobControls();
		await flushAsync();

		const toggle = getMergeToggle();
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change', { bubbles: true }));

		expect(getJobType()).toBe('merge');
		expect(mockedDependencies.updateOutputPathMock).toHaveBeenCalledTimes(1);
	});

	it('does not duplicate side effects when initialized twice on same DOM', async () => {
		initJobControls();
		await flushAsync();
		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();
		mockedDependencies.updateOutputPathMock.mockClear();

		const toggle = getMergeToggle();
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change', { bubbles: true }));

		const select = getMaxConcurrentSelect();
		select.value = '3';
		updateAppSettingsMock.mockResolvedValueOnce({
			...appSettingsFixture(),
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
		getMaxConcurrentJobsMock.mockResolvedValueOnce(3);
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushAsync();

		expect(mockedDependencies.updateOutputPathMock).toHaveBeenCalledTimes(1);
		expect(setMaxConcurrentJobsMock).not.toHaveBeenCalled();
		expect(updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
	});

	it('applies a hydrated fixed max concurrency preference and updates the toolbar indicator', async () => {
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
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.effectiveMaxConcurrent = 4;
		jobControlsState.effectiveLabel = 'Auto → 4';
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

	it('toggles disabled state and opacity on both controls', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		initJobControls();
		await flushAsync();

		const mergeToggle = getMergeToggle();
		const maxConcurrentSelect = getMaxConcurrentSelect();

		setJobControlsEnabled(false);
		await flushAsync();
		expect(mergeToggle.disabled).toBe(true);
		expect(mergeToggle.style.opacity).toBe('0.5');
		expect(maxConcurrentSelect.disabled).toBe(true);
		expect(maxConcurrentSelect.style.opacity).toBe('0.5');

		setJobControlsEnabled(true);
		await flushAsync();
		expect(mergeToggle.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
		expect(maxConcurrentSelect.disabled).toBe(false);
		expect(maxConcurrentSelect.style.opacity).toBe('1');
	});
});
