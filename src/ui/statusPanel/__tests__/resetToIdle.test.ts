import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as dom from '../dom';
import { StatusPanel } from '../logic';

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
    <input id="merge-mode-toggle" type="checkbox" />
    <select id="max-concurrent-select"></select>
  `;
}

describe('StatusPanel resetToIdle', () => {
	beforeEach(() => {
		setupDom();
		dom.resetStatusPanelDomCache();
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('clears state/timers and re-enables controls', () => {
		const panel = new StatusPanel();
		const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout');

		const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLInputElement;
		const maxConcurrent = document.getElementById('max-concurrent-select') as HTMLSelectElement;
		mergeToggle.disabled = true;
		mergeToggle.style.opacity = '0.5';
		maxConcurrent.disabled = true;
		maxConcurrent.style.opacity = '0.5';

		(panel as any).handleQueueSnapshot({
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		});
		panel.updateProgress({
			input_index: 0,
			stage: 'converting',
			percentage: 25,
			message: 'Working',
		} as any);
		vi.advanceTimersByTime(20);

		const progressUnlisten = vi.fn();
		const queueUnlisten = vi.fn();
		(panel as any).progressUnlisten = progressUnlisten;
		(panel as any).queueUnlisten = queueUnlisten;

		const timeoutId = window.setTimeout(() => {}, 10_000);
		(panel as any).batchCompletionTimeout = timeoutId;

		(panel as any).resetToIdle();

		expect(progressUnlisten).toHaveBeenCalledTimes(1);
		expect(queueUnlisten).toHaveBeenCalledTimes(1);
		expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutId);

		expect((document.getElementById('job-list') as HTMLElement).childElementCount).toBe(0);
		expect((document.getElementById('status-text') as HTMLElement).textContent).toBe('Idle');
		expect((document.getElementById('step-text') as HTMLElement).textContent).toBe(
			'Current Step: Ready to process audiobook',
		);
		expect((document.getElementById('percentage-processed') as HTMLElement).textContent).toBe(
			'0.0%',
		);
		expect((document.querySelector('.art-thumbnail') as HTMLElement).textContent).toBe('Art');
		expect(panel.isCurrentlyProcessing).toBe(false);
		expect(panel.getCurrentStatus()).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});

		expect(mergeToggle.disabled).toBe(false);
		expect(maxConcurrent.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
		expect(maxConcurrent.style.opacity).toBe('1');

		// Secondary internal check: cleanup maps are emptied.
		expect((panel as any).jobProgress.size).toBe(0);
		expect((panel as any).lastProgressRenderByKey.size).toBe(0);

		clearTimeoutSpy.mockRestore();
	});
});
