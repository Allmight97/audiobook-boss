import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../jobControls/JobControlsIsland.svelte';
import {
	getJobType,
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../jobControls';
import { tauriClient } from '../../lib/tauri/client';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		setMaxConcurrentJobs: vi.fn().mockResolvedValue(undefined),
	},
}));

const mockedDependencies = vi.hoisted(() => ({
	updateOutputPathMock: vi.fn(),
	setStatusPanelConcurrencyTextMock: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	updateOutputPath: mockedDependencies.updateOutputPathMock,
}));

vi.mock('../statusPanel/viewState.svelte', () => ({
	setStatusPanelConcurrencyText: mockedDependencies.setStatusPanelConcurrencyTextMock,
}));

const MAX_CONCURRENT_STORAGE_KEY = 'abb:maxConcurrentJobs';
const setMaxConcurrentJobsMock = vi.mocked(tauriClient.setMaxConcurrentJobs);

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

describe('Job controls merge toggle', () => {
	beforeEach(() => {
		setupDomRoot();
		localStorage.clear();
		setMaxConcurrentJobsMock.mockReset();
		mockedDependencies.updateOutputPathMock.mockReset();
		mockedDependencies.setStatusPanelConcurrencyTextMock.mockReset();
		setJobTypeSelection('batch');
		setJobControlsEnabled(true);
	});

	it('updates job type and refreshes output preview when merge mode changes', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		initJobControls();
		await flushAsync();

		const toggle = getMergeToggle();
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));

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
		toggle.dispatchEvent(new Event('change'));

		const select = getMaxConcurrentSelect();
		select.value = '3';
		select.dispatchEvent(new Event('change'));
		await flushAsync();

		expect(mockedDependencies.updateOutputPathMock).toHaveBeenCalledTimes(1);
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledTimes(1);
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
	});

	it('reads persisted max concurrency, sends numeric payload, and updates indicator + status text', async () => {
		localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, '3');
		const getItemSpy = vi.spyOn(localStorage, 'getItem');
		setMaxConcurrentJobsMock.mockResolvedValueOnce(3);

		initJobControls();
		await flushAsync();

		expect(getItemSpy).toHaveBeenCalledWith(MAX_CONCURRENT_STORAGE_KEY);
		expect(getMaxConcurrentSelect().value).toBe('3');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
		expect(getMaxConcurrentIndicator().textContent).toBe('Max 3');
		expect(mockedDependencies.setStatusPanelConcurrencyTextMock).toHaveBeenLastCalledWith(
			'Max jobs: 3',
		);
	});

	it('writes updated selection, sends auto payload, and updates indicator + status text', async () => {
		localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, '2');
		const setItemSpy = vi.spyOn(localStorage, 'setItem');
		setMaxConcurrentJobsMock.mockResolvedValueOnce(2).mockResolvedValueOnce(6);

		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();

		const select = getMaxConcurrentSelect();
		select.value = 'auto';
		select.dispatchEvent(new Event('change'));
		await flushAsync();

		expect(setItemSpy).toHaveBeenCalledWith(MAX_CONCURRENT_STORAGE_KEY, 'auto');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(null);
		expect(getMaxConcurrentIndicator().textContent).toBe('Auto → 6');
		expect(mockedDependencies.setStatusPanelConcurrencyTextMock).toHaveBeenLastCalledWith(
			'Max jobs: 6 (Auto)',
		);
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
