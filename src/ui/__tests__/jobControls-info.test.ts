import { describe, it, expect, beforeEach, vi } from 'vitest';
import { getJobType, initJobControls, setJobControlsEnabled } from '../jobControls';
import { bridge } from '../../lib/bridge';

vi.mock('../../lib/bridge', () => ({
	bridge: {
		setMaxConcurrentJobs: vi.fn().mockResolvedValue(undefined),
	},
}));

const MAX_CONCURRENT_STORAGE_KEY = 'abb:maxConcurrentJobs';
const setMaxConcurrentJobsMock = vi.mocked(bridge.setMaxConcurrentJobs);

function setupDomRoot() {
	document.body.innerHTML = '<div id="job-controls-root"></div>';
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
	});

	it('dispatches job-type change and reflects merge toggle state', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		initJobControls();
		await flushAsync();

		let fired = false;
		document.addEventListener(
			'abb:job-type-changed',
			() => {
				fired = true;
			},
			{ once: true },
		);

		const toggle = getMergeToggle();
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));

		expect(fired).toBe(true);
		expect(getJobType()).toBe('merge');
	});

	it('does not duplicate listeners when initialized twice on same DOM', async () => {
		initJobControls();
		await flushAsync();
		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();

		let jobTypeChangedCount = 0;
		document.addEventListener('abb:job-type-changed', () => {
			jobTypeChangedCount += 1;
		});

		const toggle = getMergeToggle();
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));

		const select = getMaxConcurrentSelect();
		select.value = '3';
		select.dispatchEvent(new Event('change'));
		await flushAsync();

		expect(jobTypeChangedCount).toBe(1);
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledTimes(1);
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
	});

	it('reads persisted max concurrency, sends numeric payload, and emits detail + indicator', async () => {
		localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, '3');
		const getItemSpy = vi.spyOn(localStorage, 'getItem');
		setMaxConcurrentJobsMock.mockResolvedValueOnce(3);

		let detail: { effective: number; selection: string } | null = null;
		document.addEventListener(
			'abb:max-concurrent-updated',
			(event) => {
				detail = (event as CustomEvent<{ effective: number; selection: string }>).detail;
			},
			{ once: true },
		);

		initJobControls();
		await flushAsync();

		expect(getItemSpy).toHaveBeenCalledWith(MAX_CONCURRENT_STORAGE_KEY);
		expect(getMaxConcurrentSelect().value).toBe('3');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(3);
		expect(getMaxConcurrentIndicator().textContent).toBe('Max 3');
		expect(detail).toEqual({ effective: 3, selection: '3' });
	});

	it('writes updated selection, sends auto payload, and updates indicator + event detail', async () => {
		localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, '2');
		const setItemSpy = vi.spyOn(localStorage, 'setItem');
		setMaxConcurrentJobsMock.mockResolvedValueOnce(2).mockResolvedValueOnce(6);

		initJobControls();
		await flushAsync();
		setMaxConcurrentJobsMock.mockClear();

		let detail: { effective: number; selection: string } | null = null;
		document.addEventListener(
			'abb:max-concurrent-updated',
			(event) => {
				detail = (event as CustomEvent<{ effective: number; selection: string }>).detail;
			},
			{ once: true },
		);

		const select = getMaxConcurrentSelect();
		select.value = 'auto';
		select.dispatchEvent(new Event('change'));
		await flushAsync();

		expect(setItemSpy).toHaveBeenCalledWith(MAX_CONCURRENT_STORAGE_KEY, 'auto');
		expect(setMaxConcurrentJobsMock).toHaveBeenCalledWith(null);
		expect(getMaxConcurrentIndicator().textContent).toBe('Auto → 6');
		expect(detail).toEqual({ effective: 6, selection: 'auto' });
	});

	it('toggles disabled state and opacity on both controls', async () => {
		setMaxConcurrentJobsMock.mockResolvedValueOnce(4);
		initJobControls();
		await flushAsync();

		const mergeToggle = getMergeToggle();
		const maxConcurrentSelect = getMaxConcurrentSelect();

		setJobControlsEnabled(false);
		expect(mergeToggle.disabled).toBe(true);
		expect(mergeToggle.style.opacity).toBe('0.5');
		expect(maxConcurrentSelect.disabled).toBe(true);
		expect(maxConcurrentSelect.style.opacity).toBe('0.5');

		setJobControlsEnabled(true);
		expect(mergeToggle.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
		expect(maxConcurrentSelect.disabled).toBe(false);
		expect(maxConcurrentSelect.style.opacity).toBe('1');
	});
});
