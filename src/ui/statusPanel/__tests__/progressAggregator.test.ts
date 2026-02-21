import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StatusPanel } from '../logic';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

function setupDom() {
	document.body.innerHTML = `
    <div id="progress-bar"></div>
    <div id="percentage-processed"></div>
    <div id="status-text"></div>
    <div id="step-text"></div>
    <div id="concurrency-status"></div>
    <button id="process-button"></button>
    <button id="cancel-all-button"></button>
    <div class="art-thumbnail"></div>
    <div id="job-list"></div>
    <select id="max-concurrent-select"></select>
  `;
}

function getJobRows(): string[] {
	return statusPanelViewState.jobItems.map((item) => {
		const percentage =
			typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	});
}

describe('StatusPanel aggregate progress', () => {
	beforeEach(() => {
		setupDom();
		resetStatusPanelViewState();
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('computes simple averages across active and completed jobs', () => {
		const panel = new StatusPanel();
		(panel as any).handleQueueSnapshot({
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		panel.updateProgress({
			input_index: 0,
			stage: 'converting',
			percentage: 50,
			message: 'Halfway',
		} as any);
		panel.updateProgress({
			input_index: 1,
			stage: 'completed',
			percentage: 100,
			message: 'Done',
		} as any);
		vi.advanceTimersByTime(20);

		expect(panel.getCurrentStatus()).toEqual({
			stage: 'converting',
			percentage: 75,
			message: 'Halfway',
		});
		expect(statusPanelViewState.progressPercentage).toBe(75);
		expect(statusPanelViewState.statusText).toBe('Converting');
		expect(getJobRows()).toEqual([
			'alpha.m4b • Converting (50.0%)',
			'beta.m4b • Completed (100.0%)',
		]);
	});
});
