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
import { tauriClient } from '../../lib/tauri/client';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
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
	updateStatusPanelConcurrencyStatusMock: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	updateOutputPath: mockedDependencies.updateOutputPathMock,
}));

vi.mock('../statusPanel', () => ({
	updateStatusPanelConcurrencyStatus: mockedDependencies.updateStatusPanelConcurrencyStatusMock,
}));

const setMaxConcurrentJobsMock = vi.mocked(tauriClient.setMaxConcurrentJobs);
const updateAppSettingsMock = vi.mocked(tauriClient.updateAppSettings);
const getMaxConcurrentJobsMock = vi.mocked(tauriClient.getMaxConcurrentJobs);

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
				threads: { mode: 'auto' as const },
				twoloop: true,
			},
			sampleRate: 'auto' as const,
			externalToolchain: {},
		},
		outputDefaults: {
			outputNaming: {
				preset: 'absDefault' as const,
				includeYear: false,
				customTemplate: undefined,
			},
			outputDirectory: undefined,
		},
	};
}

describe('Job controls merge toggle', () => {
	beforeEach(() => {
		setupDomRoot();
		setMaxConcurrentJobsMock.mockReset();
		updateAppSettingsMock.mockReset();
		updateAppSettingsMock.mockResolvedValue(appSettingsFixture());
		getMaxConcurrentJobsMock.mockReset();
		getMaxConcurrentJobsMock.mockResolvedValue(4);
		mockedDependencies.updateOutputPathMock.mockReset();
		mockedDependencies.updateStatusPanelConcurrencyStatusMock.mockReset();
		setJobTypeSelection('batch');
		setJobControlsEnabled(true);
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.effectiveMaxConcurrent = null;
		jobControlsState.effectiveLabel = '';
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

	it('applies a hydrated fixed max concurrency preference and updates indicator + status text', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(3);

		await applyMaxConcurrentPreference({ mode: 'fixed', value: 3 });
		await flushAsync();

		expect(getMaxConcurrentSelect().value).toBe('3');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
		expect(getMaxConcurrentIndicator().textContent).toBe('Max 3');
		expect(mockedDependencies.updateStatusPanelConcurrencyStatusMock).toHaveBeenLastCalledWith(
			'Max jobs: 3',
		);
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
		expect(mockedDependencies.updateStatusPanelConcurrencyStatusMock).toHaveBeenLastCalledWith(
			'Max jobs: 6 (Auto)',
		);
	});

	it('rolls back the visible concurrency selection when backend acceptance fails', async () => {
		jobControlsState.maxConcurrentSelection = 'auto';
		jobControlsState.effectiveMaxConcurrent = 4;
		jobControlsState.effectiveLabel = 'Auto → 4';
		updateAppSettingsMock.mockRejectedValueOnce(new Error('jobs active'));
		getMaxConcurrentJobsMock.mockResolvedValueOnce(4);

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
