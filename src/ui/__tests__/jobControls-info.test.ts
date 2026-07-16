import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../jobControls/JobControlsIsland.svelte';
import {
	getJobType,
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

const getMaxConcurrentJobsMock = vi.mocked(tauriClient.getMaxConcurrentJobs);
const getRuntimeSettingsCapabilitiesMock = vi.mocked(tauriClient.getRuntimeSettingsCapabilities);

type RerenderProps = { fileCount?: number };
let rerenderIsland: (props: RerenderProps) => Promise<void>;

function setupDomRoot(fileCount = 0) {
	const { rerender } = render(JobControlsIsland, {
		fileCount,
		onMergeModeChange: handleMergeModeChange,
	});
	rerenderIsland = rerender;
}

async function flushAsync() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

function getMergeToggle(): HTMLButtonElement {
	const toggle = document.getElementById('merge-mode-toggle') as HTMLButtonElement | null;
	if (!toggle) {
		throw new Error('Expected merge toggle to be mounted');
	}
	return toggle;
}

describe('Job controls merge toggle chip', () => {
	beforeEach(() => {
		getRuntimeSettingsCapabilitiesMock.mockReset();
		getRuntimeSettingsCapabilitiesMock.mockResolvedValue(runtimeSettingsCapabilitiesFixture());
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

	it('toggles job type and refreshes output preview when the chip is clicked', async () => {
		initJobControls();
		await flushAsync();

		const toggle = getMergeToggle();
		expect(toggle.getAttribute('aria-pressed')).toBe('false');

		toggle.click();
		await flushAsync();

		expect(getJobType()).toBe('merge');
		expect(toggle.getAttribute('aria-pressed')).toBe('true');
		expect(mockedDependencies.updateOutputPathMock).toHaveBeenCalledTimes(1);

		toggle.click();
		await flushAsync();

		expect(getJobType()).toBe('batch');
		expect(toggle.getAttribute('aria-pressed')).toBe('false');
	});

	it('does not duplicate side effects when initialized twice on same DOM', async () => {
		initJobControls();
		await flushAsync();
		initJobControls();
		await flushAsync();
		mockedDependencies.updateOutputPathMock.mockClear();

		getMergeToggle().click();
		await flushAsync();

		expect(mockedDependencies.updateOutputPathMock).toHaveBeenCalledTimes(1);
	});

	it('reflects the live file count in the on-state label', async () => {
		initJobControls();
		await flushAsync();

		const toggle = getMergeToggle();
		expect(toggle.textContent?.trim()).toBe('merge off');

		toggle.click();
		await flushAsync();
		expect(toggle.textContent?.trim()).toBe('merge — 0 files → one M4B');

		await rerenderIsland({ fileCount: 3 });
		expect(getMergeToggle().textContent?.trim()).toBe('merge — 3 files → one M4B');

		await rerenderIsland({ fileCount: 1 });
		expect(getMergeToggle().textContent?.trim()).toBe('merge — 1 file → one M4B');
	});

	it('toggles disabled state and opacity on the chip', async () => {
		initJobControls();
		await flushAsync();

		const mergeToggle = getMergeToggle();

		setJobControlsEnabled(false);
		await flushAsync();
		expect(mergeToggle.disabled).toBe(true);
		expect(mergeToggle.style.opacity).toBe('0.5');

		setJobControlsEnabled(true);
		await flushAsync();
		expect(mergeToggle.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
	});
});
