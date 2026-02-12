import { beforeEach, describe, expect, it, vi } from 'vitest';
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

		(panel as any).jobProgress.set('idx:0', {
			inputIndex: 0,
			label: 'alpha.m4b',
			status: 'processing',
			percentage: 25,
			message: 'Working',
			lastUpdate: Date.now(),
		});
		(panel as any).queueOrder = ['idx:0'];
		(panel as any).lastProgressRenderByKey.set('idx:0', Date.now());
		(panel as any).isProcessing = true;

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

		expect((panel as any).jobProgress.size).toBe(0);
		expect((panel as any).queueOrder).toEqual([]);
		expect((panel as any).lastProgressRenderByKey.size).toBe(0);
		expect((panel as any).batchCompletionTimeout).toBeUndefined();
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

		clearTimeoutSpy.mockRestore();
	});
});
